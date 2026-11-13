//! Testes de SSH: validação, montagem da linha de comando e o mapeamento de erros a partir de
//! fixtures de stderr de verdade (OpenSSH).

use super::*;
use crate::target::{LocalForward, SshTarget};

fn target() -> SshTarget {
    SshTarget {
        id: "ssh-build".into(),
        label: "build".into(),
        host: "build.example.com".into(),
        user: "ci".into(),
        port: 2222,
        identity_file: "C:/keys/id".into(),
        proxy_jump: String::new(),
        forwards: Vec::new(),
    }
}

fn forward() -> LocalForward {
    LocalForward {
        local_port: 7001,
        remote_host: "db.internal".into(),
        remote_port: 5432,
        active: false,
    }
}

fn stderr(name: &str) -> &'static str {
    match name {
        "auth" => include_str!("../../tests/fixtures/stderr/auth.txt"),
        "unknown_host" => include_str!("../../tests/fixtures/stderr/unknown_host.txt"),
        "unreachable" => include_str!("../../tests/fixtures/stderr/unreachable.txt"),
        "refused" => include_str!("../../tests/fixtures/stderr/refused.txt"),
        "host_key_unknown" => include_str!("../../tests/fixtures/stderr/host_key_unknown.txt"),
        "host_key_changed" => include_str!("../../tests/fixtures/stderr/host_key_changed.txt"),
        _ => include_str!("../../tests/fixtures/stderr/other.txt"),
    }
}

#[test]
fn the_host_is_always_the_last_argument_of_every_call() {
    let options = SshOptions::default();
    let t = target();
    assert_eq!(test_args(&t, &options).last().map(String::as_str), Some("ok"));
    let probe = test_args(&t, &options);
    assert_eq!(probe[probe.len() - 3], "build.example.com");
    let terminal = terminal_args(&t, &options, None);
    assert_eq!(terminal.last().map(String::as_str), Some("build.example.com"));
    let forward = forward_args(&t, &options, &forward());
    assert_eq!(forward.last().map(String::as_str), Some("build.example.com"));
}

#[test]
fn the_test_call_never_asks_questions_and_honours_the_timeout() {
    let options = SshOptions {
        connect_timeout_s: 12,
        ..SshOptions::default()
    };
    let args = test_args(&target(), &options);
    assert!(args.windows(2).any(|w| w == ["-o", "BatchMode=yes"]));
    assert!(args.windows(2).any(|w| w == ["-o", "ConnectTimeout=12"]));
    assert!(args.windows(2).any(|w| w == ["-p", "2222"]));
    assert!(args.windows(2).any(|w| w == ["-l", "ci"]));
    assert!(args.windows(2).any(|w| w == ["-i", "C:/keys/id"]));
}

#[test]
fn the_terminal_call_forces_a_tty_and_follows_the_settings() {
    let mut options = SshOptions {
        agent_forward: true,
        keep_alive_s: 45,
        host_key: HostKeyPolicy::Strict,
        ..SshOptions::default()
    };
    let args = terminal_args(&target(), &options, Some("/srv/app's"));
    assert_eq!(args[0], "-tt");
    assert!(args.contains(&"-A".to_owned()));
    assert!(args.windows(2).any(|w| w == ["-o", "ServerAliveInterval=45"]));
    assert!(args.windows(2).any(|w| w == ["-o", "StrictHostKeyChecking=yes"]));
    let command = args.iter().find(|a| a.starts_with("cd ")).unwrap();
    assert!(command.starts_with("cd '/srv/app'\\''s' && exec"), "the start directory is quoted: {command}");

    options.keep_alive_s = 0;
    options.agent_forward = false;
    options.host_key = HostKeyPolicy::Ask;
    let plain = terminal_args(&target(), &options, None);
    assert!(!plain.iter().any(|a| a == "-A" || a.starts_with("ServerAliveInterval")));
    assert!(!plain.iter().any(|a| a.starts_with("StrictHostKeyChecking")), "ask leaves the prompt to the terminal");
}

#[test]
fn a_forward_listens_on_loopback_only() {
    let args = forward_args(&target(), &SshOptions::default(), &forward());
    assert_eq!(args[0], "-N");
    assert!(args.windows(2).any(|w| w == ["-L", "127.0.0.1:7001:db.internal:5432"]));
    assert!(args.windows(2).any(|w| w == ["-o", "ExitOnForwardFailure=yes"]));
}

#[test]
fn targets_and_forwards_are_validated_before_any_argument_is_built() {
    assert!(validate_target(&target()).is_ok());

    let mut bad = target();
    bad.host = "-oProxyCommand=evil".into();
    assert!(validate_target(&bad).unwrap_err().contains("must not start with '-'"));

    let mut bad = target();
    bad.user = "ana lima".into();
    assert!(validate_target(&bad).unwrap_err().contains("spaces"));

    let mut twice = target();
    twice.forwards = vec![forward(), forward()];
    assert!(validate_target(&twice).unwrap_err().contains("same local port"));

    let mut ipv6 = forward();
    ipv6.remote_host = "fe80::1".into();
    assert!(validate_forward(&ipv6).unwrap_err().contains("brackets"));
    ipv6.remote_host = "[fe80::1]".into();
    assert!(validate_forward(&ipv6).is_ok());
}

#[test]
fn stderr_from_real_failures_maps_to_the_error_the_user_can_act_on() {
    assert_eq!(classify_failure(stderr("auth")), ProbeError::AuthFailed);
    assert_eq!(classify_failure(stderr("unknown_host")), ProbeError::UnknownHost);
    assert_eq!(classify_failure(stderr("unreachable")), ProbeError::HostUnreachable);
    assert_eq!(classify_failure(stderr("refused")), ProbeError::HostUnreachable);
    assert_eq!(classify_failure(stderr("host_key_unknown")), ProbeError::UnknownHostKey);
    assert_eq!(classify_failure(stderr("host_key_changed")), ProbeError::HostKeyChanged, "changed beats the generic verification message");
    assert_eq!(
        classify_failure(stderr("other")),
        ProbeError::Other("kex_exchange_identification: Connection closed by remote host".into())
    );
}

#[test]
fn a_zero_exit_without_the_ok_line_is_not_a_successful_probe() {
    assert_eq!(probe_outcome(true, "ok\n", ""), Ok(()));
    assert!(matches!(probe_outcome(true, "Welcome banner\n", ""), Err(ProbeError::Other(_))));
    assert_eq!(probe_outcome(false, "", stderr("auth")), Err(ProbeError::AuthFailed));
}

#[test]
fn every_error_has_a_translation_key() {
    for error in [
        ProbeError::AuthFailed,
        ProbeError::HostUnreachable,
        ProbeError::UnknownHost,
        ProbeError::UnknownHostKey,
        ProbeError::HostKeyChanged,
        ProbeError::Other(String::new()),
    ] {
        assert!(error.key().starts_with("remote-error-"), "{error:?}");
        assert!(USED_KEYS.contains(&error.key()), "{}", error.key());
    }
}

#[test]
fn posix_quotes_survive_single_quotes_inside_the_directory() {
    assert_eq!(posix_quote("/a b"), "'/a b'");
    assert_eq!(posix_quote("it's"), "'it'\\''s'");
}

#[test]
fn host_key_policy_words_map_to_the_options() {
    assert_eq!(HostKeyPolicy::parse("accept_new").option(), Some("accept-new"));
    assert_eq!(HostKeyPolicy::parse("ask").option(), None);
    assert_eq!(HostKeyPolicy::parse("unknown"), HostKeyPolicy::Ask);
}

#[test]
fn reuse_options_adds_control_master_when_enabled() {
    let opts = SshOptions::default();
    assert!(opts.reuse_connections);
    assert_eq!(
        reuse_options(&opts),
        vec!["-o", "ControlMaster=auto", "-o", "ControlPersist=60"]
    );
    let disabled = SshOptions {
        reuse_connections: false,
        ..opts
    };
    assert!(reuse_options(&disabled).is_empty());
}
