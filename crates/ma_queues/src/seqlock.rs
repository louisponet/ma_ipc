use crate::messages::PublishArg;
use std::arch::asm;
use std::cell::UnsafeCell;
use std::fmt;
use std::sync::atomic::{compiler_fence, fence, AtomicUsize, Ordering};

use super::ReadError;
//TODO: Make the types more rust like. I.e. on copy types -> copy on write/read, clone types -> copy std::mem::forget till read etc
/// A sequential lock
#[repr(C, align(64))]
pub struct SeqLock<T> {
    version: AtomicUsize,
    data: UnsafeCell<T>,
}
unsafe impl<T: Send> Send for SeqLock<T> {}
unsafe impl<T: Send> Sync for SeqLock<T> {}

// TODO: Try 32 bit version
impl<T: Copy> SeqLock<T> {
    /// Creates a new SeqLock with the given initial value.
    #[inline]
    pub const fn new(val: T) -> SeqLock<T> {
        SeqLock {
            version: AtomicUsize::new(0),
            data: UnsafeCell::new(val),
        }
    }

    fn _set_version(&self, version: usize) {
        self.version.store(version, Ordering::Relaxed)
    }

    pub fn set_version(&self, version: usize) {
        assert!(version & 1 == 0);
        self._set_version(version)
    }

    pub fn version(&self) -> usize {
        self.version.load(Ordering::Relaxed)
    }

    // Error returned if reader was sped past
    // If version is lower -> writer not yet written and busylock
    // if version is too high -> writer has written twice -> sped past -> error
    // if version changed -> got sped past because if v1 != expected_version it wouldn't even have started
    // reading
    #[inline(never)]
    pub fn read(&self, result: &mut T, expected_version: usize) -> Result<(), ReadError> {
        loop {
            // Load the first sequence number. The acquire ordering ensures that
            // this is done before reading the data.
            let v1 = self.version.load(Ordering::Relaxed);

            // If the sequence number is odd then it means a writer is currently
            // modifying the value.
            // Version is fine, supposedly not being written + not written twice

            // We need to use a volatile read here because the data may be
            // concurrently modified by a writer. We also use MaybeUninit in
            // case we read the data in the middle of a modification.
            unsafe {
                (result as *mut T).copy_from(self.data.get(), 1);
            }
            // Make sure the seq2 read occurs after reading the data. What we
            // ideally want is a load(Release), but the Release ordering is not
            // available on loads.

            // If the sequence number is the same then the data wasn't modified
            // while we were reading it, and can be returned.
            let v2 = self.version.load(Ordering::Relaxed);
            fence(Ordering::Acquire);
            if v1 == v2 {
                if v1 == expected_version {
                    return Ok(());
                } else if v1 < expected_version {
                    return Err(ReadError::Empty);
                } else {
                    return Err(ReadError::SpedPast);
                }
            }
        }
    }

    #[inline(never)]
    pub fn read_no_ver(&self, result: &mut T) {
        loop {
            let v1 = self.version.load(Ordering::Acquire);
            compiler_fence(Ordering::AcqRel);
            unsafe {
                *result = *self.data.get();
            }
            compiler_fence(Ordering::AcqRel);
            let v2 = self.version.load(Ordering::Acquire);
            if v1 == v2 && v1 & 1 == 0 {
                return;
            }
        }
    }

    #[inline(always)]
    fn _write<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        // Increment the sequence number. At this point, the number will be odd,
        // which will force readers to spin until we finish writing.
        let v = self.version.fetch_add(1, Ordering::Release);
        compiler_fence(Ordering::AcqRel);
        // Make sure any writes to the data happen after incrementing the
        // sequence number. What we ideally want is a store(Acquire), but the
        // Acquire ordering is not available on stores.
        f();
        compiler_fence(Ordering::AcqRel);
        self.version.store(v.wrapping_add(2), Ordering::Release);
    }

    #[inline(never)]
    pub fn write(&self, val: &T) {
        self._write(|| {
            unsafe { self.data.get().copy_from(val as *const T, 1) };
        });
    }

    #[inline(never)]
    pub fn write_arg(&self, val: &PublishArg) {
        self._write(|| {
            let t = self.data.get() as *mut u8;
            unsafe { t.copy_from(val.header as *const _ as *const u8, val.header_len as usize) };
        });
    }

    #[inline(always)]
    fn _write_unpoison<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        let v = self.version.load(Ordering::Relaxed);
        self.version.store(v.wrapping_add(v.wrapping_sub(1) & 1), Ordering::Release);
        // Make sure any writes to the data happen after incrementing the
        // sequence number. What we ideally want is a store(Acquire), but the
        // Acquire ordering is not available on stores.
        compiler_fence(Ordering::AcqRel);
        f();
        compiler_fence(Ordering::AcqRel);
        self.version.store(v.wrapping_add(1), Ordering::Relaxed);
    }

    #[inline(never)]
    pub fn write_unpoison(&self, val: &T) {
        self._write_unpoison(|| {
            let t = self.data.get() as *mut u8;
            unsafe { t.copy_from(val as *const _ as *const u8, std::mem::size_of::<T>()) };
        });
    }

    #[inline(never)]
    pub fn write_unpoison_arg(&self, val: &PublishArg) {
        self._write_unpoison(|| unsafe {
            self.data
                .get()
                .copy_from(val.content as *const _, val.header.length as usize);
        });
    }

    #[inline(always)]
    #[allow(named_asm_labels)]
    fn _write_multi<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        // Increment the sequence number. At this point, the number will be odd,
        // which will force readers to spin until we finish writing.
        let mut v = self.version.fetch_or(1, Ordering::AcqRel);
        while v & 1 == 1 {
            v = self.version.fetch_or(1, Ordering::AcqRel);
        }
        // Make sure any writes to the data happen after incrementing the
        // sequence number. What we ideally want is a store(Acquire), but the
        // Acquire ordering is not available on stores.
        f();
        compiler_fence(Ordering::AcqRel);
        self.version.store(v.wrapping_add(2), Ordering::Release);
    }
    #[inline(never)]
    pub fn write_multi(&self, val: &T) {
        self._write_multi(|| {
            unsafe { self.data.get().copy_from(val as *const T, 1) };
        });
    }
}

impl<T: Copy + Default> Default for SeqLock<T> {
    #[inline]
    fn default() -> SeqLock<T> {
        SeqLock::new(Default::default())
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for SeqLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqLock {{ data: {:?} }}", unsafe { *self.data.get() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    #[test]
    fn lock_size() {
        assert_eq!(std::mem::size_of::<SeqLock<[u8; 48]>>(), 64);
        assert_eq!(std::mem::size_of::<SeqLock<[u8; 61]>>(), 128)
    }

    fn consumer_loop<const N: usize>(lock: &SeqLock<[usize; N]>, done: &AtomicBool) {
        let mut msg = [0usize; N];
        while !done.load(Ordering::Relaxed) {
            lock.read_no_ver(&mut msg);
            let first = msg[0];
            for i in msg {
                assert_eq!(first, i);
            }
        }
    }

    fn producer_loop<const N: usize>(lock: &SeqLock<[usize; N]>, done: &AtomicBool, multi: bool) {
        let curt = Instant::now();
        let mut count = 0;
        let mut msg = [0usize; N];
        while curt.elapsed() < Duration::from_secs(1) {
            msg.fill(count);
            if multi {
                lock.write_multi(&msg);
            } else {
                lock.write(&msg);
            }
            count = count.wrapping_add(1);
        }
        done.store(true, Ordering::Relaxed);
    }

    fn read_test<const N: usize>() {
        let lock = SeqLock::new([0usize; N]);
        let done = AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                consumer_loop(&lock, &done);
            });
            s.spawn(|| {
                producer_loop(&lock, &done, false);
            });
        });
    }

    fn read_test_multi<const N: usize>() {
        let lock = SeqLock::new([0usize; N]);
        let done = AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                consumer_loop(&lock, &done);
            });
            s.spawn(|| {
                producer_loop(&lock, &done, true);
            });
            s.spawn(|| {
                producer_loop(&lock, &done, true);
            });
        });
    }

    #[test]
    fn read_16() {
        read_test::<16>()
    }
    #[test]
    fn read_32() {
        read_test::<32>()
    }
    #[test]
    fn read_64() {
        read_test::<64>()
    }
    #[test]
    fn read_128() {
        read_test::<128>()
    }
    #[test]
    fn read_large() {
        read_test::<65536>()
    }

    #[test]
    fn read_16_multi() {
        read_test_multi::<16>()
    }
    #[test]
    fn read_32_multi() {
        read_test_multi::<32>()
    }
    #[test]
    fn read_64_multi() {
        read_test_multi::<64>()
    }
    #[test]
    fn read_128_multi() {
        read_test_multi::<128>()
    }
    #[test]
    fn read_large_multi() {
        read_test_multi::<65536>()
    }

    #[test]
    fn write_unpoison() {
        let lock = SeqLock::default();
        lock._set_version(1);
        lock.write_unpoison(&1);
        assert_eq!(lock.version(), 2);
    }
}
