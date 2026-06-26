use crate::fs::cache::BlockCacheManager;
use crate::fs::BlockDevice;
use alloc::sync::Arc;
use alloc::vec::Vec;
use downcast_rs::{impl_downcast, DowncastSync};

use super::directory_tree::ROOT;
use super::ext4::{ext4fs::Ext4FileSystem, layout::Ext4OSInode, ROOT_INODE};
use super::fat32::{EasyFileSystem, FatInode, FatOSInode};
use super::file_trait::File;
use super::filesystem::{pre_mount, FS_Type};

pub trait VFS: DowncastSync {
    fn close(&self) {
        unreachable!()
    }

    fn read(&self) -> Vec<u8> {
        unreachable!()
    }

    fn write(&self, _data: Vec<u8>) -> usize {
        unreachable!()
    }

    fn get_direcotry(&self) -> ROOT {
        unreachable!()
    }

    fn alloc_blocks(&self, blocks: usize) -> Vec<usize>;

    fn get_filesystem_type(&self) -> FS_Type;

    fn block_size(&self) -> usize;
}
impl_downcast!(sync VFS);

impl dyn VFS {
    pub fn open_fs(
        block_device: Arc<dyn BlockDevice>,
        index_cache_mgr: Arc<spin::Mutex<BlockCacheManager>>,
    ) -> Arc<dyn VFS> {
        let fs_type = pre_mount();
        match fs_type {
            FS_Type::Fat32 => EasyFileSystem::open(block_device, index_cache_mgr),
            FS_Type::Ext4 => Arc::new(Ext4FileSystem::open_ext4rs(block_device, index_cache_mgr)),
            FS_Type::Null => panic!("no filesystem found"),
        }
    }
    pub fn root_osinode(vfs: &Arc<dyn VFS>) -> Arc<dyn File> {
        match vfs.get_filesystem_type() {
            FS_Type::Fat32 => FatOSInode::new(FatInode::root_inode(vfs)),
            FS_Type::Ext4 => {
                let vfs_concrete = Arc::downcast::<Ext4FileSystem>(vfs.clone()).unwrap();
                let root_inode = vfs_concrete.get_inode_ref(ROOT_INODE);
                Ext4OSInode::new(root_inode, vfs_concrete)
            }
            FS_Type::Null => panic!("Null filesystem type does not have a root inode"),
        }
    }
}

pub trait VFSFileContent {}

pub trait VFSDirEnt {}
