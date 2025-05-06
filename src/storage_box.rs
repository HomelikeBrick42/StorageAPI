use crate::{Storage, StorageAllocError, global_storage::Global};
use cfg_if::cfg_if;
use core::{
    alloc::Layout,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

cfg_if! {
    if #[cfg(feature = "nightly")] {
        /// A type that owns a single `T` allocated in a [`Storage`]
        ///
        /// This currently stores an extra dangling non-null pointer when using the `nightly` feature,
        /// so that [`CoerceUnsized`](core::ops::CoerceUnsized) can attach metadata to it when this [`Box`] get unsized
        ///
        /// [`Box`] does not support `T: ?Sized` types when not using the `nightly` feature
        pub struct Box<T: ?Sized, S: Storage = Global> {
            handle: S::Handle,
            storage: S,
            /// for storing metadata in a way that is compatible with [`CoerceUnsized`], this is an extra pointer but whatever :/
            metadata_ptr: NonNull<T>,
            _data: PhantomData<T>,
        }
    } else {
        /// A type that owns a single `T` allocated in a [`Storage`]
        pub struct Box<T, S: Storage = Global> {
            handle: S::Handle,
            storage: S,
            _data: PhantomData<T>,
        }
    }
}

cfg_if! {
    if #[cfg(feature = "nightly")] {
        macro_rules! impl_maybe_unsized_methods {
            (impl $($trait:path)? $(where [$($where:tt)*])? { $($tokens:tt)* }) => {
                impl<T: ?Sized, S: Storage> $($trait for )? Box<T, S> $(where $($where)*)? { $($tokens)* }
            };
            (unsafe impl $($trait:path)? $(where [$($where:tt)*])? { $($tokens:tt)* }) => {
                unsafe impl<T: ?Sized, S: Storage> $($trait for )? Box<T, S> $(where $($where)*)? { $($tokens)* }
            };
        }
    } else {
        macro_rules! impl_maybe_unsized_methods {
            (impl $($trait:path)? $(where [$($where:tt)*])? { $($tokens:tt)* }) => {
                impl<T, S: Storage> $($trait for )? Box<T, S> $(where $($where)*)? { $($tokens)* }
            };
            (unsafe impl $($trait:path)? $(where [$($where:tt)*])? { $($tokens:tt)* }) => {
                unsafe impl<T, S: Storage> $($trait for )? Box<T, S> $(where $($where)*)? { $($tokens)* }
            };
        }
    }
}

impl_maybe_unsized_methods! {
    unsafe impl Send
    where
        [
            T: Send,
            S: Send,
            S::Handle: Send,
        ] {}
}
impl_maybe_unsized_methods! {
    unsafe impl Sync
    where
        [
            T: Sync,
            S: Sync,
            S::Handle: Sync,
        ] {}
}

impl<T, S: Storage + Default> Box<T, S> {
    /// [`Box::new_in`] but using [`Default::default`] for the [`Storage`]
    pub fn new(value: T) -> Result<Self, StorageAllocError> {
        Self::new_in(value, Default::default())
    }

    /// [`Box::new_with_in`] but using [`Default::default`] for the [`Storage`]
    ///
    /// This function has an advantage over [`Box::new`] for large objects where because the allocation is done *before* `f` is called,
    /// the stack space for the return value of `f` may be elided by the compiler
    pub fn new_with(f: impl FnOnce() -> T) -> Result<Self, StorageAllocError> {
        Self::new_with_in(f, Default::default())
    }
}

impl<T, S: Storage> Box<T, S> {
    /// Allocates room for a `T` in `storage` and moves `value` into it
    pub fn new_in(value: T, storage: S) -> Result<Self, StorageAllocError> {
        Self::new_with_in(|| value, storage)
    }

    /// Allocates room for a `T` in `storage` and constructs `value` into it
    ///
    /// This function has an advantage over [`Box::new_in`] for large objects where because the allocation is done *before* `f` is called,
    /// the stack space for the return value of `f` may be elided by the compiler
    pub fn new_with_in(f: impl FnOnce() -> T, storage: S) -> Result<Self, StorageAllocError> {
        let (handle, _) = storage.allocate(Layout::new::<T>())?;
        unsafe {
            storage.resolve(handle).cast::<T>().write(f());
            Ok(Self::from_raw_parts(storage, handle, ()))
        }
    }

    /// Moves the `T` out of this [`Box`]
    pub fn into_inner(self) -> T {
        unsafe {
            let value = self.as_ptr().read();
            let (storage, handle, _) = Self::into_raw_parts(self);
            storage.deallocate(Layout::new::<T>(), handle);
            value
        }
    }
}

#[doc(hidden)]
pub trait Pointee {
    type Metadata;
}

impl<T: ?Sized> Pointee for T {
    cfg_if! {
        if #[cfg(feature = "nightly")] {
            type Metadata = <T as core::ptr::Pointee>::Metadata;
        } else {
            type Metadata = ();
        }
    }
}

impl_maybe_unsized_methods! {
    impl {
        /// Reconstructs a [`Box`] from a [`Storage`], [`Storage::Handle`], and [`Pointee::Metadata`](core::ptr::Pointee::Metadata)
        ///
        /// The opposite of [`Box::into_raw_parts`]
        ///
        /// # Safety
        /// - `handle` must represent a valid allocation in `storage` of `size_of::<T>()` bytes
        /// - `metadata` must be a valid pointer metadata for the `T` that `handle` represents
        pub unsafe fn from_raw_parts(
            storage: S,
            handle: S::Handle,
            #[allow(unused)]
            metadata: <T as Pointee>::Metadata,
        ) -> Self {
            Self {
                handle,
                storage,
                #[cfg(feature = "nightly")]
                metadata_ptr: NonNull::from_raw_parts(NonNull::<()>::dangling(), metadata),
                _data: PhantomData,
            }
        }

        /// Splits the [`Box`] into its [`Storage`], [`Storage::Handle`], and [`Pointee::Metadata`](core::ptr::Pointee::Metadata)
        ///
        /// The opposite of [`Box::from_raw_parts`]
        pub fn into_raw_parts(b: Self) -> (S, S::Handle, <T as Pointee>::Metadata) {
            unsafe {
                let this = ManuallyDrop::new(b);
                (
                    core::ptr::read(&this.storage),
                    this.handle,
                    {
                        #[cfg(feature = "nightly")]
                        core::ptr::metadata(this.metadata_ptr.as_ptr())
                    },
                )
            }
        }

        /// Gets a [`NonNull<T>`] to the `T` stored in this [`Box`]
        pub fn as_ptr(&self) -> NonNull<T> {
            let ptr = unsafe { self.storage.resolve(self.handle) };
            cfg_if! {
                if #[cfg(feature = "nightly")] {
                NonNull::from_raw_parts(ptr, core::ptr::metadata(self.metadata_ptr.as_ptr()))
                } else {
                    ptr.cast()
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "nightly")] {
        unsafe impl<#[may_dangle] T: ?Sized, S: Storage> Drop for Box<T, S> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = self.as_ptr();
                    let layout = Layout::for_value_raw(ptr.as_ptr());
                    ptr.drop_in_place();
                    self.storage
                        .deallocate(layout,  self.handle);
                }
            }
        }
    } else {
        impl<T, S: Storage> Drop for Box<T, S> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = self.as_ptr();
                    let layout = Layout::new::<T>();
                    ptr.drop_in_place();
                    self.storage
                        .deallocate(layout, ManuallyDrop::take(&mut self.handle));
                }
            }
        }
    }
}

impl_maybe_unsized_methods! {
    impl Deref {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { self.as_ptr().as_ref() }
        }
    }
}

impl_maybe_unsized_methods! {
    impl DerefMut {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { self.as_ptr().as_mut() }
        }
    }
}

#[cfg(feature = "nightly")]
impl<T, U, S> core::ops::CoerceUnsized<Box<U, S>> for Box<T, S>
where
    T: core::marker::Unsize<U> + ?Sized,
    U: ?Sized,
    S: Storage,
{
}
