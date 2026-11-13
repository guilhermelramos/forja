//! Teste ponta a ponta real da Forja (`tests/e2e/smoke.ps1`).
//!
//! Execução: `cargo test -p forja-app -- --ignored e2e`.

use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "launches real forja-app and drives all 13 CLI workflows"]
fn e2e() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|dir| dir.parent())
        .expect("crates/shell lives two levels below the repository root");
    let script = repo_root.join("tests").join("e2e").join("smoke.ps1");
    assert!(script.exists(), "missing smoke script at {}", script.display());

    let exe = std::env::current_exe().expect("current test exe path");
    let bin_dir = exe
        .parent()
        .and_then(|dir| dir.parent())
        .expect("test binary lives under target/<profile>/deps")
        .to_path_buf();

    let shell = if Command::new("pwsh.exe").arg("-Version").output().is_ok() {
        "pwsh.exe"
    } else {
        "powershell.exe"
    };

    let output = Command::new(shell)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script.display().to_string(),
            "-BinDir",
            &bin_dir.display().to_string(),
        ])
        .current_dir(repo_root)
        .output()
        .expect("run smoke.ps1");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{stdout}");
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    assert!(
        output.status.success(),
        "e2e smoke failed with exit code {:?}",
        output.status.code()
    );
}
