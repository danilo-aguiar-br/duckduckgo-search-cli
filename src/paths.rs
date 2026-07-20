// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (path validation and sanitization)
//! Path validation and sanitization for I/O operations.
//!
//! This module centralizes validation of output paths provided by the
//! user via `--output` / `--cookies-path`, preventing path traversal and
//! writes to system directories. Also encapsulates parent directory
//! creation, atomic writes, and Unix permissions application.
//!
//! # Threat model
//!
//! - CLI path arguments are **hostile**: reject `..`, protected roots, Windows
//!   reserved device names.
//! - TOCTOU: prefer atomic write + open-by-path over check-then-use for content;
//!   existence checks for parent dirs are best-effort (create_dir_all).
//! - Symlink following on absolute paths is a residual host-admin concern for a
//!   local CLI (user owns the process); we do not follow `..` components.

use crate::error::CliError;
use std::path::{Component, Path, PathBuf};

const PROTECTED_UNIX: &[&str] = &[
    "/etc", "/usr", "/bin", "/sbin", "/boot", "/proc", "/sys", "/dev",
];
const PROTECTED_WINDOWS: &[&str] = &[
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
];

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validates an output path provided by the user.
///
/// Rejects paths containing `..` components (path traversal), absolute
/// paths pointing to protected system directories, and filenames using
/// Windows reserved device names (CON, PRN, AUX, NUL, COM0-9, LPT0-9).
///
/// On Windows, paths longer than 260 characters may fail unless the
/// application manifest includes `longPathAware`. This is a known limitation.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if the path contains `..` components,
/// points to a protected system directory, or uses a Windows reserved name.
pub fn validate_output_path(path: &Path) -> Result<PathBuf, CliError> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(CliError::PathError {
                message: format!(
                    "output path rejected — contains '..' (path traversal): {}",
                    path.display()
                ),
            });
        }
    }

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let upper = stem.to_ascii_uppercase();
        if WINDOWS_RESERVED_NAMES.contains(&upper.as_str()) {
            return Err(CliError::PathError {
                message: format!(
                    "output path rejected — uses Windows reserved name: {}",
                    path.display()
                ),
            });
        }
    }

    if path.is_absolute() {
        let path_str = path.to_string_lossy();

        for dir in PROTECTED_UNIX {
            if path_str.starts_with(dir) {
                return Err(CliError::PathError {
                    message: format!(
                        "output path rejected — points to system directory: {}",
                        path.display()
                    ),
                });
            }
        }
        for dir in PROTECTED_WINDOWS {
            if path_str.to_lowercase().starts_with(&dir.to_lowercase()) {
                return Err(CliError::PathError {
                    message: format!(
                        "output path rejected — points to system directory: {}",
                        path.display()
                    ),
                });
            }
        }
    }

    Ok(path.to_path_buf())
}

/// Creates parent directories of a path, if needed.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if the underlying filesystem call to
/// create directories fails.
pub fn create_parent_dirs(path: &Path) -> Result<(), CliError> {
    if let Some(parent_dir) = path.parent() {
        if !parent_dir.as_os_str().is_empty() && !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir).map_err(|e| CliError::PathError {
                message: format!(
                    "failed to create parent directories: {}: {e}",
                    parent_dir.display()
                ),
            })?;
        }
    }
    Ok(())
}

/// Atomically writes `content` to `path` (rules-rust atomwrite / GAP-WS-LIFECYCLE-001 L-10).
///
/// Sequence: tempfile in the **same directory** as the target → `write_all` →
/// `flush` → `sync_data` → `persist` (atomic rename) → fsync parent dir on Unix.
///
/// Callers apply permissions after success when needed (`apply_permissions_644` /
/// `0o600` for cookie jars).
///
/// # Errors
///
/// Returns [`CliError::PathError`] on I/O failures.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), CliError> {
    create_parent_dirs(path)?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut temp = tempfile::Builder::new()
        .prefix(".ddg-atomic-")
        .tempfile_in(dir)
        .map_err(|e| CliError::PathError {
            message: format!("failed to create atomic tempfile in {}: {e}", dir.display()),
        })?;

    use std::io::Write;
    temp.write_all(content).map_err(|e| CliError::PathError {
        message: format!(
            "failed to write atomic tempfile for {}: {e}",
            path.display()
        ),
    })?;
    temp.flush().map_err(|e| CliError::PathError {
        message: format!(
            "failed to flush atomic tempfile for {}: {e}",
            path.display()
        ),
    })?;
    temp.as_file()
        .sync_data()
        .map_err(|e| CliError::PathError {
            message: format!(
                "failed to sync_data atomic tempfile for {}: {e}",
                path.display()
            ),
        })?;

    temp.persist(path).map_err(|e| CliError::PathError {
        message: format!("failed to persist atomic write to {}: {e}", path.display()),
    })?;

    // Best-effort fsync of the parent directory so the rename is durable.
    #[cfg(unix)]
    if let Some(parent_dir) = parent {
        if let Ok(dir_file) = std::fs::File::open(parent_dir) {
            let _ = dir_file.sync_all();
        }
    }

    Ok(())
}

/// Applies 0o644 permissions to a file on Unix (owner reads+writes, others read).
/// No-op on non-Unix platforms.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if setting file permissions fails
/// (e.g. permission denied or the file no longer exists).
#[cfg(unix)]
pub fn apply_permissions_644(path: &Path) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o644);
    std::fs::set_permissions(path, permissions).map_err(|e| CliError::PathError {
        message: format!(
            "failed to apply 0o644 permissions on {}: {e}",
            path.display()
        ),
    })?;
    Ok(())
}

/// # Errors
///
/// Always returns `Ok(())` on non-Unix platforms.
#[cfg(not(unix))]
pub fn apply_permissions_644(_path: &Path) -> Result<(), CliError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rejects_path_with_parent_dir() {
        let result = validate_output_path(Path::new("../../etc/passwd"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("path traversal"), "mensagem: {msg}");
    }

    #[test]
    fn rejects_path_with_parent_dir_in_middle() {
        let result = validate_output_path(Path::new("output/../../../evil.json"));
        assert!(result.is_err());
    }

    #[test]
    fn aceita_path_relativo_simples() {
        let result = validate_output_path(Path::new("output/resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn accepts_relative_path_with_single_dot() {
        let result = validate_output_path(Path::new("./resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn aceita_path_absoluto_tmp() {
        let result = validate_output_path(Path::new("/tmp/ddg_resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(unix)] // /etc and /usr are Unix-only paths; on Windows they are regular
    fn rejeita_path_absoluto_etc() {
        let result = validate_output_path(Path::new("/etc/shadow"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("system directory"), "message: {msg}");
    }

    #[test]
    #[cfg(unix)] // /usr is a Unix-only path; on Windows C:\usr is regular
    fn rejeita_path_absoluto_usr() {
        let result = validate_output_path(Path::new("/usr/bin/evil"));
        assert!(result.is_err());
    }

    #[test]
    fn aceita_path_absoluto_home() {
        let result = validate_output_path(Path::new("/home/user/resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn create_parent_dirs_with_tempdir() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("sub").join("resultado.json");
        let result = create_parent_dirs(&path);
        assert!(result.is_ok());
        assert!(path.parent().expect("has parent").exists());
    }

    #[test]
    fn atomic_write_roundtrip_and_replace() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("out.txt");
        atomic_write(&path, b"first").expect("write first");
        assert_eq!(std::fs::read(&path).expect("read"), b"first");
        atomic_write(&path, b"second").expect("replace");
        assert_eq!(std::fs::read(&path).expect("read2"), b"second");
        // No leftover .ddg-atomic-* temps in the directory.
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .expect("readdir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".ddg-atomic-"))
            .collect();
        assert!(leftovers.is_empty(), "leftover temps: {leftovers:?}");
    }

    #[test]
    fn simple_filename_without_parent() {
        let result = validate_output_path(Path::new("resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn rejeita_nome_reservado_windows_nul() {
        let result = validate_output_path(Path::new("NUL.json"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Windows reserved name"), "mensagem: {msg}");
    }

    #[test]
    fn rejeita_nome_reservado_windows_con_case_insensitive() {
        let result = validate_output_path(Path::new("con.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn rejeita_nome_reservado_windows_com1() {
        let result = validate_output_path(Path::new("output/COM1.json"));
        assert!(result.is_err());
    }

    #[test]
    fn accepts_non_reserved_name_content() {
        let result = validate_output_path(Path::new("conteudo.json"));
        assert!(result.is_ok());
    }
}
