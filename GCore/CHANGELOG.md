# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
