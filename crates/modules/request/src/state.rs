//! Estado da página de Pedidos e a regra que decide o que aparece na lista.
//!
//! Tudo aqui é função pura sobre [`State`], para que a regra seja testada sem a tela.

use forja_contracts::{CounterError, RequestItem, RequestScope, RequestState};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Janela de "Recentes", em dias.
pub const RECENT_DAYS: u64 = 7;

/// Filtros da página. Os ids são os que vão para a configuração `request.filters.default`
/// e para o RPC `request.list`; os rótulos vêm do i18n.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Filter {
    /// Atribuídos a mim ou abertos por mim.
    Mine,
    /// Pedidos de revisão para mim (pull e merge requests).
    ToReview,
    /// Pedidos abertos, de qualquer pessoa.
    Open,
    /// Atualizados nos últimos sete dias, do mais novo para o mais antigo.
    Recent,
    /// Tudo o que os balcões devolvem, mais os pedidos locais.
    #[default]
    All,
}

impl Filter {
    pub const ALL: [Filter; 5] = [
        Filter::Mine,
        Filter::ToReview,
        Filter::Open,
        Filter::Recent,
        Filter::All,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::ToReview => "to_review",
            Self::Open => "open",
            Self::Recent => "recent",
            Self::All => "all",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|filter| filter.id() == id.trim())
    }

    /// Chave i18n do rótulo do filtro.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Mine => "request-filter-mine",
            Self::ToReview => "request-filter-to_review",
            Self::Open => "request-filter-open",
            Self::Recent => "request-filter-recent",
            Self::All => "request-filter-all",
        }
    }

    /// Escopo pedido aos balcões. "Recentes" usa o mesmo escopo de "Todos".
    pub fn scope(self) -> RequestScope {
        match self {
            Self::Mine => RequestScope::Mine,
            Self::ToReview => RequestScope::ToReview,
            Self::Open => RequestScope::Open,
            Self::Recent | Self::All => RequestScope::All,
        }
    }
}

/// Projeto da bancada ativa: o que os balcões precisam para ler o remoto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Ok,
    Warn,
    Error,
}

/// Aviso da página. `key` é uma chave i18n, sem argumentos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub key: String,
}

/// Forja pedida antes de a bancada existir. Quando o `bench.created` chega, o ajudante sobe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingForge {
    pub name: String,
    pub reference: forja_contracts::RequestRef,
}

#[derive(Debug, Default)]
pub struct State {
    pub filter: Filter,
    pub filter_initialized: bool,
    pub query: String,
    pub project: Option<Project>,
    /// Último resultado de cada balcão para cada escopo.
    pub remote: BTreeMap<(String, RequestScope), Vec<RequestItem>>,
    /// Erro do último pedido de cada balcão.
    pub errors: BTreeMap<String, CounterError>,
    /// Pedidos em andamento, para mostrar que a lista está carregando.
    pub pending: BTreeSet<(String, RequestScope)>,
    pub local: Vec<RequestItem>,
    pub local_loading: bool,
    pub local_loaded: bool,
    pub selected: Option<(String, String)>,
    pub detail: Option<RequestItem>,
    pub notice: Option<Notice>,
    pub last_refresh: Option<SystemTime>,
    pub needs_refresh: bool,
    pub pending_forge: Vec<PendingForge>,
    /// Balcão mostrado: `None` mostra todos. Começa em `request.default.source`.
    pub source: Option<String>,
    pub source_initialized: bool,
}

pub type Shared = Arc<Mutex<State>>;

/// Lista visível: o que o filtro e a busca deixam passar, sem repetir pedidos.
pub fn visible(state: &State, now: SystemTime) -> Vec<RequestItem> {
    let filter = state.filter;
    let mut items: Vec<RequestItem> = state
        .remote
        .iter()
        .filter(|((_, scope), _)| *scope == filter.scope())
        .flat_map(|(_, list)| list.iter().cloned())
        .collect();
    items.extend(
        state
            .local
            .iter()
            .filter(|item| local_matches(filter, item))
            .cloned(),
    );
    if filter == Filter::Recent {
        let since = rfc3339_utc(unix_seconds(now).saturating_sub(RECENT_DAYS * 86_400));
        items.retain(|item| {
            item.updated_at
                .as_deref()
                .is_some_and(|at| at >= since.as_str())
        });
    }
    if let Some(source) = &state.source {
        items.retain(|item| &item.counter == source);
    }
    let query = state.query.trim().to_lowercase();
    if !query.is_empty() {
        items.retain(|item| matches_query(item, &query));
    }
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert((item.counter.clone(), item.key.clone())));
    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.counter.cmp(&b.counter))
            .then_with(|| a.key.cmp(&b.key))
    });
    items
}

/// Pedidos locais entram em todos os filtros, menos "Para revisar" (não há revisão local).
fn local_matches(filter: Filter, item: &RequestItem) -> bool {
    match filter {
        Filter::Mine => item.mine,
        Filter::Recent | Filter::All => true,
        Filter::ToReview => false,
        Filter::Open => item.is_open(),
    }
}

fn matches_query(item: &RequestItem, query: &str) -> bool {
    let contains = |text: &str| text.to_lowercase().contains(query);
    contains(&item.title)
        || contains(&item.key)
        || item.author.as_deref().is_some_and(contains)
        || item.labels.iter().any(|label| contains(label))
        || item.assignees.iter().any(|name| contains(name))
        || item.project.as_deref().is_some_and(contains)
}

/// Estado de um pedido na lista, para o selo da linha.
pub fn state_key(state: RequestState) -> &'static str {
    match state {
        RequestState::Open => "request-state-open",
        RequestState::Closed => "request-state-closed",
        RequestState::Merged => "request-state-merged",
    }
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Instante em RFC 3339 UTC (`2026-11-12T10:00:00Z`). Comparável como texto com os
/// `updated_at` dos balcões, que usam o mesmo formato ou um prefixo dele.
pub fn rfc3339_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Converte dias desde 1970-01-01 em (ano, mês, dia). Algoritmo de calendário gregoriano.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Intervalo entre duas atualizações, em segundos. Zero desliga o atualizador.
pub fn refresh_interval(seconds: i64) -> Option<Duration> {
    (seconds > 0).then(|| Duration::from_secs(seconds as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(counter: &str, key: &str, title: &str, updated: &str) -> RequestItem {
        let mut item = RequestItem::new(counter, key, title);
        item.updated_at = Some(updated.to_owned());
        item
    }

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn filters_have_stable_ids_and_scopes() {
        assert_eq!(Filter::from_id("to_review"), Some(Filter::ToReview));
        assert_eq!(Filter::from_id("nope"), None);
        assert_eq!(Filter::Recent.scope(), RequestScope::All);
        assert_eq!(Filter::default(), Filter::All);
        for filter in Filter::ALL {
            assert_eq!(Filter::from_id(filter.id()), Some(filter));
        }
    }

    #[test]
    fn rfc3339_matches_known_instants() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn recent_keeps_only_the_last_seven_days() {
        let now = 1_700_000_000;
        let mut state = State {
            filter: Filter::Recent,
            ..State::default()
        };
        state.remote.insert(
            ("github".into(), RequestScope::All),
            vec![
                item("github", "#1", "novo", "2023-11-14T20:00:00Z"),
                item("github", "#2", "velho", "2023-10-01T00:00:00Z"),
            ],
        );
        let keys: Vec<String> = visible(&state, at(now)).into_iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!["#1".to_owned()]);
    }

    #[test]
    fn to_review_hides_local_requests_and_mine_shows_them() {
        let mut state = State::default();
        let mut local = item("local", "LOCAL-1", "rascunho", "2026-01-01T00:00:00Z");
        local.mine = true;
        state.local.push(local);
        state.filter = Filter::ToReview;
        assert!(visible(&state, at(0)).is_empty());
        state.filter = Filter::Mine;
        assert_eq!(visible(&state, at(0)).len(), 1);
    }

    #[test]
    fn search_matches_title_key_labels_and_author_case_insensitively() {
        let mut state = State::default();
        let mut found = item("github", "#7", "Corrigir LOGIN", "2026-01-01T00:00:00Z");
        found.labels.push("auth".into());
        state.remote.insert(("github".into(), RequestScope::All), vec![found]);
        state.query = "login".into();
        assert_eq!(visible(&state, at(0)).len(), 1);
        state.query = "AUTH".into();
        assert_eq!(visible(&state, at(0)).len(), 1);
        state.query = "carlos".into();
        assert!(visible(&state, at(0)).is_empty());
    }

    #[test]
    fn duplicates_are_removed_and_newest_comes_first() {
        let mut state = State::default();
        state.remote.insert(
            ("github".into(), RequestScope::All),
            vec![
                item("github", "#1", "a", "2026-01-01T00:00:00Z"),
                item("github", "#2", "b", "2026-03-01T00:00:00Z"),
                item("github", "#1", "a again", "2026-01-01T00:00:00Z"),
            ],
        );
        let keys: Vec<String> = visible(&state, at(0)).into_iter().map(|i| i.key).collect();
        assert_eq!(keys, vec!["#2".to_owned(), "#1".to_owned()]);
    }

    #[test]
    fn zero_interval_disables_refresh() {
        assert_eq!(refresh_interval(0), None);
        assert_eq!(refresh_interval(60), Some(Duration::from_secs(60)));
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn a_selected_source_shows_only_its_counter() {
        let mut state = State::default();
        state.remote.insert(
            ("github".to_owned(), RequestScope::All),
            vec![RequestItem::new("github", "#1", "a")],
        );
        state.remote.insert(
            ("gitlab".to_owned(), RequestScope::All),
            vec![RequestItem::new("gitlab", "!2", "b")],
        );
        state.filter = Filter::All;
        assert_eq!(visible(&state, SystemTime::UNIX_EPOCH).len(), 2);
        state.source = Some("gitlab".to_owned());
        let shown = visible(&state, SystemTime::UNIX_EPOCH);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].counter, "gitlab");
        state.source = None;
        assert_eq!(visible(&state, SystemTime::UNIX_EPOCH).len(), 2);
    }
}
