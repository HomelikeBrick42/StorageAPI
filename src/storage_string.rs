use crate::{
    Storage, StorageAllocError, global_storage::Global, storage_box::Box, storage_vec::Vec,
};
use core::{
    ops::{Deref, DerefMut},
    str::FromStr,
};

/// A wrapper around [`Vec<u8, S>`] that is guarenteed to be valid UTF-8 so it can be referenced as a [`str`]
pub struct String<S: Storage = Global> {
    vec: Vec<u8, S>,
}

impl String {
    /// [`String::new_in`] but with the [`Global`] storage
    ///
    /// This is the same as [`String::with_capacity(0)`](String::with_capacity)
    pub fn new() -> Result<Self, StorageAllocError> {
        Self::new_in(Global)
    }

    /// [`String::with_capacity_in`] but with the [`Global`] storage
    pub fn with_capacity(capacity: usize) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(capacity, Global)
    }
}

impl<S: Storage> String<S> {
    /// Constructs a new [`String`] allocated in `storage`
    ///
    /// This is the same as calling [`String::with_capacity_in(0, storage)`](String::with_capacity_in)
    pub fn new_in(storage: S) -> Result<Self, StorageAllocError> {
        Self::with_capacity_in(0, storage)
    }

    /// Constructs a [`String`] with room for at least `capacity` elements allocated in `storage`
    ///
    /// Calling [`String::capacity`] on the result of this method may return a greater value than the provided `capacity`,
    /// this is because the [`Storage`] may provide more space than was requested
    pub fn with_capacity_in(capacity: usize, storage: S) -> Result<Self, StorageAllocError> {
        Ok(String {
            vec: Vec::with_capacity_in(capacity, storage)?,
        })
    }

    /// Constructs a [`String`] with the contents of `s`
    pub fn from_str_in(s: &str, storage: S) -> Result<Self, StorageAllocError> {
        let mut string = Self::with_capacity_in(s.len(), storage)?;
        string.push_str(s)?;
        Ok(string)
    }

    /// Returns the total number of bytes that this [`String`] can hold before it reallocates
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }

    /// Reconstructs a [`String`] from a [`Storage`], [`Storage::Handle`], length, and capacity
    ///
    /// The opposite of [`String::into_raw_parts`]
    ///
    /// # Safety
    /// - `handle` must represent a valid allocation in `storage` and
    ///     - have an allocated size of `capacity` bytes
    ///     - have `length` initialised bytes that are valid UTF-8
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

    /// Splits the [`String`] into its [`Storage`], [`Storage::Handle`], length, and capacity
    ///
    /// The opposite of [`String::from_raw_parts`]
    pub fn into_raw_parts(self) -> (S, S::Handle, usize, usize) {
        self.vec.into_raw_parts()
    }

    /// See [`Vec<u8, S>::reserve_exact`]
    pub fn reserve_exact(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        self.vec.reserve_exact(extra_capacity)
    }

    /// See [`Vec<u8, S>::reserve`]
    pub fn reserve(&mut self, extra_capacity: usize) -> Result<(), StorageAllocError> {
        self.vec.reserve(extra_capacity)
    }

    /// See [`Vec<u8, S>::shrink_to_fit`]
    pub fn shrink_to_fit(&mut self) -> Result<(), StorageAllocError> {
        self.vec.shrink_to_fit()
    }

    /// Converts a [`String<S>`] to [`Box<str, S>`](Box), discarding any extra capacity
    pub fn into_boxed_str(self) -> Result<Box<str, S>, StorageAllocError> {
        unsafe {
            let (storage, handle, length) = Box::into_raw_parts(self.vec.into_boxed_slice()?);
            Ok(Box::from_raw_parts(storage, handle, length))
        }
    }

    /// Encodes `c` as UTF-8, and then pushes its bytes onto the end of the [`String`]
    ///
    /// ```
    /// use storage_api::{String, InlineStorage};
    /// # use storage_api::StorageAllocError;
    ///
    /// let storage = InlineStorage::<[u8; 2]>::new(); // a storage with room for 2 bytes
    /// let mut s = String::from_str_in("a", storage).unwrap();
    /// assert_eq!(s.push('b').as_deref(), Ok("b"));
    /// assert_eq!(s.push('c'),            Err(StorageAllocError)); // out of room
    /// assert_eq!(&*s, "ab");
    /// ```
    pub fn push(&mut self, c: char) -> Result<&mut str, StorageAllocError> {
        self.push_str(c.encode_utf8(&mut [0; 4]))
    }

    /// Pushes the bytes of `s` onto the end of the [`String`]
    /// ```
    /// use storage_api::{String, InlineStorage};
    /// # use storage_api::StorageAllocError;
    ///
    /// let storage = InlineStorage::<[u8; 12]>::new(); // a storage with room for 12 bytes
    /// let mut s = String::from_str_in("Hello", storage).unwrap();
    /// assert_eq!(s.push_str(", World").as_deref(), Ok(", World"));
    /// assert_eq!(s.push_str("!"),                  Err(StorageAllocError)); // out of room
    /// assert_eq!(&*s, "Hello, World");
    /// ```
    pub fn push_str(&mut self, s: &str) -> Result<&mut str, StorageAllocError> {
        unsafe {
            Ok(str::from_utf8_unchecked_mut(
                self.vec.extend_from_slice(s.as_bytes())?,
            ))
        }
    }

    /// Returns a string slice referencing this [`String`]
    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.vec) }
    }

    /// Returns a mutable string slice referencing this [`String`]
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
