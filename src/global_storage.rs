extern crate alloc;

use crate::{MultipleStorage, Storage, StorageAllocError, StorageHandle};
use core::{alloc::Layout, ptr::NonNull};

/// The [`StorageHandle`] for [`Global`],
/// this is a wrapper around a [`NonNull<()>`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalHandle(pub NonNull<()>);

unsafe impl Send for GlobalHandle {}
unsafe impl Sync for GlobalHandle {}

impl StorageHandle for GlobalHandle {}

/// This represents the global allocator registered with the `#[global_allocator]` attribute
///
/// See [`GlobalAlloc`](alloc::alloc::GlobalAlloc) for more info
#[derive(Default, Clone, Copy)]
pub struct Global;

unsafe impl Storage for Global {
    type Handle = GlobalHandle;

    unsafe fn resolve(&self, handle: Self::Handle) -> NonNull<()> {
        handle.0
    }

    fn allocate(&self, layout: Layout) -> Result<(Self::Handle, usize), StorageAllocError> {
        match layout.size() {
            0 => Ok((
                GlobalHandle(unsafe {
                    NonNull::new_unchecked(core::ptr::without_provenance_mut(layout.align()))
                }),
                0,
            )),
            size => match NonNull::new(unsafe { alloc::alloc::alloc(layout) }.cast()) {
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
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { self.realloc(old_layout, new_layout, handle.0) }
    }

    unsafe fn shrink(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        handle: Self::Handle,
    ) -> Result<(Self::Handle, usize), StorageAllocError> {
        unsafe { self.realloc(old_layout, new_layout, handle.0) }
    }
}

impl Global {
    unsafe fn realloc(
        &self,
        old_layout: Layout,
        new_layout: Layout,
        old_alloc: NonNull<()>,
    ) -> Result<(GlobalHandle, usize), StorageAllocError> {
        match (old_layout.size(), new_layout.size()) {
            (0, 0) => Ok((GlobalHandle(old_alloc), 0)),
            (0, _) => self.allocate(new_layout),
            (_, 0) => {
                let new_alloc = self.allocate(new_layout)?;
                unsafe {
                    self.deallocate(old_layout, GlobalHandle(old_alloc));
                }
                Ok(new_alloc)
            }
            (old_size, new_size) => {
                if old_layout.align() >= new_layout.align() {
                    let ptr = NonNull::new(
                        unsafe {
                            alloc::alloc::realloc(old_alloc.as_ptr().cast(), old_layout, new_size)
                        }
                        .cast(),
                    )
                    .ok_or(StorageAllocError)?;
                    Ok((GlobalHandle(ptr), new_size))
                } else {
                    let (new_alloc, _) = self.allocate(new_layout)?;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            old_alloc.as_ptr().cast::<u8>(),
                            new_alloc.0.as_ptr().cast::<u8>(),
                            old_size,
                        );
                        self.deallocate(old_layout, GlobalHandle(old_alloc));
                    }
                    Ok((new_alloc, new_size))
                }
            }
        }
    }
}

unsafe impl MultipleStorage for Global {}
