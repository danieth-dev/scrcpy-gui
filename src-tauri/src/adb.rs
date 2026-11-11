use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::time::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::tools::find_adb;

const DEFAULT_TIMEOUT_MS: u64 = 8000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub serial: String,
    pub model: String,
    pub brand: String,
    #[serde(rename = "androidVersion")]
    pub android_version: String,
    #[serde(rename = "batteryLevel")]
    pub battery_level: String,
    #[serde(rename = "isWifi")]
    pub is_wifi: bool,
    pub state: String,
    #[serde(rename = "hardwareSerial")]
    pub hardware_serial: String,
    pub ip: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DevicesResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<DeviceInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

pub fn is_valid_serial(serial: &str) -> bool {
    let s = serial.trim();
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    s.chars().all(|c| c.is_alphanumeric() || c == '.' || c == ':' || c == '-' || c == '_')
}

pub fn is_ip_serial(serial: &str) -> bool {
    let re = Regex::new(r"^\d+\.\d+\.\d+\.\d+(:\d+)?$").unwrap();
    re.is_match(serial)
}

pub fn is_wifi_serial(serial: &str) -> bool {
    if is_ip_serial(serial) {
        return true;
    }
    let re_tls = Regex::new(r"(?i)\._adb-tls-[\w-]+\._tcp").unwrap();
    let re_local = Regex::new(r"(?i)\.local(:\d+)?$").unwrap();
    re_tls.is_match(serial) || re_local.is_match(serial)
}

pub fn is_valid_ip(ip: &str) -> bool {
    let re = Regex::new(r"^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$").unwrap();
    re.is_match(ip)
}

pub fn is_valid_port(port: u32) -> bool {
    port > 0 && port <= 65535
}

/// Ejecuta adb con un timeout y sin ventana de consola en Windows
pub async fn run_adb<I, S>(args: I, timeout_ms: u64) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let adb_path = find_adb().ok_or_else(|| "adb no encontrado en gui/vendor/scrcpy/ ni en PATH".to_string())?;

    let mut cmd = Command::new(&adb_path);
    if let Some(parent) = adb_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000);
    }

    let child = cmd.spawn().map_err(|e| format!("Error al ejecutar adb: {e}"))?;

    let output_future = child.wait_with_output();
    let res = timeout(Duration::from_millis(timeout_ms), output_future)
        .await
        .map_err(|_| format!("Timeout ejecutando adb ({timeout_ms}ms)"))?
        .map_err(|e| format!("Error esperando adb: {e}"))?;

    let stdout = String::from_utf8_lossy(&res.stdout).to_string();
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();
    let combined = format!("{stdout}{stderr}").trim().to_string();

    if res.status.success() {
        Ok(combined)
    } else {
        Err(if combined.is_empty() {
            format!("adb terminó con código de error {}", res.status.code().unwrap_or(-1))
        } else {
            combined
        })
    }
}

pub async fn adb_quiet<I, S>(args: I, timeout_ms: u64) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    run_adb(args, timeout_ms).await.unwrap_or_default()
}

pub async fn detect_device_ip(serial: &str) -> String {
    // 1. Probar `ip route`
    let route_out = adb_quiet(["-s", serial, "shell", "ip", "route"], 2500).await;
    let re_src = Regex::new(r"src\s+(\d+\.\d+\.\d+\.\d+)").unwrap();
    if let Some(caps) = re_src.captures(&route_out) {
        if let Some(m) = caps.get(1) {
            let ip = m.as_str();
            if ip != "127.0.0.1" {
                return ip.to_string();
            }
        }
    }

    // 2. Probar `ip addr show wlan0`
    let addr_out = adb_quiet(["-s", serial, "shell", "ip", "-f", "inet", "addr", "show", "wlan0"], 2500).await;
    let re_inet = Regex::new(r"inet\s+(\d+\.\d+\.\d+\.\d+)").unwrap();
    if let Some(caps) = re_inet.captures(&addr_out) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_string();
        }
    }

    String::new()
}

pub async fn describe_device(serial: &str, state: &str) -> DeviceInfo {
    let is_wifi = is_wifi_serial(serial);

    let ip_match = if let Some(caps) = Regex::new(r"^(\d+\.\d+\.\d+\.\d+)").unwrap().captures(serial) {
        caps.get(1).map(|m| m.as_str().to_string())
    } else {
        None
    };

    let prop = |name: &'static str| {
        let s = serial.to_string();
        async move { adb_quiet(["-s", &s, "shell", "getprop", name], 2500).await }
    };

    let s_serial = serial.to_string();
    let (model, brand, android_version, hw_serial, battery, detected_ip) = tokio::join!(
        prop("ro.product.model"),
        prop("ro.product.brand"),
        prop("ro.build.version.release"),
        prop("ro.serialno"),
        async { adb_quiet(["-s", &s_serial, "shell", "dumpsys", "battery"], 2500).await },
        async {
            if let Some(ip) = ip_match {
                ip
            } else {
                detect_device_ip(&s_serial).await
            }
        }
    );

    let battery_level = if let Some(caps) = Regex::new(r"level:\s*(\d+)").unwrap().captures(&battery) {
        caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default()
    } else {
        String::new()
    };

    DeviceInfo {
        serial: serial.to_string(),
        model: if model.is_empty() { serial.to_string() } else { model },
        brand,
        android_version,
        battery_level,
        is_wifi,
        state: state.to_string(),
        hardware_serial: hw_serial,
        ip: detected_ip,
    }
}

pub async fn get_devices(active_scrcpy_serials: HashSet<String>) -> DevicesResult {
    let mut output = match run_adb(["devices"], DEFAULT_TIMEOUT_MS).await {
        Ok(out) => out,
        Err(e) => return DevicesResult { ok: false, devices: None, error: Some(e) },
    };

    let parse_lines = |raw: &str| -> (Vec<String>, Vec<String>) {
        let mut online = Vec::new();
        let mut offline_wifi = Vec::new();
        for line in raw.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('*') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let s = parts[0];
            if !is_valid_serial(s) {
                continue;
            }
            let st = parts.get(1).copied().unwrap_or("");
            if st == "device" {
                online.push(s.to_string());
            } else if st == "offline" && is_wifi_serial(s) {
                offline_wifi.push(s.to_string());
            }
        }
        (online, offline_wifi)
    };

    let (mut serials, mut offline_wifi) = parse_lines(&output);

    // Si solo hay dispositivos WiFi offline, intentar reconectarlos
    if serials.is_empty() && !offline_wifi.is_empty() {
        adb_quiet(["reconnect", "offline"], 4000).await;
        if let Ok(new_out) = run_adb(["devices"], DEFAULT_TIMEOUT_MS).await {
            output = new_out;
            let (new_serials, new_offline) = parse_lines(&output);
            serials = new_serials;
            offline_wifi = new_offline;
        }
    }

    // Limpiar entradas WiFi offline obsoletas
    for off in &offline_wifi {
        if is_ip_serial(off) {
            let off_clone = off.clone();
            tokio::spawn(async move {
                adb_quiet(["disconnect", &off_clone], 3000).await;
            });
        }
    }

    // Obtener detalles de todos los dispositivos en paralelo
    let mut futures = Vec::new();
    for s in &serials {
        futures.push(describe_device(s, "device"));
    }
    let raw_devices = futures::future::join_all(futures).await;

    // Deduplicar cuando el mismo teléfono aparece dos veces por WiFi (IP + mDNS)
    let mut wifi_by_hw: HashMap<String, DeviceInfo> = HashMap::new();
    for d in &raw_devices {
        if !d.is_wifi {
            continue;
        }
        let key = if !d.hardware_serial.is_empty() {
            d.hardware_serial.clone()
        } else if !d.ip.is_empty() {
            format!("ip:{}", d.ip)
        } else {
            String::new()
        };
        if key.is_empty() {
            continue;
        }

        match wifi_by_hw.get(&key) {
            None => {
                wifi_by_hw.insert(key, d.clone());
            }
            Some(prev) => {
                let prev_active = active_scrcpy_serials.contains(&prev.serial);
                let curr_active = active_scrcpy_serials.contains(&d.serial);
                if curr_active && !prev_active {
                    wifi_by_hw.insert(key, d.clone());
                } else if !prev_active && is_ip_serial(&d.serial) && !is_ip_serial(&prev.serial) {
                    wifi_by_hw.insert(key, d.clone());
                }
            }
        }
    }

    let devices: Vec<DeviceInfo> = raw_devices
        .into_iter()
        .filter(|d| {
            if !d.is_wifi {
                return true;
            }
            let key = if !d.hardware_serial.is_empty() {
                d.hardware_serial.clone()
            } else if !d.ip.is_empty() {
                format!("ip:{}", d.ip)
            } else {
                String::new()
            };
            if key.is_empty() {
                return true;
            }
            wifi_by_hw.get(&key).map(|best| best.serial == d.serial).unwrap_or(true)
        })
        .collect();

    DevicesResult {
        ok: true,
        devices: Some(devices),
        error: None,
    }
}

pub async fn pair_wifi(host: &str, port: &str, code: &str) -> SimpleResult {
    let h = host.trim();
    let p = port.trim();
    let c = code.trim().replace(' ', "");

    if h.is_empty() || p.is_empty() || c.is_empty() {
        return SimpleResult { ok: false, message: None, error: Some("Faltan IP, puerto o código".into()), serial: None, ip: None };
    }
    if !is_valid_ip(h) {
        return SimpleResult { ok: false, message: None, error: Some("IP no válida".into()), serial: None, ip: None };
    }
    let p_num = match p.parse::<u32>() {
        Ok(n) if is_valid_port(n) => n,
        _ => return SimpleResult { ok: false, message: None, error: Some("Puerto no válido".into()), serial: None, ip: None },
    };
    if c.len() != 6 || !c.chars().all(|ch| ch.is_ascii_digit()) {
        return SimpleResult { ok: false, message: None, error: Some("El código debe tener 6 dígitos".into()), serial: None, ip: None };
    }

    let target = format!("{h}:{p_num}");
    let out = run_adb(["pair", &target, &c], 30000).await;
    let text = match out {
        Ok(t) => t,
        Err(e) => e,
    };

    let ok = text.to_lowercase().contains("successfully paired");
    SimpleResult {
        ok,
        message: Some(if ok { "Vinculado".into() } else { text.clone() }),
        error: if ok { None } else { Some(text) },
        serial: None,
        ip: None,
    }
}

pub async fn connect_wifi(ip: &str, port: &str) -> SimpleResult {
    let i = ip.trim();
    let p = if port.trim().is_empty() { "5555" } else { port.trim() };

    if !is_valid_ip(i) {
        return SimpleResult { ok: false, message: None, error: Some("IP no válida".into()), serial: None, ip: None };
    }
    let p_num = match p.parse::<u32>() {
        Ok(n) if is_valid_port(n) => n,
        _ => return SimpleResult { ok: false, message: None, error: Some("Puerto no válido".into()), serial: None, ip: None },
    };

    // Limpiar entradas offline previas de la misma IP
    let dev_out = adb_quiet(["devices"], 4000).await;
    for line in dev_out.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "offline" && parts[0].starts_with(&format!("{i}:")) {
            let stale = parts[0].to_string();
            adb_quiet(["disconnect", &stale], 3000).await;
        }
    }

    let target = format!("{i}:{p_num}");
    let out = run_adb(["connect", &target], 15000).await;
    let text = match out {
        Ok(t) => t,
        Err(e) => e,
    };

    let ok = text.contains("connected to") || text.contains("already connected");
    SimpleResult {
        ok,
        message: Some(text.clone()),
        error: if ok { None } else { Some(text) },
        serial: Some(target),
        ip: None,
    }
}

pub async fn disconnect(serial: &str) -> SimpleResult {
    if !is_valid_serial(serial) {
        return SimpleResult { ok: false, message: None, error: Some("Serial no válido".into()), serial: None, ip: None };
    }
    match run_adb(["disconnect", serial], DEFAULT_TIMEOUT_MS).await {
        Ok(out) => SimpleResult { ok: true, message: Some(out), error: None, serial: None, ip: None },
        Err(e) => SimpleResult { ok: false, message: None, error: Some(e), serial: None, ip: None },
    }
}

pub async fn enable_tcpip(serial: &str) -> SimpleResult {
    if !is_valid_serial(serial) {
        return SimpleResult { ok: false, message: None, error: Some("Serial no válido".into()), serial: None, ip: None };
    }

    let ip = detect_device_ip(serial).await;
    if let Err(e) = run_adb(["-s", serial, "tcpip", "5555"], DEFAULT_TIMEOUT_MS).await {
        return SimpleResult { ok: false, message: None, error: Some(e), serial: None, ip: None };
    }

    tokio::time::sleep(Duration::from_millis(600)).await;

    let final_ip = if !ip.is_empty() {
        ip
    } else {
        detect_device_ip(serial).await
    };

    SimpleResult {
        ok: true,
        message: Some("Modo TCP/IP 5555 activado".into()),
        error: None,
        serial: None,
        ip: Some(final_ip),
    }
}
