use crate::messages::PublishArg;
use seqlock::SeqLock;
use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, PartialEq)]
pub enum ReadError {
    #[error("Got sped past")]
    SpedPast,
    #[error("Queue empty")]
    Empty,
}

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Queue not initialized")]
    UnInitialized,
    #[error("Queue length not power of two")]
    LengthNotPowerOfTwo,
    #[error("Element size not power of two - 4")]
    ElementSizeNotPowerTwo,
    #[cfg(feature = "shmem")]
    #[error("Shmem error")]
    SharedMemoryError(#[from] shared_memory::ShmemError),
}

#[cfg(feature = "ffi")]
pub mod ffi;
pub mod messages;
pub mod seqlock;
pub mod vector;

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
    pub queue_type:         QueueType,   // 1
    elsize_shift_left_bits: u8,          // 2
    pub is_initialized:     u8,          // 3
    _pad1:                  u8,          // 4
    pub elsize:             u32,         // 8
    mask:                   usize,       // 16
    pub count:              AtomicUsize, // 24
}
impl QueueHeader {
    /// in bytes
    pub fn sizeof(&self) -> usize {
        (self.mask + 1) * (self.elsize as usize)
    }

    pub fn n_elements(&self) -> usize {
        self.mask + 1
    }

    pub fn from_ptr(ptr: *mut u8) -> &'static mut Self {
        unsafe { &mut *(ptr as *mut Self) }
    }
}

#[cfg(feature = "shmem")]
impl QueueHeader {
    pub fn shared<P: AsRef<std::path::Path>>(path: P) -> &'static mut Self {
        use shared_memory::ShmemConf;
        match ShmemConf::new()
            .flink(&path)
            .open()
        {
            Ok(shmem) => {
                let o = unsafe { &mut *(shmem.as_ptr() as *mut QueueHeader)};
                std::mem::forget(shmem);
                o
            }
            _ => panic!("couldn't open shmem")
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
    buffer:     [seqlock::SeqLock<T>],
}

impl<T: Copy> Queue<T> {
    /// Allocs (unshared) memory and initializes a new queue from it
    pub fn new(len: usize, queue_type: QueueType) -> Result<&'static Self, QueueError> {
        let real_len = len.next_power_of_two();
        let size =
            std::mem::size_of::<QueueHeader>() + real_len * std::mem::size_of::<SeqLock<T>>();

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
            Self::from_uninitialized_ptr(ptr, real_len, queue_type)
        }
    }

    pub const fn size_of(len: usize) -> usize {
        std::mem::size_of::<QueueHeader>()
            + len.next_power_of_two() * std::mem::size_of::<SeqLock<T>>()
    }

    pub fn from_uninitialized_ptr(
        ptr: *mut u8,
        len: usize,
        queue_type: QueueType,
    ) -> Result<&'static Self, QueueError> {
        if !len.is_power_of_two() {
            return Err(QueueError::LengthNotPowerOfTwo);
        }
        unsafe {
            let q = &mut *(std::ptr::slice_from_raw_parts_mut(ptr, len) as *mut Queue<T>);
            let elsize = std::mem::size_of::<SeqLock<T>>();
            if !len.is_power_of_two() {
                return Err(QueueError::LengthNotPowerOfTwo);
            }

            let mask = len - 1;

            q.header.queue_type = queue_type;
            q.header.elsize_shift_left_bits = power_of_two(elsize) as u8;
            q.header.mask = mask;
            q.header.elsize = elsize as u32;
            q.header.is_initialized = true as u8;
            Ok(q)
        }
    }

    #[allow(dead_code)]
    pub fn from_initialized_ptr(ptr: *mut QueueHeader) -> Result<&'static Self, QueueError> {
        unsafe {
            let len = (*ptr).mask + 1;
            if !len.is_power_of_two() {
                return Err(QueueError::LengthNotPowerOfTwo);
            }
            if (*ptr).is_initialized != true as u8 {
                return Err(QueueError::UnInitialized);
            }

            Ok(&*(std::ptr::slice_from_raw_parts_mut(ptr, len) as *const Queue<T>))
        }
    }

    // Note: Calling this from anywhere that's not a producer -> false sharing
    pub fn count(&self) -> usize {
        self.header.count.load(Ordering::Relaxed)
    }

    fn next_count(&self) -> usize {
        match self.header.queue_type {
            QueueType::Unknown => panic!("Unknown queue"),
            QueueType::MPMC => self.header.count.fetch_add(1, Ordering::AcqRel),
            QueueType::SPMC => {
                let c = self.header.count.load(Ordering::Relaxed);
                self.header
                    .count
                    .store(c.wrapping_add(1), Ordering::Relaxed);
                c
            }
        }
    }

    fn load(&self, pos: usize) -> &SeqLock<T> {
        unsafe { self.buffer.get_unchecked(pos) }
    }

    fn cur_pos(&self) -> usize {
        self.count() & self.header.mask
    }

    fn version(&self) -> usize {
        ((self.count() / (self.header.mask + 1)) << 1) + 2
    }

    pub fn version_of(&self, pos: usize) -> usize {
        self.load(pos).version()
    }

    // returns the current count
    fn produce(&self, item: &T) -> usize {
        let p = self.next_count();
        let lock = self.load(p & self.header.mask);
        lock.write(item);
        p
    }

    fn produce_arg(&self, item: &PublishArg) -> usize {
        let p = self.next_count();
        let lock = self.load(p & self.header.mask);
        lock.write_arg(item);
        p
    }

    fn consume(&self, el: &mut T, ri: usize, ri_ver: usize) -> Result<(), ReadError> {
        self.load(ri).read(el, ri_ver)
    }

    pub fn read(&self, el: &mut T, ri: usize) {
        self.load(ri).read_no_ver(el)
    }

    fn len(&self) -> usize {
        self.header.mask + 1
    }

    // This exists just to check the state of the queue for debugging purposes
    #[allow(dead_code)]
    pub fn verify(&self) {
        let mut prev_v = self.load(0).version();
        let mut n_changes = 0;
        for i in 1..=self.header.mask {
            let lck = self.load(i);
            let v = lck.version();
            if v & 1 == 1 {
                panic!("odd version at {i}: {prev_v} -> {v}");
            }
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

    fn produce_first(&self, item: &T) -> usize {
        // we check whether the version in the previous lock is different
        // to the one we are going to write to, otherwise there's something wrong
        // most likely the count we read from the shmem is not actually
        // the last written one. Cache not being flushed to shmem when producer
        // dies.
        loop {
            let m = self.header.mask;
            let c = self.next_count();
            let p = c & m;
            let prev_pos = if p == 0 { m } else { p - 1 };
            let prev_version = self.load(prev_pos).version();
            let lock = self.load(p);
            let curv = lock.version();
            if curv != prev_version || (p == 0 && curv == prev_version) {
                lock.write_unpoison(item);
                return c;
            }
        }
    }

    fn produce_first_arg(&self, item: &PublishArg) -> usize {
        // we check whether the version in the previous lock is different
        // to the one we are going to write to, otherwise there's something wrong
        // most likely the count we read from the shmem is not actually
        // the last written one. Cache not being flushed to shmem when producer
        // dies.
        loop {
            let m = self.header.mask;
            let c = self.next_count();
            let p = c & m;
            let prev_pos = if p == 0 { m } else { p - 1 };
            let prev_version = self.load(prev_pos).version();
            let lock = self.load(p);
            let curv = lock.version();
            if curv != prev_version || (p == 0 && curv == prev_version) {
                lock.write_unpoison_arg(item);
                return c;
            }
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

#[cfg(feature = "shmem")]
impl<T: Copy> Queue<T> {
    pub fn shared<P: AsRef<std::path::Path>>(
        shmem_flink: P,
        size: usize,
        typ: QueueType,
    ) -> Result<&'static Self, QueueError> {
        use shared_memory::{ShmemConf, ShmemError};
        match ShmemConf::new()
            .size(Self::size_of(size))
            .flink(&shmem_flink)
            .create()
        {
            Ok(shmem) => {
                let ptr = shmem.as_ptr();
                std::mem::forget(shmem);
                Self::from_uninitialized_ptr(ptr, size, typ)
            }
            Err(ShmemError::LinkExists) => {
                let shmem = ShmemConf::new().flink(shmem_flink).open().unwrap();
                let ptr = shmem.as_ptr() as *mut QueueHeader;
                std::mem::forget(shmem);
                Self::from_initialized_ptr(ptr)
            }
            Err(e) => {
                eprintln!(
                    "Unable to create or open shmem flink {:?} : {e}",
                    shmem_flink.as_ref()
                );
                Err(e.into())
            }
        }
    }

    pub fn open_shared<P: AsRef<std::path::Path>>(
        shmem_flink: P,
    ) -> Result<&'static Self, QueueError> {
        use shared_memory::ShmemConf;
        match ShmemConf::new()
            .flink(&shmem_flink)
            .open()
        {
            Ok(shmem) => {
                let ptr = shmem.as_ptr() as *mut QueueHeader;
                std::mem::forget(shmem);
                unsafe {
                    Self::shared(shmem_flink, (*ptr).n_elements(), (*ptr).queue_type)
                }
            }
            Err(e) => {
                eprintln!(
                    "Unable to create or open shmem flink {:?} : {e}",
                    shmem_flink.as_ref()
                );
                Err(e.into())
            }
        }
    }
}

/// Simply exists for the automatic produce_first
#[repr(C, align(64))]
pub struct Producer<'a, T> {
    // can't we just make this a usize since we're anyway padding?
    pub produced_first: u8, // 1
    pub queue:      &'a Queue<T>,
}

impl<'a, T: Copy> From<&'a Queue<T>> for Producer<'a, T> {
    fn from(queue: &'a Queue<T>) -> Self {
        Self {
            produced_first: 0,
            queue,
        }
    }
}

impl<'a, T: Copy> Producer<'a, T> {
    pub fn produce(&mut self, msg: &T) -> usize {
        // if self.produced_first == 0 {
        //     self.produced_first = 1;
        //     self.queue.produce_first(msg)
        // } else {
        self.queue.produce(msg)
        // }
    }
    pub fn produce_arg(&mut self, msg: &PublishArg) -> usize {
        // if self.produced_first == 0 {
        //     self.produced_first = 1;
        //     self.queue.produce_first_arg(msg)
        // } else {
        self.queue.produce_arg(msg)
        // }
    }
}

impl<'a, T> AsMut<Producer<'a, T>> for Producer<'a, T> {
    fn as_mut(&mut self) -> &mut Producer<'a, T> {
        self
    }
}

#[repr(C, align(64))]
#[derive(Debug)]
pub struct Consumer<'a, T> {
    /// Shared reference to the channel
    /// Read index pointer
    pos:              usize, // 8
    mask:             usize,        // 16
    expected_version: usize,        // 24
    is_running:       u8,           // 25
    _pad:             [u8; 7],      // 32
    queue:            &'a Queue<T>, // 48 fat ptr: (usize, pointer)
}

impl<'a, T: Copy> Consumer<'a, T> {
    pub fn recover_after_error(&mut self) {
        while self.queue.version_of(self.pos) > self.expected_version {
            self.update_pos()
        }
        self.expected_version += 2;
    }

    pub fn recover_after_error_dumb(&mut self) {
        self.expected_version += 2;
    }

    fn update_pos(&mut self) {
        self.pos = (self.pos + 1) & self.mask;
        self.expected_version += 2 * (self.pos == 0) as usize;
    }

    /// Nonblocking consume returning either Ok(()) or a ReadError
    pub fn try_consume(&mut self, el: &mut T) -> Result<(), ReadError> {
        self.queue.consume(el, self.pos, self.expected_version)?;
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

    pub fn init_header(consumer_ptr: *mut Consumer<T>, queue: &'static Queue<T>) {
        unsafe {
            (*consumer_ptr).pos = queue.cur_pos();
            (*consumer_ptr).expected_version = queue.version();
            (*consumer_ptr).mask = queue.header.mask;
            (*consumer_ptr).queue = queue
        }
    }
}

impl<'a, T> AsMut<Consumer<'a, T>> for Consumer<'a, T> {
    fn as_mut(&mut self) -> &mut Consumer<'a, T> {
        self
    }
}

impl<'a, T: Copy> From<&'a Queue<T>> for Consumer<'a, T> {
    fn from(queue: &'a Queue<T>) -> Self {
        let pos = queue.cur_pos();
        let expected_version = queue.version();
        Self {
            pos,
            mask: queue.header.mask,
            _pad: [0; 7],
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
    #[test]
    #[cfg(feature = "shmem")]
    fn basic_shared() {
        for typ in [QueueType::SPMC, QueueType::MPMC] {
            let path = std::path::Path::new("/dev/shm/blabla_test");
            let q = Queue::shared(&path, 16, typ).unwrap();
            let mut p = Producer::from(&*q);
            let mut c = Consumer::from(&*q);

            assert_eq!(q.next_count() & q.header.mask, c.pos);
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
            std::fs::remove_file(&path);
        }
    }
}
