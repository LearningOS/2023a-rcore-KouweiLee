//! Process management syscalls
use core::{mem::size_of, slice::from_raw_parts};

use crate::{
    config::MAX_SYSCALL_NUM,
    mm::{translated_byte_buffer, MapPermission, VirtAddr},
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next, get_first_execute_time,
        get_syscall_times, get_task_manager, suspend_current_and_run_next, TaskStatus,
        get_current_task_id,
    },
    timer::{get_time_ms, get_time_us},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let t = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    copy_kernel_data(&t as *const TimeVal, ts);
    0
}

fn copy_kernel_data<T>(from: *const T, to: *mut T) {
    let from_buf = unsafe { from_raw_parts(from as *const u8, size_of::<T>()) };
    let from_len = from_buf.len();
    let mut start = 0;
    let buffers = translated_byte_buffer(current_user_token(), to as *const u8, size_of::<T>());
    for buf in buffers {
        buf.copy_from_slice(&from_buf[start..from_len.min(buf.len())]);
        start += buf.len();
    }
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let task_info = TaskInfo {
        status: TaskStatus::Running,
        syscall_times: get_syscall_times(),
        time: get_time_ms() - get_first_execute_time(),
    };
    copy_kernel_data(&task_info as *const TaskInfo, ti);
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    if VirtAddr::from(start).page_offset() != 0 {
        // error!("start is not aligned");
        return -1;
    }
    if port & !0x7 != 0 || port & 0x7 == 0 {
        // error!("port is error");
        return -1;
    } 
    let mut map_permission = MapPermission::U;
    if port & 0x1 == 1{
        map_permission.insert(MapPermission::R);
    }
    if port & 0x2 == 1{
        map_permission.insert(MapPermission::W);
    }
    if port & 0x4 == 1{
        map_permission.insert(MapPermission::X);
    }     

    if len == 0 {
        return 0;
    }
    let current_task = get_current_task_id();
    let task_manager = get_task_manager();
    let result = task_manager.inner.exclusive_access().tasks[current_task]
        .memory_set
        .insert_framed_area(
            start.into(),
            (start + len - 1).into(),
            map_permission,
        );
    if result.is_err() {
        return -1;
    }
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if VirtAddr::from(start).page_offset() != 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }
    let current_task = get_current_task_id();
    let task_manager = get_task_manager();
    let result = task_manager.inner.exclusive_access().tasks[current_task]
        .memory_set
        .unmap_area(start.into(), (start + len - 1).into());
    if result.is_err() {
        return -1;
    }
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
