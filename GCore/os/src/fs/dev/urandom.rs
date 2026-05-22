use crate::fs::{dirent::Dirent, DiskInodeType};
use crate::utils::random::RNG;
use alloc::sync::Arc;
use alloc::vec::Vec;
use rand_core::RngCore;

use crate::{
    fs::{directory_tree::DirectoryTreeNode, file_trait::File, layout::Stat, StatMode},
    mm::UserBuffer,
    syscall::errno::{EINVAL, ENOTDIR, ESPIPE},
};

pub struct Urandom;

#[allow(unused)]
impl File for Urandom {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(Urandom {})
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        unsafe { RNG.fill_bytes(buf) };
        buf.len()
    }

    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        buf.len()
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        true
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 5),
            1,
            StatMode::S_IFCHR.bits() | 0o666,
            1,
            crate::makedev!(1, 5),
            0,
            0,
            0,
            0,
        )
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let len = buf.len();
        let mut tmp = alloc::vec![0u8; len];
        unsafe { RNG.fill_bytes(&mut tmp) };
        buf.write(&tmp)
    }

    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        buf.len()
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(
        &self,
        dirnode_ptr: alloc::sync::Weak<crate::fs::directory_tree::DirectoryTreeNode>,
    ) {
    }

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, flags: crate::fs::layout::OpenFlags, special_use: bool) -> Arc<dyn File> {
        Arc::new(Urandom {})
    }

    fn open_subfile(
        &self,
    ) -> Result<alloc::vec::Vec<(alloc::string::String, alloc::sync::Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, name: &str, file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EINVAL)
    }

    fn link_child(&self, name: &str, child: &Self) -> Result<(), isize>
    where
        Self: Sized,
    {
        Err(EINVAL)
    }

    fn unlink(&self, delete: bool) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn get_dirent(&self, count: usize) -> alloc::vec::Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, offset: isize, whence: crate::fs::SeekWhence) -> Result<usize, isize> {
        Err(ESPIPE)
    }

    fn modify_size(&self, diff: isize) -> Result<(), isize> {
        Ok(())
    }

    fn truncate_size(&self, new_size: usize) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn set_timestamp(&self, ctime: Option<usize>, atime: Option<usize>, mtime: Option<usize>) {}

    fn get_single_cache(
        &self,
        offset: usize,
    ) -> Result<Arc<spin::Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(
        &self,
    ) -> Result<alloc::vec::Vec<Arc<spin::Mutex<crate::fs::PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, cmd: u32, arg: u32) -> isize {
        EINVAL
    }
}
