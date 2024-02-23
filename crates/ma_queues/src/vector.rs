use std::alloc::Layout;
use crate::versioned_lock::*;

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
    buffer: [VersionedLock<T>],
}
impl<T: Copy> SeqlockVector<T> {
    pub fn new(len: usize) -> &'static Self {
        // because we don't need len to be power of 2
        let size = std::mem::size_of::<VectorHeader>()
            + len * std::mem::size_of::<VersionedLock<T>>();

        let v;

        unsafe {
            let ptr = std::alloc::alloc_zeroed(
                Layout::array::<u8>(size)
                    .unwrap()
                    .align_to(64)
                    .unwrap()
                    .pad_to_align(),
            );
            // why len? because the size in the fat pointer ONLY cares about the unsized part of the struct
            // i.e. the length of the buffer
            v = &mut *(std::ptr::slice_from_raw_parts_mut(ptr, len) as *mut SeqlockVector<T>);
        }

        v.init(len);
        v
    }
    fn init(&mut self, len: usize,) {
        let elsize = std::mem::size_of::<VersionedLock<T>>();
        self.header.bufsize = len;
        self.header.elsize = elsize;
    }

    pub fn init_header(ptr: *mut u8, len: usize)  {
        let elsize = std::mem::size_of::<VersionedLock<T>>();

        unsafe {
            let s = std::slice::from_raw_parts_mut(ptr, 128);
            let header = &mut *(s as *mut _ as *mut VectorHeader) as &mut VectorHeader;

            header.bufsize = len;
            header.elsize = elsize;
        }
    }
    pub const fn size_of(len: usize) -> usize {
        std::mem::size_of::<VectorHeader>()
            + len * std::mem::size_of::<VersionedLock<T>>()
    }

    pub fn from_ptr(ptr: *mut VectorHeader) -> &'static Self {
        unsafe {
            let len = (*ptr).bufsize;
            &*(std::ptr::slice_from_raw_parts_mut(ptr, Self::size_of(len)) as *const SeqlockVector<T>)
        }
    }
    //TODO: ErrorHandling
    fn load(&self, pos: usize) -> &VersionedLock<T> {
        unsafe { self.buffer.get_unchecked(pos) }
    }

    pub fn write(&self, pos: usize, item: &T) {
        let lock = self.load(pos);
        lock.write(item);
    }

    pub fn read(&self, pos: usize, result: &mut T) {
        let lock = self.load(pos);
        lock.read_no_ver(result);
    }

    pub fn len(&self) -> usize {
        self.header.bufsize as usize
    }
}
impl<T: Clone + std::fmt::Debug> std::fmt::Debug for SeqlockVector<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SeqlockVector:\nHeader:\n{:?}", self.header)
    }
}
