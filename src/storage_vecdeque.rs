use crate::{Global, Storage, StorageAllocError, storage_vec::PushError};
use cfg_if::cfg_if;
use core::{alloc::Layout, marker::PhantomData, mem::ManuallyDrop};

/// A double-ended ring-buffer queue
pub struct VecDeque<T, S: Storage = Global> {
    handle: S::Handle,
    head: usize,
    length: usize,
    capacity: usize,
    storage: S,
    _data: PhantomData<[T]>,
}

impl<T, S: Storage + Default> VecDeque<T, S> {
    /// [`VecDeque::new_in`] but using [`Default::default`] for the allocator
    ///
    /// This is the same as [`VecDeque::with_capacity(0)`](VecDeque::with_capacity)
    pub fn new() -> Result<Self, StorageAllocError> {
        Self::new_in(Default::default())
    }

    /// [`VecDeque::with_capacity_in`] but  using [`Default::default`] for the allocator
    pub fn with_capacity(capacity: usize) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(capacity, Default::default())
    }
}

impl<T, S: Storage> VecDeque<T, S> {
    /// Constructs a new [`VecDeque`] allocated in `storage`
    ///
    /// This is the same as calling [`VecDeque::with_capacity_in(0, storage)`](VecDeque::with_capacity_in)
    pub fn new_in(storage: S) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(0, storage)
    }

    /// Constructs a [`VecDeque`] with room for at least `capacity` elements allocated in `storage`
    ///
    /// Calling [`VecDeque::capacity`] on the result of this method may return a greater value than the provided `capacity`,
    /// this is because the [`Storage`] may provide more space than was requested
    pub fn with_capacity_in(capacity: usize, storage: S) -> Result<Self, StorageAllocError> {
        let (handle, capacity_in_bytes) =
            storage.allocate(Layout::array::<T>(capacity).map_err(|_| StorageAllocError)?)?;
        Ok(Self {
            handle,
            head: 0,
            length: 0,
            capacity: capacity_in_bytes
                .checked_div(size_of::<T>())
                .unwrap_or(usize::MAX),
            storage,
            _data: PhantomData,
        })
    }

    /// Returns the total number of elements that this [`VecDeque`] can hold before it reallocates
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the total number of elements in this [`VecDeque`]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns whether this [`VecDeque`] is empty
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Reconstructs a [`VecDeque`] from a [`Storage`], [`Storage::Handle`], head, length, and capacity
    ///
    /// The opposite of [`VecDeque::into_raw_parts`]
    ///
    /// # Safety
    /// - `handle` must represent a valid allocation in `storage` and
    ///     - have an allocated size of `capacity * size_of::<T>()` bytes
    ///     - TODO: specify start and end
    pub unsafe fn from_raw_parts(
        storage: S,
        handle: S::Handle,
        head: usize,
        length: usize,
        capacity: usize,
    ) -> Self {
        Self {
            handle,
            head,
            length,
            capacity,
            storage,
            _data: PhantomData,
        }
    }

    /// Splits the [`VecDeque`] into its [`Storage`], [`Storage::Handle`], head, length, and capacity
    ///
    /// The opposite of [`VecDeque::from_raw_parts`]
    pub fn into_raw_parts(self) -> (S, S::Handle, usize, usize, usize) {
        unsafe {
            let this = ManuallyDrop::new(self);
            (
                core::ptr::read(&this.storage),
                this.handle,
                this.head,
                this.length,
                this.capacity,
            )
        }
    }

    /// Returns whether this [`VecDeque`] is contiguous
    pub fn is_contiguous(&self) -> bool {
        self.head <= self.capacity - self.length
    }

    /// Rearanges the internal storage so that all the elements are in a single slice
    ///
    /// After calling this method, [`VecDeque::as_slices`] and [`VecDeque::as_mut_slices`] will return all the elements in the first slice
    pub fn make_contiguous(&mut self) -> &mut [T] {
        unsafe {
            let ptr = self.storage.resolve(self.handle).cast::<T>().as_ptr();

            if !self.is_contiguous() {
                todo!()
            }

            core::slice::from_raw_parts_mut(ptr.add(self.head), self.length)
        }
    }

    /// Returns a pair of slices which contain the elements of the slice in order
    ///
    /// If [`VecDeque::make_contiguous`] has been called then all the elements will be in the first slice
    pub fn as_slices(&self) -> (&[T], &[T]) {
        unsafe {
            let ptr = self.storage.resolve(self.handle).cast::<T>().as_ptr();
            let first_length = self.length.min(self.capacity - self.head);
            (
                core::slice::from_raw_parts(ptr.add(self.head), first_length),
                core::slice::from_raw_parts(ptr, self.length - first_length),
            )
        }
    }

    /// Returns a pair of slices which contain the elements of the slice in order
    ///
    /// If [`VecDeque::make_contiguous`] has been called then all the elements will be in the first slice
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        unsafe {
            let ptr = self.storage.resolve(self.handle).cast::<T>().as_ptr();
            let first_length = self.length.min(self.capacity - self.head);
            (
                core::slice::from_raw_parts_mut(ptr.add(self.head), first_length),
                core::slice::from_raw_parts_mut(ptr, self.length - first_length),
            )
        }
    }

    /// Makes room for at least `extra_capacity` elements, without using a growth factor
    ///
    /// Capacity may still be greater than the current length after this function returns successfully, just like with [`VecDeque::with_capacity`] the [`Storage`] may return more space than what is requested
    ///
    /// This method is only recomended if you dont plan on pushing more elements later, if you are going to push more elements,
    /// then [`VecDeque::reserve`] is better because it preserves the growth factor
    pub fn reserve_exact(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        let new_capacity = self
            .length
            .checked_add(extra_capacity)
            .ok_or(StorageAllocError)?;

        if new_capacity < self.capacity {
            return Ok(());
        }

        let was_contiguous = self.is_contiguous();
        let old_capacity = self.capacity;

        let new_layout = Layout::array::<T>(new_capacity).map_err(|_| StorageAllocError)?;
        let capacity_in_bytes;
        (self.handle, capacity_in_bytes) = unsafe {
            self.storage.grow(
                Layout::array::<T>(self.capacity).unwrap_unchecked(),
                new_layout,
                self.handle,
            )?
        };
        self.capacity = capacity_in_bytes
            .checked_div(size_of::<T>())
            .unwrap_or(usize::MAX);

        if !was_contiguous {
            unsafe {
                let ptr = self.storage.resolve(self.handle).cast::<T>().as_ptr();

                let new_head = self
                    .head
                    .wrapping_sub(old_capacity)
                    .wrapping_add(self.capacity);
                core::ptr::copy(
                    ptr.add(self.head),
                    ptr.add(new_head),
                    old_capacity - self.head,
                );
                self.head = new_head;
            }
        }

        Ok(())
    }

    /// Makes room for at least `extra_capacity` elements, using a growth factor
    ///
    /// To reserve space without a growth factor, see [`VecDeque::reserve_exact`]
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
}

impl<T, S: Storage> VecDeque<T, S> {
    /// Adds a value to the end of the [`VecDeque`]
    pub fn push_back(&mut self, value: T) -> Result<&mut T, PushError<T>> {
        match self.reserve(1) {
            Ok(()) => {}
            Err(alloc_error) => return Err(PushError { value, alloc_error }),
        }

        _ = value;
        todo!()
    }

    /// Adds a value to the start of the [`VecDeque`]
    pub fn push_front(&mut self, value: T) -> Result<&mut T, PushError<T>> {
        match self.reserve(1) {
            Ok(()) => {}
            Err(alloc_error) => return Err(PushError { value, alloc_error }),
        }

        _ = value;
        todo!()
    }
}

unsafe fn drop<T, S: Storage>(v: &mut VecDeque<T, S>) {
    unsafe {
        let (a, b) = v.as_mut_slices();
        core::ptr::drop_in_place(a);
        core::ptr::drop_in_place(b);
        v.storage
            .deallocate(Layout::array::<T>(v.capacity).unwrap_unchecked(), v.handle);
    }
}

cfg_if! {
    if #[cfg(feature = "nightly")] {
        unsafe impl<#[may_dangle] T, S: Storage> Drop for VecDeque<T, S> {
            fn drop(&mut self) {
                unsafe { drop(self) }
            }
        }
    } else {
        impl<T, S: Storage> Drop for VecDeque<T, S> {
            fn drop(&mut self) {
                unsafe { drop(self) }
            }
        }
    }
}
