use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let offset: u64 = 232_755_560_448;

    println!("Opening disk {}...", disk_path);
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    file.seek(SeekFrom::Start(offset))?;
    
    let mut sector = vec![0u8; 512];
    file.read_exact(&mut sector)?;

    println!("First 256 bytes at offset {}:", offset);
    println!("{:X?}", &sector[..256]);

    Ok(())
}
