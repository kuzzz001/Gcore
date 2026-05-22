use crate::{
    fs::{dirent::Dirent, file_trait::File, DiskInodeType},
    syscall::errno::{EINVAL, ENOTDIR, ESPIPE, SUCCESS},
};

pub struct Hwclock;

#[allow(unused)]
impl File for Hwclock {
    fn deep_clone(&self) -> alloc::sync::Arc<dyn File> {
        alloc::sync::Arc::new(Hwclock {})
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        0
    }

    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        0
    }

    fn r_ready(&self) -> bool {
        true
    }

    fn w_ready(&self) -> bool {
        false
    }

    fn read_user(&self, offset: Option<usize>, buf: crate::mm::UserBuffer) -> usize {
        0
    }

    fn write_user(&self, offset: Option<usize>, buf: crate::mm::UserBuffer) -> usize {
        buf.len()
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> crate::fs::Stat {
        crate::fs::Stat::new(
            crate::makedev!(0, 5),
            1,
            crate::fs::StatMode::S_IFCHR.bits() | 0o666,
            1,
            crate::makedev!(10, 135),
            0,
            0,
            0,
            0,
        )
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(
        &self,
        dirnode_ptr: alloc::sync::Weak<crate::fs::directory_tree::DirectoryTreeNode>,
    ) {
    }

    fn get_dirtree_node(
        &self,
    ) -> Option<alloc::sync::Arc<crate::fs::directory_tree::DirectoryTreeNode>> {
        None
    }

    fn open(&self, flags: crate::fs::OpenFlags, special_use: bool) -> alloc::sync::Arc<dyn File> {
        alloc::sync::Arc::new(Hwclock {})
    }

    fn open_subfile(
        &self,
    ) -> Result<alloc::vec::Vec<(alloc::string::String, alloc::sync::Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(
        &self,
        name: &str,
        file_type: DiskInodeType,
    ) -> Result<alloc::sync::Arc<dyn File>, isize> {
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
        alloc::vec::Vec::new()
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
    ) -> Result<alloc::sync::Arc<spin::Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(
        &self,
    ) -> Result<alloc::vec::Vec<alloc::sync::Arc<spin::Mutex<crate::fs::PageCache>>>, ()> {
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

    fn ioctl(&self, _cmd: u32, _argp: usize) -> isize {
        SUCCESS
    }
}
