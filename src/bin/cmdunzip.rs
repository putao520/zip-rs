use std::env;
use std::ffi::CStr;
// use std::fs::File;
use std::io;
use std::os::raw::{c_int, c_char};
// use std::path::PathBuf;

// use zip_rs::error::ZipError;

/// 错误处理函数（与 C 版本兼容）
extern "C" fn cmd_zip_error_handler(reason: *const c_char, file: *const c_char, line: c_int, _zip_errno: c_int, eno: c_int) {
    let reason_str = unsafe { if reason.is_null() { "Unknown error" } else { &CStr::from_ptr(reason).to_string_lossy().into_owned() } };
    let file_str = unsafe { if file.is_null() { "unknown" } else { &CStr::from_ptr(file).to_string_lossy().into_owned() } };

    eprintln!("zip error: `{}` in file `{}:{}`", reason_str, file_str, line);

    let exit_code = if eno < 0 {
        -eno
    } else if eno == 0 {
        1
    } else {
        eno as i32
    };

    std::process::exit(exit_code as i32);
}

fn main() {
    if let Err(err) = run() {
        eprintln!("cmdunzip error: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // 检查参数数量
    if args.len() != 3 {
        eprintln!("Usage: {} <zip-file> <target-dir>", args[0]);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid number of arguments"));
    }

    let zip_file_path = &args[1];
    let target_dir = &args[2];

    // 调用解压功能
    if let Err(e) = extract_files(zip_file_path, target_dir) {
        eprintln!("Extraction failed: {}", e);
        return Err(io::Error::new(io::ErrorKind::Other, format!("{}", e)));
    }

    println!("Successfully extracted {} to {}", zip_file_path, target_dir);
    Ok(())
}

fn extract_files(zip_path: &str, target_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 创建目标目录（如果不存在）
    use std::fs;
    fs::create_dir_all(target_dir)?;

    // 简化的实现 - 使用现有的 Rust 解压功能
    use zip_rs::unzip::ZipArchive;
    use zip_rs::FileType;
    use std::fs::File;

    // 打开 ZIP 文件
    let mut archive = ZipArchive::open(zip_path)?;

    // 获取文件列表
    let entries = archive.entries()?;

    // 提取所有文件
    for entry in entries {
        let out_path = std::path::Path::new(target_dir)
            .join(&entry.filename);

        // 确保父目录存在
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 提取文件
        if !entry.is_directory {
            // 使用文件名来提取文件
            archive.extract_to(0, &out_path)?;
        }
    }

    println!("Successfully extracted {} to {}", zip_path, target_dir);
    Ok(())
}
