# win-icon-extractor

Windows 文件图标提取库 — 纯 Rust 核心。默认 feature **无 C 依赖**。

- **原始 RGBA 像素** — 支持任意文件（exe、dll、ico，Shell 兜底），`GetIconInfo` 保留真 alpha
- **扩展名图标** — 按扩展名取关联图标；指定大尺寸时走关联默认图标，256px 不糊
- **系统预定义图标** — 文件夹、驱动器、回收站、盾牌等（进程级缓存）
- **图标枚举** — `icon_count`、`list_icon_sizes`
- **WebP / PNG 编码** — 可配置选项
- **磁盘 + 内存缓存** — mtime 感知 + TTL 热路径（亚微秒命中）
- **批量并行提取** — 基于 rayon，**保持输入顺序**

## 快速开始

```rust
use win_icon_extractor::*;

// 从文件提取原始 RGBA 像素
let icon = extract_icon(r"C:\Windows\explorer.exe").unwrap();
println!("{}x{}, {} bytes", icon.width, icon.height, icon.rgba.len());

// 按索引提取（index > 0 失败时不会静默退回 shell 默认图标）
let icon = extract_icon_at(r"C:\Windows\System32\shell32.dll", 1).unwrap();

// 按尺寸提取（返回的 width/height 始终是实际像素）
let icon = extract_icon_with_size(r"C:\Windows\explorer.exe", 256).unwrap();

// 枚举
let count = icon_count(r"C:\Windows\System32\shell32.dll").unwrap(); // Ok(335)
let sizes = list_icon_sizes(r"C:\Windows\System32\shell32.dll").unwrap(); // [16, …, 256]
```

## 扩展名图标

文件无需存在。请求大尺寸时会解析关联的**默认图标**（PE 资源），256px 保持清晰：

```rust
let icon = extract_icon_for_extension(".pdf").unwrap();
let icon = extract_icon_for_extension_sized(".exe", 256).unwrap();
```

## 系统预定义图标

```rust
use win_icon_extractor::StockIcon;

let icon = extract_stock_icon(StockIcon::Folder).unwrap();
let icon = extract_stock_icon(StockIcon::DriveFixed).unwrap();
let icon = extract_stock_icon(StockIcon::Recycler).unwrap();
let icon = extract_stock_icon(StockIcon::Shield).unwrap();

// 指定尺寸（超过原生大图标时由 GDI 缩放）
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
use std::time::Duration;
use win_icon_extractor::{IconCache, ImageFormat, WebPOptions};

// Builder：目录 + 格式 + 编码选项 + mtime TTL
let cache = IconCache::builder(r"D:\icon-cache")
    .format(ImageFormat::Png)
    .webp_options(WebPOptions::best_quality())
    .mtime_ttl(Duration::from_secs(30))
    .build()
    .unwrap();

// 或应用本地默认目录
let cache = IconCache::with_app_name("my-app").unwrap();
let path = cache.extract_to_file(r"C:\Windows\explorer.exe").unwrap();

// 尺寸参与缓存 key
let path32 = cache.extract_to_file_sized(r"C:\Windows\explorer.exe", 32).unwrap();

// 批量并行提取 — 结果保持输入顺序
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

`mtime_ttl`（默认 5 秒）内的内存命中**完全不碰文件系统** — 适合 UI 列表每帧重复查询。

扩展名图标另有进程级缓存：`extract_icon_for_extension_cached`（feature `cache`）。

## 高质量缩放管线

Windows ICO / `RT_GROUP_ICON` 是**多尺寸帧合集**（16/24/32/48/256…）。
GDI 缩放（`extract_icon_with_size`）会选一帧再拉伸，软边容易糊。
`resize` 特性改为取**最大原生帧**，在预乘 alpha 空间重采样：

```rust
use win_icon_extractor::*;

// 最大帧（常为 256）→ Lanczos3 → 32×32
let icon = extract_icon_best(r"C:\Windows\explorer.exe", 32)?;

// 指定滤波器
let icon = extract_icon_best_with(path, 48, ResizeFilter::Box)?;        // 整数倍缩放
let icon = extract_icon_best_with(path, 32, ResizeFilter::Mitchell)?;   // 更柔
let icon = extract_icon_best_with(path, 32, ResizeFilter::CatmullRom)?;
let icon = extract_icon_best_with(path, 32, ResizeFilter::Bilinear)?;

// 或对任意 IconData 自行缩放
let big = extract_icon(path)?;                 // 256×256
let s32 = big.resized_square(32, ResizeFilter::Lanczos3)?;
let custom = big.resized(64, 32, ResizeFilter::Box)?;
```

| 滤波器 | 适用 |
|--------|------|
| `Lanczos3`（默认） | 通用缩小，最锐利 |
| `Box` | 整数倍（256→32、256→64） |
| `Mitchell` | 比 Lanczos 更少振铃 |
| `CatmullRom` | 锐利三次 |
| `Bilinear` | 快速平滑 |

缩放使用 `resize::Pixel::RGBA8P`：过程中预乘 alpha、结束后还原，半透明阴影不会出毛边。

## Features

| Feature | 默认启用 | C 依赖 | 说明 |
|---------|---------|--------|------|
| `webp`  | ✓ | **无** | 无损 WebP（`image-webp`，纯 Rust） |
| `webp-libwebp` | | 有 | 有损 WebP + 完整 `WebPConfig`（libwebp） |
| `png`   |   | 无 | PNG 编码（手写编码器 + miniz_oxide） |
| `cache` | ✓ | 无 | 磁盘 + 内存缓存，mtime + TTL 过期检测 |
| `bulk`  | ✓ | 无 | 基于 rayon 的并行提取 |
| `resize`| ✓ | 无 | 高质量最大帧缩放（`extract_icon_best`） |

默认 feature **100% Rust**。只有需要有损 WebP
（`WebPOptions { lossless: false, quality, .. }`）时才启用 `webp-libwebp`。

```toml
# 默认 — 纯 Rust（webp + cache + bulk + resize）
win-icon-extractor = "0.3"

# 最小化 — 仅原始提取
win-icon-extractor = { version = "0.3", default-features = false }

# 纯 Rust + PNG
win-icon-extractor = { version = "0.3", default-features = false, features = ["webp", "png", "resize"] }

# 需要有损 WebP（会拉入 libwebp C 源码）
win-icon-extractor = { version = "0.3", features = ["webp-libwebp"] }
```

## 语义说明

- `extract_icon_at(path, n)` 在 `n > 0` 且取不到该索引时返回 `Err`，**不会**静默退回 shell 默认图标。
- `icon_count` 返回 `Result<u32>`：`Ok(0)` 表示「文件没有图标」。
- 路径参数为 `impl AsRef<Path>`（原生 UTF-16 `OsStr`）。
- `.ico` 不会做 PE 资源扫描；其他扩展名以 `MZ` 头判定后再扫描，结果进程级缓存。
- 32bpp 图标通过 `GetIconInfo` + `GetDIBits` 保留真 alpha；仅在需要缩放时才走 GDI `DrawIconEx`。

## 性能

Windows 11，release 模式：

| API | 延迟 |
|-----|------|
| `icon_count` | ~25µs |
| `extract_stock_icon`（热，进程缓存） | ~12µs |
| `extract_icon_for_extension`（冷） | ~267µs |
| `extract_icon_for_extension_cached` | ~7µs |
| `extract_icon`（文件，max_size 已缓存） | ~0.6–1.1ms |
| `IconCache` 内存命中（TTL 内） | **~250ns** |
| 50 文件批量 串行 / 并行 | 93ms / 16ms（5.7×） |

## 许可证

AGPL-3.0
