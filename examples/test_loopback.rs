use win_icon_extractor::*;
use std::fs;
use std::path::Path;

fn main() {
    let exe_path = r"D:\100_Projects\110_Daily\XunYu\refer\win-icon-extractor\AIDA64-Extreme-v7.00-Lite.exe";
    let out_dir = Path::new(r"D:\100_Projects\110_Daily\XunYu\refer\win-icon-extractor");
    
    println!("=== 开始测试回环：exe → png/webp → base64 → png/webp ===");
    println!("源文件: {}\n", exe_path);

    // 1. 测试 WebP 回环
    #[cfg(feature = "webp")]
    {
        println!("[WebP 阶段]");
        // exe → webp
        let webp_initial = extract_icon_webp(exe_path).expect("提取 WebP 二进制失败");
        let initial_path = out_dir.join("loopback_initial.webp");
        fs::write(&initial_path, &webp_initial).unwrap();
        println!("  1. 已从 exe 提取并保存初始 WebP (大小: {} 字节)", webp_initial.len());

        // webp → base64
        let webp_base64 = encode_base64(&webp_initial);
        println!("  2. 已将 WebP 转换为 Base64 字符串 (长度: {} 字符)", webp_base64.len());

        // base64 → webp
        let data_url = format!("data:image/webp;base64,{}", webp_base64);
        let (webp_restored, format) = decode_image_base64(&data_url).expect("解码 WebP Data URL 失败");
        println!("  3. 已成功解码还原 (识别格式为: {})", format);

        // 验证一致性
        assert_eq!(format, "webp", "识别格式不为 webp");
        assert_eq!(webp_initial, webp_restored, "WebP 还原前后二进制数据不一致！");
        println!("  4. 字节一致性校验成功！(还原大小: {} 字节)", webp_restored.len());

        // 保存还原的 WebP
        let restored_path = out_dir.join("loopback_restored.webp");
        fs::write(&restored_path, &webp_restored).unwrap();
        println!("  5. 已保存还原的 WebP 图标。\n");
    }

    // 2. 测试 PNG 回环
    #[cfg(feature = "png")]
    {
        println!("[PNG 阶段]");
        // exe → png
        let png_initial = extract_icon_png(exe_path).expect("提取 PNG 二进制失败");
        let initial_path = out_dir.join("loopback_initial.png");
        fs::write(&initial_path, &png_initial).unwrap();
        println!("  1. 已从 exe 提取并保存初始 PNG (大小: {} 字节)", png_initial.len());

        // png → base64
        let png_base64 = encode_base64(&png_initial);
        println!("  2. 已将 PNG 转换为 Base64 字符串 (长度: {} 字符)", png_base64.len());

        // base64 → png
        let data_url = format!("data:image/png;base64,{}", png_base64);
        let (png_restored, format) = decode_image_base64(&data_url).expect("解码 PNG Data URL 失败");
        println!("  3. 已成功解码还原 (识别格式为: {})", format);

        // 验证一致性
        assert_eq!(format, "png", "识别格式不为 png");
        assert_eq!(png_initial, png_restored, "PNG 还原前后二进制数据不一致！");
        println!("  4. 字节一致性校验成功！(还原大小: {} 字节)", png_restored.len());

        // 保存还原的 PNG
        let restored_path = out_dir.join("loopback_restored.png");
        fs::write(&restored_path, &png_restored).unwrap();
        println!("  5. 已保存还原的 PNG 图标。\n");
    }

    println!("=== 回环测试全部通过！原始提取数据与解码还原数据 100% 一致 ===");
}
