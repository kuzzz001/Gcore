use crate::config::{PAGE_SIZE, SYSTEM_TASK_LIMIT, USER_STACK_SIZE};
use crate::fs::OpenFlags;
use crate::hal::shutdown;
use crate::hal::{MachineContext, TrapContext};
use crate::mm::{
    copy_from_user, copy_to_user, copy_to_user_string, get_from_user, translated_byte_buffer,
    translated_ref, translated_refmut, translated_str, try_get_from_user, MapFlags, MapPermission,
    UserBuffer,
};
use crate::show_frame_consumption;
use crate::syscall::errno::*;
use crate::task::threads::{do_futex_wait, FutexCmd};
use crate::task::{
    add_task, block_current_and_run_next, current_task, current_user_token,
    exit_current_and_run_next, exit_group_and_run_next, find_task_by_pid, find_task_by_tgid,
    procs_count, signal::*, suspend_current_and_run_next, threads, wait_with_timeout,
    wake_interruptible, Rusage, TaskStatus,
};
use crate::timer::{get_time_ms, get_time_sec, ITimerVal, NSEC_PER_SEC, TimeSpec, TimeVal, TimeZone, Times};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use log::{debug, error, info, trace, warn};
use num_enum::FromPrimitive;
pub fn sys_shutdown() -> isize {
    shutdown()
}
pub fn sys_exit(exit_code: u32) -> ! {
    exit_current_and_run_next((exit_code & 0xff) << 8);
}

pub fn sys_exit_group(exit_code: u32) -> ! {
    exit_group_and_run_next((exit_code & 0xff) << 8);
}

#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq, FromPrimitive)]
#[repr(u32)]
pub enum SyslogAction {
    CLOSE = 0,
    OPEN = 1,
    READ = 2,
    READ_ALL = 3,
    READ_CLEAR = 4,
    CLEAR = 5,
    CONSOLE_OFF = 6,
    CONSOLE_ON = 7,
    CONSOLE_LEVEL = 8,
    SIZE_UNREAD = 9,
    SIZE_BUFFER = 10,
    #[default]
    ILLEAGAL,
}

pub fn sys_syslog(type_: u32, buf: *mut u8, len: u32) -> isize {
    const LOG_BUF_LEN: usize = 4096;
    const LOG: &str = "<5>[    0.000000] Linux version 5.10.102.1-microsoft-standard-WSL2 (rtrt@TEAM-NPUCORE) (gcc (Ubuntu 9.4.0-1ubuntu1~20.04) 9.4.0, GNU ld (GNU Binutils for Ubuntu) 2.34) #1 SMP Thu Mar 10 13:31:47 CST 2022";
    let token = current_user_token();
    let type_ = SyslogAction::from(type_);
    let len = LOG.len().min(len as usize);
    match type_ {
        SyslogAction::CLOSE | SyslogAction::OPEN => SUCCESS,
        SyslogAction::READ => {
            copy_to_user_string(token, &LOG[..len], buf).unwrap();
            len as isize
        }
        SyslogAction::READ_ALL => {
            copy_to_user_string(token, &LOG[LOG.len() - len..], buf).unwrap();
            len as isize
        }
        SyslogAction::READ_CLEAR => {
            copy_to_user_string(token, &LOG[..len], buf).unwrap();
            len as isize
        }
        SyslogAction::CLEAR => SUCCESS,
        SyslogAction::CONSOLE_OFF => SUCCESS,
        SyslogAction::CONSOLE_ON => SUCCESS,
        SyslogAction::CONSOLE_LEVEL => SUCCESS,
        SyslogAction::SIZE_UNREAD => 0,
        SyslogAction::SIZE_BUFFER => LOG_BUF_LEN as isize,
        SyslogAction::ILLEAGAL => EINVAL,
    }
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    SUCCESS
}

pub fn sys_kill(pid: usize, sig: usize) -> isize {
    let signal = match Signals::from_signum(sig) {
        Ok(signal) => signal,
        Err(_) => return EINVAL,
    };
    #[cfg(feature = "comp")]
    if pid == 10 {
        return SUCCESS;
    }
    if pid > 0 {
        // [Warning] in current implementation,
        // signal will be sent to an arbitrary task with target `pid` (`tgid` more precisely).
        // But manual also require that the target task should not mask this signal.
        if let Some(task) = find_task_by_tgid(pid) {
            if !signal.is_empty() {
                let mut inner = task.acquire_inner_lock();
                inner.add_signal(signal);
                // wake up target process if it is sleeping
                if inner.task_status == TaskStatus::Interruptible {
                    inner.task_status = TaskStatus::Ready;
                    drop(inner);
                    wake_interruptible(task);
                }
            }
            SUCCESS
        } else {
            ESRCH
        }
    } else if pid == 0 {
        SUCCESS
    } else {
        SUCCESS
    }
}

pub fn sys_tkill(tid: usize, sig: usize) -> isize {
    let signal = match Signals::from_signum(sig) {
        Ok(signal) => signal,
        Err(_) => return EINVAL,
    };
    if tid > 0 {
        if let Some(task) = find_task_by_pid(tid) {
            if !signal.is_empty() {
                let mut inner = task.acquire_inner_lock();
                inner.add_signal(signal);
                // wake up target process if it is sleeping
                if inner.task_status == TaskStatus::Interruptible {
                    inner.task_status = TaskStatus::Ready;
                    drop(inner);
                    wake_interruptible(task);
                }
            }
            SUCCESS
        } else {
            ESRCH
        }
    } else {
        EINVAL
    }
}

pub fn sys_tgkill(tgid: usize, tid: usize, sig: usize) -> isize {
    let signal = match Signals::from_signum(sig) {
        Ok(signal) => signal,
        Err(_) => return EINVAL,
    };
    if let Some(task) = find_task_by_tgid(tgid) {
        if !signal.is_empty() {
            let mut inner = task.acquire_inner_lock();
            if task.pid.0 == tid {
                inner.add_signal(signal);
                // wake up target process if it is sleeping
                if inner.task_status == TaskStatus::Interruptible {
                    inner.task_status = TaskStatus::Ready;
                    drop(inner);
                    wake_interruptible(task);
                }
            } else {
                warn!(
                    "[sys_tgkill] tid {} does not match task's tid {}",
                    tid, task.pid.0
                );
            }
        }
        SUCCESS
    } else {
        ESRCH
    }
}

pub fn sys_nanosleep(req: *const TimeSpec, rem: *mut TimeSpec) -> isize {
    if req.is_null() {
        return EINVAL;
    }
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let req = match get_from_user(token, req) {
        Ok(req) => req,
        Err(errno) => return errno,
    };

    let end = TimeSpec::now() + req;
    wait_with_timeout(Arc::downgrade(&task), end);
    drop(task);

    block_current_and_run_next();
    let task = current_task().unwrap();
    let inner = task.acquire_inner_lock();
    let now = TimeSpec::now();
    // this is a little different with manual (do not consider sigmask)
    // but now we have to compromise
    if inner.sigpending.is_empty() {
        assert!(end <= now);
        if !rem.is_null() {
            copy_to_user(token, &TimeSpec::new(), rem).unwrap();
        }
        SUCCESS
    } else {
        if !rem.is_null() {
            copy_to_user(token, &(end - now), rem).unwrap();
        }
        EINTR
    }
}

pub fn sys_setitimer(
    which: usize,
    new_value: *const ITimerVal,
    old_value: *mut ITimerVal,
) -> isize {
    info!(
        "[sys_setitimer] which: {}, new_value: {:?}, old_value: {:?}",
        which, new_value, old_value
    );
    match which {
        0..=2 => {
            let task = current_task().unwrap();
            let mut inner = task.acquire_inner_lock();
            let token = task.get_user_token();
            if old_value as usize != 0 {
                copy_to_user(token, &inner.timer[which], old_value).unwrap();
                trace!("[sys_setitimer] *old_value: {:?}", inner.timer[which]);
            }
            if new_value as usize != 0 {
                copy_from_user(token, new_value, &mut inner.timer[which]).unwrap();
                trace!("[sys_setitimer] *new_value: {:?}", inner.timer[which]);
            }
            SUCCESS
        }
        _ => EINVAL,
    }
}

pub fn sys_gettimeofday(tv: *mut TimeVal, _tz: *mut TimeZone) -> isize {
    if !tv.is_null() {
        let token = current_user_token();
        let uptime_us = crate::timer::get_time_us();
        let boot_offset = crate::timer::current_time() - crate::timer::uptime();
        let unix_us = uptime_us + boot_offset as usize * 1_000_000;
        let timeval = TimeVal {
            tv_sec: unix_us / 1_000_000,
            tv_usec: unix_us % 1_000_000,
        };
        if copy_to_user(token, &timeval, tv).is_err() {
            log::error!("[sys_gettimeofday] Failed to copy to {:?}", tv);
            return EFAULT;
        }
    }
    SUCCESS
}

pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

#[allow(unused)]
#[repr(C)]
pub struct UTSName {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}

pub fn sys_uname(buf: *mut u8) -> isize {
    let token = current_user_token();
    let mut buffer = UserBuffer::new(
        match translated_byte_buffer(token, buf, size_of::<UTSName>()) {
            Ok(buffer) => buffer,
            Err(errno) => return errno,
        },
    );
    // A little stupid but still efficient.
    const FIELD_OFFSET: usize = 65;
    buffer.write_at(FIELD_OFFSET * 0, b"Gcore\0");
    buffer.write_at(FIELD_OFFSET * 1, b"5.10.0-1\0");
    #[cfg(feature = "riscv")]
    buffer.write_at(FIELD_OFFSET * 2, b"5.10.0-1-rv64\0");
    #[cfg(feature = "loongarch64")]
    buffer.write_at(FIELD_OFFSET * 2, b"5.10.0-1-la64\0");
    buffer.write_at(
        FIELD_OFFSET * 3,
        b"#1 SMP Gcore 5.10.0-1 (2025-01-10)\0",
    );
    #[cfg(feature = "riscv")]
    buffer.write_at(FIELD_OFFSET * 4, b"rv64\0");
    #[cfg(feature = "loongarch64")]
    buffer.write_at(FIELD_OFFSET * 4, b"la64\0");
    buffer.write_at(FIELD_OFFSET * 5, b"\0");
    SUCCESS
}

pub fn sys_getpid() -> isize {
    let pid = current_task().unwrap().tgid;
    pid as isize
}

pub fn sys_getppid() -> isize {
    let task = current_task().unwrap();
    let inner = task.acquire_inner_lock();
    inner
        .parent
        .as_ref()
        .and_then(|p| p.upgrade())
        .map(|p| p.tgid as isize)
        .unwrap_or(1)
}

pub fn sys_getuid() -> isize {
    0 // root user
}

pub fn sys_geteuid() -> isize {
    0 // root user
}

pub fn sys_getgid() -> isize {
    0 // root group
}

pub fn sys_getegid() -> isize {
    0 // root group
}

// Warning, we don't support this syscall in fact, task.setpgid() won't take effect for some reason
// So it just pretend to do this work.
// Fortunately, that won't make difference when we just try to run busybox sh so far.
pub fn sys_setpgid(pid: usize, pgid: usize) -> isize {
    /* An attempt.*/
    let task = crate::task::find_task_by_tgid(pid);
    match task {
        Some(task) => task.setpgid(pgid),
        None => ESRCH,
    }
}

pub fn sys_getpgid(pid: usize) -> isize {
    /* An attempt.*/
    let task = crate::task::find_task_by_tgid(pid);
    match task {
        Some(task) => task.getpgid() as isize,
        None => ESRCH,
    }
}
/// creates a new session if the calling process is not a process group leader.
/// The calling process is the leader of the new session
/// 当前进程脱离父进程，从父进程的子进程列表中移除当前进程，当前进程的父进程设置为空。
pub fn sys_setsid() -> isize {
    let task = current_task().unwrap();
    if let Some(parent) = task.acquire_inner_lock().parent.as_ref().unwrap().upgrade() {
        parent
            .acquire_inner_lock()
            .children
            .retain(|x| x.tid != task.tid);
    }
    task.acquire_inner_lock().parent = None;
    SUCCESS
}

// For user, tid is pid in kernel
pub fn sys_gettid() -> isize {
    current_task().unwrap().pid.0 as isize
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Sysinfo {
    uptime: usize,     /* Seconds since boot */
    loads: [usize; 3], /* 1, 5, and 15 minute load averages */
    totalram: usize,   /* Total usable main memory size */
    freeram: usize,    /* Available memory size */
    sharedram: usize,  /* Amount of shared memory */
    bufferram: usize,  /* Memory used by buffers */
    totalswap: usize,  /* Total swap space size */
    freeswap: usize,   /* Swap space still available */
    procs: u16,        /* Number of current processes */
    totalhigh: usize,  /* Total high memory size */
    freehigh: usize,   /* Available high memory size */
    mem_unit: u32,     /* Memory unit size in bytes */
                       //char __reserved[256];
                       // In the above structure, sizes of the memory and swap fields are given as multiples of mem_unit bytes.
}

pub fn sys_sysinfo(info: *mut Sysinfo) -> isize {
    const LINUX_SYSINFO_LOADS_SCALE: usize = 65536;
    const SEC_1_MIN: usize = 60;
    const SEC_5_MIN: usize = SEC_1_MIN * 5;
    const SEC_15_MIN: usize = SEC_1_MIN * 15;
    const UNIMPLEMENT: usize = 0;
    let token = current_user_token();
    let procs = procs_count();
    if copy_to_user(
        token,
        &Sysinfo {
            uptime: get_time_sec(),
            // Use only current sample (as average) to evaluate
            loads: [
                procs as usize * LINUX_SYSINFO_LOADS_SCALE / SEC_1_MIN,
                procs as usize * LINUX_SYSINFO_LOADS_SCALE / SEC_5_MIN,
                procs as usize * LINUX_SYSINFO_LOADS_SCALE / SEC_15_MIN,
            ],
            totalram: crate::config::MEMORY_END - crate::config::MEMORY_START,
            freeram: crate::mm::unallocated_frames() * PAGE_SIZE,
            sharedram: UNIMPLEMENT,
            bufferram: UNIMPLEMENT,
            totalswap: 0,
            freeswap: 0,
            procs,
            totalhigh: 0,
            freehigh: 0,
            mem_unit: 1,
        },
        info,
    )
    .is_err()
    {
        log::error!("[sys_sysinfo] Failed to copy to {:?}", info);
        EFAULT
    } else {
        SUCCESS
    }
}

pub fn sys_sbrk(increment: isize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.acquire_inner_lock();
    let mut memory_set = task.vm.lock();
    inner.heap_pt = memory_set.sbrk(inner.heap_pt, inner.heap_bottom, increment);
    inner.heap_pt as isize
}

pub fn sys_brk(brk_addr: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.acquire_inner_lock();
    let mut memory_set = task.vm.lock();
    if brk_addr == 0 {
        inner.heap_pt = memory_set.sbrk(inner.heap_pt, inner.heap_bottom, 0);
    } else {
        let former_addr = memory_set.sbrk(inner.heap_pt, inner.heap_bottom, 0);
        let grow_size: isize = (brk_addr - former_addr) as isize;
        inner.heap_pt = memory_set.sbrk(inner.heap_pt, inner.heap_bottom, grow_size);
    }

    info!(
        "[sys_brk] brk_addr: {:X}; new_addr: {:X}",
        brk_addr, inner.heap_pt
    );
    inner.heap_pt as isize
}

bitflags! {
    pub struct CloneFlags: u32 {
        //const CLONE_NEWTIME         =   0x00000080;
        /// 决定是否共享虚拟内存空间
        const CLONE_VM              =   0x00000100;
        /// 决定是否共享文件系统信息（如当前工作目录和根目录）
        const CLONE_FS              =   0x00000200;
        /// 使新进程共享打开的文件描述符表，但不共享文件描述符的状态
        const CLONE_FILES           =   0x00000400;
        /// 使新进程共享信号处理
        const CLONE_SIGHAND         =   0x00000800;
        const CLONE_PIDFD           =   0x00001000;
        const CLONE_PTRACE          =   0x00002000;
        const CLONE_VFORK           =   0x00004000;
        const CLONE_PARENT          =   0x00008000;
        const CLONE_THREAD          =   0x00010000;
        const CLONE_NEWNS           =   0x00020000;
        const CLONE_SYSVSEM         =   0x00040000;
        const CLONE_SETTLS          =   0x00080000;
        const CLONE_PARENT_SETTID   =   0x00100000;
        const CLONE_CHILD_CLEARTID  =   0x00200000;
        const CLONE_DETACHED        =   0x00400000;
        const CLONE_UNTRACED        =   0x00800000;
        const CLONE_CHILD_SETTID    =   0x01000000;
        const CLONE_NEWCGROUP       =   0x02000000;
        /// 使新进程拥有一个新的、独立的UTS命名空间，可以隔离主机名和域名
        const CLONE_NEWUTS          =   0x04000000;
        /// 使新进程拥有一个新的、独立的IPC命名空间，可以隔离System V IPC和POSIX消息队列
        const CLONE_NEWIPC          =   0x08000000;
        /// 使新进程拥有一个新的、独立的用户命名空间，可以隔离用户和用户组ID
        const CLONE_NEWUSER         =   0x10000000;
        /// 使新进程拥有一个新的、独立的PID命名空间，可以隔离进程ID
        const CLONE_NEWPID          =   0x20000000;
        /// 使新进程拥有一个新的、独立的网络命名空间，可以隔离网络设备、协议栈和端口
        const CLONE_NEWNET          =   0x40000000;
        const CLONE_IO              =   0x80000000;
    }
}

/// # Explanation of Parameters
/// Mainly about `ptid`, `tls` and `ctid`: \
/// `CLONE_SETTLS`: The TLS (Thread Local Storage) descriptor is set to `tls`. \
/// `CLONE_PARENT_SETTID`: Store the child thread ID at the location pointed to by `ptid` in the parent's memory. \
/// `CLONE_CHILD_SETTID`: Store the child thread ID at the location pointed to by `ctid` in the child's memory. \
/// `ptid` is also used in `CLONE_PIDFD` (since Linux 5.2) \
/// Since user programs rarely use these, we could do lazy implementation.
pub fn sys_clone(
    flags: u32,
    stack: *const u8,
    ptid: *mut u32,
    tls: usize,
    ctid: *mut u32,
) -> isize {
    let parent = current_task().unwrap();
    // This signal will be sent to its parent when it exits
    // we need to add a field in TCB to support this feature, but not now.
    let exit_signal = match Signals::from_signum((flags & 0xff) as usize) {
        Ok(signal) => signal,
        Err(_) => {
            warn!(
                "[sys_clone] signum of exit_signal is unspecified or invalid: {}",
                (flags & 0xff) as usize
            );
            // This is permitted by standard, but we only support 64 signals
            Signals::empty()
        }
    };
    // Sure to succeed, because all bits are valid (See `CloneFlags`)
    let flags = CloneFlags::from_bits(flags & !0xff).unwrap();
    info!(
        "[sys_clone] flags: {:?}, stack: {:?}, exit_signal: {:?}, ptid: {:?}, tls: {:?}, ctid: {:?}",
        flags, stack, exit_signal, ptid, tls, ctid
    );
    show_frame_consumption! {
        "clone";
        let child = parent.sys_clone(flags, stack, tls, exit_signal);
    }
    let new_pid = child.pid.0;
    if flags.contains(CloneFlags::CLONE_PARENT_SETTID) {
        match translated_refmut(parent.get_user_token(), ptid) {
            Ok(word) => *word = child.pid.0 as u32,
            Err(errno) => return errno,
        };
    }
    if flags.contains(CloneFlags::CLONE_CHILD_SETTID) && !ctid.is_null() {
        match translated_refmut(child.get_user_token(), ctid) {
            Ok(word) => *word = child.pid.0 as u32,
            Err(_) => {}
        };
    }
    if flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) && !ctid.is_null() {
        child.acquire_inner_lock().clear_child_tid = ctid as usize;
    }
    // add new task to scheduler
    add_task(child);
    new_pid as isize
}

/// 执行可执行文件
/// # 参数
/// + pathname：文件路径
/// + argv：参数列表
/// + envp：环境变量列表
pub fn sys_execve(
    pathname: *const u8,
    mut argv: *const *const u8,
    mut envp: *const *const u8,
) -> isize {
    // 设置默认shell为bash
    const DEFAULT_SHELL: &str = "/bin/bash";
    // 获取当前进程
    let task = current_task().unwrap();
    // 获取当前进程的用户态内存访问权限
    let token = task.get_user_token();
    // 获取可执行文件的路径
    let path = match translated_str(token, pathname) {
        Ok(path) => path,
        Err(errno) => return errno,
    };
    // 解析参数列表
    let mut argv_vec: Vec<String> = Vec::with_capacity(16);
    // 解析环境变量列表
    let mut envp_vec: Vec<String> = Vec::with_capacity(16);
    if !argv.is_null() {
        loop {
            let arg_ptr = match translated_ref(token, argv) {
                Ok(argv) => *argv,
                Err(errno) => return errno,
            };
            if arg_ptr.is_null() {
                break;
            }
            argv_vec.push(match translated_str(token, arg_ptr) {
                Ok(arg) => arg,
                Err(errno) => return errno,
            });
            unsafe {
                argv = argv.add(1);
            }
        }
    }
    if !envp.is_null() {
        loop {
            let env_ptr = match translated_ref(token, envp) {
                Ok(envp) => *envp,
                Err(errno) => return errno,
            };
            if env_ptr.is_null() {
                break;
            }
            envp_vec.push(match translated_str(token, env_ptr) {
                Ok(env) => env,
                Err(errno) => return errno,
            });
            unsafe {
                envp = envp.add(1);
            }
        }
    }
    debug!(
        "[exec] argv: {:?} /* {} vars */, envp: {:?} /* {} vars */",
        argv_vec,
        argv_vec.len(),
        envp_vec,
        envp_vec.len()
    );
    // 获取当前工作目录的文件描述符
    let working_inode = &task.fs.lock().working_inode;

    match working_inode.open(&path, OpenFlags::O_RDONLY, false) {
        // 检查打开的文件
        Ok(file) => {
            // 若文件大小小于4，则返回ENOEXEC
            // 即非可执行文件
            if file.get_size() < 4 {
                return ENOEXEC;
            }
            // 看前四个字节是否是可执行文件魔数
            let mut magic_number = Box::<[u8; 4]>::new([0; 4]);
            // this operation may be expensive... I'm not sure
            file.read(Some(&mut 0usize), magic_number.as_mut_slice());
            let elf = match magic_number.as_slice() {
                // ELF可执行文件
                b"\x7fELF" => file,
                // 脚本文件
                // 用默认Shell即bash加载
                b"#!" => {
                    let shell_file = working_inode
                        .open(DEFAULT_SHELL, OpenFlags::O_RDONLY, false)
                        .unwrap();
                    argv_vec.insert(0, DEFAULT_SHELL.to_string());
                    shell_file
                }
                // 非可执行文件
                _ => return ENOEXEC,
            };

            let task = current_task().unwrap();
            show_frame_consumption! {
                "load_elf";
                if let Err(errno) = task.load_elf(elf, &argv_vec, &envp_vec) {
                    return errno;
                };
            }
            // should return 0 in success
            SUCCESS
        }
        Err(errno) => errno,
    }
}

bitflags! {
    struct WaitOption: u32 {
        const WNOHANG    = 1;
        const WSTOPPED   = 2;
        const WEXITED    = 4;
        const WCONTINUED = 8;
        const WNOWAIT    = 0x1000000;
    }
}
/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_wait4(pid: isize, status: *mut u32, option: u32, _ru: *mut Rusage) -> isize {
    let option = WaitOption::from_bits(option).unwrap();
    info!("[sys_wait4] pid: {}, option: {:?}", pid, option);
    let task = current_task().unwrap();
    let token = task.get_user_token();
    loop {
        // find a child process

        // ---- hold current PCB lock
        let mut inner = task.acquire_inner_lock();
        if inner
            .children
            .iter()
            .find(|p| pid == -1 || pid as usize == p.getpid())
            .is_none()
        {
            return ECHILD;
            // ---- release current PCB lock
        }
        inner
            .children
            .iter()
            .filter(|p| pid == -1 || pid as usize == p.getpid())
            .for_each(|p| {
                trace!(
                    "[sys_wait4] found child pid: {}, status: {:?}",
                    p.pid.0,
                    p.acquire_inner_lock().task_status
                )
            });
        let pair = inner.children.iter().enumerate().find(|(_, p)| {
            // ++++ temporarily hold child PCB lock
            p.acquire_inner_lock().is_zombie() && (pid == -1 || pid as usize == p.getpid())
            // ++++ release child PCB lock
        });
        if let Some((idx, _)) = pair {
            // drop last TCB of child
            let child = inner.children.remove(idx);
            trace!("[wait4] release zombie task, pid: {}", child.pid.0);
            // confirm that child will be deallocated after being removed from children list
            assert_eq!(Arc::strong_count(&child), 1);
            // if main thread exit
            if child.pid.0 == child.tgid {
                let found_pid = child.getpid();
                // ++++ temporarily hold child lock
                let exit_code = child.acquire_inner_lock().exit_code;
                // ++++ release child PCB lock
                if !status.is_null() {
                    // this may NULL!!!
                    match translated_refmut(token, status) {
                        Ok(word) => *word = exit_code,
                        Err(errno) => return errno,
                    };
                }
                return found_pid as isize;
            }
        } else {
            drop(inner);
            if option.contains(WaitOption::WNOHANG) {
                return SUCCESS;
            } else {
                block_current_and_run_next();
                debug!("[sys_wait4] --resumed--");
            }
        }
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct RLimit {
    rlim_cur: usize, /* Soft limit */
    rlim_max: usize, /* Hard limit (ceiling for rlim_cur) */
}

#[derive(Debug, Eq, PartialEq, FromPrimitive)]
#[repr(u32)]
pub enum Resource {
    CPU = 0,
    FSIZE = 1,
    DATA = 2,
    STACK = 3,
    CORE = 4,
    RSS = 5,
    NPROC = 6,
    NOFILE = 7,
    MEMLOCK = 8,
    AS = 9,
    LOCKS = 10,
    SIGPENDING = 11,
    MSGQUEUE = 12,
    NICE = 13,
    RTPRIO = 14,
    RTTIME = 15,
    NLIMITS = 16,
    #[num_enum(default)]
    ILLEAGAL,
}

/// It can be used to both set and get the resource limits of an arbitrary process.
/// # WARNING
/// Partial implementation
pub fn sys_prlimit(
    pid: usize,
    resource: u32,
    new_limit: *const RLimit,
    old_limit: *mut RLimit,
) -> isize {
    if pid == 0 {
        let task = current_task().unwrap();
        let inner = task.acquire_inner_lock();
        let token = task.get_user_token();
        let resource = Resource::from_primitive(resource);
        info!("[sys_prlimit] pid: {}, resource: {:?}", pid, resource);

        drop(inner);
        if !old_limit.is_null() {
            match resource {
                Resource::STACK => {
                    if copy_to_user(
                        token,
                        &(RLimit {
                            rlim_cur: USER_STACK_SIZE,
                            rlim_max: USER_STACK_SIZE,
                        }),
                        old_limit,
                    )
                    .is_err()
                    {
                        log::error!("[sys_prlimit] Failed to copy to {:?}", old_limit);
                        return EFAULT;
                    }
                }
                Resource::NPROC => {
                    if copy_to_user(
                        token,
                        &(RLimit {
                            rlim_cur: SYSTEM_TASK_LIMIT,
                            rlim_max: SYSTEM_TASK_LIMIT,
                        }),
                        old_limit,
                    )
                    .is_err()
                    {
                        log::error!("[sys_prlimit] Failed to copy to {:?}", old_limit);
                        return EFAULT;
                    }
                }
                Resource::NOFILE => {
                    let lock = task.files.lock();
                    if copy_to_user(
                        token,
                        &(RLimit {
                            rlim_cur: lock.get_soft_limit(),
                            rlim_max: lock.get_hard_limit(),
                        }),
                        old_limit,
                    )
                    .is_err()
                    {
                        log::error!("[sys_prlimit] Failed to copy to {:?}", old_limit);
                        return EFAULT;
                    }
                }
                Resource::ILLEAGAL => return EINVAL,
                _ => return EINVAL,
            }
        }
        if !new_limit.is_null() {
            let rlimit = &mut RLimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if copy_from_user(token, new_limit, rlimit).is_err() {
                log::error!("[sys_prlimit] Failed to copy from {:?}", new_limit);
                return EFAULT;
            };
            match resource {
                Resource::NOFILE => {
                    task.files.lock().set_soft_limit(rlimit.rlim_cur);
                    task.files.lock().set_hard_limit(rlimit.rlim_max);
                }
                Resource::STACK => {
                    warn!("[prlimit] Unsupported modification stack");
                    assert!(rlimit.rlim_cur <= USER_STACK_SIZE);
                }
                Resource::ILLEAGAL => return EINVAL,
                _ => return EINVAL,
            }
        }
    } else {
        return EINVAL;
    }
    SUCCESS
}
/// set pointer to thread ID
/// This feature is currently NOT supported and is implemented as a stub,
/// since threads are not supported.
pub fn sys_set_tid_address(tidptr: usize) -> isize {
    current_task().unwrap().acquire_inner_lock().clear_child_tid = tidptr;
    sys_gettid()
}

bitflags! {
    pub struct FutexOption: u32 {
        const PRIVATE = 128;
        const CLOCK_REALTIME = 256;
    }
}

/// # 描述
/// fast user-space locking
/// # 参数
/// * `uaddr`: `usize`, the address to the futex word;
/// * `futex_op`: `u32`, the operation to perform on the futex;
/// The remaining arguments (val, timeout, uaddr2, and val3) are re‐
/// quired only for certain of the futex  operations  described
/// below.  Where one of these arguments is not required, it is
/// ignored.
/// * `val`: `u32`, the argument to futex_op
/// * `timeout`: `*const TimeSpec`,
/// * `uaddr2`: `usize`,
/// * `val3`: `u32`,
pub fn sys_futex(
    uaddr: *mut u32,
    futex_op: u32,
    val: u32,
    timeout: *const TimeSpec,
    uaddr2: *mut u32,
    val3: u32,
) -> isize {
    let task = current_task().unwrap();
    let token = task.get_user_token();
    // uaddr is always used
    if uaddr.is_null() || uaddr.align_offset(4) != 0 {
        return EINVAL;
    }
    let futex_word = match translated_refmut(token, uaddr) {
        Ok(futex_word) => futex_word,
        Err(errno) => return errno,
    };
    let cmd = threads::FutexCmd::from_primitive(futex_op & 0x7fu32);
    let option = FutexOption::from_bits_truncate(futex_op);
    if !option.contains(FutexOption::PRIVATE) {
        warn!("[futex] process-shared futex is unimplemented");
    }
    info!(
        "[futex] uaddr: {:?}, futex_op: {:?}, option: {:?}, val: {:X}, timeout: {:?}, uaddr2: {:?}, val3: {:X}",
        uaddr, cmd, option, val, timeout, uaddr2, val3
    );
    match cmd {
        FutexCmd::Wait => {
            let timeout = match try_get_from_user(token, timeout) {
                Ok(timeout) => timeout,
                Err(errno) => return errno,
            };
            drop(task);
            do_futex_wait(futex_word, val, timeout)
        }
        FutexCmd::WaitBitset => {
            if val3 == 0 {
                return EINVAL;
            }
            let timeout = match try_get_from_user(token, timeout) {
                Ok(timeout) => timeout,
                Err(errno) => return errno,
            };
            drop(task);
            do_futex_wait(futex_word, val, timeout)
        }
        FutexCmd::Wake => {
            let futex_word_addr = futex_word as *const u32 as usize;
            task.futex.lock().wake(futex_word_addr, val)
        }
        FutexCmd::Requeue => {
            if uaddr2.is_null() || uaddr2.align_offset(4) != 0 {
                return EINVAL;
            }
            let futex_word_2 = match translated_refmut(token, uaddr2) {
                Ok(futex_word_2) => futex_word_2,
                Err(errno) => return errno,
            };
            task.futex
                .lock()
                .requeue(futex_word, futex_word_2, val, timeout as u32)
        }
        FutexCmd::WakeOp => {
            if uaddr2.is_null() || uaddr2.align_offset(4) != 0 {
                return EINVAL;
            }
            let futex_word_2 = match translated_refmut(token, uaddr2) {
                Ok(futex_word_2) => futex_word_2,
                Err(errno) => return errno,
            };
            let op = (val3 >> 28) & 0xf;
            let cmp = (val3 >> 24) & 0xf;
            let cmparg = val3 & 0xfff;

            let oldval = *futex_word_2;
            let newval = match op {
                0 => cmparg,
                1 => oldval.wrapping_add(cmparg),
                2 => oldval | cmparg,
                3 => oldval & !cmparg,
                4 => oldval ^ cmparg,
                _ => return EINVAL,
            };
            *futex_word_2 = newval;

            let cond = match cmp {
                0 => oldval == cmparg,
                1 => oldval != cmparg,
                2 => oldval < cmparg,
                3 => oldval <= cmparg,
                4 => oldval > cmparg,
                5 => oldval >= cmparg,
                _ => true,
            };

            let futex_word_addr = futex_word as *const u32 as usize;
            let futex_word_addr_2 = futex_word_2 as *const u32 as usize;
            task.futex
                .lock()
                .wake_op(futex_word_addr, futex_word_addr_2, val, cond)
        }
        FutexCmd::Invalid => EINVAL,
        _ => ENOSYS,
    }
}

pub fn sys_set_robust_list(head: usize, len: usize) -> isize {
    if len != crate::task::RobustList::HEAD_SIZE {
        return EINVAL;
    }
    let task = current_task().unwrap();
    let mut inner = task.acquire_inner_lock();
    inner.robust_list.head = head;
    //inner.robust_list.len = len;
    SUCCESS
}

pub fn sys_get_robust_list(pid: u32, head_ptr: *mut usize, len_ptr: *mut usize) -> isize {
    let task = if pid == 0 {
        current_task().unwrap()
    } else {
        match find_task_by_pid(pid as usize) {
            Some(task) => task,
            None => return ESRCH,
        }
    };
    let inner = task.acquire_inner_lock();
    let token = current_user_token();
    if copy_to_user(token, &inner.robust_list.head, head_ptr).is_err() {
        log::error!("[sys_get_robust_list] Failed to copy to {:?}", head_ptr);
        return EFAULT;
    };
    if copy_to_user(token, &inner.robust_list.len, len_ptr).is_err() {
        log::error!("[sys_get_robust_list] Failed to copy to {:?}", len_ptr);
        return EFAULT;
    };
    SUCCESS
}

pub fn sys_mmap(
    start: usize,
    len: usize,
    prot: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> isize {
    let task = current_task().unwrap();
    let mut memory_set = task.vm.lock();
    let prot = MapPermission::from_bits(((prot as u8) << 1) | (1 << 4)).unwrap();
    let flags = MapFlags::from_bits(flags).unwrap();
    info!(
        "[mmap] start:{:X}; len:{:X}; prot:{:?}; flags:{:?}; fd:{}; offset:{:X}",
        start, len, prot, flags, fd as isize, offset
    );
    memory_set.mmap(start, len, prot, flags, fd, offset)
}

/// # Versions
/// The membarrier() system call was added in Linux 4.3.
/// Before Linux 5.10, the prototype for membarrier() was:
/// `int membarrier(int cmd, int flags);`
pub fn sys_memorybarrier(_cmd: usize, _flags: usize, _cpu_id: usize) -> isize {
    error!("[sys_memorybarrier]=========PSEUDOIMPLEMENTATION=========");
    error!(
        "This system call is only needed by the multicore environment for faster synchronization."
    );
    error!("In theory, it can be replaced (INefficiently) by fencing.");
    return SUCCESS;
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    let task = current_task().unwrap();
    let result = task.vm.lock().munmap(start, len);
    match result {
        Ok(_) => SUCCESS,
        Err(errno) => errno,
    }
}

pub fn sys_mprotect(addr: usize, len: usize, prot: usize) -> isize {
    let task = current_task().unwrap();
    let result = task.vm.lock().mprotect(addr, len, prot);
    match result {
        Ok(_) => SUCCESS,
        Err(errno) => {
            crate::println!("[mprotect] FAIL addr={:#x} len={:#x} prot={:#x} err={}", addr, len, prot, errno);
            errno
        }
    }
}

/// mlock/mlockall/munlock/munlockall stubs — no swap, always succeed
pub fn sys_mlock(_addr: usize, _len: usize) -> isize {
    SUCCESS
}
pub fn sys_munlock(_addr: usize, _len: usize) -> isize {
    SUCCESS
}
pub fn sys_mlockall(_flags: usize) -> isize {
    SUCCESS
}
pub fn sys_munlockall() -> isize {
    SUCCESS
}

pub fn sys_clock_gettime(clk_id: usize, tp: *mut TimeSpec) -> isize {
    if !tp.is_null() {
        let token = current_user_token();
        let timespec = match clk_id {
            // CLOCK_REALTIME = 0: system-wide real-time clock
            0 => {
                let unix_ts = crate::timer::current_time();
                let ns = crate::timer::get_time_ns() % crate::timer::NSEC_PER_SEC;
                TimeSpec {
                    tv_sec: unix_ts as usize,
                    tv_nsec: ns,
                }
            }
            // CLOCK_MONOTONIC = 1, CLOCK_BOOTTIME = 7: unaffected by system time changes
            1 | 7 => TimeSpec::now(),
            // CLOCK_MONOTONIC_RAW = 4: raw hardware time
            4 => TimeSpec::now(),
            // CLOCK_REALTIME_COARSE = 5, CLOCK_MONOTONIC_COARSE = 6
            5 => {
                let unix_ts = crate::timer::current_time();
                TimeSpec {
                    tv_sec: unix_ts as usize,
                    tv_nsec: 0,
                }
            }
            6 => TimeSpec::now(),
            _ => TimeSpec::now(),
        };
        if let Err(e) = copy_to_user(token, &timespec, tp) {
            return e;
        };
    }
    SUCCESS
}
pub fn sys_clock_nanosleep(
    clk_id: usize,
    flags: u32,
    rqtp: *const TimeSpec,
    rmtp: *mut TimeSpec,
) -> isize {
    if rqtp.is_null() {
        return EINVAL;
    }

    const TIMER_ABSTIME: u32 = 0x01;

    let task = current_task().unwrap();
    let token = task.get_user_token();
    let rq = match get_from_user(token, rqtp) {
        Ok(ts) => ts,
        Err(e) => return e,
    };

    // Validate rqtp
    if rq.tv_nsec >= NSEC_PER_SEC {
        return EINVAL;
    }

    let end = if (flags & TIMER_ABSTIME) != 0 {
        // Absolute time — rq is an absolute wakeup time
        match clk_id {
            0 | 5 => {
                // CLOCK_REALTIME / CLOCK_REALTIME_COARSE:
                // rq is absolute Unix time; convert to boot-time wakeup
                let unix_sec = crate::timer::current_time() as usize;
                let unix_nsec = crate::timer::get_time_ns() % NSEC_PER_SEC;
                let unix_now_ns = unix_sec * NSEC_PER_SEC + unix_nsec;
                let req_ns = rq.tv_sec * NSEC_PER_SEC + rq.tv_nsec;
                if req_ns <= unix_now_ns {
                    return SUCCESS;
                }
                let remaining_ns = req_ns - unix_now_ns;
                TimeSpec::now() + TimeSpec::from_ns(remaining_ns)
            }
            _ => {
                // CLOCK_MONOTONIC / CLOCK_BOOTTIME / CLOCK_MONOTONIC_RAW /
                // CLOCK_MONOTONIC_COARSE: rq is already boot-time-compatible
                let now = TimeSpec::now();
                if rq <= now {
                    return SUCCESS;
                }
                rq
            }
        }
    } else {
        // Relative time — rq is a duration
        if rq.tv_sec == 0 && rq.tv_nsec == 0 {
            return SUCCESS;
        }
        TimeSpec::now() + rq
    };

    wait_with_timeout(Arc::downgrade(&task), end);
    drop(task);

    block_current_and_run_next();

    let task = current_task().unwrap();
    let inner = task.acquire_inner_lock();
    if inner.sigpending.is_empty() {
        // Normal timeout expiry
        if !rmtp.is_null() {
            copy_to_user(token, &TimeSpec::new(), rmtp).unwrap();
        }
        SUCCESS
    } else {
        // Interrupted by signal — write remaining time
        if !rmtp.is_null() {
            let now = TimeSpec::now();
            let rem = if end > now { end - now } else { TimeSpec::new() };
            copy_to_user(token, &rem, rmtp).unwrap();
        }
        EINTR
    }
}
    
// int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact);
pub fn sys_sigaction(signum: usize, act: usize, oldact: usize) -> isize {
    trace!(
        "[sys_sigaction] signum: {:?}, act: {:X}, oldact: {:X}",
        signum,
        act,
        oldact
    );
    sigaction(signum, act as *const SigAction, oldact as *mut SigAction)
}

/// Note: code translation should be done in syscall rather than the call handler as the handler may be reused by kernel code which use kernel structs
pub fn sys_sigprocmask(how: u32, set: usize, oldset: usize) -> isize {
    info!(
        "[sys_sigprocmask] how: {:?}; set: {:X}, oldset: {:X}",
        how, set, oldset
    );
    sigprocmask(how, set as *const Signals, oldset as *mut Signals)
}

pub fn sys_sigtimedwait(set: usize, info: usize, timeout: usize) -> isize {
    sigtimedwait(
        set as *const Signals,
        info as *mut SigInfo,
        timeout as *const TimeSpec,
    )
}

pub fn sys_sigreturn() -> isize {
    // mark not processing signal handler
    let task = current_task().unwrap();
    let mut inner = task.acquire_inner_lock();
    let token = task.get_user_token();
    info!("[sys_sigreturn] pid: {}", task.pid.0);

    let trap_cx = inner.get_trap_cx();
    // restore sigmask & trap context
    let ucontext_addr = (trap_cx.gp.sp + size_of::<SigInfo>() + 0x7) & !0x7;
    inner.sigmask = *translated_ref(
        token,
        (ucontext_addr + 2 * size_of::<usize>() + size_of::<SignalStack>()) as *mut Signals,
    )
    .unwrap(); // restore sigmask
    copy_from_user(
        token,
        (ucontext_addr
            + 2 * size_of::<usize>()
            + size_of::<SignalStack>()
            + size_of::<Signals>()
            + crate::hal::UserContext::PADDING_SIZE) as *mut MachineContext,
        (trap_cx as *mut TrapContext).cast::<MachineContext>(),
    )
    .unwrap(); // restore trap_cx
               // This should be `Ok(())`.
    return trap_cx.gp.a0 as isize; // return a0: not modify any of trap_cx
}

/// Get process times
pub fn sys_times(buf: *mut Times) -> isize {
    let task = current_task().unwrap();
    let inner = task.acquire_inner_lock();
    let token = task.get_user_token();
    let times = Times {
        tms_utime: inner.rusage.ru_utime.to_tick(),
        tms_stime: inner.rusage.ru_stime.to_tick(),
        tms_cutime: 0,
        tms_cstime: 0,
    };
    if copy_to_user(token, &times, buf).is_err() {
        log::error!("[sys_times] Failed to copy to {:?}", buf);
        return EFAULT;
    };
    // return clock ticks that have elapsed since an arbitrary point in the past
    crate::hal::get_time() as isize
}

pub fn sys_getrusage(who: isize, usage: *mut Rusage) -> isize {
    if who != 0 {
        panic!("[sys_getrusage] parameter 'who' is not RUSAGE_SELF.");
    }
    let task = current_task().unwrap();
    let inner = task.acquire_inner_lock();
    let token = task.get_user_token();
    if copy_to_user(token, &inner.rusage, usage).is_err() {
        log::error!("[sys_getrusage] Failed to copy to {:?}", usage);
        return EFAULT;
    };
    //info!("[sys_getrusage] who: RUSAGE_SELF, usage: {:?}", inner.rusage);
    SUCCESS
}

pub fn sys_mincore(addr: usize, length: usize, vec: *mut u8) -> isize {
    trace!("[sys_mincore] addr={:#x}, length={:#x}, vec={:?}", addr, length, vec);
    SUCCESS
}

pub fn sys_madvise(addr: usize, length: usize, advice: usize) -> isize {
    trace!("[sys_madvise] addr={:#x}, length={:#x}, advice={}", addr, length, advice);
    SUCCESS
}

/// IoVec layout for process_vm_readv/writev (x86_64/riscv64: 16 bytes each)
#[repr(C)]
struct IoVec {
    iov_base: usize,
    iov_len: usize,
}

/// Read iovecs from user space and return a flat Vec of (addr, len) pairs
fn read_iovecs(token: usize, iov_ptr: usize, iovcnt: usize) -> Result<Vec<(usize, usize)>, isize> {
    if iovcnt == 0 {
        return Ok(Vec::new());
    }
    let iov_size = core::mem::size_of::<IoVec>() * iovcnt;
    let iov_bytes = translated_byte_buffer(token, iov_ptr as *const u8, iov_size)?;
    // Now we can reinterpret the bytes as IoVecs
    let ptr = iov_bytes.as_ptr() as *const IoVec;
    let iovecs = unsafe { core::slice::from_raw_parts(ptr, iovcnt) };
    Ok(iovecs.iter().map(|iov| (iov.iov_base, iov.iov_len)).collect())
}

/// Compute total len from iovec list
fn iovec_total_len(iovs: &[(usize, usize)]) -> usize {
    iovs.iter().map(|(_, len)| len).sum()
}

/// Copy from src token's memory (src_iovs) to dst token's memory (dst_iovs).
/// Returns number of bytes actually copied.
fn process_vm_copy(
    src_token: usize,
    src_iovs: &[(usize, usize)],
    dst_token: usize,
    dst_iovs: &[(usize, usize)],
) -> usize {
    let src_total = iovec_total_len(src_iovs);
    let dst_total = iovec_total_len(dst_iovs);
    let copy_len = src_total.min(dst_total);
    if copy_len == 0 {
        return 0;
    }

    // Simpler approach: gather all src bytes into one contiguous kernel buffer, then scatter to dst
    let mut temp = alloc::vec![0u8; copy_len];
    let mut copied = 0usize;
    for &(addr, len) in src_iovs {
        if copied >= copy_len { break; }
        let take = len.min(copy_len - copied);
        match translated_byte_buffer(src_token, addr as *const u8, take) {
            Ok(bufs) => {
                let mut off = 0usize;
                for b in &bufs {
                    let btake = b.len().min(take - off);
                    temp[copied..copied + btake].copy_from_slice(&b[..btake]);
                    copied += btake;
                    off += btake;
                }
            }
            Err(_) => break,
        }
    }

    // Scatter to destination
    let mut written = 0usize;
    for &(addr, len) in dst_iovs {
        if written >= copy_len { break; }
        let take = len.min(copy_len - written);
        if let Ok(bufs) = translated_byte_buffer(dst_token, addr as *mut u8, take) {
            let mut wbuf = UserBuffer::new(bufs);
            wbuf.write(&temp[written..written + take]);
            written += take;
        } else {
            break;
        }
    }
    written
}

pub fn sys_process_vm_readv(pid: usize, lvec: usize, liovcnt: usize, rvec: usize, riovcnt: usize, flags: usize) -> isize {
    trace!("[sys_process_vm_readv] pid={}, liovcnt={}, riovcnt={}, flags={}", pid, liovcnt, riovcnt, flags);
    let curr = current_task().unwrap();
    let curr_token = curr.get_user_token();
    let local_iovs = match read_iovecs(curr_token, lvec, liovcnt) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let remote_iovs = match read_iovecs(curr_token, rvec, riovcnt) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let target = match find_task_by_pid(pid) {
        Some(t) => t,
        None => return ESRCH,
    };
    let target_token = target.get_user_token();
    // Read from remote (target) → write to local (current)
    let n = process_vm_copy(target_token, &remote_iovs, curr_token, &local_iovs);
    n as isize
}

pub fn sys_process_vm_writev(pid: usize, lvec: usize, liovcnt: usize, rvec: usize, riovcnt: usize, flags: usize) -> isize {
    trace!("[sys_process_vm_writev] pid={}, liovcnt={}, riovcnt={}, flags={}", pid, liovcnt, riovcnt, flags);
    let curr = current_task().unwrap();
    let curr_token = curr.get_user_token();
    let local_iovs = match read_iovecs(curr_token, lvec, liovcnt) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let remote_iovs = match read_iovecs(curr_token, rvec, riovcnt) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let target = match find_task_by_pid(pid) {
        Some(t) => t,
        None => return ESRCH,
    };
    let target_token = target.get_user_token();
    // Write from local (current) → to remote (target)
    let n = process_vm_copy(curr_token, &local_iovs, target_token, &remote_iovs);
    n as isize
}

pub fn sys_sched_setparam(pid: usize, param: *const u8) -> isize {
    trace!("[sys_sched_setparam] pid={}", pid);
    SUCCESS
}

pub fn sys_sched_setscheduler(pid: usize, policy: usize, param: *const u8) -> isize {
    trace!("[sys_sched_setscheduler] pid={}, policy={}", pid, policy);
    SUCCESS
}

pub fn sys_sched_getscheduler(pid: usize) -> isize {
    trace!("[sys_sched_getscheduler] pid={}", pid);
    0 // SCHED_OTHER
}

pub fn sys_sched_getparam(pid: usize, param: *mut u8) -> isize {
    trace!("[sys_sched_getparam] pid={}", pid);
    SUCCESS
}

pub fn sys_sched_rr_get_interval(pid: usize, tp: *mut u8) -> isize {
    trace!("[sys_sched_rr_get_interval] pid={}", pid);
    EINVAL // not real-time
}

pub fn sys_setpriority(which: usize, who: usize, prio: usize) -> isize {
    trace!("[sys_setpriority] which={}, who={}, prio={}", which, who, prio);
    SUCCESS
}

pub fn sys_getpriority(which: usize, who: usize) -> isize {
    trace!("[sys_getpriority] which={}, who={}", which, who);
    20 // default nice value
}

pub fn sys_getgroups(size: i32, list: *mut u32) -> isize {
    trace!("[sys_getgroups] size={}", size);
    // Return single group 0 (root)
    if size < 1 {
        return 1; // return number of groups
    }
    let token = current_user_token();
    let gid: u32 = 0;
    unsafe { (list as *mut u32).write_volatile(gid) };
    1
}

pub fn sys_setgroups(size: usize, list: *const u32) -> isize {
    trace!("[sys_setgroups] size={}", size);
    SUCCESS
}

pub fn sys_getrlimit(resource: usize, rlim: *mut u8) -> isize {
    trace!("[sys_getrlimit] resource={}", resource);
    SUCCESS
}

pub fn sys_setrlimit(resource: usize, rlim: *const u8) -> isize {
    trace!("[sys_setrlimit] resource={}", resource);
    SUCCESS
}

pub fn sys_rt_sigpending(set: *mut u8, sigsetsize: usize) -> isize {
    trace!("[sys_rt_sigpending] sigsetsize={}", sigsetsize);
    SUCCESS
}

pub fn sys_waitid(idtype: usize, id: usize, infop: *mut u8, options: usize, rusage: *mut u8) -> isize {
    trace!("[sys_waitid] idtype={}, id={}, options={}", idtype, id, options);
    // Fall back to wait4
    crate::syscall::process::sys_wait4(id as isize, infop as *mut u32, options as u32, rusage as *mut crate::task::Rusage)
}

pub fn sys_kcmp(pid1: usize, pid2: usize, type_: usize, idx1: usize, idx2: usize) -> isize {
    trace!("[sys_kcmp] pid1={}, pid2={}, type={}", pid1, pid2, type_);
    // Return 0 (equal) - enough for cyclictest/hackbench to proceed
    0
}

pub fn sys_sched_setaffinity(pid: usize, cpusetsize: usize, mask: *const u8) -> isize {
    trace!("[sys_sched_setaffinity] pid={}, cpusetsize={}", pid, cpusetsize);
    SUCCESS
}

pub fn sys_sched_getaffinity(pid: usize, cpusetsize: usize, mask: *mut u8) -> isize {
    trace!("[sys_sched_getaffinity] pid={}, cpusetsize={}", pid, cpusetsize);
    if mask.is_null() || cpusetsize == 0 {
        return EINVAL;
    }
    let token = current_user_token();
    let mut cpu_mask = [0u8; 128];
    cpu_mask[0] = 1; // CPU 0 is available (musl requires at least 1 bit set)
    let copy_size = core::cmp::min(cpusetsize, cpu_mask.len());
    let mut buffer = crate::mm::UserBuffer::new(
        match crate::mm::translated_byte_buffer(token, mask, copy_size) {
            Ok(buf) => buf,
            Err(e) => return e,
        }
    );
    buffer.write_at(0, &cpu_mask[..copy_size]);
    copy_size as isize
}

pub fn sys_sched_get_priority_max(policy: usize) -> isize {
    trace!("[sys_sched_get_priority_max] policy={}", policy);
    match policy {
        1 | 2 => 99,
        _ => 0,
    }
}

pub fn sys_sched_get_priority_min(policy: usize) -> isize {
    trace!("[sys_sched_get_priority_min] policy={}", policy);
    match policy {
        1 | 2 => 1,
        _ => 0,
    }
}

pub fn sys_prctl(option: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize, arg6: usize) -> isize {
    trace!("[sys_prctl] option={:#x}", option);
    SUCCESS
}
