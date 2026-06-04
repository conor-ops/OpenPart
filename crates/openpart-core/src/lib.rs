pub mod engine;
pub mod error;
pub mod executor;
pub mod models;
pub mod scanner;

pub use crate::engine::build_plan;
pub use crate::error::OpenPartError;
pub use crate::executor::{Executor, RealExecutor, SimulatedExecutor, lock_volume_by_offset};
// mock state removed: live scanner is used instead
pub use crate::models::{
    ApplyReport, DiskInfo, DiskState, DiskStyle, Operation, OperationStatus,
    Partition, PartitionKind, Plan, PlanAction, PlanInputs,
};
