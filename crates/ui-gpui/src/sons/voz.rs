//! 🗣️ A voz do sistema operacional, pela ferramenta que ele já tem.
//!
//! | Sistema | Falar | Vozes |
//! |---|---|---|
//! | macOS | `say` | `say -v ?` |
//! | Windows | PowerShell + `System.Speech` | `GetInstalledVoices()` |
//! | Linux | `spd-say` (speech-dispatcher), senão `espeak-ng` | `spd-say -L` / `espeak-ng --voices` |
//!
//! 🔑 **Processo, e não biblioteca.** O crate `tts` fala com as mesmas peças,
//! mas no Linux pede o `speech-dispatcher` de desenvolvimento para compilar, e
//! o balcão se atualiza compilando: uma dependência nativa nova é um balcão
//! que para de atualizar. Chamar o programa não pede nada para compilar, e
//! sem ele a voz só não fala.
//!
//! 🚨 **O texto digitado nunca passa por um shell.** No Mac e no Linux ele é
//! um argumento do processo (depois de `--`, ou com um prefixo que não começa
//! por `-`); no Windows vai pelo ambiente (`VLB_TEXTO`), e o script do
//! PowerShell é fixo. Um texto com aspas, `;` ou `$(…)` é falado, não
//! executado.

use std::process::{Command, Stdio};

use super::porta::{Fala, Voz};

/// Os sistemas, separados do `cfg` para a montagem do comando ser conferível
/// em qualquer um.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sistema {
    Mac,
    Windows,
    /// Com qual ferramenta: o `spd-say` primeiro, o `espeak-ng` se não houver.
    Linux(Ferramenta),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ferramenta {
    SpdSay,
    EspeakNg,
}

impl Sistema {
    /// O deste computador; `None` quando não há como falar.
    pub fn deste() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Sistema::Mac)
        } else if cfg!(windows) {
            Some(Sistema::Windows)
        } else if no_caminho("spd-say") {
            Some(Sistema::Linux(Ferramenta::SpdSay))
        } else if no_caminho("espeak-ng") {
            Some(Sistema::Linux(Ferramenta::EspeakNg))
        } else {
            None
        }
    }
}

fn no_caminho(programa: &str) -> bool {
    std::env::var_os("PATH")
        .map(|caminhos| {
            std::env::split_paths(&caminhos).any(|pasta| pasta.join(programa).is_file())
        })
        .unwrap_or(false)
}

/// Um processo a chamar — montado sem executar, para os testes o lerem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comando {
    pub programa: String,
    pub argumentos: Vec<String>,
    pub ambiente: Vec<(String, String)>,
}

impl Comando {
    fn novo(programa: &str) -> Self {
        Self {
            programa: programa.into(),
            argumentos: Vec::new(),
            ambiente: Vec::new(),
        }
    }

    fn arg(mut self, valor: impl Into<String>) -> Self {
        self.argumentos.push(valor.into());
        self
    }

    fn amb(mut self, nome: &str, valor: impl Into<String>) -> Self {
        self.ambiente.push((nome.into(), valor.into()));
        self
    }

    fn processo(&self) -> Command {
        let mut processo = Command::new(&self.programa);
        processo
            .args(&self.argumentos)
            .envs(self.ambiente.iter().map(|(n, v)| (n, v)))
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            // Sem a janela preta do console piscando a cada aviso.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            processo.creation_flags(CREATE_NO_WINDOW);
        }
        processo
    }

    /// Roda e espera terminar — quem chama está na thread do alto-falante.
    pub fn rodar(&self) -> Result<(), String> {
        let saida = self
            .processo()
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|erro| format!("{}: {erro}", self.programa))?;
        if saida.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{} terminou com {}: {}",
                self.programa,
                saida.status,
                String::from_utf8_lossy(&saida.stderr).trim()
            ))
        }
    }

    fn ler(&self) -> Option<String> {
        let saida = self.processo().stderr(Stdio::null()).output().ok()?;
        saida
            .status
            .success()
            .then(|| String::from_utf8_lossy(&saida.stdout).into_owned())
    }
}

/// O script do Windows: fixo, e tudo o que muda vem do ambiente.
const FALAR_NO_WINDOWS: &str = "Add-Type -AssemblyName System.Speech; \
    $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
    $s.Volume = [int]$env:VLB_VOLUME; $s.Rate = [int]$env:VLB_RITMO; \
    if ($env:VLB_VOZ) { try { $s.SelectVoice($env:VLB_VOZ) } catch {} }; \
    $s.Speak($env:VLB_TEXTO)";

const VOZES_NO_WINDOWS: &str = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
    Add-Type -AssemblyName System.Speech; \
    (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | \
    ForEach-Object { $_.VoiceInfo.Name + '|' + $_.VoiceInfo.Culture.Name }";

fn powershell(script: &str) -> Comando {
    Comando::novo("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(script)
}

/// O comando que fala `fala` com o volume (0–1) dado.
pub fn comando_de_fala(sistema: Sistema, fala: &Fala, volume: f32) -> Comando {
    let volume = volume.clamp(0.0, 1.0);
    let ritmo = fala.ritmo.clamp(0.5, 2.0);
    // 175 palavras por minuto é o ritmo normal do `say` e do `espeak-ng`.
    let palavras_por_minuto = (175.0 * ritmo).round() as i32;
    match sistema {
        Sistema::Mac => {
            let mut comando = Comando::novo("say")
                .arg("-r")
                .arg(palavras_por_minuto.to_string());
            if let Some(voz) = &fala.voz {
                comando = comando.arg("-v").arg(voz.clone());
            }
            // O `say` não tem opção de volume: o comando embutido `[[volm]]`
            // faz isso, e de quebra o texto nunca começa por `-`.
            comando.arg(format!("[[volm {volume:.2}]] {}", fala.texto))
        }
        Sistema::Windows => powershell(FALAR_NO_WINDOWS)
            .amb("VLB_TEXTO", fala.texto.clone())
            .amb("VLB_VOLUME", ((volume * 100.0).round() as i32).to_string())
            .amb(
                "VLB_RITMO",
                (((ritmo - 1.0) * 10.0).round() as i32)
                    .clamp(-10, 10)
                    .to_string(),
            )
            .amb("VLB_VOZ", fala.voz.clone().unwrap_or_default()),
        Sistema::Linux(Ferramenta::SpdSay) => {
            let mut comando = Comando::novo("spd-say")
                .arg("-w")
                .arg("-r")
                .arg(
                    (((ritmo - 1.0) * 100.0).round() as i32)
                        .clamp(-100, 100)
                        .to_string(),
                )
                .arg("-i")
                .arg(((volume * 200.0 - 100.0).round() as i32).to_string());
            comando = match &fala.voz {
                Some(voz) => comando.arg("-y").arg(voz.clone()),
                None => comando.arg("-l").arg("pt-BR"),
            };
            comando.arg("--").arg(fala.texto.clone())
        }
        Sistema::Linux(Ferramenta::EspeakNg) => Comando::novo("espeak-ng")
            .arg("-v")
            .arg(fala.voz.clone().unwrap_or_else(|| "pt-br".into()))
            .arg("-s")
            .arg(palavras_por_minuto.to_string())
            .arg("-a")
            .arg(((volume * 200.0).round() as i32).to_string())
            .arg("--")
            .arg(fala.texto.clone()),
    }
}

pub fn comando_das_vozes(sistema: Sistema) -> Comando {
    match sistema {
        Sistema::Mac => Comando::novo("say").arg("-v").arg("?"),
        Sistema::Windows => powershell(VOZES_NO_WINDOWS),
        Sistema::Linux(Ferramenta::SpdSay) => Comando::novo("spd-say").arg("-L"),
        Sistema::Linux(Ferramenta::EspeakNg) => Comando::novo("espeak-ng").arg("--voices"),
    }
}

/// As vozes deste computador, as em português primeiro.
pub fn listar(sistema: Sistema) -> Vec<Voz> {
    let Some(texto) = comando_das_vozes(sistema).ler() else {
        return Vec::new();
    };
    let mut vozes = ler_vozes(sistema, &texto);
    ordenar(&mut vozes);
    vozes
}

pub fn ler_vozes(sistema: Sistema, texto: &str) -> Vec<Voz> {
    match sistema {
        // `Luciana             pt_BR    # Olá, meu nome é Luciana…`
        Sistema::Mac => texto
            .lines()
            .filter_map(|linha| {
                let antes = linha.split('#').next()?.trim_end();
                let (nome, idioma) = antes.rsplit_once(char::is_whitespace)?;
                let nome = nome.trim();
                (!nome.is_empty()).then(|| Voz {
                    id: nome.into(),
                    nome: nome.into(),
                    idioma: idioma.replace('_', "-"),
                })
            })
            .collect(),
        // `Microsoft Maria Desktop|pt-BR`
        Sistema::Windows => texto
            .lines()
            .filter_map(|linha| {
                let (nome, idioma) = linha.trim().split_once('|')?;
                Some(Voz {
                    id: nome.into(),
                    nome: nome.into(),
                    idioma: idioma.into(),
                })
            })
            .collect(),
        // `     NAME                 LANGUAGE    VARIANT`
        // `     Portuguese (Brazil)  pt-BR       none`
        Sistema::Linux(Ferramenta::SpdSay) => texto
            .lines()
            .filter(|linha| !linha.trim_start().starts_with("NAME"))
            .filter_map(|linha| {
                let mut partes: Vec<&str> = linha.split_whitespace().collect();
                if partes.len() < 3 {
                    return None;
                }
                partes.pop(); // a variante
                let idioma = partes.pop()?;
                let nome = partes.join(" ");
                Some(Voz {
                    id: nome.clone(),
                    nome,
                    idioma: idioma.into(),
                })
            })
            .collect(),
        // `Pty Language       Age/Gender VoiceName          File        Other Languages`
        // ` 5  pt-br           --/M      Portuguese_(Brazil) roa/pt-BR`
        Sistema::Linux(Ferramenta::EspeakNg) => texto
            .lines()
            .skip(1)
            .filter_map(|linha| {
                let partes: Vec<&str> = linha.split_whitespace().collect();
                let idioma = *partes.get(1)?;
                let nome = partes.get(3)?.replace('_', " ");
                Some(Voz {
                    id: idioma.into(),
                    nome,
                    idioma: idioma.into(),
                })
            })
            .collect(),
    }
}

/// Português do Brasil primeiro, depois o resto do português, depois o
/// resto — cada grupo em ordem de nome.
pub fn ordenar(vozes: &mut [Voz]) {
    fn grupo(voz: &Voz) -> u8 {
        let idioma = voz.idioma.to_lowercase();
        if idioma.starts_with("pt-br") || idioma.starts_with("pt_br") {
            0
        } else if idioma.starts_with("pt") {
            1
        } else {
            2
        }
    }
    vozes.sort_by(|a, b| grupo(a).cmp(&grupo(b)).then_with(|| a.nome.cmp(&b.nome)));
}

/// A voz a usar quando o operador não escolheu: a primeira em português.
pub fn padrao(vozes: &[Voz]) -> Option<String> {
    vozes
        .iter()
        .find(|voz| voz.idioma.to_lowercase().starts_with("pt"))
        .map(|voz| voz.id.clone())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn fala(texto: &str) -> Fala {
        Fala {
            texto: texto.into(),
            voz: None,
            ritmo: 1.0,
        }
    }

    /// 🚨 O texto digitado chega **inteiro e literal** ao programa, num
    /// argumento só (ou no ambiente, no Windows), e nunca vira opção.
    #[test]
    fn o_texto_vai_literal_e_nunca_vira_opcao() {
        let perigoso = r#"-v x"; rm -rf ~ $(echo oi) `ls`"#;
        let mac = comando_de_fala(Sistema::Mac, &fala(perigoso), 0.5);
        assert_eq!(mac.programa, "say");
        assert_eq!(
            mac.argumentos.last().unwrap(),
            &format!("[[volm 0.50]] {perigoso}")
        );

        for ferramenta in [Ferramenta::SpdSay, Ferramenta::EspeakNg] {
            let linux = comando_de_fala(Sistema::Linux(ferramenta), &fala(perigoso), 0.5);
            let n = linux.argumentos.len();
            assert_eq!(linux.argumentos[n - 2], "--");
            assert_eq!(linux.argumentos[n - 1], perigoso);
        }

        let windows = comando_de_fala(Sistema::Windows, &fala(perigoso), 0.5);
        assert_eq!(windows.argumentos.last().unwrap(), FALAR_NO_WINDOWS);
        assert!(!windows.argumentos.iter().any(|a| a.contains("rm -rf")));
        assert!(windows
            .ambiente
            .contains(&("VLB_TEXTO".into(), perigoso.into())));
    }

    #[test]
    fn volume_ritmo_e_voz_chegam_na_escala_de_cada_programa() {
        let lenta = Fala {
            texto: "oi".into(),
            voz: Some("Luciana".into()),
            ritmo: 0.5,
        };
        let mac = comando_de_fala(Sistema::Mac, &lenta, 1.0);
        assert_eq!(
            mac.argumentos,
            vec!["-r", "88", "-v", "Luciana", "[[volm 1.00]] oi"]
        );

        let windows = comando_de_fala(Sistema::Windows, &lenta, 0.25);
        assert!(windows
            .ambiente
            .contains(&("VLB_VOLUME".into(), "25".into())));
        assert!(windows
            .ambiente
            .contains(&("VLB_RITMO".into(), "-5".into())));
        assert!(windows
            .ambiente
            .contains(&("VLB_VOZ".into(), "Luciana".into())));

        let spd = comando_de_fala(Sistema::Linux(Ferramenta::SpdSay), &fala("oi"), 0.0);
        assert_eq!(
            spd.argumentos,
            vec!["-w", "-r", "0", "-i", "-100", "-l", "pt-BR", "--", "oi"]
        );
    }

    #[test]
    fn le_as_vozes_do_mac() {
        let saida = "Albert              en_US    # Hello! My name is Albert.\n\
                     Eddy (Português (Brasil)) pt_BR    # Olá! Meu nome é Eddy.\n\
                     Luciana             pt_BR    # Olá, meu nome é Luciana.\n\
                     Joana               pt_PT    # Olá, chamo-me Joana.\n";
        let mut vozes = ler_vozes(Sistema::Mac, saida);
        ordenar(&mut vozes);
        let nomes: Vec<&str> = vozes.iter().map(|v| v.nome.as_str()).collect();
        assert_eq!(
            nomes,
            vec!["Eddy (Português (Brasil))", "Luciana", "Joana", "Albert"]
        );
        assert_eq!(vozes[1].idioma, "pt-BR");
        assert_eq!(padrao(&vozes).as_deref(), Some("Eddy (Português (Brasil))"));
    }

    #[test]
    fn le_as_vozes_do_windows_e_do_linux() {
        let windows = ler_vozes(
            Sistema::Windows,
            "Microsoft Zira Desktop|en-US\r\nMicrosoft Maria Desktop|pt-BR\r\n",
        );
        assert_eq!(windows[1].id, "Microsoft Maria Desktop");
        assert_eq!(windows[1].idioma, "pt-BR");

        let spd = ler_vozes(
            Sistema::Linux(Ferramenta::SpdSay),
            "     NAME                 LANGUAGE    VARIANT\n\
             \x20    Portuguese (Brazil)  pt-BR       none\n\
             \x20    English (America)    en-US       none\n",
        );
        assert_eq!(spd.len(), 2);
        assert_eq!(spd[0].nome, "Portuguese (Brazil)");
        assert_eq!(spd[0].idioma, "pt-BR");

        let espeak = ler_vozes(
            Sistema::Linux(Ferramenta::EspeakNg),
            "Pty Language       Age/Gender VoiceName          File                 Other Languages\n\
             \x205  pt-br           --/M      Portuguese_(Brazil) roa/pt-BR\n",
        );
        assert_eq!(espeak[0].id, "pt-br");
        assert_eq!(espeak[0].nome, "Portuguese (Brazil)");
    }

    #[test]
    fn sem_portugues_a_voz_e_a_do_sistema() {
        let vozes = vec![Voz {
            id: "Albert".into(),
            nome: "Albert".into(),
            idioma: "en-US".into(),
        }];
        assert_eq!(padrao(&vozes), None);
    }
}
