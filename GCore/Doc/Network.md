# 网络协议栈

Gcore 基于 smoltcp 实现了完整的 TCP/UDP/ICMP 协议栈，同时支持 Unix Domain Socket。提供 120+ 系统调用中的网络子系统。

## 架构

```
用户空间
  │ socket/bind/listen/accept/connect/sendmsg/recvmsg/sendto/recvfrom
  │ setsockopt/getsockopt/shutdown/poll/ppoll/pselect
  ▼
syscall 层 (os/src/syscall/net.rs)
  │ 参数验证、内存翻译、错误传播
  ▼
Socket trait (os/src/net/mod.rs)
  ├── TcpSocket  (os/src/net/tcp.rs)
  ├── UdpSocket  (os/src/net/udp.rs)
  ├── IcmpSocket (os/src/net/icmp.rs)
  └── UnixSocket (os/src/net/unix.rs)
  ▼
smoltcp 协议栈 (vendor/smoltcp)
  │ tcp::Socket / udp::Socket / icmp::Socket
  │ 状态机、重传、流控
  ▼
NET_INTERFACE (os/src/net/config.rs)
  │ Interface + DeviceCapabilities
  │ SocketSet 管理所有 socket handler
  │ poll() 驱动协议栈定时器和事件
  ▼
virtio-net / 物理网卡驱动
```

## Socket trait

```rust
// os/src/net/mod.rs
pub trait Socket: File {
    fn bind(&self, addr: IpListenEndpoint) -> SyscallRet;
    fn listen(&self) -> SyscallRet;
    fn connect(&self, addr_buf: &[u8]) -> SyscallRet;
    fn accept(&self, sockfd: u32, addr: usize, addrlen: usize) -> SyscallRet;
    fn socket_type(&self) -> SocketType;
    fn recv_buf_size(&self) -> usize;
    fn send_buf_size(&self) -> usize;
    fn set_recv_buf_size(&self, size: usize);
    fn set_send_buf_size(&self, size: usize);
    fn loacl_endpoint(&self) -> IpListenEndpoint;
    fn remote_endpoint(&self) -> Option<IpEndpoint>;
    fn shutdown(&self, how: u32) -> GeneralRet<()>;
    fn set_nagle_enabled(&self, enabled: bool) -> SyscallRet;
    fn set_keep_alive(&self, enabled: bool) -> SyscallRet;
}
```

`Socket` 继承自 `File` trait，所有 socket 类型同时是文件，可通过 fd 进行 poll/select 操作。

## TCP Socket

### 特性

- 完整的三次握手（smoltcp 状态机）
- Nagle 算法（可通过 `setsockopt(TCP_NODELAY)` 关闭）
- Keep-Alive（可通过 `setsockopt(SO_KEEPALIVE)` 开启）
- MSS 协商（`getsockopt(TCP_MAXSEG)`）
- TCP_CONGESTION 支持（返回 "reno"）

### 连接流程

```
客户机                      服务器
socket()                    socket()
connect() ──SYN──►          bind() + listen()
                           accept() 阻塞等待
        ◄──SYN+ACK──
        ──ACK──►           accept() 返回新 fd
send()/write()              recv()/read()
```

### accept 实现

```rust
// os/src/net/tcp.rs
fn accept(&self, sockfd: u32, addr: usize, addrlen: usize) -> SyscallRet {
    let peer_addr = self._accept(old_nonblock)?;
    // 创建新 socket 绑定同一本地端口
    let new_socket = TcpSocket::new();
    new_socket.bind(local)?;
    new_socket.listen()?;
    // fd 替换：旧的 socket 移到新 fd，新的替换原 fd
    fd_table.insert_at(FileDescriptor::new(cloexec, nonblock, new_socket), sockfd);
    fd_table.insert(old_file) // 旧 socket 获得新 fd 号
}
```

## UDP Socket

### 特性

- 无连接数据报
- 自动端口分配（bind 到端口 0 时随机分配）
- 端口冲突检测（`SocketTable::can_bind`）

### sendto 流程

```
sys_sendto(sockfd, buf, len, flags, dest_addr, addrlen)
  1. 检查本地端口，若为 0 则随机分配
  2. connect(dest_addr) 设置目标
  3. write() 发送数据报
```

## ICMP Raw Socket

### 创建

```c
int fd = socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);
```

### 实现

```rust
// os/src/net/icmp.rs
pub struct IcmpSocket {
    socket_handler: SocketHandle,  // smoltcp ICMP socket
}

impl Socket for IcmpSocket {
    fn socket_type(&self) -> SocketType { SocketType::SOCK_RAW }
    // ...
}
```

ICMP socket 通过 smoltcp 的 `icmp::Socket` 发送/接收 ICMP 报文，支持 `can_send()`/`can_recv()` 就绪检查。

## Unix Domain Socket

### 两种类型

| 类型 | 底层传输 | 用途 |
|------|---------|------|
| `SOCK_STREAM` | Pipe（字节流） | 可靠有序字节流 |
| `SOCK_DGRAM` | CircularQueue（消息） | 数据报 |

### 实现

```rust
// os/src/net/unix.rs
pub struct UnixSocket {
    read_end: Arc<Pipe>,
    write_end: Arc<Pipe>,
    socket_type: SocketType,
    shut_rd: Mutex<bool>,
    shut_wr: Mutex<bool>,
}
```

Unix socket 通过 `socketpair()` 预配对，`read()`/`write()` 委托给底层 Pipe。

## sendmsg / recvmsg

### MsgHdr 结构（riscv64 ABI）

```rust
#[repr(C)]
struct MsgHdr {
    msg_name: usize,          // +0  可选目标地址
    msg_namelen: u32,         // +8  地址长度
    _pad: u32,                // +12 对齐
    msg_iov: usize,           // +16 iovec 数组指针
    msg_iovlen: usize,        // +24 数组长度
    msg_control: usize,       // +32 控制信息
    msg_controllen: usize,    // +40 控制信息长度
    msg_flags: i32,           // +48 标志位
    _pad2: u32,               // +52 对齐
}
```

### Scatter-Gather I/O

```rust
// gather_iov: iovec → 内核连续缓冲区
fn gather_iov(token, iov_ptr, iovcnt) -> Result<Vec<u8>, isize> {
    for each iov {
        translated_byte_buffer(iov.iov_base, iov.iov_len)
        → UserBuffer::read()  // 从用户空间读
    }
}

// scatter_iov: 内核缓冲区 → 用户 iovec
fn scatter_iov(token, iov_ptr, iovcnt, data) -> usize {
    for each iov {
        translated_byte_buffer(iov.iov_base, iov.iov_len)
        → UserBuffer::write()  // 写到用户空间
    }
}
```

## Socket 选项

| 选项 | Level | 支持 |
|------|-------|------|
| TCP_NODELAY | SOL_TCP | setsockopt / getsockopt |
| TCP_MAXSEG | SOL_TCP | getsockopt（返回 TCP_MSS） |
| TCP_CONGESTION | SOL_TCP | getsockopt（返回 "reno"） |
| SO_SNDBUF | SOL_SOCKET | setsockopt / getsockopt |
| SO_RCVBUF | SOL_SOCKET | setsockopt / getsockopt |
| SO_KEEPALIVE | SOL_SOCKET | setsockopt / getsockopt（返回 1） |
| SO_REUSEADDR | SOL_SOCKET | setsockopt（noop）/ getsockopt（返回 1） |

## Poll / Select 支持

所有 socket 类型实现 `r_ready()` 和 `w_ready()`：

```rust
// TcpSocket
fn r_ready(&self) -> bool {
    // SynReceived/Established 或 can_recv()
}
fn w_ready(&self) -> bool {
    can_send()
}

// IcmpSocket
fn r_ready(&self) -> bool { can_recv() }
fn w_ready(&self) -> bool { can_send() }
```

`ppoll` / `pselect` 通过 `FileDescriptor::r_ready()`/`w_ready()` 遍历所有 fd，无需 socket 特殊处理。

## NET_INTERFACE

```rust
// os/src/net/config.rs
pub struct NetInterface {
    inner: spin::Mutex<InterfaceInner>,
}

impl NetInterface {
    pub fn poll(&self)          // 驱动 smoltcp 协议栈
    pub fn add_socket(&self, s) // 注册 socket 到 SocketSet
    pub fn remove(&self, h)     // 从 SocketSet 移除
    pub fn tcp_socket(&self, ...)  // 闭包访问 TCP socket
    pub fn udp_socket(&self, ...)  // 闭包访问 UDP socket
    pub fn inner_handler(&self, ...) // 通用闭包访问
}
```

`poll()` 需要在每个事件循环中定期调用，驱动协议栈的定时器、重传、状态机等。
