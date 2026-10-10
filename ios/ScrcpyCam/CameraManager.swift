import Foundation
import AVFoundation
import VideoToolbox

/// Gestor de captura de video con AVCaptureSession optimizado para 1080p / 60 fps y 4K
class CameraManager: NSObject, ObservableObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    @Published var isRunning = false
    @Published var currentPosition: AVCaptureDevice.Position = .back
    @Published var isTorchOn = false
    @Published var fps: Int = 60
    @Published var resolution: String = "1080p" // "1080p", "4k", "720p"
    
    let captureSession = AVCaptureSession()
    private let videoOutput = AVCaptureVideoDataOutput()
    private let sessionQueue = DispatchQueue(label: "com.scrcpy.camera.sessionQueue", qos: .userInteractive)
    
    var onFrameCaptured: ((CMSampleBuffer) -> Void)?
    
    override init() {
        super.init()
    }
    
    func setupCamera() {
        sessionQueue.async { [weak self] in
            guard let self = self else { return }
            self.captureSession.beginConfiguration()
            
            // Ajustar preset de resolución
            if self.resolution == "4k" {
                if self.captureSession.canSetSessionPreset(.hd4K3840x2160) {
                    self.captureSession.sessionPreset = .hd4K3840x2160
                }
            } else if self.resolution == "720p" {
                if self.captureSession.canSetSessionPreset(.hd1280x720) {
                    self.captureSession.sessionPreset = .hd1280x720
                }
            } else {
                if self.captureSession.canSetSessionPreset(.hd1920x1080) {
                    self.captureSession.sessionPreset = .hd1920x1080
                }
            }
            
            // Entrada de cámara
            guard let camera = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: self.currentPosition),
                  let videoInput = try? AVCaptureDeviceInput(device: camera) else {
                self.captureSession.commitConfiguration()
                return
            }
            
            if self.captureSession.canAddInput(videoInput) {
                self.captureSession.addInput(videoInput)
            }
            
            // Configurar 60 fps si es compatible
            self.configureFrameRate(device: camera, targetFps: self.fps)
            
            // Salida de video en formato YUV (NV12)
            self.videoOutput.alwaysDiscardsLateVideoFrames = true
            self.videoOutput.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
            ]
            self.videoOutput.setSampleBufferDelegate(self, queue: self.sessionQueue)
            
            if self.captureSession.canAddOutput(self.videoOutput) {
                self.captureSession.addOutput(self.videoOutput)
                if let connection = self.videoOutput.connection(with: .video) {
                    connection.videoOrientation = .portrait
                    if connection.isVideoStabilizationSupported {
                        connection.preferredVideoStabilizationMode = .standard
                    }
                }
            }
            
            self.captureSession.commitConfiguration()
            self.captureSession.startRunning()
            
            DispatchQueue.main.async {
                self.isRunning = self.captureSession.isRunning
            }
        }
    }
    
    private func configureFrameRate(device: AVCaptureDevice, targetFps: Int) {
        do {
            try device.lockForConfiguration()
            for range in device.activeFormat.videoSupportedFrameRateRanges {
                if range.maxFrameRate >= Double(targetFps) && range.minFrameRate <= Double(targetFps) {
                    device.activeVideoMinFrameDuration = CMTime(value: 1, timescale: CMTimeScale(targetFps))
                    device.activeVideoMaxFrameDuration = CMTime(value: 1, timescale: CMTimeScale(targetFps))
                    break
                }
            }
            device.unlockForConfiguration()
        } catch {
            print("Error configurando FPS: \(error)")
        }
    }
    
    func switchCamera() {
        sessionQueue.async { [weak self] in
            guard let self = self else { return }
            self.currentPosition = (self.currentPosition == .back) ? .front : .back
            self.captureSession.stopRunning()
            
            // Remover entradas existentes
            for input in self.captureSession.inputs {
                self.captureSession.removeInput(input)
            }
            
            self.setupCamera()
        }
    }
    
    func toggleTorch() {
        guard currentPosition == .back,
              let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
              device.hasTorch else { return }
        do {
            try device.lockForConfiguration()
            isTorchOn.toggle()
            device.torchMode = isTorchOn ? .on : .off
            device.unlockForConfiguration()
        } catch {
            print("Error controlando linterna: \(error)")
        }
    }
    
    func stop() {
        sessionQueue.async { [weak self] in
            self?.captureSession.stopRunning()
            DispatchQueue.main.async {
                self?.isRunning = false
            }
        }
    }
    
    // MARK: - AVCaptureVideoDataOutputSampleBufferDelegate
    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        onFrameCaptured?(sampleBuffer)
    }
}
