//! Criterion micro-benchmarks for the hot extraction paths.
//!
//! Uses the first exe files found under `BENCH_EXE_DIR`
//! (default: a few well-known system binaries).

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::path::{Path, PathBuf};
use win_icon_extractor::*;

fn sample_paths() -> Vec<String> {
    // Prefer a user-supplied folder of exes.
    if let Ok(dir) = std::env::var("BENCH_EXE_DIR") {
        let mut v: Vec<String> = std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()).map(|s| s.eq_ignore_ascii_case("exe"))
                    == Some(true)
                {
                    Some(p.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        v.sort();
        v.truncate(32);
        return v;
    }

    // Fallback: system binaries that exist on any Windows machine.
    let cands = [
        r"C:\Windows\explorer.exe",
        r"C:\Windows\System32\notepad.exe",
        r"C:\Windows\System32\cmd.exe",
        r"C:\Windows\System32\shell32.dll",
        r"C:\Windows\System32\imageres.dll",
    ];
    cands
        .iter()
        .filter(|p| Path::new(p).exists())
        .map(|s| s.to_string())
        .collect()
}

fn bench_extract(c: &mut Criterion) {
    let paths = sample_paths();
    if paths.is_empty() {
        eprintln!("no sample binaries found — set BENCH_EXE_DIR to a folder of exe files");
        return;
    }

    let mut g = c.benchmark_group("extract");
    g.throughput(Throughput::Elements(1));

    g.bench_function("icon_count", |b| {
        b.iter(|| icon_count(std::hint::black_box(&paths[0])).unwrap())
    });

    g.bench_function("extract_icon", |b| {
        b.iter(|| extract_icon(std::hint::black_box(&paths[0])).unwrap())
    });

    g.bench_function("extract_icon_32_gdi", |b| {
        b.iter(|| extract_icon_with_size(std::hint::black_box(&paths[0]), 32).unwrap())
    });

    #[cfg(feature = "resize")]
    g.bench_function("extract_icon_32_best", |b| {
        b.iter(|| extract_icon_best(std::hint::black_box(&paths[0]), 32).unwrap())
    });

    g.bench_function("list_icon_sizes", |b| {
        b.iter(|| list_icon_sizes(std::hint::black_box(&paths[0])).unwrap())
    });

    g.finish();
}

fn bench_cache(c: &mut Criterion) {
    let paths = sample_paths();
    if paths.is_empty() {
        return;
    }

    let dir: PathBuf = std::env::temp_dir().join("win-icon-criterion-cache");
    let _ = std::fs::remove_dir_all(&dir);
    let cache = IconCache::builder(&dir)
        .mtime_ttl(std::time::Duration::from_secs(3600))
        .build()
        .unwrap();

    for p in &paths {
        let _ = cache.extract_to_file(p);
    }

    let mut g = c.benchmark_group("cache");
    g.throughput(Throughput::Elements(1));
    g.bench_function("memory_hit_ttl", |b| {
        b.iter(|| cache.extract_to_file(std::hint::black_box(&paths[0])).unwrap())
    });
    g.finish();

    let _ = std::fs::remove_dir_all(&dir);
}

fn bench_resize(c: &mut Criterion) {
    let paths = sample_paths();
    if paths.is_empty() {
        return;
    }
    let big = extract_icon(&paths[0]).unwrap();

    let mut g = c.benchmark_group("resize");
    g.throughput(Throughput::Elements(1));
    g.bench_function("to_32_lanczos3", |b| {
        b.iter(|| big.resized_square(32, ResizeFilter::Lanczos3).unwrap())
    });
    g.bench_function("to_32_box", |b| {
        b.iter(|| big.resized_square(32, ResizeFilter::Box).unwrap())
    });
    g.finish();
}

fn bench_encode(c: &mut Criterion) {
    let paths = sample_paths();
    if paths.is_empty() {
        return;
    }
    let icon = extract_icon(&paths[0]).unwrap();

    let mut g = c.benchmark_group("encode");
    g.throughput(Throughput::Elements(1));
    #[cfg(feature = "webp")]
    g.bench_function("webp_lossless", |b| {
        b.iter(|| encode_webp(std::hint::black_box(&icon.rgba), icon.width, icon.height).unwrap())
    });
    #[cfg(feature = "png")]
    g.bench_function("png", |b| {
        b.iter(|| encode_png(std::hint::black_box(&icon.rgba), icon.width, icon.height).unwrap())
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_extract,
    bench_cache,
    bench_resize,
    bench_encode
);
criterion_main!(benches);
