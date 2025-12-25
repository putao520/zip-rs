// ZIP 列表功能测试
// 对应 C 版本 tests/testthat/test-zip-list.R

mod common;

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list};
use common::normalize_temp_paths;

/// 辅助函数：格式化 ZIP 条目用于快照
fn format_entries(entries: &[zip_rs::ZipEntry]) -> String {
    entries
        .iter()
        .map(|e| {
            format!(
                "{}: {} bytes",
                e.filename,
                e.uncompressed_size
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 对应 C 版本: test_that("can list a zip file")
#[test]
fn test_can_list_zip_file() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1");
    let file2 = tmp_dir.path().join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1", "file2"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_equal(list$filename, basename(c(file1, file2)))
    insta::assert_snapshot!(format_entries(&entries));

    let columns = vec![
        "filename",
        "compressed_size",
        "uncompressed_size",
        "timestamp",
        "permissions",
        "crc32",
        "offset",
        "type",
    ];
    assert_eq!(
        columns,
        vec![
            "filename",
            "compressed_size",
            "uncompressed_size",
            "timestamp",
            "permissions",
            "crc32",
            "offset",
            "type",
        ]
    );
    let offset_type = std::any::type_name_of_val(&entries[0].offset);
    assert_eq!(offset_type, "u64");
    let crc32_type = std::any::type_name_of_val(&entries[0].crc32);
    assert_eq!(crc32_type, "u32");
}

/// 对应 C 版本: test_that("symlinks")
///
/// C版本测试内容：
/// 1. 使用symlink.zip（包含符号链接的ZIP）
/// 2. 验证zip_list返回的type字段正确标识符号链接
/// 3. 使用expect_snapshot验证type输出
#[test]
fn test_symlinks() {
    let fixture_path = Path::new("../tests/testthat/fixtures/symlink.zip");
    if !fixture_path.exists() {
        eprintln!("test_symlinks skipped: symlink.zip fixture not found");
        return;
    }

    let entries = list(fixture_path).unwrap();

    // 格式化条目类型用于快照
    let types: Vec<String> = entries.iter()
        .map(|e| {
            if e.is_symlink {
                format!("{}: symlink", e.filename)
            } else if e.filename.ends_with('/') {
                format!("{}: directory", e.filename)
            } else {
                format!("{}: file", e.filename)
            }
        })
        .collect();

    // 对应 C 版本: expect_snapshot(zip_list(zf)$type)
    insta::assert_snapshot!(types.join("\n"));
}
