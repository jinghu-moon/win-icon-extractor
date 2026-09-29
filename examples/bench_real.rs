//! Performance benchmark against a large real-world exe folder.
//! Usage: cargo run --release --example bench_real --all-features
//!        (optional) cargo run --release --example bench_real --all-features -- "D:\path\to\exes"

use std::time::Instant;
use win_icon_extractor::*;

fn collect_exes(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {dir}: {e}"))
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()
                .and_then(|x| x.to_str())
                .map(|s| s.eq_ignore_ascii_case("exe"))
                == Some(true)
            {
                Some(p.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    v.sort();
    v
}

fn pctl(sorted_ms: &[f64], p: f64) -> f64 {
    if sorted_ms.is_empty() {
        return 0.0;
    }
    let idx = ((sorted_ms.len() as f64 - 1.0) * p).round() as usize;
    sorted_ms[idx.min(sorted_ms.len() - 1)]
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"D:\200_Areas\210_Software\211_Portable\Alone\test-exe".to_string());

    let paths = collect_exes(&dir);
    let n = paths.len();
    println!("=== win-icon-extractor bench ===");
    println!("dir: {dir}");
    println!("targets: {n} exe files\n");
    assert!(n > 0, "no exe found");

    let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

    // ── 1. icon_count (serial) ──
    let mut count_ok = 0u32;
    let mut count_fail = 0u32;
    let mut lat: Vec<f64> = Vec::with_capacity(n);
    let t = Instant::now();
    for p in &paths {
        let t0 = Instant::now();
        match icon_count(p) {
            Ok(c) => {
                if c > 0 {
                    count_ok += 1;
                }
            }
            Err(_) => count_fail += 1,
        }
        lat.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    let total = t.elapsed();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "[icon_count]      total {:>8.1?}  avg {:>7.1}µs  p50 {:>7.1}µs  p95 {:>7.1}µs  ok={count_ok} fail={count_fail}",
        total,
        total.as_micros() as f64 / n as f64,
        pctl(&lat, 0.50),
        pctl(&lat, 0.95),
    );

    // ── 2. list_icon_sizes ──
    let mut sizes_hits = 0u32;
    lat.clear();
    let t = Instant::now();
    for p in &paths {
        let t0 = Instant::now();
        if let Ok(s) = list_icon_sizes(p) {
            if !s.is_empty() {
                sizes_hits += 1;
            }
        }
        lat.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    let total = t.elapsed();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "[list_sizes]      total {:>8.1?}  avg {:>7.1}µs  p50 {:>7.1}µs  p95 {:>7.1}µs  with_sizes={sizes_hits}",
        total,
        total.as_micros() as f64 / n as f64,
        pctl(&lat, 0.50),
        pctl(&lat, 0.95),
    );

    // ── 3. extract_icon serial ──
    let mut ok = 0u32;
    let mut fail = 0u32;
    let mut sum_w = 0u64;
    lat.clear();
    let t = Instant::now();
    for p in &paths {
        let t0 = Instant::now();
        match extract_icon(p) {
            Ok(d) => {
                ok += 1;
                sum_w += d.width as u64;
            }
            Err(_) => fail += 1,
        }
        lat.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    let serial = t.elapsed();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "[extract serial]  total {:>8.1?}  avg {:>7.1}µs  p50 {:>7.1}µs  p95 {:>7.1}µs  ok={ok} fail={fail} avg_w={:.0}",
        serial,
        serial.as_micros() as f64 / n as f64,
        pctl(&lat, 0.50),
        pctl(&lat, 0.95),
        sum_w as f64 / ok.max(1) as f64,
    );

    // ── 4. extract_icons_bulk (parallel) ──
    let t = Instant::now();
    let bulk = extract_icons_bulk(&refs);
    let parallel = t.elapsed();
    let bulk_ok = bulk.iter().filter(|(_, r)| r.is_ok()).count();
    println!(
        "[extract par]     total {:>8.1?}  throughput {:>6.0} icons/s  ok={bulk_ok}/{n}  speedup {:.1}x",
        parallel,
        n as f64 / parallel.as_secs_f64(),
        serial.as_secs_f64() / parallel.as_secs_f64(),
    );

    // ── 5. extract_icon 2nd pass (process caches warm) ──
    lat.clear();
    let t = Instant::now();
    for p in &paths {
        let t0 = Instant::now();
        let _ = extract_icon(p);
        lat.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    let warm = t.elapsed();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "[extract warm]    total {:>8.1?}  avg {:>7.1}µs  p50 {:>7.1}µs  p95 {:>7.1}µs  (max_size cached)",
        warm,
        warm.as_micros() as f64 / n as f64,
        pctl(&lat, 0.50),
        pctl(&lat, 0.95),
    );

    // ── 6. IconCache cold → warm ──
    let cache_dir = std::env::temp_dir().join("win-icon-extractor-bench-real");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let cache = IconCache::builder(&cache_dir)
        .mtime_ttl(std::time::Duration::from_secs(60))
        .build()
        .unwrap();

    let t = Instant::now();
    let cold = cache.extract_to_file_bulk(&refs);
    let cache_cold = t.elapsed();
    let cold_ok = cold.iter().filter(|(_, r)| r.is_ok()).count();
    println!(
        "[cache cold]      total {:>8.1?}  throughput {:>6.0} icons/s  ok={cold_ok}/{n}",
        cache_cold,
        n as f64 / cache_cold.as_secs_f64(),
    );

    // memory hit (TTL 60s — no fs work)
    let t = Instant::now();
    for _ in 0..10 {
        for p in &refs {
            let _ = cache.extract_to_file(*p).unwrap();
        }
    }
    let warm10 = t.elapsed();
    let per_hit = warm10.as_secs_f64() / (n * 10) as f64 * 1e9;
    println!(
        "[cache warm x10]  total {:>8.1?}  per-hit {:>7.1}ns   (TTL memory hit)",
        warm10, per_hit,
    );

    // bulk warm once
    let t = Instant::now();
    let _ = cache.extract_to_file_bulk(&refs);
    let bulk_warm = t.elapsed();
    println!(
        "[cache bulk warm] total {:>8.1?}  per-hit {:>7.1}ns",
        bulk_warm,
        bulk_warm.as_secs_f64() / n as f64 * 1e9,
    );

    // ── 7. stock / extension (process cache) ──
    let stocks = [
        StockIcon::Folder,
        StockIcon::DriveFixed,
        StockIcon::Recycler,
        StockIcon::Shield,
        StockIcon::Warning,
        StockIcon::Info,
    ];
    // cold first
    let t = Instant::now();
    for &s in &stocks {
        let _ = extract_stock_icon(s).unwrap();
    }
    let stock_cold = t.elapsed();
    let t = Instant::now();
    for _ in 0..200 {
        for &s in &stocks {
            let _ = extract_stock_icon(s).unwrap();
        }
    }
    let stock_warm = t.elapsed();
    let stock_calls = 200 * stocks.len() as u32;
    println!(
        "[stock cold]      total {:>8.1?}  [stock warm] avg {:>7.2}µs/call ({} calls)",
        stock_cold,
        stock_warm.as_micros() as f64 / stock_calls as f64,
        stock_calls,
    );

    let exts = [
        ".exe", ".dll", ".txt", ".pdf", ".png", ".zip", ".rs", ".html",
    ];
    let t = Instant::now();
    for e in &exts {
        let _ = extract_icon_for_extension(e);
    }
    let ext_cold = t.elapsed();
    let t = Instant::now();
    for _ in 0..200 {
        for e in &exts {
            let _ = extract_icon_for_extension_cached(e, 0).unwrap();
        }
    }
    let ext_warm = t.elapsed();
    let ext_calls = 200 * exts.len() as u32;
    println!(
        "[ext cold]        total {:>8.1?}  [ext warm]  avg {:>7.2}µs/call ({} calls)",
        ext_cold,
        ext_warm.as_micros() as f64 / ext_calls as f64,
        ext_calls,
    );

    // ── 8. sized extract sample ──
    let sample: Vec<&str> = refs.iter().copied().take(50).collect();
    let t = Instant::now();
    for p in &sample {
        let _ = extract_icon_with_size(p, 256);
    }
    let sz = t.elapsed();
    println!(
        "[sized 256 x50]   total {:>8.1?}  avg {:>7.1}µs",
        sz,
        sz.as_micros() as f64 / sample.len() as f64,
    );

    // stats
    let st = cache.stats().unwrap();
    println!(
        "\n── cache dir: {} files, {:.2} MB ──",
        st.total_files,
        st.total_size as f64 / 1024.0 / 1024.0
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
    println!("done.");
}
