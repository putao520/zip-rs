//! 纯 Rust ZIP Reader 实现
//! 完全复刻 C 版本 mz_zip_reader 的行为

use crate::error::{Result, ZipError};
use crate::miniz::crc32::crc32;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// DOS 时间转换为 SystemTime
/// 对应 C 版本的 mz_zip_dos_to_time_t()
fn dos_to_system_time(dos_time: u16, dos_date: u16) -> std::time::SystemTime {
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

/// ZIP 文件条目信息（从中央目录读取）
#[derive(Debug, Clone)]
pub struct ZipEntryInfo {
    /// 文件名
    pub name: String,
    /// 压缩前大小
    pub uncompressed_size: u64,
    /// 压缩后大小
    pub compressed_size: u64,
    /// CRC32 校验和
    pub crc32: u32,
    /// 本地文件头偏移量
    pub local_header_offset: u64,
    /// 是否为目录
    pub is_dir: bool,
    /// 压缩方法 (0=store, 8=deflate)
    pub compression_method: u16,
    /// 外部属性（包含权限）
    pub external_attr: u32,
    /// 版本创建者（用于判断是否为 Unix 格式）
    pub version_made_by: u16,
    /// 修改时间（DOS 时间格式）
    pub mtime_dos: u16,
    /// 修改日期（DOS 日期格式）
    pub mdate_dos: u16,
}

/// EOCD (End of Central Directory) 信息
#[derive(Debug, Clone)]
struct EocdRecord {
    /// 中央目录偏移量
    central_dir_offset: u64,
    /// 中央目录大小
    central_dir_size: u64,
    /// 总记录数
    total_entries: u16,
}

/// ZIP 常量（对应 miniz.c）
mod zip_format {
    // 签名
    pub const LOCAL_DIR_HEADER_SIG: u32 = 0x04034b50;
    pub const CENTRAL_DIR_HEADER_SIG: u32 = 0x02014b50;
    pub const END_OF_CENTRAL_DIR_SIG: u32 = 0x06054b50;

    // EOCD 最大注释长度
    pub const MAX_EOCD_COMMENT_LEN: u16 = 65535;

    // EOCD 最大搜索长度
    pub const MAX_EOCD_SEARCH_LEN: usize = 65557 + 22; // comment + signature
}

/// 纯 Rust ZIP Reader
/// 对应 C 版本的 mz_zip_reader
pub struct ZipReader {
    /// ZIP 文件路径
    path: std::path::PathBuf,
    /// 所有文件条目
    entries: Vec<ZipEntryInfo>,
    /// 中央目录偏移量
    central_dir_offset: u64,
}

impl ZipReader {
    /// 打开 ZIP 文件并读取中央目录
    /// 对应 C 版本的 mz_zip_reader_init_file()
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // 打开文件
        let file = File::open(&path).map_err(|e| ZipError::FileOpen {
            path: path.clone(),
            source: e,
        })?;

        let mut reader = BufReader::new(file);

        // 查找并解析 EOCD
        let eocd = Self::find_and_parse_eocd(&mut reader)?;

        // 解析中央目录
        let entries = Self::parse_central_directory(&mut reader, &eocd)?;

        Ok(Self {
            path,
            entries,
            central_dir_offset: eocd.central_dir_offset,
        })
    }

    /// 获取所有文件条目
    pub fn entries(&self) -> &[ZipEntryInfo] {
        &self.entries
    }

    /// 查找并解析 EOCD 记录
    /// 对应 C 版本 mz_zip_reader_locate_header_sig() 的逻辑
    ///
    /// 关键修复：必须找到最接近文件末尾的有效 EOCD，而不是第一个匹配
    /// 因为 EOCD 签名可能出现在文件数据中（如数据描述符）
    fn find_and_parse_eocd<R: Read + Seek>(reader: &mut R) -> Result<EocdRecord> {
        const RECORD_SIZE: u64 = 22; // EOCD 记录大小
        const MAX_SCAN_SIZE: u64 = 65535 + RECORD_SIZE; // 最大注释长度 + 记录大小
        const BUF_SIZE: usize = 4096; // 每次读取的缓冲区大小（对应 C 版本的 buf_u32）

        // 获取文件大小
        let file_size = reader.seek(SeekFrom::End(0))?;

        if file_size < RECORD_SIZE {
            return Err(ZipError::generic("File too small to be a ZIP archive"));
        }

        // 从文件末尾开始搜索 EOCD 签名
        // 对应 C 版本：cur_file_ofs = MZ_MAX((mz_int64)pZip->m_archive_size - (mz_int64)sizeof(buf_u32), 0);
        let mut cur_file_ofs = if file_size > BUF_SIZE as u64 {
            file_size - BUF_SIZE as u64
        } else {
            0
        };

        let record_sig = zip_format::END_OF_CENTRAL_DIR_SIG;
        let mut best_eocd_offset: Option<u64> = None;

        loop {
            // 计算本次要读取的大小
            // 对应 C 版本：n = (int)MZ_MIN(sizeof(buf_u32), pZip->m_archive_size - cur_file_ofs);
            let n = std::cmp::min(BUF_SIZE as u64, file_size - cur_file_ofs) as usize;

            let mut buffer = vec![0u8; n];
            reader.seek(SeekFrom::Start(cur_file_ofs))?;
            reader.read_exact(&mut buffer)?;

            // 从后向前在缓冲区中搜索签名
            // 对应 C 版本：for (i = n - 4; i >= 0; --i)
            for i in (0..=n.saturating_sub(4)).rev() {
                let sig = u32::from_le_bytes(buffer[i..i+4].try_into().unwrap());

                if sig == record_sig {
                    // 关键验证：从找到的位置到文件末尾必须至少有 RECORD_SIZE 字节
                    // 对应 C 版本：if ((pZip->m_archive_size - (cur_file_ofs + i)) >= record_size)
                    let eocd_offset = cur_file_ofs + i as u64;
                    let bytes_from_sig_to_end = file_size - eocd_offset;

                    if bytes_from_sig_to_end >= RECORD_SIZE {
                        // 找到一个候选 EOCD
                        // 保留最接近文件末尾的（偏移量最大的）
                        match best_eocd_offset {
                            None => best_eocd_offset = Some(eocd_offset),
                            Some(best) if eocd_offset > best => best_eocd_offset = Some(eocd_offset),
                            _ => {} // 保留当前的 best
                        }
                    }
                }
            }

            // 继续向前搜索
            // 对应 C 版本：cur_file_ofs = MZ_MAX(cur_file_ofs - (sizeof(buf_u32) - 3), 0);
            if cur_file_ofs == 0 {
                break;
            }

            // 检查是否已经搜索了足够远的距离
            // 对应 C 版本：if ((pZip->m_archive_size - cur_file_ofs) >= (MZ_UINT16_MAX + record_size))
            let searched_distance = file_size - cur_file_ofs;
            if searched_distance >= MAX_SCAN_SIZE {
                break;
            }

            cur_file_ofs = if cur_file_ofs > (BUF_SIZE - 3) as u64 {
                cur_file_ofs - (BUF_SIZE - 3) as u64
            } else {
                0
            };
        }

        // 返回找到的最接近文件末尾的有效 EOCD
        match best_eocd_offset {
            Some(offset) => Self::parse_eocd_at(reader, offset, file_size),
            None => Err(ZipError::generic("Cannot find end of central directory")),
        }
    }

    /// 在指定偏移量解析 EOCD 记录
    fn parse_eocd_at<R: Read + Seek>(reader: &mut R, offset: u64, file_size: u64) -> Result<EocdRecord> {
        reader.seek(SeekFrom::Start(offset))?;

        let mut eocd_data = [0u8; 22];
        reader.read_exact(&mut eocd_data)?;

        // 验证签名
        let sig = u32::from_le_bytes(eocd_data[0..4].try_into().unwrap());
        if sig != zip_format::END_OF_CENTRAL_DIR_SIG {
            return Err(ZipError::generic(&format!(
                "Invalid EOCD signature: expected 0x{:08x}, got 0x{:08x}",
                zip_format::END_OF_CENTRAL_DIR_SIG, sig
            )));
        }

        // 解析 EOCD 字段
        let disk_num = u16::from_le_bytes(eocd_data[4..6].try_into().unwrap());
        let cdir_disk = u16::from_le_bytes(eocd_data[6..8].try_into().unwrap());
        let _num_entries_this_disk = u16::from_le_bytes(eocd_data[8..10].try_into().unwrap());
        let total_entries = u16::from_le_bytes(eocd_data[10..12].try_into().unwrap());
        let central_dir_size = u32::from_le_bytes(eocd_data[12..16].try_into().unwrap()) as u64;
        let central_dir_offset = u32::from_le_bytes(eocd_data[16..20].try_into().unwrap()) as u64;
        let comment_len = u16::from_le_bytes(eocd_data[20..22].try_into().unwrap()) as u64;

        // 基本验证
        if disk_num != 0 || cdir_disk != 0 {
            return Err(ZipError::generic("Multi-disk ZIP archives not supported"));
        }

        // 验证中央目录偏移的合理性
        if central_dir_offset >= file_size {
            return Err(ZipError::generic(&format!(
                "Invalid central directory offset: {} >= {}",
                central_dir_offset, file_size
            )));
        }

        // 验证中央目录不会超出文件范围
        if central_dir_offset + central_dir_size > file_size {
            return Err(ZipError::generic("Central directory extends beyond file"));
        }

        // 验证注释长度不会导致 EOCD 超出文件
        if offset + 22 + comment_len > file_size {
            return Err(ZipError::generic("EOCD comment extends beyond file"));
        }

        Ok(EocdRecord {
            central_dir_offset,
            central_dir_size,
            total_entries,
        })
    }

    /// 解析中央目录
    /// 对应 C 版本的 mz_zip_reader_get_num_files() + mz_zip_reader_file_stat()
    fn parse_central_directory<R: Read + Seek>(
        reader: &mut R,
        eocd: &EocdRecord,
    ) -> Result<Vec<ZipEntryInfo>> {
        let mut entries = Vec::new();

        // 定位到中央目录开始位置
        reader.seek(SeekFrom::Start(eocd.central_dir_offset))?;

        // 解析所有中央目录条目
        for _ in 0..eocd.total_entries {
            // 读取完整的中央目录头（46 字节，包括签名）
            // 对应 miniz.c:3083-3100
            let mut header = [0u8; 46];
            reader.read_exact(&mut header).map_err(|e| {
                ZipError::generic(&format!("Failed to read central directory header: {:?}", e))
            })?;

            // 验证签名（前 4 字节）
            if u32::from_le_bytes(header[0..4].try_into().unwrap()) != zip_format::CENTRAL_DIR_HEADER_SIG {
                return Err(ZipError::generic(&format!(
                    "Invalid central directory header signature: got 0x{:08x}",
                    u32::from_le_bytes(header[0..4].try_into().unwrap())
                )));
            }

            // 解析字段（偏移量从签名之后开始）
            // 对应 C 版本 miniz.c:3083-3100
            let version_made_by = u16::from_le_bytes(header[4..6].try_into().unwrap());
            let compression_method = u16::from_le_bytes(header[10..12].try_into().unwrap());
            let mtime_dos = u16::from_le_bytes(header[12..14].try_into().unwrap()); // DOS 时间
            let mdate_dos = u16::from_le_bytes(header[14..16].try_into().unwrap()); // DOS 日期
            let crc32 = u32::from_le_bytes(header[16..20].try_into().unwrap());
            let compressed_size = u32::from_le_bytes(header[20..24].try_into().unwrap()) as u64;
            let uncompressed_size = u32::from_le_bytes(header[24..28].try_into().unwrap()) as u64;
            let external_attr = u32::from_le_bytes(header[38..42].try_into().unwrap());
            let name_len = u16::from_le_bytes(header[28..30].try_into().unwrap()) as usize;
            let extra_len = u16::from_le_bytes(header[30..32].try_into().unwrap()) as usize;
            let comment_len = u16::from_le_bytes(header[32..34].try_into().unwrap()) as usize;
            let local_header_offset = u32::from_le_bytes(header[42..46].try_into().unwrap()) as u64;

            // 读取文件名
            let mut name_bytes = vec![0u8; name_len];
            reader.read_exact(&mut name_bytes).map_err(|e| {
                ZipError::generic(&format!("Failed to read filename: {:?}", e))
            })?;
            let name = String::from_utf8_lossy(&name_bytes).to_string();

            // 跳过 extra field 和 comment
            let skip_len = extra_len + comment_len;
            if skip_len > 0 {
                let mut skip_buf = vec![0u8; skip_len];
                reader.read_exact(&mut skip_buf).map_err(|e| {
                    ZipError::generic(&format!("Failed to skip extra/comment: {:?}", e))
                })?;
            }

            // 判断是否为目录
            // 对应 C 版本：m_zip_archive_file_stat.m_is_directory
            let is_dir = (external_attr & 0x10) != 0 || name.ends_with('/');

            entries.push(ZipEntryInfo {
                name,
                uncompressed_size,
                compressed_size,
                crc32,
                local_header_offset,
                is_dir,
                compression_method,
                external_attr,
                version_made_by,
                mtime_dos,
                mdate_dos,
            });
        }

        Ok(entries)
    }

    /// 获取中央目录之后的数据位置（追加模式的写入位置）
    /// 这对应 C 版本中追加文件时的起始位置
    pub fn get_append_offset(&self) -> u64 {
        // 中央目录之前的位置
        self.central_dir_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_eocd_parsing() {
        // 创建一个简单的 ZIP 文件用于测试
        let tmp_dir = TempDir::new().unwrap();
        let zip_path = tmp_dir.path().join("test.zip");

        // 创建最小的有效 ZIP 文件
        let mut file = File::create(&zip_path).unwrap();

        // 空 ZIP 只有 EOCD（22 字节）
        let eocd = [
            0x50, 0x4b, 0x05, 0x06,  // 签名
            0x00, 0x00,              // 磁盘编号
            0x00, 0x00,              // 起始磁盘
            0x00, 0x00,              // 记录数
            0x00, 0x00,              // 总记录数
            0x00, 0x00, 0x00, 0x00,  // 目录大小
            0x00, 0x00, 0x00, 0x00,  // 目录偏移
            0x00, 0x00,              // 注释长度
        ];

        file.write_all(&eocd).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // 测试读取
        let reader = ZipReader::open(&zip_path);
        assert!(reader.is_ok());

        let reader = reader.unwrap();
        assert_eq!(reader.entries.len(), 0);
    }
}
