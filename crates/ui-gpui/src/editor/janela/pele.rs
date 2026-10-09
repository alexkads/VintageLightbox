//! 🧴 O painel **Tratamento de pele**: os comandos do fluxo manual do
//! Photoshop num lugar só, sem esconder as ferramentas gerais.
//!
//! - Separação de frequências… / Regenerar…;
//! - **Retocar em** Baixa (tom) ou Alta (textura) — escolhe a camada e põe a
//!   amostra do carimbo em "Camada atual";
//! - **Ver** recomposta, só a baixa, só a alta, ou o original (só a tela);
//! - **Intensidade do tratamento**: a opacidade da baixa, que vale para o
//!   conjunto (mistura o tratado com a referência — ver
//!   `editor_core::sessao::pele`), um passo ao soltar;
//! - as ferramentas de cada frequência: Carimbo, Recuperação, Suavizar tons,
//!   Pincel misturador;
//! - **Dodge & Burn**: cria as duas Curvas em Luminosidade (máscaras pretas)
//!   e escolhe Clarear ou Escurecer com o pincel macio de fluxo baixo.
//!
//! Nada aqui decide sozinho onde retocar: sem detecção de rosto nem IA.

use gpui_kit::component::button::ButtonGroup;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Entity, Subscription, Window};

use super::aparencia;
use super::filtro::Tipo;
use super::EditorDeFoto;
use editor_core::{Ferramenta, Frequencia, Sessao, VistaDaSeparacao};

pub struct Estado {
    intensidade: Entity<SliderState>,
    /// As opções do Pincel misturador (em %).
    pub(super) umidade: Entity<SliderState>,
    pub(super) carga: Entity<SliderState>,
    pub(super) mistura: Entity<SliderState>,
    _assinaturas: Vec<Subscription>,
}

impl Estado {
    pub fn novo(window: &mut Window, cx: &mut Context<EditorDeFoto>) -> Self {
        let intensidade = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(100.0)
        });
        let assinatura = cx.subscribe_in(
            &intensidade,
            window,
            |ed: &mut EditorDeFoto, _s, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = match evento {
                    SliderEvent::Change(v) => (v.start(), false),
                    SliderEvent::Release(v) => (v.start(), true),
                };
                ed.mover_intensidade_do_tratamento(v / 100.0, soltou, cx);
            },
        );
        let pct = |v: f32, cx: &mut Context<EditorDeFoto>| {
            cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(100.0)
                    .step(1.0)
                    .default_value(v)
            })
        };
        let o = editor_core::misturador::OpcoesDoMisturador::default();
        let (umidade, carga, mistura) = (
            pct(o.umidade * 100.0, cx),
            pct(o.carga * 100.0, cx),
            pct(o.mistura * 100.0, cx),
        );
        let mut assinaturas = vec![assinatura];
        for (estado, qual) in [(&umidade, 0u8), (&carga, 1), (&mistura, 2)] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut EditorDeFoto, _s, evento: &SliderEvent, _w, cx| {
                    let v = match evento {
                        SliderEvent::Change(v) | SliderEvent::Release(v) => v.start() / 100.0,
                    };
                    if let Some(s) = ed.sessao_mut() {
                        let m = &mut s.misturador;
                        match qual {
                            0 => m.umidade = v,
                            1 => m.carga = v,
                            _ => m.mistura = v,
                        }
                    }
                    cx.notify();
                },
            ));
        }
        Self {
            intensidade,
            umidade,
            carga,
            mistura,
            _assinaturas: assinaturas,
        }
    }
}

impl EditorDeFoto {
    /// O arrasto do controle de intensidade.
    pub fn mover_intensidade_do_tratamento(
        &mut self,
        valor: f32,
        soltou: bool,
        cx: &mut Context<Self>,
    ) {
        if self.separacao.aberto.is_some() {
            return;
        }
        if let Some(s) = self.sessao_mut() {
            if s.mover_intensidade(valor) && soltou {
                s.confirmar_opacidade();
            }
        }
        cx.notify();
    }

    /// O slider acompanha o conjunto (o desfazer, a reabertura), e os do
    /// misturador as opções.
    pub(super) fn sincronizar_a_pele(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(m) = self.sessao().map(|s| s.misturador) {
            for (estado, v) in [
                (self.pele.umidade.clone(), m.umidade),
                (self.pele.carga.clone(), m.carga),
                (self.pele.mistura.clone(), m.mistura),
            ] {
                if (estado.read(cx).value().start() - v * 100.0).abs() > 0.5 {
                    estado.update(cx, |s, cx| s.set_value(v * 100.0, window, cx));
                }
            }
        }
        let Some(v) = self.sessao().and_then(Sessao::intensidade_do_tratamento) else {
            return;
        };
        let v = v * 100.0;
        if (self.pele.intensidade.read(cx).value().start() - v).abs() > 0.5 {
            self.pele
                .intensidade
                .update(cx, |s, cx| s.set_value(v, window, cx));
        }
    }

    /// As caixas e os botões do misturador.
    pub fn alternar_opcao_do_misturador(&mut self, qual: u8, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            let m = &mut s.misturador;
            match qual {
                0 => m.carregar_apos = !m.carregar_apos,
                1 => m.limpar_apos = !m.limpar_apos,
                _ => m.todas_as_camadas = !m.todas_as_camadas,
            }
        }
        cx.notify();
    }

    /// "Carregar o pincel" (a cor de frente) ou "Limpar o pincel".
    pub fn carregar_o_misturador(&mut self, carregar: bool, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            if carregar {
                let cor = s.pincel.cor;
                s.tinta.carregar(cor);
            } else {
                s.tinta.limpar();
            }
        }
        cx.notify();
    }

    /// Retocar na baixa ou na alta.
    pub fn retocar_em(&mut self, f: Frequencia, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.escolher_frequencia(f);
        });
    }

    /// O que a tela mostra do conjunto (só a tela).
    pub fn ver_o_tratamento(&mut self, vista: VistaDaSeparacao, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.exibir_separacao(vista);
        }
        cx.notify();
    }

    /// "Suavizar tons…": na baixa (escolhida antes, se há conjunto).
    pub fn suavizar_tons(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .sessao()
            .is_some_and(|s| s.conjunto_de_pele().is_some() && s.frequencia_escolhida().is_none())
        {
            self.retocar_em(Frequencia::Baixa, cx);
        }
        self.abrir_filtro(Tipo::SuavizarTons, window, cx);
    }

    /// "Dodge & Burn": cria as camadas, ou escolhe Clarear/Escurecer.
    pub fn dodge_and_burn(&mut self, clarear: Option<bool>, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| match clarear {
            None => {
                s.criar_dodge_and_burn();
            }
            Some(c) => {
                s.escolher_dodge_and_burn(c);
            }
        });
        // O pincel na mão (as opções da barra são as dele).
        self.usar(Ferramenta::Pincel, cx);
    }

    /// Uma ferramenta do fluxo, na frequência certa: carimbo e recuperação
    /// na alta, o misturador na baixa (quando há conjunto e nenhuma
    /// frequência escolhida).
    pub fn ferramenta_da_pele(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        let frequencia = match ferramenta {
            Ferramenta::Misturador => Frequencia::Baixa,
            _ => Frequencia::Alta,
        };
        if self
            .sessao()
            .is_some_and(|s| s.conjunto_de_pele().is_some() && s.frequencia_escolhida().is_none())
        {
            self.retocar_em(frequencia, cx);
        }
        self.usar(ferramenta, cx);
    }

    /// O roteiro: `pele baixa|alta|ver X|intensidade V|db|clarear|escurecer|misturador|carimbo|recuperacao|suavizar|estado`.
    pub(super) fn roteiro_da_pele(
        &mut self,
        partes: &[&str],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match partes.get(1).copied().unwrap_or_default() {
            "painel" => self.mostrar_painel(super::QualPainel::Pele, window, cx),
            "baixa" => self.retocar_em(Frequencia::Baixa, cx),
            "alta" => self.retocar_em(Frequencia::Alta, cx),
            "ver" => self.ver_o_tratamento(
                match partes.get(2).copied().unwrap_or_default() {
                    "baixa" => VistaDaSeparacao::Baixa,
                    "alta" => VistaDaSeparacao::Alta,
                    "original" | "antes" => VistaDaSeparacao::Original,
                    _ => VistaDaSeparacao::Recomposta,
                },
                cx,
            ),
            "intensidade" => {
                let v = partes
                    .get(2)
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(100.0);
                self.mover_intensidade_do_tratamento(v / 100.0, true, cx);
            }
            "db" => self.dodge_and_burn(None, cx),
            "clarear" => self.dodge_and_burn(Some(true), cx),
            "escurecer" => self.dodge_and_burn(Some(false), cx),
            "misturador" => self.ferramenta_da_pele(Ferramenta::Misturador, cx),
            "carimbo" => self.ferramenta_da_pele(Ferramenta::Carimbo, cx),
            "recuperacao" => self.ferramenta_da_pele(Ferramenta::Recuperacao, cx),
            "suavizar" => self.suavizar_tons(window, cx),
            // pele misturar umidade|carga|mistura V | carregar | limpar | todas
            "misturar" => {
                let v = partes
                    .get(3)
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(50.0)
                    / 100.0;
                match partes.get(2).copied().unwrap_or_default() {
                    "carregar" => self.carregar_o_misturador(true, cx),
                    "limpar" => self.carregar_o_misturador(false, cx),
                    "todas" => self.alternar_opcao_do_misturador(2, cx),
                    q => {
                        if let Some(s) = self.sessao_mut() {
                            match q {
                                "umidade" => s.misturador.umidade = v,
                                "carga" => s.misturador.carga = v,
                                "mistura" => s.misturador.mistura = v,
                                _ => {}
                            }
                        }
                    }
                }
                cx.notify();
            }
            _ => {
                let s = self.sessao();
                eprintln!(
                    "[roteiro] editor pele: conjunto={:?} frequencia={:?} vista={:?} intensidade={:?} ativa={:?} na_mascara={} amostra={:?} ferramenta={:?} posicao={:?}",
                    s.and_then(Sessao::conjunto_de_pele),
                    s.and_then(Sessao::frequencia_escolhida),
                    s.map(Sessao::vista_da_separacao),
                    s.and_then(Sessao::intensidade_do_tratamento),
                    s.map(Sessao::ativa),
                    self.na_mascara(),
                    s.map(|s| s.carimbo.amostra),
                    s.map(|s| s.pincel.ferramenta),
                    s.map(|s| s.historico().posicao()),
                );
            }
        }
    }

    /// O painel.
    pub(super) fn painel_da_pele(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let pronta = self.pronta();
        let s = self.sessao();
        let conjunto = s.and_then(Sessao::conjunto_de_pele);
        let raio = conjunto.and_then(|(b, _)| {
            s.and_then(|s| s.documento().camadas[b].retoque)
                .and_then(|r| r.raio())
        });
        let frequencia = s.and_then(Sessao::frequencia_escolhida);
        let vista = s.map(Sessao::vista_da_separacao).unwrap_or_default();
        let intensidade = s.and_then(Sessao::intensidade_do_tratamento).unwrap_or(1.0);
        let com_db = s.is_some_and(|s| s.camada_do_dodge_and_burn(true).is_some());
        let db_escolhido = s.and_then(|s| {
            let ativa = s.ativa();
            match s.documento().camadas.get(ativa)?.retoque? {
                editor_core::Retoque::Clarear => Some(true),
                editor_core::Retoque::Escurecer => Some(false),
                _ => None,
            }
        });
        let ferramenta = self.ferramenta();
        let titulo = |t: &'static str| {
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(c.apagado)
                .child(t)
        };
        let dica = |t: String| div().text_xs().text_color(c.apagado).child(t);
        let escolha = |id: &'static str, texto: &'static str, aceso: bool, cx: &Context<Self>| {
            if aceso {
                crate::estilo::botao_primario_pequeno(id, cx)
            } else {
                crate::estilo::botao_contorno_pequeno(id, cx)
            }
            .debug_selector(move || id.into())
            .label(texto)
            .selected(aceso)
            .disabled(!pronta)
        };
        let botao = |id: &'static str, texto: &'static str, cx: &Context<Self>| {
            crate::estilo::botao_contorno_pequeno(id, cx)
                .debug_selector(move || id.into())
                .child(texto)
                .disabled(!pronta)
        };

        let mut corpo = div()
            .id("editor-corpo-pele")
            .debug_selector(|| "editor-corpo-pele".into())
            .size_full()
            .overflow_y_scroll()
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(10.));

        // Separação.
        corpo = corpo.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(titulo("SEPARAÇÃO DE FREQUÊNCIAS"))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(botao("editor-pele-separar", "Separar…", cx).on_click(
                            cx.listener(|ed, _, window, cx| ed.abrir_separacao(false, window, cx)),
                        ))
                        .when(conjunto.is_some(), |d| {
                            d.child(
                                botao("editor-pele-regenerar", "Regenerar…", cx)
                                    .tooltip("Outro raio, com os retoques assados na nova divisão")
                                    .on_click(cx.listener(|ed, _, window, cx| {
                                        ed.abrir_separacao(true, window, cx)
                                    })),
                            )
                        }),
                )
                .child(dica(match raio {
                    Some(r) => format!(
                        "Raio {} px · baixa = tom e cor, alta = textura",
                        format!("{r:.1}").replace('.', ",")
                    ),
                    None => "Nenhuma separação no documento.".into(),
                })),
        );

        if conjunto.is_some() {
            let freqs = ButtonGroup::new("editor-pele-retocar-em")
                .xsmall()
                .child(escolha(
                    "editor-pele-baixa",
                    "Baixa (tom)",
                    frequencia == Some(Frequencia::Baixa),
                    cx,
                ))
                .child(escolha(
                    "editor-pele-alta",
                    "Alta (textura)",
                    frequencia == Some(Frequencia::Alta),
                    cx,
                ))
                .on_click(
                    cx.listener(|ed, cliques: &Vec<usize>, _, cx| match cliques.first() {
                        Some(0) => ed.retocar_em(Frequencia::Baixa, cx),
                        Some(1) => ed.retocar_em(Frequencia::Alta, cx),
                        _ => {}
                    }),
                );
            const VISTAS: [(VistaDaSeparacao, &str, &str); 4] = [
                (
                    VistaDaSeparacao::Recomposta,
                    "editor-pele-ver-recomposta",
                    "Resultado",
                ),
                (VistaDaSeparacao::Baixa, "editor-pele-ver-baixa", "Baixa"),
                (VistaDaSeparacao::Alta, "editor-pele-ver-alta", "Alta"),
                (
                    VistaDaSeparacao::Original,
                    "editor-pele-ver-original",
                    "Original",
                ),
            ];
            let vistas = ButtonGroup::new("editor-pele-ver")
                .xsmall()
                .children(
                    VISTAS
                        .iter()
                        .map(|(v, id, t)| escolha(id, t, vista == *v, cx)),
                )
                .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                    if let Some((v, _, _)) = cliques.first().and_then(|i| VISTAS.get(*i)) {
                        ed.ver_o_tratamento(*v, cx);
                    }
                }));
            corpo = corpo
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(titulo("RETOCAR EM"))
                        .child(freqs),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(titulo("VER"))
                        .child(vistas),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(titulo("INTENSIDADE DO TRATAMENTO"))
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("{}%", (intensidade * 100.0).round())),
                                ),
                        )
                        .child(
                            div()
                                .h(px(20.))
                                .debug_selector(|| "editor-pele-intensidade".into())
                                .child(crate::estilo::slider(&self.pele.intensidade)),
                        )
                        .child(dica(
                            "Mistura o resultado tratado com o original; onde nada foi retocado, a foto fica igual.".into(),
                        )),
                );
        }

        // Ferramentas.
        let ferr = |id: &'static str,
                    texto: &'static str,
                    f: Ferramenta,
                    dica: &'static str,
                    cx: &Context<Self>| {
            escolha(id, texto, ferramenta == Some(f), cx)
                .tooltip(dica)
                .on_click(cx.listener(move |ed, _, _, cx| ed.ferramenta_da_pele(f, cx)))
        };
        corpo = corpo.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(titulo("FERRAMENTAS"))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(ferr(
                            "editor-pele-carimbo",
                            "Carimbo",
                            Ferramenta::Carimbo,
                            "Textura na alta: ⌥ + clique na pele boa, depois pinte sobre a imperfeição",
                            cx,
                        ))
                        .child(ferr(
                            "editor-pele-recuperacao",
                            "Recuperação",
                            Ferramenta::Recuperacao,
                            "Na alta, a correção é aditiva: a textura vem da origem e o tom não muda",
                            cx,
                        ))
                        .child(
                            botao("editor-pele-suavizar", "Suavizar tons…", cx)
                                .tooltip("Na baixa: desfoque só com os pixels da seleção (difunda a seleção antes)")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.suavizar_tons(window, cx)
                                })),
                        )
                        .child(ferr(
                            "editor-pele-misturador",
                            "Misturador",
                            Ferramenta::Misturador,
                            "Na baixa: mistura a cor carregada com a da tela (umidade, carga e mistura na barra)",
                            cx,
                        )),
                ),
        );

        // Dodge & Burn.
        let db = if com_db {
            ButtonGroup::new("editor-pele-db")
                .xsmall()
                .child(escolha(
                    "editor-pele-clarear",
                    "Clarear",
                    db_escolhido == Some(true),
                    cx,
                ))
                .child(escolha(
                    "editor-pele-escurecer",
                    "Escurecer",
                    db_escolhido == Some(false),
                    cx,
                ))
                .on_click(
                    cx.listener(|ed, cliques: &Vec<usize>, _, cx| match cliques.first() {
                        Some(0) => ed.dodge_and_burn(Some(true), cx),
                        Some(1) => ed.dodge_and_burn(Some(false), cx),
                        _ => {}
                    }),
                )
                .into_any_element()
        } else {
            botao("editor-pele-criar-db", "Criar Dodge & Burn", cx)
                .tooltip("Duas Curvas em Luminosidade com a máscara preta: pinte de branco para clarear ou escurecer")
                .on_click(cx.listener(|ed, _, _, cx| ed.dodge_and_burn(None, cx)))
                .into_any_element()
        };
        corpo = corpo.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(titulo("DODGE & BURN"))
                .child(db)
                .child(dica(
                    "Só a luz muda (Luminosidade), não a cor. Pincel macio, fluxo 5%: construa aos poucos; a borracha devolve.".into(),
                )),
        );
        corpo.into_any_element()
    }
}
