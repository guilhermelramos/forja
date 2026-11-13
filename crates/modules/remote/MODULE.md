# remote

Destinos SSH e WSL da Forja. Um destino é um host salvo, importado do `~/.ssh/config` ou criado
na tela, e não guarda segredo: senha e passphrase ficam com o `ssh` e com o agente. Daí saem o
teste de conexão, o terminal SSH, os encaminhamentos de porta locais e o WSL (distros, terminal
numa distro com a pasta traduzida, e o comando que roda um programa dentro do WSL).

## Arquivos

| Caminho | Papel |
|---|---|
| `src/target.rs` | destino SSH, encaminhamento local e arquivo de destinos (sem segredo) |
| `src/ssh_config.rs` | leitor do `~/.ssh/config` (só leitura), para importar destinos |
| `src/ssh.rs` | argumentos do `ssh.exe` (teste, terminal, encaminhamento), validação, aspas POSIX e mapeamento do stderr |
| `src/wsl.rs` | lista do `wsl.exe -l -v` (UTF-16), tradução de caminhos e argumentos do `wsl.exe` |
| `src/forward.rs` | processos `ssh -N -L` de encaminhamento: iniciar, parar, estado e restauração |
| `src/job.rs` | Job Object do Windows (o `ssh` de encaminhamento morre com o app); no-op nos outros sistemas |
| `src/state.rs` | estado compartilhado, leitura das configurações e a foto do último teste |
| `src/actions.rs` | operações que tocam disco ou processo; tudo roda em `Ctx::spawn` |
| `src/commands.rs` | comandos, métodos RPC e as fotos que o CLI e a tela leem |
| `src/view.rs` | a seção "Remoto" em Ajustes |
| `src/fixtures.rs` | dados de captura (`FORJA_FIXTURES=1`), nunca gravados no arquivo real |
| `src/tests.rs` | contrato do kernel, terminal falso, recusas com SSH desligado |
| `tests/fixtures/` | `ssh_config/main.txt`, `wsl_list.txt` e `stderr/*.txt` (saídas reais de falhas do `ssh`) |

## O que registra

- Seção de Ajustes "Remoto" (`Slot::SettingsSection`, ordem 200): destinos com o último teste,
  encaminhamentos, distros do WSL e o formulário de destino novo.
- Comandos: `remote.ssh.save`, `remote.ssh.delete`, `remote.ssh.test`, `remote.ssh.open`,
  `remote.ssh.import`, `remote.forward.start`, `remote.forward.stop`, `remote.wsl.open`,
  `remote.wsl.wrap`.
- Métodos RPC/CLI: `remote.ssh.list` (read), `remote.ssh.test` (read, `network`),
  `remote.forward.list` (read), `remote.wsl.list` (read).
- Configurações do grupo "Remoto" (`design/settings.toml`): `remote.ssh.enabled`,
  `remote.ssh.config_path`, `remote.ssh.default_host`, `remote.ssh.connect_timeout_s`,
  `remote.ssh.keep_alive_s`, `remote.ssh.reuse_connections`, `remote.ssh.agent_forward`,
  `remote.ssh.host_key_policy`, `remote.wsl.default_distro`, `remote.wsl.default_user`,
  `remote.forward.mode`, `remote.latency.warning_ms`.

## Contratos

Consome `terminal.open` (módulo terminal) para abrir o `ssh.exe` e o `wsl.exe`. Sem esse
comando, `remote.ssh.open` e `remote.wsl.open` devolvem "comando desconhecido"; o resto
(destinos, teste, encaminhamentos, `remote.wsl.wrap`) continua funcionando. Não publica eventos.

Formas de JSON e códigos de erro: `design/contracts.md`, seção "Remoto".

## Regras

- **Sem segredo.** O arquivo de destinos (`<dados>/modules/remote.json`) guarda id, rótulo, host,
  usuário, porta, caminho da chave (`IdentityFile`), `ProxyJump` e encaminhamentos. Senha,
  passphrase e token não entram nele.
- **Importação.** `remote.ssh.import` lê o `~/.ssh/config` apenas para leitura, em `spawn`, e
  acrescenta os hosts, pulando os que já têm o mesmo host e o mesmo rótulo. Nunca escreve nesse
  arquivo.
- **Teste.** `ssh -o BatchMode=yes -o ConnectTimeout=N <destino> echo ok`, sem perguntas: um teste
  nunca abre prompt. Quando falha, o stderr vira um código (`remote-error-auth_failed`,
  `remote-error-unknown_host_key`, `remote-error-host_key_changed` etc.); a frase mais específica
  vence. Sucesso acima de `remote.latency.warning_ms` aparece como "lento".
- **Terminal SSH.** `terminal.open` com `ssh.exe` na pasta do usuário. Senha, confirmação de host
  e demais perguntas acontecem no terminal, não na Forja.
- **Encaminhamento.** `ssh -N -L 127.0.0.1:<porta>:<host>:<porta>` em segundo plano, com
  `BatchMode=yes` (a chave precisa estar liberada) e listener só em loopback. Cada processo entra
  num Job Object com `KILL_ON_JOB_CLOSE`, então morre com o app. Parar mata o processo.
- **Restauração.** `remote.forward.mode = auto` sobe, na primeira leitura, os encaminhamentos
  marcados como ativos. Com `ask` ou `off`, nada sobe sozinho. Encaminhamentos não são retomados
  depois de uma queda: quando o `ssh` sai, o estado vira `Exited`, com o código e o stderr dele
  (porta ocupada, host inalcançável), e o usuário sobe de novo pela tela.
- **WSL.** `wsl.exe -l -v` é lido em UTF-16 (com ou sem BOM). A marca `*` indica a distro padrão.
  A lista fica em cache; `remote.wsl.list` devolve o que já está lá e dispara a leitura em segundo
  plano.
- **Caminhos.** `C:\pasta` vira `/mnt/c/pasta`; `\\wsl$\<distro>\caminho` vira `/caminho`. Outros
  UNC e caminhos relativos não têm tradução, e o terminal abre sem pasta.
- **`remote.wsl.wrap` é puro.** Monta `{program, args, cwd}` para um programa rodar dentro do
  WSL (`wsl.exe -d <distro> -u <usuário> --cd <pasta> -- sh -lc <comando>`), sem rodar nada.
  Comando vazio é recusado.
- **Escrita.** Gravar é recusado enquanto o arquivo carrega e se ele existe mas não pôde ser lido
  (nada é sobrescrito). Erros de gravação aparecem como aviso, sem derrubar a tela.
- **Processos.** `ssh` e `wsl.exe` rodam sem janela (`CREATE_NO_WINDOW`), com entrada nula. Os
  testes nunca chamam os programas de verdade: usam fixtures de saída.

## Limitações conhecidas

- `remote.ssh.default_host` e `remote.ssh.reuse_connections` estão no catálogo
  (`design/settings.md`) e aparecem em Ajustes, mas ainda não mudam nenhum comportamento. Não há
  `ControlMaster` no Windows, então a reutilização de conexões não é aplicada.
- `remote.sync.mode` está adiada no catálogo: o módulo não sincroniza arquivos, então a opção não aparece em Ajustes.
- `remote.forward.mode` controla só a restauração na abertura.

## Como testar

```sh
cargo test -p forja-module-remote
FORJA_FIXTURES=1 cargo run -p forja-app -- --screenshot shots/remote-settings.png --slot settings
```

A captura com `--slot settings` abre Ajustes no topo. A seção "Remoto" fica no fim da lista, e a
captura corta o fim dela: o WSL e o formulário de destino novo não aparecem na imagem. Para ver
tudo, role Ajustes na tela.

Os testes cobrem: o leitor do `~/.ssh/config` (fixture), a montagem dos argumentos do `ssh` e do
`wsl.exe`, o mapeamento do stderr (fixtures), a lista do WSL em UTF-16 com e sem BOM, a tradução de
caminhos, a validação de destino e o id gerado, o terminal SSH com terminal falso, a recusa com SSH
desligado e a recusa de gravar antes do carregamento.
