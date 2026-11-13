# forja-module-usage

Página **Uso** no trilho: tokens por dia, modelo, projeto e sessões recentes,
lidos das transcrições locais de Claude, Codex e OpenCode.

## O que registra

- Página `usage.page` em `Slot::Rail` (ordem 70, glifo de quadro) e medidor
  `usage.status` na barra de status.
- Comando `usage.refresh` e método RPC `usage.summary` (read, idempotente).
- Configurações `usage.cost.track` e `usage.display.status_bar`, com os padrões do catálogo.
  `usage.cost.alert_usd` está adiada no catálogo (o aviso de custo fica para depois da Fase F).
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`; `keymap.toml` vazio porque o
  catálogo não tem atalho para a página.

## Contratos

- `usage.summary` devolve `UsageSummary` serializado: `today`, `last_7_days`,
  `last_30_days`, `total`, `by_agent`, `by_model`, `by_project`, `by_day` e
  `sessions` (mais recente primeiro).
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
- O custo ainda não é calculado: as duas chaves de custo existem no catálogo,
  mas não há tabela de preços local. O medidor mostra tokens.

## Como testar

```sh
cargo test -p forja-module-usage
cargo run -p forja-app
```

Os testes cobrem os três parsers com fixtures (incluindo delta de total do
Codex), agrupamento por dia/semana/mês/modelo/projeto/sessão e o cache
incremental com um arquivo temporário.
