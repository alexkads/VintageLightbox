//! 🗑️ **Descartar**: a edição que não foi salva sai, e a foto volta a ser o
//! que a galeria do site tem.
//!
//! Pedido do dono (27/set/2026): *"as vezes entramos numa foto para testar o
//! funcionamento de uma ferramenta e o botão fica ativado [...] preciso
//! conseguir descartar todas as mudanças com clique ou uma mudança específica
//! de uma foto"* — e, logo depois, *"na verdade é deixar como estava no
//! servidor"*. Sem isto, o único jeito de apagar o "Salvar na galeria e sair"
//! aceso era desfazer passo a passo, e o `⌘Z` só alcança a foto aberta.
//!
//! 🔑 **É o "Descartar a edição" da web** (`grade.tsx`): a receita local sai
//! do depósito, a foto sai da fila do "Salvar", e fica como a galeria tem.
//!
//! # Quem faz o quê
//!
//! A tela só sabe **quais** fotos: quem sabe a receita do site, a fila e o
//! depósito é a raiz (`app.rs`, `descartar_as_edicoes`). A foto aberta volta
//! por [`Revelacao::assumir_a_receita_do_site`], como um passo de histórico —
//! o `⌘Z` a traz de volta, e por isso ela não pergunta. As outras não têm
//! histórico: descartá-las pergunta antes, como a web.
//!
//! ⚠️ **A foto que nunca subiu não tem "o que está no site"**: ela volta ao que
//! era quando a Revelação a abriu.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui_kit::component::button::{Button, ButtonVariant};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt};
use gpui_kit::{div, prelude::*, AnyElement, Context, SharedString, Window};
use infrastructure::gpu_adjustments::ReceitaLocal;

use super::super::historico::Estado;
use super::super::persistencia::{self, Corte};
use super::super::processador::Ajustes;
use super::{PedidoDaRevelacao, Revelacao};
use crate::recursos::Icone;

impl Revelacao {
    /// Os ids **no site** das fotos com edição não salva: as que a raiz
    /// contou (fila e depósito) e a aberta, se ela mudou.
    pub fn a_descartar_todas(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.tira.pendentes.iter().cloned().collect();
        if self.aberta_a_salvar() {
            if let Some(no_site) = self.foto_aberta().and_then(|f| f.pos_venda_foto_id.clone()) {
                ids.push(no_site);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// A aberta mudou e não tem lugar no site — ela volta ao que abriu.
    fn aberta_local_mudou(&self) -> bool {
        self.aberta_a_salvar()
            && self
                .foto_aberta()
                .is_some_and(|f| f.pos_venda_foto_id.is_none())
    }

    /// Quantas o "Descartar todas" leva.
    pub fn quantas_a_descartar(&self) -> usize {
        self.a_descartar_todas().len() + usize::from(self.aberta_local_mudou())
    }

    /// O que a raiz vai atender — ela leva e esvazia.
    pub fn levar_a_descartar(&mut self) -> Vec<String> {
        std::mem::take(&mut self.tira.a_descartar)
    }

    fn pedir_o_descarte(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.tira.a_descartar = ids;
        cx.emit(PedidoDaRevelacao::Descartar);
    }

    /// **"Descartar esta foto"** — sem pergunta: é um passo de histórico, e o
    /// `⌘Z` a traz de volta.
    pub fn descartar_esta(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.aberta_a_salvar() {
            return;
        }
        match self.foto_aberta().and_then(|f| f.pos_venda_foto_id.clone()) {
            Some(no_site) => self.pedir_o_descarte(vec![no_site], cx),
            None => self.voltar_ao_que_abriu(window, cx),
        }
    }

    /// A foto que só existe aqui volta à receita com que abriu.
    fn voltar_ao_que_abriu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(estado) = self.receita_ao_abrir.clone() else {
            return;
        };
        self.gravar_o_que_estiver_pendente();
        self.historico.registrar(estado.clone());
        self.aplicar_do_historico(estado, window, cx);
    }

    /// **"Descartar todas (N)"**. Só a aberta é o mesmo que "esta foto"; com
    /// outras junto, pergunta antes — delas não há `⌘Z`.
    pub fn pedir_descartar_todas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self.a_descartar_todas();
        let local = self.aberta_local_mudou();
        let aberta = self.foto_aberta().and_then(|f| f.pos_venda_foto_id.clone());
        let so_a_aberta = match (ids.as_slice(), local) {
            ([], true) => true,
            ([uma], false) => self.aberta_a_salvar() && Some(uma) == aberta.as_ref(),
            _ => false,
        };
        if so_a_aberta {
            self.descartar_esta(window, cx);
            return;
        }
        if ids.is_empty() && !local {
            return;
        }
        self.confirmar_o_descarte(ids, local, window, cx);
    }

    /// "Descartar a edição" do menu da tira: os alvos com edição não salva.
    pub(super) fn descartar_pelo_menu(
        &mut self,
        alvos: Vec<PhotoViewModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let aberta = self.foto_aberta().map(|f| f.id.clone());
        let local = alvos
            .iter()
            .any(|f| Some(&f.id) == aberta.as_ref() && f.pos_venda_foto_id.is_none())
            && self.aberta_local_mudou();
        let ids: Vec<String> = alvos
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.clone())
            .collect();
        let so_a_aberta = alvos.len() == 1 && Some(&alvos[0].id) == aberta.as_ref();
        if so_a_aberta {
            self.descartar_esta(window, cx);
        } else if !ids.is_empty() || local {
            self.confirmar_o_descarte(ids, local, window, cx);
        }
    }

    /// A pergunta — o `useConfirmacao` destrutivo da web, com os textos dela.
    fn confirmar_o_descarte(
        &mut self,
        ids: Vec<String>,
        local: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let quantas = ids.len() + usize::from(local);
        let esta = cx.entity();
        window.open_alert_dialog(cx, move |dialogo, _window, _cx| {
            let para_ok = esta.clone();
            let ids = ids.clone();
            dialogo
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if quantas == 1 {
                            SharedString::from("Descartar a revelação")
                        } else {
                            SharedString::from(format!("Descartar as {quantas}"))
                        })
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("Cancelar")
                        .show_cancel(true),
                )
                .title(if quantas == 1 {
                    SharedString::from("Descartar a revelação desta foto?")
                } else {
                    SharedString::from(format!("Descartar a revelação de {quantas} fotos?"))
                })
                .child(div().text_sm().child(
                    "O que foi revelado aqui e ainda não foi salvo na galeria é apagado. \
                     Cada foto volta a ficar como está na galeria do cliente.",
                ))
                // 🚨 Fecha na hora e devolve `false` — o mesmo do "Sincronizar":
                // deixado ao kit, o foco fica num diálogo que já saiu da tela.
                .on_ok(move |_ev, window, cx| {
                    para_ok.update(cx, |tela, cx| {
                        if local {
                            tela.voltar_ao_que_abriu(window, cx);
                        }
                        tela.pedir_o_descarte(ids.clone(), cx);
                    });
                    window.close_dialog(cx);
                    false
                })
                .on_cancel(|_ev, window, cx| {
                    window.close_dialog(cx);
                    false
                })
        });
    }

    /// A raiz devolveu a receita do site para a foto **aberta**.
    ///
    /// 🔑 **Vira passo de histórico e não grava.** Gravar poria a receita no
    /// depósito numa tarefa do tokio, e a raiz a tira de lá em outra — sem
    /// ordem garantida entre as duas, o disco podia ficar com a linha, e a foto
    /// voltava "não salva" na próxima abertura do app. O `⌘Z` depois disto
    /// grava como sempre (`aplicar_do_historico`), e a foto volta à fila.
    ///
    /// `locais`: a Revelação local que está no site; `None` quando não se sabe
    /// (o site não a guarda — ver `app.rs`), e aí a de agora fica.
    pub fn assumir_a_receita_do_site(
        &mut self,
        ajustes: Ajustes,
        corte: Corte,
        locais: Option<Arc<ReceitaLocal>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.foto_aberta().map(|f| f.id.clone()) else {
            return;
        };
        // O gesto em curso entra no histórico (o `⌘Z` o alcança), e a espera
        // da gravação morre: ela gravaria depois do descarte, por cima dele.
        self.fechar_o_gesto_pendente();
        self._gravacao = None;
        let estado = Estado {
            ajustes,
            corte,
            locais: locais.unwrap_or_else(|| self.locais.clone()),
        };
        self.historico.registrar(estado.clone());
        self.mostrar_o_estado(estado.clone(), window, cx);
        // O marco do botão de salvar passa a ser a galeria.
        self.receita_ao_abrir = Some(estado);
        self.bases.remove(&id);
        if let Some(aberta) = self.aberta.as_mut() {
            persistencia::na_foto(&mut aberta.foto, ajustes, corte);
        }
        let acervo = Arc::make_mut(&mut self.acervo);
        if let Some(foto) = acervo.iter_mut().find(|f| f.id == id) {
            persistencia::na_foto(foto, ajustes, corte);
        }
        // A receita local não mora no depósito: tabela própria, sem corrida.
        self.gravar_locais_se_mudou(&id);
        cx.notify();
    }

    /// As **outras** descartadas: as cópias da tira passam a dizer o que a
    /// galeria tem — senão a seta seguinte abriria a foto com a edição que
    /// acabou de sair.
    pub fn aplicar_descartadas(
        &mut self,
        receitas: &[(String, Ajustes, Corte)],
        cx: &mut Context<Self>,
    ) {
        let acervo = Arc::make_mut(&mut self.acervo);
        for (id, ajustes, corte) in receitas {
            if let Some(foto) = acervo.iter_mut().find(|f| &f.id == id) {
                persistencia::na_foto(foto, *ajustes, *corte);
            }
            self.bases.remove(id);
        }
        cx.notify();
    }

    /// O botão da barra, com o menu das duas escolhas.
    pub(super) fn botao_de_descartar(&self, ocupado: bool, cx: &mut Context<Self>) -> AnyElement {
        let quantas = self.quantas_a_descartar();
        let esta_mudou = self.aberta_a_salvar();
        let tela = cx.entity().downgrade();
        Button::new("revelacao-descartar")
            .icon(Icon::new(Icone::RotateCcw))
            .label("Descartar")
            .small()
            .outline()
            .tooltip(if quantas == 0 {
                "Nada a descartar: o que está no canvas já é o que está na galeria"
            } else {
                "Descartar a revelação não salva — a foto volta a ficar como está na galeria"
            })
            .disabled(quantas == 0 || ocupado)
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, window, cx| {
                // 🚨 O menu devolve o foco a quem estiver no `action_context`
                // — sem ele o foco ficava num menu que já fechou (ver a tira).
                let menu = match window.focused(cx) {
                    Some(antes) => menu.action_context(antes),
                    None => menu,
                };
                let (para_esta, para_todas) = (tela.clone(), tela.clone());
                menu.item(
                    PopupMenuItem::new("Descartar esta foto")
                        .disabled(!esta_mudou)
                        .on_click(move |_ev, window, cx| {
                            let _ =
                                para_esta.update(cx, |tela, cx| tela.descartar_esta(window, cx));
                        }),
                )
                .item(
                    PopupMenuItem::new(format!("Descartar todas ({quantas})"))
                        .disabled(quantas == 0)
                        .on_click(move |_ev, window, cx| {
                            let _ = para_todas
                                .update(cx, |tela, cx| tela.pedir_descartar_todas(window, cx));
                        }),
                )
            })
            .into_any_element()
    }
}
