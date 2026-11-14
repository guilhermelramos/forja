# Problemas encontrados pelo atlas

Juntado das seções "Problemas encontrados" de cada página. Gerado automaticamente.

## [pt-BR.dark.home.default](pages/pt-BR.dark.home.default.md)

- A aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) ultrapassa a borda do painel (x≈304) e o texto aparece cortado ("Painéis de e…") na imagem.
- A aba "Visualizador" (641,50)–(739,82) passa do limite da área central (x≈724) e o texto aparece cortado ("Visualizado…") na imagem.
- A faixa de abas do painel direito transborda a largura da janela: "Painéis de extensões" (1022,50)–(1173,82) e "Ajudante (chat)" (1115,50)–(1234,82) têm retângulos além da largura da imagem (1024 px) e ficam inacessíveis visualmente.
- Retângulos sobrepostos na árvore do painel direito: "Bancada" (739,50)–(815,82) sobrepõe "Arquivos", "Revisão" (815,50)–(885,82) sobrepõe "Mudanças" (814,50)–(901,82) e "Histórico" (885,50)–(965,82) sobrepõe "Busca" (901,50)–(960,82); na imagem apenas "Arquivos", "Mudanças", "Busca" e "Portas" são visíveis, indicando medições inconsistentes da faixa de abas.
- Botões sem nome acessível (acessibilidade): o alternador de tema (845,5)–(877,37), os quatro ícones da barra do terminal (444,102)–(581,134) e a pílula de sessão "pwsh" (328,107)–(404,129) não expõem nome.
- Os botões de ícone do terminal e o alternador de tema dependem de reconhecimento visual, sem rótulo ou tooltip acessível na árvore.
- Estados com pixels idênticos a esta captura incluem modais que deveriam alterar a imagem (pt-BR.dark.modal.bench.dialogs, pt-BR.dark.modal.mcp.approval, pt-BR.dark.modal.notify.toasts, pt-BR.dark.modal.onboarding.wizard, pt-BR.dark.modal.voice.overlay e pt-BR.dark.modal.counter.poller): nessas capturas nenhum diálogo, aviso ou sobreposição aparece renderizado.
- O texto "Clique para digitar" na área do terminal (canto inferior direito, ~(650,705)–(716,716)) não corresponde a nenhum elemento da árvore de acessibilidade.

## [pt-BR.dark.open.anvil](pages/pt-BR.dark.open.anvil.md)

- Vários elementos têm retângulos além dos limites da captura (1280×800): a aba "Painéis de extensões" do painel direito vai até x=1429, e o conteúdo da Bigorna (linhas de receita e lista) se estende até y=1812 — a lista de receitas e as abas finais são cortadas na imagem.
- Sobreposição de retângulos entre abas: "Ajudante (chat)" (1115–1234) sobrepõe "Busca" (1157–1216), e a aba "Painéis de extensões" do painel esquerdo (220–371) sobrepõe a aba central "Terminal" (324–401).
- Botões "Receitas", "Shell" e "+" (y≈102–134) ficam totalmente ocultos atrás da sobreposição da Bigorna, sobreposição de camadas sem indicação visual.
- A dica "Enter executa · Ctrl+Enter mantém a Bigorna aberta · Esc fecha" (330,797)–(660,813) fica fora do retângulo da janela da Bigorna (que termina em y=792) e invade a faixa da barra de status.
- Elementos interativos sem nome acessível: o botão de alternar tema (ícone de sol, (1101,5)–(1133,37)), o campo de busca da Bigorna, as dez linhas de resultado da busca, os quatro ícones de filtro da área central e os divisores dos painéis.
- As linhas de resultado da Bigorna não expõem o texto visível como nome acessível (só aparência na imagem), o que prejudica leitores de tela.
- Abas com texto cortado na renderização: "Painéis de e..." no painel esquerdo e as abas centrais/direitas truncadas pelas bordas dos painéis.
- As barras de rolagem das listas têm apenas 2 px de largura ((948,220)–(950,580) e (597,629)–(599,783)), muito finas e de baixo contraste.

## [pt-BR.dark.open.onboarding](pages/pt-BR.dark.open.onboarding.md)

- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), estendendo-se 149 px além da largura da janela (1280 px): o texto fica cortado/fora da tela.
- As faixas de abas se sobrepõem: a aba "Painéis de extensões" da área central (965,50)–(1115,82) cobre a região das abas do painel direito ("Arquivos", "Mudanças"), e "Ajudante (chat)" (1115,50)–(1234,82) se sobrepõe a "Mudanças" (1070,50)–(1157,82) e "Busca" (1157,50)–(1216,82); há também sobreposição entre "Painéis de extensões" do painel esquerdo (220,50)–(371,82) e "Terminal" (324,50)–(401,82).
- O texto da aba "Painéis de extensões" do painel esquerdo aparece truncado na imagem ("Painéis de e") porque a faixa de abas é interrompida pela área central.
- A caixa de seleção de idioma ("Seguir o Windows") e seu elemento associado em (640,400)–(640,400) estão sem nome acessível; o segundo tem tamanho zero.
- Os quatro botões de ícone da barra do terminal (dividir, encerrar, limpar, buscar) e a aba de sessão "pwsh" estão sem nome acessível.
- O botão de alternar tema (ícone de sol, (1101,5)–(1133,37)) está sem nome acessível.

## [pt-BR.dark.open.toast](pages/pt-BR.dark.open.toast.md)

- A terceira aba do painel esquerdo, "Painéis de extensões", tem texto cortado na borda do painel ("Painéis de e…" visível), embora seu retângulo (220,50)–(371,82) ultrapasse a largura visível do painel (~310 px).
- A aba "Painéis de extensões" da área central tem retângulo (965,50)–(1115,82) que invade o painel direito; na imagem só aparece uma barra vertical estreita junto ao divisor.
- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82) totalmente fora da largura da imagem (1280 px); não é visível.
- O retângulo do botão "Ajudante (chat)" (1115,50)–(1234,82) se sobrepõe às abas "Mudanças", "Busca" e "Portas"; o botão não aparece na imagem.
- Cinco botões de ícone da barra do terminal (444,102)–(581,134) e o botão de tema (1101,5)–(1133,37) não têm nome acessível.
- Há dois botões nomeados apenas "×" (fechar janela em (1235,5) e fechar toast em (1230,703)), o que os torna ambíguos para leitores de tela.
- O texto "Clique para digitar", visível na área do terminal, não aparece na árvore de acessibilidade.
- O contador "1 avisos" está no plural para valor singular (deveria ser "1 aviso").
- Elemento de dimensão zero (640,400)–(640,400) sem nome na área central.

## [pt-BR.dark.rail.archive.tab](pages/pt-BR.dark.rail.archive.tab.md)

- A aba "Painéis de extensões" do painel esquerdo aparece truncada como "Painéis de e" e seu retângulo (220,50)–(371,82) se sobrepõe ao rótulo "Acervo" (324,50)–(379,74).
- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), ultrapassando a largura da janela (1280 px); o texto é cortado fora da tela.
- Vários elementos sem nome acessível: os dois divisores verticais, os contêineres das linhas de abas, o alternador de tema da barra superior, os dois campos de filtro (TextInput) e os dois seletores de valores ("todos os ajudantes" e "qualquer data"), o que dificulta a navegação por leitor de telas.
- Os botões de janela usam apenas os símbolos "–", "□" e "×" como nome acessível, sem rótulo descritivo.
- A barra de abas do painel direito não cabe na largura disponível (cinco abas se estendem além da borda), indicando estouro de layout.
- Os contêineres "Unknown" das linhas de abas em (64,80)–(300,82) e (992,80)–(1268,82) têm apenas 2 px de altura, sugerindo retângulos residuais possivelmente decorrentes de truncamento.

## [pt-BR.dark.bench-tab.bench.detail](pages/pt-BR.dark.bench-tab.bench.detail.md)

- A área central tem largura efetiva de ~396 px, mas a mensagem "Nenhuma bancada ativa.\nCrie uma bancada ou escolha uma na lateral ou no Quadro." ocupa (596,182)–(962,220): o texto invade a divisória do painel direito e é truncado visualmente ("Nen...", "Crie uma bancada ou...").
- Na fileira de abas do painel direito, as abas "Bancada", "Revisão", "Histórico" e "Painéis de extensões" ficam empilhadas com retângulos sobrepostos às abas visíveis ("Arquivos", "Mudanças", "Busca") e não aparecem na imagem.
- Duas ocorrências de "Painéis de extensões" (965–1115 e 1022–1173) e "Ajudante (chat)" (1115–1234) têm retângulos além da largura da imagem (1024 px): ficam fora da janela, parcial ou totalmente.
- A aba "Painéis de extensões" do painel de projetos se estende até x=371, além da divisória do painel (x=304); o texto aparece cortado ("Painéis de e...").
- O botão de tema na barra superior (845,5)–(877,37) é um Unknown sem nome acessível; só é identificável pelo ícone de sol.
- O botão "Nova bancada" começa em x=723, invadindo a faixa da divisória (716,42)–(732,736).
- A aba "Bancada" do painel direito aparece com marcado=True ao mesmo tempo que "Arquivos", sugerindo estado de seleção ambíguo.
- Vários elementos da árvore estão sem nome acessível (divisórias, barras de abas, separadores, ícones), o que reduz a navegabilidade por leitor de tela.

## [pt-BR.dark.rail.board.page](pages/pt-BR.dark.rail.board.page.md)

- O botão "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), estendendo-se além da largura da janela (1280 px); a aba fica cortada.
- O botão "Painéis de extensões" do painel esquerdo tem retângulo (220,50)–(371,82), que cruza o divisor em x=320, e seu texto aparece truncado na borda do painel ("Painéis de e").
- O cabeçalho "Pronto" da quarta coluna é cortado pela borda direita da área central em x=972.
- O Label "(ícone)" em (632,792)–(660,820) invade a barra de status e ultrapassa a altura da janela (800 px).
- A mensagem "Nenhuma bancada neste quadro ainda. Crie uma para começar." (450,828)–(842,847) está inteiramente abaixo da área visível e não aparece na tela.
- O campo de filtro de bancadas (TextInput em (324,90)–(564,122)) não tem nome acessível; o rótulo visível é só a marca d'água "Filtrar bancadas".
- Elementos sem nome acessível além do esperado: divisores verticais, faixas de abas, área do quadro, barra de rolagem horizontal e ícone de tema da barra superior.

## [pt-BR.dark.bench-tab.browser.tab](pages/pt-BR.dark.bench-tab.browser.tab.md)

- Vários retângulos vão além da largura da janela (1024 px) e ficam cortados: "Painéis de extensões" (965,50)–(1115,82), "Painéis de extensões" (1022,50)–(1173,82), "Ajudante (chat)" (1115,50)–(1234,82), "Nova aba" (1011,98)–(1093,130) e o rótulo "100%" (1100,98)–(1128,130).
- O TextInput "forja://inicio" declara o retângulo (448,103)–(876,126), que atravessa o divisor (716,42)–(732,736) e invade o painel direito; na imagem o campo termina antes do divisor — coordenadas desatualizadas. "Ir" (884,98)–(916,130) e "Histórico" (924,98)–(1003,130) também ficam fora do painel central na árvore.
- O rótulo "(ícone)" (860,114)–(888,142) sobrepõe o retângulo do campo de endereço (448,103)–(876,126).
- Retângulos sobrepostos entre abas de grupos diferentes: "Terminal" (324,50)–(401,82) dentro de "Painéis de extensões" (220,50)–(371,82); "Arquivos" (736,50)–(814,82) sobrepõe "Bancada" (739,50)–(815,82); "Mudanças" (814,50)–(901,82) sobrepõe "Revisão" (815,50)–(885,82), que sobrepõe "Histórico" (885,50)–(965,82) e "Busca" (901,50)–(960,82).
- A aba "Painéis de extensões" do painel esquerdo aparece truncada na imagem ("Painéis de e…") junto ao divisor (304,42)–(320,736).
- Botões de janela nomeados apenas por símbolo: "–", "□" e "×", sem nomes acessíveis descritivos (Minimizar/Maximizar/Fechar).
- Elementos sem nome acessível: alternador de tema (845,5)–(877,37), os dois divisores verticais, as faixas de abas (64,50)–(300,82) e (736,50)–(1012,82), as linhas (64,80)–(300,82) e (736,80)–(1012,82) e os dois rótulos "(ícone)".
- Botão "R" da barra de navegação tem nome não descritivo (não indica "Recarregar").
- O nome "Painéis de extensões" aparece três vezes na árvore sem diferenciação entre as abas.
- Baixo contraste nos textos de estado vazio ("Nenhum projeto ainda…", "Nenhuma bancada ativa…") e nas abas inativas, em cinza escuro sobre fundo escuro.

## [pt-BR.dark.bench-tab.chat.tab](pages/pt-BR.dark.bench-tab.chat.tab.md)

- O grupo de abas do cabeçalho da bancada estoura a largura da janela (≈1024 px): "Bancada", "Revisão", "Histórico", duas ocorrências de "Painéis de extensões" e a aba selecionada "Ajudante (chat)" (1115–1234) ficam fora da área visível.
- A aba ativa "Ajudante (chat)" (marcado=True) não aparece na tela; não é possível ver qual aba do grupo direito está selecionada.
- Dois conjuntos de abas ocupam a mesma faixa e se sobrepõem na árvore: "Arquivos" (736–814) sobre "Bancada" (739–815), "Mudanças" (814–901) sobre "Revisão" (815–885) e "Busca" (901–960) sobre "Histórico" (885–965).
- "Painéis de extensões" aparece três vezes, sendo duas sobrepostas (965–1115 e 1022–1173).
- A aba "Painéis de extensões" do grupo esquerdo (220–371) passa do divisor em x≈304 e o texto aparece truncado ("Painéis de e").
- O ComboBox "Esforço" (1030–1140), o botão "Terminal" (1148–1226), o botão "Enviar" (1165–1226) e o campo multilinha (até x=1226) também ultrapassam a borda direita da janela; "Enviar" não é visível.
- Campo de texto sem nome acessível exibe o texto em inglês "ANTHROPIC_MODEL" (não traduzido para pt-BR).
- TextInput e MultilineTextInput sem nome acessível.
- Botão de tema (sol, 845–877) e os rótulos de ícone em (860,114) e (168,198) sem nome acessível.
- O rótulo "Claude Code · D:\a\forja\forja ·" termina com um separador "·" solto.

## [pt-BR.dark.bench-tab.diff.tab](pages/pt-BR.dark.bench-tab.diff.tab.md)

- Vários elementos têm retângulos além da largura da janela (1053 px) e aparecem cortados: "Painéis de extensões" (965–1115) e (1022–1173), "Ajudante (chat)" (1115–1234) na faixa de abas do painel direito; "Mudança anterior" (951–1083) e "Próxima mudança" (1091–1226) na barra do diff.
- Sobreposição de abas no painel direito: "Bancada" (739–815) sobre "Arquivos" (736–814), "Revisão" (815–885) sobre "Mudanças" (814–901) e "Busca" (901–960), "Painéis de extensões" (965–1115) sobre "Histórico" (885–965) e "Portas" (960–1022); "Arquivos" aparece duas vezes com estados conflitantes (marcado=True em 736–814 e marcado=False em 739–815).
- Texto cortado pelo divisor central/direito: "Ignorar espaço" (629–738) aparece como "Ignorar espa…"; "Recolher iguais" (746–857) fica inteiramente depois do divisor e não é visível; "Escolha um arquivo para ver o diff." (566–779) é cortado no meio.
- Aba "Painéis de extensões" do painel esquerdo (220–371) é cortada pela borda do painel em x≈310, exibindo "Painéis de e…".
- Botão da barra superior sem nome acessível em (845,5)–(877,37) (ícone de sol, alternador de tema); divisores, faixas de abas, sublinhados, logo e ícones de estado vazio também não têm nome acessível.
- Mensagens de estado vazio em cinza itálico ("Escolha um arquivo para ver o diff.") têm contraste baixo sobre o fundo escuro; "Limpar" desativado também apresenta contraste reduzido (esperado para estado desabilitado).

## [pt-BR.dark.bench-tab.editor.tab](pages/pt-BR.dark.bench-tab.editor.tab.md)

- Sobreposição de abas: no grupo central, "Painéis de extensões" do painel esquerdo (220,50)–(371,82) invade a faixa do grupo central e passa por baixo de "Terminal" (324,50)–(401,82).
- Grupo de abas direito com duas fileiras sobrepostas: "Arquivos" (736,50)–(814,82) sobre "Bancada" (739,50)–(815,82), "Mudanças" (814,50)–(901,82) sobre "Revisão" (815,50)–(885,82), "Histórico" (885,50)–(965,82) sobre "Busca" (901,50)–(960,82) e "Portas" (960,50)–(1022,82) sobre "Painéis de extensões" (965,50)–(1115,82).
- Elementos fora da janela: "Painéis de extensões" (1022,50)–(1173,82) e "Ajudante (chat)" (1115,50)–(1234,82) estendem-se além da largura de 1024 px e não aparecem na imagem.
- Botões sem nome acessível: o ícone de tema na barra superior (845,5)–(877,37) é um Unknown clicável sem rótulo.
- Ícones/imagens sem alternativa textual: logotipo (12,50)–(40,78) e os ícones de estado vazio (860,114) e (168,198) aparecem como "(sem nome acessível)".
- Divisores verticais (304,42)–(320,736) e (716,42)–(732,736) são Unknown sem nome acessível, o que dificulta identificação por leitor de tela.
- Textos de estado vazio em cinza sobre fundo escuro têm contraste baixo ("Abra um arquivo no explorador ou pressione Ctrl+P.").

## [pt-BR.dark.rail.extension.page](pages/pt-BR.dark.rail.extension.page.md)

- A aba "Painéis de extensões" do painel lateral esquerdo (botão em (220,50)–(371,82)) é cortada pelo limite do painel em ~x=310: na imagem aparece só "Painéis de e", embora o retângulo acessível vá até x=371.
- A aba "Painéis de extensões" do painel direito (botão em (1278,50)–(1429,82)) tem retângulo além da largura da janela (1280 px); o rótulo fica fora da área visível e inacessível.
- O botão de alternar tema na barra superior (1101,5)–(1133,37) não tem nome acessível; só se identifica pela imagem do sol.
- O campo de busca de extensões (324,146)–(968,178) é um TextInput sem nome acessível; o rótulo "Buscar" fica fora do campo, sem associação.
- O logotipo do app (imagem em 12,50) e os dois ícones de estado vazio (168,198) e (632,202) estão sem nome acessível.
- A instrução "Coloque uma pasta com extension.toml e init.lua em ~/.forja/extensions." é exposta com papel de Botão; semanticamente é um texto de ajuda, o que pode confundir leitores de tela.

## [pt-BR.dark.right-tab.extension.panels.right](pages/pt-BR.dark.right-tab.extension.panels.right.md)

- A aba ativa "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), que ultrapassa a largura da janela (1280 px): a aba marcada como ativa está fora da área visível.
- Sobreposição na faixa de abas do painel direito: "Ajudante (chat)" (1115–1234) se sobrepõe a "Mudanças" (1070–1157), "Busca" (1157–1216) e "Portas" (1216–1278); na imagem apenas quatro abas são visíveis e "Ajudante (chat)" fica oculta.
- A aba "Painéis de extensões" da área central (965–1115) cruza o divisor (972–988) e invade a faixa de abas do painel direito, sobrepondo-se a "Arquivos" (992–1070); na imagem ela aparece cortada na borda do painel central.
- A aba "Painéis de extensões" do painel esquerdo (220–371) ultrapassa a borda do painel (x=300), invade o divisor e a área central (sobrepondo "Terminal", 324–371); na imagem o texto aparece truncado como "Painéis de e".
- Três faixas de abas coexistem na mesma linha (y 50–82) com retângulos intercalados ("Projetos"/"Bancadas"/"Painéis de extensões" até x=371, "Terminal" a partir de x=324), indicando faixas de abas de painéis diferentes invadindo umas às outras.
- Botão de tema na barra superior (1101,5)–(1133,37) sem nome acessível; mesmo problema nos ícones de fechar/dividir/maximizar/busca do terminal e no divisor de painéis.
- Nome acessível "(ícone)" no rótulo do ícone de pasta (168,198)–(196,226) não descreve o elemento.
- Botões de janela nomeados apenas com os símbolos "–", "□" e "×", sem rótulos descritivos.
- Estado inconsistente com o alvo da captura: a captura declara o slot direito "Painéis de extensões" como alvo, mas a aba correspondente está invisível, enquanto "Projetos" (esquerda) e "Terminal" (centro) também aparecem marcados como ativos.

## [pt-BR.dark.bench-tab.extension.panels.bench](pages/pt-BR.dark.bench-tab.extension.panels.bench.md)

- Estouro de largura: os botões "Painéis de extensões" (até x=1115 e x=1173) e "Ajudante (chat)" (até x=1234) do painel direito têm retângulos além da largura da janela (1053 px) e ficam cortados ou invisíveis.
- Sobreposição de abas no painel direito: "Arquivos"/"Bancada", "Mudanças"/"Revisão", "Histórico"/"Busca" e "Portas"/"Painéis de extensões" ocupam retângulos que se sobrepõem, indicando dois grupos de abas desenhados na mesma faixa.
- Nome acessível duplicado e ambíguo: dois botões "Painéis de extensões" no grupo do painel direito, além de um terceiro no painel esquerdo, com o mesmo texto.
- Estado inconsistente: no painel direito, "Painéis de extensões" está marcado=True enquanto a aba visivelmente ativa é "Arquivos" (marcado=True também), sugerindo dois grupos de abas com ativos simultâneos.
- Texto cortado por transbordo de painel: a aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) cruza o divisor em x≈310 e aparece truncada ("Painéis de e") na borda do painel.
- Elementos sem nome acessível: alternador de tema (845,5), os dois divisores verticais (304,42 e 716,42), as faixas dos grupos de abas e os indicadores de sublinhado — apenas "Unknown"/(sem nome acessível) na árvore.
- Botões de janela nomeados apenas pelos símbolos "–", "□" e "×", sem rótulo descritivo (minimizar/maximizar/fechar).

## [pt-BR.dark.right-tab.git.changes](pages/pt-BR.dark.right-tab.git.changes.md)

- "Painéis de extensões" (220,50)–(371,82): texto renderizado é cortado na borda do painel esquerdo ("Painéis de e…") e o retângulo sobrepõe o botão "Terminal" (324,50)–(401,82).
- "Painéis de extensões" (965,50)–(1115,82): aba da faixa central parcialmente coberta pelo painel direito; retângulo sobrepõe "Arquivos" e "Mudanças".
- "Ajudante (chat)" (1115,50)–(1234,82): não está visível na imagem, embora esteja na árvore; seu retângulo sobrepõe "Busca" (1157,50)–(1216,82) e "Portas" (1216,50)–(1278,82).
- "Painéis de extensões" (1278,50)–(1429,82): retângulo vai além da largura da janela (1280 px).
- Botões de janela rotulados apenas com os caracteres "–", "□" e "×" como nome acessível.
- Vários controles sem nome acessível: alternador de tema (1101,5)–(1133,37), os quatro botões de ícone da barra do terminal (444–581), o chip "pwsh", os divisores verticais e a área do terminal.
- A dica "Clique para digitar" (≈884,737–973,749), visível na imagem dentro do terminal, não tem entrada na árvore de acessibilidade.
- Abas e textos inativos em cinza médio sobre fundo escuro (por exemplo "Bancadas", "Navegador", "Histórico" do topo) têm contraste baixo.

## [pt-BR.dark.bench-tab.git.history](pages/pt-BR.dark.bench-tab.git.history.md)

- Abas "Painéis de extensões" (965,50)–(1115,82), "Painéis de extensões" (1022,50)–(1173,82) e "Ajudante (chat)" (1115,50)–(1234,82) têm retângulos além da largura da janela (1024 px): ficam fora da área visível e inalcançáveis.
- Retângulos de abas sobrepostos no grupo direito: "Arquivos"/"Bancada" (736–815), "Mudanças"/"Revisão" (814–901) e "Histórico"/"Busca" (885–965); também "Terminal" (324–401) sobre "Painéis de extensões" (220–371) — indício de elementos duplicados ou de grupos de abas sobrepostos na árvore.
- Nomes ambíguos para leitores de tela: três botões chamados "Painéis de extensões" e dois chamados "Histórico" (aba do grupo direito e sub-aba do painel Git).
- Texto cortado: no painel direito aparece apenas o "N" inicial em (~718,255) na borda do divisor (716,42)–(732,736); corresponde ao rótulo "Nenhuma mudança." (717,248)–(841,267) parcialmente ocluído.
- Rótulos de aba truncados pelos divisores de painel: "Painéis de e…" (grupo esquerdo, corte em x≈300) e "Visualizado" (painel central, corte em x≈723).
- Quatro elementos sem nome acessível além dos ícones declarados: os dois divisores verticais (304,42)–(320,736) e (716,42)–(732,736), a faixa de abas esquerda (64,50)–(300,82) e a faixa de abas direita (736,50)–(1012,82); a Image do logotipo (12,50)–(40,78) também não tem nome.
- Três Labels declarados como "(ícone)"; o de (765,212)–(793,240) não corresponde a nada visível na captura e carece de nome acessível.
- O botão de alternar tema (845,5)–(877,37) é um "Unknown" sem nome acessível, identificável apenas pelo ícone de sol.

## [pt-BR.dark.modal.helper.profile_picker](pages/pt-BR.dark.modal.helper.profile_picker.md)

- A janela do modal vai até y=804 e o botão "Cancelar" até y=795, além da altura da imagem (768): a parte inferior do modal está cortada na captura e "Cancelar" aparece só pela metade.
- Vários elementos da linha de abas do painel direito ("Painéis de extensões" em 965 e 1022 e "Ajudante (chat)" até x=1234) se estendem além da largura da imagem (1024), fora da área visível.
- O modal sobrepõe a área central: as abas "Terminal", "Editor", "Navegador", "Diferenças", "Visualizador" e os botões "+", "Receitas" e "Shell" continuam na árvore de acessibilidade mas estão invisíveis ao usuário.
- O botão "Bigorna  Ctrl+K" é parcialmente coberto pelo modal (só "Big" é visível) e a aba "Painéis de extensões" do painel esquerdo é truncada ("Painéis de e").
- Abas do painel direito com retângulos sobrepostos e entradas duplicadas: "Arquivos"/"Bancada" (736–815 vs 739–815), "Mudanças"/"Revisão" (814–901 vs 815–885), "Histórico"/"Busca" (885–965 vs 901–960) e duas ocorrências extra de "Painéis de extensões" que se sobrepõem entre si.
- Botão de tema na barra superior (~845,5–877,37) e diversos controles da área central e divisores sem nome acessível.
- Rótulos de ícone expostos como "(ícone)" (grade e pasta dos estados vazios), sem nome acessível descritivo.
- Elemento de dimensão nula na árvore em (512,384)–(512,384).

## [pt-BR.dark.rail.helper.tab](pages/pt-BR.dark.rail.helper.tab.md)

- A lista de perfis não cabe na janela: os elementos a partir de "Iniciar" (509,790)–(570,825) e "Kimi Code CLI" (324,797)–(410,816) ficam abaixo da altura da imagem (800 px), cortados sem indício visível de rolagem além da barra estreita em x=597.
- Sobreposição na base da janela: o botão "Iniciar" (463,747)–(523,782) e a área da lista invadem a faixa da barra de status (y≥768), onde estão "Pronto", "56 módulos" e "Notificações" (y 775–794).
- Texto truncado: a aba "Painéis de extensões" (220,50)–(371,82) ultrapassa o limite do painel esquerdo (divisor em x 304–320) e aparece cortada como "Painéis de e".
- Elemento fora da janela: a aba "Painéis de extensões" do painel direito (1278,50)–(1429,82) fica inteiramente além da largura de 1280 px, invisível na captura.
- Botões de janela com nomes acessíveis simbólicos "–", "□" e "×" em vez de rótulos descritivos (Minimizar/Maximizar/Fechar).
- Vários elementos sem nome acessível: dois divisores, o contêiner de abas de cada painel, o botão de tema e as imagens/ícones de estado vazio (registrados como "(sem nome acessível)" ou "(ícone)").
- Dezesseis botões com o mesmo nome acessível "Iniciar" na mesma tela, o que dificulta a identificação por leitor de tela (o alvo só é inferido pela linha do perfil).

## [pt-BR.dark.rail.muster.page](pages/pt-BR.dark.rail.muster.page.md)

- O botão "Painéis de extensões" da tira direita tem retângulo (1278,50)–(1429,82), que ultrapassa a largura da janela (1280 px); o texto fica cortado fora da área visível.
- O botão "Painéis de extensões" da tira esquerda (220,50)–(371,82) invade a região do divisor em (304,42)–(320,768); o rótulo aparece truncado ("Painéis de e") no painel.
- Dois botões com o mesmo nome acessível "Painéis de extensões" em abas diferentes, o que dificulta a identificação por leitores de tela.
- Botão de alternância de tema e as duas divisórias verticais estão sem nome acessível.
- Textos de estado vazio e subtítulo em cinza sobre fundo escuro têm contraste reduzido (por exemplo, "Tarefas, ajudantes e recados num só lugar.").

## [pt-BR.dark.right-tab.ports.panel](pages/pt-BR.dark.right-tab.ports.panel.md)

- A aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) invade a área central e sobrepõe a aba "Terminal" (324,50)–(401,82).
- A aba "Painéis de extensões" da área central (965,50)–(1115,82) cruza o divisor (972,42)–(988,768) e sobrepõe a aba "Arquivos" do painel direito (992,50)–(1070,82).
- No painel direito, a aba "Ajudante (chat)" (1115,50)–(1234,82) sobrepõe "Mudanças" (1070,50)–(1157,82) e "Busca" (1157,50)–(1216,82).
- A aba "Painéis de extensões" do painel direito (1278,50)–(1429,82) ultrapassa a largura da imagem/janela (1280 px) e fica cortada.
- Botões sem nome acessível: alternância de tema (1101,5)–(1133,37) e os quatro ícones da barra do terminal (fechar, dividir, sessão, busca).
- O chip da sessão "pwsh" (328,107)–(404,129) e seu ponto de status (385,110)–(401,126) não têm nome acessível.
- O rótulo "(ícone)" do estado vazio (168,198)–(196,226) não tem nome acessível significativo.
- "Nenhum projeto" (11,12)–(113,31) e "Notificações" (1170,775)–(1246,794) são Labels, mas expõem ações de clique e foco, papel incoerente com o tipo.
- Três abas aparecem simultaneamente com marcado=True ("Projetos", "Terminal" e "Portas"), o que torna ambíguo qual painel está de fato selecionado.

## [pt-BR.dark.rail.request.page](pages/pt-BR.dark.rail.request.page.md)

- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), que passa da largura da imagem (1280 px): o botão fica cortado e inalcançável por clique visível.
- A aba "Painéis de extensões" do painel esquerdo aparece truncada como "Painéis de e" no texto visível.
- O botão de alternância de tema no topo direito (1101,5)–(1133,37) não tem nome acessível; é identificável apenas pelo ícone de sol.
- O campo de busca (TextInput) não tem nome acessível, apenas o texto-guia visível.
- Os três ícones de estado vazio (pasta, envelope, grade) são Labels "(ícone)" sem descrição acessível.
- O nome "Todos" se repete em dois grupos de filtro (lista e origem) com retângulos distintos, o que pode gerar ambiguidade para leitores de tela.

## [pt-BR.dark.bench-tab.review.tab](pages/pt-BR.dark.bench-tab.review.tab.md)

- O formulário "Novo pull request" está cortado: os campos "Título" e "Descrição" têm retângulos até x=1234, enquanto o painel visível termina em ~724 px, e "Abrir como rascunho", "Gerar" e o campo de "Labels" ficam na borda do painel, parcialmente fora da área visível.
- As abas "Painéis de extensões" (1022,50)–(1173,82) e "Ajudante (chat)" (1115,50)–(1234,82) estendem-se além da largura da janela (~1054 px) e ficam fora da imagem.
- Há duas abas com o mesmo nome "Painéis de extensões" no conjunto da bancada, com retângulos sobrepostos entre si e com "Portas" ((965,50)–(1115,82) sobre (1022,50)–(1173,82) e (960,50)–(1022,82)).
- Sobreposição de retângulos no conjunto de abas da bancada: "Arquivos"/"Bancada" (736–814 vs 739–815), "Mudanças"/"Revisão" (814–901 vs 815–885) e "Histórico"/"Busca" (885–965 vs 901–960).
- No conjunto de abas do painel esquerdo, "Painéis de extensões" (220,50)–(371,82) se sobrepõe às abas centrais "Terminal", "Editor", "Navegador", "Diferenças" e "Visualizador"; na imagem o texto aparece truncado como "Painéis de e".
- A aba "Visualizador" aparece truncada na imagem ("Visualizado…") por colidir com a borda do painel central.
- As duas abas "Arquivos" e "Revisão" estão marcadas como ativas ao mesmo tempo (marcado=True), mas só "Arquivos" aparece sublinhada.
- O rótulo "Branch de origem:" é seguido diretamente do campo que exibe o texto "Título"; não há campo visível para informar a branch de origem.
- Campos de texto sem nome acessível: "Título" (324,211), "Descrição" (324,242), Base (360,407), Revisores (390,447) e Labels (672,447).
- Elementos interativos sem nome acessível: ícone de tema na barra superior (845,5), divisores dos painéis (304,42) e (716,42) e faixas de abas (64,50) e (736,50).
- Os divisores verticais (304,42) e (716,42) e as faixas de abas só expõem a ação "focus", sem ação de arrastar/clique associada.

## [pt-BR.dark.rail.routine.page](pages/pt-BR.dark.rail.routine.page.md)

- O botão "Painéis de extensões" do painel direito tem o retângulo (1278,50)–(1429,82) estendido além da largura da imagem/janela (1280 px): parte do elemento fica fora da tela, embora o texto visível "Portas" ainda caiba.
- O texto da aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) é cortado visualmente pela área central, aparecendo como "Painéis de e" na captura.
- Botões-ícone do trilho ("Quadro", "Ajudantes", "Mutirão", "Pedidos", "Skills", "Uso", "Extensões", "Acervo") dependem exclusivamente de ícone na interface; o nome acessível existe na árvore, mas não há rótulo visível nem dica na tela.
- Os três ícones ilustrativos dos estados vazios e os ícones da barra superior e divisores são expostos como "(sem nome acessível)", sem descrição para leitores de tela.

## [pt-BR.dark.right-tab.search.panel](pages/pt-BR.dark.right-tab.search.panel.md)

- A aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) ultrapassa a borda do painel, invade a área central e sobrepõe a aba "Terminal" (324,50)–(401,82); na imagem o texto aparece truncado ("Painéis de e...").
- A aba "Painéis de extensões" do painel direito (1278,50)–(1429,82) tem retângulo além da largura da janela (1280 px): fica fora da área visível.
- A aba "Ajudante (chat)" (1115,50)–(1234,82) sobrepõe as abas "Busca" (1157,50)–(1216,82) e "Portas" (1216,50)–(1278,82) na árvore de acessibilidade, embora não apareça desenhada na imagem.
- Há elementos Unknown sobrepostos às faixas de abas ((64,50)–(300,82) e (992,50)–(1268,82)), o que pode atrapalhar a navegação por acessibilidade.
- Os quatro campos de texto do painel Busca e os quatro botões-ícone da barra do terminal não têm nome acessível; só são identificáveis pelo placeholder ou pela aparência.
- O ícone de tema na barra superior (1101,5)–(1133,37) é um elemento Unknown sem nome acessível.
- O chip "pwsh" (328,107)–(404,129) contém um segundo elemento Unknown sobreposto (385,110)–(401,126), duplicando a região na árvore.
- Botões "Buscar", "Parar", "Prévia" e "Aplicar" estão desativados sem indicador visual de tooltip explicativo; a razão só aparece no texto "Abra uma bancada primeiro." abaixo dos botões.

## [pt-BR.dark.rail.skill.page](pages/pt-BR.dark.rail.skill.page.md)

- O botão "Painéis de extensões" do painel direito tem o retângulo (1278,50)–(1429,82) além da largura da janela (1280 px): o texto fica cortado e parte do botão não é visível/clicável.
- Na barra de abas do painel esquerdo, o botão "Painéis de extensões" (220,50)–(371,82) ultrapassa a borda do painel (x≈312); na imagem só se lê "Painéis de e", texto truncado.
- O campo de busca "Buscar skills" é um TextInput sem nome acessível; o texto de sugestão só aparece visualmente.
- O ícone de alternância de tema na barra superior (1101,5)–(1133,37) é um elemento clicável sem nome acessível; a função só é identificável pela imagem.
- Os divisores verticais, os contêineres das barras de abas e os indicadores de aba ativa são elementos sem nome acessível.
- Os quatro ícones de estado vazio aparecem como rótulos "(ícone)" sem descrição de texto alternativa.

## [pt-BR.dark.rail.usage.page](pages/pt-BR.dark.rail.usage.page.md)

- A aba "Painéis de extensões" do painel esquerdo é exibida como "Painéis de e…": o retângulo do botão (até x=371) passa do limite do painel (x=304) e o texto é cortado.
- No painel direito, o botão "Painéis de extensões" tem retângulo até x=1429, além da largura da janela (1280 px); a aba aparece truncada na borda direita.
- Linhas da série "Por dia" de 2026-11-08 a 2026-11-13 ficam além da altura da janela (y até 991, imagem de 800 px); a área é rolável e há barra de rolagem, mas o corte da última linha visível (2026-11-07) encosta na borda do cartão.
- O botão de alternância de tema na barra superior (1101,5)–(1133,37) não tem nome acessível (apenas ícone de sol).
- Os botões de janela usam os símbolos "–", "□" e "×" como nome acessível, em vez de rótulos descritivos ("Minimizar", "Maximizar", "Fechar").
- O logotipo do app (12,50)–(40,78) é uma imagem sem nome acessível.

## [pt-BR.dark.bench-tab.viewers.tab](pages/pt-BR.dark.bench-tab.viewers.tab.md)

- Sobreposição de abas: "Terminal" (324–401) invade "Painéis de extensões" (220–371) na mesma faixa; no painel direito, "Bancada" (739–815) cobre "Arquivos" (736–814), "Revisão" (815–885) cobre "Mudanças" (814–901) e "Busca" (901–960) invade "Histórico" (885–965).
- Elementos fora da janela (largura da imagem: 1024 px): "Painéis de extensões" (965–1115 e 1022–1173) e "Ajudante (chat)" (1115–1234) têm retângulos além da borda direita.
- Texto cortado na imagem: "Painéis de e…" (aba do painel esquerdo, cortada em x≈300), "Visualizado…" (aba central, cortada em x≈724) e a mensagem do visualizador, visível só até "ou us…" (cortada pelo divisor em x≈724), embora seu retângulo vá até x=909, cruzando para o painel direito.
- Duas faixas de abas reportadas na mesma linha y=50–82 com abas duplicadas ("Arquivos" e "Painéis de extensões" aparecem duas vezes), o que indica abas de painéis distintos empilhadas na mesma posição.
- Botão sem nome acessível: o ícone de tema claro/escuro em (845,5)–(877,37) não tem rótulo.
- A aba "Visualizador" (641–739) e o contêiner direito (736–1012) se sobrepõem levemente em x=736–739.

## [pt-BR.dark.settings.accounts](pages/pt-BR.dark.settings.accounts.md)

- O botão "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82) além da largura da imagem (1280 px): fica cortado/fora da janela visível.
- O modal "Ajustes" (328,58)–(1026,571) se sobrepõe às barras de abas (y 50–82) e à barra de ferramentas central, escondendo os botões "+", "Receitas", "Shell" e as abas "Terminal" a "Painéis de extensões", além do início da aba "Arquivos".
- O texto da aba "Painéis de extensões" do painel esquerdo aparece cortado ("Painéis de e") por causa dessa sobreposição.
- Retângulos de abas se sobrepõem entre si: "Painéis de extensões" central (965–1115) cruza "Arquivos" (992–1070) e "Ajudante (chat)" (1115–1234); "Mudanças" (1070–1157) também cruza "Ajudante (chat)".
- A área central sem nome (328,139)–(980,756) se sobrepõe ao retângulo do modal "Ajustes".
- O botão de fechar do modal tem nome acessível em inglês, "Close window", em uma interface em pt-BR.
- Os campos de busca de ajustes e os dois campos "Nome" são TextInput sem nome acessível; o rótulo visível é apenas o placeholder.
- O botão "Achar arquivo  Ctrl+P" tem dois espaços entre o rótulo e o atalho.
- Vários elementos da barra central e do modal são ícones sem nome acessível (incluindo o alternador de tema na barra superior), o que prejudica leitores de tela.

## [pt-BR.dark.settings.content](pages/pt-BR.dark.settings.content.md)

- A aba "Painéis de extensões" do lado direito (1278,50)–(1429,82) ultrapassa a largura da janela (1280 px): a faixa de abas transborda e a aba fica invisível fora da tela.
- No painel esquerdo, o texto da aba "Painéis de extensões" é cortado na divisória ("Painéis de e"), sem reticências.
- Botão "Close window" com nome acessível em inglês em uma interface pt-BR.
- Mensagens de diagnóstico "missing `+++` frontmatter" em inglês, sem tradução.
- Caminhos exibidos com separadores misturados e trecho relativo cru ("…\modules\content\../../../content\templates\…"), difíceis de ler.
- Retângulos de abas se sobrepõem na árvore de acessibilidade: "Ajudante (chat)" (1115–1234) sobre "Busca" (1157–1216) e "Painéis de extensões" (965–1115) sobre "Arquivos" (992–1070), indicando faixas de abas duplicadas.
- Botão da barra superior em (1101,5)–(1133,37) (ícone de tema) sem nome acessível.
- O texto visível "Clique para digitar" no rodapé da área central (aprox. 885,735–973,750) não corresponde a nenhum elemento nomeado na árvore de acessibilidade.
- A modal encobre a faixa de abas central e corta a aba "Arquivos" ao meio ("…uivos"); os grupos de quatro botões-ícone sem rótulo (444–581,102–134) também ficam totalmente atrás da modal.
- Textos secundários em cinza médio sobre fundo escuro (por exemplo, "rev 1" e os caminhos dos diagnósticos) têm contraste baixo.

## [pt-BR.dark.settings.counter_github](pages/pt-BR.dark.settings.counter_github.md)

- O botão de fechar do modal tem nome acessível em inglês, "Close window", em uma interface em pt-BR (esperado "Fechar janela").
- A segunda aba "Painéis de extensões" do grupo direito tem retângulo até x=1429, ultrapassando a largura da imagem (1280 px): elemento cortado/fora da janela.
- A aba "Painéis de extensões" do painel esquerdo aparece com o texto truncado ("Painéis de e") pela borda do painel, embora o retângulo indicado vá até x=371.
- O modal sobrepõe as abas centrais (de "Terminal" a "Painéis de extensões"), a barra de ferramentas do editor e parte da mensagem "Nenhuma bancada ativa...", que começa encoberta (só "...uma bancada ativa..." fica visível).
- O campo de busca e o campo de token não têm nome acessível (dependem apenas do placeholder "Buscar ajustes"/"Token").
- Os quatro botões-ícone da barra do editor (444–581 px) e o ícone de tema da barra superior estão sem nome acessível.
- Os retângulos das abas "Ajudante (chat)", "Mudanças" e "Busca" se sobrepõem entre si (por exemplo, "Ajudante (chat)" de 1115 a 1234 invade as duas vizinhas), indicando sobreposição de alvos de clique.
- Texto "Clique para digitar" visível na área central sem elemento correspondente na árvore de acessibilidade.
- Textos de apoio em cinza médio sobre fundo escuro (placeholder "Token", "Não conectado", mensagens de estado vazio) têm contraste baixo.

## [pt-BR.dark.settings.counter_gitlab](pages/pt-BR.dark.settings.counter_gitlab.md)

- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), que vai além da largura da janela de 1280 px: elemento cortado/fora da tela.
- O botão de fechar do modal tem nome acessível em inglês, "Close window", em uma interface em pt-BR.
- A janela "Ajustes" sobrepõe a barra de abas central ("Terminal", "Editor", "Navegador", "Diferenças", "Visualizador", "Bancada", "Revisão", "Histórico", "Painéis de extensões") e a barra de ferramentas ("+", quatro botões de ícone sem nome, "Receitas", "Shell"); o fragmento "…uivos" de "Histórico" aparece cortado ao lado do modal.
- O texto "Nenhuma bancada ativa. Abra uma bancada para ver os arquivos." começa em x=994, mas o modal se estende até x=1026: o início da frase fica encoberto.
- Três botões compartilham o mesmo nome acessível "Painéis de extensões" (ambiguidade para leitores de tela).
- Quatro botões de ícone da barra de ferramentas central ((444,102), (479,102), (514,102), (549,102)) estão sem nome acessível, e o texto de apoio "Clique para digitar" do painel central não tem elemento acessível correspondente.
- O botão "Conectar" está desativado, mas com preenchimento âmbar que o faz parecer acionável; os botões desativados ("Conectar", "Desconectar") têm contraste reduzido.
- O nome do botão "Achar arquivo  Ctrl+P" contém espaço duplo entre o rótulo e o atalho.

## [pt-BR.dark.settings.counter_jira](pages/pt-BR.dark.settings.counter_jira.md)

- Botão "Painéis de extensões" em (1278,50)–(1429,82) tem retângulo além da largura da imagem (1280 px) e fica cortado fora da janela.
- Três botões com o mesmo nome acessível "Painéis de extensões" em posições distintas ((220,50), (965,50) e (1278,50)); nomes duplicados dificultam navegação e automação por acessibilidade.
- "Ajudante (chat)" em (1115,50)–(1234,82) sobrepõe o retângulo de "Busca" (1157,50)–(1216,82) e não aparece renderizado na imagem.
- Botão "Close window" com nome acessível em inglês em uma interface em pt-BR.
- Ícone clicável de tema (sol) em (1101,5)–(1133,37) sem nome acessível.
- Vários elementos interativos sem nome acessível: campos de texto (só identificáveis pelo texto de apoio visível), botões de ícone em (444,102)–(581,134), alças de redimensionamento e divisórias da modal e sublinhados de abas.
- A modal cobre e trunca elementos de fundo: a aba "Painéis de extensões" do painel esquerdo aparece cortada ("Painéis de e…"), a aba "Arquivos" aparece como "…uivos" e o aviso "Nenhuma bancada ativa..." é parcialmente encoberto — sobreposição esperada de uma modal, mas que reduz a legibilidade dos rótulos.
- Os botões "+", "Receitas" e "Shell" (faixa y102–134) ficam totalmente sob a modal e invisíveis nesta captura.
- Texto "Clique para digitar" solto no canto inferior direito da área central (parte do painel de terminal vazio atrás da modal), sem elemento correspondente nomeado na árvore.

## [pt-BR.dark.settings.counter_linear](pages/pt-BR.dark.settings.counter_linear.md)

- A aba "Painéis de extensões" do painel direito tem retângulo (1278,50)–(1429,82), ultrapassando a largura da janela (1280 px); seu texto não pode estar integralmente visível.
- O botão de fechar o modal tem nome acessível "Close window", em inglês, na localização pt-BR.
- Sobreposição na árvore de acessibilidade: "Ajudante (chat)" (1115,50)–(1234,82) sobrepõe as abas "Mudanças" e "Busca"; na imagem apenas "Mudanças", "Busca" e "Portas" aparecem.
- Vários botões-ícone da barra da área central ("+", quatro botões entre 444–581 px) e os elementos de borda das faixas de abas e do modal estão sem nome acessível.
- O TextInput de busca e o PasswordInput de token não têm nome acessível; dependem apenas do texto de sugestão ("Buscar ajustes", "Token").
- O texto "Clique para digitar" (~883,737) é exibido em cinza escuro sobre fundo escuro, com contraste baixo.
- Duas abas aparecem marcadas simultaneamente ("Projetos" no painel esquerdo e "Terminal" na área central), o que dificulta identificar o painel realmente ativo.
- Os botões "Receitas" e "Shell" e os ícones vizinhos permanecem totalmente encobertos pelo modal, sem indicação visual de sua existência.

## [pt-BR.dark.settings.helper](pages/pt-BR.dark.settings.helper.md)

- O botão de fechar do modal tem nome acessível em inglês ("Close window") em uma tela de locale pt-BR.
- O botão "Painéis de extensões" (1278,50)–(1429,82) fica além da largura da imagem (1280 px), fora da área visível.
- Elementos de borda da janela com retângulos em y negativo ((319,-10)–(339,11), (1066,-10)–(1086,11), (339,-8)–(1066,9)), parcialmente fora da captura.
- Retângulos sobrepostos na faixa de abas direita: "Painéis de extensões" (965–1115) sobrepõe "Arquivos" (992–1070) e "Mudanças" (1070–1157); "Ajudante (chat)" (1115–1234) sobrepõe "Busca" (1157–1216) — possível sobreposição de abas.
- A janela "Ajustes" tem altura declarada de 3579 px, muito além dos 800 px da captura; todo o conteúdo dos cartões abaixo de Claude Code só é acessível rolando, sem indicação visível de rolagem na dobra exibida.
- Oito cartões exibem "Carregando…" persistente no estado de hooks (Continue CLI, Crush, Gemini CLI, Goose, Kimi Code CLI, Kiro CLI, OpenCode, Qwen Code) — status não resolvido na captura.
- O caminho do Goose termina em x=1068, tocando a borda interna do modal (limite em x≈1066), sem folga à direita.
- Vários controles sem nome acessível: quatro elementos Unknown em (444–581,102–134), o botão de tema (1101,5)–(1133,37) e todos os campos TextInput ("Argumentos extras", "Ambiente", "Buscar ajustes") sem rótulo associado.
- O botão "Ajustes" do trilho esquerdo permanece marcado=False enquanto a tela de Ajustes está aberta (estado inconsistente com a tela exibida).

## [pt-BR.dark.settings.import](pages/pt-BR.dark.settings.import.md)

- A última aba "Painéis de extensões" (1278,50)–(1429,82) ultrapassa a largura da janela (1280 px): o texto fica cortado fora da área visível.
- O botão que fecha o diálogo tem nome acessível em inglês ("Close window") numa interface em pt-BR.
- O diálogo "Ajustes" cobre a maior parte da área central e invade o painel direito, escondendo parte da mensagem "Nenhuma bancada ativa. Abra uma bancada para ver os arquivos." e as abas centrais — esperado num modal, mas a sobreposição não é indicada por sombra/contraste suficientes nas bordas.
- Vários controles sem nome acessível: o alternador de tema (1101,5), os quatro botões-ícone de filtro do terminal (444–581,102), o campo de busca do diálogo e as alças/divisores, dificultando o uso por leitor de tela.
- A faixa sem nome (992,50)–(1268,82) sobrepõe os botões nomeados "Arquivos", "Mudanças", "Ajudante (chat)", "Busca" e "Portas" no painel direito.
- Dois itens de navegação do diálogo (328,107)–(404,129) e (385,110)–(401,126) ficam atrás da barra de título/campo, sem texto legível na imagem.
- A aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) é cortada pela borda do divisor em x≈304, mostrando apenas "Painéis de e…".

## [pt-BR.dark.settings.mcp](pages/pt-BR.dark.settings.mcp.md)

- O botão "Painéis de extensões" tem retângulo (1278,50)–(1429,82), que ultrapassa a largura da janela (imagem com 1280 px): elemento cortado/fora da tela.
- A faixa de abas da área central aparece duplicada na árvore com retângulos deslocados: a mesma sequência de abas é listada em (324,50)–(965,82) e novamente entre (965,50) e (1429,82), sobrepondo o painel direito e a aba "Arquivos".
- A janela modal "Ajustes" sobrepõe a barra de ferramentas central: "+", os quatro botões sem nome (444–581,102), "Receitas" e "Shell" ficam ocultos atrás dela.
- O campo de busca do modal é um TextInput sem nome acessível (só o texto de apoio "Buscar ajustes" é visível).
- Botões sem nome acessível: o alternador de tema (ícone de sol, (1101,5)) e os quatro botões de ícone da barra do terminal ((444,102)–(581,134)).
- O botão de fechar do modal tem o rótulo em inglês ("Close window") numa interface em pt-BR.
- Ícones registrados apenas como "(ícone)" ((168,198) e (1116,114)) sem texto alternativo descritivo.
- Textos secundários ("Servidor · ligado: 52" e o aviso final do modal) em cinza médio sobre fundo escuro, com contraste aparentemente baixo.

## [pt-BR.dark.settings.onboarding](pages/pt-BR.dark.settings.onboarding.md)

- Botão "Close window" (994,76)–(1008,90) com nome acessível em inglês (não traduzido para pt-BR) e alvo muito pequeno (14×14 px).
- Aba "Painéis de extensões" (1278,50)–(1429,82) do painel direito se estende além da largura da imagem (1280 px); texto cortado/fora da janela.
- A janela "Ajustes" sobrepõe as barras de abas: a aba esquerda "Painéis de extensões" (220,50)–(371,82) aparece truncada ("Painéis de e…") e a aba "Arquivos" (992,50)–(1070,82) aparece parcialmente encoberta ("uivos").
- Retângulos sobrepostos entre "Ajudante (chat)" (1115,50)–(1234,82), "Busca" (1157,50)–(1216,82) e "Portas" (1216,50)–(1278,82).
- Vários elementos sem nome acessível: o campo de busca de ajustes (TextInput em 337,116), a grande área de texto central (328,139)–(980,756), o botão-ícone de tema (1101,5), o logotipo (12,50), os separadores verticais e os cinco ícones da barra central (444–581,102), que identificam apenas pela posição.
- Os botões "+", "Receitas" e "Shell" e os quatro ícones sem nome da faixa central (y≈102–134) ficam totalmente encobertos pela janela "Ajustes", assim como a aba central ativa "Terminal".

## [pt-BR.dark.settings.remote](pages/pt-BR.dark.settings.remote.md)

- A aba "Painéis de extensões" do painel direito (1278,50)–(1429,82) se estende além da largura da janela (1280 px): o texto fica fora da área visível.
- O botão de fechar do modal tem nome acessível em inglês, "Close window", em uma interface pt-BR.
- Os quatro campos do formulário de novo destino SSH e o campo "Buscar ajustes" são TextInput "(sem nome acessível)": o rótulo existe apenas como texto visual/espaço reservado.
- Vários elementos sem nome acessível: divisores verticais, faixas de abas e bordas do modal, quatro botões de ícone da barra de projetos (444–581, 102–134) e os ícones registrados apenas como "(ícone)".
- A janela modal sobrepõe a barra de abas central e os botões "+", "Receitas" e "Shell" do painel de projetos; as abas "Editor", "Navegador", "Diferenças", "Visualizador" e "Bancada" ficam totalmente ocultas e "Painéis de e…" aparece truncado na borda esquerda do modal (sobreposição esperada em modal, mas que esconde controles da tela de fundo).
- Os botões de janela da barra de título têm apenas símbolos como nome ("–", "□", "×").

## [pt-BR.dark.settings.trust](pages/pt-BR.dark.settings.trust.md)

- O botão "Painéis de extensões" do painel direito, em (1278,50)–(1429,82), tem retângulo além da largura da imagem (1280 px): fica cortado fora da janela.
- O diálogo "Ajustes" se sobrepõe às abas das áreas central e direita: os textos "Painéis de e…" e "Arquivos" aparecem parcialmente encobertos na captura.
- O TextInput do campo de busca de ajustes (337,116)–(1017,148) não tem nome acessível, embora exiba o texto de apoio "Buscar ajustes".
- O botão de fechar do diálogo usa o nome "Close window" em inglês, sem tradução para pt-BR.
- Quatro ícones da barra de ferramentas central (444–581, y 102–134) e o alternador de tema do topo não têm nome acessível.
- O nome "Painéis de extensões" se repete em três botões distintos, o que dificulta a identificação por leitores de tela.
- Textos secundários em cinza médio sobre fundo escuro ("Nada aprovado ainda.", "Pedidos pendentes: 0", avisos de vazio e "Clique para digitar") têm baixo contraste.

## [pt-BR.dark.settings.voice](pages/pt-BR.dark.settings.voice.md)

- Aba "Painéis de extensões" (220,50)–(371,82) do painel esquerdo é cortada na borda do painel (visível só "Painéis de e") e se sobrepõe à aba "Terminal" (324,50)–(401,82).
- A segunda aba "Painéis de extensões" (965,50)–(1115,82) fica encoberta por "Arquivos" (992,50)–(1070,82) e "Ajudante (chat)" (1115,50)–(1234,82); não aparece na captura.
- "Ajudante (chat)" (1115,50)–(1234,82) se sobrepõe a "Busca" (1157,50)–(1216,82) e "Portas" (1216,50)–(1278,82) na mesma faixa de abas.
- A terceira aba "Painéis de extensões" (1278,50)–(1429,82) tem retângulo além da largura da imagem (1280 px), indicando elemento fora da janela visível.
- Botão "Close window" com nome acessível em inglês em uma interface em pt-BR.
- TextInput de busca de ajustes (337,116)–(1017,148) e PasswordInput da chave (337,471)–(665,494) não têm nome acessível; dependem apenas do texto de exemplos/placeholder.
- O botão "Ajustes" do trilho permanece marcado=False enquanto a janela "Ajustes" está aberta em foco, inconsistência de estado de seleção.
- Vários botões-ícone sem nome acessível na barra de ferramentas central (444–581, 102–134), o que impede identificação por leitor de tela.

## [en.dark.home.default](pages/en.dark.home.default.md)

- A aba "Extension panels" do painel esquerdo (211,50)–(337,82) ultrapassa a borda do painel (x≈304) e o texto aparece cortado ("Extension pan…") na imagem.
- A aba "Extension panels" da faixa central (878,50)–(1004,82) passa do limite da área central (x≈972) e o texto aparece cortado ("Extension pane…") na imagem.
- A faixa de abas do painel direito transborda a largura da janela: "Extension panels" (1240,50)–(1366,82) tem retângulo além da largura da imagem (1280 px) e fica inacessível visualmente.
- Retângulos sobrepostos na árvore do painel direito: "Helper (chat)" (1004,50)–(1107,82) sobrepõe "Files" (992,50)–(1043,82) e "Changes" (1043,50)–(1120,82); na imagem "Helper (chat)" não é visível, indicando medições inconsistentes da faixa de abas.
- Botões sem nome acessível (acessibilidade): o alternador de tema (1101,5)–(1133,37), os quatro ícones da barra do terminal (444,102)–(581,134) e a pílula de sessão "pwsh" (328,107)–(404,129) não expõem nome.
- Os botões de ícone do terminal e o alternador de tema dependem de reconhecimento visual, sem rótulo ou tooltip acessível na árvore.
- Os rótulos de estado vazio "(ícone)" ((168,158)–(196,186) e (1116,114)–(1144,142)) estão em português na variante em inglês — texto não traduzido na árvore de acessibilidade.
- Estados com pixels idênticos a esta captura incluem modais e outras telas que deveriam alterar a imagem (en.dark.modal.bench.dialogs, en.dark.modal.counter.poller, en.dark.modal.mcp.approval, en.dark.modal.notify.toasts, en.dark.modal.onboarding.wizard, en.dark.modal.voice.overlay, en.dark.right-tab.explorer.panel e en.dark.bench-tab.terminal.tab): nessas capturas nenhum diálogo, aviso, sobreposição ou mudança de aba aparece renderizado.
- O texto "Click to type" na área do terminal (canto inferior direito, ~(910,737)–(974,750)) não corresponde a nenhum elemento da árvore de acessibilidade.

## [en.dark.open.anvil](pages/en.dark.open.anvil.md)

- Vários elementos têm retângulos além dos limites da captura (1280×800): a aba "Extension panels" do painel direito vai até x=1366, e o conteúdo da Anvil (linhas de receita e lista) se estende até y=1812 — a lista de receitas e as abas finais são cortadas na imagem.
- A dica "Enter runs · Ctrl+Enter keeps the Anvil open · Esc closes" (330,797)–(622,813) fica fora do retângulo da janela da Anvil (que termina em y=792), invade a faixa da barra de status e passa do limite inferior da captura (y=800).
- Sobreposição de retângulos entre abas: "Extension panels" do painel esquerdo (211–337) sobrepõe a aba central "Terminal" (324–401); no painel direito, "Helper (chat)" (1004–1107) sobrepõe "Files" (992–1043) e "Changes" (1043–1120), e a aba central "Extension panels" (878–1004) sobrepõe "Files".
- Botões "Recipes", "Shell" e "+" (y≈102–134) ficam totalmente ocultos atrás da sobreposição da Anvil, sobreposição de camadas sem indicação visual.
- Elementos interativos sem nome acessível: o botão de alternar tema (ícone de sol, (1101,5)–(1133,37)), o campo de busca da Anvil, as dez linhas de resultado da busca, os quatro ícones de filtro da área central e os divisores dos painéis.
- As linhas de resultado da Anvil não expõem o texto visível como nome acessível (só aparência na imagem), o que prejudica leitores de tela.
- Abas com texto cortado na renderização: "Extension pai..." no painel esquerdo, "Extension pane" no fim das abas centrais e "Ext" no início do painel direito, truncadas pelas bordas dos painéis.
- O chip "Run golangci-lint  ·  project" (330,758)–(522,793) ultrapassa a base da janela da Anvil (y=792) e invade a faixa da barra de status.
- As barras de rolagem das listas têm apenas 2 px de largura ((948,220)–(950,580) e (570,629)–(572,783)), muito finas e de baixo contraste.

## [en.dark.open.toast](pages/en.dark.open.toast.md)

- A terceira aba do painel esquerdo, "Extension panels", tem texto cortado na borda do painel ("Extension pan" visível), pois seu retângulo (211,50)–(337,82) ultrapassa a largura do painel (~304 px).
- A aba "Extension panels" da área central tem retângulo (878,50)–(1004,82) que invade o painel direito; na imagem só aparece uma barra vertical estreita junto ao divisor.
- A aba "Extension panels" do painel direito tem retângulo (1240,50)–(1366,82) que vai além da largura da imagem (1280 px); não é visível.
- O retângulo do botão "Helper (chat)" (1004,50)–(1107,82) se sobrepõe às abas "Changes", "Search" e "Ports"; o botão não aparece na imagem.
- Cinco botões de ícone da barra do terminal (444,102)–(581,134) e o botão de tema (1101,5)–(1133,37) não têm nome acessível.
- Há dois botões nomeados apenas "×" (fechar janela em (1235,5) e fechar toast em (1230,703)), o que os torna ambíguos para leitores de tela.
- O texto "Click to type", visível na área do terminal, não aparece na árvore de acessibilidade.
- O contador "1 notices" está no plural para valor singular (deveria ser "1 notice").
- Elemento de dimensão zero (640,400)–(640,400) sem nome na área central.

## [en.dark.rail.archive.tab](pages/en.dark.rail.archive.tab.md)

- Texto de sugestão do campo de filtro não traduzido: aparece "Parser, pedido, erro..." (português) na interface em inglês.
- A aba "Extension panels" do painel esquerdo aparece truncada como "Extension pan" e seu retângulo (211,50)–(337,82) se sobrepõe ao rótulo "Archive" (324,50)–(382,74).
- A aba "Extension panels" do painel direito tem retângulo (1240,50)–(1366,82), ultrapassando a largura da janela (1280 px); o texto é cortado fora da tela, visível apenas como "Ext".
- Vários elementos sem nome acessível: os dois divisores verticais, os contêineres das linhas de abas, o alternador de tema da barra superior, o logo do trilho, os dois campos de filtro (TextInput) e os dois seletores de valores ("all helpers" e "any date"), o que dificulta a navegação por leitor de telas.
- Os botões de janela usam apenas os símbolos "–", "□" e "×" como nome acessível, sem rótulo descritivo.
- A barra de abas do painel direito não cabe na largura disponível (cinco abas se estendem além da borda), indicando estouro de layout.
- Os contêineres "Unknown" das linhas de abas em (64,80)–(300,82) e (992,80)–(1268,82) têm apenas 2 px de altura, sugerindo retângulos residuais possivelmente decorrentes de truncamento.

## [en.dark.bench-tab.bench.detail](pages/en.dark.bench-tab.bench.detail.md)

- A aba "Extension panels" da fileira do painel direito tem retângulo (1240,50)–(1366,82), que passa da largura da imagem (1280 px): fica parcialmente fora da janela; na imagem só se vê o início ("Ext") cortado pela borda.
- A aba "Extension panels" da área central (878,50)–(1004,82) invade a divisória do painel direito (972,42)–(988,768); o texto aparece truncado ("Extension pane").
- A aba "Extension panels" do painel de projetos (211,50)–(337,82) invade a divisória do painel (304,42)–(320,768); o texto aparece cortado ("Extension pai").
- Na fileira de abas do painel direito, "Helper (chat)" (1004,50)–(1107,82) fica empilhada com retângulos sobrepostos às abas visíveis "Files" (992,50)–(1043,82) e "Changes" (1043,50)–(1120,82) e não aparece na imagem.
- O botão de tema na barra superior (1101,5)–(1133,37) é um Unknown sem nome acessível; só é identificável pelo ícone de sol.
- Vários elementos da árvore estão sem nome acessível (divisórias, barras de abas, separadores, ícones), o que reduz a navegabilidade por leitor de tela.
- Rótulos de estado vazio ("No project yet…", "No bench is active…", "No active bench…") em cinza-claro sobre fundo escuro, com contraste baixo.
- Diferença em relação à referência pt-BR: aqui a mensagem central "No bench is active. Create a bench, or pick one in the sidebar or board." (557,182)–(874,220) não invade a divisória e não há aba com estado marcado ambíguo no painel direito (apenas "Files" está marcada); os defeitos de truncamento e sobreposição listados acima persistem.

## [en.dark.rail.board.page](pages/en.dark.rail.board.page.md)

- O botão "Extension panels" do painel direito tem retângulo (1240,50)–(1366,82), estendendo-se além da largura da janela (1280 px); a aba fica cortada (aparece só "Ext").
- O botão "Extension panels" do painel esquerdo tem retângulo (211,50)–(337,82), que cruza o divisor em x=320, e seu texto aparece truncado na borda do painel ("Extension pai").
- O cartão da quarta coluna, "Ready", é cortado pela borda direita da área central em x=972 (o rótulo, em (902,157)–(940,176), cabe inteiro, diferentemente da referência pt-BR).
- O Label "(ícone)" em (632,792)–(660,820) invade a barra de status e ultrapassa a altura da janela (800 px).
- A mensagem "No bench in this board yet. Create one to start." (502,828)–(791,847) está inteiramente abaixo da área visível e não aparece na tela.
- O campo de filtro de bancadas (TextInput em (324,90)–(564,122)) não tem nome acessível; o rótulo visível é só a marca d'água "Filter benches".
- Elementos sem nome acessível além do esperado: divisores verticais, faixas de abas, área do quadro, barra de rolagem horizontal e ícone de tema da barra superior.

## [en.dark.bench-tab.chat.tab](pages/en.dark.bench-tab.chat.tab.md)

- O grupo de abas do cabeçalho da bancada continua estourando: "Extension panels" (1240–1366) fica além da borda direita da janela (1280) e só as letras "Ext" aparecem.
- A aba ativa "Helper (chat)" (marcado=True, 1004–1107) não aparece na tela: fica sobreposta a "Files" (992–1043) e "Changes" (1043–1120), então não se vê qual aba do grupo direito está selecionada.
- "Files" e "Helper (chat)" estão marcados ao mesmo tempo (marcado=True), um indicador ambíguo de aba ativa.
- A faixa do grupo direito (a partir de x=992) se sobrepõe à última aba do grupo central, "Extension panels" (878–1004).
- A aba "Extension panels" do grupo esquerdo (211–337) passa do divisor em x≈304 e o texto aparece truncado ("Extension pan"); a do grupo central (878–1004) passa do divisor em x≈972 e também aparece truncada ("Extension pane").
- O texto do estado vazio do painel direito ("No active bench. Open a bench to see its files.", 1004–1256) aparece na imagem apenas a partir de "Open a bench...", com o início da frase não visível.
- A linha de composição do chat avança sobre a região do painel direito: o MultilineTextInput vai até x=1099 e o botão "Send" (1045–1099) fica além do divisor 2 (972–988).
- O campo de texto sem nome acessível exibe o texto em inglês "ANTHROPIC_MODEL" (identificador de variável de ambiente, não traduzido).
- TextInput e MultilineTextInput sem nome acessível.
- Botão de tema (sol, 1101–1133) e os rótulos de ícone em (1116,114) e (168,158) sem nome acessível.
- O rótulo "Claude Code · D:\a\forja\forja ·" termina com um separador "·" solto.

## [en.dark.bench-tab.diff.tab](pages/en.dark.bench-tab.diff.tab.md)

- "Extension panels" do painel direito (1240–1366) ultrapassa a largura da janela (1280 px) e aparece cortada ("Ext…"); "Extension panels" do painel central (878–1004) passa do divisor em x≈972 e também é cortada.
- "Previous change" (971–1096) fica além do divisor central e não é visível; "Next change" (324,143) quebrou para a segunda linha da barra do diff — o par de navegação anterior/próxima fica separado e um dos dois some.
- Sobreposição de abas no painel direito: "Helper (chat)" (1004–1107) sobre "Files" (992–1043) e "Changes" (1043–1120) sobre "Helper (chat)".
- Aba "Extension panels" do painel esquerdo (211–337) é cortada pela borda do painel em x≈304, exibindo "Extension pan…".
- Botões da barra superior, divisores, faixas de abas, sublinhados, logo e ícones de estado vazio sem nome acessível (o alternador de tema em (1101,5)–(1133,37) é o caso mais relevante, pois é um comando sem rótulo).
- Controles inconsistentes: "Ignore whitespace" é desenhado como círculo (estilo botão de opção) e "Collapse unchanged" como caixa marcada, embora ambos sejam CheckBox independentes.
- Mensagens de estado vazio em cinza itálico ("Pick a file to see its diff.", "No repository changes loaded. Press Refresh.", "No project yet. Add an existing folder or create a new one.") têm contraste baixo sobre o fundo escuro; "No repository changes loaded. Press Refresh." quebra em duas linhas de forma desajeitada ("Press / Refresh."). "Clear" desativado tem contraste reduzido (esperado para estado desabilitado).

## [en.dark.bench-tab.editor.tab](pages/en.dark.bench-tab.editor.tab.md)

- Sobreposição de abas: no limite esquerdo, "Extension panels" do painel lateral (211,50)–(337,82) invade a faixa do grupo central e passa por baixo de "Terminal" (324,50)–(401,82).
- Sobreposição entre grupos central e direito: "Extension panels" do grupo central (878,50)–(1004,82) invade a faixa do grupo direito (992,50)–(1268,82) e passa por baixo de "Files".
- Grupo de abas direito com fileiras sobrepostas: "Files" (992,50)–(1043,82) sob "Helper (chat)" (1004,50)–(1107,82) e "Helper (chat)" sob "Changes" (1043,50)–(1120,82) na faixa 1043–1107.
- Elemento fora da janela: "Extension panels" (1240,50)–(1366,82) estende-se além da largura de 1280 px e aparece cortado na borda direita.
- Botão sem nome acessível: o ícone de tema na barra superior (1101,5)–(1133,37) é um Unknown clicável sem rótulo.
- Ícones/imagens sem alternativa textual: logotipo (12,50)–(40,78) e os ícones de estado vazio (1116,114) e (168,158) aparecem como "(sem nome acessível)".
- Divisores verticais (304,42)–(320,768) e (972,42)–(988,768) são Unknown sem nome acessível, o que dificulta identificação por leitor de tela.
- Textos de estado vazio em cinza sobre fundo escuro têm contraste baixo ("Open a file in the explorer or press Ctrl+P.").

## [en.dark.rail.extension.page](pages/en.dark.rail.extension.page.md)

- A aba "Extension panels" do painel lateral esquerdo (botão em (211,50)–(337,82)) é cortada pelo limite do painel em ~x=304: na imagem aparece só "Extension pan", embora o retângulo acessível vá até x=337.
- A aba "Extension panels" do painel direito (botão em (1240,50)–(1366,82)) tem retângulo além da largura da janela (1280 px); o rótulo fica fora da área visível e inacessível.
- O botão de alternar tema na barra superior (1101,5)–(1133,37) não tem nome acessível; só se identifica pela imagem do sol.
- O campo de busca de extensões (324,146)–(968,178) é um TextInput sem nome acessível; o rótulo "Search" fica fora do campo, sem associação.
- O logotipo do app (imagem em 12,50) e os três ícones de estado vazio (1116,114), (168,158) e (632,202) estão sem nome acessível.
- A instrução "Put a folder with extension.toml and init.lua in ~/.forja/extensions." é exposta com papel de Botão; semanticamente é um texto de ajuda, o que pode confundir leitores de tela.

## [en.dark.right-tab.extension.panels.right](pages/en.dark.right-tab.extension.panels.right.md)

- Nome acessível "(ícone)" no rótulo da pasta do estado vazio (168,161)–(196,189): texto em português na variante em inglês (vazamento de i18n) e sem valor descritivo.
- Botão de tema (sol) na barra superior (1741,5)–(1773,37) sem nome acessível; idem nos ícones de fechar/dividir/maximizar/busca do terminal, na aba "pwsh", na bolinha de status, nos dois divisores verticais e na área de conteúdo do terminal.
- Botões de janela nomeados apenas com os símbolos "–", "□" e "×", sem rótulos descritivos.
- As três faixas de abas exibem o botão "More tabs" mesmo com poucas abas visíveis ("Projects" à esquerda; "Terminal" a "Helper (chat)" no centro; "Extension panels" e "Files" à direita), sem indicação de quais abas estariam ocultas no menu de estouro.
- Diferença em relação à referência pt-BR (constato, não defeito): as sobreposições de abas e a aba ativa fora da janela descritas na referência não ocorrem nesta variante; o estado declarado (slot direito "Extension panels" ativo e visível) confere com a imagem.

## [en.dark.bench-tab.extension.panels.bench](pages/en.dark.bench-tab.extension.panels.bench.md)

- Estouro de largura: o botão "Extension panels" do painel direito (1240,50)–(1366,82) tem retângulo além da largura da janela (1280 px) e fica cortado ou invisível.
- Sobreposição de abas no painel direito: "Helper (chat)" (1004,50)–(1107,82) sobrepõe "Files" (992,50)–(1043,82) e "Changes" (1043,50)–(1120,82), indicando dois grupos de abas desenhados na mesma faixa.
- A aba central "Extension panels" (878,50)–(1004,82) cruza o divisor direito (x≈972–988) e avança sobre a faixa de abas do painel direito (992,50)–(1268,82).
- Texto cortado por transbordo de painel: a aba "Extension panels" do painel esquerdo (211,50)–(337,82) cruza o divisor em x≈304–320 e aparece truncada ("Extension pa") na borda do painel.
- Nomes acessíveis duplicados e ambíguos: três botões "Extension panels" (painel esquerdo, área central e painel direito) e dois botões "Changes" (área central e painel direito) com o mesmo texto.
- Elementos sem nome acessível: alternador de tema (1101,5), os dois divisores verticais (304,42 e 972,42), as faixas dos grupos de abas e os indicadores de sublinhado — apenas "Unknown"/(sem nome acessível) na árvore; o logotipo (Image) e os ícones de estado vazio também não têm nome.
- Botões de janela nomeados apenas pelos símbolos "–", "□" e "×", sem rótulo descritivo (minimizar/maximizar/fechar).

## [en.dark.right-tab.git.changes](pages/en.dark.right-tab.git.changes.md)

- Botões de janela rotulados apenas com os caracteres "–", "□" e "×" como nome acessível.
- Vários controles sem nome acessível: alternador de tema (1741,5)–(1773,37), os quatro botões de ícone da barra do terminal (444–581), o chip "pwsh", os divisores verticais, o logotipo e a área do terminal.
- A dica "Click to type" (≈1508,1016–1597,1028), visível na imagem dentro do terminal, não tem entrada na árvore de acessibilidade.
- Faixas de abas dependem de botões genéricos "More tabs" (três ocorrências) para reaching abas ocultas; não fica acessível quais abas estão escondidas sem abrir o menu.
- Abas e textos inativos em cinza médio sobre fundo escuro (por exemplo "Editor", "Browser", "Preview", "Files") têm contraste baixo.

## [en.dark.bench-tab.git.history](pages/en.dark.bench-tab.git.history.md)

- Aba "Extension panels" (1240,50)–(1366,82) tem retângulo além da largura da janela (1280 px): fica parcialmente fora da área visível, aparecendo na imagem só como "Ext…".
- Retângulos de abas sobrepostos no grupo direito: "Helper (chat)" (1004–1107) sobre "Files" (992–1043) e "Changes" (1043–1120); "Helper (chat)" não aparece como aba visível na imagem, indício de elemento duplicado ou de grupo de abas sobreposto na árvore.
- Nomes ambíguos para leitores de tela: três botões chamados "Extension panels" (grupos esquerdo, central e direito), dois chamados "History" (aba central e sub-aba do painel Git) e dois chamados "Changes" (aba central e aba do grupo direito).
- Rótulos de aba truncados pelos divisores/limites de painel: "Extension pa…" (grupo esquerdo, corte em x≈300), "Extension pane…" (painel central, corte em x≈972) e "Ext…" (painel direito, corte na borda da janela em x=1280).
- Seis elementos sem nome acessível: os dois divisores verticais (304,42)–(320,768) e (972,42)–(988,768), as duas faixas de abas (64,50)–(300,82) e (992,50)–(1268,82), a Image do logotipo (12,50)–(40,78) e o botão de alternar tema (1101,5)–(1133,37), este identificável apenas pelo ícone de sol.
- Três Labels declarados como "(ícone)" sem nome acessível: os de (1116,114)–(1144,142), (168,158)–(196,186) e (702,212)–(730,240).
- O texto da interface está integralmente em inglês (locale "en"), conforme esperado nesta variante; nenhuma mistura de idiomas foi observada.

## [en.dark.modal.helper.profile_picker](pages/en.dark.modal.helper.profile_picker.md)

- A janela do modal vai até y=804, além da altura da imagem (800): a borda inferior do modal está cortada na captura.
- A aba "Extension panels" do painel direito (1240–1366) se estende além da largura da imagem (1280), fora da área visível.
- A linha de abas do painel direito tem retângulos sobrepostos e entradas duplicadas: "Files" (992–1043) e "Helper (chat)" (1004–1107) se sobrepõem — "Helper (chat)" não é visível na captura — e "Extension panels" aparece de novo em 1240.
- O modal sobrepõe a área central: as abas "Terminal", "Editor", "Browser", "Changes", "Preview" e os botões "+", "Recipes", "Shell" e os quatro controles sem nome continuam na árvore de acessibilidade mas estão invisíveis ao usuário.
- A aba "Extension panels" do painel esquerdo é truncada pelo modal ("Extension pai") e a ocorrência na área central é cortada pela borda do painel direito ("Extension pane").
- Botão de tema na barra superior (1101–1133), divisores verticais e controles da área central sem nome acessível.
- Rótulos de ícone expostos como "(ícone)" (grade e pasta dos estados vazios), sem nome acessível descritivo.
- Elemento de dimensão nula na árvore em (640,400)–(640,400).

## [en.dark.rail.helper.tab](pages/en.dark.rail.helper.tab.md)

- A lista de perfis não cabe na janela: os elementos a partir de "Launch" (502,790)–(570,825) e "Kimi Code CLI" (324,797)–(410,816) ficam abaixo da altura da imagem (800 px), e as linhas de Kiro CLI, OpenCode e Qwen Code ficam inteiramente fora dela, sem indício visível de rolagem além da barra estreita em x=597.
- Sobreposição na base da janela: o botão "Launch" (456,747)–(524,782) e a área da lista invadem a faixa da barra de status (y≥768), onde estão "Ready", "56 modules" e "Notifications" (y 775–794).
- Texto truncado: a aba "Extension panels" (211,50)–(337,82) ultrapassa o limite do painel esquerdo (divisor em x 304–320) e aparece cortada como "Extension pai".
- Elemento fora da janela: a aba "Extension panels" do painel direito (1240,50)–(1366,82) fica parcialmente além da largura de 1280 px, visível apenas como "Ext".
- Botões de janela com nomes acessíveis simbólicos "–", "□" e "×" em vez de rótulos descritivos (Minimize/Maximize/Close).
- Vários elementos sem nome acessível: dois divisores, o contêiner de abas de cada painel, o botão de tema e as imagens/ícones de estado vazio (registrados como "(sem nome acessível)" ou "(ícone)").
- Dezesseis botões com o mesmo nome acessível "Launch" na mesma tela, o que dificulta a identificação por leitor de tela (o alvo só é inferido pela linha do perfil).

## [en.dark.rail.muster.page](pages/en.dark.rail.muster.page.md)

- O botão "Extension panels" da tira direita tem retângulo (1240,50)–(1366,82), que ultrapassa a largura da janela (1280 px); o texto fica cortado fora da área visível.
- O botão "Extension panels" da tira esquerda (211,50)–(337,82) invade a região do divisor em (304,42)–(320,768); o rótulo aparece truncado ("Extension pa") no painel.
- Dois botões com o mesmo nome acessível "Extension panels" em abas diferentes, o que dificulta a identificação por leitores de tela.
- Botão de alternância de tema e as duas divisórias verticais estão sem nome acessível.
- Textos de estado vazio e subtítulo em cinza sobre fundo escuro têm contraste reduzido (por exemplo, "Tasks, helpers and notes in one place.").

## [en.dark.right-tab.ports.panel](pages/en.dark.right-tab.ports.panel.md)

- Botões sem nome acessível: alternância de tema (1741,5)–(1773,37) e os quatro ícones da barra do terminal (fechar, dividir, sessão e busca; (444,113)–(476,145), (479,113)–(511,145), (514,113)–(546,145), (549,113)–(581,145)).
- O chip da sessão "pwsh" (328,118)–(404,140) e seu ponto de status (385,121)–(401,137) não têm nome acessível.
- Os divisores (304,42)–(320,1048) e (1612,42)–(1628,1048) e o logotipo (12,50)–(40,78) não têm nome acessível.
- A superfície do terminal (328,150)–(1604,1036) é uma grande área clicável sem nome acessível.
- O rótulo "(ícone)" do estado vazio (168,161)–(196,189) tem um nome de acessibilidade de marcação (placeholder), sem significado.
- Três botões compartilham o mesmo nome "More tabs" ((64,50), (324,50) e (1632,50)), o que é ambíguo para navegação por leitor de tela.
- Vários Labels expõem ações de clique e/ou foco, papel incoerente com o tipo: "No project", "Ports", "0 listening", "Ready", "56 modules", "Notifications", "No listening TCP port was found." e "No project yet. Add an existing folder or create a new one.".
- Três abas aparecem simultaneamente com marcado=True ("Projects", "Terminal" e "Ports"), o que torna ambíguo qual painel está de fato selecionado.
- O pequeno indicador de giro ao lado de "Refresh" (≈(1830,118)–(1848,134)) e o texto "Click to type" no canto do terminal (≈(1536,1016)–(1596,1030)) aparecem na imagem, mas não têm elemento correspondente na árvore de acessibilidade.

## [en.dark.bench-tab.review.tab](pages/en.dark.bench-tab.review.tab.md)

- O formulário "New pull request" está cortado: os campos "Title" e "Description" têm retângulos até x=1107, enquanto o painel central termina em ~972 px; ambos são cortados na borda do painel.
- A aba central "Extension panels" (878,50)–(1004,82) atravessa o divisor em x=972 e se sobrepõe à faixa de abas do painel direito (992,50)–(1268,82); na imagem o texto aparece truncado ("Extension pan").
- A aba "Extension panels" do painel direito (1240,50)–(1366,82) estende-se além da largura da janela (1280 px) e fica fora da imagem; na imagem só se vê o início ("Ext").
- Sobreposição no conjunto de abas do painel direito: "Helper (chat)" (1004,50)–(1107,82) sobrepõe "Files" (992–1043), "Changes" (1043–1120) e parte de "Search"; na imagem "Helper (chat)" não é visível.
- A aba "Extension panels" do painel esquerdo (211,50)–(337,82) atravessa o divisor em x=304 e se sobrepõe à aba central "Terminal" (324,50)–(401,82); na imagem o texto aparece truncado como "Extension pa".
- O rótulo "Source branch:" é seguido diretamente do campo que exibe o texto "Title"; não há campo visível para informar a branch de origem.
- Campos de texto sem nome acessível: "Title" (324,211), "Description" (324,242), Base (360,407), Revisores (393,447) e Labels (675,447).
- Elementos interativos sem nome acessível: ícone de tema na barra superior (1101,5), divisores dos painéis (304,42) e (972,42), faixas de abas (64,50) e (992,50), além da imagem do logotipo.
- Os divisores verticais (304,42) e (972,42) e as faixas de abas só expõem a ação "focus", sem ação de arrastar/clique associada.

## [en.dark.rail.routine.page](pages/en.dark.rail.routine.page.md)

- O botão "Extension panels" do painel direito tem o retângulo (1240,50)–(1366,82) estendido além da largura da imagem/janela (1280 px): parte do elemento fica fora da tela, e o texto visível aparece truncado como "Ext".
- O texto da aba "Extension panels" do painel esquerdo (211,50)–(337,82) é cortado visualmente pela área central, aparecendo como "Extension pa" na captura.
- Botões-ícone do trilho ("Board", "Helpers", "Muster", "Requests", "Skills", "Usage", "Extensions", "Archive") dependem exclusivamente de ícone na interface; o nome acessível existe na árvore, mas não há rótulo visível nem dica na tela.
- Os três ícones ilustrativos dos estados vazios, a logomarca, o ícone de tema e os divisores são expostos como "(sem nome acessível)", sem descrição para leitores de tela.

## [en.dark.right-tab.search.panel](pages/en.dark.right-tab.search.panel.md)

- Os quatro campos de texto do painel Search (busca, incluir, excluir e substituir) não têm nome acessível; só são identificáveis pelo placeholder ("Search in files", "Include: *.rs, src/**", "Exclude: target/**", "Replace with").
- Os quatro botões-ícone da barra do terminal ((444,113)–(476,145) a (549,113)–(581,145)) e o ícone de tema da barra superior ((1741,5)–(1773,37)) são elementos Unknown sem nome acessível.
- O chip "pwsh" ((328,118)–(404,140)) contém um segundo elemento Unknown sobreposto ((385,121)–(401,137)), duplicando a região na árvore.
- Botões "Search", "Stop", "Preview" e "Apply" estão desativados sem tooltip; a razão só aparece no texto "Open a bench first." abaixo dos botões.
- A dica "Click to type", visível no canto inferior direito do terminal, não aparece na árvore de acessibilidade.
- Os nomes acessíveis "Find file  Ctrl+P" e "Anvil  Ctrl+K" contêm espaços duplos entre o rótulo e o atalho, o que pode soar truncado em leitores de tela.

## [en.dark.rail.skill.page](pages/en.dark.rail.skill.page.md)

- O campo de busca "Search skills" é um TextInput sem nome acessível; o texto de sugestão só aparece visualmente.
- O ícone de alternância de tema na barra superior (1741,5)–(1773,37) é um elemento clicável sem nome acessível; a função só é identificável pela imagem.
- Os dois botões de abas recolhidas têm o mesmo nome acessível ("More tabs"), um em cada painel; leitores de tela não distinguem qual painel cada um controla.
- Os botões "Files" (1727,52)–(1778,84) e "Changes" (1778,52)–(1855,84) têm retângulos que se tocam em x=1778, sem folga entre eles.
- Os divisores verticais são elementos sem nome acessível.
- Os três ícones de estado vazio aparecem como rótulos "(ícone)" sem descrição de texto alternativo.
- Nenhum texto cortado ou fora da janela: todos os retângulos estão dentro de 1920×1080 px.

## [en.dark.rail.usage.page](pages/en.dark.rail.usage.page.md)

- O botão de alternância de tema na barra superior (1741,5)–(1773,37) não tem nome acessível (apenas ícone de sol).
- Os botões de janela usam os símbolos "–", "□" e "×" como nome acessível, em vez de rótulos descritivos ("Minimize", "Maximize", "Close").
- O logotipo do app (12,50)–(40,78) é uma imagem sem nome acessível.
- As demais abas dos painéis esquerdo e direito (equivalentes a "Bancadas", "Painéis de extensões", "Busca" e "Portas" do pt-BR) não aparecem na árvore de acessibilidade nesta variante; somente os botões "More tabs" que as agrupam.
- O botão "Back to bench" é exibido no topo da área central mesmo sem bancada ativa, enquanto o painel direito mostra "No active bench. Open a bench to see its files.".

## [en.dark.settings.accounts](pages/en.dark.settings.accounts.md)

- O conteúdo do segundo cartão é cortado pela borda inferior do modal: o aviso "helper executable not found" (354,482)–(503,498) ultrapassa o fim do modal (y=496) e aparece visualmente truncado; o botão "Add account" (550,506)–(652,538) e o campo "Name" (354,511)–(542,534) do cartão Codex ficam totalmente fora da área visível do modal, apesar de existirem na árvore.
- O modal "Settings" (328,58)–(1026,496) se sobrepõe à barra de abas central (y 50–85) e à barra de ferramentas, escondendo os botões "More tabs" central, "+", os quatro ícones, "Recipes", "Shell" e as abas "Terminal" a "History", além de parte de "Extension panels" (só "ion panels" fica visível).
- A aba "Terminal" está marcada como ativa (marcado=True), mas não é visível por estar sob o modal.
- Nomes duplicados na árvore prejudicam leitores de tela: três "More tabs", dois "Changes", dois "Add account" e dois "helper executable not found".
- Os campos de busca de ajustes e os dois campos "Name" são TextInput sem nome acessível; o rótulo visível é apenas o placeholder.
- O alternador de tema na barra superior e vários ícones da barra de ferramentas central não têm nome acessível.
- Os botões "Find file  Ctrl+P" e "Anvil  Ctrl+K" têm dois espaços entre o rótulo e o atalho.

## [en.dark.settings.content](pages/en.dark.settings.content.md)

- Botão-ícone de tema (1741,5)–(1773,37) na barra superior sem nome acessível.
- Os três botões "More tabs" têm o mesmo nome acessível em faixas diferentes, sem indicar qual faixa representam.
- Grupo de quatro botões-ícone sem rótulo (444–581,113–145) na faixa de terminais, totalmente encobertos pela modal.
- A modal encobre a faixa de abas central, incluindo a aba ativa "Terminal" (marcada), e os botões "More tabs", "Recipes" e "Shell" dessa faixa.
- A aba "Helper (chat)" (1099,52)–(1202,84) da faixa central só aparece parcialmente à direita da modal; "Extension panels" (973,52)–(1099,84) fica cortada pela borda da modal.
- O texto visível "Click to type" no rodapé da área central (aprox. 1530,1017–1600,1030) não corresponde a nenhum elemento nomeado na árvore de acessibilidade.
- Caminhos dos diagnósticos exibidos com separadores misturados e trecho relativo cru ("…\modules\content\../../../content\templates\…"), difíceis de ler.
- Textos secundários em cinza médio sobre fundo escuro (por exemplo, "rev 1" e os caminhos dos diagnósticos) têm contraste baixo.
- O rodapé da lista de diagnósticos ("missing `+++` frontmatter" em (337,472)–(499,491)) encosta na borda inferior da janela modal (496 px), sem folga visível.

## [en.dark.settings.counter_github](pages/en.dark.settings.counter_github.md)

- O campo de busca e o campo de token não têm nome acessível (dependem apenas dos placeholders "Search settings" e "Token").
- O ícone de tema (sol) da barra superior, os quatro botões-ícone da barra do editor (444–581 px) e o logotipo (Image) estão sem nome acessível.
- O modal sobrepõe o "More tabs" da área central, as abas de "Terminal" a "Extension panels" (esta só aparece como "…ion panels"), a barra de ferramentas do editor ("+", ícones, "Recipes", "Shell") e o topo da área de edição.
- Os retângulos de controles ocultos atrás do modal, (328,118)–(404,140) e (385,121)–(401,137), se sobrepõem ao retângulo do campo de busca do próprio modal.
- Texto "Click to type" visível na área central sem elemento correspondente na árvore de acessibilidade.
- Textos de apoio em cinza médio sobre fundo escuro (placeholders "Search settings" e "Token", "Not connected", mensagens de estado vazio e a dica "Click to type") têm contraste baixo; os botões desativados "Connect" e "Disconnect" também perdem contraste.

## [en.dark.settings.counter_gitlab](pages/en.dark.settings.counter_gitlab.md)

- A janela "Settings" sobrepõe a barra de abas central ("Terminal", "Editor", "Browser", "Changes", "Preview", "Bench", "Review" e "History" ficam totalmente encobertas; de "Extension panels" resta só o fragmento "…ion panels" à direita do modal) e a barra de ferramentas central ("+", quatro botões-ícone sem nome, "Recipes", "Shell"); o botão "More tabs" da área central ((324,50)–(411,85)) também fica praticamente inatingível.
- Três botões compartilham o mesmo nome acessível "More tabs" e dois compartilham "Changes" (ambiguidade para leitores de tela).
- Quatro botões-ícone da barra de ferramentas central ((444,113), (479,113), (514,113), (549,113)) estão sem nome acessível, e o texto "Click to type" do painel central não tem elemento acessível correspondente.
- O controle de alternância de tema ((1741,5)–(1773,37), ícone de sol) é um elemento sem nome acessível e sem papel definido (Unknown).
- O botão "Connect" está desativado, mas com preenchimento âmbar que o faz parecer acionável; os botões desativados ("Connect", "Disconnect") têm contraste reduzido.
- Os nomes "Find file  Ctrl+P" e "Anvil  Ctrl+K" contêm espaço duplo entre o rótulo e o atalho.

## [en.dark.settings.counter_jira](pages/en.dark.settings.counter_jira.md)

- Três botões com o mesmo nome acessível "More tabs" em posições distintas ((64,50), (324,50) e (1632,50)); nomes duplicados dificultam navegação e automação por acessibilidade.
- Dois botões "Changes" com o mesmo nome acessível em painéis diferentes ((629,52) na área central e (1778,52) no painel direito).
- Ícone clicável de tema (sol) em (1741,5)–(1773,37) sem nome acessível.
- Vários elementos interativos sem nome acessível: campos de texto e de senha (só identificáveis pelos textos de apoio visíveis), botões de ícone em (444,113)–(581,145), alças de redimensionamento e divisórias da modal e sublinhado de aba.
- A modal cobre a faixa de abas centrais: as abas "Terminal" a "History" ficam encobertas, "Extension panels" aparece truncada ("ion panels") e o botão "More tabs" central (324,50)–(411,85) fica invisível; os botões "+", "Recipes", "Shell" e os quatro botões de ícone (faixa y113–145) também ficam sob a modal — sobreposição esperada de uma modal, mas que reduz a legibilidade dos rótulos.
- Os elementos sem nome (328,118)–(404,140) e (385,121)–(401,137) se sobrepõem ao campo de busca da modal.
- Texto "Click to type" solto no canto inferior direito da área central (dica do painel de terminal vazio atrás da modal), sem elemento correspondente nomeado na árvore.

## [en.dark.settings.counter_linear](pages/en.dark.settings.counter_linear.md)

- A aba "Extension panels" (973,52)–(1099,84) é parcialmente encoberta pelo modal (o modal vai até x=1026); na imagem só se lê "ion panels".
- Os botões "+", os quatro botões-ícone entre 444–581 px, "Recipes" e "Shell" (y 113–145) ficam totalmente encobertos pelo modal, sem indicação visual de sua existência.
- Sobreposição na árvore de acessibilidade: "Helper (chat)" (1099,52)–(1202,84) se sobrepõe à faixa de abas que inclui "Extension panels".
- Três abas aparecem marcadas simultaneamente ("Projects" no painel esquerdo, "Terminal" na área central e "Files" no painel direito), o que dificulta identificar o painel realmente ativo.
- O TextInput de busca e o PasswordInput de token não têm nome acessível; dependem apenas do texto de sugestão ("Search settings", "Token").
- Vários elementos sem nome acessível: botão de tema na barra de título, bordas e alças do modal, bordas das faixas de abas e os quatro botões-ícone da barra central.
- O texto "Click to type" (~1565,1023) é exibido em cinza escuro sobre fundo escuro, com contraste baixo.
- Nenhum elemento ultrapassa os limites da janela (1920×1080); na variante pt-BR havia uma aba fora da borda, o que não ocorre aqui por causa da largura maior.

## [en.dark.settings.helper](pages/en.dark.settings.helper.md)

- A árvore lista conteúdo do modal até y≈3624, muito além do retângulo da janela "Settings" (termina em y=496) e da altura da imagem (1080 px); os cartões abaixo de Amp só são acessíveis rolando, e o único indicador é uma barra de rolagem de 2 px ((1015,167)–(1017,487)).
- "Enabled" (Cline CLI) (398,1064)–(465,1096) ultrapassa a borda inferior da imagem e "Cline CLI" (337,1071)–(390,1090) é cortado nela.
- Botão de tema/sol (1741,5)–(1773,37) sem nome acessível.
- Quatro controles sem nome em (444,113)–(581,145) (barra do painel central, sob o modal).
- Todos os TextInput ("Search settings", "Extra arguments", "Environment") sem nome acessível ou rótulo associado.
- Oito cartões exibem "Loading…" persistente no estado de hooks (GitHub Copilot CLI, Crush, Gemini CLI, Goose, Kimi Code CLI, Kiro CLI, OpenCode, Qwen Code).
- Caminhos absolutos misturam separadores "\" e "/" (ex.: "C:\Users\runneradmin\.gemini/config/hooks.json").
- O botão "Settings" do trilho esquerdo permanece marcado=False enquanto a tela de ajustes está aberta.
- Cantos de redimensionamento do modal com retângulos parcialmente acima do topo declarado da janela ((319,49)–(339,69) e (1016,49)–(1036,69), contra topo em y=58).
- As abas centrais "Terminal" a "History" e o "More tabs" do centro ficam encobertos pelo modal aberto.

## [en.dark.settings.import](pages/en.dark.settings.import.md)

- O diálogo "Settings" cobre as abas "Editor", "Browser", "Changes", "Preview", "Bench", "Review" e "History" (e parte de "More tabs" e "Extension panels" centrais); esperado num modal, mas a sobreposição não é indicada por sombra/contraste suficientes nas bordas.
- Vários controles sem nome acessível: o alternador de tema (1741,5), os quatro botões-ícone da barra do terminal (444–581,113), o campo de busca do diálogo, o item de navegação do diálogo (328,118)–(404,140) com seu ícone (385,121)–(401,137) e os divisores/alças, dificultando o uso por leitor de tela.
- A faixa sem nome (339,51)–(1018,67) sobrepõe a linha de abas da área central atrás do diálogo.
- O aviso "Click to type" (visível em ~(1535,1023), dentro da região sem nome (328,150)–(1604,1036)) não aparece como elemento nomeado na árvore de acessibilidade.
- O item de navegação do diálogo (328,118)–(404,140) fica sob o campo de busca, sem texto legível na imagem.

## [en.dark.settings.mcp](pages/en.dark.settings.mcp.md)

- O conteúdo da janela modal "Settings" excede o retângulo declarado (328,58)–(1026,496): o snippet do "OpenCode" tem retângulo até y=544 e o aviso final até y=571; na imagem o snippet aparece cortado na borda inferior do modal e o aviso não é visível (é preciso rolar).
- A aba "Extension panels" (973,52)–(1099,84) é parcialmente coberta pela janela modal: só o trecho "…ion panels" do texto fica visível.
- O botão "More tabs" da área central (324,50)–(411,85) é parcialmente coberto pela barra de título do modal.
- A janela modal "Settings" sobrepõe a barra de ferramentas central: "+", os quatro botões sem nome (444–581,113), "Recipes" e "Shell" ficam ocultos atrás dela.
- O campo de busca do modal é um TextInput sem nome acessível (só o texto de apoio "Search settings" é visível).
- Botões sem nome acessível: o alternador de tema (ícone de sol, (1741,5)) e os quatro botões de ícone da barra do terminal ((444,113)–(581,145)).
- Ícones registrados apenas como "(ícone)" ((168,161) e (1756,117)) sem texto alternativo descritivo.
- Textos secundários ("Server · on: 52", "Helper setup" e os estados vazios) em cinza médio sobre fundo escuro, com contraste aparentemente baixo.

## [en.dark.settings.onboarding](pages/en.dark.settings.onboarding.md)

- Campo de busca de ajustes (TextInput em 337,121)–(1017,153) sem nome acessível; só o marcador visual "Search settings" identifica a função.
- Botão-ícone de tema (1741,5)–(1773,37) e o logotipo (Image em 12,50)–(40,78) sem nome acessível.
- Três botões compartilham o mesmo nome acessível "More tabs" ((64,50), (324,50) e (1632,50)), sem contexto que os distinga; o nome "Changes" também se repete (aba central em (629,52) e aba do painel direito em (1778,52)).
- A janela "Settings" sobrepõe a faixa de abas central: a aba ativa "Terminal" (419,52)–(496,84), o "More tabs" central (324,50)–(411,85) e parte de "Extension panels" (973,52)–(1099,84) ficam encobertos; "Extension panels" aparece truncado ("ion panels").
- Os botões "+" (407,113), "Recipes" (584,113) e "Shell" (658,113) e os quatro ícones sem nome da faixa central (444–581,113) ficam totalmente ocultos atrás da janela "Settings".
- Os elementos (328,118)–(404,140) e (385,121)–(401,137), no interior da janela sobre a área do campo de busca, não têm nome acessível nem conteúdo visível identificável.

## [en.dark.settings.remote](pages/en.dark.settings.remote.md)

- O retângulo da janela modal "Settings" (328,58)–(1026,496) não contém o próprio conteúdo: o botão "Save target" (337,491)–(430,523) fica cortado pelo limite inferior e toda a seção WSL — rótulo "WSL", botão "Refresh distros" e a mensagem "Distros not read yet. Refresh to list them." (y=545–631) — cai fora da janela e não é visível nem alcançável na captura (na referência pt-BR o modal ia até y=635 e tudo cabia).
- A aba "Extension panels" (973,52)–(1099,84) é parcialmente coberta pelo modal: só o fragmento "…ion panels" fica visível; as abas "Terminal", "Editor", "Browser", "Changes", "Preview", "Bench", "Review" e "History" ficam totalmente ocultas, assim como o botão "More tabs" central (324,50)–(411,85) fica parcialmente coberto (sobreposição esperada em modal, mas que esconde controles da tela de fundo).
- Os botões "+", "Recipes", "Shell" e os quatro botões de ícone sem nome da barra de projetos (444–581, 113–145) ficam totalmente escondidos atrás do modal.
- Os cinco campos de texto (busca "Search settings" e os quatro do formulário "New target") são TextInput "(sem nome acessível)": o rótulo existe apenas como texto visual/espaço reservado.
- Vários elementos sem nome acessível: alternador de tema da barra superior, divisores verticais, faixas de cabeçalho/abas, bordas do modal, barra de rolagem do conteúdo e os quatro botões de ícone da barra de projetos; os ícones de estado vazio são registrados apenas como "(ícone)".
- Os botões de janela da barra de título têm apenas símbolos como nome ("–", "□", "×").

## [en.dark.settings.trust](pages/en.dark.settings.trust.md)

- O diálogo "Settings" se sobrepõe à linha de abas da área central: as abas "Terminal" a "History" ficam encobertas e "Extension panels" aparece parcialmente cortada pelo canto do diálogo.
- O botão "More tabs" da área central, em (324,50)–(411,85), fica praticamente todo atrás do diálogo.
- A barra de ferramentas central ("+", os quatro ícones sem nome, "Recipes" e "Shell", y 113–145) fica inteiramente coberta pelo diálogo nesta captura.
- O TextInput do campo de busca de ajustes (337,121)–(1017,153) não tem nome acessível, embora exiba o texto de apoio "Search settings".
- O alternador de tema do topo (1741,5)–(1773,37) e os quatro ícones da barra central (444–581, y 113–145) não têm nome acessível.
- Nomes repetidos em elementos distintos: "More tabs" (3 botões), "Changes" (2 abas) e "Settings" (janela, rótulo e botão), o que dificulta a identificação por leitores de tela.
- Textos secundários em cinza médio sobre fundo escuro ("Pending requests: 0", "Nothing approved yet.", "No project yet. Add an existing folder or create a new one.", "No active bench. Open a bench to see its files." e o aviso "Click to type") têm baixo contraste.

## [en.dark.settings.voice](pages/en.dark.settings.voice.md)

- Conteúdo da modal cortado: o PasswordInput da chave (337,476)–(665,499) passa do retângulo da janela "Settings" (328,58)–(1026,496) e o placeholder "Paste the key here" aparece truncado na base.
- Os botões "Save key" (337,507)–(414,539) e "Delete key" (422,507)–(511,539) ficam abaixo do retângulo da modal e não são visíveis na captura — ações essenciais fora da área da janela.
- A barra de ferramentas central ("+", quatro botões-ícone sem nome, "Recipes" e "Shell", y 113–145) e as abas "Terminal" a "History" (y 52–84) estão inteiramente atrás da modal; "Extension panels" (973,52)–(1099,84) fica parcialmente coberta por ela.
- O botão "More tabs" central (324,50)–(411,85) fica quase todo encoberto pela modal.
- TextInput de busca de ajustes (337,121)–(1017,153) e PasswordInput da chave (337,476)–(665,499) não têm nome acessível; dependem apenas do placeholder.
- Vários botões-ícone sem nome acessível: barra de ferramentas central (444–581, y 113–145), ícone de aba do terminal (328,118)–(404,140) e elemento dentro do campo de busca (385,121)–(401,137), o que impede identificação por leitor de tela.
- O botão "Settings" do trilho permanece marcado=False enquanto a janela "Settings" está aberta em foco, inconsistência de estado de seleção.
- Textos secundários e placeholders em cinza médio sobre fundo escuro apresentam contraste baixo (por exemplo "No key saved.", "Search settings", "Paste the key here").

## [en.light.home.default](pages/en.light.home.default.md)

- Botões sem nome acessível (acessibilidade): o alternador de tema (1741,5)–(1773,37) e os quatro ícones da barra do terminal (444,113)–(581,145) não expõem nome e dependem de reconhecimento visual, sem rótulo ou tooltip acessível na árvore.
- A pílula de sessão "pwsh" (328,118)–(404,140) e seu ponto de status (385,121)–(401,137), os dois divisores de painel (304,42)–(320,1048) e (1612,42)–(1628,1048) e a área do terminal (328,150)–(1604,1036) também não expõem nome acessível.
- O texto "Click to type" no canto inferior direito da área do terminal (~ (1537,1017)–(1597,1029)) não corresponde a nenhum elemento da árvore de acessibilidade e tem contraste baixo (cinza claro sobre fundo claro).
- Três botões "More tabs" são exibidos mesmo com todas as abas visíveis em suas faixas — o painel esquerdo tem uma única aba ("Projects") — o que aparenta redundância visual/funcional.
- Estados com pixels idênticos a esta captura incluem os modais bench.dialogs, counter.poller, mcp.approval, notify.toasts, onboarding.wizard, project.dialogs e voice.overlay, além de bench-tab.terminal.tab e right-tab.explorer.panel: nessas capturas nenhum diálogo, aviso, sobreposição ou painel de explorer diferente aparece renderizado.

## [en.light.open.anvil](pages/en.light.open.anvil.md)

- Vários elementos têm retângulos além dos limites da captura (1920×1080): as linhas e chips da lista da Bigorna se estendem até y=1812 (ex.: "Author name" (650,1780)–(1270,1812)) — do "Review bench changes" (y=580) em diante nada é renderizado, primeiro porque fica abaixo da borda da janela da Bigorna (que termina em y=585) e depois porque fica fora da imagem.
- Sobreposição de retângulos dentro da Bigorna: "Prepare PR description" (650,500)–(1270,532) e "Refactor for clarity" (650,540)–(1270,572) sobrepõem a seção "Terminal recipes" e o chip "Build Go packages  ·  project" sem aparecerem na tela (retângulos de lista dessincronizados).
- A aba "Recipes" (584,113)–(655,145) é cortada pela borda esquerda da janela da Bigorna e "Shell" (658,113)–(711,145) fica totalmente oculta atrás dela; as abas "Extension panels" e "Helper (chat)" (y 52–84) têm sua borda inferior coberta pela janela (que começa em y=72) e aparecem esmaecidas.
- Botões de abas com nomes ambíguos para leitores de tela: três botões "More tabs" e dois botões "Changes" com o mesmo nome.
- Elementos interativos sem nome acessível: o botão de alternar tema (ícone de sol, (1741,5)–(1773,37)), o campo de busca da Bigorna, as linhas de resultado da busca, os quatro ícones da barra de ferramentas do terminal ((444,113)–(581,145)), a aba "pwsh" e seus elementos internos, e os dois divisores laterais.
- Os rótulos de ícone dos estados vazios aparecem apenas como "(ícone)", sem nome descritivo.
- As barras de rolagem das listas têm apenas 2 px de largura ((1268,220)–(1270,460), (890,482)–(892,546) e (890,509)–(892,573)), muito finas e de baixo contraste.
- O rótulo "No project" (11,12)–(77,31) tem 66 px de largura, menos do que o texto visível ocupa na imagem, indicando retângulo acessível menor que o renderizado.

## [en.light.open.onboarding](pages/en.light.open.onboarding.md)

- O botão de alternar tema (ícone de sol, (1741,5)–(1773,37)) está sem nome acessível.
- Os quatro botões de ícone da barra do terminal — encerrar (X), limpar, dividir e buscar — ((444,113)–(476,145), (479,113)–(511,145), (514,113)–(546,145), (549,113)–(581,145)) estão sem nome acessível.
- A aba de sessão "pwsh" ((328,118)–(404,140)) e seu X de fechar ((385,121)–(401,137)) estão sem nome acessível.
- O elemento associado à seleção de idioma está sem nome acessível e tem tamanho zero ((960,540)–(960,540)).
- Três botões compartilham o mesmo nome acessível "More tabs" ((64,50)–(151,85), (324,50)–(411,85) e (1632,50)–(1719,85)), sem indicar a qual faixa de abas pertencem.
- A dica "Click to type", no canto inferior direito do terminal, é pequena e está em cinza-claro sobre fundo branco — baixo contraste.

## [en.light.open.toast](pages/en.light.open.toast.md)

- O botão de alternância de tema (1741,5)–(1773,37) e os quatro botões de ícone da barra do terminal ((444,113) a (581,145)) não têm nome acessível.
- Os dois divisores arrastáveis ((304,42)–(320,1048) e (1612,42)–(1628,1048)) recebem foco por teclado, mas não têm nome acessível.
- Há dois botões nomeados apenas "×" (fechar janela em (1875,5) e fechar toast em (1870,983)), o que os torna ambíguos para leitores de tela.
- Três botões compartilham o nome "More tabs" ((64,50), (324,50) e (1632,50)) sem indicar a qual painel pertencem.
- O texto "Click to type", visível no canto inferior direito da área do terminal, não aparece na árvore de acessibilidade.
- Elemento de dimensão zero (960,540)–(960,540) sem nome na área central.

## [en.light.bench-tab.bench.detail](pages/en.light.bench-tab.bench.detail.md)

- O botão de tema na barra superior (1741,5)–(1773,37) é um Unknown sem nome acessível; só é identificável pelo ícone de sol.
- As duas divisórias verticais (304,42)–(320,1048) e (1612,42)–(1628,1048), o logo do trilho (12,50)–(40,78) e os três ícones de estado vazio estão sem nome acessível, o que reduz a navegabilidade por leitor de tela.
- Três botões compartilham o mesmo nome acessível "More tabs" em painéis diferentes, sem contexto na árvore que os distinga.

## [en.light.bench-tab.browser.tab](pages/en.light.bench-tab.browser.tab.md)

- Texto não traduzido no idioma "en": a URL inicial "forja://inicio" usa o português ("inicio"); o esperado seria algo como "forja://home".
- O aviso "The page is hidden while a panel or window is on top." (324,1048)–(609,1064) começa exatamente na borda da barra de status (y=1048), invadindo visualmente a faixa onde ficam "Ready" e "56 modules".
- Botões de janela nomeados apenas por símbolo: "–", "□" e "×", sem nomes acessíveis descritivos (Minimize/Maximize/Close).
- Botão "x" do chip tem nome não descritivo (letra minúscula isolada); o botão "R" da barra de navegação também não indica "Reload".
- O botão "More tabs" aparece três vezes com o mesmo nome, sem indicar a qual faixa de abas pertence.
- Nomes duplicados sem diferenciação: "History" (aba do painel central e botão do navegador), "Changes" (painéis central e direito) e os dois rótulos "(ícone)".
- Elementos sem nome acessível: alternador de tema (1741,5)–(1773,37), os dois divisores verticais, o logotipo (12,50)–(40,78) e os dois rótulos "(ícone)".
- As faixas de abas dependem de "More tabs": no painel central só "Browser" fica visível e as demais nove abas ficam escondidas atrás do menu suspenso.
- Baixo contraste relativo nos textos de estado vazio ("No project yet…", "No active bench…") e nas abas inativas, em cinza claro sobre fundo creme.

## [en.light.bench-tab.chat.tab](pages/en.light.bench-tab.chat.tab.md)

- Botão-ícone de tema (sol, 1741,5–1773,37) sem nome acessível (papel Unknown).
- TextInput (1249,145–1397,168) e MultilineTextInput (332,978–1600,1038) sem nome acessível.
- Divisores verticais (304,42) e (1612,42) e a imagem do logotipo (12,50) sem nome acessível.
- Rótulos de ícone em (1756,117) e (168,161) sem nome acessível.
- O campo do cabeçalho do chat mostra o valor técnico "ANTHROPIC_MODEL" sem rótulo visível nem nome acessível que explique sua função.
- O rótulo do rodapé do chat termina com um separador "·" solto ("Claude Code · D:\a\forja\forja ·").
- Os três grupos de abas exibem o botão "More tabs" mesmo com todas as abas visíveis (no grupo central cabem as 10 abas de 419 a 1202), tornando-o redundante.

## [en.light.bench-tab.diff.tab](pages/en.light.bench-tab.diff.tab.md)

- Botão da barra superior sem nome acessível em (1741,5)–(1773,37) (ícone de sol, alternador de tema); os dois divisores verticais, o logo do trilho e os ícones de estado vazio também não têm nome acessível.
- Três botões com o mesmo nome acessível "More tabs" (64–151, 324–411 e 1632–1719) e o nome "Changes" repetido em faixas diferentes (aba ativa do painel central em 419–496 e aba do painel direito em 1778–1855), sem contexto que os distinga para leitor de tela.
- A caixa "Ignore whitespace" (marcado=False) é desenhada como um círculo vazio, semelhante a um botão de opção, em vez do quadrado com marca usado em "Collapse unchanged", o que confunde o tipo do controle.
- Mensagens de estado vazio em cinza itálico ("Pick a file to see its diff.", "No repository changes loaded. Press Refresh.", "No project yet. Add an existing folder or create a new one.") têm contraste menor que o texto padrão.
- "Clear" desativado com contraste reduzido (esperado para estado desabilitado).

## [en.light.bench-tab.editor.tab](pages/en.light.bench-tab.editor.tab.md)

- Botão sem nome acessível: o ícone de tema na barra superior (1741,5)–(1773,37) é um Unknown clicável sem rótulo.
- Ícones/imagens sem alternativa textual: o logotipo (12,50)–(40,78) e os ícones de estado vazio (1756,117) e (168,161) aparecem como "(sem nome acessível)".
- Divisores verticais (304,42)–(320,1048) e (1612,42)–(1628,1048) são Unknown sem nome acessível, o que dificulta identificação por leitor de tela.
- Textos de estado vazio em cinza ("Open a file in the explorer or press Ctrl+P.", "No project yet. Add an existing folder or create a new one.", "No active bench. Open a bench to see its files.") têm contraste reduzido sobre o fundo claro.
- Diferenças em relação à referência (pt-BR.dark), por capturarem janelas de tamanhos distintos (1920×1080 vs 1024×768): nesta variante não há sobreposição de abas nem elementos fora da janela — na referência, as abas do grupo direito se sobrepunham e "Ajudante (chat)" ficava fora da largura; aqui o grupo direito mostra apenas "Files" e "Changes", e as abas excedentes ficam sob os botões "More tabs".

## [en.light.bench-tab.extension.panels.bench](pages/en.light.bench-tab.extension.panels.bench.md)

- Elementos sem nome acessível: alternador de tema (1741,5), os dois divisores verticais (304,42 e 1612,42), o logotipo (12,50) e os ícones de estado vazio (1756,117 e 168,161) — apenas "Unknown"/(sem nome acessível) na árvore.
- Botões de janela nomeados apenas pelos símbolos "–", "□" e "×", sem rótulo descritivo (minimizar/maximizar/fechar).
- Nome acessível duplicado e ambíguo: três botões "More tabs" (um por painel) e dois botões "Changes" (um na faixa central, outro no painel direito) com o mesmo texto.
- Contraste menor nos estados vazios: ícones de grade e pasta e as mensagens cinza em fundo claro ficam abaixo do destaque do restante da interface.
- Nenhum estouro de largura ou altura: todos os retângulos cabem na janela de 1920×1080 px — os estouros e sobreposições de abas registrados na referência (janela de 1053×768 px) não ocorrem nesta variante.

## [en.light.bench-tab.git.history](pages/en.light.bench-tab.git.history.md)

- O botão de alternar tema (1741,5)–(1773,37) é um "Unknown" sem nome acessível, identificável apenas pelo ícone de sol.
- Outros elementos sem nome acessível: os dois divisores verticais (304,42)–(320,1048) e (1612,42)–(1628,1048) e a Image do logotipo (12,50)–(40,78); os três Labels declarados como "(ícone)" ((1756,117), (168,161) e (952,223)) também não têm nome que descreva o símbolo.
- Nomes ambíguos para leitores de tela: três botões "More tabs", dois botões "History" (aba central e sub-aba Git, com o título Label repetindo o nome) e dois botões "Changes" (aba central e aba do painel direito); o texto "No changes." aparece duas vezes com papéis diferentes (nota do cabeçalho e estado vazio central).
- Nenhum elemento com retângulo fora da janela (1920×1080) e nenhuma sobreposição de retângulos nesta variante — diferentemente da referência pt-BR/dark, que tinha abas além da borda e sobrepostas.

## [en.light.modal.helper.profile_picker](pages/en.light.modal.helper.profile_picker.md)

- O modal sobrepõe a área central: o botão "More tabs" (324,50)–(411,85), as abas "Terminal", "Editor", "Browser" e "Changes" e os botões "+", "Recipes" e "Shell" continuam na árvore de acessibilidade, mas estão invisíveis ao usuário.
- Botão de tema na barra superior (1741,5)–(1773,37), divisores verticais, os quatro controles da barra central, a barra de título do modal, dois elementos internos do modal e a grande região de conteúdo central sem nome acessível.
- Rótulos de ícone expostos como "(ícone)" (grade e pasta dos estados vazios), sem nome acessível descritivo.
- Elemento de dimensão nula na árvore em (960,540)–(960,540).
- Nomes acessíveis ambíguos por duplicação: "Changes" nomeia duas abas distintas (área central e painel direito) e "More tabs" se repete três vezes, o que dificulta a identificação por leitores de tela.
- Mensagens de estado vazio ("No project yet..." e "No active bench...") em cinza claro sobre fundo branco, com contraste inferior ao do restante dos textos.

## [en.light.bench-tab.review.tab](pages/en.light.bench-tab.review.tab.md)

- Campos de texto sem nome acessível: título (324,222), descrição (324,253), Base (360,418), Revisores (393,458) e Labels (675,458); os dois primeiros exibem placeholders ("Title", "Description") que poderiam servir de nome.
- O título do formulário "New pull request", visível na imagem entre "Refresh review" e "Source branch:", não aparece na árvore de acessibilidade.
- O rótulo "Source branch:" é seguido diretamente do campo que exibe o texto "Title"; não há campo visível para informar a branch de origem.
- Três botões homônimos "More tabs" ((64,50), (324,50) e (1632,50)) sem informação que os distinga para leitor de tela.
- Elementos interativos sem nome acessível: ícone de tema na barra superior (1741,5) e os dois divisores dos painéis (304,42) e (1612,42); os divisores só expõem a ação "focus", sem ação de arrastar/clique associada.
- A caixa "Open as draft" é desenhada como um círculo (aspecto de botão de opção) na imagem, embora seja exposta como CheckBox.

## [en.light.bench-tab.viewers.tab](pages/en.light.bench-tab.viewers.tab.md)

- Botão sem nome acessível: o ícone de tema (sol) em (1741,5)–(1773,37) não tem rótulo.
- Nomes duplicados sem contexto dificultam a desambiguação por leitores de tela: três botões "More tabs" (64–151, 324–411 e 1632–1719) e duas abas "Changes" (700–777 e 1778–1855).

