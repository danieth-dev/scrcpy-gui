//! Capacidades reales del dispositivo + resolución de perfil de cámara.
//!
//! Los ajustes de la UI son *deseos* (1080p, 60 fps, AV1…). Este módulo consulta
//! al teléfono qué puede hacer de verdad (cámaras, tamaños, fps, zoom,
//! codificadores hardware) y traduce los deseos a una configuración concreta
//! que el dispositivo soporta, sin forzarlo (nada de codificadores por
//! software). Lo usan tanto la cámara en ventana (scrcpy.rs) como la cámara
//! virtual (vcam.rs), para que ambas apliquen exactamente lo mismo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::tools::{find_adb, find_scrcpy};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";

// ── Utilidades de proceso compartidas ─────────────────────────────────────

pub fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub async fn run_adb(adb: &Path, args: &[&str]) -> Result<String, String> {
    let out = no_window(Command::new(adb).args(args))
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Error ejecutando adb: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!("adb {}: {}", args.join(" "), stderr.trim()))
    }
}

/// La versión del servidor debe coincidir exactamente con la que se le pasa.
pub async fn scrcpy_version(scrcpy: &Path) -> Result<String, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().unwrap().get(scrcpy) {
        return Ok(v.clone());
    }
    let out = no_window(Command::new(scrcpy).arg("--version"))
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("Error ejecutando scrcpy --version: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("scrcpy "))
        .and_then(|rest| rest.split_whitespace().next())
        .map(|v| v.to_string())
        .ok_or_else(|| "No se pudo leer la versión de scrcpy".to_string())?;
    cache.lock().unwrap().insert(scrcpy.to_path_buf(), v.clone());
    Ok(v)
}

/// Espera a que el teléfono vuelva a estar disponible en adb (tras un corte de
/// USB, un cambio de cargador o una caída de Wi-Fi). Devuelve true si volvió.
pub async fn wait_for_device(adb: &Path, serial: &str, timeout: Duration) -> bool {
    let is_ip = serial.contains(':') && !serial.contains("._adb-tls-connect");
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if is_ip {
            let _ = run_adb(adb, &["connect", serial]).await;
        }
        if let Ok(state) = run_adb(adb, &["-s", serial, "get-state"]).await {
            if state.trim() == "device" {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    false
}

/// Rutas necesarias para hablar con el servidor scrcpy.
pub struct Tools {
    pub scrcpy: PathBuf,
    pub adb: PathBuf,
    pub server: PathBuf,
}

pub fn tools() -> Result<Tools, String> {
    let scrcpy = find_scrcpy().ok_or("No se encontró scrcpy.exe")?;
    let adb = find_adb()
        .or_else(|| scrcpy.parent().map(|d| d.join("adb.exe")).filter(|p| p.is_file()))
        .ok_or("No se encontró adb.exe")?;
    let server = scrcpy
        .parent()
        .map(|d| d.join("scrcpy-server"))
        .filter(|p| p.is_file())
        .ok_or("No se encontró el fichero scrcpy-server junto a scrcpy.exe")?;
    Ok(Tools { scrcpy, adb, server })
}

// ── Capacidades del dispositivo ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CameraInfo {
    pub id: String,
    pub facing: String,
    /// Tamaños soportados (en orientación del sensor: ancho ≥ alto).
    pub sizes: Vec<(u32, u32)>,
    /// fps soportados en modo normal (rango AE fijo).
    pub fps: Vec<u32>,
    #[serde(rename = "zoomMax")]
    pub zoom_max: Option<f32>,
    /// Modo alta velocidad: (tamaño, fps disponibles).
    #[serde(rename = "highSpeed")]
    pub high_speed: Vec<((u32, u32), Vec<u32>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EncoderInfo {
    pub codec: String,
    pub name: String,
    pub hw: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceCaps {
    pub cameras: Vec<CameraInfo>,
    pub encoders: Vec<EncoderInfo>,
    pub display: Option<DisplayInfo>,
}

/// Pantalla del teléfono (tamaño en uso y frecuencias de refresco).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub refresh: Vec<u32>,
}

/// Interpreta `wm size` + la línea `supportedRefreshRates [...]` de
/// `dumpsys display`. Si hay "Override size" (resolución reducida en los
/// ajustes del teléfono), manda ese: es lo que se captura.
pub fn parse_display(out: &str) -> Option<DisplayInfo> {
    let mut physical = None;
    let mut over = None;
    let mut refresh: Vec<u32> = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Physical size:") {
            physical = parse_size(v);
        } else if let Some(v) = l.strip_prefix("Override size:") {
            over = parse_size(v);
        } else if let Some(rest) = l.split("supportedRefreshRates [").nth(1) {
            let list = rest.split(']').next().unwrap_or("");
            for x in list.split(',') {
                if let Ok(f) = x.trim().parse::<f32>() {
                    let f = f.round() as u32;
                    if f > 0 && !refresh.contains(&f) {
                        refresh.push(f);
                    }
                }
            }
        }
    }
    let (w, h) = over.or(physical)?;
    refresh.sort_unstable_by(|a, b| b.cmp(a));
    if refresh.is_empty() {
        refresh.push(60);
    }
    Some(DisplayInfo { width: w, height: h, refresh })
}

impl DeviceCaps {
    pub fn hw_encoder(&self, codec: &str) -> Option<&EncoderInfo> {
        self.encoders.iter().find(|e| e.hw && e.codec == codec)
    }

    pub fn hw_codecs(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for e in self.encoders.iter().filter(|e| e.hw) {
            if !v.contains(&e.codec) {
                v.push(e.codec.clone());
            }
        }
        v
    }
}

fn caps_cache() -> &'static Mutex<HashMap<String, DeviceCaps>> {
    static CACHE: OnceLock<Mutex<HashMap<String, DeviceCaps>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.trim().split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn parse_num_list(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

/// Interpreta la salida de `list_encoders=true list_camera_sizes=true`.
pub fn parse_caps(out: &str) -> DeviceCaps {
    let mut cameras: Vec<CameraInfo> = Vec::new();
    let mut encoders = Vec::new();
    let mut in_high_speed = false;

    for raw in out.lines() {
        let line = raw.trim();

        // --video-codec=h264 --video-encoder=c2.exynos.h264.encoder   (hw) [vendor]
        if let Some(rest) = line.strip_prefix("--video-codec=") {
            let codec = rest.split_whitespace().next().unwrap_or("").to_string();
            let name = line
                .split("--video-encoder=")
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
                .unwrap_or("")
                .to_string();
            // Los alias apuntan a otro codificador ya listado.
            if !codec.is_empty() && !name.is_empty() && !line.contains("(alias for") {
                encoders.push(EncoderInfo { codec, name, hw: line.contains("(hw)") });
            }
            continue;
        }

        // --camera-id=0    (back, 4080x3060, fps={15, 20, 24, 30}, zoom-range=[1, 8])
        if let Some(rest) = line.strip_prefix("--camera-id=") {
            in_high_speed = false;
            let id = rest.split_whitespace().next().unwrap_or("").to_string();
            let inside = rest.split_once('(').map(|(_, r)| r.trim_end_matches(')')).unwrap_or("");
            let facing = inside.split(',').next().unwrap_or("").trim().to_string();
            let fps = inside
                .split_once("fps={")
                .and_then(|(_, r)| r.split_once('}'))
                .map(|(l, _)| parse_num_list(l))
                .unwrap_or_default();
            let zoom_max = inside
                .split_once("zoom-range=[")
                .and_then(|(_, r)| r.split_once(']'))
                .and_then(|(l, _)| l.split(',').nth(1))
                .and_then(|m| m.trim().parse::<f32>().ok());
            cameras.push(CameraInfo { id, facing, sizes: vec![], fps, zoom_max, high_speed: vec![] });
            continue;
        }

        if line.starts_with("High speed capture") {
            in_high_speed = true;
            continue;
        }

        // - 1920x1080            |   - 1280x720 (fps={120, 240})
        if let Some(rest) = line.strip_prefix("- ") {
            let Some(cam) = cameras.last_mut() else { continue };
            let size_txt = rest.split_whitespace().next().unwrap_or("");
            let Some(size) = parse_size(size_txt) else { continue };
            if in_high_speed {
                let fps = rest
                    .split_once("fps={")
                    .and_then(|(_, r)| r.split_once('}'))
                    .map(|(l, _)| parse_num_list(l))
                    .unwrap_or_default();
                cam.high_speed.push((size, fps));
            } else {
                cam.sizes.push(size);
            }
        }
    }

    DeviceCaps { cameras, encoders, display: None }
}

/// Consulta (y cachea) las capacidades del dispositivo. ~1-2 s la primera vez.
pub async fn device_caps(serial: &str) -> Result<DeviceCaps, String> {
    if let Some(c) = caps_cache().lock().unwrap().get(serial) {
        return Ok(c.clone());
    }
    let t = tools()?;
    let version = scrcpy_version(&t.scrcpy).await?;
    let server = t.server.to_string_lossy().to_string();
    run_adb(&t.adb, &["-s", serial, "push", &server, DEVICE_SERVER_PATH]).await?;
    let cmd = format!(
        "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server {version} \
         log_level=info list_encoders=true list_camera_sizes=true"
    );
    let fut = no_window(Command::new(&t.adb).args(["-s", serial, "shell", &cmd]))
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(Duration::from_secs(12), fut)
        .await
        .map_err(|_| "Tiempo agotado consultando las capacidades del dispositivo".to_string())?
        .map_err(|e| format!("Error consultando capacidades: {e}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut caps = parse_caps(&text);
    // Pantalla: tamaño en uso y refresco (barato, sin el servidor).
    let disp_cmd = "wm size; dumpsys display 2>/dev/null | grep -m1 -o 'supportedRefreshRates \\[[^]]*\\]'";
    if let Ok(Ok(out)) = tokio::time::timeout(
        Duration::from_secs(5),
        run_adb(&t.adb, &["-s", serial, "shell", disp_cmd]),
    )
    .await
    {
        caps.display = parse_display(&out);
    }
    if caps.cameras.is_empty() && caps.encoders.is_empty() {
        return Err("El dispositivo no informó cámaras ni codificadores".into());
    }
    caps_cache().lock().unwrap().insert(serial.to_string(), caps.clone());
    Ok(caps)
}

pub fn forget_device(serial: &str) {
    caps_cache().lock().unwrap().remove(serial);
}

// ── Resolución de perfil ──────────────────────────────────────────────────

/// Lo que pide la UI (todos opcionales; se normalizan).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProfileRequest {
    #[serde(rename = "cameraFacing")]
    pub facing: Option<String>,
    /// Altura máxima deseada (720/1080/1440). 0 = la mayor disponible.
    #[serde(rename = "maxSize")]
    pub max_size: Option<u32>,
    #[serde(rename = "maxFps")]
    pub fps: Option<u32>,
    pub codec: Option<String>,
    /// Mbps.
    pub bitrate: Option<u32>,
    #[serde(rename = "cameraZoom")]
    pub zoom: Option<f64>,
    #[serde(rename = "cameraTorch")]
    pub torch: Option<bool>,
    pub orientation: Option<String>,
    /// Nivel de protección térmica/batería (0-2) que fija el backend; la UI
    /// no lo envía.
    #[serde(default, skip_deserializing)]
    pub throttle: Option<u8>,
    /// "max" = lo mejor que dé este teléfono (ignora resolución, fps, códec
    /// y bitrate); "custom" o vacío = lo que pida el usuario.
    #[serde(default)]
    pub quality: Option<String>,
    /// Conexión por Wi-Fi (la fija el backend según el serial).
    #[serde(default, skip_deserializing)]
    pub wifi: bool,
}

/// ¿Serial de una conexión inalámbrica? (IP:puerto o adb-tls por mDNS)
pub fn is_wifi_serial(serial: &str) -> bool {
    serial.contains(':') || serial.contains("_adb-tls-connect")
}

/// Bitrate automático de "Máxima calidad": el recomendado para esa imagen,
/// sin pasarse en Wi-Fi (que no suele sostener más de ~16 Mbps estables).
fn auto_bitrate(w: u32, h: u32, fps: u32, codec: &str, wifi: bool) -> u32 {
    let b = recommended_mbps(w, h, fps, codec).clamp(8, 40);
    if wifi { b.min(16) } else { b }
}

/// Mejor códec por hardware disponible para el destino.
fn best_hw_codec(caps: &DeviceCaps, target: Target) -> &'static str {
    if caps.hw_encoder("h265").is_some() {
        "h265"
    } else if target == Target::Window && caps.hw_encoder("av1").is_some() {
        "av1"
    } else {
        "h264"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Ventana de scrcpy: el cliente decodifica H.264/H.265/AV1.
    Window,
    /// Cámara virtual: decodificador en proceso + Unity Capture (máx. 4K, H.264/H.265).
    Vcam,
}

/// Configuración concreta que se aplicará.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedProfile {
    #[serde(rename = "cameraId")]
    pub camera_id: Option<String>,
    pub facing: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    #[serde(rename = "highSpeed")]
    pub high_speed: bool,
    pub codec: String,
    pub encoder: Option<String>,
    #[serde(rename = "bitrateMbps")]
    pub bitrate_mbps: u32,
    pub zoom: f32,
    pub torch: bool,
    /// 0/90/180/270 (rotación de la captura).
    pub orientation: u32,
    /// Ajustes que no se pudieron aplicar tal cual, en lenguaje de usuario.
    pub notes: Vec<String>,
    /// false si no se pudieron leer las capacidades (perfil "a ciegas").
    pub verified: bool,
    /// Nivel de protección térmica aplicado (0 = calidad completa).
    pub throttle: u8,
}

impl ResolvedProfile {
    /// ¿Cambia algo que obligue a relanzar la sesión? (las notas no cuentan)
    pub fn differs(&self, other: &ResolvedProfile) -> bool {
        self.camera_id != other.camera_id
            || self.facing != other.facing
            || self.width != other.width
            || self.height != other.height
            || self.fps != other.fps
            || self.high_speed != other.high_speed
            || self.codec != other.codec
            || self.encoder != other.encoder
            || self.bitrate_mbps != other.bitrate_mbps
            || (self.zoom - other.zoom).abs() > 0.01
            || self.torch != other.torch
            || self.orientation != other.orientation
    }

    /// Dimensiones del vídeo que sale del teléfono (tras rotar).
    pub fn output_size(&self) -> (u32, u32) {
        if self.orientation == 90 || self.orientation == 270 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

fn is_16_9(w: u32, h: u32) -> bool {
    // tolerancia del 2 %
    let a = (w as i64) * 9;
    let b = (h as i64) * 16;
    (a - b).abs() * 50 <= a
}

/// Bitrate orientativo para buena calidad H.264 (≈0,12 bits por píxel).
fn recommended_mbps(w: u32, h: u32, fps: u32, codec: &str) -> u32 {
    let bpp = if codec == "h264" { 0.12 } else { 0.08 };
    ((w as f64 * h as f64 * fps as f64 * bpp) / 1e6).ceil().max(2.0) as u32
}

fn codec_label(c: &str) -> &'static str {
    match c {
        "h265" => "H.265",
        "av1" => "AV1",
        _ => "H.264",
    }
}

fn label(h: u32) -> String {
    match h {
        2160 => "4K".into(),
        1440 => "1440p".into(),
        1080 => "1080p".into(),
        720 => "720p".into(),
        _ => format!("{h}p"),
    }
}

pub fn resolve(caps: Option<&DeviceCaps>, req: &ProfileRequest, target: Target) -> ResolvedProfile {
    let mut notes = Vec::new();

    let want_facing = match req.facing.as_deref() {
        Some("back") => "back",
        Some("external") => "external",
        _ => "front",
    };
    let is_max = req.quality.as_deref() == Some("max");
    let mut want_h = match req.max_size.unwrap_or(1080) {
        _ if is_max => u32::MAX,
        0 => u32::MAX,
        h => h,
    };
    let mut want_fps = req.fps.unwrap_or(30).clamp(1, 240);
    let want_codec = match req.codec.as_deref() {
        Some("h265") | Some("hevc") => "h265",
        Some("av1") => "av1",
        _ => "h264",
    };
    let mut bitrate_mbps = req.bitrate.unwrap_or(8).clamp(1, 100);

    // Protección del dispositivo: menos píxeles, fps y bitrate = menos calor
    // en el codificador y la cámara.
    let throttle = req.throttle.unwrap_or(0).min(2);
    match throttle {
        1 => {
            want_h = match want_h {
                h if h > 1440 => 1440,
                h if h > 1080 => 1080,
                _ => 720,
            };
            bitrate_mbps = (bitrate_mbps * 7 / 10).max(2);
            notes.push("Protección térmica: calidad reducida un nivel para enfriar el teléfono".into());
        }
        2 => {
            want_h = want_h.min(720);
            want_fps = want_fps.min(24);
            bitrate_mbps = (bitrate_mbps / 2).clamp(2, 6);
            notes.push("Protección térmica: calidad mínima (720p) para enfriar o ahorrar batería".into());
        }
        _ => {}
    }
    let want_zoom = req.zoom.unwrap_or(1.0).clamp(1.0, 100.0) as f32;
    let orientation = match req.orientation.as_deref() {
        Some("90") => 90,
        Some("180") => 180,
        Some("270") => 270,
        _ => 0,
    };
    let codec_ok_for_target = |c: &str| target == Target::Window || c == "h264" || c == "h265";

    // Sin capacidades: perfil conservador que funciona en casi cualquier móvil.
    let Some(caps) = caps.filter(|c| !c.cameras.is_empty()) else {
        let h = if want_h == u32::MAX { 1080 } else { want_h.min(2160) };
        let w = h * 16 / 9;
        let codec = if codec_ok_for_target(want_codec) { want_codec } else { "h264" };
        notes.push("No se pudieron leer las capacidades del teléfono; se usa un perfil estándar".into());
        return ResolvedProfile {
            camera_id: None,
            facing: want_facing.into(),
            width: w & !1,
            height: h,
            fps: want_fps.min(30),
            high_speed: false,
            codec: codec.into(),
            encoder: None,
            bitrate_mbps,
            zoom: want_zoom,
            torch: req.torch.unwrap_or(false) && want_facing == "back",
            orientation,
            notes,
            verified: false,
            throttle,
        };
    };

    // 1. Cámara
    let cam = caps
        .cameras
        .iter()
        .find(|c| c.facing == want_facing)
        .unwrap_or_else(|| {
            notes.push(format!("No hay cámara '{want_facing}'; se usa la {}", caps.cameras[0].facing));
            &caps.cameras[0]
        });

    // 2. Alta velocidad: NO se usa automáticamente. Muchos fabricantes
    //    (p. ej. Samsung) anuncian 120/240 fps pero cierran la cámara al abrir
    //    una sesión de alta velocidad desde fuera de su app (probado en un
    //    SM-A556E: "Camera disconnected" con H.264 y H.265, 120 y 240 fps).
    //    Fiabilidad primero: se queda en el máximo de fps normales.
    let high_speed = false;
    let size: Option<(u32, u32)> = None;
    let mut fps = 0u32;
    let normal_max = cam.fps.iter().copied().max().unwrap_or(30);
    if is_max {
        // Lo máximo fiable de esta cámara (con protección nivel 2, ≤24).
        want_fps = if throttle >= 2 { normal_max.min(24) } else { normal_max };
    }
    if want_fps > normal_max && !cam.high_speed.is_empty() {
        notes.push(format!(
            "La cámara anuncia más de {normal_max} fps sólo en modo alta velocidad, que no es fiable fuera de la app del fabricante"
        ));
    }

    // 3. Tamaño normal
    let size = size.unwrap_or_else(|| {
        // Tope 4K UHD en ambos modos: por encima (p. ej. 4080x2296) muchos
        // codificadores hardware fallan al arrancar; y la cámara virtual no
        // admite más.
        let fits = |&&(w, h): &&(u32, u32)| h <= want_h && w <= 3840 && h <= 2160;
        let area = |s: &&(u32, u32)| s.0 as u64 * s.1 as u64;
        let pick = cam
            .sizes
            .iter()
            .filter(fits)
            .filter(|s| is_16_9(s.0, s.1))
            .max_by_key(area)
            .or_else(|| cam.sizes.iter().filter(fits).max_by_key(area))
            .or_else(|| cam.sizes.iter().min_by_key(area))
            .copied()
            .unwrap_or((1920, 1080));
        if want_h != u32::MAX && pick.1 != want_h {
            notes.push(format!(
                "La cámara no tiene {} en 16:9; se usa {}x{}",
                label(want_h),
                pick.0,
                pick.1
            ));
        }
        pick
    });

    // 4. fps normales
    if !high_speed {
        let list = &cam.fps;
        fps = list.iter().copied().filter(|f| *f <= want_fps).max().unwrap_or_else(|| {
            list.iter().copied().min().unwrap_or(want_fps.min(30))
        });
        if list.is_empty() {
            fps = want_fps.min(30);
        }
        if fps != want_fps && fps < want_fps && throttle == 0 && !is_max {
            notes.push(format!("La cámara admite como máximo {fps} fps a esta resolución"));
        }
    }

    // 5. Codec: sólo codificadores hardware (los de software calientan y van lentos)
    let want_codec = if is_max { best_hw_codec(caps, target) } else { want_codec };
    let mut codec = want_codec;
    if !codec_ok_for_target(codec) {
        notes.push(format!("{} no es compatible con la cámara virtual", codec_label(codec)));
        codec = "h265";
    }
    let mut encoder = caps.hw_encoder(codec).map(|e| e.name.clone());
    if encoder.is_none() {
        let fallback = ["h265", "h264"]
            .into_iter()
            .find(|c| codec_ok_for_target(c) && caps.hw_encoder(c).is_some());
        if let Some(fb) = fallback {
            if fb != want_codec {
                notes.push(format!(
                    "Sin codificador por hardware para {}; se usa {}",
                    codec_label(want_codec),
                    codec_label(fb)
                ));
            }
            codec = fb;
            encoder = caps.hw_encoder(fb).map(|e| e.name.clone());
        } else {
            codec = "h264"; // último recurso: lo que elija el sistema
        }
    }

    // 6. Zoom / linterna
    let mut zoom = want_zoom;
    if let Some(max) = cam.zoom_max {
        if zoom > max {
            notes.push(format!("Zoom máximo de esta cámara: {max}×"));
            zoom = max;
        }
    }
    let mut torch = req.torch.unwrap_or(false);
    if torch && cam.facing != "back" {
        notes.push("La linterna sólo está disponible con la cámara trasera".into());
        torch = false;
    }

    // 7. Bitrate: automático en "max"; si no, se respeta y se avisa si limita
    if is_max {
        bitrate_mbps = auto_bitrate(size.0, size.1, fps, codec, req.wifi);
        if throttle >= 2 {
            bitrate_mbps = bitrate_mbps.min(6);
        }
    }
    let rec = recommended_mbps(size.0, size.1, fps, codec);
    if throttle == 0 && !is_max && bitrate_mbps * 10 < rec * 7 {
        notes.push(format!(
            "{bitrate_mbps} Mbps puede verse comprimido a {}x{}@{fps}; recomendado ≈{rec} Mbps",
            size.0, size.1
        ));
    }

    ResolvedProfile {
        camera_id: Some(cam.id.clone()),
        facing: cam.facing.clone(),
        width: size.0,
        height: size.1,
        fps,
        high_speed,
        codec: codec.into(),
        encoder,
        bitrate_mbps,
        zoom,
        torch,
        orientation,
        notes,
        verified: true,
        throttle,
    }
}

/// Consulta capacidades (si se puede) y resuelve. Nunca falla: si no hay
/// capacidades devuelve un perfil conservador.
pub async fn resolve_for(serial: &str, req: &ProfileRequest, target: Target) -> ResolvedProfile {
    let caps = device_caps(serial).await.ok();
    let mut req = req.clone();
    req.wifi = is_wifi_serial(serial);
    resolve(caps.as_ref(), &req, target)
}

// ── Opciones válidas para la UI ───────────────────────────────────────────

/// Lo que puede elegir el usuario en una cámara de este teléfono.
#[derive(Debug, Clone, Serialize)]
pub struct CameraOption {
    pub id: String,
    pub facing: String,
    /// Tamaños 16:9 utilizables (≤ 4K), de mayor a menor, uno por altura.
    pub sizes: Vec<(u32, u32)>,
    pub fps: Vec<u32>,
    #[serde(rename = "zoomMax")]
    pub zoom_max: Option<f32>,
    pub torch: bool,
}

/// Primera cámara de cada orientación (la principal), como usa `resolve`.
pub fn camera_options(caps: &DeviceCaps) -> Vec<CameraOption> {
    let mut out: Vec<CameraOption> = Vec::new();
    for cam in &caps.cameras {
        if out.iter().any(|o| o.facing == cam.facing) {
            continue;
        }
        let mut sizes: Vec<(u32, u32)> = cam
            .sizes
            .iter()
            .copied()
            .filter(|&(w, h)| is_16_9(w, h) && w <= 3840 && h <= 2160 && h >= 360)
            .collect();
        sizes.sort_by_key(|&(w, h)| std::cmp::Reverse(w as u64 * h as u64));
        sizes.dedup_by_key(|s| s.1);
        let mut fps = cam.fps.clone();
        fps.sort_unstable();
        out.push(CameraOption {
            id: cam.id.clone(),
            facing: cam.facing.clone(),
            sizes,
            fps,
            zoom_max: cam.zoom_max,
            torch: cam.facing == "back",
        });
    }
    out
}

/// Opciones de "Ver pantalla" de este teléfono.
#[derive(Debug, Clone, Serialize)]
pub struct ScreenOptions {
    /// (lado corto, ancho, alto); lado corto 0 = nativa.
    pub sizes: Vec<(u32, u32, u32)>,
    pub fps: Vec<u32>,
}

fn screen_size_for(d: &DisplayInfo, short: u32) -> (u32, u32) {
    let (dw, dh) = (d.width, d.height);
    let native_short = dw.min(dh);
    if short == 0 || short >= native_short {
        return (dw, dh);
    }
    let scale = short as f64 / native_short as f64;
    let w = ((dw as f64 * scale).round() as u32) & !1;
    let h = ((dh as f64 * scale).round() as u32) & !1;
    (w, h)
}

pub fn screen_options(d: &DisplayInfo) -> ScreenOptions {
    let native_short = d.width.min(d.height);
    let mut sizes = vec![(0, d.width, d.height)];
    for short in [1440u32, 1080, 720, 480] {
        if short < native_short {
            let (w, h) = screen_size_for(d, short);
            sizes.push((short, w, h));
        }
    }
    let max_refresh = d.refresh.first().copied().unwrap_or(60);
    let mut fps: Vec<u32> = [30u32, 60, 90, 120, 144]
        .into_iter()
        .filter(|f| *f <= max_refresh && (*f <= 60 || d.refresh.contains(f)))
        .collect();
    if fps.is_empty() {
        fps.push(max_refresh);
    }
    ScreenOptions { sizes, fps }
}

// ── "Ver pantalla" ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ScreenRequest {
    /// Lado corto deseado (720, 1080…); 0 = nativa.
    pub res: Option<u32>,
    pub fps: Option<u32>,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub quality: Option<String>,
    pub wifi: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ScreenProfile {
    /// Valor para `--max-size` (lado largo); 0 = sin límite (nativa).
    #[serde(rename = "maxSize")]
    pub max_size: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: String,
    pub encoder: Option<String>,
    #[serde(rename = "bitrateMbps")]
    pub bitrate_mbps: u32,
    pub notes: Vec<String>,
}

impl ScreenProfile {
    pub fn differs(&self, o: &ScreenProfile) -> bool {
        self.max_size != o.max_size
            || self.fps != o.fps
            || self.codec != o.codec
            || self.encoder != o.encoder
            || self.bitrate_mbps != o.bitrate_mbps
    }
}

/// Traduce los ajustes de pantalla a lo que da este teléfono. "1080p" se
/// interpreta por el lado CORTO (en un 1080×2340 es la nativa): `--max-size`
/// de scrcpy limita el lado largo y, a pelo, reducía la imagen a ~498×1080.
pub fn resolve_screen(caps: Option<&DeviceCaps>, req: &ScreenRequest) -> ScreenProfile {
    let mut notes = Vec::new();
    let is_max = req.quality.as_deref() == Some("max");
    let display = caps.and_then(|c| c.display.clone());

    let (max_size, width, height, max_refresh) = match &display {
        Some(d) => {
            let short = if is_max { 0 } else { req.res.unwrap_or(0) };
            let (w, h) = screen_size_for(d, short);
            let native = (w, h) == (d.width, d.height);
            let max_size = if native { 0 } else { w.max(h) };
            (max_size, w, h, d.refresh.first().copied().unwrap_or(60))
        }
        None => {
            // Sin datos de pantalla: no limitar (scrcpy usa la nativa).
            let short = if is_max { 0 } else { req.res.unwrap_or(0) };
            let ms = if short == 0 { 0 } else { (short as f64 * 2.2) as u32 };
            (ms, 0, 0, 60)
        }
    };

    let want_fps = if is_max { max_refresh } else { req.fps.unwrap_or(60).clamp(1, 240) };
    let fps = want_fps.min(max_refresh);
    if fps < want_fps && display.is_some() {
        notes.push(format!("La pantalla refresca como máximo a {max_refresh} Hz"));
    }

    let want_codec = match (is_max, caps) {
        (true, Some(c)) => best_hw_codec(c, Target::Window),
        _ => match req.codec.as_deref() {
            Some("h265") | Some("hevc") => "h265",
            Some("av1") => "av1",
            _ => "h264",
        },
    };
    let (codec, encoder) = match caps {
        Some(c) => match c.hw_encoder(want_codec) {
            Some(e) => (want_codec, Some(e.name.clone())),
            None => {
                let fb = if c.hw_encoder("h265").is_some() { "h265" } else { "h264" };
                if fb != want_codec {
                    notes.push(format!(
                        "Sin codificador por hardware para {}; se usa {}",
                        codec_label(want_codec),
                        codec_label(fb)
                    ));
                }
                (fb, c.hw_encoder(fb).map(|e| e.name.clone()))
            }
        },
        None => (want_codec, None),
    };

    let bitrate_mbps = if is_max && width > 0 {
        // La pantalla cambia menos que una cámara: con 60 fps de referencia
        // sobra aunque refresque a 120.
        auto_bitrate(width, height, fps.min(60), codec, req.wifi)
    } else {
        req.bitrate.unwrap_or(8).clamp(1, 100)
    };

    ScreenProfile {
        max_size,
        width,
        height,
        fps,
        codec: codec.into(),
        encoder,
        bitrate_mbps,
        notes,
    }
}

pub async fn resolve_screen_for(serial: &str, req: &ScreenRequest) -> ScreenProfile {
    let caps = device_caps(serial).await.ok();
    let mut req = req.clone();
    req.wifi = is_wifi_serial(serial);
    resolve_screen(caps.as_ref(), &req)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[server] INFO: List of video encoders:
    --video-codec=h264 --video-encoder=c2.exynos.h264.encoder         (hw) [vendor]
    --video-codec=h264 --video-encoder=c2.android.avc.encoder         (sw)
    --video-codec=h264 --video-encoder=c2.exynos.h264.wfd.encoder     (hw) [vendor] (alias for c2.exynos.h264.encoder)
    --video-codec=h265 --video-encoder=c2.exynos.hevc.encoder         (hw) [vendor]
    --video-codec=av1 --video-encoder=c2.android.av1.encoder          (sw)
[server] INFO: List of cameras:
    --camera-id=0    (back, 4080x3060, fps={15, 20, 24, 30}, zoom-range=[1, 8])
        - 4080x3060
        - 3840x2160
        - 2560x1440
        - 1920x1440
        - 1920x1080
        - 1280x720
        - 640x480
      High speed capture (--camera-high-speed):
        - 1280x720 (fps={120, 240})
    --camera-id=1    (front, 3264x2448, fps={15, 20, 24, 30}, zoom-range=[1, 4])
        - 3264x2448
        - 2560x1440
        - 1920x1080
        - 1280x720
";

    fn req(f: &str, h: u32, fps: u32, codec: &str) -> ProfileRequest {
        ProfileRequest {
            facing: Some(f.into()),
            max_size: Some(h),
            fps: Some(fps),
            codec: Some(codec.into()),
            bitrate: Some(20),
            ..Default::default()
        }
    }

    #[test]
    fn parses_device() {
        let c = parse_caps(SAMPLE);
        assert_eq!(c.cameras.len(), 2);
        assert_eq!(c.cameras[0].fps, vec![15, 20, 24, 30]);
        assert_eq!(c.cameras[0].zoom_max, Some(8.0));
        assert_eq!(c.cameras[0].high_speed, vec![((1280, 720), vec![120, 240])]);
        assert_eq!(c.cameras[1].facing, "front");
        assert_eq!(c.hw_codecs(), vec!["h264", "h265"]);
        assert_eq!(c.encoders.len(), 4); // alias descartado
    }

    #[test]
    fn exact_sizes() {
        let c = parse_caps(SAMPLE);
        let p = resolve(Some(&c), &req("front", 1440, 30, "h264"), Target::Vcam);
        assert_eq!((p.width, p.height, p.fps), (2560, 1440, 30));
        assert_eq!(p.camera_id.as_deref(), Some("1"));
        assert_eq!(p.encoder.as_deref(), Some("c2.exynos.h264.encoder"));
        assert!(p.notes.is_empty(), "{:?}", p.notes);
    }

    #[test]
    fn original_is_largest_16_9_capped_for_vcam() {
        let c = parse_caps(SAMPLE);
        let p = resolve(Some(&c), &req("back", 0, 30, "h264"), Target::Vcam);
        assert_eq!((p.width, p.height), (3840, 2160));
    }

    #[test]
    fn sixty_fps_falls_back_to_30() {
        let c = parse_caps(SAMPLE);
        let p = resolve(Some(&c), &req("front", 1080, 60, "h264"), Target::Window);
        assert_eq!(p.fps, 30);
        assert!(!p.high_speed);
        assert!(!p.notes.is_empty());
    }

    #[test]
    fn high_speed_is_never_auto_selected() {
        let c = parse_caps(SAMPLE);
        let p = resolve(Some(&c), &req("back", 1080, 120, "h264"), Target::Window);
        assert!(!p.high_speed);
        assert_eq!((p.width, p.height, p.fps), (1920, 1080, 30));
        assert!(p.notes.iter().any(|n| n.contains("alta velocidad")));
    }

    #[test]
    fn av1_software_only_falls_back_to_hw() {
        let c = parse_caps(SAMPLE);
        let p = resolve(Some(&c), &req("back", 1080, 30, "av1"), Target::Window);
        assert_eq!(p.codec, "h265");
        assert_eq!(p.encoder.as_deref(), Some("c2.exynos.hevc.encoder"));
    }

    #[test]
    fn torch_only_back_and_zoom_clamped() {
        let c = parse_caps(SAMPLE);
        let mut r = req("front", 1080, 30, "h264");
        r.torch = Some(true);
        r.zoom = Some(5.0);
        let p = resolve(Some(&c), &r, Target::Window);
        assert!(!p.torch);
        assert_eq!(p.zoom, 4.0);
    }

    #[test]
    fn throttle_levels() {
        let c = parse_caps(SAMPLE);
        let mut r = req("back", 0, 30, "h265");
        r.throttle = Some(1);
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!((p.width, p.height, p.bitrate_mbps), (2560, 1440, 14));
        r.throttle = Some(2);
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!((p.width, p.height, p.fps, p.bitrate_mbps), (1280, 720, 24, 6));
        assert_eq!(p.throttle, 2);
    }

    #[test]
    fn differs_ignores_notes_and_unreachable_fps() {
        let c = parse_caps(SAMPLE);
        let a = resolve(Some(&c), &req("front", 1080, 30, "h264"), Target::Vcam);
        // 60 fps no existe en esta cámara: el perfil resultante es el mismo
        let b = resolve(Some(&c), &req("front", 1080, 60, "h264"), Target::Vcam);
        assert!(!a.differs(&b));
        assert_ne!(a.notes, b.notes);
        let c720 = resolve(Some(&c), &req("front", 720, 30, "h264"), Target::Vcam);
        assert!(a.differs(&c720));
    }

    const A55_DISPLAY: &str = "Physical size: 1080x2340
supportedRefreshRates [120.00001, 60.0]
";

    fn a55() -> DeviceCaps {
        let mut c = parse_caps(SAMPLE);
        c.display = parse_display(A55_DISPLAY);
        c
    }

    #[test]
    fn parses_display() {
        let d = parse_display(A55_DISPLAY).unwrap();
        assert_eq!((d.width, d.height, d.refresh.clone()), (1080, 2340, vec![120, 60]));
        let o = parse_display("Physical size: 1440x3120\nOverride size: 1080x2340\n").unwrap();
        assert_eq!((o.width, o.height), (1080, 2340));
    }

    #[test]
    fn max_quality_camera() {
        let c = a55();
        let mut r = req("back", 720, 15, "h264");
        r.quality = Some("max".into());
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!((p.width, p.height, p.fps, p.codec.as_str()), (3840, 2160, 30, "h265"));
        assert_eq!(p.bitrate_mbps, 20);
        assert!(p.notes.is_empty(), "{:?}", p.notes);
        r.facing = Some("front".into());
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!((p.width, p.height), (2560, 1440)); // mayor 16:9 de SAMPLE
        r.wifi = true;
        r.facing = Some("back".into());
        assert!(resolve(Some(&c), &r, Target::Vcam).bitrate_mbps <= 16);
        r.wifi = false;
        r.throttle = Some(1);
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!((p.width, p.height), (2560, 1440));
    }

    #[test]
    fn camera_options_lists_real_choices() {
        let o = camera_options(&a55());
        let back = o.iter().find(|c| c.facing == "back").unwrap();
        assert_eq!(back.sizes, vec![(3840, 2160), (2560, 1440), (1920, 1080), (1280, 720)]);
        assert_eq!(back.fps, vec![15, 20, 24, 30]);
        assert!(back.torch);
        assert!(!o.iter().find(|c| c.facing == "front").unwrap().torch);
    }

    #[test]
    fn screen_resolution_uses_short_side() {
        let c = a55();
        let base = ScreenRequest { res: Some(1080), fps: Some(60), codec: Some("h264".into()), bitrate: Some(8), ..Default::default() };
        let p = resolve_screen(Some(&c), &base);
        assert_eq!((p.max_size, p.width, p.height), (0, 1080, 2340)); // nativa
        let p = resolve_screen(Some(&c), &ScreenRequest { res: Some(720), ..base.clone() });
        assert_eq!((p.max_size, p.width, p.height), (1560, 720, 1560));
        let p = resolve_screen(Some(&c), &ScreenRequest { fps: Some(144), ..base.clone() });
        assert_eq!(p.fps, 120);
        let p = resolve_screen(Some(&c), &ScreenRequest { quality: Some("max".into()), ..base.clone() });
        assert_eq!((p.max_size, p.fps, p.codec.as_str()), (0, 120, "h265"));
        let o = screen_options(c.display.as_ref().unwrap());
        assert_eq!(o.fps, vec![30, 60, 120]);
        assert_eq!(o.sizes[0], (0, 1080, 2340));
        assert_eq!(o.sizes[1], (720, 720, 1560));
    }

    #[test]
    fn rotated_output_size() {
        let c = parse_caps(SAMPLE);
        let mut r = req("front", 1080, 30, "h264");
        r.orientation = Some("90".into());
        let p = resolve(Some(&c), &r, Target::Vcam);
        assert_eq!(p.output_size(), (1080, 1920));
    }
}
