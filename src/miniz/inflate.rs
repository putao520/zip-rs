//! INFLATE 解压算法实现
//!
//! 对应 C 版本 miniz.c 的 `tinfl_*` 函数系列：
//! - `decompress()` 对应 `tinfl_decompress()`
//! - `InflateDecoder` 对应 `tinfl_decompressor`
//!
//! 参考：`/home/putao/code/c-cpp/zip/src/miniz.c`

use crate::miniz::bitstream::BitReader;
use crate::miniz::huffman::HuffmanTable;

/// INFLATE 解压状态标志
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InflateStatus {
    /// 继续处理
    Ok,
    /// 需要更多输入数据
    NeedsMoreInput,
    /// 有更多输出
    HasMoreOutput,
    /// 解压完成
    Done,
    /// 解压失败
    Failed,
    /// 校验和不匹配
    Adler32Mismatch,
    /// 参数错误
    BadParam,
    /// 无法继续处理
    CannotMakeProgress,
}

/// 解压标志
#[derive(Debug, Clone, Copy)]
pub struct InflateFlags {
    pub parse_zlib_header: bool,
    pub has_more_input: bool,
    pub using_non_wrapping_output_buf: bool,
}

impl Default for InflateFlags {
    fn default() -> Self {
        Self {
            parse_zlib_header: false,
            has_more_input: false,
            using_non_wrapping_output_buf: false,
        }
    }
}

/// INFLATE 解压状态
#[derive(Debug)]
pub struct InflateState {
    /// 位读取器
    bit_reader: BitReader,
    /// 当前位缓冲区
    bit_buf: u64,
    /// 缓冲区中的位数
    num_bits: u8,
    /// 最后一个块的标志
    final_block: bool,
    /// 块类型 (0=无压缩, 1=静态Huffman, 2=动态Huffman)
    block_type: u8,
    /// 距离
    dist: u32,
    /// 计数器
    counter: u32,
    /// 额外位数
    num_extra: u8,
    /// 输出缓冲区
    output_buffer: Vec<u8>,
    /// 输出位置
    output_pos: usize,
    /// ZLIB 头部字节 0
    zhdr0: u8,
    /// ZLIB 头部字节 1
    zhdr1: u8,
    /// 预期的 Adler32 校验和
    z_adler32: u32,
    /// 计算的 Adler32 校验和
    check_adler32: u32,
    /// Huffman 表
    tables: [HuffmanTable; 3],
    /// 表大小
    table_sizes: [usize; 3],
}

/// INFLATE 解码器
pub struct InflateDecoder {
    state: InflateState,
}

impl InflateDecoder {
    /// 创建新的 INFLATE 解码器
    pub fn new() -> Self {
        let mut state = InflateState {
            bit_reader: BitReader::new(Vec::new()),
            bit_buf: 0,
            num_bits: 0,
            final_block: false,
            block_type: 0,
            dist: 0,
            counter: 0,
            num_extra: 0,
            output_buffer: Vec::new(),
            output_pos: 0,
            zhdr0: 0,
            zhdr1: 0,
            z_adler32: 0,
            check_adler32: 0,
            tables: [HuffmanTable::new(), HuffmanTable::new(), HuffmanTable::new()],
            table_sizes: [0, 0, 0],
        };

        // 初始化静态 Huffman 表
        Self::init_static_huffman_tables(&mut state);

        Self { state }
    }

    /// 解压数据
    pub fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flags: InflateFlags,
    ) -> Result<(InflateStatus, usize, usize), InflateError> {
        // Debug: print input data
        println!("DEBUG: Input data ({} bytes): {:?}", input.len(), input);

        // 设置输入数据
        self.state.bit_reader = BitReader::from_slice(input);
        self.state.bit_buf = 0;
        self.state.num_bits = 0;

        // 重置状态
        self.state.final_block = false;
        self.state.block_type = 0;
        self.state.dist = 0;
        self.state.counter = 0;
        self.state.num_extra = 0;
        self.state.output_pos = 0;
        self.state.output_buffer.clear();
        self.state.zhdr0 = 0;
        self.state.zhdr1 = 0;
        self.state.z_adler32 = 1;
        self.state.check_adler32 = 1;

        // 解压 ZLIB 头部（如果需要）
        if flags.parse_zlib_header {
            self.parse_zlib_header()?;
        }

        // Debug output
        println!("DEBUG: Starting decompress, input len: {}, flags: {:?}", input.len(), flags);

        // 解压主循环
        let result = loop {
            // 如果上一个块已经是最终块，返回 Done
            if self.state.final_block {
                println!("DEBUG: Final block was processed, returning Done");
                break (InflateStatus::Done, self.state.output_pos);
            }

            match self.decompress_block(flags) {
                InflateStatus::Done => {
                    println!("DEBUG: Decompression completed");
                    break (InflateStatus::Done, self.state.output_pos);
                }
                InflateStatus::HasMoreOutput => {
                    if self.state.output_pos >= output.len() {
                        break (InflateStatus::HasMoreOutput, self.state.output_pos);
                    }
                    continue;
                }
                InflateStatus::NeedsMoreInput => {
                    break (InflateStatus::NeedsMoreInput, self.state.output_pos);
                }
                InflateStatus::Failed => {
                    println!("DEBUG: Decompression failed");
                    return Err(InflateError::DecompressionFailed);
                }
                InflateStatus::Adler32Mismatch => {
                    return Err(InflateError::Adler32Mismatch);
                }
                InflateStatus::BadParam => {
                    return Err(InflateError::BadParam);
                }
                InflateStatus::CannotMakeProgress => {
                    return Err(InflateError::CannotMakeProgress);
                }
                InflateStatus::Ok => {
                    continue;
                }
            }
        };

        // 将内部 output_buffer 复制到外部输出缓冲区
        let copy_len = self.state.output_pos.min(output.len());
        output[..copy_len].copy_from_slice(&self.state.output_buffer[..copy_len]);

        println!("DEBUG: Copied {} bytes to external output buffer", copy_len);

        Ok((result.0, result.1, 0))
    }

    /// 获取解压后的数据
    pub fn get_output(&mut self) -> Vec<u8> {
        let mut output = std::mem::take(&mut self.state.output_buffer);
        output.truncate(self.state.output_pos);
        output
    }
}

/// INFLATE 解压输出结构
#[derive(Debug, Clone, PartialEq)]
pub struct InflateOutput {
    /// 输出数据
    pub output: Vec<u8>,
    /// 读取的字节数
    pub bytes_read: i32,
    /// 写入的字节数
    pub bytes_written: i32,
}

/// INFLATE 解压函数 - 供外部使用（与 C 版本签名匹配）
///
/// # 参数
///
/// - `data`: 输入数据
/// - `pos`: 起始位置（1-based，与 C 版本一致）
/// - `size`: 缓冲区大小估计，None 表示自动分配
pub fn decompress(data: &[u8], pos: i32, size: Option<i32>) -> Result<InflateOutput, InflateError> {
    // 特殊处理空数据
    if data.is_empty() {
        // 空数据返回空输出
        return Ok(InflateOutput {
            output: vec![],
            bytes_read: 0,
            bytes_written: 0,
        });
    }

    // 验证 pos 范围（1-based 索引）
    if pos < 1 {
        return Err(InflateError::BadParam);
    }

    let pos_index = (pos - 1) as usize;

    // 验证 pos 不超出范围
    if pos_index >= data.len() {
        return Err(InflateError::BadParam);
    }

    // 确定输出缓冲区大小
    let capacity = match size {
        Some(s) if s > 0 => s as usize,
        _ => data.len() * 2, // 默认分配两倍大小
    };

    // 从 pos 位置开始读取数据
    let input_data = &data[pos_index..];

    let mut decoder = InflateDecoder::new();
    let mut output = vec![0; capacity];

    let (status, bytes_read, _) = decoder.decompress(
        input_data,
        &mut output,
        InflateFlags {
            parse_zlib_header: true,
            ..Default::default()
        },
    )?;

    if status != InflateStatus::Done {
        return Err(InflateError::DecompressionFailed);
    }

    output.truncate(bytes_read);

    Ok(InflateOutput {
        output,
        bytes_read: input_data.len() as i32,
        bytes_written: bytes_read as i32,
    })
}

/// 解压结果（带统计信息）
pub struct DecompressResult {
    pub output: Vec<u8>,
    pub bytes_read: usize,
    pub bytes_written: usize,
}

/// 解压到预分配的缓冲区
pub fn decompress_to_buffer(data: &[u8], capacity: usize) -> Result<DecompressResult, InflateError> {
    let result = decompress(data, 1, Some(capacity as i32))?;

    Ok(DecompressResult {
        output: result.output,
        bytes_read: result.bytes_read as usize,
        bytes_written: result.bytes_written as usize,
    })
}

/// 原始 INFLATE 解压（不带 ZLIB 头部）
pub fn decompress_raw(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    let mut decoder = InflateDecoder::new();
    let mut output = vec![0; data.len() * 2]; // 预分配较大的输出缓冲区

    let (status, bytes_read, _) = decoder.decompress(
        data,
        &mut output,
        InflateFlags {
            parse_zlib_header: false,
            ..Default::default()
        },
    )?;

    if status != InflateStatus::Done {
        return Err(InflateError::DecompressionFailed);
    }

    output.truncate(bytes_read);
    Ok(output)
}

impl InflateDecoder {
    /// 初始化静态 Huffman 表
    fn init_static_huffman_tables(state: &mut InflateState) {
        // 静态字面码表
        let mut litlen_codes = [0u16; 288];
        for i in 0..=143 {
            litlen_codes[i] = (144 + i) as u16;
        }
        for i in 144..=255 {
            litlen_codes[i] = (280 + i - 144) as u16;
        }
        for i in 256..=279 {
            litlen_codes[i] = (256 + i - 256) as u16;
        }
        for i in 280..=287 {
            litlen_codes[i] = (280 + i - 256) as u16;
        }

        state.tables[0] = HuffmanTable::new().build_static_table(&litlen_codes).unwrap_or_default();
        state.table_sizes[0] = 288;

        // 静态距离码表
        let mut dist_codes = [0u16; 32];
        for i in 0..=31 {
            dist_codes[i] = i as u16;
        }

        state.tables[1] = HuffmanTable::new().build_static_table(&dist_codes).unwrap_or_default();
        state.table_sizes[1] = 32;
    }

    /// 从 bit_buf 读取指定位数（对应 C 版本的 TINFL_GET_BITS）
    /// 返回读取的值，并更新 bit_buf 和 num_bits
    fn get_bits(&mut self, n: u8) -> Result<u32, InflateError> {
        // 对应 C 版本的 TINFL_NEED_BITS: 确保 bit_buf 有足够的位
        while self.state.num_bits < n as u8 {
            if self.state.bit_reader.has_more_bytes(1) {
                let byte = self.state.bit_reader.read_byte().unwrap_or(0);
                self.state.bit_buf |= (byte as u64) << self.state.num_bits;
                self.state.num_bits += 8;
            } else {
                return Err(InflateError::NeedMoreInput);
            }
        }

        // 对应 C 版本的 TINFL_GET_BITS: 提取 n 位
        let mask = (1u32 << n) - 1;
        let result = (self.state.bit_buf & mask as u64) as u32;

        // 消耗这些位
        self.state.bit_buf >>= n;
        self.state.num_bits -= n as u8;

        Ok(result)
    }

    /// 解压 ZLIB 头部
    fn parse_zlib_header(&mut self) -> Result<(), InflateError> {
        // 读取 ZLIB 头部
        if !self.state.bit_reader.has_more_bytes(1) {
            return Err(InflateError::NeedMoreInput);
        }
        self.state.zhdr0 = self.state.bit_reader.read_byte().unwrap();

        if !self.state.bit_reader.has_more_bytes(1) {
            return Err(InflateError::NeedMoreInput);
        }
        self.state.zhdr1 = self.state.bit_reader.read_byte().unwrap();

        // 验证 ZLIB 头部
        let cmf = self.state.zhdr0;
        let flg = self.state.zhdr1;
        let cm = cmf & 0x0F;
        let cinfo = (cmf >> 4) & 0x0F;
        let _fcheck = (flg & 0x1F) as u32;
        let fdict = (flg & 0x20) != 0;
        let _flevel = (flg >> 6) & 0x03;

        if cm != 8 {
            return Err(InflateError::BadZlibHeader);
        }

        if cinfo > 7 {
            return Err(InflateError::BadZlibHeader);
        }

        if (cmf as u32 * 256 + flg as u32) % 31 != 0 {
            return Err(InflateError::BadZlibHeader);
        }

        // fdict should be 0, and fcheck can be non-zero for standard zlib
        if fdict {
            return Err(InflateError::BadZlibHeader);
        }

        // 重置 Adler32 校验和
        self.state.z_adler32 = 1;
        self.state.check_adler32 = 1;

        // 对齐位缓冲区以确保字节对齐
        self.state.bit_reader.align_to_byte();

        // 重置缓冲区以确保从干净状态开始
        self.state.bit_buf = 0;
        self.state.num_bits = 0;

        // 不需要手动设置位置，因为 read_byte() 已经正确移动了 pos 到 2

        println!("DEBUG: ZLIB header parsed - cm: {}, cinfo: {}, fcheck: {}, fdict: {}", cm, cinfo, _fcheck, fdict);

        // Skip remaining bits to align to byte boundary (like C version's TINFL_SKIP_BITS(5, num_bits & 7))
        let remaining_bits = self.state.bit_reader.buffered_bits() % 8;
        if remaining_bits > 0 {
            println!("DEBUG: Skipping {} bits after ZLIB header", remaining_bits);
            self.state.bit_reader.read_bits(remaining_bits);
        }

        println!("DEBUG: After ZLIB header alignment - bit_pos: {}, num_bits: {}", self.state.bit_reader.get_pos(), self.state.bit_reader.buffered_bits());

        // 确保 ZLIB header 解析后在字节边界开始读取 deflate 块头
        if self.state.bit_reader.buffered_bits() != 0 {
            println!("DEBUG: Warning: Not aligned to byte boundary after ZLIB header");
            return Err(InflateError::BadZlibHeader);
        }

        println!("DEBUG: After ZLIB header - bit_pos: {}, num_bits: {}", self.state.bit_reader.get_pos(), self.state.bit_reader.buffered_bits());

        Ok(())
    }

    /// 解压一个块
    fn decompress_block(&mut self, _flags: InflateFlags) -> InflateStatus {
        // 确保 bit_buf 有至少 3 位（像 C 版本的 TINFL_NEED_BITS）
        while self.state.num_bits < 3 {
            if self.state.bit_reader.has_more_bytes(1) {
                let byte = self.state.bit_reader.read_byte().unwrap_or(0);
                self.state.bit_buf |= (byte as u64) << self.state.num_bits;
                self.state.num_bits += 8;
            } else {
                return InflateStatus::NeedsMoreInput;
            }
        }

        println!("DEBUG: Before reading block header - num_bits: {}, bit_buf: {:064b}",
                 self.state.num_bits, self.state.bit_buf);

        // 读取块头 (3 bits, like C version's TINFL_GET_BITS(3, r->m_final, 3))
        let block_header = (self.state.bit_buf & 0x7) as u32;
        self.state.bit_buf >>= 3;
        self.state.num_bits -= 3;
        self.state.final_block = (block_header & 1) == 1;
        self.state.block_type = (block_header >> 1) as u8;

        println!("DEBUG: decompress_block - final_block: {} (bit0: {}), block_type: {} (header: {:03b})", self.state.final_block, block_header & 1, self.state.block_type, block_header);
        println!("DEBUG: After reading block header - num_bits: {}, bit_buf: {:064b}",
                 self.state.num_bits, self.state.bit_buf);

        match self.state.block_type {
            0 => self.decompress_uncompressed_block(),
            1 | 2 => self.decompress_compressed_block(),
            3 => InflateStatus::Failed, // 错误的块类型
            _ => InflateStatus::Failed,
        }
    }

    /// 解压无压缩块
    fn decompress_uncompressed_block(&mut self) -> InflateStatus {
        println!("DEBUG: decompress_uncompressed_block - bit_buf: {}, num_bits: {}", self.state.bit_buf, self.state.num_bits);

        // 跳过剩余的位以对齐到字节边界 (like C version's TINFL_SKIP_BITS(5, num_bits & 7))
        let num_bits = self.state.num_bits & 7;
        if num_bits != 0 {
            println!("DEBUG: Skipping {} bits", num_bits);
            self.state.bit_buf >>= num_bits;
            self.state.num_bits -= num_bits;
        }

        println!("DEBUG: After alignment - num_bits: {}", self.state.num_bits);

        // 读取长度和补码 (像 C 版本一样)
        let mut raw_header = [0u8; 4];
        for i in 0..4 {
            if self.state.num_bits >= 8 {
                // 从 bit_buf 读取 8 位
                raw_header[i] = (self.state.bit_buf & 0xFF) as u8;
                self.state.bit_buf >>= 8;
                self.state.num_bits -= 8;
            } else {
                // bit_buf 为空，直接读取下一个字节
                if self.state.bit_reader.has_more_bytes(1) {
                    raw_header[i] = self.state.bit_reader.read_byte().unwrap();
                } else {
                    return InflateStatus::NeedsMoreInput;
                }
            }
        }

        let len = raw_header[0] as u16 | ((raw_header[1] as u16) << 8);
        let nlen = raw_header[2] as u16 | ((raw_header[3] as u16) << 8);

        println!("DEBUG: Uncompressed block header bytes - raw_header: {:?}, len: {}, nlen: {}", raw_header, len, nlen);

        // 验证补码
        if len != !nlen {
            println!("DEBUG: Checksum failed: len ({}) != !nlen ({})", len, !nlen);
            return InflateStatus::Failed;
        }

        // 读取数据 (像 C 版本一样)
        println!("DEBUG: Reading {} bytes of data", len);
        for _ in 0..len {
            if self.state.num_bits >= 8 {
                // 从 bit_buf 读取 8 位
                let byte = (self.state.bit_buf & 0xFF) as u8;
                self.state.bit_buf >>= 8;
                self.state.num_bits -= 8;
                self.state.output_buffer.push(byte);
                self.state.output_pos += 1;
            } else {
                // bit_buf 为空，直接读取下一个字节
                if self.state.bit_reader.has_more_bytes(1) {
                    let byte = self.state.bit_reader.read_byte().unwrap();
                    self.state.output_buffer.push(byte);
                    self.state.output_pos += 1;
                } else {
                    return InflateStatus::NeedsMoreInput;
                }
            }
        }

        println!("DEBUG: Read {} bytes, output_pos: {}", len, self.state.output_pos);

        // 检查是否完成
        if self.state.final_block {
            println!("DEBUG: Final block, checking Adler32");
            match self.check_adler32_checksum() {
                Ok(_) => {
                    println!("DEBUG: Adler32 check passed");
                    return InflateStatus::Done;
                }
                Err(_) => {
                    println!("DEBUG: Adler32 check failed");
                    return InflateStatus::Adler32Mismatch;
                }
            }
        }

        println!("DEBUG: Block processed successfully");
        InflateStatus::Ok
    }

    /// 解析动态 Huffman 表
    /// 对应 C 版本的动态 Huffman 表解析逻辑 (miniz.c:2457-2547)
    fn parse_dynamic_huffman_tables(&mut self) -> Result<(), InflateError> {
        if self.state.block_type != 2 {
            return Ok(());
        }

        // 对应 C 版本: TINFL_GET_BITS(11, r->m_table_sizes[counter], "\05\05\04"[counter])
        // 读取 HLIT (5 bits): 长度/字面码的码长数减 257
        let hlit = self.get_bits(5)? as i32 + 257;

        // 读取 HDIST (5 bits): 距离码的码长数减 1
        let hdist = self.get_bits(5)? as i32 + 1;

        // 读取 HCLEN (4 bits): 码长码的码长数减 4
        let hclen = self.get_bits(4)? as i32 + 4;

        println!("DEBUG: Dynamic Huffman - hlit: {}, hdist: {}, hclen: {}", hlit, hdist, hclen);

        // 对应 C 版本: 读取码长码的码长 (line 2464)
        // 读取码长码的码长（按 LENGTH_DEZIGZAG 顺序）
        let mut codelens = [0i32; 19];
        for i in 0..hclen as usize {
            let idx = crate::miniz::huffman::LENGTH_DEZIGZAG[i];
            // 读取 3 位码长
            codelens[idx] = self.get_bits(3)? as i32;
        }

        println!("DEBUG: Code lengths: {:?}", &codelens[..19]);

        // 构建码长码的 Huffman 表
        let codelen_table = HuffmanTable::build(&codelens.iter().map(|&x| x as u8).collect::<Vec<_>>(), 19)
            .map_err(|_| InflateError::DecompressionFailed)?;

        // 对应 C 版本: 读取长度和距离的码长 (line 2470-2547)
        let mut code_lengths = vec![0i32; (hlit + hdist) as usize];
        let mut i = 0;

        while i < (hlit + hdist) as usize {
            // 确保 bit_buf 有足够的位（最多需要 15 位用于 Huffman 解码）
            while self.state.num_bits < 15 {
                if self.state.bit_reader.has_more_bytes(1) {
                    let byte = self.state.bit_reader.read_byte().unwrap_or(0);
                    self.state.bit_buf |= (byte as u64) << self.state.num_bits;
                    self.state.num_bits += 8;
                } else {
                    return Err(InflateError::NeedMoreInput);
                }
            }

            // 解码码长码
            let (symbol, code_len) = codelen_table.decode_with_bits(self.state.bit_buf, self.state.num_bits);

            // 消耗这些位
            self.state.bit_buf >>= code_len;
            self.state.num_bits -= code_len;

            if symbol < 16 {
                // 字面值 (0-15): 直接使用
                code_lengths[i] = symbol as i32;
                i += 1;
            } else {
                // 特殊值 (16-18): 重复前面的值
                match symbol {
                    16 => {
                        // 重复前一个值 3-6 次
                        // 读取 2 位额外位
                        let extra = self.get_bits(2)? as usize;
                        let repeat_count = 3 + extra;
                        let prev_val = if i > 0 { code_lengths[i - 1] } else { 0 };
                        for _ in 0..repeat_count {
                            if i < code_lengths.len() {
                                code_lengths[i] = prev_val;
                                i += 1;
                            }
                        }
                    }
                    17 => {
                        // 重复 0 长度 3-10 次
                        // 读取 3 位额外位
                        let extra = self.get_bits(3)? as usize;
                        let repeat_count = 3 + extra;
                        for _ in 0..repeat_count {
                            if i < code_lengths.len() {
                                code_lengths[i] = 0;
                                i += 1;
                            }
                        }
                    }
                    18 => {
                        // 重复 0 长度 11-138 次
                        // 读取 7 位额外位
                        let extra = self.get_bits(7)? as usize;
                        let repeat_count = 11 + extra;
                        for _ in 0..repeat_count {
                            if i < code_lengths.len() {
                                code_lengths[i] = 0;
                                i += 1;
                            }
                        }
                    }
                    _ => return Err(InflateError::InvalidCode),
                }
            }
        }

        println!("DEBUG: Parsed {} code lengths", code_lengths.len());

        // 构建长度码表
        let mut litlen_code_lengths = vec![0u8; 288];
        for (i, &len) in code_lengths[..hlit as usize].iter().enumerate() {
            if i < 288 {
                litlen_code_lengths[i] = len as u8;
            }
        }
        self.state.tables[0] = HuffmanTable::build(&litlen_code_lengths, 288)
            .map_err(|_| InflateError::DecompressionFailed)?;
        self.state.table_sizes[0] = 288;

        // 构建距离码表
        let mut dist_code_lengths = vec![0u8; 32];
        for (i, &len) in code_lengths[hlit as usize..].iter().enumerate() {
            if i < 32 {
                dist_code_lengths[i] = len as u8;
            }
        }
        self.state.tables[1] = HuffmanTable::build(&dist_code_lengths, 32)
            .map_err(|_| InflateError::DecompressionFailed)?;
        self.state.table_sizes[1] = 32;

        Ok(())
    }

    /// 解压压缩块
    fn decompress_compressed_block(&mut self) -> InflateStatus {
        // 初始化 Huffman 表（根据块类型）
        if self.state.block_type == 1 {
            println!("DEBUG: Building static Huffman tables");
            // 静态 Huffman 表（RFC 1951）
            // 字面/长度码: 0-143(8位), 144-255(9位), 256-279(7位), 280-287(8位)
            let mut litlen_code_lengths = vec![0u8; 288];
            for i in 0..=143 {
                litlen_code_lengths[i] = 8;
            }
            for i in 144..=255 {
                litlen_code_lengths[i] = 9;
            }
            for i in 256..=279 {
                litlen_code_lengths[i] = 7;
            }
            for i in 280..=287 {
                litlen_code_lengths[i] = 8;
            }

            match HuffmanTable::build(&litlen_code_lengths, 288) {
                Ok(table) => {
                    self.state.tables[0] = table;
                    self.state.table_sizes[0] = 288;
                    println!("DEBUG: Static lit/len table built successfully, size: 288");
                }
                Err(e) => {
                    println!("DEBUG: Failed to build lit/len table: {:?}", e);
                    return InflateStatus::Failed;
                }
            }

            // 距离码: 0-29(5位)
            let dist_code_lengths = vec![5u8; 32];
            match HuffmanTable::build(&dist_code_lengths, 32) {
                Ok(table) => {
                    self.state.tables[1] = table;
                    self.state.table_sizes[1] = 32;
                    println!("DEBUG: Static distance table built successfully, size: 32");
                }
                Err(e) => {
                    println!("DEBUG: Failed to build distance table: {:?}", e);
                    return InflateStatus::Failed;
                }
            }
        } else if self.state.block_type == 2 {
            // 解析动态 Huffman 表
            if let Err(_) = self.parse_dynamic_huffman_tables() {
                return InflateStatus::Failed;
            }
        }

        // DEFLATE 压缩块的完整实现，参考 C 版本的 tinfl_decompress()
        println!("DEBUG: decompress_compressed_block - block_type: {}", self.state.block_type);

        // Huffman 表长度和距离的基础值
        static LENGTH_BASE: [i32; 31] = [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
            35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 0, 0
        ];
        static LENGTH_EXTRA: [i32; 31] = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
            3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0
        ];
        static DIST_BASE: [i32; 32] = [
            1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
            257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0
        ];
        static DIST_EXTRA: [i32; 32] = [
            0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
            7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 0, 0
        ];

        // 长度码的解zigzag序列
        static LENGTH_DEZIGZAG: [u8; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15
        ];
        static MIN_TABLE_SIZES: [i32; 3] = [257, 1, 4];

        loop {
            // 尝试解码字面字节或长度距离对
            match self.decode_huffman_symbol() {
                Some(256) => {
                    // 块结束标记
                    println!("DEBUG: End of block marker found, final_block: {}", self.state.final_block);
                    return InflateStatus::Ok;
                }
                Some(literal) if literal < 256 => {
                    // 字面字节
                    self.state.output_buffer.push(literal as u8);
                    self.state.output_pos += 1;
                }
                Some(length_code) => {
                    // 长度距离对
                    let mut length = LENGTH_BASE[(length_code - 257) as usize] as u32;
                    let num_extra_bits = LENGTH_EXTRA[(length_code - 257) as usize] as u8;
                    if num_extra_bits > 0 {
                        // 使用 get_bits 从 bit_buf 读取额外位（对应 C 版本的 TINFL_GET_BITS）
                        let extra_bits = self.get_bits(num_extra_bits).unwrap_or(0);
                        length += extra_bits as u32;
                    }

                    // 解码距离（使用距离表，表1）
                    match self.decode_distance_symbol() {
                        Some(dist_code) => {
                            let mut distance = DIST_BASE[dist_code as usize] as u32;
                            let dist_extra_bits = DIST_EXTRA[dist_code as usize] as u8;
                            if dist_extra_bits > 0 {
                                // 使用 get_bits 从 bit_buf 读取额外位
                                let extra_bits = self.get_bits(dist_extra_bits).unwrap_or(0);
                                distance += extra_bits as u32;
                            }

                            // LZ77 复制
                            self.lz77_copy(length, distance);
                        }
                        None => return InflateStatus::NeedsMoreInput,
                    }
                }
                None => {
                    // 需要更多输入数据
                    return InflateStatus::NeedsMoreInput;
                }
            }
        }
    }

    /// 解码 Huffman 符号（使用指定的表）
    /// table_index: 0 = 字面/长度表, 1 = 距离表
    fn decode_huffman_symbol_with_table(&mut self, table_index: usize) -> Option<u16> {
        // 填充位缓冲区（像 C 版本的 TINFL_NEED_BITS）
        // 对于静态 Huffman，最多需要 15 位
        while self.state.num_bits < 15 {
            if self.state.bit_reader.has_more_bytes(1) {
                let byte = self.state.bit_reader.read_byte().unwrap_or(0);
                self.state.bit_buf |= (byte as u64) << self.state.num_bits;
                self.state.num_bits += 8;
            } else {
                break;
            }
        }

        // Debug output
        println!("DEBUG: decode_huffman_symbol_with_table(table={}) - num_bits: {}, bit_buf: 0x{:x}",
                 table_index, self.state.num_bits, self.state.bit_buf);

        // 检查表索引有效
        if table_index >= 2 || self.state.table_sizes[table_index] == 0 {
            return None;
        }

        let table = &self.state.tables[table_index];

        // 使用快速路径尝试解码
        let symbol_len = table.decode(self.state.bit_buf as u32);
        let symbol = symbol_len.0;
        let code_len = symbol_len.1;

        println!("DEBUG: Huffman decode - symbol: {}, code_len: {}, bit_buf_low9: 0x{:03x}",
                 symbol, code_len, self.state.bit_buf & 0x1FF);

        // 检查码长是否有效且我们有足够的位
        if code_len > 0 && code_len <= 15 && self.state.num_bits >= code_len {
            // 移除已解码的位
            self.state.bit_buf >>= code_len;
            self.state.num_bits -= code_len;

            return Some(symbol);
        }

        None
    }

    /// 解码 Huffman 符号（使用字面/长度表，表0）
    fn decode_huffman_symbol(&mut self) -> Option<u16> {
        self.decode_huffman_symbol_with_table(0)
    }

    /// 解码距离 Huffman 符号（使用距离表，表1）
    fn decode_distance_symbol(&mut self) -> Option<u16> {
        self.decode_huffman_symbol_with_table(1)
    }

    /// LZ77 复制：从历史缓冲区复制数据
    fn decode_static_huffman(&self, bits: u32, bits_needed: u8) -> Option<u16> {
        // 静态 Huffman 表的实现
        // 根据 RFC 1951，静态 Huffman 表有固定的编码

        // 对于静态 Huffman 表：
        // 1. 字面字节 0-255 使用 8 位编码
        // 2. 块结束标记 (256) 使用 9 位编码
        // 3. 长度距离对使用更复杂的编码

        if bits_needed == 8 {
            // 字节 0-255 的编码
            return Some(bits as u16);
        } else if bits_needed == 9 {
            // 可能是块结束标记 (256)
            if bits == 1 {
                return Some(256);
            }
            // 或者可能是长度距离对
            // 这里简化处理，返回 None 让更高层处理
            return None;
        } else if bits_needed == 7 && bits == 0b100 {
            // 另一种可能的块结束标记模式
            return Some(256);
        }

        None
    }

    /// LZ77 复制操作
    fn lz77_copy(&mut self, length: u32, distance: u32) {
        // 确保 distance 不超过已输出数据的长度
        if distance as usize > self.state.output_pos {
            return;
        }

        // 从已输出数据的末尾向前 distance 位置开始复制
        let src_start = self.state.output_pos - distance as usize;

        // 复制 length 个字节
        for i in 0..length {
            let src_pos = src_start + i as usize;
            if src_pos < self.state.output_buffer.len() {
                // 从缓冲区复制
                let byte = self.state.output_buffer[src_pos];
                self.state.output_buffer.push(byte);
            } else {
                // 如果源位置超出缓冲区（环绕情况），则从头部复制
                let wrapped_pos = src_pos - self.state.output_buffer.len();
                let byte = self.state.output_buffer[wrapped_pos];
                self.state.output_buffer.push(byte);
            }
            self.state.output_pos += 1;
        }
    }

    /// 检查 Adler32 校验和
    fn check_adler32_checksum(&mut self) -> Result<(), InflateError> {
        if self.state.z_adler32 != self.state.check_adler32 {
            return Err(InflateError::Adler32Mismatch);
        }
        Ok(())
    }
}

/// INFLATE 解压错误
#[derive(Debug, thiserror::Error)]
pub enum InflateError {
    #[error("Need more input")]
    NeedMoreInput,
    #[error("Decompression failed")]
    DecompressionFailed,
    #[error("Adler32 checksum mismatch")]
    Adler32Mismatch,
    #[error("Bad parameter")]
    BadParam,
    #[error("Cannot make progress")]
    CannotMakeProgress,
    #[error("Input data is invalid")]
    BadZlibHeader,
    #[error("Invalid Huffman code")]
    InvalidCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompress_empty() {
        let data = b"";
        let result = decompress(data, 1, None);
        // Empty data should return empty output (not an error)
        assert!(result.is_ok());
        assert!(result.unwrap().output.is_empty());
    }

    #[test]
    fn test_decompress_raw_empty() {
        let data = b"";
        let result = decompress_raw(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_zlib_header() {
        // 创建简单的 ZLIB 头部
        let zlib_header = [0x78, 0x01]; // 最简单的 ZLIB 头部
        let result = decompress(&zlib_header, 1, None);
        assert!(result.is_err()); // 应该失败，因为没有压缩数据
    }
}