# forja-module-accounts

Seção **Contas** nos Ajustes: várias contas por ajudante com home de
configuração separada, conta ativa e env injetado no `helper.launch`.

## O que registra

- Seção `accounts.settings` em `Slot::SettingsSection` (ordem 27).
- Comandos: `accounts.list`, `accounts.add`, `accounts.select`,
  `accounts.remove`, `accounts.env` e `accounts.roots`.
- Métodos RPC: `accounts.list` (read), `accounts.add`, `accounts.select` e
  `accounts.remove` (write).
- Configuração `accounts.confirm.remove` (padrão ligado).
- Bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`; `keymap.toml` vazio.

## Contratos

- **Perfil.** O campo novo `[accounts] env = "CLAUDE_CONFIG_DIR"` (e `login`) é
  lido dos perfis embutidos e da camada `~/.forja/helpers`. Hoje Claude Code e
  Codex declaram o campo, com o env já citado em `[sessions].dir_env` e o login
  na referência do CLI.
- **Dados.** Contas em `<dados>/accounts/<perfil>/<id>`, estado em
  `<dados>/modules/accounts.json` (`accounts` + `active` por perfil).
- **Remoção.** Só apaga `<raiz>/<perfil>/<id>`; `..`, caminhos fora da raiz e a
  própria pasta do perfil são recusados. `accounts.confirm.remove` exige
  `confirm=true`.
- **Login.** `accounts.add` cria a pasta, grava a conta e chama `terminal.open`
  com o executável de `helper.list`, os argumentos de `[accounts].login` e
  `{env: dir}`. Sem terminal/ajudante, a conta fica criada e o login não abre.
- **Env para o ajudante.** `accounts.env {profile_id}` devolve
  `{env: {VAR: dir}}` da conta ativa, ou `{}` sem conta.
  `helper.launch` chama esse comando e funde o env no plano do terminal.
- **Uso.** `accounts.roots` devolve `{accounts: [{profile_id, dir, …}]}` para o
  módulo `usage` incluir as homes das contas na varredura.
- `FORJA_ACCOUNTS_FIXTURE=1` mostra contas de exemplo na captura.

## Como testar

```sh
cargo test -p forja-module-accounts
cargo run -p forja-app
```

Os testes cobrem leitura dos perfis embutidos, criar/escolher/remover conta,
ids distintos para rótulos repetidos, env da conta ativa, remoção com bloqueio
de `..` e confirmação, contrato do kernel e o RPC `accounts.list`.
