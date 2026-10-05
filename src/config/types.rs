//! Configuration data model split by serialized responsibility.
//!
//! Each private submodule owns one stable group of existing TOML fields while
//! this facade preserves the public crate configuration surface.

use chrono::{DateTime, Utc};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;

use super::defaults::*;

mod access;
mod api;
mod general;
mod general_impl;
mod links;
mod logging;
mod metrics;
mod network;
mod policies;
mod server;
mod web;
// WEB carrier tokens and fixed-slot policy helpers remain independent from bulky config types.
mod web_carrier;
// WEB debug capture policy is reusable by config reload and process storage.
mod web_debug;

pub use access::{AccessConfig, CidrRateLimitKey, RateLimitBps};
#[allow(unused_imports)]
pub(crate) use access::{CidrAutoTemplate, CidrAutoTemplateFamily, MAX_RATE_LIMIT_BPS};
pub use api::{ApiConfig, ApiGrayAction};
pub use general::GeneralConfig;
#[allow(unused_imports)]
pub use links::ShowLink;
pub use logging::{LogLevel, LoggingConfig, LoggingDestination};
pub use metrics::MetricsConfig;
pub use network::{UpstreamConfig, UpstreamType};
pub use policies::UserMaxUniqueIpsMode;
#[allow(unused_imports)]
pub use server::{
    ListenerConfig, ListenerTransport, ServerConfig, TimeoutsConfig, WebClientIpSource,
};
#[allow(unused_imports)]
pub use web::{
    WebCarrierNegotiationAggressiveness, WebConfig, WebDecoyConfig, WebDecoyFastTrackMode,
    WebHttpConnectionCapacityAction, WebLimitsConfig, WebProfileConfig, WebSecretMode,
    WebTimeoutsConfig, WebVhostConfig,
};
pub(crate) use web::{
    WebRuntimeConfig, WebRuntimeDecoy, WebRuntimeProfile, WebRuntimeVhost, WebStaticAsset,
    WebStaticSite,
};
pub(crate) use web_carrier::WEB_CARRIER_LEARNING_MIN_ENTRIES;
#[allow(unused_imports)]
pub use web_carrier::{WebCarrier, WebCarrierMethod, WebCarriers};
pub(crate) use web_debug::web_debug_fits_limits;
pub use web_debug::{WebDebugBodyCapture, WebDebugConfig};

fn default_quota_state_path() -> PathBuf {
    PathBuf::from("telemt.limit.json")
}
