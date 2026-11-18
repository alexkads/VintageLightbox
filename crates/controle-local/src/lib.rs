//! Local authenticated transport for the application's UI thread and its CLI.
pub mod cli;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub const LIMIT: u64 = 64 * 1024;
const RESPONSE_LIMIT: u64 = 16 * 1024 * 1024;
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Status,
    Describe {
        #[serde(default)]
        window: Option<u64>,
        #[serde(default)]
        query: Option<String>,
        #[serde(default)]
        full: bool,
    },
    Screenshot {
        path: PathBuf,
        #[serde(default)]
        window: Option<u64>,
    },
    Key {
        key: String,
        #[serde(default)]
        window: Option<u64>,
    },
    Text {
        text: String,
        #[serde(default)]
        window: Option<u64>,
    },
    Click {
        x: f32,
        y: f32,
        #[serde(default)]
        right: bool,
        #[serde(default)]
        window: Option<u64>,
    },
    Scroll {
        x: f32,
        y: f32,
        dy: f32,
        #[serde(default)]
        window: Option<u64>,
    },
    Drag {
        x: f32,
        y: f32,
        to_x: f32,
        to_y: f32,
        #[serde(default)]
        window: Option<u64>,
    },
    Action {
        name: String,
        #[serde(default)]
        args: Option<Value>,
        #[serde(default)]
        window: Option<u64>,
    },
    Node {
        id: u64,
        action: String,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        window: Option<u64>,
    },
}
impl Command {
    pub fn window(&self) -> Option<u64> {
        match self {
            Self::Status => None,
            Self::Describe { window, .. }
            | Self::Screenshot { window, .. }
            | Self::Key { window, .. }
            | Self::Text { window, .. }
            | Self::Click { window, .. }
            | Self::Scroll { window, .. }
            | Self::Drag { window, .. }
            | Self::Action { window, .. }
            | Self::Node { window, .. } => *window,
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Endpoint {
    version: u32,
    address: SocketAddr,
    token: String,
    pid: u32,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    token: String,
    command: Command,
    timeout_ms: u64,
}
pub struct Request {
    pub command: Command,
    pub expires: Instant,
    pub reply: mpsc::Sender<Value>,
}
pub struct Server {
    stop: Arc<AtomicBool>,
    path: PathBuf,
    token: String,
    _lock: File,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if read_endpoint(&self.path).is_ok_and(|e| e.token == self.token) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub fn error(message: impl ToString) -> Value {
    json!({"ok":false,"error":message.to_string()})
}
pub fn success(value: Value) -> Value {
    json!({"ok":true,"result":value})
}
fn read_endpoint(path: &Path) -> io::Result<Endpoint> {
    serde_json::from_reader(File::open(path)?.take(LIMIT)).map_err(io::Error::other)
}
pub fn start(dir: &Path) -> io::Result<(Server, mpsc::Receiver<Request>)> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(dir.join("controle.lock"))?;
    lock.try_lock()
        .map_err(|e| io::Error::other(format!("another controller owns this directory: {e}")))?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let endpoint = Endpoint {
        version: VERSION,
        address: listener.local_addr()?,
        token: uuid::Uuid::new_v4().to_string(),
        pid: std::process::id(),
    };
    let path = dir.join("controle.json");
    let temporary = dir.join("controle.tmp");
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    serde_json::to_writer(&mut file, &endpoint)?;
    file.sync_all()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(temporary, &path)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let token = endpoint.token.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // BSD/macOS may inherit O_NONBLOCK from the listener. A
                    // request can arrive just after accept; honor read_timeout
                    // rather than treating that race as a protocol failure.
                    let _ = stream.set_nonblocking(false);
                    let response = serve(&mut stream, &token, &sender).unwrap_or_else(error);
                    let _ = serde_json::to_writer(&mut stream, &response);
                    let _ = stream.write_all(b"\n");
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
    });
    Ok((
        Server {
            stop,
            path,
            token: endpoint.token,
            _lock: lock,
        },
        receiver,
    ))
}
fn serve(stream: &mut TcpStream, token: &str, sender: &mpsc::Sender<Request>) -> io::Result<Value> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    BufReader::new(stream.take(LIMIT + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Ok(error("request too large"));
    }
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if envelope.token != token {
        return Ok(error("unauthorized"));
    }
    let timeout = Duration::from_millis(envelope.timeout_ms.clamp(1, 60_000));
    let (reply, receive) = mpsc::channel();
    sender
        .send(Request {
            command: envelope.command,
            expires: Instant::now() + timeout,
            reply,
        })
        .map_err(io::Error::other)?;
    Ok(receive
        .recv_timeout(timeout)
        .unwrap_or_else(|_| error("UI timeout; query status before retrying a mutating command")))
}
pub fn call(dir: &Path, command: Command, timeout: Duration) -> io::Result<Value> {
    if timeout.is_zero() || timeout > Duration::from_secs(60) {
        return Err(io::Error::other("timeout must be between 1 ms and 60 s"));
    }
    let endpoint=read_endpoint(&dir.join("controle.json")).map_err(|e|io::Error::other(format!("controller unavailable; open the app with VLB_CONTROLE pointing to this directory: {e}")))?;
    if endpoint.version != VERSION || !endpoint.address.ip().is_loopback() {
        return Err(io::Error::other("invalid local endpoint"));
    }
    let mut stream = TcpStream::connect_timeout(&endpoint.address, timeout).map_err(|e| {
        io::Error::other(format!(
            "controller unavailable; the app may have closed since this endpoint was written: {e}"
        ))
    })?;
    stream.set_read_timeout(Some(timeout + Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(timeout))?;
    serde_json::to_writer(
        &mut stream,
        &Envelope {
            token: endpoint.token,
            command,
            timeout_ms: timeout.as_millis() as u64,
        },
    )?;
    stream.write_all(b"\n")?;
    let mut bytes = Vec::new();
    BufReader::new(stream.take(RESPONSE_LIMIT + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > RESPONSE_LIMIT {
        return Err(io::Error::other("response too large"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_unicode_and_window_selection() {
        let dir = tempfile::tempdir().unwrap();
        let (_server, requests) = start(dir.path()).unwrap();
        let handler = thread::spawn(move || {
            let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(request.expires > Instant::now());
            match request.command {
                Command::Text {
                    text,
                    window: Some(42),
                } => assert_eq!(text, "João 📷\nOlá"),
                _ => panic!("command changed in transport"),
            }
            request
                .reply
                .send(success(json!({"inserted":true})))
                .unwrap();
        });
        let response = call(
            dir.path(),
            Command::Text {
                text: "João 📷\nOlá".into(),
                window: Some(42),
            },
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(response, success(json!({"inserted":true})));
        handler.join().unwrap();
    }

    #[test]
    fn accepted_connection_waits_for_a_client_that_has_not_written_yet() {
        let dir = tempfile::tempdir().unwrap();
        let (_server, requests) = start(dir.path()).unwrap();
        let endpoint = read_endpoint(&dir.path().join("controle.json")).unwrap();
        let mut stream = TcpStream::connect(endpoint.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        thread::sleep(Duration::from_millis(80));
        serde_json::to_writer(
            &mut stream,
            &Envelope {
                token: endpoint.token,
                command: Command::Status,
                timeout_ms: 1000,
            },
        )
        .unwrap();
        stream.write_all(b"\n").unwrap();
        let request = requests.recv_timeout(Duration::from_secs(1)).unwrap();
        request.reply.send(success(json!({"ready":true}))).unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["ok"], true);
    }

    fn raw(dir: &Path, bytes: &[u8]) -> Value {
        let endpoint = read_endpoint(&dir.join("controle.json")).unwrap();
        let mut stream = TcpStream::connect(endpoint.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream.write_all(bytes).unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    #[test]
    fn unauthorized_malformed_and_oversized_commands_never_reach_ui() {
        let dir = tempfile::tempdir().unwrap();
        let (_server, requests) = start(dir.path()).unwrap();
        let token = read_endpoint(&dir.path().join("controle.json"))
            .unwrap()
            .token;
        let unauthorized = raw(
            dir.path(),
            b"{\"token\":\"wrong\",\"command\":{\"op\":\"status\"},\"timeout_ms\":100}\n",
        );
        assert_eq!(unauthorized["error"], "unauthorized");
        let malformed = format!("{{\"token\":\"{token}\",\"command\":{{\"op\":\"shell\",\"command\":\"whoami\"}},\"timeout_ms\":100}}\n");
        assert_eq!(raw(dir.path(), malformed.as_bytes())["ok"], false);
        let oversized = vec![b'x'; LIMIT as usize + 1];
        assert_eq!(raw(dir.path(), &oversized)["error"], "request too large");
        assert!(requests.try_recv().is_err());
    }

    #[test]
    fn single_owner_permissions_cleanup_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let (server, _requests) = start(dir.path()).unwrap();
        assert!(start(dir.path()).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(dir.path().join("controle.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(server);
        assert!(!dir.path().join("controle.json").exists());
        assert!(start(dir.path()).is_ok());
    }

    #[test]
    fn timeout_reports_uncertainty_and_request_expiration() {
        let dir = tempfile::tempdir().unwrap();
        let (_server, requests) = start(dir.path()).unwrap();
        let response = call(dir.path(), Command::Status, Duration::from_millis(30)).unwrap();
        assert_eq!(response["ok"], false);
        assert!(response["error"]
            .as_str()
            .unwrap()
            .contains("query status before retrying"));
        let request = requests.recv().unwrap();
        assert!(request.expires <= Instant::now());
    }

    #[test]
    fn rejects_nonlocal_or_incompatible_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = Endpoint {
            version: VERSION,
            address: "192.0.2.1:1234".parse().unwrap(),
            token: "test".into(),
            pid: 0,
        };
        fs::write(
            dir.path().join("controle.json"),
            serde_json::to_vec(&endpoint).unwrap(),
        )
        .unwrap();
        assert!(call(dir.path(), Command::Status, Duration::from_millis(10))
            .unwrap_err()
            .to_string()
            .contains("invalid local endpoint"));
    }

    #[test]
    fn stale_endpoint_explains_that_the_instance_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = Endpoint {
            version: VERSION,
            address: listener.local_addr().unwrap(),
            token: "test".into(),
            pid: 0,
        };
        drop(listener);
        fs::write(
            dir.path().join("controle.json"),
            serde_json::to_vec(&endpoint).unwrap(),
        )
        .unwrap();
        let error = call(dir.path(), Command::Status, Duration::from_millis(100)).unwrap_err();
        assert!(error.to_string().contains("controller unavailable"));
    }
}
