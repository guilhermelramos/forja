//! Actions the shell chrome performs. Host modules push; the window drains.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnvilMode {
    All,
    Commands,
    Files,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    OpenAnvil(AnvilMode),
    OpenSettings,
    OpenSettingsSection(String),
    ShowKeymap,
    OpenRail(String),
    /// A bench was activated: leave the rail page, show its tabs and name it.
    OpenBench {
        project: String,
        bench: String,
    },
    /// Active project or bench changed; refresh the top bar project crumb.
    RefreshProjectCrumb,
    /// Bring a bench tab (for example the editor) to the front.
    ShowBenchTab(String),
    ToggleSidebar,
    TogglePanel,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleFullscreen,
    NextTab,
    PreviousTab,
    FocusTabs,
    CloseTab,
    ReopenTab,
    SwitchTab(usize),
}

#[cfg(not(test))]
mod queue {
    use super::Action;
    use std::sync::Mutex;

    static QUEUE: Mutex<Vec<Action>> = Mutex::new(Vec::new());

    pub(super) fn push(action: Action) {
        if let Ok(mut queue) = QUEUE.lock() {
            queue.push(action);
        }
    }

    pub(super) fn drain() -> Vec<Action> {
        match QUEUE.lock() {
            Ok(mut queue) => std::mem::take(&mut *queue),
            Err(_) => Vec::new(),
        }
    }
}

/// Tests run in parallel in the same process, so a global queue would let one
/// test drain another one's actions. Each test thread gets its own queue.
#[cfg(test)]
mod queue {
    use super::Action;
    use std::cell::RefCell;

    std::thread_local! {
        static QUEUE: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn push(action: Action) {
        QUEUE.with(|queue| queue.borrow_mut().push(action));
    }

    pub(super) fn drain() -> Vec<Action> {
        QUEUE.with(|queue| std::mem::take(&mut *queue.borrow_mut()))
    }
}

pub fn push(action: Action) {
    queue::push(action);
}

pub fn drain() -> Vec<Action> {
    queue::drain()
}

pub fn dispatch(id: &str) {
    let action = match id {
        "anvil.open" => Action::OpenAnvil(AnvilMode::All),
        "anvil.command_mode" => Action::OpenAnvil(AnvilMode::Commands),
        "find.file" => Action::OpenAnvil(AnvilMode::Files),
        "settings.open" => Action::OpenSettings,
        "settings.keymap" => Action::OpenSettingsSection("keymap".to_owned()),
        "help.keymap" => Action::ShowKeymap,
        "sidebar.toggle" => Action::ToggleSidebar,
        "panel.toggle" => Action::TogglePanel,
        "view.zoom.in" => Action::ZoomIn,
        "view.zoom.out" => Action::ZoomOut,
        "view.zoom.reset" => Action::ZoomReset,
        "window.fullscreen" => Action::ToggleFullscreen,
        "tab.next" => Action::NextTab,
        "tab.previous" => Action::PreviousTab,
        "tab.focus" => Action::FocusTabs,
        "tab.close" | "tab.close_force" => Action::CloseTab,
        "tab.reopen" => Action::ReopenTab,
        "tab.switch_1" => Action::SwitchTab(1),
        "tab.switch_2" => Action::SwitchTab(2),
        "tab.switch_3" => Action::SwitchTab(3),
        "tab.switch_4" => Action::SwitchTab(4),
        "tab.switch_5" => Action::SwitchTab(5),
        "tab.switch_6" => Action::SwitchTab(6),
        "tab.switch_7" => Action::SwitchTab(7),
        "tab.switch_8" => Action::SwitchTab(8),
        "tab.switch_9" => Action::SwitchTab(9),
        "tab.switch_10" => Action::SwitchTab(10),
        _ => return,
    };
    push(action);
}

pub(crate) fn adjust_zoom_scale(current: f64, delta: f64) -> f64 {
    (((current + delta) * 100.0).round() / 100.0).clamp(0.8, 1.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_zoom_scale_rounds_and_clamps() {
        assert!((adjust_zoom_scale(1.0, 0.1) - 1.1).abs() < 1e-9);
        assert!((adjust_zoom_scale(1.0, -0.1) - 0.9).abs() < 1e-9);
        assert!((adjust_zoom_scale(1.5, 0.1) - 1.5).abs() < 1e-9);
        assert!((adjust_zoom_scale(0.8, -0.1) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn dispatch_maps_view_window_tab_and_keymap_commands() {
        let _ = drain();
        dispatch("view.zoom.in");
        dispatch("view.zoom.out");
        dispatch("view.zoom.reset");
        dispatch("window.fullscreen");
        dispatch("tab.next");
        dispatch("tab.previous");
        dispatch("tab.focus");
        dispatch("tab.close_force");
        dispatch("tab.reopen");
        dispatch("tab.switch_10");
        dispatch("settings.keymap");
        dispatch("help.keymap");
        assert_eq!(
            drain(),
            vec![
                Action::ZoomIn,
                Action::ZoomOut,
                Action::ZoomReset,
                Action::ToggleFullscreen,
                Action::NextTab,
                Action::PreviousTab,
                Action::FocusTabs,
                Action::CloseTab,
                Action::ReopenTab,
                Action::SwitchTab(10),
                Action::OpenSettingsSection("keymap".to_owned()),
                Action::ShowKeymap,
            ]
        );
    }
}
