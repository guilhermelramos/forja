//! WSL: leitura da lista de distros, tradução de caminhos entre Windows e Linux, e a montagem
//! dos argumentos do `wsl.exe`. Nada aqui roda um processo; quem roda é a camada de comandos.
//!
//! O `wsl.exe` escreve a lista em UTF-16LE, às vezes sem BOM. O decodificador aceita os dois casos.

/// Uma distro instalada, como `wsl.exe -l -v` a mostra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WslDistro {
    pub name: String,
    /// `Running`, `Stopped`… como o `wsl` imprime (o texto localizado fica como está).
    pub state: String,
    pub version: u8,
    /// Marcada com `*`: a distro padrão do WSL.
    pub is_default: bool,
}

/// Decodifica a saída do `wsl.exe`: UTF-16LE com ou sem BOM; sem nenhum dos dois, UTF-8.
pub fn decode_output(bytes: &[u8]) -> String {
    let (body, utf16) = if bytes.starts_with(&[0xFF, 0xFE]) {
        (&bytes[2..], true)
    } else {
        // Sem BOM, texto ASCII em UTF-16LE tem um NUL em cada byte ímpar.
        let odd_nuls = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
        (bytes, bytes.len() >= 2 && odd_nuls * 2 >= bytes.len() / 2)
    };
    if utf16 {
        let units: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(body).into_owned()
    }
}

/// Lê as linhas de `wsl -l -v`. O cabeçalho e as mensagens localizadas são pulados, porque a
/// última coluna deles não é um número.
pub fn parse_list(text: &str) -> Vec<WslDistro> {
    let mut distros = Vec::new();
    for line in text.lines() {
        let mut tokens: Vec<&str> = line.split_whitespace().collect();
        let is_default = tokens.first() == Some(&"*");
        if is_default {
            tokens.remove(0);
        }
        if tokens.len() != 3 {
            continue;
        }
        let Ok(version) = tokens[2].parse::<u8>() else {
            continue;
        };
        distros.push(WslDistro {
            name: tokens[0].to_string(),
            state: tokens[1].to_string(),
            version,
            is_default,
        });
    }
    distros
}

/// Argumentos para listar as distros.
pub fn list_args() -> Vec<String> {
    vec!["-l".into(), "-v".into()]
}

/// A distro padrão é a marcada com `*`.
pub fn default_distro(distros: &[WslDistro]) -> Option<&WslDistro> {
    distros.iter().find(|d| d.is_default)
}

/// Caminho do Windows para o caminho Linux do mesmo lugar.
/// Letras de unidade viram `/mnt/<letra>/…`; `\\wsl$\<distro>\…` e `\\wsl.localhost\<distro>\…`
/// viram o caminho dentro da distro. Outros UNC e caminhos relativos devolvem `None`.
pub fn windows_to_wsl_path(path: &str) -> Option<String> {
    let path = path.trim();
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("//wsl$/")
        .or_else(|| lower.strip_prefix("//wsl.localhost/"))
    {
        // Descarta o segmento da distro; mantém a capitalização original do restante.
        let consumed = normalized.len() - rest.len();
        let after = &normalized[consumed..];
        let inner = after.split_once('/').map(|(_, r)| r).unwrap_or("");
        return Some(collapse(&format!("/{inner}")));
    }
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = &normalized[2..];
        return Some(collapse(&format!("/mnt/{drive}/{rest}")));
    }
    None
}

/// Tira barras repetidas e a barra final (exceto na raiz).
fn collapse(path: &str) -> String {
    let mut out = String::new();
    let mut last_slash = false;
    for c in path.chars() {
        if c == '/' {
            if !last_slash {
                out.push('/');
            }
            last_slash = true;
        } else {
            out.push(c);
            last_slash = false;
        }
    }
    if out.len() > 1 {
        out = out.trim_end_matches('/').to_string();
        if out.is_empty() {
            out.push('/');
        }
    }
    out
}

/// `-d <distro>` e `-u <usuário>`, quando estão preenchidos.
fn distro_args(distro: &str, user: &str) -> Vec<String> {
    let mut args = Vec::new();
    let distro = distro.trim();
    if !distro.is_empty() {
        args.push("-d".into());
        args.push(distro.into());
    }
    let user = user.trim();
    if !user.is_empty() {
        args.push("-u".into());
        args.push(user.into());
    }
    args
}

/// Envolve um programa para rodar dentro do WSL: `wsl.exe -d <distro> --cd <caminho> -- <programa> …`.
pub fn wrap_launch(distro: &str, user: &str, cwd: Option<&str>, program: &str, args: &[String]) -> (String, Vec<String>) {
    let mut out = distro_args(distro, user);
    if let Some(cwd) = cwd.map(str::trim).filter(|c| !c.is_empty()) {
        out.push("--cd".into());
        out.push(cwd.into());
    }
    out.push("--".into());
    out.push(program.into());
    out.extend(args.iter().cloned());
    ("wsl.exe".into(), out)
}

/// Como `wrap_launch`, mas a linha é um comando de shell (`sh -lc`). Para o `remote.wsl.wrap`.
pub fn wrap_command(distro: &str, user: &str, cwd: Option<&str>, command: &str) -> (String, Vec<String>) {
    wrap_launch(distro, user, cwd, "sh", &["-lc".to_owned(), command.to_owned()])
}

/// Argumentos de um shell de login numa distro, numa pasta Linux opcional.
pub fn terminal_args(distro: &str, user: &str, cwd: Option<&str>) -> Vec<String> {
    let mut args = distro_args(distro, user);
    if let Some(cwd) = cwd.map(str::trim).filter(|c| !c.is_empty()) {
        args.push("--cd".into());
        args.push(cwd.into());
    }
    args
}

#[cfg(test)]
mod tests;
