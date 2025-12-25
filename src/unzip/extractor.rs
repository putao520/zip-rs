//! 纯 Rust ZIP Extractor 实现
//! 完全复刻 C 版本 Extractor 的行为，不使用 FFI

use crate::error::{Result, ZipError};
use crate::unzip::archive::ZipArchive;
use std::fs;
use std::path::{Path, PathBuf};

/// 提取选项
#[derive(Debug, Clone)]
pub struct ExtractorOptions {
    pub overwrite: bool,
    pub junk_paths: bool,
    pub exdir: PathBuf,
    pub files: Option<Vec<String>>,
}

impl Default for ExtractorOptions {
    fn default() -> Self {
        Self {
            overwrite: true,
            junk_paths: false,
            exdir: PathBuf::from("."),
            files: None,
        }
    }
}

/// 纯 Rust ZIP Extractor
/// 对应 C 版本使用 FFI 的 Extractor
pub struct Extractor {
    zipfile: PathBuf,
    options: ExtractorOptions,
}

impl Extractor {
    pub fn new(zipfile: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            zipfile: zipfile.as_ref().to_path_buf(),
            options: ExtractorOptions::default(),
        })
    }

    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.options.overwrite = overwrite;
        self
    }

    pub fn junk_paths(mut self, junk_paths: bool) -> Self {
        self.options.junk_paths = junk_paths;
        self
    }

    pub fn exdir(mut self, exdir: impl AsRef<Path>) -> Self {
        self.options.exdir = exdir.as_ref().to_path_buf();
        self
    }

    pub fn files(mut self, files: &[impl AsRef<str>]) -> Self {
        self.options.files = Some(
            files
                .iter()
                .map(|f| f.as_ref().to_string())
                .collect(),
        );
        self
    }

    /// 执行提取
    pub fn extract(self) -> Result<()> {
        // 打开 ZIP 文件
        let archive = ZipArchive::open(&self.zipfile)?;

        // 获取所有条目
        let all_entries = archive.entries()?;

        // 过滤出要提取的文件
        let entries_to_extract: Vec<_> = if let Some(ref files) = self.options.files {
            // 只提取指定的文件
            all_entries
                .into_iter()
                .filter(|entry| {
                    files.iter().any(|f| entry.filename == *f || entry.filename.contains(f))
                })
                .collect()
        } else {
            // 提取所有文件
            all_entries
        };

        // 创建输出目录
        fs::create_dir_all(&self.options.exdir).map_err(|e| {
            ZipError::generic(&format!("Failed to create extract directory: {:?}", e))
        })?;

        // 提取每个文件
        for entry in entries_to_extract {
            // 计算输出路径
            let output_path = if self.options.junk_paths {
                // 丢弃路径，只使用文件名
                let filename = PathBuf::from(&entry.filename)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| entry.filename.clone());
                self.options.exdir.join(filename)
            } else {
                // 保留完整路径
                self.options.exdir.join(&entry.filename)
            };

            // 如果是目录，创建目录
            if entry.is_directory {
                fs::create_dir_all(&output_path).map_err(|e| {
                    ZipError::generic(&format!(
                        "Failed to create directory {}: {:?}",
                        output_path.display(),
                        e
                    ))
                })?;
                continue;
            }

            // 检查文件是否已存在
            if output_path.exists() && !self.options.overwrite {
                continue;
            }

            // 提取文件
            // 注意：这里需要找到文件在 ZIP 中的索引
            // 暂时通过 locate_file 实现
            if let Some(index) = archive.locate_file(&entry.filename)? {
                archive.extract_to(index, &output_path)?;
            }
        }

        Ok(())
    }
}
