//! 🔧 A atualização que **compila nesta máquina**, sozinha — o mesmo
//! instalador que o balcão sempre rodou, agora rodado pelo app.
//!
//! ## O pedido (dono, 25/set/2026)
//!
//! *"O sistema tem que detectar que existe uma versão nova … se ele não
//! conseguir baixar ela compilada, o sistema tem que conseguir rodar o script
//! também, porque eu não posso ficar dependendo de lembrar de compilar e
//! colocar lá no repositório."* E: *"tudo automático, tudo claro, e não pode
//! corromper a aplicação que já está funcionando. Se por algum motivo falhar,
//! ele tem a aplicação anterior, e ela tem que continuar funcionando."*
//!
//! ## Por que nada se corrompe
//!
//! | Etapa | Se falhar |
//! |---|---|
//! | baixar o instalador | nada foi tocado; o app segue |
//! | compilar (pasta própria, `~/.vintagelightbox/target-gpui`) | nada foi tocado |
//! | o instalador **roda o binário novo** (`--versao`) antes de trocar | nada foi tocado |
//! | a troca: o instalado vira `.anterior`, o novo entra por renomeação | o instalador devolve o anterior |
//! | o app confere a versão do binário instalado | a faixa diz o que houve; o app aberto segue |
//!
//! 🚨 **Nunca `curl … | sh`.** Num encadeamento, o `curl` que falha entrega um
//! script vazio ao `sh`, que sai com **sucesso** sem ter feito nada. Aqui o
//! instalador é baixado para um arquivo, conferido, e só então rodado.
//!
//! ## Sem laço de falha, sem dois ao mesmo tempo
//!
//! - uma versão que falhou não é tentada de novo **sozinha** por
//!   [`ESPERA_DEPOIS_DE_FALHAR`] (o botão "Tentar de novo" continua valendo);
//! - uma trava em `~/.vintagelightbox` impede dois instaladores juntos (o app
//!   reaberto no meio da compilação não começa outra).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::novidades::{self, JeitoDaInstalacao, Novidades};
use super::porta::{JeitoDeAtualizar, VersaoNova};

/// Quanto esperar para tentar de novo, sozinho, uma versão que falhou.
pub const ESPERA_DEPOIS_DE_FALHAR: Duration = Duration::from_secs(24 * 3600);
/// Uma trava mais velha que isto é de um instalador que morreu sem apagá-la.
pub const TRAVA_VENCE_EM: Duration = Duration::from_secs(3 * 3600);

/// O instalador que o app baixa e roda — o mesmo da documentação.
pub const INSTALADOR: &str =
    "https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd";

// ── A decisão ──────────────────────────────────────────────────────────────

/// O que o app faz ao saber das versões.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decisao {
    /// Está em dia (ou não há como saber): silêncio.
    Nada,
    /// Há versão nova. `automatico` diz se a compilação começa sozinha.
    Avisar {
        versao: VersaoNova,
        automatico: bool,
    },
}

/// Lembrança de uma atualização que falhou (`atualizacao.json`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memoria {
    /// A versão que falhou por último.
    pub falhou: Option<String>,
    /// Quando, em segundos desde 1970.
    pub quando: Option<u64>,
}

/// A regra inteira, sem rede e sem disco — o que [`super::porta`] faz com o
/// que achou.
///
/// - `pacote`: a versão que o `latest.json` tem **para esta plataforma**
///   (`None` se não tem pacote nenhum para ela);
/// - `main`: o `novidades.json` do `main`;
/// - `ocupado`: há um instalador rodando (a trava).
pub fn decidir(
    jeito: JeitoDaInstalacao,
    atual: &str,
    pacote: Option<VersaoNova>,
    main: Option<Novidades>,
    memoria: &Memoria,
    ocupado: bool,
    agora: u64,
) -> Decisao {
    // Quem desenvolve atualiza por `git pull`: só o pacote, como sempre foi.
    if jeito == JeitoDaInstalacao::Desenvolvimento {
        return match pacote {
            Some(versao) => Decisao::Avisar {
                versao,
                automatico: false,
            },
            None => Decisao::Nada,
        };
    }
    // 🔑 Instalado por pacote e há pacote da versão nova: o caminho assinado.
    if jeito == JeitoDaInstalacao::Pacote {
        if let Some(mut versao) = pacote {
            versao.novidades = novidades::para_o_pacote(&versao.versao, main);
            return Decisao::Avisar {
                versao,
                automatico: false,
            };
        }
    }
    // O resto compila: a instalação compilada sempre, e o pacote que não tem
    // pacote da versão nova.
    let Some(versao) = novidades::aviso_do_main(atual, main) else {
        return Decisao::Nada;
    };
    let falhou_ha_pouco = memoria.falhou.as_deref() == Some(versao.versao.as_str())
        && memoria
            .quando
            .is_some_and(|q| agora.saturating_sub(q) < ESPERA_DEPOIS_DE_FALHAR.as_secs());
    Decisao::Avisar {
        automatico: !ocupado && !falhou_ha_pouco,
        versao,
    }
}

// ── O disco ────────────────────────────────────────────────────────────────

/// `~/.vintagelightbox` — a mesma casa do instalador (`$HOME/.vintagelightbox`
/// e `%USERPROFILE%\.vintagelightbox`).
pub fn casa() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(|h| PathBuf::from(h).join(".vintagelightbox"))
}

pub fn ler_memoria(arquivo: &Path) -> Memoria {
    std::fs::read(arquivo)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn guardar_memoria(arquivo: &Path, memoria: &Memoria) {
    if let Some(pai) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    if let Ok(texto) = serde_json::to_vec_pretty(memoria) {
        let _ = std::fs::write(arquivo, texto);
    }
}

/// Há um instalador rodando? A trava guarda a hora em que começou.
pub fn ocupado(trava: &Path, agora: u64) -> bool {
    std::fs::read_to_string(trava)
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .is_some_and(|inicio| agora.saturating_sub(inicio) < TRAVA_VENCE_EM.as_secs())
}

pub fn agora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── O instalador ───────────────────────────────────────────────────────────

/// O arquivo baixado é mesmo o instalador inteiro? O começo do bloco do
/// `cmd`, as duas marcas do PowerShell e **o fim do arquivo**: a parte do `sh`
/// é um grupo `{ … }` que termina na linha "Apagar o cache é seguro" e no `}`
/// que o fecha. Um HTML de erro, um arquivo vazio ou cortado em qualquer
/// ponto não passa.
pub fn instalador_inteiro(texto: &str) -> bool {
    let fim = texto.trim_end();
    let ultima_linha = fim
        .strip_suffix('}')
        .map(str::trim_end)
        .and_then(|antes| antes.lines().last());
    texto.starts_with(":<<\"::FIM-DO-CMD\"")
        && texto.contains("\n#==POWERSHELL==")
        && texto.contains("\n#==FIM-POWERSHELL==")
        && ultima_linha.is_some_and(|l| l.contains("Apagar o cache é seguro"))
}

/// Como rodar o instalador neste sistema: `(programa, argumentos)`.
///
/// - Windows: o próprio `.cmd`, pelo `cmd /c` — ele baixa o bloco PowerShell
///   mais novo do `main` e roda. `VLB_SEM_PAUSA=1` tira o `pause` do fim, que
///   prenderia um processo sem janela para sempre.
/// - Linux e macOS: `nice -n 10 sh <arquivo>` — prioridade baixa, para a
///   compilação não disputar o processador com o operador.
pub fn comando(arquivo: &Path) -> (String, Vec<String>) {
    let caminho = arquivo.display().to_string();
    if cfg!(windows) {
        ("cmd".into(), vec!["/c".into(), caminho])
    } else {
        (
            "nice".into(),
            vec!["-n".into(), "10".into(), "sh".into(), caminho],
        )
    }
}

/// Onde o instalador põe o binário — é ele que se confere no fim, e é ele que
/// o "Reabrir agora" abre.
pub fn binario_instalado() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|l| {
            PathBuf::from(l)
                .join("Programs")
                .join("VintageLightbox-GPUI")
                .join("VintageLightbox-GPUI.exe")
        })
    } else if cfg!(target_os = "macos") {
        let app = "VintageLightbox (Zed GPUI).app/Contents/MacOS/ui-gpui";
        let global = PathBuf::from("/Applications").join(app);
        if global.exists() {
            return Some(global);
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Applications").join(app))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin/vintagelightbox-gpui"))
    }
}

/// A etapa que o instalador anunciou, numa linha do registro: `> texto` no
/// PowerShell e `▸ texto` no `sh`, com ou sem as cores do terminal.
pub fn etapa(linha: &str) -> Option<String> {
    let limpa = sem_cores(linha);
    let limpa = limpa.trim();
    let texto = limpa
        .strip_prefix("▸ ")
        .or_else(|| limpa.strip_prefix("> "))?
        .trim();
    (!texto.is_empty()).then(|| texto.to_string())
}

fn sem_cores(texto: &str) -> String {
    let mut fora = String::with_capacity(texto.len());
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // `ESC [ … letra`
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        fora.push(c);
    }
    fora
}

/// A saída de `--versao` do binário instalado: a primeira linha, limpa.
pub fn versao_respondida(saida: &[u8]) -> Option<String> {
    let texto = String::from_utf8_lossy(saida);
    let linha = texto.lines().next()?.trim();
    (!linha.is_empty()).then(|| linha.to_string())
}

/// O que a compilação diz à faixa enquanto roda, e no fim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Andamento {
    Etapa(String),
    Instalada(String),
    Falhou(String),
}

/// Baixa o instalador, roda, acompanha o registro e confere o resultado.
/// **Bloqueia** — chamar numa thread própria.
pub fn rodar(versao_esperada: &str, avisar: &dyn Fn(Andamento)) {
    let Some(casa) = casa() else {
        avisar(Andamento::Falhou(
            "não achei a pasta pessoal deste usuário".into(),
        ));
        return;
    };
    let registros = casa.join("registros");
    let _ = std::fs::create_dir_all(&registros);
    let trava = casa.join("atualizando.trava");
    let memoria_arquivo = casa.join("atualizacao.json");
    let _ = std::fs::write(&trava, agora().to_string());
    let registro = registros.join(format!("atualizacao-{}.log", agora()));

    let desfecho = rodar_travado(versao_esperada, &casa, &registro, avisar);
    let _ = std::fs::remove_file(&trava);
    match desfecho {
        Ok(instalada) => {
            guardar_memoria(&memoria_arquivo, &Memoria::default());
            avisar(Andamento::Instalada(instalada));
        }
        Err(motivo) => {
            guardar_memoria(
                &memoria_arquivo,
                &Memoria {
                    falhou: Some(versao_esperada.into()),
                    quando: Some(agora()),
                },
            );
            avisar(Andamento::Falhou(format!(
                "{motivo}. O registro está em {}",
                registro.display()
            )));
        }
    }
}

fn rodar_travado(
    versao_esperada: &str,
    casa: &Path,
    registro: &Path,
    avisar: &dyn Fn(Andamento),
) -> Result<String, String> {
    avisar(Andamento::Etapa("baixando o instalador".into()));
    let texto = cargo_packager_updater::reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .and_then(|c| c.get(INSTALADOR).send())
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("não consegui baixar o instalador ({e})"))?;
    if !instalador_inteiro(&texto) {
        return Err("o instalador baixado veio incompleto".into());
    }
    let arquivo = casa.join("instalar-vintagelightbox-gpui.cmd");
    std::fs::write(&arquivo, &texto)
        .map_err(|e| format!("não consegui guardar o instalador ({e})"))?;

    let saida = std::fs::File::create(registro)
        .map_err(|e| format!("não consegui abrir o registro ({e})"))?;
    let erros = saida
        .try_clone()
        .map_err(|e| format!("não consegui abrir o registro ({e})"))?;
    let (programa, argumentos) = comando(&arquivo);
    let mut processo = std::process::Command::new(&programa);
    processo
        .args(&argumentos)
        .env("VLB_SEM_PAUSA", "1")
        .stdin(std::process::Stdio::null())
        .stdout(saida)
        .stderr(erros);
    desanexar(&mut processo);
    let mut filho = processo
        .spawn()
        .map_err(|e| format!("não consegui rodar o instalador ({e})"))?;

    // Acompanha o registro enquanto o instalador roda: cada etapa anunciada
    // vira a frase da faixa.
    let mut lidas = 0usize;
    let codigo = loop {
        if let Ok(texto) = std::fs::read_to_string(registro) {
            let linhas: Vec<&str> = texto.lines().collect();
            for linha in &linhas[lidas.min(linhas.len())..] {
                if let Some(e) = etapa(linha) {
                    avisar(Andamento::Etapa(e));
                }
            }
            lidas = linhas.len();
        }
        match filho.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(e) => return Err(format!("perdi o instalador de vista ({e})")),
        }
    };
    if !codigo.success() {
        return Err(format!(
            "o instalador parou ({}); a versão que está aberta continua instalada",
            codigo
        ));
    }

    // 🔑 Só é "instalada" se o binário no lugar responde a versão.
    let binario = binario_instalado().ok_or("não sei onde o instalador põe o app")?;
    let resposta = std::process::Command::new(&binario)
        .arg("--versao")
        .output()
        .map_err(|e| format!("o app instalado não abriu ({e})"))?;
    let instalada =
        versao_respondida(&resposta.stdout).ok_or("o app instalado não respondeu a versão")?;
    if novidades::mais_nova(versao_esperada, &instalada) {
        return Err(format!(
            "o instalador terminou, mas o app instalado é a versão {instalada}, e não a {versao_esperada}"
        ));
    }
    Ok(instalada)
}

/// O instalador sobrevive ao app: fechar o VintageLightbox no meio da
/// compilação não a interrompe — a próxima abertura já é a versão nova.
fn desanexar(processo: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        processo.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS | CREATE_NEW_PROCESS_GROUP
        processo.creation_flags(0x0800_0000 | 0x0000_4000 | 0x0000_0200);
    }
    let _ = processo;
}

/// "Reabrir agora" depois de compilar: abre **o binário instalado** e sai.
///
/// ⚠️ Não é o `current_exe`: no Linux o arquivo foi trocado, e o caminho do
/// processo aberto aponta para o antigo (`… (deleted)`); no macOS quem se abre
/// é o `.app`.
pub fn reabrir_o_instalado() {
    let Some(binario) = binario_instalado() else {
        return;
    };
    #[cfg(target_os = "macos")]
    if let Some(app) = binario.ancestors().nth(3) {
        let _ = std::process::Command::new("open")
            .arg("-n")
            .arg(app)
            .spawn();
        std::process::exit(0);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = std::process::Command::new(&binario).spawn();
        std::process::exit(0);
    }
    #[allow(unreachable_code)]
    let _ = binario;
}

/// Uma versão nova que compila, para a faixa (usado quando o "Tentar de novo"
/// precisa do aviso de volta).
pub fn aviso_de_compilar(versao: &str, novidades: Option<Novidades>) -> VersaoNova {
    VersaoNova {
        versao: versao.into(),
        notas: novidades.as_ref().map(|n| n.titulo.clone()),
        jeito: JeitoDeAtualizar::Compilar,
        novidades,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn main(versao: &str) -> Option<Novidades> {
        Some(Novidades {
            versao: versao.into(),
            titulo: "Chatbot e Agendamentos".into(),
            importante: true,
            novidades: vec!["o chatbot".into()],
            por_que_atualizar: "para não perder cliente".into(),
        })
    }

    fn pacote(versao: &str) -> Option<VersaoNova> {
        Some(VersaoNova {
            versao: versao.into(),
            notas: None,
            jeito: JeitoDeAtualizar::Pacote,
            novidades: None,
        })
    }

    const AGORA: u64 = 1_800_000_000;

    #[test]
    fn a_instalacao_compilada_compila_sozinha_a_versao_do_main() {
        let d = decidir(
            JeitoDaInstalacao::Compilado,
            "0.1.12",
            pacote("0.1.13"),
            main("0.1.13"),
            &Memoria::default(),
            false,
            AGORA,
        );
        let Decisao::Avisar { versao, automatico } = d else {
            panic!("devia avisar")
        };
        assert!(automatico, "tudo automático");
        assert_eq!(
            versao.jeito,
            JeitoDeAtualizar::Compilar,
            "compilada nunca recebe pacote"
        );
        assert_eq!(versao.versao, "0.1.13");
        assert!(versao.novidades.unwrap().importante);
    }

    #[test]
    fn em_dia_nao_avisa() {
        for jeito in [JeitoDaInstalacao::Compilado, JeitoDaInstalacao::Pacote] {
            assert_eq!(
                decidir(
                    jeito,
                    "0.1.13",
                    None,
                    main("0.1.13"),
                    &Memoria::default(),
                    false,
                    AGORA
                ),
                Decisao::Nada
            );
        }
        assert_eq!(
            decidir(
                JeitoDaInstalacao::Compilado,
                "0.1.12",
                None,
                None,
                &Memoria::default(),
                false,
                AGORA
            ),
            Decisao::Nada,
            "sem o arquivo do main não há o que dizer"
        );
    }

    #[test]
    fn o_pacote_usa_o_pacote_quando_ha_e_compila_quando_nao_ha() {
        let com_pacote = decidir(
            JeitoDaInstalacao::Pacote,
            "0.1.12",
            pacote("0.1.13"),
            main("0.1.13"),
            &Memoria::default(),
            false,
            AGORA,
        );
        let Decisao::Avisar { versao, automatico } = com_pacote else {
            panic!()
        };
        assert_eq!(versao.jeito, JeitoDeAtualizar::Pacote);
        assert!(!automatico, "o pacote instala no clique");
        assert!(
            versao.novidades.is_some(),
            "as novidades da mesma versão vão junto"
        );

        let sem_pacote = decidir(
            JeitoDaInstalacao::Pacote,
            "0.1.12",
            None,
            main("0.1.13"),
            &Memoria::default(),
            false,
            AGORA,
        );
        let Decisao::Avisar { versao, automatico } = sem_pacote else {
            panic!("sem pacote, o main ainda avisa")
        };
        assert_eq!(versao.jeito, JeitoDeAtualizar::Compilar);
        assert!(automatico);
    }

    #[test]
    fn a_falha_recente_nao_tenta_de_novo_sozinha_mas_avisa() {
        let memoria = Memoria {
            falhou: Some("0.1.13".into()),
            quando: Some(AGORA - 3600),
        };
        let Decisao::Avisar { automatico, .. } = decidir(
            JeitoDaInstalacao::Compilado,
            "0.1.12",
            None,
            main("0.1.13"),
            &memoria,
            false,
            AGORA,
        ) else {
            panic!()
        };
        assert!(!automatico, "falhou há uma hora: espera o botão");

        let ontem = Memoria {
            falhou: Some("0.1.13".into()),
            quando: Some(AGORA - 25 * 3600),
        };
        let Decisao::Avisar { automatico, .. } = decidir(
            JeitoDaInstalacao::Compilado,
            "0.1.12",
            None,
            main("0.1.13"),
            &ontem,
            false,
            AGORA,
        ) else {
            panic!()
        };
        assert!(automatico, "passadas 24 h, tenta de novo");

        let outra = Memoria {
            falhou: Some("0.1.13".into()),
            quando: Some(AGORA),
        };
        let Decisao::Avisar { automatico, .. } = decidir(
            JeitoDaInstalacao::Compilado,
            "0.1.12",
            None,
            main("0.1.14"),
            &outra,
            false,
            AGORA,
        ) else {
            panic!()
        };
        assert!(automatico, "a falha de uma versão não segura a seguinte");
    }

    #[test]
    fn com_um_instalador_rodando_nao_comeca_outro() {
        let Decisao::Avisar { automatico, .. } = decidir(
            JeitoDaInstalacao::Compilado,
            "0.1.12",
            None,
            main("0.1.13"),
            &Memoria::default(),
            true,
            AGORA,
        ) else {
            panic!()
        };
        assert!(!automatico);
    }

    #[test]
    fn quem_desenvolve_so_ve_o_pacote() {
        assert_eq!(
            decidir(
                JeitoDaInstalacao::Desenvolvimento,
                "0.1.12",
                None,
                main("0.1.13"),
                &Memoria::default(),
                false,
                AGORA
            ),
            Decisao::Nada,
            "desenvolvimento não compila por cima de si"
        );
        let Decisao::Avisar { automatico, .. } = decidir(
            JeitoDaInstalacao::Desenvolvimento,
            "0.1.12",
            pacote("0.1.13"),
            None,
            &Memoria::default(),
            false,
            AGORA,
        ) else {
            panic!()
        };
        assert!(!automatico);
    }

    #[test]
    fn a_memoria_e_a_trava_no_disco() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("sub/atualizacao.json");
        assert_eq!(ler_memoria(&arquivo), Memoria::default());
        let m = Memoria {
            falhou: Some("0.1.13".into()),
            quando: Some(AGORA),
        };
        guardar_memoria(&arquivo, &m);
        assert_eq!(ler_memoria(&arquivo), m);
        std::fs::write(&arquivo, b"{torto").unwrap();
        assert_eq!(ler_memoria(&arquivo), Memoria::default());

        let trava = dir.path().join("atualizando.trava");
        assert!(!ocupado(&trava, AGORA), "sem trava, livre");
        std::fs::write(&trava, (AGORA - 60).to_string()).unwrap();
        assert!(ocupado(&trava, AGORA));
        std::fs::write(&trava, (AGORA - 4 * 3600).to_string()).unwrap();
        assert!(!ocupado(&trava, AGORA), "trava velha é de instalador morto");
        std::fs::write(&trava, "lixo").unwrap();
        assert!(!ocupado(&trava, AGORA));
    }

    #[test]
    fn so_o_instalador_inteiro_passa() {
        let real = include_str!("../../../../scripts/instalar-vintagelightbox-gpui.cmd");
        assert!(instalador_inteiro(real), "o instalador do repositório");
        assert!(!instalador_inteiro(""));
        assert!(!instalador_inteiro("<html>404</html>"));
        // Cortado em qualquer ponto — no meio, e a um passo do fim.
        for corte in [real.len() / 2, real.len() * 9 / 10, real.len() - 3] {
            let corte = (0..=corte)
                .rev()
                .find(|i| real.is_char_boundary(*i))
                .unwrap();
            assert!(!instalador_inteiro(&real[..corte]), "cortado em {corte}");
        }
    }

    #[test]
    fn o_comando_de_cada_sistema() {
        let (programa, args) = comando(Path::new("/tmp/x.cmd"));
        if cfg!(windows) {
            assert_eq!(programa, "cmd");
            assert_eq!(args, ["/c", "/tmp/x.cmd"]);
        } else {
            assert_eq!(programa, "nice", "prioridade baixa");
            assert_eq!(args, ["-n", "10", "sh", "/tmp/x.cmd"]);
        }
    }

    #[test]
    fn as_etapas_do_instalador_viram_frase() {
        assert_eq!(
            etapa("\u{1b}[1;36m▸ compilando\u{1b}[0m").as_deref(),
            Some("compilando")
        );
        assert_eq!(
            etapa("> instalando em C:\\x").as_deref(),
            Some("instalando em C:\\x")
        );
        assert_eq!(etapa("   Compiling gpui v0.2.2"), None);
        assert_eq!(etapa("▸ "), None);
        assert_eq!(versao_respondida(b"0.1.13\n").as_deref(), Some("0.1.13"));
        assert_eq!(versao_respondida(b"  \n"), None);
    }

    #[test]
    fn o_binario_instalado_fica_onde_o_instalador_poe() {
        let caminho = binario_instalado()
            .expect("há casa neste sistema")
            .display()
            .to_string();
        if cfg!(windows) {
            assert!(caminho.ends_with("VintageLightbox-GPUI\\VintageLightbox-GPUI.exe"));
        } else if cfg!(target_os = "macos") {
            assert!(caminho.ends_with("VintageLightbox (Zed GPUI).app/Contents/MacOS/ui-gpui"));
        } else {
            assert!(caminho.ends_with(".local/bin/vintagelightbox-gpui"));
        }
    }
}
