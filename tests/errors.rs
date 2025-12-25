// 错误处理测试
// 对应 C 版本 tests/testthat/test-errors.R

mod common;

use std::fs;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list, ZipError};
use common::normalize_temp_paths;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// 对应 C 版本: test_that("non-existant file")
#[test]
fn test_non_existent_file() {
    let tmp_dir = TempDir::new().unwrap();
    let zipfile = tmp_dir.path().join("test.zip");

    // 尝试压缩不存在的文件
    let result = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["nonexistent.txt"])
        .unwrap()
        .build();

    assert!(result.is_err());

    // 快照错误消息
    if let Err(e) = result {
        let normalized = normalize_temp_paths(format!("{:?}", e));
        insta::assert_snapshot!(normalized);
    }
}

/// 对应 C 版本: test_that("appending non-existant file")
#[test]
fn test_appending_non_existent_file() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1.txt");
    fs::write(&file1, b"content").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 首先创建一个有效的 ZIP
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1.txt"])
        .unwrap()
        .build()
        .unwrap();

    // 然后尝试追加不存在的文件
    let result = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .append(true)
        .files(&["nonexistent.txt"])
        .unwrap()
        .build();

    assert!(result.is_err());

    // 快照错误消息
    if let Err(e) = result {
        let normalized = normalize_temp_paths(format!("{:?}", e));
        insta::assert_snapshot!(normalized);
    }
}

/// 对应 C 版本: test_that("empty archive, no files")
#[test]
fn test_empty_archive_no_files() {
    let tmp_dir = TempDir::new().unwrap();
    let zipfile = tmp_dir.path().join("empty.zip");

    // 创建空 ZIP（没有文件）
    let result = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&[] as &[&str])
        .unwrap()
        .build();

    assert!(result.is_ok());

    // 验证空 ZIP 文件存在
    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    assert_eq!(entries.len(), 0);

    // 快照：空文件列表
    insta::assert_snapshot!(format!("Entries: {}", entries.len()));
}

/// 对应 C 版本: test_that("single empty directory")
#[test]
fn test_single_empty_directory() {
    let tmp_dir = TempDir::new().unwrap();
    let dir = tmp_dir.path().join("testdir");
    fs::create_dir(&dir).unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .include_directories(true)
        .files(&["testdir"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    // 对应 C 版本: expect_equal(nrow(list), 1)
    let output = format!("Entries: {}\nFiles:\n{}", entries.len(),
        entries.iter().map(|e| &e.filename).cloned().collect::<Vec<_>>().join("\n"));
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("appending single empty directory")
#[test]
fn test_appending_empty_directory() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1.txt");
    let file2 = tmp_dir.path().join("file2.txt");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 首先创建一个 ZIP
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1.txt", "file2.txt"])
        .unwrap()
        .build()
        .unwrap();

    let entries_before = list(&zipfile).unwrap();
    let count_before = entries_before.len();

    // 然后追加空目录
    let empty_dir = tmp_dir.path().join("empty_dir");
    fs::create_dir(&empty_dir).unwrap();

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .append(true)
        .include_directories(true)
        .files(&["empty_dir"])
        .unwrap()
        .build()
        .unwrap();

    let entries_after = list(&zipfile).unwrap();
    // 对应 C 版本: expect_equal(nrow(list), 4)
    let output = format!("Before: {} entries\nAfter: {} entries\nFiles:\n{}",
        count_before,
        entries_after.len(),
        entries_after.iter().map(|e| &e.filename).cloned().collect::<Vec<_>>().join("\n"));
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("non readable file")
#[test]
fn test_non_readable_file_skipped() {
    let tmp_dir = TempDir::new().unwrap();

    let file = tmp_dir.path().join("unreadable.txt");
    fs::write(&file, b"content").unwrap();

    // 在 Unix 上设置文件为不可读
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&file, perms).unwrap();
    }

    let zipfile = tmp_dir.path().join("test.zip");

    // 尝试压缩不可读的文件
    let result = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["unreadable.txt"])
        .unwrap()
        .build();

    #[cfg(unix)]
    {
        // 在 Unix 上应该失败
        assert!(result.is_err());

        // 快照错误消息
        if let Err(e) = result {
            let normalized = normalize_temp_paths(format!("{:?}", e));
            insta::assert_snapshot!(normalized);
        }
    }

    // 恢复权限以便清理
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file, perms).unwrap();
    }
}

/// 对应 C 版本: test_that("single empty directory, non-recursive")
#[test]
fn test_single_empty_directory_non_recursive() {
    let tmp_dir = TempDir::new().unwrap();
    let dir = tmp_dir.path().join("testdir");
    fs::create_dir(&dir).unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // C版本: expect_warning 在非递归模式下压缩空目录
    let output = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .recurse(false)
        .files(&["testdir"])
        .unwrap()
        .build_with_warnings()
        .unwrap();

    // 对应 C 版本: expect_warning(..., "directories ignored")
    // 验证非递归模式下目录被忽略产生 DirectoriesIgnored 警告
    assert_warning!(output, DirectoriesIgnored, "Non-recursive mode should warn about ignored directories");

    // C版本: expect_equal(nrow(list), 0) - 在非递归模式下，空目录被忽略
    let entries = list(&zipfile).unwrap();
    let file_list = format!("Entries: {}\nFiles:\n{}", entries.len(),
        entries.iter().map(|e| &e.filename).cloned().collect::<Vec<_>>().join("\n"));
    let normalized = normalize_temp_paths(file_list);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("appending single empty directory, non-recursive")
#[test]
fn test_appending_single_empty_directory_non_recursive() {
    let tmp_dir = TempDir::new().unwrap();

    let dir = tmp_dir.path().join("testdir");
    fs::create_dir(&dir).unwrap();

    let file1 = dir.join("file1");
    let file2 = dir.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    // 首先创建一个 ZIP
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["testdir"])
        .unwrap()
        .build()
        .unwrap();

    let entries_before = list(&zipfile).unwrap();

    // 然后尝试在非递归模式下追加空目录
    let empty_dir = tmp_dir.path().join("empty_dir");
    fs::create_dir(&empty_dir).unwrap();

    let output = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .append(true)
        .recurse(false)
        .files(&["empty_dir"])
        .unwrap()
        .build_with_warnings()
        .unwrap();

    // 对应 C 版本: expect_warning(..., "directories ignored")
    // 验证非递归追加模式下目录被忽略产生 DirectoriesIgnored 警告
    assert_warning!(output, DirectoriesIgnored, "Non-recursive append should warn about ignored directories");

    let entries_after = list(&zipfile).unwrap();
    // C版本: 在非递归模式下，空目录被忽略，条目数不变
    let file_list = format!("Before: {} entries\nAfter: {} entries\nFiles:\n{}",
        entries_before.len(),
        entries_after.len(),
        entries_after.iter().map(|e| &e.filename).cloned().collect::<Vec<_>>().join("\n"));
    let normalized = normalize_temp_paths(file_list);
    insta::assert_snapshot!(normalized);
}
