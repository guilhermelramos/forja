# counter_github

Balcão GitHub: issues e pull requests como pedidos. Um módulo `counter_<nome>` por balcão;
o contrato está em `design/contracts.md`, seção "Pedidos e balcões".

## O que registra

- `counter_github.describe`, `.list`, `.get`, `.comment`, `.transition`, `.test`, `.connect`,
  `.disconnect`. Os de rede respondem `{started: true}` e publicam `counter.result`.
- Seção de Ajustes (slot `settings_section`): conectar com token, testar e desconectar.

## Regras

- Configurações lidas do módulo `counter`: `counter.github.host` (padrão `github.com`),
  `counter.github.include_drafts` (padrão desligado), `counter.github.label_filter`.
- Remoto: lê `origin` da pasta da bancada. O token só vai ao host configurado; host diferente
  é recusado antes de qualquer requisição.
- Cofre: `forja/counter_github`, conta = host, e `<host>#login` para o login confirmado.
- Escopos: `mine` por `assignee`, `to_review` por busca (`review-requested:<login>`), `open`
  e `all` pela lista de issues. Issues e pull requests usam o mesmo endpoint.
- Transição: só `open` e `closed` (PATCH no estado). `merged` responde `unsupported`.
- Paginação pelo cabeçalho `Link`, até 5 páginas.

## Como testar

```sh
cargo test -p forja-module-counter_github
```

As fixtures ficam em `tests/fixtures/`. Os testes cobrem parsing, paginação, 401, 403, 404,
429 (inclusive o 403 de limite zerado), montagem de requisição e a regra de não enviar o
token a outro host.
