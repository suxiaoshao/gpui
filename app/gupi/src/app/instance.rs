//! One process owns a configuration directory. The lock lives for the process;
//! a private loopback endpoint only asks its owner to show the main window.
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

pub(crate) struct Instance {
    _lock: File,
    listener: TcpListener,
    token: String,
}
struct Owner {
    _lock: File,
    _listener: Task<()>,
}
impl Global for Owner {}
#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: String,
}
impl Instance {
    /// None means the existing owner acknowledged the request.
    pub fn acquire(directory: &Path) -> io::Result<Option<Self>> {
        std::fs::create_dir_all(directory)?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // Never unlink this file: that could give another process a different lock inode.
        let lock = options.open(directory.join("instance.lock"))?;
        match lock.try_lock() {
            Ok(()) => {
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
                let token = uuid::Uuid::new_v4().to_string();
                let endpoint = Endpoint {
                    port: listener.local_addr()?.port(),
                    token: token.clone(),
                };
                let mut file = tempfile::NamedTempFile::new_in(directory)?;
                serde_json::to_writer(&mut file, &endpoint)?;
                file.flush()?;
                file.persist(directory.join("instance.json"))
                    .map_err(|error| error.error)?;
                Ok(Some(Self {
                    _lock: lock,
                    listener,
                    token,
                }))
            }
            Err(TryLockError::Error(error)) => Err(error),
            Err(TryLockError::WouldBlock) => {
                // Startup can hold the lock before it has published its listening port.
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    match notify(directory) {
                        Ok(()) => return Ok(None),
                        Err(error) if Instant::now() >= deadline => return Err(error),
                        Err(_) => std::thread::sleep(Duration::from_millis(50)),
                    }
                }
            }
        }
    }
    pub fn listen(self, cx: &mut App) -> io::Result<()> {
        use smol::io::{AsyncReadExt, AsyncWriteExt};
        let listener = smol::Async::new(self.listener)?;
        let token = self.token;
        let lock = self._lock;
        let task = cx.spawn(async move |cx| {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(client) => client,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        tracing::error!(%error, "instance wake listener failed");
                        break;
                    }
                };
                let mut request = vec![0; token.len()];
                let read =
                    smol::future::or(async { stream.read_exact(&mut request).await }, async {
                        smol::Timer::after(Duration::from_secs(1)).await;
                        Err(io::Error::from(io::ErrorKind::TimedOut))
                    })
                    .await;
                if read.is_ok() && request == token.as_bytes() {
                    cx.update(|cx| cx.defer(|cx| super::show(Some(false), cx)));
                    let _ = stream.write_all(b"ok").await;
                }
            }
        });
        // Ownership lasts for the entire app, including a failed wake listener.
        cx.set_global(Owner {
            _lock: lock,
            _listener: task,
        });
        Ok(())
    }
}
fn notify(directory: &Path) -> io::Result<()> {
    let endpoint: Endpoint = serde_json::from_reader(File::open(directory.join("instance.json"))?)?;
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, endpoint.port).into();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))?;
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(Duration::from_millis(250)))?;
    stream.write_all(endpoint.token.as_bytes())?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply)?;
    if reply == *b"ok" {
        Ok(())
    } else {
        Err(io::Error::other("invalid instance response"))
    }
}

#[cfg(test)]
mod tests {
    use super::Instance;
    use std::io::{Read, Write};
    #[test]
    fn second_launch_reuses_owner_and_released_lock_can_be_reacquired() {
        let directory = tempfile::tempdir().unwrap();
        let owner = Instance::acquire(directory.path()).unwrap().unwrap();
        let token = owner.token.clone();
        let socket = owner.listener.try_clone().unwrap();
        let accept = std::thread::spawn(move || {
            let (mut client, _) = socket.accept().unwrap();
            let mut request = vec![0; token.len()];
            client.read_exact(&mut request).unwrap();
            assert_eq!(request, token.as_bytes());
            client.write_all(b"ok").unwrap();
        });
        assert!(Instance::acquire(directory.path()).unwrap().is_none());
        accept.join().unwrap();
        let other = tempfile::tempdir().unwrap();
        assert!(Instance::acquire(other.path()).unwrap().is_some());
        drop(owner);
        assert!(Instance::acquire(directory.path()).unwrap().is_some());
    }
}
