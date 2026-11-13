//! Notificação do Windows com AppUserModelID `Forja.App`.
//!
//! O texto vai num arquivo JSON e o PowerShell só lê esse arquivo, para o
//! título não virar código. A Forja já chamou `SetCurrentProcessExplicitAppUserModelID`
//! no processo antes da janela.

use std::process::{Command, Stdio};

/// Mostra o aviso na central do Windows. Erros voltam como texto.
pub fn show(title: &str, body: &str) -> Result<(), String> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "forja-notify-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let payload = serde_json::json!({ "title": title, "body": body });
    std::fs::write(&path, payload.to_string()).map_err(|error| error.to_string())?;
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .env("FORJA_NOTIFY_FILE", &path)
        .stdin(Stdio::null())
        .output();
    let _ = std::fs::remove_file(&path);
    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "toast do Windows falhou ({}): {stderr}",
                output.status
            ))
        }
        Err(error) => Err(error.to_string()),
    }
}

const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$payload = Get-Content -Raw -Encoding UTF8 -LiteralPath $env:FORJA_NOTIFY_FILE | ConvertFrom-Json
function Esc([string]$value) {
  if ($null -eq $value) { return '' }
  return $value.Replace('&','&amp;').Replace('<','&lt;').Replace('>','&gt;')
}
$xml = "<toast><visual><binding template=`"ToastGeneric`"><text>$(Esc $payload.title)</text><text>$(Esc $payload.body)</text></binding></visual></toast>"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$doc = New-Object Windows.Data.Xml.Dom.XmlDocument
$doc.LoadXml($xml)
$toast = [Windows.UI.Notifications.ToastNotification]::new($doc)
$notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Forja.App')
$notifier.Show($toast)
"#;
