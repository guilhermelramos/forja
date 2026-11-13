//! Etapa da bancada no pedido local. `bench.stage.sync_request` põe o pedido de
//! origem em `stage.changed`; os ajustes `request.sync.*` decidem o que muda no
//! pedido local. Balcões remotos não são escritos aqui: mudar um pedido remoto é
//! ação do usuário, pelo `request.transition`.

use crate::commands::{persist_local, with_state};
use crate::local::COUNTER as LOCAL;
use crate::state::Shared;
use forja_contracts::{RequestItem, RequestState, StageChanged};
use forja_kernel::{Ctx, SettingValue};

/// Etapa que marca o pedido em revisão (`request.sync.stage_on_pr`).
pub const REVIEW_STAGE: &str = "review";
/// Etapa de entrega: a bancada chega aqui quando o PR é mesclado
/// (`request.sync.close_on_merge`).
pub const DELIVERED_STAGE: &str = "ready";

/// Aplica uma etapa de bancada ao pedido local de origem, se houver.
pub fn apply_stage(cx: &mut Ctx<'_>, shared: &Shared, event: &StageChanged) {
    let Some(key) = event.request.as_deref() else {
        return;
    };
    let stage_on_pr = flag(cx, "request.sync.stage_on_pr", true);
    let close_on_merge = flag(cx, "request.sync.close_on_merge", true);
    let changed = with_state(shared, |state| {
        state
            .local
            .iter_mut()
            .find(|item| item.counter == LOCAL && item.key == key)
            .is_some_and(|item| {
                sync_item(item, &event.to, stage_on_pr, close_on_merge)
            })
    })
    .unwrap_or(false);
    if changed {
        persist_local(cx, shared);
    }
}

/// Muda o pedido conforme a etapa de destino. Devolve `true` se algo mudou.
pub fn sync_item(
    item: &mut RequestItem,
    to: &str,
    stage_on_pr: bool,
    close_on_merge: bool,
) -> bool {
    if to == REVIEW_STAGE && stage_on_pr && !item.review_requested {
        item.review_requested = true;
        return true;
    }
    if to == DELIVERED_STAGE && close_on_merge && item.state != RequestState::Closed {
        item.state = RequestState::Closed;
        return true;
    }
    false
}

fn flag(cx: &Ctx<'_>, key: &str, fallback: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> RequestItem {
        RequestItem::new(LOCAL, "LOCAL-1", "Teste")
    }

    #[test]
    fn review_stage_marks_the_request_when_the_setting_is_on() {
        let mut request = item();
        assert!(sync_item(&mut request, REVIEW_STAGE, true, true));
        assert!(request.review_requested);
        assert!(!sync_item(&mut request, REVIEW_STAGE, true, true));
    }

    #[test]
    fn review_stage_is_ignored_when_the_setting_is_off() {
        let mut request = item();
        assert!(!sync_item(&mut request, REVIEW_STAGE, false, true));
        assert!(!request.review_requested);
    }

    #[test]
    fn delivered_stage_closes_the_request_once() {
        let mut request = item();
        assert!(sync_item(&mut request, DELIVERED_STAGE, true, true));
        assert_eq!(request.state, RequestState::Closed);
        assert!(!sync_item(&mut request, DELIVERED_STAGE, true, true));
    }

    #[test]
    fn delivered_stage_keeps_the_request_open_when_the_setting_is_off() {
        let mut request = item();
        assert!(!sync_item(&mut request, DELIVERED_STAGE, true, false));
        assert_eq!(request.state, RequestState::Open);
    }

    #[test]
    fn other_stages_do_not_touch_the_request() {
        let mut request = item();
        assert!(!sync_item(&mut request, "forging", true, true));
        assert!(!request.review_requested);
        assert_eq!(request.state, RequestState::Open);
    }
}
