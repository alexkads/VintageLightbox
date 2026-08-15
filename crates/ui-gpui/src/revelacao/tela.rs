//! A tela de Revelação: uma foto, grande, sobre fundo neutro.
//!
//! É deliberadamente só isso por enquanto. Os ajustes vêm depois, e vêm por cima
//! **desta** imagem — construir o slider antes de ter o que ele move deixaria o
//! primeiro ajuste sem como ser conferido.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui::{div, img, prelude::*, px, App, Context, RenderImage, SharedString, Window};
use gpui_component::ActiveTheme;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::imagem::para_gpui;

pub struct Revelacao {
    previews: Arc<PreviewManager>,
    /// A foto em revelação, e a imagem já convertida para o que o GPUI desenha.
    ///
    /// As duas juntas num `Option` só, e não em dois campos: "tem foto mas não
    /// tem imagem" e "tem imagem mas não tem foto" são estados que não existem,
    /// e deixá-los representáveis é convidar a tela a mostrar o nome de uma foto
    /// com o pixel de outra.
    aberta: Option<Aberta>,
}

struct Aberta {
    foto: PhotoViewModel,
    /// `None` quando o cache não tem nada gravado para esta foto — não é erro,
    /// é uma foto que ainda não foi processada.
    imagem: Option<Arc<RenderImage>>,
}

impl Revelacao {
    pub fn nova(previews: Arc<PreviewManager>) -> Self {
        Self {
            previews,
            aberta: None,
        }
    }

    /// Abre uma foto para revelar.
    ///
    /// ⚠️ **Lê e decodifica na thread da interface.** Um preview "Large" é um
    /// JPEG de alguns milissegundos, então isto não trava de forma perceptível —
    /// mas quando a Revelação passar a carregar o RAW em resolução plena, este é
    /// o ponto que tem de virar assíncrono. É a mesma pendência que a abertura da
    /// Biblioteca ainda tem.
    pub fn abrir(&mut self, foto: PhotoViewModel, cx: &mut Context<Self>) {
        // Preview primeiro, miniatura como queda. A miniatura fica borrada numa
        // tela inteira, e é de propósito: mostrar a foto errada de tamanho é
        // melhor do que mostrar retângulo vazio enquanto se decide o que fazer.
        let bruta = self
            .previews
            .get_preview(&foto.id)
            .or_else(|| self.previews.get_thumbnail(&foto.id));

        self.aberta = Some(Aberta {
            foto,
            imagem: bruta.map(para_gpui),
        });
        cx.notify();
    }

    /// Qual foto está aberta — o que a barra de cima mostra, e o que a fase 2 vai
    /// usar para saber sobre qual `EditSnapshot` os ajustes escrevem.
    pub fn foto(&self) -> Option<&PhotoViewModel> {
        self.aberta.as_ref().map(|a| &a.foto)
    }

    fn palco(&self, cx: &App) -> gpui::AnyElement {
        let moldura = div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .items_center()
            .justify_center()
            // O palco é **mais escuro** que o resto da janela, e não igual: o
            // olho julga exposição por comparação com o que está em volta, e um
            // entorno mais claro que a foto faz toda foto parecer subexposta. É
            // a mesma razão pela qual a moldura da grade é borda, e não fundo.
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
            // `ObjectFit::Contain` é o padrão do `img`, e é o que se quer: a foto
            // cabe inteira, sem recorte. `Cover` cortaria — e num estúdio de
            // retrato o recorte centralizado tira a cabeça primeiro.
            Some(Aberta {
                imagem: Some(imagem),
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
}

impl Render for Revelacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.palco(cx))
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

    fn foto_vermelha() -> DynamicImage {
        let mut img = RgbaImage::new(8, 8);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
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

    #[gpui::test]
    fn abrir_traz_o_preview_do_cache(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |_window, _cx| Revelacao::nova(previews)
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir(foto("retrato.jpg"), cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("retrato.jpg"));
                assert!(
                    tela.aberta.as_ref().unwrap().imagem.is_some(),
                    "o preview gravado tem de chegar até a tela"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Foto sem preview abre assim mesmo.
    ///
    /// A queda para a miniatura e depois para o aviso existe porque o catálogo
    /// pode ter foto importada e ainda não processada. Se `abrir` exigisse
    /// imagem, a Revelação ficaria presa na Biblioteca sem dizer por quê.
    #[gpui::test]
    fn foto_sem_nada_no_cache_abre_sem_imagem(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |_window, _cx| Revelacao::nova(previews)
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir(foto("sem-cache.NEF"), cx);
                assert_eq!(tela.foto().map(|f| f.name.as_str()), Some("sem-cache.NEF"));
                assert!(tela.aberta.as_ref().unwrap().imagem.is_none());
            })
            .expect("a janela deve estar aberta");
    }

    /// A miniatura serve quando o preview não existe.
    #[gpui::test]
    fn sem_preview_a_miniatura_serve(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_thumbnail("id-so-miniatura.jpg", &foto_vermelha())
            .expect("gravar miniatura");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |_window, _cx| Revelacao::nova(previews)
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir(foto("so-miniatura.jpg"), cx);
                assert!(
                    tela.aberta.as_ref().unwrap().imagem.is_some(),
                    "sem preview, a miniatura borrada é melhor que retângulo vazio"
                );
            })
            .expect("a janela deve estar aberta");
    }
}
