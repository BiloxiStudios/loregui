//! Shared test harness for the `lorevm` CLI integration suite.
//!
//! Every test in this crate drives the **real built binary** as a subprocess —
//! the exact contract the VS Code extension, `lore-mcp`, and the UE plugin
//! consume. The binary path comes from `CARGO_BIN_EXE_lorevm`, an env var cargo
//! sets for integration tests of a crate that declares a `[[bin]]`, so the tests
//! always run against the freshly compiled binary (no `assert_cmd`, no stale
//! bundled engine).
//!
//! The harness sets up a real on-disk lore repo the same way `lore-vm`'s own
//! `e2e_lifecycle` test does: a shared store created **outside** the working
//! tree, the repo created with `use_shared_store=true` pointed at it. Everything
//! runs `--offline` (no server) with a fixed `--identity` so author attribution
//! and revision flow are deterministic and CI-runnable with no network.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Outcome of one `lorevm` invocation: exit success + parsed stdout JSON.
///
/// The CLI prints exactly one pretty-JSON document to stdout — either the op's
/// typed result (exit 0) or an `{"error":{kind,message}}` envelope (exit 1).
pub struct Run {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// Stdout parsed as JSON. `None` only for the multi-line `--help`/usage text,
    /// which is intentionally printed raw rather than as JSON.
    pub json: Option<Value>,
}

impl Run {
    /// The parsed stdout JSON, panicking with the raw output if it did not parse.
    pub fn value(&self) -> &Value {
        self.json.as_ref().unwrap_or_else(|| {
            panic!(
                "expected JSON on stdout but it did not parse.\nstdout:\n{}\nstderr:\n{}",
                self.stdout, self.stderr
            )
        })
    }

    /// Assert this run succeeded (exit 0) and return the parsed JSON result.
    pub fn ok_value(&self) -> &Value {
        assert!(
            self.success,
            "expected exit 0 but the run failed.\nstdout:\n{}\nstderr:\n{}",
            self.stdout, self.stderr
        );
        self.value()
    }

    /// Assert this run failed (exit 1) AND emitted the canonical structured
    /// error envelope `{"error":{"kind":..,"message":..}}`. Returns `(kind,
    /// message)` so callers can assert on the specifics. This is the single most
    /// important contract for downstream drivers: a failure is *always* a clean,
    /// machine-readable envelope — never a panic, a stack trace, or bare text.
    pub fn err_envelope(&self) -> (String, String) {
        assert!(
            !self.success,
            "expected a failure exit code but the run succeeded.\nstdout:\n{}",
            self.stdout
        );
        let v = self.value();
        let err = v.get("error").unwrap_or_else(|| {
            panic!(
                "failure output is missing the `error` key.\nstdout:\n{}",
                self.stdout
            )
        });
        let kind = err
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("error envelope missing string `kind`: {v}"))
            .to_string();
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("error envelope missing string `message`: {v}"))
            .to_string();
        assert!(
            !message.is_empty(),
            "error envelope `message` is empty: {v}"
        );
        (kind, message)
    }
}

/// A real, on-disk, offline lore repository plus the shared store backing it.
///
/// Both the working tree and the store live in their own temp dirs and are
/// cleaned up on drop. Held as fields so the `TempDir`s outlive every op the
/// test runs against them.
pub struct Repo {
    _work: tempfile::TempDir,
    _store: tempfile::TempDir,
    pub dir: PathBuf,
    pub store_path: PathBuf,
    pub identity: String,
}

impl Repo {
    /// Create a brand-new offline repo authored by `identity`. Runs
    /// `repository.create` through the CLI itself, so the constructor doubles as
    /// coverage of the create op's happy path.
    pub fn create(identity: &str) -> Repo {
        let work = tempfile::tempdir().expect("create work tempdir");
        let store = tempfile::tempdir().expect("create store tempdir");
        // The store dir must exist but be a *different* tree from the repo.
        let store_path = store.path().join("shared-store");
        std::fs::create_dir_all(&store_path).expect("create shared-store dir");

        let repo = Repo {
            dir: work.path().to_path_buf(),
            store_path: store_path.clone(),
            identity: identity.to_string(),
            _work: work,
            _store: store,
        };

        let args = serde_json::json!({
            "repository_url": format!("lore://localhost/clitest-{}", std::process::id()),
            "description": "lorevm-cli integration repo",
            "use_shared_store": true,
            "shared_store_path": store_path.to_string_lossy(),
        });
        let run = repo.run("repository.create", &args.to_string());
        let v = run.ok_value();
        assert!(
            v.get("id")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty()),
            "repository.create returned no id: {v}"
        );
        repo
    }

    /// Absolute path of `name` inside the working tree.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Write `contents` to `name` inside the working tree, creating parents.
    pub fn write_file(&self, name: &str, contents: &str) -> PathBuf {
        let p = self.path(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&p, contents).expect("write file");
        p
    }

    /// Run an op against this repo `--offline` with this repo's identity, passing
    /// the given raw JSON `args` string. The single most-used helper.
    pub fn run(&self, op: &str, args_json: &str) -> Run {
        run_cli(&[
            op,
            "--dir",
            &self.dir.to_string_lossy(),
            "--offline",
            "--identity",
            &self.identity,
            "--args",
            args_json,
        ])
    }

    /// Run an op against this repo with NO `--args` (the no-arg op path).
    pub fn run_noargs(&self, op: &str) -> Run {
        run_cli(&[
            op,
            "--dir",
            &self.dir.to_string_lossy(),
            "--offline",
            "--identity",
            &self.identity,
        ])
    }

    /// Stage `name` (a working-tree file) via a separate CLI process and assert
    /// success. Returns the parsed result.
    pub fn stage(&self, name: &str) -> Value {
        let abs = self.path(name);
        let args = serde_json::json!({ "paths": [abs.to_string_lossy()], "scan": true });
        self.run("file.stage", &args.to_string()).ok_value().clone()
    }

    /// Commit the staged state via a separate CLI process. Returns the new
    /// revision hash (asserted non-empty).
    pub fn commit(&self, message: &str) -> String {
        let args = serde_json::json!({ "message": message });
        let v = self
            .run("revision.commit", &args.to_string())
            .ok_value()
            .clone();
        let rev = v
            .get("revision")
            .and_then(Value::as_str)
            .expect("commit result missing revision")
            .to_string();
        assert!(!rev.is_empty(), "commit returned an empty revision: {v}");
        rev
    }
}

/// Path to the freshly built `lorevm` binary under test.
pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lorevm"))
}

/// Invoke the `lorevm` binary with `args`, capturing exit + stdout (parsed as
/// JSON where possible). The low-level primitive every test ultimately uses.
pub fn run_cli(args: &[&str]) -> Run {
    run_cli_in(None, args)
}

/// Like [`run_cli`] but runs with `cwd` as the process working directory — used
/// to exercise `--dir` defaulting to `.`.
pub fn run_cli_in(cwd: Option<&Path>, args: &[&str]) -> Run {
    let mut cmd = Command::new(bin());
    cmd.args(args);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn lorevm {args:?}: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let json = serde_json::from_str::<Value>(&stdout).ok();
    Run {
        success: out.status.success(),
        stdout,
        stderr,
        json,
    }
}
