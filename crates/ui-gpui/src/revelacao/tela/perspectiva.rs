//! Perspectiva guiada — o *Upright guiado* do Lightroom, dentro do Enquadrar.
//!
//! O operador traça sobre a foto retas que deviam ser verticais ou horizontais
//! (um batente, uma quina, o rodapé), e o motor
//! ([`revelacao_core::perspectiva`]) acha a menor correção de câmera que as
//! endireita. Duas guias bastam; cabem quatro.
//!
//! | Gesto | Faz |
//! |---|---|
//! | botão **Guias** | liga o traçado; o retângulo de corte fica parado |
//! | arrastar na foto | traça uma guia; o eixo sai da direção do traço |
//! | arrastar a ponta de uma guia | move a ponta |
//! | clicar numa guia | seleciona; `Delete` apaga |
//! | eixo no painel | troca Vertical ↔ Horizontal |
//! | sliders Vertical / Horizontal | ajuste fino à mão, por cima das guias |
//! | Restringir ao conteúdo | o retângulo não passa dos cantos vazios |
//! | Redefinir perspectiva | tira guias e correção |
//!
//! ## 🔑 A foto fica parada enquanto a guia anda
//!
//! Durante o traço e o arrasto de uma ponta, a guia segue o ponteiro e o
//! painel mostra a correção que ela daria — mas a **foto não é reamostrada a
//! cada movimento**. Ela é refeita uma vez, ao soltar (~8 ms na cópia de
//! trabalho, `medir-perspectiva`). Não é só economia: com a foto se corrigindo
//! debaixo do ponteiro, o ponto que a mão segura andaria junto com ela, e a
//! guia fugiria do batente. É o que o Lightroom faz — a foto se transforma
//! quando a guia é solta. Os sliders, que não dependem de onde o ponteiro
//! está, movem a foto a cada quadro.
//!
//! ## Onde as guias moram
//!
//! Na **foto de pé**, normalizadas (o espaço das máscaras), com o eixo nos
//! eixos dela: girar ou espelhar depois leva as guias e a correção junto. A
//! tela converte com o mesmo [`revelacao_core::Corte::mapa`] que produz o
//! arquivo, e mostra o eixo trocado quando o giro é ímpar.

use domain::value_objects::perspectiva::MAXIMO_DE_GUIAS;
use domain::value_objects::{CropSettings, EixoDaGuia, GuiaDePerspectiva, PerspectivaGuiada};
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, point, prelude::*, px, AnyElement, Context, CursorStyle, Hsla, MouseButton,
    MouseDownEvent, PathBuilder, Pixels, Point, SharedString, Window,
};
use revelacao_core::nalgebra::Vector3;
use revelacao_core::perspectiva::{self as motor, AJUSTE_MAXIMO};

use super::Revelacao;
use crate::estilo;
use crate::recursos::Icone;
use crate::revelacao::corte;

/// O traço mais curto que vira guia, em pontos da tela.
const MENOR_TRACO: f32 = 12.;
/// O raio da bolinha da ponta.
const RAIO_DA_PONTA: f32 = 6.;
/// Quão perto (em pontos) um clique precisa estar da guia para selecioná-la.
const PERTO_DA_GUIA: f32 = 7.;
/// O passo dos sliders de ajuste fino, em graus.
pub(super) const PASSO_DO_AJUSTE: f32 = 0.1;

const COR_VERTICAL: u32 = 0x38bdf8;
const COR_HORIZONTAL: u32 = 0xf472b6;
const AMBAR: u32 = 0xfbbf24;

/// Uma guia como a tela a desenha: índice, as duas pontas no palco e o eixo
/// na tela.
type GuiaNoPalco = (usize, (f32, f32), (f32, f32), EixoDaGuia);
/// Um traço a pintar: de, até, cor e se é o destacado.
type Traco = ((f32, f32), (f32, f32), Hsla, bool);

/// Qual das duas pontas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ponta {
    De,
    Ate,
}

/// O estado da perspectiva guiada enquanto o Enquadrar está aberto.
#[derive(Debug, Default)]
pub(super) struct EdicaoDasGuias {
    /// O traçado está ligado: arrastar na foto traça guia.
    pub armadas: bool,
    /// A guia nova em curso, em pontos da janela: `(de, até)`.
    pub tracando: Option<(Point<Pixels>, Point<Pixels>)>,
    /// A ponta em arrasto.
    pub arrasto: Option<(usize, Ponta)>,
    /// As guias como estão **durante** o gesto. A foto (e o corte) só mudam
    /// ao soltar — ver o cabeçalho.
    pub rascunho: Option<[Option<GuiaDePerspectiva>; MAXIMO_DE_GUIAS]>,
    pub selecionada: Option<usize>,
    /// O que a conta tem a dizer: "trace mais uma", "grande demais"…
    pub aviso: Option<SharedString>,
}

impl EdicaoDasGuias {
    pub fn em_gesto(&self) -> bool {
        self.tracando.is_some() || self.arrasto.is_some()
    }
}

fn para_o_motor(g: &GuiaDePerspectiva) -> motor::Guia {
    motor::Guia {
        de: g.de.map(f64::from),
        ate: g.ate.map(f64::from),
        eixo: match g.eixo {
            EixoDaGuia::Vertical => motor::Eixo::Vertical,
            EixoDaGuia::Horizontal => motor::Eixo::Horizontal,
        },
    }
}

fn outro(eixo: EixoDaGuia) -> EixoDaGuia {
    match eixo {
        EixoDaGuia::Vertical => EixoDaGuia::Horizontal,
        EixoDaGuia::Horizontal => EixoDaGuia::Vertical,
    }
}

/// O que a resposta do motor quer dizer para o operador — `None` é "tudo certo".
fn aviso_da_resposta(r: &motor::Resposta, quantas: usize) -> Option<SharedString> {
    let motivo = |m: motor::GuiaIgnorada| match m {
        motor::GuiaIgnorada::Curta => "curta demais para dizer a direção",
        motor::GuiaIgnorada::Repetida => "repete outra guia do mesmo eixo",
        motor::GuiaIgnorada::Inclinada => "está longe demais do eixo — troque Vertical/Horizontal",
    };
    let fora = r
        .ignoradas
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.map(|m| format!("a guia {} {}", i + 1, motivo(m))))
        .collect::<Vec<_>>();
    let principal = match &r.resolucao {
        motor::Resolucao::PoucasGuias { .. } if quantas == 0 => None,
        motor::Resolucao::PoucasGuias { .. } => {
            Some("Trace mais uma guia: a correção começa com duas.".to_string())
        }
        motor::Resolucao::Excessiva => Some(
            "Essas guias pedem uma correção grande demais, e a foto ficou como estava. Confira o eixo de cada uma."
                .to_string(),
        ),
        motor::Resolucao::Resolvida(s) if s.desvio_maximo > 1.0 => Some(format!(
            "As guias não concordam entre si: sobra {:.1}° numa delas.",
            s.desvio_maximo
        )),
        motor::Resolucao::Resolvida(_) => None,
    };
    let mut partes: Vec<String> = principal.into_iter().collect();
    if !fora.is_empty() {
        let mut t = fora.join("; ");
        if let Some(c) = t.get(0..1) {
            t = c.to_uppercase() + &t[1..];
        }
        partes.push(format!("{t}."));
    }
    (!partes.is_empty()).then(|| partes.join(" ").into())
}

impl Revelacao {
    fn guias(&self) -> Option<&EdicaoDasGuias> {
        self.edicao.as_ref().map(|e| &e.guias)
    }

    fn guias_mut(&mut self) -> Option<&mut EdicaoDasGuias> {
        self.edicao.as_mut().map(|e| &mut e.guias)
    }

    /// As guias na tela agora: o rascunho do gesto, ou as da foto.
    fn guias_vigentes(&self) -> [Option<GuiaDePerspectiva>; MAXIMO_DE_GUIAS] {
        self.guias()
            .and_then(|g| g.rascunho)
            .unwrap_or(self.corte_atual().perspectiva().guias)
    }

    /// O traçado está ligado?
    pub fn tracando_guias(&self) -> bool {
        self.guias().is_some_and(|g| g.armadas)
    }

    /// Liga ou desliga o traçado de guias. Desarma a régua: as duas querem o
    /// mesmo arrasto.
    pub fn alternar_guias(&mut self, cx: &mut Context<Self>) {
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        edicao.guias.armadas = !edicao.guias.armadas;
        edicao.guias.tracando = None;
        edicao.guias.arrasto = None;
        edicao.guias.rascunho = None;
        edicao.guias.selecionada = None;
        edicao.regua_armada = false;
        edicao.regua = None;
        cx.notify();
    }

    /// As dimensões da cópia de trabalho, e o corte do motor de agora.
    fn geometria(&self) -> Option<(u32, u32, revelacao_core::Corte)> {
        let (l, a) = self.tamanho_da_foto()?;
        let corte = infrastructure::transformacao::corte(&self.corte_atual());
        Some((l.round().max(1.) as u32, a.round().max(1.) as u32, corte))
    }

    /// Do ponto da janela à foto de pé, normalizada — pela mesma matriz que
    /// produz o arquivo.
    pub(super) fn ponto_na_foto(&self, janela: Point<Pixels>) -> Option<[f32; 2]> {
        let (l, a, corte) = self.geometria()?;
        let (ax, ay, aw, ah) = self.area_da_foto()?;
        let local = janela - self.palco.origin;
        let e = (
            (f32::from(local.x) - ax) / aw,
            (f32::from(local.y) - ay) / ah,
        );
        let v = corte.mapa(l, a, false) * Vector3::new(e.0 as f64, e.1 as f64, 1.0);
        (v.z.abs() > 1e-12).then(|| [(v.x / v.z) as f32, (v.y / v.z) as f32])
    }

    /// O caminho inverso, em pontos **dentro do palco**.
    pub(super) fn ponto_no_palco(&self, foto: [f32; 2]) -> Option<(f32, f32)> {
        let (l, a, corte) = self.geometria()?;
        let (ax, ay, aw, ah) = self.area_da_foto()?;
        let ida = corte.mapa(l, a, false).try_inverse()?;
        let v = ida * Vector3::new(foto[0] as f64, foto[1] as f64, 1.0);
        (v.z > 1e-12).then(|| (ax + (v.x / v.z) as f32 * aw, ay + (v.y / v.z) as f32 * ah))
    }

    /// O eixo como a tela o mostra (troca com o giro ímpar).
    pub(super) fn eixo_na_tela(&self, eixo: EixoDaGuia) -> EixoDaGuia {
        if self.corte_atual().rotation_90().rem_euclid(4) % 2 == 1 {
            outro(eixo)
        } else {
            eixo
        }
    }

    /// Resolve as guias e devolve a perspectiva que elas dão (o ajuste manual
    /// fica), com o aviso para o painel.
    fn resolver_guias(
        &self,
        guias: [Option<GuiaDePerspectiva>; MAXIMO_DE_GUIAS],
    ) -> (PerspectivaGuiada, Option<SharedString>) {
        let atual = *self.corte_atual().perspectiva();
        let mut nova = PerspectivaGuiada {
            guias,
            rotacao: [0.0; 3],
            foco: domain::value_objects::perspectiva::FOCO_PADRAO,
            ..atual
        };
        let Some((l, a)) = self.tamanho_da_foto() else {
            return (nova, None);
        };
        let lista: Vec<motor::Guia> = guias.iter().flatten().map(para_o_motor).collect();
        let resposta = motor::resolver(&lista, l.round() as u32, a.round() as u32);
        if let motor::Resolucao::Resolvida(s) = &resposta.resolucao {
            let p = s.perspectiva(atual.vertical, atual.horizontal);
            nova.rotacao = p.rotacao;
            nova.foco = p.foco;
        }
        (nova, aviso_da_resposta(&resposta, lista.len()))
    }

    /// Grava uma perspectiva nova como **um** gesto: fecha o anterior, traz o
    /// retângulo para dentro da foto corrigida e vira um passo do histórico.
    ///
    /// 🔑 **Quando a correção liga, o endireitar volta a zero** — como no
    /// Lightroom, onde o Upright redefine o ângulo do corte: as guias já
    /// endireitam a foto, e um ângulo antigo por cima entortaria de novo o que
    /// elas acabaram de acertar. Depois disso o slider continua valendo, por
    /// cima da correção.
    fn gravar_perspectiva(
        &mut self,
        nova: PerspectivaGuiada,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(espaco) = self.espaco() else {
            return;
        };
        self.gravar_o_que_estiver_pendente();
        let antes = self.corte_atual();
        let desejado = self
            .edicao
            .as_ref()
            .and_then(|e| e.desejado)
            .unwrap_or_else(|| corte::retangulo_de(&antes, espaco));
        let base = if !antes.perspectiva().corrige() && nova.corrige() && antes.angle() != 0. {
            self.angulo
                .update(cx, |estado, cx| estado.set_value(0., window, cx));
            corte::endireitar(&antes, 0., espaco, Some(desejado))
        } else {
            antes
        };
        let novo = corte::com_perspectiva(&base, nova, espaco, Some(desejado));
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = Some(desejado);
        }
        self.aplicar_corte_de_perspectiva(novo, cx);
        self.gravar_o_que_estiver_pendente();
        self.sincronizar_sliders_da_perspectiva(window, cx);
    }

    /// Troca o corte sem fechar o gesto (o slider fecha pela espera).
    fn aplicar_corte_de_perspectiva(&mut self, novo: CropSettings, cx: &mut Context<Self>) {
        if novo == self.corte_atual() {
            return;
        }
        self.corte = super::enquadrar::corte_de(&novo);
        self.pendente = true;
        self.exibicao_atrasada = true;
        self.revelar_de_novo_se_a_vinheta_segue_o_corte(cx);
        cx.notify();
    }

    /// Os dois sliders vão para o valor da foto — ao abrir, desfazer, refazer.
    pub(super) fn sincronizar_sliders_da_perspectiva(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p = *self.corte_atual().perspectiva();
        self.persp_vertical
            .update(cx, |estado, cx| estado.set_value(p.vertical, window, cx));
        self.persp_horizontal
            .update(cx, |estado, cx| estado.set_value(p.horizontal, window, cx));
    }

    /// Um dos sliders andou: ajuste fino por cima das guias. A foto acompanha
    /// a cada quadro, e a espera fecha o gesto — como o do endireitar.
    pub(super) fn ajuste_do_slider(&mut self, vertical: bool, valor: f32, cx: &mut Context<Self>) {
        if self.edicao.is_none() {
            return;
        }
        let Some(espaco) = self.espaco() else {
            return;
        };
        let valor = ((valor / PASSO_DO_AJUSTE).round() * PASSO_DO_AJUSTE)
            .clamp(-AJUSTE_MAXIMO, AJUSTE_MAXIMO);
        let atual = self.corte_atual();
        let mut p = *atual.perspectiva();
        let campo = if vertical {
            &mut p.vertical
        } else {
            &mut p.horizontal
        };
        if (*campo - valor).abs() < 1e-4 {
            return;
        }
        *campo = if valor == 0. { 0. } else { valor };
        let desejado = self
            .edicao
            .as_ref()
            .and_then(|e| e.desejado)
            .unwrap_or_else(|| corte::retangulo_de(&atual, espaco));
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = Some(desejado);
        }
        let novo = corte::com_perspectiva(&atual, p, espaco, Some(desejado));
        self.aplicar_corte_de_perspectiva(novo, cx);
        self.adiar_gravacao(cx);
    }

    /// Duplo clique no rótulo de um slider: aquele ajuste volta a zero.
    fn zerar_ajuste(&mut self, vertical: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut p = *self.corte_atual().perspectiva();
        if vertical {
            p.vertical = 0.;
        } else {
            p.horizontal = 0.;
        }
        self.gravar_perspectiva(p, window, cx);
    }

    /// Liga ou desliga "restringir ao conteúdo" — um gesto, um passo.
    pub fn alternar_restringir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(espaco) = self.espaco() else {
            return;
        };
        self.gravar_o_que_estiver_pendente();
        let atual = self.corte_atual();
        let desejado = self
            .edicao
            .as_ref()
            .and_then(|e| e.desejado)
            .unwrap_or_else(|| corte::retangulo_de(&atual, espaco));
        let novo = corte::com_restringir(&atual, !atual.restringir(), espaco, Some(desejado));
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = Some(desejado);
        }
        self.aplicar_corte_de_perspectiva(novo, cx);
        self.gravar_o_que_estiver_pendente();
        let _ = window;
    }

    /// Tira guias e correção: a foto volta a como era antes da perspectiva.
    pub fn redefinir_perspectiva(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(g) = self.guias_mut() {
            g.selecionada = None;
            g.aviso = None;
            g.rascunho = None;
        }
        self.gravar_perspectiva(PerspectivaGuiada::default(), window, cx);
    }

    /// Troca o eixo de uma guia e resolve de novo.
    pub(super) fn trocar_eixo(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut guias = self.corte_atual().perspectiva().guias;
        let Some(Some(g)) = guias.get_mut(indice) else {
            return;
        };
        g.eixo = outro(g.eixo);
        self.definir_guias(guias, window, cx);
    }

    pub(super) fn apagar_guia(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut guias = self.corte_atual().perspectiva().guias;
        if indice >= MAXIMO_DE_GUIAS || guias[indice].is_none() {
            return;
        }
        guias[indice] = None;
        if let Some(g) = self.guias_mut() {
            g.selecionada = None;
        }
        self.definir_guias(guias, window, cx);
    }

    /// `Delete` com uma guia selecionada. Devolve se havia o que apagar — sem
    /// guia selecionada, a tecla segue para o retoque.
    pub fn apagar_guia_selecionada(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(indice) = self
            .guias()
            .filter(|g| g.armadas)
            .and_then(|g| g.selecionada)
        else {
            return false;
        };
        self.apagar_guia(indice, window, cx);
        true
    }

    /// As guias novas viram a correção da foto.
    fn definir_guias(
        &mut self,
        guias: [Option<GuiaDePerspectiva>; MAXIMO_DE_GUIAS],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (nova, aviso) = self.resolver_guias(guias);
        if let Some(g) = self.guias_mut() {
            g.aviso = aviso;
        }
        self.gravar_perspectiva(nova, window, cx);
    }

    /// Onde está a guia mais perto do ponto (pontos da janela), se perto o
    /// bastante para ser um clique nela.
    fn guia_sob(&self, janela: Point<Pixels>) -> Option<usize> {
        let local = janela - self.palco.origin;
        let (px_, py_) = (f32::from(local.x), f32::from(local.y));
        let mut melhor: Option<(usize, f32)> = None;
        for (i, g) in self.guias_vigentes().iter().enumerate() {
            let Some(g) = g else { continue };
            let (Some(a), Some(b)) = (self.ponto_no_palco(g.de), self.ponto_no_palco(g.ate)) else {
                continue;
            };
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let l2 = (ex * ex + ey * ey).max(1e-6);
            let t = (((px_ - a.0) * ex + (py_ - a.1) * ey) / l2).clamp(0., 1.);
            let d = (px_ - (a.0 + t * ex)).hypot(py_ - (a.1 + t * ey));
            if d <= PERTO_DA_GUIA && melhor.is_none_or(|(_, m)| d < m) {
                melhor = Some((i, d));
            }
        }
        melhor.map(|(i, _)| i)
    }

    /// O botão desceu na camada das guias: seleciona uma guia, ou começa uma.
    pub(super) fn comecar_guia(&mut self, janela: Point<Pixels>, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();
        if let Some(i) = self.guia_sob(janela) {
            if let Some(g) = self.guias_mut() {
                g.selecionada = Some(i);
            }
            cx.notify();
            return;
        }
        let quantas = self.guias_vigentes().iter().flatten().count();
        let Some(g) = self.guias_mut() else {
            return;
        };
        g.selecionada = None;
        if quantas >= MAXIMO_DE_GUIAS {
            g.aviso = Some(
                "Já há quatro guias. Apague uma (selecione e tecle Delete) para traçar outra."
                    .into(),
            );
        } else {
            g.tracando = Some((janela, janela));
        }
        cx.notify();
    }

    pub(super) fn comecar_arrasto_da_ponta(
        &mut self,
        indice: usize,
        ponta: Ponta,
        cx: &mut Context<Self>,
    ) {
        self.gravar_o_que_estiver_pendente();
        let guias = self.corte_atual().perspectiva().guias;
        if let Some(g) = self.guias_mut() {
            g.arrasto = Some((indice, ponta));
            g.rascunho = Some(guias);
            g.selecionada = Some(indice);
        }
        cx.notify();
    }

    /// O ponteiro andou num gesto de guia. Devolve se o gesto era de guia.
    ///
    /// Só a guia e o aviso andam: a foto fica como está até soltar.
    pub(super) fn mover_guia(&mut self, ponteiro: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(g) = self.guias() else {
            return false;
        };
        if g.tracando.is_some() {
            if let Some((_, ate)) = self.guias_mut().and_then(|g| g.tracando.as_mut()) {
                *ate = ponteiro;
            }
            cx.notify();
            return true;
        }
        let Some((indice, ponta)) = g.arrasto else {
            return false;
        };
        let Some(foto) = self.ponto_na_foto(ponteiro) else {
            return true;
        };
        let foto = foto.map(|v| v.clamp(-0.25, 1.25));
        let mut rascunho = self
            .guias()
            .and_then(|g| g.rascunho)
            .unwrap_or(self.corte_atual().perspectiva().guias);
        if let Some(Some(guia)) = rascunho.get_mut(indice) {
            match ponta {
                Ponta::De => guia.de = foto,
                Ponta::Ate => guia.ate = foto,
            }
        }
        let (_, aviso) = self.resolver_guias(rascunho);
        if let Some(g) = self.guias_mut() {
            g.rascunho = Some(rascunho);
            g.aviso = aviso;
        }
        cx.notify();
        true
    }

    /// O botão subiu num gesto de guia: a guia vira correção. Devolve se o
    /// gesto era de guia.
    pub(super) fn soltar_guia(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(g) = self.guias_mut() else {
            return false;
        };
        if let Some((de, ate)) = g.tracando.take() {
            let (dx, dy) = (f32::from(ate.x - de.x), f32::from(ate.y - de.y));
            if dx.hypot(dy) < MENOR_TRACO {
                cx.notify();
                return true;
            }
            let (Some(a), Some(b)) = (self.ponto_na_foto(de), self.ponto_na_foto(ate)) else {
                return true;
            };
            // O eixo sai da direção do traço **na tela**, e é guardado nos
            // eixos da foto de pé.
            let na_tela = if dy.abs() >= dx.abs() {
                EixoDaGuia::Vertical
            } else {
                EixoDaGuia::Horizontal
            };
            let eixo = self.eixo_na_tela(na_tela);
            let mut guias = self.corte_atual().perspectiva().guias;
            if let Some(vaga) = guias.iter().position(Option::is_none) {
                guias[vaga] = Some(GuiaDePerspectiva {
                    de: a,
                    ate: b,
                    eixo,
                });
                if let Some(g) = self.guias_mut() {
                    g.selecionada = Some(vaga);
                }
                self.definir_guias(guias, window, cx);
            }
            return true;
        }
        if g.arrasto.take().is_some() {
            let rascunho = g.rascunho.take();
            if let Some(guias) = rascunho {
                self.definir_guias(guias, window, cx);
            }
            cx.notify();
            return true;
        }
        false
    }

    /// `Esc` com o traçado ligado: larga o gesto; sem gesto, desliga o
    /// traçado. Devolve se consumiu a tecla.
    pub(super) fn esc_das_guias(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(g) = self.guias_mut() else {
            return false;
        };
        if g.em_gesto() {
            g.tracando = None;
            g.arrasto = None;
            g.rascunho = None;
            cx.notify();
            return true;
        }
        if g.armadas {
            g.armadas = false;
            g.selecionada = None;
            cx.notify();
            return true;
        }
        false
    }

    /// A camada das guias, por cima da foto, quando o traçado está ligado.
    pub(super) fn overlay_das_guias(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let g = self.guias().filter(|g| g.armadas)?;
        let origem = self.palco.origin;
        let selecionada = g.selecionada;
        let guias: Vec<GuiaNoPalco> = self
            .guias_vigentes()
            .iter()
            .enumerate()
            .filter_map(|(i, g)| {
                let g = g.as_ref()?;
                Some((
                    i,
                    self.ponto_no_palco(g.de)?,
                    self.ponto_no_palco(g.ate)?,
                    self.eixo_na_tela(g.eixo),
                ))
            })
            .collect();
        let tracando = g.tracando.map(|(de, ate)| {
            let (a, b) = (de - origem, ate - origem);
            (
                (f32::from(a.x), f32::from(a.y)),
                (f32::from(b.x), f32::from(b.y)),
            )
        });

        let cor = |eixo: EixoDaGuia| -> Hsla {
            match eixo {
                EixoDaGuia::Vertical => gpui_kit::rgb(COR_VERTICAL).into(),
                EixoDaGuia::Horizontal => gpui_kit::rgb(COR_HORIZONTAL).into(),
            }
        };
        let linhas: Vec<Traco> = guias
            .iter()
            .map(|(i, a, b, eixo)| (*a, *b, cor(*eixo), selecionada == Some(*i)))
            .chain(tracando.map(|(a, b)| (a, b, Hsla::from(gpui_kit::rgb(AMBAR)), true)))
            .collect();

        let mut pontas: Vec<AnyElement> = Vec::new();
        for (i, a, b, eixo) in &guias {
            for (ponta, (x, y)) in [(Ponta::De, *a), (Ponta::Ate, *b)] {
                let i = *i;
                let escolhida = selecionada == Some(i);
                pontas.push(
                    div()
                        .id(SharedString::from(format!("ponta-{i}-{ponta:?}")))
                        .absolute()
                        .left(px(x - RAIO_DA_PONTA))
                        .top(px(y - RAIO_DA_PONTA))
                        .size(px(RAIO_DA_PONTA * 2.))
                        .rounded_full()
                        .border_2()
                        .border_color(if escolhida {
                            Hsla::from(gpui_kit::rgb(AMBAR))
                        } else {
                            cor(*eixo)
                        })
                        .bg(gpui_kit::rgba(0x000000a6))
                        .cursor(CursorStyle::Crosshair)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |tela, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                tela.comecar_arrasto_da_ponta(i, ponta, cx);
                            }),
                        )
                        .into_any_element(),
                );
            }
        }

        let rotulos = guias.iter().map(|(i, a, b, eixo)| {
            let (mx, my) = ((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
            div()
                .absolute()
                .left(px(mx + 8.))
                .top(px(my - 9.))
                .px(px(5.))
                .rounded(crate::tema::canto(3.))
                .bg(gpui_kit::rgba(0x000000b3))
                .font_family("Menlo")
                .text_size(px(10.))
                .text_color(cor(*eixo))
                .child(format!(
                    "{}{}",
                    i + 1,
                    match eixo {
                        EixoDaGuia::Vertical => "V",
                        EixoDaGuia::Horizontal => "H",
                    }
                ))
        });

        Some(
            div()
                .id("camada-das-guias")
                .absolute()
                .inset_0()
                .cursor(CursorStyle::Crosshair)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|tela, evento: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                        tela.comecar_guia(evento.position, cx);
                    }),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let o = bounds.origin;
                            for (a, b, cor, forte) in &linhas {
                                for (largura, c) in [
                                    (
                                        if *forte { 4.5 } else { 3.5 },
                                        Hsla::from(gpui_kit::rgba(0x000000a6)),
                                    ),
                                    (if *forte { 2.5 } else { 1.5 }, *cor),
                                ] {
                                    let mut traco = PathBuilder::stroke(px(largura));
                                    traco.move_to(o + point(px(a.0), px(a.1)));
                                    traco.line_to(o + point(px(b.0), px(b.1)));
                                    if let Ok(caminho) = traco.build() {
                                        window.paint_path(caminho, c);
                                    }
                                }
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
                .children(rotulos)
                .children(pontas)
                .into_any_element(),
        )
    }

    /// A seção do painel do Enquadrar.
    pub(super) fn secao_da_perspectiva(&self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.corte_atual();
        let p = *atual.perspectiva();
        let tema = cx.theme();
        let (mudo, frente, muted) = (tema.muted_foreground, tema.foreground, tema.muted);
        let armadas = self.tracando_guias();
        let selecionada = self.guias().and_then(|g| g.selecionada);
        let guias = self.guias_vigentes();
        let aviso = self.guias().and_then(|g| g.aviso.clone());
        let neutra = p.e_neutra();
        let restringir = atual.restringir();

        let linha_da_guia = |i: usize, g: GuiaDePerspectiva| {
            let eixo = self.eixo_na_tela(g.eixo);
            let (nome, cor) = match eixo {
                EixoDaGuia::Vertical => ("Vertical", COR_VERTICAL),
                EixoDaGuia::Horizontal => ("Horizontal", COR_HORIZONTAL),
            };
            h_flex()
                .id(SharedString::from(format!("guia-{i}")))
                .gap(px(6.))
                .items_center()
                .px(px(6.))
                .py(px(3.))
                .rounded(crate::tema::canto(4.))
                .when(selecionada == Some(i), |l| l.bg(muted))
                .child(div().size(px(8.)).rounded_full().bg(gpui_kit::rgb(cor)))
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(frente.opacity(0.9))
                        .child(format!("Guia {}", i + 1)),
                )
                .child(
                    Button::new(SharedString::from(format!("guia-eixo-{i}")))
                        .outline()
                        .xsmall()
                        .px(px(6.))
                        .rounded(crate::tema::canto(3.))
                        .text_xs()
                        .text_color(gpui_kit::rgb(cor))
                        .tooltip("Trocar Vertical ↔ Horizontal")
                        .on_click(
                            cx.listener(move |tela, _, window, cx| tela.trocar_eixo(i, window, cx)),
                        )
                        .child(nome),
                )
                .child(
                    estilo::botao_icone(format!("guia-apagar-{i}"), Icone::X, 16., 12.)
                        .rounded(crate::tema::canto(3.))
                        .text_color(mudo)
                        .tooltip("Apagar a guia")
                        .on_click(
                            cx.listener(move |tela, _, window, cx| tela.apagar_guia(i, window, cx)),
                        ),
                )
        };

        let slider =
            |rotulo: &'static str, id: &'static str, vertical: bool, valor: f32, estado| {
                v_flex()
                    .child(
                        h_flex()
                            .justify_between()
                            .text_xs()
                            .child(
                                div()
                                    .id(id)
                                    .text_color(mudo)
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |tela, e: &gpui_kit::ClickEvent, window, cx| {
                                            if e.click_count() >= 2 {
                                                tela.zerar_ajuste(vertical, window, cx);
                                            }
                                        },
                                    ))
                                    .child(rotulo),
                            )
                            .child(
                                div()
                                    .font_family("Menlo")
                                    .text_color(if valor == 0. { mudo } else { frente })
                                    .child(super::enquadrar::rotulo_do_angulo(valor)),
                            ),
                    )
                    .child(div().mt(px(2.)).child(Slider::new(estado).horizontal()))
            };

        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_xs().text_color(mudo).child("Perspectiva guiada"))
                    .child(
                        estilo::alternador("perspectiva-guias", armadas, cx)
                            .h(px(26.))
                            .px(px(8.))
                            .text_xs()
                            .tooltip("Trace sobre a foto retas que deviam ser verticais ou horizontais")
                            .on_click(cx.listener(|tela, _, _, cx| tela.alternar_guias(cx)))
                            .child(Icon::new(Icone::Building2).size(px(14.)))
                            .child("Guias"),
                    ),
            )
            .when(armadas, |secao| {
                secao.child(
                    div().text_size(px(11.)).text_color(mudo).child(
                        "Arraste sobre a foto ao longo de um batente, uma quina, o rodapé. Duas guias bastam; cabem quatro. A foto se corrige ao soltar.",
                    ),
                )
            })
            .child(
                v_flex().gap(px(2.)).children(
                    guias
                        .iter()
                        .enumerate()
                        .filter_map(|(i, g)| g.map(|g| linha_da_guia(i, g))),
                ),
            )
            .children(aviso.map(|t| div().text_size(px(11.)).text_color(gpui_kit::rgb(AMBAR)).child(t)))
            .child(slider("Vertical", "persp-rotulo-vertical", true, p.vertical, &self.persp_vertical))
            .child(slider(
                "Horizontal",
                "persp-rotulo-horizontal",
                false,
                p.horizontal,
                &self.persp_horizontal,
            ))
            .child(
                Checkbox::new("corte-restringir")
                    .checked(restringir)
                    .xsmall()
                    .text_color(frente.opacity(0.9))
                    .label("Restringir ao conteúdo")
                    .tooltip(
                        "O retângulo não passa dos cantos sem foto que o endireitar e a perspectiva deixam",
                    )
                    .on_click(cx.listener(|tela, _: &bool, window, cx| {
                        tela.alternar_restringir(window, cx)
                    })),
            )
            .child({
                estilo::desligado(
                    estilo::botao_contorno("perspectiva-redefinir", cx)
                        .w_full()
                        .h(px(28.))
                        .text_xs()
                        .gap(px(6.))
                        .child(Icon::new(Icone::Undo).size(px(14.)))
                        .child("Redefinir perspectiva"),
                    neutra,
                )
                .when(!neutra, |b| {
                    b.on_click(cx.listener(|tela, _, window, cx| tela.redefinir_perspectiva(window, cx)))
                })
            })
            .into_any_element()
    }

    /// O estado para o roteiro do app real (`estado_perspectiva`).
    pub(super) fn descrever_perspectiva(&self) -> String {
        let c = self.corte_atual();
        let p = c.perspectiva();
        format!(
            "guias={} armadas={} rotacao=[{:.2},{:.2},{:.2}] foco={:.3} vertical={} horizontal={} angulo={} restringir={} corte=({:.4},{:.4},{:.4},{:.4}) aviso={:?}",
            p.quantas_guias(),
            self.tracando_guias(),
            p.rotacao[0],
            p.rotacao[1],
            p.rotacao[2],
            p.foco,
            p.vertical,
            p.horizontal,
            c.angle(),
            c.restringir(),
            c.crop_x(),
            c.crop_y(),
            c.crop_width(),
            c.crop_height(),
            self.guias().and_then(|g| g.aviso.clone()),
        )
    }

    /// Uma guia inteira, em frações da **foto na tela** (o espaço exibido),
    /// para o roteiro: `guia fx1 fy1 fx2 fy2`.
    pub(super) fn guia_pelo_roteiro(
        &mut self,
        de: (f32, f32),
        ate: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((ax, ay, aw, ah)) = self.area_da_foto() else {
            return;
        };
        let o = self.palco.origin;
        let na_janela = |(fx, fy): (f32, f32)| o + point(px(ax + fx * aw), px(ay + fy * ah));
        if !self.tracando_guias() {
            self.alternar_guias(cx);
        }
        if let Some(g) = self.guias_mut() {
            g.tracando = Some((na_janela(de), na_janela(ate)));
        }
        self.soltar_guia(window, cx);
    }
}
