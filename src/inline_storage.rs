use crate::{Storage, StorageAllocError, StorageHandle};
use core::{alloc::Layout, cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull};

/// The [`StorageHandle`] for [`InlineStorage`],
/// this is a ZST
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InlineStorageHandle(());

impl StorageHandle for InlineStorageHandle {}

/// Represents an inline storage with the size/alignment requirements of `T`,
/// this [`Storage`] type being possible of the main reasons for the [`Storage`] API existing
pub struct InlineStorage<T>(UnsafeCell<MaybeUninit<T>>);

unsafe impl<T> Send for InlineStorage<T> {}
unsafe impl<T> Sync for InlineStorage<T> {}

impl<T> InlineStorage<T> {
    /// Constructs a new [`InlineStorage`]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }
}

unsafe impl<T> Storage for InlineStorage<T> {
    type Handle = InlineStorageHandle;

    unsafe fn resolve(&self, InlineStorageHandle(()): Self::Handle) -> NonNull<()> {
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
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        _ = old_layout;
        _ = old_alloc;
        self.allocate(new_layout)
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        InlineStorageHandle(()): Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        _ = old_layout;
        _ = new_layout;
        self.allocate(new_layout)
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
