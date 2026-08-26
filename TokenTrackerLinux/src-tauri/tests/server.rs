//! Server lifecycle details that are easy to break silently: the port fallback
//! that OAuth depends on, the argument shapes passed to the bundled CLI, and the
//! log file handling that used to `panic!` and grow without bound.

use std::ffi::OsString;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tokentracker_linux::server::{
    boot_id, dashboard_url, leads_own_process_group, pick_available_port, process_start_time,
    rotate_log_if_oversized, serve_args, server_log_paths, server_record_dirs, server_record_name,
    sync_args, MAX_LOG_BYTES,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tokentracker-linux-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch dir should be creatable");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PREFERRED_PORT: u16 = 17680;

// Both port tests bind PREFERRED_PORT, and cargo runs tests in parallel. The
// free-port case drops its probe before calling pick_available_port() — which
// is exactly the window the fallback case spends holding that same port — so
// unserialized they race and the free-port case gets handed a fallback port.
// Poisoning is recovered from rather than propagated: one test panicking must
// not turn the other into a second, misleading failure.
static PREFERRED_PORT_GUARD: Mutex<()> = Mutex::new(());

fn preferred_port_guard() -> std::sync::MutexGuard<'static, ()> {
    PREFERRED_PORT_GUARD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[test]
fn preferred_port_is_used_when_free() {
    let _serial = preferred_port_guard();
    // Only meaningful when nothing else on the machine holds 17680.
    let Ok(probe) = TcpListener::bind(("127.0.0.1", PREFERRED_PORT)) else {
        eprintln!("port {PREFERRED_PORT} is busy on this machine; skipping");
        return;
    };
    drop(probe);

    let port = pick_available_port().expect("a port should be available");
    assert_eq!(
        port, PREFERRED_PORT,
        "OAuth redirect URLs are registered against the fixed port, so it must be preferred"
    );
}

#[test]
fn port_selection_falls_back_when_the_preferred_port_is_taken() {
    let _serial = preferred_port_guard();
    // Hold the preferred port for the duration of the call.
    let Ok(holder) = TcpListener::bind(("127.0.0.1", PREFERRED_PORT)) else {
        eprintln!("port {PREFERRED_PORT} is busy on this machine; skipping");
        return;
    };

    let port = pick_available_port().expect("fallback port should be assigned");
    assert_ne!(
        port, PREFERRED_PORT,
        "must not report a port that is already held"
    );
    assert!(port > 0);
    TcpListener::bind(("127.0.0.1", port)).expect("fallback port should be bindable");

    drop(holder);
}

#[test]
fn dashboard_url_is_loopback_only() {
    // Binding to 127.0.0.1 rather than 0.0.0.0 keeps usage data off the LAN.
    assert_eq!(dashboard_url(17680), "http://127.0.0.1:17680");
    assert!(!dashboard_url(17680).contains("0.0.0.0"));
    assert!(!dashboard_url(17680).contains("localhost"));
}

#[test]
fn serve_args_disable_browser_launch_and_startup_sync() {
    let args = serve_args(Path::new("/opt/tokentracker/bin/tracker.js"), 34567);
    assert_eq!(
        args,
        vec![
            OsString::from("/opt/tokentracker/bin/tracker.js"),
            OsString::from("serve"),
            OsString::from("--port"),
            OsString::from("34567"),
            OsString::from("--no-open"),
            OsString::from("--no-sync"),
        ],
    );
}

#[test]
fn serve_args_carry_the_selected_port() {
    // A hardcoded port here would leave the webview pointing at a dead server
    // whenever the fallback path is taken.
    let args = serve_args(Path::new("/x/tracker.js"), 45678);
    let port_index = args
        .iter()
        .position(|arg| arg == &OsString::from("--port"))
        .expect("--port should be present");
    assert_eq!(args[port_index + 1], OsString::from("45678"));
}

#[test]
fn sync_args_scan_local_sources_without_publishing() {
    let args = sync_args(Path::new("/opt/tokentracker/bin/tracker.js"));
    assert_eq!(
        args,
        vec![
            OsString::from("/opt/tokentracker/bin/tracker.js"),
            OsString::from("sync"),
            OsString::from("--auto"),
            OsString::from("--background"),
            OsString::from("--all-local-sources"),
        ],
    );
    // Background sync must never publish to the cloud on the user's behalf.
    assert!(!args.contains(&OsString::from("--publish-account")));
}

#[test]
fn log_paths_prefer_xdg_state_home() {
    let paths = server_log_paths(
        Some(PathBuf::from("/home/u/.local/state")),
        Some(PathBuf::from("/home/u")),
    );

    assert_eq!(
        paths.first(),
        Some(&PathBuf::from(
            "/home/u/.local/state/tokentracker/server.log"
        ))
    );
    // /tmp is always the last resort so a read-only home never loses logging.
    assert_eq!(
        paths.last(),
        Some(&PathBuf::from("/tmp/tokentracker-server.log"))
    );
}

#[test]
fn log_paths_fall_back_to_home_then_tmp() {
    let paths = server_log_paths(None, Some(PathBuf::from("/home/u")));
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/home/u/.local/state/tokentracker/server.log"),
            PathBuf::from("/tmp/tokentracker-server.log"),
        ]
    );
}

#[test]
fn log_paths_survive_a_missing_home() {
    let paths = server_log_paths(None, None);
    assert_eq!(paths, vec![PathBuf::from("/tmp/tokentracker-server.log")]);
}

#[test]
fn log_paths_do_not_duplicate_when_xdg_and_home_agree() {
    let paths = server_log_paths(
        Some(PathBuf::from("/home/u/.local/state")),
        Some(PathBuf::from("/home/u")),
    );
    let mut deduped = paths.clone();
    deduped.dedup();
    assert_eq!(paths.len(), deduped.len(), "got {paths:?}");
}

#[test]
fn small_logs_are_left_alone() {
    let temp = TempDir::new("log-small");
    let log = temp.path().join("server.log");
    fs::write(&log, b"a few lines\n").expect("write log");

    assert!(!rotate_log_if_oversized(&log, MAX_LOG_BYTES));
    assert_eq!(fs::read(&log).expect("log still there"), b"a few lines\n");
}

#[test]
fn oversized_logs_rotate_to_a_single_previous_generation() {
    let temp = TempDir::new("log-rotate");
    let log = temp.path().join("server.log");
    fs::write(&log, vec![b'x'; 64]).expect("write log");

    assert!(
        rotate_log_if_oversized(&log, 32),
        "a log over the limit should rotate"
    );

    let rotated = temp.path().join("server.log.1");
    assert!(rotated.exists(), "previous generation should be kept");
    assert_eq!(fs::read(&rotated).expect("rotated readable").len(), 64);
    assert!(!log.exists(), "the live log is recreated on next open");
}

#[test]
fn rotation_replaces_an_older_generation_so_growth_stays_bounded() {
    let temp = TempDir::new("log-bounded");
    let log = temp.path().join("server.log");
    fs::write(temp.path().join("server.log.1"), vec![b'o'; 8]).expect("old generation");
    fs::write(&log, vec![b'n'; 64]).expect("write log");

    assert!(rotate_log_if_oversized(&log, 32));

    // Exactly one previous generation, and it is the newer content.
    let rotated = fs::read(temp.path().join("server.log.1")).expect("rotated readable");
    assert_eq!(rotated.len(), 64);
    assert!(rotated.iter().all(|byte| *byte == b'n'));
    assert!(!temp.path().join("server.log.2").exists());
}

#[test]
fn rotating_a_missing_log_is_a_no_op() {
    let temp = TempDir::new("log-absent");
    assert!(!rotate_log_if_oversized(
        &temp.path().join("server.log"),
        MAX_LOG_BYTES
    ));
}

/// Guard against someone effectively disabling rotation by raising the cap to
/// absurdity, or making it so small the log is useless. Evaluated at compile
/// time because both sides are constants.
const _: () = {
    assert!(MAX_LOG_BYTES >= 1024 * 1024);
    assert!(MAX_LOG_BYTES <= 64 * 1024 * 1024);
};

#[test]
fn records_never_land_in_a_world_writable_directory() {
    let dirs = server_record_dirs(
        Some(PathBuf::from("/state")),
        Some(PathBuf::from("/home/u")),
    );

    assert_eq!(dirs[0], PathBuf::from("/state/tokentracker/servers"));
    assert_eq!(
        dirs[1],
        PathBuf::from("/home/u/.local/state/tokentracker/servers")
    );

    // No /tmp fallback, unlike the log: /tmp is writable by every account, so a
    // forged record there could make this user signal one of their own PIDs.
    assert!(dirs.iter().all(|dir| !dir.starts_with("/tmp")));
    assert!(server_log_paths(None, None)
        .iter()
        .any(|path| path.starts_with("/tmp")));
    assert!(server_record_dirs(None, None).is_empty());
}

#[test]
fn each_owner_gets_its_own_record_file() {
    // Two concurrent login sessions on one account each run an app instance,
    // so a shared filename would let one delete or overwrite the other's
    // record and strand its server permanently.
    assert_eq!(server_record_name(1234), "server-1234.json");
    assert_ne!(server_record_name(1234), server_record_name(5678));
}

#[test]
fn process_start_time_identifies_a_specific_process() {
    let own = std::process::id() as i32;
    let first = process_start_time(own).expect("our own start time is readable");

    // Stable across reads: this is what makes it usable as an identity, so a
    // recycled PID cannot be mistaken for the recorded process.
    assert_eq!(process_start_time(own), Some(first));

    // A PID that cannot exist has no start time, so nothing is ever signalled
    // on its behalf.
    assert_eq!(process_start_time(-1), None);
}

#[test]
fn a_record_is_bound_to_the_current_boot() {
    // Sandboxes and containers can hide /proc/sys/kernel/random/boot_id.
    // Production treats that as "record nothing, signal nothing", so absence is
    // a supported state rather than a test failure.
    let Some(id) = boot_id() else {
        return;
    };

    assert!(!id.is_empty());
    // Stable within a boot, so it can validate a persisted record; it changes
    // on reboot, which is what invalidates a stale one whose PID and
    // since-boot start tick could otherwise be matched by an unrelated process.
    assert_eq!(boot_id(), Some(id));
}

#[test]
fn only_real_group_leaders_can_be_signalled_by_group() {
    // kill(-1, sig) is POSIX's broadcast to every process the caller may
    // signal, so a record naming pid 1 must never reach the negated-pid path:
    // it would tear down the whole login session instead of one server.
    assert!(!leads_own_process_group(1));
    assert!(!leads_own_process_group(0));
    assert!(!leads_own_process_group(-1));

    // A pid that cannot exist leads no group either.
    assert!(!leads_own_process_group(i32::MAX));

    // The test binary is not a group leader (the shell that spawned it is), so
    // negating its pid would signal a group it does not own.
    let own = std::process::id() as i32;
    // SAFETY: getpgid(2) on our own pid.
    let pgid = unsafe { libc::getpgid(own) };
    assert_eq!(leads_own_process_group(own), pgid == own);
}
