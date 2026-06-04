use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskState {
    pub disks: Vec<DiskInfo>,
    pub queue: Vec<Operation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskInfo {
    pub id: String,
    pub label: String,
    pub model: String,
    pub size_gb: f64,
    pub style: DiskStyle,
    pub status: String,
    pub partitions: Vec<Partition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DiskStyle {
    Mbr,
    Gpt,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Partition {
    pub index: u32,
    pub label: String,
    pub letter: Option<String>,
    pub file_system: String,
    pub size_gb: f64,
    pub used_gb: f64,
    pub kind: PartitionKind,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PartitionKind {
    System,
    Primary,
    Logical,
    Unallocated,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Operation {
    pub id: u64,
    pub op_type: String,
    pub target: String,
    pub details: String,
    pub status: OperationStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OperationStatus {
    Pending,
    Applied,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PlanAction {
    List,
    Resize { target: String, size_gb: f64 },
    Move { target: String, delta_mib: i64 },
    Create {
        fs: String,
        size_gb: f64,
        label: Option<String>,
        drive_letter: Option<String>,
    },
    Delete { target: String },
    Convert { from: String, to: String, disk: String },
    Assign { target: String, letter: String },
    Format {
        target: String,
        fs: String,
        label: Option<String>,
    },
    Clone { source: String, target: String },
    Wipe { disk: String },
    Activate { target: String },
    Restore { disk_number: u32, snapshot_path: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlanInputs {
    pub action: Option<PlanAction>,
    pub disk: String,
    pub test_vhd: Option<String>,
    pub restore_gpt: Option<String>,
    pub dry_run: bool,
    pub move_unallocated_mib: Option<u64>,
    pub move_sectors: Option<u64>,
    pub extend_index: Option<u32>,
    pub move_index: Option<u32>,
    pub free_before_index: Option<u32>,
    pub free_after_index: Option<u32>,
    pub source_unalloc_lba: Option<u64>,
    pub chunk_mib: u64,
    pub keep_traces: bool,
    pub wipe_old_data: bool,
    pub system_letter: String,
    pub data_letter: String,
    pub out_dir: String,
    pub force_resume: bool,
    pub operation_type: Option<String>,
    pub target_lba: Option<u64>,
    pub target_sectors: Option<u64>,
    pub format_fs: Option<String>,
    #[serde(default)]
    pub allow_battery: bool,
}

impl Default for PlanInputs {
    fn default() -> Self {
        let out_dir = std::env::temp_dir().to_string_lossy().to_string();
        Self {
            action: None,
            disk: r"\\.\PhysicalDrive0".to_string(),
            test_vhd: None,
            restore_gpt: None,
            dry_run: false,
            move_unallocated_mib: None,
            move_sectors: None,
            extend_index: None,
            move_index: None,
            free_before_index: None,
            free_after_index: None,
            source_unalloc_lba: None,
            chunk_mib: 64,
            keep_traces: false,
            wipe_old_data: false,
            system_letter: "C".to_string(),
            data_letter: "D".to_string(),
            out_dir,
            force_resume: false,
            operation_type: None,
            target_lba: None,
            target_sectors: None,
            format_fs: None,
            allow_battery: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Plan {
    pub operation_type: String,
    pub inputs: PlanInputs,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApplyReport {
    pub ok: bool,
    pub dry_run: bool,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskSnapshot {
    pub layout_data: Vec<u8>,
    pub vbrs: Vec<PartitionVbr>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartitionVbr {
    pub offset: u64,
    pub main_vbr: Vec<u8>,
    pub backup_vbr: Vec<u8>,
}
