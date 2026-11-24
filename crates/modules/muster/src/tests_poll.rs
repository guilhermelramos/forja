//! Prazo do serviço de recados do Mutirão: o trabalho saiu de `ui()`.

use crate::state;
use crate::view::MusterTick;
use forja_kernel::ui_contract::View;
use forja_kernel::{Runtime, RuntimeOptions, SettingValue};
use std::time::Duration;

fn runtime(dir: &std::path::Path) -> Runtime {
    Runtime::new(
        vec![Box::new(crate::MusterModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.to_path_buf()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("muster registers")
}

#[test]
fn hidden_muster_service_schedules_only_its_real_deadline() {
    let dir = std::env::temp_dir().join(format!(
        "forja-muster-poll-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut runtime = runtime(&dir);
    runtime
        .set_setting("muster.notes.poll_s", SettingValue::Integer(7))
        .expect("poll interval is registered");
    let mut tick = MusterTick::new(state::new_shared());
    let mut cx = runtime.ctx("muster");
    tick.poll(&mut cx);
    let deadline = tick.next_poll_at().expect("a real deadline exists");
    assert!(deadline <= Duration::from_secs(7));
    assert!(deadline > Duration::from_secs(6), "{deadline:?}");
    assert!(
        deadline < Duration::from_secs(7),
        "a just-run service must not ask for a frame every tick"
    );
    drop(cx);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}
