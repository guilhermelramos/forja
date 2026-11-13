//! `counter.notifications.new_request`: tells the user about requests a counter
//! lists for the first time. The first listing of each counter only records the
//! keys, so opening Forja does not announce the whole backlog.

use forja_contracts::{CheckState, CounterOp, CounterResult, RequestItem};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// Keys already seen, per counter.
#[derive(Default)]
pub struct Seen {
    counters: BTreeMap<String, BTreeSet<String>>,
    /// Last checks state per (counter, key), for the failing transition.
    checks: BTreeMap<(String, String), CheckState>,
    checks_seeded: BTreeSet<String>,
}

impl Seen {
    /// Items whose checks are failing now and were not failing in the last listing.
    /// The first listing of a counter only records the states.
    pub fn newly_failing(&mut self, counter: &str, items: &[RequestItem]) -> Vec<RequestItem> {
        // The first call for a counter only records; later calls can announce.
        let seeded = !self.checks_seeded.insert(counter.to_owned());
        let mut out = Vec::new();
        for item in items {
            let key = (counter.to_owned(), item.key.clone());
            let previous = self.checks.insert(key, item.checks.unwrap_or_default());
            let failing = item.checks == Some(CheckState::Failing);
            if seeded && failing && previous != Some(CheckState::Failing) {
                out.push(item.clone());
            }
        }
        out
    }

    /// Items of `items` that were not in the earlier listings of `counter`.
    /// The first listing of a counter returns nothing.
    pub fn fresh_items(&mut self, counter: &str, items: &[RequestItem]) -> Vec<RequestItem> {
        let Some(known) = self.counters.get_mut(counter) else {
            let keys = items.iter().map(|item| item.key.clone()).collect();
            self.counters.insert(counter.to_owned(), keys);
            return Vec::new();
        };
        let mut fresh = Vec::new();
        for item in items {
            if known.insert(item.key.clone()) {
                fresh.push(item.clone());
            }
        }
        fresh
    }
}

/// Handles a `counter.result`: notifies about new requests and about checks that
/// just turned failing. Only list results count; the first listing of a counter
/// records state without notifying.
pub fn on_result(cx: &mut Ctx<'_>, seen: &Mutex<Seen>, result: &CounterResult) {
    if result.op != CounterOp::List {
        return;
    }
    let (fresh, failing) = match seen.lock() {
        Ok(mut seen) => {
            // Checks first: `fresh_items` marks the counter as seen.
            let failing = seen.newly_failing(&result.counter, &result.items);
            (seen.fresh_items(&result.counter, &result.items), failing)
        }
        Err(_) => return,
    };
    if setting_on(cx, "counter.notifications.new_request") {
        for item in fresh {
            let body = format!("{} {} · {}", item.key, item.title, item.counter);
            notify(cx, "counter.notice.new-request", body);
        }
    }
    if setting_on(cx, "counter.notifications.checks_failed") {
        for item in failing {
            let body = format!("{} {} · {}", item.key, item.title, item.counter);
            notify(cx, "counter.notice.checks-failed", body);
        }
    }
}

fn setting_on(cx: &Ctx<'_>, key: &str) -> bool {
    cx.setting(key).and_then(SettingValue::as_bool).unwrap_or(true)
}

fn notify(cx: &mut Ctx<'_>, title_key: &str, body: String) {
    let mut args = Args::new();
    args.insert("title", json!(cx.t(title_key)));
    args.insert("body", json!(body));
    let _ = cx.run_command("notify.show", args);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str) -> RequestItem {
        RequestItem::new("github", key, "t")
    }

    #[test]
    fn first_listing_only_records_keys() {
        let mut seen = Seen::default();
        assert!(seen.fresh_items("github", &[item("#1"), item("#2")]).is_empty());
    }

    #[test]
    fn later_listings_return_only_new_keys_once() {
        let mut seen = Seen::default();
        seen.fresh_items("github", &[item("#1")]);
        let fresh = seen.fresh_items("github", &[item("#1"), item("#3")]);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].key, "#3");
        assert!(seen.fresh_items("github", &[item("#1"), item("#3")]).is_empty());
    }

    #[test]
    fn counters_are_tracked_separately() {
        let mut seen = Seen::default();
        seen.fresh_items("github", &[item("#1")]);
        assert!(seen.fresh_items("gitlab", &[item("#1")]).is_empty());
        assert_eq!(seen.fresh_items("gitlab", &[item("#2")]).len(), 1);
    }
}

#[cfg(test)]
mod checks_tests {
    use super::*;

    fn item(key: &str, checks: Option<CheckState>) -> RequestItem {
        let mut item = RequestItem::new("github", key, "t");
        item.checks = checks;
        item
    }

    #[test]
    fn first_listing_of_a_counter_does_not_announce_failures() {
        let mut seen = Seen::default();
        seen.fresh_items("github", &[item("#1", Some(CheckState::Failing))]);
        assert!(seen
            .newly_failing("github", &[item("#1", Some(CheckState::Failing))])
            .is_empty());
    }

    #[test]
    fn only_the_transition_into_failing_is_announced_once() {
        let mut seen = Seen::default();
        seen.newly_failing("github", &[item("#1", Some(CheckState::Passing))]);
        let first = seen.newly_failing("github", &[item("#1", Some(CheckState::Failing))]);
        assert_eq!(first.len(), 1);
        assert!(seen
            .newly_failing("github", &[item("#1", Some(CheckState::Failing))])
            .is_empty());
    }

    #[test]
    fn providers_without_checks_never_announce() {
        let mut seen = Seen::default();
        seen.newly_failing("github", &[item("#1", None)]);
        assert!(seen.newly_failing("github", &[item("#1", None)]).is_empty());
    }
}
