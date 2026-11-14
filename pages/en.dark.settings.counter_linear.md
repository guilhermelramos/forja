---
state: en.dark.settings.counter_linear
capture_sha: d4fc8ffa636db94a
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.dark.settings.counter_linear.png)

state: en.dark.settings.counter_linear
capture_sha: (não informado)
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.dark.settings.counter_linear.png)

# Settings — counter_linear (Linear)

Janela modal de ajustes da conexão "Linear" (alvo `counter_linear`), aberta sobre a tela principal da Forja em inglês, com campo de busca, campo de token e ações de conexão.

## Como chegar
No app: clique no botão "Settings" no fim do trilho de módulos à esquerda (retângulo (8,1004)–(44,1040)) e abra os ajustes da integração Linear (alvo `counter_linear`). Comando de captura: `forja-app --locale en --theme dark --settings counter_linear`.

## Regiões
Esta é a variante em inglês da página de referência (pt-BR.dark.settings.counter_linear); a estrutura é a mesma, mudando os textos da interface (em inglês) e as coordenadas, pois a captura foi feita em janela de 1920×1080 (a referência tem 1280×800), o que faz mais abas caberem nas faixas.

- **Barra de título superior** (0,0)–(1920,37): rótulo "No project", botões "Find file  Ctrl+P" e "Anvil  Ctrl+K" à esquerda; à direita, botão de tema (ícone de sol, sem nome acessível, em (1741,5)–(1773,37)) e os botões de janela "–" (minimizar), "□" (maximizar) e "×" (fechar).
- **Trilho de módulos à esquerda** (0,37)–(52,1048): ícone do módulo ativo no topo (imagem sem nome em (12,50)–(40,78)) e coluna de botões-ícone: "Board", "Helpers", "Routines", "Muster", "Requests", "Skills", "Usage", "Extensions", "Archive" e, no fim, "Settings".
- **Painel lateral esquerdo (Projects)** (52,37)–(304,1048): botão "More tabs", aba "Projects" (ativa, sublinhada em laranja); botões "Add folder" (fundo laranja), "New" e "Clone"; ícone de pasta (rótulo "(ícone)" em (168,161)–(196,189)) e a mensagem "No project yet. Add an existing folder or create a new one.".
- **Área central** (304,37)–(1612,1048): faixa de abas com "More tabs", "Terminal" (marcada), "Editor", "Browser", "Changes", "Preview", "Bench", "Review", "History", "Extension panels" e "Helper (chat)", encoberta em parte pela janela modal "Settings" (328,58)–(1026,307); barra de ferramentas com "+", quatro botões-ícone sem nome, "Recipes" e "Shell" (totalmente encobertos pelo modal). Abaixo do modal resta uma grande área de texto vazia com a legenda "Click to type" no canto inferior direito (~1565,1023).
- **Janela modal "Settings"** (328,58)–(1026,307): título "Settings" à esquerda com botão "Close window" (ícone "×") à direita; campo de busca "Search settings"; seção "Linear" com estado "Not connected", campo de senha "Token" e os botões "Connect" (desativado), "Test" e "Disconnect" (desativado).
- **Painel direito** (1628,37)–(1920,1048): faixa de abas com "More tabs", "Files" (ativa, sublinhada em laranja) e "Changes"; ícone de grade (rótulo "(ícone)" em (1756,117)–(1784,145)) e a mensagem "No active bench. Open a bench to see its files.".
- **Barra de status** (0,1048)–(1920,1080): "Ready" à esquerda (em verde), "56 modules" em seguida e "Notifications" à direita.

## Elementos
| Elemento | Tipo | Posição (px) | Estado | O que faz |
|---|---|---|---|---|
| "Find file  Ctrl+P" | Button | (99,5)–(218,37) | — | Abre a busca de arquivos (atalho Ctrl+P). |
| "Anvil  Ctrl+K" | Button | (226,5)–(327,37) | — | Abre a paleta de comandos (atalho Ctrl+K). |
| (sem nome acessível) | Unknown | (1741,5)–(1773,37) | — | Ícone de sol; alterna tema claro/escuro. |
| "–" | Button | (1795,5)–(1827,37) | — | Minimiza a janela. |
| "□" | Button | (1835,5)–(1867,37) | — | Maximiza/restaura a janela. |
| "×" | Button | (1875,5)–(1909,37) | — | Fecha a janela. |
| "No project" | Label | (11,12)–(77,31) | — | Indica o projeto atual na barra de título. |
| (sem nome acessível) | Unknown | (304,42)–(320,1048) | — | Divisor vertical entre o painel esquerdo e a área central. |
| (sem nome acessível) | Unknown | (1612,42)–(1628,1048) | — | Divisor vertical entre a área central e o painel direito. |
| (sem nome acessível) | Unknown | (319,49)–(339,69) | — | Elemento de borda à esquerda da faixa de abas central. |
| (sem nome acessível) | Unknown | (1016,49)–(1036,69) | — | Elemento de borda à direita da faixa de abas central. |
| (sem nome acessível) | Image | (12,50)–(40,78) | — | Ícone do módulo ativo no topo do trilho. |
| "More tabs" | Button | (64,50)–(151,85) | — | Revela abas ocultas do painel lateral esquerdo. |
| "More tabs" | Button | (324,50)–(411,85) | — | Revela abas ocultas da área central. |
| "More tabs" | Button | (1632,50)–(1719,85) | — | Revela abas ocultas do painel direito. |
| (sem nome acessível) | Unknown | (339,51)–(1016,67) | — | Faixa de fundo das abas centrais. |
| "Projects" | Button | (159,52)–(231,84) | marcado=True | Aba ativa do painel lateral; mostra a lista de projetos. |
| "Terminal" | Button | (419,52)–(496,84) | marcado=True | Aba da área central; encoberta em parte pelo modal. |
| "Editor" | Button | (496,52)–(556,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Browser" | Button | (556,52)–(629,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Changes" | Button | (629,52)–(706,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Preview" | Button | (706,52)–(777,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Bench" | Button | (777,52)–(838,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Review" | Button | (838,52)–(905,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "History" | Button | (905,52)–(973,84) | marcado=False | Aba da área central; encoberta pelo modal. |
| "Extension panels" | Button | (973,52)–(1099,84) | marcado=False | Aba da área central; parcialmente encoberta pelo modal (só "ion panels" aparece). |
| "Helper (chat)" | Button | (1099,52)–(1202,84) | marcado=False | Aba da área central; visível à direita do modal. |
| "Files" | Button | (1727,52)–(1778,84) | marcado=True | Aba ativa do painel direito; lista de arquivos (vazia). |
| "Changes" | Button | (1778,52)–(1855,84) | marcado=False | Aba do painel direito. |
| "Settings" | Window | (328,58)–(1026,307) | foco | Janela modal de ajustes do alvo `counter_linear`. |
| "Close window" | Button | (985,67)–(1017,99) | — | Fecha a janela modal; ícone "×". |
| (sem nome acessível) | Unknown | (321,69)–(337,297) | — | Borda esquerda do modal (redimensionamento). |
| (sem nome acessível) | Unknown | (1018,69)–(1034,297) | — | Borda direita do modal (redimensionamento). |
| "Settings" | Label | (337,71)–(403,95) | — | Título da janela modal, alinhado à esquerda. |
| "Board" | Button | (8,94)–(44,130) | marcado=False | Botão-ícone do trilho esquerdo. |
| "Add folder" | Button | (64,101)–(154,133) | — | Abre o seletor para adicionar uma pasta de projeto; destaque laranja. |
| "New" | Button | (162,101)–(213,133) | — | Cria um projeto novo. |
| "Clone" | Button | (221,101)–(280,133) | — | Clona um projeto (sem projetos, sem efeito prático). |
| "+" | Button | (407,113)–(441,145) | — | Botão de adicionar da barra da área central; encoberto pelo modal. |
| (sem nome acessível) | Unknown | (444,113)–(476,145) | — | Botão-ícone da barra central, sem nome; encoberto pelo modal. |
| (sem nome acessível) | Unknown | (479,113)–(511,145) | — | Botão-ícone da barra central, sem nome; encoberto pelo modal. |
| (sem nome acessível) | Unknown | (514,113)–(546,145) | — | Botão-ícone da barra central, sem nome; encoberto pelo modal. |
| (sem nome acessível) | Unknown | (549,113)–(581,145) | — | Botão-ícone da barra central, sem nome; encoberto pelo modal. |
| "Recipes" | Button | (584,113)–(655,145) | — | Abre receitas; encoberto pelo modal. |
| "Shell" | Button | (658,113)–(711,145) | — | Abre o shell; encoberto pelo modal. |
| "(ícone)" | Label | (1756,117)–(1784,145) | — | Ícone de grade do painel direito (estado vazio). |
| (sem nome acessível) | Unknown | (328,118)–(404,140) | — | Elemento no canto superior esquerdo do modal. |
| (sem nome acessível) | TextInput | (337,121)–(1017,153) | — | Campo de busca de ajustes; texto de sugestão "Search settings". |
| (sem nome acessível) | Unknown | (385,121)–(401,137) | — | Elemento interno do campo de busca (lente/ícone). |
| "Helpers" | Button | (8,138)–(44,174) | marcado=False | Botão-ícone do trilho esquerdo. |
| (sem nome acessível) | Unknown | (328,150)–(1604,1036) | — | Área de texto central vazia; exibe "Click to type" no canto inferior direito. |
| "No active bench. Open a bench to see its files." | Label | (1644,153)–(1896,191) | — | Mensagem de estado vazio do painel de arquivos. |
| "Linear" | Label | (337,181)–(374,200) | — | Título da seção de ajustes da conexão Linear. |
| "Routines" | Button | (8,182)–(44,218) | marcado=False | Botão-ícone do trilho esquerdo. |
| "No project yet. Add an existing folder or create a new one." | Label | (67,197)–(297,235) | — | Mensagem de estado vazio do painel de projetos. |
| "Not connected" | Label | (337,208)–(428,227) | — | Estado atual da conexão Linear. |
| "Muster" | Button | (8,226)–(44,262) | marcado=False | Botão-ícone do trilho esquerdo. |
| (sem nome acessível) | PasswordInput | (337,235)–(1017,258) | — | Campo de senha para o token da conexão Linear; texto de sugestão "Token". |
| "Connect" | Button | (337,266)–(412,298) | desativado | Conecta à Linear; inativo até haver token. |
| "Test" | Button | (420,266)–(469,298) | — | Testa as credenciais informadas. |
| "Disconnect" | Button | (477,266)–(568,298) | desativado | Encerra a conexão; inativo por não haver conexão. |
| "Requests" | Button | (8,270)–(44,306) | marcado=False | Botão-ícone do trilho esquerdo. |
| (sem nome acessível) | Unknown | (319,297)–(339,317) | — | Alça inferior esquerda do modal. |
| (sem nome acessível) | Unknown | (1016,297)–(1036,317) | — | Alça inferior direita do modal. |
| (sem nome acessível) | Unknown | (339,299)–(1016,315) | — | Borda inferior do modal. |
| "Skills" | Button | (8,314)–(44,350) | marcado=False | Botão-ícone do trilho esquerdo. |
| "Usage" | Button | (8,358)–(44,394) | marcado=False | Botão-ícone do trilho esquerdo. |
| "Extensions" | Button | (8,402)–(44,438) | marcado=False | Botão-ícone do trilho esquerdo. |
| "Archive" | Button | (8,446)–(44,482) | marcado=False | Botão-ícone do trilho esquerdo. |
| "Settings" | Button | (8,1004)–(44,1040) | marcado=False | Botão-ícone do trilho esquerdo; abre a tela de ajustes. |
| "Ready" | Label | (10,1055)–(48,1074) | — | Status geral na barra de status, em verde. |
| "56 modules" | Label | (70,1055)–(142,1074) | — | Contagem de módulos carregados. |
| "Notifications" | Label | (1808,1055)–(1886,1074) | — | Área de notificações na barra de status. |

## Aparência
Igual à referência pt-BR em estrutura visual; mudam apenas os textos, agora em inglês ("Search settings", "Not connected", "Token", "Connect", "Test", "Disconnect", "No project yet…", "No active bench…", "Click to type", "Ready", "56 modules", "Notifications"). Tema escuro: fundo geral quase preto (~#1b1b1b), painéis e barras em cinza-escuro, e a janela modal "Settings" em cinza um pouco mais claro. A cor de destaque é o laranja: sublinhado e texto das abas ativas "Projects" e "Files", fundo do botão "Add folder" e rótulo do botão desativado "Connect"; o rótulo "Ready" aparece em verde na barra de status. Tipografia sans-serif de ~13–14 px. Botões com cantos levemente arredondados e espaçamento uniforme (~8 px). Campos de entrada ocupam toda a largura do modal (337–1017 px) e são empilhados com espaçamento regular. Estados vazios usam ícone centralizado e texto cinza. Como a janela tem 1920×1080, as faixas de abas mostram mais abas nomeadas do que na referência (que usa "More tabs" com mais frequência).

## Problemas encontrados
- A aba "Extension panels" (973,52)–(1099,84) é parcialmente encoberta pelo modal (o modal vai até x=1026); na imagem só se lê "ion panels".
- Os botões "+", os quatro botões-ícone entre 444–581 px, "Recipes" e "Shell" (y 113–145) ficam totalmente encobertos pelo modal, sem indicação visual de sua existência.
- Sobreposição na árvore de acessibilidade: "Helper (chat)" (1099,52)–(1202,84) se sobrepõe à faixa de abas que inclui "Extension panels".
- Três abas aparecem marcadas simultaneamente ("Projects" no painel esquerdo, "Terminal" na área central e "Files" no painel direito), o que dificulta identificar o painel realmente ativo.
- O TextInput de busca e o PasswordInput de token não têm nome acessível; dependem apenas do texto de sugestão ("Search settings", "Token").
- Vários elementos sem nome acessível: botão de tema na barra de título, bordas e alças do modal, bordas das faixas de abas e os quatro botões-ícone da barra central.
- O texto "Click to type" (~1565,1023) é exibido em cinza escuro sobre fundo escuro, com contraste baixo.
- Nenhum elemento ultrapassa os limites da janela (1920×1080); na variante pt-BR havia uma aba fora da borda, o que não ocorre aqui por causa da largura maior.
