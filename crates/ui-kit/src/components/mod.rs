//! Controles do design system Brasa.

mod button;
mod card;
mod choice;
mod field;
mod menu;
mod modal;
mod page;
mod page_layout;
mod settings;
mod status;
mod tabs;
mod tabs_fit;
mod toggle;
mod tree;

pub use button::{action_tile, button, ButtonKind};
pub use card::{card, list, ListItem};
pub use choice::{segmented, tabs};
pub use field::{select, text_field};
pub use menu::{context_menu, icon_menu, menu_panel, popup_menu, MenuItem};
pub use modal::{
    modal, modal_explicit_close, modal_preview, window_header, window_header_with_focus,
};
pub use page::{empty_state, page_heading, theme_sample};
pub use page_layout::{
    page, page_marker, page_state, Page, PageIconAction, PageMarker, PageResponse, PageSpec,
    PageState, PageWidth,
};
pub use settings::{
    highlighted_text, setting_row, settings_navigation, settings_viewport, SettingRow,
};
pub use status::{
    error_banner, error_notice, helper_missing_guide, progress_bar, stage_badge, toast,
    truncated_label, wrapped_label, HelperMissingGuide, Stage, ToastKind,
};
pub use tabs::{panel_icon_tabs, tab_strip, PanelIconTab, TabItem, TabStripOutcome};
pub use tabs_fit::{measure_tab_width, plan_tab_fit, TabFitPlan};
pub use toggle::toggle;
pub use tree::{tree, virtual_table, Column, TreeNode};
