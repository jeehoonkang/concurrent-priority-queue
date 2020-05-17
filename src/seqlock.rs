//! TODO: document

use core::mem;
use core::ops::Deref;
use core::sync::atomic::{fence, AtomicUsize, Ordering};

use crossbeam_utils::Backoff;

/// TODO: document
#[derive(Debug)]
pub struct RawSeqLock {
    seq: AtomicUsize,
}

impl RawSeqLock {
    /// TODO: document
    pub const fn new() -> Self {
        Self {
            seq: AtomicUsize::new(0),
        }
    }

    /// TODO: document
    pub fn write_begin(&self) -> usize {
        let backoff = Backoff::new();

        loop {
            let seq = self.seq.load(Ordering::Relaxed);
            if seq & 1 == 0
                && self
                    .seq
                    .compare_exchange(
                        seq,
                        seq.wrapping_add(1),
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                fence(Ordering::Release);
                return seq;
            }

            backoff.snooze();
        }
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn write_begin_exclusive(&self) -> usize {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        seq
    }

    /// TODO: document
    pub fn write_end(&self, seq: usize) {
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }

    /// TODO: document
    pub fn read_begin(&self) -> usize {
        let backoff = Backoff::new();

        loop {
            let seq = self.seq.load(Ordering::Acquire);
            if seq & 1 == 0 {
                return seq;
            }

            backoff.snooze();
        }
    }

    /// TODO: document
    pub fn read_validate(&self, seq: usize) -> bool {
        fence(Ordering::Acquire);

        seq == self.seq.load(Ordering::Relaxed)
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// `seq` must be even.
    pub unsafe fn upgrade(&self, seq: usize) -> Result<(), ()> {
        if self
            .seq
            .compare_exchange(
                seq,
                seq.wrapping_add(1),
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            return Err(());
        }

        fence(Ordering::Release);
        Ok(())
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// `seq` must be even. it should be exclusive.
    pub unsafe fn upgrade_exclusive(&self, seq: usize) {
        self.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
    }
}

/// TODO: document
#[derive(Debug)]
pub struct SeqLock<T> {
    lock: RawSeqLock,
    data: T,
}

/// TODO: document
#[derive(Debug)]
pub struct WriteGuard<'s, T> {
    lock: &'s SeqLock<T>,
    seq: usize,
}

/// TODO: document
#[derive(Debug)]
pub struct ReadGuard<'s, T> {
    lock: &'s SeqLock<T>,
    seq: usize,
}

unsafe impl<T: Send> Send for SeqLock<T> {}
unsafe impl<T: Send> Sync for SeqLock<T> {}

unsafe impl<'s, T> Send for WriteGuard<'s, T> {}
unsafe impl<'s, T: Send + Sync> Sync for WriteGuard<'s, T> {}

unsafe impl<'s, T> Send for ReadGuard<'s, T> {}
unsafe impl<'s, T: Send + Sync> Sync for ReadGuard<'s, T> {}

impl<T> SeqLock<T> {
    /// TODO: document
    pub const fn new(data: T) -> Self {
        SeqLock {
            lock: RawSeqLock::new(),
            data,
        }
    }

    /// TODO: document
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// TODO: document
    pub fn write_lock(&self) -> WriteGuard<T> {
        let seq = self.lock.write_begin();
        WriteGuard { lock: self, seq }
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn write_lock_exclusive(&self) -> WriteGuard<T> {
        let seq = self.lock.write_begin_exclusive();
        WriteGuard { lock: self, seq }
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// All reads from the underlying data should be atomic.
    pub unsafe fn read_lock(&self) -> ReadGuard<T> {
        let seq = self.lock.read_begin();
        ReadGuard { lock: self, seq }
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// All reads from the underlying data should be atomic.
    pub unsafe fn read<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        let guard = self.read_lock();
        let result = f(&guard);

        if guard.finish() {
            Some(result)
        } else {
            None
        }
    }
}

impl<'s, T> Deref for WriteGuard<'s, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.lock.data
    }
}

impl<'s, T> Drop for WriteGuard<'s, T> {
    fn drop(&mut self) {
        self.lock.lock.write_end(self.seq);
    }
}

impl<'s, T> Deref for ReadGuard<'s, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.lock.data
    }
}

impl<'s, T> Clone for ReadGuard<'s, T> {
    fn clone(&self) -> Self {
        Self {
            lock: self.lock,
            seq: self.seq,
        }
    }
}

impl<'s, T> Drop for ReadGuard<'s, T> {
    fn drop(&mut self) {
        // HACK(@jeehoonkang): we really need linear type here:
        // https://github.com/rust-lang/rfcs/issues/814
        panic!("seqlock::ReadGuard should never drop: use Self::finish() instead");
    }
}

impl<'s, T> ReadGuard<'s, T> {
    /// TODO: document
    pub fn validate(&self) -> bool {
        self.lock.lock.read_validate(self.seq)
    }

    /// TODO: document
    pub fn restart(&mut self) {
        self.seq = self.lock.lock.read_begin();
    }

    /// TODO: document
    pub fn finish(self) -> bool {
        let result = self.lock.lock.read_validate(self.seq);
        mem::forget(self);
        result
    }

    /// TODO: document
    pub fn upgrade(self) -> Result<WriteGuard<'s, T>, ()> {
        let result = if unsafe { self.lock.lock.upgrade(self.seq).is_ok() } {
            Ok(WriteGuard {
                lock: self.lock,
                seq: self.seq,
            })
        } else {
            Err(())
        };
        mem::forget(self);
        result
    }

    /// TODO: document
    ///
    /// # Safety
    ///
    /// TODO
    pub unsafe fn upgrade_exclusive(self) -> WriteGuard<'s, T> {
        self.lock.lock.upgrade_exclusive(self.seq);
        let result = WriteGuard {
            lock: self.lock,
            seq: self.seq,
        };
        mem::forget(self);
        result
    }
}
