//! Testes dos gestos: alternar, segurar e a mistura dos dois.

use super::*;

#[test]
fn toggle_starts_on_the_first_press_and_stops_on_the_second() {
    let mut gesture = Gesture::default();
    assert_eq!(gesture.feed(Mode::Toggle, Edge::Press), Action::Start);
    assert!(gesture.is_active());
    assert_eq!(gesture.feed(Mode::Toggle, Edge::Press), Action::Stop);
    assert!(!gesture.is_active());
}

#[test]
fn toggle_ignores_the_release() {
    let mut gesture = Gesture::default();
    assert_eq!(gesture.feed(Mode::Toggle, Edge::Release), Action::Nothing);
    assert!(!gesture.is_active());
    gesture.feed(Mode::Toggle, Edge::Press);
    assert_eq!(gesture.feed(Mode::Toggle, Edge::Release), Action::Nothing);
    assert!(gesture.is_active(), "a release never stops a toggle");
}

#[test]
fn hold_starts_on_press_and_stops_on_release() {
    let mut gesture = Gesture::default();
    assert_eq!(gesture.feed(Mode::Hold, Edge::Press), Action::Start);
    assert_eq!(gesture.feed(Mode::Hold, Edge::Press), Action::Nothing, "key repeat does nothing");
    assert_eq!(gesture.feed(Mode::Hold, Edge::Release), Action::Stop);
    assert!(!gesture.is_active());
}

#[test]
fn a_release_without_a_matching_press_does_nothing() {
    let mut gesture = Gesture::default();
    assert_eq!(gesture.feed(Mode::Hold, Edge::Release), Action::Nothing);
    assert!(!gesture.is_active());
}

#[test]
fn a_toggle_that_is_already_on_is_not_stopped_by_a_hold_release() {
    let mut gesture = Gesture::default();
    gesture.feed(Mode::Toggle, Edge::Press);
    assert_eq!(gesture.feed(Mode::Hold, Edge::Press), Action::Nothing);
    assert_eq!(gesture.feed(Mode::Hold, Edge::Release), Action::Nothing, "the toggle started it, not the hold");
    assert!(gesture.is_active());
}

#[test]
fn a_toggle_press_ends_a_hold_that_is_running() {
    let mut gesture = Gesture::default();
    gesture.feed(Mode::Hold, Edge::Press);
    assert_eq!(gesture.feed(Mode::Toggle, Edge::Press), Action::Stop);
    assert_eq!(gesture.feed(Mode::Hold, Edge::Release), Action::Nothing);
}

#[test]
fn reset_clears_the_state_without_an_action() {
    let mut gesture = Gesture::default();
    gesture.feed(Mode::Hold, Edge::Press);
    gesture.reset();
    assert!(!gesture.is_active());
    assert_eq!(gesture.feed(Mode::Hold, Edge::Release), Action::Nothing);
}
