//! A list with a fixed capacity, stored inline so filling it never
//! allocates (the processing path is realtime-safe).

use std::ops::{Deref, DerefMut};

/// Up to `N` items; reads as a slice of the ones pushed.
#[derive(Clone, Copy, Debug)]
pub struct FixedList<T, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for FixedList<T, N> {
    fn default() -> Self {
        FixedList {
            items: [T::default(); N],
            len: 0,
        }
    }
}

impl<T, const N: usize> FixedList<T, N> {
    /// Appends `item`. Panics when the list is full.
    pub(crate) fn push(&mut self, item: T) {
        self.items[self.len] = item;
        self.len += 1;
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }
}

impl<T, const N: usize> Deref for FixedList<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items[..self.len]
    }
}

impl<T, const N: usize> DerefMut for FixedList<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }
}

impl<T: PartialEq, const N: usize> PartialEq for FixedList<T, N> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
