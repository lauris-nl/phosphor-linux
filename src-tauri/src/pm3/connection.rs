use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use regex::Regex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

use crate::error::AppError;
use crate::pm3::client;
use crate::pm3::output_parser::strip_ansi;

/// Payload emitted as `pm3-output` events for the live terminal panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pm3OutputPayload {
    pub text: String,
    pub is_error: bool,
}

/// Emit raw PM3 output to the frontend terminal panel.
pub fn emit_output(app: &AppHandle, text: &str, is_error: bool) {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _ = app.emit(
            "pm3-output",
            Pm3OutputPayload {
                text: trimmed.to_string(),
                is_error,
            },
        );
    }
}

/// Maximum time to wait for a PM3 subprocess to complete (30 seconds).
const PM3_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Validates that a port string matches expected serial port patterns.
/// Accepts COM1-COM256+ (Windows), /dev/ttyACM0-99, /dev/ttyUSB0-99 (Linux),
/// and /dev/tty.usbmodem* (macOS).
static PORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(COM[1-9]\d*|/dev/tty(ACM|USB)\d{1,2}|/dev/tty\.usbmodem\w+|/dev/serial/by-id/[A-Za-z0-9._:+-]+)$",
    )
        .expect("bad port regex")
});

/// Validate a serial-port argument consistently for every backend command.
/// Persistent Linux `/dev/serial/by-id/...` names are intentionally accepted;
/// callers must not impose shorter arbitrary length limits on them.
pub fn validate_port(port: &str) -> Result<(), AppError> {
    if PORT_RE.is_match(port) {
        Ok(())
    } else {
        Err(AppError::CommandFailed(format!("Invalid port: {}", port)))
    }
}

/// Whether the OS device node is currently present. This is used only to
/// distinguish an actual Unix device disappearance from a PM3 command error.
/// COM ports cannot be checked with filesystem metadata, so they remain
/// present until a command produces a more specific transport diagnostic.
pub fn port_is_present(port: &str) -> bool {
    !port.starts_with("/dev/") || Path::new(port).exists()
}

/// Legacy compatibility mode for readers that still run pre-PM3a firmware.
/// Those clients take the serial port as argv[1] instead of the modern
/// `-p PORT` pair. Native launchers use the modern form unless explicitly
/// configured otherwise.
fn use_legacy_cli() -> bool {
    matches!(
        std::env::var("PHOSPHOR_PM3_LEGACY").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn pm3_args(port: &str, cmd: &str) -> Vec<String> {
    if use_legacy_cli() {
        vec![port.into(), "-f".into(), "-c".into(), cmd.into()]
    } else {
        vec![
            "-p".into(),
            port.into(),
            "-f".into(),
            "-c".into(),
            cmd.into(),
        ]
    }
}

fn validate_command_input(port: &str, cmd: &str) -> Result<(), AppError> {
    validate_port(port)?;
    if cmd.contains(';')
        || cmd.contains('\n')
        || cmd.contains('\r')
        || port.contains(';')
        || port.contains('\n')
        || port.contains('\r')
    {
        return Err(AppError::CommandFailed(
            "Invalid characters in command".into(),
        ));
    }
    Ok(())
}

/// Preserve both process streams in non-zero-exit diagnostics. Current RRG
/// prints command results to stdout but can also write incidental diagnostics
/// to stderr; selecting stderr alone loses useful results such as the explicit
/// "no tag found" response.
fn process_error_detail(stdout: &str, stderr: &str) -> String {
    let stdout = strip_ansi(stdout).trim().to_string();
    let stderr = strip_ansi(stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{}\n{}", stdout, stderr),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => "no process output".to_string(),
    }
}

/// Internal PM3 execution that does NOT emit to the frontend.
/// Handles port/command validation, resolved-client execution, output collection,
/// ANSI stripping, and timeout. The executable and every argument are passed
/// directly to the OS; no command shell is involved.
/// Returns the cleaned output string on success.
async fn execute_pm3(app: &AppHandle, port: &str, cmd: &str) -> Result<String, AppError> {
    // PM3's `-c` accepts its own separators, so reject them before building
    // argv. The executable is never invoked through a shell.
    validate_command_input(port, cmd)?;

    let resolved = client::resolve_client(app).await?;
    let args = pm3_args(port, cmd);
    let mut command = Command::new(&resolved.path);
    command.args(&args).kill_on_drop(true);
    let output = match timeout(PM3_COMMAND_TIMEOUT, command.output()).await {
        Err(_) => {
            return Err(AppError::Timeout(format!(
                "PM3 command timed out after {}s: {}",
                PM3_COMMAND_TIMEOUT.as_secs(),
                cmd
            )))
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(AppError::SerialPermissionDenied(port.into()))
        }
        Ok(Err(e)) => {
            return Err(AppError::CommandFailed(format!(
                "Failed to start configured Proxmark3 client: {e}"
            )))
        }
        Ok(Ok(output)) => output,
    };

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    log::debug!(
        "PM3 subprocess exited: executable={}, code={}, stdout_bytes={}, stderr_bytes={}",
        resolved.path,
        code,
        output.stdout.len(),
        output.stderr.len()
    );

    match code {
        0 => Ok(strip_ansi(&stdout)),
        -5 | 251 => Err(AppError::Timeout(format!("PM3 timed out running: {cmd}"))),
        _ => {
            let detail = process_error_detail(&stdout, &stderr);
            let lower = detail.to_ascii_lowercase();
            if lower.contains("permission denied") || lower.contains("access denied") {
                Err(AppError::SerialPermissionDenied(port.into()))
            } else {
                Err(AppError::CommandFailed(format!(
                    "Exit code {code}: {detail}"
                )))
            }
        }
    }
}

/// Run a single PM3 command: spawns `proxmark3 -p {port} -f -c "{cmd}"`,
/// waits for the process to exit (with a 30-second timeout), then returns cleaned stdout.
/// If the subprocess hangs (e.g., USB cable pulled), it will be killed after the timeout.
///
/// Emits the command being run and its output to the frontend terminal panel.
///
/// **Known limitation -- subprocess cancellation on reset:**
/// The process is killed if the timeout future is dropped.
pub async fn run_command(app: &AppHandle, port: &str, cmd: &str) -> Result<String, AppError> {
    emit_output(app, &format!("pm3 --> {}", cmd), false);
    match execute_pm3(app, port, cmd).await {
        Ok(output) => {
            emit_output(app, &output, false);
            Ok(output)
        }
        Err(e) => {
            emit_output(app, &e.to_string(), true);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// HF Operation State — holds child process for cancellation + dump file path
// ---------------------------------------------------------------------------

/// Managed state for long-running HF operations (autopwn, dump, write).
/// Stored via `app.manage()` in `lib.rs`.
pub struct HfOperationState {
    /// Cancellation signal owned by the running Rust-side child task.
    pub cancel: Mutex<Option<oneshot::Sender<()>>>,
    /// Dump file path set by autopwn after completion (e.g. "hf-mf-01020304-dump.bin").
    pub dump_path: Mutex<Option<String>>,
}

impl HfOperationState {
    pub fn new() -> Self {
        Self {
            cancel: Mutex::new(None),
            dump_path: Mutex::new(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Streaming command execution (HF operations)
// ---------------------------------------------------------------------------

/// Run a PM3 command with streaming output, supporting long timeouts and
/// cancellation. Unlike `run_command()` which uses `.output()` (blocks until
/// exit), this uses `.spawn()` + async line reading.
///
/// - Each stdout/stderr line is emitted as a `pm3-output` event (live terminal).
/// - A per-line callback `on_line` is invoked for real-time parsing (e.g. autopwn
///   progress events). The callback receives the cleaned line text.
/// - The child process is stored in `hf_state.child` so `cancel_hf_operation`
///   can kill it mid-run.
/// - Returns the accumulated cleaned output on success.
pub async fn run_command_streaming<F>(
    app: &AppHandle,
    port: &str,
    cmd: &str,
    timeout_secs: u64,
    hf_state: &HfOperationState,
    mut on_line: F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    validate_command_input(port, cmd)?;

    emit_output(app, &format!("pm3 --> {}", cmd), false);

    // Use the same validated, absolute executable as non-streaming commands.
    let (rx, cancel) = spawn_pm3(app, port, cmd).await?;

    // Store child for cancellation
    {
        let mut lock = hf_state
            .cancel
            .lock()
            .map_err(|e| AppError::CommandFailed(format!("HF state lock poisoned: {}", e)))?;
        *lock = Some(cancel);
    }

    // Read lines with timeout
    let result = read_stream_with_timeout(app, rx, timeout_secs, &mut on_line).await;

    // Clear child on completion (process already exited or was killed)
    {
        let mut lock = hf_state.cancel.lock().unwrap_or_else(|e| e.into_inner());
        *lock = None;
    }

    match result {
        Ok(output) => Ok(output),
        Err(e) => {
            emit_output(app, &e.to_string(), true);
            Err(e)
        }
    }
}

/// Spawn PM3 directly with argv, returning streamed events and cancellation.
#[derive(Debug)]
enum Pm3ProcessEvent {
    Stdout(String),
    Stderr(String),
    Error(String),
    Terminated(Option<i32>),
}

async fn spawn_pm3(
    app: &AppHandle,
    port: &str,
    cmd: &str,
) -> Result<(mpsc::Receiver<Pm3ProcessEvent>, oneshot::Sender<()>), AppError> {
    let resolved = client::resolve_client(app).await?;
    let args = pm3_args(port, cmd);
    let mut child = Command::new(&resolved.path)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            AppError::CommandFailed(format!("Failed to start configured Proxmark3 client: {e}"))
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::CommandFailed("Cannot capture PM3 stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::CommandFailed("Cannot capture PM3 stderr".into()))?;
    let (event_tx, event_rx) = mpsc::channel(128);
    let (cancel_tx, mut cancel_rx) = oneshot::channel();

    let stdout_tx = event_tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let _ = stdout_tx.send(Pm3ProcessEvent::Stdout(line)).await;
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = stdout_tx.send(Pm3ProcessEvent::Error(e.to_string())).await;
                    break;
                }
            }
        }
    });
    let stderr_tx = event_tx.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let _ = stderr_tx.send(Pm3ProcessEvent::Stderr(line)).await;
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = stderr_tx.send(Pm3ProcessEvent::Error(e.to_string())).await;
                    break;
                }
            }
        }
    });
    tokio::spawn(async move {
        let status = tokio::select! {
            _ = &mut cancel_rx => {
                let _ = child.kill().await;
                child.wait().await.ok().and_then(|status| status.code())
            }
            status = child.wait() => status.ok().and_then(|status| status.code()),
        };
        // Deliver all buffered output before the termination event so parsers
        // never lose a short process's final status/result line.
        let _ = stdout_task.await;
        let _ = stderr_task.await;
        let _ = event_tx.send(Pm3ProcessEvent::Terminated(status)).await;
    });
    Ok((event_rx, cancel_tx))
}

/// Read from a `CommandEvent` receiver, accumulating output and emitting lines.
/// Returns the full cleaned output when the process terminates.
async fn read_stream_with_timeout<F>(
    app: &AppHandle,
    mut rx: mpsc::Receiver<Pm3ProcessEvent>,
    timeout_secs: u64,
    on_line: &mut F,
) -> Result<String, AppError>
where
    F: FnMut(&str),
{
    let deadline = Duration::from_secs(timeout_secs);
    let mut accumulated = String::new();
    let mut exit_code: Option<i32> = None;

    loop {
        match timeout(deadline, rx.recv()).await {
            Err(_) => {
                // Timeout expired
                return Err(AppError::Timeout(format!(
                    "HF operation timed out after {}s",
                    timeout_secs
                )));
            }
            Ok(None) => {
                // Channel closed — process exited
                break;
            }
            Ok(Some(event)) => match event {
                Pm3ProcessEvent::Stdout(line) => {
                    let cleaned = strip_ansi(&line);
                    let trimmed = cleaned.trim();
                    if !trimmed.is_empty() {
                        emit_output(app, trimmed, false);
                        on_line(trimmed);
                        accumulated.push_str(trimmed);
                        accumulated.push('\n');
                    }
                }
                Pm3ProcessEvent::Stderr(line) => {
                    let cleaned = strip_ansi(&line);
                    let trimmed = cleaned.trim();
                    if !trimmed.is_empty() {
                        emit_output(app, trimmed, true);
                        on_line(trimmed);
                        accumulated.push_str(trimmed);
                        accumulated.push('\n');
                    }
                }
                Pm3ProcessEvent::Error(msg) => {
                    emit_output(app, &msg, true);
                    return Err(AppError::CommandFailed(format!("Process error: {}", msg)));
                }
                Pm3ProcessEvent::Terminated(code) => {
                    exit_code = code;
                    break;
                }
            },
        }
    }

    // Check exit code
    match exit_code {
        Some(0) | None => Ok(accumulated),
        Some(-5) | Some(251) => Err(AppError::Timeout("PM3 subprocess timed out".into())),
        Some(code) => Err(AppError::CommandFailed(format!(
            "PM3 exited with code {}",
            code
        ))),
    }
}

/// Scan common COM/serial ports trying `hw version` to find a connected PM3.
/// Returns (port, model, firmware) on success.
///
/// Uses friendly, hacker-casual terminal output. All probe messages are green
/// (non-error) except the final "not found" message.
pub async fn detect_device(app: &AppHandle) -> Result<(String, String, String), AppError> {
    // Resolve client independently of reader enumeration. Otherwise a machine
    // with neither a client nor a connected reader would misleadingly report
    // only the missing reader.
    client::resolve_client(app).await?;
    let candidates = build_port_candidates();

    // Pick a random init message for personality
    let init_msgs = [
        "[=] Sniffing USB bus... come out, Proxmark",
        "[=] Deploying port tentacles...",
        "[=] Hunting for hardware... stay still",
        "[=] Scanning the wire... don't be shy",
        "[=] Reaching out to the other side...",
    ];
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize
        % init_msgs.len();
    emit_output(app, init_msgs[idx], false);

    for port in &candidates {
        emit_output(app, &format!("[=] Knocking on {}...", port), false);

        match execute_pm3(app, port, "hw version").await {
            Ok(output) => {
                if let Some((model, firmware)) = parse_hw_version(&output) {
                    emit_output(
                        app,
                        &format!("[+] Target acquired: {} on {}", model, port),
                        false,
                    );
                    emit_output(app, &format!("[+] Firmware: {}", firmware), false);
                    return Ok((port.clone(), model, firmware));
                }
                // Got output but couldn't parse hw version -- wrong device
                emit_output(app, &format!("[-] {} -- wrong device", port), false);
            }
            Err(e) => {
                // Capabilities mismatch means the PM3 device IS present on this
                // port but the firmware doesn't match the client version. Treat
                // it as a successful detection -- the firmware check step will
                // handle the mismatch and offer to flash.
                let err_msg = e.to_string();
                if err_msg.to_lowercase().contains("capabilities") {
                    emit_output(
                        app,
                        &format!(
                            "[+] Target acquired: Proxmark3 on {} (firmware mismatch)",
                            port
                        ),
                        false,
                    );
                    return Ok((
                        port.clone(),
                        "Proxmark3".to_string(),
                        "mismatched".to_string(),
                    ));
                }

                // Distinguish "no response" (spawn succeeded but device didn't respond)
                // from other errors. If spawn itself failed (binary not found), that
                // affects ALL ports, so propagate immediately.
                if err_msg.contains("Failed to spawn proxmark3") {
                    emit_output(
                        app,
                        "[!!] Proxmark3 binary not found. Check installation.",
                        true,
                    );
                    return Err(e);
                }

                emit_output(app, &format!("[-] {} -- no response", port), false);
            }
        }
    }

    emit_output(app, "[!!] No Proxmark3 found.", true);
    emit_output(
        app,
        "[=] Try a different USB cable (some are charge-only)",
        false,
    );
    emit_output(app, "[=] Check Device Manager for a COM port", false);
    emit_output(
        app,
        "[=] PM3 Easy: may need CH340 driver (wch-ic.com)",
        false,
    );
    Err(AppError::DeviceNotFound)
}

fn build_port_candidates() -> Vec<String> {
    let mut ports = Vec::new();

    if let Ok(configured) = std::env::var("PHOSPHOR_PM3_PORT") {
        if PORT_RE.is_match(&configured) {
            return vec![configured];
        }
    }

    if cfg!(target_os = "windows") {
        // Windows COM ports -- extend to 40 to cover USB hub reassignment
        for i in 1..=40 {
            ports.push(format!("COM{}", i));
        }
    } else if cfg!(target_os = "macos") {
        // macOS: /dev/tty.usbmodem* -- cover common PM3 suffixes
        for suffix in &["iceman1", "14101", "14201", "14301", "1", "2", "3"] {
            ports.push(format!("/dev/tty.usbmodem{}", suffix));
        }
    } else {
        // Linux: prefer persistent udev names and probe only entries whose
        // names identify a Proxmark, rather than opening unrelated devices.
        if let Ok(entries) = std::fs::read_dir("/dev/serial/by-id") {
            for entry in entries.flatten() {
                let value = entry.path().to_string_lossy().to_string();
                if value.to_ascii_lowercase().contains("proxmark") && PORT_RE.is_match(&value) {
                    ports.push(value);
                }
            }
        }

        if !ports.is_empty() {
            ports.sort();
            return ports;
        }

        // Last resort: probe only serial nodes that actually exist.
        for i in 0..=5 {
            for value in [format!("/dev/ttyACM{}", i), format!("/dev/ttyUSB{}", i)] {
                if Path::new(&value).exists() {
                    ports.push(value);
                }
            }
        }
    }

    ports
}

#[cfg(test)]
mod tests {
    use super::{pm3_args, process_error_detail, validate_command_input, validate_port};

    #[test]
    fn stable_linux_by_id_port_is_valid() {
        assert!(validate_port("/dev/serial/by-id/usb-Proxmark3_Test_Reader-if00").is_ok());
    }

    #[test]
    fn invalid_port_is_rejected_without_length_heuristics() {
        assert!(validate_port("/tmp/not-a-serial-device").is_err());
        assert!(validate_port("/dev/ttyACM0;evil").is_err());
    }

    #[test]
    fn process_error_preserves_stdout_and_stderr() {
        let detail = process_error_detail(
            "\x1b[31mNo known/supported 13.56 MHz tags found\x1b[0m\n",
            "diagnostic\n",
        );
        assert!(detail.contains("No known/supported 13.56 MHz tags found"));
        assert!(detail.contains("diagnostic"));
        assert!(!detail.contains("\x1b"));
    }

    #[test]
    fn process_error_handles_empty_streams() {
        assert_eq!(process_error_detail("", ""), "no process output");
        assert_eq!(process_error_detail("result\n", ""), "result");
        assert_eq!(process_error_detail("", "failure\n"), "failure");
    }

    #[test]
    fn pm3_command_is_one_argv_value_and_never_shell_interpolated() {
        let args = pm3_args("/dev/ttyACM0", "hw version");
        assert_eq!(args.last().map(String::as_str), Some("hw version"));
        assert!(validate_command_input("/dev/ttyACM0", "hw version").is_ok());
        assert!(validate_command_input("/dev/ttyACM0", "hw version;lf t55xx wipe").is_err());
        assert!(validate_command_input("/dev/ttyACM0", "hw version\nquit").is_err());
    }
}

fn parse_hw_version(output: &str) -> Option<(String, String)> {
    use crate::pm3::version::parse_detailed_hw_version;

    let info = parse_detailed_hw_version(output);

    // Pick best version source: os_version (device firmware) > client_version
    let version_str = if !info.os_version.is_empty() {
        info.os_version
    } else if !info.client_version.is_empty() {
        info.client_version
    } else {
        // No version found — check if it's at least a PM3 device
        if output.to_lowercase().contains("proxmark") {
            return Some((info.model, "unknown".to_string()));
        }
        return None;
    };

    // Extract clean short version like "v4.20728" for sidebar display
    let firmware = extract_short_version(&version_str);
    Some((info.model, firmware))
}

/// Extract a short version string like "v4.20728" from a full version string
/// like "Iceman/master/v4.20728-358-ga2ba91043-suspect".
fn extract_short_version(version_str: &str) -> String {
    // Find 'v' followed by a digit
    let v_pos = version_str.char_indices().find(|&(i, c)| {
        c == 'v'
            && version_str.get(i + 1..i + 2).map_or(false, |s| {
                s.as_bytes().first().map_or(false, |b| b.is_ascii_digit())
            })
    });

    if let Some((pos, _)) = v_pos {
        let rest = &version_str[pos..];
        // Version is "v" + digits/dots, stop at anything else
        let end = rest
            .find(|c: char| c != 'v' && !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        rest[..end].to_string()
    } else {
        version_str.to_string()
    }
}
