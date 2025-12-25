use crate::error::{CompressionLevel, Result, ZipError, ZipMode};
use crate::zip::data::{get_zip_data, ZipData, ZipWarning};
use crate::zip::ZipWriter;
use std::fs;
use std::path::{Path, PathBuf};

/// ZIP builder options.
#[derive(Debug, Clone)]
pub struct ZipBuilderOptions {
    pub compression_level: CompressionLevel,
    pub recurse: bool,
    pub include_directories: bool,
    pub root: PathBuf,
    pub mode: ZipMode,
    pub append: bool,
}

impl Default for ZipBuilderOptions {
    fn default() -> Self {
        Self {
            compression_level: CompressionLevel::Level6,
            recurse: true,
            include_directories: true,
            root: PathBuf::from("."),
            mode: ZipMode::Mirror,
            append: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZipBuildOutput {
    pub zipfile: PathBuf,
    pub warnings: Vec<ZipWarning>,
}

pub struct ZipBuilder {
    zipfile: PathBuf,
    options: ZipBuilderOptions,
    files: Vec<String>,
}

impl ZipBuilder {
    pub fn new(zipfile: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            zipfile: zipfile.as_ref().to_path_buf(),
            options: ZipBuilderOptions::default(),
            files: Vec::new(),
        })
    }

    pub fn compression_level(mut self, level: CompressionLevel) -> Self {
        self.options.compression_level = level;
        self
    }

    pub fn recurse(mut self, recurse: bool) -> Self {
        self.options.recurse = recurse;
        self
    }

    pub fn include_directories(mut self, include: bool) -> Self {
        self.options.include_directories = include;
        self
    }

    pub fn root(mut self, root: impl AsRef<Path>) -> Self {
        self.options.root = root.as_ref().to_path_buf();
        self
    }

    pub fn mode(mut self, mode: ZipMode) -> Self {
        self.options.mode = mode;
        self
    }

    pub fn append(mut self, append: bool) -> Self {
        self.options.append = append;
        self
    }

    pub fn files(mut self, files: &[impl AsRef<str>]) -> Result<Self> {
        for file in files {
            self.files.push(file.as_ref().to_string());
        }
        Ok(self)
    }

    pub fn build(self) -> Result<PathBuf> {
        Ok(self.build_with_warnings()?.zipfile)
    }

    pub fn build_with_warnings(self) -> Result<ZipBuildOutput> {
        // 验证 ZIP 文件路径
        if self.zipfile.is_dir() {
            return Err(ZipError::generic("zipfile is a directory"));
        }

        // 追加模式需要 ZIP 文件已存在
        if self.options.append && !self.zipfile.exists() {
            return Err(ZipError::OpenAppendFailed {
                path: self.zipfile.clone(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "zipfile not found"),
            });
        }

        // 获取文件数据（包括递归扫描和警告检测）
        // 注意：不在这里验证文件存在性，让 C 层面的 zip_zip() 来处理
        // 这样可以完全复刻 C 版本的行为：在实际添加文件时打开文件
        let data = get_zip_data(
            &self.files,
            self.options.recurse,
            self.options.mode,
            self.options.include_directories,
            &self.options.root,
        )?;

        // 处理空 ZIP 文件列表
        // 注意：追加模式下，即使没有新文件，也需要保留原有条目
        // 非追加模式下，创建空 ZIP 文件（只有 EOCD 记录）
        if data.entries.is_empty() && !self.options.append {
            self.create_empty_zip()?;
            return Ok(ZipBuildOutput {
                zipfile: self.zipfile,
                warnings: data.warnings,
            });
        }

        // 调用底层 C 函数创建 ZIP
        // C 层面会在实际添加文件时打开文件，如果失败会返回错误
        // 在追加模式下，即使 data.entries 为空，也会保留原有条目
        self.call_zip_zip(&data)?;

        Ok(ZipBuildOutput {
            zipfile: self.zipfile,
            warnings: data.warnings,
        })
    }

    /// 创建空 ZIP 文件（只有 EOCD 记录）
    /// 完全复刻 C 版本：当 num_files = 0 时，循环不执行，直接 finalize archive
    fn create_empty_zip(&self) -> Result<()> {
        use std::io::Write;

        // 创建 ZIP 文件
        let mut file = fs::File::create(&self.zipfile).map_err(|e| {
            ZipError::OpenWriteFailed {
                path: self.zipfile.clone(),
                source: e,
            }
        })?;

        // 空 ZIP 的 EOCD（End of Central Directory）记录（22 字节）
        // 这是 ZIP 格式要求的最小结构
        let eocd = [
            0x50, 0x4b, 0x05, 0x06,  // 签名 (0x06054b50)
            0x00, 0x00,              // 本磁盘编号
            0x00, 0x00,              // 起始磁盘
            0x00, 0x00,              // 本磁盘记录数
            0x00, 0x00,              // 总记录数
            0x00, 0x00, 0x00, 0x00,  // 目录大小
            0x00, 0x00, 0x00, 0x00,  // 目录偏移
            0x00, 0x00,              // 注释长度
        ];

        file.write_all(&eocd).map_err(|e| {
            ZipError::generic(&format!("Failed to write empty ZIP: {}", e))
        })?;

        file.sync_all().map_err(|e| {
            ZipError::generic(&format!("Failed to sync empty ZIP: {}", e))
        })?;

        Ok(())
    }

    fn call_zip_zip(&self, data: &ZipData) -> Result<()> {
        // 对应 C 版本的 zip_zip() 函数（zip.c:319-431）
        // 使用纯 Rust 实现，不调用 FFI

        // 创建 ZIP writer
        // 对应 C 版本：根据 cappend 参数选择初始化方式
        // - cappend == 0: mz_zip_writer_init_cfile() (zip.c:346)
        // - cappend == 1: mz_zip_writer_init_from_reader() (zip.c:339-340)
        let mut zip_writer = if self.options.append {
            ZipWriter::new_with_append(
                &self.zipfile,
                self.options.compression_level,
            )?
        } else {
            ZipWriter::new(
                &self.zipfile,
                self.options.compression_level,
            )?
        };

        // 遍历所有文件并添加到 ZIP
        // 对应 C 版本的循环：for (i = 0; i < n; i++)
        for entry in &data.entries {
            if entry.dir {
                // 添加目录
                // 对应 C 版本：mz_zip_writer_add_mem_ex_v2() (zip.c:364-372)
                zip_writer.add_directory(&entry.key, &entry.file)?;
            } else {
                // 添加文件
                // 对应 C 版本：mz_zip_writer_add_cfile() (zip.c:389-402)
                // 完全复刻 C 版本的错误检测：File::open() 会自动检测文件不存在、权限等错误
                zip_writer.add_file(&entry.key, &entry.file)?;
            }
        }

        // 完成 ZIP 文件写入
        // 对应 C 版本：mz_zip_writer_finalize_archive() + mz_zip_writer_end() (zip.c:413-424)
        zip_writer.finalize()?;

        Ok(())
    }
}
