use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum AppError {
    #[error("Proxmark3 client configuration required")]
    ClientRequired,
    #[error("Selected file is not a compatible Proxmark3 client: {0}")]
    ClientInvalid(String),
    #[error("Proxmark3 serial port permission denied: {0}")]
    SerialPermissionDenied(String),
    #[error("PM3 not found on any port")]
    DeviceNotFound,
    #[error("PM3 command failed: {0}")]
    CommandFailed(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::DatabaseError(e.to_string())
    }
}
