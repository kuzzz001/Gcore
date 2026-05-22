## Gcore (RISC-V + LoongArch)

#### 一、简介

Gcore 是一个用 Rust 编写的操作系统内核，支持 RISC-V 和 LoongArch64 两种架构。

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
```

#### 二、特性

**系统调用支持**

- 支持 100+ Linux 兼容系统调用，涵盖文件 I/O、进程控制、信号、网络、内存管理
- 实现了信号（`signal`/`sigaction`/`sigtimedwait`）、线程（`clone`/`futex`）等关键功能
- 信号基于等待队列的阻塞机制，避免轮询浪费

**内存管理**

- 支持懒分配（Lazy Allocation）、写时复制（Copy-on-Write）、页面置换
- 支持 zRAM 压缩交换、虚拟内存映射
- Buddy System + SLUB 分配器，支持 OOM 回收

**文件系统**

- 支持 ext4 和 FAT32 两种文件系统
- 双缓存加速：Buffer Cache + Page Cache
- 目录树缓存减少底层访问，提升文件操作性能
- Trait 化文件操作，借鉴 Linux VFS 的多态设计

**网络协议栈**

- 基于 smoltcp 实现的 TCP/UDP 协议栈
- 支持 IPv4/IPv6、DHCPv4
- Unix Domain Socket（SOCK_STREAM / SOCK_DGRAM）

**双架构支持**

| 架构 | 支持板卡 | QEMU 模拟 |
|------|---------|----------|
| RISC-V 64 | rvqemu, fu740 | `qemu-system-riscv64` |
| LoongArch64 | laqemu, 2k1000 | `qemu-system-loongarch64` |

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
│       ├── fs/            # 文件系统 (ext4, fat32, dev, vfs)
│       ├── hal/           # 硬件抽象层 (riscv, loongarch64)
│       ├── mm/            # 内存管理
│       ├── net/           # 网络协议栈 (tcp, udp, unix)
│       ├── syscall/       # 系统调用派发
│       ├── task/          # 任务/进程/线程管理
│       └── utils/         # 工具 (随机数、错误处理)
├── user/                  # 用户态程序与测例
├── dependency/            # 依赖库 (rustsbi, riscv 等)
└── Doc/                   # 模块文档
```

#### 五、模块文档

- [信号机制](Doc/信号.md)
- [futex 快速用户空间互斥锁](Doc/futex.md)
- [nanosleep 高精度定时](Doc/Nanosleep.md)
- [tgkill 线程信号发送](Doc/tgkill.md)

#### 六、参考资料

- [Rust 官方文档](https://doc.rust-lang.org/book/index.html)
- [Rust 语言圣经](https://course.rs/about-book.html)
- [RISC-V Linux syscall table](https://jborza.com/post/2021-05-11-riscv-linux-syscalls/)
- [Linux ext4 文件系统工作原理](https://www.cnblogs.com/liwen01/p/18237062)
- [virtio-drivers](https://github.com/rcore-os/virtio-drivers)
- [rCore-Tutorial](https://github.com/rcore-os/rCore-Tutorial-v3)
