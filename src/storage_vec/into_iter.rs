use crate::{Storage, storage_vec::Vec};
use cfg_if::cfg_if;
use core::{
    alloc::Layout, iter::FusedIterator, marker::PhantomData, mem::ManuallyDrop, ptr::NonNull,
};

/// Owning iterator over a [`Vec`]
///
/// ```
/// use storage_api::Vec;
///
/// let mut v = Vec::<i32>::new().unwrap();
/// v.extend_from_slice(&[1, 2, 3]);
/// v.extend_from_slice(&[4, 5, 6]);
/// let mut count = v.len();
/// v.into_iter().zip([1, 2, 3, 4, 5, 6]).for_each(|(a, b)| {
///     assert_eq!(a, b);
///     count -= 1;
/// });
/// assert_eq!(count, 0);
/// ```
pub struct VecIntoIter<T, S: Storage> {
    handle: ManuallyDrop<S::Handle>,
    storage: S,
    start: usize,
    length: usize,
    capacity: usize,
    _data: PhantomData<[T]>,
}

impl<T, S: Storage> VecIntoIter<T, S> {
    pub(crate) fn new(vec: Vec<T, S>) -> Self {
        let (storage, handle, length, capacity) = vec.into_raw_parts();
        Self {
            handle: ManuallyDrop::new(handle),
            storage,
            start: 0,
            length,
            capacity,
            _data: PhantomData,
        }
    }

    /// Returns a slice referencing the remaining elements of this [`VecIntoIter`]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            NonNull::slice_from_raw_parts(
                self.storage
                    .resolve(&self.handle)
                    .cast::<T>()
                    .add(self.start),
                self.length,
            )
            .as_ref()
        }
    }

    /// Returns a mutable slice referencing the remaining elements of this [`VecIntoIter`]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            NonNull::slice_from_raw_parts(
                self.storage
                    .resolve(&self.handle)
                    .cast::<T>()
                    .add(self.start),
                self.length,
            )
            .as_mut()
        }
    }
}

impl<T, S: Storage> Iterator for VecIntoIter<T, S> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.length == 0 {
            return None;
        }

        unsafe {
            let value = self
                .storage
                .resolve(&self.handle)
                .cast::<T>()
                .add(self.start)
                .read();
            self.start += 1;
            self.length -= 1;
            Some(value)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.length, Some(self.length))
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.length
    }

    fn last(mut self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.next_back()
    }
}

impl<T, S: Storage> DoubleEndedIterator for VecIntoIter<T, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.length == 0 {
            return None;
        }

        unsafe {
            self.length -= 1;
            Some(
                self.storage
                    .resolve(&self.handle)
                    .cast::<T>()
                    .add(self.start + self.length)
                    .read(),
            )
        }
    }
}

impl<T, S: Storage> ExactSizeIterator for VecIntoIter<T, S> {}
impl<T, S: Storage> FusedIterator for VecIntoIter<T, S> {}

unsafe fn drop<T, S: Storage>(v: &mut VecIntoIter<T, S>) {
    unsafe {
        core::ptr::drop_in_place(v.as_mut_slice());
        v.storage.deallocate(
            Layout::array::<T>(v.capacity).unwrap_unchecked(),
            ManuallyDrop::take(&mut v.handle),
        );
    }
}

cfg_if! {
    if #[cfg(feature = "nightly")] {
        unsafe impl<#[may_dangle] T, S: Storage> Drop for VecIntoIter<T, S> {
            fn drop(&mut self) {
                unsafe { drop(self) }
            }
        }
    } else {
        impl<T, S: Storage> Drop for VecIntoIter<T, S> {
            fn drop(&mut self) {
                unsafe { drop(self) }
            }
        }
    }
}
