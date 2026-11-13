# forja-module-editor

Módulo `editor` da Forja. Dono do namespace de contrato `file` e da aba de
arquivos da bancada.

## O que registra

- namespace de contrato `file` (comando e RPC `file.open`);
- aba `editor.tab` na bancada (ordem 15; `FORJA_BENCH_TAB=editor` a traz para a
  primeira posição em screenshots);
- comandos próprios: `editor.active_file`, `editor.save`, `editor.reload`, `editor.compare`,
  `editor.preview`, `editor.format`, `editor.open_external`, `editor.close`,
  `editor.find`, `editor.replace`, `editor.replace.all`, `editor.find.next`,
  `editor.find.previous`, `editor.goto.line`, `editor.wrap.toggle`,
  `editor.indent`, `editor.outdent`, `editor.move_line.up`,
  `editor.move_line.down`, `editor.duplicate_line`, `editor.delete_line`,
  `editor.comment.toggle`, `editor.check_external`, `editor.undo`,
  `editor.redo`, `editor.cut`, `editor.copy`, `editor.paste`,
  `editor.select_all`, `editor.quick_fix`, `editor.quick_fix.apply`,
  `editor.rename`, `editor.goto.definition`, `editor.peek.definition`,
  `editor.completion`, `editor.completion.apply`, `editor.parameter_hint`;
- RPC `file.open`, `editor.save`, `editor.preview`;
- 25 configurações `editor.*` do catálogo (`design/settings.toml`);
- atalhos de `design/keymap.toml` no escopo `editor`, incluindo desfazer,
  recortar/copiar/colar, renomear e navegação de símbolos;
- bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Desenho

- Abas na bancada; clique simples (evento `file.open_requested`) abre uma aba de
  prévia reutilizável e editar fixa a aba. O comando `file.open` fixa.
- Destaque com `syntect` + `two-face`, incluindo a gramática PowerShell em
  `syntaxes/powershell.sublime-syntax`; o layout fica em cache por
  caminho/texto/quebra/tema/tamanho da fonte.
- Localizar/substituir na barra ou via comandos; a substituição é feita no
  buffer e nunca lê nem grava outros arquivos.
- Gravação atômica (`forja_kernel::store::write_atomic_bytes`) preservando
  CRLF, BOM e UTF-16 LE; remover espaços finais e garantir newline final são
  opções do catálogo.
- Salvamento automático (`off`, `delay`, `focus_change`) e formatar ao salvar
  para Rust via `rustfmt`; o trabalho roda em `cx.spawn`.
- Banner de alteração externa (checagem de mtime em tarefa) com recarregar e
  comparar; o comparador usa `similar`.
- Prévia delega a `viewers.preview` e desenha o `ViewerDocument` genérico;
  `editor.markdown.preview` desliga a prévia de Markdown.
- Fonte do editor (`editor.font.family`) carrega Cascadia Mono, Consolas ou
  JetBrains Mono dos arquivos do Windows numa tarefa e instala via
  `egui::Context::add_font`; sem o arquivo, fica a monoespaçada do ui-kit.
- Layout visual do texto: altura de linha, realce da linha do cursor, minimapa,
  guias de indentação e espaços/tabulações visíveis são desenhados a partir do
  `galley` do egui, sem tocar no conteúdo.
- Desfazer/refazer guardam até 200 retratos por aba; recortar/copiar usam a
  seleção rastreada do `TextEdit` e o contexto egui; colar pede a colagem nativa.
- Renomear, ir/espiar definição, sugestões e dica de parâmetros usam analisadores
  leves em `symbols.rs` (sem servidor de linguagem e sem ler outros arquivos).

## Contratos

- Consome/atende `file.open_requested` (`FileOpenRequested`).
- Chama `find.record_open` quando existe.
- Chama `shell.tab.open {view_id: "editor.tab"}` quando existe, para abrir o
  arquivo já na aba do Editor.
- Chama `viewers.preview` e `viewers.open_external` quando existem.
- Degrada com aviso quando o outro módulo não está carregado.

## Como testar

```sh
cargo test -p forja-module-editor
cargo clippy -p forja-module-editor --all-targets -- -D warnings
```

Screenshots: `FORJA_BENCH_TAB=editor FORJA_EDITOR_FILE=<arquivo>` e, opcionalmente,
`FORJA_EDITOR_FIND=<termo>`.
