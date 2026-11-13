# forja-module-archive

Acervo de sessões dos ajudantes: índice tantivy incremental, busca com trechos,
retomada, bifurcação e exclusão para a lixeira.

## O que registra

- Página `archive.tab` "Acervo" no trilho (`Slot::Rail`, glifo `ARCHIVE`).
- Comandos/RPC `archive.search`, `archive.resume`, `archive.delete`,
  `archive.list` e `archive.reindex`.
- Configurações `archive.sessions.index`, `archive.sessions.retention_days`,
  `archive.sessions.max_gb` e `archive.transcripts.keep`, com os padrões e
  faixas de `design/settings.toml`.

## Índice

`index.rs` usa tantivy em `%APPDATA%\Forja\archive-index` (derivado do store do
módulo, então `FORJA_DATA_DIR` isolado também isola o índice). Campos:
`doc_key`, `session_id`, `helper_id`, `project`, `path`, `started`, `updated`,
`title` e `text`.

A varredura é incremental por tamanho + mtime do arquivo. Arquivos que sumiram
saem do índice; arquivos alterados são reindexados. Tudo roda em `cx.spawn`, sem
bloquear a interface. A retenção remove sessões mais antigas que o configurado;
`archive.transcripts.keep = false` indexa só título/projeto.
`helper.sessions.keep_transcripts = false` também impede indexar texto; mudar
qualquer uma das duas políticas reconstrói o índice na próxima varredura.

## Descoberta e formatos

`profile.rs` lê os mesmos perfis de `content/helpers` e resolve:

- **Claude Code**: `CLAUDE_CONFIG_DIR` ou `~/.claude/projects/**/*.jsonl`;
- **Codex**: `CODEX_HOME` ou `~/.codex/sessions/rollout-*.jsonl`;
- **OpenCode**: `OPENCODE_DB` ou `~/.local/share/opencode/opencode.db`, com os
  JSONs de `storage/session`.

Formatos sem parser confirmado (SQLite do Gemini/goose/kiro, plugins, etc.) não
são inventados: ficam fora do índice.

`parse.rs` extrai id, projeto, título, texto e datas de cada formato. O texto é
limitado a 2 MB por sessão.

## Retomar e bifurcar

O argv vem de `[resume]`/`[fork]` do perfil, com `{id}` substituído:

- Claude Code: `--resume <id>` e `--fork-session` somado;
- Codex: `resume <id>` e `fork <id>`;
- OpenCode: `--session <id>` e `--fork` somado.

`archive.resume` chama `terminal.open` com a pasta da bancada (ou do projeto) e
a linha montada. `fork = true` escolhe `[fork]`.

## Excluir

`archive.delete` exige `confirm = true`, move o arquivo para a lixeira com a
crate `trash` e remove o documento do índice. A aba mostra um modal de
confirmação antes de chamar o comando.

## Contrato

Consome `bench.current` e `terminal.open`. Publica os comandos/RPC da lista
acima. Não importa o crate de outro módulo.

## Limitações

- Sem SQLite: sessões de CLIs que só gravam em banco ficam fora até haver um
  parser confirmado.
- O índice é reconstruído se o schema mudar.
- A varredura é disparada pela aba ou por `archive.reindex`; a CLI mostra o
  resultado depois que a tarefa em segundo plano termina.

## Como testar

```sh
cargo test -p forja-module-archive
cargo run -p forja-app
forja archive reindex
forja archive list
forja archive search --query parser --profile_id claude-code
```
