use crate::syscall::*;
pub fn dup(fd: usize) -> isize {
    sys_dup(fd)
}
pub fn open(path: &str, flags: crate::OpenFlags) -> isize {
    sys_open(path, flags.bits)
}
pub fn close(fd: usize) -> isize {
    sys_close(fd)
}
pub fn pipe(pipe_fd: &mut [i32]) -> isize {
    sys_pipe(pipe_fd)
}
pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}
pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn getchar() -> u8 {
    let mut buf: [u8; 1] = [0u8];
    sys_read(0, &mut buf);
    buf[0]
}
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code);
}
pub fn yield_() -> isize {
    sys_yield()
}
pub fn get_time() -> isize {
    sys_get_time()
}
pub fn getpid() -> isize {
    sys_getpid()
}
pub fn fork() -> isize {
    sys_fork()
}
pub fn exec(path: &str, args: &[*const u8], envp: &[*const u8]) -> isize {
    sys_exec(path, args, envp)
}
pub fn chdir(path: &str) -> isize {
    sys_chdir(path)
}

pub fn wait(exit_code: &mut i32) -> isize {
    sys_waitpid(-1, exit_code as *mut _)
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    sys_waitpid(pid as isize, exit_code as *mut _)
}
/// 非阻塞等待（WNOHANG）：返回 0 表示子进程仍在运行，>0 为结束的 pid，<0 为错误。
pub fn waitpid_nohang(pid: usize, exit_code: &mut i32) -> isize {
    sys_wait4(pid as isize, exit_code as *mut _, 1)
}
pub fn kill(pid: usize, sig: usize) -> isize {
    sys_kill(pid, sig)
}
pub fn sleep(period_ms: usize) {
    let start = sys_get_time();
    while sys_get_time() < start + period_ms as isize {
        sys_yield();
    }
}
pub fn shutdown() -> isize{
    sys_shutdown()
}