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
    // (o no arrancar si está en estado crítico). Con la protección
    // desactivada, sólo se niega en emergencia.
    let health = HealthMonitor::check(&serial).await;
    let (level, refuse) = monitor.start_level(health.as_ref());
    let is_camera = opts.camera_only.unwrap_or(false);
    if is_camera && refuse {
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
        monitor.ensure(serial, if is_camera { level } else { 0 }, app).await;
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
    let (level, refuse) = monitor.start_level(health.as_ref());
    if refuse {
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
    options: Option<ScrcpyOptions>,
    target: Option<String>,
) -> Result<serde_json::Value, String> {
    if !crate::adb::is_valid_serial(&serial) {
        return Ok(serde_json::json!({ "ok": false, "error": "Serial no válido" }));
    }
    let target = if target.as_deref() == Some("vcam") { Target::Vcam } else { Target::Window };
    let caps = profile::device_caps(&serial).await;
    let opts = options.unwrap_or_default();
    let wifi = profile::is_wifi_serial(&serial);
    let mut req = opts.profile_request();
    req.wifi = wifi;
    let mut sreq = opts.screen_request();
    sreq.wifi = wifi;
    let caps_ok = caps.as_ref().ok();
    // Lo que significaría "Máxima calidad" en este teléfono (para el perfil).
    let mut max_req = req.clone();
    max_req.quality = Some("max".into());
    let mut max_sreq = sreq.clone();
    max_sreq.quality = Some("max".into());
    Ok(serde_json::json!({
        "ok": true,
        "profile": profile::resolve(caps_ok, &req, target),
        "screenProfile": profile::resolve_screen(caps_ok, &sreq),
        "maxProfile": profile::resolve(caps_ok, &max_req, target),
        "maxScreenProfile": profile::resolve_screen(caps_ok, &max_sreq),
        "hwCodecs": caps_ok.map(|c| c.hw_codecs()).unwrap_or_default(),
        "cameras": caps_ok.map(|c| c.cameras.clone()).unwrap_or_default(),
        "cameraOptions": caps_ok.map(profile::camera_options).unwrap_or_default(),
        "screenOptions": caps_ok.and_then(|c| c.display.as_ref()).map(profile::screen_options),
        "display": caps_ok.and_then(|c| c.display.clone()),
        "capsError": caps.as_ref().err(),
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


/// Ajustes de la pestaña Ajustes aplicados en vivo a las sesiones abiertas.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LiveSettings {
    #[serde(flatten)]
    pub options: ScrcpyOptions,
    #[serde(rename = "deviceProtection")]
    pub device_protection: Option<bool>,
}

/// Aplica los ajustes a las sesiones abiertas. Sólo relanza las que cambian
/// de verdad (p. ej. pedir 60 fps a una cámara que ya va a su máximo de 30 no
/// reinicia nada). Las ventanas conservan posición y tamaño.
#[tauri::command]
pub async fn apply_settings(
    app: AppHandle,
    scrcpy: State<'_, ScrcpyManager>,
    vcam: State<'_, VcamManager>,
    monitor: State<'_, HealthMonitor>,
    settings: LiveSettings,
) -> Result<serde_json::Value, String> {
    // Una aplicación cada vez: evita relanzamientos cruzados.
    static APPLY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = APPLY.lock().await;

    if let Some(p) = settings.device_protection {
        monitor.set_protection(p);
    }
    let new = &settings.options;
    let mut window = Vec::new();

    for (serial, old) in scrcpy.all_sessions().await {
        let mut merged = old.with_settings_from(new);
        let is_camera = old.camera_only.unwrap_or(false);
        let level = monitor.level(&serial);
        merged.throttle = if is_camera && level > 0 { Some(level) } else { None };

        let (changed, profile, screen) = if is_camera {
            let before = profile::resolve_for(&serial, &old.profile_request(), Target::Window).await;
            let after = profile::resolve_for(&serial, &merged.profile_request(), Target::Window).await;
            (before.differs(&after), Some(after), None)
        } else {
            let before = profile::resolve_screen_for(&serial, &old.screen_request()).await;
            let after = profile::resolve_screen_for(&serial, &merged.screen_request()).await;
            (before.differs(&after) || old.mirror_differs(&merged), None, Some(after))
        };

        if changed {
            let res = scrcpy.relaunch_with(app.clone(), serial.clone(), merged).await;
            window.push(serde_json::json!({
                "serial": serial,
                "restarted": res.ok,
                "error": res.error,
                "pid": res.pid,
                "profile": res.profile,
                "screenProfile": res.screen_profile,
                "camera": is_camera,
            }));
        } else {
            window.push(serde_json::json!({
                "serial": serial,
                "restarted": false,
                "profile": profile,
                "screenProfile": screen,
                "camera": is_camera,
            }));
        }
    }

    let mut vcam_res = serde_json::Value::Null;
    if let Some(serial) = vcam.active_serial().await {
        let level = monitor.level(&serial);
        let mut req = new.profile_request();
        req.throttle = if level > 0 { Some(level) } else { None };
        let current = vcam.session_profile(&serial).await;
        let target = profile::resolve_for(&serial, &req, Target::Vcam).await;
        if current.as_ref().is_some_and(|c| c.differs(&target)) {
            vcam_res = match vcam.restart_with(&serial, req, level, app.clone()).await {
                Ok(p) => serde_json::json!({ "serial": serial, "restarted": true, "profile": p }),
                Err(e) => serde_json::json!({ "serial": serial, "restarted": false, "error": e }),
            };
        } else {
            vcam_res = serde_json::json!({ "serial": serial, "restarted": false, "profile": current });
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "protection": monitor.protection_enabled(),
        "window": window,
        "vcam": vcam_res,
    }))
}
