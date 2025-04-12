use crate::{Storage, StorageAllocError, global_storage::Global};
use core::{
    alloc::Layout,
    marker::{PhantomData, Unsize},
    mem::ManuallyDrop,
    ops::{CoerceUnsized, Deref, DerefMut},
    ptr::{NonNull, Pointee},
};

pub struct Box<T: ?Sized, S: Storage = Global> {
    handle: S::Handle,
    storage: S,
    /// for storing metadata in a way that is compatible with [`CoerceUnsized`], this is an extra pointer but whatever :/
    metadata_ptr: NonNull<T>,
    _data: PhantomData<T>,
}

unsafe impl<T: Send + ?Sized, S: Storage> Send for Box<T, S> {}
unsafe impl<T: Sync + ?Sized, S: Storage> Sync for Box<T, S> {}

impl<T> Box<T> {
    pub fn new(value: T) -> Result<Self, StorageAllocError> {
        Self::new_in(value, Global)
    }
}

impl<T, S: Storage> Box<T, S> {
    pub fn new_in(value: T, storage: S) -> Result<Self, StorageAllocError> {
        let (handle, _) = storage.allocate(Layout::new::<T>())?;
        unsafe {
            storage.resolve(&handle).cast::<T>().write(value);
        }
        Ok(Self::from_raw_parts(storage, handle, ()))
    }

    pub fn into_inner(self) -> T {
        unsafe {
            let value = self.as_ptr().read();
            let (storage, handle, _) = Self::into_raw_parts(self);
            storage.deallocate(Layout::new::<T>(), handle);
            value
        }
    }
}

impl<T: ?Sized, S: Storage> Box<T, S> {
    pub fn from_raw_parts(
        storage: S,
        handle: S::Handle,
        metadata: <T as Pointee>::Metadata,
    ) -> Self {
        Self {
            handle,
            storage,
            metadata_ptr: NonNull::from_raw_parts(NonNull::<()>::dangling(), metadata),
            _data: PhantomData,
        }
    }

    pub fn into_raw_parts(self) -> (S, S::Handle, <T as Pointee>::Metadata) {
        unsafe {
            let this = ManuallyDrop::new(self);
            (
                core::ptr::read(&this.storage),
                core::ptr::read(&this.handle),
                core::ptr::metadata(this.metadata_ptr.as_ptr()),
            )
        }
    }

    pub fn as_ptr(&self) -> NonNull<T> {
        let ptr = unsafe { self.storage.resolve(&self.handle) };
        NonNull::from_raw_parts(ptr, core::ptr::metadata(self.metadata_ptr.as_ptr()))
    }
}

unsafe impl<#[may_dangle] T: ?Sized, S: Storage> Drop for Box<T, S> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.as_ptr().as_ptr());
            self.storage.deallocate(
                Layout::for_value_raw(self.metadata_ptr.as_ptr()),
                core::mem::replace(&mut self.handle, S::DANGLING),
            );
        }
    }
}

impl<T: ?Sized, S: Storage> Deref for Box<T, S> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.as_ptr().as_ref() }
    }
}

impl<T: ?Sized, S: Storage> DerefMut for Box<T, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.as_ptr().as_mut() }
    }
}

impl<T, U, S> CoerceUnsized<Box<U, S>> for Box<T, S>
where
    T: Unsize<U> + ?Sized,
    U: ?Sized,
    S: Storage,
{
}
