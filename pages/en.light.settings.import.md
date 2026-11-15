---
state: en.light.settings.import
capture_sha: 29eae50cb395b98a
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.light.settings.import.png)

state: en.light.settings.import
capture_sha: pending
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.light.settings.import.png)

# Settings — Import from Harbor

Variante em inglês e tema claro da página de referência (`pt-BR.dark.settings.import`): mesma estrutura de layout, com textos em inglês, paleta clara e captura em 1920×1080 (a referência foi capturada em 1280×800, portanto as coordenadas das duas páginas não são diretamente comparáveis). Tela de configurações que exibe a seção "Import from Harbor", para trazer dados de uma instalação anterior do Harbor 0.1 (nada encontrado nesta máquina).

## Como chegar
No trilho esquerdo de ícones, clique no botão "Settings" (engrenagem, base do trilho, (8,1004)–(44,1040)); na janela "Settings" que se abre sobre a área central, a seção "Import from Harbor" está visível no topo do conteúdo. Comando de captura: `forja-app --locale en --theme light --settings import`.

## Regiões
- **Barra superior** (0,0)–(1920,42): etiqueta "No project", botões "Find file  Ctrl+P" e "Anvil  Ctrl+K" à esquerda; à direita, um botão sem nome acessível com ícone de sol (tema) e os botões de janela "–", "□" e "×".
- **Trilho à esquerda** (0,42)–(48,1048): logotipo do app (imagem sem nome, (12,50)–(40,78)) e uma coluna de botões-ícone: "Board", "Helpers", "Routines", "Muster", "Requests", "Skills", "Usage", "Extensions", "Archive" e, no fim, "Settings". Nenhum está marcado.
- **Painel lateral esquerdo (Projects)** (48,42)–(304,1048): botão "More tabs" (estouro de abas) e a aba "Projects" (marcada, com sublinhado laranja); botões "Add folder" (destaque laranja), "New project" e "Clone"; ícone de pasta central ((168,201)–(196,229)) e mensagem "No project yet. Add an existing folder or create a new one.".
- **Área central** (320,42)–(1612,1048): botão "More tabs" e abas "Terminal" (marcada), "Editor", "Browser", "Changes", "Preview", "Bench", "Review", "History", "Extension panels" e "Helper (chat)"; abaixo, a área da bancada/terminal (região sem nome (328,150)–(1604,1036)) com o aviso "Click to type" no canto inferior direito. A barra de ferramentas do terminal ("+", filtros, "Recipes", "Shell", aba "pwsh · Running") fica encoberta pelo diálogo "Settings". Nesta variante as abas secundárias do painel esquerdo aparecem recolhidas no botão "More tabs" (na referência pt-BR elas estavam nomeadas).
- **Painel direito** (1628,42)–(1920,1048): botão "More tabs" e abas "Files" (marcada, com sublinhado) e "Changes"; ícone de grade ((1756,117)–(1784,145)) e a mensagem "No active bench. Open a bench to see its files.".
- **Diálogo "Settings" (janela flutuante)** (328,58)–(1028,281): título "Settings", botão "Close window" (×), campo de busca de ajustes (placeholder "Search settings"), título "Import from Harbor", texto explicativo e a mensagem "No Harbor 0.1 data was found on this machine.". Alças de redimensionamento sem nome nas laterais.
- **Barra de status** (0,1048)–(1920,1080): "Ready" (em verde), "56 modules" e "Notifications" à direita.

## Elementos
| Elemento | Tipo | Posição (px) | Estado | O que faz |
|---|---|---|---|---|
| "Find file  Ctrl+P" | Button | (99,5)–(218,37) | | Abre a busca de arquivos (atalho Ctrl+P) |
| "Anvil  Ctrl+K" | Button | (226,5)–(327,37) | | Abre a paleta de comandos (atalho Ctrl+K) |
| (sem nome acessível) | Unknown | (1741,5)–(1773,37) | | Ícone de sol: alterna o tema |
| "–" | Button | (1795,5)–(1827,37) | | Minimiza a janela |
| "□" | Button | (1835,5)–(1867,37) | | Maximiza/restaura a janela |
| "×" | Button | (1875,5)–(1909,37) | | Fecha a janela |
| "No project" | Label | (11,12)–(77,31) | | Mostra que não há projeto aberto |
| (sem nome acessível) | Unknown | (304,42)–(320,1048) | | Divisor redimensionável entre painel esquerdo e área central |
| (sem nome acessível) | Unknown | (1612,42)–(1628,1048) | | Divisor redimensionável entre área central e painel direito |
| (sem nome acessível) | Unknown | (319,49)–(339,69) | | Alça superior do divisor esquerdo |
| (sem nome acessível) | Unknown | (1018,49)–(1038,69) | | Alça superior do divisor direito |
| (sem nome acessível) | Image | (12,50)–(40,78) | | Logotipo do app no topo do trilho |
| "More tabs" | Button | (64,50)–(151,85) | | Abre o menu de abas ocultas do painel esquerdo |
| "More tabs" | Button | (324,50)–(411,85) | | Abre o menu de abas ocultas da área central |
| "More tabs" | Button | (1632,50)–(1719,85) | | Abre o menu de abas ocultas do painel direito |
| (sem nome acessível) | Unknown | (339,51)–(1018,67) | | Faixa superior da área central atrás do diálogo |
| "Projects" | Button | (159,52)–(231,84) | marcado=True | Aba ativa do painel esquerdo; mostra a lista de projetos |
| "Terminal" | Button | (419,52)–(496,84) | marcado=True | Aba ativa da área central; mostra o terminal |
| "Editor" | Button | (496,52)–(556,84) | marcado=False | Aba do editor de arquivos |
| "Browser" | Button | (556,52)–(629,84) | marcado=False | Aba do navegador de conteúdo |
| "Changes" | Button | (629,52)–(706,84) | marcado=False | Aba de comparação de diffs |
| "Preview" | Button | (706,52)–(777,84) | marcado=False | Aba de visualização de arquivos |
| "Bench" | Button | (777,52)–(838,84) | marcado=False | Aba da bancada de trabalho |
| "Review" | Button | (838,52)–(905,84) | marcado=False | Aba de revisão |
| "History" | Button | (905,52)–(973,84) | marcado=False | Aba de histórico |
| "Extension panels" | Button | (973,52)–(1099,84) | marcado=False | Aba de painéis de extensões (parte esquerda encoberta pelo diálogo) |
| "Helper (chat)" | Button | (1099,52)–(1202,84) | marcado=False | Aba do chat com ajudantes |
| "Files" | Button | (1727,52)–(1778,84) | marcado=True | Aba ativa do painel direito; lista de arquivos |
| "Changes" | Button | (1778,52)–(1855,84) | marcado=False | Aba de mudanças pendentes |
| "Settings" | Window | (328,58)–(1028,281) | foco | Diálogo modal de configurações |
| "Close window" | Button | (987,67)–(1019,99) | | Fecha o diálogo (ícone ×) |
| (sem nome acessível) | Unknown | (321,69)–(337,271) | | Alça lateral esquerda de redimensionamento do diálogo |
| (sem nome acessível) | Unknown | (1020,69)–(1036,271) | | Alça lateral direita de redimensionamento do diálogo |
| "Settings" | Label | (337,71)–(403,95) | | Título do diálogo |
| "Board" | Button | (8,94)–(44,130) | marcado=False | Botão-ícone do trilho: quadro geral |
| "Add folder" | Button | (64,101)–(154,133) | | Abre o seletor para importar uma pasta como projeto |
| "New project" | Button | (162,101)–(261,133) | | Cria um projeto novo |
| "+" | Button | (407,113)–(441,145) | | Ação de adicionar na barra do terminal (encoberta pelo diálogo) |
| (sem nome acessível) | Unknown | (444,113)–(476,145) | | Botão-ícone de filtro na barra do terminal (encoberto pelo diálogo) |
| (sem nome acessível) | Unknown | (479,113)–(511,145) | | Botão-ícone de filtro na barra do terminal (encoberto pelo diálogo) |
| (sem nome acessível) | Unknown | (514,113)–(546,145) | | Botão-ícone de filtro na barra do terminal (encoberto pelo diálogo) |
| (sem nome acessível) | Unknown | (549,113)–(581,145) | | Botão-ícone de filtro na barra do terminal (encoberto pelo diálogo) |
| "Recipes" | Button | (584,113)–(655,145) | | Abre a lista de receitas do terminal (encoberto pelo diálogo) |
| "Shell" | Button | (658,113)–(711,145) | | Abre um terminal shell (encoberto pelo diálogo) |
| (ícone) | Label | (1756,117)–(1784,145) | | Ícone de grade no painel direito (estado vazio) |
| "pwsh · Running" | Button | (328,118)–(404,140) | | Aba da sessão de terminal pwsh em execução (encoberta pelo diálogo) |
| (sem nome acessível) | TextInput | (337,121)–(1019,153) | | Campo de busca com placeholder "Search settings" |
| "Close pwsh" | Button | (385,121)–(401,137) | | Fecha a aba pwsh (encoberto pelo diálogo) |
| "Helpers" | Button | (8,138)–(44,174) | marcado=False | Botão-ícone do trilho: ajudantes de código |
| "Clone" | Button | (64,141)–(123,173) | | Clona um repositório como projeto |
| (sem nome acessível) | Unknown | (328,150)–(1604,1036) | | Área da bancada/terminal (abaixo do diálogo; contém o aviso "Click to type") |
| "No active bench. Open a bench to see its files." | Label | (1644,153)–(1896,191) | | Mensagem de estado vazio do painel direito |
| "Import from Harbor" | Label | (337,181)–(494,205) | | Título da seção de importação |
| "Routines" | Button | (8,182)–(44,218) | marcado=False | Botão-ícone do trilho: rotinas |
| (ícone) | Label | (168,201)–(196,229) | | Ícone de pasta no estado vazio do painel de projetos |
| "Brings projects, workbenches, commands, notes, SSH targets, theme and font from Harbor 0.1. Harbor is left unchanged and no secret is imported." | Label | (337,213)–(1011,245) | | Explica o que a importação traz e o que não toca |
| "Muster" | Button | (8,226)–(44,262) | marcado=False | Botão-ícone do trilho: mutirão |
| "No project yet. Add an existing folder or create a new one." | Label | (67,237)–(297,275) | | Estado vazio do painel de projetos |
| "No Harbor 0.1 data was found on this machine." | Label | (337,253)–(626,272) | | Resultado da verificação: nada a importar |
| "Requests" | Button | (8,270)–(44,306) | marcado=False | Botão-ícone do trilho: pedidos |
| (sem nome acessível) | Unknown | (319,271)–(339,291) | | Alça inferior do divisor esquerdo |
| (sem nome acessível) | Unknown | (1018,271)–(1038,291) | | Alça inferior do divisor direito |
| (sem nome acessível) | Unknown | (339,273)–(1018,289) | | Borda inferior da área central atrás do diálogo |
| "Skills" | Button | (8,314)–(44,350) | marcado=False | Botão-ícone do trilho: skills |
| "Usage" | Button | (8,358)–(44,394) | marcado=False | Botão-ícone do trilho: uso |
| "Extensions" | Button | (8,402)–(44,438) | marcado=False | Botão-ícone do trilho: extensões |
| "Archive" | Button | (8,446)–(44,482) | marcado=False | Botão-ícone do trilho: acervo |
| "Settings" | Button | (8,1004)–(44,1040) | marcado=False | Botão-ícone do trilho: abre a janela de ajustes |
| "Ready" | Label | (10,1055)–(48,1074) | | Status geral em verde |
| "56 modules" | Label | (70,1055)–(142,1074) | | Contagem de módulos carregados |
| "Notifications" | Label | (1808,1055)–(1886,1074) | | Atalho para notificações |

## Aparência
Tema claro: fundo marfim/creme (~ #faf7f2) nas regiões, com painéis e a área central em tons muito próximos, delimitados por bordas finas cinza-claro. O destaque principal é o laranja (cor "Brasa"): a aba "Projects" ativa tem texto e sublinhado laranja e "Add folder" é um botão sólido laranja com texto branco; "Terminal" e "Files" ativos usam texto escuro com sublinhado escuro/laranja. Textos em cinza-escuro/quase preto, com descrições e estados vazios em cinza médio (contraste menor). O diálogo "Settings" usa painel branco com borda e sombra discretas, título à esquerda e campo de busca com borda arredondada sutil. A barra de status exibe "Ready" em verde e o restante em cinza. Comparada à referência escura, inverte-se a relação fundo/texto e o diálogo ganha contraste por ser branco sobre fundo creme. Tipografia sans-serif única em dois tamanhos (títulos de seção maiores, corpo menor), alinhamento à esquerda no diálogo e ícones centrados nos botões do trilho (36 px por botão, com o botão "Settings" isolado na base).

## Problemas encontrados
- O diálogo "Settings" (328,58)–(1028,281) encobre a barra de ferramentas do terminal — "+", os quatro botões de filtro sem nome, "Recipes", "Shell", a aba "pwsh · Running" e "Close pwsh" — e a metade esquerda da aba "Extension panels" (973,52)–(1099,84); esperado num modal, mas a sobreposição é indicada apenas por uma sombra/borda discretas.
- Vários controles sem nome acessível: o alternador de tema (1741,5), os quatro botões-ícone de filtro do terminal (444–581,113), o campo de busca do diálogo (337,121), os divisores e alças de redimensionamento e a grande área central (328,150), dificultando o uso por leitor de tela.
- A faixa sem nome (339,51)–(1018,67) fica sobreposta/atrás das abas centrais nomeadas na mesma faixa vertical.
- Textos secundários em cinza médio sobre fundo creme ("No project yet…", "No active bench…", "Click to type", descrição da importação) têm contraste apenas moderado.
- Nenhum elemento ultrapassa os limites da janela (1920×1080); nenhum texto estranho ou em idioma indevido nesta variante.
