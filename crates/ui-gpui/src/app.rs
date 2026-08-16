//! A raiz: a barra de navegação e qual tela está embaixo dela.
//!
//! Nasceu com a Revelação, porque até a fase 1 só havia uma tela e a janela
//! podia abrir a Biblioteca direto. A partir de duas, alguém precisa saber qual
//! está no ar — e esse alguém não pode ser nenhuma das duas.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use gpui::{actions, div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::biblioteca::tela::Biblioteca;
use crate::revelacao::persistencia::Gravador;
use crate::revelacao::presets::GuardaDePresets;
use crate::revelacao::tela::Revelacao;

actions!(
    vintagelightbox,
    [VoltarParaBiblioteca, Desfazer, Refazer, AlternarCorte]
);

/// O contexto de teclado da raiz.
///
/// Nomeado porque o `Esc` **não pode** ser global: o campo de busca da
/// Biblioteca usa `Esc` para se limpar, e uma ligação sem contexto roubaria a
/// tecla dele. Com contexto, o `Esc` só chega aqui quando nenhum campo de texto
/// está com o foco — que é exatamente quando "voltar" é o que se quer.
const CONTEXTO: &str = "Aplicativo";

pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([
        gpui::KeyBinding::new("escape", VoltarParaBiblioteca, Some(CONTEXTO)),
        // As mesmas teclas do legado (`keyboard.rs`): `Cmd+Z` e `Cmd+Shift+Z`.
        //
        // ⚠️ **A ordem importa.** O GPUI casa a ligação mais específica primeiro,
        // mas as duas são declaradas aqui juntas de propósito: separá-las em
        // chamadas diferentes deixaria fácil alguém acrescentar um `cmd-z` depois
        // do `cmd-shift-z` e engolir o refazer — que é o tipo de coisa que só
        // aparece quando alguém tenta refazer.
        gpui::KeyBinding::new("cmd-shift-z", Refazer, Some(CONTEXTO)),
        gpui::KeyBinding::new("cmd-z", Desfazer, Some(CONTEXTO)),
        // `R` de "recortar", a mesma tecla do legado (`keyboard.rs`).
        gpui::KeyBinding::new("r", AlternarCorte, Some(CONTEXTO)),
    ]);
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
        guarda_de_presets: Arc<dyn GuardaDePresets>,
        presets: Vec<Preset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let biblioteca = cx.new(|cx| Biblioteca::nova(fotos, previews.clone(), window, cx));
        let revelacao = cx
            .new(|cx| Revelacao::nova(previews, gravador, guarda_de_presets, presets, window, cx));

        // 🚨 **`track_focus` rastreia; ele não dá foco.** Enquanto ninguém focou a
        // raiz, o caminho de foco fica vazio e **nenhuma ação de teclado dela é
        // alcançada** — o `Esc` da Revelação nunca funcionou, e o commit que o
        // trouxe deu por pronto porque o teste chamava `voltar_para_biblioteca`
        // direto, nunca a tecla. Tecla que não casa não falha: ela não faz nada.
        //
        // ⚠️ **Nenhum teste exige esta linha aqui**, e é de propósito: as teclas
        // da raiz só valem dentro da Revelação, e `revelar` refoca. Ela fica
        // porque a primeira tecla que a **Biblioteca** ganhar (as setas da grade,
        // no legado) nasceria morta sem foco desde a abertura — e o sintoma seria
        // idêntico ao que acabou de custar dois commits para aparecer.
        let foco = cx.focus_handle();
        window.focus(&foco);

        Self {
            biblioteca,
            revelacao,
            tela: Tela::Biblioteca,
            foco,
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
        // 🚨 O foco volta para a raiz a cada troca de tela, e não só na abertura.
        // Quem usou o campo de busca deixou o foco **nele** — e ele para de ser
        // renderizado ao entrar na Revelação. O caminho de foco fica apontando um
        // elemento que não está mais na tela, e as teclas da raiz somem: buscar
        // uma foto antes de revelar desligaria o `Cmd+Z`, sem nenhuma pista da
        // relação entre as duas coisas.
        window.focus(&self.foco);
        cx.notify();
    }

    /// Sair da Revelação **grava o que estiver pendente**.
    ///
    /// 🚨 Sem isto, arrastar um slider e apertar `Esc` dentro dos 500 ms de
    /// espera perderia o ajuste: a tela sai, a espera continua contando, e quem
    /// olha a Biblioteca não tem como saber que a última coisa que fez não foi
    /// guardada. É a terceira porta — as outras duas são a própria espera e a
    /// troca de foto.
    pub fn voltar_para_biblioteca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _cx| tela.gravar_o_que_estiver_pendente());
        self.tela = Tela::Biblioteca;
        window.focus(&self.foco);
        cx.notify();
    }

    /// `Cmd+Z` e `Cmd+Shift+Z` só existem dentro da Revelação.
    ///
    /// 🔑 As ações moram no contexto da raiz, e não no da Revelação, porque a
    /// Revelação **não tem foco próprio** — a raiz é quem carrega o
    /// `FocusHandle`. Quando ela ganhar um (o crop overlay vai precisar), as duas
    /// ligações mudam de contexto junto e esta guarda some.
    ///
    /// ⚠️ Na Biblioteca elas não fazem nada, e é decisão: `Cmd+Z` ali seria
    /// "desfazer a última nota/sinalizador", que o legado não tem. Fazer com que
    /// desfizesse a revelação de uma foto que nem está na tela seria pior do que
    /// não fazer nada.
    fn ao_desfazer(&mut self, _acao: &Desfazer, window: &mut Window, cx: &mut Context<Self>) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.desfazer(window, cx));
        }
    }

    fn ao_refazer(&mut self, _acao: &Refazer, window: &mut Window, cx: &mut Context<Self>) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.refazer(window, cx));
        }
    }

    /// `R` entra e sai do modo de corte, e só dentro da Revelação.
    fn ao_alternar_corte(
        &mut self,
        _acao: &AlternarCorte,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tela == Tela::Revelacao {
            self.revelacao
                .update(cx, |tela, cx| tela.alternar_corte(cx));
        }
    }

    fn ao_voltar(
        &mut self,
        _acao: &VoltarParaBiblioteca,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Só volta se há de onde voltar. Sem esta guarda, `Esc` na Biblioteca
        // seria uma tecla que consome o evento e não faz nada — e o próximo
        // atalho que quisesse `Esc` ali nasceria quebrado.
        if self.tela == Tela::Revelacao {
            self.voltar_para_biblioteca(window, cx);
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
                    .on_click(cx.listener(|este, _ev, window, cx| {
                        este.voltar_para_biblioteca(window, cx);
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
            .on_action(cx.listener(Self::ao_desfazer))
            .on_action(cx.listener(Self::ao_refazer))
            .on_action(cx.listener(Self::ao_alternar_corte))
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
    use crate::revelacao::presets::mentira::GuardaDeMentira;

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
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    gravador,
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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

                // Um arrasto, e a volta imediata — sem passar a espera.
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 0.9, cx);
                });
                app.voltar_para_biblioteca(window, cx);
            })
            .expect("a janela deve estar aberta");

        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1, "o ajuste tinha de ser gravado na saída");
        assert_eq!(gravado[0].0, "id-retrato.jpg");
        assert_eq!(gravado[0].1.exposure, 0.9);
    }

    /// 🚨 A tecla `Esc` sai mesmo da Revelação.
    ///
    /// O commit que trouxe a Revelação deu isto como pronto, e o teste de lá
    /// chamava `voltar_para_biblioteca` direto — nunca a tecla.
    #[gpui::test]
    fn esc_sai_mesmo_da_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("escape");

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.tela(),
                    Tela::Biblioteca,
                    "o Esc tem de sair da Revelação"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Buscar uma foto antes de revelar **não** desliga o `Cmd+Z`.
    ///
    /// O campo de busca fica com o foco de quem digitou nele, e ele para de ser
    /// renderizado ao entrar na Revelação: o caminho de foco passa a apontar um
    /// elemento que não está na tela, e nenhuma tecla da raiz chega. Nada falha —
    /// as teclas só param de funcionar, e a relação com "eu tinha buscado antes"
    /// é invisível.
    #[gpui::test]
    fn buscar_antes_de_revelar_nao_desliga_as_teclas(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
                    window,
                    cx,
                )
            }
        });

        janela
            .update(cx, |app, window, cx| {
                app.biblioteca.update(cx, |tela, cx| {
                    tela.selecionar(Some(1), cx);
                    tela.focar_busca(window, cx);
                });
                app.revelar(window, cx);
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("escape");

        janela
            .update(cx, |app, _window, _cx| {
                assert_eq!(
                    app.tela(),
                    Tela::Biblioteca,
                    "o foco ficou no campo de busca e as teclas da raiz sumiram"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `Cmd+Z` chega mesmo à Revelação.
    ///
    /// Não é o `desfazer` da tela que este teste mede — esse já tem os dele. É a
    /// ligação: ação registrada, contexto certo, foco no lugar. Um `KeyBinding`
    /// que não casa **não falha**: a tecla simplesmente não faz nada, e a
    /// suspeita cai na funcionalidade, não na ligação.
    #[gpui::test]
    fn cmd_z_chega_a_revelacao(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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
                app.revelacao.update(cx, |tela, cx| {
                    tela.aplicar_para_teste(0, 1.5, cx);
                });
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("cmd-z");

        janela
            .update(cx, |app, _window, cx| {
                assert_eq!(
                    app.revelacao.read(cx).ajustes().exposure,
                    0.0,
                    "o Cmd+Z tem de chegar à Revelação"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `R` abre e fecha o modo de corte.
    ///
    /// Terceira tecla ligada à raiz, e a primeira que **não** é um atalho de
    /// sistema: `R` sozinho é exatamente o tipo de ligação que um campo de texto
    /// engoliria. O teste aperta a tecla de verdade, como o do `Esc` e o do
    /// `Cmd+Z` — os dois que revelaram que a ligação não existia.
    #[gpui::test]
    fn a_tecla_r_abre_e_fecha_o_corte(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        previews
            .save_preview("id-retrato.jpg", &foto_vermelha())
            .expect("gravar preview");
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let previews = previews.clone();
            |window, cx| {
                Aplicativo::novo(
                    acervo(),
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("r");

        janela
            .update(cx, |app, _window, cx| {
                assert!(app.revelacao.read(cx).cortando(), "R tem de abrir o corte");
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("r");

        janela
            .update(cx, |app, _window, cx| {
                assert!(!app.revelacao.read(cx).cortando(), "e fechar de volta");
            })
            .expect("a janela deve estar aberta");
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
                    Arc::new(GuardaDeMentira::default()),
                    Vec::new(),
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

                app.voltar_para_biblioteca(window, cx);
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
