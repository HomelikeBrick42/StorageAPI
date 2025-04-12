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

    const DANGLING: Self::Handle = InlineStorageHandle(());

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
        old_alloc: Self::Handle,
    ) -> Result<(Self::Handle, usize), Self::Handle> {
        _ = old_layout;
        self.allocate(new_layout)
            .map_err(|StorageAllocError| old_alloc)
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_alloc: Self::Handle,
    ) -> Result<(Self::Handle, usize), Self::Handle> {
        _ = old_layout;
        self.allocate(new_layout)
            .map_err(|StorageAllocError| old_alloc)
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
