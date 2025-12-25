pub mod bitstream;
pub mod crc32;
pub mod adler32;
pub mod huffman;
pub mod deflate;
pub mod inflate;

pub use bitstream::{BitReader, BitWriter};
pub use crc32::crc32;
pub use adler32::adler32;
pub use deflate::{compress, compress_to_buffer, compress_raw, CompressionLevel, DeflateError, DeflateEncoder, DeflateOptions, FlushMode, Strategy, CompressResult};
pub use inflate::{decompress, decompress_to_buffer, decompress_raw, InflateDecoder, InflateState, InflateStatus, InflateFlags, InflateError};