# forja-module-search

Módulo `search`: o painel direito **Busca** (Ctrl+Shift+F, via `find.in_files`).
Busca nos arquivos da bancada em thread, com resultados em fluxo agrupados por
arquivo. Substitui com prévia, confirmação e gravação atômica.

## O que registra

- Painel `search.panel` no slot `right_panel` (ordem 20).
- Comandos:
  - `search.focus`: leva o foco ao campo de busca (usado por `find.in_files`);
  - `search.run` (`pattern`, `regex`, `case_sensitive`, `whole_word`, `include`,
    `exclude`, todos opcionais): define o formulário e busca.
- Sem configurações próprias: usa as do módulo `find` (`find.grep.context`,
  `find.search.include_hidden`, `find.search.respect_gitignore`).
- Sem atalhos próprios: Ctrl+Shift+F é `find.in_files`, do módulo `find`.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

Consome:

- `bench.activated` (benches, `BenchActivated`): troca a bancada, para a busca em
  andamento e limpa os resultados e a prévia. Este módulo declara e assina o evento.
- `bench.current` (benches): lido uma vez ao iniciar e de novo enquanto não há bancada.
- `file.open` (editor) com `{path, line}`, quando nenhum módulo assina
  `file.open_requested`. Sem o editor, abre com o app padrão do Windows.
- `find.grep.context`, `find.search.include_hidden`, `find.search.respect_gitignore`
  (módulo `find`).

Publica:

- `file.open_requested` (`FileOpenRequested`, `contract_event`) ao clicar em uma linha,
  se algum módulo assina; senão, o fallback acima (ver `explorer`).
- Comandos `search.focus` e `search.run` (em `design/contracts.md`).

## Como funciona

- Busca (`engine.rs`): `ignore::WalkBuilder` com o `.gitignore`, os dotfiles e os
  globs (`include`/`exclude`, regras de `override`) de `query.rs`. Lê cada arquivo
  inteiro e pula binários (NUL nos primeiros 8 KiB), não UTF-8 e arquivos acima de
  2 MiB. Cada linha que casa leva `find.grep.context` linhas de contexto.
- A caminhada roda numa thread própria (é longa e manda muitos lotes). O painel
  recebe cada lote em tarefas do kernel, e cada lote atualiza a tela.
- Cancelamento: uma nova busca ou o botão "Parar" ativa uma flag, e a caminhada para
  entre arquivos. Resultados de uma busca antiga são descartados por geração.
- Limite de 2000 correspondências; ao atingir, o painel avisa que foi truncado.
  Até 400 resultados são desenhados por quadro.
- Substituição (`replace.rs`):
  - **Prévia:** tarefa que lê os arquivos do disco, calcula o novo texto e guarda o
    SHA-256 do original. Recusa mais de 500 arquivos ou 64 MiB: restrinja com globs.
  - **Aplicar:** a primeira ação só pede confirmação. A segunda relê **todos** os
    arquivos, compara o SHA-256, e se algum mudou não grava nada e lista os
    arquivos. Depois grava cada um com `write_atomic_bytes` (temporário e
    renomeação) e rechecando antes de cada gravação. Se uma gravação falhar, os
    arquivos já gravados voltam ao texto da prévia.
  - Regex usa os grupos (`$1`). Modo literal não interpreta `$`.
  - Ao terminar, a busca roda de novo para refletir o disco.

## Como testar

```sh
cargo test -p forja-module-search
```

Cobrem a compilação das opções (literal, maiúsculas, palavra inteira, regex com
erro), os globs de incluir e excluir, o motor (`.gitignore`, binários, cancelamento,
contexto nas bordas e linhas longas), a substituição (CRLF preservado, conflito
com a prévia sem gravar nada, `$1` literal e regex, binários e não casados) e um
fluxo completo com um `bench.current` de teste: busca em fluxo, prévia, confirmação
e gravação.

## Limitações conhecidas

- `find.grep.engine` = `ripgrep` não é implementado: sempre o motor interno.
- A busca é por linha. Uma regex que atravessa linhas é aplicada na prévia e na
  gravação, mas não aparece como resultado da busca.
- Quando a mudança atravessa linhas, a prévia mostra só o aviso, sem os trechos.
- Os resultados não são desenhados além de 400 por quadro.
- O painel fica empilhado com "Arquivos" no painel direito. O shell ainda não tem abas.

## Lote 3 (cobertura do catálogo)

- `find.grep.engine` (`rg.rs`): `auto` lista os arquivos com `rg --files-with-matches`
  quando `rg` está no PATH e usa o caminhante interno caso contrário. As linhas e o
  contexto são sempre calculados aqui, então os dois motores mostram os mesmos acertos.
