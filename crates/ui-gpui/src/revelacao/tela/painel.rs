//! A coluna da direita da Revelação — o porte de `revelacao/paineis.tsx`.
//!
//! De cima para baixo, como no site: o aviso de foto comprada, o cabeçalho
//! ("N ajustes fora do neutro" e "Zerar tudo"), os painéis sanfonados e o
//! rodapé que diz de que tamanho é a
//! cópia que a tela edita.
//!
//! ⚠️ **Por cima dela, fora da rolagem, fica o histograma**, que o site não
//! tem. Ele é recolhível (dono, 2026-09-17: *"precisam ter como recolher e o
//! espectograma invade o botão salvar na galeria"*) e tem altura natural: o
//! bloco de altura fixa de antes tinha mais conteúdo que altura, e o que
//! sobrava era desenhado por cima da barra do topo.
//!
//! 🗑️ **A "Curva resultante" saiu em 2026-10-01** (dono: *"Não precisamos
//! mais desse painel"*): a Curva de tons do Lightroom já mostra a curva, com o
//! histograma por trás. E o "Tom automático" que morava junto do histograma
//! virou o "Automático" do alto do Básico, onde o Lightroom o põe.
//!
//! # O que fica lembrado entre sessões
//!
//! Qual painel está aberto e se os gráficos estão à mostra — com as chaves do
//! site (`revelacao:<título>`), num arquivo ao lado do catálogo. É preferência de
//! quem opera o balcão, como a altura da tira: falhar ao ler ou gravar nunca
//! interrompe nada.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::accordion::Accordion;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{h_flex, ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::{
    canvas, div, prelude::*, px, relative, AnyElement, Bounds, Context, Empty, MouseButton,
    MouseUpEvent, Pixels, SharedString, StyleRefinement, Window,
};

mod basico;
mod curva_de_tons;

pub(crate) use curva_de_tons::ModoDaCurva;

use super::{Controle, PedidoDaRevelacao, Revelacao};
use crate::recursos::Icone;
use crate::revelacao::controles::{self, Painel, Secao};
use crate::revelacao::curva::{self, Canal, PONTOS_DA_CURVA};
use crate::revelacao::persistencia::Corte;
use crate::revelacao::processador::Ajustes;
use crate::revelacao::rodas::{self, Faixa, Vista};
use crate::tema;

/// A chave da aba escolhida — a do site.
/// A do histograma, que só o desktop tem.
const CHAVE_DO_HISTOGRAMA: &str = "revelacao:histograma";

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
    /// O que a Curva de tons mostra: a paramétrica ou um canal da curva por
    /// ponto. Como a aba do HSL, não é lembrado entre sessões.
    modo: ModoDaCurva,
    /// A região sob o ponteiro (no gráfico ou num slider de região) — a faixa
    /// sombreada e o rótulo do canto.
    regiao_em_foco: Option<curva::Regiao>,
    /// O arrasto vertical em curso no gráfico paramétrico.
    arrasto_da_regiao: Option<curva_de_tons::ArrastoDeRegiao>,
    /// O pino da barra de divisão que o ponteiro pegou.
    pino_arrastado: Option<usize>,
    /// O nó que o ponteiro pegou no último clique.
    no_arrastado: Option<usize>,
    /// Onde o gráfico da curva foi desenhado no último quadro — o clique chega
    /// em coordenada de janela.
    area_da_curva: Rc<Cell<Bounds<Pixels>>>,
    /// Onde a barra de divisão foi desenhada no último quadro.
    area_da_barra: Rc<Cell<Bounds<Pixels>>>,
    /// A rolagem da coluna, para o roteiro de depuração.
    rolagem: gpui_kit::ScrollHandle,
    /// O que o "Ajustar" da Correção de cores mostra. Como a aba do HSL, não
    /// é lembrado entre sessões, só entre fotos.
    pub(super) vista_das_rodas: Vista,
    /// Onde cada roda foi desenhada no último quadro.
    pub(super) areas_das_rodas: Rc<RefCell<HashMap<Faixa, Bounds<Pixels>>>>,
    /// O arrasto em curso numa roda.
    pub(super) arrasto_da_roda: Option<rodas::Arrasto>,
    /// O olho apertado: a prévia sai sem esta parte da Correção de cores.
    pub(super) ver_sem: Option<super::correcao_de_cores::VerSem>,
    /// 🎞️ O conta-gotas do Básico armado: o próximo clique na foto escolhe o
    /// ponto neutro. Não atravessa a troca de foto.
    pub(super) conta_gotas: bool,
    /// O par (temperatura, colorir) que o "Automático" do EB escolheu nesta
    /// foto — para a lista dizer "Automático" enquanto ninguém mexer nele.
    pub(super) balanco_automatico: Option<(f32, f32)>,
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
            modo: ModoDaCurva::Parametrica,
            regiao_em_foco: None,
            arrasto_da_regiao: None,
            pino_arrastado: None,
            no_arrastado: None,
            area_da_curva: Rc::new(Cell::new(Bounds::default())),
            area_da_barra: Rc::new(Cell::new(Bounds::default())),
            rolagem: gpui_kit::ScrollHandle::new(),
            vista_das_rodas: Vista::default(),
            areas_das_rodas: Rc::new(RefCell::new(HashMap::new())),
            arrasto_da_roda: None,
            ver_sem: None,
            conta_gotas: false,
            balanco_automatico: None,
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
    pub(super) fn aberto(&self, chave: &str, padrao: bool) -> bool {
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
            crate::telemetria::avisar!(
                "⚠️ [Revelação] a arrumação dos painéis não foi gravada: {erro}"
            );
        }
    }

    /// Abre, se estiver fechado — sem regravar o que já está aberto.
    pub(super) fn abrir(&mut self, chave: &str) {
        if !self.aberto(chave, false) {
            self.definir(chave, true);
        }
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
        perspectiva: Some(Default::default()),
        restringir: Some(true),
    }
}

/// A arte que acompanha o ponteiro no arrasto de um nó: nenhuma.
pub(super) struct SemFantasma;

impl Render for SemFantasma {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// O tipo do arrasto de um nó da curva.
#[derive(Clone, Copy)]
struct ArrastoDoNo;

/// O ponto de um conjunto de controles — âmbar é "mudou e não salvou", cinza é
/// "tem ajuste, já salvo" (`Marca` e o `<Marca>` do site).
/// 🎞️ O título de um grupo dentro do painel ("Vinheta de corte posterior",
/// "Granulado"): pequeno, apagado e no meio, como no Lightroom. Do segundo em
/// diante, uma linha de ponta a ponta o separa do grupo de cima.
fn titulo_do_grupo(secao: Secao, separar: bool, cx: &gpui_kit::App) -> AnyElement {
    titulo(secao.rotulo(), separar, cx)
}

/// O mesmo título com um nome livre — o "Tom" e o "Presença" do Básico, que
/// não são família de controle.
fn titulo(rotulo: &'static str, separar: bool, cx: &gpui_kit::App) -> AnyElement {
    div()
        .debug_selector(move || format!("grupo-{rotulo}"))
        .flex()
        .justify_center()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .when(separar, |d| {
            // O painel tem 12 px de respiro dos lados: a linha os atravessa.
            d.mx(px(-12.))
                .px(px(12.))
                .mt(px(4.))
                .pt(px(10.))
                .border_t_1()
                .border_color(cx.theme().border)
        })
        .child(rotulo)
        .into_any_element()
}

/// O subtítulo dentro de um grupo — o "posição / forma" do módulo de
/// vinheta do darktable: menor que o título e sem a linha, porque o módulo é
/// o mesmo.
fn subtitulo(rotulo: &'static str, cx: &gpui_kit::App) -> AnyElement {
    div()
        .debug_selector(move || format!("grupo-{rotulo}"))
        .mt(px(4.))
        .text_xs()
        .text_color(cx.theme().muted_foreground.opacity(0.8))
        .child(rotulo)
        .into_any_element()
}

/// O "Ligar" de um módulo: o interruptor que vira a chave do título.
fn e_a_chave_do_modulo(controle: &Controle) -> bool {
    controle.definicao.discreto
        && controle.escolha.is_none()
        && controle.definicao.rotulo == "Ligar"
}

/// A aba do espaço em que a foto é revelada: **Adobe RGB**, e só ela.
///
/// 🔑 **Dono, 2/out/2026: a aba sRGB vira Adobe RGB, e o sRGB sai da tela.**
/// Até a 0.1.64 eram duas, sRGB e RGB (os módulos do darktable); o darktable
/// saiu do motor, e a foto passou a ser lida no espaço que a câmera grava
/// (`foto_codec::espaco_de_cor`). A aba fica, sozinha, para dizer isso.
fn aba_de_espaco() -> AnyElement {
    TabBar::new("abas-de-espaco")
        .segmented()
        .xsmall()
        .w_full()
        .selected_index(0)
        .child(
            Tab::new()
                .label("Adobe RGB")
                .flex_1()
                .debug_selector(|| "aba-espaco-Adobe RGB".to_string())
                .tooltip(|window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(
                        "Os controles do Lightroom, sobre a foto no espaço que a câmera gravou",
                    )
                    .build(window, cx)
                }),
        )
        .into_any_element()
}

pub(super) fn ponto(marca: controles::Marca, cx: &gpui_kit::App) -> gpui_kit::Div {
    let cor = match marca {
        controles::Marca::NaoSalvo => tema::cores::quente(),
        controles::Marca::Ajustado => cx.theme().muted_foreground.opacity(0.5),
    };
    div().size(px(6.)).flex_none().rounded_full().bg(cor)
}

impl Revelacao {
    /// Os controles aceitam gesto? Só com a foto carregada **e** revelável — o
    /// `desabilitado={!pronto || !podeRevelar}` do site. Uma foto do site
    /// comprada (ou apagada) não se revela: o cliente pode já ter baixado o
    /// original (`Revelacao::pode_revelar`).
    pub(super) fn controles_ligados(&self) -> bool {
        self.tem_pixels() && self.pode_revelar()
    }

    /// Quantos campos do motor estão fora do neutro — **os 193**, como o
    /// cabeçalho do site conta.
    pub(super) fn quantos_alterados(&self) -> usize {
        controles::quantos_fora_do_neutro(&self.ajustes)
    }

    /// Se alguma coisa desta família saiu do neutro.
    #[cfg(test)]
    pub(super) fn secao_alterada(&self, secao: Secao) -> bool {
        controles::secao_alterada(&self.ajustes, secao)
    }

    /// A revelação com que a foto abriu — o que a galeria tem, e é contra ela
    /// que o ponto âmbar compara (o marco do site).
    pub(super) fn salvo(&self) -> Ajustes {
        self.parametros_ao_abrir
            .as_ref()
            .map(|e| e.ajustes)
            .unwrap_or(self.ajustes)
    }

    /// O ponto da Revelação local, pela mesma regra dos painéis: âmbar se as
    /// máscaras ou os retoques mudaram desde a abertura; cinza se a revelação
    /// salva já tem algum.
    pub(super) fn marca_da_revelacao_local(&self) -> Option<controles::Marca> {
        let salvo = self.parametros_ao_abrir.as_ref().map(|e| &*e.locais);
        if salvo.is_some_and(|l| *l != *self.locais) {
            return Some(controles::Marca::NaoSalvo);
        }
        (!self.locais.vazia()).then_some(controles::Marca::Ajustado)
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

    /// **Zerar tudo**: os 193 ao neutro **e** o enquadramento à foto inteira.
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

        // 🔑 Zerar põe a foto no processo atual, como o "Redefinir" do
        // Lightroom: no neutro os dois processos dão a mesma foto, e o que o
        // operador mexer daqui em diante já sai com a conta do Lightroom.
        self.ajustes = Ajustes {
            processo: 1.0,
            ..Ajustes::default()
        };
        self.corte = corte_inteiro();
        // 🔑 **A Revelação local vai junto**, como o "Redefinir" do Lightroom
        // — um passo só no histórico. A ilegível fica: ela nunca é
        // sobrescrita (ver `locais_ilegiveis`).
        if self.locais_ilegiveis.is_none() {
            self.locais = Default::default();
            self.local.selecionado = None;
            self.local.mascara_sel = None;
        }
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        // O corte não passa pela GPU: quem o mostra é a exibição.
        self.atualizar_exibicao();
        self.historico.registrar(self.estado());
        self.gravar();
        // 🚨 **A prévia local vai junto.** Ela é o que a grade e a tira mostram
        // enquanto a foto não sobe; deixá-la aqui faria o "Zerar tudo" mudar o
        // palco e não mudar a miniatura ao lado dele — a mesma foto com duas
        // caras, agora ao contrário (dono, 18/set/2026). Neutro apaga: ver
        // `guardar_a_revelada_no_cache`.
        self.guardar_a_revelada_no_cache();
        if let Some(id) = self.foto_aberta().map(|f| f.id.clone()) {
            self.esquecer_a_miniatura_da_tira(&id);
        }
        cx.notify();
    }

    /// O botão "Zerar tudo" (ou "Zerar N fotos"): esta foto pelo histórico, e
    /// as outras marcadas pela raiz.
    pub(crate) fn zerar_tudo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.controles_ligados()
            && (self.quantos_alterados() > 0 || self.enquadrada() || self.tem_revelacao_local())
        {
            self.redefinir_ajustes(window, cx);
        }
        if !self.outras_a_zerar().is_empty() {
            cx.emit(PedidoDaRevelacao::ZerarAsMarcadas);
        }
    }

    /// Um gesto discreto sobre os ajustes — o duplo clique num nó, o "Zerar"
    /// de um canal. O mesmo caminho de `devolver_ao_neutro`: fecha o pendente,
    /// muda, vira um passo e grava na hora. Sem mudança, não escreve nada.
    pub(super) fn gesto_discreto(
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
        let ModoDaCurva::Ponto(canal) = self.estado_do_painel.modo else {
            return;
        };
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
            "abrir" | "fechar" => match Painel::TODOS.into_iter().find(|p| p.rotulo() == resto) {
                Some(painel) => self
                    .estado_do_painel
                    .definir(&painel.chave(), comando == "abrir"),
                None => eprintln!("[roteiro] painel desconhecido: '{resto}'"),
            },
            "rolar" => {
                let rolagem = &self.estado_do_painel.rolagem;
                let alvo = match resto {
                    "fim" => f32::from(rolagem.max_offset().y),
                    numero => numero.parse().unwrap_or(0.0),
                };
                rolagem.set_offset(gpui_kit::point(px(0.), px(-alvo)));
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
            corpo.push(aba_de_espaco());
            let paineis: &[Painel] = &Painel::ADOBE_RGB;
            // A Revelação local vem logo abaixo do primeiro painel (o Básico),
            // como no Lightroom — recolhida por padrão.
            // 🎞️ **O P&B toma o lugar do HSL**, como no Lightroom: com a foto
            // em preto e branco não há cor para o HSL mexer, e o que sobra é a
            // Mistura de preto e branco. Desligado, o P&B some.
            let pb = self.ajustes.bw_ativo != 0.0;
            let paineis = paineis.iter().filter(|p| match p {
                Painel::Hsl => !pb,
                Painel::PretoEBranco => pb,
                _ => true,
            });
            for (i, painel) in paineis.enumerate() {
                corpo.push(self.painel_sanfonado(*painel, cx));
                if i == 0 {
                    corpo.push(self.painel_local(cx));
                }
            }
        }
        let rodape = self.rodape(cx);
        let graficos = self.painel_dos_graficos(cx);
        let zerar = (!enquadrando).then(|| self.rodape_dos_ajustes(cx));

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
                    .gap(px(if tema::cores::paineis_corridos() {
                        2.
                    } else {
                        16.
                    }))
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
            .children(zerar)
    }

    /// "Esta foto foi **comprada**…" — o aviso do site, no alto da coluna.
    fn aviso_de_comprada(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.aberta.is_none() || self.pode_revelar() {
            return None;
        }
        Some(
            div()
                .rounded(crate::tema::canto(4.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .p(px(8.))
                .text_size(crate::tema::letra::em(11.))
                .line_height(px(15.))
                .text_color(cx.theme().muted_foreground)
                .child(
                    gpui_kit::StyledText::new(
                        "Esta foto foi comprada. O cliente já pode ter baixado o original, \
                         então ela não se revela — dá para ver e baixar, não para salvar.",
                    )
                    .with_highlights([(
                        14..22,
                        gpui_kit::HighlightStyle {
                            color: Some(cx.theme().foreground),
                            font_weight: Some(gpui_kit::FontWeight::BOLD),
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
                .text_size(crate::tema::letra::em(11.))
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

    /// A foto aberta tem máscara ou retoque (a ilegível conta: ela existe).
    pub(super) fn tem_revelacao_local(&self) -> bool {
        !self.locais.camadas.is_empty()
            || !self.locais.retoques.is_empty()
            || self.locais_ilegiveis.is_some()
    }

    /// Quantos ajustes estão fora do neutro, e o botão que devolve todos.
    ///
    /// 🔑 **No rodapé da coluna, fixo, como o "Redefinir" do Lightroom**
    /// (dono, 2/out/2026: no topo poluía). Fora da rolagem e fora dos painéis
    /// sanfonados — Zerar tudo é o gesto de "recomeçar" e não pertence a
    /// nenhuma seção. O mesmo desenho do site.
    fn rodape_dos_ajustes(&self, cx: &mut Context<Self>) -> AnyElement {
        let alterados = self.quantos_alterados();
        let enquadrada = self.aberta.is_some() && self.enquadrada();
        let locais = self.aberta.is_some() && self.tem_revelacao_local();
        let texto = texto_do_cabecalho(alterados, enquadrada, locais);
        // 🚨 **O botão não pode decidir sozinho se há o que zerar.** Olhando só
        // a foto no palco, ele se apagava com ela no neutro — mesmo com quatro
        // marcadas atrás cheias de ajuste (dono, 2026-09-11).
        let outras = self.outras_a_zerar().len();
        let tem_o_que_zerar = alterados > 0 || enquadrada || locais;
        let quantas_fotos = usize::from(tem_o_que_zerar && self.controles_ligados()) + outras;
        let dica = if outras > 0 {
            format!(
                "Devolve ao neutro {quantas_fotos} foto(s) — esta e as marcadas na tira —, \
                 inclusive o enquadramento de cada uma."
            )
        } else {
            "Devolve os 53 ajustes ao neutro, o enquadramento à foto inteira e tira as \
             máscaras e os retoques."
                .to_string()
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .border_t_1()
            .border_color(cx.theme().border)
            .text_size(crate::tema::letra::em(11.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(texto)),
            )
            .child(
                Button::new("zerar-tudo")
                    .icon(Icon::new(Icone::RotateCcw).size(px(12.)))
                    .label(if quantas_fotos > 1 {
                        SharedString::from(format!("Zerar {quantas_fotos} fotos"))
                    } else {
                        SharedString::from("Zerar tudo")
                    })
                    .ghost()
                    .xsmall()
                    .disabled(quantas_fotos == 0)
                    .tooltip(dica)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.zerar_tudo(window, cx))),
            )
            .into_any_element()
    }

    /// Uma sanfona — o `PainelColapsavel` do site, no `Accordion` do gpui-kit.
    ///
    /// Aberta ou fechada é o `estado_do_painel` que diz (lembrado entre
    /// sessões); `dentro` só vem quando está aberta.
    ///
    /// ⚠️ **O `on_toggle_click` do kit escuta o clique no acordeão inteiro**,
    /// e não só no cabeçalho: um clique num slider também o chama, com o mesmo
    /// conjunto de abertos. Por isso o estado se grava pelo que o kit diz
    /// (aberto ou não), e só quando muda — alternar a cada chamada fecharia o
    /// painel no primeiro arrasto, e gravar sempre escreveria o arquivo a cada
    /// clique.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn sanfona(
        &self,
        titulo: &'static str,
        chave: String,
        padrao: bool,
        marca: Option<controles::Marca>,
        borda: bool,
        dentro: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let aberto = self.estado_do_painel.aberto(&chave, padrao);
        let id = format!("sanfona-{chave}");
        let tela = cx.entity().downgrade();
        // 🎞️ No Lightroom os painéis são faixas corridas: sem moldura, o título
        // encostado à direita, junto da seta.
        let corridos = tema::cores::paineis_corridos();
        let borda = borda && !corridos;
        Accordion::new(SharedString::from(id.clone()))
            .xsmall()
            .bordered(borda)
            .h_auto()
            .flex_none()
            .when(borda, |a| a.rounded(crate::tema::canto(6.)))
            .item(|item| {
                // 🎞️ No Lightroom o cabeçalho do painel tem fundo próprio.
                let mut cabecalho = StyleRefinement::default().px(px(12.)).py(px(8.));
                if let Some(fundo) = tema::cores::sanfona() {
                    cabecalho = cabecalho.bg(fundo);
                }
                item.open(aberto)
                    .title_style(cabecalho)
                    .content_style(StyleRefinement::default().p(px(0.)))
                    .title(
                        h_flex()
                            .debug_selector(move || id)
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(8.))
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            // 🎞️ O título do painel do Lightroom é maior que
                            // o corpo, e sem negrito.
                            .when(corridos, |c| {
                                c.justify_end()
                                    .text_sm()
                                    .font_weight(gpui_kit::FontWeight::NORMAL)
                            })
                            .when_some(marca.filter(|_| corridos), |c, m| c.child(ponto(m, cx)))
                            .child(
                                div()
                                    .when(!corridos, |d| d.flex_1())
                                    .min_w(px(0.))
                                    .truncate()
                                    .child(titulo),
                            )
                            .when_some(marca.filter(|_| !corridos), |c, m| c.child(ponto(m, cx))),
                    )
                    .children(dentro)
            })
            .on_toggle_click(move |abertos, _window, cx| {
                let quer = abertos.contains(&0);
                let _ = tela.update(cx, |tela, cx| {
                    if tela.estado_do_painel.aberto(&chave, padrao) != quer {
                        tela.estado_do_painel.definir(&chave, quer);
                        cx.notify();
                    }
                });
            })
            .text_color(cx.theme().foreground)
            .into_any_element()
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
        let marca = controles::marca_do_painel(&self.ajustes, &self.salvo(), painel);

        let conteudo = aberto.then(|| {
            let mut dentro: Vec<AnyElement> = Vec::new();
            match painel {
                Painel::Basico => dentro.extend(self.painel_basico(cx)),
                Painel::CurvaDeTons => dentro.extend(self.painel_da_curva_de_tons(cx)),
                Painel::Tonalizacao => dentro.push(self.correcao_de_cores(cx)),
                Painel::Hsl => {
                    let visivel = self.estado_do_painel.aba_hsl;
                    dentro.push(self.abas(painel.secoes(), visivel, cx));
                    dentro.extend(self.controles_da_secao(visivel, cx));
                }
                _ => {
                    // 🎞️ Painel de mais de um grupo (o Efeitos do Lightroom):
                    // cada um com o seu título. O P&B tem um grupo só, e o
                    // título dele também ("Mistura de preto e branco").
                    let com_titulos = painel.secoes().len() > 1 || painel == Painel::PretoEBranco;
                    for (n, secao) in painel.secoes().iter().enumerate() {
                        match secao {
                            // 🎞️ O módulo do darktable: a chave de ligar no
                            // título, e o "posição / forma" como subtítulo,
                            // sem linha — é o mesmo módulo.
                            Secao::VinhetaDarktable => {
                                dentro.push(self.titulo_da_vinheta_do_darktable(n > 0, cx))
                            }
                            Secao::VinhetaDarktableForma => {
                                dentro.push(subtitulo(secao.rotulo(), cx))
                            }
                            _ if com_titulos => dentro.push(titulo_do_grupo(*secao, n > 0, cx)),
                            _ => {}
                        }
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
                .into_any_element()
        });

        self.sanfona(painel.rotulo(), chave, padrao, marca, true, conteudo, cx)
    }

    /// A fileira de abas do HSL — Cor, Luminância, Matiz.
    ///
    /// ⚠️ **A aba que não está à mostra também precisa se anunciar**: o
    /// sublinhado âmbar na aba fechada (`underline decoration-amber-400`).
    fn abas(&self, secoes: &'static [Secao], visivel: Secao, cx: &mut Context<Self>) -> AnyElement {
        let tela = cx.entity().downgrade();
        TabBar::new("abas-do-hsl")
            .segmented()
            .xsmall()
            .selected_index(secoes.iter().position(|s| *s == visivel).unwrap_or(0))
            .children(secoes.iter().map(|secao| {
                let secao = *secao;
                let escolhida = secao == visivel;
                let marca = controles::marca_da_secao(&self.ajustes, &self.salvo(), secao);
                let rotulo = secao.rotulo();
                Tab::new()
                    .label(Painel::aba(secao))
                    .debug_selector(move || format!("aba-{rotulo}"))
                    .when_some(marca.filter(|_| !escolhida), |aba, m| {
                        aba.underline().text_decoration_color(match m {
                            controles::Marca::NaoSalvo => tema::cores::quente(),
                            controles::Marca::Ajustado => cx.theme().muted_foreground.opacity(0.5),
                        })
                    })
            }))
            .on_click(move |i, _window, cx| {
                let Some(secao) = secoes.get(*i).copied() else {
                    return;
                };
                let _ = tela.update(cx, |tela, cx| {
                    tela.estado_do_painel.aba_hsl = secao;
                    cx.notify();
                });
            })
            .into_any_element()
    }

    /// Os sliders de uma família, na ordem da tabela.
    ///
    /// 🎞️ O Efeitos, o Básico e o P&B já vêm no desenho do Lightroom, uma
    /// linha por controle ([`Self::linha_em_linha`]); os outros painéis ainda
    /// no de duas.
    fn controles_da_secao(&self, secao: Secao, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let darktable = matches!(
            secao,
            Secao::VinhetaDarktable | Secao::VinhetaDarktableForma
        );
        let em_linha = matches!(
            secao.painel(),
            Painel::Efeitos | Painel::Basico | Painel::PretoEBranco
        );
        self.controles
            .iter()
            .enumerate()
            .filter(|(_, controle)| controle.definicao.secao == secao)
            // A chave de ligar mora no título do grupo.
            .filter(|(_, controle)| !(darktable && e_a_chave_do_modulo(controle)))
            .map(|(i, controle)| {
                if darktable {
                    self.linha_do_darktable(i, controle, cx)
                } else if em_linha {
                    self.linha_em_linha(i, controle, cx)
                } else {
                    self.linha_do_controle(i, controle, cx)
                }
            })
            .collect()
    }

    /// 🎞️ Um controle numa linha só, como no Lightroom: o rótulo encostado à
    /// direita da sua coluna, a barra, e o valor na ponta.
    ///
    /// O duplo clique no rótulo e na barra volta ao neutro, como na
    /// [`Self::linha_do_controle`]. O controle que não age agora
    /// ([`controles::Definicao::age`]) fica apagado e não se move — como o
    /// Tamanho do grão com a Intensidade em 0, no Lightroom.
    ///
    /// Na escolha entre nomes (o Estilo da vinheta) a barra vira a lista, e o
    /// nome já é o valor.
    fn linha_em_linha(
        &self,
        indice: usize,
        controle: &Controle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let valor = (definicao.ler)(&self.ajustes);
        let ligado = self.controles_ligados();
        let age = definicao.age(&self.ajustes);
        let cor = if !age {
            cx.theme().muted_foreground.opacity(0.45)
        } else if definicao.alterado(&self.ajustes) {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };

        let rotulo = div()
            .id(SharedString::from(format!("rotulo-{indice}")))
            .debug_selector(move || format!("rotulo-{indice}"))
            .w(relative(0.36))
            .flex_none()
            .text_right()
            .truncate()
            .tooltip(|window, cx| {
                gpui_kit::component::tooltip::Tooltip::new("Duplo clique volta ao neutro")
                    .build(window, cx)
            })
            .text_color(cor)
            .child(definicao.rotulo)
            .on_click(
                cx.listener(move |tela, evento: &gpui_kit::ClickEvent, window, cx| {
                    if evento.click_count() >= 2 && tela.controles_ligados() {
                        tela.devolver_ao_neutro(indice, window, cx);
                    }
                }),
            );

        let meio = match &controle.escolha {
            Some(lista) => div()
                .flex_1()
                .min_w(px(0.))
                .debug_selector(move || format!("escolha-{indice}"))
                .child(crate::estilo::campo_pequeno(
                    Select::new(lista).xsmall().disabled(!ligado),
                ))
                .into_any_element(),
            None => div()
                .flex_1()
                .min_w(px(0.))
                .debug_selector(move || format!("barra-{indice}"))
                .when(ligado && age, |barra| {
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
                .child(
                    crate::estilo::slider(&controle.estado)
                        .trilho(definicao.trilho)
                        .neutro(definicao.neutro())
                        .disabled(!ligado || !age),
                )
                .into_any_element(),
        };

        // O valor na ponta, com largura fixa: as barras de um grupo começam e
        // terminam no mesmo lugar, seja o número "0" ou "−100".
        let numero = controle.escolha.is_none().then(|| {
            div()
                .w(px(30.))
                .flex_none()
                .text_right()
                .text_color(if age { cor.opacity(0.95) } else { cor })
                .child(SharedString::from(definicao.formatar(valor)))
        });

        h_flex()
            .gap(px(8.))
            .items_center()
            .text_xs()
            .child(rotulo)
            .child(meio)
            .children(numero)
            .into_any_element()
    }

    /// 🎞️ O título da vinheta do darktable, com a chave de ligar à direita —
    /// o botão do módulo no darktable. Desligada, os controles ficam apagados
    /// e a foto não muda.
    fn titulo_da_vinheta_do_darktable(&self, separar: bool, cx: &mut Context<Self>) -> AnyElement {
        let indice = self
            .controles
            .iter()
            .position(|c| c.definicao.secao == Secao::VinhetaDarktable && e_a_chave_do_modulo(c))
            .expect("a vinheta do darktable tem a chave de ligar");
        let ligada = self.ajustes.vinheta_do_darktable_ligada();
        let rotulo = Secao::VinhetaDarktable.rotulo();
        h_flex()
            .debug_selector(move || format!("grupo-{rotulo}"))
            .items_center()
            .text_xs()
            .when(separar, |d| {
                d.mx(px(-12.))
                    .px(px(12.))
                    .mt(px(4.))
                    .pt(px(10.))
                    .border_t_1()
                    .border_color(cx.theme().border)
            })
            // O mesmo espaço dos dois lados deixa o nome no meio, como os
            // outros títulos.
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .text_color(if ligada {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(rotulo),
            )
            .child(h_flex().flex_1().justify_end().child(self.chave(
                indice,
                "chave-da-vinheta-do-darktable",
                cx,
            )))
            .into_any_element()
    }

    /// A chave (o `Switch` do kit) de um interruptor: virar é um gesto
    /// inteiro, que entra no histórico e grava na hora.
    fn chave(&self, indice: usize, id: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let controle = &self.controles[indice];
        let definicao = controle.definicao;
        let ligada = (definicao.ler)(&self.ajustes) >= 0.5;
        let tela = cx.entity().downgrade();
        div()
            .debug_selector(move || format!("chave-{indice}"))
            .child(
                Switch::new(id)
                    .small()
                    .checked(ligada)
                    .accessibility_label(definicao.rotulo)
                    .disabled(!self.controles_ligados() || !definicao.age(&self.ajustes))
                    .on_change(move |ligar, window, cx| {
                        let valor = if *ligar { 1.0 } else { 0.0 };
                        let _ = tela.update(cx, |tela, cx| {
                            tela.gesto_discreto(|a| (definicao.aplicar)(a, valor), window, cx);
                        });
                    }),
            )
            .into_any_element()
    }

    /// 🎞️ Um controle da vinheta do darktable, no desenho do darktable: o
    /// nome à esquerda e o valor à direita, com a barra embaixo — os nomes
    /// dele ("Início do decaimento") não cabem na coluna do Lightroom. A
    /// chave e a lista ficam na mesma linha do nome. Desligado, apagado.
    fn linha_do_darktable(
        &self,
        indice: usize,
        controle: &Controle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let valor = (definicao.ler)(&self.ajustes);
        let ligado = self.controles_ligados();
        let age = definicao.age(&self.ajustes);
        let cor = if !age {
            cx.theme().muted_foreground.opacity(0.45)
        } else if definicao.alterado(&self.ajustes) {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let rotulo = div()
            .id(SharedString::from(format!("rotulo-{indice}")))
            .debug_selector(move || format!("rotulo-{indice}"))
            .min_w(px(0.))
            .truncate()
            .text_color(cor)
            .tooltip(|window, cx| {
                gpui_kit::component::tooltip::Tooltip::new("Duplo clique volta ao neutro")
                    .build(window, cx)
            })
            .child(definicao.rotulo)
            .on_click(
                cx.listener(move |tela, evento: &gpui_kit::ClickEvent, window, cx| {
                    if evento.click_count() >= 2 && tela.controles_ligados() {
                        tela.devolver_ao_neutro(indice, window, cx);
                    }
                }),
            );

        // A chave e a lista: uma linha só, o nome e o controle.
        let ao_lado = if definicao.discreto && controle.escolha.is_none() {
            Some(self.chave(indice, "chave-da-proporcao-automatica", cx))
        } else {
            controle.escolha.as_ref().map(|lista| {
                div()
                    .w(relative(0.55))
                    .flex_none()
                    .debug_selector(move || format!("escolha-{indice}"))
                    .child(crate::estilo::campo_pequeno(
                        Select::new(lista).xsmall().disabled(!ligado || !age),
                    ))
                    .into_any_element()
            })
        };
        if let Some(controle_ao_lado) = ao_lado {
            return h_flex()
                .min_h(px(24.))
                .justify_between()
                .items_center()
                .gap(px(8.))
                .text_xs()
                .child(rotulo)
                .child(controle_ao_lado)
                .into_any_element();
        }

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_xs()
            .child(
                h_flex()
                    .justify_between()
                    .items_baseline()
                    .gap(px(8.))
                    .child(rotulo)
                    .child(
                        div()
                            .flex_none()
                            .text_color(cor)
                            .child(SharedString::from(definicao.formatar(valor))),
                    ),
            )
            .child(
                div()
                    .debug_selector(move || format!("barra-{indice}"))
                    .when(ligado && age, |barra| {
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
                    .child(
                        crate::estilo::slider(&controle.estado)
                            .trilho(definicao.trilho)
                            .neutro(definicao.neutro())
                            .disabled(!ligado || !age),
                    ),
            )
            .into_any_element()
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
        self.linha_do_controle_rotulada(indice, controle, controle.definicao.rotulo, cx)
    }

    /// A mesma linha, com outro rótulo na tela — "Matiz" em vez de
    /// "Sombras — matiz" dentro da vista de uma roda, onde a faixa já está
    /// escrita em cima.
    pub(super) fn linha_do_controle_rotulada(
        &self,
        indice: usize,
        controle: &Controle,
        rotulo: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let valor = (definicao.ler)(&self.ajustes);
        let alterado = definicao.alterado(&self.ajustes);

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
                                gpui_kit::component::tooltip::Tooltip::new(
                                    "Duplo clique volta ao neutro",
                                )
                                .build(window, cx)
                            })
                            .text_color(if alterado {
                                cx.theme().foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(rotulo)
                            .on_click(cx.listener(
                                move |tela, evento: &gpui_kit::ClickEvent, window, cx| {
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
            .child(self.barra_do_controle(indice, controle, cx))
            .into_any_element()
    }

    /// Só a barra de um controle, sem rótulo nem valor — a luminância embaixo
    /// de cada roda, como no Lightroom. O duplo clique volta ao neutro igual.
    pub(super) fn barra_do_controle(
        &self,
        indice: usize,
        controle: &Controle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let ligado = self.controles_ligados();
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
            .child(
                crate::estilo::slider(&controle.estado)
                    .trilho(definicao.trilho)
                    .neutro(definicao.neutro())
                    .disabled(!ligado),
            )
            .into_any_element()
    }

    /// O histograma, recolhível — só o desktop o tem.
    ///
    /// ⚠️ **Altura natural, e não fixa.** O bloco de 200px de antes tinha
    /// 260px de conteúdo; o excesso era desenhado por cima da barra do topo.
    fn painel_dos_graficos(&self, cx: &mut Context<Self>) -> AnyElement {
        // 🚨 **Nasce recolhido.** Ele não existe no site, e aberto ocupava
        // espaço que é do Básico. Quem o quer abre uma vez — a escolha fica
        // lembrada entre sessões.
        let aberto = self.estado_do_painel.aberto(CHAVE_DO_HISTOGRAMA, false);
        let histograma = aberto.then(|| {
            div()
                .px(px(12.))
                .pb(px(8.))
                .child(self.histograma(cx))
                .into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .flex_none()
            .overflow_hidden()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(self.sanfona(
                "Histograma",
                CHAVE_DO_HISTOGRAMA.to_string(),
                false,
                None,
                false,
                histograma,
                cx,
            ))
            .into_any_element()
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
                    window.paint_quad(gpui_kit::fill(bounds, fundo));
                    if barras.is_empty() {
                        return;
                    }
                    let largura = f32::from(bounds.size.width) / barras.len() as f32;
                    let base = f32::from(bounds.origin.y) + f32::from(bounds.size.height);
                    for (i, (r, g, b)) in barras.iter().enumerate() {
                        let x = f32::from(bounds.origin.x) + i as f32 * largura;
                        // Os três canais somam luz onde se sobrepõem.
                        for (altura, cor) in [
                            (r, gpui_kit::rgba(0xff000064)),
                            (g, gpui_kit::rgba(0x00ff0064)),
                            (b, gpui_kit::rgba(0x0000ff64)),
                        ] {
                            let alta = (altura * ALTURA).min(ALTURA);
                            if alta <= 0.0 {
                                continue;
                            }
                            window.paint_quad(gpui_kit::fill(
                                Bounds {
                                    origin: gpui_kit::point(px(x), px(base - alta)),
                                    size: gpui_kit::size(px(largura.max(1.0)), px(alta)),
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
}

/// 🧪 Os gestos da coluna para os cenários de ponta a ponta (`crate::e2e`) —
/// os mesmos caminhos dos cliques, sem precisar achar o nó na tela.
#[cfg(test)]
impl Revelacao {
    /// O clique no botão de um canal da curva.
    pub(crate) fn escolher_canal_da_curva(&mut self, canal: Canal, cx: &mut Context<Self>) {
        self.estado_do_painel.modo = ModoDaCurva::Ponto(canal);
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
        if let ModoDaCurva::Ponto(canal) = self.estado_do_painel.modo {
            self.devolver_no_a_reta(canal, i, window, cx);
        }
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
    ponteiro: gpui_kit::Point<Pixels>,
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
fn altura_do_ponteiro(limites: Bounds<Pixels>, ponteiro: gpui_kit::Point<Pixels>) -> f32 {
    let lado = f32::from(limites.size.height).max(1.0);
    let area_util = LADO_DA_CURVA - MARGEM_DA_CURVA * 2.0;
    let y = (f32::from(ponteiro.y) - f32::from(limites.origin.y)) / lado * LADO_DA_CURVA;
    (((MARGEM_DA_CURVA + area_util - y) / area_util) * 255.0)
        .clamp(0.0, 255.0)
        .round()
}

/// O que está fora do neutro, em uma linha: os ajustes, o enquadramento e a
/// Revelação local.
///
/// Com mais de uma parte o "fora do neutro" cai: o rodapé é estreito, e
/// "36 ajustes + enquadramento fora do n…" cortava justo o que dizia.
fn texto_do_cabecalho(alterados: usize, enquadrada: bool, locais: bool) -> String {
    let mut partes = Vec::new();
    if alterados > 0 {
        partes.push(format!(
            "{alterados} {}",
            if alterados == 1 { "ajuste" } else { "ajustes" }
        ));
    }
    if enquadrada {
        partes.push("enquadramento".to_string());
    }
    if locais {
        partes.push("Revelação local".to_string());
    }
    match partes.as_slice() {
        [] => "Nenhum ajuste fora do neutro".to_string(),
        [so] if alterados == 0 => format!("Só {} fora do neutro", com_artigo(so)),
        [so] => format!("{so} fora do neutro"),
        _ => partes.join(" + "),
    }
}

fn com_artigo(parte: &str) -> String {
    match parte {
        "enquadramento" => "o enquadramento".to_string(),
        outra => format!("a {outra}"),
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn o_cabecalho_conta_a_revelacao_local() {
        use super::texto_do_cabecalho as t;
        assert_eq!(t(0, false, false), "Nenhum ajuste fora do neutro");
        assert_eq!(t(0, true, false), "Só o enquadramento fora do neutro");
        assert_eq!(t(0, false, true), "Só a Revelação local fora do neutro");
        assert_eq!(t(36, false, false), "36 ajustes fora do neutro");
        assert_eq!(t(2, true, false), "2 ajustes + enquadramento");
        assert_eq!(t(1, false, true), "1 ajuste + Revelação local");
    }

    use super::*;

    fn quadro() -> Bounds<Pixels> {
        Bounds {
            origin: gpui_kit::point(px(100.), px(50.)),
            size: gpui_kit::size(px(260.), px(260.)),
        }
    }

    #[test]
    fn o_ponteiro_no_topo_pede_255_e_no_pe_pede_0() {
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui_kit::point(px(0.), px(58.))),
            255.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui_kit::point(px(0.), px(302.))),
            0.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui_kit::point(px(0.), px(0.))),
            255.0
        );
        assert_eq!(
            altura_do_ponteiro(quadro(), gpui_kit::point(px(0.), px(999.))),
            0.0
        );
    }

    #[test]
    fn o_clique_pega_o_no_mais_perto() {
        let neutra = curva::curva_neutra();
        // O nó do meio fica em (8 + 122, 8 + 122) do quadro.
        let no = no_sob_o_ponteiro(quadro(), gpui_kit::point(px(231.), px(179.)), &neutra);
        assert_eq!(no, Some(4));
        let longe = no_sob_o_ponteiro(quadro(), gpui_kit::point(px(231.), px(60.)), &neutra);
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
        assert!(estado.arquivo.is_none());
        assert!(estado.aberto("revelacao:Básico", true));
        estado.definir("revelacao:Básico", false);
        assert!(!estado.aberto("revelacao:Básico", true));
    }
}
