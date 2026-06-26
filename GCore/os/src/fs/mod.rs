mod cache;
pub mod dev;
pub mod directory_tree;
mod ext4;
pub mod fat32;
pub mod file_trait;
mod filesystem;
mod layout;
pub mod poll;
#[cfg(feature = "swap")]
pub mod swap;
// Xein add this
pub mod dirent;
pub mod file_descriptor;
mod inode;
mod timestamp;
mod vfs;


pub use self::dev::{
    hwclock::*,
    // null::*,
    pipe::*,
    // socket::*, tty::*, zero::*
};

pub use self::layout::*;

pub use self::fat32::DiskInodeType;
pub use crate::drivers::block::BlockDevice;

use self::cache::PageCache;
use alloc::{
    string::String,
    sync::Arc,
};
pub use dirent::Dirent;
pub use file_descriptor::FileDescriptor;
use lazy_static::*;

lazy_static! {
    pub static ref ROOT_FD: Arc<FileDescriptor> = Arc::new(FileDescriptor::new(
        false,
        false,
        self::directory_tree::ROOT
            .open(".", OpenFlags::O_RDONLY | OpenFlags::O_DIRECTORY, true)
            .unwrap()
    ));
}
#[allow(unused)]
pub fn flush_preload() {
    extern "C" {
        fn sinitproc();
        fn einitproc();
        fn sbash();
        fn ebash();
        fn sbusybox();
        fn ebusybox();
    }
    println!(
        "sinitproc: {:X}, einitproc: {:X}, sbash: {:X}, ebash: {:X}, sbusybox: {:X}, ebusybox: {:X}",
        sinitproc as usize, einitproc as usize, sbash as usize, ebash as usize, sbusybox as usize, ebusybox as usize,
    );
    let initproc = ROOT_FD.open("initproc", OpenFlags::O_CREAT, false).unwrap();
    initproc.write(None, unsafe {
        core::slice::from_raw_parts(
            sinitproc as *const u8,
            einitproc as usize - sinitproc as usize,
        )
    });
    for ppn in crate::mm::PPNRange::new(
        crate::mm::PhysAddr::from(sinitproc as usize).floor(),
        crate::mm::PhysAddr::from(einitproc as usize).floor(),
    ) {
        crate::mm::frame_dealloc(ppn);
    }
    let bash = ROOT_FD.open("bash", OpenFlags::O_CREAT, false).unwrap();
    bash.write(None, unsafe {
        core::slice::from_raw_parts(sbash as *const u8, ebash as usize - sbash as usize)
    });
    for ppn in crate::mm::PPNRange::new(
        crate::mm::PhysAddr::from(sbash as usize).floor(),
        crate::mm::PhysAddr::from(ebash as usize).floor(),
    ) {
        crate::mm::frame_dealloc(ppn);
    }
    let busybox = ROOT_FD.open("busybox", OpenFlags::O_CREAT, false).unwrap();
    busybox.write(None, unsafe {
        core::slice::from_raw_parts(
            sbusybox as *const u8,
            ebusybox as usize - sbusybox as usize,
        )
    });
    for ppn in crate::mm::PPNRange::new(
        crate::mm::PhysAddr::from(sbusybox as usize).floor(),
        crate::mm::PhysAddr::from(ebusybox as usize).floor(),
    ) {
        crate::mm::frame_dealloc(ppn);
    }

    // Write dynamic linker to rootfs (LTP tests need it)
    #[cfg(feature = "loongarch64")]
    {
        extern "C" {
            fn sldmusl();
            fn eldmusl();
        }
        ROOT_FD
            .mkdir("lib64")
            .expect("Failed to create /lib64");
        let ld_musl = ROOT_FD
            .open(
                "lib64/ld-musl-loongarch-lp64d.so.1",
                OpenFlags::O_CREAT,
                false,
            )
            .expect("Failed to create ld-musl");
        ld_musl.write(None, unsafe {
            core::slice::from_raw_parts(
                sldmusl as *const u8,
                eldmusl as usize - sldmusl as usize,
            )
        });
        // Also put libc.so in /musl/lib/ for musl library resolution
        ROOT_FD
            .mkdir("musl")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /musl");
        ROOT_FD
            .mkdir("musl/lib")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /musl/lib");
        let libc_musl = ROOT_FD
            .open("musl/lib/libc.so", OpenFlags::O_CREAT, false)
            .expect("Failed to create musl libc.so");
        libc_musl.write(None, unsafe {
            core::slice::from_raw_parts(
                sldmusl as *const u8,
                eldmusl as usize - sldmusl as usize,
            )
        });
        // Free embedded pages
        for ppn in crate::mm::PPNRange::new(
            crate::mm::PhysAddr::from(sldmusl as usize).floor(),
            crate::mm::PhysAddr::from(eldmusl as usize).floor(),
        ) {
            crate::mm::frame_dealloc(ppn);
        }
    }

    #[cfg(feature = "riscv")]
    {
        extern "C" {
            fn sldmusl();
            fn eldmusl();
            fn sldglibc();
            fn eldglibc();
        }
        // musl: libc.so IS the dynamic linker, just at /lib/ld-musl-riscv64.so.1
        let ld_musl = ROOT_FD
            .open(
                "lib/ld-musl-riscv64.so.1",
                OpenFlags::O_CREAT,
                false,
            )
            .expect("Failed to create ld-musl-riscv64");
        ld_musl.write(None, unsafe {
            core::slice::from_raw_parts(
                sldmusl as *const u8,
                eldmusl as usize - sldmusl as usize,
            )
        });
        // glibc: ld-linux-riscv64-lp64d.so.1
        let ld_glibc = ROOT_FD
            .open(
                "lib/ld-linux-riscv64-lp64d.so.1",
                OpenFlags::O_CREAT,
                false,
            )
            .expect("Failed to create ld-linux-riscv64");
        ld_glibc.write(None, unsafe {
            core::slice::from_raw_parts(
                sldglibc as *const u8,
                eldglibc as usize - sldglibc as usize,
            )
        });
        // Also put libc.so in /musl/lib/ and /glibc/lib/
        ROOT_FD
            .mkdir("musl")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /musl");
        ROOT_FD
            .mkdir("musl/lib")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /musl/lib");
        let libc_musl = ROOT_FD
            .open("musl/lib/libc.so", OpenFlags::O_CREAT, false)
            .expect("Failed to create musl libc.so");
        libc_musl.write(None, unsafe {
            core::slice::from_raw_parts(
                sldmusl as *const u8,
                eldmusl as usize - sldmusl as usize,
            )
        });
        ROOT_FD
            .mkdir("glibc")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /glibc");
        ROOT_FD
            .mkdir("glibc/lib")
            .or_else(|e| if e == -1 { Err(e) } else { Ok(()) })
            .expect("Failed to create /glibc/lib");
        let libc_glibc = ROOT_FD
            .open("glibc/lib/libc.so.6", OpenFlags::O_CREAT, false)
            .expect("Failed to create glibc libc.so.6");
        libc_glibc.write(None, unsafe {
            core::slice::from_raw_parts(
                sldglibc as *const u8,
                eldglibc as usize - sldglibc as usize,
            )
        });
        // Free embedded pages (two separate ranges)
        for ppn in crate::mm::PPNRange::new(
            crate::mm::PhysAddr::from(sldmusl as usize).floor(),
            crate::mm::PhysAddr::from(eldmusl as usize).floor(),
        ) {
            crate::mm::frame_dealloc(ppn);
        }
        for ppn in crate::mm::PPNRange::new(
            crate::mm::PhysAddr::from(sldglibc as usize).floor(),
            crate::mm::PhysAddr::from(eldglibc as usize).floor(),
        ) {
            crate::mm::frame_dealloc(ppn);
        }
    }
}
