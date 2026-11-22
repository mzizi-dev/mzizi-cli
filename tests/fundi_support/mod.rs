//! Shared helpers for the fundi tests: a tiny fixture HTTP server and a CLI runner. No live network.
#![allow(dead_code, missing_docs)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use mzizi_cli::{Env, Io, cli};

#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct Resp {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

impl Resp {
    pub fn json(body: &str) -> Self {
        Resp {
            status: 200,
            content_type: "application/json",
            headers: vec![],
            body: body.into(),
        }
    }
    pub fn status(status: u16) -> Self {
        Resp {
            status,
            content_type: "text/plain",
            headers: vec![],
            body: String::new(),
        }
    }
}

pub struct Server {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Req>>>,
}

type Handler = dyn Fn(&Req) -> Resp + Send + Sync + 'static;

pub fn serve(handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (reqs, handler): (_, Arc<Handler>) = (requests.clone(), Arc::new(handler));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let (reqs, handler) = (reqs.clone(), handler.clone());
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let head_end = loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break p;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                let mut lines = head.split("\r\n");
                let mut first = lines.next().unwrap_or("").split(' ');
                let (method, path) = (
                    first.next().unwrap_or("").to_string(),
                    first.next().unwrap_or("").to_string(),
                );
                let headers: HashMap<String, String> = lines
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
                    .collect();
                let len: usize = headers
                    .get("content-length")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let mut body = buf[head_end + 4..].to_vec();
                while body.len() < len {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&chunk[..n]);
                }
                let req = Req {
                    method,
                    path,
                    headers,
                    body: String::from_utf8_lossy(&body).into_owned(),
                };
                reqs.lock().unwrap().push(req.clone());
                let r = handler(&req);
                let mut out = format!(
                    "HTTP/1.1 {} X\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n",
                    r.status,
                    r.content_type,
                    r.body.len()
                );
                for (k, v) in r.headers {
                    out.push_str(&format!("{k}: {v}\r\n"));
                }
                out.push_str("\r\n");
                let _ = s.write_all(out.as_bytes());
                let _ = s.write_all(r.body.as_bytes());
            });
        }
    });
    Server { url, requests }
}

impl Server {
    pub fn seen(&self) -> Vec<Req> {
        self.requests.lock().unwrap().clone()
    }
}

pub struct Out {
    pub result: Result<(), mzizi_cli::CliError>,
    pub out: String,
    pub err: String,
}

/// Run the CLI in `cwd` with exactly this environment.
pub fn run_cli(args: &[&str], env: &[(&str, &str)], cwd: &std::path::Path) -> Out {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let result = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            cwd: cwd.to_path_buf(),
            env: Env(env
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect()),
        };
        cli::run(&argv, &mut io)
    };
    Out {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

pub fn base64url(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

/// An unsigned JWT: the decoder ignores the signature.
pub fn make_jwt(claims: serde_json::Value) -> String {
    format!(
        "{}.{}.signature-not-checked",
        base64url(r#"{"alg":"none","typ":"JWT"}"#),
        base64url(&claims.to_string())
    )
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn fixture_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fundi-empty-app")
}
