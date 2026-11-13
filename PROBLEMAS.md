# Problemas encontrados pelo atlas

Juntado das seções "Problemas encontrados" de cada página. Gerado automaticamente.

## [pt-BR.dark.bench-tab.bench.detail](pages/pt-BR.dark.bench-tab.bench.detail.md)

- A área central tem largura efetiva de ~396 px, mas a mensagem "Nenhuma bancada ativa.\nCrie uma bancada ou escolha uma na lateral ou no Quadro." ocupa (596,182)–(962,220): o texto invade a divisória do painel direito e é truncado visualmente ("Nen...", "Crie uma bancada ou...").
- Na fileira de abas do painel direito, as abas "Bancada", "Revisão", "Histórico" e "Painéis de extensões" ficam empilhadas com retângulos sobrepostos às abas visíveis ("Arquivos", "Mudanças", "Busca") e não aparecem na imagem.
- Duas ocorrências de "Painéis de extensões" (965–1115 e 1022–1173) e "Ajudante (chat)" (1115–1234) têm retângulos além da largura da imagem (1024 px): ficam fora da janela, parcial ou totalmente.
- A aba "Painéis de extensões" do painel de projetos se estende até x=371, além da divisória do painel (x=304); o texto aparece cortado ("Painéis de e...").
- O botão de tema na barra superior (845,5)–(877,37) é um Unknown sem nome acessível; só é identificável pelo ícone de sol.
- O botão "Nova bancada" começa em x=723, invadindo a faixa da divisória (716,42)–(732,736).
- A aba "Bancada" do painel direito aparece com marcado=True ao mesmo tempo que "Arquivos", sugerindo estado de seleção ambíguo.
- Vários elementos da árvore estão sem nome acessível (divisórias, barras de abas, separadores, ícones), o que reduz a navegabilidade por leitor de tela.

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

## [pt-BR.dark.bench-tab.extension.panels.bench](pages/pt-BR.dark.bench-tab.extension.panels.bench.md)

- Estouro de largura: os botões "Painéis de extensões" (até x=1115 e x=1173) e "Ajudante (chat)" (até x=1234) do painel direito têm retângulos além da largura da janela (1053 px) e ficam cortados ou invisíveis.
- Sobreposição de abas no painel direito: "Arquivos"/"Bancada", "Mudanças"/"Revisão", "Histórico"/"Busca" e "Portas"/"Painéis de extensões" ocupam retângulos que se sobrepõem, indicando dois grupos de abas desenhados na mesma faixa.
- Nome acessível duplicado e ambíguo: dois botões "Painéis de extensões" no grupo do painel direito, além de um terceiro no painel esquerdo, com o mesmo texto.
- Estado inconsistente: no painel direito, "Painéis de extensões" está marcado=True enquanto a aba visivelmente ativa é "Arquivos" (marcado=True também), sugerindo dois grupos de abas com ativos simultâneos.
- Texto cortado por transbordo de painel: a aba "Painéis de extensões" do painel esquerdo (220,50)–(371,82) cruza o divisor em x≈310 e aparece truncada ("Painéis de e") na borda do painel.
- Elementos sem nome acessível: alternador de tema (845,5), os dois divisores verticais (304,42 e 716,42), as faixas dos grupos de abas e os indicadores de sublinhado — apenas "Unknown"/(sem nome acessível) na árvore.
- Botões de janela nomeados apenas pelos símbolos "–", "□" e "×", sem rótulo descritivo (minimizar/maximizar/fechar).

## [pt-BR.dark.bench-tab.git.history](pages/pt-BR.dark.bench-tab.git.history.md)

- Abas "Painéis de extensões" (965,50)–(1115,82), "Painéis de extensões" (1022,50)–(1173,82) e "Ajudante (chat)" (1115,50)–(1234,82) têm retângulos além da largura da janela (1024 px): ficam fora da área visível e inalcançáveis.
- Retângulos de abas sobrepostos no grupo direito: "Arquivos"/"Bancada" (736–815), "Mudanças"/"Revisão" (814–901) e "Histórico"/"Busca" (885–965); também "Terminal" (324–401) sobre "Painéis de extensões" (220–371) — indício de elementos duplicados ou de grupos de abas sobrepostos na árvore.
- Nomes ambíguos para leitores de tela: três botões chamados "Painéis de extensões" e dois chamados "Histórico" (aba do grupo direito e sub-aba do painel Git).
- Texto cortado: no painel direito aparece apenas o "N" inicial em (~718,255) na borda do divisor (716,42)–(732,736); corresponde ao rótulo "Nenhuma mudança." (717,248)–(841,267) parcialmente ocluído.
- Rótulos de aba truncados pelos divisores de painel: "Painéis de e…" (grupo esquerdo, corte em x≈300) e "Visualizado" (painel central, corte em x≈723).
- Quatro elementos sem nome acessível além dos ícones declarados: os dois divisores verticais (304,42)–(320,736) e (716,42)–(732,736), a faixa de abas esquerda (64,50)–(300,82) e a faixa de abas direita (736,50)–(1012,82); a Image do logotipo (12,50)–(40,78) também não tem nome.
- Três Labels declarados como "(ícone)"; o de (765,212)–(793,240) não corresponde a nada visível na captura e carece de nome acessível.
- O botão de alternar tema (845,5)–(877,37) é um "Unknown" sem nome acessível, identificável apenas pelo ícone de sol.

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

## [pt-BR.dark.bench-tab.viewers.tab](pages/pt-BR.dark.bench-tab.viewers.tab.md)

- Sobreposição de abas: "Terminal" (324–401) invade "Painéis de extensões" (220–371) na mesma faixa; no painel direito, "Bancada" (739–815) cobre "Arquivos" (736–814), "Revisão" (815–885) cobre "Mudanças" (814–901) e "Busca" (901–960) invade "Histórico" (885–965).
- Elementos fora da janela (largura da imagem: 1024 px): "Painéis de extensões" (965–1115 e 1022–1173) e "Ajudante (chat)" (1115–1234) têm retângulos além da borda direita.
- Texto cortado na imagem: "Painéis de e…" (aba do painel esquerdo, cortada em x≈300), "Visualizado…" (aba central, cortada em x≈724) e a mensagem do visualizador, visível só até "ou us…" (cortada pelo divisor em x≈724), embora seu retângulo vá até x=909, cruzando para o painel direito.
- Duas faixas de abas reportadas na mesma linha y=50–82 com abas duplicadas ("Arquivos" e "Painéis de extensões" aparecem duas vezes), o que indica abas de painéis distintos empilhadas na mesma posição.
- Botão sem nome acessível: o ícone de tema claro/escuro em (845,5)–(877,37) não tem rótulo.
- A aba "Visualizador" (641–739) e o contêiner direito (736–1012) se sobrepõem levemente em x=736–739.

