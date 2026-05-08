## Gcore (RISC-V + LoongArch)

#### 一、简介

Gcore 是一个用 Rust 编写的操作系统内核，支持 RISC-V 和 LoongArch64 两种架构。

- 仓库分支介绍

`main` —— 最新代码及文档

- 运行demo

待补充

#### 二、特性

- 系统调用支持完善：Gcore 支持 100+ 系统调用，并实现了信号、线程等关键功能。

- 内存管理高效：支持懒分配、写时复制、zRAM 和虚拟内存等机制，提升内存利用率。

- 基于等待队列的阻塞机制：使用等待队列实现如 futex 的阻塞系统调用，避免轮询浪费，提高效率。

- 双缓存加速 I/O：实现 Buffer Cache 和 Page Cache，有效提升文件系统读写性能。

- 缓存一致性与策略优化：通过共享物理页与区分策略减少冗余，提高 Fat 区和数据区访问效率。

- 激进缓存策略：Page Cache 不设上限，结合 LRU 回收机制，读写性能显著提升。

- 虚拟文件系统重构：引入目录树缓存，减少底层访问，加速文件操作。

- Trait 化文件操作：借鉴 Linux VFS 的多态设计，使用 Rust Trait 实现模块化和可扩展性。

#### 三、基础环境配置（ bash ）

1. 安装 Rust 版本管理器 rustup 和 Rust 包管理器 cargo，并设置环境变量

```bash
curl https://sh.rustup.rs -sSf | sh
source $HOME/.cargo/env
```

2. 安装 rustc 的 nightly 版本

```bash
rustup install nightly
rustup default nightly
```

3. 切换 rust 版本

```bash
rustup toolchain install nightly-2025-01-18-x86_64-unknown-linux-gnu
rustup override set nightly-2025-01-18-x86_64-unknown-linux-gnu
```

4. 安装相关软件包

```bash
cargo install cargo-binutils
rustup component add llvm-tools-preview
rustup component add rust-src
```

5. 安装 qemu-9.2.1

前往[系统能力培养赛操作系统大赛评测镜像](https://gitlab.educg.net/wangmingjian/os-contest-2024-image)下载 qemu-9.2.1.tar.xz

解压后按如下步骤进行，默认安装至 `/usr/local/bin`

```bash
# 安装
cd qemu-9.2.1
./configure \
	--target-list=loongarch64-softmmu,riscv64-softmmu \
	--enable-gcov \
	--enable-debug \
	--enable-slirp
make -j$(nproc)
sudo make install
```

6. 安装 rv64 工具链

```bash
# 添加 rv64 交叉编译支持
rustup target add riscv64gc-unknown-none-elf

# 添加 debugger for rv64
git clone https://github.com/riscv-collab/riscv-gnu-toolchain.git
cd riscv-gnu-toolchain
./configure --prefix=$HOME/OS/rv64tool --target=riscv64-unknown-elf --enable-gdb
make
make install
```

7. 安装 la64 工具链

前往[oscomp-toolchains-for-oskernel](https://gitee.com/link?target=https%3A%2F%2Fgithub.com%2FLoongsonLab%2Foscomp-toolchains-for-oskernel%2Ftree%2Fmain)下载 loongarch64-linux-gnu-gdb.tgz 并解压

前往[系统能力培养赛操作系统大赛评测镜像](https://gitee.com/link?target=https%3A%2F%2Fgitlab.educg.net%2Fwangmingjian%2Fos-contest-2024-image)下载 gcc-13.2.0-loongarch64-linux-gnu-nw.tgz 并解压

#### 四、文档

- **模块与特性相关文档**

[信号](Doc/信号.md)

[futex](Doc/futex.md)

[nanosleep](Doc/Nanosleep.md)

[tgkill](Doc/tgkill.md)

#### 五、参考资料

- Rust 教程

[1]  [官方文档](https://doc.rust-lang.org/book/index.html)

[2]  [Rust 语言圣经](https://course.rs/about-book.html)

- RISC-V

[3]  [RISC-V Linux syscall table](https://jborza.com/post/2021-05-11-riscv-linux-syscalls/)

- ext4 文件系统

[4]  [一文看懂linux ext4文件系统工作原理](https://www.cnblogs.com/liwen01/p/18237062)

- virtio库

[5]  [virtio-drivers](https://github.com/rcore-os/virtio-drivers)
