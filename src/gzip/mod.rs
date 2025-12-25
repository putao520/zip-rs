//! GZIP-style compression helpers matching the C implementation.
//!
//! Note: The underlying C implementation uses zlib streams (miniz),
//! and we mirror its behavior for byte counts and sizing.

use crate::error::{Result, ZipError};
use crate::miniz::deflate::compress_to_buffer;
use crate::miniz::inflate::decompress_to_buffer;

#[derive(Debug, Clone)]
pub struct GzipOutput {
    pub output: Vec<u8>,
    pub bytes_read: usize,
    pub bytes_written: usize,
}

/// Compress a buffer (default level 6, pos = 1).
pub fn deflate(buffer: &[u8]) -> Result<Vec<u8>> {
    Ok(deflate_with_stats(buffer, 6, 1, None)?.output)
}

/// Decompress a buffer (pos = 1).
pub fn inflate(buffer: &[u8]) -> Result<Vec<u8>> {
    Ok(inflate_with_stats(buffer, 1, None)?.output)
}

/// Compress with stats, mirroring the C R_deflate behavior.
pub fn deflate_with_stats(
    buffer: &[u8],
    level: u8,
    pos: usize,
    size: Option<usize>,
) -> Result<GzipOutput> {
    if !(1..=9).contains(&level) {
        return Err(ZipError::generic("compression level must be 1-9"));
    }
    if pos == 0 || pos > buffer.len() + 1 {
        return Err(ZipError::generic("pos is out of range"));
    }

    let start = pos.saturating_sub(1);
    let data = &buffer[start..];
    let initial_size = size.unwrap_or(data.len()).max(10);

    let result = compress_to_buffer(data, level as i32, initial_size)
        .map_err(|e| ZipError::generic(format!("deflate failed: {e}")))?;

    Ok(GzipOutput {
        output: result.output,
        bytes_read: result.bytes_read,
        bytes_written: result.bytes_written,
    })
}

/// Decompress with stats, mirroring the C R_inflate behavior.
pub fn inflate_with_stats(
    buffer: &[u8],
    pos: usize,
    size: Option<usize>,
) -> Result<GzipOutput> {
    if pos == 0 || pos > buffer.len() + 1 {
        return Err(ZipError::generic("pos is out of range"));
    }

    let start = pos.saturating_sub(1);
    let data = &buffer[start..];
    let initial_size = size.unwrap_or(data.len().saturating_mul(2)).max(10);

    let result = decompress_to_buffer(data, initial_size)
        .map_err(|e| ZipError::generic(format!("inflate failed: {e}")))?;

    Ok(GzipOutput {
        output: result.output,
        bytes_read: result.bytes_read,
        bytes_written: result.bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflate_inflate_roundtrip() {
        let data = b"Hello world!";
        let compressed = deflate_with_stats(data, 6, 1, None).unwrap();
        let inflated = inflate_with_stats(&compressed.output, 1, None).unwrap();
        assert_eq!(inflated.output, data);
    }

    #[test]
    fn test_deflate_inflate_empty() {
        let data = b"";
        let compressed = deflate_with_stats(data, 6, 1, None).unwrap();
        println!("DEBUG: Empty data compressed to: {:?}", compressed.output);
        println!("DEBUG: Compressed length: {}", compressed.output.len());
        let inflated = inflate_with_stats(&compressed.output, 1, None).unwrap();
        assert_eq!(inflated.output, data);
    }
}
