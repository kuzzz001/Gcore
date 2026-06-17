# Nanosleep / Clock Nanosleep

`clock_nanosleep` 是 Linux 中精确到纳秒级别的睡眠系统调用，允许用户进程挂起自身一段指定时间，用于延时、轮询节流、定时调度等场景。支持**相对时间**和**绝对时间（TIMER_ABSTIME）**两种模式。

## Gcore 实现

Gcore 完整实现了 `sys_clock_nanosleep`，包含以下全部功能：

- CLOCK_REALTIME / CLOCK_MONOTONIC 等多时钟源
- `TIMER_ABSTIME` 绝对时间唤醒
- 阻塞等待 + 超时自动唤醒
- 被信号中断时返回剩余时间（`rmtp`）

## 接口定义

```rust
pub fn sys_clock_nanosleep(
    clk_id: usize,
    flags: u32,
    rqtp: *const TimeSpec,
    rmtp: *mut TimeSpec,
) -> isize
```

参数：

- `clk_id`：时钟源，0=CLOCK_REALTIME，1=CLOCK_MONOTONIC，5=CLOCK_REALTIME_COARSE 等
- `flags`：`TIMER_ABSTIME=0x01` 时表示 `rqtp` 为绝对唤醒时间，否则为相对时长
- `rqtp`：请求的睡眠时间（`TimeSpec` 结构体指针）
- `rmtp`：若被信号中断，返回剩余时间

返回值：

- `SUCCESS (0)`：睡眠正常超时
- `EINVAL`：参数非法（指针为空、tv_nsec ≥ 1e9）
- `EINTR`：被信号中断，`rmtp` 已写入剩余时间

## 实现机制

### 相对时间模式（flags 不带 TIMER_ABSTIME）

```
rqtp → 计算 end = TimeSpec::now() + rqtp
     → wait_with_timeout(end)  注册到全局超时队列
     → block_current_and_run_next()  让出 CPU
     → 超时定时器中断 → 唤醒 → 返回 SUCCESS
     → 或被信号中断 → 计算剩余 = end - now → 写入 rmtp → 返回 EINTR
```

### 绝对时间模式（flags & TIMER_ABSTIME）

```
rqtp 为绝对 Unix 时间或单调时间
CLOCK_REALTIME (0/5)：rqtp 是 Unix 绝对时间
  → current_time() 转 Unix 纳秒 → 计算 remaining = rqtp - now
  → end = TimeSpec::now() + TimeSpec::from_ns(remaining)

CLOCK_MONOTONIC (1/4/6/7)：rqtp 已是 boot-time 兼容
  → 直接 end = rqtp，若已过期立即返回 SUCCESS
```

### 超时与信号交互

```
wait_with_timeout(task, end_time)
  → 插入全局 TIMEOUT_WAITQUEUE，按 end_time 排序
  → 定时器中断触发 do_wake_expired()
    → 遍历队列，唤醒所有 end_time <= now 的任务

信号到达：
  → sigpending 非空 → 返回 EINTR
  → rmtp 写入 end - now（剩余时间）
```

## 内核调用路径

```
trap_handler (SupervisorTimer)
  → do_wake_expired()         // 定时器中断：检查超时队列
  → set_next_trigger()
  → suspend_current_and_run_next()

用户态 sleep 线程恢复执行：
  → sys_clock_nanosleep 返回 SUCCESS/EINTR
```

## 与 nanosleep 的关系

`nanosleep(rqtp, rmtp)` 等价于 `clock_nanosleep(CLOCK_REALTIME, 0, rqtp, rmtp)` 的相对时间模式。

## 相关结构体

```rust
#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: usize,   // 秒
    pub tv_nsec: usize,  // 纳秒 (< 1_000_000_000)
}
```
