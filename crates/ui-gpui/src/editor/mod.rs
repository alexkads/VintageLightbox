//! 🖌️ O editor de fotos em camadas — a janela.
//!
//! Aberto pela Revelação (botão direito na tira → "Editar Foto"), numa janela
//! própria para aquela foto. O documento, o pincel, o histórico e a gravação
//! moram no `editor-core`; aqui ficam a janela, os controles e a porta das
//! edições. O contrato com a Revelação está em `docs/editor-em-camadas/`.

pub mod giro;
pub mod janela;
pub mod porta;
pub mod preenchimento;

pub use janela::{EditorDeFoto, EventoDoEditor};

gpui_kit::actions!(
    editor,
    [
        DesfazerNoEditor,
        RefazerNoEditor,
        SalvarNoEditor,
        FecharEditor,
        PincelMenor,
        PincelMaior,
        DurezaMenor,
        DurezaMaior,
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
        SegurarAMao,
        SelecionarTudo,
        Desmarcar,
        InverterSelecao,
        ApagarSelecao,
        PreencherSelecao,
        MesclarParaBaixo,
        CamadaViaRecorte,
        TransformacaoLivre,
        AplicarTransformacao,
        CancelarTransformacao,
        PreencherPeloConteudo,
        DifundirSelecao,
        TrocarCores,
        CoresPadrao,
        // As letras da barra (`janela::ferramentas`): a letra volta à última
        // ferramenta do grupo; ⇧ + letra passa para a seguinte.
        GrupoV,
        GrupoM,
        GrupoL,
        GrupoW,
        GrupoI,
        GrupoJ,
        GrupoB,
        GrupoS,
        GrupoE,
        GrupoG,
        GrupoO,
        GrupoH,
        GrupoR,
        GrupoZ,
        ProximaDoGrupoM,
        ProximaDoGrupoL,
        ProximaDoGrupoG,
        ProximaDoGrupoO,
        ProximaDoGrupoJ,
        // ⌥⌘G: cria ou libera a máscara de corte da escolhida.
        AlternarMascaraDeCorte,
        // Etapa 16: a área de transferência, o carimbo visível, inverter, a
        // sobreposição rubi e o cadeado da transparência.
        Copiar,
        CopiarMesclado,
        Recortar,
        Colar,
        ColarNoLugar,
        CarimbarVisivel,
        Inverter,
        AlternarRubi,
        BloquearTransparencia,
        // Y: o Antes/Depois (o `\\` é a sobreposição rubi da máscara, como no
        // Photoshop).
        AlternarAntesDepois,
        // ⇧⌘X: Liquidificar.
        Liquidificar,
        // Tab: barra de ferramentas, opções e painéis somem (e voltam); ⇧Tab:
        // só os painéis — como no Photoshop.
        AlternarInterface,
        AlternarPaineis,
    ]
);

/// O contexto de teclas da janela do editor.
pub const CONTEXTO: &str = "EditorDeFoto";

/// O mesmo, fora de campo de texto: a tecla solta (B, E, Z, Espaço…) não pode
/// comer a letra de quem renomeia uma camada (a regra do `app.rs`).
const SEM_CAMPO: &str = "EditorDeFoto && !Input";

/// As teclas do editor — as do Photoshop, conferidas na tabela oficial da Adobe
/// ("Keyboard shortcuts", PDF do helpx, 07/out/2026):
///
/// - ferramentas por letra: Mover (V), seleção retangular/elíptica (M), laço
///   e poligonal (L, ⇧L), varinha (W), conta-gotas (I), pincel de correção para manchas e de
///   recuperação (J, ⇧J), pincel (B),
///   carimbo (S), borracha (E), degradê/lata (G), subexposição/superexposição
///   (O), Mão (H) e Girar vista (R). **⇧ + letra passa para a ferramenta
///   seguinte do mesmo grupo** ("Use Shift Key for Tool Switch", o padrão de
///   lá). Desfoque e nitidez não têm letra — no Photoshop também não;
/// - Espaço segurado: a Mão, e a ferramenta volta ao soltar;
/// - tamanho `[` `]`, dureza `{` `}` (⇧[ ⇧]); os números dão a opacidade e ⇧ +
///   números o fluxo (tratados na janela, fora de campo de texto);
/// - X troca as cores, D volta a preto e branco, ⇧F6 difunde a seleção;
/// - camadas: ⇧⌘N nova, ⌘J via cópia, ⇧⌘J via recorte, ⌘] ⌘[ subir e descer,
///   ⌥] ⌥[ a de cima e a de baixo, ⌘E mesclar para baixo (na base de
///   uma máscara de corte, mescla o conjunto), ⌥⌘G cria ou libera a máscara de
///   corte, ⌘, mostrar/esconder
///   (o menu "Ocultar camadas" do Photoshop; ⚠️ não está na tabela em PDF);
/// - seleção: ⌘A ⌘D ⇧⌘I, Delete apaga, ⌥Delete preenche, ⇧Delete preenche
///   pelo conteúdo; ⌘T transformação livre, Enter confirma, Esc cancela;
/// - área de transferência: ⌘C copiar, ⇧⌘C copiar mesclado, ⌘X recortar, ⌘V
///   colar (no meio da vista, ou da seleção), ⇧⌘V colar no lugar; ⇧⌥⌘E
///   carimbar visível; ⌘I inverter (a máscara, ou as cores da camada); `\`
///   a máscara em rubi; `/` o cadeado da transparência. Fora de campo de
///   texto: o campo do nome da camada tem a área de transferência dele;
/// - zoom: ⌘= ⌘− ⌘0 ⌘⌥0; **Z é a Lupa**, como no Photoshop (a tecla de
///   alternar encaixe e 100% é da Revelação, não do editor);
/// - Tab esconde barra de ferramentas, opções e painéis; ⇧Tab só os painéis;
/// - as ferramentas e as letras saem de `janela::ferramentas::FERRAMENTAS`.
///
/// 🔑 **`secondary`** é o ⌘ no macOS e o Ctrl no Windows e no Linux — a regra da
/// plataforma, sem o Ctrl fazendo as vezes do ⌘ no Mac. O balcão roda Windows
/// e Linux; ali o Ctrl+Y também refaz.
pub fn init(cx: &mut gpui_kit::App) {
    use gpui_kit::KeyBinding;
    let c = Some(CONTEXTO);
    let solta = Some(SEM_CAMPO);
    cx.bind_keys([
        KeyBinding::new("secondary-shift-z", RefazerNoEditor, c),
        KeyBinding::new("secondary-z", DesfazerNoEditor, c),
        KeyBinding::new("secondary-s", SalvarNoEditor, c),
        KeyBinding::new("secondary-w", FecharEditor, c),
        KeyBinding::new("[", PincelMenor, solta),
        KeyBinding::new("]", PincelMaior, solta),
        KeyBinding::new("shift-[", DurezaMenor, solta),
        KeyBinding::new("shift-]", DurezaMaior, solta),
        KeyBinding::new("{", DurezaMenor, solta),
        KeyBinding::new("}", DurezaMaior, solta),
        KeyBinding::new("secondary-,", AlternarCamada, c),
        KeyBinding::new("secondary-shift-n", NovaCamada, c),
        KeyBinding::new("secondary-j", DuplicarCamada, c),
        KeyBinding::new("secondary-]", SubirCamada, c),
        KeyBinding::new("secondary-[", DescerCamada, c),
        KeyBinding::new("alt-]", CamadaDeCima, solta),
        KeyBinding::new("alt-[", CamadaDeBaixo, solta),
        KeyBinding::new("secondary-=", Aproximar, c),
        KeyBinding::new("secondary-+", Aproximar, c),
        KeyBinding::new("secondary--", Afastar, c),
        KeyBinding::new("secondary-0", Encaixar, c),
        KeyBinding::new("secondary-alt-0", UmPorUm, c),
        KeyBinding::new("tab", AlternarInterface, solta),
        KeyBinding::new("shift-tab", AlternarPaineis, solta),
        KeyBinding::new("space", SegurarAMao, solta),
        KeyBinding::new("secondary-a", SelecionarTudo, c),
        KeyBinding::new("secondary-d", Desmarcar, c),
        KeyBinding::new("secondary-shift-i", InverterSelecao, c),
        KeyBinding::new("backspace", ApagarSelecao, solta),
        KeyBinding::new("delete", ApagarSelecao, solta),
        KeyBinding::new("alt-backspace", PreencherSelecao, solta),
        KeyBinding::new("alt-delete", PreencherSelecao, solta),
        KeyBinding::new("secondary-shift-j", CamadaViaRecorte, c),
        KeyBinding::new("secondary-t", TransformacaoLivre, c),
        KeyBinding::new("enter", AplicarTransformacao, solta),
        KeyBinding::new("escape", CancelarTransformacao, solta),
        KeyBinding::new("x", TrocarCores, solta),
        KeyBinding::new("d", CoresPadrao, solta),
        KeyBinding::new("shift-f6", DifundirSelecao, solta),
        KeyBinding::new("shift-backspace", PreencherPeloConteudo, solta),
        KeyBinding::new("shift-delete", PreencherPeloConteudo, solta),
        KeyBinding::new("secondary-e", MesclarParaBaixo, c),
        KeyBinding::new("secondary-alt-g", AlternarMascaraDeCorte, c),
        KeyBinding::new("secondary-c", Copiar, solta),
        KeyBinding::new("secondary-shift-c", CopiarMesclado, solta),
        KeyBinding::new("secondary-x", Recortar, solta),
        KeyBinding::new("secondary-v", Colar, solta),
        KeyBinding::new("secondary-shift-v", ColarNoLugar, solta),
        KeyBinding::new("secondary-shift-alt-e", CarimbarVisivel, c),
        KeyBinding::new("secondary-i", Inverter, solta),
        KeyBinding::new("\\", AlternarRubi, solta),
        KeyBinding::new("/", BloquearTransparencia, solta),
        KeyBinding::new("secondary-shift-x", Liquidificar, c),
        KeyBinding::new("y", AlternarAntesDepois, solta),
        // As letras e ⇧ + letra. No grupo de uma ferramenta só, ⇧ + letra
        // escolhe a mesma.
        KeyBinding::new("v", GrupoV, solta),
        KeyBinding::new("shift-v", GrupoV, solta),
        KeyBinding::new("m", GrupoM, solta),
        KeyBinding::new("shift-m", ProximaDoGrupoM, solta),
        KeyBinding::new("l", GrupoL, solta),
        KeyBinding::new("shift-l", ProximaDoGrupoL, solta),
        KeyBinding::new("w", GrupoW, solta),
        KeyBinding::new("shift-w", GrupoW, solta),
        KeyBinding::new("i", GrupoI, solta),
        KeyBinding::new("shift-i", GrupoI, solta),
        KeyBinding::new("j", GrupoJ, solta),
        KeyBinding::new("shift-j", ProximaDoGrupoJ, solta),
        KeyBinding::new("b", GrupoB, solta),
        KeyBinding::new("shift-b", GrupoB, solta),
        KeyBinding::new("s", GrupoS, solta),
        KeyBinding::new("shift-s", GrupoS, solta),
        KeyBinding::new("e", GrupoE, solta),
        KeyBinding::new("shift-e", GrupoE, solta),
        KeyBinding::new("g", GrupoG, solta),
        KeyBinding::new("shift-g", ProximaDoGrupoG, solta),
        KeyBinding::new("o", GrupoO, solta),
        KeyBinding::new("shift-o", ProximaDoGrupoO, solta),
        KeyBinding::new("h", GrupoH, solta),
        KeyBinding::new("shift-h", GrupoH, solta),
        KeyBinding::new("r", GrupoR, solta),
        KeyBinding::new("shift-r", GrupoR, solta),
        KeyBinding::new("z", GrupoZ, solta),
        KeyBinding::new("shift-z", GrupoZ, solta),
    ]);
    if !cfg!(target_os = "macos") {
        cx.bind_keys([KeyBinding::new("ctrl-y", RefazerNoEditor, c)]);
    }
}
