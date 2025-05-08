use crate::{
    Box, Global, Pointee, ShareableStorage, Storage, StorageAllocError, impl_maybe_unsized_methods,
};
use cfg_if::cfg_if;
use core::{
    alloc::Layout,
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

cfg_if! {
    if #[cfg(feature = "nightly")] {
        /// A type that owns a shared `T` allocated in a [`Storage`]
        ///
        /// This currently stores an extra dangling non-null pointer when using the `nightly` feature,
        /// so that [`CoerceUnsized`](core::ops::CoerceUnsized) can attach metadata to it when this [`Rc`] get unsized
        ///
        /// [`Rc`] does not support `T: ?Sized` types when not using the `nightly` feature
        pub struct Rc<T: ?Sized, S: Storage = Global> {
            handle: S::Handle,
            storage: S,
            /// for storing metadata in a way that is compatible with [`CoerceUnsized`], this is an extra pointer but whatever :/
            metadata_ptr: NonNull<T>,
            _data: PhantomData<T>,
        }
    } else {
        /// A type that owns a shared `T` allocated in a [`Storage`]
        pub struct Rc<T, S: Storage = Global> {
            handle: S::Handle,
            storage: S,
            _data: PhantomData<T>,
        }
    }
}

struct RcInner<T: ?Sized> {
    strong: Cell<usize>,
    data: UnsafeCell<ManuallyDrop<T>>,
}

impl<T, S: Storage + Default> Rc<T, S> {
    /// [`BoxRc::new_in`] but using [`Default::default`] for the [`Storage`]
    pub fn new(value: T) -> Result<Self, StorageAllocError> {
        Self::new_in(value, Default::default())
    }

    /// [`Rc::new_with_in`] but using [`Default::default`] for the [`Storage`]
    ///
    /// This function has an advantage over [`Rc::new`] for large objects where because the allocation is done *before* `f` is called,
    /// the stack space for the return value of `f` may be elided by the compiler
    pub fn new_with(f: impl FnOnce() -> T) -> Result<Self, StorageAllocError> {
        Self::new_with_in(f, Default::default())
    }
}

impl<T, S: Storage> Rc<T, S> {
    /// Allocates room for a `T` in `storage` and moves `value` into it
    pub fn new_in(value: T, storage: S) -> Result<Self, StorageAllocError> {
        let (storage, handle, metadata) = Box::into_raw_parts(Box::new_in(
            RcInner {
                strong: Cell::new(0),
                data: UnsafeCell::new(ManuallyDrop::new(value)),
            },
            storage,
        )?);
        Ok(unsafe { Self::from_raw_parts(storage, handle, metadata) })
    }

    /// Allocates room for a `T` in `storage` and constructs `value` into it
    ///
    /// This function has an advantage over [`Rc::new_in`] for large objects where because the allocation is done *before* `f` is called,
    /// the stack space for the return value of `f` may be elided by the compiler
    pub fn new_with_in(f: impl FnOnce() -> T, storage: S) -> Result<Self, StorageAllocError> {
        let (storage, handle, metadata) = Box::into_raw_parts(Box::new_with_in(
            || RcInner {
                strong: Cell::new(0),
                data: UnsafeCell::new(ManuallyDrop::new(f())),
            },
            storage,
        )?);
        Ok(unsafe { Self::from_raw_parts(storage, handle, metadata) })
    }

    /// Moves the `T` out of this [`Rc`], if its the only [`Rc`] left
    pub fn into_inner(rc: Self) -> Option<T> {
        let inner = Self::inner(&rc);
        if inner.strong.get() != 1 {
            return None;
        }

        unsafe {
            let value = inner.data.get().read();
            let (storage, handle, _) = Self::into_raw_parts(rc);
            storage.deallocate(Layout::new::<T>(), handle);
            Some(ManuallyDrop::into_inner(value))
        }
    }
}

impl_maybe_unsized_methods! {
    impl Clone [for] Rc
    where
        [
            S: ShareableStorage,
        ]
    {
        fn clone(&self) -> Self {
            let inner = Self::inner(self);
            debug_assert_ne!(inner.strong.get(), usize::MAX);
            inner.strong.set(inner.strong.get() + 1);
            let Rc {
                handle,
                ref storage,
                #[cfg(feature = "nightly")]
                metadata_ptr,
                _data,
            } = *self;
            Rc {
                handle,
                storage: unsafe { ShareableStorage::make_shared_copy(storage) },
                #[cfg(feature = "nightly")]
                metadata_ptr,
                _data,
            }
        }
    }
}

impl_maybe_unsized_methods! {
    impl [for] Rc {
        unsafe fn from_raw_parts(
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

        unsafe fn into_raw_parts(b: Self) -> (S, S::Handle, <T as Pointee>::Metadata) {
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

        fn inner(rc: &Self) -> &RcInner<T> {
            let ptr = unsafe { rc.storage.resolve(rc.handle) };
            cfg_if! {
                if #[cfg(feature = "nightly")] {
                    unsafe { NonNull::from_raw_parts(ptr, core::ptr::metadata(rc.metadata_ptr.as_ptr())).as_ref() }
                } else {
                    unsafe { ptr.cast().as_ref() }
                }
            }
        }

        /// Gets a [`NonNull<T>`] to the `T` stored in this [`Rc`]
        pub fn as_ptr(rc: &Self) -> NonNull<T> {
            let inner = Self::inner(rc);
            unsafe { NonNull::new_unchecked(inner.data.get() as _) }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "nightly")] {
        unsafe impl<#[may_dangle] T: ?Sized, S: Storage> Drop for Rc<T, S> {
            fn drop(&mut self) {
                let inner = Self::inner(self);

                debug_assert_ne!(inner.strong.get(), 0);
                inner.strong.set(inner.strong.get() - 1);

                if inner.strong.get() == 0 {
                    unsafe {
                        let layout = Layout::for_value(inner);
                        ManuallyDrop::drop(&mut *inner.data.get());
                        self.storage
                            .deallocate(layout, self.handle);
                    }
                }
            }
        }
    } else {
        impl<T, S: Storage> Drop for Rc<T, S> {
            fn drop(&mut self) {
                let inner = Self::inner(self);

                debug_assert_ne!(inner.strong.get(), 0);
                inner.strong.set(inner.strong.get() - 1);

                if inner.strong.get() == 0 {
                    unsafe {
                        let layout = Layout::new::<T>();
                        ManuallyDrop::drop(&mut *inner.data.get());
                        self.storage
                            .deallocate(layout, self.handle);
                    }
                }
            }
        }
    }
}

impl_maybe_unsized_methods! {
    impl Deref [for] Rc {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { Self::as_ptr(self).as_ref() }
        }
    }
}

impl_maybe_unsized_methods! {
    impl DerefMut [for] Rc {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { Self::as_ptr(self).as_mut() }
        }
    }
}

#[cfg(feature = "nightly")]
impl<T, U, S> core::ops::CoerceUnsized<Rc<U, S>> for Rc<T, S>
where
    T: core::marker::Unsize<U> + ?Sized,
    U: ?Sized,
    S: Storage,
{
}

#[cfg(feature = "nightly")]
impl<S: Storage> Rc<dyn core::any::Any, S> {
    /// Attempts to downcast the [`dyn Any`](core::any::Any) to a `T`
    pub fn downcast<T: 'static>(b: Self) -> Result<Rc<T, S>, Self> {
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
    pub unsafe fn downcast_unchecked<T: 'static>(b: Self) -> Rc<T, S> {
        debug_assert!(b.is::<T>());
        unsafe {
            let (storage, handle, _) = Self::into_raw_parts(b);
            Rc::from_raw_parts(storage, handle, ())
        }
    }
}
