use std::fs::OpenOptions;
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let d_offset: u64 = 170_489_020_416; // Start of D:
    let e_offset: u64 = 244_349_665_280; // Start of E:

    println!("Opening disk {}...", disk_path);
    let mut file_read = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    let mut file_write = OpenOptions::new()
        .write(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    // Read D's boot sector
    file_read.seek(SeekFrom::Start(d_offset))?;
    let mut sector = vec![0u8; 512];
    file_read.read_exact(&mut sector)?;

    println!("Original D: boot sector signature at 0x03: {:?}", String::from_utf8_lossy(&sector[3..11]));

    // Modify Hidden Sectors for E (477,245,440 sectors = 0x1C724000)
    // Little endian: 00 40 72 1C
    sector[28] = 0x00;
    sector[29] = 0x40;
    sector[30] = 0x72;
    sector[31] = 0x1C;

    println!("Writing patched boot sector to E: start offset {}...", e_offset);
    file_write.seek(SeekFrom::Start(e_offset))?;
    file_write.write_all(&sector)?;
    file_write.flush()?;

    println!("Successfully wrote boot sector to E:!");
    Ok(())
}
