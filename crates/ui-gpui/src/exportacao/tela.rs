//! O modal de exportação: escolher como e para onde, e acompanhar o lote.
//!
//! # 🔄 Refeito em 03/10/2026, depois do print do dono
//!
//! *"A funcionalidade de exportar fotos precisa ser melhorada e corrigida, pois
//! não funciona e a UX está péssima!"* — o print mostrava `foto-16.jpg: Photo ID
//! inválido` e "0 exportadas · 16 falharam". Eram quatro defeitos de uma vez:
//!
//! 1. a foto da sessão (`site:<id>`) ia ao catálogo, que não a conhece — ver
//!    [`super::porta`];
//! 2. o nome saía do `path`, vazio na foto do site: `foto`, `foto-2`… `foto-16`;
//! 3. a marca d'água abria o seletor de **pasta**, e a prévia nunca saía;
//! 4. só a última falha aparecia, sem barra, sem parar, sem "mostrar na pasta".
//!
//! 🔑 **A tela segue o `exportar-dialogo.tsx` do site** — a referência de
//! comportamento: formatos e qualidade iguais, "Salvar em" com a pasta à
//! vista, e o rodapé do `AlertDialog`. Cada controle é uma peça do gpui-kit
//! pelos atalhos do `estilo.rs` (dono, 28/09: *"Usa tudo da GPUI KIT"*).
//!
//! ## Os dois usos (09/10/2026)
//!
//! *"A escolha de uso é PDF/Arquivo individual, sendo que a marca d'água fica
//! automática baseado no se a foto foi sinalizada com LEVADA"* (dono). O modal
//! não pergunta mais pela marca nem pede logotipo: a levada sai limpa e a
//! outra sai com a marca d'água do site (ver [`super::porta`]). O que se
//! escolhe é a forma — **arquivos** soltos ou o **fotolivro** em PDF, o mesmo
//! livro que a galeria do cliente gera (crate `fotolivro`).
//!
//! ## O desenho (10/10/2026)
//!
//! *"O design dessa tela tá muito confuso para o usuário! Precisa usar o GPUI
//! Kit e deixar bem lindo e limpo!"* (dono, com o print). Eram cinco textos do
//! mesmo peso empilhados — a frase do alto, a explicação do uso, a contagem, a
//! caixa de seleção com travessão e o "Salvar em" com dois botões sem moldura
//! — e nada dizia o que era escolha e o que era aviso. Agora a tela tem três
//! blocos, cada um uma peça do kit:
//!
//! 1. **o uso**, num `TabBar` segmentado de ponta a ponta, com uma linha só
//!    dizendo o que ele entrega;
//! 2. **o que sai**, num `GroupBox`: uma linha para as levadas, outra para as
//!    à venda, e o que cada uma leva;
//! 3. **a opção do uso**: o formato dos arquivos (outro `TabBar` segmentado —
//!    eram quatro `Radio` com uma legenda cada) ou, no fotolivro, o `Switch`
//!    "Só as fotos levadas";
//! 4. **para onde**, com a pasta num `GroupBox` próprio e o "Trocar…" de
//!    contorno.
//!
//! 📐 **A caixa não muda de tamanho** (dono, no mesmo dia: *"Eu não gosto
//! quando a tela muda de tamanho com uma ação!"*). Trocar de uso, de formato,
//! ligar a chave, o aviso do livro vazio e a passagem para o lote: tudo que se
//! alterna divide o mesmo lugar ([`estilo::mesmo_lugar`]), e a caixa tem
//! sempre a altura do maior estado.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants as _};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon};
use gpui_kit::{
    div, prelude::*, px, AnyElement, Context, Entity, EventEmitter, FontWeight, SharedString,
    StyleRefinement, Subscription, Task, Window,
};

use domain::services::pos_venda::Sessao;
use domain::value_objects::{ExportOptions, FormatoDeSaida};

use crate::estilo;
use crate::importacao::estado::Recado;
use crate::importacao::explorador::SeletorDePasta;
use crate::recursos::Icone;
use crate::revelacao::persistencia;

use super::destino::Destinos;
use super::porta::{Andamento, Exportador, FotoDoLivro, Origem, Saida};
use super::preferencias::{self, Preferencias};

/// O mesmo intervalo da colheita da importação.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// A forma da exportação. A marca d'água não é escolha: é a LEVADA de cada
/// foto que decide (ver [`super::porta`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    /// Um arquivo por foto.
    #[default]
    Arquivos,
    /// 📖 O fotolivro: um PDF diagramado, para mandar ao cliente.
    Fotolivro,
}

impl Modo {
    pub fn rotulo(self) -> &'static str {
        match self {
            Modo::Arquivos => "Arquivos individuais",
            Modo::Fotolivro => "Fotolivro (PDF)",
        }
    }

    /// O que o uso entrega, em uma linha — o que cada foto leva está no
    /// bloco "o que sai", e não aqui.
    pub fn explicacao(self) -> &'static str {
        match self {
            Modo::Arquivos => "Um arquivo por foto, com os ajustes e o corte aplicados.",
            Modo::Fotolivro => {
                "Um PDF diagramado para mandar ao cliente, com o link de cada foto na galeria."
            }
        }
    }
}

/// O que a capa e os links do fotolivro precisam saber da sessão.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InfoDoLivro {
    pub titulo: String,
    /// A galeria no site: é ela que vira os links do livro.
    pub galeria_id: Option<String>,
    /// `"2026-10-09"` — o dia do ensaio.
    pub data_iso: String,
}

/// "2026-10-09" → "9 de outubro de 2026".
pub fn data_por_extenso(iso: &str) -> String {
    const MESES: [&str; 12] = [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ];
    let partes: Vec<u32> = iso
        .get(..10)
        .unwrap_or("")
        .split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    match partes[..] {
        [ano, mes @ 1..=12, dia] => format!("{dia} de {} de {ano}", MESES[mes as usize - 1]),
        _ => String::new(),
    }
}

/// O que cada formato é, com as palavras do site (`exportar.ts`): o nome da
/// aba e a legenda que aparece quando ele é o escolhido.
fn detalhe_do_formato(formato: FormatoDeSaida) -> (&'static str, &'static str) {
    match formato {
        FormatoDeSaida::Jpeg => ("JPEG", "O de sempre: o menor arquivo."),
        FormatoDeSaida::Png => ("PNG", "Sem perda, para reeditar."),
        FormatoDeSaida::Tiff => ("TIFF", "Sem perda, o que o laboratório aceita."),
        FormatoDeSaida::Webp => ("WebP", "Sem perda, para a web."),
    }
}

/// "1 levada", "5 levadas".
fn quantas_levadas(n: usize) -> String {
    match n {
        1 => "1 levada".into(),
        n => format!("{n} levadas"),
    }
}

/// O que o bloco "o que sai" diz de cada grupo, à direita da contagem.
///
/// 💧 A não levada é a prévia marcada do site, sempre em JPEG — por isso o
/// formato só aparece nela quando o operador escolheu outro para as levadas.
fn o_que_sai(modo: Modo, formato: FormatoDeSaida, so_levadas: bool) -> (String, String) {
    let nome = detalhe_do_formato(formato).0;
    match modo {
        Modo::Arquivos => (
            format!("sem marca d'água, em {nome}"),
            if formato == FormatoDeSaida::Jpeg {
                "com a marca d'água do site".into()
            } else {
                "com a marca d'água, em JPEG".into()
            },
        ),
        Modo::Fotolivro => (
            "sem marca d'água".into(),
            if so_levadas {
                "ficam fora do livro".into()
            } else {
                "com a marca d'água e o link para comprar".into()
            },
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progresso {
    pub total: usize,
    pub feitas: usize,
    pub falhas: usize,
    pub canceladas: usize,
    pub terminou: bool,
}

impl Progresso {
    /// De 0 a 100, para a `Progress` do kit.
    pub fn porcento(&self) -> f32 {
        if self.total == 0 {
            return 0.;
        }
        (self.feitas + self.falhas) as f32 * 100. / self.total as f32
    }
}

/// Uma foto que não saiu: o nome do arquivo e o porquê, em frase.
#[derive(Debug, Clone, PartialEq)]
pub struct Falha {
    pub nome: String,
    pub erro: String,
}

/// O que a exportação pede à raiz.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoDaExportacao {
    /// "Cancelar" ou "Concluir": fechar o modal.
    Fechar,
    /// O lote terminou — a raiz avisa em toast se o modal já estiver fechado.
    Terminou { texto: String, falhou: bool },
    /// "Ver o fotolivro": a raiz abre o visualizador com o livro gravado.
    VerLivro(PathBuf),
}

impl EventEmitter<PedidoDaExportacao> for Exportacao {}

pub struct Exportacao {
    exportador: Arc<dyn Exportador>,
    seletor: Arc<dyn SeletorDePasta>,
    /// As fotos que o lote vai exportar — copiadas ao abrir.
    ///
    /// 🔑 **Copiadas, e não lidas da grade a cada quadro.** Um lote de 400 fotos
    /// que corre por um minuto não pode mudar de tamanho porque alguém clicou
    /// numa miniatura enquanto ele roda.
    fotos: Vec<PhotoViewModel>,
    /// A conta do site — sem ela a foto da sessão não tem de onde vir.
    sessao: Option<Sessao>,
    /// O bruto **neste disco** das fotos do site que subiram daqui, pelo id
    /// de lá: poupa o download.
    copias_locais: HashMap<String, PathBuf>,
    pasta: Option<PathBuf>,
    modo: Modo,
    formato: FormatoDeSaida,
    qualidade: Entity<SliderState>,
    /// 📖 No fotolivro, só as levadas — o livro do cliente, sem venda.
    so_levadas: bool,
    /// A sessão, para a capa e os links do fotolivro.
    livro: InfoDoLivro,
    /// O fotolivro gravado — o "Mostrar na pasta" aponta ele.
    arquivo_do_livro: Option<PathBuf>,
    progresso: Option<Progresso>,
    /// O último nome gravado, para a tela mostrar que algo está acontecendo.
    ultimo: Option<SharedString>,
    falhas: Vec<Falha>,
    /// O lote em curso (ou o último): é dele que sai o "tentar de novo".
    lote: Vec<Saida>,
    /// Os destinos que deram certo — "Mostrar na pasta" aponta o primeiro.
    feitas: Vec<PathBuf>,
    cancelar: Arc<AtomicBool>,
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    recados: (Sender<Recado>, Receiver<Recado>),
    esperando_pasta: bool,
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _qualidade_mudou: Subscription,
}

impl Exportacao {
    pub fn nova(
        exportador: Arc<dyn Exportador>,
        seletor: Arc<dyn SeletorDePasta>,
        cx: &mut Context<Self>,
    ) -> Self {
        let lembradas = preferencias::ler_de(&preferencias::caminho());
        let qualidade = cx.new(|_| {
            SliderState::new()
                .min(60.)
                .max(100.)
                .step(1.)
                .default_value(lembradas.qualidade.clamp(60, 100) as f32)
        });
        let qualidade_mudou = cx.subscribe(&qualidade, |tela, _, evento: &SliderEvent, cx| {
            // O `Release` chega depois do último `Change` com o mesmo valor:
            // só ele grava, para não escrever no disco a cada pixel do arrasto.
            if let SliderEvent::Release(_) = evento {
                tela.lembrar(cx);
            }
            cx.notify();
        });
        Self {
            exportador,
            seletor,
            fotos: Vec::new(),
            sessao: None,
            copias_locais: HashMap::new(),
            pasta: Some(
                lembradas
                    .pasta_que_existe()
                    .unwrap_or_else(preferencias::pasta_dos_downloads),
            ),
            modo: lembradas.modo(),
            formato: lembradas.formato(),
            qualidade,
            so_levadas: lembradas.so_levadas,
            livro: InfoDoLivro::default(),
            arquivo_do_livro: None,
            progresso: None,
            ultimo: None,
            falhas: Vec::new(),
            lote: Vec::new(),
            feitas: Vec::new(),
            cancelar: Arc::new(AtomicBool::new(false)),
            andamentos: channel(),
            recados: channel(),
            esperando_pasta: false,
            colhendo: false,
            _colheita: None,
            _qualidade_mudou: qualidade_mudou,
        }
    }

    /// Abre o modal para uma seleção.
    ///
    /// ⚠️ **O progresso do lote anterior é jogado fora aqui, e não ao fechar** —
    /// fechar e reabrir para conferir "quantas saíram mesmo?" é natural. Com um
    /// lote **em curso** nada é zerado: reabrir mostra o lote andando.
    pub fn abrir_para(
        &mut self,
        fotos: Vec<PhotoViewModel>,
        sessao: Option<Sessao>,
        copias_locais: HashMap<String, PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.correndo() {
            cx.notify();
            return;
        }
        self.fotos = fotos;
        self.sessao = sessao;
        self.copias_locais = copias_locais;
        self.progresso = None;
        self.ultimo = None;
        self.falhas.clear();
        self.feitas.clear();
        self.lote.clear();
        self.arquivo_do_livro = None;
        cx.notify();
    }

    /// A sessão das fotos — a capa e os links do fotolivro.
    pub fn definir_livro(&mut self, livro: InfoDoLivro) {
        self.livro = livro;
    }

    pub fn quantas(&self) -> usize {
        self.fotos.len()
    }

    pub fn pasta(&self) -> Option<&PathBuf> {
        self.pasta.as_ref()
    }

    pub fn modo(&self) -> Modo {
        self.modo
    }

    /// O formato das levadas. A não levada sai sempre em JPEG: é a prévia
    /// marcada do site, como ele a grava.
    pub fn formato(&self) -> FormatoDeSaida {
        self.formato
    }

    pub fn so_levadas(&self) -> bool {
        self.so_levadas
    }

    pub fn escolher_so_levadas(&mut self, sim: bool, cx: &mut Context<Self>) {
        self.so_levadas = sim;
        self.lembrar(cx);
        cx.notify();
    }

    /// Quantas saem limpas e quantas com a marca.
    pub fn contagem(&self) -> (usize, usize) {
        let levadas = self
            .fotos
            .iter()
            .filter(|f| f.comprada || f.pos_venda_foto_id.is_none())
            .count();
        (levadas, self.fotos.len() - levadas)
    }

    pub fn arquivo_do_livro(&self) -> Option<&PathBuf> {
        self.arquivo_do_livro.as_ref()
    }

    pub fn qualidade(&self, cx: &gpui_kit::App) -> u8 {
        self.qualidade.read(cx).value().start().round() as u8
    }

    pub fn falhas(&self) -> &[Falha] {
        &self.falhas
    }

    pub fn escolher_modo(&mut self, modo: Modo, cx: &mut Context<Self>) {
        self.modo = modo;
        self.lembrar(cx);
        cx.notify();
    }

    pub fn escolher_formato(&mut self, formato: FormatoDeSaida, cx: &mut Context<Self>) {
        self.formato = formato;
        self.lembrar(cx);
        cx.notify();
    }

    /// As opções das levadas: tamanho cheio, no formato e na qualidade
    /// escolhidos. A não levada não passa por elas — é a prévia marcada.
    pub fn opcoes(&self, cx: &gpui_kit::App) -> ExportOptions {
        ExportOptions::default()
            .with_quality(self.qualidade(cx))
            .with_formato(self.formato())
    }

    pub fn progresso(&self) -> Option<Progresso> {
        self.progresso
    }

    /// Abre o seletor nativo de pasta.
    pub fn escolher_pasta(&mut self, cx: &mut Context<Self>) {
        self.esperando_pasta = true;
        self.seletor.escolher_destino(self.recados.0.clone(), cx);
        self.acompanhar(cx);
        cx.notify();
    }

    /// Volta o destino para a pasta Downloads — o "Limpar" do site.
    pub fn voltar_aos_downloads(&mut self, cx: &mut Context<Self>) {
        self.pasta = Some(preferencias::pasta_dos_downloads());
        self.lembrar(cx);
        cx.notify();
    }

    /// A pasta, sem passar pelo seletor nativo.
    #[cfg(test)]
    pub fn escolher_pasta_para_teste(&mut self, pasta: PathBuf, cx: &mut Context<Self>) {
        self.pasta = Some(pasta);
        cx.notify();
    }

    /// A pasta, sem o seletor nativo e **sem lembrar** — o roteiro de
    /// depuração não pode trocar o destino de quem trabalha.
    pub fn escolher_pasta_em(&mut self, pasta: PathBuf, cx: &mut Context<Self>) {
        self.pasta = Some(pasta);
        cx.notify();
    }

    /// O que vai para o disco de cada foto: de onde ela vem.
    ///
    /// 🚨 **A foto que só existe no site não tem linha no catálogo** — o id
    /// dela é `site:<uuid>`. Mandá-la ao catálogo foi o "Photo ID inválido"
    /// do print. Ela sai pelo site, com a revelação que a grade tem para ela.
    ///
    /// 💧 **E a foto que está no site e não foi levada sai marcada, esteja
    /// onde estiver o bruto.** A que subiu deste computador também existe no
    /// catálogo, e ia por ele — limpa (09/10/2026, achado no roteiro do app
    /// real). Só a levada, ou a que nunca subiu, sai pelo catálogo.
    fn origem_de(&self, foto: &PhotoViewModel) -> Origem {
        match foto.pos_venda_foto_id.as_ref() {
            Some(no_site) if !foto.comprada => Origem::Site {
                foto_no_site: no_site.clone(),
                ajustes: persistencia::da_foto(foto),
                corte: persistencia::para_crop_settings(&persistencia::corte_da_foto(foto)),
                original_local: self.copias_locais.get(no_site).cloned(),
                levada: false,
            },
            Some(no_site) if persistencia::so_existe_no_site(foto) => Origem::Site {
                foto_no_site: no_site.clone(),
                ajustes: persistencia::da_foto(foto),
                corte: persistencia::para_crop_settings(&persistencia::corte_da_foto(foto)),
                original_local: self.copias_locais.get(no_site).cloned(),
                levada: foto.comprada,
            },
            _ => Origem::Catalogo {
                id: foto.id.clone(),
            },
        }
    }

    /// Dispara o lote.
    ///
    /// 🚨 **Os destinos são calculados aqui, de uma vez, com reserva de nome.**
    /// Escolher o nome de cada foto na hora de gravar deixaria duas fotos com o
    /// mesmo nome de origem apontando para o mesmo arquivo. Ver
    /// [`super::destino`].
    pub fn exportar(&mut self, cx: &mut Context<Self>) {
        let Some(pasta) = self.pasta.clone() else {
            return;
        };
        if self.fotos.is_empty() || self.correndo() {
            return;
        }
        if self.modo == Modo::Fotolivro {
            self.montar_o_livro(pasta, cx);
            return;
        }

        let mut destinos = Destinos::na_pasta(pasta);
        let extensao = self.formato().extensao();
        let saidas: Vec<Saida> = self
            .fotos
            .iter()
            .map(|foto| {
                let origem = self.origem_de(foto);
                // 💧 A não levada é a prévia marcada do site: JPEG, sempre.
                let extensao = match &origem {
                    Origem::Site { levada: false, .. } => "jpg",
                    _ => extensao,
                };
                Saida {
                    origem,
                    // A foto do site não tem caminho: o nome é o do arquivo da
                    // câmera, e não `foto`, `foto-2`… (o print).
                    destino: destinos.para(
                        if foto.path.is_empty() {
                            &foto.name
                        } else {
                            &foto.path
                        },
                        extensao,
                    ),
                }
            })
            .collect();
        self.mandar(saidas, cx);
    }

    /// O endereço do site, sem barra no fim.
    fn site(&self) -> String {
        crate::pos_venda::config::ler()
            .site()
            .trim_end_matches('/')
            .to_string()
    }

    /// 📖 As fotos do livro, com o link de cada uma para a galeria do cliente.
    pub fn fotos_do_livro(&self) -> Vec<FotoDoLivro> {
        let site = self.site();
        self.fotos
            .iter()
            .map(|foto| {
                let origem = self.origem_de(foto);
                let link = match (&origem, &self.livro.galeria_id) {
                    (Origem::Site { foto_no_site, .. }, Some(galeria)) => {
                        Some(format!("{site}/meus-ensaios/{galeria}?foto={foto_no_site}"))
                    }
                    _ => None,
                };
                FotoDoLivro {
                    origem,
                    nome: foto.name.clone(),
                    link,
                }
            })
            .filter(|f| !self.so_levadas || f.levada())
            .collect()
    }

    /// A capa do livro: o título, o dia e os links da galeria e do agendar.
    pub fn capa_do_livro(&self) -> fotolivro::Capa {
        let site = self.site();
        fotolivro::Capa {
            titulo: self.livro.titulo.clone(),
            lugar_e_data: data_por_extenso(&self.livro.data_iso),
            galeria: self
                .livro
                .galeria_id
                .as_ref()
                .map(|g| format!("{site}/meus-ensaios/{g}")),
            agendar: Some(format!("{site}/agendar")),
            site: site
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string(),
        }
    }

    fn montar_o_livro(&mut self, pasta: PathBuf, cx: &mut Context<Self>) {
        let fotos = self.fotos_do_livro();
        if fotos.is_empty() {
            return;
        }
        let titulo = if self.livro.titulo.trim().is_empty() {
            "Fotolivro".to_string()
        } else {
            self.livro.titulo.trim().replace(['/', '\\', ':'], "-")
        };
        let destino = Destinos::na_pasta(pasta).para(&format!("{titulo}.pdf"), "pdf");
        self.progresso = Some(Progresso {
            total: fotos.len(),
            ..Default::default()
        });
        self.ultimo = None;
        self.falhas.clear();
        self.feitas.clear();
        self.lote.clear();
        self.arquivo_do_livro = None;
        self.cancelar = Arc::new(AtomicBool::new(false));
        self.exportador.fotolivro(
            fotos,
            self.capa_do_livro(),
            self.livro.galeria_id.clone(),
            destino,
            self.sessao.clone(),
            self.cancelar.clone(),
            self.andamentos.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// Manda de novo só as que falharam, com os mesmos nomes.
    pub fn tentar_de_novo(&mut self, cx: &mut Context<Self>) {
        if self.correndo() {
            return;
        }
        // O livro é um arquivo só: tentar de novo é refazê-lo.
        if self.modo == Modo::Fotolivro {
            if let Some(pasta) = self.pasta.clone() {
                self.montar_o_livro(pasta, cx);
            }
            return;
        }
        let falharam: HashSet<String> = self.falhas.iter().map(|f| f.nome.clone()).collect();
        let saidas: Vec<Saida> = self
            .lote
            .iter()
            .filter(|s| falharam.contains(&nome(&s.destino)))
            .cloned()
            .collect();
        if !saidas.is_empty() {
            self.mandar(saidas, cx);
        }
    }

    fn mandar(&mut self, saidas: Vec<Saida>, cx: &mut Context<Self>) {
        let opcoes = self.opcoes(cx);
        self.progresso = Some(Progresso {
            total: saidas.len(),
            ..Default::default()
        });
        self.ultimo = None;
        self.falhas.clear();
        self.feitas.clear();
        self.lote = saidas.clone();
        self.cancelar = Arc::new(AtomicBool::new(false));

        self.exportador.exportar(
            saidas,
            opcoes,
            self.sessao.clone(),
            self.cancelar.clone(),
            self.andamentos.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// "Parar": as fotos em curso terminam, as outras não começam.
    pub fn parar(&mut self, cx: &mut Context<Self>) {
        self.cancelar.store(true, Ordering::SeqCst);
        cx.notify();
    }

    pub fn parando(&self) -> bool {
        self.correndo() && self.cancelar.load(Ordering::SeqCst)
    }

    /// Um lote em curso. É o que desliga o botão: dois cliques exportariam tudo
    /// duas vezes.
    pub fn correndo(&self) -> bool {
        self.progresso.is_some_and(|p| !p.terminou)
    }

    fn lembrar(&self, cx: &gpui_kit::App) {
        preferencias::gravar_em(
            &preferencias::caminho(),
            &Preferencias {
                pasta: self.pasta.clone(),
                fotolivro: self.modo == Modo::Fotolivro,
                so_levadas: self.so_levadas,
                formato: self.formato.extensao().into(),
                qualidade: self.qualidade(cx),
            },
        );
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

    /// Drena os dois canais. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            // 🔑 O seletor responde sempre, inclusive "desisti" — sem esse
            // recado a tela esperaria para sempre uma pasta que nunca vem.
            self.esperando_pasta = false;
            if let Recado::DestinoEscolhido(caminho) = recado {
                self.pasta = Some(PathBuf::from(caminho));
                self.lembrar(cx);
            }
        }

        let estava_correndo = self.correndo();
        while let Ok(andamento) = self.andamentos.1.try_recv() {
            mudou = true;
            self.anotar(andamento);
        }
        if estava_correndo && !self.correndo() {
            let falhou = self.progresso.is_some_and(|p| p.falhas > 0);
            cx.emit(PedidoDaExportacao::Terminou {
                texto: self.resumo(),
                falhou,
            });
        }

        if mudou {
            cx.notify();
        }

        let continua = self.esperando_pasta || self.correndo();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn anotar(&mut self, andamento: Andamento) {
        let Some(progresso) = self.progresso.as_mut() else {
            return;
        };

        match andamento {
            Andamento::Comecou { total } => progresso.total = total,
            Andamento::Feita { destino } => {
                progresso.feitas += 1;
                self.ultimo = Some(nome(&destino).into());
                self.feitas.push(destino);
            }
            Andamento::Falhou { destino, erro } => {
                progresso.falhas += 1;
                // ⚠️ A falha de uma foto não interrompe o lote, mas não pode
                // sumir: cada uma fica na lista, com o porquê.
                self.falhas.push(Falha {
                    nome: nome(&destino),
                    erro,
                });
            }
            Andamento::Livro { destino } => {
                self.ultimo = Some(nome(&destino).into());
                self.arquivo_do_livro = Some(destino);
            }
            Andamento::Terminou {
                sucesso,
                falhas,
                canceladas,
            } => {
                progresso.feitas = sucesso;
                progresso.falhas = falhas;
                progresso.canceladas = canceladas;
                progresso.terminou = true;
            }
        }
    }

    /// O que aconteceu, em uma linha.
    pub fn resumo(&self) -> String {
        let fotos = |n: usize| if n == 1 { "foto" } else { "fotos" };
        match self.progresso {
            Some(p) if p.terminou => {
                let mut partes = vec![format!(
                    "{} {} {}",
                    p.feitas,
                    fotos(p.feitas),
                    if p.feitas == 1 {
                        "exportada"
                    } else {
                        "exportadas"
                    }
                )];
                if p.falhas > 0 {
                    partes.push(format!(
                        "{} {}",
                        p.falhas,
                        if p.falhas == 1 { "falhou" } else { "falharam" }
                    ));
                }
                if p.canceladas > 0 {
                    partes.push(format!(
                        "{} {}",
                        p.canceladas,
                        if p.canceladas == 1 {
                            "parada"
                        } else {
                            "paradas"
                        }
                    ));
                }
                partes.join(" · ")
            }
            Some(p) => format!("{} de {}", p.feitas + p.falhas, p.total),
            None if self.fotos.is_empty() => "nenhuma foto selecionada".to_string(),
            None => format!("{} {}", self.fotos.len(), fotos(self.fotos.len())),
        }
    }

    /// O título do modal: "Exportar 16 fotos".
    pub fn titulo(&self) -> String {
        match self.fotos.len() {
            1 => "Exportar 1 foto".into(),
            n => format!("Exportar {n} fotos"),
        }
    }

    fn mostrar_na_pasta(&self, cx: &mut Context<Self>) {
        match self.arquivo_do_livro.as_ref().or(self.feitas.first()) {
            Some(arquivo) => cx.reveal_path(arquivo),
            None => {
                if let Some(pasta) = &self.pasta {
                    crate::bandeja::abrir_pasta(pasta);
                }
            }
        }
    }

    /// O rodapé do diálogo (`AlertDialogFooter`): os botões mudam com o
    /// momento do lote — antes, durante e depois.
    pub fn rodape(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let faixa =
            crate::dialogo::rodape_da_pergunta(cx).debug_selector(|| "exportacao-rodape".into());
        let fechar = cx.listener(|_, _: &gpui_kit::ClickEvent, _, cx| {
            cx.emit(PedidoDaExportacao::Fechar);
        });

        match self.progresso {
            Some(p) if !p.terminou => {
                let parando = self.parando();
                faixa
                    .child(
                        estilo::botao_contorno("exportacao-parar", cx)
                            .debug_selector(|| "exportacao-parar".into())
                            .label(if parando { "Parando…" } else { "Parar" })
                            .disabled(parando)
                            .on_click(cx.listener(|tela, _, _, cx| tela.parar(cx))),
                    )
                    .child(
                        estilo::botao_primario("exportacao-exportar", cx)
                            .debug_selector(|| "exportacao-exportar".into())
                            .label(format!(
                                "Exportando {}/{}…",
                                (p.feitas + p.falhas + 1).min(p.total),
                                p.total
                            ))
                            .loading(true)
                            .disabled(true),
                    )
                    .into_any_element()
            }
            Some(p) => faixa
                // 📖 O livro pronto se vê aqui mesmo, antes de ir ao cliente.
                .when_some(self.arquivo_do_livro.clone(), |f, livro| {
                    f.child(
                        estilo::botao_contorno("exportacao-ver-livro", cx)
                            .debug_selector(|| "exportacao-ver-livro".into())
                            .label("Ver o fotolivro")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(PedidoDaExportacao::VerLivro(livro.clone()));
                            })),
                    )
                })
                .when(p.feitas > 0, |f| {
                    f.child(
                        estilo::botao_contorno("exportacao-mostrar", cx)
                            .debug_selector(|| "exportacao-mostrar".into())
                            .icon(Icon::new(Icone::FolderOpen))
                            .label("Mostrar na pasta")
                            .on_click(cx.listener(|tela, _, _, cx| tela.mostrar_na_pasta(cx))),
                    )
                })
                .when(p.falhas > 0, |f| {
                    f.child(
                        estilo::botao_contorno("exportacao-de-novo", cx)
                            .debug_selector(|| "exportacao-de-novo".into())
                            .label(match p.falhas {
                                1 => "Tentar a que falhou".to_string(),
                                n => format!("Tentar as {n} de novo"),
                            })
                            .on_click(cx.listener(|tela, _, _, cx| tela.tentar_de_novo(cx))),
                    )
                })
                .child(
                    estilo::botao_primario("exportacao-concluir", cx)
                        .debug_selector(|| "exportacao-concluir".into())
                        .label("Concluir")
                        .on_click(fechar),
                )
                .into_any_element(),
            None => {
                let pronto = self.pasta.is_some()
                    && !self.fotos.is_empty()
                    && (self.modo == Modo::Arquivos || !self.fotos_do_livro().is_empty());
                faixa
                    .child(
                        estilo::botao_contorno("exportacao-cancelar", cx)
                            .debug_selector(|| "exportacao-cancelar".into())
                            .label("Cancelar")
                            .on_click(fechar),
                    )
                    .child(
                        estilo::botao_primario("exportacao-exportar", cx)
                            .debug_selector(|| "exportacao-exportar".into())
                            .label(match self.modo {
                                Modo::Arquivos => "Exportar",
                                Modo::Fotolivro => "Gerar o fotolivro",
                            })
                            .disabled(!pronto)
                            .on_click(cx.listener(|tela, _, _, cx| tela.exportar(cx))),
                    )
                    .into_any_element()
            }
        }
    }

    fn rotulo_de_secao(texto: &'static str) -> gpui_kit::Div {
        div().text_sm().font_weight(FontWeight::MEDIUM).child(texto)
    }

    /// Uma linha do bloco "o que sai": o ícone, a contagem e, à direita, o
    /// que aquelas fotos levam.
    fn linha_do_que_sai(
        icone: Icon,
        quantas: String,
        leva: String,
        apagada: bool,
        cx: &Context<Self>,
    ) -> gpui_kit::Div {
        h_flex()
            .gap(px(8.))
            .when(apagada, |d| d.opacity(0.5))
            .child(icone.size(px(16.)).flex_none())
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(quantas),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_xs()
                    .text_right()
                    .text_color(cx.theme().muted_foreground)
                    .child(leva),
            )
    }

    /// O respiro de um `GroupBox` de linhas: o do kit (16) é o de um
    /// formulário, e aqui o bloco tem duas ou três linhas curtas.
    fn miolo_da_caixa() -> StyleRefinement {
        StyleRefinement::default().p(px(12.)).gap(px(10.))
    }

    /// 🗂️ O uso: arquivos soltos ou o fotolivro — uma escolha só, de ponta a
    /// ponta, com o que ela entrega logo embaixo.
    fn render_uso(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let usos = [Modo::Arquivos, Modo::Fotolivro];
        v_flex()
            .gap(px(8.))
            .child(
                TabBar::new("exportacao-uso")
                    .segmented()
                    .w_full()
                    .selected_index(usos.iter().position(|m| *m == self.modo).unwrap_or(0))
                    .children(usos.map(|modo| {
                        let id = match modo {
                            Modo::Arquivos => "exportacao-modo-arquivos",
                            Modo::Fotolivro => "exportacao-modo-fotolivro",
                        };
                        Tab::new()
                            .label(modo.rotulo())
                            .flex_1()
                            .debug_selector(move || id.to_string())
                    }))
                    .on_click(cx.listener(move |tela, i: &usize, _, cx| {
                        if let Some(modo) = usos.get(*i) {
                            tela.escolher_modo(*modo, cx);
                        }
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.modo.explicacao()),
            )
    }

    /// 📦 O que sai: as levadas, as à venda e o que cada grupo leva.
    ///
    /// 📐 Sempre as duas linhas, mesmo com zero de um lado (apagada): a caixa
    /// não muda de altura de uma seleção para a outra.
    fn render_o_que_sai(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let (levadas, a_venda) = self.contagem();
        let no_livro = self.modo == Modo::Fotolivro;
        let (leva_a_levada, leva_a_venda) = o_que_sai(self.modo, self.formato(), self.so_levadas);

        div()
            .debug_selector(|| "exportacao-o-que-sai".into())
            .child(
                GroupBox::new()
                    .outline()
                    .content_style(Self::miolo_da_caixa())
                    .child(Self::linha_do_que_sai(
                        Icon::new(Icone::CircleCheck).text_color(tema.success),
                        quantas_levadas(levadas),
                        leva_a_levada,
                        levadas == 0,
                        cx,
                    ))
                    .child(Self::linha_do_que_sai(
                        Icon::new(Icone::Droplet).text_color(tema.muted_foreground),
                        format!("{a_venda} à venda"),
                        leva_a_venda,
                        a_venda == 0 || (no_livro && self.so_levadas),
                        cx,
                    )),
            )
    }

    /// 📖 A opção do fotolivro: o do cliente, só com as dele, ou o de vender.
    ///
    /// 📐 A linha do aviso existe sempre (invisível sem aviso): o livro vazio
    /// não empurra o "Salvar em" para baixo.
    fn render_livro(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let tema = cx.theme().clone();
        let (levadas, _) = self.contagem();
        // O livro só das levadas, sem nenhuma levada: o botão fica desligado,
        // e a tela diz por quê.
        let livro_vazio = self.so_levadas && levadas == 0;

        v_flex()
            .gap(px(8.))
            .child(Self::rotulo_de_secao("Conteúdo do livro"))
            .child(
                GroupBox::new()
                    .outline()
                    .content_style(Self::miolo_da_caixa())
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Só as fotos levadas"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(tema.muted_foreground)
                                            .child("O livro do cliente, sem as fotos à venda."),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .debug_selector(|| "exportacao-so-levadas".into())
                                    .child(
                                        Switch::new("exportacao-so-levadas")
                                            .checked(self.so_levadas)
                                            .accessibility_label("Só as fotos levadas")
                                            .on_change(cx.listener(
                                                |tela, marcado: &bool, _, cx| {
                                                    tela.escolher_so_levadas(*marcado, cx)
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .debug_selector(|| "exportacao-livro-vazio".into())
                    .gap(px(6.))
                    .text_xs()
                    .text_color(tema.warning)
                    .when(!livro_vazio, |d| d.invisible())
                    .child(Icon::new(Icone::TriangleAlert).size(px(14.)).flex_none())
                    .child("Nenhuma das fotos escolhidas foi levada: o livro sairia vazio."),
            )
    }

    /// 🖼️ O formato das levadas e, no JPEG, a qualidade: os do site.
    fn render_formato(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let tema = cx.theme().clone();
        let formatos = FormatoDeSaida::TODOS;
        let formato_atual = self.formato();
        let sem_perda = formato_atual.sem_perda();
        // 📐 A qualidade (JPEG) e a nota do tamanho (sem perda) dividem a
        // mesma linha: trocar de formato não mexe na altura.
        let qualidade = h_flex()
            .gap(px(12.))
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Qualidade"),
            )
            .child(div().flex_1().child(estilo::slider(&self.qualidade)))
            .child(
                div()
                    .w(px(28.))
                    .text_xs()
                    .text_right()
                    .child(self.qualidade(cx).to_string()),
            );
        let nota = div()
            .text_xs()
            .text_color(tema.muted_foreground)
            .child("O arquivo fica grande: uma foto de 24 MP passa de 70 MB.");

        v_flex()
            .gap(px(8.))
            .child(Self::rotulo_de_secao("Formato"))
            .child(
                TabBar::new("exportacao-formato")
                    .segmented()
                    .w_full()
                    .selected_index(
                        formatos
                            .iter()
                            .position(|f| *f == formato_atual)
                            .unwrap_or(0),
                    )
                    .children(formatos.map(|formato| {
                        let id = format!("exportacao-formato-{}", formato.extensao());
                        Tab::new()
                            .label(detalhe_do_formato(formato).0)
                            .flex_1()
                            .debug_selector(move || id.clone())
                    }))
                    .on_click(cx.listener(move |tela, i: &usize, _, cx| {
                        if let Some(formato) = formatos.get(*i) {
                            tela.escolher_formato(*formato, cx);
                        }
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(detalhe_do_formato(formato_atual).1),
            )
            .child(estilo::mesmo_lugar([
                (!sem_perda, qualidade.into_any_element()),
                (sem_perda, nota.into_any_element()),
            ]))
    }

    /// 📁 Para onde: a pasta à vista, o "Trocar…" e a volta aos Downloads —
    /// o "Salvar em" do site.
    fn render_destino(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let tema = cx.theme().clone();
        let downloads = preferencias::pasta_dos_downloads();
        let nos_downloads = self.pasta.as_deref() == Some(downloads.as_path());
        let sem_pasta = self.pasta.is_none();
        let pasta: SharedString = self
            .pasta
            .as_deref()
            .map(curto)
            .unwrap_or_else(|| "Escolha uma pasta".into())
            .into();

        v_flex()
            .gap(px(8.))
            .child(Self::rotulo_de_secao("Salvar em"))
            .child(
                div().debug_selector(|| "exportacao-destino".into()).child(
                    GroupBox::new()
                        .outline()
                        .content_style(StyleRefinement::default().pl(px(12.)).pr(px(8.)).py(px(8.)))
                        .child(
                            h_flex()
                                .gap(px(8.))
                                .child(
                                    Icon::new(Icone::FolderOpen)
                                        .size(px(16.))
                                        .flex_none()
                                        .text_color(tema.muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .text_sm()
                                        .when(sem_pasta, |d| d.text_color(tema.muted_foreground))
                                        .child(pasta),
                                )
                                .when(!nos_downloads, |d| {
                                    d.child(
                                        estilo::botao_icone_pequeno(
                                            "exportacao-limpar-pasta",
                                            Icone::RotateCcw,
                                        )
                                        .tooltip("Voltar a salvar na pasta Downloads")
                                        .on_click(
                                            cx.listener(|tela, _, _, cx| {
                                                tela.voltar_aos_downloads(cx)
                                            }),
                                        ),
                                    )
                                })
                                .child(
                                    estilo::botao_contorno_pequeno("exportacao-escolher-pasta", cx)
                                        .label("Trocar…")
                                        .on_click(
                                            cx.listener(|tela, _, _, cx| tela.escolher_pasta(cx)),
                                        ),
                                ),
                        ),
                ),
            )
    }

    fn render_configuracao(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let arquivos = self.modo == Modo::Arquivos;
        v_flex()
            .gap(px(16.))
            .child(self.render_uso(cx))
            .child(self.render_o_que_sai(cx))
            // 📐 A opção de cada uso, no mesmo lugar: trocar de aba não muda a
            // altura da caixa.
            .child(estilo::mesmo_lugar([
                (arquivos, self.render_formato(cx).into_any_element()),
                (!arquivos, self.render_livro(cx).into_any_element()),
            ]))
            .child(self.render_destino(cx))
            .into_any_element()
    }

    fn render_lote(&mut self, p: Progresso, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let destino = self.pasta.as_deref().map(curto).unwrap_or_default();
        let andamento = match &self.ultimo {
            Some(ultimo) if !p.terminou => format!("{} · {ultimo}", self.resumo()),
            _ if !p.terminou => format!("{} · preparando…", self.resumo()),
            _ => format!("{} de {} · em {destino}", p.feitas, p.total),
        };
        let falhou = p.terminou && p.falhas > 0;

        v_flex()
            .gap(px(12.))
            .when(p.terminou, |d| {
                d.child(estilo::aviso(
                    if falhou {
                        self.resumo()
                    } else {
                        format!("{} em {destino}", self.resumo())
                    },
                    falhou,
                    cx,
                ))
            })
            .child(
                v_flex()
                    .gap(px(6.))
                    .debug_selector(|| "exportacao-progresso".into())
                    .child(
                        Progress::new("exportacao-progresso")
                            .value(p.porcento())
                            .when(falhou, |b| b.color(tema.danger))
                            .h(px(6.)),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .text_xs()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_color(tema.muted_foreground)
                                    .child(andamento),
                            )
                            .when(p.falhas > 0 && !p.terminou, |d| {
                                d.child(div().text_color(tema.danger).child(format!(
                                    "{} {}",
                                    p.falhas,
                                    if p.falhas == 1 { "falhou" } else { "falharam" }
                                )))
                            }),
                    ),
            )
            .when(!self.falhas.is_empty(), |d| {
                d.child(
                    v_flex()
                        .id("exportacao-falhas")
                        .debug_selector(|| "exportacao-falhas".into())
                        .max_h(px(132.))
                        .overflow_y_scroll()
                        .gap(px(4.))
                        .rounded(crate::tema::canto(8.))
                        .border_1()
                        .border_color(tema.border)
                        .p(px(8.))
                        .children(self.falhas.iter().map(|f| {
                            h_flex()
                                .gap(px(8.))
                                .items_start()
                                .text_xs()
                                .child(
                                    Icon::new(Icone::CircleAlert)
                                        .size(px(14.))
                                        .text_color(tema.danger),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(f.nome.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_color(tema.muted_foreground)
                                                .child(f.erro.clone()),
                                        ),
                                )
                        })),
                )
            })
            .into_any_element()
    }
}

/// O caminho como se lê: a pasta pessoal vira `~`.
fn curto(caminho: &Path) -> String {
    let texto = caminho.to_string_lossy().to_string();
    match directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
        Some(casa) => match caminho.strip_prefix(&casa) {
            Ok(resto) if resto.as_os_str().is_empty() => "~".into(),
            Ok(resto) => format!("~/{}", resto.to_string_lossy()),
            Err(_) => texto,
        },
        None => texto,
    }
}

fn nome(caminho: &Path) -> String {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

impl gpui_kit::Render for Exportacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 🧹 Sem frase solta no alto: o que o uso entrega está embaixo das
        // abas, e durante o lote a barra já diz o que está acontecendo.
        //
        // 📐 O lote divide o lugar com a configuração (invisível por baixo):
        // apertar "Exportar" não encolhe a caixa. Ele fica no meio dela.
        let lote = self.progresso.map(|p| {
            v_flex()
                .flex_1()
                .justify_center()
                .child(self.render_lote(p, cx))
                .into_any_element()
        });
        let configurando = lote.is_none();
        estilo::mesmo_lugar(
            [(configurando, self.render_configuracao(cx))]
                .into_iter()
                .chain(lote.map(|lote| (true, lote))),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::exportacao::porta::mentira::ExportadorDeMentira;
    use crate::importacao::explorador::mentira::SeletorDeMentira;
    use gpui_kit::TestAppContext;

    fn foto_do_site(id: &str, arquivo: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("{}{id}", persistencia::PREFIXO_DO_SITE),
            name: arquivo.into(),
            pos_venda_foto_id: Some(id.into()),
            // Levada: sai revelada, no formato escolhido.
            comprada: true,
            ..Default::default()
        }
    }

    fn montar(
        cx: &mut TestAppContext,
        exportador: Arc<ExportadorDeMentira>,
        seletor: Arc<SeletorDeMentira>,
    ) -> Entity<Exportacao> {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.new(|cx| Exportacao::nova(exportador, seletor, cx)))
    }

    /// 🚨 **O print**: 16 fotos da sessão, 16 "Photo ID inválido", e os nomes
    /// `foto`…`foto-16`. A foto do site vai pelo site, com o nome da câmera.
    #[gpui_kit::test]
    fn a_foto_da_sessao_vai_pelo_site_com_o_nome_da_camera(cx: &mut TestAppContext) {
        let exportador = Arc::new(ExportadorDeMentira::default());
        let tela = montar(
            cx,
            exportador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        let pasta = tempfile::tempdir().unwrap();
        let local = PathBuf::from("/cartao/IMG_2.CR3");

        tela.update(cx, |tela, cx| {
            tela.abrir_para(
                vec![
                    foto_do_site("a", "IMG_1.CR3"),
                    foto_do_site("b", "IMG_2.CR3"),
                ],
                None,
                HashMap::from([("b".to_string(), local.clone())]),
                cx,
            );
            tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
            tela.exportar(cx);
        });

        let lote = &exportador.pedidos()[0];
        assert_eq!(lote[0].destino, pasta.path().join("IMG_1.jpg"));
        assert_eq!(lote[1].destino, pasta.path().join("IMG_2.jpg"));
        match &lote[0].origem {
            Origem::Site {
                foto_no_site,
                original_local,
                ..
            } => {
                assert_eq!(foto_no_site, "a");
                assert_eq!(*original_local, None);
            }
            outra => panic!("a foto do site foi ao catálogo: {outra:?}"),
        }
        assert!(matches!(&lote[1].origem,
            Origem::Site { original_local: Some(p), .. } if *p == local));
    }

    /// 💧 A foto que subiu daqui (está no catálogo **e** no site) e não foi
    /// levada sai marcada, pelo site — e não limpa, pelo catálogo.
    #[gpui_kit::test]
    fn a_nao_levada_que_subiu_daqui_sai_marcada(cx: &mut TestAppContext) {
        let exportador = Arc::new(ExportadorDeMentira::default());
        let tela = montar(
            cx,
            exportador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        let pasta = tempfile::tempdir().unwrap();
        let daqui = |id: &str, comprada: bool| PhotoViewModel {
            id: format!("uuid-{id}"),
            name: format!("{id}.jpg"),
            path: format!("/cartao/{id}.jpg"),
            pos_venda_foto_id: Some(id.into()),
            comprada,
            ..Default::default()
        };
        let solta = PhotoViewModel {
            id: "uuid-solta".into(),
            name: "solta.jpg".into(),
            path: "/cartao/solta.jpg".into(),
            ..Default::default()
        };
        tela.update(cx, |tela, cx| {
            tela.abrir_para(
                vec![daqui("a", false), daqui("b", true), solta],
                None,
                HashMap::new(),
                cx,
            );
            assert_eq!(tela.contagem(), (2, 1));
            tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
            tela.escolher_modo(Modo::Arquivos, cx);
            tela.exportar(cx);
        });
        let lote = &exportador.pedidos()[0];
        assert!(
            matches!(&lote[0].origem, Origem::Site { levada: false, foto_no_site, .. } if foto_no_site == "a")
        );
        assert!(
            matches!(&lote[1].origem, Origem::Catalogo { .. }),
            "a levada daqui vai pelo catálogo, limpa"
        );
        assert!(
            matches!(&lote[2].origem, Origem::Catalogo { .. }),
            "a que nunca subiu também"
        );
    }

    /// O formato escolhido muda a extensão e chega à porta.
    #[gpui_kit::test]
    fn o_formato_escolhido_chega_ao_arquivo(cx: &mut TestAppContext) {
        let exportador = Arc::new(ExportadorDeMentira::default());
        let tela = montar(
            cx,
            exportador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        let pasta = tempfile::tempdir().unwrap();

        tela.update(cx, |tela, cx| {
            tela.abrir_para(
                vec![foto_do_site("a", "IMG_1.CR3")],
                None,
                HashMap::new(),
                cx,
            );
            tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
            tela.escolher_modo(Modo::Arquivos, cx);
            tela.escolher_formato(FormatoDeSaida::Tiff, cx);
            tela.exportar(cx);
        });
        assert_eq!(
            exportador.pedidos()[0][0].destino,
            pasta.path().join("IMG_1.tif")
        );
        let opcoes = exportador.opcoes.lock().unwrap().clone().unwrap();
        assert_eq!(opcoes.formato(), FormatoDeSaida::Tiff);
    }

    /// A contagem diz quantas saem limpas e quantas com a marca, e o dia vira
    /// a data por extenso da capa.
    #[test]
    fn o_que_sai_e_a_data_por_extenso() {
        assert_eq!(quantas_levadas(1), "1 levada");
        assert_eq!(quantas_levadas(5), "5 levadas");
        // 💧 A à venda é sempre JPEG: o formato só aparece nela quando o das
        // levadas é outro.
        assert_eq!(
            o_que_sai(Modo::Arquivos, FormatoDeSaida::Jpeg, false),
            (
                "sem marca d'água, em JPEG".to_string(),
                "com a marca d'água do site".to_string()
            )
        );
        assert_eq!(
            o_que_sai(Modo::Arquivos, FormatoDeSaida::Tiff, false).1,
            "com a marca d'água, em JPEG"
        );
        assert_eq!(
            o_que_sai(Modo::Fotolivro, FormatoDeSaida::Jpeg, true).1,
            "ficam fora do livro"
        );
        assert_eq!(data_por_extenso("2026-10-09"), "9 de outubro de 2026");
        assert_eq!(
            data_por_extenso("2026-10-09T12:00:00Z"),
            "9 de outubro de 2026"
        );
        assert_eq!(data_por_extenso(""), "");
    }

    /// Durante o lote: a barra anda e "Parar" deixa de começar as outras.
    #[gpui_kit::test]
    fn parar_no_meio_conta_as_paradas(cx: &mut TestAppContext) {
        let exportador = Arc::new(ExportadorDeMentira::default());
        *exportador.segurar.lock().unwrap() = true;
        let tela = montar(
            cx,
            exportador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        let pasta = tempfile::tempdir().unwrap();

        tela.update(cx, |tela, cx| {
            tela.abrir_para(
                vec![foto_do_site("a", "A.jpg"), foto_do_site("b", "B.jpg")],
                None,
                HashMap::new(),
                cx,
            );
            tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
            tela.exportar(cx);
            tela.colher(cx);
            assert!(tela.correndo());
            assert_eq!(tela.progresso().unwrap().porcento(), 0.);
            tela.parar(cx);
            assert!(tela.parando());
        });
        exportador.soltar();
        tela.update(cx, |tela, cx| {
            tela.colher(cx);
            let p = tela.progresso().unwrap();
            assert!(p.terminou);
            assert_eq!((p.feitas, p.canceladas), (0, 2));
            assert_eq!(tela.resumo(), "0 fotos exportadas · 2 paradas");
        });
    }

    /// Cada falha fica na lista, com o nome, e "tentar de novo" manda só elas.
    #[gpui_kit::test]
    fn as_falhas_ficam_listadas_e_podem_ser_tentadas_de_novo(cx: &mut TestAppContext) {
        let exportador = Arc::new(ExportadorDeMentira::default());
        *exportador.falham.lock().unwrap() = 1;
        let tela = montar(
            cx,
            exportador.clone(),
            Arc::new(SeletorDeMentira::default()),
        );
        let pasta = tempfile::tempdir().unwrap();

        tela.update(cx, |tela, cx| {
            tela.abrir_para(
                vec![foto_do_site("a", "A.jpg"), foto_do_site("b", "B.jpg")],
                None,
                HashMap::new(),
                cx,
            );
            tela.escolher_pasta_para_teste(pasta.path().to_path_buf(), cx);
            tela.exportar(cx);
            tela.colher(cx);
            assert_eq!(tela.falhas().len(), 1);
            assert_eq!(tela.falhas()[0].nome, "A.jpg");
            assert_eq!(tela.resumo(), "1 foto exportada · 1 falhou");
        });

        *exportador.falham.lock().unwrap() = 0;
        tela.update(cx, |tela, cx| {
            tela.tentar_de_novo(cx);
            tela.colher(cx);
        });
        let pedidos = exportador.pedidos();
        assert_eq!(pedidos.len(), 2);
        assert_eq!(pedidos[1].len(), 1, "só a que falhou vai de novo");
        assert_eq!(pedidos[1][0].destino, pasta.path().join("A.jpg"));
    }

    /// A pasta, o uso, o formato e a marca voltam na próxima abertura.
    #[gpui_kit::test]
    fn as_escolhas_sao_lembradas(cx: &mut TestAppContext) {
        let pasta = tempfile::tempdir().unwrap();
        let tela = montar(
            cx,
            Arc::new(ExportadorDeMentira::default()),
            Arc::new(SeletorDeMentira::escolhe(pasta.path().to_str().unwrap())),
        );
        tela.update(cx, |tela, cx| {
            tela.escolher_formato(FormatoDeSaida::Png, cx);
            tela.escolher_pasta(cx);
            tela.colher(cx);
        });
        let outra = cx.update(|cx| {
            cx.new(|cx| {
                Exportacao::nova(
                    Arc::new(ExportadorDeMentira::default()),
                    Arc::new(SeletorDeMentira::default()),
                    cx,
                )
            })
        });
        outra.update(cx, |tela, _| {
            assert_eq!(tela.formato(), FormatoDeSaida::Png);
            assert_eq!(tela.pasta(), Some(&pasta.path().to_path_buf()));
        });
    }
}
