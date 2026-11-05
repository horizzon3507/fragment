use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Plays snippets through a persistent `mpv --idle` process driven over its
/// JSON IPC socket. `play_snippet` loads the file paused, waits for mpv's
/// `file-loaded` event, seeks to `offset`, unpauses, and pauses again once
/// `length` has elapsed. Commands carry `request_id`s and their replies are
/// checked, so a rejected command (e.g. an unloadable file) surfaces as an
/// error instead of failing silently. Dropping the player quits mpv.
pub struct Player {
    child: Child,
    ipc: Arc<Ipc>,
    generation: Arc<AtomicU64>,
    socket_path: PathBuf,
}

/// Shared IPC plumbing. The reader thread owns the read half of the
/// socket: replies are routed to `pending` by `request_id`, everything
/// else lands in `events`.
struct Ipc {
    writer: Mutex<UnixStream>,
    pending: Arc<Mutex<HashMap<u64, Sender<Value>>>>,
    events: Mutex<Receiver<Value>>,
    next_id: AtomicU64,
}

/// How long a command may wait for mpv's reply, and how long mpv may take
/// to open a file, before playback is reported as broken.
const IPC_TIMEOUT: Duration = Duration::from_secs(3);
const LOAD_TIMEOUT: Duration = Duration::from_secs(4);

impl Player {
    /// Spawn mpv (`--idle --no-video --really-quiet --input-ipc-server=…`).
    /// Errors cleanly when the `mpv` binary is not installed.
    pub fn spawn() -> Result<Self> {
        let socket_path = std::env::temp_dir().join(format!(
            "fragment-mpv-{}-{:016x}.sock",
            std::process::id(),
            rand::random::<u64>()
        ));

        let mut child = Command::new("mpv")
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--really-quiet")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                if e.kind() == ErrorKind::NotFound {
                    anyhow::anyhow!("mpv is not installed — install mpv to play snippets")
                } else {
                    anyhow::Error::new(e).context("failed to spawn mpv")
                }
            })?;

        // mpv creates the socket asynchronously; connect-retries also cover
        // the window where the file exists but mpv is not listening yet.
        let deadline = Instant::now() + Duration::from_secs(2);
        let stream = loop {
            match UnixStream::connect(&socket_path) {
                Ok(stream) => break stream,
                Err(e) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&socket_path);
                    return Err(e).context("mpv IPC connect");
                }
                Err(_) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        let _ = std::fs::remove_file(&socket_path);
                        anyhow::bail!("mpv exited before creating its IPC socket ({status})");
                    }
                    thread::sleep(Duration::from_millis(15));
                }
            }
        };

        // A wedged mpv must not hang a command write behind the mutex.
        if let Err(e) = stream.set_write_timeout(Some(Duration::from_secs(1))) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&socket_path);
            return Err(e).context("mpv IPC write timeout");
        }

        let mut reader = match stream.try_clone() {
            Ok(reader) => reader,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&socket_path);
                return Err(e).context("mpv IPC stream clone");
            }
        };

        let (events_tx, events_rx) = channel();
        let pending: Arc<Mutex<HashMap<u64, Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        thread::spawn({
            let pending = Arc::clone(&pending);
            move || read_loop(&mut reader, &pending, &events_tx)
        });

        Ok(Self {
            child,
            ipc: Arc::new(Ipc {
                writer: Mutex::new(stream),
                pending,
                events: Mutex::new(events_rx),
                next_id: AtomicU64::new(1),
            }),
            generation: Arc::new(AtomicU64::new(0)),
            socket_path,
        })
    }

    /// Play `length` seconds of `path` starting at `offset`.
    ///
    /// The file is loaded while paused so nothing from position zero is
    /// audible; mpv's `file-loaded` event gates the seek, because a seek
    /// sent while the file is still opening is silently dropped.
    pub fn play_snippet(&self, path: &Path, offset: Duration, length: Duration) -> Result<()> {
        let path = path.to_str().context("snippet path is not valid UTF-8")?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        self.set("pause", true)?;
        // Stale events from a replaced load must not satisfy the wait below.
        self.drain_events();
        self.command(json!({"command": ["loadfile", path, "replace"]}))?;
        self.wait_file_loaded(path)?;
        self.command(json!({"command": ["seek", offset.as_secs_f64(), "absolute"]}))?;
        self.set("pause", false)?;

        // Pause after `length` unless a newer play/stop superseded this call.
        // The generation check happens under the writer lock so the pause
        // cannot interleave with a newer call's command sequence; the write
        // is fire-and-forget — a reply without a request_id is discarded.
        let ipc = Arc::clone(&self.ipc);
        let generations = Arc::clone(&self.generation);
        thread::spawn(move || {
            thread::sleep(length);
            let mut stream = ipc.writer.lock().unwrap_or_else(|e| e.into_inner());
            if generations.load(Ordering::SeqCst) == generation {
                let mut line =
                    serde_json::to_vec(&json!({"command": ["set_property", "pause", true]}))
                        .unwrap_or_default();
                line.push(b'\n');
                let _ = stream.write_all(&line);
            }
        });
        Ok(())
    }

    /// Pause playback and cancel any pending pause timer.
    pub fn stop(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.set("pause", true)
    }

    /// Send a command and wait for its reply, surfacing mpv's error string
    /// instead of ignoring it.
    fn command(&self, mut command: Value) -> Result<Value> {
        let id = self.ipc.next_id.fetch_add(1, Ordering::SeqCst);
        command["request_id"] = json!(id);
        let result = send_command(&self.ipc, command);
        if result.is_err() {
            self.pending().remove(&id);
        }
        result
    }

    fn set(&self, name: &str, value: impl Into<Value>) -> Result<()> {
        self.command(json!({"command": ["set_property", name, value.into()]}))?;
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Value> {
        Ok(self
            .command(json!({"command": ["get_property", name]}))?
            .get("data")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Block until mpv emits `file-loaded` for `path`. A stale
    /// `file-loaded` from a superseded load is rejected by comparing the
    /// `path` property.
    fn wait_file_loaded(&self, path: &str) -> Result<()> {
        let deadline = Instant::now() + LOAD_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = if remaining.is_zero() {
                Err(RecvTimeoutError::Timeout)
            } else {
                let rx = self.ipc.events.lock().unwrap_or_else(|e| e.into_inner());
                rx.recv_timeout(remaining)
            };
            match event {
                Ok(v) => {
                    if v.get("event").and_then(Value::as_str) == Some("file-loaded")
                        && self.get("path").ok().as_ref().and_then(Value::as_str) == Some(path)
                    {
                        return Ok(());
                    }
                }
                Err(RecvTimeoutError::Timeout) => bail!("mpv did not load {path}"),
                Err(RecvTimeoutError::Disconnected) => bail!("mpv exited during load"),
            }
        }
    }

    /// Discard queued events so the next `file-loaded` wait only sees what
    /// belongs to the upcoming load.
    fn drain_events(&self) {
        let rx = self.ipc.events.lock().unwrap_or_else(|e| e.into_inner());
        while rx.try_recv().is_ok() {}
    }

    fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Sender<Value>>> {
        self.ipc.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = send_command(&self.ipc, json!({"command": ["quit"]}));
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Write a command with a registered `request_id` and wait for its reply.
/// The pending entry is inserted before the write so a fast reply can
/// never arrive before the waiter exists.
fn send_command(ipc: &Ipc, command: Value) -> Result<Value> {
    let id = command["request_id"].as_u64().context("request_id missing")?;
    let (tx, rx) = channel();
    ipc.pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, tx);
    {
        let mut stream = ipc.writer.lock().unwrap_or_else(|e| e.into_inner());
        let mut line = serde_json::to_vec(&command).context("mpv IPC encode")?;
        line.push(b'\n');
        stream.write_all(&line).context("mpv IPC write")?;
    }
    match rx.recv_timeout(IPC_TIMEOUT) {
        Ok(reply) => match reply.get("error").and_then(Value::as_str) {
            Some("success") => Ok(reply),
            Some(error) => bail!("mpv: {error}"),
            None => bail!("mpv: malformed reply"),
        },
        Err(RecvTimeoutError::Timeout) => bail!("mpv IPC timed out"),
        Err(RecvTimeoutError::Disconnected) => bail!("mpv IPC closed"),
    }
}

/// Reader thread: parse JSON lines, route replies by `request_id` to the
/// waiting command, forward everything else as an event. On exit, pending
/// senders are dropped so blocked commands wake up with `Disconnected`.
fn read_loop(
    reader: &mut UnixStream,
    pending: &Mutex<HashMap<u64, Sender<Value>>>,
    events: &Sender<Value>,
) {
    let mut buf: Vec<u8> = Vec::new();
    let mut raw = [0u8; 8192];
    loop {
        match reader.read(&mut raw) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.extend_from_slice(&raw[..n]);
                while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=pos).collect();
                    let Ok(v) = serde_json::from_slice::<Value>(&line) else {
                        continue;
                    };
                    if let Some(id) = v.get("request_id").and_then(Value::as_u64) {
                        if let Some(tx) =
                            pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id)
                        {
                            let _ = tx.send(v);
                        }
                    } else if v.get("event").is_some() {
                        let _ = events.send(v);
                    }
                }
            }
        }
    }
    pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
