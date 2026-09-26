//! O zoom no palco e o Navegador — os gestos de `editor.tsx` e o
//! `navegador.tsx` do site, sobre a conta de [`crate::revelacao::zoom`].
//!
//! | Gesto | Faz |
//! |---|---|
//! | clique na foto | do encaixe para o zoom no ponto clicado; ampliada, volta ao encaixe |
//! | arrastar | move a foto ampliada |
//! | `⌘`/`Ctrl` + arrastar | a caixa marcada passa a encher a tela |
//! | `⌥` + arrastar | zoom contínuo em torno de onde começou |
//! | `⌘`/`Ctrl`/`⌥` + roda, e a pinça | zoom em torno do cursor |
//! | roda | move a foto ampliada |
//! | `Z` · `⌘=` · `⌘−` · `⌘0` · `⌘⌥0` · Home · End · PgDn · PgUp | ver `app.rs` |
//!
//! 🔑 **No Enquadrar não há zoom**, como no site: o retângulo é desenhado
//! sobre a foto encaixada, e o navegador fica apagado.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, Bounds, Context, Div, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point, RenderImage,
    ScrollWheelEvent, SharedString, Stateful,
};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, Sizable};

use super::Revelacao;
use crate::recursos::Icone;
use crate::revelacao::zoom::{self, Cena, EstadoDoZoom, Medidas, Nivel, Ponto, Vista};

/// A caixa da miniatura do navegador: a coluna de 224 px menos o respiro.
const LARGURA_DO_NAVEGADOR: f32 = 208.;
const ALTURA_DO_NAVEGADOR: f32 = 156.;

/// A folha de atalhos — o `ATALHOS` de `atalhos.ts`, na mesma ordem.
pub(super) const ATALHOS: [(&str, &[(&str, &str)]); 5] = [
    (
        "Enquadrar",
        &[
            ("R", "Entra e sai da ferramenta de enquadrar"),
            ("Enter", "Confirma o enquadramento e sai"),
            ("Esc", "Fecha o editor (na ferramenta, só sai dela)"),
            ("[", "Gira 90° à esquerda"),
            ("]", "Gira 90° à direita"),
            ("⇧H", "Espelha na horizontal"),
            ("⇧V", "Espelha na vertical"),
        ],
    ),
    (
        "Revelar",
        &[
            ("\\", "Segure para ver a foto sem ajuste nenhum"),
            ("⌘Z", "Desfaz"),
            ("⇧⌘Z", "Refaz"),
        ],
    ),
    (
        "Zoom",
        &[
            (
                "Z",
                "Alterna entre encaixar e o último zoom (segure para espiar)",
            ),
            (
                "Espaço",
                "Alterna o zoom; segure e arraste para mover a foto",
            ),
            ("⌘=", "Aproxima até a próxima parada"),
            ("⌘−", "Afasta até a parada anterior"),
            ("⌘⌥0", "1:1 — um pixel da foto num pixel da tela"),
            ("⌘0", "Encaixa a foto na tela"),
            ("Home", "Vai ao canto superior esquerdo da foto ampliada"),
            ("End", "Vai ao canto inferior direito"),
            ("PgDn", "Percorre a foto ampliada em Z, uma tela por vez"),
            ("PgUp", "Percorre em Z, de volta"),
        ],
    ),
    (
        "Comparar",
        &[
            (
                "⇧C",
                "Põe as marcadas lado a lado, aqui e na tela do cliente; de novo, volta a uma",
            ),
            (
                "Esc no Comparar",
                "Sai abrindo a foto escolhida (a da borda âmbar)",
            ),
            ("0–5", "No Comparar, dá a nota à escolhida (0 tira)"),
            ("P", "No Comparar, marca a escolhida como levada no balcão"),
            ("X", "No Comparar, rejeita a escolhida (de novo, desfaz)"),
        ],
    ),
    (
        "Andar e marcar",
        &[
            ("←", "Foto anterior (no Comparar, troca a outra foto)"),
            ("→", "Próxima foto (no Comparar, troca a outra foto)"),
            ("⌘A", "Marca todas as fotos da tira"),
            ("⌘D", "Marca só esta foto"),
            ("?", "Mostra e esconde esta lista"),
        ],
    ),
];

/// Os gestos do zoom que não são tecla.
const GESTOS_DO_ZOOM: [(&str, &str); 7] = [
    ("Clique na foto", "encaixar ↔ zoom no ponto clicado"),
    ("Arrastar", "move a foto ampliada"),
    ("⌘ + arrastar", "zoom na área marcada"),
    ("⌥ + arrastar", "zoom contínuo, para a direita aproxima"),
    ("⌘ + roda, ou pinça", "zoom em torno do cursor"),
    ("Roda", "move a foto ampliada"),
    ("Navegador", "clique ou arraste para levar a tela até lá"),
];

/// O que o mouse começou a fazer na foto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum TipoDeGesto {
    Clique,
    Caixa,
    Lupa,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Gesto {
    tipo: TipoDeGesto,
    inicio: Ponto,
    vista: Vista,
    moveu: bool,
}

/// O que a navegação guarda na tela.
#[derive(Debug, Clone, Copy)]
pub(super) struct Navegacao {
    pub zoom: EstadoDoZoom,
    /// Para onde o `Z` leva a partir do encaixe: o último zoom usado.
    pub alvo_do_z: Nivel,
    pub gesto: Option<Gesto>,
    /// A caixa do `⌘ + arrastar`, em pontos da área.
    pub caixa: Option<(Ponto, Ponto)>,
    /// Pixels do dispositivo por ponto, do último quadro.
    pub dpr: f32,
    /// Onde a miniatura do navegador está na janela.
    pub miniatura: Bounds<Pixels>,
    pub arrastando_miniatura: bool,
    /// Quando o `Z` desceu — segurado, soltar devolve o zoom de antes.
    pub z_desde: Option<Instant>,
    /// O Espaço segurado: quando desceu, e se já arrastou com ele.
    pub espaco: Option<(Instant, bool)>,
    /// Onde o ponteiro esteve por último na área — o `Z` amplia ali.
    pub ultimo_ponteiro: Option<Ponto>,
    /// A folha de atalhos (`?`).
    pub ajuda: bool,
}

impl Default for Navegacao {
    fn default() -> Self {
        Self {
            zoom: EstadoDoZoom::default(),
            alvo_do_z: Nivel::Razao(1.),
            gesto: None,
            caixa: None,
            dpr: 2.,
            miniatura: Bounds::default(),
            arrastando_miniatura: false,
            z_desde: None,
            espaco: None,
            ultimo_ponteiro: None,
            ajuda: false,
        }
    }
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

impl Revelacao {
    /// A cena do zoom agora, ou `None` fora dele (sem foto, ou no Enquadrar).
    pub(super) fn cena(&self) -> Option<Cena> {
        if self.edicao.is_some() {
            return None;
        }
        self.aberta.as_ref()?.desenhada.as_ref()?;
        // 🔑 **A janela é medida na cópia de trabalho**, e não na imagem
        // desenhada: com o bruto na tela ela tem outro tamanho, e o zoom não
        // pode pular quando a textura troca. O "1:1" do bruto entra pelo
        // `fator_do_bruto`, como no site.
        let copia = self.tamanho_da_copia()?;
        let corte = self.corte_atual();
        let janela = crate::revelacao::corte::retangulo_de(
            &corte,
            crate::revelacao::corte::espaco_de(&corte, copia),
        );
        let area = self.palco.size;
        if f(area.width) < 2. || f(area.height) < 2. || janela.w < 1. || janela.h < 1. {
            return None;
        }
        Some(Cena {
            janela: Medidas {
                largura: janela.w.round(),
                altura: janela.h.round(),
            },
            area: Medidas {
                largura: f(area.width),
                altura: f(area.height),
            },
            dpr: self.navegacao.dpr,
            fator_do_bruto: self.fator_do_bruto(),
        })
    }

    pub(super) fn vista(&self) -> Option<(Cena, Vista)> {
        let cena = self.cena()?;
        Some((cena, zoom::vista_do_zoom(self.navegacao.zoom, &cena)))
    }

    /// A foto está maior que a área? É quando Home, End e as páginas valem.
    pub fn foto_ampliada(&self) -> bool {
        self.vista()
            .is_some_and(|(cena, vista)| zoom::passa_da_area(&vista, &cena))
    }

    /// O nível do zoom, para o navegador e o controle do palco.
    pub fn nivel_do_zoom(&self) -> Nivel {
        self.navegacao.zoom.nivel
    }

    fn ponto_na_area(&self, posicao: Point<Pixels>) -> Ponto {
        Ponto {
            x: f(posicao.x - self.palco.origin.x),
            y: f(posicao.y - self.palco.origin.y),
        }
    }

    /// Vai para um nível, em torno de um ponto da área (ou do centro de agora).
    pub fn ir_para_nivel(&mut self, nivel: Nivel, ponto: Option<Ponto>, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista() else {
            self.navegacao.zoom.nivel = nivel;
            cx.notify();
            return;
        };
        let escala = zoom::escala_do_nivel(nivel, &cena);
        let centro = match ponto {
            Some(p) => zoom::centro_em_torno_de(&vista, escala, p, &cena),
            None => vista.centro,
        };
        self.navegacao.zoom = EstadoDoZoom { nivel, centro };
        cx.notify();
    }

    /// O `Z` e o clique: ampliada volta ao encaixe; encaixada vai ao último zoom.
    pub fn alternar_zoom(&mut self, ponto: Option<Ponto>, cx: &mut Context<Self>) {
        if self.edicao.is_some() {
            return;
        }
        let atual = self.navegacao.zoom.nivel;
        if atual != Nivel::Encaixar {
            self.navegacao.alvo_do_z = atual;
            self.ir_para_nivel(Nivel::Encaixar, None, cx);
        } else {
            let alvo = self.navegacao.alvo_do_z;
            self.ir_para_nivel(alvo, ponto, cx);
        }
    }

    /// `⌘=` (1) e `⌘−` (-1).
    pub fn passo_de_zoom(&mut self, direcao: i32, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista() else {
            return;
        };
        self.navegacao.zoom = EstadoDoZoom {
            nivel: zoom::proxima_parada(vista.escala, direcao, &cena),
            centro: vista.centro,
        };
        cx.notify();
    }

    /// Home, End, PgDn e PgUp.
    pub fn mover_o_centro(
        &mut self,
        fazer: impl FnOnce(&Vista, &Cena) -> Ponto,
        cx: &mut Context<Self>,
    ) {
        let Some((cena, vista)) = self.vista() else {
            return;
        };
        self.navegacao.zoom.centro = fazer(&vista, &cena);
        cx.notify();
    }

    pub fn zoom_ao_inicio(&mut self, cx: &mut Context<Self>) {
        self.mover_o_centro(|_, _| Ponto { x: 0., y: 0. }, cx);
    }

    pub fn zoom_ao_fim(&mut self, cx: &mut Context<Self>) {
        self.mover_o_centro(|_, _| Ponto { x: 1., y: 1. }, cx);
    }

    pub fn zoom_por_tela(&mut self, direcao: i32, cx: &mut Context<Self>) {
        self.mover_o_centro(|v, c| zoom::centro_paginado(v, direcao, c), cx);
    }

    fn ampliar_em_torno(&mut self, fator: f32, ponto: Ponto, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista() else {
            return;
        };
        let escala = zoom::limitar_escala(vista.escala * fator, &cena);
        self.navegacao.zoom = EstadoDoZoom {
            nivel: zoom::nivel_da_escala(escala, &cena),
            centro: zoom::centro_em_torno_de(&vista, escala, ponto, &cena),
        };
        cx.notify();
    }

    fn ao_rolar(&mut self, evento: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista() else {
            return;
        };
        let delta = evento.delta.pixel_delta(px(16.));
        let m = evento.modifiers;
        if m.platform || m.control || m.alt {
            let ponto = self.ponto_na_area(evento.position);
            self.ampliar_em_torno(zoom::fator_da_roda(-f(delta.y)), ponto, cx);
            return;
        }
        if !zoom::passa_da_area(&vista, &cena) {
            return;
        }
        // A roda do GPUI já vem no sentido do conteúdo: somar move a foto.
        self.navegacao.zoom.centro = zoom::centro_arrastado(&vista, f(delta.x), f(delta.y), &cena);
        cx.notify();
    }

    fn ao_apertar(&mut self, evento: &MouseDownEvent, _cx: &mut Context<Self>) {
        let Some((_, vista)) = self.vista() else {
            return;
        };
        let m = evento.modifiers;
        self.navegacao.gesto = Some(Gesto {
            tipo: if m.platform || m.control {
                TipoDeGesto::Caixa
            } else if m.alt {
                TipoDeGesto::Lupa
            } else {
                TipoDeGesto::Clique
            },
            inicio: self.ponto_na_area(evento.position),
            vista,
            moveu: false,
        });
    }

    fn ao_mover(&mut self, evento: &MouseMoveEvent, cx: &mut Context<Self>) {
        self.navegacao.ultimo_ponteiro = Some(self.ponto_na_area(evento.position));
        let Some(mut gesto) = self.navegacao.gesto else {
            return;
        };
        let Some(cena) = self.cena() else {
            return;
        };
        let p = self.ponto_na_area(evento.position);
        let (dx, dy) = (p.x - gesto.inicio.x, p.y - gesto.inicio.y);
        // Três pontos de tolerância: um clique com a mão tremendo continua clique.
        if !gesto.moveu && dx.hypot(dy) < 3. {
            return;
        }
        gesto.moveu = true;
        self.navegacao.gesto = Some(gesto);
        if let Some((_, usado)) = self.navegacao.espaco.as_mut() {
            *usado = true;
        }
        match gesto.tipo {
            TipoDeGesto::Caixa => self.navegacao.caixa = Some((gesto.inicio, p)),
            TipoDeGesto::Lupa => {
                let escala = zoom::limitar_escala(gesto.vista.escala * (dx * 0.01).exp(), &cena);
                self.navegacao.zoom = EstadoDoZoom {
                    nivel: zoom::nivel_da_escala(escala, &cena),
                    centro: zoom::centro_em_torno_de(&gesto.vista, escala, gesto.inicio, &cena),
                };
            }
            TipoDeGesto::Clique => {
                // 🔑 Da vista do começo do gesto: somar sobre a de agora
                // perderia movimento no arrasto rápido.
                if zoom::passa_da_area(&gesto.vista, &cena) {
                    self.navegacao.zoom.centro =
                        zoom::centro_arrastado(&gesto.vista, dx, dy, &cena);
                }
            }
        }
        cx.notify();
    }

    fn ao_soltar(&mut self, posicao: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(gesto) = self.navegacao.gesto.take() else {
            return;
        };
        let p = self.ponto_na_area(posicao);
        if gesto.tipo == TipoDeGesto::Caixa {
            self.navegacao.caixa = None;
            let alvo = self
                .cena()
                .and_then(|c| zoom::zoom_da_caixa(&gesto.vista, gesto.inicio, p, &c));
            match alvo {
                Some(alvo) => {
                    self.navegacao.zoom = alvo;
                    cx.notify();
                }
                None if !gesto.moveu => self.alternar_zoom(Some(p), cx),
                None => cx.notify(),
            }
            return;
        }
        // Com o Espaço segurado o clique é da mão, e não do zoom.
        if let Some((_, usado)) = self.navegacao.espaco.as_mut() {
            *usado = true;
            return;
        }
        if gesto.tipo == TipoDeGesto::Clique && !gesto.moveu {
            self.alternar_zoom(Some(p), cx);
        }
    }

    /// Os ouvintes do zoom na caixa do palco (fora do Enquadrar).
    pub(super) fn com_gestos_de_zoom(
        &self,
        caixa: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        if self.edicao.is_some() {
            return caixa;
        }
        let ampliada = self
            .vista()
            .is_some_and(|(c, v)| zoom::passa_da_area(&v, &c));
        let mao = self.navegacao.espaco.is_some();
        caixa
            .when(mao, |c| c.cursor(gpui::CursorStyle::ClosedHand))
            .when(!mao && ampliada, |c| c.cursor_grab())
            .when(!mao && !ampliada, |c| {
                c.cursor(gpui::CursorStyle::PointingHand)
            })
            .on_scroll_wheel(cx.listener(|tela, e: &ScrollWheelEvent, _w, cx| tela.ao_rolar(e, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|tela, e: &MouseDownEvent, _w, cx| tela.ao_apertar(e, cx)),
            )
            .on_mouse_move(cx.listener(|tela, e: &MouseMoveEvent, _w, cx| tela.ao_mover(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|tela, e: &MouseUpEvent, _w, cx| tela.ao_soltar(e.position, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|tela, e: &MouseUpEvent, _w, cx| tela.ao_soltar(e.position, cx)),
            )
    }

    /// `Z` desceu. Repetição do teclado não conta.
    pub fn z_apertado(&mut self, cx: &mut Context<Self>) {
        if self.edicao.is_some() || self.navegacao.z_desde.is_some() {
            return;
        }
        self.navegacao.z_desde = Some(Instant::now());
        let ponto = self.navegacao.ultimo_ponteiro;
        self.alternar_zoom(ponto, cx);
    }

    /// `Z` subiu: segurado mais de 400 ms era espiar, e volta.
    pub fn z_solto(&mut self, cx: &mut Context<Self>) {
        let Some(desde) = self.navegacao.z_desde.take() else {
            return;
        };
        if desde.elapsed() > Duration::from_millis(400) {
            self.alternar_zoom(None, cx);
        }
    }

    /// Espaço desceu: a mão, enquanto segurado.
    pub fn espaco_apertado(&mut self, cx: &mut Context<Self>) {
        if self.edicao.is_some() || self.navegacao.espaco.is_some() {
            return;
        }
        self.navegacao.espaco = Some((Instant::now(), false));
        cx.notify();
    }

    /// Espaço subiu: tocado sem arrastar, alterna o zoom como no Lightroom.
    pub fn espaco_solto(&mut self, cx: &mut Context<Self>) {
        let Some((desde, usado)) = self.navegacao.espaco.take() else {
            return;
        };
        if !usado && self.edicao.is_none() && desde.elapsed() < Duration::from_millis(500) {
            let ponto = self.navegacao.ultimo_ponteiro;
            self.alternar_zoom(ponto, cx);
        }
        cx.notify();
    }

    /// A janela perdeu o foco: o `keyup` não vai chegar.
    pub fn soltar_as_teclas(&mut self, cx: &mut Context<Self>) {
        self.navegacao.z_desde = None;
        if self.navegacao.espaco.take().is_some() {
            cx.notify();
        }
    }

    /// O que a tela do cliente mostra: **o mesmo que o palco** — o "Antes"
    /// segurado e a predefinição sob o mouse também, como no site
    /// (`anunciarAoCliente`).
    pub fn receita_para_o_cliente(
        &self,
    ) -> (
        crate::revelacao::processador::Ajustes,
        domain::value_objects::CropSettings,
    ) {
        let ajustes = if self.mostrando_original() {
            crate::revelacao::processador::Ajustes::default()
        } else {
            self.ajustes_na_tela()
        };
        (ajustes, self.enquadramento())
    }

    /// `\\` segurado mostra a foto sem ajuste; solto, volta.
    pub fn ver_o_antes(&mut self, ver: bool, cx: &mut Context<Self>) {
        if self.mostrando_original() != ver {
            self.alternar_original(cx);
        }
    }

    pub fn alternar_ajuda(&mut self, cx: &mut Context<Self>) {
        self.navegacao.ajuda = !self.navegacao.ajuda;
        cx.notify();
    }

    pub fn ajuda_aberta(&self) -> bool {
        self.navegacao.ajuda
    }

    /// A folha de atalhos, por cima da foto — `folha-de-atalhos.tsx`.
    pub(super) fn folha_de_atalhos(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.navegacao.ajuda {
            return None;
        }
        let tema = cx.theme();
        let (mudo, frente, borda, cartao, fundo_da_tecla) = (
            tema.muted_foreground,
            tema.foreground,
            tema.border,
            tema.popover,
            tema.muted,
        );
        let tecla = move |texto: &'static str| {
            div()
                .flex_none()
                .rounded(px(4.))
                .border_1()
                .border_color(borda)
                .bg(fundo_da_tecla)
                .px(px(6.))
                .py(px(2.))
                .font_family("Menlo")
                .text_size(px(11.))
                .text_color(frente)
                .child(texto)
        };
        let titulo = move |texto: &'static str| {
            div()
                .mb(px(6.))
                .text_size(px(11.))
                .text_color(mudo)
                .child(texto.to_uppercase())
        };
        let grupos = ATALHOS.iter().map(|(nome, linhas)| {
            v_flex()
                .min_w(px(220.))
                .child(titulo(nome))
                .child(
                    v_flex()
                        .gap(px(4.))
                        .children(linhas.iter().map(|(teclas, faz)| {
                            h_flex()
                                .gap(px(8.))
                                .items_baseline()
                                .text_xs()
                                .child(tecla(teclas))
                                .child(div().text_color(mudo).child(*faz))
                        })),
                )
        });
        let gestos = GESTOS_DO_ZOOM.iter().map(|(gesto, faz)| {
            h_flex()
                .w(px(340.))
                .gap(px(8.))
                .items_baseline()
                .text_xs()
                .child(div().flex_none().text_color(frente).child(*gesto))
                .child(div().text_color(mudo).child(*faz))
        });
        Some(
            div()
                .id("folha-de-atalhos")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .bg(gpui::rgba(0x000000b3))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|tela, _, _, cx| {
                        cx.stop_propagation();
                        tela.alternar_ajuda(cx);
                    }),
                )
                .child(
                    v_flex()
                        .max_h_full()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(borda)
                        .bg(cartao)
                        .p(px(20.))
                        .shadow_2xl()
                        .child(
                            div()
                                .mb(px(12.))
                                .text_sm()
                                .text_color(frente)
                                .child("Atalhos do teclado"),
                        )
                        .child(
                            h_flex()
                                .flex_wrap()
                                .items_start()
                                .gap_x(px(32.))
                                .gap_y(px(16.))
                                .children(grupos),
                        )
                        .child(div().mt(px(16.)).child(titulo("Zoom com o mouse")))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_x(px(32.))
                                .gap_y(px(4.))
                                .children(gestos),
                        )
                        .child(
                            div()
                                .mt(px(16.))
                                .text_size(px(11.))
                                .text_color(mudo)
                                .child("Clique em qualquer lugar, ou aperte ?, para fechar."),
                        ),
                )
                .into_any_element(),
        )
    }

    /// A foto do palco na vista do zoom (ou encaixada, fora dele).
    pub(super) fn foto_na_vista(&self, imagem: Arc<RenderImage>) -> gpui::Img {
        match self.vista() {
            Some((cena, vista)) => img(imagem)
                .absolute()
                .left(px(vista.x))
                .top(px(vista.y))
                .w(px(cena.janela.largura * vista.escala))
                .h(px(cena.janela.altura * vista.escala))
                .object_fit(ObjectFit::Fill),
            // 🚨 **A conta é nossa, e não o `Contain` do GPUI**: numa caixa
            // absoluta ele preenchia a área como `cover`, e no Enquadrar a foto
            // aparecia ampliada, com o retângulo fora do lugar.
            None => {
                let tamanho = imagem.size(0);
                let (x, y, w, h) = crate::revelacao::corte::area_da_foto(
                    (f(self.palco.size.width), f(self.palco.size.height)),
                    (tamanho.width.0 as f32, tamanho.height.0 as f32),
                );
                img(imagem)
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .object_fit(ObjectFit::Fill)
            }
        }
    }

    /// A caixa do `⌘ + arrastar`, desenhada por cima da foto.
    pub(super) fn caixa_de_zoom(&self) -> Option<AnyElement> {
        let (a, b) = self.navegacao.caixa?;
        Some(
            div()
                .absolute()
                .left(px(a.x.min(b.x)))
                .top(px(a.y.min(b.y)))
                .w(px((b.x - a.x).abs()))
                .h(px((b.y - a.y).abs()))
                .border_1()
                .border_color(gpui::white())
                .bg(gpui::rgba(0xffffff1a))
                .into_any_element(),
        )
    }

    /// "− Encaixar +", no canto de baixo do palco.
    pub(super) fn controle_de_zoom(&self, cx: &mut Context<Self>) -> AnyElement {
        let desligado = self.cena().is_none();
        let botao = |id: &'static str, icone: Icone| {
            div()
                .id(id)
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .text_color(gpui::rgb(0xd4d4d4))
                .when(!desligado, |b| {
                    b.cursor_pointer().hover(|s| s.bg(gpui::rgba(0xffffff26)))
                })
                .child(Icon::new(icone).size(px(14.)))
        };
        h_flex()
            .absolute()
            .left(px(12.))
            .bottom(px(12.))
            .gap(px(4.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(gpui::rgba(0x000000a6))
            .text_xs()
            .text_color(gpui::white())
            .when(desligado, |d| d.opacity(0.5))
            .child(
                botao("zoom-afastar", Icone::Minus)
                    .on_click(cx.listener(|tela, _, _, cx| tela.passo_de_zoom(-1, cx))),
            )
            .child(
                div()
                    .id("zoom-rotulo")
                    .min_w(px(64.))
                    .flex()
                    .justify_center()
                    .cursor_pointer()
                    .child(zoom::rotulo_do_nivel(self.navegacao.zoom.nivel))
                    .on_click(cx.listener(|tela, _, _, cx| tela.alternar_zoom(None, cx))),
            )
            .child(
                botao("zoom-aproximar", Icone::Plus)
                    .on_click(cx.listener(|tela, _, _, cx| tela.passo_de_zoom(1, cx))),
            )
            .when(self.carregando_o_bruto(), |c| {
                c.child(
                    h_flex()
                        .gap(px(6.))
                        .px(px(6.))
                        .text_color(gpui::rgba(0xffffffcc))
                        .child(gpui_component::spinner::Spinner::new().xsmall())
                        .child("resolução cheia…"),
                )
            })
            .into_any_element()
    }

    fn centralizar_pela_miniatura(&mut self, posicao: Point<Pixels>, cx: &mut Context<Self>) {
        let caixa = self.navegacao.miniatura;
        if f(caixa.size.width) < 1. || f(caixa.size.height) < 1. {
            return;
        }
        self.navegacao.zoom.centro = Ponto {
            x: (f(posicao.x - caixa.origin.x) / f(caixa.size.width)).clamp(0., 1.),
            y: (f(posicao.y - caixa.origin.y) / f(caixa.size.height)).clamp(0., 1.),
        };
        cx.notify();
    }

    /// O Navegador do site: o nível, os atalhos de zoom e a miniatura com o
    /// retângulo do que está na tela, arrastável.
    pub(super) fn navegador(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (apagado, borda, fundo) = (tema.muted_foreground, tema.border, tema.muted);
        let desligado = self.edicao.is_some() || self.aberta.is_none();
        let nivel = self.navegacao.zoom.nivel;
        let ambar = crate::tema::cores::quente();

        let botao = |id: &'static str, alvo: Nivel, texto: SharedString, cx: &mut Context<Self>| {
            let aceso = nivel == alvo;
            div()
                .id(id)
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .text_size(px(11.))
                .when(aceso, |b| b.bg(ambar).text_color(gpui::black()))
                .when(!aceso, |b| b.text_color(apagado))
                .when(desligado, |b| b.opacity(0.4))
                .when(!desligado, |b| {
                    b.cursor_pointer().on_click(
                        cx.listener(move |tela, _, _, cx| tela.ir_para_nivel(alvo, None, cx)),
                    )
                })
                .child(texto)
        };

        // "Outras": as razões que não têm botão, uma atrás da outra.
        let outra = match nivel {
            Nivel::Razao(r) if r != 1. => Some(r),
            _ => None,
        };
        let proxima_outra = {
            let lista: Vec<f32> = zoom::RAZOES.iter().copied().filter(|r| *r != 1.).collect();
            let i = outra
                .and_then(|r| lista.iter().position(|x| (x - r).abs() < 1e-3))
                .map(|i| (i + 1) % lista.len())
                .unwrap_or(lista.iter().position(|x| *x == 2.).unwrap_or(0));
            lista[i]
        };

        let (miniatura, retangulo) = match self.aberta.as_ref().and_then(|a| a.desenhada.clone()) {
            Some(imagem) => {
                let tamanho = imagem.size(0);
                let (w, h) = (tamanho.width.0 as f32, tamanho.height.0 as f32);
                let escala = (LARGURA_DO_NAVEGADOR / w).min(ALTURA_DO_NAVEGADOR / h);
                let retangulo = self.vista().and_then(|(c, v)| {
                    zoom::passa_da_area(&v, &c).then(|| zoom::retangulo_visivel(&v, &c))
                });
                (Some((imagem, w * escala, h * escala)), retangulo)
            }
            None => (None, None),
        };
        let medidor = cx.entity();

        v_flex()
            .pb(px(12.))
            .mb(px(12.))
            .border_b_1()
            .border_color(borda)
            .child(
                h_flex()
                    .mb(px(4.))
                    .text_size(px(11.))
                    .text_color(apagado)
                    .child("NAVEGADOR")
                    .child(
                        div()
                            .ml_auto()
                            .text_color(tema.foreground)
                            .child(zoom::rotulo_do_nivel(nivel)),
                    ),
            )
            .child(
                h_flex()
                    .mb(px(6.))
                    .justify_between()
                    .child(botao(
                        "nav-encaixar",
                        Nivel::Encaixar,
                        "Encaixar".into(),
                        cx,
                    ))
                    .child(botao(
                        "nav-preencher",
                        Nivel::Preencher,
                        "Preencher".into(),
                        cx,
                    ))
                    .child(botao("nav-1-1", Nivel::Razao(1.), "1:1".into(), cx))
                    .child(
                        botao(
                            "nav-outra",
                            outra.map(Nivel::Razao).unwrap_or(Nivel::Razao(-1.)),
                            match outra {
                                Some(r) => zoom::rotulo_do_nivel(Nivel::Razao(r)),
                                None => "Outra".into(),
                            }
                            .into(),
                            cx,
                        )
                        .when(!desligado, |b| {
                            b.on_click(cx.listener(move |tela, _, _, cx| {
                                tela.ir_para_nivel(Nivel::Razao(proxima_outra), None, cx)
                            }))
                        }),
                    ),
            )
            // 🚨 **A área do navegador tem altura fixa.** A miniatura acompanha a
            // proporção da foto (como no site), mas o **bloco** não: sem isto,
            // trocar uma paisagem por um retrato mudava a altura daqui e a lista
            // de predefinições subia e descia a cada foto — *"o tamanho dessa
            // janela fica mudando de forma bizarra"* (dono, 18/set/2026).
            //
            // 🔑 `ALTURA_DO_NAVEGADOR` é o teto que a escala já usa: fixando o
            // bloco nele, a foto mais alta encosta nas bordas e a mais larga
            // ganha respiro em cima e embaixo — e nada se mexe em volta.
            .child(
                h_flex()
                    .h(px(ALTURA_DO_NAVEGADOR))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .child(match miniatura {
                        Some((imagem, w, h)) => div()
                            .id("navegador-miniatura")
                            .relative()
                            .w(px(w))
                            .h(px(h))
                            .overflow_hidden()
                            .rounded(px(2.))
                            .bg(fundo)
                            .when(retangulo.is_some() && !desligado, |d| {
                                d.cursor_move()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|tela, e: &MouseDownEvent, _, cx| {
                                            tela.navegacao.arrastando_miniatura = true;
                                            tela.centralizar_pela_miniatura(e.position, cx);
                                        }),
                                    )
                                    .on_mouse_move(cx.listener(
                                        |tela, e: &MouseMoveEvent, _, cx| {
                                            if tela.navegacao.arrastando_miniatura && e.dragging() {
                                                tela.centralizar_pela_miniatura(e.position, cx);
                                            }
                                        },
                                    ))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|tela, _: &MouseUpEvent, _, _| {
                                            tela.navegacao.arrastando_miniatura = false;
                                        }),
                                    )
                            })
                            .child(img(imagem).size_full().object_fit(ObjectFit::Fill))
                            .children(retangulo.map(|r| {
                                // O que está fora da tela escurece, como no site.
                                div()
                                    .absolute()
                                    .left(px(r.x * w))
                                    .top(px(r.y * h))
                                    .w(px(r.w * w))
                                    .h(px(r.h * h))
                                    .border_1()
                                    .border_color(gpui::white())
                            }))
                            .child(
                                canvas(
                                    move |bounds, _, cx| {
                                        medidor.update(cx, |tela, _| {
                                            if tela.navegacao.miniatura != bounds {
                                                tela.navegacao.miniatura = bounds;
                                            }
                                        });
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .inset_0(),
                            )
                            .into_any_element(),
                        // Sem foto aberta ainda: o mesmo retângulo, para a coluna
                        // já nascer com a altura que vai ter.
                        None => div()
                            .w(px(LARGURA_DO_NAVEGADOR))
                            .h(px(ALTURA_DO_NAVEGADOR))
                            .rounded(px(2.))
                            .bg(fundo)
                            .into_any_element(),
                    }),
            )
            .into_any_element()
    }
}
