//! Política de isolamento de bancadas entre tarefas do mesmo mutirão.

/// Como as tarefas dividem (ou não) bancadas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    /// Cada tarefa ganha uma bancada própria.
    Worktree,
    /// Todas as tarefas usam a mesma bancada.
    Shared,
}

impl Isolation {
    /// Lê a política configurada; valor desconhecido cai em `worktree`.
    pub fn parse(value: &str) -> Self {
        match value {
            "shared" => Self::Shared,
            _ => Self::Worktree,
        }
    }
}

/// Onde a tarefa deve rodar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BenchChoice {
    /// Usar uma bancada já pronta.
    Use(String),
    /// Pedir uma bancada nova ao `bench.create`.
    Create,
}

/// Compara caminhos de bancada sem depender de barras ou caixa.
fn same_bench(left: &str, right: &str) -> bool {
    fn normalize(value: &str) -> String {
        value
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    }
    normalize(left) == normalize(right)
}

/// Decide a bancada da tarefa. No isolamento `worktree`, reaproveitar a
/// bancada de outra tarefa do mesmo mutirão é recusado: cada execução precisa
/// da sua própria árvore.
pub fn choose(
    isolation: Isolation,
    explicit: Option<&str>,
    run_bench: Option<&str>,
    current: Option<&str>,
    used_by_others: &[String],
    target_new: bool,
) -> Result<BenchChoice, String> {
    if isolation == Isolation::Shared {
        return explicit
            .or(run_bench)
            .or(current)
            .map(|path| BenchChoice::Use(path.to_owned()))
            .ok_or_else(|| "informe uma bancada para o mutirão compartilhado".to_owned());
    }
    if target_new {
        return Ok(BenchChoice::Create);
    }
    let path = explicit
        .or(run_bench)
        .or(current)
        .ok_or_else(|| "informe uma bancada para a tarefa".to_owned())?;
    if used_by_others
        .iter()
        .any(|used| same_bench(used, path))
    {
        return Err(
            "o isolamento worktree exige uma bancada própria por tarefa".to_owned(),
        );
    }
    Ok(BenchChoice::Use(path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_reuses_a_bench_already_used_by_another_task() {
        let used = vec!["C:/bancadas/mesma".to_owned()];
        let choice = choose(
            Isolation::Shared,
            None,
            Some("C:\\bancadas\\mesma\\"),
            None,
            &used,
            false,
        )
        .unwrap();
        assert_eq!(choice, BenchChoice::Use("C:\\bancadas\\mesma\\".to_owned()));
    }

    #[test]
    fn worktree_refuses_a_bench_used_by_another_task() {
        let used = vec!["C:/bancadas/mesma".to_owned()];
        let error = choose(
            Isolation::Worktree,
            Some("C:\\bancadas\\mesma\\"),
            None,
            None,
            &used,
            false,
        )
        .unwrap_err();
        assert!(error.contains("bancada própria"), "{error}");
    }

    #[test]
    fn worktree_accepts_a_new_bench_and_a_distinct_one() {
        assert_eq!(
            choose(Isolation::Worktree, None, None, None, &[], true).unwrap(),
            BenchChoice::Create
        );
        assert_eq!(
            choose(
                Isolation::Worktree,
                Some("C:/bancadas/outra"),
                None,
                None,
                &["C:/bancadas/mesma".to_owned()],
                false,
            )
            .unwrap(),
            BenchChoice::Use("C:/bancadas/outra".to_owned())
        );
    }

    #[test]
    fn unknown_policy_falls_back_to_worktree() {
        assert_eq!(Isolation::parse("shared"), Isolation::Shared);
        assert_eq!(Isolation::parse("isolada"), Isolation::Worktree);
    }
}
