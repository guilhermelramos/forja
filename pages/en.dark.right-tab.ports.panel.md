---
state: en.dark.right-tab.ports.panel
capture_sha: fcd1e507cde2cc00
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.dark.right-tab.ports.panel.png)

# Painel Portas (aba lateral direita) — variante en/dark

Tela do painel "Ports", que lista as portas TCP em escuta detectadas no ambiente de trabalho; nesta captura nenhuma porta foi encontrada.

Variante em inglês (`en`) com tema escuro (`dark`) da página de referência pt-BR, capturada em janela de 1920×1080 (a referência tem 1280×800). O que muda: todos os textos ficam em inglês ("Ports", "Ready", "No project"…); surgem três botões "More tabs" no início de cada linha de abas, concentrando as abas restantes (ausentes na referência); o painel direito expõe apenas "Ports", "Files" e "Search"; a aba "Helper (chat)" aparece na área central (na referência, o chat ficava no painel direito); e, pela janela maior, não ocorrem as sobreposições e o corte de abas apontados na referência. Cores e contraste são os mesmos do tema dark.

## Como chegar
Na janela principal do Forja, clique na aba **"Ports"** na linha de abas do painel lateral direito (à direita do botão "More tabs", à esquerda de **"Files"**). A captura foi gerada com o comando `forja-app --locale en --theme dark --right-tab ports.panel`.

## Regiões
- **Barra superior (barra de título)** — (0,0)–(1920,42): rótulo do projeto atual ("No project"), botões "Find file  Ctrl+P" e "Anvil  Ctrl+K", botão-ícone de tema (sol) e os controles de janela "–", "□", "×".
- **Trilho à esquerda** — (0,42)–(52,1048): logotipo da Forja no topo e botões-ícone verticais "Board", "Helpers", "Routines", "Muster", "Requests", "Skills", "Usage", "Extensions", "Archive" e, na base, "Settings".
- **Painel lateral esquerdo (Projects)** — (52,42)–(304,1048): botão "More tabs" e aba "Projects" (ativa); botões "Add folder", "New" e "Clone"; estado vazio com ícone de pasta e o texto "No project yet. Add an existing folder or create a new one.".
- **Área central (Terminal)** — (320,42)–(1612,1048): botão "More tabs" e abas "Terminal" (ativa), "Editor", "Browser", "Changes", "Preview", "Bench", "Review", "History", "Extension panels" e "Helper (chat)"; barra do terminal com o chip "pwsh", "+", ícones de fechar/dividir/sessão/busca e botões "Recipes" e "Shell"; superfície do terminal vazia com a dica "Click to type" no canto inferior direito.
- **Painel lateral direito (Ports)** — (1628,42)–(1920,1048): botão "More tabs" e abas "Ports" (ativa), "Files" e "Search"; cabeçalho "Ports" com o contador "0 listening" e o botão "Refresh"; mensagem "No listening TCP port was found.".
- **Divisores** — (304,42)–(320,1048) e (1612,42)–(1628,1048): separadores verticais arrastáveis entre painel esquerdo/área central e área central/painel direito.
- **Barra de status** — (0,1048)–(1920,1080): "Ready" (tom de destaque) e "56 modules" à esquerda; "Notifications" à direita.

## Elementos
| Elemento | Tipo | Posição (px) | Estado | O que faz |
|---|---|---|---|---|
| "Find file  Ctrl+P" | Button | (99,5)–(218,37) | click,focus | Abre a busca rápida de arquivos (atalho Ctrl+P). |
| "Anvil  Ctrl+K" | Button | (226,5)–(327,37) | click,focus | Abre a paleta de comandos "Anvil" (bigorna; atalho Ctrl+K). |
| (sem nome acessível) | Unknown | (1741,5)–(1773,37) | click,focus | Botão-ícone de alternância de tema (sol), à esquerda dos controles de janela. |
| "–" | Button | (1795,5)–(1827,37) | click,focus | Minimiza a janela. |
| "□" | Button | (1835,5)–(1867,37) | click,focus | Restaura/maximiza a janela. |
| "×" | Button | (1875,5)–(1909,37) | click,focus | Fecha a janela. |
| "No project" | Label | (11,12)–(77,31) | click,focus | Indica o projeto atual na barra de título; sem projeto aberto. |
| (sem nome acessível) | Unknown | (304,42)–(320,1048) | focus | Divisor vertical arrastável entre o painel esquerdo e a área central. |
| (sem nome acessível) | Unknown | (1612,42)–(1628,1048) | focus | Divisor vertical arrastável entre a área central e o painel direito. |
| (sem nome acessível) | Image | (12,50)–(40,78) | click,focus | Logotipo da Forja (bigorna sobre quadrado laranja) no topo do trilho. |
| "More tabs" | Button | (64,50)–(151,85) | click,focus | Transbordo de abas do painel esquerdo; abre as abas ocultas além de "Projects". |
| "More tabs" | Button | (324,50)–(411,85) | click,focus | Transbordo de abas da área central; abre as abas ocultas além das dez visíveis. |
| "More tabs" | Button | (1632,50)–(1719,85) | click,focus | Transbordo de abas do painel direito; abre as abas ocultas além de "Ports", "Files" e "Search". |
| "Projects" | Button | (159,52)–(231,84) | click,focus, marcado=True | Aba ativa do painel esquerdo; mostra a lista de projetos. |
| "Terminal" | Button | (419,52)–(496,84) | click,focus, marcado=True | Aba ativa da área central; mostra o terminal. |
| "Editor" | Button | (496,52)–(556,84) | click,focus, marcado=False | Aba que troca a área central para o editor. |
| "Browser" | Button | (556,52)–(629,84) | click,focus, marcado=False | Aba que troca a área central para o navegador de arquivos. |
| "Changes" | Button | (629,52)–(706,84) | click,focus, marcado=False | Aba que troca a área central para a visualização de mudanças. |
| "Preview" | Button | (706,52)–(777,84) | click,focus, marcado=False | Aba que troca a área central para o visualizador. |
| "Bench" | Button | (777,52)–(838,84) | click,focus, marcado=False | Aba que troca a área central para a bancada. |
| "Review" | Button | (838,52)–(905,84) | click,focus, marcado=False | Aba que troca a área central para a revisão. |
| "History" | Button | (905,52)–(973,84) | click,focus, marcado=False | Aba que troca a área central para o histórico. |
| "Extension panels" | Button | (973,52)–(1099,84) | click,focus, marcado=False | Aba que troca a área central para os painéis de extensões. |
| "Helper (chat)" | Button | (1099,52)–(1202,84) | click,focus, marcado=False | Aba que troca a área central para o chat do ajudante. |
| "Ports" | Button | (1727,52)–(1782,84) | click,focus, marcado=True | Aba ativa do painel direito; mostra esta tela de portas em escuta. |
| "Files" | Button | (1782,52)–(1833,84) | click,focus, marcado=False | Aba do painel direito que mostra arquivos. |
| "Search" | Button | (1833,52)–(1898,84) | click,focus, marcado=False | Aba do painel direito que abre a busca. |
| "Board" | Button | (8,94)–(44,130) | click,focus, marcado=False | Botão-ícone do trilho que abre o quadro. |
| "Add folder" | Button | (64,101)–(154,133) | click,focus | Botão primário laranja; abre o seletor para adicionar uma pasta de projeto. |
| "New" | Button | (162,101)–(213,133) | click,focus | Cria um novo projeto. |
| "Clone" | Button | (221,101)–(280,133) | click,focus | Abre o fluxo de clonagem de repositório. |
| "Ports" | Label | (1632,101)–(1674,133) | click | Título do painel atual. |
| "0 listening" | Label | (1681,101)–(1736,133) | click | Contador de portas TCP detectadas; zero nesta captura. |
| "Refresh" | Button | (1744,101)–(1814,133) | click,focus | Repete a detecção de portas em escuta; um pequeno indicador de giro aparece ao lado. |
| "+" | Button | (407,113)–(441,145) | click,focus | Abre uma nova sessão/aba de terminal. |
| (sem nome acessível) | Unknown | (444,113)–(476,145) | click,focus | Botão-ícone fechar (X) da sessão atual. |
| (sem nome acessível) | Unknown | (479,113)–(511,145) | click,focus | Botão-ícone de dividir o terminal. |
| (sem nome acessível) | Unknown | (514,113)–(546,145) | click,focus | Botão-ícone de sessão/aba (retângulo). |
| (sem nome acessível) | Unknown | (549,113)–(581,145) | click,focus | Botão-ícone de busca no terminal (lupa). |
| "Recipes" | Button | (584,113)–(655,145) | click,focus | Lista de receitas de comandos para o terminal. |
| "Shell" | Button | (658,113)–(711,145) | click,focus | Seletor do shell da sessão. |
| (sem nome acessível) | Unknown | (328,118)–(404,140) | click,focus | Chip da sessão ativa "pwsh", com ponto de status verde. |
| (sem nome acessível) | Unknown | (385,121)–(401,137) | click,focus | Ponto de status verde dentro do chip pwsh. |
| "Helpers" | Button | (8,138)–(44,174) | click,focus, marcado=False | Botão-ícone do trilho que abre a lista de ajudantes. |
| (sem nome acessível) | Unknown | (328,150)–(1604,1036) | click,focus | Superfície do terminal, vazia; clique concentra o cursor ("Click to type" no canto inferior direito). |
| (ícone) | Label | (168,161)–(196,189) | click | Ícone de pasta do estado vazio da lista de projetos. |
| "No listening TCP port was found." | Label | (1632,163)–(1834,182) | click | Mensagem de estado vazio do painel de portas. |
| "Routines" | Button | (8,182)–(44,218) | click,focus, marcado=False | Botão-ícone do trilho que abre rotinas. |
| "No project yet. Add an existing folder or create a new one." | Label | (67,197)–(297,235) | click | Mensagem de estado vazio da lista de projetos. |
| "Muster" | Button | (8,226)–(44,262) | click,focus, marcado=False | Botão-ícone do trilho que abre o mutirão. |
| "Requests" | Button | (8,270)–(44,306) | click,focus, marcado=False | Botão-ícone do trilho que abre pedidos. |
| "Skills" | Button | (8,314)–(44,350) | click,focus, marcado=False | Botão-ícone do trilho que abre skills. |
| "Usage" | Button | (8,358)–(44,394) | click,focus, marcado=False | Botão-ícone do trilho que abre o uso de recursos. |
| "Extensions" | Button | (8,402)–(44,438) | click,focus, marcado=False | Botão-ícone do trilho que abre extensões. |
| "Archive" | Button | (8,446)–(44,482) | click,focus, marcado=False | Botão-ícone do trilho que abre o acervo. |
| "Settings" | Button | (8,1004)–(44,1040) | click,focus, marcado=False | Botão-ícone do trilho que abre os ajustes, fixado na base. |
| "Ready" | Label | (10,1055)–(48,1074) | click | Estado geral do app na barra de status. |
| "56 modules" | Label | (70,1055)–(142,1074) | click | Quantidade de módulos carregados. |
| "Notifications" | Label | (1808,1055)–(1886,1074) | click,focus | Área de notificações na barra de status. |

## Aparência
Tema escuro quente: fundo quase preto com tom amarronzado nas barras e painéis, e a área central do terminal mais escura que o restante. Acento laranja ("Brasa") no botão primário "Add folder", no logotipo e no sublinhado das abas ativas "Projects", "Terminal" e "Ports"; os botões secundários têm contorno fino e fundo levemente mais claro. Texto em cinza-claro com contraste bom nos rótulos principais; "Ready" na barra de status aparece em tom alaranjado de destaque, e textos secundários ("0 listening", "Click to type", atalhos "Ctrl+P"/"Ctrl+K") em cinza mais apagado. Ponto verde indica a sessão "pwsh" ativa. Tipografia sem serifa de tamanho médio (~13 px) nos botões e rótulos; cantos arredondados moderados em botões e chips; cada linha de abas começa com um botão "More tabs" contornado; o trilho de ícones tem 36 px de largura útil, alinhado ao centro; a barra de status é uma faixa fina e contínua na base. Em 1920×1080 todas as abas cabem sem sobreposição, ao contrário da referência pt-BR.

## Problemas encontrados
- Botões sem nome acessível: alternância de tema (1741,5)–(1773,37) e os quatro ícones da barra do terminal (fechar, dividir, sessão e busca; (444,113)–(476,145), (479,113)–(511,145), (514,113)–(546,145), (549,113)–(581,145)).
- O chip da sessão "pwsh" (328,118)–(404,140) e seu ponto de status (385,121)–(401,137) não têm nome acessível.
- Os divisores (304,42)–(320,1048) e (1612,42)–(1628,1048) e o logotipo (12,50)–(40,78) não têm nome acessível.
- A superfície do terminal (328,150)–(1604,1036) é uma grande área clicável sem nome acessível.
- O rótulo "(ícone)" do estado vazio (168,161)–(196,189) tem um nome de acessibilidade de marcação (placeholder), sem significado.
- Três botões compartilham o mesmo nome "More tabs" ((64,50), (324,50) e (1632,50)), o que é ambíguo para navegação por leitor de tela.
- Vários Labels expõem ações de clique e/ou foco, papel incoerente com o tipo: "No project", "Ports", "0 listening", "Ready", "56 modules", "Notifications", "No listening TCP port was found." e "No project yet. Add an existing folder or create a new one.".
- Três abas aparecem simultaneamente com marcado=True ("Projects", "Terminal" e "Ports"), o que torna ambíguo qual painel está de fato selecionado.
- O pequeno indicador de giro ao lado de "Refresh" (≈(1830,118)–(1848,134)) e o texto "Click to type" no canto do terminal (≈(1536,1016)–(1596,1030)) aparecem na imagem, mas não têm elemento correspondente na árvore de acessibilidade.
