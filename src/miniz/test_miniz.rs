//! Simple test to validate miniz core functionality

fn main() {
    println!("Testing miniz core components...");

    // Test bitstream
    use crate::bitstream::{BitWriter, BitReader};

    // Test BitWriter
    let mut writer = BitWriter::new();
    writer.write_bits(0b1010, 4);
    writer.write_bits(0b11001100, 8);
    writer.write_bit(true);
    writer.write_bit(false);
    writer.align_to_byte();

    let bytes = writer.into_bytes();
    println!("BitWriter test: {} bytes", bytes.len());

    // Test BitReader
    let mut reader = BitReader::from_slice(&bytes);
    assert_eq!(reader.read_bits(4).unwrap(), 0b1010);
    assert_eq!(reader.read_bits(8).unwrap(), 0b11001100);
    assert_eq!(reader.read_bit().unwrap(), true);
    assert_eq!(reader.read_bit().unwrap(), false);

    println!("BitReader test passed!");

    // Test CRC32
    use crate::crc32::crc32;
    let test_data = b"Hello, World!";
    let crc = crc32(test_data);
    println!("CRC32 test: 0x{:08x}", crc);

    // Test Adler32
    use crate::adler32::adler32;
    let adler = adler32(test_data);
    println!("Adler32 test: 0x{:08x}", adler);

    println!("All miniz core tests passed!");
}