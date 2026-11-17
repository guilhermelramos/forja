# Item 7 — ícones próprios (Refs #196)

## Retomada

- Base: `1c9d608`; `git merge main` em 2026-07-21: já atualizado.
- Decisão visual já aprovada e comitada em `design/ui.md`: Traço de oficina; sem fonte de ícones, sem dependências novas.
- Trabalho parcial recuperado intacto (68 arquivos rastreados modificados + SVGs/gerador/testes novos); não reimplementado.
- Limpeza inicial: somente `target/` não rastreado, 26,62 GB → workspace 0,01 GB.

## Checklist

- [x] Ler AGENTS, roteiro, especificação e issue #196; integrar main.
- [ ] Validar SVGs, gerador, API, fontes de texto e contratos dos destinos.
- [ ] Validar migração nos módulos, trilho, abas, painéis e barra superior.
- [ ] Capturas antes/depois com `--demo`, 1280×720 e 1920×1080; árvore acessível ao vivo.
- [ ] Regressões completas em 1920×1080 e 1280×800.
- [ ] Build/test/clippy/proveniência/catálogo; commits sem push.

## Decisões de execução

- A instrução direta de continuar sem refazer prevalece sobre reiniciar implementação/testes já existentes.
- A instrução de executar sozinho e não parar para revisão prevalece sobre workflows de revisão delegada.
- A issue ainda descreve uma fonte embutida como mecanismo inicial; a decisão posterior do dono e a tarefa determinam SVG autoral → comandos egui, sem fonte.

## Evidências

A preencher com comandos, resultados e caminhos reais durante a verificação.
