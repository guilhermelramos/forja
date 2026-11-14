# forja-explore

Explorador de interface sem modelo de IA e executor de regressões gravadas (`forja-explore`).

## Propósito
- Dirigir `forja-app --test-driver` pelo pipe RPC (`ui.snapshot`, `ui.click`, `ui.type`, `ui.key`, `ui.scroll`, `ui.wait`) com escolha guiada por cobertura.
- Verificar invariantes após cada ação (resposta em < 10 s, textos sem tradução, erros técnicos crus, elementos fora da janela ou abas sobrepostas, comportamento de `Escape` em janelas, elementos clicáveis sem nome) e gravar achados deduplicados em JSONL.
- Rodar regressões gravadas (`--replay <dir>`) em um app limpo por arquivo.

## Como testar
```sh
cargo test -p forja-explore
cargo build -p forja-app -p forja-explore
./target/debug/forja-explore --app target/debug/forja-app.exe --replay playtest/regressions
./target/debug/forja-explore --app target/debug/forja-app.exe --actions 50 --seed 1 --locale pt-BR
```
