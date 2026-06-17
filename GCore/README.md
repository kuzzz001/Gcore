## Gcore (RISC-V + LoongArch)

#### 一、简介

Gcore 是一个用 Rust 编写的操作系统内核，支持 RISC-V 64 和 LoongArch64 两种架构。项目定位于 Linux 兼容的操作系统内核，已实现 120+ 系统调用，涵盖进程管理、内存管理、文件系统、网络协议栈、信号处理等子系统。

- 仓库分支介绍

`main` —— 最新代码及文档

- 运行截图

```
$ make qemu          # 启动 Gcore
[kernel] Hello, world!
[fs] found ext4 filesystem
[kernel] /dev init Successfully!
[kernel] block in virt mode!
[kernel] oom_handler is enabled!
[kernel] hart 1 started (entry=0x802000d0)
[kernel] hart 2 started (entry=0x802000d0)
...
[kernel] hart 7 online
```

#### 二、特性

**系统调用支持（120+）**

- 文件 I/O：`open`/`read`/`write`/`close`/`lseek`/`stat`/`fstat`/`mmap`/`munmap`
- 进程控制：`fork`/`vfork`/`clone`/`execve`/`exit`/`wait4`/`waitid`/`kill`/`tkill`/`tgkill`
- 线程同步：`futex`（含 WakeOp/WaitBitset 等完整命令）、`set_robust_list`
- 信号：`sigaction`/`sigprocmask`/`sigreturn`/`sigtimedwait`/`rt_sigqueueinfo`
- 网络：`socket`/`bind`/`listen`/`accept`/`connect`/`sendmsg`/`recvmsg`/`sendto`/`recvfrom`
- 网络选项：`setsockopt`/`getsockopt`（SO_REUSEADDR/KeepAlive/Nagle/SNDBUF/RCVBUF/TCP_MSS）
- 定时器：`clock_gettime`/`clock_nanosleep`（含 TIMER_ABSTIME）/`nanosleep`/`gettimeofday`
- 多核：`sched_setaffinity`/`sched_getaffinity`/`sched_get_priority_max`/`sched_get_priority_min`
- 其他：`prctl`/`getrandom`/`membarrier`/`syslog`/`socketpair`/`poll`/`ppoll`/`pselect`

**内存管理**

- Buddy System 帧分配器 + SLUB 堆分配器
- 懒分配（Lazy Allocation）、写时复制（Copy-on-Write）
- zRAM 压缩内存交换 + Swap 块设备交换
- OOM handler 自动 Evict（zip → swap out）
- Page Fault 处理：匿名页 swap-in、文件映射懒加载、CoW 分裂
- 支持 `mmap`/`mprotect`/`msync`/`mlock`/`munlock`/`madvise`

**文件系统**

- ext4 和 FAT32 两种文件系统
- VFS Trait 化文件操作（多态 dispatch）
- 双缓存加速：Buffer Cache + Page Cache
- 目录树缓存减少底层访问
- `/dev` 设备文件：`null`/`zero`/`urandom`/`tty`/`hwclock`/`pipe`
- `/proc`：`/proc/meminfo`、`/proc/interrupts`
- 支持 `renameat2`、`faccessat2`、`statx`、`utimensat`

**网络协议栈**

- 基于 smoltcp 实现的完整 TCP/UDP/ICMP 协议栈
- TCP：三次握手、Nagle 算法、Keep-Alive、MSS 协商
- UDP：无连接数据报，自动端口分配
- ICMP Raw Socket：支持 `socket(AF_INET, SOCK_RAW, IPPROTO_ICMP)`
- Unix Domain Socket（SOCK_STREAM / SOCK_DGRAM）
- IPv4/IPv6 双栈 + DHCPv4
- Scatter-Gather I/O：`sendmsg`/`recvmsg` 的 iovec 支持
- `poll`/`ppoll`/`pselect` 支持 socket fd 轮询

**多核 SMP 支持**（新增）

- Per-hart Processor 数组，`hart_id()` 从 `tp` 寄存器获取
- OpenSBI HSM 启动 8 个 hart，per-hart 独立调度循环
- SBI IPI 支持（reschedule + TLB shootdown）
- Per-hart 定时器中断（STIE 位独立控制）

**双架构支持**

| 架构 | 支持板卡 | QEMU 模拟 |
|------|---------|----------|
| RISC-V 64 | rvqemu, fu740, k210, cv1811h | `qemu-system-riscv64` |
| LoongArch64 | laqemu, 2k1000 | `qemu-system-loongarch64` |

**Swap 与 OOM 处理**（已启用）

- Zram：lz4_flex 压缩，2048 槽位，支持回收复用
- Swap：bitmap 管理 swap slots，块设备后端读写
- OOM Handler：Zram 优先压缩冷页 → Swap out 兜底
- Swap Cache：`Arc<SwapTracker>` 引用计数保护 pending writeback

**进程间通信**

- 信号：基于等待队列的阻塞机制，避免轮询
- Pipe：阻塞读写，支持 poll
- Futex：完整的 PI 无关命令集 + 超时等待
- Unix Domain Socket：基于 Pipe 的配对实现

**QoS 与可靠性**

- 0 个 `todo!()` / `unreachable!()` 遗留（全部替换为合理默认值）
- 系统调用出错时返回 errno 而非内核 panic
- 编译 0 error 0 warning

#### 三、快速开始

##### 1. 安装依赖

```bash
# Rust 工具链
curl https://sh.rustup.rs -sSf | sh
source $HOME/.cargo/env
rustup install nightly
rustup target add riscv64gc-unknown-none-elf --toolchain nightly

# 工具组件
cargo install cargo-binutils
rustup component add llvm-tools-preview rust-src
```

##### 2. 下载测例

```bash
make testsuits-download
xz -dkc fs-img-dir/sdcard-la.img.xz > sdcard-la.img
xz -dkc fs-img-dir/sdcard-rv.img.xz > sdcard-rv.img
```

##### 3. 进入 Docker 环境编译

```bash
make docker        # 启动 Docker 容器
make env           # 设置 Rust 工具链
make all           # 编译内核
```

编译完成后根目录出现 `kernel-rv` 和 `kernel-la` 两个内核镜像。

##### 4. 运行

```bash
# 运行 RISC-V 测例
cd os && make rv64-run

# 运行 LoongArch 测例
cd os && make la64-run

# 批量测试（12 个分组自动循环）
TEST_ARCH=both bash run_test.sh
```

#### 四、项目结构

```
GCore/
├── os/                    # 内核主代码
│   └── src/
│       ├── drivers/       # 块设备、串口驱动
│       ├── fs/            # 文件系统 (ext4, fat32, dev, swap, vfs, cache)
│       ├── hal/           # 硬件抽象层
│       │   └── arch/
│       │       ├── riscv/ # RISC-V: trap, sbi, sv39, smp, entry
│       │       └── loongarch64/ # LoongArch: trap, tlb, boot, laflex
│       ├── mm/            # 内存管理 (frame_alloc, heap_alloc, page_table, zram)
│       ├── net/           # 网络协议栈 (tcp, udp, icmp, unix, config)
│       ├── syscall/       # 系统调用派发 (fs, net, process, syscall_id)
│       ├── task/          # 任务/进程/线程管理 (processor, signal, futex, pid)
│       └── utils/         # 工具 (random, error)
├── user/                  # 用户态程序与测例
├── dependency/            # 依赖库 (rustsbi, riscv, virtio-drivers, dep_iso, dep_pci)
└── Doc/                   # 模块文档
```

#### 五、模块文档

- [信号机制](Doc/信号.md)
- [futex 快速用户空间互斥锁](Doc/futex.md)
- [nanosleep 高精度定时](Doc/Nanosleep.md)
- [tgkill 线程信号发送](Doc/tgkill.md)
- [SMP 多核支持](Doc/SMP.md)
- [Swap 与 Zram 交换](Doc/Swap.md)
- [网络协议栈](Doc/Network.md)

#### 六、Build Features

| Feature | 说明 | 默认 |
|---------|------|------|
| `riscv` | RISC-V 64 架构 | on |
| `loongarch64` | LoongArch64 架构 | off |
| `board_rvqemu` | RISC-V QEMU 板卡 | on |
| `block_virt` | virtio 块设备 | on |
| `oom_handler` | OOM 内存回收 | on |
| `swap` | 块设备 Swap | on (via oom_handler) |
| `zram` | zRAM 压缩交换 | on (via oom_handler) |
| `log_off` / `log_info` / `log_warn` / `log_error` | 日志级别控制 | info |

#### 七、参考资料

- [Rust 官方文档](https://doc.rust-lang.org/book/index.html)
- [Rust 语言圣经](https://course.rs/about-book.html)
- [RISC-V Linux syscall table](https://jborza.com/post/2021-05-11-riscv-linux-syscalls/)
- [Linux ext4 文件系统工作原理](https://www.cnblogs.com/liwen01/p/18237062)
- [virtio-drivers](https://github.com/rcore-os/virtio-drivers)
- [rCore-Tutorial](https://github.com/rcore-os/rCore-Tutorial-v3)
- [smoltcp 协议栈](https://github.com/smoltcp-rs/smoltcp)
