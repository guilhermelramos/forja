# Ícones autorais da Forja

Os 80 SVGs desta pasta foram desenhados do zero para a Forja, segundo o
**Traço de oficina** de `design/ui.md`. Não são cópias, adaptações ou derivações
de coleções externas; não exigem fonte, biblioteca de ícones ou serviço externo.

- Grade 24×24, traço 2, margem ótica 3, pontas/junções arredondadas.
- Evitar detalhes que desapareçam a 16 px; navegação usa 20 px.
- Nome de arquivo em inglês; destinos internos têm desenhos/ids exclusivos.
- Sem preenchimento ou transformações. Elementos: path, circle, rect, line,
  polyline; comandos: M/L/H/V/C/Q/A/Z, absolutos ou relativos.
- Após editar: `cargo xtask icons`, seguido de `cargo test -p xtask icons::`.
- A tabela `crates/ui-kit/src/icons_gen.rs` é determinística e comitada; não a
  edite à mão. Arcos são convertidos em cúbicas pelo gerador.
- A UI usa `Icon`, `icon`, `icon_button` (rótulo obrigatório) e `button_with_icon`.
