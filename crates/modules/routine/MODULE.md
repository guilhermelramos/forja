# routine

Rotinas agendadas: um prompt para um ajudante, ou um comando num terminal, num horário
(predefinido ou cron de cinco campos), com alvo, pré-checagem, tolerância a horários
perdidos, histórico e a página "Rotinas".

## Arquivos

| Caminho | Papel |
|---|---|
| `src/schedule.rs` | presets, cron de cinco campos, próximo disparo, horário de verão e decisão de disparo com tolerância |
| `src/model.rs` | rotina, execução, histórico (200), retenção, validação e nome da bancada |
| `src/runner.rs` | máquina de estados das execuções: efeitos, fila, pré-checagem, tempo limite, horários perdidos (sem IO) |
| `src/runner/tests.rs` | o fluxo inteiro com efeitos falsos |
| `src/execution.rs` | corte seguro da saída e resumo mostrado no histórico |
| `src/actions.rs` | executor: roda os efeitos, carrega e grava o arquivo, passo periódico |
| `src/state.rs` | estado compartilhado, configurações lidas e mapeamento de status |
| `src/commands.rs` | comandos e métodos RPC, fotos para o CLI e a tela |
| `src/draft.rs` | rascunho do editor, convertido de e para a rotina |
| `src/view.rs` | a página "Rotinas" e o passo da barra de status |
| `src/fixtures.rs` | dados de captura (`FORJA_FIXTURES=1`), nunca gravados no arquivo real |
| `src/tests.rs` | integração com terminal, ajudante e bancada falsos |

## O que registra

- Aba `routine.page` no slot `bench_tab` (ordem 40), título "Rotinas".
- O editor abre em uma janela rolável com posse do teclado; Escape cancela o rascunho.
  Salvar e Cancelar ficam num rodapé fixo, fora da rolagem: um erro de validação
  aparece acima deles sem esconder os botões.
- Passo `routine.tick` no slot `status_bar`: avança o agendador a cada segundo, mesmo com a aba
  fechada, e mostra quantas rotinas rodam.
- Comandos: `routine.save` (grava ou edita; validação devolve um código, não uma exceção),
  `routine.delete`, `routine.run`.
- Métodos RPC/CLI: `routine.list` (read), `routine.runs` (read, `routine_id?`, `limit?`),
  `routine.run` (write, `id`).
- Configurações do grupo "Rotinas" (`design/settings.toml`): `routine.mode.enabled`,
  `routine.schedule.timezone`, `routine.schedule.missed`, `routine.schedule.grace_s`,
  `routine.run.timeout_min`, `routine.run.concurrency`, `routine.run.target`,
  `routine.logs.retention_days`, `routine.notifications.failure`,
  `routine.notifications.success`.

## Contratos

Consome `terminal.open`, `terminal.send`, `terminal.read` (terminal), `helper.launch`
(ajudantes), `bench.create` (bancadas) e os eventos `terminal.command_finished`,
`terminal.exited`, `helper.status_changed` e `bench.created`. Publica `notify.requested` em
falhas (e em sucessos, se a configuração pedir).

Se um comando não existe, a execução falha com o motivo: `terminal_unavailable`,
`helper_unavailable` ou `bench_unavailable`.

Formas de JSON e ids: `design/contracts.md`, seção "Rotinas".

## Regras

- **Agenda.** Cron de cinco campos em horário local (ou UTC, pela configuração). Aceita `*`,
  listas, intervalos, passos e nomes. Domingo é 0 ou 7. Quando dia do mês e dia da semana são
  ambos restritos, vale um ou outro (regra do cron tradicional).
- **Horário de verão.** Horário que não existe na virada é pulado. Horário repetido dispara nas
  duas ocorrências, na ordem em que acontecem.
- **Tolerância.** Um disparo dentro de `grace_s` (ou da tolerância da rotina) roda na hora. Mais
  tarde que isso, é um horário perdido, decidido por `routine.schedule.missed`: `skip` registra
  "pulada"; `run_once` roda uma vez, sem repetir a série; `ask` registra "perdida" e espera.
- **Sem repetição.** Cada rotina guarda o último disparo tratado; reiniciar o app não dispara o
  mesmo horário de novo. Rotina criada não dispara horários de antes de existir.
- **Alvo.** `ask` (padrão para rotina nova, pela configuração `routine.run.target`) registra
  "defina um alvo" e não roda. `fixed` usa uma pasta de bancada. `new` cria uma bancada por
  execução, com o nome `{rotina}`/`{data}` do modelo.
- **Pré-checagem.** Roda na pasta do alvo. Código diferente de zero pula a execução.
- **Comando.** `terminal.open` na pasta da bancada, depois `terminal.send` com a linha. O fim
  vem do evento do terminal (`command_finished` com o código, ou `exited`).
- **Prompt.** `helper.launch {bench_path, prompt, profile_id?}`. Termina quando o ajudante desta
  bancada chega a `finished`.
- **Tempo limite.** `routine.run.timeout_min`. Passou disso, falha com "tempo limite".
- **Concorrência.** `routine.run.concurrency` execuções ao mesmo tempo; as outras esperam na fila.
- **Histórico.** No máximo 200 execuções; mais que `routine.logs.retention_days` dias, somem.
- **Saída é dado.** O resumo (últimas linhas) aparece como texto no histórico e nunca é
  reenviado como instrução a um ajudante.
- **Arquivo.** Leitura e gravação em `spawn`. Se o arquivo existe e não pode ser lido, nada é
  gravado por cima dele. Execuções que estavam em andamento quando o app fechou viram
  "interrompida".

## Como testar

```sh
cargo test -p forja-module-routine
FORJA_FIXTURES=1 cargo run -p forja-app -- --screenshot shots/routines-page.png --slot bench
```

Os testes cobrem: próximo disparo e horário de verão (com um fuso de regras fixas, independente da
máquina), tolerância, pré-checagem, limite do histórico, alvo `ask`, bancada nova, ajudante
ausente, fila e tempo limite, e o fluxo de comando com terminal falso. Nenhum processo roda de
verdade e o arquivo vai para uma pasta temporária.

Gravações concorrentes são serializadas; mudanças ocorridas durante uma gravação são reunidas em um novo snapshot ao concluir.
