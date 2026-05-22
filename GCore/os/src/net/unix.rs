use super::Mutex;
use super::Socket;
use super::SocketType;
use crate::{
    fs::{
        dev::pipe::{make_pipe, Pipe},
        file_trait::File,
        OpenFlags,
    },
    utils::error::{GeneralRet, SyscallErr, SyscallRet},
};
use alloc::sync::Arc;
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use crate::mm::UserBuffer;
use crate::fs::Stat;
use crate::fs::StatMode;
use crate::fs::DiskInodeType;
use crate::fs::directory_tree::DirectoryTreeNode;
use crate::fs::Dirent;
use crate::fs::SeekWhence;
use crate::fs::fat32::PageCache;
use crate::syscall::errno::*;
use alloc::sync::Weak;
use alloc::vec::Vec;
use alloc::string::String;

pub struct UnixSocket {
    read_end: Arc<Pipe>,
    write_end: Arc<Pipe>,
    socket_type: SocketType,
    shut_rd: Mutex<bool>,
    shut_wr: Mutex<bool>,
    recv_buf_size: Mutex<usize>,
    send_buf_size: Mutex<usize>,
}

const UNIX_BUF_SIZE: usize = 256;

impl Socket for UnixSocket {
    fn bind(&self, _addr: IpListenEndpoint) -> SyscallRet {
        Ok(0)
    }

    fn listen(&self) -> SyscallRet {
        Ok(0)
    }

    fn connect(&self, _addr_buf: &[u8]) -> SyscallRet {
        Ok(0)
    }

    fn accept(&self, _sockfd: u32, _addr: usize, _addrlen: usize) -> SyscallRet {
        Err(SyscallErr::EOPNOTSUPP)
    }

    fn socket_type(&self) -> SocketType {
        self.socket_type
    }

    fn recv_buf_size(&self) -> usize {
        *self.recv_buf_size.lock()
    }

    fn send_buf_size(&self) -> usize {
        *self.send_buf_size.lock()
    }

    fn set_recv_buf_size(&self, size: usize) {
        *self.recv_buf_size.lock() = size;
    }

    fn set_send_buf_size(&self, size: usize) {
        *self.send_buf_size.lock() = size;
    }

    fn loacl_endpoint(&self) -> IpListenEndpoint {
        IpListenEndpoint {
            addr: smoltcp::wire::IpAddress::v4(0, 0, 0, 0).into(),
            port: 0,
        }
    }

    fn remote_endpoint(&self) -> Option<IpEndpoint> {
        None
    }

    fn shutdown(&self, how: u32) -> GeneralRet<()> {
        if how == super::SHUT_RD || how == super::SHUT_RDWR {
            *self.shut_rd.lock() = true;
        }
        if how == super::SHUT_WR || how == super::SHUT_RDWR {
            *self.shut_wr.lock() = true;
        }
        Ok(())
    }

    fn set_nagle_enabled(&self, _enabled: bool) -> SyscallRet {
        Err(SyscallErr::EOPNOTSUPP)
    }

    fn set_keep_alive(&self, _enabled: bool) -> SyscallRet {
        Err(SyscallErr::EOPNOTSUPP)
    }
}

impl UnixSocket {
    pub fn new(read_end: Arc<Pipe>, write_end: Arc<Pipe>, socket_type: SocketType) -> Self {
        Self {
            read_end,
            write_end,
            socket_type,
            shut_rd: Mutex::new(false),
            shut_wr: Mutex::new(false),
            recv_buf_size: Mutex::new(UNIX_BUF_SIZE),
            send_buf_size: Mutex::new(UNIX_BUF_SIZE),
        }
    }
}

impl File for UnixSocket {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(UnixSocket {
            read_end: self.read_end.clone(),
            write_end: self.write_end.clone(),
            socket_type: self.socket_type,
            shut_rd: Mutex::new(*self.shut_rd.lock()),
            shut_wr: Mutex::new(*self.shut_wr.lock()),
            recv_buf_size: Mutex::new(*self.recv_buf_size.lock()),
            send_buf_size: Mutex::new(*self.send_buf_size.lock()),
        })
    }

    fn readable(&self) -> bool {
        !*self.shut_rd.lock() && self.read_end.readable()
    }

    fn writable(&self) -> bool {
        !*self.shut_wr.lock() && self.write_end.writable()
    }

    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        if *self.shut_rd.lock() {
            return 0;
        }
        self.read_end.read(offset, buf)
    }

    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        if *self.shut_wr.lock() {
            return EPIPE as usize;
        }
        self.write_end.write(offset, buf)
    }

    fn r_ready(&self) -> bool {
        !*self.shut_rd.lock() && self.read_end.r_ready()
    }

    fn w_ready(&self) -> bool {
        !*self.shut_wr.lock() && self.write_end.w_ready()
    }

    fn read_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        if *self.shut_rd.lock() {
            return 0;
        }
        self.read_end.read_user(offset, buf)
    }

    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        if *self.shut_wr.lock() {
            return EPIPE as usize;
        }
        self.write_end.write_user(offset, buf)
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 6),
            1,
            StatMode::S_IFSOCK.bits() | 0o666,
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

    fn info_dirtree_node(&self, _dirnode_ptr: Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> {
        Arc::new(UnixSocket {
            read_end: self.read_end.clone(),
            write_end: self.write_end.clone(),
            socket_type: self.socket_type,
            shut_rd: Mutex::new(false),
            shut_wr: Mutex::new(false),
            recv_buf_size: Mutex::new(UNIX_BUF_SIZE),
            send_buf_size: Mutex::new(UNIX_BUF_SIZE),
        })
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(EOPNOTSUPP)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> {
        Err(EOPNOTSUPP)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EOPNOTSUPP)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, isize> {
        Err(ESPIPE)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EOPNOTSUPP)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EOPNOTSUPP)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<Mutex<PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        *self.shut_rd.lock() || *self.shut_wr.lock()
            || self.read_end.hang_up()
            || self.write_end.hang_up()
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        EINVAL
    }
}

pub fn make_unix_socket_pair(socket_type: SocketType) -> (Arc<UnixSocket>, Arc<UnixSocket>) {
    let (read1, write1) = make_pipe();
    let (read2, write2) = make_pipe();
    let socket1 = Arc::new(UnixSocket::new(read1, write2, socket_type));
    let socket2 = Arc::new(UnixSocket::new(read2, write1, socket_type));
    (socket1, socket2)
}
