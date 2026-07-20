use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Result of a send operation.
#[derive(Clone, Debug)]
pub struct SendResult {
    pub success: bool,
    pub response: String,
    pub duration_ms: u64,
    pub error: String,
}

/// Result of a connectivity test.
#[derive(Clone, Debug)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}

#[derive(Clone, PartialEq)]
pub enum ProxyKind {
    None,
    Socks5,
    Http,
}

/// Connect through a proxy and return the established TcpStream.
fn connect_with_proxy(
    target_addr: &SocketAddr,
    proxy_addr: &SocketAddr,
    proxy_kind: &ProxyKind,
    timeout: Duration,
) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect_timeout(proxy_addr, timeout)
        .map_err(|e| format!("代理连接失败: {}", e))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("设置超时失败: {}", e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("设置超时失败: {}", e))?;

    match proxy_kind {
        ProxyKind::Socks5 => socks5_handshake(&mut stream, target_addr, timeout),
        ProxyKind::Http => http_connect_handshake(&mut stream, target_addr, timeout),
        ProxyKind::None => unreachable!(),
    }?;

    Ok(stream)
}

/// SOCKS5 handshake (no auth).
fn socks5_handshake(
    stream: &mut TcpStream,
    target: &SocketAddr,
    _timeout: Duration,
) -> Result<(), String> {
    // greeting: SOCKS5, 1 method, no auth
    let greeting = [5u8, 1, 0];
    stream
        .write_all(&greeting)
        .map_err(|e| format!("SOCKS5 发送失败: {}", e))?;

    let mut resp = [0u8; 2];
    stream
        .read_exact(&mut resp)
        .map_err(|e| format!("SOCKS5 接收失败: {}", e))?;
    if resp[0] != 5 || resp[1] != 0 {
        return Err(format!("SOCKS5 不支持的认证方式: {:?}", resp));
    }

    // connect request
    let addr_bytes = match target {
        SocketAddr::V4(v4) => {
            let mut buf = Vec::with_capacity(10);
            buf.extend_from_slice(&[5u8, 1, 0, 1]);
            buf.extend_from_slice(&v4.ip().octets());
            buf.extend_from_slice(&v4.port().to_be_bytes());
            buf
        }
        SocketAddr::V6(v6) => {
            let mut buf = Vec::with_capacity(22);
            buf.extend_from_slice(&[5u8, 1, 0, 4]);
            buf.extend_from_slice(&v6.ip().octets());
            buf.extend_from_slice(&v6.port().to_be_bytes());
            buf
        }
    };
    stream
        .write_all(&addr_bytes)
        .map_err(|e| format!("SOCKS5 请求发送失败: {}", e))?;

    // read response (variable length, but we handle up to 22 bytes for IPv6 + 2 header)
    let mut header = [0u8; 4];
    stream
        .read_exact(&mut header)
        .map_err(|e| format!("SOCKS5 响应读取失败: {}", e))?;
    if header[0] != 5 || header[1] != 0 {
        return Err(format!(
            "SOCKS5 连接被拒绝 (code: {})",
            header[1]
        ));
    }
    // skip the rest of address according to type
    match header[3] {
        1 => {
            // IPv4: 4 bytes IP + 2 bytes port
            let mut rest = [0u8; 6];
            stream.read_exact(&mut rest).map_err(|e| format!("SOCKS5 读取失败: {}", e))?;
        }
        3 => {
            // domain name: 1 byte length + name + 2 bytes port
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).map_err(|e| format!("SOCKS5 读取失败: {}", e))?;
            let domain_len = len_buf[0] as usize;
            let mut rest = vec![0u8; domain_len + 2];
            stream.read_exact(&mut rest).map_err(|e| format!("SOCKS5 读取失败: {}", e))?;
        }
        4 => {
            // IPv6: 16 bytes IP + 2 bytes port
            let mut rest = [0u8; 18];
            stream.read_exact(&mut rest).map_err(|e| format!("SOCKS5 读取失败: {}", e))?;
        }
        _ => return Err(format!("SOCKS5 未知地址类型: {}", header[3])),
    }

    Ok(())
}

/// HTTP CONNECT handshake.
fn http_connect_handshake(
    stream: &mut TcpStream,
    target: &SocketAddr,
    _timeout: Duration,
) -> Result<(), String> {
    let host = target.to_string(); // "ip:port"
    let req = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n", host, host);
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("HTTP CONNECT 发送失败: {}", e))?;

    let mut buf = [0u8; 4096];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("HTTP CONNECT 响应读取失败: {}", e))?;
    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.starts_with("HTTP/1.1 2") && !response.starts_with("HTTP/1.0 2") {
        let first_line = response.lines().next().unwrap_or("unknown");
        return Err(format!("HTTP CONNECT 失败: {}", first_line));
    }
    Ok(())
}

/// Connect to target, optionally via proxy, send data, and read response.
/// Returns the raw bytes received (up to `max_read`).
pub fn send_and_receive(
    host: &str,
    port: u16,
    timeout_ms: u64,
    proxy_kind: &ProxyKind,
    proxy_host: &str,
    proxy_port: u16,
    payload: &[u8],
    max_read: usize,
) -> Result<(Vec<u8>, u64), String> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    let addr = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS解析失败: {}", e))?
        .next()
        .ok_or_else(|| "无法解析主机地址".to_string())?;

    let mut stream = if *proxy_kind != ProxyKind::None {
        let proxy_addr = format!("{}:{}", proxy_host, proxy_port)
            .to_socket_addrs()
            .map_err(|e| format!("代理地址解析失败: {}", e))?
            .next()
            .ok_or_else(|| "无法解析代理地址".to_string())?;
        connect_with_proxy(&addr, &proxy_addr, proxy_kind, timeout)?
    } else {
        TcpStream::connect_timeout(&addr, timeout)
            .map_err(|e| format!("连接失败: {}", e))?
    };

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("设置超时失败: {}", e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("设置超时失败: {}", e))?;

    // Send payload
    stream
        .write_all(payload)
        .map_err(|e| format!("发送数据失败: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("刷新数据失败: {}", e))?;

    // Read response
    let mut response = Vec::with_capacity(4096);
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                if response.len() >= max_read {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if response.is_empty() {
                    return Err(format!("读取超时 ({}ms)", timeout_ms));
                }
                break;
            }
            Err(e) => {
                return Err(format!("读取数据失败: {}", e));
            }
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;
    Ok((response, elapsed))
}

/// Test connectivity to target, optionally via proxy.
pub fn test_connectivity(
    host: &str,
    port: u16,
    timeout_ms: u64,
    proxy_kind: &ProxyKind,
    proxy_host: &str,
    proxy_port: u16,
) -> TestResult {
    let timeout = Duration::from_millis(timeout_ms);
    let addr = format!("{}:{}", host, port);

    let result = if *proxy_kind != ProxyKind::None {
        let target_addr = match addr.to_socket_addrs() {
            Ok(mut addrs) => addrs.next(),
            Err(e) => return TestResult {
                success: false,
                message: format!("DNS解析失败: {}", e),
            },
        };
        let target_addr = match target_addr {
            Some(a) => a,
            None => return TestResult {
                success: false,
                message: "无法解析目标地址".to_string(),
            },
        };
        let proxy_addr = match format!("{}:{}", proxy_host, proxy_port).to_socket_addrs() {
            Ok(mut addrs) => addrs.next(),
            Err(e) => return TestResult {
                success: false,
                message: format!("代理地址解析失败: {}", e),
            },
        };
        let proxy_addr = match proxy_addr {
            Some(a) => a,
            None => return TestResult {
                success: false,
                message: "无法解析代理地址".to_string(),
            },
        };
        connect_with_proxy(&target_addr, &proxy_addr, proxy_kind, timeout).map(|_| ())
    } else {
        let sock_addrs = match addr.to_socket_addrs() {
            Ok(a) => a,
            Err(e) => return TestResult {
                success: false,
                message: format!("DNS解析失败: {}", e),
            },
        };
        let sock_addr = match sock_addrs.into_iter().next() {
            Some(a) => a,
            None => return TestResult {
                success: false,
                message: "无法解析目标地址".to_string(),
            },
        };
        match TcpStream::connect_timeout(&sock_addr, timeout) {
            Ok(stream) => {
                let _ = stream;
                Ok(())
            }
            Err(e) => Err(format!("连接失败: {}", e)),
        }
    };

    match result {
        Ok(_) => TestResult {
            success: true,
            message: "成功: 连接已建立".to_string(),
        },
        Err(e) => TestResult {
            success: false,
            message: format!("失败: {}", e),
        },
    }
}
