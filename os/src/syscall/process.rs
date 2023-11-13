//! Process management syscalls
//!
use core::{slice::from_raw_parts, mem::size_of};
use alloc::sync::Arc;

use crate::{
    config::MAX_SYSCALL_NUM,
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str, VirtAddr, MapPermission, translated_byte_buffer},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,get_syscall_times, get_first_execute_time, TaskControlBlock,
    }, timer::{get_time_ms, get_time_us},
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

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}
/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
/// 功能：新建子进程，使其执行目标程序。
/// 说明：成功返回子进程id，否则返回 -1。
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let current_task = current_task().unwrap();
        let all_data = app_inode.read_all();
        let new_task = Arc::new(TaskControlBlock::new(all_data.as_slice()));  
        let new_task_id = new_task.getpid(); 
        new_task.inner_exclusive_access().parent = Some(Arc::downgrade(&current_task));
        current_task.inner_exclusive_access().children.push(new_task.clone());
        add_task(new_task);
        new_task_id as isize
    } else {
        return -1;
    }
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        // 将exit_code保存在exit_code_ptr中
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        // 如果存在满足条件的子进程，但还没有结束，则返回
        -2
    }
    // ---- release current PCB automatically
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
    if port & 0b1 != 0{
        map_permission.insert(MapPermission::R);
    }
    if port & 0b10 != 0{
        map_permission.insert(MapPermission::W);
    }
    if port & 0b100 != 0{
        map_permission.insert(MapPermission::X);
    }     
    if len == 0 {
        return 0;
    }
    let result = current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .insert_framed_area(
            start.into(),
            (start + len).into(),
            map_permission,
        );
    if result.is_err() {
        return -1;
    }
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if VirtAddr::from(start).page_offset() != 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }
    let result = current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .unmap_area(start.into(), (start + len).into());
    if result.is_err() {
        return -1;
    }
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}


// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    if prio < 2 {
        return -1;
    }
    current_task().unwrap().inner_exclusive_access().priority = prio as usize;
    prio
}
