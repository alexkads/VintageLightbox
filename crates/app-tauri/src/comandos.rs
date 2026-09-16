//! Os comandos da ponte (DESKTOP_TAURI §5). A lista é curta de propósito, e cada
//! comando está declarado no `build.rs`, que é o que o deixa negado a quem a
//! capacidade não nomeia.

use std::path::Path;

use tauri::ipc::Request;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use infrastructure::raw_processing::{is_raw_file, load_raw_as_dynamic_image, load_raw_from_bytes};

use crate::bytes::{self, Bytes};
use crate::erro::ErroDaPonte;
use crate::origem::{self, ArquivoDaOrigem, Origem};
use crate::pasta_de_saida::{Pasta, PastaDeSaida};
use crate::raizes::RaizesPermitidas;
use crate::tela_do_cliente;

/// As extensões que o seletor mostra. São as mesmas que `is_raw_file` aceita.
pub const EXTENSOES_RAW: &[&str] = &[
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
) -> Result<Bytes, ErroDaPonte> {
    let arquivo = raizes.conferir(&caminho)?;
    let pixels = tauri::async_runtime::spawn_blocking(move || decodificar(&arquivo))
        .await
        .map_err(|_| ErroDaPonte::Interrompida)??;
    Ok(pixels.into())
}

/// Os cartões e discos removíveis montados agora.
#[tauri::command]
pub async fn cartoes_montados() -> Vec<Origem> {
    tauri::async_runtime::spawn_blocking(origem::cartoes)
        .await
        .unwrap_or_default()
}

/// Escolhe de onde importar: um cartão montado, pelo caminho que
/// `cartoes_montados` devolveu, ou uma pasta pelo seletor nativo (sem caminho).
/// Devolve `None` quando o operador desiste do seletor.
#[tauri::command]
pub async fn escolher_origem(
    app: AppHandle,
    caminho: Option<String>,
    raizes: State<'_, RaizesPermitidas>,
) -> Result<Option<Origem>, ErroDaPonte> {
    let escolhida = match caminho {
        Some(caminho) => {
            // 🔒 Um caminho vindo da página só vale se for um cartão montado.
            let cartao = origem::cartao_montado(&caminho).ok_or(ErroDaPonte::ForaDasRaizes)?;
            Origem {
                caminho: raizes
                    .permitir_pasta(Path::new(&cartao.caminho))?
                    .to_string_lossy()
                    .into_owned(),
                nome: cartao.nome,
            }
        }
        // 🔧 Um roteiro de depuração não responde a seletor nativo: com
        // `VLB_ORIGEM_DE_TESTE`, a pasta é essa (já permitida na abertura).
        None if crate::navegacao::DESENVOLVIMENTO
            && std::env::var_os("VLB_ORIGEM_DE_TESTE").is_some() =>
        {
            let pasta = std::path::PathBuf::from(std::env::var_os("VLB_ORIGEM_DE_TESTE").unwrap());
            Origem::de(&raizes.permitir_pasta(&pasta)?, None)
        }
        None => {
            let Some(pasta) = app
                .dialog()
                .file()
                .set_title("De onde importar as fotos")
                .blocking_pick_folder()
            else {
                return Ok(None);
            };
            let pasta = pasta
                .into_path()
                .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
            Origem::de(&raizes.permitir_pasta(&pasta)?, None)
        }
    };
    Ok(Some(escolhida))
}

/// O que a importação sabe abrir dentro de uma origem escolhida.
#[tauri::command]
pub async fn listar_origem(
    caminho: String,
    raizes: State<'_, RaizesPermitidas>,
) -> Result<Vec<ArquivoDaOrigem>, ErroDaPonte> {
    let pasta = raizes.conferir_pasta(&caminho)?;
    tauri::async_runtime::spawn_blocking(move || origem::listar(&pasta))
        .await
        .map_err(|_| ErroDaPonte::Interrompida)?
}

/// Um arquivo de uma origem escolhida, pronto para a fila: o RAW já revelado em
/// JPEG, e o resto como está no disco.
#[tauri::command]
pub async fn ler_da_origem(
    caminho: String,
    raizes: State<'_, RaizesPermitidas>,
) -> Result<Bytes, ErroDaPonte> {
    let arquivo = raizes.conferir(&caminho)?;
    let conteudo = tauri::async_runtime::spawn_blocking(move || origem::ler(&arquivo))
        .await
        .map_err(|_| ErroDaPonte::Interrompida)??;
    Ok(conteudo.into())
}

/// A pasta de saída guardada, se houver.
#[tauri::command]
pub fn pasta_de_saida(pasta: State<'_, PastaDeSaida>) -> Option<Pasta> {
    pasta.atual()
}

/// Abre o seletor nativo de pastas. Devolve `None` quando o operador desiste,
/// e aí a escolha anterior continua valendo.
#[tauri::command]
pub async fn escolher_pasta(
    app: AppHandle,
    pasta: State<'_, PastaDeSaida>,
) -> Result<Option<Pasta>, ErroDaPonte> {
    let mut dialogo = app
        .dialog()
        .file()
        .set_title("Onde salvar as fotos exportadas");
    if let Some(atual) = pasta.atual() {
        dialogo = dialogo.set_directory(atual.caminho);
    }
    let Some(escolhida) = dialogo.blocking_pick_folder() else {
        return Ok(None);
    };
    let caminho = escolhida
        .into_path()
        .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
    pasta.escolher(&caminho).map(Some)
}

#[tauri::command]
pub fn esquecer_pasta(pasta: State<'_, PastaDeSaida>) {
    pasta.esquecer();
}

#[tauri::command]
pub async fn nomes_na_pasta(pasta: State<'_, PastaDeSaida>) -> Result<Vec<String>, ErroDaPonte> {
    pasta.nomes()
}

/// Grava um arquivo exportado na pasta escolhida.
///
/// O corpo é o arquivo (em base64 ou cru, ver `bytes.rs`), e o nome vem no
/// cabeçalho `x-nome`, codificado como componente de URL.
#[tauri::command]
pub async fn gravar_na_pasta(
    request: Request<'_>,
    pasta: State<'_, PastaDeSaida>,
) -> Result<(), ErroDaPonte> {
    let conteudo = bytes::do_pedido(&request)?;
    let nome = nome_do_cabecalho(&request)?;
    pasta.gravar(&nome, &conteudo)
}

/// Abre a tela do cliente no monitor que não é o do operador.
#[tauri::command]
pub async fn abrir_tela_do_cliente(app: AppHandle) -> Result<(), ErroDaPonte> {
    tela_do_cliente::abrir(&app).await
}

/// Fecha a tela do cliente. A página dela chama isto no lugar de
/// `window.close()`, que o webview recusa numa janela que o script não abriu.
#[tauri::command]
pub fn fechar_tela_do_cliente(app: AppHandle) {
    tela_do_cliente::fechar(&app);
}

/// A qualidade do JPEG que sai do RAW.
///
/// Ele não é o bruto: é o arquivo que entra na fila de importação no lugar do
/// RAW, e a compressão das parametrizações (C1) o recodifica ao classificar. Alta
/// para que essa segunda geração não some perda visível à primeira.
pub const QUALIDADE_DO_RAW_REVELADO: u8 = 95;

/// Revela um RAW que a página entregou em bytes e devolve um JPEG em tamanho
/// cheio, sem efeito nenhum.
///
/// 🔑 **O RAW não é o bruto** (CONTRATO_DA_FOTO, glossário e C1): o bruto é a
/// foto já nas parametrizações, e o backend, o editor e o download do cliente só
/// abrem JPEG, PNG e WebP. Este JPEG entra na fila como um arquivo comum, e daí
/// em diante o caminho é o mesmo de qualquer foto.
///
/// Os bytes chegam crus, e o nome vem em `x-nome`, só para conferir a extensão.
/// Nenhum caminho do disco é aberto.
#[tauri::command]
pub async fn converter_raw(request: Request<'_>) -> Result<Bytes, ErroDaPonte> {
    let nome = nome_do_cabecalho(&request)?;
    if !is_raw_file(&nome) {
        return Err(ErroDaPonte::NaoERaw);
    }
    let raw = bytes::do_pedido(&request)?;
    let jpeg = tauri::async_runtime::spawn_blocking(move || revelar_raw(&raw))
        .await
        .map_err(|_| ErroDaPonte::Interrompida)??;
    Ok(jpeg.into())
}

/// O RAW em JPEG, sem nada de Tauri em volta.
pub fn revelar_raw(bytes: &[u8]) -> Result<Vec<u8>, ErroDaPonte> {
    let imagem = load_raw_from_bytes(bytes).map_err(ErroDaPonte::Decodificacao)?;
    foto_codec::codificar(&imagem, QUALIDADE_DO_RAW_REVELADO)
        .map_err(|e| ErroDaPonte::Decodificacao(e.to_string()))
}

fn nome_do_cabecalho(request: &Request<'_>) -> Result<String, ErroDaPonte> {
    request
        .headers()
        .get("x-nome")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| {
            percent_encoding::percent_decode_str(valor)
                .decode_utf8()
                .ok()
                .map(|nome| nome.into_owned())
        })
        .ok_or(ErroDaPonte::PedidoIncompleto)
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
    fn bytes_que_nao_sao_raw_viram_erro() {
        assert!(matches!(
            revelar_raw(b"nao sou um raw"),
            Err(ErroDaPonte::Decodificacao(_))
        ));
    }

    /// Um RAW de verdade vira um JPEG que o navegador abre. Precisa das
    /// amostras em `~/Documents/RAWSample` (ou `VLB_RAW`).
    #[test]
    #[ignore]
    fn um_raw_de_verdade_vira_jpeg() {
        let caminho = std::env::var("VLB_RAW").unwrap_or_else(|_| {
            format!(
                "{}/Documents/RAWSample/nikon_d7200_09.nef",
                std::env::var("HOME").unwrap()
            )
        });
        let jpeg = revelar_raw(&std::fs::read(&caminho).unwrap()).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let imagem = foto_codec::decodificar(&jpeg).unwrap();
        assert!(imagem.width() > 1000, "{}", imagem.width());
        eprintln!(
            "{}: {}×{}, {:.1} MB de JPEG",
            caminho,
            imagem.width(),
            imagem.height(),
            jpeg.len() as f64 / 1e6
        );
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
