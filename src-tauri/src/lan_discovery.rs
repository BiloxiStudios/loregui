//! LAN auto-discovery of lore servers (SBAI-4073).
//!
//! Open-core, MIT, **not gated** — this is just dynamic mDNS/zeroconf discovery,
//! the same pattern `studiobrain-model-manager` uses for its gateway cluster
//! (`mdns-sd` + `_gateway._tcp.local.`). Here we advertise the **lore service**
//! type `_lore._tcp.local.` so any LoreGUI on the same LAN can find a hosted
//! server without anyone copying a `lore://` URL around by hand.
//!
//! Two halves, mirroring the model-manager design:
//!
//! - [`Announcer`] (host side): when LoreGUI hosts a server (`server_host.rs`,
//!   SBAI-4065) we register one mDNS service carrying TXT records for the repo
//!   name, the `lore://host:port/<repo>` connect URL, and a friendly instance
//!   name. The registration lives for as long as the `Announcer` is held; it is
//!   dropped (and the service unregistered) when the hosted server stops.
//! - [`Browser`] (client side): a background browse task drains
//!   `ServiceDaemon::browse`'s channel, resolving `ServiceResolved` /
//!   `ServiceRemoved` events into a live [`DiscoveredServer`] list. The Tauri
//!   layer reads a snapshot (`lan_discover_browse`) and/or subscribes to the
//!   `lan/discovered` event stream for live updates.
//!
//! **TXT-record contract** — the single source of truth for what a host
//! advertises and a client parses. Encoded by [`encode_txt`], parsed by
//! [`DiscoveredServer::from_service`]. Round-trips through [`encode_txt`] +
//! parse (unit-tested):
//!
//! | key    | value                                   |
//! |--------|-----------------------------------------|
//! | `url`  | `lore://host:port/<repo>` connect URL   |
//! | `repo` | repository name (may be empty)          |
//! | `name` | friendly instance name (host label)     |
//! | `v`    | protocol version of this TXT schema (1) |
//!
//! ## Platform caveats
//!
//! mDNS is multicast UDP on `224.0.0.251:5353`. `mdns-sd` is pure-Rust and works
//! on Windows, macOS, and Linux with no system daemon, but:
//!
//! - **Firewalls** may block inbound multicast. On Windows the first run can
//!   raise a Defender Firewall prompt; users must allow LoreGUI on private
//!   networks. Locked-down corporate networks often drop mDNS at the switch — the
//!   manual "Connect to Server" URL field always remains as a fallback.
//! - **Multiple / virtual NICs** (WSL vEthernet, Hyper-V, Docker bridges, VPN
//!   TUN/TAP) can advertise an unreachable address. [`primary_lan_ipv4`] filters
//!   those out and prefers an RFC1918 LAN address, matching model-manager's NIC
//!   selection.
//! - **VPNs** that capture all multicast can hide LAN peers; again, the manual
//!   URL path is the documented fallback.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;

/// mDNS service type for lore servers. Mirrors model-manager's
/// `_gateway._tcp.local.` convention, scoped to lore.
pub const SERVICE_TYPE: &str = "_lore._tcp.local.";

/// Current version of the TXT-record schema (see module docs). Bumped only on a
/// breaking change to the advertised keys so older clients can skip what they
/// cannot parse.
pub const TXT_SCHEMA_VERSION: &str = "1";

/// Tauri event name carrying the live discovered-server list. The frontend
/// `listen`s on this to refresh without polling (mirrors `lock/request`).
pub const LAN_DISCOVERED_EVENT: &str = "lan/discovered";

/// One lore server discovered on the LAN, as surfaced to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredServer {
    /// Stable identity within a browse session: the mDNS service fullname
    /// (`<instance>._lore._tcp.local.`). Used for dedupe + removal.
    pub id: String,
    /// Friendly instance name (TXT `name`), e.g. "Brian's Mac". Falls back to the
    /// mDNS instance label when absent.
    pub name: String,
    /// Repository name (TXT `repo`); empty string when the host advertised none.
    pub repo: String,
    /// The `lore://host:port/<repo>` URL a client connects with (TXT `url`). This
    /// is what the one-click "Connect" prefills.
    pub url: String,
    /// Resolved host (first IPv4 address, else the mDNS hostname). Display-only —
    /// the authoritative connect target is [`url`](Self::url).
    pub host: String,
    /// Advertised port (the lore QUIC/gRPC port).
    pub port: u16,
}

impl DiscoveredServer {
    /// Build a [`DiscoveredServer`] from a resolved mDNS [`ServiceInfo`].
    ///
    /// Returns `None` when the service does not carry a usable `url` TXT record
    /// (i.e. it is not one of *our* lore advertisements, or an older/newer schema
    /// we cannot read) — such services are silently skipped rather than shown as
    /// broken rows.
    pub fn from_service(info: &ServiceInfo) -> Option<Self> {
        let txt = txt_map(info);
        // A lore advertisement MUST carry a connect URL; without it the row is
        // useless (nothing to one-click). Skip anything that lacks it.
        let url = txt.get("url").map(String::as_str).unwrap_or("").trim();
        if url.is_empty() {
            return None;
        }

        let host = info
            .get_addresses_v4()
            .iter()
            .next()
            .map(|a| a.to_string())
            .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_string());

        let name = txt
            .get("name")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| instance_label(info.get_fullname()));

        Some(DiscoveredServer {
            id: info.get_fullname().to_string(),
            name,
            repo: txt
                .get("repo")
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            url: url.to_string(),
            host,
            port: info.get_port(),
        })
    }
}

/// Lowercased TXT properties as a `key -> value` map. `mdns-sd` keys are
/// case-insensitive per RFC 6763; we normalise to lowercase so lookups are
/// stable regardless of how a peer cased them.
fn txt_map(info: &ServiceInfo) -> HashMap<String, String> {
    info.get_properties()
        .iter()
        .map(|p| (p.key().to_ascii_lowercase(), p.val_str().to_string()))
        .collect()
}

/// Extract the human-facing instance label from an mDNS fullname
/// (`"<instance>._lore._tcp.local."` -> `"<instance>"`).
fn instance_label(fullname: &str) -> String {
    fullname
        .split_once("._lore._tcp")
        .map(|(label, _)| label)
        .unwrap_or(fullname)
        .to_string()
}

/// Encode the LoreGUI TXT-record set advertised for a hosted server. The single
/// place the host-side property map is built; [`DiscoveredServer::from_service`]
/// is its exact inverse. Returned as an owned `HashMap` ready for
/// [`ServiceInfo::new`].
pub fn encode_txt(connect_url: &str, repo: &str, friendly_name: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert("url".to_string(), connect_url.trim().to_string());
    props.insert("repo".to_string(), repo.trim().to_string());
    props.insert("name".to_string(), friendly_name.trim().to_string());
    props.insert("v".to_string(), TXT_SCHEMA_VERSION.to_string());
    props
}

/// Pick the primary LAN IPv4 address to advertise.
///
/// Filters loopback, link-local (169.254/16), and obvious virtual/container NICs
/// (Docker, WSL/Hyper-V vEthernet, VPN TUN/TAP, VirtualBox/VMware), preferring a
/// private RFC1918 address — the same selection model-manager uses (SBAI-3036 /
/// SBAI-3195). Returns `None` if nothing suitable is found (host advertises with
/// no explicit address; `mdns-sd` then falls back to OS-reported addresses).
pub fn primary_lan_ipv4() -> Option<Ipv4Addr> {
    let ifaces = if_addrs::get_if_addrs().ok()?;
    let mut candidates: Vec<Ipv4Addr> = Vec::new();
    for iface in ifaces {
        if iface.is_loopback() {
            continue;
        }
        let name = iface.name.to_ascii_lowercase();
        // Skip obvious virtual / container / VPN adapters whose address is not a
        // reachable LAN address for peers on the physical network: Docker/veth/
        // bridges, WSL/Hyper-V vEthernet, VMware/VirtualBox, libvirt, VPN
        // TUN/TAP, ZeroTier ("zt"), WireGuard ("wg").
        const VIRTUAL_NIC_MARKERS: &[&str] = &[
            "docker",
            "veth",
            "vethernet",
            "hyper-v",
            "vmnet",
            "vboxnet",
            "virbr",
            "utun",
            "tun",
            "tap",
            "zt",
            "wg",
        ];
        if name.starts_with("br-") || VIRTUAL_NIC_MARKERS.iter().any(|m| name.contains(m)) {
            continue;
        }
        if let std::net::IpAddr::V4(v4) = iface.ip() {
            if v4.is_link_local() {
                continue;
            }
            candidates.push(v4);
        }
    }
    // Prefer RFC1918 private addresses (real LAN) over anything else.
    candidates
        .iter()
        .find(|ip| ip.is_private())
        .copied()
        .or_else(|| candidates.first().copied())
}

/// A live mDNS advertisement of a hosted lore server. Holding this keeps the
/// service registered; dropping it unregisters (best-effort) and shuts the
/// daemon down. One per hosted server — created on `host_server_start`, dropped
/// on `host_server_stop`.
pub struct Announcer {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Announcer {
    /// Register a lore server on the LAN.
    ///
    /// * `connect_url` — the `lore://host:port/<repo>` URL clients dial.
    /// * `repo` — repository name (may be empty).
    /// * `friendly_name` — a human label for the host (e.g. the machine name).
    /// * `port` — the advertised lore port.
    /// * `lan_ip` — the address to advertise; when `None`, [`primary_lan_ipv4`]
    ///   is tried, and failing that the OS-reported addresses are used.
    pub fn start(
        connect_url: &str,
        repo: &str,
        friendly_name: &str,
        port: u16,
        lan_ip: Option<Ipv4Addr>,
    ) -> Result<Self, String> {
        let daemon =
            ServiceDaemon::new().map_err(|e| format!("mDNS daemon could not start: {e}"))?;

        // Instance name: keep it stable + readable. The friendly name plus port
        // disambiguates two hosts on one machine.
        let instance = sanitize_instance(friendly_name, port);
        let host_name = format!("{instance}.local.");
        let props = encode_txt(connect_url, repo, friendly_name);

        let ip = lan_ip.or_else(primary_lan_ipv4);
        // mdns-sd 0.13's `AsIpAddrs` is implemented for `IpAddr` (and slices of
        // it), not `Ipv4Addr` directly — collect as `IpAddr`. An empty slice
        // tells mdns-sd to auto-enumerate the host's addresses.
        let addrs: Vec<std::net::IpAddr> = ip.map(std::net::IpAddr::V4).into_iter().collect();
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &host_name,
            &addrs[..],
            port,
            Some(props),
        )
        .map_err(|e| format!("mDNS service info invalid: {e}"))?;

        let fullname = service.get_fullname().to_string();
        daemon
            .register(service)
            .map_err(|e| format!("mDNS register failed: {e}"))?;
        tracing::info!(service = %fullname, %port, "lore server announced on LAN");
        Ok(Announcer { daemon, fullname })
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        // Best-effort unregister + shutdown. Both return a receiver we don't need
        // to await: the daemon flushes the goodbye packet on its own thread.
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
        tracing::info!(service = %self.fullname, "lore server LAN announcement stopped");
    }
}

/// Make a safe mDNS instance label from a friendly name. mDNS instance names may
/// contain spaces, but to keep them readable and avoid empty labels we trim,
/// fall back to a default, and append the port for disambiguation.
fn sanitize_instance(friendly_name: &str, port: u16) -> String {
    let base = friendly_name.trim();
    let base = if base.is_empty() {
        "LoreGUI server"
    } else {
        base
    };
    format!("{base} ({port})")
}

/// A live LAN browse session. Spawns a background thread that drains the
/// `mdns-sd` event channel into a shared, deduped [`DiscoveredServer`] list.
/// Holding it keeps the browse running; dropping it stops the thread and the
/// daemon. The Tauri layer keeps one in `AppState`.
pub struct Browser {
    daemon: ServiceDaemon,
    servers: Arc<Mutex<Vec<DiscoveredServer>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Browser {
    /// Start browsing for lore servers. `on_change` is invoked (on the browse
    /// thread) with the new full list every time the set changes, so the caller
    /// can push a Tauri event. Returns immediately; discovery is asynchronous.
    pub fn start<F>(on_change: F) -> Result<Self, String>
    where
        F: Fn(Vec<DiscoveredServer>) + Send + 'static,
    {
        let daemon =
            ServiceDaemon::new().map_err(|e| format!("mDNS daemon could not start: {e}"))?;
        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| format!("mDNS browse failed: {e}"))?;

        let servers: Arc<Mutex<Vec<DiscoveredServer>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let servers_thread = servers.clone();
        let stop_thread = stop.clone();
        std::thread::spawn(move || {
            while !stop_thread.load(std::sync::atomic::Ordering::Relaxed) {
                // Block with a timeout so we periodically re-check the stop flag.
                // On any recv error we either time out (re-check stop + loop) or
                // the daemon has gone away (disconnected) -- distinguished without
                // naming the underlying channel's error type, which keeps us
                // decoupled from mdns-sd's internal channel implementation.
                let event = match receiver.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(ev) => ev,
                    Err(_) if receiver.is_disconnected() => break,
                    Err(_) => continue,
                };
                let changed = match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(found) = DiscoveredServer::from_service(&info) {
                            let mut list = servers_thread.lock().unwrap();
                            upsert(&mut list, found)
                        } else {
                            false
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        let mut list = servers_thread.lock().unwrap();
                        let before = list.len();
                        list.retain(|s| s.id != fullname);
                        list.len() != before
                    }
                    _ => false,
                };
                if changed {
                    let snapshot = servers_thread.lock().unwrap().clone();
                    on_change(snapshot);
                }
            }
        });

        Ok(Browser {
            daemon,
            servers,
            stop,
        })
    }

    /// A snapshot of the currently-discovered servers, sorted by name.
    pub fn snapshot(&self) -> Vec<DiscoveredServer> {
        let mut list = self.servers.lock().unwrap().clone();
        list.sort_by_key(|s| s.name.to_lowercase());
        list
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

/// Insert-or-update a discovered server in `list` keyed by its stable `id`.
/// Returns `true` if the list changed (new entry, or an existing entry's fields
/// differed) — so the caller only emits on real changes.
fn upsert(list: &mut Vec<DiscoveredServer>, found: DiscoveredServer) -> bool {
    if let Some(existing) = list.iter_mut().find(|s| s.id == found.id) {
        if *existing == found {
            false
        } else {
            *existing = found;
            true
        }
    } else {
        list.push(found);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The load-bearing invariant: whatever a host encodes into TXT records, a
    // client parses back identically — especially the connect URL round-trip.
    #[test]
    fn txt_round_trip_preserves_url_repo_name() {
        let url = "lore://192.168.1.42:41337/my-repo";
        let props = encode_txt(url, "my-repo", "Brian's Mac");

        assert_eq!(props.get("url").map(String::as_str), Some(url));
        assert_eq!(props.get("repo").map(String::as_str), Some("my-repo"));
        assert_eq!(props.get("name").map(String::as_str), Some("Brian's Mac"));
        assert_eq!(props.get("v").map(String::as_str), Some("1"));
    }

    // Build a real ServiceInfo from encoded TXT and parse it back; the URL,
    // repo, name and port must survive a full mdns-sd encode->decode.
    #[test]
    fn service_info_decodes_to_discovered_server() {
        let url = "lore://10.15.0.20:41337/world-bible";
        let props = encode_txt(url, "world-bible", "BRAINZ");
        let addr: std::net::IpAddr = "10.15.0.20".parse().unwrap();
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "BRAINZ (41337)",
            "BRAINZ.local.",
            &[addr][..],
            41337,
            Some(props),
        )
        .expect("valid service info");

        let found = DiscoveredServer::from_service(&info).expect("parses back");
        assert_eq!(found.url, url);
        assert_eq!(found.repo, "world-bible");
        assert_eq!(found.name, "BRAINZ");
        assert_eq!(found.port, 41337);
        assert_eq!(found.host, "10.15.0.20");
        assert!(found.id.contains("_lore._tcp"));
    }

    // A non-lore service (or one missing the url TXT key) must be skipped, not
    // surfaced as a broken row.
    #[test]
    fn service_without_url_is_skipped() {
        let mut props = HashMap::new();
        props.insert("repo".to_string(), "x".to_string());
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "stranger",
            "stranger.local.",
            "",
            41337,
            Some(props),
        )
        .expect("valid service info");
        assert!(DiscoveredServer::from_service(&info).is_none());
    }

    #[test]
    fn upsert_dedupes_and_reports_change() {
        let mut list = Vec::new();
        let a = DiscoveredServer {
            id: "a._lore._tcp.local.".into(),
            name: "A".into(),
            repo: "r".into(),
            url: "lore://1.1.1.1:1/r".into(),
            host: "1.1.1.1".into(),
            port: 1,
        };
        assert!(upsert(&mut list, a.clone())); // new -> changed
        assert!(!upsert(&mut list, a.clone())); // identical -> no change
        let mut a2 = a.clone();
        a2.repo = "r2".into();
        assert!(upsert(&mut list, a2)); // field differs -> changed
        assert_eq!(list.len(), 1); // still one entry (deduped by id)
    }

    #[test]
    fn instance_label_strips_service_suffix() {
        assert_eq!(
            instance_label("BRAINZ (41337)._lore._tcp.local."),
            "BRAINZ (41337)"
        );
        assert_eq!(instance_label("bare"), "bare");
    }
}
