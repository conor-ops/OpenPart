use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let offsets = vec![
        256_059_112_960u64, // End of 11,709,448,192 at 244,349,665,280
        256_059_989_504u64, // End of 11,709,448,192 at 244,350,541,824
        256_060_513_792u64, // End of 11,709,448,192 at 244,351,066,112
        256_058_820_096u64, // End of 11,709,154,816 at 244,349,665,280
    ];

    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    for &offset in &offsets {
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; 512];
        if file.read_exact(&mut buf).is_ok() {
            println!("Offset {}: Bytes 3..11 = {:?}", offset, String::from_utf8_lossy(&buf[3..11]));
            if &buf[3..11] == b"-FVE-FS-" {
                println!("  -> FOUND BitLocker backup boot sector!");
                println!("  -> First 64 bytes: {:X?}", &buf[..64]);
            }
        }
    }

    Ok(())
}
