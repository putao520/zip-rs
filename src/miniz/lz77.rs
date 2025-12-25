//! LZ77 压缩算法实现
//!
//! 参考 `../src/miniz.c` 中的 `tdefl_compress_fast()` 实现

use std::collections::HashMap;

/// LZ77 匹配结果
#[derive(Debug, Clone, Copy)]
pub struct Match {
    /// 匹配长度（最小3，最大258）
    pub length: u16,
    /// 匹配距离（1-32768）
    pub distance: u16,
}

/// LZ77 压缩器
pub struct LZ77Encoder {
    /// 滑动窗口字典（32KB）
    dict: Vec<u8>,
    /// 字典中有效数据的大小
    dict_size: usize,
    /// 当前写入位置
    dict_pos: usize,
    /// 哈希表：3字节序列 -> 位置
    hash_table: HashMap<u32, Vec<usize>>,
    /// 窗口大小
    window_size: usize,
    /// 最大匹配长度
    max_match_len: usize,
    /// 最小匹配长度
    min_match_len: usize,
}

impl LZ77Encoder {
    /// 创建新的 LZ77 编码器
    pub fn new() -> Self {
        Self {
            dict: vec![0; 32 * 1024], // 32KB 滑动窗口
            dict_size: 0,
            dict_pos: 0,
            hash_table: HashMap::new(),
            window_size: 32 * 1024,
            max_match_len: 258,  // DEFLATE 最大匹配长度
            min_match_len: 3,    // DEFLATE 最小匹配长度
        }
    }

    /// 添加数据到字典
    fn add_to_dict(&mut self, data: &[u8]) {
        for &byte in data {
            self.dict[self.dict_pos] = byte;
            self.dict_pos = (self.dict_pos + 1) % self.window_size;
            if self.dict_size < self.window_size {
                self.dict_size += 1;
            }
        }
    }

    /// 计算3字节序列的哈希
    fn hash_trigram(data: &[u8], pos: usize) -> u32 {
        if pos + 2 >= data.len() {
            return 0;
        }
        (data[pos] as u32) | ((data[pos + 1] as u32) << 8) | ((data[pos + 2] as u32) << 16)
    }

    /// 查找最长匹配
    fn find_match(&self, data: &[u8], pos: usize) -> Option<Match> {
        if pos + self.min_match_len > data.len() {
            return None;
        }

        let hash = Self::hash_trigram(data, pos);
        if let Some(candidates) = self.hash_table.get(&hash) {
            let mut best_match: Option<Match> = None;

            for &candidate_pos in candidates {
                // 计算距离
                let distance = pos - candidate_pos;
                if distance > 32768 {
                    continue; // 超出最大距离
                }

                // 检查是否匹配
                let match_len = self.compare(data, candidate_pos, pos);
                if match_len >= self.min_match_len {
                    if best_match.is_none() || match_len > best_match.unwrap().length as usize {
                        best_match = Some(Match {
                            length: match_len as u16,
                            distance: distance as u16,
                        });

                        // 找到最大匹配就停止
                        if match_len >= self.max_match_len {
                            break;
                        }
                    }
                }
            }

            best_match
        } else {
            None
        }
    }

    /// 比较两个位置的字符串，返回匹配长度
    fn compare(&self, data: &[u8], pos1: usize, pos2: usize) -> usize {
        let max_len = std::cmp::min(self.max_match_len, data.len() - pos2);
        let mut len = 0;

        while len < max_len {
            // 从字典读取可能的历史数据
            let byte1 = if pos1 + len < data.len() {
                data[pos1 + len]
            } else {
                // 尝试从字典读取
                let dict_idx = (pos1 + len) % self.window_size;
                self.dict[dict_idx]
            };

            let byte2 = data[pos2 + len];

            if byte1 != byte2 {
                break;
            }

            len += 1;
        }

        len
    }

    /// 压缩数据，返回LZ编码序列
    /// 返回：(字面量字节, 匹配)
    pub fn compress(&mut self, data: &[u8]) -> Vec<LZSymbol> {
        let mut symbols = Vec::new();
        let mut pos = 0;

        // 构建哈希表（第一遍扫描）
        for i in 0..data.len().saturating_sub(2) {
            let hash = Self::hash_trigram(data, i);
            self.hash_table.entry(hash).or_insert_with(Vec::new).push(i);
        }

        while pos < data.len() {
            // 查找匹配
            if let Some(m) = self.find_match(data, pos) {
                // 找到匹配，使用长度/距离对
                symbols.push(LZSymbol::Match(m));
                self.add_to_dict(&data[pos..pos + m.length as usize]);
                pos += m.length as usize;
            } else {
                // 没有匹配，输出字面量
                symbols.push(LZSymbol::Literal(data[pos]));
                self.add_to_dict(&data[pos..pos + 1]);
                pos += 1;
            }
        }

        symbols
    }
}

impl Default for LZ77Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// LZ77 符号：字面量或匹配
#[derive(Debug, Clone)]
pub enum LZSymbol {
    /// 字面量字节（0-255）
    Literal(u8),
    /// 长度/距离对
    Match(Match),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_trigram() {
        let data = b"ABCDEFGH";
        assert_eq!(LZ77Encoder::hash_trigram(data, 0), 0x00434241); // "ABC"
        assert_eq!(LZ77Encoder::hash_trigram(data, 1), 0x00444342); // "BCD"
    }

    #[test]
    fn test_repeat_pattern() {
        let data: Vec<u8> = (0..=255).cycle().take(1024).collect();
        let mut encoder = LZ77Encoder::new();
        let symbols = encoder.compress(&data);

        // 重复模式应该产生大量匹配
        let match_count = symbols.iter().filter(|s| matches!(s, LZSymbol::Match(_))).count();
        println!("Match count: {}, total symbols: {}", match_count, symbols.len());

        // 第一轮256字节是字面量，之后应该有大量匹配
        assert!(match_count > 0, "Should find matches in repeated pattern");
    }
}
