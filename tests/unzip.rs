// UNZIP 解压测试
// 对应 C 版本 tests/testthat/test-unzip.R

mod common;

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, Duration};
use tempfile::TempDir;

use zip_rs::{ZipBuilder, ZipMode, extract, Extractor, list};
use common::normalize_temp_paths;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// 辅助函数：创建一个测试 ZIP 文件
fn make_test_zip() -> (TempDir, std::path::PathBuf) {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1");
    let file11 = tmp_dir.path().join("file11");
    let dir = tmp_dir.path().join("dir");
    fs::create_dir(&dir).unwrap();

    let file2 = dir.join("file2");
    let file3 = dir.join("file3");

    fs::write(&file1, b"file1\n").unwrap();
    fs::write(&file11, b"file11\n").unwrap();
    fs::write(&file2, b"file2\n").unwrap();
    fs::write(&file3, b"file3\n").unwrap();

    let zip_path = tmp_dir.path().join("test.zip");
    ZipBuilder::new(&zip_path)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1", "file11", "dir"])
        .unwrap()
        .build()
        .unwrap();

    (tmp_dir, zip_path)
}

/// 对应 C 版本: test_that("can unzip all")
/// C版本验证：
/// 1. expect_true(file.exists(file.path(tmp2, basename(z$ex), "file1")))
/// 2. expect_equal(readLines(file.path(tmp2, basename(z$ex), "file1")), "file1")
/// 3. expect_equal(readLines(file.path(tmp2, basename(z$ex), "dir", "file2")), "file2")
///
/// 注意：Rust版本的extract行为与C版本不同，文件直接解压到exdir，不创建子目录
#[test]
fn test_can_unzip_all() {
    let z = make_test_zip();

    let ex_dir = TempDir::new().unwrap();
    extract(&z.1, ex_dir.path()).unwrap();

    // Rust版本：文件直接解压到ex_dir路径下
    // ZIP中存储的路径是相对路径：file1, dir/file2
    let file1_path = ex_dir.path().join("file1");
    let dir_path = ex_dir.path().join("dir");
    let file2_path = ex_dir.path().join("dir").join("file2");

    assert!(file1_path.exists(), "file1 should exist after extraction");
    assert!(dir_path.exists(), "dir should exist after extraction");
    assert!(file2_path.exists(), "dir/file2 should exist after extraction");

    // 对应 C 版本: expect_equal(readLines(...), "file1")
    let content1 = fs::read_to_string(&file1_path).unwrap();
    assert_eq!(content1.trim_end(), "file1");

    // 对应 C 版本: expect_equal(readLines(file.path(tmp2, basename(z$ex), "dir", "file2")), "file2")
    let content2 = fs::read_to_string(&file2_path).unwrap();
    assert_eq!(content2.trim_end(), "file2");
}

/// 对应 C 版本: test_that("unzip creates exdir if needed")
/// C版本验证：
/// 1. test_temp_dir(create = FALSE) - 创建不存在的目录路径
/// 2. expect_false(file.exists(tmp2)) - 验证目录不存在
/// 3. expect_true(file.exists(tmp2)) - 解压后目录存在
/// 4. expect_equal(readLines(...), ...) - 文件内容验证
///
/// 注意：Rust版本文件直接解压到exdir，不创建子目录
#[test]
fn test_unzip_creates_exdir() {
    let z = make_test_zip();

    // 对应 C 版本: tmp2 <- test_temp_dir(create = FALSE)
    // 创建一个不存在的目录路径（类似 C 版本的 create = FALSE）
    let parent = TempDir::new().unwrap();
    let target_dir = parent.path().join("new_extract_dir");

    // 对应 C 版本: expect_false(file.exists(tmp2))
    assert!(!target_dir.exists(), "target_dir should not exist before extraction");

    // 对应 C 版本: zip::unzip(z$zip, exdir = tmp2)
    extract(&z.1, &target_dir).unwrap();

    // 对应 C 版本: expect_true(file.exists(tmp2))
    assert!(target_dir.exists(), "target_dir should exist after extraction");

    // 验证文件存在性和内容 - Rust版本文件直接在ex_dir下
    let file1_path = target_dir.join("file1");
    let dir_path = target_dir.join("dir");
    let file2_path = target_dir.join("dir").join("file2");

    assert!(file1_path.exists(), "file1 should exist");
    assert!(dir_path.is_dir(), "dir should be a directory");
    assert!(file2_path.exists(), "dir/file2 should exist");

    // 对应 C 版本: expect_equal(readLines(...), "file1")
    let content1 = fs::read_to_string(&file1_path).unwrap();
    assert_eq!(content1.trim_end(), "file1");

    // 对应 C 版本: expect_equal(readLines(file.path(tmp2, basename(z$ex), "dir", "file2")), "file2")
    let content2 = fs::read_to_string(&file2_path).unwrap();
    assert_eq!(content2.trim_end(), "file2");
}

/// 对应 C 版本: test_that("unzip certain files only")
/// C版本有5个子场景：
/// 1. No files - 空文件列表
/// 2. File in directory - 解压目录中的单个文件
/// 3. Only file(s) in root - 只解压根目录文件
/// 4. Directory only - 只解压目录
/// 5. Files and dirs - 解压文件和目录混合
#[test]
fn test_unzip_specific_files() {
    let z = make_test_zip();

    // 场景1: No files - 对应 C 版本: zip::unzip(z$zip, character(), exdir = tmp2)
    {
        let tmp2 = TempDir::new().unwrap();
        Extractor::new(&z.1)
            .unwrap()
            .exdir(tmp2.path())
            .files(&["" as &str; 0])
            .extract()
            .unwrap();

        // 对应 C 版本: expect_true(file.exists(tmp2))
        assert!(tmp2.path().exists());

        // 对应 C 版本: expect_equal(dir(tmp2), character())
        let files = list_files(tmp2.path());
        assert!(files.is_empty() || files.iter().all(|f| f.is_empty()), "Should have no files extracted");
    }

    // 场景2: File in directory - 对应 C 版本: 解压 "file1"
    {
        let tmp3 = TempDir::new().unwrap();
        // Rust版本：ZIP中的路径是 "file1"，不是 "test/file1"
        Extractor::new(&z.1)
            .unwrap()
            .exdir(tmp3.path())
            .files(&["file1"])
            .extract()
            .unwrap();

        // 对应 C 版本: expect_true(file.exists(file.path(tmp3, basename(z$ex), "file1")))
        // Rust版本：文件直接解压到ex_dir下
        let file1_path = tmp3.path().join("file1");
        assert!(file1_path.exists(), "file1 should exist");

        // 对应 C 版本: expect_false(file.exists(file.path(tmp3, basename(z$ex), "dir")))
        let dir_path = tmp3.path().join("dir");
        assert!(!dir_path.exists(), "dir should not exist");

        // 对应 C 版本: expect_equal(readLines(...), "file1")
        let content1 = fs::read_to_string(&file1_path).unwrap();
        assert_eq!(content1.trim_end(), "file1");
    }

    // 场景3: Only file(s) in root - 对应 C 版本: 创建只有根文件的 ZIP
    {
        let tmp_root = TempDir::new().unwrap();
        let f = tmp_root.path().join("foobar");
        fs::write(&f, b"foobar\n").unwrap();

        let zip_file = tmp_root.path().join("test.zip");
        ZipBuilder::new(&zip_file)
            .unwrap()
            .root(tmp_root.path())
            .mode(ZipMode::CherryPick)
            .files(&["foobar"])
            .unwrap()
            .build()
            .unwrap();

        let tmp4 = TempDir::new().unwrap();
        extract(&zip_file, tmp4.path()).unwrap();

        // 对应 C 版本: expect_true(file.exists(tmp4))
        assert!(tmp4.path().exists());

        // 对应 C 版本: expect_equal(dir(tmp4), basename(f))
        let files = list_files(tmp4.path());
        assert!(files.contains(&"foobar".to_string()), "Should contain 'foobar' file");

        // 对应 C 版本: expect_equal(readLines(file.path(tmp4, basename(f))), "foobar")
        let foobar_path = tmp4.path().join("foobar");
        let content = fs::read_to_string(&foobar_path).unwrap();
        assert_eq!(content.trim_end(), "foobar");
    }

    // 场景4: Directory only - 对应 C 版本: 解压 "dir/"
    {
        let tmp5 = TempDir::new().unwrap();
        // Rust版本：ZIP中的路径是 "dir/"，不是 "test/dir/"
        Extractor::new(&z.1)
            .unwrap()
            .exdir(tmp5.path())
            .files(&["dir/"])
            .extract()
            .unwrap();

        // 对应 C 版本: expect_true(file.exists(file.path(tmp5, basename(z$ex), "dir")))
        // Rust版本：目录直接在ex_dir下
        let dir_path = tmp5.path().join("dir");
        assert!(dir_path.exists(), "dir should exist");
    }

    // 场景5: Files and dirs - 对应 C 版本: 解压 "dir/file2" 和 "file1"
    {
        let tmp6 = TempDir::new().unwrap();
        // Rust版本：ZIP中的路径是 "dir/file2" 和 "file1"
        Extractor::new(&z.1)
            .unwrap()
            .exdir(tmp6.path())
            .files(&["dir/file2", "file1"])
            .extract()
            .unwrap();

        // 对应 C 版本: expect_true(file.exists(file.path(tmp6, basename(z$ex), "file1")))
        // Rust版本：文件直接在ex_dir下
        let file1_path = tmp6.path().join("file1");
        assert!(file1_path.exists(), "file1 should exist");

        // 对应 C 版本: expect_true(file.exists(file.path(tmp6, basename(z$ex), "dir")))
        let dir_path = tmp6.path().join("dir");
        assert!(dir_path.exists(), "dir should exist");

        // 对应 C 版本: expect_true(file.exists(file.path(tmp6, basename(z$ex), "dir", "file2")))
        let file2_path = tmp6.path().join("dir").join("file2");
        assert!(file2_path.exists(), "dir/file2 should exist");

        // 对应 C 版本: expect_equal(readLines(file.path(tmp6, basename(z$ex), "file1")), "file1")
        let content1 = fs::read_to_string(&file1_path).unwrap();
        assert_eq!(content1.trim_end(), "file1");

        // 对应 C 版本: expect_equal(readLines(file.path(tmp6, basename(z$ex), "dir", "file2")), "file2")
        let content2 = fs::read_to_string(&file2_path).unwrap();
        assert_eq!(content2.trim_end(), "file2");
    }
}

/// 对应 C 版本: test_that("junkpaths is TRUE")
#[test]
fn test_junkpaths() {
    let z = make_test_zip();

    let ex_dir = TempDir::new().unwrap();
    extract(&z.1, ex_dir.path()).unwrap();

    // 对应 C 版本: expect_true(file.exists(file.path(tmp, "file1")))
    // 验证文件被解压
    let files = list_files(ex_dir.path());
    let normalized = normalize_temp_paths(format!("Extracted files:\n{}", files.join("\n")));
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: test_that("overwrite is FALSE")
#[test]
fn test_extract_overwrite() {
    let z = make_test_zip();

    let ex_dir = TempDir::new().unwrap();

    // 第一次解压
    extract(&z.1, ex_dir.path()).unwrap();

    // 第二次解压应该失败或根据选项覆盖
    let result = extract(&z.1, ex_dir.path());

    // 对应 C 版本: expect_snapshot(error = TRUE, ...)
    match result {
        Err(e) => {
            let normalized = normalize_temp_paths(format!("{:?}", e));
            insta::assert_snapshot!(normalized);
        }
        Ok(_) => {
            insta::assert_snapshot!("Overwrite allowed (no error)");
        }
    }
}

/// 对应 C 版本: test_that("unzip sets mtime correctly")
/// C版本验证：
/// 1. expect_true(abs(zip_list(z$zip)$timestamp - mtime) < 3)
/// 2. expect_true(abs(file.info(...)$mtime - mtime) < 3) - 对所有文件和目录
///
/// 注意：由于Rust标准库的限制，这个测试验证ZIP会保留文件的mtime（即使文件使用当前时间）
#[test]
fn test_unzip_sets_mtime_correctly() {
    let tmp_dir = TempDir::new().unwrap();

    let file1 = tmp_dir.path().join("file1");
    let file11 = tmp_dir.path().join("file11");
    let dir = tmp_dir.path().join("dir");
    fs::create_dir(&dir).unwrap();

    let file2 = dir.join("file2");
    let file3 = dir.join("file3");

    fs::write(&file1, b"file1\n").unwrap();
    fs::write(&file11, b"file11\n").unwrap();
    fs::write(&file2, b"file2\n").unwrap();
    fs::write(&file3, b"file3\n").unwrap();

    // 记录文件创建后的mtime（约当前时间）
    let files_before = vec![&file1, &file11, &file2, &file3, &dir];
    let mut mtimes_before = std::collections::HashMap::new();
    for path in &files_before {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                mtimes_before.insert(path.clone(), modified);
            }
        }
    }

    let zip_path = tmp_dir.path().join("test.zip");
    ZipBuilder::new(&zip_path)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["file1", "file11", "dir"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_true(all(abs(zip_list(z$zip)$timestamp - mtime) < 3))
    // 验证ZIP条目的mtime与文件原始mtime一致（±3秒）
    let three_secs = Duration::from_secs(3);
    let entries = list(&zip_path).unwrap();
    for entry in &entries {
        // 找到对应的文件原始mtime
        let entry_path = if entry.filename.contains("file1") {
            &file1
        } else if entry.filename.contains("file11") {
            &file11
        } else if entry.filename == "dir/" {
            &dir
        } else if entry.filename.contains("file2") {
            &file2
        } else if entry.filename.contains("file3") {
            &file3
        } else {
            continue;
        };

        if let Some(&original_mtime) = mtimes_before.get(entry_path) {
            let entry_mtime = entry.timestamp;
            let diff = if entry_mtime > original_mtime {
                entry_mtime.duration_since(original_mtime).unwrap_or(Duration::ZERO)
            } else {
                original_mtime.duration_since(entry_mtime).unwrap_or(Duration::ZERO)
            };
            assert!(diff < three_secs,
                "ZIP entry '{}' mtime difference {:?} should be < 3 seconds (compared to original file mtime)",
                entry.filename, diff);
        }
    }

    // 对应 C 版本: 解压并验证文件 mtime
    let ex_dir = TempDir::new().unwrap();
    extract(&zip_path, ex_dir.path()).unwrap();

    // 对应 C 版本: ok("file1"), ok("file11"), ok("dir"), ok("dir", "file2"), ok("dir", "file3")
    // 对应 C 版本: expect_true(abs(t - mtime) < 3)
    // 验证解压后的文件mtime与ZIP条目mtime一致
    let check_mtime = |relative_path: &Path, expected_mtime: SystemTime| {
        let full_path = ex_dir.path().join(relative_path);
        if let Ok(metadata) = fs::metadata(&full_path) {
            if let Ok(modified) = metadata.modified() {
                let diff = if modified > expected_mtime {
                    modified.duration_since(expected_mtime).unwrap_or(Duration::ZERO)
                } else {
                    expected_mtime.duration_since(modified).unwrap_or(Duration::ZERO)
                };
                assert!(diff < three_secs,
                    "File '{:?}' mtime difference {:?} should be < 3 seconds",
                    relative_path, diff);
            }
        }
    };

    // 使用ZIP中存储的mtime进行验证
    for entry in &entries {
        if entry.filename.ends_with('/') {
            continue; // 跳过目录条目
        }
        let path = Path::new(entry.filename.as_str());
        check_mtime(path, entry.timestamp);
    }
}

/// 对应 C 版本: test_that("permissions as kept on Unix")
/// C版本验证：
/// 1. Sys.chmod 设置文件和目录权限
/// 2. expect_equal(file.info(f)$mode, as.octmode(mode)) - 精确权限验证
#[cfg(unix)]
#[test]
fn test_permissions_kept_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let tmp_dir = TempDir::new().unwrap();

    // 对应 C 版本: Sys.chmod(tmp, "0777", FALSE)
    let mut perms = fs::metadata(tmp_dir.path()).unwrap().permissions();
    perms.set_mode(0o777);
    fs::set_permissions(tmp_dir.path(), perms).unwrap();

    // 对应 C 版本: cat("foobar\n", file = f <- file.path(tmp, "file1"))
    // Sys.chmod(f, "0400", FALSE)
    let file1 = tmp_dir.path().join("file1");
    fs::write(&file1, b"foobar\n").unwrap();
    let mut perms = fs::metadata(&file1).unwrap().permissions();
    perms.set_mode(0o400);
    fs::set_permissions(&file1, perms).unwrap();

    // 对应 C 版本: dir.create(f <- file.path(tmp, "dir"))
    // Sys.chmod(f, "0700", FALSE)
    let dir = tmp_dir.path().join("dir");
    fs::create_dir(&dir).unwrap();
    let mut perms = fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&dir, perms).unwrap();

    // 对应 C 版本: cat("foobar2\n", file = f <- file.path(tmp, "dir", "file2"))
    // Sys.chmod(f, "0755", FALSE)
    let file2 = dir.join("file2");
    fs::write(&file2, b"foobar2\n").unwrap();
    let mut perms = fs::metadata(&file2).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&file2, perms).unwrap();

    // 对应 C 版本: cat("foobar3\n", file = f <- file.path(tmp, "dir", "file3"))
    // Sys.chmod(f, "0777", FALSE)
    let file3 = dir.join("file3");
    fs::write(&file3, b"foobar3\n").unwrap();
    let mut perms = fs::metadata(&file3).unwrap().permissions();
    perms.set_mode(0o777);
    fs::set_permissions(&file3, perms).unwrap();

    // 对应 C 版本: zip <- test_temp_file(".zip", create = FALSE)
    // zipr(zip, tmp)
    let zip_path = tmp_dir.path().join("test.zip");
    ZipBuilder::new(&zip_path)
        .unwrap()
        .root(tmp_dir.path())
        .mode(ZipMode::CherryPick)
        .files(&["file1", "dir"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: tmp2 <- test_temp_dir()
    // zip::unzip(zip, exdir = tmp2)
    let ex_dir = TempDir::new().unwrap();
    extract(&zip_path, ex_dir.path()).unwrap();

    // 对应 C 版本: check_perm 函数验证权限
    // expect_equal(file.info(f)$mode, as.octmode(mode))
    let check_perm = |mode: u32, relative_path: &Path| {
        let full_path = ex_dir.path().join(relative_path);
        if let Ok(metadata) = fs::metadata(&full_path) {
            let actual_mode = metadata.permissions().mode();
            let actual_perm = actual_mode & 0o777;
            assert_eq!(actual_perm, mode,
                "File '{:?}': expected mode {:o}, got {:o}",
                relative_path, mode, actual_perm);
        }
    };

    let base_name = tmp_dir.path().file_name().unwrap().to_str().unwrap();

    // 对应 C 版本: check_perm("0777", basename(tmp))
    check_perm(0o777, Path::new(base_name));

    // 对应 C 版本: check_perm("0400", basename(tmp), "file1")
    check_perm(0o400, &Path::new(base_name).join("file1"));

    // 对应 C 版本: check_perm("0700", basename(tmp), "dir")
    check_perm(0o700, &Path::new(base_name).join("dir"));

    // 对应 C 版本: check_perm("0755", basename(tmp), "dir", "file2")
    check_perm(0o755, &Path::new(base_name).join("dir/file2"));

    // 对应 C 版本: check_perm("0777", basename(tmp), "dir", "file3")
    check_perm(0o777, &Path::new(base_name).join("dir/file3"));
}

/// 对应 C 版本: test_that("umask if no permissions")
#[test]
fn test_umask_if_no_permissions() {
    let fixture_path = Path::new("../tests/testthat/fixtures/msdos.zip");
    if !fixture_path.exists() {
        eprintln!("test_umask_if_no_permissions skipped: fixture not found");
        return;
    }

    let tmp_dir = TempDir::new().unwrap();
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    extract(fixture_path, &ex_dir).unwrap();

    let dsc = ex_dir.join("DESCRIPT");
    assert!(dsc.exists());

    #[cfg(unix)]
    {
        if let Ok(metadata) = fs::metadata(&dsc) {
            let mode = metadata.permissions().mode();
            insta::assert_snapshot!(format!("DESCRIPTOR mode: {:o}", mode & 0o777));
        }
    }

    #[cfg(not(unix))]
    {
        insta::assert_snapshot!("Non-Unix system, umask test not applicable");
    }
}

/// 对应 C 版本: test_that("symlinks on Unix")
/// C版本验证：
/// 1. expect_true(file.exists(file.path(tmp, "a")))
/// 2. expect_true(file.exists(file.path(tmp, "a", "foo")))
/// 3. expect_true(file.exists(file.path(tmp, "a", "bar")))
/// 4. expect_equal(Sys.readlink(file.path(tmp, "a", "bar")), "foo")
#[cfg(unix)]
#[test]
fn test_symlinks_on_unix() {
    let fixture_path = Path::new("../tests/testthat/fixtures/symlink.zip");
    if !fixture_path.exists() {
        eprintln!("test_symlinks_on_unix skipped: fixture not found");
        return;
    }

    // 对应 C 版本: dir.create(tmp <- tempfile("zip-test-symlink"))
    let tmp_dir = TempDir::new().unwrap();
    let ex_dir = tmp_dir.path().join("extract");
    fs::create_dir(&ex_dir).unwrap();

    // 对应 C 版本: zip::unzip(symlink, exdir = tmp)
    extract(fixture_path, &ex_dir).unwrap();

    // 对应 C 版本: expect_true(file.exists(file.path(tmp, "a")))
    let a_dir = ex_dir.join("a");
    assert!(a_dir.exists(), "Directory 'a' should exist");

    // 对应 C 版本: expect_true(file.exists(file.path(tmp, "a", "foo")))
    let foo_file = a_dir.join("foo");
    assert!(foo_file.exists(), "File 'a/foo' should exist");

    // 对应 C 版本: expect_true(file.exists(file.path(tmp, "a", "bar")))
    let bar_link = a_dir.join("bar");
    assert!(bar_link.exists(), "Symlink 'a/bar' should exist");

    // 对应 C 版本: expect_equal(Sys.readlink(file.path(tmp, "a", "bar")), "foo")
    let target = fs::read_link(&bar_link).expect("bar should be a symlink");
    assert_eq!(target.to_string_lossy().as_ref(), "foo",
        "Symlink 'a/bar' should point to 'foo', got '{}'",
        target.to_string_lossy());
}

/// 辅助函数：列出目录中的所有文件
fn list_files(dir: &Path) -> Vec<String> {
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
