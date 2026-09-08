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

use std::num::NonZeroUsize;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::acervo::{self, Acervo, Filtro};
use biblioteca_core::dinheiro;
use biblioteca_core::grade::colunas_que_cabem;
use biblioteca_core::selecao::{Modificadores, Selecao};
use domain::services::pos_venda::{
    EstadoDaFotoNoSite, EstadoNoBalcao, FotoDaGaleria, GaleriaAberta, LinkDeAcesso, Produto, Sessao,
};
use gpui::{
    canvas, div, img, prelude::*, px, App, Context, Entity, EventEmitter, SharedString, Task,
    Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::progress::Progress;
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use super::altura_da_tira;
use super::arquivos::SeletorDeFotos;
use crate::biblioteca::miniaturas::{CacheDeMiniaturas, Miniatura};
use crate::pos_venda::porta::{Publicador, Recado};
use crate::selos;
use crate::tema::cores;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// O tamanho do tile, em pixels — os mesmos limites da barra do site.
const ZOOM_MINIMO: f32 = 90.0;
const ZOOM_MAXIMO: f32 = 320.0;
const ZOOM_PADRAO: f32 = 160.0;
const PASSO_DO_ZOOM: f32 = 35.0;

/// O lado da miniatura da sessão, em pixels.
///
/// 🔑 **É o `ZOOM_MAXIMO`**, e não um número à parte: no zoom cheio a célula tem
/// esse tamanho, e uma miniatura menor seria ampliada — que é o defeito que o
/// `reduzir` do `thumbnail_generator` existe para não cometer. Maior que isso
/// seria pagar pixel que nenhum zoom mostra.
const LADO_DA_MINIATURA: u32 = ZOOM_MAXIMO as u32;

/// Quantas miniaturas da sessão ficam na memória.
///
/// A grade da sessão **não é virtualizada**: ela desenha todas as visíveis do
/// recorte. Então o cache precisa caber o recorte inteiro, ou cada quadro
/// descarta o que o próximo pede de volta — o mesmo defeito que o
/// `PreviewManager` (15 imagens) já cometia com as 25 desta sessão.
/// `preparar_miniaturas` cresce isto para o que estiver à vista; este é só o
/// piso de partida.
const MINIATURAS_GUARDADAS: usize = 64;

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
    /// Entrar na Revelação com **a sessão inteira na tira**, começando por
    /// `inicial`.
    ///
    /// 🔑 **Na web são dois botões, e cada um faz uma coisa**
    /// (`abrir-revelacao.tsx`): o da **barra da grade** entra no modo sem
    /// escolher foto — *"revelar é trabalho de lote, e exigir escolher uma foto
    /// antes era um passo a mais para começar"* —, e o do **painel** abre a
    /// foto em foco. Nos dois casos a galeria inteira vai junto: a tira, as
    /// setas e os botões do editor percorrem a mesma lista, e "a próxima" é a
    /// próxima da sessão. Até 7/set/2026 os dois botões faziam o mesmo aqui,
    /// mandavam **uma** foto — a tira era de uma — e o da barra ficava
    /// desligado sem foco, que é justamente o caso em que a web o usa.
    Revelar {
        fotos: Vec<FotoARevelar>,
        inicial: usize,
    },
    /// Abrir a exportação com o que está na grade — o botão que desceu da barra
    /// do app em 8/set/2026, para o lado do "Importar".
    ///
    /// 🔑 **A tela não escolhe as fotos**, como não escolhe nada que atravesse
    /// para fora dela: quem sabe o que a grade tem marcado, e o que sobra
    /// quando nada está, é a raiz.
    Exportar,
}

/// Uma foto do site, no que a Revelação precisa para abri-la: o id remoto — de
/// onde vem a cópia de trabalho — e o nome do arquivo, que a tira escreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoARevelar {
    pub id: String,
    pub arquivo: String,
}

/// O andamento de uma importação: quantas foram pedidas e quantas responderam.
///
/// 🔑 **A falha conta como pronta.** A barra mede o que falta *esperar*, não o
/// que deu certo: uma foto que o site recusou não vai responder de novo, e
/// deixá-la fora da conta prenderia a barra em 499/500 para sempre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Importacao {
    /// Quantos arquivos entraram no lote.
    pub total: usize,
    /// Quantos subiram.
    pub feitas: usize,
    /// Quantos o site recusou.
    pub falhas: usize,
}

impl Importacao {
    /// Quantas já responderam — as que subiram e as que falharam.
    pub fn prontas(&self) -> usize {
        self.feitas + self.falhas
    }

    pub fn terminou(&self) -> bool {
        self.prontas() >= self.total
    }

    /// O quanto a barra preenche, de 0 a 100.
    ///
    /// ⚠️ **Lote vazio devolve 100**, e não divide por zero. Ele não chega a
    /// existir (`enviar_arquivos` recusa lista vazia), mas um tipo que responde
    /// `NaN` num caso impossível vira `width: NaN%` no dia em que o caso deixa
    /// de ser impossível — e aí a barra some sem ninguém entender por quê.
    pub fn porcento(&self) -> f32 {
        if self.total == 0 {
            return 100.;
        }
        (self.prontas() as f32 / self.total as f32) * 100.
    }
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
    /// As miniaturas **já convertidas para textura**, uma vez por foto.
    ///
    /// 🚨 **Sem isto a grade decodificava tudo dentro do `render`.** `celula` e
    /// `tira` chamavam `get_preview` + `para_gpui` por foto, por quadro — e o
    /// `PreviewManager` guarda 15 imagens contra as 25 desta sessão, então o LRU
    /// dele acertava **zero**: todo quadro relia o BLOB, redecodificava o JPEG,
    /// clonava a imagem inteira e trocava RGBA→BGRA byte a byte. Medido em
    /// 6/set/2026 com `medir-grade-da-sessao`: **51 ms por quadro em release**,
    /// 3× o orçamento de 60fps, e a tira de baixo pagava o mesmo de novo.
    ///
    /// É o mesmo cache da grade da Biblioteca, que nasceu deste problema
    /// (`biblioteca::miniaturas`) e não tinha sido ligado aqui.
    miniaturas: CacheDeMiniaturas,
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
    /// O controle do zoom da grade.
    ///
    /// 🔑 **Um slider, e não `−` e `+`** — é o que a rota do site tem
    /// (`grade.tsx`), e a diferença não é enfeite: com dois botões de passo
    /// fixo, ir de 90 a 320px são sete cliques, e o operador não vê onde está
    /// na faixa. O slider mostra e chega em um gesto.
    zoom_slider: Entity<SliderState>,
    /// Se o aviso ao cliente está a caminho.
    avisando: bool,
    /// Se o pedido do link está no ar — é o que mantém a colheita acordada até
    /// ele voltar, e o que impede dois pedidos pelo mesmo botão.
    pedindo_link: bool,
    /// Se a segunda tela — a do cliente — está aberta agora.
    ///
    /// 🔑 **O botão é um alternador, e um alternador tem de mostrar o estado.**
    /// No site ele acende em âmbar com `aria-pressed` (`grade.tsx`); aqui ele
    /// era um botão comum, e o mesmo clique fechava ou abria sem a tela dizer
    /// qual dos dois ia acontecer. Quem sabe a resposta é a raiz, que é dona da
    /// janela — daí vir de fora, por `definir_cliente_aberta`.
    cliente_aberta: bool,
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
    /// A rolagem da grade e a da tira, para levá-las até o foco.
    ///
    /// 🔑 **Sem isto o foco anda e a tela não segue.** As setas movem a seleção
    /// pela lista inteira, e a partir da terceira fileira (ou da décima
    /// miniatura) a foto em foco está fora de vista: o operador aperta ↓ e não
    /// vê nada acontecer. É o que a tira do editor da web faz.
    rolagem_da_grade: gpui::ScrollHandle,
    rolagem_da_tira: gpui::ScrollHandle,
    /// A altura da tira, que **é** o zoom das miniaturas dela.
    altura_da_tira: f32,
    /// O arrasto do puxador: onde o ponteiro desceu e qual era a altura ali.
    arrasto_da_tira: Option<(gpui::Pixels, f32)>,
    /// O foco do quadro anterior — só rola quando ele **muda**.
    ///
    /// ⚠️ Rolar a cada quadro prenderia a barra: o operador não conseguiria
    /// arrastar a tira para olhar o resto sem ela voltar sozinha.
    ultimo_foco: Option<usize>,
    /// Quantas **mudanças em lote** ainda esperam resposta — nota, levada,
    /// negociação. Elas correm em série de propósito: são a mesma seleção, e
    /// duas rodadas sobre as mesmas fotos disputariam a última palavra.
    ///
    /// 🚨 **Nada aqui conta importação, e é essa a separação inteira.** Ver
    /// [`Self::importacao`].
    mudando: usize,
    /// O andamento da importação — o que a barra de progresso desenha.
    ///
    /// 🚨 **Ela vive separada de [`Self::mudando`] porque o operador trabalha
    /// durante ela.** Até 8/set/2026 os dois eram um contador só (`enviando`), e
    /// o preço era exatamente o cenário do dono: com 500 fotos subindo,
    /// `mudar_as_marcadas` desistia em silêncio (`if … || self.enviando > 0 {
    /// return; }`) e **classificar, sinalizar "levada" e negociar paravam de
    /// responder** — sem erro, sem aviso, sem relação visível com a importação.
    /// Pior: quando a negociação passava, ela zerava o contador do lote e a
    /// importação se dava por terminada no meio.
    importacao: Option<Importacao>,
    link: Option<LinkDeAcesso>,
    carregando: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    /// O canal **só da importação**.
    ///
    /// 🔑 **`Recado::Sincronizou` não diz quem terminou** — é o mesmo "pronto"
    /// de subir arquivo, negociar, classificar e tirar do site. Enquanto os dois
    /// trabalhos dividiam o canal, não havia como contar um sem contar o outro:
    /// classificar no meio da importação adiantava a barra. Dois canais separam
    /// sem tocar no `enum`, que é de todo mundo.
    envios: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Detalhe {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        seletor: Arc<dyn SeletorDeFotos>,
        previews: Arc<PreviewManager>,
        cx: &mut Context<Self>,
    ) -> Self {
        let zoom_slider = cx.new(|_| {
            SliderState::new()
                .min(ZOOM_MINIMO)
                .max(ZOOM_MAXIMO)
                .step(PASSO_DO_ZOOM)
                .default_value(ZOOM_PADRAO)
        });
        // ⚠️ `subscribe`, e não `subscribe_in`: mudar o tamanho do tile não
        // precisa da janela, e exigi-la obrigaria a raiz a construir o Detalhe
        // dentro de um `cx.new` com `window` — que ela não tem ali.
        cx.subscribe(&zoom_slider, |tela, _estado, evento: &SliderEvent, cx| {
            let SliderEvent::Change(valor) = evento;
            let novo = valor.start().clamp(ZOOM_MINIMO, ZOOM_MAXIMO);
            if novo != tela.zoom {
                tela.zoom = novo;
                cx.notify();
            }
        })
        .detach();

        Self {
            zoom_slider,
            publicador,
            seletor,
            escolhas: channel(),
            leva: None,
            arrastando: false,
            acervo: Acervo::novo(),
            selecao: Selecao::nova(),
            rolagem_da_grade: gpui::ScrollHandle::new(),
            rolagem_da_tira: gpui::ScrollHandle::new(),
            altura_da_tira: altura_da_tira::guardada("sessao"),
            arrasto_da_tira: None,
            ultimo_foco: None,
            sessao: None,
            galeria_id: None,
            aberta: None,
            previews,
            miniaturas: CacheDeMiniaturas::nova(
                NonZeroUsize::new(MINIATURAS_GUARDADAS).expect("não é zero"),
            ),
            pedidas: std::collections::HashSet::new(),
            baixando: 0,
            produtos: Vec::new(),
            faixa: None,
            zoom: ZOOM_PADRAO,
            avisando: false,
            pedindo_link: false,
            cliente_aberta: false,
            mudando: 0,
            importacao: None,
            link: None,
            carregando: false,
            erro: None,
            recados: channel(),
            envios: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// Se a conta do site já desceu até aqui — esta tela não sabe pedi-la.
    pub fn tem_sessao(&self) -> bool {
        self.sessao.is_some()
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
        self.importacao = None;
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

    /// Um recado da raiz para esta tela — o que aconteceu com o site.
    ///
    /// 🔑 Cai no mesmo lugar do erro porque é o mesmo lugar de olhar: a linha
    /// do cabeçalho. Dois lugares para dizer "algo aconteceu" fariam o operador
    /// aprender a ignorar um deles.
    pub fn recado(&mut self, texto: String, cx: &mut Context<Self>) {
        self.erro = Some(texto.into());
        cx.notify();
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

    pub fn ajustar_zoom(&mut self, passo: f32, window: &mut Window, cx: &mut Context<Self>) {
        let novo = (self.zoom + passo).clamp(ZOOM_MINIMO, ZOOM_MAXIMO);
        if novo == self.zoom {
            return;
        }
        self.zoom = novo;
        // 🚨 O slider tem de acompanhar quem mexeu no zoom por outro caminho,
        // ou ele passa a mostrar um número que não é o da grade. `set_value`
        // não emite `Change`, então isto não volta como um segundo ajuste.
        self.zoom_slider
            .update(cx, |estado, cx| estado.set_value(novo, window, cx));
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

    /// Leva a grade e a tira até a foto em foco, quando ele muda.
    ///
    /// 🔑 **Só quando muda.** O `ScrollHandle` guarda o pedido e o atende na
    /// próxima pintura; repeti-lo a cada quadro deixaria a tira presa no foco e
    /// impossível de arrastar com a mão.
    fn seguir_o_foco(&mut self) {
        let foco = self.selecao.foco();
        if foco == self.ultimo_foco {
            return;
        }
        self.ultimo_foco = foco;
        if let Some(posicao) = foco {
            self.rolagem_da_grade.scroll_to_item(posicao);
            self.rolagem_da_tira.scroll_to_item(posicao);
        }
    }

    /// Quantas colunas a grade da sessão desenha agora.
    ///
    /// 🚨 **O painel da foto entra na conta.** Ele tem 300px fixos e divide a
    /// linha com a grade; ignorá-lo daria mais colunas do que cabem, e a seta ↓
    /// pularia por cima de uma foto. É o mesmo cuidado que `largura_util` da
    /// Biblioteca tem com a árvore de pastas.
    ///
    /// ⚠️ Quando não há foco não há painel, e a grade é mais larga — mas ↑↓ só
    /// valem com foco, então a conta com painel é a que importa.
    pub fn colunas_visiveis(&self, window: &Window) -> usize {
        /// O `p(px(12.))` da tela, dos dois lados.
        const MARGEM: f32 = 24.0;
        /// A largura do painel da foto mais o `gap` do `corpo`.
        const PAINEL: f32 = 300.0 + 8.0;
        let largura = f32::from(window.viewport_size().width) - MARGEM - PAINEL;
        colunas_que_cabem(largura, self.zoom, 8.0)
    }

    /// Uma **linha** para cima ou para baixo — as setas ↑ e ↓.
    ///
    /// 🔑 **O passo é o número de colunas**, e por isso ele vem de fora: quem
    /// sabe a largura da janela é a raiz, não a tela. A grade é `flex_wrap`, e
    /// a mesma foto muda de linha quando a janela muda de tamanho.
    ///
    /// ⚠️ **Não dá a volta e não escorrega para outra linha.** Descer da última
    /// linha fica na última — pular para a foto final porque ela é "o mais perto
    /// que dá" faria a seta ↓ mover a seleção horizontalmente, que é o gesto da
    /// outra tecla.
    pub fn andar_linha(&mut self, passo: i32, colunas: usize, cx: &mut Context<Self>) {
        let total = self.acervo.total_visivel();
        let colunas = colunas.max(1);
        if total == 0 {
            return;
        }
        let Some(atual) = self.selecao.foco() else {
            // Sem foco, a primeira seta escolhe uma ponta — o mesmo que `andar`.
            let nova = if passo > 0 { 0 } else { total - 1 };
            self.selecao.clicar(nova, false, Modificadores::default());
            cx.notify();
            return;
        };
        let destino = if passo > 0 {
            atual + colunas
        } else {
            match atual.checked_sub(colunas) {
                Some(i) => i,
                None => return,
            }
        };
        if destino >= total {
            return;
        }
        self.selecao
            .clicar(destino, false, Modificadores::default());
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

    /// A raiz avisa quando a segunda tela abre ou fecha.
    pub fn definir_cliente_aberta(&mut self, aberta: bool, cx: &mut Context<Self>) {
        if self.cliente_aberta != aberta {
            self.cliente_aberta = aberta;
            cx.notify();
        }
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
        // 🚨 **A importação não entra nesta guarda**, e é o conserto de
        // 8/set/2026: com 500 fotos subindo, classificar, sinalizar e negociar
        // desistiam aqui em silêncio. O que ainda faz esperar é outra rodada
        // *desta mesma* operação, sobre a mesma seleção.
        if alvos.is_empty() || self.mudando > 0 {
            return;
        }

        self.erro = None;
        self.mudando = alvos.len();
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
    pub fn importar(&mut self, cx: &mut Context<Self>) {
        // ⚠️ **Uma importação de cada vez.** Não é para poupar o servidor: é que
        // o lote é um só (`Importacao`), e um segundo lote por cima faria a
        // barra recomeçar do zero no meio do primeiro. O resto da tela continua
        // solto — importar é o único gesto que a importação segura.
        if self.importando() {
            return;
        }
        self.seletor.escolher(self.escolhas.0.clone());
        self.acompanhar(cx);
    }

    /// Se há uma importação em curso — a que segura o botão e desenha a barra.
    pub fn importando(&self) -> bool {
        self.importacao.is_some_and(|i| !i.terminou())
    }

    /// O andamento da última importação, terminada ou não.
    pub fn importacao(&self) -> Option<Importacao> {
        self.importacao
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
        if caminhos.is_empty() || self.importando() {
            return;
        }

        // A ordem no site continua de onde a sessão parou.
        let ja_na_sessao = self.aberta.as_ref().map(|a| a.fotos.len()).unwrap_or(0);
        // Sem marcação vai como "à venda" — o estado de quem ainda não foi levada.
        let estado = self.leva.unwrap_or(EstadoNoBalcao::Disponivel);

        self.erro = None;
        self.importacao = Some(Importacao {
            total: caminhos.len(),
            feitas: 0,
            falhas: 0,
        });
        for (i, caminho) in caminhos.into_iter().enumerate() {
            self.publicador.enviar_arquivo(
                sessao.clone(),
                galeria_id.clone(),
                caminho,
                (ja_na_sessao + i) as u32,
                estado,
                // 🔑 **Pelo canal da importação**, e não pelo de todo mundo.
                self.envios.0.clone(),
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
        if self.pedindo_link {
            return;
        }
        // 🚨 **Sem isto o link nunca chegava.** A colheita para quando não há
        // resposta a esperar, e "esperar" era `carregando || enviando ||
        // baixando || avisando` — uma lista que esquecia o link. O pedido saía,
        // a resposta voltava 300 ms depois e ninguém mais drenava o canal: o
        // botão do passo 7 não copiava nada, e não dizia nada.
        self.pedindo_link = true;
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

        // ── As respostas da importação ───────────────────────────────────
        //
        // 🚨 **Canal próprio, contador próprio.** O que chega aqui é resposta de
        // `enviar_arquivo`, e só. Enquanto isto dividia o canal com o resto, uma
        // classificação feita durante o lote adiantava a barra em uma foto — e
        // 500 classificações a levavam ao fim com metade das fotos ainda no
        // disco.
        while let Ok(recado) = self.envios.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Sincronizou => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.feitas += 1;
                    }
                }
                // ⚠️ **A falha de uma foto não derruba o lote**, e nem para a
                // barra: o operador mandou 500, e "uma não subiu" é um recado —
                // as outras 499 continuam.
                Recado::Falhou(erro) => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.falhas += 1;
                    }
                    self.erro = Some(erro.into());
                }
                _ => {}
            }
            if self.importacao.is_some_and(|l| l.terminou()) {
                // O lote acabou: reler a sessão é o que faz as novas aparecerem
                // na grade.
                if let (Some(sessao), Some(id)) = (self.sessao.clone(), self.galeria_id.clone()) {
                    self.publicador
                        .abrir_galeria(sessao, id, self.recados.0.clone());
                    self.carregando = true;
                }
            }
        }

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Aberta(mut aberta) => {
                    self.carregando = false;
                    // 🚨 **Reler a galeria não pode desmanchar o que a mão fez.**
                    //
                    // Dar nota ou marcar "levada" manda a mudança e **relê a
                    // galeria inteira**. Antes, a releitura fazia duas coisas
                    // que o operador não pediu: aceitava a ordem que o servidor
                    // devolveu (a foto classificada pulava de lugar) e chamava
                    // `limpar_tudo` (a seleção sumia). Numa triagem, isso é dar
                    // nota a uma foto e perder de vista as outras vinte que
                    // estavam marcadas para receber a mesma.
                    //
                    // 🔑 **A seleção é guardada por id, não por posição** — que é
                    // o que a linha antiga estava certa em temer. O que estava
                    // errado era a conclusão: em vez de descartar, traduz.
                    let mesma_galeria = self
                        .aberta
                        .as_ref()
                        .is_some_and(|atual| atual.galeria.id == aberta.galeria.id);

                    let marcadas: Vec<String> = if mesma_galeria {
                        self.selecao
                            .marcadas()
                            .filter_map(|p| self.acervo.visivel(p))
                            .map(|f| f.id.clone())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    let focada = if mesma_galeria {
                        self.selecao
                            .foco()
                            .and_then(|p| self.acervo.visivel(p))
                            .map(|f| f.id.clone())
                    } else {
                        None
                    };

                    if mesma_galeria {
                        if let Some(atual) = self.aberta.as_ref() {
                            ordenar_como_antes(&mut aberta.fotos, &atual.fotos);
                        }
                    }

                    // 🔑 A conta é do core: recorte, contagens e ordem saem
                    // dele, e não de laços escritos aqui.
                    self.acervo
                        .definir(aberta.fotos.iter().map(para_o_core).collect());
                    self.selecao.limpar_tudo();
                    // ⚠️ Quem saiu do recorte não volta: classificar com a ficha
                    // "Sem nota" aberta tira a foto da lista, e é o que a ficha
                    // promete. O `posicao_de` responde `None` e ela fica de fora
                    // — as outras marcadas continuam.
                    for id in &marcadas {
                        if let Some(p) = self.acervo.posicao_de(id) {
                            self.selecao.marcar(p);
                        }
                    }
                    if let Some(p) = focada.as_deref().and_then(|id| self.acervo.posicao_de(id)) {
                        self.selecao.focar(Some(p));
                        // A releitura reposiciona; o foco tem de reaparecer na
                        // tela, e não só no estado.
                        self.ultimo_foco = None;
                    }

                    cx.emit(Pedido::FotosDoSite(aberta.fotos.clone()));
                    self.aberta = Some(*aberta);
                    abriu = true;
                }
                Recado::Miniatura { foto_id, bytes } => {
                    self.baixando = self.baixando.saturating_sub(1);
                    // Miniatura ilegível não derruba a grade: a célula fica sem
                    // imagem, com o nome do arquivo, que é melhor que nada.
                    if let Ok(imagem) = image::load_from_memory(&bytes) {
                        let chave = chave_da_miniatura(&foto_id);
                        // 🔑 **As duas.** O preview grande é o que a tela do
                        // cliente e o painel usam; a miniatura é o que a grade e
                        // a tira desenham. Gravar só o grande — que era o que
                        // acontecia — fazia a célula de 160px carregar 640px.
                        let _ = self
                            .previews
                            .save_thumbnail(&chave, &reduzir(&imagem, LADO_DA_MINIATURA));
                        if self.previews.save_preview(&chave, &imagem).is_ok() {
                            // O cache guarda a ausência: sem esquecê-la, a foto
                            // recém-chegada ficaria vazia até sair e voltar.
                            self.miniaturas.esquecer(&chave);
                            // A grade guarda "ausente" para quem ainda não tinha
                            // miniatura; sem avisar, a foto recém-baixada só
                            // apareceria quando a célula saísse e voltasse.
                            cx.emit(Pedido::MiniaturaPronta(chave));
                        }
                    }
                }
                Recado::Sincronizou => {
                    self.avisando = false;
                    self.mudando = self.mudando.saturating_sub(1);
                    if self.mudando == 0 {
                        // A rodada de mudanças acabou: reler a sessão é o que
                        // traz de volta o que o site gravou nelas.
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
                    self.pedindo_link = false;
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.url.clone()));
                    self.link = Some(link);
                }
                Recado::Falhou(erro) => {
                    self.carregando = false;
                    self.pedindo_link = false;
                    self.mudando = self.mudando.saturating_sub(1);
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
        let continua = self.carregando
            || self.mudando > 0
            || self.importando()
            || self.baixando > 0
            || self.avisando
            || self.pedindo_link;
        if !continua {
            self.colhendo = false;
        }
        continua
    }
}

/// A chave da foto do site no cache de previews.
///
/// 🔑 Num lugar só: eram três `format!("site:{}")` espalhados, e o dia em que um
/// mudasse os outros continuariam procurando no lugar antigo — a grade ficaria
/// vazia sem erro nenhum (armadilha das duas listas da mesma verdade).
fn chave_da_miniatura(foto_id: &str) -> String {
    format!(
        "{}{foto_id}",
        crate::revelacao::persistencia::PREFIXO_DO_SITE
    )
}

impl Detalhe {
    /// Põe na memória a miniatura de cada foto visível — **uma vez por quadro**,
    /// antes de o render começar.
    ///
    /// # Por que isto não está dentro da célula
    ///
    /// 🚨 Estava, e era o que travava a tela. `celula` e `tira` chamavam
    /// `get_preview` + `para_gpui` por foto **a cada quadro**, e o GPUI redesenha
    /// a cada movimento de mouse. Medido em release, 25 fotos:
    /// **51 ms por quadro**, contra 16,7 ms de orçamento — e sem melhorar nunca,
    /// porque a memória do `PreviewManager` guarda 15 imagens e a varredura era
    /// de 25: um LRU menor que a varredura acerta zero.
    ///
    /// # A miniatura que faltava
    ///
    /// As fotos da sessão só tinham o preview **grande** (640px) gravado:
    /// `Recado::Miniatura` chamava `save_preview`, nunca `save_thumbnail`. Então
    /// a célula de 160px carregava 0,3 MP para desenhar 0,02 MP, e o
    /// `get_thumbnail` da tira nunca acertava.
    ///
    /// Aqui a miniatura é gerada na primeira vez que a foto aparece e **fica
    /// gravada**: conserta também as que já estão no cache, sem precisar
    /// ressincronizar a sessão.
    fn preparar_miniaturas(&mut self) {
        let visiveis = self.acervo.total_visivel();
        if visiveis == 0 {
            return;
        }
        // A grade da sessão não é virtualizada — desenha o recorte inteiro —,
        // então o cache precisa caber o recorte inteiro.
        self.miniaturas.ajustar_capacidade(
            NonZeroUsize::new(visiveis.max(MINIATURAS_GUARDADAS)).expect("visiveis > 0"),
        );

        let chaves: Vec<String> = self
            .acervo
            .visiveis()
            .map(|foto| chave_da_miniatura(&foto.id))
            .collect();

        for chave in chaves {
            if self.miniaturas.espiar(&chave).is_some() {
                continue;
            }
            // Sem miniatura gravada: reduz o preview grande uma vez e a grava.
            // Da segunda abertura em diante o caminho é só o `get_thumbnail`.
            if self.previews.get_thumbnail(&chave).is_none() {
                if let Some(grande) = self.previews.get_preview(&chave) {
                    let pequena = reduzir(&grande, LADO_DA_MINIATURA);
                    let _ = self.previews.save_thumbnail(&chave, &pequena);
                }
            }
            self.miniaturas.obter(&self.previews, &chave);
        }
    }
}

/// Reduz sem ampliar — a mesma regra do `thumbnail_generator`.
///
/// ⚠️ **Ampliar não acrescenta detalhe**: espalha o que existe e faz toda a
/// cadeia trabalhar sobre pixels que a foto não tem. Uma foto que já é menor que
/// o lado pedido volta como está.
fn reduzir(imagem: &image::DynamicImage, lado: u32) -> image::DynamicImage {
    use image::GenericImageView;
    let (largura, altura) = imagem.dimensions();
    if largura <= lado && altura <= lado {
        return imagem.clone();
    }
    imagem.thumbnail(lado, lado)
}

impl Render for Detalhe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 🚨 **Antes de montar qualquer célula.** É o que tira o decode de dentro
        // do quadro; `celula` e `tira` daqui para baixo só leem da memória.
        self.preparar_miniaturas();
        self.seguir_o_foco();

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
    /// 🚨 **É aqui que se importa.** *"Quem faz a importação é o botão 'Escolher
    /// fotos…'"* — dono, 8/set/2026. Não é um caminho ao lado da importação: é
    /// ela, e por isso o botão passou a se chamar **"Importar"** no mesmo dia. O
    /// "Importar" que existia na barra do app saiu junto: era o segundo botão
    /// para o mesmo gesto, e o que abria o explorador errado.
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
        let lote = self.importacao;
        let ocupado = self.importando();
        let sem_sessao = self.aberta.is_none();
        let leva = self.leva;
        let visiveis = self.acervo.total_visivel();

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
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(match lote {
                                        // 🔑 **O número inteiro, e não só o que
                                        // já foi.** Quem mandou 500 quer saber
                                        // que são 500: "37 prontas" sozinho não
                                        // diz se falta pouco ou muito, e é essa
                                        // a única pergunta de quem espera.
                                        Some(l) if !l.terminou() => {
                                            let falhas = if l.falhas > 0 {
                                                format!(" · {} falharam", l.falhas)
                                            } else {
                                                String::new()
                                            };
                                            format!(
                                                "importando… {} de {}{falhas}",
                                                l.prontas(),
                                                l.total
                                            )
                                        }
                                        Some(l) if l.falhas > 0 => {
                                            format!("{} subiram · {} falharam", l.feitas, l.falhas)
                                        }
                                        Some(l) => format!("{} subiram", l.feitas),
                                        None => {
                                            "Arraste as fotos (ou a pasta) para cá.".to_string()
                                        }
                                    }),
                            )
                            // 🔑 **A barra só existe enquanto o lote corre.** Uma
                            // barra parada em 100% é ruído que o operador
                            // aprende a ignorar — e no dia em que ela importa,
                            // ele não olha.
                            .when_some(lote.filter(|l| !l.terminou()), |linha, l| {
                                linha.child(Progress::new().value(l.porcento()).h(px(4.)))
                            }),
                    )
                    .child(
                        // 🔑 **"Importar", e não "Escolher fotos…"** — dono,
                        // 8/set/2026: *"esse nome confunde, pois ao lado vai ter
                        // o botão Exportar"*. E confundia mesmo. "Escolher
                        // fotos…" descreve o **meio** (abre uma janela e se
                        // escolhe), enquanto o vizinho descreve o **fim**
                        // (exportar); lado a lado, um par que não é par faz
                        // procurar a entrada em outro lugar. Agora os dois
                        // dizem a direção: por aqui entra, por ali sai.
                        Button::new("detalhe-importar")
                            .label("Importar")
                            .xsmall()
                            .primary()
                            .disabled(ocupado || sem_sessao)
                            // 🔑 **Para o teste poder clicar onde o dedo clica.**
                            // `debug_selector` grava as coordenadas deste
                            // elemento no quadro desenhado, e é assim que o e2e
                            // acha o botão em vez de chamar o método por baixo —
                            // a diferença entre "a função existe" e "o clique
                            // chega até ela". **Não custa nada fora de teste**:
                            // sem a `feature = "test-support"` do gpui, o método
                            // é um `self` que devolve `self`.
                            .debug_selector(|| "detalhe-importar".into())
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.importar(cx))),
                    )
                    .child(
                        // 🔑 **Exportar mora ao lado de Importar**, e não
                        // na barra do app (de onde desceu em 8/set/2026, a
                        // pedido do dono). São o par: por aqui as fotos entram
                        // no ensaio, por aqui elas saem para o disco — e uma
                        // barra de distância entre os dois fazia procurar a
                        // saída em outro lugar da tela.
                        //
                        // ⚠️ **Sem seleção exporta o que a grade mostra**, como o
                        // botão da barra fazia: quem acabou de filtrar por
                        // "levadas" está pedindo essas. Quem escolhe as fotos
                        // ganha delas — a conta é da raiz, que é quem tem a
                        // grade; daqui só sai o pedido.
                        Button::new("detalhe-exportar")
                            .label("Exportar")
                            .xsmall()
                            .disabled(sem_sessao || visiveis == 0)
                            .debug_selector(|| "detalhe-exportar".into())
                            .on_click(
                                cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::Exportar)),
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
            // O zoom, como no site: duas lupas e a faixa entre elas.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("⊖")
                    .child(
                        div()
                            .w(px(112.))
                            .child(Slider::new(&self.zoom_slider).horizontal()),
                    )
                    .child("⊕"),
            )
            .child(
                // 🔑 **Sem exigir foco**, como o botão da barra do site: entra
                // no modo e a tira faz o resto. Só desliga quando não há o que
                // revelar — sessão vazia, ou só apagadas.
                Button::new("sessao-revelar")
                    .label("Revelar")
                    .xsmall()
                    .disabled(!self.tem_o_que_revelar())
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.revelar_todas(cx))),
            )
            .child(
                Button::new("sessao-tela-do-cliente")
                    .label(if self.cliente_aberta {
                        "Fechar a tela do cliente"
                    } else {
                        "Tela do cliente"
                    })
                    .xsmall()
                    .selected(self.cliente_aberta)
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

    /// As fotos que vão para a tira da Revelação: **as da sessão, na ordem da
    /// grade, sem as apagadas**.
    ///
    /// ⚠️ **O recorte da barra não encurta a lista.** É o que a web faz
    /// (`grade.tsx` passa `fotos`, a lista inteira, e `abrir-revelacao.tsx` só
    /// tira as apagadas): o operador filtra "sem nota" para achar uma foto, e
    /// dentro do editor ainda anda pelas outras. A apagada pela retenção não
    /// tem arquivo — não há o que revelar nem mostrar.
    fn fotos_a_revelar(&self) -> impl Iterator<Item = &acervo::Foto> {
        self.acervo.todas().iter().filter(|f| !f.apagada)
    }

    fn tem_o_que_revelar(&self) -> bool {
        self.fotos_a_revelar().next().is_some()
    }

    /// O pedido de entrar na Revelação — a sessão inteira, e onde começar.
    ///
    /// `comecar_em = None` é o botão da barra: a primeira que ainda pode ser
    /// revelada; se todas foram compradas, a primeira mesmo — a tira mostra a
    /// comprada marcada e não revelável, como no site. Com um id, é a posição
    /// dele na lista; um id que não está nela (a apagada em foco) não abre nada.
    fn pedido_de_revelar(&self, comecar_em: Option<&str>) -> Option<Pedido> {
        let fotos: Vec<FotoARevelar> = self
            .fotos_a_revelar()
            .map(|f| FotoARevelar {
                id: f.id.clone(),
                arquivo: f.arquivo.clone(),
            })
            .collect();
        if fotos.is_empty() {
            return None;
        }
        let inicial = match comecar_em {
            Some(id) => fotos.iter().position(|f| f.id == id)?,
            None => self
                .fotos_a_revelar()
                .position(acervo::Foto::editavel)
                .unwrap_or(0),
        };
        Some(Pedido::Revelar { fotos, inicial })
    }

    /// O botão da barra: entra na Revelação **sem escolher foto**.
    pub fn revelar_todas(&mut self, cx: &mut Context<Self>) {
        if let Some(pedido) = self.pedido_de_revelar(None) {
            cx.emit(pedido);
        }
    }

    /// O botão do painel e o duplo clique: abre a foto em foco, com a sessão
    /// inteira na tira.
    pub fn revelar_a_do_foco(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.em_foco().map(|f| f.id.clone()) else {
            return;
        };
        if let Some(pedido) = self.pedido_de_revelar(Some(&id)) {
            cx.emit(pedido);
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

        // 🚨 **`overflow_y_scroll`, e não `overflow_hidden`.** Com o hidden a
        // grade desenhava as linhas que não cabiam e as **escondia**: a última
        // fileira aparecia cortada ao meio, sem barra, e o trackpad não rolava
        // nada — não havia o que rolar, do ponto de vista do GPUI. Uma sessão de
        // 25 fotos já perdia a quarta linha.
        //
        // 🔑 O `.id()` não é enfeite: rolar é estado (a posição), e no GPUI só
        // elemento com id tem estado. Sem ele o `overflow_y_scroll` compila e
        // não rola.
        div()
            .id("grade-da-sessao")
            .track_scroll(&self.rolagem_da_grade)
            // 🔑 **Ctrl + roda dá zoom**, e é o que o site promete no `title` do
            // controle de tamanho. Sem o modificador a roda rola, que é o que
            // ela tem de fazer.
            .on_scroll_wheel(
                cx.listener(|tela, evento: &gpui::ScrollWheelEvent, window, cx| {
                    if !evento.modifiers.secondary() {
                        return;
                    }
                    let delta = evento.delta.pixel_delta(window.line_height());
                    if delta.y == px(0.) {
                        return;
                    }
                    let passo = if delta.y > px(0.) {
                        PASSO_DO_ZOOM
                    } else {
                        -PASSO_DO_ZOOM
                    };
                    tela.ajustar_zoom(passo, window, cx);
                }),
            )
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_wrap()
            .content_start()
            .gap(px(8.))
            .overflow_y_scroll()
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
        // 🔑 **Só lê.** Quem carrega é `preparar_miniaturas`, uma vez por quadro,
        // antes de o render começar — ver o campo `miniaturas`.
        let miniatura = match self.miniaturas.espiar(&chave_da_miniatura(&foto.id)) {
            Some(Miniatura::Pronta(imagem)) => Some(imagem),
            _ => None,
        };

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
                            // A web só oferece o botão quando a foto é
                            // editável: a comprada não se revela (o site
                            // responde 409 — o cliente pode já ter baixado).
                            Button::new("painel-revelar")
                                .label("Revelar")
                                .xsmall()
                                .disabled(!foto.editavel())
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

    /// A tira do rodapé — **o porte da `TiraDaBiblioteca` do site**.
    ///
    /// Era um quadrado de 56px com borda e nada dentro. O site
    /// (`tira-da-biblioteca.tsx`) tem outra coisa, e o dono pediu as duas
    /// iguais: a mesma tira aparece nas duas telas no mesmo dia de trabalho, e
    /// duas gramáticas para o mesmo gesto custam mais que qualquer das duas.
    ///
    /// O que veio de lá, e por quê:
    ///
    /// | | |
    /// |---|---|
    /// | miniatura em **paisagem** (`lado × 1,35`), recortada | um quadrado corta a foto no meio; a proporção da tira é a da foto |
    /// | **puxar a barra é o zoom** | pedido do dono, 5/set: a única dimensão livre da tira é a altura, e um controle separado seria um segundo jeito de dizer o mesmo |
    /// | nota, balcão e "comprada" **sobre** a foto | numa miniatura de 70px não há rodapé onde caibam. É a exceção consciente à regra 1 de [`crate::selos`] — lá o assunto é a célula da grade, que tem rodapé |
    /// | contador, teclas e as setas ‹ › | a tira é onde se anda, e andar sem mouse tem de estar escrito onde o gesto acontece |
    /// | sombras nas pontas | é o que diz que há mais foto fora da vista; sem elas a tira parece terminar na borda |
    ///
    /// ⚠️ **A roda vertical rola a tira.** Trackpad e mouse de roda produzem
    /// `deltaY` sobre uma faixa horizontal, e sem isto o gesto natural não faz
    /// nada — foi o primeiro relato do dono sobre esta tela.
    fn tira(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.acervo.total_visivel();
        let atual = self.selecao.foco().map(|i| i + 1).unwrap_or(0);
        let lado = altura_da_tira::lado_da_miniatura(self.altura_da_tira);
        let largura = lado * altura_da_tira::PROPORCAO;

        // As pontas: só há sombra onde ainda há foto fora da vista.
        let deslocamento = -self.rolagem_da_tira.offset().x;
        let maximo = self.rolagem_da_tira.max_offset().width;
        let tem_antes = deslocamento > px(4.);
        let tem_depois = maximo - deslocamento > px(4.);

        div()
            .flex()
            .flex_col()
            .flex_none()
            .child(self.puxador(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .pt(px(3.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(if atual > 0 {
                        format!("{atual} / {total}")
                    } else {
                        format!("{total} foto(s)")
                    }))
                    .child(tecla("←", cx))
                    .child(tecla("→", cx))
                    .child("andam ·")
                    .child(tecla("↑", cx))
                    .child(tecla("↓", cx))
                    .child("mudam de fileira ·")
                    .child(tecla("1", cx))
                    .child("–")
                    .child(tecla("5", cx))
                    .child("nota ·")
                    .child(tecla("P", cx))
                    .child("levada no balcão ·")
                    .child(tecla("Ctrl", cx))
                    .child("ou")
                    .child(tecla("Shift", cx))
                    .child("no clique marcam várias ·")
                    .child(tecla("Ctrl+A", cx))
                    .child("marca tudo,")
                    .child(tecla("Ctrl+D", cx))
                    .child("desmarca")
                    // As setas ficam na ponta direita, como no site.
                    .child(
                        div()
                            .ml_auto()
                            .flex()
                            .gap(px(2.))
                            .child(
                                Button::new("tira-anterior")
                                    .label("‹")
                                    .xsmall()
                                    .ghost()
                                    .disabled(total == 0)
                                    .on_click(cx.listener(|tela, _ev, _w, cx| tela.andar(-1, cx))),
                            )
                            .child(
                                Button::new("tira-proxima")
                                    .label("›")
                                    .xsmall()
                                    .ghost()
                                    .disabled(total == 0)
                                    .on_click(cx.listener(|tela, _ev, _w, cx| tela.andar(1, cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        div()
                            .id("tira-da-sessao")
                            .track_scroll(&self.rolagem_da_tira)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(10.))
                            .pb(px(6.))
                            .h(px(lado + 8.0))
                            .overflow_x_scroll()
                            // 🔑 A roda vertical rola na horizontal: é o gesto
                            // que a mão faz sobre uma faixa, e o mesmo que o
                            // site escuta com `passive: false`.
                            .on_scroll_wheel(cx.listener(
                                move |tela, evento: &gpui::ScrollWheelEvent, window, cx| {
                                    let delta = evento.delta.pixel_delta(window.line_height());
                                    if delta.y.abs() <= delta.x.abs() {
                                        return;
                                    }
                                    let atual = tela.rolagem_da_tira.offset();
                                    tela.rolagem_da_tira
                                        .set_offset(gpui::point(atual.x + delta.y, atual.y));
                                    cx.notify();
                                },
                            ))
                            .children(
                                self.acervo
                                    .visiveis()
                                    .enumerate()
                                    .map(|(posicao, foto)| {
                                        self.miniatura_da_tira(posicao, foto, lado, largura, cx)
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                    )
                    .when(tem_antes, |moldura| moldura.child(sombra(true, cx)))
                    .when(tem_depois, |moldura| moldura.child(sombra(false, cx))),
            )
    }

    /// Uma miniatura da tira, com os selos que o site desenha.
    fn miniatura_da_tira(
        &self,
        posicao: usize,
        foto: &acervo::Foto,
        lado: f32,
        largura: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let em_foco = self.selecao.foco() == Some(posicao);
        let marcada = self.selecao.tem(posicao);
        let miniatura = match self.miniaturas.espiar(&chave_da_miniatura(&foto.id)) {
            Some(Miniatura::Pronta(imagem)) => Some(imagem),
            _ => None,
        };
        let nota = foto.nota.unwrap_or(0).min(5) as usize;
        let levada = foto.estado == acervo::Estado::LevadaNoBalcao && !foto.apagada;
        let comprada = !foto.editavel() && !foto.apagada;

        div()
            .id(SharedString::from(format!("tira-{}", foto.id)))
            .relative()
            .w(px(largura))
            .h(px(lado))
            .flex_none()
            .overflow_hidden()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted)
            // 🔑 **Borda de 2px, e três estados** — como no site: cheia no foco,
            // esmaecida na marcada, invisível no resto. Antes eram dois estados
            // numa borda de 1px, e a tira ficava igual com uma ou dez marcadas.
            .border_2()
            .border_color(if em_foco {
                cx.theme().primary
            } else if marcada {
                cx.theme().primary.opacity(0.5)
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .when_some(miniatura, |celula, imagem| {
                celula.child(
                    img(imagem)
                        .size_full()
                        // `Cover` e não `Contain`: a miniatura preenche o
                        // retângulo, como o `object-cover` do site. Com
                        // `Contain` sobrariam duas tarjas do fundo em cada foto.
                        .object_fit(gpui::ObjectFit::Cover)
                        .when(foto.apagada, |imagem| imagem.opacity(0.4)),
                )
            })
            .when(nota > 0, |celula| {
                celula.child(
                    div()
                        .absolute()
                        .top(px(1.))
                        .left(px(3.))
                        .text_xs()
                        .text_color(cores::nota())
                        .child(SharedString::from("★".repeat(nota))),
                )
            })
            .when(levada, |celula| {
                celula.child(
                    div()
                        .absolute()
                        .top(px(3.))
                        .right(px(3.))
                        .size(px(7.))
                        .rounded_full()
                        .bg(cores::quente()),
                )
            })
            .when(comprada, |celula| {
                celula.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .py(px(1.))
                        .text_center()
                        .text_xs()
                        .bg(gpui::black().opacity(0.7))
                        .text_color(cx.theme().foreground)
                        .child("comprada"),
                )
            })
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
                    )
                }),
            )
            .into_any_element()
    }

    /// A barra que arrasta a altura da tira — **e a altura é o zoom**.
    ///
    /// 🚨 **O arrasto é escutado na janela, não no `div`.** Uma barra de 6px é
    /// menor que o primeiro movimento rápido do ponteiro: com `on_mouse_move` do
    /// próprio elemento, o cursor sai dela e o arrasto morre no meio — o defeito
    /// que faz o operador achar que "não pega". É a mesma razão do `canvas` do
    /// enquadramento da Revelação, e o mesmo remédio que o
    /// `setPointerCapture` dá no site.
    fn puxador(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ouvinte = cx.entity();
        let arrastando = self.arrasto_da_tira.is_some();

        div()
            .id("puxador-da-tira")
            .relative()
            .h(px(6.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(
                div()
                    .h(px(2.))
                    .w(px(32.))
                    .rounded_full()
                    .bg(cx.theme().border),
            )
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|tela, evento: &gpui::MouseDownEvent, _w, cx| {
                    tela.arrasto_da_tira = Some((evento.position.y, tela.altura_da_tira));
                    cx.notify();
                }),
            )
            .child(canvas(
                |_bounds, _window, _cx| {},
                move |_bounds, _prepaint, window, _cx| {
                    if !arrastando {
                        return;
                    }
                    window.on_mouse_event({
                        let esta = ouvinte.clone();
                        move |evento: &gpui::MouseMoveEvent, fase, _window, cx| {
                            if !fase.bubble() {
                                return;
                            }
                            esta.update(cx, |tela, cx| {
                                let Some((y, altura)) = tela.arrasto_da_tira else {
                                    return;
                                };
                                // Para cima é maior: a tira cresce contra o miolo.
                                let nova = altura_da_tira::limitar(
                                    altura + f32::from(y - evento.position.y),
                                );
                                if nova != tela.altura_da_tira {
                                    tela.altura_da_tira = nova;
                                    cx.notify();
                                }
                            });
                        }
                    });
                    window.on_mouse_event({
                        let esta = ouvinte.clone();
                        move |_evento: &gpui::MouseUpEvent, fase, _window, cx| {
                            if !fase.bubble() {
                                return;
                            }
                            esta.update(cx, |tela, cx| {
                                if tela.arrasto_da_tira.take().is_some() {
                                    altura_da_tira::guardar("sessao", tela.altura_da_tira);
                                    cx.notify();
                                }
                            });
                        }
                    });
                },
            ))
    }
}

/// Uma tecla escrita, como os `<kbd>` do site.
fn tecla(rotulo: &str, cx: &App) -> impl IntoElement {
    div()
        .px(px(3.))
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .child(SharedString::from(rotulo.to_string()))
}

/// A sombra de uma ponta: diz que há foto fora da vista.
fn sombra(esquerda: bool, cx: &App) -> impl IntoElement {
    let fundo = cx.theme().background;
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(px(28.))
        .when(esquerda, |lado| lado.left_0())
        .when(!esquerda, |lado| lado.right_0())
        .bg(gpui::linear_gradient(
            if esquerda { 90.0 } else { 270.0 },
            gpui::linear_color_stop(fundo, 0.0),
            gpui::linear_color_stop(fundo.opacity(0.0), 1.0),
        ))
}

/// Põe `novas` na ordem em que `antigas` já estavam.
///
/// 🚨 **A ordem da tela é da tela, não da resposta do servidor.** Numa triagem a
/// grade é um mapa que a mão memoriza: "a terceira da segunda fileira". O
/// servidor não promete ordem estável entre duas leituras, e dar nota fazia a
/// foto trocar de lugar — a próxima seta ia para outra foto, e a nota seguinte
/// caía na errada.
///
/// Quem não estava antes vai para o fim, na ordem em que veio: foto nova entra
/// no fim da grade, que é onde se espera encontrá-la.
fn ordenar_como_antes(novas: &mut [FotoDaGaleria], antigas: &[FotoDaGaleria]) {
    use std::collections::HashMap;
    let posicao: HashMap<&str, usize> = antigas
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.as_str(), i))
        .collect();
    novas.sort_by_key(|f| posicao.get(f.id.as_str()).copied().unwrap_or(usize::MAX));
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
            ajustes: None,
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        fotos: Vec<FotoDaGaleria>,
    ) -> (gpui::WindowHandle<Detalhe>, Arc<PublicadorDeMentira>) {
        let publicador = publicador_com(fotos, false);
        let janela = janela_com(
            cx,
            publicador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        (janela, publicador)
    }

    /// O publicador de mentira já com a galeria "g1" e as fotos dela.
    ///
    /// `demorada` segura as respostas até `responder()` — é o que permite
    /// afirmar sobre o **meio** de uma importação, e não só sobre o fim dela.
    fn publicador_com(fotos: Vec<FotoDaGaleria>, demorada: bool) -> Arc<PublicadorDeMentira> {
        Arc::new(PublicadorDeMentira {
            demorada,
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
        })
    }

    fn janela_com(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
        seletor: Arc<SeletorDeMentira>,
    ) -> gpui::WindowHandle<Detalhe> {
        cx.update(gpui_component::init);
        cx.add_window(move |_window, cx| {
            let dir = tempfile::TempDir::new().expect("diretório temporário");
            let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
            std::mem::forget(dir);
            let mut tela = Detalhe::nova(publicador, seletor, previews, cx);
            tela.definir_sessao(Sessao {
                access_token: "tok".into(),
                refresh_token: "ref".into(),
                access_vence_em: i64::MAX,
                refresh_vence_em: i64::MAX,
            });
            tela
        })
    }

    /// O orçamento de um quadro a 60fps — o mesmo de `medir-grade-da-sessao`.
    ///
    /// 🚨 **É o teto de um clique, e não uma meta.** Enquanto o gesto não
    /// devolve, a janela não redesenha: passar disto é largar um quadro, e o
    /// operador vê a interface "engasgar" no momento exato em que mandou 500
    /// fotos. Folgado de propósito para o `debug` do CI — o que este número pega
    /// não é meio milissegundo a mais, é o dia em que alguém puser trabalho
    /// **de lote** dentro do `on_click`.
    const ORCAMENTO_DE_UM_QUADRO: std::time::Duration = std::time::Duration::from_millis(16);

    /// Clica **no botão**, onde o dedo clicaria — e devolve quanto o clique
    /// demorou a ser respondido.
    ///
    /// 🔑 **É o que separa "a função existe" de "o clique chega até ela"**
    /// (`docs/07-E2E-TESTING.md` §1). O caminho medido é o inteiro: achar o
    /// elemento no quadro desenhado, descer o botão do mouse nas coordenadas
    /// dele, subir, e deixar os efeitos chegarem.
    fn clicar(
        cx: &mut TestAppContext,
        janela: &gpui::WindowHandle<Detalhe>,
        alvo: &'static str,
    ) -> std::time::Duration {
        let mut visual = gpui::VisualTestContext::from_window((*janela).into(), cx);
        visual.run_until_parked();
        let onde = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("o botão {alvo} não está desenhado na tela"));
        let comeco = std::time::Instant::now();
        visual.simulate_click(onde.center(), gpui::Modifiers::none());
        let gasto = comeco.elapsed();
        visual.run_until_parked();
        gasto
    }

    /// Deixa a colheita rodar — ela responde a cada `INTERVALO_DE_COLHEITA`.
    fn colher_ate_parar(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Detalhe>) {
        for _ in 0..20 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
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

    fn apagada(id: &str) -> FotoDaGaleria {
        FotoDaGaleria {
            apagada: true,
            ..foto(id, EstadoDaFotoNoSite::Disponivel, Some(3))
        }
    }

    fn ids_e_inicial(pedido: Option<Pedido>) -> (Vec<String>, usize) {
        match pedido {
            Some(Pedido::Revelar { fotos, inicial }) => {
                (fotos.into_iter().map(|f| f.id).collect(), inicial)
            }
            _ => panic!("esperava um pedido de revelar"),
        }
    }

    /// 🔑 **O botão da barra entra sem escolher foto, com a sessão inteira na
    /// tira** — o gesto do cabeçalho da web. Começa pela primeira que ainda pode
    /// ser revelada: a comprada fica na tira, marcada, mas não é por ela que se
    /// começa. A apagada nem entra: não tem arquivo.
    #[gpui::test]
    fn o_botao_da_barra_leva_a_sessao_inteira(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Comprada, Some(5)),
                apagada("b"),
                foto("c", EstadoDaFotoNoSite::Disponivel, None),
                foto("d", EstadoDaFotoNoSite::LevadaNoBalcao, Some(4)),
            ],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert!(tela.em_foco().is_none(), "nada em foco, e mesmo assim:");
                assert!(tela.tem_o_que_revelar());

                let (ids, inicial) = ids_e_inicial(tela.pedido_de_revelar(None));
                assert_eq!(ids, vec!["a", "c", "d"], "a apagada fica de fora");
                assert_eq!(
                    inicial, 1,
                    "começa na primeira editável — a sem nota é revelável"
                );

                // 🚨 O recorte da barra não encurta a tira: filtrado em "sem
                // nota" a grade mostra uma, e a Revelação recebe as três.
                tela.filtrar(Filtro::SemNota, cx);
                assert_eq!(tela.acervo.total_visivel(), 1);
                let (ids, _) = ids_e_inicial(tela.pedido_de_revelar(None));
                assert_eq!(ids.len(), 3);
            })
            .expect("a janela deve estar aberta");
    }

    /// O botão do painel abre **a foto em foco**, e a sessão vai junto: a
    /// posição é a dela na lista da tira, não na grade filtrada.
    #[gpui::test]
    fn o_botao_do_painel_abre_a_do_foco_com_a_sessao_na_tira(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Disponivel, None),
                foto("c", EstadoDaFotoNoSite::Disponivel, Some(3)),
            ],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.filtrar(Filtro::SemNota, cx);
                tela.selecao.clicar(0, false, Modificadores::default());
                assert_eq!(tela.em_foco().map(|f| f.id.as_str()), Some("b"));

                let foco = tela.em_foco().map(|f| f.id.clone()).unwrap();
                let (ids, inicial) = ids_e_inicial(tela.pedido_de_revelar(Some(&foco)));
                assert_eq!(ids, vec!["a", "b", "c"]);
                assert_eq!(
                    inicial, 1,
                    "a posição de 'b' na sessão, e não 0 na grade filtrada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Só compradas: o botão da barra ainda entra — a tira mostra o que há —,
    /// e só apagadas (ou nada) é o único caso em que ele fica desligado.
    #[gpui::test]
    fn sem_o_que_revelar_o_botao_desliga(cx: &mut TestAppContext) {
        let (so_apagadas, _) = janela(cx, vec![apagada("a"), apagada("b")]);
        entrar(cx, &so_apagadas);
        so_apagadas
            .update(cx, |tela, _window, _cx| {
                assert!(!tela.tem_o_que_revelar());
                assert!(tela.pedido_de_revelar(None).is_none());
            })
            .expect("a janela deve estar aberta");

        let (so_compradas, _) = janela(cx, vec![foto("a", EstadoDaFotoNoSite::Comprada, Some(5))]);
        entrar(cx, &so_compradas);
        so_compradas
            .update(cx, |tela, _window, _cx| {
                let (ids, inicial) = ids_e_inicial(tela.pedido_de_revelar(None));
                assert_eq!((ids, inicial), (vec!["a".to_string()], 0));
            })
            .expect("a janela deve estar aberta");
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  Os dois botões do painel de envio, clicados de verdade
    // ═══════════════════════════════════════════════════════════════════════

    /// Entra numa galeria cujo publicador segura as respostas.
    fn entrar_demorado(
        cx: &mut TestAppContext,
        janela: &gpui::WindowHandle<Detalhe>,
        publicador: &PublicadorDeMentira,
    ) {
        janela
            .update(cx, |tela, _window, cx| tela.entrar("g1".into(), cx))
            .expect("a janela deve estar aberta");
        publicador.responder();
        colher_ate_parar(cx, janela);
    }

    fn caminhos(quantos: usize) -> Vec<String> {
        (0..quantos)
            .map(|i| format!("/fotos/DSC_{i:04}.jpg"))
            .collect()
    }

    /// 🚨 **O clique no "Importar" abre o seletor do sistema e sobe o lote.**
    ///
    /// Este é o teste que o `docs/07-E2E-TESTING.md` §1 pede: o botão trocou de
    /// nome e de método em 8/set/2026 (`escolher_fotos` → `importar`), e um
    /// `on_click` que aponta para o lugar errado **não falha** — ele só não faz
    /// nada. O clique aqui é nas coordenadas do botão desenhado, e não uma
    /// chamada ao método por baixo.
    #[gpui::test]
    fn o_clique_no_importar_abre_o_seletor_e_sobe_o_lote(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::escolhe(&["/fotos/a.jpg", "/fotos/b.NEF"]));
        let publicador = publicador_com(
            vec![foto("x", EstadoDaFotoNoSite::Disponivel, Some(4))],
            false,
        );
        let janela = janela_com(cx, publicador.clone(), seletor.clone());
        entrar(cx, &janela);

        assert_eq!(seletor.pedidos(), 0, "nada abre sozinho");

        let gasto = clicar(cx, &janela, "detalhe-importar");
        assert_eq!(
            seletor.pedidos(),
            1,
            "o clique no botão não chegou ao seletor do sistema"
        );
        assert!(
            gasto < ORCAMENTO_DE_UM_QUADRO,
            "abrir o seletor custou {gasto:?}, mais que um quadro"
        );

        colher_ate_parar(cx, &janela);

        let enviados: Vec<String> = publicador
            .arquivos_enviados()
            .into_iter()
            .map(|(_, caminho, _, _)| caminho)
            .collect();
        assert_eq!(
            enviados,
            vec!["/fotos/a.jpg".to_string(), "/fotos/b.NEF".into()],
            "o que o seletor devolveu tinha de subir para a galeria"
        );
    }

    /// 🚨 **O clique no "Exportar" pede a exportação à raiz.**
    ///
    /// Ele desceu da barra do app em 8/set/2026 e, com a mudança, deixou de
    /// chamar um método: agora emite um `Pedido`. Uma ligação a mais para se
    /// perder — e o desfecho de perdê-la é um botão que não responde.
    #[gpui::test]
    fn o_clique_no_exportar_pede_a_exportacao(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Disponivel, Some(5)),
            ],
        );
        entrar(cx, &janela);

        let tela = janela.root(cx).expect("a raiz da janela");
        let pedidos = Arc::new(std::sync::Mutex::new(0usize));
        let _inscricao = cx.update({
            let pedidos = pedidos.clone();
            move |cx| {
                cx.subscribe(&tela, move |_tela, pedido: &Pedido, _cx| {
                    if matches!(pedido, Pedido::Exportar) {
                        *pedidos.lock().expect("os pedidos") += 1;
                    }
                })
            }
        });

        let gasto = clicar(cx, &janela, "detalhe-exportar");

        assert_eq!(
            *pedidos.lock().expect("os pedidos"),
            1,
            "o clique no Exportar não virou pedido à raiz"
        );
        assert!(
            gasto < ORCAMENTO_DE_UM_QUADRO,
            "pedir a exportação custou {gasto:?}, mais que um quadro"
        );
    }

    /// 🚨 **Com 500 fotos subindo, o estúdio continua trabalhando.**
    ///
    /// É o cenário do dono, 8/set/2026: *"imagine uma importação de 500 fotos;
    /// no meio dela o usuário precisa conseguir ir revelando e negociando com o
    /// cliente, fazendo classificações e sinalizações"*.
    ///
    /// Até este commit ele **não acontecia**, e falhava do pior jeito: em
    /// silêncio. `mudar_as_marcadas` — o caminho de `dar_nota` e
    /// `alternar_levada` — começava com `if alvos.is_empty() || self.enviando >
    /// 0 { return; }`, e `enviando` era o mesmo contador da importação. Durante
    /// o lote, apertar `4` ou `P` não fazia nada: sem erro, sem aviso, e sem
    /// nenhuma pista de que a culpa era da importação.
    #[gpui::test]
    fn com_a_importacao_correndo_classificar_sinalizar_e_revelar_continuam(
        cx: &mut TestAppContext,
    ) {
        let seletor = Arc::new(SeletorDeMentira {
            escolha: std::sync::Mutex::new(caminhos(500)),
            pedidos: std::sync::Mutex::new(0),
        });
        let publicador = publicador_com(
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Disponivel, Some(4)),
            ],
            true,
        );
        let janela = janela_com(cx, publicador.clone(), seletor.clone());
        entrar_demorado(cx, &janela, &publicador);

        // O lote sai — e fica no meio, que é onde o cenário acontece.
        let gasto = clicar(cx, &janela, "detalhe-importar");
        assert!(
            gasto < ORCAMENTO_DE_UM_QUADRO,
            "o clique que dispara 500 fotos custou {gasto:?}, mais que um quadro"
        );
        janela
            .update(cx, |tela, _window, cx| tela.colher(cx))
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, _window, cx| {
                let lote = tela.importacao().expect("o lote começou");
                assert_eq!(lote.total, 500);
                assert_eq!(lote.prontas(), 0, "nenhuma respondeu ainda");
                assert!(tela.importando());

                // ── Revelar ──────────────────────────────────────────────
                assert!(
                    tela.pedido_de_revelar(None).is_some(),
                    "revelar parou de responder durante a importação"
                );

                // ── Classificar ──────────────────────────────────────────
                tela.selecionar_tudo(cx);
                tela.dar_nota(5, cx);
            })
            .expect("a janela deve estar aberta");

        let notas: Vec<Option<Option<i16>>> = publicador
            .negociadas()
            .into_iter()
            .map(|(_, m)| m.nota)
            .collect();
        assert_eq!(
            notas,
            vec![Some(Some(5)), Some(Some(5))],
            "classificar não chegou ao site durante a importação"
        );

        // 🔑 A rodada de classificação responde na hora — `negociar` não é dos
        // que o `demorada` segura —, e é o que solta a próxima: duas rodadas
        // sobre a mesma seleção correm em série de propósito.
        colher_ate_parar(cx, &janela);

        // ── Sinalizar "levada no balcão" ─────────────────────────────────
        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar_tudo(cx);
                tela.alternar_levada(cx);
            })
            .expect("a janela deve estar aberta");

        let levadas = publicador
            .negociadas()
            .into_iter()
            .filter(|(_, m)| m.estado == Some(EstadoNoBalcao::LevadaNoBalcao))
            .count();
        assert_eq!(
            levadas, 2,
            "sinalizar levada não chegou ao site durante a importação"
        );

        // ── E a barra não andou por causa de nada disso ───────────────────
        janela
            .update(cx, |tela, _window, _cx| {
                let lote = tela.importacao().expect("o lote continua");
                assert_eq!(
                    lote.prontas(),
                    0,
                    "classificar e sinalizar adiantaram a barra da importação"
                );
                assert!(tela.importando(), "a importação se deu por terminada");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **A barra anda com o lote, e termina relendo a galeria.**
    ///
    /// ⚠️ **A falha conta como pronta.** Uma foto que o site recusou não
    /// responde de novo: deixá-la fora da conta prenderia a barra em 499 de 500
    /// para sempre — e o "importando…" nunca sairia da tela.
    #[gpui::test]
    fn a_barra_anda_com_o_lote_e_a_falha_conta_como_pronta(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira {
            escolha: std::sync::Mutex::new(caminhos(4)),
            pedidos: std::sync::Mutex::new(0),
        });
        let publicador = publicador_com(
            vec![foto("a", EstadoDaFotoNoSite::Disponivel, Some(4))],
            true,
        );
        let janela = janela_com(cx, publicador.clone(), seletor.clone());
        entrar_demorado(cx, &janela, &publicador);

        clicar(cx, &janela, "detalhe-importar");
        janela
            .update(cx, |tela, _window, cx| tela.colher(cx))
            .expect("a janela deve estar aberta");

        let porcento = |cx: &mut TestAppContext| {
            janela
                .update(cx, |tela, _window, _cx| {
                    tela.importacao().expect("o lote").porcento()
                })
                .expect("a janela deve estar aberta")
        };
        assert_eq!(porcento(cx), 0.);

        // Duas respondem — a barra vai à metade.
        publicador.responder_uma();
        publicador.responder_uma();
        janela
            .update(cx, |tela, _window, cx| tela.colher(cx))
            .expect("a janela deve estar aberta");
        assert_eq!(porcento(cx), 50.);

        // Uma falha. Ela **conta**: o que a barra mede é o que falta esperar.
        {
            let mut guardados = publicador.guardados.lock().expect("os guardados");
            let (canal, _) = guardados.remove(0);
            let _ = canal.send(Recado::Falhou("o site recusou".into()));
        }
        janela
            .update(cx, |tela, _window, cx| tela.colher(cx))
            .expect("a janela deve estar aberta");
        assert_eq!(porcento(cx), 75.);

        // A última. O lote acaba, e a galeria é relida para as novas
        // aparecerem na grade.
        let releituras_antes = publicador.abertas().len();
        publicador.responder_uma();
        colher_ate_parar(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                let lote = tela.importacao().expect("o lote terminou");
                assert_eq!((lote.feitas, lote.falhas), (3, 1));
                assert!(lote.terminou());
                assert!(!tela.importando(), "a barra tinha de sair da tela");
            })
            .expect("a janela deve estar aberta");
        assert!(
            publicador.abertas().len() > releituras_antes,
            "o fim do lote tem de reler a galeria — sem isso as fotos novas não aparecem"
        );
    }
}
