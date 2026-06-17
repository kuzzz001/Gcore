use alloc::{sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{config::PAGE_SIZE, drivers::BLOCK_DEVICE, hal::BLOCK_SZ};

use super::directory_tree::FILE_SYSTEM;
use lazy_static::*;

lazy_static! {
    pub static ref SWAP_DEVICE: Mutex<Swap> = Mutex::new(Swap::new(16));
}

#[derive(Debug)]
pub struct SwapTracker(pub usize);

impl Drop for SwapTracker {
    fn drop(&mut self) {
        SWAP_DEVICE.lock().discard(self.0);
    }
}

pub struct Swap {
    bitmap: Vec<u64>,
    block_ids: Vec<usize>,
    /// Whether swap is actually usable (false when no blocks allocated)
    usable: bool,
}
const BLK_PER_PG: usize = PAGE_SIZE / BLOCK_SZ;
const SWAP_SIZE: usize = 1024 * 1024;
impl Swap {
    /// size: the number of megabytes in swap
    pub fn new(size: usize) -> Self {
        let blocks = size * (SWAP_SIZE / BLOCK_SZ); // 1MiB = 512B * 2048
        let allocated = FILE_SYSTEM.alloc_blocks(blocks);
        let usable = !allocated.is_empty() && allocated.len() == blocks;
        let bit = if usable { size * (SWAP_SIZE / PAGE_SIZE) } else { 0 }; // 1MiB = 4KiB*256
        let vec_len = if usable { bit / usize::MAX.count_ones() as usize } else { 0 };
        let mut bitmap = Vec::<u64>::with_capacity(vec_len);
        bitmap.resize(bitmap.capacity(), 0);
        Self {
            bitmap,
            block_ids: allocated,
            usable,
        }
    }
    fn read_page(block_ids: &[usize], buf: &mut [u8]) {
        assert!(block_ids[0] + BLK_PER_PG - 1 == block_ids[BLK_PER_PG - 1]);
        BLOCK_DEVICE.read_block(block_ids[0], buf);
    }
    fn write_page(block_ids: &[usize], buf: &[u8]) {
        assert!(block_ids[0] + (BLK_PER_PG - 1) == block_ids[BLK_PER_PG - 1]);
        BLOCK_DEVICE.write_block(block_ids[0], buf);
    }
    fn set_bit(&mut self, pos: usize) {
        self.bitmap[pos / 64] |= 1 << (pos % 64);
    }
    fn clear_bit(&mut self, pos: usize) {
        self.bitmap[pos / 64] &= !(1 << (pos % 64));
    }
    fn alloc_page(&self) -> Option<usize> {
        for (i, bit) in self.bitmap.iter().enumerate() {
            if *bit == u64::MAX {
                continue; // 所有 64 位都已被占用，跳过
            }
            let free_bit = (!*bit).trailing_zeros() as usize;
            return Some(i * 64 + free_bit);
        }
        None
    }
    fn get_block_ids(&self, swap_id: usize) -> &[usize] {
        &self.block_ids[swap_id * BLK_PER_PG + 0..swap_id * BLK_PER_PG + BLK_PER_PG]
    }
    pub fn read(&mut self, swap_id: usize, buf: &mut [u8]) {
        if !self.usable { return; }
        Self::read_page(self.get_block_ids(swap_id), buf);
    }
    pub fn write(&mut self, buf: &[u8]) -> Option<Arc<SwapTracker>> {
        if !self.usable { return None; }
        if let Some(swap_id) = self.alloc_page() {
            Self::write_page(self.get_block_ids(swap_id), buf);
            self.set_bit(swap_id);
            Some(Arc::new(SwapTracker(swap_id)))
        } else {
            None
        }
    }    
    #[inline(always)]
    pub fn discard(&mut self, swap_id: usize) {
        self.clear_bit(swap_id);
    }
}
