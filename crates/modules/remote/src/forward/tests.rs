//! Testes do gerenciador de encaminhamentos: estado padrão, quais sobem ao abrir, validação antes
//! de qualquer processo, e que o gerenciador pode ser usado em outra thread.

use super::*;
use crate::ssh::SshOptions;
use crate::target::{LocalForward, SshTarget};

fn target() -> SshTarget {
    SshTarget {
        id: "ssh-build".into(),
        label: "build".into(),
        host: "build.example.com".into(),
        forwards: vec![
            LocalForward {
                local_port: 7001,
                remote_host: "db".into(),
                remote_port: 5432,
                active: true,
            },
            LocalForward {
                local_port: 7002,
                remote_host: "web".into(),
                remote_port: 80,
                active: false,
            },
        ],
        ..SshTarget::default()
    }
}

#[test]
fn every_configured_forward_starts_stopped_in_target_order() {
    let t = target();
    let rows = PortForwards::default().statuses(std::slice::from_ref(&t));
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.state == ForwardState::Stopped && r.pid.is_none()));
    assert_eq!(rows[0].remote_host, "db");
    assert_eq!(rows[1].local_port, 7002);
}

#[test]
fn only_the_forwards_marked_active_are_restored_on_open() {
    let t = target();
    let restore = restorable(std::slice::from_ref(&t));
    assert_eq!(restore, vec![("ssh-build".to_owned(), 7001)]);
}

#[test]
fn a_bad_forward_is_refused_before_any_process_starts() {
    let t = target();
    let bad = LocalForward {
        local_port: 0,
        remote_host: "x".into(),
        remote_port: 1,
        active: false,
    };
    let mut manager = PortForwards::default();
    assert!(manager.start("ssh.exe", &t, &SshOptions::default(), &bad).is_err());
    assert!(!manager.stop("ssh-build", 0), "nothing was tracked");
}

#[test]
fn a_program_that_does_not_exist_is_a_clear_start_error() {
    let t = target();
    let mut manager = PortForwards::default();
    let error = manager
        .start("Z:/nao/existe/ssh.exe", &t, &SshOptions::default(), &t.forwards[0])
        .unwrap_err();
    assert!(error.contains("could not start ssh"), "{error}");
    assert!(manager.statuses(std::slice::from_ref(&t)).iter().all(|r| r.state == ForwardState::Stopped));
}

#[test]
fn the_manager_can_be_moved_into_a_background_job() {
    fn assert_send<T: Send>() {}
    assert_send::<PortForwards>();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(PortForwards::default()));
    let worker = std::sync::Arc::clone(&shared);
    let stopped = std::thread::spawn(move || worker.lock().unwrap().stop("missing", 1))
        .join()
        .unwrap();
    assert!(!stopped);
}

#[test]
fn stopping_something_that_never_ran_is_a_no_op() {
    let mut manager = PortForwards::default();
    assert!(!manager.stop("missing", 1));
    manager.stop_all();
}
