//! A coluna da direita da Revelação — o porte de `revelacao/paineis.tsx`.
//!
//! De cima para baixo, como no site: o aviso de foto comprada, o cabeçalho
//! ("N ajustes fora do neutro" e "Zerar tudo"), as abas **sRGB** e **RGB**, os
//! painéis sanfonados da aba escolhida e o rodapé que diz de que tamanho é a
//! cópia que a tela edita.
//!
//! ⚠️ **Por cima dela, fora da rolagem, ficam o histograma e a curva
//! resultante**, que o site não tem. Eles são recolhíveis (dono, 2026-09-17:
//! *"precisam ter como recolher e o espectograma invade o botão salvar na
//! galeria"*) e têm altura natural: o bloco de altura fixa de antes tinha mais
//! conteúdo que altura, e o que sobrava era desenhado por cima da barra do
//! topo.
//!
//! # O que fica lembrado entre sessões
//!
//! Qual painel está aberto, qual aba (sRGB ou RGB) e se os gráficos estão à
//! mostra — com as chaves do site (`revelacao:<título>`,
//! `revelacao:aba-rgb`), num arquivo ao lado do catálogo. É preferência de
//! quem opera o balcão, como a altura da tira: falhar ao ler ou gravar nunca
//! interrompe nada.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, AnyElement, Bounds, Context, DragMoveEvent, Empty, Hsla,
    MouseButton, MouseDownEvent, MouseUpEvent, PathBuilder, Pixels, SharedString, Window,
};
use gpui_component::button::Button;
use gpui_component::slider::Slider;
use gpui_component::{ActiveTheme, Disableable, Icon, Sizable};

use super::{Aberta, Controle, PedidoDaRevelacao, Revelacao};
use crate::recursos::Icone;
use crate::revelacao::controles::{self, Painel, Secao};
use crate::revelacao::curva::{self, Canal, PONTOS_DA_CURVA};
use crate::revelacao::persistencia::Corte;
use crate::revelacao::processador::Ajustes;
use crate::tema;

/// A chave da aba escolhida — a do site.
const CHAVE_DA_ABA_RGB: &str = "revelacao:aba-rgb";
/// As dos dois gráficos, que só o desktop tem.
const CHAVE_DO_HISTOGRAMA: &str = "revelacao:histograma";
const CHAVE_DA_CURVA_RESULTANTE: &str = "revelacao:curva-resultante";

/// O lado do editor de curva, no `viewBox` do site (`editor-de-curva.tsx`).
const LADO_DA_CURVA: f32 = 260.0;
/// Sobra para o nó da borda não sair pela metade.
const MARGEM_DA_CURVA: f32 = 8.0;
/// O raio do nó, em unidades do `viewBox`.
const RAIO_DO_NO: f32 = 5.0;
/// Até onde o clique ainda pega um nó, em pixels de tela.
const ALCANCE_DO_NO: f32 = 10.0;

/// O estado da coluna que não é da foto: o que está aberto, a aba do HSL, o
/// canal da curva e o nó sendo arrastado.
pub(super) struct EstadoDoPainel {
    abertos: HashMap<String, bool>,
    /// Onde a lembrança mora. `None` nos testes, que nunca tocam o arquivo de
    /// quem trabalha.
    arquivo: Option<PathBuf>,
    /// Qual das três famílias do HSL está à mostra. Como no site, não é
    /// lembrada entre sessões (`useState(0)`), só entre fotos.
    pub(super) aba_hsl: Secao,
    /// O canal da curva por ponto à mostra.
    canal: Canal,
    /// O nó que o ponteiro pegou no último clique.
    no_arrastado: Option<usize>,
    /// Onde o editor de curva foi desenhado no último quadro — o clique chega
    /// em coordenada de janela.
    area_da_curva: Rc<Cell<Bounds<Pixels>>>,
    /// A rolagem da coluna, para o roteiro de depuração.
    rolagem: gpui::ScrollHandle,
}

impl Default for EstadoDoPainel {
    fn default() -> Self {
        let arquivo = arquivo_da_lembranca();
        let abertos = arquivo
            .as_ref()
            .and_then(|caminho| std::fs::read_to_string(caminho).ok())
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();
        Self {
            abertos,
            arquivo,
            aba_hsl: Secao::HslCor,
            canal: Canal::Rgb,
            no_arrastado: None,
            area_da_curva: Rc::new(Cell::new(Bounds::default())),
            rolagem: gpui::ScrollHandle::new(),
        }
    }
}

#[cfg(not(test))]
fn arquivo_da_lembranca() -> Option<PathBuf> {
    Some(infrastructure::paths::AppPaths::catalog_root().join("revelacao-paineis.json"))
}

#[cfg(test)]
fn arquivo_da_lembranca() -> Option<PathBuf> {
    None
}

impl EstadoDoPainel {
    /// Aberto ou fechado — o gravado, ou o padrão.
    fn aberto(&self, chave: &str, padrao: bool) -> bool {
        self.abertos.get(chave).copied().unwrap_or(padrao)
    }

    /// Guarda. Não poder lembrar não pode impedir de abrir o painel.
    fn definir(&mut self, chave: &str, valor: bool) {
        self.abertos.insert(chave.to_string(), valor);
        let Some(caminho) = self.arquivo.as_ref() else {
            return;
        };
        let Ok(texto) = serde_json::to_string_pretty(&self.abertos) else {
            return;
        };
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        if let Err(erro) = std::fs::write(caminho, texto) {
            eprintln!("⚠️ [Revelação] a arrumação dos painéis não foi gravada: {erro}");
        }
    }

    fn alternar(&mut self, chave: &str, padrao: bool) {
        let valor = !self.aberto(chave, padrao);
        self.definir(chave, valor);
    }

    fn no_rgb(&self) -> bool {
        self.aberto(CHAVE_DA_ABA_RGB, false)
    }
}

/// O enquadramento da foto inteira, **escrito** — e não o `Corte::default`,
/// que na gravação quer dizer "não mexa" e deixaria o recorte de antes de pé.
fn corte_inteiro() -> Corte {
    Corte {
        x: Some(0.0),
        y: Some(0.0),
        largura: Some(1.0),
        altura: Some(1.0),
        rotacao: Some(0),
        angulo: Some(0.0),
        espelho_h: Some(false),
        espelho_v: Some(false),
    }
}

/// A arte que acompanha o ponteiro no arrasto de um nó: nenhuma.
struct SemFantasma;

impl Render for SemFantasma {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// O tipo do arrasto de um nó da curva.
#[derive(Clone, Copy)]
struct ArrastoDoNo;

/// O ponto âmbar de "alterado" — `size-1.5 rounded-full bg-amber-400`.
fn ponto_ambar() -> gpui::Div {
    div()
        .size(px(6.))
        .flex_none()
        .rounded_full()
        .bg(tema::cores::quente())
}

impl Revelacao {
    /// Os controles aceitam gesto? Só com a foto carregada **e** revelável — o
    /// `desabilitado={!pronto || !podeRevelar}` do site. Uma foto do site
    /// comprada (ou apagada) não se revela: o cliente pode já ter baixado o
    /// original (`Revelacao::pode_revelar`).
    fn controles_ligados(&self) -> bool {
        self.tem_pixels() && self.pode_revelar()
    }

    /// Quantos campos do motor estão fora do neutro — **os 171**, como o
    /// cabeçalho do site conta.
    pub(super) fn quantos_alterados(&self) -> usize {
        controles::quantos_fora_do_neutro(&self.ajustes)
    }

    /// Se alguma coisa desta família saiu do neutro.
    pub(super) fn secao_alterada(&self, secao: Secao) -> bool {
        controles::secao_alterada(&self.ajustes, secao)
    }

    /// A foto tem enquadramento — recorte, giro, endireitamento ou espelho?
    pub(super) fn enquadrada(&self) -> bool {
        let c = self.corte_atual();
        let perto = |a: f32, b: f32| (a - b).abs() < 1e-4;
        !(perto(c.crop_x(), 0.0)
            && perto(c.crop_y(), 0.0)
            && perto(c.crop_width(), 1.0)
            && perto(c.crop_height(), 1.0)
            && c.rotation_90() == 0
            && perto(c.angle(), 0.0)
            && !c.flip_horizontal()
            && !c.flip_vertical())
    }

    /// **Zerar tudo**: os 171 ao neutro **e** o enquadramento à foto inteira.
    ///
    /// 🚨 *"A função 'Zerar tudo' tem que ser tudo mesmo, inclusive corte e
    /// rotacionamento"* (dono, 2026-09-12, `editor.tsx:2106`). Até aqui o corte
    /// sobrevivia, e uma foto só girada não reagia ao botão.
    ///
    /// É um gesto discreto: fecha o que estava a meio caminho, vira um passo de
    /// histórico — o `Cmd+Z` devolve ajustes e corte juntos — e vai para o
    /// banco na hora. Foto comprada não se zera.
    pub fn redefinir_ajustes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pode_revelar() {
            return;
        }
        self.gravar_o_que_estiver_pendente();

        self.ajustes = Ajustes::default();
        self.corte = corte_inteiro();
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        // O corte não passa pela GPU: quem o mostra é a exibição.
        self.atualizar_exibicao();
        self.historico.registrar(self.estado());
        self.gravar();
        cx.notify();
    }

    /// O botão "Zerar tudo" (ou "Zerar N fotos"): esta foto pelo histórico, e
    /// as outras marcadas pela raiz.
    fn zerar_tudo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.controles_ligados() && (self.quantos_alterados() > 0 || self.enquadrada()) {
            self.redefinir_ajustes(window, cx);
        }
        if !self.outras_a_zerar().is_empty() {
            cx.emit(PedidoDaRevelacao::ZerarAsMarcadas);
        }
    }

    /// Um gesto discreto sobre os ajustes — o duplo clique num nó, o "Zerar"
    /// de um canal. O mesmo caminho de `devolver_ao_neutro`: fecha o pendente,
    /// muda, vira um passo e grava na hora. Sem mudança, não escreve nada.
    fn gesto_discreto(
        &mut self,
        mudar: impl FnOnce(&mut Ajustes),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.controles_ligados() {
            return;
        }
        let mut novo = self.ajustes;
        mudar(&mut novo);
        if novo == self.ajustes {
            return;
        }
        self.gravar_o_que_estiver_pendente();
        self.ajustes = novo;
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        self.historico.registrar(self.estado());
        self.gravar();
        cx.notify();
    }

    /// O nó `i` do canal à mostra foi arrastado até `altura` (0–255): como
    /// os sliders, pede a GPU e deixa a espera fechar o gesto.
    fn mover_no_da_curva(&mut self, i: usize, altura: f32, cx: &mut Context<Self>) {
        let canal = self.estado_do_painel.canal;
        if canal.alturas(&self.ajustes)[i] == altura {
            return;
        }
        canal.definir(&mut self.ajustes, i, altura);
        self.pedir_revelacao(cx);
        self.adiar_gravacao(cx);
        cx.notify();
    }

    /// O duplo clique num nó: ele volta à reta, num passo de histórico.
    fn devolver_no_a_reta(
        &mut self,
        canal: Canal,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reta = curva::curva_neutra()[i];
        self.gesto_discreto(|a| canal.definir(a, i, reta), window, cx);
        self.estado_do_painel.no_arrastado = None;
    }

    /// Um passo do roteiro de depuração (`painel …`, em `depuracao.rs`).
    pub fn seguir_o_roteiro_do_painel(&mut self, pedido: &str, cx: &mut Context<Self>) {
        let (comando, resto) = pedido.split_once(' ').unwrap_or((pedido, ""));
        match comando {
            "rgb" => self.estado_do_painel.definir(CHAVE_DA_ABA_RGB, true),
            "srgb" => self.estado_do_painel.definir(CHAVE_DA_ABA_RGB, false),
            "abrir" | "fechar" => match Painel::TODOS.into_iter().find(|p| p.rotulo() == resto) {
                Some(painel) => self
                    .estado_do_painel
                    .definir(&painel.chave(), comando == "abrir"),
                None => eprintln!("[roteiro] painel desconhecido: '{resto}'"),
            },
            "rolar" => {
                let rolagem = &self.estado_do_painel.rolagem;
                let alvo = match resto {
                    "fim" => f32::from(rolagem.max_offset().height),
                    numero => numero.parse().unwrap_or(0.0),
                };
                rolagem.set_offset(gpui::point(px(0.), px(-alvo)));
            }
            outro => eprintln!("[roteiro] não sei fazer 'painel {outro}'"),
        }
        cx.notify();
    }

    /// A coluna da direita.
    ///
    /// 🔑 **No modo de enquadramento ela troca de conteúdo**, como no site
    /// (`enquadrando ? <PainelDeCorte/> : <Paineis/>`); o rodapé fica nos dois.
    pub(super) fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sem GPU não há revelação. `None` é "a thread ainda está abrindo o
        // dispositivo" — não é ausência de placa.
        let sem_motor = self.processador.disponivel() == Some(false);
        let enquadrando = self.edicao.is_some();

        let barra = self.barra_de_corte(cx).map(IntoElement::into_any_element);
        let aviso = self.aviso_de_comprada(cx);
        let mut corpo: Vec<AnyElement> = Vec::new();
        if !enquadrando {
            corpo.push(self.cabecalho_dos_ajustes(cx));
            corpo.push(self.abas_de_espaco(cx));
            let paineis: &[Painel] = if self.estado_do_painel.no_rgb() {
                &Painel::RGB
            } else {
                &Painel::SRGB
            };
            for painel in paineis {
                corpo.push(self.painel_sanfonado(*painel, cx));
            }
        }
        let rodape = self.rodape(cx);
        let graficos = self.painel_dos_graficos(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            // 🚨 Nada desenha fora da coluna — era por aqui que o histograma
            // subia por cima do "Salvar na galeria".
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .child(graficos)
            .child(
                div()
                    .id("painel-de-ajustes")
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(12.))
                    .overflow_y_scroll()
                    .track_scroll(&self.estado_do_painel.rolagem)
                    .children(aviso)
                    .when(sem_motor, |painel| {
                        painel.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child("Sem GPU disponível — os ajustes não são aplicados"),
                        )
                    })
                    .when(self.mostrando_original, |painel| {
                        painel.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child("Mostrando o original (\\ para voltar)"),
                        )
                    })
                    .children(barra)
                    .children(corpo)
                    .children(rodape),
            )
    }

    /// "Esta foto foi **comprada**…" — o aviso do site, no alto da coluna.
    fn aviso_de_comprada(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.aberta.is_none() || self.pode_revelar() {
            return None;
        }
        Some(
            div()
                .rounded(px(4.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .p(px(8.))
                .text_size(px(11.))
                .line_height(px(15.))
                .text_color(cx.theme().muted_foreground)
                .child(
                    gpui::StyledText::new(
                        "Esta foto foi comprada. O cliente já pode ter baixado o original, \
                         então ela não se revela — dá para ver e baixar, não para salvar.",
                    )
                    .with_highlights([(
                        14..22,
                        gpui::HighlightStyle {
                            color: Some(cx.theme().foreground),
                            font_weight: Some(gpui::FontWeight::BOLD),
                            ..Default::default()
                        },
                    )]),
                )
                .into_any_element(),
        )
    }

    /// "A tela edita uma cópia de W×H…" — o rodapé do site, nos dois modos
    /// (ajustes e enquadramento), como `editor.tsx:3532`.
    fn rodape(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (largura, altura) = self.tamanho_da_foto()?;
        Some(
            div()
                .text_size(px(11.))
                .line_height(px(15.))
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(format!(
                    "A tela edita uma cópia de {}×{}, para a foto abrir rápido. Ampliada além \
                     dela (Z ou 1:1), a tela carrega o bruto em resolução cheia — que é de onde \
                     o arquivo salvo sai.",
                    largura.round() as u32,
                    altura.round() as u32
                )))
                .into_any_element(),
        )
    }

    /// Quantos ajustes estão fora do neutro, e o botão que devolve todos.
    ///
    /// 🔑 **Fora dos painéis sanfonados, e no topo** — é o desenho do site.
    /// Zerar tudo é o gesto de "recomeçar" e não pertence a nenhuma seção.
    fn cabecalho_dos_ajustes(&self, cx: &mut Context<Self>) -> AnyElement {
        let alterados = self.quantos_alterados();
        let enquadrada = self.aberta.is_some() && self.enquadrada();
        let texto = match (alterados, enquadrada) {
            (0, true) => "Só o enquadramento fora do neutro".to_string(),
            (0, false) => "Nenhum ajuste fora do neutro".to_string(),
            (n, _) => format!(
                "{n} {} fora do neutro{}",
                if n == 1 { "ajuste" } else { "ajustes" },
                if enquadrada { " + enquadramento" } else { "" }
            ),
        };
        // 🚨 **O botão não pode decidir sozinho se há o que zerar.** Olhando só
        // a foto no palco, ele se apagava com ela no neutro — mesmo com quatro
        // marcadas atrás cheias de ajuste (dono, 2026-09-11).
        let outras = self.outras_a_zerar().len();
        let tem_o_que_zerar = alterados > 0 || enquadrada;
        let quantas_fotos = usize::from(tem_o_que_zerar && self.controles_ligados()) + outras;
        let dica = if outras > 0 {
            format!(
                "Devolve ao neutro {quantas_fotos} foto(s) — esta e as marcadas na tira —, \
                 inclusive o enquadramento de cada uma."
            )
        } else {
            "Devolve os 53 ajustes ao neutro, e o enquadramento à foto inteira.".to_string()
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(texto)),
            )
            .child(
                Button::new("zerar-tudo")
                    .icon(Icon::new(Icone::RotateCcw).size(px(14.)))
                    .label(if quantas_fotos > 1 {
                        SharedString::from(format!("Zerar {quantas_fotos} fotos"))
                    } else {
                        SharedString::from("Zerar tudo")
                    })
                    .outline()
                    .xsmall()
                    .disabled(quantas_fotos == 0)
                    .tooltip(dica)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.zerar_tudo(window, cx))),
            )
            .into_any_element()
    }

    /// As duas abas, pelo espaço em que os controles agem (dono, 2026-09-12).
    ///
    /// O ponto âmbar diz em qual aba há ajuste, para um estilo aplicado não
    /// ficar escondido na outra.
    fn abas_de_espaco(&self, cx: &mut Context<Self>) -> AnyElement {
        let no_rgb = self.estado_do_painel.no_rgb();
        let aba = |rgb: bool, rotulo: &'static str, dica: &'static str, cx: &mut Context<Self>| {
            let escolhida = no_rgb == rgb;
            let marcada = controles::aba_alterada(&self.ajustes, rgb);
            div()
                .id(SharedString::from(format!("aba-espaco-{rotulo}")))
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .py(px(4.))
                .rounded(px(4.))
                .cursor_pointer()
                .when(escolhida, |a| {
                    a.bg(cx.theme().muted).text_color(cx.theme().foreground)
                })
                .when(!escolhida, |a| {
                    a.text_color(cx.theme().muted_foreground)
                        .hover(|a| a.text_color(cx.theme().foreground))
                })
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(dica).build(window, cx)
                })
                .child(rotulo)
                .when(marcada, |a| a.child(ponto_ambar()))
                .on_click(cx.listener(move |tela, _ev, _window, cx| {
                    tela.estado_do_painel.definir(CHAVE_DA_ABA_RGB, rgb);
                    cx.notify();
                }))
        };

        div()
            .flex()
            .gap(px(4.))
            .p(px(2.))
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .text_xs()
            .child(aba(
                false,
                "sRGB",
                "Os controles de sempre, sobre a foto com gama — o Lightroom",
                cx,
            ))
            .child(aba(
                true,
                "RGB",
                "Os módulos em RGB linear, fiéis aos estilos do darktable — rodam antes dos sRGB",
                cx,
            ))
            .into_any_element()
    }

    /// Um cabeçalho de sanfona — o `PainelColapsavel` do site.
    fn cabecalho_da_sanfona(
        &self,
        titulo: &'static str,
        aberto: bool,
        alterado: bool,
        chave: String,
        padrao: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("sanfona-{chave}")))
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .cursor_pointer()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(cx.theme().foreground)
            .child(
                Icon::new(if aberto {
                    Icone::ChevronDown
                } else {
                    Icone::ChevronRight
                })
                .size(px(14.))
                .text_color(cx.theme().muted_foreground),
            )
            .child(div().flex_1().min_w(px(0.)).truncate().child(titulo))
            .when(alterado, |c| c.child(ponto_ambar()))
            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                tela.estado_do_painel.alternar(&chave, padrao);
                cx.notify();
            }))
    }

    /// Um painel sanfonado: o cabeçalho sempre, o conteúdo só quando aberto.
    ///
    /// 🔑 **O ponto âmbar no cabeçalho é o que faz a sanfona valer**: fechado,
    /// o painel esconde o que tem dentro — inclusive um ajuste que alguém deixou
    /// lá.
    fn painel_sanfonado(&self, painel: Painel, cx: &mut Context<Self>) -> AnyElement {
        let chave = painel.chave();
        let padrao = painel.nasce_aberto();
        let aberto = self.estado_do_painel.aberto(&chave, padrao);
        let alterado = controles::painel_alterado(&self.ajustes, painel);

        let cabecalho =
            self.cabecalho_da_sanfona(painel.rotulo(), aberto, alterado, chave, padrao, cx);

        let conteudo = aberto.then(|| {
            let mut dentro: Vec<AnyElement> = Vec::new();
            match painel {
                Painel::CurvaPorPonto => dentro.push(self.editor_de_curva(cx)),
                Painel::Hsl => {
                    let visivel = self.estado_do_painel.aba_hsl;
                    dentro.push(self.abas(painel.secoes(), visivel, cx));
                    dentro.extend(self.controles_da_secao(visivel, cx));
                }
                _ => {
                    for secao in painel.secoes() {
                        dentro.extend(self.controles_da_secao(*secao, cx));
                    }
                }
            }
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .border_t_1()
                .border_color(cx.theme().border)
                .children(dentro)
        });

        div()
            .flex()
            .flex_col()
            .flex_none()
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .child(cabecalho)
            .children(conteudo)
            .into_any_element()
    }

    /// A fileira de abas do HSL — Cor, Luminância, Matiz.
    ///
    /// ⚠️ **A aba que não está à mostra também precisa se anunciar**: o
    /// sublinhado âmbar na aba fechada (`underline decoration-amber-400`).
    fn abas(&self, secoes: &'static [Secao], visivel: Secao, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .gap(px(4.))
            .text_xs()
            .children(
                secoes
                    .iter()
                    .map(|secao| {
                        let secao = *secao;
                        let escolhida = secao == visivel;
                        let alterada = self.secao_alterada(secao);

                        div()
                            .id(SharedString::from(format!("aba-{}", secao.rotulo())))
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(escolhida, |aba| {
                                aba.bg(cx.theme().muted).text_color(cx.theme().foreground)
                            })
                            .when(!escolhida, |aba| {
                                aba.text_color(cx.theme().muted_foreground)
                                    .hover(|a| a.text_color(cx.theme().foreground))
                            })
                            .when(alterada && !escolhida, |aba| {
                                aba.underline().text_decoration_color(tema::cores::quente())
                            })
                            .child(Painel::aba(secao))
                            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                tela.estado_do_painel.aba_hsl = secao;
                                cx.notify();
                            }))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    /// Os sliders de uma família, na ordem da tabela.
    fn controles_da_secao(&self, secao: Secao, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.controles
            .iter()
            .enumerate()
            .filter(|(_, controle)| controle.definicao.secao == secao)
            .map(|(i, controle)| self.linha_do_controle(i, controle, cx))
            .collect()
    }

    /// Um controle: o rótulo, o valor e a barra.
    ///
    /// 🔑 **Duplo clique volta ao neutro — no rótulo ou na própria barra**,
    /// como no Lightroom e no site (dono, 2026-09-05: *"quando der dois cliques
    /// no meio do slide deve zerar o efeito"*).
    ///
    /// ⚠️ **Na barra ele escuta na fase de captura, e na soltura.** O punho do
    /// `Slider` para a propagação do clique, e é justamente em cima do punho —
    /// no meio, com o controle no neutro — que o duplo clique costuma cair. E
    /// o clique já moveu o slider para onde o dedo caiu: zerar na soltura
    /// desfaz esse movimento, e o que se vê é o valor pular para o neutro.
    fn linha_do_controle(
        &self,
        indice: usize,
        controle: &Controle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let valor = (definicao.ler)(&self.ajustes);
        let alterado = definicao.alterado(&self.ajustes);
        let ligado = self.controles_ligados();

        div()
            .flex()
            .flex_col()
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(format!("rotulo-{indice}")))
                            .tooltip(|window, cx| {
                                gpui_component::tooltip::Tooltip::new(
                                    "Duplo clique volta ao neutro",
                                )
                                .build(window, cx)
                            })
                            .text_color(if alterado {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(definicao.rotulo)
                            .on_click(cx.listener(
                                move |tela, evento: &gpui::ClickEvent, window, cx| {
                                    if evento.click_count() >= 2 && tela.controles_ligados() {
                                        tela.devolver_ao_neutro(indice, window, cx);
                                    }
                                },
                            )),
                    )
                    // O valor fica ao lado do rótulo, e não dentro da barra:
                    // dentro, ele se move junto com o punho e foge de quem lê.
                    .child(
                        div()
                            .flex_none()
                            .text_color(cx.theme().foreground.opacity(0.9))
                            .child(SharedString::from(definicao.formatar(valor))),
                    ),
            )
            .child(
                div()
                    .when(ligado, |barra| {
                        barra.capture_any_mouse_up(cx.listener(
                            move |tela, evento: &MouseUpEvent, window, cx| {
                                if evento.button == MouseButton::Left
                                    && evento.click_count >= 2
                                    && tela.controles_ligados()
                                {
                                    tela.devolver_ao_neutro(indice, window, cx);
                                }
                            },
                        ))
                    })
                    .child(Slider::new(&controle.estado).horizontal().disabled(!ligado)),
            )
            .into_any_element()
    }

    /// A curva por ponto: quatro canais, nove alturas cada — o
    /// `EditorDeCurva` do site.
    ///
    /// 🚨 **O traço é a conta do motor** ([`curva::avaliar_curva`], a tradução
    /// de `curva_por_ponto` do `corpo.wgsl`), e não uma spline bonita.
    ///
    /// 🔑 **Sem arrasto no eixo x**: os nove x são fixos. Duplo clique num nó
    /// devolve o nó à reta; "Zerar" devolve o canal inteiro.
    fn editor_de_curva(&self, cx: &mut Context<Self>) -> AnyElement {
        let estado = &self.estado_do_painel;
        let canal = estado.canal;
        let ligado = self.controles_ligados();
        let alturas = canal.alturas(&self.ajustes);
        let cor_do_canal = |c: Canal, cx: &Context<Self>| -> Hsla {
            match c.cor() {
                Some(hex) => gpui::rgb(hex).into(),
                None => cx.theme().foreground,
            }
        };
        let cor = cor_do_canal(canal, cx);

        let botoes: Vec<AnyElement> = Canal::TODOS
            .into_iter()
            .map(|c| {
                let escolhido = c == canal;
                let usado = !curva::curva_eh_neutra(&c.alturas(&self.ajustes));
                let cor = cor_do_canal(c, cx);
                div()
                    .id(SharedString::from(format!("canal-{}", c.rotulo())))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_size(px(11.))
                    .when(escolhido, |b| b.bg(cx.theme().muted).text_color(cor))
                    .when(!escolhido, |b| {
                        b.text_color(cx.theme().muted_foreground)
                            .hover(|b| b.bg(cx.theme().muted).text_color(cx.theme().foreground))
                    })
                    .child(c.rotulo())
                    // 🔑 O ponto avisa que **outro** canal tem curva: sem ele, um
                    // preset que mexe só no azul parece não ter feito nada.
                    .when(usado, |b| b.child(div().text_size(px(8.)).child("●")))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.estado_do_painel.canal = c;
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        let neutro = curva::curva_eh_neutra(&alturas);
        let zerar = div()
            .id("zerar-canal")
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .tooltip(|window, cx| {
                gpui_component::tooltip::Tooltip::new("Devolve este canal à reta").build(window, cx)
            })
            .when(!ligado || neutro, |z| z.opacity(0.4))
            .when(ligado && !neutro, |z| {
                z.cursor_pointer()
                    .hover(|z| z.bg(cx.theme().muted).text_color(cx.theme().foreground))
                    .on_click(cx.listener(move |tela, _ev, window, cx| {
                        let neutra = curva::curva_neutra();
                        tela.gesto_discreto(
                            |a| {
                                for (i, v) in neutra.iter().enumerate() {
                                    canal.definir(a, i, *v);
                                }
                            },
                            window,
                            cx,
                        );
                    }))
            })
            .child("Zerar");

        // O desenho.
        let area = estado.area_da_curva.clone();
        let fundo = cx.theme().background;
        let grade = cx.theme().border;
        let desenho = canvas(
            move |bounds, _window, _cx| {
                area.set(bounds);
            },
            move |bounds, _prepaint, window, _cx| {
                let escala = f32::from(bounds.size.width) / LADO_DA_CURVA;
                let x0 = f32::from(bounds.origin.x);
                let y0 = f32::from(bounds.origin.y);
                let area_util = LADO_DA_CURVA - MARGEM_DA_CURVA * 2.0;
                let para_x =
                    |nivel: f32| x0 + (MARGEM_DA_CURVA + nivel / 255.0 * area_util) * escala;
                let para_y = |nivel: f32| {
                    y0 + (MARGEM_DA_CURVA + area_util - nivel / 255.0 * area_util) * escala
                };
                let ponto = |x: f32, y: f32| gpui::point(px(x), px(y));

                window.paint_quad(gpui::fill(bounds, fundo).corner_radii(px(4.)));

                // A grade dos quartos de tom, e a diagonal do neutro por baixo.
                for f in [0.25, 0.5, 0.75] {
                    let n = f * 255.0;
                    for (a, b) in [
                        ((para_x(n), para_y(0.0)), (para_x(n), para_y(255.0))),
                        ((para_x(0.0), para_y(n)), (para_x(255.0), para_y(n))),
                    ] {
                        let mut linha = PathBuilder::stroke(px(0.5 * escala));
                        linha.move_to(ponto(a.0, a.1));
                        linha.line_to(ponto(b.0, b.1));
                        if let Ok(caminho) = linha.build() {
                            window.paint_path(caminho, grade);
                        }
                    }
                }
                let mut diagonal = PathBuilder::stroke(px(0.5 * escala))
                    .dash_array(&[px(3.0 * escala), px(3.0 * escala)]);
                diagonal.move_to(ponto(para_x(0.0), para_y(0.0)));
                diagonal.line_to(ponto(para_x(255.0), para_y(255.0)));
                if let Ok(caminho) = diagonal.build() {
                    window.paint_path(caminho, grade);
                }

                // O traço, 65 amostras como no site.
                let mut traco = PathBuilder::stroke(px(1.75 * escala));
                for i in 0..=64 {
                    let x = i as f32 / 64.0 * 255.0;
                    let p = ponto(para_x(x), para_y(curva::avaliar_curva(&alturas, x)));
                    if i == 0 {
                        traco.move_to(p);
                    } else {
                        traco.line_to(p);
                    }
                }
                if let Ok(caminho) = traco.build() {
                    window.paint_path(caminho, cor);
                }

                // Os nove nós.
                let raio = RAIO_DO_NO * escala;
                for (i, altura) in alturas.iter().enumerate() {
                    let cx_ = para_x(i as f32 * 255.0 / (PONTOS_DA_CURVA - 1) as f32);
                    let cy_ = para_y(*altura);
                    window.paint_quad(
                        gpui::fill(
                            Bounds {
                                origin: ponto(cx_ - raio, cy_ - raio),
                                size: gpui::size(px(raio * 2.0), px(raio * 2.0)),
                            },
                            cor,
                        )
                        .corner_radii(px(raio)),
                    );
                }
            },
        )
        .size_full();

        let area = estado.area_da_curva.clone();
        let quadro = div()
            .id("editor-de-curva")
            .w_full()
            .h(px(270.))
            .when(!ligado, |q| q.opacity(0.4))
            .child(desenho)
            .when(ligado, |q| {
                q.cursor_ns_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |tela, evento: &MouseDownEvent, window, cx| {
                            let limites = area.get();
                            let no = no_sob_o_ponteiro(limites, evento.position, &alturas);
                            tela.estado_do_painel.no_arrastado = no;
                            if let (Some(i), true) = (no, evento.click_count >= 2) {
                                tela.devolver_no_a_reta(canal, i, window, cx);
                            }
                        }),
                    )
                    .on_drag(ArrastoDoNo, |_, _, _, cx| cx.new(|_| SemFantasma))
                    .on_drag_move(cx.listener(
                        move |tela, evento: &DragMoveEvent<ArrastoDoNo>, _window, cx| {
                            let Some(i) = tela.estado_do_painel.no_arrastado else {
                                return;
                            };
                            if !tela.controles_ligados() {
                                return;
                            }
                            let limites = evento.bounds;
                            let altura = altura_do_ponteiro(limites, evento.event.position);
                            tela.mover_no_da_curva(i, altura, cx);
                        },
                    ))
            });

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(div().flex().gap(px(4.)).children(botoes))
                    .child(zerar),
            )
            .child(quadro)
            .into_any_element()
    }

    /// O histograma e a curva resultante, recolhíveis — só o desktop os tem.
    ///
    /// ⚠️ **Altura natural, e não fixa.** O bloco de 200px de antes tinha
    /// 260px de conteúdo; o excesso era desenhado por cima da barra do topo.
    fn painel_dos_graficos(&self, cx: &mut Context<Self>) -> AnyElement {
        let estado = &self.estado_do_painel;
        // 🚨 **Nascem recolhidos.** Eles não existem no site, e abertos ocupavam
        // mais de meia coluna: a Revelação abria sem **um** slider à vista, e o
        // que o operador vê na web ao abrir é o painel Básico. Quem os quer
        // abre uma vez — a escolha fica lembrada entre sessões.
        let com_histograma = estado.aberto(CHAVE_DO_HISTOGRAMA, false);
        let com_curva = estado.aberto(CHAVE_DA_CURVA_RESULTANTE, false);

        let cabecalho_hist = self.cabecalho_da_sanfona(
            "Histograma",
            com_histograma,
            false,
            CHAVE_DO_HISTOGRAMA.to_string(),
            true,
            cx,
        );
        let cabecalho_curva = self.cabecalho_da_sanfona(
            "Curva resultante",
            com_curva,
            false,
            CHAVE_DA_CURVA_RESULTANTE.to_string(),
            true,
            cx,
        );

        div()
            .flex()
            .flex_col()
            .flex_none()
            .overflow_hidden()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(cabecalho_hist)
            .when(com_histograma, |g| {
                g.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .px(px(12.))
                        .pb(px(8.))
                        .child(self.histograma(cx))
                        // O "Auto" mora aqui, junto da medida que ele usa: o
                        // Básico do site não tem esse botão, e ele ficou no
                        // lugar que só o desktop tem.
                        .child(self.botao_do_automatico(cx)),
                )
            })
            .child(cabecalho_curva)
            .when(com_curva, |g| {
                g.child(div().px(px(12.)).pb(px(8.)).child(self.curva_de_tons(cx)))
            })
            .into_any_element()
    }

    /// O botão do tom automático.
    ///
    /// **Desligado sem foto crua**: sem pixels no cache não há histograma.
    fn botao_do_automatico(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pronto = matches!(self.aberta.as_ref(), Some(Aberta { bruta: Some(_), .. }))
            && self.pode_revelar();

        Button::new("tom-automatico")
            .label("Tom automático")
            .xsmall()
            .w_full()
            .disabled(!pronto)
            .on_click(cx.listener(|tela, _ev, window, cx| {
                tela.tom_automatico(window, cx);
            }))
    }

    /// O histograma, desenhado com `paint_quad` dentro de um `canvas`.
    ///
    /// 🔑 **256 colunas × 3 canais não podem ser 768 `div`s.**
    fn histograma(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const ALTURA: f32 = 70.0;

        let barras: Vec<(f32, f32, f32)> = match self.histograma.as_ref() {
            Some(histograma) => histograma.alturas().collect(),
            None => Vec::new(),
        };
        let fundo = cx.theme().background;

        div().h(px(ALTURA)).w_full().flex_none().child(
            canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _prepaint, window, _cx| {
                    window.paint_quad(gpui::fill(bounds, fundo));
                    if barras.is_empty() {
                        return;
                    }
                    let largura = f32::from(bounds.size.width) / barras.len() as f32;
                    let base = f32::from(bounds.origin.y) + f32::from(bounds.size.height);
                    for (i, (r, g, b)) in barras.iter().enumerate() {
                        let x = f32::from(bounds.origin.x) + i as f32 * largura;
                        // Os três canais somam luz onde se sobrepõem.
                        for (altura, cor) in [
                            (r, gpui::rgba(0xff000064)),
                            (g, gpui::rgba(0x00ff0064)),
                            (b, gpui::rgba(0x0000ff64)),
                        ] {
                            let alta = (altura * ALTURA).min(ALTURA);
                            if alta <= 0.0 {
                                continue;
                            }
                            window.paint_quad(gpui::fill(
                                Bounds {
                                    origin: gpui::point(px(x), px(base - alta)),
                                    size: gpui::size(px(largura.max(1.0)), px(alta)),
                                },
                                cor,
                            ));
                        }
                    }
                },
            )
            .size_full(),
        )
    }

    /// A curva resultante: a diagonal pontilhada e a curva de agora, por cima.
    fn curva_de_tons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const ALTURA: f32 = 120.0;

        let pontos = curva::curva(&self.ajustes);
        let fundo = cx.theme().background;
        let borda = cx.theme().border;
        let linha = cx.theme().primary;

        div().h(px(ALTURA)).w_full().flex_none().child(
            canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _prepaint, window, _cx| {
                    window.paint_quad(gpui::fill(bounds, fundo));

                    let x0 = f32::from(bounds.origin.x);
                    let y0 = f32::from(bounds.origin.y);
                    let largura = f32::from(bounds.size.width);
                    let altura = f32::from(bounds.size.height);
                    let ponto = |t: f32, v: f32| {
                        gpui::point(px(x0 + t * largura), px(y0 + (1.0 - v) * altura))
                    };

                    let mut diagonal = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]);
                    diagonal.move_to(ponto(0.0, 0.0));
                    diagonal.line_to(ponto(1.0, 1.0));
                    if let Ok(caminho) = diagonal.build() {
                        window.paint_path(caminho, borda);
                    }

                    let mut traco = PathBuilder::stroke(px(1.5));
                    let ultimo = (pontos.len() - 1) as f32;
                    for (i, v) in pontos.iter().enumerate() {
                        let p = ponto(i as f32 / ultimo, *v);
                        if i == 0 {
                            traco.move_to(p);
                        } else {
                            traco.line_to(p);
                        }
                    }
                    if let Ok(caminho) = traco.build() {
                        window.paint_path(caminho, linha);
                    }
                },
            )
            .size_full(),
        )
    }
}

/// 🧪 Os gestos da coluna para os cenários de ponta a ponta (`crate::e2e`) —
/// os mesmos caminhos dos cliques, sem precisar achar o nó na tela.
#[cfg(test)]
impl Revelacao {
    /// O clique numa aba de espaço: sRGB (`false`) ou RGB (`true`).
    pub(crate) fn escolher_aba_rgb(&mut self, rgb: bool, cx: &mut Context<Self>) {
        self.estado_do_painel.definir(CHAVE_DA_ABA_RGB, rgb);
        cx.notify();
    }

    pub(crate) fn na_aba_rgb(&self) -> bool {
        self.estado_do_painel.no_rgb()
    }

    /// O ponto âmbar de cada aba: `(sRGB, RGB)`.
    pub(crate) fn abas_alteradas(&self) -> (bool, bool) {
        (
            controles::aba_alterada(&self.ajustes, false),
            controles::aba_alterada(&self.ajustes, true),
        )
    }

    /// O clique no botão de um canal da curva.
    pub(crate) fn escolher_canal_da_curva(&mut self, canal: Canal, cx: &mut Context<Self>) {
        self.estado_do_painel.canal = canal;
        cx.notify();
    }

    /// Pega o nó `i` e o arrasta até `altura`.
    pub(crate) fn arrastar_no_da_curva(&mut self, i: usize, altura: f32, cx: &mut Context<Self>) {
        if !self.controles_ligados() {
            return;
        }
        self.estado_do_painel.no_arrastado = Some(i);
        self.mover_no_da_curva(i, altura, cx);
    }

    /// O duplo clique no nó `i` do canal à mostra.
    pub(crate) fn duplo_clique_no_da_curva(
        &mut self,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let canal = self.estado_do_painel.canal;
        self.devolver_no_a_reta(canal, i, window, cx);
    }

    /// O clique no "Zerar tudo" do cabeçalho.
    pub(crate) fn clicar_em_zerar_tudo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.zerar_tudo(window, cx);
    }

    /// As alturas dos nove nós do canal.
    pub(crate) fn alturas_da_curva(&self, canal: Canal) -> [f32; PONTOS_DA_CURVA] {
        canal.alturas(&self.ajustes)
    }
}

/// O nó mais perto do ponteiro, se algum estiver ao alcance.
fn no_sob_o_ponteiro(
    limites: Bounds<Pixels>,
    ponteiro: gpui::Point<Pixels>,
    alturas: &[f32; PONTOS_DA_CURVA],
) -> Option<usize> {
    let escala = f32::from(limites.size.width) / LADO_DA_CURVA;
    if escala <= 0.0 {
        return None;
    }
    let area_util = LADO_DA_CURVA - MARGEM_DA_CURVA * 2.0;
    let px_ = f32::from(ponteiro.x) - f32::from(limites.origin.x);
    let py_ = f32::from(ponteiro.y) - f32::from(limites.origin.y);
    alturas
        .iter()
        .enumerate()
        .map(|(i, altura)| {
            let x =
                (MARGEM_DA_CURVA + i as f32 / (PONTOS_DA_CURVA - 1) as f32 * area_util) * escala;
            let y = (MARGEM_DA_CURVA + area_util - altura / 255.0 * area_util) * escala;
            (i, ((x - px_).powi(2) + (y - py_).powi(2)).sqrt())
        })
        .filter(|(_, distancia)| *distancia <= ALCANCE_DO_NO.max(RAIO_DO_NO * escala + 4.0))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// A altura (0–255, inteira) que o ponteiro pede — a conta de
/// `alturaDoEvento` do site.
fn altura_do_ponteiro(limites: Bounds<Pixels>, ponteiro: gpui::Point<Pixels>) -> f32 {
    let lado = f32::from(limites.size.height).max(1.0);
    let area_util = LADO_DA_CURVA - MARGEM_DA_CURVA * 2.0;
    let y = (f32::from(ponteiro.y) - f32::from(limites.origin.y)) / lado * LADO_DA_CURVA;
    (((MARGEM_DA_CURVA + area_util - y) / area_util) * 255.0)
        .clamp(0.0, 255.0)
        .round()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn quadro() -> Bounds<Pixels> {
        Bounds {
            origin: gpui::point(px(100.), px(50.)),
            size: gpui::size(px(260.), px(260.)),
        }
    }

    #[test]
    fn o_ponteiro_no_topo_pede_255_e_no_pe_pede_0() {
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui::point(px(0.), px(58.))),
            255.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui::point(px(0.), px(302.))),
            0.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui::point(px(0.), px(0.))),
            255.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui::point(px(0.), px(999.))),
            0.0
        );
    }

    #[test]
    fn o_clique_pega_o_no_mais_perto() {
        let neutra = curva::curva_neutra();
        // O nó do meio fica em (8 + 122, 8 + 122) do quadro.
        let no = no_sob_o_ponteiro(quadro(), gpui::point(px(231.), px(179.)), &neutra);
        assert_eq!(no, Some(4));
        let longe = no_sob_o_ponteiro(quadro(), gpui::point(px(231.), px(60.)), &neutra);
        assert_eq!(longe, None);
    }

    #[test]
    fn o_corte_inteiro_e_escrito_por_inteiro() {
        let c = corte_inteiro();
        assert_eq!(c.largura, Some(1.0));
        assert_eq!(c.rotacao, Some(0));
        assert_ne!(c, Corte::default(), "o padrão quer dizer 'não mexa'");
    }

    #[test]
    fn a_lembranca_dos_testes_nao_toca_o_disco() {
        let mut estado = EstadoDoPainel::default();
        assert!(!estado.no_rgb());
        estado.definir(CHAVE_DA_ABA_RGB, true);
        assert!(estado.no_rgb());
        assert!(estado.arquivo.is_none());
        assert!(estado.aberto("revelacao:Básico", true));
        estado.alternar("revelacao:Básico", true);
        assert!(!estado.aberto("revelacao:Básico", true));
    }
}
