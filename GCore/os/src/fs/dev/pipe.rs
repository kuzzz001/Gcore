use crate::fs::directory_tree::DirectoryTreeNode;
use crate::fs::dirent::Dirent;
use crate::fs::layout::Stat;
use crate::fs::DiskInodeType;
use crate::fs::StatMode;
use crate::syscall::errno::*;
use crate::task::block_current_and_run_next;
use crate::task::current_task;
use crate::task::wait_with_timeout;

use crate::timer::TimeSpec;
use crate::{fs::file_trait::File, mm::UserBuffer};
use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::ptr::copy_nonoverlapping;
use spin::Mutex;

pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
}

impl Pipe {
    pub fn read_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }
    pub fn write_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

#[cfg(feature = "board_fu740")]
const RING_DEFAULT_BUFFER_SIZE: usize = 4096 * 16;
#[cfg(not(feature = "board_fu740"))]
const RING_DEFAULT_BUFFER_SIZE: usize = 65536;

#[derive(Copy, Clone, PartialEq, Debug)]
enum RingBufferStatus {
    FULL,
    EMPTY,
    NORMAL,
}

pub struct PipeRingBuffer {
    arr: Box<[u8; RING_DEFAULT_BUFFER_SIZE]>,
    head: usize,
    tail: usize,
    status: RingBufferStatus,
    write_end: Option<Weak<Pipe>>,
    read_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    fn new() -> Self {
        // let mut vec = Vec::<u8>::with_capacity(RING_DEFAULT_BUFFER_SIZE);
        // unsafe {
        //     vec.set_len(RING_DEFAULT_BUFFER_SIZE);
        // }
        Self {
            arr: Box::new([0u8; RING_DEFAULT_BUFFER_SIZE]),
            head: 0,
            tail: 0,
            status: RingBufferStatus::EMPTY,
            write_end: None,
            read_end: None,
        }
    }
    #[allow(unused)]
    fn get_used_size(&self) -> usize {
        if self.status == RingBufferStatus::FULL {
            self.arr.len()
        } else if self.status == RingBufferStatus::EMPTY {
            0
        } else {
            assert!(self.head != self.tail);
            if self.head < self.tail {
                self.tail - self.head
            } else {
                self.tail + self.arr.len() - self.head
            }
        }
    }
    #[inline]
    fn buffer_read(&mut self, buf: &mut [u8]) -> usize {
        // get range
        let begin = self.head;
        let end = if self.tail <= self.head {
            RING_DEFAULT_BUFFER_SIZE
        } else {
            self.tail
        };
        // copy
        let read_bytes = buf.len().min(end - begin);
        unsafe {
            copy_nonoverlapping(self.arr.as_ptr().add(begin), buf.as_mut_ptr(), read_bytes);
        };
        // update head
        self.head = if begin + read_bytes == RING_DEFAULT_BUFFER_SIZE {
            0
        } else {
            begin + read_bytes
        };
        read_bytes
    }
    #[inline]
    fn buffer_write(&mut self, buf: &[u8]) -> usize {
        // get range
        let begin = self.tail;
        let end = if self.tail < self.head {
            self.head
        } else {
            RING_DEFAULT_BUFFER_SIZE
        };
        // write
        let write_bytes = buf.len().min(end - begin);
        unsafe {
            copy_nonoverlapping(buf.as_ptr(), self.arr.as_mut_ptr().add(begin), write_bytes);
        };
        // update tail
        self.tail = if begin + write_bytes == RING_DEFAULT_BUFFER_SIZE {
            0
        } else {
            begin + write_bytes
        };
        write_bytes
    }
    fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }
    fn set_read_end(&mut self, read_end: &Arc<Pipe>) {
        self.read_end = Some(Arc::downgrade(read_end));
    }
    fn all_write_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
    fn all_read_ends_closed(&self) -> bool {
        self.read_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Return (read_end, write_end)
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(Mutex::new(PipeRingBuffer::new()));
    // buffer仅剩两个强引用，这样读写端关闭后就会被释放
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    buffer.lock().set_write_end(&write_end);
    buffer.lock().set_read_end(&read_end);
    (read_end, write_end)
}

#[allow(unused)]
impl File for Pipe {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(Pipe::read_end_with_buffer(self.buffer.clone()))
    }

    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }

    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        let mut read_size = 0usize;
        loop {
            let task = current_task().unwrap();
            let inner = task.acquire_inner_lock();
            // if !inner.sigpending.difference(inner.sigmask).is_empty() {
            //     return ERESTART as usize;
            // }
            drop(inner);
            drop(task);
            let mut ring = self.buffer.lock();
            if ring.status == RingBufferStatus::EMPTY {
                if ring.all_write_ends_closed() {
                    return read_size;
                }
                drop(ring);
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                // If there are pending unblocked signals, return immediately
                // so that the trap-return path can deliver them via do_signal().
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return read_size;
                }
                drop(inner);
                wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
                drop(task);
                block_current_and_run_next();
                // After resuming, check if we were woken by a signal
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return read_size;
                }
                drop(inner);
                drop(task);
                continue;
            }
            // We guarantee that this operation will read at least one byte
            while read_size < buf.len() {
                let read_bytes = ring.buffer_read(&mut buf[read_size..]);
                read_size += read_bytes;
                if ring.head == ring.tail {
                    ring.status = RingBufferStatus::EMPTY;
                    drop(ring);
                    // wake_all_interruptible removed - too aggressive
                    return read_size;
                }
            }

            ring.status = RingBufferStatus::NORMAL;
            drop(ring);
            // wake_all_interruptible removed - too aggressive
            return read_size;
        }
    }

    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        let mut write_size = 0usize;

        loop {
            let task = current_task().unwrap();
            let inner = task.acquire_inner_lock();
            // if !inner.sigpending.difference(inner.sigmask).is_empty() {
            //     return ERESTART as usize;
            // }
            drop(inner);
            drop(task);
            let mut ring = self.buffer.lock();
            if ring.status == RingBufferStatus::FULL {
                if ring.all_read_ends_closed() {
                    return write_size;
                }
                drop(ring);
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return write_size;
                }
                drop(inner);
                wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
                drop(task);
                block_current_and_run_next();
                // After resuming, check if we were woken by a signal
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return write_size;
                }
                drop(inner);
                drop(task);
                continue;
            }
            // We guarantee that this operation will write at least one byte
            // So we modify status first
            while write_size < buf.len() {
                let write_bytes = ring.buffer_write(&buf[write_size..]);
                write_size += write_bytes;
                if ring.head == ring.tail {
                    ring.status = RingBufferStatus::FULL;
                    drop(ring);
                    // wake_all_interruptible removed - too aggressive
                    return write_size;
                }
            }
            ring.status = RingBufferStatus::NORMAL;
            drop(ring);
            // wake_all_interruptible removed - too aggressive
            return write_size;
        }
    }

    fn r_ready(&self) -> bool {
        let ring_buffer = self.buffer.lock();
        ring_buffer.status != RingBufferStatus::EMPTY
    }

    fn w_ready(&self) -> bool {
        let ring_buffer = self.buffer.lock();
        ring_buffer.status != RingBufferStatus::FULL
    }

    fn read_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        let mut read_size = 0usize;
        loop {
            let task = current_task().unwrap();
            let inner = task.acquire_inner_lock();
            // 注释掉下面内容，pipe测例通过，跟读出pipe内容有关
            // if !inner.sigpending.difference(inner.sigmask).is_empty() {
            //     return ERESTART as usize;
            // }
            drop(inner);
            drop(task);
            let mut ring = self.buffer.lock();
            if ring.status == RingBufferStatus::EMPTY {
                if ring.all_write_ends_closed() {
                    return read_size;
                }
                drop(ring);
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return read_size;
                }
                drop(inner);
                wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
                drop(task);
                block_current_and_run_next();
                // After resuming, check if we were woken by a signal
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return read_size;
                }
                drop(inner);
                drop(task);
                continue;
            }
            // We guarantee that this operation will read at least one byte
            // So we modify status first
            for buf in buf.buffers {
                let mut buf_start = 0;
                while buf_start < buf.len() {
                    let read_bytes = ring.buffer_read(&mut buf[buf_start..]);
                    buf_start += read_bytes;
                    if ring.head == ring.tail {
                        ring.status = RingBufferStatus::EMPTY;
                        read_size += buf_start;
                        drop(ring);
                        // wake_all_interruptible removed - too aggressive
                        return read_size;
                    }
                }
                read_size += buf_start;
            }
            ring.status = RingBufferStatus::NORMAL;
            drop(ring);
            // wake_all_interruptible removed - too aggressive
            return read_size;
        }
    }

    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        let mut write_size = 0usize;
        loop {
            let task = current_task().unwrap();
            let inner = task.acquire_inner_lock();
            // if !inner.sigpending.difference(inner.sigmask).is_empty() {
            //     return ERESTART as usize;
            // }
            drop(inner);
            drop(task);
            let mut ring = self.buffer.lock();
            if ring.status == RingBufferStatus::FULL {
                if ring.all_read_ends_closed() {
                    return write_size;
                }
                drop(ring);
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return write_size;
                }
                drop(inner);
                wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
                drop(task);
                block_current_and_run_next();
                // After resuming, check if we were woken by a signal
                let task = current_task().unwrap();
                let inner = task.acquire_inner_lock();
                if !inner.sigpending.difference(inner.sigmask).is_empty() {
                    drop(inner);
                    drop(task);
                    return write_size;
                }
                drop(inner);
                drop(task);
                continue;
            }
            // We guarantee that this operation will write at least one byte
            // So we modify status first
            for buf in buf.buffers {
                let mut buf_start = 0;
                while buf_start < buf.len() {
                    let write_bytes = ring.buffer_write(&buf[buf_start..]);
                    buf_start += write_bytes;
                    if ring.head == ring.tail {
                        ring.status = RingBufferStatus::FULL;
                        write_size += buf_start;
                        drop(ring);
                        // wake_all_interruptible removed - too aggressive
                        return write_size;
                    }
                }
                write_size += buf_start;
            }
            ring.status = RingBufferStatus::NORMAL;
            drop(ring);
            // wake_all_interruptible removed - too aggressive
            return write_size;
        }
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(8, 0),
            1,
            StatMode::S_IFIFO.bits() | 0o666,
            1,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, dirnode_ptr: Weak<crate::fs::directory_tree::DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, flags: crate::fs::layout::OpenFlags, special_use: bool) -> Arc<dyn File> {
        self.deep_clone()
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

    fn get_single_cache(&self, offset: usize) -> Result<Arc<Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<alloc::vec::Vec<Arc<Mutex<crate::fs::PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        // The peer has closed its end.
        // Or maybe you should only check whether both ends have been closed by the peer.
        if self.readable {
            self.buffer.lock().all_write_ends_closed()
        } else {
            //writable
            self.buffer.lock().all_read_ends_closed()
        }
    }

    fn fcntl(&self, cmd: u32, arg: u32) -> isize {
        EINVAL
    }
}
