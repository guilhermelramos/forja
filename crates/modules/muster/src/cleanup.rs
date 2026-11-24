//! Decisão de limpeza das bancadas criadas pelo Mutirão.
//!
//! A remoção em si acontece no kernel de bancadas, por `bench.store`, nunca
//! por caminho montado aqui. Este módulo só decide quais identificadores podem
//! ser enviados: os que o mutirão criou e que não têm mudanças não comitadas.

use crate::model::MusterFile;
use std::collections::BTreeMap;

/// Política de `muster.cleanup.on_finish`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupPolicy {
    Ask,
    Keep,
    Remove,
}

impl CleanupPolicy {
    /// Lê a política configurada; valor desconhecido cai em `ask`.
    pub fn parse(value: &str) -> Self {
        match value {
            "keep" => Self::Keep,
            "remove" => Self::Remove,
            _ => Self::Ask,
        }
    }
}

/// Bancadas que podem ser removidas e as que ficaram por estar sujas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupPlan {
    pub remove: Vec<String>,
    pub kept_dirty: Vec<String>,
}

/// Só as bancadas que este mutirão criou entram no plano. Bancada suja fica,
/// mesmo quando a política pede remoção. Bancada de outro mutirão, ou que não
/// tem marca de posse, nunca entra no plano.
pub fn plan(
    file: &MusterFile,
    run: &str,
    policy: CleanupPolicy,
    dirty: &BTreeMap<String, bool>,
) -> CleanupPlan {
    let mut result = CleanupPlan::default();
    if policy != CleanupPolicy::Remove {
        return result;
    }
    for owned in file.owned_benches.iter().filter(|item| item.run == run) {
        if dirty.get(&owned.bench_id).copied().unwrap_or(false) {
            result.kept_dirty.push(owned.bench_id.clone());
        } else {
            result.remove.push(owned.bench_id.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MusterFile;

    fn file_with_ownership() -> MusterFile {
        let mut file = MusterFile::default();
        file.own_bench("run-1", "task-1", "bench-ours-clean");
        file.own_bench("run-1", "task-2", "bench-ours-dirty");
        file.own_bench("run-2", "task-3", "bench-other-run");
        file
    }

    #[test]
    fn dirty_bench_is_kept_and_reported() {
        let file = file_with_ownership();
        let dirty = BTreeMap::from([
            ("bench-ours-clean".to_owned(), false),
            ("bench-ours-dirty".to_owned(), true),
            ("bench-other-run".to_owned(), false),
        ]);
        let result = plan(&file, "run-1", CleanupPolicy::Remove, &dirty);
        assert_eq!(result.remove, ["bench-ours-clean"]);
        assert_eq!(result.kept_dirty, ["bench-ours-dirty"]);
    }

    #[test]
    fn benches_without_ownership_are_never_removed() {
        let file = file_with_ownership();
        let dirty = BTreeMap::from([("bench-foreign".to_owned(), false)]);
        let result = plan(&file, "run-1", CleanupPolicy::Remove, &dirty);
        assert!(!result.remove.contains(&"bench-foreign".to_owned()));
        assert!(!result.remove.contains(&"bench-other-run".to_owned()));
        assert_eq!(result.remove, ["bench-ours-clean", "bench-ours-dirty"]);
    }

    #[test]
    fn keep_and_ask_never_remove_a_bench() {
        let file = file_with_ownership();
        let dirty = BTreeMap::new();
        assert_eq!(
            plan(&file, "run-1", CleanupPolicy::Keep, &dirty),
            CleanupPlan::default()
        );
        assert_eq!(
            plan(&file, "run-1", CleanupPolicy::Ask, &dirty),
            CleanupPlan::default()
        );
    }

    #[test]
    fn unknown_policy_falls_back_to_ask() {
        assert_eq!(CleanupPolicy::parse("apagar"), CleanupPolicy::Ask);
        assert_eq!(CleanupPolicy::parse("keep"), CleanupPolicy::Keep);
        assert_eq!(CleanupPolicy::parse("remove"), CleanupPolicy::Remove);
    }
}
