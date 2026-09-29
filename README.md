# win-icon-extractor

Extract file icons on Windows — pure Rust, no C dependency.

> **Upgrading from 0.2?** See [CHANGELOG.md](CHANGELOG.md) — 0.3 is a breaking
> release (`icon_count` → `Result`, ordered bulk, pure-Rust `webp` feature).

- **Raw RGBA pixels** from any file (exe, dll, ico, or shell fallback) with true alpha via `GetIconInfo`
- **Extension icons** — associated icon by file extension, optional sharp 256px via the association's default icon
- **System stock icons** — folder, drive, recycle bin, shield, etc. (process-cached)
- **Icon enumeration** — `icon_count`, `list_icon_sizes`
- **WebP / PNG encoding** with configurable options
- **Disk + memory cache** — mtime-aware with TTL hot path (sub-µs hits)
- **Bulk parallel extraction** via rayon, **input order preserved**

## Quick Start

```rust
use win_icon_extractor::*;

// Raw RGBA pixels from a file
let icon = extract_icon(r"C:\Windows\explorer.exe").unwrap();
println!("{}x{}, {} bytes", icon.width, icon.height, icon.rgba.len());

// By index (index > 0 never silently falls back to the shell default icon)
let icon = extract_icon_at(r"C:\Windows\System32\shell32.dll", 1).unwrap();

// By size (width/height in the result always report the pixels you got)
let icon = extract_icon_with_size(r"C:\Windows\explorer.exe", 256).unwrap();

// Enumerate
let count = icon_count(r"C:\Windows\System32\shell32.dll").unwrap(); // Ok(335)
let sizes = list_icon_sizes(r"C:\Windows\System32\shell32.dll").unwrap(); // [16, …, 256]
```

## Extension Icons

The file does not need to exist. Requesting a large size resolves the
association's *default icon* (PE resource) so 256px stays sharp:

```rust
let icon = extract_icon_for_extension(".pdf").unwrap();
let icon = extract_icon_for_extension_sized(".exe", 256).unwrap();
```

## System Stock Icons

```rust
use win_icon_extractor::StockIcon;

let icon = extract_stock_icon(StockIcon::Folder).unwrap();
let icon = extract_stock_icon(StockIcon::DriveFixed).unwrap();
let icon = extract_stock_icon(StockIcon::Recycler).unwrap();
let icon = extract_stock_icon(StockIcon::Shield).unwrap();

// Custom size (values above the native large icon are GDI-scaled)
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
// (lossless: true is the pure-Rust path; lossless: false needs `webp-libwebp`)
let opts = WebPOptions {
    quality: 90.0,          // 0.0–100.0 (default: 75.0) — lossy only
    method: 6,              // 0=fast, 6=best (default: 5) — libwebp only
    lossless: true,         // default: true (icon-friendly, pure Rust)
    alpha_quality: 100,     // 0–100 (default: 100) — libwebp only
    exact: true,            // preserve RGB under transparent areas
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
use std::time::Duration;
use win_icon_extractor::{IconCache, ImageFormat, WebPOptions};

// Builder: directory + format + encode options + mtime TTL
let cache = IconCache::builder(r"D:\icon-cache")
    .format(ImageFormat::Png)
    .webp_options(WebPOptions::best_quality())
    .mtime_ttl(Duration::from_secs(30))
    .build()
    .unwrap();

// Or the app-local default
let cache = IconCache::with_app_name("my-app").unwrap();
let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();

// Size participates in the cache key
let path32 = cache.extract_to_file_sized(r"C:\Windows\explorer.exe", 32).unwrap();

// Bulk parallel extraction — results keep input order
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

Memory hits within `mtime_ttl` (default 5s) do **no** filesystem work —
ideal for UI lists that re-query the same paths every frame.

Extension icons are also process-memoized via `extract_icon_for_extension_cached`
(feature `cache`).

## High-Quality Resize Pipeline

Windows ICO / `RT_GROUP_ICON` stores **multiple frames** (16/24/32/48/256…).
GDI scaling (`extract_icon_with_size`) picks a frame and stretches — soft edges suffer.
The `resize` feature instead takes the **largest native frame** and resamples it
in premultiplied-alpha space:

```rust
use win_icon_extractor::*;

// Largest frame (often 256) → Lanczos3 → 32×32
let icon = extract_icon_best(r"C:\Windows\explorer.exe", 32)?;

// Explicit filter
let icon = extract_icon_best_with(path, 48, ResizeFilter::Box)?;        // integer ratios
let icon = extract_icon_best_with(path, 32, ResizeFilter::Mitchell)?;   // softer
let icon = extract_icon_best_with(path, 32, ResizeFilter::CatmullRom)?;
let icon = extract_icon_best_with(path, 32, ResizeFilter::Bilinear)?;

// Or resize any IconData yourself
let big = extract_icon(path)?;                 // 256×256
let s32 = big.resized_square(32, ResizeFilter::Lanczos3)?;
let custom = big.resized(64, 32, ResizeFilter::Box)?;
```

| Filter | Best for |
|--------|----------|
| `Lanczos3` (default) | General downscale, sharpest |
| `Box` | Integer ratios (256→32, 256→64) |
| `Mitchell` | Fewer ringing artifacts than Lanczos |
| `CatmullRom` | Sharp cubic |
| `Bilinear` | Fast & smooth |

Scaling uses `resize::Pixel::RGBA8P` — alpha is premultiplied during resample
and restored after, so translucent shadows do not fringe.

## Features

| Feature | Default | C dep? | Description |
|---------|---------|--------|-------------|
| `webp`  | ✓ | **no** | Lossless WebP via `image-webp` (pure Rust) |
| `webp-libwebp` | | yes | Lossy WebP + full `WebPConfig` via libwebp |
| `png`   |   | no | PNG encoding (hand-written encoder + miniz_oxide) |
| `cache` | ✓ | no | Disk + memory cache with mtime validation + TTL |
| `bulk`  | ✓ | no | Parallel extraction via rayon |
| `resize`| ✓ | no | High-quality largest-frame downscale (`extract_icon_best`) |

Default features are **100% Rust**. Opt into `webp-libwebp` only when you need
lossy WebP (`WebPOptions { lossless: false, quality, method, .. }`).

```toml
# Default — pure Rust (webp + cache + bulk + resize)
win-icon-extractor = "0.3"

# Minimal — raw extraction only
win-icon-extractor = { version = "0.3", default-features = false }

# Pure Rust + PNG
win-icon-extractor = { version = "0.3", default-features = false, features = ["webp", "png", "resize"] }

# Need lossy WebP (pulls in libwebp C sources)
win-icon-extractor = { version = "0.3", features = ["webp-libwebp"] }
```

## Semantics worth knowing

- `extract_icon_at(path, n)` with `n > 0` returns `Err` when that index cannot
  be extracted — it does **not** silently return the shell default icon.
- `icon_count` returns `Result<u32>`: `Ok(0)` means “file has no icons”.
- Paths accept `impl AsRef<Path>` (UTF-16-native `OsStr`).
- PE resource scanning (for max icon size) is skipped for `.ico` and gated on
  an `MZ` header for other extensions; the result is process-cached.
- 32bpp icons keep a real alpha channel (`GetIconInfo` + `GetDIBits`); GDI
  `DrawIconEx` is only used when scaling is required.

## Performance

Benchmarked on Windows 11, release mode:

| API | Latency |
|-----|---------|
| `icon_count` | ~25µs |
| `extract_stock_icon` (warm, process cache) | ~12µs |
| `extract_icon_for_extension` (cold) | ~267µs |
| `extract_icon_for_extension_cached` | ~7µs |
| `extract_icon` (file, max_size cached) | ~0.6–1.1ms |
| `IconCache` memory hit (within TTL) | **~250ns** |
| 50-file bulk raw serial / parallel | 93ms / 16ms (5.7×) |

## License

AGPL-3.0
