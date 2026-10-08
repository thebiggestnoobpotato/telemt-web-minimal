use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::os::fd::AsFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::Path;
use std::sync::Arc;

use socket2::{Domain, Socket, Type};
use tokio::net::{TcpListener, UnixListener};
use tracing::{error, info, warn};

use crate::config::{ListenerEndpoint, ProxyConfig};
use crate::startup::{COMPONENT_LISTENERS_BIND, StartupTracker};
use crate::transport::find_listener_processes;
use crate::transport::socket::{activate_listener_socket, bind_listener_socket};
use crate::util::secure_fs::AnchoredPath;

use super::plan::{ListenerBindSpec, listener_bind_plan};
use crate::maestro::helpers::print_web_proxy_links;

/// Owns sockets bound before process accept loops start.
pub(crate) struct BoundListeners {
    pub(super) listeners: Vec<BoundListener>,
}

impl BoundListeners {
    pub(crate) fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

/// Active listener socket for one endpoint.
#[derive(Clone)]
pub(super) enum ListenerHandle {
    Tcp(Arc<TcpListener>),
    Unix(Arc<UnixListener>),
}

/// Active socket and immutable connection policy for one endpoint.
pub(super) struct BoundListener {
    pub(super) handle: ListenerHandle,
    pub(super) spec: ListenerBindSpec,
}

/// Socket bound for a candidate transition but not yet listening.
pub(super) struct PreparedListener {
    socket: Socket,
    spec: ListenerBindSpec,
}

fn log_bind_error(spec: &ListenerBindSpec, error_value: &std::io::Error) {
    let endpoint = &spec.endpoint;
    if error_value.kind() == ErrorKind::AddrInUse {
        if let ListenerEndpoint::Tcp(addr) = endpoint {
            let owners = find_listener_processes(*addr);
            if owners.is_empty() {
                error!(
                    endpoint = %endpoint,
                    "Failed to bind: address already in use (owner process unresolved)"
                );
            } else {
                for owner in owners {
                    error!(
                        endpoint = %endpoint,
                        pid = owner.pid,
                        process = %owner.process,
                        "Failed to bind: address already in use"
                    );
                }
            }
        } else {
            error!(endpoint = %endpoint, "Failed to bind: address already in use");
        }
    } else {
        error!(endpoint = %endpoint, error = %error_value, "Failed to bind listener");
    }
}

fn prepare_unix_listener(path: &Path, spec: &ListenerBindSpec) -> std::io::Result<Socket> {
    let anchored_path = AnchoredPath::open_trusted_parent(path)?;
    remove_stale_unix_socket(path)?;
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.set_nonblocking(true)?;
    let sock_addr = socket2::SockAddr::unix(path)?;
    socket.bind(&sock_addr)?;
    apply_unix_socket_permissions(&anchored_path, path, spec.socket_perm.as_deref())?;
    Ok(socket)
}

pub(super) fn prepare_listener(spec: ListenerBindSpec) -> std::io::Result<PreparedListener> {
    let result = match &spec.endpoint {
        ListenerEndpoint::Tcp(addr) => bind_listener_socket(*addr, &spec.options),
        ListenerEndpoint::Unix(path) => prepare_unix_listener(path, &spec),
    };
    match result {
        Ok(socket) => Ok(PreparedListener { socket, spec }),
        Err(error_value) => {
            log_bind_error(&spec, &error_value);
            Err(error_value)
        }
    }
}

impl PreparedListener {
    pub(super) fn activate(self) -> std::io::Result<BoundListener> {
        activate_listener_socket(&self.socket, self.spec.options.backlog)?;
        let handle = match &self.spec.endpoint {
            ListenerEndpoint::Tcp(_) => {
                let listener = TcpListener::from_std(self.socket.into())?;
                ListenerHandle::Tcp(Arc::new(listener))
            }
            ListenerEndpoint::Unix(_) => {
                let listener = UnixListener::from_std(self.socket.into())?;
                ListenerHandle::Unix(Arc::new(listener))
            }
        };
        Ok(BoundListener {
            handle,
            spec: self.spec,
        })
    }
}

/// Best-effort removal of a unix socket file we still own.
///
/// The inode check prevents deleting a file a replacement process already
/// bound during a rapid restart.
pub(super) fn remove_unix_listener_file(path: &Path, listener: &UnixListener) {
    let listener_identity = match nix::sys::stat::fstat(listener.as_fd()) {
        Ok(metadata) => (metadata.st_dev as u64, metadata.st_ino as u64),
        Err(_) => return,
    };
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };
    if !metadata.file_type().is_socket() || (metadata.dev(), metadata.ino()) != listener_identity {
        return;
    }
    let _ = std::fs::remove_file(path);
}

fn apply_unix_socket_permissions(
    anchored_path: &AnchoredPath,
    path: &Path,
    perm: Option<&str>,
) -> std::io::Result<()> {
    let Some(perm) = perm else {
        return Ok(());
    };
    match u32::from_str_radix(perm.trim_start_matches('0'), 8) {
        Ok(mode) => {
            use nix::sys::stat::{FchmodatFlags, Mode, fchmodat};

            let socket_metadata = std::fs::symlink_metadata(path)?;
            if !socket_metadata.file_type().is_socket() {
                return Err(IoError::new(
                    ErrorKind::AlreadyExists,
                    format!("Unix listener path {} was replaced", path.display()),
                ));
            }
            let socket_identity = unix_path_identity(&socket_metadata);
            let result = fchmodat(
                anchored_path.parent(),
                anchored_path.name(),
                Mode::from_bits_truncate(mode),
                FchmodatFlags::NoFollowSymlink,
            );
            if let Err(error_value) = result {
                warn!(
                    path = %path.display(),
                    permissions = %perm,
                    error = %error_value,
                    "Failed to set Unix socket permissions"
                );
                return Ok(());
            }
            verify_bound_unix_socket(path, socket_identity)?;
            info!(
                path = %path.display(),
                permissions = %perm,
                "Unix socket permissions applied"
            );
        }
        Err(error_value) => {
            warn!(
                path = %path.display(),
                permissions = %perm,
                error = %error_value,
                "Invalid Unix socket permissions; keeping umask-derived mode"
            );
        }
    }
    Ok(())
}

fn unix_path_identity(metadata: &std::fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn remove_stale_unix_socket(path: &Path) -> std::io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Err(IoError::new(
            ErrorKind::AlreadyExists,
            format!(
                "refusing to remove non-socket Unix listener path {}",
                path.display()
            ),
        ));
    }
    match StdUnixStream::connect(path) {
        Ok(_) => {
            return Err(IoError::new(
                ErrorKind::AddrInUse,
                format!("Unix listener {} is already active", path.display()),
            ));
        }
        Err(error) if error.kind() == ErrorKind::ConnectionRefused => {}
        Err(error) => return Err(error),
    }
    let current = std::fs::symlink_metadata(path)?;
    if !current.file_type().is_socket()
        || (current.dev(), current.ino()) != (metadata.dev(), metadata.ino())
    {
        return Err(IoError::new(
            ErrorKind::AlreadyExists,
            format!("Unix listener path {} changed during cleanup", path.display()),
        ));
    }
    std::fs::remove_file(path)
}

fn verify_bound_unix_socket(path: &Path, expected: (u64, u64)) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_socket() && (metadata.dev(), metadata.ino()) == expected {
        return Ok(());
    }
    Err(IoError::new(
        ErrorKind::AlreadyExists,
        format!("Unix listener path {} was replaced", path.display()),
    ))
}

fn log_listener_profile(spec: &ListenerBindSpec) {
    info!(
        endpoint = %spec.endpoint,
        transport = ?spec.transport,
        "Listening on listener endpoint"
    );
}

/// Binds every eligible configured listener or fails without a partial inventory.
pub(crate) async fn bind_listeners(
    config: &Arc<ProxyConfig>,
    startup_tracker: &Arc<StartupTracker>,
) -> Result<BoundListeners, Box<dyn Error>> {
    startup_tracker
        .start_component(
            COMPONENT_LISTENERS_BIND,
            Some("bind TCP/Unix listeners".to_string()),
        )
        .await;
    let plan = listener_bind_plan(config).map_err(std::io::Error::other)?;
    let mut prepared = Vec::with_capacity(plan.len());
    for spec in plan.values().cloned() {
        prepared.push(prepare_listener(spec)?);
    }
    let mut listeners = Vec::with_capacity(prepared.len());
    for candidate in prepared {
        let bound = candidate.activate()?;
        log_listener_profile(&bound.spec);
        listeners.push(bound);
    }
    print_web_proxy_links(config);

    let unix_count = listeners
        .iter()
        .filter(|listener| matches!(listener.handle, ListenerHandle::Unix(_)))
        .count();
    startup_tracker
        .complete_component(
            COMPONENT_LISTENERS_BIND,
            Some(format!(
                "listeners configured tcp={} unix={}",
                listeners.len() - unix_count,
                unix_count
            )),
        )
        .await;

    Ok(BoundListeners { listeners })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener as StdUnixListener;

    use super::*;

    #[test]
    fn unix_socket_cleanup_refuses_regular_file_and_symlink() {
        let directory = tempfile::tempdir().unwrap();
        let regular = directory.path().join("regular");
        let link = directory.path().join("listener.sock");
        std::fs::write(&regular, b"preserve").unwrap();
        symlink(&regular, &link).unwrap();

        assert!(remove_stale_unix_socket(&regular).is_err());
        assert!(remove_stale_unix_socket(&link).is_err());
        assert_eq!(std::fs::read(&regular).unwrap(), b"preserve");
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn unix_socket_cleanup_removes_only_stale_socket() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("listener.sock");
        let listener = StdUnixListener::bind(&path).unwrap();
        drop(listener);

        remove_stale_unix_socket(&path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn unix_socket_cleanup_preserves_live_listener() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("listener.sock");
        let _listener = StdUnixListener::bind(&path).unwrap();

        let error = remove_stale_unix_socket(&path).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::AddrInUse);
        assert!(path.exists());
    }
}
