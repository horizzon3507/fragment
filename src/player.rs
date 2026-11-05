use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// Plays snippets through a persistent `mpv --idle` process driven over its
/// JSON IPC socket. `play_snippet` loads the file, seeks to `offset`,
/// unpauses, and pauses again once `length` has elapsed. Dropping the player
/// quits mpv.
pub struct Player {
    child: Child,
    ipc: Arc<Mutex<UnixStream>>,
    generation: Arc<AtomicU64>,
    socket_path: PathBuf,
}

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

        // mpv pushes replies and events on the socket; drain them so the
        // socket buffer never fills. The thread exits on error or EOF.
        let mut drain = match stream.try_clone() {
            Ok(drain) => drain,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&socket_path);
                return Err(e).context("mpv IPC stream clone");
            }
        };
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while drain.read(&mut buf).is_ok_and(|n| n > 0) {}
        });

        Ok(Self {
            child,
            ipc: Arc::new(Mutex::new(stream)),
            generation: Arc::new(AtomicU64::new(0)),
            socket_path,
        })
    }

    /// Play `length` seconds of `path` starting at `offset`.
    pub fn play_snippet(&self, path: &Path, offset: Duration, length: Duration) -> Result<()> {
        let path = path.to_str().context("snippet path is not valid UTF-8")?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        {
            let mut stream = self.ipc.lock().expect("mpv command mutex poisoned");
            // Pass the offset to loadfile itself so the start lands before
            // the file is open; the seek after it is a safety net for older
            // mpv versions that ignore the options object.
            send_locked(
                &mut stream,
                json!({
                    "command": [
                        "loadfile",
                        path,
                        "replace",
                        -1,
                        {"start": offset.as_secs_f64()},
                    ]
                }),
            )?;
            send_locked(
                &mut stream,
                json!({"command": ["seek", offset.as_secs_f64(), "absolute"]}),
            )?;
            send_locked(&mut stream, json!({"command": ["set_property", "pause", false]}))?;
        }

        // Pause after `length` unless a newer play/stop superseded this call.
        // The generation check happens under the lock so the pause cannot
        // interleave with a newer call's command sequence.
        let ipc = Arc::clone(&self.ipc);
        let generations = Arc::clone(&self.generation);
        thread::spawn(move || {
            thread::sleep(length);
            let mut stream = ipc.lock().expect("mpv command mutex poisoned");
            if generations.load(Ordering::SeqCst) == generation {
                let _ = send_locked(
                    &mut stream,
                    json!({"command": ["set_property", "pause", true]}),
                );
            }
        });
        Ok(())
    }

    /// Pause playback and cancel any pending pause timer.
    pub fn stop(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let mut stream = self.ipc.lock().expect("mpv command mutex poisoned");
        send_locked(&mut stream, json!({"command": ["set_property", "pause", true]}))
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Ok(mut stream) = self.ipc.lock() {
            let _ = send_locked(&mut stream, json!({"command": ["quit"]}));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn send_locked(stream: &mut UnixStream, command: Value) -> Result<()> {
    let mut line = serde_json::to_vec(&command).context("mpv IPC encode")?;
    line.push(b'\n');
    stream.write_all(&line).context("mpv IPC write")
}
