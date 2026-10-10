import SwiftUI
import AVFoundation
import UIKit

struct ContentView: View {
    @StateObject private var camera = CameraManager()
    @StateObject private var server = USBServer()
    private let encoder = H264Encoder()
    
    @State private var isBlackScreenActive = false
    @State private var brightnessBeforeBlack: CGFloat = 0.5
    
    var body: some View {
        ZStack {
            Color.black.edgesIgnoringSafeArea(.all)
            
            if isBlackScreenActive {
                // ── MODO PANTALLA NEGRA (Ahorro OLED & Cero Calor) ─────────────
                ZStack {
                    Color.black.edgesIgnoringSafeArea(.all)
                    VStack(spacing: 16) {
                        Image(systemName: "video.fill")
                            .font(.system(size: 32))
                            .foregroundColor(Color(red: 61/255, green: 220/255, blue: 132/255).opacity(0.35))
                        
                        Text("Transmitiendo por USB")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundColor(.white.opacity(0.4))
                        
                        Text("Toca la pantalla para encender")
                            .font(.system(size: 12))
                            .foregroundColor(.white.opacity(0.2))
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture {
                    withAnimation(.easeInOut(duration: 0.25)) {
                        isBlackScreenActive = false
                        UIScreen.main.brightness = brightnessBeforeBlack
                    }
                }
            } else {
                // ── VISTA PRINCIPAL CON CONTROLES ────────────────────────────
                VStack(spacing: 0) {
                    
                    // Barra superior
                    HStack {
                        HStack(spacing: 8) {
                            Circle()
                                .fill(Color(red: 61/255, green: 220/255, blue: 132/255))
                                .frame(width: 8, height: 8)
                            Text("scrcpy")
                                .font(.system(size: 18, weight: .bold))
                                .foregroundColor(.white)
                            Text("CAM")
                                .font(.system(size: 11, weight: .bold))
                                .foregroundColor(Color(red: 61/255, green: 220/255, blue: 132/255))
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color(red: 61/255, green: 220/255, blue: 132/255).opacity(0.15))
                                .cornerRadius(4)
                        }
                        
                        Spacer()
                        
                        // Estado USB
                        HStack(spacing: 6) {
                            Circle()
                                .fill(server.isClientConnected ? Color(red: 61/255, green: 220/255, blue: 132/255) : Color.orange)
                                .frame(width: 7, height: 7)
                            Text(server.isClientConnected ? "USB Conectado" : "Esperando PC...")
                                .font(.system(size: 12, weight: .medium))
                                .foregroundColor(.white.opacity(0.85))
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(Color.white.opacity(0.08))
                        .cornerRadius(20)
                    }
                    .padding(.horizontal, 20)
                    .padding(.top, 16)
                    .padding(.bottom, 12)
                    
                    // Vista previa de cámara
                    ZStack {
                        CameraPreviewRepresentable(session: camera.captureSession)
                            .cornerRadius(16)
                            .clipped()
                            .overlay(
                                RoundedRectangle(cornerRadius: 16)
                                    .stroke(Color.white.opacity(0.12), lineWidth: 1)
                            )
                        
                        // Badge con resolución y FPS
                        VStack {
                            HStack {
                                Spacer()
                                Text("\(camera.resolution.uppercased()) · \(camera.fps) FPS")
                                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                                    .foregroundColor(.white)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(Color.black.opacity(0.65))
                                    .cornerRadius(6)
                                    .padding(12)
                            }
                            Spacer()
                        }
                    }
                    .padding(.horizontal, 16)
                    
                    // Panel de controles inferiores
                    VStack(spacing: 16) {
                        
                        // Botones de acción rápida
                        HStack(spacing: 20) {
                            // Cambiar lente
                            Button(action: { camera.switchCamera() }) {
                                VStack(spacing: 4) {
                                    Image(systemName: "camera.rotate")
                                        .font(.system(size: 20))
                                    Text("Girar")
                                        .font(.system(size: 11))
                                }
                                .foregroundColor(.white)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 10)
                                .background(Color.white.opacity(0.08))
                                .cornerRadius(12)
                            }
                            
                            // Linterna
                            Button(action: { camera.toggleTorch() }) {
                                VStack(spacing: 4) {
                                    Image(systemName: camera.isTorchOn ? "flashlight.on.fill" : "flashlight.off.fill")
                                        .font(.system(size: 20))
                                    Text("Luz")
                                        .font(.system(size: 11))
                                }
                                .foregroundColor(camera.isTorchOn ? Color(red: 61/255, green: 220/255, blue: 132/255) : .white)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 10)
                                .background(Color.white.opacity(0.08))
                                .cornerRadius(12)
                            }
                            
                            // Modo Apagar Pantalla
                            Button(action: {
                                withAnimation(.easeInOut(duration: 0.25)) {
                                    brightnessBeforeBlack = UIScreen.main.brightness
                                    isBlackScreenActive = true
                                    UIScreen.main.brightness = 0.0
                                }
                            }) {
                                VStack(spacing: 4) {
                                    Image(systemName: "moon.fill")
                                        .font(.system(size: 20))
                                    Text("Apagar")
                                        .font(.system(size: 11))
                                }
                                .foregroundColor(Color(red: 61/255, green: 220/255, blue: 132/255))
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 10)
                                .background(Color(red: 61/255, green: 220/255, blue: 132/255).opacity(0.12))
                                .cornerRadius(12)
                            }
                        }
                        
                        // Selector de calidad
                        Picker("Calidad", selection: $camera.resolution) {
                            Text("720p 60fps").tag("720p")
                            Text("1080p 60fps").tag("1080p")
                            Text("4K 60fps").tag("4k")
                        }
                        .pickerStyle(SegmentedPickerStyle())
                        .onChange(of: camera.resolution) { _ in
                            reconfigurePipeline()
                        }
                    }
                    .padding(20)
                }
            }
        }
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = true // Evitar que el iPhone entre en suspensión
            setupPipeline()
        }
    }
    
    private func setupPipeline() {
        let width: Int32 = (camera.resolution == "4k") ? 3840 : (camera.resolution == "720p") ? 1280 : 1920
        let height: Int32 = (camera.resolution == "4k") ? 2160 : (camera.resolution == "720p") ? 720 : 1080
        
        encoder.setup(width: width, height: height, fps: Int32(camera.fps))
        encoder.onEncodedPacket = { data in
            server.sendPacket(data)
        }
        
        camera.onFrameCaptured = { sampleBuffer in
            encoder.encode(sampleBuffer: sampleBuffer)
        }
        
        camera.setupCamera()
        server.start()
    }
    
    private func reconfigurePipeline() {
        camera.stop()
        setupPipeline()
    }
}

// Representable para la vista previa de UIKit
struct CameraPreviewRepresentable: UIViewRepresentable {
    let session: AVCaptureSession
    
    func makeUIView(context: Context) -> CameraPreviewView {
        let view = CameraPreviewView()
        view.videoPreviewLayer.session = session
        view.videoPreviewLayer.videoGravity = .resizeAspectFill
        return view
    }
    
    func updateUIView(_ uiView: CameraPreviewView, context: Context) {}
}

class CameraPreviewView: UIView {
    override class var layerClass: AnyClass {
        return AVCaptureVideoPreviewLayer.self
    }
    
    var videoPreviewLayer: AVCaptureVideoPreviewLayer {
        return layer as! AVCaptureVideoPreviewLayer
    }
}
