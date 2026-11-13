# forja-module-chat

Aba "Ajudante (chat)": conversa com um CLI de agente, lida ao vivo da
transcrição e enviada pelo modo headless do perfil.

## O que registra

- Aba `chat.tab` na bancada.
- Comandos `chat.open`, `chat.send`, `chat.cancel` e `chat.status`.
- RPC dos mesmos nomes (`chat.open` aceita `profile_id`/`profile` e
  `bench_path`/`bench`).
- Configurações `chat.font.size`, `chat.message.max_width`,
  `chat.input.enter_sends`, `chat.attachments.max_mb`,
  `chat.attachments.paste_images`, `chat.tools.show_calls`,
  `chat.tools.collapse_output` e `chat.diff.apply_mode`, com os padrões e
  faixas de `design/settings.toml`.

## Modelo de mensagens

`model.rs` define texto, código, chamada de ferramenta (entrada/saída/estado),
edição de arquivo com diff, lista de tarefas, raciocínio e uso de tokens. O
mesmo modelo alimenta a UI e os parsers.

## Transcrições

`transcript.rs` entende três formatos confirmados nos perfis:

- **Claude Code**: JSONL com `type`/`message`;
- **Codex**: JSONL com `response_item`, `event_msg` e `item_completed`;
- **OpenCode**: JSON exportado com `info` e `messages[].parts`.

`TranscriptTail` lê só linhas completas novas e guarda a cauda parcial para a
próxima consulta. O caminho da sessão vem de `[sessions]` do perfil
(`~/.claude`, `CODEX_HOME`, `OPENCODE_DB`), nunca de código antigo.

## Turnos headless

`profile.rs` monta os argumentos a partir do perfil:

- Claude Code: `-p --output-format stream-json --verbose`, `--resume {id}`,
  permissão, modelo, esforço;
- Codex: `exec --json`, `resume {id}`, permissão, modelo, esforço.

`runner.rs` inicia o processo com stdin do prompt, sem janela de console, e
converte cada linha em `StreamEvent`. Cancelar usa `taskkill /PID <pid> /T /F`.
`stream.rs` interpreta `system/init`, `assistant`, `user`, `result`, `error`,
`thread.started`, `item.completed` e `turn.completed`.

A fila (`TurnQueue`) mantém um turno por vez: `chat.send` com turno ativo
enfileira; ao terminar, o próximo começa. `helper.monitor.idle_minutes` para um
turno sem eventos.

## Composer

- `@arquivo` lista arquivos da bancada (caminhada com `ignore`, respeitando
  gitignore) e substitui o token pelo caminho.
- `/comando` lista arquivos `.md` de `<bancada>/.forja/commands`,
  `~/.forja/commands` e `content/commands`; comandos do agente são enviados como
  `/nome`.
- Anexar imagem usa o seletor do sistema e respeita `chat.attachments.max_mb`.
- Modelo e esforço vêm de `[models]` do perfil.
- "Terminal" retoma a mesma sessão por `archive.resume` quando o id é conhecido
  e o módulo archive existe; sem isso, abre a TUI do perfil por `helper.launch`.

## Contrato

Consome `bench.current`, `helper.launch`, `helper.monitor.idle_minutes` (setting)
e `chat.*`. Publica comandos/RPC `chat.open`, `chat.send`, `chat.cancel` e
`chat.status`. `helper.launch` com `helper.launch.mode = chat` chama
`chat.open` e, com prompt, `chat.send`.

## Limitações

- Só Claude Code e Codex têm runner estruturado; os outros perfis abrem a
  transcrição, mas `chat.send` devolve erro claro.
- A lista de arquivos do composer não consulta o módulo `find` porque o
  contrato público dele ainda só abre o picker; a caminhada local respeita as
  mesmas regras de gitignore.
- O streaming ao vivo de turnos usa o stdout do runner; a transcrição é a
  fonte ao abrir uma sessão existente.

## Como testar

```sh
cargo test -p forja-module-chat
cargo run -p forja-app
forja chat open --profile_id claude-code --session_id <id> --bench_path <pasta>
forja chat status
```

## Lote 3 (cobertura do catálogo)

- `chat.diff.apply_mode` (`diff_mode.rs`): o ajudante grava os arquivos; o modo só muda
  como o bloco de diff aparece (`review`: recolhido e marcado como aguardando revisão;
  `inline`: aberto; `auto`: recolhido e marcado como aplicado).
- `chat.attachments.paste_images` (`clipboard.rs`): Ctrl+V com imagem na área de
  transferência do Windows salva um PNG em `<dados>/modules/chat-pastes/` em segundo
  plano e anexa o arquivo.
