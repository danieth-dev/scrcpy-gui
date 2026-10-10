import Foundation
import VideoToolbox
import CoreMedia

/// Codificador de video H.264 por hardware usando VideoToolbox para latencia ultra baja (< 2 ms)
class H264Encoder {
    private var session: VTCompressionSession?
    private var isConfigured = false
    
    var onEncodedPacket: ((Data) -> Void)?
    
    private let naluStartCode = Data([0x00, 0x00, 0x00, 0x01])
    
    func setup(width: Int32, height: Int32, fps: Int32, bitrate: Int32 = 10_000_000) {
        destroy()
        
        let callback: VTCompressionOutputCallback = { outputCallbackRefCon, _, status, flags, sampleBuffer in
            guard status == noErr, let buffer = sampleBuffer, let refCon = outputCallbackRefCon else { return }
            let encoder = Unmanaged<H264Encoder>.fromOpaque(refCon).takeUnretainedValue()
            encoder.handleEncodedBuffer(buffer)
        }
        
        let status = VTCompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            width: width,
            height: height,
            codecType: kCMVideoCodecType_H264,
            encoderSpecification: nil,
            imageBufferAttributes: nil,
            compressedDataAllocator: nil,
            outputCallback: callback,
            refcon: Unmanaged.passUnretained(self).toOpaque(),
            compressionSessionOut: &session
        )
        
        guard status == noErr, let compressionSession = session else {
            print("Error creando VTCompressionSession: \(status)")
            return
        }
        
        // Configuración para latencia de transmisión en tiempo real ultra baja
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_ProfileLevel, value: kVTProfileLevel_H264_High_AutoLevel)
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_AverageBitRate, value: (bitrate as NSNumber))
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_ExpectedFrameRate, value: (fps as NSNumber))
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_MaxKeyFrameInterval, value: (fps as NSNumber))
        VTSessionSetProperty(compressionSession, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse) // Cero B-Frames = Cero retardo
        
        VTCompressionSessionPrepareToEncodeFrames(compressionSession)
        isConfigured = true
    }
    
    func encode(sampleBuffer: CMSampleBuffer) {
        guard isConfigured, let compressionSession = session,
              let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        let duration = CMSampleBufferGetDuration(sampleBuffer)
        
        VTCompressionSessionEncodeFrame(
            compressionSession,
            imageBuffer: imageBuffer,
            presentationTimeStamp: pts,
            duration: duration,
            frameProperties: nil,
            sourceFrameRefcon: nil,
            infoFlagsOut: nil
        )
    }
    
    private func handleEncodedBuffer(_ sampleBuffer: CMSampleBuffer) {
        guard let dataBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else { return }
        
        // Verificar si es un fotograma clave (Keyframe / IDR)
        var isKeyframe = false
        if let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[CFString: Any]],
           let first = attachments.first {
            isKeyframe = (first[kCMSampleAttachmentKey_NotSync] == nil)
        }
        
        // Si es fotograma clave, extraer SPS y PPS y anteponerlos
        if isKeyframe, let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer) {
            var spsSize: Int = 0
            var spsCount: Int = 0
            var spsPtr: UnsafePointer<UInt8>?
            if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(formatDesc, parameterSetIndex: 0, parameterSetPointerOut: &spsPtr, parameterSetSizeOut: &spsSize, parameterSetCountOut: &spsCount, nalUnitHeaderLengthOut: nil) == noErr,
               let sps = spsPtr {
                var spsData = naluStartCode
                spsData.append(sps, count: spsSize)
                onEncodedPacket?(spsData)
            }
            
            var ppsSize: Int = 0
            var ppsPtr: UnsafePointer<UInt8>?
            if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(formatDesc, parameterSetIndex: 1, parameterSetPointerOut: &ppsPtr, parameterSetSizeOut: &ppsSize, parameterSetCountOut: nil, nalUnitHeaderLengthOut: nil) == noErr,
               let pps = ppsPtr {
                var ppsData = naluStartCode
                ppsData.append(pps, count: ppsSize)
                onEncodedPacket?(ppsData)
            }
        }
        
        // Extraer los NAL units del buffer
        var length: Int = 0
        var dataPointer: UnsafeMutablePointer<Int8>?
        if CMBlockBufferGetDataPointer(dataBuffer, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &dataPointer) == noErr,
           let bytes = dataPointer {
            var bufferOffset = 0
            let avcCHeaderLength = 4
            
            while bufferOffset < length - avcCHeaderLength {
                var nalUnitLength: UInt32 = 0
                memcpy(&nalUnitLength, bytes + bufferOffset, avcCHeaderLength)
                nalUnitLength = CFSwapInt32BigToHost(nalUnitLength)
                
                var nalData = naluStartCode
                nalData.append(Data(bytes: bytes + bufferOffset + avcCHeaderLength, count: Int(nalUnitLength)))
                
                onEncodedPacket?(nalData)
                bufferOffset += avcCHeaderLength + Int(nalUnitLength)
            }
        }
    }
    
    func destroy() {
        if let session = session {
            VTCompressionSessionInvalidate(session)
            self.session = nil
        }
        isConfigured = false
    }
    
    deinit {
        destroy()
    }
}
