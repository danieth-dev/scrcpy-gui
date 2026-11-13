use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsStatus {
    pub scrcpy: bool,
    pub adb: bool,
    #[serde(rename = "scrcpyPath")]
    pub scrcpy_path: Option<String>,
    #[serde(rename = "adbPath")]
    pub adb_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub name: String,
    pub author: String,
    pub homepage: String,
    pub repository: Option<RepoInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub ok: bool,
    pub current: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(rename = "updateAvailable", skip_serializing_if = "Option::is_none")]
    pub update_available: Option<bool>,
    #[serde(rename = "releaseName", skip_serializing_if = "Option::is_none")]
    pub release_name: Option<String>,
    #[serde(rename = "releaseNotes", skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
    #[serde(rename = "htmlUrl", skip_serializing_if = "Option::is_none")]
    pub html_url: Option<String>,
    #[serde(rename = "releasesUrl", skip_serializing_if = "Option::is_none")]
    pub releases_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn resolve_tool(subdir: &str, exe_name: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Relativo al ejecutable actual y sus directorios superiores (ancestros)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for ancestor in dir.ancestors().take(6) {
                candidates.push(ancestor.join("vendor").join(subdir).join(exe_name));
                candidates.push(ancestor.join("gui").join("vendor").join(subdir).join(exe_name));
                candidates.push(ancestor.join("resources").join(subdir).join(exe_name));
                candidates.push(ancestor.join("resources").join("vendor").join(subdir).join(exe_name));
                candidates.push(ancestor.join(subdir).join(exe_name));
                candidates.push(ancestor.join(exe_name));
            }
        }
    }

    // 2. Relativo al directorio de trabajo actual y sus directorios superiores
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors().take(6) {
            candidates.push(ancestor.join("vendor").join(subdir).join(exe_name));
            candidates.push(ancestor.join("gui").join("vendor").join(subdir).join(exe_name));
            candidates.push(ancestor.join("resources").join(subdir).join(exe_name));
            candidates.push(ancestor.join("resources").join("vendor").join(subdir).join(exe_name));
            candidates.push(ancestor.join(subdir).join(exe_name));
            candidates.push(ancestor.join(exe_name));
        }
    }

    for path in &candidates {
        if path.is_file() {
            return Some(path.clone());
        }
    }

    // 3. Buscar en el PATH del sistema
    if let Ok(path_var) = std::env::var("PATH") {
        for p in std::env::split_paths(&path_var) {
            let candidate = p.join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn find_scrcpy() -> Option<PathBuf> {
    resolve_tool("scrcpy", "scrcpy.exe")
}

pub fn find_adb() -> Option<PathBuf> {
    resolve_tool("scrcpy", "adb.exe")
}

pub fn find_uxplay() -> Option<PathBuf> {
    resolve_tool("uxplay", "uxplay.exe")
        .or_else(|| resolve_tool("uxplay", "uxplay-windows.exe"))
}


pub fn find_obs() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files\obs-studio\bin\64bit\obs64.exe"),
        PathBuf::from(r"C:\Program Files (x86)\obs-studio\bin\64bit\obs64.exe"),
    ];

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates.push(Path::new(&local_app_data).join(r"Programs\obs-studio\bin\64bit\obs64.exe"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(Path::new(&program_files).join(r"obs-studio\bin\64bit\obs64.exe"));
    }

    candidates.into_iter().find(|p| p.is_file())
}

pub fn get_tools_status() -> ToolsStatus {
    let scrcpy_path = find_scrcpy();
    let adb_path = find_adb();

    ToolsStatus {
        scrcpy: scrcpy_path.is_some(),
        adb: adb_path.is_some(),
        scrcpy_path: scrcpy_path.map(|p| p.to_string_lossy().to_string()),
        adb_path: adb_path.map(|p| p.to_string_lossy().to_string()),
    }
}



pub fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: "scrcpy GUI".to_string(),
        author: "danieth-dev".to_string(),
        homepage: "https://github.com/danieth-dev/scrcpy-gui".to_string(),
        repository: Some(RepoInfo {
            owner: "danieth-dev".to_string(),
            repo: "scrcpy-gui".to_string(),
        }),
    }
}

fn cmp_semver(a: &str, b: &str) -> i32 {
    let parse = |s: &str| -> Vec<i32> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<i32>().unwrap_or(0))
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..3 {
        let da = *va.get(i).unwrap_or(&0);
        let db = *vb.get(i).unwrap_or(&0);
        if da != db {
            return da - db;
        }
    }
    0
}

pub async fn check_updates() -> UpdateCheckResult {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let owner = "danieth-dev";
    let repo = "scrcpy-gui";
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let releases_url = format!("https://github.com/{owner}/{repo}/releases");

    let client = match reqwest::Client::builder()
        .user_agent(format!("scrcpy-gui/{current}"))
        .build() {
            Ok(c) => c,
            Err(e) => return UpdateCheckResult {
                ok: false,
                current,
                latest: None,
                update_available: None,
                release_name: None,
                release_notes: None,
                html_url: None,
                releases_url: Some(releases_url),
                message: None,
                error: Some(e.to_string()),
            },
        };

    let resp = match client.get(&url).header("Accept", "application/vnd.github+json").send().await {
        Ok(r) => r,
        Err(e) => return UpdateCheckResult {
            ok: false,
            current,
            latest: None,
            update_available: None,
            release_name: None,
            release_notes: None,
            html_url: None,
            releases_url: Some(releases_url),
            message: None,
            error: Some(format!("Error de conexión: {e}")),
        },
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return UpdateCheckResult {
            ok: true,
            current,
            latest: None,
            update_available: Some(false),
            release_name: None,
            release_notes: None,
            html_url: None,
            releases_url: Some(releases_url),
            message: Some("Aún no hay releases en GitHub.".to_string()),
            error: None,
        };
    }

    if !resp.status().is_success() {
        return UpdateCheckResult {
            ok: false,
            current,
            latest: None,
            update_available: None,
            release_name: None,
            release_notes: None,
            html_url: None,
            releases_url: Some(releases_url),
            message: None,
            error: Some(format!("GitHub API error: {}", resp.status())),
        };
    }

    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: Option<String>,
        name: Option<String>,
        body: Option<String>,
        html_url: Option<String>,
    }

    match resp.json::<GhRelease>().await {
        Ok(data) => {
            let raw_tag = data.tag_name.or(data.name.clone()).unwrap_or_default();
            let latest = raw_tag.trim_start_matches('v').to_string();
            let update_available = !latest.is_empty() && cmp_semver(&latest, &current) > 0;
            let message = if update_available {
                format!("Hay una nueva versión: v{latest} (tienes v{current})")
            } else {
                format!("Estás al día (v{current})")
            };

            UpdateCheckResult {
                ok: true,
                current,
                latest: Some(latest),
                update_available: Some(update_available),
                release_name: data.name,
                release_notes: data.body,
                html_url: data.html_url,
                releases_url: Some(releases_url),
                message: Some(message),
                error: None,
            }
        }
        Err(e) => UpdateCheckResult {
            ok: false,
            current,
            latest: None,
            update_available: None,
            release_name: None,
            release_notes: None,
            html_url: None,
            releases_url: Some(releases_url),
            message: None,
            error: Some(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_status() {
        println!("current_exe: {:?}", std::env::current_exe());
        println!("current_dir: {:?}", std::env::current_dir());
        let status = get_tools_status();
        println!("Tools status: {:?}", status);
        assert!(status.scrcpy, "scrcpy must be found: {:?}", status.scrcpy_path);
        assert!(status.adb, "adb must be found: {:?}", status.adb_path);
    }


    #[test]
    fn test_when_cwd_is_elsewhere() {
        let exe = PathBuf::from(r"C:\Users\Usuario\Downloads\scrcpy-master\gui\src-tauri\target\release\scrcpy-gui.exe");
        let dir = exe.parent().unwrap();
        for (i, a) in dir.ancestors().take(6).enumerate() {
            println!("ancestor {}: {:?}", i, a);
            let c1 = a.join("vendor").join("scrcpy").join("scrcpy.exe");
            let c2 = a.join("gui").join("vendor").join("scrcpy").join("scrcpy.exe");
            println!("  c1: {:?} -> {}", c1, c1.is_file());
            println!("  c2: {:?} -> {}", c2, c2.is_file());
        }
    }


}

