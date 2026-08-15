//! A tela de Revelação: a foto à esquerda, os ajustes à direita.
//!
//! É onde o motor de [`super::processador`] encosta na interface. O caminho de
//! um arrasto:
//!
//! ```text
//! slider → SliderEvent::Change → Ajustes → Pedido → (thread wgpu) → Resultado → RenderImage
//! ```
//!
//! ⚠️ Nenhuma etapa disso acontece no `render`. O `render` só desenha o que já
//! chegou — é o que permite arrastar liso enquanto a GPU trabalha atrás.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui::{
    div, img, prelude::*, px, App, Context, Entity, RenderImage, SharedString, Subscription, Window,
};
use gpui_component::collapsible::Collapsible;
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::ActiveTheme;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::imagem::para_gpui;

use super::controles::{Definicao, Secao, CONTROLES};
use super::processador::{Ajustes, Pedido, Processador};

/// Largura do painel de ajustes.
const LADO_DO_PAINEL: f32 = 280.0;

/// De quanto em quanto a tela pergunta se a GPU já respondeu.
///
/// Metade de um quadro a 60fps. Mais curto gastaria acordada à toa; mais longo
/// somaria latência visível ao arrasto, que é o único momento em que isto
/// importa. E o laço **só existe enquanto há pedido pendente** — parado, a tela
/// não acorda nenhuma vez.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(8);

pub struct Revelacao {
    previews: Arc<PreviewManager>,
    processador: Processador,
    aberta: Option<Aberta>,
    ajustes: Ajustes,
    controles: Vec<Controle>,
    /// Quais seções estão abertas. Um conjunto, e não um `bool` por seção:
    /// acrescentar seção nova não pode exigir lembrar de acrescentar campo.
    abertas: HashSet<Secao>,
    /// O id do pedido que ainda não voltou. `None` é "a tela está em dia".
    aguardando: Option<u64>,
    /// Se já existe um laço de colheita rodando. Sem esta trava, cada arrasto
    /// abriria um laço novo e a tela acabaria com dezenas deles perguntando a
    /// mesma coisa.
    colhendo: bool,
    _assinaturas: Vec<Subscription>,
}

struct Controle {
    definicao: &'static Definicao,
    estado: Entity<SliderState>,
}

struct Aberta {
    foto: PhotoViewModel,
    /// Os pixels de origem, prontos para subir para a GPU.
    ///
    /// `None` quando o cache não tem nada gravado — não é erro, é foto ainda não
    /// processada. Sem origem não há o que revelar, e os sliders não têm sobre o
    /// que agir.
    origem: Option<Origem>,
    /// O que está desenhado agora: a revelada, ou a original enquanto o primeiro
    /// resultado não voltou.
    desenhada: Option<Arc<RenderImage>>,
}

struct Origem {
    /// `Arc` porque é a **identidade** dele que diz à thread se a textura
    /// precisa subir de novo. Um `Arc` novo a cada arrasto reenviaria a foto
    /// inteira para a GPU a cada milímetro de slider.
    pixels: Arc<Vec<u8>>,
    largura: u32,
    altura: u32,
}

impl Revelacao {
    pub fn nova(
        previews: Arc<PreviewManager>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut controles = Vec::with_capacity(CONTROLES.len());
        let mut assinaturas = Vec::with_capacity(CONTROLES.len());

        for definicao in CONTROLES {
            let estado = cx.new(|_| {
                SliderState::new()
                    .min(definicao.minimo)
                    .max(definicao.maximo)
                    .default_value(definicao.neutro())
            });

            // Uma assinatura por controle, e todas guardadas: `Subscription`
            // descartada cancela a inscrição na hora, e o slider passaria a se
            // mover sem mover a foto — sem erro nenhum.
            assinaturas.push(cx.subscribe_in(
                &estado,
                window,
                move |tela: &mut Self, _estado, evento: &SliderEvent, _window, cx| {
                    let SliderEvent::Change(valor) = evento;
                    (definicao.aplicar)(&mut tela.ajustes, valor.start());
                    tela.pedir_revelacao(cx);
                },
            ));

            controles.push(Controle { definicao, estado });
        }

        Self {
            previews,
            processador: Processador::novo(),
            aberta: None,
            ajustes: Ajustes::default(),
            controles,
            abertas: Secao::TODAS
                .into_iter()
                .filter(Secao::nasce_aberta)
                .collect(),
            aguardando: None,
            colhendo: false,
            _assinaturas: assinaturas,
        }
    }

    /// Abre uma foto para revelar.
    ///
    /// ⚠️ **Lê e decodifica na thread da interface.** Um preview "Large" é um
    /// JPEG de alguns milissegundos, então isto não trava de forma perceptível —
    /// mas quando a Revelação passar a carregar o RAW em resolução plena, este é
    /// o ponto que tem de virar assíncrono.
    pub fn abrir(&mut self, foto: PhotoViewModel, window: &mut Window, cx: &mut Context<Self>) {
        // Preview primeiro, miniatura como queda. A miniatura fica borrada numa
        // tela inteira, e é de propósito: mostrar a foto em tamanho errado é
        // melhor do que mostrar retângulo vazio.
        let bruta = self
            .previews
            .get_preview(&foto.id)
            .or_else(|| self.previews.get_thumbnail(&foto.id));

        let origem = bruta.as_ref().map(|imagem| {
            let rgba = imagem.to_rgba8();
            Origem {
                largura: rgba.width(),
                altura: rgba.height(),
                pixels: Arc::new(rgba.into_raw()),
            }
        });

        self.aberta = Some(Aberta {
            foto,
            origem,
            desenhada: bruta.map(para_gpui),
        });

        // Foto nova entra com os ajustes no neutro. Herdar o que estava no
        // slider anterior aplicaria a revelação de uma foto em outra — e a
        // segunda abriria alterada sem ninguém ter tocado em nada.
        //
        // Nada é pedido à GPU aqui: no neutro o resultado é a própria origem,
        // que já está desenhada. Uma volta inteira para receber o que se tem é
        // latência sem contrapartida.
        self.ajustes = Ajustes::default();
        self.aguardando = None;
        for controle in &self.controles {
            let neutro = controle.definicao.neutro();
            controle
                .estado
                .update(cx, |estado, cx| estado.set_value(neutro, window, cx));
        }

        cx.notify();
    }

    pub fn foto(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
    }

    pub fn ajustes(&self) -> Ajustes {
        self.ajustes
    }

    /// Manda os ajustes de agora para a GPU.
    fn pedir_revelacao(&mut self, cx: &mut Context<Self>) {
        let Some(Aberta {
            origem: Some(origem),
            ..
        }) = &self.aberta
        else {
            return;
        };

        let id = self.processador.proximo_id();
        self.processador.pedir(Pedido {
            id,
            pixels: origem.pixels.clone(),
            largura: origem.largura,
            altura: origem.altura,
            ajustes: self.ajustes,
        });
        self.aguardando = Some(id);
        self.acompanhar(cx);
        cx.notify();
    }

    /// Liga o laço que pergunta pelo resultado, se ainda não houver um.
    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;

        cx.spawn(async move |esta, cx| {
            loop {
                cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
                // `update` falha quando a tela morreu — fechar a janela no meio
                // de um arrasto não pode deixar um laço rodando sozinho.
                let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                    break;
                };
                if !continua {
                    break;
                }
            }
        })
        .detach();
    }

    /// Pega o resultado mais recente, se houver. Devolve se vale continuar
    /// perguntando.
    fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(resultado) = self.processador.colher() {
            if let Some(aberta) = self.aberta.as_mut() {
                aberta.desenhada = Some(para_gpui(resultado.imagem));
            }
            // Só larga a espera se o que voltou é o último pedido. No meio de um
            // arrasto chegam resultados de valores já ultrapassados, e parar de
            // colher ali deixaria a foto congelada num ajuste que o dedo já
            // passou.
            if self.aguardando == Some(resultado.id) {
                self.aguardando = None;
            }
            cx.notify();
        }

        let continua = self.aguardando.is_some();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn palco(&self, cx: &App) -> gpui::AnyElement {
        let moldura = div()
            .flex()
            .flex_1()
            .min_w(px(0.))
            .items_center()
            .justify_center()
            // O palco é neutro e escuro: o olho julga exposição por comparação
            // com o que está em volta, e entorno mais claro que a foto faz toda
            // foto parecer subexposta.
            .bg(cx.theme().background)
            .p(px(24.));

        match self.aberta.as_ref() {
            None => moldura
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Escolha uma foto na Biblioteca"),
                )
                .into_any_element(),
            // `ObjectFit::Contain` é o padrão do `img`: a foto cabe inteira, sem
            // recorte. `Cover` cortaria — e num estúdio de retrato o recorte
            // centralizado tira a cabeça primeiro.
            Some(Aberta {
                desenhada: Some(imagem),
                ..
            }) => moldura
                .child(img(imagem.clone()).size_full())
                .into_any_element(),
            Some(Aberta { foto, .. }) => moldura
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!(
                            "{} não tem preview no cache",
                            foto.name
                        ))),
                )
                .into_any_element(),
        }
    }

    fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sem GPU não há revelação, e o painel diz isso em vez de oferecer
        // sliders que não movem nada. `None` é "a thread ainda está abrindo o
        // dispositivo" — não é ausência de placa, e anunciar ausência durante os
        // milissegundos de abertura seria mentir em toda abertura.
        let sem_motor = self.processador.disponivel() == Some(false);

        div()
            .id("painel-de-ajustes")
            .flex()
            .flex_col()
            .gap(px(6.))
            .w(px(LADO_DO_PAINEL))
            .h_full()
            .p(px(12.))
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
            .border_l_1()
            .border_color(cx.theme().border)
            .when(sem_motor, |painel| {
                painel.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child("Sem GPU disponível — os ajustes não são aplicados"),
                )
            })
            .children(
                Secao::TODAS
                    .into_iter()
                    .map(|secao| self.secao(secao, cx))
                    .collect::<Vec<_>>(),
            )
    }

    /// Uma seção sanfonada: o cabeçalho sempre, os controles só quando aberta.
    ///
    /// Fechada por padrão (menos o Básico), como no legado. São 42 controles: com
    /// tudo aberto o painel vira uma coluna de dois metros, e o efeito prático é
    /// nenhum deles ser encontrado.
    fn secao(&self, secao: Secao, cx: &mut Context<Self>) -> impl IntoElement {
        let aberta = self.abertas.contains(&secao);

        Collapsible::new()
            .open(aberta)
            .child(
                div()
                    .id(SharedString::from(format!("secao-{}", secao.rotulo())))
                    .flex()
                    .items_center()
                    .justify_between()
                    .py(px(4.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(secao.rotulo())
                    // Triângulo, e não texto: é o que diz "isto abre" sem
                    // ocupar largura numa coluna de 280px.
                    .child(if aberta { "▾" } else { "▸" })
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        if !tela.abertas.remove(&secao) {
                            tela.abertas.insert(secao);
                        }
                        cx.notify();
                    })),
            )
            .content(
                div().flex().flex_col().gap(px(8.)).pb(px(8.)).children(
                    self.controles
                        .iter()
                        .filter(|controle| controle.definicao.secao == secao)
                        .map(|controle| {
                            let definicao = controle.definicao;
                            let valor = (definicao.ler)(&self.ajustes);

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
                                        .child(definicao.rotulo)
                                        // O valor fica ao lado do rótulo, e
                                        // não dentro da barra: dentro, ele se
                                        // move junto com o punho e vira um
                                        // número que foge de quem tenta lê-lo.
                                        .child(
                                            div().text_color(cx.theme().muted_foreground).child(
                                                SharedString::from(definicao.formatar(valor)),
                                            ),
                                        ),
                                )
                                .child(Slider::new(&controle.estado).horizontal())
                        })
                        .collect::<Vec<_>>(),
                ),
            )
    }
}

impl Render for Revelacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.palco(cx))
            .child(self.painel(cx))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (
            Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
            dir,
        )
    }

    fn foto_cinza() -> DynamicImage {
        let mut img = RgbaImage::new(8, 8);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([100, 100, 100, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn foto(nome: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            path: format!("/fotos/{nome}"),
            ..Default::default()
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        previews: Arc<PreviewManager>,
    ) -> gpui::WindowHandle<Revelacao> {
        cx.update(gpui_component::init);
        cx.add_window(move |window, cx| Revelacao::nova(previews, window, cx))
    }

    #[gpui::test]
    fn abrir_traz_o_preview_do_cache(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("retrato.jpg"));
                assert!(tela.aberta.as_ref().unwrap().desenhada.is_some());
                assert!(tela.aberta.as_ref().unwrap().origem.is_some());
            })
            .expect("a janela deve estar aberta");
    }

    /// Foto sem nada no cache abre assim mesmo — e sem origem para revelar.
    #[gpui::test]
    fn foto_sem_cache_abre_sem_imagem(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        let janela = janela(cx, previews);

        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("sem-cache.NEF"), window, cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("sem-cache.NEF"));
                assert!(tela.aberta.as_ref().unwrap().desenhada.is_none());
                assert!(tela.aberta.as_ref().unwrap().origem.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// Simula um arrasto: o `Change` que o `Slider` emite ao ser puxado.
    ///
    /// 🚨 **`SliderState::set_value` não serve para isso — ele não emite.** Só
    /// `update_value_by_position`, o caminho do ponteiro, publica `Change`.
    /// Emitir o evento à mão é o que exercita a inscrição de verdade; usar
    /// `set_value` daria um teste que passa com o assinante morto.
    fn arrastar(
        cx: &mut TestAppContext,
        janela: &gpui::WindowHandle<Revelacao>,
        controle: usize,
        valor: f32,
    ) {
        janela
            .update(cx, |tela, _window, cx| {
                let estado = tela.controles[controle].estado.clone();
                estado.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(
                        gpui_component::slider::SliderValue::Single(valor),
                    ));
                });
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();
    }

    /// 🚨 Mexer no slider chega até os `Ajustes`.
    ///
    /// É a mesma solda do campo de busca, e o mesmo modo de falha: são 11
    /// `Subscription` guardadas num `Vec`, e descartá-las faz **todos** os
    /// sliders se moverem sem mover a foto — sem erro, sem aviso, arrastando
    /// normalmente.
    #[gpui::test]
    fn arrastar_o_slider_escreve_nos_ajustes(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 1.5);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.ajustes().exposure, 1.5);
                assert!(
                    tela.aguardando.is_some(),
                    "mexer no slider tem de virar pedido à GPU"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Abrir uma foto **não** dispara os 11 assinantes.
    ///
    /// `abrir` chama `set_value` em todos os controles para devolvê-los ao
    /// neutro, e isso só é barato porque `set_value` **não emite** `Change`. É o
    /// oposto do `InputState::set_value` da busca, que emite — mesmo nome, dois
    /// comportamentos, na mesma biblioteca.
    ///
    /// Este teste prende essa diferença. Se uma versão nova do `gpui-component`
    /// fizer o slider passar a emitir, abrir qualquer foto viraria 11 pedidos à
    /// GPU de valores que ninguém tocou — e o sintoma seria a Revelação demorar
    /// para abrir, sem nenhuma pista do porquê.
    #[gpui::test]
    fn abrir_nao_dispara_pedido_a_toa(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_cinza())
            .expect("gravar preview");

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("retrato.jpg"), window, cx);
                assert!(
                    tela.aguardando.is_none(),
                    "no neutro o resultado é a própria origem — pedir isso à GPU é latência à toa"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Trocar de foto zera os ajustes.
    ///
    /// Herdar o slider da foto anterior aplicaria a revelação de uma foto em
    /// outra, e a segunda abriria alterada sem ninguém tocar em nada — o tipo de
    /// coisa que se atribui ao motor de cor.
    #[gpui::test]
    fn abrir_outra_foto_volta_ao_neutro(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        for id in ["id-a.jpg", "id-b.jpg"] {
            previews.save_preview(id, &foto_cinza()).expect("gravar");
        }

        let janela = janela(cx, previews);
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir(foto("a.jpg"), window, cx);
            })
            .expect("a janela deve estar aberta");

        arrastar(cx, &janela, 0, 2.0);

        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.ajustes().exposure, 2.0);

                tela.abrir(foto("b.jpg"), window, cx);
                assert_eq!(tela.ajustes().exposure, 0.0, "os ajustes voltam ao neutro");
                assert_eq!(
                    tela.controles[0].estado.read(cx).value().start(),
                    0.0,
                    "e o slider volta junto — senão a barra mente sobre o estado"
                );
            })
            .expect("a janela deve estar aberta");
    }
}
