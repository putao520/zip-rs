// UNZIP 处理测试
// 对应 C 版本 tests/testthat/test-unzip-process.R

mod common;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, extract, list};
use common::normalize_temp_paths;

/// 辅助函数：列出目录中的所有文件
fn list_files(dir: &PathBuf) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = path.strip_prefix(dir).unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                files.push(format!("{}/", name));
                files.extend(list_files(&path).into_iter().map(|f| format!("{}/{}", name, f)));
            } else {
                files.push(name);
            }
        }
    }
    files.sort();
    files
}

/// 对应 C 版本: unzip_process
#[test]
fn test_unzip_process() {
    let tmp_dir = TempDir::new().unwrap();

    // 创建测试文件
    let file1 = tmp_dir.path().join("file1.txt");
    let file2 = tmp_dir.path().join("file2.txt");
    fs::write(&file1, b"content1").unwrap();
    fs::write(&file2, b"content2").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 创建 ZIP
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1.txt", "file2.txt"])
        .unwrap()
        .build()
        .unwrap();

    // 删除原文件
    fs::remove_file(&file1).unwrap();
    fs::remove_file(&file2).unwrap();

    // 解压
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(&zipfile, &ex_dir).unwrap();

    // 验证文件已解压
    assert!(ex_dir.join("file1.txt").exists());
    assert!(ex_dir.join("file2.txt").exists());

    let content1 = fs::read_to_string(ex_dir.join("file1.txt")).unwrap();
    assert_eq!(content1, "content1");

    // 对应 C 版本: expect_snapshot of extracted files
    let files = list_files(&ex_dir);
    let normalized = normalize_temp_paths(format!("Extracted files:\n{}", files.join("\n")));
    insta::assert_snapshot!(normalized);
}
