use ma_queues::queue::QueueType;
use ma_queues::vector::SeqlockVector;
use ma_queues::{
    queue::{Consumer, Producer, Queue, QueueHeader},
    QueueError, ReadError,
};
use thiserror::Error;

#[derive(Error, Debug)]
#[repr(u32)]
pub enum FFIError {
    #[error("Success")]
    Success = 0,
    // Qeueu Creation errors
    #[error("Unsupported message size")]
    UnsupportedMessageSize,
    #[error("Queue length is not power of two")]
    QueueLengthNotPowerTwo,
    #[error("Queue was not initialized. Use InitQueue")]
    QueueUnInitialized,
    // ReadErrors
    #[error("ReadError: Queue is empty")]
    QueueEmpty,
    #[error("ReadError: Got sped past")]
    SpedPast,
}

impl From<ReadError> for FFIError {
    fn from(value: ReadError) -> Self {
        match value {
            ReadError::SpedPast => Self::SpedPast,
            ReadError::Empty => Self::QueueEmpty,
        }
    }
}
impl From<()> for FFIError {
    fn from(value: ()) -> Self {
        Self::Success
    }
}

impl From<QueueError> for FFIError {
    fn from(value: QueueError) -> Self {
        match value {
            QueueError::UnInitialized => Self::QueueUnInitialized,
            QueueError::LengthNotPowerOfTwo => Self::QueueLengthNotPowerTwo,
            QueueError::ElementSizeNotPowerTwo => Self::UnsupportedMessageSize,
            QueueError::SharedMemoryError(_) => todo!(),
        }
    }
}

impl<E: std::error::Error, T> From<Result<&'static Queue<T>, E>> for FFIError
where
    FFIError: From<E>,
{
    fn from(value: Result<&'static Queue<T>, E>) -> Self {
        match value {
            Ok(_) => FFIError::Success,
            Err(e) => FFIError::from(e),
        }
    }
}

//Vector
#[no_mangle]
pub extern "C" fn SeqlockVectorSizeInBytes(
    msgsize_bytes: u32,
    len: usize,
    vectorsize_in_bytes: &mut usize,
) -> FFIError {
    match msgsize_bytes {
        56 => *vectorsize_in_bytes = SeqlockVector::<[u8; 56]>::size_of(len),
        120 => *vectorsize_in_bytes = SeqlockVector::<[u8; 120]>::size_of(len),
        248 => *vectorsize_in_bytes = SeqlockVector::<[u8; 248]>::size_of(len),
        504 => *vectorsize_in_bytes = SeqlockVector::<[u8; 504]>::size_of(len),
        1016 => *vectorsize_in_bytes = SeqlockVector::<[u8; 1016]>::size_of(len),
        1656 => *vectorsize_in_bytes = SeqlockVector::<[u8; 1656]>::size_of(len),
        2040 => *vectorsize_in_bytes = SeqlockVector::<[u8; 2040]>::size_of(len),
        4088 => *vectorsize_in_bytes = SeqlockVector::<[u8; 4088]>::size_of(len),
        7224 => *vectorsize_in_bytes = SeqlockVector::<[u8; 7224]>::size_of(len),
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

#[no_mangle]
pub extern "C" fn seqlockvector_size_in_bytes(ptr: *mut u8, msgsize_bytes: u32, len: usize) -> FFIError {
    match msgsize_bytes {
        56 => {
            SeqlockVector::<[u8; 56]>::from_uninitialized_ptr(ptr, len);
        }
        120 => {
            SeqlockVector::<[u8; 120]>::from_uninitialized_ptr(ptr, len);
        }
        248 => {
            SeqlockVector::<[u8; 248]>::from_uninitialized_ptr(ptr, len);
        }
        504 => {
            SeqlockVector::<[u8; 504]>::from_uninitialized_ptr(ptr, len);
        }
        1016 => {
            SeqlockVector::<[u8; 1016]>::from_uninitialized_ptr(ptr, len);
        }
        1656 => {
            SeqlockVector::<[u8; 1656]>::from_uninitialized_ptr(ptr, len);
        }
        2040 => {
            SeqlockVector::<[u8; 2040]>::from_uninitialized_ptr(ptr, len);
        }
        4088 => {
            SeqlockVector::<[u8; 4088]>::from_uninitialized_ptr(ptr, len);
        }
        7224 => {
            SeqlockVector::<[u8; 7224]>::from_uninitialized_ptr(ptr, len);
        }
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

//Queues
#[no_mangle]
#[inline(always)]
pub extern "C" fn queue_size_in_bytes(msgsize_bytes: u32, len: usize, queuesize_in_bytes: &mut usize) -> FFIError {
    if !len.is_power_of_two() {
        return FFIError::QueueLengthNotPowerTwo;
    }

    match msgsize_bytes {
        56 => *queuesize_in_bytes = Queue::<[u8; 56]>::size_of(len),
        120 => *queuesize_in_bytes = Queue::<[u8; 120]>::size_of(len),
        248 => *queuesize_in_bytes = Queue::<[u8; 248]>::size_of(len),
        504 => *queuesize_in_bytes = Queue::<[u8; 504]>::size_of(len),
        1016 => *queuesize_in_bytes = Queue::<[u8; 1016]>::size_of(len),
        2040 => *queuesize_in_bytes = Queue::<[u8; 2040]>::size_of(len),
        4088 => *queuesize_in_bytes = Queue::<[u8; 4088]>::size_of(len),
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

#[no_mangle]
#[inline(always)]
pub extern "C" fn InitQueue(ptr: *mut u8, queue_type: QueueType, msgsize_bytes: u32, len: usize) -> FFIError {
    match msgsize_bytes {
        56 => Queue::<[u8; 56]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        120 => Queue::<[u8; 120]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        248 => Queue::<[u8; 248]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        504 => Queue::<[u8; 504]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        1016 => Queue::<[u8; 1016]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        2040 => Queue::<[u8; 2040]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        4088 => Queue::<[u8; 4088]>::from_uninitialized_ptr(ptr, len, queue_type).into(),
        _ => return FFIError::UnsupportedMessageSize,
    }
}
use ma_ffi_macro::ffi_msg;
ffi_msg!(56);
ffi_msg!(120);
//ffi_msg!(252);
//ffi_msg!(508);
//ffi_msg!(1020);
//ffi_msg!(2044);
ffi_msg!(1656);
ffi_msg!(4088);
ffi_msg!(7224);
