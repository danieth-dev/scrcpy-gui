//! Cámara virtual DirectShow (Unity Capture).
//!
//! Pipeline:
//!   scrcpy-server (Android, raw_stream) --adb forward--> TCP 127.0.0.1:<port>
//!     --> decodificador en proceso (DLL de FFmpeg de scrcpy, H.264/H.265 -> RGBA)
//!     --> memoria compartida "UnityCapture_*"
//!     --> filtro DirectShow "Unity Video Capture" (OBS, Zoom, Teams, Discord…)
//!
//! No se usa el cliente `scrcpy.exe` para transportar el vídeo: su `--record`
//! sólo escribe a ficheros (antepone `file:`) y además imprime logs por stdout,
//! así que no puede alimentar un pipe. Hablamos directamente con el servidor.

use std::ffi::c_void;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::health::HealthMonitor;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{watch, Mutex, Notify};

use crate::profile::{
    self, no_window, run_adb, scrcpy_version, ProfileRequest, ResolvedProfile, Target, CREATE_NO_WINDOW,
    DEVICE_SERVER_PATH,
};
use crate::decoder;

const LOG_CAP: usize = 16 * 1024;

// ── FFI Win32 mínimo para la memoria compartida de Unity Capture ─────────

type HANDLE = *mut c_void;
type BOOL = i32;
type DWORD = u32;

const PAGE_READWRITE: DWORD = 0x04;
const FILE_MAP_WRITE: DWORD = 0x0002;
const WAIT_OBJECT_0: DWORD = 0;
const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
const SYNCHRONIZE: DWORD = 0x0010_0000;
const EVENT_MODIFY_STATE: DWORD = 0x0002;

unsafe extern "system" {
    fn CreateFileMappingA(
        hFile: HANDLE,
        lpFileMappingAttributes: *mut c_void,
        flProtect: DWORD,
        dwMaximumSizeHigh: DWORD,
        dwMaximumSizeLow: DWORD,
        lpName: *const u8,
    ) -> HANDLE;
    fn OpenFileMappingA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: *const u8) -> HANDLE;
    fn MapViewOfFile(
        hFileMappingObject: HANDLE,
        dwDesiredAccess: DWORD,
        dwFileOffsetHigh: DWORD,
        dwFileOffsetLow: DWORD,
        dwNumberOfBytesToMap: usize,
    ) -> *mut c_void;
    fn UnmapViewOfFile(lpBaseAddress: *const c_void) -> BOOL;
    fn CreateMutexA(lpMutexAttributes: *mut c_void, bInitialOwner: BOOL, lpName: *const u8) -> HANDLE;
    fn OpenMutexA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: *const u8) -> HANDLE;
    fn ReleaseMutex(hMutex: HANDLE) -> BOOL;
    fn CreateEventA(
        lpEventAttributes: *mut c_void,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: *const u8,
    ) -> HANDLE;
    fn OpenEventA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: *const u8) -> HANDLE;
    fn SetEvent(hEvent: HANDLE) -> BOOL;
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    fn CloseHandle(hObject: HANDLE) -> BOOL;
}

/// Debe coincidir con `SharedMemHeader` de UnityCapture/Source/shared.inl.
#[repr(C)]
struct SharedMemHeader {
    max_size: u32,
    width: i32,
    height: i32,
    /// En PÍXELES (el plugin de Unity pasa `RowPitch / 4` para RGBA8).
    stride: i32,
    format: i32,
    resizemode: i32,
    mirrormode: i32,
    timeout: i32,
}

pub struct UnityCaptureSender {
    h_mutex: HANDLE,
    h_want_event: HANDLE,
    h_sent_event: HANDLE,
    h_shared_file: HANDLE,
    p_header: *mut SharedMemHeader,
    p_data: *mut u8,
    max_payload_size: usize,
}

// Los punteros sólo se usan desde la tarea que posee el sender.
unsafe impl Send for UnityCaptureSender {}

impl UnityCaptureSender {
    pub fn new() -> Result<Self, String> {
        // MAX_SHARED_IMAGE_SIZE de Unity Capture: 4K RGBA de 16 bits por canal.
        let max_img_size: usize = 3840 * 2160 * 4 * 2;
        let total_size = std::mem::size_of::<SharedMemHeader>() + max_img_size;

        // Capture número 0 → nombres sin sufijo (compatibilidad con el filtro).
        let name_mutx = b"UnityCapture_Mutx\0";
        let name_want = b"UnityCapture_Want\0";
        let name_sent = b"UnityCapture_Sent\0";
        let name_data = b"UnityCapture_Data\0";

        unsafe {
            let mut h_mutex = CreateMutexA(std::ptr::null_mut(), 0, name_mutx.as_ptr());
            if h_mutex.is_null() {
                h_mutex = OpenMutexA(SYNCHRONIZE, 0, name_mutx.as_ptr());
            }
            if h_mutex.is_null() {
                return Err("No se pudo crear/abrir el Mutex UnityCapture_Mutx".into());
            }

            let mut h_want = CreateEventA(std::ptr::null_mut(), 0, 0, name_want.as_ptr());
            if h_want.is_null() {
                h_want = OpenEventA(SYNCHRONIZE | EVENT_MODIFY_STATE, 0, name_want.as_ptr());
            }

            let mut h_sent = CreateEventA(std::ptr::null_mut(), 0, 0, name_sent.as_ptr());
            if h_sent.is_null() {
                h_sent = OpenEventA(SYNCHRONIZE | EVENT_MODIFY_STATE, 0, name_sent.as_ptr());
            }

            let mut h_shared_file = CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                0,
                total_size as DWORD,
                name_data.as_ptr(),
            );
            if h_shared_file.is_null() {
                h_shared_file = OpenFileMappingA(FILE_MAP_WRITE, 0, name_data.as_ptr());
            }
            let close_events = || {
                if !h_want.is_null() {
                    CloseHandle(h_want);
                }
                if !h_sent.is_null() {
                    CloseHandle(h_sent);
                }
            };
            if h_shared_file.is_null() {
                CloseHandle(h_mutex);
                close_events();
                return Err("No se pudo crear/abrir la memoria compartida UnityCapture_Data".into());
            }

            let p_view = MapViewOfFile(h_shared_file, FILE_MAP_WRITE, 0, 0, 0);
            if p_view.is_null() {
                CloseHandle(h_mutex);
                close_events();
                CloseHandle(h_shared_file);
                return Err("No se pudo mapear la memoria compartida".into());
            }

            let p_header = p_view as *mut SharedMemHeader;
            (*p_header).max_size = max_img_size as u32;
            let p_data = (p_view as *mut u8).add(std::mem::size_of::<SharedMemHeader>());

            Ok(Self {
                h_mutex,
                h_want_event: h_want,
                h_sent_event: h_sent,
                h_shared_file,
                p_header,
                p_data,
                max_payload_size: max_img_size,
            })
        }
    }

    /// Envía un frame RGBA8 compacto (stride = width). Devuelve false si se descartó.
    pub fn send_frame(&mut self, width: i32, height: i32, frame_bytes: &[u8]) -> bool {
        if frame_bytes.len() > self.max_payload_size {
            return false;
        }

        unsafe {
            if WaitForSingleObject(self.h_mutex, 30) != WAIT_OBJECT_0 {
                return false;
            }

            (*self.p_header).width = width;
            (*self.p_header).height = height;
            (*self.p_header).stride = width; // píxeles, no bytes
            (*self.p_header).format = 0; // FORMAT_UINT8 (RGBA 8 bits)
            (*self.p_header).resizemode = 1; // RESIZEMODE_LINEAR
            (*self.p_header).mirrormode = 0; // MIRRORMODE_DISABLED
            (*self.p_header).timeout = 1000;

            std::ptr::copy_nonoverlapping(frame_bytes.as_ptr(), self.p_data, frame_bytes.len());

            ReleaseMutex(self.h_mutex);

            if !self.h_sent_event.is_null() {
                SetEvent(self.h_sent_event);
            }
        }

        true
    }
}

impl Drop for UnityCaptureSender {
    fn drop(&mut self) {
        unsafe {
            if !self.p_header.is_null() {
                UnmapViewOfFile(self.p_header as *const c_void);
            }
            for h in [self.h_shared_file, self.h_sent_event, self.h_want_event, self.h_mutex] {
                if !h.is_null() {
                    CloseHandle(h);
                }
            }
        }
    }
}

// ── Tipos públicos ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct VcamStatus {
    pub running: bool,
    #[serde(rename = "isDriverInstalled")]
    pub is_driver_installed: bool,
    pub serial: Option<String>,
    pub resolution: Option<String>,
    pub fps: Option<u32>,
    pub profile: Option<ResolvedProfile>,
    pub error: Option<String>,
}

struct ActiveVcamSession {
    generation: u64,
    serial: String,
    profile: ResolvedProfile,
    /// Petición original (para relanzar con otro nivel de protección).
    request: ProfileRequest,
    stop: Arc<Notify>,
    /// true = parada interna por reinicio: no avisar a la UI de "detenida".
    quiet: Arc<AtomicBool>,
    /// Pasa a true cuando el supervisor ha liberado cámara, túnel y memoria.
    done: watch::Receiver<bool>,
}

#[derive(Default)]
struct VcamState {
    session: Option<ActiveVcamSession>,
    next_generation: u64,
    last_error: Option<String>,
    /// Reconexión automática en curso (true = cancelada por el usuario).
    reconnecting: Option<Arc<AtomicBool>>,
}

#[derive(Clone, Default)]
pub struct VcamManager {
    state: Arc<Mutex<VcamState>>,
}

// ── Utilidades ────────────────────────────────────────────────────────────

fn no_window_std(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

type SharedLog = Arc<std::sync::Mutex<String>>;

/// Drena un stream de un proceso hijo en un buffer acotado (evita bloqueos del pipe).
fn drain_into<R: AsyncRead + Unpin + Send + 'static>(mut reader: R, log: SharedLog) {
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut l = log.lock().unwrap();
                    l.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if l.len() > LOG_CAP {
                        let cut = l.len() - LOG_CAP;
                        let cut = (cut..l.len()).find(|&i| l.is_char_boundary(i)).unwrap_or(0);
                        l.drain(..cut);
                    }
                }
            }
        }
    });
}

fn log_tail(log: &SharedLog) -> String {
    let l = log.lock().unwrap();
    let lines: Vec<&str> = l.lines().filter(|s| !s.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(6);
    lines[start..].join("\n")
}

/// Extrae la línea de error más útil del log del servidor scrcpy.
fn server_error(log: &SharedLog) -> String {
    let found = {
        let l = log.lock().unwrap();
        l.lines()
            .find(|s| s.contains("ERROR") || s.contains("Exception"))
            .map(|s| s.trim().to_string())
    };
    found.unwrap_or_else(|| log_tail(log))
}

fn free_local_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("Sin puertos libres: {e}"))?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

fn random_scid() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mixed = nanos ^ (std::process::id() as u64).rotate_left(32);
    (mixed as u32 ^ (mixed >> 32) as u32) & 0x7FFF_FFFF
}

/// Con `adb forward`, adb acepta la conexión TCP aunque el servidor aún no
/// escuche (y la cierra al instante). Pedimos `send_dummy_byte=true` para
/// distinguir una conexión real: el servidor escribe 1 byte al aceptar.
async fn connect_tunnel(port: u16, server: &mut Child, log: &SharedLog) -> Result<TcpStream, String> {
    let start = Instant::now();
    loop {
        if let Ok(Some(status)) = server.try_wait() {
            return Err(format!(
                "El servidor scrcpy terminó ({status}) antes de conectar: {}",
                server_error(log)
            ));
        }
        if start.elapsed() > Duration::from_secs(12) {
            return Err(format!("Tiempo de espera agotado conectando con el teléfono. {}", log_tail(log)));
        }
        if let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)).await {
            let mut b = [0u8; 1];
            if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(3), s.read_exact(&mut b)).await {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

// ── Reconexión automática ─────────────────────────────────────────────────

/// Sólo se reconecta si la sesión llevaba funcionando un rato: un fallo al
/// arrancar no se arregla reintentando.
const RECONNECT_MIN_UPTIME: Duration = Duration::from_secs(5);
const RECONNECT_ATTEMPTS: u32 = 3;
const RECONNECT_WAIT_DEVICE: Duration = Duration::from_secs(20);

/// Función normal (no async) para cortar el ciclo de tipos
/// start -> supervisor -> reconexión -> start.
fn schedule_reconnect(
    state: Arc<Mutex<VcamState>>,
    serial: String,
    request: ProfileRequest,
    app: AppHandle,
    first_error: String,
) {
    tokio::spawn(reconnect_task(state, serial, request, app, first_error));
}

async fn reconnect_task(
    state: Arc<Mutex<VcamState>>,
    serial: String,
    request: ProfileRequest,
    app: AppHandle,
    first_error: String,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    state.lock().await.reconnecting = Some(cancel.clone());
    let mgr = VcamManager { state: state.clone() };
    let tools = profile::tools();
    let mut last = first_error;

    let cancelled = |app: &AppHandle| {
        let _ = app.emit("vcam:stopped", serde_json::json!({ "serial": serial, "error": null }));
    };

    for attempt in 1..=RECONNECT_ATTEMPTS {
        let _ = app.emit(
            "vcam:reconnecting",
            serde_json::json!({ "serial": serial, "attempt": attempt, "of": RECONNECT_ATTEMPTS, "reason": last }),
        );
        let online = match &tools {
            Ok(t) => profile::wait_for_device(&t.adb, &serial, RECONNECT_WAIT_DEVICE).await,
            Err(_) => false,
        };
        if cancel.load(Ordering::SeqCst) {
            cancelled(&app);
            return;
        }
        if online {
            // Dar un respiro a adb/cámara tras volver.
            tokio::time::sleep(Duration::from_secs(1)).await;
            match mgr.start(serial.clone(), request.clone(), app.clone()).await {
                Ok(p) => {
                    {
                        let mut st = state.lock().await;
                        if st.reconnecting.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
                            st.reconnecting = None;
                        }
                    }
                    if cancel.load(Ordering::SeqCst) {
                        let _ = mgr.stop().await;
                        cancelled(&app);
                        return;
                    }
                    let _ = app.emit("vcam:reconnected", serde_json::json!({ "serial": serial, "profile": p }));
                    let level = request.throttle.unwrap_or(0);
                    app.state::<HealthMonitor>().ensure(serial.clone(), level, app.clone()).await;
                    return;
                }
                Err(e) => last = e,
            }
        } else {
            last = "El teléfono no volvió a conectarse".into();
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        if cancel.load(Ordering::SeqCst) {
            cancelled(&app);
            return;
        }
    }

    let msg = format!("Se perdió la conexión y no se pudo recuperar: {last}");
    {
        let mut st = state.lock().await;
        if st.reconnecting.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cancel)) {
            st.reconnecting = None;
        }
        st.last_error = Some(msg.clone());
    }
    let _ = app.emit("vcam:stopped", serde_json::json!({ "serial": serial, "error": msg }));
}

// ── Gestor ────────────────────────────────────────────────────────────────

impl VcamManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_driver_installed() -> bool {
        ["scrcpy Virtual Camera", "Unity Video Capture"].iter().any(|name| {
            no_window_std(std::process::Command::new("reg.exe").args([
                "query",
                r"HKLM\SOFTWARE\Classes\CLSID\{860BB310-5D01-11d0-BD3B-00A0C911CE86}\Instance",
                "/s",
                "/f",
                name,
            ]))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        })
    }

    pub async fn get_status(&self) -> VcamStatus {
        let driver_installed = tokio::task::spawn_blocking(Self::is_driver_installed)
            .await
            .unwrap_or(false);
        let st = self.state.lock().await;
        match st.session {
            Some(ref sess) => {
                let (w, h) = sess.profile.output_size();
                VcamStatus {
                    running: true,
                    is_driver_installed: driver_installed,
                    serial: Some(sess.serial.clone()),
                    resolution: Some(format!("{w}x{h}")),
                    fps: Some(sess.profile.fps),
                    profile: Some(sess.profile.clone()),
                    error: None,
                }
            }
            None => VcamStatus {
                running: false,
                is_driver_installed: driver_installed,
                serial: None,
                resolution: None,
                fps: None,
                profile: None,
                error: st.last_error.clone(),
            },
        }
    }

    pub async fn start(
        &self,
        serial: String,
        request: ProfileRequest,
        app: AppHandle,
    ) -> Result<ResolvedProfile, String> {
        let mut st = self.state.lock().await;
        if st.session.is_some() {
            return Err("Ya hay una sesión de Cámara Virtual en ejecución".into());
        }
        st.last_error = None;

        let t = profile::tools()?;
        let adb_path = t.adb.clone();
        let scrcpy_dir = t
            .scrcpy
            .parent()
            .map(|d| d.to_path_buf())
            .ok_or("Ruta de scrcpy no válida")?;
        // Decodificador en proceso con las DLL de FFmpeg que trae scrcpy.
        decoder::available(&scrcpy_dir)?;

        // Ajustes de la UI -> configuración que este teléfono soporta (mismo
        // resolvedor que la cámara en ventana). Todo lo que va al comando
        // `adb shell` sale de aquí ya validado: nada de texto libre.
        let prof = profile::resolve_for(&serial, &request, Target::Vcam).await;
        // 1 hilo = mínima latencia (sobra hasta 1080p: ~110 fps). En 1440p/4K,
        // 2 hilos (1 frame más de latencia) para ir holgado: un hilo da ~37 fps
        // en 4K H.265 en un Ryzen 5 5600G.
        let decode_threads = if prof.width.max(prof.height) > 1920 { 2 } else { 1 };

        let version = scrcpy_version(&t.scrcpy).await?;
        let scid = format!("{:08x}", random_scid());
        let port = free_local_port()?;
        let server_local_str = t.server.to_string_lossy().to_string();

        // 1. Subir el servidor y abrir el túnel.
        run_adb(&adb_path, &["-s", &serial, "push", &server_local_str, DEVICE_SERVER_PATH]).await?;
        let fwd_local = format!("tcp:{port}");
        let fwd_remote = format!("localabstract:scrcpy_{scid}");
        run_adb(&adb_path, &["-s", &serial, "forward", &fwd_local, &fwd_remote]).await?;

        let remove_forward = {
            let adb = adb_path.clone();
            let serial = serial.clone();
            let fwd_local = fwd_local.clone();
            move || async move {
                let _ = run_adb(&adb, &["-s", &serial, "forward", "--remove", &fwd_local]).await;
            }
        };

        // 2. Lanzar el servidor con el perfil exacto.
        //    `send_dummy_byte=true` debe ir DESPUÉS de `raw_stream=true`.
        let mut cam_opts = match &prof.camera_id {
            Some(id) => format!("camera_id={id}"),
            None => format!("camera_facing={}", prof.facing),
        };
        if prof.verified {
            cam_opts.push_str(&format!(" camera_size={}x{}", prof.width, prof.height));
        } else {
            cam_opts.push_str(&format!(
                " max_size={} camera_ar={}:{}",
                prof.width.max(prof.height),
                prof.width,
                prof.height
            ));
        }
        cam_opts.push_str(&format!(" camera_fps={}", prof.fps));
        if prof.high_speed {
            cam_opts.push_str(" camera_high_speed=true");
        }
        if prof.zoom > 1.0 {
            cam_opts.push_str(&format!(" camera_zoom={}", prof.zoom));
        }
        if prof.torch {
            cam_opts.push_str(" camera_torch=true");
        }
        if prof.orientation != 0 {
            cam_opts.push_str(&format!(" capture_orientation={}", prof.orientation));
        }
        let mut codec_opts = format!(
            "video_codec={} video_bit_rate={}",
            prof.codec,
            prof.bitrate_mbps as u64 * 1_000_000
        );
        if let Some(enc) = &prof.encoder {
            codec_opts.push_str(&format!(" video_encoder={enc}"));
        }
        let server_cmd = format!(
            "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server {version}              scid={scid} log_level=info tunnel_forward=true audio=false control=false cleanup=true              send_device_meta=false send_frame_meta=true send_stream_meta=false send_dummy_byte=true video_source=camera {cam_opts} {codec_opts}              video_codec_options=low-latency=1,max-bframes=0"
        );

        let mut server = match no_window(Command::new(&adb_path).args(["-s", &serial, "shell", &server_cmd]))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                remove_forward().await;
                return Err(format!("Error lanzando el servidor scrcpy: {e}"));
            }
        };
        let server_log: SharedLog = Arc::default();
        if let Some(o) = server.stdout.take() {
            drain_into(o, server_log.clone());
        }
        if let Some(e) = server.stderr.take() {
            drain_into(e, server_log.clone());
        }

        // 3. Conectar al socket de vídeo.
        let socket = match connect_tunnel(port, &mut server, &server_log).await {
            Ok(s) => s,
            Err(e) => {
                let _ = server.kill().await;
                remove_forward().await;
                return Err(e);
            }
        };

        // 4. Decodificador (en proceso).
        let mut dec = match decoder::Decoder::new(&scrcpy_dir, &prof.codec, decode_threads) {
            Ok(d) => d,
            Err(e) => {
                let _ = server.kill().await;
                remove_forward().await;
                return Err(e);
            }
        };
        // Socket bloqueante para el hilo de decodificación; un clon permite
        // cerrarlo desde fuera para detenerlo.
        let (sock, sock_ctl) = match socket
            .into_std()
            .and_then(|s| s.set_nonblocking(false).map(|_| s))
            .and_then(|s| s.try_clone().map(|c| (s, c)))
        {
            Ok(v) => v,
            Err(e) => {
                let _ = server.kill().await;
                remove_forward().await;
                return Err(format!("Error preparando el socket de vídeo: {e}"));
            }
        };

        // 5. Memoria compartida DirectShow.
        let mut sender = match UnityCaptureSender::new() {
            Ok(s) => s,
            Err(e) => {
                let _ = server.kill().await;
                remove_forward().await;
                return Err(e);
            }
        };

        st.next_generation += 1;
        let generation = st.next_generation;
        let stop = Arc::new(Notify::new());
        let quiet = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = watch::channel(false);
        st.session = Some(ActiveVcamSession {
            generation,
            serial: serial.clone(),
            profile: prof.clone(),
            request: request.clone(),
            stop: stop.clone(),
            quiet: quiet.clone(),
            done: done_rx,
        });
        drop(st);

        // 6. Supervisor: bombea datos y limpia todo cuando algo termina.
        let state = self.state.clone();
        let started = Instant::now();
        tokio::spawn(async move {
            // Hilo dedicado: socket -> decodificar -> RGBA -> memoria Unity.
            let app_t = app.clone();
            let serial_t = serial.clone();
            let mut worker = tokio::task::spawn_blocking(move || {
                let mut sent = 0u64;
                let mut decoded = 0u64;
                let mut window_start = Instant::now();
                let res = decoder::run_stream(sock, &mut dec, |w, h, rgba| {
                    if sender.send_frame(w as i32, h as i32, rgba) {
                        sent += 1;
                    }
                    decoded += 1;
                    let elapsed = window_start.elapsed();
                    if elapsed >= Duration::from_secs(2) {
                        let secs = elapsed.as_secs_f64();
                        let _ = app_t.emit(
                            "vcam:stats",
                            serde_json::json!({
                                "serial": serial_t,
                                "fps": (sent as f64 / secs).round() as u64,
                                "decodedFps": (decoded as f64 / secs).round() as u64,
                                "dropped": decoded - sent,
                                "width": w,
                                "height": h
                            }),
                        );
                        decoded = 0;
                        sent = 0;
                        window_start = Instant::now();
                    }
                });
                // Liberar la memoria compartida aquí (un reinicio la recrea).
                drop(sender);
                drop(dec);
                res
            });

            let (error, worker_done): (Option<String>, bool) = tokio::select! {
                _ = stop.notified() => (None, false),
                r = &mut worker => (Some(match r {
                    Ok(Ok(_)) => "El vídeo terminó".to_string(),
                    Ok(Err(e)) => format!("{e}. {}", server_error(&server_log)),
                    Err(e) => format!("Fallo interno del decodificador: {e}"),
                }), true),
                s = server.wait() => (Some(format!(
                    "El servidor scrcpy terminó ({}). {}",
                    s.map(|s| s.to_string()).unwrap_or_default(),
                    server_error(&server_log)
                )), false),
            };

            // Desbloquear el hilo y esperar a que suelte la memoria compartida.
            let _ = sock_ctl.shutdown(std::net::Shutdown::Both);
            if !worker_done {
                let _ = tokio::time::timeout(Duration::from_secs(3), &mut worker).await;
            }
            let _ = server.kill().await;
            remove_forward().await;

            {
                let mut st = state.lock().await;
                if st.session.as_ref().map(|s| s.generation) == Some(generation) {
                    st.session = None;
                }
                st.last_error = error.clone();
            }
            let _ = done_tx.send(true);

            if !quiet.load(Ordering::SeqCst) {
                // Corte inesperado tras funcionar un rato (cable, cargador,
                // Wi-Fi): reconectar solo en vez de dejar la cámara caída.
                match error {
                    Some(err)
                        if started.elapsed() > RECONNECT_MIN_UPTIME
                            && !err.contains("Camera disconnected") =>
                    {
                        schedule_reconnect(state, serial, request, app, err);
                    }
                    _ => {
                        let _ = app.emit(
                            "vcam:stopped",
                            serde_json::json!({ "serial": serial, "error": error }),
                        );
                    }
                }
            }
        });

        Ok(prof)
    }

    /// Petición de la sesión activa en ese teléfono, si la hay.
    pub async fn session_request(&self, serial: &str) -> Option<ProfileRequest> {
        let st = self.state.lock().await;
        st.session.as_ref().filter(|s| s.serial == serial).map(|s| s.request.clone())
    }

    /// Relanza la sesión de ese teléfono con otro nivel de protección, sin que
    /// la UI vea una parada. Espera a que la anterior libere la cámara.
    pub async fn restart(&self, serial: &str, throttle: u8, app: AppHandle) -> Result<ResolvedProfile, String> {
        let (mut request, mut done) = {
            let mut st = self.state.lock().await;
            let Some(sess) = st.session.take_if(|s| s.serial == serial) else {
                return Err("No hay cámara virtual activa en ese teléfono".into());
            };
            sess.quiet.store(true, Ordering::SeqCst);
            sess.stop.notify_one();
            (sess.request, sess.done)
        };
        let _ = tokio::time::timeout(Duration::from_secs(6), done.wait_for(|d| *d)).await;
        request.throttle = Some(throttle);
        match self.start(serial.to_string(), request, app.clone()).await {
            Ok(p) => Ok(p),
            Err(e) => {
                let _ = app.emit("vcam:stopped", serde_json::json!({ "serial": serial, "error": e }));
                Err(e)
            }
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut st = self.state.lock().await;
        if let Some(cancel) = st.reconnecting.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        if let Some(sess) = st.session.take() {
            // notify_one guarda el permiso aunque el supervisor aún no esté esperando.
            sess.stop.notify_one();
        }
        Ok(())
    }

    pub fn trigger_driver_install() -> Result<(), String> {
        let mut bat_path = None;
        let mut dll_path = None;
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for ancestor in dir.ancestors().take(6) {
                    let b = ancestor.join("Instalar_Camara_Virtual.bat");
                    let c1 = ancestor.join("vendor").join("UnityCaptureFilter64.dll");
                    let c2 = ancestor.join("gui").join("vendor").join("UnityCaptureFilter64.dll");
                    if bat_path.is_none() && b.is_file() {
                        bat_path = Some(b);
                    }
                    if dll_path.is_none() && c1.is_file() {
                        dll_path = Some(c1);
                    }
                    if dll_path.is_none() && c2.is_file() {
                        dll_path = Some(c2);
                    }
                }
            }
        }

        // Las rutas se pasan como argumentos de PowerShell con comillas simples escapadas.
        let ps_quote = |s: &str| format!("'{}'", s.replace('\'', "''"));

        if let Some(bat) = bat_path {
            let bat_str = bat.to_string_lossy().to_string();
            no_window_std(std::process::Command::new("powershell.exe").args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!("Start-Process -FilePath {} -Verb RunAs", ps_quote(&bat_str)),
            ]))
            .spawn()
            .map_err(|e| format!("Error solicitando elevación de Administrador: {e}"))?;
            return Ok(());
        }

        let dll = dll_path.ok_or_else(|| "No se encontró UnityCaptureFilter64.dll".to_string())?;
        let dll_str = dll.to_string_lossy().to_string();

        no_window_std(std::process::Command::new("powershell.exe").args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "Start-Process regsvr32.exe -ArgumentList {} -Verb RunAs",
                ps_quote(&format!("\"{dll_str}\""))
            ),
        ]))
        .spawn()
        .map_err(|e| format!("Error solicitando elevación de Administrador: {e}"))?;

        Ok(())
    }
}
