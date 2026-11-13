# Testadores automáticos (play tests)

Agentes de IA gratuitos usam a Forja **ao vivo, como uma pessoa**: olham a tela, clicam, digitam, apertam atalhos e confirmam o resultado de cada ação. Cada um tem uma missão. Vários rodam ao mesmo tempo na nuvem, um runner Windows do GitHub Actions para cada, e o que acham vira issue com o rótulo `playtest`.

## Como funciona
1. **`forja-app --test-driver`**: só com essa opção, o app responde aos métodos `ui.*` pelo pipe local.
   - `ui.snapshot` devolve os elementos visíveis a partir da árvore de acessibilidade: nome, papel, estado e retângulo de cada um.
   - `ui.click`, `ui.type`, `ui.key` e `ui.scroll` viram **eventos reais de mouse e teclado** dentro do egui, um passo por quadro.
   - Cada ação responde depois que a interface assentou, com o que **apareceu** e o que **sumiu**.
   - Tudo é registrado em `FORJA_TEST_LOG` (JSONL).
   - Seletores nativos de pasta e arquivo respondem sozinhos (`FORJA_TEST_PICK_DIR`, `FORJA_TEST_PICK_FILE`; veja `crates/kernel/src/test_pick.rs`).
2. **`forja-mcp --player --findings <arquivo>`** expõe ao agente só as ferramentas de usuário: `ui_look`, `ui_click`, `ui_type`, `ui_key`, `ui_scroll` e `ui_wait`. A ferramenta `report_finding` grava um achado com passos, esperado e ocorrido.
3. **`playtest/run.ps1`**:
   - cria a pasta de dados isolada;
   - abre o app;
   - roda sessões do OpenCode até o prazo, com `INSTRUCTIONS.md`, a missão, o resumo da sessão anterior e os achados já registrados;
   - se o app cair, isso vira achado `crash` e o app é reaberto.
4. **`.github/workflows/playtest.yml`**:
   - roda a cada 3 horas, com um job por missão em `playtest/missions/`;
   - o job `report` junta os achados e abre as issues novas com `playtest/report.ps1`, sem repetir títulos parecidos;
   - as sessões completas (transcrições e ações) ficam como artefatos por 30 dias.

## Criar uma missão
Crie `playtest/missions/<nome>.md` com o objetivo e os passos. Depois acrescente `<nome>` em `matrix.mission` no workflow.

## Rodar localmente
```powershell
cargo build -p forja-app -p forja-mcp
./playtest/seed.ps1
./playtest/run.ps1 -Mission playtest/missions/caos.md -Minutes 15 -App target/debug/forja-app.exe -Mcp target/debug/forja-mcp.exe
```

## Configuração
- Variável `PLAYTEST_MODELS` (opcional): modelos separados por vírgula, alternados por sessão. O padrão é `opencode/glm-5.3-flash-free,opencode/deepseek-v4.1-flash-free`.
- Secret `OPENCODE_API_KEY` (opcional): só se os modelos gratuitos passarem a exigir login.
