//! A minimal, blocking client for the greetd IPC protocol
//! (<https://man.sr.ht/~kennylevinsen/greetd/>). Messages are framed as a
//! 4-byte little-endian length prefix followed by UTF-8 JSON. No async
//! runtime: this runs on its own thread, one login attempt at a time — the
//! same pattern `apps/installer` uses for the privileged backend.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    CreateSession { username: String },
    PostAuthMessageResponse { response: Option<String> },
    StartSession { cmd: Vec<String>, env: Vec<String> },
    CancelSession,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Success,
    Error {
        #[serde(default)]
        error_type: String,
        #[serde(default)]
        description: String,
    },
    AuthMessage {
        auth_message_type: String,
        #[serde(default)]
        auth_message: String,
    },
}

#[derive(Debug, Clone)]
pub enum LoginResult {
    Success,
    /// Wrong password / PAM rejected the attempt — safe to show the user and
    /// let them retry.
    Rejected(String),
    /// Something environmental went wrong (no GREETD_SOCK, socket error...).
    Error(String),
}

/// Log in as `username`/`password` and, on success, ask greetd to start
/// `mavind-session` for that user. Opens a fresh connection each call, so a
/// failed attempt leaves nothing to clean up before retrying.
pub fn attempt_login(username: &str, password: &str) -> LoginResult {
    let sock_path = match std::env::var("GREETD_SOCK") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return LoginResult::Error(
                "GREETD_SOCK is not set — mavind-greeter must be run as the greetd session command"
                    .into(),
            )
        }
    };
    match run(&sock_path, username, password) {
        Ok(()) => LoginResult::Success,
        Err(Failure::Rejected(m)) => LoginResult::Rejected(m),
        Err(Failure::Io(m)) => LoginResult::Error(m),
    }
}

enum Failure {
    Rejected(String),
    Io(String),
}
impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Failure::Io(e.to_string())
    }
}

fn run(sock_path: &str, username: &str, password: &str) -> Result<(), Failure> {
    let mut s = UnixStream::connect(sock_path)?;
    send(&mut s, &Request::CreateSession { username: username.to_string() })?;

    // Answer auth prompts until greetd says Success or Error. Our PAM setup
    // asks for exactly one secret (the password); if it ever asks more than
    // once, reuse the same password rather than getting stuck, and just
    // acknowledge informational/error prompts with an empty response.
    loop {
        match recv(&mut s)? {
            Response::Success => break,
            Response::AuthMessage { auth_message_type, .. } => {
                let answer = match auth_message_type.as_str() {
                    "visible" | "secret" => Some(password.to_string()),
                    _ => None,
                };
                send(&mut s, &Request::PostAuthMessageResponse { response: answer })?;
            }
            Response::Error { description, .. } => {
                let _ = send(&mut s, &Request::CancelSession);
                return Err(Failure::Rejected(nice_reason(&description)));
            }
        }
    }

    send(
        &mut s,
        &Request::StartSession { cmd: vec!["mavind-session".to_string()], env: vec![] },
    )?;
    match recv(&mut s)? {
        Response::Success => Ok(()),
        Response::Error { description, .. } => Err(Failure::Rejected(nice_reason(&description))),
        Response::AuthMessage { .. } => {
            Err(Failure::Io("unexpected prompt after start_session".into()))
        }
    }
}

fn nice_reason(description: &str) -> String {
    if description.is_empty() {
        "Incorrect password".into()
    } else {
        description.to_string()
    }
}

fn send(s: &mut UnixStream, req: &Request) -> io::Result<()> {
    let body = serde_json::to_vec(req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    s.write_all(&(body.len() as u32).to_le_bytes())?;
    s.write_all(&body)
}

fn recv(s: &mut UnixStream) -> io::Result<Response> {
    let mut len_buf = [0u8; 4];
    s.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf)?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
