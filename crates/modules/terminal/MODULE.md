# forja-module-terminal

Terminais completos dentro das bancadas: ConPTY com Job Object, emulação VT,
painéis e abas, seleção, rolagem, busca, links, temas próprios, receitas e o
RPC/CLI do contrato.

## Arquivos

| Caminho | Papel |
|---|---|
| `src/pty/` | ConPTY, pipes, Job Object, ambiente e encerramento da árvore |
| `src/vt/` | grade (`grid`), superfície (`surface`), parser (`parser`), SGR/modos (`sgr`) e imagens (`image`) |
| `src/terminal.rs` | ciclo de vida de um terminal, seleção, busca, rolagem e snapshot |
| `src/selection.rs` | seleção linear e intervalos por linha |
| `src/panes/tree.rs` | árvore de painéis e abas |
| `src/core/` | hub compartilhado, operações, conteúdo, persistência e utilidades |
| `src/view/` | aba de bancada, chrome, corpo do terminal, teclado e renderização |
| `src/links.rs`, `src/search.rs` | links OSC 8/arquivo e busca no histórico |
| `src/recipes.rs` | `recipes/*.toml` das três camadas |
| `src/project.rs` | detecção do tipo do projeto da bancada e ordenação das receitas |
| `src/view/recipe_picker.rs` | seletor de receitas com filtro, grupos e prévia |
| `src/shells.rs` | detecção de shell, Git Bash, WSL, ambiente e abertura externa |
| `src/theme.rs` | paletas de `content/themes/*.toml` |
| `src/settings.rs`, `src/settings_registry.rs` | retrato e esquema das 24 configurações |
| `src/commands.rs`, `src/rpc.rs`, `src/requests.rs` | comandos, métodos RPC/CLI e argumentos |

Nenhum arquivo passa de 600 linhas.

## O que registra

### Aba da bancada

- View `terminal.tab` no slot `bench_tab` (ordem 10): painéis com abas,
  divisores arrastáveis, título dinâmico (OSC 0/2) e título personalizado.
  Cada bancada mantém no máximo 8 painéis visíveis (`MAX_PANES`); um layout
  antigo maior é podado ao restaurar. O limite evita quadros pesados demais
  no renderizador por software usado nos testes (Refs #70, #71, #72, #74).
- View `terminal.recipes` no slot `anvil_source`: lista as receitas carregadas
  como resultados da Bigorna; `terminal.recipe.run` confere o retrato escolhido,
  exige a aprovação existente para receitas de projeto e executa no terminal focado.

### Comandos

Públicos (contrato): `terminal.open`, `terminal.send`, `terminal.read`,
`terminal.list`.

Ações do keymap: `terminal.copy`, `terminal.paste`, `terminal.search`,
`terminal.clear`, `terminal.split.right`, `terminal.split.down`,
`terminal.pane.next`, `terminal.pane.previous`, `terminal.pane.close`,
`terminal.scroll.page_up`, `terminal.scroll.page_down`, `terminal.scroll.top`,
`terminal.scroll.bottom`, `terminal.toggle`.

Ações de interface sem atalho no catálogo: `terminal.new_tab`,
`terminal.tab.close` e `terminal.focus`. Elas existem para o menu, a Bigorna e a
página Ajudantes, sem inventar acordes fora de `design/keymap.toml`.

### RPC/CLI

| Método | Efeito | Parâmetros | Resultado |
|---|---|---|---|
| `terminal.open` | write | `cwd`, `command?`, `args?`, `env?`, `title?`, `shell?` | `{terminal_id}` |
| `terminal.send` | write | `terminal_id`, `text`, `enter?` | `{}` |
| `terminal.read` | read | `terminal_id`, `lines?` | `{text}` (+ `alive`, `error`, `dir`) |
| `terminal.list` | read | `bench_path?` | `{terminals: [TerminalRef]}` |

`terminal.read` acrescenta `alive`, `error` e `dir` ao resultado, campos
aditivos que ajudam o CLI a diagnosticar uma sessão. `terminal.open` aceita
`shell` como `pwsh`, `powershell`, `cmd`, `git-bash`, `wsl` ou `wsl:<distro>`.

Exemplo:

```sh
forja terminal open --cwd .
forja terminal send --terminal_id 1 --text "echo forja" --enter
forja terminal read --terminal_id 1 --lines 20
forja terminal list
```

### Eventos

Declarados com `r.contract_event` (publicador e assinantes podem registrar em
qualquer ordem):

- `terminal.exited` → `TerminalExited`;
- `terminal.bell` → `TerminalBell`;
- `terminal.command_finished` → `TerminalCommandFinished`;
- `terminal.title_changed` → `TerminalTitleChanged`;
- `file.open_requested` → `FileOpenRequested`, emitido no clique com Ctrl num
  link de arquivo.

O módulo também tenta `file.open` quando o módulo de edição existe e
`browser.open` para links quando `terminal.links.open_with = app`. Sem eles,
degrada para o programa padrão do sistema.

### Configurações

As 24 chaves de `design/settings.toml` (grupo Terminal), com os mesmos
padrões, faixas e opções:

`terminal.shell.{default,path,start_dir,env}`,
`terminal.scroll.{history_lines,wheel_lines,follow_output}`,
`terminal.cursor.style`, `terminal.font.{family,size}`,
`terminal.theme.{name,follow_app}`, `terminal.color.opacity`,
`terminal.bell.mode`, `terminal.copy.{on_select,trim}`,
`terminal.paste.{warn_multiline,bracketed}`,
`terminal.links.{confirm,open_with}`, `terminal.shell.marks`,
`terminal.close.confirm_running`, `terminal.gpu.mode`,
`terminal.osc52.enabled`.

Efeitos reais: histórico e rolagem, acompanhamento da saída, cursor, fonte,
paleta e opacidade, campainha visual/sonora, cópia e colagem, confirmação de
links, marcas OSC 133, confirmação ao fechar processo, imagens embutidas
(`gpu.mode = off` não desenha as imagens) e OSC 52.

### Atalhos

`keymap.toml` usa os ids do escopo `terminal` de `design/keymap.toml`:
copiar, colar, buscar, limpar, dividir à direita/abaixo, painel anterior/próximo,
fechar painel e rolagem (PageUp/PageDown, Ctrl+Home/End). O `terminal.toggle`
é global (`Ctrl+Backtick`).

## Receitas

Lê `[[recipe]]` de `recipes/*.toml` nestas camadas, da menor para a maior
precedência: embutida (`content/recipes`), usuária (`~/.forja/recipes`) e do
projeto (`<bancada>/.forja/recipes`). O `id` repetido é substituído pela camada
superior. Campos: `id`, `title` (texto ou tabela `pt-BR`/`en`), `command`,
`scope`, `enter`, `directory` e `manifest`.

Abrir `Receitas` na barra do terminal mostra um seletor com filtro por texto,
navegação por setas e Enter. O módulo detecta o tipo da bancada pelos
manifestos (`Cargo.toml`, `go.mod`, `package.json`, `pyproject.toml`,
`requirements.txt` e equivalentes) e pela extensão dos arquivos da raiz e de
`src/`. As receitas que se aplicam ficam primeiro, agrupadas por linguagem; as
demais vão para a seção recolhida `Outras`. Cada linha mostra o comando e a
prévia exibe o comando exato e a pasta onde ele roda. A aprovação de confiança
continua valendo para receitas de projeto.

O módulo oferece a lista no menu Receitas do terminal e na Bigorna. A receita
roda no terminal focado; quando `directory` aponta para outra pasta da bancada,
abre um terminal nela. Receitas de projeto passam por `trust.check` antes de
executar; sem aprovação do hash exato, `trust.request` abre o modal e a receita
espera a decisão. Receitas da camada do projeto usam a origem can�nica no modal e s� s�o carregadas quando o destino resolvido permanece dentro da bancada; links externos s�o recusados. Receitas globais continuam executando direto.

## Shells

`auto` escolhe `pwsh.exe` quando existe e cai para `powershell.exe`, depois
`cmd.exe`. `custom` usa `terminal.shell.path`. Git Bash é localizado em
`%ProgramFiles%\Git\bin\bash.exe` e WSL em `wsl.exe`, com distros descobertas
em segundo plano por `wsl.exe -l -q`; o menu oferece cada distro.

## Temas

As cinco paletas embutidas são lidas de `content/themes/*.toml` com
`forja_ui_kit::TerminalTheme`: Brasa Escuro, Brasa Claro, Carvão, Cinza Quente
e Aço. `terminal.theme.follow_app` escolhe a paleta clara/escura pelo modo da
interface; desligado, usa `terminal.theme.name`. Temas extras em
`~/.forja/themes` e `<bancada>/.forja/themes` são carregados pelo mesmo id.

## Persistência

O layout (árvore de painéis, abas, ids e texto limitado a 256 KiB por terminal)
é gravado em `%APPDATA%\Forja\modules\terminal.json` em segundo plano, no
máximo a cada 10 s ou quando a estrutura muda. Na primeira abertura da aba, a
leitura também acontece em segundo plano; se o usuário já abriu um terminal
enquanto isso, o snapshot é descartado.

## Capturas de tela

O harness de `--screenshot` não interage com a janela. Para as evidências, a
view aceita `FORJA_TERMINAL_PREVIEW=demo|split|search`, que semeia um quadro
congelado (sem processos reais) e permite capturar tema escuro/claro,
divisões e busca. Sem a variável, o comportamento é o normal.

## Contratos e ausências

- Publica eventos de contrato declarados com `contract_event`; não importa
  crates de outros módulos.
- Tenta `bench.current` uma vez para descobrir a bancada e degrada em silêncio
  quando o módulo de bancadas não está presente.
- `terminal.toggle` cria/foca o terminal quando ele não existe. O shell, dono da
  aba da bancada, mostra a aba ao abrir/focar o terminal e fecha a aba quando o
  atalho chega com o terminal ativo; assim `Ctrl+Backtick` alterna a
  visibilidade sem o módulo conhecer o layout global.

## Como testar

```sh
cargo test -p forja-module-terminal
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask provenance
```

Os testes incluem parser VT, seleção, busca, links, temas, árvore de painéis,
persistência, contrato do kernel, RPC `open`/`send`/`read`, ConPTY real com
PowerShell (abre, envia `echo`, lê e fecha) e encerramento de processo neto pelo
Job Object.


A confirmação de fechamento considera `helper.close.confirm_running` para terminais com ajudantes ativos, independentemente da confirmação genérica.

## Lote 3 (receitas)

- `terminal.recipes.press_enter`, `terminal.recipes.confirm`,
  `terminal.recipes.show_scope` e `terminal.recipes.shell` (antes `recipe.*`): Enter após
  colar, confirmação antes de rodar (`hub_ui.rs`), escopo do seletor e shell que roda a
  receita numa aba nova (`core/hub.rs`).

## Protocolos de redesenho

O modo 2026 publica uma tela completa ao desligar, com limite fixo de 150 ms e resposta DECRQM; consultas e sinais continuam sendo processados durante a acumulação. O modo 1004 reporta transições de foco do painel e da janela por um callback ao terminar o quadro, inclusive ao esconder a aba. DECSCUSR aceita 0–6; só formas explicitamente piscantes e focadas agendam a próxima troca. SGR 9/29 e 53/55 desenham tachado e sobrelinha; 4:3 usa sublinhado simples, sem ligar itálico.

Testes: `cargo test -p forja-module-terminal`; roteiro com ferramentas reais para o dono: `playtest/terminal-manual.md`.
