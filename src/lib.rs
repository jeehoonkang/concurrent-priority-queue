#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

//! TODO: document

extern crate crossbeam_utils;

pub mod seqlock;

use seqlock::SeqLock;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// TODO: document
pub trait Max {
    /// TODO: document
    fn max() -> Self;
}

/// TODO: document
pub trait AtomicWrite {
    /// TODO: document
    fn write(&self, value: &Self);
}

impl Max for AtomicUsize {
    fn max() -> Self {
        AtomicUsize::new(usize::MAX)
    }
}

impl AtomicWrite for AtomicUsize {
    fn write(&self, value: &Self) {
        self.store(value.load(Ordering::Relaxed), Ordering::Relaxed);
    }
}

/// TODO: document
#[derive(Debug)]
pub struct MinPriorityQueue<T>
where
    T: Ord,
{
    min: SeqLock<T>,
    inner: Mutex<BTreeMap<T, usize>>,
}

impl<T> Default for MinPriorityQueue<T>
where
    T: Ord + Max,
{
    fn default() -> Self {
        Self {
            min: SeqLock::new(Max::max()),
            inner: Mutex::new(BTreeMap::new()),
        }
    }
}

impl<T> MinPriorityQueue<T>
where
    T: Ord + Max + Clone + AtomicWrite,
{
    /// TODO: document
    pub fn new() -> Self {
        Self::default()
    }

    /// TODO: document
    pub fn insert(&self, value: T) {
        let mut inner = self.inner.lock().unwrap();

        let min = unsafe { self.min.read_lock() };
        if &value < min.deref() {
            let min = unsafe { min.upgrade_exclusive() };
            min.write(&value);
        }

        *inner.entry(value).or_insert(0) += 1;
    }

    /// TODO: document
    pub fn remove(&self, value: T) {
        let mut inner = self.inner.lock().unwrap();

        let counter = inner.get_mut(&value).unwrap();
        *counter -= 1;
        if *counter == 0 {
            inner.remove(&value);

            let min = unsafe { self.min.read_lock() };
            if &value < min.deref() {
                let min = unsafe { min.upgrade_exclusive() };
                let min_value = inner.keys().next().cloned().unwrap_or_else(Max::max);
                min.write(&min_value);
            }
        }
    }

    /// TODO: document
    pub fn min(&self) -> T {
        loop {
            let min = unsafe { self.min.read_lock() };
            let result = min.deref().clone();
            if min.finish() {
                return result;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
