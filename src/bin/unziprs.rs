//! unziprs - UNZIP 命令行工具
//!
//! 命令格式: unziprs <zipfile> <exdir>

use std::env;
use std::path::Path;
use std::process::ExitCode;

// 退出码定义
const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("unziprs error: {err}");
        return ExitCode::from(EXIT_FAILURE as u8);
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // 检查参数数量
    if args.len() != 3 {
        eprintln!("Usage: {} <zipfile> <exdir>", args[0]);
        std::process::exit(EXIT_FAILURE);
    }

    let zipfile = &args[1];
    let exdir = &args[2];

    // 确保解压目录存在
    if !Path::new(exdir).exists() {
        std::fs::create_dir_all(exdir)?;
    }

    // 调用核心 UNZIP 功能
    extract_zip(zipfile, exdir)?;

    Ok(())
}

/// 解压 ZIP 文件
fn extract_zip(zipfile: &str, exdir: &str) -> Result<(), Box<dyn std::error::Error>> {
    use zip_rs::unzip::Extractor;

    let extractor = Extractor::new(zipfile)
        .map_err(|e| format!("Failed to open ZIP file: {:?}", e))?;

    extractor
        .exdir(exdir)
        .extract()
        .map_err(|e| format!("Failed to extract: {:?}", e))?;

    Ok(())
}
