//! O modal de exportação: escolher a pasta e entregar os arquivos.
//!
//! 🚨 **É a primeira vez que este app produz um arquivo.** O `ExportPhotoUseCase`
//! e o `ExportController` existiam e estavam testados desde antes da migração,
//! sem nenhum caminho da tela até eles — e o app de egui também não tinha
//! (`docs/PARIDADE-UI.md` não menciona exportação em linha nenhuma). Importar,
//! organizar, triar e revelar sem poder entregar é um catálogo, não um editor.
//!
//! ## O que este incremento faz, e o que fica para o próximo
//!
//! Faz: exportar **a seleção** para uma pasta, em JPEG, com o nome vindo do
//! arquivo de origem e sem colisão dentro do lote.
//!
//! Não faz ainda, e é decisão de ordem — o Lightroom pede tudo isso: escolher
//! formato (TIFF/PNG/DNG), qualidade, espaço de cor, redimensionamento, nitidez
//! de saída, renomeação por padrão e marca d'água. Cada um deles muda a
//! assinatura de `ImageExporter::export`, que hoje grava JPEG 90 fixo; entram
//! juntos, num commit que mexe no `domain`.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui::{div, prelude::*, px, Context, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};

use domain::value_objects::{ExportOptions, FilePath, Watermark, WatermarkPosition};

use crate::importacao::estado::Recado;
use crate::importacao::explorador::SeletorDePasta;

use super::destino::Destinos;
use super::porta::{Andamento, Exportador, Saida};

/// O mesmo intervalo da colheita da importação.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// A extensão de saída. Fixa enquanto o formato não é escolha da tela — e
/// **declarada aqui, e não espalhada**, para o dia em que virar.
const EXTENSAO: &str = "jpg";

/// Os dois desfechos de uma exportação neste estúdio.
///
/// 🔑 **São dois botões, e não dois preenchimentos do mesmo formulário.** A
/// diferença entre eles não é de configuração: é *entregar* contra *mostrar*, e
/// é a decisão que o fotógrafo já toma na triagem — esta foi comprada, esta
/// ficou para trás. Um formulário com seis campos deixaria a escolha certa
/// depender de lembrar de seis coisas, e a foto não comprada iria inteira para a
/// galeria no dia em que alguém esquecesse uma delas.
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
            Modo::Entrega => "tamanho original, sem marca d'água",
            Modo::Previa => "lado maior 2048 px, com a marca d'água no centro",
        }
    }
}

/// O lado maior da prévia. 2048 px é o que uma galeria mostra em tela cheia num
/// monitor comum, e é pequeno o bastante para não servir de entrega.
const LADO_DA_PREVIA: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progresso {
    pub total: usize,
    pub feitas: usize,
    pub falhas: usize,
    pub terminou: bool,
}

pub struct Exportacao {
    exportador: Arc<dyn Exportador>,
    seletor: Arc<dyn SeletorDePasta>,
    /// As fotos que o lote vai exportar — copiadas da Biblioteca ao abrir.
    ///
    /// 🔑 **Copiadas, e não lidas da grade a cada quadro.** Um lote de 400 fotos
    /// que corre por um minuto não pode mudar de tamanho porque alguém clicou
    /// numa miniatura enquanto ele roda.
    fotos: Vec<PhotoViewModel>,
    pasta: Option<PathBuf>,
    modo: Modo,
    /// O arquivo da marca d'água — um PNG com transparência, o logotipo do
    /// estúdio. Guardado entre exportações: escolher o mesmo logotipo a cada
    /// lote é atrito puro.
    marca: Option<PathBuf>,
    progresso: Option<Progresso>,
    /// O último nome gravado, para a tela mostrar que algo está acontecendo.
    ultimo: Option<SharedString>,
    aviso: Option<SharedString>,
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    recados: (Sender<Recado>, Receiver<Recado>),
    esperando_pasta: bool,
    /// Para qual campo a resposta do seletor vai. Sem isto, escolher a marca
    /// d'água mudaria a pasta de destino — e o lote sairia dentro da pasta do
    /// logotipo.
    escolhendo_marca: bool,
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Exportacao {
    pub fn nova(exportador: Arc<dyn Exportador>, seletor: Arc<dyn SeletorDePasta>) -> Self {
        Self {
            exportador,
            seletor,
            fotos: Vec::new(),
            pasta: None,
            modo: Modo::default(),
            marca: None,
            progresso: None,
            ultimo: None,
            aviso: None,
            andamentos: channel(),
            recados: channel(),
            esperando_pasta: false,
            escolhendo_marca: false,
            colhendo: false,
            _colheita: None,
        }
    }

    /// Abre o modal para uma seleção.
    ///
    /// ⚠️ **O progresso do lote anterior é jogado fora aqui, e não ao fechar.**
    /// Fechar e reabrir para conferir "quantas saíram mesmo?" é a coisa mais
    /// natural do mundo, e zerar ao fechar tiraria a resposta justamente de quem
    /// voltou para lê-la.
    pub fn abrir_para(&mut self, fotos: Vec<PhotoViewModel>, cx: &mut Context<Self>) {
        self.fotos = fotos;
        self.progresso = None;
        self.ultimo = None;
        self.aviso = None;
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

    pub fn escolher_modo(&mut self, modo: Modo, cx: &mut Context<Self>) {
        self.modo = modo;
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
    pub fn opcoes(&self) -> Option<ExportOptions> {
        match self.modo {
            Modo::Entrega => Some(ExportOptions::default()),
            Modo::Previa => {
                let marca = self.marca.as_ref()?;
                let arquivo = FilePath::new(marca.to_str()?).ok()?;
                Some(
                    ExportOptions::default()
                        .with_longest_edge(LADO_DA_PREVIA)
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
        self.seletor.escolher_destino(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Abre o seletor nativo para o arquivo da marca d'água.
    ///
    /// ⚠️ **Reusa o seletor de pasta e trata a resposta como caminho de
    /// arquivo.** O `rfd` do projeto está montado para pasta; abrir um segundo
    /// diálogo, de arquivo, é uma porta nova — e enquanto ela não existe, apontar
    /// a pasta que **contém** o logotipo seria pedir à pessoa que confie que o
    /// app adivinha qual arquivo. Por isso o campo aceita o caminho e a tela diz
    /// qual arquivo está valendo.
    pub fn escolher_marca(&mut self, cx: &mut Context<Self>) {
        self.esperando_pasta = true;
        self.escolhendo_marca = true;
        self.seletor.escolher(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// A pasta, sem passar pelo seletor nativo.
    ///
    /// Existe porque nenhum teste pode fazer aparecer janela do sistema na
    /// máquina de quem roda a suíte — é a mesma razão de o seletor ser uma porta.
    #[cfg(test)]
    pub fn escolher_pasta_para_teste(&mut self, pasta: PathBuf, cx: &mut Context<Self>) {
        self.pasta = Some(pasta);
        cx.notify();
    }

    /// Dispara o lote.
    ///
    /// 🚨 **Os destinos são calculados aqui, de uma vez, com reserva de nome.**
    /// Escolher o nome de cada foto na hora de gravar deixaria duas fotos com o
    /// mesmo nome de origem apontando para o mesmo arquivo — o rodapé diria 40 e
    /// a pasta teria 39. Ver [`super::destino`].
    pub fn exportar(&mut self, cx: &mut Context<Self>) {
        let Some(pasta) = self.pasta.clone() else {
            return;
        };
        let Some(opcoes) = self.opcoes() else {
            // Prévia sem marca escolhida. Ver `opcoes`.
            return;
        };
        if self.fotos.is_empty() || self.correndo() {
            return;
        }

        let mut destinos = Destinos::na_pasta(pasta);
        let saidas: Vec<Saida> = self
            .fotos
            .iter()
            .map(|foto| Saida {
                id: foto.id.clone(),
                destino: destinos.para(&foto.path, EXTENSAO),
            })
            .collect();

        self.progresso = Some(Progresso {
            total: saidas.len(),
            ..Default::default()
        });
        self.ultimo = None;
        self.aviso = None;

        self.exportador
            .exportar(saidas, opcoes, self.andamentos.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Um lote em curso. É o que desliga o botão: dois cliques exportariam tudo
    /// duas vezes, e a segunda passada gravaria por cima da primeira.
    pub fn correndo(&self) -> bool {
        self.progresso.is_some_and(|p| !p.terminou)
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
            // recado a tela esperaria para sempre uma pasta que nunca vem, e o
            // laço acordaria a cada 100ms pelo resto da sessão.
            self.esperando_pasta = false;
            match recado {
                Recado::DestinoEscolhido(caminho) if !self.escolhendo_marca => {
                    self.pasta = Some(PathBuf::from(caminho));
                }
                Recado::OrigemEscolhida(caminho) if self.escolhendo_marca => {
                    self.marca = Some(PathBuf::from(caminho));
                }
                _ => {}
            }
            self.escolhendo_marca = false;
        }

        while let Ok(andamento) = self.andamentos.1.try_recv() {
            mudou = true;
            self.anotar(andamento);
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
            }
            Andamento::Falhou { destino, erro } => {
                progresso.falhas += 1;
                // ⚠️ A falha de uma foto não interrompe o lote, mas não pode
                // sumir: o aviso é o que diz que 39 de 40 saíram.
                self.aviso = Some(format!("{}: {erro}", nome(&destino)).into());
            }
            Andamento::Terminou { sucesso, falhas } => {
                progresso.feitas = sucesso;
                progresso.falhas = falhas;
                progresso.terminou = true;
            }
        }
    }

    /// O rodapé: o que aconteceu, em uma linha.
    pub fn resumo(&self) -> String {
        match self.progresso {
            Some(p) if p.terminou && p.falhas > 0 => {
                format!("{} exportadas · {} falharam", p.feitas, p.falhas)
            }
            Some(p) if p.terminou => format!("{} exportadas", p.feitas),
            Some(p) => format!("{} de {}…", p.feitas + p.falhas, p.total),
            None if self.fotos.is_empty() => "nenhuma foto selecionada".to_string(),
            None => format!("{} para exportar", self.fotos.len()),
        }
    }
}

fn nome(caminho: &std::path::Path) -> String {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

impl gpui::Render for Exportacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pasta: SharedString = match &self.pasta {
            Some(p) => p.to_string_lossy().to_string().into(),
            None => "escolha uma pasta".into(),
        };
        // 🚨 `opcoes()` é quem decide, e não um `&&` a mais aqui: no modo prévia
        // ele devolve `None` sem marca d'água escolhida, e o botão desligado é o
        // que impede a foto não comprada de ir legível para a galeria.
        let pronto = self.pasta.is_some()
            && !self.fotos.is_empty()
            && !self.correndo()
            && self.opcoes().is_some();

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .min_w(px(420.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("exportacao-escolher-pasta")
                            .label("Pasta de destino…")
                            .xsmall()
                            .disabled(self.correndo())
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.escolher_pasta(cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(pasta),
                    ),
            )
            .child(
                // Os dois modos, lado a lado. 🔑 A escolha é entre **entregar** e
                // **mostrar**, e é por isso que ela é um par de botões e não uma
                // caixa de opção perdida num formulário.
                div().flex().items_center().gap(px(6.)).children(
                    [Modo::Entrega, Modo::Previa].map(|modo| {
                        Button::new(match modo {
                            Modo::Entrega => "exportacao-modo-entrega",
                            Modo::Previa => "exportacao-modo-previa",
                        })
                        .label(modo.rotulo())
                        .xsmall()
                        .when(self.modo == modo, |b| b.primary())
                        .selected(self.modo == modo)
                        .disabled(self.correndo())
                        .on_click(cx.listener(
                            move |tela, _ev, _window, cx| {
                                tela.escolher_modo(modo, cx);
                            },
                        ))
                    }),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "JPEG qualidade 90 · {} · com a revelação e o enquadramento aplicados",
                        self.modo.explicacao()
                    )),
            )
            .when(self.modo == Modo::Previa, |raiz| {
                let marca: SharedString = match &self.marca {
                    Some(m) => m.to_string_lossy().to_string().into(),
                    None => "nenhuma escolhida — a prévia não sai sem ela".into(),
                };
                let falta = self.marca.is_none();
                raiz.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            Button::new("exportacao-escolher-marca")
                                .label("Marca d'água…")
                                .xsmall()
                                .disabled(self.correndo())
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.escolher_marca(cx);
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .truncate()
                                .when(falta, |d| d.text_color(cx.theme().danger))
                                .when(!falta, |d| d.text_color(cx.theme().muted_foreground))
                                .child(marca),
                        ),
                )
            })
            .when_some(self.aviso.clone(), |raiz, aviso| {
                raiz.child(div().text_xs().text_color(cx.theme().danger).child(aviso))
            })
            .when_some(self.ultimo.clone(), |raiz, ultimo| {
                raiz.child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(cx.theme().muted_foreground)
                        .child(ultimo),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.resumo()),
                    )
                    .child(
                        Button::new("exportacao-exportar")
                            .label(format!("Exportar {}", self.fotos.len()))
                            .xsmall()
                            .primary()
                            .disabled(!pronto)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.exportar(cx);
                            })),
                    ),
            )
    }
}
