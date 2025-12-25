//! DEFLATE 快速压缩 - 直接复刻 C 版本 tdefl_compress_fast()
//!
//! 复刻 /home/putao/code/c-cpp/zip/src/miniz.c 的 LZ77 压缩算法

use crate::miniz::bitstream::BitWriter;

// 常量定义（完全对应 C 版本）
const TDEFL_LZ_DICT_SIZE: usize = 32768;
const TDEFL_LZ_DICT_SIZE_MASK: usize = TDEFL_LZ_DICT_SIZE - 1;
const TDEFL_MIN_MATCH_LEN: usize = 3;
const TDEFL_MAX_MATCH_LEN: usize = 258;
const TDEFL_LEVEL1_HASH_BITS: usize = 12;
const TDEFL_LEVEL1_HASH_SIZE_MASK: usize = 4095;
const TDEFL_COMP_FAST_LOOKAHEAD_SIZE: usize = 4096;

/// LZ77 快速压缩器
pub struct DeflateFast {
    /// 滑动窗口字典
    dict: Vec<u8>,
    /// 哈希表（3字节trigram -> 位置）
    hash: Vec<u16>,
}

impl DeflateFast {
    /// 创建新的压缩器
    pub fn new() -> Self {
        Self {
            // 字典大小 = TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1
            dict: vec![0; TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1],
            hash: vec![0; 4096], // TDEFL_LEVEL1_HASH_SIZE_MASK + 1
        }
    }

    /// 读取3字节trigram（小端）
    #[inline(always)]
    fn read_trigram(dict: &[u8], pos: usize) -> u32 {
        let p = &dict[pos..pos + 4];
        (p[0] as u32) | ((p[1] as u32) << 8) | ((p[2] as u32) << 16)
    }

    /// 比较16位对（用于快速匹配）
    #[inline(always)]
    fn compare_u16(p: &[u8], q: &[u8]) -> bool {
        p[0] as u16 | ((p[1] as u16) << 8) == q[0] as u16 | ((q[1] as u16) << 8)
    }

    /// 压缩数据，返回LZ编码序列
    /// 返回：(字面量字节, 长度/距离对)
    pub fn compress(&mut self, data: &[u8]) -> Vec<LZSymbol> {
        let mut symbols = Vec::new();

        if data.is_empty() {
            return symbols;
        }

        // 初始化状态（完全对应C版本）
        let mut lookahead_pos = 0usize;
        let mut lookahead_size = 0usize;
        let mut dict_size = 0usize;
        let mut src_buf = data;

        // 清空哈希表
        self.hash.fill(0);

        let mut cur_pos = lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;

        // 主压缩循环 - 使用lookahead增量处理（对应C版本line 1465-1615）
        while !src_buf.is_empty() || lookahead_size > 0 {
            // 复制数据到lookahead缓冲区（最多4096字节）
            let dst_pos = (lookahead_pos + lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK;
            let num_bytes_to_process = (TDEFL_COMP_FAST_LOOKAHEAD_SIZE - lookahead_size)
                .min(src_buf.len());

            if num_bytes_to_process > 0 {
                let src = &src_buf[..num_bytes_to_process];

                // 复制到字典（循环缓冲区）
                let mut n = (TDEFL_LZ_DICT_SIZE - dst_pos).min(num_bytes_to_process);
                self.dict[dst_pos..dst_pos + n].copy_from_slice(&src[..n]);

                // 处理边界（如果接近最大匹配长度）
                if dst_pos < (TDEFL_MAX_MATCH_LEN - 1) {
                    let overlap = n.min((TDEFL_MAX_MATCH_LEN - 1) - dst_pos);
                    self.dict[TDEFL_LZ_DICT_SIZE..TDEFL_LZ_DICT_SIZE + overlap]
                        .copy_from_slice(&src[..overlap]);
                }

                if n < num_bytes_to_process {
                    // 需要绕回缓冲区开头
                    let remaining = &src[n..];
                    let n2 = remaining.len();
                    self.dict[..n2].copy_from_slice(remaining);
                }

                src_buf = &src_buf[num_bytes_to_process..];
                lookahead_size += num_bytes_to_process;
            }

            // 更新字典大小（对应C版本line 1484）
            dict_size = (TDEFL_LZ_DICT_SIZE - lookahead_size).min(dict_size);

            // 如果lookahead不够大且还有数据，继续填充
            if !src_buf.is_empty() && lookahead_size < TDEFL_COMP_FAST_LOOKAHEAD_SIZE {
                continue;
            }

            // 主压缩循环（对应C版本line 1488-1575）
            while lookahead_size >= 4 {
                let cur_match_dist;
                let mut cur_match_len = 1;

                let p_cur_dict = &self.dict[cur_pos..];
                let first_trigram = Self::read_trigram(p_cur_dict, 0) & 0xFFFFFF;

                // 计算哈希（完全对应C版本）
                let hash = (first_trigram ^ (first_trigram >> (24 - (TDEFL_LEVEL1_HASH_BITS - 8))))
                    & TDEFL_LEVEL1_HASH_SIZE_MASK as u32;

                let probe_pos = self.hash[hash as usize] as usize;
                self.hash[hash as usize] = lookahead_pos as u16;

                // 检查是否找到匹配
                cur_match_dist = lookahead_pos - probe_pos;

                if cur_match_dist <= dict_size
                    && cur_match_dist > 0
                    && (Self::read_trigram(&self.dict, probe_pos & TDEFL_LZ_DICT_SIZE_MASK) & 0xFFFFFF)
                        == first_trigram
                {
                    // 找到可能的匹配，验证并扩展
                    let probe_pos = probe_pos & TDEFL_LZ_DICT_SIZE_MASK;
                    let p = &self.dict[cur_pos..];
                    let q = &self.dict[probe_pos..];

                    // 快速比较（每次比较2字节）
                    let mut probe_len = 32u32;
                    let mut match_len = 0;

                    while match_len < lookahead_size
                        && match_len < TDEFL_MAX_MATCH_LEN
                        && probe_len > 0
                    {
                        if p[match_len] == q[match_len] {
                            match_len += 1;
                            if match_len % 2 == 0 {
                                probe_len -= 1;
                            }
                        } else {
                            break;
                        }
                    }

                    cur_match_len = match_len;

                    // 决定使用匹配还是字面量
                    if cur_match_len < TDEFL_MIN_MATCH_LEN
                        || (cur_match_len == TDEFL_MIN_MATCH_LEN && cur_match_dist >= 8 * 1024)
                    {
                        // 匹配太短或太远，使用字面量
                        symbols.push(LZSymbol::Literal(p_cur_dict[0]));
                        cur_match_len = 1;
                    } else {
                        // 使用匹配
                        cur_match_len = cur_match_len.min(lookahead_size);
                        symbols.push(LZSymbol::Match {
                            length: cur_match_len as u16,
                            distance: (cur_match_dist - 1) as u16,
                        });
                    }
                } else {
                    // 没有找到匹配，输出字面量
                    symbols.push(LZSymbol::Literal(p_cur_dict[0]));
                    cur_match_len = 1;
                }

                // 更新状态
                lookahead_pos += cur_match_len;
                dict_size = (dict_size + cur_match_len).min(TDEFL_LZ_DICT_SIZE);
                cur_pos = (cur_pos + cur_match_len) & TDEFL_LZ_DICT_SIZE_MASK;
                lookahead_size -= cur_match_len;
            }

            // 处理剩余字节（< 4字节）- 对应C版本line 1577-1614
            while lookahead_size > 0 {
                symbols.push(LZSymbol::Literal(self.dict[cur_pos]));
                lookahead_pos += 1;
                dict_size = (dict_size + 1).min(TDEFL_LZ_DICT_SIZE);
                cur_pos = (cur_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK;
                lookahead_size -= 1;
            }
        }

        symbols
    }
}

impl Default for DeflateFast {
    fn default() -> Self {
        Self::new()
    }
}

/// LZ77 符号
#[derive(Debug, Clone)]
pub enum LZSymbol {
    /// 字面量字节
    Literal(u8),
    /// 长度/距离对
    Match { length: u16, distance: u16 },
}

/// 使用LZ77 + 静态Huffman编码压缩数据
pub fn deflate_compress_fast(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut encoder = DeflateFast::new();
    let symbols = encoder.compress(data);

    // 使用BitWriter写入DEFLATE格式
    let mut bit_writer = BitWriter::new();

    // 块头 (BFINAL=1, BTYPE=01 静态Huffman)
    bit_writer.write_bits(0x03, 3);

    // 对每个符号进行Huffman编码
    for symbol in symbols {
        match symbol {
            LZSymbol::Literal(byte) => {
                // 静态Huffman编码（RFC 1951）
                let (code, code_len) = if byte <= 143 {
                    (0x30 + byte as u32, 8)
                } else {
                    (0x190 + (byte - 144) as u32, 9)
                };

                // 反转码字位序（MSB -> LSB）
                let reversed_code = reverse_bits(code, code_len);
                bit_writer.write_bits(reversed_code, code_len);
            }
            LZSymbol::Match { length, distance } => {
                // 编码长度
                let length_base = LENGTH_BASE_TABLE;

                let mut len_code = 0;
                let mut len_extra_bits: u8 = 0;
                let mut len_extra_val = 0;

                for i in 0..length_base.len() {
                    if length as usize >= length_base[i] && (i == length_base.len() - 1 || (length as usize) < length_base[i + 1]) {
                        len_code = 257 + i as u32;
                        len_extra_bits = LENGTH_EXTRA_TABLE[i];
                        len_extra_val = (length as usize - length_base[i]) as u32;
                        break;
                    }
                }

                // 编码长度
                let len_huffman = LENGTH_HUFFMAN[len_code as usize - 257];
                let reversed_code = reverse_bits(len_huffman.0 as u32, len_huffman.1);
                bit_writer.write_bits(reversed_code, len_huffman.1);
                if len_extra_bits > 0 {
                    bit_writer.write_bits(len_extra_val, len_extra_bits as u8);
                }

                // 编码距离
                let dist_base = DIST_BASE_TABLE;

                let mut dist_code = 0;
                let mut dist_extra_bits: u8 = 0;
                let mut dist_extra_val = 0;

                for i in 0..dist_base.len() {
                    if distance as usize >= dist_base[i] && (i == dist_base.len() - 1 || (distance as usize) < dist_base[i + 1]) {
                        dist_code = i as u32;
                        dist_extra_bits = DIST_EXTRA_TABLE[i];
                        dist_extra_val = (distance as usize - dist_base[i]) as u32;
                        break;
                    }
                }

                let dist_huffman = DIST_HUFFMAN[dist_code as usize];
                let reversed_code = reverse_bits(dist_huffman.0 as u32, dist_huffman.1);
                bit_writer.write_bits(reversed_code, dist_huffman.1);
                if dist_extra_bits > 0 {
                    bit_writer.write_bits(dist_extra_val, dist_extra_bits as u8);
                }
            }
        }
    }

    // 块结束标记（符号 256）
    bit_writer.write_bits(0x0000000, 7);

    // 对齐到字节边界
    bit_writer.align_to_byte();

    Ok(bit_writer.into_bytes())
}

/// 位反转
fn reverse_bits(code: u32, len: u8) -> u32 {
    let mut result = 0u32;
    for i in 0..len {
        result |= ((code >> i) & 1) << (len - 1 - i);
    }
    result
}

// DEFLATE 静态Huffman表（RFC 1951）
const LENGTH_BASE_TABLE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];

const LENGTH_EXTRA_TABLE: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const DIST_BASE_TABLE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

const DIST_EXTRA_TABLE: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

// 静态Huffman码表（MSB）
const LENGTH_HUFFMAN: [(u16, u8); 29] = [
    (0b0010000, 5),   // 257
    (0b11100000, 5),  // 258
    (0b0100000, 5),   // 259
    (0b11000000, 5),  // 260
    (0b1010000, 5),   // 261
    (0b0110000, 5),   // 262
    (0b0001000, 5),   // 263
    (0b1001000, 5),   // 264
    (0b0001010, 5),   // 265
    (0b1001010, 5),   // 266
    (0b1001100, 5),   // 267
    (0b1001110, 5),   // 268
    (0b1110100, 6),   // 269
    (0b1111100, 6),   // 270
    (0b1111101, 6),   // 271
    (0b11100100, 7),  // 272
    (0b11101100, 7),  // 273
    (0b11101101, 7),  // 274
    (0b11110100, 7),  // 275
    (0b11110101, 7),  // 276
    (0b11110110, 7),  // 277
    (0b11110111, 7),  // 278
    (0b111110010, 8), // 279
    (0b111110011, 8), // 280
    (0b111110100, 8), // 281
    (0b111110101, 8), // 282
    (0b111110110, 8), // 283
    (0b111110111, 8), // 284
    (0b111111000, 8), // 285
];

const DIST_HUFFMAN: [(u16, u8); 30] = [
    (0b00000, 5),     // 0
    (0b00001, 5),     // 1
    (0b00010, 5),     // 2
    (0b00011, 5),     // 3
    (0b00100, 5),     // 4
    (0b00101, 5),     // 5
    (0b00110, 5),     // 6
    (0b00111, 5),     // 7
    (0b01000, 5),     // 8
    (0b01001, 5),     // 9
    (0b01010, 5),     // 10
    (0b01011, 5),     // 11
    (0b01100, 5),     // 12
    (0b01101, 5),     // 13
    (0b01110, 5),     // 14
    (0b01111, 5),     // 15
    (0b10000, 5),     // 16
    (0b10001, 5),     // 17
    (0b10010, 5),     // 18
    (0b10011, 5),     // 19
    (0b10100, 5),     // 20
    (0b10101, 5),     // 21
    (0b10110, 5),     // 22
    (0b10111, 5),     // 23
    (0b11000, 5),     // 24
    (0b11001, 5),     // 25
    (0b11010, 5),     // 26
    (0b11011, 5),     // 27
    (0b11100, 5),     // 28
    (0b11101, 5),     // 29
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repeat_pattern() {
        // 重复模式：0-255 重复多次
        let data: Vec<u8> = (0..=255).cycle().take(10240).collect();

        let result = deflate_compress_fast(&data);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        println!("Original: {} bytes, Compressed: {} bytes", data.len(), compressed.len());
        println!("Compression ratio: {:.1}%", (compressed.len() as f64 / data.len() as f64) * 100.0);

        // 应该能够显著压缩重复模式
        assert!(compressed.len() < data.len() / 2, "Should compress repeated pattern");
    }
}
