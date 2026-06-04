use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let start_offset: u64 = 232_000_000_000;
    let end_offset: u64 = 256_060_000_000;

    println!("Scanning disk {} from {} to {} in 64KB steps...", disk_path, start_offset, end_offset);
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    let mut sector = vec![0u8; 512];
    let step = 65536; // 64 KB steps
    let mut cursor = (start_offset / step) * step;

    while cursor < end_offset {
        file.seek(SeekFrom::Start(cursor))?;
        if file.read_exact(&mut sector).is_ok() {
            if &sector[3..11] == b"NTFS    " {
                println!("FOUND NTFS Boot Sector at offset {} (0x{:X})", cursor, cursor);
                println!("First 16 bytes: {:X?}", &sector[..16]);
            }
        }
        cursor += step;
    }

    println!("Scan finished.");
    Ok(())
}
