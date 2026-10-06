//! Typed access to AMX arrays: `get_as`, `set_as` and `iter_as`.
//!
//! The conversions themselves are free — `i32` is the cell, `f32` its bits,
//! `bool` a comparison — so what is measured is the access pattern. Results go
//! through `black_box`, and float results are folded as their bits: summing
//! `f32` cannot be vectorized (float addition is not associative), which would
//! time the sum rather than the conversion.
//!
//! Run with:
//!
//! ```sh
//! cargo bench -p rust-samp-sdk --bench buffer_bench --target i686-unknown-linux-gnu
//! ```

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use samp_sdk::cell::{Buffer, Ref};

const SIZES: [usize; 4] = [8, 64, 256, 1024];

fn buffer(cells: &mut [i32]) -> Buffer<'_> {
    let len = cells.len();
    // SAFETY: the pointer covers `len` live cells for the borrow of `cells`.
    Buffer::new(unsafe { Ref::new(0, cells.as_mut_ptr()) }, len)
}

fn cells(size: usize) -> Vec<i32> {
    (0..size)
        .map(|i| (i as f32 * 0.5).to_bits().cast_signed())
        .collect()
}

fn read(c: &mut Criterion) {
    let mut group = c.benchmark_group("read");
    for size in SIZES {
        let mut data = cells(size);
        let buf = buffer(&mut data);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(BenchmarkId::new("iter_as_i32", size), |b| {
            b.iter(|| {
                black_box(&buf)
                    .iter_as::<i32>()
                    .fold(0i32, i32::wrapping_add)
            });
        });
        group.bench_function(BenchmarkId::new("iter_as_f32", size), |b| {
            b.iter(|| {
                black_box(&buf)
                    .iter_as::<f32>()
                    .fold(0u32, |acc, v| acc.wrapping_add(v.to_bits()))
            });
        });
        group.bench_function(BenchmarkId::new("get_as_f32", size), |b| {
            b.iter(|| {
                let buf = black_box(&buf);
                (0..size).fold(0u32, |acc, i| {
                    acc.wrapping_add(buf.get_as::<f32>(i).map_or(0, f32::to_bits))
                })
            });
        });
    }
    group.finish();
}

fn write(c: &mut Criterion) {
    let mut group = c.benchmark_group("write");
    for size in SIZES {
        let mut data = vec![0i32; size];
        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(BenchmarkId::new("set_as_bool", size), |b| {
            b.iter(|| {
                let mut buf = buffer(&mut data);
                for i in 0..size {
                    buf.set_as(i, black_box(i % 2 == 0));
                }
                black_box(buf.as_slice()[size - 1])
            });
        });
        group.bench_function(BenchmarkId::new("set_as_f32", size), |b| {
            b.iter(|| {
                let mut buf = buffer(&mut data);
                for i in 0..size {
                    buf.set_as(i, black_box(i as f32));
                }
                black_box(buf.as_slice()[size - 1])
            });
        });
    }
    group.finish();
}

criterion_group!(benches, read, write);
criterion_main!(benches);
