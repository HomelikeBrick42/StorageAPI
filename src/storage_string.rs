use crate::{
    Storage, StorageAllocError, global_storage::Global, storage_box::Box, storage_vec::Vec,
};
use core::{
    ops::{Deref, DerefMut},
    str::FromStr,
};

pub struct String<S: Storage = Global> {
    vec: Vec<u8, S>,
}

impl String {
    pub fn new() -> Result<Self, StorageAllocError> {
        Self::new_in(Global)
    }

    pub fn with_capacity(capacity: usize) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(capacity, Global)
    }
}

impl<S: Storage> String<S> {
    pub fn new_in(storage: S) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(0, storage)
    }

    pub fn with_capacity_in(capacity: usize, storage: S) -> Result<Self, StorageAllocError> {
        Ok(String {
            vec: Vec::with_capacity_in(capacity, storage)?,
        })
    }

    pub fn from_str_in(s: &str, storage: S) -> Result<Self, StorageAllocError> {
        let mut string = Self::with_capacity_in(s.len(), storage)?;
        string.push_str(s)?;
        Ok(string)
    }

    /// # Safety
    /// TODO
    pub unsafe fn from_raw_parts(
        storage: S,
        handle: S::Handle,
        length: usize,
        capacity: usize,
    ) -> Self {
        String {
            vec: unsafe { Vec::from_raw_parts(storage, handle, length, capacity) },
        }
    }

    /// order is storage, handle, length, capacity
    pub fn into_raw_parts(self) -> (S, S::Handle, usize, usize) {
        self.vec.into_raw_parts()
    }

    pub fn reserve_exact(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        self.vec.reserve_exact(extra_capacity)
    }

    pub fn reserve(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        self.vec.reserve(extra_capacity)
    }

    pub fn shrink_to_fit(&mut self) -> Result<(), StorageAllocError> {
        self.vec.shrink_to_fit()
    }

    pub fn into_boxed_str(self) -> Result<Box<str, S>, StorageAllocError> {
        unsafe {
            let (storage, handle, length) = Box::into_raw_parts(self.vec.into_boxed_slice()?);
            Ok(Box::from_raw_parts(storage, handle, length))
        }
    }

    pub fn push(&mut self, c: char) -> Result<&mut str, StorageAllocError> {
        self.push_str(c.encode_utf8(&mut [0; 4]))
    }

    pub fn push_str(&mut self, s: &str) -> Result<&mut str, StorageAllocError> {
        unsafe {
            Ok(str::from_utf8_unchecked_mut(
                self.vec.extend_from_slice(s.as_bytes())?,
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.vec) }
    }

    pub fn as_mut_str(&mut self) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(&mut self.vec) }
    }
}

impl<S: Storage> Deref for String<S> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<S: Storage> DerefMut for String<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_str()
    }
}

impl FromStr for String {
    type Err = StorageAllocError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_in(s, Global)
    }
}
