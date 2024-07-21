use std::{
    arch::x86_64::_mm_lfence,
    sync::{
        self,
        atomic::{compiler_fence, AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    time::Duration as StdDuration,
};

use core_affinity::CoreId;
use criterion::{criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use ma_time::{Duration, Instant, Nanos};

fn consume_bench<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let q = ma_queues::Queue::new(4096, ma_queues::QueueType::SPMC).unwrap();
        let done = Arc::new(AtomicBool::new(false));
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
                 core_affinity::set_for_current(CoreId { id: 2 * i + 3 });
                 let mut m = [0u8; N_BYTES];
                 loop {
                     lock2.consume(&mut m);
                     if m[0] == 1 && m[i] == 2 {
                         break;
                     }
                 }
             });
        }

        std::thread::sleep(StdDuration::from_millis(10));
        s.spawn(move || {
             core_affinity::set_for_current(CoreId { id: 3 });
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
            group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| match size {
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
                 });
        }
        group.finish();
    }
}
// this is max contention
fn produce_bench_spmc<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let q = ma_queues::Queue::new(4096, ma_queues::QueueType::SPMC).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        for i in 0..n_contenders {
            let done1 = done.clone();
            let mut lock2 = ma_queues::Consumer::from(q);
            s.spawn(move || {
                 core_affinity::set_for_current(CoreId { id: 2 + 2 * i });
                 let mut m = [0u8; N_BYTES];
                 loop {
                     lock2.consume(&mut m);
                     // std::thread::yield_now();
                     if m[0] == 1 && m[i] == 2 || done1.load(Ordering::Relaxed) {
                         break;
                     }
                 }
             });
        }

        std::thread::sleep(StdDuration::from_millis(10));
        s.spawn(move || {
             core_affinity::set_for_current(CoreId { id: 0 });
             let mut lock2 = ma_queues::Producer::from(q);
             let mut m = [0u8; N_BYTES];
             b.iter_custom(|b| {
                  let mut tot = StdDuration::ZERO;
                  for _ in 0..b {
                      std::thread::yield_now(); //just for a bit of delaying vs readers
                      let t = std::time::Instant::now();
                      lock2.produce(&mut m);
                      tot += t.elapsed();
                  }
                  tot
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
            group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| match size {
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
                 });
        }
        group.finish();
    }
}

#[derive(Clone, Copy, Debug)]
struct LatencyMsg<const N: usize> {
    tstamp: Instant,
    msg:    [u8; N],
}

impl<const N: usize> Default for LatencyMsg<N> {
    fn default() -> Self {
        Self { tstamp: Instant::now(), msg: [0; N] }
    }
}

fn consume_latency_bench<const N_BYTES: usize>(b: &mut Bencher, n_contenders: usize) {
    std::thread::scope(|s| {
        let q = ma_queues::Queue::new(4096, ma_queues::QueueType::SPMC).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        for i in 1..n_contenders {
            let mut lock2 = ma_queues::Consumer::from(q);
            s.spawn(move || {
                 core_affinity::set_for_current(CoreId { id: 2 * i + 3 });
                 let mut m = LatencyMsg::<N_BYTES>::default();
                 loop {
                     lock2.consume(&mut m);
                     if m.msg[0] == 1 && m.msg[i] == 2 {
                         break;
                     }
                 }
             });
        }
        b.iter_custom(|n_iters| {
            let done1 = done.clone();
            let start = Arc::new(AtomicBool::new(false));
            let start1 = start.clone();
            let t1 = s.spawn(move || {
                  core_affinity::set_for_current(CoreId { id: 1 });
                  let mut m = LatencyMsg::<N_BYTES>::default();
                  let mut c = 0u8;
                  let mut lck = ma_queues::Producer::from(q);
                  while !start1.load(Ordering::SeqCst) {};
                  loop {
                      ma_time::busy_sleep(Some(Nanos(2000)));
                      m.msg.fill(c);
                      m.tstamp = Instant::now();
                      lck.produce(&m);
                      c = c.wrapping_add(1);
                      if done1.load(sync::atomic::Ordering::Relaxed) {
                          break;
                      }
                  }
              });
             core_affinity::set_for_current(CoreId { id: 3 });
             let mut lock2 = ma_queues::Consumer::from(q);
             let mut m = LatencyMsg::<N_BYTES>::default();
             let mut tot = Duration::ZERO;
             start.store(true, Ordering::SeqCst);
             for i in 0..n_iters {
                 lock2.consume(&mut m);
                 tot += Duration::elapsed(m.tstamp);
             }
             done.store(true, Ordering::Relaxed);
             t1.join();
             tot.into()
         });
        let mut m = LatencyMsg::<N_BYTES>::default();
        let mut c = 0u8;
        let mut lck = ma_queues::Producer::from(q);
        m.msg[0] = 1;
        for i in 1..N_BYTES {
            m.msg[i] = 2;
        }
        lck.produce(&m);
    });
}

fn consume_latency(c: &mut Criterion) {
    for n_readers in 0..8 {
        let mut group = c.benchmark_group(format!("consume_latency_{}_readers", n_readers));
        for size in [8, 30, 32, 60, 124, 252, 508, 1020, 2044, 4092].iter() {
            group.throughput(criterion::Throughput::Bytes(*size as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| match size {
                     8 => {
                         consume_latency_bench::<8>(b, n_readers);
                     }
                     30 => {
                         consume_latency_bench::<30>(b, n_readers);
                     }
                     32 => {
                         consume_latency_bench::<32>(b, n_readers);
                     }
                     60 => {
                         consume_latency_bench::<60>(b, n_readers);
                     }
                     124 => {
                         consume_latency_bench::<124>(b, n_readers);
                     }
                     252 => {
                         consume_latency_bench::<252>(b, n_readers);
                     }
                     508 => {
                         consume_latency_bench::<508>(b, n_readers);
                     }
                     1020 => {
                         consume_latency_bench::<1020>(b, n_readers);
                     }
                     2044 => {
                         consume_latency_bench::<2044>(b, n_readers);
                     }
                     4092 => {
                         consume_latency_bench::<4092>(b, n_readers);
                     }
                     _ => {}
                 });
        }
        group.finish();
    }
}

criterion_group! {
    name=queues;
    config=Criterion::default();
    targets = consume, produce_spmc, consume_latency,
}

criterion_main!(queues);
