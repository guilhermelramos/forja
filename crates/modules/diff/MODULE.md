# forja-module-diff

Módulo `diff` da Forja. Aba "Diferenças" da bancada.

## O que registra

- aba `diff.tab` (ordem 30; `FORJA_BENCH_TAB=diff` a traz para a primeira
  posição em screenshots);
- comandos `diff.open`, `diff.refresh`, `diff.select` e `diff.send_notes`;
- RPC `diff.changes`, `diff.file` e `diff.notes.send`;
- configurações `diff.view.mode`, `diff.context.lines`,
  `diff.whitespace.ignore`, `diff.view.collapse_unchanged`,
  `diff.notes.inline`, `diff.notes.send_mode`;
- atalho `diff.open` (Ctrl+Shift+D) do catálogo;
- bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Desenho

- Lê `git.changes` e `git.diff` do módulo git; nunca importa o crate dele.
- Lista de arquivos agrupada por pasta, com filtro e as letras de índice/worktree.
- Visões lado a lado, em linha e combinada; contexto configurável, recolhimento
  de trechos iguais e opção de ignorar espaços em branco.
- O pareamento de linhas removidas/adicionadas é feito no modelo puro
  (`model::rows`), testado sem UI.
- Diff de imagem: `git.diff` marca binário; a aba lê o blob com `git show`
  (`:arquivo` quando preparado, `HEAD:arquivo` caso contrário) e o arquivo da
  bancada em `cx.spawn`, decodifica em tarefa e sobe as texturas na UI.
- Notas de revisão por linha: copiar todas, limpar e enviar ao terminal do
  ajudante. O envio prefere `helper.list` e cai para `terminal.list`; usa
  `terminal.send`. `diff.notes.send_mode` escolhe a introdução (anexar,
  substituir ou perguntar).
- Assina `bench.activated` e `git.status_changed`; marca o estado como
  desatualizado e atualiza na próxima renderização da aba.

## Contratos

- Consome `git.changes`/`git.diff`, `bench.current`, `terminal.list`/`terminal.send` e,
  quando existir, `helper.list`.
- Eventos de contrato: assina `bench.activated` e `git.status_changed`.
- Degrada com mensagem quando o git, a bancada ou o terminal não existem.

## Como testar

```sh
cargo test -p forja-module-diff
cargo clippy -p forja-module-diff --all-targets -- -D warnings
```

Screenshots: `FORJA_BENCH_TAB=diff FORJA_DIFF_FILE=src/login.ts` e
`FORJA_DIFF_MODE=split|unified|combined`.
