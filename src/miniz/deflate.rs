//! DEFLATE 压缩算法实现
//!
//! 对应 C 版本 miniz.c 的 `tdefl_*` 函数系列：
//! - `compress()` 对应 `tdefl_compress_mem_to_heap()`
//! - `DeflateEncoder` 对应 `tdefl_compressor`
//! - 快速压缩对应 `tdefl_compress_fast()`
//!
//! 参考：`/home/putao/code/c-cpp/zip/src/miniz.c`

use crate::miniz::bitstream::BitWriter;
use crate::miniz::huffman::HuffmanTable;
use crate::miniz;
use std::mem;

/// DEFLATE 压缩级别
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionLevel {
    NoCompression = 0,
    Fastest = 1,
    Fast = 2,
    Default = 6,
    High = 7,
    Max = 9,
}

/// DEFLATE 压缩选项
#[derive(Debug, Clone)]
pub struct DeflateOptions {
    pub level: CompressionLevel,
    pub window_bits: i32,
    pub mem_level: i32,
    pub strategy: Strategy,
}

impl Default for DeflateOptions {
    fn default() -> Self {
        Self {
            level: CompressionLevel::Default,
            window_bits: 15,
            mem_level: 8,
            strategy: Strategy::Default,
        }
    }
}

/// 压缩策略
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    Default,
    Filtered,
    HuffmanOnly,
    Rle,
}

/// DEFLATE 压缩状态
#[derive(Debug)]
pub struct DeflateState {
    options: DeflateOptions,
    block_start: usize,
    next_out: usize,
    avail_out: usize,
    total_out: u64,
    hash_shift: u8,
    hash_size: u16,
    hash_mask: u16,
    hash_func: unsafe fn(usize, usize) -> usize,
    hash: Vec<u16>,
    prev: Vec<u16>,
    lit_huff: HuffmanTable,
    dist_huff: HuffmanTable,
    bit_writer: BitWriter,
}

/// DEFLATE 压缩器
pub struct DeflateEncoder {
    state: DeflateState,
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
}

impl DeflateEncoder {
    /// 创建新的 DEFLATE 压缩器
    pub fn new(options: DeflateOptions) -> Result<Self, DeflateError> {
        let hash_size = 1 << (options.window_bits - 8);
        let hash_mask = hash_size - 1;

        Ok(Self {
            state: DeflateState {
                options: options.clone(),
                block_start: 0,
                next_out: 0,
                avail_out: 0,
                total_out: 0,
                hash_shift: (options.window_bits - 8) as u8,
                hash_size: hash_size as u16,
                hash_mask: hash_mask as u16,
                hash_func: if options.level == CompressionLevel::Max || options.level == CompressionLevel::High {
                    Self::hash_func_good
                } else {
                    Self::hash_func_fast
                },
                hash: vec![0; hash_size],
                prev: vec![0; hash_size],
                lit_huff: HuffmanTable::new(),
                dist_huff: HuffmanTable::new(),
                bit_writer: BitWriter::new(),
            },
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
        })
    }

    /// 压缩数据
    pub fn compress(&mut self, data: &[u8], flush: FlushMode) -> Result<usize, DeflateError> {
        let compressed_data = self.deflate_compress(data, flush)?;

        self.output_buffer.extend_from_slice(&compressed_data);

        Ok(compressed_data.len())
    }

    /// 获取压缩后的数据
    pub fn get_compressed(&mut self) -> Result<Vec<u8>, DeflateError> {
        // 完成压缩并获取最终数据
        let mut output = mem::take(&mut self.output_buffer);

        // 如果需要 ZLIB header（window_bits > 8）
        if self.state.options.window_bits > 8 {
            // 对于空数据，使用正确的格式
            if output.is_empty() {
                // 直接返回 C 版本和 Python zlib 使用的格式
                return Ok(vec![0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
            }

            // 非空数据，添加 ZLIB header
            let zlib_header = self.create_zlib_header();
            output.splice(0..0, zlib_header);
        } else {
            // 对于空数据但没有 ZLIB header，也返回正确格式
            if output.is_empty() {
                return Ok(vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
            }
        }

        // 添加 Adler32 校验和（紧跟在 ZLIB header 或压缩数据之后）
        let adler32_val = miniz::adler32(0, &output);
        output.push((adler32_val >> 24) as u8);
        output.push((adler32_val >> 16) as u8);
        output.push((adler32_val >> 8) as u8);
        output.push(adler32_val as u8);

        Ok(output)
    }
}

/// 压缩模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlushMode {
    None,
    Sync,
    Full,
    Finish,
}

/// DEFLATE 压缩错误
#[derive(Debug, thiserror::Error)]
pub enum DeflateError {
    #[error("Invalid compression level")]
    InvalidLevel,
    #[error("Buffer too small")]
    BufferTooSmall,
    #[error("Compression error: {0}")]
    CompressionError(String),
    #[error("Bad parameter")]
    BadParam,
}

/// DEFLATE 压缩输出结构
#[derive(Debug, Clone, PartialEq)]
pub struct DeflateOutput {
    /// 输出数据
    pub output: Vec<u8>,
    /// 读取的字节数
    pub bytes_read: i32,
    /// 写入的字节数
    pub bytes_written: i32,
}

/// DEFLATE 压缩函数 - 供外部使用（与 C 版本签名匹配）
///
/// # 参数
///
/// - `data`: 输入数据
/// - `level`: 压缩级别（1-9）
/// - `pos`: 起始位置（1-based，与 C 版本一致）
/// - `_size`: 缓冲区大小估计，None 表示自动分配（当前未使用）
pub fn compress(data: &[u8], level: i32, pos: i32, _size: Option<i32>) -> Result<DeflateOutput, DeflateError> {
    // 特殊处理空数据
    if data.is_empty() {
        // 返回标准的空压缩数据（ZLIB 格式）
        return Ok(DeflateOutput {
            output: vec![0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
            bytes_read: 0,
            bytes_written: 8,
        });
    }

    // 验证 pos 范围（1-based 索引）
    if pos < 1 {
        return Err(DeflateError::BadParam);
    }

    let pos_index = (pos - 1) as usize;

    // 验证 pos 不超出范围
    if pos_index >= data.len() {
        return Err(DeflateError::BadParam);
    }

    // 从 pos 位置开始读取数据
    let input_data = &data[pos_index..];

    let options = DeflateOptions {
        level: match level {
            0 => CompressionLevel::NoCompression,
            1 => CompressionLevel::Fastest,
            2 => CompressionLevel::Fast,
            6 => CompressionLevel::Default,
            7 => CompressionLevel::High,
            9 => CompressionLevel::Max,
            _ => return Err(DeflateError::InvalidLevel),
        },
        window_bits: 15,  // ZLIB format with header
        ..Default::default()
    };

    let mut encoder = DeflateEncoder::new(options)?;
    encoder.compress(input_data, FlushMode::Finish)?;

    let output = encoder.get_compressed()?;
    let bytes_written = output.len() as i32;

    Ok(DeflateOutput {
        output,
        bytes_read: input_data.len() as i32,
        bytes_written,
    })
}

/// 压缩结果（带统计信息）
pub struct CompressResult {
    pub output: Vec<u8>,
    pub bytes_read: usize,
    pub bytes_written: usize,
}

/// 压缩到预分配的缓冲区
pub fn compress_to_buffer(data: &[u8], level: i32, capacity: usize) -> Result<CompressResult, DeflateError> {
    let result = compress(data, level, 1, Some(capacity as i32))?;

    Ok(CompressResult {
        output: result.output,
        bytes_read: result.bytes_read as usize,
        bytes_written: result.bytes_written as usize,
    })
}

/// 原始 DEFLATE 压缩（不带 ZLIB 头部）
pub fn compress_raw(data: &[u8], level: i32) -> Result<Vec<u8>, DeflateError> {
    let result = compress(data, level, 1, None)?;

    // 移除 ZLIB 头部和尾部
    if result.output.len() >= 6 {
        // 跳过 ZLIB 头部 (2字节) 和 CRC32 (4字节)
        Ok(result.output[2..result.output.len() - 4].to_vec())
    } else {
        Err(DeflateError::CompressionError("Compressed data too short".to_string()))
    }
}

impl DeflateEncoder {
    fn deflate_compress(&mut self, data: &[u8], _flush: FlushMode) -> Result<Vec<u8>, DeflateError> {
        if self.state.options.level == CompressionLevel::NoCompression {
            // 不压缩，直接存储
            let mut output = Vec::new();

            if !data.is_empty() {
                // 块头 (BFINAL=0, BTYPE=00 无压缩)
                output.push(0x00);

                // 长度 (小端)
                let len = data.len() as u16;
                output.push(len as u8);
                output.push((len >> 8) as u8);

                // 255的补码
                output.push(!len as u8);
                output.push((!len >> 8) as u8);

                // 数据
                output.extend_from_slice(data);
            } else {
                // 对于 NoCompression 级别的空数据，也使用静态 Huffman 块
                // 块头 (BFINAL=0, BTYPE=10 静态Huffman)
                output.push(0x01);  // BFINAL=0, BTYPE=10

                // 块结束标记（静态 Huffman 编码的 256）
                output.push(0x00);
                output.push(0x2d);
            }

            return Ok(output);
        }

        // 使用快速压缩实现
        self.compress_fast(data)
    }

    /// 位反转函数 - 将 MSB 优先的码转换为 LSB 优先
    fn reverse_bits(code: u16, len: u8) -> u16 {
        let mut result = 0u16;
        for i in 0..len {
            result |= ((code >> i) & 1) << (len - 1 - i);
        }
        result
    }

    /// 使用快速压缩实现
    fn compress_fast(&self, data: &[u8]) -> Result<Vec<u8>, DeflateError> {
        use crate::miniz::deflate_fast;

        // 对于空数据，让 get_compressed 处理特殊的 ZLIB 格式
        if data.is_empty() {
            return Ok(Vec::new());
        }

        // 使用快速压缩（LZ77 + 静态Huffman）
        deflate_fast::deflate_compress_fast(data).map_err(|e| DeflateError::CompressionError(e))
    }

    /// ZLIB 格式的压缩实现（带头部和尾部）
    fn compress_zlib(&self, data: &[u8]) -> Result<Vec<u8>, DeflateError> {
        let mut output = Vec::new();

        // 添加 ZLIB 头部
        output.extend_from_slice(&self.create_zlib_header());

        // 根据数据长度选择压缩策略
        if data.is_empty() {
            // 对于空数据，使用存储的块（像 C 版本一样）
            // 块头 (BFINAL=1, BTYPE=00 存储块，因为是最后一个块)
            output.push(0x03);  // BFINAL=1, BTYPE=00 (二进制 00000011)

            // 空存储块：len=0, nlen=0xFFFF
            output.push(0x00);
            output.push(0x00);
            output.push(0xFF);
            output.push(0xFF);
        } else {
            // 对于非空数据，使用 DEFLATE 压缩
            let deflate_data = self.compress_fast(data)?;
            output.extend_from_slice(&deflate_data);
        }

        // ZLIB 格式需要 Adler32 校验和已经在压缩数据中
        Ok(output)
    }

    /// 创建 ZLIB 头部
    fn create_zlib_header(&self) -> Vec<u8> {
        // CMF (Compression Method and flags): method=8 (deflate), info=7 (level 6)
        // FLG (Flags): check_bits=156 (makes CMF*256+FLG divisible by 31)
        // 0x78 = 0b01111000 (method=8, info=7)
        // 0x9C = 0b10011100 (check_bits=156) - standard zlib header
        let cmf = 0x78; // method=8, info=7
        let flg = 0x9C; // standard zlib header
        vec![cmf, flg]
    }

    fn get_static_huffman_code(&self, byte: u8) -> u16 {
        // 静态 Huffman 码表 (RFC 1951)
        match byte {
            0..=143 => 0x0030 + byte as u16,
            144..=255 => 0x0190 + (byte - 144) as u16,
        }
    }

    // 快速哈希函数
    unsafe fn hash_func_fast(_data: usize, _hash_size: usize) -> usize {
        0
    }

    // 优质的哈希函数
    unsafe fn hash_func_good(_data: usize, _hash_size: usize) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression() {
        let data = b"Hello, World!";
        let compressed = compress(data, 0, 1, None).unwrap();

        // 不压缩应该大于原始数据
        assert!(compressed.output.len() > data.len());
    }

    #[test]
    fn test_compression_level() {
        let data = b"Hello, World! This is a test string for compression.";

        for level in [1, 2, 6, 7, 9] {
            let compressed = compress(data, level, 1, None).unwrap();
            assert!(!compressed.output.is_empty());
        }
    }

    #[test]
    fn test_raw_compression() {
        let data = b"Hello, World!";
        let compressed = compress_raw(data, 6).unwrap();

        // 原始压缩应该比带头部的压缩短
        let with_header = compress(data, 6, 1, None).unwrap();
        assert!(compressed.len() < with_header.output.len());
    }
}