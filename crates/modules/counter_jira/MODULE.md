# counter_jira

Balcão Jira: issues como pedidos, pela API REST v3 com JQL. Contrato em
`design/contracts.md`, seção "Pedidos e balcões".

## O que registra

- `counter_jira.describe`, `.list`, `.get`, `.comment`, `.transition`, `.test`, `.connect`,
  `.disconnect`. Os de rede respondem `{started: true}` e publicam `counter.result`.
- Seção de Ajustes: site, e-mail e token, com conectar, testar e desconectar.

## Regras

- Configuração: `counter.jira.project` (chave do projeto; vazio mostra todos), lida do
  módulo `counter`. A chave é conferida antes de entrar no JQL (sem injeção).
- Site e e-mail não são configurações: ficam no cofre, junto com o token, ao conectar.
- Autenticação básica (`e-mail:token`, em base64) e `Accept: application/json`.
- Lista: `POST /rest/api/3/search/jql` com `nextPageToken`, até 5 páginas.
- Escopos: `mine` = `assignee = currentUser()` e não concluído; `open` = `statusCategory != Done`;
  `to_review` responde `unsupported`.
- Transição: escolhe a transição pela categoria do status de destino (`done` para fechar,
  `new` para abrir).
- Mensagens de erro vêm de `errorMessages` quando a resposta traz esse campo.
- Cofre: `forja/counter_jira`, conta = site, com `<site>#email` e `<site>#login`.

## Como testar

```sh
cargo test -p forja-module-counter_jira
```
