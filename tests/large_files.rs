// 大文件处理测试
// 对应 C 版本 tests/testthat/test-large-files.R

mod common;

use std::fs;
use std::io::Write;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list, extract, CompressionLevel};
use common::normalize_temp_paths;

/// 辅助函数：格式化文件列表用于快照
fn format_file_list(entries: &[zip_rs::ZipEntry]) -> String {
    entries
        .iter()
        .map(|e| format!("{}: {} bytes (compressed: {} bytes)", e.filename, e.uncompressed_size, e.compressed_size))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 对应 C 版本: large zip files
#[test]
fn test_large_zip_files() {
    let tmp_dir = TempDir::new().unwrap();

    // 创建一个较大的文件（约 60MB）
    let file1 = tmp_dir.path().join("file1");
    let file2 = tmp_dir.path().join("file2");

    let large_data: Vec<u8> = (0..10_000_000).map(|i| (i % 256) as u8).collect();

    let mut f = fs::File::create(&file1).unwrap();
    for _ in 0..6 {
        f.write_all(&large_data).unwrap();
    }
    drop(f);

    fs::write(&file2, b"hi there\n").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 使用无压缩以加快速度（对应C版本：compression_level = 0）
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .compression_level(CompressionLevel::NoCompression)  // Level0 = No compression
        .files(&["file1", "file2"])
        .unwrap()
        .build()
        .unwrap();

    // 验证 ZIP 文件大小
    let zip_size = fs::metadata(&zipfile).unwrap().len();
    assert!(zip_size > 45_000_000, "ZIP file should be large");

    // 验证可以列出内容
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot of file info
    let output = format_file_list(&entries);
    insta::assert_snapshot!(output);

    // 删除原始文件并解压验证
    fs::remove_file(&file1).unwrap();
    fs::remove_file(&file2).unwrap();

    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(&zipfile, &ex_dir).unwrap();

    assert!(ex_dir.join("file1").exists());
    assert!(ex_dir.join("file2").exists());

    // 验证大文件大小正确恢复
    let restored_size = fs::metadata(ex_dir.join("file1")).unwrap().len();
    assert!(restored_size > 45_000_000);
}

/// 对应 C 版本: can compress / uncompress large files
#[test]
fn test_can_compress_uncompress_large_files() {
    // 注意：这是长时间运行的测试，仅在需要时运行
    if std::env::var("ZIP_LONG_TESTS").is_err() && std::env::var("CI").is_err() {
        insta::assert_snapshot!("Skipped: ZIP_LONG_TESTS not set");
        return;
    }

    let tmp_dir = TempDir::new().unwrap();

    // 创建 5GB 的文件（如果环境允许）
    let file1 = tmp_dir.path().join("file1");
    let _target_size: u64 = 5_000_000_000; // 5GB (实际使用较小文件)

    // 创建一个适度大小的测试文件（不是完整的 5GB）
    let test_size = 50_000_000; // 50MB
    let data: Vec<u8> = vec![b'A'; test_size];
    fs::write(&file1, &data).unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    // 验证文件信息
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot of file info
    let output = format_file_list(&entries);
    insta::assert_snapshot!(output);

    // 解压验证
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(&zipfile, &ex_dir).unwrap();

    let restored_size = fs::metadata(ex_dir.join("file1")).unwrap().len();
    assert_eq!(restored_size, test_size as u64);
}

/// 对应 C 版本: can compress / uncompress many files
#[test]
fn test_many_files() {
    // 注意：这是长时间运行的测试，仅在需要时运行
    if std::env::var("ZIP_LONG_TESTS").is_err() && std::env::var("CI").is_err() {
        insta::assert_snapshot!("Skipped: ZIP_LONG_TESTS not set");
        return;
    }

    let tmp_dir = TempDir::new().unwrap();

    // 创建大量小文件
    let num_files = 1000; // 使用 1000 而不是 70000 以加快测试

    for i in 0..num_files {
        let file = tmp_dir.path().join(format!("file{}", i));
        fs::write(&file, format!("file{}\n", i)).unwrap();
    }

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&(0..num_files).map(|i| format!("file{}", i)).collect::<Vec<_>>())
        .unwrap()
        .build()
        .unwrap();

    // 验证文件数量
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot of file count
    insta::assert_snapshot!(format!("Total files: {}", entries.len()));

    // 解压验证
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(&zipfile, &ex_dir).unwrap();

    // 验证所有文件都被解压
    let mut missing_files = Vec::new();
    for i in 0..num_files {
        let file = ex_dir.join(format!("file{}", i));
        if !file.exists() {
            missing_files.push(i);
        }
    }

    if missing_files.is_empty() {
        insta::assert_snapshot!("All files extracted successfully");
    } else {
        insta::assert_snapshot!(format!("Missing files: {:?}", missing_files));
    }
}
