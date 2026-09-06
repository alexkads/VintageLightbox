//! **Dentro** de uma sessão fotográfica — a mesma tela que a web tem em
//! `/dashboard/sessoes-fotograficas/{id}`.
//!
//! # As três partes, e a ordem delas
//!
//! O desenho é o de lá, e a ordem não é arbitrária — ela é o fluxo do balcão:
//!
//! 1. **cabeçalho**: quem é o cliente, o que a sessão tem, e os dois botões de
//!    sair (avisar e copiar o link);
//! 2. **envio**: e o **estado é escolhido antes dos arquivos**, porque são duas
//!    levas — as que o cliente levou e as que ficaram à venda;
//! 3. **grade**: as fotos que já estão no site, com o que foi marcado no balcão.
//!
//! # 🚨 Por que esta tela precisou existir
//!
//! Antes de 6/set/2026 o desktop tinha a lista de sessões e nada dentro delas:
//! "abrir" só marcava a sessão como destino das próximas classificadas. Quem
//! vinha da web achava a coisa *"muito aberta e estranha"* — e estava certo: lá
//! a sessão é **onde se trabalha**, e aqui ela era um rótulo.
//!
//! # As fotos são as do site, e não as do catálogo
//!
//! 🔑 A grade daqui mostra o que está **no storage**, com as miniaturas vindas
//! da API. É o que torna a tela verdadeira: a foto que outro computador do
//! estúdio subiu aparece aqui, e a que a retenção apagou aparece como apagada —
//! coisas que o catálogo local não tem como saber.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::{
    EstadoDaFotoNoSite, EstadoNoBalcao, FotoDaGaleria, GaleriaAberta, LinkDeAcesso, Sessao,
};
use gpui::{
    div, img, prelude::*, px, Context, EventEmitter, RenderImage, SharedString, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};

use super::arquivos::SeletorDeFotos;
use crate::pos_venda::porta::{Publicador, Recado};

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// O que a tela pede à raiz — ela não sabe trocar de tela nem abrir a Revelação.
pub enum Pedido {
    /// Voltar para a lista de sessões.
    Voltar,
    /// Revelar uma foto **do site**: o id remoto, que a Revelação usa para
    /// buscar a cópia de trabalho.
    Revelar { foto_id: String, arquivo: String },
}

impl EventEmitter<Pedido> for Detalhe {}

pub struct Detalhe {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    galeria_id: Option<String>,
    aberta: Option<GaleriaAberta>,
    /// As miniaturas já decodificadas, por id da foto no site.
    miniaturas: HashMap<String, Arc<RenderImage>>,
    /// De quem já foi pedida miniatura, para não pedir duas vezes.
    pedidas: std::collections::HashSet<String>,
    /// Quem abre a janela **do sistema** para escolher as fotos.
    seletor: Arc<dyn SeletorDeFotos>,
    /// Por onde os caminhos escolhidos voltam.
    escolhas: (Sender<Vec<String>>, Receiver<Vec<String>>),
    /// A leva: como as próximas fotos entram.
    ///
    /// 🔑 **`None` é "sem marcação", e é o padrão** — a mesma escolha da web
    /// (`envio.tsx`, 2026-09-05): *a marcação de verdade nasce no balcão, com o
    /// cliente olhando*. Escolher aqui, antes de as fotos entrarem, é decidir
    /// por trinta de uma vez o que se decide uma a uma. Sem marcação a foto vai
    /// para o acervo **à venda**, que é o estado de quem ainda não foi levada.
    leva: Option<EstadoNoBalcao>,
    /// Se há algo sendo arrastado por cima — o destaque da área.
    arrastando: bool,
    enviando: usize,
    enviadas: usize,
    link: Option<LinkDeAcesso>,
    carregando: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Detalhe {
    pub fn nova(publicador: Arc<dyn Publicador>, seletor: Arc<dyn SeletorDeFotos>) -> Self {
        Self {
            publicador,
            seletor,
            escolhas: channel(),
            leva: None,
            arrastando: false,
            sessao: None,
            galeria_id: None,
            aberta: None,
            miniaturas: HashMap::new(),
            pedidas: std::collections::HashSet::new(),
            enviando: 0,
            enviadas: 0,
            link: None,
            carregando: false,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    pub fn definir_sessao(&mut self, sessao: Sessao) {
        self.sessao = Some(sessao);
    }

    /// Entra numa sessão: pede a galeria e as fotos dela.
    pub fn entrar(&mut self, galeria_id: String, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            self.erro = Some("esta tela precisa da conta do site".into());
            cx.notify();
            return;
        };
        // 🔑 Tudo o que era da sessão anterior sai: miniatura e link de outra
        // galeria na tela desta seria o pior tipo de erro — o que parece certo.
        self.galeria_id = Some(galeria_id.clone());
        self.aberta = None;
        self.miniaturas.clear();
        self.pedidas.clear();
        self.link = None;
        self.enviadas = 0;
        self.erro = None;
        self.carregando = true;

        self.publicador
            .abrir_galeria(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn galeria_id(&self) -> Option<&str> {
        self.galeria_id.as_deref()
    }

    pub fn aberta(&self) -> Option<&GaleriaAberta> {
        self.aberta.as_ref()
    }

    pub fn link(&self) -> Option<&LinkDeAcesso> {
        self.link.as_ref()
    }

    pub fn erro(&self) -> Option<&SharedString> {
        self.erro.as_ref()
    }

    /// Quantas fotos há em cada estado — o que o cabeçalho conta.
    pub fn contagem(&self) -> (usize, usize, usize) {
        let fotos = self
            .aberta
            .as_ref()
            .map(|a| a.fotos.as_slice())
            .unwrap_or(&[]);
        let levadas = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::LevadaNoBalcao)
            .count();
        let a_venda = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::Disponivel)
            .count();
        let compradas = fotos
            .iter()
            .filter(|f| f.estado == EstadoDaFotoNoSite::Comprada)
            .count();
        (levadas, a_venda, compradas)
    }

    /// A leva escolhida para as próximas fotos.
    pub fn escolher_leva(&mut self, leva: Option<EstadoNoBalcao>, cx: &mut Context<Self>) {
        self.leva = leva;
        cx.notify();
    }

    pub fn leva(&self) -> Option<EstadoNoBalcao> {
        self.leva
    }

    /// Abre a janela **do sistema** para escolher as fotos.
    pub fn escolher_fotos(&mut self, cx: &mut Context<Self>) {
        if self.enviando > 0 {
            return;
        }
        self.seletor.escolher(self.escolhas.0.clone());
        self.acompanhar(cx);
    }

    pub fn arrastando(&self) -> bool {
        self.arrastando
    }

    pub fn destacar(&mut self, arrastando: bool, cx: &mut Context<Self>) {
        if self.arrastando != arrastando {
            self.arrastando = arrastando;
            cx.notify();
        }
    }

    /// Sobe os arquivos escolhidos — do seletor ou do que foi arrastado.
    ///
    /// 🔑 **São arquivos do disco, e não fotos do catálogo.** É o envio da web:
    /// o operador exporta do Lightroom para uma pasta e manda a pasta. O que vai
    /// ao cliente não precisa estar catalogado aqui — o catálogo é da triagem em
    /// RAW, e são dois trabalhos diferentes.
    pub fn enviar_arquivos(&mut self, caminhos: Vec<String>, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        if caminhos.is_empty() || self.enviando > 0 {
            return;
        }

        // A ordem no site continua de onde a sessão parou.
        let ja_na_sessao = self.aberta.as_ref().map(|a| a.fotos.len()).unwrap_or(0);
        // Sem marcação vai como "à venda" — o estado de quem ainda não foi levada.
        let estado = self.leva.unwrap_or(EstadoNoBalcao::Disponivel);

        self.erro = None;
        self.enviadas = 0;
        self.enviando = caminhos.len();
        for (i, caminho) in caminhos.into_iter().enumerate() {
            self.publicador.enviar_arquivo(
                sessao.clone(),
                galeria_id.clone(),
                caminho,
                (ja_na_sessao + i) as u32,
                estado,
                self.recados.0.clone(),
            );
        }
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn pedir_o_link(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_id.clone())
        else {
            return;
        };
        self.publicador
            .link(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Pede as miniaturas que ainda faltam — uma vez cada.
    fn pedir_miniaturas(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let faltam: Vec<String> = self
            .aberta
            .as_ref()
            .map(|a| {
                a.fotos
                    .iter()
                    .filter(|f| !f.apagada && !self.pedidas.contains(&f.id))
                    .map(|f| f.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        for id in faltam {
            self.pedidas.insert(id.clone());
            self.publicador
                .miniatura(sessao.clone(), id, self.recados.0.clone());
        }
        if !self.pedidas.is_empty() {
            self.acompanhar(cx);
        }
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

    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        let mut abriu = false;

        // O que o seletor do sistema devolveu. Lista vazia é desistência, e não
        // erro: fechar a janela sem escolher é um gesto legítimo.
        while let Ok(caminhos) = self.escolhas.1.try_recv() {
            mudou = true;
            if !caminhos.is_empty() {
                self.enviar_arquivos(caminhos, cx);
            }
        }

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Aberta(aberta) => {
                    self.carregando = false;
                    self.aberta = Some(*aberta);
                    abriu = true;
                }
                Recado::Miniatura { foto_id, bytes } => {
                    // Miniatura ilegível não derruba a grade: a célula fica sem
                    // imagem, com o nome do arquivo, que é melhor que nada.
                    if let Ok(imagem) = image::load_from_memory(&bytes) {
                        self.miniaturas
                            .insert(foto_id, crate::imagem::para_gpui(imagem));
                    }
                }
                Recado::Sincronizou => {
                    self.enviadas += 1;
                    self.enviando = self.enviando.saturating_sub(1);
                    if self.enviando == 0 {
                        // O lote acabou: reler a sessão é o que faz as novas
                        // aparecerem na grade.
                        if let (Some(sessao), Some(id)) =
                            (self.sessao.clone(), self.galeria_id.clone())
                        {
                            self.publicador
                                .abrir_galeria(sessao, id, self.recados.0.clone());
                            self.carregando = true;
                        }
                    }
                }
                Recado::Link(link) => {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.url.clone()));
                    self.link = Some(link);
                }
                Recado::Falhou(erro) => {
                    self.carregando = false;
                    self.enviando = self.enviando.saturating_sub(1);
                    self.erro = Some(erro.into());
                }
                _ => {}
            }
        }

        if abriu {
            self.pedir_miniaturas(cx);
        }
        if mudou {
            cx.notify();
        }
        let continua =
            self.carregando || self.enviando > 0 || self.pedidas.len() > self.miniaturas.len();
        if !continua {
            self.colhendo = false;
        }
        continua
    }
}

impl Render for Detalhe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .size_full()
            .p(px(12.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.cabecalho(cx))
            .child(self.envio(cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(div().text_xs().text_color(cx.theme().danger).child(erro))
            })
            .child(self.grade(cx))
    }
}

impl Detalhe {
    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (levadas, a_venda, compradas) = self.contagem();
        let titulo = self
            .aberta
            .as_ref()
            .map(|a| a.galeria.titulo.clone())
            .unwrap_or_else(|| "…".into());
        let contato = self
            .aberta
            .as_ref()
            .and_then(|a| {
                a.galeria
                    .email
                    .clone()
                    .or_else(|| a.galeria.whatsapp.clone())
            })
            .unwrap_or_else(|| "sem contato".into());

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pb(px(8.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("detalhe-voltar")
                    .label("← Sessões")
                    .xsmall()
                    .on_click(cx.listener(|_tela, _ev, _window, cx| cx.emit(Pedido::Voltar))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(div().text_sm().child(titulo))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{contato} · {levadas} levada(s) · {a_venda} à venda · \
                                 {compradas} comprada(s)"
                            )),
                    ),
            )
            .child(
                Button::new("detalhe-link")
                    .label(if self.link.is_some() {
                        "Copiar de novo"
                    } else {
                        "Copiar link do cliente"
                    })
                    .xsmall()
                    .disabled(self.aberta.is_none())
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.pedir_o_link(cx))),
            )
    }

    /// A área de envio: **arrastar a pasta**, ou a janela do sistema.
    ///
    /// 🚨 **Não há explorador de arquivos nosso aqui.** O app tem um, no modal de
    /// importação, e ele existe para a triagem em RAW — escolher entre duzentas
    /// do cartão. Para mandar fotos ao cliente ele é atrito: quem exportou do
    /// Lightroom já está com a pasta aberta ao lado. É o gesto da web, e o
    /// pedido do dono: *"tem que usar o mesmo explorador de arquivos do sistema
    /// operacional"*.
    ///
    /// 🔑 **A leva é escolhida antes dos arquivos**, e o padrão é **sem
    /// marcação** — ver o campo [`Self::leva`].
    fn envio(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ocupado = self.enviando > 0;
        let sem_sessao = self.aberta.is_none();
        let leva = self.leva;

        let ficha = |rotulo: &'static str,
                     valor: Option<EstadoNoBalcao>,
                     cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("detalhe-leva-{rotulo}")))
                .label(rotulo)
                .xsmall()
                .selected(leva == valor)
                .disabled(ocupado)
                .on_click(cx.listener(move |tela, _ev, _window, cx| tela.escolher_leva(valor, cx)))
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(10.))
            .rounded(cx.theme().radius)
            .border_1()
            // O destaque de "solte aqui": borda viva enquanto o arrasto passa.
            .border_color(if self.arrastando {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .on_drag_move(
                cx.listener(|tela, _ev: &gpui::DragMoveEvent<()>, _window, cx| {
                    tela.destacar(true, cx)
                }),
            )
            .on_drop(
                cx.listener(|tela, arrastados: &gpui::ExternalPaths, _window, cx| {
                    tela.destacar(false, cx);
                    // 🔑 Uma pasta solta vira o conteúdo dela: o sistema entrega
                    // o caminho do diretório, e uma pasta de exportação tem
                    // arquivos, não subpastas.
                    let fotos = super::arquivos::so_as_fotos(arrastados.paths());
                    tela.enviar_arquivos(fotos, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Como esta leva entra:"),
                    )
                    .child(ficha("Sem marcação", None, cx))
                    .child(ficha("À venda", Some(EstadoNoBalcao::Disponivel), cx))
                    .child(ficha(
                        "Levadas no balcão",
                        Some(EstadoNoBalcao::LevadaNoBalcao),
                        cx,
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match leva {
                        None => {
                            "Sem marcação vai para o acervo à venda. A marcação de verdade \
                             nasce no balcão, com o cliente olhando."
                        }
                        Some(EstadoNoBalcao::Disponivel) => {
                            "À venda: o cliente vê com marca d'água e pode comprar pela galeria."
                        }
                        Some(EstadoNoBalcao::LevadaNoBalcao) => {
                            "Levadas: o cliente já pagou na hora e baixa o original."
                        }
                    }),
            )
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
                            .child(match (ocupado, self.enviadas) {
                                (true, feitas) => format!("subindo… {feitas} prontas"),
                                (false, feitas) if feitas > 0 => format!("{feitas} subiram"),
                                (false, _) => "Arraste as fotos (ou a pasta) para cá.".to_string(),
                            }),
                    )
                    .child(
                        Button::new("detalhe-escolher")
                            .label("Escolher fotos…")
                            .xsmall()
                            .primary()
                            .disabled(ocupado || sem_sessao)
                            .on_click(
                                cx.listener(|tela, _ev, _window, cx| tela.escolher_fotos(cx)),
                            ),
                    ),
            )
    }

    fn grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(aberta) = self.aberta.as_ref() else {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if self.carregando {
                    "Lendo a sessão…"
                } else {
                    "Sessão não aberta."
                })
                .into_any_element();
        };

        if aberta.fotos.is_empty() {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Nenhuma foto nesta sessão ainda — suba a primeira leva acima.")
                .into_any_element();
        }

        div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .flex_1()
            .min_h(px(0.))
            .children(aberta.fotos.iter().map(|foto| self.celula(foto, cx)))
            .into_any_element()
    }

    fn celula(&self, foto: &FotoDaGaleria, cx: &mut Context<Self>) -> impl IntoElement {
        let id = foto.id.clone();
        let arquivo = foto.arquivo.clone();
        let miniatura = self.miniaturas.get(&foto.id).cloned();
        let apagada = foto.apagada;

        div()
            .id(SharedString::from(format!("sessao-foto-{}", foto.id)))
            .w(px(150.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(4.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .when(!apagada, |celula| celula.cursor_pointer())
            .when(!apagada, |celula| {
                celula.on_click(cx.listener(move |_tela, _ev, _window, cx| {
                    cx.emit(Pedido::Revelar {
                        foto_id: id.clone(),
                        arquivo: arquivo.clone(),
                    })
                }))
            })
            .child(
                div()
                    .h(px(110.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().muted)
                    .rounded(cx.theme().radius)
                    .when_some(miniatura, |quadro, imagem| {
                        quadro.child(img(imagem).h(px(110.)))
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .child(SharedString::from(foto.arquivo.clone())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if apagada {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if apagada {
                        "apagada pela retenção".to_string()
                    } else {
                        foto.estado.rotulo().to_string()
                    }),
            )
    }
}
