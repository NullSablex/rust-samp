//! AMX strings: decoding, the cached `&str`, and writing back.
//!
//! Cells come from a `Vec`, not a VM, so this is the SDK's own cost. Every
//! measurement consumes its result through `black_box`, and setup that a real
//! call would not repeat (allocating the cells) is kept out of the timing with
//! `iter_batched`.
//!
//! Run with:
//!
//! ```sh
//! cargo bench -p rust-samp-sdk --bench string_bench --target i686-unknown-linux-gnu
//! ```

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use samp_sdk::cell::{AmxString, Buffer, Ref};

const SIZES: [usize; 4] = [8, 64, 256, 1024];

fn buffer(cells: &mut [i32]) -> Buffer<'_> {
    let len = cells.len();
    // SAFETY: the pointer covers `len` live cells for the borrow of `cells`.
    Buffer::new(unsafe { Ref::new(0, cells.as_mut_ptr()) }, len)
}

fn text(size: usize) -> String {
    (b'A'..=b'Z').cycle().take(size).map(char::from).collect()
}

/// Unpacked: one character per cell, then the terminator.
fn unpacked(text: &str) -> Vec<i32> {
    text.bytes().map(i32::from).chain([0]).collect()
}

/// Packed: four characters per cell, the first in the high byte.
fn packed(text: &str) -> Vec<i32> {
    let mut cells = vec![0i32; text.len() / 4 + 1];
    for (i, byte) in text.bytes().enumerate() {
        cells[i / 4] |= i32::from(byte) << ((3 - i % 4) * 8);
    }
    cells
}

fn to_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_bytes");
    for size in SIZES {
        let text = text(size);
        group.throughput(Throughput::Bytes(size as u64));
        for (layout, mut cells) in [("unpacked", unpacked(&text)), ("packed", packed(&text))] {
            let string = AmxString::from_buffer_parts(buffer(&mut cells), size);
            group.bench_function(BenchmarkId::new(layout, size), |b| {
                b.iter(|| black_box(string.to_bytes()));
            });
        }
    }
    group.finish();
}

/// `&*string` the first time: decodes the cells and caches the `&str`.
fn deref_first(c: &mut Criterion) {
    let mut group = c.benchmark_group("deref_first");
    for size in SIZES {
        let cells = unpacked(&text(size));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter_batched_ref(
                || cells.clone(),
                |cells| {
                    let string = AmxString::from_buffer_parts(buffer(cells), size);
                    black_box(string.as_str().len())
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// `&*string` again: served from the cache.
fn deref_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("deref_cached");
    for size in SIZES {
        let mut cells = unpacked(&text(size));
        let string = AmxString::from_buffer_parts(buffer(&mut cells), size);
        let _ = string.as_str();
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| black_box(black_box(&string).as_str().len()));
        });
    }
    group.finish();
}

/// `Buffer::write_str` into a buffer that fits the text and its terminator.
fn write_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_str");
    for size in SIZES {
        let text = text(size);
        let mut cells = vec![0i32; size + 1];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let mut out = buffer(&mut cells);
                out.write_str(black_box(&text)).unwrap();
                black_box(out.as_slice()[0])
            });
        });
    }
    group.finish();
}

/// What decoding costs without the AMX: bytes to an owned `String`.
fn baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_utf8_to_string");
    for size in SIZES {
        let bytes = text(size).into_bytes();
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| black_box(String::from_utf8_lossy(black_box(&bytes)).into_owned()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    to_bytes,
    deref_first,
    deref_cached,
    write_str,
    baseline
);
criterion_main!(benches);
