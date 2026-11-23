# Roteiro de melhorias

Fonte de verdade da ordem de trabalho. Leia junto com o `AGENTS.md` e o `design/ui.md`.
Cada item tem uma issue com o rótulo `plano` (o número do item aparece no título como `[Item N]`).

**Regra:** nenhum módulo novo até a Etapa 3 fechar. O que falta é profundidade nos módulos que existem.

## Itens

| # | Faixa | Melhoria | Esforço | Issue |
| --- | --- | --- | --- | --- |
| 1 | A | Rede de segurança para falhas: captura de pânico, arquivo de log e diálogo de erro | P | #190 |
| 2 | A | Parar de redesenhar a 60 quadros por segundo com o app parado | P | #191 |
| 3 | A | Prova de fogo do terminal (modos 2026 e 1004, SGR 9, formato do cursor) | M | #192 |
| 4 | A | Consertar a faixa de abas | M | #193 |
| 5 | A | Tela inicial de verdade quando não há projeto | M | #194 |
| 6 | A | Lembrar o estado da janela e confirmar ao fechar | P | #195 |
| 7 | B | Ícones próprios embutidos, um por página, e botões com ícone | M | #196 |
| 8 | B | Molde único de página (cabeçalho, ações, filtros, conteúdo, estados) | M | #197 |
| 9 | B | Ampliar o `ui-kit` e proibir widget cru nos módulos (`cargo xtask ui-lint`) | G | #198 |
| 10 | B | Ajustes como página inteira, com navegação lateral | M | #199 |
| 11 | B | Hierarquia de botões e de texto | M | #200 |
| 12 | B | Limpar o que é detalhe de desenvolvedor | P | #201 |
| 13 | B | Sistema de movimento (animações) | M | #202 |
| 14 | B | Atualizar as capturas do README e do guia | P | #203 |
| 15 | C | Ligar as 41 configurações adiadas | G | #204 |
| 16 | C | Opções de hardware: renderizador, economia de energia, painel de diagnóstico | M | #205 |
| 17 | C | Otimizar o desenho do terminal e do tema | M | #206 |
| 18 | C | Acessibilidade: nomes nos controles | M | #207 |
| 19 | C | Atlas com dados de demonstração (`--demo`) | M | #208 |
| 20 | D | Atualizador (sem assinatura por enquanto) | M | #209 |
| 21 | D | Arrastar arquivo para o prompt | G | #210 |
| 22 | D | Celular e outros sistemas — fora do escopo por enquanto | G | #211 |
| 23 | D | [Layout modular em árvore](layout.md), com arranjos e múltiplas instâncias. Absorve os painéis lado a lado do item 21 (#210). | G | #214 |

Faixa A: usar no dia a dia. Faixa B: aparência profissional. Faixa C: opções e desempenho. Faixa D: crescimento.
Esforço: P = até 2 dias, M = até 1 semana, G = mais de 1 semana.

## Etapas

1. **Confiar no app** — itens 1, 2, 6, 12.
   Pronta quando: um pânico provocado gera arquivo de log e diálogo; o app parado fica perto de 0% de CPU;
   fechar e abrir volta à mesma aba; "56 módulos" e os outros detalhes de desenvolvedor sumiram.
2. **Primeiro contato** — itens 3, 4, 5.
   Pronta quando: o roteiro manual do terminal passa sem defeito de desenho; nenhuma aba fica escondida
   numa janela de 1280×720; sem projeto, o app abre na página de início.
3. **Cara nova** — itens 7, 8, 10, 11.
   Pronta quando: as 9 páginas do trilho usam o molde de página; não há ícone repetido; cada tela tem uma
   única ação laranja; Ajustes abre como página com navegação lateral.
4. **Consistência e acabamento** — itens 9, 13, 14, 19.
   Pronta quando: o `ui-lint` roda no CI e os quatro contadores caíram pela metade; as animações existem e
   respeitam `reduce_motion`; o atlas captura o modo demonstração; o README mostra capturas novas.
5. **Opções e hardware** — itens 15, 16, 17, 18.
   Pronta quando: as ordens 1 a 5 das opções adiadas estão ligadas; o painel de diagnóstico existe; dá para
   trocar de renderizador em Ajustes; todo botão de ícone tem nome e dica.
6. **Crescer** — itens 23, 21 e 20, nessa ordem.
   O item 23 entrega os painéis lado a lado na segunda fatia. O item 21 trata do gesto de arquivo para o prompt.

## Decisões

- Ícones: conjunto **próprio**, desenhado para a Forja (regra de autoria do `AGENTS.md`), embutido no binário.
- Itens 4.2–4.4 (molde, `ui-kit`, hierarquia) começam por uma atualização do `design/ui.md`.
- Um item por bancada; commits citam `Refs #n`.
- Nenhum teste abre janela visível nem aciona um ajudante real (`FORJA_TEST_HELPERS=none`).
