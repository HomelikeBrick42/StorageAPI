use crate::{
    Pointee, Storage, StorageAllocError, global_storage::Global, impl_maybe_unsized_methods,
};
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

impl_maybe_unsized_methods! {
    unsafe impl Send [for] Box
    where
        [
            T: Send,
            S: Send,
            S::Handle: Send,
        ] {}
}
impl_maybe_unsized_methods! {
    unsafe impl Sync [for] Box
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
            let value = Self::as_ptr(&self).read();
            let (storage, handle, _) = Self::into_raw_parts(self);
            storage.deallocate(Layout::new::<T>(), handle);
            value
        }
    }
}

impl_maybe_unsized_methods! {
    impl [for] Box {
        /// Reconstructs a [`Box`] from a [`Storage`], [`Storage::Handle`], and [`Pointee::Metadata`](core::ptr::Pointee::Metadata)
        ///
        /// The opposite of [`Box::into_raw_parts`]
        ///
        /// # Safety
        /// - `handle` must represent a valid allocation in `storage` of `size_of::<T>()` bytes that has a valid bitpattern for `T`
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
        pub fn as_ptr(b: &Self) -> NonNull<T> {
            let ptr = unsafe { b.storage.resolve(b.handle) };
            cfg_if! {
                if #[cfg(feature = "nightly")] {
                    NonNull::from_raw_parts(ptr, core::ptr::metadata(b.metadata_ptr.as_ptr()))
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
                    let ptr = Self::as_ptr(self);
                    let layout = Layout::for_value_raw(ptr.as_ptr());
                    ptr.drop_in_place();
                    self.storage
                        .deallocate(layout, self.handle);
                }
            }
        }
    } else {
        impl<T, S: Storage> Drop for Box<T, S> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = Self::as_ptr(self);
                    let layout = Layout::new::<T>();
                    ptr.drop_in_place();
                    self.storage
                        .deallocate(layout, self.handle);
                }
            }
        }
    }
}

impl_maybe_unsized_methods! {
    impl Deref [for] Box {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { Self::as_ptr(self).as_ref() }
        }
    }
}

impl_maybe_unsized_methods! {
    impl DerefMut [for] Box {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { Self::as_ptr(self).as_mut() }
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

#[cfg(feature = "nightly")]
impl<S: Storage> Box<dyn core::any::Any, S> {
    /// Attempts to downcast the [`dyn Any`](core::any::Any) to a `T`
    pub fn downcast<T: 'static>(b: Self) -> Result<Box<T, S>, Self> {
        if b.is::<T>() {
            Ok(unsafe { Self::downcast_unchecked(b) })
        } else {
            Err(b)
        }
    }

    /// Downcasts the [`dyn Any`](core::any::Any) to a `T`, without any checks
    ///
    /// The safe version of this function is [`Box::downcast`]
    ///
    /// # Safety
    /// The contained value must be of type `T`
    pub unsafe fn downcast_unchecked<T: 'static>(b: Self) -> Box<T, S> {
        debug_assert!(b.is::<T>());
        let (storage, handle, _) = Self::into_raw_parts(b);
        unsafe { Box::from_raw_parts(storage, handle, ()) }
    }
}

#[cfg(feature = "nightly")]
impl<Args, F, S> FnOnce<Args> for Box<F, S>
where
    Args: core::marker::Tuple,
    F: FnOnce<Args> + ?Sized,
    S: Storage,
{
    type Output = <F as FnOnce<Args>>::Output;

    extern "rust-call" fn call_once(self, args: Args) -> Self::Output {
        struct NoopAllocator;

        extern crate alloc;

        unsafe impl alloc::alloc::Allocator for NoopAllocator {
            fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
                Err(core::alloc::AllocError)
            }

            unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
                _ = ptr;
                _ = layout;
            }
        }

        let (storage, handle, metadata) = Self::into_raw_parts(self);
        unsafe {
            let ptr = core::ptr::from_raw_parts_mut(storage.resolve(handle).as_ptr(), metadata);
            let b = alloc::boxed::Box::<F, NoopAllocator>::from_raw_in(ptr, NoopAllocator);
            let output = b.call_once(args);
            storage.deallocate(Layout::for_value_raw(ptr), handle);
            output
        }
    }
}

#[cfg(feature = "nightly")]
impl<Args, F, S> FnMut<Args> for Box<F, S>
where
    Args: core::marker::Tuple,
    F: FnMut<Args> + ?Sized,
    S: Storage,
{
    extern "rust-call" fn call_mut(&mut self, args: Args) -> Self::Output {
        (**self).call_mut(args)
    }
}

#[cfg(feature = "nightly")]
impl<Args, F, S> Fn<Args> for Box<F, S>
where
    Args: core::marker::Tuple,
    F: Fn<Args> + ?Sized,
    S: Storage,
{
    extern "rust-call" fn call(&self, args: Args) -> Self::Output {
        (**self).call(args)
    }
}
