//! `forja`: CLI generated from the method catalog served by the app.
//!
//! The app owns the catalog (`meta.methods`); this binary builds subcommands
//! and help from it. No method list lives here.

use forja_protocol::schema::{MethodCatalog, MethodSchema, ParamKind, ParamSchema};
use forja_protocol::ClientError;
use serde_json::{Map, Value};
use std::path::PathBuf;

const EXIT_OK: i32 = 0;
const EXIT_ERROR: i32 = 1;
const EXIT_APP_CLOSED: i32 = 2;

fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut json = false;
    let mut args = Vec::new();
    for arg in raw {
        if arg == "--json" {
            json = true;
        } else {
            args.push(arg);
        }
    }

    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") | Some("ajuda") => {
            print_general_help();
            EXIT_OK
        }
        Some("abrir") => open_app(),
        Some("metodos") => list_methods(json),
        Some(_) => run_method(&args, json),
    }
}

fn list_methods(json: bool) -> i32 {
    let catalog = match fetch_catalog() {
        Ok(catalog) => catalog,
        Err(code) => return code,
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&catalog).unwrap_or_else(|_| "{}".to_owned())
        );
    } else {
        for method in &catalog.methods {
            let description = if method.description.is_empty() {
                &method.description_key
            } else {
                &method.description
            };
            println!("{:<28} [{:<11}] {}", method.name, method.effect.as_str(), description);
        }
    }
    EXIT_OK
}

fn run_method(args: &[String], json: bool) -> i32 {
    let (method_name, rest) = if let Some(dot) = args[0].find('.') {
        if dot > 0 {
            (args[0].clone(), &args[1..])
        } else {
            eprintln!("forja: método inválido `{}`", args[0]);
            return EXIT_ERROR;
        }
    } else {
        let Some(action) = args.get(1) else {
            eprintln!("forja: informe o grupo e a ação (ex.: forja bench list)");
            print_general_help();
            return EXIT_ERROR;
        };
        (format!("{}.{}", args[0], action), &args[2..])
    };

    let catalog = match fetch_catalog() {
        Ok(catalog) => catalog,
        Err(code) => return code,
    };
    let Some(schema) = catalog.find(&method_name).cloned() else {
        eprintln!("forja: método `{method_name}` não existe. Rode `forja metodos`.");
        return EXIT_ERROR;
    };
    if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_method_help(&schema);
        return EXIT_OK;
    }
    let params = match parse_params(&schema, rest) {
        Ok(params) => params,
        Err(error) => {
            eprintln!("forja: {error}");
            print_method_help(&schema);
            return EXIT_ERROR;
        }
    };
    if let Err(error) = schema.check_params(&params) {
        eprintln!("forja: {error}");
        return EXIT_ERROR;
    }
    match forja_protocol::call(&schema.name, Value::Object(params)) {
        Ok(value) => {
            print_result(&value, json);
            EXIT_OK
        }
        Err(error) => report_client_error(error),
    }
}

fn fetch_catalog() -> Result<MethodCatalog, i32> {
    match forja_protocol::call("meta.methods", Value::Object(Map::new())) {
        Ok(value) => serde_json::from_value(value).map_err(|error| {
            eprintln!("forja: catálogo inválido: {error}");
            EXIT_ERROR
        }),
        Err(error) => Err(report_client_error(error)),
    }
}

fn report_client_error(error: ClientError) -> i32 {
    match error {
        ClientError::NotRunning => {
            eprintln!("forja: a Forja não está aberta. Rode `forja abrir`.");
            EXIT_APP_CLOSED
        }
        other => {
            eprintln!("forja: {other}");
            EXIT_ERROR
        }
    }
}

fn parse_params(
    schema: &MethodSchema,
    args: &[String],
) -> Result<Map<String, Value>, String> {
    let mut params = Map::new();
    let mut positionals = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(flag) = arg.strip_prefix("--") {
            let (name, inline) = match flag.split_once('=') {
                Some((name, value)) => (name, Some(value.to_owned())),
                None => (flag, None),
            };
            if name.is_empty() {
                return Err("flag vazia".to_owned());
            }
            let Some(param) = find_param(schema, name) else {
                return Err(format!("flag desconhecida `--{name}`"));
            };
            let value = match inline {
                Some(value) => value,
                None => {
                    if matches!(param.kind, ParamKind::Boolean) {
                        "true".to_owned()
                    } else {
                        index += 1;
                        args.get(index)
                            .cloned()
                            .ok_or_else(|| format!("`--{name}` precisa de um valor"))?
                    }
                }
            };
            params.insert(param.name.clone(), convert(&param.kind, &value)?);
        } else {
            positionals.push(arg.clone());
        }
        index += 1;
    }
    if !positionals.is_empty() {
        let required: Vec<&ParamSchema> = schema
            .params
            .iter()
            .filter(|param| param.required)
            .collect();
        for (position, value) in positionals.iter().enumerate() {
            let Some(param) = required.get(position) else {
                return Err(format!("argumento extra `{value}`"));
            };
            params.insert(param.name.clone(), convert(&param.kind, value)?);
        }
    }
    Ok(params)
}

fn find_param<'a>(schema: &'a MethodSchema, name: &str) -> Option<&'a ParamSchema> {
    schema.params.iter().find(|param| {
        param.name == name || param.aliases.iter().any(|alias| alias == name)
    })
}

fn convert(kind: &ParamKind, value: &str) -> Result<Value, String> {
    match kind {
        ParamKind::String => Ok(Value::String(value.to_owned())),
        ParamKind::Integer => value
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| format!("`{value}` não é inteiro")),
        ParamKind::Number => value
            .parse::<f64>()
            .map(Value::from)
            .map_err(|_| format!("`{value}` não é número")),
        ParamKind::Boolean => match value.to_ascii_lowercase().as_str() {
            "true" | "1" | "sim" => Ok(Value::Bool(true)),
            "false" | "0" | "nao" | "não" => Ok(Value::Bool(false)),
            _ => Err(format!("`{value}` não é booleano")),
        },
        ParamKind::Json => serde_json::from_str(value)
            .map_err(|error| format!("`{value}` não é JSON: {error}")),
    }
}

fn print_result(value: &Value, json: bool) {
    if json {
        println!("{value}");
        return;
    }
    match value {
        Value::String(text) => println!("{text}"),
        other => println!(
            "{}",
            serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())
        ),
    }
}

fn print_method_help(schema: &MethodSchema) {
    println!("{}", schema.name);
    let description = if schema.description.is_empty() {
        &schema.description_key
    } else {
        &schema.description
    };
    println!("  {description}");
    println!(
        "  efeito: {} · rede: {} · idempotente: {} · MCP: {}",
        schema.effect.as_str(),
        yes_no(schema.network),
        yes_no(schema.idempotent),
        yes_no(schema.exposed_mcp)
    );
    if !schema.params.is_empty() {
        println!("\nParâmetros:");
        for param in &schema.params {
            let requirement = if param.required { "obrigatório" } else { "opcional" };
            let aliases = if param.aliases.is_empty() {
                String::new()
            } else {
                format!(" (alias: {})", param.aliases.join(", "))
            };
            println!(
                "  --{:<20} {:<12} {}{}",
                param.name, requirement, param.description, aliases
            );
        }
    }
    if let Some(result) = &schema.result {
        let description = if result.description.is_empty() {
            &result.description_key
        } else {
            &result.description
        };
        println!("\nResultado: {description}");
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "sim" } else { "não" }
}

fn print_general_help() {
    println!(
        "forja — CLI da Forja\n\
         \n\
         Uso:\n  forja <grupo> <ação> [--param valor] [--json]\n  forja metodos [--json]\n  forja abrir\n\
         \n\
         Exemplos:\n  forja hello greet --nome Ana\n  forja metodos\n  forja --json hello greet --nome Ana\n\
         \n\
         Os subcomandos vêm do catálogo do app (meta.methods). Com a Forja fechada,\n\
         use `forja abrir`; os demais comandos saem com código 2."
    );
}

fn open_app() -> i32 {
    match forja_protocol::call("meta.methods", Value::Object(Map::new())) {
        Ok(_) => {
            println!("A Forja já está aberta.");
            return EXIT_OK;
        }
        Err(ClientError::NotRunning) => {}
        Err(error) => eprintln!("forja: não foi possível consultar o app: {error}"),
    }
    let Some(app) = app_executable() else {
        eprintln!("forja: forja-app.exe não encontrado ao lado de forja.exe");
        return EXIT_ERROR;
    };
    match std::process::Command::new(&app).spawn() {
        Ok(_) => {
            println!("Forja iniciada ({}).", app.display());
            EXIT_OK
        }
        Err(error) => {
            eprintln!("forja: não foi possível iniciar {}: {error}", app.display());
            EXIT_ERROR
        }
    }
}

fn app_executable() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    [
        dir.join("forja-app.exe"),
        dir.join("forja-app"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists())
}
