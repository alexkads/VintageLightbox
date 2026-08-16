//! A folha na tela: o papel, as células e o que decide as duas.
//!
//! A conta inteira está em [`super::pagina`], em milímetro. Aqui só se pede uma
//! escala e se multiplica — é o único lugar deste contexto que sabe o que é
//! pixel.
//!
//! ## 🚨 O que **não** foi portado, e por quê
//!
//! O painel direito do legado tem uma seção "Photo Info" com quatro caixas de
//! opção (nome do arquivo, data, câmera, exposição) e um campo "Copies". Os
//! cinco são gravados no estado e **lidos por ninguém**: `grep` nos 1.132 LOC
//! acha a escrita, os testes que afirmam que a caixa marca, e nenhum leitor. A
//! prévia nunca desenha texto nenhum debaixo da foto.
//!
//! Portá-los seria portar a promessa: quatro caixas que marcam e não mudam a
//! folha. É a mesma decisão que a fase 3 tomou com pausar e cancelar a
//! importação — melhor nascer sem o botão do que com um que não faz o que diz.
//! O mesmo vale para os botões "Print" e "Export PDF", cujo comportamento
//! inteiro é um aviso de *"coming soon"*.

use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use gpui::{div, img, prelude::*, px, App, Context, Entity, SharedString, Subscription, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::miniaturas::{CacheDeMiniaturas, Miniatura};

use super::pagina::{encaixar, Celula, Leiaute, Modelo, Orientacao, Papel};

/// Largura das duas colunas laterais.
///
/// A da esquerda é a mesma da árvore de pastas da Biblioteca — o legado usa
/// `Theme::SIDEBAR_WIDTH` nos dois lugares pelo mesmo motivo: são as duas
/// colunas de escolher, e larguras diferentes fazem a janela parecer torta ao
/// trocar de tela.
const LADO_ESQUERDO: f32 = 220.0;
const LADO_DIREITO: f32 = 240.0;

/// O respiro em volta da folha, dos dois lados de cada eixo.
const FOLGA: f32 = 24.0;

/// Os tetos dos dois campos, iguais aos do legado (`DragValue::range`).
const MARGEM_MAXIMA: f32 = 50.0;
const ESPACO_MAXIMO: f32 = 20.0;

/// Quantas miniaturas ficam na memória.
///
/// A folha mais cheia tem 48 células (6 × 8, o teto da grade personalizada), e
/// trocar de modelo troca o conjunto inteiro de uma vez. Guardar o dobro é o
/// que faz ir da folha de contato para a 2 × 2 e voltar sem redecodificar nada.
const MINIATURAS_GUARDADAS: usize = 96;

pub struct Impressao {
    /// O acervo que a Biblioteca estava mostrando na hora de entrar aqui — já
    /// filtrado, como o `filmstrip_filter.apply(&state.photos)` do legado.
    ///
    /// É uma cópia, e de propósito: os botões de coleção contam sobre o que a
    /// pessoa **estava vendo**. Ler a Biblioteca a cada quadro faria mexer num
    /// filtro lá mudar a coleção daqui sem ninguém tocar nesta tela.
    acervo: Arc<Vec<PhotoViewModel>>,
    /// Índices no acervo, na ordem em que caem no papel.
    escolhidas: Vec<usize>,
    leiaute: Leiaute,
    previews: Arc<PreviewManager>,
    cache: Arc<Mutex<CacheDeMiniaturas>>,
    margem: Entity<SliderState>,
    espaco: Entity<SliderState>,
    /// 🚨 As assinaturas moram aqui. `Subscription` descartada cancela a
    /// inscrição na hora — e os dois sliders passariam a se mover sem mexer na
    /// folha, sem erro nenhum. Foi o que a Biblioteca aprendeu com a busca.
    _assinaturas: Vec<Subscription>,
}

impl Impressao {
    pub fn nova(
        previews: Arc<PreviewManager>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let leiaute = Leiaute::default();

        let margem = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(MARGEM_MAXIMA)
                .default_value(leiaute.margem_mm)
        });
        let espaco = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(ESPACO_MAXIMO)
                .default_value(leiaute.espaco_mm)
        });

        let mut assinaturas = Vec::with_capacity(2);
        assinaturas.push(cx.subscribe_in(
            &margem,
            window,
            |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                let SliderEvent::Change(valor) = evento;
                tela.leiaute.margem_mm = valor.start();
                cx.notify();
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &espaco,
            window,
            |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                let SliderEvent::Change(valor) = evento;
                tela.leiaute.espaco_mm = valor.start();
                cx.notify();
            },
        ));

        Self {
            acervo: Arc::new(Vec::new()),
            escolhidas: Vec::new(),
            leiaute,
            previews,
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(
                std::num::NonZeroUsize::new(MINIATURAS_GUARDADAS).expect("96 não é zero"),
            ))),
            margem,
            espaco,
            _assinaturas: assinaturas,
        }
    }

    /// Entra na impressão com o que a Biblioteca estava mostrando.
    ///
    /// A coleção começa com a foto selecionada, como no legado — o botão "Print"
    /// de lá monta o estado a partir de `selected_photo_ids`, ou da única
    /// selecionada. Sem seleção ela começa vazia, e os botões de coleção estão
    /// ali para encher.
    ///
    /// ⚠️ **O leiaute sobrevive à saída e à volta.** Papel, margem e modelo são
    /// escolhas sobre o papel, não sobre a foto: refazer as quatro a cada ida à
    /// Biblioteca seria perder o ajuste no caminho de conferir uma foto.
    pub fn abrir(
        &mut self,
        acervo: Vec<PhotoViewModel>,
        selecionada: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.escolhidas = match selecionada {
            Some(id) => acervo
                .iter()
                .position(|foto| foto.id == id)
                .into_iter()
                .collect(),
            None => Vec::new(),
        };
        self.acervo = Arc::new(acervo);
        cx.notify();
    }

    pub fn leiaute(&self) -> Leiaute {
        self.leiaute
    }

    pub fn escolhidas(&self) -> usize {
        self.escolhidas.len()
    }

    pub fn paginas(&self) -> usize {
        self.leiaute.paginas(self.escolhidas.len())
    }

    pub fn definir_modelo(&mut self, modelo: Modelo, cx: &mut Context<Self>) {
        self.leiaute.modelo = modelo;
        cx.notify();
    }

    pub fn definir_papel(&mut self, papel: Papel, cx: &mut Context<Self>) {
        self.leiaute.papel = papel;
        cx.notify();
    }

    pub fn definir_orientacao(&mut self, orientacao: Orientacao, cx: &mut Context<Self>) {
        self.leiaute.orientacao = orientacao;
        cx.notify();
    }

    /// Muda a grade personalizada — e **escolhe o modelo junto**.
    ///
    /// Sem isso, mexer nos campos de uma grade que não está selecionada não faria
    /// nada visível, e o número mudaria na tela mostrando que respondeu.
    pub fn definir_grade(&mut self, colunas: u8, linhas: u8, cx: &mut Context<Self>) {
        self.leiaute.modelo = Modelo::Personalizada;
        self.leiaute.colunas = colunas.clamp(1, 6);
        self.leiaute.linhas = linhas.clamp(1, 8);
        cx.notify();
    }

    /// Todas as fotos que a Biblioteca estava mostrando.
    pub fn escolher_todas(&mut self, cx: &mut Context<Self>) {
        self.escolhidas = (0..self.acervo.len()).collect();
        cx.notify();
    }

    pub fn escolher_nenhuma(&mut self, cx: &mut Context<Self>) {
        self.escolhidas.clear();
        cx.notify();
    }

    /// Só as marcadas como boas — o `flag == Some(1)` do legado.
    pub fn escolher_sinalizadas(&mut self, cx: &mut Context<Self>) {
        self.escolhidas = self
            .acervo
            .iter()
            .enumerate()
            .filter(|(_, foto)| foto.flag == Some(1))
            .map(|(i, _)| i)
            .collect();
        cx.notify();
    }

    /// 🔑 **Inverter preserva a ordem do acervo**, e o legado não.
    ///
    /// Lá a inversão é `HashSet::difference`, cuja ordem de saída é a do hash:
    /// inverter duas vezes devolve a mesma coleção **embaralhada**, e como a
    /// posição na lista é o que decide a célula, as fotos trocam de lugar na
    /// folha sem ninguém ter mexido no leiaute.
    pub fn inverter(&mut self, cx: &mut Context<Self>) {
        let dentro: std::collections::HashSet<usize> = self.escolhidas.iter().copied().collect();
        self.escolhidas = (0..self.acervo.len())
            .filter(|i| !dentro.contains(i))
            .collect();
        cx.notify();
    }

    /// As fotos da primeira folha — a única que o legado desenha, e a única que
    /// esta tela oferece (§ fase 4 do plano: botão de página seria feature nova).
    fn fotos_da_folha(&self) -> Vec<&PhotoViewModel> {
        self.leiaute
            .fotos_da_pagina(&self.escolhidas, 0)
            .iter()
            .map(|i| &self.acervo[*i])
            .collect()
    }

    /// O que sobra para a folha depois das duas colunas.
    ///
    /// Vem de `window.viewport_size()`, como a largura útil da grade da
    /// Biblioteca: a conta responde **antes** do primeiro desenho. Medir o filho
    /// exigiria um quadro com o número errado para descobrir o certo — e é nesse
    /// quadro que a escala receberia zero e a folha sumiria.
    fn espaco_da_folha(window: &Window) -> (f32, f32) {
        const ALTURA_DA_BARRA: f32 = 34.0;
        let tamanho = window.viewport_size();
        let largura = f32::from(tamanho.width) - LADO_ESQUERDO - LADO_DIREITO - 2.0 * FOLGA;
        let altura = f32::from(tamanho.height) - ALTURA_DA_BARRA - 2.0 * FOLGA;
        (largura.max(0.0), altura.max(0.0))
    }

    /// A folha branca, com uma célula por foto.
    fn folha(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let escala = self.leiaute.escala_para(Self::espaco_da_folha(window));
        let (largura_mm, altura_mm) = self.leiaute.papel_mm();
        let celulas = self.leiaute.celulas();
        let fotos = self.fotos_da_folha();

        let desenhadas: Vec<gpui::AnyElement> = celulas
            .iter()
            .enumerate()
            .map(|(indice, celula)| {
                let foto = fotos.get(indice).copied();
                self.celula(celula, foto, escala)
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .size_full()
            .p(px(FOLGA))
            .child(
                // 🚨 A caixa `relative` é a folha **sem respiro nenhum**: as
                // margens já vêm da geometria, em milímetro. Um padding aqui
                // somaria à margem e faria o número do campo deixar de descrever
                // a distância na tela — foi o erro que o overlay de corte da fase
                // 2 encontrou com os 24px da moldura.
                div()
                    .relative()
                    .w(px(largura_mm * escala))
                    .h(px(altura_mm * escala))
                    .bg(gpui::white())
                    .shadow_md()
                    .children(desenhadas),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(self.rodape())),
            )
    }

    /// O rodapé diz **quantas folhas**, e quantas fotos ficaram de fora.
    ///
    /// ⚠️ O legado escreve "Page 1 of 7" — o que promete seis folhas que não têm
    /// como ser vistas. Enquanto não houver navegação, o texto honesto é o que
    /// diz o que está na tela e o que não está.
    fn rodape(&self) -> String {
        let paginas = self.paginas();
        match paginas {
            0 => "Nenhuma foto escolhida".to_string(),
            1 => format!(
                "{} foto{} em 1 folha",
                self.escolhidas.len(),
                plural(self.escolhidas.len())
            ),
            _ => {
                let na_folha = self.leiaute.fotos_por_pagina();
                format!(
                    "{} fotos em {paginas} folhas — a primeira mostra {na_folha}",
                    self.escolhidas.len()
                )
            }
        }
    }

    fn celula(
        &self,
        celula: &Celula,
        foto: Option<&PhotoViewModel>,
        escala: f32,
    ) -> gpui::AnyElement {
        let moldura = div()
            .absolute()
            .left(px(celula.x * escala))
            .top(px(celula.y * escala))
            .w(px(celula.largura * escala))
            .h(px(celula.altura * escala))
            // O contorno da célula vazia é o do legado: cinza claro sobre o
            // branco do papel, para a grade ser visível antes de haver foto.
            .border_1()
            .border_color(gpui::rgb(0xd0d0d0))
            .bg(gpui::rgb(0xf0f0f0));

        let Some(foto) = foto else {
            return moldura.into_any_element();
        };

        let miniatura = self
            .cache
            .lock()
            .expect("o cache de miniaturas não deve estar envenenado")
            .obter(&self.previews, &foto.id);

        match miniatura {
            Miniatura::Pronta(imagem) => {
                let tamanho = imagem.size(0);
                let aspecto = tamanho.width.0 as f32 / tamanho.height.0 as f32;
                // 🔑 A foto vai no retângulo que a geometria calculou, e não num
                // `object_fit` do contêiner: é a mesma conta que decide onde ela
                // cairia no papel de verdade, e é ela que tem teste.
                let dentro = encaixar(celula, aspecto);

                div()
                    .absolute()
                    .left(px(dentro.x * escala))
                    .top(px(dentro.y * escala))
                    .w(px(dentro.largura * escala))
                    .h(px(dentro.altura * escala))
                    .child(img(imagem).size_full())
                    .into_any_element()
            }
            // Sem preview a célula fica vazia, com a moldura. É estado normal —
            // foto recém-importada ainda não tem miniatura gravada — e não erro.
            Miniatura::Ausente => moldura.into_any_element(),
        }
    }

    fn coluna_esquerda(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (colunas, linhas) = self.leiaute.grade();

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .w(px(LADO_ESQUERDO))
            .flex_none()
            .p(px(12.))
            .border_r_1()
            .border_color(cx.theme().border)
            .child(secao("Modelos"))
            .children(Modelo::TODOS.map(|modelo| {
                let aceso = self.leiaute.modelo == modelo;
                botao(
                    &format!("modelo-{modelo:?}"),
                    modelo.nome(),
                    aceso,
                    cx.listener(move |tela, _ev, _window, cx| tela.definir_modelo(modelo, cx)),
                )
            }))
            .child(secao("Grade personalizada"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(contador(
                        "colunas",
                        "Colunas",
                        colunas,
                        cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_grade(colunas.saturating_sub(1).max(1), linhas, cx)
                        }),
                        cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_grade(colunas + 1, linhas, cx)
                        }),
                        cx,
                    ))
                    .child(contador(
                        "linhas",
                        "Linhas",
                        linhas,
                        cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_grade(colunas, linhas.saturating_sub(1).max(1), cx)
                        }),
                        cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_grade(colunas, linhas + 1, cx)
                        }),
                        cx,
                    )),
            )
            .child(secao("Coleção"))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(format!(
                        "{} de {} fotos",
                        self.escolhidas.len(),
                        self.acervo.len()
                    ))),
            )
            .child(botao(
                "todas",
                "Todas",
                false,
                cx.listener(|tela, _ev, _window, cx| tela.escolher_todas(cx)),
            ))
            .child(botao(
                "nenhuma",
                "Nenhuma",
                false,
                cx.listener(|tela, _ev, _window, cx| tela.escolher_nenhuma(cx)),
            ))
            .child(botao(
                "sinalizadas",
                "Só as sinalizadas",
                false,
                cx.listener(|tela, _ev, _window, cx| tela.escolher_sinalizadas(cx)),
            ))
            .child(botao(
                "inverter",
                "Inverter",
                false,
                cx.listener(|tela, _ev, _window, cx| tela.inverter(cx)),
            ))
    }

    fn coluna_direita(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .w(px(LADO_DIREITO))
            .flex_none()
            .p(px(12.))
            .border_l_1()
            .border_color(cx.theme().border)
            .child(secao("Papel"))
            .children(Papel::TODOS.map(|papel| {
                let aceso = self.leiaute.papel == papel;
                botao(
                    &format!("papel-{papel:?}"),
                    papel.nome(),
                    aceso,
                    cx.listener(move |tela, _ev, _window, cx| tela.definir_papel(papel, cx)),
                )
            }))
            .child(secao("Orientação"))
            .child(div().flex().gap(px(4.)).children(
                [Orientacao::Retrato, Orientacao::Paisagem].map(|orientacao| {
                    let aceso = self.leiaute.orientacao == orientacao;
                    botao(
                        &format!("orientacao-{orientacao:?}"),
                        orientacao.nome(),
                        aceso,
                        cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_orientacao(orientacao, cx)
                        }),
                    )
                }),
            ))
            .child(secao("Medidas"))
            .child(medida("Margem", self.leiaute.margem_mm, &self.margem, cx))
            .child(medida("Espaço", self.leiaute.espaco_mm, &self.espaco, cx))
    }
}

impl Render for Impressao {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.coluna_esquerda(cx))
            .child(div().flex_1().min_w(px(0.)).child(self.folha(window, cx)))
            .child(self.coluna_direita(cx))
    }
}

fn plural(quantas: usize) -> &'static str {
    if quantas == 1 {
        ""
    } else {
        "s"
    }
}

fn secao(titulo: &str) -> impl IntoElement {
    div()
        .pt(px(6.))
        .text_xs()
        .child(SharedString::from(titulo.to_string()))
}

/// O mesmo botão da barra da Biblioteca, pela mesma razão: o aceso vira
/// `primary`, porque o `selected` do secundário são dois cinzas a 5% de
/// distância e numa lista de seis modelos isso é o mesmo que não marcar nenhum.
fn botao(
    id: &str,
    texto: &str,
    aceso: bool,
    ao_clicar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Button {
    Button::new(SharedString::from(id.to_string()))
        .label(SharedString::from(texto.to_string()))
        .xsmall()
        .when(aceso, |b| b.primary())
        .selected(aceso)
        .on_click(ao_clicar)
}

/// Um número com um botão de cada lado.
///
/// O legado usa `DragValue`, que arrasta para mudar — e não tem equivalente no
/// `gpui-component`. Dois botões dizem a mesma coisa em menos passos e sem
/// inventar um componente de arrasto para dois campos que vão de 1 a 8.
fn contador(
    id: &str,
    rotulo: &str,
    valor: u8,
    ao_diminuir: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ao_aumentar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .text_xs()
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(rotulo.to_string())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(
                    Button::new(SharedString::from(format!("menos-{id}")))
                        .label("−")
                        .xsmall()
                        .on_click(ao_diminuir),
                )
                .child(SharedString::from(valor.to_string()))
                .child(
                    Button::new(SharedString::from(format!("mais-{id}")))
                        .label("+")
                        .xsmall()
                        .on_click(ao_aumentar),
                ),
        )
}

/// Um slider em milímetro, com o valor ao lado do rótulo — o mesmo arranjo dos
/// 42 controles da Revelação, pelo mesmo motivo: dentro da barra o número se
/// move junto com o punho e foge de quem tenta lê-lo.
fn medida(rotulo: &str, valor: f32, estado: &Entity<SliderState>, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(SharedString::from(rotulo.to_string()))
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!("{valor:.0} mm"))),
                ),
        )
        .child(Slider::new(estado).horizontal())
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use tempfile::TempDir;

    fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (
            Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
            dir,
        )
    }

    fn foto(nome: &str, flag: Option<i32>) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            path: format!("/fotos/{nome}"),
            flag,
            ..Default::default()
        }
    }

    fn acervo() -> Vec<PhotoViewModel> {
        vec![
            foto("a.jpg", Some(1)),
            foto("b.jpg", None),
            foto("c.jpg", Some(1)),
            foto("d.jpg", Some(-1)),
            foto("e.jpg", None),
        ]
    }

    fn tela(cx: &mut TestAppContext) -> (gpui::WindowHandle<Impressao>, TempDir) {
        let (previews, dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let janela = cx.add_window(|window, cx| Impressao::nova(previews, window, cx));
        (janela, dir)
    }

    /// Entrar na impressão leva a foto que estava selecionada — e só ela.
    #[gpui::test]
    fn abrir_comeca_com_a_foto_selecionada(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), Some("id-c.jpg".to_string()), cx);

                assert_eq!(impressao.escolhidas(), 1);
                assert_eq!(impressao.paginas(), 1);
            })
            .expect("a janela deve estar aberta");
    }

    /// Sem seleção a coleção começa vazia — e zero foto é **zero folha**.
    ///
    /// Uma folha em branco na tela diria que há algo para imprimir.
    #[gpui::test]
    fn sem_selecao_nao_ha_folha(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), None, cx);

                assert_eq!(impressao.escolhidas(), 0);
                assert_eq!(impressao.paginas(), 0);
                assert_eq!(impressao.rodape(), "Nenhuma foto escolhida");
            })
            .expect("a janela deve estar aberta");
    }

    /// Os botões de coleção contam sobre o que a Biblioteca estava mostrando.
    #[gpui::test]
    fn os_botoes_de_colecao_escolhem_sobre_o_acervo_visivel(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), None, cx);

                impressao.escolher_todas(cx);
                assert_eq!(impressao.escolhidas(), 5);

                impressao.escolher_sinalizadas(cx);
                assert_eq!(impressao.escolhidas(), 2, "só as marcadas como boas");

                impressao.escolher_nenhuma(cx);
                assert_eq!(impressao.escolhidas(), 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Inverter duas vezes devolve a coleção **na mesma ordem**.
    ///
    /// No legado a inversão passa por `HashSet::difference`, e a ordem de saída
    /// é a do hash. Como é a posição na lista que decide em qual célula a foto
    /// cai, inverter e desinverter reembaralha a folha inteira sem ninguém ter
    /// tocado no leiaute — e o desenho novo parece tão certo quanto o anterior.
    #[gpui::test]
    fn inverter_duas_vezes_devolve_a_mesma_ordem(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        // ⚠️ **40 fotos, e não as 5 de sempre.** Escrito com cinco, este teste
        // **passa** com a inversão do legado no lugar: a ordem de um `HashSet`
        // pequeno de inteiros sai crescente com frequência alta demais.
        // Conferido quebrando de propósito, que é a única forma de saber.
        let muitas: Vec<PhotoViewModel> = (0..40)
            .map(|i| foto(&format!("{i:02}.jpg"), (i % 3 == 0).then_some(1)))
            .collect();

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(muitas, None, cx);
                impressao.escolher_sinalizadas(cx);
                let antes = impressao.escolhidas.clone();
                assert_eq!(antes.len(), 14, "uma a cada três, a começar da primeira");

                impressao.inverter(cx);
                let complemento: Vec<usize> = (0..40).filter(|i| i % 3 != 0).collect();
                assert_eq!(
                    impressao.escolhidas, complemento,
                    "o complemento, na ordem do acervo"
                );

                impressao.inverter(cx);
                assert_eq!(impressao.escolhidas, antes);
            })
            .expect("a janela deve estar aberta");
    }

    /// Mexer nos campos da grade personalizada **seleciona** o modelo.
    ///
    /// Sem isso, apertar "+" numa grade que não está escolhida mudaria o número
    /// na tela e não mudaria a folha — o controle responderia sem responder.
    #[gpui::test]
    fn mexer_na_grade_escolhe_o_modelo_personalizado(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                assert_eq!(impressao.leiaute().modelo, Modelo::Unica);

                impressao.definir_grade(3, 4, cx);

                assert_eq!(impressao.leiaute().modelo, Modelo::Personalizada);
                assert_eq!(impressao.leiaute().grade(), (3, 4));
                assert_eq!(impressao.leiaute().fotos_por_pagina(), 12);
            })
            .expect("a janela deve estar aberta");
    }

    /// O leiaute sobrevive a sair e voltar: papel e margem são escolhas sobre o
    /// papel, e não sobre a foto.
    #[gpui::test]
    fn abrir_de_novo_nao_desfaz_o_leiaute(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), None, cx);
                impressao.definir_papel(Papel::A3, cx);
                impressao.definir_orientacao(Orientacao::Paisagem, cx);
                impressao.definir_modelo(Modelo::Grade3x3, cx);

                impressao.abrir(acervo(), Some("id-a.jpg".to_string()), cx);

                assert_eq!(impressao.leiaute().papel, Papel::A3);
                assert_eq!(impressao.leiaute().orientacao, Orientacao::Paisagem);
                assert_eq!(impressao.leiaute().modelo, Modelo::Grade3x3);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Redimensionar a janela muda o **tamanho** da folha, nunca a forma.
    ///
    /// É o defeito do legado medido do lado de cá: lá a folha é `0,8 × 0,9` do
    /// espaço disponível, então esticar a janela na horizontal engorda o papel.
    /// Aqui a única coisa que a janela decide é a escala.
    #[gpui::test]
    fn a_janela_muda_o_tamanho_da_folha_e_nao_a_forma(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), Some("id-a.jpg".to_string()), cx);
            })
            .expect("a janela deve estar aberta");

        let visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        let mut forma = |visual: &gpui::VisualTestContext, largura, altura| {
            visual.simulate_resize(gpui::size(px(largura), px(altura)));
            janela
                .update(cx, |impressao, window, _cx| {
                    let escala = impressao
                        .leiaute()
                        .escala_para(Impressao::espaco_da_folha(window));
                    let (l, a) = impressao.leiaute().papel_mm();
                    (l * escala, a * escala)
                })
                .expect("a janela deve estar aberta")
        };

        let estreita = forma(&visual, 900.0, 700.0);
        let larga = forma(&visual, 1600.0, 700.0);

        assert!(larga.0 > 0.0 && estreita.0 > 0.0, "a folha tem de aparecer");
        assert!(
            (estreita.0 / estreita.1 - larga.0 / larga.1).abs() < 1e-3,
            "a proporção do papel não pode acompanhar a da janela"
        );
    }

    /// 🚨 O slider de margem chega mesmo ao leiaute.
    ///
    /// A assinatura é a peça que some sem avisar: um `let _ = cx.subscribe(...)`
    /// compila, o slider se move, e a folha não muda. É o mesmo defeito que a
    /// busca da Biblioteca teve.
    #[gpui::test]
    fn arrastar_a_margem_muda_a_folha(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                // O `Change` que o `Slider` emite ao ser puxado. 🚨
                // `SliderState::set_value` **não** emite — só o caminho do
                // ponteiro publica —, e um teste escrito com ele passaria com a
                // inscrição morta. Foi o que custou dois testes na fase 2.
                impressao.margem.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(
                        gpui_component::slider::SliderValue::Single(25.0),
                    ));
                });
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |impressao, _window, _cx| {
                assert_eq!(impressao.leiaute().margem_mm, 25.0);
            })
            .expect("a janela deve estar aberta");
    }
}
