# Cobertura do catálogo (`design/settings.toml` e `design/keymap.toml`)

> Gerado automaticamente por `cargo xtask catalog`. Não edite à mão; atualize os módulos ou o catálogo e rode `cargo xtask catalog` novamente.

## Resumo

| Métrica | Valor |
|---|---|
| Configurações no catálogo | 311 |
| Configurações registradas no app | 271 (270 do catálogo + 1 extras) |
| Configurações com efeito real no código | 270 (269 do catálogo) |
| Configurações registradas sem efeito | 1 (1 do catálogo) |
| Configurações do catálogo ausentes no app | 2 |
| Configurações adiadas | 39 |
| Divergências de tipo ou padrão | 0 |
| Divergências de faixa, opções ou módulo | 0 |
| Atalhos no catálogo | 110 |
| Atalhos registrados no app | 110 (110 do catálogo + 0 extras) |
| Atalhos do catálogo ausentes no app | 0 |
| Atalhos com tecla ou escopo divergente | 0 |

## Divergências entre catálogo e app

Nenhuma divergência de tipo, padrão, faixa, opções, módulo ou tecla.

## Configurações adiadas

- `appearance.ui.font_family`: Aguardando API de seleção de fontes no ui-kit.
- `appearance.ui.font_size`: Aguardando métricas tipográficas configuráveis no ui-kit.
- `appearance.ui.code_font`: Aguardando API de seleção de fontes de código no ui-kit.
- `appearance.ui.code_size`: Aguardando métricas tipográficas configuráveis no ui-kit.
- `appearance.ui.ligatures`: Aguardando suporte de composição tipográfica a ligaduras no renderizador.
- `appearance.motion.duration_ms`: Aguardando duração de animação configurável nos tokens do ui-kit.
- `appearance.tabs.middle_click`: Aguardando contrato de fechamento das abas do shell.
- `appearance.tooltips.enabled`: Aguardando controle compartilhado de dicas nos componentes da interface.
- `helper.launch.initial_context`: Aguardando contrato de contexto do pedido e preparação assíncrona do lançamento.
- `helper.launch.include_rules`: Aguardando leitura assíncrona de regras no contrato de lançamento do ajudante.
- `helper.launch.ready_timeout_s`: Aguardando sinal de prontidão verificável do terminal e dos perfis de ajudante.
- `helper.sessions.resume_policy`: Aguardando ciclo de vida persistente de sessões e retomada automática.
- `voice.input.model_path`: Aguardando o motor local de transcrição (whisper.cpp) ser aprovado como dependência.
- `voice.output.engine`: Aguardando a síntese de voz (SAPI ou Piper) ser aprovada como dependência.
- `voice.output.model_path`: Aguardando a síntese de voz local (Piper) ser aprovada como dependência.
- `voice.output.rate`: Aguardando a síntese de voz (SAPI ou Piper) ser aprovada como dependência.
- `voice.output.auto_read`: Aguardando a síntese de voz (SAPI ou Piper) ser aprovada como dependência.
- `muster.coordinator.profile`: Aguardando ciclo de vida do ajudante coordenador.
- `muster.dispatch.target`: Aguardando contrato de conclusão e falha da criação assíncrona de bancada.
- `muster.escalation.max_retries`: Aguardando verificação de encerramento e identidade para reenvio seguro.
- `muster.cleanup.on_finish`: Aguardando rastreamento de posse das bancadas e conclusão assíncrona do arquivamento.
- `muster.workspace.isolation`: Aguardando contrato de criação e prontidão de bancada isolada.
- `mcp.client.enabled`: Cliente MCP ainda não implementado.
- `mcp.startup.timeout_s`: Cliente MCP ainda não implementado.
- `mcp.tools.auto_approve`: Cliente MCP ainda não implementado.
- `mcp.log.requests`: Aguardando observação de requisições na ponte MCP e serviço de log.
- `browser.privacy.third_party_cookies`: Aguardando a API de perfil do WebView2 para bloquear cookies de terceiros.
- `remote.sync.mode`: Aguardando contrato de sincronização de arquivos com bancada remota; o módulo ainda não sincroniza arquivos.
- `app.cli.enabled`: Aguardando ciclo de vida cancelável do servidor RPC local.
- `app.storage.data_dir`: Aguardando configuração de bootstrap do diretório de dados no kernel.
- `app.storage.content_dir`: Aguardando configuração de bootstrap do diretório de conteúdo no kernel.
- `app.storage.cleanup_days`: Aguardando contrato de retenção e limpeza dos caches de módulos.
- `app.log.level`: Aguardando serviço de log configurável da aplicação.
- `app.log.max_mb`: Aguardando serviço de log com rotação por tamanho.
- `app.log.keep`: Aguardando serviço de log com retenção de arquivos.
- `app.log.include_terminal_output`: Aguardando integração explícita e controlada da saída de terminal ao log.
- `app.performance.max_fps`: Aguardando limitador de quadros no ciclo de janela.
- `app.performance.worker_threads`: Aguardando configuração de bootstrap do executor de tarefas do kernel.
- `app.dev.mode`: Aguardando ferramentas internas e painel de diagnóstico.

## Configurações (`design/settings.toml`)

| Chave | Módulo (catálogo) | Módulo (app) | Registrada | Tipo/Padrão/Faixa | Efeito |
|---|---|---|---|---|---|
| `appearance.theme.mode` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.theme.dark_name` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.theme.light_name` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.ui.density` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.ui.scale` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.ui.font_family` | `appearance` | `-` | adiada | - | adiada |
| `appearance.ui.font_size` | `appearance` | `-` | adiada | - | adiada |
| `appearance.ui.code_font` | `appearance` | `-` | adiada | - | adiada |
| `appearance.ui.code_size` | `appearance` | `-` | adiada | - | adiada |
| `appearance.ui.ligatures` | `appearance` | `-` | adiada | - | adiada |
| `appearance.motion.reduce` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.motion.duration_ms` | `appearance` | `-` | adiada | - | adiada |
| `appearance.colors.accent` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.sidebar.visible` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.sidebar.width` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.rail.visible` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.status_bar.visible` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.window.remember_geometry` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.window.start_maximized` | `appearance` | `appearance` | sim | ok | sim |
| `appearance.tabs.middle_click` | `appearance` | `-` | adiada | - | adiada |
| `appearance.tooltips.enabled` | `appearance` | `-` | adiada | - | adiada |
| `i18n.language.mode` | `i18n` | `i18n` | sim | ok | sim |
| `i18n.language.fallback` | `i18n` | `i18n` | sim | ok | sim |
| `i18n.time.format` | `i18n` | `i18n` | sim | ok | sim |
| `i18n.date.week_start` | `i18n` | `i18n` | sim | ok | sim |
| `notifications.toast.enabled` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.toast.position` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.toast.duration_s` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.toast.max` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.os.enabled` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.os.only_unfocused` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.os.sound` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.quiet.enabled` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.quiet.start` | `notifications` | `notifications` | sim | ok | sim |
| `notifications.quiet.end` | `notifications` | `notifications` | sim | ok | sim |
| `bench.worktree.location` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.path` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.name_template` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.branch_prefix` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.base_branch` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.fetch_before` | `bench` | `bench` | sim | ok | sim |
| `bench.worktree.auto_prune_days` | `bench` | `bench` | sim | ok | sim |
| `bench.layout.tabs` | `bench` | `bench` | sim | ok | sim |
| `bench.layout.restore` | `bench` | `bench` | sim | ok | sim |
| `bench.setup.policy` | `bench` | `bench` | sim | ok | sim |
| `bench.setup.timeout_s` | `bench` | `bench` | sim | ok | sim |
| `bench.teardown.policy` | `bench` | `bench` | sim | ok | sim |
| `bench.teardown.branch` | `bench` | `bench` | sim | ok | sim |
| `bench.close.confirm_dirty` | `bench` | `bench` | sim | ok | sim |
| `bench.close.stop_helpers` | `bench` | `bench` | sim | ok | sim |
| `board.cards.show_branch` | `board` | `board` | sim | ok | sim |
| `board.cards.show_helper` | `board` | `board` | sim | ok | sim |
| `board.cards.show_request` | `board` | `board` | sim | ok | sim |
| `board.cards.compact` | `board` | `board` | sim | ok | sim |
| `board.refresh.interval_s` | `board` | `board` | sim | ok | sim |
| `bench.stage.creation_default` | `bench` | `bench` | sim | ok | sim |
| `bench.stage.wip_limit` | `bench` | `bench` | sim | ok | sim |
| `bench.stage.sync_request` | `bench` | `bench` | sim | ok | sim |
| `bench.stage.pr_open` | `bench` | `bench` | sim | ok | sim |
| `bench.stage.pr_merge` | `bench` | `bench` | sim | ok | sim |
| `bench.stage.archive_on_ready` | `bench` | `bench` | sim | ok | sim |
| `git.identity.name` | `git` | `git` | sim | ok | sim |
| `git.identity.email` | `git` | `git` | sim | ok | sim |
| `git.commit.sign` | `git` | `git` | sim | ok | sim |
| `git.commit.gpg_key` | `git` | `git` | sim | ok | sim |
| `git.commit.sign_off` | `git` | `git` | sim | ok | sim |
| `git.sync.fetch_interval_min` | `git` | `git` | sim | ok | sim |
| `git.sync.auto_pull` | `git` | `git` | sim | ok | sim |
| `git.push.auto_set_upstream` | `git` | `git` | sim | ok | sim |
| `git.protected.branches` | `git` | `git` | sim | ok | sim |
| `git.status.interval_s` | `git` | `git` | sim | ok | sim |
| `archive.sessions.index` | `archive` | `archive` | sim | ok | sim |
| `archive.sessions.retention_days` | `archive` | `archive` | sim | ok | sim |
| `archive.sessions.max_gb` | `archive` | `archive` | sim | ok | sim |
| `archive.transcripts.keep` | `archive` | `archive` | sim | ok | sim |
| `terminal.shell.default` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.shell.path` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.shell.start_dir` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.shell.env` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.scroll.history_lines` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.scroll.wheel_lines` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.scroll.follow_output` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.cursor.style` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.font.family` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.font.size` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.theme.name` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.theme.follow_app` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.color.opacity` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.bell.mode` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.copy.on_select` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.copy.trim` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.paste.warn_multiline` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.paste.bracketed` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.links.confirm` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.links.open_with` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.shell.marks` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.close.confirm_running` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.gpu.mode` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.osc52.enabled` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.recipes.press_enter` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.recipes.confirm` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.recipes.show_scope` | `terminal` | `terminal` | sim | ok | sim |
| `terminal.recipes.shell` | `terminal` | `terminal` | sim | ok | sim |
| `editor.font.family` | `editor` | `editor` | sim | ok | sim |
| `editor.font.size` | `editor` | `editor` | sim | ok | sim |
| `editor.line.height` | `editor` | `editor` | sim | ok | sim |
| `editor.gutter.line_numbers` | `editor` | `editor` | sim | ok | sim |
| `editor.indent.auto_detect` | `editor` | `editor` | sim | ok | sim |
| `editor.indent.size` | `editor` | `editor` | sim | ok | sim |
| `editor.indent.tabs` | `editor` | `editor` | sim | ok | sim |
| `editor.wrap.mode` | `editor` | `editor` | sim | ok | sim |
| `editor.wrap.column` | `editor` | `editor` | sim | ok | sim |
| `editor.highlight.current_line` | `editor` | `editor` | sim | ok | sim |
| `editor.view.minimap` | `editor` | `editor` | sim | ok | sim |
| `editor.brackets.guides` | `editor` | `editor` | sim | ok | sim |
| `editor.whitespace.show` | `editor` | `editor` | sim | ok | sim |
| `editor.save.auto` | `editor` | `editor` | sim | ok | sim |
| `editor.save.delay_ms` | `editor` | `editor` | sim | ok | sim |
| `editor.save.format` | `editor` | `editor` | sim | ok | sim |
| `editor.save.trim_trailing` | `editor` | `editor` | sim | ok | sim |
| `editor.save.final_newline` | `editor` | `editor` | sim | ok | sim |
| `editor.files.encoding` | `editor` | `editor` | sim | ok | sim |
| `editor.files.eol` | `editor` | `editor` | sim | ok | sim |
| `editor.files.large_mb` | `editor` | `editor` | sim | ok | sim |
| `editor.search.regex` | `editor` | `editor` | sim | ok | sim |
| `editor.search.case_sensitive` | `editor` | `editor` | sim | ok | sim |
| `editor.search.whole_word` | `editor` | `editor` | sim | ok | sim |
| `editor.markdown.preview` | `editor` | `editor` | sim | ok | sim |
| `diff.view.mode` | `diff` | `diff` | sim | ok | sim |
| `diff.context.lines` | `diff` | `diff` | sim | ok | sim |
| `diff.whitespace.ignore` | `diff` | `diff` | sim | ok | sim |
| `diff.view.collapse_unchanged` | `diff` | `diff` | sim | ok | sim |
| `diff.notes.inline` | `diff` | `diff` | sim | ok | sim |
| `diff.notes.send_mode` | `diff` | `diff` | sim | ok | sim |
| `find.results.limit` | `find` | `find` | sim | ok | sim |
| `find.search.respect_gitignore` | `find` | `find` | sim | ok | sim |
| `find.search.include_hidden` | `find` | `find` | sim | ok | sim |
| `find.recent.count` | `find` | `find` | sim | ok | sim |
| `find.grep.engine` | `find` | `find` | sim | ok | sim |
| `find.grep.context` | `find` | `find` | sim | ok | sim |
| `explorer.tree.show_ignored` | `explorer` | `explorer` | sim | ok | sim |
| `explorer.tree.follow_symlinks` | `explorer` | `explorer` | sim | ok | sim |
| `helper.default.profile` | `helper` | `helper` | sim | ok | sim |
| `helper.default.permissions` | `helper` | `helper` | sim | ok | sim |
| `helper.launch.auto_start` | `helper` | `helper` | sim | ok | sim |
| `helper.launch.initial_context` | `helper` | `-` | adiada | - | adiada |
| `helper.launch.include_rules` | `helper` | `-` | adiada | - | adiada |
| `helper.launch.mode` | `helper` | `helper` | sim | ok | sim |
| `helper.launch.ready_timeout_s` | `helper` | `-` | adiada | - | adiada |
| `helper.monitor.idle_minutes` | `helper` | `helper` | sim | ok | sim |
| `helper.notifications.waiting` | `helper` | `helper` | sim | ok | sim |
| `helper.notifications.done` | `helper` | `helper` | sim | ok | sim |
| `helper.notifications.error` | `helper` | `helper` | sim | ok | sim |
| `helper.sessions.resume_policy` | `helper` | `-` | adiada | - | adiada |
| `helper.sessions.keep_transcripts` | `helper` | `helper` | sim | ok | sim |
| `helper.hooks.enabled` | `helper` | `helper` | sim | ok | sim |
| `helper.hooks.timeout_s` | `helper` | `helper` | sim | ok | sim |
| `helper.close.confirm_running` | `helper` | `helper` | sim | ok | sim |
| `chat.font.size` | `chat` | `chat` | sim | ok | sim |
| `chat.message.max_width` | `chat` | `chat` | sim | ok | sim |
| `chat.input.enter_sends` | `chat` | `chat` | sim | ok | sim |
| `chat.attachments.max_mb` | `chat` | `chat` | sim | ok | sim |
| `chat.attachments.paste_images` | `chat` | `chat` | sim | ok | sim |
| `chat.tools.show_calls` | `chat` | `chat` | sim | ok | sim |
| `chat.tools.collapse_output` | `chat` | `chat` | sim | ok | sim |
| `chat.diff.apply_mode` | `chat` | `chat` | sim | ok | sim |
| `voice.mode.enabled` | `voice` | `voice` | sim | ok | sim |
| `voice.input.engine` | `voice` | `voice` | sim | ok | sim |
| `voice.input.model_path` | `voice` | `-` | adiada | - | adiada |
| `voice.input.language` | `voice` | `voice` | sim | ok | sim |
| `voice.output.engine` | `voice` | `-` | adiada | - | adiada |
| `voice.output.model_path` | `voice` | `-` | adiada | - | adiada |
| `voice.output.rate` | `voice` | `-` | adiada | - | adiada |
| `voice.output.auto_read` | `voice` | `-` | adiada | - | adiada |
| `voice.privacy.keep_audio` | `voice` | `voice` | sim | ok | sim |
| `usage.cost.track` | `usage` | `usage` | sim | ok | sim |
| `usage.budget.daily_usd` | `usage` | `usage` | sim | ok | sim |
| `usage.budget.monthly_usd` | `usage` | `usage` | sim | ok | sim |
| `usage.display.status_bar` | `usage` | `usage` | sim | ok | sim |
| `accounts.confirm.remove` | `accounts` | `accounts` | sim | ok | sim |
| `muster.mode.enabled` | `muster` | `muster` | sim | ok | sim |
| `muster.coordinator.profile` | `muster` | `-` | adiada | - | adiada |
| `muster.plan.max_tasks` | `muster` | `muster` | sim | ok | sim |
| `muster.plan.require_approval` | `muster` | `muster` | sim | ok | sim |
| `muster.dispatch.max_parallel` | `muster` | `muster` | sim | ok | sim |
| `muster.dispatch.target` | `muster` | `-` | adiada | - | adiada |
| `muster.notes.poll_s` | `muster` | `muster` | sim | ok | sim |
| `muster.notes.retention_days` | `muster` | `muster` | sim | ok | sim |
| `muster.handoff.require_evidence` | `muster` | `muster` | sim | ok | sim |
| `muster.escalation.max_retries` | `muster` | `-` | adiada | - | adiada |
| `muster.budget.max_usd` | `muster` | `muster` | sim | ok | sim |
| `muster.cleanup.on_finish` | `muster` | `-` | adiada | - | adiada |
| `muster.notifications.task_failed` | `muster` | `muster` | sim | ok | sim |
| `muster.workspace.isolation` | `muster` | `-` | adiada | - | adiada |
| `routine.mode.enabled` | `routine` | `routine` | sim | ok | sim |
| `routine.schedule.timezone` | `routine` | `routine` | sim | ok | sim |
| `routine.schedule.missed` | `routine` | `routine` | sim | ok | sim |
| `routine.schedule.grace_s` | `routine` | `routine` | sim | ok | sim |
| `routine.run.timeout_min` | `routine` | `routine` | sim | ok | sim |
| `routine.run.concurrency` | `routine` | `routine` | sim | ok | sim |
| `routine.run.target` | `routine` | `routine` | sim | ok | sim |
| `routine.logs.retention_days` | `routine` | `routine` | sim | ok | sim |
| `routine.notifications.failure` | `routine` | `routine` | sim | ok | sim |
| `routine.notifications.success` | `routine` | `routine` | sim | ok | sim |
| `request.default.source` | `request` | `request` | sim | ok | sim |
| `request.create.branch_from_title` | `request` | `request` | sim | ok | sim |
| `request.create.assign_self` | `request` | `request` | sim | ok | sim |
| `request.filters.default` | `request` | `request` | sim | ok | sim |
| `request.refresh.interval_s` | `request` | `request` | sim | ok | sim |
| `request.sync.stage_on_pr` | `request` | `request` | sim | ok | sim |
| `request.sync.close_on_merge` | `request` | `request` | sim | ok | sim |
| `request.local.prefix` | `request` | `request` | sim | ok | sim |
| `request.local.counter_start` | `request` | `request` | sim | ok | sim |
| `counter.github.host` | `counter` | `counter` | sim | ok | sim |
| `counter.github.include_drafts` | `counter` | `counter` | sim | ok | sim |
| `counter.github.label_filter` | `counter` | `counter` | sim | ok | sim |
| `counter.gitlab.host` | `counter` | `counter` | sim | ok | sim |
| `counter.linear.team` | `counter` | `counter` | sim | ok | sim |
| `counter.jira.project` | `counter` | `counter` | sim | ok | sim |
| `counter.poll.interval_s` | `counter` | `counter` | sim | ok | sim |
| `counter.notifications.new_request` | `counter` | `counter` | sim | ok | sim |
| `counter.notifications.checks_failed` | `counter` | `counter` | sim | ok | sim |
| `mcp.client.enabled` | `mcp` | `-` | adiada | - | adiada |
| `mcp.startup.timeout_s` | `mcp` | `-` | adiada | - | adiada |
| `mcp.tools.auto_approve` | `mcp` | `-` | adiada | - | adiada |
| `mcp.log.requests` | `mcp` | `-` | adiada | - | adiada |
| `mcp.server.enabled` | `mcp` | `mcp` | sim | ok | sim |
| `mcp.server.expose` | `mcp` | `mcp` | sim | ok | sim |
| `mcp.server.auto_approve` | `mcp` | `mcp` | sim | ok | sim |
| `mcp.server.timeout_s` | `mcp` | `mcp` | sim | ok | sim |
| `browser.engine.kind` | `browser` | `browser` | sim | ok | sim |
| `browser.home.url` | `browser` | `browser` | sim | ok | sim |
| `browser.search.engine` | `browser` | `browser` | sim | ok | sim |
| `browser.privacy.third_party_cookies` | `browser` | `-` | adiada | - | adiada |
| `browser.privacy.do_not_track` | `browser` | `browser` | sim | ok | sim |
| `browser.devtools.enabled` | `browser` | `browser` | sim | ok | sim |
| `browser.download.dir` | `browser` | `browser` | sim | ok | sim |
| `browser.download.ask_each` | `browser` | `browser` | sim | ok | sim |
| `browser.zoom.default` | `browser` | `browser` | sim | ok | sim |
| `browser.tabs.restore` | `browser` | `browser` | sim | ok | sim |
| `browser.auth.open_external` | `browser` | `browser` | sim | ok | sim |
| `integrations.webhook.enabled` | `integrations` | `integrations` | sim | ok | sim |
| `integrations.webhook.url` | `integrations` | `integrations` | sim | ok | sim |
| `integrations.webhook.events` | `integrations` | `integrations` | sim | ok | sim |
| `remote.ssh.enabled` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.config_path` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.default_host` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.connect_timeout_s` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.keep_alive_s` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.reuse_connections` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.agent_forward` | `remote` | `remote` | sim | ok | sim |
| `remote.ssh.host_key_policy` | `remote` | `remote` | sim | ok | sim |
| `remote.wsl.default_distro` | `remote` | `remote` | sim | ok | sim |
| `remote.wsl.default_user` | `remote` | `remote` | sim | ok | sim |
| `remote.sync.mode` | `remote` | `-` | adiada | - | adiada |
| `remote.forward.mode` | `remote` | `remote` | sim | ok | sim |
| `remote.latency.warning_ms` | `remote` | `remote` | sim | ok | sim |
| `extension.mode.enabled` | `extension` | `extension` | sim | ok | sim |
| `extension.sandbox.mode` | `extension` | `extension` | sim | ok | sim |
| `extension.update.check` | `extension` | `-` | ausente | - | ausente |
| `extension.update.auto` | `extension` | `-` | ausente | - | ausente |
| `extension.lua.memory_mb` | `extension` | `extension` | sim | ok | sim |
| `extension.lua.timeout_ms` | `extension` | `extension` | sim | ok | sim |
| `extension.fs.allow_outside_project` | `extension` | `extension` | sim | ok | sim |
| `extension.network.allow` | `extension` | `extension` | sim | ok | sim |
| `extension.process.allow_spawn` | `extension` | `extension` | sim | ok | sem efeito |
| `extension.log.level` | `extension` | `extension` | sim | ok | sim |
| `extension.dev.watch` | `extension` | `extension` | sim | ok | sim |
| `content.reload.watch` | `content` | `content` | sim | ok | sim |
| `content.trust.review_on_change` | `content` | `content` | sim | ok | sim |
| `privacy.history.enabled` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.history.retention_days` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.clipboard.clear_after_s` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.logs.redact_secrets` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.secrets.backend` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.secrets.lock_on_idle_min` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.usage.local_only` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.assistant.share_workspace` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.browser.clear_on_exit` | `privacy` | `privacy` | sim | ok | sim |
| `privacy.browser.cookies_days` | `privacy` | `privacy` | sim | ok | sim |
| `network.proxy.mode` | `network` | `network` | sim | ok | sim |
| `network.proxy.url` | `network` | `network` | sim | ok | sim |
| `network.proxy.bypass` | `network` | `network` | sim | ok | sim |
| `network.proxy.auth` | `network` | `network` | sim | ok | sim |
| `network.tls.verify` | `network` | `network` | sim | ok | sim |
| `network.http.timeout_s` | `network` | `network` | sim | ok | sim |
| `network.mode.offline` | `network` | `network` | sim | ok | sim |
| `anvil.history.limit` | `anvil` | `anvil` | sim | ok | sim |
| `anvil.history.show_recent` | `anvil` | `anvil` | sim | ok | sim |
| `keymap.file.custom_path` | `keymap` | `keymap` | sim | ok | sim |
| `keymap.conflict.policy` | `keymap` | `keymap` | sim | ok | sim |
| `app.startup.open_last` | `app` | `app` | sim | ok | sim |
| `app.close.confirm` | `app` | `app` | sim | ok | sim |
| `app.tray.enabled` | `app` | `app` | sim | ok | sim |
| `app.cli.enabled` | `app` | `-` | adiada | - | adiada |
| `app.storage.data_dir` | `app` | `-` | adiada | - | adiada |
| `app.storage.content_dir` | `app` | `-` | adiada | - | adiada |
| `app.storage.cleanup_days` | `app` | `-` | adiada | - | adiada |
| `update.channel.name` | `update` | `update` | sim | ok | sim |
| `update.check.frequency` | `update` | `update` | sim | ok | sim |
| `update.download.auto` | `update` | `update` | sim | ok | sim |
| `app.log.level` | `app` | `-` | adiada | - | adiada |
| `app.log.max_mb` | `app` | `-` | adiada | - | adiada |
| `app.log.keep` | `app` | `-` | adiada | - | adiada |
| `app.log.include_terminal_output` | `app` | `-` | adiada | - | adiada |
| `app.performance.max_fps` | `app` | `-` | adiada | - | adiada |
| `app.performance.worker_threads` | `app` | `-` | adiada | - | adiada |
| `app.dev.mode` | `app` | `-` | adiada | - | adiada |
| `ports.scan.interval_s` | `ports` | `ports` | sim | ok | sim |
| `ports.confirm.kill` | `ports` | `ports` | sim | ok | sim |
| `onboarding.projects.dir` | `onboarding` | `onboarding` | sim | ok | sim |

## Atalhos (`design/keymap.toml`)

| ID | Tecla (catálogo) | Escopo | Registrado | Tecla (app) | Situação |
|---|---|---|---|---|---|
| `anvil.open` | `Ctrl+K` | `global` | sim | `Ctrl+K` | ok |
| `anvil.command_mode` | `Ctrl+Shift+P` | `global` | sim | `Ctrl+Shift+P` | ok |
| `find.file` | `Ctrl+P` | `global` | sim | `Ctrl+P` | ok |
| `find.in_files` | `Ctrl+Shift+F` | `global` | sim | `Ctrl+Shift+F` | ok |
| `bench.create` | `Ctrl+Shift+N` | `global` | sim | `Ctrl+Shift+N` | ok |
| `bench.next` | `Alt+PageDown` | `global` | sim | `Alt+PageDown` | ok |
| `bench.previous` | `Alt+PageUp` | `global` | sim | `Alt+PageUp` | ok |
| `tab.focus` | `F6` | `global` | sim | `F6` | ok |
| `tab.next` | `Ctrl+Tab` | `global` | sim | `Ctrl+Tab` | ok |
| `tab.previous` | `Ctrl+Shift+Tab` | `global` | sim | `Ctrl+Shift+Tab` | ok |
| `tab.close` | `Ctrl+W` | `global` | sim | `Ctrl+W` | ok |
| `tab.close_force` | `Ctrl+Shift+W` | `global` | sim | `Ctrl+Shift+W` | ok |
| `tab.reopen` | `Ctrl+Shift+T` | `global` | sim | `Ctrl+Shift+T` | ok |
| `terminal.toggle` | `Ctrl+Backtick` | `global` | sim | `Ctrl+Backtick` | ok |
| `sidebar.toggle` | `Ctrl+B` | `global` | sim | `Ctrl+B` | ok |
| `panel.toggle` | `Ctrl+Shift+B` | `global` | sim | `Ctrl+Shift+B` | ok |
| `settings.open` | `Ctrl+Comma` | `global` | sim | `Ctrl+Comma` | ok |
| `settings.keymap` | `Ctrl+Shift+Comma` | `global` | sim | `Ctrl+Shift+Comma` | ok |
| `help.keymap` | `Ctrl+Shift+Slash` | `global` | sim | `Ctrl+Shift+Slash` | ok |
| `window.fullscreen` | `F11` | `global` | sim | `F11` | ok |
| `view.zoom.in` | `Ctrl+Shift+Plus` | `global` | sim | `Ctrl+Shift+Plus` | ok |
| `view.zoom.out` | `Ctrl+Shift+Minus` | `global` | sim | `Ctrl+Shift+Minus` | ok |
| `view.zoom.reset` | `Ctrl+Shift+0` | `global` | sim | `Ctrl+Shift+0` | ok |
| `helper.launch.default` | `Ctrl+Shift+A` | `global` | sim | `Ctrl+Shift+A` | ok |
| `helper.launch.profile` | `Ctrl+Alt+Shift+A` | `global` | sim | `Ctrl+Alt+Shift+A` | ok |
| `helper.focus` | `Ctrl+Shift+H` | `global` | sim | `Ctrl+Shift+H` | ok |
| `muster.open` | `Ctrl+Shift+M` | `global` | sim | `Ctrl+Shift+M` | ok |
| `diff.open` | `Ctrl+Shift+D` | `global` | sim | `Ctrl+Shift+D` | ok |
| `bench.switch_1` | `Alt+1` | `global` | sim | `Alt+1` | ok |
| `bench.switch_2` | `Alt+2` | `global` | sim | `Alt+2` | ok |
| `bench.switch_3` | `Alt+3` | `global` | sim | `Alt+3` | ok |
| `bench.switch_4` | `Alt+4` | `global` | sim | `Alt+4` | ok |
| `bench.switch_5` | `Alt+5` | `global` | sim | `Alt+5` | ok |
| `bench.switch_6` | `Alt+6` | `global` | sim | `Alt+6` | ok |
| `bench.switch_7` | `Alt+7` | `global` | sim | `Alt+7` | ok |
| `bench.switch_8` | `Alt+8` | `global` | sim | `Alt+8` | ok |
| `bench.switch_9` | `Alt+9` | `global` | sim | `Alt+9` | ok |
| `bench.switch_10` | `Alt+0` | `global` | sim | `Alt+0` | ok |
| `tab.switch_1` | `Ctrl+1` | `global` | sim | `Ctrl+1` | ok |
| `tab.switch_2` | `Ctrl+2` | `global` | sim | `Ctrl+2` | ok |
| `tab.switch_3` | `Ctrl+3` | `global` | sim | `Ctrl+3` | ok |
| `tab.switch_4` | `Ctrl+4` | `global` | sim | `Ctrl+4` | ok |
| `tab.switch_5` | `Ctrl+5` | `global` | sim | `Ctrl+5` | ok |
| `tab.switch_6` | `Ctrl+6` | `global` | sim | `Ctrl+6` | ok |
| `tab.switch_7` | `Ctrl+7` | `global` | sim | `Ctrl+7` | ok |
| `tab.switch_8` | `Ctrl+8` | `global` | sim | `Ctrl+8` | ok |
| `tab.switch_9` | `Ctrl+9` | `global` | sim | `Ctrl+9` | ok |
| `tab.switch_10` | `Ctrl+0` | `global` | sim | `Ctrl+0` | ok |
| `terminal.copy` | `Ctrl+Shift+C` | `terminal` | sim | `Ctrl+Shift+C` | ok |
| `terminal.paste` | `Ctrl+V` | `terminal` | sim | `Ctrl+V` | ok |
| `terminal.search` | `Ctrl+Shift+F` | `terminal` | sim | `Ctrl+Shift+F` | ok |
| `terminal.clear` | `Ctrl+Shift+K` | `terminal` | sim | `Ctrl+Shift+K` | ok |
| `terminal.split.right` | `Alt+Shift+R` | `terminal` | sim | `Alt+Shift+R` | ok |
| `terminal.split.down` | `Alt+Shift+D` | `terminal` | sim | `Alt+Shift+D` | ok |
| `terminal.pane.next` | `Alt+Shift+PageDown` | `terminal` | sim | `Alt+Shift+PageDown` | ok |
| `terminal.pane.previous` | `Alt+Shift+PageUp` | `terminal` | sim | `Alt+Shift+PageUp` | ok |
| `terminal.pane.close` | `Alt+Shift+W` | `terminal` | sim | `Alt+Shift+W` | ok |
| `terminal.scroll.page_up` | `PageUp` | `terminal` | sim | `PageUp` | ok |
| `terminal.scroll.page_down` | `PageDown` | `terminal` | sim | `PageDown` | ok |
| `terminal.scroll.top` | `Ctrl+Home` | `terminal` | sim | `Ctrl+Home` | ok |
| `terminal.scroll.bottom` | `Ctrl+End` | `terminal` | sim | `Ctrl+End` | ok |
| `editor.undo` | `Ctrl+Z` | `editor` | sim | `Ctrl+Z` | ok |
| `editor.redo` | `Ctrl+Y` | `editor` | sim | `Ctrl+Y` | ok |
| `editor.cut` | `Ctrl+X` | `editor` | sim | `Ctrl+X` | ok |
| `editor.copy` | `Ctrl+C` | `editor` | sim | `Ctrl+C` | ok |
| `editor.paste` | `Ctrl+V` | `editor` | sim | `Ctrl+V` | ok |
| `editor.select_all` | `Ctrl+A` | `editor` | sim | `Ctrl+A` | ok |
| `editor.save` | `Ctrl+S` | `editor` | sim | `Ctrl+S` | ok |
| `editor.find` | `Ctrl+F` | `editor` | sim | `Ctrl+F` | ok |
| `editor.replace` | `Ctrl+H` | `editor` | sim | `Ctrl+H` | ok |
| `editor.find.next` | `F3` | `editor` | sim | `F3` | ok |
| `editor.find.previous` | `Shift+F3` | `editor` | sim | `Shift+F3` | ok |
| `editor.goto.line` | `Ctrl+G` | `editor` | sim | `Ctrl+G` | ok |
| `editor.comment.toggle` | `Ctrl+Slash` | `editor` | sim | `Ctrl+Slash` | ok |
| `editor.quick_fix` | `Ctrl+Period` | `editor` | sim | `Ctrl+Period` | ok |
| `editor.rename` | `F2` | `editor` | sim | `F2` | ok |
| `editor.goto.definition` | `F12` | `editor` | sim | `F12` | ok |
| `editor.peek.definition` | `Alt+F12` | `editor` | sim | `Alt+F12` | ok |
| `editor.completion` | `Ctrl+Space` | `editor` | sim | `Ctrl+Space` | ok |
| `editor.parameter_hint` | `Ctrl+Shift+Space` | `editor` | sim | `Ctrl+Shift+Space` | ok |
| `editor.move_line.up` | `Alt+Up` | `editor` | sim | `Alt+ArrowUp` | ok |
| `editor.move_line.down` | `Alt+Down` | `editor` | sim | `Alt+ArrowDown` | ok |
| `editor.duplicate_line` | `Alt+Shift+Down` | `editor` | sim | `Alt+Shift+ArrowDown` | ok |
| `editor.delete_line` | `Ctrl+Shift+K` | `editor` | sim | `Ctrl+Shift+K` | ok |
| `editor.wrap.toggle` | `Alt+Z` | `editor` | sim | `Alt+Z` | ok |
| `editor.format` | `Ctrl+Alt+F` | `editor` | sim | `Ctrl+Alt+F` | ok |
| `editor.indent` | `Tab` | `editor` | sim | `Tab` | ok |
| `editor.outdent` | `Shift+Tab` | `editor` | sim | `Shift+Tab` | ok |
| `browser.address` | `Ctrl+L` | `browser` | sim | `Ctrl+L` | ok |
| `browser.reload` | `Ctrl+R` | `browser` | sim | `Ctrl+R` | ok |
| `browser.reload_hard` | `Ctrl+Shift+R` | `browser` | sim | `Ctrl+Shift+R` | ok |
| `browser.find` | `Ctrl+F` | `browser` | sim | `Ctrl+F` | ok |
| `browser.new_page` | `Ctrl+T` | `browser` | sim | `Ctrl+T` | ok |
| `browser.reopen_page` | `Ctrl+Shift+T` | `browser` | sim | `Ctrl+Shift+T` | ok |
| `browser.close_page` | `Ctrl+W` | `browser` | sim | `Ctrl+W` | ok |
| `browser.zoom.in` | `Ctrl+Plus` | `browser` | sim | `Ctrl+Plus` | ok |
| `browser.zoom.out` | `Ctrl+Minus` | `browser` | sim | `Ctrl+Minus` | ok |
| `browser.zoom.reset` | `Ctrl+0` | `browser` | sim | `Ctrl+0` | ok |
| `browser.back` | `Alt+Left` | `browser` | sim | `Alt+ArrowLeft` | ok |
| `browser.forward` | `Alt+Right` | `browser` | sim | `Alt+ArrowRight` | ok |
| `browser.devtools` | `F12` | `browser` | sim | `F12` | ok |
| `browser.open_external` | `Alt+Enter` | `browser` | sim | `Alt+Enter` | ok |
| `anvil.close` | `Escape` | `anvil` | sim | `Escape` | ok |
| `anvil.run` | `Enter` | `anvil` | sim | `Enter` | ok |
| `anvil.previous` | `Up` | `anvil` | sim | `ArrowUp` | ok |
| `anvil.next` | `Down` | `anvil` | sim | `ArrowDown` | ok |
| `anvil.group.next` | `Tab` | `anvil` | sim | `Tab` | ok |
| `anvil.group.previous` | `Shift+Tab` | `anvil` | sim | `Shift+Tab` | ok |
| `anvil.run_background` | `Ctrl+Enter` | `anvil` | sim | `Ctrl+Enter` | ok |
| `voice.dictate` | `Ctrl+Alt+V` | `voice` | sim | `Ctrl+Alt+V` | ok |

## Registrados no app mas ausentes do catálogo

### Configurações extras

- `board.stage.offer_helper` (módulo `board`, tipo `toggle`, padrão `true`)

## Configurações registradas sem efeito real

- **`extension`** (1): `extension.process.allow_spawn`

## Backlog priorizado (ausentes e sem efeito)

| Prioridade | Frente | Pendentes | Tamanho estimado | Observação |
|---|---|---|---|---|
| P2 | Settings ausentes: `extension` | 2 | M (~160 L) | Aguardando integração completa de limites da runtime Lua |
| P2 | Settings sem efeito: `extension` | 1 | P (~30 L) | Ligar leitura em runtime e cobrir com testes unitários |
