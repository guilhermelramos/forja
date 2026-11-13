//! Forja repository tasks.
//!
//! ```text
//! cargo xtask new-module <name>
//! cargo xtask new-extension <name>
//! cargo xtask provenance
//! cargo xtask catalog
//! cargo xtask package [--skip-build]
//! cargo xtask installer [--skip-build]
//! ```

mod catalog;
mod installer;
mod new_extension;
mod new_module;
mod package;
mod provenance;
mod util;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command, name] if command == "new-module" => new_module::run(name),
        [command, name] if command == "new-extension" => new_extension::run(name),
        [command] if command == "provenance" => provenance::run(),
        [command] if command == "catalog" => catalog::run(),
        [command] if command == "package" => package::run(false),
        [command, flag] if command == "package" && flag == "--skip-build" => package::run(true),
        [command] if command == "installer" => installer::run(false),
        [command, flag] if command == "installer" && flag == "--skip-build" => installer::run(true),
        _ => {
            print_help();
            std::process::exit(if args.is_empty() { 0 } else { 2 });
        }
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "Forja xtask\n\
         \n\
         Uso:\n\
           cargo xtask new-module <nome>     cria crates/modules/<nome> e vincula ao shell\n\
           cargo xtask new-extension <nome>  cria extensions/<nome> (carregador na Fase D)\n\
           cargo xtask provenance            recusa termos da lista de proveniência\n\
           cargo xtask catalog               compara settings e atalhos contra design/*.toml\n\
           cargo xtask package [--skip-build] empacota o release em dist/\n\
           cargo xtask installer [--skip-build] gera dist/Forja-Setup-<versão>.exe (Inno Setup 6)"
    );
}
