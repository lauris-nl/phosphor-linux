use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::pm3::connection;
use crate::state::{WizardAction, WizardMachine, WizardState};

#[tauri::command]
pub async fn detect_device(
    app: AppHandle,
    machine: State<'_, Mutex<WizardMachine>>,
) -> Result<WizardState, AppError> {
    // Transition to DetectingDevice
    {
        let mut m = machine.lock().map_err(|e| {
            AppError::CommandFailed(format!("State lock poisoned: {}", e))
        })?;
        m.transition(WizardAction::StartDetection)?;
    }

    match connection::detect_device(&app).await {
        Ok((port, model, firmware)) => {
            let mut m = machine.lock().map_err(|e| {
                AppError::CommandFailed(format!("State lock poisoned: {}", e))
            })?;
            m.transition(WizardAction::DeviceFound {
                port,
                model,
                firmware,
            })?;
            Ok(m.current.clone())
        }
        Err(e) => {
            let err_msg = e.to_string();
            let (user_message, recovery_action) = match &e {
                AppError::ClientRequired => (
                    "Proxmark3 client required. Locate a current RRG/Iceman client to continue."
                        .to_string(),
                    Some(crate::cards::types::RecoveryAction::Manual),
                ),
                AppError::ClientInvalid(_) => (
                    "Selected file is not a compatible Proxmark3 client.".to_string(),
                    Some(crate::cards::types::RecoveryAction::Manual),
                ),
                AppError::SerialPermissionDenied(_) => (
                    "Permission denied opening the Proxmark3 serial port. Check your udev rules or dialout/uucp group."
                        .to_string(),
                    Some(crate::cards::types::RecoveryAction::Manual),
                ),
                _ => (
                    "No Proxmark3 reader found. Check the USB connection.".to_string(),
                    Some(crate::cards::types::RecoveryAction::Retry),
                ),
            };
            let mut m = machine.lock().map_err(|e| {
                AppError::CommandFailed(format!("State lock poisoned: {}", e))
            })?;
            m.transition(WizardAction::ReportError {
                message: err_msg,
                user_message,
                recoverable: true,
                recovery_action,
            })?;
            Ok(m.current.clone())
        }
    }
}
