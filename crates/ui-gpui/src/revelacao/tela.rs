//! A tela de Revelação: a foto à esquerda, os ajustes à direita.
//!
//! É onde o motor de [`super::processador`] encosta na interface. O caminho de
//! um arrasto:
//!
//! ```text
//! slider → SliderEvent::Change → Ajustes → Pedido → (thread wgpu) → Resultado → RenderImage
//! ```
//!
//! ⚠️ Nenhuma etapa disso acontece no `render`. O `render` só desenha o que já
//! chegou — é o que permite arrastar liso enquanto a GPU trabalha atrás.

use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::services::PreviewType;
use domain::value_objects::CropSettings;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable};
use gpui_kit::AnimationExt;
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, Bounds, Context, Entity, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, RenderImage, SharedString, Subscription, Task, Window,
};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::miniaturas::CacheDeMiniaturas;
use crate::imagem::para_gpui;
use crate::tema;

use super::automatico;
use super::cache::{self, CacheDeReveladas};
use super::controles::{Definicao, CONTROLES};
use super::corte;
use super::fonte;
use super::histograma::Histograma;
use super::historico::{Estado, Historico};
use super::lightroom::{Arquivo, EscolhaDePresets, Relatorio};
use super::persistencia::{self, Corte, Gravador};
use super::presets::GuardaDePresets;
use super::processador::{Ajustes, Pedido, Processador};
use super::reposicao::APor;
use super::sincronizacao::{self, Escolha};
use infrastructure::gpu_adjustments::ParametrosLocais;
use infrastructure::transformacao;

/// O Enquadrar: retângulo, alças, transferidor e o painel dele.
mod comparar;
mod correcao_de_cores;
mod enquadrar;
/// 📜 O painel Histórico, no pé da coluna das predefinições.
mod historico;
/// A Revelação local: máscaras e retoques — ferramentas, gestos e painel.
mod local;
/// O zoom no palco e o Navegador.
mod navegacao;
/// A perspectiva guiada, dentro do Enquadrar.
mod perspectiva;
/// A coluna das predefinições.
mod predefinicoes;
/// O bruto em resolução cheia quando o zoom passa da cópia.
mod resolucao;
// 🔁 A caixa do "Sincronizar N": as flags, em cartões.
mod sincronizar;
pub use local::Ferramenta;
#[cfg(test)]
pub use local::VistaLocal;

/// "Descartar": a foto volta ao que a galeria do site tem.
mod descartar;
/// "Editar Foto" e a troca da fonte quando uma edição é salva.
mod edicao;
/// A coluna da direita: cabeçalho, abas sRGB/RGB, painéis e gráficos.
pub(crate) mod painel;
/// Os gestos de ponteiro para os cenários de ponta a ponta. Só testes.
#[cfg(test)]
mod para_e2e;
/// A tira do rodapé: recortes, puxador, menu e miniaturas (ver `tira.rs`).
mod tira;
/// A conta do pedaço à vista — a tira da sessão usa a mesma.
pub(crate) use tira::faixa_desenhada;
/// Os chips da tira — o rodapé confere que chama os recortes do mesmo jeito.
#[cfg(test)]
pub(crate) use tira::FILTROS_DA_TIRA;
/// O estresse de dentro da tela: tira grande, sliders, Enquadrar e zoom.
#[cfg(test)]
mod estresse;

use comparar::EmComparacao;
use enquadrar::Edicao;

/// Largura da coluna de ajustes — os `w-80` do site.
const LADO_DO_PAINEL: f32 = 320.0;

/// A coluna das predefinições, à esquerda — os `w-56` do site.
const LADO_DOS_PRESETS: f32 = 224.0;

/// A altura da barra do topo — os `h-12` do site.
const ALTURA_DO_CABECALHO: f32 = 48.0;

/// O lado da miniatura revelada que a Revelação deixa para a grade — o mesmo
/// do serviço da revelação padrão (`sessoes::revelacao_padrao`), porque é a mesma
/// chave de cache e a mesma célula desenhando.
const LADO_DA_REVELADA: u32 = 320;

/// Quantas miniaturas da tira ficam em memória.
///
/// A tira mostra o acervo **inteiro**, mas quem paga por quadro é só o que está
/// à vista; este é o teto do que fica guardado. Uma miniatura de 320px em BGRA
/// ocupa ~300 KB, então 512 são ~150 MB no pior caso — e o pior caso é ter
/// rolado a tira inteira de um acervo grande.
const MINIATURAS_DA_TIRA: usize = 512;

/// Até onde a varredura de reposição olha, para cada lado do palco.
///
/// 🚨 **Não é um limite de qualidade, é um limite de custo por tecla.** A
/// varredura roda a cada troca de foto, na thread da interface, e pergunta duas
/// coisas ao cache por foto: sem teto, um acervo de duas mil fotos seriam
/// quatro mil consultas por seta apertada. Sessenta para cada lado cobrem umas
/// seis telas de tira — muito além do que se vê — e custam cerca de um
/// milissegundo.
///
/// ⚠️ **Nada fica para trás por causa dele.** A varredura recomeça no palco
/// novo a cada troca, então a foto que ficou fora do alcance entra assim que a
/// seta chegar perto dela.
const RAIO_DA_REPOSICAO: usize = 60;

/// Quantas fotos vão num pedido de reposição.
///
/// Cerca de uma tira cheia. Mandar mais não adianta: quem repõe entrega uma por
/// vez, e as do fim da fila esperariam o mesmo tanto — só que **enfileiradas**,
/// e portanto sem poder ceder a vez para a foto que a seta abrir no meio do
/// caminho.
const A_REPOR_POR_VEZ: usize = 24;

/// Quanto a gravação espera depois do último movimento de slider.
///
/// Os mesmos 500 ms do legado (`AUTO_SAVE_DEBOUNCE_MS`, `app.rs`), e a razão é a
/// mesma: um arrasto emite dezenas de `Change` por segundo, e gravar cada um
/// seria dezenas de `UPDATE` de 54 colunas por segundo. A espera é o que
/// transforma um arrasto inteiro em uma gravação só.
///
/// ⚠️ **E ela é uma janela de perda.** Fechar o app dentro dela perde o último
/// ajuste — no legado também. O que fecha as outras portas é gravar na hora ao
/// trocar de foto e ao sair da Revelação, que é o que
/// [`Revelacao::gravar_o_que_estiver_pendente`] faz.
const ESPERA_DA_GRAVACAO: Duration = Duration::from_millis(500);

/// Quanto o slider fica parado para o gesto dar-se por acabado sem `Release`
/// — a roda do mouse e as setas mudam o valor sem soltar nada.
const FIM_DO_GESTO: Duration = Duration::from_millis(150);

/// Até onde o slider de endireitamento vai, em graus.
const ANGULO_MAXIMO: f32 = corte::ANGULO_MAXIMO;

/// Quanto dura o cruzamento entre a foto de antes e a de depois.
///
/// 🚨 **Troca seca de imagem lê-se como engasgo, não como resultado.** É UX
/// antes de ser desempenho, e é o que sobrou depois de o quadro estar medido em
/// 3,94 ms e o passo dos sliders consertado: aplicar uma predefinição, desfazer
/// ou zerar no duplo clique substitui a foto inteira de um quadro para o outro,
/// e o olho registra um salto — que ele lê como travada. Com o cruzamento, a
/// mesma troca lê-se como a revelação **acontecendo**.
///
/// 🔑 **140 ms é a faixa em que o olho vê a passagem sem esperar por ela.** Mais
/// curto não se distingue de um corte; mais longo atrapalha quem passa trinta
/// fotos.
///
/// ⚠️ **Não vale para o arrasto do slider.** Ali o resultado chega a cada poucos
/// milissegundos e cruzar dois quadros deixaria fantasma — pior do que o corte,
/// e justamente no gesto onde a resposta imediata é o valor.
const CRUZAMENTO_DA_FOTO: Duration = Duration::from_millis(140);

/// De quanto em quanto a tela pergunta se a GPU já respondeu.
///
/// Metade de um quadro a 60fps. Mais curto gastaria acordada à toa; mais longo
/// somaria latência visível ao arrasto, que é o único momento em que isto
/// importa. E o laço **só existe enquanto há pedido pendente** — parado, a tela
/// não acorda nenhuma vez.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(8);

/// O que a barra mostra no botão "Salvar na galeria e sair".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotaoDeSalvar {
    /// O texto do botão — ou o andamento do lote, enquanto ele sobe.
    pub rotulo: String,
    /// A dica, que diz **por que** ele está apagado quando está.
    pub dica: String,
    /// Aceso?
    pub habilitado: bool,
}

pub struct Revelacao {
    previews: Arc<PreviewManager>,
    gravador: Arc<dyn Gravador>,
    /// As colunas no dock: as predefinições à esquerda e os ajustes à direita,
    /// puxadas pela borda e recolhidas pelas setas (`crate::docas`). Nasce no
    /// primeiro quadro, quando a tela já existe para os painéis apontarem.
    docas: Option<crate::docas::Docas>,
    /// A lista que a Biblioteca estava mostrando, para as setas e o filmstrip.
    /// `Arc` porque o closure do filmstrip a leva consigo.
    acervo: Arc<Vec<PhotoViewModel>>,
    /// As miniaturas da tira, convertidas uma vez cada.
    ///
    /// 🚨 **A tira chamava `get_thumbnail` + `para_gpui` por item, por quadro** —
    /// o mesmo defeito que a grade da sessão tinha (`docs/08-CACHE-ARCHITECTURE.md`,
    /// 6/set/2026). Com 15 itens já eram 15 decodes por quadro; com o acervo
    /// inteiro seriam todos.
    ///
    /// 🔑 **E é ele que torna a tira completa possível.** A faixa mostrava só
    /// ±7 vizinhas, e o comentário explicava por quê: *"uma faixa com 2.000
    /// itens custaria 2.000 consultas ao cache por quadro"*. Com o cache, custa
    /// uma leitura de memória por item — a razão caiu.
    miniaturas_da_tira: CacheDeMiniaturas,
    /// A rolagem da tira, para ela seguir a foto aberta.
    rolagem_da_tira: gpui_kit::ScrollHandle,
    /// A posição que a tira já mostrou — só rola quando muda.
    ultima_na_tira: Option<usize>,
    /// Onde estamos nela.
    posicao: usize,
    /// As marcadas na tira — o lote da sincronização, em posições do acervo.
    ///
    /// 🔑 **Seleção não é a foto aberta.** No Lightroom são coisas separadas:
    /// uma está no canvas, várias estão marcadas, e o "sincronizar" leva os
    /// ajustes da primeira para as outras. É o desenho do site, e a aberta
    /// está sempre dentro (`sincronizacao::alternar`).
    marcadas: BTreeSet<usize>,
    /// O que sincronizar. `None` = ainda não lida do disco — a leitura fica
    /// para a primeira abertura da caixa, para os testes nunca tocarem o
    /// arquivo de quem trabalha.
    escolha: Option<Escolha>,
    processador: Processador,
    aberta: Option<Aberta>,
    ajustes: Ajustes,
    /// O corte que veio com a foto, guardado para ser **devolvido** na gravação.
    /// A Revelação nova ainda não sabe cortar; se ela gravasse `None` aqui,
    /// mexer num slider apagaria o enquadramento feito no app de egui.
    corte: Corte,
    /// A Revelação local da foto aberta: máscaras e retoques. É parte do
    /// [`Estado`] — o histórico e a gravação a carregam como o corte.
    locais: Arc<ParametrosLocais>,
    /// A revelação local da foto aberta não pôde ser lida (versão mais nova do
    /// app, ou corrompida): a tela mostra a foto sem ela e **não grava por
    /// cima** — o texto fica intacto para quem souber lê-lo.
    locais_ilegiveis: Option<String>,
    /// A ferramenta, o gesto em curso e os sliders da Revelação local.
    local: local::Local,
    /// O foco da foto. 🔑 **Clicar na foto tira o foco de qualquer campo de
    /// texto**, como em todo editor: com o cursor esquecido na busca de
    /// predefinições, `[`, `]`, `K`… eram texto e não atalho.
    foco_do_palco: gpui_kit::FocusHandle,
    /// Os passos de desfazer, **por foto**: trocar de foto troca de histórico.
    /// Um `Cmd+Z` que atravessasse fotos aplicaria a revelação de uma na
    /// outra — que é o mesmo defeito que a cópia da seleção já impede. Cada
    /// foto guarda o seu no catálogo (`tela/historico.rs`).
    historico: Historico,
    /// 📜 A leitura do histórico gravado da foto aberta, em curso — a geração
    /// que ela responde. Enquanto houver, **nada se grava**: o histórico curto
    /// desta abertura passaria por cima do gravado antes de ele chegar.
    historico_por_ler: Option<u64>,
    /// Cresce a cada abertura de foto: a leitura que voltar com outra geração
    /// é de uma foto que já saiu.
    geracao_do_historico: u64,
    _leitura_do_historico: Option<Task<()>>,
    /// 📜 O passo do histórico sob o ponteiro: a foto o mostra sem mudar nada,
    /// como a prévia de uma predefinição.
    previa_do_passo: Option<usize>,
    /// Se há ajuste que ainda não foi gravado. Um `bool`, e não "a tarefa existe":
    /// a tarefa continua existindo depois de terminar, e trocar de foto gravaria
    /// de novo o que já estava no banco.
    pendente: bool,
    /// As fotos que **esta abertura** gravou — um ajuste, uma predefinição, um
    /// "Sincronizar". Recomeça a cada `abrir_no_acervo`.
    ///
    /// 🔑 **É o que separa o que o operador fez do que a tira só guarda.** A
    /// cópia de cada foto na tira é um retrato de quando a Revelação abriu: a
    /// revelação padrão da sessão, aplicada depois, não chega a ela. Quem lê a
    /// tira para levar ao site o que mudou (`trazer_da_revelacao_as_que_subiram`)
    /// levaria o retrato velho por cima da revelação nova — e zeraria no site uma
    /// foto que ninguém tocou (visto rodando o app, 24/set/2026).
    gravadas: std::collections::HashSet<String>,
    /// A revelação de cada foto **antes** do primeiro gesto desta abertura sobre
    /// ela — a base da mescla quando a mesma foto muda por fora (a revelação
    /// padrão da sessão, gravada em segundo plano). Ver
    /// [`Self::parametros_mudaram_por_fora`].
    bases: std::collections::HashMap<String, persistencia::Parametros>,
    /// A revelação da foto aberta mudou por fora e os sliders ainda mostram a de
    /// antes: quem os acerta é o `render`, que tem a janela.
    sliders_atrasados: bool,
    /// A espera do próximo salvamento. Guardada porque **descartá-la cancela** —
    /// é assim que cada movimento novo do slider adia a gravação em vez de
    /// enfileirar mais uma.
    _gravacao: Option<Task<()>>,
    controles: Vec<Controle>,
    /// A fase do gesto `revelacao varrer` do roteiro de estresse.
    varredura: u32,
    /// A fase do gesto `revelacao misturar`.
    mistura: u32,
    /// O corte em edição. `None` é "fora do modo de corte" — e é a diferença
    /// entre a foto com overlay por cima e a foto sozinha.
    edicao: Option<Edicao>,
    /// Se a tela está mostrando o "antes" — a foto sem nenhum ajuste, no mesmo
    /// enquadramento. É o `\\` do legado.
    mostrando_original: bool,
    /// Quantas fotos deste ensaio têm revelação nova que o site ainda não recebeu.
    ///
    /// 🔑 **Quem conta é a raiz** — a fila de envio é dela (`a_subir` e o
    /// depósito), e esta tela só a mostra. Existe porque desde 2026-09-11 o
    /// "Sincronizar" copia parâmetros sem subir nada: sem um número no botão de
    /// salvar, o operador sairia do ensaio achando que o cliente já está vendo o
    /// que ele acabou de fazer.
    nao_salvas: usize,
    /// A foto aberta está no depósito, esperando subir — quem sabe é a raiz.
    aberta_no_deposito: bool,
    /// A revelação com que a foto abriu: diferente dela, há o que salvar.
    parametros_ao_abrir: Option<Estado>,
    /// A tela do cliente está aberta? O botão fica âmbar, como no site.
    cliente_aberto: bool,
    /// "Gerando o JPEG…" no botão de baixar.
    /// O histograma da foto **como ela está na tela**. Recalculado junto com a
    /// exibição, e `None` enquanto não há foto.
    histograma: Option<Histograma>,
    /// O slider de endireitamento. Entidade própria, como os 42 do painel.
    angulo: Entity<SliderState>,
    /// Os dois sliders do ajuste fino da perspectiva (graus, nos eixos da tela).
    persp_vertical: Entity<SliderState>,
    persp_horizontal: Entity<SliderState>,
    /// O slider contínuo da barra de zoom (0 a 1; ver `navegacao.rs`).
    trilho_do_zoom: Entity<SliderState>,
    /// O tamanho do palco no último quadro, medido no `canvas`. Sem ele não dá
    /// para converter pixel de ponteiro em fração de foto.
    palco: Bounds<Pixels>,
    /// Os presets, carregados uma vez na abertura do app. Sistema e usuário
    /// juntos, na ordem que o `ListPresetsUseCase` devolve.
    presets: Vec<Preset>,
    guarda_de_presets: Arc<dyn GuardaDePresets>,
    /// O nome digitado no diálogo de salvar preset. Entidade própria, como a
    /// busca da Biblioteca: o `InputState` guarda cursor, seleção e histórico de
    /// edição do campo.
    nome_do_preset: Entity<InputState>,
    /// O texto digitado na busca de predefinições. Entidade própria, como a
    /// busca da Biblioteca: o `InputState` guarda cursor, seleção e o histórico
    /// de edição do campo.
    busca_de_presets: Entity<InputState>,
    /// Quem abre o seletor do sistema para importar `.lrtemplate` e `.xmp`.
    escolha_de_presets: Arc<dyn EscolhaDePresets>,
    /// Por onde os arquivos escolhidos voltam. O seletor é uma janela do
    /// sistema e responde quando quiser — inclusive nunca, se desistirem.
    arquivos: (
        std::sync::mpsc::Sender<Vec<Arquivo>>,
        std::sync::mpsc::Receiver<Vec<Arquivo>>,
    ),
    /// Se há um seletor aberto — o botão fica desligado enquanto houver.
    escolhendo_arquivos: bool,
    /// O que a última importação aproveitou, enquanto ninguém o fecha.
    relatorio: Option<Relatorio>,
    /// O nome digitado ao renomear. Campo próprio, e não o de salvar: os dois
    /// diálogos guardam coisas diferentes, e um começa preenchido.
    renome_do_preset: Entity<InputState>,
    /// A predefinição sob o ponteiro, enquanto ele está lá.
    ///
    /// 🔑 **Ela muda o que a GPU desenha, e não os ajustes.** Os sliders
    /// continuam mostrando o que a foto tem: a prévia é uma pergunta ("como
    /// ficaria?"), e não uma resposta. Sair com o ponteiro devolve a foto sem
    /// passar pelo histórico — é o gesto do Lightroom, e o do site
    /// (`editor.tsx`: `previa ? {...ajustes, ...previa} : ajustes`).
    previa: Option<Preset>,
    /// 🖱️ A predefinição que acabou de ser aplicada pelo clique, com o ponteiro
    /// ainda em cima: a prévia dela fica suspensa até o ponteiro sair da linha.
    /// Sem isto a prévia voltava a ligar por cima da foto já revelada, e cada
    /// slider mexido depois sumia da tela e da tela do cliente (achado pelo e2e
    /// do ciclo de vida, 03/out/2026).
    previa_suspensa: Option<String>,
    /// Se a próxima predefinição salva guarda os 53 (e não só o que saiu do
    /// neutro) — a caixa "Zerar os outros ajustes ao aplicar" do site.
    preset_inteiro: bool,
    /// O id do pedido que ainda não voltou. `None` é "a tela está em dia".
    aguardando: Option<u64>,
    /// As revelações já feitas, para a volta não custar outra ida à GPU.
    ///
    /// 🔑 **É metade do pedido do dono** (17/set/2026: *"precisa guardar um
    /// cache e fazer uma aplicação antecipada na próxima foto"*). A outra metade
    /// é [`Self::antecipar_a_proxima`]. Ver `revelacao/cache.rs` para a chave.
    reveladas: CacheDeReveladas,
    /// O que cada pedido à GPU está revelando — e se o resultado vai ao palco.
    ///
    /// 🚨 **Sem isto, a revelação antecipada pintaria a foto errada.** Ela é da
    /// foto **seguinte**, e `colher` desenhava qualquer resultado que passasse
    /// do `descartar_ate`. Aqui cada id diz de quem é e para quê.
    pedidos: HashMap<u64, Pendente>,
    /// O Comparar (`⇧C`) em curso — ver `tela/comparar.rs`.
    comparacao: Option<EmComparacao>,
    /// O pedido da outra metade do Comparar que ainda não voltou do motor.
    pedido_do_comparar: Option<u64>,
    /// O pedido especulativo da próxima foto, enquanto ele está no ar.
    ///
    /// Um por vez: dois adiantariam a segunda foto à frente, que o operador
    /// talvez nem alcance, e cada um atrasa o pedido de quem está olhando.
    antecipando: Option<u64>,
    /// Para que lado o operador está andando na tira — é a foto que a
    /// antecipação escolhe. Nasce para a frente, que é como se percorre uma
    /// sessão.
    rumo: i32,
    /// Resultado com id até este é **de outra foto**, e não entra no palco.
    ///
    /// 🚨 Mexer num slider e apertar a seta antes de a GPU responder deixava a
    /// resposta da foto que saiu a caminho; o laço de colheita continuava de pé
    /// e a punha no palco da foto nova — a imagem de uma foto sob o nome de
    /// outra, e numa foto sem ajuste ninguém pedia outra revelação para
    /// corrigir (achado pelo estresse, 17/set/2026).
    descartar_ate: u64,
    /// O enquadramento mudou e a foto na tela ainda não foi refeita.
    ///
    /// 🚨 **O endireitar gira a foto na CPU**, e um evento do slider de ângulo
    /// (ou um passo de 0,1° do transferidor) custava um giro inteiro da cópia
    /// de trabalho: ~50 ms por evento em 2048 px, medido pelo estresse em
    /// 17/set/2026. O ponteiro manda mais eventos que isso por segundo, e a
    /// tela ficava atrás do dedo. Marcado aqui, o giro acontece **uma vez por
    /// quadro**, no `render`, com o ângulo mais recente.
    exibicao_atrasada: bool,
    /// Se já existe um laço de colheita rodando. Sem esta trava, cada arrasto
    /// abriria um laço novo e a tela acabaria com dezenas deles perguntando a
    /// mesma coisa.
    colhendo: bool,
    /// Um slider está sendo arrastado: a foto sai no tamanho do palco.
    ///
    /// 🪟 **É o que o Lightroom faz no arrasto**, e o que o Windows pedia: lá
    /// cada quadro revelado atravessa duas APIs gráficas (o motor e a janela),
    /// e revelar 2560 px para um palco de 1200 era pagar isso em dobro. Acaba no
    /// `Release` do slider, ou em [`FIM_DO_GESTO`] sem movimento — o que vier
    /// primeiro —, e aí a cópia inteira é pedida.
    em_gesto: bool,
    _fim_do_gesto: Option<Task<()>>,
    /// A foto **de antes**, enquanto a de depois entra por cima.
    ///
    /// 🔑 Guardada só nas trocas que valem cruzamento (predefinição, desfazer,
    /// zerar, automático) — ver [`CRUZAMENTO_DA_FOTO`]. `None` é o caso comum: o
    /// arrasto, em que a foto nova substitui a anterior direto.
    ///
    /// ⚠️ **Ela sai sozinha, e é de propósito que ninguém a apague por tempo.**
    /// Guardar uma `Task` para isso acordaria a tela ao fim da animação só para
    /// jogar fora um `Arc`; ela é substituída na troca seguinte, e ocupa uma
    /// imagem de palco — a mesma que o cache já guarda quinze vezes.
    saindo: Option<Arc<RenderImage>>,
    /// Se a **próxima** foto que chegar merece cruzamento.
    ///
    /// 🔑 Ligado pelas trocas discretas (predefinição, desfazer, zerar,
    /// automático) e desligado quando a foto nova é montada. O arrasto do slider
    /// não o liga: ali o resultado chega a cada poucos milissegundos.
    cruzar: bool,
    /// Qual cruzamento é este. **O `ElementId` da animação precisa mudar**, ou o
    /// GPUI reaproveita o estado da anterior e a segunda troca aparece já no
    /// fim — sem erro nenhum, e com a impressão de que o efeito "às vezes não
    /// funciona".
    cruzamento: usize,
    /// 🚨 A tarefa que carrega a tira. **Descartá-la a cancela**, e é de
    /// propósito: trocar de foto começa uma tarefa nova, que recomeça a ordem no
    /// palco novo. A antiga estaria enchendo a tira a partir de onde o operador
    /// não está mais olhando.
    _tira: Option<Task<()>>,
    /// Se alguém está indo buscar os pixels que faltam — do disco ou do site.
    ///
    /// 🔑 **A tela não busca, mas precisa saber que estão buscando.** Sem isto
    /// ela só sabia dizer "não tem preview no cache", e dizia isso durante a
    /// reposição inteira: uma frase de beco sem saída no exato momento em que o
    /// beco estava sendo aberto. Quem lê conclui que não há o que esperar, sai
    /// da foto — e cancela a única coisa que ia resolver.
    repondo: bool,
    _assinaturas: Vec<Subscription>,
    /// O zoom e o navegador (ver `navegacao.rs`).
    navegacao: navegacao::Navegacao,
    /// A coluna das predefinições (ver `predefinicoes.rs`).
    predefinicoes: predefinicoes::Predefinicoes,
    /// O que a coluna da direita lembra (ver `painel.rs`).
    estado_do_painel: painel::EstadoDoPainel,
    /// O recorte, a altura e o menu da tira (ver `tira.rs`).
    tira: tira::EstadoDaTira,
    resolucao: resolucao::Resolucao,
    /// As edições em camadas: qual imagem editada vale para cada foto
    /// (`docs/editor-em-camadas/02-CONTRATO.md`). `None` é o app sem editor —
    /// os testes que não falam dele.
    edicoes: Option<Arc<dyn crate::editor::porta::Edicoes>>,
}

struct Controle {
    definicao: &'static Definicao,
    estado: Entity<SliderState>,
    /// 🎞️ A lista, no controle que é escolha entre nomes (o Estilo da vinheta
    /// pós-corte): é ela que se desenha, e não a barra.
    escolha: Option<Entity<SelectState<Vec<&'static str>>>>,
}

/// Um pedido no motor, e o que fazer com o que voltar dele.
struct Pendente {
    chave: cache::Chave,
    destino: Destino,
    /// Revelado no tamanho do palco durante um arrasto (ver
    /// [`Revelacao::lado_do_rascunho`]): vai à tela, mas **nunca** ao cache
    /// nem à tira — a chave é a da cópia inteira.
    rascunho: bool,
}

/// Para onde vai o que o motor devolver.
enum Destino {
    /// A foto aberta, no palco.
    Palco,
    /// A revelação antecipada da próxima foto: ela só alimenta o cache, e
    /// **nunca** entra no palco — quem está na tela é outra foto.
    Cache,
    /// A outra metade do Comparar (`⇧C`). Também nunca vai ao palco da aberta.
    Comparar { posicao: usize, crop: CropSettings },
}

struct Aberta {
    foto: PhotoViewModel,
    /// A revisão da imagem editada de que `origem` saiu — `fonte::DO_BRUTO`
    /// quando é o bruto. Entra na chave do cache de reveladas (C17).
    fonte: u64,
    /// Os pixels de origem, prontos para subir para a GPU.
    ///
    /// `None` quando o cache não tem nada gravado — não é erro, é foto ainda não
    /// processada. Sem origem não há o que revelar, e os sliders não têm sobre o
    /// que agir.
    origem: Option<Origem>,
    /// A foto como saiu do cache, sem nenhum ajuste. É o "antes" do `\\`.
    bruta: Option<image::DynamicImage>,
    /// O que o shader devolveu (ou a foto do cache, antes do primeiro resultado).
    /// É a foto **inteira**: o corte e o giro entram depois, na exibição.
    revelada: Option<image::DynamicImage>,
    /// A imagem da galeria da foto do site, enquanto a cópia de trabalho não
    /// chega. Vai ao palco **como veio**: o site a entrega já revelada e
    /// enquadrada (C15), e passar o corte por cima dela a recortaria duas vezes.
    ///
    /// 🔑 Só aparece enquanto não há `revelada` — e fica até ela chegar, para a
    /// foto não sumir entre o download e a resposta do motor.
    espera: Option<image::DynamicImage>,
    /// O que está na tela: a revelada depois de espelhada, girada, endireitada e
    /// recortada.
    desenhada: Option<Arc<RenderImage>>,
}

struct Origem {
    /// `Arc` porque é a **identidade** dele que diz à thread se a textura
    /// precisa subir de novo. Um `Arc` novo a cada arrasto reenviaria a foto
    /// inteira para a GPU a cada milímetro de slider.
    pixels: Arc<Vec<u8>>,
    largura: u32,
    altura: u32,
}

impl Revelacao {
    pub fn nova(
        previews: Arc<PreviewManager>,
        gravador: Arc<dyn Gravador>,
        guarda_de_presets: Arc<dyn GuardaDePresets>,
        escolha_de_presets: Arc<dyn EscolhaDePresets>,
        presets: Vec<Preset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut controles = Vec::with_capacity(CONTROLES.len());
        let mut assinaturas = Vec::with_capacity(CONTROLES.len());

        for definicao in CONTROLES {
            let estado = cx.new(|_| {
                SliderState::new()
                    .min(definicao.minimo)
                    .max(definicao.maximo)
                    // 🚨 **Sem isto o passo é 1,0** — o padrão do
                    // `gpui-component`, que arredonda o valor a ele. A exposição
                    // tinha onze posições de −5 a +5 e o contraste três; ver
                    // `Definicao::passo`.
                    .step(definicao.passo())
                    .default_value(definicao.neutro())
            });

            // Uma assinatura por controle, e todas guardadas: `Subscription`
            // descartada cancela a inscrição na hora, e o slider passaria a se
            // mover sem mover a foto — sem erro nenhum.
            assinaturas.push(cx.subscribe_in(
                &estado,
                window,
                move |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                    // O `Release` (novo no gpui-kit 0.6) chega depois do último `Change`
                    // com o mesmo valor: tratá-lo gravaria duas vezes.
                    let SliderEvent::Change(valor) = evento else {
                        tela.fim_do_gesto(cx);
                        return;
                    };
                    crate::desempenho::operacao(crate::desempenho::Operacao::ArrastoDeSlider);
                    (definicao.aplicar)(&mut tela.ajustes, valor.start());
                    tela.gesto_do_slider(cx);
                    tela.pedir_revelacao(cx);
                    tela.adiar_gravacao(cx);
                },
            ));

            let escolha = definicao.opcoes.map(|opcoes| {
                let lista = cx.new(|cx| SelectState::new(opcoes.to_vec(), None, window, cx));
                let neutro = opcoes[definicao.neutro() as usize];
                lista.update(cx, |l, cx| l.set_selected_value(&neutro, window, cx));
                // A escolha é um gesto inteiro, como um clique: entra no
                // histórico e grava na hora.
                assinaturas.push(cx.subscribe_in(
                    &lista,
                    window,
                    move |tela: &mut Self,
                          _lista,
                          evento: &SelectEvent<Vec<&'static str>>,
                          window,
                          cx| {
                        let SelectEvent::Confirm(Some(nome)) = evento else {
                            return;
                        };
                        let Some(i) = opcoes.iter().position(|o| o == nome) else {
                            return;
                        };
                        tela.gesto_discreto(|a| (definicao.aplicar)(a, i as f32), window, cx);
                    },
                ));
                lista
            });

            controles.push(Controle {
                definicao,
                estado,
                escolha,
            });
        }

        // A busca de predefinições. A tela não lê o campo a cada quadro: ela é
        // avisada quando o texto muda.
        // Os campos de nome das predefinições: o de criar e o de renomear.
        let nome_do_preset =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nome da predefinição"));
        let renome_do_preset = cx.new(|cx| InputState::new(window, cx));
        assinaturas.extend(predefinicoes::assinar(
            &nome_do_preset,
            &renome_do_preset,
            window,
            cx,
        ));
        predefinicoes::ligar_atalhos(cx);

        let busca_de_presets = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Buscar predefinição")
                // Esc limpa o campo, como na Biblioteca: desfazer uma busca não
                // pode custar apagar caractere por caractere.
                .clean_on_escape()
        });
        assinaturas.push(cx.subscribe(
            &busca_de_presets,
            |_tela: &mut Self, _campo, evento: &InputEvent, cx| {
                // Só `Change`. `Focus` e `Blur` também chegam aqui.
                if matches!(evento, InputEvent::Change) {
                    cx.notify();
                }
            },
        ));

        // O slider de endireitamento nasce junto com os outros, mas fora da
        // tabela: ele não escreve em `Ajustes` — escreve no corte, que é outro
        // caminho e outro dono.
        // Os sliders da Revelação local (tamanho, suavização, fluxo, exposição).
        let local = local::Local::novo(window, cx, &mut assinaturas);

        let angulo = cx.new(|_| {
            SliderState::new()
                .min(-ANGULO_MAXIMO)
                .max(ANGULO_MAXIMO)
                .step(enquadrar::PASSO_DO_ANGULO)
                .default_value(0.0)
        });
        assinaturas.push(cx.subscribe_in(
            &angulo,
            window,
            move |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                // O `Release` (novo no gpui-kit 0.6) chega depois do último `Change`
                // com o mesmo valor: tratá-lo gravaria duas vezes.
                let SliderEvent::Change(valor) = evento else {
                    return;
                };
                crate::desempenho::operacao(crate::desempenho::Operacao::ArrastoDeSlider);
                tela.angulo_do_slider(valor.start(), cx);
            },
        ));

        // O ajuste fino da perspectiva: dois sliders, mesmo caminho do ângulo.
        let slider_da_perspectiva = |cx: &mut Context<Self>| {
            cx.new(|_| {
                SliderState::new()
                    .min(-revelacao_core::perspectiva::AJUSTE_MAXIMO)
                    .max(revelacao_core::perspectiva::AJUSTE_MAXIMO)
                    .step(perspectiva::PASSO_DO_AJUSTE)
                    .default_value(0.0)
            })
        };
        let persp_vertical = slider_da_perspectiva(cx);
        let persp_horizontal = slider_da_perspectiva(cx);
        for (estado, vertical) in [(&persp_vertical, true), (&persp_horizontal, false)] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                    let SliderEvent::Change(valor) = evento else {
                        return;
                    };
                    tela.ajuste_do_slider(vertical, valor.start(), cx);
                },
            ));
        }

        // O slider da barra de zoom: a posição dele é a vista, sincronizada
        // no `render`; o arrasto vira escala.
        let trilho_do_zoom = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.001)
                .default_value(0.0)
        });
        assinaturas.push(cx.subscribe_in(
            &trilho_do_zoom,
            window,
            |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                let SliderEvent::Change(valor) = evento else {
                    return;
                };
                tela.zoom_pelo_slider(valor.start(), cx);
            },
        ));

        Self {
            previews,
            gravador,
            docas: None,
            acervo: Arc::new(Vec::new()),
            miniaturas_da_tira: CacheDeMiniaturas::nova(
                NonZeroUsize::new(MINIATURAS_DA_TIRA).expect("não é zero"),
            ),
            rolagem_da_tira: gpui_kit::ScrollHandle::new(),
            ultima_na_tira: None,
            posicao: 0,
            marcadas: BTreeSet::new(),
            escolha: None,
            processador: Processador::novo(),
            aberta: None,
            ajustes: Ajustes::default(),
            corte: Corte::default(),
            locais: Arc::default(),
            locais_ilegiveis: None,
            local,
            foco_do_palco: cx.focus_handle(),
            historico: Historico::novo(Estado::default()),
            historico_por_ler: None,
            geracao_do_historico: 0,
            _leitura_do_historico: None,
            previa_do_passo: None,
            pendente: false,
            gravadas: std::collections::HashSet::new(),
            bases: std::collections::HashMap::new(),
            sliders_atrasados: false,
            _gravacao: None,
            controles,
            varredura: 0,
            mistura: 0,
            edicao: None,
            mostrando_original: false,
            nao_salvas: 0,
            aberta_no_deposito: false,
            parametros_ao_abrir: None,
            cliente_aberto: false,
            histograma: None,
            angulo,
            persp_vertical,
            persp_horizontal,
            trilho_do_zoom,
            palco: Bounds::default(),
            guarda_de_presets,
            nome_do_preset,
            presets,
            busca_de_presets,
            escolha_de_presets,
            arquivos: std::sync::mpsc::channel(),
            escolhendo_arquivos: false,
            relatorio: None,
            renome_do_preset,
            previa: None,
            previa_suspensa: None,
            preset_inteiro: false,
            aguardando: None,
            reveladas: CacheDeReveladas::default(),
            pedidos: HashMap::new(),
            comparacao: None,
            pedido_do_comparar: None,
            antecipando: None,
            rumo: 1,
            descartar_ate: 0,
            exibicao_atrasada: false,
            colhendo: false,
            em_gesto: false,
            _fim_do_gesto: None,
            saindo: None,
            cruzar: false,
            cruzamento: 0,
            _tira: None,
            repondo: false,
            _assinaturas: assinaturas,
            navegacao: navegacao::Navegacao::default(),
            predefinicoes: predefinicoes::Predefinicoes::default(),
            estado_do_painel: painel::EstadoDoPainel::default(),
            tira: tira::EstadoDaTira::novo(),
            resolucao: Default::default(),
            edicoes: None,
        }
    }

    /// Abre uma foto para revelar, com a revelação que ela já tinha.
    ///
    /// ⚠️ **Lê e decodifica na thread da interface.** Um preview "Large" é um
    /// JPEG de alguns milissegundos, então isto não trava de forma perceptível —
    /// mas quando a Revelação passar a carregar o RAW em resolução plena, este é
    /// o ponto que tem de virar assíncrono.
    /// Abre uma foto solta — um acervo de uma.
    pub fn abrir(&mut self, foto: PhotoViewModel, window: &mut Window, cx: &mut Context<Self>) {
        self.abrir_no_acervo(vec![foto], 0, window, cx);
    }

    /// Entra na Revelação com a lista que a Biblioteca estava mostrando.
    ///
    /// 🔑 **A lista vem junto porque revelar é uma sequência.** Quem revela um
    /// casamento passa foto a foto pela seta; ter de voltar à grade a cada uma
    /// transforma 200 ajustes em 400 trocas de tela. É a mesma lista filtrada
    /// que a Impressão recebe, e pela mesma razão: "a próxima" quer dizer a
    /// próxima das que se estava vendo.
    pub fn abrir_no_acervo(
        &mut self,
        acervo: Vec<PhotoViewModel>,
        posicao: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if acervo.is_empty() {
            return;
        }
        self.posicao = posicao.min(acervo.len() - 1);
        self.acervo = Arc::new(acervo);
        // Posições novas: o par do Comparar não aponta mais para as mesmas fotos.
        self.comparacao = None;
        self.pedido_do_comparar = None;
        self.gravadas.clear();
        self.bases.clear();
        // Acervo novo, posições novas: o lote antigo não aponta para nada.
        self.marcadas = sincronizacao::so(self.posicao);
        self.ultima_na_tira = None;
        self.mostrar_a_posicao(window, cx);
    }

    /// Um passo na lista, sem dar a volta — a mesma regra das setas da grade.
    ///
    /// ⚠️ **Trocar de foto aqui grava a anterior**, pelo caminho de sempre: é a
    /// primeira coisa que `mostrar_a_posicao` faz. Sem isso, andar meio segundo
    /// depois de mexer num slider deixaria a gravação atrasada sair com os
    /// ajustes já substituídos.
    pub fn andar(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        // No Comparar, as setas trocam a outra foto, pulando a escolhida.
        if self.comparacao.is_some() {
            self.andar_no_comparar(passo, cx);
            return;
        }
        // 🔑 **A seta anda sobre a tira**, e não sobre o acervo: com um recorte
        // aceso, "a próxima" é a próxima que se vê (`naTira` do site).
        if let Some(nova) = tira::vizinha(&self.na_tira(), self.posicao, passo) {
            // Para onde ele foi é para onde ele provavelmente vai de novo — e é
            // isso que a revelação antecipada adivinha (`antecipar_a_proxima`).
            if passo != 0 {
                self.rumo = passo.signum();
            }
            self.posicao = nova;
            self.mostrar_a_posicao(window, cx);
        }
    }

    /// Vai direto para uma posição — o clique no filmstrip.
    pub fn ir_para(&mut self, posicao: usize, window: &mut Window, cx: &mut Context<Self>) {
        if posicao >= self.acervo.len() || posicao == self.posicao {
            return;
        }
        self.rumo = if posicao > self.posicao { 1 } else { -1 };
        self.posicao = posicao;
        self.mostrar_a_posicao(window, cx);
    }

    pub fn posicao(&self) -> usize {
        self.posicao
    }

    /// A revelação de uma foto da tira mudou **por fora** — a revelação padrão da
    /// sessão, gravada em segundo plano depois de a tira abrir.
    ///
    /// 🚨 **A cópia da tira é um retrato da abertura.** Sem isto, a foto aberta
    /// continuava nos sliders de antes, e o primeiro ajuste gravava a foto
    /// inteira a partir deles: o contraste 1,25 da sessão voltava a 1,0 (visto
    /// contra a pilha local, 24/set/2026). E se o ajuste veio primeiro, quem
    /// escreveu por último foi a revelação padrão — e a edição sumia.
    ///
    /// 🔑 **A mescla é campo a campo** ([`persistencia::mesclar`]): o que o
    /// operador mudou desde a base fica, e o resto vem de fora. Se o resultado
    /// não é o que o catálogo tem, ele é gravado; se a foto está aberta, os
    /// sliders e a imagem mudam, e o histórico é refeito sobre a revelação nova —
    /// senão o `Cmd+Z` a desfaria.
    pub fn parametros_mudaram_por_fora(
        &mut self,
        id: &str,
        deles: persistencia::Parametros,
        cx: &mut Context<Self>,
    ) {
        let Some(copia) = self.acervo.iter().find(|f| f.id == id) else {
            return;
        };
        let aberta = self.aberta.as_ref().is_some_and(|a| a.foto.id == id);
        let meu = if aberta {
            (self.ajustes, self.corte)
        } else {
            (
                persistencia::da_foto(copia),
                persistencia::corte_da_foto(copia),
            )
        };
        let base = self
            .bases
            .get(id)
            .copied()
            .or_else(|| {
                aberta
                    .then(|| self.parametros_ao_abrir.clone())
                    .flatten()
                    .map(|e| (e.ajustes, e.corte))
            })
            .unwrap_or(meu);
        let mexeu = !persistencia::mesmos_parametros(meu, base);
        let final_ = persistencia::mesclar(base, meu, deles);
        // 🚨 **O que veio de fora, sem o que veio do operador.** O serviço da
        // revelação padrão já mescla com o que o operador gravou (ver
        // `revelacao_padrao::mesclada`), então `deles` pode trazer o ajuste dele.
        // Rebasear o histórico sobre `deles` punha o ajuste dentro do passo
        // zero, e o `⌘Z` não tinha mais o que tirar (achado pelo e2e).
        let fora = persistencia::mesclar(meu, base, deles);

        if mexeu {
            // Daqui em diante, o que o operador mudar é sobre a revelação nova.
            self.bases.insert(id.to_string(), fora);
            if aberta {
                // A revelação local não vem de fora: cada passo guarda a sua.
                self.historico.rebasear(|passo| {
                    let (ajustes, corte) =
                        persistencia::mesclar(base, (passo.ajustes, passo.corte), fora);
                    Estado {
                        ajustes,
                        corte,
                        locais: passo.locais,
                    }
                });
                self.gravar_o_historico();
            }
        } else if aberta {
            // Ninguém tinha mexido: a foto passa a abrir com a revelação nova.
            let estado = Estado {
                ajustes: deles.0,
                corte: deles.1,
                locais: self.locais.clone(),
            };
            self.parametros_ao_abrir = Some(estado.clone());
            // 📜 Com passos de antes (o histórico gravado da foto), a revelação
            // nova vira um passo — recomeçar apagaria o que a foto já passou.
            // Sem eles, é a foto que abre assim.
            if self.historico.passos().len() > 1 {
                self.historico
                    .registrar_como(estado, super::historico::DE_FORA);
                self.gravar_o_historico();
            } else {
                self.historico = Historico::novo(estado);
            }
        }
        if persistencia::mesmos_parametros(final_, meu) {
            return;
        }

        let acervo = Arc::make_mut(&mut self.acervo);
        if let Some(foto) = acervo.iter_mut().find(|f| f.id == id) {
            persistencia::na_foto(foto, final_.0, final_.1);
        }
        if aberta {
            self.ajustes = final_.0;
            self.corte = final_.1;
            if let Some(aberta) = self.aberta.as_mut() {
                persistencia::na_foto(&mut aberta.foto, final_.0, final_.1);
            }
            self.sliders_atrasados = true;
            self.pedir_revelacao(cx);
            self.atualizar_exibicao();
        }
        // O catálogo tem a de fora; a mescla ainda não está lá.
        if !persistencia::mesmos_parametros(final_, deles) {
            self.gravador.gravar(id.to_string(), final_.0, final_.1);
            self.gravadas.insert(id.to_string());
        }
        cx.notify();
    }

    /// As fotos que esta abertura gravou — ver [`Self::gravadas`].
    pub fn gravadas(&self) -> &std::collections::HashSet<String> {
        &self.gravadas
    }

    /// A lista que a tira percorre.
    pub fn acervo(&self) -> &[PhotoViewModel] {
        &self.acervo
    }

    // ------------------------------------------------ o lote da sincronização

    pub fn marcadas(&self) -> &BTreeSet<usize> {
        &self.marcadas
    }

    /// As fotos que vão receber os ajustes desta — a aberta inclusive, na
    /// ordem da tira. Se ela tem ajuste não salvo, sair daqui com as outras
    /// salvas e ela não seria o resultado que o operador viu.
    pub fn alvos_da_sincronizacao(&self) -> Vec<PhotoViewModel> {
        if self.acervo.is_empty() {
            return Vec::new();
        }
        let mut posicoes = self.marcadas.clone();
        posicoes.insert(self.posicao);
        // 🚨 A comprada fica de fora, como no site (`alvosDaSincronizacao`).
        posicoes
            .into_iter()
            .filter_map(|p| self.acervo.get(p).cloned())
            .filter(|foto| !foto.revelacao_travada)
            .collect()
    }

    /// A foto aberta pode ser revelada? A comprada não — o `podeRevelar` do
    /// site.
    pub fn pode_revelar(&self) -> bool {
        self.aberta
            .as_ref()
            .is_some_and(|aberta| !aberta.foto.revelacao_travada)
    }

    /// As **outras** marcadas que o "Zerar tudo" também limpa.
    ///
    /// 🚨 **Só as que têm o que zerar.** Incluir uma que já está no neutro
    /// gravaria no catálogo uma mudança para o mesmo valor, e o "Salvar na
    /// galeria" depois subiria um arquivo por nada. É a mesma regra do
    /// `zerar-em-lote.ts` da web.
    ///
    /// A foto aberta fica de fora: ela é zerada por [`Self::redefinir_ajustes`],
    /// que passa pelo histórico — as outras não têm histórico, e é por isso que
    /// o botão diz quantas vão junto antes do clique.
    pub fn outras_a_zerar(&self) -> Vec<PhotoViewModel> {
        // O "Zerar N fotos" do menu da tira tem alvo próprio (ver `tira.rs`).
        if let Some(ids) = self.zerar_do_menu() {
            return self
                .acervo
                .iter()
                .filter(|f| ids.contains(&f.id))
                .cloned()
                .collect();
        }
        let aberta = self.foto_aberta().map(|f| f.id.clone());
        self.alvos_da_sincronizacao()
            .into_iter()
            .filter(|f| Some(&f.id) != aberta.as_ref())
            .filter(tem_o_que_zerar)
            .collect()
    }

    pub fn escolha_da_sincronizacao(&self) -> Escolha {
        self.escolha.unwrap_or_default()
    }

    pub fn definir_escolha_da_sincronizacao(&mut self, escolha: Escolha) {
        self.escolha = Some(escolha);
    }

    fn escolha_mut(&mut self) -> &mut Escolha {
        self.escolha.get_or_insert_with(Escolha::default)
    }

    /// O enquadramento da foto aberta, como a persistência o guarda.
    pub fn corte(&self) -> Corte {
        self.corte
    }

    /// A raiz avisa quantas revelações deste ensaio ainda não foram ao site.
    /// `outras`: as fotos da sessão, **fora a aberta**, que esperam subir.
    pub fn definir_nao_salvas(
        &mut self,
        outras: usize,
        aberta_no_deposito: bool,
        cx: &mut Context<Self>,
    ) {
        if (self.nao_salvas, self.aberta_no_deposito) != (outras, aberta_no_deposito) {
            self.nao_salvas = outras;
            self.aberta_no_deposito = aberta_no_deposito;
            cx.notify();
        }
    }

    /// A foto aberta tem revelação que a galeria ainda não recebeu — o `sujo &&
    /// podeRevelar` do site.
    pub fn aberta_a_salvar(&self) -> bool {
        self.pode_revelar()
            && (self.aberta_no_deposito
                || self
                    .parametros_ao_abrir
                    .as_ref()
                    .is_some_and(|parametros| *parametros != self.estado()))
    }

    /// Há o que salvar na galeria? Sem isso o botão se apaga.
    pub fn ha_o_que_salvar(&self) -> bool {
        self.aberta_a_salvar() || self.nao_salvas > 0
    }

    /// Guarda no cache a miniatura **revelada** da foto aberta — a prévia local
    /// que a grade e a tira mostram antes de a foto subir.
    ///
    /// # 🚨 Por que a grade não pode se contentar com o servidor
    ///
    /// É o mesmo defeito que a web já pagou (`usar-previas-reveladas.ts`,
    /// 11/set/2026, dono: *"tá deixando as miniaturas e a foto central sem
    /// efeito"*) e que apareceu aqui em 18/set/2026: a grade desenha a foto do
    /// **site**, que só muda quando alguém salva na galeria; a Revelação abre
    /// com a revelação do **banco local**, que muda a cada gesto. Entre um e
    /// outro, a mesma foto tem duas caras — e quem vê as duas conclui,
    /// corretamente, que o sistema está mentindo em alguma delas.
    ///
    /// Aqui a prévia local nasce de `aberta.revelada` (o que o motor devolveu,
    /// com o enquadramento aplicado por cima, como na exportação), nas **duas**
    /// chaves de cache, porque a grade lê a miniatura e o painel lê o preview.
    ///
    /// 🔑 **Revelação neutra apaga a prévia** em vez de gravá-la: depois de um
    /// "Zerar tudo", o certo é voltar a mostrar a do servidor. É o mesmo que a
    /// web faz ao apagar a prévia local quando a foto sobe — a partir daí quem
    /// é mais novo é o site.
    ///
    /// ⚠️ **Sem `revelada` não grava nada.** Antes do primeiro resultado do
    /// motor não há o que guardar, e gravar a bruta aqui seria dizer à grade
    /// que a foto foi revelada assim.
    pub fn guardar_a_revelada_no_cache(&self) {
        let Some(aberta) = self.aberta.as_ref() else {
            return;
        };
        let chave = persistencia::chave_da_revelada(&aberta.foto.id);
        let neutro = self.sem_revelacao() && corte::e_inteiro(&self.enquadramento());
        // 🚨 **Só a local apaga no neutro.** Na foto do site, sem prévia a tira
        // e a grade voltam à imagem da galeria — que ainda tem a revelação antiga
        // até o "Salvar" (dono, 2026-09-21: zerar não mudava a tira). Ela grava
        // o neutro como qualquer outra revelação, logo abaixo.
        if neutro && !persistencia::so_existe_no_site(&aberta.foto) {
            self.previews.apagar(&chave);
            return;
        }
        let Some(revelada) = aberta.revelada.as_ref() else {
            return;
        };
        let final_ = infrastructure::transformacao::aplicar(revelada, &self.enquadramento(), true);
        let _ = self.previews.save_preview(&chave, &final_);
        let _ = self.previews.save_thumbnail(
            &chave,
            &final_.thumbnail(LADO_DA_REVELADA, LADO_DA_REVELADA),
        );
    }

    /// O botão "Salvar na galeria e sair", como a barra o desenha — e como o
    /// e2e o afirma.
    ///
    /// 🔑 **Uma função só para o que a barra mostra.** O rótulo, a dica e o
    /// aceso/apagado são exatamente os três do editor da web
    /// (`editor.tsx`, o último `<Button>` do cabeçalho): `disabled={!pronto ||
    /// ocupado || !haOQueSalvar}`, a dica dizendo **por que** ele está apagado,
    /// e o rótulo contando o lote enquanto ele sobe. Com os três aqui, o teste
    /// afirma sobre o mesmo que o operador lê.
    pub fn botao_de_salvar(&self) -> BotaoDeSalvar {
        let ha = self.ha_o_que_salvar();
        let outras = self.nao_salvas;
        // 🚨 **Não há mais "salvando" aqui** (dono, 18/set/2026): o lote sobe em
        // segundo plano e o editor fecha na hora, como no site. E o "Baixar
        // como…" também não o ocupa mais: desde 10/out/2026 ele é a exportação,
        // que corre no modal dela.
        let dica = if !ha {
            "Nada a salvar: o que está no canvas já está na galeria".to_string()
        } else if outras > 0 {
            if self.pode_revelar() {
                format!("Salva esta e mais {outras} com revelação pendente, e fecha a Revelação")
            } else {
                format!(
                    "Esta foi comprada e não se revela; salva as {outras} pendentes e fecha a Revelação"
                )
            }
        } else {
            "Salva esta foto na galeria e fecha a Revelação".to_string()
        };
        BotaoDeSalvar {
            rotulo: "Salvar na galeria e sair".to_string(),
            dica,
            habilitado: self.tem_pixels() && ha,
        }
    }

    pub fn definir_cliente_aberto(&mut self, aberto: bool, cx: &mut Context<Self>) {
        if self.cliente_aberto != aberto {
            self.cliente_aberto = aberto;
            cx.notify();
        }
    }

    /// A raiz gravou a revelação nas marcadas: as cópias da tira passam a dizer
    /// o mesmo que o banco. Sem isto a seta seguinte abriria a foto
    /// recém-sincronizada com os sliders de antes.
    pub fn aplicar_sincronizadas(
        &mut self,
        gravadas: &[(String, Ajustes, Corte)],
        cx: &mut Context<Self>,
    ) {
        self.gravadas
            .extend(gravadas.iter().map(|(id, _, _)| id.clone()));
        for (id, _, _) in gravadas {
            if let Some(foto) = self.acervo.iter().find(|f| &f.id == id) {
                let antes = (
                    persistencia::da_foto(foto),
                    persistencia::corte_da_foto(foto),
                );
                self.bases.entry(id.clone()).or_insert(antes);
            }
        }
        let acervo = Arc::make_mut(&mut self.acervo);
        for (id, ajustes, corte) in gravadas {
            if let Some(foto) = acervo.iter_mut().find(|f| &f.id == id) {
                persistencia::na_foto(foto, *ajustes, *corte);
            }
        }
        cx.notify();
    }

    fn mostrar_a_posicao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::desempenho::operacao(crate::desempenho::Operacao::TrocaDeFoto);
        // Dentro do lote, o lote fica; fora dele, recomeça (`selecaoAoTrocar`).
        self.marcadas = tira::ao_trocar(&self.marcadas, self.posicao);
        let foto = self.acervo[self.posicao].clone();
        self.mostrar(foto, window, cx);
        self.adiantar_as_vizinhas(cx);
    }

    /// Decodifica a foto anterior e a seguinte **em segundo plano**.
    ///
    /// 🔑 **É o prefetch que o app de egui tinha** (`async_loader.rs`, e
    /// `docs/08-CACHE-ARCHITECTURE.md` desde dez/2025): quem revela em série anda
    /// pela seta, e a foto seguinte já estar decodificada é a diferença entre a
    /// troca ser instantânea e custar a decodificação inteira do JPEG — medido em
    /// 18/ago/2026 no catálogo real: **16 ms por foto**, toda vez.
    ///
    /// ⚠️ **Não guarda nada aqui**: só chama o `PreviewManager`, que passou a ter
    /// cache em memória. O resultado é descartado de propósito — o efeito
    /// desejado é a foto estar quente quando a seta chegar nela.
    ///
    /// ⚠️ **E vai para o executor de fundo**, não para uma `Task` guardada: se a
    /// pessoa andar cinco fotos em dois segundos, os cinco adiantamentos correm e
    /// terminam sozinhos. Cancelá-los seria jogar fora justamente o trabalho que
    /// a próxima seta vai querer.
    fn adiantar_as_vizinhas(&self, cx: &mut Context<Self>) {
        // 🔑 **A chave é a de onde a fonte daquela foto mora** (`fonte.rs`).
        // Adiantar `site:<id>` aquecia a imagem da galeria — que a Revelação nem
        // usa como origem —, e deixava fria justamente a que a seta vai pedir.
        let vizinhas: Vec<PhotoViewModel> = [self.posicao.checked_sub(1), Some(self.posicao + 1)]
            .into_iter()
            .flatten()
            .filter_map(|i| self.acervo.get(i).cloned())
            .collect();
        if vizinhas.is_empty() {
            return;
        }

        let previews = self.previews.clone();
        let edicoes = self.edicoes.clone();
        cx.background_executor()
            .spawn(async move {
                for foto in vizinhas {
                    let _m = crate::desempenho::medir(crate::desempenho::Etapa::Decodificacao);
                    let _ = fonte::copia_de_trabalho(&previews, edicoes.as_deref(), &foto);
                }
            })
            .detach();
    }

    fn mostrar(&mut self, foto: PhotoViewModel, window: &mut Window, cx: &mut Context<Self>) {
        // 🚨 **Antes de qualquer coisa**: o que a foto anterior tinha de gravado
        // ainda pode estar dentro dos 500 ms de espera. Trocar de foto primeiro
        // faria a gravação atrasada sair com os ajustes já substituídos — a
        // revelação de uma foto gravada na outra. É a mesma ordem do legado
        // ("Check if we need to save the CURRENT photo before switching").
        self.gravar_o_que_estiver_pendente();
        // O olho apertado não atravessa a troca de foto: a prévia da próxima
        // sai com tudo, e a miniatura da que sai também.
        self.estado_do_painel.ver_sem = None;
        self.estado_do_painel.arrasto_da_roda = None;
        // O conta-gotas e o "Automático" do EB são desta foto.
        self.estado_do_painel.conta_gotas = false;
        self.estado_do_painel.balanco_automatico = None;
        // E a miniatura da que sai, para a grade não mostrar a foto sem efeito.
        self.guardar_a_revelada_no_cache();

        // 🚨 **A foto do site tem bruto em outra chave.** Em `site:<id>` a
        // grade da sessão guarda a imagem da **galeria** — depois de "Salvar na
        // galeria", a foto revelada e com marca. Servir isso ao shader aplica a
        // revelação duas vezes, em 640px, e era o que a tela fazia até 7/set.
        //
        // O bruto dela é a **cópia de trabalho**, que a raiz busca no storage e
        // guarda em `trabalho:<id>` (ver `chave_do_trabalho`). Duas imagens,
        // duas chaves: aqui a origem sai da segunda, e a da galeria fica só como
        // **espera** na tela enquanto o download não volta.
        let decodificacao = crate::desempenho::medir(crate::desempenho::Etapa::Decodificacao);
        // 🖌️ **A imagem editada, quando existe, é a entrada** (C32): o que o
        // editor salvou é o que o motor recebe, e a revelação vem por cima. Sem
        // ela, a regra de sempre, logo abaixo.
        let editada = (fonte::revisao_de(self.edicoes.as_deref(), &foto) != fonte::DO_BRUTO)
            .then(|| fonte::copia_de_trabalho(&self.previews, self.edicoes.as_deref(), &foto))
            .flatten()
            .filter(|c| c.revisao != fonte::DO_BRUTO);
        let revisao_da_fonte = editada.as_ref().map_or(fonte::DO_BRUTO, |c| c.revisao);

        let do_site = persistencia::so_existe_no_site(&foto);
        let trabalho = editada.map(|c| c.imagem).or_else(|| {
            do_site
                .then(|| {
                    self.previews
                        .get_preview(&persistencia::chave_do_trabalho(&foto.id))
                })
                .flatten()
        });

        // Preview primeiro, miniatura como queda. A miniatura fica borrada numa
        // tela inteira, e é de propósito: mostrar a foto em tamanho errado é
        // melhor do que mostrar retângulo vazio.
        let do_cache = || {
            self.previews
                .get_preview(&foto.id)
                .or_else(|| self.previews.get_thumbnail(&foto.id))
        };
        // 🚨 **Na foto do site, o que está em `foto.id` é a galeria, e não o
        // bruto**: fica como `espera`, fora da `bruta` e da `revelada`. Nelas
        // ela levaria o corte de novo na tela, seria o "antes" do `\` sem ser
        // crua, e sairia no `guardar_a_revelada_no_cache` como se o motor a
        // tivesse feito.
        let espera = (do_site && trabalho.is_none()).then(do_cache).flatten();
        let bruta = if do_site {
            trabalho.clone()
        } else {
            trabalho.clone().or_else(do_cache)
        };

        // 🔑 **Sem cópia de trabalho, a foto do site não tem origem** — e é a
        // ausência de origem que faz a raiz ir buscá-la (`tem_pixels`). Com ela,
        // a seta de volta não custa rede nenhuma.
        let para_origem = if do_site || revisao_da_fonte != fonte::DO_BRUTO {
            trabalho
        } else {
            bruta.clone()
        };
        let origem = para_origem.as_ref().map(|imagem| {
            let rgba = imagem.to_rgba8();
            Origem {
                largura: rgba.width(),
                altura: rgba.height(),
                pixels: Arc::new(rgba.into_raw()),
            }
        });
        drop(decodificacao);

        // Os ajustes vêm da **foto**, e não do que estava no painel: é o que o
        // legado faz ao selecionar (`app.rs`, "Load saved edits FIRST"), e é o
        // que impede as duas metades do mesmo defeito — herdar o slider da foto
        // anterior aplicaria a revelação de uma foto em outra, e ignorar o banco
        // mostraria o arquivo cru de uma foto que já foi revelada.
        self.ajustes = persistencia::da_foto(&foto);
        self.corte = persistencia::corte_da_foto(&foto);
        self.local.esquecer_a_foto();
        match persistencia::locais_da_foto(&foto, &*self.gravador) {
            persistencia::LocaisDaFoto::Lida(parametros) => {
                self.locais = Arc::new(parametros);
                self.locais_ilegiveis = None;
            }
            persistencia::LocaisDaFoto::Ilegivel { erro, .. } => {
                crate::telemetria::avisar!("⚠️ [Revelação local] {}: {erro}", foto.name);
                self.locais = Arc::default();
                self.locais_ilegiveis = Some(erro);
            }
        }
        // 🚨 O modo de corte fecha aqui. O retângulo que está na tela é da foto
        // que sai; mantê-lo aberto aplicaria, no clique seguinte, o enquadramento
        // de uma foto na outra — o mesmo defeito que a cópia da seleção e o
        // histórico por foto já impedem nas outras pontas.
        self.edicao = None;
        // Histórico novo, começando no que está gravado: o passo zero é o estado
        // da abertura, e é o que faz o **primeiro** `Cmd+Z` ter para onde voltar.
        // No legado não tem — lá o histórico começa depois da primeira mudança, e
        // a primeira coisa que se faz numa foto não tem volta.
        // 📜 E o gravado da foto, quando chegar do disco, entra por baixo dele
        // (`ler_o_historico_gravado`).
        self.historico = Historico::novo(self.estado());
        self.previa_do_passo = None;
        self.esquecer_a_resolucao();
        self.parametros_ao_abrir = Some(self.estado());
        self.aguardando = None;
        // 🔑 O que a GPU ainda devolver é da foto que saiu. Tomar um id novo
        // também faz a thread largar o pedido velho, se ele não começou.
        self.descartar_ate = self.processador.proximo_id();
        // 🚨 O `proximo_id` acima já moveu o `id_atual` do motor: a revelação
        // antecipada que estivesse no ar foi largada por ele. Continuar
        // esperando por ela deixaria o laço de colheita de pé para sempre.
        //
        // ⚠️ O registro dela **fica** em `pedidos`: se ela tiver começado, o
        // resultado ainda chega, e é por esse registro que `colher` sabe que ele
        // vai para o cache e não para o palco.
        self.antecipando = None;
        // 🚨 **A reposição da foto anterior não vale para esta.** Herdar o
        // sinalizador faria a foto nova abrir dizendo "preparando" sem ninguém
        // ter pedido nada — e ficar assim para sempre, porque o `Reposto` que
        // apagaria o sinalizador é o da outra.
        self.repondo = false;

        // 🚨 **A foto crua não vai para a tela quando há revelação a aplicar**
        // (dono, 17/set/2026: *"primeiro mostra sem efeito e depois é aplicado
        // a revelação"*). Ela ia — `revelada` nascia com a bruta e era desenhada
        // aqui —, e o resultado do motor a substituía alguns milissegundos
        // depois: duas exibições da mesma foto, a primeira mentindo sobre como
        // ela está revelada.
        //
        // 🔑 **É o que a web faz**, e por isso lá não existe o piscar: o
        // `editor.tsx` carrega os pixels, define o corte, chama `aplicar` e só
        // então marca `fase: "pronto"` — e a janela do palco fica `invisible`
        // até lá. O mesmo desenho já está aqui dentro, em dois lugares: a troca
        // de resolução (`resolucao.rs`) e a tela do cliente (`cliente.rs`), que
        // só desenham o que saiu do motor.
        //
        // ⚠️ **Só quando vai mesmo revelar.** Sem origem não há pedido (a foto
        // do site mostra a `espera` até a cópia de trabalho chegar); no neutro
        // o resultado é a própria origem, e segurá-la seria tela preta por nada.
        let vai_revelar = origem.is_some() && !self.sem_revelacao();
        self.aberta = Some(Aberta {
            foto,
            fonte: revisao_da_fonte,
            origem,
            bruta: bruta.clone(),
            revelada: if vai_revelar { None } else { bruta },
            espera,
            desenhada: None,
        });
        self.atualizar_exibicao();
        // 📜 Depois de a foto ser a aberta: a leitura é pelo id dela.
        self.ler_o_historico_gravado(cx);

        // `set_value` **não emite** `Change` (ao contrário do `InputState` da
        // busca), então mover os 42 sliders aqui não vira 42 pedidos à GPU. Um só
        // é pedido, e só quando há o que aplicar.
        self.espalhar_nos_sliders(window, cx);

        // No neutro o resultado é a própria origem, que já está desenhada — uma
        // volta inteira à GPU para receber o que se tem é latência sem
        // contrapartida. Fora dele, a foto na tela ainda é a original, e é este
        // pedido que a torna a foto revelada.
        //
        // ⚠️ **Com foto de verdade esta guarda quase nunca economiza nada**, e é
        // o schema que decide isso: `edit_lens_vignette_midpoint` é criada com
        // `DEFAULT 50.0`, então toda foto importada difere do neutro num campo
        // que ninguém tocou. Fica assim mesmo — o legado pede sempre, então o
        // pior caso aqui é o comportamento dele —, mas a guarda não é a defesa
        // contra abertura lenta que ela parece ser.
        if vai_revelar {
            self.pedir_revelacao(cx);
        }

        // 🔑 **A tira recomeça no palco novo.** A tarefa anterior é cancelada ao
        // ser substituída: ela estaria enchendo a tira a partir de onde o
        // operador não está mais olhando, e o que já carregou continua no cache.
        self.carregar_a_tira(cx);

        // 🔑 **E a próxima já vai sendo revelada.** Com revelação a aplicar, quem
        // dispara é `colher`, ao fim desta; no neutro não há `colher` nenhum, e
        // sem esta chamada a antecipação nunca começaria numa sessão de fotos
        // ainda não trabalhadas — que é justamente a que se percorre inteira.
        self.antecipar_a_proxima(cx);

        // Quem quiser buscar os pixels desta foto em outro lugar fica sabendo
        // agora — e não só na abertura da tela.
        cx.emit(PedidoDaRevelacao::AbriuOutraFoto);
        cx.notify();
    }

    pub fn foto(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
    }

    /// Entrega os pixels que vieram **de fora** — hoje, do storage da nuvem.
    ///
    /// 🔑 **A Revelação não sabe buscar na nuvem, e não vai passar a saber.**
    /// Quem tem a sessão do site é a raiz; aqui só se recebe o que ela trouxe.
    /// É a mesma divisão da classificação: a tela do domínio não fala com o
    /// site, e continua funcionando sem ele.
    ///
    /// 🚨 **Só pinta se a foto ainda for a mesma.** Um download que volta depois
    /// de a seta ter andado pintaria a foto errada — e o pior é que ela ficaria
    /// bonita: a imagem de uma foto sob os ajustes de outra, sem erro nenhum.
    ///
    /// Devolve se os pixels foram aproveitados.
    pub fn receber_pixels(
        &mut self,
        foto_id: &str,
        imagem: image::DynamicImage,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(aberta) = self.aberta.as_mut() else {
            return false;
        };
        if aberta.foto.id != foto_id {
            return false;
        }
        // 🖌️ **Com imagem editada, a cópia do bruto que chega não entra**: a
        // entrada desta foto é a edição (C32), e trocá-la pelo bruto apagaria o
        // que o editor salvou da tela.
        if aberta.fonte != fonte::DO_BRUTO {
            return false;
        }

        let rgba = imagem.to_rgba8();
        // 🚨 **Os pixels de origem mudaram sob o mesmo id.** Até agora a origem
        // era a imagem da galeria (ou nada); agora é a cópia de trabalho. O que
        // estivesse guardado desta foto passaria a responder por uma origem que
        // não existe mais — e responderia com a cara de certo.
        self.reveladas.esquecer(foto_id);
        self.resolucao.recomecar_na_copia();
        aberta.origem = Some(Origem {
            largura: rgba.width(),
            altura: rgba.height(),
            pixels: Arc::new(rgba.into_raw()),
        });
        aberta.bruta = Some(imagem.clone());
        // 🚨 **Mesma regra da abertura**: com revelação a aplicar, a cópia de
        // trabalho não aparece crua antes de o motor responder — senão a foto do
        // site pisca duas vezes, uma ao chegar e outra ao ser revelada. No
        // neutro o resultado é igual à origem, e segurá-la só atrasaria.
        // (A mesma pergunta de `sem_revelacao`, campo a campo: `aberta` segura
        // o empréstimo de `self`.)
        let neutra = self.ajustes.sem_efeito()
            && self.locais.para_o_motor().camadas.is_empty()
            && self.locais.retoques.is_empty();
        aberta.revelada = neutra.then_some(imagem);
        // 🚨 **Mas o palco não esvazia** (dono, 30/09/2026: *"a foto é carregada
        // e depois dá uma piscada"*). Apagar o que estava desenhado deixava a
        // foto do site sumir entre o download e a resposta do motor; o que está
        // na tela — a espera da galeria — fica até a revelada entrar, e entra
        // cruzando.
        self.repondo = false;

        // 🔑 **A tira também guardou a ausência.** O `CacheDeMiniaturas` é um
        // LRU de resultados, e `Ausente` é um resultado: sem este esquecimento
        // a célula desta foto continuaria um retângulo preto ao lado da foto
        // que acabou de aparecer, até a rolagem despejá-la por acaso.
        //
        // ⚠️ **Só a ausência.** A célula que já tem imagem fica com ela: esquecê-
        // la a punha em "sem prévia" até a releitura — a mesma piscada do palco,
        // na tira. A revelada a substitui ao chegar
        // (`atualizar_a_tira_com_o_revelado`).
        if matches!(
            self.miniaturas_da_tira.espiar(foto_id),
            Some(crate::biblioteca::miniaturas::Miniatura::Ausente)
        ) {
            self.miniaturas_da_tira.esquecer(foto_id);
        }

        if neutra {
            self.cruzar = true;
            self.atualizar_exibicao();
            self.pedir_revelacao(cx);
        } else {
            self.pedir_revelacao_cruzando(cx);
        }
        cx.notify();
        true
    }

    /// Alguém foi buscar os pixels que faltam — do disco ou do site.
    ///
    /// 🔑 **Quem sabe buscar é a raiz**, aqui e no passo 11: esta tela só passa
    /// a dizer que a espera tem fim. Ver [`Self::desistir_dos_pixels`], que é o
    /// outro desfecho.
    pub fn avisar_que_os_pixels_vem_vindo(&mut self, cx: &mut Context<Self>) {
        self.repondo = true;
        cx.notify();
    }

    /// A busca acabou sem pixels. A tela volta a dizer a verdade.
    ///
    /// 🚨 **Sem isto o "preparando" fica para sempre.** Uma falha silenciosa que
    /// se parece com carregamento é pior do que a mensagem seca de antes: quem
    /// olha espera indefinidamente por algo que já terminou.
    pub fn desistir_dos_pixels(&mut self, cx: &mut Context<Self>) {
        self.repondo = false;
        cx.notify();
    }

    /// A foto aberta, para quem precisa saber de onde buscar os pixels.
    pub fn foto_aberta(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
    }

    /// As fotos da tira que o cache não tem — **da mais urgente para a menos**.
    ///
    /// 🔑 **A ordem é o produto aqui, e não a lista.** Quem repõe entrega uma
    /// por vez, então quem sai primeiro é quem estiver na frente: o palco, e
    /// depois as vizinhas para fora, que é para onde a seta vai. Ordenar pelo
    /// acervo faria a foto que está na tela esperar as vinte anteriores — o
    /// mesmo motivo de `adiantar_as_vizinhas` existir.
    ///
    /// ⚠️ **Só foto com arquivo neste disco.** A do site tem `path` vazio e o
    /// bruto dela vem da cópia de trabalho do storage; incluí-la aqui mandaria o
    /// repositor abrir um caminho que não existe, uma vez por foto.
    ///
    /// 🚨 **É chamada a cada troca de foto, então tem de ser barata.** Quem anda
    /// pela seta chama isto dezenas de vezes por minuto, na thread da interface:
    /// varrer um acervo de duas mil fotos seriam quatro mil consultas por
    /// tecla. Daí os dois tetos abaixo — o alcance e a quantidade —, e nenhum
    /// deles deixa foto para trás: a varredura recomeça no palco novo a cada
    /// troca, então o que ficou de fora entra quando a seta chegar perto.
    pub fn fotos_a_repor(&self) -> Vec<APor> {
        self.da_posicao_para_fora()
            // 🚨 O teto é sobre as **posições visitadas**, e não sobre o que
            // sobra do filtro: sem ele, um acervo em que nada falta faria a
            // varredura inteira — duas consultas por foto, a cada tecla.
            .take(RAIO_DA_REPOSICAO * 2 + 1)
            .filter_map(|i| self.acervo.get(i))
            .filter(|foto| !foto.path.is_empty())
            .filter(|foto| !self.esta_no_cache(&foto.id))
            .map(|foto| APor {
                foto_id: foto.id.clone(),
                caminho: foto.path.clone(),
            })
            .take(A_REPOR_POR_VEZ)
            .collect()
    }

    /// As posições do acervo a partir do palco, **para fora**: 0, +1, −1, +2, −2…
    ///
    /// 🔑 **A ordem é a mesma para as duas varreduras** — a que repõe o cache e a
    /// que carrega a tira — porque a pergunta é a mesma: o que o olho está vendo,
    /// e para onde a seta vai. Duas ordens diferentes fariam uma delas encher a
    /// tira pelo começo do ensaio enquanto o operador olha o meio.
    ///
    /// A `Revelacao` sem lista (o caminho de `abrir`) tem acervo de um, e o laço
    /// cobre os dois casos sem ramificar. Posições fora da lista saem no
    /// `acervo.get`, de quem consome.
    fn da_posicao_para_fora(&self) -> impl Iterator<Item = usize> + '_ {
        let posicao = self.posicao;
        std::iter::once(posicao).chain((1..=self.acervo.len()).flat_map(move |passo| {
            [posicao.checked_add(passo), posicao.checked_sub(passo)]
                .into_iter()
                .flatten()
        }))
    }

    /// Se o cache tem as **duas** entradas desta foto.
    ///
    /// 🚨 **As duas, e não uma qualquer.** Elas são gravadas no mesmo passo da
    /// importação mas apagadas por botões diferentes ("Limpar miniaturas" e
    /// "Limpar previews", nas Configurações): quem tem só a miniatura abre a
    /// Revelação num borrão em tela cheia, e quem tem só o preview grande vê a
    /// tira vazia ao lado de um palco cheio.
    ///
    /// ⚠️ Pergunta com [`PreviewManager::tem`], que **não** decodifica: uma tira
    /// de duzentas fotos com `get_*` seriam duzentos decodes só para descobrir
    /// o que falta, e o LRU inteiro trocado no caminho.
    fn esta_no_cache(&self, id: &str) -> bool {
        self.previews.tem(id, PreviewType::Large) && self.previews.tem(id, PreviewType::Thumbnail)
    }

    /// Se há pixels para revelar — a foto abriu de verdade.
    ///
    /// 🔑 **`false` não é erro, é ausência**: a foto está no catálogo e os
    /// pixels não estão à mão. Até 6/set/2026 o único lugar de onde eles podiam
    /// vir era o cache local, e por isso a foto que só existe no storage da
    /// nuvem abria vazia — sem erro nenhum, que é o que tornava isso caro.
    pub fn tem_pixels(&self) -> bool {
        self.aberta.as_ref().is_some_and(|a| a.origem.is_some())
    }

    pub fn ajustes(&self) -> Ajustes {
        self.ajustes
    }

    /// Adia a gravação para daqui a [`ESPERA_DA_GRAVACAO`].
    ///
    /// Cada chamada **substitui** a espera anterior, e substituir a `Task` a
    /// cancela — é o que faz um arrasto inteiro virar uma gravação só, em vez de
    /// uma por milímetro.
    /// Um passo do slider: o gesto continua, e acaba sozinho se parar.
    fn gesto_do_slider(&mut self, cx: &mut Context<Self>) {
        self.em_gesto = true;
        self._fim_do_gesto = Some(cx.spawn(async move |esta, cx| {
            cx.background_executor().timer(FIM_DO_GESTO).await;
            let _ = esta.update(cx, |tela, cx| tela.fim_do_gesto(cx));
        }));
    }

    /// O dedo soltou (ou parou): a cópia inteira substitui o rascunho.
    fn fim_do_gesto(&mut self, cx: &mut Context<Self>) {
        self._fim_do_gesto = None;
        if std::mem::take(&mut self.em_gesto) {
            self.pedir_revelacao(cx);
        }
    }

    /// O maior lado do palco em pixels do dispositivo, se a foto pode sair
    /// nele — só no meio de um arrasto, e só com a foto inteira à vista.
    ///
    /// ⚠️ Com zoom, no Enquadrar ou no Comparar, a cópia inteira: lá a tela
    /// mostra mais pixels do que o palco tem, ou outra coisa que não o palco.
    fn lado_do_rascunho(&self) -> Option<u32> {
        if !self.em_gesto
            || self.edicao.is_some()
            || self.comparacao.is_some()
            || self.foto_ampliada()
        {
            return None;
        }
        let lado = f32::from(self.palco.size.width).max(f32::from(self.palco.size.height))
            * self.navegacao.dpr.max(1.0);
        (lado >= 1.0).then(|| lado.ceil() as u32)
    }

    fn adiar_gravacao(&mut self, cx: &mut Context<Self>) {
        self.pendente = true;
        self._gravacao = Some(cx.spawn(async move |esta, cx| {
            cx.background_executor().timer(ESPERA_DA_GRAVACAO).await;
            // `update` falha quando a tela morreu; aí não há o que gravar e nem
            // onde reclamar.
            let _ = esta.update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());
        }));
    }

    /// Fecha o gesto: vira um passo no histórico e vai para o banco.
    ///
    /// Chamada de três lugares, e cada um fecha uma porta por onde o trabalho
    /// sairia: o fim da espera, a troca de foto e a saída da Revelação.
    ///
    /// 🔑 **É aqui que o "um `Cmd+Z` por gesto" acontece.** O legado empurra um
    /// snapshot por quadro em que algo mudou, então um arrasto vira ~30 passos —
    /// e, com o teto de 20, o resto do histórico já foi embora. Pior: o número de
    /// passos de lá depende da taxa de quadros do monitor.
    pub fn gravar_o_que_estiver_pendente(&mut self) {
        if self.fechar_o_gesto_pendente() {
            self.gravar();
        }
    }

    /// Fecha o gesto em curso **no histórico**, sem gravar. Devolve se havia um.
    ///
    /// ⚠️ Existe separado por causa do corte: `aplicar_corte` precisa fechar o
    /// gesto anterior com o enquadramento **antigo** e gravar uma vez só, no
    /// fim. Duas gravações seguidas são duas tarefas do tokio, que terminam na
    /// ordem que quiserem — e a que chegasse por último levaria o corte velho
    /// para o banco.
    fn fechar_o_gesto_pendente(&mut self) -> bool {
        if !self.pendente {
            return false;
        }
        self.pendente = false;
        self.historico.registrar(self.estado());
        true
    }

    /// Como a foto está revelada agora — ajustes e enquadramento juntos.
    ///
    /// 🔑 É o que vai para o histórico. Ler as duas metades de um lugar só é o
    /// que impede o passo meio velho meio novo: até 6/set/2026 a pilha guardava
    /// só `Ajustes`, e o corte não entrava nela.
    fn estado(&self) -> Estado {
        Estado {
            ajustes: self.ajustes,
            corte: self.corte,
            locais: self.locais.clone(),
        }
    }

    /// Manda o estado de agora para o banco, sem passar pelo histórico.
    /// Grava a revelação da foto aberta — no banco **e na cópia que está em
    /// memória**.
    ///
    /// 🚨 **As duas, e a segunda foi a que faltou.** `mostrar` lê os sliders da
    /// `PhotoViewModel` do acervo (`persistencia::da_foto`), não do banco — é o
    /// que impede herdar o slider da foto anterior. Só que o acervo é um
    /// retrato de quando a tela abriu: gravar apenas no banco deixava a cópia em
    /// memória dizendo o que a foto era **antes** do ajuste, e a seta de ida e
    /// volta trazia a foto de volta no neutro. Trabalho perdido sem erro nenhum,
    /// e a cada troca de foto.
    ///
    /// ⚠️ **E para a foto do site o banco não responde**: o id dela é
    /// `site:<uuid>`, que não é linha do catálogo — `save_edits` devolve
    /// `PhotoNotFound`, e o `Gravador` não tem como dizer isso a ninguém (não
    /// devolve `Result`, de propósito). Sem a escrita em memória, revelar uma
    /// foto do site era escrever na água.
    fn gravar(&mut self) {
        let Some(aberta) = self.aberta.as_ref() else {
            return;
        };
        // 🚨 A comprada não se revela: nada dela vai para o banco.
        if aberta.foto.revelacao_travada {
            return;
        }
        let id = aberta.foto.id.clone();
        // A base é o que a foto tinha antes do primeiro gesto: a cópia que
        // abriu, ainda não tocada por esta gravação.
        if !self.bases.contains_key(&id) {
            let antes = (
                persistencia::da_foto(&aberta.foto),
                persistencia::corte_da_foto(&aberta.foto),
            );
            self.bases.insert(id.clone(), antes);
        }
        self.gravador.gravar(id.clone(), self.ajustes, self.corte);
        self.gravadas.insert(id.clone());
        self.gravar_locais_se_mudou(&id);

        let (ajustes, corte) = (self.ajustes, self.corte);
        if let Some(aberta) = self.aberta.as_mut() {
            persistencia::na_foto(&mut aberta.foto, ajustes, corte);
        }
        let acervo = Arc::make_mut(&mut self.acervo);
        if let Some(foto) = acervo.iter_mut().find(|f| f.id == id) {
            persistencia::na_foto(foto, ajustes, corte);
        }
        // 📜 O histórico vai junto: todo gesto, desfazer e clique no painel
        // passa por aqui.
        self.gravar_o_historico();
    }

    /// A revelação local vai ao banco **à parte** dos ajustes, e só quando mudou
    /// — ver `Gravador::gravar_locais`. Ilegível, nunca: não se grava por cima
    /// do que não se leu.
    fn gravar_locais_se_mudou(&mut self, id: &str) {
        if self.locais_ilegiveis.is_some() {
            return;
        }
        let novo = self.locais.em_json();
        let gravado = match persistencia::id_no_site(id) {
            Some(no_site) => self.gravador.locais_do_site(no_site),
            None => self.aberta.as_ref().and_then(|a| a.foto.locais.clone()),
        };
        if novo == gravado {
            return;
        }
        self.gravador.gravar_locais(id.to_string(), novo.clone());
        if persistencia::id_no_site(id).is_none() {
            if let Some(aberta) = self.aberta.as_mut() {
                aberta.foto.locais = novo.clone();
            }
            let acervo = Arc::make_mut(&mut self.acervo);
            if let Some(foto) = acervo.iter_mut().find(|f| f.id == id) {
                foto.locais = novo;
            }
        }
    }

    /// A revelação local de uma foto do acervo: a da tela, se ela está aberta;
    /// senão, a gravada.
    pub(super) fn locais_de(&self, foto: &PhotoViewModel) -> Arc<ParametrosLocais> {
        if self.aberta.as_ref().is_some_and(|a| a.foto.id == foto.id) {
            return self.locais.clone();
        }
        match persistencia::locais_da_foto(foto, &*self.gravador) {
            persistencia::LocaisDaFoto::Lida(parametros) => Arc::new(parametros),
            persistencia::LocaisDaFoto::Ilegivel { .. } => Arc::default(),
        }
    }

    /// A revelação local que a GPU revela agora — a da foto, mais o gesto que
    /// estiver em curso na Revelação local.
    pub(super) fn locais_na_tela(&self) -> Arc<ParametrosLocais> {
        if let Some(passo) = self.passo_em_previa() {
            return passo.estado.locais.clone();
        }
        self.locais_com_o_gesto()
    }

    /// A revelação local gravada de uma foto do acervo — a da tela, se ela está
    /// aberta. Para a segunda tela.
    pub fn locais_da(&self, foto: &PhotoViewModel) -> Arc<ParametrosLocais> {
        self.locais_de(foto)
    }

    /// A revelação local da foto aberta — a segunda tela revela com ela.
    pub fn locais(&self) -> Arc<ParametrosLocais> {
        if self.mostrando_original() {
            return Arc::default();
        }
        self.locais.clone()
    }

    /// Volta um passo. `Cmd+Z`.
    pub fn desfazer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 🚨 O gesto em curso **fecha antes**. Sem isto, arrastar um slider e
        // apertar `Cmd+Z` dentro dos 500 ms desfaria o passo *anterior* e deixaria
        // o arrasto de agora pendente — que gravaria logo depois, por cima do que
        // acabou de ser desfeito. O `Cmd+Z` pareceria não ter funcionado.
        self.gravar_o_que_estiver_pendente();

        if let Some(estado) = self.historico.desfazer() {
            self.aplicar_do_historico(estado, window, cx);
        }
    }

    /// Avança um passo. `Cmd+Shift+Z`.
    pub fn refazer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();

        if let Some(estado) = self.historico.refazer() {
            self.aplicar_do_historico(estado, window, cx);
        }
    }

    /// O estado que veio do histórico vira tela, foto e linha no banco.
    ///
    /// ⚠️ **Grava na hora, e não depois de 500 ms.** A espera existe para juntar
    /// os dezenas de eventos de um arrasto; `Cmd+Z` é um gesto discreto, e adiá-lo
    /// só criaria uma janela para perder o desfazer. O legado grava por outro
    /// caminho — o autosave dele nota a diferença no quadro seguinte —, mas grava.
    ///
    /// 🔑 **Não registra passo novo no histórico**: desfazer é andar nele, não
    /// escrever nele. Registrar aqui faria o `Cmd+Z` empilhar um passo igual ao
    /// que acabou de sair, e o `Cmd+Shift+Z` nunca alcançaria nada.
    fn aplicar_do_historico(
        &mut self,
        estado: Estado,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mostrar_o_estado(estado, window, cx);
        self.gravar();
        cx.notify();
    }

    /// O estado vira tela — sliders, corte, Revelação local e a GPU —, **sem
    /// gravar**. É o miolo do desfazer, e o do descarte, que não grava porque
    /// quem manda na revelação passa a ser a galeria (`descartar.rs`).
    fn mostrar_o_estado(&mut self, estado: Estado, window: &mut Window, cx: &mut Context<Self>) {
        self.ajustes = estado.ajustes;
        self.corte = estado.corte;
        self.locais = estado.locais;
        self.conferir_a_selecao_local();
        // O Enquadrar continua aberto, como no site: o retângulo é lido do
        // corte da foto, e o pedido do operador recomeça do que voltou.
        if let Some(edicao) = self.edicao.as_mut() {
            // O traçado de guias continua ligado: desfazer uma guia e traçar
            // outra é o gesto seguido mais comum da perspectiva guiada.
            let armadas = edicao.guias.armadas;
            *edicao = Edicao {
                proporcao: edicao.proporcao,
                ..Edicao::default()
            };
            edicao.guias.armadas = armadas;
        }
        let graus = self.corte_atual().angle();
        self.angulo
            .update(cx, |estado, cx| estado.set_value(graus, window, cx));
        self.sincronizar_sliders_da_perspectiva(window, cx);
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        // O corte não passa pela GPU: quem o mostra é a exibição.
        self.atualizar_exibicao();
    }

    /// Leva os ajustes de agora para as 42 barras.
    ///
    /// 🚨 Só funciona sem virar 42 pedidos à GPU porque `SliderState::set_value`
    /// **não emite** `Change` — o oposto do `InputState` da busca. É a assimetria
    /// que `abrir_sem_edicao_nao_pede_nada_a_gpu` prende.
    fn espalhar_nos_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        for controle in &self.controles {
            let valor = (controle.definicao.ler)(&self.ajustes);
            controle
                .estado
                .update(cx, |estado, cx| estado.set_value(valor, window, cx));
            // A lista também: preset, desfazer e troca de foto mudam o Estilo
            // sem passar por ela. `set_selected_value` não emite `Confirm`.
            if let (Some(lista), Some(opcoes)) = (&controle.escolha, controle.definicao.opcoes) {
                let nome = opcoes[(valor.round().max(0.0) as usize).min(opcoes.len() - 1)];
                lista.update(cx, |l, cx| {
                    if l.selected_value() != Some(&nome) {
                        l.set_selected_value(&nome, window, cx);
                    }
                });
            }
        }
    }

    /// O "Auto" do painel Básico: lê a foto e escolhe a exposição.
    ///
    /// 🚨 **Ele já existiu como preset, e não fazia nada.** Até 30/ago/2026 a
    /// lista de presets de sistema trazia um "Auto" que pedia
    /// `exposure: Some(0.0)` — o próprio neutro. Um preset é uma lista de números
    /// fixos; o que este botão faz é o contrário disso: mede **esta** foto e
    /// decide a partir dela ([`automatico`]).
    ///
    /// ⚠️ **Mede a foto crua, não a que está na tela.** `Aberta::bruta` é a
    /// imagem como saiu do cache; o histograma do painel é o da revelada. Medir a
    /// revelada faria o segundo clique decidir sobre o resultado do primeiro, e o
    /// botão andaria sozinho a cada toque em vez de convergir.
    ///
    /// É gesto discreto, como o preset: vira passo de histórico e vai para o
    /// banco na hora, sem a espera de 500 ms que existe para juntar arrasto.
    pub fn tom_automatico(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Sem foto (ou sem pixels no cache) não há o que medir. O botão já nasce
        // desligado neste caso; a guarda é para quem chamar por outro caminho.
        let Some(Aberta {
            bruta: Some(bruta), ..
        }) = self.aberta.as_ref()
        else {
            return;
        };
        let escolha = automatico::escolher(&Histograma::da_imagem(bruta));

        self.gravar_o_que_estiver_pendente();
        self.ajustes.exposure = escolha.exposure;
        self.ajustes.highlights = escolha.highlights;
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao(cx);
        self.historico
            .registrar_como(self.estado(), "Tom automático");
        self.gravar();
        cx.notify();
    }

    /// O corte gravado na foto, ou a foto inteira quando não há nenhum.
    /// O enquadramento de agora, para quem vai revelar fora desta tela.
    ///
    /// 🔑 Vai junto com [`Self::ajustes`] ao salvar na galeria: revelar sem ele
    /// mandaria ao cliente a foto inteira, com o horizonte torto que o operador
    /// acabou de endireitar.
    pub fn enquadramento(&self) -> CropSettings {
        self.corte_atual()
    }

    fn corte_atual(&self) -> CropSettings {
        persistencia::para_crop_settings(&self.corte)
    }

    /// O tamanho da **cópia de trabalho**, mesmo com o bruto na tela: é nele
    /// que o Enquadrar e o "A foto sai com" medem.
    fn tamanho_da_foto(&self) -> Option<(f32, f32)> {
        self.tamanho_da_copia()
    }

    /// Refaz o que está na tela a partir da foto revelada.
    ///
    /// 🔑 **Não é chamada durante o arrasto de alça**, e é de propósito: no modo
    /// de corte a foto aparece inteira, e o retângulo é desenho por cima. O que
    /// muda com o arrasto é o desenho, não os pixels — recalcular a cada
    /// milímetro reprocessaria a foto dezenas de vezes por segundo para produzir
    /// exatamente a mesma imagem.
    /// 🔑 **Nada a revelar: ajustes no neutro e nenhuma máscara ou retoque.**
    /// A pergunta é da revelação inteira — olhar só os ajustes deixava a foto
    /// com Revelação local aparecer crua ao abrir (a GPU nem era chamada).
    fn sem_revelacao(&self) -> bool {
        self.ajustes.sem_efeito()
            && self.locais.para_o_motor().camadas.is_empty()
            && self.locais.retoques.is_empty()
    }

    fn atualizar_exibicao(&mut self) {
        self.refazer_exibicao(true);
    }

    /// O mesmo, sem tocar no histograma — para o "antes/depois", que troca a
    /// imagem e mantém a régua.
    fn atualizar_exibicao_mantendo_histograma(&mut self) {
        self.refazer_exibicao(false);
    }

    fn refazer_exibicao(&mut self, medir: bool) {
        // Refeita agora, com o estado de agora: o pedido do quadro está atendido.
        self.exibicao_atrasada = false;
        // No modo de corte a foto aparece inteira (girada e endireitada), com o
        // retângulo por cima; fora dele, recortada. É o `apply_crop_clip` do
        // legado.
        let corte = self.corte_atual();
        let recortar = self.edicao.is_none();
        let cruzar = std::mem::take(&mut self.cruzar);

        let Some(aberta) = self.aberta.as_mut() else {
            if medir {
                self.histograma = None;
            }
            return;
        };

        // 🔑 O "antes" troca só a **fonte**, e não o enquadramento: comparar cor
        // com a foto pulando de tamanho na tela não compara nada. É o que o
        // legado faz — ele troca a textura e mantém o corte, que vive no viewer.
        let fonte = if self.mostrando_original {
            aberta.bruta.as_ref()
        } else {
            aberta.revelada.as_ref()
        };

        let exibida = {
            let _m = crate::desempenho::medir(crate::desempenho::Etapa::RecorteNaCpu);
            fonte.map(|imagem| transformacao::aplicar(imagem, &corte, recortar))
        };
        // A espera da galeria, enquanto nada melhor chegou — e sem corte: ela
        // já vem enquadrada. Com a revelada na mão, não serve mais.
        if aberta.revelada.is_some() {
            aberta.espera = None;
        }
        let exibida = exibida.or_else(|| aberta.espera.clone());

        // 🔑 O histograma mede **o que está na tela**, e não a foto crua: com os
        // sliders mexidos, o histograma do cru descreveria uma imagem que ninguém
        // está vendo. É o que o legado faz (ele calcula depois do `process_image`).
        if medir {
            let _m = crate::desempenho::medir(crate::desempenho::Etapa::Histograma);
            self.histograma = exibida.as_ref().map(Histograma::da_imagem);
        }

        // 🔑 **A que sai é guardada antes de a que entra ocupar o lugar.** O
        // palco desenha as duas empilhadas por [`CRUZAMENTO_DA_FOTO`], com a
        // nova ganhando opacidade — é o que transforma "a imagem trocou" em "a
        // revelação aconteceu".
        let anterior = aberta.desenhada.take();
        aberta.desenhada = exibida.map(para_gpui);
        let entrou = aberta.desenhada.is_some();

        self.saindo = if cruzar && entrou { anterior } else { None };
        if self.saindo.is_some() {
            self.cruzamento = self.cruzamento.wrapping_add(1);
        }
    }

    /// Alterna entre a foto revelada e a original. É o `\\` do legado.
    ///
    /// ⚠️ **Não mexe nos ajustes.** Os 42 sliders continuam onde estavam, e o
    /// histograma continua o da revelada — quem está comparando quer ver a
    /// diferença, e um histograma que pula junto tiraria a régua da comparação.
    pub fn alternar_original(&mut self, cx: &mut Context<Self>) {
        self.mostrando_original = !self.mostrando_original;
        self.atualizar_exibicao_mantendo_histograma();
        cx.notify();
    }

    pub fn mostrando_original(&self) -> bool {
        self.mostrando_original
    }

    /// Devolve **um** controle ao neutro — o duplo clique no rótulo.
    ///
    /// É um gesto discreto, como aplicar preset: fecha o que estava a meio
    /// caminho, vira um passo de histórico e vai para o banco na hora. Sem
    /// fechar o pendente, um duplo clique no meio de um arrasto juntaria os
    /// dois num passo só, e o `Cmd+Z` desfaria mais do que o olho viu.
    pub fn devolver_ao_neutro(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(controle) = self.controles.get(indice) else {
            return;
        };
        let definicao = controle.definicao;
        let neutro = definicao.neutro();
        if (definicao.ler)(&self.ajustes) == neutro {
            return;
        }

        self.gravar_o_que_estiver_pendente();
        (definicao.aplicar)(&mut self.ajustes, neutro);
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        self.historico.registrar(self.estado());
        self.gravar();
        cx.notify();
    }

    pub fn pode_desfazer(&self) -> bool {
        self.historico.pode_desfazer()
    }

    pub fn pode_refazer(&self) -> bool {
        self.historico.pode_refazer()
    }

    /// Move um controle sem passar pelo slider, para os testes da raiz.
    ///
    /// ⚠️ Ele **não** substitui o `arrastar` dos testes desta tela, que emite o
    /// `SliderEvent` de verdade e é o único que prova que a inscrição está viva.
    /// Existe porque um teste em `app.rs` precisa de "havia ajuste pendente"
    /// dentro de um único `update`, e emitir evento ali exigiria devolver o
    /// controle ao executor no meio.
    #[cfg(test)]
    pub fn aplicar_para_teste(&mut self, controle: usize, valor: f32, cx: &mut Context<Self>) {
        (self.controles[controle].definicao.aplicar)(&mut self.ajustes, valor);
        self.adiar_gravacao(cx);
    }

    /// Manda os ajustes de agora para a GPU.
    /// O mesmo pedido, com a foto **cruzando** em vez de trocar seca.
    ///
    /// 🚨 **É para o salto, e não para o arrasto.** Ver [`CRUZAMENTO_DA_FOTO`]:
    /// cruzar dois resultados que chegam a cada poucos milissegundos deixaria
    /// fantasma justamente no gesto em que a resposta imediata é o valor.
    fn pedir_revelacao_cruzando(&mut self, cx: &mut Context<Self>) {
        self.cruzar = true;
        self.pedir_revelacao(cx);
    }

    fn pedir_revelacao(&mut self, cx: &mut Context<Self>) {
        let preparacao = crate::desempenho::medir(crate::desempenho::Etapa::PreparacaoDosAjustes);
        let Some(Aberta {
            foto,
            origem: Some(origem),
            ..
        }) = &self.aberta
        else {
            return;
        };
        let foto_id = foto.id.clone();
        let pixels = origem.pixels.clone();
        let (largura, altura) = (origem.largura, origem.altura);
        let revisao_da_fonte = self.aberta.as_ref().map_or(fonte::DO_BRUTO, |a| a.fonte);

        let ajustes = self.ajustes_na_tela();
        let corte = transformacao::corte(&self.corte_na_tela());
        let locais = self.locais_na_tela();
        let chave = cache::Chave::nova(&foto_id, (largura, altura), &ajustes, &corte, &locais)
            .da_fonte(revisao_da_fonte);

        // 🔑 **O que já foi revelado não é revelado de novo.** Voltar uma seta,
        // desfazer, tirar o ponteiro de cima de uma predefinição: nos três a
        // revelação é uma que já passou pelo motor, e a ida à GPU produziria
        // exatamente o mesmo pixel. Com o palco esperando a resposta para
        // desenhar (ver `mostrar`), essa ida é o tempo de tela preta.
        if let Some(pronta) = self.reveladas.buscar(&chave) {
            if let Some(aberta) = self.aberta.as_mut() {
                aberta.revelada = Some(pronta);
            }
            // 🚨 **O pedido que ainda está na GPU ficou para trás deste.** Sem
            // isto o resultado dele chega depois e pinta por cima: passar o
            // ponteiro por uma predefinição e sair (a volta vem do cache)
            // deixava a foto com o preset — na hora, ou no próximo zoom, que
            // religa a colheita e acha o atrasado no canal.
            if let Some(em_voo) = self.aguardando {
                self.descartar_ate = self.descartar_ate.max(em_voo);
            }
            // Nada mais a esperar: um `aguardando` pendurado aqui deixaria o
            // laço de colheita perguntando por um pedido que não existe.
            self.aguardando = None;
            self.atualizar_exibicao();
            self.atualizar_a_tira_com_o_revelado();
            self.antecipar_a_proxima(cx);
            cx.notify();
            return;
        }

        let lado_na_tela = self.lado_do_rascunho();
        let id = self.processador.proximo_id();
        if crate::desempenho::ativa() {
            crate::desempenho::imagem(crate::desempenho::Imagem {
                largura,
                altura,
                formato: self
                    .aberta
                    .as_ref()
                    .and_then(|a| {
                        std::path::Path::new(&a.foto.name)
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                    })
                    .unwrap_or_else(|| "?".into()),
                mascaras: locais.camadas.len() as u32,
                retoques: locais.retoques.len() as u32,
                enquadramento: if self.edicao.is_some() {
                    "enquadrar aberto"
                } else if corte.tem_perspectiva() {
                    "perspectiva"
                } else if corte.angulo() != 0.0 {
                    "endireitado"
                } else if corte == transformacao::Corte::inteiro() {
                    "nenhum"
                } else {
                    "reto"
                }
                .into(),
            });
        }
        drop(preparacao);
        crate::desempenho::pedido_ao_motor(id);
        self.processador.pedir(Pedido {
            id,
            pixels,
            largura,
            altura,
            ajustes,
            corte,
            locais,
            lado_na_tela,
        });
        self.pedidos.insert(
            id,
            Pendente {
                chave,
                destino: Destino::Palco,
                rascunho: lado_na_tela.is_some(),
            },
        );
        // 🚨 **O especulativo ficou para trás deste.** `proximo_id` acima já moveu
        // o `id_atual` do motor, então a thread larga o pedido antecipado se ele
        // não tiver começado — esquecê-lo aqui é o que impede o laço de colheita
        // de esperar para sempre por um resultado que não vem.
        self.antecipando = None;
        self.aguardando = Some(id);
        self.acompanhar(cx);
        cx.notify();
    }

    /// Revela a **próxima** foto antes de a seta chegar nela.
    ///
    /// 🔑 **É a outra metade do pedido do dono** (17/set/2026). O cache tira a
    /// ida à GPU da segunda visita; isto tira a da primeira, que é a que o
    /// operador sente ao percorrer uma sessão inteira com a seta. O prefetch das
    /// vizinhas (`adiantar_as_vizinhas`) já deixava os pixels decodificados; o
    /// que faltava era o passo seguinte, que é o caro.
    ///
    /// Quatro recusas, e cada uma tem motivo:
    ///
    /// - **há pedido no ar** (`aguardando`) — a GPU tem o que fazer com a foto
    ///   que o operador está olhando, e furar essa fila é trocar a resposta de
    ///   agora por uma de daqui a pouco;
    /// - **já há um especulativo** — um por vez; ver [`Self::antecipando`];
    /// - **o Enquadrar está aberto**, ou os pixels estão sendo repostos — nos
    ///   dois, cada movimento do operador vira pedido;
    /// - **a próxima está no neutro** — sem revelação ela aparece direto da prévia,
    ///   e não há ida à GPU para poupar.
    fn antecipar_a_proxima(&mut self, cx: &mut Context<Self>) {
        if self.aguardando.is_some() || self.antecipando.is_some() {
            return;
        }
        // No Comparar a GPU é da outra metade: ver `tela/comparar.rs`.
        if self.comparacao.is_some() {
            return;
        }
        if self.edicao.is_some() || self.repondo {
            return;
        }
        let Some(proxima) = tira::vizinha(&self.na_tira(), self.posicao, self.rumo) else {
            return;
        };
        let Some(foto) = self.acervo.get(proxima).cloned() else {
            return;
        };

        let ajustes = persistencia::da_foto(&foto);
        let locais = self.locais_de(&foto);
        if ajustes.sem_efeito() && locais.vazia() {
            return;
        }
        let corte = transformacao::corte(&persistencia::para_crop_settings(
            &persistencia::corte_da_foto(&foto),
        ));
        // A fonte vem do mesmo lugar que a do palco (`fonte.rs`): a imagem
        // editada, se houver; senão o bruto — que na foto do site mora na cópia
        // de trabalho, não na imagem da galeria.
        let previews = self.previews.clone();
        let edicoes = self.edicoes.clone();
        let para_a_fonte = foto.clone();
        cx.spawn(async move |esta, cx| {
            // ⚠️ **A decodificação vai para o executor de fundo.** Ela custa os
            // ~16 ms que o prefetch das vizinhas mede, e gastá-los no quadro
            // seria trocar lentidão futura por lentidão agora — bem no instante
            // em que a foto acabou de aparecer.
            let origem = cx
                .background_executor()
                .spawn(async move {
                    let _m = crate::desempenho::medir(crate::desempenho::Etapa::Decodificacao);
                    let copia =
                        fonte::copia_de_trabalho(&previews, edicoes.as_deref(), &para_a_fonte)?;
                    let rgba = copia.imagem.to_rgba8();
                    let (largura, altura) = (rgba.width(), rgba.height());
                    Some((largura, altura, Arc::new(rgba.into_raw()), copia.revisao))
                })
                .await;
            let Some((largura, altura, pixels, revisao)) = origem else {
                return;
            };
            let _ = esta.update(cx, |tela, cx| {
                tela.enfileirar_a_antecipacao(
                    &foto.id, largura, altura, pixels, revisao, ajustes, corte, locais, cx,
                );
            });
        })
        .detach();
    }

    /// Põe na fila a revelação antecipada, se ela ainda fizer sentido.
    ///
    /// 🚨 **As guardas são refeitas aqui.** Entre a decisão de antecipar e a
    /// chegada dos pixels houve uma ida ao disco: o operador pode ter andado,
    /// mexido num slider ou aberto o Enquadrar. Furar a fila com trabalho
    /// especulativo nesse momento atrasaria a foto que ele está vendo.
    #[allow(clippy::too_many_arguments)]
    fn enfileirar_a_antecipacao(
        &mut self,
        foto_id: &str,
        largura: u32,
        altura: u32,
        pixels: Arc<Vec<u8>>,
        revisao_da_fonte: u64,
        ajustes: Ajustes,
        corte: transformacao::Corte,
        locais: Arc<ParametrosLocais>,
        cx: &mut Context<Self>,
    ) {
        if self.aguardando.is_some()
            || self.antecipando.is_some()
            || self.edicao.is_some()
            || self.comparacao.is_some()
        {
            return;
        }
        let chave = cache::Chave::nova(foto_id, (largura, altura), &ajustes, &corte, &locais)
            .da_fonte(revisao_da_fonte);
        if self.reveladas.buscar(&chave).is_some() {
            return;
        }

        let id = self.processador.proximo_id();
        self.processador.pedir(Pedido {
            id,
            pixels,
            largura,
            altura,
            ajustes,
            corte,
            locais,
            lado_na_tela: None,
        });
        self.pedidos.insert(
            id,
            Pendente {
                chave,
                destino: Destino::Cache,
                rascunho: false,
            },
        );
        self.antecipando = Some(id);
        // O laço precisa continuar de pé: sem ele o resultado especulativo fica
        // no canal e o cache nunca o vê.
        self.acompanhar(cx);
    }

    /// O enquadramento que a tela mostra: o do modo de corte, se ele estiver
    /// aberto; senão, o da foto.
    fn corte_na_tela(&self) -> CropSettings {
        self.corte_atual()
    }

    /// O corte da tela mudou: com vinheta ligada, a foto é revelada de novo.
    ///
    /// 🚨 **As duas vinhetas são medidas no recorte** (`Motor::definir_corte`),
    /// então mudar o corte muda pixel revelado — e só refazer a exibição
    /// recortaria a revelação velha, com a vinheta ainda no enquadramento de
    /// antes. É o que o editor do site faz ao arrastar uma alça: a vinheta
    /// acompanha o retângulo.
    ///
    /// 🔑 **Sem vinheta, não pede nada**: o enquadramento sozinho não muda o que
    /// o shader devolve, e revelar a cada milímetro de alça seria GPU por nada.
    fn revelar_de_novo_se_a_vinheta_segue_o_corte(&mut self, cx: &mut Context<Self>) {
        if self.ajustes_na_tela().vinheta_ligada() {
            self.pedir_revelacao(cx);
        }
    }

    /// Liga o laço que pergunta pelo resultado, se ainda não houver um.
    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;

        cx.spawn(async move |esta, cx| {
            loop {
                cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
                // `update` falha quando a tela morreu — fechar a janela no meio
                // de um arrasto não pode deixar um laço rodando sozinho.
                let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                    break;
                };
                if !continua {
                    break;
                }
            }
        })
        .detach();
    }

    /// Pega o resultado mais recente, se houver. Devolve se vale continuar
    /// perguntando.
    fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        // 🚨 **Sem GPU não vem resultado nenhum.** Como o palco passou a esperar
        // a revelação para desenhar, uma máquina sem adaptador ficaria com a
        // tela vazia para sempre — e este laço perguntando a cada 8 ms, também
        // para sempre. Aqui a espera acaba e a foto crua volta, que é o que o
        // `processador.rs` promete: "a Revelação mostra a foto sem ajuste —
        // honesto e visível".
        //
        // ⚠️ `Some(false)`, e não `!= Some(true)`: `None` é "a thread ainda está
        // abrindo o dispositivo", e desistir ali desistiria em toda abertura.
        if self.processador.disponivel() == Some(false) {
            let sem_nada_na_tela =
                matches!(&self.aberta, Some(aberta) if aberta.revelada.is_none());
            if sem_nada_na_tela {
                if let Some(aberta) = self.aberta.as_mut() {
                    aberta.revelada = aberta.bruta.clone();
                }
                self.atualizar_exibicao();
                cx.notify();
            }
            self.aguardando = None;
            self.colhendo = false;
            return false;
        }

        let descartar_ate = self.descartar_ate;
        if let Some(resultado) = self.processador.colher() {
            // De quem é este resultado, e para quê. Os pedidos de id menor foram
            // largados pela thread — ela só atende o mais recente —, e saem do
            // mapa junto: senão ele cresceria um registro por evento de slider.
            let pendente = self.pedidos.remove(&resultado.id);
            self.pedidos.retain(|id, _| *id > resultado.id);
            if self.antecipando.is_some_and(|id| id <= resultado.id) {
                self.antecipando = None;
            }

            // 🔑 **Só o estado parado entra no cache.** No meio de um arrasto
            // chegam dezenas de resultados de valores que o dedo já passou;
            // guardar cada um encheria o cache com revelações que ninguém vai
            // pedir de volta e despejaria as fotos que valem.
            let ultimo_do_gesto = self.aguardando == Some(resultado.id);
            let antecipado = pendente
                .as_ref()
                .is_some_and(|p| !matches!(p.destino, Destino::Palco));
            let rascunho = pendente.as_ref().is_some_and(|p| p.rascunho);
            // 🔧 Com `VLB_VIGIA=1`, o tamanho e o custo de cada revelação — a
            // conta que separa a GPU do resto quando um balcão acha lento.
            if crate::depuracao::vigia::ligado() {
                eprintln!(
                    "[revelacao] {}x{} {} em {:.1} ms no motor ({})",
                    resultado.imagem.width(),
                    resultado.imagem.height(),
                    if rascunho { "rascunho" } else { "inteira" },
                    resultado.duracao_ms,
                    self.processador.backend().unwrap_or("?"),
                );
            }
            if let Some(pendente) = &pendente {
                if (antecipado || ultimo_do_gesto) && !rascunho {
                    self.reveladas
                        .guardar(pendente.chave.clone(), &resultado.imagem);
                }
            }
            // 🚨 **O pedido do Comparar que a thread largou volta para a fila.**
            // Ela só atende o mais novo: se a aberta pediu depois (uma revelação
            // que mudou por fora), a outra metade ficaria esperando para sempre.
            if self.pedido_do_comparar.is_some_and(|id| id < resultado.id) {
                self.pedido_do_comparar = None;
                self.devolver_a_fila_do_comparar();
            }
            if let Some(Pendente {
                destino: Destino::Comparar { posicao, crop },
                ..
            }) = &pendente
            {
                if self.pedido_do_comparar == Some(resultado.id) {
                    self.pedido_do_comparar = None;
                }
                let (posicao, crop) = (*posicao, crop.clone());
                self.entregar_ao_comparar(posicao, &resultado.imagem, &crop, cx);
            }

            // 🚨 **A revelação antecipada nunca vai ao palco.** Ela é da foto
            // **seguinte**: pintá-la seria mostrar uma foto sob o nome de outra
            // — o mesmo defeito que o `descartar_ate` pegou em 17/set/2026, por
            // outro caminho. Sem registro, vale a regra antiga (o id).
            if !antecipado && resultado.id > descartar_ate {
                if let Some(aberta) = self.aberta.as_mut() {
                    aberta.revelada = Some(resultado.imagem);
                }
                self.atualizar_exibicao();
                crate::desempenho::foto_na_tela(resultado.id);
                // Só larga a espera se o que voltou é o último pedido. No meio de
                // um arrasto chegam resultados de valores já ultrapassados, e
                // parar de colher ali deixaria a foto congelada num ajuste que o
                // dedo já passou.
                if ultimo_do_gesto {
                    self.aguardando = None;
                }
                // O rascunho é só o que o dedo vê: a tira, a próxima foto e o
                // Comparar esperam a cópia inteira, que vem quando o gesto acaba.
                if ultimo_do_gesto && !rascunho {
                    // 🔑 **Chegou o último: a tira mostra o que o palco mostra.**
                    // Este é o único instante em que a foto revelada está pronta e
                    // parada — no meio de um arrasto os resultados são de valores
                    // que o dedo já passou, e reduzir cada um seria refazer a
                    // miniatura sessenta vezes por segundo para mostrar a
                    // penúltima.
                    self.atualizar_a_tira_com_o_revelado();
                    // E, com a GPU livre, a próxima foto já pode ir sendo feita.
                    self.antecipar_a_proxima(cx);
                    self.soltar_a_proxima_do_comparar(cx);
                }
            }
            cx.notify();
        }

        let continua = self.aguardando.is_some()
            || self.antecipando.is_some()
            || self.pedido_do_comparar.is_some();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn palco(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.comparacao.is_some() {
            return self.palco_do_comparar(cx);
        }
        // 🚨 **`size_full`, e não `flex_1`.** Esta moldura era filha de uma
        // linha flex antes do dock; hoje ela é a **raiz de um painel**, e
        // `flex_1` sem pai flex não cresce: a altura cai no conteúdo, o filho
        // `size_full()` vira 100% de zero, e a foto é desenhada num retângulo
        // sem tamanho. O painel continua ali, com título e área — só que vazio.
        //
        // Nada falha. Foi relatado com a tela na mão, e o que denuncia é
        // `o_palco_tem_tamanho_depois_de_desenhado`, que lê as bounds que o
        // `canvas` grava.
        let moldura = div()
            .flex()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            // 🔑 **O poço, e não o fundo do app.** O olho julga exposição por
            // comparação com o que está em volta: entorno mais claro que a foto
            // faz toda foto parecer subexposta. É o degrau mais escuro da
            // escada, reservado ao que encosta em imagem.
            .bg(tema::cores::poco())
            .p(px(24.));

        match self.aberta.as_ref() {
            None => moldura
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Escolha uma foto na Biblioteca"),
                )
                .into_any_element(),
            // `ObjectFit::Contain` é o padrão do `img`: a foto cabe inteira, sem
            // recorte. `Cover` cortaria — e num estúdio de retrato o recorte
            // centralizado tira a cabeça primeiro.
            Some(Aberta {
                desenhada: Some(imagem),
                ..
            }) => moldura
                .child(
                    // 🚨 **A caixa `relative` é esta, e não a moldura.** A moldura
                    // tem 24px de respiro; um absoluto ancorado nela se mede pela
                    // caixa **com** o respiro, enquanto a foto ocupa a de dentro.
                    // Os dois sistemas ficariam deslocados de 24px, e o retângulo
                    // de corte apareceria fora do lugar — pouco, o suficiente para
                    // parecer erro de mira do usuário. Aqui foto, overlay e
                    // medição dividem exatamente o mesmo retângulo.
                    self.com_gestos_de_zoom(
                        div()
                            .id("palco-da-foto")
                            // O clique foca o elemento que rastreia o foco.
                            .track_focus(&self.foco_do_palco)
                            .relative()
                            .size_full()
                            // O zoom mostra só o pedaço da foto que cabe.
                            .overflow_hidden(),
                        cx,
                    )
                    // 🔑 **A foto de antes fica embaixo, inteira, e a nova
                    // entra ganhando opacidade por cima.** Sem a de baixo o
                    // efeito seria a foto surgir do fundo preto — que é pior
                    // do que o corte seco, porque pisca. Ver
                    // `CRUZAMENTO_DA_FOTO`.
                    .children(
                        self.saindo
                            .clone()
                            .map(|anterior| self.foto_na_vista(anterior)),
                    )
                    .child(match self.saindo.as_ref() {
                        None => self.foto_na_vista(imagem.clone()).into_any_element(),
                        Some(_) => self
                            .foto_na_vista(imagem.clone())
                            .with_animation(
                                // 🚨 O id muda a cada cruzamento: repetido, o
                                // GPUI reaproveita o estado da animação
                                // anterior e a segunda troca nasce no fim.
                                SharedString::from(format!("cruzamento-{}", self.cruzamento)),
                                gpui_kit::Animation::new(CRUZAMENTO_DA_FOTO)
                                    .with_easing(gpui_kit::ease_out_quint()),
                                |foto, quanto| foto.opacity(quanto),
                            )
                            .into_any_element(),
                    })
                    // Ampliada perto do pixel, os pixels nítidos por cima da
                    // textura, que o GPU amplia borrando.
                    .children(self.pixels_nitidos(imagem))
                    .children(self.caixa_de_zoom())
                    .children(self.overlay_de_corte(cx))
                    .children(self.marcacoes_locais(cx))
                    // O `canvas` mede o palco e é onde o arrasto se liga:
                    // registrar ouvinte de mouse exige estar na fase de
                    // pintura, e um `div` comum não chega lá.
                    .child(self.medida_e_arrasto(cx))
                    // A barra de zoom da prévia e o navegador flutuante; no
                    // Enquadrar não há zoom, e o lugar é do transferidor.
                    .when(self.edicao.is_none(), |palco| {
                        palco
                            .child(self.controle_de_zoom(cx))
                            .children(self.navegador_flutuante(cx))
                    })
                    .children(self.folha_de_atalhos(cx)),
                )
                .into_any_element(),
            // 🔑 **Revelando é tela vazia — nunca a foto crua.** É o
            // `invisible` da web (`editor.tsx`: a janela do palco só aparece em
            // `fase: "pronto"`), e é o que faz a foto ter **uma** exibição em
            // vez de duas. As frases abaixo são para quem não tem o que
            // desenhar; aqui há, e falta só o motor devolver.
            Some(_) if self.aguardando.is_some() => moldura.into_any_element(),
            // 🔑 **Duas frases, e a diferença é se há o que esperar.** Enquanto
            // a raiz repõe (do disco, ou da cópia de trabalho do site), o que a
            // tela deve dizer é que a foto está vindo; a frase seca de antes só
            // vale quando ninguém está buscando — e aí ela é a verdade.
            Some(Aberta { foto, .. }) => moldura
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(if self.repondo {
                            format!("Preparando {}…", foto.name)
                        } else {
                            format!("{} não tem preview no cache", foto.name)
                        })),
                )
                .into_any_element(),
        }
    }

    /// Mede o palco e, enquanto há arrasto, escuta o ponteiro.
    ///
    /// 🚨 **`window.on_mouse_event` só vale na fase de pintura** — daí o
    /// `canvas`, e não um `div` com `on_mouse_move`. O ouvinte de um `div` só
    /// recebe evento **dentro** dele; arrastar uma alça para fora da foto (que é
    /// o gesto normal para encolher até a borda) sairia do elemento e o arrasto
    /// morreria no meio, deixando o retângulo preso a meio caminho.
    fn medida_e_arrasto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let medidor = cx.entity();
        let ouvinte = cx.entity();
        let arrastando = self.arrastando_no_corte();

        canvas(
            move |bounds, _window, cx| {
                // Só escreve quando muda: `update` marca a entidade como suja, e
                // gravar o mesmo tamanho a cada quadro redesenharia a tela para
                // sempre.
                medidor.update(cx, |tela, _cx| {
                    if tela.palco != bounds {
                        tela.palco = bounds;
                    }
                });
            },
            move |bounds, _prepaint, window, _cx| {
                // 🚨 **A pinça é ouvida na janela, e não com `on_pinch`**
                // (achado no app real, 2026-09-27): o do elemento exige o
                // palco "sob o mouse", e o GPUI desliga o hover depois de uma
                // tecla até o ponteiro andar — a pinça não anda o ponteiro.
                // Apertar J e pinçar em seguida não ampliava nada.
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &gpui_kit::PinchEvent, fase, _window, cx| {
                        if fase.bubble() && bounds.contains(&evento.position) {
                            esta.update(cx, |tela, cx| tela.ao_pincar(evento, cx));
                        }
                    }
                });
                if !arrastando {
                    return;
                }

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &MouseMoveEvent, fase, window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| {
                            tela.mover_no_corte(evento.position, window, cx)
                        });
                    }
                });

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| tela.soltar_no_corte(window, cx));
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    /// A barra do modo de corte: o que fazer com o retângulo que está na foto.
    ///
    /// Fica no topo do painel, e só existe enquanto o modo está aberto. No legado
    /// ela vive no painel de revelação junto com tudo o mais; aqui aparecer e
    /// sumir é o que diz, sem texto, que a tela está noutro estado.
    fn barra_de_corte(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.edicao.as_ref()?;
        Some(self.painel_de_corte(cx))
    }
}

/// O que a Revelação pede à raiz — os botões que o site tem na barra dele e que
/// só o [`crate::app::Aplicativo`] sabe cumprir.
///
/// ⚠️ O nome é longo porque `Pedido` já é o da GPU
/// ([`super::processador::Pedido`]), e os dois se cruzam neste arquivo.
///
/// 🔑 **Ela não sabe exportar nem falar com o site, e não deve saber.** Exportar
/// abre um modal com pasta de destino; salvar na galeria baixa o original,
/// revela e sobe. As duas moram na raiz — a Revelação só diz "o operador pediu
/// isto daqui".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedidoDaRevelacao {
    /// O `X` da barra: fecha a Revelação e volta de onde se veio.
    Sair,
    /// "Tela do cliente" do site: abre ou fecha a tela do segundo monitor.
    TelaDoCliente,
    /// "Salvar na galeria e sair" do site: o revelado entra no lugar do
    /// original, na foto que já é da galeria aberta.
    SalvarNaGaleria,
    /// "Sincronizar N" do site: os ajustes desta foto vão para as marcadas na
    /// tira — a revelação para o catálogo, e a foto revelada para o site.
    Sincronizar,
    /// "Zerar N fotos": as marcadas na tira voltam ao neutro.
    ///
    /// 🔑 **É a contrapartida do `Sincronizar`** — um leva a revelação desta foto
    /// para as marcadas, o outro devolve todas ao neutro (pedido do dono,
    /// 2026-09-11). A foto aberta não vem por aqui: ela é zerada na própria
    /// tela, por `redefinir_ajustes`, para o `Cmd+Z` desfazer o gesto.
    ZerarAsMarcadas,
    /// "Baixar como…" — o botão da barra, com a foto aberta, e o "Baixar
    /// como… (N)" do menu da tira: a exportação com essas fotos
    /// (`Revelacao::levar_a_baixar`).
    BaixarComo,
    /// "Descartar": as fotos de [`Revelacao::levar_a_descartar`] voltam à
    /// revelação que a galeria do site tem, e saem da fila do "Salvar".
    Descartar,
    /// Outra foto entrou no palco — pela seta, pela tira ou ao abrir.
    ///
    /// 🔑 **A Revelação não sabe buscar na nuvem, e não vai passar a saber**;
    /// mas é ela quem sabe quando a foto trocou. Sem este aviso a raiz só
    /// buscava os pixels da foto do site **na abertura**: a seta seguinte
    /// caía numa foto sem bruto, e o que aparecia era a miniatura revelada da
    /// galeria — em 640px e com a revelação por cima da revelação.
    AbriuOutraFoto,
    /// O zoom passou da cópia de trabalho: a tela quer o bruto da foto aberta
    /// em resolução cheia ([`Revelacao::receber_bruto`]).
    QueroOBruto,
    /// "Editar Foto" do menu da tira: a raiz abre a janela do editor para a
    /// foto de [`Revelacao::levar_a_editar`] — a clicada, e só ela.
    EditarFoto,
    /// "Excluir a edição" do menu da tira, já confirmado: a raiz apaga o
    /// projeto da foto de [`Revelacao::levar_a_excluir`] e a devolve ao bruto.
    ExcluirEdicao,
    /// 💛 O coração ou o arrasto mudou as favoritas: a raiz grava
    /// [`Revelacao::favoritas`] no perfil do usuário, na API.
    GuardarFavoritas,
}

impl gpui_kit::EventEmitter<PedidoDaRevelacao> for Revelacao {}

impl Render for Revelacao {
    /// A tela inteira, no desenho do editor do site (`editor.tsx`).
    ///
    /// # 🚨 Ela era um dock de cinco painéis, e o dono pediu duas vezes que não
    ///
    /// *"o Modo revelação precisa ser exatamente igual a interface da Revelação
    /// WEB"*. O dock dava a cada painel uma **aba com título** — "Foto",
    /// "Ajustes", "Presets" —, divisórias arrastáveis e um arranjo gravado em
    /// disco. Nada disso existe no site, e o efeito somado é outro programa: no
    /// site a foto ocupa a janela e as três colunas são molduras sem nome.
    ///
    /// O que se perde é arrastar painel, e é uma perda escolhida: as posições
    /// eram as mesmas em toda abertura de qualquer jeito, porque revelar é
    /// sempre o mesmo gesto.
    ///
    /// ```text
    /// ┌──────────────────────────────────────────────────┐
    /// │ ✕ ‹ › ▤  3/200 img0042.jpg  METAL   ↶ ↷ Antes ⧉ … │  cabeçalho
    /// ├──────────┬───────────────────────────┬───────────┤
    /// │ presets  │           foto            │  ajustes  │
    /// │  224px   │          flex 1           │   320px   │
    /// ├──────────┴───────────────────────────┴───────────┤
    /// │                     tira                         │
    /// └──────────────────────────────────────────────────┘
    /// ```
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.navegacao.dpr = window.scale_factor();
        self.sincronizar_o_slider_do_zoom(window, cx);
        if self.exibicao_atrasada {
            self.atualizar_exibicao();
        }
        if std::mem::take(&mut self.sliders_atrasados) {
            let graus = self.corte_atual().angle();
            self.angulo
                .update(cx, |estado, cx| estado.set_value(graus, window, cx));
            self.sincronizar_sliders_da_perspectiva(window, cx);
            self.espalhar_nos_sliders(window, cx);
        }
        self.acertar_o_trilho_da_exposicao(window, cx);
        self.acompanhar_a_resolucao(cx);
        if self.docas.is_none() {
            self.montar_as_docas(window, cx);
        }
        let cabecalho = self.cabecalho(window, cx);
        let (esquerda, direita, tira_aberta) =
            self.docas.as_ref().map_or((true, true, true), |d| {
                use crate::docas::Lado;
                (
                    d.aberta(Lado::Esquerda, cx),
                    d.aberta(Lado::Direita, cx),
                    d.tira_aberta(),
                )
            });
        let tira = if tira_aberta {
            self.filmstrip(window, cx)
        } else {
            None
        };
        let area = self.docas.as_ref().map(|d| d.area.clone());
        let seta = |id, lado, aberta, dica, cx: &mut Context<Self>| {
            let tela = cx.entity().downgrade();
            crate::docas::seta(id, lado, aberta, dica, cx, move |_, window, cx| {
                let _ = tela.update(cx, |tela, cx| tela.alternar_coluna(lado, window, cx));
            })
        };
        use crate::docas::Lado;
        let seta_esquerda = seta(
            "revelacao-seta-esquerda",
            Lado::Esquerda,
            esquerda,
            "Mostrar ou esconder as predefinições",
            cx,
        );
        let seta_direita = seta(
            "revelacao-seta-direita",
            Lado::Direita,
            direita,
            "Mostrar ou esconder os ajustes",
            cx,
        );
        let seta_de_baixo = (!self.acervo.is_empty()).then(|| {
            seta(
                "revelacao-seta-da-tira",
                Lado::Baixo,
                tira_aberta,
                "Mostrar ou esconder a tira",
                cx,
            )
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(cabecalho)
            .child(
                // `min_h(0)` na faixa do meio: sem ele, as colunas roláveis de
                // dentro empurram o pai e a rolagem nunca acontece.
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(seta_esquerda)
                    .child(div().flex_1().min_w(px(0.)).h_full().children(area))
                    .child(seta_direita),
            )
            // A seta abaixo da tira, na beirada da janela, como no Lightroom.
            .children(tira)
            .children(seta_de_baixo)
            .children(self.pergunta_de_apagar(window, cx))
    }
}

impl Revelacao {
    /// O dock das três partes: as predefinições, a foto e os ajustes.
    ///
    /// 🔑 **Os nomes são o que o dock guarda** — não mudar depois.
    fn montar_as_docas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::docas::{Docas, Lado, Lateral, Limites};
        use std::rc::Rc;
        let eu = cx.entity();
        let docas = Docas::montar(
            "revelacao",
            &eu,
            (
                "revelacao:palco",
                // 🚨 **Relativo, com a moldura absoluta dentro.** Com
                // `size_full` num item flex sem altura definida, a foto crescia
                // até o tamanho natural e passava por baixo da tira (dono,
                // 2026-09-17: "a foto precisa caber").
                Rc::new(|tela: &mut Self, _window, cx| {
                    div()
                        .relative()
                        .size_full()
                        .child(tela.palco(cx))
                        .into_any_element()
                }),
            ),
            vec![
                (
                    Lado::Esquerda,
                    Lateral {
                        nome: "revelacao:predefinicoes",
                        desenho: Rc::new(|tela: &mut Self, window, cx| {
                            let altura = f32::from(window.viewport_size().height);
                            let coluna = tela.coluna_dos_presets(altura, cx).into_any_element();
                            tela.calada_no_comparar(coluna, cx)
                        }),
                        limites: Limites {
                            minimo: 180.,
                            maximo: 420.,
                            padrao: LADO_DOS_PRESETS,
                        },
                    },
                ),
                (
                    Lado::Direita,
                    Lateral {
                        nome: "revelacao:ajustes",
                        desenho: Rc::new(|tela: &mut Self, _window, cx| {
                            let coluna = div()
                                .size_full()
                                .border_l_1()
                                .border_color(cx.theme().border)
                                .child(tela.painel(cx))
                                .into_any_element();
                            tela.calada_no_comparar(coluna, cx)
                        }),
                        limites: Limites {
                            minimo: 280.,
                            maximo: 560.,
                            padrao: LADO_DO_PAINEL,
                        },
                    },
                ),
            ],
            window,
            cx,
        );
        self.docas = Some(docas);
    }

    /// 🔑 **No Comparar não se revela**: as duas fotos estão em julgamento, e
    /// um slider mexido ali mudaria só a aberta, sem o operador ver. As duas
    /// colunas ficam esmaecidas e sem clique, como no site (`inert`).
    fn calada_no_comparar(
        &self,
        coluna: gpui_kit::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let comparando = self.comparacao.is_some();
        let fundo = cx.theme().background;
        div()
            .relative()
            .size_full()
            .child(coluna)
            .when(comparando, |d| {
                d.child(
                    div()
                        .id("coluna-calada-no-comparar")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(fundo.opacity(0.6)),
                )
            })
            .into_any_element()
    }

    /// A seta de uma borda, o botão do cabeçalho ou a tecla do Lightroom.
    pub fn alternar_coluna(
        &mut self,
        lado: crate::docas::Lado,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(docas) = self.docas.as_ref() {
            docas.alternar(lado, window, cx);
        }
        // A prévia de uma predefinição sob o ponteiro não sobrevive à coluna
        // que some com ela.
        self.prever(None, cx);
        cx.notify();
    }

    /// `Tab` (as duas colunas) e `⇧Tab` (colunas e tira), como no Lightroom.
    pub fn alternar_paineis(&mut self, tudo: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(docas) = self.docas.as_ref() {
            if tudo {
                docas.alternar_tudo(window, cx);
            } else {
                docas.alternar_as_colunas(window, cx);
            }
        }
        self.prever(None, cx);
        cx.notify();
    }

    /// A coluna deste lado está à vista? (Antes do primeiro quadro, sim.)
    pub fn coluna_aberta(&self, lado: crate::docas::Lado, cx: &gpui_kit::App) -> bool {
        self.docas.as_ref().is_none_or(|d| d.aberta(lado, cx))
    }
}

impl Revelacao {
    /// A barra do topo — a do site, na mesma ordem.
    ///
    /// ⚠️ **Ela ocupa a janela inteira**, e não só a largura da foto: no site
    /// não há barra de aplicativo por cima (o editor é `fixed inset-0`), e a
    /// raiz esconde a dela enquanto a Revelação está no ar. Por isso o `✕` daqui
    /// é o único caminho de volta visível — e ele faz o mesmo que o `Esc`.
    fn cabecalho(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use crate::recursos::Icone;
        use gpui_kit::component::Icon;

        // 🔑 "i/n" conta **a tira** (o recorte), como o site: as setas andam
        // por ela.
        let (posicao, total) = self.posicao_na_tira();
        let nome = self
            .foto()
            .map(|foto| foto.name.clone())
            .unwrap_or_default();
        let pronto = self.tem_pixels();
        let pode_revelar = self.pode_revelar();

        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(ALTURA_DO_CABECALHO))
            .flex_none()
            .px(px(12.))
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("revelacao-fechar")
                    .icon(Icon::new(Icone::X))
                    .small()
                    .ghost()
                    .tooltip("Fechar a Revelação (Esc)")
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::Sair);
                    })),
            )
            .child(
                Button::new("revelacao-anterior")
                    .icon(Icon::new(Icone::ChevronLeft))
                    .small()
                    .ghost()
                    .tooltip("Foto anterior (seta para a esquerda)")
                    .disabled(posicao == 0 || total <= 1)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.andar(-1, window, cx))),
            )
            .child(
                Button::new("revelacao-proxima")
                    .icon(Icon::new(Icone::ChevronRight))
                    .small()
                    .ghost()
                    .tooltip("Próxima foto (seta para a direita)")
                    .disabled(total <= 1 || posicao + 1 >= total)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.andar(1, window, cx))),
            )
            .child(
                Button::new("revelacao-presets")
                    .icon(Icon::new(Icone::PanelLeft))
                    .small()
                    .ghost()
                    .tooltip("Mostrar ou esconder as predefinições (Tab esconde as duas colunas)")
                    .selected(self.coluna_aberta(crate::docas::Lado::Esquerda, cx))
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.alternar_coluna(crate::docas::Lado::Esquerda, window, cx);
                    })),
            )
            // 🔑 "3/200" antes do nome, e não só o nome: revelar é trabalho de
            // lote, e a pergunta que se faz a cada foto é "quanto falta".
            .when(total > 0, |barra| {
                barra.child(
                    div()
                        .pl(px(4.))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!("{}/{total}", posicao + 1))),
                )
            })
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_xs()
                    .child(SharedString::from(nome)),
            )
            // O selo do site, ao lado do nome: lá ele diz WEBGPU ou WEBGL2, que
            // rendem diferente; aqui ele responde "a GPU está mesmo sendo usada,
            // e por qual caminho" — a pergunta que aparece toda vez que alguém
            // acha o arrasto lento.
            // 🖌️ "EDITADA": a foto aberta está sendo revelada a partir da
            // imagem do editor, e não do bruto.
            .when(self.revisao_da_aberta() != fonte::DO_BRUTO, |barra| {
                barra.child(
                    div()
                        .id("selo-editada-aberta")
                        .debug_selector(|| "selo-editada-aberta".into())
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .px(px(4.))
                        .py(px(1.))
                        .rounded(crate::tema::canto(3.))
                        .bg(tema::cores::quente().opacity(0.2))
                        .text_xs()
                        .text_color(tema::cores::quente())
                        .child(Icon::new(Icone::Pencil).size(px(10.)))
                        .child("EDITADA")
                        .tooltip(|window, cx| {
                            Tooltip::new(
                                "Editada: a revelação está sendo aplicada sobre a edição. \
                                 Botão direito na tira → Excluir a edição volta ao arquivo bruto.",
                            )
                            .build(window, cx)
                        }),
                )
            })
            .children(self.processador.backend().map(|backend| {
                div()
                    .flex_none()
                    .px(px(4.))
                    .py(px(1.))
                    .rounded(crate::tema::canto(3.))
                    .bg(cx.theme().muted)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(backend.to_uppercase()))
            }))
            .child(div().flex_1().min_w(px(0.)))
            .child(
                Button::new("revelacao-desfazer")
                    .icon(Icon::new(Icone::Undo2))
                    .small()
                    .ghost()
                    .tooltip(SharedString::from(format!(
                        "Desfazer ({}+Z)",
                        tema::modificador()
                    )))
                    .disabled(!self.pode_desfazer())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.desfazer(window, cx))),
            )
            .child(
                Button::new("revelacao-refazer")
                    .icon(Icon::new(Icone::Redo2))
                    .small()
                    .ghost()
                    .tooltip(SharedString::from(format!(
                        "Refazer (Shift+{}+Z)",
                        tema::modificador()
                    )))
                    .disabled(!self.pode_refazer())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.refazer(window, cx))),
            )
            // 🔑 **Segurar, e não alternar**, como no site: soltar o botão (ou
            // sair de cima dele) devolve a foto revelada.
            .child(
                pilula("revelacao-antes", self.mostrando_original, !pronto, cx)
                    .child("Antes")
                    .tooltip("Segure para ver a foto sem ajuste (ou a tecla \\)")
                    .when(pronto, |b| {
                        b.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(true, cx)),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(false, cx)),
                        )
                        // Soltar fora do botão também devolve a revelada. Sem
                        // `on_hover` aqui: o `Button` do kit já registra o dele,
                        // e o GPUI aceita um só por elemento.
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(false, cx)),
                        )
                    }),
            )
            .child(
                pilula(
                    "revelacao-enquadrar",
                    self.cortando(),
                    !pronto || !pode_revelar,
                    cx,
                )
                .child(Icon::new(Icone::Crop).size(px(14.)))
                .child("Enquadrar")
                .tooltip("Girar, espelhar, endireitar e recortar (tecla R)")
                .when(pronto && pode_revelar, |b| {
                    b.on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.prever(None, cx);
                        tela.alternar_corte(window, cx)
                    }))
                }),
            )
            .child(
                pilula("revelacao-tela-do-cliente", self.cliente_aberto, false, cx)
                    .child(
                        Icon::new(if self.cliente_aberto {
                            Icone::MonitorOff
                        } else {
                            Icone::Monitor
                        })
                        .size(px(14.)),
                    )
                    .child("Tela do cliente")
                    .when(self.cliente_aberto, |b| {
                        b.child(crate::cliente::tecla_da_tela_cheia())
                    })
                    .tooltip(if self.cliente_aberto {
                        format!(
                            "Fechar a tela do cliente · {} põe e tira a tela cheia",
                            crate::cliente::texto_da_tela_cheia()
                        )
                    } else {
                        format!(
                            "Abrir a tela do cliente no outro monitor: ela mostra esta foto, revelada, enquanto você ajusta · depois, {} põe e tira a tela cheia",
                            crate::cliente::texto_da_tela_cheia()
                        )
                    })
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::TelaDoCliente);
                    })),
            )
            // "Sincronizar N": só aparece quando há lote — um botão que quase
            // sempre está desligado vira ruído numa barra que já tem sete
            // controles. É o do site, no mesmo lugar.
            .when(self.marcadas.len() > 1, |barra| {
                let quantas = self.marcadas.len();
                barra.child(
                    crate::estilo::botao_contorno_pequeno("revelacao-sincronizar", cx)
                        .icon(Icon::new(Icone::Copy))
                        .label(format!("Sincronizar {quantas}"))
                        .tooltip(
                            "Copiar os ajustes desta foto para as outras escolhidas na tira — elas sobem quando você salvar",
                        )
                        .disabled(!pronto || !pode_revelar)
                        .on_click(cx.listener(|tela, _ev, window, cx| {
                            tela.abrir_sincronizacao(window, cx)
                        })),
                )
            })
            // 🗑️ "Descartar": o caminho de volta do "Salvar" (dono,
            // 27/set/2026) — ver `descartar.rs`.
            .child(self.botao_de_descartar(cx))
            // As duas últimas: "Baixar como…" e "Salvar na galeria e sair".
            // Aqui elas **pedem à raiz**, que é quem tem o modal da exportação
            // e a conversa com o pós-venda.
            //
            // 📥 **O "Baixar" é a exportação** (dono, 10/out/2026: *"tem que
            // herdar as funcionalidades da exportação"*). Era o "Baixar JPEG"
            // do site: um JPEG só, direto na pasta Downloads, sem formato, sem
            // pasta e sem a regra da marca d'água.
            .child(
                crate::estilo::botao_contorno_pequeno("revelacao-exportar", cx)
                    .icon(Icon::new(Icone::Download))
                    .label("Baixar como…")
                    .tooltip(
                        "Baixar esta foto escolhendo formato, qualidade e pasta — a exportação da sessão",
                    )
                    .disabled(!pronto)
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.baixar_a_aberta(window, cx);
                    })),
            )
            .child({
                let BotaoDeSalvar {
                    rotulo,
                    dica,
                    habilitado,
                } = self.botao_de_salvar();
                crate::estilo::botao_primario_pequeno("revelacao-salvar-na-galeria", cx)
                    .icon(Icon::new(Icone::Save))
                    .label(rotulo)
                    .tooltip(dica)
                    .disabled(!habilitado)
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::SalvarNaGaleria);
                    }))
            })
            // 🪟 A Revelação não tem o cabeçalho do app: sem a faixa das guias
            // acima, os botões de janela moram aqui — ou, no GNOME, não havia
            // como fechar nem minimizar com ela aberta (dono, 25/set/2026).
            .child(crate::janela::controles_da_tela(
                "janela-revelacao",
                cx.theme().foreground,
                window,
                cx,
            ))
            .into_any_element()
    }

    /// A coluna da esquerda: só as predefinições, sem título e sem aba.
    ///
    /// 🔑 **A coluna não rola; só as listas** (dono, 2026-09-29): com as vinte
    /// do sistema ela descia inteira, e o navegador e a busca saíam de vista
    /// justamente quando se procura uma predefinição. Cada grupo rola a sua.
    fn coluna_dos_presets(
        &self,
        altura_da_janela: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        gpui_kit::component::v_flex()
            .id("coluna-de-presets")
            .debug_selector(|| "coluna-de-presets".into())
            .size_full()
            .p(px(8.))
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            // O Navegador vem antes das predefinições, como no site.
            .child(div().flex_none().child(self.navegador(cx)))
            .child(self.presets(cx))
            // 📜 O Histórico no pé, como no Lightroom: as predefinições ficam
            // com o resto da altura.
            .child(self.painel_do_historico(altura_da_janela, cx))
    }
}

/// 📏 **Os botões de estado da barra** ("Antes", "Enquadrar", "Tela do
/// cliente"): o `Button` de contorno do kit no tamanho pequeno do template —
/// o mesmo de "Descartar", "Baixar JPEG" e das barras da sessão (dono,
/// 28/09/2026: *"botões fora de padrão"*; eram pílulas desenhadas à mão, sem
/// borda e com letra menor que a dos vizinhos). Ligado, acende no aceso do
/// tema.
fn pilula(id: &'static str, ligada: bool, desligada: bool, cx: &mut Context<Revelacao>) -> Button {
    let botao = crate::estilo::botao_contorno_pequeno(id, cx);
    let botao = if ligada {
        botao.custom(crate::tema::botao_aceso(cx))
    } else {
        botao
    };
    botao.disabled(desligada)
}

/// Ajuste **ou** enquadramento fora do neutro — o `temOQueZerar` do site.
///
/// 🔑 **Uma regra para o botão do painel e para o menu da tira.** O menu usava
/// `ja_revelada`, que compara o corte com o vazio: a foto já zerada (corte
/// inteiro gravado) continuava contando como "a zerar", e o menu reenfileirava
/// um restaurar-original à toa. No site os dois usam a mesma regra.
pub(crate) fn tem_o_que_zerar(foto: &PhotoViewModel) -> bool {
    !persistencia::da_foto(foto).sem_efeito()
        || !corte::e_inteiro(&persistencia::para_crop_settings(
            &persistencia::corte_da_foto(foto),
        ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::biblioteca::miniaturas::Miniatura;
    use crate::campo::TrocarValor as _;
    use crate::revelacao::rodas;
    use biblioteca_core::selecao::Modificadores;

    use gpui_kit::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use domain::entities::preset::PresetAdjustments;

    use super::super::controles::Secao;
    use super::super::lightroom::mentira::EscolhaDeMentira;
    use super::super::persistencia::mentira::GravadorDeMentira;
    use super::super::presets::mentira::GuardaDeMentira;
    use super::super::presets::ordem::Grupo;
    use gpui_kit::App;
    use revelacao_core::locais::Forma;

    fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (
            Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
            dir,
        )
    }

    fn foto_cinza() -> DynamicImage {
        foto_uniforme(100)
    }

    /// Uma foto de um tom só — o que torna previsível o que o histograma dirá.
    fn foto_uniforme(valor: u8) -> DynamicImage {
        let mut img = RgbaImage::new(8, 8);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([valor, valor, valor, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn foto(nome: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            path: format!("/fotos/{nome}"),
            ..Default::default()
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
    ) -> gpui_kit::WindowHandle<Revelacao> {
        com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()))
    }

    fn com_gravador(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
    ) -> gpui_kit::WindowHandle<Revelacao> {
        com_presets(cx, previews, gravador, Vec::new())
    }

    fn com_presets(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
        presets: Vec<Preset>,
    ) -> gpui_kit::WindowHandle<Revelacao> {
        com_guarda(
            cx,
            previews,
            gravador,
            Arc::new(GuardaDeMentira::default()),
            presets,
        )
    }

    fn com_guarda(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
        guarda: Arc<GuardaDeMentira>,
        presets: Vec<Preset>,
    ) -> gpui_kit::WindowHandle<Revelacao> {
        com_escolha(
            cx,
            previews,
            gravador,
            guarda,
            Arc::new(EscolhaDeMentira::default()),
            presets,
        )
    }

    /// O mesmo, com o seletor de arquivos escolhido — para a importação.
    fn com_escolha(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
        guarda: Arc<GuardaDeMentira>,
        escolha: Arc<dyn EscolhaDePresets>,
        presets: Vec<Preset>,
    ) -> gpui_kit::WindowHandle<Revelacao> {
        cx.update(gpui_kit::init);
        cx.add_window(move |window, cx| {
            Revelacao::nova(previews, gravador, guarda, escolha, presets, window, cx)
        })
    }

    /// 🚨 **O palco tem de ter tamanho depois de desenhado.**
    ///
    /// Relatado com a tela na mão em 18/ago/2026: o painel "Foto" aparecia
    /// **vazio**, com o filmstrip abaixo mostrando as miniaturas normalmente.
    ///
    /// A causa é de uma linha: a moldura do palco usava `flex_1()`, que só faz
    /// sentido dentro de um pai flex. Quando a Revelação entrou no dock, ela
    /// virou a **raiz de um painel** — sem pai flex, `flex_1` não cresce, a
    /// altura fica no conteúdo, e o filho `size_full()` vira 100% de zero.
    ///
    /// 🔑 **Nada falha.** O painel existe, tem título, tem área; a foto é
    /// desenhada num retângulo de tamanho zero. Nem o texto de "escolha uma
    /// foto" aparece, porque o caminho com imagem não passa por ele.
    ///
    /// O `canvas` do palco grava as próprias bounds em `self.palco` — é essa
    /// medida que este teste cobra.
    #[gpui_kit::test]
    fn o_palco_tem_tamanho_depois_de_desenhado(cx: &mut TestAppContext) {
        let (previews, dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        let _dir = dir;

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        // Desenhar de verdade: é o `canvas` da fase de pintura que mede, e ele
        // só roda quando a janela é pintada.
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.draw(
            gpui_kit::Point::default(),
            gpui_kit::size(px(1200.), px(800.)),
            |_window, _cx| gpui_kit::Empty,
        );
        visual.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                let palco = tela.palco;
                assert!(
                    palco.size.width > px(0.) && palco.size.height > px(0.),
                    "o palco foi desenhado com tamanho {}x{} — a foto some num retângulo de zero",
                    f32::from(palco.size.width),
                    f32::from(palco.size.height)
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// A coluna diz o espaço da foto: a aba **Adobe RGB**, e nenhuma sRGB ou
    /// RGB (dono, 2/out/2026).
    #[gpui_kit::test]
    fn a_coluna_tem_so_a_aba_adobe_rgb(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-espaco.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("espaco.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1920.), px(1080.)));
        visual.run_until_parked();

        assert!(
            visual.debug_bounds("aba-espaco-Adobe RGB").is_some(),
            "a aba Adobe RGB é desenhada"
        );
        assert!(visual.debug_bounds("aba-espaco-sRGB").is_none());
        assert!(visual.debug_bounds("aba-espaco-RGB").is_none());
    }

    /// 🎞️ O Efeitos no desenho do Lightroom (dono, 2026-09-30, com o print
    /// do painel dele): os dois grupos com título, o rótulo na mesma linha da
    /// barra, e o Estilo numa lista que entra no histórico como um clique.
    #[gpui_kit::test]
    fn o_efeitos_tem_os_grupos_e_o_estilo_do_lightroom(cx: &mut TestAppContext) {
        use super::super::controles::Secao;
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-efeitos.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("efeitos.jpg"), window, cx);
                tela.seguir_o_roteiro_do_painel("abrir Efeitos", cx);
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1920.), px(1080.)));
        visual.run_until_parked();
        janela
            .update(cx, |tela, _, cx| {
                tela.seguir_o_roteiro_do_painel("rolar fim", cx)
            })
            .expect("a janela deve estar aberta");
        visual.run_until_parked();

        let indice = |secao: Secao, rotulo: &str| {
            CONTROLES
                .iter()
                .position(|d| d.secao == secao && d.rotulo == rotulo)
                .expect("o controle existe")
        };
        // O `debug_bounds` quer `&'static str`, e o índice só se sabe aqui.
        let nome = |texto: String| -> &'static str { texto.leak() };
        let vinheta = visual
            .debug_bounds("grupo-Vinheta de corte posterior")
            .expect("o título da vinheta é desenhado");
        let grao = visual
            .debug_bounds("grupo-Granulado")
            .expect("o título do granulado é desenhado");
        assert!(grao.origin.y > vinheta.origin.y, "o Granulado vem depois");
        let darktable = visual
            .debug_bounds("grupo-Vinheta do darktable")
            .expect("o título da vinheta do darktable é desenhado");
        assert!(
            vinheta.origin.y < darktable.origin.y && darktable.origin.y < grao.origin.y,
            "a vinheta do darktable fica entre a pós-corte e o Granulado"
        );

        // Rótulo e barra na mesma linha, o rótulo à esquerda.
        let i = indice(Secao::Vinheta, "Intensidade");
        let rotulo = visual
            .debug_bounds(nome(format!("rotulo-{i}")))
            .expect("o rótulo é desenhado");
        let barra = visual
            .debug_bounds(nome(format!("barra-{i}")))
            .expect("a barra é desenhada");
        assert!(rotulo.right() <= barra.left(), "{rotulo:?} {barra:?}");
        assert!(
            rotulo.top() < barra.bottom() && barra.top() < rotulo.bottom(),
            "rótulo e barra na mesma linha: {rotulo:?} {barra:?}"
        );

        // O Estilo é lista: escolher grava, desfazer devolve — e a lista
        // acompanha o desfazer.
        let estilo = indice(Secao::Vinheta, "Estilo");
        assert!(visual
            .debug_bounds(nome(format!("escolha-{estilo}")))
            .is_some());
        let lista = janela
            .update(cx, |tela, _, _| tela.controles[estilo].escolha.clone())
            .expect("a janela deve estar aberta")
            .expect("o Estilo tem lista");
        lista.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some("Sobreposição de tinta")))
        });
        visual.run_until_parked();
        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.ajustes.pcv_style, 2.0);
                assert!(tela.pode_desfazer(), "a escolha é um passo do histórico");
                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes.pcv_style, 0.0);
            })
            .expect("a janela deve estar aberta");
        assert_eq!(
            lista.read_with(cx, |l, _| l.selected_value().copied()),
            Some("Prioridade de realces")
        );
    }

    /// A barra de zoom anda pela alça (dono, 2026-09-29: *"a barra de zoom
    /// deve ser arrastável"*): o arrasto leva a barra junto, não passa da borda
    /// do palco e o duplo clique na alça a devolve ao canto de baixo.
    #[gpui_kit::test]
    fn a_barra_de_zoom_anda_pela_alca_sem_sair_do_palco(cx: &mut TestAppContext) {
        use gpui_kit::{
            Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point,
        };
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx)
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(800.)));
        visual.run_until_parked();

        let barra = visual
            .debug_bounds("barra-de-zoom")
            .expect("a barra é desenhada");
        let alca = visual
            .debug_bounds("zoom-alca")
            .expect("a alça é desenhada");
        let palco = janela
            .update(cx, |tela, _, _| tela.palco)
            .expect("a janela deve estar aberta");
        let arrastar =
            |visual: &mut gpui_kit::VisualTestContext, de: Point<Pixels>, ate: Point<Pixels>| {
                visual.simulate_event(MouseDownEvent {
                    position: de,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                });
                visual.run_until_parked();
                visual.simulate_event(MouseMoveEvent {
                    position: ate,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                });
                visual.run_until_parked();
                visual.simulate_event(MouseUpEvent {
                    position: ate,
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                });
                visual.run_until_parked();
            };

        // Para cima e para a direita: a barra anda o mesmo que o ponteiro.
        let ate = alca.center() + gpui_kit::point(px(60.), px(-300.));
        arrastar(&mut visual, alca.center(), ate);
        let movida = visual.debug_bounds("barra-de-zoom").expect("a barra segue");
        assert!(
            (f32::from(movida.origin.x - barra.origin.x) - 60.).abs() < 1.5
                && (f32::from(movida.origin.y - barra.origin.y) + 300.).abs() < 1.5,
            "a barra não acompanhou o ponteiro: {barra:?} → {movida:?}"
        );

        // Muito além do canto de cima à esquerda: fica dentro do palco.
        let alca = visual.debug_bounds("zoom-alca").expect("a alça segue");
        arrastar(
            &mut visual,
            alca.center(),
            gpui_kit::point(px(-500.), px(-500.)),
        );
        let no_canto = visual.debug_bounds("barra-de-zoom").expect("a barra segue");
        assert!(
            no_canto.origin.x >= palco.origin.x && no_canto.origin.y >= palco.origin.y,
            "a barra saiu do palco: {no_canto:?} fora de {palco:?}"
        );

        // O duplo clique na alça a devolve ao lugar de sempre.
        let alca = visual.debug_bounds("zoom-alca").expect("a alça segue");
        visual.simulate_event(MouseDownEvent {
            position: alca.center(),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        visual.simulate_event(MouseUpEvent {
            position: alca.center(),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 2,
        });
        visual.run_until_parked();
        let de_volta = visual.debug_bounds("barra-de-zoom").expect("a barra segue");
        assert_eq!(
            de_volta.origin, barra.origin,
            "o duplo clique não a devolveu"
        );
    }

    /// 🎡 A roda da Correção de cores responde ao ponteiro como a do
    /// Lightroom: o clique leva o puck (matiz e saturação), o duplo clique
    /// zera a faixa, e o olho apertado muda **só a prévia** — os ajustes da
    /// foto ficam (dono, 2026-09-30, com o print do painel).
    ///
    /// ⚠️ **Os eventos vão direto aos tratadores**, com as coordenadas que a
    /// roda desenhou: no harness, o `simulate_event` não chega à coluna abaixo
    /// dos painéis (nem o botão do "Ajustar" responde). O caminho do clique de
    /// verdade é conferido no app, por roteiro.
    #[gpui_kit::test]
    fn a_roda_da_correcao_de_cores_move_a_faixa_e_o_olho_so_a_previa(cx: &mut TestAppContext) {
        use gpui_kit::{Modifiers, MouseButton, MouseDownEvent};
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.seguir_o_roteiro_do_painel("fechar Básico", cx);
                tela.seguir_o_roteiro_do_painel("abrir Correção de cores", cx);
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1600.), px(1200.)));
        visual.run_until_parked();

        let roda = visual
            .debug_bounds("roda-sombras")
            .expect("a roda das sombras é desenhada na vista de três");
        visual
            .debug_bounds("olho-sombras")
            .expect("o olho das sombras é desenhado");
        let centro = roda.center();
        let raio = rodas::raio_no_quadro(f32::from(roda.size.width));
        let apertar = |visual: &mut gpui_kit::VisualTestContext, onde, cliques| {
            janela
                .update(visual, |tela, window, cx| {
                    tela.apertar_a_roda(
                        rodas::Faixa::Sombras,
                        &MouseDownEvent {
                            position: onde,
                            button: MouseButton::Left,
                            modifiers: Modifiers::none(),
                            click_count: cliques,
                            first_mouse: false,
                        },
                        window,
                        cx,
                    )
                })
                .expect("a janela deve estar aberta");
        };

        // Metade do raio, para cima: 90°, saturação 50.
        apertar(
            &mut visual,
            centro + gpui_kit::point(px(0.), px(-raio / 2.)),
            1,
        );
        let (matiz, sat, medios, realces) = janela
            .update(cx, |tela, _, _| {
                let a = &tela.ajustes;
                (
                    a.split_shadow_hue,
                    a.split_shadow_sat,
                    a.split_midtone_sat,
                    a.split_highlight_sat,
                )
            })
            .expect("a janela deve estar aberta");
        assert!(
            (matiz - 90.0).abs() <= 1.0 && (sat - 50.0).abs() <= 1.0,
            "o puck não foi para debaixo do ponteiro: {matiz}° {sat}"
        );
        assert_eq!(
            (medios, realces),
            (0.0, 0.0),
            "o clique vazou para outra roda"
        );

        // O olho apertado: a prévia sem as sombras, a foto com.
        let (na_tela, na_foto) = janela
            .update(cx, |tela, _, cx| {
                tela.segurar_o_olho(
                    Some(correcao_de_cores::VerSem::Faixa(rodas::Faixa::Sombras)),
                    cx,
                );
                (
                    tela.ajustes_na_tela().split_shadow_sat,
                    tela.ajustes.split_shadow_sat,
                )
            })
            .expect("a janela deve estar aberta");
        assert_eq!(
            na_tela, 0.0,
            "o olho apertado não tirou as sombras da prévia"
        );
        assert!(
            (na_foto - 50.0).abs() <= 1.0,
            "o olho mexeu na foto: {na_foto}"
        );
        let na_tela = janela
            .update(cx, |tela, _, cx| {
                tela.segurar_o_olho(None, cx);
                tela.ajustes_na_tela().split_shadow_sat
            })
            .expect("a janela deve estar aberta");
        assert!(
            (na_tela - 50.0).abs() <= 1.0,
            "soltar o olho não devolveu a prévia"
        );

        // O duplo clique na roda zera matiz e saturação.
        apertar(&mut visual, centro, 2);
        let depois = janela
            .update(cx, |tela, _, _| {
                (tela.ajustes.split_shadow_hue, tela.ajustes.split_shadow_sat)
            })
            .expect("a janela deve estar aberta");
        assert_eq!(depois, (0.0, 0.0), "o duplo clique não zerou a faixa");

        // Fora do disco e da alça, o clique não pega nada.
        apertar(
            &mut visual,
            centro + gpui_kit::point(px(raio * 3.), px(0.)),
            1,
        );
        let fora = janela
            .update(cx, |tela, _, _| tela.estado_do_painel.arrasto_da_roda)
            .expect("a janela deve estar aberta");
        assert_eq!(fora, None);
    }

    /// 🚨 **"Não tem preview no cache" era um beco sem saída.**
    ///
    /// A foto está catalogada, o JPEG está no disco, e a Revelação parava numa
    /// frase — sem botão, sem recuperação, sem nada acontecendo. Este teste
    /// cobra a lista que a raiz usa para repor: a foto sem cache tem de
    /// aparecer nela, com o caminho do arquivo.
    #[gpui_kit::test]
    fn a_foto_sem_cache_entra_na_lista_de_reposicao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(
                    tela.fotos_a_repor(),
                    vec![APor {
                        foto_id: "id-retrato.jpg".into(),
                        caminho: "/fotos/retrato.jpg".into(),
                    }],
                    "a foto sem preview tem arquivo no disco — há de onde repor"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **As duas entradas, e não uma qualquer.**
    ///
    /// Miniatura e preview são gravados no mesmo passo da importação, mas
    /// apagados por botões diferentes nas Configurações. Quem tem só o preview
    /// grande abre o palco cheio com a tira preta ao lado — e era exatamente
    /// esse o estado que ninguém repunha, porque a única pergunta feita era
    /// sobre o palco.
    #[gpui_kit::test]
    fn so_o_preview_grande_nao_basta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(
                    tela.fotos_a_repor().len(),
                    1,
                    "falta a miniatura: a célula da tira fica preta"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **A ordem é o produto: o palco primeiro, depois para fora.**
    ///
    /// Quem repõe entrega uma por vez. Ordenar pelo acervo faria a foto que
    /// está na tela — a única que alguém está olhando — esperar as anteriores
    /// todas; do centro para fora ela sai primeiro, e as seguintes chegam na
    /// ordem em que a seta vai pedi-las.
    #[gpui_kit::test]
    fn a_reposicao_comeca_pelo_palco_e_vai_para_fora(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![foto("a.jpg"), foto("b.jpg"), foto("c.jpg"), foto("d.jpg")],
                    2,
                    window,
                    cx,
                );
                let ordem: Vec<String> = tela
                    .fotos_a_repor()
                    .into_iter()
                    .map(|a| a.foto_id)
                    .collect();
                assert_eq!(
                    ordem,
                    vec!["id-c.jpg", "id-d.jpg", "id-b.jpg", "id-a.jpg"],
                    "o palco é o `c`; depois dele, as vizinhas para fora"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **A foto do site não tem arquivo neste disco.**
    ///
    /// O bruto dela vem da cópia de trabalho do storage — o passo 11. Mandá-la
    /// ao repositor faria ele abrir um caminho vazio, uma vez por foto, e
    /// encher a tela de avisos por uma reposição que nunca poderia dar certo.
    #[gpui_kit::test]
    fn a_foto_do_site_fica_de_fora_da_reposicao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        id: "site:remota-1".into(),
                        name: "remota.jpg".into(),
                        path: String::new(),
                        pos_venda_foto_id: Some("remota-1".into()),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert!(
                    tela.fotos_a_repor().is_empty(),
                    "sem arquivo aqui, não há o que refazer do disco"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A tira também guardou a ausência.**
    ///
    /// O `CacheDeMiniaturas` é um LRU de resultados, e `Ausente` é um
    /// resultado: sem o esquecimento, a célula continuaria preta depois de a
    /// miniatura voltar ao cache — e a reposição inteira pareceria não ter
    /// acontecido.
    #[gpui_kit::test]
    fn a_miniatura_reposta_faz_a_celula_da_tira_reler(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                // A tira pergunta e guarda "não tem".
                assert!(matches!(
                    tela.miniaturas_da_tira.obter(&previews, "id-retrato.jpg"),
                    Miniatura::Ausente
                ));

                previews
                    .save_thumbnail("id-retrato.jpg", &foto_cinza())
                    .expect("gravar miniatura");
                tela.miniatura_reposta("id-retrato.jpg", cx);

                assert!(
                    matches!(
                        tela.miniaturas_da_tira.obter(&previews, "id-retrato.jpg"),
                        Miniatura::Pronta(_)
                    ),
                    "a célula ficou presa no `Ausente` que ela tinha guardado"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Trocar de foto não pode custar nada quando a tira já está pronta.**
    ///
    /// A tarefa que carrega a tira recomeça a cada troca de foto — de propósito,
    /// para reordenar a partir do palco novo. Ela perguntava "esta falta?" de
    /// dentro dela mesma, e cada pergunta era um `esta.update`: um salto
    /// agendado na thread principal, que só corre entre quadros. Com a tira
    /// cheia, andar uma foto custava um salto **por foto do ensaio** só para
    /// descobrir que não havia nada a fazer, e trocar de foto ficou
    /// "extremamente lento" sem que nada do que desenha tivesse mudado.
    #[gpui_kit::test]
    fn a_tira_carregada_nao_pede_nada_ao_trocar_de_foto(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["a.jpg", "b.jpg", "c.jpg"] {
            previews
                .save_thumbnail(&format!("id-{nome}"), &foto_cinza())
                .expect("gravar miniatura");
        }
        let janela = janela(cx, previews.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![foto("a.jpg"), foto("b.jpg"), foto("c.jpg")],
                    0,
                    window,
                    cx,
                );
                assert_eq!(
                    tela.miniaturas_faltando().len(),
                    3,
                    "ao abrir, nenhuma foi lida ainda"
                );

                // A tira acaba de carregar — é o que a tarefa faz, uma por uma.
                for nome in ["a.jpg", "b.jpg", "c.jpg"] {
                    let id = format!("id-{nome}");
                    tela.miniaturas_da_tira.obter(&previews, &id);
                }

                tela.andar(1, window, cx);
                assert!(
                    tela.miniaturas_faltando().is_empty(),
                    "a seta voltou a pedir o que já estava na tira: {:?}",
                    tela.miniaturas_faltando()
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// A régua do quadro da Revelação — **o quadro inteiro, com janela**.
    ///
    /// As outras medidas (`medir-revelacao`) contam pixels: quanto custa
    /// decodificar, converter, medir o histograma. Elas não veem o que o GPUI
    /// faz depois — montar a árvore de elementos, medir e pintar. E é aí que
    /// mora a pergunta que sobrou: *"os controles de edição não estão fluidos"*,
    /// com o caminho dos pixels já dentro do orçamento.
    ///
    /// Um arrasto de slider marca a tela suja a cada resultado da GPU, e o
    /// quadro seguinte **remonta tudo**: 53 sliders, a coluna de predefinições,
    /// o palco e uma célula de tira por foto do ensaio.
    ///
    /// ```bash
    /// cargo test --release -p ui-gpui -- --ignored --nocapture medir_o_quadro
    /// ```
    #[gpui_kit::test]
    #[ignore = "régua, não asserção — roda à mão com --nocapture"]
    fn medir_o_quadro_da_revelacao(cx: &mut TestAppContext) {
        const QUADROS: usize = 40;

        // Uma foto do tamanho que a Revelação abre de verdade.
        let (previews, _dir) = previews_descartaveis();
        let grande = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            2560,
            1707,
            image::Rgb([120, 90, 60]),
        ));
        for i in 0..125 {
            previews
                .save_thumbnail(&format!("id-f{i}.jpg"), &foto_cinza())
                .expect("gravar miniatura");
        }
        previews
            .save_preview("id-f0.jpg", &grande)
            .expect("gravar preview");

        for (rotulo, quantas) in [("tira de 125", 125usize), ("tira de 1", 1)] {
            let janela = janela(cx, previews.clone());
            janela
                .update(cx, |tela, window, cx| {
                    let acervo: Vec<PhotoViewModel> =
                        (0..quantas).map(|i| foto(&format!("f{i}.jpg"))).collect();
                    tela.abrir_no_acervo(acervo, 0, window, cx);
                })
                .expect("a janela deve estar aberta");

            let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
            visual.draw(
                gpui_kit::Point::default(),
                gpui_kit::size(px(2000.), px(1300.)),
                |_window, _cx| gpui_kit::Empty,
            );
            visual.run_until_parked();

            let inicio = std::time::Instant::now();
            for _ in 0..QUADROS {
                // O que um arrasto faz: marca sujo e o quadro remonta tudo.
                janela
                    .update(cx, |_tela, _window, cx| cx.notify())
                    .expect("a janela deve estar aberta");
                visual.draw(
                    gpui_kit::Point::default(),
                    gpui_kit::size(px(2000.), px(1300.)),
                    |_window, _cx| gpui_kit::Empty,
                );
            }
            let por_quadro = inicio.elapsed().as_secs_f64() * 1000.0 / QUADROS as f64;

            println!(
                "{} {rotulo}: {por_quadro:.2} ms por quadro  (orçamento 60fps: 16,7 ms)",
                if por_quadro > 16.7 { "🚨" } else { "✅" }
            );
        }
    }

    /// 🚨 **Troca seca de imagem lê-se como engasgo, não como resultado.**
    ///
    /// É UX antes de ser desempenho, e foi o que sobrou depois de o quadro estar
    /// medido em 3,94 ms e o passo dos sliders consertado: aplicar uma
    /// predefinição ou desfazer substitui a foto inteira de um quadro para o
    /// outro, e o olho lê o salto como travada. O palco guarda a foto que sai
    /// para desenhar as duas empilhadas enquanto a nova ganha opacidade.
    #[gpui_kit::test]
    fn desfazer_cruza_a_foto_em_vez_de_trocar_seca(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert!(
                    tela.saindo.is_none(),
                    "abrir não cruza: não há foto anterior na tela"
                );

                // Um gesto, e o desfazer dele.
                tela.ajustes.exposure = 1.5;
                tela.pedir_revelacao(cx);
                tela.historico.registrar(tela.estado());
                assert!(
                    tela.saindo.is_none(),
                    "o arrasto troca direto: cruzar deixaria fantasma"
                );

                tela.desfazer(window, cx);
                assert!(
                    tela.saindo.is_some(),
                    "desfazer trocou a foto sem cruzamento"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **Cada cruzamento precisa de um id próprio.**
    ///
    /// O `ElementId` da animação é o que o GPUI usa para achar o estado dela.
    /// Repetido, a segunda troca nasce no fim da animação — sem erro nenhum, e
    /// com a impressão de que o efeito "às vezes não funciona".
    #[gpui_kit::test]
    fn cada_cruzamento_tem_um_id_proprio(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.ajustes.exposure = 1.5;
                tela.historico.registrar(tela.estado());

                tela.desfazer(window, cx);
                let primeiro = tela.cruzamento;
                tela.refazer(window, cx);

                assert_ne!(
                    tela.cruzamento, primeiro,
                    "dois cruzamentos com o mesmo id: o segundo nasce pronto"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A tira mostrava a foto de antes enquanto o palco mostrava a de
    /// depois.**
    ///
    /// O operador deixa a foto em preto e branco no palco e a célula da tira
    /// continua colorida, lado a lado, na mesma tela — com a imagem que o
    /// importador gravou. Numa sequência de vinte fotos a tira é o que diz onde
    /// ele parou, e era a única coisa ali que não acompanhava o trabalho.
    #[gpui_kit::test]
    fn a_tira_passa_a_mostrar_a_foto_revelada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        // Sem miniatura no cache: a célula nasce vazia, e é o que separa
        // "a tira leu do disco" de "a tira recebeu o revelado".
        let janela = janela(cx, previews.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.miniaturas_da_tira.obter(&previews, "id-retrato.jpg");
                assert!(
                    matches!(
                        tela.miniaturas_da_tira.espiar("id-retrato.jpg"),
                        Some(Miniatura::Ausente)
                    ),
                    "o cache não tem miniatura desta foto"
                );

                // O que a GPU devolve ao fim de um gesto.
                if let Some(aberta) = tela.aberta.as_mut() {
                    aberta.revelada = Some(foto_cinza());
                }
                tela.atualizar_a_tira_com_o_revelado();

                assert!(
                    matches!(
                        tela.miniaturas_da_tira.espiar("id-retrato.jpg"),
                        Some(Miniatura::Pronta(_))
                    ),
                    "a célula da tira não recebeu a foto revelada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Esquecer uma miniatura sem mandar reler deixa "sem prévia" na
    /// tela** (dono, 18/set/2026, logo depois de clicar em "Sincronizar 22":
    /// *"ficou sem prévia nas miniaturas do filmstrip, e só atualiza quando eu
    /// clico em qualquer outra foto"*).
    ///
    /// O quadro só **lê** o cache da tira — quem o enche é `carregar_a_tira`.
    /// Quando a prévia local de um lote inteiro é trocada, cada célula esquecida
    /// desenha o vazio até algo disparar o carregamento; o que disparava era
    /// trocar de foto, que é o gesto que o operador não tinha por que fazer.
    #[gpui_kit::test]
    fn esquecer_a_miniatura_manda_reler_na_mesma_passada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["retrato.jpg", "paisagem.jpg"] {
            previews
                .save_preview(&format!("id-{nome}"), &foto_cinza())
                .expect("gravar preview");
            previews
                .save_thumbnail(&format!("id-{nome}"), &foto_cinza())
                .expect("gravar miniatura");
        }
        let janela = janela(cx, previews.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![foto("retrato.jpg"), foto("paisagem.jpg")],
                    0,
                    window,
                    cx,
                );
                tela.carregar_a_tira(cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(
                    matches!(
                        tela.miniaturas_da_tira.espiar("id-paisagem.jpg"),
                        Some(Miniatura::Pronta(_))
                    ),
                    "a tira carregou as duas"
                );
            })
            .expect("a janela deve estar aberta");

        // O que o "Sincronizar" e o "Zerar N" fazem em cada foto do lote.
        janela
            .update(cx, |tela, _window, cx| {
                tela.miniatura_reposta("id-paisagem.jpg", cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(
                    matches!(
                        tela.miniaturas_da_tira.espiar("id-paisagem.jpg"),
                        Some(Miniatura::Pronta(_))
                    ),
                    "a célula releu sozinha — sem isto ela fica 'sem prévia'"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Passa da espera do salvamento, sem esperar de verdade.
    fn passar_a_espera(cx: &mut TestAppContext) {
        cx.executor().advance_clock(ESPERA_DA_GRAVACAO * 2);
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn abrir_traz_o_preview_do_cache(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("retrato.jpg"));
                assert!(tela.aberta.as_ref().unwrap().desenhada.is_some());
                assert!(tela.aberta.as_ref().unwrap().origem.is_some());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A foto do site não usa a miniatura da galeria como origem.**
    ///
    /// O que a grade da sessão guarda em `site:<id>` é a miniatura da galeria —
    /// que, depois de "Salvar na galeria", é a foto **revelada**. Servir isso ao
    /// shader aplicava a revelação duas vezes, em 640px: a sépia salva ontem
    /// aparecia com os sliders no neutro, e "sincronizar" a partir dela mandava
    /// o neutro às outras. A miniatura fica só como espera na tela; a origem
    /// chega pela cópia de trabalho (`receber_pixels`).
    #[gpui_kit::test]
    fn a_foto_do_site_espera_a_copia_de_trabalho_em_vez_de_usar_a_miniatura(
        cx: &mut TestAppContext,
    ) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("site:remota-1", &foto_cinza())
            .expect("gravar a miniatura da galeria");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        id: "site:remota-1".into(),
                        name: "DSC_001.jpg".into(),
                        pos_venda_foto_id: Some("remota-1".into()),
                        ..Default::default()
                    },
                    window,
                    cx,
                );

                assert!(
                    !tela.tem_pixels(),
                    "a miniatura da galeria não é origem — é o que faz a raiz buscar o bruto"
                );
                assert!(
                    tela.aberta.as_ref().unwrap().desenhada.is_some(),
                    "mas ela aparece enquanto o bruto não chega"
                );

                assert!(tela.receber_pixels("site:remota-1", foto_cinza(), cx));
                assert!(tela.tem_pixels(), "a cópia de trabalho é a origem");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A foto revelada em outro computador aparece uma vez só** (dono,
    /// 30/09/2026: *"a foto é carregada e depois dá uma piscada"*).
    ///
    /// Enquanto a cópia de trabalho não chega, a espera é a imagem da galeria —
    /// que já vem revelada **e enquadrada** do site (C15). Havia duas falhas
    /// nesse caminho: o corte entrava de novo por cima dela, e a chegada da
    /// cópia apagava o palco até o motor responder. Agora ela aparece como veio
    /// e só sai quando a revelada entra no lugar.
    #[gpui_kit::test]
    fn a_foto_do_site_revelada_nao_pisca_quando_a_copia_chega(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let galeria =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 8, Rgba([100, 100, 100, 255])));
        previews
            .save_preview("site:remota-1", &galeria)
            .expect("gravar a imagem da galeria");
        previews
            .save_thumbnail("site:remota-1", &galeria)
            .expect("gravar a miniatura da galeria");

        let janela = janela(cx, previews.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        id: "site:remota-1".into(),
                        name: "DSC_001.jpg".into(),
                        pos_venda_foto_id: Some("remota-1".into()),
                        edit_exposure: Some(1.5),
                        edit_crop_x: Some(0.0),
                        edit_crop_y: Some(0.0),
                        edit_crop_width: Some(0.5),
                        edit_crop_height: Some(1.0),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                let largura_na_tela = |tela: &Revelacao| {
                    tela.aberta
                        .as_ref()
                        .unwrap()
                        .desenhada
                        .as_ref()
                        .map(|imagem| imagem.size(0).width.0)
                };

                assert_eq!(
                    largura_na_tela(tela),
                    Some(16),
                    "a imagem da galeria já vem enquadrada: cortar de novo a encolhe"
                );
                assert!(matches!(
                    tela.miniaturas_da_tira.obter(&previews, "site:remota-1"),
                    Miniatura::Pronta(_)
                ));

                let copia = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                    32,
                    16,
                    Rgba([90, 90, 90, 255]),
                ));
                assert!(tela.receber_pixels("site:remota-1", copia, cx));
                assert!(
                    tela.aguardando.is_some(),
                    "a revelação da cópia está a caminho"
                );
                assert_eq!(
                    largura_na_tela(tela),
                    Some(16),
                    "a espera fica no palco até o motor responder — sem quadro vazio"
                );
                assert!(
                    matches!(
                        tela.miniaturas_da_tira.espiar("site:remota-1"),
                        Some(Miniatura::Pronta(_))
                    ),
                    "e a célula da tira não volta a \"sem prévia\" no meio"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A segunda abertura da foto do site não pede pixels de novo.**
    ///
    /// A cópia de trabalho que a raiz baixou fica em `trabalho:<id>` — chave
    /// separada da miniatura da galeria, que continua sendo outra imagem. Sem
    /// isso, cada seta era um download: foi o *"voltou a ficar lento"* de
    /// 8/set, um dia depois de o cache ser desligado para esta foto.
    #[gpui_kit::test]
    fn a_copia_de_trabalho_do_site_fica_no_cache_e_a_volta_nao_baixa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        // A imagem da galeria, na chave da grade: é a revelada, e não serve de
        // origem — se ela vazar para o shader, a revelação entra duas vezes.
        previews
            .save_preview("site:remota-1", &foto_cinza())
            .expect("gravar a miniatura da galeria");
        // E a cópia de trabalho, na chave do bruto: é esta que vale.
        previews
            .save_preview(
                &persistencia::chave_do_trabalho("site:remota-1"),
                &foto_uniforme(200),
            )
            .expect("gravar a copia de trabalho");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        id: "site:remota-1".into(),
                        name: "DSC_001.jpg".into(),
                        pos_venda_foto_id: Some("remota-1".into()),
                        ..Default::default()
                    },
                    window,
                    cx,
                );

                assert!(
                    tela.tem_pixels(),
                    "com a copia de trabalho no cache, a raiz nao precisa buscar nada"
                );
                // 200 é a cópia de trabalho; 100 seria a imagem da galeria. A
                // folga é do JPEG do cache, que não devolve o byte exato.
                let origem = tela.aberta.as_ref().unwrap().origem.as_ref().unwrap();
                assert!(
                    origem.pixels[0].abs_diff(200) < 5,
                    "a origem e a copia de trabalho, nao a imagem da galeria: {}",
                    origem.pixels[0]
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Foto sem nada no cache abre assim mesmo — e sem origem para revelar.
    #[gpui_kit::test]
    fn foto_sem_cache_abre_sem_imagem(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("sem-cache.NEF"), window, cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("sem-cache.NEF"));
                assert!(tela.aberta.as_ref().unwrap().desenhada.is_none());
                assert!(tela.aberta.as_ref().unwrap().origem.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// Simula um arrasto: o `Change` que o `Slider` emite ao ser puxado.
    ///
    /// 🚨 **`SliderState::set_value` não serve para isso — ele não emite.** Só
    /// `update_value_by_position`, o caminho do ponteiro, publica `Change`.
    /// Emitir o evento à mão é o que exercita a inscrição de verdade; usar
    /// `set_value` daria um teste que passa com o assinante morto.
    fn arrastar(
        cx: &mut TestAppContext,
        janela: &gpui_kit::WindowHandle<Revelacao>,
        controle: usize,
        valor: f32,
    ) {
        janela
            .update(cx, |tela, _window, cx| {
                let estado = tela.controles[controle].estado.clone();
                estado.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(
                        gpui_kit::component::slider::SliderValue::Single(valor),
                    ));
                });
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
    }

    /// 🚨 Mexer no slider chega até os `Ajustes`.
    ///
    /// É a mesma solda do campo de busca, e o mesmo modo de falha: são 11
    /// `Subscription` guardadas num `Vec`, e descartá-las faz **todos** os
    /// sliders se moverem sem mover a foto — sem erro, sem aviso, arrastando
    /// normalmente.
    #[gpui_kit::test]
    fn arrastar_o_slider_escreve_nos_ajustes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.ajustes().exposure, 1.5);
                assert!(
                    tela.aguardando.is_some(),
                    "mexer no slider tem de virar pedido à GPU"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Uma exibição só, e já revelada** (dono, 17/set/2026: *"primeiro
    /// mostra sem efeito e depois é aplicado a revelação"*).
    ///
    /// A tela desenhava a foto crua na abertura e a trocava pelo resultado do
    /// motor alguns milissegundos depois. Duas exibições da mesma foto, e a
    /// primeira mentindo sobre como ela está revelada — na web isso não
    /// acontece, porque lá o palco só aparece depois de `aplicar` (`editor.tsx`,
    /// `fase: "pronto"`).
    ///
    /// ⚠️ **A crua continua guardada**: ela é o "antes" do `\`, e some da tela
    /// sem sair da memória.
    #[gpui_kit::test]
    fn abrir_uma_foto_revelada_nao_mostra_a_crua_antes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.5),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );

                let aberta = tela.aberta.as_ref().expect("a foto abriu");
                assert!(
                    aberta.bruta.is_some(),
                    "a crua fica guardada — é o `\\` do antes/depois"
                );
                assert!(
                    aberta.revelada.is_none(),
                    "a crua não pode ocupar o lugar da revelada"
                );
                assert!(
                    aberta.desenhada.is_none(),
                    "e nada vai ao palco antes de o motor responder"
                );
                assert!(tela.aguardando.is_some(), "a revelação está a caminho");
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **No neutro a foto aparece na hora.** Sem revelação a aplicar, o
    /// resultado do motor é a própria origem: segurar a tela ali seria um palco
    /// preto em troca de nada — e é o que separa esta regra de "nunca desenhar
    /// a crua".
    #[gpui_kit::test]
    fn no_neutro_a_foto_aparece_sem_esperar_a_gpu(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);

                assert!(tela.aguardando.is_none(), "não há o que pedir no neutro");
                assert!(
                    tela.aberta.as_ref().unwrap().desenhada.is_some(),
                    "e por isso a foto já está na tela"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Ajustes no neutro com Revelação local não é neutro** (achado no
    /// app real, 2026-09-26): a abertura olhava só os ajustes, a GPU nem era
    /// chamada, e a foto com máscara aparecia crua até alguém mexer num slider.
    #[gpui_kit::test]
    fn a_foto_so_com_mascara_abre_pedindo_a_gpu(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        let parametros = r#"{"versao":1,"camadas":[{"nome":"M","ajustes":{"exposicao_ev":1.0},
            "componentes":[{"modo":"somar","tipo":"radial","centro":[0.5,0.5],"raio_x":0.2,
            "raio_y":0.2,"angulo":0.0,"feather":0.5,"fora":false}],"invertida":false}],"retoques":[]}"#;
        let com_mascara = PhotoViewModel {
            locais: Some(parametros.to_string()),
            ..foto("retrato.jpg")
        };
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(com_mascara, window, cx);
                assert_eq!(tela.locais.camadas.len(), 1, "a revelação foi lida");
                assert!(tela.aguardando.is_some(), "a máscara vai à GPU na abertura");
                assert!(
                    tela.aberta.as_ref().unwrap().revelada.is_none(),
                    "e a crua não passa por revelada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **"Zerar tudo" tira as máscaras também**, num passo só (achado no
    /// app real, 2026-09-26: com máscara e ajustes no neutro, o botão ficava
    /// apagado e o cabeçalho dizia "Nenhum ajuste fora do neutro").
    #[gpui_kit::test]
    fn zerar_tudo_leva_a_revelacao_local_num_passo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        let parametros = r#"{"versao":1,"camadas":[{"nome":"M","ajustes":{"exposicao_ev":1.0},
            "componentes":[{"modo":"somar","tipo":"radial","centro":[0.5,0.5],"raio_x":0.2,
            "raio_y":0.2,"angulo":0.0,"feather":0.5,"fora":false}],"invertida":false}],"retoques":[]}"#;
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        locais: Some(parametros.to_string()),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );
                assert!(tela.tem_revelacao_local());
                tela.zerar_tudo(window, cx);
                assert!(tela.locais.camadas.is_empty(), "as máscaras saem");
                tela.desfazer(window, cx);
                assert_eq!(tela.locais.camadas.len(), 1, "e um ⌘Z as devolve");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A revelação já feita não é feita de novo.**
    ///
    /// Voltar uma seta, desfazer, tirar o ponteiro de cima de uma predefinição:
    /// nos três a revelação é uma que já passou pelo motor. Com o palco esperando
    /// a resposta para desenhar, cada ida repetida é tempo de tela preta — e o
    /// pixel que volta é igual ao que já se tinha.
    ///
    /// Aqui a revelação é posta no cache à mão: o que se prova é o caminho, e
    /// não a GPU (que nem sempre existe em quem roda `cargo test`).
    #[gpui_kit::test]
    fn a_revelacao_guardada_dispensa_a_gpu(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        let com_parametros = PhotoViewModel {
            edit_exposure: Some(1.5),
            ..foto("retrato.jpg")
        };

        janela
            .update(cx, |tela, _window, _cx| {
                // A revelação desta foto com esta revelação, como se o motor já a
                // tivesse devolvido. `foto_cinza` tem 8x8, que é o tamanho da
                // origem que `abrir` vai montar.
                let chave = cache::Chave::nova(
                    &com_parametros.id,
                    (8, 8),
                    &persistencia::da_foto(&com_parametros),
                    &transformacao::corte(&persistencia::para_crop_settings(
                        &persistencia::corte_da_foto(&com_parametros),
                    )),
                    &Default::default(),
                );
                assert!(tela.reveladas.guardar(chave, &foto_uniforme(200)));
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(com_parametros.clone(), window, cx);

                assert!(
                    tela.aguardando.is_none(),
                    "com a revelação guardada não há o que pedir à GPU"
                );
                assert!(
                    tela.aberta.as_ref().unwrap().desenhada.is_some(),
                    "e a foto aparece no mesmo instante, sem palco vazio"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **A revelação faz parte da chave.** Guardado com uma, pedido com outra:
    /// é miss, e a GPU é chamada. Sem isto, mexer num slider e voltar à foto
    /// traria a revelação de antes do gesto — certa na aparência, errada no
    /// conteúdo.
    #[gpui_kit::test]
    fn a_revelacao_guardada_com_outros_parametros_nao_serve(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        let com_parametros = PhotoViewModel {
            edit_exposure: Some(1.5),
            ..foto("retrato.jpg")
        };

        janela
            .update(cx, |tela, window, cx| {
                let mut outra = persistencia::da_foto(&com_parametros);
                outra.exposure = -1.0;
                let chave = cache::Chave::nova(
                    &com_parametros.id,
                    (8, 8),
                    &outra,
                    &transformacao::corte(&persistencia::para_crop_settings(
                        &persistencia::corte_da_foto(&com_parametros),
                    )),
                    &Default::default(),
                );
                tela.reveladas.guardar(chave, &foto_uniforme(200));

                tela.abrir(com_parametros.clone(), window, cx);

                assert!(
                    tela.aguardando.is_some(),
                    "a revelação é outra: o cache não pode responder por ela"
                );
                assert!(tela.aberta.as_ref().unwrap().desenhada.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **O resultado atrasado da prévia não pinta por cima.**
    ///
    /// Passar o ponteiro por uma predefinição manda um pedido à GPU; sair da
    /// linha volta à revelação que já está no cache, na hora. O pedido da
    /// prévia continuava no ar, e quando chegava pintava o preset na foto — e
    /// ele ficava ali (dono, 30/set/2026: *"só de passar pelo preset o efeito é
    /// aplicado"*, sobretudo arrastando o zoom, que religa a colheita).
    #[gpui_kit::test]
    fn resultado_atrasado_da_previa_nao_pinta_por_cima(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        let com_parametros = PhotoViewModel {
            edit_exposure: Some(1.5),
            ..foto("retrato.jpg")
        };
        let preset = Preset::system(
            "Sépia à moda antiga",
            PresetAdjustments::vazia().com("saturation", -1.0),
        );

        janela
            .update(cx, |tela, window, cx| {
                let chave = cache::Chave::nova(
                    &com_parametros.id,
                    (8, 8),
                    &persistencia::da_foto(&com_parametros),
                    &transformacao::corte(&persistencia::para_crop_settings(
                        &persistencia::corte_da_foto(&com_parametros),
                    )),
                    &Default::default(),
                );
                tela.reveladas.guardar(chave, &foto_uniforme(200));
                tela.abrir(com_parametros.clone(), window, cx);
                assert!(tela.aguardando.is_none(), "a foto sai do cache");

                tela.prever(Some(&preset), cx);
                let em_voo = tela.aguardando.expect("a prévia vai à GPU");

                tela.prever(None, cx);
                assert!(tela.aguardando.is_none(), "a volta sai do cache");
                assert!(
                    tela.descartar_ate >= em_voo,
                    "o resultado da prévia, quando chegar, não pode ir ao palco"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A próxima foto é revelada antes de a seta chegar nela.**
    ///
    /// É a outra metade do pedido do dono (17/set/2026). Sem isto, o cache só
    /// paga a **segunda** visita — e quem percorre uma sessão inteira pela seta
    /// nunca chega à segunda.
    ///
    /// ⚠️ Prova o **pedido**, e não o resultado: o resultado depende de haver
    /// GPU, e o que esta tela controla é enfileirar o trabalho certo na hora
    /// certa.
    #[gpui_kit::test]
    fn a_proxima_foto_e_revelada_antes_da_seta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(nome, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![
                        // A aberta no neutro: ela não pede nada à GPU, e é o que
                        // deixa a fila livre para a seguinte.
                        foto("a.jpg"),
                        PhotoViewModel {
                            edit_exposure: Some(1.5),
                            ..foto("b.jpg")
                        },
                    ],
                    0,
                    window,
                    cx,
                );
                assert!(tela.aguardando.is_none(), "a aberta está no neutro");
            })
            .expect("a janela deve estar aberta");

        // A decodificação da próxima vai para o executor de fundo.
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(
                    tela.antecipando.is_some(),
                    "a próxima tinha revelação e devia estar sendo revelada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Trocar de sessão não esvazia a tira da outra** (dono, 24/set/2026:
    /// a piscada no filmstrip ao trocar de guia). A sessão de uma foto
    /// encolhia o cache para uma vaga, e as miniaturas da guia de antes eram
    /// relidas do disco na volta.
    #[gpui_kit::test]
    fn a_sessao_pequena_nao_joga_fora_as_miniaturas_da_outra(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["id-a.jpg", "id-b.jpg", "id-c.jpg"] {
            previews
                .save_thumbnail(nome, &foto_cinza())
                .expect("gravar");
        }
        let janela = janela(cx, previews.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![foto("a.jpg"), foto("b.jpg")], 0, window, cx);
                for id in ["id-a.jpg", "id-b.jpg"] {
                    assert!(matches!(
                        tela.miniaturas_da_tira.obter(&previews, id),
                        Miniatura::Pronta(_)
                    ));
                }
                // A outra guia: uma sessão de uma foto, desenhada.
                tela.abrir_no_acervo(vec![foto("c.jpg")], 0, window, cx);
                assert!(tela.filmstrip(window, cx).is_some());
                // E a volta encontra as duas ainda prontas.
                tela.abrir_no_acervo(vec![foto("a.jpg"), foto("b.jpg")], 0, window, cx);
                for id in ["id-a.jpg", "id-b.jpg"] {
                    assert!(
                        matches!(
                            tela.miniaturas_da_tira.espiar(id),
                            Some(Miniatura::Pronta(_))
                        ),
                        "{id} saiu do cache"
                    );
                }
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A tira fica com uma foto só** (dono, 24/set/2026): com as guias,
    /// a sessão de uma foto ao lado da de oito trocava a moldura da Revelação
    /// a cada troca de guia. O site desenha a tira sempre.
    #[gpui_kit::test]
    fn a_tira_fica_com_uma_foto_so(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-a.jpg", &foto_cinza())
            .expect("gravar");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![foto("a.jpg")], 0, window, cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.filmstrip(window, cx).is_some());
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **No neutro não há o que antecipar.** Sem revelação a foto aparece
    /// direto da prévia: adiantar a ida à GPU seria gastar a placa para poupar
    /// uma ida que não existe.
    #[gpui_kit::test]
    fn a_proxima_no_neutro_nao_vira_pedido(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(nome, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![foto("a.jpg"), foto("b.jpg")], 0, window, cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.antecipando.is_none());
                assert!(tela.aguardando.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Abrir uma foto já revelada traz a revelação dela.
    ///
    /// Sem isto, toda foto abre no neutro — inclusive as que o fotógrafo já
    /// trabalhou. Não é "faltou uma tela": é o trabalho dele sumindo da vista,
    /// com o arquivo cru na frente. E o painel diria a mesma mentira, com os 42
    /// sliders parados no meio.
    #[gpui_kit::test]
    fn abrir_uma_foto_ja_revelada_traz_os_ajustes_dela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.5),
                        edit_saturation: Some(-0.4),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );

                assert_eq!(tela.ajustes().exposure, 1.5);
                assert_eq!(tela.ajustes().saturation, -0.4);
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    1.5,
                    "o slider tem de abrir onde o ajuste está — senão a barra mente sobre a foto"
                );
                assert!(
                    tela.aguardando.is_some(),
                    "sem este pedido a foto nunca vira a revelada"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Abrir uma foto **sem edição** não dispara os 42 assinantes.
    ///
    /// `abrir` chama `set_value` em todos os controles, e isso só é barato porque
    /// `set_value` **não emite** `Change`. É o oposto do `InputState::set_value`
    /// da busca, que emite — mesmo nome, dois comportamentos, na mesma
    /// biblioteca.
    ///
    /// Este teste prende essa diferença, e ela ficou mais cara desde que a foto
    /// traz os ajustes do banco: se uma versão nova do `gpui-component` fizer o
    /// slider passar a emitir, abrir uma foto revelada viraria 42 pedidos à GPU —
    /// um por campo, cada um com a foto meio carregada — em vez do único que
    /// `abrir` faz de propósito. O sintoma seria a Revelação demorar para abrir,
    /// sem nenhuma pista do porquê.
    #[gpui_kit::test]
    fn abrir_sem_edicao_nao_pede_nada_a_gpu(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert!(
                    tela.aguardando.is_none(),
                    "no neutro o resultado é a própria origem — pedir isso à GPU é latência à toa"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Trocar de foto não herda o arrasto que estava no painel.
    ///
    /// O que a segunda foto recebe é a revelação **dela** — aqui, nenhuma, então
    /// o neutro. Herdar o slider da foto anterior aplicaria a revelação de uma
    /// foto em outra, e a segunda abriria alterada sem ninguém tocar em nada — o
    /// tipo de coisa que se atribui ao motor de cor.
    #[gpui_kit::test]
    fn abrir_outra_foto_nao_herda_o_arrasto_da_anterior(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("a.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 2.0);

        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.ajustes().exposure, 2.0);

                tela.abrir(foto("b.jpg"), window, cx);
                assert_eq!(
                    tela.ajustes().exposure,
                    0.0,
                    "a foto b não tem nada gravado — abre no neutro dela"
                );
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.0,
                    "e o slider volta junto — senão a barra mente sobre o estado"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Um arrasto inteiro vira **uma** gravação, e só depois da pausa.
    ///
    /// Sem a espera, cada `Change` viraria um `UPDATE` de 54 colunas — dezenas
    /// por segundo enquanto o dedo se move. E gravar antes da pausa não é só
    /// desperdício: são dezenas de escritas concorrentes na mesma linha, cuja
    /// ordem de chegada ninguém controla.
    #[gpui_kit::test]
    fn o_arrasto_inteiro_vira_uma_gravacao_so(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        for valor in [0.5, 1.0, 1.5, 2.0] {
            arrastar(cx, &janela, 0, valor);
        }
        assert!(
            gravador.gravado().is_empty(),
            "gravar durante o arrasto seria um UPDATE por milímetro de slider"
        );

        passar_a_espera(cx);

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "quatro movimentos, uma gravação");
        assert_eq!(gravado[0].0, "id-retrato.jpg");
        assert_eq!(
            gravado[0].1.exposure, 2.0,
            "grava o valor onde o dedo parou"
        );
    }

    /// 🚨 Trocar de foto grava a anterior **antes** de trocar.
    ///
    /// Este é o teste que separa "grava" de "grava a coisa certa". Se `abrir`
    /// trocasse os ajustes primeiro, a espera pendente sairia depois com os
    /// valores da foto nova e o id da... também nova — e a revelação da primeira
    /// simplesmente sumiria, sem erro nenhum.
    #[gpui_kit::test]
    fn trocar_de_foto_grava_a_anterior_antes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("a.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.25);

        // Sem passar a espera: a troca tem de gravar sozinha.
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("b.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert_eq!(gravado[0].0, "id-a.jpg", "a gravação é da foto que saiu");
        assert_eq!(gravado[0].1.exposure, 1.25);

        // E a espera cancelada não pode ressuscitar e gravar de novo, agora com
        // os ajustes da foto b no id dela.
        passar_a_espera(cx);
        assert_eq!(gravador.gravado().len(), 1, "gravou duas vezes o mesmo");
    }

    /// 🚨 **Andar pela seta e voltar tem de trazer os ajustes de volta.**
    /// 🚨 **O "Zerar tudo" em lote escolhe o que apaga, e isso se prova.**
    ///
    /// Pedido do dono em 2026-09-11: o botão precisa funcionar com a tira
    /// inteira marcada. Três condições decidem quem entra, e nenhuma delas é
    /// visível na tela depois do clique — por isso ficam presas aqui.
    #[gpui_kit::test]
    fn o_zerar_em_lote_pega_as_marcadas_com_ajuste(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg", "id-c.jpg", "id-d.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let aberta = foto("a.jpg");
        // Mexida: entra.
        let mut mexida = foto("b.jpg");
        mexida.edit_exposure = Some(1.5);
        // No neutro: fica de fora — gravar nela seria mudar para o mesmo valor,
        // e o "Salvar na galeria" subiria um arquivo por nada.
        let ja_neutra = foto("c.jpg");
        // Mexida, mas não marcada: fica de fora.
        let mut de_fora = foto("d.jpg");
        de_fora.edit_exposure = Some(-2.0);

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![aberta, mexida, ja_neutra, de_fora], 0, window, cx);
                // Marca a mexida e a que já está no neutro — não a última.
                let ctrl = Modificadores {
                    aditivo: true,
                    faixa: false,
                };
                tela.clicar_na_tira(1, ctrl, window, cx);
                tela.clicar_na_tira(2, ctrl, window, cx);

                let quais: Vec<String> = tela.outras_a_zerar().into_iter().map(|f| f.id).collect();
                assert_eq!(
                    quais,
                    vec!["id-b.jpg".to_string()],
                    "só a marcada que tem o que zerar"
                );
            })
            .expect("a janela deve estar aberta");
    }

    #[gpui_kit::test]
    fn andar_e_voltar_preserva_o_que_foi_ajustado(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![foto("a.jpg"), foto("b.jpg")], 0, window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.25);
        janela
            .update(cx, |tela, window, cx| tela.andar(1, window, cx))
            .expect("a janela deve estar aberta");
        janela
            .update(cx, |tela, window, cx| tela.andar(-1, window, cx))
            .expect("a janela deve estar aberta");

        let exposicao = janela
            .update(cx, |tela, _window, _cx| tela.ajustes().exposure)
            .expect("a janela deve estar aberta");
        assert_eq!(
            exposicao, 1.25,
            "a exposicao voltou ao neutro na ida e volta"
        );
    }

    /// 🚨 Gravar ajuste **não pode apagar o corte** que a foto tinha.
    ///
    /// `SavePhotoEditsUseCase` recebe os oito campos de corte como `Option` e a
    /// entidade os atribui direto — passar `None` apaga. A Revelação nova ainda
    /// não sabe cortar, o que piora o risco: mexer num slider aqui apagaria,
    /// calado, o enquadramento feito no app de egui. O corte é lido da foto e
    /// devolvido igual.
    #[gpui_kit::test]
    fn gravar_devolve_o_corte_que_a_foto_ja_tinha(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-cortada.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_crop_x: Some(0.1),
                        edit_crop_y: Some(0.2),
                        edit_crop_width: Some(0.5),
                        edit_crop_height: Some(0.5),
                        edit_crop_rotation: Some(90),
                        edit_crop_flip_h: Some(true),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 0.75);
        passar_a_espera(cx);

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        let corte = gravado[0].2;
        assert_eq!(corte.x, Some(0.1));
        assert_eq!(corte.largura, Some(0.5));
        assert_eq!(corte.rotacao, Some(90));
        assert_eq!(corte.espelho_h, Some(true));
    }

    /// Abrir e não mexer em nada **não** grava.
    ///
    /// 🔑 Se abrir gravasse, o app novo reescreveria os 46 campos de toda foto
    /// que alguém apenas olhasse — inclusive os 18 que ele mostra mas não aplica.
    /// Uma passada pela biblioteca viraria uma edição em massa que ninguém pediu.
    #[gpui_kit::test]
    fn abrir_e_nao_mexer_em_nada_nao_grava(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.5),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );
            })
            .expect("a janela deve estar aberta");

        passar_a_espera(cx);
        assert!(gravador.gravado().is_empty());
    }

    /// 🚨 `Cmd+Z` devolve os sliders, a foto e o banco ao passo anterior.
    ///
    /// Os três juntos, e não só o número: um desfazer que mexesse em `ajustes` e
    /// deixasse a barra onde estava daria um painel mentindo sobre a foto, e um
    /// que não gravasse deixaria o banco com o estado desfeito — que volta na
    /// próxima abertura, como se o `Cmd+Z` não tivesse acontecido.
    #[gpui_kit::test]
    fn desfazer_volta_o_slider_a_foto_e_o_banco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);
        passar_a_espera(cx);

        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.pode_desfazer(), "há um gesto para desfazer");

                tela.desfazer(window, cx);

                assert_eq!(tela.ajustes().exposure, 0.0);
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.0,
                    "o slider volta junto"
                );
                assert!(tela.pode_refazer());
                assert!(!tela.pode_desfazer(), "voltou ao estado da abertura");
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 2, "o arrasto e o desfazer");
        assert_eq!(
            gravado[1].1.exposure, 0.0,
            "desfazer tem de chegar ao banco — senão volta na próxima abertura"
        );
    }

    /// 🚨 Um arrasto inteiro é **um** `Cmd+Z`.
    ///
    /// É a diferença de propósito em relação ao legado, que empurra um snapshot
    /// por quadro em que algo mudou: lá, um arrasto de meio segundo vira ~30
    /// passos e o `Cmd+Z` desfaz um milímetro por vez — com o teto de 20, o resto
    /// do histórico já foi embora. E o número de passos de lá depende da taxa de
    /// quadros do monitor.
    #[gpui_kit::test]
    fn um_arrasto_inteiro_e_um_passo_so_de_desfazer(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        for valor in [0.2, 0.4, 0.6, 0.8, 1.0] {
            arrastar(cx, &janela, 0, valor);
        }
        passar_a_espera(cx);

        janela
            .update(cx, |tela, window, cx| {
                tela.desfazer(window, cx);
                assert_eq!(
                    tela.ajustes().exposure,
                    0.0,
                    "um Cmd+Z desfaz o arrasto inteiro, e não o último milímetro"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Cmd+Z` no meio da espera desfaz o gesto de agora, e não o anterior.
    ///
    /// Sem fechar o gesto em curso antes de andar no histórico, o `Cmd+Z`
    /// desfaria o passo **anterior** e deixaria o arrasto de agora pendente — que
    /// gravaria 500 ms depois, por cima do que acabou de ser desfeito. O sintoma é
    /// o pior: o `Cmd+Z` parece funcionar e depois se desfaz sozinho.
    #[gpui_kit::test]
    fn desfazer_no_meio_da_espera_fecha_o_gesto_primeiro(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.0);
        passar_a_espera(cx);
        // Segundo gesto, e o Cmd+Z vem **antes** de a espera terminar.
        arrastar(cx, &janela, 0, 2.0);

        janela
            .update(cx, |tela, window, cx| {
                tela.desfazer(window, cx);
                assert_eq!(
                    tela.ajustes().exposure,
                    1.0,
                    "desfez o gesto de agora (2.0), voltando ao anterior"
                );
            })
            .expect("a janela deve estar aberta");

        // E a espera que ficou para trás não pode ressuscitar o 2.0.
        passar_a_espera(cx);
        let gravado = gravador.gravado();
        assert_eq!(
            gravado.last().map(|g| g.1.exposure),
            Some(1.0),
            "a última gravação é a do desfazer"
        );
    }

    /// 🚨 O histórico é por foto.
    ///
    /// Um `Cmd+Z` que atravessasse fotos aplicaria a revelação de uma na outra —
    /// o mesmo defeito que a cópia da seleção já impede na outra ponta.
    /// Renomear troca o nome na tela **e** no banco, na mesma linha.
    ///
    /// 🚨 O erro que este teste pega é renomear só no banco: `self.presets` é o
    /// que a coluna desenha, e o nome antigo continuaria ali até a próxima
    /// abertura — com o operador renomeando de novo, achando que não pegou.
    #[gpui_kit::test]
    fn renomear_troca_o_nome_na_lista_e_no_banco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let minha = Preset::user(
            "Retrato".into(),
            PresetAdjustments::vazia().com("clarity", 0.2),
        );
        let id = minha.id;
        let do_sistema = Preset::system("Hora dourada", PresetAdjustments::vazia());
        let sistema_id = do_sistema.id;

        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![do_sistema, minha],
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.renomear_preset(id, "  Retrato suave  ".into(), cx);
                tela.renomear_preset(sistema_id, "Outro nome".into(), cx);
                tela.renomear_preset(id, "   ".into(), cx);

                assert_eq!(
                    tela.presets
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>(),
                    ["Hora dourada", "Retrato suave"],
                    "a do sistema não se renomeia, e nome em branco não vale"
                );
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            guarda.renomeados(),
            vec![(id, "Retrato suave".to_string())],
            "vai ao banco uma vez só, e sem os espaços"
        );
    }

    /// Apagar tira da lista, avisa o banco e desfaz a prévia.
    ///
    /// ⚠️ **A prévia é o detalhe que escapa**: se o ponteiro estava sobre a
    /// predefinição apagada, a foto continuaria mostrando uma que não existe
    /// mais — e sair com o ponteiro não a desfaria, porque a linha sumiu antes
    /// de o `on_hover` de saída chegar.
    #[gpui_kit::test]
    fn apagar_tira_da_lista_e_desfaz_a_previa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let guarda = Arc::new(GuardaDeMentira::default());
        let minha = Preset::user(
            "Meu visual".into(),
            PresetAdjustments::vazia().com("saturation", -1.0),
        );
        let id = minha.id;
        let do_sistema = Preset::system("Hora dourada", PresetAdjustments::vazia());
        let sistema_id = do_sistema.id;

        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![do_sistema, minha.clone()],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.prever(Some(&minha), cx);
                assert_eq!(tela.ajustes_na_tela().saturation, -1.0);

                tela.apagar_preset(sistema_id, cx);
                assert_eq!(tela.presets.len(), 2, "a do sistema não se apaga");

                tela.apagar_preset(id, cx);
                assert_eq!(tela.presets.len(), 1);
                assert!(tela.previa().is_none());
                assert_eq!(tela.ajustes_na_tela().saturation, 0.0);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(guarda.apagados(), vec![id]);
    }

    /// 🔄 **"Atualizar com os ajustes atuais" grava a foto na mesma linha** —
    /// o mesmo id na lista e no banco, só o que saiu do neutro, e a marca de
    /// "na foto" acende. Nas do sistema não mexe: elas não têm linha no banco.
    #[gpui_kit::test]
    fn atualizar_preset_grava_os_ajustes_atuais_no_mesmo_id(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let guarda = Arc::new(GuardaDeMentira::default());
        let minha = Preset::user(
            "Meu visual".into(),
            PresetAdjustments::vazia().com("temperature", 5.0),
        );
        let id = minha.id;
        let sepia = use_cases::presets::presets_de_sistema()
            .into_iter()
            .find(|p| p.name == "Sépia à moda antiga")
            .expect("a sépia do sistema");
        let sepia_id = sepia.id;

        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![sepia.clone(), minha.clone()],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.aplicar_preset(&minha, window, cx);
                tela.ajustes.exposure += 0.5;

                tela.atualizar_preset(id, cx);
                let atual = tela.presets.iter().find(|p| p.id == id).cloned();
                let atual = atual.expect("a minha continua na lista");
                assert_eq!(atual.name, "Meu visual", "o nome fica");
                assert_eq!(atual.adjustments.get("temperature"), Some(5.0));
                assert_eq!(atual.adjustments.get("exposure"), Some(0.5));
                assert_eq!(
                    atual.adjustments.get("saturation"),
                    None,
                    "o neutro não vai"
                );
                assert!(tela.em_uso(&atual), "a foto agora é ela");

                tela.atualizar_preset(sepia_id, cx);
                let intacta = tela.presets.iter().find(|p| p.id == sepia_id);
                assert_eq!(
                    intacta.map(|p| &p.adjustments),
                    Some(&sepia.adjustments),
                    "a do sistema não muda"
                );
            })
            .expect("a janela deve estar aberta");

        let salvos = guarda.salvos();
        assert_eq!(salvos.len(), 1, "só a do operador vai ao banco");
        assert_eq!(salvos[0].id, id, "a mesma linha, e não uma cópia");
        assert_eq!(salvos[0].adjustments.get("exposure"), Some(0.5));
    }

    /// 🔄 A que nasceu inteira (os 53, "Zerar os outros ajustes ao aplicar")
    /// continua inteira; a que soma, com a foto no neutro, não grava vazia.
    #[gpui_kit::test]
    fn atualizar_preset_mantem_o_modo_e_nao_grava_vazia(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let guarda = Arc::new(GuardaDeMentira::default());
        let inteira = Preset::user(
            "Visual completo".into(),
            super::super::presets::dos_ajustes(&Ajustes::default(), true),
        );
        let total = inteira.adjustments.len();
        let que_soma = Preset::user(
            "Só a cor".into(),
            PresetAdjustments::vazia().com("saturation", -1.0),
        );
        let (id_inteira, id_soma) = (inteira.id, que_soma.id);

        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![inteira, que_soma],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.atualizar_preset(id_soma, cx);
                tela.ajustes.exposure = 0.7;
                tela.atualizar_preset(id_inteira, cx);
                let atual = tela.presets.iter().find(|p| p.id == id_inteira);
                assert_eq!(
                    atual.map(|p| p.adjustments.len()),
                    Some(total),
                    "continua com os 53"
                );
            })
            .expect("a janela deve estar aberta");

        let salvos = guarda.salvos();
        assert_eq!(salvos.len(), 1, "a que soma, no neutro, não grava");
        assert_eq!(salvos[0].id, id_inteira);
    }

    /// 📄 "Duplicar": a cópia vai para "Minhas" com nome novo, e a do sistema
    /// que substitui dá uma cópia que faz a mesma foto.
    #[gpui_kit::test]
    fn duplicar_preset_poe_a_copia_em_minhas_e_mantem_o_resultado(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let sistema = use_cases::presets::presets_de_sistema();
        let soma = sistema
            .iter()
            .find(|p| !p.replaces)
            .cloned()
            .expect("uma do sistema que soma");
        let substitui = sistema
            .iter()
            .find(|p| p.replaces)
            .cloned()
            .expect("uma do sistema que substitui");

        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            sistema.clone(),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.duplicar_preset(soma.id, cx);
                tela.duplicar_preset(soma.id, cx);
                tela.duplicar_preset(substitui.id, cx);
            })
            .expect("a janela deve estar aberta");

        let salvos = guarda.salvos();
        assert_eq!(salvos.len(), 3);
        assert!(
            salvos.iter().all(|p| !p.is_system),
            "as cópias são do operador"
        );
        assert_eq!(salvos[0].name, format!("{} (cópia)", soma.name));
        assert_eq!(salvos[1].name, format!("{} (cópia 2)", soma.name));
        assert_eq!(salvos[0].adjustments, soma.adjustments);
        assert_ne!(salvos[0].id, soma.id, "outra linha");

        // Por cima de uma foto já mexida, a cópia dá o mesmo que a original.
        let mexida = Ajustes {
            exposure: 0.8,
            temperature: 12.0,
            ..Ajustes::default()
        };
        assert_eq!(
            super::super::presets::aplicado(&mexida, &salvos[2]),
            super::super::presets::aplicado(&mexida, &substitui),
            "a cópia de \"{}\" também recomeça do neutro",
            substitui.name
        );

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.presets.len(), sistema.len() + 3, "entram na lista");
                assert!(
                    !tela.predefinicoes.ordem.recolhido(Grupo::Minhas),
                    "Minhas fica aberta"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// A importação do Lightroom, de ponta a ponta: escolher, traduzir, salvar.
    ///
    /// 🔑 **As novas entram na lista da tela na hora.** Elas já vão ao banco
    /// pela porta, mas quem acabou de importar quer aplicá-las agora — esperar a
    /// próxima abertura do app é o mesmo que não ter importado.
    #[gpui_kit::test]
    fn importar_do_lightroom_traduz_grava_e_entra_na_lista(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::com(vec![
            Arquivo {
                nome: "Sepia.xmp".into(),
                texto: Some(
                    r#"<x crs:PresetName="Sépia do Estúdio" crs:Saturation="-100"
                          crs:SplitToningShadowHue="35" crs:SplitToningShadowSaturation="40"
                          crs:ToneCurvePV2012="0, 22"/>"#
                        .into(),
                ),
            },
            // Ilegível: nem `.lrtemplate` nem `.xmp` reconhecem.
            Arquivo {
                nome: "Foto.jpg".into(),
                texto: Some("isto não é preset".into()),
            },
            // Não deu para ler do disco — outra coisa, e o relatório separa.
            Arquivo {
                nome: "Travado.xmp".into(),
                texto: None,
            },
        ]));

        let janela = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            escolha,
            Vec::new(),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.importar_do_lightroom(cx);
            })
            .expect("a janela deve estar aberta");

        // O seletor responde por canal, e a tela colhe num laço acordado.
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    tela.presets
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>(),
                    ["Sépia do Estúdio"]
                );
                let ajustes = &tela.presets[0].adjustments;
                assert_eq!(ajustes.get("saturation"), Some(-1.0), "-100 vira -1");
                assert_eq!(ajustes.get("split_shadow_hue"), Some(35.0));

                let relatorio = tela.relatorio.as_ref().expect("houve importação");
                assert_eq!(relatorio.arquivos, 3);
                assert_eq!(relatorio.criadas, 1);
                assert_eq!(relatorio.ilegiveis.len(), 2, "o ilegível e o que não abriu");
                // A curva em atributo é um texto só, e não a lista de pontos:
                // fica de fora calada, como no site (`lerXmp`), e não é
                // "recurso que o motor não tem" — o motor tem.
                assert!(relatorio.ignorados.is_empty());
            })
            .expect("a janela deve estar aberta");

        let salvos = guarda.salvos();
        assert_eq!(salvos.len(), 1);
        assert_eq!(salvos[0].name, "Sépia do Estúdio");
    }

    /// 📦 **Exportar uma, um grupo e todas — e importar de volta** (dono,
    /// 3/out/2026: *"exportar os presets individualmente, grupos de presets ou
    /// todos… similar ao Lightroom"*). Uma vai num `.rfpreset`; grupo e todas,
    /// num `.zip`. O pacote de todas, importado num balcão sem as do
    /// operador, recria só elas: as do sistema já estão lá com o mesmo nome.
    #[gpui_kit::test]
    fn exportar_uma_um_grupo_e_todas_e_importar_de_volta(cx: &mut TestAppContext) {
        use super::predefinicoes::Exportacao;
        use crate::revelacao::presets::arquivo::{abrir_pacote, ArquivoDePredefinicao};

        let (previews, _dir) = previews_descartaveis();
        let escolha = Arc::new(EscolhaDeMentira::default());
        let minha = Preset::user(
            "Meu visual".into(),
            PresetAdjustments::vazia().com("exposure", 0.4),
        );
        let outra = Preset::user(
            "Céu / mar".into(),
            PresetAdjustments::vazia().com("temperature", -2.0),
        );
        let do_sistema = use_cases::presets::presets_de_sistema();
        let mut todas = do_sistema.clone();
        todas.extend([minha.clone(), outra.clone()]);
        let total = todas.len();
        let lrs = do_sistema.iter().filter(|p| p.grupo.is_some()).count();

        let janela = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            Arc::new(GuardaDeMentira::default()),
            escolha.clone(),
            todas,
        );

        for alvo in [
            Exportacao::Uma(minha.id),
            Exportacao::Grupo(Grupo::Lrs),
            Exportacao::Todas,
        ] {
            janela
                .update(cx, |tela, _window, cx| {
                    tela.exportar_predefinicoes(alvo, cx)
                })
                .expect("a janela deve estar aberta");
            cx.executor().advance_clock(Duration::from_millis(200));
            cx.run_until_parked();
        }

        let gravados = escolha.gravados.lock().expect("os gravados").clone();
        let nomes: Vec<&str> = gravados.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            nomes,
            ["Meu visual.rfpreset", "LRs.zip", "Predefinições.zip"]
        );

        let uma = ArquivoDePredefinicao::ler(std::str::from_utf8(&gravados[0].1).expect("texto"))
            .expect("é um .rfpreset");
        assert_eq!(uma.nome, "Meu visual");
        assert_eq!(uma.grupo.as_deref(), Some("Minhas"));
        assert_eq!(uma.ajustes.get("exposure"), Some(&0.4));

        let do_grupo = abrir_pacote(&gravados[1].1).expect("zip");
        assert_eq!(do_grupo.len(), lrs, "só as da pasta LRs");
        let de_todas = abrir_pacote(&gravados[2].1).expect("zip");
        assert_eq!(de_todas.len(), total, "todas, cada uma uma vez");

        janela
            .update(cx, |tela, _window, _cx| {
                let avisos: Vec<&str> = tela
                    .predefinicoes
                    .avisos
                    .iter()
                    .map(|a| a.texto.as_str())
                    .collect();
                assert_eq!(
                    avisos,
                    [
                        "\"Meu visual\" exportada em Meu visual.rfpreset.".to_string(),
                        format!("{lrs} predefinições de \"LRs\" exportadas em LRs.zip."),
                        format!("{total} predefinições exportadas em Predefinições.zip."),
                    ]
                );
                assert!(tela.predefinicoes.exportando.is_none());
            })
            .expect("a janela deve estar aberta");

        // A volta: outro balcão, só com as do sistema.
        let (previews, _dir2) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let outro_balcao = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            Arc::new(EscolhaDeMentira::com(de_todas)),
            do_sistema.clone(),
        );
        outro_balcao
            .update(cx, |tela, _window, cx| tela.importar_do_lightroom(cx))
            .expect("a janela deve estar aberta");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        outro_balcao
            .update(cx, |tela, _window, _cx| {
                let relatorio = tela.relatorio.as_ref().expect("houve importação");
                assert_eq!(relatorio.criadas, 2, "{relatorio:?}");
                assert_eq!(relatorio.repetidas, do_sistema.len());
                assert!(relatorio.ilegiveis.is_empty() && relatorio.ignorados.is_empty());
            })
            .expect("a janela deve estar aberta");
        let mut salvos: Vec<(String, PresetAdjustments)> = guarda
            .salvos()
            .into_iter()
            .map(|p| (p.name, p.adjustments))
            .collect();
        salvos.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            salvos,
            [
                (outra.name, outra.adjustments),
                (minha.name, minha.adjustments)
            ]
        );
    }

    /// Fechar a janela de gravar não é resultado: nem aviso, nem botão preso.
    #[gpui_kit::test]
    fn desistir_da_exportacao_nao_avisa_nem_trava(cx: &mut TestAppContext) {
        use super::predefinicoes::Exportacao;

        let (previews, _dir) = previews_descartaveis();
        let escolha = Arc::new(EscolhaDeMentira::default());
        escolha
            .desistir
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let janela = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            Arc::new(GuardaDeMentira::default()),
            escolha,
            use_cases::presets::presets_de_sistema(),
        );
        janela
            .update(cx, |tela, _window, cx| {
                tela.exportar_predefinicoes(Exportacao::Grupo(Grupo::Sistema), cx);
                assert!(
                    tela.predefinicoes.exportando.is_some(),
                    "esperando a janela"
                );
                // Grupo vazio: avisa e não abre janela nenhuma.
                tela.predefinicoes.exportando = None;
                tela.exportar_predefinicoes(Exportacao::Grupo(Grupo::Minhas), cx);
                assert!(tela.predefinicoes.exportando.is_none());
                assert_eq!(tela.predefinicoes.avisos.len(), 1);
                tela.predefinicoes.avisos.clear();
            })
            .expect("a janela deve estar aberta");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.predefinicoes.exportando.is_none());
                assert!(tela.predefinicoes.avisos.is_empty());
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **Nome repetido é pulado, e não sobrescrito.**
    ///
    /// Reimportar a mesma pasta é gesto comum — e sobrescrever apagaria o ajuste
    /// que o fotógrafo fez em cima da predefinição depois de importá-la.
    #[gpui_kit::test]
    fn reimportar_a_mesma_pasta_nao_sobrescreve(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::com(vec![Arquivo {
            nome: "Claro.xmp".into(),
            texto: Some(r#"<x crs:PresetName="Claro" crs:Exposure2012="0.5"/>"#.into()),
        }]));

        let janela = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            escolha,
            // A mesma predefinição já está lá, com outro valor.
            vec![Preset::user(
                "Claro".into(),
                PresetAdjustments::vazia().com("exposure", 2.0),
            )],
        );

        janela
            .update(cx, |tela, _window, cx| tela.importar_do_lightroom(cx))
            .expect("a janela deve estar aberta");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.presets.len(), 1);
                assert_eq!(
                    tela.presets[0].adjustments.get("exposure"),
                    Some(2.0),
                    "a que estava lá continua como estava"
                );
                assert_eq!(tela.relatorio.as_ref().expect("houve").repetidas, 1);
            })
            .expect("a janela deve estar aberta");

        assert!(guarda.salvos().is_empty());
    }

    /// Desistir do seletor não deixa relatório: ninguém precisa ler "0 arquivos
    /// lidos" por ter fechado uma janela.
    #[gpui_kit::test]
    fn desistir_do_seletor_nao_deixa_relatorio(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = com_escolha(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            Arc::new(GuardaDeMentira::default()),
            Arc::new(EscolhaDeMentira::default()),
            Vec::new(),
        );

        janela
            .update(cx, |tela, _window, cx| tela.importar_do_lightroom(cx))
            .expect("a janela deve estar aberta");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.relatorio.is_none());
                assert!(
                    !tela.escolhendo_arquivos,
                    "e o botão volta a ligar — senão ele fica morto até fechar o app"
                );
            })
            .expect("a janela deve estar aberta");
    }

    #[gpui_kit::test]
    fn trocar_de_foto_comeca_um_historico_novo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("a.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);
        passar_a_espera(cx);

        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.pode_desfazer());

                tela.abrir(foto("b.jpg"), window, cx);
                assert!(
                    !tela.pode_desfazer(),
                    "o arrasto na foto a não pode ser desfeito estando na b"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 Desfazer e refazer **não** empilham passos novos.
    ///
    /// Andar no histórico não é escrever nele. Se o desfazer registrasse, o
    /// `Cmd+Shift+Z` nunca alcançaria nada — sempre haveria um passo novo igual ao
    /// que acabou de sair.
    /// 🔑 A prévia muda **a foto**, e não os ajustes.
    ///
    /// É o gesto do Lightroom, e o do site: o ponteiro sobre uma predefinição
    /// mostra como ficaria, e sair devolve o que estava. Se ela escrevesse em
    /// `self.ajustes`, passar o ponteiro pela lista deixaria a foto alterada —
    /// e o `Cmd+Z` não teria o que desfazer, porque nenhum gesto foi
    /// registrado.
    #[gpui_kit::test]
    fn a_previa_muda_a_foto_e_nao_os_ajustes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let preset = Preset::system(
            "Sépia à moda antiga",
            PresetAdjustments::vazia()
                .com("saturation", -1.0)
                .com("split_shadow_hue", 35.0),
        );
        let janela = com_presets(cx, previews, gravador.clone(), vec![preset.clone()]);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.0),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );

                tela.prever(Some(&preset), cx);

                assert_eq!(tela.ajustes().saturation, 0.0, "os ajustes não mudam");
                assert_eq!(tela.ajustes_na_tela().saturation, -1.0, "a foto muda");
                assert_eq!(
                    tela.ajustes_na_tela().exposure,
                    1.0,
                    "e o que a predefinição não menciona continua na prévia"
                );
                assert!(!tela.pode_desfazer(), "prévia não é gesto");

                tela.prever(None, cx);
                assert_eq!(tela.ajustes_na_tela().saturation, 0.0, "sair devolve");
            })
            .expect("a janela deve estar aberta");

        assert!(gravador.gravado().is_empty(), "e nada disso chega ao banco");
    }

    #[gpui_kit::test]
    fn refazer_alcanca_o_que_o_desfazer_deixou(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);
        passar_a_espera(cx);

        janela
            .update(cx, |tela, window, cx| {
                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0);

                tela.refazer(window, cx);
                assert_eq!(tela.ajustes().exposure, 1.5);
                assert_eq!(tela.controles[0].estado.read(cx).value().start(), 1.5);
                assert!(!tela.pode_refazer(), "chegou ao fim do histórico");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **Cmd+A, Cmd+D, Ctrl e Shift no clique montam o lote na tira** — as
    /// regras do site (`escolherNaTira`): nenhum deles troca a foto aberta, a
    /// aberta nunca sai do lote, e trocar de foto recomeça.
    #[gpui_kit::test]
    fn a_marcacao_da_tira_segue_o_site(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![foto("a.jpg"), foto("b.jpg"), foto("c.jpg"), foto("d.jpg")],
                    1,
                    window,
                    cx,
                );
                assert_eq!(
                    *tela.marcadas(),
                    BTreeSet::from([1]),
                    "abre só com a aberta"
                );
                assert_eq!(tela.alvos_da_sincronizacao().len(), 1);

                tela.marcar_todas(cx);
                assert_eq!(tela.alvos_da_sincronizacao().len(), 4, "Cmd+A");
                assert_eq!(tela.posicao(), 1, "e a aberta não mudou");

                tela.desmarcar(cx);
                assert_eq!(
                    *tela.marcadas(),
                    BTreeSet::from([1]),
                    "Cmd+D deixa só a aberta"
                );

                let ctrl = Modificadores {
                    aditivo: true,
                    faixa: false,
                };
                tela.clicar_na_tira(3, ctrl, window, cx);
                assert_eq!(tela.posicao(), 1, "Ctrl não troca a aberta");
                assert_eq!(*tela.marcadas(), BTreeSet::from([1, 3]));

                tela.clicar_na_tira(1, ctrl, window, cx);
                assert!(tela.marcadas().contains(&1), "Ctrl na aberta não a tira");

                let shift = Modificadores {
                    aditivo: false,
                    faixa: true,
                };
                tela.clicar_na_tira(3, shift, window, cx);
                assert_eq!(
                    *tela.marcadas(),
                    BTreeSet::from([1, 2, 3]),
                    "Shift substitui pela faixa"
                );

                tela.clicar_na_tira(0, Modificadores::default(), window, cx);
                assert_eq!(tela.posicao(), 0, "o clique simples troca");
                assert_eq!(*tela.marcadas(), BTreeSet::from([0]), "e recomeça o lote");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **A tira segue o site**: a seta anda sobre o recorte, o lote fica ao
    /// trocar dentro dele, e o `Cmd+A` marca só o que a tira mostra.
    #[gpui_kit::test]
    fn a_tira_recorta_e_guarda_o_lote(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);
        let com_nota = |nome: &str, nota: i32| PhotoViewModel {
            rating: nota,
            ..foto(nome)
        };

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(
                    vec![
                        com_nota("a.jpg", 3),
                        com_nota("b.jpg", 0),
                        com_nota("c.jpg", 4),
                        com_nota("d.jpg", 0),
                    ],
                    0,
                    window,
                    cx,
                );
                tela.recortar(biblioteca_core::acervo::Filtro::Classificadas, cx);
                assert_eq!(tela.na_tira(), vec![0, 2]);
                assert_eq!(tela.posicao_na_tira(), (0, 2));

                tela.marcar_todas(cx);
                assert_eq!(*tela.marcadas(), BTreeSet::from([0, 2]), "só o recorte");

                tela.andar(1, window, cx);
                assert_eq!(tela.posicao(), 2, "a seta pula a sem nota");
                assert_eq!(
                    *tela.marcadas(),
                    BTreeSet::from([0, 2]),
                    "andar dentro do lote o mantém"
                );

                tela.recortar(biblioteca_core::acervo::Filtro::Todas, cx);
                tela.clicar_na_tira(3, Modificadores::default(), window, cx);
                assert_eq!(
                    *tela.marcadas(),
                    BTreeSet::from([3]),
                    "fora do lote recomeça"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// A raiz gravou nas marcadas; as cópias da tira têm de dizer o mesmo.
    #[gpui_kit::test]
    fn as_sincronizadas_atualizam_as_copias_da_tira(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo(vec![foto("a.jpg"), foto("b.jpg")], 0, window, cx);
                let ajustes = Ajustes {
                    exposure: 1.25,
                    ..Ajustes::default()
                };
                let corte = Corte {
                    x: Some(0.2),
                    ..Corte::default()
                };
                tela.aplicar_sincronizadas(&[("id-b.jpg".to_string(), ajustes, corte)], cx);

                let b = &tela.acervo()[1];
                assert_eq!(b.edit_exposure, Some(1.25));
                assert_eq!(b.edit_crop_x, Some(0.2));
                assert!(persistencia::ja_revelada(b), "e o ponto âmbar acende");
                assert_eq!(tela.acervo()[0].edit_exposure, None, "a outra não mexe");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Clicar num preset move os sliders, a foto, o histórico e o banco.
    ///
    /// Os quatro juntos: um preset que mudasse `ajustes` sem mover as barras
    /// deixaria o painel mentindo; sem passo de histórico, o `Cmd+Z` pularia por
    /// cima dele; sem gravação, ele sumiria na próxima abertura.
    #[gpui_kit::test]
    fn aplicar_preset_move_os_sliders_o_historico_e_o_banco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let preset = Preset::system(
            "Hora dourada",
            PresetAdjustments::vazia().com("temperature", 5.0),
        );
        let janela = com_presets(cx, previews, gravador.clone(), vec![preset.clone()]);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.aplicar_preset(&preset, window, cx);

                assert_eq!(tela.ajustes().temperature, 5.0);
                // A barra mostra a escala do Lightroom: +5 no motor é +50.
                assert_eq!(
                    tela.controles[2].estado.read(cx).value().start(),
                    50.0,
                    "o terceiro controle é a temperatura — a barra tem de acompanhar"
                );
                assert!(tela.pode_desfazer(), "o preset é um passo de histórico");

                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes().temperature, 0.0);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 2, "o preset e o desfazer");
        assert_eq!(gravado[0].1.temperature, 5.0);
    }

    /// 🚨 O "Auto" tem de mover a foto — foi por não mover que ele saiu da lista
    /// de presets, onde pedia `exposure: Some(0.0)` sobre o neutro `0.0`.
    ///
    /// Os quatro juntos, como no preset: os ajustes, a barra, o histórico e o
    /// banco. Um automático que mudasse `ajustes` sem mover a barra deixaria o
    /// painel mentindo sobre a foto que ele mesmo acabou de mudar.
    #[gpui_kit::test]
    fn o_tom_automatico_move_os_sliders_o_historico_e_o_banco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_uniforme(30))
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.tom_automatico(window, cx);

                // A mediana é 30, e o alvo é 118: log2(118/30) = 1,98.
                assert_eq!(tela.ajustes().exposure, 1.98);
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    1.98,
                    "o primeiro controle é a exposição — a barra tem de acompanhar"
                );
                assert!(
                    tela.pode_desfazer(),
                    "o automático é um passo de histórico, como o preset"
                );

                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(gravador.gravado().len(), 2, "o automático e o desfazer");
    }

    /// Sem foto não há histograma — e um passo de histórico sobre nada seria um
    /// `Cmd+Z` que não desfaz coisa nenhuma.
    #[gpui_kit::test]
    fn o_tom_automatico_sem_foto_nao_faz_nada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.tom_automatico(window, cx);

                assert_eq!(tela.ajustes().exposure, 0.0);
                assert!(!tela.pode_desfazer());
            })
            .expect("a janela deve estar aberta");

        assert!(gravador.gravado().is_empty(), "nada para gravar");
    }

    /// 🚨 O preset não zera o que ele não menciona.
    ///
    /// ⚠️ Um preset escreve **o que ele traz**, e o resto fica.
    ///
    /// Um aplicado sobre uma foto com HSL trabalhado não pode apagar o HSL. Era
    /// verdade por acidente — `PresetAdjustments` não tinha campo de HSL para
    /// escrever —, e agora é verdade por decisão: campo ausente do mapa não é
    /// tocado. Quem quiser apagar guarda os 53, e é escolha de quem salva.
    #[gpui_kit::test]
    fn o_preset_nao_apaga_o_que_ele_nao_menciona(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let preset = Preset::system("Frio", PresetAdjustments::vazia().com("temperature", -5.0));
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            vec![preset.clone()],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_hsl_blue_lum: Some(-30.0),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );
                tela.aplicar_preset(&preset, window, cx);

                assert_eq!(tela.ajustes().temperature, -5.0);
                assert_eq!(
                    tela.ajustes().hsl_blue_lum,
                    -30.0,
                    "o HSL da foto não está no preset — e não pode ser apagado por ele"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A prévia de uma predefinição que substitui parte do neutro** — e é
    /// a mesma foto que o clique dá. Somando sempre, "Preto e branco" sobre uma
    /// sépia mostrava âmbar no ponteiro e cinza depois do clique.
    #[gpui_kit::test]
    fn a_previa_que_substitui_e_a_mesma_foto_do_clique(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let pb = Preset::system_replacing(
            "Preto e branco clássico",
            PresetAdjustments::vazia().com("saturation", -1.0),
        );
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            vec![pb.clone()],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.0),
                        ..foto("retrato.jpg")
                    },
                    window,
                    cx,
                );
                tela.prever(Some(&pb), cx);
                let na_previa = tela.ajustes_na_tela();
                assert_eq!(na_previa.exposure, 0.0, "recomeça do neutro");
                assert_eq!(na_previa.saturation, -1.0);

                tela.prever(None, cx);
                tela.aplicar_preset(&pb, window, cx);
                assert_eq!(
                    tela.ajustes_na_tela(),
                    na_previa,
                    "o clique dá a mesma foto"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// O formulário embutido: o `+` abre e fecha, e salvar fecha.
    ///
    /// 🚨 **Sem nada fora do neutro não salva** — a menos que a caixa "zerar os
    /// outros" esteja marcada. O diálogo antigo tinha o OK sempre ligado e
    /// gravava uma predefinição vazia.
    #[gpui_kit::test]
    fn o_formulario_so_salva_o_que_tem_o_que_guardar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            Vec::new(),
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.alternar_formulario_de_preset(window, cx);
                assert!(tela.predefinicoes.criando.esta_aberto());
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.trocar_valor("Nada", window, cx));

                tela.salvar_preset(window, cx);
                assert!(tela.presets.is_empty(), "nada fora do neutro");
                assert!(
                    tela.predefinicoes.criando.esta_aberto(),
                    "o formulário continua aberto"
                );

                // Com a caixa marcada, guarda todos — e aplicar devolve ao original.
                tela.alternar_preset_inteiro(cx);
                tela.salvar_preset(window, cx);
                assert_eq!(tela.presets.len(), 1);
                assert_eq!(
                    tela.presets[0].adjustments.len(),
                    crate::revelacao::processador::Ajustes::NOMES.len()
                );
                assert!(!tela.predefinicoes.criando.esta_aberto(), "salvar fecha");
                assert!(!tela.preset_inteiro, "a caixa volta desmarcada");
                assert_eq!(tela.nome_do_preset.read(cx).value().as_ref(), "");

                tela.alternar_formulario_de_preset(window, cx);
                tela.alternar_formulario_de_preset(window, cx);
                assert!(
                    !tela.predefinicoes.criando.esta_aberto(),
                    "o + fecha o que abriu"
                );
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.salvos().len(), 1);
    }

    /// 🚨 **Apagar pergunta antes** — "Apagar "X"? A predefinição sai da
    /// lista." —, e só "Apagar" apaga. É o único gesto da coluna que não se
    /// desfaz.
    #[gpui_kit::test]
    fn apagar_pergunta_e_so_o_sim_apaga(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let minha = Preset::user(
            "Retrato".into(),
            PresetAdjustments::vazia().com("exposure", 1.0),
        );
        let id = minha.id;
        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![minha],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.pedir_para_apagar(id, window, cx);
                assert_eq!(
                    tela.predefinicoes.pergunta.aberto(),
                    Some(&(id, "Retrato".to_string()))
                );
                tela.responder_pergunta(false, window, cx);
                assert!(!tela.predefinicoes.pergunta.esta_aberto());
                assert_eq!(tela.presets.len(), 1, "cancelar não apaga");

                tela.pedir_para_apagar(id, window, cx);
                tela.responder_pergunta(true, window, cx);
                assert!(tela.presets.is_empty());
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.apagados(), vec![id]);
    }

    /// O aviso de "entrou nas predefinições" aparece no canto e some sozinho.
    #[gpui_kit::test]
    fn o_aviso_de_salvar_some_sozinho(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.alternar_preset_inteiro(cx);
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.trocar_valor("Neutro", window, cx));
                tela.salvar_preset(window, cx);
                assert_eq!(
                    tela.predefinicoes
                        .avisos
                        .iter()
                        .map(|a| a.texto.as_str())
                        .collect::<Vec<_>>(),
                    ["\"Neutro\" entrou nas predefinições."]
                );
            })
            .expect("a janela deve estar aberta");

        cx.executor().advance_clock(Duration::from_secs(5));
        cx.run_until_parked();
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.predefinicoes.avisos.is_empty());
            })
            .expect("a janela deve estar aberta");
    }

    /// Renomear no lugar: a linha vira campo com o nome de agora, e confirmar
    /// fecha o campo.
    #[gpui_kit::test]
    fn renomear_no_lugar_comeca_com_o_nome_e_fecha_ao_confirmar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let minha = Preset::user(
            "Retrato".into(),
            PresetAdjustments::vazia().com("exposure", 1.0),
        );
        let id = minha.id;
        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            vec![minha],
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.comecar_a_renomear(id, window, cx);
                assert_eq!(tela.predefinicoes.renomeando.aberto(), Some(&id));
                assert_eq!(tela.renome_do_preset.read(cx).value().as_ref(), "Retrato");

                // Em branco não vale, e o campo fica.
                tela.renomear_preset(id, "  ".into(), cx);
                assert_eq!(tela.predefinicoes.renomeando.aberto(), Some(&id));

                tela.renomear_preset(id, "Retrato claro".into(), cx);
                assert_eq!(tela.predefinicoes.renomeando.aberto(), None);
                assert_eq!(tela.presets[0].name, "Retrato claro");

                tela.comecar_a_renomear(id, window, cx);
                tela.cancelar_renome(window, cx);
                assert_eq!(tela.predefinicoes.renomeando.aberto(), None);
                assert_eq!(tela.presets[0].name, "Retrato claro", "cancelar não muda");
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.renomeados(), vec![(id, "Retrato claro".into())]);
    }

    /// 🔑 **Uma lista só, que rola, com pastas** (dono, 2026-09-30: *"essa
    /// listagem de preset tá ruim de usar e muito pouco intuitivo"*). Antes
    /// eram três listas com teto próprio, e "Do sistema" mostrava duas das
    /// dezoito sem dizer que havia mais.
    ///
    /// Com as vinte do sistema, doze próprias e duas favoritas numa janela de
    /// 1280×720: a lista cabe na coluna, os grupos vêm na ordem Favoritas,
    /// Minhas, Do sistema — **as do fotógrafo à vista sem rolar** (dono,
    /// 2026-09-29: *"precisa aparecer os presets criados por mim"*) —, a pasta
    /// fecha no clique e a busca a reabre. O coração, clicado de verdade, leva
    /// a predefinição ao topo.
    #[gpui_kit::test]
    fn com_as_vinte_do_sistema_as_minhas_e_as_favoritas_continuam_a_vista(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let mut presets = use_cases::presets::presets_de_sistema();
        presets.extend((1..=12).map(|i| {
            Preset::user(
                format!("Minha {i}"),
                PresetAdjustments::vazia().com("exposure", 0.5),
            )
        }));
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            presets,
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.alternar_favorita("sistema:vintage-portra", cx);
                tela.alternar_favorita("sistema:vintage-kodachrome", cx);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(720.)));
        visual.run_until_parked();

        let coluna = visual
            .debug_bounds("coluna-de-presets")
            .expect("a coluna é desenhada");
        let lista = visual
            .debug_bounds("lista-de-predefinicoes")
            .expect("a lista é desenhada");
        assert!(
            coluna.bottom() <= px(720.5) && lista.bottom() <= coluna.bottom() + px(0.5),
            "a lista passou da coluna: {lista:?} em {coluna:?}"
        );
        // 📜 Com o Histórico no pé (03/10/2026), 1280×720 com o Navegador
        // aberto deixa à lista o piso dela (`PISO_DA_LISTA`): ~5 linhas, as
        // Minhas à vista (conferido abaixo). Era 180 sem o Histórico.
        assert!(
            lista.size.height >= px(115.),
            "a lista ficou sem altura: {lista:?} em {coluna:?}"
        );
        let mut anterior: Option<gpui_kit::Bounds<Pixels>> = None;
        for grupo in ["grupo-FAVORITAS", "grupo-MINHAS", "grupo-DO SISTEMA"] {
            let b = visual
                .debug_bounds(grupo)
                .unwrap_or_else(|| panic!("{grupo} não foi desenhado"));
            if let Some(a) = anterior {
                assert!(a.bottom() <= b.top() + px(0.5), "{grupo} fora de ordem");
            }
            anterior = Some(b);
        }
        let minhas = visual.debug_bounds("grupo-MINHAS").expect("as minhas");
        assert!(
            minhas.top() + px(60.) <= lista.bottom(),
            "as minhas ficaram abaixo da dobra: {minhas:?} em {lista:?}"
        );

        // 📁 A pasta fecha no clique: sobra só o título.
        let aberto = visual
            .debug_bounds("grupo-MINHAS")
            .expect("as minhas")
            .size
            .height;
        let pasta = visual.debug_bounds("pasta-MINHAS").expect("a pasta");
        visual.simulate_click(pasta.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
        let fechado = visual
            .debug_bounds("grupo-MINHAS")
            .expect("as minhas")
            .size
            .height;
        assert!(
            fechado < px(30.) && fechado < aberto,
            "a pasta não fechou: {aberto:?} → {fechado:?}"
        );
        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.grupo_fechado(Grupo::Minhas, cx));
                assert!(tela.predefinicoes.ordem.recolhido(Grupo::Minhas));
                // A busca abre todas: o resultado não se esconde numa pasta.
                tela.busca_de_presets
                    .update(cx, |campo, cx| campo.trocar_valor("Minha", window, cx));
                assert!(!tela.grupo_fechado(Grupo::Minhas, cx));
                // Apagada a busca, volta fechada — e o sistema sobe à vista.
                tela.busca_de_presets
                    .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
                assert!(tela.grupo_fechado(Grupo::Minhas, cx));
            })
            .expect("a janela deve estar aberta");
        visual.run_until_parked();

        // 💛 O coração, pelo clique: a primeira do sistema vai para o fim das
        // favoritas.
        let coracao = visual
            .debug_bounds("favorita-sistema:pb-classico")
            .expect("o coração da linha");
        visual.simulate_click(coracao.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
        janela
            .update(cx, |tela, _window, cx| {
                let favoritas: Vec<String> = tela
                    .grupos_da_coluna(cx)
                    .favoritas
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();
                assert_eq!(
                    favoritas,
                    [
                        "Vintage · Portra 400",
                        "Vintage · Kodachrome",
                        "Preto e branco clássico"
                    ]
                );
                assert!(
                    tela.grupos_da_coluna(cx)
                        .sistema
                        .iter()
                        .all(|p| p.name != "Preto e branco clássico"),
                    "a favorita sai do grupo de origem"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔍 **O Navegador recolhe pelo título** (dono, 2026-09-30: *"dê a opção
    /// de recolher o painel do zoom"*): some a miniatura com os botões, fica a
    /// linha do título, e a lista de predefinições ganha a altura. O segundo
    /// clique devolve tudo.
    #[gpui_kit::test]
    fn o_navegador_recolhe_pelo_titulo_e_a_lista_ganha_a_altura(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            use_cases::presets::presets_de_sistema(),
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx)
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(720.)));
        visual.run_until_parked();

        let altura_da_lista = |visual: &mut gpui_kit::VisualTestContext| {
            visual
                .debug_bounds("lista-de-predefinicoes")
                .expect("a lista é desenhada")
                .size
                .height
        };
        let aberta = altura_da_lista(&mut visual);
        assert!(visual.debug_bounds("navegador-miniatura").is_some());

        let pasta = visual.debug_bounds("navegador-pasta").expect("o título");
        visual.simulate_click(pasta.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
        assert!(
            visual.debug_bounds("navegador-miniatura").is_none(),
            "recolhido, a miniatura sai"
        );
        let recolhida = altura_da_lista(&mut visual);
        assert!(
            recolhida >= aberta + px(150.),
            "a lista não ganhou a altura: {aberta:?} → {recolhida:?}"
        );
        janela
            .update(cx, |tela, _, _| {
                assert!(tela.predefinicoes.ordem.navegador_recolhido())
            })
            .expect("a janela deve estar aberta");

        let pasta = visual.debug_bounds("navegador-pasta").expect("o título");
        visual.simulate_click(pasta.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
        assert!(visual.debug_bounds("navegador-miniatura").is_some());
        assert_eq!(altura_da_lista(&mut visual), aberta);
    }

    /// 📦 **O botão direito na pasta abre o menu do grupo** — o "Exportar
    /// grupo…" do Lightroom —, e não abre nem fecha a pasta.
    #[gpui_kit::test]
    fn o_botao_direito_na_pasta_abre_o_menu_do_grupo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            use_cases::presets::presets_de_sistema(),
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx)
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(800.)));
        visual.run_until_parked();

        let fechada_antes = janela
            .update(cx, |tela, _window, cx| {
                tela.grupo_fechado(Grupo::Sistema, cx)
            })
            .expect("a janela deve estar aberta");
        let pasta = visual
            .debug_bounds("pasta-DO SISTEMA")
            .expect("o título da pasta");
        let ponto = pasta.center();
        // O primeiro evento de ponteiro da janela se perde no harness.
        visual.simulate_mouse_move(ponto, None, gpui_kit::Modifiers::none());
        visual.simulate_mouse_down(
            ponto,
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        visual.simulate_mouse_up(
            ponto,
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        visual.run_until_parked();

        janela
            .update(cx, |tela, window, cx| {
                assert!(
                    window
                        .context_stack()
                        .iter()
                        .any(|c| c.contains("PopupMenu")),
                    "o menu não abriu: {:?}",
                    window.context_stack()
                );
                assert!(
                    tela.predefinicoes.alvo_do_menu.is_none(),
                    "o menu não leu a pasta do botão direito"
                );
                assert_eq!(
                    tela.grupo_fechado(Grupo::Sistema, cx),
                    fechada_antes,
                    "o botão direito abriu ou fechou a pasta"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🖱️ **O botão direito abre o menu da linha, e não aplica.** O GPUI chama
    /// o `on_click` também no botão direito: o primeiro menu aplicava a
    /// predefinição ao abrir. O menu abre com o foco e lê a linha clicada.
    #[gpui_kit::test]
    fn o_botao_direito_abre_o_menu_da_linha_sem_aplicar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            use_cases::presets::presets_de_sistema(),
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx)
            })
            .expect("a janela deve estar aberta");
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(800.)));
        visual.run_until_parked();

        let coracao = visual
            .debug_bounds("favorita-sistema:pb-classico")
            .expect("a linha do P&B");
        // No meio da linha, à esquerda do coração: em cima do nome.
        let ponto = gpui_kit::point(coracao.left() - px(80.), coracao.center().y);
        visual.simulate_mouse_down(
            ponto,
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        visual.simulate_mouse_up(
            ponto,
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        visual.run_until_parked();
        let pb = janela
            .update(cx, |tela, _window, _cx| {
                tela.presets
                    .iter()
                    .find(|p| p.name == "Preto e branco clássico")
                    .cloned()
                    .expect("o P&B")
            })
            .expect("a janela deve estar aberta");
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(!tela.em_uso(&pb), "o botão direito aplicou a predefinição");
            })
            .expect("a janela deve estar aberta");

        // O menu abriu, tem o foco e leu a linha clicada.
        janela
            .update(cx, |tela, window, _cx| {
                assert!(
                    window
                        .context_stack()
                        .iter()
                        .any(|c| c.contains("PopupMenu")),
                    "o menu não abriu: {:?}",
                    window.context_stack()
                );
                assert!(
                    tela.predefinicoes.alvo_do_menu.is_none(),
                    "o menu não leu a linha do botão direito"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ✔️ **A que está na foto fica marcada** — e a marca sai quando um
    /// controle que ela define muda.
    #[gpui_kit::test]
    fn a_predefinicao_na_foto_fica_marcada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            use_cases::presets::presets_de_sistema(),
        );
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                let sepia = tela
                    .presets
                    .iter()
                    .find(|p| p.name == "Sépia à moda antiga")
                    .cloned()
                    .expect("a sépia do sistema");
                let pb = tela
                    .presets
                    .iter()
                    .find(|p| p.name == "Preto e branco clássico")
                    .cloned()
                    .expect("o P&B do sistema");
                assert!(!tela.em_uso(&sepia), "a foto crua não tem predefinição");
                tela.aplicar_preset(&sepia, window, cx);
                assert!(tela.em_uso(&sepia), "aplicada, fica marcada");
                assert!(!tela.em_uso(&pb), "só a que está na foto");
                tela.aplicar_preset(&pb, window, cx);
                assert!(
                    tela.em_uso(&pb) && !tela.em_uso(&sepia),
                    "a marca muda de linha"
                );
                // Uma gêmea com os mesmos parâmetros também bate com a foto,
                // mas a marca fica só na clicada.
                let mut gemea = pb.clone();
                gemea.id = domain::entities::PresetId::new();
                gemea.name = "P&B da casa".into();
                gemea.is_system = false;
                assert!(!tela.em_uso(&gemea), "a gêmea não foi clicada");
                tela.aplicar_preset(&gemea, window, cx);
                assert!(
                    tela.em_uso(&gemea) && !tela.em_uso(&pb),
                    "uma marca só, na última clicada"
                );
                tela.aplicar_preset(&pb, window, cx);
                // O P&B recomeça do neutro: qualquer controle mexido depois
                // faz a foto deixar de ser "o P&B".
                assert!(super::super::presets::substitui(&pb));
                tela.ajustes.exposure += 0.3;
                assert!(!tela.em_uso(&pb), "mexer num controle tira a marca");
            })
            .expect("a janela deve estar aberta");
    }

    /// Reordenar: ↑ ↓ andam uma posição, soltar põe antes ou depois, e "ordem
    /// padrão" desfaz. Com a busca ativa, nada se move.
    #[gpui_kit::test]
    fn reordenar_anda_solta_e_volta_a_ordem_padrao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let presets = use_cases::presets::presets_de_sistema();
        let janela = com_presets(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            presets,
        );
        let nomes = |tela: &Revelacao, cx: &App| -> Vec<String> {
            tela.grupos_da_coluna(cx)
                .sistema
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(nomes(tela, cx)[0], "Preto e branco clássico");

                tela.deslocar_preset(Grupo::Sistema, "sistema:sepia", -1, cx);
                assert_eq!(
                    nomes(tela, cx)[..2],
                    ["Sépia à moda antiga", "Preto e branco clássico"]
                );

                // O estilo do estúdio vai para o topo, como no gabarito.
                tela.comecar_arrasto_de_preset(
                    Grupo::Sistema,
                    "sistema:recordarfotos-pb".into(),
                    cx,
                );
                tela.passar_sobre_preset("sistema:sepia", false, cx);
                tela.soltar_preset(cx);
                assert_eq!(nomes(tela, cx)[0], "RecordarFotos P&B");
                assert!(tela.predefinicoes.arrasto.is_none());

                // Com a busca ativa, não se reordena.
                tela.busca_de_presets
                    .update(cx, |campo, cx| campo.trocar_valor("a", window, cx));
                let antes = nomes(tela, cx);
                tela.deslocar_preset(Grupo::Sistema, "sistema:recordarfotos-pb", 1, cx);
                assert_eq!(nomes(tela, cx), antes);
                tela.busca_de_presets
                    .update(cx, |campo, cx| campo.trocar_valor("", window, cx));

                tela.definir_ordem_dos_presets(Grupo::Sistema, None, cx);
                assert_eq!(nomes(tela, cx)[0], "Preto e branco clássico");
            })
            .expect("a janela deve estar aberta");
    }

    /// Sem foto pronta (ou com a revelação travada no site) a coluna trava: o
    /// `+`, a prévia, o aplicar e o reordenar — o `desabilitado` do site. A
    /// levada no balcão continua revelável.
    #[gpui_kit::test]
    fn sem_foto_ou_com_foto_vendida_a_coluna_trava(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-vendida.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.predefinicoes_desligadas(), "sem foto");
                tela.abrir(
                    PhotoViewModel {
                        comprada: true,
                        ..foto("vendida.jpg")
                    },
                    window,
                    cx,
                );
                assert!(!tela.predefinicoes_desligadas(), "levada no balcão");
                tela.abrir(
                    PhotoViewModel {
                        revelacao_travada: true,
                        ..foto("vendida.jpg")
                    },
                    window,
                    cx,
                );
                assert!(tela.predefinicoes_desligadas(), "travada no site");
                assert!(!tela.reordenar_ligado(cx));
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Salvar um preset guarda os 15 campos e o mostra na lista.
    ///
    /// Os dois: se ele fosse só guardado, quem acabou de salvar não veria nada
    /// acontecer e salvaria de novo.
    #[gpui_kit::test]
    fn salvar_preset_guarda_os_ajustes_e_aparece_na_lista(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            Vec::new(),
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);

        janela
            .update(cx, |tela, window, cx| {
                tela.nome_do_preset.update(cx, |estado, cx| {
                    estado.trocar_valor("Retrato claro", window, cx)
                });
                tela.salvar_preset(window, cx);

                assert_eq!(
                    tela.presets
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>(),
                    ["Retrato claro"],
                    "o preset novo tem de aparecer na lista sem esperar reabrir o app"
                );
            })
            .expect("a janela deve estar aberta");

        let salvos = guarda.salvos();
        assert_eq!(salvos.len(), 1);
        assert_eq!(salvos[0].name, "Retrato claro");
        assert_eq!(
            salvos[0].adjustments.get("exposure"),
            Some(1.5),
            "guarda o que está na tela"
        );
        // 🔑 **Só o que saiu do neutro**, como no site. Iam os 15 inteiros — e
        // com isso a segunda predefinição aplicada apagava a primeira, porque
        // ela escrevia o neutro por cima do que já estava lá.
        // A versão de processo vai junto do que foi mexido (`dos_ajustes`).
        assert_eq!(salvos[0].adjustments.len(), 2);
        assert_eq!(salvos[0].adjustments.get("processo"), Some(1.0));
        assert_eq!(salvos[0].adjustments.get("contrast"), None);
    }

    /// 🚨 **O id que vai para o banco é o mesmo que fica na lista da tela.**
    ///
    /// Era o contrário: a tela punha na lista um `Preset::user` com id próprio,
    /// e o use case criava outro ao gravar. Enquanto salvar era o único gesto,
    /// ninguém notava — com renomear e apagar, o comando ia para um id que a
    /// tabela não tem, a linha sumia da tela e voltava na abertura seguinte.
    #[gpui_kit::test]
    fn a_predefinicao_salva_tem_o_mesmo_id_na_tela_e_no_banco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            Vec::new(),
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);

        janela
            .update(cx, |tela, window, cx| {
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.trocar_valor("Claro", window, cx));
                tela.salvar_preset(window, cx);

                let na_tela = tela.presets[0].id;
                assert_eq!(guarda.salvos()[0].id, na_tela);

                // E o gesto seguinte alcança a mesma linha.
                tela.renomear_preset(na_tela, "Mais claro".into(), cx);
                assert_eq!(guarda.renomeados(), vec![(na_tela, "Mais claro".into())]);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Nome vazio (ou só espaços) não salva.
    ///
    /// O legado aceita, e o resultado é uma linha sem rótulo na lista — que não
    /// dá para distinguir das outras nem para apagar, porque apagar preset não
    /// existe em nenhum dos dois apps.
    #[gpui_kit::test]
    fn preset_sem_nome_nao_e_salvo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let guarda = Arc::new(GuardaDeMentira::default());
        let janela = com_guarda(
            cx,
            previews,
            Arc::new(GravadorDeMentira::default()),
            guarda.clone(),
            Vec::new(),
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.trocar_valor("   ", window, cx));
                tela.salvar_preset(window, cx);

                assert!(tela.presets.is_empty());
            })
            .expect("a janela deve estar aberta");

        assert!(guarda.salvos().is_empty());
    }

    /// 🚨 O modo de corte começa no corte que a foto tem — não na foto inteira.
    ///
    /// Abrir o `R` numa foto já cortada e ver o retângulo cobrindo tudo faria
    /// parecer que o corte se perdeu; e aplicar dali apagaria o enquadramento de
    /// verdade.
    #[gpui_kit::test]
    fn o_corte_comeca_de_onde_a_foto_parou(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-cortada.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_crop_x: Some(0.2),
                        edit_crop_y: Some(0.1),
                        edit_crop_width: Some(0.5),
                        edit_crop_height: Some(0.6),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );

                assert!(!tela.cortando());
                tela.alternar_corte(window, cx);
                assert!(tela.cortando());

                let corte = tela.corte_atual();
                assert_eq!(corte.crop_x(), 0.2);
                assert_eq!(corte.crop_width(), 0.5);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Esc` só sai da ferramenta — o que foi feito nela fica, como no site.
    ///
    /// Não há "Cancelar": cada gesto já é o enquadramento da foto, e quem se
    /// arrepende usa `⌘Z`.
    #[gpui_kit::test]
    fn sair_do_enquadrar_mantem_o_que_foi_feito(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-cortada.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("cortada.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                tela.girar(cx);
                tela.cancelar_corte(cx);

                assert!(!tela.cortando());
                assert_eq!(tela.corte.rotacao, Some(1), "o giro ficou");
                assert!(tela.historico.pode_desfazer());
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(
            gravado.len(),
            1,
            "girar gravou uma vez, sair não grava de novo"
        );
        assert_eq!(gravado[0].2.rotacao, Some(1));
    }

    /// 🚨 Arrastar uma alça grava ao soltar — porque nada mais vai gravar por ele.
    ///
    /// O palco tem 80 px para uma foto de 8: cada 10 px de arrasto é um pixel
    /// da foto.
    #[gpui_kit::test]
    fn arrastar_a_alca_grava_ao_soltar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.palco = Bounds::new(
                    gpui_kit::point(px(0.), px(0.)),
                    gpui_kit::size(px(80.), px(80.)),
                );
                tela.alternar_corte(window, cx);

                tela.comecar_arrasto(
                    Some(corte::Alca::Esquerda),
                    gpui_kit::point(px(0.), px(40.)),
                    cx,
                );
                tela.mover_no_corte(gpui_kit::point(px(20.), px(40.)), window, cx);
                assert!(
                    gravador.gravado().is_empty(),
                    "no meio do arrasto não grava"
                );
                tela.soltar_no_corte(window, cx);

                assert!(tela.cortando(), "soltar não fecha a ferramenta");
                assert_eq!(tela.corte.x, Some(0.25));
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert_eq!(gravado[0].2.x, Some(0.25), "o corte novo foi para o banco");
        assert_eq!(gravado[0].2.largura, Some(0.75));
    }

    /// Uma parede com batentes, girada `graus` — para o Auto e a régua.
    fn parede_torta(graus: f32) -> DynamicImage {
        let reta = RgbaImage::from_fn(900, 600, |x, y| {
            let batente = [150u32, 330, 570, 750].iter().any(|b| x.abs_diff(*b) < 3);
            let rodape = y.abs_diff(480) < 3;
            if batente || rodape {
                Rgba([240, 240, 235, 255])
            } else {
                Rgba([130, 115, 100, 255])
            }
        });
        let girada =
            revelacao_core::transformacao::inclinar_inteira(&DynamicImage::ImageRgba8(reta), graus);
        // Sem os cantos transparentes do giro: eles seriam retas também.
        girada.crop_imm(90, 60, 720, 480)
    }

    /// 🚨 A régua: a reta traçada fica reta, num passo só, e a régua desarma.
    #[gpui_kit::test]
    fn a_regua_endireita_num_passo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-regua.jpg", &parede_torta(0.))
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("regua.jpg"), window, cx);
                tela.palco = Bounds::new(
                    gpui_kit::point(px(0.), px(0.)),
                    gpui_kit::size(px(400.), px(300.)),
                );
                tela.alternar_corte(window, cx);
                tela.alternar_regua(cx);

                // Desce 5 a cada 100: girada 2,86° no horário.
                tela.comecar_regua(gpui_kit::point(px(50.), px(100.)), cx);
                tela.mover_no_corte(gpui_kit::point(px(150.), px(105.)), window, cx);
                assert!(gravador.gravado().is_empty(), "no meio do traço não grava");
                tela.soltar_no_corte(window, cx);

                assert_eq!(tela.corte_atual().angle(), -2.9);
                assert!(tela.cortando(), "a régua não fecha o Enquadrar");
                assert!(
                    !tela.edicao.as_ref().unwrap().regua_armada,
                    "a régua serve uma reta e desarma"
                );

                // A mesma reta, traçada de novo sobre a foto já endireitada
                // (agora horizontal na tela), não gira mais nada.
                tela.comecar_regua(gpui_kit::point(px(50.), px(100.)), cx);
                tela.mover_no_corte(gpui_kit::point(px(150.), px(100.)), window, cx);
                tela.soltar_no_corte(window, cx);
                assert_eq!(tela.corte_atual().angle(), -2.9);

                // Um clique não é traço.
                tela.comecar_regua(gpui_kit::point(px(50.), px(100.)), cx);
                tela.mover_no_corte(gpui_kit::point(px(53.), px(98.)), window, cx);
                tela.soltar_no_corte(window, cx);
                assert_eq!(tela.corte_atual().angle(), -2.9);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(
            gravado.len(),
            1,
            "um passo: o traço repetido e o clique não gravam"
        );
        assert_eq!(gravado[0].2.angulo, Some(-2.9));
    }

    /// `Esc` com a régua armada larga só a régua.
    #[gpui_kit::test]
    fn esc_larga_a_regua_antes_do_enquadrar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-esc.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("esc.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                tela.alternar_regua(cx);
                tela.cancelar_corte(cx);
                assert!(tela.cortando());
                assert!(!tela.edicao.as_ref().unwrap().regua_armada);
                tela.cancelar_corte(cx);
                assert!(!tela.cortando());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O Auto endireita pelas retas da foto, e a resposta é um passo.
    #[gpui_kit::test]
    fn o_auto_endireita_pelas_retas(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-auto.jpg", &parede_torta(3.))
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("auto.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                tela.endireitar_automatico(window, cx);
                assert!(tela.edicao.as_ref().unwrap().procurando);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                let edicao = tela.edicao.as_ref().unwrap();
                assert!(!edicao.procurando);
                assert_eq!(edicao.aviso, None);
                assert!(
                    (tela.corte_atual().angle() + 3.).abs() <= 0.15,
                    "girada 3°, o Auto pôs {}°",
                    tela.corte_atual().angle()
                );
            })
            .expect("a janela deve estar aberta");
        assert_eq!(gravador.gravado().len(), 1);
    }

    /// Foto sem reta: o Auto avisa e não mexe em nada.
    #[gpui_kit::test]
    fn o_auto_sem_retas_avisa_e_nao_mexe(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-lisa.jpg", &foto_cinza())
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("lisa.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                tela.endireitar_automatico(window, cx);
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.edicao.as_ref().unwrap().aviso.is_some());
                assert_eq!(tela.corte_atual().angle(), 0.);
            })
            .expect("a janela deve estar aberta");
        assert!(gravador.gravado().is_empty());
    }

    /// 🚨 O endireitar encolhe o retângulo para caber, e cresce de volta.
    #[gpui_kit::test]
    fn endireitar_encolhe_e_volta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let mut grande = RgbaImage::new(100, 80);
        for pixel in grande.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-torta.jpg", &DynamicImage::ImageRgba8(grande))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("torta.jpg"), window, cx);
                tela.alternar_corte(window, cx);

                tela.definir_angulo(10., cx);
                let torto = tela.corte_atual();
                assert_eq!(torto.angle(), 10.);
                assert!(
                    torto.crop_width() < 1.,
                    "encolheu para não mostrar canto vazio"
                );

                tela.definir_angulo(0., cx);
                let reto = tela.corte_atual();
                assert!((reto.crop_width() - 1.).abs() < 1e-3, "cresceu de volta");
                assert!((reto.crop_height() - 1.).abs() < 1e-3);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A foto comprada no site não se revela: nada dela vai para o banco,
    /// e o Enquadrar não abre pelo botão.
    #[gpui_kit::test]
    fn a_foto_comprada_nao_grava(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-vendida.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        revelacao_travada: true,
                        ..foto("vendida.jpg")
                    },
                    window,
                    cx,
                );
                assert!(!tela.pode_revelar());
                assert!(!tela.ha_o_que_salvar());
                tela.ajustes.exposure = 1.;
                tela.pendente = true;
                tela.gravar_o_que_estiver_pendente();
            })
            .expect("a janela deve estar aberta");
        assert!(gravador.gravado().is_empty(), "a comprada não grava");
    }

    /// ⚠️ `R` numa foto sem pixels no cache não abre o modo.
    ///
    /// Sem imagem não há onde pôr o retângulo, e o overlay sobre o vazio daria
    /// oito alças flutuando em lugar nenhum — clicáveis, inclusive.
    #[gpui_kit::test]
    fn foto_sem_cache_nao_entra_no_modo_de_corte(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("sem-cache.NEF"), window, cx);
                tela.alternar_corte(window, cx);
                assert!(!tela.cortando());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Trocar de foto no meio do corte fecha o modo.
    ///
    /// O retângulo é da foto que estava na tela. Mantê-lo aberto aplicaria, no
    /// clique seguinte, o enquadramento de uma foto na outra — o mesmo defeito
    /// que a cópia da seleção e o histórico por foto já impedem nas outras pontas.
    #[gpui_kit::test]
    fn trocar_de_foto_fecha_o_modo_de_corte(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("a.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                assert!(tela.cortando());

                tela.abrir(foto("b.jpg"), window, cx);
                assert!(!tela.cortando(), "o retângulo era da foto a");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A foto abre **cortada** quando tem corte gravado.
    ///
    /// É a última divergência visível que sobrava em relação ao legado: até aqui
    /// a Revelação nova mostrava a foto inteira, e quem tinha enquadrado no app
    /// de egui via o corte desaparecer ao abrir no novo.
    #[gpui_kit::test]
    fn a_foto_abre_com_o_corte_aplicado(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        // 16×16 para a metade dar 8×8 redondo.
        let mut grande = RgbaImage::new(16, 16);
        for pixel in grande.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-cortada.jpg", &DynamicImage::ImageRgba8(grande))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_crop_x: Some(0.0),
                        edit_crop_y: Some(0.0),
                        edit_crop_width: Some(0.5),
                        edit_crop_height: Some(0.5),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );

                let desenhada = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(
                    desenhada.size(0).width.0,
                    8,
                    "metade de 16 — a foto na tela é a cortada, não a inteira"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 No modo de corte a foto volta a aparecer **inteira**.
    ///
    /// É o `apply_crop_clip` do legado. Sem isso, entrar no corte mostraria só o
    /// pedaço já cortado — e não haveria como aumentar o enquadramento de volta,
    /// porque o resto da foto não estaria na tela.
    #[gpui_kit::test]
    fn o_modo_de_corte_mostra_a_foto_inteira(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let mut grande = RgbaImage::new(16, 16);
        for pixel in grande.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-cortada.jpg", &DynamicImage::ImageRgba8(grande))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_crop_width: Some(0.5),
                        edit_crop_height: Some(0.5),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );
                tela.alternar_corte(window, cx);

                let desenhada = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(
                    desenhada.size(0).width.0,
                    16,
                    "inteira, para poder arrastar"
                );

                tela.cancelar_corte(cx);
                let desenhada = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(desenhada.size(0).width.0, 8, "e cortada de volta ao sair");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Cmd+Z` depois de cortar devolve o enquadramento — o item 12.
    ///
    /// Até 6/set/2026 a pilha do histórico guardava só `Ajustes`, e cortar não
    /// deixava marca nenhuma nela: desfazer depois de cortar voltava a
    /// exposição e mantinha o corte novo, como se enquadrar não fosse editar. O
    /// legado erra pior — o `EditSnapshot` de lá tem o campo do corte, grava
    /// nele e nunca o lê de volta.
    #[gpui_kit::test]
    fn desfazer_depois_de_cortar_devolve_o_enquadramento(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let mut grande = RgbaImage::new(16, 16);
        for pixel in grande.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-inteira.jpg", &DynamicImage::ImageRgba8(grande))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("inteira.jpg"), window, cx);
                assert!(
                    !tela.historico.pode_desfazer(),
                    "a foto abriu no passo zero"
                );

                tela.alternar_corte(window, cx);
                tela.gesto_do_corte(
                    CropSettings::new(0.0, 0.0, 0.5, 0.5, 0, 0.0, false, false),
                    cx,
                );
                tela.aplicar_corte(cx);

                assert_eq!(tela.corte.largura, Some(0.5));
                let desenhada = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(desenhada.size(0).width.0, 8, "a foto na tela é a cortada");
                assert!(
                    tela.historico.pode_desfazer(),
                    "cortar tem de ser um passo do histórico"
                );

                tela.desfazer(window, cx);

                assert_eq!(tela.corte, Corte::default(), "o corte voltou a não existir");
                let desenhada = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(desenhada.size(0).width.0, 16, "e a foto voltou inteira");

                tela.refazer(window, cx);
                assert_eq!(tela.corte.largura, Some(0.5), "e o refazer o traz de volta");
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ O corte entra no histórico **depois** do gesto de slider pendente.
    ///
    /// São dois passos, nesta ordem: o arrasto que ainda não fechou vira um
    /// passo com o enquadramento antigo, e só então o corte vira o seguinte.
    /// Fechar na ordem inversa colaria o corte novo num ajuste velho, e um
    /// `Cmd+Z` pularia por cima do enquadramento sem nunca o desfazer.
    #[gpui_kit::test]
    fn o_gesto_pendente_e_o_corte_sao_dois_passos(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let mut grande = RgbaImage::new(16, 16);
        for pixel in grande.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-inteira.jpg", &DynamicImage::ImageRgba8(grande))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("inteira.jpg"), window, cx);

                // Um arrasto que ainda não fechou.
                tela.ajustes.exposure = 1.5;
                tela.pendente = true;

                tela.alternar_corte(window, cx);
                tela.gesto_do_corte(
                    CropSettings::new(0.0, 0.0, 0.5, 0.5, 0, 0.0, false, false),
                    cx,
                );
                tela.aplicar_corte(cx);

                // Primeiro `Cmd+Z`: sai o corte, fica a exposição.
                tela.desfazer(window, cx);
                assert_eq!(tela.corte, Corte::default());
                assert_eq!(tela.ajustes.exposure, 1.5);

                // Segundo: sai a exposição.
                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes.exposure, 0.0);
                assert_eq!(tela.corte, Corte::default());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Girar 90° muda a foto na tela, e não só um número.
    ///
    /// Um botão que mexe no estado sem reprocessar a imagem parece quebrado — e o
    /// defeito só apareceria ao aplicar, quando a foto saltasse de orientação.
    #[gpui_kit::test]
    fn girar_muda_a_foto_na_tela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        // 16×8: deitada, para o giro trocar largura por altura de forma visível.
        let mut deitada = RgbaImage::new(16, 8);
        for pixel in deitada.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-deitada.jpg", &DynamicImage::ImageRgba8(deitada))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("deitada.jpg"), window, cx);
                tela.alternar_corte(window, cx);

                let antes = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!((antes.size(0).width.0, antes.size(0).height.0), (16, 8));

                tela.girar(cx);

                let depois = tela.aberta.as_ref().unwrap().desenhada.as_ref().unwrap();
                assert_eq!(
                    (depois.size(0).width.0, depois.size(0).height.0),
                    (8, 16),
                    "girar 90° troca largura por altura na tela"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O slider de endireitamento nasce no ângulo da foto.
    ///
    /// Numa foto já endireitada, uma barra no meio diria que ela está reta — e o
    /// primeiro toque nela desfaria o endireitamento sem aviso.
    #[gpui_kit::test]
    fn o_slider_de_angulo_abre_no_angulo_da_foto(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-torta.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_crop_angle: Some(-7.5),
                        ..foto("torta.jpg")
                    },
                    window,
                    cx,
                );
                tela.alternar_corte(window, cx);

                assert_eq!(tela.angulo.read(cx).value().start(), -7.5);
            })
            .expect("a janela deve estar aberta");
    }

    /// A proporção remodela o retângulo na hora, e vale para o arrasto seguinte.
    #[gpui_kit::test]
    fn travar_a_proporcao_remodela_e_muda_o_arrasto(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let mut deitada = RgbaImage::new(160, 80);
        for pixel in deitada.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        previews
            .save_preview("id-retrato.jpg", &DynamicImage::ImageRgba8(deitada))
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.palco = Bounds::new(
                    gpui_kit::point(px(0.), px(0.)),
                    gpui_kit::size(px(160.), px(80.)),
                );
                tela.alternar_corte(window, cx);
                tela.travar_proporcao(Some(1.), cx);

                let quadrado = |tela: &Revelacao| {
                    let c = tela.corte_atual();
                    (c.crop_width() * 160., c.crop_height() * 80.)
                };
                let (w, h) = quadrado(tela);
                assert!((w - h).abs() <= 1., "1:1 na hora: {w} × {h}");

                // Encolhe pela esquerda: a altura acompanha a largura.
                tela.comecar_arrasto(
                    Some(corte::Alca::Esquerda),
                    gpui_kit::point(px(40.), px(40.)),
                    cx,
                );
                tela.mover_no_corte(gpui_kit::point(px(60.), px(40.)), window, cx);
                tela.soltar_no_corte(window, cx);
                let (w, h) = quadrado(tela);
                assert!((w - h).abs() <= 1., "1:1 depois do arrasto: {w} × {h}");
                assert!(w < 80.);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O histograma mede a foto **depois** dos ajustes, e muda com eles.
    ///
    /// Um histograma calculado da foto crua descreveria uma imagem que ninguém
    /// está vendo — e o instrumento que existe para dizer "as altas luzes
    /// estouraram" passaria a dizer isso da foto errada.
    #[gpui_kit::test]
    fn o_histograma_acompanha_o_que_esta_na_tela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);

                let histograma = tela.histograma.as_ref().expect("há foto, há histograma");
                // A foto de teste é cinza 100 chapado.
                assert_eq!(histograma.vermelho[100], 64);
                assert_eq!(histograma.vermelho[200], 0);
            })
            .expect("a janela deve estar aberta");

        // Exposição +1 clareia: o balde 100 esvazia e um mais claro enche. A
        // foto nova está no processo do Lightroom, onde +1 leva o 100 a ~150
        // (a curva medida); no processo 0 ia a 200.
        arrastar(cx, &janela, 0, 1.0);
        cx.run_until_parked();

        // O resultado vem da GPU por um laço assíncrono; espera ele chegar —
        // até 20 s: compilar o shader no DX12 leva ~2 s nesta máquina.
        for _ in 0..2000 {
            let pronto = janela
                .update(cx, |tela, _window, _cx| {
                    tela.histograma.as_ref().is_some_and(|h| {
                        h.vermelho[100] == 0 && h.vermelho[130..].iter().any(|v| *v > 0)
                    })
                })
                .expect("a janela deve estar aberta");
            if pronto {
                return;
            }
            // ⚠️ Espera de verdade, e não só relógio: o motor roda numa thread
            // própria, com wgpu do outro lado. `advance_clock` acorda o laço de
            // colheita, mas quem tem de terminar primeiro é a GPU — e ela não
            // sabe do relógio de teste.
            std::thread::sleep(Duration::from_millis(10));
            cx.executor().advance_clock(INTERVALO_DE_COLHEITA * 2);
            cx.run_until_parked();
        }
        let baldes = janela
            .update(cx, |tela, _window, _cx| {
                tela.histograma.as_ref().map(|h| {
                    h.vermelho
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| **v > 0)
                        .map(|(i, v)| (i, *v))
                        .collect::<Vec<_>>()
                })
            })
            .expect("a janela deve estar aberta");
        panic!("o histograma não acompanhou o ajuste: {baldes:?}");
    }

    /// Sem foto não há histograma — e não há divisão por zero em lugar nenhum.
    #[gpui_kit::test]
    fn sem_foto_nao_ha_histograma(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.histograma.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O "antes" troca a foto e **não** mexe nos ajustes.
    ///
    /// Se ele zerasse os sliders para mostrar o original, voltar do "antes"
    /// exigiria refazer a revelação inteira — e o `\\` seria a tecla mais cara do
    /// app em vez da mais barata.
    #[gpui_kit::test]
    fn o_antes_troca_a_foto_e_nao_os_ajustes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);

        janela
            .update(cx, |tela, _window, cx| {
                assert!(!tela.mostrando_original());

                tela.alternar_original(cx);
                assert!(tela.mostrando_original());
                assert_eq!(
                    tela.ajustes().exposure,
                    1.5,
                    "os sliders continuam onde estavam"
                );

                tela.alternar_original(cx);
                assert!(!tela.mostrando_original());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 O duplo clique no rótulo volta **um** controle, e não a foto inteira.
    ///
    /// O erro fácil aqui é chamar `redefinir_ajustes` de dentro do duplo
    /// clique: os dois "voltam ao neutro", e o teste que só olhasse o controle
    /// clicado passaria com os outros 52 apagados junto.
    #[gpui_kit::test]
    fn o_duplo_clique_devolve_so_aquele_controle(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-uma.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(2.0),
                        edit_contrast: Some(1.4),
                        ..foto("uma.jpg")
                    },
                    window,
                    cx,
                );

                tela.devolver_ao_neutro(0, window, cx);

                assert_eq!(tela.ajustes().exposure, 0.0, "o clicado volta");
                assert_eq!(tela.ajustes().contrast, 1.4, "e só ele");
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.0,
                    "o slider acompanha"
                );
                assert!(tela.pode_desfazer(), "é um passo de histórico");
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "vai ao banco na hora, sem a espera");
        assert_eq!(gravado[0].1.exposure, 0.0);
        assert_eq!(gravado[0].1.contrast, 1.4);
    }

    /// ⚠️ **Duplo clique no que já está no neutro não escreve nada.**
    ///
    /// Sem esta guarda, clicar duas vezes num slider parado empilharia um passo
    /// de histórico idêntico ao anterior e mandaria um `UPDATE` ao banco — e o
    /// `Cmd+Z` seguinte pareceria não fazer nada, porque desfaria um passo que
    /// não mudou nada.
    #[gpui_kit::test]
    fn duplo_clique_no_neutro_nao_grava(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-parada.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("parada.jpg"), window, cx);
                tela.devolver_ao_neutro(0, window, cx);
                assert!(!tela.pode_desfazer(), "não houve gesto");
            })
            .expect("a janela deve estar aberta");

        assert!(gravador.gravado().is_empty());
    }

    /// O número do cabeçalho e o ponto âmbar dos painéis saem da mesma medida.
    ///
    /// 🚨 **Neutro não é zero em dois dos 53** (contraste e raio da nitidez), e
    /// contar "quantos são diferentes de zero" acusaria dois ajustes numa foto
    /// que ninguém tocou — com o ponto âmbar aceso em Básico e Detalhe desde a
    /// abertura.
    #[gpui_kit::test]
    fn o_cabecalho_e_o_ponto_ambar_contam_o_que_saiu_do_neutro(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-medida.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()));

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("medida.jpg"), window, cx);
                assert_eq!(tela.quantos_alterados(), 0, "foto crua não tem ajuste");
                for secao in Secao::TODAS {
                    assert!(!tela.secao_alterada(secao), "`{}`", secao.rotulo());
                }

                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.0),
                        edit_hsl_blue_lum: Some(30.0),
                        ..foto("medida.jpg")
                    },
                    window,
                    cx,
                );

                assert_eq!(tela.quantos_alterados(), 2);
                assert!(tela.secao_alterada(Secao::Basico));
                assert!(
                    tela.secao_alterada(Secao::HslLuminancia),
                    "a aba fechada também se anuncia"
                );
                assert!(!tela.secao_alterada(Secao::HslCor), "e a vizinha não");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A foto que chega revelada não acende âmbar** (dono, 2026-09-27).
    ///
    /// O caso visto: uma foto com o RecordarFotos P&B aberta sem ninguém tocar.
    /// O ponto âmbar comparava com o neutro e acendia em cada seção do estilo, com o "Salvar na
    /// galeria e sair" apagado ao lado. Agora âmbar é "mudou desde que abriu"; o
    /// que já estava salvo leva o ponto cinza (`Marca::Ajustado`).
    #[gpui_kit::test]
    fn os_parametros_salvos_leva_o_ponto_cinza_e_so_o_gesto_acende_ambar(cx: &mut TestAppContext) {
        use crate::revelacao::controles::{marca_do_painel, Marca, Painel};
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-pb.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()));
        let mut revelada = foto("pb.jpg");
        persistencia::na_foto(
            &mut revelada,
            Ajustes {
                pcv_amount: 30.0,
                ..Default::default()
            },
            Default::default(),
        );

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(revelada, window, cx);
                let marca =
                    |tela: &Revelacao, p| marca_do_painel(&tela.ajustes(), &tela.salvo(), p);
                assert_eq!(
                    marca(tela, Painel::Efeitos),
                    Some(Marca::Ajustado),
                    "a vinheta salva aparece em Efeitos, em cinza"
                );
                assert_eq!(marca(tela, Painel::Basico), None, "nenhum âmbar");
                assert!(!tela.aberta_a_salvar(), "e o botão de salvar concorda");

                let efeito = tela
                    .controle_onde(|d| d.secao.painel() == Painel::Efeitos && !d.discreto)
                    .expect("um slider contínuo de Efeitos");
                tela.arrastar_slider(efeito, 0.5, cx);
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    marca_do_painel(&tela.ajustes(), &tela.salvo(), Painel::Efeitos),
                    Some(Marca::NaoSalvo)
                );
                assert!(tela.aberta_a_salvar(), "mexeu: âmbar e o botão aceso");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Foto comprada não se zera** — o `podeRevelar` do site. E uma foto
    /// só enquadrada conta como "fora do neutro" para o botão.
    #[gpui_kit::test]
    fn foto_comprada_nao_se_zera_e_a_so_enquadrada_conta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-comprada.jpg", "id-girada.jpg"] {
            previews
                .save_preview(id, &foto_cinza())
                .expect("gravar preview");
        }

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_hsl_blue_lum: Some(30.0),
                        revelacao_travada: true,
                        ..foto("comprada.jpg")
                    },
                    window,
                    cx,
                );
                assert!(!tela.pode_revelar());
                tela.redefinir_ajustes(window, cx);
                assert_eq!(tela.ajustes().hsl_blue_lum, 30.0, "comprada não muda");

                tela.abrir(
                    PhotoViewModel {
                        edit_crop_rotation: Some(90),
                        edit_hsl_blue_lum: Some(30.0),
                        ..foto("girada.jpg")
                    },
                    window,
                    cx,
                );
                assert!(tela.enquadrada(), "girada é enquadrada");
                assert_eq!(tela.quantos_alterados(), 1);
                tela.redefinir_ajustes(window, cx);
                assert_eq!(tela.quantos_alterados(), 0);
                assert!(!tela.enquadrada());
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "só o zerar da revelável grava");
        assert_eq!(gravado[0].0, "id-girada.jpg");
    }

    /// 🚨 **"Zerar tudo" é tudo mesmo, inclusive o enquadramento** (dono,
    /// 2026-09-12, `editor.tsx:2106`). E o corte vai ao banco **escrito** como a
    /// foto inteira: o `Corte::default` quer dizer "não mexa", e deixaria o
    /// recorte de antes de pé.
    #[gpui_kit::test]
    fn zerar_tudo_zera_os_ajustes_e_o_enquadramento(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-cortada.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(2.0),
                        edit_crop_x: Some(0.2),
                        edit_crop_width: Some(0.5),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );

                tela.redefinir_ajustes(window, cx);

                assert_eq!(tela.ajustes().exposure, 0.0);
                assert_eq!(tela.ajustes().contrast, 1.0, "o neutro, e não zero");
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.0,
                    "os sliders voltam junto"
                );
                assert_eq!(tela.corte.x, Some(0.0), "o corte volta à foto inteira");
                assert_eq!(tela.corte.largura, Some(1.0));
                assert!(!tela.enquadrada());
                assert!(tela.pode_desfazer(), "redefinir é um passo de histórico");
                tela.desfazer(window, cx);
                assert_eq!(tela.corte.x, Some(0.2), "e o Cmd+Z devolve o corte junto");
                assert_eq!(tela.ajustes().exposure, 2.0);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 2, "o zerar e o desfazer");
        assert_eq!(gravado[0].1.exposure, 0.0);
        assert_eq!(
            gravado[0].2.largura,
            Some(1.0),
            "e vai ao banco com o corte inteiro escrito"
        );
    }

    /// 🚨 O arrasto da foto anterior não vaza para a próxima **pelo banco**.
    ///
    /// A metade que o teste acima não cobre: com os ajustes vindo da foto, abrir
    /// uma revelada depois de outra revelada tem de trocar os 42 valores, e não
    /// misturar os dois conjuntos. Um `ajustes` que só recebesse os campos
    /// gravados na foto nova manteria os da anterior nos demais — e a segunda
    /// abriria com metade da revelação da primeira.
    #[gpui_kit::test]
    fn abrir_outra_revelada_troca_os_ajustes_inteiros(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(1.0),
                        edit_saturation: Some(0.5),
                        ..foto("a.jpg")
                    },
                    window,
                    cx,
                );

                tela.abrir(
                    PhotoViewModel {
                        edit_exposure: Some(-1.0),
                        ..foto("b.jpg")
                    },
                    window,
                    cx,
                );

                assert_eq!(tela.ajustes().exposure, -1.0);
                assert_eq!(
                    tela.ajustes().saturation,
                    0.0,
                    "a saturação era da foto a — na b ela não existe, e tem de voltar ao neutro"
                );
            })
            .expect("a janela deve estar aberta");
    }

    // ------------------------------------------------ a Revelação local

    fn ponteiro_desce(x: f32, y: f32, alt: bool) -> gpui_kit::MouseDownEvent {
        gpui_kit::MouseDownEvent {
            button: MouseButton::Left,
            position: gpui_kit::point(px(x), px(y)),
            modifiers: gpui_kit::Modifiers {
                alt,
                ..Default::default()
            },
            click_count: 1,
            first_mouse: false,
        }
    }

    fn ponteiro_anda(x: f32, y: f32) -> MouseMoveEvent {
        MouseMoveEvent {
            position: gpui_kit::point(px(x), px(y)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Default::default(),
        }
    }

    /// Um ponto da área do palco em coordenadas da janela — o que o ponteiro
    /// entrega.
    fn na_janela(
        tela: &Revelacao,
        p: crate::revelacao::zoom::Ponto,
    ) -> crate::revelacao::zoom::Ponto {
        crate::revelacao::zoom::Ponto {
            x: p.x + f32::from(tela.palco.origin.x),
            y: p.y + f32::from(tela.palco.origin.y),
        }
    }

    /// Abre a foto cinza num palco de 800×600 — a foto de 8×8 encaixa num
    /// quadrado de 600 no meio, e o centro dela é (400, 300).
    fn aberta_no_palco(
        cx: &mut TestAppContext,
        gravador: Arc<GravadorDeMentira>,
        foto_aberta: PhotoViewModel,
    ) -> (gpui_kit::WindowHandle<Revelacao>, TempDir) {
        let (previews, dir) = previews_descartaveis();
        previews
            .save_preview(&foto_aberta.id, &foto_cinza())
            .expect("gravar preview");
        let janela = com_gravador(cx, previews, gravador);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto_aberta, window, cx);
                tela.medir_o_palco(800., 600.);
            })
            .expect("a janela deve estar aberta");
        (janela, dir)
    }

    /// 🔑 **A máscara que já existe se edita pela alça, e se volta a ela com
    /// duplo clique** (pedido do dono; conferido no app real em 2026-09-27):
    /// a ponta do linear arrastada é um passo só, e o duplo clique sem
    /// ferramenta põe a do componente na mão, com a máscara escolhida.
    #[gpui_kit::test]
    fn a_alca_edita_o_linear_e_o_duplo_clique_volta_a_ele(cx: &mut TestAppContext) {
        let gravador = Arc::new(GravadorDeMentira::default());
        let (janela, _dir) = aberta_no_palco(cx, gravador, foto("retrato.jpg"));
        janela
            .update(cx, |tela, window, cx| {
                tela.usar_ferramenta(Ferramenta::Linear, cx);
                let em = |tela: &Revelacao, q: [f32; 2]| {
                    na_janela(tela, tela.ponto_da_foto(q).expect("a foto na tela"))
                };
                let (a, b) = (em(tela, [0.5, 0.2]), em(tela, [0.5, 0.5]));
                tela.local_apertar(&ponteiro_desce(a.x, a.y, false), window, cx);
                tela.local_mover(&ponteiro_anda(b.x, b.y), cx);
                tela.local_soltar(cx);
                assert_eq!(tela.locais.camadas[0].componentes.len(), 1);

                // A ponta de baixo, arrastada até 0,8: mesmo componente.
                let c = em(tela, [0.5, 0.8]);
                tela.local_apertar(&ponteiro_desce(b.x, b.y, false), window, cx);
                tela.local_mover(&ponteiro_anda(c.x, c.y), cx);
                tela.local_soltar(cx);
                let camada = &tela.locais.camadas[0];
                assert_eq!(camada.componentes.len(), 1, "a alça não cria componente");
                let Forma::Linear(g) = &camada.componentes[0].forma else {
                    panic!("um linear");
                };
                assert!(
                    (g.fim[1] - 0.8).abs() < 0.01,
                    "a ponta foi para 0,8: {:?}",
                    g.fim
                );
                tela.desfazer(window, cx);
                let Forma::Linear(g) = &tela.locais.camadas[0].componentes[0].forma else {
                    panic!("um linear");
                };
                assert!((g.fim[1] - 0.5).abs() < 0.01, "um ⌘Z devolve a ponta");

                // Sem ferramenta, duplo clique sobre a reta.
                tela.usar_ferramenta(Ferramenta::Linear, cx);
                assert!(!tela.com_ferramenta_local());
                tela.local.mascara_sel = None;
                let m = em(tela, [0.5, 0.35]);
                let mut duplo = ponteiro_desce(m.x, m.y, false);
                duplo.click_count = 2;
                tela.ao_apertar(&duplo, window, cx);
                assert_eq!(tela.local.ferramenta, Some(Ferramenta::Linear));
                assert_eq!(tela.local.mascara_sel, Some(0), "a máscara escolhida");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **O traço depois de um ⌘Z não se perde** (achado no app real,
    /// 2026-09-27): a máscara escolhida era a que o ⌘Z desfez, e o traço
    /// seguinte ia para uma camada que não existia mais.
    #[gpui_kit::test]
    fn o_traco_depois_do_desfazer_vira_mascara_nova(cx: &mut TestAppContext) {
        let gravador = Arc::new(GravadorDeMentira::default());
        let (janela, _dir) = aberta_no_palco(cx, gravador, foto("retrato.jpg"));
        janela
            .update(cx, |tela, window, cx| {
                tela.usar_ferramenta(Ferramenta::Pincel, cx);
                let c = na_janela(
                    tela,
                    tela.ponto_da_foto([0.5, 0.5]).expect("a foto na tela"),
                );
                let pincelar =
                    |tela: &mut Revelacao, window: &mut Window, cx: &mut Context<Revelacao>| {
                        tela.local_apertar(&ponteiro_desce(c.x, c.y, false), window, cx);
                        tela.local_mover(&ponteiro_anda(c.x + 40., c.y), cx);
                        tela.local_soltar(cx);
                    };
                pincelar(tela, window, cx);
                tela.desfazer(window, cx);
                assert!(tela.locais.camadas.is_empty());
                pincelar(tela, window, cx);
                assert_eq!(tela.locais.camadas.len(), 1, "o traço virou máscara");
                assert_eq!(
                    tela.locais.camadas[0].nome, "Máscara 1",
                    "com o primeiro nome livre"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Uma pincelada é um passo só**, e o gesto em curso é provisório:
    /// revelado pela GPU, mas fora do histórico e do banco até soltar.
    #[gpui_kit::test]
    fn uma_pincelada_e_um_passo_so_e_so_grava_ao_soltar(cx: &mut TestAppContext) {
        let gravador = Arc::new(GravadorDeMentira::default());
        let (janela, _dir) = aberta_no_palco(cx, gravador.clone(), foto("retrato.jpg"));
        janela
            .update(cx, |tela, window, cx| {
                tela.usar_ferramenta(Ferramenta::Pincel, cx);
                let c = na_janela(
                    tela,
                    tela.ponto_da_foto([0.5, 0.5]).expect("a foto na tela"),
                );
                tela.local_apertar(&ponteiro_desce(c.x, c.y, false), window, cx);
                // O 100 repetido é a janela e o palco ouvindo o mesmo arrasto:
                // a posição parada não vira ponto.
                for dx in [20., 50., 80., 100., 100., 100.] {
                    tela.local_mover(&ponteiro_anda(c.x + dx, c.y), cx);
                }
                // No meio do gesto: a GPU vê, o histórico e o banco não.
                assert_eq!(tela.locais_na_tela().camadas.len(), 1);
                assert!(tela.locais.camadas.is_empty(), "provisório não é revelação");
                assert!(!tela.historico.pode_desfazer());
                assert!(gravador.locais_gravados().is_empty());

                tela.local_soltar(cx);
                let camada = &tela.locais.camadas[0];
                assert_eq!(camada.nome, "Máscara 1");
                let Forma::Pincel(traco) = &camada.componentes[0].forma else {
                    panic!("um pincel");
                };
                assert_eq!(traco.pontos.len(), 5);
                // O centro do palco é o centro da foto.
                assert!(
                    (traco.pontos[0][0] - 0.5).abs() < 0.01
                        && (traco.pontos[0][1] - 0.5).abs() < 0.01
                );
                assert_eq!(
                    gravador.locais_gravados().len(),
                    1,
                    "uma gravação da revelação local"
                );

                tela.desfazer(window, cx);
                assert!(
                    tela.locais.camadas.is_empty(),
                    "um ⌘Z desfaz a pincelada inteira"
                );
                tela.refazer(window, cx);
                assert_eq!(tela.locais.camadas.len(), 1);
            })
            .expect("a janela deve estar aberta");
    }

    /// A revelação local gravada volta ao reabrir a foto — e salvar a máscara não
    /// mexe nos ajustes nem no corte que a foto tinha.
    #[gpui_kit::test]
    fn a_mascara_volta_ao_reabrir_e_nao_mexe_no_corte(cx: &mut TestAppContext) {
        let gravador = Arc::new(GravadorDeMentira::default());
        let mut cortada = foto("cortada.jpg");
        cortada.edit_crop_width = Some(0.8);
        let (janela, _dir) = aberta_no_palco(cx, gravador.clone(), cortada.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.usar_ferramenta(Ferramenta::Radial, cx);
                let c = na_janela(
                    tela,
                    tela.ponto_da_foto([0.5, 0.5]).expect("a foto na tela"),
                );
                tela.local_apertar(&ponteiro_desce(c.x, c.y, false), window, cx);
                tela.local_mover(&ponteiro_anda(c.x + 80., c.y + 60.), cx);
                tela.local_soltar(cx);
            })
            .expect("a janela deve estar aberta");

        let (_, gravado) = gravador
            .locais_gravados()
            .pop()
            .expect("gravou a revelação local");
        let (_, ajustes, corte) = gravador.gravado().pop().expect("gravou a revelação");
        // Os ajustes ficam: o neutro, no processo do Lightroom (foto nova).
        assert!(ajustes.sem_efeito(), "os ajustes ficam");
        assert_eq!(ajustes.processo, 1.0);
        assert_eq!(corte.largura, Some(0.8), "o corte fica");

        let mut reaberta = cortada;
        reaberta.locais = gravado;
        let (outra, _dir2) = aberta_no_palco(cx, Arc::new(GravadorDeMentira::default()), reaberta);
        outra
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.locais.camadas.len(), 1);
                assert!(matches!(
                    tela.locais.camadas[0].componentes[0].forma,
                    Forma::Radial(_)
                ));
            })
            .expect("a janela deve estar aberta");
    }

    /// O band-aid nasce selecionado; arrastar a origem muda só a origem, num
    /// passo; e o `Delete` apaga o retoque — nunca a foto.
    #[gpui_kit::test]
    fn o_band_aid_muda_a_origem_arrastando_e_o_delete_o_apaga(cx: &mut TestAppContext) {
        let gravador = Arc::new(GravadorDeMentira::default());
        let (janela, _dir) = aberta_no_palco(cx, gravador, foto("retrato.jpg"));
        janela
            .update(cx, |tela, window, cx| {
                tela.usar_ferramenta(Ferramenta::BandAid, cx);
                let c = na_janela(
                    tela,
                    tela.ponto_da_foto([0.5, 0.5]).expect("a foto na tela"),
                );
                tela.local_apertar(&ponteiro_desce(c.x, c.y, false), window, cx);
                tela.local_soltar(cx);
                assert_eq!(tela.locais.retoques.len(), 1);
                assert_eq!(tela.local.selecionado, Some(0), "nasce selecionado");
                let antes = tela.locais.retoques[0]
                    .carimbo()
                    .expect("com origem")
                    .clone();

                // Pega a origem (o círculo tracejado) e arrasta 30 pontos.
                let origem = na_janela(tela, tela.ponto_da_foto(antes.origem).expect("na tela"));
                tela.local_apertar(&ponteiro_desce(origem.x, origem.y, false), window, cx);
                tela.local_mover(&ponteiro_anda(origem.x, origem.y + 30.), cx);
                tela.local_soltar(cx);
                let depois = tela.locais.retoques[0]
                    .carimbo()
                    .expect("com origem")
                    .clone();
                assert_eq!(depois.caminho, antes.caminho, "o destino fica");
                assert!(depois.origem[1] > antes.origem[1] + 0.03, "a origem desceu");

                tela.desfazer(window, cx);
                assert_eq!(
                    tela.locais.retoques[0].carimbo().unwrap().origem,
                    antes.origem
                );

                tela.local.selecionado = Some(0);
                assert!(tela.apagar_retoque_selecionado(cx));
                assert!(tela.locais.retoques.is_empty());
            })
            .expect("a janela deve estar aberta");
    }

    /// Com uma ferramenta local na mão, `Esc` larga ela antes de sair.
    #[gpui_kit::test]
    fn o_esc_larga_a_ferramenta_antes(cx: &mut TestAppContext) {
        let (janela, _dir) =
            aberta_no_palco(cx, Arc::new(GravadorDeMentira::default()), foto("a.jpg"));
        janela
            .update(cx, |tela, _window, cx| {
                tela.usar_ferramenta(Ferramenta::Laco, cx);
                assert!(tela.esc_local(cx));
                assert!(!tela.com_ferramenta_local());
                assert!(
                    !tela.esc_local(cx),
                    "sem ferramenta, o Esc segue para a raiz"
                );
            })
            .expect("a janela deve estar aberta");
    }

    // ------------------------------------------------ perspectiva guiada

    /// Um "prédio fotografado de baixo": duas colunas escuras que se aproximam
    /// no topo, sobre fundo claro. 600×400, x das colunas de baixo para cima:
    /// 150 → 190 e 450 → 410.
    fn predio_de_baixo() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(600, 400, |x, y| {
            let yf = y as f32 + 0.5;
            let coluna = |base: f32, topo: f32| base + (topo - base) * (1.0 - yf / 400.0);
            let xf = x as f32 + 0.5;
            if (xf - coluna(150., 190.)).abs() < 3. || (xf - coluna(450., 410.)).abs() < 3. {
                Rgba([20, 20, 20, 255])
            } else {
                Rgba([235, 235, 235, 255])
            }
        }))
    }

    /// O palco de 400×300 dos testes. 🚨 Fixado **dentro** de cada `update`:
    /// entre um e outro a janela desenha, e o `canvas` remede o palco com o
    /// tamanho dela.
    fn palco_de_teste(tela: &mut Revelacao) {
        tela.palco = Bounds::new(
            gpui_kit::point(px(0.), px(0.)),
            gpui_kit::size(px(400.), px(300.)),
        );
    }

    /// Onde o pixel `(x, y)` da foto 600×400 **aparece** agora no palco —
    /// girado, endireitado e corrigido como a tela o mostra. É onde o operador
    /// clicaria.
    fn onde_aparece(tela: &Revelacao, x: f32, y: f32) -> gpui_kit::Point<Pixels> {
        let (px_, py_) = tela
            .ponto_no_palco([x / 600., y / 400.])
            .expect("o ponto aparece no palco");
        gpui_kit::point(px(px_), px(py_))
    }

    /// Traça uma guia sobre os pixels `de` e `ate` da foto, como eles aparecem.
    fn tracar(
        tela: &mut Revelacao,
        de: (f32, f32),
        ate: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Revelacao>,
    ) {
        palco_de_teste(tela);
        let (a, b) = (
            onde_aparece(tela, de.0, de.1),
            onde_aparece(tela, ate.0, ate.1),
        );
        tela.comecar_guia(a, cx);
        tela.mover_no_corte(
            gpui_kit::point((a.x + b.x) / 2., (a.y + b.y) / 2.),
            window,
            cx,
        );
        tela.mover_no_corte(b, window, cx);
        tela.soltar_no_corte(window, cx);
    }

    fn abrir_o_predio(
        cx: &mut TestAppContext,
    ) -> (
        gpui_kit::WindowHandle<Revelacao>,
        Arc<GravadorDeMentira>,
        TempDir,
    ) {
        let (previews, dir) = previews_descartaveis();
        previews
            .save_preview("id-predio.jpg", &predio_de_baixo())
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("predio.jpg"), window, cx);
                tela.palco = Bounds::new(
                    gpui_kit::point(px(0.), px(0.)),
                    gpui_kit::size(px(400.), px(300.)),
                );
                tela.alternar_corte(window, cx);
                tela.alternar_guias(cx);
            })
            .expect("a janela deve estar aberta");
        (janela, gravador, dir)
    }

    /// 🚨 **Duas guias sobre as colunas endireitam a foto**, cada guia é um
    /// passo, e o arquivo sai com as colunas paralelas.
    #[gpui_kit::test]
    fn duas_guias_endireitam_o_predio(cx: &mut TestAppContext) {
        let (janela, gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                // Um ângulo antigo: a correção guiada o zera ao ligar.
                tela.angulo_como_gesto(3.0, window, cx);

                tracar(tela, (151., 380.), (188., 20.), window, cx);
                let p = *tela.corte_atual().perspectiva();
                assert_eq!(p.quantas_guias(), 1);
                assert!(!p.corrige(), "uma guia só não corrige");
                assert!(
                    tela.edicao
                        .as_ref()
                        .unwrap()
                        .guias
                        .aviso
                        .as_ref()
                        .is_some_and(|a| a.contains("mais uma")),
                    "o painel pede a segunda guia"
                );

                tracar(tela, (449., 380.), (412., 20.), window, cx);
                let c = tela.corte_atual();
                let p = *c.perspectiva();
                assert_eq!(p.quantas_guias(), 2);
                assert!(p.corrige(), "duas guias corrigem");
                assert!(p
                    .guias
                    .iter()
                    .flatten()
                    .all(|g| g.eixo == domain::value_objects::EixoDaGuia::Vertical));
                assert_eq!(
                    c.angle(),
                    0.,
                    "o endireitar volta a zero quando a correção liga"
                );
                assert!(
                    c.crop_width() < 1.0,
                    "restringir encolheu o retângulo para fora dos cantos vazios"
                );

                // O arquivo: as colunas ficam paralelas.
                let origem = predio_de_baixo();
                let arquivo =
                    infrastructure::transformacao::aplicar(&origem, &tela.enquadramento(), true)
                        .to_rgba8();
                let centro = |y: u32| -> Vec<f32> {
                    let mut xs = Vec::new();
                    let mut dentro = None;
                    for x in 0..arquivo.width() {
                        let escuro = arquivo.get_pixel(x, y).0[0] < 128;
                        match (escuro, dentro) {
                            (true, None) => dentro = Some(x),
                            (false, Some(x0)) => {
                                xs.push((x0 + x - 1) as f32 / 2.);
                                dentro = None;
                            }
                            _ => {}
                        }
                    }
                    xs
                };
                let (alto, baixo) = (
                    centro(arquivo.height() / 6),
                    centro(arquivo.height() * 5 / 6),
                );
                assert_eq!(alto.len(), 2, "{alto:?}");
                assert_eq!(baixo.len(), 2, "{baixo:?}");
                for k in 0..2 {
                    assert!(
                        (alto[k] - baixo[k]).abs() <= 2.0,
                        "coluna {k}: {} × {}",
                        alto[k],
                        baixo[k]
                    );
                }
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        // ângulo, guia 1, guia 2 (esta já com o ângulo a zero, no mesmo passo)
        assert_eq!(gravado.len(), 3, "um passo por gesto");
        let ultimo = gravado.last().unwrap().2;
        assert_eq!(ultimo.angulo, Some(0.));
        assert!(ultimo
            .perspectiva
            .is_some_and(|p| p.corrige() && p.quantas_guias() == 2));
    }

    /// Desfazer tira a correção da segunda guia; refazer a devolve.
    #[gpui_kit::test]
    fn desfazer_e_refazer_a_perspectiva(cx: &mut TestAppContext) {
        let (janela, _gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                tracar(tela, (151., 380.), (188., 20.), window, cx);
                tracar(tela, (449., 380.), (412., 20.), window, cx);
                let corrigida = *tela.corte_atual().perspectiva();
                assert!(corrigida.corrige());

                tela.desfazer(window, cx);
                let p = *tela.corte_atual().perspectiva();
                assert_eq!(p.quantas_guias(), 1);
                assert!(!p.corrige());
                assert!(tela.tracando_guias(), "desfazer não desliga o traçado");

                tela.refazer(window, cx);
                assert_eq!(*tela.corte_atual().perspectiva(), corrigida);
            })
            .expect("a janela deve estar aberta");
    }

    /// Trocar o eixo, apagar pela tecla, e o `Esc` que larga o traçado.
    #[gpui_kit::test]
    fn eixo_apagar_e_esc_das_guias(cx: &mut TestAppContext) {
        let (janela, _gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                tracar(tela, (151., 380.), (188., 20.), window, cx);
                tracar(tela, (449., 380.), (412., 20.), window, cx);
                // Um traço deitado vira guia horizontal.
                tracar(tela, (100., 300.), (500., 305.), window, cx);
                let p = *tela.corte_atual().perspectiva();
                assert_eq!(
                    p.guias[2].unwrap().eixo,
                    domain::value_objects::EixoDaGuia::Horizontal
                );

                tela.trocar_eixo(2, window, cx);
                assert_eq!(
                    tela.corte_atual().perspectiva().guias[2].unwrap().eixo,
                    domain::value_objects::EixoDaGuia::Vertical
                );
                // Vertical "deitada" demais: fica de fora da conta, e o painel diz.
                assert!(tela
                    .edicao
                    .as_ref()
                    .unwrap()
                    .guias
                    .aviso
                    .as_ref()
                    .is_some_and(|a| a.contains("guia 3")));

                tela.edicao.as_mut().unwrap().guias.selecionada = Some(2);
                assert!(tela.apagar_guia_selecionada(window, cx));
                assert_eq!(tela.corte_atual().perspectiva().quantas_guias(), 2);
                assert!(
                    tela.corte_atual().perspectiva().corrige(),
                    "as duas verticais continuam valendo"
                );

                // Esc: primeiro desliga o traçado, depois sai do Enquadrar.
                tela.cancelar_corte(cx);
                assert!(tela.cortando() && !tela.tracando_guias());
                tela.cancelar_corte(cx);
                assert!(!tela.cortando());
            })
            .expect("a janela deve estar aberta");
    }

    /// Arrastar a ponta de uma guia não reamostra nada até soltar: o corte
    /// não muda no meio do gesto, e o soltar é um passo só.
    #[gpui_kit::test]
    fn arrastar_a_ponta_corrige_ao_soltar(cx: &mut TestAppContext) {
        let (janela, gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                tracar(tela, (151., 380.), (188., 20.), window, cx);
                tracar(tela, (449., 380.), (400., 20.), window, cx); // topo errado
                let antes = tela.corte_atual();
                let gravados = gravador.gravado().len();

                palco_de_teste(tela);
                // A foto na tela está corrigida pelas guias de agora: o ponto
                // (412, 20) da foto aparece onde a conta de agora o põe.
                let alvo = onde_aparece(tela, 412., 20.);
                tela.comecar_arrasto_da_ponta(1, super::perspectiva::Ponta::Ate, cx);
                tela.mover_no_corte(onde_aparece(tela, 406., 20.), window, cx);
                tela.mover_no_corte(alvo, window, cx);
                assert_eq!(tela.corte_atual(), antes, "no meio do arrasto a foto fica");
                assert_eq!(gravador.gravado().len(), gravados, "e nada grava");
                tela.soltar_no_corte(window, cx);
                assert_ne!(tela.corte_atual(), antes);
                assert_eq!(gravador.gravado().len(), gravados + 1);
                let g = tela.corte_atual().perspectiva().guias[1].unwrap();
                assert!((g.ate[0] - 412. / 600.).abs() < 0.01, "{:?}", g.ate);
            })
            .expect("a janela deve estar aberta");
    }

    /// Os sliders fazem o ajuste fino por cima; restringir desligado devolve
    /// o retângulo pedido; redefinir tira tudo.
    #[gpui_kit::test]
    fn ajuste_fino_restringir_e_redefinir(cx: &mut TestAppContext) {
        let (janela, gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                tela.ajuste_do_slider(true, 6.0, cx);
                tela.gravar_o_que_estiver_pendente();
                let c = tela.corte_atual();
                assert_eq!(c.perspectiva().vertical, 6.0);
                assert!(c.crop_width() < 1.0, "o topo alargou: o retângulo encolheu");

                tela.alternar_restringir(window, cx);
                let c = tela.corte_atual();
                assert!(!c.restringir());
                assert_eq!(
                    (c.crop_width(), c.crop_height()),
                    (1.0, 1.0),
                    "sem restringir, o retângulo pedido volta"
                );

                tela.redefinir_perspectiva(window, cx);
                assert!(tela.corte_atual().perspectiva().e_neutra());
            })
            .expect("a janela deve estar aberta");
        let gravado = gravador.gravado();
        assert!(gravado.iter().any(|g| g.2.restringir == Some(false)));
        assert!(gravado.last().unwrap().2.perspectiva.is_none());
    }

    /// Girar depois leva guias e correção junto: a foto girada continua
    /// corrigida, e o eixo da guia aparece trocado na tela.
    #[gpui_kit::test]
    fn girar_depois_mantem_a_perspectiva(cx: &mut TestAppContext) {
        let (janela, _gravador, _dir) = abrir_o_predio(cx);
        janela
            .update(cx, |tela, window, cx| {
                tracar(tela, (151., 380.), (188., 20.), window, cx);
                tracar(tela, (449., 380.), (412., 20.), window, cx);
                let p = *tela.corte_atual().perspectiva();
                tela.girar(cx);
                assert_eq!(
                    *tela.corte_atual().perspectiva(),
                    p,
                    "girar não apaga a correção"
                );
                assert_eq!(tela.corte_atual().rotation_90(), 1);
                let g = p.guias[0].unwrap();
                assert_eq!(
                    tela.eixo_na_tela(g.eixo),
                    domain::value_objects::EixoDaGuia::Horizontal
                );
                tela.espelhar_horizontal(cx);
                assert_eq!(*tela.corte_atual().perspectiva(), p);
            })
            .expect("a janela deve estar aberta");
    }

    /// A foto reaberta traz guias e correção da revelação.
    #[gpui_kit::test]
    fn a_perspectiva_volta_ao_reabrir(cx: &mut TestAppContext) {
        let (janela, gravador, _dir) = abrir_o_predio(cx);
        let mut p = domain::value_objects::PerspectivaGuiada {
            rotacao: [7.0, 0.0, 0.5],
            vertical: 1.5,
            ..Default::default()
        };
        p.guias[0] = Some(domain::value_objects::GuiaDePerspectiva {
            de: [0.25, 0.95],
            ate: [0.31, 0.05],
            eixo: domain::value_objects::EixoDaGuia::Vertical,
        });
        janela
            .update(cx, |tela, window, cx| {
                let mut reaberta = foto("predio.jpg");
                let parametros = crate::pos_venda::porta::ajustes_em_json(
                    &Ajustes::default(),
                    &CropSettings::default()
                        .with_perspectiva(p)
                        .with_restringir(false),
                );
                reaberta.parametros = Some(parametros.to_string());
                reaberta.ler_perspectiva_dos_parametros(&parametros);
                tela.sair_do_corte(cx);
                tela.abrir(reaberta, window, cx);
                assert_eq!(*tela.corte_atual().perspectiva(), p);
                assert!(!tela.corte_atual().restringir());
                tela.alternar_corte(window, cx);
                assert_eq!(
                    tela.persp_vertical.read(cx).value().start(),
                    1.5,
                    "o slider abre no valor da foto"
                );
            })
            .expect("a janela deve estar aberta");
        let _ = gravador;
    }

    // ------------------------------------------------------------ 📜 Histórico

    /// Abre a foto e deixa a leitura do histórico gravado chegar.
    fn abrir_e_ler(
        cx: &mut TestAppContext,
        janela: &gpui_kit::WindowHandle<Revelacao>,
        nome: &str,
    ) {
        janela
            .update(cx, |tela, window, cx| tela.abrir(foto(nome), window, cx))
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
    }

    fn nomes_do_historico(
        cx: &mut TestAppContext,
        janela: &gpui_kit::WindowHandle<Revelacao>,
    ) -> Vec<String> {
        janela
            .update(cx, |tela, _, _| {
                tela.passos_do_historico()
                    .0
                    .iter()
                    .map(|p| p.rotulo.nome.clone())
                    .collect()
            })
            .expect("a janela deve estar aberta")
    }

    fn desenhar_a_coluna(
        cx: &mut TestAppContext,
        janela: &gpui_kit::WindowHandle<Revelacao>,
    ) -> gpui_kit::VisualTestContext {
        let visual = gpui_kit::VisualTestContext::from_window((*janela).into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1440.), px(900.)));
        visual.run_until_parked();
        visual
    }

    /// 📜 Cada gesto vira uma linha com nome, variação e valor — e vai ao
    /// catálogo.
    #[gpui_kit::test]
    fn cada_gesto_vira_uma_linha_do_historico(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        abrir_e_ler(cx, &janela, "retrato.jpg");

        arrastar(cx, &janela, 0, 0.3);
        passar_a_espera(cx);
        arrastar(cx, &janela, 1, 55.0);
        passar_a_espera(cx);

        assert_eq!(
            nomes_do_historico(cx, &janela),
            ["Início", "Exposição", "Contraste"]
        );
        janela
            .update(cx, |tela, _, _| {
                let (passos, atual) = tela.passos_do_historico();
                assert_eq!(atual, 2);
                assert_eq!(passos[1].rotulo.variacao.as_deref(), Some("+0,30"));
                assert_eq!(passos[1].rotulo.valor.as_deref(), Some("+0,30"));
                assert_eq!(passos[2].rotulo.variacao.as_deref(), Some("+55"));
            })
            .expect("a janela deve estar aberta");
        let gravado = gravador
            .historicos
            .lock()
            .unwrap()
            .get("id-retrato.jpg")
            .cloned()
            .expect("o histórico foi ao catálogo");
        assert!(gravado.contains("Contraste"), "{gravado}");
    }

    /// 📜 O clique numa linha antiga: a foto e o banco voltam àquele passo, e
    /// os de depois **ficam** até o próximo gesto. Clicado de verdade.
    #[gpui_kit::test]
    fn clicar_numa_linha_volta_a_foto_e_guarda_os_de_depois(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());
        abrir_e_ler(cx, &janela, "retrato.jpg");
        arrastar(cx, &janela, 0, 0.3);
        passar_a_espera(cx);
        arrastar(cx, &janela, 0, 0.8);
        passar_a_espera(cx);

        let mut visual = desenhar_a_coluna(cx, &janela);
        let linha = visual
            .debug_bounds("historico-passo-1")
            .expect("a linha do primeiro arrasto");
        let coluna = visual.debug_bounds("coluna-de-presets").expect("a coluna");
        assert!(
            coluna.contains(&linha.center()),
            "a linha ficou fora da coluna: {linha:?} em {coluna:?}"
        );
        // O primeiro evento de ponteiro da janela se perde no harness.
        visual.simulate_mouse_move(linha.center(), None, gpui_kit::Modifiers::none());
        visual.simulate_click(linha.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();

        janela
            .update(cx, |tela, _, cx| {
                assert_eq!(tela.ajustes().exposure, 0.3);
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.3,
                    "o slider volta junto"
                );
                let (passos, atual) = tela.passos_do_historico();
                assert_eq!((passos.len(), atual), (3, 1), "o 0,8 continua lá");
                assert!(tela.pode_refazer());
            })
            .expect("a janela deve estar aberta");
        assert_eq!(
            gravador.gravado().last().map(|g| g.1.exposure),
            Some(0.3),
            "o clique chega ao banco"
        );
    }

    /// 📜 Passar o mouse mostra o passo na foto sem mudar nada; sair devolve.
    #[gpui_kit::test]
    fn passar_o_mouse_numa_linha_mostra_o_passo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()));
        abrir_e_ler(cx, &janela, "retrato.jpg");
        arrastar(cx, &janela, 0, 0.6);
        passar_a_espera(cx);

        let mut visual = desenhar_a_coluna(cx, &janela);
        let inicio = visual.debug_bounds("historico-passo-0").expect("o Início");
        visual.simulate_mouse_move(inicio.center(), None, gpui_kit::Modifiers::none());
        visual.simulate_mouse_move(inicio.center(), None, gpui_kit::Modifiers::none());
        visual.run_until_parked();
        janela
            .update(cx, |tela, _, _| {
                assert_eq!(
                    tela.ajustes_na_tela().exposure,
                    0.0,
                    "a foto mostra o Início"
                );
                assert_eq!(tela.ajustes().exposure, 0.6, "e nada mudou");
            })
            .expect("a janela deve estar aberta");

        let palco = gpui_kit::point(px(700.), px(300.));
        visual.simulate_mouse_move(palco, None, gpui_kit::Modifiers::none());
        visual.run_until_parked();
        janela
            .update(cx, |tela, _, _| {
                assert_eq!(tela.ajustes_na_tela().exposure, 0.6, "sair devolve");
            })
            .expect("a janela deve estar aberta");
    }

    /// 📜 O ✕ deixa só a foto de agora.
    #[gpui_kit::test]
    fn o_x_limpa_o_historico(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()));
        abrir_e_ler(cx, &janela, "retrato.jpg");
        arrastar(cx, &janela, 0, 0.6);
        passar_a_espera(cx);

        let mut visual = desenhar_a_coluna(cx, &janela);
        let x = visual.debug_bounds("historico-limpar").expect("o ✕");
        visual.simulate_mouse_move(x.center(), None, gpui_kit::Modifiers::none());
        visual.simulate_click(x.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();

        assert_eq!(nomes_do_historico(cx, &janela), ["Início"]);
        janela
            .update(cx, |tela, _, _| {
                assert_eq!(tela.ajustes().exposure, 0.6, "a foto fica como está");
                assert!(!tela.pode_desfazer());
            })
            .expect("a janela deve estar aberta");
    }

    /// 📜 O histórico fica com a foto: trocar de foto e voltar o traz de volta.
    #[gpui_kit::test]
    fn o_historico_volta_com_a_foto(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["id-a.jpg", "id-b.jpg"] {
            previews
                .save_preview(nome, &foto_cinza())
                .expect("gravar preview");
        }
        let gravador_do_teste = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador_do_teste.clone());
        abrir_e_ler(cx, &janela, "a.jpg");
        arrastar(cx, &janela, 0, 0.4);
        passar_a_espera(cx);

        abrir_e_ler(cx, &janela, "b.jpg");
        assert_eq!(
            nomes_do_historico(cx, &janela),
            ["Início"],
            "a B tem o dela"
        );

        // A foto A volta com o que foi gravado nela — do jeito que o
        // catálogo a devolve.
        let mut a = foto("a.jpg");
        let (_, ajustes, corte) = gravador_do_teste.gravado().last().cloned().expect("gravou");
        persistencia::na_foto(&mut a, ajustes, corte);
        janela
            .update(cx, |tela, window, cx| tela.abrir(a, window, cx))
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
        assert_eq!(nomes_do_historico(cx, &janela), ["Início", "Exposição"]);
        janela
            .update(cx, |tela, window, cx| {
                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0, "o ⌘Z alcança o de antes");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A foto mudou fora da Revelação (colada na Biblioteca): o histórico
    /// ganha o passo de fora, e o `⌘Z` volta ao que estava.
    #[gpui_kit::test]
    fn a_foto_mudada_por_fora_vira_um_passo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for nome in ["id-a.jpg", "id-b.jpg"] {
            previews
                .save_preview(nome, &foto_cinza())
                .expect("gravar preview");
        }
        let gravador_do_teste = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador_do_teste.clone());
        abrir_e_ler(cx, &janela, "a.jpg");
        arrastar(cx, &janela, 0, 0.4);
        passar_a_espera(cx);
        abrir_e_ler(cx, &janela, "b.jpg");

        // Colada na Biblioteca: a exposição mudou no catálogo.
        let mut a = foto("a.jpg");
        let (_, mut ajustes, corte) = gravador_do_teste.gravado().last().cloned().expect("gravou");
        ajustes.exposure = 1.2;
        persistencia::na_foto(&mut a, ajustes, corte);
        janela
            .update(cx, |tela, window, cx| tela.abrir(a, window, cx))
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert_eq!(
            nomes_do_historico(cx, &janela),
            ["Início", "Exposição", crate::revelacao::historico::DE_FORA]
        );
        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.ajustes().exposure, 1.2, "a foto abre como está");
                tela.desfazer(window, cx);
                assert_eq!(tela.ajustes().exposure, 0.4);
            })
            .expect("a janela deve estar aberta");
    }

    /// 📜 Com muitos passos numa tela 1280×720, o Histórico rola dentro de
    /// um terço da coluna, e as predefinições continuam à vista.
    #[gpui_kit::test]
    fn com_muitos_passos_o_historico_rola_e_as_predefinicoes_ficam(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");
        let janela = com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()));
        abrir_e_ler(cx, &janela, "retrato.jpg");
        for i in 1..=30 {
            arrastar(cx, &janela, 0, i as f32 / 10.0);
            passar_a_espera(cx);
        }

        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui_kit::size(px(1280.), px(720.)));
        visual.run_until_parked();

        let coluna = visual.debug_bounds("coluna-de-presets").expect("a coluna");
        let painel = visual.debug_bounds("historico").expect("o Histórico");
        assert!(
            painel.size.height <= px(40.),
            "em 720 px, com o Navegador aberto, o Histórico é só o título: {painel:?} em {coluna:?}"
        );
        assert!(
            painel.bottom() <= coluna.bottom() + px(0.5),
            "o Histórico passou do pé da coluna: {painel:?} em {coluna:?}"
        );
        assert!(visual.debug_bounds("historico-passo-30").is_none());

        // Recolher o Navegador devolve a altura: as linhas aparecem, o mais
        // novo no topo, à vista.
        janela
            .update(cx, |tela, _, cx| tela.alternar_navegador(cx))
            .expect("a janela deve estar aberta");
        visual.run_until_parked();
        let coluna = visual.debug_bounds("coluna-de-presets").expect("a coluna");
        let painel = visual.debug_bounds("historico").expect("o Histórico");
        let janela_do_historico = visual
            .debug_bounds("janela-do-historico")
            .expect("a lista do Histórico");
        assert!(
            janela_do_historico.size.height >= px(100.),
            "o Navegador recolhido não deu linhas ao Histórico: {janela_do_historico:?}"
        );
        assert!(
            painel.bottom() <= coluna.bottom() + px(0.5),
            "o Histórico passou do pé da coluna: {painel:?} em {coluna:?}"
        );
        let novo = visual
            .debug_bounds("historico-passo-30")
            .expect("o mais novo");
        assert!(
            janela_do_historico.contains(&novo.center()),
            "o mais novo não está à vista: {novo:?} em {janela_do_historico:?}"
        );
    }
}
