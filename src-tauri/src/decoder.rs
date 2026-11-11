//! Decodificador de vídeo en proceso para la cámara virtual.
//!
//! Usa las librerías de FFmpeg que ya trae el release oficial de scrcpy
//! (`avcodec-*.dll`, `avutil-*.dll`), cargadas en tiempo de ejecución: no hace
//! falta `ffmpeg.exe` (≈220 MB) ni copiar cada frame por tuberías entre
//! procesos. La conversión YUV → RGBA se hace en Rust con SIMD y varios hilos.
//!
//! Protocolo (servidor scrcpy con `send_frame_meta=true`): cada paquete va
//! precedido de 12 bytes: `pts_y_flags: u64 BE` + `tamaño: u32 BE`.
//!   bit 62 = paquete de configuración (SPS/PPS/VPS): se antepone al siguiente.

use std::ffi::{c_char, c_int, c_void, CString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
use yuvutils_rs::{yuv420_to_rgba, YuvPlanarImage, YuvRange, YuvStandardMatrix};

const PACKET_FLAG_CONFIG: u64 = 1 << 62;
const AV_INPUT_BUFFER_PADDING_SIZE: usize = 64;
const MAX_PACKET: usize = 64 * 1024 * 1024;

const AV_CODEC_ID_H264: c_int = 27;
const AV_CODEC_ID_HEVC: c_int = 173;
const AV_PIX_FMT_YUV420P: c_int = 0;
const AV_PIX_FMT_YUVJ420P: c_int = 12;
const AVERROR_EAGAIN: c_int = -11;

// Disposición pública y estable de AVFrame (los primeros campos no cambian
// entre versiones mayores): data[8], linesize[8], extended_data, width, height,
// nb_samples, format.
const FRAME_DATA: usize = 0;
const FRAME_LINESIZE: usize = 64;
const FRAME_WIDTH: usize = 104;
const FRAME_HEIGHT: usize = 108;
const FRAME_FORMAT: usize = 116;

type Ptr = *mut c_void;

struct Api {
    _avutil: Library,
    _avcodec: Library,
    find_decoder: unsafe extern "C" fn(c_int) -> Ptr,
    alloc_context3: unsafe extern "C" fn(Ptr) -> Ptr,
    open2: unsafe extern "C" fn(Ptr, Ptr, *mut Ptr) -> c_int,
    send_packet: unsafe extern "C" fn(Ptr, Ptr) -> c_int,
    receive_frame: unsafe extern "C" fn(Ptr, Ptr) -> c_int,
    free_context: unsafe extern "C" fn(*mut Ptr),
    packet_alloc: unsafe extern "C" fn() -> Ptr,
    packet_free: unsafe extern "C" fn(*mut Ptr),
    packet_unref: unsafe extern "C" fn(Ptr),
    packet_from_data: unsafe extern "C" fn(Ptr, *mut u8, c_int) -> c_int,
    malloc: unsafe extern "C" fn(usize) -> Ptr,
    free: unsafe extern "C" fn(Ptr),
    frame_alloc: unsafe extern "C" fn() -> Ptr,
    frame_free: unsafe extern "C" fn(*mut Ptr),
    frame_unref: unsafe extern "C" fn(Ptr),
    opt_set: unsafe extern "C" fn(Ptr, *const c_char, *const c_char, c_int) -> c_int,
    opt_set_int: unsafe extern "C" fn(Ptr, *const c_char, i64, c_int) -> c_int,
    opt_get_int: unsafe extern "C" fn(Ptr, *const c_char, c_int, *mut i64) -> c_int,
}

// Las funciones de FFmpeg son reentrantes por contexto.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

fn find_dll(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(".dll"))
        })
        .collect();
    found.sort();
    found.pop()
}

impl Api {
    fn load(dir: &Path) -> Result<Self, String> {
        let avutil_path = find_dll(dir, "avutil-").ok_or("No se encontró avutil-*.dll junto a scrcpy")?;
        let avcodec_path = find_dll(dir, "avcodec-").ok_or("No se encontró avcodec-*.dll junto a scrcpy")?;
        unsafe {
            // LOAD_WITH_ALTERED_SEARCH_PATH: resolver dependencias (avutil,
            // swresample) desde la carpeta de la DLL, no la del .exe.
            let avutil = Library::load_with_flags(&avutil_path, LOAD_WITH_ALTERED_SEARCH_PATH)
                .map_err(|e| format!("No se pudo cargar {}: {e}", avutil_path.display()))?;
            let avcodec = Library::load_with_flags(&avcodec_path, LOAD_WITH_ALTERED_SEARCH_PATH)
                .map_err(|e| format!("No se pudo cargar {}: {e}", avcodec_path.display()))?;
            macro_rules! sym {
                ($lib:expr, $name:literal) => {
                    *$lib
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| format!("Falta {} en FFmpeg: {e}", $name))?
                };
            }
            Ok(Self {
                find_decoder: sym!(avcodec, "avcodec_find_decoder"),
                alloc_context3: sym!(avcodec, "avcodec_alloc_context3"),
                open2: sym!(avcodec, "avcodec_open2"),
                send_packet: sym!(avcodec, "avcodec_send_packet"),
                receive_frame: sym!(avcodec, "avcodec_receive_frame"),
                free_context: sym!(avcodec, "avcodec_free_context"),
                packet_alloc: sym!(avcodec, "av_packet_alloc"),
                packet_free: sym!(avcodec, "av_packet_free"),
                packet_unref: sym!(avcodec, "av_packet_unref"),
                packet_from_data: sym!(avcodec, "av_packet_from_data"),
                malloc: sym!(avutil, "av_malloc"),
                free: sym!(avutil, "av_free"),
                frame_alloc: sym!(avutil, "av_frame_alloc"),
                frame_free: sym!(avutil, "av_frame_free"),
                frame_unref: sym!(avutil, "av_frame_unref"),
                opt_set: sym!(avutil, "av_opt_set"),
                opt_set_int: sym!(avutil, "av_opt_set_int"),
                opt_get_int: sym!(avutil, "av_opt_get_int"),
                _avutil: avutil,
                _avcodec: avcodec,
            })
        }
    }
}

/// Carga (una vez) las librerías de FFmpeg de la carpeta indicada.
fn api(dir: &Path) -> Result<Arc<Api>, String> {
    static API: OnceLock<Result<Arc<Api>, String>> = OnceLock::new();
    API.get_or_init(|| Api::load(dir).map(Arc::new)).clone()
}

/// ¿Están disponibles las librerías? (para avisar antes de arrancar)
pub fn available(scrcpy_dir: &Path) -> Result<(), String> {
    api(scrcpy_dir).map(|_| ())
}

pub struct Decoder {
    api: Arc<Api>,
    ctx: Ptr,
    frame: Ptr,
    pkt: Ptr,
    rgba: Vec<u8>,
    color: Option<(YuvRange, YuvStandardMatrix)>,
}

// El decodificador vive y se usa en un único hilo.
unsafe impl Send for Decoder {}

impl Decoder {
    /// `codec`: "h264" o "h265". `threads`: 1 = mínima latencia.
    pub fn new(scrcpy_dir: &Path, codec: &str, threads: u32) -> Result<Self, String> {
        let api = api(scrcpy_dir)?;
        let id = if codec == "h265" { AV_CODEC_ID_HEVC } else { AV_CODEC_ID_H264 };
        unsafe {
            let dec = (api.find_decoder)(id);
            if dec.is_null() {
                return Err(format!("FFmpeg no tiene decodificador para {codec}"));
            }
            let mut ctx = (api.alloc_context3)(dec);
            if ctx.is_null() {
                return Err("No se pudo crear el contexto del decodificador".into());
            }
            let flags = CString::new("flags").unwrap();
            let low_delay = CString::new("+low_delay").unwrap();
            let threads_k = CString::new("threads").unwrap();
            let ttype = CString::new("thread_type").unwrap();
            let frame_v = CString::new("frame").unwrap();
            (api.opt_set)(ctx, flags.as_ptr(), low_delay.as_ptr(), 0);
            (api.opt_set_int)(ctx, threads_k.as_ptr(), threads.max(1) as i64, 0);
            (api.opt_set)(ctx, ttype.as_ptr(), frame_v.as_ptr(), 0);
            if (api.open2)(ctx, dec, std::ptr::null_mut()) < 0 {
                (api.free_context)(&mut ctx);
                return Err("No se pudo abrir el decodificador".into());
            }
            let frame = (api.frame_alloc)();
            let pkt = (api.packet_alloc)();
            if frame.is_null() || pkt.is_null() {
                let mut f = frame;
                let mut p = pkt;
                (api.frame_free)(&mut f);
                (api.packet_free)(&mut p);
                (api.free_context)(&mut ctx);
                return Err("Memoria insuficiente para el decodificador".into());
            }
            Ok(Self { api, ctx, frame, pkt, rgba: Vec::new(), color: None })
        }
    }

    /// Decodifica un paquete y llama a `on_frame(ancho, alto, rgba)` por cada
    /// frame que salga (normalmente 0 o 1).
    pub fn decode(&mut self, data: &[u8], mut on_frame: impl FnMut(u32, u32, &[u8])) -> Result<(), String> {
        let a = self.api.clone();
        unsafe {
            // FFmpeg exige un buffer propio con relleno a ceros al final.
            let buf = (a.malloc)(data.len() + AV_INPUT_BUFFER_PADDING_SIZE) as *mut u8;
            if buf.is_null() {
                return Err("Memoria insuficiente".into());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len());
            std::ptr::write_bytes(buf.add(data.len()), 0, AV_INPUT_BUFFER_PADDING_SIZE);
            if (a.packet_from_data)(self.pkt, buf, data.len() as c_int) < 0 {
                (a.free)(buf as Ptr);
                return Err("No se pudo preparar el paquete".into());
            }
            let r = (a.send_packet)(self.ctx, self.pkt);
            (a.packet_unref)(self.pkt);
            if r < 0 && r != AVERROR_EAGAIN {
                // Paquete corrupto: se descarta y se sigue (el siguiente
                // keyframe recupera la imagen).
                return Ok(());
            }
            loop {
                let r = (a.receive_frame)(self.ctx, self.frame);
                if r < 0 {
                    break; // EAGAIN (necesita más datos) o EOF
                }
                let res = self.convert_current();
                (a.frame_unref)(self.frame);
                let (w, h) = res?;
                on_frame(w, h, &self.rgba);
            }
        }
        Ok(())
    }

    unsafe fn convert_current(&mut self) -> Result<(u32, u32), String> {
        unsafe {
            let base = self.frame as *const u8;
            let rd_i32 = |off: usize| *(base.add(off) as *const i32);
            let rd_ptr = |i: usize| *(base.add(FRAME_DATA + i * 8) as *const *const u8);
            let w = rd_i32(FRAME_WIDTH);
            let h = rd_i32(FRAME_HEIGHT);
            let fmt = rd_i32(FRAME_FORMAT);
            if w <= 0 || h <= 0 {
                return Err("Frame sin dimensiones".into());
            }
            if fmt != AV_PIX_FMT_YUV420P && fmt != AV_PIX_FMT_YUVJ420P {
                return Err(format!("Formato de píxel no soportado ({fmt})"));
            }
            let (w, h) = (w as u32, h as u32);
            let ls: [i32; 3] = [rd_i32(FRAME_LINESIZE), rd_i32(FRAME_LINESIZE + 4), rd_i32(FRAME_LINESIZE + 8)];
            if ls.iter().any(|l| *l <= 0) {
                return Err("Stride no soportado".into());
            }
            let ch = h.div_ceil(2) as usize;
            let y = std::slice::from_raw_parts(rd_ptr(0), ls[0] as usize * h as usize);
            let u = std::slice::from_raw_parts(rd_ptr(1), ls[1] as usize * ch);
            let v = std::slice::from_raw_parts(rd_ptr(2), ls[2] as usize * ch);

            let (range, matrix) = match self.color {
                Some(c) => c,
                None => {
                    let c = self.color_info(fmt, h);
                    self.color = Some(c);
                    c
                }
            };

            let need = (w * h * 4) as usize;
            if self.rgba.len() != need {
                self.rgba.resize(need, 0);
            }
            let img = YuvPlanarImage {
                y_plane: y,
                y_stride: ls[0] as u32,
                u_plane: u,
                u_stride: ls[1] as u32,
                v_plane: v,
                v_stride: ls[2] as u32,
                width: w,
                height: h,
            };
            yuv420_to_rgba(&img, &mut self.rgba, w * 4, range, matrix)
                .map_err(|e| format!("Conversión YUV→RGBA: {e:?}"))?;
            Ok((w, h))
        }
    }

    /// Rango y matriz de color que declara el propio flujo de vídeo.
    fn color_info(&self, fmt: c_int, h: u32) -> (YuvRange, YuvStandardMatrix) {
        let get = |name: &str| -> Option<i64> {
            let k = CString::new(name).ok()?;
            let mut v = 0i64;
            let r = unsafe { (self.api.opt_get_int)(self.ctx, k.as_ptr(), 0, &mut v) };
            (r >= 0).then_some(v)
        };
        // AVColorRange: 1 = MPEG (limitado), 2 = JPEG (completo)
        let range = if fmt == AV_PIX_FMT_YUVJ420P || get("color_range") == Some(2) {
            YuvRange::Full
        } else {
            YuvRange::Limited
        };
        // AVColorSpace: 1 BT709, 5/6 BT601, 9/10 BT2020, 2 = sin especificar
        let matrix = match get("colorspace") {
            Some(1) => YuvStandardMatrix::Bt709,
            Some(5) | Some(6) => YuvStandardMatrix::Bt601,
            Some(9) | Some(10) => YuvStandardMatrix::Bt2020,
            _ if h >= 720 => YuvStandardMatrix::Bt709,
            _ => YuvStandardMatrix::Bt601,
        };
        (range, matrix)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            (self.api.frame_free)(&mut self.frame);
            (self.api.packet_free)(&mut self.pkt);
            (self.api.free_context)(&mut self.ctx);
        }
    }
}

/// Lee paquetes del socket del servidor, los decodifica y entrega cada frame
/// RGBA a `sink`. Termina cuando el socket se cierra o hay un error.
pub fn run_stream(
    mut sock: impl Read,
    decoder: &mut Decoder,
    mut sink: impl FnMut(u32, u32, &[u8]),
) -> Result<u64, String> {
    let mut header = [0u8; 12];
    let mut payload: Vec<u8> = Vec::new();
    let mut config: Vec<u8> = Vec::new();
    let mut frames = 0u64;
    loop {
        if let Err(e) = sock.read_exact(&mut header) {
            return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                Err("El teléfono cerró la conexión".into())
            } else {
                Err(format!("Error de transmisión: {e}"))
            };
        }
        let pts_flags = u64::from_be_bytes(header[0..8].try_into().unwrap());
        let size = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;
        if size == 0 || size > MAX_PACKET {
            return Err(format!("Paquete de vídeo inválido ({size} bytes)"));
        }
        payload.resize(size, 0);
        sock.read_exact(&mut payload).map_err(|e| format!("Error de transmisión: {e}"))?;

        if pts_flags & PACKET_FLAG_CONFIG != 0 {
            config.clear();
            config.extend_from_slice(&payload);
            continue;
        }
        if !config.is_empty() {
            // SPS/PPS + primer frame en el mismo paquete, como hace scrcpy.
            let mut merged = std::mem::take(&mut config);
            merged.extend_from_slice(&payload);
            payload = merged;
        }
        decoder.decode(&payload, |w, h, rgba| {
            frames += 1;
            sink(w, h, rgba);
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrcpy_dir() -> Option<PathBuf> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("vendor").join("scrcpy");
        p.join("scrcpy.exe").is_file().then_some(p)
    }

    /// Fragmenta un flujo Annex-B en NAL units y las empaqueta como lo haría
    /// el servidor (cabecera de 12 bytes). Agrupa SPS/PPS como config.
    fn packetize_h264(stream: &[u8]) -> Vec<u8> {
        let mut starts = Vec::new();
        let mut i = 0;
        while i + 3 < stream.len() {
            if stream[i] == 0 && stream[i + 1] == 0 && stream[i + 2] == 1 {
                let s = if i > 0 && stream[i - 1] == 0 { i - 1 } else { i };
                starts.push(s);
                i += 3;
            } else {
                i += 1;
            }
        }
        starts.push(stream.len());
        let mut out = Vec::new();
        let emit = |out: &mut Vec<u8>, flags: u64, data: &[u8]| {
            out.extend_from_slice(&flags.to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
        };
        // Como el servidor: SPS+PPS consecutivos van en UN paquete de config.
        let mut cfg: Vec<u8> = Vec::new();
        for w in starts.windows(2) {
            let nal = &stream[w[0]..w[1]];
            let off = if nal[2] == 1 { 3 } else { 4 };
            let t = nal[off] & 0x1f;
            if t == 7 || t == 8 {
                cfg.extend_from_slice(nal);
                continue;
            }
            if !cfg.is_empty() {
                emit(&mut out, PACKET_FLAG_CONFIG, &cfg);
                cfg.clear();
            }
            emit(&mut out, 0, nal);
        }
        out
    }

    #[test]
    fn decodes_real_stream() {
        let Some(dir) = scrcpy_dir() else { return };
        let Ok(path) = std::env::var("VCAM_TEST_H264") else { return };
        let data = std::fs::read(path).unwrap();
        let data = &data[1..data.len().min(8_000_000)]; // saltar el byte "dummy"
        let packets = packetize_h264(data);
        let mut dec = Decoder::new(&dir, "h264", 1).unwrap();
        let mut n = 0;
        let mut size = (0, 0);
        let mut center = [0u8; 4];
        let t = std::time::Instant::now();
        let _ = run_stream(&packets[..], &mut dec, |w, h, rgba| {
            n += 1;
            size = (w, h);
            let i = ((h / 2 * w + w / 2) * 4) as usize;
            center.copy_from_slice(&rgba[i..i + 4]);
        });
        eprintln!("{n} frames {size:?} en {:?}, centro RGBA {center:?}", t.elapsed());
        assert!(n > 10);
        assert_eq!(size, (1920, 1080));
        assert_eq!(center[3], 255);
    }
}
