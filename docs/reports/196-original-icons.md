# Item 7 — ícones próprios (Refs #196)

## Retomada

- Base: `1c9d608`; `git merge main` na retomada: já atualizado.
- Decisão visual já aprovada e comitada em `design/ui.md`: Traço de oficina; sem fonte de ícones, sem dependências novas.
- Trabalho parcial recuperado intacto (68 arquivos rastreados modificados + SVGs/gerador/testes novos); não reimplementado.
- Limpeza inicial: somente `target/` não rastreado, 26,62 GB → workspace 0,01 GB.

## Checklist

- [x] Ler AGENTS, roteiro, especificação e issue #196; integrar main.
- [x] Validar SVGs, gerador, API, fontes de texto e contratos dos destinos.
- [ ] Validar migração nos módulos, trilho, abas, painéis e barra superior.
- [ ] Capturas antes/depois com `--demo`, 1280×720 e 1920×1080; árvore acessível ao vivo.
- [ ] Regressões completas em 1920×1080 e 1280×800.
- [ ] Build/test/clippy/proveniência/catálogo; commits sem push.

## Decisões de execução

- A instrução direta de continuar sem refazer prevalece sobre reiniciar implementação/testes já existentes.
- A instrução de executar sozinho e não parar para revisão prevalece sobre workflows de revisão delegada.
- A issue ainda descreve uma fonte embutida como mecanismo inicial; a decisão posterior do dono e a tarefa determinam SVG autoral → comandos egui, sem fonte.

## Evidências

### Retomada e testes dirigidos

- `eecce88`: recupera e comita o conjunto de **80 SVGs**, gerador sem dependências, API vetorial, migrações e regressões 75–77.
- Gerador: 6 testes passaram inicialmente; adicionado teste de overrides SVG (preenchimento/espessura/ponta/opacidade), visto falhar com `fill='red'`; whitelist de atributos corrigida e **7/7 passaram**.
- `cargo test --workspace` inicial encontrou a opacidade de ícones desabilitados em 0,5, quando o contrato pede 0,4 (128 vs 102 de alfa). Corrigido usando o mesmo `painter_for` dos componentes existentes; teste dirigido passou.
- A árvore ao vivo revelou `project.tree.expand` sem tradução: a chave nova tinha prefixo incorreto `projects-`. Novo teste pt-BR/en falhou com o id cru, antes da correção dos dois FTLs.
- O sino da barra de status ainda era emoji. Novo teste de nome acessível falhou; agora usa `Icon::Bell`, conserva as cores de leitura e expõe o nome traduzido sem emoji.
- Removida a dica incorreta Ctrl+1 em Bancada: esse atalho troca a primeira aba de ferramenta, não é um atalho dedicado ao destino do trilho. Destinos sem atalho não inventam um.

### Evidência visual

`playtest/prove-icons.ps1` captura a bancada (trilho, faixa de abas, painéis esquerdo/direito e barra superior) e o Quadro, com `--demo --screenshot`, mais árvore bruta e transcrição `drive.ps1` em cada tamanho. A base anterior é `1c9d608`, extraída via `git archive` em `target/icon-baseline` e compilada sem tocar no trabalho corrente; executável de prova `target/forja-app-icons-before.exe`.

Capturas em `docs/reports/196-icons/`. A máquina usa DPI de 150%: preservamos `*-native.png` em 1920×1080 / 2880×1620 e publicamos também cópias em pixels lógicos de 1280×720 / 1920×1080, sem mudar o layout ou cortar conteúdo. Coordenadas acessíveis brutas são físicas; métricas registram o fator 1,5 e as dimensões lógicas dos controles.

Antes: controles de janela com aproximadamente 32 / 32,67 / 34 × 32 px lógicos. Resultados finais de build, testes, regressões e métricas depois serão registrados abaixo.
