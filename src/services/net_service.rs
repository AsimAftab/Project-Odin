use anyhow::Result;
use reqwest::Client;
use std::env;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use crate::models::doctor::Severity;
use crate::models::net::{NetCheck, NetReport, ProxyConfig};

pub struct NetService;

impl NetService {
    pub async fn run(custom_targets: Option<Vec<String>>) -> Result<NetReport> {
        let targets = custom_targets.unwrap_or_else(|| {
            vec![
                "github.com".to_string(),
                "registry.npmjs.org".to_string(),
                "registry-1.docker.io".to_string(),
                "api.nuget.org".to_string(),
                "pypi.org".to_string(),
            ]
        });

        let proxy = ProxyConfig {
            http_proxy: env::var("HTTP_PROXY")
                .or_else(|_| env::var("http_proxy"))
                .ok(),
            https_proxy: env::var("HTTPS_PROXY")
                .or_else(|_| env::var("https_proxy"))
                .ok(),
            no_proxy: env::var("NO_PROXY").or_else(|_| env::var("no_proxy")).ok(),
        };

        // If environment has a proxy, reqwest will automatically use it,
        // but we show the proxy configuration in the report for clarity.
        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

        let mut checks = Vec::new();

        for target in targets {
            let mut dns_ok = false;
            let mut http_ok = false;
            let mut latency_ms = None;
            let severity;

            // DNS check: we append :443 just to use ToSocketAddrs
            let dns_query = format!("{}:443", target);
            if let Ok(mut addrs) = dns_query.to_socket_addrs() {
                if addrs.next().is_some() {
                    dns_ok = true;
                }
            }

            if dns_ok {
                let url = format!("https://{}", target);
                let start = Instant::now();
                match client.head(&url).send().await {
                    Ok(response) => {
                        let latency = start.elapsed().as_millis() as u64;
                        latency_ms = Some(latency);
                        if response.status().is_success() || response.status().is_redirection() {
                            http_ok = true;
                            severity = if latency < 500 {
                                Severity::Info
                            } else {
                                Severity::Warning
                            };
                        } else {
                            // HTTP Error, but reached server
                            http_ok = false;
                            severity = Severity::Warning;
                        }
                    }
                    Err(_) => {
                        http_ok = false;
                        severity = Severity::Error;
                    }
                }
            } else {
                severity = Severity::Error;
            }

            checks.push(NetCheck {
                target,
                dns_ok,
                http_ok,
                latency_ms,
                status: severity,
            });
        }

        Ok(NetReport { checks, proxy })
    }
}
