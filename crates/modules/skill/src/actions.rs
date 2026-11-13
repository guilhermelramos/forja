//! Operações com efeito: ler as raízes, instalar, copiar, remover e ler o `SKILL.md`.
//!
//! Na thread da interface ficam as checagens baratas (a raiz existe, o caminho está na lista, o
//! consentimento foi dado). O disco roda em `Ctx::spawn`, e o resultado volta por `finish`. Nada
//! disso grava durante os dados de captura (`FORJA_FIXTURES`).

use crate::discovery;
use crate::fixtures;
use crate::install::{self, Placed, SystemTrash};
use crate::model::{Listing, Root, Scope, Skill, SkillError};
use crate::sources;
use crate::state::{fixtures_enabled, InstallKind, Notice, Pending, Shared};
use forja_kernel::{Args, Ctx};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tamanho máximo do `SKILL.md` mostrado na tela.
pub const BODY_LIMIT: u64 = 256 * 1024;
/// Quantos registros de consentimento ficam guardados.
const CONSENT_LIMIT: usize = 500;

/// Pedido de instalação, já com o que a tela escolheu.
#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub kind: InstallKind,
    pub source: PathBuf,
    pub owner: String,
    pub scope: Scope,
    pub overwrite: bool,
    pub consent: bool,
}

/// Registro de consentimento gravado em `<dados>/modules/skill.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConsentLog {
    pub consents: Vec<Consent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Consent {
    pub path: String,
    pub at: i64,
}

/// A pasta do projeto atual, pelo comando `bench.current`. Sem bancada, `None`.
pub fn project_path(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    let value = cx.run_command("bench.current", Args::new()).ok()?;
    value
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Relê as raízes em segundo plano. Se já há uma leitura em curso, não faz nada.
pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared) {
    let busy = shared
        .with(|state| {
            if state.scanning {
                return true;
            }
            state.scanning = true;
            false
        })
        .unwrap_or(true);
    if busy {
        return;
    }
    if fixtures_enabled() {
        let listing = fixtures::listing();
        // A primeira skill já vem selecionada, para a captura mostrar o detalhe com o SKILL.md.
        let first = listing.skills.first().map(|skill| skill.path.clone());
        shared.with(|state| {
            state.body = first.as_ref().map(|path| (path.clone(), fixtures::body(path)));
            state.selected = first;
            state.listing = listing;
            state.scanned = true;
            state.scanning = false;
        });
        return;
    }
    let project = project_path(cx);
    let roots = sources::build_roots(
        &forja_kernel::paths::default_user_dir(),
        forja_kernel::paths::home_dir().as_deref(),
        project.as_deref(),
        &sources::helper_dirs(),
    );
    let project_text = project.map(|path| path.display().to_string());
    let shared = shared.clone();
    cx.spawn(
        move || discovery::scan_all(&roots, project_text),
        move |_cx, listing: Listing| {
            shared.with(|state| {
                state.listing = listing;
                state.scanned = true;
                state.scanning = false;
            });
        },
    );
}

/// Instala uma skill. As checagens baratas acontecem já; o disco roda em segundo plano.
pub fn install(cx: &mut Ctx<'_>, shared: &Shared, request: InstallRequest) -> Result<(), SkillError> {
    refuse_in_fixtures()?;
    let root = root_for(shared, &request.owner, request.scope)?;
    require_consent(&root, request.consent)?;
    let external = !root.is_forja();
    let staging = staging_dir(cx);
    let store = store_path(cx);
    let shared = shared.clone();
    cx.spawn(
        move || {
            let result = match request.kind {
                InstallKind::Folder => match folder_of(&request.source) {
                    Ok(folder) => {
                        install::install_folder(&request.source, &folder, &root.path, request.overwrite, &SystemTrash)
                    }
                    Err(error) => Err(error),
                },
                InstallKind::Zip => install::install_zip(
                    &request.source,
                    &staging,
                    &root.path,
                    request.overwrite,
                    &SystemTrash,
                ),
            };
            if matches!(result, Ok(Placed::Installed(_))) && external {
                let _ = record_consent(&store, &root.path);
            }
            result
        },
        move |cx, result: Result<Placed, SkillError>| finish_placed(cx, &shared, result, "skill-notice-installed"),
    );
    Ok(())
}

/// Copia uma skill listada para outro dono ou escopo.
pub fn copy(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    from: PathBuf,
    owner: String,
    scope: Scope,
    overwrite: bool,
    consent: bool,
) -> Result<(), SkillError> {
    refuse_in_fixtures()?;
    let skill = listed(shared, &from)?;
    let root = root_for(shared, &owner, scope)?;
    if from.parent() == Some(root.path.as_path()) {
        return Err(SkillError::new("skill-error-same-root", root.path.display().to_string()));
    }
    require_consent(&root, consent)?;
    let external = !root.is_forja();
    let store = store_path(cx);
    let shared = shared.clone();
    cx.spawn(
        move || {
            let result = install::install_folder(&from, &skill.folder, &root.path, overwrite, &SystemTrash);
            if matches!(result, Ok(Placed::Installed(_))) && external {
                let _ = record_consent(&store, &root.path);
            }
            result
        },
        move |cx, result: Result<Placed, SkillError>| finish_placed(cx, &shared, result, "skill-notice-copied"),
    );
    Ok(())
}

/// Manda uma skill listada para a lixeira. Fora da Forja, exige consentimento.
pub fn remove(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf, consent: bool) -> Result<(), SkillError> {
    refuse_in_fixtures()?;
    let skill = listed(shared, &path)?;
    let root = root_for(shared, &skill.owner, skill.scope)?;
    require_consent(&root, consent)?;
    let external = !root.is_forja();
    let store = store_path(cx);
    let shared = shared.clone();
    cx.spawn(
        move || {
            let result = install::remove_folder(&path, &SystemTrash);
            if result.is_ok() && external {
                let _ = record_consent(&store, &root.path);
            }
            result
        },
        move |cx, result: Result<(), SkillError>| finish_unit(cx, &shared, result, "skill-notice-removed"),
    );
    Ok(())
}

/// Lê o `SKILL.md` do item selecionado, em segundo plano. Só vale se ainda for o selecionado.
pub fn read_body(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf) {
    let key = path.display().to_string();
    if fixtures_enabled() {
        let text = fixtures::body(&key);
        shared.with(|state| state.body = Some((key, text)));
        return;
    }
    let shared = shared.clone();
    cx.spawn(
        move || read_text(&path.join("SKILL.md")),
        move |cx, result: Result<String, &'static str>| {
            let text = match result {
                Ok(text) => text,
                Err(key) => cx.t(key),
            };
            shared.with(|state| {
                if state.selected.as_deref() == Some(key.as_str()) {
                    state.body = Some((key.clone(), text));
                }
            });
        },
    );
}

/// Executa a operação que a tela deixou pendente. `overwrite` vem da confirmação de sobrescrita.
pub fn run_pending(cx: &mut Ctx<'_>, shared: &Shared, overwrite: bool, consent: bool) {
    let pending = shared.with(|state| state.pending.clone()).flatten();
    let result = match pending {
        None => return,
        Some(Pending::Install {
            kind,
            source,
            owner,
            scope,
            ..
        }) => install(
            cx,
            shared,
            InstallRequest {
                kind,
                source,
                owner,
                scope,
                overwrite,
                consent,
            },
        ),
        Some(Pending::Copy {
            from,
            owner,
            scope,
            ..
        }) => copy(cx, shared, from, owner, scope, overwrite, consent),
        Some(Pending::Remove { path, .. }) => remove(cx, shared, path, consent),
    };
    if let Err(error) = result {
        shared.with(|state| {
            state.pending = None;
            state.conflict = None;
            state.notice = Some(notice_of(&error));
        });
    }
}

/// Termina uma instalação ou cópia. Conflito mantém a operação pendente, para a sobrescrita.
fn finish_placed(cx: &mut Ctx<'_>, shared: &Shared, result: Result<Placed, SkillError>, ok_key: &str) {
    match result {
        Ok(Placed::Installed(path)) => {
            shared.with(|state| {
                state.pending = None;
                state.conflict = None;
                state.notice = Some(Notice {
                    key: ok_key.to_owned(),
                    detail: path.display().to_string(),
                });
            });
            refresh(cx, shared);
        }
        Ok(Placed::Conflict(path)) => {
            shared.with(|state| {
                state.conflict = Some(path.display().to_string());
                state.notice = None;
            });
        }
        Err(error) => {
            shared.with(|state| {
                state.pending = None;
                state.conflict = None;
                state.notice = Some(notice_of(&error));
            });
        }
    }
}

/// Termina uma remoção.
fn finish_unit(cx: &mut Ctx<'_>, shared: &Shared, result: Result<(), SkillError>, ok_key: &str) {
    match result {
        Ok(()) => {
            shared.with(|state| {
                state.pending = None;
                state.conflict = None;
                state.selected = None;
                state.body = None;
                state.notice = Some(Notice {
                    key: ok_key.to_owned(),
                    detail: String::new(),
                });
            });
            refresh(cx, shared);
        }
        Err(error) => {
            shared.with(|state| {
                state.pending = None;
                state.notice = Some(notice_of(&error));
            });
        }
    }
}

/// A raiz de um dono e escopo, como está na última leitura.
pub fn root_for(shared: &Shared, owner: &str, scope: Scope) -> Result<Root, SkillError> {
    shared
        .with(|state| {
            state
                .listing
                .roots
                .iter()
                .find(|root| root.owner == owner && root.scope == scope)
                .map(|root| Root {
                    owner: root.owner.clone(),
                    owner_name: root.owner_name.clone(),
                    scope: root.scope,
                    path: PathBuf::from(&root.path),
                })
        })
        .flatten()
        .ok_or_else(|| SkillError::new("skill-error-no-root", format!("{owner} / {}", scope.as_str())))
}

/// A skill listada com esse caminho. Caminho fora da lista é recusado.
pub fn listed(shared: &Shared, path: &Path) -> Result<Skill, SkillError> {
    let key = path.display().to_string();
    shared
        .with(|state| state.listing.skills.iter().find(|skill| skill.path == key).cloned())
        .flatten()
        .ok_or_else(|| SkillError::new("skill-error-unknown-skill", key))
}

/// Raiz de outro ajudante exige consentimento explícito da tela.
pub fn require_consent(root: &Root, consent: bool) -> Result<(), SkillError> {
    if !root.is_forja() && !consent {
        return Err(SkillError::new("skill-error-consent", root.path.display().to_string()));
    }
    Ok(())
}

fn refuse_in_fixtures() -> Result<(), SkillError> {
    if fixtures_enabled() {
        return Err(SkillError::new("skill-error-fixtures", ""));
    }
    Ok(())
}

/// Nome da pasta de um caminho de origem: o último componente.
fn folder_of(source: &Path) -> Result<String, SkillError> {
    source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| SkillError::new("skill-error-folder-name", source.display().to_string()))
}

fn read_text(path: &Path) -> Result<String, &'static str> {
    let metadata = fs::metadata(path).map_err(|_| "skill-error-read")?;
    if metadata.len() > BODY_LIMIT {
        return Err("skill-error-too-large");
    }
    fs::read_to_string(path).map_err(|_| "skill-error-read")
}

/// Pasta temporária de extração de zips, dentro da pasta de dados da Forja.
fn staging_dir(cx: &Ctx<'_>) -> PathBuf {
    store_path(cx)
        .parent()
        .map(|modules| modules.join("skill-staging"))
        .unwrap_or_else(|| std::env::temp_dir().join("forja-skill-staging"))
}

/// Arquivo de consentimentos deste módulo (`<dados>/modules/skill.json`).
fn store_path(cx: &Ctx<'_>) -> PathBuf {
    cx.store().path()
}

/// Acrescenta um consentimento ao arquivo. Roda em segundo plano.
pub fn record_consent(store: &Path, target: &Path) -> io::Result<()> {
    let mut log: ConsentLog = fs::read_to_string(store)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    log.consents.push(Consent {
        path: target.display().to_string(),
        at: now_unix(),
    });
    if log.consents.len() > CONSENT_LIMIT {
        let excess = log.consents.len() - CONSENT_LIMIT;
        log.consents.drain(..excess);
    }
    if let Some(parent) = store.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&log).map_err(io::Error::other)?;
    fs::write(store, text)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Chave e texto técnico de um erro, para o aviso da tela.
pub fn notice_of(error: &SkillError) -> Notice {
    Notice {
        key: error.key.to_owned(),
        detail: error.detail.clone(),
    }
}

/// Chaves de tradução que este arquivo monta em tempo de execução.
pub const USED_KEYS: &[&str] = &[
    "skill-notice-installed",
    "skill-notice-copied",
    "skill-notice-removed",
    "skill-error-frontmatter-missing",
    "skill-error-frontmatter-unterminated",
    "skill-error-frontmatter-line",
    "skill-error-name-missing",
    "skill-error-name-invalid",
    "skill-error-description-missing",
    "skill-error-read",
    "skill-error-too-large",
    "skill-error-write",
    "skill-error-trash",
    "skill-error-not-a-folder",
    "skill-error-no-skill-file",
    "skill-error-folder-name",
    "skill-error-symlink",
    "skill-error-unsafe-entry",
    "skill-error-zip",
    "skill-error-no-skill-in-zip",
    "skill-error-not-a-skill",
    "skill-error-no-root",
    "skill-error-consent",
    "skill-error-unknown-skill",
    "skill-error-same-root",
    "skill-error-fixtures",
];

/// Lista atual, como o RPC e a tela leem: raízes, skills, pasta do projeto e o último aviso.
pub fn snapshot(shared: &Shared) -> Value {
    shared
        .with(|state| {
            serde_json::json!({
                "project": state.listing.project,
                "scanned": state.scanned,
                "scanning": state.scanning,
                "roots": state.listing.roots,
                "skills": state.listing.skills,
                "notice": state.notice.as_ref().map(|notice| serde_json::json!({
                    "key": notice.key,
                    "detail": notice.detail,
                })),
                "conflict": state.conflict,
            })
        })
        .unwrap_or(Value::Null)
}
