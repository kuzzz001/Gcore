use crate::{
    fs::{directory_tree::DirectoryTreeNode, file_trait::File, layout::Stat, StatMode,
         DiskInodeType},
    mm::UserBuffer,
    syscall::errno::*,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Write;

pub struct ProcMeminfo;

#[allow(unused)]
impl File for ProcMeminfo {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcMeminfo {})
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        let (total_kb, free_kb) = {
            let total_mem = crate::config::MEMORY_SIZE;
            let free = crate::mm::unallocated_frames() * crate::config::PAGE_SIZE;
            (total_mem / 1024, free / 1024)
        };
        let mut s = alloc::string::String::with_capacity(256);
        let _ = write!(s, "MemTotal:       {:8} kB\n", total_kb);
        let _ = write!(s, "MemFree:        {:8} kB\n", free_kb);
        let _ = write!(s, "MemAvailable:   {:8} kB\n", free_kb);
        let _ = write!(s, "Cached:         {:8} kB\n", 0u32);

        let bytes = s.as_bytes();
        let len = bytes.len().min(buf.len());
        buf[..len].copy_from_slice(&bytes[..len]);
        len
    }

    fn write(&self, _offset: Option<&mut usize>, _buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 4),
            1,
            StatMode::S_IFREG.bits() | 0o444,
            1,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, _offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let (total_kb, free_kb) = {
            let total_mem = crate::config::MEMORY_SIZE;
            let free = crate::mm::unallocated_frames() * crate::config::PAGE_SIZE;
            (total_mem / 1024, free / 1024)
        };
        let mut s = alloc::string::String::with_capacity(256);
        let _ = write!(s, "MemTotal:       {:8} kB\n", total_kb);
        let _ = write!(s, "MemFree:        {:8} kB\n", free_kb);
        let _ = write!(s, "MemAvailable:   {:8} kB\n", free_kb);
        let _ = write!(s, "Cached:         {:8} kB\n", 0u32);

        let bytes = s.into_bytes();
        buf.write(&bytes)
    }

    fn write_user(&self, _offset: Option<usize>, _buf: UserBuffer) -> usize {
        0
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(
        &self,
        _dirnode_ptr: alloc::sync::Weak<DirectoryTreeNode>,
    ) {
    }

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: crate::fs::layout::OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcMeminfo {})
    }

    fn open_subfile(
        &self,
    ) -> Result<Vec<(alloc::string::String, Arc<dyn File>)>, isize> {
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

    fn get_dirent(&self, _count: usize) -> Vec<crate::fs::dirent::Dirent> {
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

    fn get_single_cache(
        &self,
        _offset: usize,
    ) -> Result<Arc<spin::Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(
        &self,
    ) -> Result<Vec<Arc<spin::Mutex<crate::fs::PageCache>>>, ()> {
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
