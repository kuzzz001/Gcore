use super::{config::NET_INTERFACE, Socket};
use crate::{
    fs::file_trait::File, fs::OpenFlags,
    utils::error::{SyscallErr, SyscallRet},
};
use alloc::{sync::Arc, vec};
use log::info;
use smoltcp::{
    iface::SocketHandle,
    socket::icmp,
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};

use crate::mm::UserBuffer;
use crate::fs::Stat;
use crate::fs::StatMode;
use crate::fs::DiskInodeType;
use crate::syscall::errno::*;
use alloc::sync::Weak;
use crate::fs::directory_tree::DirectoryTreeNode;
use alloc::vec::Vec;
use alloc::string::String;
use crate::fs::dirent::Dirent;
use crate::fs::SeekWhence;
use crate::fs::fat32::PageCache;

pub struct IcmpSocket {
    socket_handler: SocketHandle,
}

impl Socket for IcmpSocket {
    fn bind(&self, addr: IpListenEndpoint) -> SyscallRet {
        info!("[Icmp::bind] bind to {:?}", addr);
        NET_INTERFACE.poll();
        let endpoint: icmp::Endpoint = match addr.addr {
            Some(IpAddress::Ipv4(_)) => icmp::Endpoint::Ident(0),
            _ => icmp::Endpoint::Unspecified,
        };
        NET_INTERFACE.inner_handler(|inner| {
            let socket = inner.sockets.get_mut::<icmp::Socket>(self.socket_handler);
            socket.bind(endpoint).map_err(|_| SyscallErr::EINVAL)
        })?;
        NET_INTERFACE.poll();
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

    fn socket_type(&self) -> super::SocketType {
        super::SocketType::SOCK_RAW
    }

    fn recv_buf_size(&self) -> usize { 65536 }
    fn send_buf_size(&self) -> usize { 65536 }
    fn set_recv_buf_size(&self, _size: usize) {}
    fn set_send_buf_size(&self, _size: usize) {}

    fn loacl_endpoint(&self) -> IpListenEndpoint {
        IpListenEndpoint { addr: None, port: 0 }
    }

    fn remote_endpoint(&self) -> Option<IpEndpoint> {
        None
    }

    fn shutdown(&self, _how: u32) -> crate::utils::error::GeneralRet<()> {
        Ok(())
    }

    fn set_nagle_enabled(&self, _enabled: bool) -> SyscallRet { Ok(0) }
    fn set_keep_alive(&self, _enabled: bool) -> SyscallRet { Ok(0) }
}

impl IcmpSocket {
    pub fn new() -> Self {
        let rx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 8], vec![0u8; 65536]);
        let tx_buf = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 8], vec![0u8; 65536]);
        let socket = icmp::Socket::new(rx_buf, tx_buf);
        let socket_handler = NET_INTERFACE.add_socket(socket);
        info!("[IcmpSocket::new] {}", socket_handler);
        NET_INTERFACE.poll();
        Self { socket_handler }
    }
}

impl File for IcmpSocket {
    fn deep_clone(&self) -> Arc<dyn File> { Arc::new(IcmpSocket::new()) }
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { true }

    fn read(&self, _offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        NET_INTERFACE.poll();
        let ret = NET_INTERFACE.inner_handler(|inner| {
            let socket = inner.sockets.get_mut::<icmp::Socket>(self.socket_handler);
            if !socket.can_recv() {
                return 0;
            }
            match socket.recv_slice(buf) {
                Ok((n, _addr)) => n,
                Err(_) => 0,
            }
        });
        NET_INTERFACE.poll();
        ret
    }

    fn write(&self, _offset: Option<&mut usize>, buf: &[u8]) -> usize {
        NET_INTERFACE.poll();
        // ICMP send needs a destination IP address. Default to loopback.
        let ret = NET_INTERFACE.inner_handler(|inner| {
            let socket = inner.sockets.get_mut::<icmp::Socket>(self.socket_handler);
            if !socket.can_send() {
                return 0;
            }
            match socket.send_slice(buf, IpAddress::v4(127, 0, 0, 1)) {
                Ok(()) => buf.len(),
                Err(_) => 0,
            }
        });
        NET_INTERFACE.poll();
        ret
    }

    fn r_ready(&self) -> bool {
        NET_INTERFACE.poll();
        let ret = NET_INTERFACE.inner_handler(|inner| {
            inner.sockets.get_mut::<icmp::Socket>(self.socket_handler).can_recv()
        });
        NET_INTERFACE.poll();
        ret
    }
    fn w_ready(&self) -> bool {
        NET_INTERFACE.poll();
        let ret = NET_INTERFACE.inner_handler(|inner| {
            inner.sockets.get_mut::<icmp::Socket>(self.socket_handler).can_send()
        });
        NET_INTERFACE.poll();
        ret
    }
    fn read_user(&self, _offset: Option<usize>, buf: UserBuffer) -> usize {
        let mut buffers = buf.buffers;
        let buf = unsafe { core::slice::from_raw_parts_mut(buffers[0].as_mut_ptr() as *mut u8, buf.len as usize) };
        self.read(None, buf)
    }
    fn write_user(&self, _offset: Option<usize>, buf: UserBuffer) -> usize {
        let mut buffers = buf.buffers;
        let buf = unsafe { core::slice::from_raw_parts_mut(buffers[0].as_mut_ptr() as *mut u8, buf.len as usize) };
        self.write(None, buf)
    }
    fn get_size(&self) -> usize { 0 }
    fn get_stat(&self) -> Stat {
        Stat::new(crate::makedev!(0, 7), 1, StatMode::S_IFSOCK.bits() | 0o666, 1, 0, 0, 0, 0, 0)
    }
    fn get_file_type(&self) -> DiskInodeType { DiskInodeType::File }
    fn is_dir(&self) -> bool { false }
    fn is_file(&self) -> bool { true }
    fn info_dirtree_node(&self, _dirnode_ptr: Weak<DirectoryTreeNode>) {}
    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> { None }
    fn open(&self, _flags: OpenFlags, _special_use: bool) -> Arc<dyn File> { self.deep_clone() }
    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> { Err(ENOTDIR) }
    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> { Err(EINVAL) }
    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize> { Err(EINVAL) }
    fn unlink(&self, _delete: bool) -> Result<(), isize> { Err(EINVAL) }
    fn get_dirent(&self, _count: usize) -> Vec<Dirent> { Vec::new() }
    fn get_offset(&self) -> usize { 0 }
    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, isize> { Err(ESPIPE) }
    fn modify_size(&self, _diff: isize) -> Result<(), isize> { Ok(()) }
    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> { Err(EINVAL) }
    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {}
    fn get_single_cache(&self, _offset: usize) -> Result<Arc<spin::Mutex<PageCache>>, ()> { Err(()) }
    fn get_all_caches(&self) -> Result<Vec<Arc<spin::Mutex<PageCache>>>, ()> { Err(()) }
    fn oom(&self) -> usize { 0 }
    fn hang_up(&self) -> bool { false }
    fn ioctl(&self, _cmd: u32, _argp: usize) -> isize { ENOTTY }
    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize { EINVAL }
}

impl Drop for IcmpSocket {
    fn drop(&mut self) {
        info!("[IcmpSocket::drop] drop socket {}", self.socket_handler);
        NET_INTERFACE.poll();
        NET_INTERFACE.remove(self.socket_handler);
        NET_INTERFACE.poll();
    }
}
