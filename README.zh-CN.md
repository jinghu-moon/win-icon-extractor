# win-icon-extractor

Windows 文件图标提取库 — 纯 Rust 实现，无 C 依赖。

- **原始 RGBA 像素** — 支持任意文件（exe、dll、ico，Shell 兜底）
- **扩展名图标** — 按文件扩展名获取关联图标，无需文件存在
- **系统预定义图标** — 文件夹、驱动器、回收站、盾牌等
- **图标枚举** — 查询 PE 文件包含的图标数量
- **WebP / PNG 编码** — 可配置选项
- **Base64 编解码** — 支持直接提取为 Base64 Data URL，并能通过自适应格式检测（PNG/WebP）将其解码还原
- **磁盘 + 内存缓存** — 基于 mtime 的过期检测
- **批量并行提取** — 基于 rayon

## 快速开始

```rust
use win_icon_extractor::*;

// 从文件提取原始 RGBA 像素
let icon = extract_icon(r"C:\Windows\explorer.exe").unwrap();
println!("{}x{}, {} bytes", icon.width, icon.height, icon.rgba.len());

// 按索引或尺寸提取
let icon = extract_icon_at(r"C:\Windows\explorer.exe", 1).unwrap();
let icon = extract_icon_with_size(r"C:\Windows\explorer.exe", 256).unwrap();

// 查询 PE 文件图标数量
let count = icon_count(r"C:\Windows\System32\shell32.dll"); // 335
```

## 扩展名图标

按文件扩展名获取关联图标，文件无需存在：

```rust
let icon = extract_icon_for_extension(".pdf").unwrap();
let icon = extract_icon_for_extension(".docx").unwrap();
```

## 系统预定义图标

提取 Windows 系统内置图标：

```rust
use win_icon_extractor::StockIcon;

let icon = extract_stock_icon(StockIcon::Folder).unwrap();
let icon = extract_stock_icon(StockIcon::DriveFixed).unwrap();
let icon = extract_stock_icon(StockIcon::Recycler).unwrap();
let icon = extract_stock_icon(StockIcon::Shield).unwrap();

// 指定尺寸
let icon = extract_stock_icon_sized(StockIcon::Folder, 48).unwrap();
```

可用图标：`Folder`（文件夹）、`FolderOpen`（打开的文件夹）、`DriveFixed`（硬盘）、`DriveRemovable`（可移动磁盘）、`DriveNet`（网络驱动器）、`DriveCd`、`DriveDvd`、`Recycler`（回收站-空）、`RecyclerFull`（回收站-满）、`Shield`（UAC 盾牌）、`Warning`、`Error`、`Info`、`Internet`、`Server`、`Printer`、`Users`、`ZipFile`、`Settings` 等。

## 编码

```rust
// WebP（feature = "webp"）
let webp = extract_icon_webp(r"C:\Windows\explorer.exe").unwrap();
std::fs::write("icon.webp", &webp).unwrap();

// PNG（feature = "png"）
let png = extract_icon_png(r"C:\Windows\explorer.exe").unwrap();
std::fs::write("icon.png", &png).unwrap();

// 编码原始 RGBA 数据
let webp = encode_webp(&icon.rgba, icon.width, icon.height).unwrap();
let png = encode_png(&icon.rgba, icon.width, icon.height).unwrap();
```

### 自定义编码选项

```rust
use win_icon_extractor::*;

// WebP：自定义质量、压缩方法、无损模式
let opts = WebPOptions {
    quality: 90.0,          // 0.0–100.0（默认：75.0）
    method: 6,              // 0=快速, 6=最佳（默认：5）
    lossless: true,         // 无损编码（默认：false）
    alpha_quality: 100,     // 0–100（默认：100）
    exact: true,            // 保留透明区域下的 RGB 值（默认：false）
    ..Default::default()
};
let webp = encode_webp_with(&icon.rgba, icon.width, icon.height, &opts).unwrap();

// WebP 最佳画质预设（无损、最大压缩力度）
let webp = encode_webp_with(&icon.rgba, icon.width, icon.height, &WebPOptions::best_quality()).unwrap();

// PNG：自定义滤波器和压缩级别
let opts = PngOptions {
    filter: PngFilter::None, // None（默认，最适合图标）或 Sub
    compression_level: 10,   // 0–10（默认：6）
};
let png = encode_png_with(&icon.rgba, icon.width, icon.height, &opts).unwrap();

// PNG 最佳画质预设（最大压缩、None 滤波器）
let png = encode_png_with(&icon.rgba, icon.width, icon.height, &PngOptions::best_quality()).unwrap();
```

## 缓存

```rust
use win_icon_extractor::{IconCache, ImageFormat};

let cache = IconCache::with_app_name("my-app").unwrap();
let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();

// 使用 PNG 格式替代 WebP
let mut cache = IconCache::with_app_name("my-app").unwrap();
cache.set_format(ImageFormat::Png);

// 自定义缓存编码选项
cache.set_webp_options(WebPOptions::best_quality());
cache.set_png_options(PngOptions::best_quality());

// 批量并行提取（每个路径返回独立 Result）
let paths = &[r"C:\Windows\System32\cmd.exe", r"C:\Windows\explorer.exe"];
let results = cache.extract_to_file_bulk(paths);
for (path, result) in &results {
    match result {
        Ok(cached) => println!("{path} → {}", cached.display()),
        Err(e) => eprintln!("{path}: {e}"),
    }
}

// 维护
let stats = cache.stats().unwrap();
cache.cleanup(30).unwrap(); // 清理 30 天前的缓存文件
```

## Base64 支持

支持直接将提取的图标转换为 Base64 编码字符串（非常适用于前端作为 Data URL 展示），同时也支持将其还原解码为二进制图像数据：

```rust
use win_icon_extractor::*;

// 直接提取为 WebP/PNG 的 Base64（已包含常见头部，可直接用于前端展示）
let webp_base64 = extract_icon_webp_base64(r"C:\Windows\explorer.exe").unwrap();
let png_base64 = extract_icon_png_base64(r"C:\Windows\explorer.exe").unwrap();

// 通用 Base64 编码
let base64_str = encode_base64(&raw_image_bytes);

// 解码还原并自适应格式识别（支持纯 Base64 或带头部的 Data URL）
let data_url = format!("data:image/webp;base64,{}", webp_base64);
let (decoded_bytes, format) = decode_image_base64(&data_url).unwrap();
assert_eq!(format, "webp");
```

## Features

| Feature | 默认启用 | 说明 |
|---------|---------|------|
| `webp`  | ✓ | WebP 编码（基于 libwebp-sys） |
| `png`   |   | PNG 编码（手写编码器 + miniz_oxide） |
| `cache` | ✓ | 磁盘 + 内存缓存，mtime 过期检测 |
| `bulk`  | ✓ | 基于 rayon 的并行提取 |

```toml
# 全部默认（webp + cache + bulk）
win-icon-extractor = "0.1"

# 最小化 — 仅原始提取
win-icon-extractor = { version = "0.1", default-features = false }

# 用 PNG 替代 WebP
win-icon-extractor = { version = "0.1", default-features = false, features = ["png", "cache"] }
```

## 性能

Windows 11，release 模式：

| API | 延迟 |
|-----|------|
| `icon_count` | ~40µs |
| `extract_stock_icon` | ~126µs |
| `extract_icon_for_extension` | ~235µs |
| `extract_icon`（文件） | ~1.6ms |
| 缓存命中 | ~1.4ms |

## 许可证

AGPL-3.0
