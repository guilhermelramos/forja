//! Processo de terminal sobre ConPTY.

mod conpty;

pub use conpty::PtyProcess;

#[cfg(test)]
pub use conpty::environment_block;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn read_until(
        process: &PtyProcess,
        needle: &str,
        timeout: Duration,
    ) -> String {
        let deadline = Instant::now() + timeout;
        let mut text = String::new();
        while Instant::now() < deadline {
            match process.output.recv_timeout(Duration::from_millis(30)) {
                Ok(chunk) => {
                    text.push_str(&String::from_utf8_lossy(&chunk));
                    if text.contains(needle) {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        text
    }

    #[test]
    fn environment_declares_terminal_capabilities() {
        let parent = std::env::var_os("TERM");
        let block = String::from_utf16_lossy(&environment_block(&Default::default()));
        assert!(block.contains("TERM=xterm-256color\0"));
        assert!(block.contains("COLORTERM=truecolor\0"));
        assert_eq!(std::env::var_os("TERM"), parent);
        assert!(block.ends_with("\0\0"));
    }

    #[test]
    fn cmd_terminal_reads_output_and_exits() {
        let process = PtyProcess::spawn(
            "cmd.exe",
            "/d /c echo forja_pty_marker",
            Path::new("."),
        )
        .expect("spawn cmd");
        let text = read_until(&process, "forja_pty_marker", Duration::from_secs(10));
        assert!(text.contains("forja_pty_marker"), "{text:?}");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut exited = None;
        while Instant::now() < deadline {
            if let Some(code) = process.exit_code() {
                exited = Some(code);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(exited, Some(0));
    }

    #[test]
    fn powershell_roundtrip_sends_and_reads_echo() {
        let process = PtyProcess::spawn(
            "powershell.exe",
            "-NoLogo -NoProfile",
            Path::new("."),
        )
        .expect("spawn powershell");
        // O pipe do ConPTY guarda a entrada mesmo antes do prompt aparecer.
        process
            .input
            .send(b"echo forja_roundtrip\r".to_vec())
            .expect("send input");
        let text = read_until(&process, "forja_roundtrip", Duration::from_secs(25));
        assert!(text.contains("forja_roundtrip"), "{text:?}");
        drop(process);
    }

    #[test]
    fn dropping_terminal_kills_the_grandchild() {
        let process = PtyProcess::spawn(
            "powershell.exe",
            "-NoLogo -NoProfile -Command \"$p=Start-Process cmd.exe -ArgumentList '/c ping -n 60 127.0.0.1' -PassThru -WindowStyle Hidden; Start-Sleep -Milliseconds 500; $g=Get-CimInstance Win32_Process -Filter ('ParentProcessId='+$p.Id) | Select-Object -First 1; Write-Output ('CHILD_PID='+$g.ProcessId); Start-Sleep -Seconds 60\"",
            Path::new("."),
        )
        .expect("spawn tree");
        let text = read_until(&process, "CHILD_PID=", Duration::from_secs(20));
        let tail = text.split_once("CHILD_PID=").expect("child pid").1;
        let digits: String = tail.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        let child: u32 = digits.parse().expect("numeric child pid");
        let handle = unsafe {
            windows_sys::Win32::System::Threading::OpenProcess(
                0x0010_0000 | windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                child,
            )
        };
        assert!(!handle.is_null(), "could not open child process");
        let started = Instant::now();
        drop(process);
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "drop blocked for {:?}",
            started.elapsed()
        );
        unsafe {
            let result =
                windows_sys::Win32::System::Threading::WaitForSingleObject(handle, 3_000);
            windows_sys::Win32::Foundation::CloseHandle(handle);
            assert_eq!(
                result,
                windows_sys::Win32::Foundation::WAIT_OBJECT_0,
                "descendant survived the terminal"
            );
        }
    }
}
