use crate::mm::{copy_to_user, get_from_user, translated_byte_buffer, translated_ref, translated_refmut};
use crate::{
    fs::FileDescriptor, net::{
        address::{self, SocketAddrv4},
        make_unix_socket_pair, Socket, SocketType, TCP_MSS,
    }, 
    task::current_task,
};
use super::errno::*;

use log::info;
use smoltcp::wire::IpListenEndpoint;
/// level
const SOL_SOCKET: u32 = 1;
const SOL_TCP: u32 = 6;
/// option name
const TCP_NODELAY: u32 = 1;
const TCP_MAXSEG: u32 = 2;
#[allow(unused)]
const TCP_INFO: u32 = 11;
const TCP_CONGESTION: u32 = 13;
const SO_SNDBUF: u32 = 7;
const SO_RCVBUF: u32 = 8;
const SO_KEEPALIVE: u32 = 9;
const SO_REUSEADDR: u32 = 2;

pub fn sys_socket(domain: u32, socket_type: u32, protocol: u32) -> isize {
    info!(
        "[sys_socket] domain: {}, type: {}, protocol: {}",
        domain, socket_type, protocol
    );
    let result = match <dyn Socket>::alloc(domain, socket_type){
        Ok(sockfd) => {
            info!("[sys_socket] new sockfd: {}", sockfd);
            sockfd as isize
        },
        Err(e) => {
            info!("[sys_socket] new sockfd failed");
            -(e as isize)
        }
    };
    result
}

pub fn sys_bind(sockfd: u32, addr: usize, addrlen: u32) -> isize {
    let addr_buf = trans_ref!(addr, addrlen);
    let socket = get_socket!(sockfd);
    let endpoint = match address::listen_endpoint(addr_buf) {
        Ok(ep) => ep,
        Err(_) => {
            info!("[sys_bind] cannot parse address as IP endpoint, using dummy");
            IpListenEndpoint { addr: None, port: 0 }
        }
    };
    match socket.socket_type() {
        SocketType::SOCK_STREAM => match socket.bind(endpoint) {
            Ok(v) => v as isize,
            Err(e) => -(e as isize),
        },
        SocketType::SOCK_DGRAM => {
            let res = current_task().unwrap().socket_table.lock().can_bind(endpoint);
            if res.is_none(){
                info!("[sys_bind] not find port exist");
                match socket.bind(endpoint) {
                    Ok(v) => v as isize,
                    Err(e) => -(e as isize),
                }
            }else {
                let (_,sock) = res.unwrap();
                current_task().unwrap().socket_table.lock().insert(sockfd as usize, sock.clone());
                let _ = current_task().unwrap().files.lock().insert(FileDescriptor::new(false,false,sock));
                0
            }
        }
        _ => {
            info!("[sys_bind] unsupported socket type: {:?}", socket.socket_type());
            return EINVAL;
        }
    }
}

pub fn sys_listen(sockfd: u32, _backlog: u32) -> isize {
    let socket = get_socket!(sockfd);
    match socket.listen() {
        Ok(v) => v as isize,
        Err(e) => -(e as isize),
    }
}

pub  fn sys_accept(sockfd: u32, addr: usize, addrlen: usize) -> isize {
    let socket = get_socket!(sockfd);
    match socket.accept(sockfd, addr, addrlen) {
        Ok(v) => v as isize,
        Err(e) => -(e as isize),
    }
}

pub  fn sys_connect(sockfd: u32, addr: usize, addrlen: u32) -> isize {
    let addr_buf = trans_ref!(addr, addrlen);
    let task = current_task().unwrap();
    let nonblock = task.files.lock().get_ref(sockfd as usize)
        .map(|fd| fd.get_nonblock())
        .unwrap_or(false);
    let socket = get_socket!(sockfd);
    match socket.connect(addr_buf) {
        Ok(v) => v as isize,
        // For nonblocking sockets, connection in progress is expected
        Err(e) if nonblock => EINPROGRESS,
        Err(e) => -(e as isize),
    }
}

pub fn sys_getsockname(sockfd: u32, addr: usize, addrlen: usize) -> isize {
    let socket = get_socket!(sockfd);
    match socket.addr(addr, addrlen) {
        Ok(v) => v as isize,
        Err(e) => -(e as isize),
    }
}

pub fn sys_getpeername(sockfd: u32, addr: usize, addrlen: usize) -> isize {
    let socket = get_socket!(sockfd);
    match socket.peer_addr(addr, addrlen) {
        Ok(v) => v as isize,
        Err(e) => -(e as isize),
    }
}

pub fn sys_sendto(
    sockfd: u32,
    buf: usize,
    len: usize,
    _flags: u32,
    dest_addr: usize,
    addrlen: u32,
) -> isize {
    let task = current_task().unwrap();
    let socket_file = match task.files.lock().get_ref(sockfd as usize) {
        Ok(file) => file.clone(),
        Err(e) => return e,
    };
    let buf = trans_ref!(buf, len);
    let socket = get_socket!(sockfd);
    log::info!("[sys_sendto] get socket sockfd: {}", sockfd);
    let mut offset = 0 as usize; 
    let len = match socket.socket_type() {
        SocketType::SOCK_STREAM | SocketType::SOCK_RAW => socket_file.file.write(Some(&mut offset),buf),
        SocketType::SOCK_DGRAM => {
            info!("[sys_sendto] socket is udp");
            if socket.loacl_endpoint().port == 0 {
                let addr = SocketAddrv4::new([0; 16].as_slice());
                let endpoint = IpListenEndpoint::from(addr);
                let _ = socket.bind(endpoint);
            }
            let dest_addr = trans_ref!(dest_addr, addrlen);
            let _ = socket.connect(dest_addr);
            socket_file.file.write(Some(&mut offset),buf)
        }
        _ => return EINVAL,
    };
    len as isize
}

pub  fn sys_recvfrom(
    sockfd: u32,
    buf: usize,
    len: u32,
    _flags: u32,
    src_addr: usize,
    addrlen: usize,
) -> isize {
    let socket_file = current_task().unwrap().files.lock().get_ref(sockfd as usize).unwrap().clone();
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let buf = translated_refmut(token, buf as *mut u8).unwrap();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, len as usize) };
    let socket = get_socket!(sockfd);

    info!("[sys_recvfrom] get socket sockfd: {}", sockfd);

    let mut offset = 0 as usize;
    match socket.socket_type() {
        SocketType::SOCK_STREAM | SocketType::SOCK_DGRAM | SocketType::SOCK_RAW => {
            let len = socket_file.file.read(Some(&mut offset),buf);
            if src_addr != 0 {
                let _ = socket.peer_addr(src_addr, addrlen);
            }
            len as isize
        }
        _ => return EINVAL,
    }
}

pub fn sys_getsockopt(
    sockfd: u32,
    level: u32,
    optname: u32,
    optval_ptr_: usize,
    optlen: usize,
) -> isize {
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let optval_ptr = translated_refmut(token, optval_ptr_ as *mut u32).unwrap();
    let optlen = translated_refmut(token, optlen as *mut u32).unwrap();
    match (level, optname) {
        (SOL_TCP, TCP_MAXSEG) => {
            // return max tcp fregment size (MSS)
            let len = core::mem::size_of::<u32>();
            unsafe {
                *(optval_ptr as *mut u32) = TCP_MSS;
                *(optlen as *mut u32) = len as u32;
            }
        }
        (SOL_TCP, TCP_CONGESTION) => {
            let optval_ptr = translated_refmut(token, optval_ptr_ as *mut u8).unwrap();
            let congestion = "reno";
            let buf =
                unsafe { core::slice::from_raw_parts_mut(optval_ptr as *mut u8, congestion.len()) };
            buf.copy_from_slice(congestion.as_bytes());
            unsafe {
                *(optlen as *mut u32) = congestion.len() as u32;
            }
        }
        (SOL_SOCKET, SO_SNDBUF | SO_RCVBUF) => {
            let socket = get_socket!(sockfd);
            match optname {
                SO_SNDBUF => {
                    let size = socket.send_buf_size();
                    unsafe {
                        *(optval_ptr as *mut u32) = size as u32;
                        *(optlen as *mut u32) = 4;
                    }
                }
                SO_RCVBUF => {
                    let size = socket.recv_buf_size();
                    unsafe {
                        *(optval_ptr as *mut u32) = size as u32;
                        *(optlen as *mut u32) = 4;
                    }
                }
                _ => {}
            }
        }
        (SOL_SOCKET, SO_REUSEADDR) => {
            unsafe {
                *(optval_ptr as *mut u32) = 1;
                *(optlen as *mut u32) = 4;
            }
        }
        _ => {
            log::warn!("[sys_getsockopt] level: {}, optname: {}", level, optname);
        }
    }
    0 as isize
}

pub fn sys_setsockopt(
    sockfd: u32,
    level: u32,
    optname: u32,
    optval_ptr: usize,
    _optlen: u32,
) -> isize {
    let socket = get_socket!(sockfd);
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let optval_ptr = translated_refmut(token, optval_ptr as *mut u32).unwrap();
    match (level, optname) {
        (SOL_SOCKET, SO_SNDBUF | SO_RCVBUF) => {
            let size = unsafe { *(optval_ptr as *mut u32) };
            match optname {
                SO_SNDBUF => {
                    socket.set_send_buf_size(size as usize);
                }
                SO_RCVBUF => {
                    socket.set_recv_buf_size(size as usize);
                }
                _ => {}
            }
        }
        (SOL_TCP, TCP_NODELAY) => {
            // close Nagle’s Algorithm
            let enabled = unsafe { *(optval_ptr as *const u32) };
            log::debug!("[sys_setsockopt] set TCPNODELY: {}", enabled);
            let _ = match enabled {
                0 => socket.set_nagle_enabled(true),
                _ => socket.set_nagle_enabled(false),
            };
        }
        (SOL_SOCKET, SO_KEEPALIVE) => {
            let enabled = unsafe { *(optval_ptr as *const u32) };
            log::debug!("[sys_setsockopt] set socket KEEPALIVE: {}", enabled);
            let _ = match enabled {
                1 => socket.set_keep_alive(true),
                _ => socket.set_keep_alive(false),
            };
        }
        (SOL_SOCKET, SO_REUSEADDR) => {
            // SO_REUSEADDR is a no-op — always allow port reuse
        }
        _ => {
            log::warn!("[sys_setsockopt] level: {}, optname: {}", level, optname);
        }
    }
    0 as isize
}

pub fn sys_sock_shutdown(sockfd: u32, how: u32) -> isize {
    log::info!("[sys_shutdown] sockfd {}, how {}", sockfd, how);
    let socket = get_socket!(sockfd);
    let _ = socket.shutdown(how);
    0 as isize
}

pub fn sys_socketpair(domain: u32, socket_type: u32, protocol: u32, sv: usize) -> isize {
    info!(
        "[sys_socketpair] domain {}, type {}, protocol {}, sv {}",
        domain, socket_type, protocol, sv
    );
    if domain as u16 != crate::net::AF_UNIX {
        return EAFNOSUPPORT;
    }
    let socket_type = match SocketType::from_bits(socket_type) {
        Some(t) => t,
        None => return EINVAL,
    };
    let cloexec = socket_type.contains(SocketType::SOCK_CLOEXEC);
    let nonblock = socket_type.contains(SocketType::SOCK_NONBLOCK);
    let base_type = if socket_type.contains(SocketType::SOCK_STREAM) {
        SocketType::SOCK_STREAM
    } else if socket_type.contains(SocketType::SOCK_DGRAM) {
        SocketType::SOCK_DGRAM
    } else {
        return EINVAL;
    };
    let len = 2 * core::mem::size_of::<u32>();
    let sv = unsafe { core::slice::from_raw_parts_mut(sv as *mut u32, len) };
    let (socket1, socket2) = make_unix_socket_pair(base_type);
    let fd1 = current_task().unwrap().files.lock().insert(FileDescriptor::new(cloexec, nonblock, socket1));
    let fd2 = current_task().unwrap().files.lock().insert(FileDescriptor::new(cloexec, nonblock, socket2));
    sv[0] = fd1.unwrap() as u32;
    sv[1] = fd2.unwrap() as u32;
    info!("[sys_socketpair] new sv: {:?}", sv);
    0 as isize
}

/// msghdr for sendmsg/recvmsg (riscv64 ABI: 56 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
struct MsgHdr {
    msg_name: usize,       // optional address
    msg_namelen: u32,      // size of address
    _pad: u32,             // padding
    msg_iov: usize,        // scatter/gather array ptr
    msg_iovlen: usize,     // elements in msg_iov
    msg_control: usize,    // ancillary data
    msg_controllen: usize, // ancillary data buffer len
    msg_flags: i32,        // flags on received message
    _pad2: u32,            // padding
}

/// iovec for sendmsg/recvmsg (riscv64 ABI: 16 bytes)
#[repr(C)]
struct Iov {
    iov_base: usize,
    iov_len: usize,
}

/// Gather data from Iov scatter-gather list into a contiguous Vec
fn gather_iov(token: usize, iov_ptr: usize, iovcnt: usize) -> Result<alloc::vec::Vec<u8>, isize> {
    if iovcnt == 0 {
        return Ok(alloc::vec::Vec::new());
    }
    let iov_size = core::mem::size_of::<Iov>() * iovcnt;
    let buf = crate::mm::translated_byte_buffer(token, iov_ptr as *const u8, iov_size)?;
    let ptr = buf.as_ptr() as *const Iov;
    let iovs = unsafe { core::slice::from_raw_parts(ptr, iovcnt) };
    let total = iovs.iter().map(|iov| iov.iov_len).sum::<usize>();
    let mut v = alloc::vec![0u8; total];
    let mut written = 0usize;
    for iov in iovs {
        if iov.iov_len == 0 { continue; }
        let take = iov.iov_len;
        let src_bufs = crate::mm::translated_byte_buffer(token, iov.iov_base as *const u8, take)?;
        let ubuf = crate::mm::UserBuffer::new(src_bufs);
        ubuf.read(&mut v[written..written + take]);
        written += take;
    }
    Ok(v)
}

/// Scatter data into Iov list from a byte slice. Returns bytes written.
fn scatter_iov(token: usize, iov_ptr: usize, iovcnt: usize, data: &[u8]) -> usize {
    if iovcnt == 0 { return 0; }
    let iov_size = core::mem::size_of::<Iov>() * iovcnt;
    let buf = match crate::mm::translated_byte_buffer(token, iov_ptr as *const u8, iov_size) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let ptr = buf.as_ptr() as *const Iov;
    let iovs = unsafe { core::slice::from_raw_parts(ptr, iovcnt) };
    let mut written = 0usize;
    for iov in iovs {
        if written >= data.len() { break; }
        let take = iov.iov_len.min(data.len() - written);
        if take == 0 { continue; }
        if let Ok(dst_bufs) = crate::mm::translated_byte_buffer(token, iov.iov_base as *mut u8, take) {
            let mut ubuf = crate::mm::UserBuffer::new(dst_bufs);
            ubuf.write(&data[written..written + take]);
            written += take;
        } else {
            break;
        }
    }
    written
}

pub fn sys_sendmsg(sockfd: u32, msg_ptr: *const u8, _flags: u32) -> isize {
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let msg: MsgHdr = match crate::mm::get_from_user(token, msg_ptr as *const MsgHdr) {
        Ok(m) => m,
        Err(e) => return e,
    };

    let data = match gather_iov(token, msg.msg_iov, msg.msg_iovlen) {
        Ok(d) => d,
        Err(e) => return e,
    };
    if data.is_empty() {
        return 0;
    }

    let socket_file = match task.files.lock().get_ref(sockfd as usize) {
        Ok(f) => f.clone(),
        Err(e) => return e,
    };
    let socket = get_socket!(sockfd);

    // If msg_name is set, handle it like sendto (UDP)
    if msg.msg_name != 0 && msg.msg_namelen > 0 {
        let addr_buf = trans_ref!(msg.msg_name, msg.msg_namelen);
        match socket.socket_type() {
            SocketType::SOCK_DGRAM => {
                if socket.loacl_endpoint().port == 0 {
                    let addr = SocketAddrv4::new([0; 16].as_slice());
                    let endpoint = smoltcp::wire::IpListenEndpoint::from(addr);
                    let _ = socket.bind(endpoint);
                }
                let _ = socket.connect(addr_buf);
            }
            _ => {}
        }
    }

    let mut offset = 0usize;
    let len = socket_file.file.write(Some(&mut offset), &data);
    len as isize
}

pub fn sys_recvmsg(sockfd: u32, msg_ptr: *mut u8, _flags: u32) -> isize {
    let task = current_task().unwrap();
    let token = task.get_user_token();
    let msg: MsgHdr = match crate::mm::get_from_user(token, msg_ptr as *const MsgHdr) {
        Ok(m) => m,
        Err(e) => return e,
    };

    let socket_file = match task.files.lock().get_ref(sockfd as usize) {
        Ok(f) => f.clone(),
        Err(e) => return e,
    };
    let socket = get_socket!(sockfd);

    // Calculate total Iov capacity
    let total_cap: usize = if msg.msg_iov != 0 && msg.msg_iovlen > 0 {
        let iov_size = core::mem::size_of::<Iov>() * msg.msg_iovlen;
        match crate::mm::translated_byte_buffer(token, msg.msg_iov as *const u8, iov_size) {
            Ok(buf) => {
                let ptr = buf.as_ptr() as *const Iov;
                let iovs = unsafe { core::slice::from_raw_parts(ptr, msg.msg_iovlen) };
                iovs.iter().map(|iov| iov.iov_len).sum()
            }
            Err(_) => 0,
        }
    } else {
        0
    };
    let cap = if total_cap == 0 { 1 } else { total_cap };
    let mut tmp = alloc::vec![0u8; cap];
    let mut offset = 0usize;
    let len = socket_file.file.read(Some(&mut offset), &mut tmp[..cap]);

    // Scatter data into user iovecs
    let scattered = scatter_iov(token, msg.msg_iov, msg.msg_iovlen, &tmp[..len]);

    // Fill in msg_name from remote endpoint
    if msg.msg_name != 0 {
        let _ = socket.peer_addr(msg.msg_name, 0);
        // Update msg_namelen
        let namelen_ptr = unsafe { (msg_ptr as *mut u8).add(8) as *mut u32 };
        let endpoint = socket.remote_endpoint();
        if let Some(ref _ep) = endpoint {
            let addr_len = match _ep.addr {
                smoltcp::wire::IpAddress::Ipv4(_) => core::mem::size_of::<SocketAddrv4>() as u32 + 2,
                smoltcp::wire::IpAddress::Ipv6(_) => core::mem::size_of::<crate::net::address::SocketAddrv6>() as u32 + 2,
            };
            crate::mm::copy_to_user(token, &addr_len, namelen_ptr).unwrap();
        }
    }

    // Set msg_flags: MSG_TRUNC if truncated
    if msg.msg_name != 0 && scattered < len {
        let flags_ptr = unsafe { (msg_ptr as *mut u8).add(48) as *mut i32 };
        let new_flags = msg.msg_flags | 0x20; // MSG_TRUNC
        crate::mm::copy_to_user(token, &new_flags, flags_ptr).unwrap();
    }

    scattered as isize
}
