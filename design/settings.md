# Forja: catálogo de configurações

> **Fonte de verdade dos Ajustes.** As chaves, os padrões e as justificativas deste arquivo são decisões da Forja, não de outro produto. Nenhum módulo inventa uma configuração fora deste catálogo; a versão de máquina fica em `design/settings.toml`.

## 1. Como ler este catálogo

Cada configuração pertence a **um módulo dono** e aparece em **um grupo de Ajustes**. A chave segue `<módulo>.<grupo>.<nome>`, sempre em `snake_case` e com exatamente três segmentos. O grupo da chave é o assunto dentro do módulo; o grupo de Ajustes é a aba onde a pessoa encontra a configuração. Os dois nem sempre coincidem, e isso é intencional: `git.protected.branches` é do módulo `git` e aparece em **Bancadas**, porque é ali que a pessoa decide o que a Forja pode tocar.

A coluna **Padrão** traz o valor decidido por nós e, logo depois do travessão, a justificativa curta. A coluna **Faixa / opções** define o que o schema aceita. O `†` marca a configuração que só vale depois de reiniciar o app.

### 1.1 Tipos

| Tipo | O que aceita |
|---|---|
| `toggle` | `true` ou `false`. |
| `choice` | Um valor de uma lista fechada; listas dinâmicas vêm de `themes_dark`, `themes_light` (arquivos de tema) ou `profiles` (perfis de ajudante). |
| `number` | Número dentro de uma faixa, com passo quando faz sentido. |
| `text` | Texto curto de uma linha. |
| `path` | Caminho de arquivo ou pasta; aceita `~` e variáveis do Windows. |
| `list` | Lista de itens do tipo indicado; a ordem é preservada. |
| `color` | Cor `#RRGGBB`; o tema aplica contraste e não aceita cor com alfa. |

### 1.2 Precedência e arquivos

1. **Padrão do módulo**, embutido no crate e validado contra `design/settings.toml`.
2. **Global do usuário**, em `%APPDATA%\Forja\settings.toml`, com chaves planas (`"terminal.scroll.history_lines" = 20000`).
3. **Projeto**, em `.forja\ajustes.toml`, somente para chaves da lista segura (`scope = "project"` em `design/settings.toml`; nada de caminho de cofre, proxy, aprovação MCP, permissões ou pasta de estado).
4. **Sessão**, aplicada pela linha de comando ou pelo RPC e nunca gravada.

O projeto vence o global para o que é do projeto (`projeto > usuário > padrão`); a sessão vence tudo e some ao fechar. Configuração global nunca é escrita pelo projeto. `FORJA_DATA_DIR` troca a pasta de estado em execuções isoladas e não é uma configuração de produto.

No catálogo de máquina (`design/settings.toml`), toda entrada traz `scope = "user"` ou `scope = "project"`. Apenas as chaves marcadas com `scope = "project"` podem ser lidas ou gravadas em `<projeto>\.forja\ajustes.toml`:
- **Bancadas e git:** `bench.worktree.name_template`, `bench.worktree.branch_prefix`, `bench.worktree.base_branch`, `bench.layout.tabs`, `bench.stage.creation_default`, `git.commit.sign_off`.
- **Terminal e receitas:** `terminal.shell.start_dir`, `terminal.cursor.style`, `terminal.font.family`, `terminal.font.size`, `terminal.theme.name`, `terminal.theme.follow_app`, `terminal.color.opacity`, `terminal.recipes.show_scope`, `terminal.recipes.shell`.
- **Editor e formatação:** `editor.indent.auto_detect`, `editor.indent.size`, `editor.indent.tabs`, `editor.wrap.mode`, `editor.wrap.column`, `editor.save.format`, `editor.save.trim_trailing`, `editor.save.final_newline`, `editor.files.encoding`, `editor.files.eol`.
- **Ajudantes e pedidos do projeto:** `helper.default.profile`, `request.default.source`, `request.local.prefix`, `counter.github.label_filter`, `counter.linear.team`, `counter.jira.project`.

Chaves fora dessa lista encontradas em `.forja\ajustes.toml` são ignoradas com aviso no log e nunca aplicadas; tentativas de gravar uma chave `scope = "user"` no escopo do projeto são recusadas com erro tipado no kernel.

### 1.3 Validação
- Chave desconhecida: o app avisa em **Ajustes → Avançado**, mantém a linha e nunca apaga o arquivo sozinho.
- Valor inválido: usa o padrão, mostra o motivo ao lado do campo e registra um aviso; não converte em silêncio.
- Configuração removida numa versão nova: vira órfã visível, com opção de apagar; não é aplicada.
- Segredo nunca entra em `settings.toml` nem em `ajustes.toml`: o schema só aponta para o cofre do Windows.
- Toda escrita é atômica e feita fora da thread da interface.
- A validação cruzada cobre o que o tipo sozinho não vê: `network.proxy.url` é obrigatória no modo `manual`; `terminal.shell.path` é obrigatório no shell `custom`; `git.commit.gpg_key` só vale com `git.commit.sign`; `browser.download.dir` e `bench.worktree.path` precisam existir e ser graváveis; listas não repetem item; `bench.stage.creation_default` só aceita etapa canônica; `appearance.colors.accent` precisa de 3:1 contra `bg` e `surface` e de 4,5:1 para texto sobre ele, e é recusada com a medição quando não passa. O erro aparece no campo que causou o problema, não num toast genérico.

## 2. Mapa dos grupos

| Grupo | Módulos que contribuem | Configurações |
|---|---|---:|
| **Aparência** (`appearance`) | `appearance`, `i18n`, `notifications` | 35 |
| **Bancadas** (`benches`) | `bench`, `board`, `git`, `archive` | 40 |
| **Terminal** (`terminal`) | `terminal` | 28 |
| **Editor** (`editor`) | `editor`, `diff`, `find`, `explorer` | 39 |
| **Ajudantes** (`helpers`) | `helper`, `chat`, `voice`, `usage`, `accounts` | 37 |
| **Mutirão** (`muster`) | `muster` | 14 |
| **Rotinas** (`routines`) | `routine` | 10 |
| **Pedidos e balcões** (`requests`) | `request`, `counter` | 18 |
| **Integrações** (`integrations`) | `mcp`, `browser`, `integrations` | 22 |
| **Remoto (SSH/WSL)** (`remote`) | `remote` | 13 |
| **Extensões** (`extensions`) | `extension`, `content` | 13 |
| **Privacidade** (`privacy`) | `privacy`, `network` | 17 |
| **Avançado** (`advanced`) | `anvil`, `ports`, `keymap`, `app`, `update` | 23 |
| **Total** | 36 módulos | **310** |

## 3.1 Aparência (`appearance`)

Aparência reúne o que muda a leitura e o conforto: tema, tipografia, densidade, movimento, idioma e avisos. Nada aqui altera o conteúdo do trabalho, e tudo pode ser desfeito ao vivo.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `appearance.theme.mode` | escolha | `system` — Seguir o Windows evita contraste inesperado ao alternar dia e noite. | `system`, `dark`, `light` | Tema · Theme | Segue o Windows ou fixa a interface em escuro ou claro. | `appearance` |
| `appearance.theme.dark_name` | escolha | `brasa_escuro` — Brasa Escuro é a identidade padrão e o contraste de referência. | `brasa_escuro`, `carvao`, `cinza_quente`, `aco` + descobertos de `themes_dark` | Tema escuro · Dark theme | Tema aplicado quando a interface está no modo escuro. | `appearance` |
| `appearance.theme.light_name` | escolha | `brasa_claro` — Brasa Claro preserva a mesma hierarquia no modo claro. | `brasa_claro` + descobertos de `themes_light` | Tema claro · Light theme | Tema aplicado quando a interface está no modo claro. | `appearance` |
| `appearance.ui.density` | escolha | `comfortable` — Confortável é o padrão do design system; compacto é opção para telas pequenas. | `comfortable`, `compact` | Densidade · Density | Escolhe o espaçamento entre confortável e compacto. | `appearance` |
| `appearance.ui.scale` | número | `1.0` — 100% mantém as medidas dos tokens Brasa sem arredondamentos. | 0.8–1.5, passo 0.05 | Escala da interface · UI scale | Aumenta ou reduz toda a interface. | `appearance` |
| `appearance.ui.font_family` | escolha | `auto` — Auto usa Segoe UI Variable quando o Windows oferece e cai para Segoe UI. | `auto`, `segoe_ui_variable`, `segoe_ui`, `system` | Fonte da interface · UI font | Fonte dos menus, rótulos e textos da interface. | `appearance` |
| `appearance.ui.font_size` | número | `14` — 14 px é o menor tamanho que mantém a grade de 4 px confortável. | 12–18 | Tamanho do texto · Text size | Tamanho base do texto da interface. | `appearance` |
| `appearance.ui.code_font` | escolha | `cascadia_code` — Cascadia Code é a fonte da identidade e acompanha o Windows 11; sem ela, a Forja usa Consolas. | `cascadia_code`, `cascadia_mono`, `consolas`, `jetbrains_mono` | Fonte de código · Code font | Fonte padrão para editor e terminal. | `appearance` |
| `appearance.ui.code_size` | número | `13` — 13 px mantém cerca de 100 colunas visíveis em telas comuns. | 10–24 | Tamanho do código · Code size | Tamanho base do código no editor e no terminal. | `appearance` |
| `appearance.ui.ligatures` | alternar | sim — Ligaduras são legíveis e não alteram o texto do arquivo. | — | Ligaduras · Ligatures | Desenha sequências como -> e => como um único glifo. | `appearance` |
| `appearance.motion.reduce` | escolha | `auto` — Auto respeita quem já pediu menos animação no Windows. | `auto`, `always`, `never` | Reduzir movimento · Reduce motion | Segue a preferência de acessibilidade do sistema ou força a redução. | `appearance` |
| `appearance.motion.duration_ms` | número | `120` — 120 ms é perceptível sem atrasar a resposta da interface. | 0–400 | Duração da animação (ms) · Animation duration (ms) | Duração das transições curtas. | `appearance` |
| `appearance.colors.accent` | cor | `#F2762E` — O padrão é o token accent da Brasa; a troca não muda o contraste do texto. | `#RRGGBB` | Cor de destaque · Accent color | Cor de ação principal e de foco. | `appearance` |
| `appearance.sidebar.visible` | alternar | sim — A lateral é o mapa de projetos e bancadas e começa à vista. | — | Mostrar lateral · Show sidebar | Mostra a lista de projetos e bancadas. | `appearance` |
| `appearance.sidebar.width` | número | `260` — 260 px cabe nome de projeto e branch sem truncar. | 180–480 | Largura da lateral · Sidebar width | Largura da lateral em pixels. | `appearance` |
| `appearance.rail.visible` | alternar | sim — O trilho é a navegação primária e não deve sumir por acidente. | — | Mostrar trilho · Show rail | Mostra o trilho de navegação à esquerda. | `appearance` |
| `appearance.status_bar.visible` | alternar | sim — A barra é o painel de saúde do trabalho em andamento. | — | Mostrar barra de status · Show status bar | Mostra branch, ajudantes ativos, uso e recursos na base. | `appearance` |
| `appearance.window.remember_geometry` | alternar | sim — Retomar o layout é mais útil que centralizar a janela a cada abertura. | — | Lembrar tamanho e posição · Remember size and position | Reabre a janela no tamanho e na posição anteriores. | `appearance` |
| `appearance.window.start_maximized` | alternar | não — Abrir no tamanho anterior evita surpresa em monitores múltiplos. | — | Abrir maximizada · Start maximized | Inicia a janela ocupando a tela inteira. | `appearance` |
| `appearance.tabs.middle_click` | alternar | sim — O gesto é o esperado em abas e navegadores. | — | Fechar aba com botão do meio · Middle-click closes tab | Fecha a aba ao clicar com o botão do meio. | `appearance` |
| `appearance.tooltips.enabled` | alternar | sim — Dicas curtas ensinam atalhos sem abrir a ajuda. | — | Dicas · Tooltips | Mostra uma dica ao passar o mouse sobre controles. | `appearance` |
| `i18n.language.mode` | escolha | `auto` — Auto acompanha o idioma do Windows sem exigir escolha. | `auto`, `pt_BR`, `en` | Idioma · Language | Idioma da interface ou automático pelo sistema. | `i18n` |
| `i18n.language.fallback` | escolha | `en` — Inglês é o fallback do kernel e evita rótulo vazio. | `en`, `pt_BR` | Idioma de reserva · Fallback language | Idioma usado quando falta uma tradução. | `i18n` |
| `i18n.time.format` | escolha | `auto` — Auto segue o formato de hora do Windows. | `auto`, `24h`, `12h` | Formato de hora · Time format | Relógio de 24 ou 12 horas. | `i18n` |
| `i18n.date.week_start` | escolha | `auto` — Auto respeita a região configurada no Windows. | `auto`, `sunday`, `monday` | Início da semana · Week starts on | Primeiro dia da semana em calendários. | `i18n` |
| `notifications.toast.enabled` | alternar | sim — Aviso na tela não interrompe o trabalho. | — | Avisos na tela · On-screen toasts | Mostra avisos temporários no canto da janela. | `notifications` |
| `notifications.toast.position` | escolha | `bottom_right` — Perto da barra de status, onde o olho já procura estado. | `bottom_right`, `top_right`, `bottom_left`, `top_left` | Posição dos avisos · Toast position | Canto onde os avisos aparecem. | `notifications` |
| `notifications.toast.duration_s` | número | `5` — 5 s dá tempo de ler sem cobrir o trabalho. | 2–15 | Duração do aviso (s) · Toast duration (s) | Tempo que cada aviso permanece na tela. | `notifications` |
| `notifications.toast.max` | número | `3` — Três avisos evitam uma parede de mensagens. | 1–5 | Avisos simultâneos · Simultaneous toasts | Quantidade máxima de avisos empilhados. | `notifications` |
| `notifications.os.enabled` | alternar | sim — A central do Windows alcança você com a Forja minimizada. | — | Notificação do Windows · Windows notification | Envia avisos para a central de notificações do Windows. | `notifications` |
| `notifications.os.only_unfocused` | alternar | sim — Com a janela ativa, o aviso na tela já basta. | — | Só quando a Forja não está em foco · Only when Forja is unfocused | Notifica pelo Windows só quando a Forja não está em foco. | `notifications` |
| `notifications.os.sound` | alternar | não — Som por padrão atrapalha quem ouve música; a voz tem controle próprio. | — | Som nas notificações · Notification sound | Toca o som do Windows ao notificar. | `notifications` |
| `notifications.quiet.enabled` | alternar | não — Silêncio inesperado faz perder pedido; é opt-in. | — | Horário de silêncio · Quiet hours | Silencia avisos dentro de um intervalo de horas. | `notifications` |
| `notifications.quiet.start` | texto | "22:00" — 22:00 cobre a noite típica de quem trabalha tarde. | HH:MM | Início do silêncio · Quiet start | Hora em que o horário de silêncio começa. | `notifications` |
| `notifications.quiet.end` | texto | "07:00" — 07:00 devolve os avisos junto com o começo do dia. | HH:MM | Fim do silêncio · Quiet end | Hora em que o horário de silêncio termina. | `notifications` |

## 3.2 Bancadas (`benches`)

Bancadas define onde o trabalho isolado vive, como nasce, como é guardado e como o Quadro, as etapas e o git se comportam em nome de você.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `bench.worktree.location` | escolha | `data_dir` — Fora do repositório evita que busca e git vejam worktrees alheias. | `data_dir`, `beside_project`, `custom` | Onde ficam as bancadas · Where benches live | Escolhe se as bancadas ficam na pasta de dados, ao lado do projeto ou num caminho próprio. | `bench` |
| `bench.worktree.path` | caminho | `%LOCALAPPDATA%\Forja\bancadas` — Um caminho explícito deixa limpeza e backup sob controle. | caminho absoluto | Pasta das bancadas · Benches folder | Pasta usada quando o local das bancadas é personalizado. | `bench` |
| `bench.worktree.name_template` | texto | "{pedido}-{slug}" — Pedido mais slug produz nome legível e estável. | {pedido}, {slug}, {tipo}, {numero} | Nome da bancada · Bench name | Modelo do nome da pasta e da branch da bancada. | `bench` |
| `bench.worktree.branch_prefix` | texto | "forja/" — O prefixo separa trabalho de agente do trabalho manual. | texto | Prefixo de branch · Branch prefix | Prefixo aplicado às branches criadas pela Forja. | `bench` |
| `bench.worktree.base_branch` | escolha | `auto` — Auto usa a branch padrão do remoto e reduz divergência. | `auto`, `current`, `main`, `master` | Branch base · Base branch | Branch de onde uma nova bancada nasce. | `bench` |
| `bench.worktree.fetch_before` | alternar | sim — A bancada nasce do estado mais recente sem passo manual. | — | Atualizar antes de criar · Fetch before creating | Faz fetch da branch base antes de criar a bancada. | `bench` |
| `bench.worktree.auto_prune_days` | número | `0` — 0 preserva trabalho guardado; a limpeza automática é escolha explícita. | 0–90 | Limpar bancadas antigas (dias) · Prune old benches (days) | Remove bancadas guardadas após esse prazo; 0 desliga. | `bench` |
| `bench.layout.tabs` | lista | `[terminal, helper]` — Terminal e ajudante são o par mínimo do fluxo de forja. | lista de id de aba | Abas iniciais · Initial tabs | Abas abertas automaticamente ao criar uma bancada. | `bench` |
| `bench.layout.restore` | alternar | sim — Voltar ao contexto é mais útil que começar do zero. | — | Restaurar abas · Restore tabs | Reabre as abas da última visita a cada bancada. | `bench` |
| `bench.setup.policy` | escolha | `ask` — Pedir mantém o repositório no controle do que roda. | `ask`, `always`, `never` | Rodar preparo do forja.toml · Run forja.toml setup | Executa o script de preparo do forja.toml ao criar a bancada. | `bench` |
| `bench.setup.timeout_s` | número | `300` — 5 min cobre instaladores comuns sem travar a criação. | 30–1800 | Limite do preparo (s) · Setup timeout (s) | Tempo máximo do script de preparo. | `bench` |
| `bench.teardown.policy` | escolha | `ask` — O desmonte pode apagar dados; confirmação é a regra. | `ask`, `always`, `never` | Rodar desmonte · Run teardown | Executa o script de desmonte ao guardar a bancada. | `bench` |
| `bench.teardown.branch` | escolha | `ask` — Perguntar evita perder commits que não foram enviados. | `ask`, `keep`, `delete` | Branch ao guardar · Branch on archive | Mantém, apaga ou pergunta o destino da branch ao guardar. | `bench` |
| `bench.close.confirm_dirty` | alternar | sim — Fechar não pode descartar trabalho sem aviso. | — | Confirmar com mudanças · Confirm with changes | Pede confirmação ao fechar bancada com mudanças não salvas. | `bench` |
| `bench.close.stop_helpers` | alternar | sim — Ajudante órfão consome recurso e escreve sem supervisão. | — | Parar ajudantes ao fechar · Stop helpers on close | Encerra os ajudantes da bancada ao fechá-la. | `bench` |
| `board.cards.show_branch` | alternar | sim — Branch é a identidade técnica da bancada. | — | Mostrar branch no cartão · Show branch on card | Mostra a branch no cartão da bancada. | `board` |
| `board.cards.show_helper` | alternar | sim — O cartão responde quem está trabalhando sem abrir a bancada. | — | Mostrar ajudante no cartão · Show helper on card | Mostra o estado do ajudante no cartão. | `board` |
| `board.cards.show_request` | alternar | sim — Liga o trabalho ao pedido de origem. | — | Mostrar pedido no cartão · Show request on card | Mostra o número ou título do pedido no cartão. | `board` |
| `board.cards.compact` | alternar | não — O cartão normal mostra contexto; compacto serve a quadros cheios. | — | Cartões compactos · Compact cards | Reduz o cartão a uma linha. | `board` |
| `board.refresh.interval_s` | número | `15` — 15 s mantém o quadro vivo sem consultar git o tempo todo. | 0–120 | Atualização do quadro (s) · Board refresh (s) | Intervalo de atualização automática do quadro; 0 desliga. | `board` |
| `bench.stage.creation_default` | escolha | `draft` — Toda bancada nasce em rascunho, antes de qualquer agente. | `draft`, `forging`, `review`, `ready` | Etapa inicial · Initial stage | Etapa de uma bancada recém-criada. | `stages` |
| `bench.stage.wip_limit` | número | `0` — Limite é aviso, não bloqueio; 0 respeita quem trabalha em paralelo. | 0–50 | Limite de bancadas em forjando · WIP limit in forging | Avisa quando o número de bancadas em forjando passa do limite; 0 desliga. | `stages` |
| `bench.stage.sync_request` | alternar | sim — Quadro e pedido não devem contar histórias diferentes. | — | Sincronizar com pedidos · Sync with requests | Reflete a etapa da bancada no pedido quando existe um balcão. | `stages` |
| `bench.stage.pr_open` | escolha | `always` — O fluxo do produto já trata PR aberto como revisão; automático evita esquecimento. | `always`, `ask`, `never` | Mover para revisão ao abrir PR · Move to review on PR open | Define se a bancada vai para revisão quando um PR é aberto. | `stages` |
| `bench.stage.pr_merge` | escolha | `always` — Merge é a entrega objetiva; a etapa deve refletir isso. | `always`, `ask`, `never` | Mover para pronto ao mesclar PR · Move to ready on PR merge | Define se a bancada vai para pronto quando o PR é mesclado. | `stages` |
| `bench.stage.archive_on_ready` | alternar | não — Pronto ainda pode precisar de ajustes de revisão. | — | Guardar ao entrar em pronto · Archive on ready | Guarda a bancada automaticamente ao entrar na etapa pronto. | `stages` |
| `git.identity.name` | texto | (vazio) — Vazio usa o git config; preencher só quando o repositório não define. | nome | Nome do autor · Author name | Nome de autor usado em commits quando o git não tem um configurado. | `git` |
| `git.identity.email` | texto | (vazio) — Mesma regra do nome: o git config manda. | email | E-mail do autor · Author email | E-mail de autor usado em commits quando o git não tem um configurado. | `git` |
| `git.commit.sign` | alternar | não — Assinatura exige chave e senha; opt-in evita bloquear quem não usa. | — | Assinar commits · Sign commits | Assina commits com a chave GPG configurada. | `git` |
| `git.commit.gpg_key` | texto | (vazio) — Vazio usa a chave padrão do git. | id da chave | Chave GPG · GPG key | Chave GPG usada para assinar commits. | `git` |
| `git.commit.sign_off` | alternar | não — Só repositórios que exigem DCO precisam. | — | Assinar como autor (sign-off) · Sign off commits | Acrescenta Signed-off-by ao commit. | `git` |
| `git.sync.fetch_interval_min` | número | `10` — 10 min mantém o status de sincronização honesto. | 0–60 | Buscar do remoto (min) · Fetch interval (min) | Intervalo de fetch em segundo plano; 0 desliga. | `git` |
| `git.sync.auto_pull` | escolha | `never` — Pull automático pode conflitar com agente; o padrão é não puxar. | `never`, `clean_only`, `always` | Atualizar automaticamente · Auto pull | Faz pull automaticamente quando a bancada está limpa. | `git` |
| `git.push.auto_set_upstream` | alternar | sim — Evita o primeiro push falhar por falta de upstream. | — | Configurar upstream ao enviar · Set upstream on push | Configura a branch remota no primeiro push. | `git` |
| `git.protected.branches` | lista | `[main, master]` — main e master são o mínimo para não escrever na linha principal. | lista de nome de branch | Branches protegidas · Protected branches | Branches que a Forja não altera diretamente. | `git` |
| `git.status.interval_s` | número | `5` — 5 s mantém a barra de status útil sem varrer o disco demais. | 1–60 | Atualização do status (s) · Status refresh (s) | Intervalo de leitura do status do repositório. | `git` |
| `archive.sessions.index` | alternar | sim — O Acervo é a memória do trabalho; indexar é o padrão. | — | Indexar sessões no Acervo · Index sessions in Archive | Guarda sessões de ajudantes para busca no Acervo. | `archive` |
| `archive.sessions.retention_days` | número | `365` — Um ano cobre arqueologia de projeto sem crescer para sempre. | 0–3650 | Retenção de sessões (dias) · Session retention (days) | Apaga sessões indexadas após esse prazo; 0 mantém para sempre. | `archive` |
| `archive.sessions.max_gb` | número | `10` — 10 GB é folgado para texto e evita encher o disco. | 1–50 | Tamanho máximo do Acervo (GB) · Archive size limit (GB) | Tamanho máximo do índice de sessões. | `archive` |
| `archive.transcripts.keep` | alternar | sim — Sem transcrição não há busca; desligar só para quem exporta por fora. | — | Guardar transcrições · Keep transcripts | Mantém o texto completo das sessões indexadas. | `archive` |

## 3.3 Terminal (`terminal`)

Terminal cobre o shell, a leitura da saída, a cópia e a colagem, os links e as receitas. O objetivo é que o terminal da Forja seja previsível até para quem vive nele.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `terminal.shell.default` | escolha | `auto` — Auto escolhe pwsh quando instalado e cai para powershell. | `auto`, `pwsh`, `powershell`, `cmd`, `custom` | Shell padrão · Default shell | Shell aberto em novos terminais. | `terminal` |
| `terminal.shell.path` | caminho | (vazio) — Só vale no modo personalizado; vazio usa a detecção. | executável | Caminho do shell · Shell path | Executável do shell quando a opção é personalizada. | `terminal` |
| `terminal.shell.start_dir` | escolha | `bench` — A raiz da bancada é o contexto do trabalho. | `bench`, `project`, `home` | Pasta inicial · Start folder | Pasta em que novos terminais abrem. | `terminal` |
| `terminal.shell.env` | lista | `[]` — Lista explícita é mais auditável que herdar tudo. | lista de NOME=valor | Variáveis de ambiente · Environment variables | Variáveis de ambiente extras para processos do terminal. | `terminal` |
| `terminal.scroll.history_lines` | número | `10000` — 10 mil linhas cobrem um build longo sem pesar. | 1000–200000 | Histórico de rolagem · Scrollback | Linhas de histórico guardadas por terminal. | `terminal` |
| `terminal.scroll.wheel_lines` | número | `3` — Três linhas é o passo confortável em fonte de 13 px. | 1–10 | Linhas por rolagem · Lines per wheel notch | Linhas roladas por entalhe da roda do mouse. | `terminal` |
| `terminal.scroll.follow_output` | alternar | sim — Acompanhar é o esperado num terminal de build. | — | Acompanhar saída · Follow output | Mantém a visão no fim quando chega saída nova. | `terminal` |
| `terminal.cursor.style` | escolha | `block` — Bloco é o padrão clássico e o mais visível. | `block`, `bar`, `underline` | Estilo do cursor · Cursor style | Forma do cursor do terminal. | `terminal` |
| `terminal.font.family` | escolha | `inherit` — Herdar mantém o terminal igual ao editor. | `inherit`, `cascadia_code`, `cascadia_mono`, `consolas` | Fonte do terminal · Terminal font | Fonte usada no terminal. | `terminal` |
| `terminal.font.size` | número | `13` — 13 px é o mesmo do código para não saltar entre abas. | 8–32 | Tamanho da fonte · Font size | Tamanho da fonte do terminal. | `terminal` |
| `terminal.theme.name` | escolha | `brasa_escuro` — Brasa Escuro liga o terminal à identidade do app. | `brasa_escuro`, `brasa_claro`, `carvao`, `cinza_quente`, `aco` + descobertos de `themes` | Tema do terminal · Terminal theme | Paleta de cores do terminal quando ele não segue o tema da interface. | `terminal` |
| `terminal.theme.follow_app` | alternar | sim — Um tema só evita interface clara com terminal escuro. | — | Seguir o tema do app · Follow app theme | Troca a paleta do terminal junto com o tema da interface; desligue para fixar a paleta acima. | `terminal` |
| `terminal.color.opacity` | número | `1.0` — Opaco é o padrão; transparência é gosto e custa legibilidade. | 0.6–1.0, passo 0.05 | Opacidade do fundo · Background opacity | Transparência do fundo do terminal. | `terminal` |
| `terminal.bell.mode` | escolha | `visual` — Visual avisa sem som em ambiente compartilhado. | `none`, `visual`, `audible`, `both` | Aviso sonoro · Bell | Como o terminal reage ao caractere de campainha. | `terminal` |
| `terminal.copy.on_select` | alternar | não — Cópia automática surpreende quem seleciona só para ler. | — | Copiar ao selecionar · Copy on select | Copia automaticamente o texto selecionado. | `terminal` |
| `terminal.copy.trim` | alternar | sim — Espaços invisíveis quebram comandos colados. | — | Remover espaços ao copiar · Trim spaces on copy | Remove espaços à direita ao copiar do terminal. | `terminal` |
| `terminal.paste.warn_multiline` | alternar | sim — Colar várias linhas pode executar comando sem revisão. | — | Avisar ao colar várias linhas · Warn on multiline paste | Pede confirmação antes de colar várias linhas. | `terminal` |
| `terminal.paste.bracketed` | alternar | sim — Evita que o shell execute cada linha colada na hora. | — | Colagem com colchetes · Bracketed paste | Usa o modo de colagem segura do shell. | `terminal` |
| `terminal.links.confirm` | alternar | sim — Links em saída de build podem ser hostis. | — | Confirmar abertura de links · Confirm link opening | Pede confirmação antes de abrir um link do terminal. | `terminal` |
| `terminal.links.open_with` | escolha | `system` — O navegador do sistema tem as sessões e extensões do usuário. | `system`, `app` | Abrir links com · Open links with | Abre links no navegador do sistema ou numa aba interna. | `terminal` |
| `terminal.shell.marks` | alternar | sim — Permite pular entre comandos e copiar a saída certa. | — | Marcação de prompt · Shell marks | Marca o início e o fim de cada comando no terminal. | `terminal` |
| `terminal.close.confirm_running` | alternar | sim — Fechar aba não deve matar build sem aviso. | — | Confirmar ao fechar processo · Confirm closing a running process | Pede confirmação ao fechar terminal com processo ativo. | `terminal` |
| `terminal.gpu.mode` | escolha | `auto` — Auto liga quando o driver é confiável e desliga em caso de artefato. | `auto`, `on`, `off` | Renderização por GPU · GPU rendering | Usa a GPU para desenhar o terminal. | `terminal` |
| `terminal.osc52.enabled` | alternar | não — Desligado impede que um processo remoto copie dados sem você ver. | — | Área de transferência OSC 52 · OSC 52 clipboard | Permite que programas no terminal escrevam na área de transferência via OSC 52. | `terminal` |
| `terminal.recipes.press_enter` | alternar | sim — Receita é atalho de comando; o Enter evita um passo. | — | Enviar Enter ao rodar receita · Press Enter when running a recipe | Envia Enter automaticamente depois de colar a receita. | `terminal` |
| `terminal.recipes.confirm` | alternar | não — Receitas salvas são confiáveis; confirmação fica para quem quer. | — | Confirmar antes de rodar · Confirm before running | Pede confirmação antes de executar uma receita. | `terminal` |
| `terminal.recipes.show_scope` | escolha | `all` — Todas são úteis; o filtro atende quem tem muitas receitas. | `all`, `global`, `project` | Escopo das receitas · Recipe scope | Quais receitas aparecem no seletor. | `terminal` |
| `terminal.recipes.shell` | escolha | `inherit` — Herdar o terminal evita diferença de sintaxe. | `inherit`, `pwsh`, `powershell`, `cmd` | Shell das receitas · Recipe shell | Shell usado para executar receitas de comando. | `terminal` |

## 3.4 Editor (`editor`)

Editor reúne edição de texto, diferenças e busca. As decisões favorecem revisão legível e mudanças explícitas, não efeitos de digitação.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `editor.font.family` | escolha | `inherit` — Herdar mantém o código igual ao terminal. | `inherit`, `cascadia_code`, `cascadia_mono`, `consolas`, `jetbrains_mono` | Fonte do editor · Editor font | Fonte usada no editor. | `editor` |
| `editor.font.size` | número | `13` — 13 px equilibra densidade e leitura. | 8–32 | Tamanho da fonte · Font size | Tamanho da fonte do editor. | `editor` |
| `editor.line.height` | número | `1.4` — 1,4 separa blocos sem afastar demais. | 1.0–1.8, passo 0.05 | Altura da linha · Line height | Espaço vertical entre linhas. | `editor` |
| `editor.gutter.line_numbers` | alternar | sim — Números são referência de revisão e de erro de compilação. | — | Números de linha · Line numbers | Mostra a coluna de números de linha. | `editor` |
| `editor.indent.auto_detect` | alternar | sim — Detectar evita reformatar arquivo alheio. | — | Detectar indentação · Detect indentation | Usa o estilo de indentação que o arquivo já tem. | `editor` |
| `editor.indent.size` | número | `4` — 4 é o padrão de Rust, Go e Java; a detecção cobre o resto. | 1–8 | Tamanho da indentação · Indent size | Espaços por nível quando não há detecção. | `editor` |
| `editor.indent.tabs` | escolha | `spaces` — Espaços são previsíveis em diff e revisão. | `spaces`, `tabs` | Usar tabs · Use tabs | Indenta com espaços ou tabulações. | `editor` |
| `editor.wrap.mode` | escolha | `off` — Desligado preserva a leitura de código; prosa liga por coluna. | `off`, `column`, `window` | Quebra de linha · Line wrapping | Quebra linhas longas por coluna ou pela largura da janela. | `editor` |
| `editor.wrap.column` | número | `100` — 100 colunas cabem em duas telas lado a lado. | 40–200 | Coluna de quebra · Wrap column | Coluna usada quando a quebra é por coluna. | `editor` |
| `editor.highlight.current_line` | alternar | sim — Um realce discreto ancora a leitura. | — | Destacar linha atual · Highlight current line | Realça a linha onde está o cursor. | `editor` |
| `editor.view.minimap` | alternar | não — Desligado por padrão: rouba largura e pouco acrescenta em código. | — | Minimapa · Minimap | Mostra uma miniatura do arquivo na lateral. | `editor` |
| `editor.brackets.guides` | alternar | sim — Guias ajudam a fechar blocos longos. | — | Guias de blocos · Bracket guides | Desenha linhas nos níveis de indentação. | `editor` |
| `editor.whitespace.show` | escolha | `selection` — Só na seleção mostra o invisível sem poluir o código. | `never`, `selection`, `always` | Mostrar espaços · Show whitespace | Exibe espaços e tabulações. | `editor` |
| `editor.save.auto` | escolha | `off` — Desligado mantém controle explícito; agentes salvam pelos próprios meios. | `off`, `delay`, `focus_change` | Salvamento automático · Auto save | Salva sozinho após um atraso ou ao trocar de foco. | `editor` |
| `editor.save.delay_ms` | número | `1500` — 1,5 s evita gravar a cada tecla. | 500–10000 | Atraso do salvamento (ms) · Auto save delay (ms) | Espera antes do salvamento automático. | `editor` |
| `editor.save.format` | alternar | não — Formatar pode mudar o arquivo inteiro; opt-in. | — | Formatar ao salvar · Format on save | Roda o formatador do projeto ao salvar. | `editor` |
| `editor.save.trim_trailing` | alternar | não — Pode gerar diff grande em arquivo alheio. | — | Remover espaços finais · Trim trailing whitespace | Remove espaços no fim das linhas ao salvar. | `editor` |
| `editor.save.final_newline` | alternar | sim — POSIX e git preferem arquivo terminado em newline. | — | Garantir newline final · Ensure final newline | Garante uma quebra de linha no fim do arquivo. | `editor` |
| `editor.files.encoding` | escolha | `auto` — Auto respeita BOM e padrão do sistema. | `auto`, `utf8`, `utf8_bom`, `utf16le` | Codificação · Encoding | Codificação usada ao abrir e salvar arquivos. | `editor` |
| `editor.files.eol` | escolha | `auto` — Auto preserva o que o arquivo já usa. | `auto`, `lf`, `crlf` | Fim de linha · Line endings | Mantém ou converte o fim de linha. | `editor` |
| `editor.files.large_mb` | número | `10` — 10 MB evita travar a interface com log ou minificado. | 1–100 | Limite de arquivo grande (MB) · Large file limit (MB) | Abre arquivos acima desse tamanho em modo leve. | `editor` |
| `editor.search.regex` | alternar | não — Busca literal é o caso comum; regex é escolha explícita. | — | Usar regex por padrão · Regex by default | Trata o termo de busca como expressão regular. | `editor` |
| `editor.search.case_sensitive` | alternar | não — Sem diferenciação acha mais; o botão Aa fica à mão. | — | Diferenciar maiúsculas · Case sensitive | Diferencia maiúsculas de minúsculas. | `editor` |
| `editor.search.whole_word` | alternar | não — Ligado evita trocar parte de outra palavra. | — | Palavra inteira · Whole word | Encontra apenas a palavra completa. | `editor` |
| `editor.markdown.preview` | alternar | sim — Documento e prévia lado a lado é o fluxo natural. | — | Prévia de Markdown · Markdown preview | Abre a prévia ao editar um arquivo Markdown. | `editor` |
| `diff.view.mode` | escolha | `split` — Lado a lado mostra antes e depois sem rolar. | `split`, `unified` | Modo de diferenças · Diff mode | Exibe diferenças lado a lado ou em modo unificado. | `diff` |
| `diff.context.lines` | número | `3` — 3 linhas bastam para localizar o trecho. | 0–20 | Linhas de contexto · Context lines | Linhas de contexto ao redor de cada mudança. | `diff` |
| `diff.whitespace.ignore` | alternar | não — Mostrar tudo é o padrão de revisão honesta. | — | Ignorar espaço · Ignore whitespace | Esconde diferenças que só mudam espaço em branco. | `diff` |
| `diff.view.collapse_unchanged` | alternar | sim — Revisão foca no que mudou. | — | Recolher trechos iguais · Collapse unchanged | Recolhe blocos sem nenhuma mudança. | `diff` |
| `diff.notes.inline` | alternar | sim — Nota no lugar certo evita perder contexto. | — | Notas na linha · Inline notes | Mostra notas de revisão junto à linha. | `diff` |
| `diff.notes.send_mode` | escolha | `append` — Anexar preserva o histórico da conversa. | `append`, `replace`, `ask` | Enviar notas ao ajudante · Send notes to helper | Define como as notas são enviadas ao ajudante. | `diff` |
| `find.results.limit` | número | `200` — 200 cobre o caso comum sem montar listas enormes. | 50–1000 | Limite de resultados · Result limit | Máximo de itens na busca de arquivos. | `find` |
| `find.search.respect_gitignore` | alternar | sim — Build e cache não devem poluir a busca. | — | Respeitar .gitignore · Respect .gitignore | Esconde arquivos ignorados pelo git. | `find` |
| `find.search.include_hidden` | alternar | não — Ocultos são ruído até serem o alvo. | — | Incluir ocultos · Include hidden | Inclui arquivos e pastas ocultos. | `find` |
| `find.recent.count` | número | `20` — 20 é suficiente para retomar sem virar lista. | 5–50 | Arquivos recentes · Recent files | Arquivos recentes mostrados quando a busca está vazia. | `find` |
| `find.grep.engine` | escolha | `auto` — Auto usa ripgrep quando disponível e cai para o motor interno. | `auto`, `ripgrep`, `builtin` | Motor de busca · Search engine | Motor usado para buscar no conteúdo dos arquivos. | `find` |
| `find.grep.context` | número | `2` — 2 linhas mostram o suficiente para decidir. | 0–10 | Contexto da busca (linhas) · Search context (lines) | Linhas de contexto ao redor de cada resultado. | `find` |
| `explorer.tree.show_ignored` | alternar | não — Ignorados costumam ser build e cache; mostrar é opt-in. | — | Mostrar arquivos ignorados · Show ignored files | Lista arquivos que o .gitignore esconde. | `explorer` |
| `explorer.tree.follow_symlinks` | alternar | não — Um link pode apontar para fora da bancada; seguir é opt-in. | — | Seguir links simbólicos · Follow symbolic links | Lista o destino dos links simbólicos. | `explorer` |

## 3.5 Ajudantes (`helpers`)

Ajudantes cobre como um agente nasce, como é observado, o que é guardado da sessão e como o chat e a voz se comportam. Permissões e isolamento são decisões de segurança, não conveniência.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `helper.default.profile` | escolha | `auto` — Auto usa o primeiro perfil instalado na ordem que decidimos. | `auto` + descobertos de `profiles` | Perfil padrão · Default profile | Perfil de ajudante usado quando o projeto não escolhe um. | `helper` |
| `helper.default.permissions` | escolha | `ask` — Perguntar é o padrão seguro; autonomia é escolha explícita. | `ask`, `accept_edits`, `auto`, `total` | Permissões padrão · Default permissions | Modo de permissões padrão ao iniciar um ajudante. | `helper` |
| `helper.launch.auto_start` | alternar | sim — O fluxo do pedido já nasce com um ajudante trabalhando. | — | Iniciar ajudante automaticamente · Auto-start helper | Inicia um ajudante ao criar uma bancada a partir de um pedido. | `helper` |
| `helper.launch.initial_context` | escolha | `request` — O pedido é o contrato do trabalho; o resto o ajudante lê no repositório. | `request`, `bench`, `none` | Contexto inicial · Initial context | Contexto incluído no prompt inicial do ajudante. | `helper` |
| `helper.launch.include_rules` | alternar | sim — Regras do projeto devem chegar antes da primeira ação. | — | Incluir regras do projeto · Include project rules | Inclui AGENTS.md e regras equivalentes no contexto inicial. | `helper` |
| `helper.launch.mode` | escolha | `tui` — A TUI preserva o CLI original e seus atalhos; o chat é opção. | `tui`, `chat` | Modo do ajudante · Helper mode | Abre o ajudante no terminal ou no chat da Forja. | `helper` |
| `helper.launch.ready_timeout_s` | número | `30` — 30 s cobre CLIs que carregam modelo e ferramentas. | 5–120 | Tempo para ficar pronto (s) · Ready timeout (s) | Tempo máximo para o ajudante ficar pronto após iniciar. | `helper` |
| `helper.monitor.idle_minutes` | número | `10` — 10 min distingue pensamento longo de sessão parada. | 1–120 | Tempo para considerar parado (min) · Idle time (min) | Tempo sem atividade para marcar o ajudante como parado. | `helper` |
| `helper.notifications.waiting` | alternar | sim — Ajudante que pede aprovação trava o trabalho até você ver. | — | Avisar quando precisa de você · Notify when waiting | Notifica quando um ajudante precisa de você. | `helper` |
| `helper.notifications.done` | alternar | sim — Entrega concluída é o evento que libera a revisão. | — | Avisar quando termina · Notify when done | Notifica quando um ajudante termina a tarefa. | `helper` |
| `helper.notifications.error` | alternar | sim — Falha silenciosa é o pior estado de um agente. | — | Avisar quando falha · Notify on error | Notifica quando um ajudante falha ou perde a conexão. | `helper` |
| `helper.sessions.resume_policy` | escolha | `ask` — Retomar sem perguntar pode continuar uma sessão errada. | `ask`, `auto`, `never` | Retomar sessões · Resume sessions | Define se sessões anteriores são retomadas automaticamente. | `helper` |
| `helper.sessions.keep_transcripts` | alternar | sim — Sem transcrição não há Acervo nem auditoria. | — | Guardar transcrições · Keep transcripts | Guarda o texto das sessões para o Acervo. | `helper` |
| `helper.hooks.enabled` | alternar | sim — Hooks são como o app sabe que o agente mudou de estado. | — | Aceitar hooks dos agentes · Accept agent hooks | Aceita eventos dos hooks dos CLIs de agente. | `helper` |
| `helper.hooks.timeout_s` | número | `10` — 10 s evita hook travado segurar o encerramento. | 1–60 | Limite do hook (s) · Hook timeout (s) | Tempo máximo de espera por um hook. | `helper` |
| `helper.close.confirm_running` | alternar | sim — Encerrar um agente no meio pode deixar trabalho pela metade. | — | Confirmar ao fechar ajudante · Confirm closing helper | Pede confirmação ao fechar um ajudante em execução. | `helper` |
| `chat.font.size` | número | `14` — 14 px é confortável para prosa longa. | 10–24 | Tamanho do texto do chat · Chat text size | Tamanho do texto no chat. | `chat` |
| `chat.message.max_width` | número | `760` — 760 px mantém a linha legível em telas largas. | 400–1200 | Largura das mensagens · Message width | Largura máxima das mensagens do chat. | `chat` |
| `chat.input.enter_sends` | alternar | sim — Enter envia e Shift+Enter quebra linha, como no chat comum. | — | Enter envia · Enter sends | Usa Enter para enviar e Shift+Enter para nova linha. | `chat` |
| `chat.attachments.max_mb` | número | `20` — 20 MB cobre imagem e log sem estourar a memória. | 1–100 | Tamanho máximo de anexo (MB) · Attachment size limit (MB) | Tamanho máximo de anexo enviado ao ajudante. | `chat` |
| `chat.attachments.paste_images` | alternar | sim — Colar print é parte do jeito de pedir correção. | — | Colar imagens · Paste images | Aceita imagens coladas direto no chat. | `chat` |
| `chat.tools.show_calls` | alternar | sim — Ver a ferramenta e os argumentos torna o agente auditável. | — | Mostrar chamadas de ferramenta · Show tool calls | Mostra as chamadas de ferramenta do ajudante. | `chat` |
| `chat.tools.collapse_output` | alternar | sim — Saída longa esconde a conversa; recolhida, continua a um clique. | — | Recolher saída longa · Collapse long output | Recolhe a saída longa das ferramentas. | `chat` |
| `chat.diff.apply_mode` | escolha | `review` — Revisar antes de aplicar é o padrão seguro. | `review`, `inline`, `auto` | Aplicar mudanças do chat · Apply chat changes | Define como mudanças propostas no chat são aplicadas. | `chat` |
| `voice.mode.enabled` | alternar | não — Voz é opt-in: microfone e áudio merecem consentimento. | — | Voz · Voice | Liga entrada e saída de voz na Forja. | `voice` |
| `voice.input.engine` | escolha | `windows` — O motor do Windows funciona offline sem baixar modelo. | `windows`, `whisper_cpp` | Motor de transcrição · Transcription engine | Motor de transcrição da fala. | `voice` |
| `voice.input.model_path` | caminho | `~/.forja/modelos/voz` — Só vale no motor whisper.cpp; o caminho fica no conteúdo do usuário. | pasta | Modelo de transcrição · Transcription model | Pasta com o modelo do motor whisper.cpp; o motor do Windows não usa. | `voice` |
| `voice.input.language` | escolha | `auto` — Auto usa o idioma da interface; comando técnico mistura idiomas. | `auto`, `pt_BR`, `en` | Idioma da fala · Speech language | Idioma esperado na fala. | `voice` |
| `voice.output.engine` | escolha | `sapi` — SAPI já vem no Windows e não exige download. | `sapi`, `piper` | Motor de síntese · Speech engine | Motor de síntese de voz. | `voice` |
| `voice.output.model_path` | caminho | `~/.forja/modelos/voz` — Só vale no motor Piper; o caminho fica no conteúdo do usuário. | arquivo | Modelo de síntese · Speech model | Arquivo de modelo do motor Piper; o SAPI não usa. | `voice` |
| `voice.output.rate` | número | `1.0` — 1,0 é a velocidade natural da voz do sistema. | 0.5–2.0, passo 0.1 | Velocidade da fala · Speech rate | Velocidade da fala sintetizada. | `voice` |
| `voice.output.auto_read` | alternar | não — Ler toda resposta em voz alta atrapalha quem está lendo. | — | Ler respostas em voz alta · Read responses aloud | Lê automaticamente as respostas do ajudante. | `voice` |
| `voice.privacy.keep_audio` | alternar | não — Áudio bruto é dado sensível e não é necessário depois da transcrição. | — | Guardar áudio · Keep audio | Guarda os arquivos de áudio usados na transcrição. | `voice` |
| `usage.cost.track` | alternar | sim — Uso local ajuda a entender custo sem enviar nada para fora. | — | Registrar uso · Track usage | Registra consumo de tokens e custo por ajudante. | `usage` |
| `usage.budget.daily_usd` | número | `0` — desligado até a pessoa definir um teto; 0 desliga. | 0–100000 | Limite de gasto diário (US$) · Daily spend limit (USD) | Avisa em 80% e 100% do custo estimado do dia; 0 desliga. | `usage` |
| `usage.budget.monthly_usd` | número | `0` — desligado até a pessoa definir um teto; 0 desliga. | 0–100000 | Limite de gasto mensal (US$) · Monthly spend limit (USD) | Avisa em 80% e 100% do custo estimado do mês; 0 desliga. | `usage` |
| `usage.display.status_bar` | alternar | sim - Custo visível evita surpresa no fim do dia. | - | Mostrar uso na barra de status · Show usage in status bar | Mostra o uso acumulado na barra de status. | `usage` |
| `accounts.confirm.remove` | alternar | sim - A pasta guarda a sessão e as credenciais do CLI; apagar sem confirmação é irreversível. | - | Confirmar remoção de conta · Confirm account removal | Pede confirmação antes de apagar a pasta de uma conta. | `accounts` |

## 3.6 Mutirão (`muster`)

Mutirão define o plano, o despacho, os recados e as entregas da coordenação multiajudante. O padrão é aprovar antes de gastar recursos e exigir evidência antes de aceitar trabalho.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `muster.mode.enabled` | alternar | sim — Mutirão é o diferencial do produto e deve estar disponível. | — | Mutirão · Muster | Liga a coordenação de vários ajudantes. | `muster` |
| `muster.coordinator.profile` | escolha | `auto` — Auto usa o perfil padrão e deixa o projeto escolher outro. | `auto` + descobertos de `profiles` | Perfil do coordenador · Coordinator profile | Perfil que coordena o mutirão. | `muster` |
| `muster.plan.max_tasks` | número | `20` — 20 tarefas é grande o bastante sem virar um plano impossível de revisar. | 1–100 | Máximo de tarefas · Maximum tasks | Máximo de tarefas propostas num plano. | `muster` |
| `muster.plan.require_approval` | alternar | sim — Nada é despachado antes de você ver o plano. | — | Aprovar o plano · Approve the plan | Pede aprovação do plano antes de iniciar as tarefas. | `muster` |
| `muster.dispatch.max_parallel` | número | `4` — 4 equilibra paralelismo e consumo de recursos. | 1–16 | Ajudantes em paralelo · Parallel helpers | Máximo de ajudantes trabalhando ao mesmo tempo. | `muster` |
| `muster.dispatch.target` | escolha | `ask` — Perguntar respeita o isolamento que cada trabalho pede. | `ask`, `current`, `new` | Onde despachar · Dispatch target | Onde cada tarefa é despachada. | `muster` |
| `muster.notes.poll_s` | número | `3` — 3 s mantém a caixa de entrada viva sem inundar o pipe. | 1–60 | Leitura de recados (s) · Notes polling (s) | Intervalo de leitura de recados dos ajudantes. | `muster` |
| `muster.notes.retention_days` | número | `30` — 30 dias cobre a revisão de um mutirão sem guardar ruído para sempre. | 1–365 | Retenção de recados (dias) · Notes retention (days) | Tempo de retenção dos recados. | `muster` |
| `muster.handoff.require_evidence` | alternar | sim — Entrega sem evidência não é verificável. | — | Exigir evidência na entrega · Require handoff evidence | Exige evidências na entrega de cada tarefa. | `muster` |
| `muster.escalation.max_retries` | número | `2` — Duas tentativas resolvem erro transitório sem repetir em loop. | 0–10 | Reenvios por tarefa · Retries per task | Quantas vezes uma tarefa que falha é reenviada. | `muster` |
| `muster.budget.max_usd` | número | `0` — 0 desliga o teto; quem precisa de limite escolhe um valor. | 0–10000 | Teto de custo (US$) · Cost ceiling (USD) | Teto de custo por mutirão; 0 desliga. | `muster` |
| `muster.cleanup.on_finish` | escolha | `ask` — Perguntar evita apagar bancadas que ainda têm trabalho. | `ask`, `keep`, `remove` | Limpar bancadas ao terminar · Clean up benches on finish | O que fazer com as bancadas ao terminar o mutirão. | `muster` |
| `muster.notifications.task_failed` | alternar | sim — Falha de tarefa exige decisão do coordenador. | — | Avisar falha de tarefa · Notify task failure | Notifica quando uma tarefa do mutirão falha. | `muster` |
| `muster.workspace.isolation` | escolha | `worktree` — Worktree é o isolamento que evita conflito entre ajudantes. | `worktree`, `shared` | Isolamento das tarefas · Task isolation | Define se tarefas usam bancadas isoladas ou a mesma bancada. | `muster` |

## 3.7 Rotinas (`routines`)

Rotinas define horário, alvo, limites e histórico das ações agendadas. Rotina que falha em silêncio é rotina que não existe.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `routine.mode.enabled` | alternar | sim — Rotina é parte do produto e começa disponível. | — | Rotinas · Routines | Liga a execução de rotinas agendadas. | `routine` |
| `routine.schedule.timezone` | escolha | `system` — System segue o fuso do Windows e evita horário deslocado. | `system`, `utc` | Fuso horário · Time zone | Fuso usado para interpretar os horários. | `routine` |
| `routine.schedule.missed` | escolha | `run_once` — Rodar uma vez recupera o atraso sem repetir a série inteira. | `skip`, `run_once`, `ask` | Rotina perdida · Missed routine | O que fazer quando o horário passou com a Forja fechada. | `routine` |
| `routine.schedule.grace_s` | número | `120` — 2 min absorve suspensão e relógio impreciso. | 0–3600 | Tolerância de atraso (s) · Schedule grace (s) | Tolerância para executar uma rotina atrasada. | `routine` |
| `routine.run.timeout_min` | número | `60` — 1 h cobre a maioria dos trabalhos de agente sem travar a fila. | 1–720 | Tempo máximo (min) · Maximum runtime (min) | Tempo máximo de execução de uma rotina. | `routine` |
| `routine.run.concurrency` | número | `1` — Uma por vez evita dois agentes disputando a mesma bancada. | 1–8 | Rotinas simultâneas · Concurrent routines | Quantas rotinas podem rodar ao mesmo tempo. | `routine` |
| `routine.run.target` | escolha | `ask` — Perguntar respeita rotinas de revisão e rotinas de trabalho novo. | `ask`, `fixed`, `new` | Alvo padrão · Default target | Onde a rotina trabalha quando não define um alvo. | `routine` |
| `routine.logs.retention_days` | número | `30` — 30 dias cobre a investigação de uma falha recorrente. | 1–365 | Retenção do histórico (dias) · History retention (days) | Tempo de retenção do histórico de execuções. | `routine` |
| `routine.notifications.failure` | alternar | sim — Rotina falha em silêncio é rotina que não existe. | — | Avisar falha · Notify failure | Notifica quando uma rotina falha. | `routine` |
| `routine.notifications.success` | alternar | não — Sucesso a cada hora vira ruído; o histórico já registra. | — | Avisar sucesso · Notify success | Notifica quando uma rotina termina com sucesso. | `routine` |

## 3.8 Pedidos e balcões (`requests`)

Pedidos e balcões liga o trabalho ao pedido de origem, seja local ou vindo de GitHub, GitLab, Linear ou Jira. Credenciais não são configurações: ficam no cofre do sistema.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `request.default.source` | escolha | `local` — Pedido local funciona offline e não exige conta. | `local`, `github`, `gitlab`, `linear`, `jira` | Balcão padrão · Default counter | Balcão usado quando o projeto não define um. | `request` |
| `request.create.branch_from_title` | alternar | sim — Título legível vira branch legível sem digitação. | — | Branch a partir do título · Branch from title | Gera o nome da branch a partir do título do pedido. | `request` |
| `request.create.assign_self` | alternar | sim — Quem cria o pedido normalmente é quem vai forjar. | — | Atribuir a mim · Assign to me | Atribui o pedido a você ao criar uma bancada. | `request` |
| `request.filters.default` | texto | (vazio) — Vazio mostra tudo; filtro salvo é preferência de quem tem fila grande. | expressão de filtro | Filtro padrão · Default filter | Filtro aplicado ao abrir a lista de pedidos. | `request` |
| `request.refresh.interval_s` | número | `60` — 1 min mantém a lista atual sem bater no balcão. | 0–600 | Atualização dos pedidos (s) · Request refresh (s) | Intervalo de atualização dos pedidos; 0 desliga. | `request` |
| `request.sync.stage_on_pr` | alternar | sim — Abrir PR é o sinal natural de revisão. | — | Revisão ao abrir PR · Review on PR open | Move o pedido para revisão quando o PR é aberto. | `request` |
| `request.sync.close_on_merge` | alternar | sim — Merge é a entrega objetiva do pedido. | — | Fechar ao mesclar PR · Close on PR merge | Fecha o pedido quando o PR é mesclado. | `request` |
| `request.local.prefix` | texto | "LOCAL-" — O prefixo distingue pedido local de número de balcão. | texto | Prefixo de pedido local · Local request prefix | Prefixo dos pedidos criados localmente. | `request` |
| `request.local.counter_start` | número | `1` — Começar em 1 é previsível para leitura humana. | 1–100000 | Primeiro pedido local · First local request | Número inicial dos pedidos locais. | `request` |
| `counter.github.host` | texto | "github.com" — github.com cobre o caso comum; empresa usa o host próprio. | host | Host do GitHub · GitHub host | Host do GitHub usado pelo balcão. | `counter` |
| `counter.github.include_drafts` | alternar | não — Rascunho ainda não é pedido para forjar. | — | Incluir rascunhos · Include drafts | Inclui pull requests em rascunho na lista. | `counter` |
| `counter.github.label_filter` | lista | `[]` — Vazio traz tudo; rótulos são o filtro que cada equipe já usa. | lista de rótulo | Filtro de rótulos · Label filter | Rótulos que um pedido precisa ter para aparecer. | `counter` |
| `counter.gitlab.host` | texto | "gitlab.com" — gitlab.com cobre o caso comum; instâncias próprias trocam o host. | host | Host do GitLab · GitLab host | Host do GitLab usado pelo balcão. | `counter` |
| `counter.linear.team` | texto | (vazio) — Vazio mostra todas as equipes visíveis. | equipe | Equipe do Linear · Linear team | Equipe do Linear usada como fonte de pedidos. | `counter` |
| `counter.jira.project` | texto | (vazio) — Vazio mostra todos os projetos visíveis. | chave | Projeto do Jira · Jira project | Projeto do Jira usado como fonte de pedidos. | `counter` |
| `counter.poll.interval_s` | número | `300` — 5 min é suficiente para pedidos e não gasta cota de API. | 30–3600 | Consulta aos balcões (s) · Counter polling (s) | Intervalo de consulta aos balcões. | `counter` |
| `counter.notifications.new_request` | alternar | sim — Pedido novo é trabalho que pode entrar na fila. | — | Avisar pedido novo · Notify new request | Notifica quando aparece um pedido novo. | `counter` |
| `counter.notifications.checks_failed` | alternar | sim — Check quebrado bloqueia a entrega e precisa de ação. | — | Avisar check quebrado · Notify failed checks | Notifica quando um check do PR falha. | `counter` |

## 3.9 Integrações (`integrations`)

Integrações cobre MCP, navegador interno e webhooks. Toda saída de dados é opt-in e toda credencial fica no cofre, nunca neste catálogo.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `mcp.client.enabled` | alternar | sim — MCP é como modelos operam a Forja; vem ligado. | — | Servidores MCP · MCP servers | Liga servidores MCP configurados. | `mcp` |
| `mcp.startup.timeout_s` | número | `20` — 20 s cobre servidor local que carrega ferramentas. | 1–120 | Tempo de início MCP (s) · MCP startup timeout (s) | Tempo máximo para um servidor MCP iniciar. | `mcp` |
| `mcp.tools.auto_approve` | lista | `[]` — Aprovar ferramenta por ferramenta é o padrão seguro; vazio não aprova nada. | lista de ferramenta | Ferramentas aprovadas · Approved tools | Ferramentas MCP aprovadas sem perguntar. | `mcp` |
| `mcp.log.requests` | alternar | não — Log de MCP pode conter dados do projeto; desligado por padrão. | — | Registrar requisições MCP · Log MCP requests | Registra requisições MCP no log de desenvolvimento. | `mcp` |
| `mcp.server.enabled` | alternar | sim — Agentes operam a Forja por MCP; leitura é segura e escrita pede aprovação. | — | Servidor MCP da Forja · Forja MCP server | Liga o servidor MCP que expõe a Forja a agentes. | `mcp` |
| `mcp.server.expose` | lista | `[]` — Expor além do padrão é escolha explícita; ferramenta destrutiva e de aprovação nunca entram. | lista de ferramenta | Ferramentas expostas · Exposed tools | Ferramentas além do padrão, como comandos de extensão. | `mcp` |
| `mcp.server.auto_approve` | lista | `[]` — Aprovar cada escrita é o padrão seguro; ferramenta destrutiva não pode entrar aqui. | lista de ferramenta | Escritas aprovadas · Auto-approved writes | Ferramentas de escrita de baixo risco que rodam sem perguntar. | `mcp` |
| `mcp.server.timeout_s` | número | `120` — Dois minutos dão tempo de olhar a janela sem deixar o agente esperando para sempre. | 1–600 | Tempo para aprovar (s) · Approval timeout (s) | Tempo para responder a um pedido de aprovação de agente antes de negar. | `mcp` |
| `browser.engine.kind` † | escolha | `embedded` — A aba interna mantém o contexto da bancada; o sistema é alternativa. | `embedded`, `system` | Navegador · Browser | A aba interna usa o WebView2; sistema abre a página no navegador padrão do Windows. | `browser` |
| `browser.home.url` | texto | "forja://inicio" — A página inicial local não faz requisição externa. | URL | Página inicial · Home page | Página aberta em uma aba nova do navegador. | `browser` |
| `browser.search.engine` | escolha | `duckduckgo` — DuckDuckGo é o padrão que menos perfil traça. | `duckduckgo`, `bing`, `google` | Mecanismo de busca · Search engine | Mecanismo de busca da barra de endereço. | `browser` |
| `browser.privacy.third_party_cookies` | alternar | sim — Bloquear rastreadores de terceiros é o padrão de privacidade. | — | Bloquear cookies de terceiros · Block third-party cookies | Bloqueia cookies de terceiros no navegador interno. | `browser` |
| `browser.privacy.do_not_track` | alternar | sim — Sinalizar não rastrear não custa nada e é intenção clara. | — | Enviar Do Not Track · Send Do Not Track | Envia o cabeçalho Do Not Track. | `browser` |
| `browser.devtools.enabled` | alternar | não — Devtools é ferramenta de quem desenvolve o app, não do fluxo comum. | — | Ferramentas de desenvolvedor · Developer tools | Permite abrir as ferramentas de desenvolvedor da aba. | `browser` |
| `browser.download.dir` | caminho | `%USERPROFILE%\Downloads` — Downloads é onde o Windows e o usuário já procuram arquivos. | pasta | Pasta de downloads · Downloads folder | Pasta de destino dos downloads. | `browser` |
| `browser.download.ask_each` | alternar | sim — Perguntar evita arquivo baixado para lugar inesperado. | — | Perguntar onde salvar · Ask where to save | Pergunta onde salvar cada download. | `browser` |
| `browser.zoom.default` | número | `1.0` — 100% mostra o site como foi desenhado. | 0.5–2.0, passo 0.1 | Zoom padrão · Default zoom | Zoom padrão das páginas. | `browser` |
| `browser.tabs.restore` | alternar | sim — Retomar as páginas abertas é o comportamento esperado de aba. | — | Restaurar páginas · Restore pages | Reabre as páginas da última sessão. | `browser` |
| `browser.auth.open_external` | alternar | não — Login no navegador do sistema evita copiar sessão para o app. | — | Login no navegador do sistema · Sign in with system browser | Abre fluxos de login no navegador do sistema. | `browser` |
| `integrations.webhook.enabled` | alternar | não — Webhook é saída de dados; só liga com intenção explícita. | — | Webhook · Webhook | Envia eventos da Forja para uma URL. | `integrations` |
| `integrations.webhook.url` | texto | (vazio) — Vazio não envia nada; URL só existe quando o recurso é ligado. | https:// | URL do webhook · Webhook URL | URL que recebe os eventos do webhook. | `integrations` |
| `integrations.webhook.events` | lista | `[]` — Lista vazia não envia nada; eventos são escolhidos um a um. | lista de evento | Eventos do webhook · Webhook events | Eventos enviados ao webhook. | `integrations` |

## 3.10 Remoto (SSH/WSL) (`remote`)

Remoto cobre SSH e WSL: conexão, validação de host, sincronização e conforto de latência. O padrão não encaminha chaves nem sincroniza arquivos sem intenção.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `remote.ssh.enabled` | alternar | sim — SSH é o caminho padrão para trabalhar em outra máquina. | — | SSH · SSH | Liga o suporte a bancadas e terminais remotos por SSH. | `remote` |
| `remote.ssh.config_path` | caminho | `~/.ssh/config` — Reaproveita hosts e chaves que o usuário já mantém. | arquivo | Arquivo de configuração SSH · SSH config file | Arquivo de configuração SSH lido pela Forja. | `remote` |
| `remote.ssh.default_host` | texto | (vazio) — Vazio pergunta o host; não há servidor padrão da Forja. | host | Host SSH padrão · Default SSH host | Host SSH sugerido ao abrir uma conexão. | `remote` |
| `remote.ssh.connect_timeout_s` | número | `20` — 20 s cobre rede lenta sem parecer travado. | 5–120 | Tempo de conexão (s) · Connection timeout (s) | Tempo máximo para estabelecer uma conexão SSH. | `remote` |
| `remote.ssh.keep_alive_s` | número | `30` — 30 s mantém a conexão viva atrás de NAT sem tráfego excessivo. | 0–300 | Keep-alive SSH (s) · SSH keep-alive (s) | Intervalo de keep-alive da conexão SSH; 0 desliga. | `remote` |
| `remote.ssh.reuse_connections` | alternar | sim — Reutilizar conexão reduz latência e novas autenticações. | — | Reutilizar conexões · Reuse connections | Reaproveita conexões SSH abertas. | `remote` |
| `remote.ssh.agent_forward` | alternar | não — Encaminhar o agente expõe chaves à máquina remota; opt-in. | — | Encaminhar agente SSH · Forward SSH agent | Encaminha o agente SSH para a máquina remota. | `remote` |
| `remote.ssh.host_key_policy` | escolha | `ask` — Perguntar na primeira conexão equilibra segurança e uso. | `strict`, `ask`, `accept_new` | Validar chave do host · Host key policy | Como validar a chave do host SSH. | `remote` |
| `remote.wsl.default_distro` | texto | (vazio) — Vazio usa a distro padrão do WSL. | distro | Distribuição WSL padrão · Default WSL distro | Distribuição WSL usada por padrão. | `remote` |
| `remote.wsl.default_user` | texto | (vazio) — Vazio usa o usuário padrão da distro. | usuário | Usuário WSL padrão · Default WSL user | Usuário WSL usado por padrão. | `remote` |
| `remote.sync.mode` | escolha | `manual` — Manual evita sobrescrever trabalho remoto ou local sem intenção. | `manual`, `push`, `pull` | Sincronização remota · Remote sync | Como arquivos são sincronizados com uma bancada remota. | `remote` |
| `remote.forward.mode` | escolha | `ask` — Perguntar evita abrir porta sem intenção. | `ask`, `auto`, `off` | Encaminhamento de portas · Port forwarding | Como portas detectadas numa sessão remota são encaminhadas. | `remote` |
| `remote.latency.warning_ms` | número | `300` — 300 ms é o ponto em que digitar em terminal remoto começa a incomodar. | 50–2000 | Aviso de latência (ms) · Latency warning (ms) | Avisa quando a latência da conexão passa desse valor. | `remote` |

## 3.11 Extensões (`extensions`)

Extensões define o isolamento das extensões Lua, os limites de execução e o que cada capacidade pode alcançar. O padrão é estrito: a extensão só chega ao que pediu e você aprovou.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `extension.mode.enabled` | alternar | sim — Extensões são parte do produto e começam disponíveis. | — | Extensões · Extensions | Liga o carregamento de extensões. | `extension` |
| `extension.sandbox.mode` | escolha | `strict` — Estrito é o padrão: a extensão só alcança o que pediu e foi aprovado. | `strict`, `ask` | Isolamento · Sandbox | Estrito usa só o que foi aprovado na instalação; perguntar abre uma aprovação em tempo de execução. | `extension` |
| `extension.update.check` | escolha | `weekly` — Semanal encontra correção sem consultar a rede todo dia. | `never`, `daily`, `weekly` | Verificar atualizações · Check for updates | Frequência de verificação de atualizações de extensões. | `extension` |
| `extension.update.auto` | alternar | não — Atualizar sozinho pode mudar comportamento no meio do trabalho. | — | Atualizar automaticamente · Auto-update | Atualiza extensões automaticamente. | `extension` |
| `extension.lua.memory_mb` | número | `64` — 64 MB cobre automação de texto e impede uma extensão de engolir a máquina. | 8–512 | Memória por extensão (MB) · Memory per extension (MB) | Memória máxima de uma extensão Lua. | `extension` |
| `extension.lua.timeout_ms` | número | `1000` — 1 s evita extensão travada bloquear a interface. | 100–10000 | Tempo máximo Lua (ms) · Lua timeout (ms) | Tempo máximo de execução de um trecho Lua. | `extension` |
| `extension.fs.allow_outside_project` | alternar | não — Fora do projeto é onde ficam configs de outros apps; a capacidade é pedida e aprovada caso a caso. | — | Permitir pedido fora do projeto · Allow outside-project requests | Permite que uma extensão peça acesso fora do projeto; a aprovação continua por extensão. | `extension` |
| `extension.network.allow` | alternar | não — Rede não é necessária para a maioria das extensões. | — | Permitir pedido de rede · Allow network requests | Permite que uma extensão peça acesso à rede; a aprovação continua por extensão. | `extension` |
| `extension.process.allow_spawn` | alternar | não — Criar processos é a capacidade mais perigosa e fica desligada. | — | Permitir pedido de processo · Allow process requests | Permite que uma extensão peça para criar processos; a aprovação continua por extensão. | `extension` |
| `extension.log.level` | escolha | `warn` — Warn mostra problema real sem afogar em detalhe. | `error`, `warn`, `info`, `debug` | Nível de log · Log level | Nível de log das extensões. | `extension` |
| `extension.dev.watch` | alternar | não — Recarregar a cada arquivo é útil só enquanto se desenvolve uma extensão. | — | Recarregar ao editar · Reload on edit | Recarrega extensões ao detectar mudança nos arquivos. | `extension` |
| `content.reload.watch` | alternar | sim — Editar arquivo e ver o efeito na hora é o ciclo que torna a Forja extensível sem compilar. | — | Recarregar conteúdo ao editar · Reload content on edit | Recarrega comandos, temas, atalhos e receitas quando os arquivos mudam. | `content` |
| `content.trust.review_on_change` | alternar | sim — Conteúdo de repositório pode mudar depois da confirmação; mudança de item de risco pede nova decisão. | — | Pedir nova confirmação ao mudar item de risco · Ask again when a risky item changes | Pede nova confirmação de uma pasta `.forja` do projeto quando receita ou perfil com permissão ampla muda. | `content` |

## 3.12 Privacidade (`privacy`)

Privacidade reúne histórico local, segredos, cookies, proxy e rede. A Forja não tem telemetria nem nuvem própria; o que sai da máquina é sempre uma ação visível.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `privacy.history.enabled` | alternar | sim — Histórico local ajuda a voltar atrás sem depender de nuvem. | — | Histórico local · Local history | Guarda histórico local de edições e navegação. | `privacy` |
| `privacy.history.retention_days` | número | `180` — Meio ano cobre retomada de projeto sem crescer para sempre. | 0–3650 | Retenção do histórico (dias) · History retention (days) | Tempo de retenção do histórico local; 0 mantém para sempre. | `privacy` |
| `privacy.clipboard.clear_after_s` | número | `0` — 0 desliga; limpar a área de transferência é preferência de segurança. | 0–300 | Limpar área de transferência (s) · Clear clipboard (s) | Limpa a área de transferência depois desse tempo; 0 desliga. | `privacy` |
| `privacy.logs.redact_secrets` | alternar | sim — Log não deve virar vazamento de token. | — | Ocultar segredos nos logs · Redact secrets in logs | Remove tokens e senhas dos logs. | `privacy` |
| `privacy.secrets.backend` | escolha | `credential_manager` — O cofre do Windows é o lugar certo para segredo. | `credential_manager`, `file` | Cofre de segredos · Secrets vault | Onde os segredos são guardados. | `privacy` |
| `privacy.secrets.lock_on_idle_min` | número | `15` — 15 min reduz a janela de cofre aberto sem irritar. | 0–120 | Bloquear cofre após (min) · Lock vault after (min) | Bloqueia o cofre após inatividade; 0 desliga. | `privacy` |
| `privacy.usage.local_only` | alternar | sim — Dados de uso são locais, como o resto do produto. | — | Uso só nesta máquina · Usage local only | Mantém o registro de uso apenas nesta máquina. | `privacy` |
| `privacy.assistant.share_workspace` | escolha | `ask` — Compartilhar arquivo com modelo é decisão do usuário, não padrão. | `ask`, `always`, `never` | Compartilhar com ajudante · Share with helper | Define o que é enviado ao ajudante quando ele pede contexto. | `privacy` |
| `privacy.browser.clear_on_exit` | alternar | não — Limpar tudo ao sair apaga logins que o usuário quer manter. | — | Limpar navegador ao sair · Clear browser on exit | Limpa cookies e cache ao fechar o navegador. | `privacy` |
| `privacy.browser.cookies_days` | número | `30` — 30 dias cobre sessões de trabalho sem guardar rastreador para sempre. | 0–365 | Retenção de cookies (dias) · Cookie retention (days) | Retenção de cookies do navegador; 0 mantém apenas a sessão. | `privacy` |
| `network.proxy.mode` | escolha | `system` — System respeita o proxy corporativo sem configuração extra. | `system`, `none`, `manual` | Proxy · Proxy | Como a Forja escolhe o proxy de rede. | `network` |
| `network.proxy.url` | texto | (vazio) — Vazio só é válido no modo manual com URL preenchida. | http://host:porta | URL do proxy · Proxy URL | Endereço do proxy manual. | `network` |
| `network.proxy.bypass` | lista | `[localhost, 127.0.0.1]` — Serviços locais não devem passar pelo proxy. | lista de host | Hosts sem proxy · Proxy bypass | Hosts que ignoram o proxy. | `network` |
| `network.proxy.auth` | escolha | `none` — Sem autenticação é o padrão; o modo depende da rede. | `none`, `basic`, `ntlm` | Autenticação do proxy · Proxy authentication | Tipo de autenticação do proxy. | `network` |
| `network.tls.verify` | alternar | sim — Desligar verificação de TLS não é uma opção de conveniência. | — | Verificar TLS · Verify TLS | Verifica certificados TLS nas conexões. | `network` |
| `network.http.timeout_s` | número | `30` — 30 s cobre rede lenta sem segurar a interface. | 5–120 | Tempo limite de rede (s) · Network timeout (s) | Tempo máximo de uma requisição de rede. | `network` |
| `network.mode.offline` | alternar | não — Offline é um modo explícito para trabalhar sem rede. | — | Modo offline · Offline mode | Bloqueia todo acesso de rede da Forja. | `network` |

## 3.13 Avançado (`advanced`)

Avançado cobre Bigorna, atalhos, armazenamento, atualizações, logs e desempenho. São ajustes de operação: mudam pouco, mas precisam ser claros quando mudam.
O nível do log, a rotação por tamanho, a retenção de arquivos e a inclusão opcional da saída do terminal já são aplicados pelo serviço local. A saída do terminal continua desligada por padrão porque pode conter segredos.

| Chave | Tipo | Padrão (por quê) | Faixa / opções | Rótulo pt-BR · en | Ajuda | Dono |
|---|---|---|---|---|---|---|
| `anvil.history.limit` | número | `100` — 100 comandos recentes cobrem o dia sem virar um diário. | 10–1000 | Histórico da Bigorna · Anvil history | Quantos comandos ficam no histórico da Bigorna. | `anvil` |
| `anvil.history.show_recent` | alternar | sim — Recentes são o atalho mais usado na paleta. | — | Mostrar recentes · Show recent | Mostra comandos recentes ao abrir a Bigorna. | `anvil` |
| `keymap.file.custom_path` | caminho | `~/.forja/atalhos.toml` — Um arquivo de texto editável por pessoa e por modelo. | arquivo | Arquivo de atalhos · Keymap file | Arquivo de atalhos personalizados. | `keymap` |
| `keymap.conflict.policy` | escolha | `warn` — Avisar ensina; bloquear protege quem já tem atalhos demais. | `warn`, `block` | Conflito de atalho · Shortcut conflict | O que fazer quando um atalho personalizado conflita. | `keymap` |
| `app.startup.open_last` | alternar | sim — Retomar o último projeto é o fluxo de quem trabalha todo dia. | — | Abrir último projeto · Open last project | Abre o último projeto e a última bancada ao iniciar a Forja. | `app` |
| `app.close.confirm` | alternar | sim — Fechar com ajudante ou rotina ativa não pode ser silencioso. | — | Confirmar ao fechar · Confirm on close | Pede confirmação ao fechar com trabalho em andamento. | `app` |
| `app.tray.enabled` | alternar | não — Bandeja é opcional; fechar deve fechar, não esconder. | — | Bandeja do Windows · Windows tray | Mantém a Forja na bandeja do Windows. | `app` |
| `app.cli.enabled` | alternar | sim — O CLI forja é a interface dos modelos e da automação. | — | CLI e RPC local · Local CLI and RPC | Liga o servidor local do CLI e do RPC. | `app` |
| `app.storage.data_dir` † | caminho | `%APPDATA%\Forja` — A pasta de estado do usuário é o lugar padrão no Windows. | pasta | Pasta de estado · State folder | Pasta de estado, configurações e bancadas; exige reinício. | `app` |
| `app.storage.content_dir` † | caminho | `~/.forja` — Conteúdo autoral do usuário fica separado do estado da máquina. | pasta | Pasta de conteúdo · Content folder | Pasta de comandos, perfis, temas e atalhos; exige reinício. | `app` |
| `app.storage.cleanup_days` | número | `30` — 30 dias limpa cache e logs antigos sem tocar em trabalho. | 0–365 | Limpeza de cache (dias) · Cache cleanup (days) | Remove cache antigo após esse prazo; 0 desliga. | `app` |
| `update.channel.name` | escolha | `stable` — Stable é o padrão; beta é para quem quer testar antes. | `stable`, `beta` | Canal de atualização · Update channel | Canal de atualizações do app. | `update` |
| `update.check.frequency` | escolha | `daily` — Diário encontra correção sem consultar a rede a cada abertura. | `startup`, `daily`, `weekly`, `never` | Verificar atualizações · Check for updates | Frequência de verificação de atualização do app. | `update` |
| `update.download.auto` | alternar | sim — Baixar em segundo plano deixa a instalação rápida e reversível. | — | Baixar atualizações · Download updates | Baixa atualizações automaticamente, sem instalar. | `update` |
| `app.log.level` | escolha | `info` — Info registra o suficiente para suporte sem inundar o disco. | `error`, `warn`, `info`, `debug`, `trace` | Nível de log · Log level | Nível de detalhe do log do app. | `app` |
| `app.log.max_mb` | número | `20` — 20 MB por arquivo mantém histórico útil sem crescer sem limite. | 1–100 | Tamanho do log (MB) · Log size (MB) | Tamanho máximo de cada arquivo de log. | `app` |
| `app.log.keep` | número | `5` — 5 arquivos cobrem uma semana de uso normal. | 1–20 | Arquivos de log mantidos · Log files kept | Quantos arquivos de log são mantidos. | `app` |
| `app.log.include_terminal_output` | alternar | não — Saída de terminal pode conter segredo; não vai para o log por padrão. | — | Incluir saída do terminal · Include terminal output | Inclui a saída do terminal nos logs. | `app` |
| `app.performance.max_fps` | número | `60` — 60 fps é fluido e economiza bateria. | 0–240 | Quadros por segundo · Frames per second | Limite de quadros com a janela em foco; 0 tira o limite. | `app` |
| `app.performance.unfocused_fps` | número | `10` — 10 fps mostra recado e tarefa chegando sem gastar bateria na janela de trás. | 0–120 | Quadros sem foco · Frames without focus | Limite de quadros com a janela em segundo plano; 0 tira o limite. | `app` |
| `app.diagnostics.overlay` | alternar | não — Medir antes de otimizar: sem números, ajuste de desempenho é chute. | — | Painel de diagnóstico · Diagnostics panel | Mostra quadros por segundo, tempo do quadro, memória e threads sobre a janela. | `app` |
| `app.performance.worker_threads` † | número | `0` — 0 deixa o sistema escolher entre 2 e 8 threads, conforme os núcleos. | 0–32 | Threads de trabalho · Worker threads | Threads da fila de tarefas de fundo; 0 escolhe entre 2 e 8 conforme os núcleos; exige reinício. | `app` |
| `app.dev.mode` | alternar | não - Modo de desenvolvimento expõe ferramentas internas; desligado por padrão. | - | Modo de desenvolvimento · Development mode | Liga ferramentas internas e o painel de diagnóstico. | `app` |
| `ports.scan.interval_s` | número | `30` - 30 s mostra um servidor recém-aberto sem varrer o sistema o tempo todo. | 0-600 | Varredura de portas (s) · Port scan (s) | Intervalo da varredura de portas em segundo plano; 0 desliga. | `ports` |
| `ports.confirm.kill` | alternar | sim - Um PID pode ser o terminal do próprio trabalho; confirmar evita perder estado. | - | Confirmar encerramento · Confirm process termination | Pede confirmação antes de encerrar o processo de uma porta. | `ports` |

## 4. O que deliberadamente não teremos

Estas ausências são decisões de produto, não lacunas. Se alguém pedir uma delas, a resposta começa por aqui.

- **Telemetria, analytics e relatório de falha enviado sozinho.** O produto não manda nada para nós. Diagnóstico é local e exportável por ação explícita.
- **Conta, nuvem ou sincronização da Forja.** Não há login nosso, link público, perfil em nuvem nem backup remoto.
- **Pareamento com celular, emuladores e VMs efêmeras.** Está fora do produto por decisão.
- **Loja de temas, atalhos ou extensões com instalação de uma vez.** Tema é arquivo; extensão pede capacidades e você aprova.
- **`auto_update_install`.** A Forja baixa, mas nunca instala sem confirmação; não existe configuração para pular essa etapa.
- **Aprovação global de ferramentas MCP.** A lista é explícita, ferramenta por ferramenta; não há "confiar em tudo".
- **Chave de API de provedor de modelo nos Ajustes.** Chave fica no ambiente ou no cofre, referenciada pelo perfil do ajudante; o catálogo não duplica a configuração do provedor.
- **Token de GitHub, GitLab, Linear ou Jira nos Ajustes.** O balcão usa o cofre do sistema; `settings.toml` só tem host e filtros.
- **Senha de proxy e certificado TLS desativado como conveniência.** Proxy usa o cofre; `network.tls.verify` não tem atalho para desligar em massa.
- **Fonte baixada pela Forja.** A interface usa Segoe UI Variable/Segoe UI e o código usa Cascadia Code, ambas do Windows.
- **CSS ou JavaScript na interface principal.** Tema é TOML de tokens; extensão roda em sandbox e não injeta estilo no shell.
- **Rollback de scrollback infinito.** O teto é 200 mil linhas por terminal; memória não é ilimitada.
- **"Rodar como administrador" no terminal.** A Forja não eleva processo por configuração; elevação é ação explícita do Windows.
- **Push forçado automático, amend automático e troca de branch com agente ativo.** Não são configurações; são operações que exigem comando e confirmação.
- **Gravação de tela, captura de desktop e leitura de outros apps.** Fora do escopo de privacidade do produto.

## 5. Credenciais e segredos

O catálogo não tem campo de senha, token ou chave privada. Credenciais ficam no **cofre do Windows** (`Credential Manager`) sob nomes `forja/<módulo>/<conta>`, e o schema guarda apenas a referência lógica. Quando o backend é `file` (fallback para ambiente sem cofre), o arquivo é cifrado com DPAPI e fica em `%APPDATA%\Forja\cofre.bin`; a configuração avisa que o cofre do sistema é o modo recomendado. Nenhum segredo é ecoado em log, RPC ou captura de tela.

## 6. Evolução do schema

- `schema_version = 1` no `settings.toml`. Mudança de tipo ou de significado exige versão nova e migração explícita.
- Configuração nova entra com padrão que preserva o comportamento anterior.
- Configuração renomeada ganha alias de leitura por uma versão; o valor migra na primeira gravação.
- Configuração removida continua legível como órfã por uma versão, com botão de apagar; nunca é aplicada em silêncio.
- Módulos declaram o schema no `register()`; o teste de contrato do kernel compara com este catálogo e falha se uma chave não existir dos dois lados.
