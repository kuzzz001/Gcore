//! /proc filesystem — dynamic pseudo-files
//!
//! Implements /proc/meminfo as a device-style file that generates
//! Linux-compatible memory statistics on every read.

use crate::fs::{
    directory_tree::DirectoryTreeNode, file_trait::File, layout::{Stat, StatMode},
    DiskInodeType,
};
use crate::mm::UserBuffer;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// /proc/meminfo — generates memory stats on-the-fly
pub struct ProcMeminfo;

impl File for ProcMeminfo {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(ProcMeminfo)
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        let content = generate_meminfo();
        let data = content.as_bytes();
        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
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
        generate_meminfo().len()
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            0,                                                      // st_dev
            2,                                                      // st_ino
            (StatMode::S_IFREG.bits() | 0o444) as u32,              // st_mode
            1,                                                      // st_nlink
            0,                                                      // st_rdev
            generate_meminfo().len() as i64,                        // st_size
            0,                                                      // st_atime_sec
            0,                                                      // st_mtime_sec
            0,                                                      // st_ctime_sec
        )
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let content = generate_meminfo();
        let data = content.as_bytes();
        let start = offset.unwrap_or(0);
        if start >= data.len() {
            return 0;
        }
        buf.write(&data[start..])
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

    fn open(&self, _flags: super::layout::OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(ProcMeminfo)
    }

    fn open_subfile(&self) -> Result<Vec<(alloc::string::String, Arc<dyn File>)>, isize> {
        Err(crate::syscall::errno::ENOTDIR)
    }

    fn create(
        &self,
        _name: &str,
        _file_type: DiskInodeType,
    ) -> Result<Arc<dyn File>, isize> {
        Err(crate::syscall::errno::ENOTDIR)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(crate::syscall::errno::ENOTDIR)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(crate::syscall::errno::EPERM)
    }

    fn get_dirent(&self, _count: usize) -> Vec<crate::fs::Dirent> {
        Vec::new()
    }

    fn lseek(
        &self,
        _offset: isize,
        _whence: super::layout::SeekWhence,
    ) -> Result<usize, isize> {
        Ok(0)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(crate::syscall::errno::EPERM)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(crate::syscall::errno::EPERM)
    }

    fn set_timestamp(
        &self,
        _ctime: Option<usize>,
        _atime: Option<usize>,
        _mtime: Option<usize>,
    ) {
    }

    fn get_single_cache(
        &self,
        _offset: usize,
    ) -> Result<Arc<spin::Mutex<super::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<spin::Mutex<super::cache::PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn ioctl(&self, _cmd: u32, _argp: usize) -> isize {
        crate::syscall::errno::ENOTTY
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        0
    }
}

/// Generate Linux-compatible /proc/meminfo content
fn generate_meminfo() -> alloc::string::String {
    // Query frame allocator stats via public re-export
    let total_frames = crate::config::MEMORY_SIZE / crate::config::PAGE_SIZE;
    let free_frames = crate::mm::unallocated_frames();
    let used_frames = total_frames.saturating_sub(free_frames);

    // Convert to kB (frames * 4KB / 1KB = frames * 4)
    let mem_total_kb = total_frames * 4;
    let mem_free_kb = free_frames * 4;
    let mem_available_kb = mem_free_kb; // simplified: available ≈ free

    // estimated — no swap, no real buffers/cached tracking
    let buffers_kb = 0u64;
    let cached_kb = 0u64;
    let swap_total_kb = 0u64;
    let swap_free_kb = 0u64;
    let slab_kb = 0u64;
    let anon_pages_kb = used_frames.saturating_sub(8192) as u64 * 4;
    let mapped_kb = 0u64;
    let shmem_kb = 0u64;
    let kernel_stack_kb = 0u64;
    let page_tables_kb = 0u64;
    let commit_limit_kb = mem_total_kb as u64;
    let committed_as_kb = used_frames as u64 * 4;
    let vmalloc_total_kb = 0u64;
    let hugepages_total = 0u64;
    let hugepages_free = 0u64;
    let hugepages_rsvd = 0u64;
    let hugepages_surp = 0u64;
    let hugepage_size = crate::config::PAGE_SIZE as u64 * 512; // 2MB huge page

    use alloc::format;
    format!(
        "MemTotal:       {:>8} kB\n\
         MemFree:        {:>8} kB\n\
         MemAvailable:   {:>8} kB\n\
         Buffers:        {:>8} kB\n\
         Cached:         {:>8} kB\n\
         SwapCached:     {:>8} kB\n\
         Active:         {:>8} kB\n\
         Inactive:       {:>8} kB\n\
         Active(anon):   {:>8} kB\n\
         Inactive(anon): {:>8} kB\n\
         Active(file):   {:>8} kB\n\
         Inactive(file): {:>8} kB\n\
         Unevictable:    {:>8} kB\n\
         Mlocked:        {:>8} kB\n\
         SwapTotal:      {:>8} kB\n\
         SwapFree:       {:>8} kB\n\
         Dirty:          {:>8} kB\n\
         Writeback:      {:>8} kB\n\
         AnonPages:      {:>8} kB\n\
         Mapped:         {:>8} kB\n\
         Shmem:          {:>8} kB\n\
         KReclaimable:   {:>8} kB\n\
         Slab:           {:>8} kB\n\
         SReclaimable:   {:>8} kB\n\
         SUnreclaim:     {:>8} kB\n\
         KernelStack:    {:>8} kB\n\
         PageTables:     {:>8} kB\n\
         NFS_Unstable:   {:>8} kB\n\
         Bounce:         {:>8} kB\n\
         WritebackTmp:   {:>8} kB\n\
         CommitLimit:    {:>8} kB\n\
         Committed_AS:   {:>8} kB\n\
         VmallocTotal:   {:>8} kB\n\
         VmallocUsed:    {:>8} kB\n\
         VmallocChunk:   {:>8} kB\n\
         Percpu:         {:>8} kB\n\
         HugePages_Total:{:>8}\n\
         HugePages_Free: {:>8}\n\
         HugePages_Rsvd: {:>8}\n\
         HugePages_Surp: {:>8}\n\
         Hugepagesize:   {:>8} kB\n\
         DirectMap4k:    {:>8} kB\n\
         DirectMap2M:    {:>8} kB\n\
         DirectMap1G:    {:>8} kB\n",
        mem_total_kb,
        mem_free_kb,
        mem_available_kb,
        buffers_kb,
        cached_kb,
        0u64,           // SwapCached
        used_frames as u64 * 4,   // Active
        0u64,           // Inactive
        used_frames as u64 * 4,   // Active(anon)
        0u64,           // Inactive(anon)
        0u64,           // Active(file)
        0u64,           // Inactive(file)
        0u64,           // Unevictable
        0u64,           // Mlocked
        swap_total_kb,
        swap_free_kb,
        0u64,           // Dirty
        0u64,           // Writeback
        anon_pages_kb,
        mapped_kb,
        shmem_kb,
        0u64,           // KReclaimable
        slab_kb,
        0u64,           // SReclaimable
        0u64,           // SUnreclaim
        kernel_stack_kb,
        page_tables_kb,
        0u64,           // NFS_Unstable
        0u64,           // Bounce
        0u64,           // WritebackTmp
        commit_limit_kb,
        committed_as_kb,
        vmalloc_total_kb,
        0u64,           // VmallocUsed
        0u64,           // VmallocChunk
        0u64,           // Percpu
        hugepages_total,
        hugepages_free,
        hugepages_rsvd,
        hugepages_surp,
        hugepage_size / 1024,
        mem_total_kb,   // DirectMap4k
        0u64,           // DirectMap2M
        0u64,           // DirectMap1G
    )
}
