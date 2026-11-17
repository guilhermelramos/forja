# Item 7 — ícones próprios (Refs #196)

## Retomada

- Base: `1c9d608`; `git merge main` na retomada: já atualizado.
- Decisão visual já aprovada e comitada em `design/ui.md`: Traço de oficina; sem fonte de ícones, sem dependências novas.
- Trabalho parcial recuperado intacto (68 arquivos rastreados modificados + SVGs/gerador/testes novos); não reimplementado.
- Limpeza inicial: somente `target/` não rastreado, 26,62 GB → workspace 0,01 GB.

## Checklist

- [x] Ler AGENTS, roteiro, especificação e issue #196; integrar main.
- [x] Validar SVGs, gerador, API, fontes de texto e contratos dos destinos.
- [x] Validar migração nos módulos, trilho, abas, painéis e barra superior.
- [x] Capturas antes/depois com `--demo`, 1280×720 e 1920×1080; árvore acessível ao vivo.
- [x] Regressões completas em 1920×1080 e 1280×800.
- [x] Build/test/clippy/proveniência/catálogo; commits sem push.

## Decisões de execução

- A instrução direta de continuar sem refazer prevalece sobre reiniciar implementação/testes já existentes.
- A instrução de executar sozinho e não parar para revisão prevalece sobre workflows de revisão delegada.
- A issue ainda descreve uma fonte embutida como mecanismo inicial; a decisão posterior do dono e a tarefa determinam SVG autoral → comandos egui, sem fonte.

## Entregas por contrato

1. **Fonte visual:** `design/ui.md` em `1c9d608` especifica ícones, cinco zonas/medidas de página, hierarquia de botões, escala 12/14/16-600/20-600/28 e barra superior; não antecipa a migração de layouts do item 8.
2. **Fontes autorais:** 80 SVGs em `design/icons/`, grade 24×24, traço 2, pontas/junções arredondadas; páginas, abas e painéis distintos, ações comuns e oito famílias de arquivos. Autoria e manutenção documentadas no README do conjunto.
3. **Gerador:** `cargo xtask icons`, sem dependência nova; M/L/H/V/C/Q/A/Z e circle/rect/line/polyline; arcos convertidos em cúbicas; tabela estática comitada e teste de igualdade com os SVGs. O renderer usa linhas, cúbicas e quadráticas egui, com escala e cor dos tokens.
4. **API e migrações:** `Icon`, `icon`, `icon_button` com rótulo obrigatório e `button_with_icon`; removidas fontes de ícones do Windows e os sinais usados como ícones no Navegador, janela, árvores e ações. Fontes de texto vêm de `%WINDIR%/Fonts`, com fallback e aviso no log do shell.
5. **Trilho e barra:** Bancada primeiro; desenhos de 20 px e indicador ativo de 3 px; dicas com nome e atalho real quando disponível; busca central e controles da janela com 46×32 px. Contrato de unicidade valida os registros reais de páginas/abas/painéis; regressões 75–77 validam os nomes acessíveis.

## Evidências

### Retomada e testes dirigidos

- `eecce88`: recupera e comita o conjunto de **80 SVGs**, gerador sem dependências, API vetorial, migrações e regressões 75–77.
- Gerador: 6 testes passaram inicialmente; adicionado teste de overrides SVG (preenchimento/espessura/ponta/opacidade), visto falhar com `fill='red'`; whitelist de atributos corrigida e **7/7 passaram**.
- `cargo test --workspace` inicial encontrou a opacidade de ícones desabilitados em 0,5, quando o contrato pede 0,4 (128 vs 102 de alfa). Corrigido usando o mesmo `painter_for` dos componentes existentes; teste dirigido passou.
- A árvore ao vivo revelou `project.tree.expand` sem tradução: a chave nova tinha prefixo incorreto `projects-`. Novo teste pt-BR/en falhou com o id cru, antes da correção dos dois FTLs.
- O sino da barra de status ainda era emoji. Novo teste de nome acessível falhou; agora usa `Icon::Bell`, conserva as cores de leitura e expõe o nome traduzido sem emoji.
- Removida a dica incorreta Ctrl+1 em Bancada: esse atalho troca a primeira aba de ferramenta, não é um atalho dedicado ao destino do trilho. Destinos sem atalho não inventam um; Mutirão e Ajustes mostram os atalhos reais cadastrados (Ctrl+Shift+M e Ctrl+Comma), em linha separada do nome acessível. Teste de formatação da dica visto falhar e passar.

### Evidência visual

`playtest/prove-icons.ps1` captura a bancada (trilho, faixa de abas, painéis esquerdo/direito e barra superior) e o Quadro, com `--demo --screenshot`, mais árvore bruta e transcrição `drive.ps1` em cada tamanho. A base anterior é `1c9d608`, extraída via `git archive` em `target/icon-baseline` e compilada sem tocar no trabalho corrente; executável de prova `target/forja-app-icons-before.exe`.

Capturas em `docs/reports/196-icons/`. A máquina usa DPI de 150%: preservamos `*-native.png` em 1920×1080 / 2880×1620 e publicamos também cópias em pixels lógicos de 1280×720 / 1920×1080, sem mudar o layout ou cortar conteúdo. Coordenadas acessíveis brutas são físicas; métricas registram o fator 1,5 e as dimensões lógicas dos controles.

| Métrica ao vivo | Antes | Depois (ambos os tamanhos) |
|---|---|---|
| Destinos nomeados no trilho | 10, sem Bancada explícita | 11, Bancada primeiro |
| Controles da janela | aproximadamente 32 / 32,67 / 34 × 32 px | todos **46×32 px** |
| Botões sem nome na bancada demonstrativa | 0 / 48 | **0 / 49** |
| Busca superior | dois gatilhos separados | central, “Buscar ou executar… Ctrl+K” |
| Sino | emoji no rótulo | traços de `Icon::Bell`, nome “0 avisos” |

Os botões da janela já possuíam nome na base; a melhoria é preservá-los durante a substituição dos sinais por vetores e fixar as áreas de clique. O teste de contrato percorre os destinos reais registrados no runtime (trilho, abas, laterais e painéis de extensões), recusando ícone desconhecido ou repetido.

### Verificação

- `cargo build --workspace`: passou (8,52 s na última execução incremental de todo o workspace).
- `cargo test --workspace`: **1.259 testes, zero falhas**, 114 executáveis/suítes incluindo doc-tests sem casos.
- `cargo clippy --workspace --all-targets -- -D warnings`: passou (9,66 s na última execução).
- `cargo xtask icons`: passou; sem diferença no gerado comitado.
- `cargo xtask provenance`: passou também com as capturas adicionadas ao índice; sem termos de proveniência recusados.
- `cargo xtask catalog`: passou; 281/314 configurações registradas (+1 extra), 280 com efeito, 2 ausências preexistentes; 110/110 atalhos; `design/coverage.md` sem alterações.
- Regressões completas: **74/74 em 1920×1080 e 74/74 em 1280×800**, repetidas após o ajuste final das dicas, ambas com saída 0 e stderr vazio. Logs `replay-<tamanho>.txt` e `replay-results.json` em `docs/reports/196-icons/`.

Commits de implementação: `eecce88`, `8aeacb7`, `09210ae`, `095da98` (todos Refs #196); o commit final acrescenta os resultados completos e a conclusão deste relatório. As pastas temporárias próprias da prova foram removidas; os artefatos de build foram mantidos conforme orientação do coordenador.

A primeira execução ao vivo das regressões identificou uma altura excessiva no novo sino da barra de status: a composição horizontal reservava 32 px dentro de uma área útil de 28 px. O teste foi reproduzido com os tokens reais (limite inferior 802 > 800), corrigido por medição explícita do rótulo e passou; as capturas finais mostram o botão dentro da janela. Uma tentativa de catálogo após compilar a base anterior no mesmo `target` reutilizou metadados antigos do ui-kit; limpeza **somente desse pacote** e reconstrução do código corrente eliminaram a contaminação, e o catálogo passou.

### Ajustes intencionais das regressões existentes

- 13: o alvo textual `↺` foi substituído pelo nome acessível “Descartar”; o diálogo e a verificação de Escape continuam iguais.
- 19: `+` virou “Nova aba”, preservando a ação do terminal com nome acessível.
- 20, 21 (dois cenários) e 48: gatilhos antigos da Bigorna seguem o nome da busca central.
- 10: seleciona explicitamente o painel Files, em vez de depender de um clique ambíguo no nome do projeto; continua validando o mesmo diálogo Novo arquivo.
- O resolvedor `ui.open_tab` diferencia o novo destino Bancada do trilho de uma aba de ferramenta com o mesmo título; a seleção usa a posição real do trilho, sem ids de glifos.

### Escopo preservado

Sem dependências novas, sem novo módulo, sem mudanças no manifesto raiz ou em `.github/`; layouts de página permanecem para o item 8. Ícones de projeto personalizados pelo usuário continuam sendo conteúdo fornecido pelo usuário; os ícones padrão internos são todos autorais. Nenhum ajudante real foi iniciado, nenhuma janela de teste recebeu foco, e não houve push nem fechamento de issue.
