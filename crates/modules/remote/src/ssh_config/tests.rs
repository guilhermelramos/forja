//! Testes do parser de `ssh_config`: aliases, precedência, padrões, `Include` e importação.

use super::*;
use std::collections::HashMap;

#[derive(Default)]
struct Memory(HashMap<PathBuf, String>);

impl ConfigSource for Memory {
    fn read(&self, path: &Path) -> Option<String> {
        self.0.get(path).cloned()
    }

    fn glob(&self, pattern: &Path) -> Vec<PathBuf> {
        let Some(parent) = pattern.parent() else {
            return Vec::new();
        };
        let name = pattern.file_name().unwrap().to_string_lossy().into_owned();
        let mut found: Vec<PathBuf> = self
            .0
            .keys()
            .filter(|p| p.parent() == Some(parent))
            .filter(|p| wildcard_match(&name, &p.file_name().unwrap().to_string_lossy()))
            .cloned()
            .collect();
        found.sort();
        found
    }
}

fn home() -> PathBuf {
    PathBuf::from("C:/Users/dev")
}

fn ssh_dir() -> PathBuf {
    home().join(".ssh")
}

fn parse(main: &str, files: &[(&str, &str)]) -> Vec<HostEntry> {
    let mut memory = Memory::default();
    for (name, body) in files {
        memory.0.insert(ssh_dir().join(name), (*body).to_owned());
    }
    parse_hosts(main, &ssh_dir(), &home(), &memory)
}

fn fixture_main() -> &'static str {
    include_str!("../../tests/fixtures/ssh_config/main.txt")
}

#[test]
fn concrete_aliases_come_in_file_order_and_wildcards_are_skipped() {
    let hosts = parse(fixture_main(), &[]);
    let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert_eq!(aliases, ["build", "db", "web"], "no `*`, no `!`");
}

#[test]
fn the_first_value_for_a_keyword_wins_and_wildcards_fill_the_rest() {
    let main = "Host build\n  HostName build.example.com\n  Port 2222\nHost *\n  Port 22\n  User fallback\n";
    let build = parse(main, &[]).into_iter().find(|h| h.alias == "build").unwrap();
    assert_eq!(build.hostname.as_deref(), Some("build.example.com"));
    assert_eq!(build.port, Some(2222), "the specific block came first");
    assert_eq!(build.user.as_deref(), Some("fallback"), "the wildcard fills what is missing");
}

#[test]
fn options_before_the_first_host_apply_to_everyone() {
    let main = "User global\nHost web\n  HostName web.example.com\n";
    let web = parse(main, &[]).into_iter().find(|h| h.alias == "web").unwrap();
    assert_eq!(web.user.as_deref(), Some("global"));
}

#[test]
fn a_negated_pattern_excludes_the_block_for_that_alias() {
    let main = "Host * !db\n  ForwardAgent no\nHost db\n  HostName db.internal\n";
    let hosts = parse(main, &[]);
    let db = hosts.iter().find(|h| h.alias == "db").unwrap();
    assert_eq!(db.hostname.as_deref(), Some("db.internal"));
    assert!(db.user.is_none());
}

#[test]
fn a_host_line_with_several_aliases_shares_its_options() {
    let main = "Host alpha beta\n  User shared\n";
    let hosts = parse(main, &[]);
    assert_eq!(hosts.len(), 2);
    assert!(hosts.iter().all(|h| h.user.as_deref() == Some("shared")));
}

#[test]
fn include_expands_in_place_with_globs_and_home_paths() {
    let main = "Include config.d/*.conf\nHost web\n  HostName web.example.com\n";
    let hosts = parse(
        main,
        &[
            ("config.d/a.conf", "Host from-a\n  HostName a.example.com\n"),
            ("config.d/b.conf", "Host from-b\n  IdentityFile ~/keys/b\n"),
            ("config.d/readme.txt", "Host ignored\n"),
        ],
    );
    let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
    assert!(aliases.contains(&"from-a") && aliases.contains(&"from-b"));
    assert!(!aliases.contains(&"ignored"), "the glob only matches .conf files");
    let from_b = hosts.iter().find(|h| h.alias == "from-b").unwrap();
    assert_eq!(from_b.identity_file.as_deref(), Some(home().join("keys/b").to_str().unwrap()));
}

#[test]
fn match_blocks_never_apply_their_options() {
    let main = "Match host *.corp\n  User corp\nHost web\n  HostName web.example.com\n";
    let web = parse(main, &[]).into_iter().find(|h| h.alias == "web").unwrap();
    assert!(web.user.is_none(), "the Match block was not evaluated");
}

#[test]
fn keywords_are_case_insensitive_and_values_may_be_quoted_or_use_equals() {
    let main = "host build\n  hostname=build.example.com\n  IdentityFile \"C:/Chaves Antigas/id\"\n  port 2200\n";
    let build = parse(main, &[]).into_iter().find(|h| h.alias == "build").unwrap();
    assert_eq!(build.hostname.as_deref(), Some("build.example.com"));
    assert_eq!(build.identity_file.as_deref(), Some("C:/Chaves Antigas/id"));
    assert_eq!(build.port, Some(2200));
}

#[test]
fn comments_are_only_recognised_on_their_own_line() {
    let main = "# a comment line\nHost web\n  HostName web.example.com # not a comment for OpenSSH\n";
    let web = parse(main, &[]).into_iter().find(|h| h.alias == "web").unwrap();
    assert_eq!(web.hostname.as_deref(), Some("web.example.com # not a comment for OpenSSH"));
}

#[test]
fn importing_builds_editable_targets_with_unique_ids_and_port_22_by_default() {
    let hosts = vec![
        HostEntry {
            alias: "Build".into(),
            ..HostEntry::default()
        },
        HostEntry {
            alias: "build".into(),
            port: Some(2222),
            user: Some("ci".into()),
            ..HostEntry::default()
        },
    ];
    let targets = import_targets(&hosts);
    assert_eq!(targets[0].id, "ssh-build");
    assert_eq!(targets[1].id, "ssh-build-2", "ids do not collide");
    assert_eq!(targets[0].port, DEFAULT_SSH_PORT);
    assert_eq!(targets[1].port, 2222);
    assert_eq!(targets[1].user, "ci");
    assert_eq!(targets[0].host, "Build", "the alias keeps resolving through ssh");
}

#[test]
fn wildcard_matching_is_case_insensitive() {
    assert!(wildcard_match("*.Corp", "web.corp"));
    assert!(wildcard_match("db?", "DB1"));
    assert!(!wildcard_match("db?", "db12"));
}
