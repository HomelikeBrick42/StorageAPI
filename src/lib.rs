#![doc = include_str!("../README.md")]
#![no_std]
#![feature(
    ptr_metadata,
    layout_for_ptr,
    alloc_layout_extra,
    unsize,
    coerce_unsized,
    dispatch_from_dyn,
    dropck_eyepatch
)]

pub use global_storage::Global;
pub use inline_storage::InlineStorage;
pub use slot_storage::SlotStorage;
pub use storage_box::Box;
pub use storage_string::String;
pub use storage_vec::Vec;

mod global_storage;
mod inline_storage;
mod slot_storage;
mod storage_box;
mod storage_string;
mod storage_vec;

pub mod storages {
    pub use crate::global_storage::{Global, GlobalHandle};
    pub use crate::inline_storage::{InlineStorage, InlineStorageHandle};
    pub use crate::slot_storage::{SlotStorage, SlotStorageHandle};
}

pub mod collections {
    pub use crate::storage_box::Box;
    pub use crate::storage_string::String;
    pub use crate::storage_vec::{InsertError, PushError, Vec};
}

use core::{alloc::Layout, fmt::Debug, hash::Hash, ptr::NonNull};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageAllocError;

pub trait StorageHandle: Debug + Eq + Ord + Hash {}

/// # Safety
/// TODO
pub unsafe trait Storage {
    type Handle: StorageHandle;

    /// Returns a pointer to the allocation represented by `handle`
    /// # Safety
    /// `handle` must be valid
    unsafe fn resolve(&self, handle: &Self::Handle) -> NonNull<()>;

    /// Unless `Self` implements [`MultipleStorage`] this will invalidate any previous allocations
    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError>;

    /// # Safety
    /// `layout` must be the same layout that was used to allocate it, though the size may by greater as long as its less than the available capacity returned by any of the allocation methods
    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle);

    /// # Safety
    /// TODO
    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError>;

    /// # Safety
    /// TODO
    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError>;
}

/// # Safety
/// This trait can only be implemented if calling [`Storage::allocate`] will not invalidate previous allocations
pub unsafe trait MultipleStorage: Storage {}

unsafe impl<T: MultipleStorage + ?Sized> Storage for &T {
    type Handle = T::Handle;

    unsafe fn resolve(&self, handle: &Self::Handle) -> NonNull<()> {
        unsafe { T::resolve(self, handle) }
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        T::allocate(self, layout)
    }

    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle) {
        unsafe { T::deallocate(self, layout, handle) }
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::grow(self, old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::shrink(self, old_layout, new_layout, handle) }
    }
}

unsafe impl<T: MultipleStorage + ?Sized> MultipleStorage for &T {}

unsafe impl<T: Storage + ?Sized> Storage for &mut T {
    type Handle = T::Handle;

    unsafe fn resolve(&self, handle: &Self::Handle) -> NonNull<()> {
        unsafe { T::resolve(self, handle) }
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        T::allocate(self, layout)
    }

    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle) {
        unsafe { T::deallocate(self, layout, handle) }
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::grow(self, old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::shrink(self, old_layout, new_layout, handle) }
    }
}

unsafe impl<T: MultipleStorage + ?Sized> MultipleStorage for &mut T {}
