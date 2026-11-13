//! Testes de integração: o módulo no kernel real, com um terminal falso que registra o que recebe.

use super::*;
use crate::target::SshTarget;
use forja_kernel::{contract, Args, Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// O terminal falso guarda o que `terminal.open` recebeu.
struct FakeTerminal(Arc<Mutex<Vec<Value>>>);

impl Module for FakeTerminal {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("terminal", "terminal-name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n("terminal-name = Fake terminal\nterminal-open = Open\n", "terminal-name = Terminal falso\nterminal-open = Abrir\n");
        r.uses_i18n("terminal-open");
        let opened = Arc::clone(&self.0);
        r.command("terminal.open", "terminal-open", move |_cx, args| {
            opened.lock().unwrap().push(args.into_json());
            Ok(json!({ "terminal_id": "t-9" }))
        });
    }
}

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forja-remote-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Harness {
    runtime: Runtime,
    shared: Shared,
    opened: Arc<Mutex<Vec<Value>>>,
    dir: PathBuf,
}

fn harness(label: &str) -> Harness {
    let dir = dir().join(label);
    let opened = Arc::new(Mutex::new(Vec::new()));
    let module = RemoteModule::new();
    let shared = Arc::clone(&module.shared);
    let runtime = Runtime::new(
        vec![Box::new(module), Box::new(FakeTerminal(Arc::clone(&opened)))],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("remote registers with a terminal");
    Harness {
        runtime,
        shared,
        opened,
        dir,
    }
}

fn args(pairs: &[(&str, Value)]) -> Args {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_owned(), value.clone());
    }
    Args::from_map(map)
}

fn build_target() -> SshTarget {
    SshTarget {
        id: "ssh-build".into(),
        label: "build".into(),
        host: "build.example.com".into(),
        user: "ci".into(),
        port: 2222,
        ..SshTarget::default()
    }
}

#[test]
fn contract_holds_for_the_module_alone() {
    let h = harness("contract");
    contract::assert_ok(h.runtime.registry(), h.runtime.i18n());
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn the_twelve_active_catalog_settings_are_declared() {
    let h = harness("settings");
    let keys: Vec<&str> = h.runtime.registry().settings().iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys.iter().filter(|k| k.starts_with("remote.")).count(), 12);
    assert!(keys.contains(&"remote.forward.mode"));
    // deferred in design/settings.toml: no file sync exists yet
    assert!(!keys.contains(&"remote.sync.mode"));
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn opening_ssh_hands_the_terminal_a_call_with_the_host_last() {
    let mut h = harness("open");
    h.shared.lock().unwrap().targets.push(build_target());
    let result = h.runtime.run_command("remote.ssh.open", args(&[("id", json!("ssh-build"))])).unwrap();
    assert_eq!(result["terminal_id"], "t-9");
    let opened = h.opened.lock().unwrap();
    let call = &opened[0];
    assert_eq!(call["title"], "build");
    assert_eq!(call["command"], ssh::ssh_program());
    let argv: Vec<&str> = call["args"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(argv[0], "-tt");
    assert_eq!(*argv.last().unwrap(), "build.example.com");
    assert!(argv.windows(2).any(|w| w == ["-p", "2222"]));
    drop(opened);
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn ssh_turned_off_refuses_connection_commands_without_running_anything() {
    let mut h = harness("off");
    h.shared.lock().unwrap().targets.push(build_target());
    h.runtime
        .set_setting("remote.ssh.enabled", forja_kernel::SettingValue::Bool(false))
        .unwrap();
    let refused = h
        .runtime
        .run_command("remote.ssh.test", args(&[("id", json!("ssh-build"))]))
        .unwrap_err();
    assert!(refused.to_string().contains("turned off"), "{refused}");
    let refused = h
        .runtime
        .run_command("remote.ssh.open", args(&[("id", json!("ssh-build"))]))
        .unwrap_err();
    assert!(refused.to_string().contains("turned off"), "{refused}");
    assert!(h.opened.lock().unwrap().is_empty(), "no terminal was opened");
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn saving_a_target_is_refused_until_the_store_has_loaded() {
    let mut h = harness("unloaded");
    let value = json!({ "label": "x", "host": "x.example.com" });
    let result = h.runtime.run_command("remote.ssh.save", args(&[("target", value)])).unwrap();
    assert_eq!(result["saved"], false);
    assert_eq!(result["error"], "remote-error-loading");
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn a_target_with_a_dash_host_is_refused_by_validation() {
    let mut h = harness("validation");
    h.shared.lock().unwrap().loaded = true;
    let value = json!({ "label": "x", "host": "-oProxyCommand=evil" });
    let result = h.runtime.run_command("remote.ssh.save", args(&[("target", value)])).unwrap();
    assert_eq!(result["saved"], false);
    assert_eq!(result["error"], "remote-error-validation");
    assert!(result["message"].as_str().unwrap().contains("must not start with '-'"));
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn a_valid_target_gets_an_id_from_its_label_and_is_kept() {
    let mut h = harness("save");
    h.shared.lock().unwrap().loaded = true;
    let value = json!({ "label": "Servidor de build", "host": "build.example.com", "port": 22 });
    let result = h.runtime.run_command("remote.ssh.save", args(&[("target", value)])).unwrap();
    assert_eq!(result["saved"], true);
    assert_eq!(result["id"], "ssh-servidor-de-build");
    assert_eq!(h.shared.lock().unwrap().targets.len(), 1);
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn wsl_wrap_is_pure_and_translates_the_bench_folder() {
    let mut h = harness("wrap");
    let result = h
        .runtime
        .run_command(
            "remote.wsl.wrap",
            args(&[
                ("distro", json!("Ubuntu")),
                ("cwd", json!(r"C:\projetos\forja")),
                ("command", json!("npm test")),
            ]),
        )
        .unwrap();
    assert_eq!(result["program"], "wsl.exe");
    assert_eq!(result["cwd"], "/mnt/c/projetos/forja");
    let argv: Vec<&str> = result["args"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(argv.windows(2).any(|w| w == ["--cd", "/mnt/c/projetos/forja"]));
    assert_eq!(&argv[argv.len() - 3..], ["sh", "-lc", "npm test"]);
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn wsl_wrap_refuses_an_empty_command() {
    let mut h = harness("wrap-empty");
    let error = h
        .runtime
        .run_command("remote.wsl.wrap", args(&[("command", json!("   "))]))
        .unwrap_err();
    assert!(error.to_string().contains("empty"), "{error}");
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn the_ssh_list_is_empty_on_a_fresh_install() {
    let mut h = harness("list");
    let listed = h.runtime.call_method("remote.ssh.list", Map::new()).unwrap();
    assert_eq!(listed["targets"], json!([]));
    assert_eq!(listed["enabled"], true);
    assert_eq!(listed["default_host"], "");
    let _ = std::fs::remove_dir_all(h.dir);
}

#[test]
fn open_ssh_uses_default_host_when_id_is_omitted() {
    let mut h = harness("default-host");
    h.shared.lock().unwrap().loaded = true;
    let target = SshTarget {
        id: "ssh-staging".into(),
        label: "Staging".into(),
        host: "staging.example.com".into(),
        ..SshTarget::default()
    };
    h.shared.lock().unwrap().targets.push(target);
    h.runtime
        .set_setting(
            "remote.ssh.default_host",
            forja_kernel::SettingValue::Text("staging.example.com".into()),
        )
        .unwrap();
    let result = h.runtime.run_command("remote.ssh.open", Args::new()).unwrap();
    assert_eq!(result["terminal_id"], "t-9");
    assert_eq!(h.opened.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(h.dir);
}
