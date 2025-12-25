// get_zip_data 函数测试
// 对应 C 版本 tests/testthat/test-get-zip-data.R

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

/// 对应 C 版本: get_zip_data (test_that("get_zip_data", {
#[test]
fn test_get_zip_data() {
    let tmp_dir = TempDir::new().unwrap();

    // 创建目录结构
    let empty = tmp_dir.path().join("empty");
    let foo = tmp_dir.path().join("foo");
    let bar = foo.join("bar");
    let foobar = tmp_dir.path().join("foobar");

    fs::create_dir(&empty).unwrap();
    fs::create_dir(&foo).unwrap();
    fs::create_dir(&bar).unwrap();
    fs::write(&foobar, b"foobar").unwrap();

    let bar_file = bar.join("bar.txt");
    fs::write(&bar_file, b"bar\n").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["empty", "foo", "foobar"])
        .unwrap()
        .build()
        .unwrap();

    // 验证 ZIP 文件包含所有预期的文件和目录
    let entries = list(&zipfile).unwrap();

    // 应该包含: empty/, foo/, foo/bar/, foo/bar/bar.txt, foobar
    // 对应 C 版本: expect_snapshot(list$filename)
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}
