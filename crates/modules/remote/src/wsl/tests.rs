//! Testes do WSL: a lista do `wsl -l -v` a partir de bytes UTF-16, tradução de caminhos e a
//! montagem dos argumentos.

use super::*;

const LIST: &str = include_str!("../../tests/fixtures/wsl_list.txt");

/// Bytes UTF-16LE com BOM, como o `wsl.exe` costuma escrever.
fn utf16_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

fn utf16_without_bom(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(|unit| unit.to_le_bytes()).collect()
}

#[test]
fn the_distro_list_decodes_from_utf16_with_or_without_bom() {
    for bytes in [utf16_with_bom(LIST), utf16_without_bom(LIST)] {
        let distros = parse_list(&decode_output(&bytes));
        assert_eq!(distros.len(), 3);
        assert_eq!(distros[0].name, "Ubuntu-24.04");
        assert!(distros[0].is_default, "the star marks the default distro");
        assert_eq!(distros[1].state, "Stopped");
        assert_eq!(distros[2].name, "docker-desktop");
        assert_eq!(distros[2].version, 2);
        assert_eq!(default_distro(&distros).map(|d| d.name.as_str()), Some("Ubuntu-24.04"));
    }
}

#[test]
fn utf8_output_is_still_understood() {
    let distros = parse_list(&decode_output(LIST.as_bytes()));
    assert_eq!(distros.len(), 3);
}

#[test]
fn headers_and_localized_messages_are_skipped() {
    let text = "  NOME              ESTADO          VERSÃO\n* Ubuntu  Executando  2\nA lista de distros está vazia.\n";
    let distros = parse_list(text);
    assert_eq!(distros.len(), 1, "only the row whose last column is a number counts");
    assert_eq!(distros[0].state, "Executando");
}

#[test]
fn windows_drive_paths_become_mnt_paths() {
    assert_eq!(windows_to_wsl_path(r"C:\Users\dev\forja").as_deref(), Some("/mnt/c/Users/dev/forja"));
    assert_eq!(windows_to_wsl_path("D:/dados/").as_deref(), Some("/mnt/d/dados"));
    assert_eq!(windows_to_wsl_path(r"\\?\C:\grande").as_deref(), Some("/mnt/c/grande"));
}

#[test]
fn network_paths_into_a_distro_keep_only_the_inner_path() {
    assert_eq!(
        windows_to_wsl_path(r"\\wsl$\Ubuntu-24.04\home\dev\app").as_deref(),
        Some("/home/dev/app")
    );
    assert_eq!(
        windows_to_wsl_path(r"\\wsl.localhost\Debian\srv").as_deref(),
        Some("/srv")
    );
}

#[test]
fn other_unc_and_relative_paths_cannot_be_translated() {
    assert_eq!(windows_to_wsl_path(r"\\servidor\share\x"), None);
    assert_eq!(windows_to_wsl_path("relativo/pasta"), None);
}

#[test]
fn wrapping_puts_the_distro_and_the_directory_before_the_program() {
    let (program, args) = wrap_command("Ubuntu-24.04", "dev", Some("/home/dev/app"), "npm test");
    assert_eq!(program, "wsl.exe");
    assert_eq!(
        args,
        ["-d", "Ubuntu-24.04", "-u", "dev", "--cd", "/home/dev/app", "--", "sh", "-lc", "npm test"]
    );
}

#[test]
fn an_empty_distro_lets_wsl_use_its_default() {
    let (_, args) = wrap_launch("", "", None, "uname", &["-a".to_owned()]);
    assert_eq!(args, ["--", "uname", "-a"]);
}

#[test]
fn the_terminal_call_opens_a_login_shell_in_the_translated_folder() {
    assert_eq!(
        terminal_args("Debian", "", Some("/srv/app")),
        ["-d", "Debian", "--cd", "/srv/app"]
    );
    assert!(terminal_args("", "", None).is_empty());
}
