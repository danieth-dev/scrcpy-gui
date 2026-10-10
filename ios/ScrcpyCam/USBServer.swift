import Foundation
import Network

/// Servidor TCP que escucha en el puerto 50005 para transmitir video al PC a través del cable USB (usbmuxd)
class USBServer: ObservableObject {
    @Published var isClientConnected = false
    @Published var clientAddress: String = ""
    @Published var totalBytesSent: Int64 = 0
    
    private var listener: NWListener?
    private var activeConnection: NWConnection?
    private let queue = DispatchQueue(label: "com.scrcpy.usb.serverQueue", qos: .userInteractive)
    
    let port: UInt16
    
    init(port: UInt16 = 50005) {
        self.port = port
    }
    
    func start() {
        guard let endpointPort = NWEndpoint.Port(rawValue: port) else { return }
        
        let tcpOptions = NWProtocolTCP.Options()
        tcpOptions.noDelay = true
        
        let params = NWParameters(tls: nil, tcp: tcpOptions)
        params.allowLocalEndpointReuse = true
        
        do {
            listener = try NWListener(using: params, on: endpointPort)
            
            listener?.stateUpdateHandler = { [weak self] state in
                switch state {
                case .ready:
                    print("[USBServer] Escuchando en puerto \(self?.port ?? 50005) a través de usbmuxd...")
                case .failed(let error):
                    print("[USBServer] Error en listener: \(error)")
                default:
                    break
                }
            }
            
            listener?.newConnectionHandler = { [weak self] newConnection in
                self?.handleNewConnection(newConnection)
            }
            
            listener?.start(queue: queue)
        } catch {
            print("[USBServer] No se pudo iniciar el listener: \(error)")
        }
    }
    
    private func handleNewConnection(_ connection: NWConnection) {
        // Aceptar solo 1 cliente activo (la PC a través del cable USB)
        activeConnection?.cancel()
        activeConnection = connection
        
        connection.stateUpdateHandler = { [weak self] state in
            DispatchQueue.main.async {
                switch state {
                case .ready:
                    self?.isClientConnected = true
                    self?.clientAddress = "\(connection.endpoint)"
                    print("[USBServer] PC conectada por cable USB: \(connection.endpoint)")
                case .failed, .cancelled:
                    self?.isClientConnected = false
                    self?.clientAddress = ""
                    print("[USBServer] Conexión USB cerrada")
                default:
                    break
                }
            }
        }
        
        connection.start(queue: queue)
    }
    
    func sendPacket(_ data: Data) {
        guard let connection = activeConnection, isClientConnected else { return }
        
        connection.send(content: data, completion: .contentProcessed { [weak self] error in
            if error == nil {
                DispatchQueue.main.async {
                    self?.totalBytesSent += Int64(data.count)
                }
            }
        })
    }
    
    func stop() {
        activeConnection?.cancel()
        activeConnection = nil
        listener?.cancel()
        listener = nil
        DispatchQueue.main.async {
            self.isClientConnected = false
        }
    }
    
    deinit {
        stop()
    }
}
