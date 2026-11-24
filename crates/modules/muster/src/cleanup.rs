//! Decisão de limpeza das bancadas criadas pelo Mutirão.
//!
//! A remoção acontece no kernel de bancadas, por `bench.store`, nunca por
//! caminho montado aqui. Este módulo só decide quais identificadores podem ser
//! enviados: as bancadas que o mutirão possui. O git é conferido pelo módulo de
//! bancadas dentro da tarefa que apaga, logo antes da remoção; o resultado
//! volta em `bench.store_outcome`.

use crate::model::MusterFile;

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

/// Bancadas que este mutirão possui e que podem ser enviadas a `bench.store`.
/// Bancada de outro mutirão, ou sem marca de posse, nunca entra na lista.
pub fn plan(file: &MusterFile, run: &str, policy: CleanupPolicy) -> Vec<String> {
    if policy != CleanupPolicy::Remove {
        return Vec::new();
    }
    file.owned_benches
        .iter()
        .filter(|item| item.run == run)
        .map(|item| item.bench_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MusterFile;

    fn file_with_ownership() -> MusterFile {
        let mut file = MusterFile::default();
        file.own_bench("run-1", "task-1", "bench-ours");
        file.own_bench("run-2", "task-2", "bench-other-run");
        file
    }

    #[test]
    fn only_owned_benches_are_planned() {
        let file = file_with_ownership();
        assert_eq!(plan(&file, "run-1", CleanupPolicy::Remove), ["bench-ours"]);
        assert!(!plan(&file, "run-1", CleanupPolicy::Remove).contains(&"bench-other-run".to_owned()));
        assert!(!plan(&file, "run-1", CleanupPolicy::Remove).contains(&"bench-foreign".to_owned()));
    }

    #[test]
    fn keep_and_ask_never_remove_a_bench() {
        let file = file_with_ownership();
        assert!(plan(&file, "run-1", CleanupPolicy::Keep).is_empty());
        assert!(plan(&file, "run-1", CleanupPolicy::Ask).is_empty());
    }

    #[test]
    fn unknown_policy_falls_back_to_ask() {
        assert_eq!(CleanupPolicy::parse("apagar"), CleanupPolicy::Ask);
        assert_eq!(CleanupPolicy::parse("keep"), CleanupPolicy::Keep);
        assert_eq!(CleanupPolicy::parse("remove"), CleanupPolicy::Remove);
    }
}
