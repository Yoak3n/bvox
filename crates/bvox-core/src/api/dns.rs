//! DNS resolution: prefer IPv4 and put a reachable CDN address first.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use tokio::sync::Mutex;

/// Cached DNS answers: the address list plus the time it was fetched.
type ResolverCache = HashMap<String, (Vec<SocketAddr>, Instant)>;

/// Prefer IPv4 and put a reachable address first. Some Bilibili CDN nodes
/// publish addresses that accept DNS but silently drop connections, and
/// reqwest does not perform Happy Eyeballs fallback by itself.
#[derive(Debug)]
pub(super) struct Ipv4FirstResolver {
    cache: Arc<Mutex<ResolverCache>>,
}

impl Ipv4FirstResolver {
    pub(super) fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) async fn forget(&self, host: &str) {
        self.cache.lock().await.remove(host);
    }
}

impl Resolve for Ipv4FirstResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let cache = self.cache.clone();
        Box::pin(async move {
            let host = name.as_str().to_string();

            {
                let cached = cache.lock().await;
                if let Some((addrs, fetched_at)) = cached.get(&host)
                    && fetched_at.elapsed() < Duration::from_secs(300)
                {
                    return Ok(Box::new(addrs.clone().into_iter()) as Addrs);
                }
            }

            let mut addrs = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?
                .collect::<Vec<SocketAddr>>();
            let ipv4 = addrs
                .iter()
                .copied()
                .filter(SocketAddr::is_ipv4)
                .collect::<Vec<_>>();
            if !ipv4.is_empty() {
                addrs = ipv4;
            }

            if addrs.len() > 1
                && let Some(reachable) = first_reachable(&host, &addrs).await
            {
                addrs.retain(|addr| *addr != reachable);
                addrs.insert(0, reachable);
            }

            cache
                .lock()
                .await
                .insert(host, (addrs.clone(), Instant::now()));
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// Some CDN nodes accept TCP but never finish TLS/HTTP. Probe with the same
/// reqwest/rustls stack the real request will use and return the first working
/// address.
pub(super) async fn first_reachable(host: &str, addrs: &[SocketAddr]) -> Option<SocketAddr> {
    let mut tasks = tokio::task::JoinSet::new();
    for addr in addrs.iter().copied() {
        let host = host.to_string();
        tasks.spawn(async move {
            let pinned = SocketAddr::new(addr.ip(), 443);
            let reachable = async {
                let client = reqwest::Client::builder()
                    .resolve(&host, pinned)
                    .timeout(Duration::from_secs(2))
                    .build()
                    .ok()?;
                let url = format!("https://{host}/");
                client.head(url).send().await.ok().map(|_| ())
            }
            .await
            .is_some();
            (addr, reachable)
        });
    }

    while let Some(result) = tasks.join_next().await {
        if let Ok((addr, true)) = result {
            return Some(addr);
        }
    }
    None
}
