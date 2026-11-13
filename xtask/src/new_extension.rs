//! `cargo xtask new-extension <name>`.
//!
//! Gera uma extensão Lua que já funciona: manifesto, `init.lua` com comando,
//! painel e configuração, i18n e README. Nada precisa ser recompilado.

use crate::util::{pascal_case, repo_root, validate_extension_name, write};

pub fn run(name: &str) -> Result<(), String> {
    validate_extension_name(name)?;
    let root = repo_root();
    let dir = root.join("extensions").join(name);
    if dir.exists() {
        return Err(format!(
            "{} already exists; choose another name or remove it",
            dir.display()
        ));
    }
    let display = pascal_case(name);
    let files = [
        (
            "extension.toml",
            format!(
                r#"# Extensão da Forja. Copie esta pasta para ~/.forja/extensions/ e aprove.
id = "{name}"
name = {{ pt-BR = "{display}", en = "{display}" }}
version = "0.1.0"
author = "Forja"
description = {{ pt-BR = "Extensão de exemplo {display}.", en = "{display} example extension." }}
api = "1"
entry = "init.lua"

[capabilities]
commands = true
"ui.panel" = true
settings = true
"#,
            ),
        ),
        (
            "init.lua",
            format!(
                r#"-- Extensão `{name}` da Forja.
-- O host entrega `forja.*`; nada aqui abre arquivos ou processos diretamente.

forja.settings.schema({{
  {{
    name = "greeting",
    type = "text",
    default = "Olá",
    label = {{ ["pt-BR"] = "Saudação", en = "Greeting" }},
  }},
}})

local function render(message)
  forja.ui.panel({{
    id = "hello",
    title = {{ ["pt-BR"] = "{display}", en = "{display}" }},
    slot = "right_panel",
    tree = {{
      forja.ui.text({{ value = message, style = "title" }}),
      forja.ui.button({{
        id = "again",
        label = {{ ["pt-BR"] = "De novo", en = "Again" }},
        action = function(ev)
          render(forja.settings.get("greeting") .. "!")
        end,
      }}),
    }},
  }})
end

forja.commands.register({{
  id = "greet",
  title = {{ ["pt-BR"] = "Cumprimentar", en = "Greet" }},
  arguments = {{ {{ name = "name", type = "text", required = false, default = "mundo" }} }},
  run = function(args, ctx)
    local message = forja.settings.get("greeting") .. ", " .. args.name .. "!"
    render(message)
    return {{ message = message }}
  end,
}})

render(forja.settings.get("greeting") .. "!")
"#,
            ),
        ),
        (
            "i18n/pt-BR.toml",
            "greeting = \"Olá\"\n".to_owned(),
        ),
        (
            "i18n/en.toml",
            "greeting = \"Hello\"\n".to_owned(),
        ),
        (
            "README.md",
            format!(
                r#"# {display}

Extensão de exemplo criada por `cargo xtask new-extension {name}`.

- `extension.toml` declara id, nome, versão, entrada e capacidades.
- `init.lua` registra um comando, um painel e uma configuração.
- `i18n/` guarda os textos da extensão.

Para usar, copie a pasta para `~/.forja/extensions/{name}/` e aprove na página
Extensões. Para desenvolver, ligue `extension.dev.watch`: mudanças de código
sem mudança de capacidades recarregam sozinhas.
"#,
            ),
        ),
    ];
    for (relative, contents) in files {
        write(&dir.join(relative), &contents)?;
    }
    println!("forja: extension `{name}` created in {}", dir.display());
    println!("forja: copy it to ~/.forja/extensions/{name}/ and approve it in the Extensions page");
    Ok(())
}
