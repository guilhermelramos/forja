# forja-kernel

Núcleo modular da Forja. Uma funcionalidade = um crate que implementa
`Module` e publica uma fábrica com `#[forja_kernel::module]`. O kernel não
depende de egui no build padrão.

## Contrato de módulo

```rust
pub struct ModuleManifest { id, name_key, version, depends_on }
pub trait Module: Send {
    fn manifest(&self) -> ModuleManifest;
    fn register(&mut self, r: &mut Registrar<'_>);
}
```

`linkme` descobre as fábricas; `build_registry` ordena por `depends_on`,
executa `register` e valida ids duplicados, prefixos e schema. A macro
`#[forja_kernel::module]` vive em `forja-kernel-macros` e só prepende o
atributo do `linkme` (por isso o crate do módulo depende de `linkme`).

## O que o `Registrar` registra

- comandos: `CommandId`, chave i18n do título, disponibilidade e handler
  `Fn(&mut Ctx, Args) -> Result<Value>`;
- views: slot (`rail`, `sidebar`, `bench_tab`, `right_panel`, `status_bar`,
  `settings_section`, `modal`, `anvil_source`, `top_bar`) + fábrica. Um item de
  `rail` é uma página: `view_glyph` / `view_png` desenham o ícone no trilho e
  `ui()` ocupa o centro. `Ctx::open_modal` / `close_modal` controlam o slot
  `modal`. `View::anvil_hits` devolve linhas ranqueadas (título, subtítulo, ícone
  opcional, comando e argumentos) para a consulta da Bigorna. `open_modal_with`
  guarda um payload JSON.
  O registry guarda `Box<dyn Any>`; com a feature `egui`, `Registrar::view` aceita
  `Box<dyn ui_contract::View>`;
- settings: `SettingSpec { key, kind, default, label_key, help_key, order, scope }`,
  com `Bool`, `Integer`, `Number`, `Text`, `Choice`, `Path`, `List` e `Color`,
  faixas, opções e escopo (`SettingScope::User` ou `SettingScope::Project`,
  preenchido automaticamente a partir de `PROJECT_SAFE_KEYS` de `design/settings.toml`
  ou ajustado com `.scope(...)`);
- atalhos: `keymap.toml` embutido com `include_str!` ou `KeyBinding` direto;
- eventos: `event::<T>("modulo.evento")` para eventos do próprio módulo e
  `contract_event::<T>(forja_contracts::names::...)` para eventos de contrato
  (publicador e assinante declaram; a ordem não importa); qualquer módulo também
  pode assinar `SettingsChanged` (`"settings.changed"`) com `r.subscribe(...)`;
- RPC: `Method` (nome, `description_key`, schema de parâmetros e resultado,
  `effect`, `network`, `idempotent`, `exposed_mcp`) + handler
  `Fn(&mut Ctx, params) -> Result<Value, RpcError>`;
- i18n: bundles `en.ftl` e `pt-BR.ftl` embutidos;
- `uses_i18n(key)` para chaves usadas só em runtime (contrato).

## Serviços do `Ctx`

- `tasks.spawn(work, done)`: trabalho em thread, callback de volta na thread da
  interface via `Runtime::poll_tasks`;
- `store()`: JSON por módulo em `<data>/modules/<id>.json`, gravação atômica e
  quarentena de arquivo corrompido;
- `settings()` / `setting(key)` / `set_setting()`: leitura efetiva com
  precedência `projeto > usuário > padrão`, validação contra o schema e
  persistência global em `<data>/settings.toml` numa thread de escrita dedicada;
- ajustes por projeto (`<projeto>\.forja\ajustes.toml`):
  - `set_active_project(project_dir)` / `reload_project_settings(project_dir)` /
    `active_project()`: troca o projeto ativo, recarrega `.forja\ajustes.toml`,
    ignora chaves fora da lista segura (`PROJECT_SAFE_KEYS`) ou arquivos/valores
    inválidos com aviso no log (caindo para o valor do usuário), incrementa
    `settings_generation()` e emite `SettingsChanged`;
  - `set_project_setting(key, value)` / `unset_project_setting(key)`: grava ou
    remove uma sobreposição do projeto ativo de forma assíncrona e atômica
    (`write_atomic_bytes` na thread `SettingsWriter`), recusando chaves fora da
    lista segura com `KernelError::UnsafeProjectSetting(key)` (e
    `KernelError::NoActiveProject` quando não há projeto ativo);
  - `user_setting(key)`, `project_setting(key)` e `setting_source(key)`
    (`SettingSource::{Default, User, Project}`) inspecionam cada camada;
- `keymap()`: padrões dos módulos < pacotes em `~/.forja/keymaps/*.toml` <
  `~/.forja/keymap.toml` < `<data>/keymap-platform.toml`, com detecção de
  conflito por escopo e gramática `Ctrl+Shift+K`;
- `t(key)` / `t_args(key, args)`: Fluent com fallback para inglês e depois para
  a própria chave;
- `emit(event)`, `run_command(id, args)`, `setting(key)`, `registry()`.

## Decisões

- Chaves i18n usam pontos no código (`hello.tab.title`), mas o Fluent não aceita
  pontos em ids. O kernel troca `.` por `-` ao ler o bundle (`hello-tab-title`).
- O kernel não conhece egui; `ui_contract` só existe com `--features egui`.
- Testes sempre passam `data_dir` e `user_dir` explícitos; nada toca
  `%APPDATA%` nem `~/.forja`.

## Como testar

```sh
cargo test -p forja-kernel
cargo test -p forja-kernel --features egui
cargo clippy -p forja-kernel --all-targets -- -D warnings
```

Os testes de contrato que rodam sobre todos os módulos linkados ficam no shell
(`crates/shell/tests/contracts.rs`).
