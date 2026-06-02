use crate::hal::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;
use core::alloc::{GlobalAlloc, Layout};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use spin::Mutex;

// The buddy allocator for small allocations
static BUDDY_ALLOC: LockedHeap<32> = LockedHeap::empty();

// Track large page-based allocations (key = virtual address)
lazy_static! {
    static ref LARGE_ALLOCS: Mutex<BTreeMap<usize, Vec<Arc<super::FrameTracker>>>> =
        Mutex::new(BTreeMap::new());
}

// Global heap memory space for the buddy allocator
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// Initialize the kernel heap
pub fn init_heap() {
    unsafe {
        BUDDY_ALLOC
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Try buddy allocator first
        let ptr = BUDDY_ALLOC.alloc(layout);
        if !ptr.is_null() {
            return ptr;
        }

        // Fallback: use page allocator for allocations >= PAGE_SIZE
        const PAGE_SIZE: usize = 0x1000;
        if layout.size() >= PAGE_SIZE {
            let num_pages = (layout.size() + PAGE_SIZE - 1) / PAGE_SIZE;
            if let Some(frames) = super::frames_alloc_contiguous(num_pages) {
                let vaddr = frames[0].ppn.0 * PAGE_SIZE;
                LARGE_ALLOCS.lock().insert(vaddr, frames);
                return vaddr as *mut u8;
            }
        }

        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        // Check if this is a page-allocated block
        let frames = LARGE_ALLOCS.lock().remove(&addr);
        if let Some(frames) = frames {
            // Drop frames AFTER releasing the BTreeMap lock to avoid deadlock
            // (FrameTracker drop takes FRAME_ALLOCATOR write lock)
            drop(frames);
            return;
        }

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