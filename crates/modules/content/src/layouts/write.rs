//! Escrita, remoção e confinamento dos arquivos de arranjo.

use forja_kernel::layout_spec::{validate_segment, LayoutSpecV1};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Caminho seguro para `<root>/layouts/<id>.toml`.
pub fn layout_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    validate_segment("id", id).map_err(|error| error.to_string())?;
    let directory = root.join(super::LAYOUT_DIR);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let root_canon = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let dir_canon = std::fs::canonicalize(&directory).map_err(|error| error.to_string())?;
    if !dir_canon.starts_with(&root_canon) {
        return Err("layouts folder escapes the content root".to_owned());
    }
    let name = format!("{id}.toml");
    let path = dir_canon.join(&name);
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err(format!("`{name}` is a link"));
        }
    }
    Ok(path)
}

fn current_hash(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| crate::model::sha256_hex(&text))
}

/// Gravação atômica. `expected_hash` detecta revisão concorrente.
pub fn write_layout(
    root: &Path,
    id: &str,
    text: &str,
    expected_hash: Option<&str>,
) -> Result<String, String> {
    let spec = LayoutSpecV1::parse(text).map_err(|error| error.to_string())?;
    if spec.id != id {
        return Err(format!("file must be named `{}.toml`", spec.id));
    }
    let path = layout_path(root, id)?;
    if let Some(expected) = expected_hash {
        if current_hash(&path).as_deref() != Some(expected) {
            return Err("layout changed since it was opened".to_owned());
        }
    }
    let hash = crate::model::sha256_hex(text);
    forja_kernel::store::write_atomic_bytes(&path, text.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(hash)
}

/// Remoção atômica: rename para um irmão temporário e só depois delete.
pub fn remove_layout(root: &Path, id: &str, expected_hash: Option<&str>) -> Result<(), String> {
    let path = layout_path(root, id)?;
    if let Some(expected) = expected_hash {
        if current_hash(&path).as_deref() != Some(expected) {
            return Err("layout changed since it was opened".to_owned());
        }
    }
    if !path.is_file() {
        return Err(format!("layout `{id}` does not exist in this layer"));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let temporary = path.with_extension(format!("remove-{}-{nanos}", std::process::id()));
    std::fs::rename(&path, &temporary).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::remove_file(&temporary) {
        let _ = std::fs::rename(&temporary, &path);
        return Err(error.to_string());
    }
    Ok(())
}
