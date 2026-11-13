# Módulos da Forja

Uma funcionalidade é **uma pasta**: `crates/modules/<nome>/`. O crate se
registra sozinho no kernel, e o shell não conhece nomes de módulos.

Este guia usa o módulo `hello` como exemplo; ele é gerado por
`cargo xtask new-module hello` e vive em `crates/modules/hello`.

## 1. Criar um módulo

```sh
cargo xtask new-module hello
```

A ferramenta cria:

```
crates/modules/hello/
├── Cargo.toml
├── MODULE.md
├── i18n/
│   ├── en.ftl
│   └── pt-BR.ftl
├── keymap.toml
└── src/lib.rs
```

E acrescenta **somente** uma linha na região `# forja:modules` de
`crates/shell/Cargo.toml`:

```toml
# forja:modules:start
forja-module-hello = { path = "../modules/hello" }
# forja:modules:end
```

Nenhuma pessoa precisa editar outro arquivo. O nome deve ser `snake_case`
minúsculo (`hello`, `bench_board`). O id do módulo e o prefixo de todos os ids
são esse nome.

## 2. O contrato

```rust
pub trait Module: Send {
    fn manifest(&self) -> ModuleManifest;
    fn register(&mut self, r: &mut Registrar<'_>);
}
```

O `hello` publica a fábrica com a macro:

```rust
#[forja_kernel::module]
static HELLO: fn() -> Box<dyn Module> = || Box::new(HelloModule { state: Shared::new() });
```

A macro adiciona a entrada do `linkme` e exporta `__forja_module()`, usado
pelo build script do shell quando o linker não reúne as seções distribuídas
(Windows GNU). Um módulo por crate.

O manifesto traz `id`, `name_key` (chave i18n), `version` e `depends_on`.
Dependências são ordenadas antes do módulo; ciclos e ids desconhecidos falham
na inicialização.

## 3. O que registrar

Dentro de `register()`:

```rust
r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
r.keymap(include_str!("../keymap.toml"));

r.setting(SettingSpec::new(
    "hello.greeting.style",
    SettingKind::Choice { options: vec!["warm".into(), "plain".into()] },
    SettingValue::Text("warm".into()),
    "setting-hello-greeting-style-label",
    "setting-hello-greeting-style-help",
));

r.command("hello.greet", "hello.command.greet", |cx, args| {
    let message = greet(cx, args.str("name"), &state)?;
    Ok(json!({ "message": message }))
});

r.rpc(
    Method::new("hello.greet", "hello.method.greet.description")
        .effect(MethodEffect::Read)
        .idempotent(true)
        .param(ParamSchema::required("name", ParamKind::String)
            .describe("Nome de quem recebe o cumprimento.")
            .alias("nome"))
        .result(ResultSchema::new("hello.method.greet.result").schema(json!({
            "type": "object",
            "properties": { "message": { "type": "string" } },
            "required": ["message"]
        }))),
    |cx, params| { /* ... */ Ok(json!({ "message": message })) },
);

r.view(Slot::BenchTab, "hello.tab", "hello.tab.title", 100, move || {
    Box::new(HelloView { state: view_state.clone() })
});
```

### Comandos
- Id `<módulo>.<ação>` em inglês.
- Comandos de **papel** (ex.: `file.open`, atendido por qualquer editor) usam um namespace de contrato: o módulo chama `r.contract_namespace("file")` e então pode registrar `file.*` (comandos e RPC). Só um módulo pode servir cada namespace; ele não pode coincidir com um id de módulo. Os namespaces de contrato estão em `design/contracts.md`.
- Título por chave i18n.
- `Availability::{Always, NeedsProject, NeedsBench}`.
- Handler `Fn(&mut Ctx, Args) -> Result<Value>`; use `Args::str`, `i64`,
  `bool` e `required_str`.

### Views
Slots: `rail`, `sidebar`, `bench_tab`, `right_panel`, `status_bar`,
`settings_section`, `modal`, `anvil_source`, `top_bar`. O registry guarda `Box<dyn Any>`;
o shell faz o downcast para `ui_contract::View`:

```rust
impl View for HelloView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ui.heading(cx.t("hello.tab.title"));
        if ui.button(cx.t("hello.view.button")).clicked() {
            if let Err(error) = cx.run_command("hello.greet", Args::new()) {
                eprintln!("{error}");
            }
        }
    }
}
```

`ui_contract::egui` é reexportado pelo kernel; o módulo não depende de eframe
diretamente.

Um item no slot `rail` é uma página inteira. O shell desenha o ícone no trilho
(glifo com `r.view_glyph(id, "▣")` ou PNG com `r.view_png(id, include_bytes!(...))`,
chamados depois de `r.view`) e o tooltip sai da chave de título. O clique mostra
`ui()` no centro, no lugar das abas da bancada, até outro item do trilho, o
mesmo ícone de novo, ou o logo (volta à bancada).

O slot `top_bar` fica na barra superior, entre o nome do projeto e os controles
da janela. O shell esconde o botão próprio de "Achar arquivo" quando alguma view
ocupa esse slot.

Um modal só aparece depois de `cx.open_modal(view_id)` ou
`cx.open_modal_with(view_id, payload)`. `cx.modal_payload` lê o JSON.
`cx.close_modal` esconde. `poll` roda todo frame, mesmo fechado, e é o lugar
para abrir o modal sozinho (primeiro uso, toast). `cx.viewport_focused()` diz
se a janela estava em foco no começo do frame.

Uma fonte da Bigorna que implementa `provides_anvil_hits` recebe a consulta em
`anvil_hits` e devolve `AnvilHit { title, detail, icon_glyph, score, command, args }`.
`detail` é o subtítulo. O shell mistura essas linhas com as dele, ranqueia de
novo e mostra o atalho do comando quando existe. Não desenha `ui()` dessa fonte
dentro da Bigorna. `cx.anvil_query()` repete o texto.

O separador entre painéis é uma linha de 1 px. A faixa que aceita o arrasto é
mais larga e não é pintada.

### Configurações
Chave com exatamente três segmentos `<módulo>.<grupo>.<nome>`. O formato de
referência do produto é `design/settings.toml`. Tipos implementados:
`Bool`, `Integer { min, max }`, `Number { min, max }`, `Text { max_len }`,
`Choice { options }`, `Path`, `List { item }` e `Color` (`#RRGGBB`). O shell
gera os controles, agrupa por módulo e grava em `%APPDATA%\Forja\settings.toml`
depois de validar contra o schema.
Valores TOML de chaves ainda não registradas são preservados nas gravações,
inclusive quando uma opção está adiada; se uma estrutura desconhecida conflitar
com uma chave registrada, a gravação falha em vez de sobrescrever esses dados.

### Atalhos
`keymap.toml` usa os campos do catálogo (`id`, `key`, `scope`):

```toml
[[binding]]
id = "hello.greet"
key = "Ctrl+Alt+H"
scope = "hello"
```

Precedência: padrão do módulo < pacotes em `~/.forja/keymaps/*.toml` <
`~/.forja/keymap.toml` < `<data>/keymap-platform.toml`. O parser também aceita
`command` e `chord`. Conflito é o mesmo acorde em comandos diferentes **no
mesmo escopo**. `title_key` é opcional e aponta para a chave i18n da ação.

### i18n
Código usa chaves com pontos (`hello.tab.title`); o Fluent não aceita pontos
em ids, então o kernel troca `.` por `-` (`hello-tab-title`). Toda chave usada
só em runtime deve ser anunciada:

```rust
for key in ["hello.greeting.warm", "hello.greeting.plain", "hello.default_name"] {
    r.uses_i18n(key);
}
```

Chaves de comando, view, setting e manifesto são coletadas automaticamente.
O contrato exige a chave em **en** e **pt-BR**.

### Eventos
```rust
// evento próprio do módulo (nome com o prefixo do módulo)
r.event::<Greeted>("hello.greeted");
// evento de contrato entre módulos (tipo em forja-contracts); publicador e assinante declaram
r.contract_event::<BenchActivated>(forja_contracts::names::BENCH_ACTIVATED);
r.subscribe(|cx, event: &BenchActivated| { /* ... */ });
```
A ordem de registro entre publicador e assinante não importa: `subscribe` é validado no fim.
`cx.emit(&event)` entrega aos assinantes na thread da interface.

### RPC e CLI
O schema do método alimenta `meta.methods`; o CLI `forja` monta os subcomandos
a partir dele. `Method` carrega `description_key`, parâmetros, resultado e as
anotações de segurança: `effect` (`read`, `write`, `destructive`), `network`,
`idempotent` e `exposed_mcp`. Métodos de leitura são expostos ao MCP por
padrão; escrita e destruição exigem opt-in explícito.

```toml
[[binding]]
id = "hello.greet"
key = "Ctrl+Alt+H"
scope = "hello"
title_key = "hello.command.greet"
```

O exemplo responde ao CLI quando o módulo está ligado ao app. O `hello` é só o molde e não entra na região `# forja:modules` de `crates/shell/Cargo.toml`, então, no app atual, use um método de um módulo ligado (como `forja terminal list`):

```sh
forja hello greet --nome Ana
forja --json hello greet --name Ana
```

Aliases de parâmetro (`nome`) existem para os exemplos em português; o nome
canônico continua em inglês.

## 4. Serviços do `Ctx`

- `t` / `t_args`: Fluent com fallback para inglês e depois para a chave.
- `run_command(id, args)`.
- `setting(key)` / `set_setting(key, value)`.
- `spawn(work, done)`: trabalho em thread; `done` volta na thread da interface.
- `store()`: JSON atômico em `%APPDATA%\Forja\modules\<id>.json`, com
  quarentena de arquivo corrompido.
- `emit(event)`.
- `registry()`: metadados somente leitura.

## 5. Testes e qualidade

O crate gerado inclui um teste que:

1. monta um `Runtime` com pastas temporárias;
2. roda `contract::assert_ok` (ids, i18n en/pt-BR, defaults, conflitos);
3. chama `hello.greet` por RPC e por comando.

O shell também tem um teste de contrato sobre **todos** os módulos linkados
(`crates/shell/src/main.rs`, `contract_runs_over_every_linked_module`). Esse
teste exige `anvil.open` e uma ida a `meta.methods`. O crate `hello` continua
sendo o molde do `xtask`, mas não entra na região `# forja:modules` do app.

Antes de entregar:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask provenance
```

### Capturas de tela sem janela

`forja-app --screenshot <png>` renderiza uma tela e fecha. Os flags abaixo escolhem o
que aparece. Todos são só para captura: não mudam o que o app salva.

| Flag | O que faz |
|---|---|
| `--screenshot <png>` | grava o PNG e fecha |
| `--slot <nome>` | recorta: `all`, `top`, `rail`, `sidebar`, `bench`, `right`, `status`, `settings`, `anvil` |
| `--rail <id>` | abre a página do trilho com esse id, por exemplo `board.page`, `skill.page`, `archive.tab` |
| `--settings <módulo>` | abre os Ajustes só com a seção desse módulo, por exemplo `remote`; se o módulo tem seção própria, ela substitui as linhas dele |
| `--modal <id>` | mostra a camada modal dessa view (`Slot::Modal`) mesmo fechada, com o payload vazio, por exemplo `voice.overlay`; id desconhecido imprime um aviso e segue |
| `--locale <tag>`, `--theme <modo>`, `--density <modo>` | idioma, tema e densidade desta janela |
| `--open <painel>` | `anvil`, `onboarding`, `toast` ou `rail` (primeira página do trilho) |

Dados de exemplo entram com `FORJA_FIXTURES=1`. A janela de voz de exemplo só abre com
`FORJA_FIXTURES_VOICE=1`. Exemplo para a página de Skills em inglês, tema claro:

```sh
FORJA_FIXTURES=1 cargo run -p forja-app -- --screenshot shots/skills-en-light.png --slot all --rail skill.page --locale en --theme light
```

Camadas modais usam `--modal <id>`, que força a camada a aparecer sem abrir o diálogo de
verdade. Ela mostra o que a view desenha com o estado atual; para ver conteúdo de exemplo,
use os fixtures. Exemplo para a janela de voz:

```sh
FORJA_FIXTURES=1 FORJA_FIXTURES_VOICE=1 cargo run -p forja-app -- --screenshot shots/modal-voice-overlay.png --slot all --modal voice.overlay --locale pt-BR --theme dark
```

## 6. Regras de autoria

- Nada de copiar ids, textos, atalhos ou estrutura de outro produto.
- Identificadores em inglês; interface só em Fluent pt-BR + en.
- Arquivos com até ~600 linhas; o crate do módulo com até ~5k linhas.
- Sem disco, rede ou processo na thread da interface: use `Ctx::spawn`.
- Não escreva fora da pasta do módulo. Se precisar do kernel, proponha uma
  tarefa separada.
