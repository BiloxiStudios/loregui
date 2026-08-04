//! SBAI-5910 — the pasted-bearer trust boundary, proven at the SHARED seam.
//!
//! sb-secure found that denying only in the Tauri command left the `lorevm`
//! CLI and the `lorevm-ffi` C ABI able to reach the dangerous path, because
//! both route `<domain>.<op>` through `lore_vm::dispatch` (see
//! `crates/lorevm-cli/src/main.rs` and `crates/lorevm-ffi/src/lib.rs`). The
//! refusal therefore lives in the op itself, which is the one function every
//! driver reaches.
//!
//! The property under test, identical at every surface: **no pasted bearer is
//! ever sent to an endpoint the untrusted remote named.** Upstream only asks
//! the remote for its advertised endpoint when `auth_url` is empty, so the
//! gate is exactly "an explicit operator-supplied endpoint is required".

use lore_vm::api::LoreApi;
use serde_json::json;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// A stand-in for the attacker-controlled remote. Polls non-blockingly and
/// stops on a flag, so the test never connects to it — the asserted metrics
/// are literally zero.
struct AttackerRemote {
    addr: std::net::SocketAddr,
    accepts: Arc<AtomicUsize>,
    bytes: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl AttackerRemote {
    fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        listener.set_nonblocking(true).expect("nonblocking");
        let accepts = Arc::new(AtomicUsize::new(0));
        let bytes = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (a, b, s) = (accepts.clone(), bytes.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        a.fetch_add(1, Ordering::SeqCst);
                        let _ =
                            stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
                        let mut buf = [0u8; 4096];
                        while let Ok(n) = stream.read(&mut buf) {
                            if n == 0 {
                                break;
                            }
                            b.fetch_add(n, Ordering::SeqCst);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr,
            accepts,
            bytes,
            stop,
            handle: Some(handle),
        }
    }

    /// Sample after giving any forbidden in-flight dial time to land.
    fn finish(mut self) -> (usize, usize) {
        std::thread::sleep(std::time::Duration::from_millis(150));
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        (
            self.accepts.load(Ordering::SeqCst),
            self.bytes.load(Ordering::SeqCst),
        )
    }
}

const SECRET: &str = "super-secret-pasted-bearer-token";

/// The CLI and the FFI both call `lore_vm::dispatch`; covering dispatch is
/// therefore the shared proof for both external drivers.
#[tokio::test]
async fn dispatch_refuses_pasted_token_without_an_explicit_endpoint() {
    let attacker = AttackerRemote::start();
    let tmp = tempfile::tempdir().expect("tempdir");
    let api = LoreApi::new(tmp.path().to_path_buf());

    // auth_url omitted — this is the remote-advertised discovery path.
    let args = json!({
        "remote_url": format!("lore://{}/attacker-controlled", attacker.addr),
        "token": SECRET,
    });
    let error = lore_vm::dispatch(&api, "auth.login_with_token", args)
        .await
        .expect_err("dispatch must refuse the remote-advertised path");

    let text = error.to_string();
    assert!(
        text.contains("explicit authentication endpoint"),
        "refusal must name the missing endpoint: {text}"
    );
    assert!(!text.contains(SECRET), "refusal must not echo the token");
    assert!(
        !text.contains(&attacker.addr.to_string()),
        "refusal must not echo the remote URL"
    );

    let (accepts, bytes) = attacker.finish();
    assert_eq!(accepts, 0, "attacker remote must observe ZERO connections");
    assert_eq!(bytes, 0, "attacker remote must receive ZERO bytes");
}

/// Empty and whitespace-only endpoints are the same dangerous case.
#[tokio::test]
async fn dispatch_refuses_blank_endpoints_too() {
    let attacker = AttackerRemote::start();
    let tmp = tempfile::tempdir().expect("tempdir");
    let api = LoreApi::new(tmp.path().to_path_buf());

    for blank in ["", "   "] {
        let args = json!({
            "remote_url": format!("lore://{}/attacker", attacker.addr),
            "token": SECRET,
            "auth_url": blank,
        });
        let error = lore_vm::dispatch(&api, "auth.login_with_token", args)
            .await
            .expect_err("blank endpoint must be refused");
        assert!(error
            .to_string()
            .contains("explicit authentication endpoint"));
    }

    let (accepts, bytes) = attacker.finish();
    assert_eq!(accepts, 0);
    assert_eq!(bytes, 0);
}

/// The legitimate headless case is NOT broken: with an operator-supplied
/// endpoint the gate lets the call through (it then fails on the unreachable
/// endpoint, which proves the gate is not what rejected it).
#[tokio::test]
async fn explicit_operator_endpoint_passes_the_gate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let api = LoreApi::new(tmp.path().to_path_buf());
    let args = json!({
        "remote_url": "lore://127.0.0.1:1/unreachable",
        "token": SECRET,
        "auth_url": "ucs-auth://auth.operator.example",
    });
    let error = lore_vm::dispatch(&api, "auth.login_with_token", args)
        .await
        .expect_err("the unreachable endpoint still fails");
    assert!(
        !error
            .to_string()
            .contains("explicit authentication endpoint"),
        "the gate must NOT be what rejected an operator-supplied endpoint: {error}"
    );
}

/// The op stays routable — removing it would break headless drivers, which is
/// the regression this design deliberately avoids.
#[test]
fn op_remains_advertised_for_headless_drivers() {
    assert!(
        lore_vm::supported_ops().contains(&"auth.login_with_token"),
        "the op must remain dispatchable with an explicit endpoint"
    );
}
