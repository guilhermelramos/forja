//! Optional egui contract. Enable the `egui` feature to contribute views.
//!
//! The shell owns rendering and instantiates every factory once. A module
//! implements [`View`] and registers a factory with
//! [`crate::registry::Registrar::view`]. The registry itself stores
//! `Box<dyn Any>`, which is why `forja-kernel` has no egui dependency by
//! default.

use crate::runtime::Ctx;
use crate::ui::AnvilHit;

pub use eframe::egui;

/// One piece of interface contributed by a module.
pub trait View {
    /// Draws the view. `cx` gives access to commands, settings, i18n and the
    /// rest of the module services.
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>);

    /// Called every frame before [`Self::ui`], including for a modal that is
    /// still closed. Use it to call [`Ctx::open_modal`] when the view should appear.
    fn poll(&mut self, _cx: &mut Ctx<'_>) {}

    /// When true, the shell asks [`Self::anvil_hits`] for rows and does not
    /// paint [`Self::ui`] inside the Bigorna.
    fn provides_anvil_hits(&self) -> bool {
        false
    }

    /// Ranked rows for the shell query. `query` is the text in the Bigorna field.
    fn anvil_hits(&mut self, _query: &str, _cx: &mut Ctx<'_>) -> Vec<AnvilHit> {
        Vec::new()
    }

    /// True for views that start empty until content arrives or the user opens
    /// them explicitly (e.g., Viewer, Extension panels, History).
    fn empty_by_nature(&self) -> bool {
        false
    }

    /// True while this view has content to display.
    fn has_content(&self) -> bool {
        true
    }
}

/// Wraps a typed factory into the opaque shape stored by the registry.
pub fn factory<F>(make: F) -> impl Fn() -> Box<dyn std::any::Any> + Send + Sync
where
    F: Fn() -> Box<dyn View> + Send + Sync + 'static,
{
    move || Box::new(make()) as Box<dyn std::any::Any>
}

/// Recovers a [`View`] from a registry contribution.
pub fn downcast(value: Box<dyn std::any::Any>) -> Option<Box<dyn View>> {
    value.downcast::<Box<dyn View>>().ok().map(|boxed| *boxed)
}
