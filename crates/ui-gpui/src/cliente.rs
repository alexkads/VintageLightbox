//! A segunda tela: a foto grande, em fundo preto, no outro monitor.
//!
//! É o "Client View" do legado (`components/secondary_window.rs`) — a janela que
//! o fotógrafo vira para o cliente enquanto tria: sem barra de título, sem
//! controle nenhum, só a foto que está selecionada na janela principal.
//!
//! ## 🔑 O que muda do egui para o GPUI
//!
//! Lá é um `ViewportDeferred` dentro do mesmo `Context`, e a comunicação entre
//! as duas telas passa por `ctx.data_mut` com chaves de texto
//! (`"secondary_window_close_req"`) — memória global compartilhada, porque o
//! closure do viewport não alcança o estado do app. O `Esc` de lá **não fecha a
//! janela**: ele grava um sinalizador que o quadro seguinte da janela principal
//! lê e consome.
//!
//! Aqui é uma janela de verdade, com entidade própria: fechar é fechar, e a foto
//! chega por chamada de método. O que some junto é a necessidade de o app
//! principal repintar a 60fps só para manter a segunda tela em dia — o legado
//! faz `ctx.request_repaint()` incondicional nos dois lados enquanto ela estiver
//! aberta.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui::{
    actions, div, img, prelude::*, px, Context, FocusHandle, RenderImage, SharedString, Window,
};

actions!(vintagelightbox, [FecharCliente, AlternarInfoDoCliente]);

/// O contexto de teclado da segunda tela.
///
/// Próprio, e não o da raiz: as duas janelas existem ao mesmo tempo, e `Esc` na
/// principal volta para a Biblioteca enquanto `Esc` aqui fecha esta janela. Com
/// um contexto só, a tecla faria as duas coisas em janelas diferentes conforme
/// quem estivesse com o foco — que é o pior desfecho possível para uma tecla que
/// fecha coisas.
const CONTEXTO: &str = "Cliente";

pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([
        gpui::KeyBinding::new("escape", FecharCliente, Some(CONTEXTO)),
        // `I` liga e desliga o rodapé com nome e nota, como no legado.
        gpui::KeyBinding::new("i", AlternarInfoDoCliente, Some(CONTEXTO)),
    ]);
}

/// Em qual monitor a segunda tela abre.
///
/// A regra é a do legado: **o primeiro que não for o principal**, e o principal
/// se não houver outro. Com uma tela só ela abre por cima da janela do app, que
/// é o que permite conferir a funcionalidade sem um segundo monitor plugado —
/// e é o que o `MonitorDetector::get_secondary_monitor` de lá faz.
///
/// 🔑 **Recebe os ids em vez de ler o sistema**, e é genérica no tipo deles:
/// o `DisplayId` do GPUI **não pode ser construído de fora do crate** (o campo é
/// `pub(crate)`), então uma assinatura concreta só poderia ser conferida numa
/// máquina com dois monitores plugados — que é o mesmo que não conferir.
pub fn monitor_do_cliente<T: Copy + PartialEq>(todos: &[T], principal: Option<T>) -> Option<T> {
    todos
        .iter()
        .find(|id| Some(**id) != principal)
        .or_else(|| todos.first())
        .copied()
}

pub struct Cliente {
    foto: Option<PhotoViewModel>,
    imagem: Option<Arc<RenderImage>>,
    mostrar_info: bool,
    /// A janela precisa de foco próprio para `Esc` e `I` chegarem — a mesma
    /// lição que custou dois commits na Revelação: `track_focus` rastreia o
    /// foco, não o concede.
    foco: FocusHandle,
}

impl Cliente {
    pub fn novo(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let foco = cx.focus_handle();
        window.focus(&foco);

        Self {
            foto: None,
            imagem: None,
            // Nasce ligado, como no legado — quem mostra ao cliente costuma
            // querer o nome do arquivo à vista, e desligar é uma tecla.
            mostrar_info: true,
            foco,
        }
    }

    /// Troca a foto que está na tela.
    ///
    /// ⚠️ **Sem imagem, a foto some da segunda tela.** Chega assim quando a foto
    /// não tem preview no cache, e mostrar o nome sobre o preto seria pior do que
    /// mostrar o preto: o cliente veria uma tela escura com um nome de arquivo.
    pub fn mostrar(
        &mut self,
        foto: Option<PhotoViewModel>,
        imagem: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.foto = foto;
        self.imagem = imagem;
        cx.notify();
    }

    /// O nome da foto que está na tela — para a raiz poder afirmar sobre ela.
    pub fn foto_mostrada(&self) -> Option<String> {
        self.foto.as_ref().map(|foto| foto.name.clone())
    }

    pub fn mostrando_info(&self) -> bool {
        self.mostrar_info
    }

    pub fn alternar_info(&mut self, cx: &mut Context<Self>) {
        self.mostrar_info = !self.mostrar_info;
        cx.notify();
    }

    /// `Esc` fecha esta janela — e só esta.
    ///
    /// 🔑 **É a janela que se remove**, e não um recado para a principal ler
    /// depois. No legado o `Esc` do viewport grava `secondary_window_close_req`
    /// na memória global do egui, e a janela principal fecha a segunda no quadro
    /// seguinte: um caminho a mais para o mesmo fim, que existe só porque o
    /// closure do viewport não alcança o estado do app.
    fn ao_fechar(&mut self, _acao: &FecharCliente, window: &mut Window, _cx: &mut Context<Self>) {
        window.remove_window();
    }

    fn ao_alternar_info(
        &mut self,
        _acao: &AlternarInfoDoCliente,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.alternar_info(cx);
    }

    /// Nome do arquivo e a nota em estrelas — o mesmo rodapé do legado.
    fn info(&self) -> Option<(SharedString, SharedString)> {
        let foto = self.foto.as_ref()?;
        let estrelas = if foto.rating > 0 {
            "★".repeat(foto.rating.max(0) as usize)
        } else {
            String::new()
        };
        Some((foto.name.clone().into(), estrelas.into()))
    }
}

impl Render for Cliente {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let info = self.mostrar_info.then(|| self.info()).flatten();

        div()
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(Self::ao_alternar_info))
            .on_action(cx.listener(Self::ao_fechar))
            .relative()
            .size_full()
            // 🔑 **Preto, e não o fundo do tema.** A segunda tela é onde a cor da
            // foto é julgada por quem paga por ela; qualquer entorno claro muda
            // como a foto é percebida. É a mesma razão de o tema do app inteiro
            // não poder acompanhar o claro/escuro do sistema.
            .bg(gpui::black())
            .flex()
            .items_center()
            .justify_center()
            .children(self.imagem.clone().map(|imagem| {
                // `ObjectFit::Contain` é o padrão do `img`: a foto cabe inteira,
                // sem corte e sem esticar. Numa apresentação, cobrir a tela
                // cortaria justamente o enquadramento que se está mostrando.
                img(imagem).max_w_full().max_h_full()
            }))
            .children(info.map(|(nome, estrelas)| {
                div()
                    .absolute()
                    .left(px(20.))
                    .bottom(px(20.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(4.))
                    .bg(gpui::rgba(0x000000b4))
                    .flex()
                    .flex_col()
                    .child(div().text_sm().text_color(gpui::white()).child(nome))
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0xd0d0d0))
                            .child(estrelas),
                    )
            }))
            .child(
                // As instruções, como no legado. Elas **não** dependem do `I`:
                // desligar o rodapé e perder junto a única pista de como fechar
                // a janela deixaria uma tela preta sem saída visível, num monitor
                // que muitas vezes está de costas para quem a abriu.
                div()
                    .absolute()
                    .right(px(20.))
                    .top(px(20.))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(4.))
                    .bg(gpui::rgba(0x00000078))
                    .text_xs()
                    .text_color(gpui::rgb(0xb4b4b4))
                    .child("Esc fecha • I mostra o nome"),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;

    fn foto(nome: &str, nota: i32) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            rating: nota,
            ..Default::default()
        }
    }

    /// A regra do legado: o primeiro que não for o principal.
    #[test]
    fn o_cliente_abre_no_monitor_que_nao_e_o_principal() {
        let (principal, outro) = (1u32, 2u32);

        assert_eq!(
            monitor_do_cliente(&[principal, outro], Some(principal)),
            Some(outro)
        );
        assert_eq!(
            monitor_do_cliente(&[outro, principal], Some(principal)),
            Some(outro),
            "a ordem da lista não decide — quem decide é não ser o principal"
        );
    }

    /// ⚠️ Com um monitor só, ela abre nele mesmo.
    ///
    /// É o `or_else` do legado, e é o que permite conferir a segunda tela sem um
    /// segundo monitor plugado. Devolver `None` faria o botão não fazer nada na
    /// máquina de quem está desenvolvendo — e "não faz nada" é indistinguível de
    /// defeito.
    #[test]
    fn com_um_monitor_so_ela_abre_nele() {
        let unico = 1u32;
        assert_eq!(monitor_do_cliente(&[unico], Some(unico)), Some(unico));
    }

    /// Sem monitor nenhum não há onde abrir — e quem chama não abre janela.
    #[test]
    fn sem_monitor_nao_ha_onde_abrir() {
        assert_eq!(monitor_do_cliente::<u32>(&[], None), None);
    }

    /// A nota vira estrelas, e nota zero não vira nada.
    #[gpui::test]
    fn o_rodape_mostra_o_nome_e_as_estrelas(cx: &mut TestAppContext) {
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("retrato.jpg", 3)), None, cx);
                let (nome, estrelas) = cliente.info().expect("há foto");
                assert_eq!(nome, "retrato.jpg");
                assert_eq!(estrelas, "★★★");

                cliente.mostrar(Some(foto("crua.NEF", 0)), None, cx);
                let (_, estrelas) = cliente.info().expect("há foto");
                assert_eq!(estrelas, "", "nota zero não desenha estrela vazia");

                cliente.mostrar(None, None, cx);
                assert!(cliente.info().is_none(), "sem foto não há rodapé");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `I` chega mesmo à segunda tela.
    ///
    /// Ela tem contexto e foco próprios — e ligação que não casa **não falha**,
    /// ela só não faz nada. É o defeito que custou dois commits na Revelação.
    #[gpui::test]
    fn a_tecla_i_liga_e_desliga_o_rodape(cx: &mut TestAppContext) {
        cx.update(init);
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(cliente.mostrando_info(), "nasce ligado, como no legado");
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("i");

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(!cliente.mostrando_info());
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("i");

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(cliente.mostrando_info(), "e volta");
            })
            .expect("a janela deve estar aberta");
    }
}
