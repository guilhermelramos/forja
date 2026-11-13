//! Fuzzy matching and ranking for the file finder.
//!
//! A query matches when its characters appear in order inside the path
//! (case-insensitive). The score rewards matches that start a word (after `/`,
//! `\`, `_`, `-`, `.` or a space), runs of consecutive characters, and matches
//! inside the file name. Long paths lose a little, so short direct paths win ties.
//! Recent and frequently opened files get a bonus from [`History`].

use crate::history::History;

/// A path prepared for matching: folded characters and word-start flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub rel: String,
    folded: Vec<char>,
    boundary: Vec<bool>,
    name_start: usize,
}

fn is_separator(ch: char) -> bool {
    matches!(ch, '/' | '\\' | '_' | '-' | '.' | ' ')
}

impl Candidate {
    /// `rel` uses `/` separators and is relative to the bench root.
    pub fn new(rel: impl Into<String>) -> Self {
        let rel = rel.into();
        let mut folded = Vec::with_capacity(rel.len());
        let mut boundary = Vec::with_capacity(rel.len());
        let mut name_start = 0usize;
        let mut previous: Option<char> = None;
        for ch in rel.chars() {
            if matches!(ch, '/' | '\\') {
                name_start = folded.len() + 1;
            }
            let starts_word = previous.is_none_or(is_separator);
            for lower in ch.to_lowercase() {
                folded.push(lower);
                boundary.push(starts_word);
            }
            previous = Some(ch);
        }
        Self {
            rel,
            folded,
            boundary,
            name_start,
        }
    }

    fn len(&self) -> i32 {
        i32::try_from(self.folded.len()).unwrap_or(i32::MAX)
    }
}

/// Lowercases a query the same way candidates are folded.
pub fn fold(query: &str) -> Vec<char> {
    query.chars().flat_map(char::to_lowercase).collect()
}

/// Scores `candidate` against folded `query`. `None` means the query does not match.
pub fn score(query: &[char], candidate: &Candidate) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let mut matched = 0usize;
    let mut total = 0i32;
    let mut previous: Option<usize> = None;
    let mut run = 0i32;
    for (index, &ch) in candidate.folded.iter().enumerate() {
        if matched < query.len() && ch == query[matched] {
            let mut bonus = 10;
            if candidate.boundary[index] {
                bonus += 12;
            }
            if index >= candidate.name_start {
                bonus += 20;
            }
            if previous.is_some_and(|p| p + 1 == index) {
                run += 1;
                bonus += run * 5;
            } else {
                run = 0;
            }
            total += bonus;
            previous = Some(index);
            matched += 1;
        }
    }
    (matched == query.len()).then(|| total - candidate.len() / 3)
}

/// Splits `path/to/file.rs:42` (or `file.rs:42:7`) into the searchable part and a 1-based line.
pub fn split_location(query: &str) -> (&str, Option<u32>) {
    let mut text = query.trim();
    let mut numbers: Vec<u32> = Vec::new();
    for _ in 0..2 {
        let Some((head, tail)) = text.rsplit_once(':') else {
            break;
        };
        match tail.parse::<u32>() {
            Ok(number) if number > 0 && !head.is_empty() => {
                numbers.push(number);
                text = head;
            }
            _ => break,
        }
    }
    (text, numbers.last().copied())
}

/// Ranks `files` for `query` and returns the best `limit` relative paths.
///
/// An empty query lists the recently opened files first (up to `recent_count`),
/// then fills with the start of the index.
pub fn rank(
    files: &[Candidate],
    query: &str,
    history: &History,
    root_key: &str,
    now: u64,
    limit: usize,
    recent_count: usize,
) -> Vec<String> {
    let (text, _) = split_location(query);
    let folded = fold(text);
    if folded.is_empty() {
        let mut out: Vec<String> = history
            .recent(root_key, recent_count)
            .into_iter()
            .filter(|rel| files.iter().any(|file| file.rel == *rel))
            .collect();
        for file in files {
            if out.len() >= limit {
                break;
            }
            if !out.contains(&file.rel) {
                out.push(file.rel.clone());
            }
        }
        out.truncate(limit);
        return out;
    }
    let mut scored: Vec<(i32, &Candidate)> = files
        .iter()
        .filter_map(|file| {
            score(&folded, file).map(|s| (s + history.bonus(root_key, &file.rel, now), file))
        })
        .collect();
    scored.sort_by(|(a, fa), (b, fb)| {
        b.cmp(a)
            .then_with(|| fa.rel.len().cmp(&fb.rel.len()))
            .then_with(|| fa.rel.cmp(&fb.rel))
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(_, file)| file.rel.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(paths: &[&str]) -> Vec<Candidate> {
        paths.iter().map(|p| Candidate::new(*p)).collect()
    }

    fn best(query: &str, paths: &[&str]) -> Option<String> {
        let files = candidates(paths);
        rank(&files, query, &History::default(), "/r", 0, 1, 0)
            .into_iter()
            .next()
    }

    #[test]
    fn subsequence_matches_case_insensitively() {
        let file = Candidate::new("src/ui/Quick_Open.rs");
        assert!(score(&fold("uiqo"), &file).is_some());
        assert!(score(&fold("QUICK"), &file).is_some());
        assert!(score(&fold("zz"), &file).is_none());
    }

    #[test]
    fn filename_and_word_start_matches_beat_scattered_ones() {
        assert_eq!(
            best("main", &["docs/domain/notes.md", "src/main.rs"]).as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(
            best("qo", &["qa/other/quiet_zone.txt", "src/quick_open.rs"]).as_deref(),
            Some("src/quick_open.rs")
        );
    }

    #[test]
    fn shorter_paths_win_ties() {
        assert_eq!(
            best("lib", &["a/very/deep/folder/lib.rs", "lib.rs"]).as_deref(),
            Some("lib.rs")
        );
    }

    #[test]
    fn history_breaks_a_tie_between_equal_matches() {
        let files = candidates(&["src/alpha.rs", "src/beta.rs"]);
        let mut history = History::default();
        history.record("/r", "src/beta.rs", 1_000);
        let ranked = rank(&files, "rs", &history, "/r", 1_000, 2, 0);
        assert_eq!(ranked[0], "src/beta.rs");
    }

    #[test]
    fn empty_query_shows_recent_files_then_the_index() {
        let files = candidates(&["a.rs", "b.rs", "c.rs"]);
        let mut history = History::default();
        history.record("/r", "c.rs", 10);
        let ranked = rank(&files, "", &history, "/r", 10, 3, 5);
        assert_eq!(ranked, vec!["c.rs", "a.rs", "b.rs"]);
    }

    #[test]
    fn line_suffix_is_split_off_the_query() {
        assert_eq!(split_location("src/main.rs:42"), ("src/main.rs", Some(42)));
        assert_eq!(
            split_location("main.rs:42:7"),
            ("main.rs", Some(42)),
            "line before column"
        );
        assert_eq!(split_location("plain"), ("plain", None));
        assert_eq!(split_location("C:\\x\\y.rs"), ("C:\\x\\y.rs", None));
        assert_eq!(split_location("a.rs:0"), ("a.rs:0", None));
        assert_eq!(
            best("main.rs:12", &["src/main.rs"]).as_deref(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn non_matches_are_dropped() {
        assert_eq!(best("xyz", &["a.rs", "b.rs"]), None);
    }
}
