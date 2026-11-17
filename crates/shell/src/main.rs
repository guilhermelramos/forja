//! `forja-app` entry point: window setup, CLI flags and screenshot mode.

// Always a window app (release and the debug builds used by tests): Windows
// never opens a console window for Forja. When started from a terminal, it
// attaches to that terminal so --help and logs still print there.
#![windows_subsystem = "windows"]

mod actions;
mod anvil;
mod anvil_ask;
mod anvil_keys;
mod app;
mod appearance;
mod crash;
mod demo;
mod diagnostics;
mod hosts;
mod icon;
mod layout;
mod overlays;
mod pacer;
mod settings_catalog;
mod settings_controls;
mod settings_page;
mod settings_transfer;
mod rpc;
mod screenshot;
mod shell_module;
mod system;
mod tabs;
mod theme;
mod test_driver;
mod tray;
mod ui_tree;
mod window_geometry;

mod linked_modules {
    include!(concat!(env!("OUT_DIR"), "/modules.rs"));
}

use app::ForjaApp;
use eframe::egui;
use screenshot::ScreenshotJob;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct Flags {
    screenshot: Option<PathBuf>,
    slot: Option<String>,
    no_modules: bool,
    dump_catalog: bool,
    locale: Option<String>,
    theme: Option<String>,
    density: Option<String>,
    open: Option<String>,
    anvil_query: Option<String>,
    rail: Option<String>,
    settings: Option<String>,
    modal: Option<String>,
    ui_tree: Option<PathBuf>,
    bench_tab: Option<String>,
    right_tab: Option<String>,
    test_driver: bool,
    demo: bool,
}

fn main() {
    crash::install_panic_hook();
    attach_parent_console();
    let code = run();
    forja_kernel::logging::flush();
    std::process::exit(code);
}

fn run() -> i32 {
    let flags = match parse_flags() {
        Ok(Some(flags)) => flags,
        Ok(None) => return 0,
        Err(error) => {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Error,
                format!("forja-app: {error}; use `forja-app --help` para ver as opções"),
            );
            return 2;
        }
    };

    if flags.dump_catalog {
        return dump_catalog(&flags);
    }

    if flags.screenshot.is_some() {
        std::env::set_var("FORJA_SCREENSHOT", "1");
    }
    if flags.test_driver {
        // Módulos que leem dados da máquina (import) precisam saber que esta
        // execução é de teste, mesmo antes da janela existir.
        std::env::set_var("FORJA_TEST_DRIVER", "1");
    }
    set_app_user_model_id();

    // Captures and test-driver runs never touch the person's real data (window
    // position, projects, settings) unless a data dir is given explicitly.
    let isolated = flags.screenshot.is_some() || flags.test_driver || flags.demo;
    let isolated_dir =
        std::env::temp_dir().join(format!("forja-screenshot-{}", std::process::id()));
    let data_dir = if isolated && std::env::var_os("FORJA_DATA_DIR").is_none() {
        Some(isolated_dir.clone())
    } else {
        None
    };
    let user_dir = if isolated && std::env::var_os("FORJA_CONTENT_DIR").is_none() {
        Some(isolated_dir.join("user"))
    } else {
        None
    };
    let effective_data_dir = data_dir
        .clone()
        .unwrap_or_else(forja_kernel::paths::default_data_dir);
    crash::configure_data_dir(effective_data_dir.clone());
    if flags.demo && !flags.no_modules {
        // Sample state must exist before the modules read their files.
        if let Err(error) = demo::seed(&effective_data_dir) {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Error,
                format!("forja-app: --demo could not prepare the sample data: {error}"),
            );
        }
    }

    let locale_locked = flags.locale.is_some();
    if let Some(locale) = flags.locale.as_deref() {
        crash::set_locale(forja_kernel::Locale::parse(locale));
    }
    let runtime =
        match app::build_runtime(flags.locale.clone(), data_dir, user_dir, !flags.no_modules) {
            Ok(runtime) => runtime,
            Err(error) => {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Error,
                    format!("forja-app: could not start: {error}"),
                );
                return 1;
            }
        };
    crash::set_locale(runtime.i18n().locale());

    let background = (flags.test_driver || flags.screenshot.is_some())
        && std::env::var("FORJA_TEST_WINDOW").as_deref() != Ok("visible");
    // Windows may clamp the initial size to the monitor. The app re-applies this
    // size after the window exists so the layout sees the exact logical size.
    let test_size = background.then(test_window_size);
    let screenshot_error = Arc::new(Mutex::new(None::<String>));
    let error_slot = Arc::clone(&screenshot_error);
    let expected = flags.screenshot.clone();
    let job = flags
        .screenshot
        .clone()
        .map(|path| ScreenshotJob::new(path, flags.slot.clone()).with_ui_tree(flags.ui_tree.clone()));
    let session = app::Session {
        theme: flags.theme.clone(),
        density: flags.density.clone(),
        open: flags.open.clone(),
        anvil_query: flags.anvil_query.clone(),
        rail: flags.rail.clone(),
        settings: flags.settings.clone(),
        modal: flags.modal.clone(),
        bench_tab: flags.bench_tab.clone(),
        right_tab: flags.right_tab.clone(),
        test_driver: flags.test_driver,
        test_window_size: test_size,
        locale_locked,
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Forja")
        .with_decorations(false)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([900.0, 600.0]);
    if flags.screenshot.is_none() && runtime.settings().values()
        .get("appearance.window.remember_geometry")
        .and_then(forja_kernel::SettingValue::as_bool).unwrap_or(true)
    {
        if let forja_kernel::store::LoadResult::Loaded(value) =
            runtime.store().load::<serde_json::Value>("window_geometry")
        {
            if let Some(geometry) = window_geometry::Geometry::from_json(&value)
                .filter(|geometry| geometry.on_screen())
            {
                viewport = viewport.with_position([geometry.x, geometry.y])
                    .with_inner_size([geometry.width, geometry.height]);
            }
        }
    }
    if background {
        // Tests and captures must not interrupt whoever uses the computer: the window
        // is placed off-screen, never takes focus and has no taskbar button. It still
        // renders every frame. `FORJA_TEST_WINDOW=visible` shows it for debugging.
        viewport = viewport
            .with_active(false)
            .with_taskbar(false)
            .with_position([-20000.0, -20000.0])
            .with_inner_size(test_size.unwrap_or([1920.0, 1080.0]));
    } else if start_maximized(runtime.settings().values().get("appearance.window.start_maximized"), flags.screenshot.is_some()) {
        viewport = viewport.with_maximized(true);
    }
    if let Some(icon) = icon::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Forja",
        options,
        Box::new(move |creation| {
            let app = ForjaApp::new(creation, runtime, job, error_slot, session);
            Ok(Box::new(app))
        }),
    );
    if let Err(error) = result {
        forja_kernel::logging::log(
            forja_kernel::logging::LogLevel::Error,
            format!("forja-app: {error}"),
        );
        return 1;
    }
    if let Some(error) = screenshot_error
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
    {
        forja_kernel::logging::log(
            forja_kernel::logging::LogLevel::Error,
            format!("forja-app: screenshot failed: {error}"),
        );
        return 1;
    }
    if let Some(path) = expected {
        if !path.exists() {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Error,
                format!("forja-app: screenshot was not written to {}", path.display()),
            );
            return 1;
        }
    }
    0
}

fn start_maximized(value: Option<&forja_kernel::SettingValue>, screenshot: bool) -> bool {
    !screenshot && value.and_then(forja_kernel::SettingValue::as_bool).unwrap_or(false)
}

/// Test windows default to 1920x1080. `FORJA_TEST_WINDOW_SIZE=1280x800`
/// reproduces the window size cited in a play-test issue.
fn test_window_size() -> [f32; 2] {
    match std::env::var("FORJA_TEST_WINDOW_SIZE") {
        Ok(value) => match parse_window_size(&value) {
            Some(size) => size,
            None => {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Warn,
                    format!("forja-app: FORJA_TEST_WINDOW_SIZE=`{value}` is not WIDTHxHEIGHT; using 1920x1080"),
                );
                [1920.0, 1080.0]
            }
        },
        Err(_) => [1920.0, 1080.0],
    }
}

/// Parses `WIDTHxHEIGHT`; the multiplication sign is accepted because issues
/// often write `1280×800`.
fn parse_window_size(value: &str) -> Option<[f32; 2]> {
    let (width, height) = value.trim().split_once(['x', 'X', '×'])?;
    let width: f32 = width.trim().parse().ok()?;
    let height: f32 = height.trim().parse().ok()?;
    if !width.is_finite() || !height.is_finite() || width < 1.0 || height < 1.0 {
        return None;
    }
    Some([width, height])
}

fn dump_catalog(flags: &Flags) -> i32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("forja-dump-catalog-{}-{nanos}", std::process::id()));
    let result = build_catalog_json(flags, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Error,
                format!("forja-app: --dump-catalog failed: {error}"),
            );
            1
        }
    }
}

fn build_catalog_json(flags: &Flags, dir: &std::path::Path) -> Result<String, String> {
    let runtime = app::build_runtime(
        flags.locale.clone(),
        Some(dir.to_path_buf()),
        Some(dir.join("user")),
        !flags.no_modules,
    )
    .map_err(|error| error.to_string())?;
    let registry = runtime.registry();
    let settings: Vec<_> = registry
        .settings()
        .iter()
        .map(|spec| {
            serde_json::json!({
                "key": spec.key,
                "module": spec.module(),
                "kind": spec.kind,
                "default": spec.default,
            })
        })
        .collect();
    let keybinds: Vec<_> = registry
        .keybinds()
        .iter()
        .map(|binding| {
            let module = registry
                .command(binding.command.as_str())
                .map(|command| command.module.as_str())
                .unwrap_or("");
            serde_json::json!({
                "id": binding.command.as_str(),
                "key": binding.chord.to_string(),
                "scope": binding.scope.as_str(),
                "module": module,
            })
        })
        .collect();
    let views: Vec<_> = registry
        .views()
        .iter()
        .map(|view| {
            serde_json::json!({
                "id": view.id,
                "slot": view.slot.id(),
                "module": view.module,
                "title": runtime.i18n().t(&view.title_key),
                "order": view.order,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "settings": settings,
        "keybinds": keybinds,
        "views": views,
    }))
    .map_err(|error| error.to_string())
}

fn set_app_user_model_id() {
    #[cfg(windows)]
    {
        let id: Vec<u16> = "Forja.App"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let _ =
                windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
        }
    }
}

fn parse_flags() -> Result<Option<Flags>, String> {
    let mut flags = Flags {
        screenshot: None,
        slot: None,
        no_modules: false,
        dump_catalog: false,
        locale: None,
        theme: None,
        density: None,
        open: None,
        anvil_query: None,
        rail: None,
        settings: None,
        modal: None,
        ui_tree: None,
        bench_tab: None,
        right_tab: None,
        test_driver: false,
        demo: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(None);
            }
            "--screenshot" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--screenshot needs a PNG path".to_owned())?;
                flags.screenshot = Some(PathBuf::from(value));
            }
            "--slot" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--slot needs a slot name".to_owned())?;
                let valid = [
                    "all", "top", "rail", "sidebar", "bench", "right", "status", "settings",
                    "anvil",
                ];
                if !valid.contains(&value.as_str()) {
                    return Err(format!(
                        "unknown slot `{value}`; use one of {}",
                        valid.join(", ")
                    ));
                }
                flags.slot = Some(value);
            }
            "--no-modules" => flags.no_modules = true,
            "--dump-catalog" => flags.dump_catalog = true,
            "--locale" => {
                flags.locale = Some(
                    args.next()
                        .ok_or_else(|| "--locale needs a language tag".to_owned())?,
                );
            }
            "--theme" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--theme needs dark, light or system".to_owned())?;
                if !matches!(value.as_str(), "dark" | "light" | "system") {
                    return Err("--theme must be dark, light or system".to_owned());
                }
                flags.theme = Some(value);
            }
            "--density" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--density needs comfortable or compact".to_owned())?;
                if !matches!(value.as_str(), "comfortable" | "compact") {
                    return Err("--density must be comfortable or compact".to_owned());
                }
                flags.density = Some(value);
            }
            "--open" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--open needs anvil, onboarding, toast or rail".to_owned())?;
                if !matches!(value.as_str(), "anvil" | "onboarding" | "toast" | "rail") {
                    return Err("--open must be anvil, onboarding, toast or rail".to_owned());
                }
                flags.open = Some(value);
            }
            "--rail" => {
                flags.rail = Some(
                    args.next()
                        .ok_or_else(|| "--rail needs a rail page id, e.g. board.page".to_owned())?,
                );
            }
            "--modal" => {
                flags.modal = Some(
                    args.next()
                        .ok_or_else(|| "--modal needs a modal view id, e.g. voice.overlay".to_owned())?,
                );
            }
            "--settings" => {
                flags.settings = Some(
                    args.next()
                        .ok_or_else(|| "--settings needs a module id, e.g. remote".to_owned())?,
                );
            }
            "--demo" => flags.demo = true,
            "--test-driver" => flags.test_driver = true,
            "--ui-tree" => {
                flags.ui_tree = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--ui-tree needs a JSON path".to_owned())?,
                ));
            }
            "--bench-tab" => {
                flags.bench_tab = Some(
                    args.next()
                        .ok_or_else(|| "--bench-tab needs a bench tab view id".to_owned())?,
                );
            }
            "--right-tab" => {
                flags.right_tab = Some(
                    args.next()
                        .ok_or_else(|| "--right-tab needs a right panel view id".to_owned())?,
                );
            }
            "--anvil-query" => {
                flags.anvil_query = Some(
                    args.next()
                        .ok_or_else(|| "--anvil-query needs text".to_owned())?,
                );
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Some(flags))
}

fn print_help() {
    println!(
        "Forja — janela do aplicativo\n\
         \n\
         Uso:\n  forja-app [opções]\n\
         \n\
         Opções:\n\
           --screenshot <png>   renderiza, salva um PNG e fecha\n\
           --slot <nome>        recorta o PNG: all, top, rail, sidebar, bench,\n\
                                right, status, settings, anvil\n\
           --no-modules         não carrega módulos de produto (shell vazio)\n\
           --dump-catalog       imprime em JSON as configurações e atalhos registrados e sai\n\
           --locale <tag>       pt-BR (padrão) ou en\n\
           --theme <modo>       dark, light ou system (só nesta janela)\n\
           --density <modo>     comfortable ou compact (só nesta janela)\n\
           --open <painel>      anvil, onboarding, toast ou rail\n\
           --rail <id>          abre a página do trilho com esse id (ex.: board.page)\n\
           --settings <módulo>  abre os Ajustes só com a seção desse módulo (ex.: remote)\n\
           --modal <id>         mostra a camada modal dessa view, mesmo fechada (ex.: voice.overlay)\n\
           --anvil-query <texto>  texto inicial da Bigorna\n\
           --bench-tab <id>     abre essa aba da bancada (ex.: editor.tab)\n\
           --right-tab <id>     abre essa aba do painel direito\n\
           --demo               dados de demonstração fixos (2 projetos, 6 bancadas,
                                mutirão, 30 dias de uso, 10 pedidos) numa pasta
                                temporária; combina com --test-driver e --screenshot
           --test-driver        modo de teste: responde ui.snapshot, ui.click, ui.type,\n\
                                ui.key, ui.scroll e ui.wait pelo RPC (agente testador)\n\
           --ui-tree <json>     com --screenshot: grava a árvore de elementos (papel,\n\
                                rótulo, estado e retângulo de cada widget)\n\
           -h, --help           mostra esta ajuda"
    );
}

/// Prints to the terminal that started Forja, if any, without creating one.
fn attach_parent_console() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        // SAFETY: AttachConsole has no preconditions; it fails harmlessly when
        // there is no parent console or stdio is already redirected.
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn maximize_setting_is_applied_only_to_normal_windows() {
        use forja_kernel::SettingValue;
        assert!(super::start_maximized(Some(&SettingValue::Bool(true)), false));
        assert!(!super::start_maximized(Some(&SettingValue::Bool(false)), false));
        assert!(!super::start_maximized(Some(&SettingValue::Bool(true)), true));
    }
    use super::*;

    #[test]
    fn window_size_parser_accepts_x_and_multiplication_sign() {
        assert_eq!(super::parse_window_size("1280x800"), Some([1280.0, 800.0]));
        assert_eq!(super::parse_window_size("1280X800"), Some([1280.0, 800.0]));
        assert_eq!(
            super::parse_window_size(" 1280 × 800 "),
            Some([1280.0, 800.0])
        );
        assert_eq!(
            super::parse_window_size("1920x1080"),
            Some([1920.0, 1080.0])
        );
        assert_eq!(super::parse_window_size("1280*800"), None);
        assert_eq!(super::parse_window_size("1280"), None);
        assert_eq!(super::parse_window_size("0x800"), None);
        assert_eq!(super::parse_window_size("-1280x800"), None);
        assert_eq!(super::parse_window_size("widex800"), None);
        assert_eq!(super::parse_window_size(""), None);
    }

    #[test]
    fn contract_runs_over_every_linked_module() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut runtime = app::build_runtime(
            Some("pt-BR".to_owned()),
            Some(dir.clone()),
            Some(dir.join("user")),
            true,
        )
        .expect("linked modules register cleanly");
        forja_kernel::contract::assert_ok(runtime.registry(), runtime.i18n());
        assert!(runtime
            .registry()
            .commands()
            .iter()
            .any(|command| command.id.as_str() == "anvil.open"));
        let value = runtime
            .call_method("meta.methods", serde_json::Map::new())
            .expect("RPC");
        assert!(value["methods"]
            .as_array()
            .is_some_and(|methods| !methods.is_empty()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dump_catalog_emits_settings_and_keybinds_json() {
        let dir = std::env::temp_dir().join(format!(
            "forja-dump-catalog-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let flags = Flags {
            screenshot: None,
            slot: None,
            demo: false,
            no_modules: false,
            dump_catalog: true,
            locale: Some("pt-BR".to_owned()),
            theme: None,
            density: None,
            open: None,
            anvil_query: None,
            rail: None,
            settings: None,
            modal: None,
            ui_tree: None,
            bench_tab: None,
            right_tab: None,
            test_driver: false,
        };
        let json = build_catalog_json(&flags, &dir).expect("catalog json");
        let _ = std::fs::remove_dir_all(dir);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(parsed["settings"].as_array().is_some_and(|s| !s.is_empty()));
        assert!(parsed["keybinds"].as_array().is_some_and(|k| !k.is_empty()));
    }
}

