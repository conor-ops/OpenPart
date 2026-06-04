use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let offset: u64 = 170_489_020_416; // Start of D: partition

    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    file.seek(SeekFrom::Start(offset))?;
    
    let mut sector = vec![0u8; 512];
    file.read_exact(&mut sector)?;

    println!("Offset (Hex) | Bytes");
    println!("-------------|--------------------------------------------------");
    for chunk in sector.chunks(16).enumerate() {
        let hex_offset = format!("0x{:02X}", chunk.0 * 16);
        let bytes: Vec<String> = chunk.1.iter().map(|b| format!("{:02X}", b)).collect();
        println!("{:11} | {}", hex_offset, bytes.join(" "));
    }

    Ok(())
}
