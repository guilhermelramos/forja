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
- A Bigorna oferece a escolha de página para `shell.rail.open` e pede título para
  `muster.run`; os comandos `appearance.theme.light` e `appearance.theme.dark`
  alteram o modo. O menu de abas permite alcançar contribuições fora da faixa visível.
- Comandos que declaram `CommandField` (bancada, projeto, etapa, nome) pedem o
  valor na própria Bigorna: lista filtrável lida de outro comando ou texto no
  campo de busca. O mecanismo é genérico (`anvil_ask.rs`); o módulo só diz qual
  comando devolve as opções e quais campos ler.
- Ajustes ocupa a área central, com engrenagem selecionada e retorno por Escape ou
  pela engrenagem à página anterior. Os 13 grupos vêm de `design/settings.toml`,
  com seções próprias abaixo do separador; a busca cruza rótulos, ajuda e chaves.
  Linhas mostram ajuda, alteração/restauração, reinício e validação no local;
  Aparência tem prévia viva. A busca recebe foco ao abrir, PageUp/PageDown e a
  roda rolam a lista e Tab revela o controle focado. O rodapé importa/exporta
  TOML em Tasks, restaura com confirmação e desativa abrir arquivo nos testes;
  o escopo continua usuário até #212;
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
| `src/anvil_ask.rs` | valores que faltam, perguntados na Bigorna |
| `src/overlays.rs` | janela de atalhos e helpers de texto/rolagem |
| `src/settings_page.rs` | página e navegação de Ajustes |
| `src/settings_catalog.rs` | grupos, ordem e avisos do catálogo embutido |
| `src/settings_controls.rs` | controles gerados e validação por linha |
| `src/settings_transfer.rs` | seletor interno e importação/exportação assíncronas |
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
