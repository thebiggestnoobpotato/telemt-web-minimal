//! Client listener planning, binding, lifecycle control, and accept loops.
//!
//! Submodules keep process-owned socket state separate from generation-owned
//! runtime state:
//! - `plan` derives deterministic bind intent from validated configuration.
//! - `bind` prepares and activates sockets without partial startup binding.
//! - `accept` runs cancellation-aware TCP accept loops.
//! - `control` coordinates reversible listener transitions and shutdown.
//! - `web_overload` handles accepted WEB sockets outside ordinary capacity.

mod accept;
mod bind;
mod control;
mod plan;
mod web_overload;

pub(crate) use bind::bind_listeners;
pub(crate) use control::{ListenerManager, PreparedListenerTransition};
pub(crate) use plan::listener_rebind_supported;
