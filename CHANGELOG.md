# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] — 2026-05

### Highlights

- **Pure-Rust default WebP** (`image-webp`): default features now contain **no C dependency**.
- **High-quality resize pipeline**: extract the largest native frame, then Lanczos/Box/Mitchell downscale in premultiplied-alpha space.
- **Cache hot path ~150 ns** (was ~1.4 ms): mtime TTL, richer cache keys, ordered bulk.
- True alpha via `GetIconInfo` + `GetDIBits`; GDI `DrawIconEx` only for scaling.

### Breaking

- `icon_count` now returns `Result<u32, IconError>` (`Ok(0)` = no icons).
- `extract_icon_at(path, n)` with `n > 0` no longer falls back to the shell default icon — it returns `Err`.
- `extract_icons_bulk` / `extract_to_file_bulk` return `Vec<(String, Result<…>)>` (input order preserved) instead of `HashMap`.
- Path arguments are `impl AsRef<Path>` (UTF-16-native `OsStr`); `&str` still works via deref.
- Feature `webp` is now pure-Rust lossless (`image-webp`). For the previous libwebp backend (lossy), enable **`webp-libwebp`**.
- `WebPOptions::lossless` defaults to `true`. `lossless: false` without `webp-libwebp` returns an error instead of silently encoding lossless.
- `IconError` is `#[non_exhaustive]`.

### Added

- `extract_icon_best` / `extract_icon_best_with` — largest frame → high-quality resample (feature `resize`).
- `resize_rgba`, `resize_icon`, `IconData::resized` / `resized_square`, `ResizeFilter` (Lanczos3 / Mitchell / CatmullRom / Bilinear / Box).
- `list_icon_sizes` / `list_icon_sizes_wide` — enumerate declared icon sizes.
- `extract_icon_for_extension_sized` — sharp large icons via association default icon.
- `extract_icon_for_extension_cached` — process-level extension icon memo (feature `cache`).
- `IconCache::builder()` with `format` / encode options / `mtime_ttl`; `extract_to_file_sized`.
- Stock icons and PE max-size are process-cached.
- Criterion benchmarks (`benches/extract.rs`), GitHub Actions CI, unit tests.

### Changed

- PE detection: `.ico` skips resource scan; other extensions gated on an `MZ` header.
- `windows` crate 0.58 → 0.61.
- Defaults: `webp + cache + bulk + resize` (all pure Rust).

## [0.2.0] — 2026-05-22

- Published feature set: `webp` (libwebp-sys), `png`, `cache`, `bulk`.
- See crates.io for the 0.2.0 tree.

## [0.1.0] — 2026-02-21

- Initial release.

[0.3.0]: https://github.com/jinghu-moon/win-icon-extractor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jinghu-moon/win-icon-extractor/releases/tag/v0.2.0
[0.1.0]: https://github.com/jinghu-moon/win-icon-extractor/releases/tag/v0.1.0
