// 路径处理测试
// 对应 C 版本 tests/testthat/test-paths.R

mod common;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use zip_rs::{extract, list, ZipBuilder, ZipMode, ZipProcess, UnzipProcess};
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

fn list_files_recursive(root: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    collect_file_entries(root, root, &mut entries);
    entries.sort();
    entries
}

fn collect_file_entries(root: &Path, dir: &Path, entries: &mut Vec<String>) {
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_file_entries(root, &path, entries);
            } else if let Ok(relative) = path.strip_prefix(root) {
                let relative = relative.to_string_lossy().replace('\\', "/");
                entries.push(relative);
            }
        }
    }
}

/// 对应 C 版本: base path with spaces (lines 1-31)
/// C版本验证：
/// 1. Mirror 模式 zip + zip_list 验证
/// 2. unzip 验证
/// 3. Cherry-pick 模式 zip
/// 4. unzip 验证 (cherry-pick)
#[test]
fn test_base_path_with_spaces() {
    let tmp_dir = TempDir::new().unwrap();

    let dir1 = tmp_dir.path().join("space 1 2");
    let dir1_inner = dir1.join("dir1");
    let dir2 = dir1.join("dir2");
    fs::create_dir_all(&dir1_inner).unwrap();
    fs::create_dir(&dir2).unwrap();

    let file1 = dir1_inner.join("file1");
    let file2 = dir2.join("file2");
    fs::write(&file1, b"file1").unwrap();
    fs::write(&file2, b"file2").unwrap();

    let zipfile1 = dir1.join("zip1.zip");

    // 对应 C 版本: zip::zip("zip1.zip", c("dir1", "dir2"), mode = "mirror")
    ZipBuilder::new(&zipfile1)
        .unwrap()
        .root(&dir1)
        .mode(ZipMode::Mirror)
        .files(&["dir1", "dir2"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(zip_list("zip1.zip")$filename, c("dir1/", "dir1/file1", "dir2/", "dir2/file2"))
    let entries = list(&zipfile1).unwrap();
    insta::assert_snapshot!(format_file_list(&entries));

    // 对应 C 版本: dir.create("ex1"); unzip 验证
    let ex1 = dir1.join("ex1");
    fs::create_dir(&ex1).unwrap();
    extract(&zipfile1, &ex1).unwrap();
    assert_eq!(list_files_recursive(&ex1), vec!["dir1/file1".to_string(), "dir2/file2".to_string()]);

    // 对应 C 版本: zip::zip("zip2.zip", c("dir1", "dir2/file2"), mode = "cherry-pick")
    let zipfile2 = dir1.join("zip2.zip");
    ZipBuilder::new(&zipfile2)
        .unwrap()
        .root(&dir1)
        .mode(ZipMode::CherryPick)
        .files(&["dir1", "dir2/file2"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(zip_list("zip2.zip")$filename, c("dir1/", "dir1/file1", "file2"))
    let entries2 = list(&zipfile2).unwrap();
    insta::assert_snapshot!(format_file_list(&entries2));

    // 对应 C 版本: dir.create("ex2"); unzip 验证
    let ex2 = dir1.join("ex2");
    fs::create_dir(&ex2).unwrap();
    extract(&zipfile2, &ex2).unwrap();
    assert_eq!(list_files_recursive(&ex2), vec!["dir1/file1".to_string(), "file2".to_string()]);
}

/// 对应 C 版本: uncompressed path with spaces (lines 33-61)
/// C版本验证：
/// 1. Mirror 模式 zip + zip_list 验证
/// 2. unzip 验证
/// 3. Cherry-pick 模式 zip
/// 4. unzip 验证 (cherry-pick)
#[test]
fn test_uncompressed_path_with_spaces() {
    let tmp_dir = TempDir::new().unwrap();

    let root = tmp_dir.path().join("root 1 2");
    fs::create_dir(&root).unwrap();

    let file = root.join("file 3 4");
    fs::write(&file, b"contents\n").unwrap();

    let zipfile1 = tmp_dir.path().join("zip1.zip");

    // 对应 C 版本: zip("zip1.zip", root, mode = "mirror")
    ZipBuilder::new(&zipfile1)
        .unwrap()
        .root(tmp_dir.path())
        .mode(ZipMode::Mirror)
        .files(&["root 1 2"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(zip_list("zip1.zip")$filename, ...)
    let entries = list(&zipfile1).unwrap();
    insta::assert_snapshot!(format_file_list(&entries));

    // 对应 C 版本: dir.create("ex1"); unzip 验证
    let ex1 = tmp_dir.path().join("ex1");
    fs::create_dir(&ex1).unwrap();
    extract(&zipfile1, &ex1).unwrap();
    assert_eq!(list_files_recursive(&ex1), vec!["root 1 2/file 3 4".to_string()]);

    // 对应 C 版本: zip("zip2.zip", root, mode = "cherry-pick")
    let zipfile2 = tmp_dir.path().join("zip2.zip");
    ZipBuilder::new(&zipfile2)
        .unwrap()
        .root(tmp_dir.path())
        .mode(ZipMode::CherryPick)
        .files(&["root 1 2"])
        .unwrap()
        .build()
        .unwrap();

    // 对应 C 版本: expect_equal(zip_list("zip2.zip")$filename, ...)
    let entries2 = list(&zipfile2).unwrap();
    insta::assert_snapshot!(format_file_list(&entries2));

    // 对应 C 版本: dir.create("ex2"); unzip 验证
    let ex2 = tmp_dir.path().join("ex2");
    fs::create_dir(&ex2).unwrap();
    extract(&zipfile2, &ex2).unwrap();
    assert_eq!(list_files_recursive(&ex2), vec!["root 1 2/file 3 4".to_string()]);
}

/// 对应 C 版本: zip file with spaces
#[test]
fn test_zip_file_with_spaces() {
    let tmp_dir = TempDir::new().unwrap();

    let dir1 = tmp_dir.path().join("dir1");
    let dir2 = tmp_dir.path().join("dir 2");
    fs::create_dir(&dir1).unwrap();
    fs::create_dir(&dir2).unwrap();

    let file1 = dir1.join("file 1");
    let file2 = dir2.join("file2");
    fs::write(&file1, b"file1").unwrap();
    fs::write(&file2, b"file2").unwrap();

    let zipfile = tmp_dir.path().join("zip 1.zip");

    ZipBuilder::new(&zipfile)
        .unwrap()
        .root(tmp_dir.path())
        .files(&["dir1", "dir 2"])
        .unwrap()
        .build()
        .unwrap();

    assert!(zipfile.exists());

    // 验证可以列出内容
    let entries = list(&zipfile).unwrap();

    // 对应 C 版本: expect_snapshot(list$filename)
    let output = format_file_list(&entries);
    let normalized = normalize_temp_paths(output);
    insta::assert_snapshot!(normalized);
}

/// 对应 C 版本: base path with non-ASCII characters (lines 63-97)
/// C版本: skip("Only on Windows")
/// Rust版本: 在所有平台上测试非ASCII路径支持
/// C版本验证：
/// 1. Mirror 模式 zip + zip_list 验证
/// 2. unzip 验证
/// 3. Cherry-pick 模式 zip
/// 4. unzip 验证 (cherry-pick)
#[test]
fn test_base_path_with_non_ascii_characters() {
    let tmp_dir = TempDir::new().unwrap();

    // C版本使用: enc2native("\u00fa\u00e1\u00f6\u0151\u00e9")
    // 创建包含非ASCII字符的目录名
    let root_name = "úáöőé"; // UTF-8 编码的非ASCII字符

    // 在支持非ASCII文件名的系统上测试
    let root = tmp_dir.path().join(root_name);
    let dir_created = fs::create_dir(&root).is_ok();

    if dir_created {
        let dir1 = root.join("dir1");
        let dir2 = root.join("dir2");
        fs::create_dir(&dir1).unwrap();
        fs::create_dir(&dir2).unwrap();

        let file1 = dir1.join("file1");
        let file2 = dir2.join("file2");
        fs::write(&file1, b"file1").unwrap();
        fs::write(&file2, b"file2").unwrap();

        let zipfile1 = root.join("zip1.zip");

        // 对应 C 版本: zip::zip("zip1.zip", c("dir1", "dir2"), mode = "mirror")
        ZipBuilder::new(&zipfile1)
            .unwrap()
            .root(&root)
            .mode(ZipMode::Mirror)
            .files(&["dir1", "dir2"])
            .unwrap()
            .build()
            .unwrap();

        // 对应 C 版本: expect_equal(zip_list("zip1.zip")$filename, c("dir1/", "dir1/file1", "dir2/", "dir2/file2"))
        let entries = list(&zipfile1).unwrap();
        insta::assert_snapshot!(format_file_list(&entries));

        // 对应 C 版本: dir.create("ex1"); unzip 验证
        let ex1 = root.join("ex1");
        fs::create_dir(&ex1).unwrap();
        extract(&zipfile1, &ex1).unwrap();
        assert_eq!(list_files_recursive(&ex1), vec!["dir1/file1".to_string(), "dir2/file2".to_string()]);

        // 对应 C 版本: zip::zip("zip2.zip", c("dir1", "dir2/file2"), mode = "cherry-pick")
        let zipfile2 = root.join("zip2.zip");
        ZipBuilder::new(&zipfile2)
            .unwrap()
            .root(&root)
            .mode(ZipMode::CherryPick)
            .files(&["dir1", "dir2/file2"])
            .unwrap()
            .build()
            .unwrap();

        // 对应 C 版本: expect_equal(zip_list("zip2.zip")$filename, c("dir1/", "dir1/file1", "file2"))
        let entries2 = list(&zipfile2).unwrap();
        insta::assert_snapshot!(format_file_list(&entries2));

        // 对应 C 版本: dir.create("ex2"); unzip 验证
        let ex2 = root.join("ex2");
        fs::create_dir(&ex2).unwrap();
        extract(&zipfile2, &ex2).unwrap();
        assert_eq!(list_files_recursive(&ex2), vec!["dir1/file1".to_string(), "file2".to_string()]);
    } else {
        insta::assert_snapshot!("Skipped: filesystem does not support non-ASCII names");
    }
}

/// 对应 C 版本: uncompressed path with non-ASCII characters (lines 99-132)
/// C版本: skip("Only on Windows")
/// Rust版本: 在所有平台上测试非ASCII路径支持
/// C版本验证：
/// 1. Mirror 模式 zip + zip_list 验证
/// 2. unzip 验证
/// 3. Cherry-pick 模式 zip
/// 4. unzip 验证 (cherry-pick)
#[test]
fn test_uncompressed_path_with_non_ascii_characters() {
    let tmp_dir = TempDir::new().unwrap();

    // C版本使用: enc2native("\u00fa\u00e1\u00f6\u0151\u00e9")
    // 创建包含非ASCII字符的目录名
    let root_name = "úáöőé";
    let root = tmp_dir.path().join(root_name);
    let dir_created = fs::create_dir(&root).is_ok();

    if dir_created {
        // C版本使用: ufile <- enc2native("ufile\u00fa\u00e1")
        let ufile_name = "ufileúá";
        let ufile = root.join(ufile_name);

        fs::write(&ufile, b"contents\n").unwrap();

        let zipfile1 = tmp_dir.path().join("zip1.zip");

        // 对应 C 版本: zip("zip1.zip", root, mode = "mirror")
        ZipBuilder::new(&zipfile1)
            .unwrap()
            .root(tmp_dir.path())
            .mode(ZipMode::Mirror)
            .files(&[root_name])
            .unwrap()
            .build()
            .unwrap();

        // 对应 C 版本: expect_equal(zip_list("zip1.zip")$filename, ...)
        let entries = list(&zipfile1).unwrap();
        insta::assert_snapshot!(format_file_list(&entries));

        // 对应 C 版本: dir.create("ex1"); unzip 验证
        // C版本: zip::unzip(symlink, exdir = ex1)
        let ex1 = tmp_dir.path().join("ex1");
        fs::create_dir(&ex1).unwrap();
        extract(&zipfile1, &ex1).unwrap();
        assert_eq!(list_files_recursive(&ex1), vec![format!("{}/{}", root_name, ufile_name)]);

        // 对应 C 版本: zip("zip2.zip", root, mode = "cherry-pick")
        let zipfile2 = tmp_dir.path().join("zip2.zip");
        ZipBuilder::new(&zipfile2)
            .unwrap()
            .root(tmp_dir.path())
            .mode(ZipMode::CherryPick)
            .files(&[root_name])
            .unwrap()
            .build()
            .unwrap();

        // 对应 C 版本: expect_equal(zip_list("zip2.zip")$filename, ...)
        let entries2 = list(&zipfile2).unwrap();
        insta::assert_snapshot!(format_file_list(&entries2));

        // 对应 C 版本: dir.create("ex2"); unzip 验证
        let ex2 = tmp_dir.path().join("ex2");
        fs::create_dir(&ex2).unwrap();
        extract(&zipfile2, &ex2).unwrap();
        assert_eq!(list_files_recursive(&ex2), vec![format!("{}/{}", root_name, ufile_name)]);
    } else {
        insta::assert_snapshot!("Skipped: filesystem does not support non-ASCII names");
    }
}

/// 对应 C 版本: zip file with non-ASCII characters (lines 164-219)
/// C版本: skip_on_cran()
#[test]
fn test_zip_file_with_non_ascii_characters() {
    let tmp_dir = TempDir::new().unwrap();

    // C版本使用: enc2native("x-\u00fa\u00e1\u00f6\u0151\u00e9.zip")
    // 创建包含非ASCII字符的ZIP文件名
    let zipfile_name = "x-úáöőé.zip";
    let zipfile = tmp_dir.path().join(zipfile_name);

    let dir1 = tmp_dir.path().join("dir1");
    let dir2 = tmp_dir.path().join("dir2");
    fs::create_dir(&dir1).unwrap();
    fs::create_dir(&dir2).unwrap();

    let file1 = dir1.join("file1");
    let file2 = dir2.join("file2");
    fs::write(&file1, b"file1").unwrap();
    fs::write(&file2, b"file2").unwrap();

    // 尝试创建非ASCII文件名的ZIP
    let result = ZipBuilder::new(&zipfile)
        .and_then(|b| Ok(b.root(tmp_dir.path()).mode(ZipMode::Mirror)))
        .and_then(|b| b.files(&["dir1", "dir2"]));

    match result {
        Ok(builder) => {
            builder.build().unwrap();

            // 验证可以列出内容
            let entries = list(&zipfile).unwrap();
            assert_eq!(
                entries.iter().map(|e| e.filename.clone()).collect::<Vec<_>>(),
                vec![
                    "dir1/".to_string(),
                    "dir1/file1".to_string(),
                    "dir2/".to_string(),
                    "dir2/file2".to_string(),
                ]
            );

            let ex1 = tmp_dir.path().join("ex1");
            fs::create_dir(&ex1).unwrap();
            extract(&zipfile, &ex1).unwrap();
            assert_eq!(
                list_files_recursive(&ex1),
                vec!["dir1/file1".to_string(), "dir2/file2".to_string()]
            );

            let _ = fs::remove_file(&zipfile);

            let result = ZipBuilder::new(&zipfile)
                .and_then(|b| Ok(b.root(tmp_dir.path()).mode(ZipMode::CherryPick)))
                .and_then(|b| b.files(&["dir1", "dir2/file2"]));
            let builder = result.unwrap();
            builder.build().unwrap();

            let entries = list(&zipfile).unwrap();
            assert_eq!(
                entries.iter().map(|e| e.filename.clone()).collect::<Vec<_>>(),
                vec![
                    "dir1/".to_string(),
                    "dir1/file1".to_string(),
                    "file2".to_string(),
                ]
            );

            let ex2 = tmp_dir.path().join("ex2");
            fs::create_dir(&ex2).unwrap();
            extract(&zipfile, &ex2).unwrap();
            assert_eq!(
                list_files_recursive(&ex2),
                vec!["dir1/file1".to_string(), "file2".to_string()]
            );

            let _ = fs::remove_file(&zipfile);

            // 测试命令行工具 - 需要在正确的目录下运行
            // 注意：如果ziprs不在PATH中，这部分测试会失败
            let _guard = CurrentDirGuard::new(tmp_dir.path());
            // 使用相对路径，因为当前目录已改为tmp_dir.path()
            let zipfile_name = zipfile.file_name().unwrap().to_string_lossy().to_string();
            match ZipProcess::new(&zipfile_name, &["dir1", "dir2"], true, true) {
                Ok(mut process) => {
                    process.wait(Some(5000)).unwrap();
                    let _ = process.kill();
                    assert_eq!(process.get_exit_status(), Some(0));

                    // ZipProcess 成功后验证ZIP内容
                    let entries = list(&zipfile).unwrap();
                    assert_eq!(
                        entries.iter().map(|e| e.filename.clone()).collect::<Vec<_>>(),
                        vec![
                            "dir1/".to_string(),
                            "dir1/file1".to_string(),
                            "dir2/".to_string(),
                            "dir2/file2".to_string(),
                        ]
                    );

                    let ex3 = tmp_dir.path().join("ex3");
                    fs::create_dir(&ex3).unwrap();
                    let zipfile_name = zipfile.file_name().unwrap().to_string_lossy().to_string();
                    match UnzipProcess::new(&zipfile_name, &ex3) {
                        Ok(mut unzip_process) => {
                            unzip_process.wait(Some(5000)).unwrap();
                            let _ = unzip_process.kill();
                            assert_eq!(unzip_process.get_exit_status(), Some(0));
                            assert_eq!(
                                list_files_recursive(&ex3),
                                vec!["dir1/file1".to_string(), "dir2/file2".to_string()]
                            );
                        }
                        Err(e) => {
                            eprintln!("Skipping UnzipProcess test (unziprs not found): {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    // 如果ziprs找不到，跳过命令行工具测试
                    // 这在某些环境中是预期的（当ziprs不在PATH中时）
                    eprintln!("Skipping ZipProcess test (ziprs not found): {:?}", e);
                }
            }
        }
        Err(e) => {
            // 对应 C 版本: expect_snapshot(error = TRUE)
            let error_msg = format!("{:?}", e);
            insta::assert_snapshot!(error_msg);
        }
    }
}
