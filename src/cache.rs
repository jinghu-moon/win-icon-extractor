//! Configurable disk + memory icon cache

use crate::error::IconError;
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use xxhash_rust::xxh3::xxh3_64;

/// Icon cache with configurable directory and memory layer.
pub struct IconCache {
    dir: PathBuf,
    /// Memory cache: path → (cache_file, mtime_secs) for staleness detection
    mem: DashMap<String, (PathBuf, u64)>,
    /// Per-key locks to prevent concurrent duplicate extraction
    locks: DashMap<String, Arc<Mutex<()>>>,
    #[cfg(any(feature = "webp", feature = "png"))]
    format: ImageFormat,
    #[cfg(feature = "webp")]
    webp_opts: crate::encode::WebPOptions,
    #[cfg(feature = "png")]
    png_opts: crate::png::PngOptions,
}

/// Get file mtime as seconds since epoch (0 if unavailable).
#[inline]
fn file_mtime_secs(path: &str) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0)
}

/// Cache statistics.
pub struct CacheStats {
    pub total_files: usize,
    pub total_size: u64,
    pub cache_path: String,
}

/// Output image format for cached icon files.
#[cfg(any(feature = "webp", feature = "png"))]
#[derive(Clone, Copy)]
pub enum ImageFormat {
    #[cfg(feature = "webp")]
    Webp,
    #[cfg(feature = "png")]
    Png,
}

impl IconCache {
    /// Create a cache with the given directory (created if missing).
    pub fn new(dir: PathBuf) -> Result<Self, IconError> {
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(Self {
            dir,
            mem: DashMap::new(),
            locks: DashMap::new(),
            #[cfg(any(feature = "webp", feature = "png"))]
            format: Self::default_format(),
            #[cfg(feature = "webp")]
            webp_opts: Default::default(),
            #[cfg(feature = "png")]
            png_opts: Default::default(),
        })
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

    #[cfg(any(feature = "webp", feature = "png"))]
    fn default_format() -> ImageFormat {
        #[cfg(feature = "webp")]
        { ImageFormat::Webp }
        #[cfg(all(not(feature = "webp"), feature = "png"))]
        { ImageFormat::Png }
    }

    /// Cache directory path.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Cache key: hash combining path + mtime.
    fn cache_key(path: &str, mtime_secs: u64) -> String {
        let h = xxh3_64(path.as_bytes()) ^ mtime_secs.wrapping_mul(0x9E3779B97F4A7C15);
        format!("{:016x}", h)
    }

    /// Look up or extract+encode, returning the cached file path.
    /// Thread-safe: per-key lock prevents duplicate extraction.
    #[cfg(any(feature = "webp", feature = "png"))]
    pub fn extract_to_file(&self, path: &str) -> Result<PathBuf, IconError> {
        let mtime = file_mtime_secs(path);

        // Fast path: memory cache hit with mtime validation
        if let Some(entry) = self.mem.get(path) {
            let (cached_path, cached_mtime) = entry.value();
            if *cached_mtime == mtime {
                return Ok(cached_path.clone());
            }
        }

        // Acquire per-key lock (double-checked locking)
        let lock = self.locks
            .entry(path.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let lock_for_cleanup = lock.clone();
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        // Ensure lock cleanup on all exit paths (including ? early returns)
        // Only remove from map if no other thread is waiting (strong_count <= 3: map + lock + cleanup)
        let locks_ref = &self.locks;
        struct LockCleanup<'a> {
            locks: &'a DashMap<String, Arc<Mutex<()>>>,
            key: &'a str,
            arc: Arc<Mutex<()>>,
        }
        impl Drop for LockCleanup<'_> {
            fn drop(&mut self) {
                if Arc::strong_count(&self.arc) <= 3 {
                    self.locks.remove(self.key);
                }
            }
        }
        let _cleanup = LockCleanup { locks: locks_ref, key: path, arc: lock_for_cleanup };

        // Re-check after lock (with mtime)
        if let Some(entry) = self.mem.get(path) {
            let (cached_path, cached_mtime) = entry.value();
            if *cached_mtime == mtime {
                return Ok(cached_path.clone());
            }
        }

        let ext = self.format_ext();
        let file = self.dir.join(format!("{}.{ext}", Self::cache_key(path, mtime)));

        // Disk cache hit
        if file.exists() {
            self.mem.insert(path.to_string(), (file.clone(), mtime));
            return Ok(file);
        }

        // Extract → encode → write
        let data = crate::extract::extract_icon(path)?;
        let bytes = self.encode_icon(&data)?;
        fs::write(&file, &bytes)?;
        self.mem.insert(path.to_string(), (file.clone(), mtime));
        Ok(file)
    }

    #[cfg(any(feature = "webp", feature = "png"))]
    fn format_ext(&self) -> &'static str {
        match self.format {
            #[cfg(feature = "webp")]
            ImageFormat::Webp => "webp",
            #[cfg(feature = "png")]
            ImageFormat::Png => "png",
        }
    }

    #[cfg(any(feature = "webp", feature = "png"))]
    fn encode_icon(&self, data: &crate::extract::IconData) -> Result<Vec<u8>, IconError> {
        match self.format {
            #[cfg(feature = "webp")]
            ImageFormat::Webp => crate::encode::encode_webp_with(
                &data.rgba, data.width, data.height, &self.webp_opts,
            ),
            #[cfg(feature = "png")]
            ImageFormat::Png => crate::png::encode_png_with(
                &data.rgba, data.width, data.height, &self.png_opts,
            ),
        }
    }

    /// Bulk extract with caching + parallel execution.
    #[cfg(all(any(feature = "webp", feature = "png"), feature = "bulk"))]
    pub fn extract_to_file_bulk(
        &self,
        paths: &[&str],
    ) -> std::collections::HashMap<String, Result<PathBuf, IconError>> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|&p| (p.to_string(), self.extract_to_file(p)))
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
