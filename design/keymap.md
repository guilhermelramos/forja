# Forja: mapa de atalhos

> **Fonte de verdade dos atalhos.** Os ids, os padrões e as regras deste arquivo são decisões da Forja. A versão de máquina fica em `design/keymap.toml` e é a que o kernel lê como padrão.

## 1. Princípios

1. **Ctrl+K é a Bigorna, Ctrl+P acha arquivo e Ctrl+Shift+P é o modo comando da Bigorna.** A Bigorna é o caminho universal: todo comando existe nela, mesmo quando o atalho está ocupado pelo shell.
2. **Alt+número muda de bancada; Ctrl+número muda de aba.** As duas famílias são paralelas e previsíveis: Alt para o lugar onde o trabalho acontece, Ctrl para a aba dentro dele. `0` é sempre o décimo item.
3. **O terminal recebe todas as teclas, exceto as marcadas como globais mesmo no terminal.** Um shell precisa de Ctrl+C, Ctrl+D, Ctrl+R, Ctrl+W, Ctrl+U e afins. A Forja só toma uma tecla do terminal quando ela é uma âncora do produto e está listada na seção 4.
4. **As convenções universais do Windows ficam onde o usuário espera.** Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+S, Ctrl+Z, Ctrl+Y, Ctrl+A, Ctrl+F, Ctrl+W, Ctrl+Tab, F2, F3, F11, F12 e Alt+←/→ mantêm o significado de sempre na superfície em que fazem sentido.
5. **Toda ação tem comando e título; atalho é aceleração, não única porta.** Nenhuma ação existe só no teclado.
6. **Teclas do Windows ficam com o Windows.** A Forja não registra combinações com a tecla `Win`; reserva o espaço para o sistema e para o usuário.
7. **Nada de modo modal por padrão.** A Forja não tem Vim mode embutido; quem quiser usa uma extensão que declara o próprio escopo.

## 2. Escopos e precedência

| Escopo | Nome | Quando vale |
|---|---|---|
| `global` | Global / Global | Fora do terminal, vale em qualquer superfície. No terminal, só vale se estiver marcado como global mesmo no terminal. |
| `terminal` | Terminal / Terminal | Vale quando o terminal tem foco e vence o global, exceto pelos atalhos marcados como globais no terminal. |
| `editor` | Editor / Editor | Vale quando o editor ou a aba de diferenças tem foco e vence o global. |
| `browser` | Navegador / Browser | Vale quando a aba Navegador tem foco e vence o global. |
| `anvil` | Bigorna / Anvil | Vale enquanto a Bigorna está aberta e tem a maior precedência. |

A resolução é **Bigorna > superfície com foco > global**. A superfície com foco pode ser `terminal`, `editor` ou `browser`. Um atalho global marcado `even_in_terminal = true` vence o escopo `terminal`, mas nunca vence `anvil`, `editor` ou `browser`. Onde a Bigorna está aberta, `Escape` fecha, `Enter` executa e as setas navegam.

Ids seguem `<módulo>.<ação>` e podem ter um terceiro nível quando a ação é uma variante: `terminal.split.right`, `bench.switch_3`. Tudo em `snake_case`, nunca camelCase.

## 3. Atalhos por família

### Bigorna e busca / Anvil and search

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Abrir a Bigorna · Open the Anvil | `anvil.open` | `Ctrl+K` | `global` + terminal | Funciona mesmo com o foco no terminal. |
| Modo comando da Bigorna · Anvil command mode | `anvil.command_mode` | `Ctrl+Shift+P` | `global` + terminal | Abre a Bigorna já filtrando comandos. |
| Achar arquivo · Find file | `find.file` | `Ctrl+P` | `global` + terminal | Funciona mesmo com o foco no terminal. |
| Buscar nos arquivos · Search in files | `find.in_files` | `Ctrl+Shift+F` | `global` | No terminal, esta tecla não abre a busca global; use a Bigorna. |
| Fechar a Bigorna · Close the Anvil | `anvil.close` | `Esc` | `anvil` | — |
| Executar o item selecionado · Run selected item | `anvil.run` | `Enter` | `anvil` | — |
| Item anterior · Previous item | `anvil.previous` | `↑` | `anvil` | — |
| Próximo item · Next item | `anvil.next` | `↓` | `anvil` | — |
| Próximo grupo · Next group | `anvil.group.next` | `Tab` | `anvil` | — |
| Grupo anterior · Previous group | `anvil.group.previous` | `Shift+Tab` | `anvil` | — |
| Executar em segundo plano · Run in background | `anvil.run_background` | `Ctrl+Enter` | `anvil` | — |

### Bancadas / Benches

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Nova bancada · New bench | `bench.create` | `Ctrl+Shift+N` | `global` + terminal | — |
| Próxima bancada · Next bench | `bench.next` | `Alt+PageDown` | `global` + terminal | — |
| Bancada anterior · Previous bench | `bench.previous` | `Alt+PageUp` | `global` + terminal | — |
| Ir para a bancada 1 · Go to bench 1 | `bench.switch_1` | `Alt+1` | `global` + terminal | — |
| Ir para a bancada 2 · Go to bench 2 | `bench.switch_2` | `Alt+2` | `global` + terminal | — |
| Ir para a bancada 3 · Go to bench 3 | `bench.switch_3` | `Alt+3` | `global` + terminal | — |
| Ir para a bancada 4 · Go to bench 4 | `bench.switch_4` | `Alt+4` | `global` + terminal | — |
| Ir para a bancada 5 · Go to bench 5 | `bench.switch_5` | `Alt+5` | `global` + terminal | — |
| Ir para a bancada 6 · Go to bench 6 | `bench.switch_6` | `Alt+6` | `global` + terminal | — |
| Ir para a bancada 7 · Go to bench 7 | `bench.switch_7` | `Alt+7` | `global` + terminal | — |
| Ir para a bancada 8 · Go to bench 8 | `bench.switch_8` | `Alt+8` | `global` + terminal | — |
| Ir para a bancada 9 · Go to bench 9 | `bench.switch_9` | `Alt+9` | `global` + terminal | — |
| Ir para a bancada 10 · Go to bench 10 | `bench.switch_10` | `Alt+0` | `global` + terminal | — |

### Abas / Tabs

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Focar abas de conteúdo · Focus content tabs | `tab.focus` | `F6` | `global` + terminal | Sai do terminal ou editor (onde `Esc` também libera o foco) para navegar entre as abas com `Tab`/`Shift+Tab` e ativar com `Enter`/`Espaço`. |
| Próxima aba · Next tab | `tab.next` | `Ctrl+Tab` | `global` + terminal | — |
| Aba anterior · Previous tab | `tab.previous` | `Ctrl+Shift+Tab` | `global` + terminal | — |
| Fechar aba · Close tab | `tab.close` | `Ctrl+W` | `global` | No terminal, Ctrl+W vai para o shell; use Ctrl+Shift+W para fechar a aba. |
| Fechar aba em qualquer superfície · Close tab anywhere | `tab.close_force` | `Ctrl+Shift+W` | `global` + terminal | Funciona mesmo com o foco no terminal. |
| Reabrir aba fechada · Reopen closed tab | `tab.reopen` | `Ctrl+Shift+T` | `global` + terminal | — |
| Ir para a aba 1 · Go to tab 1 | `tab.switch_1` | `Ctrl+1` | `global` + terminal | — |
| Ir para a aba 2 · Go to tab 2 | `tab.switch_2` | `Ctrl+2` | `global` + terminal | — |
| Ir para a aba 3 · Go to tab 3 | `tab.switch_3` | `Ctrl+3` | `global` + terminal | — |
| Ir para a aba 4 · Go to tab 4 | `tab.switch_4` | `Ctrl+4` | `global` + terminal | — |
| Ir para a aba 5 · Go to tab 5 | `tab.switch_5` | `Ctrl+5` | `global` + terminal | — |
| Ir para a aba 6 · Go to tab 6 | `tab.switch_6` | `Ctrl+6` | `global` + terminal | — |
| Ir para a aba 7 · Go to tab 7 | `tab.switch_7` | `Ctrl+7` | `global` + terminal | — |
| Ir para a aba 8 · Go to tab 8 | `tab.switch_8` | `Ctrl+8` | `global` + terminal | — |
| Ir para a aba 9 · Go to tab 9 | `tab.switch_9` | `Ctrl+9` | `global` + terminal | — |
| Ir para a aba 10 · Go to tab 10 | `tab.switch_10` | `Ctrl+0` | `global` + terminal | — |

### Terminal / Terminal

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Mostrar ou ocultar terminal · Toggle terminal | `terminal.toggle` | `Ctrl+Backtick` | `global` + terminal | — |
| Copiar do terminal · Copy from terminal | `terminal.copy` | `Ctrl+Shift+C` | `terminal` | Ctrl+C continua sendo interrupção do processo. |
| Colar no terminal · Paste into terminal | `terminal.paste` | `Ctrl+V` | `terminal` | A colagem segura é a mesma de qualquer campo do Windows. |
| Buscar no terminal · Search terminal | `terminal.search` | `Ctrl+Shift+F` | `terminal` | No terminal, esta tecla vence a busca global. |
| Limpar terminal · Clear terminal | `terminal.clear` | `Ctrl+Shift+K` | `terminal` | — |
| Dividir à direita · Split right | `terminal.split.right` | `Alt+Shift+R` | `terminal` | R de right; D divide abaixo. |
| Dividir abaixo · Split down | `terminal.split.down` | `Alt+Shift+D` | `terminal` | — |
| Próximo painel · Next pane | `terminal.pane.next` | `Alt+Shift+PageDown` | `terminal` | PageDown sem Shift troca de bancada. |
| Painel anterior · Previous pane | `terminal.pane.previous` | `Alt+Shift+PageUp` | `terminal` | — |
| Fechar painel · Close pane | `terminal.pane.close` | `Alt+Shift+W` | `terminal` | — |
| Rolar uma página para cima · Scroll page up | `terminal.scroll.page_up` | `PageUp` | `terminal` | — |
| Rolar uma página para baixo · Scroll page down | `terminal.scroll.page_down` | `PageDown` | `terminal` | — |
| Ir ao início do histórico · Scroll to top | `terminal.scroll.top` | `Ctrl+Home` | `terminal` | — |
| Ir ao fim do histórico · Scroll to bottom | `terminal.scroll.bottom` | `Ctrl+End` | `terminal` | — |

### Editor / Editor

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Desfazer · Undo | `editor.undo` | `Ctrl+Z` | `editor` | — |
| Refazer · Redo | `editor.redo` | `Ctrl+Y` | `editor` | — |
| Recortar · Cut | `editor.cut` | `Ctrl+X` | `editor` | — |
| Copiar · Copy | `editor.copy` | `Ctrl+C` | `editor` | — |
| Colar · Paste | `editor.paste` | `Ctrl+V` | `editor` | — |
| Selecionar tudo · Select all | `editor.select_all` | `Ctrl+A` | `editor` | — |
| Salvar · Save | `editor.save` | `Ctrl+S` | `editor` | — |
| Buscar no arquivo · Find in file | `editor.find` | `Ctrl+F` | `editor` | — |
| Substituir no arquivo · Replace in file | `editor.replace` | `Ctrl+H` | `editor` | — |
| Próxima ocorrência · Find next | `editor.find.next` | `F3` | `editor` | — |
| Ocorrência anterior · Find previous | `editor.find.previous` | `Shift+F3` | `editor` | — |
| Ir para linha · Go to line | `editor.goto.line` | `Ctrl+G` | `editor` | — |
| Comentar ou descomentar · Toggle comment | `editor.comment.toggle` | `Ctrl+/` | `editor` | — |
| Correção rápida · Quick fix | `editor.quick_fix` | `Ctrl+.` | `editor` | — |
| Renomear símbolo · Rename symbol | `editor.rename` | `F2` | `editor` | — |
| Ir para definição · Go to definition | `editor.goto.definition` | `F12` | `editor` | No navegador, F12 abre as ferramentas de desenvolvedor. |
| Espiar definição · Peek definition | `editor.peek.definition` | `Alt+F12` | `editor` | — |
| Sugestões · Suggestions | `editor.completion` | `Ctrl+Space` | `editor` | — |
| Dica de parâmetros · Parameter hints | `editor.parameter_hint` | `Ctrl+Shift+Space` | `editor` | — |
| Mover linha para cima · Move line up | `editor.move_line.up` | `Alt+↑` | `editor` | — |
| Mover linha para baixo · Move line down | `editor.move_line.down` | `Alt+↓` | `editor` | — |
| Duplicar linha · Duplicate line | `editor.duplicate_line` | `Alt+Shift+↓` | `editor` | — |
| Apagar linha · Delete line | `editor.delete_line` | `Ctrl+Shift+K` | `editor` | No terminal, Ctrl+Shift+K limpa a tela. |
| Quebrar linha · Toggle word wrap | `editor.wrap.toggle` | `Alt+Z` | `editor` | — |
| Formatar arquivo · Format file | `editor.format` | `Ctrl+Alt+F` | `editor` | — |
| Indentar · Indent | `editor.indent` | `Tab` | `editor` | — |
| Remover indentação · Outdent | `editor.outdent` | `Shift+Tab` | `editor` | — |

### Navegador / Browser

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Barra de endereço · Address bar | `browser.address` | `Ctrl+L` | `browser` | — |
| Recarregar página · Reload page | `browser.reload` | `Ctrl+R` | `browser` | — |
| Recarregar sem cache · Hard reload | `browser.reload_hard` | `Ctrl+Shift+R` | `browser` | — |
| Buscar na página · Find in page | `browser.find` | `Ctrl+F` | `browser` | — |
| Nova página · New page | `browser.new_page` | `Ctrl+T` | `browser` | — |
| Reabrir página fechada · Reopen closed page | `browser.reopen_page` | `Ctrl+Shift+T` | `browser` | Sobrepõe reabrir aba enquanto o navegador tem foco. |
| Fechar página · Close page | `browser.close_page` | `Ctrl+W` | `browser` | Sobrepõe o fechamento de aba enquanto o navegador tem foco. |
| Aumentar página · Zoom page in | `browser.zoom.in` | `Ctrl+=` | `browser` | — |
| Reduzir página · Zoom page out | `browser.zoom.out` | `Ctrl+-` | `browser` | — |
| Restaurar página a 100% · Reset page zoom | `browser.zoom.reset` | `Ctrl+0` | `browser` | Sobrepõe a aba 10 enquanto o navegador tem foco. |
| Voltar · Back | `browser.back` | `Alt+←` | `browser` | — |
| Avançar · Forward | `browser.forward` | `Alt+→` | `browser` | — |
| Ferramentas de desenvolvedor · Developer tools | `browser.devtools` | `F12` | `browser` | Sobrepõe 'ir para definição' quando o navegador tem foco. |
| Abrir no navegador do sistema · Open in system browser | `browser.open_external` | `Alt+Enter` | `browser` | — |

### Diferenças / Diff

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Abrir Diferenças · Open Diff | `diff.open` | `Ctrl+Shift+D` | `global` + terminal | — |

### Ajudantes e Mutirão / Helpers and Muster

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Iniciar ajudante padrão · Launch default helper | `helper.launch.default` | `Ctrl+Shift+A` | `global` + terminal | — |
| Iniciar ajudante de um perfil · Launch helper from profile | `helper.launch.profile` | `Ctrl+Alt+Shift+A` | `global` + terminal | Abre o seletor de perfis antes de iniciar. |
| Ir para o ajudante da bancada · Focus bench helper | `helper.focus` | `Ctrl+Shift+H` | `global` + terminal | — |
| Abrir Mutirão · Open Muster | `muster.open` | `Ctrl+Shift+M` | `global` + terminal | — |

### Sistema, janela e painéis / System, window and panels

| Ação (pt-BR · en) | id | Atalho | Escopo | Observação |
|---|---|---|---|---|
| Mostrar ou ocultar lateral · Toggle sidebar | `sidebar.toggle` | `Ctrl+B` | `global` | No terminal, Ctrl+B continua indo para o shell. |
| Mostrar ou ocultar painel direito · Toggle right panel | `panel.toggle` | `Ctrl+Shift+B` | `global` + terminal | — |
| Abrir Ajustes · Open Settings | `settings.open` | `Ctrl+,` | `global` + terminal | — |
| Abrir atalhos · Open keymap | `settings.keymap` | `Ctrl+Shift+,` | `global` + terminal | — |
| Ver todos os atalhos · Show all shortcuts | `help.keymap` | `Ctrl+Shift+/` | `global` + terminal | — |
| Tela cheia · Full screen | `window.fullscreen` | `F11` | `global` + terminal | — |
| Aumentar interface · Zoom in UI | `view.zoom.in` | `Ctrl+Shift+=` | `global` + terminal | Zoom da interface; o zoom da página no navegador usa Ctrl+=. |
| Reduzir interface · Zoom out UI | `view.zoom.out` | `Ctrl+Shift+-` | `global` + terminal | Zoom da interface; o zoom da página no navegador usa Ctrl+-. |
| Restaurar interface a 100% · Reset UI zoom | `view.zoom.reset` | `Ctrl+Shift+0` | `global` + terminal | Ctrl+0 sem Shift muda para a aba 10. |

## 4. O terminal recebe quase tudo

O terminal é uma superfície de texto: o shell espera receber praticamente toda combinação. A Forja só intercepta o que está na tabela abaixo. Cada linha é uma exceção consciente, com o custo declarado.

| Atalho | Ação da Forja | O que o shell perde | Como conviver |
|---|---|---|---|
| `Ctrl+K` | Abrir a Bigorna | kill-line no readline/PSReadLine | use Ctrl+U para apagar a linha; ou rebinde a Bigorna em `~/.forja/atalhos.toml` |
| `Ctrl+Shift+P` | Modo comando da Bigorna | nada comum | — |
| `Ctrl+P` | Achar arquivo | histórico anterior no readline | use ↑; a busca de arquivo é a âncora do produto |
| `Ctrl+Shift+N` | Nova bancada | nada comum | — |
| `Alt+PageDown` | Próxima bancada | nada comum | PageDown sem Alt rola o terminal |
| `Alt+PageUp` | Bancada anterior | nada comum | PageUp sem Alt rola o terminal |
| `F6` | Focar abas de conteúdo | nada comum | sai do terminal para as abas sem trocar de aba |
| `Ctrl+Tab` | Próxima aba | nada comum | Ctrl+Shift+Tab volta |
| `Ctrl+Shift+Tab` | Aba anterior | nada comum | — |
| `Ctrl+Shift+W` | Fechar aba em qualquer superfície | nada comum | fecha a aba mesmo com o shell focado |
| `Ctrl+Shift+T` | Reabrir aba fechada | nada comum | — |
| `Ctrl+Backtick` | Mostrar ou ocultar terminal | nada comum | — |
| `Ctrl+Shift+B` | Mostrar ou ocultar painel direito | nada comum | — |
| `Ctrl+,` | Abrir Ajustes | nada comum | — |
| `Ctrl+Shift+,` | Abrir atalhos | nada comum | — |
| `Ctrl+Shift+/` | Ver todos os atalhos | nada comum | — |
| `F11` | Tela cheia | tela cheia em alguns TUIs | use o menu da janela se o TUI precisar de F11 |
| `Ctrl+Shift+=` | Aumentar interface | nada comum | — |
| `Ctrl+Shift+-` | Reduzir interface | nada comum | — |
| `Ctrl+Shift+0` | Restaurar interface a 100% | nada comum | — |
| `Ctrl+Shift+A` | Iniciar ajudante padrão | selecionar tudo em alguns terminais | a Forja usa para iniciar o ajudante padrão |
| `Ctrl+Alt+Shift+A` | Iniciar ajudante de um perfil | nada comum | — |
| `Ctrl+Shift+H` | Ir para o ajudante da bancada | nada comum | — |
| `Ctrl+Shift+M` | Abrir Mutirão | nada comum | — |
| `Ctrl+Shift+D` | Abrir Diferenças | nada comum | — |
| `Alt+1` | Ir para a bancada 1 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+2` | Ir para a bancada 2 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+3` | Ir para a bancada 3 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+4` | Ir para a bancada 4 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+5` | Ir para a bancada 5 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+6` | Ir para a bancada 6 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+7` | Ir para a bancada 7 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+8` | Ir para a bancada 8 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+9` | Ir para a bancada 9 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Alt+0` | Ir para a bancada 10 | argumento numérico no readline | use o menu de bancadas ou Alt+PageUp/Down |
| `Ctrl+1` | Ir para a aba 1 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+2` | Ir para a aba 2 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+3` | Ir para a aba 3 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+4` | Ir para a aba 4 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+5` | Ir para a aba 5 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+6` | Ir para a aba 6 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+7` | Ir para a aba 7 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+8` | Ir para a aba 8 | caractere de controle do shell | raramente usado; a família de abas é âncora do produto |
| `Ctrl+9` | Ir para a aba 9 | sem uso comum no shell | a família de abas é âncora do produto |
| `Ctrl+0` | Ir para a aba 10 | sem uso comum no shell | a família de abas é âncora do produto |

Fora dessa lista, o terminal é dono do teclado. `Ctrl+C` é interrupção, `Ctrl+D` é fim de entrada, `Ctrl+R` é busca de histórico, `Ctrl+W` apaga a palavra anterior e `Ctrl+U` apaga a linha. A cópia do terminal é `Ctrl+Shift+C`; a colagem é `Ctrl+V`.

## 5. Convenções universais do Windows

| Convenção | Onde vale | Atalho da Forja |
|---|---|---|
| Copiar, recortar e colar | Editor e campos de texto | `Ctrl+C`, `Ctrl+X`, `Ctrl+V` |
| Colar no terminal | Terminal | `Ctrl+V` (a cópia é `Ctrl+Shift+C`) |
| Salvar | Editor | `Ctrl+S` |
| Desfazer e refazer | Editor | `Ctrl+Z`, `Ctrl+Y` |
| Selecionar tudo | Editor | `Ctrl+A` |
| Buscar | Editor e navegador | `Ctrl+F` |
| Fechar | Aba, página e janela | `Ctrl+W`; no terminal, `Ctrl+Shift+W` |
| Trocar de aba | Qualquer superfície | `Ctrl+Tab`, `Ctrl+Shift+Tab` |
| Renomear | Editor | `F2` |
| Próxima busca | Editor | `F3`, `Shift+F3` |
| Tela cheia | Qualquer superfície | `F11` |
| Definição e devtools | Editor e navegador | `F12` |
| Voltar e avançar | Navegador | `Alt+←`, `Alt+→` |

## 6. Verificação de conflitos

A checagem lê `design/keymap.toml`, normaliza cada acorde (modificadores e nomes de tecla) e compara **dentro de cada escopo**. Ela também compara o global marcado como `even_in_terminal` com o escopo `terminal`, porque essa é a única colisão que rouba tecla do shell de fato.

| Escopo | Atalhos | Conflitos internos | Colisões global-no-terminal | Resultado |
|---|---:|---:|---:|---|
| `global` | 47 | 0 | — | ok |
| `terminal` | 13 | 0 | 0 | ok |
| `editor` | 27 | 0 | — | ok |
| `browser` | 14 | 0 | — | ok |
| `anvil` | 7 | 0 | — | ok |
| **Total** | **108** | **0** | **0** | **ok** |

Sobreposições entre superfícies diferentes são intencionais e não são conflito: o mesmo acorde pode significar coisas distintas no editor e no navegador. As sombras reais estão listadas abaixo; em todas, o escopo local vence e o global continua disponível na Bigorna.

| Acorde | Global | Local | Quem vence | Por quê |
|---|---|---|---|---|
| `Ctrl+Shift+F` | `find.in_files` | `terminal.search` (`terminal`) | `terminal.search` | o terminal usa a tecla para buscar no histórico; a busca global fica na Bigorna |
| `Ctrl+W` | `tab.close` | `browser.close_page` (`browser`) | `browser.close_page` | o navegador tem foco e a convenção da página vence |
| `Ctrl+Shift+T` | `tab.reopen` | `browser.reopen_page` (`browser`) | `browser.reopen_page` | o navegador tem foco e a convenção da página vence |
| `Ctrl+0` | `tab.switch_10` | `browser.zoom.reset` (`browser`) | `browser.zoom.reset` | o navegador tem foco e a convenção da página vence |

Fora dessas sombras, não há dois atalhos com o mesmo acorde no mesmo escopo nem colisão entre global-no-terminal e terminal.

## 7. Personalização

O usuário pode sobrescrever qualquer id em `~/.forja/atalhos.toml`. O projeto pode sobrescrever de novo em `.forja/atalhos.toml`. A precedência é: padrão do módulo < usuário < projeto. Um id desconhecido é avisado e ignorado; um acorde inválido é recusado com o motivo.

```toml
schema_version = 1

[[binding]]
id = "anvil.open"
key = "Ctrl+Alt+K"

[[binding]]
id = "tab.close"
key = ""          # desliga o atalho; a ação continua na Bigorna
```

Regras da sobrescrita:

- O `id` precisa existir no mapa; não se cria ação nova por arquivo de atalho.
- Um acorde que já pertence a outro id **no mesmo escopo** é recusado. O app mostra o dono atual e pede para desligar um dos dois.
- Tomar uma tecla que o terminal usa exige que o binding global seja marcado `even_in_terminal = true` no padrão; a sobrescrita do usuário não ganha esse direito sozinha.
- `key = ""` desliga o atalho sem quebrar o id.
- `keymap.conflict_policy` decide entre avisar e bloquear a gravação; o teste de contrato do kernel sempre bloqueia conflito dentro do mesmo escopo.

## 8. Acessibilidade e descoberta

- A Bigorna lista toda ação com o atalho ao lado; quem não pode usar o teclado chega a tudo por ela.
- O rodapé da Bigorna mostra o acorde e o escopo do item selecionado, inclusive quando o atalho está sombreado naquela superfície.
- Nenhum atalho depende de duplo toque, pressão ou gesto; todos são acordes de teclado.
- A ajuda (`Ctrl+Shift+/`) abre esta tabela filtrada pela superfície com foco.
- Rebinding é texto; leitor de tela lê o id, o título e o acorde a partir das mesmas chaves i18n.
