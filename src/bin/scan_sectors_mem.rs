use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let start_offset: u64 = 232_000_000_000;
    let end_offset: u64 = 256_060_514_304;

    println!("Scanning disk sequentially for BitLocker signatures (-FVE-FS-)...");
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    file.seek(SeekFrom::Start(start_offset))?;

    let chunk_size = 64 * 1024 * 1024; // 64 MB
    let mut buffer = vec![0u8; chunk_size];
    let mut current_offset = start_offset;

    while current_offset < end_offset {
        let bytes_to_read = std::cmp::min(chunk_size as u64, end_offset - current_offset) as usize;
        if bytes_to_read == 0 { break; }

        file.read_exact(&mut buffer[..bytes_to_read])?;

        // Search for "-FVE-FS-" signature in the buffer
        for i in 0..(bytes_to_read - 8) {
            if &buffer[i..i+8] == b"-FVE-FS-" {
                let signature_offset = current_offset + i as u64;
                let sector_offset = signature_offset - 3;
                if sector_offset % 512 == 0 {
                    println!("FOUND BitLocker Boot Sector at offset {} (0x{:X})", sector_offset, sector_offset);
                }
            }
        }

        current_offset += bytes_to_read as u64;
    }

    println!("Scan finished.");
    Ok(())
}
