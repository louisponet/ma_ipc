use std::{sync::{atomic::Ordering, self, Arc}, arch::x86_64::__rdtscp, time::Duration};

use core_affinity::CoreId;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Bencher};
fn write_bench<const N_BYTES:usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let lock =
            Arc::new(ma_queues::versioned_lock::VersionedLock::new([0u8; N_BYTES]));
        for i in 0..n_contenders {
            let lock2 = lock.clone();
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 2*i + 3});
                let mut m = [0u8; N_BYTES];
                loop {
                    lock2.read_no_ver(&mut m);
                    if m[0] == 1 && m[i+1] == 2 {
                        break;
                    }
                }
            });
        }
        std::thread::sleep(Duration::from_millis(1));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            let mut m = [0u8; N_BYTES];
            let mut c = 0u8;
            b.iter(|| {
                c = c.wrapping_add(1);
                m[0] = c;
                m[1] = c;
                lock.write(&m)
            });
            m[0] = 1;
            for i in 1..N_BYTES {
                m[i] = 2;
            }
            lock.write(&m);
        });
    });
}

fn write(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("write_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, &size| match size {
                    8 => {
                        write_bench::<8>(b, n_readers);
                    }
                    30 => {
                        write_bench::<30>(b, n_readers);
                    }
                    32 => {
                        write_bench::<32>(b, n_readers);
                    }
                    60 => {
                        write_bench::<60>(b, n_readers);
                    }
                    124 => {
                        write_bench::<124>(b, n_readers);
                    }
                    252 => {
                        write_bench::<252>(b, n_readers);
                    }
                    508 => {
                        write_bench::<508>(b, n_readers);
                    }
                    1020 => {
                        write_bench::<1020>(b, n_readers);
                    }
                    2044 => {
                        write_bench::<2044>(b, n_readers);
                    }
                    4092 => {
                        write_bench::<4092>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}

fn read_into_bench<const N_BYTES:usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let lock =
            Arc::new(ma_queues::versioned_lock::VersionedLock::new([0u8; N_BYTES]));
        let done = Arc::new(sync::atomic::AtomicBool::new(false));
        let done1 = done.clone();
        let lock1 = lock.clone();
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            let mut m = [0u8; N_BYTES];
            let mut c = 0u8;
            let lck = lock1.as_ref();
            loop {
                m.fill(c);
                lck.write(&m);
                c = c.wrapping_add(1);
                if done1.load(sync::atomic::Ordering::Relaxed) {
                    break;
                }
                std::thread::yield_now();
            }
            m[0] = 1;
            for i in 1..N_BYTES {
                m[i] = 2;
            }
            lck.write(&m);
        });
        for i in 1..n_contenders {
            let lock2 = lock.clone();
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 2*i + 3});
                let mut m = [0u8; N_BYTES];
                loop {
                    lock2.read_no_ver(&mut m);
                    if m[0] == 1 && m[i] == 2 {
                        break;
                    }
                }
            });
        }

        let lock2 = lock.clone();
        std::thread::sleep(Duration::from_millis(10));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 3});
            let lck = lock2.as_ref();
            let mut m = [0u8; N_BYTES];
            b.iter(|| {
                lck.read_no_ver(&mut m);
            });
            done.store(true, Ordering::Relaxed);
        });
    });
}

fn read_into(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("read_into_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, &size| match size {
                    8 => {
                        read_into_bench::<8>(b, n_readers);
                    }
                    30 => {
                        read_into_bench::<30>(b, n_readers);
                    }
                    32 => {
                        read_into_bench::<32>(b, n_readers);
                    }
                    60 => {
                        read_into_bench::<60>(b, n_readers);
                    }
                    124 => {
                        read_into_bench::<124>(b, n_readers);
                    }
                    252 => {
                        read_into_bench::<252>(b, n_readers);
                    }
                    508 => {
                        read_into_bench::<508>(b, n_readers);
                    }
                    1020 => {
                        read_into_bench::<1020>(b, n_readers);
                    }
                    2044 => {
                        read_into_bench::<2044>(b, n_readers);
                    }
                    4092 => {
                        read_into_bench::<4092>(b, n_readers);
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
    rdtsc: u64,
    data: [u8; N]
}
fn rdtscp() -> u64 {
    unsafe { __rdtscp(&mut 0u32 as *mut _) }
}

fn latency_bench<const N_BYTES:usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let clock = quanta::Clock::new();
        clock.now();

        let lock =
            Arc::new(ma_queues::versioned_lock::VersionedLock::new(TimingMessage{rdtsc:clock.raw(), data:[0u8; N_BYTES]}));
        let done = Arc::new(sync::atomic::AtomicBool::new(false));
        let done1 = done.clone();
        let lock1 = lock.clone();
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            let mut m = TimingMessage{rdtsc:0, data:[0u8; N_BYTES]};
            let mut c = 0u8;
            let lck = lock1.as_ref();
            loop {
                m.data.fill(c);
                m.rdtsc = rdtscp();
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
                core_affinity::set_for_current(CoreId { id: 2*i + 3});
                let mut m = TimingMessage{rdtsc: 0, data:[0u8; N_BYTES]};
                loop {
                    lock2.read_no_ver(&mut m);
                    if m.data[0] == 1 && m.data[i] == 2 {
                        break;
                    }
                }
            });
        }

        let lock2 = lock.clone();
        std::thread::sleep(Duration::from_millis(10));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 3});
            let lck = lock2.as_ref();
            let mut m = TimingMessage{rdtsc: 0, data:[0u8; N_BYTES]};
            let mut curver = 0;
            b.iter_custom(|iters| {
                let mut avg_lat = 0;
                for i in 0..iters{
                    loop {
                        let newver = lck.read_no_ver(&mut m);
                        let now = rdtscp();
                        if newver != curver {
                            curver = newver;
                            avg_lat += now - m.rdtsc;
                            break;
                        }
                    }
                }
                Duration::from_nanos(clock.delta_as_nanos(0, avg_lat))
            });
            done.store(true, Ordering::Relaxed);
        });
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
    name=versioned_lock;
    config=Criterion::default();
    targets = write, read_into, latency, 
}
criterion_main!(versioned_lock);
