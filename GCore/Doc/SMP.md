# SMP 多核支持

Gcore 实现了基于 RISC-V + OpenSBI 的对称多处理（SMP），支持最多 8 个 hart 同时运行。

## 架构概览

```
hart 0 (boot)              hart 1..7 (secondary)
    │                           │
rust_main()                 _secondary_start (entry.asm)
    │                           │
machine_init()              set per-hart stack (tp * 64KiB)
    │                           │
smp_start_secondary()       rust_secondary()
  ──ecall hsm_start──►       machine_init()
    │                       run_tasks()
run_tasks()               loop: fetch_task / wfi
```

## Per-Hart Processor

### 数据结构

```rust
// os/src/task/processor.rs
const MAX_HARTS: usize = 8;

static PROCESSORS: Mutex<[Processor; MAX_HARTS]> = ...;

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}
```

每个 hart 在 `PROCESSORS` 数组中拥有独立的 `Processor` 槽位，通过 `hart_id()` 索引访问。共用 `Mutex` 保护整个数组，但各 hart 访问不同槽位互不冲突。

### hart_id() 获取

```rust
// os/src/hal/arch/riscv/smp.rs
#[inline(always)]
pub fn hart_id() -> usize {
    let id: usize;
    unsafe { asm!("mv {}, tp", out(reg) id); }
    id
}
```

RISC-V 的 `tp` 寄存器由 OpenSBI 在 SBI 初始化时设置为 hart ID。

### 调度循环

```rust
pub fn run_tasks() {
    let is_primary = hart_id() == 0;
    loop {
        let mut guard = PROCESSORS.lock();
        if let Some(task) = fetch_task() {
            // 取出就绪任务、切换上下文
            let id = hart_id();
            guard[id].current = Some(task);
            __switch(idle_cx, next_cx);
        } else {
            drop(guard);
            if is_primary {
                do_wake_expired();  // Boot hart: 检查超时
            } else {
                do_wake_expired();
                asm!("wfi");         // Secondary: 等待中断
            }
        }
    }
}
```

## Secondary Hart 启动

### 入口 `_secondary_start`

```asm
# os/src/hal/arch/riscv/entry.asm
_secondary_start:
    # tp = hart_id (set by OpenSBI)
    li t0, 65536          # stack size = 64 KiB
    mv t1, tp              # hart_id
    mul t1, t1, t0         # offset = hart_id * 64KiB
    la t0, secondary_stacks
    add sp, t0, t1
    addi sp, sp, 65536     # sp = top of per-hart stack
    call rust_secondary
```

### OpenSBI HSM 启动

```rust
// os/src/main.rs
fn smp_start_secondary_harts() {
    extern "C" { fn _secondary_start(); }
    let entry = _secondary_start as usize;
    for hart in 1..8 {
        let result: usize;
        unsafe {
            asm!("ecall", in("a6") 0u32, in("a0") hart,
                 in("a1") entry, in("a2") 0usize,
                 lateout("a0") result);
        }
    }
}
```

### Per-Hart 栈

```asm
# linker.ld
.bss.secondary_stacks:
    .space 4096 * 16 * 8   # 8 harts × 64 KiB
```

## IPI（核间中断）

通过 SBI ecall 实现，无需 PLIC 参与：

```rust
// os/src/hal/arch/riscv/sbi.rs
pub fn send_ipi(hart_mask: usize) {
    sbi_call(SBI_SEND_IPI, hart_mask, 0, 0);
}

pub fn remote_sfence_vma(start: usize, size: usize) {
    sbi_call(SBI_REMOTE_SFENCE_VMA, start, size, 0);
}
```

### 使用场景

- **TLB shootdown**：页表修改后广播 `sfence.vma` 到所有 hart
- **Reschedule**：未来扩展，发送 IPI 触发目标 hart 的调度

## TLB Shootdown

```rust
// os/src/hal/arch/riscv/sv39.rs
pub fn tlb_invalidate() {
    unsafe { asm!("sfence.vma"); }
    // SMP: broadcast to all harts
    crate::hal::arch::riscv::sbi::remote_sfence_vma(0, 0);
}
```

每次页表修改后，先刷新当前 hart 的 TLB，再通过 SBI 通知所有其他 hart 作废 TLB。

## 定时器中断

每个 hart 独立控制 `sie` 寄存器中的 `STIE` 位：

```rust
// os/src/hal/arch/riscv/trap/mod.rs
pub static mut TIMER_INTERRUPTS: [usize; 8] = [0; 8];
```

Per-hart 定时器计数器，用于 `/proc/interrupts` 统计。

## 未实现/后续工作

- **Per-hart TASK_MANAGER**：当前全局共享 `TASK_MANAGER`，使用 `Mutex` 保护。未来可改为 per-hart 就绪队列 + work stealing
- **CPU Affinity**：`sched_setaffinity` 当前为空桩，需要绑核逻辑
- **IPI Reschedule**：当前仅 secondary hart idle 时 `wfi` 等待，无主动 IPI 唤醒调度机制
