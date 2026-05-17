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
