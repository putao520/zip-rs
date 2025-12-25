//! miniz复刻 - DEFLATE/INFLATE/CRC32 算法
//!
//! 这个模块复刻了 miniz 的核心压缩算法，不依赖外部库。

pub mod crc32;
pub mod deflate;
pub mod deflate_fast;
pub mod inflate;
pub mod huffman;
pub mod bitstream;
pub mod lz77;

pub use crc32::{crc32, Crc32};
pub use deflate::{compress, compress_raw, compress_to_buffer, CompressResult, DeflateEncoder, DeflateOptions};
pub use inflate::{decompress, decompress_to_buffer, decompress_raw, DecompressResult, InflateDecoder};
pub use huffman::{
    HuffmanTable, LENGTH_BASE, LENGTH_EXTRA, DIST_BASE, DIST_EXTRA,
    FIXED_LITLEN_CODE_LENGTHS, FIXED_DISTANCE_CODE_LENGTHS,
};
pub use bitstream::BitReader;

/// Adler32 校验和计算
pub fn adler32(adler: u32, data: &[u8]) -> u32 {
    const ADLER_MOD: u32 = 65521;

    let mut s1 = adler & 0xFFFF;
    let mut s2 = (adler >> 16) & 0xFFFF;

    // Process in blocks of 5552 bytes (the largest N such that 255*N(N+1)/2 < 2^32)
    let mut remaining = data;
    while !remaining.is_empty() {
        let block_len = remaining.len().min(5552);
        let block = &remaining[..block_len];

        // Process 8 bytes at a time
        let mut i = 0;
        while i + 8 <= block.len() {
            s1 += block[i] as u32;
            s2 += s1;
            s1 += block[i + 1] as u32;
            s2 += s1;
            s1 += block[i + 2] as u32;
            s2 += s1;
            s1 += block[i + 3] as u32;
            s2 += s1;
            s1 += block[i + 4] as u32;
            s2 += s1;
            s1 += block[i + 5] as u32;
            s2 += s1;
            s1 += block[i + 6] as u32;
            s2 += s1;
            s1 += block[i + 7] as u32;
            s2 += s1;
            i += 8;
        }

        // Process remaining bytes
        while i < block.len() {
            s1 += block[i] as u32;
            s2 += s1;
            i += 1;
        }

        s1 %= ADLER_MOD;
        s2 %= ADLER_MOD;
        remaining = &remaining[block_len..];
    }

    (s2 << 16) | s1
}

/// Adler32 初始值
pub const ADLER32_INIT: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adler32() {
        let data = b"Hello world!";
        let result = adler32(ADLER32_INIT, data);
        // Verified with Python zlib.adler32 and miniz.c
        assert_eq!(result, 0x1D09045E);
    }

    #[test]
    fn test_adler32_empty() {
        let result = adler32(ADLER32_INIT, &[]);
        assert_eq!(result, ADLER32_INIT);
    }
}
