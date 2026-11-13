# Extensões da Forja

A Forja tem duas camadas de extensão:

| Camada | O que é | Precisa compilar? | Precisa aprovar? |
|---|---|---|---|
| conteúdo | comandos, temas, modelos, receitas e atalhos em arquivos | não | só o conteúdo de projeto que executa algo |
| extensão Lua | uma pasta com `extension.toml` e `init.lua` | não | sim, pelo hash da pasta inteira |

Este guia é para pessoas e para modelos. O contrato completo de formato e API
está em `design/extensions.md`; aqui ficam o caminho curto e as receitas.

## 1. Onde os arquivos moram

```
~/.forja/                          usuário
├── keymap.toml                    suas trocas de atalho
├── commands/*.md                  comandos da Bigorna e do /comandos
├── helpers/*.toml                 perfis de ajudante
├── skills/<nome>/SKILL.md         skills
├── themes/*.toml                  temas de terminal e de interface
├── keymaps/*.toml                 pacotes de atalhos
├── recipes/*.toml                 receitas do terminal
├── templates/*.md                 modelos de arquivo
└── extensions/<id>/               extensões Lua
    ├── extension.toml
    ├── init.lua
    ├── lib/*.lua
    └── i18n/<locale>.toml
```

`<projeto>/.forja/` usa exatamente o mesmo formato. A camada de projeto
substitui a do usuário, que substitui a embutida em `content/`.

O que a Forja grava (aprovações, logs, store) fica em
`%APPDATA%\Forja\` ou em `FORJA_DATA_DIR`, nunca no repositório.

## 2. Conteúdo

### 2.1 Comando `.md`

```markdown
+++
title = { pt-BR = "Revisar a bancada", en = "Review the bench" }
description = "Pede uma revisão do diff."
scope = ["anvil", "chat"]
run = "helper"
helper = "claude-code"
permissions = "ask"
context = ["diff", "bench"]
arguments = [ { name = "focus", required = false, default = "all" } ]
+++
Revise o diff com foco em {{focus}}.
Branch: {{bench.branch}}
```

- O frontmatter é TOML entre `+++`.
- O id é `cmd.<nome-do-arquivo>`.
- `run` aceita `helper`, `recipe` ou `terminal`.
- `permissions` aceita `ask`, `accept-edits`, `automatic` ou `full`.
- `{{argumento}}` vem de `arguments`; `{{bench.branch}}` e `{{bench.name}}`
  vêm do contexto; `{{diff.summary}}` vem de `context = ["diff"]`.
- Comando de projeto com `run = "terminal"`, `permissions = "automatic"` ou
  `"full"` só executa depois de aprovado no `trust`. Ao tentar rodar sem
  aprovação, a Forja abre o pedido de confirmação daquele arquivo; a próxima
  execução já passa.

O comando aparece na Bigorna (fonte `content.commands`) e no `/comandos` do
chat. O contrato é `content.commands.list`; `content.run {id, arguments}`
executa.

### 2.2 Tema

```toml
id = "brasa-noturna"
name = "Brasa noturna"
background = "#1B1714"
foreground = "#F3E9DC"
cursor = "#E8743B"
selection = "#3A2E26"
selection_foreground = "#F3E9DC"

[ansi]
black = "#161311"
red = "#C75B4A"
# ... as 16 cores ANSI são obrigatórias
```

O terminal consulta `content.themes.list` e aplica os temas `kind =
"terminal"`. A interface aceita um tema com `mode`/`[color]` e o shell consulta
o mesmo contrato.

### 2.3 Modelo de arquivo

```markdown
+++
name = "Descrição de PR"
use = "pr"
+++
## Resumo
{{summary}}

## Como testar
{{tests}}
```

O diálogo **Novo arquivo** do painel Arquivos lista os modelos e grava o corpo
escolhido. `content.templates.render {id, fields}` troca `{{campo}}` pelos
valores informados.

### 2.4 Receita

```toml
[[recipe]]
id = "lint"
title = { pt-BR = "Lint do workspace", en = "Workspace lint" }
command = "cargo clippy --workspace --all-targets -- -D warnings"
scope = "project"
enter = false
directory = "."
```

O terminal consulta `content.recipes.list`. Receita de projeto só roda depois
do `trust`; rodar sem aprovação abre o pedido daquele arquivo.

### 2.5 Atalhos

```toml
[[binding]]
id = "cmd.review-bench"
key = "Ctrl+Alt+R"
scope = "global"
title = { pt-BR = "Revisar a bancada", en = "Review the bench" }
```

O kernel lê `~/.forja/keymaps/*.toml`, `<projeto>/.forja/keymaps/*.toml` e
`~/.forja/keymap.toml` na inicialização e de novo quando o módulo `content`
carrega a bancada; o arquivo do usuário vence os pacotes, e o pacote de projeto
vence o do usuário. Um pacote de projeto só pode ligar comandos `cmd.*` e
`ext.*`: tentativas em ações da Forja são ignoradas com aviso.

## 3. Extensão Lua

### 3.1 Manifesto

```toml
id = "todo-counter"
name = { pt-BR = "Contador de TODOs", en = "TODO counter" }
version = "1.0.0"
author = "Forja"
description = { pt-BR = "Conta TODO e FIXME.", en = "Counts TODO and FIXME." }
api = "1"
entry = "init.lua"

[capabilities]
commands = true
"ui.panel" = true
settings = true
events = ["project.opened"]
"fs.read" = ["**/*.rs", "**/*.md"]
```

| Capacidade | Forma | Libera |
|---|---|---|
| `commands` | bool | `forja.commands.register` |
| `"ui.panel"` | bool | `forja.ui.panel`, `remove_panel` |
| `settings` | bool | `forja.settings.*` |
| `events` | lista | `forja.on` só para esses nomes |
| `"terminal.send"` | bool | `forja.terminal.*` |
| `"git.read"` | bool | `forja.git.*` |
| `"fs.read"` | globs | `forja.fs.list/read/exists` |
| `"fs.write"` | globs | `forja.fs.write` |
| `http` | hosts | `forja.http.get/post` |
| `"ui.open_url"` | hosts | `forja.ui.open_url` |

Sem a capacidade, a função existe mas responde
`[capability_denied] ...`. `forja.log`, `forja.notify`, `forja.i18n`,
`forja.store`, `forja.time`, `forja.project`, `forja.bench` e `forja.load`
estão sempre disponíveis.

### 3.2 `init.lua`

```lua
forja.settings.schema({
  {
    name = "max_files",
    type = "number",
    default = 300,
    range = { min = 10, max = 2000 },
    label = { ["pt-BR"] = "Arquivos analisados", en = "Files scanned" },
  },
})

forja.commands.register({
  id = "count",
  title = { ["pt-BR"] = "Contar TODOs", en = "Count TODOs" },
  arguments = { { name = "folder", type = "text", required = false, default = "." } },
  run = function(args, ctx)
    forja.ui.panel({
      id = "todos",
      title = "TODOs",
      slot = "right_panel",
      tree = {
        forja.ui.text({ value = "pronto", style = "title" }),
        forja.ui.button({
          id = "refresh",
          label = { ["pt-BR"] = "Atualizar", en = "Refresh" },
          action = function(ev) end,
        }),
      },
    })
    return { message = "contado" }
  end,
})

forja.on("project.opened", function(ev)
  forja.log.info("projeto " .. ev.name)
end)
```

Regras:

- `forja.commands.register`, `forja.on` e `forja.settings.schema` só valem
  durante o `init.lua`.
- O id do comando vira `ext.<extensão>.<id>`.
- `forja.ui.panel` pode ser chamado em `init.lua`, num comando ou num evento.
- Um painel com o mesmo `id` substitui a árvore inteira.
- Nós interativos precisam de `id`. Limites: 500 nós, profundidade 4, 1000
  linhas por tabela, 1000 itens por lista.

### 3.3 Árvore de painel

| Nó | Campos |
|---|---|
| `text` | `value`, `style` (`normal`, `title`, `secondary`, `warning`, `error`, `code`) |
| `button` | `id`, `label`, `primary?`, `disabled?`, `action` |
| `list` | `id`, `items` (`{id, label, detail?}`), `action` |
| `table` | `id`, `columns` (`{key, label, width?}`), `rows` (`{id, ...}`), `action?` |
| `form` | `id`, `fields`, `submit` (`{label, action}`) |
| `field` | `id`, `type` (`text`, `number`, `choice`, `boolean`, `path`), `label`, `value?`, `options?` |
| `divider` | — |
| `group` | `id?`, `title?`, `children` |

Construtores: `forja.ui.text{...}`, `forja.ui.button{...}`, etc. O evento da
ação traz `node`, `kind` (`click`, `change`, `select`, `submit`) e o campo
correspondente (`item`, `row`, `value`, `values`).

### 3.4 Outras funções

```lua
forja.terminal.list()
forja.terminal.send("current", "npm test", { enter = true })
forja.git.status(); forja.git.branch(); forja.git.remote_url("origin")
forja.git.log({ limit = 50, since = "2026-11-12" }); forja.git.diff_stat()
forja.fs.list("**/*.rs"); forja.fs.read("src/main.rs")
forja.fs.write("notes.md", "texto")            -- fs.write
local r = forja.http.get("https://api.github.com/x", { timeout_ms = 5000 })
forja.ui.open_url("https://github.com/owner/repo/compare/main...feature")
forja.notify({ title = "Pronto", text = "acabou", level = "info" })
forja.store.set("last", { at = forja.time.now() })
forja.log.info("mensagem")
forja.load("lib/helpers.lua")
```

## 4. Aprovação, hash e desenvolvimento

1. A Forja calcula SHA-256 de todos os arquivos da pasta, em ordem de caminho.
2. Sem aprovação, o estado é `pending` e nada roda.
3. **Aprovar** mostra cada capacidade e o hash; a decisão vale só para aquele
   hash.
4. Qualquer byte novo muda o hash: o estado vira `changed` e a extensão para.
5. **Revogar** apaga a decisão e descarrega a extensão.
6. `extension.dev.watch` ligado, extensão em `~/.forja/extensions/` e as mesmas
   capacidades: o novo código é aprovado sozinho e a página mostra o selo
   **recarga de desenvolvimento**. Projeto nunca entra nesse modo.
7. Três erros em 60 s suspendem a extensão; os painéis somem até recarregar.

## 5. Sandbox e limites

- Um estado Lua e uma thread por extensão.
- Bibliotecas: `coroutine`, `table`, `string`, `utf8`, `math`.
- Sem `os`, `io`, `package`, `require`, `debug`, `dofile`, `loadfile`,
  `string.dump` nem bytecode. `load` só aceita texto.
- Memória: `extension.lua.memory_mb` (64 MB por padrão).
- CPU por chamada: `extension.lua.timeout_ms` (1000 ms por padrão), com hook de
  instruções; um laço infinito é interrompido.
- Texto lido/escrito: 1 MB. HTTP: 2 MB e 30 pedidos por minuto. Terminal:
  4 KB e 10 envios por minuto. Store: 1 MB.
- `forja.http` só HTTPS, host exato aprovado, sem `Authorization`/`Cookie`.
- `forja.ui.open_url` só a partir de uma ação do usuário.
- `extension.sandbox.mode` em `ask` transforma cada capacidade negada num aviso
  visível pedindo para editar o manifesto e aprovar de novo; `strict` só registra
  o erro no log.
- Caminhos sempre negados: `.git/**`, `.env`, `.env.*`, `*.pem`, `*.key`;
  escrita também nega `.forja/**`.

## 6. Receitas curtas

### Criar uma extensão

```sh
cargo xtask new-extension minha-extensao
```

A pasta `extensions/minha-extensao/` já tem comando, painel e configuração.
Copie para `~/.forja/extensions/minha-extensao/`, abra **Extensões**, aprove e
rode o comando na Bigorna. Nada foi recompilado.

### Desenvolver com recarga

1. Ligue `extension.dev.watch` nos Ajustes.
2. Edite `init.lua`.
3. A página mostra `recarga de desenvolvimento` e a extensão volta ativa sem
   pedir aprovação de novo, desde que as capacidades não mudem.

### Adicionar um comando de conteúdo

1. Crie `~/.forja/commands/meu-comando.md` com `title` e corpo.
2. Abra a Bigorna: a linha aparece na hora.
3. No chat, digite `/meu-comando` para ver o nome na lista.

### Diagnosticar

- **Extensões** mostra estado, capacidades, hash, erros e as últimas linhas do
  log.
- Conteúdo inválido aparece em **Ajustes > Conteúdo**, com arquivo e mensagem.
- O log fica em `%APPDATA%\Forja\logs\extensions\<id>.log`.

## 7. Contratos entre módulos

| Comando | Retorno |
|---|---|
| `content.commands.list {scope?, project_path?}` | `{revision, commands[], diagnostics[]}` |
| `content.run {id, arguments?}` | `{id, run, warnings, result}` |
| `content.themes.list {project_path?}` | `{revision, themes[]}` |
| `content.themes.ui {mode?}` | `{revision, toml?}`: tema de interface |
| `content.recipes.list {project_path?}` | `{revision, recipes[]}` |
| `content.templates.list` | `{templates[]}` |
| `content.templates.render {id, fields?}` | `{id, text}` |
| `extension.list` | `{revision, extensions[]}` |
| `extension.run {extension, command, arguments?}` | valor devolvido pelo Lua |
| `extension.action {extension, action, event}` | valor devolvido pelo Lua |
| `extension.approve {id}` | `{approved}` ou `{pending, request_id}` |
| `extension.revoke {id}` | `{revoked}` |
| `extension.reload {id}` | `{reloaded}` |
| `extension.rescan` | `{rescanned}` |

Eventos entregues às extensões: `app.ready`, `project.opened`,
`bench.selected`, `bench.created`, `helper.status`, `git.changed` e
`settings.changed`.
