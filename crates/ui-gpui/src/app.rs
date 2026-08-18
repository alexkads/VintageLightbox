//! A raiz: a barra de navegação e qual tela está embaixo dela.
//!
//! Nasceu com a Revelação, porque até a fase 1 só havia uma tela e a janela
//! podia abrir a Biblioteca direto. A partir de duas, alguém precisa saber qual
//! está no ar — e esse alguém não pode ser nenhuma das duas.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use gpui::{actions, div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::acervo::Acervo;
use crate::biblioteca::colecoes::Colecoes;
use crate::biblioteca::marcacao::Marcador;
use crate::biblioteca::tela::Biblioteca;
use crate::cliente::{monitor_do_cliente, Cliente};
use crate::configuracoes::Configuracoes;
use crate::exportacao::porta::Exportador;
use crate::exportacao::tela::Exportacao;
use crate::importacao::explorador::{Explorador, GeradorDeMiniaturas, Importador, SeletorDePasta};
use crate::importacao::tela::{Importacao, Importou};
use crate::impressao::tela::Impressao;
use crate::revelacao::persistencia::{self, Gravador};
use crate::revelacao::presets::GuardaDePresets;
use crate::revelacao::processador::Ajustes;
use crate::revelacao::tela::Revelacao;

/// As portas para o mundo de fora, num pacote só.
///
/// 🔑 **Existe porque `Aplicativo::novo` chegou a dez argumentos.** Cinco deles
/// eram `Arc<dyn …>` posicionais, todos do mesmo naipe — a ordem entre eles não é
/// óbvia para ninguém, e trocar dois de lugar compila e falha em tempo de
/// execução, no primeiro clique.
pub struct Portas {
    pub gravador: Arc<dyn Gravador>,
    /// Quem sabe reler o catálogo depois que a importação o muda.
    pub acervo: Arc<dyn Acervo>,
    /// Quem grava os arquivos exportados.
    pub exportador: Arc<dyn Exportador>,
    /// As coleções — o ensaio do cliente mora numa.
    pub colecoes: Arc<dyn Colecoes>,
    /// Quem monta o PDF da folha e o entrega ao disco ou à impressora.
    pub folha: Arc<dyn crate::impressao::porta::Folha>,
    pub marcador: Arc<dyn Marcador>,
    pub gerador: Arc<dyn GeradorDeMiniaturas>,
    pub guarda_de_presets: Arc<dyn GuardaDePresets>,
    pub explorador: Arc<dyn Explorador>,
    pub importador: Arc<dyn Importador>,
    pub seletor: Arc<dyn SeletorDePasta>,
}

actions!(
    vintagelightbox,
    [
        VoltarParaBiblioteca,
        Desfazer,
        Refazer,
        AlternarCorte,
        CopiarRevelacao,
        ColarRevelacao,
        ApagarFotos,
        AlternarOriginal,
        // As treze teclas de triagem da Biblioteca, mais as duas setas. São
        // ações sem dado porque o `actions!` só declara struct de unidade — o
        // valor de cada uma está na tabela de ligações, logo abaixo, que é onde
        // ele fica legível ao lado da tecla.
        Adiante,
        Atras,
        SemNota,
        UmaEstrela,
        DuasEstrelas,
        TresEstrelas,
        QuatroEstrelas,
        CincoEstrelas,
        CorVermelha,
        CorAmarela,
        CorVerde,
        CorAzul,
        Escolher,
        Rejeitar,
        Desmarcar,
        SelecionarTudo,
        LimparSelecao
    ]
);

/// O contexto de teclado da raiz.
///
/// Nomeado porque o `Esc` **não pode** ser global: o campo de busca da
/// Biblioteca usa `Esc` para se limpar, e uma ligação sem contexto roubaria a
/// tecla dele.
const CONTEXTO: &str = "Aplicativo";

/// O contexto da raiz **com nenhum campo de texto no caminho do foco**.
///
/// 🚨 **Não é a mesma coisa que `CONTEXTO`, e a diferença custou a letra `r`.**
/// O GPUI procura ligação em **todos os prefixos** do caminho de foco
/// (`KeyBindingContextPredicate::depth_of`), então uma ligação no contexto da
/// raiz continua casando enquanto se digita num campo de texto lá dentro — e
/// tecla que casa vira ação, **não vira letra**. Com `r` ligado ao recorte,
/// digitar "retrato" na busca escrevia `etato`: as duas letras sumiam, sem erro,
/// sem aviso, e a suspeita cai no campo de busca.
///
/// O `!Input` é o contexto do `InputState` do `gpui-component`
/// (`input::CONTEXT`), e o `Not` do predicado varre a pilha **inteira** — é o que
/// faz a ligação desaparecer enquanto o campo tem o foco.
///
/// ⚠️ **Vale para toda tecla sem modificador.** `Cmd+Z` não precisa disto
/// (`cmd` não produz letra), e `Esc` também não: o campo tem ligação **própria**
/// para ele, e o GPUI prefere a mais profunda. É a tecla solta que compete com o
/// texto.
const SEM_CAMPO_DE_TEXTO: &str = "Aplicativo && !Input";

pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([
        gpui::KeyBinding::new("escape", VoltarParaBiblioteca, Some(CONTEXTO)),
        // As mesmas teclas do legado (`keyboard.rs`): `Cmd+Z` e `Cmd+Shift+Z`.
        //
        // ⚠️ **A ordem importa.** O GPUI casa a ligação mais específica primeiro,
        // mas as duas são declaradas aqui juntas de propósito: separá-las em
        // chamadas diferentes deixaria fácil alguém acrescentar um `cmd-z` depois
        // do `cmd-shift-z` e engolir o refazer — que é o tipo de coisa que só
        // aparece quando alguém tenta refazer.
        gpui::KeyBinding::new("cmd-shift-z", Refazer, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-z", Desfazer, Some(CONTEXTO)),
        // Copiar e colar revelação — as mesmas teclas do Lightroom.
        //
        // 🔑 **Levam `CONTEXTO` e não `SEM_CAMPO_DE_TEXTO`**, como o `Cmd+Z`:
        // com modificador não há disputa com quem está digitando, porque tecla
        // com `Cmd` não vira letra.
        gpui::KeyBinding::new("cmd-shift-c", CopiarRevelacao, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-shift-v", ColarRevelacao, Some(CONTEXTO)),
        // Apagar. 🚨 Vai em `SEM_CAMPO_DE_TEXTO` porque `Delete` e `Backspace`
        // apagam **letra** dentro de um campo de busca — e roubar a tecla de lá
        // faria digitar virar um pedido para tirar foto do catálogo.
        gpui::KeyBinding::new("delete", ApagarFotos, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("backspace", ApagarFotos, Some(SEM_CAMPO_DE_TEXTO)),
        // `R` de "recortar", a mesma tecla do legado (`keyboard.rs`).
        gpui::KeyBinding::new("r", AlternarCorte, Some(SEM_CAMPO_DE_TEXTO)),
        // `\` mostra o antes/depois, como no legado.
        gpui::KeyBinding::new("\\", AlternarOriginal, Some(SEM_CAMPO_DE_TEXTO)),
        // As teclas de triagem da Biblioteca, na tabela do `keyboard.rs` do
        // legado: setas para andar, `0`–`5` nota, `6`–`9` cor, `P`/`X`/`U`
        // sinalizador. **Todas** com `!Input`, porque todas são tecla solta —
        // sem isso, buscar `DSC_0512` daria nota 5, 1 e 2 em fotos diferentes
        // enquanto o número não aparecia no campo.
        gpui::KeyBinding::new("right", Adiante, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("left", Atras, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("0", SemNota, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("1", UmaEstrela, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("2", DuasEstrelas, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("3", TresEstrelas, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("4", QuatroEstrelas, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("5", CincoEstrelas, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("6", CorVermelha, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("7", CorAmarela, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("8", CorVerde, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("9", CorAzul, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("p", Escolher, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("x", Rejeitar, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("u", Desmarcar, Some(SEM_CAMPO_DE_TEXTO)),
        // `Cmd+A` e `Cmd+D`, da Biblioteca. Levam `CONTEXTO` e não
        // `SEM_CAMPO_DE_TEXTO`: com modificador não há disputa com o texto — e
        // o campo de busca tem o **próprio** `Cmd+A` (selecionar tudo no
        // campo), que o GPUI prefere por ser mais profundo. Ligá-los com
        // `!Input` tiraria o `Cmd+A` de dentro do campo sem ganhar nada.
        gpui::KeyBinding::new("cmd-a", SelecionarTudo, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-d", LimparSelecao, Some(CONTEXTO)),
    ]);
}

/// Os nomes das quatro cores, como o banco os guarda.
///
/// ⚠️ **Roxo não tem tecla, e no legado também não** — o `color_labels.rs` de lá
/// oferece cinco cores no menu e o `keyboard.rs` liga só quatro. Acrescentar a
/// quinta seria feature nova (§7.1), ainda que a tecla `0` esteja "livre": ela
/// é a nota zero.
const VERMELHO: &str = "Red";
const AMARELO: &str = "Yellow";
const VERDE: &str = "Green";
const AZUL: &str = "Blue";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    Biblioteca,
    Revelacao,
    Impressao,
}

pub struct Aplicativo {
    biblioteca: Entity<Biblioteca>,
    revelacao: Entity<Revelacao>,
    /// A folha de impressão. Guarda o leiaute entre uma visita e outra: papel,
    /// margem e modelo são escolhas sobre o papel, não sobre a foto.
    impressao: Entity<Impressao>,
    /// O modal de importação. **Sempre existe**, e só aparece quando aberto: ele
    /// guarda a listagem, e recriá-lo a cada abertura jogaria fora o que o
    /// fotógrafo já marcou ao fechar o modal por engano.
    importacao: Entity<Importacao>,
    importando: bool,
    /// O modal de exportação — o único caminho do app até um arquivo no disco.
    exportacao: Entity<Exportacao>,
    exportando: bool,
    /// As Configurações, no mesmo formato do modal de importação: elas são um
    /// lugar onde se entra e de onde se sai, e não uma quarta tela.
    configuracoes: Entity<Configuracoes>,
    configurando: bool,
    /// A segunda tela, quando aberta. É uma **janela**, e não uma tela desta —
    /// as duas existem ao mesmo tempo, em monitores diferentes.
    cliente: Option<gpui::WindowHandle<Cliente>>,
    /// O cache de previews, guardado para alimentar a segunda tela.
    previews: Arc<PreviewManager>,
    /// 🚨 A inscrição que mantém a segunda tela em dia. Descartada, ela para de
    /// acompanhar a seleção **sem erro nenhum** — a foto congela no que estava, e
    /// quem está do outro lado do monitor não tem como saber que congelou.
    _observador: gpui::Subscription,
    tela: Tela,
    /// A raiz precisa de foco próprio para as ações de teclado chegarem nela.
    /// Sem isto, `Esc` só funcionaria enquanto algum filho focável estivesse
    /// ativo — e a tela de Revelação não tem nenhum ainda.
    foco: FocusHandle,
    /// Quem relê o catálogo quando a importação termina.
    acervo: Arc<dyn Acervo>,
    /// Quem grava — aqui usado pelo colar, que escreve em N fotos de uma vez.
    gravador: Arc<dyn Gravador>,
    /// A revelação copiada, esperando ser colada.
    ///
    /// 🔑 **São os 46 ajustes, e não o corte.** É o que o Lightroom faz, e o
    /// motivo é forte: colar o enquadramento de uma foto em outras 40 move o
    /// assunto de todas elas para onde ele estava só na primeira. Cor se repete
    /// numa sessão; composição não.
    area_de_transferencia: Option<Ajustes>,
    /// O canal por onde o acervo relido volta. A releitura é assíncrona: o
    /// `LibraryController` é `async` do tokio, e o GPUI não roda futuros dele.
    releituras: (Sender<Vec<PhotoViewModel>>, Receiver<Vec<PhotoViewModel>>),
    /// 🚨 A `Task` que espera a releitura chegar. **Descartá-la a cancela** — e
    /// o sintoma seria a grade nunca receber as fotos importadas, que é
    /// exatamente o defeito que esta ligação existe para consertar.
    _releitura: Option<gpui::Task<()>>,
    /// 🚨 A inscrição no fim da importação. Sem ela nada acusa: o lote entra no
    /// banco, o modal conta as fotos, e a grade continua vazia.
    _fim_da_importacao: gpui::Subscription,
}

impl Aplicativo {
    pub fn novo(
        fotos: Vec<PhotoViewModel>,
        previews: Arc<PreviewManager>,
        presets: Vec<Preset>,
        portas: Portas,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // O mesmo cache de previews da Biblioteca: a importação grava miniatura
        // com a chave `import::` na mesma tabela, e dois `PreviewManager` para o
        // mesmo arquivo seriam dois caches do mesmo lugar.
        let previews_para_importar = previews.clone();
        let previews_para_imprimir = previews.clone();
        let previews_do_cliente = previews.clone();
        let previews_das_configuracoes = previews.clone();
        // O mesmo seletor nativo da importação: escolher pasta é interação com
        // o sistema, e dois seletores seriam duas janelas do SO para a mesma
        // pergunta.
        let seletor_para_exportar = portas.seletor.clone();
        // O mesmo gravador da Revelação: colar escreve pelo caminho que já
        // existe, e dois gravadores seriam duas esperas de 500 ms sobre a mesma
        // foto.
        let gravador_para_colar = portas.gravador.clone();
        let seletor_para_imprimir = portas.seletor.clone();
        let biblioteca = cx.new(|cx| {
            Biblioteca::nova(
                fotos,
                previews.clone(),
                portas.marcador,
                portas.colecoes,
                window,
                cx,
            )
        });
        // 🚨 O dock é montado **depois** da entidade existir: os quatro painéis
        // guardam uma referência fraca a ela, e dentro do construtor ela ainda
        // não foi entregue ao `cx`.
        biblioteca.update(cx, |tela, cx| {
            let eu = cx.entity();
            tela.montar_o_dock(&eu, window, cx);
        });
        let revelacao = cx.new(|cx| {
            Revelacao::nova(
                previews,
                portas.gravador,
                portas.guarda_de_presets,
                presets,
                window,
                cx,
            )
        });

        revelacao.update(cx, |tela, cx| {
            let eu = cx.entity();
            tela.montar_o_dock(&eu, window, cx);
        });

        // 🚨 **`track_focus` rastreia; ele não dá foco.** Enquanto ninguém focou a
        // raiz, o caminho de foco fica vazio e **nenhuma ação de teclado dela é
        // alcançada** — o `Esc` da Revelação nunca funcionou, e o commit que o
        // trouxe deu por pronto porque o teste chamava `voltar_para_biblioteca`
        // direto, nunca a tecla. Tecla que não casa não falha: ela não faz nada.
        //
        // ⚠️ **Nenhum teste exige esta linha aqui**, e é de propósito: as teclas
        // da raiz só valem dentro da Revelação, e `revelar` refoca. Ela fica
        // porque a primeira tecla que a **Biblioteca** ganhar (as setas da grade,
        // no legado) nasceria morta sem foco desde a abertura — e o sintoma seria
        // idêntico ao que acabou de custar dois commits para aparecer.
        let foco = cx.focus_handle();
        window.focus(&foco);

        // A segunda tela acompanha a seleção da Biblioteca. `observe` dispara a
        // cada `notify` dela — que é exatamente quando a seleção pode ter mudado.
        let observador = cx.observe(&biblioteca, |raiz, biblioteca, cx| {
            if raiz.cliente.is_none() {
                return;
            }
            if let Some(foto) = biblioteca.read(cx).foto_selecionada() {
                raiz.mostrar_ao_cliente(&foto, cx);
            }
        });

        let importacao = cx.new(|cx| {
            Importacao::nova(
                portas.explorador,
                portas.importador,
                portas.seletor,
                portas.gerador,
                previews_para_importar,
                cx,
            )
        });

        // 🚨 **O fio que faltava.** A importação grava no banco e a Biblioteca
        // carrega a lista uma vez, antes de a janela existir: sem esta
        // inscrição, o modal conta "65 importadas" e a grade atrás continua
        // exatamente como estava. Nada falha — e quem importou conclui que a
        // importação não funciona, com as 65 fotos já no banco e no disco.
        let fim_da_importacao = cx.subscribe(&importacao, |raiz, _, _: &Importou, cx| {
            raiz.reler_o_acervo(cx);
        });

        Self {
            biblioteca,
            revelacao,
            impressao: cx.new(|cx| {
                Impressao::nova(
                    previews_para_imprimir,
                    portas.folha,
                    seletor_para_imprimir,
                    window,
                    cx,
                )
            }),
            importacao,
            importando: false,
            exportacao: cx.new(|_| Exportacao::nova(portas.exportador, seletor_para_exportar)),
            exportando: false,
            configuracoes: cx.new(|_| Configuracoes::nova(previews_das_configuracoes)),
            configurando: false,
            cliente: None,
            previews: previews_do_cliente,
            _observador: observador,
            tela: Tela::Biblioteca,
            foco,
            acervo: portas.acervo,
            gravador: gravador_para_colar,
            area_de_transferencia: None,
            releituras: channel(),
            _releitura: None,
            _fim_da_importacao: fim_da_importacao,
        }
    }

    /// Pede o catálogo de novo e entrega à Biblioteca quando ele chegar.
    ///
    /// ⚠️ **A espera é um laço curto, e ele acaba.** Um laço eterno acordaria a
    /// cada 100ms pelo resto da sessão; este morre na primeira resposta, e
    /// desiste depois de 30s — uma releitura que não volta deixa a grade como
    /// estava, que é o pior desfecho aceitável (ver [`Acervo::recarregar`]).
    fn reler_o_acervo(&mut self, cx: &mut Context<Self>) {
        self.acervo.recarregar(self.releituras.0.clone());

        self._releitura = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..300 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(chegou) = raiz.update(cx, |raiz, cx| {
                    let Ok(fotos) = raiz.releituras.1.try_recv() else {
                        return false;
                    };
                    raiz.biblioteca
                        .update(cx, |tela, cx| tela.trocar_acervo(fotos, cx));
                    cx.notify();
                    true
                }) else {
                    return;
                };
                if chegou {
                    return;
                }
            }
        }));
    }

    pub fn tela(&self) -> Tela {
        self.tela
    }

    /// Leva a foto selecionada na Biblioteca para a Revelação.
    ///
    /// A seleção é **copiada**, e não compartilhada: as duas telas escolhem
    /// fotos por razões diferentes — na Biblioteca escolher é comparar, na
    /// Revelação é editar. Um estado só faria mudar de foto na grade trocar,
    /// silenciosamente, a foto que está sendo editada.
    ///
    /// Sem seleção não faz nada, e o botão que chama isto fica desligado — a
    /// Revelação vazia não responde nenhuma pergunta.
    pub fn revelar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(foto) = self.biblioteca.read(cx).foto_selecionada() else {
            return;
        };
        // A lista que a grade estava mostrando vai junto: é ela que as setas e o
        // filmstrip da Revelação percorrem. A posição é a da foto escolhida
        // **dentro dela** — e não o índice no acervo, que com filtro ativo
        // apontaria para outra foto.
        let acervo = self.biblioteca.read(cx).fotos_visiveis();
        let posicao = acervo
            .iter()
            .position(|outra| outra.id == foto.id)
            .unwrap_or(0);

        self.revelacao.update(cx, |tela, cx| {
            tela.abrir_no_acervo(acervo, posicao, window, cx)
        });
        self.tela = Tela::Revelacao;
        // 🚨 O foco volta para a raiz a cada troca de tela, e não só na abertura.
        // Quem usou o campo de busca deixou o foco **nele** — e ele para de ser
        // renderizado ao entrar na Revelação. O caminho de foco fica apontando um
        // elemento que não está mais na tela, e as teclas da raiz somem: buscar
        // uma foto antes de revelar desligaria o `Cmd+Z`, sem nenhuma pista da
        // relação entre as duas coisas.
        window.focus(&self.foco);
        cx.notify();
    }

    /// Leva para a folha de impressão o que a Biblioteca está mostrando.
    ///
    /// 🔑 **O acervo que vai é o filtrado, e a foto selecionada é quem começa a
    /// coleção** — as duas coisas são o que o legado faz ao entrar no módulo de
    /// impressão. Sem seleção o botão fica desligado, como o da Revelação: uma
    /// folha vazia não responde nenhuma pergunta, e os botões de coleção lá
    /// dentro só valem depois de a tela existir.
    pub fn imprimir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let escolhidas = self.biblioteca.read(cx).ids_selecionados();
        if escolhidas.is_empty() {
            return;
        }
        let acervo = self.biblioteca.read(cx).fotos_visiveis();

        self.impressao
            .update(cx, |tela, cx| tela.abrir(acervo, escolhidas, cx));
        self.tela = Tela::Impressao;
        // O mesmo motivo da Revelação: o foco pode ter ficado no campo de busca,
        // que para de ser renderizado aqui — e as teclas da raiz sumiriam.
        window.focus(&self.foco);
        cx.notify();
    }

    /// Sair da Revelação **grava o que estiver pendente**.
    ///
    /// 🚨 Sem isto, arrastar um slider e apertar `Esc` dentro dos 500 ms de
    /// espera perderia o ajuste: a tela sai, a espera continua contando, e quem
    /// olha a Biblioteca não tem como saber que a última coisa que fez não foi
    /// guardada. É a terceira porta — as outras duas são a própria espera e a
    /// troca de foto.
    pub fn voltar_para_biblioteca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());
        self.tela = Tela::Biblioteca;
        window.focus(&self.foco);
        cx.notify();
    }

    /// Abre ou fecha a segunda tela — a janela que se vira para o cliente.
    ///
    /// 🔑 **É uma janela de verdade, e não um painel da principal.** Ela mora no
    /// outro monitor, sem barra de título e sem controle nenhum: o que o cliente
    /// vê é a foto, e nada mais. `Esc` dentro dela fecha; o botão daqui também.
    ///
    /// ⚠️ **Sem seleção não abre.** Uma segunda tela preta não diz ao cliente
    /// que nada foi escolhido — diz que o programa quebrou.
    pub fn alternar_cliente(&mut self, cx: &mut Context<Self>) {
        if let Some(janela) = self.cliente.take() {
            // Fechar é remover a janela. Se ela já não existe (o `Esc` de dentro
            // dela chegou primeiro), o `update` devolve erro e não há o que
            // fazer além de esquecer o handle — que é o que o `take` já fez.
            let _ = janela.update(cx, |_cliente, window, _cx| window.remove_window());
            return;
        }

        let Some(foto) = self.biblioteca.read(cx).foto_selecionada() else {
            return;
        };

        let telas: Vec<gpui::DisplayId> = cx.displays().iter().map(|tela| tela.id()).collect();
        let principal = cx.primary_display().map(|tela| tela.id());
        let Some(escolhida) = monitor_do_cliente(&telas, principal) else {
            return;
        };

        // A janela nasce em tela cheia, no monitor escolhido, sem barra de
        // título e sem poder ser movida: quem está do outro lado dela não tem
        // por que poder arrastá-la, e um título escrito "VintageLightbox" sobre
        // a foto é exatamente o que uma apresentação não quer.
        let opcoes = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Fullscreen(
                cx.displays()
                    .iter()
                    .find(|tela| tela.id() == escolhida)
                    .map(|tela| tela.bounds())
                    .unwrap_or_default(),
            )),
            display_id: Some(escolhida),
            titlebar: None,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };

        match cx.open_window(opcoes, |window, cx| cx.new(|cx| Cliente::novo(window, cx))) {
            Ok(janela) => {
                self.cliente = Some(janela);
                self.mostrar_ao_cliente(&foto, cx);
            }
            // Abrir janela é pedido ao sistema, e ele pode recusar. Sem monitor
            // não há segunda tela — e derrubar o app por causa disso seria trocar
            // "o botão não fez nada" por "perdi a triagem inteira".
            Err(erro) => eprintln!("⚠️  Não foi possível abrir a segunda tela: {erro}"),
        }
        cx.notify();
    }

    pub fn cliente_aberto(&self) -> bool {
        self.cliente.is_some()
    }

    /// Manda para a segunda tela a foto que está selecionada aqui.
    ///
    /// ⚠️ **A imagem é lida e decodificada na thread da interface**, como na
    /// abertura da Revelação: é um JPEG de preview, de poucos milissegundos. É a
    /// mesma pendência que a fase 1 deixou, e ela vale para os dois lugares.
    fn mostrar_ao_cliente(&mut self, foto: &PhotoViewModel, cx: &mut Context<Self>) {
        let Some(janela) = self.cliente.as_ref() else {
            return;
        };

        let imagem = self
            .previews
            .get_preview(&foto.id)
            .or_else(|| self.previews.get_thumbnail(&foto.id))
            .map(crate::imagem::para_gpui);

        let foto = foto.clone();
        // 🚨 O `update` falha quando a janela **já foi fechada** — pelo `Esc` de
        // dentro dela, que a raiz não tem como saber que aconteceu. Aqui é onde
        // isso é descoberto, e o handle morto é jogado fora; sem isto o botão da
        // barra continuaria dizendo "fechar" para uma janela que não existe.
        let viva = janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto), imagem, cx);
            })
            .is_ok();

        if !viva {
            self.cliente = None;
            cx.notify();
        }
    }

    /// Abre as Configurações, relendo o cache na hora.
    ///
    /// 🔑 **O número é lido ao abrir, e não guardado**: entre uma abertura e
    /// outra o cache cresce a cada foto revelada, e mostrar o retrato de ontem
    /// faria o "Limpar tudo" prometer um espaço que não é o que vai sair.
    pub fn abrir_configuracoes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.configurando = true;
        self.configuracoes.update(cx, |tela, cx| tela.atualizar(cx));
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn fechar_configuracoes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.configurando = false;
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn configurando(&self) -> bool {
        self.configurando
    }

    /// Abre o modal de importação sobre a Biblioteca.
    pub fn importar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.importando = true;
        // Cartões e recentes são pedidos a cada abertura: um cartão plugado
        // depois de o app subir não apareceria numa lista buscada uma vez só.
        self.importacao
            .update(cx, |tela, cx| tela.pedir_origens(cx));
        // 🚨 O foco vai para o modal: as cinco teclas dele (`Enter`, `espaço`,
        // `⌘A`, setas) só chegam a quem está focado, e `track_focus` rastreia sem
        // conceder. Foi o que deixou o `Esc` da Revelação morto por dois commits.
        self.importacao.read(cx).focar(window);
        cx.notify();
    }

    /// Copia a revelação da foto selecionada.
    ///
    /// 🔑 **Lê o que está gravado na foto, e não o que a Revelação tem na tela.**
    /// Assim copiar funciona da Biblioteca, sem abrir foto nenhuma — que é onde
    /// quem revela 800 fotos está quando decide repetir um ajuste.
    pub fn copiar_revelacao(&mut self, cx: &mut Context<Self>) {
        let Some(foto) = self.biblioteca.read(cx).foto_selecionada() else {
            return;
        };
        self.area_de_transferencia = Some(persistencia::da_foto(&foto));
        cx.notify();
    }

    /// Cola a revelação copiada em toda a seleção.
    ///
    /// 🚨 **Cada foto conserva o próprio corte.** `SavePhotoEditsUseCase` recebe
    /// os oito campos de corte e a entidade faz atribuição direta, sem mesclar:
    /// gravar sem reenviá-los **apaga o enquadramento**. Colar em 40 fotos
    /// apagaria o enquadramento de 40 — sem erro, sem aviso, e sem desfazer.
    pub fn colar_revelacao(&mut self, cx: &mut Context<Self>) {
        let Some(ajustes) = self.area_de_transferencia else {
            return;
        };

        let alvos = {
            let biblioteca = self.biblioteca.read(cx);
            let selecionadas = biblioteca.fotos_selecionadas();
            if selecionadas.is_empty() {
                biblioteca.foto_selecionada().into_iter().collect()
            } else {
                selecionadas
            }
        };
        if alvos.is_empty() {
            return;
        }

        for foto in &alvos {
            self.gravador
                .gravar(foto.id.clone(), ajustes, persistencia::corte_da_foto(foto));
        }

        // 🔑 O acervo em memória ficou velho: as `PhotoViewModel` da grade ainda
        // têm os ajustes anteriores, e abrir a Revelação numa foto colada
        // mostraria o estado de antes. Reler é o que existe para isso.
        self.reler_o_acervo(cx);
        cx.notify();
    }

    /// Se há revelação copiada — o que liga o "Colar" na barra.
    pub fn tem_revelacao_copiada(&self) -> bool {
        self.area_de_transferencia.is_some()
    }

    /// Abre a exportação com a seleção da Biblioteca.
    ///
    /// 🔑 **Leva `fotos_visiveis` quando não há seleção múltipla**, e a seleção
    /// quando há — é a mesma regra da Impressão. Exportar sem ter escolhido nada
    /// é o pedido mais provável de quem acabou de filtrar por ★★★★★.
    pub fn exportar(&mut self, cx: &mut Context<Self>) {
        let biblioteca = self.biblioteca.read(cx);
        let selecionadas = biblioteca.fotos_selecionadas();
        let fotos = if selecionadas.is_empty() {
            biblioteca.fotos_visiveis()
        } else {
            selecionadas
        };

        self.exportacao
            .update(cx, |tela, cx| tela.abrir_para(fotos, cx));
        self.exportando = true;
        cx.notify();
    }

    /// Fecha o modal. ⚠️ **Não cancela o lote em curso** — a `Task` de colheita
    /// vive na entidade da exportação, que continua existindo. Fechar por engano
    /// no meio de 400 fotos não pode interromper as 400.
    pub fn fechar_exportacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.exportando = false;
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn exportando(&self) -> bool {
        self.exportando
    }

    /// Fecha o modal, **sem** jogar a listagem fora.
    ///
    /// 🔑 Quem fecha por engano depois de marcar 300 fotos de um cartão não pode
    /// perder a marcação. A listagem só se perde ao escolher outra origem.
    pub fn fechar_importacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.importando = false;
        // E o foco volta para a raiz, senão `Esc` e `Cmd+Z` param de funcionar
        // depois de a importação fechar — o mesmo buraco do campo de busca.
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn importando(&self) -> bool {
        self.importando
    }

    /// `Cmd+Z` e `Cmd+Shift+Z` só existem dentro da Revelação.
    ///
    /// 🔑 As ações moram no contexto da raiz, e não no da Revelação, porque a
    /// Revelação **não tem foco próprio** — a raiz é quem carrega o
    /// `FocusHandle`. Quando ela ganhar um (o crop overlay vai precisar), as duas
    /// ligações mudam de contexto junto e esta guarda some.
    ///
    /// ⚠️ Na Biblioteca elas não fazem nada, e é decisão: `Cmd+Z` ali seria
    /// "desfazer a última nota/sinalizador", que o legado não tem. Fazer com que
    /// desfizesse a revelação de uma foto que nem está na tela seria pior do que
    /// não fazer nada.
    fn ao_apagar_fotos(
        &mut self,
        _acao: &ApagarFotos,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // ⚠️ Só na Biblioteca. Na Revelação a tecla apagaria a foto que está
        // sendo revelada, deixando a tela com uma foto que já não existe.
        if self.tela != Tela::Biblioteca {
            return;
        }
        self.biblioteca
            .update(cx, |tela, cx| tela.pedir_para_apagar(cx));
    }

    fn ao_copiar_revelacao(
        &mut self,
        _acao: &CopiarRevelacao,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copiar_revelacao(cx);
    }

    fn ao_colar_revelacao(
        &mut self,
        _acao: &ColarRevelacao,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.colar_revelacao(cx);
    }

    fn ao_desfazer(&mut self, _acao: &Desfazer, window: &mut Window, cx: &mut Context<Self>) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.desfazer(window, cx));
        }
    }

    fn ao_refazer(&mut self, _acao: &Refazer, window: &mut Window, cx: &mut Context<Self>) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.refazer(window, cx));
        }
    }

    /// `R` entra e sai do modo de corte, e só dentro da Revelação.
    fn ao_alternar_corte(
        &mut self,
        _acao: &AlternarCorte,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.alternar_corte(window, cx));
        }
    }

    /// `\` alterna entre a foto revelada e a original.
    fn ao_alternar_original(
        &mut self,
        _acao: &AlternarOriginal,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.alternar_original(cx));
        }
    }

    /// As quinze teclas de triagem só valem na Biblioteca.
    ///
    /// 🔑 **Elas moram na raiz, e não na Biblioteca**, pela mesma razão que o
    /// `Cmd+Z` da Revelação: quem tem o `FocusHandle` é a raiz, e ação só é
    /// alcançada em quem está no **caminho do foco**. Uma ligação declarada no
    /// contexto da Biblioteca nasceria morta — o caminho vai da raiz até o nó
    /// focado, e a Biblioteca é filha dele, não ancestral.
    ///
    /// ⚠️ **Na Revelação elas não fazem nada, e é decisão.** O legado tria de lá
    /// também (`get_target_photos` aceita `Develop`), mas a Revelação nova não
    /// tem filmstrip nem seleção própria de acervo — dar nota lá exigiria
    /// escolher em qual foto, e a resposta certa depende de uma tela que ainda
    /// não existe. Fica registrado como pendente da fase 4.
    fn na_biblioteca(
        &mut self,
        cx: &mut Context<Self>,
        acao: impl FnOnce(&mut Biblioteca, &mut Context<Biblioteca>),
    ) {
        if self.tela != Tela::Biblioteca {
            return;
        }
        self.biblioteca.update(cx, |tela, cx| acao(tela, cx));
    }

    /// Um passo na tela que está no ar.
    fn andar(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        match self.tela {
            Tela::Biblioteca => self.biblioteca.update(cx, |tela, cx| tela.andar(passo, cx)),
            Tela::Revelacao => self
                .revelacao
                .update(cx, |tela, cx| tela.andar(passo, window, cx)),
            // Na Impressão as setas não andam: quem escolhe ali é a faixa de
            // baixo, e "a próxima" não quer dizer nada sobre uma folha.
            Tela::Impressao => {}
        }
    }

    fn ao_voltar(
        &mut self,
        _acao: &VoltarParaBiblioteca,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Só volta se há de onde voltar. Sem esta guarda, `Esc` na Biblioteca
        // seria uma tecla que consome o evento e não faz nada — e o próximo
        // atalho que quisesse `Esc` ali nasceria quebrado.
        //
        // ⚠️ **O `Esc` sai da Impressão também, e no legado não sai** — lá a
        // condição é `current_view == CurrentView::Develop`, e do módulo de
        // impressão só se sai clicando em "Library". A alternativa a esta linha
        // é uma tecla que responde numa tela e emudece na outra, que é mais
        // caro de aprender do que qualquer uma das duas regras inteiras.
        if self.tela != Tela::Biblioteca {
            self.voltar_para_biblioteca(window, cx);
        }
    }

    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Paridade com o app de egui: o botão de Revelação só liga quando há
        // seleção na Biblioteca ("Develop button enabled if Library has a
        // selection", `app.rs`).
        let tem_selecao = self.biblioteca.read(cx).foto_selecionada().is_some();
        // Exportar não exige seleção: com a grade filtrada e nada marcado, o
        // pedido natural é "exporte o que estou vendo".
        let tem_o_que_exportar = !self.biblioteca.read(cx).fotos_visiveis().is_empty();
        let na_revelacao = self.tela == Tela::Revelacao;
        let na_impressao = self.tela == Tela::Impressao;

        let titulo: SharedString = match (na_revelacao, self.revelacao.read(cx).foto()) {
            (true, Some(foto)) => foto.name.clone().into(),
            _ => "VintageLightbox".into(),
        };

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("nav-biblioteca")
                    .label("Biblioteca")
                    .xsmall()
                    .when(self.tela == Tela::Biblioteca, |b| b.primary())
                    .selected(self.tela == Tela::Biblioteca)
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.voltar_para_biblioteca(window, cx);
                    })),
            )
            .child(
                Button::new("nav-revelacao")
                    .label("Revelação")
                    .xsmall()
                    .when(na_revelacao, |b| b.primary())
                    .selected(na_revelacao)
                    .disabled(!tem_selecao)
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.revelar(window, cx);
                    })),
            )
            .child(
                // Paridade: no legado o botão de impressão também só liga com
                // seleção (`selected_photo_ids` ou a foto da Biblioteca).
                Button::new("nav-impressao")
                    .label("Impressão")
                    .xsmall()
                    .when(na_impressao, |b| b.primary())
                    .selected(na_impressao)
                    .disabled(!tem_selecao)
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.imprimir(window, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(titulo),
            )
            .child(
                // A segunda tela. Só liga com seleção, como a Revelação e a
                // Impressão: mostrar preto ao cliente não diz "não escolhi
                // nada", diz "quebrou".
                Button::new("nav-cliente")
                    .label("Segunda tela")
                    .xsmall()
                    .when(self.cliente.is_some(), |b| b.primary())
                    .selected(self.cliente.is_some())
                    .disabled(!tem_selecao)
                    .on_click(cx.listener(|este, _ev, _window, cx| {
                        este.alternar_cliente(cx);
                    })),
            )
            .child(
                // Copiar e colar revelação. 🔑 **Têm botão além da tecla** porque
                // são a resposta a "acabei de acertar esta foto e quero as
                // outras 40 iguais" — e quem acabou de acertar está com o
                // ponteiro na tela, não com a mão no `Cmd`.
                Button::new("nav-copiar-revelacao")
                    .label("Copiar")
                    .xsmall()
                    .disabled(!tem_selecao)
                    .on_click(cx.listener(|este, _ev, _window, cx| {
                        este.copiar_revelacao(cx);
                    })),
            )
            .child(
                Button::new("nav-colar-revelacao")
                    .label("Colar")
                    .xsmall()
                    .disabled(!self.tem_revelacao_copiada() || !tem_selecao)
                    .on_click(cx.listener(|este, _ev, _window, cx| {
                        este.colar_revelacao(cx);
                    })),
            )
            .child(
                // 🚨 O primeiro caminho que este app teve até um arquivo no
                // disco. Liga com seleção **ou** com grade não vazia: exportar
                // o que se está vendo é o pedido de quem acabou de filtrar.
                Button::new("nav-exportar")
                    .label("Exportar")
                    .xsmall()
                    .when(self.exportando, |b| b.primary())
                    .selected(self.exportando)
                    .disabled(!tem_o_que_exportar)
                    .on_click(cx.listener(|este, _ev, _window, cx| {
                        este.exportar(cx);
                    })),
            )
            .child(
                Button::new("nav-importar")
                    .label("Importar")
                    .xsmall()
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.importar(window, cx);
                    })),
            )
            .child(
                Button::new("nav-configuracoes")
                    .label("Configurações")
                    .xsmall()
                    .when(self.configurando, |b| b.primary())
                    .selected(self.configurando)
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        if este.configurando {
                            este.fechar_configuracoes(window, cx);
                        } else {
                            este.abrir_configuracoes(window, cx);
                        }
                    })),
            )
    }

    /// As Configurações, no mesmo véu do modal de importação.
    fn modal_de_configuracoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000099))
            .child(
                div()
                    .w(px(520.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(6.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(12.))
                            .py(px(6.))
                            .bg(cx.theme().title_bar)
                            .child(div().text_xs().child("Configurações"))
                            .child(
                                Button::new("fechar-configuracoes")
                                    .label("Fechar")
                                    .xsmall()
                                    .on_click(cx.listener(|este, _ev, window, cx| {
                                        este.fechar_configuracoes(window, cx);
                                    })),
                            ),
                    )
                    .child(self.configuracoes.clone()),
            )
    }

    /// O modal por cima de tudo, com um véu que escurece o que ficou atrás.
    ///
    /// ⚠️ **O véu não é enfeite**: ele diz que o que está atrás não responde. Sem
    /// ele, clicar numa foto da Biblioteca durante a importação pareceria
    /// funcionar e não faria nada.
    /// O modal de exportação. Menor que o de importação de propósito: a
    /// escolha inteira é uma pasta, e uma janela grande em volta de dois botões
    /// sugere que falta preencher alguma coisa.
    /// O aviso de apagar — o segundo aviso deste app sobre algo que não volta.
    ///
    /// 🚨 **O que ele precisa dizer, e diz**: quantas fotos, que o **arquivo
    /// continua no disco**, e que **a revelação vai junto**. Sem a segunda
    /// frase, quem lê "apagar" imagina perda de arquivo e não clica; sem a
    /// terceira, clica achando que reimportar desfaz — e reimportar devolve o
    /// arquivo, não os 46 ajustes.
    fn aviso_de_apagar(&self, quantas: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let titulo = if quantas == 1 {
            "Tirar 1 foto do catálogo?".to_string()
        } else {
            format!("Tirar {quantas} fotos do catálogo?")
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x000000aa))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(16.))
                    .max_w(px(420.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(6.))
                    .child(div().text_sm().child(titulo))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "O arquivo continua no disco — sai só do catálogo. \
                                 ⚠️ A revelação vai junto: reimportar devolve a foto, \
                                 não os ajustes.",
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(
                                Button::new("apagar-cancelar")
                                    .label("Cancelar")
                                    .xsmall()
                                    .on_click(cx.listener(|este, _ev, _window, cx| {
                                        este.biblioteca
                                            .update(cx, |tela, cx| tela.cancelar_apagar(cx));
                                    })),
                            )
                            .child(
                                Button::new("apagar-confirmar")
                                    .label("Tirar do catálogo")
                                    .xsmall()
                                    .danger()
                                    .on_click(cx.listener(|este, _ev, _window, cx| {
                                        este.biblioteca
                                            .update(cx, |tela, cx| tela.apagar_confirmado(cx));
                                    })),
                            ),
                    ),
            )
    }

    fn modal_de_exportacao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000099))
            .child(
                div()
                    .max_w_full()
                    .max_h_full()
                    .flex()
                    .flex_col()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(6.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(24.))
                            .px(px(12.))
                            .py(px(6.))
                            .bg(cx.theme().title_bar)
                            .child(div().text_xs().child("Exportar fotos"))
                            .child(
                                Button::new("fechar-exportacao")
                                    .label("Fechar")
                                    .xsmall()
                                    .on_click(cx.listener(|este, _ev, window, cx| {
                                        este.fechar_exportacao(window, cx);
                                    })),
                            ),
                    )
                    .child(self.exportacao.clone()),
            )
    }

    fn modal_de_importacao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000099))
            .child(
                div()
                    .w(px(760.))
                    .h(px(560.))
                    .max_w_full()
                    .max_h_full()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(6.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(12.))
                            .py(px(6.))
                            .bg(cx.theme().title_bar)
                            .child(div().text_xs().child("Importar fotos"))
                            .child(
                                Button::new("fechar-importacao")
                                    .label("Fechar")
                                    .xsmall()
                                    .on_click(cx.listener(|este, _ev, window, cx| {
                                        este.fechar_importacao(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h(px(0.))
                            .child(self.importacao.clone()),
                    ),
            )
    }
}

impl Render for Aplicativo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(CONTEXTO)
            .relative()
            .track_focus(&self.foco)
            .on_action(cx.listener(Self::ao_voltar))
            .on_action(cx.listener(Self::ao_desfazer))
            .on_action(cx.listener(Self::ao_refazer))
            .on_action(cx.listener(Self::ao_alternar_corte))
            .on_action(cx.listener(Self::ao_alternar_original))
            .on_action(cx.listener(Self::ao_copiar_revelacao))
            .on_action(cx.listener(Self::ao_colar_revelacao))
            .on_action(cx.listener(Self::ao_apagar_fotos))
            // A tabela de triagem. Quinze linhas de uma linha: a regra de cada
            // uma está em `biblioteca::marcacao`, e aqui só se diz qual ação
            // chama qual método — como a tabela de `controles.rs` faz com os 42
            // sliders da Revelação.
            // 🔑 As setas valem nas **duas** telas, e cada uma anda na sua
            // lista: na Biblioteca a seleção da grade, na Revelação a foto que
            // está sendo revelada. É o que o legado faz (`navigate_library` e o
            // ramo `Develop` do mesmo bloco), e é o que permite revelar 200
            // fotos sem voltar à grade entre uma e outra.
            .on_action(cx.listener(|este, _: &Adiante, window, cx| {
                este.andar(1, window, cx);
            }))
            .on_action(cx.listener(|este, _: &Atras, window, cx| {
                este.andar(-1, window, cx);
            }))
            .on_action(cx.listener(|este, _: &SemNota, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(0, cx))
            }))
            .on_action(cx.listener(|este, _: &UmaEstrela, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(1, cx))
            }))
            .on_action(cx.listener(|este, _: &DuasEstrelas, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(2, cx))
            }))
            .on_action(cx.listener(|este, _: &TresEstrelas, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(3, cx))
            }))
            .on_action(cx.listener(|este, _: &QuatroEstrelas, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(4, cx))
            }))
            .on_action(cx.listener(|este, _: &CincoEstrelas, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_nota(5, cx))
            }))
            .on_action(cx.listener(|este, _: &CorVermelha, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_cor(VERMELHO, cx))
            }))
            .on_action(cx.listener(|este, _: &CorAmarela, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_cor(AMARELO, cx))
            }))
            .on_action(cx.listener(|este, _: &CorVerde, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_cor(VERDE, cx))
            }))
            .on_action(cx.listener(|este, _: &CorAzul, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.dar_cor(AZUL, cx))
            }))
            .on_action(cx.listener(|este, _: &Escolher, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.sinalizar(1, cx))
            }))
            .on_action(cx.listener(|este, _: &Rejeitar, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.sinalizar(-1, cx))
            }))
            .on_action(cx.listener(|este, _: &Desmarcar, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.sinalizar(0, cx))
            }))
            .on_action(cx.listener(|este, _: &SelecionarTudo, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.selecionar_tudo(cx))
            }))
            .on_action(cx.listener(|este, _: &LimparSelecao, _w, cx| {
                este.na_biblioteca(cx, |tela, cx| tela.limpar_selecao(cx))
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.barra(cx))
            .child(
                // `min_h(0)` no contêiner da tela: sem ele, o conteúdo rolável
                // de dentro empurra o pai e a rolagem nunca acontece. Foi a
                // mesma correção que a linha de pastas + grade precisou.
                div().flex().flex_1().min_h(px(0.)).child(match self.tela {
                    Tela::Biblioteca => self.biblioteca.clone().into_any_element(),
                    Tela::Revelacao => self.revelacao.clone().into_any_element(),
                    Tela::Impressao => self.impressao.clone().into_any_element(),
                }),
            )
            .when(self.importando, |raiz| {
                raiz.child(self.modal_de_importacao(cx))
            })
            .when(self.exportando, |raiz| {
                raiz.child(self.modal_de_exportacao(cx))
            })
            .when_some(
                self.biblioteca.read(cx).confirmando_apagar(),
                |raiz, quantas| raiz.child(self.aviso_de_apagar(quantas, cx)),
            )
            .when(false, |raiz| raiz)
            .when(self.configurando, |raiz| {
                raiz.child(self.modal_de_configuracoes(cx))
            })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use crate::biblioteca::acervo::mentira::AcervoDeMentira;
    use crate::biblioteca::colecoes::mentira::ColecoesDeMentira;
    use crate::biblioteca::marcacao::mentira::MarcadorDeMentira;
    use crate::biblioteca::marcacao::Marca;
    use crate::exportacao::porta::mentira::ExportadorDeMentira;
    use crate::exportacao::tela::Modo;
    use crate::importacao::explorador::mentira::{
        ExploradorDeMentira, GeradorDeMentira, ImportadorDeMentira, SeletorDeMentira,
    };
    use crate::impressao::porta::mentira::FolhaDeMentira;
    use crate::revelacao::persistencia::mentira::GravadorDeMentira;
    use crate::revelacao::presets::mentira::GuardaDeMentira;

    /// As portas de mentira, que é o que quase todo teste daqui quer.
    fn portas() -> Portas {
        Portas {
            gravador: Arc::new(GravadorDeMentira::default()),
            acervo: Arc::new(AcervoDeMentira::default()),
            exportador: Arc::new(ExportadorDeMentira::default()),
            colecoes: Arc::new(ColecoesDeMentira::default()),
            folha: Arc::new(FolhaDeMentira::default()),
            marcador: Arc::new(MarcadorDeMentira::default()),
            gerador: Arc::new(GeradorDeMentira::default()),
            guarda_de_presets: Arc::new(GuardaDeMentira::default()),
            explorador: Arc::new(ExploradorDeMentira::default()),
            importador: Arc::new(ImportadorDeMentira::default()),
            seletor: Arc::new(SeletorDeMentira::default()),
        }
    }

    fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (
            Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
            dir,
        )
    }

    fn foto_vermelha() -> DynamicImage {
        let mut img = RgbaImage::new(8, 8);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
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

    fn acervo() -> Vec<PhotoViewModel> {
        vec![foto("DSC_001.NEF"), foto("retrato.jpg")]
    }

    /// 🚨 Sem seleção, revelar não faz nada — e não troca de tela.
    ///
    /// O botão fica desligado, mas o método é público e o `Esc` já mostra que
    /// atalho chega antes de botão. Uma Revelação aberta sem foto seria uma tela
    /// vazia sem caminho de volta óbvio.
    #[gpui::test]
    fn revelar_sem_selecao_nao_troca_de_tela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.revelar(window, cx);
                assert_eq!(app.tela(), Tela::Biblioteca);
            })
            .expect("a janela deve estar aberta");
    }

    /// A seleção da Biblioteca chega à Revelação.
    #[gpui::test]
    fn revelar_leva_a_foto_selecionada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                assert_eq!(app.tela(), Tela::Revelacao);
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg")
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Sair da Revelação grava o ajuste que ainda estava esperando.
    ///
    /// A espera de 500 ms é uma janela de perda, e `Esc` cai bem no meio dela:
    /// arrastar um slider e voltar para a Biblioteca é uma sequência de dois
    /// segundos. Sem esta gravação, o último ajuste sumiria — e sem aviso, porque
    /// a Biblioteca não tem como mostrar o que não foi guardado.
    #[gpui::test]
    fn sair_da_revelacao_grava_o_que_estava_esperando(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        gravador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                // Um arrasto, e a volta imediata — sem passar a espera.
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 0.9, cx);
                });
                app.voltar_para_biblioteca(window, cx);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "o ajuste tinha de ser gravado na saída");
        assert_eq!(gravado[0].0, "id-retrato.jpg");
        assert_eq!(gravado[0].1.exposure, 0.9);
    }

    /// 🚨 A tecla `Esc` sai mesmo da Revelação.
    ///
    /// O commit que trouxe a Revelação deu isto como pronto, e o teste de lá
    /// chamava `voltar_para_biblioteca` direto — nunca a tecla.
    #[gpui::test]
    fn esc_sai_mesmo_da_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);
                assert_eq!(app.tela(), Tela::Revelacao);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("escape");

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.tela(),
                    Tela::Biblioteca,
                    "o Esc tem de sair da Revelação"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Buscar uma foto antes de revelar **não** desliga o `Cmd+Z`.
    ///
    /// O campo de busca fica com o foco de quem digitou nele, e ele para de ser
    /// renderizado ao entrar na Revelação: o caminho de foco passa a apontar um
    /// elemento que não está na tela, e nenhuma tecla da raiz chega. Nada falha —
    /// as teclas só param de funcionar, e a relação com "eu tinha buscado antes"
    /// é invisível.
    #[gpui::test]
    fn buscar_antes_de_revelar_nao_desliga_as_teclas(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca.update(cx, |tela, cx| {
                    tela.selecionar(Some(1), cx);
                    tela.focar_busca(window, cx);
                });
                app.revelar(window, cx);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("escape");

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.tela(),
                    Tela::Biblioteca,
                    "o foco ficou no campo de busca e as teclas da raiz sumiram"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `Cmd+Z` chega mesmo à Revelação.
    ///
    /// Não é o `desfazer` da tela que este teste mede — esse já tem os dele. É a
    /// ligação: ação registrada, contexto certo, foco no lugar. Um `KeyBinding`
    /// que não casa **não falha**: a tecla simplesmente não faz nada, e a
    /// suspeita cai na funcionalidade, não na ligação.
    #[gpui::test]
    fn cmd_z_chega_a_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 1.5, cx);
                });
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("cmd-z");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.revelacao.read(cx).ajustes().exposure,
                    0.0,
                    "o Cmd+Z tem de chegar à Revelação"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `R` abre e fecha o modo de corte.
    ///
    /// Terceira tecla ligada à raiz, e a primeira que **não** é um atalho de
    /// sistema: `R` sozinho é exatamente o tipo de ligação que um campo de texto
    /// engoliria. O teste aperta a tecla de verdade, como o do `Esc` e o do
    /// `Cmd+Z` — os dois que revelaram que a ligação não existia.
    #[gpui::test]
    fn a_tecla_r_abre_e_fecha_o_corte(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("r");

        janela
            .update(cx, |app, _window, cx| {
                assert!(app.revelacao.read(cx).cortando(), "R tem de abrir o corte");
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("r");

        janela
            .update(cx, |app, _window, cx| {
                assert!(!app.revelacao.read(cx).cortando(), "e fechar de volta");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Digitar `r` na busca escreve `r` — e não abre o recorte.
    ///
    /// O GPUI procura ligação em **todos os prefixos** do caminho de foco, então
    /// a ligação da raiz continuava casando com o campo de texto focado — e
    /// tecla que vira ação **não vira letra**. Digitar "retrato" escrevia
    /// `etato`, sem erro e sem aviso: as letras sumiam e a suspeita caía no campo
    /// de busca, que estava certo.
    ///
    /// ⚠️ **A janela deste teste é montada com o `Root` do `gpui-component`**,
    /// como a do `main.rs`. É a única forma de digitar de verdade: o campo
    /// procura o `Root` com um `expect` ao inserir texto, e sem ele o teste morre
    /// antes de responder qualquer coisa.
    #[gpui::test]
    fn digitar_r_na_busca_escreve_r(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let mut guardado: Option<Entity<Aplicativo>> = None;
        let janela = cx.add_window({
            let previews = previews.clone();
            let guardado = &mut guardado;
            move |window, cx| {
                let app = cx.new(|cx| {
                    Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
                });
                *guardado = Some(app.clone());
                gpui_component::Root::new(app, window, cx)
            }
        });
        let app = guardado.expect("o aplicativo tem de ter sido construído");

        janela
            .update(cx, |_raiz, window, cx| {
                app.update(cx, |app, cx| {
                    app.biblioteca
                        .update(cx, |tela, cx| tela.focar_busca(window, cx));
                });
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_input("retrato");

        janela
            .update(cx, |_raiz, _window, cx| {
                assert_eq!(
                    app.read(cx).biblioteca.read(cx).texto_da_busca(cx),
                    "retrato",
                    "o atalho de recorte estava comendo os `r` da busca"
                );
                assert!(
                    !app.read(cx).revelacao.read(cx).cortando(),
                    "e digitando não se abre o corte"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 As teclas de triagem chegam à Biblioteca — e gravam.
    ///
    /// Quinze ligações novas, e ligação que não casa **não falha**: a tecla
    /// simplesmente não faz nada, e a suspeita cai na funcionalidade. Este teste
    /// aperta as teclas de verdade e confere os dois lados: o que a grade passou
    /// a mostrar, e o que foi mandado ao banco.
    #[gpui::test]
    fn as_teclas_de_triagem_marcam_a_foto_selecionada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let marcador = Arc::new(MarcadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        marcador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        // Nota, cor, sinalizador — uma de cada família.
        visual.simulate_keystrokes("3 7 p");

        let marcado = marcador.marcado();
        assert_eq!(
            marcado,
            vec![
                ("id-DSC_001.NEF".to_string(), Marca::Nota(3)),
                (
                    "id-DSC_001.NEF".to_string(),
                    Marca::Cor(Some("Yellow".to_string()))
                ),
                ("id-DSC_001.NEF".to_string(), Marca::Sinalizador(1)),
            ]
        );

        janela
            .update(cx, |app, _window, cx| {
                let foto = app
                    .biblioteca
                    .read(cx)
                    .foto_selecionada()
                    .expect("a foto selecionada");
                // E a tela mostra o resultado **antes** do banco responder: numa
                // triagem se aperta tecla mais rápido do que um `UPDATE` volta.
                assert_eq!(foto.rating, 3);
                assert_eq!(foto.color_label.as_deref(), Some("Yellow"));
                assert_eq!(foto.flag, Some(1));
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A mesma cor duas vezes tira a cor — pela tecla, não pela função.
    ///
    /// A regra de alternância tem teste próprio em `marcacao.rs`; o que se
    /// confere aqui é que a tela **lê o valor atual** antes de decidir. Lendo o
    /// valor errado (o da foto errada, ou o de antes da primeira tecla), a
    /// segunda tecla marcaria de novo em vez de desmarcar.
    #[gpui::test]
    fn a_mesma_cor_duas_vezes_desmarca_pela_tecla(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let marcador = Arc::new(MarcadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        marcador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("8 8");

        let marcado = marcador.marcado();
        assert_eq!(marcado.len(), 2);
        assert_eq!(marcado[0].1, Marca::Cor(Some("Green".to_string())));
        assert_eq!(marcado[1].1, Marca::Cor(None), "a segunda tira a cor");
    }

    /// 🚨 As setas andam pela grade — e a primeira seta escolhe sem seleção.
    ///
    /// ⚠️ **Não dão a volta**, como no legado: chegar ao fim e continuar
    /// apertando fica no fim. Numa triagem longa, voltar ao começo sem aviso
    /// faria retrabalhar as primeiras fotos sem perceber.
    #[gpui::test]
    fn as_setas_andam_pela_grade_sem_dar_a_volta(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        let nome = |cx: &mut TestAppContext| {
            janela
                .update(cx, |app, _window, cx| {
                    app.biblioteca.read(cx).foto_selecionada().map(|f| f.name)
                })
                .expect("a janela deve estar aberta")
        };

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("right");
        assert_eq!(
            nome(cx).as_deref(),
            Some("DSC_001.NEF"),
            "sem seleção, a primeira seta escolhe a primeira foto"
        );

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("right right right");
        assert_eq!(
            nome(cx).as_deref(),
            Some("retrato.jpg"),
            "e para na última, sem dar a volta"
        );

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("left left");
        assert_eq!(nome(cx).as_deref(), Some("DSC_001.NEF"));
    }

    /// 🚨 As setas andam **dentro da Revelação**, na lista que veio da grade.
    ///
    /// É o que transforma revelar 200 fotos em 200 ajustes, e não em 400 trocas
    /// de tela. E a lista é a **filtrada**: "a próxima" quer dizer a próxima das
    /// que se estava vendo.
    #[gpui::test]
    fn as_setas_andam_pela_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-DSC_001.NEF", "id-retrato.jpg"] {
            previews
                .save_preview(id, &foto_vermelha())
                .expect("gravar preview");
        }
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.revelar(window, cx);
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("DSC_001.NEF")
                );
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("right");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg"),
                    "a seta tem de trocar a foto em revelação"
                );
            })
            .expect("a janela deve estar aberta");

        // E não dá a volta: a última continua sendo a última.
        visual.simulate_keystrokes("right");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg")
                );
                assert_eq!(app.revelacao.read(cx).posicao(), 1);
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("left");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("DSC_001.NEF")
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Andar na Revelação **grava o ajuste pendente da foto que sai**.
    ///
    /// A espera de 500 ms é uma janela de perda, e a seta cai bem no meio dela:
    /// arrastar um slider e apertar → é a sequência normal de quem revela em
    /// série. Sem esta gravação, a gravação atrasada sairia com os ajustes já
    /// substituídos — a revelação de uma foto gravada na outra.
    #[gpui::test]
    fn andar_na_revelacao_grava_a_foto_que_sai(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-DSC_001.NEF", "id-retrato.jpg"] {
            previews
                .save_preview(id, &foto_vermelha())
                .expect("gravar preview");
        }
        cx.update(gpui_component::init);
        cx.update(init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        gravador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.revelar(window, cx);
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 1.2, cx);
                });
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("right");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "a foto que saiu tinha de ser gravada");
        assert_eq!(gravado[0].0, "id-DSC_001.NEF");
        assert_eq!(gravado[0].1.exposure, 1.2);
    }

    /// 🚨 Abrir as Configurações **relê o cache**.
    ///
    /// Entre uma abertura e outra o cache cresce a cada foto revelada. Guardando
    /// o retrato da primeira vez, o "Limpar tudo" prometeria um espaço que não é
    /// o que vai sair — e o número na tela seria mais velho que a decisão que ele
    /// informa.
    #[gpui::test]
    fn abrir_as_configuracoes_rele_o_cache(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.abrir_configuracoes(window, cx);
                assert!(app.configurando());
                assert_eq!(
                    app.configuracoes
                        .read(cx)
                        .estatisticas()
                        .expect("leu o cache")
                        .thumbnail_count,
                    0
                );
            })
            .expect("a janela deve estar aberta");

        // O cache cresce enquanto a janela está aberta — como quando o fotógrafo
        // revela uma foto e volta às Configurações.
        previews
            .save_thumbnail("id-nova", &foto_vermelha())
            .expect("gravar miniatura");

        janela
            .update(cx, |app, window, cx| {
                app.fechar_configuracoes(window, cx);
                app.abrir_configuracoes(window, cx);

                assert_eq!(
                    app.configuracoes
                        .read(cx)
                        .estatisticas()
                        .expect("releu")
                        .thumbnail_count,
                    1,
                    "o número tem de ser o de agora, não o da primeira abertura"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A segunda tela abre, recebe a foto, e **acompanha a seleção**.
    ///
    /// A inscrição que faz isso é a peça que some sem avisar: descartada, a
    /// janela abre, mostra a primeira foto e congela ali. Do outro lado do
    /// monitor não há como saber que congelou — e quem tria continua achando que
    /// o cliente está vendo a foto da vez.
    #[gpui::test]
    fn a_segunda_tela_acompanha_a_selecao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-DSC_001.NEF", &foto_vermelha())
            .expect("gravar preview");
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.alternar_cliente(cx);
                assert!(app.cliente_aberto(), "com seleção, o botão abre a janela");
            })
            .expect("a janela deve estar aberta");

        let nome_no_cliente = |cx: &mut TestAppContext| {
            janela
                .update(cx, |app, _window, cx| {
                    app.cliente
                        .as_ref()
                        .and_then(|c| c.read(cx).ok().and_then(|c| c.foto_mostrada()))
                })
                .expect("a janela deve estar aberta")
        };

        assert_eq!(nome_no_cliente(cx).as_deref(), Some("DSC_001.NEF"));

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert_eq!(
            nome_no_cliente(cx).as_deref(),
            Some("retrato.jpg"),
            "trocar de foto na grade tem de trocar o que o cliente vê"
        );

        janela
            .update(cx, |app, _window, cx| {
                app.alternar_cliente(cx);
                assert!(!app.cliente_aberto(), "e o mesmo botão fecha");
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Sem seleção a segunda tela não abre.
    ///
    /// Uma janela preta virada para o cliente não diz "não escolhi nada"; diz
    /// que o programa quebrou.
    #[gpui::test]
    fn sem_selecao_a_segunda_tela_nao_abre(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, _window, cx| {
                app.alternar_cliente(cx);
                assert!(!app.cliente_aberto());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Cmd+A` e `Cmd+D` chegam à Biblioteca.
    ///
    /// E `Cmd+A` pega **o que está na grade**: com filtro ligado, o `select_all`
    /// do legado marca também as que não estão na tela, e a próxima tecla de nota
    /// cai em todas elas.
    #[gpui::test]
    fn cmd_a_e_cmd_d_selecionam_e_limpam(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("cmd-a");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.biblioteca.read(cx).quantas_selecionadas(),
                    2,
                    "as duas fotos do acervo de teste"
                );
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("cmd-d");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(app.biblioteca.read(cx).quantas_selecionadas(), 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Uma tecla de nota com várias selecionadas grava **todas**.
    ///
    /// É o que a seleção múltipla existe para fazer: triar em lote. Sem isso ela
    /// seria só um desenho diferente na grade.
    #[gpui::test]
    fn a_triagem_em_lote_grava_todas_as_selecionadas(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let marcador = Arc::new(MarcadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        marcador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("cmd-a 4");

        let marcado = marcador.marcado();
        assert_eq!(marcado.len(), 2, "uma gravação por foto selecionada");
        assert!(marcado.iter().all(|(_, marca)| *marca == Marca::Nota(4)));
    }

    /// 🚨 Digitar `5` na busca escreve `5` — não dá nota 5.
    ///
    /// O gêmeo do defeito do `r`, e a razão de as quinze ligações nascerem com
    /// `!Input`: buscar `DSC_0512` daria nota 5, depois 1, depois 2, em fotos
    /// diferentes, enquanto o número não aparecia no campo.
    #[gpui::test]
    fn digitar_numero_na_busca_nao_da_nota(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let marcador = Arc::new(MarcadorDeMentira::default());
        let mut guardado: Option<Entity<Aplicativo>> = None;
        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            let guardado = &mut guardado;
            move |window, cx| {
                let app = cx.new(|cx| {
                    Aplicativo::novo(
                        acervo(),
                        previews,
                        Vec::new(),
                        Portas {
                            marcador,
                            ..portas()
                        },
                        window,
                        cx,
                    )
                });
                *guardado = Some(app.clone());
                gpui_component::Root::new(app, window, cx)
            }
        });
        let app = guardado.expect("o aplicativo tem de ter sido construído");

        janela
            .update(cx, |_raiz, window, cx| {
                app.update(cx, |app, cx| {
                    app.biblioteca.update(cx, |tela, cx| {
                        tela.selecionar(Some(0), cx);
                        tela.focar_busca(window, cx);
                    });
                });
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_input("DSC_0512");

        janela
            .update(cx, |_raiz, _window, cx| {
                assert_eq!(
                    app.read(cx).biblioteca.read(cx).texto_da_busca(cx),
                    "DSC_0512"
                );
            })
            .expect("a janela deve estar aberta");
        assert!(
            marcador.marcado().is_empty(),
            "digitar na busca não pode marcar foto nenhuma"
        );
    }

    /// 🚨 A tecla `\` alterna o antes/depois.
    #[gpui::test]
    fn a_tecla_barra_invertida_mostra_o_original(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("\\");

        janela
            .update(cx, |app, _window, cx| {
                assert!(app.revelacao.read(cx).mostrando_original());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Fechar o modal de importação **não** joga a listagem fora.
    ///
    /// Quem fecha por engano depois de marcar 300 fotos de um cartão não pode
    /// perder a marcação. A listagem só se perde ao escolher outra origem — que é
    /// quando ela deixou de valer.
    #[gpui::test]
    fn fechar_a_importacao_guarda_o_que_ja_foi_marcado(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF"],
        ));
        let janela = cx.add_window({
            let previews = previews.clone();
            let explorador = explorador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        explorador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.importar(window, cx);
                assert!(app.importando());

                app.importacao
                    .update(cx, |tela, cx| tela.abrir_origem("/cartao".into(), cx));
            })
            .expect("a janela deve estar aberta");

        for _ in 0..10 {
            let _ = janela.update(cx, |app, _window, cx| {
                app.importacao.update(cx, |tela, cx| tela.colher(cx))
            });
            cx.run_until_parked();
        }

        janela
            .update(cx, |app, window, cx| {
                app.importacao.update(cx, |tela, _cx| {
                    tela.estado.alternar(0);
                    assert_eq!(tela.estado.marcados(), 1);
                });

                app.fechar_importacao(window, cx);
                assert!(!app.importando());

                app.importar(window, cx);
                app.importacao.update(cx, |tela, _cx| {
                    assert_eq!(tela.estado.candidatos.len(), 2, "a listagem ficou");
                    assert_eq!(tela.estado.marcados(), 1, "e a marcação também");
                });
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Terminar de importar recarrega a grade.**
    ///
    /// Sem esta ligação a importação grava no banco e a Biblioteca continua com
    /// a lista lida no `main.rs`, antes de a janela existir: o modal conta "65
    /// importadas · 1 falharam" e a grade atrás fica exatamente como estava.
    /// **Nada falha** — e a leitura de quem usa é que a importação não funciona,
    /// com as fotos já no banco e no disco. Elas só apareciam ao reabrir o app.
    ///
    /// 🔑 O teste mede a ponta: o acervo de mentira responde com uma foto a mais
    /// e a grade tem de passar a mostrá-la. Não afirma nada sobre o caminho.
    #[gpui::test]
    fn terminar_de_importar_recarrega_a_grade(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF"],
        ));
        // O que o banco passa a ter depois da importação: as duas de antes mais
        // as duas do cartão.
        let acervo_novo = Arc::new(AcervoDeMentira::default());
        *acervo_novo.fotos.lock().expect("as fotos") = vec![
            foto("DSC_001.NEF"),
            foto("retrato.jpg"),
            foto("a.NEF"),
            foto("b.NEF"),
        ];

        let janela = cx.add_window({
            let previews = previews.clone();
            let explorador = explorador.clone();
            let acervo_novo = acervo_novo.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        explorador,
                        acervo: acervo_novo,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                assert_eq!(
                    app.biblioteca.read(cx).quantas_fotos(),
                    2,
                    "a grade começa com o acervo lido na abertura"
                );
                app.importar(window, cx);
                app.importacao
                    .update(cx, |tela, cx| tela.abrir_origem("/cartao".into(), cx));
            })
            .expect("a janela deve estar aberta");

        for _ in 0..10 {
            let _ = janela.update(cx, |app, _window, cx| {
                app.importacao.update(cx, |tela, cx| tela.colher(cx))
            });
            cx.run_until_parked();
        }

        janela
            .update(cx, |app, _window, cx| {
                app.importacao.update(cx, |tela, cx| {
                    tela.estado.marcar_todos(true);
                    tela.importar(cx);
                });
            })
            .expect("a janela deve estar aberta");

        // A colheita vê o `Terminou` e emite o evento; o `cx.emit` enfileira um
        // efeito, que só chega ao inscrito quando o laço de efeitos roda.
        for _ in 0..10 {
            let _ = janela.update(cx, |app, _window, cx| {
                app.importacao.update(cx, |tela, cx| tela.colher(cx))
            });
            cx.run_until_parked();
        }

        // A releitura é assíncrona, e a espera dela é um `timer` de 100ms.
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.biblioteca.read(cx).quantas_fotos(),
                    4,
                    "as duas do cartão entraram no banco e a grade não releu — \
                     é o defeito de 17/ago: o modal conta as fotos e a grade fica vazia"
                );
                assert_eq!(
                    *acervo_novo.pedidos.lock().expect("os pedidos"),
                    1,
                    "uma releitura por lote, e não uma por colheita"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Exportar entrega arquivo — o app nunca teve esse caminho.**
    ///
    /// `ExportPhotoUseCase`, `ExportController` e `ImageExporterImpl` existiam e
    /// estavam testados desde antes da migração, e **nunca eram construídos no
    /// `main.rs`**: as ocorrências de "export" em `crates/ui-gpui/src` eram todas
    /// comentário. O app de egui também não tinha, e é o que explica a migração
    /// não ter acusado — paridade com quem não exporta é não exportar.
    ///
    /// 🔑 O teste mede o pedido que chega à porta: quantas fotos, com que
    /// destino. É onde o defeito moraria, porque as camadas de dentro já
    /// passavam todas.
    #[gpui::test]
    fn exportar_manda_a_selecao_para_a_pasta_escolhida(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let exportador = Arc::new(ExportadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let exportador = exportador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        exportador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.exportar(cx);
                assert!(app.exportando());
                app.exportacao.update(cx, |tela, cx| {
                    assert_eq!(
                        tela.quantas(),
                        2,
                        "sem seleção múltipla, exporta o que a grade está mostrando"
                    );
                    // Sem pasta escolhida não sai nada: gravar em algum lugar
                    // padrão espalharia arquivo onde ninguém foi procurar.
                    tela.exportar(cx);
                });
            })
            .expect("a janela deve estar aberta");

        assert!(
            exportador.pedidos().is_empty(),
            "exportar sem pasta escolhida não pode gravar nada"
        );

        let pasta = tempfile::tempdir().expect("pasta de saída");
        janela
            .update(cx, |app, _window, cx| {
                app.exportacao.update(cx, |tela, cx| {
                    tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
                    tela.exportar(cx);
                    tela.colher(cx);
                });
            })
            .expect("a janela deve estar aberta");

        let pedidos = exportador.pedidos();
        assert_eq!(pedidos.len(), 1, "um lote, e não um pedido por foto");
        let saidas = &pedidos[0];
        assert_eq!(saidas.len(), 2);
        assert_eq!(
            saidas[0].destino,
            pasta.path().join("DSC_001.jpg"),
            "o nome vem da origem, com a extensão do formato de saída"
        );
        assert_eq!(saidas[1].destino, pasta.path().join("retrato.jpg"));

        janela
            .update(cx, |app, _window, cx| {
                app.exportacao.update(cx, |tela, cx| {
                    tela.colher(cx);
                    let p = tela.progresso().expect("o lote começou");
                    assert!(p.terminou);
                    assert_eq!((p.feitas, p.falhas), (2, 0));
                    assert_eq!(tela.resumo(), "2 exportadas");
                });
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **Colar revelação não pode apagar o enquadramento das fotos coladas.**
    ///
    /// `SavePhotoEditsUseCase` recebe os oito campos de corte e a entidade faz
    /// atribuição direta, **sem mesclar**: gravar sem reenviá-los apaga o corte.
    /// Colar em 40 fotos apagaria o enquadramento de 40 — sem erro, sem aviso e
    /// sem desfazer, no gesto que existe justamente para poupar trabalho.
    ///
    /// 🔑 **E cada foto conserva o SEU corte, não o da origem** — o que o
    /// Lightroom faz, porque cor se repete numa sessão e composição não.
    ///
    /// ⚠️ **Esse segundo caso não é garantido por este teste, e sim pelo tipo**:
    /// a área de transferência é `Option<Ajustes>`, e `Ajustes` não tem os oito
    /// campos de corte. Não há como colar o enquadramento da origem porque não
    /// há onde guardá-lo. Tentei quebrar de propósito para conferir e a quebra
    /// não passou de um no-op — o que é a resposta certa: uma garantia que o
    /// tipo dá não precisa de teste, precisa de estar escrita.
    ///
    /// O que este teste cobre é o caso que **é** alcançável: o corte do destino
    /// ser esquecido na gravação.
    #[gpui::test]
    fn colar_revelacao_preserva_o_corte_de_cada_foto(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());

        let mut origem = foto("DSC_001.NEF");
        origem.edit_exposure = Some(1.5);
        origem.edit_crop_x = Some(0.1);
        origem.edit_crop_width = Some(0.5);
        let mut destino = foto("retrato.jpg");
        destino.edit_crop_x = Some(0.4);
        destino.edit_crop_width = Some(0.2);

        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            move |window, cx| {
                Aplicativo::novo(
                    vec![origem, destino],
                    previews,
                    Vec::new(),
                    Portas {
                        gravador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.copiar_revelacao(cx);
                assert!(app.tem_revelacao_copiada());

                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.colar_revelacao(cx);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "uma gravação, na foto de destino");
        let (id, ajustes, corte) = &gravado[0];

        assert_eq!(id, "id-retrato.jpg");
        assert_eq!(
            ajustes.exposure, 1.5,
            "a exposição da origem tinha de ser colada"
        );
        assert_eq!(
            (corte.x, corte.largura),
            (Some(0.4), Some(0.2)),
            "a foto de destino perdeu o próprio enquadramento — ou recebeu o da origem"
        );
    }

    /// 🔑 **Colar vale para a seleção inteira** — é o que torna 800 fotos
    /// viáveis. Uma gravação por foto, e não uma só na principal.
    #[gpui::test]
    fn colar_revelacao_vale_para_a_selecao_inteira(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        gravador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.copiar_revelacao(cx);

                // ⚠️ `selecionar` já põe a foto na seleção múltipla, então aqui
                // basta acrescentar a segunda — alternar a primeira a tiraria.
                app.biblioteca
                    .update(cx, |tela, cx| tela.alternar_uma(1, cx));
                app.colar_revelacao(cx);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            gravador.gravado().len(),
            2,
            "colar tinha de gravar nas duas selecionadas"
        );
    }

    /// ⚠️ **Colar sem ter copiado não grava nada.**
    ///
    /// Sem esta guarda, `Cmd+Shift+V` numa sessão recém-aberta gravaria o neutro
    /// em cima da revelação de todas as selecionadas — e o gesto que apaga o
    /// trabalho seria vizinho de teclado do que o repete.
    #[gpui::test]
    fn colar_sem_copiar_nao_grava_nada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        gravador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                assert!(!app.tem_revelacao_copiada());
                app.colar_revelacao(cx);
            })
            .expect("a janela deve estar aberta");

        assert!(gravador.gravado().is_empty());
    }

    /// 🚨 **A prévia não sai sem marca d\'água.**
    ///
    /// É o teste mais importante da exportação, e o defeito que ele impede é o
    /// pior que este aplicativo pode cometer: a foto que o cliente **não
    /// comprou** indo legível e em tamanho cheio para a galeria. Nada falharia —
    /// o lote termina, o rodapé conta certo, e os arquivos estão lá.
    ///
    /// 🔑 O botão desligado não é a defesa; é o sintoma dela. Quem decide é
    /// `opcoes()`, que devolve `None` — e `exportar` sai sem pedir nada.
    #[gpui::test]
    fn a_previa_nao_sai_sem_marca_dagua(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let exportador = Arc::new(ExportadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let exportador = exportador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        exportador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let pasta = tempfile::tempdir().expect("pasta de saída");
        janela
            .update(cx, |app, _window, cx| {
                app.exportar(cx);
                app.exportacao.update(cx, |tela, cx| {
                    tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
                    tela.escolher_modo(Modo::Previa, cx);
                    assert!(
                        tela.opcoes().is_none(),
                        "sem marca escolhida não pode haver opções válidas"
                    );
                    tela.exportar(cx);
                });
            })
            .expect("a janela deve estar aberta");

        assert!(
            exportador.pedidos().is_empty(),
            "a prévia saiu sem marca d'água — a foto não comprada iria legível para a galeria"
        );

        // Com a marca escolhida, sai — e leva a marca junto.
        let logo = pasta.path().join("logo.png");
        janela
            .update(cx, |app, _window, cx| {
                app.exportacao.update(cx, |tela, cx| {
                    tela.escolher_marca_para_teste(logo.clone(), cx);
                    tela.exportar(cx);
                });
            })
            .expect("a janela deve estar aberta");

        let opcoes = exportador
            .opcoes
            .lock()
            .expect("as opções")
            .clone()
            .expect("o lote saiu");
        let marca = opcoes.watermark().expect("a prévia tem de levar marca");
        assert_eq!(marca.file().as_str().unwrap(), logo.to_str().unwrap());
        assert_eq!(
            opcoes.longest_edge(),
            Some(2048),
            "a prévia também reduz — tamanho cheio na galeria é entrega, não prévia"
        );
    }

    /// ⚠️ **A entrega final vai inteira e sem marca**, e o modo padrão é ela.
    ///
    /// O padrão importa: se fosse a prévia, a primeira exportação de um cliente
    /// sairia com logotipo em cima — visível, e por isso corrigível. Sendo a
    /// entrega, o erro possível é o inverso e **não** é visível, e é por isso que
    /// o modo aparece escrito na tela em vez de ficar implícito.
    #[gpui::test]
    fn a_entrega_final_e_o_padrao_e_vai_sem_marca(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let exportador = Arc::new(ExportadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let exportador = exportador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        exportador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let pasta = tempfile::tempdir().expect("pasta de saída");
        janela
            .update(cx, |app, _window, cx| {
                app.exportar(cx);
                app.exportacao.update(cx, |tela, cx| {
                    assert_eq!(tela.modo(), Modo::Entrega, "o padrão é a entrega");
                    tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
                    tela.exportar(cx);
                });
            })
            .expect("a janela deve estar aberta");

        let opcoes = exportador
            .opcoes
            .lock()
            .expect("as opções")
            .clone()
            .expect("o lote saiu");
        assert!(opcoes.watermark().is_none(), "a entrega não leva marca");
        assert_eq!(
            opcoes.longest_edge(),
            None,
            "a entrega vai no tamanho cheio"
        );
    }

    /// ⚠️ **A falha de uma foto não some, e não interrompe o lote.**
    ///
    /// Parar na 7ª de 400 desperdiça as 393 que sairiam; engolir a falha faz o
    /// rodapé dizer 400 com 399 na pasta. As duas são piores que contar.
    #[gpui::test]
    fn uma_falha_no_meio_aparece_e_o_lote_continua(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let exportador = Arc::new(ExportadorDeMentira::default());
        *exportador.falham.lock().expect("as falhas") = 1;

        let janela = cx.add_window({
            let previews = previews.clone();
            let exportador = exportador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        exportador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let pasta = tempfile::tempdir().expect("pasta de saída");
        janela
            .update(cx, |app, _window, cx| {
                app.exportar(cx);
                app.exportacao.update(cx, |tela, cx| {
                    tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
                    tela.exportar(cx);
                    tela.colher(cx);

                    let p = tela.progresso().expect("o lote começou");
                    assert_eq!(
                        (p.feitas, p.falhas),
                        (1, 1),
                        "a segunda saiu mesmo com a primeira falhando"
                    );
                    assert_eq!(tela.resumo(), "1 exportadas · 1 falharam");
                });
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Trocar de foto na grade **não** troca a foto em revelação.
    ///
    /// A seleção é copiada, não compartilhada. Compartilhada, voltar à
    /// Biblioteca e clicar em outra miniatura trocaria calado o que está sendo
    /// editado — e o próximo ajuste cairia na foto errada.
    #[gpui::test]
    fn mudar_a_selecao_depois_nao_troca_o_que_esta_em_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                app.voltar_para_biblioteca(window, cx);
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));

                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg"),
                    "a Revelação guarda a foto que recebeu, não a que a grade mostra agora"
                );
            })
            .expect("a janela deve estar aberta");
    }
}
