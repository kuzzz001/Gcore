use crate::{
    fs::{directory_tree::DirectoryTreeNode, dirent::Dirent, file_trait::File, layout::Stat, StatMode, DiskInodeType},
    mm::UserBuffer,
    syscall::errno::{EINVAL, ENOTDIR, ESPIPE},
    timer::TimeSpec,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

/// Inner mutable state for TimerFd
struct TimerFdInner {
    /// expiration count since last read (returned by read())
    expirations: u64,
    /// whether the timer has been set
    armed: bool,
}

/// TimerFd state for a single timerfd instance
pub struct TimerFd {
    inner: Mutex<TimerFdInner>,
}

impl TimerFd {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TimerFdInner {
                expirations: 0,
                armed: false,
            }),
        }
    }

    pub fn set_time(&self, _flags: u32, _new_value: &TimeSpec, _old_value: Option<&mut TimeSpec>) {
        let mut inner = self.inner.lock();
        inner.armed = true;
    }

    pub fn get_time(&self, _curr_value: &mut TimeSpec) {
        _curr_value.tv_sec = 0;
        _curr_value.tv_nsec = 0;
    }

    pub fn is_ready(&self) -> bool {
        self.inner.lock().expirations > 0
    }

    fn read_expirations(&self) -> u64 {
        let mut inner = self.inner.lock();
        let val = inner.expirations;
        inner.expirations = 0;
        val
    }
}

#[allow(unused)]
impl File for TimerFd {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(TimerFd::new())
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        let val = self.read_expirations();
        let bytes = val.to_ne_bytes();
        let len = bytes.len().min(buf.len());
        buf[..len].copy_from_slice(&bytes[..len]);
        len
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        self.is_ready()
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 14),
            1,
            StatMode::S_IFCHR.bits() | 0o444,
            1,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, _offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let val = self.read_expirations();
        let bytes = val.to_ne_bytes();
        buf.write(&bytes)
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        0
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: crate::fs::layout::OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(TimerFd::new())
    }

    fn open_subfile(&self) -> Result<Vec<(alloc::string::String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EINVAL)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, _offset: isize, _whence: crate::fs::SeekWhence) -> Result<usize, isize> {
        Err(ESPIPE)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Ok(())
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<spin::Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<spin::Mutex<crate::fs::PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        EINVAL
    }
}
