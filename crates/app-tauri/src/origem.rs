//! Importar de um cartão da câmera ou de uma pasta (DESKTOP_TAURI §5, Fase 2).
//!
//! O navegador só entrega os arquivos que o operador marcou um a um. Aqui o app
//! lista os cartões montados, com a mesma detecção do `ui-gpui`
//! (`DeviceService`), varre as subpastas (`DCIM/100CANON/…`) com o mesmo
//! `FileScanner` e entrega à página, um arquivo por vez, o que ela sabe importar.
//!
//! 🔒 A origem escolhida vira raiz permitida (`RaizesPermitidas::permitir_pasta`).
//! Um cartão só pode ser escolhido se ainda estiver montado. Uma pasta
//! qualquer, só pelo seletor nativo.

use std::path::{Path, PathBuf};

use serde::Serialize;

use infrastructure::devices::DeviceService;
use infrastructure::file_system::FileScanner;
use infrastructure::raw_processing::is_raw_file;

use crate::comandos::{revelar_raw, EXTENSOES_RAW};
use crate::erro::ErroDaPonte;

/// As extensões que a importação do site aceita, além das de RAW.
///
/// ⚠️ É a lista de `importacao/formatos.ts`, sem o ponto. As duas precisam andar
/// juntas: um formato que só uma delas conhece some da varredura ou é recusado
/// depois de lido.
pub const EXTENSOES_DE_FOTO: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "avif", "heic", "heif", "bmp", "gif",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Origem {
    pub nome: String,
    pub caminho: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArquivoDaOrigem {
    pub nome: String,
    pub caminho: String,
    pub bytes: u64,
    /// `true` quando o app vai revelar antes de entregar.
    pub raw: bool,
}

impl Origem {
    pub fn de(caminho: &Path, nome: Option<String>) -> Self {
        Origem {
            nome: nome.unwrap_or_else(|| {
                caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| caminho.to_string_lossy().into_owned())
            }),
            caminho: caminho.to_string_lossy().into_owned(),
        }
    }
}

/// Os cartões e discos removíveis montados agora.
pub fn cartoes() -> Vec<Origem> {
    DeviceService::new()
        .get_mounted_devices()
        .into_iter()
        .map(|fonte| Origem::de(&fonte.path, Some(fonte.name)))
        .collect()
}

/// O cartão com este caminho, se ele estiver montado agora.
pub fn cartao_montado(caminho: &str) -> Option<Origem> {
    let pedido = Path::new(caminho).canonicalize().ok()?;
    cartoes().into_iter().find(|cartao| {
        Path::new(&cartao.caminho)
            .canonicalize()
            .map(|c| c == pedido)
            .unwrap_or(false)
    })
}

/// Tudo o que a importação sabe abrir dentro da pasta, subpastas incluídas,
/// em ordem de caminho (a ordem em que a câmera numerou).
pub fn listar(pasta: &Path) -> Result<Vec<ArquivoDaOrigem>, ErroDaPonte> {
    let extensoes = EXTENSOES_DE_FOTO
        .iter()
        .chain(EXTENSOES_RAW.iter())
        .map(|e| e.to_string())
        .collect();
    let caminhos = FileScanner::with_extensions(extensoes)
        .scan_directory_with_depth(pasta, true)
        .map_err(|e| ErroDaPonte::Leitura(e.to_string()))?;
    Ok(caminhos
        .into_iter()
        .filter(|caminho| !oculto(caminho))
        .filter_map(|caminho| {
            let bytes = std::fs::metadata(&caminho).ok()?.len();
            let texto = caminho.to_string_lossy().into_owned();
            Some(ArquivoDaOrigem {
                nome: caminho.file_name()?.to_string_lossy().into_owned(),
                raw: is_raw_file(&texto),
                caminho: texto,
                bytes,
            })
        })
        .collect())
}

/// O arquivo como a fila deve recebê-lo: o RAW já revelado em JPEG, e o resto
/// como está no disco.
pub fn ler(arquivo: &PathBuf) -> Result<Vec<u8>, ErroDaPonte> {
    let bytes = std::fs::read(arquivo).map_err(|e| ErroDaPonte::Leitura(e.to_string()))?;
    if is_raw_file(&arquivo.to_string_lossy()) {
        revelar_raw(&bytes)
    } else {
        Ok(bytes)
    }
}

/// `._IMG_0001.JPG` é o que o macOS deixa num cartão FAT, e não é foto.
fn oculto(caminho: &Path) -> bool {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(true)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_varredura_desce_no_dcim_e_so_traz_foto() {
        let cartao = tempfile::tempdir().unwrap();
        let dcim = cartao.path().join("DCIM").join("100CANON");
        std::fs::create_dir_all(&dcim).unwrap();
        std::fs::write(dcim.join("IMG_0002.CR3"), b"raw").unwrap();
        std::fs::write(dcim.join("IMG_0001.JPG"), b"jpg").unwrap();
        std::fs::write(dcim.join("._IMG_0001.JPG"), b"lixo do mac").unwrap();
        std::fs::write(dcim.join("IMG_0001.XMP"), b"xmp").unwrap();
        std::fs::write(cartao.path().join("capa.webp"), b"webp").unwrap();

        let nomes: Vec<(String, bool)> = listar(cartao.path())
            .unwrap()
            .into_iter()
            .map(|a| (a.nome, a.raw))
            .collect();
        assert_eq!(
            nomes,
            vec![
                ("IMG_0001.JPG".to_string(), false),
                ("IMG_0002.CR3".to_string(), true),
                ("capa.webp".to_string(), false),
            ]
        );
    }

    #[test]
    fn um_arquivo_comum_sai_como_esta() {
        let pasta = tempfile::tempdir().unwrap();
        let foto = pasta.path().join("a.jpg");
        std::fs::write(&foto, b"\xFF\xD8conteudo").unwrap();
        assert_eq!(ler(&foto).unwrap(), b"\xFF\xD8conteudo");
    }

    #[test]
    fn uma_pasta_qualquer_nao_e_cartao_montado() {
        let pasta = tempfile::tempdir().unwrap();
        assert_eq!(cartao_montado(pasta.path().to_str().unwrap()), None);
    }

    /// A pasta de amostras de verdade: lista tudo, e o primeiro RAW sai JPEG.
    #[test]
    #[ignore]
    fn uma_pasta_de_verdade() {
        let pasta = std::env::var("VLB_RAWS")
            .unwrap_or_else(|_| format!("{}/Documents/RAWSample", std::env::var("HOME").unwrap()));
        let arquivos = listar(Path::new(&pasta)).unwrap();
        let raws = arquivos.iter().filter(|a| a.raw).count();
        eprintln!("{} arquivos, {} RAW", arquivos.len(), raws);
        assert!(arquivos
            .iter()
            .all(|a| !a.nome.ends_with(".xmp") && !a.nome.ends_with(".psd")));
        let primeiro = arquivos.iter().find(|a| a.raw).expect("nenhum RAW");
        let jpeg = ler(&PathBuf::from(&primeiro.caminho)).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn as_extensoes_sao_as_do_site() {
        // Espelha `EXTENSOES_ACEITAS` de `importacao/formatos.ts`.
        let do_site = [
            ".jpg", ".jpeg", ".png", ".tif", ".tiff", ".webp", ".avif", ".heic", ".heif", ".bmp",
            ".gif",
        ];
        let daqui: Vec<String> = EXTENSOES_DE_FOTO.iter().map(|e| format!(".{e}")).collect();
        assert_eq!(daqui, do_site);
    }
}
