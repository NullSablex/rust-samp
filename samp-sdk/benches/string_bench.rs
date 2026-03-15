//! Benchmarks de parsing de strings AMX.
//!
//! Compara desempenho dos caminhos unpacked (1 byte/célula) e packed (4 bytes/célula),
//! custo de `put_in_buffer` / `write_str`, e baseline sem AMX.
//!
//! Execute com:
//! ```sh
//! cargo bench --target i686-unknown-linux-gnu
//! ```

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

use samp_sdk::cell::string::put_in_buffer;
use samp_sdk::cell::{AmxString, Buffer, Ref};

// ---------------------------------------------------------------------------
// Helpers de construção (sem AMX real)
// ---------------------------------------------------------------------------

fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
    let len = data.len();
    let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
    Buffer::new(r, len)
}

/// Constrói um buffer packed a partir de bytes.
/// Formato: 4 chars por célula, big-endian. Primeiro célula > 0x00FF_FFFF.
fn build_packed_cells(bytes: &[u8]) -> Vec<i32> {
    let n_cells = bytes.len().div_ceil(4) + 1;
    let mut cells = vec![0i32; n_cells];
    for (i, &b) in bytes.iter().enumerate() {
        let cell = i / 4;
        let shift = (3 - (i % 4)) * 8;
        cells[cell] |= (b as i32) << shift;
    }
    cells
}

// ---------------------------------------------------------------------------
// bench: put_in_buffer — escrita de string em buffer AMX
// ---------------------------------------------------------------------------

fn bench_put_in_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_in_buffer");

    for size in [8usize, 64, 256, 1024] {
        let input = "A".repeat(size);
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 2];
                let mut buf = make_buffer(&mut data);
                put_in_buffer(&mut buf, &input).unwrap()
            })
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: write_str (UnsizedBuffer) — nova API ergonômica
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
                ub.write_str(len, &input).unwrap()
            })
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: AmxString::new — construção (lazy — sem decode imediato)
// ---------------------------------------------------------------------------

fn bench_amx_string_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("amx_string_new");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, &size| {
            b.iter(|| {
                let mut data = vec![0i32; size + 1];
                let buf = make_buffer(&mut data);
                // AmxString não pode ser retornado pois empresta `data`.
                // is_packed() não aloca — força a construção sem acionar o decode lazy.
                let s = unsafe { AmxString::new(buf, &input) };
                s.len() // retorna self.len sem acionar o decode lazy
            })
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: to_bytes unpacked — parsing com um byte por célula
// ---------------------------------------------------------------------------

fn bench_to_bytes_unpacked(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_bytes/unpacked");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        let mut data: Vec<i32> = input.iter().map(|&b| b as i32).chain(std::iter::once(0)).collect();
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, &input) };

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| s.to_bytes())
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: to_bytes packed — parsing com 4 bytes por célula
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
            b.iter(|| s.to_bytes())
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: Deref — acesso lazy cacheado (sem alocação na segunda chamada)
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
                // Primeiro acesso — decoda e cacheia
                s.len()
            })
        });
    }
    group.finish();
}

fn bench_deref_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("deref/cached");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        let mut data: Vec<i32> = input.iter().map(|&b| b as i32).chain(std::iter::once(0)).collect();
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, &input) };
        let _ = &*s; // aquece o cache

        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            // Acesso subsequente — só lê o campo decoded
            b.iter(|| s.len())
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// bench: baseline — String::from_utf8_lossy sem AMX (comparação com samp-rs)
// ---------------------------------------------------------------------------

fn bench_baseline_from_utf8(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/from_utf8_lossy");

    for size in [8usize, 64, 256, 1024] {
        let input: Vec<u8> = (b'A'..=b'Z').cycle().take(size).collect();
        group.bench_with_input(BenchmarkId::new("len", size), &size, |b, _| {
            b.iter(|| String::from_utf8_lossy(&input).into_owned())
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_put_in_buffer,
    bench_write_str,
    bench_amx_string_new,
    bench_to_bytes_unpacked,
    bench_to_bytes_packed,
    bench_deref_first_access,
    bench_deref_cached,
    bench_baseline_from_utf8,
);
criterion_main!(benches);
