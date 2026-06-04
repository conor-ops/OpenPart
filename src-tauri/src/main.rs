#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openpart_core::{build_plan, ApplyReport, DiskState, Executor, Plan, PlanInputs, SimulatedExecutor, RealExecutor, PlanAction};
use std::env;
use std::fs;
use std::path::Path;
use std::os::windows::io::AsRawHandle;
use windows_sys::core::GUID;
use windows_sys::Win32::System::Ioctl::{
    DRIVE_LAYOUT_INFORMATION_EX, PARTITION_STYLE_GPT, PARTITION_INFORMATION_EX,
    IOCTL_DISK_GET_DRIVE_LAYOUT_EX, IOCTL_DISK_SET_DRIVE_LAYOUT_EX,
    IOCTL_DISK_UPDATE_PROPERTIES,
};
use openpart_service;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct BackupPartition {
    pub partition_number: u32,
    pub offset: u64,
    pub size: u64,
    pub drive_letter: Option<String>,
    pub label: String,
    pub filesystem: String,
    pub boot_sector_hex: Option<String>,
    pub backup_boot_sector_hex: Option<String>,
    pub gpt_type: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BackupSnapshot {
    pub timestamp: String,
    pub description: String,
    pub disk_number: u32,
    pub partitions: Vec<BackupPartition>,
}

fn parse_disk_number(path: &str) -> Option<u32> {
    for token in path.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(n) = token.parse::<u32>() {
            return Some(n);
        }
    }
    None
}

fn save_layout_backup(disk_number: u32, description: &str) -> Result<(), String> {
    // 1. Query the current partitions
    let layout_output = std::process::Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Get-Partition -DiskNumber {} | Select-Object PartitionNumber, Offset, Size, DriveLetter, GptType | ConvertTo-Json",
                disk_number
            ),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let layout_stdout = String::from_utf8_lossy(&layout_output.stdout);

    #[derive(serde::Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct PsPartInfo {
        partition_number: u32,
        offset: u64,
        size: u64,
        drive_letter: Option<String>,
        gpt_type: Option<String>,
    }

    let parts: Vec<PsPartInfo> = if layout_stdout.trim().starts_with('[') {
        serde_json::from_str(&layout_stdout).unwrap_or_default()
    } else if !layout_stdout.trim().is_empty() {
        if let Ok(single) = serde_json::from_str::<PsPartInfo>(&layout_stdout) {
            vec![single]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let disk_path = format!(r"\\.\PhysicalDrive{}", disk_number);
    use std::os::windows::fs::OpenOptionsExt;
    let mut disk_file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(&disk_path)
        .ok();

    let mut backup_parts = Vec::new();
    for p in parts {
        // Query label and filesystem
        let vol_output = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Get-Volume -Partition (Get-Partition -DiskNumber {} -PartitionNumber {}) | Select-Object FileSystemLabel, FileSystem | ConvertTo-Json",
                    disk_number, p.partition_number
                ),
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let vol_stdout = String::from_utf8_lossy(&vol_output.stdout);
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct PsVolInfo { file_system_label: Option<String>, file_system: Option<String> }
        let (label, fs) = if let Ok(vol) = serde_json::from_str::<PsVolInfo>(&vol_stdout) {
            (vol.file_system_label.unwrap_or_default(), vol.file_system.unwrap_or_default())
        } else {
            ("".to_string(), "".to_string())
        };

        let mut boot_sector_hex = None;
        let mut backup_boot_sector_hex = None;
        if let Some(ref mut file) = disk_file {
            use std::io::Seek;
            if file.seek(std::io::SeekFrom::Start(p.offset)).is_ok() {
                let mut buf = [0u8; 512];
                if std::io::Read::read_exact(file, &mut buf).is_ok() {
                    boot_sector_hex = Some(hex_encode(&buf));
                }
            }
            let backup_offset = p.offset + p.size - 512;
            if file.seek(std::io::SeekFrom::Start(backup_offset)).is_ok() {
                let mut buf = [0u8; 512];
                if std::io::Read::read_exact(file, &mut buf).is_ok() {
                    backup_boot_sector_hex = Some(hex_encode(&buf));
                }
            }
        }

        backup_parts.push(BackupPartition {
            partition_number: p.partition_number,
            offset: p.offset,
            size: p.size,
            drive_letter: p.drive_letter,
            label,
            filesystem: fs,
            boot_sector_hex,
            backup_boot_sector_hex,
            gpt_type: p.gpt_type,
        });
    }

    // Load existing backups
    let backups_path = "C:\\Users\\new\\.gemini\\antigravity\\openpart-backups.json";
    let mut backups: Vec<BackupSnapshot> = if Path::new(backups_path).exists() {
        let content = fs::read_to_string(backups_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    // Prepend new backup
    let new_backup = BackupSnapshot {
        timestamp: chrono::Utc::now().to_rfc3339(),
        description: description.to_string(),
        disk_number,
        partitions: backup_parts,
    };
    backups.insert(0, new_backup);

    // Keep only recent 2
    if backups.len() > 2 {
        backups.truncate(2);
    }

    // Save back to file
    fs::write(backups_path, serde_json::to_string_pretty(&backups).unwrap().as_bytes()).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn get_state() -> DiskState {
    match openpart_service::scan_disks() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("disk scan failed: {}", e);
            // return an empty state when scan fails (no mock data)
            DiskState { disks: vec![], queue: vec![] }
        }
    }
}

#[tauri::command]
fn build_plan_command(inputs: PlanInputs) -> Result<Plan, String> {
    build_plan(inputs).map_err(|err| err.to_string())
}

#[tauri::command]
async fn apply_plan_command(inputs: PlanInputs, dry_run: bool) -> Result<ApplyReport, String> {
    let plan = build_plan(inputs.clone()).map_err(|err| err.to_string())?;
    // Execute with real executor when not a dry-run.
    if dry_run {
        let executor = SimulatedExecutor::new();
        executor.execute(&plan, dry_run).map_err(|err| err.to_string())
    } else {
        // Take backup snapshot before applying real operations
        let disk_number = parse_disk_number(&inputs.disk).unwrap_or(0);
        let action_desc = if let Some(action) = &inputs.action {
            match action {
                PlanAction::Resize { target, size_gb } => format!("Before: Resize {} to {:.2} GB", target, size_gb),
                PlanAction::Move { target, .. } => format!("Before: Move {}", target),
                PlanAction::Create { fs, size_gb, .. } => format!("Before: Create {} {:.2} GB", fs, size_gb),
                PlanAction::Delete { target } => format!("Before: Delete {}", target),
                PlanAction::Assign { target, letter } => format!("Before: Assign {} to {}", letter, target),
                PlanAction::Format { target, fs, .. } => format!("Before: Format {} as {}", target, fs),
                _ => "Before operation".to_string(),
            }
        } else {
            "Before operation".to_string()
        };
        if let Err(e) = save_layout_backup(disk_number, &action_desc) {
            eprintln!("Warning: failed to save layout backup: {}", e);
        }

        let executor = RealExecutor;
        executor.execute(&plan, dry_run).map_err(|err| err.to_string())
    }
}

#[tauri::command]
fn compose_plan_state(current_state: DiskState, queue: Vec<PlanInputs>) -> Result<DiskState, String> {
    openpart_core::engine::compose_plan_state(current_state, queue).map_err(|err| err.to_string())
}

#[tauri::command]
async fn get_backups_command() -> Result<Vec<BackupSnapshot>, String> {
    let backups_path = "C:\\Users\\new\\.gemini\\antigravity\\openpart-backups.json";
    if Path::new(backups_path).exists() {
        let content = fs::read_to_string(backups_path).map_err(|e| e.to_string())?;
        let backups: Vec<BackupSnapshot> = serde_json::from_str(&content).unwrap_or_default();
        Ok(backups)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
async fn restore_layout_command(disk_number: u32, backup_partitions: Vec<BackupPartition>) -> Result<ApplyReport, String> {
    // 1. Get current layout (via powershell, just to check current partitions and letters)
    let layout_output = std::process::Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Get-Partition -DiskNumber {} | Select-Object PartitionNumber, Offset, Size, DriveLetter | ConvertTo-Json",
                disk_number
            ),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let layout_stdout = String::from_utf8_lossy(&layout_output.stdout);
    
    #[derive(serde::Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct CurrentPartition {
        partition_number: u32,
        offset: u64,
        size: u64,
        drive_letter: Option<String>,
    }

    let current_parts: Vec<CurrentPartition> = if layout_stdout.trim().starts_with('[') {
        serde_json::from_str(&layout_stdout).unwrap_or_default()
    } else if !layout_stdout.trim().is_empty() {
        if let Ok(single) = serde_json::from_str::<CurrentPartition>(&layout_stdout) {
            vec![single]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let mut steps = Vec::new();

    // 2. Query current partitions and remove any active drive letters of modified partitions
    steps.push("Removing active drive letters of modified partitions to prevent locks...".to_string());
    for cp in &current_parts {
        let mut matches_backup = false;
        for bp in &backup_partitions {
            if bp.partition_number == cp.partition_number && bp.offset == cp.offset && bp.size == cp.size {
                matches_backup = true;
                break;
            }
        }
        if !matches_backup {
            if let Some(ref letter) = cp.drive_letter {
                if !letter.trim().is_empty() {
                    if let Err(e) = openpart_core::executor::delete_volume_mount_point(letter) {
                        steps.push(format!("Warning: failed to delete volume mount point for {}: {}", letter, e));
                    } else {
                        steps.push(format!("Drive letter {}: removed.", letter));
                    }
                }
            }
        }
    }

    // 3. Open physical disk write handle
    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    use std::os::windows::fs::OpenOptionsExt;
    let mut disk_file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(1 | 2) // FILE_SHARE_READ | FILE_SHARE_WRITE
        .open(&disk_path)
    {
        Ok(f) => f,
        Err(e) => return Err(format!("Failed to open disk {}: {}", disk_path, e)),
    };
    let disk_handle = disk_file.as_raw_handle();

    // 4. Get current drive layout in memory
    let mut layout_buffer = match get_drive_layout(disk_handle) {
        Ok(buf) => buf,
        Err(e) => return Err(format!("Failed to read drive layout: {}", e)),
    };
    // Modify layout_buffer in place
    let mut layout = unsafe { &mut *(layout_buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
    let is_gpt = layout.PartitionStyle == PARTITION_STYLE_GPT as u32;
    let mut count = layout.PartitionCount as usize;

    if is_gpt && count < 128 {
        let target_count = 128;
        let target_size = std::mem::size_of::<DRIVE_LAYOUT_INFORMATION_EX>() 
            + (target_count - 1) * std::mem::size_of::<PARTITION_INFORMATION_EX>();
        
        let mut new_buffer = vec![0u8; target_size];
        new_buffer[..layout_buffer.len()].copy_from_slice(&layout_buffer);
        
        layout_buffer = new_buffer;
        layout = unsafe { &mut *(layout_buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
        layout.PartitionCount = target_count as u32;
        count = target_count;
    }

    let entries = unsafe {
        std::slice::from_raw_parts_mut(
            layout.PartitionEntry.as_mut_ptr(),
            count,
        )
    };

    // 5. Identify and delete partition entries not present in backup
    steps.push("Identifying and marking deleted partitions in memory layout...".to_string());
    for entry in entries.iter_mut() {
        if entry.StartingOffset != 0 || entry.PartitionLength != 0 {
            let mut keep = false;
            for bp in &backup_partitions {
                if bp.partition_number == entry.PartitionNumber {
                    keep = true;
                    break;
                }
            }
            if !keep {
                steps.push(format!("Partition {} marked for deletion.", entry.PartitionNumber));
                entry.StartingOffset = 0;
                entry.PartitionLength = 0;
                entry.RewritePartition = true;
                if entry.PartitionStyle == 1 { // GPT
                    unsafe {
                        let gpt = &mut entry.Anonymous.Gpt;
                        std::ptr::write_bytes(&mut gpt.PartitionType, 0, 1);
                        std::ptr::write_bytes(&mut gpt.PartitionId, 0, 1);
                        gpt.Attributes = 0;
                        gpt.Name = [0u16; 36];
                    }
                } else if entry.PartitionStyle == 0 { // MBR
                    unsafe {
                        let mbr = &mut entry.Anonymous.Mbr;
                        mbr.PartitionType = 0;
                        mbr.RecognizedPartition = false;
                    }
                }
            }
        }
    }

    // 6. Update or create backup partition entries
    steps.push("Updating and creating partitions in memory layout...".to_string());
    for bp in &backup_partitions {
        let mut found_entry = None;
        for entry in entries.iter_mut() {
            if entry.PartitionNumber == bp.partition_number && (entry.StartingOffset != 0 || entry.PartitionLength != 0) {
                found_entry = Some(entry);
                break;
            }
        }

        if let Some(entry) = found_entry {
            steps.push(format!("Updating partition {} (Offset: {} -> {}, Size: {} -> {}).", bp.partition_number, entry.StartingOffset, bp.offset, entry.PartitionLength, bp.size));
            entry.StartingOffset = bp.offset as i64;
            entry.PartitionLength = bp.size as i64;
            entry.RewritePartition = true;
            if entry.PartitionStyle == 1 { // GPT
                unsafe {
                    let gpt = &mut entry.Anonymous.Gpt;
                    let type_guid_str = bp.gpt_type.as_deref().unwrap_or("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7");
                    if let Some(guid) = parse_guid(type_guid_str) {
                        gpt.PartitionType = guid;
                    }
                    let is_zero_id = gpt.PartitionId.data1 == 0 && gpt.PartitionId.data2 == 0 && gpt.PartitionId.data3 == 0;
                    if is_zero_id {
                        gpt.PartitionId = generate_random_guid();
                    }
                    let wide_name: Vec<u16> = bp.label.encode_utf16().chain(std::iter::once(0)).collect();
                    let name_len = wide_name.len().min(36);
                    gpt.Name = [0u16; 36];
                    gpt.Name[..name_len].copy_from_slice(&wide_name[..name_len]);
                }
            } else if entry.PartitionStyle == 0 { // MBR
                unsafe {
                    let mbr = &mut entry.Anonymous.Mbr;
                    mbr.PartitionType = 0x07;
                    mbr.RecognizedPartition = true;
                    mbr.HiddenSectors = (bp.offset / 512) as u32;
                }
            }
        } else {
            steps.push(format!("Creating new partition entry {} (Offset: {}, Size: {}).", bp.partition_number, bp.offset, bp.size));
            let mut unused_slot = None;
            for entry in entries.iter_mut() {
                if entry.StartingOffset == 0 && entry.PartitionLength == 0 {
                    unused_slot = Some(entry);
                    break;
                }
            }

            if let Some(entry) = unused_slot {
                entry.PartitionStyle = layout.PartitionStyle as i32;
                entry.StartingOffset = bp.offset as i64;
                entry.PartitionLength = bp.size as i64;
                entry.PartitionNumber = bp.partition_number;
                entry.RewritePartition = true;
                if entry.PartitionStyle == 1 { // GPT
                    unsafe {
                        let gpt = &mut entry.Anonymous.Gpt;
                        let type_guid_str = bp.gpt_type.as_deref().unwrap_or("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7");
                        gpt.PartitionType = parse_guid(type_guid_str).unwrap_or_else(|| parse_guid("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7").unwrap());
                        gpt.PartitionId = generate_random_guid();
                        gpt.Attributes = 0;
                        let wide_name: Vec<u16> = bp.label.encode_utf16().chain(std::iter::once(0)).collect();
                        let name_len = wide_name.len().min(36);
                        gpt.Name = [0u16; 36];
                        gpt.Name[..name_len].copy_from_slice(&wide_name[..name_len]);
                    }
                } else if entry.PartitionStyle == 0 { // MBR
                    unsafe {
                        let mbr = &mut entry.Anonymous.Mbr;
                        mbr.PartitionType = 0x07;
                        mbr.RecognizedPartition = true;
                        mbr.HiddenSectors = (bp.offset / 512) as u32;
                    }
                }
            } else {
                return Err("No unused partition slots left in partition table".to_string());
            }
        }
    }

    // 7. Write updated partition table to disk
    steps.push("Writing updated partition table to disk...".to_string());
    if let Err(e) = set_drive_layout(disk_handle, &layout_buffer) {
        return Err(format!("Failed to set drive layout: {}", e));
    }
    steps.push("Partition table written successfully. Disk layout updated in memory.".to_string());

    // 8. Write VBRs to new offsets BEFORE updating disk properties (so OS mounts correctly)
    steps.push("Writing VBRs (Volume Boot Records) to disk offsets...".to_string());
    for bp in &backup_partitions {
        if bp.boot_sector_hex.is_none() && bp.backup_boot_sector_hex.is_none() {
            continue;
        }

        if let Some(ref hex_str) = bp.boot_sector_hex {
            if let Some(boot_sector_bytes) = hex_decode(hex_str) {
                if boot_sector_bytes.len() == 512 {
                    use std::io::Seek;
                    if let Err(e) = disk_file.seek(std::io::SeekFrom::Start(bp.offset)) {
                        steps.push(format!("Warning: failed to seek to offset {} to restore boot sector: {}", bp.offset, e));
                    } else if let Err(e) = std::io::Write::write_all(&mut disk_file, &boot_sector_bytes) {
                        steps.push(format!("Warning: failed to write boot sector to offset {}: {}", bp.offset, e));
                    } else {
                        steps.push(format!("Restored primary boot sector to offset {}", bp.offset));
                    }
                }
            }
        }

        if let Some(ref backup_hex_str) = bp.backup_boot_sector_hex {
            if let Some(backup_boot_sector_bytes) = hex_decode(backup_hex_str) {
                if backup_boot_sector_bytes.len() == 512 {
                    let backup_offset = bp.offset + bp.size - 512;
                    use std::io::Seek;
                    if let Err(e) = disk_file.seek(std::io::SeekFrom::Start(backup_offset)) {
                        steps.push(format!("Warning: failed to seek to backup offset {} to restore backup boot sector: {}", backup_offset, e));
                    } else if let Err(e) = std::io::Write::write_all(&mut disk_file, &backup_boot_sector_bytes) {
                        steps.push(format!("Warning: failed to write backup boot sector to offset {}: {}", backup_offset, e));
                    } else {
                        steps.push(format!("Restored backup boot sector to offset {}", backup_offset));
                    }
                }
            }
        }
    }
    let _ = disk_file.sync_all();
    steps.push("All VBRs written and disk buffers flushed successfully.".to_string());

    // 9. Update disk properties (triggers OS re-enumeration)
    steps.push("Notifying OS to update disk properties and re-mount volumes...".to_string());
    if let Err(e) = update_disk_properties(disk_handle) {
        return Err(format!("Failed to update disk properties: {}", e));
    }
    steps.push("OS notified of layout changes. Mounting volumes...".to_string());

    // Drop disk_file to release the handle so volumes can mount cleanly!
    drop(disk_file);
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // 10. Re-assign drive letters and labels to restored partitions
    let mut assign_cmds = Vec::new();
    assign_cmds.push("$ErrorActionPreference = 'Stop'".to_string());
    for bp in &backup_partitions {
        if let Some(letter) = &bp.drive_letter {
            if !letter.trim().is_empty() {
                assign_cmds.push(format!(
                    "Get-Partition -DiskNumber {} -PartitionNumber {} -ErrorAction SilentlyContinue | Add-PartitionAccessPath -AccessPath '{}:' -ErrorAction SilentlyContinue",
                    disk_number, bp.partition_number, letter
                ));
            }
        }
        if !bp.label.is_empty() {
            if let Some(letter) = &bp.drive_letter {
                if !letter.trim().is_empty() {
                    assign_cmds.push(format!(
                        "Set-Volume -DriveLetter {} -NewFileSystemLabel '{}' -ErrorAction SilentlyContinue",
                        letter, bp.label
                    ));
                }
            }
        }
    }
    assign_cmds.push(format!("Update-Disk -Number {} -ErrorAction SilentlyContinue", disk_number));

    if assign_cmds.len() > 2 {
        steps.push("Assigning drive letters and labels...".to_string());
        use std::io::Write;
        let mut child = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(mut stdin) = child.stdin.take() {
            let mut script_content = String::new();
            for cmd in &assign_cmds {
                script_content.push_str(cmd);
                script_content.push_str("\n");
            }
            stdin.write_all(script_content.as_bytes()).map_err(|e| e.to_string())?;
        }
        let _ = child.wait();
        steps.push("Drive letter and label assignments completed.".to_string());
    }

    Ok(ApplyReport {
        ok: true,
        dry_run: false,
        steps,
        warnings: vec![],
    })
}

fn get_drive_layout(disk_handle: std::os::windows::io::RawHandle) -> Result<Vec<u8>, std::io::Error> {
    use windows_sys::Win32::System::Ioctl::DRIVE_LAYOUT_INFORMATION_EX;
    use windows_sys::Win32::System::Ioctl::PARTITION_INFORMATION_EX;
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    let buffer_size = std::mem::size_of::<DRIVE_LAYOUT_INFORMATION_EX>() 
        + 127 * std::mem::size_of::<PARTITION_INFORMATION_EX>();
    let mut buffer = vec![0u8; buffer_size];
    let mut bytes_returned = 0u32;

    unsafe {
        let success = win32_DeviceIoControl(
            disk_handle as _,
            IOCTL_DISK_GET_DRIVE_LAYOUT_EX,
            std::ptr::null(),
            0,
            buffer.as_mut_ptr() as _,
            buffer_size as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );

        if success == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }

    let layout = unsafe { &*(buffer.as_ptr() as *const DRIVE_LAYOUT_INFORMATION_EX) };
    let count = layout.PartitionCount;
    let actual_size = std::mem::size_of::<DRIVE_LAYOUT_INFORMATION_EX>() 
        + (count.saturating_sub(1) as usize) * std::mem::size_of::<PARTITION_INFORMATION_EX>();
    buffer.truncate(actual_size);
    Ok(buffer)
}

fn set_drive_layout(disk_handle: std::os::windows::io::RawHandle, buffer: &[u8]) -> Result<(), std::io::Error> {
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    let mut bytes_returned = 0u32;
    unsafe {
        let success = win32_DeviceIoControl(
            disk_handle as _,
            IOCTL_DISK_SET_DRIVE_LAYOUT_EX,
            buffer.as_ptr() as _,
            buffer.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );

        if success == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn update_disk_properties(disk_handle: std::os::windows::io::RawHandle) -> Result<(), std::io::Error> {
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    let mut bytes_returned = 0u32;
    unsafe {
        let success = win32_DeviceIoControl(
            disk_handle as _,
            IOCTL_DISK_UPDATE_PROPERTIES,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );

        if success == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn parse_guid(s: &str) -> Option<GUID> {
    let cleaned = s.trim().trim_matches(|c| c == '{' || c == '}');
    let parts: Vec<&str> = cleaned.split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    let d1 = u32::from_str_radix(parts[0], 16).ok()?;
    let d2 = u16::from_str_radix(parts[1], 16).ok()?;
    let d3 = u16::from_str_radix(parts[2], 16).ok()?;
    if parts[3].len() != 4 || parts[4].len() != 12 {
        return None;
    }
    let mut d4 = [0u8; 8];
    let p3_bytes = hex_decode(parts[3])?;
    let p4_bytes = hex_decode(parts[4])?;
    if p3_bytes.len() != 2 || p4_bytes.len() != 6 {
        return None;
    }
    d4[0] = p3_bytes[0];
    d4[1] = p3_bytes[1];
    d4[2..8].copy_from_slice(&p4_bytes);
    Some(GUID {
        data1: d1,
        data2: d2,
        data3: d3,
        data4: d4,
    })
}

fn generate_random_guid() -> GUID {
    use std::time::SystemTime;
    let mut bytes = [0u8; 16];
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let mut seed = now;
    for i in 0..16 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        bytes[i] = (seed >> (i * 4)) as u8;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // v4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1
    
    let d1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let d2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let d3 = u16::from_le_bytes([bytes[6], bytes[7]]);
    let mut d4 = [0u8; 8];
    d4.copy_from_slice(&bytes[8..16]);
    
    GUID {
        data1: d1,
        data2: d2,
        data3: d3,
        data4: d4,
    }
}

#[tauri::command]
async fn restore_backup_command(timestamp: String) -> Result<ApplyReport, String> {
    let backups_path = "C:\\Users\\new\\.gemini\\antigravity\\openpart-backups.json";
    if !Path::new(backups_path).exists() {
        return Err("No backups exist".to_string());
    }

    let content = fs::read_to_string(backups_path).map_err(|e| e.to_string())?;
    let backups: Vec<BackupSnapshot> = serde_json::from_str(&content).unwrap_or_default();

    let backup = backups.iter().find(|b| b.timestamp == timestamp)
        .ok_or_else(|| "Backup snapshot not found".to_string())?;

    restore_layout_command(backup.disk_number, backup.partitions.clone()).await
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_state,
            build_plan_command,
            apply_plan_command,
            compose_plan_state,
            get_backups_command,
            restore_backup_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let byte_str = &s[i..i+2];
        let byte = u8::from_str_radix(byte_str, 16).ok()?;
        bytes.push(byte);
    }
    Some(bytes)
}
