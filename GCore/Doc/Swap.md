# Swap 与 Zram 交换

Gcore 实现了双层内存回收机制：**Zram 压缩**（优先）和 **Swap 块设备交换**（兜底），配合 **OOM Handler** 在内存压力下自动释放物理页。该功能通过 `oom_handler` feature 默认启用。

## 架构概览

```
内存压力
  │
  ▼
do_oom() / force_swap()
  ├── frame.zip()          → Zram (lz4_flex 压缩)
  │     ├── 成功 → 释放物理页，保持压缩副本
  │     ├── 共享页 → 跳过
  │     └── Zram 满 → 进入 swap 路径
  │
  └── frame.swap_out()     → Swap 块设备
        ├── 成功 → 释放物理页，写回块设备
        ├── 共享页 → 跳过
        └── Swap 满 → 跳过（内存压力持续）

页面恢复（Page Fault）
  │
  ▼
do_page_fault()
  ├── Frame::Compressed → unzip() (lz4 解压)
  ├── Frame::SwappedOut → swap_in() (块设备读回)
  └── Frame::Unallocated → lazy alloc (零页填充)
```

## Frame 状态机

```rust
// os/src/mm/map_area.rs
pub enum Frame {
    InMemory(Arc<FrameTracker>),   // 物理页在内存中
    Compressed(Arc<ZramTracker>),  // lz4 压缩副本在 Zram 中
    SwappedOut(Arc<SwapTracker>),  // 页面已写入 Swap 设备
    Unallocated,                   // 尚未分配（匿名页首次访问）
}
```

## Zram 压缩

### 数据结构

```rust
// os/src/mm/zram.rs
pub struct Zram {
    compressed: Vec<Option<Vec<u8>>>,  // 2048 槽位
    recycled: Vec<u16>,                // 回收的空闲槽位
    tail: u16,                         // 当前分配位置
}
```

### 核心操作

| 方法 | 功能 |
|------|------|
| `write(buf)` | `lz4_flex::compress_prepend_size()` → 分配槽位 → 返回 `Arc<ZramTracker>` |
| `read(id, buf)` | `lz4_flex::decompress_size_prepended()` → 解压到缓冲区 |
| `discard(id)` | 释放槽位，加入 `recycled` 或回退 `tail` |

### 引用计数保护

```rust
pub struct ZramTracker(pub usize);

impl Drop for ZramTracker {
    fn drop(&mut self) {
        ZRAM_DEVICE.lock().discard(self.0).unwrap();
    }
}
```

ZramTracker 被 `Arc` 包裹，仍在使用的压缩页不会被回收（类似于 Swap Cache）。

## Swap 块设备交换

### 数据结构

```rust
// os/src/fs/swap.rs
pub struct Swap {
    bitmap: Vec<u64>,         // 位图管理 swap slot 分配
    block_ids: Vec<usize>,    // 每个 slot 对应的块设备扇区
    usable: bool,             // 是否成功分配块设备空间
}
```

### 核心操作

| 方法 | 功能 |
|------|------|
| `new(size)` | 从文件系统 `alloc_blocks()` 分配连续扇区 |
| `alloc_page()` | bitmap 扫描空闲位 → 返回 swap_id |
| `write(buf)` | `alloc_page()` → 写入块设备 → 置位 |
| `read(id, buf)` | 从块设备读回对应扇区 |
| `discard(id)` | 清零 bitmap 位 |

### 引用计数保护

```rust
pub struct SwapTracker(pub usize);

impl Drop for SwapTracker {
    fn drop(&mut self) {
        SWAP_DEVICE.lock().discard(self.0);
    }
}
```

与 ZramTracker 相同机制，保护 pending writeback 的 swap slot。

## OOM Handler

```rust
// os/src/mm/memory_set.rs
pub fn do_shallow_clean(&mut self) -> usize {
    self.areas.iter_mut()
        .map(|area| area.do_oom(page_table))
        .sum()
}
```

### do_oom() 流程

```
遍历 active 队列（最近访问的页面，从队首开始）
  1. 尝试 frame.zip()     → Zram 压缩（优先）
  2. 失败则 frame.swap_out()  → 写 Swap 设备
  3. 成功后 unmap 页表项
```

`force_swap()` 跳过 zram 压缩步骤，直接向 swap 设备写入。

## Page Fault 恢复路径

```rust
// os/src/mm/memory_set.rs
pub fn do_page_fault(&mut self, addr: VirtAddr) -> Result<PhysAddr, MemoryError> {
    match frame {
        Frame::Compressed(_) => {
            let ppn = frame.unzip().unwrap();            // Zram 解压
            self.page_table.map(vpn, ppn, area.map_perm); // 重映射页表
            ppn
        }
        Frame::SwappedOut(_) => {
            let ppn = frame.swap_in().unwrap();           // 块设备读回
            self.page_table.map(vpn, ppn, area.map_perm);
            ppn
        }
        // ...
    }
}
```

恢复后的页面被加入 `active` 队列，作为 OOM 回收的候选。

## Build Features

```toml
# Cargo.toml
[features]
default = ["board_rvqemu", "block_virt", "oom_handler"]
oom_handler = ["swap", "zram"]
swap = []
zram = []
```

## 依赖

- `lz4_flex = "0.9.0"`：LZ4 压缩/解压算法
- `os/src/fs/swap.rs`：Swap 块设备后端
- `os/src/mm/zram.rs`：Zram 压缩内存后端
