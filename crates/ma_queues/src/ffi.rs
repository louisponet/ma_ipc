use std::sync::atomic::Ordering::SeqCst;
use crate::{messages::*, Consumer, Producer, Queue, QueueError, QueueHeader, ReadError, vector::SeqlockVector};
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

impl From<QueueError> for FFIError {
    fn from(value: QueueError) -> Self {
        match value {
            QueueError::UnInitialized => Self::QueueUnInitialized,
            QueueError::LengthNotPowerOfTwo => Self::QueueLengthNotPowerTwo,
            QueueError::ElementSizeNotPowerTwo => Self::UnsupportedMessageSize,
        }
    }
}

impl<E: std::error::Error> From<Result<(), E>> for FFIError
where
    FFIError: From<E>,
{
    fn from(value: Result<(), E>) -> Self {
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
    len: u32,
    vectorsize_in_bytes: &mut usize,
) -> FFIError {

    match msgsize_bytes {
        60 => *vectorsize_in_bytes = SeqlockVector::<Message60>::size_of(len as usize),
        124 => *vectorsize_in_bytes = SeqlockVector::<Message124>::size_of(len as usize),
        252 => *vectorsize_in_bytes = SeqlockVector::<Message252>::size_of(len as usize),
        508 => *vectorsize_in_bytes = SeqlockVector::<Message508>::size_of(len as usize),
        1020 => *vectorsize_in_bytes = SeqlockVector::<Message1020>::size_of(len as usize),
        1660 => *vectorsize_in_bytes = SeqlockVector::<Message1660>::size_of(len as usize),
        2044 => *vectorsize_in_bytes = SeqlockVector::<Message2044>::size_of(len as usize),
        4092 => *vectorsize_in_bytes = SeqlockVector::<Message4092>::size_of(len as usize),
        7228 => *vectorsize_in_bytes = SeqlockVector::<Message7228>::size_of(len as usize),
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

#[no_mangle]
pub extern "C" fn InitSeqlockVector(
    ptr: *mut u8,
    msgsize_bytes: u32,
    len: u32,
) -> FFIError {
    match msgsize_bytes {
        60 => SeqlockVector::<Message60>::init_header(ptr, len as usize),
        124 => SeqlockVector::<Message124>::init_header(ptr, len as usize),
        252 => SeqlockVector::<Message252>::init_header(ptr, len as usize),
        508 => SeqlockVector::<Message508>::init_header(ptr, len as usize),
        1020 => SeqlockVector::<Message1020>::init_header(ptr, len as usize),
        1660 => SeqlockVector::<Message1660>::init_header(ptr, len as usize),
        2044 => SeqlockVector::<Message2044>::init_header(ptr, len as usize),
        4092 => SeqlockVector::<Message4092>::init_header(ptr, len as usize),
        7228 => SeqlockVector::<Message7228>::init_header(ptr, len as usize),
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

//Queues
#[no_mangle]
#[inline(always)]
pub extern "C" fn QueueSizeInBytes(
    msgsize_bytes: u32,
    len: u32,
    queuesize_in_bytes: &mut usize,
) -> FFIError {
    if !len.is_power_of_two() {
        return FFIError::QueueLengthNotPowerTwo;
    }

    match msgsize_bytes {
        60 => *queuesize_in_bytes = Queue::<Message60>::size_of(len),
        124 => *queuesize_in_bytes = Queue::<Message124>::size_of(len),
        252 => *queuesize_in_bytes = Queue::<Message252>::size_of(len),
        508 => *queuesize_in_bytes = Queue::<Message508>::size_of(len),
        1020 => *queuesize_in_bytes = Queue::<Message1020>::size_of(len),
        2044 => *queuesize_in_bytes = Queue::<Message2044>::size_of(len),
        4092 => *queuesize_in_bytes = Queue::<Message4092>::size_of(len),
        _ => return FFIError::UnsupportedMessageSize,
    }
    FFIError::Success
}

#[no_mangle]
#[inline(always)]
pub extern "C" fn InitQueue(
    ptr: *mut u8,
    queue_type: crate::QueueType,
    msgsize_bytes: u32,
    len: u32,
) -> FFIError {
    match msgsize_bytes {
        60 => Queue::<Message60>::init_header(ptr, queue_type, len).into(),
        124 => Queue::<Message124>::init_header(ptr, queue_type, len).into(),
        252 => Queue::<Message252>::init_header(ptr, queue_type, len).into(),
        508 => Queue::<Message508>::init_header(ptr, queue_type, len).into(),
        1020 => Queue::<Message1020>::init_header(ptr, queue_type, len).into(),
        2044 => Queue::<Message2044>::init_header(ptr, queue_type, len).into(),
        4092 => Queue::<Message4092>::init_header(ptr, queue_type, len).into(),
        _ => return FFIError::UnsupportedMessageSize,
    }
}
use ffi_msg::ffi_msg;
ffi_msg!(60);
ffi_msg!(124);
//ffi_msg!(252);
//ffi_msg!(508);
//ffi_msg!(1020);
//ffi_msg!(2044);
ffi_msg!(1660);
ffi_msg!(4092);
ffi_msg!(7228);