/// 将字节切片编码为 Base64 字符串。
/// 该实现为纯 Rust 手写，无需外部依赖。
pub fn encode_base64(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    // 计算编码后的字符串容量并一次性分配，避免动态扩容开销。
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut chunks = data.chunks_exact(3);
    
    while let Some(chunk) = chunks.next() {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        result.push(CHARSET[((n >> 18) & 63) as usize] as char);
        result.push(CHARSET[((n >> 12) & 63) as usize] as char);
        result.push(CHARSET[((n >> 6) & 63) as usize] as char);
        result.push(CHARSET[(n & 63) as usize] as char);
    }
    
    let remainder = chunks.remainder();
    if remainder.len() == 1 {
        let n = (remainder[0] as u32) << 16;
        result.push(CHARSET[((n >> 18) & 63) as usize] as char);
        result.push(CHARSET[((n >> 12) & 63) as usize] as char);
        result.push('=');
        result.push('=');
    } else if remainder.len() == 2 {
        let n = ((remainder[0] as u32) << 16) | ((remainder[1] as u32) << 8);
        result.push(CHARSET[((n >> 18) & 63) as usize] as char);
        result.push(CHARSET[((n >> 12) & 63) as usize] as char);
        result.push(CHARSET[((n >> 6) & 63) as usize] as char);
        result.push('=');
    }
    
    result
}

/// 将 Base64 字符映射到 6 位数值的查找表。
/// 255 表示无效字符，64 表示填充字符 '='。
const DECODE_TABLE: [u8; 256] = {
    let mut table = [255; 256];
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0;
    while i < 64 {
        table[charset[i] as usize] = i as u8;
        i += 1;
    }
    table[b'=' as usize] = 64;
    table
};

/// 解码纯 Base64 字符串为原始字节切片。
/// 支持跳过常见的空白字符（空格、换行、回车、制表符）。
pub fn decode_base64(encoded: &str) -> Result<Vec<u8>, crate::error::IconError> {
    let bytes = encoded.as_bytes();
    // 预估最大所需大小以一次性分配空间，避免频繁扩容。
    let mut result = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0;

    for &b in bytes {
        if b == b'\r' || b == b'\n' || b == b' ' || b == b'\t' {
            continue;
        }
        let val = DECODE_TABLE[b as usize];
        if val == 255 {
            return Err(crate::error::IconError::Decode(format!("无效的 Base64 字符: '{}'", b as char)));
        }
        if val == 64 {
            break;
        }
        buffer = (buffer << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Ok(result)
}

/// 解析可能带有 MIME 头部（Data URL）的 Base64 图像数据。
/// 解码后自动根据魔数或头部信息判定图片格式（返回解码数据及格式名称，如 "png" 或 "webp"）。
pub fn decode_image_base64(data_url: &str) -> Result<(Vec<u8>, String), crate::error::IconError> {
    let clean_str = data_url.trim();
    if clean_str.starts_with("data:") {
        if let Some(comma_pos) = clean_str.find(',') {
            let header = &clean_str[..comma_pos];
            let base64_part = &clean_str[comma_pos + 1..];
            
            let format = if header.contains("image/webp") {
                "webp".to_string()
            } else if header.contains("image/png") {
                "png".to_string()
            } else {
                "unknown".to_string()
            };
            
            let decoded = decode_base64(base64_part)?;
            return Ok((decoded, format));
        }
    }
    
    // 如果没有 Data URL 头部，默认作为纯 Base64 解码，并根据二进制魔数判定格式。
    let decoded = decode_base64(clean_str)?;
    let format = if decoded.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png".to_string()
    } else if decoded.starts_with(b"RIFF") && decoded.len() > 12 && &decoded[8..12] == b"WEBP" {
        "webp".to_string()
    } else {
        "unknown".to_string()
    };
    
    Ok((decoded, format))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        // 空数据测试
        assert_eq!(encode_base64(b""), "");
        
        // 长度余 1 测试
        assert_eq!(encode_base64(b"rust"), "cnVzdA==");
        
        // 长度余 2 测试
        assert_eq!(encode_base64(b"hello"), "aGVsbG8=");
        
        // 长度余 0 测试
        assert_eq!(encode_base64(b"world!"), "d29ybGQh");
        
        // 正常长字符串测试
        let original = b"The quick brown fox jumps over the lazy dog";
        let expected = "VGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIHRoZSBsYXp5IGRvZw==";
        assert_eq!(encode_base64(original), expected);
    }

    #[test]
    fn test_base64_decode() {
        // 空数据解码
        assert_eq!(decode_base64("").unwrap(), b"");
        
        // 长度填充对齐测试
        assert_eq!(decode_base64("cnVzdA==").unwrap(), b"rust");
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("d29ybGQh").unwrap(), b"world!");
        
        // 过滤空白符测试
        assert_eq!(decode_base64("cnVz\r\ndA==\n").unwrap(), b"rust");
        
        // 异常字符测试
        assert!(decode_base64("invalid_char_#").is_err());
    }

    #[test]
    fn test_decode_image_base64() {
        // 带 Data URL 头部的 WebP 测试
        let webp_url = "data:image/webp;base64,UklGRo4OAABXRUJQVlA4IIIOAABUSgCdASoAAQABAAA0Bu4Wxg==";
        let (decoded, format) = decode_image_base64(webp_url).unwrap();
        assert_eq!(format, "webp");
        assert!(decoded.starts_with(b"RIFF"));

        // 带 Data URL 头部的 PNG 测试
        let png_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAYAAABccqhmAAASPElEQVR4nO2dPXIcRw9A+wAMnDB1lQ4gx4ocMXb5ACynypwo9+aqcqrMiXIewIHP4CP4CD7CfsInjrWidgDMXwO9eKh6iUsmhzPTb/oP6PZna2cAqEkb6WIBYF8QAEBhEABAYRAAQGE=";
        let (decoded, format) = decode_image_base64(png_url).unwrap();
        assert_eq!(format, "png");
        assert!(decoded.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
