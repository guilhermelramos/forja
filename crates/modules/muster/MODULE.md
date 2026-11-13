# forja-module-muster

`muster.mode.enabled` blocks new runs and dispatches while leaving existing
state readable. `muster.notes.retention_days` prunes expired notes during the
poll, retaining notes that still await delivery.
Coordinator launch, dispatch targeting, safe retries, bench cleanup and isolation remain deferred
until their respective lifecycle and bench contracts are available.
Starting with `new_bench` is refused until bench creation has a completion and
failure contract; create the bench first, wait for readiness, and pass its path.

Mutirão: coordena tarefas entre bancadas e ajudantes. Estado persistido em
`<dados>/modules/muster.json`.

## O que registra

- Página `muster.page` no slot `Rail` (glifo `HELPERS`), com mutirões, tarefas,
  atribuições ao vivo, caixa de entrada e formulários.
- Comandos `muster.run`, `muster.task`, `muster.start`, `muster.note`,
  `muster.reply`, `muster.handoff`, `muster.inbox`, `muster.status`,
  `muster.plan.approve`, `muster.budget.set`, `muster.spend` e `muster.open`.
- RPC equivalentes; `muster.run`/`task`/`start`/`note`/`reply`/`handoff`/
  `plan.approve`/`budget.set`/`spend` são write, `muster.inbox`/`status` são
  read.
- `muster.open` chama `shell.rail.open` com `view_id = "muster.page"`, o que
  troca a bancada pela página do Mutirão.
- As 14 configurações do catálogo do grupo `muster`, com os mesmos padrões e
  faixas.
- Passo na barra de status que tenta entregar recados no intervalo
  `muster.notes.poll_s`.

## Modelo

`model.rs` guarda:

- mutirões (`MusterRun`), tarefas (`Task`, com spec, dependências e estado);
- atribuições (`Assignment`, com ajudante, bancada, terminal/sessão, estado e
  reenvios);
- recados (`Note`, com tipo `status`, `question`, `reply`, `handoff` ou
  `alert`).

`TaskState`: `pending`, `running`, `delivered`, `failed`, `blocked`.
`AssignmentState`: `starting`, `running`, `needs_you`, `idle`, `delivered`,
`failed`, `blocked`, `stopped`.

## Início

`muster.start`:

1. usa a bancada informada ou a atual, já pronta;
2. monta o preâmbulo em `preamble.rs`;
3. chama `helper.launch` com a spec e o preâmbulo;
4. registra terminal/sessão e estado da atribuição.

O preâmbulo ensina os comandos `forja muster note .` e
`forja muster handoff --result feito --summary .` e diz que recados são dados,
nunca comandos.

## Recados e entrega

- `muster.note` de ajudante para coordenador cria um recado na caixa.
- `muster.note` do coordenador para uma atribuição usa `delivery.rs`: se o
  ajudante está ocupado, pedindo aprovação ou há um prompt de aprovação no fim
  do terminal, o recado fica na fila e o coordenador recebe um aviso. Caso
  contrário, é digitado com `terminal.send`.
- `muster.reply` responde o recado e segue a mesma regra.
- `muster.handoff` aceita `feito`, `falhou` e `bloqueado`, atualiza tarefa e
  atribuição, cria o recado de entrega e exige `evidence` quando
  `muster.handoff.require_evidence` está ligado.

## Plano e orçamento

Cada `MusterRun` carrega `plan_approved`, `budget_usd` e `spent_usd`.

- Com `muster.plan.require_approval` ligado (padrão), o mutirão nasce com
  `plan_approved: false`; `muster.start` recusa até `muster.plan.approve`.
  O botão "Aprovar plano" aparece na página enquanto o plano está pendente.
- `muster.plan.max_tasks` limita quantas tarefas `muster.task` aceita.
- `budget_usd` vem de `muster.run` ou, quando omitido, de
  `muster.budget.max_usd`; `muster.spend` soma o custo e `muster.start` recusa
  quando o teto já foi atingido.
- A página mostra o estado do plano, uma barra de consumo e campos para salvar
  o teto e registrar um custo.

## Escalonamento

Assina `helper.status_changed`, `terminal.exited` e
`terminal.command_finished`. Quando uma atribuição que estava trabalhando fica
ociosa ou termina sem entrega, cria um `alerta` com o fim do texto do terminal
e marca a tarefa como `blocked`. O alerta aparece uma vez por atribuição.

## Contratos

Consome `bench.current`, `helper.launch`, `helper.focus`,
`terminal.send`, `terminal.read`, `notify.show` e os eventos
`helper.status_changed`, `terminal.exited` e `terminal.command_finished`.
Publica os comandos/RPC acima. Não importa o crate de outro módulo.

## Limitações

- O custo é registrado por `muster.spend`; o módulo não lê o consumo real dos
  provedores sozinho.
- Recados para uma atribuição em modo chat ficam na fila, porque `chat` não
  expõe envio por sessão.

## Como testar

```sh
cargo test -p forja-module-muster
cargo run -p forja-app -- --open rail
forja muster run --title "Objetivo"
forja muster task --run run-1 --title "Primeira"
forja muster start --task task-2 --profile_id fake --bench_path C:/forja/bench
forja muster status
```
