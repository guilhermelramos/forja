//! When the native WebView may stay visible.
//!
//! egui cannot paint over a native child, so any overlay, popup, unfocused
//! window or minimized viewport hides the page until it is gone.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub tab_active: bool,
    pub window_focused: bool,
    pub minimized: bool,
    pub overlay_open: bool,
    pub popup_open: bool,
    pub history_open: bool,
}

pub fn visible(input: Input) -> bool {
    input.tab_active
        && input.window_focused
        && !input.minimized
        && !input.overlay_open
        && !input.popup_open
        && !input.history_open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Input {
        Input {
            tab_active: true,
            window_focused: true,
            ..Input::default()
        }
    }

    #[test]
    fn each_condition_hides_the_page() {
        assert!(visible(base()));
        for change in [
            |input: &mut Input| input.tab_active = false,
            |input: &mut Input| input.window_focused = false,
            |input: &mut Input| input.minimized = true,
            |input: &mut Input| input.overlay_open = true,
            |input: &mut Input| input.popup_open = true,
            |input: &mut Input| input.history_open = true,
        ] {
            let mut input = base();
            change(&mut input);
            assert!(!visible(input), "{input:?}");
        }
    }
}
