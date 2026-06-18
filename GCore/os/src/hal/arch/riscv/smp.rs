// SMP (Symmetric Multi-Processing) support for RISC-V
use core::arch::asm;

/// Maximum number of harts (hardware threads)
pub const MAX_HARTS: usize = 8;

/// Get current hart ID from tp register (set by entry.asm from OpenSBI a0)
/// If tp is not properly initialized by firmware, fall back to 0 (boot hart).
#[inline(always)]
pub fn hart_id() -> usize {
    let id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) id);
    }
    // Sanity check: hart ID should be < MAX_HARTS
    if id < MAX_HARTS { id } else { 0 }
}

/// Send IPI to target hart(s)
pub fn send_ipi(hart_mask: usize) {
    crate::hal::arch::riscv::sbi::send_ipi(hart_mask);
}

/// TLB shootdown: invalidate all TLB entries on all harts
pub fn tlb_shootdown() {
    crate::hal::arch::riscv::sbi::remote_sfence_vma(0, 0);
}

/// Check if we are the boot hart (hart 0)
pub fn is_boot_hart() -> bool {
    hart_id() == 0
}
