use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::adb::{adb_quiet, is_ip_serial, is_valid_serial};
use crate::profile::{self, ProfileRequest, ResolvedProfile, ScreenProfile, ScreenRequest, Target};
use crate::tools::find_scrcpy;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScrcpyOptions {
    #[serde(rename = "windowTitle")]
    pub window_title: Option<String>,
    #[serde(rename = "cameraOnly")]
    pub camera_only: Option<bool>,
    #[serde(rename = "cameraFacing")]
    pub camera_facing: Option<String>,
    #[serde(rename = "maxFps")]
    pub max_fps: Option<u32>,
    #[serde(rename = "maxSize")]
    pub max_size: Option<u32>,
    #[serde(rename = "cameraZoom")]
    pub camera_zoom: Option<f64>,
    #[serde(rename = "cameraTorch")]
    pub camera_torch: Option<bool>,
    #[serde(rename = "noAudio")]
    pub no_audio: Option<bool>,
    #[serde(rename = "stayAwake")]
    pub stay_awake: Option<bool>,
    #[serde(rename = "screenOff")]
    pub screen_off: Option<bool>,
    pub bitrate: Option<u32>,
    pub codec: Option<String>,
    pub orientation: Option<String>,
    /// "Ver pantalla": lado corto deseado (720, 1080…); 0 = nativa.
    #[serde(rename = "screenRes")]
    pub screen_res: Option<u32>,
    /// "Ver pantalla": fps deseados (≤ refresco de la pantalla).
    #[serde(rename = "screenFps")]
    pub screen_fps: Option<u32>,
    /// "max" = lo mejor que dé el teléfono; "custom" = lo elegido.
    pub quality: Option<String>,
    /// Nivel de protección térmica (lo fija el backend, no la UI).
    #[serde(default, skip_deserializing)]
    pub throttle: Option<u8>,
    /// Posición y tamaño (x, y, ancho, alto) de la ventana al relanzar, para
    /// que no salte de sitio (lo fija el backend).
    #[serde(default, skip_deserializing)]
    pub window_geometry: Option<(i32, i32, u32, u32)>,
}

impl ScrcpyOptions {
    /// Petición de perfil de cámara equivalente a estos ajustes.
    pub fn profile_request(&self) -> ProfileRequest {
        ProfileRequest {
            facing: self.camera_facing.clone(),
            max_size: self.max_size,
            fps: self.max_fps,
            codec: self.codec.clone(),
            bitrate: self.bitrate,
            zoom: self.camera_zoom,
            torch: self.camera_torch,
            orientation: self.orientation.clone(),
            throttle: self.throttle,
            quality: self.quality.clone(),
            wifi: false,
        }
    }

    /// Petición de "Ver pantalla" equivalente a estos ajustes.
    pub fn screen_request(&self) -> ScreenRequest {
        ScreenRequest {
            res: self.screen_res,
            fps: self.screen_fps,
            codec: self.codec.clone(),
            bitrate: self.bitrate,
            quality: self.quality.clone(),
            wifi: false,
        }
    }

    /// Copia los ajustes de la pestaña Ajustes, conservando el modo de la
    /// sesión (título, cámara/pantalla) y su geometría.
    pub fn with_settings_from(&self, new: &ScrcpyOptions) -> ScrcpyOptions {
        let mut o = self.clone();
        o.camera_facing = new.camera_facing.clone();
        o.max_fps = new.max_fps;
        o.max_size = new.max_size;
        o.camera_zoom = new.camera_zoom;
        o.camera_torch = new.camera_torch;
        o.no_audio = new.no_audio;
        o.stay_awake = new.stay_awake;
        o.screen_off = new.screen_off;
        o.bitrate = new.bitrate;
        o.codec = new.codec.clone();
        o.orientation = new.orientation.clone();
        o.screen_res = new.screen_res;
        o.screen_fps = new.screen_fps;
        o.quality = new.quality.clone();
        o
    }

    /// ¿Cambia algo de "Ver pantalla" que no sea la imagen? (la imagen se
    /// compara con `ScreenProfile::differs`)
    pub fn mirror_differs(&self, other: &ScrcpyOptions) -> bool {
        self.orientation.as_deref().unwrap_or("0") != other.orientation.as_deref().unwrap_or("0")
            || self.no_audio != other.no_audio
            || self.stay_awake != other.stay_awake
            || self.screen_off != other.screen_off
    }
}

#[derive(Debug, Default, Serialize)]
pub struct LaunchResult {
    /// Configuración de cámara realmente aplicada (sólo modo cámara).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ResolvedProfile>,
    /// Configuración de pantalla realmente aplicada (sólo "Ver pantalla").
    #[serde(rename = "screenProfile", skip_serializing_if = "Option::is_none")]
    pub screen_profile: Option<ScreenProfile>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextureEvent {
    pub serial: String,
    pub width: u32,
    pub height: u32,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoppedEvent {
    pub serial: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(rename = "earlyFail")]
    pub early_fail: bool,
}

/// Mantiene el log en ≤16 KB sin cortar un carácter UTF-8 por la mitad
/// (los mensajes llevan acentos: cortar a ciegas provoca un panic).
fn cap_log(b: &mut String) {
    const CAP: usize = 16 * 1024;
    if b.len() > CAP {
        let mut cut = b.len() - CAP;
        while !b.is_char_boundary(cut) {
            cut += 1;
        }
        b.drain(..cut);
    }
}

struct SessionControl {
    child: Child,
    generation: u64,
    /// Ajustes con los que se lanzó (para relanzar con protección térmica).
    options: ScrcpyOptions,
}

#[derive(Clone, Default)]
pub struct ScrcpyManager {
    sessions: Arc<Mutex<HashMap<String, SessionControl>>>,
    generation_counter: Arc<Mutex<u64>>,
    /// Teléfonos con reconexión automática en curso (stop() la cancela).
    reconnecting: Arc<Mutex<HashSet<String>>>,
}

impl ScrcpyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_active_sessions(&self) -> bool {
        if let Ok(lock) = self.sessions.try_lock() {
            !lock.is_empty()
        } else {
            false
        }
    }

    pub async fn get_sessions(&self) -> Vec<String> {
        let lock = self.sessions.lock().await;
        lock.keys().cloned().collect()
    }

    /// Relanza la sesión de ese teléfono con otros ajustes, manteniendo la
    /// ventana donde estaba y con el mismo tamaño.
    pub async fn relaunch_with(&self, app: AppHandle, serial: String, mut options: ScrcpyOptions) -> LaunchResult {
        let pid = {
            let lock = self.sessions.lock().await;
            lock.get(&serial).and_then(|s| s.child.id())
        };
        options.window_geometry = match pid {
            Some(pid) => tokio::task::spawn_blocking(move || window_rect(pid)).await.ok().flatten(),
            None => None,
        };
        self.launch(app, serial, options).await
    }

    /// Todas las sesiones activas con sus ajustes.
    pub async fn all_sessions(&self) -> Vec<(String, ScrcpyOptions)> {
        let lock = self.sessions.lock().await;
        lock.iter().map(|(k, v)| (k.clone(), v.options.clone())).collect()
    }

    /// Ajustes de la sesión activa en ese teléfono, si la hay.
    pub async fn session_options(&self, serial: &str) -> Option<ScrcpyOptions> {
        self.sessions.lock().await.get(serial).map(|s| s.options.clone())
    }

    pub async fn stop(&self, serial: &str) {
        self.reconnecting.lock().await.remove(serial);
        let mut lock = self.sessions.lock().await;
        if let Some(mut session) = lock.remove(serial) {
            let _ = session.child.kill().await;
        }
    }

    pub async fn stop_all(&self) {
        let mut lock = self.sessions.lock().await;
        for (_, mut session) in lock.drain() {
            let _ = session.child.kill().await;
        }
    }

    pub async fn launch(
        &self,
        app: AppHandle,
        serial: String,
        options: ScrcpyOptions,
    ) -> LaunchResult {
        if !is_valid_serial(&serial) {
            return LaunchResult { profile: None, screen_profile: None, ok: false, pid: None, error: Some("Serial no válido".into()) };
        }

        let scrcpy_path = match find_scrcpy() {
            Some(p) => p,
            None => return LaunchResult {
                profile: None,
                screen_profile: None,
                ok: false,
                pid: None,
                error: Some("scrcpy.exe no encontrado en vendor/scrcpy/ ni en PATH".into()),
            },
        };

        // Detener sesión previa para el mismo serial
        self.stop(&serial).await;

        // Si es Wi-Fi por IP:puerto y quedó offline, reconectar
        if is_ip_serial(&serial) {
            let state = adb_quiet(["-s", &serial, "get-state"], 2500).await;
            if state != "device" {
                adb_quiet(["disconnect", &serial], 2500).await;
                adb_quiet(["connect", &serial], 5000).await;
            }
        }

        let options_for_session = options.clone();
        let mut args: Vec<String> = vec![
            "-s".to_string(),
            serial.clone(),
            "--window-title".to_string(),
            options.window_title.clone().unwrap_or_else(|| "scrcpy".to_string()),
        ];
        if let Some((x, y, w, h)) = options.window_geometry {
            args.push(format!("--window-x={x}"));
            args.push(format!("--window-y={y}"));
            args.push(format!("--window-width={w}"));
            args.push(format!("--window-height={h}"));
        }

        let mut applied_profile: Option<ResolvedProfile> = None;
        let mut applied_screen: Option<ScreenProfile> = None;
        let camera_only = options.camera_only.unwrap_or(false);
        if camera_only {
            // Mismo resolvedor que la cámara virtual: ajustes de la UI ->
            // configuración que el teléfono soporta de verdad.
            let req = options.profile_request();
            let p = profile::resolve_for(&serial, &req, Target::Window).await;
            args.push("--video-source=camera".to_string());
            args.push("--no-audio".to_string());
            match &p.camera_id {
                Some(id) => args.push(format!("--camera-id={id}")),
                None => args.push(format!("--camera-facing={}", p.facing)),
            }
            args.push(format!("--camera-size={}x{}", p.width, p.height));
            args.push(format!("--camera-fps={}", p.fps));
            if p.high_speed {
                args.push("--camera-high-speed".to_string());
            }
            if p.zoom > 1.0 {
                args.push(format!("--camera-zoom={}", p.zoom));
            }
            if p.torch {
                args.push("--camera-torch".to_string());
            }
            if p.orientation != 0 {
                args.push(format!("--capture-orientation={}", p.orientation));
            }
            args.push(format!("--video-codec={}", p.codec));
            if let Some(enc) = &p.encoder {
                args.push(format!("--video-encoder={enc}"));
            }
            args.push(format!("--video-bit-rate={}M", p.bitrate_mbps));
            applied_profile = Some(p);
        } else {
            if options.no_audio.unwrap_or(false) {
                args.push("--no-audio".to_string());
            }
            if options.stay_awake.unwrap_or(false) {
                args.push("--stay-awake".to_string());
            }
            if options.screen_off.unwrap_or(false) {
                args.push("--turn-screen-off".to_string());
            }
            // Imagen adaptada a la pantalla de este teléfono (resolución por
            // lado corto, fps ≤ refresco, códec por hardware).
            let sp = profile::resolve_screen_for(&serial, &options.screen_request()).await;
            if sp.max_size > 0 {
                args.push(format!("--max-size={}", sp.max_size));
            }
            args.push(format!("--max-fps={}", sp.fps));
            args.push(format!("--video-codec={}", sp.codec));
            if let Some(enc) = &sp.encoder {
                args.push(format!("--video-encoder={enc}"));
            }
            args.push(format!("--video-bit-rate={}M", sp.bitrate_mbps));
            applied_screen = Some(sp);

        }

        args.push("--video-buffer=0".to_string());
        args.push("--video-codec-options=low-latency=1,max-bframes=0".to_string());

        if !camera_only {
            if let Some(ref o) = options.orientation {
                if o != "0" && !o.is_empty() {
                    args.push("--orientation".to_string());
                    args.push(o.clone());
                }
            }
        }

        let generation = {
            let mut g = self.generation_counter.lock().await;
            *g += 1;
            *g
        };

        let mut cmd = Command::new(&scrcpy_path);
        if let Some(parent) = scrcpy_path.parent() {
            cmd.current_dir(parent);
        }
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(0x08000000);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return LaunchResult {
                profile: None,
                screen_profile: None,
                ok: false,
                pid: None,
                error: Some(format!("Error al iniciar scrcpy: {e}")),
            },
        };

        let pid = child.id().unwrap_or(0);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let started_at = Instant::now();
        let log_buffer = Arc::new(Mutex::new(String::new()));

        // Stream de stdout
        if let Some(out) = stdout {
            let serial_c = serial.clone();
            let app_c = app.clone();
            let buf_c = log_buffer.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                let re_tex = Regex::new(r"(?i)INFO:\s*Texture:\s*(\d+)x(\d+)").unwrap();
                while let Ok(Some(line)) = reader.next_line().await {
                    {
                        let mut b = buf_c.lock().await;
                        b.push_str(&line);
                        b.push('\n');
                        cap_log(&mut b);
                    }
                    if let Some(caps) = re_tex.captures(&line) {
                        let w: u32 = caps[1].parse().unwrap_or(0);
                        let h: u32 = caps[2].parse().unwrap_or(0);
                        let _ = app_c.emit("scrcpy:texture", TextureEvent {
                            serial: serial_c.clone(),
                            width: w,
                            height: h,
                            resolution: format!("{w}×{h}"),
                        });
                    }
                }
            });
        }

        // Stream de stderr
        if let Some(err) = stderr {
            let buf_c = log_buffer.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut b = buf_c.lock().await;
                    b.push_str(&line);
                    b.push('\n');
                    cap_log(&mut b);
                }
            });
        }

        // Registrar sesión activa
        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(serial.clone(), SessionControl {
                child,
                generation,
                options: options_for_session,
            });
        }

        // Tarea de monitoreo de salida del proceso
        let sessions_ref = self.sessions.clone();
        let serial_for_monitor = serial.clone();
        let app_for_monitor = app.clone();
        let log_for_monitor = log_buffer.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut lock = sessions_ref.lock().await;
                if let Some(session) = lock.get_mut(&serial_for_monitor) {
                    if session.generation != generation {
                        return; // Reemplazado por otra sesión
                    }
                    match session.child.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code();
                            let removed = lock.remove(&serial_for_monitor);
                            drop(lock);

                            let log = log_for_monitor.lock().await.clone();
                            let run_time = started_at.elapsed();

                            // Corte de conexión (cable, cargador, Wi-Fi) tras
                            // funcionar un rato: reconectar en vez de cerrar.
                            // "Camera disconnected" = fallo de la cámara, no del
                            // enlace: reintentar no lo arregla.
                            let lower = log.to_lowercase();
                            if run_time > Duration::from_secs(5)
                                && lower.contains("device disconnected")
                                && !lower.contains("camera disconnected")
                            {
                                if let Some(sess) = removed {
                                    schedule_reconnect(app_for_monitor, serial_for_monitor, sess.options, log);
                                    return;
                                }
                            }
                            let early_fail = run_time < Duration::from_millis(2500);

                            let looks_err = log.to_lowercase().contains("error")
                                || log.to_lowercase().contains("failed")
                                || log.to_lowercase().contains("exception")
                                || log.to_lowercase().contains("unable")
                                || log.to_lowercase().contains("denied");

                            let error = if early_fail && (code != Some(0) || looks_err || !log.trim().is_empty()) {
                                Some(if log.trim().is_empty() {
                                    format!("scrcpy terminó (código {:?})", code)
                                } else {
                                    log.trim().to_string()
                                })
                            } else if code.unwrap_or(0) != 0 && looks_err {
                                Some(log.trim().to_string())
                            } else {
                                None
                            };

                            let _ = app_for_monitor.emit("scrcpy:stopped", StoppedEvent {
                                serial: serial_for_monitor,
                                error,
                                code,
                                early_fail,
                            });
                            return;
                        }
                        Ok(None) => {}
                        Err(_) => {
                            lock.remove(&serial_for_monitor);
                            return;
                        }
                    }
                } else {
                    return;
                }
            }
        });

        // Verificación de early-fail (900ms)
        tokio::time::sleep(Duration::from_millis(900)).await;
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get_mut(&serial) {
                if session.generation == generation {
                    if let Ok(Some(status)) = session.child.try_wait() {
                        let code = status.code().unwrap_or(-1);
                        sessions.remove(&serial);
                        let log = log_buffer.lock().await.clone();
                        return LaunchResult {
                            profile: None,
                            screen_profile: None,
                            ok: false,
                            pid: None,
                            error: Some(if log.trim().is_empty() {
                                format!("scrcpy salió de inmediato (código {code})")
                            } else {
                                log.trim().to_string()
                            }),
                        };
                    }
                }
            }
        }

        LaunchResult {
            profile: applied_profile,
            screen_profile: applied_screen,
            ok: true,
            pid: Some(pid),
            error: None,
        }
    }
}

// ── Reconexión automática ─────────────────────────────────────────────────

const RECONNECT_ATTEMPTS: u32 = 3;
const RECONNECT_WAIT_DEVICE: Duration = Duration::from_secs(20);

/// Función normal (no async) para cortar el ciclo de tipos
/// launch -> monitor -> reconexión -> launch.
fn schedule_reconnect(app: AppHandle, serial: String, options: ScrcpyOptions, log: String) {
    tokio::spawn(reconnect_task(app, serial, options, log));
}

async fn reconnect_task(app: AppHandle, serial: String, options: ScrcpyOptions, log: String) {
    use tauri::Manager;
    let mgr = app.state::<ScrcpyManager>().inner().clone();
    mgr.reconnecting.lock().await.insert(serial.clone());
    let still_wanted = |mgr: ScrcpyManager, serial: String| async move {
        mgr.reconnecting.lock().await.contains(&serial)
    };
    let tools = profile::tools();
    let mut last = log
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Se perdió la conexión")
        .trim()
        .to_string();

    for attempt in 1..=RECONNECT_ATTEMPTS {
        let _ = app.emit(
            "scrcpy:reconnecting",
            serde_json::json!({ "serial": serial, "attempt": attempt, "of": RECONNECT_ATTEMPTS, "reason": last }),
        );
        let online = match &tools {
            Ok(t) => profile::wait_for_device(&t.adb, &serial, RECONNECT_WAIT_DEVICE).await,
            Err(_) => false,
        };
        if !still_wanted(mgr.clone(), serial.clone()).await {
            return; // el usuario la detuvo; la UI ya lo sabe
        }
        if online {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let res = mgr.launch(app.clone(), serial.clone(), options.clone()).await;
            if res.ok {
                mgr.reconnecting.lock().await.remove(&serial);
                let _ = app.emit(
                    "scrcpy:reconnected",
                    serde_json::json!({ "serial": serial, "pid": res.pid, "profile": res.profile }),
                );
                let level = options.throttle.unwrap_or(0);
                app.state::<crate::health::HealthMonitor>()
                    .ensure(serial.clone(), level, app.clone())
                    .await;
                return;
            }
            last = res.error.unwrap_or_else(|| "scrcpy no arrancó".into());
        } else {
            last = "El teléfono no volvió a conectarse".into();
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    mgr.reconnecting.lock().await.remove(&serial);
    let _ = app.emit(
        "scrcpy:stopped",
        StoppedEvent {
            serial,
            error: Some(format!("Se perdió la conexión y no se pudo recuperar: {last}")),
            code: None,
            early_fail: false,
        },
    );
}

// ── Geometría de la ventana de scrcpy (Win32) ─────────────────────────────

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

type Hwnd = *mut std::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(cb: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> i32;
}

struct FindWindow {
    pid: u32,
    found: Hwnd,
}

unsafe extern "system" fn enum_cb(hwnd: Hwnd, lparam: isize) -> i32 {
    unsafe {
        let ctx = &mut *(lparam as *mut FindWindow);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == ctx.pid && IsWindowVisible(hwnd) != 0 {
            ctx.found = hwnd;
            return 0; // parar
        }
        1
    }
}

/// Área de la ventana visible del proceso (x, y, ancho, alto) en pantalla.
fn window_rect(pid: u32) -> Option<(i32, i32, u32, u32)> {
    unsafe {
        let mut ctx = FindWindow { pid, found: std::ptr::null_mut() };
        EnumWindows(enum_cb, &mut ctx as *mut FindWindow as isize);
        let hwnd = ctx.found;
        if hwnd.is_null() || IsIconic(hwnd) != 0 {
            return None; // minimizada: que scrcpy decida
        }
        let mut r = Rect::default();
        if GetClientRect(hwnd, &mut r) == 0 {
            return None;
        }
        let mut p = Point::default();
        if ClientToScreen(hwnd, &mut p) == 0 {
            return None;
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w < 100 || h < 100 {
            return None;
        }
        Some((p.x, p.y, w as u32, h as u32))
    }
}
