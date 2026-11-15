# Testadores automáticos (play tests)

Agentes de IA gratuitos usam a Forja **ao vivo, como uma pessoa**: olham a tela, clicam, digitam, apertam atalhos e confirmam o resultado de cada ação. Cada um tem uma missão. Vários rodam ao mesmo tempo na nuvem, um runner Windows do GitHub Actions para cada, e o que acham vira issue com o rótulo `playtest`.

## Como funciona
1. **`forja-app --test-driver`**: só com essa opção, o app responde aos métodos `ui.*` pelo pipe local.
   - `ui.snapshot` devolve os elementos visíveis a partir da árvore de acessibilidade: nome, papel, estado e retângulo de cada um. `region` limita a uma parte (`cima`, `baixo`, `esquerda`, `direita`, `centro` ou o nome de uma janela) e `filter` a nomes/papéis que contenham o texto.
   - `ui.click`, `ui.type`, `ui.key` e `ui.scroll` viram **eventos reais de mouse e teclado** dentro do egui, um passo por quadro.
   - Cada ação responde depois que a interface assentou, só com o que **apareceu**, **sumiu**, **mudou** ou é novo.
   - Tudo é registrado em `FORJA_TEST_LOG` (JSONL).
   - Seletores nativos de pasta e arquivo respondem sozinhos (`FORJA_TEST_PICK_DIR`, `FORJA_TEST_PICK_FILE`; veja `crates/kernel/src/test_pick.rs`).
2. **`forja-mcp --player --findings <arquivo>`** expõe ao agente só as ferramentas de usuário: `ui_look`, `ui_click`, `ui_type`, `ui_key`, `ui_scroll` e `ui_wait`. `ui_look` é compacto (agrupa repetidos, corta nomes longos e omite retângulos de elementos não acionáveis). A ferramenta `report_finding` grava um achado com passos, esperado e ocorrido; nos vereditos ela também grava a lista estruturada `expectations`. `--max-steps <n>` limita as ações de uma sessão.
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
# Reproduzir uma janela específica:
./playtest/run.ps1 -Mission playtest/missions/caos.md -WindowSize 1280x800
```

A janela de teste em segundo plano usa 1920x1080 por padrão. `FORJA_TEST_WINDOW_SIZE=1280x800` (ou `-WindowSize` no `run.ps1`) muda o tamanho sem trazer a janela para a frente.

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

O código de saída é `0` quando a exploração rodou, mesmo que tenha achados (inclusive crashes/hangs, que viram linhas do `findings.jsonl`). Só há falha de processo (`1`) quando o app não abre, o pipe não responde no início ou uma regressão gravada não passa; argumentos inválidos saem com `2`.

O workflow `.github/workflows/explore.yml` roda 6 exploradores em paralelo de hora em hora (sementes e locales `pt-BR`/`en`), guarda achados (`findings.jsonl`) e trilhas (`trail.jsonl`) como artefatos e abre issues com os rótulos `playtest` e `explore` via `playtest/report.ps1`.

## Regressões gravadas (`playtest/regressions/`)
Cada arquivo JSON em `playtest/regressions/` descreve um fluxo reproduzível no formato do `drive.ps1` e suas expectativas (`appeared`, `present`, `present_contains`, `absent`, `absent_contains`, `window_count`, `window_count_min`, `window_count_max`, com `step` opcional). O campo opcional `seed` aponta uma pasta, relativa ao JSON, copiada para a pasta de dados antes de o app abrir; use para layouts gravados que uma sequência de cliques não alcança de forma determinística:

```powershell
./target/debug/forja-explore.exe --app target/debug/forja-app.exe --replay playtest/regressions
```

O CI (`.github/workflows/ci.yml`, job `regressions`) executa todas as regressões em um app limpo por arquivo e falha se alguma expectativa não bater.

## Verificar correções de issues

O workflow `.github/workflows/verify.yml` seleciona até 8 issues com o rótulo
`playtest`, `explore` ou `ideia` e um comentário do dono do repositório iniciado
por `Corrigido na main`, `Tratado na main` ou `Concluído na main`. A ordem segue
o triador: primeiro as issues com `prioridade-5`, depois `prioridade-4`, `3`, `2`
e `1`, e por último as que não têm rótulo de prioridade; dentro da mesma
prioridade, a mais antiga vem primeiro. Issues com o rótulo `em-andamento` são
puladas com uma linha de log, porque um worker ainda está mexendo nelas. A issue
entra quando a correção mais nova é posterior ao veredito mais novo; o comentário
decisivo não precisa ser o último da thread. Cada item do JSON selecionado traz
`priority` (5 a 1, ou `0` quando não há rótulo).
Issues fechadas também entram quando **já tinham um veredito**, a correção foi
postada depois do fechamento e o **último evento de fechamento** foi do
`github-actions[bot]` (consultado em `gh api repos/OWNER/REPO/issues/N/events`).
Se a issue foi fechada por uma pessoa, ela é ignorada com uma linha de log e
**nunca é reaberta**, nem com `still_broken`. Foi o caso da #13, fechada pela
conta `guilhermelramos` 13 segundos antes do comentário novo.

No runner Windows, `playtest/verify.ps1` abre uma instância limpa de
`forja-app --test-driver` para cada tentativa, com pasta de dados isolada,
projeto de exemplo de `seed.ps1`, no máximo 40 ações e 8 minutos por issue. O
tamanho da janela é o citado pela issue ou pelo comentário de correção
(`1280×800` ou `1280x800` viram 1280x800); sem citação, usa 1920x1080. A opção
`-WindowSize 1280x800` força um tamanho em execuções locais.

O OpenCode só recebe as ferramentas do `forja-mcp --player`; o modelo deve chamar
`report_finding` com `VEREDITO: fixed`, `VEREDITO: still_broken` ou
`VEREDITO: cannot_reproduce` e com a lista estruturada
`expectations: [{ item, ok, observed }]`. `fixed` exige que **todos** os itens
tenham `ok: true`; qualquer item falso vira `still_broken`, e condições
impossíveis de montar viram `cannot_reproduce`. A lista é a fonte do veredito:
o script não rebaixa mais por palavras, porque frases como "Não abriu nenhuma
janela de Ajustes" podem descrever o comportamento esperado. O padrão usa
modelos gratuitos no OpenRouter e `VERIFY_MODELS` permite trocar a lista,
aceitando apenas ids que terminem em `-free` ou `:free`. Configure
`OPENROUTER_API_KEY` nos secrets do repositório; `VERIFY_MODELS` pode ser
definido como variável do repositório.

O job final `apply` só age sobre issues com correção pendente. `fixed` fecha a
issue com comentário e evidências; se a issue já estava fechada por um pedido de
re-verificação, apenas comenta a confirmação. Os outros dois resultados
comentam e aplicam `reaberto` ou `verificar-manual`, reabrindo antes quando for
uma re-verificação de issue fechada **pelo bot**; issue fechada por pessoa é
pulada sem comentário, rótulo ou `gh issue reopen`. Um `fixed` sem a lista
estruturada é aceito como veredito legado, mas a heurística de palavras (`não
teve efeito`, `não funcionou`, `não abriu`, `falhou`, `sem efeito`, `did not`,
`no effect`) fica registrada apenas como aviso no log e não rebaixa o resultado.
A opção `-DryRun` mostra o que o script faria sem alterar issues:

```powershell
./playtest/apply-verdicts.ps1 -VerdictsPath verification-out/verdicts.jsonl -DryRun
```

As regras de tamanho, de veredito e de seleção têm testes de fixture que rodam
sem abrir o app nem tocar no GitHub. O teste de seleção usa o histórico real da
#13 (fechada por pessoa) e uma variante fechada pelo bot, roda o
`apply-verdicts.ps1 -DryRun` contra as duas e cobre a escolha por prioridade com
fixtures (mistura de prioridades, `em-andamento` e sem rótulo), inclusive o
corte de `MaxIssues` depois da ordenação:

```powershell
./playtest/test-window-size.ps1
./playtest/test-verdicts.ps1
./playtest/test-selection.ps1
./playtest/test-triage.ps1
```

## Triagem automática de issues

O workflow `.github/workflows/triage.yml` executa `playtest/triage.ps1` (em
`ubuntu-latest`, ao abrir/reabrir issues, de hora em hora ou via
`workflow_dispatch`) para classificar issues abertas sem `prioridade-N` segundo
`playtest/TRIAGE_RUBRIC.md`. O triador aplica atalhos determinísticos antes da
IA (`crash`/`travou` → `5`, `ideia` sem perda de dados → máximo `3`, `explore`
`elemento sem nome acessível` → `2`), valida o JSON do modelo (`priority` em
`1–5`, `area` válida, `rationale`, `duplicate_of`), comenta uma única linha e
nunca fecha issues nem altera o rótulo `em-andamento`. A variável `TRIAGE_MODELS`
aceita modelos gratuitos (`-free` ou `:free`) e `-DryRun` mostra o plano sem
escrever no GitHub:

```powershell
./playtest/triage.ps1 -Repository guilhermelramos/forja -DryRun -MaxIssues 20 -Models opencode/glm-5.3-flash-free
./playtest/test-triage.ps1
```

As opções de CLI e os controles de ferramentas seguem a documentação oficial
do [GitHub CLI para issues](https://cli.github.com/manual/gh_issue), [API do GitHub CLI](https://cli.github.com/manual/gh_api),
[GitHub Actions](https://docs.github.com/actions) e do [OpenCode](https://opencode.ai/docs/config/), [CLI](https://opencode.ai/docs/cli/), [modelos](https://opencode.ai/docs/models/), [permissões](https://opencode.ai/docs/permissions/), [servidores MCP](https://opencode.ai/docs/mcp-servers/) e [provedores](https://opencode.ai/docs/providers/).
