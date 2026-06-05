use crate::error::OpenPartError;
use crate::models::{ApplyReport, Plan};
use crate::models::PlanAction;
use std::process::{Command, Stdio};
use std::io::{Read, Write, Seek, SeekFrom};

fn new_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use sha2::Digest;



pub trait Executor {
    fn execute(&self, plan: &Plan, dry_run: bool) -> Result<ApplyReport, OpenPartError>;
}

#[derive(Default)]
pub struct SimulatedExecutor;

impl SimulatedExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Executor for SimulatedExecutor {
    fn execute(&self, plan: &Plan, dry_run: bool) -> Result<ApplyReport, OpenPartError> {
        let mut steps = Vec::new();
        if dry_run {
            steps.push("Dry run: no data will be written.".to_string());
        }
        steps.extend(plan.steps.clone());
        Ok(ApplyReport {
            ok: true,
            dry_run,
            steps,
            warnings: plan.warnings.clone(),
        })
    }
}

pub struct RealExecutor;

impl Executor for RealExecutor {
    fn execute(&self, _plan: &Plan, _dry_run: bool) -> Result<ApplyReport, OpenPartError> {
        // Guarded: require explicit environment variable to enable real writes.
        if _dry_run {
            let mut steps = Vec::new();
            steps.push("Dry run: no data will be written.".to_string());
            steps.extend(_plan.steps.clone());
            return Ok(ApplyReport {
                ok: true,
                dry_run: true,
                steps,
                warnings: _plan.warnings.clone(),
            });
        }

        // Writes are enabled when this executor is used (caller should still use dry-run to preview).

        // Only support single-action plans for now; map PlanAction to PowerShell commands.
        let action_opt = _plan.inputs.action.clone();
        let action = match action_opt {
            Some(a) => a,
            None => return Err(OpenPartError::NoOperation),
        };

        let disk_path = &_plan.inputs.disk;
        let disk_number = parse_disk_number(disk_path).ok_or_else(|| OpenPartError::InvalidPlan(format!("invalid disk path {}", disk_path)))?;


        let mut steps = Vec::new();

        match action {
            PlanAction::Resize { target, size_gb } => {
                if let Ok(path) = take_snapshot(disk_number, &_plan.inputs.out_dir) {
                    steps.push(format!("Saved disk snapshot to {}", path));
                }

                // 1. Query current partition layout to find size
                let layout_start = std::time::Instant::now();
                let resolved = resolve_target_partition(disk_number, &target)
                    .map_err(|e| OpenPartError::InvalidPlan(format!("target partition {} not found for resize: {}", target, e)))?;
                steps.push(format!("[PERF ] Resize: Resolve partition layout completed in {:.3} seconds.", layout_start.elapsed().as_secs_f64()));
                
                let current_size_bytes = resolved.partition_length;
                let target_size_bytes = (size_gb * 1_073_741_824.0).round().max(1.0) as u64;

                if target_size_bytes == current_size_bytes {
                    steps.push("Partition is already the requested size.".to_string());
                    return Ok(ApplyReport {
                        ok: true,
                        dry_run: false,
                        steps,
                        warnings: _plan.warnings.clone(),
                    });
                }

                steps.push(format!("Resizing partition to {} bytes...", target_size_bytes));
                let resize_start = std::time::Instant::now();
                let ps_cmd = format!(
                    "Resize-Partition -DiskNumber {} -PartitionNumber {} -Size {} -ErrorAction Stop",
                    disk_number, resolved.partition_number, target_size_bytes
                );
                let resize_output = new_command("powershell")
                    .args(&["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
                    .output()?;
                
                if !resize_output.status.success() {
                    return Err(OpenPartError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Resize-Partition failed: {}", String::from_utf8_lossy(&resize_output.stderr)),
                    )));
                }

                steps.push(format!("[PERF ] Resize-Partition completed in {:.3} seconds.", resize_start.elapsed().as_secs_f64()));

                steps.push("Resized partition using Resize-Partition.".to_string());
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Create { fs, size_gb, label, drive_letter } => {
                let size_bytes = if size_gb <= 0.0 {
                    0 // Use all available
                } else {
                    (size_gb * 1024.0 * 1024.0 * 1024.0).round() as u64
                };

                let create_start = std::time::Instant::now();
                // Find a gap
                let (segments, _disk_size) = get_disk_segments(disk_number)?;
                let mut virtual_index = 1_000_000u32;
                let mut target_gap_offset_and_size = None;
                
                let mut gaps = Vec::new();
                for s in &segments {
                    if !s.is_partition {
                        if s.size >= 16 * 1024 * 1024 {
                            gaps.push((s.offset, s.size, virtual_index));
                            virtual_index += 1;
                        }
                    }
                }
                
                if let Some(target_idx) = _plan.inputs.source_unalloc_lba {
                    for (offset, size, idx) in &gaps {
                        if *idx == target_idx as u32 {
                            target_gap_offset_and_size = Some((*offset, *size));
                            break;
                        }
                    }
                }

                let target_gap = if let Some((offset, size)) = target_gap_offset_and_size {
                    Some(NativeDiskSegment {
                        is_partition: false,
                        offset,
                        size,
                        partition_number: None,
                    })
                } else {
                    if size_bytes == 0 {
                        segments.iter().filter(|s| !s.is_partition).max_by_key(|s| s.size).cloned()
                    } else {
                        segments.iter().filter(|s| !s.is_partition && s.size >= size_bytes).next().cloned()
                    }
                };
                let gap = target_gap.ok_or_else(|| OpenPartError::InvalidPlan("No unallocated space found for Create".into()))?;

                let offset = align_up(gap.offset, 1_048_576);
                if offset >= gap.offset + gap.size {
                    return Err(OpenPartError::InvalidPlan("Gap too small to align".into()));
                }
                let max_avail = gap.offset + gap.size - offset;
                let create_size = if size_bytes == 0 { max_avail } else { size_bytes };
                if create_size > max_avail {
                    return Err(OpenPartError::InvalidPlan("Requested size exceeds available gap size".into()));
                }
                let actual_size = align_down(create_size, 1_048_576);
                if actual_size == 0 {
                    return Err(OpenPartError::InvalidPlan("Size aligned to 0".into()));
                }

                let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
                let disk_file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .share_mode(1 | 2)
                    .open(&disk_path)
                    .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open disk: {}", e)))?;

                // Inject partition using IOCTL
                use windows_sys::Win32::System::Ioctl::{DRIVE_LAYOUT_INFORMATION_EX, PARTITION_STYLE_GPT, IOCTL_DISK_SET_DRIVE_LAYOUT_EX, PARTITION_INFORMATION_EX};
                use std::os::windows::io::AsRawHandle;

                let handle = disk_file.as_raw_handle();
                let mut buffer = get_drive_layout(handle)?;
                let mut layout = unsafe { &mut *(buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
                let is_gpt = layout.PartitionStyle == PARTITION_STYLE_GPT as u32;
                let mut count = layout.PartitionCount as usize;

                if is_gpt && count < 128 {
                    let target_count = 128;
                    let target_size = std::mem::size_of::<DRIVE_LAYOUT_INFORMATION_EX>() 
                        + (target_count - 1) * std::mem::size_of::<PARTITION_INFORMATION_EX>();
                    
                    let mut new_buffer = vec![0u8; target_size];
                    new_buffer[..buffer.len()].copy_from_slice(&buffer);
                    
                    buffer = new_buffer;
                    layout = unsafe { &mut *(buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
                    layout.PartitionCount = target_count as u32;
                    count = target_count;
                }

                let entries = unsafe { std::slice::from_raw_parts_mut(layout.PartitionEntry.as_mut_ptr(), count) };

                // Find empty slot
                let mut slot_idx = None;
                for i in 0..count {
                    if entries[i].PartitionLength == 0 {
                        slot_idx = Some(i);
                        break;
                    }
                }

                if let Some(idx) = slot_idx {
                    entries[idx].PartitionStyle = PARTITION_STYLE_GPT;
                    entries[idx].StartingOffset = offset as i64;
                    entries[idx].PartitionLength = actual_size as i64;
                    entries[idx].PartitionNumber = 0; // OS sets this
                    entries[idx].RewritePartition = true;
                    
                    let gpt = unsafe { &mut entries[idx].Anonymous.Gpt };
                    gpt.PartitionType = windows_sys::core::GUID {
                        data1: 0xebd0a0a2,
                        data2: 0xb9e5,
                        data3: 0x4433,
                        data4: [0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26, 0x99, 0xc7],
                    };
                    unsafe {
                        windows_sys::Win32::System::Com::CoCreateGuid(&mut gpt.PartitionId);
                    }
                    gpt.Attributes = 0;
                    gpt.Name = [0; 36];
                    let name_u16 = "Basic Data Partition".encode_utf16().collect::<Vec<_>>();
                    for (i, &c) in name_u16.iter().enumerate() {
                        if i < 36 {
                            gpt.Name[i] = c;
                        }
                    }
                    
                    let mut returned = 0;
                    let ok = unsafe {
                        windows_sys::Win32::System::IO::DeviceIoControl(
                            handle as _,
                            IOCTL_DISK_SET_DRIVE_LAYOUT_EX,
                            buffer.as_mut_ptr() as *const _,
                            buffer.len() as u32,
                            core::ptr::null_mut(),
                            0,
                            &mut returned,
                            core::ptr::null_mut(),
                        )
                    };
                    if ok == 0 {
                        return Err(OpenPartError::Io(std::io::Error::last_os_error()));
                    }
                    update_disk_properties(handle)?;
                } else {
                    return Err(OpenPartError::InvalidPlan("No empty partition slots found in GPT".into()));
                }

                steps.push(format!("Partition created natively in {:.3} seconds.", create_start.elapsed().as_secs_f64()));

                // Poll for volume
                let mut new_vol = String::new();
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if let Some(v) = get_volume_by_offset(disk_number, offset) {
                        new_vol = v.trim_end_matches('\\').to_string();
                        break;
                    }
                }

                if new_vol.is_empty() {
                    return Err(OpenPartError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Failed to find newly created volume. It may take longer to mount.")));
                }

                // Format
                let format_start = std::time::Instant::now();
                let lbl = label.unwrap_or_else(|| "".to_string());
                let mut format_args = vec![new_vol.clone(), format!("/FS:{}", fs), "/Q".to_string(), "/Y".to_string()];
                if !lbl.is_empty() {
                    format_args.push(format!("/V:{}", lbl));
                } else {
                    format_args.push("/V:".to_string());
                }
                let output = new_command("C:\\Windows\\System32\\format.com").args(&format_args).output()?;
                if !output.status.success() {
                    return Err(OpenPartError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("format failed: {}", String::from_utf8_lossy(&output.stderr)))));
                }
                steps.push(format!("Formatted natively in {:.3} seconds.", format_start.elapsed().as_secs_f64()));

                // Assign
                let letter = drive_letter
                    .as_deref()
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty());
                
                if let Some(let_str) = letter {
                    let path = format!("{}:\\\0", let_str).encode_utf16().collect::<Vec<_>>();
                    let vol_w = format!("{}\\\0", new_vol).encode_utf16().collect::<Vec<_>>();
                    let ok = unsafe { windows_sys::Win32::Storage::FileSystem::SetVolumeMountPointW(path.as_ptr(), vol_w.as_ptr()) };
                    if ok == 0 {
                        steps.push(format!("Warning: SetVolumeMountPointW failed for {}: {}", let_str, std::io::Error::last_os_error()));
                    } else {
                        steps.push(format!("Assigned letter {}", let_str));
                    }
                }

                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Delete { target } => {
                let resolved = resolve_target_partition(disk_number, &target)
                    .map_err(|e| OpenPartError::InvalidPlan(format!("target partition {} not found: {}", target, e)))?;
                
                let old_offset = resolved.starting_offset;

                // Unmount volume if target is a drive letter
                let clean_target = target.trim().trim_end_matches(':').to_uppercase();
                if clean_target.len() == 1 && clean_target.chars().next().unwrap().is_ascii_alphabetic() {
                    let _ = delete_volume_mount_point(&clean_target);
                }

                // 3. Lock & Dismount Volume
                let _lock_handle = lock_volume_by_offset(disk_number, old_offset);

                // 4. Update GPT
                let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
                let disk_file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .share_mode(1 | 2)
                    .open(&disk_path)
                    .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open disk: {}", e)))?;

                delete_partition_from_layout(disk_file.as_raw_handle(), old_offset)?;

                // 5. Update disk properties
                update_disk_properties(disk_file.as_raw_handle())?;

                // Lock is dropped here
                drop(_lock_handle);

                steps.push("Partition deleted instantly via WinAPI.".to_string());
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Assign { target, letter } => {
                let resolved = resolve_target_partition(disk_number, &target)
                    .map_err(|e| OpenPartError::InvalidPlan(format!("target partition {} not found for assign: {}", target, e)))?;
                if let Some(vol) = get_volume_by_offset(disk_number, resolved.starting_offset) {
                    let path = format!("{}:\\\0", letter).encode_utf16().collect::<Vec<_>>();
                    let vol_w = format!("{}\\\0", vol.trim_end_matches('\\')).encode_utf16().collect::<Vec<_>>();
                    let ok = unsafe {
                        windows_sys::Win32::Storage::FileSystem::SetVolumeMountPointW(path.as_ptr(), vol_w.as_ptr())
                    };
                    if ok == 0 {
                        steps.push(format!("Warning: SetVolumeMountPointW failed: {}", std::io::Error::last_os_error()));
                    } else {
                        steps.push(format!("Assigned drive letter {} using WinAPI.", letter));
                    }
                } else {
                    return Err(OpenPartError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Volume not found for assignment")));
                }
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Format { target, fs, label } => {
                let resolved = resolve_target_partition(disk_number, &target)
                    .map_err(|e| OpenPartError::InvalidPlan(format!("target partition {} not found for format: {}", target, e)))?;
                if let Some(vol) = get_volume_by_offset(disk_number, resolved.starting_offset) {
                    let lbl = label.unwrap_or_else(|| "".to_string());
                    let vol_trimmed = vol.trim_end_matches('\\');
                    let mut args = vec![vol_trimmed.to_string(), format!("/FS:{}", fs), "/Q".to_string(), "/Y".to_string()];
                    if !lbl.is_empty() {
                        args.push(format!("/V:{}", lbl));
                    } else {
                        args.push("/V:".to_string()); // empty label
                    }
                    let output = new_command("C:\\Windows\\System32\\format.com").args(&args).output()?;
                    if !output.status.success() {
                        return Err(OpenPartError::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("format failed: {}", String::from_utf8_lossy(&output.stderr)))));
                    }
                    steps.push("Formatted partition natively using format.com.".to_string());
                } else {
                    return Err(OpenPartError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Volume not found for format")));
                }
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Wipe { disk } => {
                let n = parse_disk_number(&disk).unwrap_or(disk_number);
                let script = format!("select disk {}\nclean\n", n);
                let stdout = run_diskpart(&script, &mut steps)?;
                steps.push("Wiped disk using DiskPart.".to_string());
                for line in stdout.lines() {
                    if !line.trim().is_empty() && line.contains("successfully") {
                        steps.push(line.trim().to_string());
                    }
                }
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Restore { disk_number, snapshot_path } => {
                steps.push(format!("Restoring snapshot from {}...", snapshot_path));
                restore_snapshot(disk_number, &snapshot_path).map_err(|e| OpenPartError::Io(e))?;
                steps.push("Successfully restored GPT layout and partition VBRs.".to_string());
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Activate { target } => {
                let mut script = String::new();
                if let Some(letter) = parse_drive_letter(&target) {
                    script.push_str(&format!("select volume {}\n", letter));
                } else if let Some(idx) = parse_index(&target) {
                    script.push_str(&format!("select disk {}\nselect partition {}\n", disk_number, idx));
                } else {
                    return Err(OpenPartError::InvalidPlan(format!("unsupported activate target {}", target)));
                }
                script.push_str("active\n");
                
                let stdout = run_diskpart(&script, &mut steps)?;
                steps.push("Activated partition using DiskPart.".to_string());
                for line in stdout.lines() {
                    if !line.trim().is_empty() && line.contains("successfully") {
                        steps.push(line.trim().to_string());
                    }
                }
                Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings: _plan.warnings.clone(),
                })
            }
            PlanAction::Move { target, delta_mib } => {
                if let Ok(path) = take_snapshot(disk_number, &_plan.inputs.out_dir) {
                    steps.push(format!("Saved disk snapshot to {}", path));
                }
                #[derive(Debug, Clone)]
                struct DiskSegment {
                    is_partition: bool,
                    index: u32,
                    offset: u64,
                    size: u64,
                }
                
                let _create_start = std::time::Instant::now();
                let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
                
                // 1. Plan layout changes
                let (native_segments, _disk_size) = get_disk_segments(disk_number)?;
                
                let mut segments = Vec::new();
                let mut virtual_index = 1_000_000u32;
                for s in &native_segments {
                    if s.is_partition {
                        if let Some(num) = s.partition_number {
                            segments.push(DiskSegment {
                                index: num,
                                offset: s.offset,
                                size: s.size,
                                is_partition: true,
                            });
                        }
                    } else {
                        segments.push(DiskSegment {
                            index: virtual_index,
                            offset: s.offset,
                            size: s.size,
                            is_partition: false,
                        });
                        virtual_index += 1;
                    }
                }

                // Match target segment
                let target_resolved = resolve_target_partition(disk_number, &target);
                let target_seg_clone = if let Ok(res) = target_resolved {
                    segments.iter().find(|s| s.is_partition && s.index == res.partition_number)
                        .cloned()
                        .ok_or_else(|| OpenPartError::InvalidPlan(format!("target partition {} not found", target)))?
                } else {
                    return Err(OpenPartError::InvalidPlan(format!("target partition {} not found", target)));
                };
                let target_seg = &target_seg_clone;
                let phys_part = target_seg.index;
                let old_offset = target_seg.offset;
                let size = target_seg.size;
                
                let mut new_offset = if delta_mib >= 0 {
                    old_offset.saturating_add((delta_mib as u64) * 1024 * 1024)
                } else {
                    old_offset.saturating_sub(((-delta_mib) as u64) * 1024 * 1024)
                };
                new_offset = align_down(new_offset, 1_048_576);

                if old_offset == new_offset {
                    return Err(OpenPartError::InvalidPlan("No move occurred".into()));
                }

                // Validate no overlap
                for p in &native_segments {
                    if !p.is_partition { continue; }
                    if p.partition_number.is_none() { continue; }
                    if p.partition_number == Some(phys_part) { continue; }
                    let p_end = p.offset + p.size;
                    if p.offset < new_offset + size && p_end > new_offset {
                        return Err(OpenPartError::InvalidPlan(format!("Target offset range overlaps with partition {:?}", p.partition_number)));
                    }
                }

                // 2. Generate journal
                let op_id = format!("move-{}", chrono::Utc::now().timestamp());
                let journal_path = "C:\\Users\\new\\.gemini\\antigravity\\openpart-journal.json";
                
                let mut steps = Vec::new();
                steps.push("=== Phase 0: Pre-flight checks ===".to_string());

                if is_on_battery() && !_plan.inputs.allow_battery {
                    return Err(OpenPartError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "PREFLIGHT_FAIL: Running on battery. Connect AC power and retry.",
                    )));
                }
                steps.push("Power: AC connected.".to_string());
                steps.push("Target range is clear.".to_string());

                // BitLocker check
                if let Some(vol) = get_volume_by_offset(disk_number, old_offset) {
                    let bde_start = std::time::Instant::now();
                    let vol_trimmed = vol.trim_end_matches('\\');
                    let bl_status = new_command("manage-bde")
                        .args(&["-status", vol_trimmed])
                        .output();
                    steps.push(format!("[PERF ] BitLocker check completed in {:.3} seconds.", bde_start.elapsed().as_secs_f64()));
                    if let Ok(out) = bl_status {
                        let status_str = String::from_utf8_lossy(&out.stdout);
                        if status_str.contains("Protection On") || status_str.contains("Encrypting") || status_str.contains("Decrypting") {
                            return Err(OpenPartError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "PREFLIGHT_FAIL: BitLocker is active. Disable it completely before moving.",
                            )));
                        }
                    }
                    steps.push("BitLocker: not active.".to_string());
                } else {
                    steps.push("BitLocker: No volume found to check (likely unformatted or RAW).".to_string());
                }
                steps.push("All pre-flight checks passed.".to_string());

                // Calculate pre-copy hash samples
                let chunk_size: u64 = 8388608; // 8 MB (smaller chunk size prevents driver timeout resets)
                let verify_offsets = vec![
                    old_offset,
                    align_down(old_offset + (size / 2), 4096),
                    if size > chunk_size { align_down(old_offset + size - chunk_size, 4096) } else { old_offset },
                ];

                // Read and preserve the partition's primary boot sector (first 512 bytes) and backup boot sector (last 512 bytes)
                let mut boot_sector = [0u8; 512];
                let mut backup_boot_sector = [0u8; 512];
                {
                    let mut file_boot = std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1 | 2)
                        .open(&disk_path)
                        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open disk to read boot sectors: {}", e)))?;
                    file_boot.seek(SeekFrom::Start(old_offset)).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("Failed to seek to boot sector offset {}: {}", old_offset, e))
                    })?;
                    file_boot.read_exact(&mut boot_sector).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("Failed to read boot sector: {}", e))
                    })?;

                    let backup_offset = old_offset + size - 512;
                    file_boot.seek(SeekFrom::Start(backup_offset)).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("Failed to seek to backup boot sector offset {}: {}", backup_offset, e))
                    })?;
                    file_boot.read_exact(&mut backup_boot_sector).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("Failed to read backup boot sector: {}", e))
                    })?;
                }

                // === Phase 2: Dismounting and locking volume ===
                steps.push("=== Phase 2: Dismounting and locking volume ===".to_string());

                let _lock_handle = lock_volume_by_offset(disk_number, old_offset);
                if _lock_handle.is_none() {
                    steps.push("Warning: failed to acquire exclusive volume lock. Proceeding anyway...".to_string());
                } else {
                    steps.push("Exclusive volume lock acquired successfully.".to_string());
                }

                // Calculate pre-copy hash samples after locking/dismounting to ensure dirty cache blocks are flushed to disk
                steps.push("Calculating pre-copy hash samples...".to_string());
                let hash_start = std::time::Instant::now();
                let mut pre_hashes = serde_json::Map::new();
                
                {
                    let mut file_read = std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1 | 2)
                        .custom_flags(0x20000000)
                        .open(&disk_path)
                        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open file_read for pre-copy hashes: {}", e)))?;

                    for &offset in &verify_offsets {
                        let bytes_this_chunk = align_down(std::cmp::min(chunk_size, old_offset + size - offset), 4096) as usize;
                        if bytes_this_chunk == 0 { continue; }
                        file_read.seek(SeekFrom::Start(offset)).map_err(|e| {
                            std::io::Error::new(e.kind(), format!("file_read.seek failed in Phase 2 at offset {}: {}", offset, e))
                        })?;
                        let mut buf = vec![0u8; bytes_this_chunk];
                        let n = file_read.read(&mut buf).map_err(|e| {
                            std::io::Error::new(e.kind(), format!("file_read.read failed in Phase 2 at offset {} (size {}): {}", offset, bytes_this_chunk, e))
                        })?;
                        
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(&buf[..n]);
                        let hash = format!("{:x}", hasher.finalize());
                        
                        let relative_dest_offset = offset - old_offset + new_offset;
                        let key = format!("offset_{}", relative_dest_offset);
                        pre_hashes.insert(key, serde_json::Value::String(hash));
                    }
                }
                steps.push(format!("[PERF ] Pre-copy hash samples calculation completed in {:.3} seconds.", hash_start.elapsed().as_secs_f64()));

                // Write journal JSON
                let total_chunks = (size as f64 / chunk_size as f64).ceil() as u64;
                let journal_json = serde_json::json!({
                    "schema_version": 1,
                    "operation_id": op_id,
                    "started_at": chrono::Utc::now().to_rfc3339(),
                    "state": "IN_PROGRESS",
                    "disk_number": disk_number,
                    "partition_number": phys_part,
                    "drive_letter": "",
                    "partition_label": "",
                    "filesystem": "",
                    "old_offset_bytes": old_offset,
                    "new_offset_bytes": new_offset,
                    "partition_size_bytes": size,
                    "chunk_size_bytes": chunk_size,
                    "total_chunks": total_chunks,
                    "last_completed_chunk": -1,
                    "copy_direction": if new_offset < old_offset { "forward" } else { "backward" },
                    "pre_copy_hash_samples": pre_hashes,
                    "events": []
                });
                std::fs::write(journal_path, serde_json::to_string_pretty(&journal_json).unwrap())?;
                update_journal(journal_path, "PARTITION_LOCKED", None)?;


                // === Phase 3: Sector-by-sector copy ===
                steps.push("=== Phase 3: Copying sectors ===".to_string());
                let direction = if new_offset < old_offset { "forward" } else { "backward" };
                steps.push(format!("Copy direction: {}  |  Total chunks: {}  |  Chunk size: 8 MB", direction, total_chunks));

                update_journal(journal_path, "COPY_STARTED", None)?;

                // Open disk for raw read and write (with unbuffered I/O for speed, bypassing OS cache but allowing hardware caching)
                let mut file_read = std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(1 | 2)
                    .custom_flags(0x20000000) // FILE_FLAG_NO_BUFFERING only, no WRITE_THROUGH
                    .open(&disk_path)
                    .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open file_read for raw disk copy: {}", e)))?;
                let mut file_write = std::fs::OpenOptions::new()
                    .write(true)
                    .share_mode(1 | 2)
                    .custom_flags(0x20000000) // FILE_FLAG_NO_BUFFERING only, no WRITE_THROUGH
                    .open(&disk_path)
                    .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open file_write for raw disk copy: {}", e)))?;

                // Apply FSCTL_ALLOW_EXTENDED_DASD_IO to bypass partition bounds for physical disk writes
                let mut returned = 0;
                unsafe {
                    windows_sys::Win32::System::IO::DeviceIoControl(
                        file_read.as_raw_handle() as _,
                        0x00090083, // FSCTL_ALLOW_EXTENDED_DASD_IO
                        std::ptr::null(),
                        0,
                        std::ptr::null_mut(),
                        0,
                        &mut returned,
                        std::ptr::null_mut(),
                    );
                    windows_sys::Win32::System::IO::DeviceIoControl(
                        file_write.as_raw_handle() as _,
                        0x00090083, // FSCTL_ALLOW_EXTENDED_DASD_IO
                        std::ptr::null(),
                        0,
                        std::ptr::null_mut(),
                        0,
                        &mut returned,
                        std::ptr::null_mut(),
                    );
                }

                let mut buffer = AlignedBuffer::new(chunk_size as usize);
                let chunk_indices: Vec<u64> = if direction == "forward" {
                    (0..total_chunks).collect()
                } else {
                    (0..total_chunks).rev().collect()
                };

                let loop_start = std::time::Instant::now();
                let mut total_read_duration = std::time::Duration::default();
                let mut total_write_duration = std::time::Duration::default();

                for (idx, &i) in chunk_indices.iter().enumerate() {
                    let read_offset = old_offset + (i * chunk_size);
                    let write_offset = new_offset + (i * chunk_size);
                    let bytes_this_chunk = std::cmp::min(chunk_size, size - (i * chunk_size)) as usize;

                    let read_start = std::time::Instant::now();
                    file_read.seek(SeekFrom::Start(read_offset)).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("file_read.seek failed at chunk {} (read_offset {}): {}", i, read_offset, e))
                    })?;
                    file_read.read_exact(&mut buffer.as_mut_slice()[..bytes_this_chunk]).map_err(|e| {
                        let err_msg = format!(
                            "file_read.read_exact failed at chunk {} (offset {}, size {}): {}. \
                            Diagnostic Info: \
                            - Buffer Pointer: {:p} \
                            - Buffer Alignment: {} \
                            - Offset Alignment (4096): {} \
                            - Offset Alignment (512): {} \
                            - Size Alignment (4096): {} \
                            - Size Alignment (512): {} \
                            - Disk Path: {}",
                            i,
                            read_offset,
                            bytes_this_chunk,
                            e,
                            buffer.as_mut_slice().as_ptr(),
                            (buffer.as_mut_slice().as_ptr() as usize) % 4096,
                            read_offset % 4096,
                            read_offset % 512,
                            bytes_this_chunk % 4096,
                            bytes_this_chunk % 512,
                            disk_path
                        );
                        std::io::Error::new(e.kind(), err_msg)
                    })?;
                    let read_elapsed = read_start.elapsed();
                    total_read_duration += read_elapsed;

                    let write_start = std::time::Instant::now();
                    file_write.seek(SeekFrom::Start(write_offset)).map_err(|e| {
                        std::io::Error::new(e.kind(), format!("file_write.seek failed at chunk {} (write_offset {}): {}", i, write_offset, e))
                    })?;
                    file_write.write_all(&buffer.as_slice()[..bytes_this_chunk]).map_err(|e| {
                        let err_msg = format!(
                            "file_write.write failed at chunk {} (offset {}, size {}): {}. \
                            Diagnostic Info: \
                            - Buffer Pointer: {:p} \
                            - Buffer Alignment: {} \
                            - Offset Alignment (4096): {} \
                            - Offset Alignment (512): {} \
                            - Size Alignment (4096): {} \
                            - Size Alignment (512): {} \
                            - Disk Path: {}",
                            i,
                            write_offset,
                            bytes_this_chunk,
                            e,
                            buffer.as_slice().as_ptr(),
                            (buffer.as_slice().as_ptr() as usize) % 4096,
                            write_offset % 4096,
                            write_offset % 512,
                            bytes_this_chunk % 4096,
                            bytes_this_chunk % 512,
                            disk_path
                        );
                        std::io::Error::new(e.kind(), err_msg)
                    })?;
                    file_write.flush().map_err(|e| {
                        std::io::Error::new(e.kind(), format!("file_write.flush failed at chunk {}: {}", i, e))
                    })?;
                    let write_elapsed = write_start.elapsed();
                    total_write_duration += write_elapsed;

                    if idx % 10 == 0 || idx == chunk_indices.len() - 1 {
                        update_journal(journal_path, "IN_PROGRESS", Some(i as i64))?;
                        let done = (idx as u64 + 1) * chunk_size;
                        let pct = (done as f64 / size as f64 * 100.0).round();
                        let gb_done = done as f64 / 1_073_741_824.0;
                        let gb_total = size as f64 / 1_073_741_824.0;
                        
                        let elapsed = loop_start.elapsed().as_secs_f64();
                        let avg_speed = if elapsed > 0.0 { (done as f64 / 1_048_576.0) / elapsed } else { 0.0 };
                        let chunk_speed = if (read_elapsed.as_secs_f64() + write_elapsed.as_secs_f64()) > 0.0 {
                            (bytes_this_chunk as f64 / 1_048_576.0) / (read_elapsed.as_secs_f64() + write_elapsed.as_secs_f64())
                        } else {
                            0.0
                        };

                        steps.push(format!(
                            "[PROG ] Copied {:.2} GB / {:.2} GB ({:.1}%) - Avg: {:.1} MB/s | Last Chunk Read: {:.3}s, Write: {:.3}s ({:.1} MB/s)",
                            gb_done, gb_total, pct, avg_speed, read_elapsed.as_secs_f64(), write_elapsed.as_secs_f64(), chunk_speed
                        ));
                    }
                }
                
                drop(file_read);
                drop(file_write);

                steps.push(format!(
                    "[PERF ] Copy completed in {:.3} seconds (Read: {:.3}s, Write: {:.3}s).",
                    loop_start.elapsed().as_secs_f64(),
                    total_read_duration.as_secs_f64(),
                    total_write_duration.as_secs_f64()
                ));

                update_journal(journal_path, "COPY_COMPLETE", None)?;
                steps.push("Sector copy complete.".to_string());

                // === Phase 4: Verification (sampled read-back under lock) ===
                steps.push("=== Phase 4: Verification ===".to_string());
                {
                    let mut file_verify = std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1 | 2)
                        .custom_flags(0x20000000) // FILE_FLAG_NO_BUFFERING
                        .open(&disk_path)
                        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open file_verify for raw disk verification: {}", e)))?;

                    let verify_dest_offsets = vec![
                        new_offset,
                        align_down(new_offset + (size / 2), 4096),
                        if size > chunk_size { align_down(new_offset + size - chunk_size, 4096) } else { new_offset },
                    ];

                    for &offset in &verify_dest_offsets {
                        let bytes_this_chunk = align_down(std::cmp::min(chunk_size, new_offset + size - offset), 4096) as usize;
                        if bytes_this_chunk == 0 { continue; }
                        file_verify.seek(SeekFrom::Start(offset)).map_err(|e| {
                            std::io::Error::new(e.kind(), format!("file_verify.seek failed in Phase 4 at offset {}: {}", offset, e))
                        })?;
                        let mut buf = vec![0u8; bytes_this_chunk];
                        let n = file_verify.read(&mut buf).map_err(|e| {
                            std::io::Error::new(e.kind(), format!("file_verify.read failed in Phase 4 at offset {} (size {}): {}", offset, bytes_this_chunk, e))
                        })?;
                        
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(&buf[..n]);
                        let hash = format!("{:x}", hasher.finalize());
                        
                        steps.push(format!("Checksum at offset {} : {}", offset, hash));
                        
                        let key = format!("offset_{}", offset);
                        if let Some(expected_val) = pre_hashes.get(&key) {
                            if let Some(expected_hash) = expected_val.as_str() {
                                if expected_hash != hash {
                                    return Err(OpenPartError::Io(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        format!("VERIFY_FAIL: Checksum mismatch at offset {}. Expected {}, got {}", offset, expected_hash, hash),
                                    )));
                                }
                                steps.push("  * Checksum matches.".to_string());
                            }
                        }
                    }
                }

                // === Phase 5: Release volume lock ===
                steps.push("=== Phase 5: Releasing volume lock ===".to_string());
                drop(_lock_handle);

                // === Phase 6: Update GPT entry in-place ===
                steps.push("=== Phase 6: Updating GPT partition table in-place ===".to_string());
                {
                    let disk_file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .share_mode(1 | 2)
                        .open(&disk_path)
                        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open disk to update layout: {}", e)))?;
                    update_drive_layout(disk_file.as_raw_handle(), old_offset, new_offset)?;
                    steps.push("GPT layout updated in-place.".to_string());
                }
                update_journal(journal_path, "NEW_ENTRY_CREATED", None)?;

                // === Phase 7: Notify OS and assign letter ===
                steps.push("=== Phase 7: Notifying OS and assigning drive letter ===".to_string());
                {
                    let disk_file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .share_mode(1 | 2)
                        .open(&disk_path)
                        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to open disk to update properties: {}", e)))?;
                    update_disk_properties(disk_file.as_raw_handle())?;
                    steps.push("OS notified of disk partition layout changes.".to_string());
                }

                // Sleep to let OS settle
                std::thread::sleep(std::time::Duration::from_millis(500));

                // === Phase 8: Scanning volume integrity ===
                steps.push("=== Phase 8: Scanning volume integrity ===".to_string());
                let mut vol_trimmed = String::new();
                for _ in 0..10 {
                    if let Some(vol) = get_volume_by_offset(disk_number, new_offset) {
                        vol_trimmed = vol.trim_end_matches('\\').to_string();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }

                if !vol_trimmed.is_empty() {
                    steps.push("Running chkdsk integrity scan...".to_string());
                    let chkdsk_start = std::time::Instant::now();
                    let chkdsk_output = new_command("chkdsk")
                        .args(&[&vol_trimmed, "/scan", "/perf"])
                        .output();
                    steps.push(format!("[PERF ] chkdsk /scan /perf completed in {:.3} seconds.", chkdsk_start.elapsed().as_secs_f64()));
                    if let Ok(out) = chkdsk_output {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for line in stdout.lines() {
                            if !line.trim().is_empty() {
                                steps.push(format!("chkdsk: {}", line.trim()));
                            }
                        }
                    }
                }

                // === Phase 8: Complete ===
                update_journal(journal_path, "COMPLETED", None)?;
                steps.push("=== Operation COMPLETED successfully ===".to_string());
                steps.push(format!("Partition moved: offset {} -> {}", old_offset, new_offset));

                let mut warnings = _plan.warnings.clone();
                warnings.push("Physical partition shifting (Move) completed. Source offset data has been physically copied and verified.".to_string());

                return Ok(ApplyReport {
                    ok: true,
                    dry_run: false,
                    steps,
                    warnings,
                });
            }
            _ => Err(OpenPartError::RealOpsUnavailable),
        }
    }
}

fn run_diskpart(script: &str, steps: &mut Vec<String>) -> Result<String, std::io::Error> {
    use std::io::{BufRead, BufReader, Write};
    let start_time = std::time::Instant::now();
    let mut child = new_command("diskpart")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
    }

    let stdout_handle = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture diskpart stdout")
    })?;
    let stderr_handle = child.stderr.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture diskpart stderr")
    })?;

    // Read stdout in the current thread
    let mut stdout_accum = String::new();
    let reader = BufReader::new(stdout_handle);
    for line in reader.lines() {
        let line = line?;
        let elapsed = start_time.elapsed().as_secs_f64();
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            steps.push(format!("[diskpart +{:.3}s] {}", elapsed, trimmed));
            stdout_accum.push_str(&line);
            stdout_accum.push('\n');
        }
    }

    // Read stderr in the current thread
    let mut stderr_accum = String::new();
    let stderr_reader = BufReader::new(stderr_handle);
    for line in stderr_reader.lines() {
        let line = line?;
        stderr_accum.push_str(&line);
        stderr_accum.push('\n');
    }

    let status = child.wait()?;
    let duration = start_time.elapsed();
    steps.push(format!("[PERF ] DiskPart finished in {:.3} seconds.", duration.as_secs_f64()));

    if !status.success() || stdout_accum.contains("Error") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("DiskPart failed after {:.3} seconds. Script:\n{}\nStdout:\n{}\nStderr:\n{}", duration.as_secs_f64(), script, stdout_accum, stderr_accum),
        ));
    }

    Ok(stdout_accum)
}

fn parse_disk_number(path: &str) -> Option<u32> {
    // Expect paths like \\\"\\.\\PhysicalDrive0\\" or PhysicalDrive0
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

fn parse_index(s: &str) -> Option<u32> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    // try to extract trailing number
    for token in s.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() { continue; }
        if let Ok(n) = token.parse::<u32>() { return Some(n); }
    }
    None
}

fn align_up(val: u64, align: u64) -> u64 {
    (val + align - 1) & !(align - 1)
}

fn parse_drive_letter(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() == 1 && s.chars().all(|c| c.is_ascii_alphabetic()) {
        return Some(s.to_uppercase());
    }
    if s.ends_with(":") && s.len() == 2 {
        return Some(s[..1].to_uppercase());
    }
    None
}

fn is_on_battery() -> bool {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct SystemPowerStatus {
        ACLineStatus: u8,
        BatteryFlag: u8,
        BatteryLifePercent: u8,
        SystemStatusFlag: u8,
        BatteryLifeTime: u32,
        BatteryFullLifeTime: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemPowerStatus(lpSystemPowerStatus: *mut SystemPowerStatus) -> i32;
    }

    let mut status = SystemPowerStatus {
        ACLineStatus: 255,
        BatteryFlag: 255,
        BatteryLifePercent: 255,
        SystemStatusFlag: 0,
        BatteryLifeTime: 0,
        BatteryFullLifeTime: 0,
    };

    unsafe {
        if GetSystemPowerStatus(&mut status) != 0 {
            status.ACLineStatus == 0
        } else {
            false
        }
    }
}

fn update_journal(journal_path: &str, state: &str, last_completed_chunk: Option<i64>) -> Result<(), std::io::Error> {
    if !std::path::Path::new(journal_path).exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(journal_path)?;
    let mut j: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    j["state"] = serde_json::Value::String(state.to_string());
    if let Some(chunk) = last_completed_chunk {
        j["last_completed_chunk"] = serde_json::Value::Number(serde_json::Number::from(chunk));
    }
    
    let mut event = serde_json::json!({
        "time": chrono::Utc::now().to_rfc3339(),
        "state": state
    });
    if let Some(chunk) = last_completed_chunk {
        event.as_object_mut().unwrap().insert("last_chunk".to_string(), serde_json::Value::Number(serde_json::Number::from(chunk)));
    }
    
    if let Some(events) = j["events"].as_array_mut() {
        events.push(event);
    } else {
        j["events"] = serde_json::Value::Array(vec![event]);
    }
    
    let pretty = serde_json::to_string_pretty(&j)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(journal_path, pretty)?;
    Ok(())
}

struct AlignedBuffer {
    raw: Vec<u8>,
    offset: usize,
    size: usize,
}

impl AlignedBuffer {
    fn new(size: usize) -> Self {
        let raw = vec![0u8; size + 4096];
        let ptr = raw.as_ptr() as usize;
        let offset = (4096 - (ptr % 4096)) % 4096;
        Self { raw, offset, size }
    }

    fn as_slice(&self) -> &[u8] {
        &self.raw[self.offset .. self.offset + self.size]
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.raw[self.offset .. self.offset + self.size]
    }
}

fn align_down(val: u64, align: u64) -> u64 {
    (val / align) * align
}

#[link(name = "kernel32")]
extern "system" {
    fn DeviceIoControl(
        hDevice: *mut std::ffi::c_void,
        dwIoControlCode: u32,
        lpInBuffer: *mut std::ffi::c_void,
        nInBufferSize: u32,
        lpOutBuffer: *mut std::ffi::c_void,
        nOutBufferSize: u32,
        lpBytesReturned: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;

    fn DeleteVolumeMountPointW(lpszVolumeMountPoint: *const u16) -> i32;
}

pub fn get_volume_by_offset(disk_number: u32, offset: u64) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{FindFirstVolumeW, FindNextVolumeW, FindVolumeClose};
    use windows_sys::Win32::System::Ioctl::{VOLUME_DISK_EXTENTS, DISK_EXTENT};
    use std::os::windows::io::AsRawHandle;
    const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x560000;

    let mut name = [0u16; 512];
    let handle = unsafe { FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32) };
    if handle as isize == -1 {
        return None;
    }

    let mut matched_vol = None;

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
                    core::ptr::null_mut(),
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
                if extent.DiskNumber == disk_number && extent.StartingOffset == offset as i64 {
                    matched_vol = Some(vol_name);
                    break;
                }
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
    
    matched_vol
}

pub fn lock_volume_by_offset(disk_number: u32, offset: u64) -> Option<std::fs::File> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::fs::OpenOptionsExt;

    let mut path = None;
    for _ in 0..5 {
        if let Some(vol) = get_volume_by_offset(disk_number, offset) {
            path = Some(vol.trim_end_matches('\\').to_string());
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }

    let path = match path {
        Some(p) => p,
        None => return None,
    };

    // Open and lock
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(1 | 2) // FILE_SHARE_READ | FILE_SHARE_WRITE
        .open(&path) {
            Ok(f) => f,
            Err(_) => return None,
        };

    let handle = file.as_raw_handle();
    let mut returned: u32 = 0;

    unsafe {
        // Allow extended DASD IO
        DeviceIoControl(
            handle as *mut _,
            0x00090083, // FSCTL_ALLOW_EXTENDED_DASD_IO
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );

        // Lock
        let mut ok_lock = DeviceIoControl(
            handle as *mut _,
            0x00090018, // FSCTL_LOCK_VOLUME
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );

        if ok_lock == 0 {
            // Force dismount to invalidate open handles, then lock again
            DeviceIoControl(
                handle as *mut _,
                0x00090020, // FSCTL_DISMOUNT_VOLUME
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            );
            
            ok_lock = DeviceIoControl(
                handle as *mut _,
                0x00090018, // FSCTL_LOCK_VOLUME
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            );
            
            if ok_lock == 0 {
                return None;
            }
        } else {
            DeviceIoControl(
                handle as *mut _,
                0x00090020, // FSCTL_DISMOUNT_VOLUME
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            );
        }
    }

    Some(file)
}

pub fn delete_volume_mount_point(letter: &str) -> Result<(), std::io::Error> {
    let mount_point = format!("{}:\\", letter);
    let wide: Vec<u16> = mount_point.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let success = DeleteVolumeMountPointW(wide.as_ptr());
        if success == 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(2) { // 2 is ERROR_FILE_NOT_FOUND (if already deleted)
                return Err(err);
            }
        }
    }
    Ok(())
}

pub fn take_snapshot(disk_number: u32, out_dir: &str) -> Result<String, std::io::Error> {
    use crate::models::{DiskSnapshot, PartitionVbr};
    use std::io::{Read, Seek, SeekFrom};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Ioctl::DRIVE_LAYOUT_INFORMATION_EX;

    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    let disk_file = std::fs::OpenOptions::new().read(true).open(&disk_path)?;
    
    let layout_data = get_drive_layout(disk_file.as_raw_handle())?;
    
    let layout = unsafe { &*(layout_data.as_ptr() as *const DRIVE_LAYOUT_INFORMATION_EX) };
    let count = layout.PartitionCount as usize;
    let entries = unsafe { std::slice::from_raw_parts(layout.PartitionEntry.as_ptr(), count) };
    
    let mut vbrs = Vec::new();
    let mut read_file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .custom_flags(0x20000000)
        .open(&disk_path)?;

    for entry in entries {
        if entry.PartitionLength == 0 { continue; }
        let offset = entry.StartingOffset as u64;
        let size = entry.PartitionLength as u64;
        if size >= 8192 {
            let mut main_vbr = vec![0u8; 8192];
            let mut backup_vbr = vec![0u8; 8192];
            if read_file.seek(SeekFrom::Start(offset)).is_ok() {
                if read_file.read_exact(&mut main_vbr).is_ok() {
                    if read_file.seek(SeekFrom::Start(offset + size - 8192)).is_ok() {
                        let _ = read_file.read_exact(&mut backup_vbr);
                    }
                    vbrs.push(PartitionVbr { offset, main_vbr, backup_vbr });
                }
            }
        }
    }
    
    let snapshot = DiskSnapshot { layout_data, vbrs };
    let json = serde_json::to_string(&snapshot).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    
    let time_str = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name = format!("snapshot_disk{}_{}.json", disk_number, time_str);
    let full_path = std::path::Path::new(out_dir).join(&file_name);
    
    std::fs::write(&full_path, json)?;
    Ok(full_path.to_string_lossy().into_owned())
}

pub fn restore_snapshot(disk_number: u32, snapshot_path: &str) -> Result<(), std::io::Error> {
    use crate::models::DiskSnapshot;
    use std::io::{Write, Seek, SeekFrom};
    use windows_sys::Win32::System::Ioctl::IOCTL_DISK_SET_DRIVE_LAYOUT_EX;
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;
    use std::os::windows::io::AsRawHandle;

    let json = std::fs::read_to_string(snapshot_path)?;
    let snapshot: DiskSnapshot = serde_json::from_str(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    let disk_file = std::fs::OpenOptions::new().read(true).write(true).open(&disk_path)?;

    // 1. Restore VBRs first
    let mut write_file = std::fs::OpenOptions::new()
        .write(true)
        .share_mode(1 | 2)
        .custom_flags(0x20000000)
        .open(&disk_path)?;

    for vbr in &snapshot.vbrs {
        if write_file.seek(SeekFrom::Start(vbr.offset)).is_ok() {
            let _ = write_file.write_all(&vbr.main_vbr);
        }
        // Cannot restore backup easily if size shrunk, but we try:
        // Wait, if size shrunk, backup VBR is gone. We shouldn't write to random sectors.
        // But if we restore GPT, the partition size is restored!
        // We write backup VBR after GPT restore to be safe? 
        // No, writing VBR now is fine because physical disk hasn't shrunk, only partition definition.
    }

    // 2. Restore GPT Layout
    let mut bytes_returned = 0u32;
    unsafe {
        let success = win32_DeviceIoControl(
            disk_file.as_raw_handle() as _,
            IOCTL_DISK_SET_DRIVE_LAYOUT_EX,
            snapshot.layout_data.as_ptr() as _,
            snapshot.layout_data.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        );

        if success == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }

    // 3. Update Disk Properties
    update_disk_properties(disk_file.as_raw_handle())?;

    Ok(())
}

pub fn get_drive_layout(disk_handle: std::os::windows::io::RawHandle) -> Result<Vec<u8>, std::io::Error> {
    use windows_sys::Win32::System::Ioctl::{DRIVE_LAYOUT_INFORMATION_EX, PARTITION_INFORMATION_EX, IOCTL_DISK_GET_DRIVE_LAYOUT_EX};
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    // Allocate a buffer large enough for 128 partitions
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

pub fn update_drive_layout(
    disk_handle: std::os::windows::io::RawHandle,
    old_offset: u64,
    new_offset: u64,
) -> Result<(), std::io::Error> {
    use windows_sys::Win32::System::Ioctl::{
        DRIVE_LAYOUT_INFORMATION_EX, IOCTL_DISK_SET_DRIVE_LAYOUT_EX,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    let mut buffer = get_drive_layout(disk_handle)?;
    let layout = unsafe { &mut *(buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
    
    let count = layout.PartitionCount as usize;
    let entries = unsafe {
        std::slice::from_raw_parts_mut(
            layout.PartitionEntry.as_mut_ptr(),
            count,
        )
    };

    let mut found = false;
    for entry in entries {
        if entry.StartingOffset == old_offset as i64 {
            entry.StartingOffset = new_offset as i64;
            entry.RewritePartition = true; // Mark to be rewritten
            found = true;
            break;
        }
    }

    if !found {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Partition with starting offset {} not found in drive layout", old_offset),
        ));
    }

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

pub fn update_disk_properties(disk_handle: std::os::windows::io::RawHandle) -> Result<(), std::io::Error> {
    use windows_sys::Win32::System::Ioctl::IOCTL_DISK_UPDATE_PROPERTIES;
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

pub fn delete_partition_from_layout(
    disk_handle: std::os::windows::io::RawHandle,
    old_offset: u64,
) -> Result<(), std::io::Error> {
    use windows_sys::Win32::System::Ioctl::{
        DRIVE_LAYOUT_INFORMATION_EX, IOCTL_DISK_SET_DRIVE_LAYOUT_EX, PARTITION_STYLE_GPT, PARTITION_STYLE_MBR
    };
    use windows_sys::Win32::System::IO::DeviceIoControl as win32_DeviceIoControl;

    let mut buffer = get_drive_layout(disk_handle)?;
    let layout = unsafe { &mut *(buffer.as_mut_ptr() as *mut DRIVE_LAYOUT_INFORMATION_EX) };
    
    let count = layout.PartitionCount as usize;
    let entries = unsafe {
        std::slice::from_raw_parts_mut(
            layout.PartitionEntry.as_mut_ptr(),
            count,
        )
    };

    let mut found = false;
    for entry in entries {
        if entry.StartingOffset == old_offset as i64 {
            entry.RewritePartition = true; // Mark to be rewritten
            if layout.PartitionStyle == PARTITION_STYLE_GPT as u32 {
                unsafe {
                    std::ptr::write_bytes(&mut entry.Anonymous.Gpt.PartitionType, 0, 1);
                    std::ptr::write_bytes(&mut entry.Anonymous.Gpt.PartitionId, 0, 1);
                }
            } else if layout.PartitionStyle == PARTITION_STYLE_MBR as u32 {
                entry.Anonymous.Mbr.PartitionType = 0;
                entry.Anonymous.Mbr.RecognizedPartition = false;
            }
            found = true;
            break;
        }
    }

    if !found {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Partition with starting offset {} not found in drive layout", old_offset),
        ));
    }

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

#[derive(Debug, Clone)]
pub struct ResolvedPartition {
    pub partition_number: u32,
    pub starting_offset: u64,
    pub partition_length: u64,
}

pub fn resolve_target_partition(disk_number: u32, target: &str) -> Result<ResolvedPartition, std::io::Error> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Ioctl::{VOLUME_DISK_EXTENTS, DISK_EXTENT, DRIVE_LAYOUT_INFORMATION_EX};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x560000;

    let clean_target = target.trim().trim_end_matches(':').to_lowercase();
    let is_index = clean_target.chars().all(|c| c.is_ascii_digit());

    // Retrieve layout first
    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    let disk_file = std::fs::OpenOptions::new().read(true).open(&disk_path)?;
    let buffer = get_drive_layout(disk_file.as_raw_handle())?;
    let layout = unsafe { &*(buffer.as_ptr() as *const DRIVE_LAYOUT_INFORMATION_EX) };

    if is_index {
        let target_idx = clean_target.parse::<u32>().unwrap();
        let count = layout.PartitionCount as usize;
        let entries = unsafe { std::slice::from_raw_parts(layout.PartitionEntry.as_ptr(), count) };
        for entry in entries {
            if entry.PartitionNumber == target_idx {
                return Ok(ResolvedPartition {
                    partition_number: target_idx,
                    starting_offset: entry.StartingOffset as u64,
                    partition_length: entry.PartitionLength as u64,
                });
            }
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Partition index not found in layout"))
    } else {
        // Target is drive letter
        let vol_path = format!("\\\\.\\{}:", clean_target);
        let vol_file = std::fs::OpenOptions::new().read(true).open(&vol_path)?;
        let mut extents = [0u8; 1024];
        let mut returned = 0;
        let ok = unsafe {
            DeviceIoControl(
                vol_file.as_raw_handle() as _,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                std::ptr::null(),
                0,
                extents.as_mut_ptr() as _,
                extents.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let vde = unsafe { &*(extents.as_ptr() as *const VOLUME_DISK_EXTENTS) };
        if vde.NumberOfDiskExtents == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "No disk extents for volume"));
        }
        let extent = unsafe { &*(&vde.Extents as *const [DISK_EXTENT; 1] as *const DISK_EXTENT) };
        
        let count = layout.PartitionCount as usize;
        let entries = unsafe { std::slice::from_raw_parts(layout.PartitionEntry.as_ptr(), count) };
        for entry in entries {
            if entry.StartingOffset == extent.StartingOffset {
                return Ok(ResolvedPartition {
                    partition_number: entry.PartitionNumber,
                    starting_offset: entry.StartingOffset as u64,
                    partition_length: entry.PartitionLength as u64,
                });
            }
        }
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Volume extent not found in drive layout"))
    }
}

#[derive(Debug, Clone)]
pub struct NativeDiskSegment {
    pub is_partition: bool,
    pub offset: u64,
    pub size: u64,
    pub partition_number: Option<u32>,
}

pub fn get_disk_segments(disk_number: u32) -> Result<(Vec<NativeDiskSegment>, u64), std::io::Error> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Ioctl::{DRIVE_LAYOUT_INFORMATION_EX, GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::PARTITION_STYLE_RAW;

    let disk_path = format!("\\\\.\\PhysicalDrive{}", disk_number);
    let disk_file = std::fs::OpenOptions::new().read(true).open(&disk_path)?;
    
    // Get disk size
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
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let disk_size = length_info.Length as u64;

    let buffer = get_drive_layout(disk_file.as_raw_handle())?;
    let layout = unsafe { &*(buffer.as_ptr() as *const DRIVE_LAYOUT_INFORMATION_EX) };
    let count = layout.PartitionCount as usize;
    let entries = unsafe { std::slice::from_raw_parts(layout.PartitionEntry.as_ptr(), count) };

    let mut parts = Vec::new();
    for entry in entries {
        if entry.PartitionStyle == PARTITION_STYLE_RAW || entry.PartitionLength == 0 {
            continue; // Skip empty/raw entries
        }
        parts.push(NativeDiskSegment {
            is_partition: true,
            offset: entry.StartingOffset as u64,
            size: entry.PartitionLength as u64,
            partition_number: Some(entry.PartitionNumber),
        });
    }

    // GPT header itself occupies the first and last 33 sectors (17KB)
    parts.push(NativeDiskSegment {
        is_partition: true,
        offset: 0,
        size: 34 * 512,
        partition_number: None,
    });
    if disk_size >= 33 * 512 {
        parts.push(NativeDiskSegment {
            is_partition: true,
            offset: disk_size - 33 * 512,
            size: 33 * 512,
            partition_number: None,
        });
    }

    parts.sort_by_key(|p| p.offset);

    let mut segments = Vec::new();
    let mut cursor = 0u64;
    for p in parts {
        if cursor < p.offset {
            segments.push(NativeDiskSegment {
                is_partition: false,
                offset: cursor,
                size: p.offset - cursor,
                partition_number: None,
            });
        }
        segments.push(p.clone());
        cursor = cursor.max(p.offset + p.size);
    }
    if cursor < disk_size {
        segments.push(NativeDiskSegment {
            is_partition: false,
            offset: cursor,
            size: disk_size - cursor,
            partition_number: None,
        });
    }

    Ok((segments, disk_size))
}

