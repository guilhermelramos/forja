# forja-protocol

Contrato de fio entre o shell (`forja-app`), o CLI (`forja`) e o hook
(`forja-hook`). Não depende do kernel nem de egui, então o CLI compila e roda
sem a pilha gráfica.

## O que oferece

- `codec`: JSON-RPC 2.0 de uma linha por mensagem. Lotes são recusados; uma
  requisição sem `id` é notificação e não recebe resposta.
- `schema`: `Method`, `ParamSchema`, `ParamKind`, `ResultSchema` e
  `MethodCatalog`. `Method` carrega nome, `description_key`, parâmetros,
  resultado e as anotações `effect`, `network`, `idempotent` e `exposed_mcp`
  (leitura exposta por padrão). Esse é o dado devolvido por `meta.methods`; o
  CLI gera subcomandos e ajuda a partir dele.
- `pipe` (Windows): servidor de named pipe com `PIPE_REJECT_REMOTE_CLIENTS`,
  no máximo 16 conexões simultâneas, frames de até 1 MiB e timeout de 120 s
  para a thread da interface responder.
- `client`: chamada de uma linha com timeout de 30 s, notificação sem resposta
  e distinção entre "app fechado" e erro de protocolo.

## Nome do pipe

`\\.\pipe\forja-<usuario>` em minúsculas. `FORJA_PIPE` sobrepõe o nome para
testes isolados; nenhuma execução normal usa outro nome.

## Como testar

```sh
cargo test -p forja-protocol
```

Os testes cobrem o codec, o schema, o roundtrip real pelo pipe e o limite de
conexões. Os testes criam nomes de pipe únicos por processo e horário, então
duas execuções simultâneas não colidem.
