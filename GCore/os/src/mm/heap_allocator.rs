use crate::hal::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;
use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;

/// The buddy allocator for small allocations
static BUDDY_ALLOC: LockedHeap<32> = LockedHeap::empty();

/// Global heap memory space for the buddy allocator
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// Initialize the kernel heap
pub fn init_heap() {
    unsafe {
        BUDDY_ALLOC
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

const PAGE_SIZE: usize = 0x1000;

/// Maximum number of large allocations tracked simultaneously.
/// Each entry is 16 bytes (2 × usize). 1024 entries = 16KB.
const MAX_LARGE_ALLOCS: usize = 1024;

/// Static table for tracking large page-based allocations.
/// Uses pre-allocated arrays — NEVER allocates from the heap,
/// avoiding circular dependency in the global allocator.
///
/// `vaddrs[i] == 0` means slot is empty (vaddr 0 is never valid in our system).
struct LargeAllocTable {
    vaddrs: [usize; MAX_LARGE_ALLOCS],
    num_pages: [usize; MAX_LARGE_ALLOCS],
}

const EMPTY_TABLE: LargeAllocTable = LargeAllocTable {
    vaddrs: [0; MAX_LARGE_ALLOCS],
    num_pages: [0; MAX_LARGE_ALLOCS],
};

static LARGE_ALLOCS: Mutex<LargeAllocTable> = Mutex::new(EMPTY_TABLE);

struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Try buddy allocator first
        let ptr = BUDDY_ALLOC.alloc(layout);
        if !ptr.is_null() {
            return ptr;
        }

        // Fallback: use page allocator for allocations >= PAGE_SIZE
        if layout.size() >= PAGE_SIZE {
            let num_pages = (layout.size() + PAGE_SIZE - 1) / PAGE_SIZE;
            if let Some(start_ppn) = super::frames_alloc_contiguous_raw(num_pages) {
                let vaddr = start_ppn * PAGE_SIZE;
                // Track in the static table (no heap allocation needed)
                let mut table = LARGE_ALLOCS.lock();
                for i in 0..MAX_LARGE_ALLOCS {
                    if table.vaddrs[i] == 0 {
                        table.vaddrs[i] = vaddr;
                        table.num_pages[i] = num_pages;
                        return vaddr as *mut u8;
                    }
                }
                // Table full — leak the pages but system keeps running
                return vaddr as *mut u8;
            }
        }

        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        // Check if this is a page-allocated block
        let mut table = LARGE_ALLOCS.lock();
        for i in 0..MAX_LARGE_ALLOCS {
            if table.vaddrs[i] == addr {
                // Free the contiguous pages back to the frame allocator
                super::frames_dealloc_contiguous(addr / PAGE_SIZE, table.num_pages[i]);
                table.vaddrs[i] = 0;
                return;
            }
        }
        drop(table);

        // Buddy allocator dealloc
        BUDDY_ALLOC.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL_ALLOC: KernelAllocator = KernelAllocator;

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[allow(unused)]
/// 堆测试函数
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for i in 0..500 {
        assert_eq!(v[i], i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}
