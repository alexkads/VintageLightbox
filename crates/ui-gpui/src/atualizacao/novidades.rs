//! 📣 As novidades da versão nova — e o aviso para quem instalou **compilando**.
//!
//! ## O buraco que isto fecha
//!
//! A faixa de atualização só enxergava o manifesto dos **pacotes**
//! (`latest.json`), e em 25/set/2026 ele só tinha o macOS. A maior parte dos
//! balcões — Windows e Linux — instalou pelo `instalar-vintagelightbox-gpui.cmd`,
//! que **compila o `main`**: para eles não havia pacote, e portanto nunca havia
//! aviso. O Chatbot e os Agendamentos chegaram ao `main` sem que esses balcões
//! tivessem como saber (dono, 25/set: *"vai ter que implementar essa
//! comunicação no app dizendo que tem uma versão nova, e mostrar quais são as
//! novidades e por que ela é tão importante"*).
//!
//! ## O que é "versão nova" para cada instalação
//!
//! 🔑 **A versão nova é a do `main`, para todo mundo.** Lançar pacote exige a
//! máquina de cada sistema; esperar por isso deixava o balcão sem saber
//! (dono: *"não posso ficar dependendo de lembrar de compilar e colocar lá"*).
//!
//! | Instalou por | Há pacote da versão no `latest.json`? | "Atualizar" faz |
//! |---|---|---|
//! | pacote (`release`) | sim | baixa e instala o pacote assinado |
//! | pacote (`release`) | não | roda o instalador, que compila o `main` |
//! | instalador que compila (`instalador`) | — | roda o instalador de novo |
//!
//! A instalação compilada **nunca** recebe pacote: ele cairia ao lado da
//! compilada (Windows) ou por cima dela (Linux), o caso que o README do
//! empacotamento proíbe.
//!
//! 🔑 **O `novidades.json` anda com o `Cargo.toml`.** Um teste prende a versão
//! dele à `CARGO_PKG_VERSION`: subir a versão sem escrever as novidades (ou o
//! contrário) não passa. É o mesmo commit de "Versão 0.1.N".

use serde::Deserialize;

use super::porta::{JeitoDeAtualizar, VersaoNova};

/// O arquivo no repositório. Serve às duas instalações: a compilada o lê no
/// `main`, e a do pacote o usa para mostrar o que mudou.
pub const ARQUIVO: &str = "docs/novidades.json";

/// Onde o app lê as novidades, em ordem.
///
/// - o `raw` do GitHub vale no instante em que o `main` anda (o `make mains`);
/// - o GitHub Pages é a cópia, para quando o `raw` não responder.
///
/// ⚠️ Como os endereços de atualização, **só se acrescenta**: o app instalado
/// só conhece os endereços com que foi compilado.
pub const ENDERECOS: [&str; 2] = [
    "https://raw.githubusercontent.com/alexkads/VintageLightbox/main/docs/novidades.json",
    "https://alexkads.github.io/VintageLightbox/novidades.json",
];

/// O que o `novidades.json` diz, escrito para o operador do balcão.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Novidades {
    pub versao: String,
    /// Uma linha: o que o operador vai notar primeiro.
    pub titulo: String,
    /// Atualização que não deve esperar — a faixa fica em destaque.
    #[serde(default)]
    pub importante: bool,
    /// O que mudou, um item por linha.
    pub novidades: Vec<String>,
    /// Por que vale atualizar agora.
    pub por_que_atualizar: String,
}

/// Lê o arquivo. Torto, sem versão ou sem título é `None`: melhor não avisar
/// do que avisar em branco.
pub fn ler(texto: &str) -> Option<Novidades> {
    let novidades: Novidades = serde_json::from_str(texto).ok()?;
    let valido =
        versao_em_numeros(&novidades.versao).is_some() && !novidades.titulo.trim().is_empty();
    valido.then_some(novidades)
}

fn versao_em_numeros(versao: &str) -> Option<Vec<u64>> {
    let limpa = versao.trim().trim_start_matches('v');
    let numeros: Option<Vec<u64>> = limpa
        .split('.')
        .map(|parte| parte.parse::<u64>().ok())
        .collect();
    numeros.filter(|n| !n.is_empty())
}

/// A `candidata` é maior que a `atual`? `0.1.13 > 0.1.9` (número a número,
/// e não como texto). Ilegível nunca é "mais nova".
pub fn mais_nova(candidata: &str, atual: &str) -> bool {
    match (versao_em_numeros(candidata), versao_em_numeros(atual)) {
        (Some(mut c), Some(mut a)) => {
            let tamanho = c.len().max(a.len());
            c.resize(tamanho, 0);
            a.resize(tamanho, 0);
            c > a
        }
        _ => false,
    }
}

/// Como este binário foi instalado — gravado nele pelo `build.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JeitoDaInstalacao {
    /// O instalador do balcão, que compila o `main` (perfil `instalador`).
    Compilado,
    /// O pacote assinado (perfil `release`).
    Pacote,
    /// Quem desenvolve (`debug`).
    Desenvolvimento,
}

impl JeitoDaInstalacao {
    pub fn do_texto(texto: &str) -> JeitoDaInstalacao {
        match texto {
            "compilado" => JeitoDaInstalacao::Compilado,
            "pacote" => JeitoDaInstalacao::Pacote,
            _ => JeitoDaInstalacao::Desenvolvimento,
        }
    }
}

pub fn jeito_desta_instalacao() -> JeitoDaInstalacao {
    JeitoDaInstalacao::do_texto(env!("VLB_JEITO_DE_INSTALAR"))
}

/// O aviso para quem instalou compilando: há versão nova no `main`?
pub fn aviso_do_main(atual: &str, novidades: Option<Novidades>) -> Option<VersaoNova> {
    let novidades = novidades?;
    if !mais_nova(&novidades.versao, atual) {
        return None;
    }
    Some(VersaoNova {
        versao: novidades.versao.clone(),
        notas: Some(novidades.titulo.clone()),
        jeito: JeitoDeAtualizar::Compilar,
        novidades: Some(novidades),
    })
}

/// As novidades só acompanham o pacote da **mesma** versão: as de outra
/// versão contariam o que o pacote não traz.
pub fn para_o_pacote(versao_do_pacote: &str, novidades: Option<Novidades>) -> Option<Novidades> {
    novidades.filter(|n| {
        versao_em_numeros(&n.versao).is_some()
            && versao_em_numeros(&n.versao) == versao_em_numeros(versao_do_pacote)
    })
}

/// Busca o arquivo nos [`ENDERECOS`], em ordem. **Bloqueia**: chamar fora da
/// thread da interface (a procura já roda numa thread própria).
pub fn buscar() -> Option<Novidades> {
    let cliente = cargo_packager_updater::reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    ENDERECOS.iter().find_map(|endereco| {
        let resposta = cliente.get(*endereco).send().ok()?;
        if !resposta.status().is_success() {
            eprintln!("[novidades] {endereco} respondeu {}", resposta.status());
            return None;
        }
        ler(&resposta.text().ok()?)
    })
}

/// O que o operador faz para atualizar, neste sistema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComoAtualizar {
    pub passos: Vec<&'static str>,
    /// Se a compilação pelo app falhar: a linha para colar no Terminal (Linux
    /// e macOS).
    pub comando: Option<&'static str>,
    /// Se a compilação pelo app falhar: de onde baixar o instalador (Windows).
    pub baixar: Option<&'static str>,
}

/// A linha de instalação do Linux e do macOS — a mesma do site de documentação.
pub const COMANDO_DO_INSTALADOR: &str = "curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd | sh";

/// O instalador do Windows — o mesmo link do site de documentação.
pub const INSTALADOR_DO_WINDOWS: &str = "https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-gpui.cmd";

pub fn como_atualizar(jeito: JeitoDeAtualizar) -> ComoAtualizar {
    match jeito {
        JeitoDeAtualizar::Pacote => ComoAtualizar {
            passos: vec![
                "Clique em Atualizar: o app baixa a versão nova e confere a assinatura antes de instalar.",
                "No fim, clique em Reabrir agora.",
            ],
            comando: None,
            baixar: None,
        },
        // O app roda o instalador sozinho; o comando e o link ficam como o
        // caminho à mão, para quando a compilação pelo app falhar.
        JeitoDeAtualizar::Compilar => ComoAtualizar {
            passos: vec![
                "Clique em Atualizar: o app baixa o código da versão nova e compila nesta máquina, com o mesmo instalador de sempre.",
                "Leva alguns minutos. Você pode continuar trabalhando enquanto isso.",
                "No fim, clique em Reabrir agora. O catálogo e as fotos continuam lá.",
            ],
            comando: (!cfg!(target_os = "windows")).then_some(COMANDO_DO_INSTALADOR),
            baixar: cfg!(target_os = "windows").then_some(INSTALADOR_DO_WINDOWS),
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const EXEMPLO: &str = r#"{
        "versao": "0.1.13",
        "titulo": "Chatbot e Agendamentos",
        "importante": true,
        "novidades": ["O chatbot no app", "A agenda no app"],
        "por_que_atualizar": "Para não perder cliente."
    }"#;

    #[test]
    fn le_o_arquivo_e_recusa_o_torto() {
        let n = ler(EXEMPLO).unwrap();
        assert_eq!(n.versao, "0.1.13");
        assert!(n.importante);
        assert_eq!(n.novidades.len(), 2);
        assert!(ler("{").is_none());
        assert!(
            ler(r#"{"versao":"x","titulo":"t","novidades":[],"por_que_atualizar":""}"#).is_none()
        );
        assert!(
            ler(r#"{"versao":"1.0","titulo":"  ","novidades":[],"por_que_atualizar":""}"#)
                .is_none()
        );
        let sem_importante =
            ler(r#"{"versao":"1.0","titulo":"t","novidades":[],"por_que_atualizar":"p"}"#).unwrap();
        assert!(!sem_importante.importante, "ausente é não importante");
    }

    #[test]
    fn compara_versao_numero_a_numero() {
        assert!(mais_nova("0.1.13", "0.1.9"), "13 > 9, e não como texto");
        assert!(mais_nova("0.2.0", "0.1.99"));
        assert!(mais_nova("1.0", "0.9.9"));
        assert!(mais_nova("v0.1.13", "0.1.12"));
        assert!(!mais_nova("0.1.12", "0.1.12"));
        assert!(!mais_nova("0.1.12", "0.1.13"));
        assert!(!mais_nova("0.1", "0.1.0"), "0.1 é 0.1.0");
        assert!(!mais_nova("lixo", "0.1.0"));
        assert!(!mais_nova("0.2.0", "lixo"));
    }

    #[test]
    fn o_jeito_de_instalar_vem_do_perfil() {
        assert_eq!(
            JeitoDaInstalacao::do_texto("compilado"),
            JeitoDaInstalacao::Compilado
        );
        assert_eq!(
            JeitoDaInstalacao::do_texto("pacote"),
            JeitoDaInstalacao::Pacote
        );
        assert_eq!(
            JeitoDaInstalacao::do_texto("desenvolvimento"),
            JeitoDaInstalacao::Desenvolvimento
        );
        // Os testes rodam no perfil de desenvolvimento.
        assert_eq!(jeito_desta_instalacao(), JeitoDaInstalacao::Desenvolvimento);
    }

    #[test]
    fn a_instalacao_compilada_e_avisada_da_versao_do_main() {
        let aviso = aviso_do_main("0.1.12", ler(EXEMPLO)).expect("há versão nova");
        assert_eq!(aviso.versao, "0.1.13");
        assert_eq!(aviso.jeito, JeitoDeAtualizar::Compilar);
        assert_eq!(aviso.notas.as_deref(), Some("Chatbot e Agendamentos"));
        assert!(aviso.novidades.unwrap().importante);
        assert!(
            aviso_do_main("0.1.13", ler(EXEMPLO)).is_none(),
            "já está nela"
        );
        assert!(
            aviso_do_main("0.1.14", ler(EXEMPLO)).is_none(),
            "está à frente"
        );
        assert!(
            aviso_do_main("0.1.12", None).is_none(),
            "sem arquivo, sem aviso"
        );
    }

    #[test]
    fn as_novidades_so_acompanham_o_pacote_da_mesma_versao() {
        assert!(para_o_pacote("0.1.13", ler(EXEMPLO)).is_some());
        assert!(para_o_pacote("v0.1.13", ler(EXEMPLO)).is_some());
        assert!(para_o_pacote("0.1.14", ler(EXEMPLO)).is_none());
        assert!(para_o_pacote("0.1.13", None).is_none());
    }

    #[test]
    fn como_atualizar_em_cada_jeito() {
        let pacote = como_atualizar(JeitoDeAtualizar::Pacote);
        assert!(pacote.comando.is_none() && pacote.baixar.is_none());
        let compilado = como_atualizar(JeitoDeAtualizar::Compilar);
        assert_eq!(compilado.passos.len(), 3);
        assert!(compilado.passos[0].contains("compila nesta máquina"));
        if cfg!(target_os = "windows") {
            assert_eq!(compilado.baixar, Some(INSTALADOR_DO_WINDOWS));
        } else {
            assert_eq!(compilado.comando, Some(COMANDO_DO_INSTALADOR));
        }
    }

    /// 🔑 **O `novidades.json` do repositório é da versão do `Cargo.toml`.**
    /// Subir a versão sem escrever as novidades (ou o contrário) faz os balcões
    /// ou não serem avisados, ou serem avisados do que não existe.
    #[test]
    fn o_novidades_json_anda_com_a_versao_do_cargo() {
        let arquivo = include_str!("../../../../docs/novidades.json");
        let n = ler(arquivo).expect("docs/novidades.json legível, com versão e título");
        assert_eq!(
            n.versao,
            env!("CARGO_PKG_VERSION"),
            "docs/novidades.json e o [workspace.package] version do Cargo.toml andam juntos"
        );
        assert!(!n.novidades.is_empty(), "novidade sem item não diz nada");
        assert!(!n.por_que_atualizar.trim().is_empty());
    }

    /// Os endereços são https e terminam no arquivo — e o `raw` do `main` vem
    /// primeiro, porque vale no instante do `make mains`.
    #[test]
    fn os_enderecos_das_novidades() {
        for e in ENDERECOS {
            assert!(
                e.starts_with("https://") && e.ends_with("/novidades.json"),
                "{e}"
            );
        }
        assert!(
            ENDERECOS[0].contains("raw.githubusercontent.com") && ENDERECOS[0].contains("/main/")
        );
    }
}
