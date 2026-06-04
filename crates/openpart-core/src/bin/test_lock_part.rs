use std::os::windows::io::AsRawHandle;
use std::os::windows::fs::OpenOptionsExt;
use std::io::{Seek, SeekFrom, Write, Read};
use windows_sys::Win32::System::Ioctl::FSCTL_ALLOW_EXTENDED_DASD_IO;
use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

fn main() {
    let disk_number = 0;
    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    let offset = 232755560448u64; // Partition 6

    println!("Opening {}", disk_path);

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(1 | 2)
        .open(&disk_path)
        .expect("Failed to open physical drive");

    let handle = file.as_raw_handle();
    let mut returned = 0;
    
    println!("Applying DASD IO...");
    unsafe {
        let s1 = win32_DeviceIoControl(handle as _, FSCTL_ALLOW_EXTENDED_DASD_IO, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut returned, std::ptr::null_mut());
        println!("DASD: {}", s1 != 0);
    }

    println!("Attempting to read boot sector of partition 6...");
    let mut buf = [0u8; 512];
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.read_exact(&mut buf).unwrap();
    println!("Read successfully. Signature: {:02x}{:02x}", buf[510], buf[511]);

    println!("Attempting to write back the exact same boot sector...");
    file.seek(SeekFrom::Start(offset)).unwrap();
    match file.write_all(&buf) {
        Ok(_) => println!("Write SUCCESS!"),
        Err(e) => println!("Write FAILED: {}", e),
    }
}
