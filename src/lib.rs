//! zip-rs - ZIP 压缩库
//!
//! 一个纯 Rust 实现的 ZIP 压缩库，复刻 miniz 的 DEFLATE/INFLATE 算法。
//!
//! ## 功能
//!
//! - 创建 ZIP 文件
//! - 追加文件到现有 ZIP
//! - 列出 ZIP 内容
//! - 解压 ZIP 文件
//! - GZIP 压缩/解压
//! - 跨平台支持（Unix/Windows）
//! - 权限保留（Unix）
//!
//! ## 示例
//!
//! ```no_run
//! use zip_rs::zip::ZipBuilder;
//!
//! // 创建 ZIP 文件
//! ZipBuilder::new("archive.zip")
//!     .unwrap()
//!     .files(&["file1.txt", "file2.txt"])
//!     .unwrap()
//!     .build()
//!     .unwrap();
//! ```

pub mod error;
pub mod gzip;
pub mod miniz;
pub mod platform;
pub mod process;
pub mod unzip;
pub mod zip;

// 重导出常用类型
pub use error::{
    CompressionLevel, FileType, Result, ZipEntry, ZipError, ZipErrorCode, ZipMode,
};
pub use gzip::{deflate as gzip_deflate, inflate as gzip_inflate};
pub use miniz::{adler32, crc32};
pub use process::{UnzipProcess, ZipProcess};
pub use zip::append;
pub use zip::{ZipBuildOutput, ZipBuilder};
pub use zip::data::ZipWarning;

// 纯 Rust unzip 模块
pub use unzip::{Extractor, ZipArchive};

// 纯 Rust ZIP writer
pub use zip::writer::ZipWriter;

/// 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 创建 ZIP 文件（便捷函数）
///
/// # 参数
///
/// - `zipfile`: 输出 ZIP 文件路径
/// - `files`: 要添加的文件列表
///
/// # 示例
///
/// ```no_run
/// use zip_rs;
///
/// zip_rs::create("archive.zip", &["file1.txt", "file2.txt"]).unwrap();
/// ```
pub fn create(zipfile: impl AsRef<std::path::Path>, files: &[impl AsRef<str>]) -> crate::error::Result<()> {
    ZipBuilder::new(zipfile)?
        .files(files)?
        .build()?;
    Ok(())
}

/// 解压 ZIP 文件（便捷函数）
///
/// # 参数
///
/// - `zipfile`: ZIP 文件路径
/// - `exdir`: 输出目录
///
/// # 示例
///
/// ```no_run
/// use zip_rs;
///
/// zip_rs::extract("archive.zip", "output").unwrap();
/// ```
pub fn extract(zipfile: impl AsRef<std::path::Path>, exdir: impl AsRef<std::path::Path>) -> crate::error::Result<()> {
    Extractor::new(zipfile)?
        .exdir(exdir)
        .extract()
}

/// 列出 ZIP 内容（便捷函数）
///
/// # 参数
///
/// - `zipfile`: ZIP 文件路径
///
/// # 返回
///
/// 文件条目列表
///
/// # 示例
///
/// ```no_run
/// use zip_rs;
///
/// let entries = zip_rs::list("archive.zip").unwrap();
/// for entry in entries {
///     println!("{}", entry.filename);
/// }
/// ```
pub fn list(zipfile: impl AsRef<std::path::Path>) -> crate::error::Result<Vec<ZipEntry>> {
    ZipArchive::list(zipfile)
}

// GZIP 模块便捷函数
pub mod gzip_func {
    use super::*;

    /// GZIP 压缩
    pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
        gzip::deflate(data)
    }

    /// GZIP 解压
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        gzip::inflate(data)
    }
}

/// 当前平台信息
pub fn current_platform() -> &'static impl platform::Platform {
    platform::current_platform()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_current_platform() {
        let _platform = current_platform();
        // 只验证可以访问平台
        assert!(true);
    }
}
