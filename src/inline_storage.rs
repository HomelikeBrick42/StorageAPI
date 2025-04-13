use crate::{Storage, StorageAllocError, StorageHandle};
use core::{alloc::Layout, cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InlineStorageHandle(());

impl StorageHandle for InlineStorageHandle {}

pub struct InlineStorage<T>(UnsafeCell<MaybeUninit<T>>);

impl<T> InlineStorage<T> {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }
}

unsafe impl<T> Storage for InlineStorage<T> {
    type Handle = InlineStorageHandle;

    unsafe fn resolve(&self, InlineStorageHandle(()): &Self::Handle) -> NonNull<()> {
        unsafe { NonNull::new_unchecked(self.0.get().cast()) }
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        if layout.align() <= align_of::<T>() && layout.size() <= size_of::<T>() {
            Ok((InlineStorageHandle(()), size_of::<T>()))
        } else {
            Err(StorageAllocError)
        }
    }

    unsafe fn deallocate(&self, layout: Layout, InlineStorageHandle(()): Self::Handle) {
        _ = layout;
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_alloc: &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        _ = old_layout;
        _ = old_alloc;
        self.allocate(new_layout)
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        InlineStorageHandle(()): &Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        _ = old_layout;
        _ = new_layout;
        Ok((InlineStorageHandle(()), size_of::<T>()))
    }
}

impl<T> Default for InlineStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for InlineStorage<T> {
    fn clone(&self) -> Self {
        Self::new()
    }
}
