# forja-module-mcp

Configuração do servidor MCP da Forja. A ponte stdio vive em `crates/mcp`
(`forja-mcp`); este módulo decide a exposição e mostra os snippets.

## O que registra

- Configurações do grupo `mcp`: `mcp.server.enabled`,
  `mcp.server.expose`, `mcp.server.auto_approve`, `mcp.server.timeout_s`,
  `mcp.log.requests` e `mcp.tools.auto_approve`.
- `mcp.log.requests` (desligado por padrão) grava uma linha por requisição que
  o servidor MCP recebe, no log do app: método, `ok`/`error` e duração em ms.
  Argumentos nunca entram na linha.
- `mcp.tools.auto_approve` (lista vazia por padrão) soma-se a
  `mcp.server.auto_approve` no fluxo de aprovação por chamada; um método que o
  código nunca expõe não é aprovado nem se estiver na lista.
- Comandos/RPC `mcp.tools` (read) e `mcp.setup` (read).
- Comandos/RPC `mcp.authorize` (write) e `mcp.approval` (read) para a
  aprovação por chamada.
- Seção de Ajustes com o estado do servidor, a contagem de ferramentas e os
  snippets dos ajudantes.
- Modal `mcp.approval`, que mostra cliente, ferramenta, efeito e pedidos na
  fila, com as decisões "permitir uma vez", "permitir até a Forja fechar" e
  "recusar".

## Exposição

`mcp.tools` percorre o registro de métodos e devolve
`{enabled, tools: [{id, name, description, effect, network, idempotent,
auto_approve}]}`.

- `mcp.server.enabled = false` devolve lista vazia.
- Leitura entra por padrão, exceto `archive.search`, `archive.list`,
  `terminal.read`, `settings.get` e `helper.hook`.
- Escrita entra só quando o id está em `mcp.server.expose`.
- Destrutivo nunca entra. O namespace `mcp.*`, `trust.*`, `approval.*`,
  `permission.*`, `extension.*`, `helper.hooks.*`, `settings.set`,
  `terminal.send`, `bench.remove`, `bench.archive` e `hook.event` são
  proibidos por código.
- `mcp.server.auto_approve` só marca escritas de baixo risco; nunca
  destrutivas.

## Setup

`mcp.setup` devolve snippets para Claude Code (JSON `mcpServers`), Codex (TOML
`mcp_servers`) e OpenCode (JSON `mcp` local), com a fonte de cada perfil. O
campo `writes` é sempre `false`: o módulo nunca grava na configuração de outro
CLI.

## Aprovação por chamada

Uma escrita exposta que não está em `mcp.server.auto_approve` chama
`mcp.authorize` antes de rodar. O pedido entra numa fila de no máximo cinco e
abre o modal `mcp.approval` com `open_modal_with`:

- **permitir uma vez** libera apenas aquela chamada;
- **permitir até fechar** libera o par cliente/ferramenta enquanto o app viver;
- **recusar** devolve `denied`;
- sem decisão até `mcp.server.timeout_s`, o estado vira `timed_out` e a chamada
  é negada.

O bridge consulta `mcp.approval` a cada 200 ms (prazo rígido de 650 s) e só
chama o método depois de `approved`. O modal nunca aparece para leituras nem
para escritas em `mcp.server.auto_approve`.

## Contrato

Consome o registro de métodos do kernel. Publica `mcp.tools`, `mcp.setup`,
`mcp.authorize` e `mcp.approval`. Não importa o crate de outro módulo.

## Limitações

- O cliente MCP ainda não existe; suas três opções estão adiadas no catálogo
  e não são apresentadas como configurações funcionais.
- O registro de requisições está adiado até a ponte expor os eventos e haver
  um serviço de log.
- A ponte valida a existência da ferramenta e a exposição; a validação completa
  de `outputSchema` fica para a evolução do `crates/mcp`.
- O timeout é aplicado na leitura de `mcp.approval` (poll do bridge), não por
  um timer da interface.

## Como testar

```sh
cargo test -p forja-module-mcp -p forja-mcp
forja mcp tools
forja mcp setup
```
