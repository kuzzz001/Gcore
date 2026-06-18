pub mod config;
pub mod kern_stack;
pub mod sbi;
pub mod smp;
pub mod sv39;
pub mod switch;
pub mod time;
pub mod trap;

#[path = "../../platform/riscv/qemu.rs"]
pub mod rv_board;

pub fn machine_init() {
    trap::init();
    // Timer interrupt is enabled later, after task system is ready.
    // See: rust_main() in main.rs
}

use time::set_next_trigger;

pub use trap::context::MachineContext;

pub type KernelPageTableImpl = sv39::Sv39PageTable;
pub type PageTableImpl = sv39::Sv39PageTable;
pub type TrapImpl = riscv::register::scause::Trap;
pub type InterruptImpl = riscv::register::scause::Interrupt;
pub type ExceptionImpl = riscv::register::scause::Exception;

pub fn bootstrap_init() {}
