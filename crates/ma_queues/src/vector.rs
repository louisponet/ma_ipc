use std::{alloc::Layout, mem::MaybeUninit, ops::Index};
use crate::seqlock::*;

#[derive(Debug)]
#[repr(C)]
pub struct VectorHeader {
    elsize: usize,
    bufsize: usize
}
// TODO: Multi producer writing, in versionedlock basically just add write safe using compare and swap
/// This is a seqlocked vector, should only be written to by one producer
#[repr(C, align(64))]
pub struct SeqlockVector<T> {
    header: VectorHeader,
    buffer: [SeqLock<T>],
}
impl<T: Copy> SeqlockVector<T> {
    pub fn new(len: usize) -> &'static Self {
        // because we don't need len to be power of 2
        let size = std::mem::size_of::<VectorHeader>()
            + len * std::mem::size_of::<SeqLock<T>>();

        unsafe {
            let ptr = std::alloc::alloc_zeroed(
                Layout::array::<u8>(size)
                    .unwrap()
                    .align_to(64)
                    .unwrap()
                    .pad_to_align(),
            );
            Self::from_uninitialized_ptr(ptr, len)
        }
    }

    pub const fn size_of(len: usize) -> usize {
        std::mem::size_of::<VectorHeader>()
            + len * std::mem::size_of::<SeqLock<T>>()
    }

    pub fn from_uninitialized_ptr(
        ptr: *mut u8,
        len: usize,
    ) -> &'static Self {
        unsafe {
            // why len? because the size in the fat pointer ONLY cares about the unsized part of the struct
            // i.e. the length of the buffer
            let q = &mut *(std::ptr::slice_from_raw_parts_mut(ptr, len) as *mut SeqlockVector<T>);
            let elsize = std::mem::size_of::<SeqLock<T>>();
            q.header.bufsize = len;
            q.header.elsize = elsize;
            q
        }
    }

    #[allow(dead_code)]
    fn from_initialized_ptr(ptr: *mut VectorHeader) -> &'static mut Self {
        unsafe {
            let len = (*ptr).bufsize;
            &mut *(std::ptr::slice_from_raw_parts_mut(ptr, len) as *mut SeqlockVector<T>)
        }
    }

    //TODO: ErrorHandling
    fn load(&self, pos: usize) -> &SeqLock<T> {
        unsafe { self.buffer.get_unchecked(pos) }
    }

    pub fn write(&self, pos: usize, item: &T) {
        let lock = self.load(pos);
        lock.write(item);
    }

    pub fn write_multi(&self, pos: usize, item: &T) {
        let lock = self.load(pos);
        lock.write_multi(item);
    }

    pub fn read(&self, pos: usize, result: &mut T) {
        let lock = self.load(pos);
        lock.read_no_ver(result);
    }

    pub fn read_copy(&self, pos: usize) -> T {
        let mut out = unsafe {MaybeUninit::uninit().assume_init()};
        let lock = self.load(pos);
        lock.read_no_ver(&mut out);
        out
    }

    pub fn len(&self) -> usize {
        self.header.bufsize as usize
    }
    pub fn iter(&self) -> VectorIterator<'_, T> {
        VectorIterator{vector: self, next_id: 0}
    }
}

#[cfg(feature = "shmem")]
impl<T: Copy> SeqlockVector<T> {
    pub fn shared<P: AsRef<std::path::Path>>(
        shmem_flink: P,
        len: usize,
    ) -> Result<&'static Self, &'static str> {
        use shared_memory::{ShmemConf, ShmemError};
        match ShmemConf::new()
            .size(Self::size_of(len))
            .flink(&shmem_flink)
            .create()
        {
            Ok(shmem) => {
                let ptr = shmem.as_ptr();
                std::mem::forget(shmem);
                Ok(Self::from_uninitialized_ptr(ptr, len))
            }
            Err(ShmemError::LinkExists) => {
                let shmem = ShmemConf::new().flink(shmem_flink).open().unwrap();
                let ptr = shmem.as_ptr() as *mut VectorHeader;
                std::mem::forget(shmem);
                let v = Self::from_initialized_ptr(ptr);
                if v.header.bufsize < len {
                    Err("Existing shmem too small")
                } else {
                    v.header.bufsize = len;
                    Ok(v)
                }
            }
            Err(_) => {
                Err("Unable to create or open shmem flink.")
            }
        }
    }
}
impl<T: Clone + std::fmt::Debug> std::fmt::Debug for SeqlockVector<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SeqlockVector:\nHeader:\n{:?}", self.header)
    }
}

pub struct VectorIterator<'a, T> {
    vector: &'a SeqlockVector<T>,
    next_id: usize
}

impl<'a, T: Copy + Clone> Iterator for VectorIterator<'a, T>
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.next_id >= self.vector.len() {
            None
        } else {
            let out = self.vector.read_copy(self.next_id);
            self.next_id = self.next_id.wrapping_add(1);
            Some(out)
        }

}}
