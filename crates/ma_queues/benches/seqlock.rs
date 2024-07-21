use std::{
    arch::x86_64::__rdtscp,
    sync::{self, atomic::{AtomicUsize, Ordering}, Arc},
    time::Duration,
};

use core_affinity::CoreId;
use criterion::{criterion_group, criterion_main, Bencher, BenchmarkId, Criterion, SamplingMode};
use ma_queues::seqlock::Seqlock;

#[derive(Debug, Clone, Copy)]
struct Message<const N: usize> {
    data: [u8; N]
}

impl<const N: usize> Default for Message<N> {
    fn default() -> Self {
        Self { data: [0; N]}
    }
}

fn write_bench<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let lock = Arc::new(Seqlock::default());
        for i in 0..n_contenders {
            let lock2 = lock.clone();
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 2 * i + 3 });
                let mut m = Message::<N_BYTES>::default();
                loop {
                    lock2.read_no_ver(&mut m);
                    if m.data[0] == 1 && m.data[1] == 2 {
                        break;
                    }
                }
            });
        }
        std::thread::sleep(Duration::from_millis(1));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            let mut m = Message::<N_BYTES>::default();
            let mut c = 0u8;
            b.iter(|| {
                c = c.wrapping_add(1);
                m.data[0] = c;
                m.data[1] = c;
                lock.write(&m)
            });
            m.data[0] = 1;
            for i in 1..N_BYTES {
                m.data[i] = 2;
            }
            lock.write(&m);
        });
    });
}

fn write(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("write_{}_readers", n_readers));
        for p in 4..=12 {
            let size = 2usize.pow(p);
            group.throughput(criterion::Throughput::Bytes(size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &size,
                |b, &size| match size {
                    16 => {
                        write_bench::<2>(b, n_readers);
                    }
                    32 => {
                        write_bench::<4>(b, n_readers);
                    }
                    64 => {
                        write_bench::<8>(b, n_readers);
                    }
                    128 => {
                        write_bench::<16>(b, n_readers);
                    }
                    256 => {
                        write_bench::<32>(b, n_readers);
                    }
                    512 => {
                        write_bench::<64>(b, n_readers);
                    }
                    1024 => {
                        write_bench::<128>(b, n_readers);
                    }
                    2048 => {
                        write_bench::<256>(b, n_readers);
                    }
                    4096 => {
                        write_bench::<512>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}

fn read_bench<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {

    b.iter_custom(|iters| {
        std::thread::scope(|s| {
            let clock = quanta::Clock::new();
            clock.now();
            let lock = Arc::new(Seqlock::default());
            let done = Arc::new(sync::atomic::AtomicBool::new(false));
            let done1 = done.clone();
            let lock1 = lock.clone();
            let lock2 = lock.clone();
            for i in 1..n_contenders {
                let lock2 = lock.clone();
                s.spawn(move || {
                    core_affinity::set_for_current(CoreId { id: 2 * i + 3 });
                    let mut m = Message::<N_BYTES>::default();
                    loop {
                        lock2.read_no_ver(&mut m);
                        if m.data[0] == 1 && m.data[1] == 2 {
                            break;
                        }
                    }
                });
            }
            let out = s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 3 });
                let lck = lock2.as_ref();
                let mut m = Message::<N_BYTES>::default();
                let mut avg_lat = 0;
                let mut last = 0;
                for _ in 0..iters {
                    loop {
                        let curt = rdtscp();
                        lck.read_no_ver(&mut m);
                        let delta = rdtscp() - curt;
                        if m.data[0] != last {
                            last = m.data[0];
                            avg_lat += delta;
                            break;
                        }
                    }
                }
                done.store(true, Ordering::Relaxed);
                Duration::from_nanos(avg_lat*10/33)
            });
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 1 });
                let mut m = Message::<N_BYTES>::default();
                let mut c = 0u8;
                let lck = lock1.as_ref();
                loop {
                    lck.write(&m);
                    let last_write = rdtscp();
                    c = c.wrapping_add(1);
                    m.data.fill(c);
                    if done1.load(sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    while rdtscp() - last_write < 330 {}
                }
                m.data[0] = 1;
                m.data[1] = 2;
                lck.write(&m);
            });
            out.join().unwrap()
        })
    });
}

fn read(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("read_{}_readers", n_readers));
        for p in 4..=12 {
            let size = 2usize.pow(p);
            group.throughput(criterion::Throughput::Bytes(size as u64));
            // group.sampling_mode(SamplingMode::Flat);
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &size,
                |b, &size| match size {
                    16 => {
                        read_bench::<2>(b, n_readers);
                    }
                    32 => {
                        read_bench::<4>(b, n_readers);
                    }
                    64 => {
                        read_bench::<8>(b, n_readers);
                    }
                    128 => {
                        read_bench::<16>(b, n_readers);
                    }
                    256 => {
                        read_bench::<32>(b, n_readers);
                    }
                    512 => {
                        read_bench::<64>(b, n_readers);
                    }
                    1024 => {
                        read_bench::<128>(b, n_readers);
                    }
                    2048 => {
                        read_bench::<256>(b, n_readers);
                    }
                    4096 => {
                        read_bench::<512>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}

#[derive(Clone, Copy)]
struct TimingMessage<const N: usize> {
    rdtscp: u64,
    data: [u8; N],
}
fn rdtscp() -> u64 {
    unsafe { __rdtscp(&mut 0u32 as *mut _) }
}
impl<const N: usize> Default for TimingMessage<N> {
    fn default() -> Self {
        Self {
            rdtscp: 0,
            data: [0; N],
        }
    }
}

fn latency_bench<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {
    b.iter_custom(|iters| {
        std::thread::scope(|s| {
            let clock = quanta::Clock::new();
            clock.now();

            let lock = Arc::new(Seqlock::default());
            let done = Arc::new(sync::atomic::AtomicBool::new(false));
            let done1 = done.clone();
            let lock1 = lock.clone();
            let lock2 = lock.clone();
            let out = s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 3 });
                let lck = lock2.as_ref();
                let mut m = TimingMessage {
                    rdtscp: 0,
                    data: [0u8; N_BYTES],
                };
                let mut last_t = 0;
                let mut avg_lat = 0;
                for i in 0..iters {
                    loop {
                        lck.read_no_ver(&mut m);
                        let now = rdtscp();
                        if m.rdtscp != last_t {
                            last_t = m.rdtscp;
                            avg_lat += now - m.rdtscp;
                            break;
                        }
                    }
                }
                Duration::from_nanos(clock.delta_as_nanos(0, avg_lat))
            });
            done.store(true, Ordering::Relaxed);
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 1 });
                let mut m = TimingMessage {
                    rdtscp: 0,
                    data: [0u8; N_BYTES],
                };
                let mut c = 0u8;
                let lck = lock1.as_ref();
                loop {
                    m.data.fill(c);
                    m.rdtscp = rdtscp();
                    lck.write(&m);
                    c = c.wrapping_add(1);
                    if done1.load(sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    // std::thread::sleep(Duration::from_micros(2));
                }
                m.data[0] = 1;
                for i in 1..N_BYTES {
                    m.data[i] = 2;
                }
                lck.write(&m);
            });

            for i in 1..n_contenders {
                let lock2 = lock.clone();
                s.spawn(move || {
                    core_affinity::set_for_current(CoreId { id: 2 * i + 3 });
                    let mut m = TimingMessage {
                        rdtscp: 0,
                        data: [0u8; N_BYTES],
                    };
                    loop {
                        lock2.read_no_ver(&mut m);
                        if m.data[0] == 1 && m.data[i] == 2 {
                            break;
                        }
                    }
                });
            }
            out.join()
        })
        .unwrap()
    });
}

fn latency(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("latency_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, &size| match size {
                    8 => {
                        latency_bench::<8>(b, n_readers);
                    }
                    30 => {
                        latency_bench::<30>(b, n_readers);
                    }
                    32 => {
                        latency_bench::<32>(b, n_readers);
                    }
                    60 => {
                        latency_bench::<60>(b, n_readers);
                    }
                    124 => {
                        latency_bench::<124>(b, n_readers);
                    }
                    252 => {
                        latency_bench::<252>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}
criterion_group! {
    name=seqlock;
    config=Criterion::default().sample_size(2000).measurement_time(std::time::Duration::from_secs(10));
    targets = write, read, latency
}
criterion_main!(seqlock);
