//! Hand-written PNG encoder — CRC32 lookup + miniz_oxide zlib

use crate::error::IconError;

// ── CRC32 lookup table (ISO 3309 polynomial) ──

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0u32;
    while n < 256 {
        let mut c = n;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n as usize] = c;
        n += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = make_crc_table();

fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &b in data {
        crc = CRC_TABLE[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

// ── PNG encoder ──

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    // CRC covers chunk_type + data — zero allocation
    let crc = crc32_update(crc32_update(0xFFFF_FFFF, chunk_type), data) ^ 0xFFFF_FFFF;
    out.extend_from_slice(&crc.to_be_bytes());
}

/// PNG row filter method.
#[derive(Clone, Copy, Debug, Default)]
pub enum PngFilter {
    /// No filtering (filter byte 0) — best for icons with sharp edges.
    #[default]
    None,
    /// Sub filter (filter byte 1) — better for smooth gradients.
    Sub,
}

/// PNG encoding options.
#[derive(Clone, Copy, Debug)]
pub struct PngOptions {
    /// Row filter method. Default: None.
    pub filter: PngFilter,
    /// Zlib compression level (0–10). Default: 6.
    pub compression_level: u8,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self { filter: PngFilter::None, compression_level: 6 }
    }
}

impl PngOptions {
    /// Best visual quality preset: max compression, no filter (optimal for icons).
    pub fn best_quality() -> Self {
        Self { filter: PngFilter::None, compression_level: 10 }
    }
}

/// Encode RGBA pixels to PNG bytes (default options: None filter, level 6).
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, IconError> {
    encode_png_with(rgba, width, height, &PngOptions::default())
}

/// Encode RGBA pixels to PNG with custom options.
pub fn encode_png_with(
    rgba: &[u8], width: u32, height: u32, opts: &PngOptions,
) -> Result<Vec<u8>, IconError> {
    let expected = (width as usize) * (height as usize) * 4;
    if rgba.len() != expected {
        return Err(IconError::Encode(format!(
            "RGBA buffer size mismatch: expected {expected}, got {}", rgba.len()
        )));
    }

    let mut out = Vec::with_capacity(expected / 2);

    // PNG signature
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR: width, height, bit_depth=8, color_type=6 (RGBA)
    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8;  // bit depth
    ihdr[9] = 6;  // color type: RGBA
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT: filtered row data, zlib compressed
    let stride = (width as usize) * 4;
    let mut raw = Vec::with_capacity(height as usize * (1 + stride));
    match opts.filter {
        PngFilter::None => {
            for row in rgba.chunks_exact(stride) {
                raw.push(0);
                raw.extend_from_slice(row);
            }
        }
        PngFilter::Sub => {
            for row in rgba.chunks_exact(stride) {
                raw.push(1);
                raw.extend_from_slice(&row[..4]);
                for i in 4..stride {
                    raw.push(row[i].wrapping_sub(row[i - 4]));
                }
            }
        }
    }
    let level = opts.compression_level.min(10);
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, level);
    write_chunk(&mut out, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}
