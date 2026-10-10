//! A tela do visualizador: a barra (página, zoom, pasta) e a coluna de
//! páginas que rola.
//!
//! # Só o que está à vista é desenhado
//!
//! Um fotolivro tem dezenas de páginas, e cada uma em tela cheia são alguns
//! megabytes de pixels. A cada quadro a tela olha onde a rolagem está
//! ([`super::folhas::a_vista`]), pede à [`Fila`] as páginas à vista (e uma de
//! folga para cada lado) na escala da tela, e solta as que ficaram longe — a
//! `imagem::coleta` devolve a textura à GPU.
//!
//! Trocar o zoom não apaga nada: a página antiga fica esticada no lugar até a
//! nova, na escala certa, chegar.
//!
//! # 🔗 Os links do livro funcionam
//!
//! Dono, 10/out/2026: *"Os clicks nos links do PDF funcionam no novo
//! visualizador?"*. Cada link da folha é uma área clicável por cima da página
//! (a fração que o crate entrega vezes o tamanho da página na tela): o
//! endereço abre no navegador do sistema, e o salto interno ("Folhear o
//! álbum", "Galeria ›") rola até a página. Só `http(s)` e `mailto` saem do
//! app — um PDF de fora não manda abrir `file:` nem outro esquema.
//!
//! # 📐 Tamanho fixo
//!
//! O palco mede sempre o mesmo ([`tamanho_do_palco`], da janela): abrindo,
//! aberto ou com erro, a caixa do diálogo não muda (dono, 10/out/2026: *"Eu
//! não gosto quando a tela muda de tamanho com uma ação!"*).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon};
use gpui_kit::{
    div, img, point, prelude::*, px, size, AnyElement, Context, FontWeight, Pixels, RenderImage,
    ScrollHandle, SharedString, Size, Task, Window,
};

use visualizador_pdf::{Destino, Fila, Folha, Link, Ordem, Pedido, Resposta};

use super::folhas::{self, ENTRE, RESPIRO};
use crate::estilo;
use crate::recursos::Icone;

/// De quanto em quanto a tela colhe o que a fila desenhou.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(30);
/// Páginas desenhadas de folga, antes e depois das que estão à vista.
const FOLGA: usize = 1;
/// Além disto (para cada lado das que estão à vista) a imagem é solta.
const GUARDADAS: usize = 4;
/// A maior largura do diálogo do visualizador.
const LARGURA_MAXIMA: f32 = 1120.;

/// A largura do diálogo para esta janela — a raiz e a tela usam a mesma.
pub fn largura_do_dialogo(window: &Window) -> f32 {
    (f32::from(window.viewport_size().width) - 96.).clamp(480., LARGURA_MAXIMA)
}

/// O palco (a área das páginas): a largura do diálogo sem o respiro dele, e a
/// altura que sobra na janela depois do cabeçalho, da barra e das margens.
pub fn tamanho_do_palco(window: &Window) -> Size<Pixels> {
    let altura = (f32::from(window.viewport_size().height) - 200.).max(240.);
    size(px(largura_do_dialogo(window) - 32.), px(altura))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Estado {
    Abrindo,
    Aberto,
    /// O arquivo não abriu: a frase para o operador.
    Falhou(SharedString),
}

/// Uma página que já está na tela, e a escala em que foi desenhada.
struct Desenhada {
    imagem: Arc<RenderImage>,
    escala: f32,
}

pub struct VisualizadorDePdf {
    arquivo: PathBuf,
    estado: Estado,
    fila: Option<Fila>,
    folhas: Vec<Folha>,
    /// 🔗 As áreas clicáveis de cada folha.
    links: Vec<Vec<Link>>,
    desenhadas: Vec<Option<Desenhada>>,
    /// A página que não pôde ser desenhada, e por quê — não é pedida de novo.
    falhas: Vec<Option<SharedString>>,
    /// `1.0` é a página inteira dentro do palco.
    zoom: f32,
    rolagem: ScrollHandle,
    /// O palco do último quadro: a navegação faz a conta com ele.
    palco: Size<Pixels>,
    /// A última lista mandada à fila.
    pedidos: Vec<Pedido>,
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _leitura: Task<()>,
}

impl VisualizadorDePdf {
    /// Abre o arquivo: a leitura e o desenho ficam fora da thread da tela.
    pub fn abrir(arquivo: PathBuf, cx: &mut Context<Self>) -> Self {
        let caminho = arquivo.clone();
        let lido = cx
            .background_executor()
            .spawn(async move { std::fs::read(&caminho) });
        let leitura = cx.spawn(async move |esta, cx| {
            let lido = lido.await;
            let _ = esta.update(cx, |tela, cx| {
                match lido {
                    Ok(bytes) => {
                        tela.fila = Some(Fila::abrir(Arc::new(bytes), Ordem::Bgra));
                        tela.acompanhar(cx);
                    }
                    Err(erro) => {
                        tela.estado =
                            Estado::Falhou(format!("O arquivo não pôde ser lido: {erro}.").into());
                    }
                }
                cx.notify();
            });
        });
        Self {
            arquivo,
            estado: Estado::Abrindo,
            fila: None,
            folhas: Vec::new(),
            links: Vec::new(),
            desenhadas: Vec::new(),
            falhas: Vec::new(),
            zoom: 1.0,
            rolagem: ScrollHandle::new(),
            palco: size(px(800.), px(600.)),
            pedidos: Vec::new(),
            colhendo: false,
            _colheita: None,
            _leitura: leitura,
        }
    }

    pub fn arquivo(&self) -> &Path {
        &self.arquivo
    }

    /// O nome do arquivo — o título do diálogo.
    pub fn titulo(&self) -> SharedString {
        self.arquivo
            .file_name()
            .map(|nome| nome.to_string_lossy().to_string())
            .unwrap_or_else(|| "PDF".into())
            .into()
    }

    pub fn estado(&self) -> &Estado {
        &self.estado
    }

    pub fn paginas(&self) -> usize {
        self.folhas.len()
    }

    /// Quantas páginas têm imagem na tela agora.
    pub fn desenhadas(&self) -> usize {
        self.desenhadas.iter().flatten().count()
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    fn largura_da_pagina(&self) -> f32 {
        // A proporção da primeira folha decide o "caber inteira": no livro
        // todas são iguais.
        let aspecto = self.folhas.first().map_or(297. / 210., Folha::aspecto);
        folhas::largura_da_pagina(
            (f32::from(self.palco.width), f32::from(self.palco.height)),
            aspecto,
            self.zoom,
        )
    }

    fn posicoes(&self) -> Vec<(f32, f32)> {
        folhas::posicoes(&self.folhas, self.largura_da_pagina())
    }

    /// Quanto a coluna está rolada para baixo, em pixels.
    fn rolado(&self) -> f32 {
        -f32::from(self.rolagem.offset().y)
    }

    /// A página no meio do palco, contada de 0.
    pub fn pagina_atual(&self) -> usize {
        folhas::pagina_atual(
            &self.posicoes(),
            self.rolado(),
            f32::from(self.palco.height),
        )
    }

    /// Põe a página (contada de 0) no alto do palco.
    pub fn ir_para(&mut self, pagina: usize, cx: &mut Context<Self>) {
        let alvo = folhas::rolar_ate(
            &self.posicoes(),
            pagina.min(self.folhas.len().saturating_sub(1)),
            f32::from(self.palco.height),
        );
        let atual = self.rolagem.offset();
        self.rolagem.set_offset(point(atual.x, px(-alvo)));
        cx.notify();
    }

    pub fn pagina_anterior(&mut self, cx: &mut Context<Self>) {
        self.ir_para(self.pagina_atual().saturating_sub(1), cx);
    }

    pub fn proxima_pagina(&mut self, cx: &mut Context<Self>) {
        self.ir_para(self.pagina_atual() + 1, cx);
    }

    /// Troca o zoom mantendo a mesma página no alto.
    pub fn definir_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        if zoom == self.zoom {
            return;
        }
        let pagina = self.pagina_atual();
        self.zoom = zoom;
        self.ir_para(pagina, cx);
    }

    pub fn aproximar(&mut self, cx: &mut Context<Self>) {
        self.definir_zoom(folhas::degrau(self.zoom, 1), cx);
    }

    pub fn afastar(&mut self, cx: &mut Context<Self>) {
        self.definir_zoom(folhas::degrau(self.zoom, -1), cx);
    }

    /// De volta à página inteira dentro do palco.
    pub fn ajustar(&mut self, cx: &mut Context<Self>) {
        self.definir_zoom(1.0, cx);
    }

    /// Os links da página (contada de 0).
    pub fn links_da_pagina(&self, pagina: usize) -> &[Link] {
        self.links.get(pagina).map_or(&[], Vec::as_slice)
    }

    /// 🔗 Segue um link do documento: o endereço abre no navegador, e o salto
    /// interno rola até a página.
    pub fn seguir(&mut self, destino: &Destino, cx: &mut Context<Self>) {
        match destino {
            Destino::Pagina(pagina) => self.ir_para(*pagina, cx),
            Destino::Endereco(endereco) if endereco_que_abre(endereco) => cx.open_url(endereco),
            Destino::Endereco(_) => {}
        }
    }

    fn mostrar_na_pasta(&self, cx: &mut Context<Self>) {
        cx.reveal_path(&self.arquivo);
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

    /// Recebe o que a fila desenhou. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Some(resposta) = self.fila.as_ref().and_then(Fila::colher) {
            mudou = true;
            match resposta {
                Resposta::Aberto { folhas, links } => {
                    self.links = links;
                    self.desenhadas = folhas.iter().map(|_| None).collect();
                    self.falhas = folhas.iter().map(|_| None).collect();
                    self.folhas = folhas;
                    self.estado = if self.folhas.is_empty() {
                        Estado::Falhou("O documento não tem nenhuma página.".into())
                    } else {
                        Estado::Aberto
                    };
                }
                Resposta::Falhou(erro) => {
                    self.estado = Estado::Falhou(primeira_maiuscula(&erro.to_string()).into());
                }
                Resposta::Pagina { pedido, imagem } => {
                    self.pedidos.retain(|p| *p != pedido);
                    let Some(lugar) = self.desenhadas.get_mut(pedido.pagina) else {
                        continue;
                    };
                    match imagem.map(|i| crate::imagem::de_bgra(i.largura, i.altura, i.pixels)) {
                        Ok(Some(imagem)) => {
                            *lugar = Some(Desenhada {
                                imagem,
                                escala: pedido.escala,
                            });
                        }
                        Ok(None) => {}
                        Err(erro) => {
                            self.falhas[pedido.pagina] =
                                Some(primeira_maiuscula(&erro.to_string()).into());
                        }
                    }
                }
            }
        }
        let continua = self.estado == Estado::Abrindo || !self.pedidos.is_empty();
        if !continua {
            self.colhendo = false;
        }
        if mudou {
            cx.notify();
        }
        continua
    }

    /// Pede as páginas à vista que ainda não estão na escala da tela, e solta
    /// as que ficaram longe. Chamado a cada quadro, com a rolagem já no lugar.
    fn pedir_o_que_falta(&mut self, densidade: f32, cx: &mut Context<Self>) {
        if self.estado != Estado::Aberto {
            return;
        }
        let posicoes = self.posicoes();
        let a_vista = folhas::a_vista(&posicoes, self.rolado(), f32::from(self.palco.height));
        let largura = self.largura_da_pagina();
        let total = self.folhas.len();

        let guardar = a_vista.start.saturating_sub(GUARDADAS)..(a_vista.end + GUARDADAS).min(total);
        for (pagina, desenhada) in self.desenhadas.iter_mut().enumerate() {
            if !guardar.contains(&pagina) {
                *desenhada = None;
            }
        }

        let querer = a_vista.start.saturating_sub(FOLGA)..(a_vista.end + FOLGA).min(total);
        // As que estão à vista primeiro; a folga por último.
        let mut ordem: Vec<usize> = a_vista.clone().collect();
        ordem.extend(querer.clone().filter(|p| !a_vista.contains(p)));
        let pedidos: Vec<Pedido> = ordem
            .into_iter()
            .filter(|&pagina| self.falhas[pagina].is_none())
            .filter_map(|pagina| {
                let escala = arredondada(largura / self.folhas[pagina].largura.max(1.) * densidade);
                let em_dia = self.desenhadas[pagina]
                    .as_ref()
                    .is_some_and(|d| d.escala == escala);
                (!em_dia).then_some(Pedido { pagina, escala })
            })
            .collect();
        if pedidos != self.pedidos {
            if let Some(fila) = &self.fila {
                fila.pedir(pedidos.clone());
            }
            self.pedidos = pedidos;
        }
        if !self.pedidos.is_empty() {
            self.acompanhar(cx);
        }
    }

    /// A barra: página, zoom e a pasta — botões do gpui-kit, na medida
    /// pequena do template.
    fn render_barra(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let aberto = self.estado == Estado::Aberto;
        let total = self.folhas.len();
        let atual = self.pagina_atual();
        let pagina: SharedString = if aberto {
            format!("Página {} de {total}", atual + 1).into()
        } else {
            "Página — de —".into()
        };

        h_flex()
            .debug_selector(|| "pdf-barra".into())
            .gap(px(8.))
            .child(
                estilo::botao_icone_pequeno("pdf-anterior", Icone::ArrowUp)
                    .tooltip("Página anterior")
                    .disabled(!aberto || atual == 0)
                    .on_click(cx.listener(|tela, _, _, cx| tela.pagina_anterior(cx))),
            )
            .child(
                // Largura fixa: "Página 9 de 56" e "Página 10 de 56" não
                // empurram os botões.
                div()
                    .debug_selector(|| "pdf-pagina".into())
                    .w(px(132.))
                    .text_center()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(pagina),
            )
            .child(
                estilo::botao_icone_pequeno("pdf-proxima", Icone::ArrowDown)
                    .tooltip("Próxima página")
                    .disabled(!aberto || atual + 1 >= total)
                    .on_click(cx.listener(|tela, _, _, cx| tela.proxima_pagina(cx))),
            )
            .child(div().flex_1())
            .child(
                estilo::botao_icone_pequeno("pdf-afastar", Icone::ZoomOut)
                    .tooltip("Afastar")
                    .disabled(!aberto || self.zoom <= folhas::DEGRAUS[0])
                    .on_click(cx.listener(|tela, _, _, cx| tela.afastar(cx))),
            )
            .child(
                div()
                    .debug_selector(|| "pdf-zoom".into())
                    .w(px(44.))
                    .text_center()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(format!("{:.0}%", self.zoom * 100.)),
            )
            .child(
                estilo::botao_icone_pequeno("pdf-aproximar", Icone::ZoomIn)
                    .tooltip("Aproximar")
                    .disabled(!aberto || self.zoom >= folhas::DEGRAUS[folhas::DEGRAUS.len() - 1])
                    .on_click(cx.listener(|tela, _, _, cx| tela.aproximar(cx))),
            )
            .child(
                estilo::botao_contorno_pequeno("pdf-ajustar", cx)
                    .label("Página inteira")
                    .disabled(!aberto || self.zoom == 1.0)
                    .on_click(cx.listener(|tela, _, _, cx| tela.ajustar(cx))),
            )
            .child(div().flex_1())
            .child(
                estilo::botao_contorno_pequeno("pdf-mostrar", cx)
                    .icon(Icon::new(Icone::FolderOpen))
                    .label("Mostrar na pasta")
                    .on_click(cx.listener(|tela, _, _, cx| tela.mostrar_na_pasta(cx))),
            )
            .into_any_element()
    }

    /// A coluna de páginas. Cada uma é uma folha de papel branca já no
    /// tamanho final: a imagem entra quando chega, sem nada mudar de lugar.
    fn render_paginas(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let largura = self.largura_da_pagina();
        let posicoes = self.posicoes();
        let a_vista = folhas::a_vista(&posicoes, self.rolado(), f32::from(self.palco.height));
        let desenhar = a_vista.start.saturating_sub(FOLGA)..a_vista.end + FOLGA;
        let realce = tema.primary.opacity(0.14);

        v_flex()
            .items_center()
            // A coluna nunca é mais estreita que a página (no zoom alto é ela
            // que dá a largura para a rolagem de lado) nem que o palco (a
            // página menor que ele fica no meio).
            .min_w(px(
                (largura + 2. * RESPIRO).max(f32::from(self.palco.width) - 2.)
            ))
            .p(px(RESPIRO))
            .gap(px(ENTRE))
            .children(posicoes.iter().enumerate().map(|(pagina, (_, altura))| {
                let altura = *altura;
                let folha = div()
                    .flex_none()
                    .relative()
                    .w(px(largura))
                    .h(px(altura))
                    .bg(gpui_kit::white())
                    .shadow_md();
                if !desenhar.contains(&pagina) {
                    return folha;
                }
                // 🔗 As áreas clicáveis, por cima do que a página tiver.
                let links: Vec<_> = self
                    .links_da_pagina(pagina)
                    .iter()
                    .enumerate()
                    .map(|(i, link)| {
                        let [esquerda, topo, l, a] = link.caixa;
                        let destino = link.destino.clone();
                        let dica: SharedString = match &link.destino {
                            Destino::Pagina(alvo) => format!("Ir para a página {}", alvo + 1),
                            Destino::Endereco(endereco) => format!("Abrir {endereco}"),
                        }
                        .into();
                        div()
                            .id(("pdf-link", pagina * 1000 + i))
                            .debug_selector(move || format!("pdf-link-{}-{}", pagina + 1, i + 1))
                            .absolute()
                            .left(px(esquerda * largura))
                            .top(px(topo * altura))
                            .w(px(l * largura))
                            .h(px(a * altura))
                            .cursor_pointer()
                            .hover(move |s| s.bg(realce))
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(dica.clone())
                                    .build(window, cx)
                            })
                            .on_click(cx.listener(move |tela, _, _, cx| tela.seguir(&destino, cx)))
                    })
                    .collect();
                let folha = match (&self.desenhadas[pagina], &self.falhas[pagina]) {
                    // A moldura já tem a proporção da folha (a altura sai
                    // dela), então a imagem a preenche inteira.
                    (Some(desenhada), _) => folha
                        .debug_selector(move || format!("pdf-pagina-{}", pagina + 1))
                        .child(img(desenhada.imagem.clone()).size_full()),
                    (None, Some(falha)) => folha.child(
                        v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .px(px(24.))
                            .text_sm()
                            .text_color(tema.danger)
                            .child(Icon::new(Icone::CircleAlert).size(px(20.)))
                            .child(falha.clone()),
                    ),
                    (None, None) => folha.child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Spinner::new().color(gpui_kit::black().opacity(0.4))),
                    ),
                };
                folha.children(links)
            }))
            .into_any_element()
    }
}

/// Três casas bastam para a página não ser pedida de novo por ruído do ponto
/// flutuante, e para duas escalas diferentes não parecerem a mesma.
fn arredondada(escala: f32) -> f32 {
    (escala * 1000.).round() / 1000.
}

/// Só o que um navegador ou o programa de e-mail abre sai do app.
fn endereco_que_abre(endereco: &str) -> bool {
    let minusculo = endereco.trim_start().to_ascii_lowercase();
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|esquema| minusculo.starts_with(esquema))
}

fn primeira_maiuscula(frase: &str) -> String {
    let mut letras = frase.chars();
    match letras.next() {
        Some(primeira) => format!("{}{}.", primeira.to_uppercase(), letras.as_str()),
        None => String::new(),
    }
}

impl gpui_kit::Render for VisualizadorDePdf {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.palco = tamanho_do_palco(window);
        self.pedir_o_que_falta(window.scale_factor(), cx);

        let tema = cx.theme().clone();
        let miolo = match &self.estado {
            Estado::Aberto => self.render_paginas(cx),
            Estado::Abrindo => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .text_sm()
                .text_color(tema.muted_foreground)
                .child(Spinner::new())
                .child("Abrindo o livro…")
                .into_any_element(),
            Estado::Falhou(porque) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .child(
                    div()
                        .debug_selector(|| "pdf-falhou".into())
                        .child(estilo::aviso(porque.clone(), true, cx)),
                )
                .into_any_element(),
        };

        v_flex().gap(px(12.)).child(self.render_barra(cx)).child(
            div()
                .id("pdf-palco")
                .debug_selector(|| "pdf-palco".into())
                .w(self.palco.width)
                .h(self.palco.height)
                .rounded(crate::tema::canto(8.))
                .border_1()
                .border_color(tema.border)
                .bg(tema.muted)
                .overflow_scroll()
                .track_scroll(&self.rolagem)
                .child(miolo),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use gpui_kit::{TestAppContext, VisualTestContext};

    const GALERIA: &str = "https://recordarfotos.com.br/meus-ensaios/abc";

    /// O fotolivro de verdade, num arquivo: é o que o balcão abre.
    fn livro_em_arquivo(pasta: &Path, fotos: usize) -> PathBuf {
        let capa = fotolivro::Capa {
            titulo: "Ensaio de teste".into(),
            site: "recordarfotos.com.br".into(),
            galeria: Some(GALERIA.into()),
            ..Default::default()
        };
        let folhas = (0..fotos)
            .map(|i| fotolivro::FotoDaFolha {
                imagem: image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                    400,
                    300,
                    image::Rgb([200, 30 + i as u8 * 20, 30]),
                )),
                nome: format!("foto-{i}.jpg"),
                levada: i % 2 == 0,
                link: Some(format!("{GALERIA}?foto=foto-{i}")),
            })
            .collect();
        let arquivo = pasta.join("Ensaio de teste.pdf");
        std::fs::write(&arquivo, fotolivro::gerar(&capa, folhas).unwrap()).unwrap();
        arquivo
    }

    struct Janela(gpui_kit::Entity<VisualizadorDePdf>);

    impl gpui_kit::Render for Janela {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.clone())
        }
    }

    fn abrir(
        arquivo: PathBuf,
        cx: &mut TestAppContext,
    ) -> (gpui_kit::Entity<VisualizadorDePdf>, &mut VisualTestContext) {
        cx.update(gpui_kit::init);
        let (janela, cx) =
            cx.add_window_view(|_, cx| Janela(cx.new(|cx| VisualizadorDePdf::abrir(arquivo, cx))));
        let tela = janela.read_with(cx, |janela, _| janela.0.clone());
        (tela, cx)
    }

    /// ⏱️ A fila desenha numa thread de verdade e o relógio do teste é
    /// virtual: anda os dois até a condição valer.
    fn esperar(
        tela: &gpui_kit::Entity<VisualizadorDePdf>,
        cx: &mut VisualTestContext,
        o_que: &str,
        pronto: impl Fn(&VisualizadorDePdf) -> bool,
    ) {
        for _ in 0..2000 {
            cx.executor().advance_clock(INTERVALO_DE_COLHEITA);
            cx.run_until_parked();
            if tela.read_with(cx, |tela, _| pronto(tela)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("o visualizador não chegou a: {o_que}");
    }

    /// 🎬 O caminho inteiro: o livro abre, as páginas à vista ganham imagem, a
    /// navegação anda e o zoom troca a escala sem perder a página.
    #[gpui_kit::test]
    fn o_fotolivro_abre_e_se_folheia(cx: &mut TestAppContext) {
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = livro_em_arquivo(pasta.path(), 6);
        let (tela, cx) = abrir(arquivo, cx);
        tela.read_with(cx, |tela, _| {
            assert_eq!(tela.estado(), &Estado::Abrindo);
            assert_eq!(tela.titulo().as_ref(), "Ensaio de teste.pdf");
        });

        esperar(&tela, cx, "abrir", |tela| tela.estado() == &Estado::Aberto);
        let total = fotolivro::Plano::de(6).total();
        tela.read_with(cx, |tela, _| {
            assert_eq!(tela.paginas(), total);
            assert_eq!(tela.pagina_atual(), 0);
        });

        // A primeira página ganha imagem, e ela é a folha desenhada na tela.
        esperar(&tela, cx, "desenhar a primeira página", |tela| {
            tela.desenhadas[0].is_some()
        });
        cx.run_until_parked();
        let (palco, pagina) = (
            cx.debug_bounds("pdf-palco").expect("o palco"),
            cx.debug_bounds("pdf-pagina-1").expect("a primeira página"),
        );
        assert!(
            pagina.left() >= palco.left() && pagina.right() <= palco.right(),
            "a página vaza do palco: {pagina:?} × {palco:?}"
        );
        // Deitada, como a folha do livro.
        let proporcao = f32::from(pagina.size.width) / f32::from(pagina.size.height);
        assert!((proporcao - 297. / 210.).abs() < 0.01, "{proporcao}");
        // 🚨 Nem toda página é desenhada: só a vizinhança das que estão à vista.
        tela.read_with(cx, |tela, _| {
            assert!(
                tela.desenhadas() < total,
                "o livro inteiro foi desenhado de uma vez"
            );
        });

        // Pela barra: a seta leva à página 2.
        let proxima = cx.debug_bounds("pdf-proxima").expect("a seta");
        cx.simulate_mouse_move(proxima.center(), None, Default::default());
        cx.simulate_click(proxima.center(), Default::default());
        tela.read_with(cx, |tela, _| assert_eq!(tela.pagina_atual(), 1));

        // A última página: longe da primeira, que é solta; a nova é pedida.
        tela.update(cx, |tela, cx| tela.ir_para(total - 1, cx));
        esperar(&tela, cx, "desenhar a última página", |tela| {
            tela.desenhadas[total - 1].is_some()
        });
        tela.read_with(cx, |tela, _| {
            assert_eq!(tela.pagina_atual(), total - 1);
            assert!(
                tela.desenhadas[0].is_none(),
                "a primeira ficou guardada, a {} páginas de distância",
                total - 1
            );
        });

        // O zoom troca a escala e fica na mesma página.
        tela.update(cx, |tela, cx| tela.ir_para(2, cx));
        esperar(&tela, cx, "desenhar a página 3", |tela| {
            tela.desenhadas[2].is_some()
        });
        let antes = tela.read_with(cx, |tela, _| tela.desenhadas[2].as_ref().unwrap().escala);
        tela.update(cx, |tela, cx| tela.aproximar(cx));
        tela.read_with(cx, |tela, _| {
            assert_eq!(tela.zoom(), 1.5);
            assert_eq!(tela.pagina_atual(), 2, "o zoom trocou de página");
            assert!(
                tela.desenhadas[2].is_some(),
                "a página some enquanto a nova escala não chega"
            );
        });
        esperar(&tela, cx, "redesenhar na escala nova", |tela| {
            tela.desenhadas[2]
                .as_ref()
                .is_some_and(|d| d.escala > antes * 1.4)
        });
        tela.update(cx, |tela, cx| tela.ajustar(cx));
        tela.read_with(cx, |tela, _| assert_eq!(tela.zoom(), 1.0));
    }

    /// 🔗 *"Os clicks nos links do PDF funcionam no novo visualizador?"* — o
    /// clique na área do link, na página desenhada: o endereço abre no
    /// navegador e o salto interno muda de página.
    #[gpui_kit::test]
    fn os_links_do_livro_respondem_ao_clique(cx: &mut TestAppContext) {
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = livro_em_arquivo(pasta.path(), 3);
        let (tela, cx) = abrir(arquivo, cx);
        esperar(&tela, cx, "abrir", |tela| tela.estado() == &Estado::Aberto);
        cx.run_until_parked();

        let plano = fotolivro::Plano::de(3);
        let da_capa: Vec<Destino> = tela.read_with(cx, |tela, _| {
            tela.links_da_pagina(0)
                .iter()
                .map(|l| l.destino.clone())
                .collect()
        });
        let galeria = da_capa
            .iter()
            .position(|d| *d == Destino::Endereco(GALERIA.into()))
            .expect("a capa sem o link da galeria");
        let folhear = da_capa
            .iter()
            .position(|d| *d == Destino::Pagina(plano.primeira_do_livro() - 1))
            .expect("a capa sem o \"Folhear o álbum\"");

        // A área do link fica dentro da página, onde o botão foi impresso.
        let pagina = cx.debug_bounds("pdf-palco").expect("o palco");
        let clicar = |cx: &mut VisualTestContext, qual: usize| {
            let area = cx
                .debug_bounds(Box::leak(
                    format!("pdf-link-1-{}", qual + 1).into_boxed_str(),
                ))
                .expect("a área do link não está desenhada");
            assert!(
                area.left() >= pagina.left() && area.right() <= pagina.right(),
                "o link vaza do palco: {area:?}"
            );
            cx.simulate_mouse_move(area.center(), None, Default::default());
            cx.simulate_click(area.center(), Default::default());
        };

        clicar(cx, galeria);
        assert_eq!(
            cx.opened_url().as_deref(),
            Some(GALERIA),
            "o clique no botão da galeria não abriu o endereço"
        );
        tela.read_with(cx, |tela, _| assert_eq!(tela.pagina_atual(), 0));

        clicar(cx, folhear);
        tela.read_with(cx, |tela, _| {
            assert_eq!(
                tela.pagina_atual(),
                plano.primeira_do_livro() - 1,
                "o \"Folhear o álbum\" não mudou de página"
            );
        });

        // Um esquema que não é de navegador não sai do app.
        assert!(endereco_que_abre("https://recordarfotos.com.br"));
        assert!(endereco_que_abre("  HTTP://exemplo.com"));
        assert!(endereco_que_abre("mailto:alguem@exemplo.com"));
        assert!(!endereco_que_abre("file:///etc/passwd"));
        assert!(!endereco_que_abre("javascript:alert(1)"));
    }

    /// O que não é PDF, e o arquivo que sumiu: a tela diz, do mesmo tamanho.
    #[gpui_kit::test]
    fn o_que_nao_abre_vira_aviso(cx: &mut TestAppContext) {
        let pasta = tempfile::tempdir().unwrap();
        let lixo = pasta.path().join("lixo.pdf");
        std::fs::write(&lixo, b"isto nao e um pdf").unwrap();
        let (tela, cx) = abrir(lixo, cx);
        let palco_antes = cx.debug_bounds("pdf-palco").expect("o palco, abrindo");
        esperar(&tela, cx, "recusar o lixo", |tela| {
            matches!(tela.estado(), Estado::Falhou(_))
        });
        cx.run_until_parked();
        tela.read_with(cx, |tela, _| {
            let Estado::Falhou(porque) = tela.estado() else {
                unreachable!()
            };
            assert!(
                porque.starts_with("O arquivo não é um PDF legível"),
                "{porque}"
            );
        });
        assert!(cx.debug_bounds("pdf-falhou").is_some(), "sem o aviso");
        assert_eq!(
            cx.debug_bounds("pdf-palco").unwrap().size,
            palco_antes.size,
            "o palco mudou de tamanho ao falhar"
        );
    }

    #[gpui_kit::test]
    fn o_arquivo_que_sumiu_vira_aviso(cx: &mut TestAppContext) {
        let pasta = tempfile::tempdir().unwrap();
        let (tela, cx) = abrir(pasta.path().join("nao-existe.pdf"), cx);
        esperar(&tela, cx, "avisar do arquivo que sumiu", |tela| {
            matches!(tela.estado(), Estado::Falhou(_))
        });
        tela.read_with(cx, |tela, _| {
            let Estado::Falhou(porque) = tela.estado() else {
                unreachable!()
            };
            assert!(
                porque.starts_with("O arquivo não pôde ser lido"),
                "{porque}"
            );
        });
    }
}
