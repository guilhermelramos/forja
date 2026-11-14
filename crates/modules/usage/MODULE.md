# forja-module-usage

Página **Uso** no trilho: tokens e custo por dia, ajudante, modelo, bancada e
sessões recentes, lidos das transcrições locais de Claude, Codex e OpenCode.

## O que registra

- Página `usage.page` em `Slot::Rail` (ordem 70, glifo de quadro) e medidor
  `usage.status` na barra de status.
- Comandos `usage.refresh` e `usage.spend`; métodos RPC `usage.summary` e
  `usage.spend` (read, idempotentes).
- Configurações `usage.cost.track`, `usage.budget.daily_usd`,
  `usage.budget.monthly_usd` e `usage.display.status_bar`.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`; `keymap.toml` vazio porque o
  catálogo não tem atalho para a página.

## Custo

- O custo informado pelo próprio log vence a estimativa: OpenCode traz `cost`
  por mensagem, Claude aceita `costUSD` por mensagem e o `total_cost_usd` do
  evento `result` (somado uma vez, sem estimar de novo os mesmos tokens), e
  Codex aceita `cost`/`cost_usd` no evento.
- Sem custo no log, o módulo consulta `content/pricing/*.toml` (camadas
  embutida, `~/.forja` e `<bancada>/.forja`). Cada entrada declara preço por
  milhão de tokens de entrada, saída, cache lido e cache escrito, além de
  `source` e `checked` (data da conferência). Erros de leitura/validação
  aparecem na página em vez de virarem preço inventado.
- Modelo fora da tabela mostra **sem preço**; os tokens entram em
  `unpriced_tokens` e ficam fora de `cost_usd`. Modelos `-free`/`:free` custam
  zero.
- `usage.budget.daily_usd` e `usage.budget.monthly_usd` avisam por `notify.show`
  em 80% e 100% do gasto (uma vez por limiar por dia/mês). Zero desliga.

## Contratos

- `usage.summary` devolve `UsageSummary` serializado: `today`, `last_7_days`,
  `last_30_days`, `total`, `by_agent`, `by_model`, `by_project`, `by_day`,
  `periods` (chaves `today`, `last7`, `last30`, `all`) e `sessions` (mais
  recente primeiro). Cada `UsageTotals` carrega `cost_usd` e
  `unpriced_tokens`.
- `usage.spend` recebe `{project?, period?}` e devolve `{period, project,
  matched_projects, usd, tokens, unpriced_tokens, priced, projects,
  scanned_at_ms, pending}`. `project` aceita a pasta da bancada e soma os logs
  dela e das subpastas; `period` aceita `today`, `last7`, `last30` e `all`.
  É o contrato que o Mutirão usa para consultar gasto por bancada em vez de
  pedir o custo digitado.
- Claude lê `<config>/projects/**/*.jsonl` (`type=assistant` e
  `message.usage`); Codex lê `<CODEX_HOME>/sessions/**/rollout-*.jsonl`
  (`session_meta`, `turn_context`, `event_msg/token_count`); OpenCode lê
  `storage/**/*.json` (objetos com `tokens` e sessões com `id`).
- Raízes: `CLAUDE_CONFIG_DIR`/`~/.claude`, `CODEX_HOME`/`~/.codex`, as homes das
  contas ativas (`accounts.roots`), e `APPDATA`/`LOCALAPPDATA`/`~/.local/share`/
  `~/.config/opencode` para o OpenCode.
- O cache é incremental por tamanho + mtime; arquivo inalterado não é relido.
  A varredura roda em `cx.spawn` a cada 60 s, e o medidor respeita
  `usage.display.status_bar`. `usage.cost.track=off` desliga a varredura.
- `FORJA_USAGE_FIXTURE=1` usa um resumo de exemplo para captura de tela.

## Como testar

```sh
cargo test -p forja-module-usage
cargo run -p forja-app
```

Os testes cobrem os três parsers com fixtures (incluindo delta de total do
Codex e custo informado), cálculo de custo com e sem preço, camadas e
diagnósticos da tabela de preços, agregação por ajudante/modelo/bancada com
filtro de período, formato de data por locale, limites de gasto e o contrato
`usage.spend`.
