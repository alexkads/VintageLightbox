//! Os comandos da ponte (DESKTOP_TAURI §5). A lista é curta de propósito, e cada
//! comando está declarado no `build.rs`, que é o que o deixa negado a quem a
//! capacidade não nomeia.

use std::path::Path;

use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use infrastructure::raw_processing::{is_raw_file, load_raw_as_dynamic_image};

use crate::erro::ErroDaPonte;
use crate::raizes::RaizesPermitidas;

/// As extensões que o seletor mostra. São as mesmas que `is_raw_file` aceita.
const EXTENSOES_RAW: &[&str] = &[
    "nef", "cr2", "cr3", "arw", "dng", "orf", "raw", "rw2", "raf", "pef", "srw", "x3f",
];

/// Abre o seletor nativo e registra o arquivo escolhido como permitido.
///
/// Devolve `None` quando o operador cancela.
#[tauri::command]
pub async fn escolher_raw(
    app: AppHandle,
    raizes: State<'_, RaizesPermitidas>,
) -> Result<Option<String>, ErroDaPonte> {
    // ⚠️ `blocking_*` não pode rodar na thread principal. Comando `async` roda
    // no runtime do Tauri, fora dela.
    let Some(escolhido) = app
        .dialog()
        .file()
        .set_title("Escolher um arquivo RAW")
        .add_filter("RAW", EXTENSOES_RAW)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let caminho = escolhido
        .into_path()
        .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
    let canonico = raizes.permitir(&caminho)?;
    Ok(Some(canonico.to_string_lossy().into_owned()))
}

/// Decodifica um RAW escolhido antes e devolve os pixels.
///
/// O corpo da resposta é binário, e não JSON: 8 bytes de cabeçalho (largura e
/// altura, `u32` little-endian) seguidos de RGBA8. A página recebe um
/// `ArrayBuffer`. Serializar 96 MB de pixels em JSON custaria mais que a própria
/// decodificação.
///
/// 🧪 Fase 0: o formato ainda não é o da fila de importação. O que se mede aqui
/// é o caminho página → Rust → página com uma imagem grande. A Fase 2 decide o
/// que a fila recebe, respeitando o CONTRATO_DA_FOTO (o RAW é o bruto).
#[tauri::command]
pub async fn ler_raw(
    caminho: String,
    raizes: State<'_, RaizesPermitidas>,
) -> Result<Response, ErroDaPonte> {
    let arquivo = raizes.conferir(&caminho)?;
    let pixels = tauri::async_runtime::spawn_blocking(move || decodificar(&arquivo))
        .await
        .map_err(|_| ErroDaPonte::Interrompida)??;
    Ok(Response::new(pixels))
}

/// A decodificação, sem nada de Tauri em volta.
pub fn decodificar(arquivo: &Path) -> Result<Vec<u8>, ErroDaPonte> {
    let texto = arquivo.to_string_lossy();
    if !is_raw_file(&texto) {
        return Err(ErroDaPonte::NaoERaw);
    }
    let imagem = load_raw_as_dynamic_image(&texto)
        .map_err(ErroDaPonte::Decodificacao)?
        .to_rgba8();
    let (largura, altura) = imagem.dimensions();
    let rgba = imagem.into_raw();

    let mut corpo = Vec::with_capacity(8 + rgba.len());
    corpo.extend_from_slice(&largura.to_le_bytes());
    corpo.extend_from_slice(&altura.to_le_bytes());
    corpo.extend_from_slice(&rgba);
    Ok(corpo)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_extensoes_do_seletor_sao_as_que_a_leitura_aceita() {
        for extensao in EXTENSOES_RAW {
            assert!(is_raw_file(&format!("foto.{extensao}")), "{extensao}");
        }
    }

    #[test]
    fn um_jpeg_nao_passa_por_raw() {
        let pasta = tempfile::tempdir().unwrap();
        let jpeg = pasta.path().join("foto.jpg");
        std::fs::write(&jpeg, b"x").unwrap();
        assert_eq!(decodificar(&jpeg), Err(ErroDaPonte::NaoERaw));
    }

    #[test]
    fn um_raw_corrompido_vira_erro_e_nao_panico() {
        let pasta = tempfile::tempdir().unwrap();
        let raw = pasta.path().join("foto.cr2");
        std::fs::write(&raw, b"nao sou um raw").unwrap();
        assert!(matches!(
            decodificar(&raw),
            Err(ErroDaPonte::Decodificacao(_))
        ));
    }
}
