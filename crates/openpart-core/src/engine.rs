use crate::error::OpenPartError;
use crate::models::{Plan, PlanAction, PlanInputs, DiskState, Partition, PartitionKind};

pub fn build_plan(inputs: PlanInputs) -> Result<Plan, OpenPartError> {
    let operation_type = infer_operation_type(&inputs);
    if operation_type == "noop" {
        return Err(OpenPartError::NoOperation);
    }

    let mut steps = Vec::new();
    let mut warnings = Vec::new();

    steps.push(format!("Load disk {}", inputs.disk));

    if let Some(action) = &inputs.action {
        apply_action(action, &mut steps);
    } else {
        if let Some(path) = &inputs.test_vhd {
            steps.push(format!("Use test VHD {}", path));
        }
        if let Some(path) = &inputs.restore_gpt {
            steps.push(format!("Restore GPT from {}", path));
        }

        if let Some(mib) = inputs.move_unallocated_mib {
            steps.push(format!("Move unallocated space by {} MiB", mib));
        }
        if let Some(sectors) = inputs.move_sectors {
            steps.push(format!("Move sectors by {}", sectors));
        }
        if let Some(index) = inputs.extend_index {
            steps.push(format!("Extend partition index {}", index));
        }
        if let Some(index) = inputs.move_index {
            steps.push(format!("Move partition index {}", index));
        }
        if let Some(index) = inputs.free_before_index {
            steps.push(format!("Free space before partition index {}", index));
        }
        if let Some(index) = inputs.free_after_index {
            steps.push(format!("Free space after partition index {}", index));
        }
        if let Some(lba) = inputs.source_unalloc_lba {
            steps.push(format!("Use source unallocated LBA {}", lba));
        }
        if let Some(lba) = inputs.target_lba {
            steps.push(format!("Target LBA {}", lba));
        }
        if let Some(sectors) = inputs.target_sectors {
            steps.push(format!("Target sectors {}", sectors));
        }
        if let Some(fs) = &inputs.format_fs {
            steps.push(format!("Format filesystem {}", fs));
        }
    }

    steps.push(format!("Chunk size {} MiB", inputs.chunk_mib));

    if inputs.wipe_old_data {
        warnings.push("Wipe old data requested".to_string());
    }
    if let Some(PlanAction::Move { .. }) = &inputs.action {
        warnings.push("Physical partition shifting (Move) is simulated. Windows does not support native in-place partition moving without risking data loss. No raw sectors were copied.".to_string());
    }

    Ok(Plan {
        operation_type,
        inputs,
        steps,
        warnings,
    })
}

fn infer_operation_type(inputs: &PlanInputs) -> String {
    if let Some(action) = &inputs.action {
        return action_name(action).to_string();
    }
    if let Some(op) = &inputs.operation_type {
        return op.clone();
    }
    if inputs.restore_gpt.is_some() {
        return "restore-gpt".to_string();
    }
    if inputs.test_vhd.is_some() {
        return "test-vhd".to_string();
    }
    if inputs.format_fs.is_some() {
        return "format".to_string();
    }
    if inputs.move_unallocated_mib.is_some()
        || inputs.move_sectors.is_some()
        || inputs.move_index.is_some()
    {
        return "move".to_string();
    }
    if inputs.extend_index.is_some()
        || inputs.free_before_index.is_some()
        || inputs.free_after_index.is_some()
    {
        return "resize".to_string();
    }
    "noop".to_string()
}

fn apply_action(action: &PlanAction, steps: &mut Vec<String>) {
    match action {
        PlanAction::List => {
            steps.push("Scan disk topology".to_string());
        }
        PlanAction::Resize { target, size_gb } => {
            steps.push(format!("Resize {} to {:.2} GB", target, size_gb));
        }
        PlanAction::Move { target, delta_mib } => {
            steps.push(format!(
                "Move {} by {} MiB",
                target,
                delta_mib
            ));
        }
        PlanAction::Create { fs, size_gb, label, drive_letter } => {
            let label = label.as_deref().unwrap_or("Unlabeled");
            let letter = drive_letter.as_deref().unwrap_or("Auto");
            steps.push(format!(
                "Create {} {:.2} GB partition (label {}, letter {})",
                fs, size_gb, label, letter
            ));
        }
        PlanAction::Delete { target } => {
            steps.push(format!("Delete partition {}", target));
        }
        PlanAction::Convert { from, to, disk } => {
            steps.push(format!("Convert {} from {} to {}", disk, from, to));
        }
        PlanAction::Assign { target, letter } => {
            steps.push(format!("Assign drive letter {} to {}", letter, target));
        }
        PlanAction::Format { target, fs, label } => {
            let label = label.as_deref().unwrap_or("Unlabeled");
            steps.push(format!("Format {} as {} ({})", target, fs, label));
        }
        PlanAction::Clone { source, target } => {
            steps.push(format!("Clone {} to {}", source, target));
        }
        PlanAction::Wipe { disk } => {
            steps.push(format!("Wipe disk {}", disk));
        }
        PlanAction::Activate { target } => {
            steps.push(format!("Will mark {} as active partition.", target));
        }
        PlanAction::Restore { disk_number, snapshot_path } => {
            steps.push(format!("Will restore disk {} from snapshot {}.", disk_number, snapshot_path));
        }
    }
}



fn action_name(action: &PlanAction) -> &'static str {
    match action {
        PlanAction::List => "list",
        PlanAction::Resize { .. } => "resize",
        PlanAction::Move { .. } => "move",
        PlanAction::Create { .. } => "create",
        PlanAction::Delete { .. } => "delete",
        PlanAction::Convert { .. } => "convert",
        PlanAction::Assign { .. } => "assign",
        PlanAction::Format { .. } => "format",
        PlanAction::Clone { .. } => "clone",
        PlanAction::Wipe { .. } => "wipe",
        PlanAction::Activate { .. } => "active",
        PlanAction::Restore { .. } => "restore",
    }
}

fn match_disk_id(id_a: &str, id_b: &str) -> bool {
    let clean_a = id_a.replace('\\', "").to_lowercase();
    let clean_b = id_b.replace('\\', "").to_lowercase();
    clean_a == clean_b
}

pub fn compose_plan_state(mut state: DiskState, queue: Vec<PlanInputs>) -> Result<DiskState, OpenPartError> {
    for inputs in queue {
        if let Some(action) = inputs.action {
            let mut found_index = None;
            for (idx, disk) in state.disks.iter().enumerate() {
                if disk.id == inputs.disk || match_disk_id(&disk.id, &inputs.disk) {
                    found_index = Some(idx);
                    break;
                }
            }
            let target_index = found_index.or_else(|| {
                if state.disks.len() == 1 {
                    Some(0)
                } else {
                    None
                }
            });
            if let Some(idx) = target_index {
                if let Some(disk) = state.disks.get_mut(idx) {
                    simulate_action(&mut disk.partitions, &action, inputs.source_unalloc_lba);
                    coalesce_unallocated(&mut disk.partitions);
                }
            }
        }
    }
    Ok(state)
}

fn simulate_action(partitions: &mut Vec<Partition>, action: &PlanAction, source_unalloc_lba: Option<u64>) {
    match action {
        PlanAction::Delete { target } => {
            let clean_target = target.trim().trim_end_matches(':');
            if let Some(pos) = partitions.iter().position(|p| {
                if let Some(letter) = &p.letter {
                    if letter.eq_ignore_ascii_case(clean_target) {
                        return true;
                    }
                }
                p.index.to_string() == clean_target
            }) {
                partitions[pos].label = "Unallocated".to_string();
                partitions[pos].letter = None;
                partitions[pos].file_system = "".to_string();
                partitions[pos].used_gb = 0.0;
                partitions[pos].kind = PartitionKind::Unallocated;
                partitions[pos].status = "Unallocated".to_string();
            }
        }
        PlanAction::Create { fs, size_gb, label, drive_letter } => {
            let target_pos = if let Some(target_idx) = source_unalloc_lba {
                partitions.iter().position(|p| {
                    matches!(p.kind, PartitionKind::Unallocated) && p.index == target_idx as u32
                })
            } else {
                None
            };
            
            let pos_opt = target_pos.or_else(|| {
                partitions.iter().position(|p| {
                    matches!(p.kind, PartitionKind::Unallocated) && (*size_gb <= 0.0 || p.size_gb + 0.01 >= *size_gb)
                })
            });

            if let Some(pos) = pos_opt {
                let unalloc_size = partitions[pos].size_gb;
                let new_size = if *size_gb <= 0.0 { unalloc_size } else { *size_gb };
                
                let assigned_letter = match drive_letter {
                    Some(letter) if !letter.trim().is_empty() => Some(letter.clone()),
                    _ => {
                        let mut next_letter = None;
                        for ch in b'D'..=b'Z' {
                            let letter_str = (ch as char).to_string();
                            let is_used = partitions.iter().any(|p| {
                                p.letter.as_ref().map(|l| l.to_ascii_uppercase()) == Some(letter_str.clone())
                            });
                            if !is_used {
                                next_letter = Some(letter_str);
                                break;
                            }
                        }
                        next_letter
                    }
                };

                let new_part = Partition {
                    index: get_next_index(partitions),
                    label: label.clone().unwrap_or_else(|| "New Volume".to_string()),
                    letter: assigned_letter,
                    file_system: fs.clone(),
                    size_gb: new_size,
                    used_gb: 0.0,
                    kind: PartitionKind::Primary,
                    status: "Pending".to_string(),
                };

                if new_size >= unalloc_size {
                    partitions[pos] = new_part;
                } else {
                    partitions[pos].size_gb = ((unalloc_size - new_size) * 1000.0).round() / 1000.0;
                    partitions.insert(pos, new_part);
                }
            }
        }
        PlanAction::Resize { target, size_gb } => {
            let clean_target = target.trim().trim_end_matches(':');
            if let Some(pos) = partitions.iter().position(|p| {
                if let Some(letter) = &p.letter {
                    if letter.eq_ignore_ascii_case(clean_target) {
                        return true;
                    }
                }
                p.index.to_string() == clean_target
            }) {
                let current_size = partitions[pos].size_gb;
                let target_size = *size_gb;
                let diff = target_size - current_size;

                if diff != 0.0 {
                    partitions[pos].size_gb = target_size;
                    
                    if diff > 0.0 {
                        let mut consumed = false;
                        if pos + 1 < partitions.len() && matches!(partitions[pos + 1].kind, PartitionKind::Unallocated) {
                            let unalloc_size = partitions[pos + 1].size_gb;
                            if unalloc_size >= diff {
                                partitions[pos + 1].size_gb = unalloc_size - diff;
                                consumed = true;
                            } else {
                                partitions[pos + 1].size_gb = 0.0;
                                let remaining = diff - unalloc_size;
                                if pos > 0 && matches!(partitions[pos - 1].kind, PartitionKind::Unallocated) {
                                    let prev_unalloc_size = partitions[pos - 1].size_gb;
                                    partitions[pos - 1].size_gb = (prev_unalloc_size - remaining).max(0.0);
                                }
                                consumed = true;
                            }
                        }
                        
                        if !consumed && pos > 0 && matches!(partitions[pos - 1].kind, PartitionKind::Unallocated) {
                            let unalloc_size = partitions[pos - 1].size_gb;
                            partitions[pos - 1].size_gb = (unalloc_size - diff).max(0.0);
                        }
                    } else {
                        let shrink_amount = -diff;
                        if pos + 1 < partitions.len() && matches!(partitions[pos + 1].kind, PartitionKind::Unallocated) {
                            partitions[pos + 1].size_gb += shrink_amount;
                        } else {
                            let unalloc_part = Partition {
                                index: get_next_index(partitions),
                                label: "Unallocated".to_string(),
                                letter: None,
                                file_system: "".to_string(),
                                size_gb: shrink_amount,
                                used_gb: 0.0,
                                kind: PartitionKind::Unallocated,
                                status: "Unallocated".to_string(),
                            };
                            partitions.insert(pos + 1, unalloc_part);
                        }
                    }
                }
            }
        }
        PlanAction::Format { target, fs, label } => {
            let clean_target = target.trim().trim_end_matches(':');
            if let Some(p) = partitions.iter_mut().find(|p| {
                if let Some(letter) = &p.letter {
                    if letter.eq_ignore_ascii_case(clean_target) {
                        return true;
                    }
                }
                p.index.to_string() == clean_target
            }) {
                p.file_system = fs.clone();
                if let Some(lbl) = label {
                    p.label = lbl.clone();
                }
            }
        }
        PlanAction::Assign { target, letter } => {
            let clean_target = target.trim().trim_end_matches(':');
            if let Some(p) = partitions.iter_mut().find(|p| {
                if let Some(letter) = &p.letter {
                    if letter.eq_ignore_ascii_case(clean_target) {
                        return true;
                    }
                }
                p.index.to_string() == clean_target
            }) {
                p.letter = Some(letter.clone());
            }
        }
        PlanAction::Move { target, delta_mib } => {
            let clean_target = target.trim().trim_end_matches(':');
            if let Some(pos) = partitions.iter().position(|p| {
                if let Some(letter) = &p.letter {
                    if letter.eq_ignore_ascii_case(clean_target) {
                        return true;
                    }
                }
                p.index.to_string() == clean_target
            }) {
                let delta_gb = *delta_mib as f64 / 1024.0;
                if delta_gb > 0.0 {
                    // Moving right: decrease space after, increase space before
                    if pos + 1 < partitions.len() && matches!(partitions[pos + 1].kind, PartitionKind::Unallocated) {
                        let after_size = partitions[pos + 1].size_gb;
                        partitions[pos + 1].size_gb = (after_size - delta_gb).max(0.0);
                    }
                    if pos > 0 && matches!(partitions[pos - 1].kind, PartitionKind::Unallocated) {
                        partitions[pos - 1].size_gb += delta_gb;
                    } else {
                        let new_unalloc = Partition {
                            index: get_next_index(partitions),
                            label: "Unallocated".to_string(),
                            letter: None,
                            file_system: "".to_string(),
                            size_gb: delta_gb,
                            used_gb: 0.0,
                            kind: PartitionKind::Unallocated,
                            status: "Unallocated".to_string(),
                        };
                        partitions.insert(pos, new_unalloc);
                    }
                } else if delta_gb < 0.0 {
                    // Moving left: decrease space before, increase space after
                    let shift_left = -delta_gb;
                    if pos > 0 && matches!(partitions[pos - 1].kind, PartitionKind::Unallocated) {
                        let before_size = partitions[pos - 1].size_gb;
                        partitions[pos - 1].size_gb = (before_size - shift_left).max(0.0);
                    }
                    if pos + 1 < partitions.len() && matches!(partitions[pos + 1].kind, PartitionKind::Unallocated) {
                        partitions[pos + 1].size_gb += shift_left;
                    } else {
                        let new_unalloc = Partition {
                            index: get_next_index(partitions),
                            label: "Unallocated".to_string(),
                            letter: None,
                            file_system: "".to_string(),
                            size_gb: shift_left,
                            used_gb: 0.0,
                            kind: PartitionKind::Unallocated,
                            status: "Unallocated".to_string(),
                        };
                        partitions.insert(pos + 1, new_unalloc);
                    }
                }
            }
        }
        _ => {}
    }
}

fn get_next_index(partitions: &[Partition]) -> u32 {
    let max_index = partitions.iter().map(|p| p.index).max().unwrap_or(0);
    if max_index >= 1_000_000 {
        max_index + 1
    } else {
        1_000_000.max(max_index + 1)
    }
}

fn coalesce_unallocated(partitions: &mut Vec<Partition>) {
    let mut merged: Vec<Partition> = Vec::new();
    for part in partitions.drain(..) {
        if matches!(part.kind, PartitionKind::Unallocated) {
            if part.size_gb < 0.001 {
                continue;
            }
            if let Some(last) = merged.last_mut() {
                if matches!(last.kind, PartitionKind::Unallocated) {
                    last.size_gb = ((last.size_gb + part.size_gb) * 1000.0).round() / 1000.0;
                    continue;
                }
            }
        }
        merged.push(part);
    }

    // Re-assign virtual indices to unallocated partitions starting from 1_000_000
    let mut virtual_index = 1_000_000u32;
    for part in &mut merged {
        if matches!(part.kind, PartitionKind::Unallocated) {
            part.index = virtual_index;
            virtual_index = virtual_index.saturating_add(1);
        }
    }

    *partitions = merged;
}

