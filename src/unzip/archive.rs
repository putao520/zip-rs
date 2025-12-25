//! 纯 Rust ZIP Archive 实现
//! 完全复刻 C 版本 ZipArchive 的行为，不使用 FFI

use crate::error::{FileType, Result, ZipEntry, ZipError};
use crate::miniz::inflate;
use crate::zip::reader::{ZipEntryInfo, ZipReader};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// 从 external_attr 提取 Unix 权限
/// 对应 C 版本 zip.c:111-123 的 zip_get_permissions()
///
/// C 版本逻辑：
///   mz_uint32 external_attr = (stat->m_external_attr >> 16) & 0xFFFF;
///   if (version_by != 3 || external_attr == 0) {
///     *mode = stat->m_is_directory ? 0700 : 0600;
///   } else {
///     *mode = (mode_t) external_attr & 0777;
///   }
fn extract_permissions(external_attr: u32, version_made_by: u16, is_dir: bool) -> u32 {
    // 提取高16位（Unix 权限）
    let unix_attr = (external_attr >> 16) & 0xFFFF;

    // 提取 version_made_by 的高字节（创建系统）
    // 3 = Unix
    let version_by = (version_made_by >> 8) & 0xFF;

    // 如果不是 Unix 格式，或者权限字段为0，使用默认值
    if version_by != 3 || unix_attr == 0 {
        // 默认权限：目录 0700，文件 0600
        if is_dir {
            0o700
        } else {
            0o600
        }
    } else {
        // 提取权限位（低9位）
        unix_attr & 0o777
    }
}

/// DOS 时间转换为 SystemTime
/// 对应 C 版本的 mz_zip_dos_to_time_t()
fn dos_to_system_time(dos_time: u16, dos_date: u16) -> std::time::SystemTime {
    use std::time::UNIX_EPOCH;

    // DOS 日期格式：bit 9-15=year, bit 5-8=month, bit 0-4=day
    let year = ((dos_date >> 9) & 0x7F) as i32 + 1980;
    let month = ((dos_date >> 5) & 0x0F) as u8;
    let day = (dos_date & 0x1F) as u8;

    // DOS 时间格式：bit 11-15=hour, bit 5-10=minute, bit 0-4=second/2
    let hour = ((dos_time >> 11) & 0x1F) as u8;
    let minute = ((dos_time >> 5) & 0x3F) as u8;
    let second = ((dos_time << 1) & 0x3E) as u8;

    // 如果日期为 0（未设置或无效），返回 UNIX_EPOCH
    if dos_date == 0 || month == 0 || day == 0 {
        return UNIX_EPOCH;
    }

    // 构造时间
    let datetime = match time::Date::from_calendar_date(year, time::Month::try_from(month).unwrap_or(time::Month::January), day) {
        Ok(date) => {
            match time::Time::from_hms(hour, minute, second) {
                Ok(time) => Some(date.with_time(time).assume_utc()),
                Err(_) => None,
            }
        }
        Err(_) => None,
    };

    match datetime {
        Some(dt) => {
            // time crate 的 OffsetDateTime 转 SystemTime
            let timestamp = dt.unix_timestamp();
            if timestamp >= 0 {
                UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
            } else {
                UNIX_EPOCH - std::time::Duration::from_secs((-timestamp) as u64)
            }
        }
        None => UNIX_EPOCH, // 无效时间返回 epoch
    }
}

/// 纯 Rust ZIP Archive
/// 对应 C 版本使用 FFI 的 ZipArchive
pub struct ZipArchive {
    path: PathBuf,
}

impl ZipArchive {
    /// 打开 ZIP 文件
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    /// 列出 ZIP 文件内容
    /// 对应 C 版本的 zip_list()
    pub fn list(path: impl AsRef<Path>) -> Result<Vec<ZipEntry>> {
        let reader = ZipReader::open(path)?;
        reader.entries().iter().map(|info| {
            // 对应 C 版本 zip.c:111-123 的 zip_get_permissions()
            // 从 external_attr 提取 Unix 权限
            let permissions = extract_permissions(info.external_attr, info.version_made_by, info.is_dir);

            Ok(ZipEntry {
                filename: info.name.clone(),
                compressed_size: info.compressed_size,
                uncompressed_size: info.uncompressed_size,
                crc32: info.crc32,
                offset: info.local_header_offset,
                is_directory: info.is_dir,
                timestamp: dos_to_system_time(info.mtime_dos, info.mdate_dos),
                permissions,
                file_type: if info.is_dir {
                    FileType::Directory
                } else if info.compression_method == 8 {
                    FileType::File
                } else {
                    FileType::File
                },
                is_symlink: false,
            })
        }).collect()
    }

    /// 获取所有条目
    pub fn entries(&self) -> Result<Vec<ZipEntry>> {
        Self::list(&self.path)
    }

    /// 定位文件
    pub fn locate_file(&self, name: &str) -> Result<Option<u32>> {
        let reader = ZipReader::open(&self.path)?;
        for (i, entry) in reader.entries().iter().enumerate() {
            if entry.name == name {
                return Ok(Some(i as u32));
            }
        }
        Ok(None)
    }

    /// 提取单个文件到指定路径
    pub fn extract_to(&self, file_index: u32, output: &Path) -> Result<()> {
        let reader = ZipReader::open(&self.path)?;
        let entries = reader.entries();

        if file_index as usize >= entries.len() {
            return Err(ZipError::CorruptEntry {
                name: format!("index {}", file_index),
                archive: self.path.clone(),
                reason: "file index out of bounds".to_string(),
            });
        }

        let entry = &entries[file_index as usize];

        // 打开 ZIP 文件读取数据
        let file = File::open(&self.path).map_err(|e| ZipError::FileOpen {
            path: self.path.clone(),
            source: e,
        })?;
        let mut reader = BufReader::new(file);

        // 定位到本地文件头
        reader
            .seek(SeekFrom::Start(entry.local_header_offset))
            .map_err(|e| {
                ZipError::generic(&format!("Failed to seek to local header: {:?}", e))
            })?;

        // 读取本地文件头（30 字节）
        let mut local_header = [0u8; 30];
        reader.read_exact(&mut local_header).map_err(|e| {
            ZipError::generic(&format!("Failed to read local header: {:?}", e))
        })?;

        // 验证签名
        let sig = u32::from_le_bytes(local_header[0..4].try_into().unwrap());
        if sig != 0x04034b50 {
            return Err(ZipError::CorruptEntry {
                name: entry.name.clone(),
                archive: self.path.clone(),
                reason: format!("invalid local header signature: 0x{:08x}", sig),
            });
        }

        // 解析字段
        let name_len =
            u16::from_le_bytes(local_header[26..28].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(local_header[28..30].try_into().unwrap()) as usize;
        let compression_method =
            u16::from_le_bytes(local_header[8..10].try_into().unwrap());
        let compressed_size = u32::from_le_bytes(local_header[18..22].try_into().unwrap()) as u64;
        let _uncompressed_size =
            u32::from_le_bytes(local_header[22..26].try_into().unwrap()) as u64;
        let crc32_expected = u32::from_le_bytes(local_header[14..18].try_into().unwrap());

        // 跳过文件名和 extra field
        let skip = name_len + extra_len;
        if skip > 0 {
            let mut skip_buf = vec![0u8; skip];
            reader.read_exact(&mut skip_buf).map_err(|e| {
                ZipError::generic(&format!("Failed to skip filename/extra: {:?}", e))
            })?;
        }

        // 读取压缩数据
        let mut compressed_data = vec![0u8; compressed_size as usize];
        reader
            .read_exact(&mut compressed_data)
            .map_err(|e| ZipError::generic(&format!("Failed to read compressed data: {:?}", e)))?;

        // 解压数据
        let decompressed_data = if compression_method == 8 {
            // DEFLATE 压缩
            // 注意：ZIP 格式的 DEFLATE 不包含 zlib 头尾
            // 使用 parse_zlib_header=false 的 inflate 解码
            inflate::decompress_raw(&compressed_data).map_err(|e| {
                ZipError::CorruptEntry {
                    name: entry.name.clone(),
                    archive: self.path.clone(),
                    reason: format!("decompression failed: {}", e),
                }
            })?
        } else if compression_method == 0 {
            // 无压缩（STORE）
            compressed_data
        } else {
            return Err(ZipError::CorruptEntry {
                name: entry.name.clone(),
                archive: self.path.clone(),
                reason: format!("unsupported compression method: {}", compression_method),
            });
        };

        // 验证 CRC32
        use crate::miniz::crc32::crc32;
        let crc32_actual = crc32(0, &decompressed_data);
        if crc32_actual != crc32_expected {
            return Err(ZipError::CorruptEntry {
                name: entry.name.clone(),
                archive: self.path.clone(),
                reason: format!(
                    "CRC32 mismatch: expected 0x{:08x}, got 0x{:08x}",
                    crc32_expected, crc32_actual
                ),
            });
        }

        // 创建父目录
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ZipError::generic(&format!(
                "Failed to create output directory: {:?}",
                e
            )))?;
        }

        // 检查是否为符号链接
        // 对应 C 版本：attr = file_stat.m_external_attr >> 16; S_ISLNK(attr)
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            const S_IFMT: u32 = 0o170000; // 文件类型掩码
            const S_IFLNK: u32 = 0o120000; // 符号链接文件类型

            let attr = (entry.external_attr >> 16) as u32;
            // 正确的检查：使用 S_IFMT 掩码提取文件类型，然后比较
            if (attr & S_IFMT) == S_IFLNK {
                // 符号链接：解压的数据是目标路径
                let target = String::from_utf8_lossy(&decompressed_data).to_string();
                symlink(&target, output).map_err(|e| ZipError::generic(&format!(
                    "Failed to create symlink '{}' -> '{}': {:?}",
                    output.display(),
                    target,
                    e
                )))?;
                return Ok(());
            }
        }

        // 普通文件：写入输出文件
        let mut output_file =
            std::fs::File::create(output).map_err(|e| ZipError::OpenWriteFailed {
                path: output.to_path_buf(),
                source: e,
            })?;
        output_file.write_all(&decompressed_data).map_err(|e| {
            ZipError::generic(&format!("Failed to write output file: {:?}", e))
        })?;
        output_file.sync_all().map_err(|e| {
            ZipError::generic(&format!("Failed to sync output file: {:?}", e))
        })?;

        Ok(())
    }
}
