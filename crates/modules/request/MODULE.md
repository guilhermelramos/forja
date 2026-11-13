# request

Página **Pedidos**: lista unificada dos pedidos de todos os balcões instalados, com
filtros, busca, gaveta de detalhes, comentários, fechar ou reabrir e o botão **Forjar**
(cria a bancada a partir do pedido). Guarda também os pedidos locais.

## O que registra

- Aba `request.page` (slot `bench_tab`, título `request-tab-title`). Aparece na faixa de abas
  mesmo sem bancada ativa. A entrada pelo trilho depende de um slot de página no shell.
- Comandos: `request.list`, `request.refresh`, `request.forge`, `request.comment`,
  `request.transition`, `request.open`, `request.local.create`.
- Método RPC `request.list` (CLI: `forja request list --json`).
- Configurações do catálogo (`design/settings.toml`, grupo "Pedidos e balcões"):
  `request.default.source`, `request.create.branch_from_title`, `request.create.assign_self`,
  `request.filters.default`, `request.refresh.interval_s`, `request.sync.stage_on_pr`,
  `request.sync.close_on_merge`, `request.local.prefix`, `request.local.counter_start`.

## Contratos

Publica nada além dos comandos acima. Consome `counter.result` (de cada balcão),
`counter.sources`, `counter_<nome>.*`, `bench.current`, `bench.create`, `bench.activated`,
`bench.created` e `helper.launch`. Ausente qualquer um deles, a tela degrada e avisa.

Detalhes e formas de JSON: `design/contracts.md`, seção "Pedidos e balcões".

## Filtros

| id | Rótulo pt-BR · en | Escopo ao balcão | Mostra |
|---|---|---|---|
| `mine` | Meus · Mine | `mine` | atribuídos a mim ou abertos por mim |
| `to_review` | Para revisar · To review | `to_review` | pedidos em que sou revisor |
| `open` | Abertos · Open | `open` | pedidos abertos |
| `recent` | Recentes · Recent | `all` | atualizados nos últimos 7 dias |
| `all` | Todos · All | `all` | tudo |

## Forjar

`bench.create {project_id, name, from_request}`, com `name` = `pedido-<chave>-<título>`.
Se a bancada não chega com o caminho, espera `bench.created` com o mesmo nome. Depois sobe
o ajudante com `helper.launch`, se `helper.launch.auto_start` estiver ligado.

## Pedidos locais

Guardados em `<dados>/modules/request.json`. Leitura e gravação rodam em `Ctx::spawn`.

## Como testar

```sh
cargo test -p forja-module-request
FORJA_FIXTURES=1 cargo run -p forja-app -- --screenshot shots/requests-page.png
```

Os testes usam um `bench.create` e um `helper.launch` falsos, registrados num módulo de
teste (`src/tests.rs`). `FORJA_FIXTURES=1` faz os balcões devolverem as fixtures, sem rede,
e abre o primeiro pedido para mostrar a gaveta.

## Lote 3 (cobertura do catálogo)

- `request.default.source`: a página Pedidos começa no balcão configurado (padrão
  `local`) e tem uma linha para trocar entre local, cada balcão instalado e todos.
  O refresh pede só o balcão escolhido.
- `request.sync.stage_on_pr` e `request.sync.close_on_merge` (`sync.rs`): recebem
  `stage.changed` com o pedido de origem. Etapa `review` marca o pedido local em revisão;
  etapa `ready` fecha o pedido local. Pedidos de balcão remoto não são escritos aqui.
