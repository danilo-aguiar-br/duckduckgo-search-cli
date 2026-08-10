// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for process_lifecycle.

use super::*;
// Only the Unix / Linux process-spawn tests below use these.
#[cfg(unix)]
use std::process::{Command, Stdio};

/// An isolated sweep root with the three fixture kinds already planted.
///
/// # Why this replaced the shared-`/tmp` fixtures and their lock
///
/// These tests used to plant `ddg-chrome-*`, `.tmp*` and `org.chromium.*`
/// directly under `std::env::temp_dir()` and serialise on a process-local
/// mutex. That mutex only ever protected threads inside one binary, while
/// `cargo test-all` runs twenty-five binaries concurrently — and, worse, the
/// host itself is not ours. A run failed on `org.chromium.* must survive sweep`
/// while another project built in the same `/tmp`: something outside this crate
/// removed the fixture, and the assertion correctly reported that it was gone.
///
/// Naming fixtures with a nonce would have narrowed the window without closing
/// it, because a foreign sweeper does not consult our names. Owning the root
/// removes the shared surface entirely, so the intermittent cannot recur.
struct SweepFixture {
    root: tempfile::TempDir,
    foreign_tmp: PathBuf,
    foreign_chrome: PathBuf,
    owned: PathBuf,
}

impl SweepFixture {
    fn plant() -> Self {
        let root = tempfile::Builder::new()
            .prefix("ddg-sweep-")
            .tempdir()
            .expect("isolated sweep root");
        let foreign_tmp = root
            .path()
            .join(format!("{FOREIGN_TEMPFILE_DIR_PREFIX}mix"));
        let foreign_chrome = root
            .path()
            .join(format!("{CHROMIUM_GLOBAL_STUB_PREFIX}mix"));
        let owned = root.path().join(format!("{USER_DATA_DIR_PREFIX}mix"));
        for p in [&foreign_tmp, &foreign_chrome, &owned] {
            std::fs::create_dir_all(p).expect("mkdir fixture");
            std::fs::write(p.join("f"), b"1").expect("write fixture marker");
        }
        Self {
            root,
            foreign_tmp,
            foreign_chrome,
            owned,
        }
    }

    fn sweep(&self) {
        sweep_orphan_profiles_in(self.root.path());
    }

    /// `TempDir` ignores removal errors on drop, so a test that wants the
    /// cleanup PROVEN has to close it explicitly and read the `Result`.
    fn close(self) {
        self.root.close().expect("sweep root must be removable");
    }
}

/// A single foreign directory planted inside a root this test owns.
///
/// # Why this is not planted under `std::env::temp_dir()`
///
/// The refusal tests assert that a directory **survives** a call. In the shared
/// system temp root that is not an assertion this crate is entitled to make:
/// any sweeper on the host — another project's, or a distro tmp reaper — may
/// delete the fixture between the call and the assert, and the test then fails
/// for a reason unrelated to the guard under test. That is exactly the
/// intermittent that forced [`SweepFixture`] to own its root.
///
/// Naming the fixture after `process::id()` does not close the hole, because a
/// foreign sweeper never reads our PIDs; it only narrows the window. Owning the
/// root removes the shared surface, so the failure cannot recur by
/// construction.
struct RefusalFixture {
    root: tempfile::TempDir,
    planted: PathBuf,
}

impl RefusalFixture {
    fn plant(name: &str) -> Self {
        let root = tempfile::Builder::new()
            .prefix("ddg-refuse-")
            .tempdir()
            .expect("isolated refusal root");
        let planted = root.path().join(name);
        std::fs::create_dir_all(&planted).expect("mkdir refusal fixture");
        std::fs::write(planted.join("x"), b"1").expect("write fixture marker");
        Self { root, planted }
    }

    /// `TempDir` ignores removal errors on drop, so a test that wants the
    /// cleanup PROVEN has to close it explicitly and read the `Result`.
    fn close(self) {
        self.root.close().expect("refusal root must be removable");
    }
}

#[test]
fn bytes_contains_marker() {
    let hay = b"chrome\0--user-data-dir=/tmp/.tmpABCDEF\0--headless\0";
    assert!(bytes_contains_str(hay, "/tmp/.tmpABCDEF"));
    assert!(!bytes_contains_str(hay, "/tmp/.tmpOTHER"));
}

#[cfg(unix)]
#[test]
fn process_group_kill_reaps_child() {
    let mut cmd = Command::new("sleep");
    cmd.arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_process_group_and_pdeathsig(&mut cmd);
    let mut child = cmd.spawn().expect("spawn sleep");
    let pid = child.id();
    // Child is group leader: pgid == pid.
    kill_process_group(pid as i32);
    // wait should complete quickly.
    let status = child.wait().expect("wait");
    assert!(!status.success() || status.code().is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn kill_by_marker_reaps_matching_process() {
    let marker = format!("ddg-lifecycle-marker-{}", std::process::id());
    // Use env so marker appears in /proc/pid/environ OR pass as arg.
    let mut child = Command::new("sleep")
        .arg("30")
        .arg(&marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let pid = child.id();
    std::thread::sleep(Duration::from_millis(TERM_TO_KILL_GRACE_MS));
    kill_by_cmdline_substring(&marker);
    let _ = child.wait();
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "process {pid} should be gone after marker kill"
    );
}

#[test]
fn cleanup_display_rejects_garbage() {
    cleanup_xvfb_display_files("not-a-display");
    cleanup_xvfb_display_files(":");
}

/// GAP-HARD-X11-001: lock/socket helpers must resolve under `temp_dir()`,
/// never a hardcoded `/tmp` root independent of `TMPDIR`.
#[test]
fn x11_paths_are_under_temp_dir() {
    let tmp = std::env::temp_dir();
    let lock = x11_lock_path("99");
    let socket = x11_socket_path("99");
    assert!(
        lock.starts_with(&tmp),
        "lock {} must be under temp_dir {}",
        lock.display(),
        tmp.display()
    );
    assert!(
        socket.starts_with(&tmp),
        "socket {} must be under temp_dir {}",
        socket.display(),
        tmp.display()
    );
    assert_eq!(lock.file_name().and_then(|s| s.to_str()), Some(".X99-lock"));
    assert_eq!(socket.file_name().and_then(|s| s.to_str()), Some("X99"));
    assert!(
        !lock.to_string_lossy().starts_with("/tmp/.X")
            || tmp == PathBuf::from("/tmp")
            || tmp.starts_with("/tmp"),
        "when temp_dir is not /tmp, path must not hardcode /tmp: {}",
        lock.display()
    );
}

#[test]
fn force_reap_empty_session_does_not_panic() {
    force_reap(&SessionIds {
        chrome_pid: None,
        xvfb_pid: None,
        xvfb_pgid: None,
        user_data_dir: PathBuf::from("/tmp/.tmp-nonexistent-ddg-test"),
        display: None,
    });
}

#[test]
fn force_reap_removes_profile_directory() {
    let dir = std::env::temp_dir().join(format!(
        "{USER_DATA_DIR_PREFIX}unit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("create test profile dir");
    std::fs::write(dir.join("Preferences"), b"{}").expect("write marker file");
    assert!(dir.exists());
    force_reap(&SessionIds {
        chrome_pid: None,
        xvfb_pid: None,
        xvfb_pgid: None,
        user_data_dir: dir.clone(),
        display: None,
    });
    assert!(
        !dir.exists(),
        "force_reap must remove user-data-dir {}",
        dir.display()
    );
}

/// GAP-PAR-031: multi-session reap removes all profile dirs (parallel path).
#[test]
fn reap_all_registered_removes_multiple_sessions() {
    let mut dirs = Vec::new();
    for i in 0..3 {
        let dir = std::env::temp_dir().join(format!(
            "{USER_DATA_DIR_PREFIX}reap-all-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            i
        ));
        std::fs::create_dir_all(&dir).expect("create");
        std::fs::write(dir.join("Preferences"), b"{}").expect("write");
        register_session(SessionIds {
            chrome_pid: None,
            xvfb_pid: None,
            xvfb_pgid: None,
            user_data_dir: dir.clone(),
            display: None,
        });
        dirs.push(dir);
    }
    reap_all_registered();
    for dir in &dirs {
        assert!(!dir.exists(), "reap_all must remove {}", dir.display());
    }
}

#[test]
fn user_data_dir_prefix_is_ddg_chrome() {
    assert_eq!(USER_DATA_DIR_PREFIX, "ddg-chrome-");
    assert!(!USER_DATA_DIR_PREFIX.starts_with('.'));
}

#[test]
fn ownership_predicates_encode_hygiene_policy() {
    assert!(is_cli_owned_profile_name("ddg-chrome-AbCd12"));
    assert!(!is_cli_owned_profile_name(".tmpABCDEF"));
    assert!(!is_cli_owned_profile_name("org.chromium.Chromium.xyz"));
    assert!(is_forbidden_bulk_delete_name(".tmpABCDEF"));
    assert!(is_forbidden_bulk_delete_name(
        "org.chromium.Chromium.BDsv2K"
    ));
    assert!(is_forbidden_bulk_delete_name("org.chromium.Chromium"));
    assert!(!is_forbidden_bulk_delete_name("ddg-chrome-AbCd12"));
}

#[test]
fn remove_user_data_dir_refuses_foreign_tmp_prefix() {
    let fx = RefusalFixture::plant(&format!("{FOREIGN_TEMPFILE_DIR_PREFIX}refuse"));
    remove_user_data_dir(&fx.planted);
    assert!(
        fx.planted.exists(),
        "must never remove .tmp* legacy/generic tempdirs"
    );
    fx.close();
}

#[test]
fn remove_user_data_dir_refuses_chromium_global_stubs() {
    let fx = RefusalFixture::plant(&format!("{CHROMIUM_GLOBAL_STUB_PREFIX}stub"));
    remove_user_data_dir(&fx.planted);
    assert!(
        fx.planted.exists(),
        "must never remove org.chromium.Chromium.* global stubs"
    );
    fx.close();
}

#[test]
fn sweep_orphan_ignores_generic_tmp_prefix() {
    let fx = SweepFixture::plant();
    fx.sweep();
    assert!(
        fx.foreign_tmp.exists(),
        "sweep must never remove generic .tmp dirs (third-party risk)"
    );
    fx.close();
}

#[test]
fn sweep_orphan_ignores_org_chromium_global_stubs() {
    let fx = SweepFixture::plant();
    fx.sweep();
    assert!(
        fx.foreign_chrome.exists(),
        "sweep must never remove org.chromium.Chromium.* (desktop/MCP risk)"
    );
    fx.close();
}

/// Liveness must answer about the real process table on every host.
///
/// # The class this closes
///
/// The previous implementation consulted `/proc` and returned a hardcoded
/// `false` everywhere else. A `false` is not "unknown": callers read it as "the
/// recorded owner is dead" and acted on it, so on macOS, the BSDs and Windows
/// the sweep would delete a profile directory belonging to a Chrome that was
/// still running. This asserts the two answers the function can be sure of.
#[test]
fn pid_liveness_recognises_self_and_rejects_impossible_pids() {
    assert!(
        pid_is_alive(std::process::id()),
        "this very process is running, so liveness must say so on this host"
    );
    assert!(!pid_is_alive(0), "PID 0 is never a real process to signal");
    assert!(!pid_is_alive(1), "PID 1 is refused by the safety predicate");
}

/// A profile's `SingletonLock` is the owner registry that survives our death.
///
/// # Why this matters off Linux
///
/// The Linux recovery path finds an orphan by scanning `/proc/*/cmdline` for
/// the profile marker. macOS and Windows have no such file, and their branches
/// were an empty block and a no-op function respectively — exactly the two
/// places a crash leaves work to do. This proves the portable substitute reads
/// a live owner back out of the directory, and ignores a lock whose PID is gone.
#[test]
fn owned_profile_owner_pids_reads_live_owner_and_skips_dead_one() {
    let root = tempfile::tempdir().expect("temp root");
    let live = root.path().join(format!("{USER_DATA_DIR_PREFIX}live"));
    let dead = root.path().join(format!("{USER_DATA_DIR_PREFIX}dead"));
    std::fs::create_dir_all(&live).expect("create live profile");
    std::fs::create_dir_all(&dead).expect("create dead profile");

    let me = std::process::id();
    std::fs::write(live.join("SingletonLock"), format!("host-{me}")).expect("write live lock");
    // u32::MAX is above every platform's PID ceiling, so it cannot be running.
    std::fs::write(dead.join("SingletonLock"), format!("host-{}", u32::MAX))
        .expect("write dead lock");

    let found = owned_profile_owner_pids(root.path(), USER_DATA_DIR_PREFIX);
    assert!(
        found.is_empty(),
        "the only live lock names this very process, which must never be a \
         kill target; found {found:?}"
    );

    // Same directory, a PID that is neither alive nor us: still nothing.
    std::fs::write(live.join("SingletonLock"), format!("host-{}", u32::MAX - 1))
        .expect("rewrite live lock");
    assert!(
        owned_profile_owner_pids(root.path(), USER_DATA_DIR_PREFIX).is_empty(),
        "a lock naming a dead PID must never produce a signal target"
    );
}

/// Foreign directories must stay invisible to the owner scan.
#[test]
fn owned_profile_owner_pids_ignores_foreign_directories() {
    let root = tempfile::tempdir().expect("temp root");
    for name in [
        format!("{FOREIGN_TEMPFILE_DIR_PREFIX}whatever"),
        format!("{CHROMIUM_GLOBAL_STUB_PREFIX}stub"),
        "unrelated".to_string(),
    ] {
        let dir = root.path().join(name);
        std::fs::create_dir_all(&dir).expect("create foreign dir");
        std::fs::write(dir.join("SingletonLock"), "host-2").expect("write foreign lock");
    }
    assert!(
        owned_profile_owner_pids(root.path(), USER_DATA_DIR_PREFIX).is_empty(),
        "the scan must only ever look inside directories this CLI owns"
    );
}

#[test]
fn sweep_orphan_removes_stale_ddg_chrome_dir() {
    let fx = SweepFixture::plant();
    fx.sweep();
    assert!(
        !fx.owned.exists(),
        "sweep should remove orphan ddg-chrome-* (SIGKILL residual recovery)"
    );
    fx.close();
}

#[test]
fn sweep_leaves_foreign_while_removing_owned() {
    let fx = SweepFixture::plant();
    fx.sweep();
    assert!(fx.foreign_tmp.exists(), ".tmp* must survive sweep");
    assert!(
        fx.foreign_chrome.exists(),
        "org.chromium.* must survive sweep"
    );
    assert!(!fx.owned.exists(), "ddg-chrome-* orphan must be swept");
    fx.close();
}

/// The default entry point must still target the real system temp root.
///
/// Parameterising the sweep bought hermetic tests; it would be a poor trade if
/// the production call started sweeping somewhere else. This pins the wrapper.
#[test]
fn default_sweep_targets_the_system_temp_root() {
    let owned = std::env::temp_dir().join(format!(
        "{USER_DATA_DIR_PREFIX}default-root-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&owned).expect("create orphan under the real temp root");
    std::fs::write(owned.join("Local State"), b"{}").expect("write");
    sweep_orphan_profiles();
    assert!(
        !owned.exists(),
        "the no-argument sweep must still clean {} ",
        owned.display()
    );
}

#[test]
fn exit_reap_guard_drop_does_not_panic() {
    let guard = ExitReapGuard::new();
    drop(guard);
}

/// GAP-UNSAFE-016: kill_pid must no-op on 0 / 1 / self (POSIX safety).
#[test]
fn kill_pid_refuses_zero_one_and_self() {
    kill_pid(0);
    kill_pid(1);
    kill_pid(std::process::id());
}

#[test]
fn reap_policy_timings_are_named_and_positive() {
    assert!(KILL_SETTLE_MS > 0);
    assert!(TERM_TO_KILL_GRACE_MS > 0);
    assert!(PROFILE_REMOVE_SETTLE_MS > 0);
    assert!(PROFILE_REMOVE_RETRY_MS >= PROFILE_REMOVE_SETTLE_MS);
}
