//! A tela do caixa: estúdio e caixa no cabeçalho, sessões e cupom no conteúdo,
//! o movimento do caixa na lateral — o `pdv-do-caixa.tsx` do site.
//!
//! # A carga é a da página, inteira
//!
//! No site o estado mora na URL (`?estudio=` e `?sessao=`) e cada navegação
//! relê tudo no servidor (`carregar.ts`). Aqui é o mesmo: [`Caixa::navegar`]
//! pede os estúdios, as sessões, os funcionários, o catálogo, a sessão escolhida
//! e — decidido o estúdio — o caixa aberto dele, e só **troca a tela quando
//! tudo chegou**. Trocar pedaço a pedaço mostraria, por um instante, o caixa
//! de um estúdio ao lado das sessões de outro.
//!
//! 🔑 **Cada navegação tem o seu canal.** A resposta atrasada de uma navegação
//! anterior chega num canal que ninguém lê mais, e some — sem precisar
//! adivinhar, pelo JSON, a que pedido ela respondia (um caixa fechado é `null`,
//! e `null` não diz de que estúdio é).
//!
//! As gravações têm um canal só delas, que não troca: uma venda que responde
//! depois de o operador clicar noutra sessão ainda precisa dar o recibo.
//!
//! Depois de cada gravação a carga se repete, como o `revalidatePath` do site.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::caixa::{
    self as regras, Cupom, Desfecho, Faixa, FotoDoCupom, ModoDoDesconto, Pessoas, SessaoACobrar,
    SituacaoDoPdv, Venda,
};
use biblioteca_core::dinheiro;
use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Context, Div, EventEmitter, FocusHandle,
    FontWeight, Hsla, SharedString, Task, Window,
};
use serde::{Deserialize, Serialize};

use super::dados::{
    self, CaixaDoBalcao, EstudioDoCaixa, FaixaDoCatalogo, Funcionario, GaleriaDoCaixa,
    SessaoDaLista,
};
use super::dialogos::{DesSinalizacao, Dialogo, EmCurso, TipoDeDialogo};
use super::flutuante::{Lote, Modo};
use crate::estilo;
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::recursos::Icone;
use crate::tema::cores;

/// De quanto em quanto a tela pergunta se o site respondeu.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);
/// O catálogo vem em páginas; página incompleta é o fim.
const PAGINA_DO_CATALOGO: usize = 200;
const MAXIMO_DE_PAGINAS: usize = 20;

gpui_kit::actions!(
    caixa,
    [
        // As teclas F do PDV, na tela inteira.
        TeclaF1,
        TeclaF2,
        TeclaF3,
        TeclaF4,
        TeclaF6,
        TeclaF7,
        TeclaF8,
        // Dentro dos diálogos.
        FecharDialogo,
        ConfirmarDialogo,
        ConcluirVenda,
        Forma1,
        Forma2,
        Forma3,
        Forma4,
        Forma5,
        Forma6,
        Forma7,
        Forma8,
        TirarUltimo,
        EmReais,
        EmPercentual,
        Sangria,
        Suprimento
    ]
);

/// O contexto de teclado da tela.
///
/// Próprio, e não o da raiz: as teclas F só valem com o caixa à vista. Um F8
/// global abriria o diálogo de fechar o caixa por cima da Biblioteca.
pub(super) const CONTEXTO: &str = "Caixa";
/// O de um diálogo aberto. O tipo do diálogo entra junto no contexto
/// (`CaixaDialogo Pagamento`), para as teclas de um não valerem no outro.
pub(super) const DIALOGO: &str = "CaixaDialogo";

/// As teclas da tela. A raiz chama uma vez; [`Caixa::nova`] também chama, para
/// a tela funcionar sozinha nos testes — ligação repetida é inofensiva.
pub fn init(cx: &mut App) {
    use gpui_kit::KeyBinding;
    const SEM_CAMPO: &str = "CaixaDialogo && !Input";
    cx.bind_keys([
        KeyBinding::new("f1", TeclaF1, Some(CONTEXTO)),
        KeyBinding::new("f2", TeclaF2, Some(CONTEXTO)),
        KeyBinding::new("f3", TeclaF3, Some(CONTEXTO)),
        KeyBinding::new("f4", TeclaF4, Some(CONTEXTO)),
        KeyBinding::new("f6", TeclaF6, Some(CONTEXTO)),
        KeyBinding::new("f7", TeclaF7, Some(CONTEXTO)),
        KeyBinding::new("f8", TeclaF8, Some(CONTEXTO)),
        KeyBinding::new("escape", FecharDialogo, Some(DIALOGO)),
        // 🔑 O `Enter` de um campo de texto **passa adiante** (o `InputState`
        // propaga no campo de uma linha): é por isso que ele confirma o
        // formulário com o foco no campo, como o `submit` do site.
        KeyBinding::new("enter", ConfirmarDialogo, Some(DIALOGO)),
        KeyBinding::new("secondary-enter", ConcluirVenda, Some(DIALOGO)),
        KeyBinding::new("f4", ConcluirVenda, Some("CaixaDialogo && Pagamento")),
        // No pagamento, fora do campo: `1`–`8` escolhem a forma, `⌫` tira o
        // último lançado.
        KeyBinding::new("1", Forma1, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("2", Forma2, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("3", Forma3, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("4", Forma4, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("5", Forma5, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("6", Forma6, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("7", Forma7, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new("8", Forma8, Some("CaixaDialogo && Pagamento && !Input")),
        KeyBinding::new(
            "backspace",
            TirarUltimo,
            Some("CaixaDialogo && Pagamento && !Input"),
        ),
        KeyBinding::new(
            "delete",
            TirarUltimo,
            Some("CaixaDialogo && Pagamento && !Input"),
        ),
        // No desconto, `%` troca para percentual **mesmo no campo** (é o que se
        // digita depois do número); `R`, só fora dele.
        KeyBinding::new("%", EmPercentual, Some("CaixaDialogo && Desconto")),
        KeyBinding::new("shift-5", EmPercentual, Some("CaixaDialogo && Desconto")),
        KeyBinding::new("r", EmReais, Some("CaixaDialogo && Desconto && !Input")),
        KeyBinding::new(
            "shift-r",
            EmReais,
            Some("CaixaDialogo && Desconto && !Input"),
        ),
        // Na sangria: `S` e `U`, fora do campo.
        KeyBinding::new("s", Sangria, Some("CaixaDialogo && Movimento && !Input")),
        KeyBinding::new(
            "shift-s",
            Sangria,
            Some("CaixaDialogo && Movimento && !Input"),
        ),
        KeyBinding::new("u", Suprimento, Some("CaixaDialogo && Movimento && !Input")),
        KeyBinding::new(
            "shift-u",
            Suprimento,
            Some("CaixaDialogo && Movimento && !Input"),
        ),
    ]);
    let _ = SEM_CAMPO;
}

/// O que o caixa pede à raiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoDoCaixa {
    /// "Abrir sessão": a galeria da sessão escolhida, com a volta para cá.
    AbrirSessao(String),
}

impl EventEmitter<PedidoDoCaixa> for Caixa {}

/// O desconto no total como foi digitado — o `DescontoNoTotal` do site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DescontoNoTotal {
    pub modo: ModoDoDesconto,
    pub texto: String,
    pub motivo: String,
}

/// A sessão escolhida, com o que o cupom e o diálogo de vendas precisam.
pub(super) struct SessaoNoCaixa {
    pub id: String,
    pub titulo: String,
    pub vendas: Vec<Venda>,
}

/// O que a tela mostra — o `carregarCaixa` do site, pronto.
pub(super) struct Vista {
    pub estudios: Vec<EstudioDoCaixa>,
    pub estudio_id: Option<String>,
    pub caixa: Option<CaixaDoBalcao>,
    /// A API não respondeu: não dá para saber se o caixa está aberto.
    pub indisponivel: bool,
    pub sessoes: Vec<SessaoACobrar>,
    pub sessao: Option<SessaoNoCaixa>,
    pub funcionarios: Vec<Funcionario>,
    pub avisos: Vec<String>,
    pub cupom: Cupom,
    /// As fotos da sessão e onde cada uma acha nome e preço — guardadas para o
    /// cupom acompanhar a galeria sem reler tudo (o painel flutuante).
    pub fotos: Vec<FotoDoCupom>,
    pub faixas: Vec<FaixaDoCatalogo>,
    pub padrao: Faixa,
}

impl Vista {
    /// Refaz o cupom com as fotos de agora — a mesma conta de sempre.
    pub fn recalcular_cupom(&mut self) {
        let vendidas = self
            .sessao
            .as_ref()
            .map(|s| regras::vendidas_na(&s.vendas))
            .unwrap_or_default();
        let (faixas, padrao) = (&self.faixas, &self.padrao);
        self.cupom = regras::fechar_caixa(
            &self.fotos,
            |f| {
                faixas
                    .iter()
                    .find(|p| p.id == f.produto_efetivo)
                    .map(|p| p.faixa.clone())
                    .unwrap_or_else(|| padrao.clone())
            },
            &vendidas,
        );
    }

    pub fn situacao(&self) -> SituacaoDoPdv {
        SituacaoDoPdv {
            tem_estudio: self.estudio_id.is_some(),
            indisponivel: self.indisponivel,
            aberto: self.caixa.is_some(),
        }
    }

    fn estudio(&self) -> Option<&EstudioDoCaixa> {
        let id = self.estudio_id.as_deref()?;
        self.estudios.iter().find(|e| e.id == id)
    }
}

/// Uma navegação em curso: o que já chegou.
#[derive(Default)]
struct Carga {
    estudio_pedido: Option<String>,
    sessao_pedida: Option<String>,
    estudios: Option<Vec<EstudioDoCaixa>>,
    galerias: Option<Option<Vec<SessaoDaLista>>>,
    funcionarios: Option<Vec<Funcionario>>,
    catalogo: Vec<FaixaDoCatalogo>,
    catalogo_pronto: bool,
    paginas: usize,
    /// `Some(None)`: a sessão pedida não existe, foi excluída ou não carregou.
    galeria: Option<Option<GaleriaDoCaixa>>,
    vendas: Option<Option<Vec<Venda>>>,
    /// Decidido quando os estúdios e a sessão pedida chegaram.
    estudio_id: Option<Option<String>>,
    /// `Some(None)`: a leitura do caixa falhou.
    caixa: Option<Option<Option<CaixaDoBalcao>>>,
}

impl Carga {
    fn sessao_chegou(&self) -> bool {
        self.sessao_pedida.is_none() || (self.galeria.is_some() && self.vendas.is_some())
    }

    fn pronta(&self) -> bool {
        self.estudios.is_some()
            && self.galerias.is_some()
            && self.funcionarios.is_some()
            && self.catalogo_pronto
            && self.sessao_chegou()
            && match &self.estudio_id {
                Some(Some(_)) => self.caixa.is_some(),
                Some(None) => true,
                None => false,
            }
    }
}

/// Um aviso passageiro no canto — o `toast` do site.
pub(super) struct RecadoNaTela {
    pub id: u64,
    pub texto: String,
    pub tipo: TipoDeRecado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TipoDeRecado {
    Sucesso,
    Erro,
    Informacao,
}

pub struct Caixa {
    pub(super) publicador: Arc<dyn Publicador>,
    pub(super) sessao: Option<Sessao>,
    /// A sessão escolhida na lista (o `?sessao=` do site).
    pub(super) escolhida: Option<String>,
    /// O estúdio pedido (o `?estudio=`). `None` = o da sessão, ou o primeiro.
    pub(super) estudio_pedido: Option<String>,
    pub(super) vista: Option<Vista>,
    carga: Option<Carga>,
    leituras: (Sender<Recado>, Receiver<Recado>),
    pub(super) gravacoes: (Sender<Recado>, Receiver<Recado>),
    /// Gravações no ar — a colheita continua enquanto houver.
    pub(super) gravando: usize,
    colhendo: bool,
    _colheita: Option<Task<()>>,
    pub(super) foco: FocusHandle,
    pub(super) foco_do_dialogo: FocusHandle,
    /// Pôr o foco na tela no próximo quadro (a tela apareceu, um diálogo fechou).
    pub(super) pedir_foco: bool,
    busca: gpui_kit::Entity<InputState>,
    menu_do_estudio: bool,
    // O PDV.
    pub(super) desconto: DescontoNoTotal,
    pub(super) pessoas: Pessoas,
    lembranca: PathBuf,
    /// Os cadastrados agora pelo botão "Cadastrar" — até a lista ser relida.
    pub(super) cadastrados: Vec<Funcionario>,
    pub(super) ultima_venda: Option<Venda>,
    pub(super) dialogo: Option<Dialogo>,
    /// F4 sem os nomes abre as pessoas antes; ao confirmar, segue para o pagamento.
    pub(super) depois_das_pessoas: bool,
    pub(super) recados: Vec<RecadoNaTela>,
    proximo_recado: u64,
    /// O que as gravações no ar precisam para dar o recibo.
    pub(super) em_curso: Vec<EmCurso>,
    /// As fotos de um estorno voltando a "à venda".
    pub(super) des_sinalizacao: Option<DesSinalizacao>,
    /// Um campo a receber o foco no próximo quadro.
    pub(super) foco_pendente: Option<gpui_kit::Entity<InputState>>,
    /// A rota inteira, ou o painel flutuante da galeria.
    pub(super) modo: Modo,
    /// Uma mudança de faixa ou de negociação em várias fotos, no ar.
    pub(super) lote: Option<Lote>,
    /// Leituras fora de uma carga (a galeria relida pelo painel) — a colheita
    /// continua enquanto houver.
    pub(super) leituras_soltas: usize,
    /// Quem tinha o foco antes de o diálogo abrir — volta a ele ao fechar.
    pub(super) foco_antes: crate::modal::DevolverFoco,
}

/// O último trio escolhido — o balcão costuma repeti-lo o dia inteiro. No site
/// fica no `localStorage`; aqui, ao lado do catálogo.
#[derive(Serialize, Deserialize, Default)]
struct Lembranca {
    fotografo: Option<String>,
    atendente: Option<String>,
    auxiliar: Option<String>,
}

#[cfg(not(test))]
fn caminho_da_lembranca() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("caixa-funcionarios.json")
}

/// 🚨 Nos testes, um arquivo temporário por tela: gravar no catálogo do
/// fotógrafo durante o `cargo test` é o que a tela de sessões já evita.
#[cfg(test)]
fn caminho_da_lembranca() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMA: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-caixa-pessoas-teste-{}-{}.json",
        std::process::id(),
        PROXIMA.fetch_add(1, Ordering::SeqCst)
    ))
}

fn pessoas_lembradas(caminho: &Path) -> Pessoas {
    let lida = std::fs::read_to_string(caminho)
        .ok()
        .and_then(|t| serde_json::from_str::<Lembranca>(&t).ok())
        .unwrap_or_default();
    Pessoas {
        fotografo: lida.fotografo,
        atendente: lida.atendente,
        auxiliar: lida.auxiliar,
    }
}

impl Caixa {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::construir(publicador, Modo::Rota, window, cx)
    }

    pub(super) fn construir(
        publicador: Arc<dyn Publicador>,
        modo: Modo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        init(cx);
        let lembranca = caminho_da_lembranca();
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Buscar sessão ou cliente…"));
        cx.observe(&busca, |_, _, cx| cx.notify()).detach();
        Self {
            publicador,
            sessao: None,
            escolhida: None,
            estudio_pedido: None,
            vista: None,
            carga: None,
            leituras: channel(),
            gravacoes: channel(),
            gravando: 0,
            colhendo: false,
            _colheita: None,
            foco: cx.focus_handle(),
            foco_do_dialogo: cx.focus_handle(),
            pedir_foco: false,
            busca,
            menu_do_estudio: false,
            desconto: DescontoNoTotal::default(),
            pessoas: pessoas_lembradas(&lembranca),
            lembranca,
            cadastrados: Vec::new(),
            ultima_venda: None,
            dialogo: None,
            depois_das_pessoas: false,
            recados: Vec::new(),
            proximo_recado: 0,
            em_curso: Vec::new(),
            des_sinalizacao: None,
            foco_pendente: None,
            modo,
            lote: None,
            leituras_soltas: 0,
            foco_antes: crate::modal::DevolverFoco::default(),
        }
    }

    /// Quem recebe as teclas F do caixa em tela cheia.
    pub fn foco(&self) -> FocusHandle {
        self.foco.clone()
    }

    /// É o painel flutuante da galeria?
    pub fn flutuante(&self) -> bool {
        matches!(self.modo, Modo::Flutuante(_))
    }

    /// A conta do site chegou.
    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// A tela vai aparecer: relê o que o caixa mostra.
    ///
    /// Um diálogo que ficou aberto quando o operador saiu some, como a página
    /// do site que foi desmontada.
    pub fn abrir(&mut self, cx: &mut Context<Self>) {
        self.pedir_foco = true;
        self.dialogo = None;
        self.depois_das_pessoas = false;
        self.recarregar(cx);
    }

    /// Volta da galeria: a mesma sessão continua escolhida. O estúdio passa a
    /// ser o dela — o link de volta do site só leva `?sessao=`. Quem mostra a
    /// tela depois chama [`Caixa::abrir`], que relê tudo.
    pub fn escolher_sessao(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if id.is_some() {
            self.estudio_pedido = None;
        }
        self.escolhida = id;
        cx.notify();
    }

    /// A sessão escolhida agora.
    pub fn sessao_escolhida(&self) -> Option<&str> {
        self.escolhida.as_deref()
    }

    /// O estúdio do caixa na tela, quando já carregou.
    pub fn estudio_do_caixa(&self) -> Option<&str> {
        self.vista.as_ref()?.estudio_id.as_deref()
    }

    /// A tela ainda está buscando o que mostra.
    pub fn carregando(&self) -> bool {
        self.carga.is_some()
    }

    /// Relê a mesma tela — o `revalidatePath` depois de cada gravação.
    pub(super) fn recarregar(&mut self, cx: &mut Context<Self>) {
        self.navegar(self.estudio_pedido.clone(), self.escolhida.clone(), cx);
    }

    /// Vai para `?estudio=&sessao=`: pede tudo de novo.
    pub(super) fn navegar(
        &mut self,
        estudio: Option<String>,
        sessao: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.estudio_pedido = estudio.clone();
        self.escolhida = sessao.clone();
        self.menu_do_estudio = false;
        let Some(conta) = self.sessao.clone() else {
            cx.notify();
            return;
        };
        // Um canal novo: o que a navegação anterior ainda devia cai no vazio.
        self.leituras = channel();
        self.carga = Some(Carga {
            estudio_pedido: estudio,
            sessao_pedida: sessao.clone(),
            ..Carga::default()
        });
        let pedir = |pedido: PedidoJson| {
            self.publicador
                .pedir_json(conta.clone(), pedido, self.leituras.0.clone())
        };
        // 🔑 O painel da galeria não tem lista de sessões nem seletor de
        // estúdio: o caixa é o do estúdio da sessão, e só.
        let flutuante = matches!(self.modo, Modo::Flutuante(_));
        if !flutuante {
            pedir(PedidoJson::ler("estudios", "/bookings/studios/admin"));
            pedir(PedidoJson::ler("galerias", "/pos-venda/galerias"));
        }
        pedir(PedidoJson::ler("funcionarios", "/pos-venda/funcionarios"));
        pedir(PedidoJson::ler("catalogo", pagina_do_catalogo(0)));
        if let Some(id) = &sessao {
            let id = codificar(id);
            pedir(PedidoJson::ler(
                "galeria",
                format!("/pos-venda/galerias/{id}"),
            ));
            pedir(PedidoJson::ler(
                "vendas",
                format!("/pos-venda/caixa/galerias/{id}/vendas"),
            ));
        }
        if flutuante {
            if let Some(carga) = self.carga.as_mut() {
                carga.estudios = Some(Vec::new());
                carga.galerias = Some(Some(Vec::new()));
            }
        }
        self.acompanhar(cx);
        cx.notify();
    }

    /// O canal das leituras da navegação de agora.
    pub(super) fn leituras_de_agora(&self) -> Sender<Recado> {
        self.leituras.0.clone()
    }

    pub(super) fn acompanhar(&mut self, cx: &mut Context<Self>) {
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

    /// Drena os canais. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.leituras.1.try_recv() {
            mudou = true;
            if let Recado::Json { rotulo, resultado } = recado {
                self.receber_leitura(rotulo, resultado);
            }
        }
        while let Ok(recado) = self.gravacoes.1.try_recv() {
            mudou = true;
            if let Recado::Json { rotulo, resultado } = recado {
                self.gravando = self.gravando.saturating_sub(1);
                self.receber_gravacao(rotulo, resultado, cx);
            }
        }
        if self.carga.as_ref().is_some_and(Carga::pronta) {
            if let Some(carga) = self.carga.take() {
                self.vista = Some(self.montar_vista(carga));
            }
            mudou = true;
        }
        if mudou {
            cx.notify();
        }
        let continua = self.carga.is_some() || self.gravando > 0 || self.leituras_soltas > 0;
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn receber_leitura(&mut self, rotulo: &str, resultado: Result<serde_json::Value, String>) {
        if rotulo == "galeria-viva" {
            self.receber_galeria_viva(resultado);
            return;
        }
        let flutuante = self.flutuante();
        let Some(carga) = self.carga.as_mut() else {
            return;
        };
        match rotulo {
            "estudios" => {
                carga.estudios = Some(resultado.and_then(dados::ler_estudios).unwrap_or_default());
            }
            "galerias" => {
                carga.galerias = Some(resultado.and_then(dados::ler_galerias).ok());
            }
            "funcionarios" => {
                carga.funcionarios = Some(
                    resultado
                        .and_then(dados::ler_funcionarios)
                        .unwrap_or_default(),
                );
            }
            "catalogo" => match resultado.and_then(dados::ler_catalogo) {
                Ok(pagina) => {
                    let cheia = pagina.len() == PAGINA_DO_CATALOGO;
                    carga.catalogo.extend(pagina);
                    carga.paginas += 1;
                    if cheia && carga.paginas < MAXIMO_DE_PAGINAS {
                        if let Some(conta) = self.sessao.clone() {
                            self.publicador.pedir_json(
                                conta,
                                PedidoJson::ler("catalogo", pagina_do_catalogo(carga.paginas)),
                                self.leituras.0.clone(),
                            );
                        }
                    } else {
                        carga.catalogo_pronto = true;
                    }
                }
                // Sem catálogo, as faixas da galeria bastam — o site faz igual.
                Err(_) => {
                    carga.catalogo.clear();
                    carga.catalogo_pronto = true;
                }
            },
            "galeria" => {
                carga.galeria = Some(resultado.and_then(dados::ler_galeria).ok());
            }
            "vendas" => {
                carga.vendas = Some(resultado.and_then(dados::ler_vendas).ok());
            }
            "caixa" => {
                carga.caixa = Some(match resultado.and_then(dados::ler_caixa_aberto) {
                    Ok(caixa) => Some(caixa),
                    Err(erro) => {
                        eprintln!("[caixa] leitura falhou: {erro}");
                        None
                    }
                });
            }
            _ => {}
        }
        self.decidir_estudio(flutuante);
    }

    /// O pedido manda; senão o da sessão aberta; senão **o estúdio desta
    /// máquina**; senão o primeiro ativo. Com o estúdio decidido, o caixa dele.
    ///
    /// No painel da galeria o caixa é **o do estúdio da sessão**, ativo ou não
    /// (`carregarGaleria`); sessão sem estúdio não tem caixa.
    fn decidir_estudio(&mut self, flutuante: bool) {
        let Some(carga) = self.carga.as_mut() else {
            return;
        };
        if carga.estudio_id.is_some() || !carga.sessao_chegou() {
            return;
        }
        let Some(estudios) = &carga.estudios else {
            return;
        };
        let ativos: Vec<String> = estudios.iter().map(|e| e.id.clone()).collect();
        let da_sessao = carga
            .galeria
            .as_ref()
            .and_then(|g| g.as_ref())
            .and_then(|g| g.estudio_id.clone());
        let estudio = if flutuante {
            da_sessao
        } else {
            // 🏢 **O estúdio da entrada das sessões entra aqui** (dono,
            // 2026-09-18): sem ele, o caixa abria no primeiro ativo — o de
            // outra cidade metade das vezes.
            let da_maquina = crate::sessoes::tela::estudio_de_trabalho_guardado();
            regras::escolher_estudio(
                &ativos,
                carga.estudio_pedido.as_deref(),
                da_sessao.as_deref(),
                da_maquina.as_deref(),
            )
        };
        if let (Some(id), Some(conta)) = (&estudio, self.sessao.clone()) {
            self.publicador.pedir_json(
                conta,
                PedidoJson::ler(
                    "caixa",
                    format!("/pos-venda/caixa?estudio_id={}", codificar(id)),
                ),
                self.leituras.0.clone(),
            );
        }
        carga.estudio_id = Some(estudio);
    }

    fn montar_vista(&self, carga: Carga) -> Vista {
        let estudios = carga.estudios.unwrap_or_default();
        let estudio_id = carga.estudio_id.flatten();
        let (caixa, indisponivel) = match (&estudio_id, carga.caixa) {
            (Some(_), Some(Some(caixa))) => (caixa, false),
            (Some(_), _) => (None, true),
            (None, _) => (None, false),
        };
        let mut avisos = Vec::new();
        let galerias = carga.galerias.flatten();
        if galerias.is_none() {
            avisos.push("A lista de sessões não carregou. Recarregue a página.".to_string());
        }
        let mut sessoes: Vec<SessaoACobrar> = galerias
            .unwrap_or_default()
            .into_iter()
            .filter(|g| estudio_id.is_some() && g.estudio_id == estudio_id)
            .map(|g| g.sessao)
            .collect();
        regras::ordenar_sessoes(&mut sessoes);

        let mut sessao = None;
        let mut cupom = Cupom::default();
        let mut fotos = Vec::new();
        let mut faixas_da_vista = Vec::new();
        let mut padrao_da_vista = Faixa {
            nome: String::new(),
            preco: 0,
        };
        if carga.sessao_pedida.is_some() {
            match carga.galeria.flatten() {
                None => {
                    avisos.push("A sessão pedida não existe mais ou foi excluída.".into());
                }
                Some(g) if g.estudio_id != estudio_id => avisos.push(if g.estudio_id.is_some() {
                    format!(
                        "“{}” é de outro estúdio: escolha o estúdio dela para cobrar.",
                        g.titulo
                    )
                } else {
                    format!(
                        "“{}” não tem estúdio: escolha o estúdio dela na sessão antes de cobrar.",
                        g.titulo
                    )
                }),
                Some(g) => {
                    let vendas_ok = carga.vendas.as_ref().is_some_and(Option::is_some);
                    let vendas = carga.vendas.flatten().unwrap_or_default();
                    // O caixa da sessão é o do estúdio dela — o mesmo desta tela.
                    if !vendas_ok || indisponivel {
                        avisos.push(
                            "As vendas desta sessão não carregaram: o cupom pode incluir foto já vendida."
                                .into(),
                        );
                    }
                    // O mesmo cupom da galeria: as sinalizadas, com a negociação,
                    // sem as já vendidas; a faixa é a do catálogo, senão a padrão.
                    let faixas = dados::juntar_faixas(&carga.catalogo, &g.produtos);
                    let padrao = g.padrao.clone();
                    cupom = regras::fechar_caixa(
                        &g.fotos,
                        |f| {
                            faixas
                                .iter()
                                .find(|p| p.id == f.produto_efetivo)
                                .map(|p| p.faixa.clone())
                                .unwrap_or_else(|| Faixa {
                                    nome: padrao.nome.clone(),
                                    preco: padrao.preco,
                                })
                        },
                        &regras::vendidas_na(&vendas),
                    );
                    sessao = Some(SessaoNoCaixa {
                        id: g.id,
                        titulo: g.titulo,
                        vendas,
                    });
                    fotos = g.fotos;
                    faixas_da_vista = faixas;
                    padrao_da_vista = padrao;
                }
            }
        }

        Vista {
            estudios,
            estudio_id,
            caixa,
            indisponivel,
            sessoes,
            sessao,
            funcionarios: carga.funcionarios.unwrap_or_default(),
            avisos,
            cupom,
            fotos,
            faixas: faixas_da_vista,
            padrao: padrao_da_vista,
        }
    }

    // ── O PDV ───────────────────────────────────────────────────────────────

    /// Os funcionários conhecidos: a lista, mais os cadastrados agora.
    pub(super) fn todos_os_funcionarios(&self) -> Vec<Funcionario> {
        let mut todos = self
            .vista
            .as_ref()
            .map(|v| v.funcionarios.clone())
            .unwrap_or_default();
        for c in &self.cadastrados {
            if !todos.iter().any(|f| f.id == c.id) {
                todos.push(c.clone());
            }
        }
        todos
    }

    /// O trio guardado, só com quem continua no cadastro e ativo.
    pub(super) fn pessoas_validas(&self) -> Pessoas {
        let ativos: HashSet<String> = self
            .todos_os_funcionarios()
            .into_iter()
            .filter(|f| f.ativo)
            .map(|f| f.id)
            .collect();
        self.pessoas.so_ativos(&ativos)
    }

    pub(super) fn nome_do_funcionario(&self, id: Option<&str>) -> Option<String> {
        let id = id?;
        self.todos_os_funcionarios()
            .into_iter()
            .find(|f| f.id == id)
            .map(|f| f.nome)
    }

    pub(super) fn guardar_pessoas(&mut self, pessoas: Pessoas) {
        let lembranca = Lembranca {
            fotografo: pessoas.fotografo.clone(),
            atendente: pessoas.atendente.clone(),
            auxiliar: pessoas.auxiliar.clone(),
        };
        self.pessoas = pessoas;
        if let Ok(texto) = serde_json::to_string_pretty(&lembranca) {
            if let Some(pasta) = self.lembranca.parent() {
                let _ = std::fs::create_dir_all(pasta);
            }
            // Sem disco, vale até fechar o app — como o site sem armazenamento.
            let _ = std::fs::write(&self.lembranca, texto);
        }
    }

    pub(super) fn cupom(&self) -> Cupom {
        self.vista
            .as_ref()
            .map(|v| v.cupom.clone())
            .unwrap_or_default()
    }

    pub(super) fn desconto_no_total(&self) -> i64 {
        let base = self.vista.as_ref().map_or(0, |v| v.cupom.total);
        regras::ler_desconto(&self.desconto.texto, self.desconto.modo, base).unwrap_or(0)
    }

    pub(super) fn a_receber(&self) -> i64 {
        self.vista.as_ref().map_or(0, |v| v.cupom.total) - self.desconto_no_total()
    }

    fn situacao(&self) -> SituacaoDoPdv {
        self.vista.as_ref().map_or(
            SituacaoDoPdv {
                tem_estudio: false,
                indisponivel: false,
                aberto: false,
            },
            Vista::situacao,
        )
    }

    /// Aplica um desfecho do core: segue com `seguir`, ou abre o caixa, ou avisa.
    fn resolver(
        &mut self,
        desfecho: Desfecho,
        seguir: TipoDeDialogo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A frase de "falta o estúdio" diz onde escolhê-lo — e no painel da
        // galeria é no cabeçalho da sessão.
        let desfecho = match desfecho {
            Desfecho::Erro(frase) if self.flutuante() && frase == regras::SEM_ESTUDIO => {
                Desfecho::Erro(regras::SESSAO_SEM_ESTUDIO.into())
            }
            outro => outro,
        };
        match desfecho {
            Desfecho::Seguir => self.abrir_dialogo(seguir, window, cx),
            Desfecho::AbrirCaixa => self.abrir_dialogo(TipoDeDialogo::Abrir, window, cx),
            Desfecho::Erro(frase) => self.avisar(frase, TipoDeRecado::Erro, cx),
            Desfecho::Aviso(frase) => self.avisar(frase, TipoDeRecado::Informacao, cx),
            Desfecho::PessoasAntes => {
                self.abrir_dialogo(TipoDeDialogo::Pessoas, window, cx);
                self.depois_das_pessoas = true;
            }
        }
    }

    /// F2 e F6: o diálogo pedido, ou o de abrir o caixa quando ele está fechado.
    pub(super) fn com_caixa(
        &mut self,
        tipo: TipoDeDialogo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let desfecho = regras::precisa_caixa(self.situacao());
        self.resolver(desfecho, tipo, window, cx);
    }

    /// F4.
    pub(super) fn finalizar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tem_sessao = self.vista.as_ref().is_some_and(|v| v.sessao.is_some());
        let itens = self.vista.as_ref().map_or(0, |v| v.cupom.itens.len());
        let desfecho = regras::finalizar(
            self.situacao(),
            tem_sessao,
            itens,
            self.pessoas_validas().completas(),
        );
        self.resolver(desfecho, TipoDeDialogo::Pagamento, window, cx);
    }

    /// F8: abre o caixa fechado, fecha o aberto.
    pub(super) fn abrir_ou_fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let situacao = self.situacao();
        let seguir = if situacao.aberto {
            TipoDeDialogo::Fechar
        } else {
            TipoDeDialogo::Abrir
        };
        self.resolver(regras::abrir_ou_fechar(situacao), seguir, window, cx);
    }

    pub(super) fn tecla(&mut self, tecla: u8, window: &mut Window, cx: &mut Context<Self>) {
        // 🔑 Com diálogo aberto as teclas F esperam — menos o F4 do pagamento,
        // que conclui a venda mesmo com o foco fora do formulário.
        if let Some(dialogo) = &self.dialogo {
            if tecla == 4 && matches!(dialogo, Dialogo::Pagamento(_)) {
                self.concluir_venda(window, cx);
            }
            return;
        }
        match tecla {
            1 => self.abrir_dialogo(TipoDeDialogo::Atalhos, window, cx),
            2 => self.com_caixa(TipoDeDialogo::Desconto, window, cx),
            3 => self.abrir_dialogo(TipoDeDialogo::Pessoas, window, cx),
            4 => self.finalizar(window, cx),
            6 => self.com_caixa(TipoDeDialogo::Movimento, window, cx),
            7 => self.abrir_dialogo(TipoDeDialogo::Vendas, window, cx),
            8 => self.abrir_ou_fechar(window, cx),
            9 => self.alternar_painel(cx),
            _ => {}
        }
    }

    /// Um aviso no canto, que some sozinho.
    pub(super) fn avisar(
        &mut self,
        texto: impl Into<String>,
        tipo: TipoDeRecado,
        cx: &mut Context<Self>,
    ) {
        let duracao = if tipo == TipoDeRecado::Erro { 8 } else { 5 };
        self.avisar_por(texto, tipo, Duration::from_secs(duracao), cx);
    }

    pub(super) fn avisar_por(
        &mut self,
        texto: impl Into<String>,
        tipo: TipoDeRecado,
        duracao: Duration,
        cx: &mut Context<Self>,
    ) {
        let id = self.proximo_recado;
        self.proximo_recado += 1;
        self.recados.push(RecadoNaTela {
            id,
            texto: texto.into(),
            tipo,
        });
        cx.spawn(async move |esta, cx| {
            cx.background_executor().timer(duracao).await;
            let _ = esta.update(cx, |tela, cx| {
                tela.recados.retain(|r| r.id != id);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Os avisos na tela agora — para os testes.
    pub fn avisos_passageiros(&self) -> Vec<String> {
        self.recados.iter().map(|r| r.texto.clone()).collect()
    }

    /// Uma gravação em nome da conta. A resposta volta em [`Caixa::colher`].
    pub(super) fn gravar(
        &mut self,
        rotulo: &'static str,
        metodo: &'static str,
        caminho: String,
        corpo: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(conta) = self.sessao.clone() else {
            self.avisar(
                "Esta tela precisa da conta do site.",
                TipoDeRecado::Erro,
                cx,
            );
            return false;
        };
        self.gravando += 1;
        self.publicador.pedir_json(
            conta,
            PedidoJson::gravar(rotulo, metodo, caminho, corpo),
            self.gravacoes.0.clone(),
        );
        self.acompanhar(cx);
        true
    }
}

fn pagina_do_catalogo(pagina: usize) -> String {
    format!(
        "/products/admin?limit={PAGINA_DO_CATALOGO}&offset={}",
        pagina * PAGINA_DO_CATALOGO
    )
}

/// O `encodeURIComponent` do site, para ids e parâmetros.
pub(super) fn codificar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for b in texto.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => saida.push(b as char),
            _ => saida.push_str(&format!("%{b:02X}")),
        }
    }
    saida
}

// ── O desenho ───────────────────────────────────────────────────────────────

/// Um rótulo com a tecla ao lado (`Desconto F2`).
pub(super) fn com_tecla(
    botao: gpui_kit::component::button::Button,
    texto: &str,
    tecla: &str,
) -> gpui_kit::component::button::Button {
    botao
        .child(texto.to_string())
        .child(estilo::tecla(tecla.to_string()))
}

impl Render for Caixa {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.flutuante() {
            return self.render_flutuante(window, cx);
        }
        if self.pedir_foco {
            self.pedir_foco = false;
            if self.dialogo.is_none() {
                window.focus(&self.foco, cx);
            }
        }
        self.aplicar_pendencias_do_dialogo(window, cx);
        let (fundo, texto) = (cx.theme().background, cx.theme().foreground);
        let dialogo = self.render_dialogo(window, cx);
        let recados = self.render_recados(window, cx);

        v_flex()
            .id("caixa")
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(|t, _: &TeclaF1, w, cx| t.tecla(1, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF2, w, cx| t.tecla(2, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF3, w, cx| t.tecla(3, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF4, w, cx| t.tecla(4, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF6, w, cx| t.tecla(6, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF7, w, cx| t.tecla(7, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF8, w, cx| t.tecla(8, w, cx)))
            .relative()
            .size_full()
            .min_h(px(0.))
            .p(px(24.))
            .gap(px(24.))
            .bg(fundo)
            .text_color(texto)
            .text_sm()
            .child(self.cabecalho(cx))
            .children(self.avisos_da_pagina(cx))
            .child(
                // `flex` sem `items_center`: as colunas esticam até o rodapé.
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(24.))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(16.))
                            .child(self.lista_de_sessoes(cx))
                            .child(self.cupom_da_tela(cx)),
                    )
                    .child(self.movimento_do_caixa(cx)),
            )
            .children(dialogo)
            .children(recados)
            .into_any_element()
    }
}

impl Caixa {
    fn cabecalho(&mut self, cx: &mut Context<Self>) -> Div {
        let apagado = cx.theme().muted_foreground;
        let (descricao, situacao, cor) = match &self.vista {
            None if self.sessao.is_none() => (
                "Esta tela precisa da conta do site. Feche e abra o app para entrar.".to_string(),
                "Caixa indisponível",
                cx.theme().danger,
            ),
            None => ("Carregando o caixa…".to_string(), "Caixa fechado", apagado),
            Some(v) => {
                let descricao = match (v.estudio(), &v.caixa) {
                    (None, _) => "Nenhum estúdio ativo no cadastro.".to_string(),
                    (Some(e), Some(c)) => format!(
                        "{} · aberto às {} por {} · fundo de troco {}",
                        e.nome,
                        dados::hora_br(&c.aberto_em),
                        c.operador_email,
                        dinheiro::formatar(c.fundo_de_troco)
                    ),
                    (Some(e), None) => format!(
                        "{} · escolha a sessão, confira o cupom e finalize o pagamento.",
                        e.nome
                    ),
                };
                let (situacao, cor) = if v.indisponivel {
                    ("Caixa indisponível", cx.theme().danger)
                } else if v.caixa.is_some() {
                    ("Caixa aberto", esmeralda_500())
                } else {
                    ("Caixa fechado", apagado)
                };
                (descricao, situacao, cor)
            }
        };

        let selo = estilo::selo_contorno(cx)
            .child(div().size(px(8.)).rounded_full().bg(cor))
            .child(situacao);

        let aberto = self.situacao().aberto;
        let fechar = if aberto {
            com_tecla(estilo::botao_perigo("caixa-f8", cx), "Fechar caixa", "F8")
        } else {
            com_tecla(estilo::botao_primario("caixa-f8", cx), "Abrir caixa", "F8")
        };
        let acoes = h_flex()
            .gap(px(8.))
            .child(self.seletor_de_estudio(cx))
            .child(
                com_tecla(
                    estilo::botao_contorno("caixa-f6", cx),
                    "Sangria/suprimento",
                    "F6",
                )
                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                    t.com_caixa(TipoDeDialogo::Movimento, w, cx)
                })),
            )
            .child(
                com_tecla(estilo::botao_contorno("caixa-f7", cx), "Vendas", "F7").on_click(
                    cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.abrir_dialogo(TipoDeDialogo::Vendas, w, cx)
                    }),
                ),
            )
            .child(
                fechar.on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.abrir_ou_fechar(w, cx))),
            )
            .child(
                estilo::botao_fantasma("caixa-f1", cx)
                    .w(px(32.))
                    .px(px(0.))
                    .child(Icon::new(Icone::Keyboard).size(px(16.)))
                    .tooltip("Atalhos (F1)")
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.abrir_dialogo(TipoDeDialogo::Atalhos, w, cx)
                    })),
            );

        estilo::cabecalho_da_pagina(
            "Caixa",
            Some(div().child(descricao).into_any_element()),
            Some(selo.into_any_element()),
            Some(acoes.into_any_element()),
            cx,
        )
    }

    /// O `Select` do estúdio: um botão de 224 px e a lista embaixo.
    fn seletor_de_estudio(&mut self, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (apagado, fundo_do_menu, borda, acento) = (
            tema.muted_foreground,
            tema.popover,
            tema.border,
            tema.accent,
        );
        let estudios = self
            .vista
            .as_ref()
            .map(|v| v.estudios.clone())
            .unwrap_or_default();
        let atual = self.vista.as_ref().and_then(|v| v.estudio_id.clone());
        let nome = self
            .vista
            .as_ref()
            .and_then(Vista::estudio)
            .map(|e| e.nome.clone());

        let botao = estilo::botao_contorno("caixa-estudio", cx)
            .w(px(224.))
            .justify_between()
            .child(
                // `flex_1`: o conteúdo do `Button` centraliza, e o seletor do
                // site tem o nome à esquerda e a seta à direita.
                div()
                    .flex_1()
                    .truncate()
                    .when(nome.is_none(), |d| d.text_color(apagado))
                    .child(nome.unwrap_or_else(|| "Estúdio…".into())),
            )
            .child(
                Icon::new(Icone::ChevronDown)
                    .size(px(16.))
                    .text_color(apagado),
            )
            .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                t.menu_do_estudio = !t.menu_do_estudio;
                cx.notify();
            }));

        let menu = self.menu_do_estudio.then(|| {
            gpui_kit::deferred(
                gpui_kit::anchored()
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        v_flex()
                            .id("caixa-estudios")
                            .occlude()
                            .mt(px(4.))
                            .w(px(224.))
                            .p(px(4.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(borda)
                            .bg(fundo_do_menu)
                            .shadow_md()
                            .on_mouse_down_out(cx.listener(|t, _, _, cx| {
                                t.menu_do_estudio = false;
                                cx.notify();
                            }))
                            .children(estudios.into_iter().map(|e| {
                                let escolhido = atual.as_deref() == Some(e.id.as_str());
                                let id = e.id.clone();
                                h_flex()
                                    .id(SharedString::from(format!("caixa-estudio-{}", e.id)))
                                    .h(px(32.))
                                    .px(px(8.))
                                    .gap(px(8.))
                                    .rounded(px(6.))
                                    .justify_between()
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(acento))
                                    .child(div().truncate().child(e.nome))
                                    .when(escolhido, |d| {
                                        d.child(Icon::new(Icone::Check).size(px(16.)))
                                    })
                                    .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                        t.menu_do_estudio = false;
                                        if Some(&id)
                                            != t.vista.as_ref().and_then(|v| v.estudio_id.as_ref())
                                        {
                                            t.navegar(Some(id.clone()), None, cx);
                                        }
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
            .with_priority(1)
        });

        div().child(botao).children(menu)
    }

    fn avisos_da_pagina(&self, cx: &mut Context<Self>) -> Option<Div> {
        let vista = self.vista.as_ref()?;
        if !vista.indisponivel && vista.avisos.is_empty() {
            return None;
        }
        Some(
            v_flex()
                .gap(px(8.))
                .when(vista.indisponivel, |d| {
                    d.child(estilo::aviso(
                        "O caixa não respondeu: não dá para saber se ele está aberto. Recarregue a página.",
                        true,
                        cx,
                    ))
                })
                .children(vista.avisos.iter().map(|a| estilo::aviso(a.clone(), false, cx))),
        )
    }

    fn lista_de_sessoes(&mut self, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado, realce, secundario, sobre_secundario) = (
            tema.border,
            tema.muted_foreground,
            tema.muted,
            tema.secondary,
            tema.secondary_foreground,
        );
        let busca = self.busca.read(cx).value().to_string();
        let sessoes = self
            .vista
            .as_ref()
            .map(|v| v.sessoes.clone())
            .unwrap_or_default();
        let filtradas: Vec<SessaoACobrar> = regras::filtrar_sessoes(&sessoes, &busca)
            .into_iter()
            .cloned()
            .collect();
        let escolhida = self
            .vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| s.id.clone());
        let estudio = self.vista.as_ref().and_then(|v| v.estudio_id.clone());

        let vazia = filtradas.is_empty().then(|| {
            div()
                .p(px(16.))
                .text_center()
                .text_color(apagado)
                .child(if self.vista.is_none() {
                    "Carregando…"
                } else if sessoes.is_empty() {
                    "Nenhuma sessão neste estúdio."
                } else {
                    "Nenhuma sessão com esse nome."
                })
        });

        v_flex()
            .w(px(272.))
            .flex_none()
            .min_h(px(0.))
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .overflow_hidden()
            .child(
                div().p(px(8.)).border_b_1().border_color(borda).child(
                    Input::new(&self.busca)
                        .prefix(Icon::new(Icone::Search).size(px(16.)).text_color(apagado)),
                ),
            )
            .child(
                v_flex()
                    .id("caixa-sessoes")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(vazia)
                    .children(filtradas.into_iter().map(|s| {
                        let e_a_escolhida = escolhida.as_deref() == Some(s.id.as_str());
                        let id = s.id.clone();
                        let estudio = estudio.clone();
                        let linha2 = [Some(dados::data_br(&s.criada_em)), s.contato.clone()]
                            .into_iter()
                            .flatten()
                            .filter(|p| !p.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        v_flex()
                            .id(SharedString::from(format!("caixa-sessao-{}", s.id)))
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(borda)
                            .cursor_pointer()
                            .hover(move |d| d.bg(realce.opacity(0.6)))
                            .when(e_a_escolhida, |d| d.bg(realce))
                            .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                t.navegar(estudio.clone(), Some(id.clone()), cx);
                            }))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(s.titulo.clone()),
                                    )
                                    .when(s.sinalizadas > 0, |d| {
                                        d.child(
                                            div()
                                                .flex_none()
                                                .h(px(20.))
                                                .min_w(px(20.))
                                                .px(px(6.))
                                                .rounded_full()
                                                .bg(secundario)
                                                .text_color(sobre_secundario)
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(s.sinalizadas.to_string()),
                                        )
                                    }),
                            )
                            .child(div().truncate().text_xs().text_color(apagado).child(linha2))
                    })),
            )
    }

    fn cupom_da_tela(&mut self, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado, cartao, realce, frente, fundo) = (
            tema.border,
            tema.muted_foreground,
            tema.popover,
            tema.muted,
            tema.foreground,
            tema.background,
        );
        let mono = tema.mono_font_family.clone();
        let cupom = self.cupom();
        let n = cupom.itens.len();
        let sessao = self
            .vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| (s.id.clone(), s.titulo.clone()));
        let pessoas = self.pessoas_validas();
        let desconto_no_total = self.desconto_no_total();
        let a_receber = self.a_receber();

        let cabeca = h_flex()
            .justify_between()
            .gap(px(12.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(borda)
            .bg(realce.opacity(0.4))
            .child(
                v_flex()
                    .min_w(px(0.))
                    .child(
                        div().truncate().font_weight(FontWeight::MEDIUM).child(
                            sessao
                                .as_ref()
                                .map_or("Nenhuma sessão escolhida".to_string(), |s| s.1.clone()),
                        ),
                    )
                    .child(div().truncate().text_xs().text_color(apagado).child(
                        if sessao.is_some() {
                            let mut frase = format!("{} a cobrar", regras::itens(n));
                            if cupom.ja_vendidas > 0 {
                                frase.push_str(&format!(
                                    " · {} já vendida(s), fora do cupom",
                                    cupom.ja_vendidas
                                ));
                            }
                            frase
                        } else {
                            "Escolha a sessão na lista para montar o cupom.".to_string()
                        },
                    )),
            )
            .when_some(sessao.clone(), |d, (id, _)| {
                d.child(
                    h_flex()
                        .id("caixa-abrir-sessao")
                        .flex_none()
                        .gap(px(4.))
                        .text_color(apagado)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(frente))
                        .child("Abrir sessão")
                        .child(Icon::new(Icone::ExternalLink).size(px(14.)))
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(
                                "Abrir a sessão para sinalizar ou negociar fotos",
                            )
                            .build(window, cx)
                        })
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(PedidoDoCaixa::AbrirSessao(id.clone()));
                        })),
                )
            });

        let banner = self.ultima_venda.as_ref().filter(|_| n == 0).map(|v| {
            let (fundo_verde, _, _) = cores::destaque_esmeralda();
            div()
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(borda)
                .bg(fundo_verde)
                .child(format!(
                    "Venda #{} registrada · {}",
                    v.numero,
                    if v.troco > 0 {
                        format!("troco {}", dinheiro::formatar(v.troco))
                    } else {
                        dinheiro::formatar(v.total)
                    }
                ))
        });

        let vazio = (n == 0).then(|| {
            div()
                .px(px(12.))
                .py(px(32.))
                .text_center()
                .text_color(apagado)
                .child(if sessao.is_some() {
                    "Nenhuma foto sinalizada a cobrar nesta sessão. Sinalize com P na galeria."
                } else {
                    "O cupom aparece aqui."
                })
        });

        let itens = v_flex()
            .id("caixa-itens")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .children(banner)
            .children(vazio.map(|d| d.font_family(mono.clone())))
            .children(cupom.itens.iter().enumerate().map(|(indice, i)| {
                let mut conta: Vec<AnyElement> = vec![
                    div()
                        .child(format!("1 UN × {}", dinheiro::formatar(i.cheio)))
                        .into_any_element(),
                    div().child(i.faixa.clone()).into_any_element(),
                ];
                if let Some(etiqueta) = &i.etiqueta {
                    let (fundo_ambar, _, texto_ambar) = cores::selo_ambar();
                    conta.push(
                        div()
                            .px(px(4.))
                            .rounded(px(4.))
                            .bg(fundo_ambar)
                            .text_color(texto_ambar)
                            .child(etiqueta.clone())
                            .into_any_element(),
                    );
                }
                if i.desconto > 0 {
                    conta.push(
                        div()
                            .child(format!("desc. −{}", dinheiro::formatar(i.desconto)))
                            .into_any_element(),
                    );
                }
                if i.desconto < 0 {
                    conta.push(
                        div()
                            .child(format!("acrésc. +{}", dinheiro::formatar(-i.desconto)))
                            .into_any_element(),
                    );
                }
                if let Some(fora) = i.pago_fora {
                    conta.push(
                        div()
                            .child(format!("pagou {} lá", dinheiro::formatar(fora)))
                            .into_any_element(),
                    );
                }
                h_flex()
                    .items_start()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(borda.opacity(0.8))
                    .font_family(mono.clone())
                    .text_xs()
                    .child(
                        div()
                            .w(px(36.))
                            .flex_none()
                            .text_color(apagado)
                            .child(format!("{:03}", indice + 1)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .child(i.arquivo.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(dinheiro::formatar(i.cobrado)),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_x(px(8.))
                                    .text_size(px(11.))
                                    .text_color(apagado)
                                    .children(conta),
                            ),
                    )
            }));

        let linha = |rotulo: String, valor: String| {
            h_flex()
                .justify_between()
                .gap(px(12.))
                .child(div().text_color(apagado).child(rotulo.to_uppercase()))
                .child(div().text_right().child(valor))
        };
        let mut resumo = v_flex()
            .gap(px(2.))
            .px(px(12.))
            .py(px(8.))
            .font_family(mono.clone())
            .text_xs()
            .child(linha(
                format!("Subtotal ({})", regras::itens(n)),
                dinheiro::formatar(cupom.subtotal),
            ));
        if cupom.descontos > 0 {
            resumo = resumo.child(linha(
                "Descontos nos itens".into(),
                format!("− {}", dinheiro::formatar(cupom.descontos)),
            ));
        }
        if cupom.descontos < 0 {
            resumo = resumo.child(linha(
                "Acréscimos".into(),
                format!("+ {}", dinheiro::formatar(-cupom.descontos)),
            ));
        }
        if cupom.pago_em_parceiro > 0 {
            resumo = resumo.child(linha(
                "Pago em site parceiro".into(),
                format!("− {}", dinheiro::formatar(cupom.pago_em_parceiro)),
            ));
        }
        if desconto_no_total > 0 {
            let rotulo = if self.desconto.modo == ModoDoDesconto::Percentual {
                format!("Desconto no total ({}%)", self.desconto.texto)
            } else {
                "Desconto no total".to_string()
            };
            resumo = resumo.child(linha(
                rotulo,
                format!("− {}", dinheiro::formatar(desconto_no_total)),
            ));
        }
        let quem = if pessoas.alguma() {
            let nome = |id: &Option<String>| {
                self.nome_do_funcionario(id.as_deref())
                    .unwrap_or_else(|| "?".into())
            };
            format!(
                "{} · {} · {}",
                nome(&pessoas.fotografo),
                nome(&pessoas.atendente),
                nome(&pessoas.auxiliar)
            )
        } else {
            "F3 escolhe".to_string()
        };
        resumo = resumo.child(linha("Fotografou · atendeu · auxiliou".into(), quem));

        // `border-t-4 border-double`: duas linhas finas com um vão.
        let borda_dupla = v_flex()
            .child(div().h(px(1.)).bg(borda))
            .child(div().h(px(2.)))
            .child(div().h(px(1.)).bg(borda));

        let total = h_flex()
            .items_end()
            .justify_between()
            .gap(px(12.))
            .px(px(16.))
            .py(px(12.))
            .bg(frente)
            .text_color(fundo)
            .font_family(mono.clone())
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("TOTAL A RECEBER"),
            )
            .child(
                div()
                    .text_size(px(36.))
                    .line_height(px(36.))
                    .font_weight(FontWeight::BOLD)
                    .child(dinheiro::formatar(a_receber)),
            );

        let acoes = h_flex()
            .gap(px(8.))
            .p(px(12.))
            .child(
                com_tecla(estilo::botao_contorno("caixa-f2", cx), "Desconto", "F2").on_click(
                    cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.com_caixa(TipoDeDialogo::Desconto, w, cx)
                    }),
                ),
            )
            .child(
                com_tecla(estilo::botao_contorno("caixa-f3", cx), "Pessoas", "F3").on_click(
                    cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.abrir_dialogo(TipoDeDialogo::Pessoas, w, cx)
                    }),
                ),
            )
            .child(div().flex_1())
            .child(
                com_tecla(
                    estilo::botao_primario("caixa-f4", cx),
                    "Finalizar pagamento",
                    "F4",
                )
                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.finalizar(w, cx))),
            );

        v_flex()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .bg(cartao)
            .overflow_hidden()
            .child(cabeca)
            .child(itens)
            .child(borda_dupla)
            .child(resumo)
            .child(total)
            .child(acoes)
    }

    /// O que aconteceu no caixa aberto: as vendas e os movimentos, um a um.
    ///
    /// 🙈 **Sem total por forma de pagamento**: seria o esperado do fechamento à
    /// vista, e o fechamento conta às cegas antes de conferir.
    fn movimento_do_caixa(&self, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado) = (tema.border, tema.muted_foreground);
        let mono = tema.mono_font_family.clone();
        let caixa = self.vista.as_ref().and_then(|v| v.caixa.as_ref());
        let indisponivel = self.vista.as_ref().is_some_and(|v| v.indisponivel);

        let corpo: Vec<AnyElement> = match caixa {
            None => vec![div()
                .text_color(apagado)
                .child(if indisponivel {
                    "Sem resposta do caixa."
                } else {
                    "Caixa fechado. Abra com F8 para vender."
                })
                .into_any_element()],
            Some(c) => {
                let mut partes = vec![div()
                    .text_xs()
                    .text_color(apagado)
                    .child(format!(
                        "{} venda(s) · {} sangria(s) ou suprimento(s) · {} estorno(s)",
                        c.vendas.len(),
                        c.movimentos.len(),
                        c.estornos.len()
                    ))
                    .into_any_element()];
                if !c.vendas.is_empty() {
                    partes.push(
                        v_flex()
                            .children(c.vendas.iter().rev().enumerate().map(|(i, v)| {
                                let mut formas =
                                    regras::formas_juntas(v.pagamentos.iter().map(|p| p.forma));
                                if v.estornado > 0 {
                                    formas.push_str(&format!(
                                        " · estornado {}",
                                        dinheiro::formatar(v.estornado)
                                    ));
                                }
                                v_flex()
                                    .py(px(6.))
                                    .when(i > 0, |d| d.border_t_1().border_color(borda))
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .gap(px(8.))
                                            .font_family(mono.clone())
                                            .child(format!(
                                                "#{} · {}",
                                                v.numero,
                                                dados::hora_br(&v.criada_em)
                                            ))
                                            .child(dinheiro::formatar(v.total)),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(apagado)
                                            .child(formas),
                                    )
                            }))
                            .into_any_element(),
                    );
                }
                if !c.movimentos.is_empty() {
                    partes.push(
                        v_flex()
                            .border_t_1()
                            .border_color(borda)
                            .pt(px(4.))
                            .children(c.movimentos.iter().enumerate().map(|(i, m)| {
                                h_flex()
                                    .justify_between()
                                    .gap(px(8.))
                                    .py(px(6.))
                                    .when(i > 0, |d| d.border_t_1().border_color(borda))
                                    .child(
                                        h_flex()
                                            .min_w(px(0.))
                                            .child(format!("{} · ", m.tipo.rotulo()))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_color(apagado)
                                                    .child(m.motivo.clone()),
                                            ),
                                    )
                                    .child(div().flex_none().font_family(mono.clone()).child(
                                        format!(
                                            "{} {}",
                                            m.tipo.sinal(),
                                            dinheiro::formatar(m.valor)
                                        ),
                                    ))
                            }))
                            .into_any_element(),
                    );
                }
                partes.push(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child(
                            "Os totais por forma aparecem na conferência do fechamento, depois da contagem.",
                        )
                        .into_any_element(),
                );
                partes
            }
        };

        div().w(px(320.)).flex_none().min_h(px(0.)).child(
            v_flex()
                .id("caixa-movimento")
                .max_h_full()
                .overflow_y_scroll()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(borda)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Movimento do caixa"),
                )
                .children(corpo),
        )
    }

    pub(super) fn render_recados(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.recados.is_empty() {
            return None;
        }
        let tema = cx.theme();
        let (fundo, borda, texto, perigo) =
            (tema.popover, tema.border, tema.foreground, tema.danger);
        let tamanho = window.viewport_size();
        Some(
            gpui_kit::deferred(
                gpui_kit::anchored()
                    .anchor(gpui_kit::Anchor::BottomRight)
                    .position(gpui_kit::point(
                        tamanho.width - px(16.),
                        tamanho.height - px(16.),
                    ))
                    .position_mode(gpui_kit::AnchoredPositionMode::Window)
                    .child(
                        v_flex()
                            .gap(px(8.))
                            .w(px(356.))
                            .children(self.recados.iter().map(|r| {
                                let (icone, cor) = match r.tipo {
                                    TipoDeRecado::Sucesso => (Icone::Check, cores::sucesso()),
                                    TipoDeRecado::Erro => (Icone::CircleAlert, perigo),
                                    TipoDeRecado::Informacao => (Icone::Info, texto),
                                };
                                h_flex()
                                    .items_start()
                                    .gap(px(8.))
                                    .p(px(16.))
                                    .rounded(px(8.))
                                    .border_1()
                                    .border_color(borda)
                                    .bg(fundo)
                                    .shadow_lg()
                                    .text_sm()
                                    .child(Icon::new(icone).size(px(16.)).text_color(cor))
                                    .child(div().flex_1().text_color(texto).child(r.texto.clone()))
                            })),
                    ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }
}

/// O `bg-emerald-500` da bolinha de "Caixa aberto".
fn esmeralda_500() -> Hsla {
    gpui_kit::rgb(0x00bc7d).into()
}

#[cfg(test)]
#[path = "testes.rs"]
mod testes;
