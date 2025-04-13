use crate::{Storage, StorageAllocError, global_storage::Global, storage_box::Box};
use core::{
    alloc::Layout,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

pub struct Vec<T, S: Storage = Global> {
    handle: ManuallyDrop<S::Handle>,
    length: usize,
    capacity: usize,
    storage: S,
    _data: PhantomData<[T]>,
}

impl<T> Vec<T> {
    pub fn new() -> Result<Self, StorageAllocError> {
        Self::new_in(Global)
    }
}

impl<T, S: Storage> Vec<T, S> {
    pub fn new_in(storage: S) -> Result<Self, StorageAllocError> {
        let (handle, capacity_in_bytes) =
            storage.allocate(unsafe { Layout::array::<T>(0).unwrap_unchecked() })?;
        Ok(Self {
            handle: ManuallyDrop::new(handle),
            length: 0,
            capacity: capacity_in_bytes
                .checked_div(size_of::<T>())
                .unwrap_or(usize::MAX),
            storage,
            _data: PhantomData,
        })
    }

    /// # Safety
    /// TODO
    pub unsafe fn from_raw_parts(
        storage: S,
        handle: S::Handle,
        length: usize,
        capacity: usize,
    ) -> Self {
        Self {
            handle: ManuallyDrop::new(handle),
            length,
            capacity,
            storage,
            _data: PhantomData,
        }
    }

    /// order is storage, handle, length, capacity
    pub fn into_raw_parts(self) -> (S, S::Handle, usize, usize) {
        unsafe {
            let mut this = ManuallyDrop::new(self);
            (
                core::ptr::read(&this.storage),
                ManuallyDrop::take(&mut this.handle),
                this.length,
                this.capacity,
            )
        }
    }

    pub fn reserve_exact(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        let new_capacity = self
            .length
            .checked_add(extra_capacity)
            .ok_or(StorageAllocError)?;

        if new_capacity < self.capacity {
            return Ok(());
        }

        let new_layout = Layout::array::<T>(new_capacity).map_err(|_| StorageAllocError)?;
        let (new_handle, capacity_in_bytes) = unsafe {
            self.storage.grow(
                Layout::array::<T>(self.capacity).unwrap_unchecked(),
                new_layout,
                &self.handle,
            )?
        };
        *self.handle = new_handle;
        self.capacity = capacity_in_bytes
            .checked_div(size_of::<T>())
            .unwrap_or(usize::MAX);

        Ok(())
    }

    pub fn reserve(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        let new_capacity = self
            .length
            .checked_add(extra_capacity)
            .ok_or(StorageAllocError)?;

        if new_capacity <= self.capacity {
            return Ok(());
        }

        if let Some(mut doubled_capacity) = self.capacity.checked_mul(2) {
            doubled_capacity = doubled_capacity.max(1);
            if doubled_capacity > new_capacity {
                if let Ok(()) = self.reserve_exact(doubled_capacity) {
                    return Ok(());
                }
            }
        }

        self.reserve_exact(extra_capacity)
    }

    pub fn shrink_to_fit(&mut self) -> Result<(), StorageAllocError> {
        if self.capacity == self.length {
            return Ok(());
        }

        let (new_handle, capacity_in_bytes) = unsafe {
            self.storage.shrink(
                Layout::array::<T>(self.capacity).unwrap_unchecked(),
                Layout::array::<T>(self.length).unwrap_unchecked(),
                &self.handle,
            )?
        };
        *self.handle = new_handle;
        self.capacity = capacity_in_bytes
            .checked_div(size_of::<T>())
            .unwrap_or(usize::MAX);

        Ok(())
    }

    pub fn into_boxed_slice(mut self) -> Result<Box<[T], S>, StorageAllocError> {
        unsafe {
            self.shrink_to_fit()?;
            let (storage, handle, length, _) = Self::into_raw_parts(self);
            Ok(Box::from_raw_parts(storage, handle, length))
        }
    }

    pub fn push(&mut self, value: T) -> Result<&mut T, PushError<T>> {
        if self.length == self.capacity {
            match self.reserve(1) {
                Ok(()) => {}
                Err(alloc_error) => return Err(PushError { value, alloc_error }),
            }
        }

        unsafe {
            let mut ptr = self
                .storage
                .resolve(&self.handle)
                .cast::<T>()
                .add(self.length);

            ptr.write(value);
            self.length += 1;
            Ok(ptr.as_mut())
        }
    }

    pub fn insert(&mut self, index: usize, value: T) -> Result<&mut T, InsertError<T>> {
        if index > self.length {
            return Err(InsertError {
                value,
                alloc_error: None,
            });
        }
        if self.length == self.capacity {
            match self.reserve(1) {
                Ok(()) => {}
                Err(error) => {
                    return Err(InsertError {
                        value,
                        alloc_error: Some(error),
                    });
                }
            }
        }

        unsafe {
            let mut ptr = self
                .storage
                .resolve(&self.handle)
                .cast::<T>()
                .add(self.length);
            ptr.copy_to(ptr.add(1), self.length - index);
            self.length += 1;
            ptr.write(value);
            Ok(ptr.as_mut())
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        unsafe {
            self.length -= 1;
            Some(
                self.storage
                    .resolve(&self.handle)
                    .cast::<T>()
                    .add(self.length)
                    .read(),
            )
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.length {
            return None;
        }

        unsafe {
            self.length -= 1;
            let ptr = self.storage.resolve(&self.handle).cast::<T>().add(index);
            let value = ptr.read();
            ptr.copy_from(ptr.add(1), self.length - index);
            Some(value)
        }
    }

    pub fn as_ptr(&self) -> NonNull<T> {
        unsafe { self.storage.resolve(&self.handle).cast() }
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe { NonNull::slice_from_raw_parts(self.as_ptr(), self.length).as_ref() }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { NonNull::slice_from_raw_parts(self.as_ptr(), self.length).as_mut() }
    }
}

impl<T, S: Storage> Deref for Vec<T, S> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, S: Storage> DerefMut for Vec<T, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, S: Storage> Drop for Vec<T, S> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.as_mut_slice());
            self.storage.deallocate(
                Layout::array::<T>(self.capacity).unwrap_unchecked(),
                ManuallyDrop::take(&mut self.handle),
            );
        }
    }
}

pub struct PushError<T> {
    pub value: T,
    pub alloc_error: StorageAllocError,
}

pub struct InsertError<T> {
    pub value: T,
    pub alloc_error: Option<StorageAllocError>,
}
