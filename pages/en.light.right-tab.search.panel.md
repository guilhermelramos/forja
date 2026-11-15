---
state: en.light.right-tab.search.panel
capture_sha: 207b016bbf2e2256
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.light.right-tab.search.panel.png)

state: en.light.right-tab.search.panel
capture_sha: (pendente — preencher com o hash da captura)
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.light.right-tab.search.panel.png)

# Search Panel (right panel)

Tela de busca e substituição de texto nos arquivos da bancada, exibida como aba do painel direito; com o estado inicial "No project", a busca fica desativada até que uma bancada seja aberta.

## Como chegar
Na barra superior, clique em "Find file  Ctrl+P" para localizar arquivos ou use a faixa de abas do painel direito e clique em "Search" (é a aba marcada). Por linha de comando: `forja-app --locale en --theme light --right-tab search.panel`.

## Regiões
- **Barra superior** (0,0)–(1920,42): rótulo de projeto "No project", botões "Find file  Ctrl+P" e "Anvil  Ctrl+K", ícone de tema sem nome à direita e botões de janela "–", "□", "×".
- **Trilho à esquerda** (0,42)–(48,1048): logotipo (imagem sem nome em (12,50)–(40,78)) e uma coluna de botões-ícone com rótulo acessível: "Board", "Helpers", "Routines", "Muster", "Requests", "Skills", "Usage", "Extensions", "Archive" e, na base, "Settings"; nenhum está marcado.
- **Painel lateral esquerdo** (48,42)–(304,1048): botão "More tabs" e a aba "Projects" (marcada); botões "Add folder" (laranja), "New project" e "Clone"; estado vazio com ícone de pasta e o texto "No project yet. Add an existing folder or create a new one."
- **Área central** (320,42)–(1612,1048): faixa de abas ("More tabs" + "Terminal" marcada, "Editor", "Browser", "Changes", "Preview", "Bench", "Review", "History", "Extension panels", "Helper (chat)"), barra de ferramentas do terminal (chip "pwsh · Running" com o botão "Close pwsh" embutido, botão "+", quatro ícones sem nome, "Recipes", "Shell") e o terminal vazio com a dica "Click to type" no canto inferior direito.
- **Painel direito** (1628,42)–(1920,1048): botão "More tabs" e abas "Files", "Search" (marcada), "Ports"; formulário de busca (campo de busca, opções "Regex", "Match case", "Whole word", campos de incluir/excluir, botões "Search"/"Stop"), aviso "Open a bench first." e seção "Replace" (campo de substituição, botões "Preview"/"Apply").
- **Barra de status** (0,1048)–(1920,1080): "Ready" à esquerda, "56 modules" e "Notifications" à direita.

## Elementos
| Elemento | Tipo | Posição (px) | Estado | O que faz |
|---|---|---|---|---|
| "Find file  Ctrl+P" | Button | (99,5)–(218,37) | normal | Abre a busca rápida de arquivos pelo atalho Ctrl+P. |
| "Anvil  Ctrl+K" | Button | (226,5)–(327,37) | normal | Abre a paleta de comandos (Anvil) pelo atalho Ctrl+K. |
| (sem nome acessível) | Unknown | (1741,5)–(1773,37) | normal | Ícone de sol/tema claro na barra superior; alterna o tema pelo clique. |
| "–" | Button | (1795,5)–(1827,37) | normal | Minimiza a janela. |
| "□" | Button | (1835,5)–(1867,37) | normal | Maximiza/restaura a janela. |
| "×" | Button | (1875,5)–(1909,37) | normal | Fecha a janela. |
| "No project" | Label | (11,12)–(77,31) | normal | Indica que nenhum projeto está aberto; clique abre a lista de projetos. |
| (sem nome acessível) | Unknown | (304,42)–(320,1048) | normal | Divisor arrastável entre o painel esquerdo e a área central. |
| (sem nome acessível) | Unknown | (1612,42)–(1628,1048) | normal | Divisor arrastável entre a área central e o painel direito. |
| (sem nome acessível) | Image | (12,50)–(40,78) | normal | Logotipo da Forja no topo do trilho esquerdo. |
| "More tabs" | Button | (64,50)–(151,85) | normal | Abre a lista das abas restantes do painel esquerdo (recursos não visíveis como abas). |
| "More tabs" | Button | (324,50)–(411,85) | normal | Abre a lista das abas restantes da área central. |
| "More tabs" | Button | (1632,50)–(1719,85) | normal | Abre a lista das abas restantes do painel direito. |
| "Projects" | Button | (159,52)–(231,84) | marcado=True | Aba ativa do painel esquerdo: lista de projetos. |
| "Terminal" | Button | (419,52)–(496,84) | marcado=True | Aba ativa da área central: terminal integrado. |
| "Editor" | Button | (496,52)–(556,84) | marcado=False | Aba do editor de arquivos. |
| "Browser" | Button | (556,52)–(629,84) | marcado=False | Aba do navegador de repositório. |
| "Changes" | Button | (629,52)–(706,84) | marcado=False | Aba de comparação de diferenças. |
| "Preview" | Button | (706,52)–(777,84) | marcado=False | Aba do visualizador de arquivos. |
| "Bench" | Button | (777,52)–(838,84) | marcado=False | Aba da visão de bancada. |
| "Review" | Button | (838,52)–(905,84) | marcado=False | Aba de revisão. |
| "History" | Button | (905,52)–(973,84) | marcado=False | Aba do histórico de commits. |
| "Extension panels" | Button | (973,52)–(1099,84) | marcado=False | Aba de painéis de extensões da área central. |
| "Helper (chat)" | Button | (1099,52)–(1202,84) | marcado=False | Aba do chat do ajudante de código. |
| "Files" | Button | (1727,52)–(1778,84) | marcado=False | Aba de árvore de arquivos do painel direito. |
| "Search" | Button | (1778,52)–(1843,84) | marcado=True | Aba ativa do painel direito: busca e substituição nos arquivos. |
| "Ports" | Button | (1843,52)–(1898,84) | marcado=False | Aba de portas/serviços em escuta. |
| "Board" | Button | (8,94)–(44,130) | marcado=False | Botão-ícone do trilho: abre o quadro (kanban). |
| "Add folder" | Button | (64,101)–(154,133) | normal | Botão em destaque laranja: adiciona uma pasta de projeto. |
| "New project" | Button | (162,101)–(261,133) | normal | Cria um projeto novo. |
| (sem nome acessível) | TextInput | (1632,101)–(1908,133) | normal | Campo "Search in files" (texto-placeholder visível); sem nome acessível. |
| "+" | Button | (407,113)–(441,145) | normal | Abre nova aba/sessão de terminal. |
| (sem nome acessível) | Unknown | (444,113)–(476,145) | normal | Ícone em X na barra do terminal (fechar/limpar sessão); sem nome acessível. |
| (sem nome acessível) | Unknown | (479,113)–(511,145) | normal | Ícone de painel/divisão do terminal; sem nome acessível. |
| (sem nome acessível) | Unknown | (514,113)–(546,145) | normal | Ícone de divisão/nova área do terminal; sem nome acessível. |
| (sem nome acessível) | Unknown | (549,113)–(581,145) | normal | Ícone de lupa (busca no terminal); sem nome acessível. |
| "Recipes" | Button | (584,113)–(655,145) | normal | Abre a lista de receitas de terminal. |
| "Shell" | Button | (658,113)–(711,145) | normal | Seletor do shell da sessão (pwsh). |
| "pwsh · Running" | Button | (328,118)–(404,140) | normal | Chip da sessão pwsh com indicador verde de sessão ativa. |
| "Close pwsh" | Button | (385,121)–(401,137) | normal | Fecha a sessão pwsh; desenhado dentro do chip "pwsh · Running". |
| "Helpers" | Button | (8,138)–(44,174) | marcado=False | Botão-ícone do trilho: lista de ajudantes de código. |
| "Clone" | Button | (64,141)–(123,173) | normal | Clona um repositório como novo projeto. |
| "Regex" | CheckBox | (1632,145)–(1717,169) | marcado=False | Interpreta o termo buscado como expressão regular. |
| "Match case" | CheckBox | (1725,145)–(1843,169) | marcado=False | Diferencia maiúsculas de minúsculas na busca. |
| (sem nome acessível) | Unknown | (328,150)–(1604,1036) | normal | Área do terminal (viewport da sessão pwsh), vazia; sem nome acessível. |
| "Routines" | Button | (8,182)–(44,218) | marcado=False | Botão-ícone do trilho: rotinas/tarefas. |
| "Whole word" | CheckBox | (1632,185)–(1755,209) | marcado=False | Casa apenas palavras inteiras. |
| (ícone) | Label | (168,201)–(196,229) | normal | Ícone de pasta do estado vazio do painel de projetos. |
| (sem nome acessível) | TextInput | (1632,221)–(1908,253) | normal | Padrões de inclusão, com placeholder "Include: *.rs, src/**"; sem nome acessível. |
| "Muster" | Button | (8,226)–(44,262) | marcado=False | Botão-ícone do trilho: mutirão (execução em vários projetos). |
| "No project yet. Add an existing folder or create a new one." | Label | (67,237)–(297,275) | normal | Mensagem de estado vazio do painel de projetos. |
| (sem nome acessível) | TextInput | (1632,261)–(1908,293) | normal | Padrões de exclusão, com placeholder "Exclude: target/**"; sem nome acessível. |
| "Requests" | Button | (8,270)–(44,306) | marcado=False | Botão-ícone do trilho: pedidos (solicitações ao ajudante). |
| "Search" | Button | (1632,301)–(1697,333) | desativado | Inicia a busca; desativado porque não há bancada aberta. |
| "Stop" | Button | (1705,301)–(1757,333) | desativado | Interrompe a busca em andamento; desativado. |
| "Skills" | Button | (8,314)–(44,350) | marcado=False | Botão-ícone do trilho: skills dos ajudantes. |
| "Open a bench first." | Label | (1632,341)–(1748,360) | normal | Explica por que a busca está indisponível. |
| "Usage" | Button | (8,358)–(44,394) | marcado=False | Botão-ícone do trilho: uso de recursos. |
| "Replace" | Label | (1632,390)–(1680,409) | normal | Título da seção de substituição. |
| "Extensions" | Button | (8,402)–(44,438) | marcado=False | Botão-ícone do trilho: extensões instaladas. |
| (sem nome acessível) | TextInput | (1632,417)–(1908,449) | normal | Campo de substituição, com placeholder "Replace with"; sem nome acessível. |
| "Archive" | Button | (8,446)–(44,482) | marcado=False | Botão-ícone do trilho: acervo de conteúdo. |
| "Preview" | Button | (1632,457)–(1703,489) | desativado | Mostra a prévia das substituições; desativado. |
| "Apply" | Button | (1711,457)–(1771,489) | desativado | Aplica as substituições; desativado. |
| "Settings" | Button | (8,1004)–(44,1040) | marcado=False | Botão-ícone do trilho, na base: abre os ajustes. |
| "Ready" | Label | (10,1055)–(48,1074) | normal | Estado geral na barra de status. |
| "56 modules" | Label | (70,1055)–(142,1074) | normal | Quantidade de módulos carregados. |
| "Notifications" | Label | (1808,1055)–(1886,1074) | normal | Abre o centro de notificações. |

## Aparência
Tema claro: fundo geral off-white (faixas de abas, trilho e painéis em branco/cinza muito claro; a área do terminal levemente mais escura que os painéis), com bordas discretas em cinza-claro. Acento laranja (Brasa) usado no sublinhado das abas ativas ("Projects", "Terminal", "Search"), no botão "Add folder", no botão "Apply" (com tom mais apagado por estar desativado) e no logotipo. Texto em cinza-escuro para itens ativos e cinza médio para inativos, placeholders e a dica "Click to type"; rótulo "Ready" na barra de status em verde suave. Tipografia sans-serif de tamanho uniforme (~13–14 px), sem hierarquia forte além das abas. As três opções de busca ("Regex", "Match case", "Whole word") aparecem como interruptores circulares escuros (estilo toggle) seguidos do rótulo. Espaçamento consistente: 8 px entre botões do formulário, campos de texto com ~32 px de altura e cantos levemente arredondados; os três painéis dividem a janela com divisores de 16 px entre eles. Comparada à referência pt-BR/dark, muda o idioma dos textos (em inglês), a paleta (fundo claro em vez de escuro, com texto escuro) e a estrutura das abas: nos três painéis parte das abas foi recolhida nos botões "More tabs" em vez de exibidas em sequência.

## Problemas encontrados
- Os quatro campos de texto do painel Search e os quatro ícones da barra do terminal não têm nome acessível; só são identificáveis pelo placeholder ou pela aparência.
- O ícone de tema na barra superior (1741,5)–(1773,37), os dois divisores ((304,42)–(320,1048) e (1612,42)–(1628,1048)), o logotipo e o viewport do terminal são elementos Unknown/Image sem nome acessível.
- O botão "Close pwsh" (385,121)–(401,137) fica sobreposto dentro do chip "pwsh · Running" (328,118)–(404,140); na imagem o ícone de fechamento aparece sem a legenda do botão.
- Há nomes duplicados na árvore: "Search" (aba e botão), "Preview" (aba e botão) e três botões "More tabs", o que pode confundir navegação por acessibilidade.
- Botões "Search", "Stop", "Preview" e "Apply" estão desativados sem indicador visual de tooltip explicativo; a razão só aparece no texto "Open a bench first." abaixo dos botões.
- As opções "Regex", "Match case" e "Whole word" são desenhadas como interruptores (toggles), mas expostas como CheckBox; a aparência de toggle pode não comunicar o estado marcado/desmarcado a usuários de leitor de tela.
