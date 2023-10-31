# lab1-李国玮

## 编程作业

为了实现对单个任务的执行信息跟踪，我首先修改了任务控制块TaskControlBlock，增加对系统调用次数和第一次被调用时间的记录。通过在TaskManager中增加相应的函数逻辑就可以将其暴露给系统调用模块。

对于系统调用次数的记录，我在syscall函数的一开始就根据`syscall_id`对当前任务控制块中的系统调用次数数组进行修改。

对于当前运行时间，我则通过使用`get_time_ms`函数获取。

## 简答作业

### 1

在os目录下执行 `make run BASE=1 CHAPTER=2 LOG=ERROR`时，会执行ch2b开头的应用程序，可以观察到内核的报错依次为：

```
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003c4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```

即访问0地址出错，执行非法指令出错。sbi为：

```
RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0
```

### 2

1. 刚进入`__restore`时，`a0`代表内核栈栈顶。其两种使用场景分别是：
   * 当系统调用返回到用户态时，restore上下文
   * 当执行用户程序时，从S态变为U态，并初始化寄存器
2. 特殊处理了sstatus、sepc、sscratch寄存器。sstatus指明了要返回U态、sepc指明了返回U态后取址地址、sscratch中则存放了用户栈栈顶
3. x2为sp寄存器，在之后会和sscratch交换，得到真正的用户栈栈顶，现在恢复的值没有意义；x4为一个没用的寄存器，不需要恢复
4. sp此时的值是用户栈栈顶，sscratch为内核栈栈顶
5. sret发生状态切换。该指令会将CPU当前特权级按sstatus中的SPP字段进行设置。
6. sp表示内核栈栈顶，sscratch表示用户栈栈顶
7. ecall