// ZIP 处理测试
// 对应 C 版本 tests/testthat/test-zip-process.R

mod common;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list};
use common::normalize_temp_paths;

/// 辅助函数：格式化文件列表用于快照
fn format_file_list(entries: &[zip_rs::ZipEntry]) -> String {
    entries
        .iter()
        .map(|e| &e.filename)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// 对应 C 版本: zip_process
#[test]
fn test_zip_process() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1.txt");
    let file2 = tmp_dir.path().join("file2.txt");

    fs::write(&file1, b"content1").unwrap();
    fs::write(&file2, b"content2").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 基本的 ZIP 处理流程
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1.txt", "file2.txt"])
        .unwrap()
        .build()
        .unwrap();

    // 验证 ZIP 文件存在且有效
    assert!(zipfile.exists());
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot(list$filename)
    let output = format_file_list(&entries);
    insta::assert_snapshot!(output);
}

/// 对应 C 版本: can omit directories
#[test]
fn test_can_omit_directories() {
    let tmp_dir = TempDir::new().unwrap();

    let dir = tmp_dir.path().join("testdir");
    fs::create_dir(&dir).unwrap();

    let file1 = dir.join("file1.txt");
    let file2 = dir.join("file2.txt");
    fs::write(&file1, b"file1 content").unwrap();
    fs::write(&file2, b"file2 content").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .include_directories(false)
        .files(&["testdir"])
        .unwrap()
        .build()
        .unwrap();

    // 验证只包含文件，不包含目录条目
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot(list$filename)
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}
