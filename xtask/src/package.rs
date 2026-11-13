//! `cargo xtask package`: builds the release and stages `dist/`.
//!
//! The zip is written in stored (uncompressed) mode by this file; no external
//! zip tool is required. Only `dist/` is written.

use crate::util::{repo_root, write};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn run(skip_build: bool) -> Result<(), String> {
    let root = repo_root();
    let version = workspace_version(&root)?;
    let name = format!("forja-{version}-win-x64");
    let dist = root.join("dist");
    let stage = dist.join(&name);
    let zip = dist.join(format!("{name}.zip"));

    if !skip_build {
        let status = std::process::Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&root)
            .status()
            .map_err(|error| format!("could not run cargo: {error}"))?;
        if !status.success() {
            return Err(format!("cargo build --release failed with {status}"));
        }
    }

    let release = root.join("target").join("release");
    // WebView2Loader.dll is linked at load time by forja-app.exe (browser tab);
    // without it next to the exe the app does not start.
    let binaries = [
        "forja-app.exe",
        "forja.exe",
        "forja-hook.exe",
        "forja-mcp.exe",
        "WebView2Loader.dll",
    ];
    for binary in binaries {
        if !release.join(binary).exists() {
            return Err(format!(
                "{} is missing from target/release; run the build first",
                binary
            ));
        }
    }

    if stage.exists() {
        std::fs::remove_dir_all(&stage)
            .map_err(|error| format!("{}: {error}", stage.display()))?;
    }
    if zip.exists() {
        std::fs::remove_file(&zip).map_err(|error| format!("{}: {error}", zip.display()))?;
    }
    std::fs::create_dir_all(&stage).map_err(|error| error.to_string())?;

    for binary in binaries {
        std::fs::copy(release.join(binary), stage.join(binary))
            .map_err(|error| format!("{}: {error}", binary))?;
    }
    for file in ["README.md", "LICENSE", "NOTICE"] {
        let source = root.join(file);
        if source.exists() {
            std::fs::copy(&source, stage.join(file))
                .map_err(|error| format!("{}: {error}", file))?;
        }
    }
    let docs = stage.join("docs");
    std::fs::create_dir_all(&docs).map_err(|error| error.to_string())?;
    let modules_doc = root.join("docs").join("modules.md");
    if modules_doc.exists() {
        std::fs::copy(&modules_doc, docs.join("modules.md"))
            .map_err(|error| error.to_string())?;
    }
    copy_tree(&root.join("content"), &stage.join("content"))?;
    copy_tree(&root.join("extensions"), &stage.join("extensions"))?;
    write(&stage.join("INSTALL.txt"), &install_text(&version))?;

    let bytes = zip_dir(&stage, &zip, &name)?;
    let digest = Sha256::digest(std::fs::read(&zip).map_err(|error| error.to_string())?);
    let hash: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    println!("Package: {}", zip.display());
    println!("Size:    {bytes} bytes ({:.2} MB)", bytes as f64 / 1_048_576.0);
    println!("SHA-256: {hash}");
    Ok(())
}

pub(crate) fn workspace_version(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("Cargo.toml: {error}"))?;
    let value: toml::Value = toml::from_str(&text).map_err(|error| error.to_string())?;
    value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .or_else(|| {
            value
                .get("package")
                .and_then(|package| package.get("version"))
                .and_then(toml::Value::as_str)
        })
        .map(str::to_owned)
        .ok_or_else(|| "no version found in Cargo.toml".to_owned())
}

fn install_text(version: &str) -> String {
    format!(
        "Forja {version} (win-x64)\n\
         \n\
         Execute forja-app.exe para abrir a oficina.\n\
         O CLI forja.exe conversa com o app aberto; forja-hook.exe e usado pelos\n\
         ajudantes para enviar eventos de status; forja-mcp.exe liga clientes MCP ao app.\n\
         WebView2Loader.dll precisa ficar na mesma pasta de forja-app.exe.\n\
         \n\
         Dados de estado: %APPDATA%\\Forja\n\
         Conteudo do usuario: %USERPROFILE%\\.forja\n"
    )
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else if path.is_file() {
            std::fs::copy(&path, &target)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn zip_dir(stage: &Path, zip: &Path, prefix: &str) -> Result<u64, String> {
    let mut files = Vec::new();
    walk(stage, stage, &mut files)?;
    files.sort();
    if files.len() > u16::MAX as usize {
        return Err("too many files for a zip without zip64".to_owned());
    }
    let mut output: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for relative in &files {
        let data = std::fs::read(stage.join(relative))
            .map_err(|error| format!("{}: {error}", relative.display()))?;
        let name = format!(
            "{prefix}/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        let crc = crc32(&data);
        let offset = u32::try_from(output.len()).map_err(|_| "zip is too large".to_owned())?;
        write_local(&mut output, &name, crc, data.len() as u32);
        output.extend_from_slice(&data);
        write_central(&mut central, &name, crc, data.len() as u32, offset);
    }
    let central_offset =
        u32::try_from(output.len()).map_err(|_| "zip is too large".to_owned())?;
    let central_size =
        u32::try_from(central.len()).map_err(|_| "zip is too large".to_owned())?;
    output.extend_from_slice(&central);
    push_u32(&mut output, 0x0605_4b50);
    push_u16(&mut output, 0);
    push_u16(&mut output, 0);
    push_u16(&mut output, files.len() as u16);
    push_u16(&mut output, files.len() as u16);
    push_u32(&mut output, central_size);
    push_u32(&mut output, central_offset);
    push_u16(&mut output, 0);
    let mut file = std::fs::File::create(zip).map_err(|error| error.to_string())?;
    file.write_all(&output).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(output.len() as u64)
}

fn walk(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files)?;
        } else if path.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn write_local(out: &mut Vec<u8>, name: &str, crc: u32, size: u32) {
    push_u32(out, 0x0403_4b50);
    push_u16(out, 20);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0x0021);
    push_u32(out, crc);
    push_u32(out, size);
    push_u32(out, size);
    push_u16(out, name.len() as u16);
    push_u16(out, 0);
    out.extend_from_slice(name.as_bytes());
}

fn write_central(out: &mut Vec<u8>, name: &str, crc: u32, size: u32, offset: u32) {
    push_u32(out, 0x0201_4b50);
    push_u16(out, 20);
    push_u16(out, 20);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0x0021);
    push_u32(out, crc);
    push_u32(out, size);
    push_u32(out, size);
    push_u16(out, name.len() as u16);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u16(out, 0);
    push_u32(out, 0);
    push_u32(out, offset);
    out.extend_from_slice(name.as_bytes());
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
