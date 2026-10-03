//! A predefinição fora do app: o arquivo `.rfpreset` e o pacote `.zip`.
//!
//! Pedido do dono (3/out/2026): *"Possibilite a opção de exportar os presets
//! individualmente, grupos de presets ou todos. Quando for vários exporte
//! zipado. Use algum tipo de formato organizado com extensão própria. Faça um
//! mecanismo similar ao Lightroom."*
//!
//! | Gesto | O Lightroom grava | Aqui |
//! |---|---|---|
//! | botão direito na predefinição → Exportar… | um `.xmp` | um `.rfpreset` |
//! | botão direito na pasta → Exportar grupo… | um `.zip` com os `.xmp` | um `.zip` com os `.rfpreset` |
//! | (não tem) | — | todas, num `.zip` com uma pasta por grupo |
//!
//! E a volta é a do Lightroom: o mesmo "Importar" lê o `.rfpreset`, o `.zip`
//! (com `.rfpreset`, `.xmp` ou `.lrtemplate` dentro) e os do Lightroom soltos.
//!
//! # O `.rfpreset`
//!
//! 🔑 **JSON legível, com os nomes do motor** — os de `Ajustes::NOMES`, que são
//! os do `nomes.json` do site e os da coluna `ajustes` da API. Abre em qualquer
//! editor de texto, e quem lê entende o que a predefinição mexe:
//!
//! ```json
//! {
//!   "formato": "recordarfotos.predefinicao",
//!   "versao": 1,
//!   "nome": "Hora dourada",
//!   "grupo": "Do sistema",
//!   "recomeca_do_neutro": true,
//!   "ajustes": { "exposure": 0.15, "temperature": 2.5 },
//!   "exportada_por": "VintageLightbox 0.1.73"
//! }
//! ```
//!
//! ⚠️ **`recomeca_do_neutro` é a regra de aplicar, e não um detalhe.** Uma
//! predefinição que recomeça do neutro e chega sem ela vira uma que soma — e
//! "Preto e branco" sobre uma sépia sai âmbar (`Preset::replaces`). No arquivo
//! ela vai **enxuta** (só o que sai do neutro, mais a marca); na volta, como as
//! do operador não guardam a marca no banco, ela entra **inteira** — os campos
//! que faltam no neutro —, que é o "Zerar os outros ajustes ao aplicar" de
//! quem salva. Aplicar dá a mesma foto.
//!
//! # O pacote
//!
//! Um `.zip` comum, que o Finder e o Explorer abrem: um `.rfpreset` por
//! predefinição e um `manifesto.json` com a lista. Em "todas", uma pasta por
//! grupo ("Minhas", "Do sistema", "LRs"). As "Favoritas" não ganham pasta em
//! "todas": elas já estão no grupo de origem, e repetir importaria duas vezes.

use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use domain::entities::preset::PresetAdjustments;
use domain::entities::Preset;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::revelacao::lightroom::Arquivo;
use crate::revelacao::processador::Ajustes;

/// A extensão de uma predefinição — **R**ecordar**F**otos *preset*.
pub const EXTENSAO: &str = "rfpreset";

/// O que o campo `formato` de um `.rfpreset` diz. É ele, e não a extensão,
/// que decide a leitura: um arquivo renomeado continua sendo reconhecido.
pub const FORMATO: &str = "recordarfotos.predefinicao";

/// O `formato` do `manifesto.json` de um pacote.
pub const FORMATO_DO_PACOTE: &str = "recordarfotos.pacote-de-predefinicoes";

/// A versão do formato que este app escreve.
pub const VERSAO: u32 = 1;

/// O nome do índice dentro do pacote.
pub const MANIFESTO: &str = "manifesto.json";

/// As extensões que o "Importar" aceita dentro de um pacote.
const DENTRO_DO_PACOTE: [&str; 3] = [EXTENSAO, "xmp", "lrtemplate"];

/// Um `.rfpreset`, campo a campo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArquivoDePredefinicao {
    pub formato: String,
    pub versao: u32,
    pub nome: String,
    /// A pasta da coluna de onde ela saiu. Só informação: na volta ela entra
    /// em "Minhas", como toda importada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grupo: Option<String>,
    /// Aplicar parte do neutro em vez de somar — ver o módulo.
    #[serde(default)]
    pub recomeca_do_neutro: bool,
    /// Nome do motor → valor, na escala do shader.
    pub ajustes: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exportada_por: Option<String>,
}

/// Quem gravou o arquivo — o app e a versão.
pub fn assinatura() -> String {
    format!("VintageLightbox {}", env!("CARGO_PKG_VERSION"))
}

impl ArquivoDePredefinicao {
    /// A predefinição como vai para o disco.
    ///
    /// 🔑 **Recomeça do neutro** a que tem a marca (`Preset::replaces`, as do
    /// sistema) e a do operador que guarda todos os campos (a caixa "Zerar os
    /// outros ajustes ao aplicar" — `atualizar_preset` usa a mesma conta).
    /// Essas vão enxutas: o campo no neutro não sai, porque recomeçar já o põe
    /// lá. ⚠️ **A que soma vai como está**: nela, um campo no neutro é um
    /// pedido ("saturação de volta a zero"), e não sobra.
    pub fn do_preset(preset: &Preset, grupo: &str) -> Self {
        let recomeca = preset.replaces || preset.adjustments.len() == Ajustes::NOMES.len();
        let neutro = Ajustes::default().como_vetor();
        let no_neutro =
            |campo: &str, valor: f32| posicao_de(campo).is_some_and(|i| neutro[i] == valor);
        let ajustes = preset
            .adjustments
            .iter()
            .filter(|(campo, valor)| !(recomeca && no_neutro(campo, *valor)))
            .map(|(campo, valor)| (campo.to_string(), valor))
            .collect();
        Self {
            formato: FORMATO.into(),
            versao: VERSAO,
            nome: preset.name.clone(),
            grupo: Some(grupo.to_string()),
            recomeca_do_neutro: recomeca,
            ajustes,
            exportada_por: Some(assinatura()),
        }
    }

    /// O texto do arquivo — JSON com recuo, para ler num editor.
    pub fn texto(&self) -> String {
        serde_json::to_string_pretty(self).expect("só texto e números finitos") + "\n"
    }

    /// Lê um `.rfpreset`. `None` quando o texto não é um.
    ///
    /// ⚠️ **Versão mais nova é lida assim mesmo**: o que ela trouxer de novo é
    /// campo que este motor não tem, e esse vai para o relatório — a mesma
    /// regra do `.xmp` com recurso que o motor não tem.
    pub fn ler(texto: &str) -> Option<Self> {
        let arquivo: Self = serde_json::from_str(texto.trim_start_matches('\u{feff}')).ok()?;
        (arquivo.formato == FORMATO).then_some(arquivo)
    }

    /// Os ajustes como uma predefinição do operador os guarda, e os nomes que
    /// o motor não conhece (ou com valor que não é número).
    ///
    /// A que recomeça do neutro volta **inteira**: os campos que o arquivo não
    /// traz entram no neutro — ver o módulo.
    pub fn para_guardar(&self) -> (PresetAdjustments, Vec<String>) {
        let mut ignorados = Vec::new();
        let mut ajustes: BTreeMap<&str, f32> = if self.recomeca_do_neutro {
            let neutro = Ajustes::default().como_vetor();
            Ajustes::NOMES.iter().copied().zip(neutro).collect()
        } else {
            BTreeMap::new()
        };
        for (campo, valor) in &self.ajustes {
            match posicao_de(campo) {
                Some(i) if valor.is_finite() => {
                    ajustes.insert(Ajustes::NOMES[i], *valor);
                }
                _ => ignorados.push(format!("ajuste \"{campo}\", que este motor não tem")),
            }
        }
        (ajustes.into_iter().collect(), ignorados)
    }
}

fn posicao_de(campo: &str) -> Option<usize> {
    Ajustes::NOMES.iter().position(|nome| *nome == campo)
}

/// O nome de arquivo de uma predefinição: o nome dela, sem os caracteres que
/// o macOS, o Windows ou o Linux recusam.
pub fn nome_de_arquivo(nome: &str, extensao: &str) -> String {
    let limpo: String = nome
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let limpo = limpo.trim().trim_matches('.').trim();
    let limpo = if limpo.is_empty() {
        "Predefinição"
    } else {
        limpo
    };
    format!("{limpo}.{extensao}")
}

/// Uma predefinição a pôr no pacote, e em que pasta.
pub struct ItemDoPacote<'a> {
    pub preset: &'a Preset,
    /// O grupo da coluna de onde ela sai — vai para o campo `grupo`.
    pub grupo: &'a str,
    /// A pasta dentro do zip. `None` é a raiz.
    pub pasta: Option<&'a str>,
}

#[derive(Serialize)]
struct Manifesto<'a> {
    formato: &'a str,
    versao: u32,
    exportado_por: String,
    predefinicoes: Vec<Entrada>,
}

#[derive(Serialize)]
struct Entrada {
    arquivo: String,
    nome: String,
    grupo: String,
}

/// O `.zip` com um `.rfpreset` por predefinição e o `manifesto.json`.
///
/// 🔑 **Dois nomes iguais na mesma pasta não se sobrescrevem**: o segundo
/// ganha " (2)" — o que o Finder faz ao copiar. Sem isso, "Sépia" do sistema e
/// "Sépia" do operador, numa pasta só, virariam um arquivo.
pub fn pacote(itens: &[ItemDoPacote<'_>]) -> std::io::Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    // Texto encolhe bem: aqui o deflate vale a pena (no backup, de foto, não).
    // A hora é a de agora — sem ela, o Finder mostra 1º/jan/1980.
    let opcoes = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(agora());
    let mut usados = HashSet::new();
    let mut entradas = Vec::with_capacity(itens.len());

    for item in itens {
        let arquivo = ArquivoDePredefinicao::do_preset(item.preset, item.grupo);
        let base = nome_de_arquivo(&item.preset.name, EXTENSAO);
        let caminho = livre(&mut usados, item.pasta, &base);
        zip.start_file(caminho.as_str(), opcoes)?;
        zip.write_all(arquivo.texto().as_bytes())?;
        entradas.push(Entrada {
            arquivo: caminho,
            nome: item.preset.name.clone(),
            grupo: item.grupo.to_string(),
        });
    }

    let manifesto = Manifesto {
        formato: FORMATO_DO_PACOTE,
        versao: VERSAO,
        exportado_por: assinatura(),
        predefinicoes: entradas,
    };
    zip.start_file(MANIFESTO, opcoes)?;
    zip.write_all(
        serde_json::to_string_pretty(&manifesto)
            .expect("só texto")
            .as_bytes(),
    )?;
    Ok(zip.finish()?.into_inner())
}

/// A hora local, no formato do zip (que não guarda fuso).
fn agora() -> zip::DateTime {
    use chrono::{Datelike, Timelike};
    let agora = chrono::Local::now();
    zip::DateTime::from_date_and_time(
        agora.year().clamp(1980, 2107) as u16,
        agora.month() as u8,
        agora.day() as u8,
        agora.hour() as u8,
        agora.minute() as u8,
        agora.second() as u8,
    )
    .unwrap_or_default()
}

/// O caminho do arquivo no zip, sem repetir um que já está lá (sem caixa: o
/// disco do Mac e o do Windows não separam "Sépia" de "sépia").
fn livre(usados: &mut HashSet<String>, pasta: Option<&str>, base: &str) -> String {
    let (raiz, extensao) = base.rsplit_once('.').unwrap_or((base, ""));
    let prefixo = pasta.map(|p| format!("{p}/")).unwrap_or_default();
    let mut n = 1;
    loop {
        let nome = if n == 1 {
            format!("{prefixo}{base}")
        } else {
            format!("{prefixo}{raiz} ({n}).{extensao}")
        };
        if usados.insert(nome.to_lowercase()) {
            return nome;
        }
        n += 1;
    }
}

/// O que há de predefinição dentro de um `.zip` — os `.rfpreset` e os do
/// Lightroom, em qualquer pasta. `None` quando os bytes não são zip.
///
/// Fica de fora o lixo que o Finder põe no zip (`__MACOSX/`, `._*`), o
/// manifesto e o que não é predefinição — uma foto de capa no pacote de um
/// fotógrafo não é ilegível, só não é da conta do importar.
pub fn abrir_pacote(bytes: &[u8]) -> Option<Vec<Arquivo>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut arquivos = Vec::new();
    for i in 0..zip.len() {
        let Ok(mut entrada) = zip.by_index(i) else {
            continue;
        };
        if entrada.is_dir() {
            continue;
        }
        let caminho = entrada.name().to_string();
        let nome = caminho.rsplit('/').next().unwrap_or(&caminho).to_string();
        let extensao = nome.rsplit_once('.').map(|(_, e)| e.to_lowercase());
        let serve = extensao.is_some_and(|e| DENTRO_DO_PACOTE.contains(&e.as_str()));
        if !serve || caminho.starts_with("__MACOSX/") || nome.starts_with("._") {
            continue;
        }
        let mut texto = String::new();
        let lido = entrada.read_to_string(&mut texto).is_ok();
        arquivos.push(Arquivo {
            nome,
            texto: lido.then_some(texto),
        });
    }
    Some(arquivos)
}

/// Um arquivo escolhido no "Importar", lido do disco: o `.zip` vira o que tem
/// dentro, o resto vira um [`Arquivo`].
///
/// ⚠️ **Zip que não abre, ou sem predefinição nenhuma dentro, é ilegível** —
/// e não silêncio: quem escolheu o arquivo precisa saber que nada saiu dele.
pub fn ler_do_disco(caminho: &Path) -> Vec<Arquivo> {
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| caminho.to_string_lossy().to_string());
    let eh_zip = caminho
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if eh_zip {
        return match std::fs::read(caminho).ok().and_then(|b| abrir_pacote(&b)) {
            Some(dentro) if !dentro.is_empty() => dentro,
            _ => vec![Arquivo { nome, texto: None }],
        };
    }
    // ⚠️ **`read_to_string` recusa o que não é UTF-8**, e é o que se quer: os
    // formatos são texto, e um binário lido como preset entraria como "sem
    // nenhum ajuste" em vez de "não deu para ler".
    vec![Arquivo {
        nome,
        texto: std::fs::read_to_string(caminho).ok(),
    }]
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sistema(nome: &str) -> Preset {
        use_cases::presets::presets_de_sistema()
            .into_iter()
            .find(|p| p.name == nome)
            .expect("a do sistema")
    }

    #[test]
    fn o_arquivo_vai_e_volta_com_os_mesmos_numeros() {
        let minha = Preset::user(
            "Meu visual".into(),
            PresetAdjustments::vazia()
                .com("exposure", 0.5)
                .com("saturation", 0.0),
        );
        let arquivo = ArquivoDePredefinicao::do_preset(&minha, "Minhas");
        assert!(!arquivo.recomeca_do_neutro, "a que soma continua somando");
        assert_eq!(
            arquivo.ajustes.get("saturation"),
            Some(&0.0),
            "na que soma, o neutro é pedido"
        );

        let lido = ArquivoDePredefinicao::ler(&arquivo.texto()).expect("é um .rfpreset");
        assert_eq!(lido, arquivo);
        let (ajustes, ignorados) = lido.para_guardar();
        assert_eq!(ajustes, minha.adjustments);
        assert!(ignorados.is_empty());
    }

    /// 🚨 A que recomeça do neutro vai enxuta e volta inteira: aplicar a
    /// importada dá a mesma foto que aplicar a original.
    #[test]
    fn a_que_recomeca_volta_inteira_e_aplica_igual() {
        let sepia = sistema("Sépia à moda antiga");
        assert!(sepia.replaces);
        let arquivo = ArquivoDePredefinicao::do_preset(&sepia, "Do sistema");
        assert!(arquivo.recomeca_do_neutro);
        assert_eq!(arquivo.grupo.as_deref(), Some("Do sistema"));

        let (ajustes, _) = ArquivoDePredefinicao::ler(&arquivo.texto())
            .expect("lido")
            .para_guardar();
        assert_eq!(ajustes.len(), Ajustes::NOMES.len(), "volta com todos");

        let foto = Ajustes {
            exposure: 1.2,
            temperature: 3.0,
            ..Ajustes::default()
        };
        let importada = Preset::user("Sépia".into(), ajustes);
        assert_eq!(
            crate::revelacao::presets::aplicado(&foto, &importada).como_vetor(),
            crate::revelacao::presets::aplicado(&foto, &sepia).como_vetor(),
        );
    }

    /// A do operador com todos os campos — "Zerar os outros ajustes ao
    /// aplicar" — sai enxuta, com a marca.
    #[test]
    fn a_inteira_do_operador_sai_enxuta() {
        let ajustes = Ajustes {
            exposure: 0.7,
            ..Ajustes::default()
        };
        let inteira = Preset::user(
            "Completa".into(),
            crate::revelacao::presets::dos_ajustes(&ajustes, true),
        );
        let arquivo = ArquivoDePredefinicao::do_preset(&inteira, "Minhas");
        assert!(arquivo.recomeca_do_neutro);
        assert_eq!(arquivo.ajustes.get("exposure"), Some(&0.7));
        assert!(
            arquivo.ajustes.len() < 5,
            "o neutro não vai: {:?}",
            arquivo.ajustes
        );
    }

    #[test]
    fn campo_desconhecido_e_texto_alheio() {
        let texto = r#"{"formato":"recordarfotos.predefinicao","versao":9,"nome":"Futura",
            "ajustes":{"exposure":0.3,"holografia":12}}"#;
        let (ajustes, ignorados) = ArquivoDePredefinicao::ler(texto)
            .expect("versão nova também se lê")
            .para_guardar();
        assert_eq!(ajustes.get("exposure"), Some(0.3));
        assert_eq!(ajustes.len(), 1);
        assert_eq!(ignorados.len(), 1);
        assert!(ignorados[0].contains("holografia"));

        assert!(ArquivoDePredefinicao::ler(
            r#"{"formato":"outro","versao":1,"nome":"x","ajustes":{}}"#
        )
        .is_none());
        assert!(ArquivoDePredefinicao::ler("<x:xmpmeta/>").is_none());
    }

    #[test]
    fn nome_de_arquivo_sem_caractere_proibido() {
        assert_eq!(
            nome_de_arquivo("P&B / 70: \"ok\"?", EXTENSAO),
            "P&B - 70- -ok--.rfpreset"
        );
        assert_eq!(nome_de_arquivo("  ..  ", EXTENSAO), "Predefinição.rfpreset");
        assert_eq!(
            nome_de_arquivo("Vintage · Kodachrome", "zip"),
            "Vintage · Kodachrome.zip"
        );
    }

    #[test]
    fn o_pacote_tem_pastas_manifesto_e_nao_sobrescreve_nome_igual() {
        let a = Preset::user(
            "Sépia".into(),
            PresetAdjustments::vazia().com("exposure", 0.1),
        );
        let b = Preset::user(
            "sépia".into(),
            PresetAdjustments::vazia().com("exposure", 0.2),
        );
        let c = sistema("Hora dourada");
        let bytes = pacote(&[
            ItemDoPacote {
                preset: &a,
                grupo: "Minhas",
                pasta: Some("Minhas"),
            },
            ItemDoPacote {
                preset: &b,
                grupo: "Minhas",
                pasta: Some("Minhas"),
            },
            ItemDoPacote {
                preset: &c,
                grupo: "Do sistema",
                pasta: Some("Do sistema"),
            },
        ])
        .expect("zip");

        let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).expect("zip legível");
        let nomes: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).expect("entrada").name().to_string())
            .collect();
        assert_eq!(
            nomes,
            [
                "Minhas/Sépia.rfpreset",
                "Minhas/sépia (2).rfpreset",
                "Do sistema/Hora dourada.rfpreset",
                MANIFESTO,
            ]
        );
        let mut manifesto = String::new();
        zip.by_name(MANIFESTO)
            .expect("manifesto")
            .read_to_string(&mut manifesto)
            .expect("texto");
        assert!(manifesto.contains(FORMATO_DO_PACOTE));
        assert!(manifesto.contains("\"grupo\": \"Do sistema\""));

        let dentro = abrir_pacote(&bytes).expect("abre");
        assert_eq!(dentro.len(), 3, "o manifesto não é predefinição");
        let lida = ArquivoDePredefinicao::ler(dentro[2].texto.as_deref().expect("texto"))
            .expect("rfpreset");
        assert_eq!(lida.nome, "Hora dourada");
    }

    /// O zip do Lightroom (deflate, com o lixo do Finder) também abre.
    #[test]
    fn o_zip_do_lightroom_abre_sem_o_lixo_do_finder() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let deflate =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (nome, texto) in [
            ("Estúdio/Quente.xmp", "<x:xmpmeta/>"),
            ("__MACOSX/Estúdio/._Quente.xmp", "lixo"),
            ("Estúdio/capa.jpg", "jpg"),
            ("Estúdio/Antigo.LRTEMPLATE", "s = {}"),
        ] {
            zip.start_file(nome, deflate).expect("entrada");
            zip.write_all(texto.as_bytes()).expect("texto");
        }
        let bytes = zip.finish().expect("zip").into_inner();
        let dentro = abrir_pacote(&bytes).expect("abre");
        let nomes: Vec<&str> = dentro.iter().map(|a| a.nome.as_str()).collect();
        assert_eq!(nomes, ["Quente.xmp", "Antigo.LRTEMPLATE"]);
        assert_eq!(dentro[0].texto.as_deref(), Some("<x:xmpmeta/>"));

        assert!(abrir_pacote(b"nao sou zip").is_none());
    }

    #[test]
    fn ler_do_disco_abre_zip_e_avisa_zip_vazio() {
        let pasta = tempfile::tempdir().expect("pasta");
        let solto = pasta.path().join("Meu.rfpreset");
        std::fs::write(&solto, "{}").expect("grava");
        assert_eq!(ler_do_disco(&solto)[0].texto.as_deref(), Some("{}"));

        let vazio = pasta.path().join("Vazio.zip");
        std::fs::write(&vazio, pacote(&[]).expect("zip")).expect("grava");
        let lido = ler_do_disco(&vazio);
        assert_eq!(
            lido,
            vec![Arquivo {
                nome: "Vazio.zip".into(),
                texto: None
            }]
        );
    }
}
