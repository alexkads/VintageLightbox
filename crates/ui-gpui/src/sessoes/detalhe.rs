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
use std::ops::Range;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::acervo::{self, Acervo, Filtro};
use biblioteca_core::dados_do_cliente::{self, DadosDoCliente};
use biblioteca_core::dinheiro;
use biblioteca_core::grade::{colunas_que_cabem, linhas_necessarias};
use biblioteca_core::selecao::{Modificadores, Selecao};
use domain::services::pos_venda::{
    EstadoDaFotoNoSite, EstadoNoBalcao, Estudio, FotoDaGaleria, GaleriaAberta, LinkDeAcesso,
    MudancaDaGaleria, Produto, Sessao,
};
use domain::services::PreviewType;
use gpui::{
    canvas, div, img, prelude::*, px, App, Context, Entity, EventEmitter, Focusable, SharedString,
    Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::progress::Progress;
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Disableable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use super::altura_da_tira;
use super::arquivos::SeletorDeFotos;
use crate::biblioteca::miniaturas::{CacheDeMiniaturas, Miniatura};
use crate::importacao::explorador::{Andamento, Freios, Importador};
use crate::pos_venda::porta::{GestoDoFim, Publicador, Recado};
use crate::revelacao::tela::faixa_desenhada;
use crate::selos;
use crate::tema::cores;
use domain::value_objects::{ImportMode, ImportOptions, OrganizationStrategy, RenamePattern};

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

/// O piso do cache de miniaturas da sessão.
///
/// 🚨 **Até 17/set/2026 o cache crescia até o recorte inteiro** — a grade não
/// era virtualizada, desenhava todas, e o cache tinha de caber todas: 2.000
/// fotos, 2.000 texturas de 320 px na memória, e um quadro de 537 ms (medido
/// pelo estresse). Agora a grade desenha só as linhas à vista e a tira só o
/// pedaço à vista; `preparar_miniaturas` dimensiona o cache pelo que **cabe na
/// janela** (três telas de cada), e este número é só o piso.
const MINIATURAS_GUARDADAS: usize = 64;

/// O respiro entre as células da grade, nas duas direções.
const VAO_DA_GRADE: f32 = 8.0;
/// O respiro entre as miniaturas da tira.
const VAO_DA_TIRA: f32 = 6.0;
/// O recuo da tira à esquerda — o `px(10.)` dela.
const RECUO_DA_TIRA: f32 = 10.0;
/// O `p(px(12.))` da tela, dos dois lados.
const MARGEM_DA_GRADE: f32 = 24.0;
/// A largura do painel da foto mais o `gap` do `corpo`.
const LARGURA_DO_PAINEL: f32 = 300.0 + 8.0;
/// Quanto tempo de um quadro pode ir para carregar miniatura **fora** da vista.
///
/// 🔑 A que está à vista carrega sempre, no mesmo quadro — célula vazia
/// piscando é pior que um quadro mais lento. A margem (a tela de cima e a de
/// baixo) é adiantamento: ela para no orçamento e continua no quadro seguinte.
const ORCAMENTO_DA_MARGEM: Duration = Duration::from_millis(4);

/// A largura da grade no último quadro desenhado, e em que condição.
///
/// 🔑 **A largura vem do quadro, e não da janela**: o menu lateral e o painel
/// da foto dividem a linha com a grade, e contar as colunas pela janela dava
/// mais colunas do que cabem. A janela e o painel ficam guardados para a conta
/// valer também entre um redimensionamento e o quadro seguinte.
#[derive(Debug, Clone, Copy)]
struct MedidaDaGrade {
    largura: f32,
    janela: f32,
    painel: bool,
}

/// Os recortes da barra, na ordem da web.
///
/// 🚨 **"Sem nota" é o último de propósito**: é um recorte de exceção — o que
/// está sem classificação não pode ir à venda, não recebe marca d'água e não
/// devia estar no storage. Ele existe para esvaziar, não para consultar.
/// O modificador do sistema, como o `useTeclaDeAtalho` do site: `⌘` no Mac,
/// `Ctrl` no resto. A dica que mostra a tecla errada ensina o gesto errado.
#[cfg(target_os = "macos")]
const MODIFICADOR: &str = "⌘";
#[cfg(not(target_os = "macos"))]
const MODIFICADOR: &str = "Ctrl";
#[cfg(target_os = "macos")]
const MODIFICADOR_A: &str = "⌘+A";
#[cfg(not(target_os = "macos"))]
const MODIFICADOR_A: &str = "Ctrl+A";
#[cfg(target_os = "macos")]
const MODIFICADOR_D: &str = "⌘+D";
#[cfg(not(target_os = "macos"))]
const MODIFICADOR_D: &str = "Ctrl+D";

const FILTROS: [(&str, Filtro); 7] = [
    ("Todas", Filtro::Todas),
    // 🔑 **Os dois passos do balcão, na ordem em que acontecem**: classificar
    // (a nota, que é o que sobe a foto) e sinalizar (a tecla P). "Sinalizada"
    // **é** a levada no balcão — *"as levadas são as sinalizadas"* (dono,
    // 2026-09-11) —, então o chip é o recorte de sempre com o nome que se usa
    // no balcão, e não um segundo caminho para a mesma conta.
    ("Classificadas", Filtro::Classificadas),
    (
        "Sinalizadas",
        Filtro::Situacao(acervo::Estado::LevadaNoBalcao),
    ),
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
    /// Voltar para a lista de sessões (ou para o caixa, se veio de lá).
    Voltar,
    /// Abrir ou recolher o menu lateral: nesta tela o botão dele mora na
    /// barra de cima, porque a galeria não tem o cabeçalho do painel.
    AlternarMenu,
    /// "Negociação…": o balcão com as fotos marcadas.
    Negociar(Vec<String>),
    /// "Apagar": a foto sai do site, com os dois arquivos.
    ApagarDoSite(String),
    /// A tecla `0`: estas fotos voltam para esta máquina e **depois** saem do
    /// acervo — a cláusula C21 do contrato da foto, em `app::resgate`.
    TirarDoAcervo(Vec<crate::app::resgate::AFotoQueVolta>),
    /// "Imprimir…": a folha de impressão com as fotos marcadas (só no desktop).
    Imprimir(Vec<String>),
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
    /// Classificar fotos que **só existem no disco** — o passo 3, pedido da
    /// grade da sessão.
    ///
    /// 🔑 **Quem grava a nota e decide quem sobe é a Biblioteca**, que é a dona
    /// do catálogo (`Classificou` → `subir_classificada`). Esta tela só diz
    /// quais e quanto.
    Classificar { ids: Vec<String>, nota: i32 },
    /// A importação gravou no catálogo local: a raiz precisa reler o acervo e
    /// devolver as fotos deste ensaio.
    ///
    /// 🔑 **A tela não fala com o banco**, como não fala com o site: quem tem a
    /// porta do acervo é a raiz.
    CatalogoMudou,
    /// Abrir a exportação com o que está na grade — o botão que desceu da barra
    /// do app em 8/set/2026, para o lado do "Importar".
    ///
    /// 🔑 **A tela não escolhe as fotos**, como não escolhe nada que atravesse
    /// para fora dela: quem sabe o que a grade tem marcado, e o que sobra
    /// quando nada está, é a raiz.
    Exportar,
}

/// Uma foto da grade da sessão, no que a Revelação precisa para abri-la.
///
/// 🚨 **`no_disco` não é enfeite, é a chave do cache.** A grade é uma só e tem
/// duas famílias dentro: a do site, cuja imagem foi baixada e gravada sob
/// `site:<id>`, e a que só existe no disco, gravada pelo importador sob o id do
/// catálogo, cru. É a mesma distinção que [`Detalhe::chave_da_foto`] já fazia
/// para desenhar a célula — e que faltava aqui.
///
/// Sem ela, a raiz tratava **toda** foto da sessão como do site: prefixava
/// `site:` no id do catálogo, zerava o caminho do arquivo e mandava buscar uma
/// cópia de trabalho que nunca existiu. O resultado era a Revelação abrindo em
/// "não tem preview no cache" com o JPEG e o preview ali do lado, no disco —
/// e a tira inteira preta. É o defeito de 8/set/2026 repetido uma tela adiante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoARevelar {
    pub id: String,
    pub arquivo: String,
    /// Se esta foto **ainda não subiu**: o id é o do catálogo local, e o arquivo
    /// está neste disco.
    pub no_disco: bool,
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
    ///
    /// 🚨 **Ninguém as carregava.** O campo, o `escolher_faixa` e o `faixa()`
    /// existiam desde o porte, e a lista chegava **vazia**: a barra não tinha o
    /// seletor, e a leva inteira subia na faixa da galeria. No site a faixa é
    /// uma das **duas escolhas antes dos arquivos** (`envio.tsx`), porque a
    /// sessão mista sobe em levas — a mãe sozinha numa faixa, a família em
    /// outra.
    produtos: Vec<Produto>,
    /// Os estúdios do cadastro — o seletor do cabeçalho, como no site.
    estudios: Vec<Estudio>,
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
    ///
    /// 🚨 **As duas desenham só o pedaço à vista.** Antes montavam o recorte
    /// inteiro a cada quadro: 76 ms com 300 fotos e 537 ms com 2.000 (estresse,
    /// 17/set/2026). As linhas de fora viram espaçadores do mesmo tamanho —
    /// ver [`Detalhe::grade`].
    rolagem_da_grade: gpui::ScrollHandle,
    rolagem_da_tira: gpui::ScrollHandle,
    /// A largura da grade no último quadro — ver [`MedidaDaGrade`].
    medida_da_grade: Option<MedidaDaGrade>,
    /// A altura de uma linha de texto `text_xs` — a célula tem duas.
    linha_de_texto: f32,
    /// A linha de texto como o quadro a desenhou, quando difere da conta.
    linha_medida: Option<f32>,
    /// As colunas que o quadro de fato desenhou, para a janela, o painel e o
    /// zoom daquele quadro — vence a conta quando as duas discordam.
    colunas_vistas: Option<((f32, bool, f32), usize)>,
    /// As colunas e as linhas com que este quadro montou a grade.
    colunas_da_grade: usize,
    linhas_da_grade: usize,
    /// As linhas da grade que viram elemento neste quadro: `[de, ate)`.
    grade_desenhada: (usize, usize),
    /// A foto que a grade ainda tem de trazer à vista.
    grade_a_seguir: Option<usize>,
    /// Se o quadro anterior desenhou a grade — só então a medida dela vale.
    grade_no_quadro: bool,
    /// A janela e o painel no quadro em curso — o que a medida guarda.
    janela_no_quadro: (f32, f32),
    painel_no_quadro: bool,
    /// O pedaço da tira que vira elemento neste quadro: `[de, ate)`.
    tira_desenhada: (usize, usize),
    /// A foto que a tira ainda tem de trazer à vista — espera a tira ter
    /// medida (o primeiro quadro não tem).
    tira_a_seguir: Option<usize>,
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
    /// Se a janela do sistema está aberta, esperando o operador escolher.
    ///
    /// 🚨 **Sem isto o clique no "Importar" não fazia nada** (achado pelo dono
    /// em 8/set/2026, com o app rodando). A colheita é um laço que acorda a cada
    /// 100 ms e **desiste quando não há mais nada a esperar** — e "esperar o
    /// operador escolher" não estava na conta. No primeiro tique depois do
    /// clique nada estava carregando, subindo nem baixando: o laço morria com a
    /// janela ainda aberta, e os caminhos chegavam a um canal que ninguém mais
    /// drenava. Nenhum erro, nenhum pisco — o gesto simplesmente não existia.
    ///
    /// ⚠️ **É seguro porque o seletor responde sempre** ([`SeletorDeFotos`]):
    /// lista vazia é a desistência de quem fechou a janela. Um seletor que
    /// engolisse a resposta deixaria este laço acordado para sempre — e é por
    /// isso que aquele contrato está escrito na `trait`, e não só combinado.
    escolhendo: bool,
    /// As fotos que o site já tem — a metade de cima do acervo.
    do_site: Vec<acervo::Foto>,
    /// As fotos **deste ensaio que só existem no disco** — importadas e ainda
    /// não classificadas.
    ///
    /// 🚨 **Sem elas a importação seria invisível**, que é o mesmo desfecho de
    /// não ter importado. A foto entra no catálogo pelo passo 1 e só vai ao
    /// site no passo 3; entre um e outro, a grade da sessão é o **único** lugar
    /// em que ela existe para o operador — e é dali que ele a classifica.
    ///
    /// 🔑 **Quem as traz é a raiz**, por [`Detalhe::definir_locais`]: a porta do
    /// catálogo é dela, como a do site.
    locais: Vec<acervo::Foto>,
    /// Os ids de [`Self::locais`], para decidir a chave da miniatura em O(1).
    ///
    /// 🚨 **A chave da foto local é o id cru; a da foto do site leva o prefixo
    /// `site:`.** Elas moram em caches diferentes porque vêm de lugares
    /// diferentes: a do site é baixada da API, a local é gravada pelo
    /// importador (`preview_storage.save(&photo.id(), …)`). Procurar a local
    /// sob `site:<id>` não acha nada, e o sintoma é a **célula preta** — a foto
    /// aparece na grade, com nome, estado e faixa, e sem imagem.
    ids_locais: std::collections::HashSet<String>,
    /// Quais fotos locais já têm a **revelada da receita padrão** no cache.
    ///
    /// 🔑 **Consultado uma vez por foto, e não por quadro.** Saber se a chave
    /// existe custa disco; `chave_da_foto` roda por foto visível a cada quadro,
    /// e era assim que a grade já travou uma vez (ver `preparar_miniaturas`).
    /// `revelada_chegou` tira a foto daqui, e a próxima consulta a refaz.
    com_revelada: std::collections::HashMap<String, bool>,
    /// Os controles do painel da foto em foco — ver [`CamposDoPainel`].
    campos_do_painel: Option<CamposDoPainel>,
    /// A gaveta do atendimento está aberta?
    atendimento_aberto: bool,
    /// A sanfona "Faixa, negociação e preço" do painel — **fechada por
    /// padrão**, como no site (`PainelColapsavel`, `padrao={false}`): o que se
    /// faz a cada foto fica em cima; o que se faz uma vez por atendimento, atrás
    /// de um clique.
    faixa_e_precos_aberto: bool,
    /// O "detalhes" do cabeçalho — os números e os prazos, como o popover do
    /// site. Ele se consulta uma vez por atendimento, e por isso não fica na
    /// faixa de cima: cada pixel ali é uma foto a menos na primeira olhada.
    detalhes_abertos: bool,
    /// A foto cujo "Apagar" está sendo perguntado — `(id, nome do arquivo)`.
    ///
    /// 🚨 **Um diálogo, como no site** (`useConfirmacao`): apagar tira a foto
    /// **e os arquivos dela** do site, e não há como desfazer pela tela. O
    /// balcão é tela de dedo rápido; a pergunta é o freio.
    apagar_confirmando: Option<(String, String)>,
    /// 🧪 O registro dos avisos de "esta foto mudou, releia".
    #[cfg(test)]
    reveladas_avisadas: Vec<String>,
    /// As fotos que a tecla `0` vai tirar do acervo, à espera do "sim".
    tirar_do_acervo_confirmando: Option<Vec<acervo::Foto>>,
    /// As predefinições que este app conhece — para a gaveta dizer o **nome** do
    /// preset padrão, e não o id.
    presets_da_receita: Vec<domain::entities::Preset>,
    /// O seletor da **faixa da próxima leva**, na barra de envio (site:
    /// `envio.tsx`). Criado no primeiro render, porque `nova` não tem `window`.
    escolha_da_leva: Option<Entity<SelectState<SearchableVec<OpcaoDaFaixa>>>>,
    /// O seletor do **estúdio da sessão**, no cabeçalho (site:
    /// `estudio-da-galeria.tsx`).
    escolha_do_estudio: Option<Entity<SelectState<SearchableVec<OpcaoDaFaixa>>>>,
    /// As assinaturas dos dois — sem elas o `Confirm` não chega a lugar nenhum.
    _escolhas_da_barra: Vec<gpui::Subscription>,
    /// Quem grava a foto **no catálogo local** — o SQLite desta máquina.
    ///
    /// 🚨 **A importação não sobe nada**, e essa é a regra do dono (8/set/2026):
    /// *"a importação não vai imediatamente para o storage cloud, pois o cliente
    /// precisa classificar a foto; ela fica local usando sqlite"*. Até esse dia
    /// o botão chamava `Publicador::enviar_arquivo`, e o site devolvia **400
    /// Bad Request: a foto sobe classificada, informe a nota de 1 a 5** — 21 de
    /// 21 falhavam, e a sessão ficava vazia. O site estava certo: quem autoriza
    /// a foto a subir é o passo 3, e não o passo 1.
    importador: Arc<dyn Importador>,
    /// O canal por onde o lote conta o que já fez.
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    /// Pausa e cancelamento do lote — a tela é dona deles.
    freios: Freios,
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
    /// ✏️ O formulário dos dados do cliente, quando aparece — o "Editar" e o
    /// pedido de contato do fim da sessão. Ver [`FormularioDoCliente`].
    dados_do_cliente: Option<FormularioDoCliente>,
    /// O que foi mandado ao site e ainda não voltou: o motivo (para seguir o
    /// gesto) e os dados conferidos (para aplicar à sessão aberta). Fora do
    /// formulário de propósito — fechá-lo no meio não perde a resposta.
    gravando_dados: Option<(MotivoDoFormulario, DadosDoCliente)>,
    carregando: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

/// Por que o formulário dos dados do cliente está aberto — e o que fazer depois
/// de gravar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotivoDoFormulario {
    /// "Editar": título, e-mail e WhatsApp. O contato pode ficar vazio.
    Editar,
    /// 🔚 O "Copiar link" pediu o contato; depois de gravar, pede o link.
    ContatoParaOLink,
    /// 🔚 O "Avisar" pediu o contato; depois de gravar, avisa (se veio e-mail).
    ContatoParaAvisar,
}

/// ✏️ Um formulário só para os dados do cliente, nos dois modos — o mesmo
/// desenho do `dados-do-cliente-dialog.tsx` da web (dono, 2026-09-13).
struct FormularioDoCliente {
    motivo: MotivoDoFormulario,
    /// `None` até a primeira pintura com janela — ver
    /// `Detalhe::preparar_formulario_do_cliente`.
    campos: Option<CamposDoCliente>,
    /// A frase da recusa: a conferência local, o `400` do site, ou o `422`
    /// que abriu o pedido.
    erro: Option<SharedString>,
}

struct CamposDoCliente {
    titulo: Entity<InputState>,
    email: Entity<InputState>,
    whatsapp: Entity<InputState>,
    /// Enter grava — as inscrições vivem enquanto os campos viverem.
    _enter: [gpui::Subscription; 3],
}

/// Um campo já com o valor que a sessão tem.
/// Uma faixa (tipo de ensaio) no menu do painel. `id` vazio = o padrão da
/// galeria, que é como o site também oferece a primeira opção.
#[derive(Debug, Clone)]
pub(crate) struct OpcaoDaFaixa {
    id: String,
    titulo: SharedString,
}

impl SelectItem for OpcaoDaFaixa {
    type Value = String;

    fn title(&self) -> SharedString {
        self.titulo.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }

    fn display_title(&self) -> Option<gpui::AnyElement> {
        None
    }
}

/// Os controles do painel da foto em foco — a faixa e o preço de venda.
///
/// 🔑 **Criados sob demanda, e um jogo por foto.** `Detalhe::nova` não tem
/// `window` (ver o comentário do `zoom_slider`), e `SelectState`/`InputState`
/// pedem uma; então eles nascem no primeiro render em que há foco, como os
/// campos do cliente. Trocar de foto refaz o jogo: um `InputState` guarda texto,
/// e reaproveitá-lo mostraria o preço da foto anterior na foto de agora.
struct CamposDoPainel {
    foto_id: String,
    faixa: Entity<SelectState<SearchableVec<OpcaoDaFaixa>>>,
    preco: Entity<InputState>,
    _assinaturas: Vec<gpui::Subscription>,
}

fn campo_preenchido(
    valor: &str,
    dica: &'static str,
    window: &mut Window,
    cx: &mut Context<Detalhe>,
) -> Entity<InputState> {
    let estado = cx.new(|cx| InputState::new(window, cx).placeholder(dica));
    let valor = valor.to_string();
    estado.update(cx, |campo, cx| campo.set_value(valor, window, cx));
    estado
}

impl Detalhe {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        seletor: Arc<dyn SeletorDeFotos>,
        importador: Arc<dyn Importador>,
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
            medida_da_grade: None,
            linha_de_texto: 16.0,
            linha_medida: None,
            colunas_vistas: None,
            colunas_da_grade: 1,
            linhas_da_grade: 0,
            grade_desenhada: (0, 0),
            grade_a_seguir: None,
            grade_no_quadro: false,
            janela_no_quadro: (0.0, 1000.0),
            painel_no_quadro: false,
            tira_desenhada: (0, 0),
            tira_a_seguir: None,
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
            estudios: Vec::new(),
            faixa: None,
            zoom: ZOOM_PADRAO,
            avisando: false,
            pedindo_link: false,
            cliente_aberta: false,
            do_site: Vec::new(),
            locais: Vec::new(),
            ids_locais: std::collections::HashSet::new(),
            com_revelada: std::collections::HashMap::new(),
            campos_do_painel: None,
            atendimento_aberto: false,
            faixa_e_precos_aberto: false,
            detalhes_abertos: false,
            apagar_confirmando: None,
            #[cfg(test)]
            reveladas_avisadas: Vec::new(),
            tirar_do_acervo_confirmando: None,
            presets_da_receita: Vec::new(),
            escolha_da_leva: None,
            escolha_do_estudio: None,
            _escolhas_da_barra: Vec::new(),
            importador,
            andamentos: channel(),
            freios: Freios::default(),
            mudando: 0,
            escolhendo: false,
            importacao: None,
            link: None,
            dados_do_cliente: None,
            gravando_dados: None,
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
        //
        // 🚨 **Inclusive a grade** (dono, 18/set/2026: *"parece que a sessão
        // anterior ainda estava na memória, uma por cima da outra"*). As fotos
        // do site só eram trocadas quando a galeria nova respondia, e as
        // **locais** só quando a raiz mandasse outras — se a sessão nova não
        // tivesse nenhuma foto no disco, as da anterior ficavam na tela para
        // sempre. Zerar aqui deixa a grade vazia pelo tempo da leitura, que é o
        // estado honesto: ainda não se sabe o que esta sessão tem.
        self.galeria_id = Some(galeria_id.clone());
        self.aberta = None;
        self.do_site.clear();
        self.locais.clear();
        self.ids_locais.clear();
        self.com_revelada.clear();
        self.miniaturas.esvaziar();
        self.recompor_acervo();
        self.selecao.limpar_tudo();
        self.pedidas.clear();
        self.baixando = 0;
        self.link = None;
        // O formulário de outro cliente aberto na tela deste seria o mesmo erro.
        self.dados_do_cliente = None;
        self.gravando_dados = None;
        self.importacao = None;
        self.erro = None;
        self.carregando = true;

        // O catálogo e os estúdios vêm junto: são eles que enchem o seletor de
        // faixa da barra de envio e o do estúdio no cabeçalho.
        self.publicador
            .produtos(sessao.clone(), self.recados.0.clone());
        self.publicador
            .estudios(sessao.clone(), self.recados.0.clone());
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

    /// A conta do site — o caixa flutuante fala com a API em nome dela.
    pub fn sessao(&self) -> Option<&Sessao> {
        self.sessao.as_ref()
    }

    /// Leva a grade até uma foto — o clique num item do cupom do caixa. Ela
    /// fica sozinha marcada e em foco, como o `focar` da grade do site. A foto
    /// fora do recorte fica onde está: trocar o recorte por baixo do operador
    /// seria pior que não seguir.
    pub fn focar_foto(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(p) = self.acervo.posicao_de(id) {
            self.selecao.clicar(p, false, Modificadores::default());
            cx.notify();
        }
    }

    /// Relê a galeria aberta **sem sair dela** — seleção e foco ficam, pela
    /// mesma tradução por id da releitura depois de uma nota. É o que o caixa
    /// pede depois de gravar faixa ou negociação por fora desta tela.
    pub fn reler(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(id)) = (self.sessao.clone(), self.galeria_id.clone()) else {
            return;
        };
        self.publicador
            .abrir_galeria(sessao, id, self.recados.0.clone());
        self.acompanhar(cx);
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

    /// Marca estas fotos (ids da grade) — a seleção que volta da Revelação.
    ///
    /// 🔑 *"As fotos selecionadas no Filmstrip da sessão precisam serem
    /// selecionadas também no filmstrip da revelação"* (dono, 2026-09-14): a
    /// seleção é uma só, nas duas direções. As que o recorte esconde ficam de
    /// fora, como na grade.
    pub fn marcar_ids(&mut self, ids: &[String], cx: &mut Context<Self>) {
        self.selecao.limpar_tudo();
        for posicao in ids.iter().filter_map(|id| self.acervo.posicao_de(id)) {
            self.selecao.marcar(posicao);
        }
        cx.notify();
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

    /// A primeira foto do recorte — o que a tela do cliente mostra sem foco.
    pub fn primeira_visivel(&self) -> Option<&acervo::Foto> {
        self.acervo.visivel(0)
    }

    pub fn posicao_em_foco(&self) -> Option<usize> {
        self.selecao.foco()
    }

    /// 🧪 O "Apagar" do painel, sem o clique — o cenário e2e o usa para afirmar
    /// **o que a pergunta diz** antes de confirmar.
    #[cfg(test)]
    pub(crate) fn apagar_do_site_para_teste(&mut self, cx: &mut Context<Self>) {
        self.apagar_do_site(cx);
    }

    /// 🧪 O arquivo citado na pergunta de apagar, se ela está aberta.
    #[cfg(test)]
    pub(crate) fn arquivo_na_pergunta_de_apagar(&self) -> Option<String> {
        self.apagar_confirmando
            .as_ref()
            .map(|(_, arquivo)| arquivo.clone())
    }

    /// 🧪 A faixa da foto em foco, sem abrir o seletor.
    #[cfg(test)]
    pub(crate) fn mudar_faixa_para_teste(&mut self, produto_id: &str, cx: &mut Context<Self>) {
        self.mudar_faixa(produto_id.to_string(), cx);
    }

    /// Os ids das fotos do recorte em vigor, na ordem da grade — que é a mesma
    /// da tira.
    ///
    /// 🔑 **É o que os cenários e2e afirmam.** Contar fotos não diz **quais**
    /// ficaram: um recorte que troque duas fotos de lugar, ou que deixe entrar a
    /// comprada em "à venda", passa por qualquer teste que só some.
    pub fn ids_visiveis(&self) -> Vec<String> {
        (0..self.acervo.total_visivel())
            .filter_map(|p| self.acervo.visivel(p).map(|f| f.id.clone()))
            .collect()
    }

    /// 🧪 As fotos sobre as quais a grade foi avisada — "esta mudou, releia".
    #[cfg(test)]
    pub(crate) fn reveladas_avisadas(&self) -> Vec<String> {
        self.reveladas_avisadas.clone()
    }

    /// `(estado, nota, revelada)` de uma foto visível — o que a célula mostra.
    pub fn como_esta(&self, id: &str) -> Option<(acervo::Estado, Option<u8>, bool)> {
        let p = self.acervo.posicao_de(id)?;
        let f = self.acervo.visivel(p)?;
        Some((f.estado, f.nota, f.revelada))
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
    ///
    /// 🔑 **A rolagem é por conta.** A grade e a tira só têm como elemento o
    /// pedaço à vista, então "rolar até o filho N" não serve: a posição dele é
    /// `N × passo`, e é essa conta que decide — o mínimo, como o
    /// `scroll_to_item` do `div` fazia.
    fn seguir_o_foco(&mut self) {
        let foco = self.selecao.foco();
        if foco != self.ultimo_foco {
            self.ultimo_foco = foco;
            if let Some(posicao) = foco {
                self.grade_a_seguir = Some(posicao);
                self.tira_a_seguir = Some(posicao);
            }
        }
        self.seguir_na_grade();
        self.seguir_na_tira();
    }

    /// Traz à vista da grade a foto pendente, se a grade já tem medida.
    fn seguir_na_grade(&mut self) {
        let Some(posicao) = self.grade_a_seguir else {
            return;
        };
        let vista = f32::from(self.rolagem_da_grade.bounds().size.height);
        if vista <= 0. {
            return;
        }
        self.grade_a_seguir = None;
        let passo = self.passo_da_grade();
        let topo = (posicao / self.colunas_da_grade.max(1)) as f32 * passo;
        let base = topo + passo - VAO_DA_GRADE;
        let atual = self.rolagem_da_grade.offset();
        let y = f32::from(atual.y);
        let novo = if topo + y < 0. {
            -topo
        } else if base + y > vista {
            vista - base
        } else {
            return;
        };
        self.rolagem_da_grade
            .set_offset(gpui::point(atual.x, px(novo)));
    }

    /// A distância de uma linha da grade à seguinte.
    ///
    /// 🔑 **Por conta, e exata**: a célula é `lado × 0,72` de foto, dois
    /// respiros de 2 px e duas linhas de texto. Com a medida arredondada do
    /// quadro, a décima linha já caía meio pixel fora de onde o `flex_wrap`
    /// a punha.
    fn passo_da_grade(&self) -> f32 {
        let linha = self.linha_medida.unwrap_or(self.linha_de_texto);
        self.zoom * 0.72 + 4.0 + 2.0 * linha + VAO_DA_GRADE
    }

    /// Traz à vista da tira a foto pendente, se a tira já tem medida.
    fn seguir_na_tira(&mut self) {
        let Some(posicao) = self.tira_a_seguir else {
            return;
        };
        let vista = f32::from(self.rolagem_da_tira.bounds().size.width);
        if vista <= 0. {
            return;
        }
        self.tira_a_seguir = None;
        let largura = self.largura_na_tira();
        let esquerda = RECUO_DA_TIRA + posicao as f32 * (largura + VAO_DA_TIRA);
        let direita = esquerda + largura;
        let atual = self.rolagem_da_tira.offset();
        let x = f32::from(atual.x);
        let novo = if esquerda + x < 0. {
            -esquerda
        } else if direita + x > vista {
            vista - direita
        } else {
            return;
        };
        self.rolagem_da_tira
            .set_offset(gpui::point(px(novo), atual.y));
    }

    /// A largura de uma miniatura da tira — a altura dela é o zoom.
    fn largura_na_tira(&self) -> f32 {
        altura_da_tira::lado_da_miniatura(self.altura_da_tira) * altura_da_tira::PROPORCAO
    }

    /// A largura que a grade tem, com a janela desta largura.
    ///
    /// 🚨 **O painel da foto entra na conta.** Ele tem 300px fixos e divide a
    /// linha com a grade; ignorá-lo daria mais colunas do que cabem, e a seta ↓
    /// pularia por cima de uma foto. É o mesmo cuidado que `largura_util` da
    /// Biblioteca tem com a árvore de pastas.
    ///
    /// 🔑 **Com medida, o menu lateral também entra** — a medida é a largura que
    /// o quadro deu à grade. Sem ela (antes do primeiro quadro), a conta é pela
    /// janela, como era.
    fn largura_da_grade(&self, janela: f32) -> f32 {
        let painel = self.em_foco().is_some();
        match self.medida_da_grade {
            Some(m) => {
                let mut largura = m.largura + (janela - m.janela);
                if m.painel && !painel {
                    largura += LARGURA_DO_PAINEL;
                } else if !m.painel && painel {
                    largura -= LARGURA_DO_PAINEL;
                }
                largura
            }
            None => janela - MARGEM_DA_GRADE - if painel { LARGURA_DO_PAINEL } else { 0. },
        }
    }

    /// Quantas colunas a grade da sessão desenha agora.
    ///
    /// 🔑 **É a mesma conta que monta as linhas**, então ↑↓ andam exatamente
    /// uma linha da tela.
    pub fn colunas_visiveis(&self, window: &Window) -> usize {
        let janela = f32::from(window.viewport_size().width);
        let chave = (janela, self.em_foco().is_some(), self.zoom);
        if let Some((vista, colunas)) = self.colunas_vistas {
            if vista == chave {
                return colunas;
            }
        }
        colunas_que_cabem(self.largura_da_grade(janela), self.zoom, VAO_DA_GRADE)
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
            self.pedir_para_tirar_do_acervo(cx);
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
    /// A segunda tela está no ar? É o que o botão da barra desenha.
    pub fn cliente_aberta(&self) -> bool {
        self.cliente_aberta
    }

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

        // 🚨 **A foto que só existe no disco faz outro caminho, e é o passo 3.**
        // Ela não tem linha no site: mandar `negociar` com o id local devolveria
        // erro para todas. O que a leva ao site é a **nota** — e é por isso que
        // só a classificação atravessa daqui. Sinalizar "levada" numa foto que
        // ainda não subiu não tem onde ser gravado, e o silêncio seria a pior
        // resposta: a linha de erro abaixo diz o que fazer antes.
        let (locais, alvos): (Vec<String>, Vec<String>) = alvos
            .into_iter()
            .partition(|id| self.locais.iter().any(|f| &f.id == id));
        if !locais.is_empty() {
            match mudanca.nota {
                Some(Some(nota)) => cx.emit(Pedido::Classificar {
                    ids: locais,
                    nota: nota as i32,
                }),
                _ => {
                    self.erro = Some(
                        "estas fotos ainda não subiram — classifique-as (1 a 5) antes \
                         de marcar no balcão"
                            .into(),
                    );
                    cx.notify();
                }
            }
        }

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
        // 🚨 **Antes de abrir a janela, e não depois.** É isto que segura a
        // colheita de pé pelos segundos em que o operador procura a pasta.
        self.escolhendo = true;
        self.seletor.escolher(self.escolhas.0.clone());
        self.acompanhar(cx);
    }

    /// As fotos deste ensaio que a raiz achou no catálogo local.
    ///
    /// 🚨 **Só as que ainda não subiram.** Uma foto classificada existe dos dois
    /// lados — linha no SQLite *e* linha no site —, e pôr as duas na grade
    /// mostraria a mesma foto duas vezes, com estados diferentes. Quem já subiu
    /// vale pela do site, que é a que tem preço, nota e negociação.
    pub fn definir_locais(&mut self, fotos: Vec<acervo::Foto>, cx: &mut Context<Self>) {
        if self.locais == fotos {
            return;
        }
        self.locais = fotos;
        self.ids_locais = self.locais.iter().map(|f| f.id.clone()).collect();
        self.recompor_acervo();
        cx.notify();
    }

    /// Monta o acervo da grade: **o que está no site, e o que só está no disco**.
    ///
    /// 🔑 **As locais vêm depois**, e é de propósito: a ordem da grade é a ordem
    /// da sessão, e o que acabou de ser importado é o mais novo. Quem importa
    /// 500 quer vê-las onde as deixou — no fim.
    fn recompor_acervo(&mut self) {
        let marcadas = self.ids_marcados();
        let focada = self.em_foco().map(|f| f.id.clone());

        let mut todas = self.do_site.clone();
        todas.extend(self.locais.iter().cloned());
        self.acervo.definir(todas);

        // A seleção fala em **posição**, e a lista mudou de tamanho: quem
        // continua visível volta marcado, e quem saiu do recorte fica de fora.
        self.selecao.limpar_tudo();
        for id in &marcadas {
            if let Some(p) = self.acervo.posicao_de(id) {
                self.selecao.marcar(p);
            }
        }
        if let Some(p) = focada.as_deref().and_then(|id| self.acervo.posicao_de(id)) {
            self.selecao.focar(Some(p));
            self.ultimo_foco = None;
        }
    }

    fn ids_marcados(&self) -> Vec<String> {
        self.selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p))
            .map(|f| f.id.clone())
            .collect()
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

    /// Grava os arquivos escolhidos **no catálogo local** — o passo 1.
    ///
    /// 🚨 **Nada sobe aqui, e é a regra do dono** (8/set/2026): *"a importação
    /// não vai imediatamente para o storage cloud, pois o cliente precisa
    /// classificar a foto; ela fica local usando sqlite"*. Quem autoriza a foto
    /// a ir para o site é o **passo 3** — classificar —, e o site já dizia isso
    /// sozinho: `400 Bad Request: a foto sobe classificada: informe a nota de 1
    /// a 5`, 21 vezes em 21 arquivos.
    ///
    /// 🔑 **A foto entra carimbada com o ensaio** (`sessao_id`). Sem o carimbo
    /// ela chega ao catálogo sem dono e não aparece na grade da sessão que a
    /// importou — e o sintoma é "a importação não funcionou".
    ///
    /// 🚨 **O arquivo é COPIADO para a pasta do ensaio, e nunca catalogado onde
    /// está** (regra do dono, 8/set/2026). O motivo é o cartão de memória: com
    /// `ImportMode::Add` o catálogo guardaria `/Volumes/NIKON D750/DCIM/…`, e a
    /// foto **desapareceria do app no instante em que o cartão saísse** — no
    /// meio de uma sessão, com o cliente na frente. Pior: formatar o cartão
    /// para a próxima sessão apagaria o ensaio inteiro, sem aviso e sem volta.
    ///
    /// 🔑 **E vai para um lugar previsível**: `<catálogo>/Ensaios/<ensaio>`, numa
    /// pasta só, com os nomes que saíram da câmera. É o que responde *"onde
    /// estão as fotos do Teste 003?"* sem abrir o app. Os padrões do
    /// `ImportOptions` diriam outra coisa, e por isso os três são escritos aqui:
    ///
    /// | Padrão | O que faria | Por que não serve aqui |
    /// |---|---|---|
    /// | `ByDate` | `YYYY/MM/DD` da EXIF | um ensaio de dois dias vira duas pastas, e um cartão com fotos antigas se espalha por meses |
    /// | `Standard` | renomeia para `photo-2026-09-08-001.jpg` | o operador procura por `DSC_2571.jpg`, que é o que a câmera deu e o que ele vê no Lightroom |
    ///
    /// ⚠️ **A pasta é decidida pelo id da galeria**, com o título junto só para
    /// ser achável no Finder. Renomear o ensaio no site faz o **próximo** lote
    /// ir para uma pasta nova; o que já entrou fica onde está, e continua
    /// catalogado — o caminho de cada foto está no banco, não no nome da pasta.
    pub fn enviar_arquivos(&mut self, caminhos: Vec<String>, cx: &mut Context<Self>) {
        let Some(galeria_id) = self.galeria_id.clone() else {
            return;
        };
        if caminhos.is_empty() || self.importando() {
            return;
        }

        self.erro = None;
        self.importacao = Some(Importacao {
            total: caminhos.len(),
            feitas: 0,
            falhas: 0,
        });
        self.freios = Freios::default();
        let titulo = self
            .aberta
            .as_ref()
            .map(|a| a.galeria.titulo.as_str())
            .unwrap_or_default();
        self.importador.importar(
            caminhos,
            ImportOptions {
                sessao_id: Some(galeria_id.clone()),
                // 🚨 Copiar, nunca catalogar onde está — ver o aviso acima.
                mode: ImportMode::Copy,
                destination: Some(
                    pasta_do_ensaio(titulo, &galeria_id)
                        .to_string_lossy()
                        .into_owned(),
                ),
                organization: OrganizationStrategy::IntoOneFolder,
                // 🚨 **UUID no disco, nome de origem no catálogo** — proposta
                // do dono, 8/set/2026, e o que a nuvem já fazia. Ver
                // `RenamePattern::Uuid` e a migration 022.
                rename_pattern: RenamePattern::Uuid,
                ..Default::default()
            },
            self.freios.clone(),
            self.andamentos.0.clone(),
        );
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
        // 🔚 Fim da sessão sem e-mail: pede o e-mail e segue depois de gravar.
        if self.aberta.is_some() && !self.tem_email() {
            self.abrir_formulario_do_cliente(MotivoDoFormulario::ContatoParaAvisar, None, cx);
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
        // 🔚 Fim da sessão sem e-mail: pede o e-mail e segue depois de gravar.
        if self.aberta.is_some() && !self.tem_email() {
            self.abrir_formulario_do_cliente(MotivoDoFormulario::ContatoParaOLink, None, cx);
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

    // ── Os dados do cliente: título, e-mail e WhatsApp ──────────────────────

    /// A sessão aberta tem e-mail — o que o link e o aviso exigem?
    ///
    /// 🚨 O WhatsApp não conta: o link entra na conta do e-mail, e o aviso vai
    /// por e-mail (usuários em produção, 2026-09-13).
    pub fn tem_email(&self) -> bool {
        self.aberta
            .as_ref()
            .is_some_and(|a| dados_do_cliente::tem_email(a.galeria.email.as_deref()))
    }

    /// Por que o formulário dos dados do cliente está aberto — `None` fechado.
    pub fn motivo_do_formulario(&self) -> Option<MotivoDoFormulario> {
        self.dados_do_cliente.as_ref().map(|f| f.motivo)
    }

    /// ✏️ "Editar": título, e-mail e WhatsApp da sessão aberta.
    ///
    /// *"Dentro da sessão precisa ser possível mudar o Título, email e o
    /// whatsapp."* — dono, 2026-09-13. O mesmo gesto da web: o botão ao lado do
    /// título, ou o clique no próprio título.
    pub fn editar_dados_do_cliente(&mut self, cx: &mut Context<Self>) {
        self.abrir_formulario_do_cliente(MotivoDoFormulario::Editar, None, cx);
    }

    fn abrir_formulario_do_cliente(
        &mut self,
        motivo: MotivoDoFormulario,
        aviso: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        if self.aberta.is_none() {
            return;
        }
        match self.dados_do_cliente.as_mut() {
            // Reabrir pelo mesmo motivo mantém o que já foi digitado.
            Some(formulario) if formulario.motivo == motivo => {
                if aviso.is_some() {
                    formulario.erro = aviso;
                }
            }
            _ => {
                self.dados_do_cliente = Some(FormularioDoCliente {
                    motivo,
                    campos: None,
                    erro: aviso,
                })
            }
        }
        cx.notify();
    }

    pub fn fechar_formulario_do_cliente(&mut self, cx: &mut Context<Self>) {
        self.dados_do_cliente = None;
        cx.notify();
    }

    fn dados_atuais(&self) -> Option<DadosDoCliente> {
        let galeria = &self.aberta.as_ref()?.galeria;
        Some(DadosDoCliente {
            titulo: galeria.titulo.clone(),
            email: galeria.email.clone(),
            whatsapp: galeria.whatsapp.clone(),
        })
    }

    /// 🧪 Preenche o formulário aberto (título, e-mail, WhatsApp) como quem
    /// digita — `None` deixa o campo como está.
    #[cfg(test)]
    pub(crate) fn digitar_dados_do_cliente(
        &mut self,
        titulo: Option<&str>,
        email: Option<&str>,
        whatsapp: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preparar_formulario_do_cliente(window, cx);
        let Some(campos) = self
            .dados_do_cliente
            .as_ref()
            .and_then(|f| f.campos.as_ref())
        else {
            return;
        };
        let pares = [
            (&campos.titulo, titulo),
            (&campos.email, email),
            (&campos.whatsapp, whatsapp),
        ];
        for (campo, valor) in pares {
            if let Some(valor) = valor {
                let valor = valor.to_string();
                campo.update(cx, |c, cx| c.set_value(valor, window, cx));
            }
        }
    }

    /// Cria os campos na primeira pintura com janela.
    ///
    /// ⚠️ **Não na abertura**: o `422` chega pela colheita, que não tem
    /// `Window`, e um `InputState` não nasce sem ela.
    fn preparar_formulario_do_cliente(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(motivo) = self
            .dados_do_cliente
            .as_ref()
            .filter(|f| f.campos.is_none())
            .map(|f| f.motivo)
        else {
            return;
        };
        let Some(atual) = self.dados_atuais() else {
            return;
        };

        let titulo = campo_preenchido(&atual.titulo, "Ensaio da Maria", window, cx);
        let email = campo_preenchido(
            atual.email.as_deref().unwrap_or(""),
            "cliente@exemplo.com",
            window,
            cx,
        );
        let whatsapp = campo_preenchido(
            atual.whatsapp.as_deref().unwrap_or(""),
            "(47) 99999-8888",
            window,
            cx,
        );
        // Enter grava, em qualquer um dos três.
        let enter = [&titulo, &email, &whatsapp].map(|estado| {
            cx.subscribe(
                estado,
                |tela: &mut Detalhe, _estado, evento: &InputEvent, cx| {
                    if matches!(evento, InputEvent::PressEnter { .. }) {
                        tela.gravar_dados_do_cliente(cx);
                    }
                },
            )
        });
        // Editar começa pelo título; o pedido do fim da sessão, pelo e-mail.
        let foco = if motivo == MotivoDoFormulario::Editar {
            &titulo
        } else {
            &email
        };
        // ⚠️ **Foco só com o `Root` na janela.** O `Input` do `gpui-component`
        // focado lê `Root::read(window, cx)` ao pintar, e aquilo é `unwrap()`:
        // numa janela sem `Root` (as de teste desta tela) o foco derrubava a
        // pintura. No app a primeira camada é sempre o `Root` (`main.rs`). O
        // foco é conforto — gravar e o Enter não dependem dele.
        if window.root::<gpui_component::Root>().flatten().is_some() {
            window.focus(&foco.read(cx).focus_handle(cx));
        }

        if let Some(formulario) = self.dados_do_cliente.as_mut() {
            formulario.campos = Some(CamposDoCliente {
                titulo,
                email,
                whatsapp,
                _enter: enter,
            });
        }
    }

    /// Grava o formulário: confere, manda **só o que mudou** e — quando o fim da
    /// sessão o abriu — segue com o gesto.
    pub fn gravar_dados_do_cliente(&mut self, cx: &mut Context<Self>) {
        if self.gravando_dados.is_some() {
            return;
        }
        let (Some(sessao), Some(galeria_id), Some(atual)) = (
            self.sessao.clone(),
            self.galeria_id.clone(),
            self.dados_atuais(),
        ) else {
            return;
        };
        let Some(formulario) = self.dados_do_cliente.as_ref() else {
            return;
        };
        let Some(campos) = formulario.campos.as_ref() else {
            return;
        };
        let motivo = formulario.motivo;
        let titulo = campos.titulo.read(cx).value().to_string();
        let email = campos.email.read(cx).value().to_string();
        let whatsapp = campos.whatsapp.read(cx).value().to_string();
        let modo = if motivo == MotivoDoFormulario::Editar {
            dados_do_cliente::Modo::Editar
        } else {
            dados_do_cliente::Modo::PedirEmail
        };

        let novo = match dados_do_cliente::conferir(modo, &titulo, &email, &whatsapp) {
            Ok(novo) => novo,
            Err(recusa) => {
                if let Some(formulario) = self.dados_do_cliente.as_mut() {
                    formulario.erro = Some(recusa.frase.into());
                }
                cx.notify();
                return;
            }
        };

        let mudancas = dados_do_cliente::mudancas(&atual, &novo);
        if mudancas.vazia() {
            // Nada mudou: não vai à rede — mas o gesto que abriu segue.
            self.dados_do_cliente = None;
            self.seguir_o_gesto(motivo, cx);
            cx.notify();
            return;
        }

        if let Some(formulario) = self.dados_do_cliente.as_mut() {
            formulario.erro = None;
        }
        self.gravando_dados = Some((motivo, novo));
        self.publicador.atualizar_galeria(
            sessao,
            galeria_id,
            MudancaDaGaleria {
                titulo: mudancas.titulo,
                email: mudancas.email,
                whatsapp: mudancas.whatsapp,
                ..Default::default()
            },
            self.recados.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// Depois de gravar: o gesto do fim da sessão que pediu o contato.
    fn seguir_o_gesto(&mut self, motivo: MotivoDoFormulario, cx: &mut Context<Self>) {
        match motivo {
            MotivoDoFormulario::Editar => {}
            MotivoDoFormulario::ContatoParaOLink => self.pedir_o_link(cx),
            // O formulário exige o e-mail neste motivo: o aviso tem para onde ir.
            MotivoDoFormulario::ContatoParaAvisar => self.avisar(cx),
        }
    }

    /// A receita padrão revelou esta foto: a miniatura velha sai do cache, e a
    /// próxima passada de `preparar_miniaturas` lê a nova.
    ///
    /// 🔑 **É a metade visível do C17** ("mudou a origem, os caches derivados
    /// saem e são refeitos"): quem gravou foi o serviço, em disco; aqui só se
    /// descarta o que está na memória desta tela.
    pub fn revelada_chegou(&mut self, foto_id: &str) {
        #[cfg(test)]
        self.reveladas_avisadas.push(foto_id.to_string());
        self.miniaturas.esquecer(foto_id);
        self.miniaturas.esquecer(&chave_do_site(foto_id));
        self.miniaturas
            .esquecer(&crate::revelacao::persistencia::chave_da_revelada(foto_id));
        self.miniaturas
            .esquecer(&crate::revelacao::persistencia::chave_da_revelada(
                &chave_do_site(foto_id),
            ));
        // A próxima `chave_da_foto` pergunta ao disco de novo — e agora acha.
        self.com_revelada.remove(foto_id);
    }

    /// **O site passou a ter a revelada desta foto** — e a miniatura que a
    /// grade desenha veio de antes dela.
    ///
    /// 🚨 **Esquecer não basta: tem de pedir de novo** (dono, 18/set/2026:
    /// *"não travou, mas não fez a atualização das miniaturas"*). Enquanto a
    /// releitura era `entrar`, o cache inteiro era esvaziado e `pedidas`
    /// zerava, então todas as miniaturas voltavam a ser baixadas — a
    /// atualização aparecia, ao preço da tela piscando em branco. Agora que a
    /// releitura mantém a grade de pé, `pedir_miniaturas` não pediria esta:
    /// ela já está em `pedidas`, e a célula continuaria mostrando o "antes".
    ///
    /// 🔑 **Só esta foto.** O envio muda uma por vez, e re-baixar a galeria
    /// inteira a cada resposta é exatamente o custo do qual se saiu.
    pub fn revelada_subiu(&mut self, foto_id: &str, cx: &mut Context<Self>) {
        self.revelada_chegou(foto_id);
        let (Some(sessao), true) = (self.sessao.clone(), self.tem_para_baixar(foto_id)) else {
            return;
        };
        // ⚠️ **O pedido é direto, e não por `pedir_miniaturas`**: aquela varre a
        // galeria inteira atrás do que falta, e durante um lote de duzentas
        // seriam duzentas varreduras de trezentas fotos — o mesmo tipo de custo
        // que tirou a grade do ar. Aqui se sabe qual foto mudou.
        self.pedidas.insert(foto_id.to_string());
        self.baixando += 1;
        self.publicador
            .miniatura(sessao, foto_id.to_string(), self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Esta foto está na galeria aberta e tem miniatura para baixar? A apagada
    /// aparece desbotada na grade, sem arquivo nenhum — pedi-la seria um 404
    /// por foto.
    fn tem_para_baixar(&self, foto_id: &str) -> bool {
        self.aberta
            .as_ref()
            .and_then(|a| a.fotos.iter().find(|f| f.id == foto_id))
            .is_some_and(|f| !f.apagada)
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
            // ⚠️ **Desliga também na lista vazia.** Fechar a janela sem escolher
            // é um gesto legítimo, e é o único jeito de o laço voltar a poder
            // parar depois dele.
            self.escolhendo = false;
            if !caminhos.is_empty() {
                self.enviar_arquivos(caminhos, cx);
            }
        }

        // ── O andamento da importação ────────────────────────────────────
        //
        // 🚨 **Canal próprio, contador próprio.** O que chega aqui é o lote do
        // catálogo local, e só. Enquanto isto dividia canal com o resto, uma
        // classificação feita durante o lote adiantava a barra em uma foto — e
        // 500 classificações a levavam ao fim com metade das fotos por gravar.
        while let Ok(andamento) = self.andamentos.1.try_recv() {
            mudou = true;
            match andamento {
                // O importador conta de novo o que já sabemos: um lote pode ser
                // menor que a lista (duplicata que ele descarta antes).
                Andamento::Comecou { total } => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.total = total;
                    }
                }
                Andamento::Feito { .. } => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.feitas += 1;
                    }
                }
                // ⚠️ **A pulada conta como pronta.** Ela é a duplicata que já
                // está no catálogo: não vai responder de novo, e fora da conta
                // prenderia a barra a um passo do fim para sempre.
                Andamento::Pulado { .. } => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.falhas += 1;
                    }
                }
                // ⚠️ **A falha de um arquivo não derruba o lote**, e nem para a
                // barra: o operador mandou 500, e "uma não entrou" é um recado —
                // as outras 499 continuam.
                Andamento::Falhou { caminho, erro } => {
                    if let Some(lote) = self.importacao.as_mut() {
                        lote.falhas += 1;
                    }
                    self.erro = Some(format!("{caminho}: {erro}").into());
                }
                Andamento::Terminou {
                    sucesso,
                    falhas,
                    pulados,
                } => {
                    self.importacao = Some(Importacao {
                        total: sucesso + falhas + pulados,
                        feitas: sucesso,
                        falhas: falhas + pulados,
                    });
                }
            }
            if self.importacao.is_some_and(|l| l.terminou()) {
                // 🔑 **Quem relê o catálogo é a raiz** — ela é que tem a porta
                // do acervo. Sem esta linha as fotos ficariam gravadas e
                // invisíveis, que é o mesmo desfecho de não ter importado.
                cx.emit(Pedido::CatalogoMudou);
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
                    self.do_site = aberta.fotos.iter().map(para_o_core).collect();
                    self.recompor_acervo();
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
                        let chave = chave_do_site(&foto_id);
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
                Recado::FaltaEmail { gesto, frase } => {
                    // 🔚 A tela achava que havia contato e o site diz que não
                    // (tela velha, ou outra máquina apagou): o mesmo pedido.
                    let motivo = match gesto {
                        GestoDoFim::Link => {
                            self.pedindo_link = false;
                            MotivoDoFormulario::ContatoParaOLink
                        }
                        GestoDoFim::Avisar => {
                            self.avisando = false;
                            MotivoDoFormulario::ContatoParaAvisar
                        }
                    };
                    self.abrir_formulario_do_cliente(motivo, Some(frase.into()), cx);
                }
                Recado::Produtos(lista) => self.produtos = lista,
                Recado::Estudios(lista) => self.estudios = lista,
                Recado::GaleriaAtualizada => {
                    if let Some((motivo, novo)) = self.gravando_dados.take() {
                        if let Some(aberta) = self.aberta.as_mut() {
                            aberta.galeria.titulo = novo.titulo;
                            aberta.galeria.email = novo.email;
                            aberta.galeria.whatsapp = novo.whatsapp;
                        }
                        self.dados_do_cliente = None;
                        // 🔑 O recado não é a verdade: relê. O site normaliza
                        // (e-mail em minúsculas, WhatsApp só dígitos), e é o que
                        // ele gravou que o cabeçalho deve mostrar.
                        if let (Some(sessao), Some(id)) =
                            (self.sessao.clone(), self.galeria_id.clone())
                        {
                            self.publicador
                                .abrir_galeria(sessao, id, self.recados.0.clone());
                            self.carregando = true;
                        }
                        self.seguir_o_gesto(motivo, cx);
                    }
                }
                Recado::GaleriaNaoAtualizada(frase) => {
                    self.gravando_dados = None;
                    match self.dados_do_cliente.as_mut() {
                        Some(formulario) => formulario.erro = Some(frase.into()),
                        None => self.erro = Some(frase.into()),
                    }
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
            || self.escolhendo
            || self.mudando > 0
            || self.importando()
            || self.baixando > 0
            || self.avisando
            || self.pedindo_link
            || self.gravando_dados.is_some();
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
fn chave_do_site(foto_id: &str) -> String {
    format!(
        "{}{foto_id}",
        crate::revelacao::persistencia::PREFIXO_DO_SITE
    )
}

impl Detalhe {
    /// A chave desta foto no cache de previews.
    ///
    /// 🚨 **Depende de onde ela mora, e não há como adivinhar pelo id.** A do
    /// site foi baixada da API e gravada sob `site:<id>`; a local foi gravada
    /// pelo importador sob o id do catálogo, cru. Uma chave só para as duas
    /// deixa metade da grade preta — foi o que aconteceu no dia em que a
    /// importação passou a entrar na grade (8/set/2026): 21 fotos com nome,
    /// estado e faixa, e nenhuma imagem.
    fn chave_da_foto(&self, foto_id: &str) -> String {
        let base = if self.ids_locais.contains(foto_id) {
            foto_id.to_string()
        } else {
            chave_do_site(foto_id)
        };
        // 🚨 **A revelada local vem antes do bruto e antes do site.**
        //
        // Na local é a promessa da etapa 2 do assistente: a predefinição e o
        // corte que o operador viu lá (dono, 17/set/2026 — a sessão abria
        // mostrando os brutos).
        //
        // 🚨 **Na foto do site é a mesma prévia local da web**
        // (`usar-previas-reveladas.ts`): a grade desenha o que o servidor tem,
        // e o servidor só muda quando alguém salva na galeria — enquanto isso a
        // Revelação já abre com a receita do banco local. O dono viu as duas
        // caras da mesma foto em 18/set/2026: *"na galeria estava com um efeito,
        // dei dois cliques e a revelação estava com outro"*. A prévia local é o
        // que faz as duas dizerem a mesma coisa antes de subir.
        //
        // Quem pergunta ao disco é `resolver_revelada`, uma vez por foto, em
        // `preparar_miniaturas`. Aqui só se lê o que ela respondeu: desenhar não
        // é hora de tocar em disco.
        if self.com_revelada.get(foto_id) == Some(&true) {
            return crate::revelacao::persistencia::chave_da_revelada(&base);
        }
        base
    }

    /// Esta foto local já tem a revelada da receita padrão? — pergunta ao disco
    /// **uma vez** e guarda a resposta (ver `com_revelada`).
    fn resolver_revelada(&mut self, foto_id: &str) {
        if self.com_revelada.contains_key(foto_id) {
            return;
        }
        let base = if self.ids_locais.contains(foto_id) {
            foto_id.to_string()
        } else {
            chave_do_site(foto_id)
        };
        let revelada = crate::revelacao::persistencia::chave_da_revelada(&base);
        let tem = self.previews.tem(&revelada, PreviewType::Thumbnail)
            || self.previews.tem(&revelada, PreviewType::Large);
        self.com_revelada.insert(foto_id.to_string(), tem);
    }

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
    ///
    /// # Só o que está à vista, e uma tela de cada lado
    ///
    /// 🚨 Até 17/set/2026 isto carregava o recorte **inteiro** e o cache crescia
    /// até ele. Agora: as linhas à vista da grade e o pedaço à vista da tira
    /// carregam sempre; a tela de cima, a de baixo e as pontas da tira carregam
    /// dentro de [`ORCAMENTO_DA_MARGEM`]. O cache tem o tamanho dessas faixas —
    /// não cresce com a sessão.
    ///
    /// Devolve `true` quando a margem ficou para o próximo quadro.
    fn preparar_miniaturas(&mut self) -> bool {
        let total = self.acervo.total_visivel();
        if total == 0 {
            return false;
        }
        let (grade_vista, grade_margem) = self.faixas_da_grade(total);
        let (tira_vista, tira_margem) = self.faixas_da_tira(total);
        let capacidade = (grade_margem.len() + tira_margem.len()).max(MINIATURAS_GUARDADAS);
        self.miniaturas
            .ajustar_capacidade(NonZeroUsize::new(capacidade).expect("o piso não é zero"));

        let inicio = std::time::Instant::now();
        let mut faltou = false;
        // A ordem é a da urgência: o que está à vista primeiro.
        let urgentes = grade_vista.clone().chain(tira_vista.clone());
        let margem = grade_margem
            .filter(|p| !grade_vista.contains(p))
            .chain(tira_margem.filter(|p| !tira_vista.contains(p)));
        for (posicao, urgente) in urgentes
            .map(|p| (p, true))
            .chain(margem.map(|p| (p, false)))
        {
            let Some(foto) = self.acervo.visivel(posicao) else {
                continue;
            };
            let foto_id = foto.id.clone();
            self.resolver_revelada(&foto_id);
            let chave = self.chave_da_foto(&foto_id);
            if self.miniaturas.espiar(&chave).is_some() {
                // `tocar`: o que está perto da vista não é o que o LRU descarta.
                self.miniaturas.tocar(&chave);
                continue;
            }
            if !urgente && inicio.elapsed() > ORCAMENTO_DA_MARGEM {
                faltou = true;
                continue;
            }
            self.carregar_miniatura(&chave);
        }
        faltou
    }

    /// Lê a miniatura do disco para a memória — gerando-a, se faltar.
    fn carregar_miniatura(&mut self, chave: &str) {
        // Sem miniatura gravada: reduz o preview grande uma vez e a grava.
        // Da segunda abertura em diante o caminho é só o `get_thumbnail`.
        if self.previews.get_thumbnail(chave).is_none() {
            if let Some(grande) = self.previews.get_preview(chave) {
                let pequena = reduzir(&grande, LADO_DA_MINIATURA);
                let _ = self.previews.save_thumbnail(chave, &pequena);
            }
        }
        self.miniaturas.obter(&self.previews, chave);
    }

    /// As linhas da grade à vista, e as com uma tela de margem de cada lado.
    ///
    /// Pela rolagem e pela altura da grade no último quadro; antes dele, pela
    /// altura da janela.
    fn linhas_da_vista(&self) -> (Range<usize>, Range<usize>) {
        let linhas = self.linhas_da_grade;
        let passo = self.passo_da_grade();
        let topo = -f32::from(self.rolagem_da_grade.offset().y);
        let vista = match f32::from(self.rolagem_da_grade.bounds().size.height) {
            v if v > 0. => v,
            _ => self.janela_no_quadro.1,
        };
        let primeira = ((topo / passo).floor().max(0.) as usize).min(linhas);
        let ultima = (((topo + vista) / passo).ceil().max(0.) as usize).clamp(primeira, linhas);
        let tela = (vista / passo).ceil().max(1.) as usize;
        (
            primeira..ultima,
            primeira.saturating_sub(tela)..(ultima + tela).min(linhas),
        )
    }

    /// As posições da grade à vista e as com uma tela de margem de cada lado.
    fn faixas_da_grade(&self, total: usize) -> (Range<usize>, Range<usize>) {
        let colunas = self.colunas_da_grade.max(1);
        let posicoes = |linhas: Range<usize>| {
            (linhas.start * colunas).min(total)..(linhas.end * colunas).min(total)
        };
        let (vista, margem) = self.linhas_da_vista();
        (posicoes(vista), posicoes(margem))
    }

    /// As posições da tira à vista e as que viram elemento (`faixa_desenhada`).
    fn faixas_da_tira(&self, total: usize) -> (Range<usize>, Range<usize>) {
        let passo = self.largura_na_tira() + VAO_DA_TIRA;
        let deslocamento = -f32::from(self.rolagem_da_tira.offset().x);
        let vista = match f32::from(self.rolagem_da_tira.bounds().size.width) {
            v if v > 0. => v,
            _ => self.janela_no_quadro.0,
        };
        let de = (((deslocamento - RECUO_DA_TIRA) / passo).floor().max(0.) as usize).min(total);
        let ate = ((((deslocamento + vista) / passo).ceil().max(0.) as usize) + 1).clamp(de, total);
        let (m_de, m_ate) = faixa_desenhada(total, passo, deslocamento, vista);
        (de..ate, m_de..m_ate)
    }

    /// Quantas miniaturas estão na memória agora — o teto é o do estresse.
    #[cfg(test)]
    pub(crate) fn miniaturas_na_memoria(&self) -> usize {
        self.miniaturas.quantas_na_memoria()
    }

    /// Anota a janela deste quadro e decide colunas, linhas e o pedaço à vista.
    fn medir_o_quadro(&mut self, window: &Window) {
        // A largura que o quadro anterior deu à grade, com a janela e o painel
        // **daquele** quadro — só se ele desenhou a grade.
        let largura = f32::from(self.rolagem_da_grade.bounds().size.width);
        if self.grade_no_quadro && largura > 0. {
            self.medida_da_grade = Some(MedidaDaGrade {
                largura,
                janela: self.janela_no_quadro.0,
                painel: self.painel_no_quadro,
            });
        }
        let mut texto = window.text_style();
        texto.font_size = gpui::rems(0.75).into();
        self.linha_de_texto = f32::from(texto.line_height_in_pixels(window.rem_size()));

        let janela = window.viewport_size();
        self.janela_no_quadro = (f32::from(janela.width), f32::from(janela.height));
        self.painel_no_quadro = self.em_foco().is_some();
        self.colunas_da_grade = self.colunas_visiveis(window);
        let total = self.acervo.total_visivel();
        self.grade_no_quadro = total > 0;
        self.linhas_da_grade = linhas_necessarias(total, self.colunas_da_grade);
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 🚨 **Antes de montar qualquer célula.** É o que tira o decode de dentro
        // do quadro; `celula` e `tira` daqui para baixo só leem da memória.
        self.medir_o_quadro(window);
        self.seguir_o_foco();
        let total = self.acervo.total_visivel();
        let tira = self.faixas_da_tira(total).1;
        self.tira_desenhada = (tira.start, tira.end);
        let grade = self.linhas_da_vista().0;
        self.grade_desenhada = (grade.start, grade.end);
        let a_seguir = self.grade_a_seguir.is_some() || self.tira_a_seguir.is_some();
        let faltou = self.preparar_miniaturas();
        if faltou || a_seguir {
            // A margem ficou pela metade, ou a rolagem ainda não tinha medida.
            cx.on_next_frame(window, |_tela, _window, cx| cx.notify());
        }
        self.preparar_formulario_do_cliente(window, cx);
        self.preparar_painel(window, cx);
        self.preparar_seletores(window, cx);

        // 🎨 **As faixas do site**: o cabeçalho de 48 px, a barra da importação
        // e a dos recortes, cada uma com o traço de baixo, e a grade encostada
        // nas bordas (`-m-4` na galeria do site). A galeria não tem o cabeçalho
        // do painel: o botão do menu mora na faixa de cima.
        div()
            .flex()
            .flex_col()
            // 🚨 **Ela é a tela inteira**, e a grade dentro dela é que recebe o
            // `flex_1`.
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.cabecalho(window, cx))
            .children(
                self.formulario_do_cliente(cx)
                    .map(|f| div().p(px(12.)).child(f)),
            )
            .children(self.detalhes(cx))
            .children(self.atendimento(cx))
            .children(self.dialogo_de_apagar(cx))
            .children(self.dialogo_de_tirar_do_acervo(cx))
            .child(self.envio(cx))
            .child(self.barra_da_grade(cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(
                    div()
                        .px(px(12.))
                        .pt(px(8.))
                        .child(crate::estilo::aviso(erro, true, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(12.))
                    .child(self.corpo(cx)),
            )
            .child(self.tira(cx))
            // 🪟 **As duas camadas do site, por cima da tela**: os "detalhes"
            // ancorados no botão que os abriu (o `Popover` de lá) e o
            // atendimento como gaveta que entra pela direita (o `Drawer`).
            // Elas ficam **por último** para nascerem acima da grade, e são
            // desenhadas na própria tela — o `deferred` do GPUI não aceita
            // outro `deferred` dentro (ver `caixa/dialogos.rs`).
            .children(self.detalhes(cx))
            .children(self.atendimento(cx))
    }
}

impl Detalhe {
    /// O cabeçalho da sessão: quem é o cliente, o que ela tem, e as saídas —
    /// numa faixa de 48 px, como o `header` de `tela-da-galeria.tsx`.
    ///
    /// 🔑 **As contagens aqui são por estado, cruas** — `2 levadas · 2 à venda ·
    /// 0 compradas`. As fichas da barra contam outra coisa: recorte por situação
    /// **exige classificação**. Os dois números estão certos, e respondem
    /// perguntas diferentes.
    fn cabecalho(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::estilo;
        use crate::recursos::Icone;
        use gpui_component::Icon;

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
        // 🔚 A sessão pode nascer sem contato (dono, 2026-09-13); o selo avisa
        // sem travar nada — quem pede é o link e o aviso.
        let sem_email = self.aberta.is_some() && !self.tem_email();
        let tema = cx.theme();
        let (borda, apagado, texto, campo) = (
            tema.border,
            tema.muted_foreground,
            tema.foreground,
            tema.input,
        );
        let sem_galeria = self.aberta.is_none();

        let selo = |cores: (gpui::Hsla, gpui::Hsla, gpui::Hsla), rotulo: &'static str| {
            let (fundo, _, frente) = cores;
            div()
                .flex_none()
                .px(px(6.))
                .rounded_full()
                .bg(fundo)
                .text_color(frente)
                .text_size(px(10.))
                .child(rotulo)
        };

        // 🚨 **No Linux esta barra é a barra de título da galeria.** A tela da
        // sessão não tem o cabeçalho de 56 px do app (`Tela::tem_cabecalho`), e
        // sem isto a janela não se move nem maximiza enquanto ela estiver
        // aberta — que é onde o operador passa a maior parte do tempo. Ver
        // `crate::janela`.
        crate::janela::como_barra_de_titulo(div(), "barra-da-galeria", window, cx)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(48.))
            .px(px(12.))
            .overflow_hidden()
            .border_b_1()
            .border_color(borda)
            .child(
                estilo::botao_do_menu("galeria-menu", cx)
                    .on_click(cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::AlternarMenu))),
            )
            .child(
                div()
                    .id("galeria-voltar")
                    .flex_none()
                    .cursor_pointer()
                    .text_color(apagado)
                    .hover(move |s| s.text_color(texto))
                    .child(Icon::new(Icone::ChevronLeft).size(px(20.)))
                    .on_click(cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::Voltar))),
            )
            .child(
                // ✏️ Clicar no título edita — o mesmo gesto da web.
                div()
                    .id("sessao-titulo")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(Icon::new(Icone::Pencil).size(px(14.)).text_color(apagado))
                    .child(
                        div()
                            .max_w(px(320.))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .truncate()
                            .child(titulo),
                    )
                    .on_click(
                        cx.listener(|tela, _ev, _window, cx| tela.editar_dados_do_cliente(cx)),
                    ),
            )
            .child(
                div()
                    .min_w(px(0.))
                    .text_xs()
                    .truncate()
                    .text_color(apagado)
                    .child(contato),
            )
            .when(sem_email, |cabecalho| {
                cabecalho.child(selo(cores::selo_ambar(), "sem e-mail"))
            })
            .when(ja_abriu, |cabecalho| {
                // 🔑 O sinal que o fotógrafo espera para cobrar.
                cabecalho.child(selo(cores::selo_esmeralda(), "já abriu"))
            })
            .child(div().flex_1())
            .child(
                // Os números da galeria, como o "detalhes" do site.
                estilo::botao_contorno("sessao-contagem", cx)
                    .text_xs()
                    .text_color(apagado)
                    .child(Icon::new(Icone::Info).size(px(14.)))
                    .child(format!(
                        "{levadas} levadas · {a_venda} à venda · {compradas} compradas"
                    ))
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.detalhes_abertos = !tela.detalhes_abertos;
                        cx.notify();
                    })),
            )
            // 📋 **Atendimento** — o que o assistente coletou nas sete etapas.
            // No site é uma gaveta ao lado do "Dados do cliente", e é onde se
            // corrige um voucher errado sem recriar a sessão.
            .child(estilo::desligado(
                estilo::botao_fantasma("sessao-atendimento", cx)
                    .border_1()
                    .border_color(campo)
                    .child(Icon::new(Icone::ClipboardList).size(px(16.)))
                    .child(SharedString::from(match self.quantas_associacoes() {
                        0 => "Atendimento".to_string(),
                        n => format!("Atendimento · {n}"),
                    }))
                    .when(!sem_galeria, |b| {
                        b.on_click(
                            cx.listener(|tela, _ev, _window, cx| tela.alternar_atendimento(cx)),
                        )
                    }),
                sem_galeria,
            ))
            // 🏠 **O estúdio da sessão**, como no cabeçalho do site.
            .children(self.escolha_do_estudio.as_ref().map(|escolha| {
                div()
                    .w(px(200.))
                    .child(Select::new(escolha).xsmall().placeholder("Estúdio…"))
            }))
            .child(estilo::desligado(
                estilo::botao_fantasma("sessao-editar-cliente", cx)
                    .border_1()
                    .border_color(campo)
                    .child(Icon::new(Icone::ClipboardList).size(px(16.)))
                    .child("Dados do cliente")
                    .when(!sem_galeria, |b| {
                        b.on_click(
                            cx.listener(|tela, _ev, _window, cx| tela.editar_dados_do_cliente(cx)),
                        )
                    }),
                sem_galeria,
            ))
            .child(estilo::desligado(
                estilo::botao_contorno("sessao-link", cx)
                    .child(Icon::new(Icone::Link2).size(px(16.)))
                    .child(if self.link.is_some() {
                        "Copiar de novo"
                    } else {
                        "Copiar link"
                    })
                    .when(!sem_galeria, |b| {
                        b.on_click(cx.listener(|tela, _ev, _window, cx| tela.pedir_o_link(cx)))
                    }),
                sem_galeria,
            ))
            // 📧 O aviso sai por e-mail; sem contato nenhum ele aparece e pede o
            // contato — o mesmo gesto da web. Só com WhatsApp, sem botão.
            .when(email.is_some() || sem_email, |cabecalho| {
                cabecalho.child(estilo::desligado(
                    estilo::botao_contorno("sessao-avisar", cx)
                        .child(Icon::new(Icone::Send).size(px(16.)))
                        .child(if self.avisando {
                            "Avisando…"
                        } else {
                            "Avisar cliente"
                        })
                        .when(!self.avisando, |b| {
                            b.on_click(cx.listener(|tela, _ev, _window, cx| tela.avisar(cx)))
                        }),
                    self.avisando,
                ))
            })
    }

    /// ✏️ O formulário dos dados do cliente — embutido sob o cabeçalho, como o
    /// "Nova sessão fotográfica" da lista (`tela.rs`).
    ///
    /// ⚠️ **Embutido, e não `open_dialog`**: o diálogo do `gpui-component` exige
    /// o `Root` na janela, e as janelas de teste desta tela não o têm — o
    /// pedido de contato ficaria sem teste justamente no caminho do `422`.
    fn formulario_do_cliente(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let formulario = self.dados_do_cliente.as_ref()?;
        let campos = formulario.campos.as_ref()?;
        let gravando = self.gravando_dados.is_some();
        let (titulo, dica, rotulo_de_gravar) = match formulario.motivo {
            MotivoDoFormulario::Editar => (
                "Dados do cliente",
                "E-mail e WhatsApp podem ficar vazios agora: o link e o aviso pedem o e-mail no fim da sessão.",
                "Gravar",
            ),
            MotivoDoFormulario::ContatoParaOLink => (
                "Falta o e-mail do cliente para gerar o link",
                "O link que entra sem senha nasce do e-mail do cliente. O WhatsApp é opcional.",
                "Gravar e gerar o link",
            ),
            MotivoDoFormulario::ContatoParaAvisar => (
                "Falta o e-mail do cliente para avisar",
                "O aviso de fotos prontas vai por e-mail. O WhatsApp é opcional.",
                "Gravar e avisar",
            ),
        };
        let apagado = cx.theme().muted_foreground;
        let perigo = cx.theme().danger;
        let campo = |rotulo: &'static str, estado: &Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .child(div().text_xs().text_color(apagado).child(rotulo))
                .child(Input::new(estado).xsmall())
        };
        let linha = div()
            .flex()
            .gap(px(8.))
            .child(campo("título", &campos.titulo))
            .child(campo("e-mail do cliente", &campos.email))
            .child(campo("WhatsApp", &campos.whatsapp));

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .child(div().text_xs().child(titulo))
                .child(linha)
                .child(div().text_xs().text_color(apagado).child(dica))
                .when_some(formulario.erro.clone(), |bloco, erro| {
                    bloco.child(div().text_xs().text_color(perigo).child(erro))
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(
                            Button::new("sessao-cliente-cancelar")
                                .label("Cancelar")
                                .xsmall()
                                .disabled(gravando)
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.fechar_formulario_do_cliente(cx)
                                })),
                        )
                        .child(
                            Button::new("sessao-cliente-gravar")
                                .label(if gravando {
                                    "Gravando…"
                                } else {
                                    rotulo_de_gravar
                                })
                                .xsmall()
                                .primary()
                                .disabled(gravando)
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.gravar_dados_do_cliente(cx)
                                })),
                        ),
                ),
        )
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
        use crate::estilo;
        use crate::recursos::Icone;
        use gpui_component::Icon;

        let lote = self.importacao;
        let ocupado = self.importando();
        let sem_sessao = self.aberta.is_none();
        let leva = self.leva;
        let visiveis = self.acervo.total_visivel();
        let tema = cx.theme();
        let (borda, apagado, primaria, acento) = (
            tema.border,
            tema.muted_foreground,
            tema.primary,
            tema.accent,
        );

        // "Entram como": as fichas do site, a escolhida em âmbar.
        let ficha = |rotulo: &'static str,
                     valor: Option<EstadoNoBalcao>,
                     cx: &mut Context<Self>| {
            pilula(
                SharedString::from(format!("detalhe-leva-{rotulo}")),
                rotulo,
                None,
                leva == valor,
                cx,
            )
            .when(!ocupado, |p| {
                p.on_click(cx.listener(move |tela, _ev, _window, cx| tela.escolher_leva(valor, cx)))
            })
        };

        let andamento = match lote {
            // 🔑 **O número inteiro, e não só o que já foi.** Quem mandou 500
            // quer saber que são 500.
            Some(l) if !l.terminou() => {
                let falhas = if l.falhas > 0 {
                    format!(" · {} falharam", l.falhas)
                } else {
                    String::new()
                };
                format!("importando… {} de {}{falhas}", l.prontas(), l.total)
            }
            // ⚠️ **"importadas", e não "subiram"** — nada sobe no passo 1.
            Some(l) if l.falhas > 0 => {
                format!("{} importadas · {} não entraram", l.feitas, l.falhas)
            }
            Some(l) => format!("{} importadas", l.feitas),
            None => match leva {
                None => "Arraste as fotos (ou a pasta) para cá. Sem marcação vai para o \
                         acervo à venda; a marcação de verdade nasce no balcão."
                    .to_string(),
                Some(EstadoNoBalcao::Disponivel) => {
                    "À venda: o cliente vê com marca d'água e pode comprar pela galeria."
                        .to_string()
                }
                Some(EstadoNoBalcao::LevadaNoBalcao) => {
                    "Levadas: o cliente já pagou na hora e baixa o original.".to_string()
                }
            },
        };

        div()
            .id("detalhe-envio")
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            // O destaque de "solte aqui": a faixa acende enquanto o arrasto passa.
            .border_color(if self.arrastando { primaria } else { borda })
            .when(self.arrastando, |d| d.bg(acento))
            .on_drag_move(
                cx.listener(|tela, _ev: &gpui::DragMoveEvent<()>, _window, cx| {
                    tela.destacar(true, cx)
                }),
            )
            .on_drop(
                cx.listener(|tela, arrastados: &gpui::ExternalPaths, _window, cx| {
                    tela.destacar(false, cx);
                    // 🔑 Uma pasta solta vira o conteúdo dela.
                    let fotos = super::arquivos::so_as_fotos(arrastados.paths());
                    tela.enviar_arquivos(fotos, cx);
                }),
            )
            // 🔑 **"Importar fotos"**, o botão do site: abre a janela do
            // sistema, e as fotos entram no catálogo desta máquina, na leva
            // escolhida. `debug_selector` é o que deixa o teste clicar onde o
            // dedo clica.
            .child(estilo::desligado(
                estilo::botao_primario("detalhe-importar", cx)
                    .debug_selector(|| "detalhe-importar".into())
                    .child(Icon::new(Icone::Upload).size(px(16.)))
                    .child("Importar fotos")
                    .when(!(ocupado || sem_sessao), |b| {
                        b.on_click(cx.listener(|tela, _ev, _window, cx| tela.importar(cx)))
                    }),
                ocupado || sem_sessao,
            ))
            // ⚠️ **Exportar mora ao lado de Importar**: por aqui as fotos
            // entram, por ali saem. Sem seleção exporta o que a grade mostra.
            .child(estilo::desligado(
                estilo::botao_contorno("detalhe-exportar", cx)
                    .debug_selector(|| "detalhe-exportar".into())
                    .child(Icon::new(Icone::FolderInput).size(px(16.)))
                    .child("Exportar")
                    .when(!(sem_sessao || visiveis == 0), |b| {
                        b.on_click(cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::Exportar)))
                    }),
                sem_sessao || visiveis == 0,
            ))
            .child(div().text_sm().text_color(apagado).child("Entram como"))
            .child(ficha("sem marcação", None, cx))
            .child(ficha("à venda", Some(EstadoNoBalcao::Disponivel), cx))
            .child(ficha("levadas", Some(EstadoNoBalcao::LevadaNoBalcao), cx))
            // 🧾 **A faixa da próxima leva** — a segunda escolha antes dos
            // arquivos, como no site: a sessão mista sobe em levas, e sem ela
            // toda foto nascia na faixa da galeria.
            .children(self.escolha_da_leva.as_ref().map(|escolha| {
                div()
                    .w(px(232.))
                    .child(Select::new(escolha).xsmall().placeholder("Faixa…"))
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(200.))
                    .gap(px(4.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .truncate()
                            .child(andamento),
                    )
                    // 🔑 **A barra só existe enquanto o lote corre.**
                    .when_some(lote.filter(|l| !l.terminou()), |linha, l| {
                        linha.child(Progress::new().value(l.porcento()).h(px(4.)))
                    }),
            )
    }

    /// A barra da grade: recortes · zoom · Revelar · Tela do cliente · seleção.
    fn barra_da_grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::estilo;
        use crate::recursos::Icone;
        use gpui_component::Icon;

        let contagens = self.contagens();
        let ativo = self.filtro();
        let visiveis = self.acervo.total_visivel();
        let marcadas = self.quantas_marcadas();
        let todas_marcadas = visiveis > 0 && marcadas == visiveis;
        let tema = cx.theme();
        let (borda, apagado, acento) = (tema.border, tema.muted_foreground, tema.accent);
        let pode_revelar = self.tem_o_que_revelar();

        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(borda)
            .children(FILTROS.into_iter().filter_map(|(rotulo, filtro)| {
                let quantas = contagens.de(filtro);
                // 🔑 O recorte vazio some — **menos** quando é o escolhido:
                // sumir o filtro ativo tiraria o caminho de volta.
                let mostrar = filtro == Filtro::Todas || quantas > 0 || ativo == filtro;
                mostrar.then(|| {
                    pilula(
                        SharedString::from(format!("sessao-filtro-{rotulo}")),
                        rotulo,
                        Some(quantas),
                        ativo == filtro,
                        cx,
                    )
                    .on_click(cx.listener(move |tela, _ev, _window, cx| tela.filtrar(filtro, cx)))
                })
            }))
            .child(div().flex_1())
            // O zoom, como no site: duas lupas e a faixa entre elas.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(apagado)
                    .child(Icon::new(Icone::ZoomOut).size(px(16.)))
                    .child(
                        div()
                            .w(px(112.))
                            .child(Slider::new(&self.zoom_slider).horizontal()),
                    )
                    .child(Icon::new(Icone::ZoomIn).size(px(16.))),
            )
            // 🔑 **Sem exigir foco**, como o botão da barra do site: entra no
            // modo e a tira faz o resto.
            .child(estilo::desligado(
                estilo::botao_fantasma("sessao-revelar", cx)
                    .bg(acento)
                    .child(Icon::new(Icone::SlidersHorizontal).size(px(16.)))
                    .child("Revelar")
                    .when(pode_revelar, |b| {
                        b.on_click(cx.listener(|tela, _ev, _window, cx| tela.revelar_todas(cx)))
                    }),
                !pode_revelar,
            ))
            .child(
                estilo::botao_contorno("sessao-tela-do-cliente", cx)
                    .when(self.cliente_aberta, |b| b.bg(acento))
                    .child(Icon::new(Icone::Monitor).size(px(16.)))
                    .child(if self.cliente_aberta {
                        "Fechar a tela do cliente"
                    } else {
                        "Tela do cliente"
                    })
                    .on_click(
                        cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::TelaDoCliente)),
                    ),
            )
            // 🖨️ O que se faz **com as marcadas**: a negociação do balcão (o
            // "Negociação…" do site) e a folha de impressão, que só o desktop tem.
            .when(marcadas > 0, |barra| {
                barra
                    .child(
                        estilo::botao_contorno("sessao-negociar", cx)
                            .child(Icon::new(Icone::ShoppingCart).size(px(16.)))
                            .child(format!("Negociação… ({marcadas})"))
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                cx.emit(Pedido::Negociar(tela.marcadas()))
                            })),
                    )
                    .child(
                        estilo::botao_contorno("sessao-imprimir", cx)
                            .child(Icon::new(Icone::Printer).size(px(16.)))
                            .child("Imprimir…")
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                cx.emit(Pedido::Imprimir(tela.marcadas()))
                            })),
                    )
            })
            .child(estilo::desligado(
                estilo::botao_fantasma("sessao-selecionar-visiveis", cx)
                    .text_color(apagado)
                    .child(
                        Icon::new(if todas_marcadas {
                            Icone::SquareCheck
                        } else {
                            Icone::Square
                        })
                        .size(px(16.)),
                    )
                    .child(format!(
                        "{} as {visiveis} visíveis",
                        if todas_marcadas {
                            "Desmarcar"
                        } else {
                            "Selecionar"
                        }
                    ))
                    .when(visiveis > 0, |b| {
                        b.on_click(cx.listener(|tela, _ev, _window, cx| tela.alternar_todas(cx)))
                    }),
                visiveis == 0,
            ))
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
                no_disco: self.ids_locais.contains(&f.id),
            })
            .collect();
        if fotos.is_empty() {
            return None;
        }
        let inicial = match comecar_em {
            Some(id) => fotos.iter().position(|f| f.id == id)?,
            // 🚨 O botão da barra abre **na primeira marcada**, depois na em
            // foco, e só então na primeira editável (`fotoParaAbrir` do site):
            // é dela que o "Sincronizar" copia.
            None => {
                let marcadas = self.marcadas();
                let foco = self.em_foco().map(|f| f.id.clone());
                marcadas
                    .iter()
                    .find_map(|id| fotos.iter().position(|f| &f.id == id))
                    .or_else(|| foco.and_then(|id| fotos.iter().position(|f| f.id == id)))
                    .or_else(|| self.fotos_a_revelar().position(acervo::Foto::editavel))
                    .unwrap_or(0)
            }
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
        //
        // 🚨 **Só as linhas à vista viram elemento.** As de cima e as de baixo
        // são dois espaçadores de largura cheia — cada um ocupa uma fileira do
        // `flex_wrap` —, com `k × passo − respiro` de altura: somado ao respiro
        // da fileira, a linha `k` cai **exatamente** onde caía com todas as
        // células montadas, e a rolagem tem a mesma altura.
        let colunas = self.colunas_da_grade.max(1);
        let total = self.acervo.total_visivel();
        let linhas = self.linhas_da_grade;
        let passo = self.passo_da_grade();
        let (de, ate) = self.grade_desenhada;
        let ate = ate.min(linhas);
        let de = de.min(ate);
        let espacador = |itens: usize| {
            div()
                .w_full()
                .h(px(itens as f32 * passo - VAO_DA_GRADE))
                .into_any_element()
        };
        let posicoes = (de * colunas).min(total)..(ate * colunas).min(total);
        let quantas = posicoes.len();
        let mut filhos: Vec<gpui::AnyElement> = Vec::with_capacity(quantas + 2);
        if de > 0 {
            filhos.push(espacador(de));
        }
        filhos.extend(posicoes.filter_map(|posicao| {
            self.acervo
                .visivel(posicao)
                .map(|foto| self.celula(posicao, foto, cx).into_any_element())
        }));
        if ate < linhas {
            filhos.push(espacador(linhas - ate));
        }

        // 🔑 **O quadro confere a conta.** As colunas e a linha de texto são
        // previstas antes do leiaute; se o `flex_wrap` discordar, o que ele
        // desenhou vence e o quadro seguinte sai com o número dele.
        let esta = cx.entity().downgrade();
        let chave = (self.janela_no_quadro.0, self.painel_no_quadro, self.zoom);
        let linha_usada = self.linha_medida.unwrap_or(self.linha_de_texto);
        let lado = self.zoom;
        let pular = usize::from(de > 0);
        // ⚠️ Com `track_scroll`, o GPUI guarda as caixas dos filhos na alça da
        // rolagem, e não na lista que o ouvinte recebe (que chega vazia).
        let rolagem = self.rolagem_da_grade.clone();
        let conferir =
            move |_: Vec<gpui::Bounds<gpui::Pixels>>, window: &mut Window, _cx: &mut App| {
                let celulas: Vec<_> = (pular..pular + quantas)
                    .map_while(|i| rolagem.bounds_for_item(i))
                    .collect();
                let Some(primeira) = celulas.first() else {
                    return;
                };
                let na_fileira = celulas
                    .iter()
                    .take_while(|c| c.origin.y == primeira.origin.y)
                    .count();
                let colunas_vistas = (na_fileira < celulas.len() || na_fileira > colunas)
                    .then_some(na_fileira)
                    .filter(|n| *n != colunas);
                let linha = ((f32::from(primeira.size.height) - lado * 0.72 - 4.0) / 2.0).round();
                let linha_vista = (linha > 0. && linha != linha_usada).then_some(linha);
                if colunas_vistas.is_none() && linha_vista.is_none() {
                    return;
                }
                let esta = esta.clone();
                window.on_next_frame(move |_window, cx| {
                    let _ = esta.update(cx, |tela, cx| {
                        if let Some(n) = colunas_vistas {
                            tela.colunas_vistas = Some((chave, n));
                        }
                        if linha_vista.is_some() {
                            tela.linha_medida = linha_vista;
                        }
                        cx.notify();
                    });
                });
            };

        div()
            .on_children_prepainted(conferir)
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
            .gap(px(VAO_DA_GRADE))
            .overflow_y_scroll()
            .children(filhos)
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
        let miniatura = match self.miniaturas.espiar(&self.chave_da_foto(&foto.id)) {
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
                    .overflow_hidden()
                    // 🎨 O cartão do site: canto de 10 px, a foto sobre o poço.
                    .bg(cores::poco())
                    .rounded(px(10.))
                    // 🔑 A marcação é **borda**, e não fundo: fundo colorido
                    // mudaria a cor que o olho usa para julgar a foto ao lado.
                    .border_2()
                    .border_color(if em_foco {
                        cx.theme().primary
                    } else if marcada {
                        cores::quente()
                    } else {
                        gpui::transparent_black()
                    })
                    .when_some(miniatura, |quadro, imagem| {
                        // 🚨 **`max_*`, e nunca `size_full` com `Contain`.** O
                        // `Img` do GPUI grava `style.aspect_ratio` com a
                        // proporção da foto em todo layout: com largura e
                        // altura em 100%, o taffy tira a altura da largura, o
                        // elemento fica maior que o quadro e o `overflow_hidden`
                        // daqui transforma o `Contain` em corte — a mesma
                        // armadilha que cortava a tela do cliente
                        // (`cliente::camada`, 17/set/2026).
                        quadro.child(img(imagem).max_w_full().max_h_full())
                    })
                    // O selo do estado, no canto — como na tela do site, e
                    // agora com a cor do que ele diz (`crate::selos`).
                    .child(
                        div()
                            .absolute()
                            .top(px(4.))
                            .left(px(4.))
                            // 🚨 A importada não é "à venda": ela nem chegou ao
                            // site. Ver `selos::selo_de_so_no_disco`.
                            .child(if self.ids_locais.contains(&foto.id) {
                                selos::selo_de_so_no_disco(cx).into_any_element()
                            } else {
                                selos::selo_do_estado(foto.estado, foto.apagada, cx)
                                    .into_any_element()
                            }),
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
    /// Monta os seletores da barra e do cabeçalho quando o catálogo chega.
    ///
    /// Como o painel da foto: eles pedem `window`, e `Detalhe::nova` não tem uma.
    /// Refazer quando a lista muda é de propósito — a primeira carga chega com o
    /// catálogo vazio.
    fn preparar_seletores(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.produtos.is_empty() && self.estudios.is_empty() {
            return;
        }
        if self.escolha_da_leva.is_none() && !self.produtos.is_empty() {
            let mut opcoes = vec![OpcaoDaFaixa {
                id: String::new(),
                titulo: SharedString::from(format!(
                    "Padrão da galeria ({})",
                    self.nome_da_faixa(&self.produto_padrao_da_galeria())
                )),
            }];
            opcoes.extend(self.produtos.iter().map(|p| OpcaoDaFaixa {
                id: p.id.clone(),
                titulo: SharedString::from(self.nome_da_faixa(&p.id)),
            }));
            let escolha =
                cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
            let atual = self.faixa.clone().unwrap_or_default();
            escolha.update(cx, |estado, cx| {
                estado.set_selected_value(&atual, window, cx);
            });
            self._escolhas_da_barra.push(cx.subscribe_in(
                &escolha,
                window,
                |tela, _, evento: &SelectEvent<SearchableVec<OpcaoDaFaixa>>, _window, cx| {
                    let SelectEvent::Confirm(valor) = evento;
                    let id = valor.clone().unwrap_or_default();
                    tela.escolher_faixa((!id.trim().is_empty()).then_some(id), cx);
                },
            ));
            self.escolha_da_leva = Some(escolha);
        }
        if self.escolha_do_estudio.is_none() && !self.estudios.is_empty() {
            let opcoes: Vec<OpcaoDaFaixa> = self
                .estudios
                .iter()
                .map(|e| OpcaoDaFaixa {
                    id: e.id.clone(),
                    titulo: SharedString::from(if e.cidade.trim().is_empty() {
                        e.nome.clone()
                    } else {
                        format!("{} — {}", e.nome, e.cidade)
                    }),
                })
                .collect();
            let escolha =
                cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
            let atual = self.estudio_da_galeria();
            escolha.update(cx, |estado, cx| {
                estado.set_selected_value(&atual, window, cx);
            });
            self._escolhas_da_barra.push(cx.subscribe_in(
                &escolha,
                window,
                |tela, _, evento: &SelectEvent<SearchableVec<OpcaoDaFaixa>>, _window, cx| {
                    let SelectEvent::Confirm(valor) = evento;
                    tela.mudar_estudio(valor.clone().unwrap_or_default(), cx);
                },
            ));
            self.escolha_do_estudio = Some(escolha);
        }
    }

    /// A raiz entrega as predefinições carregadas na abertura.
    pub fn definir_presets(&mut self, presets: Vec<domain::entities::Preset>) {
        self.presets_da_receita = presets;
    }

    /// Abre ou fecha os detalhes da sessão (números e prazos).
    pub fn alternar_detalhes(&mut self, cx: &mut Context<Self>) {
        self.detalhes_abertos = !self.detalhes_abertos;
        cx.notify();
    }

    /// Abre ou fecha a gaveta do atendimento.
    pub fn alternar_atendimento(&mut self, cx: &mut Context<Self>) {
        self.atendimento_aberto = !self.atendimento_aberto;
        cx.notify();
    }

    /// Quantas associações a sessão tem — o número do botão do cabeçalho.
    /// A mesma conta do site (`quantasAssociacoes`).
    fn quantas_associacoes(&self) -> usize {
        let Some(aberta) = self.aberta.as_ref() else {
            return 0;
        };
        let g = &aberta.galeria;
        [
            g.ensaio_id.is_some(),
            g.voucher_id.is_some(),
            g.pedido_id.is_some(),
            g.como_conheceu.is_some(),
            g.preset_padrao_id.is_some() || g.proporcao_padrao.is_some(),
        ]
        .into_iter()
        .filter(|tem| *tem)
        .count()
    }

    /// Troca a resposta de "como conheceu" da sessão aberta.
    ///
    /// 🚨 **Os três campos vão juntos.** Trocar a resposta sem limpar o parceiro
    /// manda `parceiro_id` com outra origem, e o site recusa (`400`); clicar na
    /// que já está marcada desmarca — "não perguntei" é diferente de qualquer
    /// resposta, como no `ToggleGroup` do site.
    fn escolher_como_conheceu(&mut self, valor: &str, cx: &mut Context<Self>) {
        let atual = self
            .aberta
            .as_ref()
            .and_then(|a| a.galeria.como_conheceu.clone());
        let novo = (atual.as_deref() != Some(valor)).then(|| valor.to_string());
        let vira_parceiro = novo.as_deref() == Some("parceiro");
        let vira_outro = novo.as_deref() == Some("outro");
        self.gravar_a_galeria(
            MudancaDaGaleria {
                como_conheceu: Some(novo),
                // O parceiro só sobrevive à resposta "parceiro"; o texto, à
                // "outro". A gaveta ainda não escolhe parceiro — quem o associa
                // é o assistente —, então trocar para "parceiro" mantém o que
                // houver.
                parceiro_id: (!vira_parceiro).then_some(None),
                como_conheceu_detalhe: (!vira_outro).then_some(None),
                ..Default::default()
            },
            cx,
        );
    }

    /// Troca o corte padrão da sessão. `""` é "sem corte".
    fn escolher_corte_padrao(&mut self, valor: &str, cx: &mut Context<Self>) {
        let novo = (!valor.trim().is_empty()).then(|| valor.to_string());
        let atual = self
            .aberta
            .as_ref()
            .and_then(|a| a.galeria.proporcao_padrao.clone());
        if atual == novo {
            return;
        }
        self.gravar_a_galeria(
            MudancaDaGaleria {
                proporcao_padrao: Some(novo),
                ..Default::default()
            },
            cx,
        );
    }

    /// Os números e os prazos da galeria — o "detalhes" do cabeçalho do site.
    ///
    /// 🔑 **Nada aqui muda foto nenhuma**: é conferência, e por isso fica atrás
    /// de um clique. Prazo ausente some da lista em vez de virar "—": a galeria
    /// sem vencimento de venda não tem essa data, e inventar um traço sugeriria
    /// que alguém esqueceu de preencher.
    fn detalhes(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.detalhes_abertos {
            return None;
        }
        let aberta = self.aberta.as_ref()?;
        let g = &aberta.galeria;
        let tema = cx.theme();
        let apagado = tema.muted_foreground;
        let (levadas, a_venda, compradas) = self.contagem();
        let apagadas = self.acervo.contagens().de(Filtro::Apagadas);

        let linha = |rotulo: &'static str, valor: String| {
            div()
                .flex()
                .gap(px(8.))
                .text_sm()
                .child(div().w(px(150.)).text_color(apagado).child(rotulo))
                .child(div().flex_1().truncate().child(SharedString::from(valor)))
        };
        let dia = |quando: Option<i64>| {
            quando
                .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
                .map(|d| {
                    d.with_timezone(&chrono::FixedOffset::west_opt(3 * 3600).expect("fuso"))
                        .format("%d/%m/%Y")
                        .to_string()
                })
        };
        let padrao = self.produto_padrao_da_galeria();

        Some(
            // O véu ocupa a tela e fecha ao clique, como o `Popover` do site
            // fecha ao clicar fora; o painel nasce abaixo do botão que o abriu,
            // alinhado à direita dele.
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .id("detalhes-veu")
                .on_click(cx.listener(|tela, _ev, _w, cx| {
                    tela.detalhes_abertos = false;
                    cx.notify();
                }))
                .child(
            div()
                .absolute()
                .top(px(52.))
                .right(px(12.))
                .w(px(384.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .rounded(tema.radius)
                .border_1()
                .border_color(tema.border)
                .bg(tema.background)
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("Detalhes da sessão"),
                        )
                        .child(
                            Button::new("detalhes-fechar")
                                .label("Fechar")
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|tela, _ev, _w, cx| {
                                    tela.detalhes_abertos = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(linha(
                    "Fotos",
                    match apagadas {
                        0 => format!("{levadas} levadas · {a_venda} à venda · {compradas} compradas"),
                        n => format!(
                            "{levadas} levadas · {a_venda} à venda · {compradas} compradas · {n} apagadas"
                        ),
                    },
                ))
                .child(linha("Preço padrão", self.nome_da_faixa(&padrao)))
                .children(
                    dia(aberta.vence_venda).map(|quando| linha("À venda até", quando)),
                )
                .children(
                    dia(aberta.vence_download).map(|quando| linha("Download até", quando)),
                )
                .children(dia(g.expira_em).map(|quando| linha("Galeria até", quando)))
                .child(linha(
                    "Criada",
                    {
                        // `2026-09-18` → `18/09/2026`: a data se lê como no
                        // resto da tela, e não como o banco a guarda.
                        let criada = g
                            .criada_em_iso
                            .split('-')
                            .collect::<Vec<_>>()
                            .as_slice()
                            .try_into()
                            .map(|[a, m, d]: [&str; 3]| format!("{d}/{m}/{a}"))
                            .unwrap_or_else(|_| g.criada_em_iso.clone());
                        match g.criada_por.as_deref().filter(|q| !q.trim().is_empty()) {
                            Some(quem) => format!("{criada} por {quem}"),
                            None => criada,
                        }
                    },
                )),
                ),
        )
    }

    /// A gaveta do atendimento: o que veio junto com o cliente.
    ///
    /// 🔑 **Ids valem como "associado".** A API devolve o id sempre e o resumo
    /// quando o tem; um `voucher_id` sem resumo aparece como "associado", porque
    /// esconder diria que **não há** voucher — e o gesto seguinte do operador
    /// seria associar outro por cima (a mesma regra do `atendimento.ts`).
    fn atendimento(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.atendimento_aberto {
            return None;
        }
        let aberta = self.aberta.as_ref()?;
        let g = aberta.galeria.clone();
        let tema = cx.theme();
        let apagado = tema.muted_foreground;

        let linha = |rotulo: &'static str, valor: String| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_sm()
                .child(div().w(px(150.)).text_color(apagado).child(rotulo))
                .child(div().flex_1().truncate().child(SharedString::from(valor)))
        };
        // 🔑 O resumo quando ele veio; "associado" quando só há o id; "—" quando
        // não há associação nenhuma. Nunca o contrário: esconder um id sem
        // resumo diria que não há associação, e o gesto seguinte seria associar
        // outra por cima (a mesma regra do `atendimento.ts`).
        let resumos = aberta.resumos.clone();
        let com_resumo = |id: Option<String>,
                          resumo: Option<domain::services::pos_venda::ResumoSimples>,
                          associado: &str| match (id, resumo) {
            (None, _) => "—".to_string(),
            (Some(_), Some(r)) if !r.detalhe.trim().is_empty() => {
                format!("{} — {}", r.titulo, r.detalhe)
            }
            (Some(_), Some(r)) => r.titulo,
            (Some(_), None) => associado.to_string(),
        };
        let preset = g
            .preset_padrao_id
            .as_deref()
            .map(|id| {
                crate::sessoes::nova::receita::presets_da_sessao(
                    &self.presets_da_receita,
                    Vec::new(),
                )
                .into_iter()
                .find(|p| p.id == id)
                .map(|p| p.nome)
                .unwrap_or_else(|| id.to_string())
            })
            .unwrap_or_else(|| "—".to_string());
        let proporcao = g
            .proporcao_padrao
            .as_deref()
            .map(|p| crate::sessoes::nova::estado::rotulo_da_proporcao(p).to_string())
            .unwrap_or_else(|| "Sem corte".to_string());

        Some(
            // 🪟 **Gaveta pela direita, como o `Drawer` do site** — e não um
            // bloco que empurra a tela: a galeria continua atrás, e é dela que
            // o operador voltou a olhar assim que fecha. O véu fecha ao clique,
            // como o `onOpenChange` de lá.
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(gpui::black().opacity(0.4))
                .id("atendimento-veu")
                .on_click(cx.listener(|tela, _ev, _w, cx| {
                    tela.atendimento_aberto = false;
                    cx.notify();
                }))
                .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .h_full()
                .w(px(576.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(16.))
                .id("atendimento-gaveta")
                .border_l_1()
                .border_color(tema.border)
                .bg(tema.background)
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("Atendimento"),
                        )
                        .child(
                            Button::new("atendimento-fechar")
                                .label("Fechar")
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|tela, _ev, _w, cx| {
                                    tela.atendimento_aberto = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child("O que veio junto com o cliente, gravado no assistente."),
                )
                .child(linha(
                    "Agendamento",
                    com_resumo(
                        g.ensaio_id.clone(),
                        resumos.agendamento.clone(),
                        "Agendamento associado",
                    ),
                ))
                .child(linha(
                    "Voucher",
                    com_resumo(
                        g.voucher_id.clone(),
                        resumos.voucher.clone(),
                        "Voucher associado",
                    ),
                ))
                .child(linha(
                    "Compra antecipada",
                    com_resumo(
                        g.pedido_id.clone(),
                        resumos.pedido.clone(),
                        "Compra antecipada associada",
                    ),
                ))
                // ✏️ **Editável aqui**, como no site: entrar na sessão com tudo e
                // não poder corrigir a resposta sem recriar a sessão seria meio
                // pedido (`atendimento-da-sessao.tsx`). Trocar limpa o parceiro
                // e o texto de "Outro" — é o que o site recusaria com `400`.
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_start()
                        .text_sm()
                        .child(div().w(px(150.)).text_color(apagado).child("Como conheceu"))
                        .child(
                            div().flex().flex_wrap().gap(px(6.)).children(
                                crate::sessoes::nova::associacoes::COMO_CONHECEU.iter().map(
                                    |(valor, rotulo)| {
                                        let escolhido = g.como_conheceu.as_deref() == Some(*valor);
                                        let valor = *valor;
                                        pilula(
                                            SharedString::from(format!("atend-origem-{valor}")),
                                            rotulo,
                                            None,
                                            escolhido,
                                            cx,
                                        )
                                        .on_click(cx.listener(move |tela, _ev, _w, cx| {
                                            tela.escolher_como_conheceu(valor, cx)
                                        }))
                                    },
                                ),
                            ),
                        ),
                )
                .children(
                    g.como_conheceu_detalhe
                        .as_deref()
                        .filter(|d| !d.trim().is_empty())
                        .map(|detalhe| linha("Detalhe", detalhe.to_string())),
                )
                .child(linha(
                    "Parceiro",
                    com_resumo(
                        g.parceiro_id.clone(),
                        resumos.parceiro.clone(),
                        "Parceiro associado",
                    ),
                ))
                .child(linha("Preset padrão", preset))
                // ✂️ **O corte padrão se troca aqui**, e vale para as próximas
                // fotos da sessão: o serviço da receita repassa a proporção
                // nova a quem ainda não subiu (ver `aplicar_a_receita_da_sessao`
                // na raiz). O preset continua sendo escolhido no assistente,
                // onde as amostras existem.
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_start()
                        .text_sm()
                        .child(div().w(px(150.)).text_color(apagado).child("Corte padrão"))
                        .child(
                            div().flex().flex_wrap().gap(px(6.)).children(
                                std::iter::once(("", "Sem corte"))
                                    .chain(
                                        crate::sessoes::nova::estado::PROPORCOES_PADRAO
                                            .iter()
                                            .map(|p| {
                                                (
                                                    *p,
                                                    crate::sessoes::nova::estado::rotulo_da_proporcao(p),
                                                )
                                            }),
                                    )
                                    .map(|(valor, rotulo)| {
                                        let escolhido = match valor {
                                            "" => g.proporcao_padrao.is_none(),
                                            v => g.proporcao_padrao.as_deref() == Some(v),
                                        };
                                        let valor = valor.to_string();
                                        pilula(
                                            SharedString::from(format!("atend-corte-{valor}")),
                                            rotulo,
                                            None,
                                            escolhido,
                                            cx,
                                        )
                                        .on_click(cx.listener(move |tela, _ev, _w, cx| {
                                            tela.escolher_corte_padrao(&valor, cx)
                                        }))
                                    }),
                            ),
                        ),
                )
                .child(div().text_xs().text_color(apagado).child(
                    SharedString::from(format!("Agora: {proporcao}. Vale para as próximas fotos desta sessão.")),
                )),
                ),
        )
    }

    /// "Apagar" no painel: pergunta primeiro, no diálogo.
    ///
    /// 🚨 **A foto sai do site com os arquivos dela.** É o mesmo `DELETE` que
    /// zerar a classificação usa (`tirar_do_site`), e o site recusa com `409` a
    /// comprada — que aqui nem chega a oferecer o botão.
    fn apagar_do_site(&mut self, cx: &mut Context<Self>) {
        let Some(foto) = self.em_foco().cloned() else {
            return;
        };
        if !foto.editavel() {
            self.recado("Foto comprada não se apaga.".into(), cx);
            return;
        }
        self.apagar_confirmando = Some((foto.id, foto.arquivo));
        cx.notify();
    }

    /// A tecla `0` nas marcadas: **tirar do acervo**, com a cópia vindo antes.
    ///
    /// 🚨 **Era um bloqueio, e o bloqueio é que estava errado** (dono,
    /// 18/set/2026: *"fui tirar a classificação de uma foto e fui bloqueado —
    /// eu preciso desclassificar, retirar a foto da nuvem e trazer a foto para
    /// a minha máquina, exatamente como a versão WEB faz"*). É a cláusula C21
    /// do contrato da foto, e era a divergência D14: o desktop recusava com
    /// *"use Apagar"*, e "Apagar" removia da nuvem sem trazer nada para cá.
    ///
    /// 🔑 **As recusas continuam existindo — só que por foto, e não pelo lote**
    /// (`resgate::pode_voltar`): a comprada tem cobrança atrás, a levada no
    /// balcão não perde a nota, a apagada já não está lá. As outras seguem, e a
    /// tela conta as que ficaram de fora — recusar o lote inteiro faria o
    /// operador procurar qual foi.
    ///
    /// ⚠️ **A foto que só existe no disco não passa por aqui**: ela não está na
    /// nuvem, e tirar a nota dela é só tirar a nota.
    fn pedir_para_tirar_do_acervo(&mut self, cx: &mut Context<Self>) {
        let marcadas: Vec<acervo::Foto> = self
            .selecao
            .marcadas()
            .filter_map(|p| self.acervo.visivel(p))
            .cloned()
            .collect();
        if marcadas.is_empty() {
            return;
        }
        let (podem, ficam): (Vec<acervo::Foto>, Vec<acervo::Foto>) = marcadas
            .into_iter()
            // A local não está na nuvem: não há o que resgatar nem o que remover.
            .filter(|f| !self.locais.iter().any(|l| l.id == f.id))
            .partition(|f| crate::app::resgate::pode_voltar(f.estado, f.apagada));

        if !ficam.is_empty() {
            let nomes: Vec<&str> = ficam.iter().map(|f| f.arquivo.as_str()).collect();
            self.erro = Some(
                format!(
                    "{} foto(s) ficam como estão — comprada ou levada no balcão não perde a \
                     nota ({})",
                    ficam.len(),
                    nomes.join(" / ")
                )
                .into(),
            );
        }
        if podem.is_empty() {
            cx.notify();
            return;
        }
        self.tirar_do_acervo_confirmando = Some(podem);
        cx.notify();
    }

    /// O "Tirar do acervo" do diálogo: agora sim, o gesto vai para a raiz.
    pub fn confirmar_tirar_do_acervo(&mut self, cx: &mut Context<Self>) {
        let Some(fotos) = self.tirar_do_acervo_confirmando.take() else {
            return;
        };
        let fotos = fotos
            .into_iter()
            .map(|f| {
                // Os PARÂMETROS que estão na nuvem voltam com ela — é o que
                // impede a foto de voltar crua (C21).
                let do_site = self
                    .aberta
                    .as_ref()
                    .and_then(|a| a.fotos.iter().find(|g| g.id == f.id));
                let (ajustes, corte) = do_site
                    .and_then(|g| g.ajustes.as_ref())
                    .map(crate::revelacao::persistencia::de_json)
                    .unwrap_or_default();
                crate::app::resgate::AFotoQueVolta {
                    no_site: f.id,
                    arquivo: f.arquivo,
                    ajustes,
                    corte,
                }
            })
            .collect();
        cx.emit(Pedido::TirarDoAcervo(fotos));
        cx.notify();
    }

    pub fn cancelar_tirar_do_acervo(&mut self, cx: &mut Context<Self>) {
        self.tirar_do_acervo_confirmando = None;
        cx.notify();
    }

    /// 🧪 Quais fotos a pergunta "tirar do acervo?" está segurando.
    #[cfg(test)]
    pub(crate) fn fotos_na_pergunta_de_tirar_do_acervo(&self) -> Vec<String> {
        self.tirar_do_acervo_confirmando
            .as_ref()
            .map(|fotos| fotos.iter().map(|f| f.arquivo.clone()).collect())
            .unwrap_or_default()
    }

    /// O "Apagar a foto" do diálogo.
    pub fn confirmar_apagar(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.apagar_confirmando.take() else {
            return;
        };
        cx.emit(Pedido::ApagarDoSite(id));
        cx.notify();
    }

    /// O "Cancelar" do diálogo.
    pub fn cancelar_apagar(&mut self, cx: &mut Context<Self>) {
        self.apagar_confirmando = None;
        cx.notify();
    }

    /// O diálogo de "Tirar do acervo?" — os mesmos textos do site.
    ///
    /// 🔑 **A descrição conta o caminho inteiro**, porque é ele que tira o medo
    /// do gesto: a foto sai do acervo, o cliente deixa de vê-la, e **volta para
    /// esta máquina** — de onde sobe de novo assim que for classificada. E a
    /// última frase é a garantia: o arquivo vem para cá **antes** de sair de lá;
    /// a que não conseguir vir continua no acervo.
    fn dialogo_de_tirar_do_acervo(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let fotos = self.tirar_do_acervo_confirmando.as_ref()?;
        let quantas = fotos.len();
        let titulo = if quantas == 1 {
            "Tirar esta foto do acervo?".to_string()
        } else {
            format!("Tirar {quantas} fotos do acervo?")
        };
        let confirmar = if quantas == 1 {
            "Tirar do acervo".to_string()
        } else {
            format!("Tirar as {quantas}")
        };
        Some(
            crate::estilo::veu_do_dialogo()
                .id("tirar-do-acervo-veu")
                .on_click(cx.listener(|tela, _ev, _w, cx| tela.cancelar_tirar_do_acervo(cx)))
                .child(
                    crate::estilo::caixa_do_dialogo(cx)
                        .w(px(520.))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(crate::estilo::cabecalho_do_dialogo(
                            titulo,
                            "Sem classificação a foto não fica no servidor: ela sai do acervo, o \
                             cliente deixa de vê-la, e volta para esta máquina — de onde sobe de \
                             novo assim que você a classificar. O arquivo vem para cá antes de \
                             sair de lá; a que não conseguir vir continua no acervo.",
                            Some(crate::recursos::Icone::Undo2),
                            cx,
                        ))
                        .child(
                            crate::estilo::rodape_do_dialogo()
                                .child(
                                    crate::estilo::botao_contorno("tirar-do-acervo-cancelar", cx)
                                        .child("Cancelar")
                                        .on_click(cx.listener(|tela, _ev, _w, cx| {
                                            tela.cancelar_tirar_do_acervo(cx)
                                        })),
                                )
                                .child(
                                    crate::estilo::botao_perigo("tirar-do-acervo-confirmar", cx)
                                        .child(SharedString::from(confirmar))
                                        .on_click(cx.listener(|tela, _ev, _w, cx| {
                                            tela.confirmar_tirar_do_acervo(cx)
                                        })),
                                ),
                        ),
                ),
        )
    }

    /// O diálogo de "Apagar esta foto?" — os mesmos textos do site.
    fn dialogo_de_apagar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (_, arquivo) = self.apagar_confirmando.clone()?;
        Some(
            crate::estilo::veu_do_dialogo()
                .id("apagar-veu")
                .on_click(cx.listener(|tela, _ev, _w, cx| tela.cancelar_apagar(cx)))
                .child(
                    crate::estilo::caixa_do_dialogo(cx)
                        .w(px(460.))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(crate::estilo::cabecalho_do_dialogo(
                            "Apagar esta foto?",
                            SharedString::from(format!(
                                "{arquivo} sai da galeria com os arquivos dela — original, prévia \
                                 e miniatura. Não há como desfazer pela tela."
                            )),
                            Some(crate::recursos::Icone::Trash2),
                            cx,
                        ))
                        .child(
                            crate::estilo::rodape_do_dialogo()
                                .child(
                                    crate::estilo::botao_contorno("apagar-cancelar", cx)
                                        .child("Cancelar")
                                        .on_click(cx.listener(|tela, _ev, _w, cx| {
                                            tela.cancelar_apagar(cx)
                                        })),
                                )
                                .child(
                                    crate::estilo::botao_perigo("apagar-confirmar", cx)
                                        .child("Apagar a foto")
                                        .on_click(cx.listener(|tela, _ev, _w, cx| {
                                            tela.confirmar_apagar(cx)
                                        })),
                                ),
                        ),
                ),
        )
    }

    /// a tela passa a mostrar o que gravou, sem inventar o estado novo aqui.
    fn gravar_a_galeria(&mut self, mudanca: MudancaDaGaleria, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        if mudanca.vazia() {
            return;
        }
        self.erro = None;
        self.publicador
            .atualizar_galeria(sessao, galeria_id, mudanca, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// O estúdio da sessão aberta, ou vazio.
    fn estudio_da_galeria(&self) -> String {
        self.aberta
            .as_ref()
            .and_then(|a| a.galeria.estudio_id.clone())
            .unwrap_or_default()
    }

    /// Troca o estúdio da sessão — o `EstudioDaGaleria` do site.
    fn mudar_estudio(&mut self, estudio_id: String, cx: &mut Context<Self>) {
        if estudio_id.trim().is_empty() || estudio_id == self.estudio_da_galeria() {
            return;
        }
        let mudanca = MudancaDaGaleria {
            estudio_id: Some(Some(estudio_id)),
            ..Default::default()
        };
        self.gravar_a_galeria(mudanca, cx);
    }

    /// A foto **como o site a devolveu** — o que a grade não carrega.
    ///
    /// 🔑 A grade fala em [`acervo::Foto`], que é do core e serve aos dois
    /// apps; tamanho, faixa própria e prazos são do painel, e ficam aqui.
    fn do_site(&self, foto_id: &str) -> Option<&FotoDaGaleria> {
        self.aberta.as_ref()?.fotos.iter().find(|f| f.id == foto_id)
    }

    /// A faixa padrão desta galeria — a que vale para a foto sem faixa própria.
    fn produto_padrao_da_galeria(&self) -> String {
        self.aberta
            .as_ref()
            .map(|a| a.galeria.produto_id.clone())
            .unwrap_or_default()
    }

    /// Manda uma mudança para **uma** foto do site — o gesto do painel, que age
    /// na foto em foco, e não na seleção.
    fn pedir_mudanca(
        &mut self,
        foto_id: String,
        mudanca: domain::services::pos_venda::MudancaDaFoto,
        cx: &mut Context<Self>,
    ) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        // A foto que só existe no disco não tem linha no site: o que a leva para
        // lá é a nota (ver `mudar_as_marcadas`).
        if self.locais.iter().any(|f| f.id == foto_id) {
            self.recado(
                "esta foto ainda não subiu — classifique-a (1 a 5) antes".into(),
                cx,
            );
            return;
        }
        if mudanca.vazia() || self.mudando > 0 {
            return;
        }
        self.erro = None;
        self.mudando = 1;
        self.publicador
            .negociar(sessao, foto_id, mudanca, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Monta (ou refaz) os controles do painel para a foto em foco.
    ///
    /// Chamado do `render`, que é quem tem `window`. Sem foco, os campos somem —
    /// e com eles o texto digitado e não aplicado, que era de outra foto.
    fn preparar_painel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(foto) = self.em_foco().cloned() else {
            self.campos_do_painel = None;
            return;
        };
        if self
            .campos_do_painel
            .as_ref()
            .is_some_and(|c| c.foto_id == foto.id)
        {
            return;
        }
        // Mudou a foto em foco: o "Apagar mesmo?" da anterior não vale mais.
        self.apagar_confirmando = None;

        let mut opcoes = vec![OpcaoDaFaixa {
            id: String::new(),
            titulo: SharedString::from(format!(
                "Padrão da galeria ({})",
                self.nome_da_faixa(&self.produto_padrao_da_galeria())
            )),
        }];
        opcoes.extend(self.produtos.iter().map(|p| OpcaoDaFaixa {
            id: p.id.clone(),
            titulo: SharedString::from(format!(
                "{} — {}",
                p.nome,
                dinheiro::ler_campo(&p.preco.replace('.', ","))
                    .map(dinheiro::formatar)
                    .unwrap_or_else(|| format!("R$ {}", p.preco))
            )),
        }));
        // 🚨 **A faixa **própria** da foto, e não a efetiva.** Marcar a efetiva
        // diria que a foto tem faixa fixada quando ela só está seguindo a
        // galeria — e o primeiro clique no seletor "fixaria" sem querer o que
        // era herdado.
        let efetiva = self
            .do_site(&foto.id)
            .and_then(|f| f.produto_id.clone())
            .unwrap_or_default();
        let faixa = cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
        faixa.update(cx, |estado, cx| {
            // A opção "Padrão da galeria" tem id vazio: escolher por valor
            // acerta as duas pontas sem saber a posição de nenhuma.
            estado.set_selected_value(&efetiva, window, cx);
        });

        // O preço fixado vem como decimal em texto ("19.90"); a tela fala em
        // vírgula, como o resto do dinheiro no app.
        let preco = cx.new(|cx| InputState::new(window, cx).placeholder("19,90"));
        // O campo fala em vírgula, como o resto do dinheiro no app; a API leva
        // decimal com ponto (ver `aplicar_preco_de_venda`).
        let valor = foto
            .preco_de_venda
            .map(|centavos| format!("{},{:02}", centavos / 100, centavos % 100))
            .unwrap_or_default();
        preco.update(cx, |campo, cx| campo.set_value(valor, window, cx));

        let mut assinaturas = Vec::new();
        assinaturas.push(cx.subscribe_in(
            &faixa,
            window,
            |tela, _, evento: &SelectEvent<SearchableVec<OpcaoDaFaixa>>, _window, cx| {
                let SelectEvent::Confirm(valor) = evento;
                tela.mudar_faixa(valor.clone().unwrap_or_default(), cx);
            },
        ));
        // Enter aplica, como no campo do site.
        assinaturas.push(cx.subscribe(
            &preco,
            |tela: &mut Detalhe, _estado, evento: &InputEvent, cx| {
                if matches!(evento, InputEvent::PressEnter { .. }) {
                    tela.aplicar_preco_de_venda(cx);
                }
            },
        ));

        self.campos_do_painel = Some(CamposDoPainel {
            foto_id: foto.id,
            faixa,
            preco,
            _assinaturas: assinaturas,
        });
    }

    /// A faixa da foto em foco. Id vazio = devolver ao padrão da galeria.
    ///
    /// 🔑 **É o "Faixa" do painel do site** (`FaixaDaFoto`): a sessão mista tem
    /// fotos de faixas diferentes, e é aqui que a leva que subiu na faixa errada
    /// se conserta — sem isso, só reimportando.
    fn mudar_faixa(&mut self, produto_id: String, cx: &mut Context<Self>) {
        let Some(foto) = self.em_foco().cloned() else {
            return;
        };
        if !foto.editavel() {
            self.recado("Foto comprada não muda de faixa.".into(), cx);
            return;
        }
        let no_site = foto.id.clone();
        let mudanca = domain::services::pos_venda::MudancaDaFoto {
            produto_id: Some(if produto_id.trim().is_empty() {
                None
            } else {
                Some(produto_id)
            }),
            ..Default::default()
        };
        self.pedir_mudanca(no_site, mudanca, cx);
    }

    /// "Preço de venda online" — o que o cliente paga por **esta** foto.
    ///
    /// Campo vazio devolve ao preço da faixa (`Some(None)`). Zero é recusado
    /// aqui, como o site recusa com `400`: zero é cortesia, e cortesia é
    /// negociação — que é outro campo, e não muda o que a compra cobra.
    fn aplicar_preco_de_venda(&mut self, cx: &mut Context<Self>) {
        let Some(foto) = self.em_foco().cloned() else {
            return;
        };
        let Some(campos) = self.campos_do_painel.as_ref() else {
            return;
        };
        if !foto.editavel() {
            self.recado("Foto comprada não muda de preço.".into(), cx);
            return;
        }
        let texto = campos.preco.read(cx).value().trim().to_string();
        let novo = if texto.is_empty() {
            None
        } else {
            match dinheiro::ler_campo(&texto) {
                Some(centavos) if centavos > 0 => {
                    Some(format!("{}.{:02}", centavos / 100, centavos % 100))
                }
                Some(_) => {
                    self.recado(
                        "Preço zero é cortesia: registre pela negociação, no caixa.".into(),
                        cx,
                    );
                    return;
                }
                None => {
                    self.recado("Preço inválido. Use 19,90.".into(), cx);
                    return;
                }
            }
        };
        let mudanca = domain::services::pos_venda::MudancaDaFoto {
            preco_de_venda: Some(novo),
            ..Default::default()
        };
        self.pedir_mudanca(foto.id.clone(), mudanca, cx);
    }

    fn painel(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        use crate::recursos::Icone;
        use gpui_component::Icon;
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
                // 📂 **A sanfona do site**: tamanho, faixa, negociação, preço e
                // apagar ficam atrás de "Faixa, negociação e preço", fechada por
                // padrão. Antes eram onze controles empilhados com o mesmo peso,
                // e o operador procurava o botão a cada foto.
                .child(
                    div()
                        .id("painel-sanfona")
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .py(px(4.))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            Icon::new(if self.faixa_e_precos_aberto {
                                Icone::ChevronDown
                            } else {
                                Icone::ChevronRight
                            })
                            .size(px(14.)),
                        )
                        .child("Faixa, negociação e preço")
                        .on_click(cx.listener(|tela, _ev, _w, cx| {
                            tela.faixa_e_precos_aberto = !tela.faixa_e_precos_aberto;
                            cx.notify();
                        })),
                )
                .when(self.faixa_e_precos_aberto, |painel| {
                    painel
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from({
                                    // "12,7 MB · Downloads 0 · na nuvem" — as três
                                    // linhas do painel do site numa só, porque aqui a
                                    // coluna é estreita.
                                    let tamanho = self
                                        .do_site(&foto.id)
                                        .and_then(|f| f.tamanho_bytes)
                                        .map(|bytes| {
                                            format!("{:.1} MB · ", bytes as f64 / 1_048_576.0)
                                                .replace('.', ",")
                                        })
                                        .unwrap_or_default();
                                    let onde = if self.ids_locais.contains(&foto.id) {
                                        "neste computador"
                                    } else {
                                        "na nuvem"
                                    };
                                    format!("{tamanho}Downloads {} · {onde}", foto.downloads)
                                })),
                        )
                        // 🧾 **Faixa e preço, como no painel do site** (`FaixaDaFoto` e
                        // `PrecoDeVendaDaFoto`). Eles não existiam aqui, e a frase que
                        // mandava ao Balcão não resolvia: lá se registra o que **entrou
                        // no balcão**; a faixa e o preço de venda são o que o cliente vê
                        // e paga na galeria dele.
                        .children(self.campos_do_painel.as_ref().map(|campos| {
                            let apagado = cx.theme().muted_foreground;
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(div().text_xs().text_color(apagado).child("Faixa"))
                                .child(
                                    Select::new(&campos.faixa)
                                        .xsmall()
                                        .placeholder("Escolha…")
                                        .w_full(),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(apagado)
                                        .child("Preço de venda online"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .items_center()
                                        .child(div().text_xs().text_color(apagado).child("R$"))
                                        .child(
                                            div()
                                                .flex_1()
                                                .child(Input::new(&campos.preco).xsmall()),
                                        )
                                        .child(
                                            Button::new("painel-preco-aplicar")
                                                .label("Aplicar")
                                                .xsmall()
                                                .disabled(!foto.editavel())
                                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                                    tela.aplicar_preco_de_venda(cx)
                                                })),
                                        ),
                                )
                                .child(div().text_xs().text_color(apagado).child(
                                    match foto.preco_de_venda {
                                        Some(centavos) => format!(
                                            "O cliente paga {} por esta foto.",
                                            dinheiro::formatar(centavos)
                                        ),
                                        None => {
                                            "Sem valor fixado: vale o preço da faixa.".to_string()
                                        }
                                    },
                                ))
                        }))
                        .when(negociada, |painel| {
                            painel.child(div().text_xs().text_color(cx.theme().warning).child(
                                match foto.preco_negociado {
                                    Some(centavos) => {
                                        format!("Balcão: {}", dinheiro::formatar(centavos))
                                    }
                                    None => "Balcão: registrado".to_string(),
                                },
                            ))
                        })
                        // 🤝 **Negociação e apagar**, os dois últimos controles que o
                        // painel do site tem e este não tinha.
                        .when(foto.editavel(), |painel| {
                            painel.child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(
                                        Button::new("painel-negociar")
                                            .label("Negociação…")
                                            .xsmall()
                                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                                let Some(foto) = tela.em_foco().cloned() else {
                                                    return;
                                                };
                                                cx.emit(Pedido::Negociar(vec![foto.id]));
                                            })),
                                    )
                                    .child(
                                        Button::new("painel-apagar")
                                            .label("Apagar")
                                            .xsmall()
                                            .danger()
                                            .ghost()
                                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                                tela.apagar_do_site(cx)
                                            })),
                                    ),
                            )
                        })
                }),
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

        // Só o pedaço à vista vira elemento; o resto são dois espaçadores do
        // mesmo tamanho, e a rolagem não percebe a diferença.
        let passo = largura + VAO_DA_TIRA;
        let (de, ate) = self.tira_desenhada;
        let ate = ate.min(total);
        let de = de.min(ate);
        let espacador = |itens: usize| {
            div()
                .flex_none()
                .w(px(itens as f32 * passo - VAO_DA_TIRA))
                .h(px(1.))
                .into_any_element()
        };
        let mut itens: Vec<gpui::AnyElement> = Vec::with_capacity(ate - de + 2);
        if de > 0 {
            itens.push(espacador(de));
        }
        itens.extend((de..ate).filter_map(|posicao| {
            self.acervo
                .visivel(posicao)
                .map(|foto| self.miniatura_da_tira(posicao, foto, lado, largura, cx))
        }));
        if ate < total {
            itens.push(espacador(total - ate));
        }

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
                    // 🔑 **O `0` estava faltando**, e ele é metade do gesto: a
                    // nota sobe a foto, e tirá-la a traz de volta (C20–C22). A
                    // linha do site o traz desde sempre.
                    .child(tecla("0", cx))
                    .child("tira a nota ·")
                    .child(tecla("P", cx))
                    .child("levada no balcão ·")
                    // ⌘ no Mac, Ctrl no resto — o `useTeclaDeAtalho` do site.
                    .child(tecla(MODIFICADOR, cx))
                    .child("ou")
                    .child(tecla("Shift", cx))
                    .child("no clique marcam várias ·")
                    .child(tecla(MODIFICADOR_A, cx))
                    .child("marca tudo,")
                    .child(tecla(MODIFICADOR_D, cx))
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
                            .gap(px(VAO_DA_TIRA))
                            .px(px(RECUO_DA_TIRA))
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
                            .children(itens),
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
        let miniatura = match self.miniaturas.espiar(&self.chave_da_foto(&foto.id)) {
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
                        // 🚨 `Contain`, e não `Cover`. Preencher o retângulo
                        // custa **cortar**, e numa foto que já foi
                        // **enquadrada** isso corta o que o operador escolheu
                        // manter: a mesma foto aparecia com um pedaço a menos
                        // aqui e inteira no editor e na tela do cliente (o
                        // dono, 2026-09-11). É a mesma decisão que a
                        // Biblioteca já tinha tomado ao lado — *"num estúdio
                        // de retrato o recorte centralizado tira a cabeça
                        // primeiro"* —, e as duas tarjas de fundo são o preço.
                        .object_fit(gpui::ObjectFit::Contain)
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
/// Uma ficha da galeria do site: pílula com borda, a acesa em âmbar.
fn pilula(
    id: SharedString,
    rotulo: &'static str,
    quantas: Option<usize>,
    acesa: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let tema = cx.theme();
    let (borda, apagado, acento) = (tema.border, tema.muted_foreground, tema.accent);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(10.))
        .rounded_full()
        .border_1()
        .text_sm()
        .cursor_pointer()
        .when(acesa, |p| {
            p.bg(cores::quente())
                .border_color(cores::quente())
                .text_color(cores::sobre_quente())
        })
        .when(!acesa, |p| {
            p.border_color(borda).hover(move |s| s.bg(acento))
        })
        .child(rotulo)
        .when_some(quantas, |p, n| {
            p.child(
                div()
                    .when(!acesa, |d| d.text_color(apagado))
                    .when(acesa, |d| d.opacity(0.75))
                    .child(n.to_string()),
            )
        })
}

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
/// A pasta deste ensaio dentro do catálogo — `<catálogo>/Ensaios/<título> - <id>`.
///
/// 🔑 **O id é o que a torna previsível; o título é o que a torna achável.** Só
/// o id daria uma pasta com nome de UUID, que ninguém reconhece no Finder; só o
/// título daria colisão entre dois "Ensaio da Ana" e mudaria de lugar a cada
/// correção de nome.
///
/// ⚠️ **O título passa por [`sanear`] antes de virar caminho.** Uma barra no
/// nome do ensaio ("Ana / Bruno") criaria uma subpasta sem ninguém pedir, e dois
/// pontos quebram o caminho no macOS.
pub fn pasta_do_ensaio(titulo: &str, galeria_id: &str) -> std::path::PathBuf {
    let nome = match sanear(titulo) {
        t if t.is_empty() => galeria_id.to_string(),
        t => format!("{t} - {galeria_id}"),
    };
    infrastructure::paths::AppPaths::catalog_root()
        .join("Ensaios")
        .join(nome)
}

/// Deixa só o que é seguro num nome de pasta, nos três sistemas.
fn sanear(texto: &str) -> String {
    let limpo: String = texto
        .chars()
        .map(|c| match c {
            c if c.is_alphanumeric() => c,
            ' ' | '-' | '_' | '.' => c,
            _ => '-',
        })
        .collect();
    // Espaço e ponto no fim somem no Windows, e um nome que termina em ponto
    // vira outro nome sem ninguém saber.
    limpo.trim().trim_end_matches('.').trim().to_string()
}

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
    use crate::importacao::explorador::mentira::ImportadorDeMentira;
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
            ..Default::default()
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
                ..Default::default()
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
        janela_completa(
            cx,
            publicador,
            seletor,
            Arc::new(ImportadorDeMentira::default()),
        )
    }

    fn janela_completa(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
        seletor: Arc<SeletorDeMentira>,
        importador: Arc<ImportadorDeMentira>,
    ) -> gpui::WindowHandle<Detalhe> {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
        std::mem::forget(dir);
        janela_com_previews(cx, publicador, seletor, importador, previews)
    }

    /// 🚨 **O cache entra por parâmetro**, e não é detalhe de teste: um
    /// `PreviewManager::new()` aqui gravaria no cache **do fotógrafo** durante o
    /// `cargo test` — já aconteceu duas vezes neste repositório
    /// (`docs/07-E2E-TESTING.md` §4).
    fn janela_com_previews(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
        seletor: Arc<SeletorDeMentira>,
        importador: Arc<ImportadorDeMentira>,
        previews: Arc<PreviewManager>,
    ) -> gpui::WindowHandle<Detalhe> {
        cx.update(gpui_component::init);
        cx.add_window(move |_window, cx| {
            let mut tela = Detalhe::nova(publicador, seletor, importador, previews, cx);
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
    /// 🚨 **Entrar noutra sessão esvazia a grade** (dono, 18/set/2026: *"parece
    /// que a sessão anterior ainda estava na memória, uma por cima da outra"*).
    ///
    /// As fotos do site só eram trocadas quando a galeria nova respondia, e as
    /// **locais** só quando a raiz mandasse outras — se a sessão nova não
    /// tivesse nenhuma foto no disco, as da anterior ficavam na tela para
    /// sempre. O estado honesto enquanto se lê a nova é o vazio.
    #[gpui::test]
    fn entrar_noutra_sessao_nao_deixa_a_anterior_na_tela(cx: &mut TestAppContext) {
        let (janela, _publicador) =
            janela(cx, vec![foto("a", EstadoDaFotoNoSite::Disponivel, Some(5))]);
        janela
            .update(cx, |tela, _window, cx| tela.entrar("g1".into(), cx))
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        // Uma foto local desta sessão, como a raiz manda depois de importar.
        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_locais(
                    vec![biblioteca_core::acervo::Foto {
                        id: "id-local.jpg".into(),
                        arquivo: "local.jpg".into(),
                        estado: biblioteca_core::acervo::Estado::Disponivel,
                        apagada: false,
                        produto_efetivo: String::new(),
                        preco_negociado: None,
                        tem_observacao: false,
                        preco_de_venda: None,
                        pedido_id: None,
                        downloads: 0,
                        revelada: false,
                        nota: None,
                        ordem: 1,
                    }],
                    cx,
                );
                assert_eq!(
                    tela.ids_visiveis(),
                    vec!["a".to_string(), "id-local.jpg".to_string()],
                    "a grade tem a do site e a do disco"
                );
            })
            .expect("a janela deve estar aberta");

        // Outra sessão: a grade começa vazia, e não com as fotos da anterior.
        janela
            .update(cx, |tela, _window, cx| {
                tela.entrar("g2".into(), cx);
                assert!(
                    tela.ids_visiveis().is_empty(),
                    "a sessão anterior ficou na tela: {:?}",
                    tela.ids_visiveis()
                );
            })
            .expect("a janela deve estar aberta");
    }

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

    /// A galeria "g1" do publicador sem contato nenhum — como o site a teria.
    fn tirar_o_contato(publicador: &PublicadorDeMentira) {
        let mut galerias = publicador.galerias.lock().expect("as galerias");
        galerias[0].email = None;
        galerias[0].whatsapp = None;
    }

    /// Os três campos do formulário, já criados.
    fn campos_do_cliente(
        tela: &mut Detalhe,
        window: &mut Window,
        cx: &mut Context<Detalhe>,
    ) -> (Entity<InputState>, Entity<InputState>, Entity<InputState>) {
        tela.preparar_formulario_do_cliente(window, cx);
        let campos = tela
            .dados_do_cliente
            .as_ref()
            .and_then(|f| f.campos.as_ref())
            .expect("o formulário tem campos");
        (
            campos.titulo.clone(),
            campos.email.clone(),
            campos.whatsapp.clone(),
        )
    }

    /// 🔚 Dono, 2026-09-13: *"Essas informações são obrigatórias no final da
    /// sessão."* O "Copiar link" numa sessão sem contato **não pede o link**:
    /// abre o pedido de contato, grava por `PATCH` só o e-mail e segue com o
    /// gesto — o link sai sem um segundo clique.
    #[gpui::test]
    fn copiar_o_link_sem_email_pede_o_contato_grava_e_segue_o_gesto(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, vec![]);
        tirar_o_contato(&publicador);
        entrar(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                assert!(!tela.tem_email());
                tela.pedir_o_link(cx);
                assert_eq!(
                    tela.motivo_do_formulario(),
                    Some(MotivoDoFormulario::ContatoParaOLink)
                );
                assert!(!tela.pedindo_link, "sem contato o pedido não sai");

                // Sem nada preenchido, o pedido não grava.
                let (_, email, _) = campos_do_cliente(tela, window, cx);
                tela.gravar_dados_do_cliente(cx);
                assert!(tela.gravando_dados.is_none());
                assert!(tela
                    .dados_do_cliente
                    .as_ref()
                    .is_some_and(|f| f.erro.is_some()));

                email.update(cx, |campo, cx| {
                    campo.set_value("ana@exemplo.com", window, cx)
                });
                tela.gravar_dados_do_cliente(cx);
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert_eq!(
            *publicador.atualizacoes.lock().expect("as atualizacoes"),
            vec![(
                "g1".to_string(),
                MudancaDaGaleria {
                    email: Some(Some("ana@exemplo.com".into())),
                    ..Default::default()
                }
            )],
            "só o e-mail vai no PATCH"
        );
        assert_eq!(
            *publicador.links.lock().expect("os links"),
            vec!["g1".to_string()],
            "depois de gravar, o gesto seguiu"
        );
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.link().is_some());
                assert!(tela.tem_email());
                assert!(tela.motivo_do_formulario().is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔚 A tela velha acha que há contato e o site responde `422`: o mesmo
    /// pedido de contato abre, com a frase do site — e a colheita não fica
    /// presa esperando um aviso que não vem.
    #[gpui::test]
    fn o_422_do_site_abre_o_mesmo_pedido_de_contato(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, vec![]);
        tirar_o_contato(&publicador);
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                if let Some(aberta) = tela.aberta.as_mut() {
                    aberta.galeria.email = Some("ana@x.com".into());
                }
                tela.avisar(cx);
                assert!(tela.avisando, "a tela achou que havia contato");
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert!(publicador.avisadas.lock().expect("as avisadas").is_empty());
        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    tela.motivo_do_formulario(),
                    Some(MotivoDoFormulario::ContatoParaAvisar)
                );
                assert!(tela
                    .dados_do_cliente
                    .as_ref()
                    .and_then(|f| f.erro.clone())
                    .is_some_and(|frase| frase.contains("informe o e-mail")));
                assert!(!tela.avisando);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Só com WhatsApp o link também pede o e-mail** (usuários em
    /// produção, 2026-09-13: *"ensaios sem e-mail estão gerando problema ao
    /// gerar o link"*). O WhatsApp vem preenchido e o PATCH leva só o e-mail.
    #[gpui::test]
    fn so_com_whatsapp_o_link_pede_o_email(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, vec![]);
        {
            let mut galerias = publicador.galerias.lock().expect("as galerias");
            galerias[0].email = None;
            galerias[0].whatsapp = Some("5547999998888".into());
        }
        entrar(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                assert!(!tela.tem_email());
                tela.pedir_o_link(cx);
                assert_eq!(
                    tela.motivo_do_formulario(),
                    Some(MotivoDoFormulario::ContatoParaOLink)
                );
                let (_, email, whatsapp) = campos_do_cliente(tela, window, cx);
                assert_eq!(whatsapp.read(cx).value().to_string(), "5547999998888");

                // Só o WhatsApp não grava neste motivo.
                tela.gravar_dados_do_cliente(cx);
                assert!(tela.gravando_dados.is_none());

                email.update(cx, |campo, cx| {
                    campo.set_value("ana@exemplo.com", window, cx)
                });
                tela.gravar_dados_do_cliente(cx);
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert_eq!(
            *publicador.atualizacoes.lock().expect("as atualizacoes"),
            vec![(
                "g1".to_string(),
                MudancaDaGaleria {
                    email: Some(Some("ana@exemplo.com".into())),
                    ..Default::default()
                }
            )]
        );
        assert_eq!(
            *publicador.links.lock().expect("os links"),
            vec!["g1".to_string()]
        );
    }

    /// ✏️ *"Dentro da sessão precisa ser possível mudar o Título, email e o
    /// whatsapp."* — dono, 2026-09-13. Os campos vêm com o que a sessão tem, e
    /// o `PATCH` leva só o que mudou.
    #[gpui::test]
    fn editar_os_dados_do_cliente_manda_so_o_que_mudou(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, vec![]);
        entrar(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.editar_dados_do_cliente(cx);
                let (titulo, email, _) = campos_do_cliente(tela, window, cx);
                assert_eq!(titulo.read(cx).value().to_string(), "Ensaio");
                assert_eq!(email.read(cx).value().to_string(), "ana@x.com");
                titulo.update(cx, |campo, cx| campo.set_value("Ensaio da Ana", window, cx));
                tela.gravar_dados_do_cliente(cx);
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert_eq!(
            *publicador.atualizacoes.lock().expect("as atualizacoes"),
            vec![(
                "g1".to_string(),
                MudancaDaGaleria {
                    titulo: Some("Ensaio da Ana".into()),
                    ..Default::default()
                }
            )]
        );
        assert!(
            publicador.links.lock().expect("os links").is_empty(),
            "editar não segue gesto nenhum"
        );
        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    tela.aberta().map(|a| a.galeria.titulo.clone()),
                    Some("Ensaio da Ana".to_string())
                );
                assert!(tela.motivo_do_formulario().is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// ✏️ Na edição o título vazio não grava, e o **último contato se apaga** —
    /// ele só é exigido no fim da sessão.
    #[gpui::test]
    fn na_edicao_titulo_vazio_nao_grava_e_o_ultimo_contato_se_apaga(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, vec![]);
        entrar(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.editar_dados_do_cliente(cx);
                let (titulo, email, _) = campos_do_cliente(tela, window, cx);

                titulo.update(cx, |campo, cx| campo.set_value("   ", window, cx));
                tela.gravar_dados_do_cliente(cx);
                assert!(tela.gravando_dados.is_none(), "título vazio não vai à rede");

                titulo.update(cx, |campo, cx| campo.set_value("Ensaio", window, cx));
                email.update(cx, |campo, cx| campo.set_value("", window, cx));
                tela.gravar_dados_do_cliente(cx);
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert_eq!(
            *publicador.atualizacoes.lock().expect("as atualizacoes"),
            vec![(
                "g1".to_string(),
                MudancaDaGaleria {
                    email: Some(None),
                    ..Default::default()
                }
            )]
        );
        janela
            .update(cx, |tela, _window, _cx| assert!(!tela.tem_email()))
            .expect("a janela deve estar aberta");
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

    fn caminhos_de_teste(quantos: usize) -> Vec<String> {
        (0..quantos)
            .map(|i| format!("/fotos/DSC_{i:04}.jpg"))
            .collect()
    }

    /// 🚨 **O clique no "Importar" grava no catálogo local — e não sobe nada.**
    ///
    /// Duas coisas num teste só, porque são a mesma regra vista dos dois lados.
    ///
    /// A primeira é a que o `docs/07-E2E-TESTING.md` §1 pede: o botão trocou de
    /// nome e de método em 8/set/2026 (`escolher_fotos` → `importar`), e um
    /// `on_click` que aponta para o lugar errado **não falha** — ele só não faz
    /// nada. O clique aqui é nas coordenadas do botão desenhado.
    ///
    /// A segunda é o **destino**, regra do dono do mesmo dia: *"a importação não
    /// vai imediatamente para o storage cloud, pois o cliente precisa
    /// classificar a foto; ela fica local usando sqlite"*. Até então o botão
    /// chamava `enviar_arquivo`, e o site devolvia **400 Bad Request: a foto
    /// sobe classificada** — 21 de 21 arquivos, e a sessão vazia na tela.
    #[gpui::test]
    fn o_clique_no_importar_grava_no_catalogo_e_nao_sobe_nada(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::escolhe(&["/fotos/a.jpg", "/fotos/b.NEF"]));
        let publicador = publicador_com(
            vec![foto("x", EstadoDaFotoNoSite::Disponivel, Some(4))],
            false,
        );
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela_completa(cx, publicador.clone(), seletor.clone(), importador.clone());
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

        let lotes = importador.importados();
        assert_eq!(lotes.len(), 1, "um lote foi para o catálogo local");
        let (arquivos, opcoes) = &lotes[0];
        assert_eq!(
            arquivos,
            &vec!["/fotos/a.jpg".to_string(), "/fotos/b.NEF".into()]
        );
        // 🚨 O carimbo do ensaio entra na criação: sem ele a foto chega ao
        // catálogo sem dono e não aparece na grade da sessão que a importou.
        assert_eq!(opcoes.sessao_id.as_deref(), Some("g1"));
        assert!(
            publicador.arquivos_enviados().is_empty(),
            "a importação subiu para o site — quem autoriza a foto a subir é a nota"
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
            escolha: std::sync::Mutex::new(caminhos_de_teste(500)),
            ..Default::default()
        });
        let publicador = publicador_com(
            vec![
                foto("a", EstadoDaFotoNoSite::Disponivel, Some(4)),
                foto("b", EstadoDaFotoNoSite::Disponivel, Some(4)),
            ],
            true,
        );
        let importador = Arc::new(ImportadorDeMentira::demorado());
        let janela = janela_completa(cx, publicador.clone(), seletor.clone(), importador.clone());
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
                assert_eq!(lote.prontas(), 0, "nenhum arquivo respondeu ainda");
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
            escolha: std::sync::Mutex::new(caminhos_de_teste(4)),
            ..Default::default()
        });
        let publicador = publicador_com(
            vec![foto("a", EstadoDaFotoNoSite::Disponivel, Some(4))],
            true,
        );
        let importador = Arc::new(ImportadorDeMentira::demorado());
        let janela = janela_completa(cx, publicador.clone(), seletor.clone(), importador.clone());
        entrar_demorado(cx, &janela, &publicador);

        clicar(cx, &janela, "detalhe-importar");
        let colher = |cx: &mut TestAppContext| {
            janela
                .update(cx, |tela, _window, cx| tela.colher(cx))
                .expect("a janela deve estar aberta")
        };
        colher(cx);

        let porcento = |cx: &mut TestAppContext| {
            janela
                .update(cx, |tela, _window, _cx| {
                    tela.importacao().expect("o lote").porcento()
                })
                .expect("a janela deve estar aberta")
        };

        // O `Comecou` do importador: ele conta o lote de novo, e é ele que vale
        // — a lista pode encolher (duplicata que o importador descarta antes).
        importador.responder_uma();
        colher(cx);
        assert_eq!(porcento(cx), 0.);

        // Dois arquivos entram — a barra vai à metade.
        importador.responder_uma();
        importador.responder_uma();
        colher(cx);
        assert_eq!(porcento(cx), 50.);

        // Um falha. Ela **conta**: o que a barra mede é o que falta esperar.
        {
            let mut guardados = importador.guardados.lock().expect("os guardados");
            guardados[0].1 = Andamento::Falhou {
                caminho: "/fotos/DSC_0002.jpg".into(),
                erro: "o disco recusou".into(),
            };
        }
        importador.responder_uma();
        colher(cx);
        assert_eq!(porcento(cx), 75.);
        janela
            .update(cx, |tela, _window, _cx| {
                let lote = tela.importacao().expect("o lote");
                assert_eq!((lote.feitas, lote.falhas), (2, 1));
                assert!(tela.importando(), "ainda falta uma");
            })
            .expect("a janela deve estar aberta");

        // A última. O lote acaba, e a raiz é chamada para reler o catálogo —
        // sem isso as fotos ficariam gravadas e invisíveis.
        let tela = janela.root(cx).expect("a raiz da janela");
        let releituras = Arc::new(std::sync::Mutex::new(0usize));
        let _inscricao = cx.update({
            let releituras = releituras.clone();
            move |cx| {
                cx.subscribe(&tela, move |_tela, pedido: &Pedido, _cx| {
                    if matches!(pedido, Pedido::CatalogoMudou) {
                        *releituras.lock().expect("as releituras") += 1;
                    }
                })
            }
        });

        importador.responder_uma();
        colher(cx);
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                let lote = tela.importacao().expect("o lote terminou");
                assert_eq!((lote.feitas, lote.falhas), (3, 1));
                assert!(lote.terminou());
                assert!(!tela.importando(), "a barra tinha de sair da tela");
            })
            .expect("a janela deve estar aberta");
        assert_eq!(
            *releituras.lock().expect("as releituras"),
            1,
            "o fim do lote tem de pedir a releitura do catálogo à raiz"
        );
    }

    /// 🚨 **"Cliquei em importar, selecionei as fotos, e não aconteceu nada."**
    ///
    /// Relatado pelo dono em 8/set/2026, com o app rodando — e nenhum teste
    /// pegava, porque todos eles tinham um seletor que respondia **na mesma
    /// linha** em que era chamado. A janela do sistema não responde na mesma
    /// linha: ela fica aberta os segundos que o operador levar para achar a
    /// pasta.
    ///
    /// O que acontecia nesses segundos: a colheita da tela é um laço que acorda
    /// a cada 100 ms e **desiste quando não há mais nada a esperar**
    /// (`colher` devolve `continua`). "Esperar o operador escolher" não estava
    /// na lista. Primeiro tique depois do clique: nada carregando, nada
    /// subindo, nada baixando — o laço morria. Quando os caminhos enfim
    /// chegavam ao canal, **não havia mais ninguém drenando**: eles ficavam lá,
    /// para sempre, e a tela não piscava.
    ///
    /// ⚠️ **A resposta imediata da mentira é o que escondia isto**, e é a mesma
    /// lição que o `demorada` do publicador já tinha ensinado
    /// (`docs/07-E2E-TESTING.md` §4): o teste que não deixa o tempo passar não
    /// pode ver um defeito que só existe no tempo.
    #[gpui::test]
    fn escolher_as_fotos_com_calma_ainda_sobe_o_lote(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::demorado(&[
            "/fotos/a.jpg",
            "/fotos/b.jpg",
        ]));
        let publicador = publicador_com(
            vec![foto("x", EstadoDaFotoNoSite::Disponivel, Some(4))],
            false,
        );
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela_completa(cx, publicador.clone(), seletor.clone(), importador.clone());
        entrar(cx, &janela);

        clicar(cx, &janela, "detalhe-importar");
        assert_eq!(seletor.pedidos(), 1, "a janela do sistema abriu");

        // O operador procura a pasta. Cinco segundos — nada demais.
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(5));
        cx.run_until_parked();

        // E enfim escolhe.
        seletor.responder();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        let lotes = importador.importados();
        assert_eq!(
            lotes.len(),
            1,
            "as fotos escolhidas ficaram no canal: a colheita desistiu enquanto \
             a janela do sistema estava aberta"
        );
        assert_eq!(
            lotes[0].0,
            vec!["/fotos/a.jpg".to_string(), "/fotos/b.jpg".into()]
        );
    }

    /// ⚠️ **Fechar a janela sem escolher deixa a colheita parar.**
    ///
    /// A contraprova do teste acima, e ela não é adorno: o que segura o laço de
    /// pé é `escolhendo`, e um `escolhendo` que só desligasse na lista **não
    /// vazia** deixaria o laço acordando a cada 100 ms para sempre depois de um
    /// `Cancelar` — sem sintoma nenhum além do ventilador. Desistir é um gesto
    /// legítimo, e o seletor responde a ele com lista vazia.
    #[gpui::test]
    fn fechar_a_janela_sem_escolher_deixa_a_colheita_parar(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::demorado(&[]));
        let publicador = publicador_com(Vec::new(), false);
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela_completa(cx, publicador, seletor.clone(), importador.clone());
        entrar(cx, &janela);

        clicar(cx, &janela, "detalhe-importar");
        seletor.responder();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        let continua = janela
            .update(cx, |tela, _window, cx| tela.colher(cx))
            .expect("a janela deve estar aberta");
        assert!(
            !continua,
            "o Cancelar deixou a colheita acordando a cada 100 ms, para sempre"
        );
        assert!(importador.importados().is_empty(), "não importou nada");
    }

    /// A foto que a raiz achou no catálogo, na linguagem da grade.
    fn local(id: &str) -> acervo::Foto {
        acervo::Foto {
            id: id.into(),
            arquivo: format!("{id}.jpg"),
            estado: acervo::Estado::Disponivel,
            apagada: false,
            produto_efetivo: String::new(),
            preco_negociado: None,
            tem_observacao: false,
            preco_de_venda: None,
            pedido_id: None,
            downloads: 0,
            revelada: false,
            nota: None,
            ordem: 0,
        }
    }

    /// ✏️ **A gaveta do atendimento corrige sem recriar a sessão.**
    ///
    /// "Como conheceu" e o corte padrão vão num `PATCH` com **só** o que mudou —
    /// e trocar a resposta limpa o parceiro, que é o que o site recusaria.
    #[gpui::test]
    fn o_atendimento_corrige_a_origem_e_o_corte(cx: &mut TestAppContext) {
        let publicador = publicador_com(Vec::new(), false);
        {
            let mut galerias = publicador.galerias.lock().unwrap();
            for galeria in galerias.iter_mut() {
                if galeria.id == "g1" {
                    galeria.como_conheceu = Some("parceiro".into());
                    galeria.parceiro_id = Some("pa1".into());
                }
            }
        }
        let janela = janela_com(
            cx,
            publicador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_como_conheceu("instagram", cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let mudancas = publicador.atualizacoes();
        assert_eq!(mudancas.len(), 1);
        let m = &mudancas[0].1;
        assert_eq!(m.como_conheceu.as_ref(), Some(&Some("instagram".into())));
        assert_eq!(
            m.parceiro_id.as_ref(),
            Some(&None),
            "trocar a origem solta o parceiro — senão o site recusa com 400"
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_corte_padrao("1:1", cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let mudancas = publicador.atualizacoes();
        assert_eq!(
            mudancas.last().map(|(_, m)| m.proporcao_padrao.clone()),
            Some(Some(Some("1:1".into())))
        );
    }

    /// 🗑️ **"Apagar" pergunta antes, e some com a foto do site.**
    ///
    /// O diálogo é o `useConfirmacao` do site: o balcão é tela de dedo rápido, e
    /// o `DELETE` leva os arquivos junto.
    #[gpui::test]
    fn apagar_no_painel_pede_confirmacao_no_dialogo(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![foto("f1", EstadoDaFotoNoSite::Disponivel, Some(4))],
        );
        entrar(cx, &janela);

        let apagados = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let recebidos = apagados.clone();
        let raiz = cx.update(|cx| janela.root(cx).expect("a tela"));
        let _assinatura = cx.update(|cx| {
            cx.subscribe(&raiz, move |_, evento: &Pedido, _| {
                if let Pedido::ApagarDoSite(id) = evento {
                    recebidos.borrow_mut().push(id.clone());
                }
            })
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.clicar(0, Modificadores::default(), cx);
                tela.apagar_do_site(cx);
                assert_eq!(
                    tela.apagar_confirmando.as_ref().map(|(id, _)| id.as_str()),
                    Some("f1"),
                    "o clique abre o diálogo, e nada é apagado ainda"
                );
                tela.confirmar_apagar(cx);
                assert!(
                    tela.apagar_confirmando.is_none(),
                    "confirmar fecha o diálogo"
                );
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert_eq!(apagados.borrow().as_slice(), ["f1"]);
    }

    /// 📋 **O atendimento entra na sessão, e o estúdio se troca no cabeçalho.**
    ///
    /// O que o assistente coletou nas sete etapas ficava só no banco: a sessão
    /// abria sem dizer se havia agendamento, voucher, compra, como conheceu ou
    /// receita padrão — e o estúdio não tinha onde ser corrigido.
    #[gpui::test]
    fn o_cabecalho_mostra_o_atendimento_e_troca_o_estudio(cx: &mut TestAppContext) {
        let publicador = publicador_com(
            vec![foto("f1", EstadoDaFotoNoSite::Disponivel, Some(4))],
            false,
        );
        {
            let mut galerias = publicador.galerias.lock().unwrap();
            for galeria in galerias.iter_mut() {
                if galeria.id == "g1" {
                    galeria.como_conheceu = Some("instagram".into());
                    galeria.voucher_id = Some("v1".into());
                    galeria.preset_padrao_id = Some("sistema:sepia".into());
                    galeria.proporcao_padrao = Some("3:2".into());
                    galeria.estudio_id = Some("e1".into());
                }
            }
        }
        let janela = janela_com(
            cx,
            publicador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(
                    tela.quantas_associacoes(),
                    3,
                    "voucher, como conheceu e receita padrão"
                );
                assert_eq!(tela.estudio_da_galeria(), "e1");

                // Trocar o estúdio vai num `PATCH` só com esse campo.
                tela.mudar_estudio("e2".into(), cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let mudancas = publicador.atualizacoes();
        assert_eq!(mudancas.len(), 1);
        assert_eq!(
            mudancas[0].1.estudio_id.as_ref(),
            Some(&Some("e2".to_string()))
        );
        assert!(
            mudancas[0].1.titulo.is_none() && mudancas[0].1.email.is_none(),
            "ausente não é nulo: o PATCH leva só o que mudou"
        );
    }

    /// 🧾 **A faixa e o preço de venda se mudam no painel, como no site.**
    ///
    /// Eles não existiam no app: a tela mandava ao Balcão, que é onde se
    /// registra o que **entrou no balcão** — outra conta. A faixa decide o preço
    /// da foto na galeria do cliente, e o preço de venda o substitui; sem os
    /// dois, uma leva que subiu na faixa errada só se conserta pelo site.
    #[gpui::test]
    fn o_painel_muda_a_faixa_e_o_preco_de_venda(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(
            cx,
            vec![foto("f1", EstadoDaFotoNoSite::Disponivel, Some(4))],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.clicar(0, Modificadores::default(), cx);
                tela.mudar_faixa("p2".into(), cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let negociadas = publicador.negociadas();
        assert_eq!(negociadas.len(), 1);
        assert_eq!(negociadas[0].0, "f1");
        assert_eq!(
            negociadas[0].1.produto_id.as_ref(),
            Some(&Some("p2".to_string())),
            "a faixa escolhida tinha de ir no PATCH"
        );

        // E o preço de venda sai como decimal com ponto, que é o que a API lê.
        janela
            .update(cx, |tela, window, cx| {
                tela.mudando = 0;
                tela.preparar_painel(window, cx);
                if let Some(campos) = tela.campos_do_painel.as_ref() {
                    campos
                        .preco
                        .update(cx, |campo, cx| campo.set_value("19,90", window, cx));
                }
                tela.aplicar_preco_de_venda(cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let negociadas = publicador.negociadas();
        assert_eq!(
            negociadas.last().map(|(_, m)| m.preco_de_venda.clone()),
            Some(Some(Some("19.90".to_string()))),
            "o preço fixado tinha de ir como 19.90"
        );
    }

    /// 🚨 **A grade mostra a foto revelada pela receita padrão, e não o bruto.**
    ///
    /// O operador escolhe a predefinição e a proporção na etapa 2, vê as
    /// miniaturas mudarem — e ao entrar na sessão via os brutos de volta. Eram
    /// **dois caches diferentes**: o serviço gravava a revelada com
    /// `save_preview` (`Large`) e a grade procurava por `get_thumbnail`
    /// (`Thumbnail`), na chave do bruto. Achado do dono, 17/set/2026.
    #[gpui::test]
    fn a_grade_mostra_a_revelada_da_receita_padrao(cx: &mut TestAppContext) {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
        let janela = janela_com_previews(
            cx,
            publicador_com(Vec::new(), false),
            Arc::new(SeletorDeMentira::default()),
            Arc::new(ImportadorDeMentira::default()),
            previews.clone(),
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_locais(vec![local("nova-1")], cx);

                // Sem revelada, a chave é a do bruto.
                tela.resolver_revelada("nova-1");
                assert_eq!(tela.chave_da_foto("nova-1"), "nova-1");

                // O serviço grava a revelada e avisa.
                let imagem = image::DynamicImage::ImageRgba8(image::RgbaImage::new(8, 8));
                let revelada = crate::revelacao::persistencia::chave_da_revelada("nova-1");
                previews
                    .save_thumbnail(&revelada, &imagem)
                    .expect("gravar a revelada");
                tela.revelada_chegou("nova-1");

                tela.resolver_revelada("nova-1");
                assert_eq!(
                    tela.chave_da_foto("nova-1"),
                    revelada,
                    "a grade tinha de passar a ler a revelada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A foto que subiu revelada troca de miniatura na grade** (dono,
    /// 18/set/2026: *"não travou, mas não fez a atualização das miniaturas"*).
    ///
    /// A miniatura guardada veio de antes do envio. Enquanto a releitura era
    /// `entrar`, ela sumia junto com todas as outras e voltava do site — a
    /// atualização aparecia, ao preço de a galeria piscar em branco. Agora que
    /// a grade fica de pé, quem pede a nova é `revelada_subiu`, **só para esta
    /// foto**.
    #[gpui::test]
    fn a_foto_que_subiu_pede_a_miniatura_de_novo(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(
            cx,
            vec![
                foto("f1", EstadoDaFotoNoSite::Disponivel, None),
                foto("f2", EstadoDaFotoNoSite::Disponivel, None),
            ],
        );
        entrar(cx, &janela);

        // A abertura pediu uma miniatura por foto, e só uma.
        assert_eq!(
            publicador.miniaturas_pedidas(),
            vec!["f1".to_string(), "f2".into()]
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.revelada_subiu("f1", cx);
            })
            .expect("a janela deve estar aberta");
        colher_ate_parar(cx, &janela);

        assert_eq!(
            publicador.miniaturas_pedidas(),
            vec!["f1".to_string(), "f2".into(), "f1".into()],
            "a foto revelada tinha de ser pedida de novo — e só ela"
        );
    }

    /// 🚨 **A foto importada aparece na grade, no recorte "Sem nota".**
    ///
    /// É o que fecha o passo 1: ela fica no SQLite até ser classificada, e entre
    /// um e outro a grade da sessão é o **único** lugar em que ela existe para o
    /// operador. Gravada e invisível é o mesmo desfecho de não ter importado —
    /// e foi o que a tela mostrou no dia em que a importação subia direto:
    /// *"Nenhuma foto nesta sessão ainda"*, com 21 arquivos no disco.
    #[gpui::test]
    fn a_foto_importada_entra_na_grade_sem_nota(cx: &mut TestAppContext) {
        let (janela, _) = janela(
            cx,
            vec![foto("no-site", EstadoDaFotoNoSite::Disponivel, Some(4))],
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(tela.acervo.total_visivel(), 1, "só a do site, por enquanto");

                tela.definir_locais(vec![local("nova-1"), local("nova-2")], cx);

                assert_eq!(
                    tela.acervo.total_visivel(),
                    3,
                    "as importadas entraram na grade"
                );
                let contagens = tela.contagens();
                assert_eq!(
                    contagens.de(Filtro::SemNota),
                    2,
                    "a importada nasce sem nota — é o recorte de onde ela é classificada"
                );
                assert_eq!(
                    contagens.de(Filtro::Situacao(acervo::Estado::Disponivel)),
                    1,
                    "e não entra em 'à venda': quem está à venda é quem subiu"
                );

                // 🔑 **No fim da lista**: quem importou 500 quer vê-las onde as
                // deixou, e a ordem da grade é a ordem da sessão.
                tela.filtrar(Filtro::Todas, cx);
                let ids: Vec<String> = tela.acervo.todas().iter().map(|f| f.id.clone()).collect();
                assert_eq!(ids, vec!["no-site", "nova-1", "nova-2"]);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Classificar a foto importada é o que a manda para o site — passo 3.**
    ///
    /// ⚠️ **E sinalizar "levada" nela, não.** A negociação do balcão se grava na
    /// foto **do site**, e uma que nunca subiu não tem em qual linha ser
    /// gravada: mandar `negociar` com o id local devolveria erro para todas. O
    /// que este teste segura é que a tela **diz isso**, em vez de não fazer nada
    /// — o silêncio é a pior resposta possível a um gesto que o operador acabou
    /// de fazer com o cliente ao lado.
    #[gpui::test]
    fn classificar_a_importada_pede_o_passo_3_e_sinalizar_avisa(cx: &mut TestAppContext) {
        let (janela, publicador) = janela(cx, Vec::new());
        entrar(cx, &janela);

        let tela = janela.root(cx).expect("a raiz da janela");
        let pedidos = Arc::new(std::sync::Mutex::new(Vec::<(Vec<String>, i32)>::new()));
        let _inscricao = cx.update({
            let pedidos = pedidos.clone();
            move |cx| {
                cx.subscribe(&tela, move |_tela, pedido: &Pedido, _cx| {
                    if let Pedido::Classificar { ids, nota } = pedido {
                        pedidos
                            .lock()
                            .expect("os pedidos")
                            .push((ids.clone(), *nota));
                    }
                })
            }
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_locais(vec![local("nova-1"), local("nova-2")], cx);
                tela.selecionar_tudo(cx);
                tela.dar_nota(4, cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert_eq!(
            *pedidos.lock().expect("os pedidos"),
            vec![(vec!["nova-1".to_string(), "nova-2".into()], 4)],
            "classificar a importada não pediu o passo 3 à raiz"
        );
        assert!(
            publicador.negociadas().is_empty(),
            "a foto local não tem linha no site: negociar com o id local daria erro"
        );

        // O mesmo gesto, mas de balcão: a tela recusa e diz por quê.
        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar_tudo(cx);
                tela.alternar_levada(cx);
                assert!(
                    tela.erro
                        .as_deref()
                        .is_some_and(|e| e.contains("ainda não subiram")),
                    "sinalizar uma foto que não subiu não pode falhar em silêncio: {:?}",
                    tela.erro
                );
            })
            .expect("a janela deve estar aberta");
        assert!(publicador.negociadas().is_empty());
    }

    /// 🚨 **O arquivo é copiado para a pasta do ensaio — nunca catalogado no
    /// cartão.**
    ///
    /// Regra do dono, 8/set/2026: *"precisa ser para pasta padrão, pois o
    /// usuário pode usar um cartão de memória e seria perigoso para a operação
    /// de carga e descarga de fotos, e precisa estar numa pasta de forma
    /// previsível"*.
    ///
    /// O perigo é concreto e não avisa: com `ImportMode::Add` o catálogo
    /// guardaria `/Volumes/NIKON D750/DCIM/…`, e a foto sumiria do app no
    /// instante em que o cartão saísse — no meio da sessão, com o cliente na
    /// frente. Formatar o cartão para o próximo ensaio apagaria o anterior.
    ///
    /// ⚠️ **Os padrões do `ImportOptions` são o oposto do previsível**, e é por
    /// isso que este teste afirma cada um: `ByDate` espalharia o ensaio por
    /// `YYYY/MM/DD` da EXIF, e `Standard` renomearia para
    /// `photo-2026-09-08-001.jpg` — um nome que ninguém procura e que também não
    /// é estável.
    #[gpui::test]
    fn a_importacao_copia_para_a_pasta_previsivel_do_ensaio(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::escolhe(&[
            "/Volumes/NIKON D750/DCIM/DSC_2571.jpg",
        ]));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela_completa(
            cx,
            publicador_com(Vec::new(), false),
            seletor,
            importador.clone(),
        );
        entrar(cx, &janela);

        clicar(cx, &janela, "detalhe-importar");
        colher_ate_parar(cx, &janela);

        let (_, opcoes) = importador.importados().remove(0);
        assert_eq!(
            opcoes.mode,
            ImportMode::Copy,
            "catalogar no cartão faz a foto sumir quando ele sai"
        );
        assert_eq!(
            opcoes.organization,
            OrganizationStrategy::IntoOneFolder,
            "por data, um ensaio de dois dias vira duas pastas"
        );
        // 🚨 **UUID no disco desde 8/set/2026** — e o operador continua vendo
        // `DSC_2571.jpg`, que agora mora em `photos.nome_original`
        // (migration 022). Com `KeepOriginal`, dois cartões com a mesma
        // `DSC_2571.jpg` no mesmo ensaio faziam a segunda virar `DSC_2571_1.jpg`
        // — um nome que não existe em lugar nenhum além do nosso disco.
        assert_eq!(opcoes.rename_pattern, RenamePattern::Uuid);

        let destino = std::path::PathBuf::from(opcoes.destination.expect("a pasta do ensaio"));
        assert!(
            destino.starts_with(infrastructure::paths::AppPaths::catalog_root()),
            "a pasta tem de ficar dentro do catálogo: {destino:?}"
        );
        assert!(
            destino.ends_with("Ensaios/Ensaio - g1"),
            "a pasta tem de ser previsível pelo ensaio: {destino:?}"
        );
    }

    /// ⚠️ **O título do ensaio vira nome de pasta, e nem todo título pode.**
    ///
    /// Uma barra em "Ana / Bruno" criaria uma subpasta que ninguém pediu — e as
    /// fotos do ensaio ficariam num lugar diferente do que a regra promete. Dois
    /// pontos quebram o caminho no macOS, e ponto no fim vira outro nome no
    /// Windows.
    #[test]
    fn o_titulo_do_ensaio_vira_pasta_sem_quebrar_o_caminho() {
        assert!(pasta_do_ensaio("Ana / Bruno", "g1").ends_with("Ensaios/Ana - Bruno - g1"));
        assert!(pasta_do_ensaio("15:30 · praia", "g2").ends_with("Ensaios/15-30 - praia - g2"));
        assert!(pasta_do_ensaio("Ensaio.", "g3").ends_with("Ensaios/Ensaio - g3"));
        // 🔑 Sem título, o id sozinho — que é o que garante a previsibilidade.
        assert!(pasta_do_ensaio("   ", "g4").ends_with("Ensaios/g4"));
    }

    /// 🚨 **A foto importada não pode aparecer preta na grade.**
    ///
    /// Relatado pelo dono em 8/set/2026, com o app rodando: 21 fotos
    /// importadas, cada célula com nome, "À venda", faixa e contagem de
    /// downloads — e **nenhuma imagem**.
    ///
    /// A causa é uma chave só para dois caches. A miniatura da foto **do site**
    /// é baixada da API e gravada sob `site:<id>`; a da foto **local** é gravada
    /// pelo importador sob o id do catálogo, cru
    /// (`preview_storage.save(&photo.id(), …)`). A grade procurava tudo sob
    /// `site:` — e para a local isso não acha nada.
    ///
    /// ⚠️ **E não falha**: `espiar` devolve `None`, a célula desenha o retângulo
    /// vazio, e o resto da linha continua certo. É o pior formato de defeito —
    /// tudo funciona, menos a única coisa que o operador foi ver.
    #[gpui::test]
    fn a_foto_importada_nao_aparece_preta(cx: &mut TestAppContext) {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
        // A miniatura que o importador gravou: sob o id do catálogo, sem prefixo.
        let imagem = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([200, 30, 30]),
        ));
        previews
            .save_thumbnail("id-do-catalogo", &imagem)
            .expect("gravar a miniatura da importada");

        let janela = janela_com_previews(
            cx,
            publicador_com(Vec::new(), false),
            Arc::new(SeletorDeMentira::default()),
            Arc::new(ImportadorDeMentira::default()),
            previews,
        );
        entrar(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_locais(vec![local("id-do-catalogo")], cx);
                tela.preparar_miniaturas();

                assert!(
                    tela.miniaturas.espiar("id-do-catalogo").is_some(),
                    "a célula da foto importada ficou preta: a grade procurou a \
                     miniatura sob 'site:', onde só mora a do site"
                );
                assert!(
                    tela.miniaturas.espiar("site:id-do-catalogo").is_none(),
                    "a local não mora sob o prefixo do site"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A grade e a tira desenham só o pedaço à vista, e seguem o foco.**
    ///
    /// Com o recorte inteiro montado, "rolar até a foto N" era o GPUI achar o
    /// filho N. Agora as fotos de fora são espaçadores, e a rolagem é conta: a
    /// seta até a última tem de levar as duas até ela, e a volta até a
    /// primeira tem de trazê-las ao começo — com o quadro montando poucas
    /// células, e não 400.
    #[gpui::test]
    fn a_grade_e_a_tira_seguem_o_foco_sem_montar_tudo(cx: &mut TestAppContext) {
        const N: usize = 400;
        let fotos = (0..N)
            .map(|i| foto(&format!("f{i:03}"), EstadoDaFotoNoSite::Disponivel, Some(3)))
            .collect();
        let (janela, _publicador) = janela(cx, fotos);
        entrar(cx, &janela);
        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        let desenhar = |visual: &mut gpui::VisualTestContext| {
            for _ in 0..3 {
                janela
                    .update(visual, |_tela, _w, cx| cx.notify())
                    .expect("a janela deve estar aberta");
                visual.run_until_parked();
            }
        };
        desenhar(&mut visual);

        let conferir = |tela: &Detalhe, posicao: usize| {
            let (de, ate) = tela.grade_desenhada;
            let colunas = tela.colunas_da_grade;
            assert!(
                (de * colunas..ate * colunas).contains(&posicao),
                "a linha da foto {posicao} está entre as desenhadas ({de}..{ate} × {colunas})"
            );
            assert!(
                (ate - de) * colunas < N / 4,
                "a grade montou {} linhas de {}",
                ate - de,
                tela.linhas_da_grade
            );
            let passo = tela.passo_da_grade();
            let topo = (posicao / colunas) as f32 * passo;
            let y = -f32::from(tela.rolagem_da_grade.offset().y);
            let vista = f32::from(tela.rolagem_da_grade.bounds().size.height);
            assert!(
                topo >= y && topo + passo - VAO_DA_GRADE <= y + vista + 0.5,
                "a linha {topo} está à vista em {y}..{}",
                y + vista
            );
            let (de, ate) = tela.tira_desenhada;
            assert!(
                (de..ate).contains(&posicao),
                "a tira desenha a foto {posicao}"
            );
            assert!(ate - de < N / 4, "a tira montou {} de {N}", ate - de);
        };

        janela
            .update(&mut visual, |tela, _w, cx| {
                for _ in 0..N {
                    tela.andar(1, cx);
                }
            })
            .expect("a janela deve estar aberta");
        desenhar(&mut visual);
        janela
            .update(&mut visual, |tela, _w, _cx| {
                assert_eq!(tela.posicao_em_foco(), Some(N - 1));
                assert!(f32::from(tela.rolagem_da_grade.offset().y) < 0.);
                assert!(f32::from(tela.rolagem_da_tira.offset().x) < 0.);
                conferir(tela, N - 1);
            })
            .expect("a janela deve estar aberta");

        janela
            .update(&mut visual, |tela, _w, cx| {
                for _ in 0..N {
                    tela.andar(-1, cx);
                }
            })
            .expect("a janela deve estar aberta");
        desenhar(&mut visual);
        janela
            .update(&mut visual, |tela, _w, _cx| {
                assert_eq!(tela.posicao_em_foco(), Some(0));
                assert_eq!(f32::from(tela.rolagem_da_grade.offset().y), 0.);
                // O `scroll_to_item` do `div` encostava a miniatura na borda,
                // deixando o recuo de fora — a conta faz o mesmo.
                assert_eq!(f32::from(tela.rolagem_da_tira.offset().x), -RECUO_DA_TIRA);
                conferir(tela, 0);
            })
            .expect("a janela deve estar aberta");
    }
}
