//! Barra de status: ordem/filtro das contribuições, edição da lista e prazos
//! dos serviços. Toda a leitura vem do estado já carregado; nada aqui toca
//! disco, rede ou processo na thread da interface.

use crate::app::ForjaApp;
use eframe::egui;
use forja_kernel::SettingValue;
use std::time::{Duration, Instant};

/// Ordem padrão do design, usada enquanto não há escolha explícita.
pub(crate) const DEFAULT_ITEMS: [&str; 5] = [
    "bench.status",
    "helper.status-bar",
    "usage.status",
    "ports.status",
    "notify.center",
];

fn poll_deadline_id() -> egui::Id {
    egui::Id::new("forja.status.poll-deadline")
}

fn items_menu_id() -> egui::Id {
    egui::Id::new("forja.status.items-menu")
}

/// A lista configurada, com os ids desconhecidos preservados. Uma lista vazia
/// (estado antigo ou apagada) significa "usar o padrão do design".
pub(crate) fn configured_items(app: &ForjaApp) -> Vec<String> {
    app.runtime
        .settings()
        .values()
        .get("appearance.status_bar.items")
        .and_then(SettingValue::as_list)
        .filter(|items| !items.is_empty())
        .map(|items| items.to_vec())
        .unwrap_or_else(|| DEFAULT_ITEMS.iter().map(|id| (*id).to_owned()).collect())
}

/// Índices das contribuições presentes, na ordem esquerda → direita da lista.
/// Ids da lista sem contribuição registrada ficam apenas guardados no arquivo.
pub(crate) fn ordered_view_indexes(app: &ForjaApp) -> Vec<usize> {
    let items = configured_items(app);
    let mut indexes: Vec<usize> = (0..app.status_views.len())
        .filter(|index| items.iter().any(|id| id == &app.status_views[*index].id))
        .collect();
    indexes.sort_by_key(|index| {
        items
            .iter()
            .position(|id| id == &app.status_views[*index].id)
            .unwrap_or(usize::MAX)
    });
    indexes
}

/// Alterna um item. Ao ligar, insere na posição canônica; ao desligar, remove
/// só aquele id. Qualquer id desconhecido permanece onde estava.
pub(crate) fn apply_toggle(items: &mut Vec<String>, id: &str, registration: &[String]) {
    if let Some(position) = items.iter().position(|item| item == id) {
        items.remove(position);
        return;
    }
    let rank = |item: &str| {
        registration
            .iter()
            .position(|registered| registered == item)
            .unwrap_or(registration.len())
    };
    let position = items
        .iter()
        .position(|item| rank(item) > rank(id))
        .unwrap_or(items.len());
    items.insert(position, id.to_owned());
}

/// Move um item uma posição, sem tocar nos demais (inclusive desconhecidos).
pub(crate) fn apply_move(items: &mut [String], id: &str, delta: isize) {
    let Some(from) = items.iter().position(|item| item == id) else {
        return;
    };
    let last = items.len().saturating_sub(1) as isize;
    let to = (from as isize + delta).clamp(0, last) as usize;
    if from != to {
        items.swap(from, to);
    }
}

pub(crate) fn schedule_due(now: Instant, deadline: Option<Instant>) -> bool {
    deadline.is_none_or(|at| now >= at)
}

/// "Restaurar padrão": volta à ordem de registro e preserva os ids que ainda
/// não têm contribuição registrada (o usuário não pode mexer neles na interface).
pub(crate) fn restore_order(registered: &[String], current: &[String]) -> Vec<String> {
    let mut restored = registered.to_vec();
    for id in current {
        if !registered.iter().any(|registered| registered == id) && !restored.contains(id) {
            restored.push(id.clone());
        }
    }
    restored
}

impl ForjaApp {
    fn save_items(&mut self, items: Vec<String>) {
        let _ = self
            .runtime
            .set_setting("appearance.status_bar.items", SettingValue::List(items));
    }

    fn registration_order(&self) -> Vec<String> {
        let registered: Vec<String> = self
            .status_views
            .iter()
            .map(|view| view.id.clone())
            .collect();
        let mut order: Vec<String> = DEFAULT_ITEMS
            .iter()
            .filter(|id| registered.iter().any(|registered| registered == *id))
            .map(|id| (*id).to_owned())
            .collect();
        for id in registered {
            if !order.contains(&id) {
                order.push(id);
            }
        }
        order
    }

    /// Roda uma vez por quadro, antes da barra. Só pede repaint no prazo real:
    /// em repouso os serviços silenciam e o app volta perto de 0% de CPU.
    pub(crate) fn poll_status_services(&mut self, ctx: &egui::Context) {
        let mut soonest: Option<Duration> = None;
        for view in self.status_views.iter_mut() {
            let module = view.module.clone();
            let mut cx = self.runtime.ctx(&module);
            view.view.poll(&mut cx);
            if let Some(delay) = view.view.next_poll_at() {
                soonest = Some(soonest.map_or(delay, |current| current.min(delay)));
            }
        }
        let now = Instant::now();
        let deadline = ctx
            .data(|data| data.get_temp::<Option<Instant>>(poll_deadline_id()))
            .flatten();
        match soonest {
            Some(delay) if schedule_due(now, deadline) => {
                let delay = delay.max(Duration::from_millis(1));
                ctx.data_mut(|data| data.insert_temp(poll_deadline_id(), Some(now + delay)));
                ctx.request_repaint_after(delay);
            }
            Some(_) => {}
            None => {
                let _ = ctx.data_mut(|data| data.remove_temp::<Option<Instant>>(poll_deadline_id()));
            }
        }
    }

    pub(crate) fn restore_status_items(&mut self) {
        let registered: Vec<String> = self
            .status_views
            .iter()
            .map(|view| view.id.clone())
            .collect();
        let current = configured_items(self);
        let restored = restore_order(&registered, &current);
        self.save_items(restored);
    }

    pub(crate) fn toggle_status_item(&mut self, id: &str) {
        let registration = self.registration_order();
        let mut items = configured_items(self);
        apply_toggle(&mut items, id, &registration);
        self.save_items(items);
    }

    pub(crate) fn move_status_item(&mut self, id: &str, delta: isize) {
        let mut items = configured_items(self);
        apply_move(&mut items, id, delta);
        self.save_items(items);
    }

    /// Menu de contexto da barra: alterna itens e move com setas, com nomes
    /// acessíveis e botões do ui-kit.
    pub(crate) fn status_items_context(&mut self, ctx: &egui::Context) {
        let mut open = ctx
            .data(|data| data.get_temp::<bool>(items_menu_id()))
            .unwrap_or(false);
        if !open {
            return;
        }
        let ids: Vec<(String, String)> = self
            .status_views
            .iter()
            .map(|view| {
                let label = self.text(&view.title_key);
                (view.id.clone(), label)
            })
            .collect();
        let items = configured_items(self);
        let title = self.text("shell.layout.status-items-menu");
        let up = self.text("shell.layout.up");
        let down = self.text("shell.layout.down");
        egui::Window::new(title.clone())
            .id(egui::Id::new("forja.status.items-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                for (id, label) in ids {
                    let mut on = items.iter().any(|item| item == &id);
                    let accessible =
                        self.text_args("shell.layout.status-item", &[("name", &label)]);
                    if forja_ui_kit::toggle_labeled(ui, &mut on, &accessible).changed() {
                        self.toggle_status_item(&id);
                    }
                    ui.horizontal(|ui| {
                        let button_up = forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &up,
                        );
                        if forja_ui_kit::tooltip(button_up, up.clone()).clicked() {
                            self.move_status_item(&id, -1);
                        }
                        let button_down = forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &down,
                        );
                        if forja_ui_kit::tooltip(button_down, down.clone()).clicked() {
                            self.move_status_item(&id, 1);
                        }
                        forja_ui_kit::text(ui, &label);
                    });
                }
            });
        ctx.data_mut(|data| data.insert_temp(items_menu_id(), open));
    }
}

pub(crate) fn open_status_items_menu(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(items_menu_id(), true));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<String> {
        DEFAULT_ITEMS.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn status_bar_uses_the_design_order_and_filters_unlisted_views() {
        let app_items = items();
        let registered = [
            "ports.status".to_owned(),
            "bench.status".to_owned(),
            "trust.settings".to_owned(),
        ];
        let mut indexes: Vec<&str> = registered
            .iter()
            .filter(|id| app_items.iter().any(|item| item == *id))
            .map(String::as_str)
            .collect();
        indexes.sort_by_key(|id| app_items.iter().position(|item| item == *id));
        assert_eq!(indexes, vec!["bench.status", "ports.status"]);
    }

    #[test]
    fn editing_the_list_preserves_unknown_ids() {
        let registration = DEFAULT_ITEMS
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>();
        let mut edited = items();
        edited.push("plugin.future".to_owned());
        apply_toggle(&mut edited, "ports.status", &registration);
        assert!(!edited.iter().any(|id| id == "ports.status"));
        assert!(edited.iter().any(|id| id == "plugin.future"));
        apply_toggle(&mut edited, "ports.status", &registration);
        apply_move(&mut edited, "ports.status", -1);
        assert!(edited.iter().any(|id| id == "plugin.future"));
        assert_eq!(edited.len(), DEFAULT_ITEMS.len() + 1);
    }

    #[test]
    fn restoring_defaults_orders_by_registration_and_keeps_unknown_ids() {
        let registered = [
            "bench.status".to_owned(),
            "helper.status-bar".to_owned(),
            "usage.status".to_owned(),
            "ports.status".to_owned(),
        ];
        let current = vec![
            "ports.status".to_owned(),
            "plugin.future".to_owned(),
            "helper.status-bar".to_owned(),
        ];
        let restored = restore_order(&registered, &current);
        assert_eq!(
            restored,
            vec![
                "bench.status".to_owned(),
                "helper.status-bar".to_owned(),
                "usage.status".to_owned(),
                "ports.status".to_owned(),
                "plugin.future".to_owned(),
            ]
        );
        assert_eq!(restored.iter().filter(|id| *id == "plugin.future").count(), 1);
    }

    #[test]
    fn a_poll_deadline_never_wakes_every_frame() {
        let now = Instant::now();
        assert!(schedule_due(now, None));
        assert!(!schedule_due(now, Some(now + Duration::from_secs(3))));
        assert!(schedule_due(
            now + Duration::from_secs(4),
            Some(now + Duration::from_secs(3))
        ));
    }
}
