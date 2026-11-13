# Forja: Interface (Brasa)

Fonte de verdade visual do app. As cores de marca vêm de `design/brand.md`. Este arquivo fixa ritmo, estados, componentes e o contraste medido. O crate `forja-ui-kit` implementa o que está aqui.

O contraste usa a luminância relativa do WCAG 2. Texto comum precisa de **4.5:1**. Componente gráfico e anel de foco precisam de **3:1**. Controle desabilitado fica de fora, com opacidade 0.4.

## 1. Tokens

### 1.1 Cor

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `bg` | `#121010` | `#FAF8F5` | fundo da janela |
| `surface` | `#1B1816` | `#FFFFFF` | painéis, cartões, campos |
| `surface_alt` | `#231F1C` | `#F2EEE9` | listas, trilho de segmento, abas em repouso |
| `border` | `#2E2925` | `#E3DDD6` | divisores e contorno |
| `text` | `#EDE7E1` | `#1F1A17` | texto |
| `text_muted` | `#A69C93` | `#6B625B` | texto secundário |
| `accent` | `#F2762E` | `#D9601B` | ação principal e anel de foco |
| `accent_soft` | `#3A2416` | `#FCE6D7` | linha selecionada |
| `spark` | `#FFC14D` | `#B7860B` | atenção |
| `ok` | `#6CC48A` | `#2E8A4E` | sucesso |
| `danger` | `#E5534B` | `#C23A32` | erro e destrutivo |
| `info` | `#6FA8DC` | `#2F6FA8` | links e informação |

Texto sobre preenchimento, escolhido para passar AA:

| Token | Escuro | Claro |
|---|---|---|
| `on_accent` | `#1F1A17` | `#1F1A17` |
| `on_danger` | `#1F1A17` | `#FFFFFF` |

Um tema de extensão é um TOML. O que faltar vem do preset do `mode` (`dark` ou `light`).

```toml
name = "Brasa da bancada"
mode = "dark"
density = "compact"
reduce_motion = false

[color]
accent = "#E06A20"
```

### 1.2 Espaço

A grade é de **4 px**.

| Passo | px |
|---|---|
| 1 | 4 |
| 2 | 8 |
| 3 | 12 |
| 4 | 16 |
| 5 | 20 |
| 6 | 24 |
| 8 | 32 |
| 10 | 40 |
| 12 | 48 |

### 1.3 Raio

| Superfície | px |
|---|---|
| Controle, campo, badge, menu interno | 6 |
| Cartão, modal, painel de menu | 10 |
| Trilho do toggle e barra de progresso | metade da altura |

### 1.4 Tipografia

Segoe UI Variable, com Segoe UI por baixo. Código e terminal usam Cascadia Code. As duas são lidas de `C:\Windows\Fonts` na hora, sem redistribuir. Título usa Segoe UI Semibold (peso 600).

| Papel | Confortável | Compacta | Peso |
|---|---|---|---|
| Legenda, badge | 12 | 11 | 400 |
| Corpo, botão, campo | 14 | 13 | 400 |
| Código | 13 | 12 | 400 |
| Título | 18 | 16 | 600 |

### 1.5 Sombra e elevação

A sombra não substitui o contorno. O nível sobe com a camada.

| Nível | Onde | Escuro (y, blur, alfa) | Claro (y, blur, alfa) |
|---|---|---|---|
| 0 | campos, botões, linhas | sem sombra | sem sombra |
| 1 | cartão | 2, 8, 50 | 1, 6, 28 |
| 2 | menu, toast | 4, 16, 80 | 4, 16, 40 |
| 3 | modal | 8, 24, 120 | 8, 28, 64 |

O véu do modal é preto a 160 de alfa no escuro e 90 no claro.

### 1.6 Estados

| Estado | O que muda |
|---|---|
| Repouso | preenchimento do token |
| Hover | 8% de branco no escuro, 8% de preto no claro, por cima do preenchimento |
| Ativo (pressão) | 14% do mesmo véu |
| Foco | anel de 2 px em `accent`, 2 px fora do controle |
| Desabilitado | opacidade 0.4, sem clique |

O hover de uma linha de menu troca o fundo para `surface_alt`. O texto da linha continua `text`. Fantasma não tem preenchimento até o hover, que usa `surface_alt`.

### 1.7 Movimento

Transições de 120 ms. Com `reduce_motion`, a duração vai a 0.

## 2. Densidade

Confortável é o padrão. Compacta é opção.

| | Confortável | Compacta |
|---|---|---|
| Controle (botão, campo, aba, segmentado) | 32 | 24 |
| Linha de lista e tabela | 32 | 24 |
| Badge | 20 | 16 |
| Progresso | 8 | 4 |
| Toggle | 40×24, botão raio 8 | 32×20, botão raio 6 |
| Folga interna | 12×8 | 8×4 |
| Espaço entre itens | 8 | 4 |
| Recuo da árvore | 16 | 12 |

## 3. Componentes

Os rótulos vêm de quem chama. O ui-kit não escreve texto de produto.

### Botões

| Tipo | Preenchimento | Texto | Contorno |
|---|---|---|---|
| Primário | `accent` | `on_accent` | não |
| Secundário | `surface` | `text` | `border` |
| Fantasma | transparente | `text` | não |
| Perigo | `danger` | `on_danger` | não |

Uma ação principal por grupo. Perigo é o que apaga ou descarta.

### Campo de texto

Uma linha. Fundo `surface`, contorno `border`, raio 6. A dica usa `text_muted`. No foco, o anel `accent` substitui a confiança no contorno.

### Seletor

Mesma caixa do campo, com o valor atual e um chevron. O clique abre a lista. A opção ativa usa `accent_soft` e texto `text`.

### Toggle

Trilho de pílula mais rótulo. Ligado: trilho `accent`, botão `on_accent`. Desligado: trilho `surface_alt` com contorno, botão `text`. A posição do botão é o estado; a cor confirma. Enter ou Espaço com foco também invertem.

### Segmentado

Um trilho `surface_alt` com segmentos iguais. O ativo preenche `accent` com texto `on_accent`. Os outros usam `text`.

### Abas

Texto `text_muted` em repouso, `text` na ativa e no hover. A ativa ganha um sublinhado de 2 px em `accent`. No tema claro o laranja não serve de texto pequeno (3.51:1 sobre `bg`), por isso a aba ativa não pinta o rótulo de `accent`.

### Cartão

Fundo `surface`, contorno `border`, raio 10, elevação 1, folga de 16 px no confortável (8 no compacto, duas vezes o espaço entre itens). Título opcional em peso 600.

### Lista

Linha na altura da densidade. Hover pinta `surface_alt`. Seleção pinta `accent_soft` com texto `text`. Detalhe à direita em `text_muted`.

### Menu de contexto

Painel `surface`, raio 10, elevação 2. Cada linha usa texto `text`. Hover usa `surface_alt`. Entrada de perigo ganha uma marca de 3×12 px em `danger`; o rótulo não fica vermelho, porque `danger` sobre `surface_alt` no escuro fica em 4.42:1.

### Modal

Véu, cartão centrado, raio 10, elevação 3, título em peso 600. Fecha no clique do véu ou no Escape. A galeria também mostra o cartão sem véu, para o componente aparecer junto dos outros.

### Toast

Faixa `surface` com elevação 2. O texto é `text`. Uma barra de 4 px à esquerda carrega `info`, `ok`, `spark` ou `danger`.

### Badge de etapa

Fundo `surface_alt`, texto `text`, marca de 4×8 px:

| Etapa | Marca |
|---|---|
| Rascunho | `text_muted` |
| Forjando | `accent` |
| Revisão | `spark` |
| Pronto | `ok` |

### Barra de progresso

Trilho `surface_alt`, preenchimento `accent`, valor cortado para 0–1. A altura é 8 ou 4. O raio é a metade da altura.

### Árvore

Recuo por nível. Nó com filhos tem um triângulo: para a direita fechado, para baixo aberto. O rótulo é `text`. O triângulo é `text_muted`.

### Tabela virtualizada

Cabeçalho fixo em `surface_alt` com título `text_muted`. O corpo só pinta as linhas dentro da altura pedida. Linhas ímpares usam `bg` para zebrar sem baixar o contraste do texto.

## 4. Acessibilidade

O foco é sempre um anel de 2 px em `accent`, fora do controle, nas três superfícies. Não depende só da cor do preenchimento: aba ativa tem sublinhado, toggle tem a posição do botão, perigo no menu tem marca, etapa tem marca e o nome por extenso.

### Contraste medido

| Par | Escuro | Claro | Piso |
|---|---|---|---|
| `text` / `bg` | 15.46 | 16.26 | 4.5 |
| `text` / `surface` | 14.40 | 17.24 | 4.5 |
| `text` / `surface_alt` | 13.33 | 14.92 | 4.5 |
| `text_muted` / `bg` | 7.04 | 5.62 | 4.5 |
| `text_muted` / `surface` | 6.56 | 5.96 | 4.5 |
| `text_muted` / `surface_alt` | 6.07 | 5.16 | 4.5 |
| `on_accent` / `accent` | 6.08 | 4.63 | 4.5 |
| `on_danger` / `danger` | 4.65 | 5.32 | 4.5 |
| `accent` / `bg` (foco) | 6.69 | 3.51 | 3.0 |
| `accent` / `surface` | 6.23 | 3.72 | 3.0 |
| `accent` / `surface_alt` | 5.77 | 3.22 | 3.0 |
| `info` / `bg` | 7.50 | 5.00 | 4.5 |
| `danger` / `bg` | 5.12 | 5.02 | 4.5 |
| `danger` / `surface` | 4.77 | 5.32 | 4.5 |
| `ok` / `surface` | 8.33 | 4.32 | 3.0 no claro, 4.5 no escuro |
| `spark` / `surface` | 10.92 | 3.27 | 3.0 no claro, 4.5 no escuro |

Regras que saem da medição:

- No claro, `accent` (3.51 sobre `bg`) é anel, preenchimento e marca. Não é texto de corpo.
- No claro, `spark` (3.08 sobre `bg`, 3.27 sobre `surface`) e `ok` (4.07 sobre `bg`, 4.32 sobre `surface`) são marca ou ícone. A frase usa `text`.
- `danger` como texto pequeno não vai sobre `surface_alt` no escuro (4.42:1). Sobre `bg` e `surface` passa. No menu, a marca carrega o perigo e o rótulo fica em `text`.

O teste `contrast` do crate trava esses pisos.

## 5. Temas de terminal

Cinco paletas em `content/themes/`. Cada uma tem fundo, frente, cursor, seleção, texto da seleção e as 16 cores ANSI. Foram criadas para a Forja. A frente e cada ANSI passam de 4.5:1 sobre o fundo, para continuarem legíveis quando viram texto. O cursor passa de 3:1. O texto da seleção passa de 4.5:1 sobre o fundo da seleção.

No tema claro, o índice "branco" também é uma tinta escura o bastante para ler. O brilho deixa a tinta mais clara, sem cair abaixo de 4.5:1.

| Tema | Fundo | Frente | Frente:1 | Cursor:1 | Seleção (texto):1 | Menor ANSI |
|---|---|---|---|---|---|---|
| Brasa Escuro | `#121010` | `#EDE7E1` | 15.46 | 11.73 | 6.74 | 4.60 (`black`) |
| Brasa Claro | `#FAF8F5` | `#1F1A17` | 16.26 | 6.55 | 9.32 | 4.92 (`bright_white`) |
| Carvão | `#101416` | `#E4E8EA` | 15.02 | 11.04 | 8.51 | 5.65 (`black`) |
| Cinza Quente | `#2C2825` | `#F7F1EB` | 13.04 | 8.30 | 7.48 | 4.75 (`black`) |
| Aço | `#152028` | `#E7F0F4` | 14.32 | 10.02 | 8.02 | 5.47 (`black`) |

Brasa Escuro e Brasa Claro reaproveitam os tons da interface quando eles já passam o piso (`text`, `danger`, `ok`, `spark`, `info` no escuro). Os outros índices, e os temas Carvão, Cinza Quente e Aço, são tintas próprias: carvão frio, pedra aquecida e aço azulado.

Enquanto as tintas próprias não forem desenhadas, `ui-kit` deriva essas três paletas de fundo e texto (`Tokens::carvao`, `Tokens::cinza_quente`, `Tokens::aco`): superfícies sobem do fundo 4%, 8% e 16% rumo ao texto; o texto secundário desce 35% rumo ao fundo; o acento e os estados vêm do Brasa Escuro. O teste `contrast` trava os pisos de AA dessas paletas.

## 6. Ícone

Marca: bigorna de lado, geométrica, com uma centelha em losango acima. O ícone do app põe a bigorna em branco-quente (`#FFF6EE`) sobre um quadrado de raio 7 (numa grade de 32) com gradiente `#F2762E` → `#8C2E12`. A centelha é `#FFC14D`.

`design/assets/make-icon.py` grava `logo.svg`, `logo-16.png`, `logo-32.png`, `logo-48.png`, `logo-256.png` e `logo.ico`.
