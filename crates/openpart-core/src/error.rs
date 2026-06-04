use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpenPartError {
    #[error("no operation was specified")]
    NoOperation,
    #[error("invalid plan: {0}")]
    InvalidPlan(String),
    #[error("real disk operations are not implemented in this reconstruction")]
    RealOpsUnavailable,
    #[error("real disk writes are not allowed; set OPENPART_ALLOW_WRITE=1 to enable")]
    WriteNotAllowed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
