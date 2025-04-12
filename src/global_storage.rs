extern crate alloc;

use crate::{MultipleStorage, Storage, StorageAllocError, StorageHandle};
use core::{alloc::Layout, ptr::NonNull};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalHandle(pub NonNull<()>);

impl StorageHandle for GlobalHandle {}

#[derive(Clone, Copy)]
pub struct Global;

unsafe impl Storage for Global {
    type Handle = GlobalHandle;

    const DANGLING: Self::Handle = GlobalHandle(NonNull::dangling());

    unsafe fn resolve(&self, handle: &Self::Handle) -> NonNull<()> {
        handle.0
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        match layout.size() {
            0 => Ok((GlobalHandle(layout.dangling().cast()), 0)),
            size => match unsafe { NonNull::new(alloc::alloc::alloc(layout).cast()) } {
                Some(ptr) => Ok((GlobalHandle(ptr), size)),
                None => Err(StorageAllocError),
            },
        }
    }

    unsafe fn deallocate(&self, layout: Layout, handle: Self::Handle) {
        match layout.size() {
            0 => (),
            _ => unsafe { alloc::alloc::dealloc(handle.0.as_ptr().cast(), layout) },
        }
    }

    unsafe fn grow(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), Self::Handle> {
        unsafe { self.realloc(old_layout, new_layout, handle) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), Self::Handle> {
        unsafe { self.realloc(old_layout, new_layout, handle) }
    }
}

impl Global {
    unsafe fn realloc(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_alloc: GlobalHandle,
    ) -> Result<(GlobalHandle, usize), GlobalHandle> {
        match (old_layout.size(), new_layout.size()) {
            (0, 0) => Ok((old_alloc, 0)),
            (0, _) => self
                .allocate(new_layout)
                .map_err(|StorageAllocError| old_alloc),
            (_, 0) => match self.allocate(new_layout) {
                Ok(new_alloc) => {
                    unsafe {
                        self.deallocate(old_layout, old_alloc);
                    }
                    Ok(new_alloc)
                }
                Err(StorageAllocError) => Err(old_alloc),
            },
            (old_size, new_size) => {
                if old_layout.align() >= new_layout.align() {
                    match unsafe {
                        NonNull::new(
                            alloc::alloc::realloc(
                                old_alloc.0.as_ptr().cast(),
                                old_layout,
                                new_size,
                            )
                            .cast(),
                        )
                    } {
                        Some(ptr) => Ok((GlobalHandle(ptr), new_size)),
                        None => Err(old_alloc),
                    }
                } else {
                    match self.allocate(new_layout) {
                        Ok((new_alloc, _)) => {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    old_alloc.0.as_ptr().cast::<u8>(),
                                    new_alloc.0.as_ptr().cast::<u8>(),
                                    old_size,
                                );
                                self.deallocate(old_layout, old_alloc);
                            }
                            Ok((new_alloc, new_size))
                        }
                        Err(StorageAllocError) => Err(old_alloc),
                    }
                }
            }
        }
    }
}

unsafe impl MultipleStorage for Global {}
