# forja-mcp

Ponte MCP stdio sem estado entre um cliente e o app da Forja.

```sh
forja-mcp
```

O processo lê JSON-RPC por linha no stdin e escreve apenas mensagens MCP no
stdout. Ele não abre o app: cada chamada vai pelo pipe local criado pelo
`forja-app`.

## Métodos

- `initialize` — aceita `2026-07-28` e `2025-11-25`;
- `ping`;
- `tools/list` — lê `meta.methods` e `mcp.tools`, filtra exposição e gera nomes
  `forja.<id>`;
- `tools/call` — chama o método correspondente e devolve `content`,
  `structuredContent` e `isError`.

Ferramenta desconhecida ou não exposta é erro JSON-RPC `-32602`. App fechado
vira resultado `isError` com `code = "app_closed"`.

## Segurança

- O stdout carrega só mensagens MCP; diagnósticos vão para stderr.
- A ponte não guarda token, não lê configuração e não escreve arquivos.
- A política de exposição e as listas de proibição são aplicadas no app pelo
  módulo `mcp`; a ponte repete a lista fixa de proteção.

## Testes

`cargo test -p forja-mcp` cobre `initialize`, `tools/list`, `tools/call`,
notificações, ferramenta proibida e app fechado com um backend falso.
