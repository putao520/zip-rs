//! 纯 Rust ZIP Writer 实现
//! 完全复刻 C 版本 zip.c 和 miniz.c 的行为

use crate::error::{CompressionLevel, Result, ZipError};
use crate::miniz::deflate::compress_raw;
use crate::miniz::crc32::crc32;
use crate::zip::reader::ZipReader;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// ZIP 文件条目信息（对应中央目录）
#[derive(Debug, Clone)]
struct ZipEntry {
    /// 文件名（在 ZIP 内）
    name: String,
    /// 压缩前大小
    uncompressed_size: u64,
    /// 压缩后大小
    compressed_size: u64,
    /// CRC32 校验和
    crc32: u32,
    /// 本地文件头偏移量
    local_header_offset: u64,
    /// 是否为目录
    is_dir: bool,
    /// 修改时间（DOS 时间格式）
    mtime_dos: u16,
    /// 修改日期（DOS 日期格式）
    mdate_dos: u16,
    /// Unix 权限（如果适用）
    external_attr: u32,
}

/// 纯 Rust ZIP Writer
/// 对应 C 版本的 mz_zip_archive + zip_zip() 逻辑
pub struct ZipWriter {
    /// ZIP 文件路径
    path: PathBuf,
    /// BufWriter 用于高效写入
    writer: BufWriter<File>,
    /// 所有已添加的条目（用于写入中央目录）
    entries: Vec<ZipEntry>,
    /// 是否已 finalized
    finalized: bool,
    /// 压缩级别
    compression_level: CompressionLevel,
}

/// ZIP 文件格式常量（对应 miniz.c:3061-3149）
mod zip_format {
    // 签名
    pub const LOCAL_DIR_HEADER_SIG: u32 = 0x04034b50;
    pub const CENTRAL_DIR_HEADER_SIG: u32 = 0x02014b50;
    pub const END_OF_CENTRAL_DIR_SIG: u32 = 0x06054b50;

    // 头大小
    pub const LOCAL_DIR_HEADER_SIZE: u16 = 30;
    pub const CENTRAL_DIR_HEADER_SIZE: u16 = 46;
    pub const END_OF_CENTRAL_DIR_SIZE: u16 = 22;

    // 版本
    pub const VERSION_NEEDED: u16 = 20; // 2.0（兼容大多数工具）
    pub const VERSION_MADE_BY: u16 = 0x0317; // Unix (3) + 2.3 (23)

    // 压缩方法
    pub const METHOD_STORE: u16 = 0; // 无压缩
    pub const METHOD_DEFLATE: u16 = 8; // DEFLATE 压缩

    // DOS 目录属性标志
    pub const DOS_DIR_ATTR: u32 = 0x10;
}

/// 从文件 metadata 计算 external_attr
/// 对应 C 版本 zip.c:93-94 的权限处理
///
/// C 版本逻辑：
///   external_attr &= 0x0000FFFF;
///   external_attr |= (st.st_mode & 0777) << 16;
///
/// Unix 权限存储在 external_attr 的高16位（bit 16-31）
/// 低16位保留给 DOS 属性
fn compute_external_attr(metadata: &Metadata, is_dir: bool) -> u32 {
    // 默认值（Windows 或无法获取权限时）
    let default_attr = if is_dir {
        zip_format::DOS_DIR_ATTR
    } else {
        0
    };

    #[cfg(unix)]
    {
        // 提取 Unix 权限（st.st_mode & 0777）
        let mode = metadata.permissions().mode() & 0o777;

        // 高16位存储 Unix 权限，低16位保留 DOS 属性
        let mut external_attr: u32 = (mode as u32) << 16;

        // 设置 DOS 目录属性（如果需要）
        if is_dir {
            external_attr |= zip_format::DOS_DIR_ATTR;
        }

        external_attr
    }

    #[cfg(not(unix))]
    {
        // 非 Unix 系统使用默认值
        default_attr
    }
}

impl ZipWriter {
    /// 验证ZIP文件名
    /// 对应 C 版本的 mz_zip_writer_validate_archive_name()
    ///
    /// 规则：
    /// 1. 不能以 '/' 开头（绝对路径）
    /// 2. 不能包含反斜杠 '\'（DOS风格路径分隔符）
    fn validate_archive_name(name: &str) -> Result<()> {
        // 规则1: 不能以 '/' 开头
        if name.starts_with('/') {
            return Err(ZipError::generic(&format!(
                "Invalid filename: cannot start with '/': '{}'",
                name
            )));
        }

        // 规则2: 不能包含反斜杠
        if name.contains('\\') {
            return Err(ZipError::generic(&format!(
                "Invalid filename: cannot contain backslash: '{}'",
                name
            )));
        }

        Ok(())
    }

    /// 创建新的 ZIP writer
    /// 对应 C 版本的 mz_zip_writer_init_cfile()
    pub fn new(path: impl AsRef<Path>, compression_level: CompressionLevel) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // 打开文件进行写入
        let file = File::create(&path).map_err(|e| ZipError::OpenWriteFailed {
            path: path.clone(),
            source: e,
        })?;

        Ok(Self {
            path,
            writer: BufWriter::new(file),
            entries: Vec::new(),
            finalized: false,
            compression_level,
        })
    }

    /// 创建追加模式的 ZIP writer
    /// 对应 C 版本的 mz_zip_writer_init_from_reader()
    ///
    /// C 版本流程 (zip.c:332-344):
    /// 1. mz_zip_reader_init_cfile() - 读取现有 ZIP
    /// 2. mz_zip_writer_init_from_reader() - 从 reader 初始化 writer
    pub fn new_with_append(
        path: impl AsRef<Path>,
        compression_level: CompressionLevel,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // 1. 读取现有 ZIP 文件的中央目录
        // 对应 C 版本：mz_zip_reader_init_cfile()
        let reader = ZipReader::open(&path).map_err(|e| ZipError::OpenAppendFailed {
            path: path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;

        // 2. 获取追加位置（中央目录之前的偏移）
        // 对应 C 版本：writer 从已有数据之后继续
        let append_offset = reader.get_append_offset();

        // 3. 转换 ZipEntryInfo 到内部 ZipEntry 格式
        let existing_entries: Vec<ZipEntry> = reader.entries().iter().map(|info| ZipEntry {
            name: info.name.clone(),
            uncompressed_size: info.uncompressed_size,
            compressed_size: info.compressed_size,
            crc32: info.crc32,
            local_header_offset: info.local_header_offset,
            is_dir: info.is_dir,
            mtime_dos: 0, // 时间信息不保存，重新读取时为 0
            mdate_dos: 0,
            external_attr: info.external_attr,
        }).collect();

        // 4. 打开文件进行追加（不截断）
        // 对应 C 版本：以追加模式打开文件
        let file = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| ZipError::OpenAppendFailed {
                path: path.clone(),
                source: e,
            })?;

        let mut writer = BufWriter::new(file);

        // 5. 定位到追加位置（中央目录之前）
        // 对应 C 版本：覆盖旧的中央目录和 EOCD
        writer.seek(SeekFrom::Start(append_offset)).map_err(|e| {
            ZipError::generic(&format!("Failed to seek to append position: {:?}", e))
        })?;

        Ok(Self {
            path,
            writer,
            entries: existing_entries,  // ✅ 保留已有条目
            finalized: false,
            compression_level,
        })
    }

    /// 添加一个文件到 ZIP
    /// 完全复刻 C 版本 zip.c:374-402 的逻辑
    ///
    /// C 代码流程：
    /// 1. fopen(filename, "r") - 打开源文件
    /// 2. zip_file_size() - 获取文件大小
    /// 3. mz_zip_writer_add_cfile() - 添加到 ZIP
    /// 4. fclose() - 关闭源文件
    pub fn add_file(&mut self, name: &str, source_path: &Path) -> Result<()> {
        // 对应 C 版本：mz_zip_writer_validate_archive_name() (miniz.c:6349)
        // 验证文件名：不能以/开头，不能包含反斜杠
        Self::validate_archive_name(name)?;

        // 对应 C 版本：FILE* fh = zip_open_utf8(filename, ZIP__READ, ...)
        // 完全复刻 C 版本的错误检测：在实际打开文件时检测
        let mut source_file = File::open(source_path).map_err(|e| {
            // 对应 C 版本：if (fh == NULL) ZIP_ERROR(R_ZIP_EADDFILE, key, czipfile)
            ZipError::FileOpen {
                path: source_path.to_path_buf(),
                source: e,
            }
        })?;

        // 对应 C 版本：zip_file_size(fh, &uncomp_size)
        let metadata = source_file.metadata().map_err(|e| ZipError::FileSizeFailed {
            path: source_path.to_path_buf(),
        })?;
        let uncompressed_size = metadata.len();

        // 读取文件内容
        let mut buffer = Vec::with_capacity(uncompressed_size as usize);
        std::io::copy(&mut source_file, &mut buffer).map_err(|e| ZipError::generic(&format!(
            "Failed to read file {}: {:?}",
            source_path.display(),
            e
        )))?;

        // 计算 CRC32（初始值为 0）
        let crc = crc32(0, &buffer);

        // 获取修改时间
        let mtime = metadata.modified().ok();
        let (mtime_dos, mdate_dos) = system_time_to_dos(mtime);

        // 压缩数据（如果需要）
        // 对应 C 版本：mz_zip_writer_add_cfile() 内部的压缩逻辑
        // 注意：C 版本中 compression_level = 0 表示无压缩（STORE）
        let (compressed_data, method) = match self.compression_level {
            CompressionLevel::NoCompression => {
                // NoCompression = 0: 直接存储，不压缩（对应 C 版本的 STORE 模式）
                (buffer.clone(), zip_format::METHOD_STORE)  // compression_method = 0
            }
            CompressionLevel::Level1 => {
                // 使用纯 DEFLATE 压缩（不带 ZLIB 头，ZIP 格式要求）
                // 对应 miniz.c 的 tdefl_compress()
                let compressed = compress_raw(&buffer, 1).map_err(|e| {
                    ZipError::generic(&format!("Compression failed: {:?}", e))
                })?;
                // 始终使用 DEFLATE 方法（compression_method=8）
                // 如果压缩后没有变小，使用 uncompressed block（BTYPE=00）
                // 这对应 C 版本 miniz 的行为
                let final_data = if compressed.len() >= buffer.len() {
                    // 创建 uncompressed block（BTYPE=00）
                    // 格式：[BFINAL+BTYPE] [LEN] [NLEN] [DATA]
                    let mut uncompressed_block = Vec::with_capacity(5 + buffer.len());
                    uncompressed_block.push(0x01); // BFINAL=1, BTYPE=00
                    let len = buffer.len() as u16;
                    uncompressed_block.push(len as u8);
                    uncompressed_block.push((len >> 8) as u8);
                    uncompressed_block.push((!len) as u8);
                    uncompressed_block.push((!len >> 8) as u8);
                    uncompressed_block.extend_from_slice(&buffer);
                    uncompressed_block
                } else {
                    compressed
                };
                (final_data, zip_format::METHOD_DEFLATE)  // compression_method = 8
            }
            _ => {
                let compressed = compress_raw(&buffer, self.compression_level.as_u8() as i32).map_err(|e| {
                    ZipError::generic(&format!("Compression failed: {:?}", e))
                })?;
                // 同样的逻辑：如果压缩无效，使用 uncompressed block
                let final_data = if compressed.len() >= buffer.len() {
                    let mut uncompressed_block = Vec::with_capacity(5 + buffer.len());
                    uncompressed_block.push(0x01);
                    let len = buffer.len() as u16;
                    uncompressed_block.push(len as u8);
                    uncompressed_block.push((len >> 8) as u8);
                    uncompressed_block.push((!len) as u8);
                    uncompressed_block.push((!len >> 8) as u8);
                    uncompressed_block.extend_from_slice(&buffer);
                    uncompressed_block
                } else {
                    compressed
                };
                (final_data, zip_format::METHOD_DEFLATE)
            }
        };

        let compressed_size = compressed_data.len() as u64;

        // 记录当前偏移量（用于中央目录）
        let local_header_offset = self.stream_position()?;

        // 写入本地文件头
        self.write_local_file_header(
            name,
            uncompressed_size,
            compressed_data.len() as u64,
            crc,
            method,
            mtime_dos,
            mdate_dos,
        )?;

        // 写入文件名
        self.write_all(name.as_bytes())?;

        // 写入压缩/原始数据
        self.write_all(&compressed_data)?;

        // 保存条目信息（用于中央目录）
        // 对应 C 版本 zip.c:93-94 的权限处理
        // external_attr 高16位存储 Unix 权限 (st.st_mode & 0777) << 16
        let external_attr = compute_external_attr(&metadata, false);

        self.entries.push(ZipEntry {
            name: name.to_string(),
            uncompressed_size,
            compressed_size: compressed_data.len() as u64,
            crc32: crc,
            local_header_offset,
            is_dir: false,
            mtime_dos,
            mdate_dos,
            external_attr,
        });

        Ok(())
    }

    /// 添加一个目录到 ZIP
    /// 对应 C 版本 zip.c:364-372: mz_zip_writer_add_mem_ex_v2()
    pub fn add_directory(&mut self, name: &str, dir_path: &Path) -> Result<()> {
        // 对应 C 版本：mz_zip_writer_validate_archive_name() (miniz.c:6349)
        // 验证文件名：不能以/开头，不能包含反斜杠
        Self::validate_archive_name(name)?;

        // 确保目录名以 / 结尾
        let dir_name = if name.ends_with('/') {
            name.to_string()
        } else {
            format!("{}/", name)
        };

        // 读取目录元数据以获取权限和时间
        // 如果目录不存在，使用当前目录的元数据作为后备
        let metadata = std::fs::metadata(dir_path).or_else(|_| std::fs::metadata("."));

        // 获取修改时间（如果元数据可用）
        let mtime = metadata.as_ref().ok().and_then(|m| m.modified().ok());
        let (mtime_dos, mdate_dos) = system_time_to_dos(mtime);

        // 计算 external_attr（如果元数据可用，使用默认值）
        let external_attr = if let Ok(meta) = metadata {
            compute_external_attr(&meta, true)
        } else {
            // 无法读取元数据时使用默认值
            zip_format::DOS_DIR_ATTR
        };

        // 记录偏移量
        let local_header_offset = self.stream_position()?;

        // 写入本地文件头（目录无数据）
        self.write_local_file_header(
            &dir_name,
            0, // uncompressed_size
            0, // compressed_size
            0, // crc32
            zip_format::METHOD_STORE,
            mtime_dos,
            mdate_dos,
        )?;

        // 写入文件名
        self.write_all(dir_name.as_bytes())?;

        // 保存条目 - 使用 compute_external_attr 读取实际权限
        // 对应 C 版本 zip.c:93-94 的权限处理
        // external_attr 高16位存储 Unix 权限 (st.st_mode & 0777) << 16
        self.entries.push(ZipEntry {
            name: dir_name,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            local_header_offset,
            is_dir: true,
            mtime_dos,
            mdate_dos,
            external_attr,
        });

        Ok(())
    }

    /// 完成 ZIP 文件写入
    /// 对应 C 版本 zip.c:413-424: mz_zip_writer_finalize_archive() + mz_zip_writer_end()
    pub fn finalize(&mut self) -> Result<()> {
        if self.finalized {
            return Ok(());
        }

        // 对应 C 版本：mz_zip_writer_finalize_archive()
        // 写入中央目录
        let central_dir_offset = self.stream_position()?;
        self.write_central_directory()?;

        // 对应 C 版本：写入 EOCD
        let central_dir_size = self.stream_position()? - central_dir_offset;
        self.write_end_of_central_directory(central_dir_offset, central_dir_size)?;

        // 刷新缓冲区
        self.writer.flush().map_err(|e| ZipError::generic(&format!(
            "Failed to flush ZIP file: {:?}",
            e
        )))?;

        self.finalized = true;
        Ok(())
    }

    /// 写入本地文件头
    /// 对应 miniz.c 的本地文件头格式
    fn write_local_file_header(
        &mut self,
        name: &str,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: u32,
        method: u16,
        mtime_dos: u16,
        mdate_dos: u16,
    ) -> Result<()> {
        let name_len = name.len() as u16;

        // 构建本地文件头（30 字节）
        // 对应 miniz.c:3101-3113
        let mut header = [0u8; 30];

        // 签名 (0x04034b50)
        header[0..4].copy_from_slice(&zip_format::LOCAL_DIR_HEADER_SIG.to_le_bytes());

        // 版本需要
        header[4..6].copy_from_slice(&zip_format::VERSION_NEEDED.to_le_bytes());

        // 位标志
        header[6..8].copy_from_slice(&0u16.to_le_bytes());

        // 压缩方法
        header[8..10].copy_from_slice(&method.to_le_bytes());

        // 文件时间/日期 (DOS 格式)
        header[10..12].copy_from_slice(&mtime_dos.to_le_bytes());
        header[12..14].copy_from_slice(&mdate_dos.to_le_bytes());

        // CRC32
        header[14..18].copy_from_slice(&crc32.to_le_bytes());

        // 压缩后大小
        header[18..22].copy_from_slice(&(compressed_size as u32).to_le_bytes());

        // 压缩前大小
        header[22..26].copy_from_slice(&(uncompressed_size as u32).to_le_bytes());

        // 文件名长度
        header[26..28].copy_from_slice(&name_len.to_le_bytes());

        // Extra field 长度
        header[28..30].copy_from_slice(&0u16.to_le_bytes());

        self.write_all(&header)?;

        Ok(())
    }

    /// 写入中央目录
    /// 对应 miniz.c:3083-3100
    fn write_central_directory(&mut self) -> Result<()> {
        // 先准备所有中央目录数据，避免借用冲突
        let mut central_dir_data = Vec::new();

        for entry in &self.entries {
            // 中央目录头（46 字节）
            let mut header = [0u8; 46];

            // 签名 (0x02014b50)
            header[0..4].copy_from_slice(&zip_format::CENTRAL_DIR_HEADER_SIG.to_le_bytes());

            // Version made by
            header[4..6].copy_from_slice(&zip_format::VERSION_MADE_BY.to_le_bytes());

            // Version needed
            header[6..8].copy_from_slice(&zip_format::VERSION_NEEDED.to_le_bytes());

            // Bit flag
            header[8..10].copy_from_slice(&0u16.to_le_bytes());

            // Compression method
            let method = if entry.is_dir || entry.compressed_size == entry.uncompressed_size {
                zip_format::METHOD_STORE
            } else {
                zip_format::METHOD_DEFLATE
            };
            header[10..12].copy_from_slice(&method.to_le_bytes());

            // File time/date
            header[12..14].copy_from_slice(&entry.mtime_dos.to_le_bytes());
            header[14..16].copy_from_slice(&entry.mdate_dos.to_le_bytes());

            // CRC32
            header[16..20].copy_from_slice(&entry.crc32.to_le_bytes());

            // Compressed size
            header[20..24].copy_from_slice(&(entry.compressed_size as u32).to_le_bytes());

            // Uncompressed size
            header[24..28].copy_from_slice(&(entry.uncompressed_size as u32).to_le_bytes());

            // Filename length
            let name_len = entry.name.len() as u16;
            header[28..30].copy_from_slice(&name_len.to_le_bytes());

            // Extra field length
            header[30..32].copy_from_slice(&0u16.to_le_bytes());

            // File comment length
            header[32..34].copy_from_slice(&0u16.to_le_bytes());

            // Disk number start
            header[34..36].copy_from_slice(&0u16.to_le_bytes());

            // Internal attributes
            header[36..38].copy_from_slice(&0u16.to_le_bytes());

            // External attributes
            header[38..42].copy_from_slice(&entry.external_attr.to_le_bytes());

            // Local header offset
            header[42..46].copy_from_slice(&(entry.local_header_offset as u32).to_le_bytes());

            central_dir_data.extend_from_slice(&header);
            central_dir_data.extend_from_slice(entry.name.as_bytes());
        }

        // 一次性写入所有中央目录数据
        self.write_all(&central_dir_data)?;

        Ok(())
    }

    /// 写入 EOCD (End of Central Directory)
    /// 对应 miniz.c:3115-3123
    fn write_end_of_central_directory(
        &mut self,
        central_dir_offset: u64,
        central_dir_size: u64,
    ) -> Result<()> {
        let num_entries = self.entries.len() as u16;

        let mut eocd = [0u8; 22];

        // Signature (0x06054b50)
        eocd[0..4].copy_from_slice(&zip_format::END_OF_CENTRAL_DIR_SIG.to_le_bytes());

        // Disk number
        eocd[4..6].copy_from_slice(&0u16.to_le_bytes());

        // Central dir disk
        eocd[6..8].copy_from_slice(&0u16.to_le_bytes());

        // Entries on this disk
        eocd[8..10].copy_from_slice(&num_entries.to_le_bytes());

        // Total entries
        eocd[10..12].copy_from_slice(&num_entries.to_le_bytes());

        // Central dir size
        eocd[12..16].copy_from_slice(&(central_dir_size as u32).to_le_bytes());

        // Central dir offset
        eocd[16..20].copy_from_slice(&(central_dir_offset as u32).to_le_bytes());

        // Comment length
        eocd[20..22].copy_from_slice(&0u16.to_le_bytes());

        self.write_all(&eocd)?;

        Ok(())
    }

    /// 获取当前写入位置
    fn stream_position(&mut self) -> Result<u64> {
        self.writer.stream_position().map_err(|e| {
            ZipError::generic(&format!("Failed to get stream position: {:?}", e))
        })
    }

    /// 写入字节
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).map_err(|e| {
            ZipError::generic(&format!("Failed to write to ZIP file: {:?}", e))
        })
    }
}

/// 转换 SystemTime 到 DOS 时间/日期格式
/// 对应 C 版本的 mz_zip_time_t_to_dos_time() (miniz.c:3278-3292)
///
/// C 版本使用 localtime() 将 time_t 转换为 tm 结构，然后提取字段：
/// - tm_year: 年份（自 1900 年起）
/// - tm_mon: 月份（0-11）
/// - tm_mday: 日（1-31）
/// - tm_hour: 小时（0-23）
/// - tm_min: 分钟（0-59）
/// - tm_sec: 秒（0-60）
///
/// DOS 格式：
/// - 时间: HHHHHHHHMMMMMMSSSS (5+6+5 = 16 bits)
/// - 日期: YYYYYYYMMMMDDDDD (7+4+5 = 16 bits)
fn system_time_to_dos(time: Option<SystemTime>) -> (u16, u16) {
    use std::time::UNIX_EPOCH;
    use time::OffsetDateTime;

    let duration = time
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();

    let secs = duration.as_secs();

    // 使用 time crate 将 Unix 时间戳转换为本地时间的分解时间
    // 对应 C 版本的 localtime()
    let datetime = match OffsetDateTime::from_unix_timestamp(secs as i64) {
        Ok(dt) => dt,
        Err(_) => return (0, 0),
    };

    // 提取各个时间字段（对应 C 版本的 tm 结构）
    let year = datetime.year() as u16;      // 完整年份（如 2020）
    let month = datetime.month() as u16;    // 月份（1-12）
    let day = datetime.day() as u16;        // 日（1-31）
    let hour = datetime.hour() as u16;      // 小时（0-23）
    let minute = datetime.minute() as u16;  // 分钟（0-59）
    let second = datetime.second() as u16;  // 秒（0-60）

    // DOS 时间格式: HHHHHHHHMMMMMMSSSS (5+6+5 = 16 bits)
    // 对应 C: (tm->tm_hour << 11) + ((tm->tm_min) << 5) + ((tm->tm_sec) >> 1)
    let dos_time = (hour << 11) | (minute << 5) | (second >> 1);

    // DOS 日期格式: YYYYYYYMMMMDDDDD (7+4+5 = 16 bits)
    // 对应 C: ((tm->tm_year + 1900 - 1980) << 9) + ((tm->tm_mon + 1) << 5) + tm->tm_mday
    // year - 1980 是因为 DOS 年份从 1980 年开始（值为 0-127）
    let dos_date = ((year.saturating_sub(1980)) << 9) | (month << 5) | day;

    (dos_time, dos_date)
}

impl Drop for ZipWriter {
    fn drop(&mut self) {
        if !self.finalized {
            // 尝试 finalize，但不 panic
            let _ = self.finalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_system_time_to_dos() {
        // 测试时间转换
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1577836800); // 2020-01-01 00:00:00 UTC
        let (dos_time, dos_date) = system_time_to_dos(Some(time));

        // DOS 时间应该接近午夜
        assert!(dos_time < 100); // 小时/分/秒

        // DOS 日期应该表示 2020-01-01
        // 2020 = (2020 - 1980) = 40 years
        // 格式: YYYYYYYMMMMDDDDD
        let year = (dos_date >> 9) & 0x7F;
        let month = (dos_date >> 5) & 0x0F;
        let day = dos_date & 0x1F;

        assert_eq!(year, 40);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }
}
