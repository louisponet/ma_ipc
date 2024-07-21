use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, PartialEq)]
pub enum ReadError {
    #[error("Got sped past")]
    SpedPast,
    #[error("Lock empty")]
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
pub mod seqlock;
pub mod vector;
pub mod queue;

pub use queue::{Queue, Producer, Consumer, QueueType};
pub use vector::{SeqlockVector};
