# forja-module-helper

Saídas com código de erro usam `helper.notifications.error`; saídas normais
continuam usando `helper.notifications.done`.
As opções de contexto, regras, prontidão e retomada automática estão adiadas
no catálogo até os contratos de lançamento e de sessão correspondentes existirem.

Perfis, detecção, início e status dos ajudantes (CLIs de agente) da Forja.

## O que registra

- Comandos `helper.list`, `helper.detect`, `helper.launch`, `helper.focus`,
  `helper.hook`, `helper.hooks.consent`, `helper.hooks.status`,
  `helper.hooks.install`, `helper.hooks.remove`.
- RPC de leitura/execução, exceto a concessão interna `helper.hooks.consent`;
  `helper.launch` aceita `--profile`/`profile_id` e
  devolve `terminal_id`, `profile_id` e `command_line`.
- Aba `helper.tab` "Ajudantes", com ativos, status, bancada, duração e foco do
  terminal.
- Seção de Ajustes: habilitar/desabilitar perfil, argumentos extras, ambiente,
  consentimento e instalar/remover hooks.
- Evento `helper.status_changed` (`HelperStatusChanged`).

## Perfis

Lê as três camadas, com precedência crescente: `content/helpers/*.toml`
(embutido, 16 perfis), `~/.forja/helpers/*.toml` e
`<projeto>/.forja/helpers/*.toml`. O formato é `design/helper-profile.md`.

- Valida id, nome, mantenedor, site, `detect.executables`, `start.command` e
  `prompt.mode` (`argument`, `flag`, `stdin`, `none`; vazio = `none`).
- Perfis de projeto com `permissions.automatic` ou `permissions.full` só
  iniciam depois de `trust.check` com `kind = "helper_profile"`.
- A ordem padrão é a de `design/helpers.md`: instalados primeiro, depois
  completude, confiança da fonte e id.
- `helper.default.profile` aceita `auto` ou um id; `helper.default.permissions`
  alimenta o modo quando o pedido não escolhe um.

## Detecção

Roda em segundo plano com cache por perfil. Procura cada executável em PATH com
PATHEXT, `.exe`, `.cmd` e `.ps1` e, quando encontra, executa os argumentos de
`[detect].version` sem janela de console. `helper.list` inicia a detecção na
primeira chamada.

## Início e prompt

`helper.launch` monta `[start].args` + `[permissions.<modo>].args` + prompt
conforme `[prompt].mode`, modelo/esforço e argumentos extras. Chama
`terminal.open`; se `mode` for `stdin`/`none` (ou vazio), entrega o prompt
depois por `terminal.send`. Sem o módulo terminal, devolve erro claro e não
quebra o resto do app.

Com `helper.launch.mode = "chat"` e o módulo `chat` presente, `helper.launch`
chama `chat.open` e, quando há prompt, `chat.send`, devolvendo `mode = "chat"`,
`terminal_id = null` e `session_id`. Sem o módulo `chat`, cai na TUI.

## Contas

Perfis com `[accounts] env = "CLAUDE_CONFIG_DIR"` (ou equivalente) podem ter
várias homes de configuração. Antes de abrir o terminal, `helper.launch` chama
`accounts.env {profile_id}` e funde o `{env}` devolvido no plano; sem o módulo
`accounts`, o env fica vazio e o CLI usa a home normal. O campo `login` do
mesmo bloco alimenta o botão de adicionar conta na seção Contas.

## Status

- Assina `terminal.exited`, `terminal.bell`, `terminal.command_finished` e
  `terminal.title_changed`; infere pelo título/saída quando não há hook.
- A ponte `hook.event` é registrada pelo módulo `hook` (o binário `forja-hook`
  chama esse método) e repassa para `helper.hook`. O payload mapeia eventos de
  permissão para `needs_you`, ferramenta/prompt para `working`, stop/sessão para
  `finished` e idle para `idle`.
- Mudanças publicam `helper.status_changed`.

## Hooks

Instalação idempotente em arquivos JSON de outros CLIs, apenas entradas com o
marcador `forja-hook --agent <id>`. Faz backup `.forja-bak`, preserva as demais
chaves e remove só o que é nosso.

Proteções:

- consentimento explícito por origem, hash do perfil e caminho de configuração;
  a concessão só é um comando interno da janela, nunca RPC/CLI;
- nunca escreve em testes ou capturas (`FORJA_SCREENSHOT=1`);
- nunca escreve com `FORJA_DATA_DIR` isolado;
- `FORJA_ALLOW_GLOBAL_CONFIG=1` é o opt-in explícito para execuções isoladas.

Leitura, instalação e remoção rodam em tarefas de fundo, serializadas; o resultado fica no estado do módulo. Os comandos e RPCs de alteração retornam `{pending: true}` quando aceitos, e `helper.hooks.status` retorna `pending` até o resultado estar disponível ou um erro se a operação falhou. Para reler a configuração externa, chame o status com `refresh = true` e consulte novamente sem esse parâmetro.

Perfis cujo formato de hook não é JSON aparecem como `unsupported`.

## Energia

Mantém o computador acordado (`SetThreadExecutionState`) enquanto existe um run
não terminado e libera ao terminar.

## Pendências

- O botão Focar usa `terminal.focus`, acrescentado ao contrato do terminal nesta
  onda.
- Perfis de projeto com risco dependem do módulo trust; sem ele, ficam
  bloqueados.
- O instalador de hooks cobre o formato JSON nested; Codex/OpenCode e formatos
  de plugin ficam como `unsupported` nesta entrega.

## Como testar

```sh
cargo test -p forja-module-helper
cargo run -p forja-app
forja helper list
forja helper launch --profile fake --prompt "hello"
forja terminal read --terminal_id <id>
```

## Lote 3 (cobertura do catálogo)

- Comando `helper.launch.profile` (Ctrl+Alt+Shift+A): abre o seletor de perfis
  (`picker.rs`, view modal `helper.profile_picker`); escolher um perfil chama `helper.launch`.
