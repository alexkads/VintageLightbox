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
use gpui::{
    canvas, div, img, prelude::*, px, uniform_list, App, Context, Entity, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, SharedString, Subscription, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::grade::{colunas_que_cabem, fotos_da_linha, linhas_necessarias};
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

/// A faixa de escolher fotos, no rodapé.
///
/// 🚨 **Ela é uma grade que rola na vertical, e no legado é uma fila
/// horizontal.** O GPUI virtualiza lista **vertical** (`uniform_list`) e só ela:
/// uma fila horizontal com o acervo inteiro seria 2.000 `div`s e 2.000 consultas
/// ao cache **por quadro**, que é exatamente o que a fase 1 mediu engasgando. O
/// egui não tem esse problema porque desenha em modo imediato e recorta o que
/// sai da área.
///
/// Duas linhas visíveis é o que cabe sem tirar espaço da folha, que é a razão de
/// a tela existir.
const ALTURA_DA_FAIXA: f32 = 172.0;
const LADO_DA_ESCOLHA: f32 = 72.0;
const PASSO_DA_ESCOLHA: f32 = LADO_DA_ESCOLHA + 8.0;

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
    /// Onde a foto de cada célula foi empurrada, **em milímetro**.
    ///
    /// A chave é o índice da célula na folha, como no legado (`cell_offsets`) —
    /// ou seja, o deslocamento pertence ao **lugar**, e não à foto: trocar de
    /// modelo mantém o que foi ajustado na primeira célula, e a foto que passar
    /// a ocupá-la herda o empurrão. É o comportamento de lá.
    ///
    /// ⚠️ **Milímetro, e não pixel de tela como no legado.** Guardado em pixel,
    /// redimensionar a janela mudaria o quanto a foto está deslocada **no
    /// papel** — o mesmo defeito que o papel com a forma da janela tem, de novo.
    deslocamentos: std::collections::HashMap<usize, (f32, f32)>,
    /// A célula em arrasto e a última posição do ponteiro.
    arrasto: Option<(usize, gpui::Point<gpui::Pixels>)>,
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
            deslocamentos: std::collections::HashMap::new(),
            arrasto: None,
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
        selecionadas: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        // ⚠️ **Traduz id em posição, e ignora o que não está na grade.** O que
        // chega é a seleção da Biblioteca; o acervo daqui é o que ela estava
        // mostrando. Uma foto selecionada antes de o filtro mudar não está na
        // lista — e guardá-la por id, sem posição, faria a folha ter uma célula
        // apontando para o nada.
        self.escolhidas = selecionadas
            .iter()
            .filter_map(|id| acervo.iter().position(|foto| &foto.id == id))
            .collect();
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

    /// Põe ou tira **uma** foto da coleção.
    ///
    /// 🔑 **Quem entra, entra no fim** — é o `push` do legado, e não uma inserção
    /// na ordem do acervo. A ordem da coleção é a ordem das células, então
    /// clicar em três fotos monta a folha na ordem em que se clicou; obrigar a
    /// ordem do acervo tiraria de quem escolhe a única forma que existe hoje de
    /// dizer o que vai onde.
    pub fn alternar(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        match self.escolhidas.iter().position(|&i| i == no_acervo) {
            Some(posicao) => {
                self.escolhidas.remove(posicao);
            }
            None => self.escolhidas.push(no_acervo),
        }
        cx.notify();
    }

    pub fn deslocamento_de(&self, celula: usize) -> (f32, f32) {
        self.deslocamentos
            .get(&celula)
            .copied()
            .unwrap_or((0.0, 0.0))
    }

    pub fn tem_deslocamento(&self) -> bool {
        !self.deslocamentos.is_empty()
    }

    /// "Redefinir posições" — o botão que o legado só mostra quando há o que
    /// redefinir, e que é a única saída de um empurrão que ficou torto.
    pub fn redefinir_posicoes(&mut self, cx: &mut Context<Self>) {
        self.deslocamentos.clear();
        cx.notify();
    }

    fn comecar_arrasto(
        &mut self,
        celula: usize,
        posicao: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.arrasto = Some((celula, posicao));
        cx.notify();
    }

    /// O ponteiro andou: empurra a foto da célula em arrasto.
    ///
    /// 🔑 **O delta é medido contra a posição anterior do ponteiro**, e não
    /// contra o começo do gesto. Assim o limite abaixo trunca o movimento sem
    /// desalinhar o dedo da foto no gesto seguinte.
    fn mover_arrasto(
        &mut self,
        posicao: gpui::Point<gpui::Pixels>,
        espaco: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        let Some((celula, anterior)) = self.arrasto else {
            return;
        };
        let escala = self.leiaute.escala_para(espaco);
        if escala <= 0.0 {
            return;
        }

        let dx = f32::from(posicao.x - anterior.x) / escala;
        let dy = f32::from(posicao.y - anterior.y) / escala;
        self.arrasto = Some((celula, posicao));

        let Some(retangulo) = self.celulas_da_folha().get(celula).copied() else {
            return;
        };
        let (limite_x, limite_y) = limite_do_empurrao(&retangulo);

        let (x, y) = self.deslocamento_de(celula);
        self.deslocamentos.insert(
            celula,
            (
                (x + dx).clamp(-limite_x, limite_x),
                (y + dy).clamp(-limite_y, limite_y),
            ),
        );
        cx.notify();
    }

    fn celulas_da_folha(&self) -> Vec<Celula> {
        self.leiaute.celulas()
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
        let altura = f32::from(tamanho.height) - ALTURA_DA_BARRA - ALTURA_DA_FAIXA - 2.0 * FOLGA;
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
                self.celula(indice, celula, foto, escala, cx)
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
                    .children(desenhadas)
                    // Enquanto há arrasto, quem escuta o ponteiro é a janela.
                    .child(self.ouvinte_do_arrasto(cx)),
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

    /// A faixa do rodapé: escolher foto a foto.
    ///
    /// Os quatro botões da coluna da esquerda montam a coleção em bloco; aqui é
    /// onde ela se ajusta uma foto por vez, que é o que o filmstrip do legado
    /// serve para fazer.
    fn faixa(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = f32::from(window.viewport_size().width) - 2.0 * FOLGA;
        let colunas = colunas_que_cabem(largura, PASSO_DA_ESCOLHA);
        let total = self.acervo.len();
        let linhas = linhas_necessarias(total, colunas);

        let acervo = self.acervo.clone();
        let previews = self.previews.clone();
        let cache = self.cache.clone();
        // A coleção inteira, e não só quem está na primeira folha: a marcação
        // tem de aparecer também nas fotos que caíram na folha 2 — senão elas se
        // parecem com as que não foram escolhidas.
        let escolhidas: Arc<Vec<usize>> = Arc::new(self.escolhidas.clone());
        // O closure do `uniform_list` é `'static` e recebe `&mut App`, e não
        // `&mut self`: escrever no estado exige levar a entidade junto.
        let eu = cx.entity();

        div()
            .h(px(ALTURA_DA_FAIXA))
            .flex_none()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(uniform_list(
                "escolher-para-imprimir",
                linhas,
                move |faixa, _window, cx| {
                    faixa
                        .map(|indice| {
                            let desta_linha = fotos_da_linha(indice, colunas, total);
                            div()
                                .flex()
                                .gap(px(8.))
                                .p(px(4.))
                                .children(
                                    desta_linha
                                        .map(|no_acervo| {
                                            let eu = eu.clone();
                                            escolha(
                                                &acervo[no_acervo],
                                                &previews,
                                                &cache,
                                                escolhidas.contains(&no_acervo),
                                                move |_ev, _window, cx| {
                                                    eu.update(cx, |tela, cx| {
                                                        tela.alternar(no_acervo, cx);
                                                    });
                                                },
                                                cx,
                                            )
                                        })
                                        .collect::<Vec<_>>(),
                                )
                                .into_any_element()
                        })
                        .collect()
                },
            ))
    }

    fn celula(
        &self,
        indice: usize,
        celula: &Celula,
        foto: Option<&PhotoViewModel>,
        escala: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let moldura = div()
            .absolute()
            .left(px(celula.x * escala))
            .top(px(celula.y * escala))
            .w(px(celula.largura * escala))
            .h(px(celula.altura * escala))
            // 🚨 `relative` **e** `overflow_hidden`: a foto é filha da célula e
            // se posiciona por ela, e o que sai da célula é cortado. Sem o
            // recorte, arrastar a foto a faria invadir a célula vizinha — e a
            // folha na tela deixaria de descrever a folha impressa.
            .relative()
            .overflow_hidden()
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
                let (dx, dy) = self.deslocamento_de(indice);

                moldura
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            // Relativo à **célula**, e não ao papel: a caixa que
                            // ancora o absoluto passou a ser a célula, e medir do
                            // papel deslocaria a foto de tudo o que a margem vale.
                            .left(px((dentro.x - celula.x + dx) * escala))
                            .top(px((dentro.y - celula.y + dy) * escala))
                            .w(px(dentro.largura * escala))
                            .h(px(dentro.altura * escala))
                            .child(img(imagem).size_full()),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |tela, evento: &MouseDownEvent, _window, cx| {
                            tela.comecar_arrasto(indice, evento.position, cx);
                        }),
                    )
                    .into_any_element()
            }
            // Sem preview a célula fica vazia, com a moldura. É estado normal —
            // foto recém-importada ainda não tem miniatura gravada — e não erro.
            Miniatura::Ausente => moldura.into_any_element(),
        }
    }

    /// Escuta o ponteiro enquanto há arrasto — e some quando não há.
    ///
    /// 🚨 **`window.on_mouse_event`, e não `div().on_mouse_move`**: o ouvinte de
    /// um `div` só recebe evento **dentro** dele, e empurrar a foto até encostar
    /// na borda da célula é justamente o gesto que sai dela. O arrasto morreria
    /// no meio, com a foto parada a meio caminho. É a mesma lição do overlay de
    /// corte da fase 2, e por isso também o `canvas`: registrar ouvinte de mouse
    /// exige a fase de pintura, aonde um `div` comum não chega.
    fn ouvinte_do_arrasto(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let arrastando = self.arrasto.is_some();
        let ouvinte = cx.entity();

        canvas(
            |_bounds, _window, _cx| {},
            move |_bounds, _prepaint, window, _cx| {
                if !arrastando {
                    return;
                }

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &MouseMoveEvent, fase, window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        let espaco = Self::espaco_da_folha(window);
                        esta.update(cx, |tela, cx| {
                            tela.mover_arrasto(evento.position, espaco, cx)
                        });
                    }
                });

                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, _window, cx| {
                        if !fase.bubble() {
                            return;
                        }
                        esta.update(cx, |tela, cx| {
                            tela.arrasto = None;
                            cx.notify();
                        });
                    }
                });
            },
        )
        .absolute()
        .size_full()
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
            // Como no legado: o botão só existe quando há o que redefinir. Um
            // botão permanente que quase sempre não faz nada ensina a ignorá-lo.
            .when(self.tem_deslocamento(), |coluna| {
                coluna.child(botao(
                    "redefinir-posicoes",
                    "Redefinir posições",
                    false,
                    cx.listener(|tela, _ev, _window, cx| tela.redefinir_posicoes(cx)),
                ))
            })
    }
}

impl Render for Impressao {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                // `min_h(0)` na linha, senão a faixa de baixo é empurrada para
                // fora da janela pelo conteúdo do meio — foi o que a Biblioteca
                // aprendeu com a árvore de pastas.
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.coluna_esquerda(cx))
                    .child(div().flex_1().min_w(px(0.)).child(self.folha(window, cx)))
                    .child(self.coluna_direita(cx)),
            )
            .child(self.faixa(window, cx))
    }
}

/// Uma foto da faixa: a miniatura, e se ela está na coleção.
fn escolha(
    foto: &PhotoViewModel,
    previews: &PreviewManager,
    cache: &Mutex<CacheDeMiniaturas>,
    escolhida: bool,
    ao_clicar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    cx: &App,
) -> gpui::AnyElement {
    let miniatura = cache
        .lock()
        .expect("o cache de miniaturas não deve estar envenenado")
        .obter(previews, &foto.id);

    div()
        .id(SharedString::from(format!("escolha-{}", foto.id)))
        .w(px(LADO_DA_ESCOLHA))
        .h(px(LADO_DA_ESCOLHA))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        // A borda é a marca, e ela tem **2 px** quando acesa: a diferença de cor
        // sozinha some numa faixa de miniaturas coloridas, que é justamente o
        // que enche esta faixa.
        .when(escolhida, |item| {
            item.border_2().border_color(cx.theme().primary)
        })
        .when(!escolhida, |item| {
            item.border_1().border_color(cx.theme().border)
        })
        .child(match miniatura {
            Miniatura::Pronta(imagem) => img(imagem)
                .max_w(px(LADO_DA_ESCOLHA - 6.0))
                .max_h(px(LADO_DA_ESCOLHA - 6.0))
                .into_any_element(),
            Miniatura::Ausente => div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("—")
                .into_any_element(),
        })
        .on_click(ao_clicar)
        .into_any_element()
}

/// Até onde a foto pode ser empurrada dentro da célula, em milímetro.
///
/// Um quinto da célula em cada direção, que é aproximadamente o que o legado
/// permite (lá a conta é `render_width * 0.2` mais metade do que sobra, medida
/// em pixel de tela). Sem limite, a foto sai inteira da célula e desaparece
/// atrás do recorte — e quem arrastou não tem como saber para que lado ela foi.
fn limite_do_empurrao(celula: &Celula) -> (f32, f32) {
    (celula.largura * 0.2, celula.altura * 0.2)
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
                impressao.abrir(acervo(), vec!["id-c.jpg".to_string()], cx);

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
                impressao.abrir(acervo(), Vec::new(), cx);

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
                impressao.abrir(acervo(), Vec::new(), cx);

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
                impressao.abrir(muitas, Vec::new(), cx);
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
                impressao.abrir(acervo(), Vec::new(), cx);
                impressao.definir_papel(Papel::A3, cx);
                impressao.definir_orientacao(Orientacao::Paisagem, cx);
                impressao.definir_modelo(Modelo::Grade3x3, cx);

                impressao.abrir(acervo(), vec!["id-a.jpg".to_string()], cx);

                assert_eq!(impressao.leiaute().papel, Papel::A3);
                assert_eq!(impressao.leiaute().orientacao, Orientacao::Paisagem);
                assert_eq!(impressao.leiaute().modelo, Modelo::Grade3x3);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 Clicar na faixa põe a foto **no fim** da coleção, e clicar de novo tira.
    ///
    /// A ordem da coleção é a ordem das células, então entrar no fim é o que
    /// permite montar a folha na ordem em que se clicou — é o `push` do legado.
    /// Inserir na ordem do acervo tiraria de quem escolhe a única forma que
    /// existe hoje de dizer o que vai onde.
    #[gpui::test]
    fn clicar_na_faixa_poe_no_fim_e_tira_de_onde_estiver(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), Vec::new(), cx);

                impressao.alternar(3, cx);
                impressao.alternar(1, cx);
                impressao.alternar(4, cx);
                assert_eq!(impressao.escolhidas, vec![3, 1, 4], "na ordem dos cliques");

                // Tirar a do meio não mexe nas outras duas.
                impressao.alternar(1, cx);
                assert_eq!(impressao.escolhidas, vec![3, 4]);

                // E ela volta para o fim, não para o lugar de antes.
                impressao.alternar(1, cx);
                assert_eq!(impressao.escolhidas, vec![3, 4, 1]);
            })
            .expect("a janela deve estar aberta");
    }

    /// A foto que cai na folha 2 continua marcada na faixa.
    ///
    /// ⚠️ A marcação é lida da coleção inteira, e não das fotos da folha — lê-la
    /// da folha faria tudo que passa da primeira página parecer não escolhido, e
    /// o clique seguinte tiraria da coleção o que quem clicou queria acrescentar.
    #[gpui::test]
    fn a_marcacao_vale_para_a_colecao_inteira_e_nao_so_para_a_primeira_folha(
        cx: &mut TestAppContext,
    ) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), Vec::new(), cx);
                impressao.escolher_todas(cx);
                // Modelo "Uma foto": cinco fotos, cinco folhas.
                assert_eq!(impressao.paginas(), 5);
                assert_eq!(impressao.fotos_da_folha().len(), 1);

                assert!(
                    impressao.escolhidas.contains(&4),
                    "a última está na coleção, ainda que não esteja na folha 1"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O empurrão para no limite da célula — a foto não sai por baixo do
    /// recorte.
    ///
    /// Sem limite, um arrasto longo leva a foto inteira para fora da célula: ela
    /// desaparece atrás do `overflow_hidden`, a célula fica igual a uma vazia, e
    /// quem arrastou não tem como saber para que lado ela foi nem como trazê-la
    /// de volta.
    #[gpui::test]
    fn o_empurrao_para_no_limite_da_celula(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), vec!["id-a.jpg".to_string()], cx);
                // Uma folha, uma célula: A4 retrato com 10 mm de margem.
                let celula = impressao.leiaute().celulas()[0];
                let (limite_x, limite_y) = limite_do_empurrao(&celula);

                // Escala 1 mm = 1 px deixa a conta legível.
                let espaco = impressao.leiaute().papel_mm();

                impressao.comecar_arrasto(0, gpui::point(px(0.), px(0.)), cx);
                impressao.mover_arrasto(gpui::point(px(9999.), px(9999.)), espaco, cx);

                let (x, y) = impressao.deslocamento_de(0);
                assert!(
                    (x - limite_x).abs() < 1e-3 && (y - limite_y).abs() < 1e-3,
                    "o empurrão tinha de parar no limite: {x} × {y}"
                );

                // E para o outro lado, pelo mesmo limite.
                impressao.mover_arrasto(gpui::point(px(-9999.), px(-9999.)), espaco, cx);
                let (x, y) = impressao.deslocamento_de(0);
                assert!((x + limite_x).abs() < 1e-3 && (y + limite_y).abs() < 1e-3);
            })
            .expect("a janela deve estar aberta");
    }

    /// Sem arrasto começado, mover o ponteiro não move nada.
    ///
    /// O ouvinte é da **janela**, e não da célula: ele recebe todo movimento do
    /// ponteiro enquanto está ligado. Sem esta guarda, passar o mouse pela folha
    /// depois de soltar continuaria empurrando a foto.
    #[gpui::test]
    fn sem_arrasto_o_ponteiro_nao_empurra_nada(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), vec!["id-a.jpg".to_string()], cx);
                let espaco = impressao.leiaute().papel_mm();

                impressao.mover_arrasto(gpui::point(px(50.), px(50.)), espaco, cx);

                assert_eq!(impressao.deslocamento_de(0), (0.0, 0.0));
                assert!(!impressao.tem_deslocamento());
            })
            .expect("a janela deve estar aberta");
    }

    /// "Redefinir posições" desfaz todos os empurrões — e some quando não há
    /// nenhum, como no legado.
    #[gpui::test]
    fn redefinir_posicoes_devolve_todas_as_fotos_ao_centro(cx: &mut TestAppContext) {
        let (janela, _dir) = tela(cx);

        janela
            .update(cx, |impressao, _window, cx| {
                impressao.abrir(acervo(), vec!["id-a.jpg".to_string()], cx);
                assert!(!impressao.tem_deslocamento(), "nasce sem botão");

                let espaco = impressao.leiaute().papel_mm();
                impressao.comecar_arrasto(0, gpui::point(px(0.), px(0.)), cx);
                impressao.mover_arrasto(gpui::point(px(10.), px(4.)), espaco, cx);
                assert!(impressao.tem_deslocamento());

                impressao.redefinir_posicoes(cx);

                assert_eq!(impressao.deslocamento_de(0), (0.0, 0.0));
                assert!(!impressao.tem_deslocamento(), "e o botão some de novo");
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
                impressao.abrir(acervo(), vec!["id-a.jpg".to_string()], cx);
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
