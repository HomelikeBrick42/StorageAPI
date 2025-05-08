use crate::{StableStorage, Storage, StorageAllocError, StorageHandle};
use core::{alloc::Layout, cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull};

/// The [`StorageHandle`] for [`SlotStorage`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotStorageHandle {
    offset: usize,
}

impl StorageHandle for SlotStorageHandle {}

/// Represents a buffer of bytes that can be allocated from
///
/// This is similar to [`InlineStorage`](crate::InlineStorage) but because it doesnt store its buffer directly,
/// it can potentially allocate memory with large alignments that are not known at compile time
pub struct SlotStorage<'a> {
    storage: &'a [UnsafeCell<MaybeUninit<u8>>],
}

unsafe impl Send for SlotStorage<'_> {}

unsafe impl Storage for SlotStorage<'_> {
    type Handle = SlotStorageHandle;

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()> {
        unsafe {
            NonNull::new_unchecked(self.storage.as_ptr().byte_add(handle.offset).cast_mut()).cast()
        }
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        let offset = unsafe { self.resolve(SlotStorageHandle { offset: 0 }).cast::<u8>() }
            .align_offset(layout.align());
        if offset == usize::MAX {
            return Err(StorageAllocError);
        }

        let size = self
            .storage
            .len()
            .checked_sub(offset)
            .ok_or(StorageAllocError)?;

        Ok((SlotStorageHandle { offset }, size))
    }

    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle) {
        _ = layout;
        _ = handle;
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        let (new_handle, new_size) = self.allocate(new_layout)?;
        unsafe {
            let old_alloc = self.resolve(old_handle).cast::<u8>();
            let new_alloc = self.resolve(new_handle).cast::<u8>();
            new_alloc.copy_from(old_alloc, old_layout.size());
        }
        Ok((new_handle, new_size))
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        let (new_handle, new_size) = self.allocate(new_layout)?;
        unsafe {
            let old_alloc = self.resolve(old_handle).cast::<u8>();
            let new_alloc = self.resolve(new_handle).cast::<u8>();
            new_alloc.copy_from(old_alloc, new_layout.size());
            _ = old_layout;
        }
        Ok((new_handle, new_size))
    }
}

unsafe impl StableStorage for SlotStorage<'_> {}
