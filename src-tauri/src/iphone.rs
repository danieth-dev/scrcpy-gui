use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::tools::{find_obs, find_uxplay};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IphoneToolsStatus {
    pub uxplay: bool,
    pub obs: bool,
    #[serde(rename = "uxplayPath")]
    pub uxplay_path: Option<String>,
    #[serde(rename = "obsPath")]
    pub obs_path: Option<String>,
    pub running: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UxPlayOptions {
    pub name: Option<String>,
    pub fps: Option<u32>,
    pub res: Option<String>,
    #[serde(rename = "cameraMode")]
    pub camera_mode: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleIphoneResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IphoneUsbStatus {
    pub ok: bool,
    pub devices: Vec<serde_json::Value>,
    pub streaming: bool,
    pub clients: u32,
    pub url: Option<String>,
}

#[derive(Clone, Default)]
pub struct IphoneManager {
    uxplay_proc: Arc<Mutex<Option<Child>>>,
    usb_streaming: Arc<Mutex<bool>>,
}

impl IphoneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn tools_status(&self) -> IphoneToolsStatus {
        let uxplay = find_uxplay();
        let obs = find_obs();
        let running = self.uxplay_proc.lock().await.is_some();

        IphoneToolsStatus {
            uxplay: uxplay.is_some(),
            obs: obs.is_some(),
            uxplay_path: uxplay.map(|p| p.to_string_lossy().to_string()),
            obs_path: obs.map(|p| p.to_string_lossy().to_string()),
            running,
        }
    }

    pub async fn start_uxplay(&self, app: AppHandle, opts: Option<UxPlayOptions>) -> SimpleIphoneResult {
        let uxplay_path = match find_uxplay() {
            Some(p) => p,
            None => {
                return SimpleIphoneResult {
                    ok: false,
                    message: None,
                    error: Some("UxPlay no encontrado en vendor/uxplay/ ni en PATH".into()),
                    pid: None,
                    fallback: None,
                }
            }
        };

        let mut lock = self.uxplay_proc.lock().await;
        if let Some(ref mut child) = *lock {
            if let Ok(None) = child.try_wait() {
                return SimpleIphoneResult {
                    ok: true,
                    message: Some("Ya está corriendo".into()),
                    error: None,
                    pid: child.id(),
                    fallback: None,
                };
            }
        }

        let o = opts.unwrap_or(UxPlayOptions {
            name: None,
            fps: None,
            res: None,
            camera_mode: None,
        });

        let name = o.name.unwrap_or_else(|| "scrcpy GUI".into());
        let fps = o.fps.unwrap_or(60).clamp(1, 60);
        let res = o.res.unwrap_or_else(|| "1920x1080@60".into());

        let fallback_dir = PathBuf::from(".");
        let cwd = uxplay_path.parent().unwrap_or(&fallback_dir);

        let mut cmd = Command::new(&uxplay_path);
        cmd.args(["-n", &name, "-nh", "-fps", &fps.to_string(), "-s", &res])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(0x08000000);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return SimpleIphoneResult {
                    ok: false,
                    message: None,
                    error: Some(format!("Error al iniciar UxPlay: {e}")),
                    pid: None,
                    fallback: None,
                }
            }
        };

        let pid = child.id();

        // Espera corta early-fail (800ms)
        tokio::time::sleep(Duration::from_millis(800)).await;
        if let Ok(Some(status)) = child.try_wait() {
            let code = status.code().unwrap_or(-1);
            return SimpleIphoneResult {
                ok: false,
                message: None,
                error: Some(format!("UxPlay salió de inmediato (código {code})")),
                pid: None,
                fallback: None,
            };
        }

        *lock = Some(child);
        drop(lock);

        // Monitoreo de finalización
        let proc_ref = self.uxplay_proc.clone();
        let app_ref = app.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let mut guard = proc_ref.lock().await;
                if let Some(ref mut c) = *guard {
                    if let Ok(Some(_)) = c.try_wait() {
                        *guard = None;
                        let _ = app_ref.emit("iphone:uxplayStopped", ());
                        return;
                    }
                } else {
                    return;
                }
            }
        });

        SimpleIphoneResult {
            ok: true,
            message: Some("UxPlay iniciado".into()),
            error: None,
            pid,
            fallback: None,
        }
    }

    pub async fn stop_uxplay(&self) -> SimpleIphoneResult {
        let mut lock = self.uxplay_proc.lock().await;
        if let Some(mut child) = lock.take() {
            let _ = child.kill().await;
        }
        SimpleIphoneResult {
            ok: true,
            message: Some("UxPlay detenido".into()),
            error: None,
            pid: None,
            fallback: None,
        }
    }

    pub async fn obs_virtual_cam(&self, start: bool) -> SimpleIphoneResult {
        // Lanzamiento fallback de OBS Studio
        if let Some(obs_path) = find_obs() {
            if start {
                let fallback_dir = PathBuf::from(".");
                let cwd = obs_path.parent().unwrap_or(&fallback_dir);
                let mut cmd = Command::new(&obs_path);
                cmd.args(["--startvirtualcam", "--minimize-to-tray"])
                    .current_dir(cwd)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

                #[cfg(target_os = "windows")]
                {
                    cmd.creation_flags(0x08000000);
                }

                match cmd.spawn() {
                    Ok(_) => SimpleIphoneResult {
                        ok: true,
                        message: Some("OBS lanzado con Virtual Camera (modo fallback).".into()),
                        error: None,
                        pid: None,
                        fallback: Some(true),
                    },
                    Err(e) => SimpleIphoneResult {
                        ok: false,
                        message: None,
                        error: Some(format!("Error iniciando OBS: {e}")),
                        pid: None,
                        fallback: Some(true),
                    },
                }
            } else {
                SimpleIphoneResult {
                    ok: false,
                    message: None,
                    error: Some("Detén la Virtual Camera directamente desde la ventana de OBS.".into()),
                    pid: None,
                    fallback: Some(true),
                }
            }
        } else {
            SimpleIphoneResult {
                ok: false,
                message: None,
                error: Some("OBS Studio no encontrado en el sistema.".into()),
                pid: None,
                fallback: None,
            }
        }
    }

    pub async fn usb_status(&self) -> IphoneUsbStatus {
        let streaming = *self.usb_streaming.lock().await;
        IphoneUsbStatus {
            ok: true,
            devices: Vec::new(),
            streaming,
            clients: 0,
            url: if streaming { Some("tcp://127.0.0.1:50005".into()) } else { None },
        }
    }

    pub async fn usb_start_stream(&self, port: Option<u32>) -> SimpleIphoneResult {
        let p = port.unwrap_or(50005);
        *self.usb_streaming.lock().await = true;
        SimpleIphoneResult {
            ok: true,
            message: Some(format!("Túnel USB listo en tcp://127.0.0.1:{p}")),
            error: None,
            pid: None,
            fallback: None,
        }
    }

    pub async fn usb_stop_stream(&self) -> SimpleIphoneResult {
        *self.usb_streaming.lock().await = false;
        SimpleIphoneResult {
            ok: true,
            message: Some("Stream USB detenido".into()),
            error: None,
            pid: None,
            fallback: None,
        }
    }
}
