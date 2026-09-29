//! Transport layer: connection pooling, socket utilities, proxy protocol

pub mod pool;
pub mod socket;
pub mod socks;
pub mod upstream;

#[allow(unused_imports)]
pub use pool::ConnectionPool;
#[allow(unused_imports)]
pub use socket::*;
#[allow(unused_imports)]
pub use socks::*;
#[allow(unused_imports)]
pub use upstream::{
    DcPingResult, StartupPingResult, UpstreamEgressInfo, UpstreamManager, UpstreamRouteKind,
    UpstreamStream,
};
