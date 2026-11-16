# forja-explore

Explorador de interface sem modelo de IA e executor de regressões gravadas (`forja-explore`).

## Propósito
- Dirigir `forja-app --test-driver` pelo pipe RPC (`ui.snapshot`, `ui.click`, `ui.type`, `ui.key`, `ui.scroll`, `ui.wait`) com escolha guiada por cobertura.
- Verificar invariantes após cada ação (resposta em < 10 s, textos sem tradução, erros técnicos crus, elementos fora da janela ou abas sobrepostas, comportamento de `Escape` em janelas, elementos clicáveis sem nome) e gravar achados deduplicados em JSONL.
- Rodar regressões gravadas (`--replay <dir>`) em um app limpo por arquivo. Expectativas em `expect[]` também aceitam `max_frames_delta` (com `from_step` opcional; padrão é o passo anterior) para validar que a janela não redesenha em repouso usando o campo `frames` dos snapshots.
- Sair com `0` quando a exploração rodou, mesmo com achados; `1` só em erro real (app não abre, pipe sem resposta inicial, regressão falha); `2` em argumentos inválidos.

## Como testar
```sh
cargo test -p forja-explore
cargo build -p forja-app -p forja-explore
./target/debug/forja-explore --app target/debug/forja-app.exe --replay playtest/regressions
./target/debug/forja-explore --app target/debug/forja-app.exe --actions 50 --seed 1 --locale pt-BR
```
