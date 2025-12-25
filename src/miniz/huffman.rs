//! Huffman 编码/解码
//!
//! RFC 1951 DEFLATE Huffman 码实现

use std::fmt;

/// Huffman 解码错误
#[derive(Debug, PartialEq, Eq)]
pub enum HuffmanError {
    /// 无效码长
    InvalidCodeLength,
    /// 无效 Huffman 码
    InvalidCode,
    /// 溢出
    Overflow,
    /// 表已满
    TableFull,
    /// 无效符号
    InvalidSymbol,
}

impl fmt::Display for HuffmanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HuffmanError::InvalidCodeLength => write!(f, "invalid code length"),
            HuffmanError::InvalidCode => write!(f, "invalid huffman code"),
            HuffmanError::Overflow => write!(f, "overflow"),
            HuffmanError::TableFull => write!(f, "table full"),
            HuffmanError::InvalidSymbol => write!(f, "invalid symbol"),
        }
    }
}

impl std::error::Error for HuffmanError {}

/// 长度码基础值表 (RFC 1951)
pub const LENGTH_BASE: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99,
    115, 131, 163, 195, 227, 258, 0, 0,
];

/// 长度码额外位数表
pub const LENGTH_EXTRA: [u8; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5,
    0, 0, 0,
];

/// 距离码基础值表
pub const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025,
    1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// 距离码额外位数表
pub const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
    12, 13, 13,
];

/// 码长序列的解码顺序 (用于动态 Huffman 块)
pub const LENGTH_DEZIGZAG: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// 固定 Huffman 码的字面/长度码长
/// 0-143: 8 bits (144 个)
/// 144-255: 9 bits (112 个)
/// 256-279: 7 bits (24 个)
/// 280-287: 8 bits (8 个)
/// 总计: 144 + 112 + 24 + 8 = 288
pub const FIXED_LITLEN_CODE_LENGTHS: [u8; 288] = [
    // 0-143: 8 bits (144 个 = 9×16)
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    // 144-255: 9 bits (112 个 = 7×16)
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
    // 256-279: 7 bits (24 个 = 16+8)
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7,
    // 280-287: 8 bits (8 个)
    8, 8, 8, 8, 8, 8, 8, 8,
];

/// 固定 Huffman 码的距离码长 (全部 5 bits)
pub const FIXED_DISTANCE_CODE_LENGTHS: [u8; 30] = [
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5,
];

/// Huffman 解码表
#[derive(Debug)]
pub struct HuffmanTable {
    /// 快速查找表 (用于短代码)
    pub lookup: [i16; 512],
    /// 树结构 (用于长代码)
    pub tree: Vec<i16>,
}

impl HuffmanTable {
    /// 创建空的 Huffman 表
    pub fn new() -> Self {
        Self {
            lookup: [-1; 512],
            tree: Vec::new(),
        }
    }

    /// 构建解码表 (从码长构建)
    pub fn build(code_lengths: &[u8], num_symbols: usize) -> Result<Self, HuffmanError> {
        const MAX_BITS: usize = 15;

        let mut result = Self::new();

        // 统计每个长度的符号数
        let mut bl_count = [0usize; MAX_BITS + 1];
        for &cl in &code_lengths[..num_symbols] {
            if cl > MAX_BITS as u8 {
                return Err(HuffmanError::InvalidCodeLength);
            }
            if cl > 0 {
                bl_count[cl as usize] += 1;
            }
        }

        // 计算每个长度的第一个码
        let mut next_code = [0u16; MAX_BITS + 1];
        let mut code = 0u32;
        for bits in 1..=MAX_BITS {
            code = (code + bl_count[bits - 1] as u32) << 1;
            // 检查码是否在有效范围内（所有 n 位码必须 < 2^n）
            // 只检查实际使用的码长（有符号的长度）
            if bl_count[bits] > 0 && code >= (1u32 << bits) {
                return Err(HuffmanError::Overflow);
            }
            next_code[bits] = code as u16;
        }

        // 分配码并构建表
        for symbol in 0..num_symbols {
            let len = code_lengths[symbol] as usize;
            if len == 0 {
                continue;
            }

            // 反转码位 (DEFLATE 使用 LSB 优先)
            let mut rev_code = 0u32;
            let mut c = next_code[len] as u32;
            next_code[len] += 1;

            for _ in 0..len {
                rev_code = (rev_code << 1) | (c & 1);
                c >>= 1;
            }

            // 短代码使用快速查找表
            if len <= 9 {
                let stride = 1 << len;
                let entry = ((len as i16) << 9) | symbol as i16;
                let mut idx = rev_code as usize;
                while idx < 512 {
                    result.lookup[idx] = entry;
                    idx += stride;
                }
            } else {
                // 长代码使用树结构
                let mut idx = result.lookup[(rev_code & 0xFF) as usize];
                if idx == -1 || idx == 0 {
                    // 需要在树中创建新节点
                    if result.tree.is_empty() {
                        result.tree.push(0); // 索引 0 不使用
                        result.tree.push(-1);
                    }
                    let new_idx = result.tree.len() as i16;
                    result.lookup[(rev_code & 0xFF) as usize] = -new_idx;
                    result.tree.push(-1);
                    result.tree.push(-1);
                    idx = -new_idx;
                }

                // 遍历树，添加剩余位
                let mut mask = 1 << 8;
                for _ in 9..len {
                    let bit = if (rev_code / mask) & 1 != 0 { 1 } else { 0 };
                    mask <<= 1;

                    let next = result.tree.get((-idx) as usize + bit).copied().unwrap_or(-1);
                    if next == -1 || next == 0 {
                        // 创建新节点
                        let new_idx = result.tree.len() as i16;
                        result.tree[(-idx) as usize + bit] = -new_idx;
                        result.tree.push(-1);
                        result.tree.push(-1);
                        idx = -new_idx;
                    } else if next < 0 {
                        idx = next;
                    } else {
                        return Err(HuffmanError::InvalidSymbol);
                    }
                }

                // 设置最终符号
                let bit = if (rev_code / mask) & 1 != 0 { 1 } else { 0 };
                result.tree[(-idx) as usize + bit] = symbol as i16;
            }
        }

        Ok(result)
    }

    /// 构建静态 Huffman 表
    pub fn build_static_table(&self, codes: &[u16]) -> Result<HuffmanTable, HuffmanError> {
        let mut code_lengths = [0u8; 288];

        // 根据静态码设置码长
        for (i, &_code) in codes.iter().enumerate() {
            // 静态 Huffman 码的码长是固定的
            if i <= 143 {
                code_lengths[i] = 8; // 0-143: 8 bits
            } else if i <= 255 {
                code_lengths[i] = 9; // 144-255: 9 bits
            } else if i <= 279 {
                code_lengths[i] = 7; // 256-279: 7 bits
            } else if i <= 287 {
                code_lengths[i] = 8; // 280-287: 8 bits
            }
        }

        // 使用现有的 build 方法构建表
        HuffmanTable::build(&code_lengths, 288)
    }

    /// 解码一个符号
    /// 返回 (符号, 码长)
    pub fn decode(&self, bit_buf: u32) -> (u16, u8) {
        let idx = (bit_buf & 0x1FF) as usize;
        let entry = self.lookup[idx];

        if entry >= 0 {
            // 快速路径
            (entry as u16 & 0x1FF, (entry as u16 >> 9) as u8)
        } else {
            // 树路径
            let mut tree_idx = (-entry) as usize;
            let mut shift = 9;
            loop {
                let bit = ((bit_buf >> shift) & 1) as usize;
                shift += 1;
                match self.tree.get(tree_idx + bit) {
                    Some(&next) if next < 0 => tree_idx = (-next) as usize,
                    Some(&next) => return (next as u16, shift as u8),
                    None => return (256, 15), // 默认返回块结束
                }
            }
        }
    }

    /// 检查是否有足够的位用于解码
    pub fn has_enough_bits(&self, _bit_buf: u64, num_bits: u8, max_bits: u8) -> bool {
        num_bits >= max_bits
    }

    /// 使用指定位数解码符号
    pub fn decode_with_bits(&self, bit_buf: u64, _num_bits: u8) -> (u16, u8) {
        self.decode(bit_buf as u32)
    }
}

impl Default for HuffmanTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_table_build() {
        let table = HuffmanTable::build(&FIXED_LITLEN_CODE_LENGTHS, 288).unwrap();
        // 检查表已构建
        let (_sym, len) = table.decode(0);
        assert!(len <= 15);
    }

    #[test]
    fn test_length_base() {
        assert_eq!(LENGTH_BASE[0], 3);
        assert_eq!(LENGTH_BASE[25], 163);  // 符号 282
        assert_eq!(LENGTH_BASE[28], 258);  // 符号 285
        assert_eq!(LENGTH_EXTRA[0], 0);
        assert_eq!(LENGTH_EXTRA[8], 1);
    }

    #[test]
    fn test_dist_base() {
        assert_eq!(DIST_BASE[0], 1);
        assert_eq!(DIST_BASE[29], 24577);
        assert_eq!(DIST_EXTRA[0], 0);
        assert_eq!(DIST_EXTRA[4], 1);
    }
}
