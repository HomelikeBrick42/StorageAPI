use crate::{MultipleStorage, ShareableStorage, StableStorage, Storage, StorageAllocError};
use core::{alloc::Layout, ptr::NonNull};

/// A wrapper to turn a `&mut impl Storage` into a [`ShareableStorage`]
pub struct ShareableStorageWrapper<'a, T: ?Sized>(&'a T);

impl<'a, T: ?Sized> ShareableStorageWrapper<'a, T> {
    /// Constructs a [`ShareableStorageWrapper`]
    pub fn new(storage: &'a mut T) -> Self {
        Self(storage)
    }

    /// Unsafely constructs a [`ShareableStorageWrapper`] from a `&impl Storage`
    ///
    /// # Safety
    /// This has the same safety requirements as [`ShareableStorage::make_shared_copy`]
    pub unsafe fn new_unchecked(storage: &'a T) -> Self {
        Self(storage)
    }
}

unsafe impl<T: Storage + ?Sized> Storage for ShareableStorageWrapper<'_, T> {
    type Handle = T::Handle;

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()> {
        unsafe { T::resolve(self.0, handle) }
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        T::allocate(self.0, layout)
    }

    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle) {
        unsafe { T::deallocate(self.0, layout, handle) }
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::grow(self.0, old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { T::shrink(self.0, old_layout, new_layout, handle) }
    }
}

unsafe impl<T: Storage + ?Sized> ShareableStorage for ShareableStorageWrapper<'_, T> {
    unsafe fn make_shared_copy(&self) -> Self {
        Self { ..*self }
    }
}

unsafe impl<T: MultipleStorage + ?Sized> MultipleStorage for ShareableStorageWrapper<'_, T> {}
unsafe impl<T: Storage + ?Sized> StableStorage for ShareableStorageWrapper<'_, T> {}
