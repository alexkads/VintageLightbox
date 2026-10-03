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
//! ## Os dois usos
//!
//! "Entrega final" e "Prévia da galeria" continuam sendo a decisão da frente:
//! entregar contra mostrar. A prévia não sai sem marca d'água — ver
//! [`Exportacao::opcoes`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui_kit::component::button::ButtonGroup;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::radio::Radio;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::{
    div, prelude::*, px, AnyElement, Context, Entity, EventEmitter, FontWeight, SharedString,
    Subscription, Task, Window,
};

use domain::services::pos_venda::Sessao;
use domain::value_objects::{
    ExportOptions, FilePath, FormatoDeSaida, Watermark, WatermarkPosition,
};

use crate::estilo;
use crate::importacao::estado::Recado;
use crate::importacao::explorador::SeletorDePasta;
use crate::recursos::Icone;
use crate::revelacao::persistencia;

use super::destino::Destinos;
use super::porta::{Andamento, Exportador, Origem, Saida};
use super::preferencias::{self, Preferencias};

/// O mesmo intervalo da colheita da importação.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// Os dois desfechos de uma exportação neste estúdio.
///
/// 🔑 **São dois botões, e não dois preenchimentos do mesmo formulário.** A
/// diferença entre eles não é de configuração: é *entregar* contra *mostrar*, e
/// é a decisão que o fotógrafo já toma na triagem — esta foi comprada, esta
/// ficou para trás.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    /// O que o cliente comprou: tamanho original, sem marca.
    #[default]
    Entrega,
    /// O que ficou para trás: reduzida e marcada, para a galeria.
    Previa,
}

impl Modo {
    pub fn rotulo(self) -> &'static str {
        match self {
            Modo::Entrega => "Entrega final",
            Modo::Previa => "Prévia da galeria",
        }
    }

    pub fn explicacao(self) -> &'static str {
        match self {
            Modo::Entrega => "Tamanho original, sem marca d'água — o que o cliente comprou.",
            Modo::Previa => {
                "Lado maior de 2048 px, com a marca d'água no centro — para mostrar sem entregar."
            }
        }
    }
}

/// O que cada formato é, com as palavras do site (`exportar.ts`).
fn detalhe_do_formato(formato: FormatoDeSaida) -> (&'static str, &'static str) {
    match formato {
        FormatoDeSaida::Jpeg => ("JPEG", "o de sempre — menor arquivo"),
        FormatoDeSaida::Png => ("PNG", "sem perda, para reeditar"),
        FormatoDeSaida::Tiff => ("TIFF", "sem perda, o que o lab aceita"),
        FormatoDeSaida::Webp => ("WebP", "sem perda, para web"),
    }
}

/// O lado maior da prévia. 2048 px é o que uma galeria mostra em tela cheia num
/// monitor comum, e é pequeno o bastante para não servir de entrega.
const LADO_DA_PREVIA: u32 = 2048;

/// As extensões aceitas para a marca d'água.
const IMAGENS_DE_MARCA: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

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
    /// O arquivo da marca d'água — um PNG com transparência, o logotipo do
    /// estúdio. Lembrado entre exportações e entre aberturas do app.
    marca: Option<PathBuf>,
    /// A marca escolhida não era imagem.
    aviso_da_marca: Option<SharedString>,
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
    /// Para qual campo a resposta do seletor vai. Sem isto, escolher a marca
    /// d'água mudaria a pasta de destino.
    escolhendo_marca: bool,
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
            marca: lembradas.marca.clone(),
            aviso_da_marca: None,
            progresso: None,
            ultimo: None,
            falhas: Vec::new(),
            lote: Vec::new(),
            feitas: Vec::new(),
            cancelar: Arc::new(AtomicBool::new(false)),
            andamentos: channel(),
            recados: channel(),
            esperando_pasta: false,
            escolhendo_marca: false,
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
        self.aviso_da_marca = None;
        cx.notify();
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

    pub fn formato(&self) -> FormatoDeSaida {
        // 🔑 A prévia vai sempre em JPEG: é para a galeria, e um TIFF de 2048
        // px marcado não serve a ninguém.
        match self.modo {
            Modo::Previa => FormatoDeSaida::Jpeg,
            Modo::Entrega => self.formato,
        }
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

    pub fn marca(&self) -> Option<&PathBuf> {
        self.marca.as_ref()
    }

    /// As opções que o modo atual pede.
    ///
    /// 🚨 **A prévia sem marca escolhida devolve `None`, e o botão fica
    /// desligado por causa disso.** Exportar prévia sem marca produziria
    /// exatamente o arquivo que não pode existir: a foto não comprada, legível,
    /// na galeria. Um `unwrap_or_default` aqui seria o defeito mais caro do
    /// aplicativo.
    pub fn opcoes_com(&self, qualidade: u8) -> Option<ExportOptions> {
        let base = ExportOptions::default()
            .with_quality(qualidade)
            .with_formato(self.formato());
        match self.modo {
            Modo::Entrega => Some(base),
            Modo::Previa => {
                let marca = self.marca.as_ref()?;
                let arquivo = FilePath::new(marca.to_str()?).ok()?;
                Some(
                    base.with_longest_edge(LADO_DA_PREVIA)
                        .with_watermark(Watermark::new(
                            arquivo,
                            WatermarkPosition::Center,
                            0.35,
                            0.55,
                        )),
                )
            }
        }
    }

    pub fn opcoes(&self, cx: &gpui_kit::App) -> Option<ExportOptions> {
        self.opcoes_com(self.qualidade(cx))
    }

    /// A marca d'água, sem passar pelo seletor nativo.
    #[cfg(test)]
    pub fn escolher_marca_para_teste(&mut self, marca: PathBuf, cx: &mut Context<Self>) {
        self.marca = Some(marca);
        cx.notify();
    }

    pub fn progresso(&self) -> Option<Progresso> {
        self.progresso
    }

    /// Abre o seletor nativo de pasta.
    pub fn escolher_pasta(&mut self, cx: &mut Context<Self>) {
        self.esperando_pasta = true;
        self.escolhendo_marca = false;
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

    /// Abre o seletor nativo de **arquivo** para a marca d'água.
    pub fn escolher_marca(&mut self, cx: &mut Context<Self>) {
        self.esperando_pasta = true;
        self.escolhendo_marca = true;
        self.seletor.escolher_arquivo(self.recados.0.clone(), cx);
        self.acompanhar(cx);
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
    fn origem_de(&self, foto: &PhotoViewModel) -> Origem {
        match foto.pos_venda_foto_id.as_ref() {
            Some(no_site) if persistencia::so_existe_no_site(foto) => Origem::Site {
                foto_no_site: no_site.clone(),
                ajustes: persistencia::da_foto(foto),
                corte: persistencia::para_crop_settings(&persistencia::corte_da_foto(foto)),
                original_local: self.copias_locais.get(no_site).cloned(),
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
        if self.fotos.is_empty() || self.correndo() || self.opcoes(cx).is_none() {
            // Prévia sem marca escolhida. Ver `opcoes`.
            return;
        }

        let mut destinos = Destinos::na_pasta(pasta);
        let extensao = self.formato().extensao();
        let saidas: Vec<Saida> = self
            .fotos
            .iter()
            .map(|foto| Saida {
                origem: self.origem_de(foto),
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
            })
            .collect();
        self.mandar(saidas, cx);
    }

    /// Manda de novo só as que falharam, com os mesmos nomes.
    pub fn tentar_de_novo(&mut self, cx: &mut Context<Self>) {
        if self.correndo() {
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
        let Some(opcoes) = self.opcoes(cx) else {
            return;
        };
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
                marca: self.marca.clone(),
                previa: self.modo == Modo::Previa,
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
            match recado {
                Recado::DestinoEscolhido(caminho) if !self.escolhendo_marca => {
                    self.pasta = Some(PathBuf::from(caminho));
                    self.lembrar(cx);
                }
                Recado::ArquivoEscolhido(caminho) if self.escolhendo_marca => {
                    let caminho = PathBuf::from(caminho);
                    if e_imagem(&caminho) {
                        self.marca = Some(caminho);
                        self.aviso_da_marca = None;
                        self.lembrar(cx);
                    } else {
                        self.aviso_da_marca =
                            Some("A marca d'água tem de ser uma imagem PNG ou JPEG.".into());
                    }
                }
                _ => {}
            }
            self.escolhendo_marca = false;
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
        match self.feitas.first() {
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
                let pronto =
                    self.pasta.is_some() && !self.fotos.is_empty() && self.opcoes(cx).is_some();
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
                            .label("Exportar")
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

    /// Uma caixa de "onde": ícone, rótulo, o caminho e os botões — o
    /// "Salvar em" do site.
    fn caixa_de_caminho(
        icone: Icone,
        rotulo: &'static str,
        valor: SharedString,
        apagado: bool,
        cx: &Context<Self>,
    ) -> gpui_kit::Div {
        let tema = cx.theme();
        h_flex()
            .gap(px(8.))
            .px(px(10.))
            .py(px(6.))
            .rounded(crate::tema::canto(8.))
            .border_1()
            .border_color(tema.border)
            .bg(tema.muted.opacity(0.4))
            .text_xs()
            .child(
                Icon::new(icone)
                    .size(px(16.))
                    .text_color(tema.muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(
                        gpui_kit::div()
                            .text_color(tema.muted_foreground)
                            .child(rotulo),
                    )
                    .child(
                        div()
                            .when(apagado, |d| d.text_color(tema.muted_foreground))
                            .when(!apagado, |d| d.text_color(tema.foreground))
                            .child(valor),
                    ),
            )
    }

    fn render_configuracao(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let downloads = preferencias::pasta_dos_downloads();
        let pasta: SharedString = self
            .pasta
            .as_deref()
            .map(curto)
            .unwrap_or_else(|| "escolha uma pasta".into())
            .into();
        let nos_downloads = self.pasta.as_deref() == Some(downloads.as_path());
        let formato_atual = self.formato();
        let qualidade = self.qualidade(cx);

        v_flex()
            .gap(px(16.))
            // Uso: entregar ou mostrar.
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(Self::rotulo_de_secao("Uso"))
                    .child(
                        ButtonGroup::new("exportacao-uso")
                            .outline()
                            .xsmall()
                            .children([Modo::Entrega, Modo::Previa].map(|modo| {
                                estilo::botao_contorno_pequeno(
                                    match modo {
                                        Modo::Entrega => "exportacao-modo-entrega",
                                        Modo::Previa => "exportacao-modo-previa",
                                    },
                                    cx,
                                )
                                .label(modo.rotulo())
                                .selected(self.modo == modo)
                            }))
                            .on_click(cx.listener(|tela, cliques: &Vec<usize>, _, cx| {
                                let modo = if cliques.contains(&1) {
                                    Modo::Previa
                                } else {
                                    Modo::Entrega
                                };
                                tela.escolher_modo(modo, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(tema.muted_foreground)
                            .child(self.modo.explicacao()),
                    ),
            )
            // Formato e qualidade: os do site.
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(Self::rotulo_de_secao("Formato"))
                    .children(FormatoDeSaida::TODOS.map(|formato| {
                        let (rotulo, detalhe) = detalhe_do_formato(formato);
                        let travado = self.modo == Modo::Previa && formato != FormatoDeSaida::Jpeg;
                        h_flex()
                            .gap(px(8.))
                            .child(
                                Radio::new(("exportacao-formato", formato as usize))
                                    .label(rotulo)
                                    .checked(formato_atual == formato)
                                    .disabled(travado)
                                    .on_click(cx.listener(move |tela, _: &bool, _, cx| {
                                        tela.escolher_formato(formato, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(tema.muted_foreground)
                                    .child(detalhe),
                            )
                    }))
                    .when(self.modo == Modo::Previa, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(tema.muted_foreground)
                                .child("A prévia sai sempre em JPEG."),
                        )
                    })
                    .when(!formato_atual.sem_perda(), |d| {
                        d.child(
                            h_flex()
                                .gap(px(12.))
                                .pt(px(4.))
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
                                        .child(qualidade.to_string()),
                                ),
                        )
                    })
                    .when(formato_atual.sem_perda(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(tema.muted_foreground)
                                .child("Sem perda: o arquivo fica grande — uma foto de 24 MP passa de 70 MB."),
                        )
                    }),
            )
            // Para onde.
            .child(
                Self::caixa_de_caminho(Icone::FolderOpen, "Salvar em: ", pasta, false, cx)
                    .debug_selector(|| "exportacao-destino".into())
                    .child(
                        estilo::botao_fantasma_pequeno("exportacao-escolher-pasta", cx)
                            .label("Trocar…")
                            .on_click(cx.listener(|tela, _, _, cx| tela.escolher_pasta(cx))),
                    )
                    .when(!nos_downloads, |d| {
                        d.child(
                            estilo::botao_fantasma_pequeno("exportacao-limpar-pasta", cx)
                                .label("Limpar")
                                .tooltip("Voltar a salvar na pasta Downloads")
                                .on_click(cx.listener(|tela, _, _, cx| {
                                    tela.voltar_aos_downloads(cx)
                                })),
                        )
                    }),
            )
            // A marca d'água, só na prévia.
            .when(self.modo == Modo::Previa, |raiz| {
                let (texto, apagado): (SharedString, bool) = match &self.marca {
                    Some(m) => (nome(m).into(), false),
                    None => ("nenhuma escolhida".into(), true),
                };
                raiz.child(
                    Self::caixa_de_caminho(Icone::ImagePlus, "Marca d'água: ", texto, apagado, cx)
                        .debug_selector(|| "exportacao-marca".into())
                        .child(
                            estilo::botao_fantasma_pequeno("exportacao-escolher-marca", cx)
                                .label(if self.marca.is_some() {
                                    "Trocar…"
                                } else {
                                    "Escolher…"
                                })
                                .on_click(cx.listener(|tela, _, _, cx| tela.escolher_marca(cx))),
                        ),
                )
                .when(self.marca.is_none() || self.aviso_da_marca.is_some(), |raiz| {
                    raiz.child(estilo::aviso(
                        self.aviso_da_marca.clone().unwrap_or_else(|| {
                            "A prévia só sai com a marca d'água: escolha o logotipo do estúdio (PNG com transparência)."
                                .into()
                        }),
                        true,
                        cx,
                    ))
                })
            })
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

fn e_imagem(caminho: &Path) -> bool {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGENS_DE_MARCA.contains(&e.to_lowercase().as_str()))
}

fn nome(caminho: &Path) -> String {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

impl gpui_kit::Render for Exportacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let corpo = match self.progresso {
            Some(p) => self.render_lote(p, cx),
            None => self.render_configuracao(cx),
        };
        v_flex()
            .gap(px(16.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Os arquivos saem com os ajustes e o corte aplicados."),
            )
            .child(corpo)
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

    /// O formato escolhido muda a extensão e chega à porta; a prévia força JPEG.
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
            tela.escolher_modo(Modo::Entrega, cx);
            tela.escolher_formato(FormatoDeSaida::Tiff, cx);
            tela.exportar(cx);
        });
        assert_eq!(
            exportador.pedidos()[0][0].destino,
            pasta.path().join("IMG_1.tif")
        );
        let opcoes = exportador.opcoes.lock().unwrap().clone().unwrap();
        assert_eq!(opcoes.formato(), FormatoDeSaida::Tiff);

        tela.update(cx, |tela, cx| {
            tela.escolher_modo(Modo::Previa, cx);
            assert_eq!(tela.formato(), FormatoDeSaida::Jpeg, "a prévia vai em JPEG");
            tela.escolher_modo(Modo::Entrega, cx);
        });
    }

    /// 🚨 A marca d'água é um **arquivo**: o seletor certo, e só imagem vale.
    #[gpui_kit::test]
    fn a_marca_dagua_e_escolhida_como_arquivo(cx: &mut TestAppContext) {
        let seletor = Arc::new(SeletorDeMentira::escolhe("/logos/estudio.png"));
        let tela = montar(
            cx,
            Arc::new(ExportadorDeMentira::default()),
            seletor.clone(),
        );

        tela.update(cx, |tela, cx| {
            tela.escolher_modo(Modo::Previa, cx);
            tela.escolher_marca(cx);
            tela.colher(cx);
            assert_eq!(tela.marca(), Some(&PathBuf::from("/logos/estudio.png")));
            assert!(tela.opcoes(cx).is_some());
            tela.escolher_modo(Modo::Entrega, cx);
        });

        *seletor.escolha.lock().unwrap() = Some("/logos".into());
        tela.update(cx, |tela, cx| {
            tela.escolher_marca(cx);
            tela.colher(cx);
            assert_eq!(
                tela.marca(),
                Some(&PathBuf::from("/logos/estudio.png")),
                "uma pasta não pode virar a marca d'água"
            );
        });
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
