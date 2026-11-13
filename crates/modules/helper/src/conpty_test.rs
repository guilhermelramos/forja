//! Teste de aceite: perfil com `.cmd` falso recebe o prompt num ConPTY real.

use crate::launch::{self, LaunchRequest, ProfileOverride};
use crate::profile::{Layer, Profile};
use std::ffi::c_void;
use std::io::Read;
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::*,
    System::{Console::*, Pipes::CreatePipe, Threading::*},
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

#[test]
fn fake_cmd_profile_receives_prompt_through_conpty() {
    let dir = std::env::temp_dir().join(format!(
        "forja-helper-conpty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake-agent.cmd");
    std::fs::write(
        &script,
        "@echo off\r\necho PROMPT_RECEIVED=%*\r\nexit /b 0\r\n",
    )
    .unwrap();
    let profile_text = format!(
        r#"
id = "fake-cmd"
name = "Fake CMD"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["cmd.exe"]
[start]
command = "cmd.exe"
args = ["/d", "/c", "{}"]
[prompt]
mode = "argument"
[permissions.ask]
args = []
"#,
        script.display().to_string().replace('\\', "\\\\")
    );
    let mut profile =
        Profile::parse(&profile_text, PathBuf::from("fake-cmd.toml"), Layer::User).unwrap();
    profile.trusted = true;
    let plan = launch::build(
        &profile,
        &LaunchRequest {
            prompt: Some("hello from forja".to_owned()),
            ..LaunchRequest::default()
        },
        "ask",
        dir.clone(),
        &ProfileOverride::default(),
    )
    .unwrap();
    assert!(plan.args_text().contains("hello from forja"));
    let output = run_in_conpty(&plan.program, &plan.args_text(), &dir);
    assert!(output.contains("hello from forja"), "{output}");
    assert!(output.contains("PROMPT_RECEIVED"), "{output}");
    let _ = std::fs::remove_dir_all(dir);
}

fn run_in_conpty(program: &str, arguments: &str, cwd: &Path) -> String {
    unsafe {
        let (mut input_read, mut input_write, mut output_read, mut output_write) = (
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        assert_ne!(CreatePipe(&mut input_read, &mut input_write, ptr::null(), 0), 0);
        assert_ne!(CreatePipe(&mut output_read, &mut output_write, ptr::null(), 0), 0);
        let mut console = 0;
        assert!(
            CreatePseudoConsole(
                COORD { X: 100, Y: 30 },
                input_read,
                output_write,
                0,
                &mut console,
            ) >= 0
        );
        let mut bytes = 0;
        InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes);
        let mut storage = vec![0usize; bytes.div_ceil(std::mem::size_of::<usize>())];
        let attributes = storage.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        assert_ne!(InitializeProcThreadAttributeList(attributes, 1, 0, &mut bytes), 0);
        assert_ne!(
            UpdateProcThreadAttribute(
                attributes,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                console as *const c_void,
                std::mem::size_of::<HPCON>(),
                ptr::null_mut(),
                ptr::null(),
            ),
            0
        );
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.lpAttributeList = attributes;
        let mut process = PROCESS_INFORMATION::default();
        let mut command = wide(&format!(
            "{} {}",
            launch::quote_arg(program),
            arguments
        ));
        let directory = wide(&cwd.display().to_string());
        let created = CreateProcessW(
            ptr::null(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        );
        DeleteProcThreadAttributeList(attributes);
        CloseHandle(input_read);
        CloseHandle(output_write);
        let _input = std::fs::File::from_raw_handle(input_write);
        assert_ne!(created, 0);
        ResumeThread(process.hThread);
        CloseHandle(process.hThread);

        let mut reader = std::fs::File::from_raw_handle(output_read);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if sender.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut output = String::new();
        while Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(chunk) => output.push_str(&String::from_utf8_lossy(&chunk)),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if output.contains("PROMPT_RECEIVED") {
                break;
            }
        }
        WaitForSingleObject(process.hProcess, 5_000);
        ClosePseudoConsole(console);
        CloseHandle(process.hProcess);
        output
    }
}
