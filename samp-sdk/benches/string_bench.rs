//! Benchmarks for AMX string parsing.
//!
//! Compares performance of the unpacked (1 byte/cell) and packed (4 bytes/cell) paths,
//! the cost of `Buffer::write_str` / `UnsizedBuffer::write_str`, and a baseline without AMX.
//!
//! Run with:
//! ```sh
//! cargo bench --target i686-unknown-linux-gnu
//! ```

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use samp_sdk::cell::{AmxString, Buffer, Ref};

// ---------------------------------------------------------------------------
// Construction helpers (no real AMX)
// ---------------------------------------------------------------------------

fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
    let len = data.len();
    let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
    Buffer::new(r, len)
}

/// Builds a packed buffer from bytes.
/// Format: 4 chars per cell, big-endian. First cell > `0x00FF_FFFF`.
fn build_packed_cells(bytes: &[u8]) -> Vec<i32> {
    let n_cells = bytes.len().div_ceil(4) + 1;
    let mut cells = vec![0i32; n_cells];
    for (i, &b) in bytes.iter().enumerate() {
        let cell = i / 4;
        let shift = (3 - (i % 4)) * 8;
        cells[cell] |= i32::from(b) << shift;
    }
    cells
}

// ---------------------------------------------------------------------------
// bench: Buffer::write_str — string write into an AMX buffer (direct API)
// ---------------------------------------------------------------------------

fn bench_buffer_write_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_write_str");

    for size in [8usize, 64, 256, 1024] {
        let input = "A".repeat(size);
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 2];
                let mut buf = make_buffer(&mut data);
                buf.write_str(&input).unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: write_str (UnsizedBuffer) — new ergonomic API
// ---------------------------------------------------------------------------

fn bench_write_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_str");

    for size in [8usize, 64, 256, 1024] {
        let input = "A".repeat(size);
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 2];
                let len = data.len();
                let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
                let ub = samp_sdk::cell::UnsizedBuffer::from_raw_parts(r);
                ub.write_str(len, &input).unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: AmxString::new — construction (lazy — no immediate decode)
// ---------------------------------------------------------------------------

fn bench_amx_string_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("amx_string_new");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 1];
                let buf = make_buffer(&mut data);
                // AmxString cannot be returned because it borrows `data`.
                // is_packed() does not allocate — forces construction without triggering the lazy decode.
                let s = unsafe { AmxString::new(buf, &input) };
                // returns self.len without triggering the lazy decode
                black_box(s.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: to_bytes unpacked — parsing with one byte per cell
// ---------------------------------------------------------------------------

fn bench_to_bytes_unpacked(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_bytes/unpacked");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        let mut data: Vec<i32> = input
            .iter()
            .map(|&b| i32::from(b))
            .chain(std::iter::once(0))
            .collect();
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, &input) };

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| s.to_bytes());
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: to_bytes packed — parsing with 4 bytes per cell
// ---------------------------------------------------------------------------

fn bench_to_bytes_packed(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_bytes/packed");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        let mut data = build_packed_cells(&input);
        let len = data.len();
        let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
        let buf = Buffer::new(r, len);
        let s = AmxString::from_buffer_parts(buf, input.len());

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| s.to_bytes());
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: Deref — cached lazy access (no allocation on the second call)
// ---------------------------------------------------------------------------

fn bench_deref_first_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("deref/first_access");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 1];
                let buf = make_buffer(&mut data);
                let s = unsafe { AmxString::new(buf, &input) };
                // First access — decodes and caches
                black_box(s.len());
            });
        });
    }
    group.finish();
}

fn bench_deref_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("deref/cached");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        let mut data: Vec<i32> = input
            .iter()
            .map(|&b| i32::from(b))
            .chain(std::iter::once(0))
            .collect();
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, &input) };
        let _ = &*s; // warm up the cache

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            // Subsequent access — only reads the decoded field
            b.iter(|| s.len());
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: baseline — String::from_utf8_lossy without AMX (comparison with samp-rs)
// ---------------------------------------------------------------------------

fn bench_baseline_from_utf8(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/from_utf8_lossy");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| String::from_utf8_lossy(&input).into_owned());
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_write_str,
    bench_write_str,
    bench_amx_string_new,
    bench_to_bytes_unpacked,
    bench_to_bytes_packed,
    bench_deref_first_access,
    bench_deref_cached,
    bench_baseline_from_utf8,
);
criterion_main!(benches);
