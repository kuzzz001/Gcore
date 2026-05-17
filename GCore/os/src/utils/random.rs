use rand_core::RngCore;

use crate::timer::get_time_ns;

pub struct Rng {
    pub state: u64,
}

impl RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let val = self.next_u64().to_le_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&val[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl Rng {
    pub fn positive_u32(&mut self) -> u32 {
        (self.next_u64() >> 33) as u32
    }
}

pub static mut RNG: Rng = Rng {
    state: 0x9e3779b97f4a7c15,
};

pub fn init_rng() {
    unsafe {
        RNG.state = RNG.state.wrapping_add(get_time_ns() as u64);
    }
}
