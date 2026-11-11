use std::collections::HashSet;
use tauri::{AppHandle, Manager, State, Window};

use crate::adb::{self, DevicesResult, SimpleResult};
use crate::iphone::{IphoneManager, IphoneToolsStatus, IphoneUsbStatus, SimpleIphoneResult, UxPlayOptions};
use crate::scrcpy::{LaunchResult, ScrcpyManager, ScrcpyOptions};
use crate::tools::{self, AppInfo, ToolsStatus, UpdateCheckResult};
use crate::health::{self, HealthMonitor};
use crate::profile::{self, ProfileRequest, Target};
use crate::vcam::{VcamManager, VcamStatus};


#[tauri::command]
pub async fn win_minimize(window: Window) {
    let _ = window.minimize();
}

#[tauri::command]
pub async fn win_maximize(window: Window) {
    if let Ok(is_max) = window.is_maximized() {
        if is_max {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

#[tauri::command]
pub async fn win_close(window: Window, app: AppHandle) {
    let scrcpy_manager = app.state::<ScrcpyManager>();
    if scrcpy_manager.has_active_sessions() {
        let _ = window.hide();
    } else {
        app.exit(0);
    }
}




#[tauri::command]
pub async fn get_tools_status() -> ToolsStatus {
    tools::get_tools_status()
}

#[tauri::command]
pub async fn get_app_info() -> AppInfo {
    tools::get_app_info()
}

#[tauri::command]
pub async fn check_updates() -> UpdateCheckResult {
    tools::check_updates().await
}

#[tauri::command]
pub async fn open_external(url: String) -> serde_json::Value {
    if url.starts_with("http://") || url.starts_with("https://") {
        let ok = open::that(&url).is_ok();
        serde_json::json!({ "ok": ok })
    } else {
        serde_json::json!({ "ok": false, "error": "URL no permitida" })
    }
}

#[tauri::command]
pub async fn adb_get_devices(scrcpy: State<'_, ScrcpyManager>) -> Result<DevicesResult, String> {
    let active_sessions = scrcpy.get_sessions().await;
    let set: HashSet<String> = active_sessions.into_iter().collect();
    Ok(adb::get_devices(set).await)
}

#[tauri::command]
pub async fn adb_pair(host: String, port: String, code: String) -> Result<SimpleResult, String> {
    Ok(adb::pair_wifi(&host, &port, &code).await)
}

#[tauri::command]
pub async fn adb_connect(ip: String, port: String) -> Result<SimpleResult, String> {
    Ok(adb::connect_wifi(&ip, &port).await)
}

#[tauri::command]
pub async fn adb_disconnect(serial: String) -> Result<SimpleResult, String> {
    Ok(adb::disconnect(&serial).await)
}

#[tauri::command]
pub async fn adb_enable_tcpip(serial: String) -> Result<SimpleResult, String> {
    Ok(adb::enable_tcpip(&serial).await)
}

#[tauri::command]
pub async fn scrcpy_launch(
    app: AppHandle,
    scrcpy: State<'_, ScrcpyManager>,
    monitor: State<'_, HealthMonitor>,
    serial: String,
    options: Option<ScrcpyOptions>,
) -> Result<LaunchResult, String> {
    let mut opts = options.unwrap_or_default();
    // Si el teléfono ya está caliente, arrancar directamente con menos calidad
    // (o no arrancar si está en estado crítico).
    let health = HealthMonitor::check(&serial).await;
    let level = health.as_ref().map(health::required_level).unwrap_or(0);
    let is_camera = opts.camera_only.unwrap_or(false);
    if is_camera && level >= health::MAX_LEVEL {
        return Ok(LaunchResult {
            ok: false,
            error: Some(format!(
                "El teléfono está demasiado caliente para usar la cámara ({}). Deja que se enfríe unos minutos.",
                health.as_ref().map(health::reason).unwrap_or_default()
            )),
            ..Default::default()
        });
    }
    if is_camera && level > 0 {
        opts.throttle = Some(level);
    }
    let res = scrcpy.launch(app.clone(), serial.clone(), opts).await;
    if res.ok {
        monitor.ensure(serial, level.min(health::MAX_LEVEL - 1), app).await;
    }
    Ok(res)
}

#[tauri::command]
pub async fn scrcpy_stop(
    scrcpy: State<'_, ScrcpyManager>,
    serial: String,
) -> Result<serde_json::Value, String> {
    scrcpy.stop(&serial).await;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub async fn scrcpy_get_sessions(
    scrcpy: State<'_, ScrcpyManager>,
) -> Result<Vec<String>, String> {
    Ok(scrcpy.get_sessions().await)
}

#[tauri::command]
pub async fn iphone_tools_status(
    iphone: State<'_, IphoneManager>,
) -> Result<IphoneToolsStatus, String> {
    Ok(iphone.tools_status().await)
}

#[tauri::command]
pub async fn iphone_start_uxplay(
    app: AppHandle,
    iphone: State<'_, IphoneManager>,
    opts: Option<UxPlayOptions>,
) -> Result<SimpleIphoneResult, String> {
    Ok(iphone.start_uxplay(app, opts).await)
}

#[tauri::command]
pub async fn iphone_stop_uxplay(
    iphone: State<'_, IphoneManager>,
) -> Result<SimpleIphoneResult, String> {
    Ok(iphone.stop_uxplay().await)
}

#[tauri::command]
pub async fn iphone_obs_virtual_cam(
    iphone: State<'_, IphoneManager>,
    opts: Option<serde_json::Value>,
) -> Result<SimpleIphoneResult, String> {
    let start = opts
        .as_ref()
        .and_then(|v| v.get("start"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Ok(iphone.obs_virtual_cam(start).await)
}

#[tauri::command]
pub async fn iphone_usb_status(
    iphone: State<'_, IphoneManager>,
) -> Result<IphoneUsbStatus, String> {
    Ok(iphone.usb_status().await)
}

#[tauri::command]
pub async fn iphone_usb_start_stream(
    iphone: State<'_, IphoneManager>,
    opts: Option<serde_json::Value>,
) -> Result<SimpleIphoneResult, String> {
    let port = opts
        .as_ref()
        .and_then(|v| v.get("port"))
        .and_then(|v| v.as_u64())
        .map(|p| p as u32);
    Ok(iphone.usb_start_stream(port).await)
}

#[tauri::command]
pub async fn iphone_usb_stop_stream(
    iphone: State<'_, IphoneManager>,
) -> Result<SimpleIphoneResult, String> {
    Ok(iphone.usb_stop_stream().await)
}

#[tauri::command]
pub async fn vcam_get_status(vcam: State<'_, VcamManager>) -> Result<VcamStatus, String> {
    Ok(vcam.get_status().await)
}

#[tauri::command]
pub async fn vcam_start(
    vcam: State<'_, VcamManager>,
    monitor: State<'_, HealthMonitor>,
    app: AppHandle,
    serial: String,
    options: Option<ProfileRequest>,
) -> Result<serde_json::Value, String> {
    if !crate::adb::is_valid_serial(&serial) {
        return Ok(serde_json::json!({ "ok": false, "error": "Serial no válido" }));
    }
    let mut req = options.unwrap_or_default();
    let health = HealthMonitor::check(&serial).await;
    let level = health.as_ref().map(health::required_level).unwrap_or(0);
    if level >= health::MAX_LEVEL {
        return Ok(serde_json::json!({
            "ok": false,
            "error": format!(
                "El teléfono está demasiado caliente para usar la cámara ({}). Deja que se enfríe unos minutos.",
                health.as_ref().map(health::reason).unwrap_or_default()
            ),
        }));
    }
    if level > 0 {
        req.throttle = Some(level);
    }
    match vcam.start(serial.clone(), req, app.clone()).await {
        Ok(profile) => {
            monitor.ensure(serial, level, app).await;
            Ok(serde_json::json!({ "ok": true, "profile": profile }))
        }
        Err(e) => Ok(serde_json::json!({ "ok": false, "error": e })),
    }
}

/// Qué configuración de cámara se aplicaría en este teléfono con estos ajustes
/// (y qué codecs hardware tiene). La primera llamada por dispositivo tarda ~1-2 s.
#[tauri::command]
pub async fn device_profile(
    serial: String,
    options: Option<ProfileRequest>,
    target: Option<String>,
) -> Result<serde_json::Value, String> {
    if !crate::adb::is_valid_serial(&serial) {
        return Ok(serde_json::json!({ "ok": false, "error": "Serial no válido" }));
    }
    let target = if target.as_deref() == Some("vcam") { Target::Vcam } else { Target::Window };
    let caps = profile::device_caps(&serial).await;
    let req = options.unwrap_or_default();
    let resolved = profile::resolve(caps.as_ref().ok(), &req, target);
    Ok(serde_json::json!({
        "ok": true,
        "profile": resolved,
        "hwCodecs": caps.as_ref().map(|c| c.hw_codecs()).unwrap_or_default(),
        "cameras": caps.as_ref().map(|c| c.cameras.clone()).unwrap_or_default(),
        "capsError": caps.err(),
    }))
}

#[tauri::command]
pub async fn vcam_stop(vcam: State<'_, VcamManager>) -> Result<serde_json::Value, String> {
    let _ = vcam.stop().await;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub async fn vcam_install_driver() -> Result<serde_json::Value, String> {
    match VcamManager::trigger_driver_install() {
        Ok(_) => Ok(serde_json::json!({ "ok": true })),
        Err(e) => Ok(serde_json::json!({ "ok": false, "error": e })),
    }
}

