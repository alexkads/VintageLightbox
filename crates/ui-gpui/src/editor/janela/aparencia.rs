//! 🎨 Os tokens do editor: medidas, vãos, letra e cores da área de trabalho.
//!
//! O editor é a tela do Photoshop dentro do app — barra de ferramentas,
//! opções, docas e status densos, de borda discreta e quase sem canto. As
//! medidas moram aqui para a barra, as opções e os painéis não inventarem
//! cada um a sua; as cores **derivam do tema do app** (o kit é global, e um
//! botão do kit claro ao lado de uma faixa escura à mão seria pior que
//! seguir o tema). O tema global não muda por causa do editor.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Hsla};

/// As medidas, em pontos.
pub mod medida {
    /// O botão de uma ferramenta na barra da esquerda (quadrado).
    pub const BOTAO_DA_FERRAMENTA: f32 = 30.0;
    /// O ícone dentro dele.
    pub const ICONE_DA_FERRAMENTA: f32 = 16.0;
    /// A barra em uma coluna e em duas (o `»` do topo alterna).
    pub const BARRA_UMA_COLUNA: f32 = 40.0;
    pub const BARRA_DUAS_COLUNAS: f32 = 70.0;
    /// As faixas horizontais: menus, opções da ferramenta, aba do documento
    /// e status. Fixas — trocar de ferramenta não muda a altura do palco.
    pub const ALTURA_DOS_MENUS: f32 = 30.0;
    pub const ALTURA_DAS_OPCOES: f32 = 36.0;
    pub const ALTURA_DA_ABA: f32 = 26.0;
    pub const ALTURA_DO_STATUS: f32 = 24.0;
    /// A faixa de ícones dos painéis recolhidos.
    pub const FAIXA_DE_ICONES: f32 = 36.0;
    /// O vão entre controles de uma faixa, e o miúdo (entre um rótulo e o
    /// controle dele).
    pub const VAO: f32 = 8.0;
    pub const VAO_MIUDO: f32 = 4.0;
    /// A largura de um campo numérico das opções.
    pub const CAMPO_NUMERICO: f32 = 64.0;
    /// O canto dos quadrados da barra (o Photoshop quase não arredonda).
    pub const CANTO: f32 = 3.0;
    /// O flyout das variantes de um grupo da barra.
    pub const LARGURA_DO_FLYOUT: f32 = 280.0;
    pub const LINHA_DO_FLYOUT: f32 = 26.0;
    /// O aperto prolongado que abre o flyout (o do Photoshop é ~ meio
    /// segundo).
    pub const APERTO_PROLONGADO_MS: u64 = 350;
    /// As amostras de cor embaixo da barra: o quadrado de cada cor e o
    /// deslocamento do de fundo.
    pub const AMOSTRA_DA_COR: f32 = 20.0;
    pub const DESLOCAMENTO_DO_FUNDO: f32 = 10.0;
}

/// O tamanho da letra das faixas, em px do template (passa por
/// `tema::letra::em`, então acompanha o `⌘ +` da letra do app).
pub const LETRA: f32 = 12.0;
pub const LETRA_MIUDA: f32 = 11.0;

/// As cores da área de trabalho, tiradas do tema.
#[derive(Clone, Copy)]
pub struct Cores {
    /// O fundo das faixas e dos painéis.
    pub cromo: Hsla,
    /// O fundo dos painéis, um tom abaixo do das faixas.
    pub painel: Hsla,
    pub borda: Hsla,
    pub texto: Hsla,
    pub apagado: Hsla,
    /// O passar do mouse.
    pub realce: Hsla,
    /// O texto sobre o ligado (a ferramenta na mão é o botão primário).
    pub ativo_texto: Hsla,
    /// A moldura do alvo (pixels ou máscara) e o foco.
    pub alvo: Hsla,
    /// Em volta da foto.
    pub poco: Hsla,
}

pub fn cores(cx: &App) -> Cores {
    let t = cx.theme();
    Cores {
        cromo: t.background,
        painel: t.background,
        borda: t.border,
        texto: t.foreground,
        apagado: t.muted_foreground,
        realce: t.muted,
        ativo_texto: t.primary_foreground,
        alvo: t.ring,
        poco: crate::tema::cores::poco(),
    }
}
