//! Benchmarks for Buffer/CellConvert operations.
//!
//! Compares performance of `get_as`, `set_as` and `iter_as` for typed types
//! (`f32`, `bool`, `i32`) across buffers of varying sizes.
//!
//! Run with:
//! ```sh
//! cargo bench -p samp-sdk --target i686-unknown-linux-gnu
//! ```

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use samp_sdk::cell::repr::CellConvert;
use samp_sdk::cell::{Buffer, Ref};

// ---------------------------------------------------------------------------
// Construction helper (no real AMX)
// ---------------------------------------------------------------------------

fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
    let len = data.len();
    let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
    Buffer::new(r, len)
}

// ---------------------------------------------------------------------------
// bench: get_as::<f32> — typed cell read
// ---------------------------------------------------------------------------

fn bench_get_as_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/get_as_f32");

    for size in [8usize, 64, 256, 1024] {
        // `size` is a small literal (max 1024) — `as f32` is exact.
        #[allow(clippy::cast_precision_loss)]
        let values: Vec<f32> = (0..size).map(|i| i as f32 * 0.5).collect();
        let mut data: Vec<i32> = values.iter().map(|&v| v.into_cell()).collect();

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let buf = make_buffer(&mut data);
                let mut sum = 0.0f32;
                for i in 0..size {
                    if let Some(v) = buf.get_as::<f32>(i) {
                        sum += v;
                    }
                }
                black_box(sum);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: set_as::<bool> — typed cell write
// ---------------------------------------------------------------------------

fn bench_set_as_bool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/set_as_bool");

    for size in [8usize, 64, 256, 1024] {
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size];
                let mut buf = make_buffer(&mut data);
                for i in 0..size {
                    buf.set_as(i, i % 2 == 0);
                }
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: iter_as::<i32> — typed iteration over the whole buffer
// ---------------------------------------------------------------------------

fn bench_iter_as_i32(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/iter_as_i32");

    for size in [8usize, 64, 256, 1024] {
        let size_i32 = i32::try_from(size).expect("size literal fits in i32");
        let mut data: Vec<i32> = (0..size_i32).collect();

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| {
                let buf = make_buffer(&mut data);
                let sum: i32 = buf.iter_as::<i32>().sum();
                black_box(sum);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: iter_as::<f32> — typed iteration with bit conversion
// ---------------------------------------------------------------------------

fn bench_iter_as_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/iter_as_f32");

    for size in [8usize, 64, 256, 1024] {
        // `size` is a small literal (max 1024) — `as f32` is exact.
        #[allow(clippy::cast_precision_loss)]
        let values: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let mut data: Vec<i32> = values.iter().map(|&v| v.into_cell()).collect();

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| {
                let buf = make_buffer(&mut data);
                let sum: f32 = buf.iter_as::<f32>().sum();
                black_box(sum);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_get_as_f32,
    bench_set_as_bool,
    bench_iter_as_i32,
    bench_iter_as_f32,
);
criterion_main!(benches);
