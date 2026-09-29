use std::time::Instant;
use win_icon_extractor::IconCache;

fn main() {
    // Collect test targets from System32
    let sys32 = r"C:\Windows\System32";
    let paths: Vec<String> = std::fs::read_dir(sys32)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("exe") {
                Some(p.to_string_lossy().into())
            } else {
                None
            }
        })
        .take(50)
        .collect();

    let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    println!("Test targets: {} exe files from System32\n", paths.len());

    // ── Benchmark 1: Raw extraction (no cache) ──
    let t = Instant::now();
    let mut ok = 0;
    for p in &paths {
        if win_icon_extractor::extract_icon(p).is_ok() {
            ok += 1;
        }
    }
    let raw_serial = t.elapsed();
    println!(
        "[Raw serial]     {ok}/{} icons, {:?}",
        paths.len(),
        raw_serial
    );

    // ── Benchmark 2: Bulk parallel raw extraction ──
    let t = Instant::now();
    let bulk = win_icon_extractor::extract_icons_bulk(&refs);
    let raw_parallel = t.elapsed();
    println!(
        "[Raw parallel]   {}/{} icons, {:?}",
        bulk.len(),
        paths.len(),
        raw_parallel
    );

    // ── Benchmark 3: Cache cold start (first run, disk write) ──
    let cache_dir = std::env::temp_dir().join("icon-bench-cache");
    let _ = std::fs::remove_dir_all(&cache_dir);
    let cache = IconCache::new(cache_dir.clone()).unwrap();

    let t = Instant::now();
    let bulk_cached = cache.extract_to_file_bulk(&refs);
    let cache_cold = t.elapsed();
    let ok_cold = bulk_cached.iter().filter(|(_, r)| r.is_ok()).count();
    println!(
        "[Cache cold]     {ok_cold}/{} icons, {:?}",
        paths.len(),
        cache_cold
    );

    // ── Benchmark 4: Cache warm (memory hit, TTL = 5s → no stat) ──
    let t = Instant::now();
    for _ in 0..5 {
        let _ = cache.extract_to_file_bulk(&refs);
    }
    let cache_warm = t.elapsed();
    println!(
        "[Cache warm x5]  {:?}  (avg {:?}/round)",
        cache_warm,
        cache_warm / 5
    );

    // ── Benchmark 5: Cache warm single ──
    let t = Instant::now();
    for p in &paths {
        let _ = cache.extract_to_file(p).unwrap();
    }
    let cache_single = t.elapsed();
    println!(
        "[Cache warm seq]  {}/{} icons, {:?}  ({:?}/hit)",
        paths.len(),
        paths.len(),
        cache_single,
        cache_single / paths.len() as u32
    );

    // ── Summary ──
    println!("\n── Summary ──");
    let speedup_parallel = raw_serial.as_micros() as f64 / raw_parallel.as_micros() as f64;
    println!("Parallel vs serial:  {:.1}x", speedup_parallel);
    let speedup_cache =
        raw_serial.as_micros() as f64 / (cache_single.as_micros() as f64 / paths.len() as f64);
    println!("Cache warm vs raw:   {:.0}x (per-hit)", speedup_cache);

    // Cleanup
    let _ = std::fs::remove_dir_all(&cache_dir);
}
