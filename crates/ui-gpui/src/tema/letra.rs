//! 🔠 O tamanho da letra do app, com `Cmd +`, `Cmd −` e `Cmd 0` — o gesto do
//! Zed (dono, 29/09/2026).
//!
//! # Como o Zed faz, e por que aqui é igual
//!
//! Ele não amplia a janela: muda o **`rem_size`**, e tudo o que a tela escreveu
//! em rems cresce junto. O GPUI até tem `set_scale_factor`, que ampliaria a
//! janela inteira, mas só com `test-support`. O `Root` do gpui-kit já faz
//! `window.set_rem_size(theme.font_size)` a cada quadro — então basta mudar o
//! `font_size` do tema e redesenhar.
//!
//! # O que cresce e o que fica
//!
//! Cresce o texto (`text_sm()` e os outros são rems), os controles do kit e as
//! medidas que passam por [`em`]: os botões e campos de `estilo` e os
//! `text_size` das telas. **Fica** o layout em px — largura de painel, grade de
//! miniaturas, palco da foto —, como no Zed.
//!
//! ⚠️ **Na Revelação a tecla é da foto** (`Cmd +` aproxima, como no
//! Lightroom). A letra muda nas outras telas: o `na_revelacao` deixa a tecla
//! passar quando a Revelação não está no ar.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use gpui_kit::component::Theme;
use gpui_kit::App;

use super::medidas;

/// O menor e o maior tamanho, em px. Com o template de 14, são 4 passos
/// abaixo e 10 acima.
pub const MENOR: f32 = 10.;
pub const MAIOR: f32 = 24.;

thread_local! {
    /// Quanto a letra está acima (ou abaixo) da do template, em px.
    ///
    /// 🧵 **Por thread, e não global**: o GPUI desenha tudo na thread
    /// principal, e os testes do harness (um por thread, em paralelo) mudam a
    /// letra sem mexer na medida dos outros.
    static AJUSTE: Cell<i32> = const { Cell::new(0) };
}

/// A letra do template — a de quando o ajuste é zero.
fn base() -> f32 {
    medidas().letra
}

/// O ajuste que cabe entre [`MENOR`] e [`MAIOR`] para esta base.
fn preso(ajuste: i32, base: f32) -> i32 {
    ajuste.clamp((MENOR - base).round() as i32, (MAIOR - base).round() as i32)
}

/// O tamanho da letra agora, em px.
pub fn tamanho() -> f32 {
    base() + AJUSTE.get() as f32
}

/// 📏 **Uma medida escrita em px na letra do template, em rems.** Com o ajuste
/// zero dá os mesmos pixels de sempre; com a letra maior, cresce na mesma
/// proporção. É o que liga botões, campos e `text_size` à tecla.
pub fn em(px_na_casa: f32) -> gpui_kit::Rems {
    gpui_kit::rems(px_na_casa / base())
}

/// Muda a letra: `+1`/`-1` px por toque, e `0` volta à do template. Grava a
/// escolha e redesenha as janelas. Devolve o tamanho novo.
pub fn passo(delta: i32, cx: &mut App) -> f32 {
    let novo = if delta == 0 {
        0
    } else {
        preso(AJUSTE.get() + delta, base())
    };
    AJUSTE.set(novo);
    guardar(&arquivo(), novo);
    Theme::global_mut(cx).font_size = gpui_kit::px(tamanho());
    cx.refresh_windows();
    tamanho()
}

/// Lê o ajuste guardado. Chamar antes do `tema::aplicar`, que é quem põe a
/// letra no tema.
pub fn carregar() {
    AJUSTE.set(preso(guardado(&arquivo()), base()));
}

/// Onde o ajuste fica lembrado nesta máquina, ao lado do `tema.json`.
#[cfg(not(test))]
fn arquivo() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("letra.json")
}

/// Nos testes, um arquivo temporário por chamada — nunca o do catálogo de
/// quem roda a suíte (o mesmo cuidado de `tema::arquivo_da_escolha`).
#[cfg(test)]
fn arquivo() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-letra-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Arquivo ausente ou estragado é zero.
fn guardado(arquivo: &Path) -> i32 {
    std::fs::read(arquivo)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(0)
}

/// Falhar só faz a próxima abertura voltar à letra do template.
fn guardar(arquivo: &Path, ajuste: i32) {
    if let Some(pai) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    let _ = std::fs::write(arquivo, ajuste.to_string());
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn em_da_os_pixels_do_template() {
        let base = base();
        assert_eq!(em(base).to_pixels(gpui_kit::px(base)), gpui_kit::px(base));
        assert_eq!(em(32.).to_pixels(gpui_kit::px(base)), gpui_kit::px(32.));
        // Dois px a mais de letra, e o botão de 32 cresce na mesma proporção.
        let maior = em(32.).to_pixels(gpui_kit::px(base + 2.));
        assert_eq!(maior, gpui_kit::px(32. * (base + 2.) / base));
    }

    #[test]
    fn o_ajuste_fica_entre_o_menor_e_o_maior() {
        assert_eq!(preso(30, 14.), 10, "24 px no máximo");
        assert_eq!(preso(-30, 14.), -4, "10 px no mínimo");
        assert_eq!(preso(3, 14.), 3);
        assert_eq!(preso(30, 12.), 12, "a base densa sobe mais");
    }

    #[test]
    fn o_ajuste_sobrevive_ao_arquivo() {
        let arquivo = arquivo();
        assert_eq!(guardado(&arquivo), 0, "sem arquivo é zero");
        guardar(&arquivo, 3);
        assert_eq!(guardado(&arquivo), 3);
        std::fs::write(&arquivo, "estragado").unwrap();
        assert_eq!(guardado(&arquivo), 0, "estragado é zero");
        let _ = std::fs::remove_file(arquivo);
    }
}
