// Build-time preflight for the BoringSSL toolchain pulled in by `wreq` ->
// `btls-sys`. On native Windows MSVC builds, the BoringSSL CMake build requires
// four tools that the VS Installer does not bundle by default:
//   - NASM assembler (GAP-WS-28, fixed in v0.7.4)
//   - CMake 3.20+ (GAP-WS-29, fixed in v0.7.5)
//   - MSVC C/C++ compiler (cl.exe) + linker (link.exe) (GAP-WS-30, fixed in v0.7.5)
//   - Perl interpreter (GAP-WS-31, fixed in v0.7.5)
//
// The `btls-sys` build script only defines `OPENSSL_NO_ASM` for cross-compiles
// (its host == target early-return skips that branch), which means a missing
// NASM surfaces minutes into the build as the cryptic CMake error
// "No CMAKE_ASM_NASM_COMPILER could be found". The `cmake` crate 0.1.58 also
// panics on missing `cmake.exe` (in `lib.rs:1132`) before the CMake script even
// runs, surfacing as "program not found / is 'cmake' not installed?".
// This check fails fast with actionable messages instead. See gaps.md
// (GAP-WS-28, GAP-WS-29, GAP-WS-30, GAP-WS-31).
fn main() {
    println!("cargo:rerun-if-env-changed=DDG_SKIP_NASM_CHECK");
    println!("cargo:rerun-if-env-changed=DDG_SKIP_CMAKE_CHECK");
    println!("cargo:rerun-if-env-changed=DDG_SKIP_MSVC_CHECK");
    println!("cargo:rerun-if-env-changed=DDG_SKIP_PERL_CHECK");
    println!("cargo:rerun-if-env-changed=PATH");
    if std::env::var_os("DDG_SKIP_NASM_CHECK").is_some() {
        return;
    }
    if std::env::var_os("DDG_SKIP_CMAKE_CHECK").is_some() {
        return;
    }
    if std::env::var_os("DDG_SKIP_MSVC_CHECK").is_some() {
        return;
    }
    if std::env::var_os("DDG_SKIP_PERL_CHECK").is_some() {
        return;
    }
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let host = std::env::var("HOST").unwrap_or_default();
    // Native Windows MSVC builds only: cross-compiles to Windows take the
    // OPENSSL_NO_ASM path inside btls-sys and do not need any of these tools.
    if target_os != "windows" || target_env != "msvc" || !host.contains("windows") {
        return;
    }
    // GAP-WS-28 (v0.7.4+): NASM is required for BoringSSL's enable_language(ASM_NASM).
    if !nasm_in_path() {
        let hint = known_nasm_dir().map_or(String::new(), |dir| {
            format!("\nNASM was found at \"{dir}\" but that directory is not in PATH.\n")
        });
        panic!(
            "\n\
            NASM assembler not found in PATH.\n\
            {hint}\n\
            duckduckgo-search-cli v0.7.3+ links BoringSSL (via wreq/btls-sys), which\n\
            requires NASM to assemble its crypto routines on Windows MSVC targets.\n\
            Without this check, the build would fail minutes later with:\n\
            \"CMake Error: No CMAKE_ASM_NASM_COMPILER could be found\".\n\
            \n\
            Fix (PowerShell):\n\
            \x20 winget install -e --id NASM.NASM\n\
            \x20 $env:Path += \";C:\\Program Files\\NASM\"\n\
            then re-run: cargo install duckduckgo-search-cli\n\
            \n\
            Or run the helper script: scripts/install-windows.ps1\n\
            Set DDG_SKIP_NASM_CHECK=1 to bypass this preflight.\n"
        );
    }
    // GAP-WS-29 (v0.7.5+): CMake is required by the `cmake` crate 0.1.58 BEFORE
    // enable_language(ASM_NASM) is evaluated, so the NASM preflight above
    // cannot catch a missing cmake.
    if !cmake_in_path() {
        let hint = known_cmake_dir().map_or(String::new(), |dir| {
            format!("\nCMake was found at \"{dir}\" but that directory is not in PATH.\n")
        });
        panic!(
            "\n\
            CMake not found in PATH.\n\
            {hint}\n\
            duckduckgo-search-cli v0.7.3+ requires CMake to build BoringSSL (via wreq/btls-sys).\n\
            Without this check, the build would fail 30-60s later with:\n\
            \"failed to execute command: program not found / is 'cmake' not installed?\".\n\
            \n\
            Fix A — Visual Studio Installer (preferred):\n\
            \x20 Open Visual Studio Installer -> Modify on your VS 2022 install ->\n\
            \x20 Workloads -> check \"Desktop development with C++\" ->\n\
            \x20 Installation details (right pane) -> expand that workload ->\n\
            \x20 check \"C++ CMake tools for Windows\" (desmarcado por default!) ->\n\
            \x20 click Modify.\n\
            \n\
            Fix B — Standalone CMake via winget:\n\
            \x20 winget install -e --id Kitware.CMake\n\
            \n\
            Fix C — Chocolatey:\n\
            \x20 choco install cmake -y --installargs 'ADD_CMAKE_TO_PATH=System'\n\
            \n\
            Or run the helper script: scripts/install-windows.ps1\n\
            Set DDG_SKIP_CMAKE_CHECK=1 to bypass this preflight.\n"
        );
    }
    // GAP-WS-30 (v0.7.5+): MSVC C/C++ compiler + linker are required for any
    // CMake build that uses the "Visual Studio 17 2022" generator.
    if !cl_in_path() || !link_in_path() {
        let missing: Vec<&str> = [
            if cl_in_path() { None } else { Some("cl.exe") },
            if link_in_path() {
                None
            } else {
                Some("link.exe")
            },
        ]
        .into_iter()
        .flatten()
        .collect();
        panic!(
            "\n\
            MSVC toolchain incomplete — missing: {}\n\
            duckduckgo-search-cli v0.7.3+ requires MSVC to compile and link\n\
            BoringSSL on Windows MSVC targets. CMake's \"Visual Studio 17 2022\"\n\
            generator calls `cl.exe` and `link.exe` directly.\n\
            \n\
            Fix A — open Developer Command Prompt for VS 2022:\n\
            \x20 Start menu -> \"Developer Command Prompt for VS 2022\"\n\
            (this shell has PATH, INCLUDE, and LIB pre-set for MSVC)\n\
            \n\
            Fix B — from a regular PowerShell, run:\n\
            \x20 & \"C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\Common7\\Tools\\Launch-VsDevShell.ps1\"\n\
            (adjust path for BuildTools / Professional / Enterprise edition)\n\
            \n\
            Fix C — install Visual Studio Build Tools 2019+ with the C++ workload:\n\
            \x20 https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022\n\
            \n\
            Or run the helper script: scripts/install-windows.ps1\n\
            Set DDG_SKIP_MSVC_CHECK=1 to bypass this preflight.\n",
            missing.join(", ")
        );
    }
    // GAP-WS-31 (v0.7.5+): Perl is required by BoringSSL's perlasm generator
    // which emits crypto assembly in NASM format.
    if !perl_in_path() {
        let hint = known_perl_dir().map_or(String::new(), |dir| {
            format!("\nPerl was found at \"{dir}\" but that directory is not in PATH.\n")
        });
        panic!(
            "\n\
            Perl interpreter not found in PATH.\n\
            {hint}\n\
            duckduckgo-search-cli v0.7.3+ requires Perl to generate BoringSSL\n\
            crypto assembly on Windows MSVC targets (perlasm -> NASM pipeline).\n\
            \n\
            Fix A — Strawberry Perl via winget (preferred):\n\
            \x20 winget install -e --id StrawberryPerl.StrawberryPerl\n\
            \n\
            Fix B — Chocolatey:\n\
            \x20 choco install strawberryperl -y\n\
            \n\
            Or run the helper script: scripts/install-windows.ps1\n\
            Set DDG_SKIP_PERL_CHECK=1 to bypass this preflight.\n"
        );
    }
}

fn nasm_in_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("nasm.exe").is_file())
    })
}

fn cmake_in_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("cmake.exe").is_file())
    })
}

fn cl_in_path() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("cl.exe").is_file()))
}

fn link_in_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("link.exe").is_file())
    })
}

fn perl_in_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("perl.exe").is_file())
    })
}

/// Common NASM install locations whose installer does not update PATH.
fn known_nasm_dir() -> Option<String> {
    ["C:\\Program Files\\NASM", "C:\\Program Files (x86)\\NASM"]
        .into_iter()
        .find(|dir| std::path::Path::new(dir).join("nasm.exe").is_file())
        .map(str::to_owned)
}

/// Common `CMake` install locations — standalone Kitware and the Visual Studio
/// Installer "C++ `CMake` tools for `Windows`" sub-component.
fn known_cmake_dir() -> Option<String> {
    [
        "C:\\Program Files\\CMake\\bin",
        "C:\\Program Files (x86)\\CMake\\bin",
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin",
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin",
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\Professional\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin",
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\\Common7\\IDE\\CommonExtensions\\Microsoft\\CMake\\CMake\\bin",
    ]
    .into_iter()
    .find(|dir| std::path::Path::new(dir).join("cmake.exe").is_file())
    .map(str::to_owned)
}

/// Common Perl install locations — Strawberry Perl is the de-facto Windows Perl
/// for `BoringSSL` builds; `ActiveState` `Perl` may also work.
fn known_perl_dir() -> Option<String> {
    [
        "C:\\Strawberry\\perl\\bin",
        "C:\\Strawberry64\\perl\\bin",
        "C:\\Perl64\\bin",
        "C:\\Perl\\bin",
    ]
    .into_iter()
    .find(|dir| std::path::Path::new(dir).join("perl.exe").is_file())
    .map(str::to_owned)
}
