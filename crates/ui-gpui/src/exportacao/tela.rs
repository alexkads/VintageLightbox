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
use gpui_component::{ActiveTheme, Disableable, Sizable};

use crate::importacao::estado::Recado;
use crate::importacao::explorador::SeletorDePasta;

use super::destino::Destinos;
use super::porta::{Andamento, Exportador, Saida};

/// O mesmo intervalo da colheita da importação.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// A extensão de saída. Fixa enquanto o formato não é escolha da tela — e
/// **declarada aqui, e não espalhada**, para o dia em que virar.
const EXTENSAO: &str = "jpg";

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
    progresso: Option<Progresso>,
    /// O último nome gravado, para a tela mostrar que algo está acontecendo.
    ultimo: Option<SharedString>,
    aviso: Option<SharedString>,
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    recados: (Sender<Recado>, Receiver<Recado>),
    esperando_pasta: bool,
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
            progresso: None,
            ultimo: None,
            aviso: None,
            andamentos: channel(),
            recados: channel(),
            esperando_pasta: false,
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

    pub fn progresso(&self) -> Option<Progresso> {
        self.progresso
    }

    /// Abre o seletor nativo de pasta.
    pub fn escolher_pasta(&mut self, cx: &mut Context<Self>) {
        self.esperando_pasta = true;
        self.seletor.escolher_destino(self.recados.0.clone());
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

        self.exportador.exportar(saidas, self.andamentos.0.clone());
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
            if let Recado::DestinoEscolhido(caminho) = recado {
                self.pasta = Some(PathBuf::from(caminho));
            }
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
        let pronto = self.pasta.is_some() && !self.fotos.is_empty() && !self.correndo();

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
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    // O formato é fixo neste incremento, e a tela diz isso em vez
                    // de deixar quem exporta descobrir abrindo a pasta.
                    .child("JPEG, qualidade 90 · com a revelação e o enquadramento aplicados"),
            )
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
