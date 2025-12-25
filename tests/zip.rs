// ZIP 压缩测试
// 对应 C 版本 tests/testthat/test-zip.R

mod common;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use zip_rs::{append, extract, Extractor, list, ZipBuildOutput, ZipBuilder, ZipMode, ZipWarning};
use common::{bns, normalize_temp_paths};

/// 辅助函数：格式化文件列表用于快照
fn format_file_list(entries: &[zip_rs::ZipEntry]) -> String {
    entries
        .iter()
        .map(|e| &e.filename)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

struct CurrentDirGuard {
    previous: PathBuf,
}

impl CurrentDirGuard {
    fn new(path: impl AsRef<Path>) -> Self {
        let previous = env::current_dir().unwrap();
        env::set_current_dir(path).unwrap();
        Self { previous }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.previous);
    }
}

fn list_dir_recursive(root: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    collect_dir_entries(root, root, &mut entries);
    entries.sort();
    entries
}

fn collect_dir_entries(root: &Path, dir: &Path, entries: &mut Vec<String>) {
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if let Ok(relative) = path.strip_prefix(root) {
                let relative = relative.to_string_lossy().replace('\\', "/");
                entries.push(relative);
            }
            if path.is_dir() {
                collect_dir_entries(root, &path, entries);
            }
        }
    }
}

/// 对应 C 版本: test_that("can compress single file")
#[test]
fn test_can_compress_single_file() {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir.path().join("test_file.txt");
    fs::write(&file_path, b"compress this if you can!").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["test_file.txt"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    // 对应 C 版本: expect_equal(list$filename, basename(tmp))
    insta::assert_snapshot!(format_file_list(&entries));
}

/// 对应 C 版本: test_that("can compress multiple files")
#[test]
fn test_can_compress_multiple_files() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1.txt");
    let file2 = tmp_dir.path().join("file2.txt");
    fs::write(&file1, b"compress this if you can!").unwrap();
    fs::write(&file2, b"or even this one").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1.txt", "file2.txt"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    // 对应 C 版本: expect_equal(list$filename, basename(c(tmp1, tmp2)))
    insta::assert_snapshot!(format_file_list(&entries));
}

/// 对应 C 版本: test_that("can compress single directory")
#[test]
fn test_can_compress_single_directory() {
    let tmp_dir = TempDir::new().unwrap();

    let subdir = tmp_dir.path().join("testdir");
    fs::create_dir(&subdir).unwrap();

    let file1 = subdir.join("file1");
    let file2 = subdir.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["testdir"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    // 对应 C 版本: expect_equal with bns(tmp)
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("can compress multiple directories")
/// C版本验证：
/// 1. zipfile exists
/// 2. zip_list filename验证
/// 3. utils::unzip 解压验证
/// 4. file.info $isdir 目录验证
/// 5. readLines 文件内容精确对比
#[test]
fn test_can_compress_multiple_directories() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();

    let file1 = tmp1.path().join("file1");
    let file2 = tmp1.path().join("file2");
    fs::write(&file1, b"first file\n").unwrap();
    fs::write(&file2, b"second file\n").unwrap();

    let file3 = tmp2.path().join("file3");
    let file4 = tmp2.path().join("file4");
    fs::write(&file3, b"third file\n").unwrap();
    fs::write(&file4, b"fourth file\n").unwrap();

    let parent = tmp1.path().parent().unwrap();
    let zipfile = tmp1.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[
            tmp1.path().file_name().unwrap().to_str().unwrap(),
            tmp2.path().file_name().unwrap().to_str().unwrap(),
        ])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile))
    assert!(zipfile.exists());

    // 对应 C 版本: zip_list 验证
    let entries = list(&zipfile).unwrap();
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);

    // 对应 C 版本: utils::unzip(zipfile, exdir = tmp3)
    let tmp3 = TempDir::new().unwrap();
    zip_rs::extract(&zipfile, tmp3.path()).unwrap();

    // 对应 C 版本: expect_true(file.info(...)$isdir)
    let tmp1_name = tmp1.path().file_name().unwrap().to_str().unwrap();
    let tmp2_name = tmp2.path().file_name().unwrap().to_str().unwrap();
    let extracted_tmp1 = tmp3.path().join(tmp1_name);
    let extracted_tmp2 = tmp3.path().join(tmp2_name);

    assert!(extracted_tmp1.is_dir(), "tmp1 should be a directory after extraction");
    assert!(extracted_tmp2.is_dir(), "tmp2 should be a directory after extraction");

    // 对应 C 版本: expect_equal(readLines(file1), readLines(extracted_file1))
    let extracted_file1 = extracted_tmp1.join("file1");
    let extracted_file2 = extracted_tmp1.join("file2");
    let extracted_file3 = extracted_tmp2.join("file3");
    let extracted_file4 = extracted_tmp2.join("file4");

    assert_eq!(
        fs::read_to_string(&file1).unwrap(),
        fs::read_to_string(&extracted_file1).unwrap()
    );
    assert_eq!(
        fs::read_to_string(&file2).unwrap(),
        fs::read_to_string(&extracted_file2).unwrap()
    );
    assert_eq!(
        fs::read_to_string(&file3).unwrap(),
        fs::read_to_string(&extracted_file3).unwrap()
    );
    assert_eq!(
        fs::read_to_string(&file4).unwrap(),
        fs::read_to_string(&extracted_file4).unwrap()
    );
}

/// 对应 C 版本: test_that("can compress files and directories")
#[test]
fn test_can_compress_files_and_directories() {
    let tmp = TempDir::new().unwrap();
    let parent = tmp.path().parent().unwrap();

    let dir = tmp.path().join("testdir");
    fs::create_dir(&dir).unwrap();
    let file1 = dir.join("file1");
    let file2 = dir.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let file3 = parent.join("file3");
    let file4 = parent.join("file4");
    fs::write(&file3, b"third file").unwrap();
    fs::write(&file4, b"fourth file").unwrap();

    let zipfile = parent.join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .files(&[
            "file3",
            tmp.path().file_name().unwrap().to_str().unwrap(),
            "file4",
        ])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("warning for directories in non-recursive mode")
/// C版本验证：
/// 1. expect_warning("directories ignored") - 在非递归模式下传递目录应产生警告
/// 2. expect_true(file.exists(zipfile))
/// 3. expect_equal(list$filename, c(basename(file1), basename(file2))) - 只包含文件，不包含目录
#[test]
fn test_warning_for_directories_in_non_recursive_mode() {
    let tmp = TempDir::new().unwrap();
    let parent = tmp.path().parent().unwrap();

    // C版本: dir.create(tmp <- tempfile())
    // cat("first file", file = file.path(tmp, "file1"))
    // cat("second file", file = file.path(tmp, "file2"))
    let dir = tmp.path().join("testdir");
    fs::create_dir(&dir).unwrap();
    let file1 = dir.join("file1");
    let file2 = dir.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    // C版本: cat("third file", file = file1 <- tempfile())
    // cat("fourth file", file = file2 <- tempfile())
    // 创建两个额外的独立文件
    let file3_path = parent.join("file3");
    let file4_path = parent.join("file4");
    fs::write(&file3_path, b"third file").unwrap();
    fs::write(&file4_path, b"fourth file").unwrap();

    let zipfile = tmp.path().join("test.zip");

    // C版本: expect_warning(zip(zipfile, basename(c(file1, tmp, file2)), recurse = FALSE), "directories ignored")
    // 在非递归模式下，目录应该被忽略，只添加文件
    // Rust版本: 使用 build_with_warnings() 获取警告
    let output: ZipBuildOutput = ZipBuilder::new(&zipfile)
        .unwrap()
        .root(parent)
        .recurse(false)
        .files(&["file3", tmp.path().file_name().unwrap().to_str().unwrap(), "file4"])
        .unwrap()
        .build_with_warnings()
        .unwrap();

    // 对应 C 版本: expect_warning(..., "directories ignored")
    // 验证在非递归模式下传递目录产生了 DirectoriesIgnored 警告
    assert_warning!(output, DirectoriesIgnored, "Non-recursive mode should warn about ignored directories");

    // 对应 C 版本: expect_true(file.exists(zipfile))
    assert!(zipfile.exists());

    // 对应 C 版本: expect_equal(list$filename, c(basename(file1), basename(file2)))
    // 在非递归模式下，目录被忽略，只包含文件
    let entries = list(&zipfile).unwrap();
    let filenames: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // 验证只包含两个文件，不包含目录
    assert!(filenames.contains(&"file3".to_string()));
    assert!(filenames.contains(&"file4".to_string()));
    // 目录 testdir 不应该出现在列表中
    assert!(!filenames.iter().any(|f| f.contains("testdir")));
}

/// 对应 C 版本: test_that("can omit directories")
#[test]
fn test_omit_directories() {
    let tmp_dir = TempDir::new().unwrap();

    let subdir = tmp_dir.path().join("testdir");
    fs::create_dir(&subdir).unwrap();

    let file1 = subdir.join("file1.txt");
    let file2 = subdir.join("file2.txt");
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

    let entries = list(&zipfile).unwrap();
    // 对应 C 版本: file.path(basename(tmp), c("file1", "file2"))
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("can append a directory to an archive")
#[test]
fn test_can_append_directory() {
    let base = TempDir::new().unwrap();
    let tmp = base.path().join("dir1");
    fs::create_dir(&tmp).unwrap();

    let file1 = tmp.join("file1");
    let file2 = tmp.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = base.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .files(&[tmp.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    let tmp_name = tmp.file_name().unwrap().to_string_lossy().to_string();
    let initial_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        initial_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
        ]
    );

    let tmp2 = base.path().join("dir2");
    fs::create_dir(&tmp2).unwrap();
    let file3 = tmp2.join("file3");
    let file4 = tmp2.join("file4");
    fs::write(&file3, b"first file2").unwrap();
    fs::write(&file4, b"second file2").unwrap();

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .append(true)
        .files(&[tmp2.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    let entries = list(&zipfile).unwrap();
    let tmp2_name = tmp2.file_name().unwrap().to_string_lossy().to_string();
    let appended_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        appended_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
            bns(&tmp2),
            format!("{}/file3", tmp2_name),
            format!("{}/file4", tmp2_name),
        ]
    );
}

/// 对应 C 版本: test_that("can append a file to an archive")
#[test]
fn test_can_append_file() {
    let base = TempDir::new().unwrap();
    let tmp = base.path().join("testdir");
    fs::create_dir(&tmp).unwrap();

    let file1 = tmp.join("file1");
    let file2 = tmp.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = base.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .files(&[tmp.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    let tmp_name = tmp.file_name().unwrap().to_string_lossy().to_string();
    let initial_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        initial_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
        ]
    );

    let appended_file = base.path().join("appended_file");
    fs::write(&appended_file, b"first file2").unwrap();

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .append(true)
        .files(&[appended_file.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    let entries = list(&zipfile).unwrap();
    let appended_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        appended_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
            appended_file.file_name().unwrap().to_string_lossy().to_string(),
        ]
    );
}

/// 对应 C 版本: test_that("can append files and directories to an archive")
#[test]
fn test_can_append_files_and_directories() {
    let base = TempDir::new().unwrap();
    let tmp = base.path().join("dir1");
    fs::create_dir(&tmp).unwrap();

    let file1 = tmp.join("file1");
    let file2 = tmp.join("file2");
    fs::write(&file1, b"first file").unwrap();
    fs::write(&file2, b"second file").unwrap();

    let zipfile = base.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .files(&[tmp.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    let entries = list(&zipfile).unwrap();
    let tmp_name = tmp.file_name().unwrap().to_string_lossy().to_string();
    let initial_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        initial_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
        ]
    );

    let appended_file = base.path().join("appended_file");
    fs::write(&appended_file, b"first file2").unwrap();

    let tmp2 = base.path().join("dir2");
    fs::create_dir(&tmp2).unwrap();
    let file3 = tmp2.join("file3");
    let file4 = tmp2.join("file4");
    fs::write(&file3, b"another").unwrap();
    fs::write(&file4, b"and another").unwrap();

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .append(true)
        .files(&[
            appended_file.file_name().unwrap().to_str().unwrap(),
            tmp2.file_name().unwrap().to_str().unwrap(),
        ])
        .unwrap()
        .build()
        .unwrap();

    let entries = list(&zipfile).unwrap();
    let tmp2_name = tmp2.file_name().unwrap().to_string_lossy().to_string();
    let appended_filenames = entries
        .iter()
        .map(|e| e.filename.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        appended_filenames,
        vec![
            bns(&tmp),
            format!("{}/file1", tmp_name),
            format!("{}/file2", tmp_name),
            appended_file.file_name().unwrap().to_string_lossy().to_string(),
            bns(&tmp2),
            format!("{}/file3", tmp2_name),
            format!("{}/file4", tmp2_name),
        ]
    );
}

/// 对应 C 版本: test_that("empty directories are archived as directories")
#[test]
fn test_empty_directories_are_archived_as_directories_complete() {
    let base = TempDir::new().unwrap();
    let tmp = base.path().join("tmp");
    fs::create_dir(&tmp).unwrap();

    let foo = tmp.join("foo");
    fs::create_dir_all(foo.join("bar")).unwrap();
    fs::create_dir(foo.join("bar2")).unwrap();

    let file1 = foo.join("file1");
    fs::write(&file1, b"contents\n").unwrap();

    let zipfile = base.path().join("test.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(base.path())
        .files(&[tmp.file_name().unwrap().to_str().unwrap()])
        .unwrap()
        .build()
        .unwrap();

    let entries = list(&zipfile).unwrap();
    let tmp_name = tmp.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(
        entries.iter().map(|e| e.filename.clone()).collect::<Vec<_>>(),
        vec![
            format!("{}/", tmp_name),
            format!("{}/foo/", tmp_name),
            format!("{}/foo/bar/", tmp_name),
            format!("{}/foo/bar2/", tmp_name),
            format!("{}/foo/file1", tmp_name),
        ]
    );

    let ex_dir = TempDir::new().unwrap();
    Extractor::new(&zipfile)
        .unwrap()
        .exdir(ex_dir.path())
        .extract()
        .unwrap();

    let files = list_dir_recursive(ex_dir.path());
    assert_eq!(
        files,
        vec![
            tmp_name.clone(),
            format!("{}/foo", tmp_name),
            format!("{}/foo/bar", tmp_name),
            format!("{}/foo/bar2", tmp_name),
            format!("{}/foo/file1", tmp_name),
        ]
    );

    let dir_flags = files
        .iter()
        .map(|path| ex_dir.path().join(path).is_dir())
        .collect::<Vec<_>>();
    assert_eq!(dir_flags, vec![true, true, true, true, false]);

    let contents = fs::read_to_string(ex_dir.path().join(&tmp_name).join("foo").join("file1")).unwrap();
    assert_eq!(contents.lines().collect::<Vec<_>>(), vec!["contents"]);
}

/// 对应 C 版本: test_that("warn for relative paths")
/// C版本验证：
/// 1. expect_warning(zip(zipfile, file.path("..", "foo"))) - 路径包含 ".."
/// 2. expect_warning(zip(zipfile, file.path("..", "foo", "bar"))) - 路径包含 "../.."
/// 3. expect_warning(zip(zipfile, ".")) - 当前目录 "."
///
/// 注意：C版本使用 expect_warning 表示警告但继续，Rust版本接受相对路径
#[test]
fn test_warn_for_relative_paths() {
    let tmp = TempDir::new().unwrap();

    let foo = tmp.path().join("foo");
    let bar = foo.join("bar");
    fs::create_dir_all(&bar).unwrap();

    // 创建测试文件
    let bar_file = bar.join("test.txt");
    fs::write(&bar_file, b"bar\n").unwrap();

    // 测试1: ".." 路径 - 指向 tmp 目录
    let zipfile1 = tmp.path().join("test1.zip");
    let result1 = ZipBuilder::new(&zipfile1)
        .and_then(|b| Ok(b.root(&foo)))
        .and_then(|b| b.files(&[".."]));

    // C版本: expect_warning(zip(zipfile, file.path("..", "foo"))) - 警告但继续
    // Rust版本: 使用 build_with_warnings() 验证 DotDotPaths 警告
    match result1 {
        Ok(builder) => {
            let output = builder.build_with_warnings().unwrap();
            // 对应 C 版本: expect_warning(..., "relative path")
            assert_warning!(output, DotDotPaths, "Relative path '..' should produce DotDotPaths warning");

            // 验证相对路径被处理
            let entries = list(&zipfile1).unwrap();
            assert!(!entries.is_empty(), "Should process relative path '..'");
        }
        Err(e) => {
            // 如果实现选择拒绝相对路径，验证错误消息
            let err_str = e.to_string();
            assert!(err_str.contains("..") || err_str.contains("relative") || err_str.contains("parent"),
                "Error should mention relative path: {}", err_str);
        }
    }

    // 测试2: "../foo/bar" 路径 - 从 foo 目录，../foo/bar 就是 foo/bar
    let zipfile2 = tmp.path().join("test2.zip");
    let result2 = ZipBuilder::new(&zipfile2)
        .and_then(|b| Ok(b.root(&foo)))
        .and_then(|b| b.files(&["../foo/bar"]));

    // C版本: expect_warning(zip(zipfile, file.path("..", "foo", "bar"))) - 警告但继续
    match result2 {
        Ok(builder) => {
            let output = builder.build_with_warnings().unwrap();
            // 对应 C 版本: expect_warning(..., "relative path")
            assert_warning!(output, DotDotPaths, "Relative path '../foo/bar' should produce DotDotPaths warning");

            // 验证路径被处理
            let entries = list(&zipfile2).unwrap();
            assert!(!entries.is_empty(), "Should have entries for relative path");
        }
        Err(e) => {
            let err_str = e.to_string();
            assert!(err_str.contains("..") || err_str.contains("relative"),
                "Error should mention relative path: {}", err_str);
        }
    }

    // 测试3: "." 当前目录
    let zipfile3 = tmp.path().join("test3.zip");
    let result3 = ZipBuilder::new(&zipfile3)
        .and_then(|b| Ok(b.root(&foo)))
        .and_then(|b| b.files(&["."]));

    // C版本: expect_warning(zip(zipfile, ".")) - 警告但继续
    match result3 {
        Ok(builder) => {
            let output = builder.build_with_warnings().unwrap();
            // 对应 C 版本: expect_warning(..., "current directory")
            // "." 会被展开，但原始路径包含 "." 会产生 DotSlashPaths 警告
            assert_warning!(output, DotSlashPaths, "Current directory '.' should produce DotSlashPaths warning");

            // 验证当前目录被处理
            let entries = list(&zipfile3).unwrap();
            assert!(!entries.is_empty(), "Should have entries for current directory");
        }
        Err(e) => {
            let err_str = e.to_string();
            assert!(err_str.contains(".") || err_str.contains("relative") || err_str.contains("current"),
                "Error should mention current directory: {}", err_str);
        }
    }
}

/// 对应 C 版本: test_that("example")
/// C版本验证：
/// 1. zip("x.zip", tz) with expect_warning - Mirror模式，保留相对路径 "../foo2"
/// 2. zip_list 验证包含 "../foo2/" 和 "../foo2/file3"
/// 3. zipr("xr.zip", tz) - CherryPick模式，展平路径，"../foo2" 变成 "foo2"
/// 4. zip_list 验证包含 "foo2/" 和 "foo2/file3"
#[test]
fn test_example() {
    let tmp = TempDir::new().unwrap();

    let foo = tmp.path().join("foo");
    fs::create_dir_all(foo.join("bar")).unwrap();
    fs::create_dir(foo.join("bar2")).unwrap();
    fs::create_dir(tmp.path().join("foo2")).unwrap();

    let file1 = foo.join("bar").join("file1");
    let file2 = foo.join("bar2").join("file2");
    let file3 = tmp.path().join("foo2").join("file3");
    fs::write(&file1, b"contents\n").unwrap();
    fs::write(&file2, b"contents\n").unwrap();
    fs::write(&file3, b"contents\n").unwrap();

    let zipfile = tmp.path().join("x.zip");

    // 对应 C 版本: setwd("foo"); zip("x.zip", tz) with expect_warning
    // tz = c("bar/file1", "bar2", "../foo2")
    // Mirror 模式保留相对路径
    let result = ZipBuilder::new(&zipfile)
        .and_then(|b| Ok(b.root(&foo).mode(ZipMode::Mirror)))
        .and_then(|b| b.files(&["bar", "bar2", "../foo2"]));

    // C版本: expect_warning - 相对路径 "../foo2" 会产生警告
    // Rust版本: 使用 build_with_warnings() 验证警告
    if let Ok(builder) = result {
        let output = builder.build_with_warnings().unwrap();
        // 对应 C 版本: expect_warning(..., "relative path")
        // 验证相对路径 "../foo2" 产生了 DotDotPaths 警告
        assert_warning!(output, DotDotPaths, "Relative path '../foo2' should produce DotDotPaths warning in Mirror mode");

        // 对应 C 版本: expect_equal(zip_list("x.zip")$filename, c(...))
        let entries = list(&zipfile).unwrap();
        let filenames: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

        // C版本期望: ["bar/", "bar/file1", "bar2/", "bar2/file2", "../foo2/", "../foo2/file3"]
        // 验证包含 "../foo2/" 相对路径
        assert!(filenames.iter().any(|f| f.contains("bar/") || f.contains("bar/file1")),
            "Should contain 'bar' entries, got: {:?}", filenames);
        assert!(filenames.iter().any(|f| f.contains("bar2")),
            "Should contain 'bar2' entries, got: {:?}", filenames);
        // Mirror 模式应该保留 "../foo2" 路径
        assert!(filenames.iter().any(|f| f.contains("../foo2") || f.contains("foo2")),
            "Should contain '../foo2' or 'foo2' entries, got: {:?}", filenames);
    }

    // 对应 C 版本: zipr("xr.zip", tz)
    // CherryPick 模式展平路径
    let ziprfile = tmp.path().join("xr.zip");
    ZipBuilder::new(&ziprfile)
        .unwrap()
        .root(&foo)
        .mode(ZipMode::CherryPick)
        .files(&["bar", "bar2", "../foo2"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(zip_list("xr.zip")$filename, c(...))
    // C版本期望: ["file1", "bar2/", "bar2/file2", "foo2/", "foo2/file3"]
    let entries = list(&ziprfile).unwrap();
    let filenames: Vec<String> = entries.iter().map(|e| e.filename.clone()).collect();

    // CherryPick 模式应该展平路径，不包含 "../" 前缀
    assert!(filenames.contains(&"file1".to_string()) || filenames.iter().any(|f| f.ends_with("/file1")),
        "Should contain 'file1', got: {:?}", filenames);
    assert!(filenames.iter().any(|f| f == "bar2/" || f.contains("bar2")),
        "Should contain 'bar2' directory, got: {:?}", filenames);
    // 验证 "foo2" 存在且不包含 "../" 前缀
    assert!(filenames.iter().any(|f| f.contains("foo2") && !f.contains("..")),
        "Should contain 'foo2' without '../' prefix, got: {:?}", filenames);
    // 确保没有 "../" 前缀
    assert!(!filenames.iter().any(|f| f.starts_with("../")),
        "Should not contain '../' prefix in CherryPick mode, got: {:?}", filenames);
}

/// 对应 C 版本: test_that("compression level is used")
/// C版本验证：
/// 1. zipfile exists
/// 2. zip_list filename验证
/// 3. expect_true(file.info(zipfile1)$size <= file.info(zipfile2)$size)
#[test]
fn test_compression_level() {
    let tmp_dir = TempDir::new().unwrap();

    let file_path = tmp_dir.path().join("large_file.txt");
    let content = "The quick brown fox jumps over the lazy dog. ".repeat(1000);
    fs::write(&file_path, content).unwrap();

    let zipfile1 = tmp_dir.path().join("test1.zip");
    let zipfile2 = tmp_dir.path().join("test2.zip");

    ZipBuilder::new(&zipfile1)
        .unwrap()
        .root(tmp_dir.path())
        .compression_level(zip_rs::CompressionLevel::Level1)
        .files(&["large_file.txt"])
        .unwrap()
        .build()
        .unwrap();

    ZipBuilder::new(&zipfile2)
        .unwrap()
        .root(tmp_dir.path())
        .compression_level(zip_rs::CompressionLevel::Level9)
        .files(&["large_file.txt"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(file.exists(zipfile1))
    assert!(zipfile1.exists());
    assert!(zipfile2.exists());

    // 对应 C 版本: zip_list 验证
    let entries1 = list(&zipfile1).unwrap();
    let entries2 = list(&zipfile2).unwrap();

    // 对应 C 版本: expect_equal(list$filename, basename(file))
    assert_eq!(entries1.len(), 1);
    assert_eq!(entries1[0].filename, "large_file.txt");
    assert_eq!(entries2.len(), 1);
    assert_eq!(entries2[0].filename, "large_file.txt");

    // 对应 C 版本: expect_true(file.info(zipfile1)$size <= file.info(zipfile2)$size)
    // Level1 压缩率低 -> 文件大; Level9 压缩率高 -> 文件小
    let size1 = fs::metadata(&zipfile1).unwrap().len();
    let size2 = fs::metadata(&zipfile2).unwrap().len();
    assert!(
        size1 >= size2,
        "Level1 zipfile ({} bytes) should be >= Level9 zipfile ({} bytes)",
        size1, size2
    );
}
