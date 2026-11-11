//! Protección del dispositivo: temperatura y batería.
//!
//! Mientras haya una sesión activa en un teléfono, se lee cada 5 s su estado
//! térmico (Android `thermalservice`, 0-6) y la batería. Si se calienta, las
//! sesiones de cámara se reinician con menos calidad (el resolvedor de perfil
//! aplica el nivel de protección); si llega a crítico, se detienen. Cuando se
//! enfría de forma sostenida, se recupera la calidad paso a paso.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use crate::profile::{self, run_adb};
use crate::scrcpy::ScrcpyManager;
use crate::vcam::VcamManager;

const POLL: Duration = Duration::from_secs(5);
/// Tiempo enfriado (de forma continua) antes de recuperar un nivel.
const COOL_HOLD: Duration = Duration::from_secs(60);
/// Separación mínima entre dos reinicios por cambio de nivel.
const MIN_GAP: Duration = Duration::from_secs(20);

pub const MAX_LEVEL: u8 = 3;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Health {
    /// Android PowerManager.THERMAL_STATUS_*: 0 nada, 1 leve, 2 moderado,
    /// 3 grave, 4 crítico, 5 emergencia, 6 apagado.
    #[serde(rename = "thermalStatus")]
    pub thermal_status: Option<u8>,
    #[serde(rename = "batteryTemp")]
    pub battery_temp_c: Option<f32>,
    #[serde(rename = "batteryLevel")]
    pub battery_level: Option<u8>,
    pub charging: bool,
}

pub fn parse(out: &str) -> Health {
    let mut h = Health::default();
    let mut powered = false;
    let mut status_charging = false;
    for line in out.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Thermal Status:") {
            h.thermal_status = h.thermal_status.or_else(|| v.trim().parse().ok());
        } else if let Some(v) = l.strip_prefix("level:") {
            h.battery_level = h.battery_level.or_else(|| v.trim().parse().ok());
        } else if let Some(v) = l.strip_prefix("temperature:") {
            // décimas de grado
            h.battery_temp_c = h
                .battery_temp_c
                .or_else(|| v.trim().parse::<f32>().ok().map(|t| t / 10.0));
        } else if let Some(v) = l.strip_prefix("status:") {
            // BatteryManager: 2 cargando, 5 llena
            status_charging = matches!(v.trim(), "2" | "5");
        } else if l.ends_with("powered: true") {
            powered = true;
        }
    }
    h.charging = powered || status_charging;
    h
}

pub async fn read(adb: &Path, serial: &str) -> Result<Health, String> {
    let cmd = "dumpsys thermalservice | grep -m1 'Thermal Status'; \
               dumpsys battery | grep -E '^  (level|temperature|status|AC powered|USB powered|Wireless powered): '";
    let out = tokio::time::timeout(Duration::from_secs(4), run_adb(adb, &["-s", serial, "shell", cmd]))
        .await
        .map_err(|_| "Tiempo agotado leyendo el estado del teléfono".to_string())??;
    let h = parse(&out);
    if h.thermal_status.is_none() && h.battery_temp_c.is_none() {
        return Err("El teléfono no informó temperatura".into());
    }
    Ok(h)
}

fn level_with(h: &Health, margin: f32) -> u8 {
    let ts = h.thermal_status.unwrap_or(0);
    let t = h.battery_temp_c.unwrap_or(0.0);
    let mut l = 0;
    if ts >= 2 || t >= 41.0 - margin {
        l = 1;
    }
    if ts >= 3 || t >= 44.0 - margin {
        l = 2;
    }
    if ts >= 4 || t >= 47.0 - margin {
        l = 3;
    }
    // Batería casi vacía y sin cargar: ahorrar energía.
    if !h.charging && h.battery_level.is_some_and(|b| b <= 5) {
        l = l.max(2);
    }
    l
}

/// Nivel de protección que exige el estado actual (0-3).
pub fn required_level(h: &Health) -> u8 {
    level_with(h, 0.0)
}

/// Explicación breve para el usuario.
pub fn reason(h: &Health) -> String {
    let mut parts = Vec::new();
    if let Some(t) = h.battery_temp_c {
        parts.push(format!("{t:.1} °C"));
    }
    if let Some(s) = h.thermal_status.filter(|s| *s >= 2) {
        let name = match s {
            2 => "moderado",
            3 => "grave",
            4 => "crítico",
            5 => "emergencia",
            _ => "apagado inminente",
        };
        parts.push(format!("estado térmico {name}"));
    }
    if !h.charging {
        if let Some(b) = h.battery_level.filter(|b| *b <= 15) {
            parts.push(format!("batería {b} %"));
        }
    }
    parts.join(", ")
}

/// Control con histéresis: sube de inmediato, baja de uno en uno tras
/// enfriarse de forma sostenida.
#[derive(Debug, Clone)]
pub struct Controller {
    pub level: u8,
    cool_since: Option<Instant>,
    last_change: Option<Instant>,
}

impl Controller {
    pub fn new(level: u8) -> Self {
        Self { level, cool_since: None, last_change: None }
    }

    /// Devuelve el nuevo nivel si debe cambiar.
    pub fn step(&mut self, h: &Health, now: Instant) -> Option<u8> {
        let required = required_level(h);
        let gap_ok = self.last_change.is_none_or(|t| now.duration_since(t) >= MIN_GAP);

        if required > self.level {
            self.cool_since = None;
            // Crítico: actuar ya, sin esperar el intervalo mínimo.
            if gap_ok || required >= MAX_LEVEL {
                self.level = required;
                self.last_change = Some(now);
                return Some(required);
            }
            return None;
        }

        // ¿Suficientemente frío para bajar un nivel? (2 °C de margen)
        if self.level > 0 && level_with(h, 2.0) < self.level {
            let since = *self.cool_since.get_or_insert(now);
            if now.duration_since(since) >= COOL_HOLD && gap_ok {
                self.level -= 1;
                self.cool_since = None;
                self.last_change = Some(now);
                return Some(self.level);
            }
        } else {
            self.cool_since = None;
        }
        None
    }
}

// ── Monitor ───────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct HealthMonitor {
    active: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, Clone, Serialize)]
struct HealthEvent<'a> {
    serial: &'a str,
    #[serde(flatten)]
    health: &'a Health,
    level: u8,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lectura puntual (para decidir el nivel inicial antes de arrancar).
    pub async fn check(serial: &str) -> Option<Health> {
        let t = profile::tools().ok()?;
        read(&t.adb, serial).await.ok()
    }

    /// Arranca la vigilancia de un teléfono si no estaba ya activa. Termina
    /// sola cuando no quedan sesiones en ese teléfono.
    pub async fn ensure(&self, serial: String, initial_level: u8, app: AppHandle) {
        {
            let mut active = self.active.lock().await;
            if !active.insert(serial.clone()) {
                return;
            }
        }
        let active = self.active.clone();
        tokio::spawn(async move {
            monitor(serial.clone(), initial_level, app).await;
            active.lock().await.remove(&serial);
        });
    }
}

async fn monitor(serial: String, initial_level: u8, app: AppHandle) {
    let Ok(t) = profile::tools() else { return };
    let scrcpy = app.state::<ScrcpyManager>().inner().clone();
    let vcam = app.state::<VcamManager>().inner().clone();
    let mut ctl = Controller::new(initial_level);
    let mut warned_battery: Option<u8> = None;
    let mut failures = 0u32;

    loop {
        tokio::time::sleep(POLL).await;

        let window = scrcpy.session_options(&serial).await;
        let vcam_req = vcam.session_request(&serial).await;
        if window.is_none() && vcam_req.is_none() {
            let _ = app.emit("device:health", serde_json::json!({ "serial": serial, "active": false }));
            return;
        }

        let h = match read(&t.adb, &serial).await {
            Ok(h) => {
                failures = 0;
                h
            }
            Err(_) => {
                failures += 1;
                if failures >= 3 {
                    let _ = app.emit("device:health", serde_json::json!({ "serial": serial, "active": false }));
                }
                continue;
            }
        };

        let _ = app.emit("device:health", HealthEvent { serial: &serial, health: &h, level: ctl.level });

        // Avisos de batería (una vez por umbral, sólo descargando).
        if !h.charging {
            if let Some(b) = h.battery_level {
                let threshold = [5u8, 10, 15].into_iter().find(|th| b <= *th);
                if let Some(th) = threshold {
                    if warned_battery.is_none_or(|w| th < w) {
                        warned_battery = Some(th);
                        let _ = app.emit(
                            "device:alert",
                            serde_json::json!({
                                "serial": serial,
                                "kind": "battery",
                                "message": format!("Batería del teléfono al {b} %: conéctalo a un cargador"),
                            }),
                        );
                    }
                }
            }
        } else {
            warned_battery = None;
        }

        let Some(new_level) = ctl.step(&h, Instant::now()) else { continue };
        let why = reason(&h);
        let is_camera = window.as_ref().is_some_and(|o| o.camera_only.unwrap_or(false));

        if new_level >= MAX_LEVEL {
            if is_camera {
                scrcpy.stop(&serial).await;
            }
            if vcam_req.is_some() {
                let _ = vcam.stop().await;
            }
            let _ = app.emit(
                "device:throttle",
                serde_json::json!({
                    "serial": serial,
                    "level": new_level,
                    "action": if is_camera || vcam_req.is_some() { "stopped" } else { "warn" },
                    "reason": why,
                }),
            );
            if is_camera || vcam_req.is_some() {
                return;
            }
            continue;
        }

        // Reiniciar las sesiones de cámara con el nuevo nivel.
        let mut window_profile = None;
        if let (true, Some(mut opts)) = (is_camera, window.clone()) {
            opts.throttle = Some(new_level);
            let res = scrcpy.launch(app.clone(), serial.clone(), opts).await;
            window_profile = res.profile;
        }
        let mut vcam_profile = None;
        if vcam_req.is_some() {
            vcam_profile = vcam.restart(&serial, new_level, app.clone()).await.ok();
        }

        let _ = app.emit(
            "device:throttle",
            serde_json::json!({
                "serial": serial,
                "level": new_level,
                "action": if is_camera || vcam_req.is_some() { "restarted" } else { "warn" },
                "reason": why,
                "windowProfile": window_profile,
                "vcamProfile": vcam_profile,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Thermal Status: 0
  AC powered: false
  USB powered: true
  Wireless powered: false
  status: 2
  level: 16
  temperature: 356
";

    fn h(ts: u8, t: f32, lvl: u8, charging: bool) -> Health {
        Health { thermal_status: Some(ts), battery_temp_c: Some(t), battery_level: Some(lvl), charging }
    }

    #[test]
    fn parses_real_output() {
        let p = parse(SAMPLE);
        assert_eq!(p, h(0, 35.6, 16, true));
    }

    #[test]
    fn levels() {
        assert_eq!(required_level(&h(0, 36.0, 80, true)), 0);
        assert_eq!(required_level(&h(2, 36.0, 80, true)), 1);
        assert_eq!(required_level(&h(0, 44.5, 80, true)), 2);
        assert_eq!(required_level(&h(4, 36.0, 80, true)), 3);
        assert_eq!(required_level(&h(0, 30.0, 4, false)), 2);
        assert_eq!(required_level(&h(0, 30.0, 4, true)), 0);
    }

    #[test]
    fn hysteresis() {
        let t0 = Instant::now();
        let mut c = Controller::new(0);
        assert_eq!(c.step(&h(0, 42.0, 80, true), t0), Some(1));
        // sigue caliente: nada
        assert_eq!(c.step(&h(0, 41.5, 80, true), t0 + Duration::from_secs(30)), None);
        // 40 °C: por debajo de 41 pero no del margen (39): no baja
        assert_eq!(c.step(&h(0, 40.0, 80, true), t0 + Duration::from_secs(40)), None);
        // frío (38 °C) pero sin esperar 60 s
        assert_eq!(c.step(&h(0, 38.0, 80, true), t0 + Duration::from_secs(50)), None);
        assert_eq!(c.step(&h(0, 38.0, 80, true), t0 + Duration::from_secs(80)), None);
        // 60 s enfriado: baja
        assert_eq!(c.step(&h(0, 38.0, 80, true), t0 + Duration::from_secs(111)), Some(0));
    }

    #[test]
    fn critical_acts_immediately() {
        let t0 = Instant::now();
        let mut c = Controller::new(0);
        assert_eq!(c.step(&h(2, 36.0, 80, true), t0), Some(1));
        // dentro del intervalo mínimo, pero crítico: actúa
        assert_eq!(c.step(&h(4, 36.0, 80, true), t0 + Duration::from_secs(5)), Some(3));
    }
}
