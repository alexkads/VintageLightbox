//! 📋 **A gaveta do atendimento** — o que o assistente gravou, e onde se
//! corrige: agendamento, voucher, compra antecipada, como conheceu e a receita
//! padrão. É o `atendimento-da-sessao.tsx` do site, com os mesmos textos.
//!
//! 🔑 **Os campos são os do assistente** ([`super::associacao`]): cartão com
//! "Buscar…", o mesmo modal de busca, as mesmas pílulas do "como conheceu" e a
//! mesma grade de presets com amostra. Até 2026-09-26 a gaveta do app era uma
//! lista de texto que só mostrava — agendamento, voucher e compra não se
//! trocavam, o parceiro não se escolhia e o preset nem aparecia para escolher
//! (dono: *"o modal de atendimento da web também tá muito diferente no
//! desktop"*).
//!
//! 🔑 **Cada troca é gravada na hora**, num `PATCH` só com o campo que mudou
//! (ausente não é nulo). Quem grava é a tela da sessão, que é dona da galeria:
//! a gaveta só diz o quê ([`EventoDoAtendimento::Gravar`]).
//!
//! ⚠️ **Nada fica desabilitado enquanto grava** (armadilha nº 36 do site): a
//! escolha aparece na hora, e se o site recusar ela volta ao que era, com a
//! frase dele ([`Atendimento::recusado`]).
//!
//! O "como conheceu" é a exceção: resposta, parceiro e texto só fazem sentido
//! juntos — gravar "Parceiro" antes de haver parceiro é o que o site recusa —,
//! então eles se editam aqui e vão num gesto só, o "Gravar".

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use domain::entities::Preset;
use domain::services::pos_venda::{GaleriaAberta, MudancaDaGaleria, ResumoSimples, Sessao};
use gpui::{
    div, img, prelude::*, px, AnyElement, App, Context, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, Hsla, KeyDownEvent, RenderImage, SharedString, Subscription, Task,
    Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};
use image::DynamicImage;

use super::associacao::{Associador, Cartao, EventoDaAssociacao};
use super::nova::amostras::Amostras;
use super::nova::associacoes::{self as assoc, ParceiroEscolhido};
use super::nova::estado::{rotulo_da_proporcao, PROPORCOES_PADRAO};
use super::nova::receita::{self, Grupo, PresetDaSessao};
use super::nova::tela::{ItemDaBusca, TipoDeBusca};
use crate::estilo;
use crate::modal::DevolverFoco;
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::recursos::Icone;
use crate::tema;

const VERMELHO: u32 = 0xdc2626;
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(50);
/// Quantos cartões de preset cabem numa linha da gaveta — a conta das setas.
const COLUNAS_DOS_PRESETS: usize = 4;
/// `sm:[--drawer-content-width:36rem]` do site.
const LARGURA: f32 = 576.;

fn cor(hex: u32) -> Hsla {
    gpui::rgb(hex).into()
}

// ── Os valores ───────────────────────────────────────────────────────────

/// Uma associação da sessão: o id, que é o que se grava, e o cartão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Associado {
    pub id: String,
    pub cartao: Cartao,
}

/// O atendimento de uma sessão que já existe — o `AtendimentoDaSessao` do
/// site, no formato dos campos que o editam.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValoresDoAtendimento {
    pub agendamento: Option<Associado>,
    pub voucher: Option<Associado>,
    pub compra: Option<Associado>,
    pub como_conheceu: Option<String>,
    pub como_conheceu_detalhe: String,
    pub parceiro: Option<ParceiroEscolhido>,
    pub preset_id: Option<String>,
    pub proporcao: Option<String>,
}

/// 🔑 **Id sem resumo não é "sem associação".** Um `voucher_id` que chega sem
/// o resumo (a API no meio de um deploy, o voucher apagado) aparece como
/// "Voucher associado": esconder diria que não há voucher, e o gesto seguinte
/// do operador seria associar outro por cima (a mesma regra do `atendimento.ts`).
fn associado(
    id: Option<&String>,
    resumo: Option<&ResumoSimples>,
    sem_resumo: &str,
) -> Option<Associado> {
    let id = id?.clone();
    let cartao = match resumo {
        Some(r) => Cartao {
            titulo: r.titulo.clone(),
            selo: None,
            linhas: vec![r.detalhe.clone()],
        },
        None => Cartao {
            titulo: sem_resumo.to_string(),
            ..Default::default()
        },
    };
    Some(Associado { id, cartao })
}

/// O `montarAtendimento` do site: a galeria aberta no formato da gaveta.
pub fn montar(aberta: &GaleriaAberta) -> ValoresDoAtendimento {
    let g = &aberta.galeria;
    let r = &aberta.resumos;
    ValoresDoAtendimento {
        agendamento: associado(
            g.ensaio_id.as_ref(),
            r.agendamento.as_ref(),
            "Agendamento associado",
        ),
        voucher: associado(
            g.voucher_id.as_ref(),
            r.voucher.as_ref(),
            "Voucher associado",
        ),
        compra: associado(
            g.pedido_id.as_ref(),
            r.pedido.as_ref(),
            "Compra antecipada associada",
        ),
        como_conheceu: g
            .como_conheceu
            .clone()
            .filter(|v| assoc::eh_como_conheceu(v)),
        como_conheceu_detalhe: g.como_conheceu_detalhe.clone().unwrap_or_default(),
        parceiro: g.parceiro_id.as_ref().map(|id| ParceiroEscolhido {
            id: id.clone(),
            nome: r
                .parceiro
                .as_ref()
                .map(|p| p.titulo.clone())
                .unwrap_or_else(|| "Parceiro associado".into()),
            // O resumo do parceiro leva o tipo cru no detalhe.
            tipo: r
                .parceiro
                .as_ref()
                .map(|p| p.detalhe.clone())
                .unwrap_or_default(),
            whatsapp: None,
            email: None,
            ativo: true,
        }),
        preset_id: g.preset_padrao_id.clone(),
        proporcao: g
            .proporcao_padrao
            .clone()
            .filter(|p| PROPORCOES_PADRAO.contains(&p.as_str())),
    }
}

/// O "como conheceu" enquanto se edita: resposta, texto de "Outro" e parceiro.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Origem {
    pub valor: Option<String>,
    pub detalhe: String,
    pub parceiro: Option<ParceiroEscolhido>,
}

impl Origem {
    fn de(v: &ValoresDoAtendimento) -> Self {
        Self {
            valor: v.como_conheceu.clone(),
            detalhe: v.como_conheceu_detalhe.clone(),
            parceiro: v.parceiro.clone(),
        }
    }

    /// Mudou em relação ao que está gravado? É o que mostra "Gravar".
    pub fn mudou(&self, v: &ValoresDoAtendimento) -> bool {
        self.valor != v.como_conheceu
            || self.detalhe != v.como_conheceu_detalhe
            || self.parceiro.as_ref().map(|p| &p.id) != v.parceiro.as_ref().map(|p| &p.id)
    }

    /// "Parceiro" sem parceiro é o que o site recusa.
    pub fn coerente(&self) -> bool {
        self.valor.as_deref() != Some("parceiro") || self.parceiro.is_some()
    }

    /// Trocar a resposta. Clicar de novo na marcada desmarca — "não
    /// perguntei" é diferente de qualquer resposta. O parceiro só sobrevive à
    /// resposta "parceiro"; o texto, à "outro".
    pub fn escolher(&mut self, valor: &str) {
        let novo = (self.valor.as_deref() != Some(valor)).then(|| valor.to_string());
        if novo.as_deref() != Some("parceiro") {
            self.parceiro = None;
        }
        if novo.as_deref() != Some("outro") {
            self.detalhe.clear();
        }
        self.valor = novo;
    }

    /// O `PATCH` do "Gravar": os três campos juntos.
    pub fn mudanca(&self) -> MudancaDaGaleria {
        let outro = self.valor.as_deref() == Some("outro");
        let parceiro = self.valor.as_deref() == Some("parceiro");
        MudancaDaGaleria {
            como_conheceu: Some(self.valor.clone()),
            como_conheceu_detalhe: Some(
                (outro && !self.detalhe.trim().is_empty()).then(|| self.detalhe.trim().to_string()),
            ),
            parceiro_id: Some(
                parceiro
                    .then(|| self.parceiro.as_ref().map(|p| p.id.clone()))
                    .flatten(),
            ),
            ..Default::default()
        }
    }
}

// ── O componente ─────────────────────────────────────────────────────────

/// O que a gaveta pede à tela da sessão.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoDoAtendimento {
    /// Um `PATCH` na galeria, e a frase de quando ele voltar certo.
    Gravar {
        mudanca: Box<MudancaDaGaleria>,
        ok: &'static str,
    },
    /// A gaveta fechou (véu, `Esc`).
    Fechou,
}

/// Uma opção da "Proporção do corte".
#[derive(Clone)]
struct OpcaoDeCorte {
    valor: String,
    rotulo: SharedString,
}

impl SelectItem for OpcaoDeCorte {
    type Value = String;

    fn title(&self) -> SharedString {
        self.rotulo.clone()
    }

    fn value(&self) -> &String {
        &self.valor
    }
}

type EscolhaDeCorte = SelectState<SearchableVec<OpcaoDeCorte>>;

pub struct Atendimento {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    /// Como o site devolveu na última leitura — para onde a gaveta volta se o
    /// site recusar uma troca.
    do_servidor: ValoresDoAtendimento,
    /// O que a gaveta mostra: o do site, mais as trocas que ainda não voltaram.
    valores: ValoresDoAtendimento,
    origem: Origem,
    associador: Entity<Associador>,
    /// Os campos que precisam de janela para nascer — criados na primeira
    /// pintura (a raiz abre a gaveta sem janela à mão, no roteiro).
    detalhe: Option<Entity<InputState>>,
    corte: Option<Entity<EscolhaDeCorte>>,
    /// O campo de texto e o `Select` repetem o que está nos valores na
    /// próxima pintura (eles só mudam com a janela à mão).
    espelhar_o_detalhe: bool,
    espelhar_o_corte: bool,
    presets_do_sistema: Vec<Preset>,
    presets: Vec<PresetDaSessao>,
    presets_pedidos: bool,
    presets_a_caminho: bool,
    amostras: Amostras,
    /// A setas e o Espaço da grade de presets.
    foco_dos_presets: FocusHandle,
    foco_do_preset: usize,
    foco: FocusHandle,
    devolver: DevolverFoco,
    tomar_foco: bool,
    /// Quantas trocas ainda não voltaram.
    gravando: usize,
    /// A frase da última que voltou certo, ou a recusa do site.
    ultimo: Option<(String, bool)>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

impl EventEmitter<EventoDoAtendimento> for Atendimento {}

impl Focusable for Atendimento {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl Atendimento {
    pub fn novo(publicador: Arc<dyn Publicador>, cx: &mut Context<Self>) -> Self {
        let associador = cx.new(|_| Associador::novo(publicador.clone(), "atendimento"));
        let assinaturas = vec![
            cx.observe(&associador, |_, _, cx| cx.notify()),
            cx.subscribe(&associador, |tela, _, evento: &EventoDaAssociacao, cx| {
                tela.associacao(evento.clone(), cx)
            }),
        ];
        Self {
            publicador,
            sessao: None,
            do_servidor: ValoresDoAtendimento::default(),
            valores: ValoresDoAtendimento::default(),
            origem: Origem::default(),
            associador,
            detalhe: None,
            corte: None,
            espelhar_o_detalhe: false,
            espelhar_o_corte: false,
            presets_do_sistema: Vec::new(),
            presets: Vec::new(),
            presets_pedidos: false,
            presets_a_caminho: false,
            amostras: Amostras::default(),
            foco_dos_presets: cx.focus_handle(),
            foco_do_preset: 0,
            foco: cx.focus_handle(),
            devolver: DevolverFoco::default(),
            tomar_foco: false,
            gravando: 0,
            ultimo: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
            _assinaturas: assinaturas,
        }
    }

    /// A galeria (re)lida pela tela da sessão.
    ///
    /// O "como conheceu" em edição não é atropelado: só acompanha a leitura
    /// quando não havia nada por gravar.
    pub fn definir(
        &mut self,
        sessao: Option<Sessao>,
        aberta: &GaleriaAberta,
        cx: &mut Context<Self>,
    ) {
        let editando = self.origem.mudou(&self.valores);
        self.sessao = sessao.clone();
        self.associador.update(cx, |a, _| a.definir_sessao(sessao));
        self.do_servidor = montar(aberta);
        self.valores = self.do_servidor.clone();
        if !editando {
            self.origem = Origem::de(&self.valores);
            self.espelhar_o_detalhe = true;
        }
        self.espelhar_o_corte = true;
        cx.notify();
    }

    pub fn definir_presets(&mut self, presets: Vec<Preset>) {
        self.presets_do_sistema = presets;
        if self.presets.is_empty() {
            self.presets = receita::presets_da_sessao(&self.presets_do_sistema, Vec::new());
        }
    }

    /// A foto das amostras — a primeira local da sessão; sem nenhuma, a de
    /// exemplo (a mesma regra do site).
    pub fn definir_base(
        &mut self,
        origem: Option<String>,
        imagem: impl FnOnce() -> Option<DynamicImage>,
    ) {
        self.amostras.definir_base(origem, imagem);
    }

    /// Abre: pede o foco na próxima pintura e os presets do estúdio.
    pub fn abrir(&mut self, cx: &mut Context<Self>) {
        self.tomar_foco = true;
        self.ultimo = None;
        self.foco_do_preset = self.indice_do_escolhido();
        self.pedir_presets();
        self.acompanhar(cx);
        cx.notify();
    }

    /// Fecha sem devolver o foco — quem chama já cuida dele.
    pub fn largar(&mut self, cx: &mut Context<Self>) {
        self.tomar_foco = false;
        self.devolver.esquecer();
        self.associador.update(cx, |a, _| a.largar());
    }

    fn fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.associador.update(cx, |a, _| a.largar());
        self.devolver.devolver(window);
        cx.emit(EventoDoAtendimento::Fechou);
    }

    /// Uma troca voltou certa.
    pub fn gravou(&mut self, ok: &'static str, cx: &mut Context<Self>) {
        self.gravando = self.gravando.saturating_sub(1);
        self.ultimo = Some((ok.to_string(), false));
        cx.notify();
    }

    /// O site recusou: a gaveta volta ao que ele tem, com a frase dele.
    pub fn recusado(&mut self, frase: String, cx: &mut Context<Self>) {
        self.gravando = self.gravando.saturating_sub(1);
        self.valores = self.do_servidor.clone();
        self.espelhar_o_corte = true;
        self.ultimo = Some((frase, true));
        cx.notify();
    }

    fn gravar(
        &mut self,
        mudanca: MudancaDaGaleria,
        ok: &'static str,
        otimista: impl FnOnce(&mut ValoresDoAtendimento),
        cx: &mut Context<Self>,
    ) {
        otimista(&mut self.valores);
        self.gravando += 1;
        self.ultimo = None;
        cx.emit(EventoDoAtendimento::Gravar {
            mudanca: Box::new(mudanca),
            ok,
        });
        cx.notify();
    }

    fn associacao(&mut self, evento: EventoDaAssociacao, cx: &mut Context<Self>) {
        match evento {
            EventoDaAssociacao::Escolheu(item) => {
                let cartao = Cartao::do_item(&item);
                match *item {
                    ItemDaBusca::Agendamento(a) => self.gravar(
                        MudancaDaGaleria {
                            ensaio_id: Some(Some(a.id.clone())),
                            ..Default::default()
                        },
                        "Agendamento associado.",
                        |v| v.agendamento = Some(Associado { id: a.id, cartao }),
                        cx,
                    ),
                    ItemDaBusca::Voucher(vo) => self.gravar(
                        MudancaDaGaleria {
                            voucher_id: Some(Some(vo.id.clone())),
                            ..Default::default()
                        },
                        "Voucher associado.",
                        |v| v.voucher = Some(Associado { id: vo.id, cartao }),
                        cx,
                    ),
                    ItemDaBusca::Compra(c) => self.gravar(
                        MudancaDaGaleria {
                            pedido_id: Some(Some(c.id.clone())),
                            ..Default::default()
                        },
                        "Compra antecipada associada.",
                        |v| v.compra = Some(Associado { id: c.id, cartao }),
                        cx,
                    ),
                    // O parceiro entra no "como conheceu" em edição: vai com o
                    // "Gravar", junto da resposta.
                    ItemDaBusca::Parceiro(p) => {
                        self.origem.parceiro = Some(p);
                        cx.notify();
                    }
                }
            }
            EventoDaAssociacao::Removeu(tipo) => match tipo {
                TipoDeBusca::Agendamento => self.gravar(
                    MudancaDaGaleria {
                        ensaio_id: Some(None),
                        ..Default::default()
                    },
                    "Agendamento retirado.",
                    |v| v.agendamento = None,
                    cx,
                ),
                TipoDeBusca::Voucher => self.gravar(
                    MudancaDaGaleria {
                        voucher_id: Some(None),
                        ..Default::default()
                    },
                    "Voucher retirado.",
                    |v| v.voucher = None,
                    cx,
                ),
                TipoDeBusca::Compra => self.gravar(
                    MudancaDaGaleria {
                        pedido_id: Some(None),
                        ..Default::default()
                    },
                    "Compra antecipada retirada.",
                    |v| v.compra = None,
                    cx,
                ),
                TipoDeBusca::Parceiro => {
                    self.origem.parceiro = None;
                    cx.notify();
                }
            },
        }
    }

    // ── Como conheceu ────────────────────────────────────────────────────

    pub fn escolher_como_conheceu(&mut self, valor: &str, cx: &mut Context<Self>) {
        self.origem.escolher(valor);
        self.espelhar_o_detalhe = true;
        cx.notify();
    }

    /// O "Gravar" do como conheceu.
    pub fn gravar_origem(&mut self, cx: &mut Context<Self>) {
        if !self.origem.mudou(&self.valores) || !self.origem.coerente() {
            return;
        }
        let origem = self.origem.clone();
        self.gravar(
            origem.mudanca(),
            "Origem do cliente gravada.",
            move |v| {
                v.como_conheceu = origem.valor.clone();
                v.como_conheceu_detalhe = if origem.valor.as_deref() == Some("outro") {
                    origem.detalhe.clone()
                } else {
                    String::new()
                };
                v.parceiro = if origem.valor.as_deref() == Some("parceiro") {
                    origem.parceiro.clone()
                } else {
                    None
                };
            },
            cx,
        );
        // O que ficou gravado é o que se edita agora — sem isto, "Gravar"
        // continuaria à vista por uma diferença de espaços no texto.
        self.origem = Origem::de(&self.valores);
    }

    /// O "Desfazer" do como conheceu.
    pub fn desfazer_origem(&mut self, cx: &mut Context<Self>) {
        self.origem = Origem::de(&self.valores);
        self.espelhar_o_detalhe = true;
        cx.notify();
    }

    // ── Preset e corte ───────────────────────────────────────────────────

    pub fn escolher_preset(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if self.valores.preset_id == id {
            return;
        }
        self.gravar(
            MudancaDaGaleria {
                preset_padrao_id: Some(id.clone()),
                ..Default::default()
            },
            "Preset padrão gravado.",
            |v| v.preset_id = id,
            cx,
        );
    }

    pub fn escolher_proporcao(&mut self, proporcao: Option<String>, cx: &mut Context<Self>) {
        if self.valores.proporcao == proporcao {
            return;
        }
        self.gravar(
            MudancaDaGaleria {
                proporcao_padrao: Some(proporcao.clone()),
                ..Default::default()
            },
            "Proporção padrão gravada.",
            |v| v.proporcao = proporcao,
            cx,
        );
    }

    /// Os cartões da grade, na ordem das setas: "Nenhum", os do sistema e os
    /// do estúdio.
    fn ordem_dos_cartoes(&self) -> Vec<Option<String>> {
        std::iter::once(None)
            .chain(
                self.presets
                    .iter()
                    .filter(|p| p.grupo == Grupo::Sistema)
                    .chain(self.presets.iter().filter(|p| p.grupo == Grupo::Minhas))
                    .map(|p| Some(p.id.clone())),
            )
            .collect()
    }

    fn indice_do_escolhido(&self) -> usize {
        self.ordem_dos_cartoes()
            .iter()
            .position(|id| *id == self.valores.preset_id)
            .unwrap_or(0)
    }

    fn tecla_dos_presets(&mut self, evento: &KeyDownEvent, cx: &mut Context<Self>) {
        let ordem = self.ordem_dos_cartoes();
        let ultimo = ordem.len().saturating_sub(1);
        let atual = self.foco_do_preset.min(ultimo);
        let novo = match evento.keystroke.key.as_str() {
            "left" => atual.saturating_sub(1),
            "right" => (atual + 1).min(ultimo),
            "up" => atual.saturating_sub(COLUNAS_DOS_PRESETS),
            "down" => (atual + COLUNAS_DOS_PRESETS).min(ultimo),
            "space" | "enter" => {
                cx.stop_propagation();
                if let Some(id) = ordem.get(atual).cloned() {
                    self.escolher_preset(id, cx);
                }
                return;
            }
            _ => return,
        };
        cx.stop_propagation();
        self.foco_do_preset = novo;
        cx.notify();
    }

    fn pedir_presets(&mut self) {
        if self.presets_pedidos {
            return;
        }
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.presets_pedidos = true;
        self.presets_a_caminho = true;
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler("atendimento-presets", "/revelacao/presets"),
            self.recados.0.clone(),
        );
    }

    fn amostra(&mut self, id: Option<&str>) -> Option<Arc<RenderImage>> {
        let preset = id.and_then(|id| self.presets.iter().find(|p| p.id == id));
        let ajustes = receita::ajustes_da_receita(preset.map(|p| &p.preset));
        self.amostras.obter(id.unwrap_or("nenhum"), ajustes)
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Os presets do estúdio e as amostras que o motor terminou.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = self.amostras.colher();
        while let Ok(recado) = self.recados.1.try_recv() {
            if let Recado::Json {
                rotulo: "atendimento-presets",
                resultado,
            } = recado
            {
                let do_servidor = resultado
                    .as_ref()
                    .map(receita::presets_do_servidor)
                    .unwrap_or_default();
                self.presets = receita::presets_da_sessao(&self.presets_do_sistema, do_servidor);
                // Sem a lista, o pedido pode ser refeito na próxima abertura.
                self.presets_pedidos = resultado.is_ok();
                self.presets_a_caminho = false;
                self.foco_do_preset = self.indice_do_escolhido();
                mudou = true;
            }
        }
        if mudou {
            cx.notify();
        }
        let esperando = self.amostras.esperando() || self.presets_a_caminho;
        if !esperando {
            self.colhendo = false;
        }
        esperando
    }

    /// 🧪 O que a gaveta mostra agora.
    pub fn valores(&self) -> &ValoresDoAtendimento {
        &self.valores
    }

    /// 🧪 O "como conheceu" em edição.
    pub fn origem(&self) -> &Origem {
        &self.origem
    }

    pub fn associador(&self) -> &Entity<Associador> {
        &self.associador
    }
}

// ── O desenho ────────────────────────────────────────────────────────────

/// Uma seção da gaveta: o título pequeno e o corpo — o `Secao` do site.
fn secao(titulo: &'static str, corpo: impl IntoElement) -> gpui::Div {
    v_flex()
        .gap(px(8.))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(titulo),
        )
        .child(corpo)
}

impl Atendimento {
    /// Os campos que precisam de janela: nascem na primeira pintura, e
    /// repetem os valores quando eles mudam por fora.
    fn preparar_campos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.detalhe.is_none() {
            let campo = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Rádio, feira, cartaz na praça…")
            });
            self._assinaturas.push(cx.subscribe_in(
                &campo,
                window,
                |tela, campo, evento: &InputEvent, _, cx| {
                    if let InputEvent::Change = evento {
                        let texto: String = campo.read(cx).value().chars().take(200).collect();
                        if tela.origem.detalhe != texto {
                            tela.origem.detalhe = texto;
                            cx.notify();
                        }
                    }
                },
            ));
            self.detalhe = Some(campo);
            self.espelhar_o_detalhe = true;
        }
        if self.corte.is_none() {
            let opcoes: Vec<OpcaoDeCorte> = std::iter::once(("", "Sem corte padrão"))
                .chain(
                    PROPORCOES_PADRAO
                        .iter()
                        .map(|p| (*p, rotulo_da_proporcao(p))),
                )
                .map(|(valor, rotulo)| OpcaoDeCorte {
                    valor: valor.to_string(),
                    rotulo: SharedString::from(rotulo.to_string()),
                })
                .collect();
            let escolha =
                cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
            self._assinaturas.push(cx.subscribe_in(
                &escolha,
                window,
                |tela, _, evento: &SelectEvent<SearchableVec<OpcaoDeCorte>>, _, cx| {
                    let SelectEvent::Confirm(valor) = evento;
                    let proporcao = valor.clone().filter(|v| !v.trim().is_empty());
                    tela.escolher_proporcao(proporcao, cx);
                },
            ));
            self.corte = Some(escolha);
            self.espelhar_o_corte = true;
        }
        if std::mem::take(&mut self.espelhar_o_detalhe) {
            if let Some(campo) = self.detalhe.clone() {
                let texto = self.origem.detalhe.clone();
                campo.update(cx, |c, cx| {
                    if c.value().as_ref() != texto {
                        c.set_value(texto, window, cx)
                    }
                });
            }
        }
        if std::mem::take(&mut self.espelhar_o_corte) {
            if let Some(escolha) = self.corte.clone() {
                let atual = self.valores.proporcao.clone().unwrap_or_default();
                escolha.update(cx, |e, cx| e.set_selected_value(&atual, window, cx));
            }
        }
        if std::mem::take(&mut self.tomar_foco) {
            self.devolver.lembrar(window, cx);
            window.focus(&self.foco);
        }
    }

    fn como_conheceu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let origem = self.origem.clone();
        let mudou = origem.mudou(&self.valores);
        let coerente = origem.coerente();
        v_flex()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(assoc::COMO_CONHECEU.iter().map(|(valor, texto)| {
                        let marcado = origem.valor.as_deref() == Some(*valor);
                        div()
                            .id(SharedString::from(format!("atendimento-conheceu-{valor}")))
                            .h(px(32.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .border_1()
                            .border_color(if marcado { tema.primary } else { tema.input })
                            .text_sm()
                            .cursor_pointer()
                            .map(|b| {
                                if marcado {
                                    b.bg(tema.primary).text_color(tema.primary_foreground)
                                } else {
                                    b.hover(|h| h.bg(tema.accent))
                                }
                            })
                            .child(*texto)
                            .on_click(cx.listener(move |tela, _, _, cx| {
                                tela.escolher_como_conheceu(valor, cx)
                            }))
                    })),
            )
            .when(origem.valor.as_deref() == Some("parceiro"), |c| {
                let cartao = origem.parceiro.as_ref().map(Cartao::do_parceiro);
                let sem_parceiro = origem.parceiro.is_none();
                let (campo, cadastro) = self.associador.update(cx, |a, cx| {
                    (
                        a.campo(TipoDeBusca::Parceiro, cartao, cx),
                        sem_parceiro.then(|| a.cadastro_de_parceiro(false, cx)),
                    )
                });
                c.child(
                    v_flex()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Qual parceiro?"),
                        )
                        .child(campo)
                        .children(cadastro),
                )
            })
            .when(origem.valor.as_deref() == Some("outro"), |c| {
                c.child(
                    v_flex()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Como foi?"),
                        )
                        .children(
                            self.detalhe
                                .as_ref()
                                .map(|campo| Input::new(campo).w_full()),
                        ),
                )
            })
            .when(mudou, |c| {
                c.child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            estilo::desligado(
                                estilo::botao_primario("atendimento-gravar-origem", cx)
                                    .child("Gravar"),
                                !coerente,
                            )
                            .when(coerente, |b| {
                                b.on_click(cx.listener(|tela, _, _, cx| tela.gravar_origem(cx)))
                            }),
                        )
                        .child(
                            estilo::botao_fantasma("atendimento-desfazer-origem", cx)
                                .child("Desfazer")
                                .on_click(cx.listener(|tela, _, _, cx| tela.desfazer_origem(cx))),
                        )
                        .when(!coerente, |c| {
                            c.child(
                                div()
                                    .text_xs()
                                    .text_color(cor(VERMELHO))
                                    .child("Escolha o parceiro."),
                            )
                        }),
                )
            })
    }

    fn seletor_de_preset(&mut self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let escolhido = self.valores.preset_id.clone();
        let razao = receita::valor_da_proporcao(self.valores.proporcao.as_deref())
            .unwrap_or(3. / 2.)
            .max(0.4);
        let com_foco = self.foco_dos_presets.contains_focused(window, cx);
        let ordem = self.ordem_dos_cartoes();
        let nomes: Vec<String> = ordem
            .iter()
            .map(|id| match id {
                None => "Nenhum".to_string(),
                Some(id) => self
                    .presets
                    .iter()
                    .find(|p| &p.id == id)
                    .map(|p| p.nome.clone())
                    .unwrap_or_else(|| id.clone()),
            })
            .collect();
        let mut cartoes: Vec<(Option<Grupo>, AnyElement)> = Vec::new();
        for (indice, (id, nome)) in ordem.into_iter().zip(nomes).enumerate() {
            let grupo = id
                .as_deref()
                .and_then(|id| self.presets.iter().find(|p| p.id == id))
                .map(|p| p.grupo);
            let marcado = escolhido == id;
            let em_foco = com_foco && self.foco_do_preset == indice;
            let amostra = self.amostra(id.as_deref());
            let chave = id.clone().unwrap_or_else(|| "nenhum".into());
            let cartao = v_flex()
                .id(SharedString::from(format!("atendimento-preset-{chave}")))
                .relative()
                .w(px(122.))
                .p(px(6.))
                .gap(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(if marcado || em_foco {
                    tema.primary
                } else {
                    tema.border
                })
                .when(marcado, |c| c.border_2())
                .cursor_pointer()
                .hover(|c| c.bg(tema.accent))
                .child(
                    div()
                        .w_full()
                        .h(px(110. / razao))
                        .max_h(px(160.))
                        .rounded(px(4.))
                        .overflow_hidden()
                        .bg(tema.muted)
                        .when_some(amostra, |c, imagem| {
                            c.child(img(imagem).size_full().object_fit(gpui::ObjectFit::Cover))
                        }),
                )
                .when(marcado, |c| {
                    c.child(
                        div()
                            .absolute()
                            .top(px(12.))
                            .right(px(12.))
                            .size(px(20.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(tema.primary)
                            .text_color(tema.primary_foreground)
                            .child(Icon::new(Icone::Check).size(px(12.))),
                    )
                })
                .child(div().text_xs().line_clamp(2).child(nome))
                .on_click(cx.listener(move |tela, _, window, cx| {
                    tela.foco_do_preset = indice;
                    window.focus(&tela.foco_dos_presets);
                    tela.escolher_preset(id.clone(), cx);
                }))
                .into_any_element();
            cartoes.push((grupo, cartao));
        }
        if self.amostras.esperando() {
            self.acompanhar(cx);
        }
        let mut nenhum = Vec::new();
        let mut do_sistema = Vec::new();
        let mut minhas = Vec::new();
        for (grupo, cartao) in cartoes {
            match grupo {
                None => nenhum.push(cartao),
                Some(Grupo::Sistema) => do_sistema.push(cartao),
                Some(Grupo::Minhas) => minhas.push(cartao),
            }
        }
        let fora_da_lista = escolhido
            .as_deref()
            .is_some_and(|id| self.presets.iter().all(|p| p.id != id));
        if fora_da_lista {
            nenhum.push(
                div()
                    .w(px(122.))
                    .p(px(6.))
                    .rounded(px(8.))
                    .border_2()
                    .border_color(tema.primary)
                    .text_xs()
                    .child("Preset fora da lista")
                    .into_any_element(),
            );
        }
        let grade = |cartoes: Vec<AnyElement>| {
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .children(cartoes)
        };
        let rotulo = |titulo: &'static str| {
            div()
                .mt(px(8.))
                .mb(px(4.))
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(titulo)
        };
        v_flex()
            .id("atendimento-presets")
            .track_focus(&self.foco_dos_presets)
            .on_key_down(
                cx.listener(|tela, evento: &KeyDownEvent, _, cx| {
                    tela.tecla_dos_presets(evento, cx)
                }),
            )
            .w_full()
            .p(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(tema.border)
            .child(grade(nenhum))
            .when(!do_sistema.is_empty(), |c| {
                c.child(rotulo("Do sistema")).child(grade(do_sistema))
            })
            .when(!minhas.is_empty(), |c| {
                c.child(rotulo("Minhas")).child(grade(minhas))
            })
    }
}

impl Render for Atendimento {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.preparar_campos(window, cx);
        let tema = cx.theme().clone();
        let v = self.valores.clone();
        let (agendamento, voucher, compra) = self.associador.update(cx, |a, cx| {
            (
                a.campo(
                    TipoDeBusca::Agendamento,
                    v.agendamento.map(|x| x.cartao),
                    cx,
                ),
                a.campo(TipoDeBusca::Voucher, v.voucher.map(|x| x.cartao), cx),
                a.campo(TipoDeBusca::Compra, v.compra.map(|x| x.cartao), cx),
            )
        });
        let como_conheceu = self.como_conheceu(cx);
        let presets = self.seletor_de_preset(window, cx);

        // 🪟 **Gaveta pela direita, como o `Drawer` do site** — a galeria
        // continua atrás, e é dela que o operador volta a olhar quando fecha.
        // O véu fecha ao clique, como o `onOpenChange` de lá.
        div()
            .id("atendimento-veu")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(tema::cores::veu().opacity(0.5))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|tela, _, window, cx| tela.fechar(window, cx)),
            )
            .child(
                v_flex()
                    .id("atendimento-gaveta")
                    .track_focus(&self.foco)
                    .on_key_down(cx.listener(|tela, evento: &KeyDownEvent, window, cx| {
                        if evento.keystroke.key == "escape" {
                            cx.stop_propagation();
                            tela.fechar(window, cx);
                        }
                    }))
                    .occlude()
                    .absolute()
                    .top_0()
                    .right_0()
                    .h_full()
                    .w(px(LARGURA))
                    .border_l_1()
                    .border_color(tema.border)
                    .bg(tema.background)
                    .shadow_lg()
                    .child(
                        v_flex()
                            .gap(px(4.))
                            .p(px(16.))
                            .pb(px(12.))
                            .border_b_1()
                            .border_color(tema.border)
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Atendimento"),
                                    )
                                    .when(self.gravando > 0, |c| {
                                        c.child(
                                            Icon::new(Icone::LoaderCircle)
                                                .size(px(16.))
                                                .text_color(tema.muted_foreground),
                                        )
                                    }),
                            )
                            .child(div().text_sm().text_color(tema.muted_foreground).child(
                                "O que veio junto com o cliente. Cada troca é gravada na hora.",
                            ))
                            .when_some(self.ultimo.clone(), |c, (frase, erro)| {
                                c.child(
                                    div()
                                        .id("atendimento-ultimo")
                                        .text_xs()
                                        .text_color(if erro {
                                            cor(VERMELHO)
                                        } else {
                                            tema.muted_foreground
                                        })
                                        .child(frase),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .id("atendimento-corpo")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .p(px(16.))
                            .gap(px(24.))
                            .child(secao("Agendamento", agendamento))
                            .child(secao("Voucher", voucher))
                            .child(secao("Compra antecipada", compra))
                            .child(secao("Como conheceu o estúdio", como_conheceu))
                            .child(secao(
                                "Preset padrão",
                                v_flex()
                                    .gap(px(12.))
                                    .child(div().text_xs().text_color(tema.muted_foreground).child(
                                        "Preset e corte com que as fotos desta sessão chegam. \
                                         Mudar aqui vale para o que for importado daqui em \
                                         diante. Setas para percorrer, Espaço para escolher.",
                                    ))
                                    .child(
                                        v_flex()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Preset"),
                                            )
                                            .child(presets),
                                    )
                                    .child(
                                        v_flex()
                                            .gap(px(6.))
                                            .max_w(px(256.))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Proporção do corte"),
                                            )
                                            .children(
                                                self.corte
                                                    .as_ref()
                                                    .map(|escolha| Select::new(escolha).w_full()),
                                            ),
                                    ),
                            )),
                    ),
            )
            // O modal da busca, por cima de tudo — inclusive da gaveta.
            .child(self.associador.clone())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use domain::services::pos_venda::{GaleriaDoPainel, ResumosDoAtendimento};

    fn aberta(
        ajuste: impl FnOnce(&mut GaleriaDoPainel, &mut ResumosDoAtendimento),
    ) -> GaleriaAberta {
        let mut galeria = GaleriaDoPainel::default();
        let mut resumos = ResumosDoAtendimento::default();
        ajuste(&mut galeria, &mut resumos);
        GaleriaAberta {
            galeria,
            fotos: Vec::new(),
            vence_venda: None,
            vence_download: None,
            faixas: Vec::new(),
            avisos: Vec::new(),
            resumos,
        }
    }

    #[test]
    fn id_sem_resumo_aparece_como_associado() {
        let v = montar(&aberta(|g, r| {
            g.voucher_id = Some("v1".into());
            g.ensaio_id = Some("a1".into());
            r.agendamento = Some(ResumoSimples {
                titulo: "Maria".into(),
                detalhe: "13/09/2026 14:00 · Gramado".into(),
            });
        }));
        let voucher = v.voucher.expect("o id basta para haver voucher");
        assert_eq!(voucher.id, "v1");
        assert_eq!(voucher.cartao.titulo, "Voucher associado");
        assert_eq!(v.agendamento.unwrap().cartao.titulo, "Maria");
        assert!(v.compra.is_none(), "sem id não há compra");
    }

    #[test]
    fn resposta_e_corte_fora_da_lista_nao_valem() {
        let v = montar(&aberta(|g, _| {
            g.como_conheceu = Some("jornal".into());
            g.proporcao_padrao = Some("5:4".into());
        }));
        assert_eq!(v.como_conheceu, None);
        assert_eq!(v.proporcao, None);
    }

    #[test]
    fn trocar_a_resposta_solta_parceiro_e_texto() {
        let mut o = Origem {
            valor: Some("parceiro".into()),
            detalhe: String::new(),
            parceiro: Some(ParceiroEscolhido {
                id: "p1".into(),
                nome: "Hotel".into(),
                tipo: "hotel".into(),
                whatsapp: None,
                email: None,
                ativo: true,
            }),
        };
        o.escolher("instagram");
        assert_eq!(o.parceiro, None);
        let m = o.mudanca();
        assert_eq!(m.como_conheceu, Some(Some("instagram".into())));
        assert_eq!(m.parceiro_id, Some(None), "senão o site recusa com 400");
        assert_eq!(m.como_conheceu_detalhe, Some(None));

        // Clicar de novo desmarca: "não perguntei".
        o.escolher("instagram");
        assert_eq!(o.valor, None);
    }

    #[test]
    fn parceiro_sem_parceiro_nao_e_coerente() {
        let mut o = Origem::default();
        o.escolher("parceiro");
        assert!(!o.coerente());
        assert!(o.mudou(&ValoresDoAtendimento::default()));
    }

    #[test]
    fn o_texto_de_outro_vai_aparado() {
        let mut o = Origem::default();
        o.escolher("outro");
        o.detalhe = "  feira  ".into();
        assert_eq!(
            o.mudanca().como_conheceu_detalhe,
            Some(Some("feira".into()))
        );
    }
}
