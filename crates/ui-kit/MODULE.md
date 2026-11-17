# forja-ui-kit

Design system Brasa: tokens, tema do egui e os controles da oficina.

## O que registra

Nada no kernel. Este crate não é um módulo. O shell chama `apply` e desenha com as funções daqui.

## Contrato

- `Tokens::brasa_dark` / `Tokens::brasa_light` são a paleta de `design/brand.md`.
- `Tokens::from_toml` lê um tema de extensão. Campo ausente fica com o preset do `mode`.
- `apply(ctx, &tokens)` guarda os tokens e ajusta estilo, tipo e sombras.
- `install_fonts(ctx)` lê somente fontes de texto em `%WINDIR%/Fonts`, conserva
  fallbacks e informa ausências para o log do shell. Não há fonte de ícones.
- `Icon` contém 80 desenhos autorais de `design/icons/`, gerados por `cargo xtask icons`.
  `icon(ui, Icon, size)` e `paint_icon` desenham os traços; `icon_button(ui, Icon, label)`
  exige nome acessível/dica; `button_with_icon(ui, Icon, text, kind)` conserva a hierarquia.
  Linhas, curvas cúbicas e quadráticas usam a mesma espessura e os tokens da Forja.
  `cargo test -p xtask icons::` verifica a identidade entre SVGs e o arquivo comitado.
- `TerminalTheme::load_dir` lê `content/themes/*.toml`.
- `page(ui, PageSpec)` é o molde de cinco zonas das páginas do trilho: cabeçalho
  20/600 com no máximo uma ação principal, atualizar fixo à direita, filtros na ordem
  busca/segmentos/seletores, corpo com largura `Content` (até 960 px) ou `Wide`,
  estados compartilhados por `page_state` e rodapé opcional ancorado.
  As closures de `Page::filters`, `body` e `footer` são sequenciais para compartilhar
  o contexto mutável sem callbacks simultâneos. Os chamadores conservam a rolagem
  interna de quadros e tabelas. `page_marker(ctx, view_id)` expõe o contrato real de
  identidade/geometria para testes; nenhum id interno vira texto de interface.
- Componentes recebem o rótulo. Não há string de produto embutida.
- `setting_row` padroniza rótulo/ajuda, controle à direita, marca de alteração,
  restauração e avisos por linha. `settings_navigation` oferece linhas de seção
  com estado selecionado acessível e indicador Brasa; `highlighted_text` mantém
  limites Unicode ao enfatizar termos de busca.
- `select` abre com clique, Space ou Enter no gatilho focado; as setas movem o
  destaque da lista, Enter escolhe e Escape fecha. As opções abertas aparecem na
  árvore de acessibilidade como botões.
- Janelas flutuantes chamam `register_keyboard_layer(ui, id)` a cada quadro e
  `consume_layer_escape(ctx, id)` antes de desenhar. O Tab fica na camada do
  topo e Escape fecha apenas ela; um menu aberto recebe Escape primeiro. Use um
  `id` estável e diferente para cada janela simultânea.
- `modal` fecha no véu ou no Escape. `modal_explicit_close` usa a mesma camada,
  mas ignora o clique no véu: fecha só por Escape, botão de fechar ou
  `Ui::close` — para fluxos guiados que não podem sumir por engano.
- Densidade `comfortable` (controle 32 px) ou `compact` (24 px). Grade de 4 px.

## Como testar

```
cargo test -p forja-ui-kit
cargo test -p forja-app every_registered_rail_view_uses_the_page_component
cargo clippy -p forja-ui-kit --all-targets -- -D warnings
cargo run -p forja-ui-kit --example gallery -- --theme dark --screenshot design/assets/galeria-escuro.png
cargo run -p forja-ui-kit --example gallery -- --theme light --screenshot design/assets/galeria-claro.png
python design/assets/make-icon.py
```

Os testes de layout usam um `egui::Context` sem janela. O contraste trava os pisos de `design/ui.md`. O teste dos temas lê `content/themes`.

`action_tile(ui, glyph, label, width, primary)` desenha uma ação grande de início, com ícone, nome acessível e foco de teclado; o chamador fornece textos traduzidos e largura responsiva.

`theme_sample(ui, tokens, title, body)` mostra uma amostra de tema sem alterar o contexto global nem criar uma ação fictícia.
