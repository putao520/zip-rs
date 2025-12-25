// 特殊点路径测试
// 对应 C 版本 tests/testthat/test-special-dot.R

mod common;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list, ZipMode};
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

/// 对应 C 版本: `.` is special in cherry picking mode
#[test]
fn test_dot_special_in_cherry_pick_mode() {
    let tmp_dir = TempDir::new().unwrap();

    let xxx = tmp_dir.path().join("xxx");
    fs::create_dir(&xxx).unwrap();

    let bar = xxx.join("bar");
    let foo = xxx.join("foo");
    fs::write(&bar, b"bar\n").unwrap();
    fs::write(&foo, b"foo\n").unwrap();

    let zipfile = tmp_dir.path().join("out.zip");

    // 使用 cherry-pick 模式压缩当前目录 (.)
    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(&xxx)
        .mode(ZipMode::CherryPick)
        .include_directories(false)
        .files(&["."])
        .unwrap()
        .build()
        .unwrap();

    // 验证内容
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot(list$filename)
    let output = format_file_list(&entries);
    insta::assert_snapshot!(output);
}
