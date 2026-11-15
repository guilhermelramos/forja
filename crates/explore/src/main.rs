//! CLI entry point for `forja-explore`.

use forja_explore::outcome::Outcome;
use forja_explore::replay;
use forja_explore::runner::{self, ExploreConfig};
use std::path::PathBuf;

enum Mode {
    Help,
    Explore(ExploreConfig),
    Replay {
        dir: PathBuf,
        app: PathBuf,
        locale: String,
    },
}

fn main() {
    let outcome = match parse_args() {
        Ok(Mode::Help) => {
            print_help();
            Outcome::Help
        }
        Ok(Mode::Replay { dir, app, locale }) => {
            Outcome::Replay(replay::run_directory(&dir, &app, &locale))
        }
        Ok(Mode::Explore(config)) => Outcome::Explore(runner::run(&config)),
        Err(err) => Outcome::Usage(err),
    };
    match &outcome {
        Outcome::Explore(Ok(summary)) => {
            println!(
                "explore: {} actions, {} screens, {} elements, {} keybinds, {} findings ({} crashes/hangs, {} restarts)",
                summary.actions,
                summary.screens,
                summary.elements,
                summary.keybinds,
                summary.findings,
                summary.crashes_or_hangs,
                summary.restarts
            );
        }
        Outcome::Explore(Err(err)) | Outcome::Replay(Err(err)) => {
            eprintln!("forja-explore: {err}");
        }
        Outcome::Usage(err) => {
            eprintln!("forja-explore: {err}");
            eprintln!("Use `forja-explore --help` para ver as opções.");
        }
        Outcome::Help | Outcome::Replay(Ok(())) => {}
    }
    std::process::exit(outcome.exit_code());
}

fn parse_args() -> Result<Mode, String> {
    let mut app: Option<PathBuf> = None;
    let mut pipe: Option<String> = None;
    let mut minutes: Option<u64> = None;
    let mut actions: Option<usize> = None;
    let mut seed: u64 = 1;
    let mut locale = "pt-BR".to_owned();
    let mut out_dir = PathBuf::from("explore-out");
    let mut findings_path: Option<PathBuf> = None;
    let mut trail_path: Option<PathBuf> = None;
    let mut pick_dir: Option<PathBuf> = None;
    let mut repo_root: Option<PathBuf> = None;
    let mut replay_dir: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Mode::Help),
            "--app" => {
                app = Some(PathBuf::from(next_val(&mut args, "--app")?));
            }
            "--pipe" => {
                pipe = Some(next_val(&mut args, "--pipe")?);
            }
            "--minutes" => {
                let raw = next_val(&mut args, "--minutes")?;
                minutes = Some(
                    raw.parse::<u64>()
                        .map_err(|_| format!("invalid --minutes value `{raw}`"))?,
                );
            }
            "--actions" => {
                let raw = next_val(&mut args, "--actions")?;
                actions = Some(
                    raw.parse::<usize>()
                        .map_err(|_| format!("invalid --actions value `{raw}`"))?,
                );
            }
            "--seed" => {
                let raw = next_val(&mut args, "--seed")?;
                seed = raw
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --seed value `{raw}`"))?;
            }
            "--locale" => {
                locale = next_val(&mut args, "--locale")?;
            }
            "--out" => {
                out_dir = PathBuf::from(next_val(&mut args, "--out")?);
            }
            "--findings" => {
                findings_path = Some(PathBuf::from(next_val(&mut args, "--findings")?));
            }
            "--trail" => {
                trail_path = Some(PathBuf::from(next_val(&mut args, "--trail")?));
            }
            "--pick-dir" => {
                pick_dir = Some(PathBuf::from(next_val(&mut args, "--pick-dir")?));
            }
            "--repo-root" => {
                repo_root = Some(PathBuf::from(next_val(&mut args, "--repo-root")?));
            }
            "--replay" => {
                replay_dir = Some(PathBuf::from(next_val(&mut args, "--replay")?));
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }

    if let Some(dir) = replay_dir {
        let app_exe = app.unwrap_or_else(default_app_exe);
        return Ok(Mode::Replay {
            dir,
            app: app_exe,
            locale,
        });
    }

    Ok(Mode::Explore(ExploreConfig {
        app,
        pipe,
        minutes,
        actions,
        seed,
        locale,
        out_dir,
        findings_path,
        trail_path,
        pick_dir,
        repo_root,
    }))
}

fn next_val(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} requires a value"))
}

fn default_app_exe() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("forja-app.exe");
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from("forja-app.exe")
}

fn print_help() {
    println!(
        "forja-explore — explorador sem IA e executor de regressões\n\
         \n\
         Uso:\n  forja-explore [opções]\n\
         \n\
         Opções:\n\
           --app <exe>        abre e gerencia forja-app.exe com dados isolados\n\
           --pipe <nome>      pipe RPC quando o app já está aberto (ou FORJA_PIPE)\n\
           --minutes <N>      duração máxima da exploração em minutos\n\
           --actions <N>      número máximo de ações\n\
           --seed <N>         semente do RNG (padrão 1)\n\
           --locale <tag>     pt-BR (padrão) ou en\n\
           --out <dir>        pasta de saída (padrão explore-out)\n\
           --findings <arq>   arquivo JSONL de achados (padrão <out>/findings.jsonl)\n\
           --trail <arq>      arquivo JSONL da trilha de ações (padrão <out>/trail.jsonl)\n\
           --pick-dir <dir>   pasta de exemplo para FORJA_TEST_PICK_DIR\n\
           --repo-root <dir>  raiz do repositório para ler todos os .ftl\n\
           --replay <dir>     executa regressões gravadas (*.json) em app limpo\n\
           -h, --help         mostra esta ajuda"
    );
}
