use std::env;
use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::raw::{c_int, c_char};

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
        eprintln!("cmdzip error: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // 检查参数数量
    if args.len() != 3 {
        eprintln!("Usage: {} <zip-file> <input-file>", args[0]);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid number of arguments"));
    }

    let zip_file_path = &args[1];
    let input_file_path = &args[2];

    // 调用压缩功能
    if let Err(e) = compress_file(zip_file_path, input_file_path) {
        eprintln!("Compression failed: {}", e);
        return Err(io::Error::new(io::ErrorKind::Other, format!("{}", e)));
    }

    println!("Successfully compressed {} to {}", input_file_path, zip_file_path);
    Ok(())
}

fn compress_file(zip_path: &str, input_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 打开输入文件
    let input_file = match File::open(input_path) {
        Ok(file) => file,
        Err(e) => return Err(Box::new(e)),
    };

    // 读取文件信息
    let file_metadata = input_file.metadata()?;
    let file_size = file_metadata.len();

    // 创建 ZIP 归档（简化版本，直接使用 C FFI）
    println!("Compressing {} to {} (size: {} bytes)", input_path, zip_path, file_size);

    // 使用 Rust 的 builder API
    use zip_rs::zip::ZipBuilder;

    // 使用 ZipBuilder 创建 ZIP
    let builder = ZipBuilder::new(zip_path)?;
    builder.compression_level(zip_rs::CompressionLevel::Level6)
           .files(&[input_path])?
           .build()?;

    Ok(())
}

fn read_i32<R: Read>(reader: &mut R) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(i32::from_ne_bytes(buf))
}

fn read_f64<R: Read>(reader: &mut R) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(f64::from_ne_bytes(buf))
}

fn read_bytes<R: Read>(reader: &mut R, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn split_nul_strings(buf: &[u8], expected: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(expected);
    let mut start = 0;
    for _ in 0..expected {
        if let Some(end) = buf[start..].iter().position(|b| *b == 0) {
            let slice = &buf[start..start + end];
            out.push(String::from_utf8_lossy(slice).to_string());
            start += end + 1;
        } else {
            out.push(String::new());
            break;
        }
    }
    out
}
