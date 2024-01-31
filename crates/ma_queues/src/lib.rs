use crate::messages::PublishArg;
use std::{
    alloc::Layout,
    sync::atomic::{fence, AtomicUsize, Ordering},
};
use thiserror::Error;
use versioned_lock::VersionedLock;

#[derive(Error, Debug, Copy, Clone, PartialEq)]
pub enum ReadError {
    #[error("Got sped past")]
    SpedPast,
    #[error("Queue empty")]
    Empty,
}

#[derive(Error, Debug, Copy, Clone)]
pub enum QueueError {
    #[error("Queue not initialized")]
    UnInitialized,
    #[error("Queue length not power of two")]
    LengthNotPowerOfTwo,
    #[error("Element size not power of two - 4")]
    ElementSizeNotPowerTwo,
}

#[cfg(feature = "ffi")]
pub mod ffi;
pub mod messages;
pub mod vector;
pub mod versioned_lock;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum QueueType {
    Unknown,
    MPMC,
    SPMC,
}

#[derive(Debug)]
#[repr(C, align(64))]
pub struct QueueHeader {
    pub queue_type: QueueType,  // 1
    elsize_shift_left_bits: u8, // 2
    is_initialized: u8,         // 3
    _pad1: u8,                  // 4
    elsize: u32,                // 8
    bufsize: u32,               // 12
    mask: u32,                  // 16
    count: AtomicUsize,         // 24
    start: *const u8
}

impl QueueHeader {
    #[inline]
    pub unsafe fn consume(
        &self,
        el: *mut u8,
        pos: u32,
        expected_version: u32,
    ) -> Result<(), ReadError> {

        let lock = self.start.offset((pos << self.elsize_shift_left_bits) as isize);
        let v = lock as *const u32;
        let dp = lock.offset(4);
        let l = self.elsize as usize;

        loop {
            let v1 = *v;

            fence(Ordering::Acquire);
            if v1 < expected_version {
                return Err(ReadError::Empty);
            }
            if v1 > expected_version {
                return Err(ReadError::SpedPast);
            }
            el.copy_from(dp, l);
            fence(Ordering::AcqRel);
            let v2 = *v;
            fence(Ordering::Acquire);
            if v1 == v2 {
                return Ok(());
            } else {
                return Err(ReadError::SpedPast);
            }
        }

    }
}

fn power_of_two(mut v: usize) -> usize {
    let mut n = 0;
    while v % 2 == 0 {
        v /= 2;
        n += 1;
    }
    n
}

#[repr(C, align(64))]
pub struct Queue<T> {
    pub header: QueueHeader,
    buffer: [versioned_lock::VersionedLock<T>],
}

impl<T> Queue<T> {
    /// Allocs (unshared) memory and initializes a new queue from it
    pub fn new(len: usize, queue_type: QueueType) -> Result<&'static Self, QueueError> {
        let real_len = len.next_power_of_two();
        let size =
            std::mem::size_of::<QueueHeader>() + real_len * std::mem::size_of::<VersionedLock<T>>();

        let q;

        unsafe {
            let ptr = std::alloc::alloc_zeroed(
                Layout::array::<u8>(size)
                    .unwrap()
                    .align_to(64)
                    .unwrap()
                    .pad_to_align(),
            );
            // Why real len you may ask. The size of the fat pointer ONLY includes the length of the
            // unsized part of the struct i.e. the buffer.
            q = &mut *(std::ptr::slice_from_raw_parts_mut(ptr, real_len) as *mut Queue<T>);
        }

        q.init(queue_type, len)?;
        Ok(q)
    }

    fn init(&mut self, queue_type: QueueType, len: usize) -> Result<(), QueueError> {
        let elsize = std::mem::size_of::<VersionedLock<T>>();
        if !len.is_power_of_two() {
            return Err(QueueError::LengthNotPowerOfTwo);
        }

        let mask = len - 1;

        self.header.queue_type = queue_type;
        self.header.elsize_shift_left_bits = power_of_two(elsize) as u8;
        self.header.bufsize = len as u32;
        self.header.mask = mask as u32;
        self.header.elsize = elsize as u32;
        self.header.start = unsafe{(self as *const _ as *const u8).offset(64)};
        self.header.is_initialized = true as u8;
        Ok(())
    }

    pub fn init_header(ptr: *mut u8, queue_type: QueueType, len: u32) -> Result<(), QueueError> {
        let elsize = std::mem::size_of::<VersionedLock<T>>();
        if !elsize.is_power_of_two() {
            return Err(QueueError::ElementSizeNotPowerTwo);
        }
        if !len.is_power_of_two() {
            return Err(QueueError::LengthNotPowerOfTwo);
        }

        let mask = len - 1;

        unsafe {
            let s = std::slice::from_raw_parts_mut(ptr, 1024);
            let header = &mut *(s as *mut _ as *mut QueueHeader) as &mut QueueHeader;
            header.queue_type = queue_type;

            header.elsize_shift_left_bits = power_of_two(elsize) as u8;
            header.bufsize = len as u32;
            header.mask = mask as u32;
            header.elsize = elsize as u32;
            header.is_initialized = true as u8;
        }
        Ok(())
    }

    pub const fn size_of(len: u32) -> usize {
        std::mem::size_of::<QueueHeader>()
            + (len.next_power_of_two() as usize) * std::mem::size_of::<VersionedLock<T>>()
    }

    pub fn from_ptr(ptr: *mut QueueHeader) -> Result<&'static Self, QueueError> {
        unsafe {
            let len = (*ptr).bufsize;
            if !len.is_power_of_two() {
                return Err(QueueError::LengthNotPowerOfTwo);
            }
            if (*ptr).is_initialized != true as u8 {
                return Err(QueueError::UnInitialized);
            }

            Ok(
                &*(std::ptr::slice_from_raw_parts_mut(ptr, (*ptr).bufsize as usize)
                    as *const Queue<T>),
            )
        }
    }

    // Note: Calling this from anywhere that's not a producer -> false sharing
    pub fn count(&self) -> usize {
        self.header.count.load(Ordering::Relaxed)
    }

    fn next_pos(&self) -> u32 {
        match &self.header.queue_type {
            QueueType::Unknown => panic!("Unknown queue"),
            QueueType::MPMC => {
                self.header.count.fetch_add(1, Ordering::AcqRel) as u32 & self.header.mask
            }
            QueueType::SPMC => self.header.count.load(Ordering::Relaxed) as u32 & self.header.mask,
        }
    }

    fn update_pos(&self) {
        match &self.header.queue_type {
            QueueType::Unknown => {
                panic!("Unknown Queue type")
            }
            QueueType::MPMC => {}
            QueueType::SPMC => {
                let c = self.header.count.load(Ordering::Relaxed);
                self.header
                    .count
                    .store(c.wrapping_add(1), Ordering::Relaxed);
            }
        }
    }

    fn load(&self, pos: u32) -> &VersionedLock<T> {
        unsafe { self.buffer.get_unchecked(pos as usize) }
    }

    fn cur_pos(&self) -> u32 {
        self.count() as u32 & self.header.mask
    }

    fn version(&self) -> u32 {
        (((self.count() / (self.header.mask as usize + 1)) << 1) + 2) as u32
    }

    fn produce(&self, item: &T) {
        let p = self.next_pos();
        let lock = self.load(p);
        lock.write(item);
        self.update_pos();
    }

    fn produce_arg(&self, item: &PublishArg) {
        let p = self.next_pos();
        let lock = self.load(p);
        lock.write_arg(item);
        self.update_pos();
    }

    fn consume(&self, el: &mut T, ri: u32, ri_ver: u32) -> Result<(), ReadError> {
        self.load(ri).read(el, ri_ver)
    }

    fn consume_danger(&self, el: &mut T, ri: u32, ri_ver: u32) -> Result<(), ReadError> {
        unsafe{self.header.consume(el as *mut _ as *mut u8, ri, ri_ver)}
    }

    fn len(&self) -> usize {
        self.header.bufsize as usize
    }

    // This exists just to check the state of the queue for debugging purposes
    #[allow(dead_code)]
    fn verify(&self) {
        let mut prev_v = self.load(0).version();
        let mut n_changes = 0;
        for i in 1..=self.header.mask {
            let lck = self.load(i as u32);
            let v = lck.version();
            if v != prev_v && v & 1 == 0 {
                n_changes += 1;
                log::debug!("version change at {i}: {prev_v} -> {v}");
                prev_v = v;
            }
        }
        if n_changes > 1 {
            panic!("what")
        }
    }

    fn produce_first(&self, item: &T) {
        // we check whether the version in the previous lock is different
        // to the one we are going to write to, otherwise there's something wrong
        // most likely the count we read from the shmem is not actually
        // the last written one. Cache not being flushed to shmem when producer
        // dies.
        loop {
            let p = self.next_pos();
            let prev_pos = if p == 0 { self.header.mask } else { p - 1 };
            let prev_version = self.load(prev_pos).version();
            let lock = self.load(p);
            let curv = lock.version();
            if curv != prev_version || (p == 0 && curv == prev_version) {
                lock.write_unpoison(item);
                self.update_pos();
                break;
            }
            self.update_pos();
        }
    }

    fn produce_first_arg(&self, item: &PublishArg) {
        // we check whether the version in the previous lock is different
        // to the one we are going to write to, otherwise there's something wrong
        // most likely the count we read from the shmem is not actually
        // the last written one. Cache not being flushed to shmem when producer
        // dies.
        loop {
            let p = self.next_pos();
            let prev_pos = if p == 0 { self.header.mask } else { p - 1 };
            let prev_version = self.load(prev_pos).version();
            let lock = self.load(p);
            let curv = lock.version();
            if curv != prev_version || (p == 0 && curv == prev_version) {
                lock.write_unpoison_arg(item);
                self.update_pos();
                break;
            }
            self.update_pos();
        }
    }
}

unsafe impl<T> Send for Queue<T> {}
unsafe impl<T> Sync for Queue<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for Queue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Queue:\nHeader:\n{:?}", self.header)
    }
}

/// Simply exists for the automatic produce_first
#[repr(C, align(64))]
pub struct Producer<'a, T> {
    // can't we just make this a usize since we're anyway padding?
    produced_first: u8, // 1
    pub queue: &'a Queue<T>,
}

impl<'a, T> From<&'a Queue<T>> for Producer<'a, T> {
    fn from(queue: &'a Queue<T>) -> Self {
        Self {
            produced_first: 0,
            queue: queue,
        }
    }
}

impl<'a, T> Producer<'a, T> {
    pub fn produce(&mut self, msg: &T) {
        if self.produced_first == 0 {
            self.queue.produce_first(msg);
            self.produced_first = 1
        } else {
            self.queue.produce(msg)
        }
    }
    pub fn produce_arg(&mut self, msg: &PublishArg) {
        if self.produced_first == 0 {
            self.queue.produce_first_arg(msg);
            self.produced_first = 1
        } else {
            self.queue.produce_arg(msg)
        }
    }
}

#[repr(C, align(64))]
#[derive(Debug)]
pub struct Consumer<'a, T> {
    /// Shared reference to the channel
    /// Read index pointer
    pos: u32, // 4
    mask: u32,             // 8
    expected_version: u32, // 12
    is_running: u8,        // 13
    _pad: [u8; 3],         // 16
    queue: &'a Queue<T>,   //32 fat ptr: (usize, pointer)
}

impl<'a, T> Consumer<'a, T> {
    pub fn recover_after_error(&mut self) {
        self.expected_version += 2
    }

    fn update_pos(&mut self) {
        self.pos = (self.pos + 1) & self.mask;
        self.expected_version += 2 * (self.pos == 0) as u32;
    }

    /// Nonblocking consume returning either Ok(()) or a ReadError
    pub fn try_consume(&mut self, el: &mut T) -> Result<(), ReadError> {
        self.queue.consume(el, self.pos, self.expected_version)?;
        self.update_pos();
        Ok(())
    }
    /// Nonblocking consume returning either Ok(()) or a ReadError
    pub fn try_consume_danger(&mut self, el: &mut T) -> Result<(), ReadError> {
        self.queue.consume_danger(el, self.pos, self.expected_version)?;
        self.update_pos();
        Ok(())
    }

    /// Blocking consume
    pub fn consume(&mut self, el: &mut T) {
        loop {
            match self.try_consume(el) {
                Ok(_) => {
                    return;
                }
                Err(ReadError::Empty) => {
                    continue;
                }
                Err(ReadError::SpedPast) => {
                    self.recover_after_error();
                }
            }
        }
    }
    /// Blocking consume
    pub fn consume_danger(&mut self, el: &mut T) {
        loop {
            match self.try_consume_danger(el) {
                Ok(_) => {
                    return;
                }
                Err(ReadError::Empty) => {
                    continue;
                }
                Err(ReadError::SpedPast) => {
                    self.recover_after_error();
                }
            }
        }
    }

    pub fn init_header(consumer_ptr: *mut Consumer<T>, queue: &'static Queue<T>) {
        unsafe {
            (*consumer_ptr).pos = queue.cur_pos();
            (*consumer_ptr).expected_version = queue.version();
            (*consumer_ptr).mask = queue.header.mask;
            (*consumer_ptr).queue = queue
        }
    }
}

impl<'a, T> From<&'a Queue<T>> for Consumer<'a, T> {
    fn from(queue: &'a Queue<T>) -> Self {
        let pos = queue.cur_pos();
        let expected_version = queue.version();
        Self {
            pos,
            mask: queue.header.mask,
            _pad: [0; 3],
            expected_version,
            is_running: 1,
            queue,
        }
    }
}

#[cfg(test)]
mod queue {
    use crate::messages::Message60;

    use super::*;

    #[test]
    fn power_of_two_test() {
        let t = 128;
        assert_eq!(power_of_two(t), 7);
    }
    #[test]
    fn headersize() {
        assert_eq!(64, std::mem::size_of::<QueueHeader>());
        assert_eq!(64, std::mem::size_of::<Consumer<'_, Message60>>())
    }

    #[test]
    fn basic() {
        for typ in [QueueType::SPMC, QueueType::MPMC] {
            let q = Queue::new(16, typ).unwrap();
            let mut p = Producer::from(&*q);
            let mut c = Consumer::from(&*q);
            p.produce(&1);
            let mut m = 0;

            assert_eq!(c.try_consume(&mut m), Ok(()));
            assert!(matches!(c.try_consume(&mut m), Err(ReadError::Empty)));
            assert_eq!(m, 1);
            for i in 0..16 {
                p.produce(&i);
            }
            for i in 0..16 {
                c.try_consume(&mut m).unwrap();
                assert_eq!(m, i);
            }

            assert!(matches!(c.try_consume(&mut m), Err(ReadError::Empty)));

            for i in 0..20 {
                p.produce(&1);
            }

            assert!(matches!(c.try_consume(&mut m), Err(ReadError::SpedPast)));
        }
    }

    fn multithread(n_writers: usize, n_readers: usize, tot_messages: usize) {
        let q = Queue::new(16, QueueType::MPMC).unwrap();

        let mut readhandles = Vec::new();
        for n in 0..n_readers {
            let mut c1 = Consumer::from(&*q);
            let cons = std::thread::spawn(move || {
                let mut c = 0;
                let mut m = 0;
                while c < tot_messages {
                    c1.consume(&mut m);
                    c += m;
                }
                assert_eq!(c, (0..tot_messages).sum());
            });
            readhandles.push(cons)
        }
        let mut writehandles = Vec::new();
        for n in 0..n_writers {
            let mut p1 = Producer::from(&*q);
            let prod1 = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(20));
                let mut c = n;
                while c < tot_messages {
                    p1.produce(&c);
                    c += n_writers;
                    std::thread::yield_now();
                }
            });
            writehandles.push(prod1);
        }

        for h in readhandles {
            h.join();
        }
        for h in writehandles {
            h.join();
        }
    }
    #[test]
    fn multithread_1_2() {
        multithread(1, 2, 100000);
    }
    #[test]
    fn multithread_1_4() {
        multithread(1, 4, 100000);
    }
    #[test]
    fn multithread_2_4() {
        multithread(2, 4, 100000);
    }
    #[test]
    fn multithread_4_4() {
        multithread(4, 4, 100000);
    }
    #[test]
    fn multithread_8_8() {
        multithread(8, 8, 100000);
    }
}
