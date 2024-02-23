use crate::messages::PublishArg;
use std::cell::UnsafeCell;
use std::sync::atomic::{fence, Ordering};
use std::fmt;

use super::ReadError;
//TODO: Make the types more rust like. I.e. on copy types -> copy on write/read, clone types -> copy std::mem::forget till read etc
/// A sequential lock
#[repr(C, align(64))]
pub struct VersionedLock<T> {
    version: UnsafeCell<usize>,
    data: UnsafeCell<T>,
}
unsafe impl<T: Send> Send for VersionedLock<T> {}
unsafe impl<T: Send> Sync for VersionedLock<T> {}

// TODO: Try 32 bit version
impl<T: Copy> VersionedLock<T> {
    /// Creates a new SeqLock with the given initial value.
    #[inline]
    pub const fn new(val: T) -> VersionedLock<T> {
        VersionedLock {
            version: UnsafeCell::new(0),
            data: UnsafeCell::new(val),
        }
    }

    fn _set_version(&self, version: usize) {
        unsafe { *self.version.get() = version }
    }
    pub fn set_version(&self, version: usize) {
        assert!(version & 1 == 0);
        self._set_version(version)
    }

    pub fn version(&self) -> usize {
        unsafe { *self.version.get() }
    }

    // Error returned if reader was sped past
    // If version is lower -> writer not yet written and busylock
    // if version is too high -> writer has written twice -> sped past -> error
    // if version changed -> got sped past because if v1 != expected_version it wouldn't even have started
    // reading
    #[inline]
    pub fn read(&self, result: &mut T, expected_version: usize) -> Result<(), ReadError> {
        // Load the first sequence number. The acquire ordering ensures that
        // this is done before reading the data.
        let v1 = unsafe { *self.version.get() };

        fence(Ordering::Acquire);
        // If the sequence number is odd then it means a writer is currently
        // modifying the value.
        if v1 < expected_version {
            return Err(ReadError::Empty);
        }

        // Here v1 has to be larger or equal than expected. If larger -> sped past -> error
        if v1 > expected_version {
            return Err(ReadError::SpedPast);
        }
        // Version is fine, supposedly not being written + not written twice

        // We need to use a volatile read here because the data may be
        // concurrently modified by a writer. We also use MaybeUninit in
        // case we read the data in the middle of a modification.
        unsafe {
                (result as *mut T).copy_from(self.data.get(), 1);
        }
        fence(Ordering::AcqRel);
        // Make sure the seq2 read occurs after reading the data. What we
        // ideally want is a load(Release), but the Release ordering is not
        // available on loads.

        // If the sequence number is the same then the data wasn't modified
        // while we were reading it, and can be returned.
        let v2 = unsafe { *self.version.get() };
        fence(Ordering::Acquire);
        if v1 == v2 {
            return Ok(());
        } else {
            return Err(ReadError::SpedPast);
        }
    }

    #[inline]
    pub fn read_no_ver(&self, result: &mut T) -> usize {
        loop {
            let v1 = unsafe { *self.version.get() };

            fence(Ordering::Acquire);
            if v1 & 1 == 1 {
                continue;
            }
            unsafe {
                (result as *mut  T).copy_from(self.data.get(), 1);
            }
            let v2 = unsafe { *self.version.get() };
            fence(Ordering::Acquire);
            if v1 == v2 {
                return v1;
            }
        }
    }

    #[inline]
    fn _write<F>(&self, f: F)
    where
        F: FnOnce() -> (),
    {
        unsafe {
            // Increment the sequence number. At this point, the number will be odd,
            // which will force readers to spin until we finish writing.
            let v = *self.version.get();
            *self.version.get() = v.wrapping_add(1) ;
            fence(Ordering::Release);
            fence(Ordering::AcqRel);
            // Make sure any writes to the data happen after incrementing the
            // sequence number. What we ideally want is a store(Acquire), but the
            // Acquire ordering is not available on stores.
            f();
            fence(Ordering::AcqRel);
            *self.version.get() = v.wrapping_add(2);
            fence(Ordering::Release);
        }
    }
    #[inline]
    pub fn write(&self, val: &T) {
        self._write(|| {
            unsafe { self.data.get().copy_from(val as *const T, 1) };
        });
    }

    pub fn write_arg(&self, val: &PublishArg) {
        self._write(|| {
            let t = self.data.get() as *mut u8;
            unsafe { t.copy_from(val.header as *const _ as *const u8, val.header_len as usize) };
        });
    }

    #[inline]
    fn _write_unpoison<F>(&self, f: F)
    where
        F: FnOnce() -> (),
    {
        unsafe {
            let v = *self.version.get();
            let v1 = v.wrapping_add(v.wrapping_sub(1) & 1);
            *self.version.get() = v1;
            fence(Ordering::Release);
            f();
            *self.version.get() = v1.wrapping_add(1);
            fence(Ordering::Release);
        }
    }
    #[inline]
    pub fn write_unpoison(&self, val: &T) {
        self._write_unpoison(|| {
            let t = self.data.get() as *mut u8;
            unsafe { t.copy_from(val as *const _ as *const u8, std::mem::size_of::<T>()) };
        });
    }
    #[inline]
    pub fn write_unpoison_arg(&self, val: &PublishArg) {
        self._write_unpoison(|| unsafe {
            self.data
                .get()
                .copy_from(val.content as *const _, val.header.length as usize);
        });
    }
}

impl<T: Copy + Default> Default for VersionedLock<T> {
    #[inline]
    fn default() -> VersionedLock<T> {
        VersionedLock::new(Default::default())
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for VersionedLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqLock {{ data: {:?} }}", unsafe { *self.data.get() })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn lock_size() {
        assert_eq!(std::mem::size_of::<VersionedLock<[u8; 48]>>(), 64);
        assert_eq!(std::mem::size_of::<VersionedLock<[u8; 61]>>(), 128)
    }
    #[test]
    fn read_no_ver() {
        use crate::messages::Message60;
        let lock = VersionedLock::default();
        std::thread::scope(|s| {
            s.spawn(|| {
                core_affinity::set_for_current(core_affinity::CoreId { id:  3 });
                let mut m = Message60::default();
                let mut count = 0u8;
                let mut curver = 0;
                let mut prevdat = 255;
                loop {
                    let newver = lock.read_no_ver(&mut m);
                    if newver > curver + 1  {
                        if m.data[0] == 1 && m.data[1] == 2 {
                            return;
                        }
                        for i in m.data {
                            assert_eq!(count, i);
                            assert_ne!(prevdat, i);

                        }
                        prevdat = m.data[0];

                        count = count.wrapping_add(1);
                        curver = newver;
                    }
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(100));
            s.spawn(|| {
                core_affinity::set_for_current(core_affinity::CoreId { id:  1 });
                let curt = std::time::Instant::now();
                let mut count = 0u8;
                while curt.elapsed() < std::time::Duration::from_secs(10) {
                    lock.write(&Message60 {
                        data: [count; 60],
                    });
                    count = count.wrapping_add(1);
                    std::thread::sleep(std::time::Duration::from_micros(20));
                }
                let mut data = [count; 60];
                data[0] = 1;
                data[1] = 2;
                lock.write(&Message60 { data: data });
            });
        });
    }

    #[test]
    fn read() {
        use crate::messages::Message1020;
        let lock: VersionedLock<Message1020> = VersionedLock::default();
        let mut m = Default::default();
        assert!(matches!(lock.read(&mut m, 2), Err(ReadError::Empty)));

        std::thread::scope(|s| {
            let consumer = s.spawn(|| {
                let mut prev = 0;
                let mut ver = 2;
                let mut m = Message1020::default();
                let mut tot_read = 0;
                loop {
                    match lock.read(&mut m, ver) {
                        Ok(()) => {
                            if m.data[0] == 1 && m.data[1] == 2 {
                                break;
                            }
                            assert!(m.data.iter().all(|d| *d == m.data[0]));
                            prev = prev.max(m.data[0]);
                            ver = ver.wrapping_add(2);
                            tot_read += 1;
                        }
                        Err(ReadError::SpedPast) => {
                            ver = ver.wrapping_add(2);
                        }
                        _ => {}
                    }
                }
                assert!(prev > 0);
                tot_read
            });
            let producer = s.spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let curt = std::time::Instant::now();
                let mut count = 0;
                let mut tot = 0;
                while curt.elapsed() < std::time::Duration::from_secs(1) {
                    lock.write(&Message1020 {
                        data: [count; 1020],
                    });
                    count = count.wrapping_add(1);
                    tot += 1;
                    std::thread::sleep(std::time::Duration::from_micros(500));
                }
                let mut data = [count; 1020];
                data[0] = 1;
                data[1] = 2;
                lock.write(&Message1020 { data: data });
                tot
            });
            assert_eq!(producer.join().unwrap(), consumer.join().unwrap())
        });
    }
    #[test]
    fn write_unpoison() {
        let lock = VersionedLock::default();
        lock._set_version(1);
        lock.write_unpoison(&1);
        assert_eq!(lock.version(), 2);
    }
}
