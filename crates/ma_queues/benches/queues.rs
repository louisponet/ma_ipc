use std::{sync::{atomic::Ordering, self, Arc}, time::Duration};

use core_affinity::CoreId;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Bencher};

fn consume_bench<const N_BYTES:usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let q = ma_queues::Queue::new(4096, ma_queues::QueueType::SPMC).unwrap();
        let done = Arc::new(sync::atomic::AtomicBool::new(false));
        let done1 = done.clone();
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            let mut m = [0u8; N_BYTES];
            let mut c = 0u8;
            let mut lck = ma_queues::Producer::from(q);
            loop {
                m.fill(c);
                lck.produce(&m);
                c = c.wrapping_add(1);
                if done1.load(sync::atomic::Ordering::Relaxed) {
                    break;
                }
                // std::thread::yield_now();
            }
            m[0] = 1;
            for i in 1..N_BYTES {
                m[i] = 2;
            }
            lck.produce(&m);
        });
        for i in 1..n_contenders {
            let mut lock2 = ma_queues::Consumer::from(q);
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 2*i + 3});
                let mut m = [0u8; N_BYTES];
                loop {
                    lock2.consume(&mut m);
                    if m[0] == 1 && m[i] == 2 {
                        break;
                    }
                }
            });
        }

        std::thread::sleep(Duration::from_millis(10));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 3});
            let mut lock2 = ma_queues::Consumer::from(q);
            let mut m = [0u8; N_BYTES];
            b.iter(|| {
                lock2.consume(&mut m);
            });
            done.store(true, Ordering::Relaxed);
        });
    });
}

fn consume(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("consume_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, &size| match size {
                    8 => {
                        consume_bench::<8>(b, n_readers);
                    }
                    30 => {
                        consume_bench::<30>(b, n_readers);
                    }
                    32 => {
                        consume_bench::<32>(b, n_readers);
                    }
                    60 => {
                        consume_bench::<60>(b, n_readers);
                    }
                    124 => {
                        consume_bench::<124>(b, n_readers);
                    }
                    252 => {
                        consume_bench::<252>(b, n_readers);
                    }
                    508 => {
                        consume_bench::<508>(b, n_readers);
                    }
                    1020 => {
                        consume_bench::<1020>(b, n_readers);
                    }
                    2044 => {
                        consume_bench::<2044>(b, n_readers);
                    }
                    4092 => {
                        consume_bench::<4092>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}

fn produce_bench_spmc<const N_BYTES:usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let q = ma_queues::Queue::new(4096, ma_queues::QueueType::SPMC).unwrap();
        let done = Arc::new(sync::atomic::AtomicBool::new(false));
        for i in 0..n_contenders {
            let done1 = done.clone();
            let mut lock2 = ma_queues::Consumer::from(q);
            s.spawn(move || {
                core_affinity::set_for_current(CoreId { id: 2 + 2*i});
                let mut m = [0u8; N_BYTES];
                loop {
                    lock2.consume(&mut m);
                    std::thread::yield_now();
                    if m[0] == 1 && m[i] == 2 || done1.load(Ordering::Relaxed){
                        break;
                    }
                }
            });
        }

        std::thread::sleep(Duration::from_millis(10));
        s.spawn(move || {
            core_affinity::set_for_current(CoreId { id: 0});
            let mut lock2 = ma_queues::Producer::from(q);
            let mut m = [0u8; N_BYTES];
            b.iter(|| {
                lock2.produce(&mut m);
            });
            done.store(true, Ordering::Relaxed);
            m[0] = 1;
            for i in 1..N_BYTES {
                m[i] = 2;
            }
            lock2.produce(&m);
        });
    });
}

fn produce_spmc(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("produce_spmc_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, &size| match size {
                    8 => {
                        produce_bench_spmc::<8>(b, n_readers);
                    }
                    30 => {
                        produce_bench_spmc::<30>(b, n_readers);
                    }
                    32 => {
                        produce_bench_spmc::<32>(b, n_readers);
                    }
                    60 => {
                        produce_bench_spmc::<60>(b, n_readers);
                    }
                    124 => {
                        produce_bench_spmc::<124>(b, n_readers);
                    }
                    252 => {
                        produce_bench_spmc::<252>(b, n_readers);
                    }
                    508 => {
                        produce_bench_spmc::<508>(b, n_readers);
                    }
                    1020 => {
                        produce_bench_spmc::<1020>(b, n_readers);
                    }
                    2044 => {
                        produce_bench_spmc::<2044>(b, n_readers);
                    }
                    4092 => {
                        produce_bench_spmc::<4092>(b, n_readers);
                    }
                    _ => {}
                },
            );
        }
        group.finish();
    }
}

criterion_group! {
    name=queues;
    config=Criterion::default();
    targets = consume, produce_spmc
}
criterion_main!(queues);
