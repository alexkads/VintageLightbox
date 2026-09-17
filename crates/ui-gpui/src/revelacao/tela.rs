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

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::services::PreviewType;
use domain::value_objects::CropSettings;
use gpui::AnimationExt;
use gpui::{
    canvas, div, prelude::*, px, AnyElement, Bounds, Context, Entity, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, RenderImage, SharedString,
    Subscription, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::slider::{SliderEvent, SliderState};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable, WindowExt};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::miniaturas::CacheDeMiniaturas;
use crate::imagem::para_gpui;
use crate::tema;

use super::automatico;
use super::controles::{Definicao, CONTROLES};
use super::corte;
use super::histograma::Histograma;
use super::historico::{Estado, Historico};
use super::lightroom::{Arquivo, EscolhaDePresets, Relatorio};
use super::persistencia::{self, Corte, Gravador};
use super::presets::GuardaDePresets;
use super::processador::{Ajustes, Pedido, Processador};
use super::reposicao::APor;
use super::sincronizacao::{self, Escolha, Grupo};
use infrastructure::transformacao;

/// O Enquadrar: retângulo, alças, transferidor e o painel dele.
mod enquadrar;
/// O zoom no palco e o Navegador.
mod navegacao;
/// O bruto em resolução cheia quando o zoom passa da cópia.
mod resolucao;
/// A coluna das predefinições.
mod predefinicoes;

/// A coluna da direita: cabeçalho, abas sRGB/RGB, painéis e gráficos.
mod painel;
/// A tira do rodapé: recortes, puxador, menu e miniaturas (ver `tira.rs`).
mod tira;

use enquadrar::Edicao;

/// Largura da coluna de ajustes — os `w-80` do site.
const LADO_DO_PAINEL: f32 = 320.0;

/// A coluna das predefinições, à esquerda — os `w-56` do site.
const LADO_DOS_PRESETS: f32 = 224.0;

/// A altura da barra do topo — os `h-12` do site.
const ALTURA_DO_CABECALHO: f32 = 48.0;

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

pub struct Revelacao {
    previews: Arc<PreviewManager>,
    gravador: Arc<dyn Gravador>,
    /// Se a coluna das predefinições está à mostra. É o botão de painel do
    /// site, e some por inteiro quando fechada — uma coluna vazia de 224px
    /// roubaria da foto o espaço que ela não usa.
    presets_a_mostra: bool,
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
    rolagem_da_tira: gpui::ScrollHandle,
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
    /// Os passos de desfazer, **por foto**: trocar de foto começa um histórico
    /// novo. Um `Cmd+Z` que atravessasse fotos aplicaria a revelação de uma na
    /// outra — que é o mesmo defeito que a cópia da seleção já impede.
    historico: Historico,
    /// Se há ajuste que ainda não foi gravado. Um `bool`, e não "a tarefa existe":
    /// a tarefa continua existindo depois de terminar, e trocar de foto gravaria
    /// de novo o que já estava no banco.
    pendente: bool,
    /// A espera do próximo salvamento. Guardada porque **descartá-la cancela** —
    /// é assim que cada movimento novo do slider adia a gravação em vez de
    /// enfileirar mais uma.
    _gravacao: Option<Task<()>>,
    controles: Vec<Controle>,
    /// O corte em edição. `None` é "fora do modo de corte" — e é a diferença
    /// entre a foto com overlay por cima e a foto sozinha.
    edicao: Option<Edicao>,
    /// Se a tela está mostrando o "antes" — a foto sem nenhum ajuste, no mesmo
    /// enquadramento. É o `\\` do legado.
    mostrando_original: bool,
    /// Quantas fotos deste ensaio têm receita nova que o site ainda não recebeu.
    ///
    /// 🔑 **Quem conta é a raiz** — a fila de envio é dela (`a_subir` e o
    /// depósito), e esta tela só a mostra. Existe porque desde 2026-09-11 o
    /// "Sincronizar" copia parâmetros sem subir nada: sem um número no botão de
    /// salvar, o operador sairia do ensaio achando que o cliente já está vendo o
    /// que ele acabou de fazer.
    nao_salvas: usize,
    /// A foto aberta está no depósito, esperando subir — quem sabe é a raiz.
    aberta_no_deposito: bool,
    /// A receita com que a foto abriu: diferente dela, há o que salvar.
    receita_ao_abrir: Option<Estado>,
    /// A tela do cliente está aberta? O botão fica âmbar, como no site.
    cliente_aberto: bool,
    /// "Gerando o JPEG…" no botão de baixar.
    gerando_jpeg: bool,
    /// O "Salvar na galeria e sair" em curso: `(respondidas, total)`.
    salvando: Option<(usize, usize)>,
    /// O histograma da foto **como ela está na tela**. Recalculado junto com a
    /// exibição, e `None` enquanto não há foto.
    histograma: Option<Histograma>,
    /// O slider de endireitamento. Entidade própria, como os 42 do painel.
    angulo: Entity<SliderState>,
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
    /// Se a próxima predefinição salva guarda os 53 (e não só o que saiu do
    /// neutro) — a caixa "Zerar os outros ajustes ao aplicar" do site.
    preset_inteiro: bool,
    /// O id do pedido que ainda não voltou. `None` é "a tela está em dia".
    aguardando: Option<u64>,
    /// Se já existe um laço de colheita rodando. Sem esta trava, cada arrasto
    /// abriria um laço novo e a tela acabaria com dezenas deles perguntando a
    /// mesma coisa.
    colhendo: bool,
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
}

struct Controle {
    definicao: &'static Definicao,
    estado: Entity<SliderState>,
}

struct Aberta {
    foto: PhotoViewModel,
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
                    let SliderEvent::Change(valor) = evento;
                    (definicao.aplicar)(&mut tela.ajustes, valor.start());
                    tela.pedir_revelacao(cx);
                    tela.adiar_gravacao(cx);
                },
            ));

            controles.push(Controle { definicao, estado });
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
                let SliderEvent::Change(valor) = evento;
                tela.angulo_do_slider(valor.start(), cx);
            },
        ));

        Self {
            previews,
            gravador,
            presets_a_mostra: true,
            acervo: Arc::new(Vec::new()),
            miniaturas_da_tira: CacheDeMiniaturas::nova(
                NonZeroUsize::new(MINIATURAS_DA_TIRA).expect("não é zero"),
            ),
            rolagem_da_tira: gpui::ScrollHandle::new(),
            ultima_na_tira: None,
            posicao: 0,
            marcadas: BTreeSet::new(),
            escolha: None,
            processador: Processador::novo(),
            aberta: None,
            ajustes: Ajustes::default(),
            corte: Corte::default(),
            historico: Historico::novo(Estado::default()),
            pendente: false,
            _gravacao: None,
            controles,
            edicao: None,
            mostrando_original: false,
            nao_salvas: 0,
            aberta_no_deposito: false,
            receita_ao_abrir: None,
            cliente_aberto: false,
            gerando_jpeg: false,
            salvando: None,
            histograma: None,
            angulo,
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
            preset_inteiro: false,
            aguardando: None,
            colhendo: false,
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
        // 🔑 **A seta anda sobre a tira**, e não sobre o acervo: com um recorte
        // aceso, "a próxima" é a próxima que se vê (`naTira` do site).
        if let Some(nova) = tira::vizinha(&self.na_tira(), self.posicao, passo) {
            self.posicao = nova;
            self.mostrar_a_posicao(window, cx);
        }
    }

    /// Vai direto para uma posição — o clique no filmstrip.
    pub fn ir_para(&mut self, posicao: usize, window: &mut Window, cx: &mut Context<Self>) {
        if posicao >= self.acervo.len() || posicao == self.posicao {
            return;
        }
        self.posicao = posicao;
        self.mostrar_a_posicao(window, cx);
    }

    pub fn posicao(&self) -> usize {
        self.posicao
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
            // Ajuste **ou** enquadramento fora do neutro — o `temOQueZerar`.
            .filter(|f| {
                persistencia::da_foto(f) != Ajustes::default()
                    || !corte::e_inteiro(&persistencia::para_crop_settings(
                        &persistencia::corte_da_foto(f),
                    ))
            })
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

    /// A foto aberta tem receita que a galeria ainda não recebeu — o `sujo &&
    /// podeRevelar` do site.
    pub fn aberta_a_salvar(&self) -> bool {
        self.pode_revelar()
            && (self.aberta_no_deposito
                || self
                    .receita_ao_abrir
                    .is_some_and(|receita| receita != self.estado()))
    }

    /// Há o que salvar na galeria? Sem isso o botão se apaga.
    pub fn ha_o_que_salvar(&self) -> bool {
        self.aberta_a_salvar() || self.nao_salvas > 0
    }

    pub fn definir_gerando_jpeg(&mut self, gerando: bool, cx: &mut Context<Self>) {
        if self.gerando_jpeg != gerando {
            self.gerando_jpeg = gerando;
            cx.notify();
        }
    }

    pub fn definir_salvando(&mut self, salvando: Option<(usize, usize)>, cx: &mut Context<Self>) {
        if self.salvando != salvando {
            self.salvando = salvando;
            cx.notify();
        }
    }

    pub fn gerando_jpeg(&self) -> bool {
        self.gerando_jpeg
    }

    pub fn definir_cliente_aberto(&mut self, aberto: bool, cx: &mut Context<Self>) {
        if self.cliente_aberto != aberto {
            self.cliente_aberto = aberto;
            cx.notify();
        }
    }

    /// A raiz gravou a receita nas marcadas: as cópias da tira passam a dizer
    /// o mesmo que o banco. Sem isto a seta seguinte abriria a foto
    /// recém-sincronizada com os sliders de antes.
    pub fn aplicar_sincronizadas(
        &mut self,
        gravadas: &[(String, Ajustes, Corte)],
        cx: &mut Context<Self>,
    ) {
        let acervo = Arc::make_mut(&mut self.acervo);
        for (id, ajustes, corte) in gravadas {
            if let Some(foto) = acervo.iter_mut().find(|f| &f.id == id) {
                persistencia::na_foto(foto, *ajustes, *corte);
            }
        }
        cx.notify();
    }

    /// A caixa do "Sincronizar N" — as flags do Lightroom, como no site
    /// (`sincronizar-dialogo.tsx`, pedido do dono de 2026-09-05: *"coloque
    /// flags, escolhe tudo e desmarcar algumas coisas"*).
    ///
    /// ⚠️ O diálogo é do `gpui-component`, e depende do `Root` na primeira
    /// camada da janela — o mesmo aviso do diálogo de salvar preset.
    pub fn abrir_sincronizacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let quantas = self.alvos_da_sincronizacao().len();
        if quantas < 2 {
            return;
        }
        // A escolha da última vez, lida do disco uma vez por abertura do app.
        if self.escolha.is_none() {
            self.escolha = Some(sincronizacao::ler());
        }
        let esta = cx.entity();

        window.open_dialog(cx, move |dialogo, _window, cx| {
            let escolha = esta.read(cx).escolha_da_sincronizacao();
            let tudo = escolha.tudo();
            let para_tudo = esta.clone();
            let para_ok = esta.clone();

            dialogo
                .title(SharedString::from(format!("Sincronizar {quantas} fotos")))
                .confirm()
                .child(
                    div().text_xs().child(
                        "O que estiver marcado vai desta foto para as outras escolhidas na tira. \
                         O resto fica como está em cada uma.",
                    ),
                )
                .child(
                    div()
                        .id("sincronizar-tudo")
                        .pt(px(8.))
                        .cursor_pointer()
                        .text_xs()
                        .child(if tudo { "Desmarcar tudo" } else { "Marcar tudo" })
                        .on_click(move |_ev, _window, cx| {
                            para_tudo.update(cx, |tela, cx| {
                                tela.escolha_mut().marcar_tudo(!tudo);
                                cx.notify();
                            });
                        }),
                )
                .children(Grupo::TODOS.into_iter().map(|grupo| {
                    let ligado = escolha.ligado(grupo);
                    let para_alternar = esta.clone();
                    div()
                        .id(SharedString::from(format!("sincronizar-{}", grupo.chave())))
                        .pt(px(4.))
                        .cursor_pointer()
                        .text_sm()
                        .child(SharedString::from(format!(
                            "{} {}",
                            if ligado { "☑" } else { "☐" },
                            grupo.rotulo()
                        )))
                        .children(grupo.detalhe().map(|detalhe| {
                            div().pl(px(18.)).text_xs().child(detalhe)
                        }))
                        // 🚨 O único que costuma estar errado no destino, e a
                        // caixa diz isso quando ele é ligado.
                        .when(grupo == Grupo::Enquadramento && ligado, |item| {
                            item.child(div().pl(px(18.)).text_xs().child(
                                "O recorte desta foto vale para todas — confira se a composição é a mesma.",
                            ))
                        })
                        .on_click(move |_ev, _window, cx| {
                            para_alternar.update(cx, |tela, cx| {
                                tela.escolha_mut().alternar(grupo);
                                cx.notify();
                            });
                        })
                }))
                .child(div().pt(px(8.)).text_xs().child(
                    "A receita vai para cada foto marcada; as que já estão no site são \
                     reveladas em resolução cheia e salvas na galeria.",
                ))
                .on_ok(move |_ev, _window, cx| {
                    para_ok.update(cx, |tela, cx| {
                        let escolha = tela.escolha_da_sincronizacao();
                        // Nada marcado é nada a fazer — o site desliga o botão;
                        // aqui a caixa fecha sem sincronizar.
                        if !escolha.tem_algo() {
                            return;
                        }
                        sincronizacao::gravar(&escolha);
                        cx.emit(PedidoDaRevelacao::Sincronizar);
                    });
                    true
                })
        });
    }

    fn mostrar_a_posicao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let vizinhas: Vec<String> = [self.posicao.checked_sub(1), Some(self.posicao + 1)]
            .into_iter()
            .flatten()
            .filter_map(|i| self.acervo.get(i))
            // 🔑 **A chave é a de onde o bruto daquela foto mora.** Adiantar
            // `site:<id>` aquecia a imagem da galeria — que a Revelação nem usa
            // como origem —, e deixava fria justamente a que a seta vai pedir.
            .map(|foto| {
                if persistencia::so_existe_no_site(foto) {
                    persistencia::chave_do_trabalho(&foto.id)
                } else {
                    foto.id.clone()
                }
            })
            .collect();
        if vizinhas.is_empty() {
            return;
        }

        let previews = self.previews.clone();
        cx.background_executor()
            .spawn(async move {
                for id in vizinhas {
                    let _ = previews.get_preview(&id);
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

        // 🚨 **A foto do site tem bruto em outra chave.** Em `site:<id>` a
        // grade da sessão guarda a imagem da **galeria** — depois de "Salvar na
        // galeria", a foto revelada e com marca. Servir isso ao shader aplica a
        // receita duas vezes, em 640px, e era o que a tela fazia até 7/set.
        //
        // O bruto dela é a **cópia de trabalho**, que a raiz busca no storage e
        // guarda em `trabalho:<id>` (ver `chave_do_trabalho`). Duas imagens,
        // duas chaves: aqui a origem sai da segunda, e a da galeria fica só como
        // **espera** na tela enquanto o download não volta.
        let do_site = persistencia::so_existe_no_site(&foto);
        let trabalho = do_site
            .then(|| {
                self.previews
                    .get_preview(&persistencia::chave_do_trabalho(&foto.id))
            })
            .flatten();

        // Preview primeiro, miniatura como queda. A miniatura fica borrada numa
        // tela inteira, e é de propósito: mostrar a foto em tamanho errado é
        // melhor do que mostrar retângulo vazio.
        let bruta = trabalho.clone().or_else(|| {
            self.previews
                .get_preview(&foto.id)
                .or_else(|| self.previews.get_thumbnail(&foto.id))
        });

        // 🔑 **Sem cópia de trabalho, a foto do site não tem origem** — e é a
        // ausência de origem que faz a raiz ir buscá-la (`tem_pixels`). Com ela,
        // a seta de volta não custa rede nenhuma.
        let para_origem = if do_site { trabalho } else { bruta.clone() };
        let origem = para_origem.as_ref().map(|imagem| {
            let rgba = imagem.to_rgba8();
            Origem {
                largura: rgba.width(),
                altura: rgba.height(),
                pixels: Arc::new(rgba.into_raw()),
            }
        });

        // Os ajustes vêm da **foto**, e não do que estava no painel: é o que o
        // legado faz ao selecionar (`app.rs`, "Load saved edits FIRST"), e é o
        // que impede as duas metades do mesmo defeito — herdar o slider da foto
        // anterior aplicaria a revelação de uma foto em outra, e ignorar o banco
        // mostraria o arquivo cru de uma foto que já foi revelada.
        self.ajustes = persistencia::da_foto(&foto);
        self.corte = persistencia::corte_da_foto(&foto);
        // 🚨 O modo de corte fecha aqui. O retângulo que está na tela é da foto
        // que sai; mantê-lo aberto aplicaria, no clique seguinte, o enquadramento
        // de uma foto na outra — o mesmo defeito que a cópia da seleção e o
        // histórico por foto já impedem nas outras pontas.
        self.edicao = None;
        // Histórico novo, começando no que está gravado: o passo zero é o estado
        // da abertura, e é o que faz o **primeiro** `Cmd+Z` ter para onde voltar.
        // No legado não tem — lá o histórico começa depois da primeira mudança, e
        // a primeira coisa que se faz numa foto não tem volta.
        self.historico = Historico::novo(self.estado());
        self.esquecer_a_resolucao();
        self.receita_ao_abrir = Some(self.estado());
        self.aguardando = None;
        // 🚨 **A reposição da foto anterior não vale para esta.** Herdar o
        // sinalizador faria a foto nova abrir dizendo "preparando" sem ninguém
        // ter pedido nada — e ficar assim para sempre, porque o `Reposto` que
        // apagaria o sinalizador é o da outra.
        self.repondo = false;

        self.aberta = Some(Aberta {
            foto,
            origem,
            bruta: bruta.clone(),
            revelada: bruta,
            desenhada: None,
        });
        self.atualizar_exibicao();

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
        if self.ajustes != Ajustes::default() {
            self.pedir_revelacao(cx);
        }

        // 🔑 **A tira recomeça no palco novo.** A tarefa anterior é cancelada ao
        // ser substituída: ela estaria enchendo a tira a partir de onde o
        // operador não está mais olhando, e o que já carregou continua no cache.
        self.carregar_a_tira(cx);

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

        let rgba = imagem.to_rgba8();
        self.resolucao.recomecar_na_copia();
        aberta.origem = Some(Origem {
            largura: rgba.width(),
            altura: rgba.height(),
            pixels: Arc::new(rgba.into_raw()),
        });
        aberta.bruta = Some(imagem.clone());
        aberta.revelada = Some(imagem);
        aberta.desenhada = None;
        self.repondo = false;

        // 🔑 **A tira também guardou a ausência.** O `CacheDeMiniaturas` é um
        // LRU de resultados, e `Ausente` é um resultado: sem este esquecimento
        // a célula desta foto continuaria um retângulo preto ao lado da foto
        // que acabou de aparecer, até a rolagem despejá-la por acaso.
        self.miniaturas_da_tira.esquecer(foto_id);

        self.atualizar_exibicao();
        self.pedir_revelacao(cx);
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
        }
    }

    /// Manda o estado de agora para o banco, sem passar pelo histórico.
    /// Grava a receita da foto aberta — no banco **e na cópia que está em
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
        self.gravador.gravar(id.clone(), self.ajustes, self.corte);

        let (ajustes, corte) = (self.ajustes, self.corte);
        if let Some(aberta) = self.aberta.as_mut() {
            persistencia::na_foto(&mut aberta.foto, ajustes, corte);
        }
        let acervo = Arc::make_mut(&mut self.acervo);
        if let Some(foto) = acervo.iter_mut().find(|f| f.id == id) {
            persistencia::na_foto(foto, ajustes, corte);
        }
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
        self.ajustes = estado.ajustes;
        self.corte = estado.corte;
        // O Enquadrar continua aberto, como no site: o retângulo é lido do
        // corte da foto, e o pedido do operador recomeça do que voltou.
        if let Some(edicao) = self.edicao.as_mut() {
            *edicao = Edicao {
                proporcao: edicao.proporcao,
                ..Edicao::default()
            };
        }
        let graus = self.corte_atual().angle();
        self.angulo
            .update(cx, |estado, cx| estado.set_value(graus, window, cx));
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        // O corte não passa pela GPU: quem o mostra é a exibição.
        self.atualizar_exibicao();
        self.gravar();
        cx.notify();
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
        self.historico.registrar(self.estado());
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
        CropSettings::new(
            self.corte.x.unwrap_or(0.0),
            self.corte.y.unwrap_or(0.0),
            self.corte.largura.unwrap_or(1.0),
            self.corte.altura.unwrap_or(1.0),
            self.corte.rotacao.unwrap_or(0),
            self.corte.angulo.unwrap_or(0.0),
            self.corte.espelho_h.unwrap_or(false),
            self.corte.espelho_v.unwrap_or(false),
        )
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
    fn atualizar_exibicao(&mut self) {
        self.refazer_exibicao(true);
    }

    /// O mesmo, sem tocar no histograma — para o "antes/depois", que troca a
    /// imagem e mantém a régua.
    fn atualizar_exibicao_mantendo_histograma(&mut self) {
        self.refazer_exibicao(false);
    }

    fn refazer_exibicao(&mut self, medir: bool) {
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

        let exibida = fonte.map(|imagem| transformacao::aplicar(imagem, &corte, recortar));

        // 🔑 O histograma mede **o que está na tela**, e não a foto crua: com os
        // sliders mexidos, o histograma do cru descreveria uma imagem que ninguém
        // está vendo. É o que o legado faz (ele calcula depois do `process_image`).
        if medir {
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
        let Some(Aberta {
            origem: Some(origem),
            ..
        }) = &self.aberta
        else {
            return;
        };

        let id = self.processador.proximo_id();
        self.processador.pedir(Pedido {
            id,
            pixels: origem.pixels.clone(),
            largura: origem.largura,
            altura: origem.altura,
            ajustes: self.ajustes_na_tela(),
            corte: transformacao::corte(&self.corte_na_tela()),
        });
        self.aguardando = Some(id);
        self.acompanhar(cx);
        cx.notify();
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
        if let Some(resultado) = self.processador.colher() {
            if let Some(aberta) = self.aberta.as_mut() {
                aberta.revelada = Some(resultado.imagem);
            }
            self.atualizar_exibicao();
            // Só larga a espera se o que voltou é o último pedido. No meio de um
            // arrasto chegam resultados de valores já ultrapassados, e parar de
            // colher ali deixaria a foto congelada num ajuste que o dedo já
            // passou.
            if self.aguardando == Some(resultado.id) {
                self.aguardando = None;
                // 🔑 **Chegou o último: a tira mostra o que o palco mostra.**
                // Este é o único instante em que a foto revelada está pronta e
                // parada — no meio de um arrasto os resultados são de valores
                // que o dedo já passou, e reduzir cada um seria refazer a
                // miniatura sessenta vezes por segundo para mostrar a penúltima.
                self.atualizar_a_tira_com_o_revelado();
            }
            cx.notify();
        }

        let continua = self.aguardando.is_some();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn palco(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
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
                                    gpui::Animation::new(CRUZAMENTO_DA_FOTO)
                                        .with_easing(gpui::ease_out_quint()),
                                    |foto, quanto| foto.opacity(quanto),
                                )
                                .into_any_element(),
                        })
                        .children(self.caixa_de_zoom())
                        .children(self.overlay_de_corte(cx))
                        // O `canvas` mede o palco e é onde o arrasto se liga:
                        // registrar ouvinte de mouse exige estar na fase de
                        // pintura, e um `div` comum não chega lá.
                        .child(self.medida_e_arrasto(cx))
                        // "− Encaixar +", como no canto do palco do site; no
                        // Enquadrar não há zoom, e o lugar é do transferidor.
                        .when(self.edicao.is_none(), |palco| {
                            palco.child(self.controle_de_zoom(cx))
                        })
                        .children(self.folha_de_atalhos(cx)),
                )
                .into_any_element(),
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
            move |_bounds, _prepaint, window, _cx| {
                if !arrastando {
                    return;
                }

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &MouseMoveEvent, fase, window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| tela.mover_no_corte(evento.position, window, cx));
                    }
                });

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, _window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| tela.soltar_no_corte(cx));
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
    /// "Baixar JPEG" do site.
    Exportar,
    /// "Tela do cliente" do site: abre ou fecha a tela do segundo monitor.
    TelaDoCliente,
    /// "Salvar na galeria e sair" do site: o revelado entra no lugar do
    /// original, na foto que já é da galeria aberta.
    SalvarNaGaleria,
    /// "Sincronizar N" do site: os ajustes desta foto vão para as marcadas na
    /// tira — a receita para o catálogo, e a foto revelada para o site.
    Sincronizar,
    /// "Zerar N fotos": as marcadas na tira voltam ao neutro.
    ///
    /// 🔑 **É a contrapartida do `Sincronizar`** — um leva a receita desta foto
    /// para as marcadas, o outro devolve todas ao neutro (pedido do dono,
    /// 2026-09-11). A foto aberta não vem por aqui: ela é zerada na própria
    /// tela, por `redefinir_ajustes`, para o `Cmd+Z` desfazer o gesto.
    ZerarAsMarcadas,
    /// "Baixar como… (N)" do menu da tira: a exportação com essas fotos
    /// (`Revelacao::levar_a_baixar`).
    BaixarComo,
    /// Outra foto entrou no palco — pela seta, pela tira ou ao abrir.
    ///
    /// 🔑 **A Revelação não sabe buscar na nuvem, e não vai passar a saber**;
    /// mas é ela quem sabe quando a foto trocou. Sem este aviso a raiz só
    /// buscava os pixels da foto do site **na abertura**: a seta seguinte
    /// caía numa foto sem bruto, e o que aparecia era a miniatura revelada da
    /// galeria — em 640px e com a receita por cima da receita.
    AbriuOutraFoto,
    /// O zoom passou da cópia de trabalho: a tela quer o bruto da foto aberta
    /// em resolução cheia ([`Revelacao::receber_bruto`]).
    QueroOBruto,
}

impl gpui::EventEmitter<PedidoDaRevelacao> for Revelacao {}

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
        self.acompanhar_a_resolucao(cx);
        let cabecalho = self.cabecalho(cx);
        let presets = self
            .presets_a_mostra
            .then(|| self.coluna_dos_presets(cx).into_any_element());
        let palco = self.palco(cx);
        let ajustes = self.painel(cx).into_any_element();
        let tira = self.filmstrip(window, cx);

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
                    .children(presets)
                    // 🚨 **Relativo, com a moldura absoluta dentro.** Com
                    // `size_full` num item flex sem altura definida, a foto
                    // crescia até o tamanho natural e passava por baixo da tira
                    // (dono, 2026-09-17: "a foto precisa caber").
                    .child(div().relative().flex_1().min_w(px(0.)).child(palco))
                    .child(
                        div()
                            .w(px(LADO_DO_PAINEL))
                            .flex_none()
                            .border_l_1()
                            .border_color(cx.theme().border)
                            .child(ajustes),
                    ),
            )
            .children(tira)
    }
}

impl Revelacao {
    /// A barra do topo — a do site, na mesma ordem.
    ///
    /// ⚠️ **Ela ocupa a janela inteira**, e não só a largura da foto: no site
    /// não há barra de aplicativo por cima (o editor é `fixed inset-0`), e a
    /// raiz esconde a dela enquanto a Revelação está no ar. Por isso o `✕` daqui
    /// é o único caminho de volta visível — e ele faz o mesmo que o `Esc`.
    fn cabecalho(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::recursos::Icone;
        use gpui_component::Icon;

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
                    .tooltip("Mostrar ou esconder as predefinições")
                    .selected(self.presets_a_mostra)
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.presets_a_mostra = !tela.presets_a_mostra;
                        tela.prever(None, cx);
                        cx.notify();
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
            .children(self.processador.backend().map(|backend| {
                div()
                    .flex_none()
                    .px(px(4.))
                    .py(px(1.))
                    .rounded(px(3.))
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
                    .tooltip(|w, cx| {
                        Tooltip::new("Segure para ver a foto sem ajuste (ou a tecla \\)").build(w, cx)
                    })
                    .when(pronto, |b| {
                        b.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(true, cx)),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(false, cx)),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|tela, _, _, cx| tela.ver_o_antes(false, cx)),
                        )
                        .on_hover(cx.listener(|tela, dentro: &bool, _, cx| {
                            if !dentro {
                                tela.ver_o_antes(false, cx);
                            }
                        }))
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
                .tooltip(|w, cx| {
                    Tooltip::new("Girar, espelhar, endireitar e recortar (tecla R)").build(w, cx)
                })
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
                    .tooltip({
                        let texto = if self.cliente_aberto {
                            "Fechar a tela do cliente"
                        } else {
                            "Abrir a tela do cliente no outro monitor: ela mostra esta foto, revelada, enquanto você ajusta"
                        };
                        move |w, cx| Tooltip::new(texto).build(w, cx)
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
                    Button::new("revelacao-sincronizar")
                        .icon(Icon::new(Icone::Copy))
                        .label(format!("Sincronizar {quantas}"))
                        .small()
                        .outline()
                        .tooltip(
                            "Copiar os ajustes desta foto para as outras escolhidas na tira — elas sobem quando você salvar",
                        )
                        .disabled(!pronto || !pode_revelar)
                        .on_click(cx.listener(|tela, _ev, window, cx| {
                            tela.abrir_sincronizacao(window, cx)
                        })),
                )
            })
            // As duas últimas do site: "Baixar JPEG" e "Salvar na galeria e
            // sair". Aqui elas **pedem à raiz**, que é quem tem o modal da
            // pasta de destino e a conversa com o pós-venda.
            .child(
                Button::new("revelacao-exportar")
                    .when(!self.gerando_jpeg, |b| b.icon(Icon::new(Icone::Download)))
                    .loading(self.gerando_jpeg)
                    .label(if self.gerando_jpeg {
                        "Gerando o JPEG…"
                    } else {
                        "Baixar JPEG"
                    })
                    .small()
                    .outline()
                    .disabled(!pronto || self.gerando_jpeg)
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::Exportar);
                    })),
            )
            .child({
                let ha = self.ha_o_que_salvar();
                let outras = self.nao_salvas;
                let dica = if !ha {
                    "Nada a salvar: o que está no canvas já está na galeria".to_string()
                } else if outras > 0 {
                    if pode_revelar {
                        format!("Salva esta e mais {outras} com edição pendente, e fecha o editor")
                    } else {
                        format!(
                            "Esta foi comprada e não se revela; salva as {outras} pendentes e fecha o editor"
                        )
                    }
                } else {
                    "Salva esta foto na galeria e fecha o editor".to_string()
                };
                let rotulo = match self.salvando {
                    None => "Salvar na galeria e sair".to_string(),
                    Some((_, 1)) => "Gravando…".to_string(),
                    Some((feitas, total)) => format!("Salvando {}/{total}…", (feitas + 1).min(total)),
                };
                Button::new("revelacao-salvar-na-galeria")
                    .when(self.salvando.is_none(), |b| b.icon(Icon::new(Icone::Save)))
                    .loading(self.salvando.is_some())
                    .label(rotulo)
                    .tooltip(dica)
                    .small()
                    .primary()
                    .disabled(!pronto || !ha || self.salvando.is_some())
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::SalvarNaGaleria);
                    }))
            })
            .into_any_element()
    }

    /// A coluna da esquerda: só as predefinições, sem título e sem aba.
    fn coluna_dos_presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("coluna-de-presets")
            .w(px(LADO_DOS_PRESETS))
            .flex_none()
            .h_full()
            .p(px(8.))
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            // O Navegador vem antes das predefinições, como no site.
            .child(self.navegador(cx))
            .child(self.presets(cx))
    }
}

/// Os botões pequenos da barra do site (`rounded px-2 py-1 text-xs`):
/// cinza, ou âmbar quando ligados.
fn pilula(
    id: &'static str,
    ligada: bool,
    desligada: bool,
    cx: &mut Context<Revelacao>,
) -> gpui::Stateful<gpui::Div> {
    let (fundo, texto) = (cx.theme().muted, cx.theme().foreground);
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(4.))
        .text_xs()
        .when(ligada, |b| b.bg(gpui::rgb(0xfbbf24)).text_color(gpui::black()))
        .when(!ligada, |b| {
            b.bg(fundo)
                .text_color(texto.opacity(0.9))
                .hover(move |s| s.text_color(texto))
        })
        .when(desligada, |b| b.opacity(0.4))
        .when(!desligada, |b| b.cursor_pointer())
}

#[cfg(test)]
mod testes {
    use super::*;
    use biblioteca_core::selecao::Modificadores;
    use crate::biblioteca::miniaturas::Miniatura;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use domain::entities::preset::PresetAdjustments;

    use super::super::controles::Secao;
    use super::super::lightroom::mentira::EscolhaDeMentira;
    use super::super::persistencia::mentira::GravadorDeMentira;
    use super::super::presets::mentira::GuardaDeMentira;
    use super::super::presets::ordem::Grupo;
    use gpui::App;

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
    ) -> gpui::WindowHandle<Revelacao> {
        com_gravador(cx, previews, Arc::new(GravadorDeMentira::default()))
    }

    fn com_gravador(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
    ) -> gpui::WindowHandle<Revelacao> {
        com_presets(cx, previews, gravador, Vec::new())
    }

    fn com_presets(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
        gravador: Arc<GravadorDeMentira>,
        presets: Vec<Preset>,
    ) -> gpui::WindowHandle<Revelacao> {
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
    ) -> gpui::WindowHandle<Revelacao> {
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
    ) -> gpui::WindowHandle<Revelacao> {
        cx.update(gpui_component::init);
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
    #[gpui::test]
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
        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.draw(
            gpui::Point::default(),
            gpui::size(px(1200.), px(800.)),
            |_window, _cx| gpui::Empty,
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

    /// 🚨 **"Não tem preview no cache" era um beco sem saída.**
    ///
    /// A foto está catalogada, o JPEG está no disco, e a Revelação parava numa
    /// frase — sem botão, sem recuperação, sem nada acontecendo. Este teste
    /// cobra a lista que a raiz usa para repor: a foto sem cache tem de
    /// aparecer nela, com o caminho do arquivo.
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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

            let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
            visual.draw(
                gpui::Point::default(),
                gpui::size(px(2000.), px(1300.)),
                |_window, _cx| gpui::Empty,
            );
            visual.run_until_parked();

            let inicio = std::time::Instant::now();
            for _ in 0..QUADROS {
                // O que um arrasto faz: marca sujo e o quadro remonta tudo.
                janela
                    .update(cx, |_tela, _window, cx| cx.notify())
                    .expect("a janela deve estar aberta");
                visual.draw(
                    gpui::Point::default(),
                    gpui::size(px(2000.), px(1300.)),
                    |_window, _cx| gpui::Empty,
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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

    /// Passa da espera do salvamento, sem esperar de verdade.
    fn passar_a_espera(cx: &mut TestAppContext) {
        cx.executor().advance_clock(ESPERA_DA_GRAVACAO * 2);
        cx.run_until_parked();
    }

    #[gpui::test]
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
    /// shader aplicava a receita duas vezes, em 640px: a sépia salva ontem
    /// aparecia com os sliders no neutro, e "sincronizar" a partir dela mandava
    /// o neutro às outras. A miniatura fica só como espera na tela; a origem
    /// chega pela cópia de trabalho (`receber_pixels`).
    #[gpui::test]
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

    /// 🚨 **A segunda abertura da foto do site não pede pixels de novo.**
    ///
    /// A cópia de trabalho que a raiz baixou fica em `trabalho:<id>` — chave
    /// separada da miniatura da galeria, que continua sendo outra imagem. Sem
    /// isso, cada seta era um download: foi o *"voltou a ficar lento"* de
    /// 8/set, um dia depois de o cache ser desligado para esta foto.
    #[gpui::test]
    fn a_copia_de_trabalho_do_site_fica_no_cache_e_a_volta_nao_baixa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        // A imagem da galeria, na chave da grade: é a revelada, e não serve de
        // origem — se ela vazar para o shader, a receita entra duas vezes.
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
    #[gpui::test]
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
        janela: &gpui::WindowHandle<Revelacao>,
        controle: usize,
        valor: f32,
    ) {
        janela
            .update(cx, |tela, _window, cx| {
                let estado = tela.controles[controle].estado.clone();
                estado.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(
                        gpui_component::slider::SliderValue::Single(valor),
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
    #[gpui::test]
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

    /// 🚨 Abrir uma foto já revelada traz a revelação dela.
    ///
    /// Sem isto, toda foto abre no neutro — inclusive as que o fotógrafo já
    /// trabalhou. Não é "faltou uma tela": é o trabalho dele sumindo da vista,
    /// com o arquivo cru na frente. E o painel diria a mesma mentira, com os 42
    /// sliders parados no meio.
    #[gpui::test]
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
                    "a foto na tela ainda é a original; sem este pedido ela nunca vira a revelada"
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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

    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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

    /// A importação do Lightroom, de ponta a ponta: escolher, traduzir, salvar.
    ///
    /// 🔑 **As novas entram na lista da tela na hora.** Elas já vão ao banco
    /// pela porta, mas quem acabou de importar quer aplicá-las agora — esperar a
    /// próxima abertura do app é o mesmo que não ter importado.
    #[gpui::test]
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

    /// ⚠️ **Nome repetido é pulado, e não sobrescrito.**
    ///
    /// Reimportar a mesma pasta é gesto comum — e sobrescrever apagaria o ajuste
    /// que o fotógrafo fez em cima da predefinição depois de importá-la.
    #[gpui::test]
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
    #[gpui::test]
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

    #[gpui::test]
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
    #[gpui::test]
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

    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
                assert_eq!(*tela.marcadas(), BTreeSet::from([3]), "fora do lote recomeça");
            })
            .expect("a janela deve estar aberta");
    }

    /// A raiz gravou nas marcadas; as cópias da tira têm de dizer o mesmo.
    #[gpui::test]
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
    #[gpui::test]
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
                assert_eq!(
                    tela.controles[2].estado.read(cx).value().start(),
                    5.0,
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
                assert_eq!(tela.ajustes_na_tela(), na_previa, "o clique dá a mesma foto");
            })
            .expect("a janela deve estar aberta");
    }

    /// O formulário embutido: o `+` abre e fecha, e salvar fecha.
    ///
    /// 🚨 **Sem nada fora do neutro não salva** — a menos que a caixa "zerar os
    /// outros" esteja marcada. O diálogo antigo tinha o OK sempre ligado e
    /// gravava uma predefinição vazia.
    #[gpui::test]
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
                assert!(tela.predefinicoes.criando);
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.set_value("Nada", window, cx));

                tela.salvar_preset(window, cx);
                assert!(tela.presets.is_empty(), "nada fora do neutro");
                assert!(tela.predefinicoes.criando, "o formulário continua aberto");

                // Com a caixa marcada, guarda todos — e aplicar devolve ao original.
                tela.alternar_preset_inteiro(cx);
                tela.salvar_preset(window, cx);
                assert_eq!(tela.presets.len(), 1);
                assert_eq!(
                    tela.presets[0].adjustments.len(),
                    crate::revelacao::processador::Ajustes::NOMES.len()
                );
                assert!(!tela.predefinicoes.criando, "salvar fecha");
                assert!(!tela.preset_inteiro, "a caixa volta desmarcada");
                assert_eq!(tela.nome_do_preset.read(cx).value().as_ref(), "");

                tela.alternar_formulario_de_preset(window, cx);
                tela.alternar_formulario_de_preset(window, cx);
                assert!(!tela.predefinicoes.criando, "o + fecha o que abriu");
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.salvos().len(), 1);
    }

    /// 🚨 **Apagar pergunta antes** — "Apagar "X"? A predefinição sai da
    /// lista." —, e só "Apagar" apaga. É o único gesto da coluna que não se
    /// desfaz.
    #[gpui::test]
    fn apagar_pergunta_e_so_o_sim_apaga(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let minha = Preset::user("Retrato".into(), PresetAdjustments::vazia().com("exposure", 1.0));
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
                    tela.predefinicoes.pergunta,
                    Some((id, "Retrato".to_string()))
                );
                tela.responder_pergunta(false, cx);
                assert!(tela.predefinicoes.pergunta.is_none());
                assert_eq!(tela.presets.len(), 1, "cancelar não apaga");

                tela.pedir_para_apagar(id, window, cx);
                tela.responder_pergunta(true, cx);
                assert!(tela.presets.is_empty());
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.apagados(), vec![id]);
    }

    /// O aviso de "entrou nas predefinições" aparece no canto e some sozinho.
    #[gpui::test]
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
                    .update(cx, |estado, cx| estado.set_value("Neutro", window, cx));
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
    #[gpui::test]
    fn renomear_no_lugar_comeca_com_o_nome_e_fecha_ao_confirmar(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let minha = Preset::user("Retrato".into(), PresetAdjustments::vazia().com("exposure", 1.0));
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
                assert_eq!(tela.predefinicoes.renomeando, Some(id));
                assert_eq!(tela.renome_do_preset.read(cx).value().as_ref(), "Retrato");

                // Em branco não vale, e o campo fica.
                tela.renomear_preset(id, "  ".into(), cx);
                assert_eq!(tela.predefinicoes.renomeando, Some(id));

                tela.renomear_preset(id, "Retrato claro".into(), cx);
                assert_eq!(tela.predefinicoes.renomeando, None);
                assert_eq!(tela.presets[0].name, "Retrato claro");

                tela.comecar_a_renomear(id, window, cx);
                tela.cancelar_renome(cx);
                assert_eq!(tela.predefinicoes.renomeando, None);
                assert_eq!(tela.presets[0].name, "Retrato claro", "cancelar não muda");
            })
            .expect("a janela deve estar aberta");
        assert_eq!(guarda.renomeados(), vec![(id, "Retrato claro".into())]);
    }

    /// Reordenar: ↑ ↓ andam uma posição, soltar põe antes ou depois, e "ordem
    /// padrão" desfaz. Com a busca ativa, nada se move.
    #[gpui::test]
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
                .0
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(nomes(tela, cx)[0], "Preto e branco clássico");

                tela.deslocar_preset(Grupo::Sistema, "sistema:sepia", -1, cx);
                assert_eq!(nomes(tela, cx)[..2], ["Sépia à moda antiga", "Preto e branco clássico"]);

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
                    .update(cx, |campo, cx| campo.set_value("a", window, cx));
                let antes = nomes(tela, cx);
                tela.deslocar_preset(Grupo::Sistema, "sistema:recordarfotos-pb", 1, cx);
                assert_eq!(nomes(tela, cx), antes);
                tela.busca_de_presets
                    .update(cx, |campo, cx| campo.set_value("", window, cx));

                tela.definir_ordem_dos_presets(Grupo::Sistema, None, cx);
                assert_eq!(nomes(tela, cx)[0], "Preto e branco clássico");
            })
            .expect("a janela deve estar aberta");
    }

    /// Sem foto pronta (ou com a revelação travada no site) a coluna trava: o
    /// `+`, a prévia, o aplicar e o reordenar — o `desabilitado` do site. A
    /// levada no balcão continua revelável.
    #[gpui::test]
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
    #[gpui::test]
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
                    estado.set_value("Retrato claro", window, cx)
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
        assert_eq!(salvos[0].adjustments.len(), 1);
        assert_eq!(salvos[0].adjustments.get("contrast"), None);
    }

    /// 🚨 **O id que vai para o banco é o mesmo que fica na lista da tela.**
    ///
    /// Era o contrário: a tela punha na lista um `Preset::user` com id próprio,
    /// e o use case criava outro ao gravar. Enquanto salvar era o único gesto,
    /// ninguém notava — com renomear e apagar, o comando ia para um id que a
    /// tabela não tem, a linha sumia da tela e voltava na abertura seguinte.
    #[gpui::test]
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
                    .update(cx, |estado, cx| estado.set_value("Claro", window, cx));
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
    #[gpui::test]
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
                    .update(cx, |estado, cx| estado.set_value("   ", window, cx));
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
    #[gpui::test]
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
    #[gpui::test]
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
        assert_eq!(gravado.len(), 1, "girar gravou uma vez, sair não grava de novo");
        assert_eq!(gravado[0].2.rotacao, Some(1));
    }

    /// 🚨 Arrastar uma alça grava ao soltar — porque nada mais vai gravar por ele.
    ///
    /// O palco tem 80 px para uma foto de 8: cada 10 px de arrasto é um pixel
    /// da foto.
    #[gpui::test]
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
                tela.palco = Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(80.), px(80.)));
                tela.alternar_corte(window, cx);

                tela.comecar_arrasto(Some(corte::Alca::Esquerda), gpui::point(px(0.), px(40.)), cx);
                tela.mover_no_corte(gpui::point(px(20.), px(40.)), window, cx);
                assert!(gravador.gravado().is_empty(), "no meio do arrasto não grava");
                tela.soltar_no_corte(cx);

                assert!(tela.cortando(), "soltar não fecha a ferramenta");
                assert_eq!(tela.corte.x, Some(0.25));
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert_eq!(gravado[0].2.x, Some(0.25), "o corte novo foi para o banco");
        assert_eq!(gravado[0].2.largura, Some(0.75));
    }

    /// 🚨 O endireitar encolhe o retângulo para caber, e cresce de volta.
    #[gpui::test]
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
                assert!(torto.crop_width() < 1., "encolheu para não mostrar canto vazio");

                tela.definir_angulo(0., cx);
                let reto = tela.corte_atual();
                assert!((reto.crop_width() - 1.).abs() < 1e-3, "cresceu de volta");
                assert!((reto.crop_height() - 1.).abs() < 1e-3);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A foto comprada no site não se revela: nada dela vai para o banco,
    /// e o Enquadrar não abre pelo botão.
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
                tela.palco = Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(160.), px(80.)));
                tela.alternar_corte(window, cx);
                tela.travar_proporcao(Some(1.), cx);

                let quadrado = |tela: &Revelacao| {
                    let c = tela.corte_atual();
                    (c.crop_width() * 160., c.crop_height() * 80.)
                };
                let (w, h) = quadrado(tela);
                assert!((w - h).abs() <= 1., "1:1 na hora: {w} × {h}");

                // Encolhe pela esquerda: a altura acompanha a largura.
                tela.comecar_arrasto(Some(corte::Alca::Esquerda), gpui::point(px(40.), px(40.)), cx);
                tela.mover_no_corte(gpui::point(px(60.), px(40.)), window, cx);
                tela.soltar_no_corte(cx);
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
    #[gpui::test]
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

        // Exposição +1 dobra o valor: o balde 100 esvazia e o 200 enche.
        arrastar(cx, &janela, 0, 1.0);
        cx.run_until_parked();

        // O resultado vem da GPU por um laço assíncrono; espera ele chegar.
        for _ in 0..200 {
            let pronto = janela
                .update(cx, |tela, _window, _cx| {
                    tela.histograma
                        .as_ref()
                        .is_some_and(|h| h.vermelho[200] > 0)
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
        panic!("o histograma não acompanhou o ajuste");
    }

    /// Sem foto não há histograma — e não há divisão por zero em lugar nenhum.
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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
    #[gpui::test]
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

    /// 🚨 **Foto comprada não se zera** — o `podeRevelar` do site. E uma foto
    /// só enquadrada conta como "fora do neutro" para o botão.
    #[gpui::test]
    fn foto_comprada_nao_se_zera_e_a_so_enquadrada_conta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-comprada.jpg", "id-girada.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar preview");
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
    #[gpui::test]
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
    #[gpui::test]
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
}
