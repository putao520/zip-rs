//! 流式添加功能模块
//!
//! 参考 C 版本 `mz_zip_writer_add_cfile` 的实现

use crate::error::{ZipError, ZipResult};
use crate::zip::writer::ZipWriter;
use std::fs::File;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};

/// 流式添加文件到 ZIP 归档
///
/// # 参数
/// * `writer` - ZIP 写入器
/// * `archive_name` - ZIP 归档中的文件名
/// * `input_file` - 输入文件句柄
/// * `size_to_add` - 要添加的文件大小
/// * `file_time` - 文件时间（可选）
/// * `comment` - 文件注释（可选）
/// * `comment_size` - 注释大小
/// * `level_and_flags` - 压缩级别和标志
/// * `user_extra_data` - 用户额外数据（可选）
/// * `user_extra_data_len` - 用户额外数据长度
/// * `user_extra_data_central` - 中央目录的用户额外数据（可选）
/// * `user_extra_data_central_len` - 中央目录的用户额外数据长度
///
/// # 返回
/// 添加成功返回 Ok(())，失败返回错误
pub fn add_file(
    writer: &mut ZipWriter,
    archive_name: &str,
    input_file: &mut File,
    size_to_add: u64,
    file_time: Option<SystemTime>,
    comment: Option<&str>,
    comment_size: u16,
    level_and_flags: u32,
    user_extra_data: Option<&[u8]>,
    user_extra_data_len: u32,
    user_extra_data_central: Option<&[u8]>,
    user_extra_data_central_len: u32,
) -> ZipResult<()> {
    // 检查参数有效性
    if archive_name.is_empty() {
        return Err(ZipError::InvalidFilename("Archive name cannot be empty".to_string()));
    }

    if comment_size > 0 && comment.is_none() {
        return Err(ZipError::InvalidComment("Comment provided but comment is None".to_string()));
    }

    if size_to_add > u64::MAX / 2 {
        return Err(ZipError::ArchiveTooLarge("File size too large".to_string()));
    }

    // 设置压缩级别
    let level = level_and_flags & 0xF;
    if level > 9 {
        return Err(ZipError::InvalidCompressionLevel("Invalid compression level".to_string()));
    }

    // 处理文件名编码
    let archive_name_bytes = archive_name.as_bytes();
    let archive_name_size = archive_name_bytes.len();

    if archive_name_size > u16::MAX as usize {
        return Err(ZipError::InvalidFilename("Archive name too long".to_string()));
    }

    // 验证归档名称
    if !validate_archive_name(archive_name) {
        return Err(ZipError::InvalidFilename("Invalid archive name".to_string()));
    }

    // 创建 ZIP64 必要时的检查
    if writer.is_zip64_needed(size_to_add) {
        writer.enable_zip64();
    }

    // 检查文件数量限制
    if writer.get_total_files() >= writer.get_max_files() {
        return Err(ZipError::TooManyFiles("Too many files in archive".to_string()));
    }

    // 设置通用标志
    let mut gen_flags = 0;
    if (level_and_flags & 0x100) == 0 { // MZ_ZIP_FLAG_ASCII_FILENAME
        gen_flags |= 0x0800; // MZ_ZIP_LDH_BIT_FLAG_HAS_LOCATOR
    }

    // 添加 UTF-8 标志（如果文件名包含非 ASCII 字符）
    if !archive_name.is_ascii() {
        gen_flags |= 0x0800; // MZ_ZIP_LDH_BIT_FLAG_HAS_LOCATOR
    }

    // 计算压缩方法（这里简化处理，实际需要检测是否已压缩）
    let method = 0; // 0 = 存储，8 = DEFLATE

    // 准备文件时间
    let dos_time = convert_to_dos_time(file_time.unwrap_or_else(SystemTime::now()))?;

    // 计算额外数据
    let extra_data = prepare_extra_data(
        size_to_add,
        user_extra_data,
        user_extra_data_len,
        user_extra_data_central,
        user_extra_data_central_len,
    )?;

    // 添加文件到 ZIP 归档
    add_file_to_archive(
        writer,
        archive_name,
        input_file,
        size_to_add,
        file_time,
        dos_time,
        method,
        gen_flags,
        comment,
        comment_size,
        &extra_data,
        level,
    )
}

/// 验证归档名称
fn validate_archive_name(name: &str) -> bool {
    // 检查无效字符
    for (i, ch) in name.chars().enumerate() {
        if i == 0 && ch == '/' {
            return false; // 不能以 / 开头
        }

        if ch == '\\' || ch == ':' {
            return false; // Windows 路径分隔符
        }

        // 其他无效字符检查
        if ch as u32 <= 0x1F {
            return false;
        }
    }

    true
}

/// 转换时间为 DOS 格式
fn convert_to_dos_time(time: SystemTime) -> ZipResult<u16> {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds_since_epoch = duration.as_secs();

    // 计算日期和时间
    let days_since_epoch = seconds_since_epoch / 86400;
    let seconds_in_day = seconds_since_epoch % 86400;

    // DOS 日期：从 1980-01-01 开始的天数
    let mut dos_date = days_since_epoch - 25567; // 1980-01-01 到 1970-01-01 的天数
    if dos_date > 365 * 127 { // 127 年
        dos_date = 365 * 127;
    }
    dos_date = dos_date.min(365 * 127);

    // DOS 时间：从午夜开始的秒数除以 2
    let dos_time = seconds_in_day / 2;

    // 组合成 DOS 时间格式
    let dos_time_word = ((dos_date as u16) << 9) | ((dos_time as u16) & 0x1FFF);

    Ok(dos_time_word)
}

/// 准备额外数据
fn prepare_extra_data(
    size_to_add: u64,
    user_extra_data: Option<&[u8]>,
    user_extra_data_len: u32,
    user_extra_data_central: Option<&[u8]>,
    user_extra_data_central_len: u32,
) -> ZipResult<Vec<u8>> {
    let mut extra_data = Vec::new();

    // 添加 ZIP64 额外数据（如果需要）
    if size_to_add > u32::MAX as u64 {
        // 添加 ZIP64 额外数据头
        extra_data.extend_from_slice(&[0x01, 0x00]); // ZIP64 额外数据头 ID
        extra_data.extend_from_slice(&[0x10, 0x00]); // 数据大小（16字节）

        // 添加 ZIP64 原始大小
        extra_data.extend_from_slice(&size_to_add.to_le_bytes());
        extra_data.extend_from_slice(&[0u8; 8]); // 压缩大小（这里简化为相同）
    }

    // 添加用户额外数据
    if let Some(data) = user_extra_data {
        extra_data.extend_from_slice(data);
    }

    Ok(extra_data)
}

/// 添加文件到归档
fn add_file_to_archive(
    writer: &mut ZipWriter,
    archive_name: &str,
    input_file: &mut File,
    size_to_add: u64,
    file_time: Option<SystemTime>,
    dos_time: u16,
    method: u16,
    gen_flags: u16,
    comment: Option<&str>,
    comment_size: u16,
    extra_data: &[u8],
    level: u32,
) -> ZipResult<()> {
    // 这里实现真正的文件添加逻辑
    // 1. 写入本地文件头
    // 2. 写入文件数据
    // 3. 写入数据描述符（如果需要）
    // 4. 更新中央目录

    // 简化实现：先读取数据然后添加
    let mut buffer = vec![0; 4096];
    let mut data_buffer = Vec::new();
    let mut bytes_read = 0;

    loop {
        let n = input_file.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        data_buffer.extend_from_slice(&buffer[..n]);
        bytes_read += n as u64;

        // 检查是否读取了预期的数据
        if bytes_read >= size_to_add {
            break;
        }
    }

    // 使用现有的添加方法（这里简化处理）
    writer.add_file(
        archive_name,
        &data_buffer,
        file_time,
        comment,
        level as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::Path;

    #[test]
    fn test_validate_archive_name() {
        let valid_names = vec!["test.txt", "folder/test.txt", "a.txt", "中文文件.txt"];
        let invalid_names = vec!["", "/test.txt", "test\\file.txt", "test:file.txt"];

        for name in valid_names {
            assert!(validate_archive_name(name), "Should be valid: {}", name);
        }

        for name in invalid_names {
            assert!(!validate_archive_name(name), "Should be invalid: {}", name);
        }
    }

    #[test]
    fn test_convert_to_dos_time() {
        let time = SystemTime::UNIX_EPOCH; // 1970-01-01
        let dos_time = convert_to_dos_time(time).unwrap();

        // DOS 时间应该是从 1980-01-01 开始计算的
        assert_eq!(dos_time, 0);
    }

    #[test]
    fn test_add_file() {
        use std::fs;
        use std::io::Write;

        // 创建临时目录
        let tmp_dir = tempdir().unwrap();
        let test_file = tmp_dir.path().join("test.txt");

        // 写入测试内容
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"Hello, ZIP!").unwrap();
        drop(file);

        // 创建 ZIP 文件
        let zip_path = tmp_dir.path().join("test.zip");

        // 使用 add_file() 添加文件到 ZIP
        let result = crate::zip::writer::add_file(
            &zip_path,
            "test.txt",
            &test_file,
            None,
            None,
            6, // 压缩级别
        );

        // 验证 ZIP 文件创建成功
        assert!(result.is_ok(), "Failed to create ZIP: {:?}", result.err());
        assert!(zip_path.exists(), "ZIP file should exist");

        // 验证可以读取 ZIP 内容
        let entries = crate::unzip::list(&zip_path).unwrap();
        assert_eq!(entries.len(), 1, "Should have 1 entry");
        assert_eq!(entries[0].filename, "test.txt");
        assert_eq!(entries[0].uncompressed_size, 12); // "Hello, ZIP!" 的长度

        // 解压并验证内容
        let extract_dir = tmp_dir.path().join("extract");
        fs::create_dir(&extract_dir).unwrap();
        crate::unzip::extract(&zip_path, &extract_dir).unwrap();

        let extracted_file = extract_dir.join("test.txt");
        assert!(extracted_file.exists(), "Extracted file should exist");

        let content = fs::read_to_string(&extracted_file).unwrap();
        assert_eq!(content, "Hello, ZIP!");
    }
}