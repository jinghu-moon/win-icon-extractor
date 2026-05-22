# win-icon-extractor

Extract file icons on Windows — pure Rust, no C dependency.

- **Raw RGBA pixels** from any file (exe, dll, ico, or shell fallback)
- **Extension icons** — get associated icon by file extension, no file needed
- **System stock icons** — folder, drive, recycle bin, shield, etc.
- **Icon enumeration** — query how many icons a PE file contains
- **WebP / PNG encoding** with configurable options
- **Base64 encoding & decoding** — extract as Base64 Data URL, and decode back with format auto-detection
- **Disk + memory cache** with mtime-based staleness detection
- **Bulk parallel extraction** via rayon

## Quick Start

```rust
use win_icon_extractor::*;

// Raw RGBA pixels from a file
let icon = extract_icon(r"C:\Windows\explorer.exe").unwrap();
println!("{}x{}, {} bytes", icon.width, icon.height, icon.rgba.len());

// By index or size
let icon = extract_icon_at(r"C:\Windows\explorer.exe", 1).unwrap();
let icon = extract_icon_with_size(r"C:\Windows\explorer.exe", 256).unwrap();

// Icon count in a PE file
let count = icon_count(r"C:\Windows\System32\shell32.dll"); // 335
```

## Extension Icons

Get the associated icon for any file extension — the file does not need to exist:

```rust
let icon = extract_icon_for_extension(".pdf").unwrap();
let icon = extract_icon_for_extension(".docx").unwrap();
```

## System Stock Icons

Extract Windows system-defined icons:

```rust
use win_icon_extractor::StockIcon;

let icon = extract_stock_icon(StockIcon::Folder).unwrap();
let icon = extract_stock_icon(StockIcon::DriveFixed).unwrap();
let icon = extract_stock_icon(StockIcon::Recycler).unwrap();
let icon = extract_stock_icon(StockIcon::Shield).unwrap();

// Custom size
let icon = extract_stock_icon_sized(StockIcon::Folder, 48).unwrap();
```

Available stock icons: `Folder`, `FolderOpen`, `DriveFixed`, `DriveRemovable`, `DriveNet`, `DriveCd`, `DriveDvd`, `Recycler`, `RecyclerFull`, `Shield`, `Warning`, `Error`, `Info`, `Internet`, `Server`, `Printer`, `Users`, `ZipFile`, `Settings`, and more.

## Encoding

```rust
// WebP (feature = "webp")
let webp = extract_icon_webp(r"C:\Windows\explorer.exe").unwrap();
std::fs::write("icon.webp", &webp).unwrap();

// PNG (feature = "png")
let png = extract_icon_png(r"C:\Windows\explorer.exe").unwrap();
std::fs::write("icon.png", &png).unwrap();

// Encode raw RGBA data
let webp = encode_webp(&icon.rgba, icon.width, icon.height).unwrap();
let png = encode_png(&icon.rgba, icon.width, icon.height).unwrap();
```

### Custom Encoding Options

```rust
use win_icon_extractor::*;

// WebP: custom quality, method, lossless mode
let opts = WebPOptions {
    quality: 90.0,          // 0.0–100.0 (default: 75.0)
    method: 6,              // 0=fast, 6=best (default: 5)
    lossless: true,         // lossless encoding (default: false)
    alpha_quality: 100,     // 0–100 (default: 100)
    exact: true,            // preserve RGB under transparent areas (default: false)
    ..Default::default()
};
let webp = encode_webp_with(&icon.rgba, icon.width, icon.height, &opts).unwrap();

// WebP best quality preset (lossless, max effort)
let webp = encode_webp_with(&icon.rgba, icon.width, icon.height, &WebPOptions::best_quality()).unwrap();

// PNG: custom filter and compression level
let opts = PngOptions {
    filter: PngFilter::None, // None (default, best for icons) or Sub
    compression_level: 10,   // 0–10 (default: 6)
};
let png = encode_png_with(&icon.rgba, icon.width, icon.height, &opts).unwrap();

// PNG best quality preset (max compression, None filter)
let png = encode_png_with(&icon.rgba, icon.width, icon.height, &PngOptions::best_quality()).unwrap();
```

## Caching

```rust
use win_icon_extractor::{IconCache, ImageFormat};

let cache = IconCache::with_app_name("my-app").unwrap();
let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();

// Use PNG format instead of WebP
let mut cache = IconCache::with_app_name("my-app").unwrap();
cache.set_format(ImageFormat::Png);

// Custom encoding options on cache
cache.set_webp_options(WebPOptions::best_quality());
cache.set_png_options(PngOptions::best_quality());

// Bulk parallel extraction (returns Result per path)
let paths = &[r"C:\Windows\System32\cmd.exe", r"C:\Windows\explorer.exe"];
let results = cache.extract_to_file_bulk(paths);
for (path, result) in &results {
    match result {
        Ok(cached) => println!("{path} → {}", cached.display()),
        Err(e) => eprintln!("{path}: {e}"),
    }
}

// Maintenance
let stats = cache.stats().unwrap();
cache.cleanup(30).unwrap(); // remove files older than 30 days
```

## Base64 Support

You can extract icons directly into Base64 encoded strings (useful for web display as Data URLs) or decode them back into binary image files:

```rust
use win_icon_extractor::*;

// Extract directly to WebP/PNG Base64 (Data URL ready)
let webp_base64 = extract_icon_webp_base64(r"C:\Windows\explorer.exe").unwrap();
let png_base64 = extract_icon_png_base64(r"C:\Windows\explorer.exe").unwrap();

// Standard Base64 encode
let base64_str = encode_base64(&raw_image_bytes);

// Decode back with adaptive format detection (supports pure Base64 or Data URLs)
let data_url = format!("data:image/webp;base64,{}", webp_base64);
let (decoded_bytes, format) = decode_image_base64(&data_url).unwrap();
assert_eq!(format, "webp");
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `webp`  | ✓ | WebP encoding via libwebp-sys |
| `png`   |   | PNG encoding (hand-written encoder + miniz_oxide) |
| `cache` | ✓ | Disk + memory cache with mtime validation |
| `bulk`  | ✓ | Parallel extraction via rayon |

```toml
# All defaults (webp + cache + bulk)
win-icon-extractor = "0.1"

# Minimal — raw extraction only
win-icon-extractor = { version = "0.1", default-features = false }

# PNG instead of WebP
win-icon-extractor = { version = "0.1", default-features = false, features = ["png", "cache"] }
```

## Performance

Benchmarked on Windows 11, release mode:

| API | Latency |
|-----|---------|
| `icon_count` | ~40µs |
| `extract_stock_icon` | ~126µs |
| `extract_icon_for_extension` | ~235µs |
| `extract_icon` (file) | ~1.6ms |
| Cache warm hit | ~1.4ms |

## License

AGPL-3.0
