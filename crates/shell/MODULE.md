# forja-app

Shell do produto. Abre a janela sem bordas com os tokens Brasa, monta as regiões
fixas a partir dos slots e liga o RPC por named pipe.

## O que faz

- carrega os módulos linkados (`linkme` + fallback gerado pelo `build.rs`);
- aplica `forja_ui_kit::install_fonts` e `apply`, com tema escuro, claro ou o do Windows;
- densidade confortável/compacta e zoom por `appearance.ui.scale`;
- monta barra superior (views `top_bar`, ou "Achar arquivo" quando o slot está vazio),
  Bigorna, um botão de ícone para o tema e a janela; trilho com o logo e os
  glifos Segoe Fluent Icons das páginas; o clique num ícone troca a bancada pela
  página do módulo até o logo ou o mesmo ícone; lateral e painel direito viram
  abas quando mais de um módulo contribui, e a escolha fica na sessão; barra de status;
- o crate `hello` fica como molde do `xtask`, fora da região `# forja:modules`;
- modais só aparecem com `Ctx::open_modal`; `poll` pode abri-los sozinho;
- Bigorna com busca fuzzy, grupos, histórico, modo comando, atalhos visíveis
  e linhas de `View::anvil_hits`;
- gera Ajustes a partir do schema de cada módulo;
- executa os atalhos do keymap; Ctrl+K, Ctrl+Shift+P, Ctrl+P, Ctrl+Vírgula,
  Ctrl+Shift+B, setas entre abas e Ctrl+1..0 funcionam com o texto focado;
- abas da bancada: próxima/anterior, fechar (Ctrl+W e Ctrl+Shift+W em qualquer
  superfície), reabrir a última fechada e saltar para a aba 1..10; com o
  navegador ativo, Ctrl+W, Ctrl+Shift+T e Ctrl+0 ficam com a página;
- `settings.keymap` abre os Ajustes na seção de atalhos e `help.keymap` abre a
  janela com todos os atalhos efetivos;
- esconde a janela na bandeja quando `app.tray.enabled` está ligado;
- grava o ícone da janela e o recurso 1 do `.exe` a partir de `design/assets/logo.ico`;
- define o AppUserModelID `Forja.App` antes da janela;
- serve `meta.methods` e os métodos dos módulos no pipe;
- expõe `shell.rail.open {view_id}`, que enfileira a ativação de uma página do
  trilho; a janela valida o id e troca a bancada pela página (id desconhecido
  vira erro na barra de status). `muster.open` e a Bigorna usam esse contrato;
- entrega resultados de `Ctx::spawn` na thread da interface.

## Módulos do shell

O prefixo de uma configuração precisa ser um módulo registrado. O shell registra
âncoras `appearance`, `app`, `i18n`, `notifications`, `anvil`, `find`, `settings`,
`sidebar`, `panel`, `view`, `window`, `tab` e `help`. O módulo `notify` grava as
chaves `notifications.*`.

## Arquivos

| Arquivo | Papel |
|---|---|
| `src/main.rs` | flags, ícone, AppUserModelID, runtime |
| `src/app.rs` | estado, atalhos, bandeja, screenshot |
| `src/appearance.rs` | tema, densidade, zoom e idioma |
| `src/layout.rs` | regiões do produto |
| `src/anvil.rs` | Bigorna |
| `src/overlays.rs` | Ajustes gerados |
| `src/theme.rs` | tokens a partir do modo e da densidade |
| `src/system.rs` | tema e idioma do Windows |
| `src/hosts.rs` | módulos âncora e comandos do chrome |
| `src/tray.rs` | bandeja do Windows |
| `src/icon.rs` | PNG da janela e bytes do `.ico` |
| `src/actions.rs` | fila entre os comandos e a janela |
| `src/rpc.rs` | host do pipe |
| `src/screenshot.rs` | PNG e recorte por slot |
| `src/shell_module.rs` | i18n do chrome |
| `build.rs` | região `# forja:modules` e `windres` do ícone |

## Flags de screenshot

`--theme dark|light|system`, `--density comfortable|compact`, `--open anvil|onboarding|toast|rail`
e `--anvil-query <texto>` valem só naquela janela. Screenshot define `FORJA_SCREENSHOT=1`
e usa um diretório temporário, sem tocar em `%APPDATA%\Forja`.

## Como testar

```sh
cargo test -p forja-app
cargo run -p forja-app -- --screenshot shots/empty-dark.png --no-modules --theme dark
cargo run -p forja-app -- --screenshot shots/anvil.png --open anvil --anvil-query greet
```

O teste de contrato roda sobre os módulos ligados e as âncoras do shell.
