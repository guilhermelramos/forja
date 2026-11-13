# Forja: etapas das bancadas

> **Fonte de verdade do fluxo de trabalho.** Este arquivo define as etapas padrão, as transições permitidas, as cores e a sincronização com pedidos. Os ids são nossos, em inglês; os rótulos são pt-BR e en.

## 1. Etapa não é estado de execução

Uma **etapa** responde "em que ponto do fluxo este trabalho está". Ela não responde "o que o ajudante está fazendo agora", "o PR passou nos checks" ou "a bancada está guardada". Misturar essas coisas faz o Quadro mentir.

| Coisa | Onde vive | Exemplos |
|---|---|---|
| Etapa da bancada | `bench.stage` | `draft`, `forging`, `review`, `ready` |
| Estado do ajudante | `helper.status` | `starting`, `working`, `waiting`, `idle`, `done`, `failed` |
| Estado do PR | `pull_request.state` | `draft`, `open`, `merged`, `closed` |
| Checks | `pull_request.checks` | `pending`, `passing`, `failing` |
| Arquivo | `bench.archived` | `false`, `true` |
| Sincronização | `bench.stage_sync` | `synced`, `pending`, `conflict` |

Um ajudante pode estar `waiting` em `forging`; um PR pode estar `open` com checks `failing` em `review`; uma bancada `ready` pode estar arquivada ou não. Nenhuma dessas combinações muda a etapa sozinha.

## 2. As quatro etapas padrão

| Ordem | id | pt-BR | en | O que significa | Como entra | Como sai |
|---:|---|---|---|---|---|---|
| 10 | `draft` | Rascunho | Draft | A bancada existe, mas ninguém começou a forjar. | Criação de bancada. | Primeiro ajudante inicia, ou você move para `forging`; ou a bancada é guardada. |
| 20 | `forging` | Forjando | Forging | Há trabalho ativo: ajudante trabalhando, edição manual ou commits locais. | Início do primeiro ajudante, ou movimento seu. | Revisão solicitada, PR aberto ou bancada guardada. |
| 30 | `review` | Revisão | Review | O resultado está sob revisão: PR aberto, diff em leitura ou checks rodando. | PR aberto ou movimento seu. | Aprovação/merge vai para `ready`; pedido de mudança volta para `forging`; PR fechado sem merge volta para `forging`. |
| 40 | `ready` | Pronto | Ready | O trabalho foi entregue: PR mesclado ou resultado aceito por você. | Merge do PR, aceite explícito ou movimento seu. | Reabertura vai para `review` (se há PR aberto) ou `forging`; guardar é uma ação separada. |

**Guardar não é etapa.** `ready` é entrega; `archived` é organização. Uma bancada `ready` continua na lateral e no Quadro até ser guardada. Uma bancada `forging` pode ser guardada sem virar `ready` — nesse caso ela volta com a mesma etapa.

## 3. Cores e acessibilidade

As cores vêm dos tokens **Brasa** de `design/brand.md`. O token pinta o ponto e a borda esquerda do cartão; o texto do selo usa sempre `text`, nunca a cor da etapa. Assim o contraste do rótulo não depende da etapa e a cor é redundante, não a única informação.

| Etapa | Token do ponto/borda | Escuro | Claro | Fundo do selo | Uso |
|---|---|---:|---:|---|---|
| `draft` | `text_muted` | #A69C93 | #6B625B | `bg` | Neutro: ainda não começou. |
| `forging` | `accent` | #F2762E | #D9601B | `bg` | Brasa: trabalho ativo. |
| `review` | `spark` | #FFC14D | #B7860B | `bg` | Centelha: precisa de atenção. |
| `ready` | `ok` | #6CC48A | #2E8A4E | `bg` | Sucesso: entregue. |
| guardada | `text_muted` (contorno) | #A69C93 | #6B625B | `surface_alt` | Não é etapa; usa ícone de caixa e o mesmo rótulo da etapa original. |

Contraste medido (WCAG 2.1):

| Par | Escuro | Claro | Mínimo | Resultado |
|---|---:|---:|---:|---|
| Texto do selo (`text`) sobre `bg` | 15,46:1 | 16,26:1 | 4,5:1 | AA |
| Texto secundário (`text_muted`) sobre `bg` | 7,04:1 | 5,62:1 | 4,5:1 | AA |
| Ponto `accent` sobre `bg` | 6,69:1 | 3,51:1 | 3:1 | AA não textual |
| Ponto `spark` sobre `bg` | 11,73:1 | 3,08:1 | 3:1 | AA não textual |
| Ponto `ok` sobre `bg` | 8,94:1 | 4,32:1 | 3:1 | AA não textual |
| Ponto `text_muted` sobre `bg` | 7,04:1 | 5,62:1 | 3:1 | AA não textual |

O ponto `spark` claro passa por margem curta (3,08:1). Por isso o selo usa `bg` e não `surface_alt`: sobre `#F2EEE9` o mesmo ponto cairia para 2,83:1 e falharia. O tema claro é verificado no teste de contraste do `ui-kit`; nenhum tema pode trocar o fundo do selo sem refazer essa conta.

Regras visuais:

- O selo tem **texto sempre presente** (`Rascunho`, `Forjando`, `Revisão`, `Pronto`), um ponto de 8 px e uma borda esquerda de 3 px no cartão. Cor nunca é o único canal.
- A coluna de `forging` no Quadro pode usar `accent_soft` no cabeçalho; as demais usam `surface_alt`. O selo do cartão continua em `bg`.
- A ordem das colunas é a ordem das etapas; a cor não altera a ordem.
- Em tema de alto contraste do Windows, o selo ganha contorno `text` de 1 px e o ponto usa a cor de sistema; a etiqueta não desaparece.
- Nada de gradiente, brilho ou animação por etapa. A única animação é a transição de 120 ms do cartão ao mudar de coluna, e ela respeita `appearance.motion.reduce`.

## 4. Mapa de transições

```text
                  primeiro ajudante
        ┌───────────────────────────────┐
        │                               ▼
    ┌────────┐                      ┌─────────┐
    │ draft  │─────────────────────▶│ forging │
    └────────┘   movimento seu      └─────────┘
        │                               │  ▲
        │ guardar                       │  │ pedido de mudança
        ▼                               ▼  │ PR fechado sem merge
    (arquivada)                    ┌──────────┐
                                   │  review  │
                                   └──────────┘
                                        │
                                        │ merge ou aceite
                                        ▼
                                   ┌──────────┐
                                   │  ready   │
                                   └──────────┘
                                        │
                                        │ reabrir
                                        └────▶ review (PR aberto) ou forging
```

| De | Para | Quem | Condição | Confirmação |
|---|---|---|---|---|
| `draft` | `forging` | Você, ajudante ao iniciar, rotina explícita, mutirão ao despachar | Nenhuma. | Não |
| `draft` | `ready` | — | **Proibido.** Entregar sem forjar não é um fluxo; guarde a bancada. | — |
| `forging` | `draft` | Você | Só se a Forja nunca criou commit nessa bancada e nenhum ajudante chegou a trabalhar. | Sim |
| `forging` | `review` | Você, PR aberto, sugestão de ajudante aceita, rotina explícita | PR aberto quando a origem é automática. | Não |
| `forging` | `ready` | — | **Proibido** fora de merge de PR ou aceite explícito com "entrega sem revisão". | — |
| `review` | `forging` | Você, PR com mudanças pedidas, PR fechado sem merge | Nenhuma. | Sim quando é movimento manual para trás |
| `review` | `ready` | Você, merge do PR | Merge ou aceite explícito. | Não no merge; sim no aceite manual |
| `ready` | `review` | Você, PR reaberto | Existe PR aberto. | Sim |
| `ready` | `forging` | Você | Reabertura para trabalho novo. | Sim |
| `ready` | `draft` | — | **Proibido.** Crie outra bancada. | — |
| `review` | `draft` | — | **Proibido.** | — |
| Qualquer | arquivada | Você, rotina de limpeza | A etapa não muda. | Depende de `bench.close.confirm_dirty` |

Regras gerais:

- **Para frente, você pode pular uma subetapa; não pode pular `review` para chegar a `ready`.** `draft → forging → review → ready` é a espinha; etapas extras entram entre elas.
- **Para trás, sempre pergunta.** Exceto quando o próprio PR ou o balcão pediu a volta; aí a mudança é registrada e notificada, mas não bloqueia.
- **Transição inválida é recusada com o motivo**, não apenas desabilitada sem explicação. O Quadro mostra "Pronto exige revisão ou merge" ao arrastar.
- **Toda transição é um evento** `stage.changed` com `from`, `to`, `actor`, `origin`, `reason` e `at`. O histórico da bancada mostra a linha, mesmo quando a mudança veio de um PR.
- **Mudança de etapa não apaga histórico de etapa anterior.** O Quadro mostra a atual; a linha do tempo guarda todas.

## 5. Regras automáticas

Automação só existe onde o sinal é objetivo. "O ajudante disse que terminou" não é sinal de entrega; "o PR foi mesclado" é.

| Evento | Efeito na etapa | Configuração | Observação |
|---|---|---|---|
| `bench.created` | `draft` | `stages.creation.default` | O padrão é rascunho. |
| `helper.started` | `draft → forging` | — | Só quando a etapa ainda é `draft`. |
| `pr.opened` | `forging → review` | `stages.pr.open` = `always`, `ask`, `never` | `ask` mostra um aviso no cartão e na notificação; `never` só registra a sugestão. |
| `pr.merged` | qualquer → `ready` | `stages.pr.merge` = `always`, `ask`, `never` | `always` é o padrão; merge é entrega objetiva. |
| `pr.closed` sem merge | `review → forging` | — | Sempre, com nota "PR fechado sem merge". |
| `pr.reopened` | `ready → review` | — | Só se havia PR aberto e a etapa era `ready`. |
| `checks.failed` | nenhuma | — | Check é estado do PR; o cartão mostra o ícone, a etapa fica em `review`. |
| `helper.handoff` = `feito` | nenhuma; cria sugestão | — | A sugestão aparece no cartão e na linha do tempo; você aceita para ir a `review`. |
| `helper.handoff` = `bloqueado` ou `falhou` | nenhuma | — | O ajudante e a notificação contam a história; a etapa não muda. |
| `helper.waiting` | nenhuma | — | "Precisa de você" é estado do ajudante. |
| `routine.finished` | nenhuma | — | Uma rotina pode chamar o comando de etapa explicitamente; aí a origem é `routine`. |
| `bench.archived` | nenhuma | — | Guardar não é transição. |
| `bench.restored` | nenhuma | — | Volta com a etapa que tinha. |
| `request.status.changed` | conforme mapa da seção 7 | `stages.sync.request` e regras do balcão | Mudança externa é sugestão quando há ajudante ativo. |

O que **não** é automático por decisão:

- Ajudante terminar não move para `review`. Entrega de agente é uma alegação; revisão é um ato.
- Checks passarem não move para `ready`. Check verde não é merge.
- Rotina terminar não move etapa. Rotina é execução, não aprovação.
- Comentário no chat não move etapa. Texto de conversa não é comando.
- Fechar a janela não move etapa. Estado de UI não é estado de trabalho.

## 6. Quem pode mudar

| Ator | Pode mudar? | Como | Auditoria |
|---|---|---|---|
| Você | Sim, em qualquer transição legal | Arrastar no Quadro, menu da bancada, CLI `forja bancada etapa` ou RPC `bench.stage.set` | `actor = user` |
| Ajudante | Só sugere, por padrão | Evento `stage.suggest` com motivo; a sugestão aparece para você | `actor = helper:<id>` |
| Ajudante com permissão `auto` ou `total` | Sim, via comando explícito | `forja bancada etapa --para revisao --motivo "..."` | `actor = helper:<id>`, com o comando no histórico |
| Coordenador de mutirão | Sim, para bancadas do plano, quando o plano declara | RPC do mutirão com `stage` na tarefa | `actor = muster:<id>` |
| Rotina | Só quando o comando da rotina chama a etapa explicitamente | CLI/RPC dentro da ação | `actor = routine:<id>` |
| Extensão | Só por comando aprovado, nunca em evento passivo | RPC com capacidade `bench.stage` aprovada | `actor = extension:<id>` |
| Evento de PR | Sim, pelas regras da seção 5 | Webhook local/consulta do PR | `actor = git`, `origin = pr` |
| Balcão | Sim, pelas regras da seção 7 | Consulta do pedido | `actor = counter`, `origin = request` |

O ajudante nunca muda etapa por "ter entendido" uma instrução no chat. Ou ele chama o comando, ou sugere. A diferença é a mesma entre pedir e fazer.

## 7. Sincronização com pedidos

`stages.sync.request` liga o espelhamento. Ele é independente de `request.sync.stage_on_pr` e `request.sync.close_on_merge`: os dois primeiros falam da bancada; os dois últimos falam do pedido.

### 7.1 Mapa de status

| Etapa (ou `parent`) | Pedido local | GitHub Projects | Linear | Jira |
|---|---|---|---|---|
| `draft` | `aberto` | `Todo` ou `Backlog` | `Todo` ou `Backlog` | `To Do` |
| `forging` | `em andamento` | `In Progress` | `In Progress` | `In Progress` |
| `review` | `em revisão` | `In Review`, se existir; senão `In Progress` | `In Review`, se existir; senão `In Progress` | `In Review`, se existir; senão `In Progress` |
| `ready` | `concluído` | `Done` | `Done` | `Done` |

O mapa usa a etapa canônica **pai** de uma etapa extra. Uma etapa "QA" com `parent = "review"` espelha como revisão. O balcão nunca ganha um estado que já não exista: se `In Review` não existe, a Forja usa o estado anterior e registra uma nota, em vez de criar coluna no projeto alheio.

### 7.2 Direção e conflito

1. **Mudança local manda durante o trabalho.** Se a etapa mudou por você, por ajudante ou por PR, a Forja escreve no pedido quando `stages.sync.request` está ligado. O comentário diz a etapa, a origem e o motivo, sem despejar o diff.
2. **Mudança externa é sugestão quando há ajudante ativo.** Se o pedido mudou no balcão e a bancada tem ajudante `working` ou `waiting`, a Forja mostra uma faixa: "O pedido mudou para X no balcão" com **Aplicar** e **Ignorar**. A etapa não muda sozinha no meio de um agente trabalhando.
3. **Sem ajudante ativo, mudança externa aplica.** Se o status externo mapeia para uma etapa e é mais novo que a última escrita local, a etapa acompanha, com nota na linha do tempo.
4. **Pedido fechado vira `ready`; pedido reaberto vira `forging`.** Se o fechamento veio de merge, a regra do PR já leva a `ready`; o balcão só confirma.
5. **Sem laço.** Transições com `origin = request` não escrevem de volta no balcão. Cada bancada guarda `stage_sync.last_local_write` e `stage_sync.last_remote_read`; só o mais novo aplica.
6. **`ready` não fecha o pedido sozinho.** `request.sync.close_on_merge` fecha no merge; fora disso, fechar é ação explícita. Etapa é etapa; issue fechada é issue fechada.
7. **Falha de rede não bloqueia o Quadro.** A bancada fica com `stage_sync = pending`, o cartão mostra um ícone discreto e a Forja tenta de novo com recuo. Nenhuma transição local espera o balcão.
8. **Pedido local não tem rede.** O status local espelha a etapa na hora e nunca gera conflito externo.

### 7.3 O que a sincronização não escreve

- Não copia descrição, título, labels ou assignees da Forja para o pedido sem comando.
- Não comenta o diff, o prompt ou a conversa do ajudante.
- Não cria status/coluna no balcão.
- Não move pedido de outro projeto ou de outra pessoa.
- Não fecha issue por etapa `ready` sem a configuração de merge ou um comando explícito.

## 8. Etapas extras

O usuário pode acrescentar etapas em `~/.forja/etapas.toml` e no `.forja/etapas.toml` do projeto. O projeto vence o usuário. As quatro canônicas sempre existem e não podem ser apagadas; podem ser renomeadas, recoloridas e reordenadas.

```toml
schema_version = 1

[[stage]]
id = "draft"
label_pt = "Rascunho"
label_en = "Draft"
order = 10
parent = "draft"
dot = "text_muted"
locked = true

[[stage]]
id = "forging"
label_pt = "Forjando"
label_en = "Forging"
order = 20
parent = "forging"
dot = "accent"
locked = true
```

Uma etapa extra declara `parent` (a canônica que dá cor, limite e mapa de status), `order` (posição na coluna) e um `dot` que precisa ser **nome de token Brasa**, nunca hexadecimal. Tokens aceitos: `text_muted`, `accent`, `spark`, `ok`, `info`, `danger`. O tema resolve o hex; trocar o tema não quebra a etapa.

```toml
[[stage]]
id = "qa"
label_pt = "QA"
label_en = "QA"
order = 25
parent = "review"
dot = "info"
locked = false
```

Regras das etapas extras (v1):

- O fluxo é **linear**. Para trabalho paralelo, use etiquetas ou tarefas do mutirão, não colunas.
- `order` é único e positivo; a canônica mantém sua faixa (10, 20, 30, 40). Uma extra entre `forging` e `review` usa, por exemplo, `25`.
- A etapa extra herda o `parent` para cor padrão, WIP e sincronização, mas pode ter `dot` próprio.
- Você pode pular uma extra para frente; não pode pular `review` para chegar a `ready`.
- Apagar uma etapa extra que tem bancadas exige mover essas bancadas para o `parent` antes; a Forja não deixa cartão órfão.
- Id em `snake_case`, único, sem renomear depois de criado. Rótulo muda; id não.
- `locked = true` nas quatro canônicas. Uma extra não pode usar id canônico nem `archived`.
- Nada de transição condicional, script por coluna ou automação por etapa na v1. Se precisar, é uma rotina com comando explícito.

## 9. Quadro e limites

- As colunas seguem `order`. Colunas vazias continuam visíveis para o fluxo ficar legível.
- O filtro padrão esconde bancadas guardadas; "mostrar guardadas" as traz com o selo de caixa.
- Arrastar para uma etapa inválida é recusado com o motivo da seção 4. O cartão volta e uma faixa explica.
- `stages.limit.wip` conta bancadas cujo `parent` é `forging` e avisa quando passa do limite. É aviso, não bloqueio: a coluna fica com um contador em `spark`, e nada impede o trabalho.
- `board.cards.*` controla o que aparece no cartão (branch, ajudante, pedido, modo compacto). O selo de etapa aparece sempre.
- O cabeçalho da coluna mostra nome e contagem. A cor do cabeçalho é a do `parent`; o selo do cartão é o da etapa concreta.

## 10. Eventos, RPC e CLI

| Superfície | Nome | Contrato |
|---|---|---|
| Evento | `stage.changed` | `bench`, `from`, `to`, `actor`, `origin`, `reason`, `at` |
| Evento | `stage.suggested` | `bench`, `to`, `actor`, `reason`, `at` |
| RPC | `bench.stage.set` | `{ bench, stage, reason, confirm_backward }`; recusa transição ilegal com código e motivo |
| RPC | `bench.stage.list` | Lista as etapas disponíveis, com `id`, rótulos, `order`, `parent`, `locked`, `dot` |
| CLI | `forja bancada etapa <id> --para <etapa> --motivo <texto>` | Mesma validação do RPC; `--motivo` é obrigatório para voltar |
| CLI | `forja bancada etapas` | Lista o mapa efetivo, incluindo extras do projeto |

O evento é emitido depois da persistência. Se a gravação falhar, não há evento e a interface mostra o erro; não existe transição "otimista" que depois some.

## 11. Configurações que mexem nas etapas

| Chave | Padrão | Efeito |
|---|---|---|
| `bench.stage.creation_default` | `draft` | Etapa de uma bancada nova. |
| `bench.stage.wip_limit` | `0` | Aviso de excesso em `forging`; 0 desliga. |
| `bench.stage.sync_request` | `true` | Espelha a etapa no pedido. |
| `bench.stage.pr_open` | `always` | Move para `review` ao abrir PR; `ask` ou `never` como alternativa. |
| `bench.stage.pr_merge` | `always` | Move para `ready` ao mesclar PR. |
| `bench.stage.archive_on_ready` | `false` | Guarda automaticamente ao entrar em `ready`. |
| `request.sync.stage_on_pr` | `true` | Move o pedido para revisão ao abrir PR. |
| `request.sync.close_on_merge` | `true` | Fecha o pedido ao mesclar PR. |
| `board.refresh.interval_s` | `15` | Atualização do Quadro; 0 desliga. |

## 12. O que deliberadamente não muda a etapa

- Ajudante `waiting`, `idle`, `done` ou `failed`.
- Chat com mensagem nova, pergunta ou resposta.
- Check de CI passando, falhando ou pendente.
- Commit local, push ou force-push.
- Comentário em PR.
- Tempo parado.
- Abrir ou fechar a janela.
- Trocar de tema, densidade ou idioma.
- Guardar ou restaurar a bancada.
- Um comando de leitura (`forja bancada list`, `forja status`) — só `bench.stage.set` muda etapa.

Se algum desses eventos precisar mudar o fluxo, ele vira uma **rotina** com comando explícito ou uma **sugestão** que você aceita. O padrão é o contrário de mágica: a etapa só anda quando alguém ou um sinal objetivo manda.
