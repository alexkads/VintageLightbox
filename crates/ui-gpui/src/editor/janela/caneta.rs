//! ✒️ A Caneta na janela: as ferramentas de caminho na mão, o ponteiro
//! convertido para o documento, a sobreposição no palco, a barra de opções,
//! o painel Caminhos e os diálogos "Fazer seleção" e do nome.
//!
//! 🔑 **As regras moram no `editor-core`** (`vetor::caneta`): aqui só se
//! converte o ponteiro (com zoom, deslocamento e giro — a mesma conta do
//! pincel, [`EditorDeFoto::na_foto_sem_limite`]) e os modificadores (⌘ no
//! macOS, Ctrl no Windows e no Linux), e se desenha o que a ferramenta diz.
//! A tolerância do clique chega ao núcleo em pixels do documento por ponto
//! da tela ([`Medida`]): os pontos e as alças têm o mesmo tamanho na tela em
//! qualquer zoom.
//!
//! 🔑 **A sobreposição é só da tela**: desenhada por cima dos ladrilhos com
//! `PathBuilder`, no mesmo quadro do GPUI (sem contexto de GPU próprio), e
//! nunca entra na imagem editada, nas miniaturas nem na exportação — essas
//! saem da composição do documento, que não sabe de caminho nenhum (só da
//! máscara vetorial, que é da camada).

use editor_core::vetor::caneta::{Acao, FerramentaVetorial, Medida, Modificadores};
use editor_core::vetor::{
    edicao, geometria, Lado, Ligacao, LugarDoCaminho, OperacaoDoComponente, Ponto,
    RegraDePreenchimento, Subcaminho,
};
use editor_core::Sessao;
use gpui_kit::component::button::ButtonGroup;
use gpui_kit::component::input::{InputEvent, InputState, NumberInput};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, Context, Entity, Focusable as _, MouseButton,
    PathBuilder, Pixels, Point, SharedString, Window,
};

use super::aparencia;
use super::ferramentas::na_plataforma;
use super::{f, nome_do_modo, EditorDeFoto, Item, Tela};
use crate::recursos::Icone;

/// A cor das linhas do caminho na tela ("Opções de caminho" do Photoshop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorDoCaminho {
    Azul,
    Vermelho,
    Verde,
    Magenta,
    Preto,
    Branco,
}

impl CorDoCaminho {
    pub const TODAS: [CorDoCaminho; 6] = [
        CorDoCaminho::Azul,
        CorDoCaminho::Vermelho,
        CorDoCaminho::Verde,
        CorDoCaminho::Magenta,
        CorDoCaminho::Preto,
        CorDoCaminho::Branco,
    ];

    pub fn nome(self) -> &'static str {
        match self {
            CorDoCaminho::Azul => "Azul",
            CorDoCaminho::Vermelho => "Vermelho",
            CorDoCaminho::Verde => "Verde",
            CorDoCaminho::Magenta => "Magenta",
            CorDoCaminho::Preto => "Preto",
            CorDoCaminho::Branco => "Branco",
        }
    }

    pub fn hsla(self) -> gpui_kit::Hsla {
        let rgb: u32 = match self {
            CorDoCaminho::Azul => 0x1f8bff,
            CorDoCaminho::Vermelho => 0xff3030,
            CorDoCaminho::Verde => 0x22d34a,
            CorDoCaminho::Magenta => 0xff3cf0,
            CorDoCaminho::Preto => 0x000000,
            CorDoCaminho::Branco => 0xffffff,
        };
        gpui_kit::rgb(rgb).into()
    }
}

/// Como o caminho aparece no palco: espessura (pontos da tela) e cor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AparenciaDoCaminho {
    pub espessura: f32,
    pub cor: CorDoCaminho,
}

impl Default for AparenciaDoCaminho {
    fn default() -> Self {
        Self {
            espessura: 1.0,
            cor: CorDoCaminho::Azul,
        }
    }
}

/// O diálogo aberto das ferramentas de caminho.
pub enum DialogoDoCaminho {
    /// "Fazer seleção": a difusão (o campo), e o antisserrilhado e a operação
    /// das opções guardadas.
    FazerSelecao(Entity<InputState>),
    /// O nome de um caminho: `None` salva o de trabalho; `Some` renomeia.
    Nome(Option<LugarDoCaminho>, Entity<InputState>),
}

/// O lado de uma âncora na tela, em pontos.
const LADO_DA_ANCORA: f32 = 7.0;
/// O raio do ponto de uma alça, em pontos.
const RAIO_DA_ALCA: f32 = 3.0;

/// Os modificadores do gesto na regra da plataforma: ⌘ no macOS, Ctrl no
/// Windows e no Linux.
pub fn modificadores(m: gpui_kit::Modifiers) -> Modificadores {
    Modificadores {
        shift: m.shift,
        alt: m.alt,
        comando: m.secondary(),
    }
}

fn ponto(p: (f32, f32)) -> Ponto {
    Ponto::novo(p.0 as f64, p.1 as f64)
}

impl EditorDeFoto {
    /// A ferramenta de caminho na mão.
    pub fn ferramenta_vetorial(&self) -> Option<FerramentaVetorial> {
        self.vetorial
    }

    /// Uma ferramenta de caminho na mão (a barra, as letras P e A).
    pub fn usar_ferramenta_vetorial(&mut self, f: FerramentaVetorial, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() {
            return;
        }
        self.vetorial = Some(f);
        self.selecionando = None;
        self.auxiliar = None;
        self.gesto_de_selecao = None;
        self.lembrar_na_barra(Item::P(f));
        if let Some(s) = self.sessao_mut() {
            s.usar_ferramenta_vetorial(f);
        }
        cx.notify();
    }

    /// Outra ferramenta foi para a mão: o gesto vetorial termina e o desenho
    /// aberto fica aberto (a geometria confirmada não muda).
    pub(super) fn largar_ferramenta_vetorial(&mut self) {
        if self.vetorial.take().is_none() {
            return;
        }
        let medida = self.medida_vetorial();
        if let Some(s) = self.sessao_mut() {
            s.caneta_soltar(medida);
            s.caneta_encerrar();
        }
    }

    /// Um ponto da foto (pixels do documento) na janela, com zoom,
    /// deslocamento e giro — o contrário de `na_foto_sem_limite` (os testes e
    /// o roteiro clicam por aqui).
    pub fn ponto_da_foto_na_janela(&self, x: f32, y: f32) -> Option<Point<Pixels>> {
        let (cena, v) = self.vista_do_zoom()?;
        let tela = Tela {
            v,
            area: cena.area,
            fator_da_tela: self.dpr,
            angulo: self.giro,
        };
        let (sx, sy) = tela.p(x, y);
        Some(gpui_kit::point(
            self.palco.origin.x + px(sx),
            self.palco.origin.y + px(sy),
        ))
    }

    /// Pixels do documento por ponto da tela, no zoom de agora.
    pub fn medida_vetorial(&self) -> Medida {
        let escala = self
            .vista_do_zoom()
            .map_or(1.0, |(_, v)| v.escala)
            .max(1e-4);
        Medida {
            por_ponto: 1.0 / escala as f64,
        }
    }

    /// O botão desceu no palco com uma ferramenta de caminho.
    pub fn apertar_vetorial(
        &mut self,
        posicao: Point<Pixels>,
        m: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.na_foto_sem_limite(posicao) else {
            return;
        };
        let medida = self.medida_vetorial();
        let mods = modificadores(m);
        self.ponteiro = Some(posicao);
        self.na_sessao(cx, |s| {
            s.caneta_apertar(ponto(p), mods, medida);
        });
    }

    /// O ponteiro andou com uma ferramenta de caminho (com ou sem botão).
    pub(super) fn mover_vetorial(
        &mut self,
        posicao: Point<Pixels>,
        m: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.na_foto_sem_limite(posicao) else {
            return;
        };
        let medida = self.medida_vetorial();
        let mods = modificadores(m);
        if let Some(s) = self.sessao_mut() {
            s.caneta_arrastar(ponto(p), mods, medida);
        }
        cx.notify();
    }

    /// O botão subiu: o gesto vira um passo. Falso sem gesto vetorial.
    pub(super) fn soltar_vetorial(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.sessao().is_some_and(Sessao::caneta_em_gesto) {
            return false;
        }
        let medida = self.medida_vetorial();
        if let Some(s) = self.sessao_mut() {
            s.caneta_soltar(medida);
        }
        self.aviso = None;
        cx.notify();
        true
    }

    /// Esc com uma ferramenta de caminho. Falso se a tecla não era dela.
    pub(super) fn esc_vetorial(&mut self, cx: &mut Context<Self>) -> bool {
        let gesto = self.sessao().is_some_and(Sessao::caneta_em_gesto);
        if self.vetorial.is_none() && !gesto {
            return false;
        }
        let usou = self.sessao_mut().is_some_and(|s| s.caneta_esc());
        cx.notify();
        usou || gesto
    }

    /// Enter com a Caneta: termina o desenho aberto (o caminho fica aberto).
    pub(super) fn enter_vetorial(&mut self, cx: &mut Context<Self>) -> bool {
        if self.vetorial.is_none() {
            return false;
        }
        let ok = self.sessao_mut().is_some_and(|s| s.caneta_encerrar());
        cx.notify();
        ok
    }

    /// Delete / ⌫ com uma ferramenta de caminho: a última âncora do desenho,
    /// as âncoras ou os componentes escolhidos. Falso sem nada disso (o Delete
    /// de sempre segue).
    pub(super) fn excluir_vetorial(&mut self, cx: &mut Context<Self>) -> bool {
        if self.vetorial.is_none() {
            return false;
        }
        let ok = self.sessao_mut().is_some_and(|s| s.caneta_excluir());
        cx.notify();
        ok
    }

    /// As setas com uma ferramenta de caminho: 1 px, 10 com ⇧.
    pub(super) fn empurrar_vetorial(
        &mut self,
        tecla: &str,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.vetorial.is_none() {
            return false;
        }
        let passo = if shift { 10.0 } else { 1.0 };
        let d = match tecla {
            "left" => (-passo, 0.0),
            "right" => (passo, 0.0),
            "up" => (0.0, -passo),
            "down" => (0.0, passo),
            _ => return false,
        };
        let ok = self.sessao_mut().is_some_and(|s| s.caneta_empurrar(d));
        cx.notify();
        ok
    }

    // ------------------------------------------------- comandos do caminho

    /// ⌘↵ (Ctrl+Enter): desenhando, termina o caminho aberto (a regra atual
    /// da Adobe); senão, o caminho escolhido vira seleção com as opções da
    /// última vez.
    pub fn comando_enter_do_caminho(&mut self, cx: &mut Context<Self>) {
        let desenhando = self
            .sessao()
            .is_some_and(|s| s.caneta.construindo().is_some());
        if desenhando {
            self.enter_vetorial(cx);
            return;
        }
        self.fazer_selecao_do_caminho(cx);
    }

    /// "Fazer seleção" com as opções guardadas.
    pub fn fazer_selecao_do_caminho(&mut self, cx: &mut Context<Self>) {
        let opcoes = self.opcoes_da_selecao_do_caminho;
        let tem = self
            .sessao()
            .and_then(Sessao::caminho_alvo)
            .is_some_and(|c| c.tem_area());
        if !tem {
            self.aviso = Some((
                "Escolha um caminho com área no painel Caminhos (ou desenhe com a Caneta, P)"
                    .into(),
                true,
            ));
            cx.notify();
            return;
        }
        self.na_sessao(cx, |s| {
            s.fazer_selecao_do_caminho(opcoes);
        });
    }

    pub fn abrir_fazer_selecao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let valor = self.opcoes_da_selecao_do_caminho.difusao.to_string();
        let campo = cx.new(|cx| {
            InputState::new(window, cx)
                .step(1.0)
                .min(0.0)
                .max(250.0)
                .default_value(valor)
        });
        let sub = cx.subscribe_in(
            &campo,
            window,
            |ed: &mut Self, _c, evento: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = evento {
                    ed.confirmar_dialogo_do_caminho(window, cx);
                }
            },
        );
        self._assinatura_do_caminho = Some(sub);
        window.focus(&campo.focus_handle(cx), cx);
        self.dialogo_do_caminho = Some(DialogoDoCaminho::FazerSelecao(campo));
        cx.notify();
    }

    /// O nome do caminho: salvar o de trabalho (`None`) ou renomear.
    pub fn abrir_nome_do_caminho(
        &mut self,
        lugar: Option<LugarDoCaminho>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let atual = match lugar {
            Some(l) => self
                .sessao()
                .map(|s| s.nome_do_caminho(l))
                .unwrap_or_default(),
            None => self
                .sessao()
                .map(|s| s.documento().caminhos.proximo_nome())
                .unwrap_or_default(),
        };
        let campo = cx.new(|cx| InputState::new(window, cx).default_value(atual));
        let sub = cx.subscribe_in(
            &campo,
            window,
            |ed: &mut Self, _c, evento: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = evento {
                    ed.confirmar_dialogo_do_caminho(window, cx);
                }
            },
        );
        self._assinatura_do_caminho = Some(sub);
        window.focus(&campo.focus_handle(cx), cx);
        self.dialogo_do_caminho = Some(DialogoDoCaminho::Nome(lugar, campo));
        cx.notify();
    }

    pub fn confirmar_dialogo_do_caminho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialogo) = self.dialogo_do_caminho.take() else {
            return;
        };
        window.focus(&self.foco, cx);
        match dialogo {
            DialogoDoCaminho::FazerSelecao(campo) => {
                let texto = campo.read(cx).value().to_string();
                match texto.trim().replace(',', ".").parse::<f32>() {
                    Ok(v) if (0.0..=250.0).contains(&v) => {
                        self.opcoes_da_selecao_do_caminho.difusao = v.round() as u32;
                        self.fazer_selecao_do_caminho(cx);
                    }
                    _ => {
                        self.aviso = Some(("A difusão vai de 0 a 250 px".into(), true));
                        cx.notify();
                    }
                }
            }
            DialogoDoCaminho::Nome(lugar, campo) => {
                let nome = campo.read(cx).value().to_string();
                self.na_sessao(cx, |s| match lugar {
                    None => {
                        s.salvar_caminho_de_trabalho(&nome);
                    }
                    Some(l) => {
                        s.renomear_caminho(l, &nome);
                    }
                });
            }
        }
        self._assinatura_do_caminho = None;
    }

    pub fn cancelar_dialogo_do_caminho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogo_do_caminho.take().is_some() {
            self._assinatura_do_caminho = None;
            window.focus(&self.foco, cx);
            cx.notify();
        }
    }

    pub fn dialogo_do_caminho_aberto(&self) -> bool {
        self.dialogo_do_caminho.is_some()
    }

    pub(super) fn desenhar_dialogo_do_caminho(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tema = cx.theme().clone();
        let rodape = |cx: &mut Context<Self>| {
            crate::estilo::rodape_do_dialogo()
                .child(
                    crate::estilo::botao_contorno("editor-caminho-cancelar", cx)
                        .debug_selector(|| "editor-caminho-cancelar".into())
                        .child("Cancelar")
                        .on_click(cx.listener(|ed, _, window, cx| {
                            ed.cancelar_dialogo_do_caminho(window, cx)
                        })),
                )
                .child(
                    crate::estilo::botao_primario("editor-caminho-ok", cx)
                        .debug_selector(|| "editor-caminho-ok".into())
                        .child("OK")
                        .on_click(cx.listener(|ed, _, window, cx| {
                            ed.confirmar_dialogo_do_caminho(window, cx)
                        })),
                )
        };
        let corpo = match self.dialogo_do_caminho.as_ref()? {
            DialogoDoCaminho::FazerSelecao(campo) => {
                let o = self.opcoes_da_selecao_do_caminho;
                let operacoes = ButtonGroup::new("editor-caminho-operacao-da-selecao")
                    .xsmall()
                    .children(super::MODOS.iter().enumerate().map(|(i, modo)| {
                        let id: &'static str = [
                            "editor-caminho-selecao-nova",
                            "editor-caminho-selecao-somar",
                            "editor-caminho-selecao-subtrair",
                            "editor-caminho-selecao-intersectar",
                        ][i];
                        if o.operacao == *modo {
                            crate::estilo::botao_primario_pequeno(id, cx)
                        } else {
                            crate::estilo::botao_contorno_pequeno(id, cx)
                        }
                        .debug_selector(move || id.into())
                        .label(nome_do_modo(*modo))
                        .selected(o.operacao == *modo)
                    }))
                    .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                        if let Some(modo) = cliques.first().and_then(|i| super::MODOS.get(*i)) {
                            ed.opcoes_da_selecao_do_caminho.operacao = *modo;
                            cx.notify();
                        }
                    }));
                gpui_kit::component::v_flex()
                    .gap(px(16.))
                    .child(crate::estilo::cabecalho_do_dialogo(
                        "Fazer seleção",
                        "O caminho escolhido vira seleção, rasterizado na resolução da foto. Editar o caminho depois não muda a seleção feita.",
                        None,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_sm().text_color(tema.muted_foreground).child("Raio de difusão"))
                            .child(
                                div()
                                    .w(px(120.))
                                    .debug_selector(|| "editor-caminho-difusao".into())
                                    .child(crate::estilo::campo(
                                        NumberInput::new(campo).suffix(div().text_sm().child("px")),
                                    )),
                            ),
                    )
                    .child(
                        div().debug_selector(|| "editor-caminho-antisserrilhado".into()).child(
                            gpui_kit::component::checkbox::Checkbox::new(
                                "editor-caminho-antisserrilhado",
                            )
                            .label("Antisserrilhado")
                            .checked(o.suavizar)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                ed.opcoes_da_selecao_do_caminho.suavizar =
                                    !ed.opcoes_da_selecao_do_caminho.suavizar;
                                cx.notify();
                            })),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_sm().text_color(tema.muted_foreground).child("Operação"))
                            .child(operacoes),
                    )
            }
            DialogoDoCaminho::Nome(lugar, campo) => gpui_kit::component::v_flex()
                .gap(px(16.))
                .child(crate::estilo::cabecalho_do_dialogo(
                    if lugar.is_none() {
                        "Salvar caminho"
                    } else {
                        "Renomear caminho"
                    },
                    "Caminhos nomeados ficam no projeto; o de trabalho é trocado quando se começa outro desenho sem caminho escolhido.",
                    None,
                    cx,
                ))
                .child(
                    div()
                        .debug_selector(|| "editor-caminho-nome".into())
                        .child(crate::estilo::campo(gpui_kit::component::input::Input::new(campo))),
                ),
        };
        Some(
            corpo
                .debug_selector(|| "editor-dialogo-do-caminho".into())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(rodape(cx))
                .into_any_element(),
        )
    }

    // -------------------------------------------- operações e opções

    /// A operação dos próximos componentes; com componentes (ou âncoras)
    /// escolhidos, também a deles — num passo.
    pub fn escolher_operacao_do_caminho(
        &mut self,
        op: OperacaoDoComponente,
        cx: &mut Context<Self>,
    ) {
        self.na_sessao(cx, |s| {
            s.caneta.opcoes.operacao = op;
            let mut subs: Vec<u64> = s.caneta.componentes_escolhidos().iter().copied().collect();
            subs.extend(s.caneta.pontos_escolhidos().iter().map(|r| r.sub));
            subs.sort_unstable();
            subs.dedup();
            if !subs.is_empty() {
                s.editar_caminho_alvo(op.nome(), |c| edicao::definir_operacao(c, &subs, op));
            }
        });
    }

    pub fn escolher_regra_do_caminho(
        &mut self,
        regra: RegraDePreenchimento,
        cx: &mut Context<Self>,
    ) {
        self.na_sessao(cx, |s| {
            s.editar_caminho_alvo("Regra de preenchimento", |c| {
                let mudou = c.regra != regra;
                c.regra = regra;
                mudou
            });
        });
    }

    /// Canto, suave ou simétrico nas âncoras escolhidas (Seleção direta).
    pub fn escolher_ligacao(&mut self, ligacao: Ligacao, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            let refs: Vec<_> = s.caneta.pontos_escolhidos().iter().copied().collect();
            if refs.is_empty() {
                return;
            }
            s.editar_caminho_alvo("Converter ponto", |c| {
                let mut mudou = false;
                for r in &refs {
                    mudou |= edicao::definir_ligacao(c, *r, ligacao);
                }
                mudou
            });
        });
    }

    pub fn duplicar_componentes(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            let subs: Vec<u64> = s.caneta.componentes_escolhidos().iter().copied().collect();
            if subs.is_empty() {
                return;
            }
            let mut novos = Vec::new();
            s.editar_caminho_alvo("Duplicar componente", |c| {
                novos = edicao::duplicar_subcaminhos(c, &subs);
                !novos.is_empty()
            });
            s.caneta.escolher_componentes(novos);
        });
    }

    pub fn excluir_componentes(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.caneta_excluir();
        });
    }

    pub fn escolher_caminho_no_painel(
        &mut self,
        lugar: Option<LugarDoCaminho>,
        cx: &mut Context<Self>,
    ) {
        self.na_sessao(cx, |s| s.escolher_caminho(lugar));
    }

    pub fn criar_mascara_vetorial(&mut self, cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("criar a máscara vetorial", cx) {
            return;
        }
        let tem = self
            .sessao()
            .is_some_and(|s| s.camada_ativa().mascara_vetorial.is_some());
        if tem {
            self.aviso = Some(("A camada já tem uma máscara vetorial".into(), true));
            cx.notify();
            return;
        }
        self.na_sessao(cx, |s| {
            s.criar_mascara_vetorial();
        });
    }

    pub fn preencher_caminho(&mut self, cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("preencher o caminho", cx) {
            return;
        }
        let mut ok = false;
        self.na_sessao(cx, |s| {
            let opacidade = s.pincel.opacidade;
            ok = s.preencher_caminho(opacidade);
        });
        if !ok {
            self.aviso = Some((
                "Nada a preencher: escolha um caminho com área e uma camada visível (e veja a seleção)"
                    .into(),
                true,
            ));
            cx.notify();
        }
    }

    pub fn contornar_caminho(&mut self, cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("contornar o caminho", cx) {
            return;
        }
        let mut ok = false;
        self.na_sessao(cx, |s| ok = s.contornar_caminho());
        if !ok {
            self.aviso = Some((
                "Nada a contornar: escolha um caminho e uma camada visível".into(),
                true,
            ));
            cx.notify();
        }
    }

    /// As opções da barra para as ferramentas de caminho — só o que funciona:
    /// não há modo Forma (o editor não tem camada vetorial de forma).
    pub(super) fn opcoes_da_caneta(
        &self,
        f: FerramentaVetorial,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = aparencia::cores(cx);
        let rotulo = |t: &'static str| {
            div()
                .flex_shrink_0()
                .text_xs()
                .text_color(c.apagado)
                .child(t)
                .into_any_element()
        };
        let separador = || {
            div()
                .flex_shrink_0()
                .w(px(1.))
                .h(px(18.))
                .bg(c.borda)
                .into_any_element()
        };
        let sessao = self.sessao();
        let opcoes = sessao.map(|s| s.caneta.opcoes).unwrap_or_default();
        let tem_alvo = sessao.and_then(Sessao::caminho_alvo).is_some();
        let tem_area = sessao
            .and_then(Sessao::caminho_alvo)
            .is_some_and(|c| c.tem_area());
        let regra = sessao
            .and_then(Sessao::caminho_alvo)
            .map(|c| c.regra)
            .unwrap_or_default();
        let mut v: Vec<AnyElement> = vec![rotulo("Modo: Caminho")];
        // Operações dos componentes.
        v.push(separador());
        v.push(rotulo("Componentes"));
        v.push(
            ButtonGroup::new("editor-caminho-operacoes")
                .xsmall()
                .children(
                    OperacaoDoComponente::TODAS
                        .iter()
                        .enumerate()
                        .map(|(i, op)| {
                            let id: &'static str = [
                                "editor-caminho-somar",
                                "editor-caminho-subtrair",
                                "editor-caminho-intersectar",
                                "editor-caminho-excluir-sobreposicao",
                            ][i];
                            let rotulo = ["Somar", "Subtrair", "Cruzar", "Excluir"][i];
                            if opcoes.operacao == *op {
                                crate::estilo::botao_primario_pequeno(id, cx)
                            } else {
                                crate::estilo::botao_contorno_pequeno(id, cx)
                            }
                            .debug_selector(move || id.into())
                            .label(rotulo)
                            .tooltip(format!(
                                "{} — vale para o próximo componente e para os escolhidos",
                                op.nome()
                            ))
                            .selected(opcoes.operacao == *op)
                        }),
                )
                .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                    if let Some(op) = cliques
                        .first()
                        .and_then(|i| OperacaoDoComponente::TODAS.get(*i))
                    {
                        ed.escolher_operacao_do_caminho(*op, cx);
                    }
                }))
                .into_any_element(),
        );
        // A regra de preenchimento do caminho escolhido.
        v.push(
            ButtonGroup::new("editor-caminho-regra")
                .xsmall()
                .child(
                    if regra == RegraDePreenchimento::NaoZero {
                        crate::estilo::botao_primario_pequeno("editor-caminho-nao-zero", cx)
                    } else {
                        crate::estilo::botao_contorno_pequeno("editor-caminho-nao-zero", cx)
                    }
                    .label("Não-zero")
                    .tooltip("Regra de preenchimento: autointerseção cheia (a estrela sai inteira)")
                    .disabled(!tem_alvo)
                    .selected(regra == RegraDePreenchimento::NaoZero),
                )
                .child(
                    if regra == RegraDePreenchimento::ParImpar {
                        crate::estilo::botao_primario_pequeno("editor-caminho-par-impar", cx)
                    } else {
                        crate::estilo::botao_contorno_pequeno("editor-caminho-par-impar", cx)
                    }
                    .label("Par-ímpar")
                    .tooltip(
                        "Regra de preenchimento: autointerseção vazada (o miolo da estrela sai)",
                    )
                    .disabled(!tem_alvo)
                    .selected(regra == RegraDePreenchimento::ParImpar),
                )
                .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                    let regra = if cliques.first() == Some(&1) {
                        RegraDePreenchimento::ParImpar
                    } else {
                        RegraDePreenchimento::NaoZero
                    };
                    ed.escolher_regra_do_caminho(regra, cx);
                }))
                .into_any_element(),
        );
        v.push(separador());
        match f {
            FerramentaVetorial::Caneta => {
                v.push(self.marcar_da_caneta(
                    "editor-caneta-auto",
                    "Adicionar/excluir automaticamente",
                    opcoes.auto_adicionar_excluir,
                    |o| o.auto_adicionar_excluir = !o.auto_adicionar_excluir,
                    cx,
                ));
                v.push(self.marcar_da_caneta(
                    "editor-caneta-faixa",
                    "Faixa elástica",
                    opcoes.previa,
                    |o| o.previa = !o.previa,
                    cx,
                ));
            }
            FerramentaVetorial::SelecaoDireta | FerramentaVetorial::ConverterPonto => {
                let escolhidos = sessao.is_some_and(|s| !s.caneta.pontos_escolhidos().is_empty());
                v.push(rotulo("Ponto"));
                v.push(
                    ButtonGroup::new("editor-caminho-ligacao")
                        .xsmall()
                        .children(
                            [Ligacao::Canto, Ligacao::Suave, Ligacao::Simetrico]
                                .iter()
                                .map(|l| {
                                    let id: &'static str = match l {
                                        Ligacao::Canto => "editor-ponto-canto",
                                        Ligacao::Suave => "editor-ponto-suave",
                                        Ligacao::Simetrico => "editor-ponto-simetrico",
                                    };
                                    crate::estilo::botao_contorno_pequeno(id, cx)
                                        .debug_selector(move || id.into())
                                        .label(l.nome())
                                        .disabled(!escolhidos)
                                }),
                        )
                        .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                            let l = match cliques.first() {
                                Some(0) => Ligacao::Canto,
                                Some(1) => Ligacao::Suave,
                                _ => Ligacao::Simetrico,
                            };
                            ed.escolher_ligacao(l, cx);
                        }))
                        .into_any_element(),
                );
            }
            FerramentaVetorial::SelecaoDeCaminho => {
                let escolhidos =
                    sessao.is_some_and(|s| !s.caneta.componentes_escolhidos().is_empty());
                v.push(
                    crate::estilo::botao_contorno_pequeno("editor-componente-duplicar", cx)
                        .label("Duplicar")
                        .tooltip(na_plataforma(
                            "Duplica os componentes escolhidos (⌥ + arrasto também)",
                        ))
                        .disabled(!escolhidos)
                        .on_click(cx.listener(|ed, _, _, cx| ed.duplicar_componentes(cx)))
                        .into_any_element(),
                );
                v.push(
                    crate::estilo::botao_contorno_pequeno("editor-componente-excluir", cx)
                        .label("Excluir")
                        .tooltip("Exclui os componentes escolhidos (Delete)")
                        .disabled(!escolhidos)
                        .on_click(cx.listener(|ed, _, _, cx| ed.excluir_componentes(cx)))
                        .into_any_element(),
                );
            }
            FerramentaVetorial::AdicionarPonto | FerramentaVetorial::ExcluirPonto => {}
        }
        v.push(separador());
        v.push(
            crate::estilo::botao_contorno_pequeno("editor-caminho-fazer-selecao", cx)
                .label("Seleção…")
                .tooltip(na_plataforma(
                    "Fazer seleção do caminho escolhido (⌘↵ com as últimas opções)",
                ))
                .disabled(!tem_area)
                .on_click(cx.listener(|ed, _, window, cx| ed.abrir_fazer_selecao(window, cx)))
                .into_any_element(),
        );
        v.push(
            crate::estilo::botao_contorno_pequeno("editor-caminho-mascara", cx)
                .label("Máscara")
                .tooltip("Máscara vetorial na camada escolhida, com o caminho escolhido (sem caminho: revela tudo)")
                .on_click(cx.listener(|ed, _, _, cx| ed.criar_mascara_vetorial(cx)))
                .into_any_element(),
        );
        // A aparência do caminho na tela.
        v.push(separador());
        let aparencia = self.aparencia_do_caminho;
        let ed = cx.entity();
        v.push(
            crate::estilo::botao_contorno_pequeno("editor-caminho-aparencia", cx)
                .debug_selector(|| "editor-caminho-aparencia".into())
                .label(format!(
                    "{} · {} px",
                    aparencia.cor.nome(),
                    aparencia.espessura as u32
                ))
                .tooltip("Opções de caminho: a cor e a espessura das linhas na tela (não vão para a foto)")
                .dropdown_menu_with_anchor(gpui_kit::Anchor::TopLeft, move |mut menu, _w, _cx| {
                    for cor in CorDoCaminho::TODAS {
                        let ed = ed.clone();
                        let id: &'static str = match cor {
                            CorDoCaminho::Azul => "editor-caminho-cor-azul",
                            CorDoCaminho::Vermelho => "editor-caminho-cor-vermelho",
                            CorDoCaminho::Verde => "editor-caminho-cor-verde",
                            CorDoCaminho::Magenta => "editor-caminho-cor-magenta",
                            CorDoCaminho::Preto => "editor-caminho-cor-preto",
                            CorDoCaminho::Branco => "editor-caminho-cor-branco",
                        };
                        menu = menu.item(
                            crate::estilo::item_de_menu(id, cor.nome(), None)
                            .checked(aparencia.cor == cor)
                            .on_click(move |_ev, _w, cx| {
                                ed.update(cx, |ed, cx| {
                                    ed.aparencia_do_caminho.cor = cor;
                                    cx.notify();
                                })
                            }),
                        );
                    }
                    menu = menu.separator();
                    for (espessura, id) in [
                        (1.0f32, "editor-caminho-espessura-1"),
                        (2.0, "editor-caminho-espessura-2"),
                        (3.0, "editor-caminho-espessura-3"),
                    ] {
                        let ed = ed.clone();
                        menu = menu.item(
                            crate::estilo::item_de_menu(
                                id,
                                format!("Espessura {} px", espessura as u32),
                                None,
                            )
                            .checked(aparencia.espessura == espessura)
                            .on_click(move |_ev, _w, cx| {
                                ed.update(cx, |ed, cx| {
                                    ed.aparencia_do_caminho.espessura = espessura;
                                    cx.notify();
                                })
                            }),
                        );
                    }
                    menu
                })
                .into_any_element(),
        );
        v
    }

    fn marcar_da_caneta(
        &self,
        id: &'static str,
        nome: &'static str,
        ligada: bool,
        mudar: fn(&mut editor_core::vetor::caneta::OpcoesDaCaneta),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex_shrink_0()
            .debug_selector(move || id.into())
            .child(
                gpui_kit::component::checkbox::Checkbox::new(id)
                    .xsmall()
                    .label(nome)
                    .checked(ligada)
                    .on_click(cx.listener(move |ed, _: &bool, _, cx| {
                        if let Some(s) = ed.sessao_mut() {
                            mudar(&mut s.caneta.opcoes);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    // ------------------------------------------------- a sobreposição

    /// O que um clique faria agora sob o ponteiro (o cursor e o selo).
    pub fn indicacao_da_caneta(&self, m: gpui_kit::Modifiers) -> Option<Acao> {
        let s = self.sessao()?;
        self.vetorial?;
        let p = self.na_foto_sem_limite(self.ponteiro?)?;
        let medida = self.medida_vetorial();
        let mods = modificadores(m);
        let vazio;
        let c = match s.caminho_alvo() {
            Some(c) => c,
            None => {
                vazio = editor_core::vetor::Caminho::novo(0, "");
                &vazio
            }
        };
        if s.caneta_em_gesto() {
            return None;
        }
        Some(s.caneta.decidir(c, ponto(p), mods, medida))
    }

    /// O cursor das ferramentas de caminho: a mira na Caneta (com o selo da
    /// ação ao lado), a seta nas de seleção.
    pub(super) fn cursor_da_caneta(&self, m: gpui_kit::Modifiers) -> Option<gpui_kit::CursorStyle> {
        let f = self.vetorial?;
        let efetiva = self
            .sessao()
            .map_or(f, |s| s.caneta.ferramenta_efetiva(modificadores(m)));
        Some(match efetiva {
            FerramentaVetorial::SelecaoDeCaminho | FerramentaVetorial::SelecaoDireta => {
                gpui_kit::CursorStyle::Arrow
            }
            _ => gpui_kit::CursorStyle::Crosshair,
        })
    }

    /// Os elementos do caminho por cima da foto: as curvas, as âncoras
    /// (cheias as escolhidas), as alças com as linhas de direção, a ponta em
    /// construção, a faixa elástica, o retângulo de seleção e o selo da
    /// ação sob o ponteiro.
    pub(super) fn sobreposicao_da_caneta(
        &self,
        tela: &Tela,
        sessao: &Sessao,
        window: &Window,
    ) -> Vec<AnyElement> {
        let mut v = Vec::new();
        let Some(c) = sessao.caminho_alvo() else {
            if let (Some(f), Some(acao)) =
                (self.vetorial, self.indicacao_da_caneta(window.modifiers()))
            {
                let _ = f;
                v.extend(self.selo(acao));
            }
            return v;
        };
        let aparencia = self.aparencia_do_caminho;
        let cor = aparencia.cor.hsla();
        // As curvas, na tela: a afim leva a Bézier na Bézier, então os pontos
        // de controle vão à tela e a curva é achatada lá (meio ponto de erro).
        let na_tela = |s: &Subcaminho| -> Subcaminho {
            let mut t = s.clone();
            let m = |p: Ponto| {
                let (x, y) = tela.p(p.x as f32, p.y as f32);
                Ponto::novo(x as f64, y as f64)
            };
            for a in &mut t.ancoras {
                a.ponto = m(a.ponto);
                a.entrada = a.entrada.map(m);
                a.saida = a.saida.map(m);
            }
            t
        };
        let mut linhas: Vec<Vec<(f32, f32)>> = Vec::new();
        for s in &c.subcaminhos {
            let t = na_tela(s);
            let mut pontos: Vec<(f32, f32)> = geometria::achatar(&t, 0.35, false)
                .into_iter()
                .map(|p| (p.x as f32, p.y as f32))
                .collect();
            if s.fechado {
                if let Some(primeiro) = pontos.first().copied() {
                    pontos.push(primeiro);
                }
            }
            if pontos.len() >= 2 {
                linhas.push(pontos);
            }
        }
        // A faixa elástica: o próximo segmento até o ponteiro.
        if let Some(seg) = sessao.caneta.previa(c, window.modifiers().shift) {
            let pontos: Vec<(f32, f32)> = (0..=32)
                .map(|k| {
                    let q = geometria::avaliar(&seg, k as f64 / 32.0);
                    tela.p(q.x as f32, q.y as f32)
                })
                .collect();
            v.push(traco(vec![pontos], 1.0, cor.opacity(0.7)));
        }
        if !linhas.is_empty() {
            v.push(traco(linhas, aparencia.espessura, cor));
        }
        let f = self
            .vetorial
            .unwrap_or(FerramentaVetorial::SelecaoDeCaminho);
        let efetiva = sessao
            .caneta
            .ferramenta_efetiva(modificadores(window.modifiers()));
        let escolhidos = sessao.caneta.pontos_escolhidos();
        let componentes = sessao.caneta.componentes_escolhidos();
        let com_alcas = sessao.caneta.alcas_a_mostrar(c);
        let ponta = sessao
            .caneta
            .construindo()
            .and_then(|(sub, extremo)| edicao::ancora_do_extremo(c, sub, extremo));
        let so_componentes = efetiva == FerramentaVetorial::SelecaoDeCaminho;
        // As alças primeiro (as âncoras ficam por cima das linhas delas).
        let mut hastes: Vec<Vec<(f32, f32)>> = Vec::new();
        let mut pontos_das_alcas: Vec<(f32, f32)> = Vec::new();
        if self.vetorial.is_some() && !so_componentes {
            for s in &c.subcaminhos {
                for a in &s.ancoras {
                    let r = editor_core::vetor::RefAncora {
                        sub: s.id,
                        ancora: a.id,
                    };
                    if !com_alcas.contains(&r) {
                        continue;
                    }
                    let pa = tela.p(a.ponto.x as f32, a.ponto.y as f32);
                    for lado in [Lado::Entrada, Lado::Saida] {
                        if let Some(h) = a.alca(lado) {
                            let ph = tela.p(h.x as f32, h.y as f32);
                            hastes.push(vec![pa, ph]);
                            pontos_das_alcas.push(ph);
                        }
                    }
                }
            }
        }
        if !hastes.is_empty() {
            v.push(traco(hastes, 1.0, cor));
        }
        for (x, y) in pontos_das_alcas {
            v.push(
                div()
                    .absolute()
                    .left(px(x - RAIO_DA_ALCA))
                    .top(px(y - RAIO_DA_ALCA))
                    .size(px(RAIO_DA_ALCA * 2.0))
                    .rounded_full()
                    .bg(cor)
                    .into_any_element(),
            );
        }
        // As âncoras.
        let mostrar_ancoras = self.vetorial.is_some() || !componentes.is_empty();
        if mostrar_ancoras {
            for s in &c.subcaminhos {
                let componente_escolhido = componentes.contains(&s.id);
                if so_componentes && !componente_escolhido {
                    continue;
                }
                for a in &s.ancoras {
                    let r = editor_core::vetor::RefAncora {
                        sub: s.id,
                        ancora: a.id,
                    };
                    let cheia = escolhidos.contains(&r) || (so_componentes && componente_escolhido);
                    let e_ponta = ponta == Some(r);
                    let lado = if e_ponta {
                        LADO_DA_ANCORA + 2.0
                    } else {
                        LADO_DA_ANCORA
                    };
                    let (x, y) = tela.p(a.ponto.x as f32, a.ponto.y as f32);
                    let id = format!("editor-ancora-{}-{}", s.id, a.id);
                    v.push(
                        div()
                            .debug_selector(move || id.clone())
                            .absolute()
                            .left(px(x - lado / 2.0))
                            .top(px(y - lado / 2.0))
                            .size(px(lado))
                            .border_1()
                            .border_color(cor)
                            .bg(if cheia || e_ponta {
                                cor
                            } else {
                                gpui_kit::white()
                            })
                            .into_any_element(),
                    );
                }
            }
        }
        // O retângulo de seleção.
        if let Some((a, b)) = sessao.caneta.retangulo() {
            v.push(tela.contorno(vec![
                (a.x as f32, a.y as f32),
                (b.x as f32, a.y as f32),
                (b.x as f32, b.y as f32),
                (a.x as f32, b.y as f32),
                (a.x as f32, a.y as f32),
            ]));
        }
        let _ = f;
        if let Some(acao) = self.indicacao_da_caneta(window.modifiers()) {
            v.extend(self.selo(acao));
        }
        v
    }

    /// O selo da ação ao lado do ponteiro (o que o cursor do Photoshop diz
    /// com o sinal junto da caneta).
    fn selo(&self, acao: Acao) -> Option<AnyElement> {
        let (texto, dica): (&str, &str) = match acao {
            Acao::NovoComponente => ("✱", "novo componente"),
            Acao::Fechar { .. } => ("○", "fechar"),
            Acao::Retomar { .. } => ("⟋", "continuar daqui"),
            Acao::Unir { .. } => ("⊸", "unir"),
            Acao::Adicionar { .. } => ("+", "adicionar ponto"),
            Acao::Excluir(_) => ("−", "excluir ponto"),
            Acao::Converter(_) | Acao::AjustarUltima { .. } => ("⌃", "converter ponto"),
            Acao::MoverAlca { .. } => ("◦", "mover alça"),
            Acao::MoverAncoras(_) => ("▪", "mover ponto"),
            Acao::MoverComponente(_) => ("▣", "mover componente"),
            Acao::DobrarSegmento { .. } => ("◠", "dobrar a curva"),
            Acao::Continuar { .. } | Acao::Retangulo | Acao::Nada | Acao::Encerrar => return None,
        };
        let ponteiro = self.ponteiro? - self.palco.origin;
        let dica = dica.to_string();
        Some(
            div()
                .debug_selector(move || format!("editor-indicacao-da-caneta {dica}"))
                .absolute()
                .left(ponteiro.x + px(10.))
                .top(ponteiro.y + px(8.))
                .px(px(4.))
                .rounded(crate::tema::canto(3.))
                .bg(gpui_kit::black().opacity(0.75))
                .text_color(gpui_kit::white())
                .text_xs()
                .child(texto.to_string())
                .into_any_element(),
        )
    }

    /// O slider da densidade (0–100) ou da difusão (px) da máscara vetorial
    /// da camada escolhida andou; soltar vira um passo.
    pub fn mover_propriedade_vetorial(
        &mut self,
        e_densidade: bool,
        valor: f32,
        soltou: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(s) = self.sessao_mut() {
            let i = s.ativa();
            if e_densidade {
                s.mover_propriedades_da_mascara_vetorial(i, Some(valor / 100.0), None);
            } else {
                s.mover_propriedades_da_mascara_vetorial(i, None, Some(valor));
            }
            if soltou {
                s.confirmar_mascara_vetorial();
            }
        }
        cx.notify();
    }

    /// As Propriedades da máscara vetorial (a escolhida no painel Caminhos ou
    /// pela miniatura): densidade, difusão, ligar, vincular, excluir.
    pub(super) fn propriedades_da_mascara_vetorial(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let s = self.sessao()?;
        let i = s.ativa();
        if s.alvo_vetorial() != Some(LugarDoCaminho::Mascara(i)) {
            return None;
        }
        let camada = s.camada_ativa();
        let m = camada.mascara_vetorial.as_ref()?;
        let tema = cx.theme().clone();
        let rotulo = |nome: &'static str, valor: String| {
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(nome)
                .child(valor)
        };
        let botao = |id: &'static str, texto: &'static str, dica: &'static str| {
            crate::estilo::botao_secundario_pequeno(id, cx)
                .debug_selector(move || id.into())
                .label(texto)
                .tooltip(dica)
        };
        Some(
            div()
                .debug_selector(|| "editor-propriedades-da-mascara-vetorial".into())
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(format!("Máscara vetorial de {}", camada.nome)),
                )
                .child(rotulo("Densidade", format!("{:.0}%", m.densidade * 100.0)))
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(|| "editor-vetorial-densidade".into())
                        .child(crate::estilo::slider(&self.densidade_vetorial)),
                )
                .child(rotulo("Difusão", format!("{:.1} px", m.difusao)))
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(|| "editor-vetorial-difusao".into())
                        .child(crate::estilo::slider(&self.difusao_vetorial)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(
                            botao(
                                "editor-vetorial-ligar",
                                if m.ativa { "Desligar" } else { "Ligar" },
                                "Desligar ou ligar a máscara vetorial (⇧ + clique na miniatura) — o caminho fica",
                            )
                            .on_click(cx.listener(move |ed, _, _, cx| {
                                ed.na_sessao(cx, |s| {
                                    s.alternar_mascara_vetorial(i);
                                })
                            })),
                        )
                        .child(
                            botao(
                                "editor-vetorial-vinculo",
                                if m.vinculada { "Soltar" } else { "Vincular" },
                                "Com a corrente, o Mover e o ⌘T levam o caminho junto com a camada",
                            )
                            .on_click(cx.listener(move |ed, _, _, cx| {
                                ed.na_sessao(cx, |s| {
                                    s.alternar_vinculo_vetorial(i);
                                })
                            })),
                        )
                        .child(
                            botao("editor-vetorial-excluir", "Excluir", "Excluir a máscara vetorial")
                                .on_click(cx.listener(move |ed, _, _, cx| {
                                    ed.na_sessao(cx, |s| {
                                        s.excluir_mascara_vetorial(i);
                                    })
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    // ------------------------------------------------- o painel Caminhos

    /// O painel Caminhos do Photoshop: o caminho de trabalho (em itálico,
    /// provisório), os nomeados e a máscara vetorial da camada escolhida.
    /// Clicar escolhe (e mostra); clicar de novo no escolhido o esconde;
    /// duplo clique renomeia (no de trabalho, salva). No rodapé: preencher,
    /// contornar, fazer seleção, máscara vetorial, novo e excluir.
    pub(super) fn painel_de_caminhos(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let c = aparencia::cores(cx);
        let Some(s) = self.sessao() else {
            return div().into_any_element();
        };
        let alvo = s.alvo_vetorial();
        let doc = s.documento();
        let mut itens: Vec<(LugarDoCaminho, String, bool)> = Vec::new();
        if doc.caminhos.trabalho.is_some() {
            itens.push((
                LugarDoCaminho::Trabalho,
                editor_core::vetor::NOME_DO_TRABALHO.into(),
                true,
            ));
        }
        for n in &doc.caminhos.nomeados {
            itens.push((LugarDoCaminho::Nomeado(n.id), n.nome.clone(), false));
        }
        let ativa = s.ativa();
        if doc
            .camadas
            .get(ativa)
            .is_some_and(|c| c.mascara_vetorial.is_some())
        {
            itens.push((
                LugarDoCaminho::Mascara(ativa),
                s.nome_do_caminho(LugarDoCaminho::Mascara(ativa)),
                true,
            ));
        }
        let linhas = itens
            .into_iter()
            .enumerate()
            .map(|(k, (lugar, nome, italico))| {
                let escolhido = alvo == Some(lugar);
                let id = SharedString::from(format!("editor-caminho-{k}"));
                div()
                    .id(id.clone())
                    .debug_selector(move || id.to_string())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(6.))
                    .py(px(3.))
                    .rounded(crate::tema::canto(4.))
                    .cursor_pointer()
                    .when(escolhido, |d| {
                        d.bg(tema.accent).text_color(tema.accent_foreground)
                    })
                    .when(!escolhido, |d| d.hover(|d| d.bg(tema.muted)))
                    .child(
                        gpui_kit::component::Icon::new(
                            if matches!(lugar, LugarDoCaminho::Mascara(_)) {
                                Icone::LayerMask
                            } else {
                                Icone::Spline
                            },
                        )
                        .size_4(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_sm()
                            .when(italico, |d| d.italic())
                            .child(nome),
                    )
                    .on_click(
                        cx.listener(move |ed, e: &gpui_kit::ClickEvent, window, cx| {
                            if e.click_count() >= 2 && !matches!(lugar, LugarDoCaminho::Mascara(_))
                            {
                                let renomear = (lugar != LugarDoCaminho::Trabalho).then_some(lugar);
                                ed.abrir_nome_do_caminho(renomear, window, cx);
                                return;
                            }
                            let ja = ed.sessao().and_then(Sessao::alvo_vetorial) == Some(lugar);
                            ed.escolher_caminho_no_painel(if ja { None } else { Some(lugar) }, cx);
                            window.focus(&ed.foco, cx);
                        }),
                    )
            });
        let tem_alvo = alvo.is_some();
        let tem_trabalho = doc.caminhos.trabalho.is_some();
        let tem_area = s.caminho_alvo().is_some_and(|c| c.tem_area());
        let botao = |id: &'static str, icone: Icone, dica: &'static str, ligado: bool| {
            crate::estilo::botao_icone_pequeno(id, icone)
                .debug_selector(move || id.into())
                .tooltip(na_plataforma(dica))
                .disabled(!ligado)
        };
        let ed = cx.entity();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            .child(
                div()
                    .id("editor-caminhos")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(4.))
                    .py(px(2.))
                    .gap(px(1.))
                    .children(linhas)
                    .child(
                        // O vazio do painel: clicar esconde o caminho, como no
                        // Photoshop.
                        div()
                            .id("editor-caminhos-vazio")
                            .debug_selector(|| "editor-caminhos-vazio".into())
                            .flex_1()
                            .min_h(px(24.))
                            .when(!tem_alvo && !tem_trabalho && doc.caminhos.nomeados.is_empty(), |d| {
                                d.p(px(6.))
                                    .text_xs()
                                    .text_color(c.apagado)
                                    .child("Desenhe com a Caneta (P): o caminho de trabalho aparece aqui.")
                            })
                            .on_click(cx.listener(|ed, _, window, cx| {
                                ed.escolher_caminho_no_painel(None, cx);
                                window.focus(&ed.foco, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(2.))
                    .px(px(4.))
                    .py(px(3.))
                    .border_t_1()
                    .border_color(c.borda)
                    .child(
                        botao("editor-caminho-preencher", Icone::PaintBucket, "Preencher o caminho com a cor de frente (na camada escolhida, dentro da seleção)", tem_area)
                            .on_click(cx.listener(|ed, _, _, cx| ed.preencher_caminho(cx))),
                    )
                    .child(
                        botao("editor-caminho-contornar", Icone::Paintbrush, "Contornar o caminho com o pincel de agora", tem_alvo)
                            .on_click(cx.listener(|ed, _, _, cx| ed.contornar_caminho(cx))),
                    )
                    .child(
                        botao("editor-caminho-selecao", Icone::CircleDashed, "Carregar o caminho como seleção (⌘↵)", tem_area)
                            .on_click(cx.listener(|ed, _, _, cx| ed.fazer_selecao_do_caminho(cx))),
                    )
                    .child(
                        botao("editor-caminho-nova-mascara", Icone::LayerMask, "Adicionar máscara vetorial à camada escolhida", true)
                            .on_click(cx.listener(|ed, _, _, cx| ed.criar_mascara_vetorial(cx))),
                    )
                    .child(
                        botao("editor-caminho-novo", Icone::Plus, "Novo caminho", true)
                            .on_click(cx.listener(|ed, _, _, cx| {
                                ed.na_sessao(cx, |s| {
                                    s.novo_caminho();
                                })
                            })),
                    )
                    .child(
                        botao("editor-caminho-mais", Icone::EllipsisVertical, "Mais: salvar, duplicar, renomear, máscara vetorial", true)
                            .dropdown_menu_with_anchor(gpui_kit::Anchor::BottomLeft, move |menu, _w, _cx| {
                                let item = |id: &'static str, rotulo: &'static str, ligado: bool, fazer: fn(&mut EditorDeFoto, &mut Window, &mut Context<EditorDeFoto>)| {
                                    let ed = ed.clone();
                                    crate::estilo::item_de_menu(id, na_plataforma(rotulo), None)
                                        .disabled(!ligado)
                                        .on_click(move |_ev, window, cx| ed.update(cx, |ed, cx| fazer(ed, window, cx)))
                                };
                                menu.item(item("editor-caminho-salvar", "Salvar caminho…", tem_trabalho, |ed, w, cx| ed.abrir_nome_do_caminho(None, w, cx)))
                                    .item(item("editor-caminho-duplicar", "Duplicar caminho", tem_alvo, |ed, _, cx| {
                                        ed.na_sessao(cx, |s| {
                                            if let Some(l) = s.alvo_vetorial() {
                                                s.duplicar_caminho(l);
                                            }
                                        })
                                    }))
                                    .item(item("editor-caminho-renomear", "Renomear…", tem_alvo, |ed, w, cx| {
                                        let alvo = ed.sessao().and_then(Sessao::alvo_vetorial);
                                        match alvo {
                                            Some(LugarDoCaminho::Trabalho) => ed.abrir_nome_do_caminho(None, w, cx),
                                            Some(LugarDoCaminho::Mascara(_)) | None => {}
                                            Some(l) => ed.abrir_nome_do_caminho(Some(l), w, cx),
                                        }
                                    }))
                                    .item(item("editor-caminho-fazer-selecao-menu", "Fazer seleção…", tem_area, |ed, w, cx| ed.abrir_fazer_selecao(w, cx)))
                                    .item(item("editor-caminho-ocultar", "Ocultar o caminho  ⇧⌘H", tem_alvo, |ed, _, cx| ed.escolher_caminho_no_painel(None, cx)))
                                    .separator()
                                    .item(item("editor-caminho-mascara-ligar", "Ligar/desligar a máscara vetorial", true, |ed, _, cx| {
                                        ed.na_sessao(cx, |s| {
                                            let i = s.ativa();
                                            s.alternar_mascara_vetorial(i);
                                        })
                                    }))
                                    .item(item("editor-caminho-mascara-vinculo", "Vincular/soltar a máscara vetorial", true, |ed, _, cx| {
                                        ed.na_sessao(cx, |s| {
                                            let i = s.ativa();
                                            s.alternar_vinculo_vetorial(i);
                                        })
                                    }))
                                    .item(item("editor-caminho-mascara-excluir", "Excluir a máscara vetorial", true, |ed, _, cx| {
                                        ed.na_sessao(cx, |s| {
                                            let i = s.ativa();
                                            s.excluir_mascara_vetorial(i);
                                        })
                                    }))
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        botao("editor-caminho-excluir", Icone::Trash2, "Excluir o caminho escolhido", tem_alvo)
                            .on_click(cx.listener(|ed, _, _, cx| {
                                ed.na_sessao(cx, |s| {
                                    if let Some(l) = s.alvo_vetorial() {
                                        s.excluir_caminho(l);
                                    }
                                })
                            })),
                    ),
            )
            .into_any_element()
    }
}

/// Polilinhas da tela (pontos do palco) num traço só, recortado ao palco.
fn traco(linhas: Vec<Vec<(f32, f32)>>, largura: f32, cor: gpui_kit::Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |limites, _, window, _| {
            let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
            let mut caminho = PathBuilder::stroke(px(largura.max(0.5)));
            let mut algum = false;
            for linha in &linhas {
                let Some(primeiro) = linha.first() else {
                    continue;
                };
                caminho.move_to(gpui_kit::point(px(ox + primeiro.0), px(oy + primeiro.1)));
                for p in &linha[1..] {
                    caminho.line_to(gpui_kit::point(px(ox + p.0), px(oy + p.1)));
                }
                algum = true;
            }
            if !algum {
                return;
            }
            if let Ok(c) = caminho.build() {
                // 🔑 O caminho não é recortado pelo pai (a lição das guias).
                window.with_content_mask(Some(gpui_kit::ContentMask { bounds: limites }), |w| {
                    w.paint_path(c, cor)
                });
            }
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}
