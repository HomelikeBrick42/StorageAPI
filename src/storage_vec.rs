use crate::{Storage, StorageAllocError, global_storage::Global, storage_box::Box};
use core::{
    alloc::Layout,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// A collection for managing a list of elements
pub struct Vec<T, S: Storage = Global> {
    handle: ManuallyDrop<S::Handle>,
    length: usize,
    capacity: usize,
    storage: S,
    _data: PhantomData<[T]>,
}

impl<T> Vec<T> {
    /// [`Vec::new_in`] but with the [`Global`] allocator
    ///
    /// This is the same as [`Vec::with_capacity(0)`](Vec::with_capacity)
    pub fn new() -> Result<Self, StorageAllocError> {
        Self::new_in(Global)
    }

    /// [`Vec::with_capacity_in`] but with the [`Global`] allocator
    pub fn with_capacity(capacity: usize) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(capacity, Global)
    }
}

impl<T, S: Storage> Vec<T, S> {
    /// Constructs a new [`Vec`] allocated in `storage`
    ///
    /// This is the same as calling [`Vec::with_capacity_in(0, storage)`](Vec::with_capacity_in)
    pub fn new_in(storage: S) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(0, storage)
    }

    /// Constructs a [`Vec`] with room for at least `capacity` elements allocated in `storage`
    ///
    /// Calling [`Vec::capacity`] on the result of this method may return a greater value than the provided `capacity`,
    /// this is because the [`Storage`] may provide more space than was requested
    pub fn with_capacity_in(capacity: usize, storage: S) -> Result<Self, StorageAllocError> {
        let (handle, capacity_in_bytes) =
            storage.allocate(Layout::array::<T>(capacity).map_err(|_| StorageAllocError)?)?;
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

    /// Returns the total number of elements that this [`Vec`] can hold before it reallocates
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Reconstructs a [`Vec`] from a [`Storage`], [`Storage::Handle`], length, and capacity
    ///
    /// The opposite of [`Vec::into_raw_parts`]
    ///
    /// # Safety
    /// - `handle` must represent a valid allocation in `storage` and
    ///     - have an allocated size of `capacity * size_of::<T>()` bytes
    ///     - have `length` initialised elements
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

    /// Splits the [`Vec`] into its [`Storage`], [`Storage::Handle`], length, and capacity
    ///
    /// The opposite of [`Vec::from_raw_parts`]
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

    /// Makes room for at least `extra_capacity` elements, without using a growth factor
    ///
    /// Capacity may still be greater than the current length after this function returns successfully, just like with [`Vec::with_capacity`] the [`Storage`] may return more space than what is requested
    ///
    /// This method is only recomended if you dont plan on pushing more elements later, if you are going to push more elements,
    /// then [`Vec::reserve`] is better because it preserves the growth factor
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

    /// Makes room for at least `extra_capacity` elements, using a growth factor
    ///
    /// To reserve space without a growth factor, see [`Vec::reserve_exact`]
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

    /// Attempts to shrink the allocated capacity to the current length
    ///
    /// Capacity may still be greater than the current length after this function returns successfully, just like with [`Vec::with_capacity`] the [`Storage`] may return more space than what is requested
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

    /// Converts a [`Vec<T, S>`] to [`Box<[T], S>`](Box), discarding any extra capacity
    pub fn into_boxed_slice(mut self) -> Result<Box<[T], S>, StorageAllocError> {
        unsafe {
            self.shrink_to_fit()?;
            let (storage, handle, length, _) = Self::into_raw_parts(self);
            Ok(Box::from_raw_parts(storage, handle, length))
        }
    }

    /// Adds an element to the end of a [`Vec`]
    /// ```
    /// use storage_api::{Vec, InlineStorage};
    /// # use storage_api::{StorageAllocError, collections::PushError};
    ///
    /// let storage = InlineStorage::<[i32; 2]>::new(); // a storage with room for 2 `i32`s
    /// let mut v = Vec::new_in(storage).unwrap();
    /// assert_eq!(v.push(1), Ok(&mut 1));
    /// assert_eq!(v.push(2), Ok(&mut 2));
    /// assert_eq!(v.push(3), Err(PushError { value: 3, alloc_error: StorageAllocError })); // this will fail because there is not enough room
    /// assert_eq!(&*v, &[1, 2]);
    /// ```
    pub fn push(&mut self, value: T) -> Result<&mut T, PushError<T>> {
        match self.reserve(1) {
            Ok(()) => {}
            Err(alloc_error) => return Err(PushError { value, alloc_error }),
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

    /// Inserts an element at `index` in the [`Vec`]
    /// ```
    /// use storage_api::{Vec, InlineStorage};
    /// # use storage_api::{StorageAllocError, collections::InsertError};
    ///
    /// let storage = InlineStorage::<[i32; 3]>::new(); // a storage with room for 3 `i32`s
    /// let mut v = Vec::new_in(storage).unwrap();
    /// assert_eq!(v.insert(1, 1), Err(InsertError { value: 1, alloc_error: None })); // this will fail because `index` is out of range
    /// assert_eq!(v.insert(0, 2), Ok(&mut 2)); // inserting at the "end" works
    /// assert_eq!(v.insert(1, 3), Ok(&mut 3));
    /// assert_eq!(v.insert(1, 4), Ok(&mut 4));
    /// assert_eq!(v.insert(1, 5), Err(InsertError { value: 5, alloc_error: Some(StorageAllocError) })); // this will fail because there is not enough room
    /// assert_eq!(&*v, &[2, 4, 3]);
    /// ```
    pub fn insert(&mut self, index: usize, value: T) -> Result<&mut T, InsertError<T>> {
        if index > self.length {
            return Err(InsertError {
                value,
                alloc_error: None,
            });
        }
        match self.reserve(1) {
            Ok(()) => {}
            Err(error) => {
                return Err(InsertError {
                    value,
                    alloc_error: Some(error),
                });
            }
        }

        unsafe {
            let mut ptr = self.storage.resolve(&self.handle).cast::<T>().add(index);
            ptr.copy_to(ptr.add(1), self.length - index);
            self.length += 1;
            ptr.write(value);
            Ok(ptr.as_mut())
        }
    }

    /// Removes the last element from the [`Vec`], returning [`None`] if the [`Vec`] is empty
    ///
    /// ```
    /// use storage_api::{Vec, InlineStorage};
    ///
    /// let storage = InlineStorage::<[i32; 3]>::new(); // a storage with room for 3 `i32`s
    /// let mut v = Vec::new_in(storage).unwrap();
    /// v.extend_from_slice(&[1, 2, 3]).unwrap();
    /// assert_eq!(v.pop(), Some(3));
    /// assert_eq!(v.pop(), Some(2));
    /// v.push(4).unwrap();
    /// assert_eq!(v.pop(), Some(4));
    /// assert_eq!(v.pop(), Some(1));
    /// assert_eq!(v.pop(), None); // its empty
    /// assert_eq!(&*v, &[]);
    /// ```
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

    /// Removes an element from the [`Vec`], returning [`None`] if the `index` is out of range
    ///
    /// ```
    /// use storage_api::{Vec, InlineStorage};
    ///
    /// let storage = InlineStorage::<[i32; 3]>::new(); // a storage with room for 3 `i32`s
    /// let mut v = Vec::new_in(storage).unwrap();
    /// v.extend_from_slice(&[1, 2, 3]).unwrap();
    /// assert_eq!(v.remove(3), None); // out of range
    /// assert_eq!(v.remove(1), Some(2));
    /// assert_eq!(v.remove(0), Some(1));
    /// assert_eq!(v.remove(0), Some(3));
    /// assert_eq!(v.remove(0), None); // empty
    /// assert_eq!(&*v, &[]);
    /// ```
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

    /// Returns a slice referencing the initialised elements of this [`Vec`]
    pub fn as_slice(&self) -> &[T] {
        unsafe { NonNull::from_raw_parts(self.storage.resolve(&self.handle), self.length).as_ref() }
    }

    /// Returns a mutable slice referencing the initialised elements of this [`Vec`]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { NonNull::from_raw_parts(self.storage.resolve(&self.handle), self.length).as_mut() }
    }
}

impl<T: Copy, S: Storage> Vec<T, S> {
    /// Appends the elements of a slice to the end of the [`Vec`]
    ///
    /// ```
    /// use storage_api::{Vec, InlineStorage};
    /// # use storage_api::StorageAllocError;
    ///
    /// let storage = InlineStorage::<[i32; 3]>::new(); // a storage with room for 3 `i32`s
    /// let mut v = Vec::new_in(storage).unwrap();
    /// assert_eq!(v.extend_from_slice(&[1, 2]), Ok(&mut [1, 2] as _));
    /// v.remove(1).unwrap();
    /// assert_eq!(v.extend_from_slice(&[3, 4]), Ok(&mut [3, 4] as _));
    /// assert_eq!(v.extend_from_slice(&[5]), Err(StorageAllocError)); // not enough room
    /// assert_eq!(&*v, &[1, 3, 4]);
    /// ```
    pub fn extend_from_slice(&mut self, values: &[T]) -> Result<&mut [T], StorageAllocError> {
        let index = self.length;
        let length = values.len();
        self.reserve(length)?;
        unsafe {
            let ptr = self.storage.resolve(&self.handle).cast::<T>().add(index);
            ptr.as_ptr().copy_from(values.as_ptr(), length);
            self.length += length;
            Ok(NonNull::slice_from_raw_parts(ptr, length).as_mut())
        }
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

unsafe impl<#[may_dangle] T, S: Storage> Drop for Vec<T, S> {
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

/// The error returned by [`Vec::push`]
#[derive(Debug, PartialEq, Eq)]
pub struct PushError<T> {
    /// The value that was attempted to be pushed
    pub value: T,
    /// The allocation error
    pub alloc_error: StorageAllocError,
}

/// The error returned by [`Vec::insert`]
#[derive(Debug, PartialEq, Eq)]
pub struct InsertError<T> {
    /// The value that was attempted to be inserted
    pub value: T,
    /// this is [`None`] if the index to insert was out of range, otherwise its [`Some`] with the allocation error
    pub alloc_error: Option<StorageAllocError>,
}
