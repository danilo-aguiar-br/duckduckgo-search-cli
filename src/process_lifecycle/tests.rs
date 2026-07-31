// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for process_lifecycle.

use super::*;
use std::process::{Command, Stdio};

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
        assert_eq!(
            lock.file_name().and_then(|s| s.to_str()),
            Some(".X99-lock")
        );
        assert_eq!(
            socket.file_name().and_then(|s| s.to_str()),
            Some("X99")
        );
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
            "{USER_DATA_DIR_PREFIX}unit-{}",
            std::process::id()
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
                "{USER_DATA_DIR_PREFIX}reap-all-{}-{}",
                std::process::id(),
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
            assert!(
                !dir.exists(),
                "reap_all must remove {}",
                dir.display()
            );
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
        assert!(is_forbidden_bulk_delete_name("org.chromium.Chromium.BDsv2K"));
        assert!(is_forbidden_bulk_delete_name("org.chromium.Chromium"));
        assert!(!is_forbidden_bulk_delete_name("ddg-chrome-AbCd12"));
    }

    #[test]
    fn remove_user_data_dir_refuses_foreign_tmp_prefix() {
        let alien = std::env::temp_dir().join(format!(
            "{FOREIGN_TEMPFILE_DIR_PREFIX}refuse-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&alien).expect("mkdir .tmp alien");
        std::fs::write(alien.join("x"), b"1").expect("write");
        remove_user_data_dir(&alien);
        assert!(
            alien.exists(),
            "must never remove .tmp* legacy/generic tempdirs"
        );
        let _ = std::fs::remove_dir_all(&alien);
    }

    #[test]
    fn remove_user_data_dir_refuses_chromium_global_stubs() {
        let stub = std::env::temp_dir().join(format!(
            "{CHROMIUM_GLOBAL_STUB_PREFIX}stub-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&stub).expect("mkdir chromium stub");
        std::fs::write(stub.join("x"), b"1").expect("write");
        remove_user_data_dir(&stub);
        assert!(
            stub.exists(),
            "must never remove org.chromium.Chromium.* global stubs"
        );
        let _ = std::fs::remove_dir_all(&stub);
    }

    #[test]
    fn sweep_orphan_ignores_generic_tmp_prefix() {
        let alien = std::env::temp_dir().join(format!(
            "{FOREIGN_TEMPFILE_DIR_PREFIX}ddg-sweep-alien-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&alien);
        sweep_orphan_profiles();
        assert!(
            alien.exists(),
            "sweep must never remove generic .tmp dirs (third-party risk)"
        );
        let _ = std::fs::remove_dir_all(&alien);
    }

    #[test]
    fn sweep_orphan_ignores_org_chromium_global_stubs() {
        let stub = std::env::temp_dir().join(format!(
            "{CHROMIUM_GLOBAL_STUB_PREFIX}sweep-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&stub).expect("mkdir stub");
        std::fs::write(stub.join("socket"), b"").expect("write");
        sweep_orphan_profiles();
        assert!(
            stub.exists(),
            "sweep must never remove org.chromium.Chromium.* (desktop/MCP risk)"
        );
        let _ = std::fs::remove_dir_all(&stub);
    }

    #[test]
    fn sweep_orphan_removes_stale_ddg_chrome_dir() {
        let dir = std::env::temp_dir().join(format!(
            "{USER_DATA_DIR_PREFIX}orphan-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create orphan");
        std::fs::write(dir.join("Local State"), b"{}").expect("write");
        sweep_orphan_profiles();
        assert!(
            !dir.exists(),
            "sweep should remove orphan ddg-chrome-* (SIGKILL residual recovery)"
        );
    }

    #[test]
    fn sweep_leaves_foreign_while_removing_owned() {
        let pid = std::process::id();
        let foreign_tmp = std::env::temp_dir().join(format!("{FOREIGN_TEMPFILE_DIR_PREFIX}mix-{pid}"));
        let foreign_chrome =
            std::env::temp_dir().join(format!("{CHROMIUM_GLOBAL_STUB_PREFIX}mix-{pid}"));
        let owned = std::env::temp_dir().join(format!("{USER_DATA_DIR_PREFIX}mix-{pid}"));
        for p in [&foreign_tmp, &foreign_chrome, &owned] {
            std::fs::create_dir_all(p).expect("mkdir");
            std::fs::write(p.join("f"), b"1").expect("write");
        }
        sweep_orphan_profiles();
        assert!(foreign_tmp.exists(), ".tmp* must survive sweep");
        assert!(foreign_chrome.exists(), "org.chromium.* must survive sweep");
        assert!(!owned.exists(), "ddg-chrome-* orphan must be swept");
        let _ = std::fs::remove_dir_all(&foreign_tmp);
        let _ = std::fs::remove_dir_all(&foreign_chrome);
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
