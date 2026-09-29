//! Visual A/B: GDI scale vs high-quality downscale on a few sample exes.
//! Writes PNGs to test_output/compare/.

use std::path::Path;
use win_icon_extractor::*;

fn save(name: &str, data: &IconData, dir: &Path) {
    let png = encode_png(&data.rgba, data.width, data.height).unwrap();
    std::fs::write(dir.join(name), &png).unwrap();
    println!(
        "  {name}: {}x{}, {:.1}KB",
        data.width,
        data.height,
        png.len() as f64 / 1024.0
    );
}

fn main() {
    let dir = Path::new("test_output/compare");
    std::fs::create_dir_all(dir).unwrap();

    // Pick a handful of well-known system exes (rich alpha + fine detail).
    let samples = [
        r"C:\Windows\explorer.exe",
        r"C:\Windows\System32\shell32.dll",
        r"C:\Windows\System32\notepad.exe",
        r"C:\Windows\System32\cmd.exe",
        r"C:\Windows\System32\mspaint.exe",
    ];

    for path in samples {
        let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        println!("\n{stem}:");

        // source at max native size
        let big = extract_icon(path).unwrap();
        println!("  source: {}x{}", big.width, big.height);
        save(&format!("{stem}_src.png"), &big, dir);

        // GDI scale (legacy extract_icon_with_size)
        let gdi = extract_icon_with_size(path, 32).unwrap();
        save(&format!("{stem}_gdi32.png"), &gdi, dir);

        // High-quality pipelines from the SAME largest frame
        let l3 = big.resized_square(32, ResizeFilter::Lanczos3).unwrap();
        save(&format!("{stem}_lanczos32.png"), &l3, dir);

        let bx = big.resized_square(32, ResizeFilter::Box).unwrap();
        save(&format!("{stem}_box32.png"), &bx, dir);

        let mi = big.resized_square(32, ResizeFilter::Mitchell).unwrap();
        save(&format!("{stem}_mitchell32.png"), &mi, dir);

        // extract_icon_best convenience
        let best = extract_icon_best(path, 32).unwrap();
        assert_eq!(best.width, 32);
    }

    println!(
        "\noutput: {}",
        std::fs::canonicalize(dir).unwrap().display()
    );
}
