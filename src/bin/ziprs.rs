//! ziprs - ZIP 命令行工具
//!
//! 命令格式: ziprs <zipfile> <params-file>
//!
//! 参数文件格式（二进制）:
//! - 文件数量 (4 bytes, i32, little-endian)
//! - 键总长度 (4 bytes)
//! - 键数据 (n bytes, null-terminated strings)
//! - 文件名总长度 (4 bytes)
//! - 文件名数据 (m bytes, null-terminated strings)
//! - 目录标志 (n bytes, bool vector)
//! - 修改时间 (n * 8 bytes, f64 vector)

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

// 退出码定义
const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("ziprs error: {err}");
        return ExitCode::from(EXIT_FAILURE as u8);
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // 检查参数数量
    if args.len() != 3 {
        eprintln!("Usage: {} <zipfile> <params-file>", args[0]);
        std::process::exit(EXIT_FAILURE);
    }

    let zipfile = &args[1];
    let params_file = &args[2];

    // 读取参数文件
    let params = read_params_file(params_file)?;

    // 调用核心 ZIP 功能
    create_zip(zipfile, &params)?;

    Ok(())
}

/// 参数文件数据结构
#[derive(Debug)]
struct ParamsFileData {
    keys: Vec<String>,
    filenames: Vec<String>,
    is_directory: Vec<bool>,
    mtime: Vec<f64>,
}

/// 读取二进制参数文件
fn read_params_file(path: &str) -> io::Result<ParamsFileData> {
    let mut file = File::open(path)?;

    // 读取文件数量 (4 bytes, i32, little-endian)
    let num_files = read_i32(&mut file)?;
    if num_files < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Negative file count",
        ));
    }
    let num_files = num_files as usize;

    // 读取键总长度 (4 bytes)
    let keys_len = read_i32(&mut file)?;
    if keys_len < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Negative keys length",
        ));
    }
    let keys_len = keys_len as usize;

    // 读取键数据
    let mut keys_data = vec![0u8; keys_len];
    file.read_exact(&mut keys_data)?;
    let keys = split_nul_strings(&keys_data, num_files);

    // 读取文件名总长度 (4 bytes)
    let filenames_len = read_i32(&mut file)?;
    if filenames_len < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Negative filenames length",
        ));
    }
    let filenames_len = filenames_len as usize;

    // 读取文件名数据
    let mut filenames_data = vec![0u8; filenames_len];
    file.read_exact(&mut filenames_data)?;
    let filenames = split_nul_strings(&filenames_data, num_files);

    // 读取目录标志
    let mut is_directory = vec![false; num_files];
    for i in 0..num_files {
        let mut flag = [0u8; 1];
        file.read_exact(&mut flag)?;
        is_directory[i] = flag[0] != 0;
    }

    // 读取修改时间 (f64 vector)
    let mut mtime = vec![0.0; num_files];
    for i in 0..num_files {
        let mut time_bytes = [0u8; 8];
        file.read_exact(&mut time_bytes)?;
        mtime[i] = f64::from_le_bytes(time_bytes);
    }

    Ok(ParamsFileData {
        keys,
        filenames,
        is_directory,
        mtime,
    })
}

/// 读取 i32 (little-endian)
fn read_i32<R: Read>(reader: &mut R) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

/// 分割 null 分隔的字符串
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

/// 创建 ZIP 文件
fn create_zip(zipfile: &str, params: &ParamsFileData) -> io::Result<()> {
    // 过滤出实际的文件（不是目录）
    let files: Vec<&String> = params
        .filenames
        .iter()
        .zip(params.is_directory.iter())
        .filter(|(_, &is_dir)| !is_dir)
        .map(|(f, _)| f)
        .collect();

    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No files to compress",
        ));
    }

    // 使用 ZipBuilder 创建 ZIP
    use zip_rs::zip::ZipBuilder;

    let builder = ZipBuilder::new(zipfile)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?;

    // 转换字符串切片为 &str 引用
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();

    builder
        .compression_level(zip_rs::CompressionLevel::Level6)
        .files(&file_refs)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?;

    Ok(())
}
