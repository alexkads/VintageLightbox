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
        NovaCamada,
        DuplicarCamada,
        SubirCamada,
        DescerCamada,
        CamadaDeCima,
        CamadaDeBaixo,
        Aproximar,
        Afastar,
        Encaixar,
        UmPorUm,
        AlternarZoom,
        SegurarAMao,
        SelecaoRetangular,
        SelecaoEliptica,
        SelecaoLaco,
        SelecionarTudo,
        Desmarcar,
        InverterSelecao,
        ApagarSelecao,
        PreencherSelecao,
        MesclarParaBaixo,
        UsarCarimbo,
        UsarContaGotas,
        UsarMover,
        CamadaViaRecorte,
        TransformacaoLivre,
        AplicarTransformacao,
        CancelarTransformacao,
        UsarCorrecao,
        PreencherPeloConteudo,
        UsarSubexposicao,
        UsarSuperexposicao,
        UsarDesfoque,
        UsarNitidez,
    ]
);

/// O contexto de teclas da janela do editor.
pub const CONTEXTO: &str = "EditorDeFoto";

/// O mesmo, fora de campo de texto: a tecla solta (B, E, Z, Espaço…) não pode
/// comer a letra de quem renomeia uma camada (a regra do `app.rs`).
const SEM_CAMPO: &str = "EditorDeFoto && !Input";

/// As teclas do editor — as do Photoshop para pincel (B), borracha (E),
/// carimbo (S), conta-gotas (I), mover (V), transformação livre (⌘T, Enter,
/// Esc), camada via cópia e via recorte (⌘J, ⇧⌘J), pincel de correção (J) e
/// preencher a seleção pelo conteúdo (⇧⌫), subexposição e superexposição (O,
/// ⇧O), desfoque e nitidez (R, ⇧R),
/// tamanho (`[` `]`) e camadas (⇧⌘N nova, ⌘J duplicar, ⌘] ⌘[ subir e descer,
/// ⌥] ⌥[ escolher a de cima e a de baixo, ⌘E mesclar para baixo), seleção (M
/// retângulo, ⇧M elipse, L laço, ⌘A ⌘D ⇧⌘I, Delete apaga, ⌥Delete preenche); as da Revelação para o zoom (Z,
/// Espaço, ⌘= ⌘− ⌘0 ⌘⌥0); `Cmd`/`Ctrl` para desfazer, refazer e salvar.
///
/// 🔑 **Com `Ctrl` também**: o balcão roda Windows e Linux, onde desfazer é
/// `Ctrl+Z` e mais nada (a mesma regra do `app.rs`).
pub fn init(cx: &mut gpui_kit::App) {
    use gpui_kit::KeyBinding;
    let c = Some(CONTEXTO);
    let solta = Some(SEM_CAMPO);
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
        KeyBinding::new("b", UsarPincel, solta),
        KeyBinding::new("e", UsarBorracha, solta),
        KeyBinding::new("[", PincelMenor, solta),
        KeyBinding::new("]", PincelMaior, solta),
        KeyBinding::new("h", AlternarCamada, solta),
        KeyBinding::new("cmd-shift-n", NovaCamada, c),
        KeyBinding::new("ctrl-shift-n", NovaCamada, c),
        KeyBinding::new("cmd-j", DuplicarCamada, c),
        KeyBinding::new("ctrl-j", DuplicarCamada, c),
        KeyBinding::new("cmd-]", SubirCamada, c),
        KeyBinding::new("ctrl-]", SubirCamada, c),
        KeyBinding::new("cmd-[", DescerCamada, c),
        KeyBinding::new("ctrl-[", DescerCamada, c),
        KeyBinding::new("alt-]", CamadaDeCima, solta),
        KeyBinding::new("alt-[", CamadaDeBaixo, solta),
        KeyBinding::new("cmd-=", Aproximar, c),
        KeyBinding::new("cmd-+", Aproximar, c),
        KeyBinding::new("ctrl-=", Aproximar, c),
        KeyBinding::new("ctrl-+", Aproximar, c),
        KeyBinding::new("cmd--", Afastar, c),
        KeyBinding::new("ctrl--", Afastar, c),
        KeyBinding::new("cmd-0", Encaixar, c),
        KeyBinding::new("ctrl-0", Encaixar, c),
        KeyBinding::new("cmd-alt-0", UmPorUm, c),
        KeyBinding::new("ctrl-alt-0", UmPorUm, c),
        KeyBinding::new("z", AlternarZoom, solta),
        KeyBinding::new("space", SegurarAMao, solta),
        KeyBinding::new("m", SelecaoRetangular, solta),
        KeyBinding::new("shift-m", SelecaoEliptica, solta),
        KeyBinding::new("l", SelecaoLaco, solta),
        KeyBinding::new("cmd-a", SelecionarTudo, c),
        KeyBinding::new("ctrl-a", SelecionarTudo, c),
        KeyBinding::new("cmd-d", Desmarcar, c),
        KeyBinding::new("ctrl-d", Desmarcar, c),
        KeyBinding::new("cmd-shift-i", InverterSelecao, c),
        KeyBinding::new("ctrl-shift-i", InverterSelecao, c),
        KeyBinding::new("backspace", ApagarSelecao, solta),
        KeyBinding::new("delete", ApagarSelecao, solta),
        KeyBinding::new("alt-backspace", PreencherSelecao, solta),
        KeyBinding::new("alt-delete", PreencherSelecao, solta),
        KeyBinding::new("s", UsarCarimbo, solta),
        KeyBinding::new("i", UsarContaGotas, solta),
        KeyBinding::new("v", UsarMover, solta),
        KeyBinding::new("cmd-shift-j", CamadaViaRecorte, c),
        KeyBinding::new("ctrl-shift-j", CamadaViaRecorte, c),
        KeyBinding::new("cmd-t", TransformacaoLivre, c),
        KeyBinding::new("ctrl-t", TransformacaoLivre, c),
        KeyBinding::new("enter", AplicarTransformacao, solta),
        KeyBinding::new("escape", CancelarTransformacao, solta),
        KeyBinding::new("j", UsarCorrecao, solta),
        KeyBinding::new("o", UsarSubexposicao, solta),
        KeyBinding::new("shift-o", UsarSuperexposicao, solta),
        KeyBinding::new("r", UsarDesfoque, solta),
        KeyBinding::new("shift-r", UsarNitidez, solta),
        KeyBinding::new("shift-backspace", PreencherPeloConteudo, solta),
        KeyBinding::new("shift-delete", PreencherPeloConteudo, solta),
        KeyBinding::new("cmd-e", MesclarParaBaixo, c),
        KeyBinding::new("ctrl-e", MesclarParaBaixo, c),
    ]);
}
