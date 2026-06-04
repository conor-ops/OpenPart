use windows_sys::Win32::System::Ioctl::{VOLUME_DISK_EXTENTS, DISK_EXTENT};
const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x560000;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::Storage::FileSystem::{FindFirstVolumeW, FindNextVolumeW, FindVolumeClose};
use std::os::windows::io::AsRawHandle;

fn main() {
    let mut name = [0u16; 512];
    let handle = unsafe { FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32) };
    if handle as isize == -1 {
        println!("FindFirstVolumeW failed: {}", std::io::Error::last_os_error());
        return;
    }

    loop {
        let vol_name_w = name.iter().take_while(|&&c| c != 0).copied().collect::<Vec<u16>>();
        let vol_name = String::from_utf16_lossy(&vol_name_w);
        let trimmed = vol_name.trim_end_matches('\\');
        
        if let Ok(file) = std::fs::OpenOptions::new().read(true).open(trimmed) {
            let mut extents = [0u8; 1024];
            let mut returned = 0;
            let ok = unsafe {
                DeviceIoControl(
                    file.as_raw_handle() as _,
                    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                    std::ptr::null(),
                    0,
                    extents.as_mut_ptr() as _,
                    extents.len() as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            if ok != 0 {
                let vde = unsafe { &*(extents.as_ptr() as *const VOLUME_DISK_EXTENTS) };
                let extent = unsafe { &*(&vde.Extents as *const [DISK_EXTENT; 1] as *const DISK_EXTENT) };
                println!("Vol: {} -> Disk {}, Offset {}", trimmed, extent.DiskNumber, extent.StartingOffset);
            }
        }

        let mut name_next = [0u16; 512];
        let ok = unsafe { FindNextVolumeW(handle as _, name_next.as_mut_ptr(), name_next.len() as u32) };
        if ok == 0 {
            break;
        }
        name = name_next;
    }
    unsafe { FindVolumeClose(handle as _) };
}
