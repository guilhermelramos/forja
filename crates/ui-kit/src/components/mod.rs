//! Controles do design system Brasa.

mod button;
mod card;
mod choice;
mod field;
mod menu;
mod modal;
mod page;
mod page_layout;
mod primitives;
mod settings;
pub use primitives::{
    form_row, list_row, number_card, section_heading, skeleton, spinner, splitter, tag, text,
    toolbar, toolbar_more, toolbar_separator, FormRow, ListRow, Spacing,
};
mod status;
mod tabs;
mod tabs_fit;
mod toggle;
mod tree;

pub use button::{action_tile, button, ButtonKind};
pub use card::{card, list, ListItem};
pub use choice::{segmented, tabs};
pub use field::{select, text_field};
pub use menu::{context_menu, icon_menu, menu_panel, popup_menu, popup_menu_contents, MenuItem};
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
    error_banner, error_notice, helper_missing_guide, progress_bar, selectable_wrapped_label,
    stage_badge, toast, toast_transition, truncated_label, wrapped_label, HelperMissingGuide,
    Stage, ToastKind,
};
pub use tabs::{panel_icon_tabs, tab_strip, PanelIconTab, TabItem, TabStripOutcome};
pub use tabs_fit::{measure_tab_width, plan_tab_fit, TabFitPlan};
pub use toggle::toggle;
pub use tree::{tree, virtual_table, Column, TreeNode};
