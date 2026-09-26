//! O caixa do balcão **dentro da galeria**: o total da negociação, arrastável
//! e sempre por cima da grade e da revelação — o `caixa-da-negociacao.tsx` e a
//! `edicao-rapida.tsx` do site.
//!
//! # Uma tela só com dois desenhos
//!
//! O painel é a mesma [`Caixa`] da rota em outro [`Modo`]: a carga, o cupom, os
//! diálogos, as gravações e as teclas F são os mesmos (no site, os dois usam o
//! `usePdv`). O que muda aqui é o desenho, de onde vem a sessão — da galeria
//! aberta, e não de uma lista — e as teclas do cupom.
//!
//! # O cupom acompanha a galeria
//!
//! O painel observa o [`Detalhe`]. Quando uma foto muda lá (nota, sinalizar,
//! negociar, faixa), ele relê a galeria em JSON e refaz o cupom com a conta do
//! core; quando o painel grava faixa ou negociação, pede ao [`Detalhe`] que se
//! releia, e a volta chega pelo mesmo caminho.
//!
//! # O teclado
//!
//! 🔑 **As teclas F valem na janela inteira com o painel à vista**, como no site
//! (um ouvinte na `window`). O GPUI só entrega ação a quem está no caminho do
//! foco, e o foco costuma estar na grade — então elas passam por
//! `intercept_keystrokes`, que roda antes de qualquer ligação.
//!
//! O mesmo interceptador cuida do teclado **dentro** do painel e dos diálogos
//! dele: `↑ ↓ E N T C D S ⌫` no cupom, `1`–`8` no pagamento. No site o cupom
//! para a propagação de tudo que não é tecla F — senão o `P` do cupom
//! sinalizaria a foto e o `1` do pagamento daria nota —, e aqui é o mesmo: com
//! o foco no painel, nenhuma letra chega às ligações da grade.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use biblioteca_core::caixa::{self as regras, ItemDoCupom, TipoDeMovimento};
use biblioteca_core::dinheiro;
use biblioteca_core::negociacao::{self, Negociacao, Tipo, PARCEIROS};
use gpui::{
    canvas, div, prelude::*, px, AnyElement, AnyWindowHandle, ClickEvent, Context, Div, Entity,
    FocusHandle, Focusable, FontWeight, Hsla, KeystrokeEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ScrollHandle, SharedString, Stateful, Subscription, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::dados;
use super::dialogos::{Dialogo, TipoDeDialogo};
use super::tela::{codificar, Caixa, TipoDeRecado};
use crate::pos_venda::porta::{PedidoJson, Publicador};
use crate::recursos::Icone;
use crate::sessoes::detalhe::Detalhe;
use crate::tema::cores;

/// A folga do painel até a borda da janela — o `bottom-4`/`right-4` do site.
const MARGEM: f32 = 16.;
/// Quantas gravações de um lote ficam no ar ao mesmo tempo (`EM_VOO` do site).
const EM_VOO: usize = 4;
/// O contexto de teclado do cupom.
pub(super) const CUPOM: &str = "CaixaCupom";

/// Qual das duas telas esta [`Caixa`] é.
pub(super) enum Modo {
    /// A rota `/dashboard/caixa`.
    Rota,
    /// O painel da galeria.
    Flutuante(Box<Painel>),
}

/// Um campo aberto na edição rápida do item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModoRapido {
    Desconto,
    Parceiro,
}

/// A barra de ajuste de um item — tipo de ensaio e negociação sem sair do cupom.
pub(super) struct EdicaoRapida {
    foto_id: String,
    modo: Option<ModoRapido>,
    /// A próxima ação vale para todos os itens do cupom (`Shift`).
    em_todos: bool,
    valor: Entity<InputState>,
    cupom: Entity<InputState>,
    parceiro: &'static str,
    /// A lista dos tipos de ensaio aberta (a tecla `T`).
    tipos_abertos: bool,
}

pub(super) struct Painel {
    detalhe: Entity<Detalhe>,
    janela: AnyWindowHandle,
    /// A raiz o mostra agora (galeria ou revelação).
    visivel: bool,
    minimizado: bool,
    /// O deslocamento a partir do canto inferior direito: `(0, 0)` é o canto,
    /// negativo leva para a esquerda e para cima (`limitesDoCaixa` do site).
    posicao: (f32, f32),
    /// Onde o arrasto começou: o ponteiro e a posição do painel ali.
    arrasto: Option<((f32, f32), (f32, f32))>,
    /// O tamanho do painel no último quadro — o limite do arrasto.
    tamanho: Rc<Cell<(f32, f32)>>,
    foco_do_cupom: FocusHandle,
    rolagem: ScrollHandle,
    editando: Option<EdicaoRapida>,
    /// A foto em foco na grade — o visor mostra o item dela.
    em_foco: Option<String>,
    ultimo_foco: Option<String>,
    galeria_id: Option<String>,
    /// O retrato das fotos da galeria no último quadro, para saber se mudou.
    retrato: u64,
    lembranca: PathBuf,
    _assinaturas: Vec<Subscription>,
}

/// Faixa ou negociação em várias fotos, poucas por vez (`emLote` do site).
pub(super) struct Lote {
    tipo: TipoDeLote,
    corpo: Value,
    fila: Vec<String>,
    no_ar: usize,
    total: usize,
    feitas: usize,
    falhas: Vec<String>,
    /// "esta foto" ou "N foto(s)" — a frase do recibo.
    quantas: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoDeLote {
    /// Pela edição rápida; `removida` = a negociação saiu.
    NegociacaoRapida {
        removida: bool,
    },
    /// Pelo diálogo completo (`N`).
    NegociacaoDoDialogo {
        removida: bool,
    },
    Tipo,
}

/// Onde o painel foi largado e se está minimizado (`caixa-guardado.ts`). Um só
/// para todas as sessões: o lugar do caixa é da mesa de quem opera.
#[derive(Serialize, Deserialize, Default)]
struct Guardado {
    x: f32,
    y: f32,
    minimizado: bool,
}

#[cfg(not(test))]
fn caminho_do_guardado() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("caixa-flutuante.json")
}

#[cfg(test)]
fn caminho_do_guardado() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-caixa-flutuante-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Os limites do deslocamento: do canto até a outra ponta, com a folga dos
/// dois lados, sem inverter quando o painel é maior que a janela.
fn dentro_dos_limites(posicao: (f32, f32), painel: (f32, f32), janela: (f32, f32)) -> (f32, f32) {
    let esquerda = (-(janela.0 - painel.0 - 2. * MARGEM)).min(0.);
    let topo = (-(janela.1 - painel.1 - 2. * MARGEM)).min(0.);
    (posicao.0.clamp(esquerda, 0.), posicao.1.clamp(topo, 0.))
}

/// O que as fotos da galeria dizem ao cupom — mudou, relê.
fn retrato_das_fotos(detalhe: &Detalhe) -> u64 {
    let mut h = DefaultHasher::new();
    if let Some(aberta) = detalhe.aberta() {
        aberta.galeria.id.hash(&mut h);
        for f in &aberta.fotos {
            f.id.hash(&mut h);
            format!("{:?}", f.estado).hash(&mut h);
            f.nota.hash(&mut h);
            f.apagada.hash(&mut h);
            f.produto_efetivo.hash(&mut h);
            f.preco_negociado.hash(&mut h);
            f.observacao_da_negociacao.hash(&mut h);
            f.ordem.hash(&mut h);
        }
    }
    h.finish()
}

impl Caixa {
    /// O painel da galeria, preso à tela da sessão.
    pub fn painel(
        publicador: Arc<dyn Publicador>,
        detalhe: Entity<Detalhe>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let lembranca = caminho_do_guardado();
        let guardado: Guardado = std::fs::read_to_string(&lembranca)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let observacao = cx.observe(&detalhe, |tela, detalhe, cx| {
            tela.acompanhar_detalhe(&detalhe, cx)
        });
        let fraca = cx.entity().downgrade();
        let intercepto = cx.intercept_keystrokes(move |evento, window, cx| {
            if let Some(tela) = fraca.upgrade() {
                tela.update(cx, |tela, cx| tela.interceptar(evento, window, cx));
            }
        });
        let painel = Painel {
            detalhe,
            janela: window.window_handle(),
            visivel: false,
            minimizado: guardado.minimizado,
            posicao: (guardado.x.min(0.), guardado.y.min(0.)),
            arrasto: None,
            tamanho: Rc::new(Cell::new((0., 0.))),
            foco_do_cupom: cx.focus_handle(),
            rolagem: ScrollHandle::new(),
            editando: None,
            em_foco: None,
            ultimo_foco: None,
            galeria_id: None,
            retrato: 0,
            lembranca,
            _assinaturas: vec![observacao, intercepto],
        };
        Self::construir(publicador, Modo::Flutuante(Box::new(painel)), window, cx)
    }

    fn painel_ref(&self) -> Option<&Painel> {
        match &self.modo {
            Modo::Flutuante(p) => Some(p),
            Modo::Rota => None,
        }
    }

    fn painel_mut(&mut self) -> Option<&mut Painel> {
        match &mut self.modo {
            Modo::Flutuante(p) => Some(p),
            Modo::Rota => None,
        }
    }

    /// A raiz diz se o painel está à vista (galeria ou revelação). Escondido,
    /// ele não intercepta tecla nenhuma.
    pub fn definir_visivel(&mut self, visivel: bool) {
        if let Some(p) = self.painel_mut() {
            p.visivel = visivel;
        }
    }

    pub fn visivel(&self) -> bool {
        self.painel_ref().is_some_and(|p| p.visivel)
    }

    pub fn minimizado(&self) -> bool {
        self.painel_ref().is_some_and(|p| p.minimizado)
    }

    /// Uma tecla F, como se viesse do teclado (o roteiro de depuração).
    pub fn teclar_f(&mut self, tecla: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.tecla(tecla, window, cx);
    }

    /// F9, o duplo clique na alça e os botões de minimizar e abrir.
    pub fn alternar_painel(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.painel_mut() {
            p.minimizado = !p.minimizado;
            guardar(p);
            cx.notify();
        }
    }

    /// A galeria mudou: outra sessão, outra conta, outras fotos, outro foco.
    fn acompanhar_detalhe(&mut self, detalhe: &Entity<Detalhe>, cx: &mut Context<Self>) {
        let (galeria, conta, retrato, foco) = {
            let d = detalhe.read(cx);
            (
                d.galeria_id().map(str::to_string),
                d.sessao().cloned(),
                retrato_das_fotos(d),
                d.em_foco().map(|f| f.id.clone()),
            )
        };
        if let Some(conta) = conta {
            if self.sessao.as_ref() != Some(&conta) {
                self.sessao = Some(conta);
            }
        }
        let Some(p) = self.painel_mut() else {
            return;
        };
        let mudou_o_foco = p.em_foco != foco;
        p.em_foco = foco;
        if p.galeria_id != galeria {
            p.galeria_id = galeria.clone();
            p.retrato = retrato;
            p.editando = None;
            self.dialogo = None;
            self.desconto = Default::default();
            self.ultima_venda = None;
            match galeria {
                Some(id) => self.navegar(None, Some(id), cx),
                None => {
                    self.vista = None;
                    self.escolhida = None;
                }
            }
            cx.notify();
            return;
        }
        if p.retrato != retrato {
            p.retrato = retrato;
            self.reler_galeria(cx);
        }
        if mudou_o_foco {
            cx.notify();
        }
    }

    /// Relê só a galeria — as fotos e as faixas — para o cupom acompanhar.
    fn reler_galeria(&mut self, cx: &mut Context<Self>) {
        let (Some(conta), Some(id)) = (self.sessao.clone(), self.escolhida.clone()) else {
            return;
        };
        if self.carregando() {
            // A carga em curso já traz a galeria de agora.
            return;
        }
        self.leituras_soltas += 1;
        self.publicador.pedir_json(
            conta,
            PedidoJson::ler(
                "galeria-viva",
                format!("/pos-venda/galerias/{}", codificar(&id)),
            ),
            self.leituras_de_agora(),
        );
        self.acompanhar(cx);
    }

    pub(super) fn receber_galeria_viva(&mut self, resultado: Result<Value, String>) {
        self.leituras_soltas = self.leituras_soltas.saturating_sub(1);
        let Ok(galeria) = resultado.and_then(dados::ler_galeria) else {
            return;
        };
        let Some(vista) = self.vista.as_mut() else {
            return;
        };
        if vista.sessao.as_ref().map(|s| s.id.as_str()) != Some(galeria.id.as_str()) {
            return;
        }
        vista.faixas = dados::juntar_faixas(&vista.faixas, &galeria.produtos);
        vista.padrao = galeria.padrao;
        vista.fotos = galeria.fotos;
        vista.recalcular_cupom();
    }

    /// Pede ao [`Detalhe`] que se releia — depois de o painel gravar na foto.
    pub(super) fn reler_detalhe(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.painel_ref() {
            let detalhe = p.detalhe.clone();
            detalhe.update(cx, |d, cx| d.reler(cx));
        }
    }

    // ── O teclado ───────────────────────────────────────────────────────────

    fn interceptar(
        &mut self,
        evento: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.painel_ref() else {
            return;
        };
        if !p.visivel || window.window_handle() != p.janela {
            return;
        }
        let m = evento.keystroke.modifiers;
        if m.control || m.platform || m.alt {
            return;
        }
        let contexto = |nome: &str| evento.context_stack.iter().any(|c| c.contains(nome));
        let no_dialogo = contexto("CaixaDialogo") && contexto("Flutuante");
        let no_cupom = contexto(CUPOM);
        let no_campo = contexto("Input");
        let tipo = [
            "Pagamento",
            "Desconto",
            "Movimento",
            "Pessoas",
            "Negociacao",
        ]
        .into_iter()
        .find(|t| no_dialogo && contexto(t));
        if self.teclar_no_painel(
            &evento.keystroke.key,
            m.shift,
            no_dialogo,
            tipo,
            no_cupom,
            no_campo,
            window,
            cx,
        ) {
            cx.stop_propagation();
        }
    }

    /// Uma tecla com o painel à vista. Devolve se ela foi do painel — e então
    /// não chega a mais ninguém.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn teclar_no_painel(
        &mut self,
        tecla: &str,
        shift: bool,
        no_dialogo: bool,
        tipo_do_dialogo: Option<&str>,
        no_cupom: bool,
        no_campo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // As teclas F: da janela inteira.
        if let Some(n) = tecla
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|n| (1..=9).contains(n) && *n != 5)
        {
            self.tecla(n, window, cx);
            return true;
        }
        let uma_tecla = tecla.chars().count() == 1
            || matches!(
                tecla,
                "backspace" | "delete" | "up" | "down" | "left" | "right" | "space"
            );

        if no_dialogo {
            let percentual = tecla == "%" || (shift && tecla == "5");
            if tipo_do_dialogo == Some("Desconto") && percentual {
                self.mudar_modo_do_desconto(true, cx);
                return true;
            }
            if no_campo {
                return false;
            }
            match (tipo_do_dialogo, tecla) {
                (Some("Pagamento"), n) if n.len() == 1 && ("1"..="8").contains(&n) => {
                    if let Ok(n) = n.parse::<usize>() {
                        if let Some(f) = regras::FormaDePagamento::da_tecla(n) {
                            self.escolher_forma_de_pagamento(f, window, cx);
                        }
                    }
                }
                (Some("Pagamento"), "backspace" | "delete") => self.tirar_ultimo_lancado(cx),
                (Some("Desconto"), "r") => self.mudar_modo_do_desconto(false, cx),
                (Some("Movimento"), "s") => {
                    self.mudar_tipo_do_movimento(TipoDeMovimento::Sangria, cx)
                }
                (Some("Movimento"), "u") => {
                    self.mudar_tipo_do_movimento(TipoDeMovimento::Suprimento, cx)
                }
                _ => {}
            }
            // O resto não chega à grade: o `1` daria nota às fotos de trás.
            return uma_tecla;
        }

        if !no_cupom || self.dialogo.is_some() {
            return false;
        }

        let editando = self
            .painel_ref()
            .and_then(|p| p.editando.as_ref())
            .map(|e| (e.foto_id.clone(), e.modo));
        if no_campo {
            // Os campos da edição rápida: Enter grava, Esc fecha o campo.
            return match tecla {
                "enter" => {
                    self.enviar_campo_rapido(cx);
                    true
                }
                "escape" => {
                    self.abrir_modo_rapido(None, false, window, cx);
                    true
                }
                _ => false,
            };
        }
        match tecla {
            "escape" if editando.is_some() => {
                self.fechar_edicao(window, cx);
                return true;
            }
            "up" | "down" => {
                self.mover(if tecla == "down" { 1 } else { -1 }, window, cx);
                return true;
            }
            "e" => {
                if let Some(foco) = self.foco_no_cupom() {
                    self.alternar_edicao(foco, window, cx);
                }
                return true;
            }
            "n" => {
                self.abrir_negociacao(shift, window, cx);
                return true;
            }
            _ => {}
        }
        if editando.is_some() && !self.lote_no_ar() {
            match tecla {
                "t" => {
                    if let Some(e) = self.painel_mut().and_then(|p| p.editando.as_mut()) {
                        e.em_todos = shift;
                        e.tipos_abertos = !e.tipos_abertos;
                    }
                    cx.notify();
                }
                "c" => self.alternar_cortesia(shift, cx),
                "d" => self.abrir_modo_rapido(Some(ModoRapido::Desconto), shift, window, cx),
                "s" => self.abrir_modo_rapido(Some(ModoRapido::Parceiro), shift, window, cx),
                "backspace" | "delete" => self.negociar_rapido(None, shift, cx),
                _ => {}
            }
        }
        uma_tecla
    }

    // ── O cupom ─────────────────────────────────────────────────────────────

    fn foco_no_cupom(&self) -> Option<String> {
        let foco = self.painel_ref()?.em_foco.clone()?;
        self.cupom()
            .itens
            .iter()
            .any(|i| i.foto_id == foco)
            .then_some(foco)
    }

    /// Clique num item: a grade vai até a foto, e o teclado fica no cupom.
    pub(super) fn selecionar_item(
        &mut self,
        foto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.painel_mut() else {
            return;
        };
        // Trocar de item fecha a barra: ela é do item em que foi aberta.
        if p.editando.as_ref().is_some_and(|e| e.foto_id != foto) {
            p.editando = None;
        }
        p.em_foco = Some(foto.to_string());
        let (detalhe, foco) = (p.detalhe.clone(), p.foco_do_cupom.clone());
        detalhe.update(cx, |d, cx| d.focar_foto(foto, cx));
        window.focus(&foco);
        cx.notify();
    }

    fn mover(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        let itens = self.cupom().itens;
        if itens.is_empty() {
            return;
        }
        let atual = self
            .painel_ref()
            .and_then(|p| p.em_foco.as_deref())
            .and_then(|f| itens.iter().position(|i| i.foto_id == f));
        let proximo = match atual {
            None => 0,
            Some(i) => (i as i32 + passo).clamp(0, itens.len() as i32 - 1) as usize,
        };
        let id = itens[proximo].foto_id.clone();
        self.selecionar_item(&id, window, cx);
    }

    fn alternar_edicao(&mut self, foto: String, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .painel_ref()
            .and_then(|p| p.editando.as_ref())
            .is_some_and(|e| e.foto_id == foto)
        {
            self.fechar_edicao(window, cx);
            return;
        }
        self.selecionar_item(&foto, window, cx);
        let item = self.cupom().itens.into_iter().find(|i| i.foto_id == foto);
        let valor_inicial = item
            .filter(|i| i.tipo == Some(Tipo::Desconto))
            .map(|i| dinheiro::formatar_campo(i.cobrado))
            .unwrap_or_default();
        let valor = cx.new(|cx| InputState::new(window, cx).placeholder("0,00"));
        valor.update(cx, |c, cx| c.set_value(valor_inicial, window, cx));
        let cupom = cx.new(|cx| InputState::new(window, cx).placeholder("o código do cliente"));
        if let Some(p) = self.painel_mut() {
            p.editando = Some(EdicaoRapida {
                foto_id: foto.clone(),
                modo: None,
                em_todos: false,
                valor,
                cupom,
                parceiro: PARCEIROS[0],
                tipos_abertos: false,
            });
            if let Some(i) = self.cupom().itens.iter().position(|i| i.foto_id == foto) {
                if let Some(p) = self.painel_ref() {
                    p.rolagem.scroll_to_item(i);
                }
            }
        }
        cx.notify();
    }

    fn fechar_edicao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.painel_mut() {
            p.editando = None;
            let foco = p.foco_do_cupom.clone();
            window.focus(&foco);
        }
        cx.notify();
    }

    fn item_editado(&self) -> Option<ItemDoCupom> {
        let id = self.painel_ref()?.editando.as_ref()?.foto_id.clone();
        self.cupom().itens.into_iter().find(|i| i.foto_id == id)
    }

    /// Os ids dos itens do cupom — o alvo do `Shift`.
    fn editaveis(&self) -> Vec<String> {
        self.cupom().itens.into_iter().map(|i| i.foto_id).collect()
    }

    fn abrir_modo_rapido(
        &mut self,
        modo: Option<ModoRapido>,
        todos: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.painel_mut() else {
            return;
        };
        let Some(e) = p.editando.as_mut() else {
            return;
        };
        e.modo = modo;
        e.em_todos = todos && modo.is_some();
        let foco = match modo {
            Some(ModoRapido::Desconto) => e.valor.read(cx).focus_handle(cx),
            Some(ModoRapido::Parceiro) => e.cupom.read(cx).focus_handle(cx),
            None => p.foco_do_cupom.clone(),
        };
        window.focus(&foco);
        cx.notify();
    }

    /// 🔁 Os chips ligam e desligam: clicar no aceso tira a negociação; com um
    /// campo aberto, clicar no chip dele fecha o campo.
    fn escolher_chip(&mut self, chave: Option<Tipo>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item_editado() else {
            return;
        };
        let modo = self
            .painel_ref()
            .and_then(|p| p.editando.as_ref())
            .and_then(|e| e.modo);
        match chave {
            None => {
                self.abrir_modo_rapido(None, false, window, cx);
                if item.tipo.is_some() {
                    self.negociar_rapido(None, false, cx);
                }
            }
            Some(Tipo::Cortesia) => self.alternar_cortesia(false, cx),
            Some(t) => {
                let m = if t == Tipo::Desconto {
                    ModoRapido::Desconto
                } else {
                    ModoRapido::Parceiro
                };
                if modo == Some(m) {
                    self.abrir_modo_rapido(None, false, window, cx);
                } else if modo.is_none() && item.tipo == Some(t) {
                    self.negociar_rapido(None, false, cx);
                } else {
                    self.abrir_modo_rapido(Some(m), false, window, cx);
                }
            }
        }
    }

    fn alternar_cortesia(&mut self, todos: bool, cx: &mut Context<Self>) {
        let Some(item) = self.item_editado() else {
            return;
        };
        if let Some(e) = self.painel_mut().and_then(|p| p.editando.as_mut()) {
            e.modo = None;
        }
        if item.tipo == Some(Tipo::Cortesia) && !todos {
            self.negociar_rapido(None, false, cx);
        } else {
            let cortesia = Negociacao {
                tipo: Tipo::Cortesia,
                ..Default::default()
            };
            self.negociar_rapido(Some(cortesia), todos, cx);
        }
    }

    fn enviar_campo_rapido(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.painel_ref().and_then(|p| p.editando.as_ref()) else {
            return;
        };
        let todos = e.em_todos;
        match e.modo {
            Some(ModoRapido::Desconto) => {
                let texto = e.valor.read(cx).value().to_string();
                match dinheiro::ler_campo(&texto) {
                    Some(preco) => {
                        let n = Negociacao {
                            tipo: Tipo::Desconto,
                            preco: Some(preco),
                            ..Default::default()
                        };
                        self.negociar_rapido(Some(n), todos, cx);
                    }
                    None => self.avisar(
                        "Valor inválido. Use o formato 15,00.",
                        TipoDeRecado::Erro,
                        cx,
                    ),
                }
            }
            Some(ModoRapido::Parceiro) => {
                let n = Negociacao {
                    tipo: Tipo::Parceiro,
                    preco: None,
                    parceiro: e.parceiro.to_string(),
                    cupom: e.cupom.read(cx).value().to_string(),
                    ..Default::default()
                };
                self.negociar_rapido(Some(n), todos, cx);
            }
            None => {}
        }
    }

    fn quantas(&self, todos: bool) -> (Vec<String>, String) {
        if todos {
            let ids = self.editaveis();
            let frase = format!("{} foto(s)", ids.len());
            (ids, frase)
        } else {
            let ids = self
                .item_editado()
                .map(|i| vec![i.foto_id])
                .unwrap_or_default();
            (ids, "esta foto".into())
        }
    }

    fn negociar_rapido(&mut self, n: Option<Negociacao>, todos: bool, cx: &mut Context<Self>) {
        let corpo = match n.as_ref().map(negociacao::montar) {
            Some(Err(erro)) => {
                self.avisar(erro, TipoDeRecado::Erro, cx);
                return;
            }
            Some(Ok(g)) => corpo_da_negociacao(g.preco_negociado, g.observacao),
            None => corpo_da_negociacao(None, None),
        };
        let (ids, quantas) = self.quantas(todos);
        self.gravar_lote(
            ids,
            corpo,
            TipoDeLote::NegociacaoRapida {
                removida: n.is_none(),
            },
            quantas,
            cx,
        );
    }

    fn mudar_tipo_de_ensaio(
        &mut self,
        produto: Option<String>,
        todos: bool,
        cx: &mut Context<Self>,
    ) {
        let (ids, quantas) = self.quantas(todos);
        self.gravar_lote(
            ids,
            json!({ "produto_id": produto }),
            TipoDeLote::Tipo,
            quantas,
            cx,
        );
    }

    /// 🤝 O botão de negociação do PDV (`N`): o diálogo completo. Com um item
    /// em foco, só ele, já preenchido; sem item (ou com `Shift`), todas as
    /// fotos do cupom.
    pub(super) fn abrir_negociacao(
        &mut self,
        todos: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let itens = self.cupom().itens;
        let escolhido = if todos {
            None
        } else {
            self.foco_no_cupom()
                .and_then(|f| itens.iter().find(|i| i.foto_id == f).cloned())
        };
        let alvos: Vec<ItemDoCupom> = match &escolhido {
            Some(i) => vec![i.clone()],
            None => itens.clone(),
        };
        if alvos.is_empty() {
            self.avisar(
                "Nenhuma foto do cupom para negociar.",
                TipoDeRecado::Informacao,
                cx,
            );
            return;
        }
        let faixas: HashSet<i64> = alvos.iter().map(|i| i.cheio).collect();
        let titulo = match &escolhido {
            Some(i) => format!("Negociação de {}", i.arquivo),
            None => format!("Negociação de {} foto(s) do cupom", alvos.len()),
        };
        let inicial = match &escolhido {
            Some(i) => negociacao::interpretar(i.preco_negociado, i.observacao.as_deref()),
            None => Negociacao::default(),
        };
        self.abrir_dialogo_de_negociacao(
            alvos.iter().map(|i| i.foto_id.clone()).collect(),
            titulo,
            inicial,
            (faixas.len() == 1).then(|| alvos[0].cheio),
            alvos.iter().any(ItemDoCupom::negociado),
            window,
            cx,
        );
    }

    // ── As gravações em lote ────────────────────────────────────────────────

    pub(super) fn lote_no_ar(&self) -> bool {
        self.lote.is_some()
    }

    fn gravar_lote(
        &mut self,
        ids: Vec<String>,
        corpo: Value,
        tipo: TipoDeLote,
        quantas: String,
        cx: &mut Context<Self>,
    ) {
        if ids.is_empty() || self.lote.is_some() {
            return;
        }
        let total = ids.len();
        let mut fila = ids;
        fila.reverse();
        self.lote = Some(Lote {
            tipo,
            corpo,
            fila,
            no_ar: 0,
            total,
            feitas: 0,
            falhas: Vec::new(),
            quantas,
        });
        self.soltar_lote(cx);
        cx.notify();
    }

    /// Manda as próximas do lote, até [`EM_VOO`] no ar.
    fn soltar_lote(&mut self, cx: &mut Context<Self>) {
        let mut pedidos = Vec::new();
        if let Some(lote) = self.lote.as_mut() {
            while lote.no_ar < EM_VOO {
                let Some(id) = lote.fila.pop() else {
                    break;
                };
                lote.no_ar += 1;
                pedidos.push((id, lote.corpo.clone()));
            }
        }
        for (id, corpo) in pedidos {
            if !self.gravar(
                "lote",
                "PATCH",
                format!("/pos-venda/fotos/{}", codificar(&id)),
                corpo,
                cx,
            ) {
                // Sem conta: nada vai sair, e o lote acaba aqui.
                self.lote = None;
                return;
            }
        }
    }

    pub(super) fn receber_lote(
        &mut self,
        resultado: Result<Value, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(lote) = self.lote.as_mut() else {
            return;
        };
        lote.no_ar = lote.no_ar.saturating_sub(1);
        match resultado {
            Ok(_) => lote.feitas += 1,
            Err(erro) => {
                let padrao = match lote.tipo {
                    TipoDeLote::Tipo => "Não foi possível mudar a faixa da foto.",
                    _ => "Não foi possível registrar a negociação.",
                };
                let frase = dados::mensagem_do_erro_com(
                    &erro,
                    padrao,
                    "Sua conta não pode administrar galerias.",
                );
                if !lote.falhas.contains(&frase) {
                    lote.falhas.push(frase);
                }
            }
        }
        if !lote.fila.is_empty() {
            self.soltar_lote(cx);
            return;
        }
        if lote.no_ar > 0 {
            return;
        }
        let Some(lote) = self.lote.take() else {
            return;
        };
        let oito = std::time::Duration::from_secs(8);
        if !lote.falhas.is_empty() {
            self.avisar_por(
                format!(
                    "{} não mudaram: {}",
                    lote.total - lote.feitas,
                    lote.falhas.join(" / ")
                ),
                TipoDeRecado::Erro,
                oito,
                cx,
            );
        }
        match lote.tipo {
            TipoDeLote::NegociacaoRapida { removida } => {
                if lote.falhas.is_empty() {
                    self.avisar(
                        if removida {
                            format!("Negociação removida de {}.", lote.quantas)
                        } else {
                            format!("Negociação registrada em {}.", lote.quantas)
                        },
                        TipoDeRecado::Sucesso,
                        cx,
                    );
                }
                if let Some(e) = self.painel_mut().and_then(|p| p.editando.as_mut()) {
                    e.modo = None;
                    e.em_todos = false;
                }
            }
            TipoDeLote::Tipo => {
                if lote.falhas.is_empty() {
                    self.avisar(
                        format!("Tipo de ensaio alterado em {}.", lote.quantas),
                        TipoDeRecado::Sucesso,
                        cx,
                    );
                }
                if let Some(e) = self.painel_mut().and_then(|p| p.editando.as_mut()) {
                    e.em_todos = false;
                    e.tipos_abertos = false;
                }
            }
            TipoDeLote::NegociacaoDoDialogo { removida } => {
                if lote.feitas > 0 {
                    self.avisar(
                        format!(
                            "{} foto(s) com a negociação {}.",
                            lote.feitas,
                            if removida { "removida" } else { "registrada" }
                        ),
                        TipoDeRecado::Sucesso,
                        cx,
                    );
                }
                self.negociacao_terminou(lote.feitas > 0, cx);
            }
        }
        if lote.feitas > 0 {
            self.reler_detalhe(cx);
            self.reler_galeria(cx);
        }
        cx.notify();
    }

    /// Grava a negociação do diálogo completo nas fotos dele.
    pub(super) fn gravar_negociacao_do_dialogo(
        &mut self,
        ids: Vec<String>,
        valor: Option<negociacao::Gravavel>,
        cx: &mut Context<Self>,
    ) {
        let removida = valor.is_none();
        let corpo = match valor {
            Some(g) => corpo_da_negociacao(g.preco_negociado, g.observacao),
            None => corpo_da_negociacao(None, None),
        };
        let quantas = format!("{} foto(s)", ids.len());
        self.gravar_lote(
            ids,
            corpo,
            TipoDeLote::NegociacaoDoDialogo { removida },
            quantas,
            cx,
        );
    }

    // ── O arrasto ───────────────────────────────────────────────────────────

    fn comecar_arrasto(&mut self, evento: &MouseDownEvent, cx: &mut Context<Self>) {
        if evento.click_count >= 2 {
            self.alternar_painel(cx);
            return;
        }
        if let Some(p) = self.painel_mut() {
            let ponto = (f32::from(evento.position.x), f32::from(evento.position.y));
            p.arrasto = Some((ponto, p.posicao));
            cx.notify();
        }
    }

    fn arrastar(&mut self, evento: &MouseMoveEvent, janela: (f32, f32), cx: &mut Context<Self>) {
        let Some(p) = self.painel_mut() else {
            return;
        };
        let Some((inicio, de)) = p.arrasto else {
            return;
        };
        let (x, y) = (f32::from(evento.position.x), f32::from(evento.position.y));
        let nova = (de.0 + x - inicio.0, de.1 + y - inicio.1);
        p.posicao = dentro_dos_limites(nova, p.tamanho.get(), janela);
        cx.notify();
    }

    fn soltar_arrasto(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.painel_mut() {
            if p.arrasto.take().is_some() {
                guardar(p);
                cx.notify();
            }
        }
    }

    /// Onde o painel está agora — para os testes.
    pub fn posicao_do_painel(&self) -> (f32, f32) {
        self.painel_ref().map_or((0., 0.), |p| p.posicao)
    }

    // ── O desenho ───────────────────────────────────────────────────────────

    pub(super) fn render_flutuante(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.pedir_foco = false;
        self.aplicar_pendencias_do_dialogo(window, cx);
        let janela = window.viewport_size();
        let janela = (f32::from(janela.width), f32::from(janela.height));
        let Some(p) = self.painel_ref() else {
            return div().into_any_element();
        };
        // Com a janela menor, o painel volta para dentro.
        let posicao = dentro_dos_limites(p.posicao, p.tamanho.get(), janela);
        let arrastando = p.arrasto.is_some();
        let minimizado = p.minimizado;
        let tamanho = p.tamanho.clone();
        // O item em foco rola para dentro do cupom, como a linha que acabou de
        // passar no caixa.
        if p.em_foco != p.ultimo_foco {
            if let Some(i) = p
                .em_foco
                .as_deref()
                .and_then(|f| self.cupom().itens.iter().position(|i| i.foto_id == f))
            {
                p.rolagem.scroll_to_item(i);
            }
            if let Some(p) = self.painel_mut() {
                p.ultimo_foco = p.em_foco.clone();
            }
        }

        let visivel = self.vista.is_some() && self.escolhida.is_some();
        let conteudo = if minimizado {
            self.painel_minimizado(cx).into_any_element()
        } else {
            self.painel_aberto(janela, cx).into_any_element()
        };
        let tema = cx.theme();
        let (cartao, frente) = (tema.popover, tema.foreground);

        let painel = div()
            .id("caixa-flutuante")
            .key_context(CUPOM)
            .track_focus(
                &self
                    .painel_ref()
                    .map(|p| p.foco_do_cupom.clone())
                    .unwrap_or_else(|| self.foco.clone()),
            )
            .occlude()
            .absolute()
            .right(px(MARGEM - posicao.0))
            .bottom(px(MARGEM - posicao.1))
            .overflow_hidden()
            .border_1()
            .border_color(frente.opacity(0.25))
            .bg(cartao)
            .shadow_2xl()
            .map(|d| {
                if minimizado {
                    d.rounded_full()
                } else {
                    d.rounded(px(12.))
                }
            })
            .child(conteudo)
            .child(
                canvas(
                    move |limites, _, _| {
                        tamanho.set((
                            f32::from(limites.size.width),
                            f32::from(limites.size.height),
                        ))
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );

        let arrasto = arrastando.then(|| {
            let fraca = cx.entity().downgrade();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let para_mover = fraca.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, _, _, cx| {
                        if let Some(t) = para_mover.upgrade() {
                            t.update(cx, |t, cx| t.arrastar(e, janela, cx));
                        }
                    });
                    let para_soltar = fraca.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                        if let Some(t) = para_soltar.upgrade() {
                            t.update(cx, |t, cx| t.soltar_arrasto(cx));
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        });

        let dialogo = self.render_dialogo(window, cx);
        let recados = self.render_recados(window, cx);
        div()
            .absolute()
            .inset_0()
            .children(arrasto)
            .when(visivel, |d| d.child(painel))
            .children(dialogo)
            .children(recados)
            .into_any_element()
    }

    fn situacao_do_painel(&self, cx: &Context<Self>) -> (String, Hsla) {
        let Some(v) = self.vista.as_ref() else {
            return ("Carregando…".into(), cx.theme().muted_foreground);
        };
        if v.estudio_id.is_none() {
            ("Sessão sem estúdio".into(), cores::atencao())
        } else if v.indisponivel {
            ("Caixa indisponível".into(), cx.theme().danger)
        } else if let Some(c) = &v.caixa {
            (
                format!("Caixa aberto · {}", dados::hora_br(&c.aberto_em)),
                gpui::rgb(0x00bc7d).into(),
            )
        } else {
            (
                "Caixa fechado · F8 abre".into(),
                cx.theme().muted_foreground,
            )
        }
    }

    fn alca(&self, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        h_flex().id(id).cursor_move().on_mouse_down(
            MouseButton::Left,
            cx.listener(|t, e: &MouseDownEvent, _, cx| t.comecar_arrasto(e, cx)),
        )
    }

    fn painel_minimizado(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let tema = cx.theme();
        let (apagado, mono, frente, realce) = (
            tema.muted_foreground,
            tema.mono_font_family.clone(),
            tema.foreground,
            tema.muted,
        );
        let (situacao, cor) = self.situacao_do_painel(cx);
        let n = self.cupom().itens.len();
        self.alca("caixa-alca-minimizada", cx)
            .gap(px(8.))
            .py(px(4.))
            .pl(px(8.))
            .pr(px(4.))
            .child(
                Icon::new(Icone::GripVertical)
                    .size(px(16.))
                    .text_color(apagado),
            )
            .child(
                div()
                    .id("caixa-situacao-minimizada")
                    .size(px(8.))
                    .rounded_full()
                    .bg(cor)
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(situacao.clone()).build(window, cx)
                    }),
            )
            .child(
                Icon::new(Icone::ShoppingCart)
                    .size(px(16.))
                    .text_color(apagado),
            )
            .child(div().text_xs().text_color(apagado).child(regras::itens(n)))
            .child(
                div()
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(apagado)
                    .child("TOTAL"),
            )
            .child(
                div()
                    .font_family(mono)
                    .text_lg()
                    .line_height(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .child(dinheiro::formatar(self.a_receber())),
            )
            .child(
                div()
                    .id("caixa-maximizar")
                    .size(px(28.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(apagado)
                    .cursor_pointer()
                    .hover(move |s| s.bg(realce).text_color(frente))
                    .child(Icon::new(Icone::Maximize2).size(px(16.)))
                    .tooltip(|window, cx| {
                        gpui_component::tooltip::Tooltip::new("Abrir o caixa (F9)")
                            .build(window, cx)
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.alternar_painel(cx))),
            )
    }

    fn painel_aberto(&self, janela: (f32, f32), cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado, realce, frente, fundo) = (
            tema.border,
            tema.muted_foreground,
            tema.muted,
            tema.foreground,
            tema.background,
        );
        let mono = tema.mono_font_family.clone();
        let cupom = self.cupom();
        let n = cupom.itens.len();
        let (situacao, cor) = self.situacao_do_painel(cx);
        let em_foco = self.painel_ref().and_then(|p| p.em_foco.clone());
        let item_em_foco = em_foco
            .as_deref()
            .and_then(|f| cupom.itens.iter().find(|i| i.foto_id == f))
            .cloned();

        let botao_de_icone = |id: &'static str, icone: Icone, dica: &'static str| {
            div()
                .id(id)
                .size(px(28.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(apagado)
                .cursor_pointer()
                .hover(move |s| s.bg(realce).text_color(frente))
                .child(Icon::new(icone).size(px(16.)))
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(dica).build(window, cx)
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };

        let barra =
            self.alca("caixa-alca", cx)
                .gap(px(8.))
                .px(px(12.))
                .py(px(6.))
                .border_b_1()
                .border_color(borda)
                .bg(realce.opacity(0.6))
                .child(
                    Icon::new(Icone::GripVertical)
                        .size(px(16.))
                        .text_color(apagado),
                )
                .child(Icon::new(Icone::ShoppingCart).size(px(16.)))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("CAIXA"),
                )
                .child(
                    h_flex()
                        .gap(px(6.))
                        .text_xs()
                        .text_color(apagado)
                        .child(div().size(px(8.)).rounded_full().bg(cor))
                        .child(situacao),
                )
                .child(div().flex_1())
                .child(
                    botao_de_icone("caixa-painel-atalhos", Icone::Keyboard, "Atalhos (F1)")
                        .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                            t.abrir_dialogo(TipoDeDialogo::Atalhos, w, cx)
                        })),
                )
                .child(
                    botao_de_icone(
                        "caixa-minimizar",
                        Icone::Minimize2,
                        "Minimizar — o total continua à vista (F9)",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.alternar_painel(cx))),
                );

        let visor_texto = |titulo: String, detalhe: String, valor: String| {
            h_flex()
                .w_full()
                .justify_between()
                .gap(px(12.))
                .child(
                    v_flex()
                        .min_w(px(0.))
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(titulo.to_uppercase()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(apagado)
                                .child(detalhe),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(24.))
                        .font_weight(FontWeight::BOLD)
                        .child(valor),
                )
        };
        let visor = h_flex()
            .min_h(px(56.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(borda)
            .bg(realce.opacity(0.3))
            .font_family(mono.clone())
            .child(match (&item_em_foco, &self.ultima_venda) {
                (Some(i), _) => {
                    let mut detalhe = format!("{} · {}", i.codigo(), i.faixa);
                    if let Some(e) = &i.etiqueta {
                        detalhe.push_str(&format!(" · {e}"));
                    }
                    visor_texto(i.arquivo.clone(), detalhe, dinheiro::formatar(i.cobrado))
                        .into_any_element()
                }
                (None, Some(v)) if n == 0 => {
                    let quem = [
                        Some(v.fotografo.clone()),
                        Some(v.atendente.clone()),
                        v.auxiliar.clone(),
                    ]
                    .into_iter()
                    .flatten()
                    .filter(|q| !q.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                    let valor = if v.troco > 0 {
                        format!("Troco {}", dinheiro::formatar(v.troco))
                    } else {
                        dinheiro::formatar(v.total)
                    };
                    visor_texto(format!("Venda #{} registrada", v.numero), quem, valor)
                        .into_any_element()
                }
                _ => div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .text_color(apagado)
                    .child(if n == 0 {
                        "CAIXA LIVRE".to_string()
                    } else if em_foco.is_some() {
                        "FOTO EM FOCO FORA DO CUPOM".to_string()
                    } else {
                        format!("{} NO CUPOM", regras::itens(n).to_uppercase())
                    })
                    .into_any_element(),
            });

        let lista = self.itens_do_cupom(&cupom, em_foco.as_deref(), cx);

        let linha = |rotulo: String, valor: String| {
            h_flex()
                .justify_between()
                .gap(px(12.))
                .child(div().text_color(apagado).child(rotulo.to_uppercase()))
                .child(div().truncate().child(valor))
        };
        let desconto_no_total = self.desconto_no_total();
        let pessoas = self.pessoas_validas();
        let nome = |id: &Option<String>| {
            self.nome_do_funcionario(id.as_deref())
                .unwrap_or_else(|| "?".into())
        };
        let resumo = v_flex()
            .gap(px(2.))
            .px(px(12.))
            .py(px(8.))
            .font_family(mono.clone())
            .text_xs()
            .child(linha(
                format!("Subtotal ({})", regras::itens(n)),
                dinheiro::formatar(cupom.subtotal),
            ))
            .when(cupom.descontos > 0, |d| {
                d.child(linha(
                    "Descontos nos itens".into(),
                    format!("− {}", dinheiro::formatar(cupom.descontos)),
                ))
            })
            .when(cupom.descontos < 0, |d| {
                d.child(linha(
                    "Acréscimos".into(),
                    format!("+ {}", dinheiro::formatar(-cupom.descontos)),
                ))
            })
            .when(cupom.pago_em_parceiro > 0, |d| {
                d.child(linha(
                    "Pago em site parceiro".into(),
                    format!("− {}", dinheiro::formatar(cupom.pago_em_parceiro)),
                ))
            })
            .when(desconto_no_total > 0, |d| {
                let rotulo = if self.desconto.modo == regras::ModoDoDesconto::Percentual {
                    format!("Desconto no total ({}%)", self.desconto.texto)
                } else {
                    "Desconto no total".into()
                };
                d.child(linha(
                    rotulo,
                    format!("− {}", dinheiro::formatar(desconto_no_total)),
                ))
            })
            .child(linha(
                "Fotografou · atendeu · auxiliou".into(),
                if pessoas.alguma() {
                    format!(
                        "{} · {} · {}",
                        nome(&pessoas.fotografo),
                        nome(&pessoas.atendente),
                        nome(&pessoas.auxiliar)
                    )
                } else {
                    "F3 escolhe".into()
                },
            ));

        let total = h_flex()
            .items_end()
            .justify_between()
            .gap(px(12.))
            .px(px(16.))
            .py(px(12.))
            .bg(frente)
            .text_color(fundo)
            .font_family(mono.clone())
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("TOTAL A RECEBER"),
            )
            .child(
                div()
                    .text_size(px(36.))
                    .line_height(px(36.))
                    .font_weight(FontWeight::BOLD)
                    .child(dinheiro::formatar(self.a_receber())),
            );

        let aberto = self.vista.as_ref().is_some_and(|v| v.caixa.is_some());
        let atalho = |id: &'static str, tecla: &'static str, rotulo: String, destaque: bool| {
            h_flex()
                .id(id)
                .gap(px(4.))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(6.))
                .border_1()
                .text_size(px(11.))
                .cursor_pointer()
                .map(|d| {
                    if destaque {
                        d.border_color(frente)
                            .bg(frente)
                            .text_color(fundo)
                            .hover(|s| s.opacity(0.9))
                    } else {
                        d.border_color(borda)
                            .text_color(frente)
                            .hover(move |s| s.bg(realce))
                    }
                })
                .child(
                    div()
                        .font_family(mono.clone())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tecla),
                )
                .child(rotulo)
        };
        let vendidas_antes: Vec<String> = self
            .vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| {
                s.vendas
                    .iter()
                    .filter(|v| !v.fotos_vendidas.is_empty())
                    .map(|v| format!("#{}", v.numero))
                    .collect()
            })
            .unwrap_or_default();
        let rodape = v_flex()
            .gap(px(6.))
            .px(px(12.))
            .py(px(8.))
            .text_size(px(11.))
            .text_color(apagado)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(
                        atalho("caixa-painel-f2", "F2", "Desconto".into(), false).on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| {
                                t.com_caixa(TipoDeDialogo::Desconto, w, cx)
                            }),
                        ),
                    )
                    .child(
                        atalho("caixa-painel-f3", "F3", "Pessoas".into(), false).on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| {
                                t.abrir_dialogo(TipoDeDialogo::Pessoas, w, cx)
                            }),
                        ),
                    )
                    .child(
                        atalho("caixa-painel-f4", "F4", "Finalizar".into(), true)
                            .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.finalizar(w, cx))),
                    )
                    .child(
                        atalho("caixa-painel-f6", "F6", "Sangria/suprim.".into(), false).on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| {
                                t.com_caixa(TipoDeDialogo::Movimento, w, cx)
                            }),
                        ),
                    )
                    .child(
                        atalho("caixa-painel-f7", "F7", "Vendas".into(), false).on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| {
                                t.abrir_dialogo(TipoDeDialogo::Vendas, w, cx)
                            }),
                        ),
                    )
                    .child(
                        atalho(
                            "caixa-painel-f8",
                            "F8",
                            if aberto {
                                "Fechar caixa"
                            } else {
                                "Abrir caixa"
                            }
                            .into(),
                            false,
                        )
                        .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.abrir_ou_fechar(w, cx))),
                    ),
            )
            .when(!vendidas_antes.is_empty(), |d| {
                d.child(format!(
                    "{} foto(s) já vendida(s) nesta sessão: {} — fora do cupom.",
                    cupom.ja_vendidas,
                    vendidas_antes.join(", ")
                ))
            })
            .when(cupom.negociadas_fora > 0, |d| {
                d.child(div().text_color(cores::quente_clara()).child(
                    if cupom.negociadas_fora == 1 {
                        "1 negociação está numa foto não sinalizada — fora do total.".to_string()
                    } else {
                        format!(
                            "{} negociações estão em fotos não sinalizadas — fora do total.",
                            cupom.negociadas_fora
                        )
                    },
                ))
            });

        // `min(36rem, 100vw − 2rem)` e `min(44rem, 100dvh − 2rem)`.
        let largura = 576f32.min(janela.0 - 2. * MARGEM);
        let altura = 704f32.min(janela.1 - 2. * MARGEM);
        v_flex()
            .w(px(largura))
            .max_h(px(altura))
            .child(barra)
            .child(visor)
            .child(lista)
            .child(
                v_flex()
                    .child(div().h(px(1.)).bg(borda))
                    .child(div().h(px(2.)))
                    .child(div().h(px(1.)).bg(borda)),
            )
            .child(resumo)
            .child(total)
            .child(rodape)
    }

    fn itens_do_cupom(
        &self,
        cupom: &regras::Cupom,
        em_foco: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let tema = cx.theme();
        let (borda, apagado, realce, cartao, frente) = (
            tema.border,
            tema.muted_foreground,
            tema.muted,
            tema.popover,
            tema.foreground,
        );
        let mono = tema.mono_font_family.clone();
        let editando = self
            .painel_ref()
            .and_then(|p| p.editando.as_ref())
            .map(|e| e.foto_id.clone());
        let rolagem = self
            .painel_ref()
            .map(|p| p.rolagem.clone())
            .unwrap_or_default();

        let vazio = cupom.itens.is_empty().then(|| {
            h_flex()
                .flex_wrap()
                .justify_center()
                .gap(px(4.))
                .px(px(12.))
                .py(px(24.))
                .text_color(apagado)
                .child("Nenhuma foto sinalizada a cobrar. Marque com")
                .child(crate::estilo::tecla("P"))
                .child("as que o cliente leva.")
        });

        v_flex()
            .id("caixa-cupom")
            .flex_shrink()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&rolagem)
            .text_xs()
            .children(vazio)
            .children(cupom.itens.iter().enumerate().map(|(indice, i)| {
                let e_o_foco = em_foco == Some(i.foto_id.as_str());
                let aberto = editando.as_deref() == Some(i.foto_id.as_str());
                let (fundo_ambar, _, texto_ambar) = cores::selo_ambar();
                let mut conta: Vec<AnyElement> = vec![
                    div()
                        .child(format!("1 UN × {}", dinheiro::formatar(i.cheio)))
                        .into_any_element(),
                    div().child(i.faixa.clone()).into_any_element(),
                ];
                if let Some(e) = &i.etiqueta {
                    conta.push(
                        div()
                            .px(px(4.))
                            .rounded(px(4.))
                            .bg(fundo_ambar)
                            .text_color(texto_ambar)
                            .child(e.clone())
                            .into_any_element(),
                    );
                }
                if i.desconto > 0 {
                    conta.push(
                        div()
                            .child(format!("desc. −{}", dinheiro::formatar(i.desconto)))
                            .into_any_element(),
                    );
                }
                if i.desconto < 0 {
                    conta.push(
                        div()
                            .child(format!("acrésc. +{}", dinheiro::formatar(-i.desconto)))
                            .into_any_element(),
                    );
                }
                if let Some(f) = i.pago_fora {
                    conta.push(
                        div()
                            .child(format!("pagou {} lá", dinheiro::formatar(f)))
                            .into_any_element(),
                    );
                }
                let grupo = SharedString::from(format!("caixa-item-{}", i.foto_id));
                let id = i.foto_id.clone();
                let id_do_ajuste = i.foto_id.clone();
                let linha = v_flex()
                    .id(SharedString::from(format!("caixa-linha-{}", i.foto_id)))
                    .group(grupo.clone())
                    .relative()
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(borda)
                    .font_family(mono.clone())
                    .cursor_pointer()
                    .hover(move |s| s.bg(realce.opacity(0.6)))
                    .when(e_o_foco, |d| d.bg(cores::quente().opacity(0.15)))
                    .on_click(
                        cx.listener(move |t, _: &ClickEvent, w, cx| t.selecionar_item(&id, w, cx)),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .w(px(36.))
                                    .flex_none()
                                    .text_color(apagado)
                                    .child(format!("{:03}", indice + 1)),
                            )
                            .child(
                                div()
                                    .w(px(56.))
                                    .flex_none()
                                    .text_color(apagado)
                                    .child(i.codigo()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .child(i.arquivo.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(dinheiro::formatar(i.cobrado)),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_x(px(8.))
                            .pl(px(108.))
                            .pr(px(80.))
                            .text_size(px(11.))
                            .text_color(apagado)
                            .children(conta),
                    )
                    // O gatilho da barra: à vista no item em foco ou com a barra
                    // aberta, e ao passar o mouse nos outros.
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("caixa-ajustar-{}", i.foto_id)))
                            .absolute()
                            .right(px(12.))
                            .bottom(px(6.))
                            .gap(px(4.))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(6.))
                            .border_1()
                            .text_size(px(11.))
                            .map(|d| {
                                if aberto {
                                    d.border_color(cores::quente())
                                        .bg(cores::quente())
                                        .text_color(cores::sobre_quente())
                                } else {
                                    d.border_color(borda)
                                        .bg(cartao)
                                        .text_color(frente)
                                        .hover(move |s| s.bg(realce))
                                }
                            })
                            .when(!(aberto || e_o_foco), |d| {
                                d.opacity(0.).group_hover(grupo.clone(), |s| s.opacity(1.))
                            })
                            .child(Icon::new(Icone::SlidersHorizontal).size(px(12.)))
                            .child(if aberto { "Fechar" } else { "Ajustar" })
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(10.))
                                    .opacity(0.7)
                                    .child("E"),
                            )
                            .on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                                cx.stop_propagation();
                                t.alternar_edicao(id_do_ajuste.clone(), w, cx)
                            })),
                    );
                v_flex()
                    .child(linha)
                    .when(aberto && e_o_foco, |d| d.child(self.edicao_rapida(i, cx)))
            }))
    }

    fn edicao_rapida(&self, item: &ItemDoCupom, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado, realce, cartao, fundo, frente) = (
            tema.border,
            tema.muted_foreground,
            tema.muted,
            tema.popover,
            tema.background,
            tema.foreground,
        );
        let mono = tema.mono_font_family.clone();
        let letra = tema.font_family.clone();
        let Some(e) = self.painel_ref().and_then(|p| p.editando.as_ref()) else {
            return div();
        };
        let pendente = self.lote_no_ar();
        let n_editaveis = self.editaveis().len();
        let (modo, em_todos) = (e.modo, e.em_todos);
        let vista = self.vista.as_ref();
        let padrao = vista.map(|v| v.padrao.clone());
        let faixas = vista.map(|v| v.faixas.clone()).unwrap_or_default();
        let nomear = |nome: &str, preco: i64| format!("{nome} — {}", dinheiro::formatar(preco));

        let rotulo = |texto: &str| {
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(apagado)
                .child(texto.to_uppercase())
        };
        let link = |id: &'static str, texto: String, desligado: bool| {
            div()
                .id(id)
                .text_size(px(11.))
                .text_color(apagado)
                .when(desligado, |d| d.opacity(0.4))
                .when(!desligado, move |d| {
                    d.cursor_pointer()
                        .hover(move |s| s.text_color(frente).underline())
                })
                .child(texto)
        };

        // O tipo de ensaio: o escolhido no botão, e a lista embaixo quando aberta.
        let escolhido = match &item.produto_id {
            None => padrao
                .as_ref()
                .map(|p| format!("Padrão da galeria ({})", nomear(&p.nome, p.preco)))
                .unwrap_or_default(),
            Some(id) => faixas
                .iter()
                .find(|f| &f.id == id)
                .map(|f| nomear(&f.faixa.nome, f.faixa.preco))
                .unwrap_or_else(|| "fora do catálogo".into()),
        };
        let mut opcoes: Vec<(Option<String>, String)> = vec![(
            None,
            padrao
                .as_ref()
                .map(|p| format!("Padrão da galeria ({})", nomear(&p.nome, p.preco)))
                .unwrap_or_else(|| "Padrão da galeria".into()),
        )];
        opcoes.extend(
            faixas
                .iter()
                .map(|f| (Some(f.id.clone()), nomear(&f.faixa.nome, f.faixa.preco))),
        );
        let lista_de_tipos = e.tipos_abertos.then(|| {
            v_flex()
                .id("caixa-rapido-tipos")
                .max_h(px(180.))
                .overflow_y_scroll()
                .p(px(4.))
                .rounded(px(6.))
                .border_1()
                .border_color(borda)
                .bg(fundo)
                .children(opcoes.into_iter().enumerate().map(|(n, (valor, texto))| {
                    let atual = valor == item.produto_id;
                    h_flex()
                        .id(("caixa-rapido-tipo", n))
                        .h(px(28.))
                        .px(px(8.))
                        .gap(px(8.))
                        .rounded(px(4.))
                        .justify_between()
                        .cursor_pointer()
                        .hover(move |s| s.bg(realce))
                        .child(div().truncate().child(texto))
                        .when(atual, |d| d.child(Icon::new(Icone::Check).size(px(14.))))
                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                            let todos = t
                                .painel_ref()
                                .and_then(|p| p.editando.as_ref())
                                .is_some_and(|e| e.em_todos);
                            t.mudar_tipo_de_ensaio(valor.clone(), todos, cx);
                        }))
                }))
        });

        let tipo = v_flex()
            .gap(px(6.))
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(rotulo("Tipo de ensaio"))
                            .child(crate::estilo::tecla("T")),
                    )
                    .child(
                        link(
                            "caixa-rapido-tipo-todos",
                            "Aplicar a todos os itens".into(),
                            pendente || n_editaveis < 2,
                        )
                        .when(!(pendente || n_editaveis < 2), |d| {
                            let produto = item.produto_id.clone();
                            d.on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                t.mudar_tipo_de_ensaio(produto.clone(), true, cx)
                            }))
                        }),
                    ),
            )
            .child(
                crate::estilo::botao_contorno("caixa-rapido-tipo", cx)
                    .w_full()
                    .h(px(28.))
                    .justify_between()
                    .text_xs()
                    .child(div().truncate().child(escolhido))
                    .child(Icon::new(Icone::ChevronDown).size(px(14.)))
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        if let Some(e) = t.painel_mut().and_then(|p| p.editando.as_mut()) {
                            e.tipos_abertos = !e.tipos_abertos;
                        }
                        cx.notify();
                    })),
            )
            .children(lista_de_tipos);

        let aceso = |t: Option<Tipo>| match modo {
            Some(ModoRapido::Desconto) => t == Some(Tipo::Desconto),
            Some(ModoRapido::Parceiro) => t == Some(Tipo::Parceiro),
            None => item.tipo == t,
        };
        let chips: [(Option<Tipo>, &str, &str); 4] = [
            (None, "Sem", "⌫"),
            (Some(Tipo::Cortesia), "Cortesia", "C"),
            (Some(Tipo::Desconto), "Desconto", "D"),
            (Some(Tipo::Parceiro), "Site parceiro", "S"),
        ];
        let negociacao_chips = div()
            .grid()
            .grid_cols(4)
            .gap(px(4.))
            .p(px(4.))
            .rounded(px(8.))
            .bg(realce)
            .children(
                chips
                    .into_iter()
                    .enumerate()
                    .map(|(n, (chave, texto, tecla))| {
                        let ativo = aceso(chave);
                        v_flex()
                            .id(("caixa-rapido-chip", n))
                            .items_center()
                            .gap(px(2.))
                            .px(px(4.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .text_xs()
                            .cursor_pointer()
                            .when(pendente, |d| d.opacity(0.5))
                            .map(|d| {
                                if ativo {
                                    d.bg(cores::quente())
                                        .text_color(cores::sobre_quente())
                                        .font_weight(FontWeight::MEDIUM)
                                } else {
                                    d.hover(move |s| s.bg(fundo))
                                }
                            })
                            .child(div().truncate().child(texto))
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(10.))
                                    .opacity(0.6)
                                    .child(tecla),
                            )
                            .when(!pendente, |d| {
                                d.on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                                    t.escolher_chip(chave, w, cx)
                                }))
                            })
                    }),
            );

        let campo_de = |texto: &str, campo: &Entity<InputState>| {
            v_flex()
                .gap(px(4.))
                .flex_1()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(apagado)
                        .child(texto.to_string()),
                )
                .child(h_flex().gap(px(6.)).child(Input::new(campo)))
        };
        let acoes = |cx: &mut Context<Self>| {
            h_flex()
                .justify_end()
                .gap(px(6.))
                .child(
                    crate::estilo::botao_fantasma("caixa-rapido-cancelar", cx)
                        .h(px(26.))
                        .text_xs()
                        .text_color(apagado)
                        .child("Cancelar")
                        .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                            t.abrir_modo_rapido(None, false, w, cx)
                        })),
                )
                .child(
                    h_flex()
                        .id("caixa-rapido-gravar")
                        .h(px(26.))
                        .px(px(10.))
                        .gap(px(4.))
                        .rounded(px(6.))
                        .bg(frente)
                        .text_color(fundo)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .child("Gravar")
                        .child(div().text_size(px(10.)).opacity(0.7).child("↵"))
                        .on_click(
                            cx.listener(|t, _: &ClickEvent, _, cx| t.enviar_campo_rapido(cx)),
                        ),
                )
        };
        let quadro = match modo {
            Some(ModoRapido::Desconto) => Some(
                v_flex()
                    .gap(px(8.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(borda)
                    .bg(realce.opacity(0.4))
                    .child(
                        h_flex()
                            .items_end()
                            .gap(px(12.))
                            .child(campo_de(
                                &if em_todos {
                                    format!("Valor cobrado · em todos os {n_editaveis} itens")
                                } else {
                                    "Valor cobrado".into()
                                },
                                &e.valor,
                            ))
                            .child(
                                div()
                                    .pb(px(8.))
                                    .text_size(px(11.))
                                    .text_color(apagado)
                                    .child(format!("faixa {}", dinheiro::formatar(item.cheio))),
                            ),
                    )
                    .child(acoes(cx)),
            ),
            Some(ModoRapido::Parceiro) => Some(
                v_flex()
                    .gap(px(8.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(borda)
                    .bg(realce.opacity(0.4))
                    .child(
                        h_flex()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap(px(4.))
                                    .child(div().text_size(px(11.)).text_color(apagado).child(
                                        if em_todos {
                                            format!("Site · em todos os {n_editaveis} itens")
                                        } else {
                                            "Site".into()
                                        },
                                    ))
                                    .child(h_flex().flex_wrap().gap(px(4.)).children(
                                        PARCEIROS.iter().enumerate().map(|(n, p)| {
                                            let escolhido = e.parceiro == *p;
                                            let p: &'static str = p;
                                            crate::estilo::botao_contorno(
                                                SharedString::from(format!(
                                                    "caixa-rapido-site-{n}"
                                                )),
                                                cx,
                                            )
                                            .h(px(26.))
                                            .text_xs()
                                            .when(escolhido, |d| d.border_color(cores::quente()))
                                            .child(p)
                                            .on_click(
                                                cx.listener(move |t, _: &ClickEvent, _, cx| {
                                                    if let Some(e) = t
                                                        .painel_mut()
                                                        .and_then(|pa| pa.editando.as_mut())
                                                    {
                                                        e.parceiro = p;
                                                    }
                                                    cx.notify();
                                                }),
                                            )
                                        }),
                                    )),
                            )
                            .child(campo_de("Cupom", &e.cupom)),
                    )
                    .child(acoes(cx)),
            ),
            None => None,
        };

        let arquivo = item.arquivo.clone();
        v_flex()
            .mx(px(8.))
            .my(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(frente.opacity(0.2))
            .bg(cartao)
            .shadow_lg()
            .overflow_hidden()
            .when(pendente, |d| d.opacity(0.7))
            .child(
                h_flex()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(borda)
                    .bg(realce.opacity(0.5))
                    .child(
                        Icon::new(Icone::SlidersHorizontal)
                            .size(px(14.))
                            .text_color(apagado),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Ajustar item"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_family(mono.clone())
                            .text_size(px(11.))
                            .text_color(apagado)
                            .child(arquivo),
                    )
                    .child(
                        div()
                            .id("caixa-rapido-fechar")
                            .size(px(24.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(apagado)
                            .cursor_pointer()
                            .hover(move |s| s.bg(realce).text_color(frente))
                            .child(Icon::new(Icone::X).size(px(14.)))
                            .on_click(
                                cx.listener(|t, _: &ClickEvent, w, cx| t.fechar_edicao(w, cx)),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(12.))
                    .p(px(12.))
                    .font_family(letra)
                    .child(tipo)
                    .child(
                        v_flex()
                            .gap(px(6.))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .gap(px(8.))
                                    .child(rotulo("Negociação"))
                                    .child(
                                        link(
                                            "caixa-rapido-mais",
                                            if item.tipo == Some(Tipo::Outro) {
                                                "Editar acerto · N".into()
                                            } else {
                                                "Mais opções · N".into()
                                            },
                                            pendente,
                                        )
                                        .when(
                                            !pendente,
                                            |d| {
                                                d.on_click(cx.listener(
                                                    |t, _: &ClickEvent, w, cx| {
                                                        t.abrir_negociacao(false, w, cx)
                                                    },
                                                ))
                                            },
                                        ),
                                    ),
                            )
                            .child(negociacao_chips)
                            .when(item.tipo == Some(Tipo::Outro) && modo.is_none(), |d| {
                                d.child(div().text_size(px(11.)).text_color(apagado).child(
                                    "Esta foto tem um acerto em texto livre — use “Editar acerto”.",
                                ))
                            }),
                    )
                    .children(quadro),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_t_1()
                    .border_color(borda)
                    .text_size(px(10.))
                    .text_color(apagado)
                    .child(crate::estilo::tecla("Shift"))
                    .child("+ tecla aplica a todos os itens ·")
                    .child(crate::estilo::tecla("Esc"))
                    .child("fecha"),
            )
    }
}

fn corpo_da_negociacao(preco: Option<i64>, observacao: Option<String>) -> Value {
    let observacao = observacao
        .map(|o| o.trim().chars().take(500).collect::<String>())
        .filter(|o| !o.is_empty());
    json!({
        "preco_negociado": preco.map(regras::decimal_da_api),
        "observacao_da_negociacao": observacao,
    })
}

fn guardar(p: &Painel) {
    let g = Guardado {
        x: p.posicao.0,
        y: p.posicao.1,
        minimizado: p.minimizado,
    };
    if let Ok(texto) = serde_json::to_string(&g) {
        if let Some(pasta) = p.lembranca.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        // Sem disco, vale até fechar o app — como o site sem armazenamento.
        let _ = std::fs::write(&p.lembranca, texto);
    }
}

// Os desfechos da edição rápida que o diálogo usa.
impl Caixa {
    fn mudar_modo_do_desconto(&mut self, percentual: bool, cx: &mut Context<Self>) {
        self.mudar_modo(
            if percentual {
                regras::ModoDoDesconto::Percentual
            } else {
                regras::ModoDoDesconto::Valor
            },
            cx,
        );
    }

    fn mudar_tipo_do_movimento(&mut self, tipo: TipoDeMovimento, cx: &mut Context<Self>) {
        self.mudar_tipo(tipo, cx);
    }

    fn tirar_ultimo_lancado(&mut self, cx: &mut Context<Self>) {
        self.tirar_ultimo(cx);
    }

    /// O diálogo de negociação acabou de gravar.
    fn negociacao_terminou(&mut self, fechar: bool, cx: &mut gpui::App) {
        if let Some(Dialogo::Negociacao(form)) = self.dialogo.as_mut() {
            form.enviando = false;
            if fechar {
                self.fechar_dialogo_depois(cx);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_painel_fica_dentro_da_janela() {
        let painel = (300., 200.);
        let janela = (1000., 800.);
        assert_eq!(dentro_dos_limites((0., 0.), painel, janela), (0., 0.));
        assert_eq!(
            dentro_dos_limites((-900., 40.), painel, janela),
            (-668., 0.)
        );
        // Maior que a janela: preso no canto, sem limite invertido.
        assert_eq!(
            dentro_dos_limites((-10., -10.), (1200., 900.), janela),
            (0., 0.)
        );
    }

    #[test]
    fn a_negociacao_vai_em_decimal_e_a_vazia_apaga() {
        assert_eq!(
            corpo_da_negociacao(Some(1500), Some(" Desconto ".into())),
            json!({ "preco_negociado": "15.00", "observacao_da_negociacao": "Desconto" })
        );
        assert_eq!(
            corpo_da_negociacao(None, Some("  ".into())),
            json!({ "preco_negociado": null, "observacao_da_negociacao": null })
        );
    }
}

#[cfg(test)]
#[path = "testes_do_painel.rs"]
mod testes_do_painel;

/// O cupom de 400 fotos e a negociação em lote sob desordem. Só testes.
#[cfg(test)]
#[path = "estresse_do_lote.rs"]
mod estresse_do_lote;
