use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::{problem, tasks, RootHolder};

/// HTTP server on 127.0.0.1:<port>. Competitive Companion POSTs problem
/// JSON to `/`; `/health` is used for port-collision detection.
pub fn serve(root: RootHolder, port: u16) -> io::Result<()> {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            handle_port_collision(port);
        }
        Err(e) => return Err(e),
    };
    eprintln!("[cph] listening on 127.0.0.1:{port}");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let root = root.clone();
            std::thread::spawn(move || handle(stream, root));
        }
    }
    Ok(())
}

/// Another process owns the port. If it is one of ours, exit quietly
/// (first instance wins); otherwise warn loudly.
fn handle_port_collision(port: u16) -> ! {
    match health_check(port) {
        Ok(body) if body.contains("cph-helper") => {
            eprintln!("[cph] :{port} already served by another cph-helper, exiting");
            std::process::exit(0);
        }
        _ => {
            eprintln!("[cph] :{port} is taken by something else; set CPH_PORT to change it");
            std::process::exit(1);
        }
    }
}

fn health_check(port: u16) -> io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(stream, "GET /health HTTP/1.0\r\n\r\n")?;
    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    Ok(body)
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn handle(stream: TcpStream, root: RootHolder) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(stream);

    let Some(request) = read_request(&mut reader) else {
        return;
    };

    let (status, body) = route(&request, &root);
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok()?;
            }
        }
    }

    if content_length > 20 * 1024 * 1024 {
        return None;
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, path, body })
}

fn route(request: &Request, root: &RootHolder) -> (u16, String) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => (200, "\"cph-helper ok\"\n".to_string()),
        ("POST", "/") => {
            let root = root.read().unwrap().clone();
            let Some(root) = root else {
                return (503, "{\"error\":\"worktree root unknown yet\"}".to_string());
            };
            match problem::receive(&root, &request.body) {
                Ok(dir) => {
                    tasks::ensure(&root);
                    (200, format!("{{\"dir\":\"{}\"}}", dir.display()))
                }
                Err(e) => {
                    eprintln!("[cph] problem rejected: {e}");
                    (400, format!("{{\"error\":\"{e}\"}}"))
                }
            }
        }
        _ => (404, "{\"error\":\"not found\"}".to_string()),
    }
}
