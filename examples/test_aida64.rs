use win_icon_extractor::*;
use std::fs;

fn main() {
    let exe_path = r"D:\100_Projects\110_Daily\XunYu\refer\win-icon-extractor\AIDA64-Extreme-v7.00-Lite.exe";
    let out_dir = r"D:\100_Projects\110_Daily\XunYu\refer\win-icon-extractor";

    // 提取 WebP 字节并计算其 Base64
    #[cfg(feature = "webp")]
    {
        let webp_bytes = extract_icon_webp(exe_path).unwrap();
        fs::write(format!("{}\\aida64_icon.webp", out_dir), &webp_bytes).unwrap();
        println!("已保存 WebP 图标文件。");

        let webp_base64 = encode_base64(&webp_bytes);
        fs::write(format!("{}\\aida64_webp_base64.txt", out_dir), &webp_base64).unwrap();
        println!("已保存 WebP Base64 文本。");

        // 验证解码还原 (纯 Base64 及 Data URL 两种形式)
        let (decoded_pure, format_pure) = decode_image_base64(&webp_base64).unwrap();
        assert_eq!(format_pure, "webp");
        assert_eq!(decoded_pure, webp_bytes);

        let data_url = format!("data:image/webp;base64,{}", webp_base64);
        let (decoded_url, format_url) = decode_image_base64(&data_url).unwrap();
        assert_eq!(format_url, "webp");
        assert_eq!(decoded_url, webp_bytes);
        println!("WebP Base64 还原验证成功，检测格式为: {}", format_url);
    }

    // 提取 PNG 字节并计算其 Base64
    #[cfg(feature = "png")]
    {
        let png_bytes = extract_icon_png(exe_path).unwrap();
        fs::write(format!("{}\\aida64_icon.png", out_dir), &png_bytes).unwrap();
        println!("已保存 PNG 图标文件。");

        let png_base64 = encode_base64(&png_bytes);
        fs::write(format!("{}\\aida64_png_base64.txt", out_dir), &png_base64).unwrap();
        println!("已保存 PNG Base64 文本。");

        // 验证解码还原 (纯 Base64 及 Data URL 两种形式)
        let (decoded_pure, format_pure) = decode_image_base64(&png_base64).unwrap();
        assert_eq!(format_pure, "png");
        assert_eq!(decoded_pure, png_bytes);

        let data_url = format!("data:image/png;base64,{}", png_base64);
        let (decoded_url, format_url) = decode_image_base64(&data_url).unwrap();
        assert_eq!(format_url, "png");
        assert_eq!(decoded_url, png_bytes);
        println!("PNG Base64 还原验证成功，检测格式为: {}", format_url);
    }
}
