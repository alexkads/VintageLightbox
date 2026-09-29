//! O comando que roda o `--recuperar` com privilégio de administrador.
//!
//! Ler o cartão setor por setor exige administrador nos três sistemas, e a
//! janela não roda assim (nem deve). Quem sobe de nível é **só o processo da
//! varredura**, e só pelo tempo dela. Cada sistema tem o seu jeito de pedir a
//! senha, e os três esperam o processo terminar:
//!
//! | sistema | como | desistência |
//! |---|---|---|
//! | Linux | `pkexec` (o diálogo do polkit) | sai com 126 |
//! | macOS | `osascript … with administrator privileges` | sai com 1 (-128) |
//! | Windows | `Start-Process -Verb RunAs -Wait` (o UAC) | o PowerShell sai com 1 |
//!
//! 🔑 A porta não depende do código de saída para saber se o operador
//! desistiu: se o processo acabou **sem nunca ter gravado andamento**, a
//! varredura não chegou a começar (`porta::acompanhar`).
//!
//! ⚠️ As aspas são a parte frágil. Um destino com espaço ou apóstrofo (`Fotos
//! d'Ana`) atravessa dois interpretadores no macOS (AppleScript e shell) e dois
//! no Windows (PowerShell e a linha de comando). As funções de aspas são puras e
//! testadas uma a uma.

use std::path::Path;
use std::process::Command;

/// Os argumentos do modo `--recuperar` do binário.
pub fn argumentos(dispositivo: &str, destino: &str, tamanho: u64) -> Vec<String> {
    vec![
        "--recuperar".into(),
        dispositivo.into(),
        destino.into(),
        tamanho.to_string(),
    ]
}

/// Aspas simples do shell POSIX: `d'Ana` vira `'d'\''Ana'`.
pub fn aspas_do_shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Uma string literal de AppleScript.
pub fn string_do_applescript(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', r"\\").replace('"', "\\\""))
}

/// Uma string entre aspas simples do PowerShell: `'` se dobra.
pub fn aspas_do_powershell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Um argumento na linha de comando do Windows (as regras do
/// `CommandLineToArgvW`): barras só contam antes de aspas.
pub fn argumento_do_windows(s: &str) -> String {
    if !s.is_empty() && !s.contains([' ', '\t', '"']) {
        return s.to_string();
    }
    let mut saida = String::from("\"");
    let mut barras = 0;
    for c in s.chars() {
        match c {
            '\\' => barras += 1,
            '"' => {
                saida.push_str(&"\\".repeat(barras * 2 + 1));
                saida.push('"');
                barras = 0;
            }
            _ => {
                saida.push_str(&"\\".repeat(barras));
                saida.push(c);
                barras = 0;
            }
        }
    }
    saida.push_str(&"\\".repeat(barras * 2));
    saida.push('"');
    saida
}

/// O `do shell script` do macOS.
pub fn script_do_macos(executavel: &str, args: &[String]) -> String {
    let linha: Vec<String> = std::iter::once(executavel)
        .chain(args.iter().map(String::as_str))
        .map(aspas_do_shell)
        .collect();
    format!(
        "do shell script {} with administrator privileges",
        string_do_applescript(&linha.join(" "))
    )
}

/// O comando do PowerShell que abre o UAC e espera.
pub fn comando_do_windows(executavel: &str, args: &[String]) -> String {
    let linha: Vec<String> = args.iter().map(|a| argumento_do_windows(a)).collect();
    format!(
        "$p = Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode",
        aspas_do_powershell(executavel),
        aspas_do_powershell(&linha.join(" "))
    )
}

/// O comando que pede a senha e roda a varredura.
pub fn comando(executavel: &Path, dispositivo: &str, destino: &str, tamanho: u64) -> Command {
    let exe = executavel.to_string_lossy().to_string();
    let args = argumentos(dispositivo, destino, tamanho);
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("osascript");
        c.arg("-e").arg(script_do_macos(&exe, &args));
        c
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("powershell");
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &comando_do_windows(&exe, &args),
        ]);
        // CREATE_NO_WINDOW: o PowerShell não pisca uma janela preta no balcão.
        c.creation_flags(0x0800_0000);
        c
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut c = Command::new("pkexec");
        c.arg(exe).args(args);
        c
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_apostrofo_atravessa_o_shell() {
        assert_eq!(aspas_do_shell("Fotos d'Ana"), r"'Fotos d'\''Ana'");
    }

    #[test]
    fn o_script_do_macos_escapa_as_duas_camadas() {
        let args = argumentos("/dev/rdisk4", "/Users/ana/Fotos \"novas\"", 31_914_983_424);
        let script = script_do_macos("/Applications/VLB/ui-gpui", &args);
        assert_eq!(
            script,
            "do shell script \"'/Applications/VLB/ui-gpui' '--recuperar' '/dev/rdisk4' \
             '/Users/ana/Fotos \\\"novas\\\"' '31914983424'\" with administrator privileges"
        );
    }

    #[test]
    fn a_linha_do_windows_segue_o_command_line_to_argv() {
        assert_eq!(
            argumento_do_windows(r"\\.\PhysicalDrive2"),
            r"\\.\PhysicalDrive2"
        );
        assert_eq!(
            argumento_do_windows(r"C:\Users\Ana Paula\Recuperadas"),
            r#""C:\Users\Ana Paula\Recuperadas""#
        );
        // A barra antes da aspa de fechamento se dobra, senão ela escaparia a
        // aspa e o argumento engoliria o seguinte.
        assert_eq!(
            argumento_do_windows(r"C:\Minhas Fotos\"),
            r#""C:\Minhas Fotos\\""#
        );
        assert_eq!(argumento_do_windows(""), r#""""#);
    }

    #[test]
    fn o_powershell_dobra_o_apostrofo() {
        let args = argumentos(r"\\.\PhysicalDrive2", r"C:\Fotos d'Ana", 10);
        let comando = comando_do_windows(r"C:\VLB\ui-gpui.exe", &args);
        assert_eq!(
            comando,
            "$p = Start-Process -FilePath 'C:\\VLB\\ui-gpui.exe' -ArgumentList \
             '--recuperar \\\\.\\PhysicalDrive2 \"C:\\Fotos d''Ana\" 10' \
             -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode"
        );
    }
}
