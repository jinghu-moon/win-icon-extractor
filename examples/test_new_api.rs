use win_icon_extractor::*;
use std::path::Path;

fn main() {
    let out = Path::new("test_output");
    std::fs::create_dir_all(out).unwrap();

    // 1. icon_count
    let count = icon_count(r"C:\Windows\explorer.exe");
    println!("explorer.exe: {count} icons");
    let count2 = icon_count(r"C:\Windows\System32\shell32.dll");
    println!("shell32.dll: {count2} icons");
    assert!(count > 0, "explorer.exe should have icons");
    assert!(count2 > 10, "shell32.dll should have many icons");

    // 2. Extension icons → save as PNG
    for ext in [".pdf", ".docx", ".rs", ".txt", ".zip", ".exe"] {
        let data = extract_icon_for_extension(ext).unwrap();
        validate_and_save(&data, &out.join(format!("ext_{}.png", &ext[1..])));
    }

    // 3. Stock icons → save as PNG
    let stocks = [
        ("folder", StockIcon::Folder),
        ("drive_fixed", StockIcon::DriveFixed),
        ("recycler", StockIcon::Recycler),
        ("shield", StockIcon::Shield),
        ("internet", StockIcon::Internet),
        ("warning", StockIcon::Warning),
    ];
    for (name, id) in stocks {
        let data = extract_stock_icon(id).unwrap();
        validate_and_save(&data, &out.join(format!("stock_{name}.png")));
    }

    // 4. File icon (existing file) → save as PNG
    let data = extract_icon(r"C:\Windows\explorer.exe").unwrap();
    validate_and_save(&data, &out.join("file_explorer.png"));

    // 5. Extract by index (shell32.dll has many)
    for i in 0..3 {
        let data = extract_icon_at(r"C:\Windows\System32\shell32.dll", i).unwrap();
        validate_and_save(&data, &out.join(format!("shell32_idx{i}.png")));
    }

    // 6. Base64 提取 API 测试
    #[cfg(feature = "webp")]
    {
        let webp_base64 = extract_icon_webp_base64(r"C:\Windows\explorer.exe").unwrap();
        assert!(!webp_base64.is_empty(), "WebP base64 should not be empty");
        println!("[OK] WebP base64 (first 30 chars): {}", &webp_base64[..30]);
    }
    #[cfg(feature = "png")]
    {
        let png_base64 = extract_icon_png_base64(r"C:\Windows\explorer.exe").unwrap();
        assert!(!png_base64.is_empty(), "PNG base64 should not be empty");
        println!("[OK] PNG base64 (first 30 chars): {}", &png_base64[..30]);
    }

    println!("\n=== ALL PASSED ===");
    println!("Output: {}", std::fs::canonicalize(out).unwrap().display());
}

fn validate_and_save(data: &IconData, path: &Path) {
    let pixel_count = (data.width * data.height) as usize;
    let expected_bytes = pixel_count * 4;
    assert_eq!(data.rgba.len(), expected_bytes, "RGBA buffer size mismatch");
    assert!(data.width > 0 && data.height > 0, "zero dimensions");

    // Check not all zeros (blank image)
    let non_zero = data.rgba.iter().filter(|&&b| b != 0).count();
    let ratio = non_zero as f64 / data.rgba.len() as f64;
    assert!(ratio > 0.01, "image appears blank ({:.1}% non-zero)", ratio * 100.0);

    // Check alpha channel has content
    let alpha_nonzero = data.rgba.chunks_exact(4).filter(|px| px[3] != 0).count();
    assert!(alpha_nonzero > 0, "all pixels fully transparent");

    // Encode to PNG and save
    let png = encode_png(&data.rgba, data.width, data.height).unwrap();
    assert!(png.len() > 50, "PNG too small, likely corrupt");

    // Verify PNG signature
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "invalid PNG signature");

    std::fs::write(path, &png).unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    println!("[OK] {name}: {}x{}, {:.1}KB, alpha={}/{pixel_count}, non-zero={:.0}%",
        data.width, data.height,
        png.len() as f64 / 1024.0,
        alpha_nonzero, ratio * 100.0,
    );
}
