# forja-module-explorer

Módulo `explorer`: o painel direito **Arquivos**. Mostra a árvore da bancada
atual, mantém o cache atualizado pelo sistema e faz as operações de arquivo
com desfazer e refazer.

## O que registra

- Painel `explorer.panel` no slot `right_panel` (ordem 10).
- Comandos (ids em inglês, títulos em `i18n/`):
  - `explorer.new_file`, `explorer.new_folder` (argumentos: `path` da pasta, `name` opcional;
    sem `name` abre o diálogo de nome);
  - `explorer.rename` (`path`, `name` opcional), `explorer.duplicate` (`path`),
    `explorer.trash` (`path`; move para a lixeira do Windows);
  - `explorer.undo`, `explorer.redo`;
  - `explorer.copy_absolute_path`, `explorer.copy_relative_path` (`path`);
  - `explorer.reveal` (`path`), `explorer.open_terminal` (`path` = pasta);
  - `explorer.refresh`.
- Configurações: `explorer.tree.show_ignored` (padrão falso) e
  `explorer.tree.follow_symlinks` (padrão falso).
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.
- Sem atalhos: o catálogo não tem atalhos do painel (`keymap.toml` só tem comentário).

## Contratos

Consome (ver `design/contracts.md`):

- `bench.activated` (benches, `BenchActivated`): troca a raiz da árvore, o watcher
  e o cache. Este módulo declara e assina o evento (`contract_event` + `subscribe`).
- `git.status_changed` (git, `GitStatusChanged`): os caminhos alterados publicados
  pelo git viram badges na árvore (`changed.rs`), sem ler git nem disco na view.
- `bench.current` (benches): lido uma vez ao iniciar e de novo enquanto não há
  bancada (ela pode aparecer depois). Sem o módulo `benches`, o painel mostra
  "Nenhuma bancada ativa".
- `terminal.open` (terminal) com `{cwd}`, para "abrir terminal aqui". Sem o módulo,
  mostra um aviso.
- `file.open` (editor) com `{path, line?}`, quando nenhum módulo assina
  `file.open_requested`. Sem o editor, abre com o app padrão do Windows.

Publica:

- `file.open_requested` (`FileOpenRequested`, `contract_event`): é emitido ao clicar
  em um arquivo, **se algum módulo assina** (o editor). Sem assinante, o painel usa o
  comando `file.open` e, sem ele, o app padrão. A checagem é feita pelas assinaturas
  do registro, porque todo publicador declara o evento e `emit` sempre responde.

## Como funciona

- Disco só em tarefas do kernel (`Ctx::spawn`). Listagens e operações de arquivo
  rodam em tarefas. A interface lê só o cache.
- Cache por pasta (`tree.rs`), com geração: resultados de uma bancada antiga são
  descartados. `.gitignore` é respeitado por pasta, com a regra do `ignore`.
- Watcher recursivo (`notify`) na raiz. Os caminhos alterados invalidam só as
  pastas que estão no cache. Mudanças em `.git` são ignoradas.
- Operações (`ops.rs`): criar, renomear, duplicar (pasta inteira, sem seguir links),
  com histórico de até 100 passos. Cada passo é simétrico: desfazer e refazer
  usam o mesmo `Action`.
- Lixeira (`platform.rs`): `SHFileOperationW` com `FOF_ALLOWUNDO`. A lixeira
  restaura o item, mas o desfazer do Forja **não** desfaz a exclusão.
- Nada é gravado fora da bancada, e a exclusão só ocorre por ação do usuário.

## Como testar

```sh
cargo test -p forja-module-explorer
```

Os testes usam pastas temporárias. Cobrem `.gitignore` (inclusive aninhado),
pastas antes de arquivos, descarte de carga antiga, operações com desfazer e
refazer, duplicação com nome livre, o watcher de verdade, e o fluxo completo
"bancada → listagem → arquivo novo pelo watcher" com um `bench.current` de
teste.

## Limitações conhecidas

- Desfazer não restaura itens da lixeira (use a lixeira do Windows).
- Symlinks só são listados quando `explorer.tree.follow_symlinks` está ligado, e
  nunca são copiados.
- As duas configurações estão no catálogo (`design/settings.toml` e `design/settings.md`,
  grupo Editor).
