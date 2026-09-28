use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::RootHolder;

/// Minimal LSP server: enough of the protocol for Zed to start us and
/// consider us a valid language server. All editor-facing behaviour is a
/// no-op; the real work happens in the HTTP server thread.
pub fn run(root: RootHolder) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    while let Some(message) = read_message(&mut reader) {
        let Ok(value) = serde_json::from_slice::<Value>(&message) else {
            continue;
        };
        let method = value.get("method").and_then(Value::as_str).unwrap_or("");
        let id = value.get("id").cloned();

        match (id, method) {
            (Some(id), "initialize") => {
                if let Some(uri) = root_uri(&value) {
                    let mut guard = root.write().unwrap();
                    if guard.is_none() {
                        *guard = Some(uri);
                    }
                }
                respond(&mut writer, id, json!({ "capabilities": {} }));
            }
            (Some(id), "shutdown") => {
                respond(&mut writer, id, Value::Null);
            }
            (Some(id), _) => {
                respond(&mut writer, id, Value::Null);
            }
            (None, "exit") => std::process::exit(0),
            (None, _) => {}
        }
    }
}

fn respond(writer: &mut impl Write, id: Value, result: Value) {
    let body = json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string();
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = writer.flush();
}

fn read_message(reader: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return None, // EOF
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    break;
                }
                if let Some(value) = line.strip_prefix("Content-Length:") {
                    content_length = value.trim().parse::<usize>().ok();
                }
            }
            Err(_) => return None,
        }
    }
    let length = content_length?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some(body)
}

/// Extract the worktree root from initialize params (`rootUri` or the first
/// workspace folder), decoding the file:// URI.
fn root_uri(message: &Value) -> Option<PathBuf> {
    let params = message.get("params")?;
    let uri = params
        .get("rootUri")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            params
                .get("workspaceFolders")
                .and_then(Value::as_array)
                .and_then(|folders| folders.first())
                .and_then(|f| f.get("uri"))
                .and_then(Value::as_str)
        })?;
    decode_file_uri(uri)
}

fn decode_file_uri(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    let mut out = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            let value = u8::from_str_radix(hex, 16).ok()?;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}
