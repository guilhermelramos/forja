---
state: en.dark.rail.skill.page
capture_sha: 5d702bd613672467
model: opencode/glm-5.3-flash-free
---

![Captura](../captures/en.dark.rail.skill.page.png)

# Skills (página central)

Tela de gerenciamento de skills (pastas com `SKILL.md`): permite instalar, criar e buscar skills da Forja e dos ajudantes, enquanto o restante da janela mostra projetos e arquivos. Variante em inglês (`en`) do tema escuro; a referência estrutural é a página pt-BR.

## Como chegar
Na barra vertical à esquerda, clique no ícone de skills (o sexto botão do trilho, marcado em laranja). Equivalente a abrir o app com `forja-app --locale en --theme dark --rail skill.page`.

## Regiões
Janela de 1920×1080 px. Em relação à referência pt-BR (1280×800), a janela é maior e as regiões escalam proporcionalmente; a estrutura é a mesma, com duas diferenças: as barras de abas dos painéis laterais aparecem recolhidas no botão "More tabs" (só a aba ativa fica visível ao lado dele) e há um botão "Back to bench" no topo da área central, ausente na referência.

- **Barra superior** (0,0)–(1920,42): rótulo do projeto atual ("No project"), botões "Find file  Ctrl+P" e "Anvil  Ctrl+K", ícone de tema e botões de janela (minimizar "–", maximizar "□", fechar "×").
- **Trilho à esquerda** (0,42)–(52,1048): logotipo da Forja no topo e ícones de navegação (Board, Helpers, Routines, Muster, Requests, Skills, Usage, Extensions, Archive), com "Settings" fixado no rodapé do trilho.
- **Painel lateral esquerdo** (52,42)–(304,1048): botão "More tabs" e a aba ativa "Projects"; ações "Add folder", "New" e "Clone"; estado vazio com ícone de pasta e o texto "No project yet. Add an existing folder or create a new one.".
- **Área central** (320,42)–(1612,1048): página "Skills", com botão "Back to bench", descrição, ações de instalação, seletor de destino, busca ("Search skills") e duas mensagens de estado vazio ("No skill found." e "Choose a skill to see its SKILL.md.").
- **Painel lateral direito** (1628,42)–(1920,1048): botão "More tabs", abas "Files" (ativa) e "Changes"; ícone de grade e texto "No active bench. Open a bench to see its files.".
- **Barra de status** (0,1048)–(1920,1080): status "Ready", contagem "56 modules" e "Notifications" à direita.

## Elementos
| Elemento | Tipo | Posição (px) | Estado | O que faz |
|---|---|---|---|---|
| "Find file  Ctrl+P" | Botão | (99,5)–(218,37) | — | Abre a busca rápida de arquivos (atalho Ctrl+P) |
| "Anvil  Ctrl+K" | Botão | (226,5)–(327,37) | — | Abre a paleta de comandos (atalho Ctrl+K); nesta localização a paleta se chama "Anvil" |
| (sem nome acessível) | Ícone | (1741,5)–(1773,37) | — | Ícone de sol na barra superior; alterna o tema |
| "–" | Botão | (1795,5)–(1827,37) | — | Minimiza a janela |
| "□" | Botão | (1835,5)–(1867,37) | — | Maximiza/restaura a janela |
| "×" | Botão | (1875,5)–(1909,37) | — | Fecha a janela |
| "No project" | Rótulo | (11,12)–(77,31) | — | Mostra o projeto ativo na barra superior |
| (sem nome acessível) | Divisor | (304,42)–(320,1048) | — | Separador vertical entre o painel esquerdo e a área central |
| (sem nome acessível) | Divisor | (1612,42)–(1628,1048) | — | Separador vertical entre a área central e o painel direito |
| (sem nome acessível) | Imagem | (12,50)–(40,78) | — | Logotipo da Forja no topo do trilho |
| "More tabs" | Botão | (64,50)–(151,85) | — | Abre as demais abas do painel esquerdo (recolhidas) |
| "Back to bench" | Botão | (324,50)–(435,82) | — | Volta da página de skills para a página da bancada |
| "More tabs" | Botão | (1632,50)–(1719,85) | — | Abre as demais abas do painel direito (recolhidas) |
| "Projects" | Botão | (159,52)–(231,84) | marcado=True | Aba ativa do painel esquerdo; mostra a lista de projetos |
| "Files" | Botão | (1727,52)–(1778,84) | marcado=True | Aba ativa do painel direito; lista de arquivos |
| "Changes" | Botão | (1778,52)–(1855,84) | marcado=False | Aba de mudanças (versionamento) |
| "Board" | Botão | (8,94)–(44,130) | marcado=False | Ícone do trilho para o quadro |
| "Skills" | Rótulo | (324,90)–(363,114) | — | Título da página central |
| "Skills are folders with a SKILL.md. Forja's own skills and each helper's folders appear here." | Rótulo | (324,122)–(794,138) | — | Descrição da página |
| "Add folder" | Botão | (64,101)–(154,133) | — | Adiciona uma pasta de projeto existente |
| "New" | Botão | (162,101)–(213,133) | — | Cria um projeto novo |
| "Clone" | Botão | (221,101)–(280,133) | — | Clona um projeto |
| "Install folder" | Botão | (324,146)–(424,178) | — | Instala uma skill a partir de uma pasta (botão destacado em laranja) |
| "Install zip" | Botão | (432,146)–(513,178) | — | Instala uma skill a partir de um arquivo zip |
| "Refresh" | Botão | (521,146)–(591,178) | — | Atualiza a lista de skills |
| (ícone) | Rótulo | (1756,117)–(1784,145) | — | Ícone de grade do estado vazio do painel de arquivos |
| "Helpers" | Botão | (8,138)–(44,174) | marcado=False | Ícone do trilho para ajudantes |
| "No active bench. Open a bench to see its files." | Rótulo | (1644,153)–(1896,191) | — | Estado vazio do painel de arquivos |
| (ícone) | Rótulo | (168,161)–(196,189) | — | Ícone de pasta do estado vazio do painel de projetos |
| "Routines" | Botão | (8,182)–(44,218) | marcado=False | Ícone do trilho para rotinas |
| "Install to" | Rótulo | (324,186)–(376,218) | — | Rótulo do seletor de destino da instalação |
| "Forja" | Caixa de seleção | (384,186)–(512,218) | — | Combobox que escolhe o escopo de instalação; mostra "Forja" |
| "User" | Botão | (520,186)–(587,218) | marcado=True | Escopo de instalação no usuário (selecionado, laranja) |
| "Project" | Botão | (587,186)–(654,218) | marcado=False | Escopo de instalação no projeto |
| "Muster" | Botão | (8,226)–(44,262) | marcado=False | Ícone do trilho para o mutirão |
| "Open a bench to see the project's skills." | Rótulo | (324,226)–(567,245) | — | Aviso de que as skills de projeto dependem de uma bancada |
| (sem nome acessível) | Campo de texto | (324,267)–(962,299) | — | Campo de busca com o texto de sugestão "Search skills" |
| "Requests" | Botão | (8,270)–(44,306) | marcado=False | Ícone do trilho para pedidos |
| (ícone) | Rótulo | (1275,283)–(1303,311) | — | Ícone de documento do estado vazio direito da área central |
| "Skills" | Botão | (8,314)–(44,350) | marcado=True | Ícone do trilho da página atual (destacado em laranja) |
| "Choose a skill to see its SKILL.md." | Rótulo | (1187,319)–(1392,338) | — | Estado vazio do painel de detalhe da skill |
| (ícone) | Rótulo | (629,323)–(657,351) | — | Ícone de documento do estado vazio esquerdo da área central |
| "Usage" | Botão | (8,358)–(44,394) | marcado=False | Ícone do trilho para uso |
| "No skill found." | Rótulo | (599,359)–(688,378) | — | Estado vazio da lista de skills |
| "Extensions" | Botão | (8,402)–(44,438) | marcado=False | Ícone do trilho para extensões |
| "Archive" | Botão | (8,446)–(44,482) | marcado=False | Ícone do trilho para o acervo |
| "Settings" | Botão | (8,1004)–(44,1040) | marcado=False | Ícone do trilho para ajustes, fixado no rodapé |
| "Ready" | Rótulo | (10,1055)–(48,1074) | — | Status atual na barra inferior |
| "56 modules" | Rótulo | (70,1055)–(142,1074) | — | Contagem de módulos carregados |
| "Notifications" | Rótulo | (1808,1055)–(1886,1074) | — | Abre as notificações na barra de status |

## Aparência
Idêntica à referência pt-BR em cores e tipografia: tema escuro com fundo quase preto e leve tom marrom nas três colunas, separadas por divisores verticais finos e discretos. A cor de destaque é o laranja, usado nos botões "Install folder" e "User" e no ícone ativo "Skills" do trilho; a aba ativa de cada painel ("Projects", "Files") fica marcada ao lado do botão "More tabs". Tipografia sans-serif de corpo pequeno (rótulos em torno de 13–16 px), cinza-claro sobre fundo escuro, com o título "Skills" em tamanho maior. Ícones de estado vazio são contornos cinza centralizados com a mensagem abaixo. Botões secundários têm borda fina arredondada e fundo levemente mais claro que o painel; o espaçamento entre ações é uniforme (cerca de 8–16 px). Tudo o que é texto aparece em inglês, conforme o idioma `en` desta variante.

## Problemas encontrados
- O campo de busca "Search skills" é um TextInput sem nome acessível; o texto de sugestão só aparece visualmente.
- O ícone de alternância de tema na barra superior (1741,5)–(1773,37) é um elemento clicável sem nome acessível; a função só é identificável pela imagem.
- Os dois botões de abas recolhidas têm o mesmo nome acessível ("More tabs"), um em cada painel; leitores de tela não distinguem qual painel cada um controla.
- Os botões "Files" (1727,52)–(1778,84) e "Changes" (1778,52)–(1855,84) têm retângulos que se tocam em x=1778, sem folga entre eles.
- Os divisores verticais são elementos sem nome acessível.
- Os três ícones de estado vazio aparecem como rótulos "(ícone)" sem descrição de texto alternativo.
- Nenhum texto cortado ou fora da janela: todos os retângulos estão dentro de 1920×1080 px.
