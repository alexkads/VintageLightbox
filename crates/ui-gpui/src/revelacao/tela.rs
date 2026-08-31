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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::value_objects::{AspectRatio, CropSettings};
use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, App, Bounds, Context, Entity, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, SharedString,
    Subscription, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::collapsible::Collapsible;
use gpui_component::dock::{register_panel, DockArea, DockEvent, DockItem, PanelView};
use gpui_component::input::{Input, InputState};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable, WindowExt};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::imagem::para_gpui;

use super::automatico;
use super::controles::{Definicao, Secao, CONTROLES};
use super::corte::{self, Alca};
use super::curva;
use super::histograma::Histograma;
use super::historico::Historico;
use super::paineis::{PainelDaRevelacao, Qual};
use super::persistencia::{self, Corte, Gravador};
use super::presets::{self, GuardaDePresets};
use super::processador::{Ajustes, Pedido, Processador};
use crate::biblioteca::arranjo;
use infrastructure::transformacao;

/// Largura do painel de ajustes.
const LADO_DO_PAINEL: f32 = 280.0;

/// A coluna dos presets, à esquerda — os 18% do `create_develop_layout`.
const LADO_DOS_PRESETS: f32 = 200.0;

/// A altura dos dois gráficos, no topo da coluna da direita.
const ALTURA_DOS_GRAFICOS: f32 = 230.0;

/// A altura da faixa de miniaturas, embaixo do palco.
const ALTURA_DO_FILMSTRIP: f32 = 84.0;

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
    /// O dock: quem arruma os cinco painéis. `Option` porque nasce depois do
    /// construtor — os painéis precisam da entidade, que ainda não existe lá.
    dock: Option<Entity<DockArea>>,
    /// A gravação adiada do arranjo. Descartá-la cancela, e é o que faz um
    /// arrasto de divisória virar uma escrita só.
    _arranjo: Option<Task<()>>,
    /// Em qual arquivo o arranjo é gravado. Definido ao montar o dock.
    arranjo_em: std::path::PathBuf,
    /// A lista que a Biblioteca estava mostrando, para as setas e o filmstrip.
    /// `Arc` porque o closure do filmstrip a leva consigo.
    acervo: Arc<Vec<PhotoViewModel>>,
    /// Onde estamos nela.
    posicao: usize,
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
    /// Se a lista de presets está aberta. Nasce **fechada**: são 5 de sistema
    /// mais os do usuário empurrando os 42 controles para baixo, e o painel aqui
    /// é uma coluna de 280px — no legado eles moram num dock separado, que esta
    /// Revelação não tem.
    presets_abertos: bool,
    /// Quais seções estão abertas. Um conjunto, e não um `bool` por seção:
    /// acrescentar seção nova não pode exigir lembrar de acrescentar campo.
    abertas: HashSet<Secao>,
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
            dock: None,
            _arranjo: None,
            arranjo_em: std::path::PathBuf::new(),
            acervo: Arc::new(Vec::new()),
            posicao: 0,
            processador: Processador::novo(),
            aberta: None,
            ajustes: Ajustes::default(),
            corte: Corte::default(),
            historico: Historico::novo(Ajustes::default()),
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
            presets_abertos: false,
            abertas: Secao::TODAS
                .into_iter()
                .filter(Secao::nasce_aberta)
                .collect(),
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

    fn mostrar_a_posicao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        let origem = bruta.as_ref().map(|imagem| {
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
        self.historico = Historico::novo(self.ajustes);
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

        cx.notify();
    }

    pub fn foto(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
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
        if !self.pendente {
            return;
        }
        self.pendente = false;
        self.historico.registrar(self.ajustes);
        self.gravar();
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

        if let Some(ajustes) = self.historico.desfazer() {
            self.aplicar_do_historico(ajustes, window, cx);
        }
    }

    /// Avança um passo. `Cmd+Shift+Z`.
    pub fn refazer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();

        if let Some(ajustes) = self.historico.refazer() {
            self.aplicar_do_historico(ajustes, window, cx);
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
        ajustes: Ajustes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ajustes = ajustes;
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao(cx);
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
        self.historico.registrar(self.ajustes);
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
        self.historico.registrar(self.ajustes);
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

        let ajustes = presets::dos_ajustes(&self.ajustes);
        self.guarda_de_presets.salvar(nome.clone(), ajustes.clone());
        self.presets.push(Preset::user(nome, ajustes));

        self.nome_do_preset
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        cx.notify();
    }

    /// Entra ou sai do modo de corte. É o `R` do legado.
    ///
    /// ⚠️ Sair por aqui **descarta** o que estava sendo cortado, como o `R` de
    /// lá: quem aplica usa o botão. Sem essa distinção, uma tecla teria dois
    /// significados conforme o estado, e nenhum aviso de qual valeu.
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

        self.corte = Corte {
            x: Some(edicao.corte.crop_x()),
            y: Some(edicao.corte.crop_y()),
            largura: Some(edicao.corte.crop_width()),
            altura: Some(edicao.corte.crop_height()),
            rotacao: Some(edicao.corte.rotation_90()),
            angulo: Some(edicao.corte.angle()),
            espelho_h: Some(edicao.corte.flip_horizontal()),
            espelho_v: Some(edicao.corte.flip_vertical()),
        };

        // O que estiver a meio caminho fecha antes, senão a espera pendente grava
        // depois com o corte já trocado — e o passo de histórico sairia com o
        // corte novo colado num ajuste antigo.
        self.gravar_o_que_estiver_pendente();
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
        self.historico.registrar(self.ajustes);
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
            ajustes: self.ajustes,
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
            // O palco é neutro e escuro: o olho julga exposição por comparação
            // com o que está em volta, e entorno mais claro que a foto faz toda
            // foto parecer subexposta.
            .bg(cx.theme().background)
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
    fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sem GPU não há revelação, e o painel diz isso em vez de oferecer
        // sliders que não movem nada. `None` é "a thread ainda está abrindo o
        // dispositivo" — não é ausência de placa, e anunciar ausência durante os
        // milissegundos de abertura seria mentir em toda abertura.
        let sem_motor = self.processador.disponivel() == Some(false);

        div()
            .id("painel-de-ajustes")
            .flex()
            .flex_col()
            .gap(px(6.))
            .size_full()
            .p(px(12.))
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
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
            .children(self.barra_de_corte(cx))
            .children(
                Secao::TODAS
                    .into_iter()
                    .map(|secao| self.secao(secao, cx))
                    .collect::<Vec<_>>(),
            )
            .child(
                div().pt(px(8.)).child(
                    Button::new("redefinir-ajustes")
                        .label("Redefinir ajustes")
                        .xsmall()
                        .w_full()
                        .on_click(cx.listener(|tela, _ev, window, cx| {
                            tela.redefinir_ajustes(window, cx);
                        })),
                ),
            )
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

    /// A lista de presets, sozinha — como a aba `Presets` do legado.
    fn painel_dos_presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("painel-de-presets")
            .flex()
            .flex_col()
            .size_full()
            .p(px(10.))
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
            .child(self.presets(cx))
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
    fn presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (sistema, usuario) = presets::separar(&self.presets);

        let mut lista = Vec::new();
        lista.push(self.rotulo_de_grupo("Sistema", cx).into_any_element());
        lista.extend(sistema.iter().map(|p| self.botao_de_preset(p, cx)));
        lista.push(self.rotulo_de_grupo("Meus", cx).into_any_element());
        if usuario.is_empty() {
            lista.push(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Nenhum preset salvo")
                    .into_any_element(),
            );
        } else {
            lista.extend(usuario.iter().map(|p| self.botao_de_preset(p, cx)));
        }

        Collapsible::new()
            .open(self.presets_abertos)
            .child(
                div()
                    .id("secao-presets")
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(4.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Presets")
                    .child(if self.presets_abertos { "▾" } else { "▸" })
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.presets_abertos = !tela.presets_abertos;
                        cx.notify();
                    })),
            )
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .pb(px(8.))
                    .children(lista)
                    .child(div().pt(px(6.)).child(self.salvar_como_preset(cx))),
            )
    }

    /// O botão que abre o diálogo de salvar preset.
    ///
    /// ⚠️ **O diálogo é do `gpui-component`, e depende do `Root`** estar na
    /// primeira camada da janela (`main.rs`) — sem ele, `open_dialog` derruba o
    /// app num `expect` em vez de abrir o diálogo.
    fn salvar_como_preset(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("salvar-preset")
            .label("+ Salvar como preset")
            .xsmall()
            .w_full()
            .on_click(cx.listener(|tela, _ev, window, cx| {
                // O campo começa vazio a cada abertura: o nome do preset anterior
                // sugerido como padrão convida a salvar dois com o mesmo nome, e
                // nada no banco impede.
                tela.nome_do_preset
                    .update(cx, |estado, cx| estado.set_value("", window, cx));

                let campo = tela.nome_do_preset.clone();
                let esta = cx.entity();

                window.open_dialog(cx, move |dialogo, _window, _cx| {
                    let campo = campo.clone();
                    let esta = esta.clone();

                    dialogo
                        .title("Salvar como preset")
                        .confirm()
                        .child(Input::new(&campo))
                        .on_ok(move |_ev, window, cx| {
                            esta.update(cx, |tela, cx| tela.salvar_preset(window, cx));
                            true
                        })
                });
            }))
    }

    fn rotulo_de_grupo(&self, texto: &'static str, cx: &App) -> impl IntoElement {
        div()
            .pt(px(4.))
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(texto)
    }

    /// 🔑 O id do elemento é o **id do preset**, e não a posição na lista.
    ///
    /// Dois presets com o mesmo nome são possíveis (nada impede salvar "Retrato"
    /// duas vezes), e id por posição faria o GPUI confundir o estado de dois
    /// botões quando a lista mudasse de tamanho — salvar um preset novo trocaria
    /// qual deles parece pressionado.
    fn botao_de_preset(&self, preset: &Preset, cx: &mut Context<Self>) -> gpui::AnyElement {
        let id = SharedString::from(format!("preset-{}", preset.id));
        let nome = SharedString::from(preset.name.clone());
        let escolhido = preset.clone();

        Button::new(id)
            .label(nome)
            .xsmall()
            .w_full()
            .justify_start()
            .on_click(cx.listener(move |tela, _ev, window, cx| {
                tela.aplicar_preset(&escolhido, window, cx);
            }))
            .into_any_element()
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

    /// Uma seção sanfonada: o cabeçalho sempre, os controles só quando aberta.
    ///
    /// Fechada por padrão (menos o Básico), como no legado. São 42 controles: com
    /// tudo aberto o painel vira uma coluna de dois metros, e o efeito prático é
    /// nenhum deles ser encontrado.
    fn secao(&self, secao: Secao, cx: &mut Context<Self>) -> impl IntoElement {
        let aberta = self.abertas.contains(&secao);

        Collapsible::new()
            .open(aberta)
            .child(
                div()
                    .id(SharedString::from(format!("secao-{}", secao.rotulo())))
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(4.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(secao.rotulo())
                    // Triângulo, e não texto: é o que diz "isto abre" sem
                    // ocupar largura numa coluna de 280px.
                    .child(if aberta { "▾" } else { "▸" })
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        if !tela.abertas.remove(&secao) {
                            tela.abertas.insert(secao);
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
                    // O "Auto" mora no Básico e só nele — é onde ele fica no
                    // Lightroom, junto dos tons que decide. Fora daqui ele seria
                    // mais um botão à procura de dono.
                    .children((secao == Secao::Basico).then(|| self.botao_do_automatico(cx)))
                    .children(
                        self.controles
                            .iter()
                            .filter(|controle| controle.definicao.secao == secao)
                            .map(|controle| {
                                let definicao = controle.definicao;
                                let valor = (definicao.ler)(&self.ajustes);

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
                                            .child(definicao.rotulo)
                                            // O valor fica ao lado do rótulo, e
                                            // não dentro da barra: dentro, ele se
                                            // move junto com o punho e vira um
                                            // número que foge de quem tenta lê-lo.
                                            .child(
                                                div()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(SharedString::from(
                                                        definicao.formatar(valor),
                                                    )),
                                            ),
                                    )
                                    .child(Slider::new(&controle.estado).horizontal())
                            })
                            .collect::<Vec<_>>(),
                    ),
            )
    }
}

impl Revelacao {
    /// A faixa do rodapé: onde esta foto está na sequência.
    ///
    /// Mostra a **vizinhança** da atual, e não o acervo inteiro — é a mesma
    /// decisão do filmstrip da Biblioteca, e a mesma pergunta: "o que vem antes
    /// e depois desta". Uma faixa com 2.000 itens custaria 2.000 consultas ao
    /// cache por quadro para responder o mesmo.
    ///
    /// ⚠️ **Some com um acervo de uma foto.** Uma faixa com um item só ocupa
    /// espaço da foto para não dizer nada — e é o que acontece ao abrir a
    /// Revelação sem lista (os testes, e o caminho de `abrir`).
    fn filmstrip(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        /// Quantas de cada lado. Ímpar de propósito: a atual fica no meio.
        const VIZINHAS: usize = 7;
        const LADO: f32 = 52.0;

        if self.acervo.len() < 2 {
            return None;
        }

        let inicio = self.posicao.saturating_sub(VIZINHAS);
        let fim = (inicio + VIZINHAS * 2 + 1).min(self.acervo.len());

        let itens: Vec<gpui::AnyElement> = (inicio..fim)
            .map(|posicao| {
                let foto = &self.acervo[posicao];
                let atual = posicao == self.posicao;
                let miniatura = self
                    .previews
                    .get_thumbnail(&foto.id)
                    .map(crate::imagem::para_gpui);

                div()
                    .id(SharedString::from(format!("faixa-revelacao-{}", foto.id)))
                    .w(px(LADO))
                    .h(px(LADO))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(if atual {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .children(
                        miniatura
                            .map(|imagem| img(imagem).max_w(px(LADO - 4.0)).max_h(px(LADO - 4.0))),
                    )
                    .on_click(cx.listener(move |tela, _ev, window, cx| {
                        tela.ir_para(posicao, window, cx);
                    }))
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .h(px(LADO + 16.0))
                .flex_none()
                .border_t_1()
                .border_color(cx.theme().border)
                .children(itens)
                .into_any_element(),
        )
    }
}

impl Revelacao {
    /// O desenho de um painel, para a view fina do dock chamar.
    ///
    /// 🔑 **Os painéis não têm estado próprio** — a mesma decisão da Biblioteca,
    /// pela mesma razão: os `cx.listener` dos 42 controles esperam
    /// `Context<Revelacao>`, e movê-los para dentro de views novas trocaria todos
    /// eles por `entidade.update(…)`.
    pub fn desenhar_painel(
        &mut self,
        qual: Qual,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match qual {
            Qual::Palco => self.palco(cx),
            Qual::Ajustes => self.painel(cx).into_any_element(),
            Qual::Graficos => self.painel_dos_graficos(cx).into_any_element(),
            Qual::Presets => self.painel_dos_presets(cx).into_any_element(),
            // ⚠️ Sem lista, o filmstrip é um painel vazio — e não some, como
            // sumia antes do dock: um painel que desaparece do arranjo salvo
            // levaria junto o lugar dele, e a foto seguinte reapareceria noutro
            // canto da tela.
            Qual::Filmstrip => self
                .filmstrip(cx)
                .unwrap_or_else(|| div().size_full().into_any_element()),
        }
    }

    /// Monta o dock com o arranjo padrão, e restaura o salvo por cima.
    ///
    /// 🚨 **Só pode ser chamado depois de a entidade existir** — a mesma regra da
    /// Biblioteca: os painéis guardam uma referência fraca a ela.
    ///
    /// O arranjo é o do legado (`create_develop_layout`): presets à esquerda, a
    /// foto no centro, gráficos e ajustes à direita, filmstrip embaixo.
    pub fn montar_o_dock(
        &mut self,
        eu: &Entity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.montar_o_dock_em(eu, arranjo::caminho("revelacao"), window, cx);
    }

    /// O mesmo, com o arquivo de arranjo escolhido.
    ///
    /// 🚨 **Existe pela mesma razão da Biblioteca**: o caminho padrão é o
    /// catálogo de verdade, e um teste que exercitasse a gravação por ele
    /// escreveria no catálogo de quem roda a suíte — o defeito que a fase 0
    /// encontrou. E ele não é hipotético aqui: enquanto este dock era escrito, a
    /// gravação **rodou uma vez contra o catálogo real**, porque a troca do
    /// caminho não tinha pegado no arquivo.
    pub fn montar_o_dock_em(
        &mut self,
        eu: &Entity<Self>,
        arquivo: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arquivo_para_ler = arquivo.clone();
        self.arranjo_em = arquivo;
        let dock = cx.new(|cx| DockArea::new("revelacao", Some(arranjo::VERSAO), window, cx));
        let fraca = dock.downgrade();

        for qual in Qual::TODOS {
            let revelacao = eu.downgrade();
            register_panel(
                cx,
                qual.nome(),
                move |_dock, _estado, _info, _window, cx| {
                    let revelacao = revelacao.clone();
                    Box::new(cx.new(|cx| PainelDaRevelacao::novo(qual, revelacao, cx)))
                },
            );
        }

        let painel = |qual: Qual, eu: &Entity<Self>, cx: &mut gpui::App| {
            let revelacao = eu.downgrade();
            let entidade = cx.new(|cx| PainelDaRevelacao::novo(qual, revelacao, cx));
            std::sync::Arc::new(entidade) as std::sync::Arc<dyn PanelView>
        };

        let presets = DockItem::tabs(vec![painel(Qual::Presets, eu, cx)], &fraca, window, cx);
        let palco = DockItem::tabs(vec![painel(Qual::Palco, eu, cx)], &fraca, window, cx);
        let graficos = DockItem::tabs(vec![painel(Qual::Graficos, eu, cx)], &fraca, window, cx);
        let ajustes = DockItem::tabs(vec![painel(Qual::Ajustes, eu, cx)], &fraca, window, cx);
        let filmstrip = DockItem::tabs(vec![painel(Qual::Filmstrip, eu, cx)], &fraca, window, cx);

        // A direita é uma coluna: os gráficos em cima, os ajustes embaixo — a
        // proporção do legado (20% / 80%), que é o que deixa os 42 controles
        // com espaço para rolar.
        let direita = DockItem::split_with_sizes(
            gpui::Axis::Vertical,
            vec![graficos, ajustes],
            vec![Some(px(ALTURA_DOS_GRAFICOS)), None],
            &fraca,
            window,
            cx,
        );

        let meio = DockItem::split_with_sizes(
            gpui::Axis::Vertical,
            vec![palco, filmstrip],
            vec![None, Some(px(ALTURA_DO_FILMSTRIP))],
            &fraca,
            window,
            cx,
        );

        let centro = DockItem::split_with_sizes(
            gpui::Axis::Horizontal,
            vec![presets, meio, direita],
            vec![Some(px(LADO_DOS_PRESETS)), None, Some(px(LADO_DO_PAINEL))],
            &fraca,
            window,
            cx,
        );

        dock.update(cx, |area, cx| {
            area.set_center(centro, window, cx);

            if let Some(salvo) = arranjo::ler_de(&arquivo_para_ler) {
                if let Err(erro) = area.load(salvo, window, cx) {
                    eprintln!("⚠️  Arranjo salvo da Revelação não pôde ser restaurado: {erro}");
                }
            }
        });

        let assinatura = cx.subscribe_in(
            &dock,
            window,
            |tela: &mut Self, area, evento: &DockEvent, _window, cx| {
                if !matches!(evento, DockEvent::LayoutChanged) {
                    return;
                }
                let area = area.clone();
                // A mesma espera de 500 ms da Biblioteca e dos ajustes: um
                // arrasto de divisória emite dezenas de eventos por segundo.
                tela._arranjo = Some(cx.spawn(async move |tela, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let _ = tela.update(cx, |tela, cx| {
                        arranjo::gravar_em(&tela.arranjo_em, &area.read(cx).dump(cx));
                    });
                }));
            },
        );

        self._assinaturas.push(assinatura);
        self.dock = Some(dock);
    }
}

impl Render for Revelacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .children(self.dock.clone())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use domain::entities::preset::PresetAdjustments;

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
        cx.update(gpui_component::init);
        let janela = cx.add_window(move |window, cx| {
            Revelacao::nova(previews, gravador, guarda, presets, window, cx)
        });

        // 🚨 **O dock é montado aqui também.** Sem esta linha os testes
        // desenhariam uma Revelação **sem painel nenhum** — a mesma armadilha
        // que os quinze testes da Biblioteca esconderam até o primeiro teste de
        // dock acusar.
        // 🚨 **Num arquivo descartável**, e não no caminho de verdade: montar lê
        // o arranjo salvo, e um teste que lesse o do catálogo real passaria a
        // depender da tela que o fotógrafo arrumou ontem.
        let arquivo = std::env::temp_dir().join(format!(
            "vlb-teste-arranjo-revelacao-{}.json",
            std::process::id()
        ));
        janela
            .update(cx, |tela, window, cx| {
                let eu = cx.entity();
                tela.montar_o_dock_em(&eu, arquivo.clone(), window, cx);
            })
            .expect("a janela deve estar aberta");

        janela
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

    /// 🚨 Os cinco painéis da Revelação voltam do arranjo gravado.
    ///
    /// A conferência é pelos painéis **vivos**, e não pelo retrato: o
    /// `InvalidPanel::dump` devolve o estado antigo com o nome original dentro,
    /// então comparar retratos passaria com o registro faltando inteiro. Foi o
    /// que o mesmo teste da Biblioteca mostrou primeiro.
    #[gpui::test]
    fn o_arranjo_da_revelacao_volta_com_os_cinco_paineis(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        fn vivos(item: &gpui_component::dock::DockItem, cx: &gpui::App) -> Vec<&'static str> {
            use gpui_component::dock::DockItem;
            match item {
                DockItem::Tabs { items, .. } => {
                    items.iter().map(|view| view.panel_name(cx)).collect()
                }
                DockItem::Split { items, .. } => {
                    items.iter().flat_map(|filho| vivos(filho, cx)).collect()
                }
                DockItem::Panel { view, .. } => vec![view.panel_name(cx)],
                DockItem::Tiles { .. } => Vec::new(),
            }
        }

        let esperados = vec![
            "revelacao:presets",
            "revelacao:palco",
            "revelacao:filmstrip",
            "revelacao:graficos",
            "revelacao:ajustes",
        ];

        let retrato = janela
            .update(cx, |tela, _window, cx| {
                let dock = tela.dock.as_ref().expect("o dock foi montado");
                assert_eq!(vivos(dock.read(cx).items(), cx), esperados);
                dock.read(cx).dump(cx)
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, window, cx| {
                let dock = tela.dock.as_ref().expect("o dock foi montado").clone();
                dock.update(cx, |area, cx| {
                    area.load(retrato, window, cx).expect("restaurar o arranjo");
                });

                assert_eq!(
                    vivos(dock.read(cx).items(), cx),
                    esperados,
                    "restaurar tem de reconstruir os painéis, e não InvalidPanel"
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
            "Warm",
            PresetAdjustments {
                temperature: Some(5.0),
                ..Default::default()
            },
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
    /// `PresetAdjustments` tem 15 dos 46 campos. Um preset aplicado sobre uma
    /// foto com HSL trabalhado não pode apagar o HSL — é o comportamento do
    /// legado (`apply_preset` escreve campo a campo, só o que é `Some`), e o
    /// contrário destruiria trabalho sem aviso.
    #[gpui::test]
    fn o_preset_nao_apaga_os_31_campos_que_ele_nao_tem(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let preset = Preset::system(
            "Cool",
            PresetAdjustments {
                temperature: Some(-5.0),
                ..Default::default()
            },
        );
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
        assert_eq!(salvos[0].0, "Retrato claro");
        assert_eq!(salvos[0].1.exposure, Some(1.5), "guarda o que está na tela");
        assert_eq!(
            salvos[0].1.contrast,
            Some(1.0),
            "os 15 vão inteiros, e não só os que diferem do neutro"
        );
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
