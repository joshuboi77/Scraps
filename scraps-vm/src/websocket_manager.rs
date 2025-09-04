use std::collections::HashMap;
use std::net::TcpStream;
use tungstenite::{connect, Message, WebSocket, Error as WsError};
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub struct WebSocketManager {
    connections: HashMap<usize, WebSocket<MaybeTlsStream<TcpStream>>>,
    next_id: usize,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self { connections: HashMap::new(), next_id: 1 }
    }

    pub fn connect(&mut self, url: &str) -> Result<usize, String> {
        let url = Url::parse(url).map_err(|e| format!("Invalid WebSocket URL: {}", e))?;
        let (ws, _resp) = connect(url).map_err(|e| format!("WebSocket connect error: {}", e))?;
        // Switch underlying stream to non-blocking so we can implement try-receive
        match ws.get_ref() {
            MaybeTlsStream::Plain(tcp) => {
                let _ = tcp.set_nonblocking(true);
            }
            MaybeTlsStream::NativeTls(tls) => {
                let _ = tls.get_ref().set_nonblocking(true);
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
        let id = self.next_id;
        self.next_id += 1;
        self.connections.insert(id, ws);
        Ok(id)
    }

    pub fn send_text(&mut self, id: usize, text: &str) -> Result<bool, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        ws.send(Message::Text(text.to_string()))
            .map(|_| true)
            .map_err(|e| format!("WebSocket send error: {}", e))
    }

    pub fn send_binary(&mut self, id: usize, bytes: &[u8]) -> Result<bool, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        ws.send(Message::Binary(bytes.to_vec()))
            .map(|_| true)
            .map_err(|e| format!("WebSocket send error: {}", e))
    }

    pub fn receive_text(&mut self, id: usize) -> Result<String, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        match ws.read() {
            Ok(msg) => match msg {
                Message::Text(s) => Ok(s),
                Message::Binary(b) => String::from_utf8(b).map_err(|e| format!("Non-UTF8 binary: {}", e)),
                Message::Ping(_) | Message::Pong(_) => Ok(String::new()),
                Message::Close(_) => Ok(String::new()),
                _ => Ok(String::new()),
            },
            Err(e) => {
                if matches!(e, WsError::Io(ref ioe) if ioe.kind() == std::io::ErrorKind::WouldBlock) {
                    Ok(String::new())
                } else {
                    Err(format!("WebSocket receive error: {}", e))
                }
            }
        }
    }

    pub fn try_receive_text(&mut self, id: usize) -> Result<Option<String>, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        match ws.read() {
            Ok(msg) => {
                let s = match msg {
                    Message::Text(s) => s,
                    Message::Binary(b) => String::from_utf8(b).map_err(|e| format!("Non-UTF8 binary: {}", e))?,
                    Message::Ping(_) | Message::Pong(_) | Message::Close(_) => String::new(),
                    _ => String::new(),
                };
                Ok(Some(s))
            }
            Err(e) => {
                if matches!(e, WsError::Io(ref ioe) if ioe.kind() == std::io::ErrorKind::WouldBlock) {
                    Ok(None)
                } else {
                    Err(format!("WebSocket receive error: {}", e))
                }
            }
        }
    }

    pub fn receive_bytes(&mut self, id: usize) -> Result<Vec<u8>, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        match ws.read() {
            Ok(msg) => match msg {
                Message::Text(s) => Ok(s.into_bytes()),
                Message::Binary(b) => Ok(b),
                _ => Ok(Vec::new()),
            },
            Err(e) => {
                if matches!(e, WsError::Io(ref ioe) if ioe.kind() == std::io::ErrorKind::WouldBlock) {
                    Ok(Vec::new())
                } else {
                    Err(format!("WebSocket receive error: {}", e))
                }
            }
        }
    }

    pub fn try_receive_bytes(&mut self, id: usize) -> Result<Option<Vec<u8>>, String> {
        let ws = self.connections.get_mut(&id).ok_or("WebSocket not found")?;
        match ws.read() {
            Ok(msg) => {
                let v = match msg {
                    Message::Text(s) => s.into_bytes(),
                    Message::Binary(b) => b,
                    _ => Vec::new(),
                };
                Ok(Some(v))
            }
            Err(e) => {
                if matches!(e, WsError::Io(ref ioe) if ioe.kind() == std::io::ErrorKind::WouldBlock) {
                    Ok(None)
                } else {
                    Err(format!("WebSocket receive error: {}", e))
                }
            }
        }
    }

    pub fn close(&mut self, id: usize) -> Result<bool, String> {
        if let Some(mut ws) = self.connections.remove(&id) {
            let _ = ws.close(None);
            Ok(true)
        } else {
            Err("WebSocket not found".to_string())
        }
    }
}
