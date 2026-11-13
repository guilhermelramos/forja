//! Comandos `trust.*`.

use crate::state::{parse_kind, TrustState};
use crate::Shared;
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::{Args, Ctx, KernelError, Registrar};
use serde_json::json;
use std::sync::Arc;

pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
    let check_core = Arc::clone(core);
    r.command("trust.check", "trust.command.check", move |cx, args| {
        let kind = required_kind(&args)?;
        let source = args.required_str("source_path")?.to_owned();
        let hash = args.required_str("content_hash")?.to_owned();
        let (approved, decided_at) = check_core
            .with_state(cx, |state| state.check(kind, &source, &hash))
            .map_err(error)?;
        Ok(json!({ "approved": approved, "decided_at": decided_at }))
    });

    let request_core = Arc::clone(core);
    r.command("trust.request", "trust.command.request", move |cx, args| {
        let kind = required_kind(&args)?;
        let source = args.required_str("source_path")?.to_owned();
        let content = args.required_str("content")?.to_owned();
        let summary = args.str("summary").unwrap_or_default().to_owned();
        let (request_id, approved) = request_core
            .with_state(cx, |state| {
                state.request(kind, source, &content, summary)
            })
            .map_err(error)?;
        Ok(json!({ "request_id": request_id, "approved": approved }))
    });

    let grant_core = Arc::clone(core);
    r.command("trust.grant", "trust.command.grant", move |cx, args| {
        let kind = required_kind(&args)?;
        let source = args.required_str("source_path")?.to_owned();
        let content = args.required_str("content")?.to_owned();
        let summary = args.str("summary").unwrap_or_default().to_owned();
        grant_core
            .with_state(cx, |state| {
                state.grant(kind, source, &content, summary);
            })
            .map_err(error)?;
        persist(cx, &grant_core)?;
        Ok(json!({ "granted": true }))
    });

    let revoke_core = Arc::clone(core);
    r.command("trust.revoke", "trust.command.revoke", move |cx, args| {
        let kind = required_kind(&args)?;
        let source = args.required_str("source_path")?.to_owned();
        let hash = args.str("content_hash");
        let revoked = revoke_core
            .with_state(cx, |state| state.revoke(kind, &source, hash))
            .map_err(error)?;
        if revoked {
            persist(cx, &revoke_core)?;
        }
        Ok(json!({ "revoked": revoked }))
    });

    let list_core = Arc::clone(core);
    r.command("trust.list", "trust.command.list", move |cx, _| {
        let entries = list_core
            .with_state(cx, |state| state.list())
            .map_err(error)?;
        Ok(json!({ "entries": entries }))
    });
}

fn required_kind(args: &Args) -> forja_kernel::Result<forja_contracts::TrustKind> {
    let raw = args.required_str("kind")?;
    parse_kind(raw).ok_or_else(|| {
        KernelError::Message(format!("unknown trust kind `{raw}`"))
    })
}

fn error(message: impl ToString) -> KernelError {
    KernelError::Message(message.to_string())
}

fn persist(cx: &mut Ctx<'_>, core: &Shared) -> forja_kernel::Result<()> {
    let (file, path) = core
        .with_state(cx, |state| (state.file(), state.store_path().map(Path::to_path_buf)))
        .map_err(error)?;
    if let Some(path) = path {
        cx.spawn(
            move || {
                if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
                    let _ = write_atomic_bytes(&path, &bytes);
                }
            },
            |_, _| {},
        );
    }
    Ok(())
}

use std::path::Path;

impl crate::Core {
    pub(crate) fn with_state<T>(
        &self,
        cx: &mut Ctx<'_>,
        run: impl FnOnce(&mut TrustState) -> T,
    ) -> Result<T, String> {
        let mut state = self.state.lock().map_err(|_| "trust state is poisoned".to_owned())?;
        state.ensure_loaded(&cx.store().path());
        Ok(run(&mut state))
    }
}
