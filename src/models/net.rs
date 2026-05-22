use serde::{Deserialize, Serialize};

use crate::models::doctor::Severity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetReport {
    pub checks: Vec<NetCheck>,
    pub proxy: ProxyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetCheck {
    pub target: String,
    pub dns_ok: bool,
    pub http_ok: bool,
    pub latency_ms: Option<u64>,
    pub status: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
}
