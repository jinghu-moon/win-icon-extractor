//! Export 32x32 WebP icons. Default path: high-quality (largest frame → Lanczos).
//! Usage:
//!   cargo run --release --example export_webp32 --all-features -- <in_dir> <out_dir> [gdi|best|box|lanczos|mitchell]

use std::path::Path;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let in_dir = args
        .next()
        .unwrap_or_else(|| r"D:\200_Areas\210_Software\211_Portable\Alone\test-exe".into());
    let out_dir = args
        .next()
        .unwrap_or_else(|| r"D:\200_Areas\210_Software\211_Portable\Alone\test-exe-webp-32".into());
    let mode = args.next().unwrap_or_else(|| "lanczos".into());

    let out = Path::new(&out_dir);
    std::fs::create_dir_all(out).unwrap();

    let mut files: Vec<_> = std::fs::read_dir(&in_dir)
        .unwrap_or_else(|e| panic!("read_dir {in_dir}: {e}"))
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .map(|s| s.eq_ignore_ascii_case("exe"))
                == Some(true)
        })
        .collect();
    files.sort();

    println!("in   : {in_dir}");
    println!("out  : {out_dir}");
    println!("mode : {mode}");
    println!("targets: {}\n", files.len());

    use win_icon_extractor::{
        encode_webp, extract_icon_best_with, extract_icon_with_size, ResizeFilter,
    };
    let filter = match mode.as_str() {
        "box" => Some(ResizeFilter::Box),
        "mitchell" => Some(ResizeFilter::Mitchell),
        "catrom" | "catmull" => Some(ResizeFilter::CatmullRom),
        "bilinear" => Some(ResizeFilter::Bilinear),
        "gdi" => None,
        _ => Some(ResizeFilter::Lanczos3),
    };

    let t = Instant::now();
    let mut ok = 0u32;
    let mut fail = 0u32;
    let mut bytes_total = 0u64;

    for p in &files {
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "icon".into());

        let result = match filter {
            None => extract_icon_with_size(p, 32),
            Some(f) => extract_icon_best_with(p, 32, f),
        }
        .and_then(|d| {
            if d.width != 32 || d.height != 32 {
                // tolerate rare cases where source is smaller; pad is not done —
                // just require actual 32 for the export contract.
                return Err(win_icon_extractor::IconError::Extract(format!(
                    "got {}x{}, expected 32x32",
                    d.width, d.height
                )));
            }
            encode_webp(&d.rgba, d.width, d.height)
        });

        match result {
            Ok(webp) => {
                let dest = out.join(format!("{stem}.webp"));
                if let Err(e) = std::fs::write(&dest, &webp) {
                    eprintln!("[FAIL write] {} : {e}", dest.display());
                    fail += 1;
                } else {
                    ok += 1;
                    bytes_total += webp.len() as u64;
                }
            }
            Err(e) => {
                eprintln!("[FAIL] {} : {e}", p.display());
                fail += 1;
            }
        }
    }

    let dt = t.elapsed();
    println!("=== done ===");
    println!(
        "ok={ok} fail={fail} in {dt:?} ({:.0} icons/s)",
        ok as f64 / dt.as_secs_f64()
    );
    println!(
        "total {:.2} KB, avg {:.1} B/icon",
        bytes_total as f64 / 1024.0,
        bytes_total as f64 / ok.max(1) as f64
    );
}
