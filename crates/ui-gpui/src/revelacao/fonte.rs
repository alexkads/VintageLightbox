//! De onde a Revelação tira os pixels de uma foto — **num lugar só**.
//!
//! ```text
//! bruto → base neutra → [editor em camadas] → imagem editada → revelacao-core
//! ```
//!
//! Até o editor existir eram sete leitores, cada um com a regra dele
//! (`docs/editor-em-camadas/01-DIAGNOSTICO.md`). Agora todos perguntam aqui:
//!
//! - **com imagem editada vigente** (C32): a cópia de trabalho dela, na chave
//!   `editada:<foto>:<revisão>` do cache de prévias — feita na primeira vez a
//!   partir da PNG, reduzida ao lado das prévias do bruto;
//! - **sem**: exatamente a regra de antes — o preview da foto local, ou a cópia
//!   de trabalho (`trabalho:<id>`) da foto do site.
//!
//! 🔑 A **revisão** volta junto: ela entra na chave do cache de reveladas
//! (`cache::Chave::da_fonte`), e é o que impede uma revelação da fonte antiga
//! de ser servida como a da nova (C17).
//!
//! 🚨 **C31**: uma versão cuja proporção não bate com a do bruto é recusada — a
//! foto volta ao bruto e a recusa é avisada. Máscara e corte são frações da
//! foto inteira de pé; outra proporção os poria no lugar errado sem erro nenhum.

use adapters::view_models::PhotoViewModel;
use editor_core::VersaoEditada;
use image::DynamicImage;
use infrastructure::cache::preview_manager::PreviewManager;

use super::persistencia;
use super::reposicao::LADO_DO_PREVIEW;
use crate::editor::porta::{versao_da_foto, Edicoes};

/// A revisão do bruto — "sem imagem editada".
pub const DO_BRUTO: u64 = 0;

/// A chave do cache de prévias onde mora a cópia de trabalho **do bruto**.
///
/// 🔑 A foto do site tem o bruto em `trabalho:<id>`; `site:<id>` é a imagem da
/// galeria, já revelada — servi-la ao motor aplicaria a receita duas vezes.
pub fn chave_do_bruto(foto: &PhotoViewModel) -> String {
    if persistencia::so_existe_no_site(foto) {
        persistencia::chave_do_trabalho(&foto.id)
    } else {
        foto.id.clone()
    }
}

/// A cópia de trabalho que o motor recebe, e de qual revisão ela é.
pub struct Copia {
    pub imagem: DynamicImage,
    pub revisao: u64,
}

/// A versão editada que vale para a foto, **já conferida** contra o bruto (C31).
///
/// `bruto` é a cópia de trabalho do bruto, quando já está à mão — a conferência
/// da proporção usa as medidas dela. Sem ela (foto do site ainda sem cópia), a
/// versão é aceita: as medidas dela são as da base de que nasceu.
pub fn versao_valida(
    edicoes: Option<&dyn Edicoes>,
    foto: &PhotoViewModel,
    bruto: Option<(u32, u32)>,
) -> Option<VersaoEditada> {
    let versao = versao_da_foto(edicoes?, foto)?;
    if let Some((largura, altura)) = bruto {
        if !versao.cabe_em(largura, altura) {
            crate::telemetria::avisar!(
                "⚠️ [Editor] a imagem editada de {} ({}×{}) não tem a proporção da foto ({largura}×{altura}): a Revelação usa o bruto (C31)",
                foto.name,
                versao.largura,
                versao.altura
            );
            return None;
        }
    }
    Some(versao)
}

/// A cópia de trabalho da foto — **bloqueante** (decodifica): no executor de
/// fundo, ou onde o código já lia o cache de prévias de forma síncrona.
pub fn copia_de_trabalho(
    previews: &PreviewManager,
    edicoes: Option<&dyn Edicoes>,
    foto: &PhotoViewModel,
) -> Option<Copia> {
    let do_bruto = || previews.get_preview(&chave_do_bruto(foto));
    let Some(versao) = versao_da_foto_se(edicoes, foto) else {
        return do_bruto().map(|imagem| Copia {
            imagem,
            revisao: DO_BRUTO,
        });
    };
    let bruto = do_bruto();
    let medidas = bruto.as_ref().map(|b| (b.width(), b.height()));
    if versao_valida(edicoes, foto, medidas).is_none() {
        return bruto.map(|imagem| Copia {
            imagem,
            revisao: DO_BRUTO,
        });
    }
    match copia_da_versao(previews, &versao, &foto.id) {
        Some(imagem) => Some(Copia {
            imagem,
            revisao: versao.revisao,
        }),
        // A PNG sumiu ou não abre: o bruto, e o aviso — nunca a tela vazia.
        None => {
            crate::telemetria::avisar!(
                "⚠️ [Editor] a imagem editada de {} não abriu ({}): a Revelação usa o bruto",
                foto.name,
                versao.arquivo.display()
            );
            bruto.map(|imagem| Copia {
                imagem,
                revisao: DO_BRUTO,
            })
        }
    }
}

fn versao_da_foto_se(
    edicoes: Option<&dyn Edicoes>,
    foto: &PhotoViewModel,
) -> Option<VersaoEditada> {
    versao_da_foto(edicoes?, foto)
}

/// A cópia de trabalho de uma versão editada: do cache, ou feita agora da PNG.
///
/// 🔑 Reduzida ao **mesmo lado** das prévias do bruto (Lanczos3, como a
/// reposição): a mesma foto não pode abrir com nitidez diferente por ter sido
/// editada.
pub fn copia_da_versao(
    previews: &PreviewManager,
    versao: &VersaoEditada,
    foto_id: &str,
) -> Option<DynamicImage> {
    let chave = versao.chave_da_copia(foto_id);
    if let Some(guardada) = previews.get_preview(&chave) {
        return Some(guardada);
    }
    let bytes = std::fs::read(&versao.arquivo).ok()?;
    let inteira = editor_core::projeto::ler_png(&bytes).ok()?;
    let inteira = DynamicImage::ImageRgb8(inteira);
    let copia = if inteira.width().max(inteira.height()) > LADO_DO_PREVIEW {
        inteira.resize(
            LADO_DO_PREVIEW,
            LADO_DO_PREVIEW,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        inteira
    };
    let _ = previews.save_preview(&chave, &copia);
    Some(copia)
}

/// A imagem editada em resolução cheia, quando vale — o zoom 1:1 e a entrega.
pub fn arquivo_editado(
    edicoes: Option<&dyn Edicoes>,
    foto: &PhotoViewModel,
) -> Option<VersaoEditada> {
    versao_da_foto_se(edicoes, foto)
}

/// A revisão que vale para a foto, sem ler pixel nenhum (0 = o bruto).
pub fn revisao_de(edicoes: Option<&dyn Edicoes>, foto: &PhotoViewModel) -> u64 {
    versao_da_foto_se(edicoes, foto)
        .map(|v| v.revisao)
        .unwrap_or(DO_BRUTO)
}
