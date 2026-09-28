//! 🖌️ O editor de fotos em camadas — a janela.
//!
//! Aberto pela Revelação (botão direito na tira → "Editar Foto"), numa janela
//! própria para aquela foto. O documento, o pincel, o histórico e a gravação
//! moram no `editor-core`; aqui ficam a janela, os controles e a porta das
//! edições. O contrato com a Revelação está em `docs/editor-em-camadas/`.

pub mod janela;
pub mod porta;

pub use janela::{EditorDeFoto, EventoDoEditor};

gpui_kit::actions!(
    editor,
    [
        DesfazerNoEditor,
        RefazerNoEditor,
        SalvarNoEditor,
        FecharEditor,
        UsarPincel,
        UsarBorracha,
        PincelMenor,
        PincelMaior,
        AlternarCamada,
    ]
);

/// O contexto de teclas da janela do editor.
pub const CONTEXTO: &str = "EditorDeFoto";

/// As teclas do editor — as do Photoshop para pincel (B), borracha (E) e
/// tamanho (`[` `]`); `Cmd`/`Ctrl` para desfazer, refazer e salvar.
///
/// 🔑 **Com `Ctrl` também**: o balcão roda Windows e Linux, onde desfazer é
/// `Ctrl+Z` e mais nada (a mesma regra do `app.rs`).
pub fn init(cx: &mut gpui_kit::App) {
    use gpui_kit::KeyBinding;
    let c = Some(CONTEXTO);
    cx.bind_keys([
        KeyBinding::new("cmd-shift-z", RefazerNoEditor, c),
        KeyBinding::new("cmd-z", DesfazerNoEditor, c),
        KeyBinding::new("ctrl-shift-z", RefazerNoEditor, c),
        KeyBinding::new("ctrl-y", RefazerNoEditor, c),
        KeyBinding::new("ctrl-z", DesfazerNoEditor, c),
        KeyBinding::new("cmd-s", SalvarNoEditor, c),
        KeyBinding::new("ctrl-s", SalvarNoEditor, c),
        KeyBinding::new("cmd-w", FecharEditor, c),
        KeyBinding::new("ctrl-w", FecharEditor, c),
        KeyBinding::new("b", UsarPincel, c),
        KeyBinding::new("e", UsarBorracha, c),
        KeyBinding::new("[", PincelMenor, c),
        KeyBinding::new("]", PincelMaior, c),
        KeyBinding::new("h", AlternarCamada, c),
    ]);
}
