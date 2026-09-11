//! A raiz: a barra de navegação e qual tela está embaixo dela.
//!
//! Nasceu com a Revelação, porque até a fase 1 só havia uma tela e a janela
//! podia abrir a Biblioteca direto. A partir de duas, alguém precisa saber qual
//! está no ar — e esse alguém não pode ser nenhuma das duas.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::value_objects::CropSettings;
use gpui::{actions, div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::atualizacao::faixa::{self, Pedido as PedidoDeAtualizacao};
use crate::atualizacao::porta::{Atualizador, AtualizadorDaWeb, Aviso};
use crate::balcao::tela::Balcao;
use crate::biblioteca::acervo::Acervo;
use crate::biblioteca::colecoes::Colecoes;
use crate::biblioteca::marcacao::Marcador;
use crate::biblioteca::tela::Biblioteca;
use crate::biblioteca::tela::Classificou;
use crate::cliente::{monitor_do_cliente, Cliente};
use crate::configuracoes::Configuracoes;
use crate::entrada::{Entrada, Entrou};
use crate::exportacao::porta::Exportador;
use crate::exportacao::tela::Exportacao;
use crate::importacao::explorador::{Explorador, GeradorDeMiniaturas, Importador, SeletorDePasta};
use crate::importacao::tela::{Importacao, Importou};
use crate::impressao::tela::Impressao;
use crate::pos_venda::porta::Publicador;
use crate::pos_venda::porta::Recado as PosVendaRecado;
use crate::revelacao::persistencia::{self, Gravador};
use crate::revelacao::presets::GuardaDePresets;
use crate::revelacao::processador::Ajustes;
use crate::revelacao::reposicao::{APor, Recado as ReposicaoRecado, Repositor};
use crate::revelacao::sincronizacao;
use crate::revelacao::tela::{PedidoDaRevelacao, Revelacao};
use crate::sessoes::arquivos::SeletorDeFotos;
use crate::sessoes::detalhe::{Detalhe, FotoARevelar, Pedido as DetalhePedido};
use crate::sessoes::tela::{Escolhida, Sessoes};
use crate::tema;

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
    /// Quem fala com o pós-venda do site — o vão que o projeto existe para fechar.
    pub publicador: Arc<dyn Publicador>,
    /// As coleções — o ensaio do cliente mora numa.
    pub colecoes: Arc<dyn Colecoes>,
    /// Quem monta o PDF da folha e o entrega ao disco ou à impressora.
    pub folha: Arc<dyn crate::impressao::porta::Folha>,
    pub marcador: Arc<dyn Marcador>,
    pub gerador: Arc<dyn GeradorDeMiniaturas>,
    /// Quem refaz o preview de uma foto catalogada a partir do arquivo.
    ///
    /// ⚠️ **Separado do [`Self::gerador`], que é da importação**: aquele gera
    /// miniatura de arquivo que ainda **não** está no catálogo, e grava sob a
    /// chave `import::<caminho>`. Este grava sob o id da foto. Juntar os dois
    /// numa porta só faria um deles escrever na chave do outro — e o sintoma
    /// seria a grade mostrar a foto certa no lugar errado.
    pub repositor: Arc<dyn Repositor>,
    pub guarda_de_presets: Arc<dyn GuardaDePresets>,
    /// Quem abre a janela do sistema para escolher `.lrtemplate` e `.xmp`.
    pub escolha_de_presets: Arc<dyn crate::revelacao::lightroom::EscolhaDePresets>,
    pub explorador: Arc<dyn Explorador>,
    pub importador: Arc<dyn Importador>,
    pub seletor: Arc<dyn SeletorDePasta>,
    /// Quem abre a janela **do sistema** para escolher as fotos da sessão.
    ///
    /// ⚠️ Separado do [`Self::seletor`], que escolhe **pastas** para a
    /// importação: são janelas diferentes do sistema, com filtros diferentes, e
    /// juntá-las numa porta só faria uma delas mentir sobre o que devolve.
    pub seletor_de_fotos: Arc<dyn SeletorDeFotos>,
    /// Quem descobre que há versão nova e a instala.
    ///
    /// 🔑 O app não passa por loja nenhuma — é baixado de
    /// `recordarfotos.com.br/vintageLightbox` — então **sem esta porta ninguém
    /// atualiza**: corrigir um defeito viraria pedir a cada fotógrafo que
    /// reinstale à mão, e o que acontece de verdade é a versão velha rodar por
    /// meses.
    pub atualizador: Arc<dyn Atualizador>,
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
        Acima,
        Abaixo,
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
        AlternarComprada,
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
        // 🚨 **E com Ctrl, pelo mesmo motivo do `Cmd+A` lá embaixo**: este app
        // roda em Windows e Linux, onde desfazer é `Ctrl+Z` e mais nada. Com
        // só o `cmd-`, o gesto mais universal que existe não fazia nada fora do
        // Mac — e ninguem reclama de um desfazer que nao funciona, apenas para
        // de confiar no programa (dono, 2026-09-11).
        gpui::KeyBinding::new("ctrl-shift-z", Refazer, Some(CONTEXTO)),
        gpui::KeyBinding::new("ctrl-z", Desfazer, Some(CONTEXTO)),
        // Copiar e colar revelação — as mesmas teclas do Lightroom.
        //
        // 🔑 **Levam `CONTEXTO` e não `SEM_CAMPO_DE_TEXTO`**, como o `Cmd+Z`:
        // com modificador não há disputa com quem está digitando, porque tecla
        // com `Cmd` não vira letra.
        gpui::KeyBinding::new("cmd-shift-c", CopiarRevelacao, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-shift-v", ColarRevelacao, Some(CONTEXTO)),
        gpui::KeyBinding::new("ctrl-shift-c", CopiarRevelacao, Some(CONTEXTO)),
        gpui::KeyBinding::new("ctrl-shift-v", ColarRevelacao, Some(CONTEXTO)),
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
        // 🚨 **↑ e ↓ andam uma linha**, e faltavam: numa grade de 7 colunas,
        // chegar à foto de baixo custava sete ← ou →. É o que o Lightroom faz e
        // o que a mão espera de qualquer grade.
        gpui::KeyBinding::new("up", Acima, Some(SEM_CAMPO_DE_TEXTO)),
        gpui::KeyBinding::new("down", Abaixo, Some(SEM_CAMPO_DE_TEXTO)),
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
        // `B` de balcão: a foto que o cliente levou. Não é tecla do Lightroom —
        // é a única decisão deste fluxo que ele não tem onde guardar.
        gpui::KeyBinding::new("b", AlternarComprada, Some(SEM_CAMPO_DE_TEXTO)),
        // `Cmd+A` e `Cmd+D`, da Biblioteca. Levam `CONTEXTO` e não
        // `SEM_CAMPO_DE_TEXTO`: com modificador não há disputa com o texto — e
        // o campo de busca tem o **próprio** `Cmd+A` (selecionar tudo no
        // campo), que o GPUI prefere por ser mais profundo. Ligá-los com
        // `!Input` tiraria o `Cmd+A` de dentro do campo sem ganhar nada.
        gpui::KeyBinding::new("cmd-a", SelecionarTudo, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-d", LimparSelecao, Some(CONTEXTO)),
        // 🚨 **E com Ctrl também**, como a web (`e.ctrlKey || e.metaKey`): o
        // dono aperta Ctrl+A na tira da Revelação, e com `cmd-a` só o lote
        // nunca se formava — o botão "Sincronizar N" não aparecia e parecia
        // que a sincronização não existia (7/set/2026).
        gpui::KeyBinding::new("ctrl-a", SelecionarTudo, Some(CONTEXTO)),
        gpui::KeyBinding::new("ctrl-d", LimparSelecao, Some(CONTEXTO)),
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
    /// As sessões fotográficas — a mesma tela que o site tem em
    /// `/dashboard/sessoes-fotograficas`.
    ///
    /// 🔑 **É tela, e não modal**, ao contrário da publicação: listar sessões é
    /// para onde se volta o dia inteiro, e um modal obrigaria a fechar a
    /// triagem a cada consulta.
    Sessoes,
    /// **Dentro** de uma sessão — a rota `[id]` da web.
    ///
    /// 🚨 É a família de tela em que o app passa a abrir depois do login, e isso
    /// é escolha: na web a sessão é **onde se trabalha**, e o desktop abria num
    /// catálogo global com as sessões de lado. Quem vinha de lá achava a coisa
    /// "muito aberta e estranha" — e estava certo.
    Sessao,
}

pub struct Aplicativo {
    pub(crate) biblioteca: Entity<Biblioteca>,
    pub(crate) revelacao: Entity<Revelacao>,
    /// A folha de impressão. Guarda o leiaute entre uma visita e outra: papel,
    /// margem e modelo são escolhas sobre o papel, não sobre a foto.
    impressao: Entity<Impressao>,
    /// O modal de importação. **Sempre existe**, e só aparece quando aberto: ele
    /// guarda a listagem, e recriá-lo a cada abertura jogaria fora o que o
    /// fotógrafo já marcou ao fechar o modal por engano.
    pub(crate) importacao: Entity<Importacao>,
    importando: bool,
    /// O modal de exportação — o único caminho do app até um arquivo no disco.
    exportacao: Entity<Exportacao>,
    exportando: bool,
    /// O modal do pós-venda — o único caminho do app até o site.
    /// O balcão: o que o cliente acertou ao levar a foto na hora.
    pub(crate) balcao: Entity<Balcao>,
    no_balcao: bool,
    /// A lista de sessões fotográficas.
    pub(crate) sessoes: Entity<Sessoes>,
    /// Dentro de uma sessão: cabeçalho, envio e a grade do site.
    pub(crate) detalhe: Entity<Detalhe>,
    /// 🚨 A inscrição no que a tela da sessão pede. Descartada, o botão de
    /// voltar e o clique para revelar param de responder — sem erro nenhum.
    _pedido_da_sessao: gpui::Subscription,
    /// A barra da Revelação pedindo o que só esta raiz sabe fazer.
    _pedido_da_revelacao: gpui::Subscription,
    /// A sessão escolhida para receber as fotos. `None` é "nenhuma aberta".
    sessao_aberta: Option<String>,
    /// As fotos que já estão **no site**, do ensaio aberto, na linguagem da
    /// grade.
    ///
    /// 🔑 **A grade é uma só, como na web**: as locais (ainda não enviadas) e as
    /// do acervo aparecem juntas. Separá-las em duas telas foi o que fez o app
    /// parecer que tinha dois lugares para a mesma coisa.
    fotos_do_site: Vec<PhotoViewModel>,
    /// Quem fala com o pós-venda do site. Guardado porque a classificação
    /// sobe e tira fotos fora do modal de publicação.
    publicador: Arc<dyn Publicador>,
    /// Por onde volta o resultado de subir/tirar uma foto classificada.
    sincronias: (Sender<PosVendaRecado>, Receiver<PosVendaRecado>),
    /// 🚨 A `Task` que espera o site responder. **Descartá-la a cancela**, e o
    /// sintoma seria a grade nunca reler o catálogo depois de uma foto subir —
    /// o id remoto estaria gravado no banco e ausente da tela.
    _sincronia: Option<gpui::Task<()>>,
    /// Quantas respostas do site ainda são esperadas.
    ///
    /// 🚨 **Sem esta conta o laço parava na primeira.** Ele desligava assim que
    /// `colher_sincronia` dizia "chegou alguma coisa" — e num lote as respostas
    /// chegam **espalhadas no tempo**, uma por foto, cada uma depois de baixar,
    /// revelar e subir. Da segunda em diante ninguém as lia: a grade mostrava
    /// só a primeira revelada, e os erros das outras não apareciam em lugar
    /// nenhum. Era o "não está sincronizando" de 7/set/2026, e valia também
    /// para classificar trinta fotos de uma vez.
    sincronias_pendentes: usize,
    /// As fotos que receberam receita nova aqui e cujo JPEG no site ainda é o
    /// de antes — a fila que o "Salvar na galeria" esvazia.
    ///
    /// 🚨 **Ela existe porque a foto do catálogo não tem depósito.** A que só
    /// existe no site guarda a receita não enviada em
    /// `Gravador::guardadas_do_site`, que é tabela e sobrevive a fechar o app; a
    /// que foi importada aqui e depois subiu grava no catálogo, e lá não há
    /// coluna dizendo "isto ainda não foi para o site". Enquanto não houver,
    /// esta lista é o que sabe — e o que ela sabe morre com a sessão aberta.
    /// Fechar o app antes de salvar não perde ajuste nenhum (a receita está
    /// gravada, e a foto reabre com ela); perde o **aviso** de que o cliente
    /// ainda vê o JPEG antigo.
    a_subir: Vec<(String, Ajustes, CropSettings)>,
    /// Quem refaz o preview que o cache perdeu, e por onde a resposta volta.
    repositor: Arc<dyn Repositor>,
    reposicoes: (Sender<ReposicaoRecado>, Receiver<ReposicaoRecado>),
    /// Quantas fotos ainda faltam voltar do disco.
    ///
    /// 🚨 **Sem esta conta o laço parava na primeira** — o mesmo defeito que
    /// `sincronias_pendentes` documenta, e aqui a lista é a tira inteira.
    reposicoes_pendentes: usize,
    /// As fotos já pedidas ao disco e que ainda não voltaram.
    ///
    /// 🚨 **A varredura roda a cada troca de foto, e o cache só muda quando a
    /// reposição termina.** Sem esta lembrança, andar cinco fotos pela seta
    /// enquanto a primeira ainda decodifica pediria as mesmas cinco outra vez, a
    /// cada tecla — o repositor trabalha em série, então a fila cresceria mais
    /// depressa do que anda, e a foto do palco ficaria atrás de dezenas de
    /// duplicatas dela mesma.
    reposicoes_pedidas: std::collections::HashSet<String>,
    /// 🚨 A `Task` que espera o disco responder. **Descartá-la a cancela**, e a
    /// foto ficaria em "Preparando…" com o preview já gravado no cache.
    _reposicao: Option<gpui::Task<()>>,
    /// 🚨 A inscrição na travessia do zero da classificação. Sem ela nada acusa:
    /// as estrelas entram no banco, e nada sobe nem sai do site.
    _classificacao: gpui::Subscription,
    /// 🚨 A inscrição na escolha da sessão. Descartada, a tela marca a linha e
    /// o resto do app continua sem saber em qual galeria as fotos entram.
    _sessao_escolhida: gpui::Subscription,
    /// A porta do app: entrar na conta do site. Não há outra.
    ///
    /// 🚨 **Enquanto [`Self::sessao`] é `None`, é só ela que aparece.** Foi a
    /// reversão pedida pelo dono em 6/set/2026 do princípio "o app tem de ser
    /// útil sozinho" — e a saída dela, o botão "trabalhar offline", caiu no
    /// mesmo dia: o propósito do app é a integração com o pós-venda, e sem
    /// conta não há ensaio a que as fotos pertençam. Trabalhar sem rede volta
    /// como **sincronização**, que guarda e concilia — não como um botão que
    /// desliga o site.
    entrada: Entity<Entrada>,
    /// A conta que entrou nesta abertura. `None` é "ainda na porta".
    sessao: Option<domain::services::pos_venda::Sessao>,
    /// 🚨 A inscrição na entrada da porta. Descartada, o app fica na tela de
    /// login para sempre — com o login funcionando e sem nada acontecendo.
    _escolha: gpui::Subscription,
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
    pub(crate) tela: Tela,
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
    /// Quem procura versão nova e a instala.
    atualizador: Arc<dyn Atualizador>,
    /// O que a faixa do rodapé mostra sobre a atualização.
    atualizacao: faixa::Estado,
    /// O canal por onde o resultado da procura e o da instalação voltam.
    avisos_de_versao: (Sender<Aviso>, Receiver<Aviso>),
    /// 🚨 A `Task` que espera o aviso chegar. **Descartá-la a cancela**, e o
    /// sintoma seria a faixa nunca aparecer — versão nova publicada, ninguém
    /// sabendo.
    _atualizacao: Option<gpui::Task<()>>,
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
        // 🔑 **A procura por versão nova começa aqui, na abertura**, e não no
        // primeiro clique em nada. É o único momento em que o operador não está
        // no meio de coisa nenhuma — depois disso ele está triando, e a hora de
        // avisar já passou. `procurar` devolve na hora; a resposta chega pelo
        // canal, e `esperar_aviso` é quem a recolhe.
        let avisos_de_versao = channel();
        portas.atualizador.procurar(avisos_de_versao.0.clone());
        let atualizacao = Self::esperar_aviso(cx);

        // O mesmo cache de previews da Biblioteca: a importação grava miniatura
        // com a chave `import::` na mesma tabela, e dois `PreviewManager` para o
        // mesmo arquivo seriam dois caches do mesmo lugar.
        let previews_para_importar = previews.clone();
        let previews_para_imprimir = previews.clone();
        let previews_do_cliente = previews.clone();
        let previews_do_detalhe = previews.clone();
        // 🔑 **O mesmo importador do modal.** A tela da sessão grava no mesmo
        // catálogo, com o mesmo caminho: o que muda é a porta de entrada, e não
        // o destino.
        let importador_do_detalhe = portas.importador.clone();
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
                portas.escolha_de_presets,
                presets,
                window,
                cx,
            )
        });

        // 🔑 A Revelação **pede** e não faz: exportar abre um modal com pasta
        // de destino e publicar fala com o pós-venda — as duas coisas valem
        // para a seleção inteira e moram aqui.
        let pedido_da_revelacao = cx.subscribe_in(
            &revelacao,
            window,
            |raiz, _tela, pedido: &PedidoDaRevelacao, window, cx| {
                raiz.atender_a_revelacao(*pedido, window, cx);
            },
        );

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
            let (foto, posicao) = {
                let b = biblioteca.read(cx);
                (b.foto_selecionada(), b.posicao_da_selecao())
            };
            if let Some(foto) = foto {
                raiz.mostrar_ao_cliente(&foto, posicao, cx);
            }
        });

        // A porta do app. O mesmo `Publicador` do pós-venda: entrar é a mesma
        // chamada, e dois clientes HTTP para o mesmo site seriam dois lugares
        // onde a base da API pode divergir.
        let publicador_das_sessoes = portas.publicador.clone();
        let publicador_da_raiz = portas.publicador.clone();
        let publicador_do_balcao = portas.publicador.clone();
        let publicador_do_detalhe = portas.publicador.clone();
        let entrada = cx.new(|cx| {
            Entrada::nova(
                portas.publicador,
                crate::pos_venda::config::ler(),
                window,
                cx,
            )
        });
        let escolha = cx.subscribe(&entrada, |raiz, _entrada, evento: &Entrou, cx| {
            raiz.entrar_na_conta(evento.0.clone(), cx);
        });

        // 🔑 A Biblioteca **notifica** que a classificação atravessou o zero, e
        // não sobe nada: quem tem a sessão e a galeria aberta é esta raiz. É o
        // que deixa a grade não precisar saber que existe um site.
        let classificacao = cx.subscribe(&biblioteca, |raiz, _tela, evento: &Classificou, cx| {
            raiz.sincronizar_classificacao(evento.clone(), cx);
        });

        let balcao = cx.new(|cx| Balcao::nova(publicador_do_balcao, window, cx));
        let sessoes = cx.new(|cx| Sessoes::nova(publicador_das_sessoes, window, cx));
        let sessao_escolhida = cx.subscribe(&sessoes, |raiz, _tela, evento: &Escolhida, cx| {
            raiz.entrar_na_sessao(evento.0.clone(), cx);
        });

        let detalhe = cx.new(|cx| {
            Detalhe::nova(
                publicador_do_detalhe,
                portas.seletor_de_fotos,
                importador_do_detalhe,
                previews_do_detalhe,
                cx,
            )
        });
        // 🔑 `subscribe_in`, e não `subscribe`: revelar precisa da janela — os
        // 42 sliders são espalhados com ela. Sem isso o pedido teria de ficar
        // guardado até o próximo quadro, e "clique que só responde no quadro
        // seguinte" é indistinguível de clique perdido.
        let pedido_da_sessao = cx.subscribe_in(
            &detalhe,
            window,
            |raiz, _tela, pedido: &DetalhePedido, window, cx| {
                raiz.atender_a_sessao(pedido, window, cx);
            },
        );

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
            entrada,
            sessao: None,
            _escolha: escolha,
            balcao,
            no_balcao: false,
            sessoes,
            detalhe,
            _pedido_da_sessao: pedido_da_sessao,
            _pedido_da_revelacao: pedido_da_revelacao,
            sessao_aberta: None,
            fotos_do_site: Vec::new(),
            publicador: publicador_da_raiz,
            sincronias: channel(),
            _sincronia: None,
            sincronias_pendentes: 0,
            a_subir: Vec::new(),
            repositor: portas.repositor,
            reposicoes: channel(),
            reposicoes_pendentes: 0,
            reposicoes_pedidas: std::collections::HashSet::new(),
            _reposicao: None,
            _classificacao: classificacao,
            _sessao_escolhida: sessao_escolhida,
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
            atualizador: portas.atualizador,
            atualizacao: faixa::Estado::default(),
            avisos_de_versao,
            _atualizacao: Some(atualizacao),
        }
    }

    /// Recolhe o próximo aviso de atualização e o põe na faixa.
    ///
    /// ⚠️ **O laço acaba.** Como o da releitura, ele morre na primeira resposta
    /// e desiste depois de 60s — a procura por versão nova é acessória, e um
    /// laço eterno acordaria a cada 200ms pelo resto da sessão para não dizer
    /// nada. A instalação começa um laço novo.
    fn esperar_aviso(cx: &mut Context<Self>) -> gpui::Task<()> {
        cx.spawn(async move |raiz, cx| {
            for _ in 0..300 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
                let Ok(chegou) = raiz.update(cx, |raiz, cx| {
                    let Ok(aviso) = raiz.avisos_de_versao.1.try_recv() else {
                        return false;
                    };
                    raiz.atualizacao.instalando = false;
                    raiz.atualizacao.aviso = Some(aviso);
                    cx.notify();
                    true
                }) else {
                    return;
                };
                if chegou {
                    return;
                }
            }
        })
    }

    /// O que cada botão da faixa faz.
    fn atender(&mut self, pedido: PedidoDeAtualizacao, cx: &mut Context<Self>) {
        match pedido {
            PedidoDeAtualizacao::Instalar => {
                // A faixa troca para "Baixando…" **antes** de o download
                // começar: sem isto, um clique num pacote de 60 MB não muda
                // nada na tela por meio minuto, e o gesto seguinte é clicar de
                // novo — duas instalações da mesma versão em paralelo.
                self.atualizacao.instalando = true;
                self.atualizador.instalar(self.avisos_de_versao.0.clone());
                self._atualizacao = Some(Self::esperar_aviso(cx));
            }
            PedidoDeAtualizacao::Reabrir => AtualizadorDaWeb::reabrir(),
            PedidoDeAtualizacao::Dispensar => {
                // 🔑 Guardar **a versão**, e não um booleano: dispensar a 0.2.0
                // não pode calar a 0.3.0, que pode ser justamente a correção
                // que ele precisa. E some só nesta abertura — na próxima o
                // aviso volta.
                if let Some(Aviso::Disponivel(nova)) = &self.atualizacao.aviso {
                    self.atualizacao.dispensada = Some(nova.versao.clone());
                } else {
                    self.atualizacao.aviso = None;
                }
            }
        }
        cx.notify();
    }

    /// A faixa do rodapé, quando há o que dizer.
    fn faixa_de_atualizacao(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        // O `cx.listener` é o que dá à faixa acesso a `&mut Aplicativo` de
        // dentro de um clique; ela própria não conhece a raiz — só diz qual
        // [`PedidoDeAtualizacao`] o operador fez.
        let agir = cx.listener(|este, pedido: &PedidoDeAtualizacao, _window, cx| {
            este.atender(*pedido, cx);
        });
        faixa::desenhar(
            &self.atualizacao,
            cx,
            Arc::new(move |pedido, window, app| agir(&pedido, window, app)),
        )
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
                    // 🔑 **A tela da sessão recebe as locais deste ensaio.**
                    // Sem isto a foto importada ficaria gravada e invisível —
                    // o mesmo desfecho de não ter importado. Antes das do
                    // site entrarem na lista: a partir daí não dá mais para
                    // separar quem é quem.
                    raiz.mostrar_as_locais_na_sessao(&fotos, cx);
                    // 🔑 As do site entram na mesma lista — a grade é uma só.
                    let mut todas = fotos;
                    todas.extend(raiz.fotos_do_site.iter().cloned());
                    raiz.biblioteca
                        .update(cx, |tela, cx| tela.trocar_acervo(todas, cx));
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

    /// Entrega à tela da sessão as fotos **deste ensaio que só existem aqui**.
    ///
    /// 🚨 **A que já subiu fica de fora.** Ela existe dos dois lados — linha no
    /// SQLite e linha no site —, e mandar as duas mostraria a mesma foto duas
    /// vezes na grade, com estados diferentes. Vale a do site: é a que tem nota,
    /// preço e negociação.
    ///
    /// 🔑 **A nota não atravessa.** Uma foto local com nota é uma que ainda não
    /// terminou de subir; na grade da sessão ela entra como não classificada,
    /// que é o que o recorte "Sem nota" existe para encontrar.
    fn mostrar_as_locais_na_sessao(&mut self, fotos: &[PhotoViewModel], cx: &mut Context<Self>) {
        let Some(galeria) = self.sessao_aberta.clone() else {
            return;
        };
        let locais: Vec<biblioteca_core::acervo::Foto> = fotos
            .iter()
            .filter(|f| f.sessao_id.as_deref() == Some(galeria.as_str()))
            .filter(|f| f.pos_venda_foto_id.is_none())
            .enumerate()
            .map(|(i, f)| local_para_a_grade(f, i as i64))
            .collect();
        self.detalhe
            .update(cx, |tela, cx| tela.definir_locais(locais, cx));
    }

    /// Entra numa sessão — o mesmo gesto que abre a rota `[id]` na web.
    ///
    pub fn entrar_na_sessao(&mut self, galeria_id: String, cx: &mut Context<Self>) {
        self.sessao_aberta = Some(galeria_id.clone());
        // 🚨 **A grade passa a ser a do ensaio.** É o modelo da web: dentro da
        // sessão é que se revela e se escolhe com o cliente, e a grade tem de
        // mostrar as fotos dele — não as de todos os clientes juntos.
        self.biblioteca.update(cx, |tela, cx| {
            tela.escopar_na_sessao(Some(galeria_id.clone()), cx)
        });
        self.detalhe
            .update(cx, |tela, cx| tela.entrar(galeria_id, cx));
        self.tela = Tela::Sessao;
        cx.notify();
    }

    /// As fotos do site do ensaio aberto entraram: a grade se refaz com elas.
    ///
    /// 🔑 **A ordem é a do site**, e as locais vêm antes: é o que a web faz
    /// ordenando pela `ordem`, e o efeito é o mesmo — a foto que sobe não pula
    /// de lugar na tela.
    pub fn absorver_as_do_site(&mut self, fotos: Vec<PhotoViewModel>, cx: &mut Context<Self>) {
        self.fotos_do_site = fotos;
        self.aplicar_o_deposito_do_site();

        // 🚨 **Entram na hora, e não na próxima releitura.** A releitura do
        // catálogo anda por relógio; esperar por ela deixaria a grade sem as
        // fotos que o site acabou de responder — e o operador, olhando uma
        // sessão que parece vazia.
        let locais: Vec<PhotoViewModel> = self
            .biblioteca
            .read(cx)
            .todas_as_fotos()
            .into_iter()
            // As do site anteriores saem: elas vêm de novo, e ficar com as duas
            // versões duplicaria a foto na grade.
            .filter(|f| f.pos_venda_foto_id.is_none())
            .collect();
        let mut todas = locais;
        todas.extend(self.fotos_do_site.iter().cloned());
        self.biblioteca
            .update(cx, |tela, cx| tela.trocar_acervo(todas, cx));

        // E o catálogo é relido assim mesmo: o que mudou no banco desde a última
        // leitura entra junto quando chegar.
        self.reler_o_acervo(cx);
    }

    /// Escreve, sobre o que a API respondeu, a revelação que **ainda não
    /// subiu** — o depósito local ganha da galeria.
    ///
    /// 🔑 **É a mesma regra do editor do site** (`editor.tsx`: o que está no
    /// depósito e não está sincronizado é o que volta para os sliders). A foto
    /// que o operador revelou ontem e não salvou tem de abrir hoje como ele a
    /// deixou; abrir com o que o servidor tem seria mostrar o trabalho de
    /// anteontem e chamar isso de "a foto".
    ///
    /// ⚠️ **Só o que está no depósito é tocado.** A foto sem linha lá abre com a
    /// receita da API, que é a verdade dela — e é o caso da grande maioria.
    fn aplicar_o_deposito_do_site(&mut self) {
        let guardadas = self.gravador.guardadas_do_site();
        if guardadas.is_empty() {
            return;
        }
        for (no_site, ajustes) in guardadas {
            let Some(foto) = self
                .fotos_do_site
                .iter_mut()
                .find(|f| f.pos_venda_foto_id.as_deref() == Some(no_site.as_str()))
            else {
                continue;
            };
            // JSON estragado não apaga a receita da API: ele é ignorado, e a
            // foto abre com o que o servidor tem — que é pior que o depósito e
            // muito melhor que o neutro.
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&ajustes) else {
                continue;
            };
            let (ajustes, corte) = persistencia::de_json(&json);
            persistencia::na_foto(foto, ajustes, corte);
        }
    }

    /// Sai do ensaio: a grade volta a ser o catálogo, e nada mais trabalha.
    pub fn sair_da_sessao(&mut self, cx: &mut Context<Self>) {
        self.sessao_aberta = None;
        self.fotos_do_site.clear();
        // A fila é do ensaio que estava aberto: levá-la para o próximo mandaria
        // ao site fotos de outro cliente no primeiro "Salvar na galeria" de lá.
        self.a_subir.clear();
        self.biblioteca
            .update(cx, |tela, cx| tela.escopar_na_sessao(None, cx));
        self.tela = Tela::Sessoes;
        cx.notify();
    }

    /// O que a tela da sessão pede — ela não troca de tela nem abre a Revelação.
    pub(crate) fn atender_a_sessao(
        &mut self,
        pedido: &DetalhePedido,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pedido {
            DetalhePedido::Voltar => {
                self.tela = Tela::Sessoes;
                cx.notify();
            }
            DetalhePedido::Revelar { fotos, inicial } => {
                self.revelar_da_sessao(fotos, *inicial, window, cx);
            }
            DetalhePedido::TelaDoCliente => self.alternar_cliente(cx),
            DetalhePedido::Exportar => self.exportar(cx),
            // 🔑 **A importação da sessão grava no catálogo local**, e as fotos
            // só aparecem depois desta releitura — a porta do acervo é daqui.
            DetalhePedido::CatalogoMudou => self.reler_o_acervo(cx),
            // 🔑 **A Biblioteca é quem classifica**, mesmo quando o gesto veio
            // da grade da sessão: é ela que grava a nota, lê a travessia do zero
            // e emite `Classificou` — que é o que faz a foto subir (passo 3).
            DetalhePedido::Classificar { ids, nota } => {
                let (ids, nota) = (ids.clone(), *nota);
                self.biblioteca
                    .update(cx, |tela, cx| tela.classificar_ids(&ids, nota, cx));
            }
            DetalhePedido::MiniaturaPronta(chave) => {
                let chave = chave.clone();
                self.biblioteca
                    .update(cx, |tela, cx| tela.esquecer_miniatura(&chave, cx));
            }
            DetalhePedido::FotosDoSite(fotos) => {
                let sessao = self.sessao_aberta.clone();
                let convertidas = fotos
                    .iter()
                    .map(|f| do_site_para_a_grade(f, sessao.clone()))
                    .collect();
                self.absorver_as_do_site(convertidas, cx);
            }
        }
    }

    /// Entra na Revelação com **a sessão na tira**, aberta em `inicial`.
    ///
    /// 🔑 **A lista inteira vai junto**, como nos dois botões da web
    /// (`abrir-revelacao.tsx`): a tira, as setas e os botões do editor percorrem
    /// o ensaio, e "a próxima" é a próxima do ensaio — sem voltar à grade a cada
    /// foto. Até 7/set/2026 a sessão mandava **uma** foto, e a Revelação abria
    /// com uma tira de uma.
    ///
    /// 🚨 **A grade tem duas famílias, e tratá-las igual foi o defeito de
    /// 8/set/2026 repetido uma tela adiante.** A foto do site entra como
    /// `PhotoViewModel` sem caminho e com o id remoto (`site:…`), e daí o passo
    /// 11 busca a cópia de trabalho no storage. A foto que **só está no disco**
    /// entra como ela é no catálogo: id cru, caminho do arquivo, receita já
    /// gravada.
    ///
    /// Antes disto, toda foto da sessão virava foto do site: o id do catálogo
    /// ganhava `site:` na frente, o caminho era zerado, e a Revelação ia pedir
    /// à nuvem uma cópia de trabalho de uma foto que nunca subiu. Abria em "não
    /// tem preview no cache" — com o JPEG e o preview no disco, a um passo — e
    /// com a tira inteira preta, porque a miniatura dela também mora sob o id
    /// cru. Foi o que o dono viu com as 21 fotos importadas do ensaio.
    ///
    /// ⚠️ **O id `site:` não colide com o do catálogo**, e é isso que faz as
    /// duas famílias caberem na mesma tira: são espaços de nome diferentes, e
    /// misturá-los faria a Revelação gravar ajustes numa foto que ninguém abriu.
    fn revelar_da_sessao(
        &mut self,
        fotos: &[FotoARevelar],
        inicial: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 🚨 A mesma guarda de `revelar`: logado e sem sessão, nada trabalha.
        if !self.pode_trabalhar() {
            return;
        }
        let sessao_id = self.sessao_aberta.clone();
        let do_catalogo = self.biblioteca.read(cx).todas_as_fotos();
        let acervo: Vec<PhotoViewModel> = fotos
            .iter()
            .map(|foto| {
                // 🔑 **A local vale pela linha do catálogo**, e não por uma
                // cópia montada aqui: é ela que carrega o caminho do arquivo, a
                // receita já gravada e o id sob o qual as previews estão no
                // cache. Recriá-la à mão perderia os três de uma vez.
                if foto.no_disco {
                    if let Some(local) = do_catalogo.iter().find(|f| f.id == foto.id) {
                        return local.clone();
                    }
                }
                let id = format!("{}{}", persistencia::PREFIXO_DO_SITE, foto.id);
                self.fotos_do_site
                    .iter()
                    .find(|da_grade| da_grade.id == id)
                    .cloned()
                    .unwrap_or_else(|| PhotoViewModel {
                        id,
                        name: foto.arquivo.clone(),
                        pos_venda_foto_id: Some(foto.id.clone()),
                        sessao_id: sessao_id.clone(),
                        ..Default::default()
                    })
            })
            .collect();
        if acervo.is_empty() {
            return;
        }
        // Os pixels do storage são pedidos por `AbriuOutraFoto`, que o
        // `abrir_no_acervo` emite — o mesmo caminho da seta e da tira.
        self.revelacao.update(cx, |tela, cx| {
            tela.abrir_no_acervo(acervo, inicial, window, cx)
        });
        self.tela = Tela::Revelacao;
        self.recontar_o_que_falta_subir(cx);
        // O foco volta para a raiz a cada troca de tela — ver `revelar`.
        window.focus(&self.foco);
        cx.notify();
    }

    /// De onde vêm os pixels que faltam — a decisão, antes de ir buscá-los.
    ///
    /// 🔑 **Duas origens, e a foto diz qual é a dela.** Com `path` preenchido o
    /// original está neste disco e o que faltou foi o **cache** (apagado nas
    /// Configurações, catálogo copiado sem o `Previews.lrdata`, importação que
    /// gravou a foto e não o preview) — refazer é ler o arquivo. Sem `path` a
    /// foto só existe no site, e o bruto dela vem da cópia de trabalho: é o
    /// passo 11, que já existia.
    ///
    /// 🚨 **A local não tinha saída nenhuma até aqui.** A Revelação dizia "não
    /// tem preview no cache" e parava — com o JPEG de 6 MB no disco, a um
    /// decode de distância. O beco sem saída era o defeito, não a ausência.
    fn repor_os_pixels(&mut self, cx: &mut Context<Self>) {
        // 🔑 **A tira entra mesmo com o palco cheio.** Quando "Limpar
        // miniaturas" leva só as pequenas, a foto aberta desenha e a tira fica
        // uma fileira de retângulos pretos — sem isto, ninguém repõe nunca,
        // porque a única pergunta feita era sobre o palco.
        let a_repor: Vec<APor> = self
            .revelacao
            .read(cx)
            .fotos_a_repor()
            .into_iter()
            .filter(|foto| !self.reposicoes_pedidas.contains(&foto.foto_id))
            .collect();
        if !a_repor.is_empty() {
            self.refazer_o_cache_do_disco(a_repor, cx);
        }

        let revelacao = self.revelacao.read(cx);
        if revelacao.tem_pixels() {
            return;
        }
        // Sem arquivo neste disco, os pixels só existem no storage: passo 11.
        // Com arquivo, a reposição acima já está a caminho, e a foto do palco é
        // a primeira da fila.
        if revelacao.foto_aberta().is_some_and(|f| f.path.is_empty()) {
            self.buscar_os_pixels_na_nuvem(cx);
        }
    }

    /// Manda refazer o cache das fotos que o disco ainda tem.
    ///
    /// ⚠️ **Fora da thread da interface, sempre.** Decodificar um JPEG de 6 MB
    /// custa dezenas de milissegundos, e um `--release` esconde o quanto: em
    /// `debug` são 57× mais (`CLAUDE.md`), o bastante para a janela travar
    /// visivelmente — e aqui não é uma foto, é a tira inteira.
    fn refazer_o_cache_do_disco(&mut self, a_repor: Vec<APor>, cx: &mut Context<Self>) {
        // Só o palco tem frase a mostrar. Se ele já está desenhado, a reposição
        // é da tira e acontece em silêncio, célula a célula.
        if !self.revelacao.read(cx).tem_pixels() {
            self.revelacao
                .update(cx, |tela, cx| tela.avisar_que_os_pixels_vem_vindo(cx));
        }
        self.reposicoes_pendentes += a_repor.len();
        self.reposicoes_pedidas
            .extend(a_repor.iter().map(|foto| foto.foto_id.clone()));
        self.repositor.repor(a_repor, self.reposicoes.0.clone());
        self.esperar_a_reposicao(cx);
    }

    /// Espera o disco responder — **uma foto de cada vez, até a última**.
    ///
    /// 🚨 **O laço não pode morrer na primeira resposta.** É a mesma lição de
    /// `esperar_a_sincronia`, e aqui ela vale ainda mais: as respostas chegam
    /// espalhadas no tempo por construção, uma por decodificação, e desligar na
    /// primeira deixaria a tira acender uma célula e parar.
    ///
    /// ⏱️ O teto é de 60 s por foto esperada — um RAW é o pior caso conhecido e
    /// cabe folgado.
    fn esperar_a_reposicao(&mut self, cx: &mut Context<Self>) {
        let voltas = 600 * self.reposicoes_pendentes.max(1);
        self._reposicao = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..voltas {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(acabou) = raiz.update(cx, |raiz, cx| {
                    raiz.colher_reposicao(cx);
                    raiz.reposicoes_pendentes == 0
                }) else {
                    return;
                };
                if acabou {
                    return;
                }
            }
        }));
    }

    /// Drena o que o disco respondeu, e pinta o que chegou.
    pub(crate) fn colher_reposicao(&mut self, cx: &mut Context<Self>) -> bool {
        let mut chegou = false;
        while let Ok(recado) = self.reposicoes.1.try_recv() {
            chegou = true;
            self.reposicoes_pendentes = self.reposicoes_pendentes.saturating_sub(1);
            // Respondida, sai da lembrança: se ela voltar a faltar (o cache
            // apagado de novo), a varredura seguinte pode pedi-la outra vez.
            self.reposicoes_pedidas.remove(match &recado {
                ReposicaoRecado::Reposta { foto_id, .. } => foto_id,
                ReposicaoRecado::Falhou { foto_id, .. } => foto_id,
            });
            match recado {
                ReposicaoRecado::Reposta { foto_id, imagem } => {
                    // 🔑 **O mesmo `receber_pixels` do passo 11**, e não um
                    // caminho paralelo: é ele que confere se a foto ainda é a
                    // do palco. Uma reposição que termina depois de a seta ter
                    // andado pintaria a foto errada — e ficaria bonita, que é o
                    // que torna esse defeito caro.
                    //
                    // `false` é o caso comum aqui, e não é falha: são as fotos
                    // da tira, que ninguém está olhando de perto. Elas só
                    // precisam que a célula releia o cache.
                    let era_do_palco = self
                        .revelacao
                        .update(cx, |tela, cx| tela.receber_pixels(&foto_id, *imagem, cx));
                    if !era_do_palco {
                        self.revelacao
                            .update(cx, |tela, cx| tela.miniatura_reposta(&foto_id, cx));
                    }
                }
                ReposicaoRecado::Falhou { foto_id, erro } => {
                    // 🔑 **O disco falhou, mas pode haver cópia no site.** É a
                    // foto que já subiu e cujo arquivo daqui saiu de baixo —
                    // movida para outro disco, cartão desmontado, pasta
                    // renomeada. Ela tem `path` (por isso veio parar aqui) e tem
                    // id no site, então o passo 11 ainda responde.
                    //
                    // ⚠️ **Só quando o palco está vazio, e só para a foto dele.**
                    // A tira também passa por aqui, e uma rede por miniatura
                    // que faltou transformaria "abriu numa pasta antiga" em
                    // duzentos downloads.
                    let e_o_palco_vazio = {
                        let revelacao = self.revelacao.read(cx);
                        !revelacao.tem_pixels()
                            && revelacao.foto_aberta().is_some_and(|f| f.id == foto_id)
                    };
                    // 🚨 **Não achou saída, a tela para de prometer.** Ela volta
                    // a dizer "não tem preview no cache", que passou a ser a
                    // verdade — um "Preparando…" eterno é pior do que a frase
                    // seca, porque quem lê continua esperando.
                    //
                    // ⚠️ E só a falha **da foto do palco** apaga a promessa: a
                    // dele é a primeira da fila, então uma da tira falhando
                    // enquanto ele espera não é motivo para desistir por ele.
                    if e_o_palco_vazio && !self.buscar_os_pixels_na_nuvem(cx) {
                        self.revelacao
                            .update(cx, |tela, cx| tela.desistir_dos_pixels(cx));
                    }
                    self.avisar_onde_esta_olhando(erro, cx);
                }
            }
        }
        chegou
    }

    /// O passo 11 do fluxo: os pixels que só existem no storage.
    ///
    /// 🔑 **Só quando não há nada local.** A cópia de trabalho do site custa uma
    /// ida à rede por foto; pedir sempre transformaria a revelação em série —
    /// que é como se revela um casamento — em duzentos downloads que o disco já
    /// tinha respondido.
    ///
    /// ⚠️ **E só depois de a Revelação ter tentado abrir.** É ela quem sabe se o
    /// cache local tinha alguma coisa; perguntar antes seria adivinhar.
    ///
    /// Devolve se o download foi mesmo pedido — quem chama precisa saber, para
    /// não deixar a tela em "Preparando…" esperando o que ninguém foi buscar.
    fn buscar_os_pixels_na_nuvem(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(sessao) = self.sessao().cloned() else {
            return false;
        };
        let revelacao = self.revelacao.read(cx);
        if revelacao.tem_pixels() {
            return false;
        }
        let Some(foto) = revelacao.foto_aberta() else {
            return false;
        };
        let (Some(no_site), local) = (foto.pos_venda_foto_id.clone(), foto.id.clone()) else {
            return false;
        };

        self.publicador
            .copia_de_trabalho(sessao, local, no_site, self.sincronias.0.clone());
        self.esperar_a_sincronia(1, cx);
        true
    }

    /// Despacha um gesto de triagem para **a grade que está na frente**.
    ///
    /// 🚨 **São duas grades, e nunca as duas ao mesmo tempo**: a do ensaio, que
    /// mostra o que está no site, e a Biblioteca, que é o catálogo desta
    /// máquina. As teclas da legenda
    /// (`1`–`5`, `P`, `Ctrl+A`, `Ctrl+D`, setas) valem nas duas — o que muda é
    /// quem responde.
    fn na_grade(
        &mut self,
        cx: &mut Context<Self>,
        na_sessao: impl FnOnce(&mut Detalhe, &mut Context<Detalhe>),
        na_biblioteca: impl FnOnce(&mut Biblioteca, &mut Context<Biblioteca>),
    ) {
        match self.tela {
            Tela::Sessao => self.detalhe.update(cx, na_sessao),
            Tela::Biblioteca => self.biblioteca.update(cx, na_biblioteca),
            _ => {}
        }
    }

    /// O passo 6 do fluxo: **o cliente paga no balcão**.
    ///
    /// ⚠️ **Leva a seleção, e não a grade inteira** — ao contrário de publicar.
    /// Negociação é acerto sobre fotos específicas; aplicá-la ao que estivesse
    /// visível daria cortesia a duzentas fotos por um filtro mal escolhido.
    pub fn abrir_balcao(&mut self, cx: &mut Context<Self>) {
        // 🚨 **Logado, nada acontece fora de uma sessão.** A guarda fica aqui, e
        // não só no botão: atalho de teclado chega antes de botão, e foi assim
        // que a nota caiu numa grade que ninguém estava vendo.
        if !self.pode_trabalhar() {
            return;
        }
        let fotos = self.biblioteca.read(cx).fotos_selecionadas();
        if fotos.is_empty() {
            return;
        }
        self.balcao
            .update(cx, |tela, cx| tela.abrir_para(fotos, cx));
        self.no_balcao = true;
        cx.notify();
    }

    pub fn fechar_balcao(&mut self, cx: &mut Context<Self>) {
        self.no_balcao = false;
        // O que foi registrado mudou a foto no site, e a grade mostra o selo do
        // que está lá: reler é o que faz a mudança aparecer.
        self.reler_o_acervo(cx);
        cx.notify();
    }

    pub fn no_balcao(&self) -> bool {
        self.no_balcao
    }

    /// O passo 3 do fluxo: o que ganhou nota sobe, o que a perdeu sai.
    ///
    /// ⚠️ **Sem galeria aberta não sobe nada, e a tela diz isso.** Classificar
    /// 200 fotos e descobrir no balcão que nenhuma foi para o site é o desfecho
    /// que este aviso existe para impedir — silêncio aqui seria pior que erro.
    fn sincronizar_classificacao(&mut self, evento: Classificou, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao().cloned() else {
            // Sem conta não se chega aqui: a porta vem antes de tudo, e a
            // Biblioteca não é desenhada enquanto ela não for respondida.
            return;
        };

        // Tirar do site não precisa de galeria aberta: a foto já sabe onde está.
        for id in &evento.sairam {
            self.publicador
                .tirar_do_site(sessao.clone(), id.clone(), self.sincronias.0.clone());
        }
        // Uma resposta por pedido: é o que o laço de espera conta.
        let mut esperadas = evento.sairam.len();

        if !evento.subiram.is_empty() {
            let Some(galeria) = self.sessao_aberta.clone() else {
                let quantas = evento.subiram.len();
                self.biblioteca.update(cx, |tela, cx| {
                    tela.avisar(
                        format!(
                            "{quantas} foto(s) classificada(s) e nenhuma sessão aberta —                              escolha uma em Sessões para elas subirem"
                        ),
                        cx,
                    )
                });
                self.esperar_a_sincronia(esperadas, cx);
                return;
            };

            for (ordem, id) in evento.subiram.iter().enumerate() {
                self.publicador.subir_classificada(
                    sessao.clone(),
                    galeria.clone(),
                    crate::pos_venda::porta::FotoClassificada {
                        foto_id: id.clone(),
                        ordem: ordem as u32,
                        // 🔑 `None`: quem classifica não escolheu leva nenhuma,
                        // e o estado sai da tecla `B` de cada foto. A escolha
                        // por lote existe na tela da sessão, onde ela é o gesto.
                        estado: None,
                        // 🚨 **A nota vem do evento, e não do banco.** Ver
                        // `Classificou::nota`: a gravação dela ainda pode estar
                        // correndo quando o envio lê a linha da foto.
                        nota: evento.nota,
                    },
                    self.sincronias.0.clone(),
                );
            }
            esperadas += evento.subiram.len();
        }

        self.esperar_a_sincronia(esperadas, cx);
    }

    /// Espera o site responder e relê o catálogo quando alguma foto muda de
    /// lado — é o que traz o id remoto para a tela.
    ///
    /// `quantas` é o número de pedidos que acabaram de sair. Ele **soma** ao que
    /// já estava pendente, e o laço só desliga quando a conta zera: quem manda
    /// dez fotos espera dez respostas, e não uma.
    ///
    /// ⏱️ O teto é de 30 s por resposta esperada. Um lote de dez fotos pode
    /// levar minutos — cada uma baixa o original, revela na GPU e sobe — e um
    /// teto fixo cortaria o lote pela metade, em silêncio.
    fn esperar_a_sincronia(&mut self, quantas: usize, cx: &mut Context<Self>) {
        self.sincronias_pendentes += quantas;
        let voltas = 300 * self.sincronias_pendentes.max(1);
        self._sincronia = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..voltas {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(acabou) = raiz.update(cx, |raiz, cx| {
                    raiz.colher_sincronia(cx);
                    raiz.sincronias_pendentes == 0
                }) else {
                    return;
                };
                if acabou {
                    return;
                }
            }
        }));
    }

    /// Drena o que o site respondeu. Devolve se não há mais o que esperar.
    pub(crate) fn colher_sincronia(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.sincronias.1.try_recv() {
            // 🔑 **Todo recado fecha o pedido que o gerou.** É a conta que
            // mantém o laço de pé até a última foto do lote responder — e o
            // `saturating_sub` é o que impede uma resposta a mais (um recado
            // que ninguém pediu) de a fazer dar a volta.
            self.sincronias_pendentes = self.sincronias_pendentes.saturating_sub(1);
            match recado {
                PosVendaRecado::Sincronizou => mudou = true,
                PosVendaRecado::Pixels { foto_id, bytes } => {
                    // 🚨 Decodificar pode falhar — resposta truncada, formato
                    // que o `image` não lê. Falhar aqui deixa a foto como
                    // estava (vazia), que é o mesmo desfecho de não ter pedido:
                    // ruim, e honesto.
                    match image::load_from_memory(&bytes) {
                        Ok(imagem) => {
                            // 🔑 **Vai para o cache antes de ir para a tela.** É
                            // o que faz a seta de volta não pagar outro
                            // download: o L1 responde na hora e o L2 (SQLite)
                            // atravessa o fechar do app. A chave é a do bruto,
                            // separada da miniatura da galeria — ver
                            // `persistencia::chave_do_trabalho`.
                            //
                            // ⚠️ Falha de gravação não impede de mostrar: o
                            // cache é acelerador, e a foto na mão é o que o
                            // operador pediu.
                            let _ = self
                                .previews
                                .save_preview(&persistencia::chave_do_trabalho(&foto_id), &imagem);
                            let aproveitou = self
                                .revelacao
                                .update(cx, |tela, cx| tela.receber_pixels(&foto_id, imagem, cx));
                            if !aproveitou {
                                // A seta andou enquanto o download vinha. Não é
                                // erro: é o motivo de o id vir junto.
                            }
                        }
                        Err(erro) => {
                            self.biblioteca.update(cx, |tela, cx| {
                                tela.avisar(format!("a foto do site não abriu: {erro}"), cx)
                            });
                        }
                    }
                }
                // 🔑 O revelado entrou no lugar do original: a sessão relê,
                // e é a releitura que troca a miniatura da grade pela foto
                // revelada. Sem ela o operador salvaria e continuaria vendo o
                // "antes" — o pior desfecho, porque parece que não salvou.
                PosVendaRecado::RevelacaoSalva { foto_no_site } => {
                    // 🔑 **Subiu: sai do depósito.** A partir daqui a receita
                    // desta foto é a do servidor, e deixá-la também aqui faria
                    // a abertura seguinte preferir uma cópia que ninguém mais
                    // atualiza — a foto voltaria ao que era antes do envio no
                    // dia em que a galeria mudasse por outra tela.
                    self.a_subir.retain(|(ja, _, _)| ja != &foto_no_site);
                    self.gravador.esquecer_do_site(foto_no_site);
                    self.recontar_o_que_falta_subir(cx);
                    self.avisar_onde_esta_olhando("revelação salva na galeria".into(), cx);
                    if let Some(galeria) = self.sessao_aberta.clone() {
                        self.detalhe.update(cx, |tela, cx| tela.entrar(galeria, cx));
                    }
                    mudou = true;
                }
                PosVendaRecado::Falhou(erro) => {
                    self.avisar_onde_esta_olhando(erro, cx);
                }
                // Os outros recados são de quem os pediu: esta raiz só sincroniza.
                _ => {}
            }
        }
        if mudou {
            // 🔑 A releitura é o que traz o id remoto para a tela. Sem ela a foto
            // está no site e a grade não sabe — e o próximo gesto que dependa
            // disso (tirar do storage, negociar) não teria em quem cair.
            self.reler_o_acervo(cx);
        }
        // Uma só rodada de colheita por resposta: quem manda mais fotos religa
        // o laço.
        mudou
    }

    pub fn tela(&self) -> Tela {
        self.tela
    }

    /// Quantas respostas do site ainda faltam — é o que mantém o laço de pé.
    #[cfg(test)]
    pub(crate) fn sincronias_pendentes(&self) -> usize {
        self.sincronias_pendentes
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
        // 🚨 **Logado, nada acontece fora de uma sessão.** A guarda fica aqui, e
        // não só no botão: atalho de teclado chega antes de botão, e foi assim
        // que a nota caiu numa grade que ninguém estava vendo.
        if !self.pode_trabalhar() {
            return;
        }
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

        // 📸 Passo 11 — os pixels do storage, quando o cache não tem o bruto —
        // é pedido por `AbriuOutraFoto`, que `abrir_no_acervo` emite.
        self.revelacao.update(cx, |tela, cx| {
            tela.abrir_no_acervo(acervo, posicao, window, cx)
        });
        self.tela = Tela::Revelacao;
        self.recontar_o_que_falta_subir(cx);
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
        // 🚨 Logado, nada acontece fora de uma sessão — ver `pode_trabalhar`.
        if !self.pode_trabalhar() {
            return;
        }
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
    /// Volta para a grade — que é a do ensaio quando há um.
    ///
    /// 🔑 **São quatro telas** (decisão do dono, 6/set/2026): a lista de
    /// sessões, a sessão (onde se escolhe com o cliente e se negocia), a
    /// revelação e a impressão. "Biblioteca" não é uma delas: ela é a grade
    /// **dentro** da sessão — o catálogo desta máquina, recortado pelo ensaio
    /// que está aberto.
    pub fn voltar_para_biblioteca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());
        self.guardar_as_receitas_do_site(cx);
        self.tela = if self.sessao_aberta.is_some() {
            Tela::Sessao
        } else {
            Tela::Biblioteca
        };
        window.focus(&self.foco);
        cx.notify();
    }

    /// Traz de volta, para as fotos do site que a raiz guarda, o que a
    /// Revelação ajustou nelas.
    ///
    /// 🚨 **A foto do site não tem linha no catálogo**: o id dela é
    /// `site:<uuid>`, e `save_edits` responde `PhotoNotFound` — calado, porque o
    /// `Gravador` não devolve `Result`. Então o único lugar em que a receita
    /// dela existe é a cópia em memória, e a cópia da Revelação morre quando a
    /// tela fecha. Sem isto, revelar uma foto do site e apertar Esc perdia tudo:
    /// a próxima abertura remontava o acervo a partir de `fotos_do_site`, que
    /// veio da API.
    ///
    /// ⚠️ **O disco é a outra metade, e é o `Gravador` quem cuida dela**: cada
    /// gesto numa foto do site vai para `revelacoes_do_site`, no mesmo SQLite do
    /// catálogo. Isto aqui é o espelho da tela; aquilo é o que atravessa o
    /// fechar do app.
    fn guardar_as_receitas_do_site(&mut self, cx: &mut Context<Self>) {
        if self.fotos_do_site.is_empty() {
            return;
        }
        let receitas: Vec<(String, Ajustes, persistencia::Corte)> = self
            .revelacao
            .read(cx)
            .acervo()
            .iter()
            .filter(|f| persistencia::so_existe_no_site(f))
            .map(|f| {
                (
                    f.id.clone(),
                    persistencia::da_foto(f),
                    persistencia::corte_da_foto(f),
                )
            })
            .collect();
        for (id, ajustes, corte) in receitas {
            if let Some(foto) = self.fotos_do_site.iter_mut().find(|f| f.id == id) {
                persistencia::na_foto(foto, ajustes, corte);
            }
        }
    }

    /// Atende os três botões da barra da Revelação que não são dela.
    ///
    /// 🔑 **São os dois últimos botões do editor do site, com os nomes de lá**:
    /// "Baixar JPEG" (aqui, "Exportar JPEG", porque no desktop ele escreve num
    /// disco) e "Salvar na galeria e sair". Valem para a foto aberta, que na
    /// Revelação **é** a seleção — foi ela que trouxe o operador até aqui.
    pub(crate) fn atender_a_revelacao(
        &mut self,
        pedido: PedidoDaRevelacao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pedido {
            PedidoDaRevelacao::Sair => self.voltar_para_biblioteca(window, cx),
            PedidoDaRevelacao::Exportar => self.exportar(cx),
            PedidoDaRevelacao::SalvarNaGaleria => self.salvar_na_galeria(window, cx),
            PedidoDaRevelacao::Sincronizar => self.sincronizar_revelacao(window, cx),
            // 📸 Passo 11 a cada troca de foto, e não só na abertura: se o
            // cache local não tem o bruto e a foto está no site, os pixels vêm
            // de lá. A pergunta é feita **depois** de a Revelação abrir a
            // foto, porque é ela quem sabe se sobrou vazio.
            PedidoDaRevelacao::AbriuOutraFoto => self.repor_os_pixels(cx),
        }
    }

    /// Leva os ajustes da foto aberta para as outras marcadas na tira — o
    /// "Sincronizar" do Lightroom e do site (pedido do dono, 2026-09-05).
    ///
    /// # O que acontece com cada marcada
    ///
    /// A **receita** dela recebe os grupos escolhidos na caixa e vai para o
    /// catálogo; o que não foi escolhido fica como estava, e o enquadramento
    /// só viaja se o operador o ligou (`sincronizacao::mesclar`). É o que
    /// `colar_revelacao` já fazia sem flags — e com a mesma regra: **cada
    /// foto conserva o próprio corte**, porque gravar sem reenviá-lo apaga o
    /// enquadramento.
    ///
    /// # 🚨 Ele copia parâmetros, e não revela nada
    ///
    /// Até 2026-09-11 a foto que já estava no site era **revelada e subida**
    /// aqui mesmo: baixar o original em resolução cheia, decodificar 24 MP,
    /// codificar um JPEG e enviá-lo — por foto. Sincronizar sete custava
    /// minutos, e o dono estranhou com razão: *"não faz sentido, é somente uma
    /// casca de parâmetros que é passado"*. A web foi corrigida no mesmo dia
    /// (`sincronizar` em `editor.tsx`), e o gesto tem de custar o mesmo nas
    /// duas.
    ///
    /// O trabalho pesado não sumiu — mudou para onde ele é inevitável: o
    /// "Salvar na galeria", que agora sobe a aberta **e** as que ficaram no
    /// depósito (`Gravador::guardadas_do_site`). Até lá a receita está gravada,
    /// a tira e a grade já mostram o resultado, e o cliente continua vendo o
    /// JPEG de antes — que é exatamente o que "ainda não salvei" significa.
    pub fn sincronizar_revelacao(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // 🚨 Logado, nada acontece fora de uma sessão — ver `pode_trabalhar`.
        if !self.pode_trabalhar() {
            return;
        }
        // O gesto a meio caminho fecha antes: a receita que viaja é a que o
        // operador está vendo, inclusive o arrasto de meio segundo atrás.
        self.revelacao
            .update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());

        let (aberta, ajustes, corte, escolha, alvos) = {
            let revelacao = self.revelacao.read(cx);
            (
                revelacao.foto_aberta().cloned(),
                revelacao.ajustes(),
                revelacao.corte(),
                revelacao.escolha_da_sincronizacao(),
                revelacao.alvos_da_sincronizacao(),
            )
        };
        let Some(aberta) = aberta else {
            return;
        };
        if alvos.len() < 2 || !escolha.tem_algo() {
            return;
        }

        let mut gravadas: Vec<(String, Ajustes, persistencia::Corte)> = Vec::new();
        for alvo in &alvos {
            // A aberta já está no banco com o que está na tela: reescrevê-la
            // aqui seria gravar o que o autosave acabou de gravar.
            if alvo.id == aberta.id {
                continue;
            }
            let finais = sincronizacao::mesclar(persistencia::da_foto(alvo), ajustes, &escolha);
            let corte_final = if escolha.enquadramento {
                corte
            } else {
                persistencia::corte_da_foto(alvo)
            };
            // 🚨 **Só a receita.** Para a que só existe no site isto a põe no
            // depósito; para a do catálogo, no catálogo. Nos dois casos o que
            // está no servidor continua sendo o JPEG de antes — e é por isso
            // que o aviso abaixo diz onde elas estão.
            self.gravador.gravar(alvo.id.clone(), finais, corte_final);
            gravadas.push((alvo.id.clone(), finais, corte_final));

            // A que já tem lugar na galeria entra na fila de envio.
            if let Some(no_site) = alvo.pos_venda_foto_id.clone() {
                self.enfileirar_para_subir(
                    no_site,
                    finais,
                    persistencia::para_crop_settings(&corte_final),
                );
            }
        }

        // 🔑 As cópias da tira e a grade ficaram velhas: sem isto a seta
        // seguinte abriria a foto sincronizada com os sliders de antes.
        self.revelacao
            .update(cx, |tela, cx| tela.aplicar_sincronizadas(&gravadas, cx));
        self.reler_o_acervo(cx);
        self.recontar_o_que_falta_subir(cx);
        self.avisar_onde_esta_olhando(
            format!(
                "{} foto(s) receberam estes ajustes — elas sobem quando você salvar na galeria",
                gravadas.len()
            ),
            cx,
        );
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
        // 🚨 Logado, nada acontece fora de uma sessão — ver `pode_trabalhar`.
        if !self.pode_trabalhar() {
            return;
        }
        if let Some(janela) = self.cliente.take() {
            // Fechar é remover a janela. Se ela já não existe (o `Esc` de dentro
            // dela chegou primeiro), o `update` devolve erro e não há o que
            // fazer além de esquecer o handle — que é o que o `take` já fez.
            let _ = janela.update(cx, |_cliente, window, _cx| window.remove_window());
            self.detalhe
                .update(cx, |tela, cx| tela.definir_cliente_aberta(false, cx));
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
                self.detalhe
                    .update(cx, |tela, cx| tela.definir_cliente_aberta(true, cx));
                let posicao = self.biblioteca.read(cx).posicao_da_selecao();
                self.mostrar_ao_cliente(&foto, posicao, cx);
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
    fn mostrar_ao_cliente(
        &mut self,
        foto: &PhotoViewModel,
        posicao: Option<(usize, usize)>,
        cx: &mut Context<Self>,
    ) {
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
                cliente.mostrar(Some(foto), imagem, posicao, cx);
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
    /// Abre a importação. Dentro de um ensaio, o lote entra **nele**.
    pub fn importar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 🚨 **Logado, nada acontece fora de uma sessão.** A guarda fica aqui, e
        // não só no botão: atalho de teclado chega antes de botão, e foi assim
        // que a nota caiu numa grade que ninguém estava vendo.
        if !self.pode_trabalhar() {
            return;
        }
        self.importando = true;
        // 🚨 **O lote entra no ensaio aberto.** Sem este carimbo a foto chega ao
        // catálogo sem dono e não aparece na grade da sessão que a importou — e
        // o sintoma é "a importação não funcionou".
        let sessao = self.sessao_aberta.clone();
        self.importacao
            .update(cx, |tela, _cx| tela.importar_para_a_sessao(sessao));
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
        // 🚨 **Logado, nada acontece fora de uma sessão.** A guarda fica aqui, e
        // não só no botão: atalho de teclado chega antes de botão, e foi assim
        // que a nota caiu numa grade que ninguém estava vendo.
        if !self.pode_trabalhar() {
            return;
        }
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

    /// **Salvar na galeria e sair** — o botão do editor, no fluxo do site.
    ///
    /// 🚨 **Não abre modal nenhum, e é essa a correção.** Até 7/set/2026 este
    /// gesto abria "Publicar no pós-venda", que **cria galeria nova**: quem
    /// entrava na revelação a partir de uma sessão levava na cara um formulário
    /// pedindo título, e-mail e produto de uma galeria que já existia — com a
    /// frase "nenhum produto no catálogo" por cima. A rota
    /// `/dashboard/sessoes-fotograficas/{id}/revelacao` não tem esse popup: lá o
    /// botão grava o revelado **na foto que já é da galeria**, e volta.
    ///
    /// # A foto que ainda não subiu
    ///
    /// Ela não vai para o site — é a mesma regra do editor da web
    /// (`salvarNaAreaTemporaria`), e o motivo é do dono: foto sem classificação
    /// indica que o cliente não gostou. O que este caminho faz é gravar os
    /// ajustes; ela sobe já revelada quando ganhar nota.
    ///
    /// # E as que o "Sincronizar" deixou pendentes
    ///
    /// 🔑 **Sobem aqui, junto.** Desde 2026-09-11 o "Sincronizar" copia só a
    /// receita (ver [`Self::sincronizar_revelacao`]), e este é o momento em que
    /// revelar é inevitável: o que estiver no depósito e for desta sessão entra
    /// no mesmo lote da aberta. Sem isto, sincronizar seria um botão que grava
    /// numa gaveta que ninguém esvazia — e o cliente continuaria vendo o JPEG
    /// de antes sem ninguém perceber.
    ///
    /// # Por que sai antes de o site responder
    ///
    /// Porque o que se perderia é nada: os ajustes já foram para o banco local,
    /// e uma falha volta como aviso na tela da sessão — de onde o operador abre
    /// a foto e salva de novo. Segurar o editor aberto por uma ida à rede de
    /// segundos, no meio de uma revelação em série, custaria mais do que
    /// protege.
    pub fn salvar_na_galeria(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 🚨 Logado, nada acontece fora de uma sessão — ver `pode_trabalhar`.
        if !self.pode_trabalhar() {
            return;
        }

        let Some(foto) = self
            .revelacao
            .read(cx)
            .foto_aberta()
            .map(|f| (f.id.clone(), f.pos_venda_foto_id.clone()))
        else {
            return;
        };
        let (ajustes, corte) = {
            let revelacao = self.revelacao.read(cx);
            (revelacao.ajustes(), revelacao.enquadramento())
        };
        let (_local, no_site) = foto;

        // O que está pendente vai para o banco antes de sair — é o que
        // `voltar_para_biblioteca` já faria, e aqui ele precisa vir **antes** do
        // envio: os mesmos ajustes que sobem ficam gravados aqui.
        self.voltar_para_biblioteca(window, cx);

        let Some(sessao) = self.sessao().cloned() else {
            return;
        };

        // A aberta primeiro, quando ela é do site; e em seguida as que o
        // "Sincronizar" deixou só com a receita, das duas procedências.
        let mut lote: Vec<(String, Ajustes, CropSettings)> = Vec::new();
        if let Some(no_site) = no_site.clone() {
            lote.push((no_site, ajustes, corte));
        }
        // 🚨 **A fila não é esvaziada aqui, e sim quando a foto chega.** Quem a
        // tira é o `RevelacaoSalva`, e a que falhou fica — senão um lote com
        // uma rede ruim no meio apagaria da lista a foto que não subiu, e o
        // número no botão diria que já não há o que salvar.
        for (outra, ajustes_dela, corte_dela) in self.a_subir.clone() {
            if lote.iter().any(|(ja, _, _)| ja == &outra) {
                continue;
            }
            lote.push((outra, ajustes_dela, corte_dela));
        }
        for (outra, json) in self.pendentes_do_site() {
            if lote.iter().any(|(ja, _, _)| ja == &outra) {
                continue;
            }
            let Ok(valor) = serde_json::from_str::<serde_json::Value>(&json) else {
                continue;
            };
            let (ajustes_dela, corte_dela) = persistencia::de_json(&valor);
            lote.push((
                outra,
                ajustes_dela,
                persistencia::para_crop_settings(&corte_dela),
            ));
        }

        if lote.is_empty() {
            self.avisar_onde_esta_olhando(
                "revelação guardada — ela sobe revelada quando a foto for classificada".into(),
                cx,
            );
            return;
        }

        let quantas = lote.len();
        for (no_site, ajustes, corte) in lote {
            self.publicador.salvar_revelacao(
                sessao.clone(),
                no_site,
                ajustes,
                corte,
                self.sincronias.0.clone(),
            );
        }
        self.avisar_onde_esta_olhando(
            if quantas == 1 {
                "salvando a revelação na galeria…".into()
            } else {
                format!("salvando {quantas} revelações na galeria…")
            },
            cx,
        );
        self.esperar_a_sincronia(quantas, cx);
    }

    /// Põe uma foto da galeria na fila de envio — a última receita ganha.
    ///
    /// 🔑 **Uma entrada por foto.** Sincronizar duas vezes seguidas com ajustes
    /// diferentes mandaria a foto duas vezes ao site, a segunda desfazendo a
    /// primeira depois de dois downloads e dois JPEGs — e a ordem de chegada
    /// decidiria o que o cliente vê.
    fn enfileirar_para_subir(&mut self, no_site: String, ajustes: Ajustes, corte: CropSettings) {
        self.a_subir.retain(|(ja, _, _)| ja != &no_site);
        self.a_subir.push((no_site, ajustes, corte));
    }

    /// Quantas fotos deste ensaio esperam ir ao site — o número no botão de
    /// salvar da Revelação.
    ///
    /// 🚨 **É a conta do lote, e não a das listas.** O número aparece no botão
    /// "Salvar na galeria", e ele tem de dizer quantas fotos aquele clique
    /// manda — por isso a aberta entra (ela sempre vai) e por isso as duas
    /// procedências contam uma vez só: a foto que só existe no site está na
    /// fila **e** no depósito depois de um sincronizar.
    fn quantas_a_subir(&self, aberta: Option<&str>) -> usize {
        let mut ids: Vec<&str> = self.a_subir.iter().map(|(id, _, _)| id.as_str()).collect();
        let do_deposito = self.pendentes_do_site();
        ids.extend(do_deposito.iter().map(|(id, _)| id.as_str()));
        ids.extend(aberta);
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// Reescreve o número no botão "Salvar na galeria" da Revelação.
    fn recontar_o_que_falta_subir(&mut self, cx: &mut Context<Self>) {
        let aberta = self
            .revelacao
            .read(cx)
            .foto_aberta()
            .and_then(|f| f.pos_venda_foto_id.clone());
        let quantas = self.quantas_a_subir(aberta.as_deref());
        self.revelacao
            .update(cx, |tela, cx| tela.definir_nao_salvas(quantas, cx));
    }

    /// As revelações **desta sessão** que ainda não subiram, por id do site.
    ///
    /// 🔑 **O depósito é do app inteiro, e o lote é da galeria aberta.** Quem
    /// revelou ontem o ensaio de outro cliente e não salvou não pode ver aquelas
    /// fotos subirem porque hoje clicou em salvar aqui — é o mesmo recorte que o
    /// `salvarESair` da web faz ao filtrar por `fotos` em vez de pelo depósito.
    fn pendentes_do_site(&self) -> Vec<(String, String)> {
        self.gravador
            .guardadas_do_site()
            .into_iter()
            .filter(|(no_site, _)| {
                self.fotos_do_site
                    .iter()
                    .any(|f| f.pos_venda_foto_id.as_deref() == Some(no_site.as_str()))
            })
            .collect()
    }

    /// Põe o aviso na tela que está na frente.
    ///
    /// 🔑 A Biblioteca sempre teve `avisar`, e ela era o único destino — mas
    /// depois de salvar na galeria quem está olhando é a **sessão**, e o aviso
    /// caía numa tela que ninguém estava vendo.
    fn avisar_onde_esta_olhando(&mut self, texto: String, cx: &mut Context<Self>) {
        match self.tela {
            Tela::Sessao => self.detalhe.update(cx, |tela, cx| tela.recado(texto, cx)),
            _ => self
                .biblioteca
                .update(cx, |tela, cx| tela.avisar(texto, cx)),
        }
    }

    /// A porta foi respondida: o app passa a existir.
    ///
    /// 🔑 **A sessão desce para as telas aqui**, e não é pedida de novo em
    /// nenhuma delas: entrar duas vezes na mesma conta, na mesma abertura, é
    /// atrito puro.
    pub fn entrar_na_conta(
        &mut self,
        sessao: domain::services::pos_venda::Sessao,
        cx: &mut Context<Self>,
    ) {
        // A lista já pede as sessões: quem entrou vai querer ver em qual
        // galeria está trabalhando antes de qualquer outra coisa.
        self.sessoes
            .update(cx, |tela, cx| tela.definir_sessao(sessao.clone(), cx));
        self.balcao
            .update(cx, |tela, cx| tela.definir_sessao(sessao.clone(), cx));
        self.detalhe
            .update(cx, |tela, _cx| tela.definir_sessao(sessao.clone()));
        // 🔑 **Entrou: a primeira tela é a lista de sessões.** Na web é de
        // onde tudo parte, e abrir no catálogo global foi o que fez o app
        // parecer "aberto e estranho" para quem vinha de lá.
        self.tela = Tela::Sessoes;
        self.sessao = Some(sessao);
        cx.notify();
    }

    /// A conta do site, quando o app já passou da porta.
    pub fn sessao(&self) -> Option<&domain::services::pos_venda::Sessao> {
        self.sessao.as_ref()
    }

    /// Se o app já passou da porta.
    pub fn entrou(&self) -> bool {
        self.sessao.is_some()
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
    pub(crate) fn na_biblioteca(
        &mut self,
        cx: &mut Context<Self>,
        acao: impl FnOnce(&mut Biblioteca, &mut Context<Biblioteca>),
    ) {
        // A Biblioteca é o catálogo local; a grade do ensaio mostra o que está
        // no site, e quem despacha para ela é `na_grade`.
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
            // baixo, e "a próxima" não quer dizer nada sobre uma folha. Nas
            // Sessões, pelo mesmo motivo: a lista se percorre com a busca.
            // Na sessão as setas andam na grade dela — é o que a legenda da
            // tira promete.
            Tela::Sessao => self.detalhe.update(cx, |tela, cx| tela.andar(passo, cx)),
            Tela::Impressao | Tela::Sessoes => {}
        }
    }

    /// Uma linha para cima ou para baixo, na grade que estiver no ar.
    ///
    /// 🔑 **Quem sabe quantas colunas cabem é aqui**, e não a tela: a conta
    /// depende da largura da janela, que é do `Window`. Cada grade tem a sua —
    /// a da sessão divide a linha com o painel da foto, a da Biblioteca com a
    /// árvore de pastas.
    ///
    /// ⚠️ **Na Revelação as setas ↑↓ não andam.** Lá não há linha: a foto é uma
    /// só, e ← → já percorrem a tira. Andar de sete em sete numa lista de uma
    /// dimensão seria um salto sem sentido na tela.
    fn andar_linha(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        match self.tela {
            Tela::Biblioteca => {
                let colunas = self.biblioteca.read(cx).colunas_visiveis(window);
                self.biblioteca
                    .update(cx, |tela, cx| tela.andar_linha(passo, colunas, cx))
            }
            Tela::Sessao => {
                let colunas = self.detalhe.read(cx).colunas_visiveis(window);
                self.detalhe
                    .update(cx, |tela, cx| tela.andar_linha(passo, colunas, cx))
            }
            Tela::Revelacao | Tela::Impressao | Tela::Sessoes => {}
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

    /// Se há uma sessão aberta — sem ela, nada trabalha.
    ///
    /// 🚨 **Tudo acontece dentro de uma sessão** — regra do dono, 6/set/2026:
    /// importar, revelar, escolher com o cliente, exportar e gerar o link são
    /// gestos *sobre um ensaio*, e não sobre um catálogo solto. Fora dela só
    /// existe a lista, que é onde se escolhe em qual entrar.
    ///
    /// ⚠️ **E não há mais a saída pela qual isto era opcional.** O botão
    /// "trabalhar offline" caiu em 6/set/2026, no mesmo dia em que nasceu: o
    /// propósito do app é a integração com o pós-venda, e catálogo solto não
    /// pertence a ensaio nenhum. Sem rede volta como sincronização.
    ///
    /// 🔑 **A impressão entra na regra pelo mesmo motivo que o resto**, e o dono
    /// disse por quê: revelação e emolduramento vão virar **produtos com custo**
    /// dentro do ensaio. Uma folha impressa fora de uma sessão seria trabalho
    /// que ninguém tem como cobrar — e o lugar de descobrir isso não é depois de
    /// o cliente sair.
    ///
    /// 📌 **E é por isso que a trava é o pré-requisito de uma coisa que ainda não
    /// existe**: o dono avisou em 6/set/2026 que o sistema vai **contabilizar
    /// pedidos de revelação**. Contar quantas revelações um ensaio teve só é
    /// possível se toda revelação pertencer a um ensaio — e é exatamente isso
    /// que esta guarda passa a garantir, antes de haver o que contar.
    pub fn pode_trabalhar(&self) -> bool {
        self.sessao_aberta.is_some()
    }

    /// O título da sessão aberta, para a barra dizer onde se está.
    fn nome_da_sessao(&self, cx: &Context<Self>) -> Option<SharedString> {
        self.detalhe
            .read(cx)
            .aberta()
            .map(|a| SharedString::from(a.galeria.titulo.clone()))
    }

    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Paridade com o app de egui: o botão de Revelação só liga quando há
        // seleção na Biblioteca ("Develop button enabled if Library has a
        // selection", `app.rs`).
        let tem_selecao = self.biblioteca.read(cx).foto_selecionada().is_some();
        let na_revelacao = self.tela == Tela::Revelacao;
        let na_impressao = self.tela == Tela::Impressao;
        // 🚨 Logado e sem sessão aberta, **nada** trabalha: os botões existem
        // desligados em vez de sumirem, porque sumir esconderia o app inteiro e
        // deixaria a impressão de que ele quebrou.
        let trabalhando = self.pode_trabalhar();
        let tem_selecao = tem_selecao && trabalhando;

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(6.))
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            // ── Onde estou ────────────────────────────────────────────────
            //
            // 🔑 **A barra é feita de grupos separados por divisor**, e não de
            // botões iguais em fila. Eles não fazem coisas da mesma natureza:
            // navegar entre telas e mexer no dinheiro do cliente — e quando tudo
            // tem o mesmo peso, achar o que se quer custa uma varredura da barra
            // inteira, toda vez. O grupo do meio, "o que faço com a foto", era o
            // terceiro; ele saiu em 8/set/2026 e o vão abaixo diz para onde.
            .child(
                grupo()
                    // 🚨 A saída da sessão vem primeiro — e ela é a única coisa
                    // que a barra oferece antes de haver uma sessão aberta.
                    //
                    // 🚨 **Dentro de um ensaio não há aba de biblioteca**, e é o
                    // ponto que custou mais para eu entender: a grade do ensaio
                    // já está na tela, logo abaixo do cabeçalho. Uma aba
                    // "Escolher com o cliente" ao lado dizia que a escolha
                    // acontece em outro lugar — que é exatamente o equívoco.
                    .child(
                        Button::new("nav-sessoes")
                            .label(if self.sessao_aberta.is_some() {
                                "← Sessões"
                            } else {
                                "Sessões"
                            })
                            .xsmall()
                            .when(self.tela == Tela::Sessoes, |b| b.primary())
                            .selected(self.tela == Tela::Sessoes)
                            .on_click(cx.listener(|este, _ev, _window, cx| {
                                este.sair_da_sessao(cx);
                            })),
                    )
                    .when_some(self.nome_da_sessao(cx), |grupo, nome| {
                        grupo.child(
                            // O ensaio aberto é da família âmbar: é o contexto
                            // do cliente, e não uma tela a mais.
                            Button::new("nav-sessao-aberta")
                                .label(nome)
                                .xsmall()
                                .when(self.tela == Tela::Sessao, |b| {
                                    b.custom(tema::botao_quente(cx))
                                })
                                .selected(self.tela == Tela::Sessao)
                                .on_click(cx.listener(|este, _ev, _window, cx| {
                                    este.tela = Tela::Sessao;
                                    cx.notify();
                                })),
                        )
                    })
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
                        // Paridade: no legado o botão de impressão também só
                        // liga com seleção (`selected_photo_ids` ou a foto da
                        // Biblioteca).
                        Button::new("nav-impressao")
                            .label("Impressão")
                            .xsmall()
                            .when(na_impressao, |b| b.primary())
                            .selected(na_impressao)
                            .disabled(!tem_selecao || !trabalhando)
                            .on_click(cx.listener(|este, _ev, window, cx| {
                                este.imprimir(window, cx);
                            })),
                    ),
            )
            // ── O vão até o cliente ───────────────────────────────────────
            //
            // 🚨 **O grupo "o que faço com a foto" saiu daqui em 8/set/2026**, a
            // pedido do dono, e com ele o rótulo "VintageLightbox" que ocupava
            // este vão. Os quatro botões diziam coisas que a sessão já diz
            // melhor, ou não dizia nenhuma:
            //
            // - **Copiar e Colar revelação** são gestos *da* Revelação, e a
            //   barra não existe lá (`self.tela != Tela::Revelacao`). Ficavam
            //   apagados o tempo todo, prometendo um trabalho que dali não
            //   começava. As teclas continuam — `⌘C`/`⌘V`, em `ao_copiar_revelacao`.
            // - **Importar** abria o explorador *do framework*, ao lado de um
            //   "Escolher fotos…" que usa o do sistema. Dois botões para o mesmo
            //   gesto, e o certo era o de baixo: *"é o Escolher fotos… que faz a
            //   ação correta"*. O de baixo herdou o nome, e hoje é o "Importar"
            //   da tela da sessão.
            // - **Exportar** desceu para junto dele, na tela da sessão: entrar e
            //   sair de um ensaio são o mesmo par de gestos, e estavam a uma
            //   barra de distância um do outro.
            // - O **título** era papel de parede: fora da Revelação ele só sabia
            //   escrever o nome do app, e a barra nunca aparece dentro dela.
            .child(div().flex_1())
            // ── O cliente e o dinheiro ────────────────────────────────────
            //
            // 🔑 **Tudo o que atravessa para o site é âmbar**, e fica junto no
            // fim da barra: balcão, publicação e a tela que o cliente vê. É a
            // separação que o azul sozinho não fazia — "aplicar na foto" e
            // "cobrar do cliente" tinham a mesma cor e a mesma vizinhança.
            .child(
                grupo()
                    .child(
                        // 🔑 Só com seleção: negociação é acerto sobre fotos
                        // específicas, e a grade inteira não é uma escolha.
                        Button::new("nav-balcao")
                            .label("Balcão")
                            .xsmall()
                            .custom(tema::botao_quente(cx))
                            .disabled(!tem_selecao)
                            .on_click(cx.listener(|este, _ev, _window, cx| {
                                este.abrir_balcao(cx);
                            })),
                    )
                    // 📸 **Publicar cria uma galeria nova, e não tem botão** —
                    // dentro de um ensaio o gesto é subir para *esta* sessão, e
                    // dois caminhos para o mesmo lugar com desfechos diferentes
                    // é a forma mais cara de confundir. O modal continua, e
                    // quem o abre é o fluxo do balcão; a galeria nova nasce na
                    // lista de sessões.
                    .child(
                        // A segunda tela. Só liga com seleção, como a Revelação
                        // e a Impressão: mostrar preto ao cliente não diz "não
                        // escolhi nada", diz "quebrou".
                        Button::new("nav-cliente")
                            .label("Segunda tela")
                            .xsmall()
                            .ghost()
                            .when(self.cliente.is_some(), |b| b.custom(tema::botao_quente(cx)))
                            .selected(self.cliente.is_some())
                            .disabled(!tem_selecao)
                            .on_click(cx.listener(|este, _ev, _window, cx| {
                                este.alternar_cliente(cx);
                            })),
                    ),
            )
            .child(divisor(cx))
            .child(
                Button::new("nav-configuracoes")
                    .label("Configurações")
                    .xsmall()
                    .ghost()
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
            .bg(tema::cores::veu())
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
            .bg(tema::cores::veu())
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
            .bg(tema::cores::veu())
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

    fn modal_do_balcao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(tema::cores::veu())
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
                            .child(div().text_xs().child("Pago no balcão"))
                            .child(
                                Button::new("fechar-balcao")
                                    .label("Fechar")
                                    .xsmall()
                                    .on_click(cx.listener(|este, _ev, _window, cx| {
                                        este.fechar_balcao(cx);
                                    })),
                            ),
                    )
                    .child(self.balcao.clone()),
            )
    }

    fn modal_de_importacao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(tema::cores::veu())
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
        // 🚨 A porta vem antes de tudo, inclusive das teclas: com o app inteiro
        // desenhado por baixo, as quinze teclas de triagem continuariam
        // chegando à Biblioteca por trás da tela de login.
        if self.sessao.is_none() {
            return div()
                .size_full()
                .child(self.entrada.clone())
                .into_any_element();
        }

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
            .on_action(cx.listener(|este, _: &Abaixo, window, cx| {
                este.andar_linha(1, window, cx);
            }))
            .on_action(cx.listener(|este, _: &Acima, window, cx| {
                este.andar_linha(-1, window, cx);
            }))
            .on_action(cx.listener(|este, _: &SemNota, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(0, cx),
                    |grade, cx| grade.dar_nota(0, cx),
                )
            }))
            .on_action(cx.listener(|este, _: &UmaEstrela, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(1, cx),
                    |grade, cx| grade.dar_nota(1, cx),
                )
            }))
            .on_action(cx.listener(|este, _: &DuasEstrelas, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(2, cx),
                    |grade, cx| grade.dar_nota(2, cx),
                )
            }))
            .on_action(cx.listener(|este, _: &TresEstrelas, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(3, cx),
                    |grade, cx| grade.dar_nota(3, cx),
                )
            }))
            .on_action(cx.listener(|este, _: &QuatroEstrelas, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(4, cx),
                    |grade, cx| grade.dar_nota(4, cx),
                )
            }))
            .on_action(cx.listener(|este, _: &CincoEstrelas, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.dar_nota(5, cx),
                    |grade, cx| grade.dar_nota(5, cx),
                )
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
            .on_action(cx.listener(|este, _: &AlternarComprada, _w, cx| {
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.alternar_levada(cx),
                    |grade, cx| grade.marcar_comprada(cx),
                )
            }))
            // 🔑 Na Revelação, `Cmd+A` e `Cmd+D` são da **tira**: marcam o
            // lote da sincronização, como no site — e nenhum dos dois troca a
            // foto aberta.
            .on_action(cx.listener(|este, _: &SelecionarTudo, _w, cx| {
                if este.tela == Tela::Revelacao {
                    este.revelacao.update(cx, |tela, cx| tela.marcar_todas(cx));
                    return;
                }
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.selecionar_tudo(cx),
                    |grade, cx| grade.selecionar_tudo(cx),
                )
            }))
            .on_action(cx.listener(|este, _: &LimparSelecao, _w, cx| {
                if este.tela == Tela::Revelacao {
                    este.revelacao.update(cx, |tela, cx| tela.desmarcar(cx));
                    return;
                }
                este.na_grade(
                    cx,
                    |sessao, cx| sessao.limpar_selecao(cx),
                    |grade, cx| grade.limpar_selecao(cx),
                )
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            // 🚨 **A barra do app some na Revelação**, e é o que faz a tela ser
            // a do site: lá o editor é `fixed inset-0` e cobre a janela — não há
            // navegação por cima dele. O caminho de volta é o `✕` da barra dela,
            // que faz o mesmo que o `Esc`.
            .children((self.tela != Tela::Revelacao).then(|| self.barra(cx)))
            .child(
                // `min_h(0)` no contêiner da tela: sem ele, o conteúdo rolável
                // de dentro empurra o pai e a rolagem nunca acontece. Foi a
                // mesma correção que a linha de pastas + grade precisou.
                div().flex().flex_1().min_h(px(0.)).child(match self.tela {
                    Tela::Biblioteca => self.biblioteca.clone().into_any_element(),
                    Tela::Revelacao => self.revelacao.clone().into_any_element(),
                    Tela::Impressao => self.impressao.clone().into_any_element(),
                    Tela::Sessoes => self.sessoes.clone().into_any_element(),
                    // 🚨 **A sessão é uma tela só**, com tudo dentro: cabeçalho,
                    // envio, barra, grade, painel da foto e a tira. É a rota
                    // `[id]` do site, e foi o que o dono pediu ao mandar a
                    // imagem dela: *"não invente nada"*.
                    Tela::Sessao => self.detalhe.clone().into_any_element(),
                }),
            )
            .when(self.importando, |raiz| {
                raiz.child(self.modal_de_importacao(cx))
            })
            .when(self.exportando, |raiz| {
                raiz.child(self.modal_de_exportacao(cx))
            })
            .when(self.no_balcao, |raiz| raiz.child(self.modal_do_balcao(cx)))
            .when_some(
                self.biblioteca.read(cx).confirmando_apagar(),
                |raiz, quantas| raiz.child(self.aviso_de_apagar(quantas, cx)),
            )
            .when(false, |raiz| raiz)
            .when(self.configurando, |raiz| {
                raiz.child(self.modal_de_configuracoes(cx))
            })
            // 🔑 **Depois dos modais, e por cima deles.** A faixa é a última
            // coisa desenhada de propósito: ela ocupa uma linha do rodapé e
            // precisa continuar legível com o modal de importação aberto — que
            // é justamente quando um download longo termina.
            .when_some(self.faixa_de_atualizacao(cx), |raiz, faixa| {
                raiz.child(faixa)
            })
            .into_any_element()
    }
}

/// Uma foto **do site** na linguagem da grade.
///
/// 🔑 **A grade é uma só**, como na web: as locais e as do acervo aparecem
/// juntas. Para isso a foto do site precisa falar `PhotoViewModel` — e o que ela
/// não tem (caminho no disco, ajustes locais) fica vazio de propósito.
///
/// ⚠️ **O id leva o prefixo `site:`** e não colide com o do catálogo: são
/// espaços de nome diferentes, e misturá-los faria a Revelação gravar ajustes
/// numa foto local que ninguém abriu.
/// Uma foto **do disco** na linguagem da grade da sessão.
///
/// 🔑 **Ela nasce "à venda" e sem nota**, e os dois são deliberados: `Estado` só
/// sabe falar do que existe no site (levada · à venda · comprada), e destes o
/// único honesto para quem ainda não subiu é o neutro. A nota vazia é o que a
/// põe no recorte "Sem nota" — o lugar de onde o operador a classifica, que é o
/// gesto que a faz subir.
fn local_para_a_grade(foto: &PhotoViewModel, ordem: i64) -> biblioteca_core::acervo::Foto {
    biblioteca_core::acervo::Foto {
        id: foto.id.clone(),
        arquivo: foto.name.clone(),
        estado: biblioteca_core::acervo::Estado::Disponivel,
        apagada: false,
        produto_efetivo: String::new(),
        preco_negociado: None,
        tem_observacao: false,
        preco_de_venda: None,
        pedido_id: None,
        downloads: 0,
        revelada: persistencia::ja_revelada(foto),
        nota: None,
        ordem,
    }
}

fn do_site_para_a_grade(
    foto: &domain::services::pos_venda::FotoDaGaleria,
    sessao_id: Option<String>,
) -> PhotoViewModel {
    use domain::services::pos_venda::EstadoDaFotoNoSite;
    let mut vm = PhotoViewModel {
        id: format!("{}{}", persistencia::PREFIXO_DO_SITE, foto.id),
        name: foto.arquivo.clone(),
        // Sem caminho: ela não está no disco desta máquina.
        path: String::new(),
        rating: foto.nota.unwrap_or(0) as i32,
        // A levada no balcão é a que o cliente já pagou na hora — é o que a
        // tecla `B` marca do lado de cá.
        comprada: matches!(
            foto.estado,
            EstadoDaFotoNoSite::LevadaNoBalcao | EstadoDaFotoNoSite::Comprada
        ),
        pos_venda_foto_id: Some(foto.id.clone()),
        sessao_id,
        ..Default::default()
    };
    // 🔑 **A receita vem junto.** É o `completar(foto.ajustes)` do editor do
    // site: sem isto a foto já revelada abria aqui com os 53 sliders no
    // neutro, e "sincronizar" a partir dela mandava o neutro às outras.
    if let Some(json) = &foto.ajustes {
        let (ajustes, corte) = persistencia::de_json(json);
        persistencia::na_foto(&mut vm, ajustes, corte);
    }
    vm
}

/// A conta de teste. **Um só lugar constrói `Sessao` neste arquivo**: a struct
/// vive no domínio e ganha campo quando a autorização muda, e cinco cópias da
/// mesma literal é cinco lugares para consertar por campo novo.
/// Uma sessão fotográfica como a listagem do site a devolve — só o id importa
/// nos testes que apenas precisam que ela exista.
#[cfg(test)]
fn galeria_do_painel(id: &str) -> domain::services::pos_venda::GaleriaDoPainel {
    domain::services::pos_venda::GaleriaDoPainel {
        id: id.into(),
        titulo: "Ensaio".into(),
        email: Some("cliente@exemplo.com".into()),
        whatsapp: None,
        produto_id: "p1".into(),
        user_id: None,
        criada_em_iso: "2026-09-07".into(),
        expira_em: None,
        fotos: Default::default(),
        totais: None,
    }
}

#[cfg(test)]
fn sessao_de_teste() -> domain::services::pos_venda::Sessao {
    domain::services::pos_venda::Sessao {
        access_token: "tok".into(),
        refresh_token: "renova".into(),
        // Longe: teste que renova sozinho no meio do caminho é teste que falha
        // por relógio, e não pelo que ele diz conferir.
        access_vence_em: 4_102_444_800,
        refresh_vence_em: 4_102_444_800,
    }
}

/// O app já do lado de dentro da porta, e com um ensaio aberto.
///
/// 🔑 **Existe para os testes que não são sobre a porta**, que são todos menos
/// um: sem ele, os 33 testes de tecla e de modal passariam a exercitar a tela
/// de login, porque é só ela que o `render` desenha enquanto ninguém entrou.
/// A porta em si é conferida por `a_porta_vem_antes_de_tudo`.
///
/// 🚨 **A sessão aberta faz parte do "dentro"**, e não é conveniência de teste:
/// desde que o botão "trabalhar offline" caiu (6/set/2026), nada trabalha fora
/// de um ensaio — `pode_trabalhar` é `sessao_aberta.is_some()`. Um app logado e
/// sem ensaio é a lista de sessões, e é o que `nada_acontece_fora_de_uma_sessao`
/// confere.
#[cfg(test)]
impl Aplicativo {
    #[allow(clippy::too_many_arguments)]
    pub fn ja_dentro(
        fotos: Vec<PhotoViewModel>,
        previews: Arc<PreviewManager>,
        presets: Vec<Preset>,
        portas: Portas,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut app = Self::novo(fotos, previews, presets, portas, window, cx);
        app.sessao = Some(sessao_de_teste());
        // Direto no campo, sem `entrar_na_sessao`: a grade destes testes é o
        // acervo que eles montaram, e não o que o site devolveria para "g1".
        app.sessao_aberta = Some("g1".into());
        app
    }
}

/// Um grupo de botões da barra: eles se tocam, e o divisor separa do próximo.
fn grupo() -> gpui::Div {
    div().flex().items_center().gap(px(2.))
}

/// A linha entre dois grupos da barra.
///
/// 🔑 **Um pixel, e não um espaço maior.** Espaço separa quando há pouca coisa;
/// com a barra cheia, o que separa é a linha — e ela custa 1px de largura em vez
/// dos 12 que o respiro pediria.
fn divisor(cx: &gpui::App) -> gpui::Div {
    div().w(px(1.)).h(px(16.)).flex_none().bg(cx.theme().border)
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use crate::atualizacao::porta::mentira::AtualizadorDeMentira;
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
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use crate::revelacao::lightroom::mentira::EscolhaDeMentira;
    use crate::revelacao::persistencia::mentira::GravadorDeMentira;
    use crate::revelacao::presets::mentira::GuardaDeMentira;
    use crate::revelacao::reposicao::mentira::RepositorDeMentira;

    /// As portas de mentira, que é o que quase todo teste daqui quer.
    fn portas() -> Portas {
        Portas {
            gravador: Arc::new(GravadorDeMentira::default()),
            acervo: Arc::new(AcervoDeMentira::default()),
            exportador: Arc::new(ExportadorDeMentira::default()),
            publicador: Arc::new(PublicadorDeMentira::default()),
            colecoes: Arc::new(ColecoesDeMentira::default()),
            folha: Arc::new(FolhaDeMentira::default()),
            marcador: Arc::new(MarcadorDeMentira::default()),
            gerador: Arc::new(GeradorDeMentira::default()),
            // O padrão de mentira não devolve imagem nenhuma: quem quiser
            // afirmar sobre a reposição troca esta porta por
            // `RepositorDeMentira::que_devolve`.
            repositor: Arc::new(RepositorDeMentira::default()),
            guarda_de_presets: Arc::new(GuardaDeMentira::default()),
            escolha_de_presets: Arc::new(EscolhaDeMentira::default()),
            explorador: Arc::new(ExploradorDeMentira::default()),
            importador: Arc::new(ImportadorDeMentira::default()),
            seletor: Arc::new(SeletorDeMentira::default()),
            seletor_de_fotos: Arc::new(
                crate::sessoes::arquivos::mentira::SeletorDeMentira::default(),
            ),
            // Silêncio: o padrão do atualizador de mentira é não achar nada, e
            // é o que quase todo teste daqui quer — a faixa fora do caminho.
            atualizador: Arc::new(AtualizadorDeMentira::default()),
        }
    }

    /// As mesmas portas, com uma versão nova esperando para ser vista.
    fn portas_com_versao(versao: &str) -> (Portas, Arc<AtualizadorDeMentira>) {
        let atualizador = Arc::new(AtualizadorDeMentira::com_versao(versao));
        (
            Portas {
                atualizador: atualizador.clone(),
                ..portas()
            },
            atualizador,
        )
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

    /// Uma foto do site como a API a devolve, com ou sem receita.
    fn foto_do_site(
        id: &str,
        ajustes: Option<serde_json::Value>,
    ) -> domain::services::pos_venda::FotoDaGaleria {
        domain::services::pos_venda::FotoDaGaleria {
            id: id.into(),
            arquivo: format!("{id}.jpg"),
            estado: domain::services::pos_venda::EstadoDaFotoNoSite::Disponivel,
            ordem: 0,
            preco_negociado: None,
            observacao_da_negociacao: None,
            apagada: false,
            nota: Some(4),
            produto_efetivo: "p1".into(),
            preco_de_venda: None,
            pedido_id: None,
            downloads: 0,
            revelada: ajustes.is_some(),
            ajustes,
        }
    }

    /// 🚨 **O cenário do dono, 7/set/2026: "a sincronização não funciona".**
    ///
    /// Três fotos do site na tira, a primeira já revelada em sépia. Ela abria
    /// com os sliders no neutro (a receita ficava no `http.rs`) e mostrando a
    /// miniatura revelada da galeria como se fosse o bruto; "Sincronizar 3"
    /// mandava esse neutro às outras duas — e nada mudava. Este teste anda o
    /// caminho inteiro: abrir, trocar de foto pela seta, voltar, sincronizar.
    #[gpui::test]
    fn sincronizar_a_partir_da_foto_do_site_leva_a_receita_dela_e_nao_o_neutro(
        cx: &mut TestAppContext,
    ) {
        use crate::revelacao::sincronizacao::Escolha;
        use crate::sessoes::detalhe::FotoARevelar;

        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let publicador = Arc::new(PublicadorDeMentira::default());

        let janela = cx.add_window({
            let publicador = publicador.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(
                    Vec::new(),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let a_revelar = |id: &str| FotoARevelar {
            id: id.into(),
            arquivo: format!("{id}.jpg"),
            no_disco: false,
        };

        janela
            .update(cx, |app, window, cx| {
                // A sessão respondeu: a primeira já foi revelada em sépia.
                app.atender_a_sessao(
                    &DetalhePedido::FotosDoSite(vec![
                        foto_do_site(
                            "remota-1",
                            Some(serde_json::json!({
                                "saturation": -1.0,
                                "split_shadow_hue": 35,
                                "split_shadow_sat": 45
                            })),
                        ),
                        foto_do_site("remota-2", None),
                        foto_do_site("remota-3", None),
                    ]),
                    window,
                    cx,
                );
                app.atender_a_sessao(
                    &DetalhePedido::Revelar {
                        fotos: vec![
                            a_revelar("remota-1"),
                            a_revelar("remota-2"),
                            a_revelar("remota-3"),
                        ],
                        inicial: 0,
                    },
                    window,
                    cx,
                );
                assert_eq!(app.tela(), Tela::Revelacao);

                let revelacao = app.revelacao.read(cx);
                assert_eq!(
                    revelacao.ajustes().saturation,
                    -1.0,
                    "a revelada abre com os sliders no lugar, como no site"
                );
                assert!(
                    !revelacao.tem_pixels(),
                    "e não usa a miniatura da galeria como origem"
                );
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
                assert!(
                    app.revelacao.read(cx).tem_pixels(),
                    "a cópia de trabalho chegou do storage"
                );
            })
            .expect("a janela deve estar aberta");
        assert_eq!(publicador.baixadas(), vec!["remota-1".to_string()]);

        // 🔑 A seta também busca o bruto — antes só a abertura buscava, e a
        // segunda foto ficava com a miniatura de 640px como origem.
        janela
            .update(cx, |app, window, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.andar(1, window, cx));
            })
            .expect("a janela deve estar aberta");
        janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
            })
            .expect("a janela deve estar aberta");
        assert_eq!(
            publicador.baixadas(),
            vec!["remota-1".to_string(), "remota-2".to_string()],
            "trocar de foto pede o bruto da nova"
        );

        // De volta à revelada: marca as três e sincroniza.
        janela
            .update(cx, |app, window, cx| {
                app.revelacao.update(cx, |tela, cx| {
                    tela.andar(-1, window, cx);
                    assert_eq!(tela.ajustes().saturation, -1.0, "a receita voltou com ela");
                    tela.marcar_todas(cx);
                    tela.definir_escolha_da_sincronizacao(Escolha::default());
                    assert_eq!(tela.alvos_da_sincronizacao().len(), 3);
                });
                app.sincronizar_revelacao(window, cx);
            })
            .expect("a janela deve estar aberta");

        // 🚨 **Sincronizar não sobe nada** — ele copia a receita, e só. Era o
        // "muito lento" de 11/set/2026: três fotos, três downloads do original
        // e três JPEGs de 24 MP para um gesto que é uma casca de parâmetros.
        assert!(
            publicador.reveladas().is_empty(),
            "o sincronizar revelou e subiu: {:?}",
            publicador.reveladas()
        );

        // E é o "Salvar na galeria" que esvazia a gaveta: a aberta e as duas
        // que ficaram só com a receita sobem no mesmo lote.
        janela
            .update(cx, |app, window, cx| {
                app.salvar_na_galeria(window, cx);
            })
            .expect("a janela deve estar aberta");

        let reveladas = publicador.reveladas();
        assert_eq!(reveladas.len(), 3, "as três foram ao site: {reveladas:?}");
        for (id, ajustes, _) in &reveladas {
            assert_eq!(
                ajustes.saturation, -1.0,
                "{id} recebe a receita da aberta — era o neutro que ia"
            );
            assert_eq!(ajustes.split_shadow_hue, 35.0, "{id}");
        }
    }

    /// 🚨 **Revelar uma foto do site, sair e voltar: a receita tem de estar
    /// lá.**
    ///
    /// Achado do dono em 8/set: *"parece que os parâmetros de edição não estão
    /// sendo gravados"*. E não estavam mesmo — em lugar nenhum. O id da foto do
    /// site é `site:<uuid>`, que não é linha do catálogo: `save_edits` responde
    /// `PhotoNotFound` e o `Gravador` engole (não devolve `Result`, de
    /// propósito). O que restava era a cópia em memória do acervo da Revelação,
    /// que morre com a tela.
    ///
    /// O teste anda o gesto inteiro: revela, sai pela barra, e manda revelar de
    /// novo — que remonta o acervo a partir de `fotos_do_site`.
    #[gpui::test]
    fn revelar_a_foto_do_site_sair_e_voltar_traz_a_receita(cx: &mut TestAppContext) {
        use crate::sessoes::detalhe::FotoARevelar;

        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let publicador = Arc::new(PublicadorDeMentira::default());

        let janela = cx.add_window({
            let publicador = publicador.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(
                    Vec::new(),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        let revelar = |app: &mut Aplicativo, window: &mut Window, cx: &mut Context<Aplicativo>| {
            app.atender_a_sessao(
                &DetalhePedido::Revelar {
                    fotos: vec![FotoARevelar {
                        id: "remota-1".into(),
                        arquivo: "DSC_001.jpg".into(),
                        no_disco: false,
                    }],
                    inicial: 0,
                },
                window,
                cx,
            );
        };

        janela
            .update(cx, |app, window, cx| {
                app.atender_a_sessao(
                    &DetalhePedido::FotosDoSite(vec![foto_do_site("remota-1", None)]),
                    window,
                    cx,
                );
                revelar(app, window, cx);
                // O operador mexe num slider e sai pela barra.
                app.revelacao
                    .update(cx, |tela, cx| tela.aplicar_para_teste(0, 1.25, cx));
                app.atender_a_revelacao(PedidoDaRevelacao::Sair, window, cx);
                assert_eq!(app.tela(), Tela::Sessao);

                revelar(app, window, cx);
                assert_eq!(
                    app.revelacao.read(cx).ajustes().exposure,
                    1.25,
                    "a revelacao da foto do site nao sobreviveu a sair da tela"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **O depósito local ganha da galeria — e sai quando a foto sobe.**
    ///
    /// É a última metade do *"os parâmetros de edição não estão sendo
    /// gravados"* (dono, 8/set/2026): a receita de uma foto do site que ainda
    /// não subiu tem de atravessar o fechar do app. Ela mora em
    /// `revelacoes_do_site`, no mesmo SQLite do catálogo, e é lida na abertura;
    /// aqui o `GravadorDeMentira` faz o papel de "o app achou isto lá".
    ///
    /// O teste anda as duas pontas que sobram: a leitura vencendo a API, e a
    /// linha saindo do depósito quando a revelação sobe.
    #[gpui::test]
    fn o_deposito_local_ganha_da_galeria_e_sai_quando_a_foto_sobe(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let publicador = Arc::new(PublicadorDeMentira::default());
        // No disco: o operador revelou ontem e não salvou.
        let gravador = Arc::new(GravadorDeMentira::com_o_deposito(vec![(
            "remota-1".to_string(),
            r#"{"exposure":1.25}"#.to_string(),
        )]));

        let janela = cx.add_window({
            let publicador = publicador.clone();
            let gravador = gravador.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(
                    Vec::new(),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
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
                // A API responde a receita **de quando a foto subiu** — outra.
                app.atender_a_sessao(
                    &DetalhePedido::FotosDoSite(vec![foto_do_site(
                        "remota-1",
                        Some(serde_json::json!({ "exposure": 0.25 })),
                    )]),
                    window,
                    cx,
                );

                let na_grade = app
                    .fotos_do_site
                    .iter()
                    .find(|f| f.pos_venda_foto_id.as_deref() == Some("remota-1"))
                    .expect("a foto do site está na grade");
                assert_eq!(
                    persistencia::da_foto(na_grade).exposure,
                    1.25,
                    "o que nao subiu ganha do que a API tem"
                );
            })
            .expect("a janela deve estar aberta");

        // E quando ela sobe, sai do depósito: a verdade passa a ser o servidor.
        janela
            .update(cx, |app, _window, cx| {
                let _ = app.sincronias.0.send(PosVendaRecado::RevelacaoSalva {
                    foto_no_site: "remota-1".into(),
                });
                app.colher_sincronia(cx);
            })
            .expect("a janela deve estar aberta");
        assert!(
            gravador.deposito().is_empty(),
            "a que subiu tem de sair do deposito: {:?}",
            gravador.deposito()
        );
    }

    /// 🚨 **A receita do site chega à grade — e por ela à Revelação.**
    ///
    /// A API sempre mandou os ajustes por nome; o `http.rs` guardava só "tem ou
    /// não tem". A foto já revelada abria aqui com os 53 sliders no neutro, e
    /// era essa receita vazia que "sincronizar" levava às outras.
    #[test]
    fn a_foto_do_site_traz_a_receita_para_a_grade() {
        let com_receita = do_site_para_a_grade(
            &foto_do_site(
                "remota-1",
                Some(serde_json::json!({
                    "saturation": -1.0,
                    "split_shadow_hue": 35,
                    "corte_x": 0.1, "corte_y": 0.0, "corte_largura": 0.8, "corte_altura": 1.0,
                    "corte_giro90": 0, "corte_angulo": 0, "corte_espelho_h": 0, "corte_espelho_v": 0
                })),
            ),
            Some("g1".into()),
        );
        assert_eq!(com_receita.edit_saturation, Some(-1.0));
        assert_eq!(com_receita.edit_split_shadow_hue, Some(35.0));
        assert_eq!(com_receita.edit_contrast, Some(1.0), "ausente é o neutro");
        assert_eq!(com_receita.edit_crop_x, Some(0.1));
        assert!(persistencia::ja_revelada(&com_receita));
        assert!(persistencia::so_existe_no_site(&com_receita));

        let nunca_revelada = do_site_para_a_grade(&foto_do_site("remota-2", None), None);
        assert_eq!(nunca_revelada.edit_saturation, None);
        assert!(!persistencia::ja_revelada(&nunca_revelada));
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

    /// O mesmo acervo, mas de um ensaio — a grade escopada só mostra o dele.
    fn acervo_da_sessao(id: &str) -> Vec<PhotoViewModel> {
        acervo()
            .into_iter()
            .map(|mut f| {
                f.sessao_id = Some(id.to_string());
                f
            })
            .collect()
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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

    /// 🚨 **A foto que só está no disco não é foto do site — e a sessão tratava
    /// as duas igual.**
    ///
    /// O que o dono viu em 8/set/2026, com as 21 fotos importadas do ensaio: a
    /// Revelação abrindo em "não tem preview no cache" e a tira inteira preta,
    /// com o JPEG e o preview no disco, a um passo. A grade da sessão mostra
    /// duas famílias — a do site, baixada e gravada sob `site:<id>`, e a que só
    /// existe aqui, gravada pelo importador sob o id cru do catálogo — e o
    /// pedido de revelar mandava as duas como se fossem do site: id prefixado,
    /// caminho zerado, e uma cópia de trabalho pedida à nuvem para uma foto que
    /// nunca subiu.
    ///
    /// É o mesmo defeito que `Detalhe::chave_da_foto` já conhecia na célula,
    /// repetido uma tela adiante.
    #[gpui::test]
    fn revelar_da_sessao_abre_a_foto_local_pelo_catalogo_e_nao_como_do_site(
        cx: &mut TestAppContext,
    ) {
        let (previews, _dir) = previews_descartaveis();
        // O preview está no cache sob o id **do catálogo**, que é onde o
        // importador o grava.
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        previews
            .save_thumbnail("id-retrato.jpg", &foto_vermelha())
            .expect("gravar miniatura");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::ja_dentro(
                    acervo_da_sessao("g1"),
                    previews,
                    Vec::new(),
                    portas(),
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.atender_a_sessao(
                    &DetalhePedido::Revelar {
                        fotos: vec![FotoARevelar {
                            id: "id-retrato.jpg".into(),
                            arquivo: "retrato.jpg".into(),
                            no_disco: true,
                        }],
                        inicial: 0,
                    },
                    window,
                    cx,
                );

                let aberta = app
                    .revelacao
                    .read(cx)
                    .foto_aberta()
                    .cloned()
                    .expect("a Revelação abriu com uma foto");
                assert_eq!(
                    aberta.id, "id-retrato.jpg",
                    "o id do catálogo não pode virar `site:…` — é sob ele que a \
                     preview está no cache"
                );
                assert_eq!(
                    aberta.path, "/fotos/retrato.jpg",
                    "o caminho do arquivo se perdia, e com ele a única fonte local"
                );
                assert!(
                    aberta.pos_venda_foto_id.is_none(),
                    "ela nunca subiu: dar-lhe um id no site manda pedir à nuvem \
                     uma cópia de trabalho que não existe"
                );
                assert!(
                    app.revelacao.read(cx).tem_pixels(),
                    "com o id certo, o preview do cache responde na hora"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **A do site continua vindo do site.** O mesmo caminho, a outra família:
    /// id prefixado, sem caminho local, e a cópia de trabalho pedida à nuvem.
    #[gpui::test]
    fn revelar_da_sessao_mantem_a_foto_do_site_como_do_site(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::ja_dentro(Vec::new(), previews, Vec::new(), portas(), window, cx)
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.atender_a_sessao(
                    &DetalhePedido::Revelar {
                        fotos: vec![FotoARevelar {
                            id: "remota-1".into(),
                            arquivo: "DSC_001.jpg".into(),
                            no_disco: false,
                        }],
                        inicial: 0,
                    },
                    window,
                    cx,
                );

                let aberta = app
                    .revelacao
                    .read(cx)
                    .foto_aberta()
                    .cloned()
                    .expect("a Revelação abriu com uma foto");
                assert_eq!(aberta.id, "site:remota-1");
                assert!(aberta.path.is_empty(), "ela não está neste disco");
                assert_eq!(aberta.pos_venda_foto_id.as_deref(), Some("remota-1"));
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **O beco sem saída de "não tem preview no cache".**
    ///
    /// A foto está catalogada, o arquivo está no disco e o cache perdeu o
    /// preview — "Limpar previews" nas Configurações basta. A Revelação abria
    /// numa frase e ficava lá: nada era pedido a ninguém, e o único jeito de
    /// sair era reimportar a foto.
    #[gpui::test]
    fn abrir_sem_cache_manda_refazer_o_preview_do_disco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let repositor = Arc::new(RepositorDeMentira::que_devolve(foto_vermelha()));
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let (previews, repositor) = (previews.clone(), repositor.clone());
            |window, cx| {
                Aplicativo::ja_dentro(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        repositor,
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
                assert!(
                    !app.revelacao.read(cx).tem_pixels(),
                    "o cache está vazio: o palco abre sem imagem"
                );
            })
            .expect("a janela deve estar aberta");
        // 🔑 O `AbriuOutraFoto` é um evento, e evento não chega dentro do mesmo
        // `update` que o emitiu — é no efeito seguinte que a raiz o atende.
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                // O laço de espera anda por relógio; aqui a colheita é chamada
                // à mão, como nos outros testes de porta.
                app.colher_reposicao(cx);
                assert!(
                    app.revelacao.read(cx).tem_pixels(),
                    "o preview refeito do disco tem de chegar ao palco"
                );
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            repositor.pedidos().first().map(|a| a.foto_id.clone()),
            Some("id-retrato.jpg".into()),
            "a foto do palco é a primeira da fila: {:?}",
            repositor.pedidos()
        );
    }

    /// 🚨 **A tira entra no mesmo pedido, atrás do palco.**
    ///
    /// Quando o cache some, some inteiro: a foto aberta e as vinte da tira. Se
    /// só o palco fosse reposto, o fotógrafo ficaria com uma foto boa cercada de
    /// retângulos pretos — e sem nada acontecendo, porque a única pergunta feita
    /// era sobre o palco.
    #[gpui::test]
    fn a_tira_inteira_entra_na_reposicao_atras_do_palco(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let repositor = Arc::new(RepositorDeMentira::que_devolve(foto_vermelha()));
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let (previews, repositor) = (previews.clone(), repositor.clone());
            |window, cx| {
                Aplicativo::ja_dentro(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        repositor,
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
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let pedidas: Vec<String> = repositor.pedidos().into_iter().map(|a| a.foto_id).collect();
        assert_eq!(
            pedidas,
            vec!["id-retrato.jpg", "id-DSC_001.NEF"],
            "o palco na frente, a vizinha da tira em seguida"
        );
    }

    /// 🚨 **Andar pela seta não pode reenfileirar o que já foi pedido.**
    ///
    /// `AbriuOutraFoto` dispara a cada troca de foto, e o cache só muda quando a
    /// reposição termina — então a varredura seguinte veria as mesmas faltas.
    /// Com o repositor trabalhando em série, a fila cresceria mais depressa do
    /// que anda, e a foto do palco acabaria atrás de dezenas de duplicatas dela
    /// mesma.
    #[gpui::test]
    fn andar_pela_tira_nao_pede_a_mesma_foto_duas_vezes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let repositor = Arc::new(RepositorDeMentira::que_devolve(foto_vermelha()));
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let (previews, repositor) = (previews.clone(), repositor.clone());
            |window, cx| {
                Aplicativo::ja_dentro(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        repositor,
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
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
        assert_eq!(repositor.pedidos().len(), 2, "as duas do acervo, uma vez");

        // A seta anda **sem** que ninguém tenha colhido as respostas: é o caso
        // real, porque a colheita só acontece de 100 em 100 ms.
        janela
            .update(cx, |app, window, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.andar(1, window, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert_eq!(
            repositor.pedidos().len(),
            2,
            "a seta pediu de novo o que já estava na fila: {:?}",
            repositor.pedidos()
        );
    }

    /// ⚠️ **O que já está no cache não é refeito.**
    ///
    /// Repor é decodificar o arquivo inteiro. Fazer isso na abertura de toda
    /// foto transformaria a revelação em série — que é como se revela um
    /// casamento — em duzentas decodificações que o cache já tinha respondido.
    #[gpui::test]
    fn o_que_esta_no_cache_nao_e_refeito(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-retrato.jpg", "id-DSC_001.NEF"] {
            previews
                .save_preview(id, &foto_vermelha())
                .expect("gravar preview");
            previews
                .save_thumbnail(id, &foto_vermelha())
                .expect("gravar miniatura");
        }
        let repositor = Arc::new(RepositorDeMentira::que_devolve(foto_vermelha()));
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let (previews, repositor) = (previews.clone(), repositor.clone());
            |window, cx| {
                Aplicativo::ja_dentro(
                    acervo(),
                    previews,
                    Vec::new(),
                    Portas {
                        repositor,
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
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert!(
            repositor.pedidos().is_empty(),
            "o cache respondeu por todas: {:?}",
            repositor.pedidos()
        );
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
                Aplicativo::ja_dentro(
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                // 🚨 **`Tela::Sessao`, e não `Biblioteca`** — o `ja_dentro`
                // abre uma sessão (`sessao_aberta = Some("g1")`), e sair da
                // Revelação é voltar **de onde se veio**, que é a grade do
                // ensaio. As duas asserções pediam `Biblioteca` e falhavam
                // desde que o `ja_dentro` passou a abrir a sessão: o defeito
                // estava no que o teste esperava, e não no caminho de volta.
                assert_eq!(app.tela(), Tela::Sessao, "o Esc tem de sair da Revelação");
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                    Tela::Sessao,
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                    Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                Aplicativo::ja_dentro(
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                Aplicativo::ja_dentro(
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
                    Aplicativo::ja_dentro(
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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
    /// 🚨 A porta vem antes de tudo — inclusive das teclas.
    ///
    /// O dono reverteu em 6/set/2026 o princípio "o app tem de ser útil
    /// sozinho", primeiro com uma saída — "trabalhar offline" — que caiu no
    /// mesmo dia: o propósito do app é a integração com o pós-venda. O que este
    /// teste prende é o **antes**: com o app inteiro desenhado por baixo da tela
    /// de login, as quinze teclas de triagem continuariam chegando à Biblioteca
    /// por trás dela — nota dada numa grade que ninguém está vendo.
    /// 🔑 **A procura começa na abertura, sem ninguém pedir.** É o único momento
    /// em que o operador não está no meio de coisa nenhuma. Se este teste falhar,
    /// o sintoma no app é silencioso: versão nova publicada, ninguém sabendo.
    #[gpui::test]
    fn o_app_procura_versao_nova_ao_abrir(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let (portas, atualizador) = portas_com_versao("0.2.0");

        let _janela = cx.add_window(|window, cx| {
            Aplicativo::novo(acervo(), previews, Vec::new(), portas, window, cx)
        });

        assert_eq!(
            *atualizador.procuras.lock().expect("as procuras"),
            1,
            "abrir o app é procurar versão nova, uma vez"
        );
    }

    /// A resposta chega pelo canal e a faixa passa a ter o que dizer.
    #[gpui::test]
    fn a_versao_encontrada_chega_a_faixa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let (portas, _atualizador) = portas_com_versao("0.2.0");

        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas, window, cx)
        });

        // O laço de `esperar_aviso` acorda a cada 200ms; deixar o executor rodar
        // é o que faz o `try_recv` chegar ao canal.
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.atualizacao.texto().as_deref(),
                    Some("Versão 0.2.0 disponível"),
                    "a faixa avisa o que a porta encontrou"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **"Depois" some com o aviso desta versão, não com o da próxima.** Sem
    /// isto, quem clicar "Depois" uma vez deixa de ver a correção urgente que
    /// vier em seguida — e não há nada na tela que revele isso.
    #[gpui::test]
    fn depois_dispensa_so_a_versao_avisada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let (portas, _atualizador) = portas_com_versao("0.2.0");

        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas, window, cx)
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                app.atender(PedidoDeAtualizacao::Dispensar, cx);
                assert_eq!(app.atualizacao.texto(), None, "a 0.2.0 sai da vista");

                // A 0.3.0 chega depois, pelo mesmo canal.
                app.atualizacao.aviso =
                    Some(Aviso::Disponivel(crate::atualizacao::porta::VersaoNova {
                        versao: "0.3.0".into(),
                        notas: None,
                    }));
                assert_eq!(
                    app.atualizacao.texto().as_deref(),
                    Some("Versão 0.3.0 disponível"),
                    "dispensar uma versão não cala a seguinte"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Clicar "Atualizar" pede a instalação **e** muda a frase na hora — sem
    /// isso um download de 60 MB deixa a tela parada e o gesto seguinte é
    /// clicar de novo.
    #[gpui::test]
    fn atualizar_instala_e_avisa_que_esta_baixando(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let (portas, atualizador) = portas_com_versao("0.2.0");
        // A mentira responde na hora; para ver o estado "baixando" o desfecho
        // fica em silêncio.
        *atualizador.desfecho.lock().expect("o desfecho") = None;

        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas, window, cx)
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                app.atender(PedidoDeAtualizacao::Instalar, cx);
                assert_eq!(
                    *atualizador.instalacoes.lock().expect("as instalações"),
                    1,
                    "o clique chega até a porta"
                );
                assert_eq!(
                    app.atualizacao.texto().as_deref(),
                    Some("Baixando a versão 0.2.0…")
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// A falha da instalação aparece — houve um clique esperando resposta.
    #[gpui::test]
    fn a_falha_da_instalacao_chega_a_faixa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let (portas, atualizador) = portas_com_versao("0.2.0");
        *atualizador.desfecho.lock().expect("o desfecho") =
            Some(Aviso::Falhou("assinatura inválida".into()));

        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas, window, cx)
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                app.atender(PedidoDeAtualizacao::Instalar, cx);
            })
            .expect("a janela deve estar aberta");
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.atualizacao.texto().as_deref(),
                    Some("Não consegui atualizar: assinatura inválida"),
                    "a assinatura que não bate é o que impede um pacote trocado \
                     de ser instalado — e o operador precisa ver isso"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **Um app sem versão nova não mostra faixa nenhuma.** Silêncio é a
    /// resposta normal, e é o que quase toda abertura devolve.
    #[gpui::test]
    fn sem_versao_nova_nao_ha_faixa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(app.atualizacao.texto(), None);
            })
            .expect("a janela deve estar aberta");
    }

    #[gpui::test]
    fn a_porta_vem_antes_de_tudo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        // `novo`, e não `ja_dentro`: este é o teste da porta.
        let janela = cx.add_window(|window, cx| {
            Aplicativo::novo(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, _window, cx| {
                assert!(!app.entrou(), "o app abre na porta");
                assert_eq!(app.sessao(), None);

                app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.na_biblioteca(cx, |tela, cx| tela.dar_nota(3, cx));
                assert_eq!(
                    app.biblioteca.read(cx).fotos_visiveis()[0].rating,
                    3,
                    "o método continua funcionando — quem não chega até ele é a tecla"
                );

                // Entrar na conta é o único jeito de abrir o app.
                app.entrar_na_conta(sessao_de_teste(), cx);
                assert!(app.entrou());
                assert!(app.sessao().is_some(), "passar da porta é ter conta");
                // 🚨 E entrar **não** é poder trabalhar: falta escolher o ensaio.
                assert!(!app.pode_trabalhar(), "logado e sem ensaio: só a lista");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 Entrar na porta desce a sessão para o pós-venda — ninguém entra duas
    /// vezes na mesma conta na mesma abertura.
    #[gpui::test]
    fn entrar_na_porta_desce_a_sessao_para_o_pos_venda(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let publicador = Arc::new(PublicadorDeMentira {
            produtos: vec![domain::services::pos_venda::Produto {
                id: "p1".into(),
                nome: "Foto avulsa".into(),
                preco: "29.90".into(),
                inativo: false,
            }],
            ..Default::default()
        });
        let janela = cx.add_window({
            let previews = previews.clone();
            let publicador = publicador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo_da_sessao("g7"),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.entrar_na_conta(sessao_de_teste(), cx);
                // Entrar leva à lista de sessões; a triagem acontece na grade.
                app.voltar_para_biblioteca(_window, cx);
                assert!(app.entrou());
                assert_eq!(app.sessao().map(|s| s.access_token.as_str()), Some("tok"));

                // E a sessão da sessão fotográfica já nasce com ela: nenhuma
                // das telas volta a pedir a conta.
                assert!(app.detalhe.read(cx).tem_sessao());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A resposta que demora não pode chegar depois de a colheita desistir.**
    ///
    /// Pedir o link liga a colheita — o laço que drena o canal a cada 100 ms —,
    /// e a condição de continuar acordado era
    /// `carregando || enviando || baixando || avisando`: uma lista de estados
    /// ao lado, que esquecia justamente o link. No primeiro giro, 100 ms depois,
    /// a resposta do Fly ainda não voltou; o laço morria, e o `Recado::Link`
    /// chegava a um canal que ninguém mais drenava. O botão do passo 7 não
    /// copiava nada e não dizia nada.
    ///
    /// Nenhum teste via porque o `PublicadorDeMentira` responde no mesmo
    /// instante — o recado já estava no canal antes do primeiro `colher` — e
    /// porque os testes chamavam `colher` à mão. Este **não chama**, de
    /// propósito: é a colheita sozinha que estava quebrada.
    #[gpui::test]
    fn a_resposta_que_demora_ainda_chega_a_tela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let publicador = Arc::new(PublicadorDeMentira {
            // A galeria precisa existir: `entrar` a abre, e a sessão que não
            // existe responde `Falhou` — que encerra a espera do link junto.
            galerias: std::sync::Mutex::new(vec![galeria_do_painel("g7")]),
            // A rede do estúdio até o Fly leva centenas de milissegundos.
            demorada: true,
            ..Default::default()
        });
        let janela = cx.add_window({
            let previews = previews.clone();
            let publicador = publicador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo_da_sessao("g7"),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.entrar_na_conta(sessao_de_teste(), cx);
                app.detalhe.update(cx, |tela, cx| {
                    tela.entrar("g7".into(), cx);
                    tela.pedir_o_link(cx);
                });
            })
            .expect("a janela deve estar aberta");

        // O tempo passa e a resposta não chegou: a colheita tem de continuar de pé.
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(500));
        cx.run_until_parked();
        janela
            .update(cx, |app, _window, cx| {
                assert!(
                    app.detalhe.read(cx).link().is_none(),
                    "a rede ainda não respondeu"
                );
            })
            .expect("a janela deve estar aberta");

        publicador.responder();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.detalhe.read(cx).link().map(|l| l.url.as_str()),
                    Some("https://recordarfotos.com.br/entrar?t=g7"),
                    "o link que demorou tem de chegar à tela sozinho"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 📸 O passo 3 do fluxo do dono: **classifico as fotos** — e elas sobem.
    ///
    /// 🚨 O que este teste prende é a **travessia**, e não o estado. Ir de 3
    /// para 4 estrelas não sobe nada: a foto já estava lá. O que conta é sair do
    /// zero (sobe) e voltar a ele (sai do storage).
    #[gpui::test]
    fn classificar_sobe_e_zerar_tira_do_site(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let publicador = publicador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo_da_sessao("g7"),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.entrar_na_conta(sessao_de_teste(), cx);
                // Entrar leva à lista de sessões; a triagem acontece na grade.
                app.voltar_para_biblioteca(_window, cx);
                // A sessão escolhida é para onde as fotos vão.
                app.sessoes
                    .update(cx, |tela, cx| tela.abrir("g7".into(), cx));
            })
            .expect("a janela deve estar aberta");
        // 🔑 O evento da escolha só chega ao assinante quando o `update` fecha:
        // é aqui que a raiz entra na sessão.
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(app.tela(), Tela::Sessao, "escolher a sessão entra nela");
                // A triagem deste teste é a do catálogo local — a grade dele é a
                // Biblioteca.
                app.tela = Tela::Biblioteca;

                app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.na_biblioteca(cx, |tela, cx| tela.dar_nota(4, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        let subidas = publicador.subidas();
        assert_eq!(subidas.len(), 1, "uma foto atravessou o zero");
        assert_eq!(subidas[0].0, "g7", "para a sessão aberta");

        janela
            .update(cx, |app, _window, cx| {
                // De 4 para 5: continua no site, e nada é pedido de novo.
                app.na_biblioteca(cx, |tela, cx| tela.dar_nota(5, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
        assert_eq!(
            publicador.subidas().len(),
            1,
            "mudar de 4 para 5 estrelas não é travessia"
        );

        janela
            .update(cx, |app, _window, cx| {
                app.na_biblioteca(cx, |tela, cx| tela.dar_nota(0, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
        assert_eq!(
            publicador.tiradas().len(),
            1,
            "zerar a nota tira a foto do storage"
        );
    }

    /// 🚨 Classificar sem sessão aberta **avisa**, e não sobe calado.
    ///
    /// Classificar 200 fotos e descobrir no balcão que nenhuma foi para o site é
    /// o desfecho que este aviso existe para impedir. Silêncio aqui seria pior
    /// que erro: o operador não teria como saber que faltou um passo.
    #[gpui::test]
    fn classificar_sem_sessao_aberta_avisa_em_vez_de_sumir(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let publicador = publicador.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo_da_sessao("g7"),
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
                        ..portas()
                    },
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, _window, cx| {
                app.entrar_na_conta(sessao_de_teste(), cx);
                // Entrar leva à lista de sessões; a triagem acontece na grade.
                app.voltar_para_biblioteca(_window, cx);
                app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.na_biblioteca(cx, |tela, cx| tela.dar_nota(3, cx));
            })
            .expect("a janela deve estar aberta");
        // 🔑 O evento só chega ao assinante quando os efeitos são drenados — é
        // por isso que a leitura do aviso vem depois, e não no mesmo `update`.
        cx.run_until_parked();

        janela
            .update(cx, |app, _window, cx| {
                let aviso = app
                    .biblioteca
                    .read(cx)
                    .aviso()
                    .cloned()
                    .expect("a tela tinha de avisar");
                assert!(aviso.contains("nenhuma sessão aberta"), "{aviso}");
            })
            .expect("a janela deve estar aberta");

        assert!(
            publicador.subidas().is_empty(),
            "sem sessão aberta não há para onde subir"
        );
    }

    #[gpui::test]
    fn exportar_manda_a_selecao_para_a_pasta_escolhida(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let exportador = Arc::new(ExportadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let exportador = exportador.clone();
            |window, cx| {
                Aplicativo::ja_dentro(
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

    /// 🚨 **O Exportar da sessão abre a mesma exportação que o da barra abria.**
    ///
    /// O botão desceu da barra do app para o lado do "Importar"
    /// (8/set/2026), e com isso deixou de chamar `exportar` direto: agora ele
    /// emite um pedido, e é a raiz que atende. Uma ligação a mais para se
    /// perder, e um botão que não faz nada é indistinguível de um clique
    /// perdido — por isso o pedido tem teste, e não só o método.
    #[gpui::test]
    fn o_exportar_da_sessao_abre_a_exportacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                assert!(!app.exportando(), "a exportação começa fechada");
                app.atender_a_sessao(&DetalhePedido::Exportar, window, cx);
                assert!(app.exportando(), "o pedido da sessão não abriu a exportação");
                app.exportacao.update(cx, |tela, _cx| {
                    assert_eq!(
                        tela.quantas(),
                        2,
                        "sem seleção, vai o que a grade está mostrando — como o botão da barra fazia"
                    );
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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

    /// 🔑 **Sincronizar grava a receita nas marcadas, respeita as flags e
    /// preserva o corte de cada uma** — o "Sincronizar" do site. E a tira já
    /// sabe: a seta seguinte abre a sincronizada com os sliders novos.
    #[gpui::test]
    fn sincronizar_grava_nas_marcadas_e_preserva_o_corte(cx: &mut TestAppContext) {
        use crate::revelacao::sincronizacao::Escolha;
        use domain::entities::preset::PresetAdjustments;

        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let mut destino = foto("retrato.jpg");
        destino.edit_crop_x = Some(0.4);
        destino.edit_sharpen_amount = Some(30.0);

        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(
                    vec![foto("DSC_001.NEF"), destino],
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
                assert_eq!(app.tela(), Tela::Revelacao);

                let preset = Preset::system(
                    "Clareia e afia",
                    PresetAdjustments::vazia()
                        .com("exposure", 1.5)
                        .com("sharpen_amount", 80.0),
                );
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_preset(&preset, window, cx);
                    tela.marcar_todas(cx);
                    assert_eq!(tela.alvos_da_sincronizacao().len(), 2);
                    tela.definir_escolha_da_sincronizacao(Escolha {
                        detalhe: false,
                        ..Escolha::default()
                    });
                });
                app.sincronizar_revelacao(window, cx);

                let revelacao = app.revelacao.read(cx);
                let outra = revelacao
                    .acervo()
                    .iter()
                    .find(|f| f.id == "id-retrato.jpg")
                    .expect("a outra continua na tira");
                assert_eq!(outra.edit_exposure, Some(1.5), "a tira já sabe");
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        let (_, ajustes, corte) = gravado
            .iter()
            .rfind(|(id, _, _)| id == "id-retrato.jpg")
            .expect("a marcada foi gravada");
        assert_eq!(ajustes.exposure, 1.5, "Básico viaja");
        assert_eq!(
            ajustes.sharpen_amount, 30.0,
            "Detalhe desmarcado fica o dela"
        );
        assert_eq!(corte.x, Some(0.4), "o corte é o dela, não o da aberta");
    }

    /// 🚨 `Cmd+A`, `Ctrl+A` e `Cmd+D` chegam à **tira da Revelação** — e não à
    /// grade da Biblioteca por trás dela. Sem isto o lote nunca se forma, o
    /// botão "Sincronizar N" não aparece, e a caixa de flags nunca abre.
    #[gpui::test]
    fn na_revelacao_cmd_a_e_ctrl_a_marcam_a_tira(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
        });
        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.revelar(window, cx);
                assert_eq!(app.tela(), Tela::Revelacao);
                assert_eq!(app.revelacao.read(cx).marcadas().len(), 1);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        for (tecla, esperado) in [("cmd-a", 2), ("cmd-d", 1), ("ctrl-a", 2), ("ctrl-d", 1)] {
            visual.simulate_keystrokes(tecla);
            janela
                .update(cx, |app, _window, cx| {
                    assert_eq!(
                        app.revelacao.read(cx).marcadas().len(),
                        esperado,
                        "{tecla} na Revelação"
                    );
                    assert_eq!(
                        app.biblioteca.read(cx).quantas_selecionadas(),
                        1,
                        "{tecla} não mexe na grade por trás"
                    );
                })
                .expect("a janela deve estar aberta");
        }
    }

    /// 🚨 **O laço espera uma resposta por foto, e não só a primeira.**
    ///
    /// Num lote, cada foto baixa, revela e sobe — as respostas chegam
    /// espalhadas por segundos. O laço antigo desligava assim que a primeira
    /// chegava, e as outras ficavam no canal sem ninguém para lê-las: a grade
    /// mostrava uma revelada, e o erro das demais não aparecia. Era o "não está
    /// sincronizando" de 7/set/2026.
    #[gpui::test]
    fn a_espera_conta_uma_resposta_por_foto_do_lote(cx: &mut TestAppContext) {
        use crate::revelacao::sincronizacao::Escolha;

        let (previews, _dir) = previews_descartaveis();
        // Com prévia no cache, a Revelação não pede pixels à nuvem (o passo 11)
        // — e o contador fica só com o que o lote mandou.
        previews
            .save_preview("id-DSC_001.NEF", &foto_vermelha())
            .expect("gravar preview");
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        // A rede que **segura** as respostas: é assim que elas chegam uma a uma.
        let publicador = Arc::new(PublicadorDeMentira {
            demorada: true,
            ..Default::default()
        });

        let mut primeira = foto("DSC_001.NEF");
        primeira.pos_venda_foto_id = Some("remota-1".into());
        let mut segunda = foto("retrato.jpg");
        segunda.pos_venda_foto_id = Some("remota-2".into());

        let janela = cx.add_window({
            let previews = previews.clone();
            let publicador = publicador.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(
                    vec![primeira, segunda],
                    previews,
                    Vec::new(),
                    Portas {
                        publicador,
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
                    tela.marcar_todas(cx);
                    tela.definir_escolha_da_sincronizacao(Escolha::default());
                });
                app.sincronizar_revelacao(window, cx);
                // Sincronizar copia a receita; quem manda ao site é o salvar.
                assert_eq!(app.sincronias_pendentes(), 0, "o sincronizar não sobe nada");
                app.salvar_na_galeria(window, cx);

                assert_eq!(
                    app.sincronias_pendentes(),
                    2,
                    "duas fotos no site, duas respostas a esperar"
                );
            })
            .expect("a janela deve estar aberta");

        // A primeira responde: **o laço não pode desligar aqui**.
        publicador.responder_uma();
        janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
                assert_eq!(app.sincronias_pendentes(), 1, "ainda falta uma");
            })
            .expect("a janela deve estar aberta");

        publicador.responder_uma();
        janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
                assert_eq!(app.sincronias_pendentes(), 0, "agora o lote acabou");
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            publicador.reveladas().len(),
            2,
            "as duas foram mandadas ao site"
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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
                Aplicativo::ja_dentro(
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
            |window, cx| Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
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
