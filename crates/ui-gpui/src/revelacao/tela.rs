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

use std::collections::{BTreeSet, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::selecao::Modificadores;
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Preset, PresetId};
use domain::value_objects::{AspectRatio, CropSettings};
use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, Bounds, Context, Entity, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, SharedString,
    Subscription, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::collapsible::Collapsible;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable, WindowExt};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::miniaturas::{CacheDeMiniaturas, Miniatura};
use crate::imagem::para_gpui;
use crate::tema;

use super::automatico;
use super::controles::{Definicao, Painel, Secao, CONTROLES};
use super::corte::{self, Alca};
use super::curva;
use super::histograma::Histograma;
use super::historico::{Estado, Historico};
use super::lightroom::{self, Arquivo, EscolhaDePresets, Relatorio};
use super::persistencia::{self, Corte, Gravador};
use super::presets::{self, GuardaDePresets};
use super::processador::{Ajustes, Pedido, Processador};
use super::sincronizacao::{self, Escolha, Grupo};
use infrastructure::transformacao;

/// Largura da coluna de ajustes — os `w-80` do site.
const LADO_DO_PAINEL: f32 = 320.0;

/// A coluna das predefinições, à esquerda — os `w-56` do site.
const LADO_DOS_PRESETS: f32 = 224.0;

/// A altura da barra do topo — os `h-12` do site.
const ALTURA_DO_CABECALHO: f32 = 48.0;

/// A altura dos dois gráficos, no topo da coluna da direita.
///
/// ⚠️ **O site não tem histograma nenhum**, e este ficou. Não é divergência por
/// esquecimento: é o gráfico que responde "estourou o branco?" — a pergunta que
/// nenhum slider responde —, ele existe no Lightroom, e tirá-lo para igualar
/// seria apagar trabalho que funciona. Fica onde o Lightroom o põe: no alto da
/// coluna da direita, acima dos painéis.
const ALTURA_DOS_GRAFICOS: f32 = 200.0;

/// A altura da faixa de miniaturas, embaixo do palco.
///
/// ⚠️ **Ela é fixa desde que o dock saiu.** Antes a divisória do dock a
/// redimensionava; no site a tira tem altura própria (e um puxador que ainda não
/// existe aqui).
const ALTURA_DO_FILMSTRIP: f32 = 84.0;

/// Quantas miniaturas da tira ficam em memória.
///
/// A tira mostra o acervo **inteiro**, mas quem paga por quadro é só o que está
/// à vista; este é o teto do que fica guardado. Uma miniatura de 320px em BGRA
/// ocupa ~300 KB, então 512 são ~150 MB no pior caso — e o pior caso é ter
/// rolado a tira inteira de um acervo grande.
const MINIATURAS_DA_TIRA: usize = 512;

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

/// As proporções que o legado oferece, na ordem do combo dele
/// (`crop_panel.rs`).
///
/// ⚠️ **Não é a lista inteira do `AspectRatio`.** O enum tem 13 variantes; o combo
/// do legado mostra estas. Acrescentar uma aqui seria feature nova — e o corte
/// travado numa proporção que o outro app não tem viraria diferença de pixel sem
/// explicação na conferência da fase 5.
const PROPORCOES: [(&str, AspectRatio); 8] = [
    ("Livre", AspectRatio::Free),
    ("Original", AspectRatio::Original),
    ("1:1", AspectRatio::Square),
    ("4:3", AspectRatio::FourThree),
    ("3:4", AspectRatio::ThreeFour),
    ("5:4", AspectRatio::FiveFour),
    ("16:9", AspectRatio::SixteenNine),
    ("9:16", AspectRatio::NineSixteen),
];

/// Até onde o slider de endireitamento vai, em graus. É o limite que o
/// `CropSettings` impõe (`MAX_ANGLE`), repetido aqui porque a barra precisa dele
/// para desenhar — e o teste `todo_neutro_cabe_na_faixa` do painel de ajustes já
/// mostrou o que acontece quando faixa e valor discordam.
const ANGULO_MAXIMO: f32 = 45.0;

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
    previa: Option<PresetAdjustments>,
    /// Se a próxima predefinição salva guarda os 53 (e não só o que saiu do
    /// neutro) — a caixa "Zerar os outros ajustes ao aplicar" do site.
    preset_inteiro: bool,
    /// Quais painéis estão abertos. Um conjunto, e não um `bool` por painel:
    /// acrescentar painel novo não pode exigir lembrar de acrescentar campo.
    abertos: HashSet<Painel>,
    /// Qual das três famílias do HSL está à mostra.
    ///
    /// 🔑 **Uma aba, e não três painéis** — o desenho do site e o do Lightroom.
    /// Guardar a escolha (em vez de voltar a "Cor" a cada abertura) é o que
    /// permite passar trinta fotos mexendo só na luminância.
    aba_hsl: Secao,
    /// O id do pedido que ainda não voltou. `None` é "a tela está em dia".
    aguardando: Option<u64>,
    /// Se já existe um laço de colheita rodando. Sem esta trava, cada arrasto
    /// abriria um laço novo e a tela acabaria com dezenas deles perguntando a
    /// mesma coisa.
    colhendo: bool,
    _assinaturas: Vec<Subscription>,
}

struct Controle {
    definicao: &'static Definicao,
    estado: Entity<SliderState>,
}

/// O modo de corte, enquanto ele está aberto.
struct Edicao {
    /// O corte sendo editado — uma **cópia**. O corte da foto só é substituído em
    /// "Aplicar": sem isso, cancelar não teria o que restaurar.
    corte: CropSettings,
    /// A grade de terços. Desligada por padrão, como no legado
    /// (`show_composition_grid` nasce `false`).
    grade: bool,
    /// O que o ponteiro está movendo, e onde ele estava no quadro anterior.
    arrasto: Option<(Arrasto, Point<Pixels>)>,
    /// A proporção travada. `Free` é o padrão do legado — corte livre até alguém
    /// escolher outra coisa.
    proporcao: AspectRatio,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Arrasto {
    Alca(Alca),
    Retangulo,
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
                .default_value(0.0)
        });
        assinaturas.push(cx.subscribe_in(
            &angulo,
            window,
            move |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                let SliderEvent::Change(valor) = evento;
                tela.definir_angulo(valor.start(), cx);
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
            histograma: None,
            angulo,
            palco: Bounds::default(),
            guarda_de_presets,
            nome_do_preset: cx.new(|cx| InputState::new(window, cx).placeholder("Nome do preset")),
            presets,
            busca_de_presets,
            escolha_de_presets,
            arquivos: std::sync::mpsc::channel(),
            escolhendo_arquivos: false,
            relatorio: None,
            renome_do_preset: cx.new(|cx| InputState::new(window, cx)),
            previa: None,
            preset_inteiro: false,
            abertos: Painel::TODOS
                .into_iter()
                .filter(Painel::nasce_aberto)
                .collect(),
            aba_hsl: Secao::HslCor,
            aguardando: None,
            colhendo: false,
            _assinaturas: assinaturas,
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
        self.mostrar_a_posicao(window, cx);
    }

    /// Um passo na lista, sem dar a volta — a mesma regra das setas da grade.
    ///
    /// ⚠️ **Trocar de foto aqui grava a anterior**, pelo caminho de sempre: é a
    /// primeira coisa que `mostrar_a_posicao` faz. Sem isso, andar meio segundo
    /// depois de mexer num slider deixaria a gravação atrasada sair com os
    /// ajustes já substituídos.
    pub fn andar(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        if self.acervo.is_empty() {
            return;
        }
        let ultima = self.acervo.len() - 1;
        let nova = if passo > 0 {
            (self.posicao + 1).min(ultima)
        } else {
            self.posicao.saturating_sub(1)
        };

        if nova != self.posicao {
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

    /// O clique na tira: sozinho troca de foto, com Ctrl marca, com Shift marca
    /// a faixa — como no Lightroom e no site.
    ///
    /// ⚠️ **Ctrl e Shift não trocam a foto aberta.** Trocar grava a anterior e
    /// recomeça o lote; montar um lote de dez fotos trocando dez vezes deixaria
    /// o operador sempre com uma só marcada. Quem manda no canvas é o clique
    /// simples.
    pub fn clicar_na_tira(
        &mut self,
        posicao: usize,
        modificadores: Modificadores,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if posicao >= self.acervo.len() {
            return;
        }
        if modificadores.aditivo {
            sincronizacao::alternar(&mut self.marcadas, self.posicao, posicao);
            cx.notify();
        } else if modificadores.faixa {
            self.marcadas = sincronizacao::faixa(self.posicao, posicao);
            cx.notify();
        } else {
            self.ir_para(posicao, window, cx);
        }
    }

    /// `Cmd+A`: a tira inteira.
    pub fn marcar_todas(&mut self, cx: &mut Context<Self>) {
        if self.acervo.is_empty() {
            return;
        }
        self.marcadas = sincronizacao::todas(self.acervo.len());
        cx.notify();
    }

    /// `Cmd+D`: só a aberta.
    pub fn desmarcar(&mut self, cx: &mut Context<Self>) {
        if self.acervo.is_empty() {
            return;
        }
        self.marcadas = sincronizacao::so(self.posicao);
        cx.notify();
    }

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
        posicoes
            .into_iter()
            .filter_map(|p| self.acervo.get(p).cloned())
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
        // Trocar de foto recomeça o lote: herdar o anterior sincronizaria fotos
        // que o operador já tinha esquecido de ter marcado.
        self.marcadas = sincronizacao::so(self.posicao);
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
            .map(|foto| foto.id.clone())
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

        // Preview primeiro, miniatura como queda. A miniatura fica borrada numa
        // tela inteira, e é de propósito: mostrar a foto em tamanho errado é
        // melhor do que mostrar retângulo vazio.
        let bruta = self
            .previews
            .get_preview(&foto.id)
            .or_else(|| self.previews.get_thumbnail(&foto.id));

        // 🚨 **A foto do site não tem bruto neste cache.** O que a grade da
        // sessão guardou em `site:<id>` é a **miniatura da galeria** — que,
        // depois de "Salvar na galeria", é a foto **revelada**. Servir isso ao
        // shader aplicaria a receita duas vezes, em 640px, e era o que a tela
        // fazia: a sépia salva ontem aparecia com os sliders no neutro, e
        // "sincronizar" a partir dela mandava o neutro às outras. A miniatura
        // fica só como **espera** na tela; a origem é a cópia de trabalho, que
        // a raiz busca ao receber `AbriuOutraFoto`.
        let origem = if persistencia::so_existe_no_site(&foto) {
            None
        } else {
            bruta.as_ref().map(|imagem| {
                let rgba = imagem.to_rgba8();
                Origem {
                    largura: rgba.width(),
                    altura: rgba.height(),
                    pixels: Arc::new(rgba.into_raw()),
                }
            })
        };

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
        self.aguardando = None;

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
        aberta.origem = Some(Origem {
            largura: rgba.width(),
            altura: rgba.height(),
            pixels: Arc::new(rgba.into_raw()),
        });
        aberta.bruta = Some(imagem.clone());
        aberta.revelada = Some(imagem);
        aberta.desenhada = None;

        self.atualizar_exibicao();
        self.pedir_revelacao(cx);
        cx.notify();
        true
    }

    /// A foto aberta, para quem precisa saber de onde buscar os pixels.
    pub fn foto_aberta(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
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
    fn gravar(&self) {
        let Some(aberta) = self.aberta.as_ref() else {
            return;
        };
        self.gravador
            .gravar(aberta.foto.id.clone(), self.ajustes, self.corte);
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
        // 🚨 O modo de corte fecha ao desfazer. O retângulo na tela é o de antes
        // do passo que acabou de sair; deixá-lo aberto faria o botão "Aplicar"
        // reintroduzir, no clique seguinte, o enquadramento que o `Cmd+Z` tirou.
        self.edicao = None;
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao(cx);
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

    /// O que a GPU desenha: os ajustes de verdade, ou eles com a predefinição
    /// sob o ponteiro por cima.
    ///
    /// 🔑 **A prévia não entra em `self.ajustes`**, e é o que a mantém
    /// reversível de graça: nada precisa ser guardado para desfazê-la, e um
    /// travamento com o ponteiro em cima de uma predefinição não deixa a foto
    /// alterada.
    fn ajustes_na_tela(&self) -> Ajustes {
        match &self.previa {
            Some(preset) => {
                let mut ajustes = self.ajustes;
                presets::aplicar(&mut ajustes, preset);
                ajustes
            }
            None => self.ajustes,
        }
    }

    /// Mostra (ou tira) a prévia de uma predefinição.
    ///
    /// Sem foto aberta não há o que prever, e sem mudança não há o que
    /// redesenhar — pedir à GPU o mesmo quadro a cada movimento do ponteiro
    /// sobre a mesma linha seria trabalho por nada.
    pub fn prever(&mut self, preset: Option<PresetAdjustments>, cx: &mut Context<Self>) {
        if self.previa == preset {
            return;
        }
        self.previa = preset;
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// A predefinição que está sendo prevista, para os testes.
    #[cfg(test)]
    pub fn previa(&self) -> Option<&PresetAdjustments> {
        self.previa.as_ref()
    }

    /// Aplica um preset: 15 dos 46 campos, de uma vez.
    ///
    /// É um gesto discreto, como o `Cmd+Z` — vira passo de histórico e vai para o
    /// banco **na hora**, sem passar pela espera de 500 ms, que existe para juntar
    /// os eventos de um arrasto.
    pub fn aplicar_preset(&mut self, preset: &Preset, window: &mut Window, cx: &mut Context<Self>) {
        // O que estiver a meio caminho fecha primeiro, pelo mesmo motivo do
        // desfazer: senão a espera pendente grava por cima do preset.
        self.gravar_o_que_estiver_pendente();

        presets::aplicar(&mut self.ajustes, &preset.adjustments);
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao(cx);
        self.historico.registrar(self.estado());
        self.gravar();
        cx.notify();
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

    /// Guarda os ajustes de agora como preset do usuário.
    ///
    /// 🚨 **Nome vazio não salva.** O legado aceita — o diálogo dele grava o que
    /// estiver no campo — e o resultado é uma linha sem rótulo na lista, que não
    /// dá para distinguir nem para apagar (apagar preset não existe em nenhum dos
    /// dois).
    ///
    /// ⚠️ **O preset que aparece na lista tem id local.** O `SavePresetUseCase`
    /// cria o `Preset` lá dentro, com id próprio, e a porta é `fire-and-forget`
    /// como a de gravação — então o que se vê até fechar o app é um gêmeo com
    /// outro id. Nada depende do id hoje (apagar preset não foi portado, e o
    /// legado também não o tem), mas é a primeira coisa a consertar quando
    /// depender.
    pub fn salvar_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nome = self.nome_do_preset.read(cx).value().trim().to_string();
        if nome.is_empty() {
            return;
        }

        // 🔑 **O `Preset` nasce aqui, com o id que vai para os dois lados.** A
        // lista da tela e a tabela do banco passam a falar da mesma linha — sem
        // isso, renomear ou apagar o que acabou de ser salvo manda o comando
        // para um id que a tabela não tem.
        let novo = Preset::user(
            nome,
            presets::dos_ajustes(&self.ajustes, self.preset_inteiro),
        );
        self.guarda_de_presets.salvar(novo.clone());
        self.presets.push(novo);

        self.nome_do_preset
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        cx.notify();
    }

    /// Entra ou sai do modo de corte. É o `R` do legado.
    ///
    /// ⚠️ Sair por aqui **descarta** o que estava sendo cortado, como o `R` de
    /// lá: quem aplica usa o botão. Sem essa distinção, uma tecla teria dois
    /// significados conforme o estado, e nenhum aviso de qual valeu.
    /// Abre o seletor do sistema para importar predefinições do Lightroom.
    ///
    /// 🚨 **O laço de colheita sobe antes da resposta**, e não depois: o seletor
    /// é uma janela do sistema e pode voltar a qualquer momento. Sem isto, os
    /// arquivos escolhidos ficariam parados no canal até alguma outra coisa
    /// acordar a tela — um arrasto de slider, por acaso.
    pub fn importar_do_lightroom(&mut self, cx: &mut Context<Self>) {
        if self.escolhendo_arquivos {
            return;
        }
        self.escolhendo_arquivos = true;
        self.relatorio = None;
        self.escolha_de_presets.escolher(self.arquivos.0.clone());
        self.esperar_arquivos(cx);
        cx.notify();
    }

    /// Acorda a tela de tempos em tempos enquanto o seletor está aberto.
    fn esperar_arquivos(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |tela, cx| {
            loop {
                // ⚠️ Uma espera bem mais longa que a da GPU: aqui do outro lado
                // há uma pessoa procurando arquivo numa janela do sistema, e
                // acordar a 8 ms para descobrir que ela ainda não escolheu é
                // gastar quadro por nada.
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let continua = tela
                    .update(cx, |tela, cx| tela.colher_arquivos(cx))
                    .unwrap_or(false);
                if !continua {
                    break;
                }
            }
        })
        .detach();
    }

    /// Drena o que o seletor mandou. Devolve se vale continuar acordando.
    fn colher_arquivos(&mut self, cx: &mut Context<Self>) -> bool {
        let mut chegou = false;
        while let Ok(arquivos) = self.arquivos.1.try_recv() {
            chegou = true;
            self.importar(arquivos, cx);
        }
        if chegou {
            self.escolhendo_arquivos = false;
            cx.notify();
        }
        self.escolhendo_arquivos
    }

    /// Traduz o que foi lido e guarda o que virou predefinição.
    ///
    /// ⚠️ **A lista da tela recebe as novas na hora.** Elas já estão no banco
    /// pela porta, mas quem acabou de importar quer aplicá-las agora — esperar a
    /// próxima abertura do app é o mesmo que não ter importado.
    pub fn importar(&mut self, arquivos: Vec<Arquivo>, cx: &mut Context<Self>) {
        let nomes: Vec<String> = self.presets.iter().map(|p| p.name.clone()).collect();
        let (novas, relatorio) = lightroom::preparar(&arquivos, &nomes);

        for traduzida in novas {
            let preset = Preset::user(traduzida.nome, traduzida.ajustes);
            self.guarda_de_presets.salvar(preset.clone());
            self.presets.push(preset);
        }

        // Nada escolhido não é resultado: quem desiste do seletor não precisa
        // ler "0 arquivos lidos".
        self.relatorio = (relatorio.arquivos > 0).then_some(relatorio);
        cx.notify();
    }

    /// Fecha o resultado da última importação.
    pub fn fechar_relatorio(&mut self, cx: &mut Context<Self>) {
        self.relatorio = None;
        cx.notify();
    }

    /// Troca o nome de uma predefinição do fotógrafo.
    ///
    /// 🔑 **A lista da tela muda junto com o banco**, e não só depois de
    /// reabrir o app: `self.presets` é o que a coluna desenha, e deixá-la
    /// desatualizada faria o nome antigo continuar ali até a próxima abertura —
    /// com o operador renomeando de novo, achando que o primeiro não pegou.
    pub fn renomear_preset(&mut self, id: PresetId, nome: String, cx: &mut Context<Self>) {
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            return;
        }

        let Some(preset) = self
            .presets
            .iter_mut()
            .find(|preset| preset.id == id && !preset.is_system)
        else {
            return;
        };
        preset.name = nome.clone();

        self.guarda_de_presets.renomear(id, nome);
        cx.notify();
    }

    /// Apaga uma predefinição do fotógrafo.
    ///
    /// ⚠️ **As de sistema não se apagam** — elas nascem em código a cada
    /// listagem, e apagar mandaria um `DELETE` para um id que a tabela não tem:
    /// a linha sumiria da tela e voltaria na abertura seguinte.
    pub fn apagar_preset(&mut self, id: PresetId, cx: &mut Context<Self>) {
        if !self
            .presets
            .iter()
            .any(|preset| preset.id == id && !preset.is_system)
        {
            return;
        }

        self.presets.retain(|preset| preset.id != id);
        // A prévia pode ser justamente a que sumiu — deixá-la faria a foto
        // continuar mostrando uma predefinição que não existe mais.
        self.prever(None, cx);
        self.guarda_de_presets.apagar(id);
        cx.notify();
    }

    pub fn alternar_corte(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edicao.is_some() {
            self.edicao = None;
        } else {
            // Sem foto (ou sem pixels no cache) não há o que cortar, e abrir o
            // overlay sobre o vazio daria alças flutuando em lugar nenhum.
            let Some(Aberta {
                origem: Some(_), ..
            }) = self.aberta.as_ref()
            else {
                return;
            };
            let corte = self.corte_atual();
            // 🚨 O slider tem de nascer no ângulo da foto, e não em zero: numa
            // foto já endireitada, uma barra no meio diria que ela está reta — e o
            // primeiro toque nela desfaria o endireitamento sem aviso.
            //
            // `set_value` não emite `Change`, então isto não vira um pedido de
            // reprocessamento (a mesma assimetria de `abrir`).
            let graus = corte.angle();
            self.angulo
                .update(cx, |estado, cx| estado.set_value(graus, window, cx));

            self.edicao = Some(Edicao {
                corte,
                grade: false,
                arrasto: None,
                proporcao: AspectRatio::Free,
            });
        }
        self.atualizar_exibicao();
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

    /// Confirma o corte: ele vira o corte da foto e vai para o banco.
    ///
    /// 🚨 **Gravar aqui não é opcional.** O corte não faz parte de `Ajustes`,
    /// então nenhum slider vai levá-lo ao banco depois — sem esta gravação, o
    /// enquadramento só existiria até fechar a tela.
    pub fn aplicar_corte(&mut self, cx: &mut Context<Self>) {
        let Some(edicao) = self.edicao.take() else {
            return;
        };

        let novo = Corte {
            x: Some(edicao.corte.crop_x()),
            y: Some(edicao.corte.crop_y()),
            largura: Some(edicao.corte.crop_width()),
            altura: Some(edicao.corte.crop_height()),
            rotacao: Some(edicao.corte.rotation_90()),
            angulo: Some(edicao.corte.angle()),
            espelho_h: Some(edicao.corte.flip_horizontal()),
            espelho_v: Some(edicao.corte.flip_vertical()),
        };

        // 🚨 A ordem aqui é o item 12 inteiro. O gesto a meio caminho fecha
        // **antes** da troca, para virar um passo com o corte antigo; só então o
        // corte novo entra e vira o passo seguinte. Fechar depois colaria o
        // enquadramento novo num ajuste velho, e o `Cmd+Z` pularia por cima do
        // corte sem nunca o desfazer.
        self.fechar_o_gesto_pendente();
        self.corte = novo;
        self.historico.registrar(self.estado());
        self.gravar();
        self.atualizar_exibicao();
        cx.notify();
    }

    /// Sai sem aplicar. O corte da foto continua o que era.
    pub fn cancelar_corte(&mut self, cx: &mut Context<Self>) {
        self.edicao = None;
        self.atualizar_exibicao();
        cx.notify();
    }

    pub fn cortando(&self) -> bool {
        self.edicao.is_some()
    }

    /// Onde a foto está desenhada dentro do palco, em pixels.
    ///
    /// `None` quando não há foto, quando o palco ainda não foi medido (primeiro
    /// quadro) ou quando a foto não tem pixels — nos três casos não há onde pôr
    /// overlay nenhum.
    fn area_da_foto(&self) -> Option<(f32, f32, f32, f32)> {
        let Some(Aberta {
            origem: Some(origem),
            ..
        }) = self.aberta.as_ref()
        else {
            return None;
        };

        let palco = (
            f32::from(self.palco.size.width),
            f32::from(self.palco.size.height),
        );
        let area = corte::area_da_foto(palco, (origem.largura as f32, origem.altura as f32));
        (area.2 > 0.0 && area.3 > 0.0).then_some(area)
    }

    /// Aplica o movimento do ponteiro ao corte em edição.
    fn mover_corte(&mut self, ponteiro: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((area, tamanho)) = self.area_da_foto().zip(self.tamanho_da_foto()) else {
            return;
        };
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        let Some((arrasto, anterior)) = edicao.arrasto else {
            return;
        };

        // 🔑 O delta é convertido em **fração da foto exibida**, e não em pixels:
        // é o que faz o mesmo arrasto dar o mesmo corte numa janela grande e numa
        // pequena.
        let dx = f32::from(ponteiro.x - anterior.x) / area.2;
        let dy = f32::from(ponteiro.y - anterior.y) / area.3;

        edicao.corte = match arrasto {
            Arrasto::Alca(alca) => corte::mover_alca(&edicao.corte, alca, dx, dy, tamanho, None),
            Arrasto::Retangulo => corte::arrastar(&edicao.corte, dx, dy),
        };
        edicao.arrasto = Some((arrasto, ponteiro));
        cx.notify();
    }

    fn tamanho_da_foto(&self) -> Option<(f32, f32)> {
        match self.aberta.as_ref() {
            Some(Aberta {
                origem: Some(origem),
                ..
            }) => Some((origem.largura as f32, origem.altura as f32)),
            _ => None,
        }
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
        let corte = match self.edicao.as_ref() {
            Some(edicao) => edicao.corte.clone(),
            None => self.corte_atual(),
        };
        let recortar = self.edicao.is_none();

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
        aberta.desenhada = exibida.map(para_gpui);
    }

    /// Gira 90° no sentido horário. Só faz sentido dentro do modo de corte.
    pub fn girar(&mut self, cx: &mut Context<Self>) {
        self.mexer_no_corte(corte::girar, cx);
    }

    pub fn espelhar_horizontal(&mut self, cx: &mut Context<Self>) {
        self.mexer_no_corte(corte::espelhar_horizontal, cx);
    }

    pub fn espelhar_vertical(&mut self, cx: &mut Context<Self>) {
        self.mexer_no_corte(corte::espelhar_vertical, cx);
    }

    /// Muda o ângulo de endireitamento, em graus, a partir do que já está lá.
    pub fn inclinar(&mut self, graus: f32, cx: &mut Context<Self>) {
        self.mexer_no_corte(|corte| corte::inclinar(corte, graus), cx);
    }

    /// Põe o ângulo num valor absoluto — é o que o slider manda.
    fn definir_angulo(&mut self, graus: f32, cx: &mut Context<Self>) {
        self.mexer_no_corte(|corte| corte::inclinar(corte, graus - corte.angle()), cx);
    }

    /// Volta ao corte que ocupa a foto inteira, sem giro nem espelho.
    ///
    /// É o "Reset" do painel de corte do legado — e ele **não** aplica: quem
    /// desiste de vez usa Cancelar, quem quer recomeçar do zero continua no modo.
    pub fn recomecar_corte(&mut self, cx: &mut Context<Self>) {
        self.mexer_no_corte(|_| corte::foto_inteira(), cx);
    }

    pub fn travar_proporcao(&mut self, proporcao: AspectRatio, cx: &mut Context<Self>) {
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        edicao.proporcao = proporcao;
        cx.notify();
    }

    /// 🔑 Toda mudança de corte passa por aqui, e por isso **toda** mudança
    /// reprocessa a foto exibida. Girar sem reprocessar mudaria um número e
    /// deixaria a tela igual — o botão pareceria quebrado, e o defeito só
    /// apareceria ao aplicar.
    fn mexer_no_corte(
        &mut self,
        como: impl Fn(&CropSettings) -> CropSettings,
        cx: &mut Context<Self>,
    ) {
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        edicao.corte = como(&edicao.corte);
        self.atualizar_exibicao();
        cx.notify();
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

    /// Devolve os 46 ajustes ao neutro. É o "Reset All" do painel do legado.
    ///
    /// 🔑 **O corte não entra.** Lá o `reset_edits` também não o toca: quem quer
    /// desfazer enquadramento usa "Recomeçar", dentro do modo de corte. Misturar
    /// os dois faria um botão de cor apagar trabalho de composição.
    pub fn redefinir_ajustes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();

        self.ajustes = Ajustes::default();
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao(cx);
        self.historico.registrar(self.estado());
        self.gravar();
        cx.notify();
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
        self.pedir_revelacao(cx);
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
        });
        self.aguardando = Some(id);
        self.acompanhar(cx);
        cx.notify();
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
            .size_full()
            .min_w(px(0.))
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
                    div()
                        .relative()
                        .size_full()
                        .child(img(imagem.clone()).size_full())
                        .children(self.overlay_de_corte(cx))
                        // O `canvas` mede o palco e é onde o arrasto se liga:
                        // registrar ouvinte de mouse exige estar na fase de
                        // pintura, e um `div` comum não chega lá.
                        .child(self.medida_e_arrasto(cx)),
                )
                .into_any_element(),
            Some(Aberta { foto, .. }) => moldura
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!(
                            "{} não tem preview no cache",
                            foto.name
                        ))),
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
        let arrastando = self
            .edicao
            .as_ref()
            .is_some_and(|edicao| edicao.arrasto.is_some());

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
                    move |evento: &MouseMoveEvent, fase, _window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| tela.mover_corte(evento.position, cx));
                    }
                });

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, _window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| {
                            if let Some(edicao) = tela.edicao.as_mut() {
                                edicao.arrasto = None;
                                cx.notify();
                            }
                        });
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    /// O overlay inteiro: escurecimento, retângulo, grade e as oito alças.
    ///
    /// Tudo com `div` posicionado — o GPUI não tem pincel, e não precisa: um
    /// retângulo é um `div` absoluto com fundo, e é o layout que faz a conta.
    fn overlay_de_corte(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let edicao = self.edicao.as_ref()?;
        let (ax, ay, aw, ah) = self.area_da_foto()?;

        // O retângulo de corte em pixels de tela, a partir das frações.
        let cx0 = ax + edicao.corte.crop_x() * aw;
        let cy0 = ay + edicao.corte.crop_y() * ah;
        let cw = edicao.corte.crop_width() * aw;
        let ch = edicao.corte.crop_height() * ah;

        let escuro = gpui::rgba(0x00000078);
        let mut faixas = Vec::new();
        // Quatro faixas em volta do corte, e não um retângulo com furo: o GPUI
        // não recorta buraco, e quatro divs custam o mesmo.
        for (x, y, w, h) in [
            (ax, ay, aw, cy0 - ay),                    // acima
            (ax, cy0 + ch, aw, ay + ah - (cy0 + ch)),  // abaixo
            (ax, cy0, cx0 - ax, ch),                   // à esquerda
            (cx0 + cw, cy0, ax + aw - (cx0 + cw), ch), // à direita
        ] {
            if w > 0.0 && h > 0.0 {
                faixas.push(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(w))
                        .h(px(h))
                        .bg(escuro)
                        .into_any_element(),
                );
            }
        }

        let mut grade = Vec::new();
        if edicao.grade {
            let linha = gpui::rgba(0xffffff66);
            for i in 1..3 {
                let fracao = i as f32 / 3.0;
                grade.push(
                    div()
                        .absolute()
                        .left(px(cx0 + cw * fracao))
                        .top(px(cy0))
                        .w(px(1.))
                        .h(px(ch))
                        .bg(linha)
                        .into_any_element(),
                );
                grade.push(
                    div()
                        .absolute()
                        .left(px(cx0))
                        .top(px(cy0 + ch * fracao))
                        .w(px(cw))
                        .h(px(1.))
                        .bg(linha)
                        .into_any_element(),
                );
            }
        }

        let alcas: Vec<_> = Alca::TODAS
            .into_iter()
            .map(|alca| {
                let (fx, fy) = alca.posicao();
                // 12px de lado, centrada no ponto — o mesmo `HANDLE_SIZE` do
                // legado. Menor que isso vira alvo difícil de acertar com o dedo
                // no trackpad.
                const LADO: f32 = 12.0;
                div()
                    .id(SharedString::from(format!("alca-{alca:?}")))
                    .absolute()
                    .left(px(cx0 + cw * fx - LADO / 2.0))
                    .top(px(cy0 + ch * fy - LADO / 2.0))
                    .w(px(LADO))
                    .h(px(LADO))
                    .bg(gpui::white())
                    .border_1()
                    .border_color(gpui::black())
                    .rounded(px(2.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |tela, evento: &MouseDownEvent, _window, cx| {
                            if let Some(edicao) = tela.edicao.as_mut() {
                                edicao.arrasto = Some((Arrasto::Alca(alca), evento.position));
                                cx.notify();
                            }
                        }),
                    )
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .absolute()
                .inset_0()
                .children(faixas)
                .child(
                    // O retângulo: só a borda, para não cobrir a foto.
                    div()
                        .id("retangulo-de-corte")
                        .absolute()
                        .left(px(cx0))
                        .top(px(cy0))
                        .w(px(cw))
                        .h(px(ch))
                        .border_2()
                        .border_color(gpui::white())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|tela, evento: &MouseDownEvent, _window, cx| {
                                if let Some(edicao) = tela.edicao.as_mut() {
                                    edicao.arrasto = Some((Arrasto::Retangulo, evento.position));
                                    cx.notify();
                                }
                            }),
                        ),
                )
                .children(grade)
                .children(alcas)
                .into_any_element(),
        )
    }

    /// O painel dos ajustes: os 42 controles, o corte e o redefinir.
    ///
    /// 🔑 **Os avisos moram aqui**, e não no palco: "sem GPU" e "mostrando o
    /// original" são recados sobre o que os controles estão fazendo, e sobre a
    /// foto o palco já fala sozinho.
    /// A coluna da direita: o cabeçalho, os sete painéis e nada mais.
    ///
    /// 🔑 **No modo de enquadramento ela troca de conteúdo**, como no site
    /// (`editor.tsx`: `enquadrando ? <PainelDeCorte/> : <Paineis/>`). Antes a
    /// barra de corte entrava por cima dos 53 sliders, e quem estava cortando
    /// rolava por uma coluna inteira de controles que não tinham nada a ver com
    /// o gesto em curso.
    fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sem GPU não há revelação, e o painel diz isso em vez de oferecer
        // sliders que não movem nada. `None` é "a thread ainda está abrindo o
        // dispositivo" — não é ausência de placa, e anunciar ausência durante os
        // milissegundos de abertura seria mentir em toda abertura.
        let sem_motor = self.processador.disponivel() == Some(false);
        let enquadrando = self.edicao.is_some();

        let barra = self.barra_de_corte(cx).map(IntoElement::into_any_element);
        let cabecalho = (!enquadrando).then(|| self.cabecalho_dos_ajustes(cx));
        let paineis: Vec<AnyElement> = if enquadrando {
            Vec::new()
        } else {
            Painel::TODOS
                .into_iter()
                .map(|painel| self.painel_sanfonado(painel, cx))
                .collect()
        };

        let graficos = self.painel_dos_graficos(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().sidebar)
            // ⚠️ **O histograma fica fora da rolagem**, no alto — é onde o
            // Lightroom o põe, e é o que ele precisa ser para servir: uma medida
            // que se olha **enquanto** se arrasta o slider. Rolando junto com os
            // 53 controles, ele desaparece da tela na primeira seção aberta.
            .child(
                div()
                    .h(px(ALTURA_DOS_GRAFICOS))
                    .flex_none()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(graficos),
            )
            .child(
                div()
                    .id("painel-de-ajustes")
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(12.))
                    .overflow_y_scroll()
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
                    .children(cabecalho)
                    .children(paineis),
            )
    }

    /// Quantos ajustes estão fora do neutro, e o botão que devolve todos.
    ///
    /// 🔑 **Fora dos painéis sanfonados, e no topo** — é o desenho do site.
    /// Zerar tudo é o gesto de "recomeçar" e não pertence a nenhuma seção:
    /// dentro de uma delas pareceria zerar só aquela. Ele estava no **rodapé**,
    /// depois de 53 sliders, com o rótulo "Redefinir ajustes" — para chegar
    /// nele era preciso rolar a coluna inteira.
    ///
    /// E o número diz o que se perde: "10 ajustes fora do neutro" é a única
    /// coisa na tela que responde "esta foto foi mexida?" sem abrir sete
    /// painéis.
    fn cabecalho_dos_ajustes(&self, cx: &mut Context<Self>) -> AnyElement {
        let alterados = self.quantos_alterados();
        let texto = match alterados {
            0 => "Nenhum ajuste fora do neutro".to_string(),
            1 => "1 ajuste fora do neutro".to_string(),
            n => format!("{n} ajustes fora do neutro"),
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(6.))
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(texto)),
            )
            .child(
                Button::new("zerar-tudo")
                    .label("Zerar tudo")
                    .xsmall()
                    .disabled(alterados == 0)
                    .tooltip("Devolve os 53 ajustes ao neutro. O enquadramento não muda.")
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.redefinir_ajustes(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// Quantos dos 53 estão fora do próprio neutro.
    fn quantos_alterados(&self) -> usize {
        CONTROLES
            .iter()
            .filter(|definicao| (definicao.ler)(&self.ajustes) != definicao.neutro())
            .count()
    }

    /// Se alguma coisa desta família saiu do neutro — o ponto âmbar do
    /// cabeçalho.
    fn secao_alterada(&self, secao: Secao) -> bool {
        CONTROLES
            .iter()
            .filter(|definicao| definicao.secao == secao)
            .any(|definicao| (definicao.ler)(&self.ajustes) != definicao.neutro())
    }

    /// Os dois gráficos, juntos: o histograma e a curva de tons.
    ///
    /// ⚠️ **No legado eles moram em lugares diferentes** — o histograma é aba
    /// própria e a curva vive dentro de "AllAdjustments". Aqui os dois são
    /// desenho da mesma coisa (a foto que está na tela, medida), e nenhum tem
    /// controle: separá-los daria uma aba de 120px de altura para um gráfico só.
    fn painel_dos_graficos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .size_full()
            .p(px(10.))
            .bg(cx.theme().sidebar)
            .child(self.histograma(cx))
            .child(self.curva_de_tons(cx))
    }

    /// A barra do modo de corte: o que fazer com o retângulo que está na foto.
    ///
    /// Fica no topo do painel, e só existe enquanto o modo está aberto. No legado
    /// ela vive no painel de revelação junto com tudo o mais; aqui aparecer e
    /// sumir é o que diz, sem texto, que a tela está noutro estado.
    fn barra_de_corte(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let edicao = self.edicao.as_ref()?;
        let grade_ligada = edicao.grade;
        let angulo = edicao.corte.angle();
        let atual = edicao.proporcao.clone();

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .pb(px(8.))
                .mb(px(4.))
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().text_xs().child("Corte"))
                .child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .child(
                            Button::new("corte-aplicar")
                                .label("Aplicar")
                                .xsmall()
                                .primary()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.aplicar_corte(cx);
                                })),
                        )
                        .child(
                            Button::new("corte-cancelar")
                                .label("Cancelar")
                                .xsmall()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.cancelar_corte(cx);
                                })),
                        ),
                )
                .child(
                    Button::new("corte-grade")
                        .label("Grade de terços")
                        .xsmall()
                        .w_full()
                        .when(grade_ligada, |b| b.primary())
                        .selected(grade_ligada)
                        .on_click(cx.listener(|tela, _ev, _window, cx| {
                            if let Some(edicao) = tela.edicao.as_mut() {
                                edicao.grade = !edicao.grade;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .child(
                            Button::new("corte-girar")
                                .label("Girar 90°")
                                .xsmall()
                                .flex_1()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.girar(cx);
                                })),
                        )
                        .child(Button::new("corte-espelho-h").label("⇄").xsmall().on_click(
                            cx.listener(|tela, _ev, _window, cx| {
                                tela.espelhar_horizontal(cx);
                            }),
                        ))
                        .child(Button::new("corte-espelho-v").label("⇅").xsmall().on_click(
                            cx.listener(|tela, _ev, _window, cx| {
                                tela.espelhar_vertical(cx);
                            }),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .child("Endireitar")
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(format!("{:+.1}°", angulo))),
                        ),
                )
                .child(Slider::new(&self.angulo).horizontal())
                .child(
                    // As proporções que o legado oferece no combo, na mesma ordem.
                    div().flex().flex_wrap().gap(px(2.)).children(
                        PROPORCOES
                            .iter()
                            .map(|(rotulo, proporcao)| {
                                let escolhida = *proporcao == atual;
                                let proporcao = proporcao.clone();
                                Button::new(SharedString::from(format!("prop-{rotulo}")))
                                    .label(*rotulo)
                                    .xsmall()
                                    .when(escolhida, |b| b.primary())
                                    .selected(escolhida)
                                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                        tela.travar_proporcao(proporcao.clone(), cx);
                                    }))
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>(),
                    ),
                )
                .child(
                    Button::new("corte-recomecar")
                        .label("Recomeçar")
                        .xsmall()
                        .w_full()
                        .on_click(cx.listener(|tela, _ev, _window, cx| {
                            tela.recomecar_corte(cx);
                        })),
                ),
        )
    }

    /// O histograma, desenhado com `paint_quad` dentro de um `canvas`.
    ///
    /// 🔑 **256 colunas × 3 canais não podem ser 768 `div`s.** Cada `div` é um nó
    /// de layout, e o layout roda a cada quadro; o painel inteiro tem menos de
    /// cem hoje. Aqui vale a exceção — pintar retângulo direto é o que o `canvas`
    /// existe para permitir, e é o análogo do `painter` que o legado usa.
    fn histograma(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const ALTURA: f32 = 70.0;

        let barras: Vec<(f32, f32, f32)> = match self.histograma.as_ref() {
            Some(histograma) => histograma.alturas().collect(),
            None => Vec::new(),
        };
        let fundo = cx.theme().background;

        div().h(px(ALTURA)).w_full().mb(px(6.)).child(
            canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _prepaint, window, _cx| {
                    window.paint_quad(gpui::fill(bounds, fundo));

                    if barras.is_empty() {
                        return;
                    }

                    let largura = f32::from(bounds.size.width) / barras.len() as f32;
                    let base = f32::from(bounds.origin.y) + f32::from(bounds.size.height);

                    for (i, (r, g, b)) in barras.iter().enumerate() {
                        let x = f32::from(bounds.origin.x) + i as f32 * largura;
                        // Os três canais somam luz onde se sobrepõem — cinza
                        // vira branco, que é o que se espera de um
                        // histograma. Alfa fixo, como no legado (100/255).
                        for (altura, cor) in [
                            (r, gpui::rgba(0xff000064)),
                            (g, gpui::rgba(0x00ff0064)),
                            (b, gpui::rgba(0x0000ff64)),
                        ] {
                            let alta = altura * ALTURA;
                            if alta <= 0.0 {
                                continue;
                            }
                            window.paint_quad(gpui::fill(
                                Bounds {
                                    origin: gpui::point(px(x), px(base - alta)),
                                    size: gpui::size(px(largura.max(1.0)), px(alta)),
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

    /// A curva de tons: a diagonal tracejada e a curva de agora, por cima.
    ///
    /// 🔑 Desenhada como 100 segmentos horizontais de 1px — o GPUI não tem
    /// primitiva de linha, e `paint_quad` é o que existe. Para uma curva que
    /// atravessa 280px, um retângulo por passo é indistinguível de uma linha, e
    /// custa o mesmo que o histograma ao lado.
    fn curva_de_tons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const ALTURA: f32 = 120.0;

        let pontos = curva::curva(&self.ajustes);
        let fundo = cx.theme().background;
        let borda = cx.theme().border;
        let linha = cx.theme().primary;

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .mb(px(6.))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Curva de tons"),
            )
            .child(
                div().h(px(ALTURA)).w_full().child(
                    canvas(
                        |_bounds, _window, _cx| {},
                        move |bounds, _prepaint, window, _cx| {
                            window.paint_quad(gpui::fill(bounds, fundo));

                            let x0 = f32::from(bounds.origin.x);
                            let y0 = f32::from(bounds.origin.y);
                            let largura = f32::from(bounds.size.width);
                            let altura = f32::from(bounds.size.height);
                            let passo = largura / (pontos.len() - 1) as f32;

                            // A diagonal de referência, pontilhada: sem ela não dá
                            // para ver se a curva está levantando ou baixando.
                            for i in (0..pontos.len()).step_by(3) {
                                let t = i as f32 / (pontos.len() - 1) as f32;
                                window.paint_quad(gpui::fill(
                                    Bounds {
                                        origin: gpui::point(
                                            px(x0 + t * largura),
                                            px(y0 + (1.0 - t) * altura),
                                        ),
                                        size: gpui::size(px(2.), px(1.)),
                                    },
                                    borda,
                                ));
                            }

                            for i in 0..pontos.len() - 1 {
                                let y_a = y0 + (1.0 - pontos[i]) * altura;
                                let y_b = y0 + (1.0 - pontos[i + 1]) * altura;
                                // O segmento vira um retângulo que cobre a subida
                                // entre os dois pontos: sem isso, uma curva
                                // íngreme apareceria como escada de pontos soltos.
                                let topo = y_a.min(y_b);
                                let alta = (y_a - y_b).abs().max(2.0);
                                window.paint_quad(gpui::fill(
                                    Bounds {
                                        origin: gpui::point(px(x0 + i as f32 * passo), px(topo)),
                                        size: gpui::size(px(passo.max(1.0)), px(alta)),
                                    },
                                    linha,
                                ));
                            }
                        },
                    )
                    .size_full(),
                ),
            )
    }

    /// A lista de presets: os de sistema e os do usuário, como no legado.
    ///
    /// ⚠️ **Fica no mesmo painel dos ajustes, e no legado é um dock à parte.** A
    /// Revelação nova não tem docking (fase 4), e inventar um painel esquerdo só
    /// para isto seria decidir agora um layout que a fase 4 vai refazer. O que
    /// importa para a paridade — quais presets existem, o que cada um aplica — é
    /// igual.
    /// A lista de predefinições — o desenho do site (`painel-presets.tsx`).
    ///
    /// Busca em cima com o botão de salvar ao lado, dois grupos com contagem
    /// ("Do sistema 7", "Minhas 0"), o número de campos que cada uma escreve à
    /// direita do nome, e o rodapé dizendo o que o ponteiro faz.
    ///
    /// 🚨 **A lista inteira era uma sanfona fechada.** O motivo estava escrito e
    /// tinha data: ela dividia a coluna de 280px com os 42 sliders. Só que ela
    /// não divide mais nada desde que virou painel próprio do dock — e fechada
    /// por padrão, num painel que existe só para ela, o que se via ao abrir a
    /// Revelação era a palavra "Presets" e um triângulo.
    fn presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busca = self.busca_de_presets.read(cx).value().to_string();
        let (do_sistema, minhas) = presets::separar_filtrando(&self.presets, &busca);
        let nenhuma = do_sistema.is_empty() && minhas.is_empty();
        let busca = busca.trim().to_string();
        // ⚠️ **"Nenhuma com esse nome" e "nenhuma ainda" são coisas diferentes.**
        // Sem a distinção, quem digitasse errado leria que o app não tem
        // predefinição nenhuma — e iria criar a que já existe.
        let vazio = if busca.is_empty() {
            "Nenhuma predefinição ainda."
        } else {
            "Nenhuma predefinição com esse nome."
        };

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.busca_de_presets).xsmall()),
                    )
                    .child(self.salvar_como_preset(cx))
                    .child(
                        Button::new("importar-do-lightroom")
                            .label("↑")
                            .xsmall()
                            .tooltip("Importar do Lightroom (.lrtemplate, .xmp)")
                            .disabled(self.escolhendo_arquivos)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.importar_do_lightroom(cx);
                            })),
                    ),
            )
            .children(self.resultado_da_importacao(cx))
            .when(nenhuma, |painel| {
                painel.child(
                    div()
                        .py(px(8.))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(vazio),
                )
            })
            .when(!nenhuma, |painel| {
                painel
                    .child(self.grupo_de_presets("Do sistema", &do_sistema, None, cx))
                    .child(
                        self.grupo_de_presets(
                            "Minhas",
                            &minhas,
                            // Só quando não há nenhuma salva — com a busca vazia de
                            // resultados, a explicação de como criar seria resposta
                            // à pergunta errada.
                            presets::nenhuma_do_usuario(&self.presets)
                                .then_some("Ajuste uma foto e use o + para guardar."),
                            cx,
                        ),
                    )
            })
            .child(
                div()
                    .pt(px(4.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Passe o ponteiro para ver na foto, clique para aplicar. \
                         Cada uma escreve só os controles que define — os outros ficam como estão.",
                    ),
            )
    }

    /// O que a última importação aproveitou — e o que não.
    ///
    /// 🔑 **Fica na coluna, e não num diálogo que se fecha sozinho.** A lista de
    /// recursos ignorados é longa quando os presets são de coleção comercial, e
    /// ela é a resposta para "por que este preset mudou tão pouco?" — pergunta
    /// que só aparece depois de aplicar o primeiro.
    fn resultado_da_importacao(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let relatorio = self.relatorio.as_ref()?;

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(6.))
                .rounded(px(4.))
                .bg(cx.theme().muted)
                .text_xs()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(4.))
                        .child(div().flex_1().child(SharedString::from(relatorio.resumo())))
                        .child(
                            Button::new("fechar-relatorio")
                                .label("✕")
                                .xsmall()
                                .ghost()
                                .tooltip("Fechar o resultado")
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.fechar_relatorio(cx);
                                })),
                        ),
                )
                .children(
                    relatorio
                        .linhas()
                        .into_iter()
                        .map(|linha| {
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(linha))
                        })
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
        )
    }

    /// Um bloco da lista, com o título e a contagem — o desenho do Lightroom.
    fn grupo_de_presets(
        &self,
        titulo: &'static str,
        presets: &[&Preset],
        vazio: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .pb(px(2.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(titulo.to_uppercase()))
                    .child(SharedString::from(presets.len().to_string())),
            )
            .children(match (presets.is_empty(), vazio) {
                (true, Some(texto)) => Some(
                    div()
                        .pb(px(4.))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(texto),
                ),
                _ => None,
            })
            .children(
                presets
                    .iter()
                    .map(|preset| self.botao_de_preset(preset, cx))
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    /// O botão que abre o diálogo de salvar preset.
    ///
    /// ⚠️ **O diálogo é do `gpui-component`, e depende do `Root`** estar na
    /// primeira camada da janela (`main.rs`) — sem ele, `open_dialog` derruba o
    /// app num `expect` em vez de abrir o diálogo.
    fn salvar_como_preset(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("salvar-preset")
            .label("+")
            .xsmall()
            .tooltip("Salvar os ajustes atuais como predefinição")
            .on_click(cx.listener(|tela, _ev, window, cx| {
                // O campo começa vazio a cada abertura: o nome do preset anterior
                // sugerido como padrão convida a salvar dois com o mesmo nome, e
                // nada no banco impede.
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.set_value("", window, cx));
                tela.preset_inteiro = false;

                let campo = tela.nome_do_preset.clone();
                let esta = cx.entity();
                let quantos = tela.quantos_alterados();

                window.open_dialog(cx, move |dialogo, _window, cx| {
                    let campo = campo.clone();
                    let esta = esta.clone();
                    let inteiro = esta.read(cx).preset_inteiro;
                    let para_marcar = esta.clone();

                    dialogo
                        .title("Salvar como predefinição")
                        .confirm()
                        .child(Input::new(&campo))
                        // 🔑 **A caixa do site, e ela não precisa de campo novo
                        // no banco**: guardar os 53 — inclusive os que estão no
                        // neutro — já é "zerar o resto ao aplicar". É a
                        // predefinição que é um visual inteiro, e não um retoque
                        // para somar.
                        .child(
                            div()
                                .id("preset-inteiro")
                                .pt(px(8.))
                                .cursor_pointer()
                                .text_xs()
                                .child(SharedString::from(format!(
                                    "{} Zerar os outros ajustes ao aplicar",
                                    if inteiro { "☑" } else { "☐" }
                                )))
                                .on_click(move |_ev, _window, cx| {
                                    para_marcar.update(cx, |tela, cx| {
                                        tela.preset_inteiro = !tela.preset_inteiro;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            div()
                                .pt(px(4.))
                                .text_xs()
                                .child(SharedString::from(if inteiro {
                                    "Guarda os 53 ajustes: aplicar devolve ao neutro o que ela não pede."
                                        .to_string()
                                } else if quantos == 0 {
                                    "Nenhum ajuste fora do neutro: não há o que guardar.".to_string()
                                } else {
                                    format!("Guarda {quantos} ajustes — os que saíram do neutro.")
                                })),
                        )
                        .on_ok(move |_ev, window, cx| {
                            esta.update(cx, |tela, cx| tela.salvar_preset(window, cx));
                            true
                        })
                });
            }))
    }

    /// 🔑 O id do elemento é o **id do preset**, e não a posição na lista.
    ///
    /// Dois presets com o mesmo nome são possíveis (nada impede salvar "Retrato"
    /// duas vezes), e id por posição faria o GPUI confundir o estado de dois
    /// botões quando a lista mudasse de tamanho — salvar um preset novo trocaria
    /// qual deles parece pressionado.
    ///
    /// ⚠️ **O ponteiro em cima mostra na foto, e o clique aplica.** São dois
    /// caminhos diferentes de propósito: a prévia não passa pelo histórico nem
    /// pelo banco, e sair com o ponteiro a desfaz. Sem ela, escolher entre sete
    /// predefinições custa sete aplicações e sete `Cmd+Z`.
    fn botao_de_preset(&self, preset: &Preset, cx: &mut Context<Self>) -> gpui::AnyElement {
        let nome = SharedString::from(preset.name.clone());
        let quantos = SharedString::from(presets::quantos_campos(preset).to_string());
        let escolhido = preset.clone();
        let para_prever = preset.adjustments.clone();

        div()
            .id(SharedString::from(format!("preset-{}", preset.id)))
            .flex()
            .items_center()
            .gap(px(2.))
            .px(px(4.))
            .py(px(1.))
            .rounded(px(4.))
            .text_xs()
            .hover(|estilo| estilo.bg(cx.theme().accent))
            .on_hover(cx.listener(move |tela, sobre: &bool, _window, cx| {
                tela.prever(sobre.then(|| para_prever.clone()), cx);
            }))
            // 🚨 **O clique mora no nome, e não na linha.** Renomear e apagar
            // são filhos dela; com o `on_click` na linha inteira, clicar no
            // lixo aplicaria a predefinição antes de abrir a pergunta — e a
            // resposta "cancelar" deixaria a foto alterada mesmo assim.
            .child(
                div()
                    .id(SharedString::from(format!("aplicar-{}", preset.id)))
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .items_center()
                    .gap(px(4.))
                    .py(px(2.))
                    .cursor_pointer()
                    .child(div().flex_1().truncate().child(nome))
                    .child(div().text_color(cx.theme().muted_foreground).child(quantos))
                    .on_click(cx.listener(move |tela, _ev, window, cx| {
                        // 🚨 A prévia sai **antes** de aplicar: se ela ficasse,
                        // o resultado na tela seria o preset por cima dele
                        // mesmo — igual por acaso, e diferente assim que o
                        // ponteiro saísse.
                        tela.prever(None, cx);
                        tela.aplicar_preset(&escolhido, window, cx);
                    })),
            )
            // ⚠️ **Renomear e apagar só aparecem nas do fotógrafo.** No site
            // eles ficam escondidos até o ponteiro passar (`opacity-0
            // group-hover`); aqui ficam visíveis, porque um botão de apagar
            // invisível continua clicável — no navegador é risco pequeno, num
            // app de catálogo é o gesto que ninguém desfaz.
            .children(self.acoes_do_preset(preset, cx))
            .into_any_element()
    }

    /// Renomear e apagar — só para as do fotógrafo.
    ///
    /// Uma do sistema não tem linha no banco para apagar, e o botão mandaria um
    /// `DELETE` para um id que o repositório não conhece: ela sumiria da tela e
    /// voltaria na abertura seguinte.
    fn acoes_do_preset(&self, preset: &Preset, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if preset.is_system {
            return Vec::new();
        }

        let id = preset.id;
        let nome = preset.name.clone();
        let para_renomear = nome.clone();
        let para_apagar = nome.clone();

        vec![
            Button::new(SharedString::from(format!("renomear-{id}")))
                .label("✎")
                .xsmall()
                .ghost()
                .tooltip(SharedString::from(format!("Renomear \"{nome}\"")))
                .on_click(cx.listener(move |tela, _ev, window, cx| {
                    let nome = para_renomear.clone();
                    // O campo começa com o nome de agora: renomear é corrigir
                    // uma palavra, e um campo vazio obrigaria a redigitar tudo.
                    tela.renome_do_preset.update(cx, |estado, cx| {
                        estado.set_value(nome.clone(), window, cx);
                    });

                    let campo = tela.renome_do_preset.clone();
                    let esta = cx.entity();
                    window.open_dialog(cx, move |dialogo, _window, _cx| {
                        let campo = campo.clone();
                        let esta = esta.clone();
                        dialogo
                            .title("Renomear predefinição")
                            .confirm()
                            .child(Input::new(&campo))
                            .on_ok(move |_ev, _window, cx| {
                                let nome = campo.read(cx).value().to_string();
                                esta.update(cx, |tela, cx| tela.renomear_preset(id, nome, cx));
                                true
                            })
                    });
                }))
                .into_any_element(),
            Button::new(SharedString::from(format!("apagar-{id}")))
                .label("🗑")
                .xsmall()
                .ghost()
                .tooltip(SharedString::from(format!("Apagar \"{nome}\"")))
                .on_click(cx.listener(move |_tela, _ev, window, cx| {
                    // 🚨 **Apagar pergunta antes**, e é o único gesto desta
                    // coluna que não se desfaz: a predefinição não está em foto
                    // nenhuma, então nem o `Cmd+Z` nem reabrir a trazem de volta.
                    let esta = cx.entity();
                    let nome = para_apagar.clone();
                    window.open_dialog(cx, move |dialogo, _window, _cx| {
                        let esta = esta.clone();
                        dialogo
                            .title("Apagar predefinição")
                            .confirm()
                            .child(SharedString::from(format!(
                                "Apagar \"{nome}\"? Ela não volta."
                            )))
                            .on_ok(move |_ev, _window, cx| {
                                esta.update(cx, |tela, cx| tela.apagar_preset(id, cx));
                                true
                            })
                    });
                }))
                .into_any_element(),
        ]
    }

    /// O botão do tom automático, no topo do Básico.
    ///
    /// **Desligado sem foto crua**: sem pixels no cache não há histograma, e um
    /// botão que aceita o clique para não fazer nada é a promessa vazia que este
    /// módulo inteiro existe para desfazer.
    fn botao_do_automatico(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pronto = matches!(self.aberta.as_ref(), Some(Aberta { bruta: Some(_), .. }));

        Button::new("tom-automatico")
            .label("Auto")
            .xsmall()
            .w_full()
            .disabled(!pronto)
            .on_click(cx.listener(|tela, _ev, window, cx| {
                tela.tom_automatico(window, cx);
            }))
    }

    /// Um painel sanfonado: o cabeçalho sempre, os controles só quando aberto.
    ///
    /// Fechado por padrão (menos o Básico), como no site e como no legado. São
    /// 53 controles: com tudo aberto a coluna vira dois metros de sliders, e o
    /// efeito prático é nenhum deles ser encontrado.
    ///
    /// 🔑 **O ponto âmbar no cabeçalho é o que faz a sanfona valer.** Fechado,
    /// um painel esconde o que tem dentro — inclusive um ajuste que alguém
    /// deixou lá. O ponto responde "mexeram nisto" sem abrir, e é o mesmo sinal
    /// do site (`<span className="bg-amber-400" aria-label="alterado" />`).
    fn painel_sanfonado(&self, painel: Painel, cx: &mut Context<Self>) -> AnyElement {
        let aberto = self.abertos.contains(&painel);
        let secoes = painel.secoes();
        let alterado = secoes.iter().any(|secao| self.secao_alterada(*secao));

        // Com mais de uma família, quem manda é a aba escolhida; com uma só, a
        // aba não existe e a família é a própria.
        let visivel = if secoes.len() > 1 {
            self.aba_hsl
        } else {
            secoes[0]
        };
        let abas = (secoes.len() > 1).then(|| self.abas(secoes, visivel, cx));

        Collapsible::new()
            .open(aberto)
            .child(
                div()
                    .id(SharedString::from(format!("painel-{}", painel.rotulo())))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .py(px(4.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().flex_1().child(painel.rotulo()))
                    .when(alterado, |cabecalho| {
                        cabecalho.child(
                            div()
                                .size(px(5.))
                                .rounded_full()
                                .bg(tema::cores::quente())
                                .flex_shrink_0(),
                        )
                    })
                    // Triângulo, e não texto: é o que diz "isto abre" sem
                    // ocupar largura numa coluna de 280px.
                    .child(if aberto { "▾" } else { "▸" })
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        if !tela.abertos.remove(&painel) {
                            tela.abertos.insert(painel);
                        }
                        cx.notify();
                    })),
            )
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .pb(px(8.))
                    .children(abas)
                    // O "Auto" mora no Básico e só nele — é onde ele fica no
                    // Lightroom, junto dos tons que decide. Fora daqui ele seria
                    // mais um botão à procura de dono.
                    .children((painel == Painel::Basico).then(|| self.botao_do_automatico(cx)))
                    .children(self.controles_da_secao(visivel, cx)),
            )
            .into_any_element()
    }

    /// A fileira de abas do HSL — Cor, Luminância, Matiz.
    ///
    /// ⚠️ **A aba que não está à mostra também precisa se anunciar.** Uma
    /// alteração na luminância fica invisível enquanto a aba aberta é a de cor,
    /// e o ponto do cabeçalho diz "algum HSL foi mexido" sem dizer qual. O
    /// sublinhado âmbar na aba fechada é o que fecha essa lacuna — é o que o
    /// site faz (`underline decoration-amber-400`).
    fn abas(&self, secoes: &'static [Secao], visivel: Secao, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .gap(px(4.))
            .children(
                secoes
                    .iter()
                    .map(|secao| {
                        let secao = *secao;
                        let escolhida = secao == visivel;
                        let alterada = self.secao_alterada(secao);

                        div()
                            .id(SharedString::from(format!("aba-{}", secao.rotulo())))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_xs()
                            .when(escolhida, |aba| {
                                aba.bg(cx.theme().accent)
                                    .text_color(cx.theme().accent_foreground)
                            })
                            .when(!escolhida, |aba| {
                                aba.text_color(cx.theme().muted_foreground)
                            })
                            .when(alterada && !escolhida, |aba| {
                                aba.underline().text_decoration_color(tema::cores::quente())
                            })
                            .child(Painel::aba(secao))
                            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                tela.aba_hsl = secao;
                                cx.notify();
                            }))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    /// Os sliders de uma família, na ordem da tabela.
    fn controles_da_secao(&self, secao: Secao, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.controles
            .iter()
            .enumerate()
            .filter(|(_, controle)| controle.definicao.secao == secao)
            .map(|(i, controle)| self.linha_do_controle(i, controle, cx))
            .collect()
    }

    /// Um controle: o rótulo, o valor e a barra.
    ///
    /// 🔑 **Duplo clique no rótulo devolve o neutro** — o gesto do Lightroom, e
    /// o que o dono pediu ao site em 2026-09-05 (*"quando der dois cliques no
    /// meio do slide deve zerar o efeito"*). Sem ele, voltar um único ajuste ao
    /// lugar exige arrastar até acertar um número que a barra nem sempre
    /// alcança: `sharpen_radius` neutro é 1,0 numa faixa de 0,5 a 3,0.
    ///
    /// ⚠️ **E o rótulo muda de cor quando o controle sai do neutro.** Com sete
    /// painéis fechando e abrindo, "o que eu mexi aqui dentro" não tem outra
    /// resposta senão comparar 53 números com 53 neutros de cabeça.
    fn linha_do_controle(
        &self,
        indice: usize,
        controle: &Controle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definicao = controle.definicao;
        let valor = (definicao.ler)(&self.ajustes);
        let neutro = definicao.neutro();
        let no_neutro = valor == neutro;

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(
                        div()
                            .id(SharedString::from(format!("rotulo-{indice}")))
                            .cursor_pointer()
                            .tooltip(|window, cx| {
                                gpui_component::tooltip::Tooltip::new(
                                    "Duplo clique volta ao neutro",
                                )
                                .build(window, cx)
                            })
                            .text_color(if no_neutro {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().foreground
                            })
                            .child(definicao.rotulo)
                            .on_click(cx.listener(
                                move |tela, evento: &gpui::ClickEvent, window, cx| {
                                    if evento.click_count() >= 2 {
                                        tela.devolver_ao_neutro(indice, window, cx);
                                    }
                                },
                            )),
                    )
                    // O valor fica ao lado do rótulo, e não dentro da barra:
                    // dentro, ele se move junto com o punho e vira um número
                    // que foge de quem tenta lê-lo.
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(definicao.formatar(valor))),
                    ),
            )
            .child(Slider::new(&controle.estado).horizontal())
            .into_any_element()
    }
}

impl Revelacao {
    /// A faixa do rodapé: **o acervo inteiro**, rolável, com a atual em vista.
    ///
    /// # 🚨 Ela mostrava só ±7 vizinhas, e não havia como chegar no resto
    ///
    /// O motivo estava escrito e era honesto: *"uma faixa com 2.000 itens
    /// custaria 2.000 consultas ao cache por quadro"*. Só que a conta era do
    /// caminho antigo, que decodificava o JPEG dentro do `render`. Com o
    /// [`CacheDeMiniaturas`] cada item custa **uma leitura de memória**, e a
    /// razão para esconder o acervo caiu junto.
    ///
    /// O que sobrava do jeito antigo: quem revelava a foto 3 de 200 não tinha
    /// como pular para a 150 sem voltar à grade — e "voltar à grade" é
    /// exatamente o que o filmstrip existe para evitar.
    ///
    /// ⚠️ **Some com um acervo de uma foto.** Uma faixa com um item só ocupa
    /// espaço da foto para não dizer nada — e é o que acontece ao abrir a
    /// Revelação sem lista (os testes, e o caminho de `abrir`).
    fn filmstrip(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        // A miniatura é o que sobra da altura da faixa, como no site.
        const LADO: f32 = ALTURA_DO_FILMSTRIP - 16.0;

        if self.acervo.len() < 2 {
            return None;
        }

        // Carrega o que falta **antes** de montar — o quadro só lê.
        self.miniaturas_da_tira.ajustar_capacidade(
            NonZeroUsize::new(self.acervo.len().clamp(1, MINIATURAS_DA_TIRA))
                .expect("o piso 1 garante que não é zero"),
        );
        let ids: Vec<String> = self.acervo.iter().map(|f| f.id.clone()).collect();
        for id in &ids {
            if self.miniaturas_da_tira.espiar(id).is_none() {
                self.miniaturas_da_tira.obter(&self.previews, id);
            }
        }

        let itens: Vec<gpui::AnyElement> = (0..self.acervo.len())
            .map(|posicao| {
                let foto = &self.acervo[posicao];
                let atual = posicao == self.posicao;
                // A marcada para sincronizar: âmbar, como no site; a aberta
                // continua com a cor de sempre.
                let marcada = !atual && self.marcadas.contains(&posicao);
                let miniatura = match self.miniaturas_da_tira.espiar(&foto.id) {
                    Some(Miniatura::Pronta(imagem)) => Some(imagem),
                    _ => None,
                };
                let revelada = persistencia::ja_revelada(foto);
                let dica = SharedString::from(format!("{}. {}", posicao + 1, foto.name));

                div()
                    .id(SharedString::from(format!("faixa-revelacao-{}", foto.id)))
                    .w(px(LADO))
                    .h(px(LADO))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(if atual {
                        cx.theme().primary
                    } else if marcada {
                        tema::cores::quente()
                    } else {
                        cx.theme().border
                    })
                    .relative()
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(dica.clone()).build(window, cx)
                    })
                    .children(
                        miniatura
                            .map(|imagem| img(imagem).max_w(px(LADO - 4.0)).max_h(px(LADO - 4.0))),
                    )
                    // ✅ O ponto âmbar de "já revelada" — o mesmo do site. Numa
                    // sessão de duzentas, é a única coisa que responde "onde eu
                    // parei" sem abrir foto por foto.
                    .when(revelada, |item| {
                        item.child(
                            div()
                                .absolute()
                                .top(px(3.))
                                .right(px(3.))
                                .size(px(6.))
                                .rounded_full()
                                .bg(tema::cores::quente()),
                        )
                    })
                    .on_click(
                        cx.listener(move |tela, evento: &gpui::ClickEvent, window, cx| {
                            let m = evento.modifiers();
                            // 🚨 Ctrl **e** Cmd acrescentam, como na web
                            // (`ctrlKey || metaKey`). No macOS `secondary()` é
                            // só o Cmd, e o Ctrl+clique chega como clique
                            // esquerdo com `control` — ignorá-lo deixava o
                            // dono sem lote nenhum (7/set/2026).
                            tela.clicar_na_tira(
                                posicao,
                                Modificadores {
                                    aditivo: m.secondary() || m.control,
                                    faixa: m.shift,
                                },
                                window,
                                cx,
                            );
                        }),
                    )
                    .into_any_element()
            })
            .collect();

        // 🔑 A tira segue a foto aberta, e **só quando ela muda**: pedir a cada
        // quadro prenderia a barra e o operador não conseguiria arrastá-la para
        // olhar o resto do acervo.
        if self.ultima_na_tira != Some(self.posicao) {
            self.ultima_na_tira = Some(self.posicao);
            self.rolagem_da_tira.scroll_to_item(self.posicao);
        }

        Some(
            div()
                .id("faixa-da-revelacao")
                .track_scroll(&self.rolagem_da_tira)
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(6.))
                .h(px(ALTURA_DO_FILMSTRIP))
                .flex_none()
                .overflow_x_scroll()
                .border_t_1()
                .border_color(cx.theme().border)
                .children(itens)
                .into_any_element(),
        )
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
    /// "Salvar na galeria e sair" do site: o revelado entra no lugar do
    /// original, na foto que já é da galeria aberta.
    SalvarNaGaleria,
    /// "Sincronizar N" do site: os ajustes desta foto vão para as marcadas na
    /// tira — a receita para o catálogo, e a foto revelada para o site.
    Sincronizar,
    /// Outra foto entrou no palco — pela seta, pela tira ou ao abrir.
    ///
    /// 🔑 **A Revelação não sabe buscar na nuvem, e não vai passar a saber**;
    /// mas é ela quem sabe quando a foto trocou. Sem este aviso a raiz só
    /// buscava os pixels da foto do site **na abertura**: a seta seguinte
    /// caía numa foto sem bruto, e o que aparecia era a miniatura revelada da
    /// galeria — em 640px e com a receita por cima da receita.
    AbriuOutraFoto,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cabecalho = self.cabecalho(cx);
        let presets = self
            .presets_a_mostra
            .then(|| self.coluna_dos_presets(cx).into_any_element());
        let palco = self.palco(cx);
        let ajustes = self.painel(cx).into_any_element();
        let tira = self.filmstrip(cx);

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
                    .child(div().flex().flex_1().min_w(px(0.)).child(palco))
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
        let total = self.acervo.len();
        let posicao = self.posicao();
        let nome = self
            .foto()
            .map(|foto| foto.name.clone())
            .unwrap_or_default();
        let tem_foto = self.aberta.is_some();

        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(ALTURA_DO_CABECALHO))
            .flex_none()
            .px(px(8.))
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("revelacao-fechar")
                    .label("✕")
                    .xsmall()
                    .ghost()
                    .tooltip("Fechar a Revelação (Esc)")
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::Sair);
                    })),
            )
            .child(
                Button::new("revelacao-anterior")
                    .label("‹")
                    .xsmall()
                    .ghost()
                    .tooltip("Foto anterior (seta para a esquerda)")
                    .disabled(posicao == 0 || total <= 1)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.andar(-1, window, cx))),
            )
            .child(
                Button::new("revelacao-proxima")
                    .label("›")
                    .xsmall()
                    .ghost()
                    .tooltip("Próxima foto (seta para a direita)")
                    .disabled(total <= 1 || posicao + 1 >= total)
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.andar(1, window, cx))),
            )
            .child(
                Button::new("revelacao-presets")
                    .label("▤")
                    .xsmall()
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
            .when(total > 1, |barra| {
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
                    .label("↶")
                    .xsmall()
                    .ghost()
                    .tooltip("Desfazer (Cmd+Z)")
                    .disabled(!self.pode_desfazer())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.desfazer(window, cx))),
            )
            .child(
                Button::new("revelacao-refazer")
                    .label("↷")
                    .xsmall()
                    .ghost()
                    .tooltip("Refazer (Cmd+Shift+Z)")
                    .disabled(!self.pode_refazer())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.refazer(window, cx))),
            )
            .child(
                Button::new("revelacao-antes")
                    .label("Antes")
                    .xsmall()
                    .tooltip("Ver a foto sem ajuste (\\)")
                    .when(self.mostrando_original, |b| {
                        b.custom(tema::botao_quente(cx))
                    })
                    .selected(self.mostrando_original)
                    .disabled(!tem_foto)
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.alternar_original(cx))),
            )
            .child(
                Button::new("revelacao-enquadrar")
                    .label("Enquadrar")
                    .xsmall()
                    .tooltip("Girar, espelhar, endireitar e recortar (C)")
                    .when(self.cortando(), |b| b.custom(tema::botao_quente(cx)))
                    .selected(self.cortando())
                    .disabled(!self.tem_pixels())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.alternar_corte(window, cx))),
            )
            // "Sincronizar N": só aparece quando há lote — um botão que quase
            // sempre está desligado vira ruído numa barra que já tem sete
            // controles. É o do site, no mesmo lugar.
            .when(self.marcadas.len() > 1, |barra| {
                let quantas = self.marcadas.len();
                barra.child(
                    Button::new("revelacao-sincronizar")
                        .label(format!("Sincronizar {quantas}"))
                        .xsmall()
                        .tooltip(
                            "Aplicar os ajustes desta foto nas outras marcadas na tira \
                             (Ctrl no clique marca, Shift marca a faixa, Cmd+A marca todas, Cmd+D desmarca)",
                        )
                        .disabled(!tem_foto)
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
                    .label("Exportar JPEG")
                    .xsmall()
                    .disabled(!tem_foto)
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::Exportar);
                    })),
            )
            .child(
                Button::new("revelacao-salvar-na-galeria")
                    .label("Salvar na galeria e sair")
                    .xsmall()
                    .custom(tema::botao_quente(cx))
                    .disabled(!tem_foto)
                    .on_click(cx.listener(|_tela, _ev, _window, cx| {
                        cx.emit(PedidoDaRevelacao::SalvarNaGaleria);
                    })),
            )
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
            .child(self.presets(cx))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use domain::entities::preset::PresetAdjustments;

    use super::super::lightroom::mentira::EscolhaDeMentira;
    use super::super::persistencia::mentira::GravadorDeMentira;
    use super::super::presets::mentira::GuardaDeMentira;

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
                tela.prever(Some(minha.adjustments.clone()), cx);
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
                assert_eq!(relatorio.ignorados, vec![("curva por ponto", 1)]);
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

                tela.prever(Some(preset.adjustments.clone()), cx);

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

                let corte = &tela.edicao.as_ref().unwrap().corte;
                assert_eq!(corte.crop_x(), 0.2);
                assert_eq!(corte.crop_width(), 0.5);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Cancelar devolve o corte que estava gravado.
    ///
    /// A edição é uma **cópia**. Se o modo de corte mexesse direto no corte da
    /// foto, "Cancelar" não teria o que restaurar — e o botão viraria enfeite.
    #[gpui::test]
    fn cancelar_o_corte_nao_muda_a_foto(cx: &mut TestAppContext) {
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
                        edit_crop_x: Some(0.2),
                        edit_crop_width: Some(0.5),
                        ..foto("cortada.jpg")
                    },
                    window,
                    cx,
                );
                tela.alternar_corte(window, cx);

                // Mexe no corte em edição e desiste.
                if let Some(edicao) = tela.edicao.as_mut() {
                    edicao.corte = corte::arrastar(&edicao.corte, 0.3, 0.0);
                }
                tela.cancelar_corte(cx);

                assert!(!tela.cortando());
                assert_eq!(tela.corte.x, Some(0.2), "o corte da foto é o de antes");
            })
            .expect("a janela deve estar aberta");

        assert!(
            gravador.gravado().is_empty(),
            "cancelar não pode gravar nada"
        );
    }

    /// 🚨 Aplicar grava — porque nada mais vai gravar por ele.
    ///
    /// O corte não faz parte de `Ajustes`, então nenhum slider o leva ao banco
    /// depois. Sem esta gravação, o enquadramento só existiria até fechar a tela.
    #[gpui::test]
    fn aplicar_o_corte_grava_na_hora(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = com_gravador(cx, previews, gravador.clone());

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.alternar_corte(window, cx);

                if let Some(edicao) = tela.edicao.as_mut() {
                    edicao.corte = corte::mover_alca(
                        &edicao.corte,
                        Alca::Esquerda,
                        0.25,
                        0.0,
                        (100.0, 100.0),
                        None,
                    );
                }
                tela.aplicar_corte(cx);

                assert!(!tela.cortando(), "aplicar fecha o modo");
                assert_eq!(tela.corte.x, Some(0.25));
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert_eq!(gravado[0].2.x, Some(0.25), "o corte novo foi para o banco");
        assert_eq!(gravado[0].2.largura, Some(0.75));
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
                tela.edicao
                    .as_mut()
                    .expect("o modo de corte está aberto")
                    .corte = CropSettings::new(0.0, 0.0, 0.5, 0.5, 0, 0.0, false, false);
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
                tela.edicao
                    .as_mut()
                    .expect("o modo de corte está aberto")
                    .corte = CropSettings::new(0.0, 0.0, 0.5, 0.5, 0, 0.0, false, false);
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

    /// A proporção travada vale para o arrasto seguinte.
    #[gpui::test]
    fn travar_a_proporcao_muda_o_que_o_arrasto_faz(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                tela.alternar_corte(window, cx);
                tela.travar_proporcao(AspectRatio::Square, cx);

                // Encolhe pela esquerda: com 1:1 travado, a altura tem de
                // acompanhar a largura.
                if let Some(edicao) = tela.edicao.as_mut() {
                    let proporcao = corte::proporcao_de(&edicao.proporcao, (100.0, 100.0));
                    edicao.corte = corte::mover_alca(
                        &edicao.corte,
                        Alca::Esquerda,
                        0.4,
                        0.0,
                        (100.0, 100.0),
                        proporcao,
                    );
                }

                let corte = &tela.edicao.as_ref().unwrap().corte;
                assert!(
                    (corte.crop_width() - corte.crop_height()).abs() < 1e-4,
                    "1:1 numa foto quadrada: {} × {}",
                    corte.crop_width(),
                    corte.crop_height()
                );
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

    /// 🚨 "Redefinir ajustes" zera os 46 e **não** toca no corte.
    ///
    /// No legado o `reset_edits` também não o toca: quem quer desfazer
    /// enquadramento usa "Recomeçar", dentro do modo de corte. Misturar os dois
    /// faria um botão de cor apagar trabalho de composição.
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

    #[gpui::test]
    fn redefinir_zera_os_ajustes_e_preserva_o_corte(cx: &mut TestAppContext) {
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
                assert_eq!(tela.corte.x, Some(0.2), "o corte fica");
                assert!(tela.pode_desfazer(), "redefinir é um passo de histórico");
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert_eq!(gravado[0].1.exposure, 0.0);
        assert_eq!(
            gravado[0].2.x,
            Some(0.2),
            "e vai ao banco com o corte junto"
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
