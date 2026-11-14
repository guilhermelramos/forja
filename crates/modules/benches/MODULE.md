# forja-module-benches

Módulo `bench` (pasta `benches`): worktrees git, etapas e os scripts do
`forja.toml`.

## O que registra

- Comandos/RPC `bench.current`, `bench.list`, `bench.list_cards`,
  `bench.activate`, `bench.create`, `bench.store`, `bench.stage.set`,
  `bench.stage.list`, `bench.sleep`, `bench.wake`, `bench.pin`, `bench.read`,
  `bench.refresh` e `bench.script.approve`.
- Comandos de interface `bench.create_dialog`, `bench.store_dialog` e
  `bench.approve_dialog`; os diálogos são desenhados pela view
  `bench.dialogs` no slot modal, aberta por `poll` só quando há diálogo. Sem
  projeto, o diálogo de criação explica o que falta e oferece "Adicionar pasta".
- Os comandos que precisam de `bench_id`, `project_id` ou `stage` declaram
  `CommandField`s: na Bigorna o valor sai de uma lista filtrável (`bench.list`,
  `project.list`, `bench.stage.list`) ou é digitado (`name`), em vez de falhar
  com argumento ausente.
- Atalhos `bench.create` (`Ctrl+Shift+N`), `bench.next` / `bench.previous`
  (`Alt+PageDown` / `Alt+PageUp`) e `bench.switch_1..10` (`Alt+1..0`).
- Eventos de contrato `bench.activated`, `bench.created`, `bench.stored`,
  `bench.sleep_changed` e `stage.changed`.
- A view `bench.detail` no slot de aba de bancada (ordem 40): resumo da bancada
  atual e ações. O Quadro é do módulo `board`.
- As 21 configurações do catálogo para o módulo `bench` (worktree, layout,
  preparo, desmonte, fechamento e etapa).

## Bancada = worktree

- A bancada principal é o checkout original. `bench.list` cria o registro dela
  na primeira leitura, sem rodar git.
- `bench.activate` emite `bench.activated` e chama `shell.bench.focus`, quando
  existe, para o shell sair da página do trilho e mostrar as abas da bancada.
- `bench.refresh` lê `git worktree list --porcelain`, atualiza branches e junta
  worktrees externos. Worktrees que sumiram ficam arquivados, nunca apagados
  pelo refresh.
- `bench.create` calcula nome, caminho e branch, cria o worktree e dispara o
  evento `bench.created`. As abas iniciais são abertas com `terminal.open`; se
  o terminal não existir, a criação continua e a dica aparece na tela.
- `bench.store` roda o desmonte aprovado, remove o worktree, mantém ou apaga a
  branch e marca o registro como arquivado. É destrutivo e nunca remove a
  bancada principal.

## `forja.toml`

O arquivo fica na raiz do projeto. O formato é fechado: chave desconhecida é
erro e desliga os scripts, para um typo não passar em silêncio.

```toml
schema_version = 1

# Comandos de shell. Rodam na pasta da bancada, com limite de tempo.
setup = "npm ci"
teardown = "npm run clean"

# Abas iniciais, na ordem. Ids livres; o terminal recebe cada uma.
tabs = ["terminal", "helper"]

[request]
# Modelo exibido no detalhe da bancada quando ela veio de um pedido.
command = "forja pedido abrir {id}"
```

- `setup` só roda quando o hash SHA-256 exato do texto do script foi aprovado
  para aquele projeto. A aprovação fica em `<dados>/modules/bench.json`.
- `setup = ""` e `teardown = ""` são válidos e não rodam nada.
- `tabs` vazio usa `bench.layout.tabs`. Ids são normalizados e duplicatas são
  descartadas com aviso.
- `request.command` não é executado automaticamente; fica disponível para o
  módulo de pedidos e para o detalhe da bancada.
- Sem o arquivo, valem os padrões do catálogo.

## Etapas

As quatro canônicas vêm de `design/stages.md` (`draft`, `forging`, `review`,
`ready`). Extras podem ser declaradas em `<projeto>/.forja/etapas.toml`; o
arquivo do projeto é lido em `bench.stage.list` e em `bench.stage.set`.
Transições para trás exigem `confirm_backward: true`; `ready` exige revisão ou
merge; `forging -> draft` é recusado depois de commits novos. Toda transição
persistida emite `stage.changed` com ator, origem, motivo e horário.

## Segurança e desempenho

- Worktree, script e rede rodam em `Ctx::spawn`; a interface só lê estado.
- Scripts têm `bench.setup.timeout_s` e rodam com `FORJA_BENCH` apontando para a
  bancada.
- Git usa `GIT_TERMINAL_PROMPT=0` e stdin nulo.
- `bench.worktree.auto_prune_days > 0` remove registros arquivados antigos no
  `bench.refresh`; nunca apaga uma pasta que ainda exista.

## Como testar

```sh
cargo test -p forja-module-benches
```

Os testes criam repositórios temporários, criam e guardam bancadas, aprovam um
hash e conferem que o preparo só roda depois da aprovação.

## Lote 3 (cobertura do catálogo)

- `bench.layout.restore`: ao sair de uma bancada, guarda as abas abertas
  (`BenchRecord.open_tabs`); ao voltar, reabre as que foram fechadas (`tabs.rs`).
- `bench.stage.sync_request`: liga o pedido de origem (`BenchRecord.request_key`) em
  `stage.changed.request`; sem a configuração, o campo fica vazio.
