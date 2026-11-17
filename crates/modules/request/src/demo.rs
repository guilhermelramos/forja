//! Ten sample local requests for `forja-app --demo`.
//!
//! Written as the module already stores them (`request.json`, `LocalFile`), so
//! the page shows issues and changes without any counter installed.

use crate::local::{LocalFile, COUNTER};
use forja_contracts::{RequestItem, RequestKind, RequestState};
use forja_kernel::Store;

struct Sample {
    title: &'static str,
    kind: RequestKind,
    state: RequestState,
    labels: &'static [&'static str],
    project: &'static str,
    day: u32,
    mine: bool,
    review: bool,
    draft: bool,
}

const SAMPLES: [Sample; 10] = [
    Sample {
        title: "Conferir os números do trimestre",
        kind: RequestKind::Issue,
        state: RequestState::Open,
        labels: &["relatorio", "numeros"],
        project: "oficina",
        day: 3,
        mine: true,
        review: false,
        draft: false,
    },
    Sample {
        title: "Resumo executivo na primeira página",
        kind: RequestKind::Issue,
        state: RequestState::Open,
        labels: &["texto"],
        project: "oficina",
        day: 6,
        mine: true,
        review: false,
        draft: false,
    },
    Sample {
        title: "Gráficos com a cor de destaque",
        kind: RequestKind::Change,
        state: RequestState::Open,
        labels: &["graficos"],
        project: "oficina",
        day: 9,
        mine: false,
        review: true,
        draft: false,
    },
    Sample {
        title: "Corrigir a legenda do segundo gráfico",
        kind: RequestKind::Change,
        state: RequestState::Merged,
        labels: &["graficos", "correcao"],
        project: "oficina",
        day: 12,
        mine: false,
        review: false,
        draft: false,
    },
    Sample {
        title: "Nomes das seções em minúsculas",
        kind: RequestKind::Issue,
        state: RequestState::Closed,
        labels: &["texto"],
        project: "estudio",
        day: 15,
        mine: true,
        review: false,
        draft: false,
    },
    Sample {
        title: "Tabela de diferenças do mês",
        kind: RequestKind::Issue,
        state: RequestState::Open,
        labels: &["numeros", "tabela"],
        project: "estudio",
        day: 18,
        mine: false,
        review: true,
        draft: false,
    },
    Sample {
        title: "Rascunho do anexo técnico",
        kind: RequestKind::Change,
        state: RequestState::Open,
        labels: &["texto", "anexo"],
        project: "estudio",
        day: 21,
        mine: true,
        review: false,
        draft: true,
    },
    Sample {
        title: "Revisar o cabeçalho impresso",
        kind: RequestKind::Issue,
        state: RequestState::Open,
        labels: &["impressao"],
        project: "oficina",
        day: 24,
        mine: false,
        review: true,
        draft: false,
    },
    Sample {
        title: "Fechar a página final",
        kind: RequestKind::Change,
        state: RequestState::Merged,
        labels: &["fechamento"],
        project: "estudio",
        day: 27,
        mine: false,
        review: false,
        draft: false,
    },
    Sample {
        title: "Repetir a conferência depois da revisão",
        kind: RequestKind::Issue,
        state: RequestState::Open,
        labels: &["numeros", "revisao"],
        project: "oficina",
        day: 30,
        mine: true,
        review: false,
        draft: false,
    },
];

/// Writes the ten sample requests.
pub fn demo_seed(store: &Store) -> Result<(), String> {
    let mut items = Vec::with_capacity(SAMPLES.len());
    for (index, sample) in SAMPLES.iter().enumerate() {
        let mut item = RequestItem::new(
            COUNTER,
            format!("LOCAL-{}", index + 1),
            sample.title,
        );
        item.kind = sample.kind;
        item.state = sample.state;
        item.draft = sample.draft;
        item.labels = sample.labels.iter().map(|label| (*label).to_owned()).collect();
        item.project = Some(sample.project.to_owned());
        item.author = Some("voce".to_owned());
        item.mine = sample.mine;
        item.review_requested = sample.review;
        item.updated_at = Some(format!("2026-11-{:02}T10:00:00Z", sample.day));
        item.body = Some(format!(
            "Amostra de demonstração número {}; nenhum pedido real.",
            index + 1
        ));
        items.push(item);
    }
    store
        .save("request", &LocalFile { items })
        .map_err(|error| format!("demo: could not write requests: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_ten_requests_with_issues_and_changes() {
        let dir = std::env::temp_dir().join(format!("forja-request-demo-{}", std::process::id()));
        let store = Store::new(&dir);
        demo_seed(&store).unwrap();
        let file: LocalFile = store.load("request").into_option().expect("requests");
        assert_eq!(file.items.len(), 10);
        assert!(file.items.iter().any(|item| item.kind == RequestKind::Issue));
        assert!(file.items.iter().any(|item| item.kind == RequestKind::Change));
        assert!(file.items.iter().any(|item| item.review_requested));
        let _ = std::fs::remove_dir_all(dir);
    }
}
