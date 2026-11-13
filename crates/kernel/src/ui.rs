//! UI contributions as opaque values.
//!
//! The kernel never depends on egui in its default build: a view factory
//! returns `Box<dyn Any>`, and the shell downcasts it to
//! [`crate::ui_contract::View`] (available behind the `egui` feature). This
//! keeps the registry testable without a graphical stack.

use std::any::Any;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    Rail,
    Sidebar,
    BenchTab,
    RightPanel,
    StatusBar,
    SettingsSection,
    Modal,
    AnvilSource,
    /// The top bar, between the project crumb and the window controls.
    TopBar,
}

impl Slot {
    pub const ALL: [Slot; 9] = [
        Slot::Rail,
        Slot::Sidebar,
        Slot::BenchTab,
        Slot::RightPanel,
        Slot::StatusBar,
        Slot::SettingsSection,
        Slot::Modal,
        Slot::AnvilSource,
        Slot::TopBar,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Rail => "rail",
            Self::Sidebar => "sidebar",
            Self::BenchTab => "bench_tab",
            Self::RightPanel => "right_panel",
            Self::StatusBar => "status_bar",
            Self::SettingsSection => "settings_section",
            Self::Modal => "modal",
            Self::AnvilSource => "anvil_source",
            Self::TopBar => "top_bar",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|slot| slot.id() == value)
    }
}

type AnyViewFactory = Arc<dyn Fn() -> Box<dyn Any> + Send + Sync>;

/// One ranked row a Bigorna source offers for the current query.
///
/// `command` is run with `args` when the row is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct AnvilHit {
    pub title: String,
    /// Second line. The shell shows it when the row has no keyboard hint.
    pub detail: String,
    /// Optional glyph drawn in front of the title.
    pub icon_glyph: Option<String>,
    pub score: i32,
    pub command: String,
    pub args: serde_json::Map<String, serde_json::Value>,
}

/// A factory plus the metadata the shell needs before instantiating it.
#[derive(Clone)]
pub struct ViewContribution {
    pub module: String,
    pub slot: Slot,
    pub id: String,
    pub title_key: String,
    pub order: i32,
    /// Short glyph drawn on the rail. The page itself is `ui()`.
    pub icon_glyph: Option<String>,
    /// PNG bytes for the rail icon. Takes precedence over [`Self::icon_glyph`].
    pub icon_png: Option<Arc<[u8]>>,
    factory: AnyViewFactory,
}

impl ViewContribution {
    pub fn new(
        module: impl Into<String>,
        slot: Slot,
        id: impl Into<String>,
        title_key: impl Into<String>,
        order: i32,
        factory: AnyViewFactory,
    ) -> Self {
        Self {
            module: module.into(),
            slot,
            id: id.into(),
            title_key: title_key.into(),
            order,
            icon_glyph: None,
            icon_png: None,
            factory,
        }
    }

    /// Rail glyph. A page in [`Slot::Rail`] draws this on the rail and `ui()` in the center.
    pub fn with_glyph(mut self, glyph: impl Into<String>) -> Self {
        self.icon_glyph = Some(glyph.into());
        self
    }

    /// Rail icon encoded as PNG.
    pub fn with_png(mut self, png: impl Into<Arc<[u8]>>) -> Self {
        self.icon_png = Some(png.into());
        self
    }

    /// Builds a fresh view instance. The shell calls this once per slot.
    pub fn instantiate(&self) -> Box<dyn Any> {
        (self.factory)()
    }
}
