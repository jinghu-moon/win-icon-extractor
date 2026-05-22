use std::time::Instant;
use win_icon_extractor::*;

fn main() {
    let rounds = 50;

    // Warmup
    let _ = icon_count(r"C:\Windows\explorer.exe");
    let _ = extract_icon_for_extension(".pdf");
    let _ = extract_stock_icon(StockIcon::Folder);

    // 1. icon_count
    let t = Instant::now();
    for _ in 0..rounds {
        let _ = icon_count(r"C:\Windows\System32\shell32.dll");
    }
    let avg = t.elapsed() / rounds;
    println!("[icon_count]     shell32.dll: {:?}/call", avg);

    // 2. extract_icon_for_extension
    let exts = [".pdf", ".docx", ".rs", ".txt", ".zip", ".exe", ".mp3", ".png", ".jpg", ".html"];
    let t = Instant::now();
    for _ in 0..rounds {
        for ext in &exts {
            let _ = extract_icon_for_extension(ext);
        }
    }
    let total_calls = rounds * exts.len() as u32;
    let avg = t.elapsed() / total_calls;
    println!("[ext_icon]       avg over {total_calls} calls: {:?}/call", avg);

    // 3. extract_stock_icon
    let stocks = [
        StockIcon::Folder, StockIcon::DriveFixed, StockIcon::Recycler,
        StockIcon::Shield, StockIcon::Internet, StockIcon::Warning,
    ];
    let t = Instant::now();
    for _ in 0..rounds {
        for &id in &stocks {
            let _ = extract_stock_icon(id);
        }
    }
    let total_calls = rounds * stocks.len() as u32;
    let avg = t.elapsed() / total_calls;
    println!("[stock_icon]     avg over {total_calls} calls: {:?}/call", avg);

    // 4. Comparison: extract_icon (existing file)
    let t = Instant::now();
    for _ in 0..rounds {
        let _ = extract_icon(r"C:\Windows\explorer.exe");
    }
    let avg = t.elapsed() / rounds;
    println!("[extract_icon]   explorer.exe: {:?}/call (baseline)", avg);
}
