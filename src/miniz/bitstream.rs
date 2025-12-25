//! 位流读取器和写入器
//!
//! 提供按位读取和写入字节数据的功能

/// 位写入器
#[derive(Debug)]
pub struct BitWriter {
    /// 数据缓冲区
    data: Vec<u8>,
    /// 位缓冲区
    bit_buf: u64,
    /// 缓冲区中的位数
    num_bits: u8,
}

impl BitWriter {
    /// 创建新的位写入器
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// 从字节数据创建
    pub fn from_bytes(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// 写入 n 位 (小端序)
    #[inline]
    pub fn write_bits(&mut self, bits: u32, n: u8) {
        if n == 0 {
            return;
        }

        self.bit_buf |= (bits as u64) << self.num_bits;
        self.num_bits += n;

        // 每 8 位写入一个字节
        while self.num_bits >= 8 {
            let byte = (self.bit_buf & 0xFF) as u8;
            self.data.push(byte);
            self.bit_buf >>= 8;
            self.num_bits -= 8;
        }
    }

    /// 写入单个位
    #[inline]
    pub fn write_bit(&mut self, bit: bool) {
        self.write_bits(bit as u32, 1);
    }

    /// 写入单个字节
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        self.write_bits(byte as u32, 8);
    }

    /// 写入多个字节
    #[inline]
    pub fn write_bytes(&mut self, data: &[u8]) {
        for &byte in data {
            self.write_bits(byte as u32, 8);
        }
    }

    /// 对齐到字节边界
    #[inline]
    pub fn align_to_byte(&mut self) {
        let bits_to_pad = 8 - (self.num_bits % 8);
        if bits_to_pad < 8 {
            self.write_bits(0, bits_to_pad);
        }
    }

    /// 获取写入的字节数
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 获取数据
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.align_to_byte();
        self.data
    }

    /// 获取数据引用
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.data.clear();
        self.bit_buf = 0;
        self.num_bits = 0;
    }

    /// 获取当前缓冲区的位数
    pub fn buffered_bits(&self) -> u8 {
        self.num_bits
    }

    /// 获取当前缓冲区的字节数
    pub fn buffered_bytes(&self) -> usize {
        self.data.len() + (self.num_bits / 8) as usize
    }
}

/// 位流读取器
#[derive(Debug)]
pub struct BitReader {
    /// 数据引用
    data: Vec<u8>,
    /// 当前位置 (字节)
    pos: usize,
    /// 位缓冲区
    bit_buf: u64,
    /// 缓冲区中的有效位数
    num_bits: u8,
}

impl BitReader {
    /// 创建新的位读取器
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            pos: 0,
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// 从引用创建
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            pos: 0,
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// 确保缓冲区有足够的位
    #[inline]
    fn fill_bits(&mut self, n: u8) {
        while self.num_bits < n {
            if self.pos >= self.data.len() {
                // 没有更多数据，填充 0
                self.bit_buf |= 0u64 << self.num_bits;
                break;
            } else {
                let byte = self.data[self.pos] as u64;
                self.pos += 1;
                self.bit_buf |= byte << self.num_bits;
                self.num_bits += 8;
            }
        }
    }

    /// 读取 n 位 (小端序)
    #[inline]
    pub fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if n > 32 {
            return None;
        }

        self.fill_bits(n);

        if self.num_bits < n {
            return None; // 没有足够的位
        }

        let result = (self.bit_buf & ((1u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.num_bits -= n;

        Some(result)
    }

    /// 读取 n 位，如果失败返回错误
    #[inline]
    pub fn read_bits_checked(&mut self, n: u8) -> Result<u32, ()> {
        self.read_bits(n).ok_or(())
    }

    /// 读取单个位
    #[inline]
    pub fn read_bit(&mut self) -> Option<bool> {
        self.fill_bits(1);

        if self.num_bits < 1 {
            return None;
        }

        let result = (self.bit_buf & 1) != 0;
        self.bit_buf >>= 1;
        self.num_bits -= 1;

        Some(result)
    }

    /// 读取单个字节 (对齐)
    #[inline]
    pub fn read_byte(&mut self) -> Option<u8> {
        // 跳过剩余的位以到达字节边界
        let num_bits = self.num_bits % 8;
        if num_bits > 0 {
            println!("DEBUG: read_byte skipping {} bits at position {}", num_bits, self.pos);
            self.bit_buf >>= num_bits;
            self.num_bits -= num_bits;
        }

        // 现在我们在字节边界，可以读取下一个字节
        if self.pos < self.data.len() {
            let result = self.data[self.pos];
            println!("DEBUG: read_byte reading byte {} from position {}", result, self.pos);
            self.pos += 1;
            Some(result)
        } else {
            println!("DEBUG: read_byte no more data at position {}", self.pos);
            None
        }
    }

    /// 读取多个字节
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<(), ()> {
        // 先消耗位缓冲区中的完整字节
        while self.num_bits >= 8 && !buf.is_empty() {
            buf[0] = (self.bit_buf & 0xFF) as u8;
            self.bit_buf >>= 8;
            self.num_bits -= 8;
            let _rest = &mut buf[1..];
            // 递归处理剩余缓冲区
            // 但避免递归深度问题，改用迭代
        }

        // 直接从数据复制
        let remaining = &self.data[self.pos..];
        if remaining.len() < buf.len() {
            return Err(());
        }

        let copy_len = buf.len().min(remaining.len());
        buf[..copy_len].copy_from_slice(&remaining[..copy_len]);
        self.pos += copy_len;

        Ok(())
    }

    /// 对齐到字节边界
    pub fn align_to_byte(&mut self) {
        let bits_to_skip = self.num_bits % 8;
        if bits_to_skip > 0 {
            self.bit_buf >>= bits_to_skip;
            self.num_bits -= bits_to_skip;
        }
    }

    /// 查看当前位缓冲区 (不移除)
    #[inline]
    pub fn peek_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if n > 32 {
            return None;
        }

        self.fill_bits(n);

        if self.num_bits < n {
            return None;
        }

        Some((self.bit_buf & ((1u64 << n) - 1)) as u32)
    }

    /// 跳过 n 位
    #[inline]
    pub fn skip_bits(&mut self, n: u8) {
        if n == 0 {
            return;
        }

        // 如果缓冲区中位数不足，先加载字节
        while self.num_bits < n && self.pos < self.data.len() {
            let byte = self.data[self.pos] as u64;
            self.pos += 1;
            self.bit_buf |= byte << self.num_bits;
            self.num_bits += 8;
        }

        // 丢弃 n 位（右移）
        self.bit_buf >>= n;
        self.num_bits -= n;
    }

    /// 获取当前位缓冲区
    pub fn get_bit_buf(&self) -> u64 {
        self.bit_buf
    }

    /// 获取当前有效位数
    pub fn get_num_bits(&self) -> u8 {
        self.num_bits
    }

    /// 检查是否有足够的位
    pub fn has_more_bits(&self, n: u8) -> bool {
        if n == 0 {
            return true;
        }
        if n > self.num_bits {
            // 估算是否还有足够的数据
            let needed_bytes = ((n - self.num_bits + 7) / 8) as usize;
            self.pos + needed_bytes <= self.data.len()
        } else {
            true
        }
    }

    /// 检查是否还有更多字节
    pub fn has_more_bytes(&self, n: usize) -> bool {
        self.pos + n <= self.data.len()
    }

    /// 获取当前位置
    pub fn get_pos(&self) -> usize {
        self.pos
    }

    /// 设置当前位置
    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos.min(self.data.len());
        if self.pos < self.data.len() {
            self.fill_bits(1); // 触发重新填充位缓冲区
        }
    }

    /// 获取当前位置 (字节)
    pub fn position(&self) -> usize {
        self.pos - (self.num_bits / 8) as usize
    }

    /// 获取剩余字节数
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position())
    }

    /// 检查是否还有数据
    pub fn has_more(&self) -> bool {
        self.pos < self.data.len() || self.num_bits > 0
    }

    /// 获取当前缓冲区的位数
    pub fn buffered_bits(&self) -> u8 {
        self.num_bits
    }

    /// 获取数据引用
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits() {
        let data = vec![0b10110010, 0b01101001];
        let mut reader = BitReader::new(data);

        assert_eq!(reader.read_bits(4), Some(0b0010)); // LSB 优先
        assert_eq!(reader.read_bits(4), Some(0b1011));
        assert_eq!(reader.read_bits(8), Some(0b01101001));
        assert_eq!(reader.read_bits(1), None);
    }

    #[test]
    fn test_read_bit() {
        let data = vec![0b10110010];
        let mut reader = BitReader::new(data);

        assert_eq!(reader.read_bit(), Some(false)); // bit 0 = 0
        assert_eq!(reader.read_bit(), Some(true));  // bit 1 = 1
        assert_eq!(reader.read_bit(), Some(false)); // bit 2 = 0
        assert_eq!(reader.read_bit(), Some(false)); // bit 3 = 0
        assert_eq!(reader.read_bit(), Some(true));  // bit 4 = 1
        assert_eq!(reader.read_bit(), Some(true));  // bit 5 = 1
        assert_eq!(reader.read_bit(), Some(false)); // bit 6 = 0
        assert_eq!(reader.read_bit(), Some(true));  // bit 7 = 1
    }

    #[test]
    fn test_peek_bits() {
        let data = vec![0b11110000, 0b00001111];
        let mut reader = BitReader::new(data);

        // 查看但不移除
        assert_eq!(reader.peek_bits(4), Some(0b0000));
        assert_eq!(reader.peek_bits(4), Some(0b0000));

        // 读取后查看
        assert_eq!(reader.read_bits(4), Some(0b0000));
        assert_eq!(reader.peek_bits(4), Some(0b1111));
    }

    #[test]
    fn test_skip_bits() {
        let data = vec![0b11110000, 0b10101010];
        let mut reader = BitReader::new(data);

        // 跳过 4 位 (0b0000), 读取 4 位 (0b1111)
        reader.skip_bits(4);
        assert_eq!(reader.read_bits(4), Some(0b1111));

        // 此时 byte 0 已完全消费，跳过 byte 1 的 8 位
        reader.skip_bits(8);
        // byte 1 已被跳过，无数据可读
        assert_eq!(reader.read_bits(8), None);
    }

    #[test]
    fn test_align_to_byte() {
        let data = vec![0b11110000, 0b10101010];
        let mut reader = BitReader::new(data);

        reader.read_bits(3).unwrap();
        reader.align_to_byte();

        // 应该对齐到第二个字节
        assert_eq!(reader.read_bits(8), Some(0b10101010));
    }
}
