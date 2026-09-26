//! As guias de sessão: cada guia é uma sessão fotográfica aberta, como uma aba
//! do navegador com a rota `[id]` do site.
//!
//! 🔑 **Por que guias, e não uma cópia da tela por sessão.** Na web o operador
//! já trabalha assim — uma aba por cliente —, e o navegador paga uma página
//! inteira por aba. Aqui a galeria, a grade e a Revelação são **uma só**, e a
//! guia guarda só o que precisa sobreviver à troca: qual sessão é, o nome dela
//! e a fila do "Salvar na galeria". Trocar de guia é sair de uma sessão e
//! entrar na outra — o mesmo caminho testado de sempre —, e a memória de vídeo
//! não cresce com o número de clientes (a Haswell já caiu por falta dela).
//!
//! 🚨 **A fila do "Salvar" fica estacionada na guia, e não jogada fora.** Sair
//! da sessão a zerava para não mandar ao site foto de outro cliente; com guias,
//! jogá-la fora na troca apagaria o aviso de que o cliente ainda vê o JPEG
//! antigo. Estacionada, ela volta quando a guia volta, e continua sem vazar
//! para a sessão ao lado.
//!
//! As guias ficam **lembradas entre aberturas** (`guias-das-sessoes.json`, ao
//! lado do catálogo), presas à conta que as abriu: outra conta nesta máquina
//! não herda as sessões de quem saiu.
//!
//! 🖱️ **A guia se arruma como aba de navegador**: arrasta-se para mudar de
//! lugar, e o botão direito abre o menu dela — renomear, dar cor, mover e
//! fechar (esta, as outras, as da direita). O nome dado ali é **só da guia**:
//! a sessão no site continua com o dela, que se troca pelo lápis do cabeçalho.
//! A cor é uma das cinco etiquetas, as mesmas das fotos.
//!
//! 🎞️ **A faixa fica também na Revelação** (dono, 24/set/2026: *"não tem guias
//! de sessão no modo revelação, deixando pouco intuitivo"*). Na web o editor
//! cobre a página, mas as abas do navegador continuam em cima dele; aqui a
//! faixa sumia, e o operador perdia de vista de quem era a foto aberta e como
//! ir ao próximo cliente.
//!
//! 🎞️ **E cada guia lembra o modo em que ficou** (dono, no mesmo dia: *"quando
//! eu mudar de guia não pode fechar o modo revelação se o mesmo estiver
//! aberto na sessão"*). Como a aba do navegador, que volta na página em que
//! foi deixada: a guia que sai da frente com a Revelação aberta guarda a tira,
//! a foto e o recorte ([`RevelacaoEstacionada`]), e volta na Revelação. A tela
//! da Revelação continua **uma só** — a memória de vídeo não cresce por
//! cliente —, e a saída grava o pendente pela porta de sempre
//! (`sair_da_revelacao`).

use crate::campo::TrocarValor as _;
use std::collections::HashMap;
use std::path::PathBuf;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::acervo::Filtro;
use domain::value_objects::{ColorLabel, CropSettings};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, ActiveTheme, Icon, Sizable};
use gpui_kit::{
    div, prelude::*, px, Context, DragMoveEvent, Entity, FontWeight, MouseButton, SharedString,
    Subscription, WeakEntity, Window,
};
use serde::{Deserialize, Serialize};

use super::{Aplicativo, Tela};
use crate::recursos::Icone;
use crate::revelacao::processador::Ajustes;
use crate::tema::cores;

/// Altura da faixa — a das abas do Chrome, menos o arredondado.
const ALTURA_DA_FAIXA: f32 = 36.;
/// Largura máxima de uma guia; com muitas abertas elas encolhem até o mínimo.
const LARGURA_MAXIMA: f32 = 220.;
const LARGURA_MINIMA: f32 = 96.;

/// Uma revelação à espera do "Salvar na galeria": `(id no site, receita, corte)`.
pub(super) type Pendente = (String, Ajustes, CropSettings);

/// A Revelação que uma guia deixou aberta ao sair da frente.
///
/// 🔑 **Guarda a tira, e não o pedido de abri-la.** Refazer a tira pela grade
/// esperaria a galeria responder, e a guia voltaria mostrando a grade por um
/// instante — o "fechou" que o dono não quer ver. A tira guardada abre na hora.
///
/// ⚠️ Só vive enquanto o app está aberto: não vai para o
/// `guias-das-sessoes.json`, porque abrir o app direto num editor de uma foto
/// de ontem seria surpresa, e não continuidade.
#[derive(Debug, Clone)]
pub(super) struct RevelacaoEstacionada {
    pub acervo: Vec<PhotoViewModel>,
    pub posicao: usize,
    pub recorte: Filtro,
    /// As marcadas da tira, com o id da grade.
    pub marcadas: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guia {
    /// O id da galeria no site.
    pub id: String,
    /// O nome da sessão, quando já se sabe. `None` enquanto a galeria não
    /// respondeu — a guia diz "Carregando…".
    #[serde(default)]
    pub titulo: Option<String>,
    /// O nome que o operador deu à guia pelo botão direito. Manda sobre o da
    /// sessão enquanto existir; a sessão no site não muda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apelido: Option<String>,
    /// A cor da guia: o nome de uma das cinco etiquetas (`ColorLabel`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cor: Option<String>,
}

impl Guia {
    /// O que a faixa escreve: o nome dado pelo operador, senão o da sessão.
    pub fn nome(&self) -> Option<&str> {
        self.apelido.as_deref().or(self.titulo.as_deref())
    }
}

/// A lista de guias, sem tela — é aqui que moram as regras de abrir, fechar e
/// andar, e é isto que os testes conferem.
#[derive(Debug, Default)]
pub struct Guias {
    lista: Vec<Guia>,
    /// As filas do "Salvar" das guias que não estão à frente, por galeria.
    estacionadas: HashMap<String, Vec<Pendente>>,
    /// As guias de trás que ficaram na Revelação, por galeria.
    reveladas: HashMap<String, RevelacaoEstacionada>,
}

impl Guias {
    pub fn lista(&self) -> &[Guia] {
        &self.lista
    }

    pub fn vazia(&self) -> bool {
        self.lista.is_empty()
    }

    /// Abre a guia da sessão, ou só a encontra se ela já estiver aberta.
    ///
    /// 🔑 **Uma sessão, uma guia.** Abrir de novo a que já está aberta leva a
    /// ela, como o "trocar para esta aba" do navegador — duas guias da mesma
    /// sessão seriam duas filas do "Salvar" para as mesmas fotos.
    ///
    /// A nova entra **logo depois da ativa**, e não no fim: é o que o navegador
    /// faz com a aba aberta a partir de outra, e mantém juntas as sessões que
    /// o operador abriu uma a partir da outra.
    pub fn abrir(&mut self, id: &str, depois_de: Option<&str>) -> bool {
        if self.posicao(id).is_some() {
            return false;
        }
        let guia = Guia {
            id: id.to_string(),
            titulo: None,
            apelido: None,
            cor: None,
        };
        match depois_de.and_then(|ativa| self.posicao(ativa)) {
            Some(i) => self.lista.insert(i + 1, guia),
            None => self.lista.push(guia),
        }
        true
    }

    /// Fecha a guia e diz qual assume o lugar dela: a da direita, ou a da
    /// esquerda quando a fechada era a última — a regra de todo navegador.
    pub fn fechar(&mut self, id: &str) -> Option<String> {
        let i = self.posicao(id)?;
        self.lista.remove(i);
        self.estacionadas.remove(id);
        self.reveladas.remove(id);
        self.lista
            .get(i)
            .or_else(|| i.checked_sub(1).and_then(|j| self.lista.get(j)))
            .map(|g| g.id.clone())
    }

    /// A guia `passo` casas adiante (ou atrás), dando a volta nas pontas, como
    /// `Ctrl+Tab`. Sem guia ativa, anda a partir da primeira.
    pub fn vizinha(&self, ativa: Option<&str>, passo: isize) -> Option<String> {
        let n = self.lista.len() as isize;
        if n == 0 {
            return None;
        }
        let i = match ativa.and_then(|a| self.posicao(a)) {
            Some(i) => (i as isize + passo).rem_euclid(n),
            None => 0,
        };
        Some(self.lista[i as usize].id.clone())
    }

    /// A guia na posição `n` (0 = a primeira), como `Cmd+1`…`Cmd+8`; `Cmd+9`
    /// é sempre a última, no navegador também.
    pub fn na_posicao(&self, n: usize) -> Option<String> {
        let guia = if n >= 8 {
            self.lista.last()
        } else {
            self.lista.get(n)
        };
        guia.map(|g| g.id.clone())
    }

    pub fn dar_nome(&mut self, id: &str, titulo: &str) -> bool {
        let titulo = titulo.trim();
        if titulo.is_empty() {
            return false;
        }
        match self.lista.iter_mut().find(|g| g.id == id) {
            Some(guia) if guia.titulo.as_deref() != Some(titulo) => {
                guia.titulo = Some(titulo.to_string());
                true
            }
            _ => false,
        }
    }

    /// O nome dado pelo botão direito. Vazio, ou igual ao da sessão, volta a
    /// guia ao nome da sessão — não há "apelido" que só repete o nome.
    pub fn apelidar(&mut self, id: &str, apelido: &str) -> bool {
        let apelido = apelido.trim();
        let Some(guia) = self.lista.iter_mut().find(|g| g.id == id) else {
            return false;
        };
        let novo = (!apelido.is_empty() && guia.titulo.as_deref() != Some(apelido))
            .then(|| apelido.to_string());
        if guia.apelido == novo {
            return false;
        }
        guia.apelido = novo;
        true
    }

    /// Dá (ou tira, com `None`) a cor da guia.
    pub fn colorir(&mut self, id: &str, cor: Option<ColorLabel>) -> bool {
        let cor = cor.map(|c| c.name().to_string());
        match self.lista.iter_mut().find(|g| g.id == id) {
            Some(guia) if guia.cor != cor => {
                guia.cor = cor;
                true
            }
            _ => false,
        }
    }

    /// Leva a guia para a posição `destino` (limitada às pontas).
    pub fn mover(&mut self, id: &str, destino: usize) -> bool {
        let Some(i) = self.posicao(id) else {
            return false;
        };
        let destino = destino.min(self.lista.len() - 1);
        if i == destino {
            return false;
        }
        let guia = self.lista.remove(i);
        self.lista.insert(destino, guia);
        true
    }

    /// O arrasto passou sobre `alvo`: a arrastada toma o lugar dela, e a
    /// `alvo` recua uma casa na direção de onde a arrastada veio — como a aba
    /// do navegador, que abre espaço enquanto a outra passa.
    pub fn passar_sobre(&mut self, arrastada: &str, alvo: &str) -> bool {
        match self.posicao(alvo) {
            Some(destino) if arrastada != alvo => self.mover(arrastada, destino),
            _ => false,
        }
    }

    /// As guias que "Fechar as outras" fecha: todas menos esta.
    pub fn outras(&self, id: &str) -> Vec<String> {
        self.lista
            .iter()
            .filter(|g| g.id != id)
            .map(|g| g.id.clone())
            .collect()
    }

    /// As guias que "Fechar as da direita" fecha.
    pub fn a_direita(&self, id: &str) -> Vec<String> {
        match self.posicao(id) {
            Some(i) => self.lista[i + 1..].iter().map(|g| g.id.clone()).collect(),
            None => Vec::new(),
        }
    }

    pub fn guia(&self, id: &str) -> Option<&Guia> {
        self.lista.iter().find(|g| g.id == id)
    }

    /// Guarda a fila do "Salvar" da guia que sai da frente.
    pub fn estacionar(&mut self, id: &str, fila: Vec<Pendente>) {
        if fila.is_empty() || self.posicao(id).is_none() {
            self.estacionadas.remove(id);
        } else {
            self.estacionadas.insert(id.to_string(), fila);
        }
    }

    /// Devolve a fila que a guia deixou ao sair da frente.
    pub fn retomar(&mut self, id: &str) -> Vec<Pendente> {
        self.estacionadas.remove(id).unwrap_or_default()
    }

    /// Quantas revelações a guia (fora da frente) ainda tem por salvar.
    pub fn por_salvar(&self, id: &str) -> usize {
        self.estacionadas.get(id).map_or(0, Vec::len)
    }

    /// A foto subiu: sai de toda fila estacionada. A resposta do site pode
    /// chegar depois de o operador trocar de guia, e a guia de antes não pode
    /// continuar dizendo que a foto não foi salva.
    pub fn deu_baixa(&mut self, no_site: &str) {
        for fila in self.estacionadas.values_mut() {
            fila.retain(|(id, _, _)| id != no_site);
        }
        self.estacionadas.retain(|_, fila| !fila.is_empty());
    }

    /// Esquece tudo — é o "Sair" da conta.
    pub fn esvaziar(&mut self) {
        self.lista.clear();
        self.estacionadas.clear();
        self.reveladas.clear();
    }

    /// A guia sai da frente com a Revelação aberta: ela fica guardada.
    pub(super) fn estacionar_revelacao(&mut self, id: &str, revelacao: RevelacaoEstacionada) {
        if self.posicao(id).is_some() && !revelacao.acervo.is_empty() {
            self.reveladas.insert(id.to_string(), revelacao);
        }
    }

    /// A Revelação que a guia deixou, se deixou — e ela sai daqui.
    pub(super) fn tirar_revelacao(&mut self, id: &str) -> Option<RevelacaoEstacionada> {
        self.reveladas.remove(id)
    }

    pub fn em_revelacao(&self, id: &str) -> bool {
        self.reveladas.contains_key(id)
    }

    pub fn posicao(&self, id: &str) -> Option<usize> {
        self.lista.iter().position(|g| g.id == id)
    }

    // ── O arquivo ────────────────────────────────────────────────────────

    /// O que vai para o disco: a conta e as guias, sem as filas — a receita
    /// não salva já mora no depósito do app, e é de lá que ela volta.
    pub fn em_json(&self, conta: Option<&str>) -> String {
        serde_json::to_string_pretty(&Arquivo {
            conta: conta.map(str::to_string),
            guias: self.lista.clone(),
        })
        .unwrap_or_default()
    }

    /// Reabre as guias de uma abertura anterior, **só se forem desta conta**.
    pub fn de_json(json: &str, conta: &str) -> Vec<Guia> {
        match serde_json::from_str::<Arquivo>(json) {
            Ok(arquivo) if arquivo.conta.as_deref() == Some(conta) => arquivo.guias,
            _ => Vec::new(),
        }
    }

    pub fn repor(&mut self, guias: Vec<Guia>) {
        for guia in guias {
            if self.posicao(&guia.id).is_none() {
                self.lista.push(guia);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Arquivo {
    conta: Option<String>,
    guias: Vec<Guia>,
}

/// Onde as guias ficam entre aberturas. Os testes nunca tocam o arquivo de
/// quem trabalha.
pub(super) fn arquivo() -> Option<PathBuf> {
    (!cfg!(test))
        .then(|| infrastructure::paths::AppPaths::catalog_root().join("guias-das-sessoes.json"))
}

/// O que a faixa está fazendo agora — nada disso vai para o disco.
#[derive(Default)]
pub(super) struct Edicao {
    /// A guia do último botão direito. O menu é montado depois do evento, e lê.
    alvo_do_menu: Option<String>,
    /// A guia com o nome em edição — um campo que some com o foco dentro, e
    /// por isso no contrato de [`crate::modal::Modal`].
    renome: crate::modal::Modal<Renome>,
    /// Onde cada guia foi desenhada no último quadro — o roteiro de depuração
    /// abre o menu sobre ela.
    desenhadas: std::rc::Rc<std::cell::RefCell<Vec<gpui_kit::Bounds<gpui_kit::Pixels>>>>,
    /// O menu aberto pelo roteiro de depuração, e onde.
    menu_do_roteiro: Option<(Entity<PopupMenu>, gpui_kit::Point<gpui_kit::Pixels>)>,
}

struct Renome {
    id: String,
    campo: Entity<InputState>,
    _assinatura: Subscription,
}

/// O que viaja no arrasto de uma guia.
#[derive(Clone)]
struct ArrastoDeGuia {
    id: String,
    nome: SharedString,
}

/// A guia que acompanha o ponteiro durante o arrasto.
struct FantasmaDaGuia {
    nome: SharedString,
}

impl Render for FantasmaDaGuia {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap(px(6.))
            .px(px(12.))
            .h(px(ALTURA_DA_FAIXA - 6.))
            .rounded(px(6.))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .opacity(0.9)
            .text_sm()
            .text_color(cx.theme().foreground)
            .child(Icon::new(Icone::Camera).size(px(14.)))
            .child(self.nome.clone())
    }
}

/// As cores do menu, na ordem das etiquetas, com o nome que o operador lê.
const CORES: [(ColorLabel, &str); 5] = [
    (ColorLabel::Red, "Vermelho"),
    (ColorLabel::Yellow, "Amarelo"),
    (ColorLabel::Green, "Verde"),
    (ColorLabel::Blue, "Azul"),
    (ColorLabel::Purple, "Roxo"),
];

/// O que o menu precisa saber da guia do botão direito.
struct MenuDaGuia {
    id: String,
    nome: String,
    apelidada: bool,
    cor: Option<String>,
    posicao: usize,
    total: usize,
}

impl Aplicativo {
    /// Grava as guias ao lado do catálogo.
    pub(super) fn guardar_guias(&self) {
        // 🚨 **Sem conta, não grava**: antes de o `/auth/me` responder, gravar
        // apagaria as guias da última abertura, que ainda nem foram repostas.
        let (Some(arquivo), Some(conta)) = (&self.arquivo_das_guias, &self.conta) else {
            return;
        };
        let _ = std::fs::write(arquivo, self.guias.em_json(Some(&conta.email)));
    }

    /// A conta chegou: as guias da última abertura voltam, se forem dela.
    pub(super) fn repor_guias(&mut self) {
        let (Some(arquivo), Some(conta)) = (&self.arquivo_das_guias, &self.conta) else {
            return;
        };
        let Ok(json) = std::fs::read_to_string(arquivo) else {
            return;
        };
        self.guias.repor(Guias::de_json(&json, &conta.email));
    }

    /// Clique numa guia, `Ctrl+Tab` ou `Cmd+1`…`9`.
    pub fn ir_para_a_guia(
        &mut self,
        id: String,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        // A guia da frente não faz nada, nem na Revelação: é a aba ativa do
        // navegador, e clicar nela não tira ninguém do editor.
        if self.sessao_aberta.as_deref() == Some(id.as_str())
            && matches!(self.tela, Tela::Sessao | Tela::Revelacao)
        {
            return;
        }
        self.estacionar_a_revelacao(cx);
        let revelada = self.guias.tirar_revelacao(&id);
        self.entrar_na_sessao(id, cx);
        if let Some(revelada) = revelada {
            self.voltar_a_revelacao(revelada, window, cx);
        }
        window.focus(&self.foco, cx);
    }

    /// O `×` da guia (ou `Cmd+W`).
    ///
    /// 🔑 **Fechar a guia da frente leva à vizinha**; fechar a última leva à
    /// lista de sessões, que é a "página nova" deste app.
    pub fn fechar_guia(
        &mut self,
        id: String,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let da_frente = self.sessao_aberta.as_deref() == Some(id.as_str());
        if da_frente {
            self.largar_a_revelacao(cx);
            // A fila sai da frente antes de a guia sumir: senão `sair_da_sessao`
            // a estacionaria numa guia que já não existe.
            self.a_subir.clear();
        }
        let vizinha = self.guias.fechar(&id);
        if da_frente {
            match vizinha {
                Some(proxima) => self.ir_para_a_guia(proxima, window, cx),
                None => self.ir_para(Tela::Sessoes, window, cx),
            }
        }
        self.guardar_guias();
        cx.notify();
    }

    /// A Revelação sai pela porta de sempre antes de a guia fechar.
    ///
    /// 🚨 `entrar_na_sessao` e `ir_para` não passam por `sair_da_revelacao`:
    /// sem isto, o ajuste dos últimos 500 ms se perderia, e a grade da sessão
    /// seguiria com a miniatura de antes da foto que estava aberta.
    pub(super) fn largar_a_revelacao(&mut self, cx: &mut Context<Self>) {
        if self.tela == Tela::Revelacao {
            self.sair_da_revelacao(cx);
        }
    }

    /// A guia da frente sai da frente (outra guia, o `+`, o menu lateral): se
    /// estava na Revelação, sai dela gravando o pendente, e a guia a guarda
    /// para voltar nela.
    pub(super) fn estacionar_a_revelacao(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self
            .sessao_aberta
            .clone()
            .filter(|_| self.tela == Tela::Revelacao)
        else {
            return;
        };
        self.sair_da_revelacao(cx);
        let revelada = {
            let tela = self.revelacao.read(cx);
            RevelacaoEstacionada {
                acervo: tela.acervo().to_vec(),
                posicao: tela.posicao(),
                recorte: tela.recorte(),
                marcadas: tela.marcadas_na_grade(),
            }
        };
        self.guias.estacionar_revelacao(&id, revelada);
    }

    /// A guia volta à frente na Revelação em que ficou.
    ///
    /// 🔑 **Se a tela da Revelação ainda é a desta guia** (ninguém revelou
    /// outra sessão no meio), ela só volta a aparecer: o zoom e o desfazer
    /// continuam onde estavam. Senão, a tira guardada é aberta de novo.
    fn voltar_a_revelacao(
        &mut self,
        revelada: RevelacaoEstacionada,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let intacta = {
            let tela = self.revelacao.read(cx);
            tela.posicao() == revelada.posicao
                && tela
                    .acervo()
                    .iter()
                    .map(|f| &f.id)
                    .eq(revelada.acervo.iter().map(|f| &f.id))
        };
        if !intacta {
            self.revelacao.update(cx, |tela, cx| {
                tela.abrir_no_acervo(revelada.acervo, revelada.posicao, window, cx);
                tela.herdar_da_sessao(revelada.recorte, &revelada.marcadas, cx);
            });
        }
        self.tela = Tela::Revelacao;
        self.recontar_o_que_falta_subir(cx);
        self.atualizar_o_cliente(true, cx);
        cx.notify();
    }

    /// O `+` da faixa e o `⌘T`: a lista de sessões, que é a "página nova".
    fn abrir_guia_nova(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        self.ir_para(Tela::Sessoes, window, cx);
    }

    /// Anda `passo` guias a partir da da frente.
    pub fn andar_nas_guias(
        &mut self,
        passo: isize,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.guias.vizinha(self.sessao_aberta.as_deref(), passo) {
            self.ir_para_a_guia(id, window, cx);
        }
    }

    pub fn guia_na_posicao(
        &mut self,
        n: usize,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.guias.na_posicao(n) {
            self.ir_para_a_guia(id, window, cx);
        }
    }

    /// O nome da sessão da frente, assim que a galeria responde.
    pub(super) fn nomear_a_guia_da_frente(&mut self, cx: &Context<Self>) {
        if let Some(id) = self.sessao_aberta.clone() {
            self.nomear_a_guia(&id, cx);
        }
    }

    /// O nome vem da galeria que a tela da sessão tem aberta — e só se for a
    /// desta guia: na troca, a tela ainda pode estar com a de antes.
    pub(super) fn nomear_a_guia(&mut self, id: &str, cx: &Context<Self>) {
        let titulo = self
            .detalhe
            .read(cx)
            .aberta()
            .filter(|a| a.galeria.id == id)
            .map(|a| a.galeria.titulo.clone());
        if let Some(titulo) = titulo {
            if self.guias.dar_nome(id, &titulo) {
                self.guardar_guias();
            }
        }
    }

    /// As teclas do navegador.
    ///
    /// 🚨 **Na Revelação, só as que andam** (`Ctrl+Tab`, `⌘1`…`⌘9`): `⌘W`
    /// fecharia a sessão com a foto aberta no meio de um ajuste, e `⌘T` a
    /// levaria para a lista — as duas por um dedo que escorregou. Fechar ou
    /// abrir guia, ali, é pelo mouse.
    pub(super) fn ouvir_atalhos_das_guias(
        &self,
        raiz: gpui_kit::Div,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Div {
        let raiz = match self.tela {
            Tela::Sessao | Tela::Sessoes => raiz
                .on_action(cx.listener(|raiz, _: &super::GuiaNova, window, cx| {
                    raiz.abrir_guia_nova(window, cx);
                }))
                .on_action(cx.listener(|raiz, _: &super::FecharGuia, window, cx| {
                    if raiz.tela == Tela::Sessao {
                        if let Some(id) = raiz.sessao_aberta.clone() {
                            raiz.fechar_guia(id, window, cx);
                        }
                    }
                })),
            Tela::Revelacao if self.sessao_aberta.is_some() => raiz,
            _ => return raiz,
        };
        raiz.on_action(cx.listener(|raiz, _: &super::ProximaGuia, window, cx| {
            raiz.andar_nas_guias(1, window, cx);
        }))
        .on_action(cx.listener(|raiz, _: &super::GuiaAnterior, window, cx| {
            raiz.andar_nas_guias(-1, window, cx);
        }))
        .on_action(cx.listener(|raiz, _: &super::Guia1, w, cx| raiz.guia_na_posicao(0, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia2, w, cx| raiz.guia_na_posicao(1, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia3, w, cx| raiz.guia_na_posicao(2, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia4, w, cx| raiz.guia_na_posicao(3, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia5, w, cx| raiz.guia_na_posicao(4, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia6, w, cx| raiz.guia_na_posicao(5, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia7, w, cx| raiz.guia_na_posicao(6, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia8, w, cx| raiz.guia_na_posicao(7, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::UltimaGuia, w, cx| raiz.guia_na_posicao(8, w, cx)))
    }

    pub fn titulos_das_guias_para_teste(&self) -> Vec<Option<String>> {
        self.guias
            .lista()
            .iter()
            .map(|g| g.titulo.clone())
            .collect()
    }

    pub fn guia_em_revelacao_para_teste(&self, id: &str) -> bool {
        self.guias.em_revelacao(id)
    }

    pub fn guias_para_teste(&self) -> Vec<String> {
        self.guias.lista().iter().map(|g| g.id.clone()).collect()
    }

    // ── O menu do botão direito ─────────────────────────────────────────

    /// A guia vira campo, começando com o nome que ela mostra.
    ///
    /// 🔑 **O foco vai ao campo depois de o menu fechar**: o menu devolve o
    /// foco a quem o tinha, e o campo perderia o foco — e se confirmaria — no
    /// mesmo quadro em que nasceu.
    pub fn comecar_a_renomear_guia(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(guia) = self.guias.guia(&id) else {
            return;
        };
        let nome = guia.nome().unwrap_or_default().to_string();
        let campo = cx.new(|cx| InputState::new(window, cx).placeholder("Nome da guia"));
        campo.update(cx, |campo, cx| campo.trocar_valor(nome, window, cx));
        let assinatura = cx.subscribe_in(
            &campo,
            window,
            |raiz: &mut Self, _campo, evento: &InputEvent, window, cx| match evento {
                InputEvent::PressEnter { .. } => raiz.confirmar_renome_da_guia(Some(window), cx),
                // Clicou fora: o foco já foi para onde o operador quis.
                InputEvent::Blur => raiz.confirmar_renome_da_guia(None, cx),
                _ => {}
            },
        );
        let focar = campo.clone();
        window.defer(cx, move |window, cx| {
            focar.update(cx, |campo, cx| campo.focus(window, cx));
        });
        // O nome inteiro selecionado: digitar já troca, como no Finder. Só
        // depois de o campo ser desenhado — antes, a ação não acha a quem chegar.
        let foco = gpui_kit::Focusable::focus_handle(campo.read(cx), cx);
        cx.spawn_in(window, async move |_raiz, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;
            let _ = cx.update(|window, cx| {
                foco.dispatch_action(&gpui_kit::component::input::SelectAll, window, cx);
            });
        })
        .detach();
        // 🔑 **O foco volta para a raiz**, e não para "quem o tinha": pelo menu
        // de contexto, quem o tinha é o próprio menu, que já sumiu.
        self.edicao_das_guias.renome.abrir_devolvendo_a(
            Renome {
                id,
                campo,
                _assinatura: assinatura,
            },
            self.foco.clone(),
            window,
        );
        cx.notify();
    }

    /// Enter, ou clicar fora: o nome digitado vale. Vazio volta ao da sessão.
    ///
    /// Com `window` (o Enter) o foco volta à raiz; sem ela (o `Blur`), ele
    /// fica onde o operador clicou.
    pub fn confirmar_renome_da_guia(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let renome = match window {
            Some(window) => self.edicao_das_guias.renome.fechar(window, cx),
            None => self.edicao_das_guias.renome.largar(),
        };
        let Some(renome) = renome else {
            return;
        };
        let nome = renome.campo.read(cx).value().to_string();
        self.renomear_guia(&renome.id, &nome, cx);
    }

    /// Esc: a guia fica com o nome que tinha.
    pub fn cancelar_renome_da_guia(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edicao_das_guias.renome.fechar(window, cx).is_some() {
            cx.notify();
        }
    }

    pub fn renomear_guia(&mut self, id: &str, nome: &str, cx: &mut Context<Self>) {
        if self.guias.apelidar(id, nome) {
            self.guardar_guias();
        }
        cx.notify();
    }

    pub fn colorir_guia(&mut self, id: &str, cor: Option<ColorLabel>, cx: &mut Context<Self>) {
        if self.guias.colorir(id, cor) {
            self.guardar_guias();
            cx.notify();
        }
    }

    pub fn mover_guia(&mut self, id: &str, destino: usize, cx: &mut Context<Self>) {
        if self.guias.mover(id, destino) {
            self.guardar_guias();
            cx.notify();
        }
    }

    /// "Fechar as outras" e "Fechar as da direita".
    ///
    /// 🔑 **Se a da frente está entre as que fecham, a do menu vem à frente
    /// antes** — é para ela que o operador apontou, e fechar uma a uma levaria
    /// a vizinhas que também vão fechar, abrindo sessões à toa.
    pub fn fechar_guias(
        &mut self,
        fica: String,
        fecham: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let da_frente = matches!(self.tela, Tela::Sessao | Tela::Revelacao)
            .then(|| self.sessao_aberta.clone())
            .flatten();
        if da_frente.is_some_and(|f| fecham.contains(&f)) {
            self.ir_para_a_guia(fica, window, cx);
        }
        for id in &fecham {
            self.guias.fechar(id);
        }
        self.guardar_guias();
        cx.notify();
    }

    fn menu_da_guia(&self, id: &str) -> Option<MenuDaGuia> {
        let guia = self.guias.guia(id)?;
        Some(MenuDaGuia {
            id: guia.id.clone(),
            nome: guia.nome().unwrap_or("Carregando…").to_string(),
            apelidada: guia.apelido.is_some(),
            cor: guia.cor.clone(),
            posicao: self.guias.posicao(id)?,
            total: self.guias.lista().len(),
        })
    }

    /// Um gesto do roteiro de depuração (`guias …`).
    ///
    /// ⚠️ O `ContextMenu` não abre por fora: o `menu` monta **o mesmo** menu e
    /// o desenha sobre a guia, como o `tira menu` da Revelação.
    pub(super) fn seguir_o_roteiro_das_guias(
        &mut self,
        gesto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let partes: Vec<&str> = gesto.split_whitespace().collect();
        let n = |i: usize| partes.get(i).and_then(|p| p.parse::<usize>().ok());
        let id = n(1).and_then(|i| self.guias.lista().get(i).map(|g| g.id.clone()));
        match (partes.first().copied().unwrap_or_default(), id) {
            ("menu", Some(id)) => {
                let Some(dados) = self.menu_da_guia(&id) else {
                    return;
                };
                let onde = self
                    .edicao_das_guias
                    .desenhadas
                    .borrow()
                    .get(n(1).unwrap_or(0))
                    .copied();
                let Some(onde) = onde else {
                    return;
                };
                let esta = cx.entity().downgrade();
                let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                    montar_o_menu(menu, dados, esta, window, cx)
                });
                self.edicao_das_guias.menu_do_roteiro = Some((menu, onde.center()));
            }
            ("fechar_menu", _) => self.edicao_das_guias.menu_do_roteiro = None,
            ("renomear", Some(id)) => self.comecar_a_renomear_guia(id, window, cx),
            ("nome", Some(id)) => self.renomear_guia(&id, &partes[2..].join(" "), cx),
            ("cor", Some(id)) => {
                let cor = partes.get(2).and_then(|c| ColorLabel::from_name(c).ok());
                self.colorir_guia(&id, cor, cx);
            }
            ("mover", Some(id)) => self.mover_guia(&id, n(2).unwrap_or(0), cx),
            (outro, _) => eprintln!("[roteiro] gesto das guias desconhecido: {gesto} ({outro})"),
        }
        cx.notify();
    }

    pub fn apelidos_das_guias_para_teste(&self) -> Vec<Option<String>> {
        self.guias
            .lista()
            .iter()
            .map(|g| g.apelido.clone())
            .collect()
    }

    pub fn cores_das_guias_para_teste(&self) -> Vec<Option<String>> {
        self.guias.lista().iter().map(|g| g.cor.clone()).collect()
    }

    pub fn renomeando_guia_para_teste(&self) -> Option<&str> {
        self.edicao_das_guias.renome.aberto().map(|r| r.id.as_str())
    }

    // ── O desenho ────────────────────────────────────────────────────────

    /// A faixa das guias, sobre a galeria, a lista de sessões e a Revelação
    /// de uma sessão.
    ///
    /// 🔑 **Só aparece com guia aberta**: quem trabalha com uma sessão por vez
    /// não ganha uma faixa a mais na tela.
    pub(super) fn faixa_das_guias(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let na_tela = match self.tela {
            Tela::Sessao | Tela::Sessoes => true,
            Tela::Revelacao => self.sessao_aberta.is_some(),
            _ => false,
        };
        if self.guias.vazia() || !na_tela {
            return None;
        }
        let tema = cx.theme();
        let (borda, fundo_da_faixa, fundo_ativo) = (tema.border, tema.muted, tema.background);
        let (frente, apagado, destaque) = (tema.foreground, tema.muted_foreground, tema.primary);
        let da_frente = matches!(self.tela, Tela::Sessao | Tela::Revelacao)
            .then(|| self.sessao_aberta.clone())
            .flatten();

        let renome = self
            .edicao_das_guias
            .renome
            .aberto()
            .map(|r| (r.id.clone(), r.campo.clone()));

        let guias = self.guias.lista().iter().enumerate().map(|(i, guia)| {
            let ativa = da_frente.as_deref() == Some(guia.id.as_str());
            let revelando = if ativa {
                self.tela == Tela::Revelacao
            } else {
                self.guias.em_revelacao(&guia.id)
            };
            let por_salvar = if ativa {
                self.a_subir.len()
            } else {
                self.guias.por_salvar(&guia.id)
            };
            let titulo: SharedString = guia.nome().unwrap_or("Carregando…").to_string().into();
            let dica = match (i, por_salvar) {
                (0..=7, 0) => format!("{titulo}  ·  ⌘{}", i + 1),
                (_, 0) => titulo.to_string(),
                (_, n) => format!("{titulo}  ·  {n} revelação(ões) por salvar"),
            };
            let cor = guia.cor.as_deref().and_then(cores::etiqueta);
            let editando = renome
                .as_ref()
                .filter(|(id, _)| id == &guia.id)
                .map(|(_, campo)| campo.clone());
            let id_clique = guia.id.clone();
            let id_meio = guia.id.clone();
            let id_fechar = guia.id.clone();
            let id_direito = guia.id.clone();
            let id_sob_o_arrasto = guia.id.clone();
            let arrasto = ArrastoDeGuia {
                id: guia.id.clone(),
                nome: titulo.clone(),
            };
            h_flex()
                .id(SharedString::from(format!("guia-{}", guia.id)))
                // Para o teste arrastar onde o dedo arrasta.
                .debug_selector({
                    let id = guia.id.clone();
                    move || format!("guia-{id}")
                })
                .relative()
                .flex_1()
                .min_w(px(LARGURA_MINIMA))
                .max_w(px(LARGURA_MAXIMA))
                .h_full()
                .pl(px(12.))
                .pr(px(4.))
                .gap(px(6.))
                .border_r_1()
                .border_color(borda)
                .cursor_pointer()
                .text_sm()
                .when(ativa, |g| {
                    g.bg(fundo_ativo)
                        .text_color(frente)
                        .font_weight(FontWeight::MEDIUM)
                        .border_t_2()
                        .border_color(destaque)
                })
                .when(!ativa, |g| {
                    g.text_color(apagado)
                        .when_some(cor, |g, cor| g.bg(cor.opacity(0.10)))
                        .hover(move |g| g.bg(fundo_ativo.opacity(0.5)).text_color(frente))
                })
                // 🎨 A cor é uma faixa no alto da guia, como o grupo de abas do
                // Chrome — por cima da borda da ativa, que continua dizendo
                // "esta é a da frente" pelo fundo e pelo peso da letra.
                .when_some(cor, |g, cor| {
                    g.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(3.))
                            .bg(cor),
                    )
                })
                .when(editando.is_none(), |g| {
                    g.tooltip(move |window, cx| Tooltip::new(dica.clone()).build(window, cx))
                })
                .on_click(
                    cx.listener(move |raiz, evento: &gpui_kit::ClickEvent, window, cx| {
                        if raiz.renomeando_guia_para_teste() == Some(id_clique.as_str()) {
                            return;
                        }
                        raiz.ir_para_a_guia(id_clique.clone(), window, cx);
                        // O duplo clique no nome renomeia, como no Finder.
                        if evento.click_count() == 2 {
                            raiz.comecar_a_renomear_guia(id_clique.clone(), window, cx);
                        }
                    }),
                )
                // O botão do meio fecha, como em todo navegador.
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |raiz, _, window, cx| {
                        raiz.fechar_guia(id_meio.clone(), window, cx);
                    }),
                )
                // O botão direito anota qual guia foi; o menu da faixa lê.
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |raiz, _, _window, _cx| {
                        raiz.edicao_das_guias.alvo_do_menu = Some(id_direito.clone());
                    }),
                )
                // 🔑 **Arrastar muda a guia de lugar ao vivo**: passar sobre a
                // vizinha já troca as duas, como no navegador, e a ordem nova é
                // gravada na hora — soltar fora da faixa não a desfaz.
                .when(editando.is_none(), |g| {
                    g.on_drag(arrasto, |valor, _posicao, _window, cx| {
                        let nome = valor.nome.clone();
                        cx.new(|_| FantasmaDaGuia { nome })
                    })
                })
                .on_drag_move(cx.listener(
                    move |raiz, evento: &DragMoveEvent<ArrastoDeGuia>, _window, cx| {
                        if !evento.bounds.contains(&evento.event.position) {
                            return;
                        }
                        let arrastada = evento.drag(cx).id.clone();
                        if raiz.guias.passar_sobre(&arrastada, &id_sob_o_arrasto) {
                            raiz.guardar_guias();
                            cx.notify();
                        }
                    },
                ))
                .child(
                    // 🎞️ A guia que está (ou ficou) na Revelação troca a câmera
                    // pelo ícone do "Revelar": volta-se nela no editor.
                    Icon::new(if revelando {
                        Icone::SlidersHorizontal
                    } else {
                        Icone::Camera
                    })
                    .size(px(14.))
                    .flex_none()
                    .when_some(cor, |i, cor| i.text_color(cor)),
                )
                .map(|g| match editando {
                    // Enter confirma, Esc desiste, clicar fora confirma.
                    Some(campo) => g.child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .on_action(cx.listener(|raiz, _: &Escape, window, cx| {
                                raiz.cancelar_renome_da_guia(window, cx)
                            }))
                            .child(Input::new(&campo).xsmall()),
                    ),
                    None => g.child(div().flex_1().min_w(px(0.)).truncate().child(titulo)),
                })
                // 🔑 O ponto diz "tem revelação por salvar", como o ponto oco da
                // tira — trocar de guia não pode esconder isso.
                .when(por_salvar > 0, |g| {
                    g.child(div().flex_none().size(px(6.)).rounded_full().bg(destaque))
                })
                .child(
                    div()
                        .id(SharedString::from(format!("fechar-guia-{}", guia.id)))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(px(4.))
                        .hover(move |b| b.bg(borda))
                        .child(Icon::new(Icone::X).size(px(12.)))
                        .on_click(cx.listener(move |raiz, _, window, cx| {
                            cx.stop_propagation();
                            raiz.fechar_guia(id_fechar.clone(), window, cx);
                        })),
                )
        });

        let esta = cx.entity().downgrade();
        let desenhadas = self.edicao_das_guias.desenhadas.clone();
        let menu_do_roteiro = self.edicao_das_guias.menu_do_roteiro.clone();
        // 🪟 **Com a faixa na tela, ela é a primeira linha da janela, e os
        // botões de janela moram na ponta dela** — como no Zed e no Chrome do
        // GNOME. Fora da parte que rola: guia demais não os empurra para fora.
        let controles = crate::janela::controles("janela-guias", frente, window, cx);
        let faixa = h_flex()
            .on_children_prepainted(move |limites, _window, _cx| {
                *desenhadas.borrow_mut() = limites;
            })
            .id("faixa-das-guias")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_x_scroll()
            .children(guias)
            .child(
                div()
                    .id("guia-nova")
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(ALTURA_DA_FAIXA))
                    .text_color(apagado)
                    .cursor_pointer()
                    .hover(move |b| b.text_color(frente).bg(fundo_ativo.opacity(0.5)))
                    .tooltip(|window, cx| {
                        Tooltip::new("Abrir outra sessão numa guia").build(window, cx)
                    })
                    .child(Icon::new(Icone::Plus).size(px(16.)))
                    .on_click(cx.listener(|raiz, _, window, cx| {
                        raiz.abrir_guia_nova(window, cx);
                    })),
            )
            .children(menu_do_roteiro.map(|(menu, ponto)| {
                gpui_kit::deferred(gpui_kit::anchored().position(ponto).child(menu))
                    .with_priority(1)
            }))
            // 🔑 **Um menu para a faixa inteira**, como o da tira da
            // Revelação: o `ContextMenu` guarda estado por id, e um por guia
            // seriam vários com o mesmo. Fora de uma guia (no `+`, no vão),
            // não há alvo e o menu não abre.
            .context_menu(move |menu, window, cx| {
                // 🚨 Devolver o foco ao fechar — ver o menu da tira da Revelação:
                // sem o `action_context`, as teclas da raiz morriam depois dele.
                let menu = match window.focused(cx) {
                    Some(antes) => menu.action_context(antes),
                    None => menu,
                };
                let Some(dados) = esta
                    .update(cx, |raiz, _cx| {
                        raiz.edicao_das_guias
                            .alvo_do_menu
                            .take()
                            .and_then(|id| raiz.menu_da_guia(&id))
                    })
                    .ok()
                    .flatten()
                else {
                    return menu;
                };
                montar_o_menu(menu, dados, esta.clone(), window, cx)
            });
        Some(
            h_flex()
                .flex_none()
                .w_full()
                .h(px(ALTURA_DA_FAIXA))
                .bg(fundo_da_faixa)
                .border_b_1()
                .border_color(borda)
                .child(faixa)
                .child(controles),
        )
    }
}

/// O menu do botão direito numa guia.
fn montar_o_menu(
    menu: PopupMenu,
    dados: MenuDaGuia,
    esta: WeakEntity<Aplicativo>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    type Gesto = fn(&mut Aplicativo, String, &mut Window, &mut Context<Aplicativo>);
    let com = |gesto: Gesto| {
        let (esta, id) = (esta.clone(), dados.id.clone());
        move |_ev: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut gpui_kit::App| {
            let _ = esta.update(cx, |raiz, cx| gesto(raiz, id.clone(), window, cx));
        }
    };
    let ultima = dados.posicao + 1 == dados.total;
    let (posicao, cor_atual) = (dados.posicao, dados.cor.clone());
    let (esta_cor, id_cor) = (esta.clone(), dados.id.clone());

    menu.label(dados.nome)
        .separator()
        .item(
            PopupMenuItem::new("Renomear guia…").on_click(com(|_raiz, id, window, cx| {
                // Depois de o menu fechar: é ele quem devolve o foco ao fechar.
                let esta = cx.entity();
                window.defer(cx, move |window, cx| {
                    esta.update(cx, |raiz, cx| raiz.comecar_a_renomear_guia(id, window, cx));
                });
            })),
        )
        .item(
            PopupMenuItem::new("Voltar ao nome da sessão")
                .disabled(!dados.apelidada)
                .on_click(com(|raiz, id, _window, cx| raiz.renomear_guia(&id, "", cx))),
        )
        .submenu("Cor da guia", window, cx, move |sub, _window, _cx| {
            let esta = esta_cor.clone();
            let id = id_cor.clone();
            let pintar = move |cor: Option<ColorLabel>| {
                let (esta, id) = (esta.clone(), id.clone());
                move |_ev: &gpui_kit::ClickEvent, _window: &mut Window, cx: &mut gpui_kit::App| {
                    let _ = esta.update(cx, |raiz, cx| raiz.colorir_guia(&id, cor, cx));
                }
            };
            let sub = sub.item(
                PopupMenuItem::new("Sem cor")
                    .checked(cor_atual.is_none())
                    .on_click(pintar(None)),
            );
            CORES.iter().fold(sub, |sub, (cor, nome)| {
                sub.item(
                    PopupMenuItem::new(*nome)
                        .checked(cor_atual.as_deref() == Some(cor.name()))
                        .on_click(pintar(Some(*cor))),
                )
            })
        })
        .separator()
        .item(
            PopupMenuItem::new("Mover para a esquerda")
                .disabled(posicao == 0)
                .on_click({
                    let (esta, id) = (esta.clone(), dados.id.clone());
                    move |_ev, _window, cx| {
                        let _ = esta.update(cx, |raiz, cx| {
                            raiz.mover_guia(&id, posicao.saturating_sub(1), cx)
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Mover para a direita")
                .disabled(ultima)
                .on_click({
                    let (esta, id) = (esta.clone(), dados.id.clone());
                    move |_ev, _window, cx| {
                        let _ = esta.update(cx, |raiz, cx| raiz.mover_guia(&id, posicao + 1, cx));
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new("Fechar guia")
                .on_click(com(|raiz, id, window, cx| raiz.fechar_guia(id, window, cx))),
        )
        .item(
            PopupMenuItem::new("Fechar as outras guias")
                .disabled(dados.total < 2)
                .on_click(com(|raiz, id, window, cx| {
                    let fecham = raiz.guias.outras(&id);
                    raiz.fechar_guias(id, fecham, window, cx);
                })),
        )
        .item(
            PopupMenuItem::new("Fechar as guias à direita")
                .disabled(ultima)
                .on_click(com(|raiz, id, window, cx| {
                    let fecham = raiz.guias.a_direita(&id);
                    raiz.fechar_guias(id, fecham, window, cx);
                })),
        )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pendente(id: &str) -> Pendente {
        (id.into(), Ajustes::default(), CropSettings::default())
    }

    fn ids(guias: &Guias) -> Vec<&str> {
        guias.lista().iter().map(|g| g.id.as_str()).collect()
    }

    #[test]
    fn abrir_a_mesma_sessao_duas_vezes_nao_duplica_a_guia() {
        let mut guias = Guias::default();
        assert!(guias.abrir("g1", None));
        assert!(!guias.abrir("g1", Some("g1")));
        assert_eq!(ids(&guias), ["g1"]);
    }

    #[test]
    fn a_guia_nova_entra_logo_depois_da_ativa() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.abrir("g2", Some("g1"));
        guias.abrir("g3", Some("g1"));
        assert_eq!(ids(&guias), ["g1", "g3", "g2"]);
        // Sem ativa (vindo da lista), vai para o fim.
        guias.abrir("g4", None);
        assert_eq!(ids(&guias), ["g1", "g3", "g2", "g4"]);
    }

    #[test]
    fn fechar_passa_para_a_direita_e_na_ultima_para_a_esquerda() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.fechar("g2").as_deref(), Some("g3"));
        assert_eq!(guias.fechar("g3").as_deref(), Some("g1"));
        assert_eq!(guias.fechar("g1"), None);
        assert!(guias.vazia());
        assert_eq!(guias.fechar("nenhuma"), None);
    }

    #[test]
    fn ctrl_tab_da_a_volta_nas_pontas() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.vizinha(Some("g3"), 1).as_deref(), Some("g1"));
        assert_eq!(guias.vizinha(Some("g1"), -1).as_deref(), Some("g3"));
        assert_eq!(guias.vizinha(None, 1).as_deref(), Some("g1"));
        assert_eq!(Guias::default().vizinha(None, 1), None);
    }

    #[test]
    fn cmd_9_e_sempre_a_ultima() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.na_posicao(0).as_deref(), Some("g1"));
        assert_eq!(guias.na_posicao(8).as_deref(), Some("g3"));
        assert_eq!(guias.na_posicao(5), None);
    }

    #[test]
    fn a_fila_do_salvar_volta_com_a_guia_e_nao_vaza_para_a_vizinha() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.abrir("g2", None);
        guias.estacionar("g1", vec![pendente("f1"), pendente("f2")]);
        assert_eq!(guias.por_salvar("g1"), 2);
        assert!(guias.retomar("g2").is_empty());
        let fila = guias.retomar("g1");
        assert_eq!(fila.len(), 2);
        // Retomada, sai do estacionamento: não volta duas vezes.
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn a_foto_que_subiu_sai_da_fila_estacionada() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.estacionar("g1", vec![pendente("f1"), pendente("f2")]);
        guias.deu_baixa("f1");
        assert_eq!(guias.por_salvar("g1"), 1);
        guias.deu_baixa("f2");
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn fechar_a_guia_esquece_a_fila_dela() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.estacionar("g1", vec![pendente("f1")]);
        guias.fechar("g1");
        guias.abrir("g1", None);
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn as_guias_so_voltam_para_a_mesma_conta() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.dar_nome("g1", "Casamento Ana");
        let json = guias.em_json(Some("a@x.com"));
        let de_volta = Guias::de_json(&json, "a@x.com");
        assert_eq!(de_volta.len(), 1);
        assert_eq!(de_volta[0].titulo.as_deref(), Some("Casamento Ana"));
        assert!(Guias::de_json(&json, "outra@x.com").is_empty());
        assert!(Guias::de_json("lixo", "a@x.com").is_empty());
    }

    #[test]
    fn nome_vazio_nao_apaga_o_nome_da_guia() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        assert!(guias.dar_nome("g1", "Ensaio"));
        assert!(!guias.dar_nome("g1", "   "));
        assert!(!guias.dar_nome("g1", "Ensaio"));
        assert_eq!(guias.lista()[0].titulo.as_deref(), Some("Ensaio"));
    }

    fn tres() -> Guias {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        guias
    }

    #[test]
    fn o_apelido_manda_no_nome_e_vazio_volta_ao_da_sessao() {
        let mut guias = tres();
        guias.dar_nome("g1", "Ensaio da Ana");
        assert!(guias.apelidar("g1", "  Ana — prova  "));
        assert_eq!(guias.guia("g1").unwrap().nome(), Some("Ana — prova"));
        // A sessão continua com o nome dela: o site renomeia, a guia não perde o apelido.
        guias.dar_nome("g1", "Ensaio da Ana Paula");
        assert_eq!(guias.guia("g1").unwrap().nome(), Some("Ana — prova"));
        assert!(guias.apelidar("g1", ""));
        assert_eq!(
            guias.guia("g1").unwrap().nome(),
            Some("Ensaio da Ana Paula")
        );
        // Repetir o nome da sessão não vira apelido.
        assert!(!guias.apelidar("g1", "Ensaio da Ana Paula"));
        assert_eq!(guias.guia("g1").unwrap().apelido, None);
        assert!(!guias.apelidar("nenhuma", "x"));
    }

    #[test]
    fn a_cor_e_uma_das_etiquetas_e_sai_com_sem_cor() {
        let mut guias = tres();
        assert!(guias.colorir("g2", Some(ColorLabel::Green)));
        assert!(!guias.colorir("g2", Some(ColorLabel::Green)));
        assert_eq!(guias.guia("g2").unwrap().cor.as_deref(), Some("green"));
        assert!(cores::etiqueta("green").is_some(), "a faixa sabe pintar");
        assert!(guias.colorir("g2", None));
        assert_eq!(guias.guia("g2").unwrap().cor, None);
    }

    #[test]
    fn mover_leva_a_guia_e_para_nas_pontas() {
        let mut guias = tres();
        assert!(guias.mover("g3", 0));
        assert_eq!(ids(&guias), ["g3", "g1", "g2"]);
        assert!(guias.mover("g3", 99));
        assert_eq!(ids(&guias), ["g1", "g2", "g3"]);
        assert!(!guias.mover("g3", 2));
        assert!(!guias.mover("nenhuma", 0));
    }

    #[test]
    fn arrastar_sobre_a_vizinha_troca_de_lugar_nos_dois_sentidos() {
        let mut guias = tres();
        // Da primeira para a direita, passando por cima de cada uma.
        assert!(guias.passar_sobre("g1", "g2"));
        assert_eq!(ids(&guias), ["g2", "g1", "g3"]);
        assert!(guias.passar_sobre("g1", "g3"));
        assert_eq!(ids(&guias), ["g2", "g3", "g1"]);
        // Sobre si mesma não faz nada — o ponteiro fica sobre ela depois da troca.
        assert!(!guias.passar_sobre("g1", "g1"));
        // E de volta, pulando direto para a primeira.
        assert!(guias.passar_sobre("g1", "g2"));
        assert_eq!(ids(&guias), ["g1", "g2", "g3"]);
    }

    #[test]
    fn fechar_as_outras_e_as_da_direita_escolhem_as_certas() {
        let guias = tres();
        assert_eq!(guias.outras("g2"), ["g1", "g3"]);
        assert_eq!(guias.a_direita("g1"), ["g2", "g3"]);
        assert!(guias.a_direita("g3").is_empty());
        assert!(guias.a_direita("nenhuma").is_empty());
    }

    #[test]
    fn apelido_cor_e_ordem_voltam_na_proxima_abertura() {
        let mut guias = tres();
        guias.apelidar("g2", "Prova");
        guias.colorir("g2", Some(ColorLabel::Blue));
        guias.mover("g2", 0);
        let de_volta = Guias::de_json(&guias.em_json(Some("a@x.com")), "a@x.com");
        assert_eq!(de_volta[0].id, "g2");
        assert_eq!(de_volta[0].apelido.as_deref(), Some("Prova"));
        assert_eq!(de_volta[0].cor.as_deref(), Some("blue"));
    }

    #[test]
    fn o_arquivo_de_antes_das_cores_ainda_abre() {
        let antigo = r#"{"conta":"a@x.com","guias":[{"id":"g1","titulo":"Ensaio"}]}"#;
        let guias = Guias::de_json(antigo, "a@x.com");
        assert_eq!(guias.len(), 1);
        assert_eq!(guias[0].apelido, None);
        assert_eq!(guias[0].cor, None);
    }
}
