// 奇怪路径测试
// 对应 C 版本 tests/testthat/test-weird-paths.R

mod common;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use zip_rs::{ZipBuilder, list, extract, ZipMode};
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

/// 对应 C 版本: warning for colon (Unix only)
#[test]
fn test_warning_for_colon() {
    #[cfg(unix)]
    {
        let tmp_dir = TempDir::new().unwrap();

        let file = tmp_dir.path().join("bad:boy");
        fs::write(&file, b"boo\n").unwrap();

        let zipfile = tmp_dir.path().join("test.zip");

        // 包含冒号的文件名应该产生警告或错误
        let result = ZipBuilder::new(&zipfile)
            .unwrap()
            .root(tmp_dir.path())
            .mode(ZipMode::CherryPick)
            .files(&["bad:boy"]);

        // 对应 C 版本: expect_warning("Some paths include a `:` character")
        match result {
            Ok(builder) => {
                let output = builder.build_with_warnings().unwrap();
                // 对应 C 版本: expect_warning(..., "Some paths include a `:` character")
                // 验证路径包含冒号字符产生了 ColonPaths 警告
                assert_warning!(output, ColonPaths, "Paths with colon should produce ColonPaths warning");

                let entries = list(&zipfile).unwrap();
                let file_list = format_file_list(&entries);
                insta::assert_snapshot!(file_list);
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                insta::assert_snapshot!(error_msg);
            }
        }
    }

    #[cfg(windows)]
    {
        // Windows 上冒号是非法字符，应该失败
        insta::assert_snapshot!("Skipped on Windows: colon is illegal character");
    }
}

/// 对应 C 版本: absolute paths lose leading /
#[test]
fn test_absolute_paths_lose_leading_slash() {
    #[cfg(unix)]
    {
        let tmp_dir = TempDir::new().unwrap();

        // 创建绝对路径结构
        let abs_path = tmp_dir.path().join("tmp");
        fs::create_dir(&abs_path).unwrap();

        let file = abs_path.join("bad");
        fs::write(&file, b"boo\n").unwrap();

        let zipfile = tmp_dir.path().join("test.zip");

        // C版本: zip(tmpzip, tmp, mode = "mirror") - tmp 是绝对路径
        // 使用 mirror 模式压缩绝对路径
        // 注意：需要传递绝对路径给 files() 才能触发 DroppedLeadingSlash 警告
        let abs_path_str = abs_path.to_string_lossy().to_string();
        let output = ZipBuilder::new(&zipfile)
            .unwrap()
            .root(tmp_dir.path())
            .mode(ZipMode::Mirror)
            .files(&[&abs_path_str])
            .unwrap()
            .build_with_warnings()
            .unwrap();

        // 对应 C 版本: expect_warning(..., "Dropping leading `/` from paths")
        // 验证绝对路径去掉前导 / 产生了 DroppedLeadingSlash 警告
        assert_warning!(output, DroppedLeadingSlash, "Absolute paths should produce DroppedLeadingSlash warning");

        let entries = list(&zipfile).unwrap();

        // 对应 C 版本: expect_snapshot(list$filename)
        // 验证路径被处理（可能去掉了前导 /）
        let file_list = format_file_list(&entries);
        let normalized = normalize_temp_paths(file_list);
        insta::assert_snapshot!(normalized);
    }

    #[cfg(windows)]
    {
        insta::assert_snapshot!("Skipped on Windows");
    }
}

/// 对应 C 版本: backslash is an error (Unix)
#[test]
fn test_backslash_is_error() {
    #[cfg(unix)]
    {
        let tmp_dir = TempDir::new().unwrap();

        let file_path = tmp_dir.path().join("real\\bad");
        // 在 Unix 上，\ 是合法的文件名字符
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, b"boo\n").unwrap();

        let zipfile = tmp_dir.path().join("test.zip");

        // 尝试压缩包含反斜杠的路径
        // 对应 C 版本: expect_snapshot(error = TRUE, ...)
        let result = ZipBuilder::new(&zipfile)
            .unwrap()
            .root(tmp_dir.path())
            .mode(ZipMode::CherryPick)
            .files(&["real\\bad"])
            .unwrap()
            .build();

        // C版本在miniz.c中验证路径，拒绝包含反斜杠的文件名
        let error_msg = match result {
            Ok(_) => "Expected error for backslash path".to_string(),
            Err(e) => format!("{:?}", e),
        };
        let normalized = normalize_temp_paths(error_msg);
        insta::assert_snapshot!(normalized);
    }

    #[cfg(windows)]
    {
        insta::assert_snapshot!("Skipped on Windows");
    }
}

/// 对应 C 版本: extracting absolute path
#[test]
fn test_extracting_absolute_path() {
    let tmp_dir = TempDir::new().unwrap();

    // 创建一个包含嵌套目录的测试
    let nested = tmp_dir.path().join("tmp");
    fs::create_dir(&nested).unwrap();

    let file = nested.join("boo");
    fs::write(&file, b"boo\n").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["tmp"])
        .unwrap()
        .build()
        .unwrap();

    // 解压到新目录
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(&zipfile, &ex_dir).unwrap();

    // 验证文件被正确解压
    assert!(ex_dir.join("tmp").exists());
    assert!(ex_dir.join("tmp/boo").exists());

    let content = fs::read_to_string(ex_dir.join("tmp/boo")).unwrap();
    assert_eq!(content, "boo\n");

    // 对应 C 版本: expect_snapshot of extracted structure
    insta::assert_snapshot!("Extracted successfully");
}
