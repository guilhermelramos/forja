# forja-module-viewers

Módulo `viewers` da Forja. Visualização somente leitura por extensão.

## O que registra

- aba `viewers.tab` (ordem 34; `FORJA_BENCH_TAB=viewers` a traz para a primeira
  posição em screenshots);
- comandos `viewers.list`, `viewers.preview`, `viewers.open`,
  `viewers.open_external` e `viewers.close`;
- RPC `viewers.list` e `viewers.preview`;
- assinatura de `file.open_requested`;
- bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Desenho

- Registro por extensão: Markdown, CSV/TSV, `.ipynb`, imagens (incluindo GIF) e
  PDF.
- Markdown com `pulldown-cmark` (GFM, callouts `> [!NOTE]` etc.) e sumário;
  blocos de código com botão que chama `browser.mermaid.render` quando existir.
- Tabelas paginadas, notebook somente leitura com células e saídas, imagens com
  zoom e GIF animado, PDF com contagem de páginas e abertura no app padrão.
- O modelo do documento fica em `crates/contracts/src/viewers.rs`; o editor usa
  `viewers.preview` para desenhar a prévia.
- Imagens e GIFs são lidos e decodificados em `cx.spawn`; a UI só cria texturas.

## Contratos

- Publica o modelo `ViewerDocument`/`ViewerDescriptor` descrito em
  `design/contracts.md`.
- Consome `file.open_requested` para acompanhar prévias abertas por links,
  busca ou diferenças.

## Como testar

```sh
cargo test -p forja-module-viewers
cargo clippy -p forja-module-viewers --all-targets -- -D warnings
```

Screenshots: `FORJA_BENCH_TAB=viewers FORJA_VIEWER=<arquivo>`.
