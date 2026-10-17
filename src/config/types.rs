//! Configuration data model split by serialized responsibility.
//!
//! Each private submodule owns one stable group of existing YAML fields while
//! this facade preserves the public crate configuration surface.

use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;

use super::defaults::*;

// Per-user access credentials and optional WEB ceilings.
mod access;
mod api;
mod general;
mod general_impl;
mod logging;
mod metrics;
mod network;
mod server;
mod web;
// WEB carrier tokens and fixed-slot policy helpers remain independent from bulky config types.
mod web_carrier;
// WEB debug capture policy stays independent from the bulky config types.
mod web_debug;

pub use access::UserEntry;
pub use api::{ApiConfig, ApiGrayAction};
pub use general::GeneralConfig;
pub use logging::{LogLevel, LoggingConfig, LoggingDestination};
pub use metrics::MetricsConfig;
pub use network::{UpstreamConfig, UpstreamType};
#[allow(unused_imports)]
pub use server::{ListenerConfig, ListenerEndpoint, WebClientIpSource};
#[allow(unused_imports)]
pub use web::{
    WebCarrierNegotiationAggressiveness, WebConfig, WebFallbackFastTrackMode, WebFallbackResolve,
    WebHttpConnectionCapacityAction, WebLimitsConfig, WebSecretMode, WebTimeoutsConfig,
    WebVhostConfig,
};
pub(crate) use web::{
    FallbackEndpoint, WebFallbackDnsSnapshot, WebRuntimeConfig, WebRuntimeFallback, WebRuntimeProfile,
    WebRuntimeVhost,
};
pub(crate) use web_carrier::WEB_CARRIER_LEARNING_MIN_ENTRIES;
#[allow(unused_imports)]
pub use web_carrier::{WebCarrier, WebCarrierMethod, WebCarriers};
pub use web_debug::{WebDebugBodyCapture, WebDebugConfig};
