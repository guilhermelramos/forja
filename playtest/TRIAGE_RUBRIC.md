# Régua de triagem automática de issues

Toda issue **aberta** (com os rótulos `playtest`, `explore`, `ideia` ou sem rótulo) recebe exatamente **um rótulo de prioridade** (`prioridade-1` a `prioridade-5`) e **um rótulo de área** (`area-*`), além de `duplicata?` quando parecer repetir outra issue aberta.

O triador **nunca** fecha issues, **nunca** mexe em issues fechadas e **nunca** adiciona ou remove o rótulo `em-andamento` (reservado ao coordenador).

## Prioridades (`prioridade-1` a `prioridade-5`)

| Rótulo | Nível | Critério | Exemplos reais |
|---|---|---|---|
| `prioridade-5` | Urgente | Perda de dados, app trava (`hang`) ou fecha (`crash`), ou bloqueia o fluxo principal (adicionar projeto, criar/abrir bancada, terminal, ajudante, salvar/abrir no editor). | `#70`, `#71` (`Tipo: crash`), `#74` (app travou >10 s), `#52` (`Ctrl+Shift+K` apaga linha e `Ctrl+Z` não desfaz), `#19` (projeto nunca fica ativo), `#11` (não é possível abrir bancada), `#18` (abrir arquivo não abre o editor). |
| `prioridade-4` | Alta | Fluxo importante quebrado mesmo com contorno, ou problema evidente que todo usuário encontra logo no início. | `#6` (Ajustes não rola e esconde metade das configurações), `#28` / `#37` / `#57` (chat do ajudante preso em CLI ausente sem troca), `#30` (Navegador abre página em branco), `#86` (`Escape` não fecha diálogo modal), `#58` (OpenCode falha com `os error 2` e mostra `Ocioso`). |
| `prioridade-3` | Média | Bug ou problema de usabilidade perceptível que possui caminho alternativo. | `#32` (abrir PR sem título/descrição falha sem aviso; contorno: preencher antes), `#36` (histórico da bancada desatualizado após commit), `#68` (Bigorna abre sem foco até clicar no campo), `#66` (diálogo `Nova rotina` cortado em janela menor). |
| `prioridade-2` | Baixa | Detalhe visual, texto/i18n, caso raro de estresse ou acessibilidade menor. | `#98` / `#99` (elemento fora da janela em caso de estresse), `#100` / `#105` (abas/botões sobrepostos na mesma faixa), `#75` / `#77` (id interno entre crases ou erro cru na tela), achados do `explore` de `elemento sem nome acessível`. |
| `prioridade-1` | Futuro | Ideia ou melhoria de produto para pensar adiante, sem dor bloqueante atual. | `#69` (cards de custo e quebra por ajudante na tela Uso), `#67` (Mutirão em 3 colunas), `#45` (filtro por linguagem nas Receitas do terminal). |

## Áreas (`area-*`)

Escolha **exatamente uma** área entre as 17 abaixo:

- `area-teclado` — Atalhos globais, foco de teclado, navegação por `Tab`/`Shift+Tab`, `Escape` em diálogos/modais (`#24`, `#25`, `#51`, `#54`, `#81`, `#86`).
- `area-bancadas` — Projetos, criação/abertura/guarda de bancadas e Quadro de etapas (`#11`, `#17`, `#19`, `#63`).
- `area-git` — Git, diferenças, revisão/PR, painel Mudanças, fetch/pull/push (`#32`, `#36`, `#42`, `#62`).
- `area-editor` — Editor de texto, abertura de arquivos no editor, desfazer e salvar (`#18`, `#52`).
- `area-terminal` — Terminal, abas de terminal, shell e Receitas (`#29`, `#43`, `#45`, `#70`, `#91`).
- `area-ajudantes` — Perfis de ajudantes, detecção de CLI instalado, iniciar/focar/parar ajudante e skills (`#37`, `#44`, `#58`, `#65`).
- `area-chat` — Painel/aba de chat do ajudante, envio de mensagens e seleção de ajudante no chat (`#28`, `#40`, `#57`, `#61`).
- `area-mutirao` — Coordenação multiajudante: plano, tarefas, aprovação, início e recados (`#46`, `#59`, `#67`).
- `area-ajustes` — Janela e seções de Ajustes, busca interna, dropdowns, rolagem e idioma (`#6`, `#8`, `#23`, `#79`).
- `area-bigorna` — Paleta de comandos Bigorna (`Ctrl+K`), busca e execução de comandos (`#53`, `#56`, `#68`).
- `area-layout` — Geometria de janelas e painéis, divisores, elementos fora da janela e sobreposição de abas/botões (`#97`, `#98`, `#99`, `#100`, `#105`).
- `area-textos` — Traduções faltantes, i18n, pluralização e erros técnicos crus exibidos ao usuário (`#75`, `#77`, `#78`, `#80`, `#82`, `#84`, `#87`).
- `area-navegador` — Aba Navegador, barra de endereço e renderização de páginas (`#30`, `#73`).
- `area-rotinas` — Rotinas agendadas, diálogo `Nova rotina`, cron e histórico (`#66`).
- `area-uso` — Tela Uso, contagem de tokens, custos e filtros por período (`#69`).
- `area-explorador` — Árvore de arquivos, Busca, Achar arquivo (`Ctrl+P`), Portas, Acervo, Extensões e Clonar repositório (`#60`, `#71`, `#72`).
- `area-outros` — Assuntos transversais ou que não se encaixam claramente nas áreas acima (`#1`).

## Rótulos especiais

- `duplicata?` — Sugerido apenas quando a issue aberta repete claramente o mesmo problema ou pedido de outra issue aberta (ex.: `#61` / `#57` / `#37` / `#28` sobre `Claude Code` ausente como padrão; `#66` e diálogos cortados de rotina). O triador só marca `duplicata?` e menciona o número na justificativa; quem decide fechar é uma pessoa ou o fluxo de verificação.
- `em-andamento` — Usado pelo coordenador para indicar que alguém já está trabalhando na issue. O triador **nunca** adiciona nem remove esse rótulo.

## Atalhos determinísticos (antes de chamar o modelo)

Antes de consultar a IA, aplicam-se estas travas sobre a prioridade:

1. Achado com `kind: crash` (ex.: `**Tipo:** crash` no corpo) ou título contendo `"travou"` → **prioridade mínima 5** (portanto `5`).
2. Issue com rótulo `ideia` → **prioridade máxima 3**, a menos que o texto indique perda de dados (ex.: `perda de dados`, `perde dados`, `apaga ... sem desfazer`, `corrompe`).
3. Achado do `explore` sobre `"elemento sem nome acessível"` (ou `"elemento clicável sem nome acessível"`) → **prioridade 2**.

## Formato de saída esperado do modelo

O modelo deve responder **apenas** com um objeto JSON válido:

```json
{
  "priority": 4,
  "area": "area-ajustes",
  "rationale": "Ajustes não rola e impede acessar metade das configurações.",
  "duplicate_of": null
}
```

- `priority`: inteiro de `1` a `5`.
- `area`: uma das 17 chaves `area-*` listadas acima.
- `rationale`: 1 frase curta em português explicando a nota e a área.
- `duplicate_of`: número inteiro de outra issue aberta com o mesmo problema, ou `null`.

O comentário aplicado na issue tem sempre **uma única linha**:

`Triagem: prioridade N (área X) — motivo.`
