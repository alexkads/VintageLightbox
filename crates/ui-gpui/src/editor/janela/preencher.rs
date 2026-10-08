//! 🪣 O diálogo "Preencher" do Photoshop (⇧⌫ / Editar › Preencher…): o
//! conteúdo — sensível ao conteúdo (o remendo direto, sem prévia), a cor de
//! frente ou de fundo, preto, 50% cinza ou branco — na seleção. O
//! Preenchimento sensível ao conteúdo com prévia, o PatchMatch e a IA local
//! continuam no espaço próprio (Editar › Preenchimento sensível ao
//! conteúdo…).

use gpui_kit::component::select::Select;
use gpui_kit::{div, prelude::*, px, AnyElement, Context, MouseButton, Window};

use super::EditorDeFoto;

/// (chave, nome) de cada conteúdo, na ordem do Photoshop.
pub const CONTEUDOS: [(&str, &str); 6] = [
    ("conteudo", "Sensível ao conteúdo"),
    ("frente", "Cor de frente"),
    ("fundo", "Cor de fundo"),
    ("preto", "Preto"),
    ("cinza", "50% cinza"),
    ("branco", "Branco"),
];

impl EditorDeFoto {
    /// ⇧⌫: abre o diálogo (o conteúdo é o da última vez).
    pub fn abrir_dialogo_de_preencher(&mut self, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() || !self.pronta() {
            return;
        }
        self.dialogo_de_preencher = true;
        cx.notify();
    }

    pub fn dialogo_de_preencher_aberto(&self) -> bool {
        self.dialogo_de_preencher
    }

    pub fn cancelar_preencher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialogo_de_preencher = false;
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// OK: preenche com o conteúdo escolhido.
    pub fn confirmar_preencher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chave = self
            .seletor_do_conteudo
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| CONTEUDOS[0].0.to_string());
        self.dialogo_de_preencher = false;
        window.focus(&self.foco, cx);
        self.preencher_com(&chave, cx);
    }

    /// O preenchimento de um conteúdo (o mesmo caminho do ⌥⌫ para as cores).
    pub fn preencher_com(&mut self, chave: &str, cx: &mut Context<Self>) {
        if chave == "conteudo" {
            self.preencher_a_selecao_pelo_conteudo(cx);
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let cor = match chave {
            "frente" => s.pincel.cor,
            "fundo" => s.pincel.cor_de_fundo,
            "preto" => [0; 3],
            "cinza" => [128; 3],
            _ => [255; 3],
        };
        let antes = s.pincel.cor;
        s.pincel.cor = cor;
        self.preencher_selecao(cx);
        if let Some(s) = self.sessao_mut() {
            s.pincel.cor = antes;
        }
        cx.notify();
    }

    pub(super) fn dialogo_do_preencher(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        Some(
            gpui_kit::component::v_flex()
                .gap(px(16.))
                .debug_selector(|| "editor-dialogo-preencher".into())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(crate::estilo::cabecalho_do_dialogo(
                    "Preencher",
                    "Na seleção (sem seleção, na camada inteira). O Preenchimento sensível ao conteúdo com prévia e IA local fica em Editar.",
                    None,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_sm().child("Conteúdo"))
                        .child(
                            div()
                                .flex_1()
                                .debug_selector(|| "editor-preencher-conteudo".into())
                                .child(crate::estilo::campo(Select::new(&self.seletor_do_conteudo))),
                        ),
                )
                .child(
                    crate::estilo::rodape_do_dialogo()
                        .child(
                            crate::estilo::botao_contorno("editor-preencher-cancelar", cx)
                                .child("Cancelar")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.cancelar_preencher(window, cx)
                                })),
                        )
                        .child(
                            crate::estilo::botao_primario("editor-preencher-ok", cx)
                                .child("OK")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.confirmar_preencher(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
