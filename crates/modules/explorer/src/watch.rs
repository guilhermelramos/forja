//! Keeps the Files cache in step with the disk, using `notify`.
//!
//! The watcher sends every changed path into a channel. A chain of kernel tasks
//! waits on that channel, collects one burst of changes, and invalidates the
//! affected folders on the interface thread. The OS watcher itself is created in
//! a task too, because starting it touches the file system.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Quiet time after the first change, so one save makes one refresh.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// Upper bound per batch, so a huge burst is processed in slices.
const BATCH_LIMIT: usize = 5_000;

/// Starts a recursive watcher on `root`. Changed paths arrive on the receiver.
pub fn start(root: &Path) -> Result<(RecommendedWatcher, Receiver<PathBuf>), String> {
    let (sender, receiver) = channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result {
            for path in event.paths {
                let _ = sender.send(path);
            }
        }
    })
    .map_err(|error| error.to_string())?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    Ok((watcher, receiver))
}

/// One burst of changes. `alive` is false once the watcher has been dropped.
pub struct Batch {
    pub receiver: Receiver<PathBuf>,
    pub paths: Vec<PathBuf>,
    pub alive: bool,
}

/// Blocks until a change arrives, waits [`DEBOUNCE`], then drains what came in.
/// Runs on a kernel task thread, never on the interface thread.
pub fn wait_batch(receiver: Receiver<PathBuf>) -> Batch {
    let Ok(first) = receiver.recv() else {
        return Batch {
            receiver,
            paths: Vec::new(),
            alive: false,
        };
    };
    let mut paths = vec![first];
    std::thread::sleep(DEBOUNCE);
    while paths.len() < BATCH_LIMIT {
        match receiver.try_recv() {
            Ok(path) => paths.push(path),
            Err(_) => break,
        }
    }
    Batch {
        receiver,
        paths,
        alive: true,
    }
}

/// True for paths inside `.git`, which change constantly and are never listed.
pub fn is_git_internal(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str() == ".git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_reports_a_new_file_under_the_root() {
        let root = std::env::temp_dir().join(format!(
            "forja-explorer-watch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let (watcher, receiver) = start(&root).expect("watcher starts");
        // start() returns only after the OS watcher is attached.
        std::fs::write(root.join("fresh.txt"), "x").unwrap();
        loop {
            let path = receiver.recv_timeout(Duration::from_secs(60)).expect("watcher did not report a change");
            if path.starts_with(&root) { break; }
        }
        drop(watcher);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_internal_paths_are_recognized() {
        assert!(is_git_internal(Path::new("/repo/.git/index")));
        assert!(!is_git_internal(Path::new("/repo/src/main.rs")));
        assert!(!is_git_internal(Path::new("/repo/.github/x.yml")));
    }

    #[test]
    fn a_dropped_sender_ends_the_batch_chain() {
        let (sender, receiver) = channel::<PathBuf>();
        drop(sender);
        let batch = wait_batch(receiver);
        assert!(!batch.alive);
        assert!(batch.paths.is_empty());
    }
}
