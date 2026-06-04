use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;
use std::fs::OpenOptions;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{
    FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetDiskFreeSpaceExW, GetVolumeInformationW,
    GetVolumePathNamesForVolumeNameW,
};
use windows_sys::Win32::System::Ioctl::{
    IOCTL_DISK_GET_LENGTH_INFO, DRIVE_LAYOUT_INFORMATION_EX, GET_LENGTH_INFORMATION,
    PARTITION_STYLE_GPT, PARTITION_STYLE_MBR, PARTITION_STYLE_RAW,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::models::{DiskInfo, DiskState, DiskStyle, Partition, PartitionKind};
use crate::error::OpenPartError;
use crate::executor::get_drive_layout;

const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x560000;

#[repr(C)]
#[allow(non_snake_case)]
struct DISK_EXTENT {
    DiskNumber: u32,
    StartingOffset: i64,
    ExtentLength: i64,
}

#[repr(C)]
#[allow(non_snake_case)]
struct VOLUME_DISK_EXTENTS {
    NumberOfDiskExtents: u32,
    Extents: [DISK_EXTENT; 1],
}

#[derive(Debug, Clone)]
struct VolumeInfo {
    _guid_path: String,
    disk_number: u32,
    offset: u64,
    letter: Option<String>,
    file_system: String,
    label: String,
    free_bytes: u64,
    total_bytes: u64,
}

fn u16_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        OsString::from_wide(slice).to_string_lossy().into_owned()
    }
}

pub fn scan_disks() -> Result<DiskState, OpenPartError> {
    let mut volumes = Vec::new();

    // 1. Scan all volumes
    unsafe {
        let mut name_buf = [0u16; 512];
        let h_find = FindFirstVolumeW(name_buf.as_mut_ptr(), name_buf.len() as u32);
        if h_find != INVALID_HANDLE_VALUE {
            loop {
                let vol_path = u16_ptr_to_string(name_buf.as_ptr());
                
                if let Ok(file) = OpenOptions::new().read(true).open(vol_path.trim_end_matches('\\')) {
                    let mut extents_buf = [0u8; 1024];
                    let mut returned = 0;
                    let ok = DeviceIoControl(
                        file.as_raw_handle() as _,
                        IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                        std::ptr::null(),
                        0,
                        extents_buf.as_mut_ptr() as _,
                        extents_buf.len() as u32,
                        &mut returned,
                        null_mut(),
                    );
                    if ok != 0 {
                        let extents = &*(extents_buf.as_ptr() as *const VOLUME_DISK_EXTENTS);
                        if extents.NumberOfDiskExtents > 0 {
                            let disk_number = extents.Extents[0].DiskNumber;
                            let offset = extents.Extents[0].StartingOffset as u64;

                            let mut path_names_buf = [0u16; 512];
                            let mut path_returned = 0;
                            GetVolumePathNamesForVolumeNameW(
                                name_buf.as_ptr(),
                                path_names_buf.as_mut_ptr(),
                                path_names_buf.len() as u32,
                                &mut path_returned,
                            );
                            let path_names_str = u16_ptr_to_string(path_names_buf.as_ptr());
                            let letter = if !path_names_str.is_empty() && path_names_str.contains(":\\") {
                                Some(path_names_str[..1].to_string())
                            } else {
                                None
                            };

                            let mut fs_name_buf = [0u16; 128];
                            let mut vol_name_buf = [0u16; 128];
                            GetVolumeInformationW(
                                name_buf.as_ptr(),
                                vol_name_buf.as_mut_ptr(),
                                vol_name_buf.len() as u32,
                                null_mut(),
                                null_mut(),
                                null_mut(),
                                fs_name_buf.as_mut_ptr(),
                                fs_name_buf.len() as u32,
                            );

                            let mut free_bytes = 0u64;
                            let mut total_bytes = 0u64;
                            GetDiskFreeSpaceExW(
                                name_buf.as_ptr(),
                                null_mut(),
                                &mut total_bytes,
                                &mut free_bytes,
                            );

                            volumes.push(VolumeInfo {
                                _guid_path: vol_path,
                                disk_number,
                                offset,
                                letter,
                                file_system: u16_ptr_to_string(fs_name_buf.as_ptr()),
                                label: u16_ptr_to_string(vol_name_buf.as_ptr()),
                                free_bytes,
                                total_bytes,
                            });
                        }
                    }
                }

                if FindNextVolumeW(h_find, name_buf.as_mut_ptr(), name_buf.len() as u32) == 0 {
                    break;
                }
            }
            FindVolumeClose(h_find);
        }
    }

    // 2. Scan disks
    let mut disks = Vec::new();
    for disk_number in 0..32 {
        let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
        if let Ok(disk_file) = OpenOptions::new().read(true).open(&disk_path) {
            let mut length_info = GET_LENGTH_INFORMATION { Length: 0 };
            let mut returned = 0;
            let ok = unsafe {
                DeviceIoControl(
                    disk_file.as_raw_handle() as _,
                    IOCTL_DISK_GET_LENGTH_INFO,
                    std::ptr::null(),
                    0,
                    &mut length_info as *mut _ as *mut _,
                    std::mem::size_of::<GET_LENGTH_INFORMATION>() as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            
            if ok == 0 { continue; }
            let disk_size = length_info.Length as u64;

            let buffer = match get_drive_layout(disk_file.as_raw_handle()) {
                Ok(b) => b,
                Err(_) => continue,
            };
            
            let layout = unsafe { &*(buffer.as_ptr() as *const DRIVE_LAYOUT_INFORMATION_EX) };
            let count = layout.PartitionCount as usize;
            let entries = unsafe { std::slice::from_raw_parts(layout.PartitionEntry.as_ptr(), count) };

            let style = if layout.PartitionStyle == PARTITION_STYLE_GPT as u32 {
                DiskStyle::Gpt
            } else {
                DiskStyle::Mbr
            };

            let mut partitions = Vec::new();
            
            // Collect valid partitions
            let mut parts = Vec::new();
            for entry in entries {
                if entry.PartitionStyle == PARTITION_STYLE_RAW || entry.PartitionLength == 0 {
                    continue;
                }
                parts.push(entry);
            }
            parts.sort_by_key(|e| e.StartingOffset);

            let mut cursor = 0u64;
            let mut virtual_index = 1_000_000u32;
            
            for entry in parts {
                let offset = entry.StartingOffset as u64;
                let size = entry.PartitionLength as u64;
                
                if cursor < offset {
                    let gap = offset - cursor;
                    if gap >= 16 * 1024 * 1024 {
                        partitions.push(Partition {
                            index: virtual_index,
                            label: "Unallocated".to_string(),
                            letter: None,
                            file_system: "".to_string(),
                            size_gb: (gap as f64) / 1_073_741_824.0,
                            used_gb: 0.0,
                            kind: PartitionKind::Unallocated,
                            status: "Online".to_string(),
                        });
                        virtual_index += 1;
                    }
                }

                let vol = volumes.iter().find(|v| v.disk_number == disk_number && v.offset == offset);
                
                let mut kind = PartitionKind::Primary;
                let mut status = "Basic".to_string();

                let mut label = "Partition".to_string();
                if let Some(v) = vol {
                    if !v.label.is_empty() {
                        label = v.label.clone();
                    }
                }

                if layout.PartitionStyle == PARTITION_STYLE_GPT as u32 {
                    let gpt_type = unsafe { entry.Anonymous.Gpt.PartitionType };
                    // efi: c12a7328-f81f-11d2-ba4b-00a0c93ec93b
                    if gpt_type.data1 == 0xc12a7328 {
                        kind = PartitionKind::System;
                        label = "EFI System".to_string();
                        status = "EFI".to_string();
                    }
                    // msr: e3c9e316-0b5c-4db8-817d-f92df00215ae
                    if gpt_type.data1 == 0xe3c9e316 {
                        kind = PartitionKind::System;
                        label = "Microsoft Reserved".to_string();
                        status = "MSR".to_string();
                    }
                } else if layout.PartitionStyle == PARTITION_STYLE_MBR as u32 {
                    let mbr_type = unsafe { entry.Anonymous.Mbr.PartitionType };
                    if mbr_type == 0x0F || mbr_type == 0x05 {
                        kind = PartitionKind::Logical; // Extended actually, but openpart handles it as such
                    }
                }

                let size_gb = (size as f64) / 1_073_741_824.0;
                let used_gb = if let Some(v) = vol {
                    let used = v.total_bytes.saturating_sub(v.free_bytes);
                    (used as f64) / 1_073_741_824.0
                } else {
                    0.0
                };

                partitions.push(Partition {
                    index: entry.PartitionNumber,
                    label,
                    letter: vol.and_then(|v| v.letter.clone()),
                    file_system: vol.map(|v| v.file_system.clone()).unwrap_or_default(),
                    size_gb,
                    used_gb,
                    kind,
                    status,
                });

                cursor = offset + size;
            }

            if cursor < disk_size {
                let gap = disk_size - cursor;
                if gap >= 16 * 1024 * 1024 {
                    partitions.push(Partition {
                        index: virtual_index,
                        label: "Unallocated".to_string(),
                        letter: None,
                        file_system: "".to_string(),
                        size_gb: (gap as f64) / 1_073_741_824.0,
                        used_gb: 0.0,
                        kind: PartitionKind::Unallocated,
                        status: "Online".to_string(),
                    });
                }
            }

            disks.push(DiskInfo {
                id: disk_path.clone(),
                label: format!("Disk {}", disk_number),
                model: "Disk Drive".to_string(),
                size_gb: (disk_size as f64) / 1_073_741_824.0,
                style,
                status: "Online".to_string(),
                partitions,
            });
        }
    }

    Ok(DiskState {
        disks,
        queue: vec![],
    })
}
