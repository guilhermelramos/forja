# counter_linear

Balcão Linear: issues como pedidos, por GraphQL. Contrato em `design/contracts.md`, seção
"Pedidos e balcões".

## O que registra

- `counter_linear.describe`, `.list`, `.get`, `.comment`, `.transition`, `.test`, `.connect`,
  `.disconnect`. Os de rede respondem `{started: true}` e publicam `counter.result`.
- Seção de Ajustes: conectar com chave de API, testar e desconectar.

## Regras

- Configuração: `counter.linear.team` (chave da equipe; vazio mostra as visíveis), lida do
  módulo `counter`.
- Endpoint único `https://api.linear.app/graphql`. A chave vai pura no cabeçalho
  `Authorization`, sem `Bearer`.
- Erros de GraphQL vêm com HTTP 200 e uma lista `errors`; o código (`AUTHENTICATION_ERROR`,
  `FORBIDDEN`, `NOT_FOUND`, `RATELIMITED`) vira o tipo de erro.
- `to_review` responde `unsupported`, sem chamada de rede: o Linear não tem revisão.
- Transição: escolhe o estado do time pelo tipo (`completed` para fechar, `unstarted` para
  abrir). Comentário: resolve o UUID do issue antes de criar o comentário.
- Cofre: `forja/counter_linear`, conta `api.linear.app`, e `api.linear.app#login`.

## Como testar

```sh
cargo test -p forja-module-counter_linear
```
