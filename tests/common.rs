// 公共测试辅助函数
// 对应 C 版本 tests/testthat/helper.R

use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use tempfile::TempDir;
use zip_rs::{ZipBuilder, list};

/// 返回带斜杠的目录名
pub fn bns(path: &Path) -> String {
    format!("{}/", path.file_name().unwrap().to_string_lossy())
}

/// 创建临时测试目录
pub fn test_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp dir")
}

/// 创建临时测试文件
pub fn test_temp_file() -> TempDir {
    TempDir::new().expect("Failed to create temp dir for file")
}

/// 创建一个包含测试文件的 ZIP
pub fn make_a_zip() -> ZipFixture {
    let tmp_dir = test_temp_dir();

    // 创建测试文件
    let file1 = tmp_dir.path().join("file1");
    let file11 = tmp_dir.path().join("file11");
    let dir = tmp_dir.path().join("dir");
    fs::create_dir(&dir).unwrap();
    let file2 = dir.join("file2");
    let file3 = dir.join("file3");

    fs::write(&file1, "file1\n").unwrap();
    fs::write(&file11, "file11\n").unwrap();
    fs::write(&file2, "file2\n").unwrap();
    fs::write(&file3, "file3\n").unwrap();

    // 创建 ZIP 文件
    let zip_path = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zip_path)
        .expect("Failed to create ZIP builder")
        .root(tmp_dir.path())
        .files(&["file1", "file11", "dir"])
        .expect("Failed to add files")
        .build()
        .expect("Failed to build ZIP");

    ZipFixture {
        zip: zip_path,
        ex: tmp_dir.path().to_path_buf(),
    }
}

/// ZIP 测试数据结构（对应 C 版本的 make_a_zip）
#[derive(Debug, Clone)]
pub struct ZipData {
    pub zipfile: PathBuf,
    pub files: Vec<String>,
}

/// 创建 ZIP 数据结构（简化版本）
pub fn make_zip_data(files: &[&str]) -> ZipData {
    let tmp_dir = test_temp_dir();

    // 创建测试文件
    for file in files {
        let file_path = tmp_dir.path().join(file);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, format!("{}\n", file)).unwrap();
    }

    // 创建 ZIP 文件
    let zip_path = tmp_dir.path().join("test.zip");

    ZipBuilder::new(&zip_path)
        .expect("Failed to create ZIP builder")
        .root(tmp_dir.path())
        .files(files)
        .expect("Failed to add files")
        .build()
        .expect("Failed to build ZIP");

    ZipData {
        zipfile: zip_path,
        files: files.iter().map(|s| s.to_string()).collect(),
    }
}

/// 标准化临时路径（对应 C 版本的 transform_tempdir）
///
/// 将临时目录路径替换为占位符，用于快照测试
pub fn normalize_temp_paths(output: String) -> String {
    let mut output = output;

    // 替换系统临时目录路径
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        output = output.replace(&tmpdir, "<tempdir>");
    }
    if let Ok(tmpdir) = std::env::var("TEMP") {
        output = output.replace(&tmpdir, "<tempdir>");
    }
    if let Ok(tmpdir) = std::env::var("TMP") {
        output = output.replace(&tmpdir, "<tempdir>");
    }
    output = output.replace("/tmp", "<tempdir>");
    output = output.replace("\\tmp", "<tempdir>");

    // Windows 路径处理
    #[cfg(windows)]
    {
        output = output.replace("\\", "/");
        // 替换 Windows 临时路径 (如 C:\Users\...\AppData\Local\Temp)
        let re = regex::Regex::new(r"[A-Z]:.*[\\/]Temp[\\/]").unwrap();
        output = re.replace_all(&output, "<tempdir>/").to_string();
        // 替换 Rtmp 风格的临时路径
        let re = regex::Regex::new(r"[A-Z]:.*Rtmp[a-zA-Z0-9]+[\\/]").unwrap();
        output = re.replace_all(&output, "<tempdir>/").to_string();
    }

    // 替换临时文件名 (file 后跟随机字母数字)
    let re = regex::Regex::new(r"[\\/]file[a-zA-Z0-9]+").unwrap();
    output = re.replace_all(&output, "/<tempfile>").to_string();

    // 替换相对临时目录路径 (如 .tmpXXXXXX/ 或 /tmp/.tmpXXXXXX/)
    // 这处理 tempfile crate 创建的目录
    let re = regex::Regex::new(r"\.tmp[a-zA-Z0-9]+").unwrap();
    output = re.replace_all(&output, ".tmpXXXXXX").to_string();

    output
}

/// ZIP 测试夹具
pub struct ZipFixture {
    pub zip: PathBuf,
    pub ex: PathBuf,
}

/// 警告验证宏 - 对应 C 版本的 expect_warning
///
/// 用法：assert_warning!(output, DirectoriesIgnored)
#[macro_export]
macro_rules! assert_warning {
    ($output:expr, $warning_variant:ident) => {
        assert!(
            $output.warnings.iter().any(|w| matches!(w, zip_rs::ZipWarning::$warning_variant)),
            "Expected warning {} not found in: {:?}",
            stringify!($warning_variant),
            $output.warnings
        );
    };
    ($output:expr, $warning_variant:ident, $message:expr) => {
        assert!(
            $output.warnings.iter().any(|w| matches!(w, zip_rs::ZipWarning::$warning_variant)),
            "{} - Expected warning {} not found in: {:?}",
            $message,
            stringify!($warning_variant),
            $output.warnings
        );
    };
}
