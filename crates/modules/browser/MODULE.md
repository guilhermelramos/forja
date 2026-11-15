# forja-module-browser

Aba **Navegador** da bancada: WebView2 embutido, abas, histórico, busca, zoom,
downloads confinados e Mermaid offline.

## O que registra

- Aba `browser.tab` em `Slot::BenchTab`.
- Comandos: `browser.open`, `browser.navigate`, `browser.mermaid.render`,
  `browser.open_external`, `browser.history`, `browser.history.clear`,
  `browser.address`, `browser.reload`, `browser.reload_hard`, `browser.find`,
  `browser.new_page`, `browser.reopen_page`, `browser.close_page`,
  `browser.zoom.in`, `browser.zoom.out`, `browser.zoom.reset`, `browser.back`,
  `browser.forward`.
- Métodos RPC: `browser.open` e `browser.history`.
- As onze configurações `browser.*` do catálogo (`engine.kind`, `home.url`,
  `search.engine`, privacidade, devtools, downloads, zoom, abas e login).
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl` e o `keymap.toml` com os
  atalhos de navegador do catálogo.

## Contratos

- Cada aba guarda URL, título, tipo (URL ou HTML), zoom e estado de carga; no
  máximo quatro abas vivas. Fechar uma aba a coloca na pilha de reabertura.
- O WebView é filho da janela da Forja. O pai é encontrado por processo e maior
  janela visível, não por título. `WebView2Loader.dll` é copiada para ao lado do
  executável pelo `build.rs` e pré-carregada com `SetDllDirectoryW` +
  `LoadLibraryW`.
- A escala de rasterização do controlador é fixada em 1.0 e o zoom da página
  aplica o DPI da janela; assim a página usa a largura lógica disponível sem
  recortar o conteúdo em monitores a 125% ou 150%.
- A página nativa some quando a aba não está ativa, a janela perde foco, é
  minimizada, há overlay/popup aberto ou o histórico está aberto. O egui não
  pinta por cima de filho nativo, então ocultar é a regra de visibilidade.
  O WebView2 só é criado quando a página está visível; criar o controle é
  trabalho de processo/COM que não pode atrasar um quadro invisível (Refs #73).
- A página inicial padrão (`forja://inicio`, `home.rs` e `home_data.rs`) é
  renderizada em egui sem criar WebView2 e sem requisições à rede externa:
  lista servidores locais HTTP das bancadas via contrato `ports.list` (com o
  mesmo filtro por bancada e busca do painel Portas), links para `README*` e
  `docs/` do projeto ou bancada ativa (abertos no Visualizador via
  `viewers.open` + `shell.tab.open`), e atalhos recentes do histórico local.
- Downloads usam `browser.download.dir`; o nome vem do servidor ou da URL e é
  reduzido a um nome simples (sem pastas, drives, ADS ou nomes de dispositivo).
- O histórico fica em `<data>/browser-history.json`, com no máximo 500 entradas
  e reordenação ao revisitar. As abas de URL ficam em
  `<data>/browser-tabs.json` e são reabertas quando `browser.tabs.restore` está
  ligado.
- Perfis do WebView2 ficam em `<data>/webview/<slug>-<hash>`; o escopo vem de
  `FORJA_BROWSER_PROFILE_SCOPE` (`shared`, `repository`, `workspace`) e
  `FORJA_BROWSER_WORKSPACE`.
- Mermaid 11.4.1 está embarcado em `assets/mermaid` (MIT, `LICENSE` e
  `VERSION`) e é servido pelo protocolo `forja://`; nenhum CDN é usado.
- `FORJA_BROWSER_URL` abre uma página na inicialização e
  `FORJA_BROWSER_FORCE_VISIBLE=1` mantém a página visível para captura de tela.
- `crates/modules/browser/build.rs` copia o loader; os assets do Mermaid estão
  na lista de exceções de proveniência com a licença ao lado.

## Como testar

```sh
cargo test -p forja-module-browser
cargo run -p forja-app
```

Os testes cobrem normalização de URL e busca, teto do histórico, perfis,
confinamento do nome de download, regra de visibilidade, extração/renderização
do Mermaid, contrato do kernel, limite de quatro abas e os comandos de abrir e
renderizar. A captura usa `shots/capture-browser-composite.ps1`: `PrintWindow`
da janela e da página nativa, compostos porque o egui não captura filhos.

## Lote 3 (cobertura do catálogo)

- `browser.download.ask_each`: abre a caixa de salvar a cada download.
- `browser.auth.open_external` (`auth.rs`): fluxos de entrada (heurística de host e
  caminho) abrem no navegador do sistema. Nenhum cookie é lido ou copiado.
- `browser.privacy.do_not_track`: define `navigator.doNotTrack`. O cabeçalho de
  requisição não é configurável pelo wry; só páginas que leem a propriedade veem o sinal.
- Adiada no catálogo: `browser.privacy.third_party_cookies` (exige a API de perfil do WebView2).
