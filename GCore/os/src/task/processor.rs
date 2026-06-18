use super::{__switch, do_wake_expired};
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
const MAX_HARTS: usize = 8;

#[cfg(feature = "riscv")]
use crate::hal::arch::riscv::smp;

#[cfg(feature = "riscv")]
fn hart_id() -> usize { smp::hart_id() }

#[cfg(not(feature = "riscv"))]
fn hart_id() -> usize { 0 }
use crate::hal::TrapContext;
use alloc::sync::Arc;
use spin::Mutex;
use lazy_static::lazy_static;

/// 每个 hart 的独立 Processor 对象
pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

impl Processor {
    pub const fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    static ref PROCESSORS: Mutex<[Processor; MAX_HARTS]> = Mutex::new([
        Processor::new(), Processor::new(), Processor::new(), Processor::new(),
        Processor::new(), Processor::new(), Processor::new(), Processor::new(),
    ]);
}

/// 每个 hart 的核心调度循环
pub fn run_tasks() {
    // Enable timer interrupt now that we have tasks ready
    #[cfg(feature = "riscv")]
    {
        crate::hal::arch::riscv::trap::enable_timer_interrupt();
        crate::hal::arch::riscv::time::set_next_trigger();
    }

    let is_primary = hart_id() == 0;
    loop {
        let task = {
            let mut guard = PROCESSORS.lock();
            fetch_task().map(|task| {
                let id = hart_id();
                let processor = &mut guard[id];
                let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
                let next_task_cx_ptr = {
                    let mut task_inner = task.acquire_inner_lock();
                    task_inner.task_status = TaskStatus::Running;
                    &task_inner.task_cx as *const TaskContext
                };
                processor.current = Some(task);
                (idle_task_cx_ptr, next_task_cx_ptr)
            })
        };
        if let Some((idle_ptr, next_ptr)) = task {
            unsafe {
                __switch(idle_ptr, next_ptr);
            }
        } else {
            if is_primary {
                do_wake_expired();
            } else {
                do_wake_expired();
                unsafe { core::arch::asm!("wfi") };
            }
        }
    }
}

/// 取出当前 hart 正在运行的任务
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    let id = hart_id();
    PROCESSORS.lock()[id].take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    let id = hart_id();
    PROCESSORS.lock()[id].current()
}

/// 获取当前 hart 正在运行的任务的用户态页表令牌
pub fn current_user_token() -> usize {
    current_task().unwrap().get_user_token()
}

/// 获取当前 hart 正在运行的任务的陷阱上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().acquire_inner_lock().get_trap_cx()
}

/// 切换到空闲任务上下文（当前 hart）
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let id = hart_id();
    let mut guard = PROCESSORS.lock();
    let idle_task_cx_ptr = guard[id].get_idle_task_cx_ptr();
    // Guard must be dropped before __switch, but we took the pointer already
    drop(guard);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
