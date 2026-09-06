//! **Dentro** de uma sessão fotográfica — a mesma tela que a web tem em
//! `/dashboard/sessoes-fotograficas/{id}`.
//!
//! # As três partes, e a ordem delas
//!
//! O desenho é o de lá, e a ordem não é arbitrária — ela é o fluxo do balcão:
//!
//! 1. **cabeçalho**: quem é o cliente, o que a sessão tem, e os dois botões de
//!    sair (avisar e copiar o link);
//! 2. **envio**: e o **estado é escolhido antes dos arquivos**, porque são duas
//!    levas — as que o cliente levou e as que ficaram à venda;
//! 3. **grade**: as fotos que já estão no site, com o que foi marcado no balcão.
//!
//! # 🚨 Por que esta tela precisou existir
//!
//! Antes de 6/set/2026 o desktop tinha a lista de sessões e nada dentro delas:
//! "abrir" só marcava a sessão como destino das próximas classificadas. Quem
//! vinha da web achava a coisa *"muito aberta e estranha"* — e estava certo: lá
//! a sessão é **onde se trabalha**, e aqui ela era um rótulo.
//!
//! # As fotos são as do site, e não as do catálogo
//!
//! 🔑 A grade daqui mostra o que está **no storage**, com as miniaturas vindas
//! da API. É o que torna a tela verdadeira: a foto que outro computador do
//! estúdio subiu aparece aqui, e a que a retenção apagou aparece como apagada —
//! coisas que o catálogo local não tem como saber.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::acervo::{self, Acervo, Filtro};
use biblioteca_core::dinheiro;
use biblioteca_core::selecao::{Modificadores, Selecao};
use domain::services::pos_venda::{
    EstadoDaFotoNoSite, EstadoNoBalcao, FotoDaGaleria, GaleriaAberta, LinkDeAcesso, Produto, Sessao,
};
use gpui::{div, img, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use super::arquivos::SeletorDeFotos;
use crate::pos_venda::porta::{Publicador, Recado};
use crate::selos;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// O tamanho do tile, em pixels — os mesmos limites da barra do site.
const ZOOM_MINIMO: f32 = 90.0;
const ZOOM_MAXIMO: f32 = 320.0;
const ZOOM_PADRAO: f32 = 160.0;
const PASSO_DO_ZOOM: f32 = 35.0;

/// Os recortes da barra, na ordem da web.
///
/// 🚨 **"Sem nota" é o último de propósito**: é um recorte de exceção — o que
/// está sem classificação não pode ir à venda, não recebe marca d'água e não
/// devia estar no storage. Ele existe para esvaziar, não para consultar.
const FILTROS: [(&str, Filtro); 6] = [
    ("Todas", Filtro::Todas),
    ("Levadas", Filtro::Situacao(acervo::Estado::LevadaNoBalcao)),
    ("À venda", Filtro::Situacao(acervo::Estado::Disponivel)),
    ("Compradas", Filtro::Situacao(acervo::Estado::Comprada)),
    ("Apagadas", Filtro::Apagadas),
    ("Sem nota", Filtro::SemNota),
];

// 📌 A barra de recortes do site saiu daqui em 6/set/2026, junto com a grade
// duplicada: a grade passou a ser a da Biblioteca, escopada ao ensaio. Os
// recortes por situação (levadas · à venda · compradas · sem nota) são da tela
// da sessão na web e **ainda não existem na barra da Biblioteca** — é o próximo
// passo, e o `biblioteca_core::acervo` já os calcula.

/// O que a tela pede à raiz — ela não sabe trocar de tela nem abrir a Revelação.
pub enum Pedido {
    /// Voltar para a lista de sessões.
    Voltar,
    /// A miniatura de uma foto do site chegou ao cache, sob esta chave.
    MiniaturaPronta(String),
    /// Abrir (ou fechar) a segunda tela, a do cliente.
    TelaDoCliente,
    /// A sessão abriu (ou foi relida): estas são as fotos que já estão no site.
    ///
    /// 🔑 Quem as põe na grade é a raiz — a grade é uma só, e nela as do site
    /// convivem com as locais, como na web.
    FotosDoSite(Vec<domain::services::pos_venda::FotoDaGaleria>),
    /// Revelar uma foto **do site**: o id remoto, que a Revelação usa para
    /// buscar a cópia de trabalho.
    Revelar { foto_id: String, arquivo: String },
}

impl EventEmitter<Pedido> for Detalhe {}

pub struct Detalhe {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    galeria_id: Option<String>,
    aberta: Option<GaleriaAberta>,
    /// O cache de previews do app — o mesmo que a grade lê.
    ///
    /// 🔑 **A miniatura que vem do site é gravada nele**, sob a chave
    /// `site:<id>`, que é o id com que a foto entra na grade. Sem isso a grade
    /// desenharia célula vazia para tudo o que já subiu: o nome apareceria, a
    /// foto não.
    previews: Arc<PreviewManager>,
    /// De quem já foi pedida miniatura, para não pedir duas vezes.
    pedidas: std::collections::HashSet<String>,
    /// Quantas miniaturas ainda estão a caminho.
    baixando: usize,
    /// As faixas de preço do catálogo — o `select` da barra de envio.
    produtos: Vec<Produto>,
    /// A faixa escolhida para a próxima leva. `None` = a padrão da galeria.
    faixa: Option<String>,
    /// O lado do tile, em pixels.
    zoom: f32,
    /// Se o aviso ao cliente está a caminho.
    avisando: bool,
    /// Quem abre a janela **do sistema** para escolher as fotos.
    seletor: Arc<dyn SeletorDeFotos>,
    /// Por onde os caminhos escolhidos voltam.
    escolhas: (Sender<Vec<String>>, Receiver<Vec<String>>),
    /// A leva: como as próximas fotos entram.
    ///
    /// 🔑 **`None` é "sem marcação", e é o padrão** — a mesma escolha da web
    /// (`envio.tsx`, 2026-09-05): *a marcação de verdade nasce no balcão, com o
    /// cliente olhando*. Escolher aqui, antes de as fotos entrarem, é decidir
    /// por trinta de uma vez o que se decide uma a uma. Sem marcação a foto vai
    /// para o acervo **à venda**, que é o estado de quem ainda não foi levada.
    leva: Option<EstadoNoBalcao>,
    /// Se há algo sendo arrastado por cima — o destaque da área.
    arrastando: bool,
    /// O recorte, as contagens e a ordem — **a mesma conta da grade do site**
    /// (`biblioteca_core::acervo`). A tela não filtra nem conta por si.
    acervo: Acervo,
    /// O que está marcado, pelas mesmas regras da grade do site.
    selecao: Selecao,
    enviando: usize,
    enviadas: usize,
    link: Option<LinkDeAcesso>,
    carregando: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Detalhe {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        seletor: Arc<dyn SeletorDeFotos>,
        previews: Arc<PreviewManager>,
    ) -> Self {
        Self {
            publicador,
            seletor,
            escolhas: channel(),
            leva: None,
            arrastando: false,
            acervo: Acervo::novo(),
            selecao: Selecao::nova(),
            sessao: None,
            galeria_id: None,
            aberta: None,
            previews,
            pedidas: std::collections::HashSet::new(),
            baixando: 0,
            produtos: Vec::new(),
            faixa: None,
            zoom: ZOOM_PADRAO,
            avisando: false,
            enviando: 0,
            enviadas: 0,
            link: None,
            carregando: false,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// Entra numa sessão: pede a galeria e as fotos dela.
    pub fn entrar(&mut self, galeria_id: String, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            self.erro = Some("esta tela precisa da conta do site".into());
            cx.notify();
            return;
        };
        // 🔑 Tudo o que era da sessão anterior sai: miniatura e link de outra
        // galeria na tela desta seria o pior tipo de erro — o que parece certo.
        self.galeria_id = Some(galeria_id.clone());
        self.aberta = None;
        self.pedidas.clear();
        self.baixando = 0;
        self.link = None;
        self.enviadas = 0;
        self.erro = None;
        self.carregando = true;

        self.publicador
            .abrir_galeria(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn galeria_id(&self) -> Option<&str> {
        self.galeria_id.as_deref()
    }

    pub fn aberta(&self) -> Option<&GaleriaAberta> {
        self.aberta.as_ref()
    }

    pub fn link(&self) -> Option<&LinkDeAcesso> {
        self.link.as_ref()
    }

    pub fn erro(&self) -> Option<&SharedString> {
        self.erro.as_ref()
    }

    /// O recorte em vigor.
    pub fn filtro(&self) -> Filtro {
        self.acervo.filtro()
    }

    pub fn filtrar(&mut self, filtro: Filtro, cx: &mut Context<Self>) {
        self.acervo.filtrar(filtro);
        // 🔑 **Trocar o recorte limpa a seleção** — a mesma regra do site: o que
        // se vê é o que se opera, e as posições passam a apontar outras fotos.
        self.selecao.limpar_tudo();
        cx.notify();
    }

    pub fn contagens(&self) -> acervo::Contagens {
        self.acervo.contagens()
    }

    /// O clique numa foto da grade — as regras são as do core.
    pub fn clicar(&mut self, posicao: usize, modificadores: Modificadores, cx: &mut Context<Self>) {
        self.selecao.clicar(posicao, false, modificadores);
        cx.notify();
    }

    /// "Selecionar as N visíveis" — e o mesmo botão desmarca quando já estão
    /// todas, porque é o mesmo botão.
    pub fn alternar_todas(&mut self, cx: &mut Context<Self>) {
        self.selecao.alternar_todas(self.acervo.total_visivel());
        cx.notify();
    }

    pub fn quantas_marcadas(&self) -> usize {
        self.selecao.quantas()
    }

    /// Os ids no site das fotos marcadas, na ordem da grade.
    pub fn marcadas(&self) -> Vec<String> {
        self.selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p).map(|f| f.id.clone()))
            .collect()
    }

    /// Muda o estado das marcadas — as ações em lote da barra.
    ///
    /// ⚠️ **A comprada e a apagada ficam de fora**, e quem decide isso é o core
    /// (`Foto::editavel`): a comprada tem cobrança atrás dela, e a apagada não
    /// tem arquivo. Mandar assim mesmo traria um erro por foto.
    pub fn marcar_como(&mut self, estado: EstadoNoBalcao, cx: &mut Context<Self>) {
        self.mudar_as_marcadas(
            domain::services::pos_venda::MudancaDaFoto {
                estado: Some(estado),
                ..Default::default()
            },
            cx,
        );
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn ajustar_zoom(&mut self, passo: f32, cx: &mut Context<Self>) {
        self.zoom = (self.zoom + passo).clamp(ZOOM_MINIMO, ZOOM_MAXIMO);
        cx.notify();
    }

    pub fn produtos(&self) -> &[Produto] {
        &self.produtos
    }

    pub fn faixa(&self) -> Option<&str> {
        self.faixa.as_deref()
    }

    pub fn escolher_faixa(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.faixa = id;
        cx.notify();
    }

    /// A foto em foco — a que o painel da direita descreve.
    pub fn em_foco(&self) -> Option<&acervo::Foto> {
        self.selecao.foco().and_then(|p| self.acervo.visivel(p))
    }

    pub fn posicao_em_foco(&self) -> Option<usize> {
        self.selecao.foco()
    }

    pub fn total_visivel(&self) -> usize {
        self.acervo.total_visivel()
    }

    /// As setas da tira: um passo, sem dar a volta.
    pub fn andar(&mut self, passo: i32, cx: &mut Context<Self>) {
        let total = self.acervo.total_visivel();
        if total == 0 {
            return;
        }
        let nova = match (self.selecao.foco(), passo > 0) {
            (Some(i), true) => (i + 1).min(total - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => 0,
            (None, false) => total - 1,
        };
        self.selecao.clicar(nova, false, Modificadores::default());
        cx.notify();
    }

    /// `1`–`5` dão a nota; `0` a tira.
    ///
    /// 🚨 **Tirar a nota de uma foto do acervo é removê-la**, e o site recusa
    /// `nota: null` justamente por isso — foi a classificação que a autorizou a
    /// subir. Aqui a tecla `0` avisa, em vez de mandar um pedido que voltaria
    /// recusado.
    pub fn dar_nota(&mut self, nota: u8, cx: &mut Context<Self>) {
        if nota == 0 {
            self.erro =
                Some("tirar a nota de uma foto do acervo é removê-la do site — use Apagar".into());
            cx.notify();
            return;
        }
        self.mudar_as_marcadas(
            domain::services::pos_venda::MudancaDaFoto {
                nota: Some(Some(nota as i16)),
                ..Default::default()
            },
            cx,
        );
    }

    /// `P`: levada no balcão, e o mesmo gesto devolve à venda.
    pub fn alternar_levada(&mut self, cx: &mut Context<Self>) {
        let todas_levadas = self
            .selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p))
            .all(|f| f.estado == acervo::Estado::LevadaNoBalcao);
        let estado = if todas_levadas {
            EstadoNoBalcao::Disponivel
        } else {
            EstadoNoBalcao::LevadaNoBalcao
        };
        self.marcar_como(estado, cx);
    }

    pub fn limpar_selecao(&mut self, cx: &mut Context<Self>) {
        self.selecao.desmarcar();
        cx.notify();
    }

    pub fn selecionar_tudo(&mut self, cx: &mut Context<Self>) {
        self.selecao.marcar_todas(self.acervo.total_visivel());
        cx.notify();
    }

    /// Manda a mesma mudança para todas as marcadas que ainda podem mudar.
    ///
    /// ⚠️ **A comprada e a apagada ficam de fora**, e quem decide é o core
    /// (`Foto::editavel`): a comprada tem cobrança atrás dela, e a apagada não
    /// tem arquivo.
    fn mudar_as_marcadas(
        &mut self,
        mudanca: domain::services::pos_venda::MudancaDaFoto,
        cx: &mut Context<Self>,
    ) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let alvos: Vec<String> = self
            .selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p))
            .filter(|f| f.editavel())
            .map(|f| f.id.clone())
            .collect();
        if alvos.is_empty() || self.enviando > 0 {
            return;
        }

        self.erro = None;
        self.enviadas = 0;
        self.enviando = alvos.len();
        for id in alvos {
            self.publicador
                .negociar(sessao.clone(), id, mudanca.clone(), self.recados.0.clone());
        }
        self.acompanhar(cx);
        cx.notify();
    }

    /// Quantas fotos há em cada estado — o que o cabeçalho conta.
    pub fn contagem(&self) -> (usize, usize, usize) {
        let fotos = self
            .aberta
            .as_ref()
            .map(|a| a.fotos.as_slice())
            .unwrap_or(&[]);
        let levadas = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::LevadaNoBalcao)
            .count();
        let a_venda = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::Disponivel)
            .count();
        let compradas = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::Comprada)
            .count();
        (levadas, a_venda, compradas)
    }

    /// A leva escolhida para as próximas fotos.
    pub fn escolher_leva(&mut self, leva: Option<EstadoNoBalcao>, cx: &mut Context<Self>) {
        self.leva = leva;
        cx.notify();
    }

    pub fn leva(&self) -> Option<EstadoNoBalcao> {
        self.leva
    }

    /// Abre a janela **do sistema** para escolher as fotos.
    pub fn escolher_fotos(&mut self, cx: &mut Context<Self>) {
        if self.enviando > 0 {
            return;
        }
        self.seletor.escolher(self.escolhas.0.clone());
        self.acompanhar(cx);
    }

    pub fn arrastando(&self) -> bool {
        self.arrastando
    }

    pub fn destacar(&mut self, arrastando: bool, cx: &mut Context<Self>) {
        if self.arrastando != arrastando {
            self.arrastando = arrastando;
            cx.notify();
        }
    }

    /// Sobe os arquivos escolhidos — do seletor ou do que foi arrastado.
    ///
    /// 🔑 **São arquivos do disco, e não fotos do catálogo.** É o envio da web:
    /// o operador exporta do Lightroom para uma pasta e manda a pasta. O que vai
    /// ao cliente não precisa estar catalogado aqui — o catálogo é da triagem em
    /// RAW, e são dois trabalhos diferentes.
    pub fn enviar_arquivos(&mut self, caminhos: Vec<String>, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        if caminhos.is_empty() || self.enviando > 0 {
            return;
        }

        // A ordem no site continua de onde a sessão parou.
        let ja_na_sessao = self.aberta.as_ref().map(|a| a.fotos.len()).unwrap_or(0);
        // Sem marcação vai como "à venda" — o estado de quem ainda não foi levada.
        let estado = self.leva.unwrap_or(EstadoNoBalcao::Disponivel);

        self.erro = None;
        self.enviadas = 0;
        self.enviando = caminhos.len();
        for (i, caminho) in caminhos.into_iter().enumerate() {
            self.publicador.enviar_arquivo(
                sessao.clone(),
                galeria_id.clone(),
                caminho,
                (ja_na_sessao + i) as u32,
                estado,
                self.recados.0.clone(),
            );
        }
        self.acompanhar(cx);
        cx.notify();
    }

    /// 📧 Manda o e-mail "suas fotos estão prontas".
    ///
    /// ⚠️ **Quem escreve e manda é o site**; o app só pede. A falha do aviso não
    /// é falha da sessão — as fotos continuam no ar, e o operador reenvia.
    pub fn avisar(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        if self.avisando {
            return;
        }
        self.avisando = true;
        self.publicador
            .avisar(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn pedir_o_link(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        self.publicador
            .link(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Pede as miniaturas que ainda faltam — uma vez cada.
    fn pedir_miniaturas(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let faltam: Vec<String> = self
            .aberta
            .as_ref()
            .map(|a| {
                a.fotos
                    .iter()
                    .filter(|f| !f.apagada && !self.pedidas.contains(&f.id))
                    .map(|f| f.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        for id in faltam {
            self.pedidas.insert(id.clone());
            self.baixando += 1;
            self.publicador
                .miniatura(sessao.clone(), id, self.recados.0.clone());
        }
        if !self.pedidas.is_empty() {
            self.acompanhar(cx);
        }
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

    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        let mut abriu = false;

        // O que o seletor do sistema devolveu. Lista vazia é desistência, e não
        // erro: fechar a janela sem escolher é um gesto legítimo.
        while let Ok(caminhos) = self.escolhas.1.try_recv() {
            mudou = true;
            if !caminhos.is_empty() {
                self.enviar_arquivos(caminhos, cx);
            }
        }

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Aberta(aberta) => {
                    self.carregando = false;
                    // 🔑 A conta é do core: recorte, contagens e ordem saem
                    // dele, e não de laços escritos aqui.
                    self.acervo
                        .definir(aberta.fotos.iter().map(para_o_core).collect());
                    // Trocar o conteúdo embaralha as posições: a seleção fala em
                    // posição, e mantê-la apontaria para outras fotos.
                    self.selecao.limpar_tudo();
                    cx.emit(Pedido::FotosDoSite(aberta.fotos.clone()));
                    self.aberta = Some(*aberta);
                    abriu = true;
                }
                Recado::Miniatura { foto_id, bytes } => {
                    self.baixando = self.baixando.saturating_sub(1);
                    // Miniatura ilegível não derruba a grade: a célula fica sem
                    // imagem, com o nome do arquivo, que é melhor que nada.
                    if let Ok(imagem) = image::load_from_memory(&bytes) {
                        let chave = format!("site:{foto_id}");
                        if self.previews.save_preview(&chave, &imagem).is_ok() {
                            // A grade guarda "ausente" para quem ainda não tinha
                            // miniatura; sem avisar, a foto recém-baixada só
                            // apareceria quando a célula saísse e voltasse.
                            cx.emit(Pedido::MiniaturaPronta(chave));
                        }
                    }
                }
                Recado::Sincronizou => {
                    self.avisando = false;
                    self.enviadas += 1;
                    self.enviando = self.enviando.saturating_sub(1);
                    if self.enviando == 0 {
                        // O lote acabou: reler a sessão é o que faz as novas
                        // aparecerem na grade.
                        if let (Some(sessao), Some(id)) =
                            (self.sessao.clone(), self.galeria_id.clone())
                        {
                            self.publicador
                                .abrir_galeria(sessao, id, self.recados.0.clone());
                            self.carregando = true;
                        }
                    }
                }
                Recado::Link(link) => {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.url.clone()));
                    self.link = Some(link);
                }
                Recado::Falhou(erro) => {
                    self.carregando = false;
                    self.enviando = self.enviando.saturating_sub(1);
                    self.erro = Some(erro.into());
                }
                _ => {}
            }
        }

        if abriu {
            self.pedir_miniaturas(cx);
        }
        if mudou {
            cx.notify();
        }
        // 🔑 O laço para quando não há mais resposta a esperar. As miniaturas
        // não entram na conta: elas chegam pelo mesmo canal, e o `abriu` religa
        // o laço quando um lote novo é pedido.
        let continua = self.carregando || self.enviando > 0 || self.baixando > 0 || self.avisando;
        if !continua {
            self.colhendo = false;
        }
        continua
    }
}

impl Render for Detalhe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            // 🚨 **Ela é a tela inteira**, e a grade dentro dela é que recebe o
            // `flex_1`. Foi o contrário disto que deixou a sessão parecendo
            // vazia: um cabeçalho com `size_full` comendo a coluna toda.
            .size_full()
            .p(px(12.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.cabecalho(cx))
            .child(self.envio(cx))
            .child(self.barra_da_grade(cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(div().text_xs().text_color(cx.theme().danger).child(erro))
            })
            .child(self.corpo(cx))
            .child(self.tira(cx))
    }
}

impl Detalhe {
    /// O cabeçalho da sessão: quem é o cliente, o que ela tem, e as duas saídas.
    ///
    /// 🔑 **As contagens aqui são por estado, cruas** — `2 levadas · 2 à venda ·
    /// 0 compradas`. As fichas da barra contam outra coisa: recorte por situação
    /// **exige classificação**, e por isso uma galeria com 4 levadas sem nota
    /// mostra `Levadas 0` lá e `4 levadas` aqui. Os dois números estão certos, e
    /// respondem perguntas diferentes.
    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (levadas, a_venda, compradas) = self.contagem();
        let titulo = self
            .aberta
            .as_ref()
            .map(|a| a.galeria.titulo.clone())
            .unwrap_or_else(|| "…".into());
        let contato = self
            .aberta
            .as_ref()
            .map(|a| {
                let mut partes = Vec::new();
                if let Some(email) = &a.galeria.email {
                    partes.push(email.clone());
                }
                if let Some(zap) = &a.galeria.whatsapp {
                    partes.push(zap.clone());
                }
                partes.join(" · ")
            })
            .unwrap_or_default();
        let ja_abriu = self
            .aberta
            .as_ref()
            .is_some_and(|a| a.galeria.user_id.is_some());
        let email = self.aberta.as_ref().and_then(|a| a.galeria.email.clone());

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(8.))
            .rounded(cx.theme().radius)
            // 🔑 **O cabeçalho é uma superfície, e não uma linha com traço
            // embaixo.** Ele é o único lugar da tela que diz de quem é o ensaio
            // aberto; encostado no mesmo cinza da grade, ele se lia como a
            // primeira fileira de fotos.
            .bg(cx.theme().sidebar)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                // A marca do ensaio, na família âmbar — a mesma que a barra do
                // topo usa para o botão da sessão aberta.
                div()
                    .w(px(3.))
                    .h(px(16.))
                    .flex_none()
                    .rounded(px(2.))
                    .bg(crate::tema::cores::quente()),
            )
            .child(div().text_sm().truncate().child(titulo))
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(contato),
            )
            .when(ja_abriu, |cabecalho| {
                // 🔑 O sinal que o fotógrafo espera para cobrar — e por isso é
                // selo, e não mais uma linha de texto pequeno.
                cabecalho.child(selos::selo(selos::Tom::Bom, "já abriu", cx))
            })
            .child(div().flex_1())
            .child(
                // As três contagens, cada uma com o tom do que ela conta: âmbar
                // é o balcão, verde é vendido, o resto é o comum.
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(selos::selo(
                        selos::Tom::Quente,
                        format!("{levadas} levadas"),
                        cx,
                    ))
                    .child(selos::selo(
                        selos::Tom::Neutro,
                        format!("{a_venda} à venda"),
                        cx,
                    ))
                    .child(selos::selo(
                        selos::Tom::Bom,
                        format!("{compradas} compradas"),
                        cx,
                    )),
            )
            .child(
                Button::new("sessao-link")
                    .label(if self.link.is_some() {
                        "Copiar de novo"
                    } else {
                        "Copiar link"
                    })
                    .xsmall()
                    .disabled(self.aberta.is_none())
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.pedir_o_link(cx))),
            )
            // 📧 O aviso só existe com e-mail: a conta do cliente nasce dele, e
            // sem ele não há para onde mandar "fotos prontas".
            .when_some(email, |cabecalho, email| {
                cabecalho.child(
                    Button::new("sessao-avisar")
                        .label(SharedString::from(format!("Avisar {email}: fotos prontas")))
                        .xsmall()
                        .primary()
                        .disabled(self.avisando)
                        .on_click(cx.listener(|tela, _ev, _window, cx| tela.avisar(cx))),
                )
            })
    }

    /// A área de envio: **arrastar a pasta**, ou a janela do sistema.
    ///
    /// 🚨 **Não há explorador de arquivos nosso aqui.** O app tem um, no modal de
    /// importação, e ele existe para a triagem em RAW — escolher entre duzentas
    /// do cartão. Para mandar fotos ao cliente ele é atrito: quem exportou do
    /// Lightroom já está com a pasta aberta ao lado. É o gesto da web, e o
    /// pedido do dono: *"tem que usar o mesmo explorador de arquivos do sistema
    /// operacional"*.
    ///
    /// 🔑 **A leva é escolhida antes dos arquivos**, e o padrão é **sem
    /// marcação** — ver o campo [`Self::leva`].
    fn envio(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ocupado = self.enviando > 0;
        let sem_sessao = self.aberta.is_none();
        let leva = self.leva;

        let ficha = |rotulo: &'static str,
                     valor: Option<EstadoNoBalcao>,
                     cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("detalhe-leva-{rotulo}")))
                .label(rotulo)
                .xsmall()
                .selected(leva == valor)
                .disabled(ocupado)
                .on_click(cx.listener(move |tela, _ev, _window, cx| tela.escolher_leva(valor, cx)))
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(10.))
            .rounded(cx.theme().radius)
            .border_1()
            // O destaque de "solte aqui": borda viva enquanto o arrasto passa.
            .border_color(if self.arrastando {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .on_drag_move(
                cx.listener(|tela, _ev: &gpui::DragMoveEvent<()>, _window, cx| {
                    tela.destacar(true, cx)
                }),
            )
            .on_drop(
                cx.listener(|tela, arrastados: &gpui::ExternalPaths, _window, cx| {
                    tela.destacar(false, cx);
                    // 🔑 Uma pasta solta vira o conteúdo dela: o sistema entrega
                    // o caminho do diretório, e uma pasta de exportação tem
                    // arquivos, não subpastas.
                    let fotos = super::arquivos::so_as_fotos(arrastados.paths());
                    tela.enviar_arquivos(fotos, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Como esta leva entra:"),
                    )
                    .child(ficha("Sem marcação", None, cx))
                    .child(ficha("À venda", Some(EstadoNoBalcao::Disponivel), cx))
                    .child(ficha(
                        "Levadas no balcão",
                        Some(EstadoNoBalcao::LevadaNoBalcao),
                        cx,
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match leva {
                        None => {
                            "Sem marcação vai para o acervo à venda. A marcação de verdade \
                             nasce no balcão, com o cliente olhando."
                        }
                        Some(EstadoNoBalcao::Disponivel) => {
                            "À venda: o cliente vê com marca d'água e pode comprar pela galeria."
                        }
                        Some(EstadoNoBalcao::LevadaNoBalcao) => {
                            "Levadas: o cliente já pagou na hora e baixa o original."
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match (ocupado, self.enviadas) {
                                (true, feitas) => format!("subindo… {feitas} prontas"),
                                (false, feitas) if feitas > 0 => format!("{feitas} subiram"),
                                (false, _) => "Arraste as fotos (ou a pasta) para cá.".to_string(),
                            }),
                    )
                    .child(
                        Button::new("detalhe-escolher")
                            .label("Escolher fotos…")
                            .xsmall()
                            .primary()
                            .disabled(ocupado || sem_sessao)
                            .on_click(
                                cx.listener(|tela, _ev, _window, cx| tela.escolher_fotos(cx)),
                            ),
                    ),
            )
    }

    /// Uma linha dizendo o que a grade abaixo está mostrando.
    /// A barra da grade: recortes · zoom · Revelar · Tela do cliente · seleção.
    fn barra_da_grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let contagens = self.contagens();
        let ativo = self.filtro();
        let visiveis = self.acervo.total_visivel();
        let marcadas = self.quantas_marcadas();
        let todas_marcadas = visiveis > 0 && marcadas == visiveis;

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .children(FILTROS.into_iter().filter_map(|(rotulo, filtro)| {
                let quantas = contagens.de(filtro);
                // 🔑 O recorte vazio some — **menos** quando é o escolhido:
                // sumir o filtro ativo tiraria o caminho de volta.
                let mostrar = filtro == Filtro::Todas || quantas > 0 || ativo == filtro;
                mostrar.then(|| {
                    Button::new(SharedString::from(format!("sessao-filtro-{rotulo}")))
                        .label(format!("{rotulo} {quantas}"))
                        .xsmall()
                        .selected(ativo == filtro)
                        .on_click(
                            cx.listener(move |tela, _ev, _window, cx| tela.filtrar(filtro, cx)),
                        )
                })
            }))
            .child(div().flex_1())
            .child(
                Button::new("sessao-zoom-menos")
                    .label("−")
                    .xsmall()
                    .on_click(
                        cx.listener(|tela, _ev, _window, cx| tela.ajustar_zoom(-PASSO_DO_ZOOM, cx)),
                    ),
            )
            .child(
                Button::new("sessao-zoom-mais")
                    .label("+")
                    .xsmall()
                    .on_click(
                        cx.listener(|tela, _ev, _window, cx| tela.ajustar_zoom(PASSO_DO_ZOOM, cx)),
                    ),
            )
            .child(
                Button::new("sessao-revelar")
                    .label("Revelar")
                    .xsmall()
                    .disabled(self.em_foco().is_none())
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.revelar_a_do_foco(cx))),
            )
            .child(
                Button::new("sessao-tela-do-cliente")
                    .label("Tela do cliente")
                    .xsmall()
                    .on_click(
                        cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::TelaDoCliente)),
                    ),
            )
            .child(
                Button::new("sessao-selecionar-visiveis")
                    .label(format!(
                        "{} as {visiveis} visíveis",
                        if todas_marcadas {
                            "Desmarcar"
                        } else {
                            "Selecionar"
                        }
                    ))
                    .xsmall()
                    .disabled(visiveis == 0)
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.alternar_todas(cx))),
            )
    }

    /// Pede à raiz que revele a foto em foco.
    pub fn revelar_a_do_foco(&mut self, cx: &mut Context<Self>) {
        if let Some(foto) = self.em_foco() {
            cx.emit(Pedido::Revelar {
                foto_id: foto.id.clone(),
                arquivo: foto.arquivo.clone(),
            });
        }
    }

    /// A grade e o painel da foto, lado a lado — como na tela do site.
    fn corpo(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .gap(px(8.))
            .child(self.grade(cx))
            .children(self.painel(cx))
    }

    fn grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.acervo.total_visivel() == 0 {
            let frase = if self.aberta.is_none() && self.carregando {
                "Lendo a sessão…"
            } else if self.acervo.todas().is_empty() {
                "Nenhuma foto nesta sessão ainda — arraste a primeira leva acima."
            } else {
                "Nenhuma foto neste recorte."
            };
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(frase)
                .into_any_element();
        }

        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .overflow_hidden()
            .children(
                self.acervo
                    .visiveis()
                    .enumerate()
                    .map(|(posicao, foto)| self.celula(posicao, foto, cx))
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    fn celula(
        &self,
        posicao: usize,
        foto: &acervo::Foto,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let marcada = self.selecao.tem(posicao);
        let em_foco = self.selecao.foco() == Some(posicao);
        let lado = self.zoom;
        let chave = format!("site:{}", foto.id);
        let miniatura = self
            .previews
            .get_preview(&chave)
            .or_else(|| self.previews.get_thumbnail(&chave))
            .map(crate::imagem::para_gpui);

        div()
            .id(SharedString::from(format!("sessao-tile-{}", foto.id)))
            .w(px(lado))
            .flex()
            .flex_col()
            .gap(px(2.))
            .cursor_pointer()
            .on_click(
                cx.listener(move |tela, evento: &gpui::ClickEvent, _window, cx| {
                    if evento.click_count() >= 2 {
                        tela.selecao
                            .clicar(posicao, false, Modificadores::default());
                        tela.revelar_a_do_foco(cx);
                        return;
                    }
                    let m = evento.modifiers();
                    tela.clicar(
                        posicao,
                        Modificadores {
                            aditivo: m.secondary(),
                            faixa: m.shift,
                        },
                        cx,
                    );
                }),
            )
            .child(
                div()
                    .relative()
                    .h(px(lado * 0.72))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().muted)
                    .rounded(cx.theme().radius)
                    .border_1()
                    // 🔑 A marcação é **borda**, e não fundo: fundo colorido
                    // mudaria a cor que o olho usa para julgar a foto ao lado.
                    .border_color(if em_foco {
                        cx.theme().primary
                    } else if marcada {
                        cx.theme().warning
                    } else {
                        cx.theme().border
                    })
                    .when_some(miniatura, |quadro, imagem| {
                        quadro.child(img(imagem).h(px(lado * 0.72)))
                    })
                    // O selo do estado, no canto — como na tela do site, e
                    // agora com a cor do que ele diz (`crate::selos`).
                    .child(
                        div()
                            .absolute()
                            .top(px(4.))
                            .left(px(4.))
                            .child(selos::selo_do_estado(foto.estado, foto.apagada, cx)),
                    )
                    .when(marcada, |quadro| {
                        quadro.child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .right(px(4.))
                                .size(px(14.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(cx.theme().primary)
                                .text_color(cx.theme().primary_foreground)
                                .text_xs()
                                .child("✓"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .child(SharedString::from(format!(
                                "{}. {}",
                                posicao + 1,
                                foto.arquivo
                            ))),
                    )
                    // 🚨 **A nota aparece na grade do ensaio**, e é ela que
                    // autoriza a foto a estar aqui (regra do dono, 5/set/2026:
                    // só sobe o que foi classificado). Sem nota, a fileira fica
                    // apagada — é como se encontra o que subiu sem passar pela
                    // triagem.
                    .child(selos::estrelas(foto.nota.unwrap_or(0) as i32, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(format!(
                        "{} · {} download(s)",
                        self.nome_da_faixa(&foto.produto_efetivo),
                        foto.downloads
                    ))),
            )
    }

    /// O nome da faixa, como o operador a conhece.
    fn nome_da_faixa(&self, id: &str) -> String {
        self.produtos
            .iter()
            .find(|p| p.id == id)
            .map(|p| {
                let centavos = dinheiro::ler_campo(&p.preco).unwrap_or(0);
                format!("{} — {}", p.nome, dinheiro::formatar(centavos))
            })
            .unwrap_or_else(|| "Padrão da galeria".to_string())
    }

    /// O painel da direita: o que se sabe e o que se muda **nesta** foto.
    fn painel(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let foto = self.em_foco()?;
        let posicao = self.selecao.foco()? + 1;
        let negociada = foto.tem_negociacao();

        Some(
            div()
                .w(px(300.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .truncate()
                        .child(SharedString::from(format!("{posicao}. {}", foto.arquivo))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(foto.estado.rotulo()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(match foto.nota {
                            Some(n) => format!("Nota {}", "★".repeat(n as usize)),
                            None => "Sem nota".to_string(),
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            Button::new("painel-estado")
                                .label(if foto.estado == acervo::Estado::LevadaNoBalcao {
                                    "Pôr à venda"
                                } else {
                                    "Levada no balcão"
                                })
                                .xsmall()
                                .disabled(!foto.editavel())
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.alternar_levada(cx)),
                                ),
                        )
                        .child(
                            Button::new("painel-revelar")
                                .label("Revelar")
                                .xsmall()
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| {
                                        tela.revelar_a_do_foco(cx)
                                    }),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!(
                            "Faixa: {}",
                            self.nome_da_faixa(&foto.produto_efetivo)
                        ))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!(
                            "Downloads {} · {}",
                            foto.downloads,
                            match foto.preco_de_venda {
                                Some(centavos) =>
                                    format!("preço fixado {}", dinheiro::formatar(centavos)),
                                None => "sem valor fixado: vale o preço da faixa".to_string(),
                            }
                        ))),
                )
                .when(negociada, |painel| {
                    painel.child(div().text_xs().text_color(cx.theme().warning).child(
                        match foto.preco_negociado {
                            Some(centavos) => format!("Balcão: {}", dinheiro::formatar(centavos)),
                            None => "Balcão: registrado".to_string(),
                        },
                    ))
                })
                // 📌 A negociação, o preço de venda e o apagar ficam para o
                // próximo passo — eles pedem campos e confirmação, e entram
                // inteiros ou não entram.
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Negociação e preço: pelo Balcão, na barra de cima."),
                ),
        )
    }

    /// A tira e a legenda das teclas — o rodapé da tela do site.
    fn tira(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.acervo.total_visivel();
        let atual = self.selecao.foco().map(|i| i + 1).unwrap_or(0);

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pt(px(6.))
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(format!(
                        "{atual} / {total} · ←→ andam · 1–5 nota · P levada no balcão · \
                         Ctrl ou Shift no clique marcam várias · Ctrl+A marca tudo, \
                         Ctrl+D desmarca"
                    ))),
            )
            .child(
                div().flex().gap(px(4.)).overflow_hidden().children(
                    self.acervo
                        .visiveis()
                        .enumerate()
                        .map(|(posicao, foto)| {
                            let chave = format!("site:{}", foto.id);
                            let miniatura = self
                                .previews
                                .get_thumbnail(&chave)
                                .or_else(|| self.previews.get_preview(&chave))
                                .map(crate::imagem::para_gpui);
                            div()
                                .id(SharedString::from(format!("tira-{}", foto.id)))
                                .w(px(56.))
                                .h(px(56.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(cx.theme().radius)
                                .cursor_pointer()
                                .border_1()
                                .border_color(if self.selecao.foco() == Some(posicao) {
                                    cx.theme().primary
                                } else {
                                    cx.theme().border
                                })
                                .bg(cx.theme().muted)
                                .when_some(miniatura, |celula, imagem| {
                                    celula.child(img(imagem).h(px(54.)))
                                })
                                .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                    tela.clicar(posicao, Modificadores::default(), cx)
                                }))
                        })
                        .collect::<Vec<_>>(),
                ),
            )
    }
}

/// A foto do site na linguagem do core.
///
/// 🔑 **A conversão mora num lugar só.** Ela é onde o decimal em texto do site
/// vira centavos e o estado vira o enum do core — e espalhá-la faria os dois
/// darem respostas diferentes para a mesma foto.
fn para_o_core(foto: &FotoDaGaleria) -> acervo::Foto {
    acervo::Foto {
        id: foto.id.clone(),
        arquivo: foto.arquivo.clone(),
        estado: match foto.estado {
            EstadoDaFotoNoSite::LevadaNoBalcao => acervo::Estado::LevadaNoBalcao,
            EstadoDaFotoNoSite::Disponivel => acervo::Estado::Disponivel,
            EstadoDaFotoNoSite::Comprada => acervo::Estado::Comprada,
        },
        apagada: foto.apagada,
        produto_efetivo: foto.produto_efetivo.clone(),
        preco_negociado: foto
            .preco_negociado
            .as_deref()
            .and_then(dinheiro::ler_campo),
        tem_observacao: foto
            .observacao_da_negociacao
            .as_deref()
            .is_some_and(|o| !o.trim().is_empty()),
        preco_de_venda: foto.preco_de_venda.as_deref().and_then(dinheiro::ler_campo),
        pedido_id: foto.pedido_id.clone(),
        downloads: foto.downloads,
        revelada: foto.revelada,
        nota: foto.nota,
        ordem: foto.ordem as i64,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use crate::sessoes::arquivos::mentira::SeletorDeMentira;
    use gpui::TestAppContext;

    fn foto(id: &str, estado: EstadoDaFotoNoSite, nota: Option<u8>) -> FotoDaGaleria {
        FotoDaGaleria {
            id: id.into(),
            arquivo: format!("{id}.jpg"),
            estado,
            ordem: 0,
            preco_negociado: None,
            observacao_da_negociacao: None,
            apagada: false,
            nota,
            produto_efetivo: "p1".into(),
            preco_de_venda: None,
            pedido_id: None,
            downloads: 0,
            revelada: false,
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        fotos: Vec<FotoDaGaleria>,
    ) -> (gpui::WindowHandle<Detalhe>, Arc<PublicadorDeMentira>) {
        cx.update(gpui_component::init);
        let publicador = Arc::new(PublicadorDeMentira {
            galerias: std::sync::Mutex::new(vec![domain::services::pos_venda::GaleriaDoPainel {
                id: "g1".into(),
                titulo: "Ensaio".into(),
                email: Some("ana@x.com".into()),
                whatsapp: None,
                produto_id: "p1".into(),
                user_id: None,
                criada_em_iso: "2026-09-06".into(),
                expira_em: None,
                fotos: Default::default(),
                totais: None,
            }]),
            fotos_da_sessao: std::sync::Mutex::new(fotos),
            ..Default::default()
        });
        let janela = cx.add_window({
            let publicador = publicador.clone();
            move |_window, _cx| {
                let dir = tempfile::TempDir::new().expect("diretório temporário");
                let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
                std::mem::forget(dir);
                let mut tela =
                    Detalhe::nova(publicador, Arc::new(SeletorDeMentira::default()), previews);
                tela.definir_sessao(Sessao {
                    access_token: "tok".into(),
                });
                tela
            }
        });
        (janela, publicador)
    }

    fn entrar(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Detalhe>) {
        janela
            .update(cx, |tela, _window, cx| tela.entrar("g1".into(), cx))
            .expect("a janela deve estar aberta");
        for _ in 0..10 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
    }

    /// 🚨 **Sem classificação não é "à venda" nem "levada".**
    ///
    /// É a regra do dono de 2026-09-05, e ela mora no core: contar a sem nota no
    /// recorte de venda dizia o contrário na primeira linha da tela — *"à venda
    /// 8"* numa galeria em que nenhuma das oito tinha nota. Elas moram no
    /// recorte "Sem nota", que existe para esvaziar.
    #[gpui::test]
    fn o_recorte_por_situacao_exige_classificacao(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Disponivel, None),
                foto("c", EstadoDaFotoNoSite::LevadaNoBalcao, Some(5)),
            ],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                let contagens = tela.contagens();
                assert_eq!(contagens.todas, 3);
                assert_eq!(
                    contagens.de(Filtro::Situacao(acervo::Estado::Disponivel)),
                    1,
                    "a sem nota não entra em 'à venda'"
                );
                assert_eq!(contagens.de(Filtro::SemNota), 1);

                tela.filtrar(Filtro::SemNota, cx);
                assert_eq!(tela.acervo.total_visivel(), 1);
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Trocar o recorte limpa a seleção — o que se vê é o que se opera.
    ///
    /// A seleção fala em **posição**, e o recorte muda quem está em cada uma:
    /// mantê-la faria a próxima ação em lote cair em fotos que ninguém marcou.
    #[gpui::test]
    fn trocar_o_recorte_limpa_a_selecao(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::LevadaNoBalcao, Some(4)),
            ],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.alternar_todas(cx);
                assert_eq!(tela.quantas_marcadas(), 2);

                tela.filtrar(Filtro::Situacao(acervo::Estado::Disponivel), cx);
                assert_eq!(tela.quantas_marcadas(), 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O lote não toca na comprada nem na apagada.
    ///
    /// Quem decide isso é o core (`Foto::editavel`): a comprada tem cobrança
    /// atrás dela — mudar o estado mudaria o que já foi pago — e a apagada não
    /// tem arquivo. Mandar assim mesmo traria um erro por foto, com o cliente na
    /// frente.
    #[gpui::test]
    fn o_lote_nao_toca_na_comprada(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Comprada, Some(4)),
            ],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.alternar_todas(cx);
                assert_eq!(tela.quantas_marcadas(), 2, "as duas estão marcadas");
                tela.marcar_como(EstadoNoBalcao::LevadaNoBalcao, cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let negociadas = publicador.negociadas();
        assert_eq!(negociadas.len(), 1, "só a que ainda pode mudar");
        assert_eq!(negociadas[0].0, "a");
    }
}
