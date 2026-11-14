# Contratos entre módulos

Um módulo **nunca depende do crate de outro**. Eles conversam de duas formas:

1. **Eventos tipados.** Os tipos ficam em `crates/contracts` (`forja-contracts`), e os nomes em `forja_contracts::names`. Quem publica chama `r.contract_event::<T>(NAME)` + `cx.emit(&T)`, e quem consome chama `r.subscribe::<T>(…)`.
2. **Comandos por id**, com argumentos e retorno em JSON documentados abaixo. A chamada é `cx.run_command("terminal.open", args)`.

**Mudanças nos contratos são só aditivas** (tipo novo, campo opcional novo ou comando novo). Quem precisar de um contrato novo o acrescenta aqui e em `forja-contracts`, e avisa na entrega.

## Comandos públicos (onda C1)

| Comando | Dono | Argumentos | Retorno |
|---|---|---|---|
| `project.list` | projects | — | `{projects: [ProjectRef]}` |
| `project.add` | projects | `{path, name?, icon?, color?}` | `ProjectRef` |
| `project.remove` | projects | `{project_id}` | `{removed: true}` (não apaga a pasta) |
| `project.create` | projects | `{path, name?, icon?, color?}` | `{pending: true}`; assíncrono |
| `project.clone` | projects | `{url, path, name?, icon?, color?}` | `{pending: true}`; assíncrono |
| `project.update` | projects | `{project_id, name?, icon?, color?}` | `ProjectRef` |
| `project.add_dialog` | projects | - | `{pending: true}`. Abre o diálogo de adicionar pasta de qualquer tela. |
| `bench.current` | benches | — | `BenchRef \| null` |
| `bench.list` | benches | `{project_id?, include_archived?}` | `{benches: [BenchRef]}` |
| `bench.list_cards` | benches | `{project_id?, include_archived?}` | `{benches: [BenchCard]}` |
| `bench.activate` | benches | `{bench_id}` | `BenchRef` |
| `bench.create` | benches | `{project_id, name, base?, from_request?: RequestRef, fetch?}` | `BenchRef` (assíncrono: evento `bench.created`) |
| `bench.store` | benches | `{bench_id, delete_branch?, force?}` | `{pending: true}` (destrutivo; assíncrono: evento `bench.stored`) |
| `bench.stage.set` | benches | `{bench_id, stage, reason?, confirm_backward?}` | `BenchCard` (emite `stage.changed`) |
| `bench.stage.list` | benches | `{project_id?}` | `{stages: [{id, label, order, parent, locked, dot}], warnings}` |
| `bench.sleep` / `bench.wake` | benches | `{bench_id}` | `BenchCard` (emite `bench.sleep_changed`) |
| `bench.pin` / `bench.read` | benches | `{bench_id, pinned?}` / `{bench_id, unread?}` | `BenchCard` |
| `bench.refresh` | benches | — | `{pending: true}`; assíncrono |
| `bench.script.approve` | benches | `{project_id, kind?}` | `{approved: true, setup_hash, teardown_hash}` |
| `terminal.open` | terminal | `{cwd, command?, args?, env?, title?, shell?}` | `{terminal_id}` |
| `terminal.send` | terminal | `{terminal_id, text, enter?: bool}` | `{}` |
| `terminal.read` | terminal | `{terminal_id, lines?}` | `{text}` |
| `terminal.list` | terminal | `{bench_path?}` | `{terminals: [TerminalRef]}` |
| `terminal.focus` | terminal | `{terminal_id}` | `{focused: bool}`. Usado pela página Ajudantes. |
| `file.open` | editor | `{path, line?, column?}` | `{}`. Também atende o evento `file.open_requested`. |
| `editor.active_file` | editor | - | `{path: string \| null}`. Arquivo da aba ativa, usado pelo blame. |
| `editor.save` / `editor.reload` / `editor.compare` | editor | `{path?}` (ativo quando omitido) | `{pending: true}` |
| `editor.find` | editor | `{query?, replacement?, replace?}` | `{open: true}` |
| `editor.replace.all` | editor | `{path?, find?, replace?}` | `{replaced: n}` |
| `editor.preview` | editor | `{path?}` | `{preview: true}`. Usa `viewers.preview`. |
| `editor.format` | editor | `{path?}` | `{pending: true}` |
| `editor.wrap.toggle` / `editor.close` | editor | `{path?}` | `{wrap: bool}` / `{closed: true}` |
| `diff.open` | diff | `{path?, file?, staged?}` | `{open: true}` ou `{files, detached, upstream}` |
| `diff.refresh` | diff | `{path?}` | `{files, detached, upstream}` |
| `diff.select` | diff | `{path?, file, staged?}` | `{file, binary, hunks}` |
| `diff.send_notes` | diff | `{terminal_id?}` | `{sent: bool, terminal_id?}`. Usa `terminal.send`; prefere `helper.list` e cai para `terminal.list`. |
| `viewers.list` | viewers | - | `{viewers: [ViewerDescriptor]}` |
| `viewers.preview` | viewers | `{path}` | `ViewerDocument` |
| `viewers.open` | viewers | `{path}` | `{opened: bool, viewer?}` |
| `viewers.open_external` | viewers | `{path}` | `{pending: true}` |
| `viewers.close` | viewers | - | `{closed: true}` |
| `git.status` | git | `{path}` | `GitSummary` (emite `git.status_changed`) |
| `git.changes` | git | `{path}` | `{summary, files, upstream, detached}` |
| `git.diff` | git | `{path, file, staged?}` | `{path, staged, binary, hunks}` em JSON |
| `git.stage` / `git.unstage` | git | `{path, files}` | `{pending: true}` |
| `git.discard` | git | `{path, files, confirm: true}` | `{pending: true}` (destrutivo) |
| `git.commit` | git | `{path, message, all?}` | `{pending: true}` |
| `git.push` / `git.pull` / `git.fetch` / `git.sync` / `git.publish` | git | `{path, force_with_lease?, publish?}` | `{pending: true}` (rede) |
| `git.rebase` / `git.abort` / `git.conflicts` | git | `{path, base?}` | `{pending: true}` / `{pending: true}` / `{files}` |
| `git.log` | git | `{path, limit?}` | `{lines: [{graph, commit?}]}` |
| `git.blame` | git | `{path, file}` | `{lines: [{line, hash, short, author, time, content}]}` |
| `notify.show` | notify | `{title, body, level?, bench_path?}` | `{}`. Também atende o evento `notify.requested`. |
| `find.file` | find | — | `{open: true}`. Abre o picker "Achar arquivo" (Ctrl+P). |
| `find.record_open` | find | `{path}` (absoluto, dentro da bancada ativa) | `{recorded: bool}`. Registra uma abertura no histórico. Para o editor e outros módulos que abrem arquivos. |
| `search.focus` | search | — | `{focused: true}`. Leva o foco ao campo do painel Busca (usado por `find.in_files`). |
| `trust.check` | trust | `{kind, source_path, content_hash}` | `{approved: bool, decided_at?}`. `kind`: `setup`, `teardown`, `recipe`, `command`, `extension`, `helper_profile`. |
| `trust.request` | trust | `{kind, source_path, content, summary}` | `{request_id?, approved: bool}`. Sem `request_id`, o conteudo ja esta aprovado. A decisao chega pelo evento `trust.decided`. |
| `trust.revoke` | trust | `{kind, source_path, content_hash?}` | `{revoked: bool}`. Sem hash, remove todas as decisoes daquele caminho. |
| `trust.list` | trust | - | `{entries: [TrustEntry]}`. |
| `helper.detect` | helper | - | `{started: true}`. A deteccao roda em segundo plano; `helper.list` mostra o resultado. |
| `helper.list` | helper | `{bench_path?}` | `{profiles: [HelperProfileInfo]}`. |
| `helper.launch` | helper | `{profile_id?, bench_path?, prompt?, permission?, model?, effort?}` | `{terminal_id, profile_id, command_line, mode}`. `mode = tui` chama `terminal.open`; `mode = chat` (configuração `helper.launch.mode`) chama `chat.open` e, com prompt, `chat.send`, devolvendo `terminal_id = null` e `session_id`. |
| `helper.hook` | helper | `{payload}` | `{}`. Recebe o evento que o modulo `hook` repassa; atualiza o status e publica `helper.status_changed`. |
| `helper.hooks.consent` | helper | `{profile_id, granted}` | `{granted: bool}`. Consentimento explicito por CLI; padrao falso. |
| `helper.hooks.status` | helper | `{profile_id}` | `{state, config_path?, detail?}`. |
| `helper.hooks.install` / `helper.hooks.remove` | helper | `{profile_id}` | `{state, config_path?, detail?}`. Recusa sem consentimento, em testes ou com `FORJA_DATA_DIR` isolado. |
| `chat.open` | chat | `{profile_id, bench_path?, session_id?, permission?, model?, effort?}` | `{profile_id, bench_path, session_id?, transcript?, messages}`. Abre a aba do ajudante com a transcrição da sessão, quando existir. |
| `chat.send` | chat | `{prompt}` | `{queued, pending}`. Sem turno ativo inicia o modo headless do perfil; com turno ativo enfileira. |
| `chat.cancel` | chat | - | `{}`. Cancela o processo do turno e limpa a fila. |
| `chat.status` | chat | - | `{profile_id, session_id?, bench_path, status, messages, pending, usage}`. |
| `archive.search` | archive | `{query?, profile_id?, project?, since_days?, limit?}` | `{hits: [ArchiveHit]}`. Trechos e filtros por ajudante, projeto e data. |
| `archive.resume` | archive | `{profile_id, session_id, bench_path?, path?, fork?}` | `{terminal_id, profile_id, session_id, fork, command_line}`. Usa o argv `[resume]` ou `[fork]` do perfil e chama `terminal.open`. |
| `archive.delete` | archive | `{path, confirm}` | `{deleted: bool}`. Move a transcrição para a lixeira e remove do índice; exige confirmação. |
| `archive.list` | archive | - | `{sessions: [ArchiveSession]}`. Sessões conhecidas pelo cache. |
| `archive.reindex` | archive | - | `{started: bool}`. Inicia a varredura incremental em segundo plano. |
| `search.run` | search | `{pattern, regex?, case_sensitive?, whole_word?, include?, exclude?}` (todos opcionais) | `{started: bool}`. Define o formulário do painel Busca e inicia a busca. |

Comandos da bancada de arquivos (`explorer.*`) são internos ao painel Arquivos
(`crates/modules/explorer/MODULE.md`). Só `explorer.reveal` e `explorer.open_terminal`
são para uso geral; os demais recebem `path` e operam sobre ele.

Operações de bancada e de git que podem demorar (worktree, script, rede ou vários
processos) devolvem um aceite e concluem em segundo plano; o efeito aparece no
evento correspondente ou na próxima leitura. `git.status`, `git.changes`,
`git.diff`, `git.log` e `git.blame` são leituras curtas e respondem na hora.

## Eventos (onda C1)

| Evento | Publicado por | Payload |
|---|---|---|
| `project.added` / `project.removed` | projects | `ProjectAdded` / `ProjectRemoved` |
| `bench.activated` / `bench.created` / `bench.stored` | benches | `BenchActivated` / `BenchCreated` / `BenchStored` |
| `bench.sleep_changed` | benches | `BenchSleepChanged` |
| `stage.changed` | benches | `StageChanged` (`bench_id`, `from`, `to`, `actor`, `origin`, `reason`, `at`, `request?`: chave do pedido de origem, só com `bench.stage.sync_request` ligado; aditivo) |
| `terminal.exited` / `terminal.bell` / `terminal.command_finished` / `terminal.title_changed` | terminal | `TerminalExited` … |
| `file.open_requested` | terminal (links), search, diff… | `FileOpenRequested` |
| `git.status_changed` | git | `GitStatusChanged` |
| `helper.status_changed` | helper (onda C2) | `HelperStatusChanged` |
| `trust.decided` | trust | `TrustDecided` |
| `notify.requested` | qualquer módulo | `NotifyRequested` |

**Ponte de hooks.** O binario `forja-hook` envia `hook.event {payload}` pelo RPC. O modulo `hook` apenas repassa o payload ao comando `helper.hook`; assim o prefixo do metodo pertence ao modulo que o registra e o `helper` continua dono do status.

**Como declarar:** eventos de contrato (tipos de `forja-contracts`) usam `r.contract_event::<T>(names::X)`. Tanto quem publica quanto quem assina declara, em qualquer ordem, e o kernel junta as declarações. Eventos próprios de um módulo continuam com `r.event::<T>("<módulo>.<nome>")`. `subscribe` só é validado no fim do registro.

## Namespaces de contrato
Comandos de papel, que qualquer módulo pode atender, usam um namespace próprio declarado com `r.contract_namespace(...)`:

| Namespace | Papel | Quem atende hoje |
|---|---|---|
| `file` | abrir e editar arquivos (`file.open`) | `editor` |

## Visualizadores (onda C2)

`crates/contracts/src/viewers.rs` é o contrato somente leitura usado pelo
módulo `viewers` e pela prévia do editor:

- `ViewerDescriptor { id, title_key, extensions }` — o registro por extensão,
  devolvido por `viewers.list`;
- `ViewerDocument { viewer, path, title, kind, markdown?, table?, notebook?, image?, pdf?, error? }`
  — o resultado de `viewers.preview`; `kind` é `markdown`, `table`, `notebook`,
  `image` ou `pdf`;
- `MarkdownView { blocks, toc }` com `MarkdownBlock { kind, level, text, language?, callout? }`;
  `callout` é `note`, `tip`, `important`, `warning` ou `caution`;
- `TableView { headers, rows, truncated, delimiter }`;
- `NotebookView { cells, language }` com `NotebookCell { kind, source, outputs }`;
- `ImageView { path, width, height, frames }` e `PdfView { path, pages }`.

O editor chama `viewers.preview` quando a prévia é pedida; sem o módulo
`viewers`, mostra `editor.status.viewer-missing` e continua editando o texto.

## Regra de ausência
Um módulo deve funcionar mesmo que outro não esteja presente. Se `run_command` devolver "comando desconhecido", degrade com elegância. Por exemplo: sem o módulo `editor`, um link de arquivo no terminal abre no app padrão do sistema.

## Pedidos e balcões (onda C2)

Tipos em `crates/contracts/src/requests.rs` (`forja_contracts`). O JSON abaixo é o que os comandos aceitam e devolvem. Um balcão é uma pasta: `crates/modules/counter_<nome>` com os comandos desta seção. O módulo `counter` é dono das configurações `counter.*` e lista os balcões instalados. Os balcões usam `crates/counter-kit` (biblioteca de apoio, não módulo).

### Tipos comuns

| Tipo | Campos principais | Observações |
|---|---|---|
| `RequestItem` | `counter`, `key`, `title`, `kind` (`issue`, `change`), `state` (`open`, `closed`, `merged`), `draft`, `url?`, `author?`, `assignees[]`, `labels[]`, `project?`, `updated_at?`, `mine`, `review_requested`, `body?`, `comments[]`, `checks?` (aditivo: `pending`, `passing`, `failing`; ausente quando o balcão não informa) | `(counter, key)` identifica o pedido. `key` é a chave do próprio balcão: `#123`, `!45`, `ENG-42`, `PROJ-7`, `LOCAL-1`. |
| `RequestComment` | `id?`, `author`, `body`, `created_at?` | Comentários de sistema do GitLab ficam de fora. |
| `RequestRef` | `counter`, `key`, `title`, `url?` | É o `from_request` de `bench.create`. |
| `RequestScope` | `mine`, `to_review`, `open`, `all` | Escopo pedido a um balcão. |
| `CounterError` | `{"kind": ...}`: `not_connected`, `unauthorized` (401), `forbidden` (403), `not_found` (404), `rate_limited` (429, `retry_after_s?`), `rejected` (400/409/422, `message`), `server` (5xx, `status`), `network`, `decode`, `config`, `vault`, `unsupported` | A tela mostra a chave i18n `<módulo>-error-<kind>`, nunca a mensagem do balcão. |
| `CounterResult` | `counter`, `op`, `op_id?`, `scope?`, `items[]`, `item?`, `comment?`, `account?`, `error?` | Payload do evento `counter.result`. |

### Eventos

| Evento | Publicado por | Payload | Consumido por |
|---|---|---|---|
| `counter.result` | cada `counter_<nome>` | `CounterResult` | `request`, cada `counter_<nome>` (seção de Ajustes) |

Os comandos de balcão que fazem rede respondem na hora com `{"started": true}`. O resultado chega em `counter.result`, com o `op_id` de quem chamou. Assim nenhuma chamada HTTP roda na thread da interface.

### Comandos de balcão (`counter_<nome>.*`)

| Comando | Argumentos | Resultado em `counter.result` (`op`) |
|---|---|---|
| `counter_<nome>.describe` | — | Síncrono: `{counter, module, label_key}` (`CounterSource`) |
| `counter_<nome>.list` | `{project_path, scope, op_id?, query?}` | `list`: `items[]`, `scope` |
| `counter_<nome>.get` | `{project_path, key, op_id?}` | `get`: `item` com `body` e `comments` |
| `counter_<nome>.comment` | `{project_path, key, body, op_id?}` | `comment`: `comment`. Corpo vazio é recusado sem chamada de rede. |
| `counter_<nome>.transition` | `{project_path, key, to: open\|closed, op_id?}` | `transition`: `item`. `merged` responde `unsupported`. |
| `counter_<nome>.test` | `{op_id?}` | `test`: `account` |
| `counter_<nome>.connect` | `{token, site?, email?, op_id?}` | `connect`: `account`. Só grava no cofre depois que o balcão aceita o token. |
| `counter_<nome>.disconnect` | `{op_id?}` | `disconnect` |

`project_path` é a pasta da bancada. O balcão lê o remoto `origin` dela e recusa enviar o token se o host do remoto não for o configurado. O Linear e o Jira não usam o remoto: o Linear lê `counter.linear.team`, e o Jira usa o site guardado no cofre.

Escopos por balcão: `to_review` vale para GitHub (busca por revisor pedido) e GitLab (merge requests com `reviewer_username`). Linear e Jira respondem `unsupported`, porque não têm revisão.

Segredos: cofre do Windows, sob o serviço `forja/counter_<nome>`, com a conta sendo o host (ou o site, no Jira). Nunca em arquivo nem em resposta RPC.

### Módulo `request` (página Pedidos)

| Comando | Argumentos | Retorno |
|---|---|---|
| `request.list` (também método RPC: `forja request list --json`) | `{filter?, query?}` | `{filter, query, loading, local_loaded, items[], errors}`: a lista visível agora. Dispara a atualização. |
| `request.refresh` | — | `{started: true}` |
| `request.forge` | `{counter, key}` | `{forged: true, bench, helper}`, `{forged: true, bench, waiting: true}` ou `{forged: false, reason: "bench_unavailable"}` |
| `request.comment` | `{counter, key, body}` | `{started: true}` (balcão) ou o comentário (local) |
| `request.transition` | `{counter, key, to: open\|closed}` | `{started: true}` (balcão) ou o pedido local |
| `request.open` | `{counter, key}` | `{started: true}`: abre a gaveta e pede o pedido completo |
| `request.local.create` | `{title, body?}` | o pedido local criado |

Consome, sem depender de módulo nenhum: `counter.sources`, `counter_<nome>.*`, `bench.current` (projeto atual), `bench.create`, `bench.activated` e `bench.created` (evento), `helper.launch`. Se um comando não existe, a tela degrada e mostra um aviso.

#### Filtros

Os ids vão para `request.filters.default` e para o RPC. Os rótulos vêm do i18n.

| id | pt-BR | en | escopo ao balcão | o que mostra |
|---|---|---|---|---|
| `mine` | Meus | Mine | `mine` | atribuídos a mim ou abertos por mim |
| `to_review` | Para revisar | To review | `to_review` | pedidos em que sou revisor; Linear e Jira não têm revisão |
| `open` | Abertos | Open | `open` | pedidos abertos |
| `recent` | Recentes | Recent | `all` | atualizados nos últimos 7 dias, do mais novo |
| `all` | Todos | All | `all` | tudo o que os balcões devolvem, mais os locais |

Pedidos locais aparecem em todos os filtros, exceto "Para revisar". Busca: título, chave, rótulos, autor, responsável e projeto, sem diferenciar maiúsculas. Ela filtra o que já chegou; não chama o balcão.

#### Forjar

1. `bench.create {project_id, name, from_request: RequestRef}`. O `project_id` vem da bancada atual. O `name` é `pedido-<chave>-<título-curto>`, por exemplo `pedido-123-corrigir-login-do-github`, sem o título se `request.create.branch_from_title` estiver desligado.
2. Se `bench.create` devolve `path`, sobe o ajudante já. Se não devolve, espera o evento `bench.created` com o mesmo `name`.
3. `helper.launch {bench_path, from_request, initial_context: "request"}`, só se `helper.launch.auto_start` estiver ligado (padrão ligado) e o comando existir.

### Armazenamento

Pedidos locais ficam em `<dados>/modules/request.json` (store do kernel). A gravação roda em `spawn` e falha com aviso.

## Rotinas (onda C2)

Módulo `routine` (`crates/modules/routine`). Os formatos abaixo são os do JSON que os comandos
aceitam e devolvem. Um erro de validação não é exceção: volta como `{"saved": false, "error": "<chave>"}`.

### Rotina

| Campo | Tipo | Observações |
|---|---|---|
| `id` | texto | vazio ao criar; o módulo gera |
| `name` | texto | obrigatório |
| `enabled` | bool | rotina desligada não dispara |
| `schedule` | objeto | ver "Agenda" |
| `target` | objeto | `{"kind": "ask"}`, `{"kind": "fixed", "bench_path"}` ou `{"kind": "new", "project_id", "project_path", "name_template"}` |
| `action` | objeto | `{"kind": "prompt", "profile_id": null, "prompt"}` ou `{"kind": "command", "command"}` |
| `precheck` | texto ou `null` | comando de pré-checagem, rodado na pasta do alvo |
| `tolerance_s` | inteiro ou `null` | vazio usa `routine.schedule.grace_s` |
| `created_at`, `last_fire` | unix | o módulo preenche; `routine.save` preserva os dois ao editar |

`name_template` aceita `{rotina}` e `{data}` (AAAA-MM-DD, data local).

#### Agenda

- `{"kind": "preset", "preset": {"name": "hourly"}}`
- `{"kind": "preset", "preset": {"name": "daily", "hour": 9, "minute": 0}}`
- `{"kind": "preset", "preset": {"name": "weekdays", "hour": 9, "minute": 0}}`
- `{"kind": "preset", "preset": {"name": "weekly", "weekday": 1, "hour": 9, "minute": 0}}` (0 é domingo)
- `{"kind": "cron", "expression": "0 9 * * 1-5"}`: cinco campos, em horário local ou UTC (`routine.schedule.timezone`)

#### Execução

`{id, routine_id, routine_name, trigger, status, started, finished?, bench_path?, terminal_id?, summary}`

- `trigger`: `schedule`, `catch_up` ou `manual`.
- `status`: `{"state": "queued"}`, `running`, `succeeded`, `missed`, `{"state": "failed", "reason": …}` ou `{"state": "skipped", "reason": …}`.
- Motivo de falha (`reason`): `{"kind": "timeout"}`, `{"kind": "command_exit", "code"}`, `terminal_unavailable`, `helper_unavailable`, `bench_unavailable`, `terminal_closed`, `interrupted` (o app fechou com a execução em andamento) ou `{"kind": "detail", "message"}`.
- Motivo de pulo (`reason`): `{"kind": "precheck_failed", "code"}`, `{"kind": "needs_target"}` ou `{"kind": "missed_skipped"}`.
- `summary` é o trecho final da saída: texto de histórico, nunca instrução.

Limite: 200 execuções no total.

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `routine.save` | `{routine: Rotina}` | `{saved: true, id}`, ou `{saved: false, error}` com uma de: `routine-error-name_empty`, `routine-error-schedule`, `routine-error-fixed_bench_empty`, `routine-error-new_fields_empty`, `routine-error-prompt_empty`, `routine-error-command_empty`, `routine-error-precheck_empty`, `routine-error-tolerance_out_of_range`, `routine-error-invalid`, `routine-error-loading`, `routine-notice-load-failed` |
| `routine.delete` | `{id}` | `{deleted: true}` |
| `routine.run` | `{id}` | `{run_id}`: id da execução criada. Erro se a rotina não existe, se ainda carrega ou se o arquivo não pôde ser lido |

### Métodos RPC / CLI

| Método | Efeito | Parâmetros | Retorno |
|---|---|---|---|
| `routine.list` | read | — | `{routines: [{id, name, enabled, schedule, next_run, last_status, last_run_at}], running, queued, loaded}`. `schedule` é o texto do agendamento; `next_run` é unix ou `null` (rotina desligada); `last_status` é o estado da última execução |
| `routine.runs` | read | `routine_id?`, `limit?` (padrão 20) | `{runs: [Execução]}`, da mais recente |
| `routine.run` | write | `id` | `{run_id}` |

### Eventos e contratos consumidos

Consome `terminal.open`, `terminal.send`, `terminal.read`, `helper.launch` e `bench.create`, e os eventos `terminal.command_finished`, `terminal.exited`, `helper.status_changed` e `bench.created`. Publica `notify.requested` (`NotifyRequested`) em falhas, e em sucessos se `routine.notifications.success` estiver ligado. O evento é declarado com `contract_event`, como o módulo `notify` faz, e a rotina o emite. Não publica eventos próprios.

### Armazenamento

`<dados>/modules/routine.json` (store do kernel): rotinas, execuções e a data da última checagem. Leitura e gravação em `spawn`; se o arquivo existe e não pôde ser lido, gravar é recusado.

## Remoto (onda C2)

Módulo `remote` (`crates/modules/remote`). Nenhum destino guarda senha, passphrase ou token.

### Destino SSH

| Campo | Tipo | Observações |
|---|---|---|
| `id` | texto | gerado a partir do rótulo (`ssh-<rótulo>`), único entre os destinos; vazio ao criar |
| `label` | texto | nome na tela; vazio usa o host |
| `host` | texto | alias do `~/.ssh/config` ou endereço. Não pode começar com `-` |
| `user` | texto | opcional |
| `port` | inteiro | padrão 22; `0` vira 22 |
| `identity_file` | texto | caminho da chave; só o caminho |
| `proxy_jump` | texto | opcional |
| `forwards` | lista | `[{local_port, remote_host, remote_port, active}]`; `active` sobe sozinho com `remote.forward.mode = auto` |

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `remote.ssh.save` | `{target: Destino}` | `{saved: true, id}`, ou `{saved: false, error}`: `remote-error-invalid`, `remote-error-validation` (com `message`) ou `remote-error-loading` |
| `remote.ssh.delete` | `{id}` | `{deleted: true}`; para os encaminhamentos do destino |
| `remote.ssh.test` | `{id}` | `{started: true}`. O resultado aparece em `remote.ssh.list` |
| `remote.ssh.open` | `{id, remote_dir?}` | o retorno de `terminal.open`, com `ssh.exe` e as opções do destino |
| `remote.ssh.import` | — | `{started: true}`. Lê o `remote.ssh.config_path` e pula hosts já existentes com o mesmo host e rótulo |
| `remote.forward.start` | `{target_id, local_port}` | `{started: true}` |
| `remote.forward.stop` | `{target_id, local_port}` | `{started: true}` |
| `remote.wsl.open` | `{distro?, cwd?}` | o retorno de `terminal.open`, com `wsl.exe`; `cwd` do Windows vira caminho Linux |
| `remote.wsl.wrap` | `{command, distro?, cwd?}` | `{program: "wsl.exe", args, cwd}`. Puro: monta a linha e não roda nada. `command` vazio é erro |

Sem `distro`, usa `remote.wsl.default_distro`, depois a distro padrão já listada.

### Métodos RPC / CLI

| Método | Efeito | Parâmetros | Retorno |
|---|---|---|---|
| `remote.ssh.list` | read | — | `{enabled, targets: [{id, label, host, user, port, probe, forwards}]}`. `probe` é `null` ou `{ok, error, elapsed_ms, slow, at}`; `slow` marca sucesso acima de `remote.latency.warning_ms`. Cada `forwards` traz `running` |
| `remote.ssh.test` | read (network) | `id` | `{started: true}` |
| `remote.forward.list` | read | — | a mesma forma de `remote.ssh.list` |
| `remote.wsl.list` | read | — | `{loading, distros}`: `distros` é `null` até a primeira leitura, depois `[{name, state, version, default}]` |

### Códigos de erro do teste

`probe.error` é uma chave i18n, mapeada do stderr do `ssh`: `remote-error-auth_failed`, `remote-error-host_unreachable`, `remote-error-unknown_host`, `remote-error-unknown_host_key`, `remote-error-host_key_changed` ou `remote-error-other`.

### Eventos e armazenamento

Não publica nem consome eventos. Consome `terminal.open` (terminal) para `ssh.exe` e `wsl.exe`.
Armazenamento: `<dados>/modules/remote.json` (destinos e encaminhamentos, sem segredo), gravado em `spawn`.

## Skills (onda C2)

Módulo `skill` (`crates/modules/skill`). Uma skill é uma pasta com `SKILL.md`, cujo front matter tem `name` e `description`. Raízes: as da Forja (`<dados do usuário>/skills`, que é `~/.forja/skills`, e `<projeto>/.forja/skills`) e as de cada ajudante, lidas do bloco `[skills]` dos perfis em `content/helpers` (`user`, relativa ao home; `project`, relativa à raiz do projeto).

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `skill.refresh` | — | `{started: true}` |
| `skill.install` | `{source, kind?: folder \| zip, owner, scope: user \| project, overwrite?, consent?}` | `{started: true}`. O resultado sai em `skill.list` (`notice`, `conflict`). Erros na hora: `skill-error-no-root`, `skill-error-consent` |
| `skill.copy` | `{from, owner, scope, overwrite?, consent?}` | `{started: true}` |
| `skill.remove` | `{path, consent?}` | `{started: true}`. Manda a pasta para a lixeira |

### Métodos RPC / CLI

| Método | Efeito | Parâmetros | Retorno |
|---|---|---|---|
| `skill.list` | read | — | `{project, scanned, scanning, roots: [{owner, owner_name, scope, path, exists}], skills: [{owner, owner_name, scope, folder, path, name, description, error}], notice: {key, detail} \| null, conflict: caminho \| null}`. Se ainda não foi lida, começa a leitura |
| `skill.install` | write | `source`, `owner`, `scope` (obrigatórios); `kind?`, `overwrite?`, `consent?` | `{started: true}` |

### Regras

- **Sobrescrever.** Se já existe uma skill com o mesmo nome, a operação devolve `conflict` e não altera nada. Com `overwrite: true`, a versão antiga vai para a lixeira.
- **Consentimento.** Escrever na raiz de outro ajudante exige `consent: true`. A tela só envia esse valor depois de mostrar o caminho e o usuário confirmar. O consentimento fica registrado em `<dados>/modules/skill.json`.
- **Zip.** Uma entrada com `..`, caminho absoluto, letra de unidade, `:` no nome ou link simbólico recusa a instalação inteira. Nada é gravado fora da pasta temporária.
- **Nome.** 1 a 64 caracteres, só minúsculas, dígitos e hífens, sem hífen no começo, no fim ou duplo.
- **Erros de front matter** aparecem na lista com a chave: `skill-error-frontmatter-missing`, `skill-error-frontmatter-unterminated`, `skill-error-frontmatter-line`, `skill-error-name-missing`, `skill-error-name-invalid`, `skill-error-description-missing`.
- **Captura.** Com `FORJA_FIXTURES=1`, a lista vem de dados de exemplo, sem disco, e toda escrita é recusada (`skill-error-fixtures`).

### Eventos e contratos consumidos

Consome `bench.current` (pasta do projeto atual). Não publica nem consome eventos.

### Armazenamento

`<dados>/modules/skill.json` (registro de consentimentos, até 500 entradas) e `<dados>/modules/skill-staging/` (pasta temporária de extração de zip, apagada ao fim).

## Voz (onda C2)

Módulo `voice` (`crates/modules/voice`). Ditado pelo Windows (Win+H) pela tecla do keymap, gravação local pelo microfone padrão com medidor, transcrição opcional pela OpenAI, e inserção no terminal, no editor e no chat com confirmação.

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `voice.dictate` | — | `{ok: true}`. Alterna o ditado do Windows (Win+H) |
| `voice.hold` | `{phase: press \| release}` | `{ok: true}`. Liga ao apertar e desliga ao soltar |
| `voice.record.start` | — | `{recording: true}` |
| `voice.record.stop` | — | `{recording: false}` |
| `voice.transcribe` | — | `{started: true}`. Envia o último áudio à OpenAI (exige a chave) |
| `voice.insert` | `{target: terminal \| editor \| chat, terminal_id?, confirmed: true}` | `{inserted: true}`. Sem `confirmed: true`, erro `voice-error-confirm` e nada é inserido |
| `voice.discard` | — | `{discarded: true}` |
| `voice.key.save` | `{key}` | `{started: true}`. Guarda a chave no cofre; a chave não volta em nenhuma resposta |
| `voice.key.delete` | — | `{started: true}` |
| `voice.targets.refresh` | — | `{started: true}`. Lê os terminais com `terminal.list` |

Não há método RPC. Chaves de configuração: `voice.mode.enabled`, `voice.input.engine` (`windows` ou `whisper_cpp`), `voice.input.language` (`auto`, `pt_BR`, `en`), `voice.privacy.keep_audio`.

### Eventos e contratos consumidos

Consome `terminal.list` e `terminal.send` (`{terminal_id, text, enter: false}`), e `editor.insert` e `chat.insert` quando existirem. Sem eles, o destino aparece indisponível. Não publica nem consome eventos.

### Regras

- **Desligada por padrão.** Ditado e gravação exigem `voice.mode.enabled`.
- **`whisper_cpp` não está disponível.** O ditado só usa o motor `windows`; a escolha de `whisper_cpp` responde `voice-error-engine`.
- **Chave.** Só no cofre do Windows (`forja.voice` / `openai`). Nunca em arquivo, nunca no aviso, nunca em resposta.
- **Áudio.** Só vai à OpenAI (`https://api.openai.com/v1/audio/transcriptions`) quando o usuário pede a transcrição. Sem pedido, nada sai da máquina.
- **Inserção.** O texto só vai ao destino depois da confirmação da tela (`confirmed: true`).

### Armazenamento

`<dados>/modules/voice-audio/` guarda os WAV só com `voice.privacy.keep_audio` ligado. Sem a opção, o áudio fica na memória e some ao fechar.

## Shell (onda C3)

| Comando | Argumentos | Retorno |
|---|---|---|
| `shell.rail.open` | `{view_id}` | `{opening, view_id}`. Abre uma página do trilho pelo id da view; usado por `muster.open` e pela Bigorna. |
| `shell.bench.focus` | `{project, bench}` | `{project, bench}`. Sai da página do trilho e mostra as abas da bancada ativa; o cabeçalho passa a nomeá-la. |
| `shell.tab.open` | `{view_id}` | `{opening, view_id}`. Traz uma aba da bancada para a frente (o editor usa em `file.open_requested`). |

## Mutirão (onda C3)

Módulo `muster` (`crates/modules/muster`). Coordena tarefas em bancadas e ajudantes. O estado fica em `<dados>/modules/muster.json`.

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `muster.run` | `{title, objective?, coordinator?, bench_path?, budget_usd?}` | `MusterRun` |
| `muster.task` | `{run, title, spec?, deps?}` | `Task` (`deps` é uma lista separada por vírgulas) |
| `muster.start` | `{task, profile_id?, bench_path?, new_bench?, permission?}` | `{assignment, bench_path, preamble}`. Abre bancada e chama `helper.launch` |
| `muster.note` | `{body, task?, assignment?, kind?, from?}` | `{note, queued, delivered, reason?}`. De ajudante para coordenador ou do coordenador para uma atribuição |
| `muster.reply` | `{note, body}` | `{note, queued, delivered, reason?}` |
| `muster.handoff` | `{summary, result?, evidence?, task?, assignment?}` | `{note, result, delivered}`. `result`: `feito`, `falhou` ou `bloqueado` |
| `muster.plan.approve` | `{run}` | `MusterRun` com `plan_approved: true` |
| `muster.budget.set` | `{run, budget_usd}` | `MusterRun` atualizado |
| `muster.spend` | `{run, usd}` | `MusterRun` atualizado; soma `spent_usd` |
| `muster.inbox` | `{run?, all?}` | `{notes: [Note]}` |
| `muster.status` | `{run?}` | `{runs, tasks, assignments, inbox, loaded, notice}` |
| `muster.open` | - | `{open: true}`. Atalho do trilho |

Métodos RPC equivalentes: `muster.run`, `muster.task`, `muster.start`, `muster.note`, `muster.reply`, `muster.handoff`, `muster.plan.approve`, `muster.budget.set`, `muster.spend` (write) e `muster.inbox`, `muster.status` (read).

### Regras

- **Preâmbulo.** `muster.start` entrega ao ajudante um texto escrito pela Forja com a spec, a bancada e os comandos `forja muster note .` e `forja muster handoff --result feito --summary .`.
- **Recados.** Um recado para um ajudante com status `needs_you` (ou ocupado) fica na fila; o coordenador recebe um aviso. A entrega usa `terminal.send` só quando o estado e o fim do terminal permitem.
- **Escalonamento.** Ajudante que para sem entregar gera um `alerta` com o fim do texto do terminal e a tarefa fica `blocked`.
- **Dados, não comandos.** O corpo de recados e a saída do terminal nunca viram comandos.
- **Entrega.** `muster.handoff.require_evidence` exige `evidence` quando o resultado é `feito`.
- **Plano.** Com `muster.plan.require_approval` ligado, o mutirão nasce com `plan_approved: false`; `muster.start` recusa até `muster.plan.approve`. `muster.plan.max_tasks` limita `muster.task`.
- **Orçamento.** `budget_usd` vem do pedido ou de `muster.budget.max_usd`; `muster.spend` soma o custo e `muster.start` recusa quando o teto já foi atingido.
- **Eventos.** Consome `helper.status_changed`, `terminal.exited` e `terminal.command_finished`; não publica eventos próprios.

## MCP (onda C3)

Módulo `mcp` (`crates/modules/mcp`) e binário `forja-mcp` (`crates/mcp`). O binário é uma ponte stdio sem estado para o pipe do app; fala `initialize`, `tools/list`, `tools/call` e `ping`.

### Comandos

| Comando | Argumentos | Retorno |
|---|---|---|
| `mcp.tools` | - | `{enabled, tools: [{id, name, description, effect, network, idempotent, auto_approve}]}` |
| `mcp.setup` | `{profile_id?}` | `{clients: [{profile_id, name, format, source, snippet, writes}], writes: false}` |
| `mcp.authorize` | `{tool, client?, summary?, effect?}` | `{approved, request_id?, code?}`. Abre o modal; `code = pending` enquanto espera |
| `mcp.approval` | `{request_id}` | `{status, approved}`. `status`: `pending`, `approved`, `denied` ou `timed_out` |

`mcp.tools` é somente leitura e nunca lista métodos destrutivos, aprovações, `terminal.send`, `bench.remove`/`archive` nem o próprio namespace `mcp.*`. Escritas entram só por `mcp.server.expose`; `mcp.server.auto_approve` marca as de baixo risco. Sem autoaprovação, o bridge chama `mcp.authorize` e espera o modal; sem decisão até `mcp.server.timeout_s`, a chamada é negada. `mcp.setup` só mostra o snippet; nunca escreve na configuração de outro CLI.

### Ferramentas MCP

O binário lê `meta.methods` e `mcp.tools`, e monta nomes `forja.<id>` com `inputSchema`/`outputSchema` e anotações (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`). Erros de domínio voltam como `isError` com `{code, message}`; app fechado é `app_closed`. Ferramenta desconhecida é erro JSON-RPC `-32602`.

## Revisão e navegador (onda C3)

Módulos `review` (`crates/modules/review`) e `browser` (`crates/modules/browser`). O primeiro entrega a aba de PR/MR da bancada; o segundo, o navegador embutido.

### Comandos de `review`

| Comando | Argumentos | Retorno |
|---|---|---|
| `review.refresh` | `{bench_path?}` | `{pending: true}`; o estado da aba recebe PR, checks e threads |
| `review.pr.create` | `{bench_path?, title, body, base?, draft?, reviewers?, labels?}` | `{pending: true}` |
| `review.merge` | `{method?, delete_branch?, auto_merge?}` | `{pending: true}` |
| `review.thread.reply` | `{thread_id, body}` | `{pending: true}` |
| `review.thread.resolve` | `{thread_id, resolved?}` | `{pending: true}` |
| `review.checks.rerun` | `{id}` | `{pending: true}` |
| `review.checks.log` | `{id}` | `{log}` ou `{pending: true}` quando o log do job do GitLab ainda vai chegar |
| `review.generate` | `{kind: "commit" \| "pr" \| "branch", bench_path?}` | `{pending: true}`; o texto chega na aba |
| `review.fix` | `{bench_path?}` | resposta de `helper.launch` com o fim do log do check que falhou |

Métodos RPC: `review.refresh` (read), `review.pr.create`, `review.merge` e `review.generate` (write).

Regras:

- O provedor sai do remoto `origin` e do host configurado em `counter.github.host` ou `counter.gitlab.host`; o host do remoto precisa bater com o do balcão antes de usar o token.
- O token é o mesmo salvo pelo balcão (`counter_github` / `counter_gitlab`) no cofre do sistema.
- `review.generate` usa o modo headless do perfil do ajudante (`prompt.headless` e `headless_stdin` do TOML), com timeout de 120 s e contexto de até 12 mil caracteres.
- PR aberto move a bancada para `review` quando `bench.stage.pr_open` não é `never`; merge move para `ready` quando `bench.stage.pr_merge` não é `never`.
- `FORJA_REVIEW_FIXTURE=1` (ou `FORJA_FIXTURES=1`) troca a rede por `fixtures::snapshot`.

### Comandos de `browser`

| Comando | Argumentos | Retorno |
|---|---|---|
| `browser.open` | `{url}` | `{opened, url?}`; abre em aba nova |
| `browser.navigate` | `{url}` | `{opened, url?}`; navega na aba ativa |
| `browser.mermaid.render` | `{source}` | `{opened, version, license}` |
| `browser.open_external` | `{url}` | `{pending: true}` |
| `browser.history` | - | `{open}`; alterna o painel |
| `browser.history.clear` | - | `{cleared: true}` |
| `browser.address` | - | `{focused: true}` |
| `browser.reload`, `browser.reload_hard` | - | `{}` |
| `browser.find` | - | `{open: true}` |
| `browser.new_page`, `browser.reopen_page`, `browser.close_page` | - | `{}` |
| `browser.zoom.in`, `browser.zoom.out`, `browser.zoom.reset` | - | `{}` |
| `browser.back`, `browser.forward` | - | `{}` |

Métodos RPC: `browser.open` (write) e `browser.history` (read, `{entries, count}`).

Regras:

- Até quatro abas vivas. A barra de endereço aceita URL, host sem esquema (localhost usa `http`) ou texto livre, que vira busca no mecanismo configurado.
- O WebView2 é filho da janela da Forja; o pai é achado pelo processo e pela maior janela visível. `WebView2Loader.dll` é copiada pelo `build.rs` e pré-carregada antes de criar a página.
- A página nativa fica oculta quando a aba não está ativa, a janela perde foco ou é minimizada, há overlay/popup aberto ou o histórico está aberto. `FORJA_BROWSER_FORCE_VISIBLE=1` mantém a página para captura.
- Downloads vão para `browser.download.dir` e o nome é reduzido a um arquivo simples; pastas, drives, ADS e nomes de dispositivo são recusados.
- Histórico em `<dados>/browser-history.json`, teto de 500 entradas. Abas de URL em `<dados>/browser-tabs.json`, reabertas quando `browser.tabs.restore` está ligado. Perfis do WebView2 em `<dados>/webview/<slug>-<hash>`, com escopo `shared`, `repository` ou `workspace`.
- Mermaid 11.4.1 é embarcado em `crates/modules/browser/assets/mermaid` com `LICENSE` MIT e `VERSION`, servido por `forja://`; o CDN não é usado.
- `FORJA_BROWSER_URL` abre a página inicial na captura; `FORJA_BROWSER_PROFILE_SCOPE` e `FORJA_BROWSER_WORKSPACE` escolhem o perfil.

## Conteúdo e extensões (fase D)

Módulos `content` (`crates/modules/content`) e `extension`
(`crates/modules/extension`). O primeiro lê arquivos; o segundo roda Lua.

### Comandos de `content`

| Comando | Argumentos | Retorno |
|---|---|---|
| `content.commands.list` | `{scope?, project_path?}` | `{revision, commands[], diagnostics[]}`. Cada comando traz `id` (`cmd.*`), `title`, `scope`, `run`, `layer`, `source_path`, `content_hash`, `blocked` e `block_reason` |
| `content.run` | `{id, arguments?}` | `{id, run, warnings, result}`. Comando de projeto de risco sem `trust` abre o pedido de aprovação e responde `[trust]` |
| `content.themes.list` | `{project_path?}` | `{revision, themes[]}` com `id`, `name`, `kind` (`terminal`/`interface`), `toml`, `layer`, `content_hash` |
| `content.themes.ui` | `{mode?, project_path?}` | `{revision, toml?}`: tema de interface para o modo pedido |
| `content.recipes.list` | `{project_path?}` | `{revision, recipes[]}` com `id`, `title`, `command`, `scope`, `enter`, `directory`, `layer`, `content_hash` |
| `content.templates.list` | — | `{templates[]}` com `id`, `name`, `use`, `layer` |
| `content.templates.get` | `{id}` | `{id, body}` |
| `content.templates.render` | `{id, fields?}` | `{id, text}`; `{{campo}}` é trocado pelo valor |
| `content.reload` | — | `{reloaded, revision}` |

O terminal consulta `content.themes.list` e `content.recipes.list`; sem o
módulo, cai para a leitura local. O chat consulta `content.commands.list` para
o `/comandos`; o painel Arquivos consulta `content.templates.*` no diálogo
"Novo arquivo".

### Comandos de `extension`

| Comando | Argumentos | Retorno |
|---|---|---|
| `extension.list` | — | `{revision, extensions[]}` com estado, capacidades, comandos, configurações, painéis, erros e log |
| `extension.run` | `{extension, command, arguments?}` | valor devolvido pelo `run` Lua |
| `extension.action` | `{extension, action, event}` | valor devolvido pela ação Lua |
| `extension.approve` | `{id}` | `{approved: true}` ou `{pending: true, request_id}` |
| `extension.revoke` | `{id}` | `{revoked: true}` |
| `extension.reload` | `{id}` | `{reloaded: true}` |
| `extension.rescan` | — | `{rescanned: true}` |
| `extension.set` | `{id, key, value}` | `{set: true}` |
| `extension.logs` | `{id, lines?}` | `{id, text}` |

Métodos RPC equivalentes: `extension.list` (read), `extension.run`,
`extension.approve`, `extension.revoke`, `extension.reload`, `extension.rescan`
(write/destructive).

### Eventos entregues ao Lua

`app.ready`, `project.opened`, `bench.selected`, `bench.created`,
`helper.status`, `git.changed` e `settings.changed`. A extensão só recebe o
evento declarado em `[capabilities].events`.

## Importação do Harbor (fase F)

Módulo `import` (`crates/modules/import`). Lê `session.json` e `settings.json` do Harbor 0.1 em `%APPDATA%\Harbor` (ou `FORJA_HARBOR_DIR`), só para leitura. Nenhum segredo é lido. Só comandos; sem eventos nem tipos novos em `forja-contracts`.

| Comando | Argumentos | Retorno |
|---|---|---|
| `import.harbor.status` | — | `{loaded, busy, available, pending, decided, error, items[], summary}`. Não lê disco. `pending` vale quando `session.json` existe e não há decisão. `items[]`: `{id, kind, label, detail, imported}` |
| `import.harbor.preview` | — | `{started: true}`. Lê o Harbor e o store em segundo plano |
| `import.harbor.run` | `{items?: [id]}` | `{started}`; `{started: false, reason: "busy"}` se já há operação. Sem `items`, importa os pendentes. O resultado vai para `summary` |
| `import.harbor.skip` | — | `{started}`. Grava a decisão `skipped` |

Consumidos: `project.list`, `project.add`, `remote.ssh.list`, `remote.ssh.save` e `set_setting`. Gravações fora do store só em `~/.forja` (`recipes/imported.toml`, `imported-notes.md`). Quem consome o status é o passo "Importar do Harbor" do onboarding (`crates/modules/onboarding`), que aparece depois de Pasta quando `pending` é verdadeiro.

## Portas, uso e contas (onda C4)

Módulos `ports`, `usage` e `accounts` (`crates/modules/*`). O primeiro alimenta o painel direito Portas e a barra de status; o segundo, a página Uso; o terceiro, a seção Contas dos Ajustes e o env do `helper.launch`.

### Comandos de `ports`

| Comando | Argumentos | Retorno |
|---|---|---|
| `ports.refresh` | - | `{pending: true}`; invalida o cache e inicia a varredura |
| `ports.kill` | `{pid, port?, confirm?}` | `{pending: true, pid}`; recusa PID 0/4 e o próprio processo |

Método RPC: `ports.list` (read, idempotente) devolve `{ports, scanned_at_ms, busy, error}`. O parser reconhece LISTENING pelo peer `0.0.0.0:0`/`[::]:0`, sem depender do idioma; o nome do processo vem de `tasklist /FO CSV /NH`. O cwd é lido da PEB quando possível e a bancada é o maior caminho de `bench.list` que o contém. `ports.confirm.kill` exige `confirm=true`; a varredura roda em segundo plano no intervalo de `ports.scan.interval_s` (0 desliga).

### Comandos de `usage`

| Comando | Argumentos | Retorno |
|---|---|---|
| `usage.refresh` | - | `{pending: true}`; invalida e relê as transcrições |
| `usage.spend` | `{project?, period?}` | `{period, project, matched_projects, usd, tokens, unpriced_tokens, priced, projects, scanned_at_ms, pending}`; `project` soma a pasta da bancada e as subpastas e `period` aceita `today`, `last7`, `last30` e `all` |

Métodos RPC: `usage.summary` (read, idempotente) devolve `UsageSummary` com `today`, `last_7_days`, `last_30_days`, `total`, `by_agent`, `by_model`, `by_project`, `by_day`, `periods` (chaves `today`, `last7`, `last30`, `all`) e `sessions`; cada `UsageTotals` traz `cost_usd` e `unpriced_tokens`. `usage.spend` (read, idempotente) devolve o gasto do período, opcionalmente somado por bancada, para o Mutirão consultar em vez de pedir custo digitado. O custo vem do log do ajudante quando existe; senão, de `content/pricing/*.toml`, com fonte e data por entrada, e modelo sem preço aparece como "sem preço". As raízes padrão são `CLAUDE_CONFIG_DIR`/`~/.claude`, `CODEX_HOME`/`~/.codex` e as pastas do OpenCode; as homes de `accounts.roots` entram na varredura. O cache é por tamanho + mtime e a varredura roda a cada 60 s, sem bloquear a interface. `usage.budget.daily_usd` e `usage.budget.monthly_usd` avisam por `notify.show` em 80% e 100%.

### Comandos de `accounts`

| Comando | Argumentos | Retorno |
|---|---|---|
| `accounts.list` | - | `{profiles: [{profile_id, name, env, login, installed, executable, accounts}]}` |
| `accounts.add` | `{profile_id, label?}` | `{account, terminal_id}`; cria a home e abre o login |
| `accounts.select` | `{profile_id, id}` | `{selected}` |
| `accounts.remove` | `{profile_id, id, confirm?}` | `{removed}` |
| `accounts.env` | `{profile_id}` | `{env}` da conta ativa, ou `{}` |
| `accounts.roots` | - | `{accounts: [Account]}` para a varredura de uso |

Métodos RPC: `accounts.list` (read), `accounts.add`, `accounts.select` e `accounts.remove` (write).

Regras:

- **Campo novo no perfil.** `[accounts] env = "CLAUDE_CONFIG_DIR"` (e `login = [...]`) é lido dos perfis embutidos e de `~/.forja/helpers`. Claude Code e Codex declaram o campo com a fonte já citada no perfil.
- **Dados.** Contas em `<dados>/accounts/<perfil>/<id>` e estado em `<dados>/modules/accounts.json`. A remoção aceita só `<raiz>/<perfil>/<id>`; `..` e caminhos fora da raiz são recusados. `accounts.confirm.remove` exige `confirm=true`.
- **Login.** `accounts.add` abre `terminal.open` com o executável detectado pelo `helper.list`, os argumentos de `[accounts].login` e o env da conta. Sem terminal ou ajudante, a conta é criada e o login não abre.
- **Env no ajudante.** `helper.launch` chama `accounts.env {profile_id}` e funde `{env}` no plano do terminal. O contrato é `accounts.env` → `{env}`.
- **Captura.** `FORJA_PORTS_FIXTURE`, `FORJA_USAGE_FIXTURE` e `FORJA_ACCOUNTS_FIXTURE` usam dados de exemplo; `FORJA_RIGHT_TAB=ports` põe o painel Portas como primeira aba direita.

## Rede, privacidade e sistema (fase F)

Módulos `network`, `privacy` e `update` (`crates/modules/*`).

| Comando | Dono | Argumentos | Retorno |
|---|---|---|---|
| `network.proxy` | network | - | `NetworkPolicy`: `{mode, url, bypass, auth, tls_verify, timeout_s, offline}` |
| `network.check` | network | `{url?}` | `{allowed, reason, use_proxy, proxy_url, tls_verify, timeout_s, offline}` |
| `privacy.policy` | privacy | - | `{history_enabled, history_retention_days, clipboard_clear_after_s, logs_redact_secrets, secrets_backend, secrets_lock_on_idle_min, usage_local_only, assistant_share_workspace, browser_clear_on_exit, browser_cookies_days}` |
| `privacy.redact` | privacy | `{text}` | `{text, redacted: bool}` |
| `privacy.clipboard.schedule_clear` | privacy | `{text?}` | `{scheduled: bool, clear_after_s}` |
| `privacy.secrets.status` | privacy | `{idle_min?}` | `{backend, lock_on_idle_min, locked: bool}` |
| `privacy.browser.policy` | privacy | - | `{clear_on_exit, cookies_days}` |
| `update.check` | update | `{latest_version?}` | `{channel, frequency, auto_download, current_version, latest_version, update_available, checked: bool}` |
| `update.status` | update | - | `{channel, frequency, auto_download, current_version, last_checked_at?}` |

Clientes HTTP (`forja-counter-kit`, `review`, `voice`, `browser`) consultam `network.proxy` antes de abrir conexões externas e aplicam proxy, lista de exceções (`bypass`), timeout, verificação TLS e bloqueio do modo offline (`network.mode.offline`).

`update.check` recebe a versão mais recente que o chamador já conhece localmente; o módulo compara com `CARGO_PKG_VERSION`, respeita `update.check.frequency` e nunca abre conexão nem envia telemetria. `privacy.*` é local: `privacy.redact` remove tokens/senhas, `privacy.clipboard.schedule_clear` usa a área de transferência do Windows e `privacy.secrets.status` apenas reporta cofre e bloqueio por inatividade.

