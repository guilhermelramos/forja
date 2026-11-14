# forja-module-find

Módulo `find`: **Achar arquivo** (Ctrl+P). Indexa a bancada, faz busca fuzzy
com histórico de abertura, aceita `arquivo:linha` e contribui como fonte da Bigorna.

## O que registra

- Trigger e picker flutuante `find.trigger` no slot `top_bar`. O picker abre
  com Ctrl+P (`find.file`).
- Fonte da Bigorna `find.anvil`: recebe a consulta do shell em `anvil_hits` e
  devolve até 12 arquivos. A ação da linha é `file.open`.
- Comandos:
  - `find.file`: abre o picker;
  - `find.in_files` (Ctrl+Shift+F, id do catálogo): chama `search.focus`, do módulo
    `search`. Sem ele, não faz nada;
  - `find.record_open` (`path`, absoluto): registra uma abertura no histórico, se o
    arquivo estiver na bancada ativa. Outros módulos podem chamá-lo ao abrir um arquivo.
- Atalhos (`keymap.toml`): `find.file` = `Ctrl+P` e `find.in_files` = `Ctrl+Shift+F`,
  escopo `global`, com os ids de `design/keymap.toml`.
- Configurações do catálogo (`design/settings.toml`, módulo `find`):
  `find.results.limit` (50–1000, padrão 200), `find.search.respect_gitignore`
  (padrão sim), `find.search.include_hidden` (padrão não), `find.recent.count`
  (5–50, padrão 20), `find.grep.engine` (`auto`, `ripgrep`, `builtin`) e
  `find.grep.context` (0–10, padrão 2). O motor é usado pelo módulo `search`.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

Consome:

- `bench.activated` (benches, `BenchActivated`): troca a bancada e zera o índice.
  Este módulo declara e assina o evento (`contract_event` + `subscribe`).
- `bench.current` (benches): lido uma vez ao iniciar e de novo enquanto não há
  bancada.
- `file.open` (editor) com `{path, line?}`, quando nenhum módulo assina
  `file.open_requested`. Sem o editor, abre com o app padrão do Windows.
- `search.focus` (search), no comando `find.in_files`.

Publica:

- `file.open_requested` (`FileOpenRequested`, `contract_event`): emitido ao abrir um
  resultado, se algum módulo assina; senão, o fallback acima (ver `explorer`).
- Comandos `find.file`, `find.in_files` e `find.record_open` (em `design/contracts.md`).

## Como funciona

- Índice (`index.rs`): `git ls-files` com `--cached --others --exclude-standard`
  (tracked e não ignorados). Fora de um repositório, cai para uma caminhada com o
  crate `ignore`. Dotfiles e `.git` ficam de fora, a não ser que o setting
  `find.search.include_hidden` esteja ligado. A lista é limitada a 50 mil arquivos,
  e o picker avisa quando está parcial.
- Git roda com `GIT_TERMINAL_PROMPT=0`, entrada nula e sem janela de console.
- Ranking (`fuzzy.rs`): cada caractere da consulta casa em ordem e sem diferenciar
  caixa. Bônus para início de palavra, sequências e nome do arquivo. Pontuação
  menor para caminhos longos. O histórico soma recência (1 h, 1 dia, 7 dias) e uso.
  Consulta vazia lista os recentes e depois o início do índice.
- Ranking roda em tarefa do kernel, e só a resposta mais nova é usada.
- Histórico (`history.rs`): por bancada, em `%APPDATA%\Forja\modules\find.json`.
  É lido e gravado em tarefas, com gravação atômica.
- O picker desenha a própria janela, controlada por um flag de abertura, porque o
  slot `modal` do shell fica sempre aberto. A camada de teclado prende o Tab à
  janela, e Escape a fecha. Texto digitado com o foco em qualquer controle dela
  (inclusive o botão de fechar) volta para o campo de busca antes de o quadro
  desenhar, então a primeira letra não se perde.

## Como testar

```sh
cargo test -p forja-module-find
```

Cobrem o ranking fuzzy e o histórico, `arquivo:linha`, a indexação com e sem git
(e o limite), o histórico em JSON, e um fluxo completo com um `bench.current` de
teste: abre o picker, indexa em tarefa, registra uma abertura, e confere a
ordem dos recentes e de uma consulta.

## Limitações conhecidas

- O atalho Ctrl+P depende do shell repassar o atalho quando o terminal está com o
  foco. Isso não foi verificado aqui.
- `find.grep.engine` = `ripgrep` não existe: o motor é sempre o interno.
- Sem o módulo `bench`, o picker mostra "Abra uma bancada para achar os arquivos dela".
