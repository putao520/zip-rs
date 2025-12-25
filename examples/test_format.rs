use zip_rs::gzip::{deflate_with_stats, inflate_with_stats};

fn main() {
    let data = b"Hello world!";
    println!("Original: {:?}", data);

    let compressed = deflate_with_stats(data, 6, 1, None).unwrap();
    println!("Compressed: {:?}", compressed.output);
    println!("Hex: {}", compressed.output.iter().map(|b| format!("{:02x}", b)).collect::<String>());

    // Check first 2 bytes for ZLIB header
    if compressed.output.len() >= 2 {
        println!("First two bytes: 0x{:02x}{:02x}", compressed.output[0], compressed.output[1]);
        println!("Is ZLIB header (0x789c): {}{}",
                 if compressed.output[0] == 0x78 { "Yes" } else { "No" },
                 if compressed.output.len() >= 2 && compressed.output[1] == 0x9c { "Yes" } else { "No" });
    }

    let inflated = inflate_with_stats(&compressed.output, 1, None);
    match inflated {
        Ok(result) => {
            println!("Inflated: {:?}", result.output);
            println!("Success: {}", result.output == data);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}