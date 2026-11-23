//! Escrita, remoção e confinamento dos arquivos de arranjo.

use forja_kernel::layout_spec::{validate_segment, LayoutSpecV1};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

/// Links, junctions e outros reparse points são recusados: canonicalizar
/// seguiria o redirecionamento e aceitaria um destino fora da raiz.
fn is_redirect(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Verifica um componente sem seguir links. `create` vale para diretórios.
fn check_component(path: &Path, create: bool) -> Result<(), String> {
    if create && !path.exists() {
        std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if is_redirect(&metadata) => {
            Err(format!("`{}` is a link or junction", path.display()))
        }
        Ok(metadata) if create && !metadata.is_dir() => {
            Err(format!("`{}` is not a folder", path.display()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Caminho seguro para `<root>/layouts/<id>.toml`, componente a componente.
pub fn layout_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    validate_segment("id", id).map_err(|error| error.to_string())?;
    let directory = root.join(super::LAYOUT_DIR);
    check_component(&directory, true)?;
    let path = directory.join(format!("{id}.toml"));
    check_component(&path, false)?;
    if path.exists() && !path.is_file() {
        return Err(format!("`{}` is not a file", path.display()));
    }
    Ok(path)
}

/// Raiz `.forja` de um projeto, sem seguir junções no próprio `.forja`.
pub fn project_layout_root(project: &Path) -> Result<PathBuf, String> {
    let root = project.join(".forja");
    check_component(&root, true)?;
    Ok(root)
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
    if std::fs::symlink_metadata(&temporary).is_ok() {
        return Err("removal temporary already exists".to_owned());
    }
    // A visibilidade do arquivo termina no rename: nunca devolvemos o destino
    // ao estado anterior, para não competir com um save concorrente.
    std::fs::rename(&path, &temporary).map_err(|error| error.to_string())?;
    std::fs::remove_file(&temporary).map_err(|error| error.to_string())

}
