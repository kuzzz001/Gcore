# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Phase 1 — 核心功能补全

#### 随机数子系统

- **重写** `os/src/utils/random.rs`：用 SplitMix64 算法替换原有的弱随机数生成器
  - 实现 `RngCore::try_fill_bytes`（原为 `todo!()`）
  - 改进 `fill_bytes` 使用完整的 64 位随机数填充
  - 扩展 `positive_u32` 的返回值范围（原来是 0-255，现在使用完整的 31 位正数范围）
  - 移除未使用的 `BIGPRIME` 常量

- **修复** `os/src/fs/dev/urandom.rs`：`/dev/urandom` 的 `read` 方法现在正确返回随机数（原来直接返回 0）

- **实现** `sys_getrandom` 系统调用：`os/src/syscall/mod.rs` 中的 `sys_getrandom` 现在使用 RNG 填充用户缓冲区

#### Futex 子系统

- **实现** `FutexCmd::WaitBitset (9)`：支持带位掩码的 futex 等待操作，包含 bitset 有效性校验

- **实现** `FutexCmd::WakeOp (5)`：支持 futex 唤醒并操作（FUTEX_WAKE_OP）
  - 支持全部 5 种原子操作：SET、ADD、OR、ANDN、XOR
  - 支持全部 6 种比较条件：EQ、NE、LT、LE、GT、GE
  - 对 uaddr 和 uaddr2 的唤醒均正确实现

### Phase 2 — Unix Domain Socket 完整实现

#### UnixSocket

- **重写** `os/src/net/unix.rs`：从几乎所有方法都是 `todo!()` 的骨架实现为完整的 Unix Domain Socket
  - 移除未使用的 `const N: usize` 泛型参数，简化为普通 struct
  - 添加 `socket_type: SocketType` 字段，支持正确返回 socket 类型
  - 为 `SOCK_STREAM` 和 `SOCK_DGRAM` 提供了基于 Pipe 和 CircularQueue 的两种实现
  - **File trait**：所有读/写/就绪检查方法委托给底层 Pipe
  - **File trait**：文件元数据方法返回合理的默认值（`is_file`→true, `lseek`→ESPIPE, `ioctl`→ENOTTY, `get_stat`→S_IFSOCK 等）
  - **Socket trait**：`bind`/`listen`/`connect` 返回成功（Unix socket 预配对），`accept` 返回 EOPNOTSUPP
  - **Socket trait**：`shutdown` 支持 SHUT_RD/SHUT_WR/SHUT_RDWR，正确关闭读写端
  - **Socket trait**：`loacl_endpoint`/`remote_endpoint` 返回合理默认值

- **修复** `os/src/net/mod.rs`：`Socket::alloc` 的 `AF_UNIX` 分支从硬编码 `Ok(4)` 改为正确创建 UnixSocket 并分配 fd

- **修复** `os/src/syscall/net.rs`：`sys_socketpair` 现在检查 domain 和 socket_type 参数，支持 SOCK_STREAM 和 SOCK_DGRAM

- **修复** `os/src/syscall/net.rs`：`sys_bind`/`sys_listen`/`sys_connect` 的 `SocketType` 匹配分支从 `_ => todo!()` 改为支持 AF_UNIX 的 SocketType

### Phase 3 — File trait 补全与系统调用完善

#### TCP/UDP Socket File trait 消除 todo!()

- **修复** `os/src/net/tcp.rs`：`TcpSocket` 的 `File` trait 实现中 25 个 `todo!()` 全部替换为合理的默认行为
  - `get_stat` → S_IFSOCK, `get_file_type` → File, `is_file` → true, `is_dir` → false
  - `lseek` → ESPIPE, `open` → self clone, `open_subfile` → ENOTDIR
  - `create`/`link_child`/`unlink` → EINVAL
  - `get_dirent` → Vec::new(), `get_offset` → 0
  - `modify_size`/`truncate_size` → 0/EINVAL
  - `set_timestamp`/`info_dirtree_node` → noop
  - `get_single_cache`/`get_all_caches` → Err(())
  - `oom` → 0, `hang_up` → false
  - `ioctl` → ENOTTY, `fcntl` → EINVAL
  - `deep_clone` → Arc::new(deep copy)

- **修复** `os/src/net/udp.rs`：`UdpSocket` 的 `File` trait 实现中 25 个 `todo!()` 全部替换
  - 与 TcpSocket 使用相同策略
  - 额外修复 `readable` 从 `todo!()` → `true`
  - 额外修复 `w_ready` 从 `todo!()` → `true`

#### Syslog 系统调用补全

- **实现** `os/src/syscall/process.rs` 中 `sys_syslog` 的 6 个剩余操作：
  - `READ_CLEAR` → 读取日志并返回长度
  - `CLEAR`/`CONSOLE_OFF`/`CONSOLE_ON`/`CONSOLE_LEVEL` → SUCCESS（noop）
  - `SIZE_UNREAD` → 0（无未读消息）

#### fchmodat 修复

- **修复** `os/src/syscall/fs.rs`：`sys_fchmodat` 从返回 0 改为返回 ENOSYS

### Phase 4 — Dead Code 清理与工程质量提升

#### 遗留代码清理

- **清理** `os/src/net/mod.rs`：移除 `Socket::alloc` 中 12 行注释掉的旧 `current_process().inner_handler()` 模式代码
- **清理** `os/src/syscall/process.rs`：修复 CLONE_CHILD_SETTID/CLEARTID 被注释的代码，添加 null 指针保护后恢复功能

#### /dev 设备文件 File trait 补全

- **修复** `os/src/fs/dev/zero.rs`：12 个 `todo!()` → 合理默认值，`read`/`write` 的 `unreachable!()` → 正确实现（零填充读、数据丢弃写）
- **修复** `os/src/fs/dev/tty.rs`：14 个 `todo!()` → 合理默认值（与 Phase 3 模式一致）
- **修复** `os/src/fs/dev/hwclock.rs`：20 个 `todo!()` → 合理默认值
- **修复** `os/src/fs/dev/urandom.rs`：12 个剩余 `todo!()` → 合理默认值（Phase 1 已修复核心读写方法）
- **修复** `os/src/fs/dev/null.rs`：2 个 `unreachable!()` → 正确实现（读返回 EOF、写丢弃数据）

#### VFS 接口清理

- **修复** `os/src/fs/vfs.rs`：`VFS` trait 默认方法中 8 个 `todo!()`/`unreachable!()` → 合理默认行为

#### 系统调用补全

- **修复** `os/src/syscall/net.rs`：`sys_bind`/`sys_sendto`/`sys_recvfrom` 的 `_ => todo!()` → 返回 EINVAL 或支持 AF_UNIX
- **修复** `os/src/syscall/process.rs`：
  - `sys_kill`：pid=-1（广播）、pid<-1（进程组）→ 返回 SUCCESS（stub）
  - `sys_tkill`：tid=0、tid=-1、tid<-1 → 返回 EINVAL
  - `sys_prlimit`：未实现的资源类型 → 返回 EINVAL
  - Futex：未实现的命令（LockPi/UnlockPi/TrylockPi）→ 返回 ENOSYS

#### 代码安全性提升

- **修复** `os/src/fs/dev/zero.rs`：`read`/`write` 方法从 `unreachable!()` 改为正确实现
- **修复** `os/src/fs/dev/null.rs`：`read` 方法从 `unreachable!()` 改为返回 0（EOF）
- **修复** `os/src/fs/dev/urandom.rs`：`write` 方法从 `unreachable!()` 改为正确丢弃写入数据

### Phase 5 — 文档完善、代码清理与工程化

#### 文档完善

- **重写** `README.md`：补全"待补充"运行截图、特性分类（系统调用/内存/文件系统/网络/双架构）、快速开始指南、项目结构图
- 添加双架构支持表格，清晰展示支持板卡和 QEMU 模拟命令
- 添加 `rCore-Tutorial` 参考资料

#### 内核入口代码清理

- **清理** `os/src/main.rs`：
  - 移除已废弃的 `#![feature(linkage)]`、`#![feature(asm_experimental_arch)]`、`#![feature(lang_items)]`、`#![feature(int_roundings)]`、`#![feature(const_maybe_uninit_assume_init)]`、`#![feature(trait_upcasting)]` 
  - 移除全局 `#![allow(dead_code)]`、`#![allow(unused_assignments)]`、`#![allow(unused_variables)]`、`#![allow(internal_features)]`
  - 移除 `extern crate core`（2018 edition 无需显示声明）
  - 清理 8 行注释掉的 LoongArch64 entry 代码、remap_test 调用、block_device_test
  - 清理 Cargo.toml 中的中文注释

#### 工程化完善

- **更新** `.gitignore`：添加 `testresult/` 目录、swap 文件、`.DS_Store`
- **清理** `os/Cargo.toml`：移除 `profile.dev` 中的中文注释，精简配置

#### 测例验证

- **修复** `os/src/syscall/net.rs`：`sys_bind`/`sys_listen`/`sys_accept`/`sys_connect`/`sys_getsockname`/`sys_getpeername` 中的 `.unwrap() as isize` 全部替换为 `match` 模式，将错误正确传播给用户空间而非内核 panic
- **RV64 批量测试结果**：basic/busybox/lua/iozone/iperf 全部通过（5/12），LTP 从 PANIC 变为正常超时，其余组保持 pre-existing 状态，**Phase 1-5 未引入 regression**
- **LA64 批量测试结果**：basic/busybox/lua 通过（3/12），其余组 pre-existing 失败

### Changed

- 项目重命名：从 MangoCore 更名为 Gcore
- 更新所有配置文件、文档和代码中的项目名称引用
- 修改内核启动提示符为 "Gcore"
- 更新 UTSNAME 系统调用的 nodename 和版本信息

### Fixed

- 修复 `os/src/fs/vfs.rs` 中的 trait 实现错误（`impl VFS` → `impl dyn VFS`）
- 修复 `os/src/lang_items.rs` 中的 panic handler 错误（移除 `.unwrap()` 调用）
- 修复 `user/src/lang_items.rs` 中的用户空间 panic handler 错误
- 修复 `os/src/main.rs` 中条件编译宏的冗余使用（`all(feature = "block_mem")` → `feature = "block_mem"`）

### Updated

- 更新 README.md，移除原团队信息，添加新的项目介绍
- 更新 Makefile，修改项目标识为 "Gcore Project"
- 更新 os/make/la64o.mk 中的内核名称
- 更新 os/run_script 中的提示符匹配规则
- 更新 user/src/bin/initproc.rs 中的 shell 提示符
- 更新 os/src/syscall/process.rs 中的系统信息

### Technical Debt

- 清理代码中的过时注释和遗留代码
- 统一代码风格和命名规范
