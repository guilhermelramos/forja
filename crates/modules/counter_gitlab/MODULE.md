# counter_gitlab

Balcão GitLab: issues (`#iid`) e merge requests (`!iid`) como pedidos. Contrato em
`design/contracts.md`, seção "Pedidos e balcões".

## O que registra

- `counter_gitlab.describe`, `.list`, `.get`, `.comment`, `.transition`, `.test`, `.connect`,
  `.disconnect`. Os de rede respondem `{started: true}` e publicam `counter.result`.
- Seção de Ajustes: conectar com token, testar e desconectar.

## Regras

- Configuração: `counter.gitlab.host` (padrão `gitlab.com`), lida do módulo `counter`.
- Token no cabeçalho `PRIVATE-TOKEN`, só para o host configurado.
- Caminho do projeto codificado como um segmento (`grupo%2Fprojeto`).
- Escopos: issues e merge requests são listados em duas consultas paginadas (`X-Next-Page`).
  `to_review` lista só merge requests com `reviewer_username`, porque issues não têm revisão.
- Notas de sistema (`system: true`) não são comentários.
- Transição por `state_event` (`close` ou `reopen`). `merged` responde `unsupported`.
- Cofre: `forja/counter_gitlab`, conta = host, e `<host>#login`.

## Como testar

```sh
cargo test -p forja-module-counter_gitlab
```
