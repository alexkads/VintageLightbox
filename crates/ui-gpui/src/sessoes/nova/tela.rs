//! A tela do assistente: o estado e os gestos. O desenho está em
//! [`super::desenho`].
//!
//! # O que é igual ao site (`nova/assistente.tsx`)
//!
//! - Sete etapas; **nenhuma trava a outra**. Só a etapa 3 segura o "Avançar"
//!   quando falta preço ou estúdio, e "Criar" leva à primeira pendência.
//! - O rascunho é gravado a cada mudança, e as fotos entram no catálogo com o
//!   `sessao_id` provisório (`rascunho:<uuid>`). **Nada sobe enquanto é
//!   rascunho**: sem sessão no site, não há para onde subir.
//! - Criar espera a cópia local, grava o id devolvido **antes** de mover as
//!   fotos, e só então as passa para a sessão.
//!
//! # O que é do desktop
//!
//! A etapa 1 é sempre "feita" (o app já é o aplicativo), e o assistente abre
//! na etapa 2.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::services::pos_venda::{Estudio, GaleriaDoPainel, Produto, Sessao};
use domain::services::PreviewType;
use domain::value_objects::{ImportMode, ImportOptions, OrganizationStrategy, RenamePattern};
use gpui::{
    prelude::*, App, Context, Entity, EventEmitter, FocusHandle, Focusable, RenderImage,
    ScrollHandle, SharedString, Subscription, Task, Window,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::{SearchableVec, SelectEvent, SelectItem, SelectState};
use infrastructure::cache::preview_manager::PreviewManager;

use super::amostras::Amostras;
use super::associacoes::{
    self as assoc, AgendamentoEscolhido, CompraEscolhida, ParceiroEscolhido, VoucherEscolhido,
};
use super::estado::{self, Campo, EstadoDaEtapa, Formulario, Rascunho};
use super::receita::{self, PresetDaSessao};
use crate::biblioteca::acervo::Acervo;
use crate::importacao::estado::Recado as RecadoDaImportacao;
use crate::importacao::explorador::{Andamento, Explorador, Freios, Importador, SeletorDePasta};
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::revelacao::persistencia::{self, Gravador};
use crate::sessoes::arquivos::SeletorDeFotos;
use crate::sessoes::detalhe::{pasta_do_ensaio, Importacao};

/// Um canal com as duas pontas guardadas juntas.
type Canal<T> = (Sender<T>, Receiver<T>);

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);
const ESPERA_DA_BUSCA: Duration = Duration::from_millis(300);
const DURACAO_DO_AVISO: Duration = Duration::from_secs(5);
const DURACAO_DO_AVISO_LONGO: Duration = Duration::from_secs(12);

/// O que a tela pede à raiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoDaNova {
    /// "← Sessões fotográficas": o rascunho continua guardado.
    Voltar,
    /// A sessão existe e as fotos passaram para ela.
    Criada(String),
    /// O catálogo mudou (importação, descarte, troca de dono).
    CatalogoMudou,
    /// O botão do menu lateral.
    AlternarMenu,
    /// Fotos entraram na fila da receita padrão: a raiz liga a colheita dos
    /// avisos, que ela repassa a esta tela e à da sessão.
    RevelandoReceita,
}

impl EventEmitter<PedidoDaNova> for NovaSessao {}

/// Uma opção dos `Select` de preço, estúdio e tipo de parceiro.
#[derive(Clone)]
pub(super) struct Opcao {
    pub id: String,
    pub titulo: SharedString,
}

impl SelectItem for Opcao {
    type Value = String;

    fn title(&self) -> SharedString {
        self.titulo.clone()
    }

    fn value(&self) -> &String {
        &self.id
    }
}

pub(super) type Escolha = SelectState<SearchableVec<Opcao>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDeBusca {
    Agendamento,
    Voucher,
    Compra,
    Parceiro,
}

impl TipoDeBusca {
    fn rotulo(self) -> &'static str {
        match self {
            TipoDeBusca::Agendamento => "nova-busca-agendamentos",
            TipoDeBusca::Voucher => "nova-busca-vouchers",
            TipoDeBusca::Compra => "nova-busca-compras",
            TipoDeBusca::Parceiro => "nova-busca-parceiros",
        }
    }

    fn o_que(self) -> &'static str {
        match self {
            TipoDeBusca::Agendamento => "agendamentos",
            TipoDeBusca::Voucher => "vouchers",
            TipoDeBusca::Compra => "compras antecipadas",
            TipoDeBusca::Parceiro => "parceiros",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemDaBusca {
    Agendamento(AgendamentoEscolhido),
    Voucher(VoucherEscolhido),
    Compra(CompraEscolhida),
    Parceiro(ParceiroEscolhido),
}

/// O modal de busca de uma associação.
pub(super) struct Busca {
    pub tipo: TipoDeBusca,
    pub campo: Entity<InputState>,
    pub carregando: bool,
    pub erro: Option<String>,
    pub itens: Vec<ItemDaBusca>,
    pub com_texto: bool,
    enviados: u32,
    recebidos: u32,
    agendada: Option<Instant>,
    _assinatura: Subscription,
}

/// O cadastro rápido de parceiro — na etapa 6 ou no rodapé do modal.
pub(super) struct Cadastro {
    pub nome: Entity<InputState>,
    pub whatsapp: Entity<InputState>,
    pub email: Entity<InputState>,
    pub tipo: Entity<Escolha>,
    pub tipo_escolhido: Option<String>,
    pub enviando: bool,
    pub erro: Option<String>,
    pub existente: Option<ParceiroEscolhido>,
    _assinatura: Subscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmacao {
    /// O "Descartar" do cabeçalho.
    Descartar,
    /// O "Descartar e começar outra" do cartão de retomada.
    DescartarGuardado,
    /// As fotos de rascunhos antigos.
    ApagarOrfas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fase {
    /// Esperando a cópia local terminar.
    Copiando,
    /// O `POST` da galeria saiu.
    Criando,
    /// A sessão existe; as fotos estão mudando de dono.
    Movendo,
}

pub(super) struct Aviso {
    pub texto: String,
    pub erro: bool,
    ate: Instant,
}

/// O andamento da receita padrão sobre as fotos do rascunho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Receita {
    pub total: usize,
    pub prontas: usize,
}

/// Os cartões montados, para o menu "Do cartão ou pasta…".
pub(super) struct MenuDaOrigem {
    pub procurando: bool,
    pub cartoes: Vec<(String, String)>,
}

/// As portas que a tela usa.
pub struct PortasDaNova {
    pub publicador: Arc<dyn Publicador>,
    pub seletor_de_fotos: Arc<dyn SeletorDeFotos>,
    pub importador: Arc<dyn Importador>,
    pub acervo: Arc<dyn Acervo>,
    pub gravador: Arc<dyn Gravador>,
    pub previews: Arc<PreviewManager>,
    /// Quem revela a receita padrão em segundo plano.
    ///
    /// 🔑 **Porta, e não campo da tela.** O assistente sai de cena quando a
    /// sessão nasce, e a revelação não pode sair com ele — é o pedido do dono
    /// ("em segundo plano, e ao abrir a sessão ir acontecendo"). O mesmo serviço
    /// serve a sessão, e por isso as duas telas mostram a mesma imagem.
    pub receita_padrao: Arc<crate::sessoes::receita_padrao::ReceitaPadrao>,
    pub explorador: Arc<dyn Explorador>,
    pub seletor_de_pasta: Arc<dyn SeletorDePasta>,
    pub presets_do_sistema: Vec<Preset>,
}

pub struct NovaSessao {
    pub(super) portas: PortasDaNova,
    sessao: Option<Sessao>,
    caminho: PathBuf,
    pub(super) rascunho: Rascunho,
    /// O rascunho achado ao abrir, esperando "Retomar" ou "Descartar".
    pub(super) guardado: Option<Rascunho>,
    /// As fotos do rascunho guardado (para a frase do cartão).
    pub(super) fotos_do_guardado: usize,
    pub(super) procurando_rascunho: bool,
    pub(super) produtos: Vec<Produto>,
    pub(super) estudios: Vec<Estudio>,
    galerias: Option<Vec<GaleriaDoPainel>>,
    pub(super) presets: Vec<PresetDaSessao>,
    presets_chegaram: bool,
    pub(super) titulo: Entity<InputState>,
    pub(super) email: Entity<InputState>,
    pub(super) whatsapp: Entity<InputState>,
    pub(super) detalhe: Entity<InputState>,
    pub(super) escolha_do_produto: Entity<Escolha>,
    pub(super) escolha_do_estudio: Entity<Escolha>,
    /// Os erros por campo só aparecem depois da primeira tentativa.
    pub(super) tentou: bool,
    pub(super) fase: Option<Fase>,
    pub(super) erro: Option<(String, Option<usize>)>,
    pub(super) confirmacao: Option<Confirmacao>,
    pub(super) busca: Option<Busca>,
    pub(super) cadastro: Option<Cadastro>,
    pub(super) menu_da_origem: Option<MenuDaOrigem>,
    /// As fotos do catálogo com o `sessao_id` do rascunho.
    pub(super) fotos: Vec<PhotoViewModel>,
    /// As de outros rascunhos, que ninguém mais vai criar.
    pub(super) orfas: Vec<String>,
    pub(super) importacao: Option<Importacao>,
    /// As levas soltas enquanto uma cópia andava, na ordem em que chegaram.
    ///
    /// 🚨 **Antes, soltar fotos durante a cópia era recusado** — "Espere a cópia
    /// em curso terminar para adicionar mais fotos." (achado do dono,
    /// 17/set/2026: a importação tem de andar em segundo plano). O site nunca
    /// recusou: `copiarParaORascunho` põe a leva numa cadeia de promessas e o
    /// operador segue soltando cartões (`nova/copia-local.ts`).
    ///
    /// 🔑 **Fila, e não paralelo**: `ImportadorDoDisco::importar` dá um
    /// `tokio.spawn` por chamada, e duas levas ao mesmo tempo intercalariam a
    /// ordem das fotos no catálogo — que é a ordem em que o balcão as mostra.
    /// A cadeia do site serializa pelo mesmo motivo.
    pub(super) fila_de_levas: std::collections::VecDeque<Vec<String>>,
    pub(super) falhas_da_copia: Option<(usize, String)>,
    freios: Freios,
    escolhendo: bool,
    pub(super) receita: Receita,
    receita_aplicada: HashMap<String, String>,
    pub(super) arrastando: bool,
    pub(super) aviso: Option<Aviso>,
    pub(super) rolagem: ScrollHandle,
    pub(super) foco: FocusHandle,
    pub(super) foco_do_preset: usize,
    /// As setas andaram pelos cartões: o anel de foco aparece.
    pub(super) teclado_no_preset: bool,
    /// As miniaturas das fotos do rascunho, por id.
    pub(super) miniaturas: HashMap<String, Arc<RenderImage>>,
    /// Fotos foram para a fila da receita: o próximo quadro avisa a raiz.
    ///
    /// 🔑 **Bandeira, e não `cx.emit` direto**: `aplicar_receita` é chamado de
    /// lugares sem `Context` (a releitura do catálogo, a chegada dos presets), e
    /// dar um `cx` a ela só para isto espalharia o contexto por meio módulo.
    pub(super) pedir_colheita_das_reveladas: bool,
    pub(super) amostras: Amostras,
    /// O campo que o próximo quadro deve focar.
    focar: Option<Campo>,
    recados: (Sender<Recado>, Receiver<Recado>),
    escolhas: (Sender<Vec<String>>, Receiver<Vec<String>>),
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    releituras: (Sender<Vec<PhotoViewModel>>, Receiver<Vec<PhotoViewModel>>),
    catalogo: Canal<Result<usize, String>>,
    origens: (Sender<RecadoDaImportacao>, Receiver<RecadoDaImportacao>),
    esperando_catalogo: Option<Fase>,
    esperando_descarte: bool,
    esperando_releitura: bool,
    carregando: bool,
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn opcoes_de_produto(produtos: &[Produto], atual: &str) -> Vec<Opcao> {
    let mut opcoes: Vec<Opcao> = produtos
        .iter()
        .map(|p| Opcao {
            id: p.id.clone(),
            titulo: format!(
                "{} — {}{}",
                p.nome,
                biblioteca_core::dinheiro::ler_campo(&p.preco.replace('.', ","))
                    .map(biblioteca_core::dinheiro::formatar)
                    .unwrap_or_else(|| format!("R$ {}", p.preco)),
                if p.inativo { " (fora da vitrine)" } else { "" }
            )
            .into(),
        })
        .collect();
    if !atual.is_empty() && !produtos.iter().any(|p| p.id == atual) {
        opcoes.push(Opcao {
            id: atual.to_string(),
            titulo: "Produto fora do catálogo".into(),
        });
    }
    opcoes
}

fn opcoes_de_estudio(estudios: &[Estudio], atual: &str) -> Vec<Opcao> {
    let mut opcoes: Vec<Opcao> = estudios
        .iter()
        .map(|e| Opcao {
            id: e.id.clone(),
            titulo: if e.cidade.trim().is_empty() {
                e.nome.clone()
            } else {
                format!("{} — {}", e.nome, e.cidade)
            }
            .into(),
        })
        .collect();
    if !atual.is_empty() && !estudios.iter().any(|e| e.id == atual) {
        opcoes.push(Opcao {
            id: atual.to_string(),
            titulo: "Estúdio fora do cadastro".into(),
        });
    }
    opcoes
}

impl NovaSessao {
    pub fn nova(portas: PortasDaNova, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let campo = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let titulo = campo("Ensaio da Maria — 13/09", window, cx);
        let email = campo("maria@exemplo.com", window, cx);
        let whatsapp = campo("(47) 99999-8888", window, cx);
        let detalhe = campo("Rádio, feira, cartaz na praça…", window, cx);
        let escolha_do_produto =
            cx.new(|cx| SelectState::new(SearchableVec::new(Vec::new()), None, window, cx));
        let escolha_do_estudio =
            cx.new(|cx| SelectState::new(SearchableVec::new(Vec::new()), None, window, cx));

        let mut assinaturas = Vec::new();
        #[derive(Clone, Copy)]
        enum Texto {
            Titulo,
            Email,
            Whatsapp,
            Detalhe,
        }
        for (entidade, qual) in [
            (&titulo, Texto::Titulo),
            (&email, Texto::Email),
            (&whatsapp, Texto::Whatsapp),
            (&detalhe, Texto::Detalhe),
        ] {
            assinaturas.push(cx.subscribe_in(
                entidade,
                window,
                move |tela, estado, evento: &InputEvent, window, cx| match evento {
                    InputEvent::Change => {
                        let valor = estado.read(cx).value().to_string();
                        let f = &mut tela.rascunho.formulario;
                        let alvo = match qual {
                            Texto::Titulo => &mut f.titulo,
                            Texto::Email => &mut f.email,
                            Texto::Whatsapp => &mut f.whatsapp,
                            Texto::Detalhe => &mut f.como_conheceu_detalhe,
                        };
                        if *alvo == valor {
                            return;
                        }
                        *alvo = valor;
                        tela.guardar();
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => tela.acao_principal(window, cx),
                    _ => {}
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &escolha_do_produto,
            window,
            |tela, _, evento: &SelectEvent<SearchableVec<Opcao>>, _, cx| {
                let SelectEvent::Confirm(valor) = evento;
                tela.rascunho.formulario.produto_id = valor.clone().unwrap_or_default();
                tela.guardar();
                cx.notify();
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &escolha_do_estudio,
            window,
            |tela, _, evento: &SelectEvent<SearchableVec<Opcao>>, _, cx| {
                let SelectEvent::Confirm(valor) = evento;
                tela.rascunho.formulario.estudio_id = valor.clone().unwrap_or_default();
                tela.guardar();
                cx.notify();
            },
        ));

        let caminho = estado::caminho_do_rascunho();
        Self {
            portas,
            sessao: None,
            rascunho: Rascunho::novo(
                &uuid::Uuid::new_v4().to_string(),
                Formulario::default(),
                agora(),
            ),
            caminho,
            guardado: None,
            fotos_do_guardado: 0,
            procurando_rascunho: false,
            produtos: Vec::new(),
            estudios: Vec::new(),
            galerias: None,
            presets: Vec::new(),
            presets_chegaram: false,
            titulo,
            email,
            whatsapp,
            detalhe,
            escolha_do_produto,
            escolha_do_estudio,
            tentou: false,
            fase: None,
            erro: None,
            confirmacao: None,
            busca: None,
            cadastro: None,
            menu_da_origem: None,
            fotos: Vec::new(),
            orfas: Vec::new(),
            importacao: None,
            fila_de_levas: std::collections::VecDeque::new(),
            falhas_da_copia: None,
            freios: Freios::default(),
            escolhendo: false,
            receita: Receita::default(),
            receita_aplicada: HashMap::new(),
            arrastando: false,
            aviso: None,
            rolagem: ScrollHandle::new(),
            foco: cx.focus_handle(),
            foco_do_preset: 0,
            teclado_no_preset: false,
            miniaturas: HashMap::new(),
            pedir_colheita_das_reveladas: false,
            amostras: Amostras::default(),
            focar: None,
            recados: channel(),
            escolhas: channel(),
            andamentos: channel(),
            releituras: channel(),
            catalogo: channel(),
            origens: channel(),
            esperando_catalogo: None,
            esperando_descarte: false,
            esperando_releitura: false,
            carregando: false,
            colhendo: false,
            _colheita: None,
            _assinaturas: assinaturas,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// A tela vai aparecer: carrega as listas e procura um rascunho.
    pub fn abrir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.erro = None;
        self.confirmacao = None;
        self.busca = None;
        self.menu_da_origem = None;
        self.tentou = false;
        if self.fase.is_none() {
            self.carregar_rascunho(window, cx);
        }
        if let Some(sessao) = self.sessao.clone() {
            self.carregando = true;
            self.galerias = None;
            let canal = self.recados.0.clone();
            self.portas
                .publicador
                .galerias(sessao.clone(), canal.clone());
            self.portas
                .publicador
                .estudios(sessao.clone(), canal.clone());
            self.portas
                .publicador
                .produtos(sessao.clone(), canal.clone());
            self.portas.publicador.pedir_json(
                sessao,
                PedidoJson::ler("nova-presets", "/revelacao/presets"),
                canal,
            );
        }
        self.reler_fotos();
        self.acompanhar(window, cx);
        window.focus(&self.foco);
        cx.notify();
    }

    /// Com rascunho guardado que valha perguntar, a tela mostra o cartão de
    /// retomada; sem, começa um novo.
    fn carregar_rascunho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match estado::ler_rascunho(&self.caminho) {
            Some(guardado) if guardado.id_provisorio == self.rascunho.id_provisorio => {
                // Já é o que está na tela (voltar e entrar de novo).
                let _ = (window, cx);
            }
            Some(guardado) => {
                self.procurando_rascunho = true;
                self.guardado = Some(guardado);
            }
            None => self.comecar_novo(window, cx),
        }
    }

    /// Um rascunho novo, com as sugestões desta máquina e do servidor.
    fn comecar_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rascunho = Rascunho::novo(
            &uuid::Uuid::new_v4().to_string(),
            Formulario::default(),
            agora(),
        );
        self.rascunho.etapa = 2;
        self.receita_aplicada.clear();
        self.receita = Receita::default();
        self.falhas_da_copia = None;
        self.tentou = false;
        self.erro = None;
        self.sugerir();
        self.espelhar_nos_campos(window, cx);
        self.reler_fotos();
    }

    /// Preço da última sessão; estúdio, preset e corte lembrados nesta máquina.
    fn sugerir(&mut self) {
        let lembrado = lembranca::ler();
        let f = &mut self.rascunho.formulario;
        if f.produto_id.is_empty() {
            if let Some(ultima) = self.galerias.as_ref().and_then(|g| g.first()) {
                f.produto_id = ultima.produto_id.clone();
            }
        }
        if f.estudio_id.is_empty() {
            if let Some(estudio) = lembrado
                .estudio_id
                .filter(|id| self.estudios.is_empty() || self.estudios.iter().any(|e| &e.id == id))
            {
                f.estudio_id = estudio;
            }
        }
        if f.preset_id.is_none() {
            f.preset_id = lembrado
                .preset_id
                .filter(|id| !self.presets_chegaram || self.presets.iter().any(|p| &p.id == id));
        }
        if f.proporcao.is_none() {
            f.proporcao = lembrado
                .proporcao
                .filter(|p| estado::PROPORCOES_PADRAO.contains(&p.as_str()));
        }
    }

    /// "Retomar" no cartão.
    pub fn retomar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(guardado) = self.guardado.take() else {
            return;
        };
        self.procurando_rascunho = false;
        self.rascunho = guardado;
        if self.rascunho.etapa < 2 {
            self.rascunho.etapa = 2;
        }
        self.espelhar_nos_campos(window, cx);
        self.reler_fotos();
        self.guardar();
        self.acompanhar(window, cx);
        cx.notify();
    }

    /// Leva o formulário para os campos (ao retomar e ao associar).
    fn espelhar_nos_campos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = self.rascunho.formulario.clone();
        for (entidade, valor) in [
            (&self.titulo, f.titulo.clone()),
            (&self.email, f.email.clone()),
            (&self.whatsapp, f.whatsapp.clone()),
            (&self.detalhe, f.como_conheceu_detalhe.clone()),
        ] {
            entidade.update(cx, |estado, cx| {
                if estado.value() != valor.as_str() {
                    estado.set_value(valor, window, cx);
                }
            });
        }
        self.atualizar_escolhas(window, cx);
    }

    /// Refaz as listas dos `Select` e marca o escolhido.
    fn atualizar_escolhas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = &self.rascunho.formulario;
        let produtos = opcoes_de_produto(&self.produtos, &f.produto_id);
        let estudios = opcoes_de_estudio(&self.estudios, &f.estudio_id);
        let (produto, estudio) = (f.produto_id.clone(), f.estudio_id.clone());
        self.escolha_do_produto.update(cx, |estado, cx| {
            estado.set_items(SearchableVec::new(produtos), window, cx);
            if produto.is_empty() {
                estado.set_selected_index(None, window, cx);
            } else {
                estado.set_selected_value(&produto, window, cx);
            }
        });
        self.escolha_do_estudio.update(cx, |estado, cx| {
            estado.set_items(SearchableVec::new(estudios), window, cx);
            if estudio.is_empty() {
                estado.set_selected_index(None, window, cx);
            } else {
                estado.set_selected_value(&estudio, window, cx);
            }
        });
    }

    /// Grava o rascunho — a cada mudança, como o site.
    pub(super) fn guardar(&mut self) {
        // 🚨 Com o cartão de retomada na tela, o arquivo ainda é o do rascunho
        // guardado: gravar agora o apagaria antes de o operador decidir.
        if self.guardado.is_some() {
            return;
        }
        self.rascunho.atualizado_em = agora();
        estado::guardar_rascunho(&self.caminho, &self.rascunho);
    }

    // ── Navegação ────────────────────────────────────────────────────────

    pub fn etapa(&self) -> usize {
        self.rascunho.etapa
    }

    pub(super) fn formulario(&self) -> &Formulario {
        &self.rascunho.formulario
    }

    /// Quantas fotos o rascunho tem — as gravadas e as que ainda estão
    /// copiando.
    pub fn quantas_fotos(&self) -> usize {
        let em_curso = self
            .importacao
            .map(|i| i.total.saturating_sub(i.prontas()))
            .unwrap_or(0);
        self.fotos.len() + self.importacao.map(|i| i.feitas).unwrap_or(0) + em_curso
    }

    pub(super) fn estado_da(&self, etapa: usize) -> EstadoDaEtapa {
        estado::estado_da_etapa(etapa, self.formulario(), self.quantas_fotos(), true)
    }

    pub fn ir(&mut self, etapa: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.guardado.is_some() {
            return;
        }
        let etapa = etapa.clamp(1, estado::TOTAL_DE_ETAPAS);
        if etapa == self.rascunho.etapa {
            return;
        }
        self.rascunho.etapa = etapa;
        self.menu_da_origem = None;
        self.busca = None;
        self.guardar();
        self.rolagem
            .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn voltar_etapa(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let anterior = self.rascunho.etapa.saturating_sub(1).max(1);
        self.ir(anterior, window, cx);
    }

    /// "Avançar". Só a etapa 3 segura, quando falta preço ou estúdio.
    pub fn avancar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rascunho.etapa == 3 {
            let f = self.formulario();
            let falta = [
                (f.produto_id.trim().is_empty(), estado::FALTA_PRECO),
                (f.estudio_id.trim().is_empty(), estado::FALTA_ESTUDIO),
            ]
            .into_iter()
            .find(|(falta, _)| *falta)
            .map(|(_, m)| m);
            if let Some(mensagem) = falta {
                self.tentou = true;
                self.avisar(mensagem, true);
                self.levar_ao_campo(mensagem, window, cx);
                cx.notify();
                return;
            }
        }
        let proxima = self.rascunho.etapa + 1;
        self.ir(proxima, window, cx);
    }

    /// Enter: Avançar, ou Criar na etapa 7.
    pub fn acao_principal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirmacao.is_some()
            || self.busca.is_some()
            || self.guardado.is_some()
            || self.cadastro.is_some()
        {
            return;
        }
        if self.rascunho.etapa >= estado::TOTAL_DE_ETAPAS {
            self.criar(window, cx);
        } else {
            self.avancar(window, cx);
        }
    }

    fn levar_ao_campo(&mut self, mensagem: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(campo) = estado::campo_da_pendencia(mensagem) else {
            return;
        };
        let etapa = match campo {
            Campo::Parceiro => 6,
            _ => 3,
        };
        self.ir(etapa, window, cx);
        self.focar = Some(campo);
        cx.notify();
    }

    /// Chamado pelo desenho: foca o campo pedido, depois de ele existir.
    pub(super) fn focar_pendente(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(campo) = self.focar.take() else {
            return;
        };
        match campo {
            Campo::Titulo => self.titulo.update(cx, |c, cx| c.focus(window, cx)),
            Campo::Email => self.email.update(cx, |c, cx| c.focus(window, cx)),
            Campo::Produto => {
                let foco = self.escolha_do_produto.focus_handle(cx);
                window.focus(&foco);
            }
            Campo::Estudio => {
                let foco = self.escolha_do_estudio.focus_handle(cx);
                window.focus(&foco);
            }
            Campo::Parceiro => {}
        }
    }

    pub(super) fn avisar(&mut self, texto: impl Into<String>, erro: bool) {
        self.aviso = Some(Aviso {
            texto: texto.into(),
            erro,
            ate: Instant::now() + DURACAO_DO_AVISO,
        });
    }

    fn avisar_longo(&mut self, texto: impl Into<String>) {
        self.aviso = Some(Aviso {
            texto: texto.into(),
            erro: false,
            ate: Instant::now() + DURACAO_DO_AVISO_LONGO,
        });
    }

    pub fn voltar_as_sessoes(&mut self, cx: &mut Context<Self>) {
        self.guardar();
        cx.emit(PedidoDaNova::Voltar);
    }

    // ── Etapa 2: fotos e receita ─────────────────────────────────────────

    /// "Escolher fotos" / "Adicionar mais fotos".
    pub fn escolher_fotos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.escolhendo = true;
        self.portas
            .seletor_de_fotos
            .escolher(self.escolhas.0.clone());
        self.acompanhar(window, cx);
    }

    /// "Do cartão ou pasta…": abre o menu e procura os cartões.
    pub fn abrir_menu_da_origem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_da_origem.take().is_some() {
            cx.notify();
            return;
        }
        self.menu_da_origem = Some(MenuDaOrigem {
            procurando: true,
            cartoes: Vec::new(),
        });
        self.portas.explorador.origens(self.origens.0.clone());
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn ler_cartao(&mut self, caminho: String, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_da_origem = None;
        let nome = std::path::Path::new(&caminho)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| caminho.clone());
        self.avisar(format!("Lendo de {nome}…"), false);
        self.portas
            .explorador
            .varrer(caminho, true, self.origens.0.clone());
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn escolher_pasta(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_da_origem = None;
        self.portas
            .seletor_de_pasta
            .escolher(self.origens.0.clone(), cx);
        self.escolhendo = true;
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn destacar(&mut self, arrastando: bool, cx: &mut Context<Self>) {
        if self.arrastando != arrastando {
            self.arrastando = arrastando;
            cx.notify();
        }
    }

    pub fn importando(&self) -> bool {
        self.importacao.is_some_and(|i| !i.terminou()) || !self.fila_de_levas.is_empty()
    }

    /// Copia as fotos para o catálogo, sob o id do rascunho. **Nada sobe.**
    pub fn importar_arquivos(
        &mut self,
        caminhos: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.arrastando = false;
        if caminhos.is_empty() {
            return;
        }
        if self.importando() {
            let quantas = caminhos.len();
            self.fila_de_levas.push_back(caminhos);
            self.avisar(
                format!(
                    "{} na fila: entram assim que a cópia atual terminar.",
                    estado::plural(quantas, "foto", "fotos")
                ),
                false,
            );
            cx.notify();
            return;
        }
        self.despachar_leva(caminhos, window, cx);
    }

    /// Manda uma leva ao importador. Quem decide se é agora ou depois da fila é
    /// `importar_arquivos`.
    fn despachar_leva(
        &mut self,
        caminhos: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.falhas_da_copia = None;
        self.importacao = Some(Importacao {
            total: caminhos.len(),
            feitas: 0,
            falhas: 0,
        });
        self.freios = Freios::default();
        let id = self.rascunho.id_provisorio.clone();
        let pasta = pasta_do_ensaio("", &id.replace(':', "-"));
        self.portas.importador.importar(
            caminhos,
            ImportOptions {
                sessao_id: Some(id),
                // 🚨 Copiar, nunca catalogar onde está: o cartão sai da máquina.
                mode: ImportMode::Copy,
                destination: Some(pasta.to_string_lossy().into_owned()),
                organization: OrganizationStrategy::IntoOneFolder,
                rename_pattern: RenamePattern::Uuid,
                ..Default::default()
            },
            self.freios.clone(),
            self.andamentos.0.clone(),
        );
        self.acompanhar(window, cx);
        cx.notify();
    }

    fn reler_fotos(&mut self) {
        self.esperando_releitura = true;
        self.portas.acervo.recarregar(self.releituras.0.clone());
    }

    /// O preset padrão — o cartão clicado (Espaço ou Enter no focado).
    pub fn escolher_preset(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if self.rascunho.formulario.preset_id == id {
            return;
        }
        self.rascunho.formulario.preset_id = id;
        self.guardar();
        self.esquecer_a_fila_da_receita();
        self.aplicar_receita();
        cx.notify();
    }

    /// A proporção do corte padrão. Clicar na marcada não desmarca.
    pub fn escolher_proporcao(&mut self, proporcao: Option<String>, cx: &mut Context<Self>) {
        if self.rascunho.formulario.proporcao == proporcao {
            return;
        }
        self.rascunho.formulario.proporcao = proporcao;
        self.guardar();
        self.esquecer_a_fila_da_receita();
        self.aplicar_receita();
        cx.notify();
    }

    pub(super) fn preset_escolhido(&self) -> Option<&PresetDaSessao> {
        let id = self.formulario().preset_id.as_deref()?;
        self.presets.iter().find(|p| p.id == id)
    }

    /// Os cartões do seletor na ordem da tela: "Nenhum", depois a lista.
    pub(super) fn cartoes_de_preset(&self) -> Vec<Option<String>> {
        let mut cartoes = vec![None];
        cartoes.extend(self.presets.iter().map(|p| Some(p.id.clone())));
        cartoes
    }

    /// As setas andam o foco pelos cartões; Espaço e Enter marcam.
    pub fn mover_foco_do_preset(&mut self, passo: isize, cx: &mut Context<Self>) {
        let total = self.cartoes_de_preset().len() as isize;
        if total == 0 {
            return;
        }
        self.foco_do_preset = (self.foco_do_preset as isize + passo).clamp(0, total - 1) as usize;
        self.teclado_no_preset = true;
        cx.notify();
    }

    pub fn marcar_preset_em_foco(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.cartoes_de_preset().get(self.foco_do_preset).cloned() {
            self.escolher_preset(id, cx);
        }
    }

    /// Aplica o preset e o corte padrão às fotos do rascunho que ainda não os
    /// têm. **Parte sempre do neutro**: no rascunho ninguém revelou nada.
    fn aplicar_receita(&mut self) {
        let f = &self.rascunho.formulario;
        let preset = self.preset_escolhido().map(|p| p.preset.clone());
        if f.preset_id.is_some() && preset.is_none() && !self.presets_chegaram {
            // O preset ainda não chegou do servidor: aplica quando chegar.
            return;
        }
        let proporcao = f.proporcao.clone();
        let chave = format!("{:?}|{:?}", f.preset_id, proporcao);
        let ajustes = receita::ajustes_da_receita(preset.as_ref());
        let sem_efeito = f.preset_id.is_none() && proporcao.is_none();
        let mut pediu = false;
        for foto in &self.fotos {
            let ja = self.receita_aplicada.get(&foto.id) == Some(&chave);
            if !ja {
                // Sem efeito também grava: é o que desfaz a receita anterior.
                if !(sem_efeito && !self.receita_aplicada.contains_key(&foto.id)) {
                    // Os PARÂMETROS na hora, com o que se sabe agora — e o
                    // serviço os regrava com o corte medido na imagem.
                    //
                    // 🚨 **O corte daqui costuma sair vazio, e é de propósito
                    // que ele não é o último a falar**: `width`/`height` vêm do
                    // EXIF (`PixelXDimension`), que JPEG, NEF e CR2 de verdade
                    // não trazem — medido em quatro arquivos, nenhum tinha.
                    // Gravar aqui garante que a foto nunca fica **sem** receita
                    // (C8); quem põe o corte certo é quem abre a imagem.
                    let corte = receita::corte_centralizado(
                        proporcao.as_deref(),
                        foto.width.unwrap_or(0),
                        foto.height.unwrap_or(0),
                    );
                    self.portas.gravador.gravar(foto.id.clone(), ajustes, corte);
                    // 🚨 **Gravar os PARÂMETROS não revela nada** — e era só
                    // isto que acontecia aqui. A foto ia para a sessão com o
                    // preset escolhido e a imagem do bruto, e a barra "Preset
                    // padrão" enchia na hora porque contava este laço em vez do
                    // trabalho (achado do dono, 17/set/2026). Agora o serviço
                    // revela em segundo plano e a barra segue **ele**.
                    //
                    // 🔑 **E é ele quem grava o corte de verdade**, medido na
                    // imagem que abriu — o site faz igual: quem grava a receita
                    // é o trabalhador que revela (`exportacao/worker.ts`).
                    self.portas
                        .receita_padrao
                        .pedir(foto.id.clone(), ajustes, proporcao.clone());
                    pediu = true;
                }
                self.receita_aplicada.insert(foto.id.clone(), chave.clone());
            }
        }
        // O progresso é do serviço: quem conta é quem termina.
        let andamento = self.portas.receita_padrao.progresso();
        self.receita.total = andamento.total;
        self.receita.prontas = andamento.prontas;
        if pediu {
            self.pedir_colheita_das_reveladas = true;
        }
    }

    /// A receita mudou de cara: o que está na fila não vale mais.
    ///
    /// Chamado de `escolher_preset` e `escolher_proporcao`, antes de aplicar —
    /// é o `recomecar` do serviço visto daqui.
    fn esquecer_a_fila_da_receita(&mut self) {
        self.portas.receita_padrao.recomecar();
    }

    pub fn apagar_orfas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmacao = None;
        for sessao in self.orfas.clone() {
            self.portas
                .acervo
                .apagar_da_sessao(sessao, self.catalogo.0.clone());
        }
        self.esperando_descarte = true;
        self.acompanhar(window, cx);
        cx.notify();
    }

    // ── Etapa 3: cliente e preço ─────────────────────────────────────────

    /// 🧪 Escolhe o preço e o estúdio sem o `Select` (os testes não o abrem).
    pub fn escolher_produto(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.rascunho.formulario.produto_id = id.to_string();
        self.guardar();
        self.atualizar_escolhas(window, cx);
        cx.notify();
    }

    pub fn escolher_estudio(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.rascunho.formulario.estudio_id = id.to_string();
        self.guardar();
        self.atualizar_escolhas(window, cx);
        cx.notify();
    }

    /// 🧪 Digita nos campos, como o operador.
    #[cfg(test)]
    pub(crate) fn digitar(
        &mut self,
        titulo: &str,
        email: &str,
        whatsapp: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rascunho.formulario.titulo = titulo.into();
        self.rascunho.formulario.email = email.into();
        self.rascunho.formulario.whatsapp = whatsapp.into();
        self.espelhar_nos_campos(window, cx);
        self.guardar();
    }

    // ── Etapas 4 a 7: associações ───────────────────────────────────────

    pub fn abrir_busca(&mut self, tipo: TipoDeBusca, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = match tipo {
            TipoDeBusca::Agendamento => "Nome, WhatsApp, e-mail ou estúdio…",
            TipoDeBusca::Voucher => "Número, nome, WhatsApp, e-mail ou parceiro…",
            TipoDeBusca::Compra => "Nome, e-mail, WhatsApp ou número do pedido…",
            TipoDeBusca::Parceiro => "Nome do hotel, pousada, guia…",
        };
        let campo = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let assinatura = cx.subscribe_in(
            &campo,
            window,
            |tela, _, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => {
                    if let Some(busca) = tela.busca.as_mut() {
                        busca.agendada = Some(Instant::now() + ESPERA_DA_BUSCA);
                    }
                    tela.acompanhar(window, cx);
                }
                InputEvent::PressEnter { .. } => {
                    let primeiro = tela.busca.as_ref().and_then(|b| b.itens.first().cloned());
                    if let Some(item) = primeiro {
                        tela.escolher_da_busca(item, window, cx);
                    }
                }
                _ => {}
            },
        );
        campo.update(cx, |c, cx| c.focus(window, cx));
        self.busca = Some(Busca {
            tipo,
            campo,
            carregando: false,
            erro: None,
            itens: Vec::new(),
            com_texto: false,
            enviados: 0,
            recebidos: 0,
            agendada: None,
            _assinatura: assinatura,
        });
        self.buscar(cx);
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn fechar_busca(&mut self, cx: &mut Context<Self>) {
        self.busca = None;
        cx.notify();
    }

    fn buscar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(busca) = self.busca.as_mut() else {
            return;
        };
        let texto = busca.campo.read(cx).value().trim().to_string();
        busca.agendada = None;
        busca.carregando = true;
        busca.erro = None;
        busca.com_texto = !texto.is_empty();
        busca.enviados += 1;
        let limite = Some(assoc::LIMITE_DA_BUSCA.to_string());
        let com_texto = (!texto.is_empty()).then_some(texto);
        let caminho = match busca.tipo {
            TipoDeBusca::Agendamento => {
                // Sem texto, a janela em volta de hoje; com texto, qualquer data.
                let janela = com_texto.is_none().then(|| {
                    let hoje = chrono::Utc::now();
                    (
                        (hoje - chrono::Duration::days(assoc::DIAS_ATRAS))
                            .format("%Y-%m-%d")
                            .to_string(),
                        (hoje + chrono::Duration::days(assoc::DIAS_A_FRENTE))
                            .format("%Y-%m-%d")
                            .to_string(),
                    )
                });
                format!(
                    "/pos-venda/busca/agendamentos{}",
                    assoc::consulta(&[
                        ("busca", com_texto),
                        ("de", janela.as_ref().map(|j| j.0.clone())),
                        ("ate", janela.map(|j| j.1)),
                        ("limite", limite),
                    ])
                )
            }
            TipoDeBusca::Voucher => format!(
                "/pos-venda/busca/vouchers{}",
                assoc::consulta(&[("busca", com_texto), ("limite", limite)])
            ),
            TipoDeBusca::Compra => format!(
                "/pos-venda/busca/compras-antecipadas{}",
                assoc::consulta(&[("busca", com_texto), ("limite", limite)])
            ),
            TipoDeBusca::Parceiro => format!(
                "/pos-venda/parceiros{}",
                assoc::consulta(&[("busca", com_texto)])
            ),
        };
        self.portas.publicador.pedir_json(
            sessao,
            PedidoJson::ler(busca.tipo.rotulo(), caminho),
            self.recados.0.clone(),
        );
    }

    fn receber_busca(&mut self, tipo: TipoDeBusca, resultado: Result<serde_json::Value, String>) {
        let Some(busca) = self.busca.as_mut().filter(|b| b.tipo == tipo) else {
            return;
        };
        busca.recebidos += 1;
        // Só a resposta do último pedido vale: as anteriores são de um texto
        // que o operador já mudou.
        if busca.recebidos < busca.enviados {
            return;
        }
        busca.carregando = false;
        match resultado {
            Ok(valor) => {
                busca.itens = match tipo {
                    TipoDeBusca::Agendamento => {
                        let mut lista = assoc::agendamentos_da_api(&valor);
                        assoc::ordenar_agendamentos(&mut lista, agora());
                        lista.into_iter().map(ItemDaBusca::Agendamento).collect()
                    }
                    TipoDeBusca::Voucher => {
                        let mut lista = assoc::vouchers_da_api(&valor);
                        assoc::ordenar_vouchers(&mut lista);
                        lista.into_iter().map(ItemDaBusca::Voucher).collect()
                    }
                    TipoDeBusca::Compra => {
                        let mut lista = assoc::compras_da_api(&valor);
                        assoc::ordenar_compras(&mut lista);
                        lista.into_iter().map(ItemDaBusca::Compra).collect()
                    }
                    TipoDeBusca::Parceiro => assoc::parceiros_da_api(&valor)
                        .into_iter()
                        .map(ItemDaBusca::Parceiro)
                        .collect(),
                };
            }
            Err(erro) => {
                busca.itens.clear();
                busca.erro = Some(mensagem_da_busca(&erro, tipo.o_que()));
            }
        }
    }

    pub fn escolher_da_busca(
        &mut self,
        item: ItemDaBusca,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busca = None;
        self.associar(item, window, cx);
    }

    fn associar(&mut self, item: ItemDaBusca, window: &mut Window, cx: &mut Context<Self>) {
        let f = &mut self.rascunho.formulario;
        match item {
            ItemDaBusca::Agendamento(a) => estado::associar_agendamento(f, Some(a)),
            ItemDaBusca::Voucher(v) => estado::associar_voucher(f, Some(v)),
            ItemDaBusca::Compra(c) => estado::associar_compra(f, Some(c)),
            ItemDaBusca::Parceiro(p) => {
                f.parceiro = Some(p);
                self.cadastro = None;
            }
        }
        self.guardar();
        self.espelhar_nos_campos(window, cx);
        cx.notify();
    }

    pub fn remover(&mut self, tipo: TipoDeBusca, cx: &mut Context<Self>) {
        let f = &mut self.rascunho.formulario;
        match tipo {
            TipoDeBusca::Agendamento => estado::associar_agendamento(f, None),
            TipoDeBusca::Voucher => estado::associar_voucher(f, None),
            TipoDeBusca::Compra => estado::associar_compra(f, None),
            TipoDeBusca::Parceiro => f.parceiro = None,
        }
        self.guardar();
        cx.notify();
    }

    /// Clicar de novo na marcada desmarca.
    pub fn escolher_como_conheceu(
        &mut self,
        valor: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let novo =
            (self.formulario().como_conheceu.as_deref() != Some(valor)).then(|| valor.to_string());
        estado::escolher_como_conheceu(&mut self.rascunho.formulario, novo);
        if self.formulario().como_conheceu.as_deref() != Some("parceiro") {
            self.cadastro = None;
        }
        self.guardar();
        self.espelhar_nos_campos(window, cx);
        cx.notify();
    }

    pub fn abrir_cadastro(
        &mut self,
        nome: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let campo = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let nome_campo = campo("", window, cx);
        if let Some(nome) = nome {
            nome_campo.update(cx, |c, cx| c.set_value(nome, window, cx));
        }
        let opcoes: Vec<Opcao> = assoc::TIPOS_DE_PARCEIRO
            .iter()
            .map(|(v, r)| Opcao {
                id: v.to_string(),
                titulo: (*r).into(),
            })
            .collect();
        let tipo = cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
        let assinatura = cx.subscribe_in(
            &tipo,
            window,
            |tela, _, evento: &SelectEvent<SearchableVec<Opcao>>, _, cx| {
                let SelectEvent::Confirm(valor) = evento;
                if let Some(cadastro) = tela.cadastro.as_mut() {
                    cadastro.tipo_escolhido = valor.clone();
                    cx.notify();
                }
            },
        );
        nome_campo.update(cx, |c, cx| c.focus(window, cx));
        self.cadastro = Some(Cadastro {
            nome: nome_campo,
            whatsapp: campo("", window, cx),
            email: campo("", window, cx),
            tipo,
            tipo_escolhido: None,
            enviando: false,
            erro: None,
            existente: None,
            _assinatura: assinatura,
        });
        cx.notify();
    }

    pub fn fechar_cadastro(&mut self, cx: &mut Context<Self>) {
        self.cadastro = None;
        cx.notify();
    }

    /// 🧪 Escolhe o tipo sem abrir o `Select`.
    pub fn escolher_tipo_do_parceiro(&mut self, tipo: &str, cx: &mut Context<Self>) {
        if let Some(cadastro) = self.cadastro.as_mut() {
            cadastro.tipo_escolhido = Some(tipo.to_string());
            cx.notify();
        }
    }

    /// "Cadastrar e usar".
    pub fn cadastrar_parceiro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(cadastro) = self.cadastro.as_mut() else {
            return;
        };
        if cadastro.enviando {
            return;
        }
        let nome = cadastro.nome.read(cx).value().trim().to_string();
        let ou = |e: &Entity<InputState>| {
            let v = e.read(cx).value().trim().to_string();
            (!v.is_empty()).then_some(v)
        };
        let (whatsapp, email) = (ou(&cadastro.whatsapp), ou(&cadastro.email));
        cadastro.existente = None;
        if nome.is_empty() {
            cadastro.erro = Some("Informe o nome do parceiro.".into());
            cx.notify();
            return;
        }
        let Some(tipo) = cadastro.tipo_escolhido.clone() else {
            cadastro.erro = Some("Escolha o tipo do parceiro.".into());
            cx.notify();
            return;
        };
        cadastro.erro = None;
        cadastro.enviando = true;
        self.portas.publicador.pedir_json(
            sessao,
            PedidoJson::gravar(
                "nova-parceiro-criado",
                "POST",
                "/pos-venda/parceiros",
                serde_json::json!({
                    "nome": nome,
                    "tipo": tipo,
                    "whatsapp": whatsapp,
                    "email": email,
                }),
            ),
            self.recados.0.clone(),
        );
        self.acompanhar(window, cx);
        cx.notify();
    }

    /// O `409`: o existente é procurado pelo nome, para "usar este".
    fn receber_cadastro(
        &mut self,
        resultado: Result<serde_json::Value, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(cadastro) = self.cadastro.as_mut() else {
            return;
        };
        cadastro.enviando = false;
        match resultado {
            Ok(valor) => match assoc::parceiro_da_api(&valor) {
                Some(parceiro) => {
                    self.rascunho.formulario.parceiro = Some(parceiro);
                    self.cadastro = None;
                    self.busca = None;
                    self.guardar();
                }
                None => {
                    cadastro.erro =
                        Some("Não foi possível cadastrar o parceiro. Tente de novo.".into());
                }
            },
            Err(erro) => {
                let (status, frase) = estado::ler_erro_do_site(&erro);
                cadastro.erro = Some(match status {
                    Some(409) => {
                        let nome = cadastro.nome.read(cx).value().trim().to_string();
                        if let Some(sessao) = self.sessao.clone() {
                            self.portas.publicador.pedir_json(
                                sessao,
                                PedidoJson::ler(
                                    "nova-parceiro-existente",
                                    format!(
                                        "/pos-venda/parceiros{}",
                                        assoc::consulta(&[
                                            ("busca", Some(nome)),
                                            ("incluir_inativos", Some("true".into())),
                                        ])
                                    ),
                                ),
                                self.recados.0.clone(),
                            );
                        }
                        "Já existe um parceiro com este nome.".into()
                    }
                    Some(403) => "Sua conta não pode cadastrar parceiros.".into(),
                    Some(400) => frase,
                    _ => "Não foi possível cadastrar o parceiro. Tente de novo.".into(),
                });
            }
        }
    }

    pub fn usar_existente(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(parceiro) = self.cadastro.as_ref().and_then(|c| c.existente.clone()) else {
            return;
        };
        self.busca = None;
        self.associar(ItemDaBusca::Parceiro(parceiro), window, cx);
    }

    // ── Criar ────────────────────────────────────────────────────────────

    pub(super) fn rotulo_de_criar(&self) -> &'static str {
        if self.rascunho.criada_id.is_some() {
            "Terminar e abrir a sessão"
        } else {
            "Criar sessão"
        }
    }

    /// "Criar sessão" (rodapé na etapa 7 ou cabeçalho).
    pub fn criar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fase.is_some() {
            return;
        }
        self.tentou = true;
        self.erro = None;
        if let Some((_, mensagem)) = estado::pendencias_para_criar(self.formulario())
            .first()
            .copied()
        {
            self.avisar(mensagem, true);
            self.levar_ao_campo(mensagem, window, cx);
            cx.notify();
            return;
        }
        self.fase = Some(Fase::Copiando);
        self.seguir_criacao(window, cx);
        self.acompanhar(window, cx);
        cx.notify();
    }

    /// Anda a criação um passo, quando o anterior terminou.
    fn seguir_criacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.fase {
            Some(Fase::Copiando) if !self.importando() => {
                if self.rascunho.criada_id.is_some() {
                    self.mover_fotos();
                } else if let Some(sessao) = self.sessao.clone() {
                    self.fase = Some(Fase::Criando);
                    self.portas.publicador.pedir_json(
                        sessao,
                        PedidoJson::gravar(
                            "nova-criada",
                            "POST",
                            "/pos-venda/galerias",
                            estado::para_envio(self.formulario()),
                        ),
                        self.recados.0.clone(),
                    );
                } else {
                    self.fase = None;
                    self.erro = Some(("Entre na conta para criar a sessão.".into(), None));
                }
            }
            _ => {}
        }
        let _ = (window, cx);
    }

    fn mover_fotos(&mut self) {
        let Some(para) = self.rascunho.criada_id.clone() else {
            return;
        };
        self.fase = Some(Fase::Movendo);
        self.esperando_catalogo = Some(Fase::Movendo);
        self.portas.acervo.trocar_sessao(
            self.rascunho.id_provisorio.clone(),
            para,
            self.catalogo.0.clone(),
        );
    }

    fn receber_criacao(&mut self, resultado: Result<serde_json::Value, String>) {
        match resultado {
            Ok(valor) => {
                let Some(id) = valor.get("id").and_then(|v| v.as_str()).map(str::to_string) else {
                    self.fase = None;
                    self.erro = Some(("Não foi possível criar a sessão.".into(), None));
                    return;
                };
                // 🚨 Gravado **na hora**: sem isto, uma falha na troca criaria
                // uma segunda galeria no próximo clique.
                self.rascunho.criada_id = Some(id);
                self.guardar();
                let f = self.formulario();
                let faltou_preset = f.preset_id.is_some()
                    && valor.get("preset_padrao_id").is_some_and(|v| v.is_null());
                let faltou_corte = f.proporcao.is_some()
                    && valor.get("proporcao_padrao").is_some_and(|v| v.is_null());
                let o_que = match (faltou_preset, faltou_corte) {
                    (true, true) => Some("o preset e o corte padrão"),
                    (true, false) => Some("o preset padrão"),
                    (false, true) => Some("o corte padrão"),
                    _ => None,
                };
                if let Some(o_que) = o_que {
                    self.avisar_longo(format!(
                        "O servidor criou a sessão mas não guardou {o_que}. A receita continua \
                         aplicada às fotos desta máquina; em outra máquina, escolha de novo na \
                         gaveta \"Atendimento\"."
                    ));
                }
                self.mover_fotos();
            }
            Err(erro) => {
                self.fase = None;
                let (status, mensagem) = estado::mensagem_da_criacao(&erro);
                let etapa =
                    estado::etapa_do_erro(status, &mensagem).filter(|e| *e != self.rascunho.etapa);
                self.erro = Some((mensagem, etapa));
            }
        }
    }

    fn receber_troca(&mut self, resultado: Result<usize, String>, cx: &mut Context<Self>) {
        self.esperando_catalogo = None;
        match resultado {
            Ok(_) => {
                let id = self.rascunho.criada_id.clone().unwrap_or_default();
                let fotos = self.fotos.len();
                estado::apagar_rascunho(&self.caminho);
                lembranca::gravar(&lembranca::Lembranca {
                    estudio_id: Some(self.formulario().estudio_id.clone())
                        .filter(|e| !e.is_empty()),
                    preset_id: self.formulario().preset_id.clone(),
                    proporcao: self.formulario().proporcao.clone(),
                });
                let frase = if fotos > 0 {
                    format!(
                        "Sessão criada com {}.",
                        estado::plural(fotos, "foto", "fotos")
                    )
                } else {
                    "Sessão criada.".to_string()
                };
                if self.aviso.is_none() {
                    self.avisar(frase, false);
                }
                self.fase = None;
                self.fotos.clear();
                self.receita_aplicada.clear();
                self.receita = Receita::default();
                // O próximo "Nova sessão" começa limpo.
                self.rascunho = Rascunho::novo(
                    &uuid::Uuid::new_v4().to_string(),
                    Formulario::default(),
                    agora(),
                );
                self.rascunho.etapa = 2;
                cx.emit(PedidoDaNova::CatalogoMudou);
                cx.emit(PedidoDaNova::Criada(id));
            }
            Err(msg) => {
                self.fase = None;
                self.erro = Some((
                    format!(
                        "A sessão foi criada, mas as fotos ainda não passaram para ela ({msg}). \
                         Nada se perdeu: clique em \"Criar sessão\" de novo para tentar mover."
                    ),
                    Some(2).filter(|e| *e != self.rascunho.etapa),
                ));
            }
        }
    }

    // ── Descartar ────────────────────────────────────────────────────────

    pub fn pedir_confirmacao(&mut self, qual: Confirmacao, cx: &mut Context<Self>) {
        self.confirmacao = Some(qual);
        cx.notify();
    }

    pub fn cancelar_confirmacao(&mut self, cx: &mut Context<Self>) {
        self.confirmacao = None;
        cx.notify();
    }

    pub(super) fn confirmar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.confirmacao.take() {
            Some(Confirmacao::Descartar) => self.descartar(window, cx),
            Some(Confirmacao::DescartarGuardado) => {
                if let Some(guardado) = self.guardado.take() {
                    self.procurando_rascunho = false;
                    self.portas
                        .acervo
                        .apagar_da_sessao(guardado.id_provisorio, self.catalogo.0.clone());
                    self.esperando_descarte = true;
                    estado::apagar_rascunho(&self.caminho);
                    self.comecar_novo(window, cx);
                    self.acompanhar(window, cx);
                }
            }
            Some(Confirmacao::ApagarOrfas) => self.apagar_orfas(window, cx),
            None => {}
        }
        cx.notify();
    }

    /// Apaga as fotos do rascunho e o preenchimento; um novo começa na hora.
    /// A sessão já criada no site (se houver) continua lá.
    fn descartar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fase.is_some() {
            return;
        }
        if self.importando() {
            self.freios.cancelar();
        }
        self.portas
            .acervo
            .apagar_da_sessao(self.rascunho.id_provisorio.clone(), self.catalogo.0.clone());
        self.esperando_descarte = true;
        estado::apagar_rascunho(&self.caminho);
        self.fotos.clear();
        self.importacao = None;
        self.comecar_novo(window, cx);
        self.avisar("Rascunho descartado.", false);
        self.acompanhar(window, cx);
    }

    // ── Colheita ─────────────────────────────────────────────────────────

    pub(super) fn acompanhar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn_in(window, async move |tela, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let continua = tela
                .update_in(cx, |tela, window, cx| tela.colher(window, cx))
                .unwrap_or(false);
            if !continua {
                break;
            }
        }));
    }

    /// Drena os canais. Devolve se vale continuar acordando.
    pub fn colher(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;

        // 🚨 **O aviso de "estou revelando" sai daqui, e não de `aplicar_receita`.**
        // Ela é chamada de lugares sem `Context` (a releitura do catálogo, a
        // chegada dos presets), então deixa a bandeira e quem tem `cx` a leva à
        // raiz. Sem este trecho o serviço revelava no disco, ninguém colhia os
        // avisos, as miniaturas ficavam nas de antes e a barra "Preset padrão"
        // parava em 0/N — a receita acontecia e a tela não mostrava (achado do
        // dono, 17/set/2026: *"aplicou o filtro padrão e corte e não faz o que
        // promete"*).
        if std::mem::take(&mut self.pedir_colheita_das_reveladas) {
            cx.emit(PedidoDaNova::RevelandoReceita);
        }
        // E o progresso é lido do serviço a cada volta: quem conta é quem
        // termina o trabalho.
        let andamento = self.portas.receita_padrao.progresso();
        if (andamento.total, andamento.prontas) != (self.receita.total, self.receita.prontas) {
            self.receita.total = andamento.total;
            self.receita.prontas = andamento.prontas;
            mudou = true;
        }

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Galerias(lista) => {
                    self.galerias = Some(lista);
                    self.chegou_lista(window, cx);
                }
                Recado::Produtos(lista) => {
                    self.produtos = lista;
                    self.chegou_lista(window, cx);
                }
                Recado::Estudios(lista) => {
                    self.estudios = lista;
                    self.chegou_lista(window, cx);
                }
                Recado::Json { rotulo, resultado } => match rotulo {
                    "nova-presets" => {
                        let do_servidor = resultado
                            .as_ref()
                            .map(receita::presets_do_servidor)
                            .unwrap_or_default();
                        self.presets = receita::presets_da_sessao(
                            &self.portas.presets_do_sistema,
                            do_servidor,
                        );
                        self.presets_chegaram = true;
                        self.aplicar_receita();
                    }
                    "nova-criada" => self.receber_criacao(resultado),
                    "nova-parceiro-criado" => self.receber_cadastro(resultado, cx),
                    "nova-parceiro-existente" => {
                        if let (Some(cadastro), Ok(valor)) = (self.cadastro.as_mut(), resultado) {
                            let nome = cadastro.nome.read(cx).value().to_string();
                            cadastro.existente = assoc::achar_parceiro_pelo_nome(
                                &assoc::parceiros_da_api(&valor),
                                &nome,
                            )
                            .cloned();
                        }
                    }
                    outro => {
                        let tipo = [
                            TipoDeBusca::Agendamento,
                            TipoDeBusca::Voucher,
                            TipoDeBusca::Compra,
                            TipoDeBusca::Parceiro,
                        ]
                        .into_iter()
                        .find(|t| t.rotulo() == outro);
                        if let Some(tipo) = tipo {
                            self.receber_busca(tipo, resultado);
                        }
                    }
                },
                Recado::Falhou(erro) => {
                    eprintln!("⚠️ [Nova sessão] {erro}");
                    self.carregando = false;
                }
                _ => {}
            }
        }

        while let Ok(escolhidos) = self.escolhas.1.try_recv() {
            mudou = true;
            self.escolhendo = false;
            self.importar_arquivos(escolhidos, window, cx);
        }

        while let Ok(recado) = self.origens.1.try_recv() {
            mudou = true;
            match recado {
                RecadoDaImportacao::Origens { cartoes, .. } => {
                    if let Some(menu) = self.menu_da_origem.as_mut() {
                        menu.procurando = false;
                        menu.cartoes = cartoes.into_iter().map(|o| (o.nome, o.caminho)).collect();
                    }
                }
                RecadoDaImportacao::OrigemEscolhida(pasta) => {
                    self.escolhendo = false;
                    self.portas
                        .explorador
                        .varrer(pasta, true, self.origens.0.clone());
                }
                RecadoDaImportacao::SemEscolha => self.escolhendo = false,
                RecadoDaImportacao::Varrido { raiz, arquivos } => {
                    let nome = std::path::Path::new(&raiz)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or(raiz);
                    let fotos: Vec<String> = arquivos
                        .into_iter()
                        .filter(|a| {
                            crate::sessoes::arquivos::so_as_fotos(&[PathBuf::from(a)]).len() == 1
                        })
                        .collect();
                    if fotos.is_empty() {
                        self.avisar(format!("Nenhuma foto em {nome}."), false);
                    } else {
                        self.avisar(
                            format!(
                                "{} de {nome} na fila.",
                                estado::plural(fotos.len(), "foto", "fotos")
                            ),
                            false,
                        );
                        self.importar_arquivos(fotos, window, cx);
                    }
                }
                RecadoDaImportacao::Falhou(erro) => {
                    self.escolhendo = false;
                    self.avisar(erro, true);
                }
                _ => {}
            }
        }

        let mut terminou_copia = false;
        while let Ok(andamento) = self.andamentos.1.try_recv() {
            mudou = true;
            let Some(lote) = self.importacao.as_mut() else {
                continue;
            };
            match andamento {
                Andamento::Comecou { total } => lote.total = total,
                Andamento::Feito { .. } | Andamento::Pulado { .. } => lote.feitas += 1,
                Andamento::Falhou { erro, .. } => {
                    lote.falhas += 1;
                    self.falhas_da_copia = Some((lote.falhas, erro));
                }
                Andamento::Terminou {
                    sucesso,
                    falhas,
                    pulados,
                } => {
                    lote.feitas = sucesso + pulados;
                    lote.falhas = falhas;
                    lote.total = lote.total.max(sucesso + pulados + falhas);
                    terminou_copia = true;
                }
            }
            if lote.terminou() {
                terminou_copia = true;
            }
        }
        if terminou_copia {
            self.reler_fotos();
            cx.emit(PedidoDaNova::CatalogoMudou);
        }

        while let Ok(fotos) = self.releituras.1.try_recv() {
            mudou = true;
            self.esperando_releitura = false;
            let id = self.rascunho.id_provisorio.clone();
            let mut orfas: Vec<String> = fotos
                .iter()
                .filter_map(|f| f.sessao_id.clone())
                .filter(|s| estado::eh_rascunho(s) && *s != id)
                .filter(|s| self.guardado.as_ref().is_none_or(|g| &g.id_provisorio != s))
                .collect();
            orfas.sort();
            orfas.dedup();
            self.orfas = orfas;
            if let Some(guardado) = &self.guardado {
                self.fotos_do_guardado = fotos
                    .iter()
                    .filter(|f| f.sessao_id.as_deref() == Some(guardado.id_provisorio.as_str()))
                    .count();
                if !estado::rascunho_tem_conteudo(guardado, self.fotos_do_guardado) {
                    // Nada que valha perguntar: segue com ele.
                    self.retomar(window, cx);
                }
                self.procurando_rascunho = false;
            }
            self.fotos = fotos
                .into_iter()
                .filter(|f| f.sessao_id.as_deref() == Some(self.rascunho.id_provisorio.as_str()))
                .collect();
            if self.importacao.is_some_and(|i| i.terminou()) {
                self.importacao = None;
            }
            // 🔑 **Depois da releitura, e não quando a leva termina.** As fotos
            // da leva anterior só entram em `self.fotos` aqui; despachar antes
            // faria o total da barra cair (ele soma as gravadas ao lote em
            // curso) e a contagem andaria para trás na frente do operador.
            if self.importacao.is_none() {
                if let Some(proxima) = self.fila_de_levas.pop_front() {
                    self.despachar_leva(proxima, window, cx);
                }
            }
            self.aplicar_receita();
        }

        while let Ok(resultado) = self.catalogo.1.try_recv() {
            mudou = true;
            if self.esperando_catalogo == Some(Fase::Movendo) {
                self.receber_troca(resultado, cx);
            } else {
                self.esperando_descarte = false;
                if resultado.is_err() {
                    self.avisar("Não foi possível apagar as fotos. Tente de novo.", true);
                }
                self.reler_fotos();
                cx.emit(PedidoDaNova::CatalogoMudou);
            }
        }

        if self.fase == Some(Fase::Copiando) && !self.importando() {
            self.seguir_criacao(window, cx);
            mudou = true;
        }

        if let Some(busca) = self.busca.as_ref() {
            if busca
                .agendada
                .is_some_and(|quando| Instant::now() >= quando)
            {
                self.buscar(cx);
                mudou = true;
            }
        }

        if self.amostras.colher() {
            mudou = true;
        }

        if self.aviso.as_ref().is_some_and(|a| Instant::now() >= a.ate) {
            self.aviso = None;
            mudou = true;
        }

        if mudou {
            cx.notify();
        }

        let continua = self.carregando
            || self.escolhendo
            || self.importando()
            || self.fase.is_some()
            || self.esperando_catalogo.is_some()
            || self.esperando_descarte
            || self.esperando_releitura
            || self.aviso.is_some()
            || self.amostras.esperando()
            // A receita anda numa thread: enquanto ela não termina, a tela
            // continua acordando para colher os avisos e mover a barra.
            || self.portas.receita_padrao.progresso().andando()
            || self.menu_da_origem.as_ref().is_some_and(|m| m.procurando)
            || self
                .cadastro
                .as_ref()
                .is_some_and(|c| c.enviando || c.erro.is_some())
            || self
                .busca
                .as_ref()
                .is_some_and(|b| b.carregando || b.agendada.is_some());
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn chegou_lista(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.galerias.is_some() && !self.produtos.is_empty() {
            self.carregando = false;
        }
        if self.guardado.is_none() {
            self.sugerir();
            self.guardar();
        }
        self.atualizar_escolhas(window, cx);
    }

    /// Lê as miniaturas que faltam (até 60) e a foto das amostras.
    ///
    /// 🚨 **A grade mostra a foto com a receita, e não como ela veio.** Era o
    /// contrário: a miniatura entrava no mapa uma vez e nunca mais era refeita,
    /// então escolher o preset padrão mudava os cartões do seletor e deixava os
    /// quadros na imagem de antes (achado do dono, 17/set/2026).
    ///
    /// 🔑 **Quem revela é o serviço, não esta tela.** `sessoes::receita_padrao`
    /// grava a miniatura revelada em `revelada:<id>` e avisa; aqui só se lê,
    /// preferindo essa chave e caindo no bruto quando ela ainda não existe — é
    /// o mesmo arranjo do site, onde o trabalhador revela e a grade lê a prévia.
    /// Ler é também o que faz a **sessão** mostrar a mesma imagem que o
    /// assistente mostrou, sem revelar de novo.
    pub(super) fn preparar_miniaturas(&mut self) {
        let previews = self.portas.previews.clone();
        let ids: Vec<String> = self.fotos.iter().take(60).map(|f| f.id.clone()).collect();
        for id in ids {
            if self.miniaturas.contains_key(&id) {
                continue;
            }
            let revelada = persistencia::chave_da_revelada(&id);
            let imagem = previews
                .get_thumbnail(&revelada)
                .or_else(|| previews.get_thumbnail(&id))
                .or_else(|| previews.get_preview(&id).map(|g| g.thumbnail(320, 320)));
            if let Some(imagem) = imagem {
                self.miniaturas
                    .insert(id.clone(), crate::imagem::para_gpui(imagem));
            }
        }
        let primeira = self
            .fotos
            .iter()
            .find(|f| self.miniaturas.contains_key(&f.id))
            .map(|f| f.id.clone());
        self.amostras.definir_base(primeira.clone(), || {
            let id = primeira?;
            previews
                .get_preview(&id)
                .or_else(|| previews.get_thumbnail(&id))
        });
    }

    /// Uma foto acabou de ser revelada pela receita: a miniatura velha sai, e a
    /// próxima passada de `preparar_miniaturas` lê a nova.
    ///
    /// 🔑 **Esquecer, e não gravar aqui.** Quem gravou foi o serviço, em disco;
    /// esta tela só descarta o que tem na mão — C17 do Contrato da Foto ("os
    /// caches derivados saem e são refeitos") visto do lado de quem desenha.
    pub fn revelada_chegou(&mut self, foto_id: &str) {
        self.miniaturas.remove(foto_id);
    }

    /// A amostra do cartão deste preset (`None` = "Nenhum").
    pub(super) fn amostra(&mut self, id: Option<&str>) -> Option<Arc<RenderImage>> {
        let preset = id.and_then(|id| self.presets.iter().find(|p| p.id == id));
        let ajustes = receita::ajustes_da_receita(preset.map(|p| &p.preset));
        self.amostras.obter(id.unwrap_or("nenhum"), ajustes)
    }

    /// Quantas fotos já têm prévia.
    pub(super) fn quantas_previas(&self) -> usize {
        self.fotos
            .iter()
            .filter(|f| {
                self.portas.previews.tem(&f.id, PreviewType::Thumbnail)
                    || self.portas.previews.tem(&f.id, PreviewType::Large)
            })
            .count()
    }

    /// 🧪 O botão de confirmar do diálogo.
    #[cfg(test)]
    pub(crate) fn confirmar_para_teste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmar(window, cx);
    }

    /// 🧪 O aviso na tela.
    #[cfg(test)]
    pub(crate) fn aviso_para_teste(&self) -> Option<String> {
        self.aviso.as_ref().map(|a| a.texto.clone())
    }

    /// 🧪 O erro da criação.
    #[cfg(test)]
    pub(crate) fn erro_para_teste(&self) -> Option<(String, Option<usize>)> {
        self.erro.clone()
    }

    /// 🧪 O rascunho como está.
    #[cfg(test)]
    pub(crate) fn rascunho_para_teste(&self) -> Rascunho {
        self.rascunho.clone()
    }

    /// 🧪 Os itens da busca aberta.
    #[cfg(test)]
    pub(crate) fn itens_da_busca(&self) -> Vec<ItemDaBusca> {
        self.busca
            .as_ref()
            .map(|b| b.itens.clone())
            .unwrap_or_default()
    }

    /// 🧪 Se há um cartão de retomada à vista.
    #[cfg(test)]
    pub(crate) fn perguntando_se_retoma(&self) -> bool {
        self.guardado.is_some()
    }
}

impl Focusable for NovaSessao {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

fn mensagem_da_busca(erro: &str, o_que: &str) -> String {
    let (status, frase) = estado::ler_erro_do_site(erro);
    match status {
        Some(403) => format!("Sua conta não pode ver {o_que}."),
        Some(404) => format!("A busca de {o_que} ainda não está disponível na API."),
        Some(400) => frase,
        _ => format!("Não foi possível buscar {o_que}. Tente de novo."),
    }
}

/// O que esta máquina lembra da última sessão criada: estúdio, preset e
/// corte (dono, 2026-09-13: *"Grave o último preset selecionado e o corte
/// utilizado e o estúdio."*).
///
/// 🔑 **O mesmo arquivo da criação antiga** (`sessao-nova.json`), que só tinha
/// o estúdio: quem já escolheu um continua com ele sugerido.
pub mod lembranca {
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Lembranca {
        pub estudio_id: Option<String>,
        pub preset_id: Option<String>,
        pub proporcao: Option<String>,
    }

    #[cfg(not(test))]
    fn caminho() -> PathBuf {
        infrastructure::paths::AppPaths::catalog_root().join("sessao-nova.json")
    }

    #[cfg(test)]
    fn caminho() -> PathBuf {
        std::env::temp_dir().join(format!("vlb-lembranca-teste-{}.json", std::process::id()))
    }

    pub fn ler() -> Lembranca {
        std::fs::read_to_string(caminho())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn gravar(lembranca: &Lembranca) {
        let Ok(texto) = serde_json::to_string_pretty(lembranca) else {
            return;
        };
        let caminho = caminho();
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        let _ = std::fs::write(caminho, texto);
    }
}
