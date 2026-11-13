# forja-module-board

Módulo `board`: o Quadro por etapas e a lista de bancadas da lateral.

## O que registra

- Comando `board.refresh`.
- View `board.page` no slot `rail` (ordem 20): o Quadro como página do trilho.
  O shell desta onda ainda desenha o trilho estreito; nesse caso a view mostra
  um item compacto `▦` com a contagem de bancadas não lidas.
- View `board.tab` no slot de aba de bancada (ordem 20), provisória enquanto o
  shell não renderiza páginas do trilho.
- View `board.sidebar` no slot `sidebar` (ordem 20): filtro, agrupamento,
  ordenação, fixar e lido/não lido.
- Configurações `board.cards.show_branch`, `board.cards.show_helper`,
  `board.cards.show_request`, `board.cards.compact` e
  `board.refresh.interval_s`.
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos consumidos

O módulo não depende do crate de bancadas. Ele usa os comandos públicos:

- `bench.list_cards` — cartões com etapa, arquivada, fixada, não lida e dormindo;
- `bench.stage.list` — colunas, rótulos, ordem, pai e token do ponto;
- `bench.stage.set` — arrastar um cartão;
- `bench.activate`, `bench.create_dialog`, `bench.store_dialog`,
  `bench.pin`, `bench.read`, `bench.sleep`, `bench.wake` e
  `bench.approve_dialog`;
- `project.list` — nome do projeto em cada cartão.

Se o módulo de bancadas não estiver presente, o Quadro mostra uma faixa de
indisponibilidade e não quebra o resto do app.

## Desenho

- Colunas seguem `order`, inclusive as vazias. `forging` usa `accent_soft` no
  cabeçalho; as demais usam `surface_alt`.
- O cartão tem uma barra de 3 px na cor do ponto da etapa, o nome, a etapa, a
  branch, o projeto e a base. Sinalizações: fixada (`★`), não lida (ponto),
  dormindo (`z`) e guardada (quadrado de contorno).
- O limite de WIP (`bench.stage.wip_limit`) é aviso: o cabeçalho mostra a
  contagem em `spark` e nada bloqueia o trabalho.
- Arrastar para frente aplica a etapa; arrastar para trás abre uma faixa de
  confirmação antes de chamar `bench.stage.set` com `confirm_backward`.
- Preferências da lateral ficam em `<dados>/modules/board.json`.

## Evidência

O shell atual empilha as views do painel direito. Enquanto não houver páginas
do trilho, os screenshots usam a aba provisória. O Quadro também é a primeira
aba de bancada (ordem 20).

```sh
cargo test -p forja-module-board
cargo run -p forja-app -- --screenshot shots/quadro.png
cargo run -p forja-app -- --screenshot shots/lateral.png --slot sidebar
```

## Lote 3 (cobertura do catálogo)

- `board.cards.show_helper`: o refresh lê `helper.status` e mostra o estado do último
  ajudante de cada pasta no cartão (chaves `board.card.helper.*`).
- `board.cards.show_request`: mostra a chave do pedido de origem (`BenchCard.request`).
