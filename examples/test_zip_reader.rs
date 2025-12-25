use zip_rs::zip::ZipReader;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let zip_path = if args.len() > 1 {
        &args[1]
    } else {
        "/tmp/test.zip"
    };

    let result = ZipReader::open(zip_path);
    match result {
        Ok(reader) => {
            println!("ZIP opened successfully!");
            println!("Entries: {}", reader.entries().len());
            for entry in reader.entries() {
                println!("  - name='{}' size={} bytes", entry.name, entry.uncompressed_size);
            }
        }
        Err(e) => {
            println!("Failed to open ZIP: {:?}", e);
        }
    }
}
