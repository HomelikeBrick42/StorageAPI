#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]
#![no_std]
#![cfg_attr(
    feature = "nightly",
    feature(
        ptr_metadata,
        layout_for_ptr,
        alloc_layout_extra,
        unsize,
        coerce_unsized,
        dispatch_from_dyn,
        dropck_eyepatch,
        tuple_trait,
        unboxed_closures,
        fn_traits,
        allocator_api
    )
)]

pub use global_storage::Global;
pub use inline_storage::InlineStorage;
pub use sharable_storage_wrapper::ShareableStorageWrapper;
pub use slot_storage::SlotStorage;
pub use storage_box::Box;
pub use storage_string::String;
pub use storage_vec::Vec;

mod global_storage;
mod inline_storage;
mod sharable_storage_wrapper;
mod slot_storage;
mod storage_box;
mod storage_string;
mod storage_vec;

/// The types that implement [`Storage`]
pub mod storages {
    pub use crate::global_storage::{Global, GlobalHandle};
    pub use crate::inline_storage::{InlineStorage, InlineStorageHandle};
    pub use crate::sharable_storage_wrapper::ShareableStorageWrapper;
    pub use crate::slot_storage::{SlotStorage, SlotStorageHandle};
}

/// The collections that use a [`Storage`] for their backing data
pub mod collections {
    pub use crate::storage_box::Box;
    pub use crate::storage_string::String;
    pub use crate::storage_vec::{InsertError, PushError, Vec, VecIntoIter};
}

use core::{alloc::Layout, fmt::Debug, hash::Hash, ptr::NonNull};

/// The error returned when allocating using a [`Storage`] fails
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageAllocError;

/// The trait that all [`Storage::Handle`]s must implement
pub trait StorageHandle: Debug + Eq + Ord + Hash + Copy {}

/// The trait for allocating memory in a storage
///
/// # Safety
/// - [`Storage::resolve`] must return a valid pointer to the allocation when passed a valid [`Storage::Handle`]
pub unsafe trait Storage {
    /// The [`StorageHandle`] type that represents an allocation by this [`Storage`]
    type Handle: StorageHandle;

    /// Returns a pointer to the allocation represented by `handle`
    /// # Safety
    /// `handle` must be valid
    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()>;

    /// Allocates memory with a layout specified by `layout`
    ///
    /// Also returns the total amount of bytes actually allocated, which may be more than requested by `layout`
    ///
    /// Unless `Self` implements [`MultipleStorage`] this will invalidate any previous allocations
    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError>;

    /// Deallocates (and invalidates) a [`StorageHandle`] that was allocated with this [`Storage`]
    ///
    /// # Safety
    /// - `layout` must be the same layout that was used to allocate it,
    ///   though the size may by greater as long as its less than the available capacity returned by any of the allocation methods ([`Storage::allocate`]/[`Storage::grow`]/[`Storage::shrink`])
    /// - `handle` must be valid
    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle);

    /// Grows (increases the size of) an allocation
    ///
    /// Similar to [`Storage::allocate`] this method also returns the number of bytes actually allocated, which may be more than requested with `new_layout`
    ///
    /// # Safety
    /// - `new_layout.size() >= old_layout.size()`
    /// - `handle` must be valid
    /// - if this method succeeds, `handle` is now invalid and cannot be used
    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError>;

    /// Shrinks (decreases the size of) an allocation
    ///
    /// Similar to [`Storage::allocate`] this method also returns the number of bytes actually allocated, which may be more than requested with `new_layout`
    ///
    /// # Safety
    /// - `new_layout.size() <= old_layout.size()`
    /// - `handle` must be valid
    /// - if this method succeeds, `handle` is now invalid and cannot be used
    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError>;
}

/// Allows making shared copies of a [`Storage`] that all act as-if they were the original
///
/// # Safety
/// This trait can only be implemented if the value returned by [`ShareableStorage::make_shared_copy`] acts the same as `self`
pub unsafe trait ShareableStorage: Storage {
    /// Makes a shared copy of `self` that acts as-if it was `self`
    ///
    /// # Safety
    /// This method is `unsafe` because many data structures assume that they have the only copy of the [`Storage`] and that their handles wont randomly get invalidated (like from something calling [`Storage::allocate`]),
    /// so `unsafe` code must be careful when exposing these copies to safe code
    unsafe fn make_shared_copy(&self) -> Self;
}

/// A marker trait related to [`Storage`] that guarentees that multiple allocations can be made from a [`Storage`] without invalidating old ones
///
/// # Safety
/// This trait can only be implemented if calling [`Storage::allocate`] will not invalidate previous allocations
pub unsafe trait MultipleStorage: Storage {}

/// A marker trait related to [`Storage`] that guarentees that moving the [`Storage`] wont invalidate pointers/references into it
///
/// # Safety
/// This trait can only be implemented if moving `Self` will not invalidate pointers/references that have been retrived from [`Storage::resolve`]
pub unsafe trait StableStorage: Storage {}

unsafe impl<T: MultipleStorage + ?Sized> Storage for &T {
    type Handle = T::Handle;

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()> {
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
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::grow(self, old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::shrink(self, old_layout, new_layout, handle) }
    }
}

unsafe impl<T: MultipleStorage + ?Sized> MultipleStorage for &T {}
unsafe impl<T: MultipleStorage + ?Sized> StableStorage for &T {}
unsafe impl<T: MultipleStorage + ?Sized> ShareableStorage for &T {
    unsafe fn make_shared_copy(&self) -> Self {
        self
    }
}

unsafe impl<T: Storage + ?Sized> Storage for &mut T {
    type Handle = T::Handle;

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()> {
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
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::grow(self, old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::shrink(self, old_layout, new_layout, handle) }
    }
}

unsafe impl<T: MultipleStorage + ?Sized> MultipleStorage for &mut T {}
unsafe impl<T: Storage + ?Sized> StableStorage for &mut T {}
