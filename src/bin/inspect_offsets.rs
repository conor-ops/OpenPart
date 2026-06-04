use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let offsets = vec![
        244_351_066_112u64,
        244_350_541_824u64,
        244_349_665_280u64,
    ];

    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    for &offset in &offsets {
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; 512];
        file.read_exact(&mut buf)?;
        println!("Offset {}: {:X?}", offset, &buf[..64]);
    }

    Ok(())
}
