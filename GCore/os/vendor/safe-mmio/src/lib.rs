// Copyright 2025 The safe-mmio Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Types for safe MMIO device access, especially in systems with an MMU.

#![no_std]
#![feature(const_mut_refs)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(raw_ref_op)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_arch = "aarch64")]
mod aarch64_mmio;
pub mod fields;
mod physical;
#[cfg(not(target_arch = "aarch64"))]
mod volatile_mmio;

pub use self::fields::Field;
pub use self::fields::FieldValue;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// An interface for a region of memory-mapped I/O.
///
/// `P` is typically a [`RegisterBlock`] and `Mmio` can only be constructed for types
/// where an access width is defined.
///
/// `Mmio` dereferences to `P` and the access width is enforced by the
/// [`Deref`](core::ops::Deref) and [`DerefMut`](core::ops::DerefMut) implementations.
///
/// [`RegisterBlock`]: crate::fields::RegisterBlock
pub struct Mmio<P>
where
    P: MmioWidth + ?Sized,
{
    _phantom: PhantomData<P>,
    ptr: NonNull<P>,
}

/// Indicates that a type has a well-defined MMIO access width (u8, u16, u32 or u64)
pub trait MmioWidth {
    /// The MMIO access width in bits
    const WIDTH: u8;
}

impl<P> Mmio<P>
where
    P: MmioWidth + ?Sized,
{
    /// Create a new `Mmio` from a physical address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the given address is valid for MMIO access for the lifetime
    /// of the returned `Mmio`, and that the address is correctly aligned for the access width.
    pub unsafe fn new(addr: usize) -> Self {
        Self {
            _phantom: PhantomData,
            ptr: NonNull::new(addr as *mut P).unwrap(),
        }
    }
}

impl<P> core::ops::Deref for Mmio<P>
where
    P: MmioWidth + ?Sized,
{
    type Target = P;

    fn deref(&self) -> &P {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<P> core::ops::DerefMut for Mmio<P>
where
    P: MmioWidth + ?Sized,
{
    fn deref_mut(&mut self) -> &mut P {
        unsafe { &mut *self.ptr.as_ptr() }
    }
}

// Safety: Mmio provides exclusive access to a memory region, so it is safe to Send and Sync
// as long as the underlying P is.
unsafe impl<P> Send for Mmio<P> where P: MmioWidth + Send + ?Sized {}
unsafe impl<P> Sync for Mmio<P> where P: MmioWidth + Sync + ?Sized {}
