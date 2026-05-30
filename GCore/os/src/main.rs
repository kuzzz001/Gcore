#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(string_remove_matches)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![feature(lang_items)]
#![feature(trait_upcasting)]
#![feature(core_intrinsics)]
pub use hal::config;
extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
mod console;
mod drivers;
mod fs;
mod hal;
mod lang_items;
mod math;
mod mm;
mod net;
mod syscall;
mod task;
mod timer;
mod utils;

#[cfg(feature = "block_mem")]
use crate::config::DISK_IMAGE_BASE;
use crate::hal::bootstrap_init;
use crate::hal::machine_init;
#[cfg(feature = "riscv")]
core::arch::global_asm!(include_str!("hal/arch/riscv/entry.asm"));
#[cfg(all(feature = "block_mem", feature = "loongarch64"))]
core::arch::global_asm!(include_str!("load_img.S"));
#[cfg(all(feature = "block_mem", feature = "riscv"))]
core::arch::global_asm!(include_str!("load_img-rv.S"));
#[cfg(all(not(feature = "block_mem"), feature = "loongarch64"))]
core::arch::global_asm!(include_str!("preload_app.S"));
#[cfg(all(not(feature = "block_mem"), feature = "riscv"))]
core::arch::global_asm!(include_str!("preload_app-rv.S"));

fn mem_clear() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    #[cfg(feature = "zero_init")]
    unsafe {
        core::slice::from_raw_parts_mut(
            sbss as usize as *mut u8,
            crate::config::MEMORY_END - sbss as usize,
        )
        .fill(0);
    }
    #[cfg(not(feature = "zero_init"))]
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[cfg(feature = "block_mem")]
fn move_to_high_address() {
    extern "C" {
        fn simg();
        fn eimg();
    }
    unsafe {
        // 加载根文件系统镜像
        let img =
            core::slice::from_raw_parts(simg as usize as *mut u8, eimg as usize - simg as usize);
        // 以DISK_IMAGE_BASE到MEMORY_END上的内存作为根文件系统镜像
        let mem_disk = core::slice::from_raw_parts_mut(
            DISK_IMAGE_BASE as *mut u8,
            // 大小为256MB
            0x1000_0000,
        );
        // 清空mem_disk上的内容
        mem_disk.fill(0);
        mem_disk[..img.len()].copy_from_slice(img);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    bootstrap_init();
    mem_clear();
    #[cfg(feature = "block_mem")]
    move_to_high_address();
    console::log_init();
    println!("[kernel] Console initialized.");
    mm::init();
    println!("[kernel] Hello, world!");

    machine_init();
    timer::init_time_source(&timer::MTime);
    #[cfg(feature = "board_rvqemu")]
    timer::init_rtc_time();
    utils::random::init_rng();

    fs::directory_tree::init_fs();
    net::config::init();
    #[cfg(feature = "block_virt")]
    println!("[kernel] block in virt mode!");
    #[cfg(feature = "oom_handler")]
    println!("[kernel] oom_handler is enabled!");
    #[cfg(feature = "riscv")]
    fs::flush_preload();
    task::add_initproc();
    // note that in run_tasks(), there is yet *another* pre_start_init(),
    // which is used to turn on interrupts in some archs like LoongArch.
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

#[cfg(test)]
fn test_runner(_tests: &[&dyn Fn()]) {}
