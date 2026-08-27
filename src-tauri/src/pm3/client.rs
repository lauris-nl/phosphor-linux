use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::AppError;

const CLIENT_CONFIG_FILE: &str = "pm3-client.json";
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Pm3ClientInfo {
    pub path: String,
    pub source: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pm3ClientConfig {
    executable: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateSource {
    Environment,
    Saved,
    Path,
    System,
    UserLocal,
}

impl CandidateSource {
    fn label(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Saved => "saved",
            Self::Path => "PATH",
            Self::System => "system",
            Self::UserLocal => "user-local",
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join(CLIENT_CONFIG_FILE))
        .map_err(|e| {
            AppError::CommandFailed(format!("Cannot resolve application config directory: {e}"))
        })
}

fn load_saved_path(path: &Path) -> Result<Option<PathBuf>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read(path).map_err(|e| {
        AppError::ClientInvalid(format!("Cannot read saved PM3 client setting: {e}"))
    })?;
    let config: Pm3ClientConfig = serde_json::from_slice(&data)
        .map_err(|e| AppError::ClientInvalid(format!("Invalid saved PM3 client setting: {e}")))?;
    Ok(Some(PathBuf::from(config.executable)))
}

fn save_path(path: &Path, executable: &Path) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::CommandFailed("PM3 client config path has no parent directory".into())
    })?;
    fs::create_dir_all(parent).map_err(|e| {
        AppError::CommandFailed(format!("Cannot create application config directory: {e}"))
    })?;
    let temp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&Pm3ClientConfig {
        executable: executable.to_string_lossy().into_owned(),
    })
    .map_err(|e| AppError::CommandFailed(format!("Cannot encode PM3 client setting: {e}")))?;
    fs::write(&temp, data)
        .map_err(|e| AppError::CommandFailed(format!("Cannot save PM3 client setting: {e}")))?;
    fs::rename(&temp, path)
        .map_err(|e| AppError::CommandFailed(format!("Cannot commit PM3 client setting: {e}")))
}

fn path_candidates(path: Option<&str>) -> Vec<PathBuf> {
    path.into_iter()
        .flat_map(env::split_paths)
        .map(|dir| dir.join("proxmark3"))
        .collect()
}

fn discovery_candidates(
    explicit: Option<PathBuf>,
    saved: Option<PathBuf>,
    inherited_path: Option<&str>,
    home: Option<&Path>,
) -> Vec<(PathBuf, CandidateSource)> {
    let mut result = Vec::new();
    if let Some(value) = explicit {
        result.push((value, CandidateSource::Environment));
    }
    if let Some(value) = saved {
        result.push((value, CandidateSource::Saved));
    }
    result.extend(
        path_candidates(inherited_path)
            .into_iter()
            .map(|value| (value, CandidateSource::Path)),
    );
    result.push((
        PathBuf::from("/usr/local/bin/proxmark3"),
        CandidateSource::System,
    ));
    result.push((PathBuf::from("/usr/bin/proxmark3"), CandidateSource::System));
    if let Some(home) = home {
        result.push((
            home.join(".local/bin/proxmark3"),
            CandidateSource::UserLocal,
        ));
    }
    result
}

fn validate_executable_file(path: &Path) -> Result<PathBuf, AppError> {
    let metadata = fs::metadata(path)
        .map_err(|e| AppError::ClientInvalid(format!("{} ({e})", path.display())))?;
    if !metadata.is_file() {
        return Err(AppError::ClientInvalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(AppError::ClientInvalid(format!(
                "{} is not executable",
                path.display()
            )));
        }
    }
    fs::canonicalize(path)
        .map_err(|e| AppError::ClientInvalid(format!("cannot resolve {} ({e})", path.display())))
}

async fn validate_client(path: &Path, source: CandidateSource) -> Result<Pm3ClientInfo, AppError> {
    let canonical = validate_executable_file(path)?;
    let output = timeout(
        VERSION_TIMEOUT,
        Command::new(&canonical).arg("--version").output(),
    )
    .await
    .map_err(|_| AppError::ClientInvalid("version check timed out".into()))?
    .map_err(|e| AppError::ClientInvalid(format!("version check failed: {e}")))?;

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = combined.to_ascii_lowercase();
    if !output.status.success()
        || !lower.contains("client:")
        || !(lower.contains("iceman") || lower.contains("proxmark3"))
    {
        return Err(AppError::ClientInvalid(
            "Selected file is not a compatible Proxmark3 client.".into(),
        ));
    }

    let version = combined
        .lines()
        .map(str::trim)
        .find(|line| line.to_ascii_lowercase().starts_with("client:"))
        .unwrap_or("Compatible RRG/Iceman client")
        .to_string();
    Ok(Pm3ClientInfo {
        path: canonical.to_string_lossy().into_owned(),
        source: source.label().into(),
        version,
    })
}

async fn resolve_from(
    candidates: Vec<(PathBuf, CandidateSource)>,
) -> Result<Pm3ClientInfo, AppError> {
    for (path, source) in candidates {
        if !path.exists() {
            if matches!(
                source,
                CandidateSource::Environment | CandidateSource::Saved
            ) {
                return Err(AppError::ClientInvalid(format!(
                    "configured executable does not exist: {}",
                    path.display()
                )));
            }
            continue;
        }
        match validate_client(&path, source).await {
            Ok(info) => return Ok(info),
            Err(error)
                if matches!(
                    source,
                    CandidateSource::Environment | CandidateSource::Saved
                ) =>
            {
                return Err(error)
            }
            Err(_) => continue,
        }
    }
    Err(AppError::ClientRequired)
}

pub async fn resolve_client(app: &AppHandle) -> Result<Pm3ClientInfo, AppError> {
    let explicit = env::var_os("PHOSPHOR_MODERN_PM3_BIN").map(PathBuf::from);
    let saved = load_saved_path(&config_path(app)?)?;
    let inherited_path = env::var("PATH").ok();
    let home = env::var_os("HOME").map(PathBuf::from);
    resolve_from(discovery_candidates(
        explicit,
        saved,
        inherited_path.as_deref(),
        home.as_deref(),
    ))
    .await
}

#[tauri::command]
pub async fn get_pm3_client_info(app: AppHandle) -> Result<Pm3ClientInfo, AppError> {
    resolve_client(&app).await
}

#[tauri::command]
pub async fn set_pm3_client_path(app: AppHandle, path: String) -> Result<Pm3ClientInfo, AppError> {
    let info = validate_client(Path::new(&path), CandidateSource::Saved).await?;
    save_path(&config_path(&app)?, Path::new(&info.path))?;
    Ok(info)
}

/// Open the native chooser from the trusted Rust backend. The frontend never
/// receives general shell or filesystem execution permission.
#[tauri::command]
pub async fn choose_pm3_client(app: AppHandle) -> Result<Option<String>, AppError> {
    let selected = app
        .dialog()
        .file()
        .set_title("Locate the RRG/Iceman proxmark3 client")
        .blocking_pick_file();
    match selected {
        Some(path) => path
            .into_path()
            .map(|path| Some(path.to_string_lossy().into_owned()))
            .map_err(|e| {
                AppError::CommandFailed(format!("Selected path is not a local file: {e}"))
            }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "phosphor-client-test-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fake_client(dir: &Path, name: &str, valid: bool) -> PathBuf {
        let path = dir.join(name);
        let body = if valid {
            "#!/bin/sh\nprintf 'Client: Iceman/master/v4.99999\\n'\n"
        } else {
            "#!/bin/sh\nprintf 'not a PM3 client\\n'\n"
        };
        fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[tokio::test]
    async fn explicit_path_precedes_saved_and_path() {
        let dir = temp_dir("explicit");
        let explicit = fake_client(&dir, "explicit", true);
        let saved = fake_client(&dir, "saved", true);
        let path_dir = dir.join("bin");
        fs::create_dir_all(&path_dir).unwrap();
        fake_client(&path_dir, "proxmark3", true);
        let result = resolve_from(discovery_candidates(
            Some(explicit.clone()),
            Some(saved),
            Some(path_dir.to_str().unwrap()),
            None,
        ))
        .await
        .unwrap();
        assert_eq!(Path::new(&result.path), fs::canonicalize(explicit).unwrap());
        assert_eq!(result.source, "environment");
    }

    #[tokio::test]
    async fn saved_path_precedes_path_when_environment_is_absent() {
        let dir = temp_dir("saved");
        let saved = fake_client(&dir, "saved", true);
        let path_dir = dir.join("bin");
        fs::create_dir_all(&path_dir).unwrap();
        fake_client(&path_dir, "proxmark3", true);
        let result = resolve_from(discovery_candidates(
            None,
            Some(saved.clone()),
            Some(path_dir.to_str().unwrap()),
            None,
        ))
        .await
        .unwrap();
        assert_eq!(Path::new(&result.path), fs::canonicalize(saved).unwrap());
        assert_eq!(result.source, "saved");
    }

    #[tokio::test]
    async fn inherited_path_lookup_works() {
        let dir = temp_dir("path");
        let client = fake_client(&dir, "proxmark3", true);
        let result = resolve_from(discovery_candidates(
            None,
            None,
            Some(dir.to_str().unwrap()),
            None,
        ))
        .await
        .unwrap();
        assert_eq!(Path::new(&result.path), fs::canonicalize(client).unwrap());
        assert_eq!(result.source, "PATH");
    }

    #[tokio::test]
    async fn system_fallback_candidate_is_executable_and_validated() {
        let dir = temp_dir("system");
        let client = fake_client(&dir, "proxmark3", true);
        let result = resolve_from(vec![(client.clone(), CandidateSource::System)])
            .await
            .unwrap();
        assert_eq!(Path::new(&result.path), fs::canonicalize(client).unwrap());
        assert_eq!(result.source, "system");
    }

    #[test]
    fn system_fallback_order_is_local_then_usr() {
        let candidates = discovery_candidates(None, None, Some(""), None);
        let paths: Vec<_> = candidates.iter().map(|(p, _)| p.as_path()).collect();
        let local = paths
            .iter()
            .position(|p| *p == Path::new("/usr/local/bin/proxmark3"))
            .unwrap();
        let usr = paths
            .iter()
            .position(|p| *p == Path::new("/usr/bin/proxmark3"))
            .unwrap();
        assert!(local < usr);
    }

    #[tokio::test]
    async fn invalid_selected_file_is_rejected() {
        let dir = temp_dir("invalid");
        let invalid = fake_client(&dir, "not-pm3", false);
        let error = validate_client(&invalid, CandidateSource::Saved)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::ClientInvalid(_)));
    }

    #[tokio::test]
    async fn no_candidate_requires_configuration() {
        let dir = temp_dir("missing");
        let error = resolve_from(discovery_candidates(
            None,
            None,
            Some(dir.to_str().unwrap()),
            None,
        ))
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::ClientRequired));
    }

    #[test]
    fn saved_path_round_trips() {
        let dir = temp_dir("persist");
        let config = dir.join(CLIENT_CONFIG_FILE);
        let selected = dir.join("proxmark3 client with spaces");
        save_path(&config, &selected).unwrap();
        assert_eq!(load_saved_path(&config).unwrap(), Some(selected));
    }

    #[test]
    fn production_frontend_exposes_configuration_and_retry_actions() {
        let api = include_str!("../../../src/lib/api.ts");
        let error_step = include_str!("../../../src/components/wizard/ErrorStep.tsx");
        let settings = include_str!("../../../src/components/settings/SettingsView.tsx");
        assert!(api.contains("choose_pm3_client"));
        assert!(api.contains("set_pm3_client_path"));
        assert!(error_step.contains("LOCATE PROXMARK3"));
        assert!(error_step.contains("RETRY"));
        assert!(settings.contains("pm3Client.path"));
    }
}
