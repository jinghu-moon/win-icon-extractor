//! Configurable disk + memory icon cache

use crate::error::IconError;
use dashmap::DashMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use xxhash_rust::xxh3::Xxh3;

/// Icon cache with configurable directory and memory layer.
pub struct IconCache {
    dir: PathBuf,
    /// Memory cache: source path → entry (validates via mtime with TTL)
    mem: DashMap<PathBuf, CacheEntry>,
    /// Per-key locks to prevent concurrent duplicate extraction
    locks: DashMap<PathBuf, Arc<Mutex<()>>>,
    #[cfg(any(feature = "webp", feature = "png"))]
    format: ImageFormat,
    #[cfg(feature = "webp")]
    webp_opts: crate::encode::WebPOptions,
    #[cfg(feature = "png")]
    png_opts: crate::png::PngOptions,
    /// Skip fs::mtime while an entry is younger than this (hot path).
    mtime_ttl: Duration,
}

#[derive(Clone)]
struct CacheEntry {
    file: PathBuf,
    mtime: u64,
    /// When we last confirmed `mtime` against the filesystem.
    validated_at: Instant,
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_files: usize,
    pub total_size: u64,
    pub cache_path: String,
}

/// Output image format for cached icon files.
#[cfg(any(feature = "webp", feature = "png"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    #[cfg(feature = "webp")]
    Webp,
    #[cfg(feature = "png")]
    Png,
}

/// Get file mtime as seconds since epoch (0 if unavailable).
#[inline]
fn file_mtime_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0)
}

// ── Extension icon process cache ──

type ExtCacheMap = Mutex<HashMap<(String, u32), Arc<IconData>>>;

fn ext_icon_cache() -> &'static ExtCacheMap {
    static CACHE: OnceLock<ExtCacheMap> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Extract (and memoize) the associated icon for an extension at a given size.
/// Size 0 = system large icon. Shared across the whole process.
pub fn extract_icon_for_extension_cached(ext: &str, size: u32) -> Result<IconData, IconError> {
    let key = ext.to_ascii_lowercase();
    let size_key = size.min(256);
    {
        let map = ext_icon_cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(data) = map.get(&(key.clone(), size_key)) {
            return Ok((**data).clone());
        }
    }
    let data = crate::extract::extract_icon_for_extension_sized(ext, size)?;
    let mut map = ext_icon_cache().lock().unwrap_or_else(|e| e.into_inner());
    let arc = Arc::new(data);
    map.insert((key, size_key), arc.clone());
    Ok((*arc).clone())
}

type IconData = crate::extract::IconData;

impl IconCache {
    /// Start a builder rooted at `dir`.
    pub fn builder(dir: impl Into<PathBuf>) -> IconCacheBuilder {
        IconCacheBuilder {
            dir: dir.into(),
            #[cfg(any(feature = "webp", feature = "png"))]
            format: Self::default_format(),
            #[cfg(feature = "webp")]
            webp_opts: Default::default(),
            #[cfg(feature = "png")]
            png_opts: Default::default(),
            mtime_ttl: Duration::from_secs(5),
        }
    }

    /// Create a cache with the given directory (created if missing).
    pub fn new(dir: PathBuf) -> Result<Self, IconError> {
        Self::builder(dir).build()
    }

    /// Default cache under `%LOCALAPPDATA%/<app_name>/icon_cache`.
    pub fn with_app_name(app_name: &str) -> Result<Self, IconError> {
        let base = std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .map_err(|e| IconError::Cache(format!("env var: {e}")))?;
        Self::new(PathBuf::from(base).join(app_name).join("icon_cache"))
    }

    /// Set custom WebP encoding options.
    #[cfg(feature = "webp")]
    pub fn set_webp_options(&mut self, opts: crate::encode::WebPOptions) {
        self.webp_opts = opts;
    }

    /// Set custom PNG encoding options.
    #[cfg(feature = "png")]
    pub fn set_png_options(&mut self, opts: crate::png::PngOptions) {
        self.png_opts = opts;
    }

    /// Set output image format for cached files.
    #[cfg(any(feature = "webp", feature = "png"))]
    pub fn set_format(&mut self, format: ImageFormat) {
        self.format = format;
    }

    /// How long a memory-cache entry is trusted without an `fs::metadata` revalidation.
    pub fn set_mtime_ttl(&mut self, ttl: Duration) {
        self.mtime_ttl = ttl;
    }

    #[cfg(any(feature = "webp", feature = "png"))]
    fn default_format() -> ImageFormat {
        #[cfg(feature = "webp")]
        {
            ImageFormat::Webp
        }
        #[cfg(all(not(feature = "webp"), feature = "png"))]
        {
            ImageFormat::Png
        }
    }

    /// Cache directory path.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Stable digest of the active encode settings (included in the file name).
    fn opts_digest(&self) -> u64 {
        let mut h = Xxh3::new();
        #[cfg(feature = "webp")]
        {
            if matches!(self.format, ImageFormat::Webp) {
                h.update(b"webp");
                h.update(&self.webp_opts.quality.to_bits().to_le_bytes());
                h.update(&self.webp_opts.method.to_le_bytes());
                h.update(&[self.webp_opts.lossless as u8, self.webp_opts.exact as u8]);
                h.update(&self.webp_opts.alpha_quality.to_le_bytes());
            }
        }
        #[cfg(feature = "png")]
        {
            if matches!(self.format, ImageFormat::Png) {
                h.update(b"png");
                h.update(&[self.png_opts.compression_level]);
                h.update(&[self.png_opts.filter as u8]);
            }
        }
        #[cfg(feature = "webp")]
        {
            if matches!(self.format, ImageFormat::Webp) {
                return h.digest();
            }
        }
        #[cfg(feature = "png")]
        {
            if matches!(self.format, ImageFormat::Png) {
                return h.digest();
            }
        }
        #[allow(unreachable_code)]
        h.digest()
    }

    /// Cache key: hash of path + mtime + size + format + encode options.
    fn cache_key(
        path: &Path,
        mtime_secs: u64,
        size: u32,
        format_tag: u8,
        opts_hash: u64,
    ) -> String {
        let mut h = Xxh3::new();
        h.update(path.to_string_lossy().as_bytes());
        h.update(&mtime_secs.to_le_bytes());
        h.update(&size.to_le_bytes());
        h.update(&[format_tag]);
        h.update(&opts_hash.to_le_bytes());
        format!("{:016x}", h.digest())
    }

    #[cfg(any(feature = "webp", feature = "png"))]
    fn format_tag_and_ext(&self) -> (u8, &'static str) {
        match self.format {
            #[cfg(feature = "webp")]
            ImageFormat::Webp => (1, "webp"),
            #[cfg(feature = "png")]
            ImageFormat::Png => (2, "png"),
        }
    }

    /// Look up or extract+encode, returning the cached file path.
    ///
    /// Hot path (memory hit within `mtime_ttl`) does no filesystem work.
    #[cfg(any(feature = "webp", feature = "png"))]
    pub fn extract_to_file(&self, path: impl AsRef<Path>) -> Result<PathBuf, IconError> {
        self.extract_to_file_sized(path, 0)
    }

    /// Like [`extract_to_file`](Self::extract_to_file) but selects a target size
    /// (0 = default/largest). The size participates in the cache key.
    #[cfg(any(feature = "webp", feature = "png"))]
    pub fn extract_to_file_sized(
        &self,
        path: impl AsRef<Path>,
        size: u32,
    ) -> Result<PathBuf, IconError> {
        let path = path.as_ref();
        let size = size.min(256);

        // Fast path: fresh memory hit — no stat.
        if let Some(entry) = self.mem.get(path) {
            if entry.validated_at.elapsed() < self.mtime_ttl {
                return Ok(entry.file.clone());
            }
        }

        let mtime = file_mtime_secs(path);

        // Memory hit with mtime check (slow path / TTL expired).
        if let Some(entry) = self.mem.get(path) {
            let entry = entry.value();
            if entry.mtime == mtime {
                // refresh TTL
                self.mem.insert(
                    path.to_path_buf(),
                    CacheEntry {
                        file: entry.file.clone(),
                        mtime,
                        validated_at: Instant::now(),
                    },
                );
                return Ok(entry.file.clone());
            }
        }

        // Acquire per-key lock (double-checked locking)
        let lock = self
            .locks
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let lock_for_cleanup = lock.clone();
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        struct LockCleanup<'a> {
            locks: &'a DashMap<PathBuf, Arc<Mutex<()>>>,
            key: &'a Path,
            arc: Arc<Mutex<()>>,
        }
        impl Drop for LockCleanup<'_> {
            fn drop(&mut self) {
                if Arc::strong_count(&self.arc) <= 3 {
                    self.locks.remove(self.key);
                }
            }
        }
        let _cleanup = LockCleanup {
            locks: &self.locks,
            key: path,
            arc: lock_for_cleanup,
        };

        // Re-check after lock (fresh hit within TTL).
        if let Some(entry) = self.mem.get(path) {
            let e = entry.value();
            if e.validated_at.elapsed() < self.mtime_ttl || e.mtime == mtime {
                let file = e.file.clone();
                drop(entry);
                self.mem.insert(
                    path.to_path_buf(),
                    CacheEntry {
                        file: file.clone(),
                        mtime,
                        validated_at: Instant::now(),
                    },
                );
                return Ok(file);
            }
        }

        let (fmt_tag, ext) = self.format_tag_and_ext();
        let opts_hash = self.opts_digest();
        let file = self.dir.join(format!(
            "{}.{ext}",
            Self::cache_key(path, mtime, size, fmt_tag, opts_hash)
        ));

        // Disk cache hit
        if file.exists() {
            self.mem.insert(
                path.to_path_buf(),
                CacheEntry {
                    file: file.clone(),
                    mtime,
                    validated_at: Instant::now(),
                },
            );
            return Ok(file);
        }

        // Extract → encode → write
        let data = if size > 0 {
            crate::extract::extract_icon_with_size(path, size)?
        } else {
            crate::extract::extract_icon(path)?
        };
        let bytes = self.encode_icon(&data)?;
        fs::write(&file, &bytes)?;
        self.mem.insert(
            path.to_path_buf(),
            CacheEntry {
                file: file.clone(),
                mtime,
                validated_at: Instant::now(),
            },
        );
        Ok(file)
    }

    #[cfg(any(feature = "webp", feature = "png"))]
    fn encode_icon(&self, data: &IconData) -> Result<Vec<u8>, IconError> {
        match self.format {
            #[cfg(feature = "webp")]
            ImageFormat::Webp => crate::encode::encode_webp_with(
                &data.rgba,
                data.width,
                data.height,
                &self.webp_opts,
            ),
            #[cfg(feature = "png")]
            ImageFormat::Png => {
                crate::png::encode_png_with(&data.rgba, data.width, data.height, &self.png_opts)
            }
        }
    }

    /// Bulk extract with caching + parallel execution.
    /// Results are returned in **input order**.
    #[cfg(all(any(feature = "webp", feature = "png"), feature = "bulk"))]
    pub fn extract_to_file_bulk(
        &self,
        paths: &[&str],
    ) -> Vec<(String, Result<PathBuf, IconError>)> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|p| (p.to_string(), self.extract_to_file(*p)))
            .collect()
    }

    /// Clear memory cache.
    pub fn clear_memory(&self) {
        self.mem.clear();
    }

    /// Cache statistics.
    pub fn stats(&self) -> Result<CacheStats, IconError> {
        let (total_files, total_size) = fs::read_dir(&self.dir)?
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .fold((0, 0u64), |(c, s), m| (c + 1, s + m.len()));
        Ok(CacheStats {
            total_files,
            total_size,
            cache_path: self.dir.to_string_lossy().into(),
        })
    }

    /// Remove cache files older than `max_age_days`.
    pub fn cleanup(&self, max_age_days: u64) -> Result<(), IconError> {
        let max_age = Duration::from_secs(max_age_days * 86400);
        let now = SystemTime::now();
        for entry in fs::read_dir(&self.dir)?.flatten() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if now.duration_since(modified).unwrap_or_default() > max_age {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
        self.clear_memory();
        Ok(())
    }
}

/// Builder for [`IconCache`].
pub struct IconCacheBuilder {
    dir: PathBuf,
    #[cfg(any(feature = "webp", feature = "png"))]
    format: ImageFormat,
    #[cfg(feature = "webp")]
    webp_opts: crate::encode::WebPOptions,
    #[cfg(feature = "png")]
    png_opts: crate::png::PngOptions,
    mtime_ttl: Duration,
}

impl IconCacheBuilder {
    #[cfg(any(feature = "webp", feature = "png"))]
    pub fn format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    #[cfg(feature = "webp")]
    pub fn webp_options(mut self, opts: crate::encode::WebPOptions) -> Self {
        self.webp_opts = opts;
        self
    }

    #[cfg(feature = "png")]
    pub fn png_options(mut self, opts: crate::png::PngOptions) -> Self {
        self.png_opts = opts;
        self
    }

    /// Trust memory-cache entries for this long before revalidating mtime.
    pub fn mtime_ttl(mut self, ttl: Duration) -> Self {
        self.mtime_ttl = ttl;
        self
    }

    pub fn build(self) -> Result<IconCache, IconError> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir)?;
        }
        Ok(IconCache {
            dir: self.dir,
            mem: DashMap::new(),
            locks: DashMap::new(),
            #[cfg(any(feature = "webp", feature = "png"))]
            format: self.format,
            #[cfg(feature = "webp")]
            webp_opts: self.webp_opts,
            #[cfg(feature = "png")]
            png_opts: self.png_opts,
            mtime_ttl: self.mtime_ttl,
        })
    }
}
