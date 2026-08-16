//! A raiz: a barra de navegação e qual tela está embaixo dela.
//!
//! Nasceu com a Revelação, porque até a fase 1 só havia uma tela e a janela
//! podia abrir a Biblioteca direto. A partir de duas, alguém precisa saber qual
//! está no ar — e esse alguém não pode ser nenhuma das duas.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui::{actions, div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::tela::Biblioteca;
use crate::revelacao::persistencia::Gravador;
use crate::revelacao::tela::Revelacao;

actions!(vintagelightbox, [VoltarParaBiblioteca]);

/// O contexto de teclado da raiz.
///
/// Nomeado porque o `Esc` **não pode** ser global: o campo de busca da
/// Biblioteca usa `Esc` para se limpar, e uma ligação sem contexto roubaria a
/// tecla dele. Com contexto, o `Esc` só chega aqui quando nenhum campo de texto
/// está com o foco — que é exatamente quando "voltar" é o que se quer.
const CONTEXTO: &str = "Aplicativo";

pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([gpui::KeyBinding::new(
        "escape",
        VoltarParaBiblioteca,
        Some(CONTEXTO),
    )]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    Biblioteca,
    Revelacao,
}

pub struct Aplicativo {
    biblioteca: Entity<Biblioteca>,
    revelacao: Entity<Revelacao>,
    tela: Tela,
    /// A raiz precisa de foco próprio para as ações de teclado chegarem nela.
    /// Sem isto, `Esc` só funcionaria enquanto algum filho focável estivesse
    /// ativo — e a tela de Revelação não tem nenhum ainda.
    foco: FocusHandle,
}

impl Aplicativo {
    pub fn novo(
        fotos: Vec<PhotoViewModel>,
        previews: Arc<PreviewManager>,
        gravador: Arc<dyn Gravador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let biblioteca = cx.new(|cx| Biblioteca::nova(fotos, previews.clone(), window, cx));
        let revelacao = cx.new(|cx| Revelacao::nova(previews, gravador, window, cx));

        Self {
            biblioteca,
            revelacao,
            tela: Tela::Biblioteca,
            foco: cx.focus_handle(),
        }
    }

    pub fn tela(&self) -> Tela {
        self.tela
    }

    /// Leva a foto selecionada na Biblioteca para a Revelação.
    ///
    /// A seleção é **copiada**, e não compartilhada: as duas telas escolhem
    /// fotos por razões diferentes — na Biblioteca escolher é comparar, na
    /// Revelação é editar. Um estado só faria mudar de foto na grade trocar,
    /// silenciosamente, a foto que está sendo editada.
    ///
    /// Sem seleção não faz nada, e o botão que chama isto fica desligado — a
    /// Revelação vazia não responde nenhuma pergunta.
    pub fn revelar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(foto) = self.biblioteca.read(cx).foto_selecionada() else {
            return;
        };

        self.revelacao
            .update(cx, |tela, cx| tela.abrir(foto, window, cx));
        self.tela = Tela::Revelacao;
        cx.notify();
    }

    /// Sair da Revelação **grava o que estiver pendente**.
    ///
    /// 🚨 Sem isto, arrastar um slider e apertar `Esc` dentro dos 500 ms de
    /// espera perderia o ajuste: a tela sai, a espera continua contando, e quem
    /// olha a Biblioteca não tem como saber que a última coisa que fez não foi
    /// guardada. É a terceira porta — as outras duas são a própria espera e a
    /// troca de foto.
    pub fn voltar_para_biblioteca(&mut self, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());
        self.tela = Tela::Biblioteca;
        cx.notify();
    }

    fn ao_voltar(
        &mut self,
        _acao: &VoltarParaBiblioteca,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Só volta se há de onde voltar. Sem esta guarda, `Esc` na Biblioteca
        // seria uma tecla que consome o evento e não faz nada — e o próximo
        // atalho que quisesse `Esc` ali nasceria quebrado.
        if self.tela == Tela::Revelacao {
            self.voltar_para_biblioteca(cx);
        }
    }

    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Paridade com o app de egui: o botão de Revelação só liga quando há
        // seleção na Biblioteca ("Develop button enabled if Library has a
        // selection", `app.rs`).
        let tem_selecao = self.biblioteca.read(cx).foto_selecionada().is_some();
        let na_revelacao = self.tela == Tela::Revelacao;

        let titulo: SharedString = match (na_revelacao, self.revelacao.read(cx).foto()) {
            (true, Some(foto)) => foto.name.clone().into(),
            _ => "VintageLightbox".into(),
        };

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("nav-biblioteca")
                    .label("Biblioteca")
                    .xsmall()
                    .when(!na_revelacao, |b| b.primary())
                    .selected(!na_revelacao)
                    .on_click(cx.listener(|este, _ev, _window, cx| {
                        este.voltar_para_biblioteca(cx);
                    })),
            )
            .child(
                Button::new("nav-revelacao")
                    .label("Revelação")
                    .xsmall()
                    .when(na_revelacao, |b| b.primary())
                    .selected(na_revelacao)
                    .disabled(!tem_selecao)
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.revelar(window, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(titulo),
            )
    }
}

impl Render for Aplicativo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(Self::ao_voltar))
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.barra(cx))
            .child(
                // `min_h(0)` no contêiner da tela: sem ele, o conteúdo rolável
                // de dentro empurra o pai e a rolagem nunca acontece. Foi a
                // mesma correção que a linha de pastas + grade precisou.
                div().flex().flex_1().min_h(px(0.)).child(match self.tela {
                    Tela::Biblioteca => self.biblioteca.clone().into_any_element(),
                    Tela::Revelacao => self.revelacao.clone().into_any_element(),
                }),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    use crate::revelacao::persistencia::mentira::GravadorDeMentira;

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

    fn acervo() -> Vec<PhotoViewModel> {
        vec![foto("DSC_001.NEF"), foto("retrato.jpg")]
    }

    /// 🚨 Sem seleção, revelar não faz nada — e não troca de tela.
    ///
    /// O botão fica desligado, mas o método é público e o `Esc` já mostra que
    /// atalho chega antes de botão. Uma Revelação aberta sem foto seria uma tela
    /// vazia sem caminho de volta óbvio.
    #[gpui::test]
    fn revelar_sem_selecao_nao_troca_de_tela(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.revelar(window, cx);
                assert_eq!(app.tela(), Tela::Biblioteca);
            })
            .expect("a janela deve estar aberta");
    }

    /// A seleção da Biblioteca chega à Revelação.
    #[gpui::test]
    fn revelar_leva_a_foto_selecionada(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                assert_eq!(app.tela(), Tela::Revelacao);
                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg")
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Sair da Revelação grava o ajuste que ainda estava esperando.
    ///
    /// A espera de 500 ms é uma janela de perda, e `Esc` cai bem no meio dela:
    /// arrastar um slider e voltar para a Biblioteca é uma sequência de dois
    /// segundos. Sem esta gravação, o último ajuste sumiria — e sem aviso, porque
    /// a Biblioteca não tem como mostrar o que não foi guardado.
    #[gpui::test]
    fn sair_da_revelacao_grava_o_que_estava_esperando(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);

        let gravador = Arc::new(GravadorDeMentira::default());
        let janela = cx.add_window({
            let previews = previews.clone();
            let gravador = gravador.clone();
            |window, cx| Aplicativo::novo(acervo(), previews, gravador, window, cx)
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                // Um arrasto, e a volta imediata — sem passar a espera.
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 0.9, cx);
                });
                app.voltar_para_biblioteca(cx);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "o ajuste tinha de ser gravado na saída");
        assert_eq!(gravado[0].0, "id-retrato.jpg");
        assert_eq!(gravado[0].1.exposure, 0.9);
    }

    /// 🚨 Trocar de foto na grade **não** troca a foto em revelação.
    ///
    /// A seleção é copiada, não compartilhada. Compartilhada, voltar à
    /// Biblioteca e clicar em outra miniatura trocaria calado o que está sendo
    /// editado — e o próximo ajuste cairia na foto errada.
    #[gpui::test]
    fn mudar_a_selecao_depois_nao_troca_o_que_esta_em_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(1), cx));
                app.revelar(window, cx);

                app.voltar_para_biblioteca(cx);
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));

                assert_eq!(
                    app.revelacao.read(cx).foto().map(|f| f.name.as_str()),
                    Some("retrato.jpg"),
                    "a Revelação guarda a foto que recebeu, não a que a grade mostra agora"
                );
            })
            .expect("a janela deve estar aberta");
    }
}
