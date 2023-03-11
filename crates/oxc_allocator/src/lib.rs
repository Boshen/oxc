use std::{
    alloc::Layout,
    fmt,
    hash::{Hash, Hasher},
    ops::{self, Deref},
    ptr::{self, NonNull},
};

use allocator_api2::alloc::AllocError;
use blink_alloc::BlinkAlloc;
// use bumpalo::Bump;
use serde::ser::{Serialize, Serializer};

// #[derive(Debug)]
pub struct Allocator {
    blink: BlinkAlloc,
}

impl Allocator {
    #[inline(always)]
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let ptr = self.blink.allocate(Layout::new::<T>()).unwrap();
        let ptr = ptr.as_ptr().cast::<T>();
        unsafe {
            ptr.write(value);
        }
        unsafe { &mut *ptr }
    }

    #[inline(always)]
    pub fn alloc_str(&self, value: &str) -> &mut str {
        let ptr = self.blink.allocate(Layout::for_value(value)).unwrap();
        let ptr = ptr.as_ptr().cast::<u8>();
        let slice = unsafe { std::slice::from_raw_parts_mut(ptr, value.len()) };
        slice.copy_from_slice(value.as_bytes());
        unsafe { std::str::from_utf8_unchecked_mut(slice) }
    }
}

// SAFETY: Make Bump Sync and Send, it's our responsibility to never
// simultaneously mutate across threads.
unsafe impl Send for Allocator {}
unsafe impl Sync for Allocator {}

unsafe impl allocator_api2::alloc::Allocator for Allocator {
    #[inline(always)]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // self.blink
        //     .try_alloc_layout(layout)
        //     .map(|ptr| unsafe {
        //         NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
        //             ptr.as_ptr(),
        //             layout.size(),
        //         ))
        //     })
        //     .map_err(|_| AllocError)

        self.blink.allocate(layout)
    }

    #[inline(always)]
    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {}
}

// pub type Box<'a, T> = allocator_api2::boxed::Box<T, &'a Allocator>;
pub type Vec<'a, T> = allocator_api2::vec::Vec<T, &'a Allocator>;

impl Default for Allocator {
    fn default() -> Self {
        Self { blink: BlinkAlloc::new() }
    }
}

impl Deref for Allocator {
    type Target = BlinkAlloc;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.blink
    }
}

pub struct String<'a> {
    vec: Vec<'a, u8>,
}

impl<'a> String<'a> {
    #[inline(always)]
    pub fn new_in(alloc: &'a Allocator) -> Self {
        String { vec: Vec::new_in(alloc) }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.vec) }
    }

    #[inline(always)]
    pub fn push(&mut self, ch: char) {
        match ch.len_utf8() {
            1 => self.vec.push(ch as u8),
            _ => self.vec.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }

    #[inline(always)]
    pub fn leak(self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.vec.leak()) }
    }

    #[inline(always)]
    pub fn from_str_in(s: &str, alloc: &'a Allocator) -> Self {
        let mut vec = Vec::with_capacity_in(s.len(), alloc);
        vec.extend_from_slice(s.as_bytes());
        String { vec }
    }

    #[inline(always)]
    pub fn push_str(&mut self, s: &str) {
        self.vec.extend_from_slice(s.as_bytes());
    }
}

impl<'a> fmt::Debug for String<'a> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

/// Bumpalo Box
pub struct Box<'alloc, T: ?Sized>(pub &'alloc mut T);

impl<'alloc, T> Box<'alloc, T> {
    #[must_use]
    pub fn into_inner(b: Self) -> T {
        // This pointer read is safe because the reference `self.0` is
        // guaranteed to be unique--not just now, but we're guaranteed it's not
        // borrowed from some other reference. This in turn is because we never
        // construct an alloc::Box with a borrowed reference, only with a fresh
        // one just allocated from a Bump.
        unsafe { ptr::read(b.0 as *mut T) }
    }
}

impl<'alloc, T: ?Sized> ops::Deref for Box<'alloc, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        self.0
    }
}

impl<'alloc, T: ?Sized> ops::DerefMut for Box<'alloc, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        self.0
    }
}

impl<'alloc, T: ?Sized + fmt::Debug> fmt::Debug for Box<'alloc, T> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'alloc, T> PartialEq for Box<'alloc, T>
where
    T: PartialEq<T> + ?Sized,
{
    #[inline(always)]
    fn eq(&self, other: &Box<'alloc, T>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}

impl<'alloc, T> Serialize for Box<'alloc, T>
where
    T: Serialize,
{
    #[inline(always)]
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(s)
    }
}

impl<'alloc, T: Hash> Hash for Box<'alloc, T> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
