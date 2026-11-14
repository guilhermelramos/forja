# Testadores automáticos (play tests)

Agentes de IA gratuitos usam a Forja **ao vivo, como uma pessoa**: olham a tela, clicam, digitam, apertam atalhos e confirmam o resultado de cada ação. Cada um tem uma missão. Vários rodam ao mesmo tempo na nuvem, um runner Windows do GitHub Actions para cada, e o que acham vira issue com o rótulo `playtest`.

## Como funciona
1. **`forja-app --test-driver`**: só com essa opção, o app responde aos métodos `ui.*` pelo pipe local.
   - `ui.snapshot` devolve os elementos visíveis a partir da árvore de acessibilidade: nome, papel, estado e retângulo de cada um. `region` limita a uma parte (`cima`, `baixo`, `esquerda`, `direita`, `centro` ou o nome de uma janela) e `filter` a nomes/papéis que contenham o texto.
   - `ui.click`, `ui.type`, `ui.key` e `ui.scroll` viram **eventos reais de mouse e teclado** dentro do egui, um passo por quadro.
   - Cada ação responde depois que a interface assentou, só com o que **apareceu**, **sumiu**, **mudou** ou é novo.
   - Tudo é registrado em `FORJA_TEST_LOG` (JSONL).
   - Seletores nativos de pasta e arquivo respondem sozinhos (`FORJA_TEST_PICK_DIR`, `FORJA_TEST_PICK_FILE`; veja `crates/kernel/src/test_pick.rs`).
2. **`forja-mcp --player --findings <arquivo>`** expõe ao agente só as ferramentas de usuário: `ui_look`, `ui_click`, `ui_type`, `ui_key`, `ui_scroll` e `ui_wait`. `ui_look` é compacto (agrupa repetidos, corta nomes longos e omite retângulos de elementos não acionáveis). A ferramenta `report_finding` grava um achado com passos, esperado e ocorrido. `--max-steps <n>` limita as ações de uma sessão.
3. **`playtest/run.ps1`**:
   - cria a pasta de dados isolada;
   - abre o app;
   - roda sessões do OpenCode até o prazo, com `INSTRUCTIONS.md`, a missão, o resumo da sessão anterior e os achados já registrados;
   - roda o OpenCode com um diretório de configuração só do teste, sem skills, plugins ou MCP globais da máquina;
   - limita cada sessão a `-MaxSteps` ações (padrão 40) e passa à próxima um **resumo estruturado** (telas visitadas, testado, achados, a seguir) em vez do texto final cru;
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

## Explorador sem IA (`forja-explore`)
O binário `forja-explore` (`crates/explore`) dirige `forja-app --test-driver` sem modelo de IA, com escolha guiada por cobertura (assinatura de tela = hash dos papéis/nomes visíveis, priorização de elementos ainda não acionados, entradas difíceis em campos editáveis, atalhos de `forja-app --dump-catalog`, `Escape`, `Tab`, rolagem e retorno à tela inicial).

Após cada ação, verifica seis invariantes:
1. app vivo e resposta em `< 10 s` (senão registra `crash` ou `hang` com os últimos passos e reabre);
2. texto visível sem tradução comparando com todos os `i18n/{en,pt-BR}.ftl` do repositório;
3. erro técnico cru na tela (`failed`, `error:`, `os error`, `invalid setting`, crases com ids internos, `{"`, `\\?\`);
4. elemento fora dos limites da janela ou abas da mesma faixa sobrepostas;
5. `Escape` com janela aberta fecha exatamente uma (`Window`);
6. elemento clicável sem nome acessível (uma vez por tela).

```powershell
cargo build -p forja-app -p forja-explore
./target/debug/forja-explore.exe --app target/debug/forja-app.exe --minutes 10 --seed 1 --locale pt-BR --out explore-out
```

O workflow `.github/workflows/explore.yml` roda 6 exploradores em paralelo de hora em hora (sementes e locales `pt-BR`/`en`), guarda achados (`findings.jsonl`) e trilhas (`trail.jsonl`) como artefatos e abre issues com os rótulos `playtest` e `explore` via `playtest/report.ps1`.

## Regressões gravadas (`playtest/regressions/`)
Cada arquivo JSON em `playtest/regressions/` descreve um fluxo reproduzível no formato do `drive.ps1` e suas expectativas (`appeared`, `present`, `present_contains`, `absent`, `absent_contains`, `window_count`, `window_count_min`, `window_count_max`, com `step` opcional):

```powershell
./target/debug/forja-explore.exe --app target/debug/forja-app.exe --replay playtest/regressions
```

O CI (`.github/workflows/ci.yml`, job `regressions`) executa todas as regressões em um app limpo por arquivo e falha se alguma expectativa não bater.

## Verificar correções de issues

O workflow `.github/workflows/verify.yml` seleciona até 8 issues abertas, da
mais antiga para a mais nova, com o rótulo `playtest`, `explore` ou `ideia` e
um comentário do dono do repositório iniciado por `Corrigido na main`,
`Tratado na main` ou `Concluído na main`. Ele ignora uma issue cujo comentário
mais recente já seja um veredito, a menos que exista uma correção posterior.

No runner Windows, `playtest/verify.ps1` abre uma instância limpa de
`forja-app --test-driver` para cada tentativa, com pasta de dados isolada,
projeto de exemplo de `seed.ps1`, no máximo 40 ações e 8 minutos por issue. O
OpenCode só recebe as ferramentas do `forja-mcp --player`; o modelo deve chamar
`report_finding` com `VEREDITO: fixed`, `VEREDITO: still_broken` ou
`VEREDITO: cannot_reproduce`. O padrão usa modelos gratuitos no OpenRouter e
`VERIFY_MODELS` permite trocar a lista, aceitando apenas ids que terminem em
`-free` ou `:free`. Configure `OPENROUTER_API_KEY` nos secrets do repositório;
`VERIFY_MODELS` pode ser definido como variável do repositório.

O job final `apply` só age sobre issues que continuam abertas e cuja correção
continua sendo a mesma. `fixed` fecha com comentário e evidências; os outros
dois resultados comentam e aplicam `reaberto` ou `verificar-manual`. A opção
`-DryRun` mostra o que o script faria sem alterar issues:

```powershell
./playtest/apply-verdicts.ps1 -VerdictsPath verification-out/verdicts.jsonl -DryRun
```

As opções de CLI e os controles de ferramentas seguem a documentação oficial
do [GitHub CLI para issues](https://cli.github.com/manual/gh_issue), [API do GitHub CLI](https://cli.github.com/manual/gh_api),
[GitHub Actions](https://docs.github.com/actions) e do [OpenCode](https://opencode.ai/docs/config/), [CLI](https://opencode.ai/docs/cli/), [modelos](https://opencode.ai/docs/models/), [permissões](https://opencode.ai/docs/permissions/), [servidores MCP](https://opencode.ai/docs/mcp-servers/) e [provedores](https://opencode.ai/docs/providers/).
