# forja-cli

CLI `forja`. Não tem lista de comandos: busca `meta.methods` no app e gera
subcomandos, ajuda e conversão de flags a partir do schema.

## Comandos

```sh
forja hello greet --nome Ana     # método hello.greet, alias do parâmetro
forja --json hello greet --name Ana
forja metodos                    # catálogo resumido
forja metodos --json             # catálogo completo
forja abrir                      # inicia forja-app.exe ao lado do CLI
```

- Código 0 em sucesso.
- Código 1 para erro de método, parâmetro ou protocolo.
- Código 2 quando o app não está aberto (`ClientError::NotRunning`).

Flags viram o objeto `params`; posicionais preenchem os parâmetros
obrigatórios na ordem do schema. Tipos são convertidos por `ParamKind`.

## Como testar

```sh
cargo build -p forja-cli
./target/debug/forja.exe metodos        # com o app aberto
./target/debug/forja.exe metodos        # com o app fechado: saída 2
```
