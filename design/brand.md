# Forja: Identidade

## Nome
**Forja**. O binário do app é `forja-app.exe`, o CLI é `forja` e o hook de agentes é `forja-hook`. As pastas de dados ficam em `%APPDATA%\Forja` (estado) e `~/.forja` (conteúdo do usuário). A config por repositório é `forja.toml`, e a pasta de conteúdo do projeto é `.forja/`.

## Conceito visual
- **Marca:** uma bigorna estilizada vista de lado, com uma centelha (losango) acima. Ela é geométrica, feita com poucas formas e legível em 16 px.
- **Ícone do app:** a bigorna em branco-quente sobre um quadrado arredondado em gradiente "brasa".
- **Paleta "Brasa"** (tokens no `ui-kit`):

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `bg` | #121010 | #FAF8F5 | fundo da janela |
| `surface` | #1B1816 | #FFFFFF | painéis |
| `surface_alt` | #231F1C | #F2EEE9 | listas e abas inativas |
| `border` | #2E2925 | #E3DDD6 | divisores |
| `text` | #EDE7E1 | #1F1A17 | texto |
| `text_muted` | #A69C93 | #6B625B | secundário |
| `accent` | #F2762E | #D9601B | ação principal, foco (brasa) |
| `accent_soft` | #3A2416 | #FCE6D7 | fundo de destaque |
| `spark` | #FFC14D | #B7860B | avisos, atenção (centelha) |
| `ok` | #6CC48A | #2E8A4E | sucesso |
| `danger` | #E5534B | #C23A32 | erro e destrutivo |
| `info` | #6FA8DC | #2F6FA8 | links e informação |

- **Tipografia:** Segoe UI Variable / Segoe UI na interface e Cascadia Code no código e no terminal. Os títulos usam peso 600.
- **Forma:** raio de 6 px nos controles e 10 px nos cartões, grade de 4 px, densidade "confortável" por padrão e "compacta" como opção.
- **Movimento:** transições curtas (120 ms) e respeito a "reduzir efeitos".

## Tom de voz
Direto, caloroso e em português claro. Verbos de ofício: *forjar*, *guardar*, *entregar*. Mensagens de erro dizem o que aconteceu e o que fazer em seguida, sem culpar o usuário.

## Terminal: temas próprios
- **Brasa Escuro** (padrão escuro) e **Brasa Claro** (padrão claro), além de **Carvão**, **Cinza Quente** e **Aço**. Todas as paletas são criadas por nós e ficam em `content/themes/`.
- Paletas públicas conhecidas só entram como extensão opcional, com créditos.
