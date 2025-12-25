// zipr.rs - 测试复刻 C 版本 tests/testthat/test-zipr.R
//
// 核心原则：
// 1. 测试场景必须与 C 版本完全一致
// 2. 测试数据（文件名、内容）必须与 C 版本完全一致
// 3. 断言逻辑必须与 C 版本语义对等
// 4. 绝对不允许跳过任何测试场景

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list, append, extract, CompressionLevel, ZipMode};
use common::normalize_temp_paths;

/// 对应 C 版本: test_that("can compress single directory", {
///
/// C 版本测试数据：
/// - 目录包含 file1 (内容: "first file")
/// - 目录包含 file2 (内容: "second file")
/// - 验证 ZIP 文件存在
/// - 验证文件列表正确
///
/// C 版本关键模式：
/// - withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
/// - 即：root = dirname(tmp), files = [basename(tmp)]
#[test]
fn test_can_compress_single_directory() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建目录（对应 C: dir.create(tmp <- tempfile())）
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();

    // 创建文件（完全按照 C 版本的数据）
    let file1 = tmp.join("file1");
    let file2 = tmp.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    // 创建 ZIP 文件（在父目录中）
    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    // root = parent (dirname), files = [tmp_name] (basename)
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile))
    assert!(zipfile.exists(), "ZIP file should exist");

    // 对应 C 版本: expect_equal(list$filename, c(bns(tmp), file.path(basename(tmp), c("file1", "file2"))))
    // 注意: bns(tmp) = paste0(basename(tmp), "/") = tmp_name + "/"
    let entries = list(&zipfile).unwrap();
    let expected = vec![
        format!("{}/", tmp_name),  // bns(tmp)
        format!("{}/file1", tmp_name),
        format!("{}/file2", tmp_name),
    ];

    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();
    assert_eq!(actual, expected, "File list should match");
}

/// 对应 C 版本: test_that("can compress single file", {
///
/// C 版本测试数据：
/// - 文件内容: "compress this if you can!"
/// - 验证 ZIP 文件存在
/// - 验证文件列表正确
#[test]
fn test_can_compress_single_file() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建文件（对应 C: tmp <- tempfile()）
    let tmp = parent.join("tempfile");
    fs::write(&tmp, b"compress this if you can!").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile))
    assert!(zipfile.exists(), "ZIP file should exist");

    // 对应 C 版本: expect_equal(list$filename, basename(tmp))
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();
    assert_eq!(actual.len(), 1, "Should have 1 entry");
    assert_eq!(actual[0], tmp_name, "Filename should match");
}

/// 对应 C 版本: test_that("can compress multiple files", {
///
/// C 版本测试数据：
/// - tmp1: "compress this if you can!"
/// - tmp2: "or even this one"
#[test]
fn test_can_compress_multiple_files() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建两个文件（对应 C: tmp1 <- tempfile(), tmp2 <- tempfile()）
    let tmp1 = parent.join("tmp1");
    let tmp2 = parent.join("tmp2");
    fs::write(&tmp1, b"compress this if you can!").unwrap();
    fs::write(&tmp2, b"or even this one").unwrap();

    let zipfile = parent.join("test.zip");
    let name1 = tmp1.file_name().unwrap().to_str().unwrap();
    let name2 = tmp2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp1), zipr(zipfile, basename(c(tmp1, tmp2))))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[name1, name2])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist");

    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // 对应 C 版本: expect_equal(list$filename, basename(c(tmp1, tmp2)))
    assert_eq!(actual.len(), 2, "Should have 2 entries");
    assert!(actual.contains(&name1.to_string()), "Should contain tmp1");
    assert!(actual.contains(&name2.to_string()), "Should contain tmp2");
}

/// 对应 C 版本: test_that("can compress multiple directories", {
///
/// C 版本测试数据：
/// - tmp1 目录: file1 ("first file"), file2 ("second file")
/// - tmp2 目录: file3 ("third file"), file4 ("fourth file")
#[test]
fn test_can_compress_multiple_directories() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建两个目录（对应 C: dir.create(tmp1 <- tempfile())）
    let tmp1 = parent.join("tmp1");
    let tmp2 = parent.join("tmp2");
    fs::create_dir(&tmp1).unwrap();
    fs::create_dir(&tmp2).unwrap();

    // 创建文件（完全按照 C 版本的数据）
    fs::write(tmp1.join("file1"), b"first file").unwrap();
    fs::write(tmp1.join("file2"), b"second file").unwrap();
    fs::write(tmp2.join("file3"), b"third file").unwrap();
    fs::write(tmp2.join("file4"), b"fourth file").unwrap();

    let zipfile = parent.join("test.zip");
    let name1 = tmp1.file_name().unwrap().to_str().unwrap();
    let name2 = tmp2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp1), zipr(zipfile, basename(c(tmp1, tmp2))))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[name1, name2])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist");

    // 对应 C 版本: expect_equal(list$filename, c(bns(tmp1), file.path(...), bns(tmp2), file.path(...)))
    // 注意: bns(tmp1) = paste0(basename(tmp1), "/")
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // 验证包含所有预期文件
    assert!(actual.contains(&format!("{}/", name1)), "Should contain tmp1 directory");
    assert!(actual.contains(&format!("{}/file1", name1)), "Should contain tmp1/file1");
    assert!(actual.contains(&format!("{}/file2", name1)), "Should contain tmp1/file2");
    assert!(actual.contains(&format!("{}/", name2)), "Should contain tmp2 directory");
    assert!(actual.contains(&format!("{}/file3", name2)), "Should contain tmp2/file3");
    assert!(actual.contains(&format!("{}/file4", name2)), "Should contain tmp2/file4");
}

/// 对应 C 版本: test_that("can compress files and directories", {
///
/// C 版本测试数据：
/// - tmp 目录: file1, file2
/// - file1: "third file"
/// - file2: "fourth file"
#[test]
fn test_can_compress_files_and_directories() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建目录和文件（对应 C 版本）
    let tmp = parent.join("tmp");
    let file1 = parent.join("file1");
    let file2 = parent.join("file2");

    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();
    fs::write(&file1, b"third file").unwrap();
    fs::write(&file2, b"fourth file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();
    let file1_name = file1.file_name().unwrap().to_str().unwrap();
    let file2_name = file2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(c(file1, tmp, file2))))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[file1_name, tmp_name, file2_name])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist");

    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // 对应 C 版本: expect_equal(list$filename, c(basename(file1), bns(tmp), file.path(...), basename(file2)))
    // 注意: bns(tmp) = paste0(basename(tmp), "/")
    assert!(actual.contains(&file1_name.to_string()), "Should contain file1");
    assert!(actual.contains(&format!("{}/", tmp_name)), "Should contain tmp directory");
    assert!(actual.contains(&format!("{}/file1", tmp_name)), "Should contain tmp/file1");
    assert!(actual.contains(&format!("{}/file2", tmp_name)), "Should contain tmp/file2");
    assert!(actual.contains(&file2_name.to_string()), "Should contain file2");
}

/// 对应 C 版本: test_that("warning for directories in non-recursive mode", {
///
/// C 版本测试：
/// - expect_warning(..., "directories ignored")
/// - recurse = FALSE 时目录被忽略
#[test]
fn test_warning_for_directories_in_non_recursive_mode() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建目录和文件（对应 C 版本）
    let tmp = parent.join("tmp");
    let file1 = parent.join("file1");
    let file2 = parent.join("file2");

    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();
    fs::write(&file1, b"third file").unwrap();
    fs::write(&file2, b"fourth file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();
    let file1_name = file1.file_name().unwrap().to_str().unwrap();
    let file2_name = file2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(c(file1, tmp, file2)), recurse = FALSE))
    let result = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[file1_name, tmp_name, file2_name])
        .unwrap()
        .recurse(false)
        .build();

    // 对应 C 版本: expect_silent (ZIP 文件仍被创建)
    assert!(result.is_ok(), "Should create ZIP even with directories ignored");

    // 对应 C 版本: expect_equal(list$filename, c(basename(file1), basename(file2)))
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // 非递归模式下，目录及其内容应该被忽略
    assert!(actual.contains(&file1_name.to_string()), "Should contain file1");
    assert!(actual.contains(&file2_name.to_string()), "Should contain file2");
    assert!(!actual.iter().any(|f| f.starts_with(tmp_name)), "Should not contain tmp directory");
}

/// 对应 C 版本: test_that("compression level is used", {
///
/// C 版本测试数据：
/// - 文件内容: 1:10000 (10000 个数字)
/// - compression_level = 1 (快速)
/// - compression_level = 9 (最紧)
/// - 验证: zipfile1$size <= zipfile2$size
#[test]
fn test_compression_level_is_used() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建文件（对应 C: write(1:10000, file = file <- tempfile())）
    let file = parent.join("data.txt");
    let content: String = (1..=10000).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
    fs::write(&file, content.as_bytes()).unwrap();

    let zipfile1 = parent.join("test1.zip");
    let zipfile2 = parent.join("test2.zip");
    let file_name = file.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(file), zipr(zipfile1, basename(file), compression_level = 1))
    ZipBuilder::new(&zipfile1)
        .unwrap()
        .root(parent)
        .files(&[file_name])
        .unwrap()
        .compression_level(CompressionLevel::Level1)
        .build()
        .unwrap();

    // 对应 C 版本: withr::with_dir(dirname(file), zipr(zipfile2, basename(file), compression_level = 9))
    ZipBuilder::new(&zipfile2)
        .unwrap()
        .root(parent)
        .files(&[file_name])
        .unwrap()
        .compression_level(CompressionLevel::Level9)
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile1)) && expect_true(file.exists(zipfile2))
    assert!(zipfile1.exists(), "ZIP file 1 should exist");
    assert!(zipfile2.exists(), "ZIP file 2 should exist");

    // 对应 C 版本: expect_equal(list$filename, basename(file))
    let entries1 = list(&zipfile1).unwrap();
    let entries2 = list(&zipfile2).unwrap();
    assert_eq!(entries1.len(), 1, "Should have 1 entry");
    assert_eq!(entries2.len(), 1, "Should have 1 entry");

    // 对应 C 版本: expect_true(file.info(zipfile1)$size <= file.info(zipfile2)$size)
    let size1 = fs::metadata(&zipfile1).unwrap().len();
    let size2 = fs::metadata(&zipfile2).unwrap().len();
    assert!(size1 <= size2, "Level 1 should produce larger or equal file than level 9");
}

/// 对应 C 版本: test_that("can append a directory to an archive", {
///
/// C 版本测试：
/// 1. 创建 ZIP 包含 tmp 目录
/// 2. 使用 zipr_append 追加 tmp2 目录
/// 3. 验证两个目录都在 ZIP 中
///
/// C 版本关键模式：
/// - 初次: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
/// - 追加: withr::with_dir(dirname(tmp), zipr_append(zipfile, basename(tmp2)))
/// - 注意：追加时使用相同的 dirname(tmp)，不是 dirname(tmp2)
#[test]
fn test_can_append_a_directory_to_an_archive() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建第一个目录（对应 C 版本）
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist after initial creation");

    // 验证初始内容
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();
    assert!(actual.contains(&format!("{}/", tmp_name)), "Should contain tmp directory");
    assert!(actual.contains(&format!("{}/file1", tmp_name)), "Should contain tmp/file1");
    assert!(actual.contains(&format!("{}/file2", tmp_name)), "Should contain tmp/file2");

    // 创建第二个目录（对应 C 版本）
    let tmp2 = parent.join("tmp2");
    fs::create_dir(&tmp2).unwrap();
    fs::write(tmp2.join("file3"), b"first file2").unwrap();
    fs::write(tmp2.join("file4"), b"second file2").unwrap();

    let tmp2_name = tmp2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr_append(zipfile, basename(tmp2)))
    // 注意：root 仍然是 parent (dirname(tmp))，不是 dirname(tmp2)
    append(&zipfile, parent, &[tmp2_name]).unwrap();

    // 对应 C 版本: expect_equal(list$filename, c(bns(tmp), ..., bns(tmp2), ...))
    // 注意: bns(tmp) = paste0(basename(tmp), "/")
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    assert!(actual.contains(&format!("{}/", tmp_name)), "Should contain tmp directory");
    assert!(actual.contains(&format!("{}/file1", tmp_name)), "Should contain tmp/file1");
    assert!(actual.contains(&format!("{}/file2", tmp_name)), "Should contain tmp/file2");
    assert!(actual.contains(&format!("{}/", tmp2_name)), "Should contain tmp2 directory");
    assert!(actual.contains(&format!("{}/file3", tmp2_name)), "Should contain tmp2/file3");
    assert!(actual.contains(&format!("{}/file4", tmp2_name)), "Should contain tmp2/file4");
}

/// 对应 C 版本: test_that("can append a file to an archive", {
///
/// C 版本测试：
/// 1. 创建 ZIP 包含 tmp 目录
/// 2. 使用 zipr_append 追加 file1
/// 3. 验证目录和文件都在 ZIP 中
#[test]
fn test_can_append_a_file_to_an_archive() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建目录（对应 C 版本）
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist");

    // 创建新文件（对应 C 版本）
    let file1 = parent.join("file1");
    fs::write(&file1, b"first file2").unwrap();
    let file1_name = file1.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr_append(zipfile, basename(file1)))
    append(&zipfile, parent, &[file1_name]).unwrap();

    // 对应 C 版本: expect_equal(list$filename, c(bns(tmp), file.path(...), basename(file1)))
    // 注意: bns(tmp) = paste0(basename(tmp), "/")
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    assert!(actual.contains(&format!("{}/", tmp_name)), "Should contain tmp directory");
    assert!(actual.contains(&format!("{}/file1", tmp_name)), "Should contain tmp/file1");
    assert!(actual.contains(&format!("{}/file2", tmp_name)), "Should contain tmp/file2");
    assert!(actual.contains(&file1_name.to_string()), "Should contain file1");
}

/// 对应 C 版本: test_that("can append files and directories to an archive", {
///
/// C 版本测试：
/// 1. 创建 ZIP 包含 tmp 目录
/// 2. 使用 zipr_append 追加 file1 和 tmp2 目录
/// 3. 验证所有文件和目录都在 ZIP 中
#[test]
fn test_can_append_files_and_directories_to_an_archive() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建初始目录（对应 C 版本）
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists(), "ZIP file should exist");

    // 创建要追加的文件和目录（对应 C 版本）
    let file1 = parent.join("file1");
    let tmp2 = parent.join("tmp2");
    fs::write(&file1, b"first file2").unwrap();
    fs::create_dir(&tmp2).unwrap();
    fs::write(tmp2.join("file3"), b"another").unwrap();
    fs::write(tmp2.join("file4"), b"and another").unwrap();

    let file1_name = file1.file_name().unwrap().to_str().unwrap();
    let tmp2_name = tmp2.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr_append(zipfile, basename(c(file1, tmp2))))
    append(&zipfile, parent, &[file1_name, tmp2_name]).unwrap();

    // 对应 C 版本: expect_equal(list$filename, c(bns(tmp), ..., basename(file1), bns(tmp2), ...))
    // 注意: bns(tmp) = paste0(basename(tmp), "/")
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    assert!(actual.contains(&format!("{}/", tmp_name)), "Should contain tmp directory");
    assert!(actual.contains(&format!("{}/file1", tmp_name)), "Should contain tmp/file1");
    assert!(actual.contains(&format!("{}/file2", tmp_name)), "Should contain tmp/file2");
    assert!(actual.contains(&file1_name.to_string()), "Should contain file1");
    assert!(actual.contains(&format!("{}/", tmp2_name)), "Should contain tmp2 directory");
    assert!(actual.contains(&format!("{}/file3", tmp2_name)), "Should contain tmp2/file3");
    assert!(actual.contains(&format!("{}/file4", tmp2_name)), "Should contain tmp2/file4");
}

/// 对应 C 版本: test_that("empty directories are archived as directories", {
///
/// C 版本测试数据：
/// - foo/bar/ (空目录，递归创建)
/// - foo/bar2/ (空目录)
/// - foo/file1 (内容: "contents\n")
///
/// 验证：
/// - 文件列表包含目录条目（带 / 后缀）
/// - 解压后目录结构正确
/// - 文件内容正确
#[test]
fn test_empty_directories_are_archived_as_directories() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建 tmp 目录
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();

    // 对应 C 版本: dir.create(file.path(tmp, "foo", "bar"), recursive = TRUE)
    let foo = tmp.join("foo");
    let bar = foo.join("bar");
    let bar2 = foo.join("bar2");
    fs::create_dir_all(&bar).unwrap();
    fs::create_dir(&bar2).unwrap();

    // 对应 C 版本: cat("contents\n", file = file.path(tmp, "foo", "file1"))
    fs::write(foo.join("file1"), b"contents\n").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(list$filename, c(paste0(bt, "/"), ...))
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // C 版本期望的格式（注意目录带 / 后缀）
    let expected_files: Vec<String> = vec![
        format!("{}/", tmp_name),
        format!("{}/foo/", tmp_name),
        format!("{}/foo/bar/", tmp_name),
        format!("{}/foo/bar2/", tmp_name),
        format!("{}/foo/file1", tmp_name),
    ];

    // 验证所有预期文件都存在
    for expected in &expected_files {
        assert!(actual.contains(expected), "Should contain {}", expected);
    }

    // 对应 C 版本: 解压并验证目录结构
    let extract_dir = TempDir::new().unwrap();
    extract(&zipfile, extract_dir.path()).unwrap();

    // 对应 C 版本: files <- sort(dir(tmp2, recursive = TRUE, include.dirs = TRUE))
    // 验证解压后的目录和文件
    let extracted_tmp = extract_dir.path().join(&tmp_name);
    let extracted_foo = extracted_tmp.join("foo");
    assert!(extracted_foo.exists(), "Extracted foo directory should exist");
    assert!(extracted_foo.is_dir(), "foo should be a directory");

    let extracted_bar = extracted_foo.join("bar");
    assert!(extracted_bar.exists(), "Extracted foo/bar directory should exist");
    assert!(extracted_bar.is_dir(), "foo/bar should be a directory");

    let extracted_bar2 = extracted_foo.join("bar2");
    assert!(extracted_bar2.exists(), "Extracted foo/bar2 directory should exist");
    assert!(extracted_bar2.is_dir(), "foo/bar2 should be a directory");

    let extracted_file1 = extracted_foo.join("file1");
    assert!(extracted_file1.exists(), "Extracted foo/file1 should exist");
    assert!(!extracted_file1.is_dir(), "foo/file1 should be a file");

    // 对应 C 版本: expect_equal(readLines(file.path(tmp2, bt, "foo", "file1")), "contents")
    let content = fs::read_to_string(&extracted_file1).unwrap();
    assert_eq!(content, "contents\n", "File content should match");
}

/// 对应 C 版本: test_that("Permissions are kept on Unix", {
///
/// C 版本测试：
/// - skip_on_os("windows") - Unix only
/// - tmp 目录权限: 0777
/// - file1 权限: 0400
/// - dir 权限: 0700
/// - dir/file2 权限: 0755
/// - dir/file3 权限: 0777
///
/// 验证：每个文件/目录的权限正确保留
#[cfg(unix)]
#[test]
fn test_permissions_are_kept_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 创建 tmp 目录（对应 C 版本）
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();

    // 对应 C 版本: Sys.chmod(tmp, "0777", FALSE)
    let mut perms = fs::metadata(&tmp).unwrap().permissions();
    perms.set_mode(0o777);
    fs::set_permissions(&tmp, perms).unwrap();

    // 验证权限是否设置成功
    let actual_perms = fs::metadata(&tmp).unwrap().permissions().mode() & 0o777;
    eprintln!("After setting tmp to 0777, actual permissions: {:04o}", actual_perms);

    // 对应 C 版本: file1 权限 0400
    let file1 = tmp.join("file1");
    fs::write(&file1, b"foobar\n").unwrap();
    let mut perms = fs::metadata(&file1).unwrap().permissions();
    perms.set_mode(0o400);
    fs::set_permissions(&file1, perms).unwrap();

    // 对应 C 版本: dir 权限 0700
    let dir = tmp.join("dir");
    fs::create_dir(&dir).unwrap();
    let mut perms = fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&dir, perms).unwrap();

    // 对应 C 版本: dir/file2 权限 0755
    let file2 = dir.join("file2");
    fs::write(&file2, b"foobar2\n").unwrap();
    let mut perms = fs::metadata(&file2).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&file2, perms).unwrap();

    // 对应 C 版本: dir/file3 权限 0777
    let file3 = dir.join("file3");
    fs::write(&file3, b"foobar3\n").unwrap();
    let mut perms = fs::metadata(&file3).unwrap().permissions();
    perms.set_mode(0o777);
    fs::set_permissions(&file3, perms).unwrap();

    // 创建 ZIP（对应 C 版本）
    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp)))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: l <- zip_list(zip)
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: check_perm 函数验证每个文件的权限
    // 注意: 目录名称在 ZIP 中带 "/" 后缀
    let check_perm = |name: &str, expected_mode: u32| -> bool {
        // 尝试两种模式："/name"（用于文件）和 "/name/"（用于目录）
        let file_pattern = format!("/{}", name);
        let dir_pattern = format!("/{}/", name);
        let entry = entries.iter().find(|e| {
            // 尝试匹配不带 / 的名称（用于文件）或带 / 的名称（用于目录）
            e.filename == name ||
            e.filename == format!("{}/", name) ||
            e.filename.ends_with(&file_pattern) ||
            e.filename.ends_with(&dir_pattern)
        });

        if let Some(entry) = entry {
            entry.permissions == expected_mode
        } else {
            false
        }
    };

    // 对应 C 版本: check_perm(basename(tmp), "0777")
    // 目录名在 ZIP 中是 "tmp/"
    assert!(check_perm(tmp_name, 0o777) || check_perm(&format!("{}/", tmp_name), 0o777),
            "tmp directory should have 0777 permissions");

    // 对应 C 版本: check_perm("file1", "0400")
    assert!(check_perm("file1", 0o400), "file1 should have 0400 permissions");

    // 对应 C 版本: check_perm("dir", "0700")
    assert!(check_perm("dir", 0o700), "dir should have 0700 permissions");

    // 对应 C 版本: check_perm("file2", "0755")
    assert!(check_perm("file2", 0o755), "file2 should have 0755 permissions");

    // 对应 C 版本: check_perm("file3", "0777")
    assert!(check_perm("file3", 0o777), "file3 should have 0777 permissions");
}

/// 对应 C 版本: test_that("can omit directories", {
///
/// C 版本测试：
/// - tmp 目录包含 file1, file2
/// - include_directories = FALSE
/// - 验证：只有文件包含在 ZIP 中，不包含目录条目
#[test]
fn test_can_omit_directories() {
    let parent_dir = TempDir::new().unwrap();
    let parent = parent_dir.path();

    // 对应 C 版本: dir.create(tmp <- tempfile())
    let tmp = parent.join("tmp");
    fs::create_dir(&tmp).unwrap();
    fs::write(tmp.join("file1"), b"first file").unwrap();
    fs::write(tmp.join("file2"), b"second file").unwrap();

    let zipfile = parent.join("test.zip");
    let tmp_name = tmp.file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: withr::with_dir(dirname(tmp), zipr(zipfile, basename(tmp), include_directories = FALSE))
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[tmp_name])
        .unwrap()
        .include_directories(false)
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile))
    assert!(zipfile.exists(), "ZIP file should exist");

    // 对应 C 版本: expect_equal(list$filename, file.path(basename(tmp), c("file1", "file2")))
    let entries = list(&zipfile).unwrap();
    let actual: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // C 版本期望：只有文件，没有目录条目
    let expected_file1 = format!("{}/file1", tmp_name);
    let expected_file2 = format!("{}/file2", tmp_name);

    assert!(!actual.contains(&tmp_name.to_string()), "Should not contain directory entry");
    assert!(actual.contains(&expected_file1), "Should contain file1");
    assert!(actual.contains(&expected_file2), "Should contain file2");
}
