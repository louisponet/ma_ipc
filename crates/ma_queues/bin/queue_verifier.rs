use ma_queues::{seqlock::SeqLock, Consumer, Queue, QueueError, QueueHeader};
use std::env;
struct GenericQueue {
    header: QueueHeader,
    buffer: [u8],
}

impl GenericQueue {
    #[allow(dead_code)]
    fn from_initialized_ptr(ptr: *mut QueueHeader) -> Result<&'static Self, QueueError> {
        unsafe {
            let len = (*ptr).n_elements();
            if !len.is_power_of_two() {
                return Err(QueueError::LengthNotPowerOfTwo);
            }

            if (*ptr).is_initialized != true as u8 {
                return Err(QueueError::UnInitialized);
            }

            println!("sizeof {}, elsize {}", (*ptr).sizeof(),(*ptr).elsize);
            Ok(&*(std::ptr::slice_from_raw_parts_mut(ptr, (*ptr).sizeof()) as *const Self))
        }
    }

    pub fn shared<P: AsRef<std::path::Path>>(
        shmem_flink: P,
    ) -> Result<&'static Self, QueueError> {
        use shared_memory::{ShmemConf, ShmemError};
        match ShmemConf::new()
            .size(std::mem::size_of::<QueueHeader>())
            .flink(&shmem_flink)
            .open()
        {
            Ok(shmem) => {
                let ptr = shmem.as_ptr();
                std::mem::forget(shmem);
                Self::from_initialized_ptr(QueueHeader::from_ptr(ptr) as *mut _)
            }
            Err(e) => {
                eprintln!(
                    "Unable to open queue at {:?} : {e}",
                    shmem_flink.as_ref()
                );
                Err(e.into())
            }
        }
    }

    fn version(&self, pos: usize) -> usize {
        let start = pos * self.header.elsize as usize;
        usize::from_be_bytes(self.buffer[start..start + 8].try_into().unwrap())
    }

    pub fn verify(&self) {

        let mut prev_v = self.version(0);
        let mut n_changes = 0;

        for i in 1..self.header.n_elements() {
            let v = self.version(i);
            if v & 1 == 1 {
                panic!("odd version at {i}: {prev_v} -> {v}");
            }
            if v != prev_v && v & 1 == 0 {
                n_changes += 1;
                println!("version change at {i}: {prev_v} -> {v}");
                prev_v = v;
            }
        }
        if n_changes > 1 {
            panic!("what")
        }
    }

}


#[derive(Debug, Copy, Clone, PartialEq,  Default)]
pub struct Timestamp {
    pub ingestion_t: u64, //16
    pub exchange_t:  u64, //24
}

#[derive(Debug, Clone, Copy, Default)]
pub struct L2Update {
    pub timestamp:      Timestamp,
    pub instrument_ids: u64,
    pub flags:          u8,
    pub price:          f64,
    pub volume:         f64,
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(u8)]
pub enum QueueMessage<T> {
    Data((u64, u64), T),
    TimeUpdate(u32, Timestamp),
    #[default]
    Stop,
}

fn main() {
    let q = Queue::<QueueMessage<L2Update>>::open_shared("/dev/shm/mantra/queues/L2Update").unwrap();
    let mut c = Consumer::from(q);
    for i in 0..131072 {
        eprintln!("{:?}", q.version_of(i));
    }

    // let mut m = Default::default();
    // eprintln!("{:?}", c.try_consume(&mut m));
    // let args: Vec<String> = env::args().collect();
    // if args.len() != 2 {
    //     eprintln!("please supply the path as argument");
    //     return;
    // }

    // let p = std::path::Path::new(&args[1]);

    // if !p.exists() {
    //     eprintln!("{p:?} does not exist");
    //     return;
    // }
    // let q = GenericQueue::shared(p).unwrap();
    // q.verify();
}



