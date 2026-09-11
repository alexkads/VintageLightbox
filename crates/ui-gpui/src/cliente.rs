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
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui::{
    actions, div, ease_in_out, img, prelude::*, px, Animation, AnimationExt, Context, FocusHandle,
    RenderImage, SharedString, Window,
};

/// Quanto dura o cruzamento entre uma foto e a seguinte.
///
/// 🔑 **Um segundo, como na web desde 2026-09-11.** Meio segundo era o número
/// herdado da galeria, onde o operador atravessa a tira com a seta e a
/// transição vira melado — mas esta é a tela do **cliente**: quem olha não está
/// navegando, está decidindo se leva a foto, e o que ele viu foi *"tá muito
/// rápido"* (dono). O `ease_in_out` da animação é o que faz o tempo maior ser
/// percebido como apresentação e não como lentidão: o `ease_out` sai do zero a
/// toda velocidade e a foto aparece de estalo.
const CRUZAMENTO: Duration = Duration::from_millis(1000);

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
    /// A foto que está **saindo** — ela some enquanto a nova aparece.
    ///
    /// 🔑 **Ela fica montada com opacidade zero depois do cruzamento**, e não é
    /// limpa por um temporizador. Custa uma textura, nunca duas: a próxima
    /// troca a substitui pela que estiver saindo então. Um agendamento só para
    /// devolver essa textura seria mais código do que o que ele economiza.
    saindo: Option<Arc<RenderImage>>,
    /// Quantas trocas já houve.
    ///
    /// 🔑 **É o que reinicia a animação.** O GPUI guarda o instante inicial por
    /// `ElementId` (`AnimationState`, em `elements/animation.rs`): com um id
    /// fixo, a segunda troca nasceria com o `delta` da primeira já em 1 e a
    /// foto apareceria de uma vez. É o mesmo papel da `chave` das camadas na
    /// web.
    troca: usize,
    /// Onde a foto está na sequência do operador, e de quantas. Ver `info`.
    posicao: Option<(usize, usize)>,
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
            saindo: None,
            troca: 0,
            posicao: None,
            // Nasce ligado, como no legado — quem mostra ao cliente costuma
            // querer o nome do arquivo à vista, e desligar é uma tecla.
            mostrar_info: true,
            foco,
        }
    }

    /// Troca a foto que está na tela — **uma atravessando a outra**.
    ///
    /// ⚠️ **Sem imagem, a foto some da segunda tela.** Chega assim quando a foto
    /// não tem preview no cache, e mostrar o nome sobre o preto seria pior do que
    /// mostrar o preto: o cliente veria uma tela escura com um nome de arquivo.
    ///
    /// ⚠️ **Continuar na mesma foto troca os pixels sem cruzamento nenhum.** A
    /// raiz chama isto em situações que não são troca de foto — o operador
    /// acabou de revelar e a imagem é outra, a nota mudou —, e cruzar a foto
    /// com ela mesma faria a tela piscar na cara do cliente. O conteúdo
    /// atualiza no lugar; a animação é só para quando a foto muda.
    ///
    /// 🚨 **Quem decide é o id da foto, nunca o `Arc` da imagem.**
    /// `imagem::para_gpui` decodifica e **embrulha num `Arc` novo a cada
    /// chamada**: um `Arc::ptr_eq` aqui seria falso sempre, a guarda não
    /// guardaria nada, e o defeito seria invisível — a tela pisca, e o código
    /// diz que não pisca.
    pub fn mostrar(
        &mut self,
        foto: Option<PhotoViewModel>,
        imagem: Option<Arc<RenderImage>>,
        posicao: Option<(usize, usize)>,
        cx: &mut Context<Self>,
    ) {
        self.posicao = posicao;
        if self.e_a_mesma_foto(foto.as_ref()) {
            self.foto = foto;
            self.imagem = imagem;
            cx.notify();
            return;
        }
        self.saindo = self.imagem.take();
        self.troca = self.troca.wrapping_add(1);
        self.foto = foto;
        self.imagem = imagem;
        cx.notify();
    }

    /// Continuamos na mesma foto? (Sem foto dos dois lados também conta.)
    fn e_a_mesma_foto(&self, nova: Option<&PhotoViewModel>) -> bool {
        match (self.foto.as_ref(), nova) {
            (Some(atual), Some(nova)) => atual.id == nova.id,
            (None, None) => true,
            _ => false,
        }
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

    /// O que o rodapé conta: onde estamos, o nome, a nota e se o cliente já
    /// levou esta foto.
    ///
    /// 🔑 **Os três dados respondem a perguntas que o cliente faz em voz alta**
    /// — *"em qual estamos?"*, *"esta é das boas?"*, *"essa eu já levo?"* —, e
    /// entraram na web em 2026-09-11. Sem eles, quem está do outro lado
    /// acompanha o operador no escuro e pergunta a cada foto.
    ///
    /// ⚠️ **As cinco estrelas sempre aparecem**, as vazias em cinza: é o que
    /// deixa a nota ser lida de longe, sem contar o que não está lá. O legado
    /// mostrava só as cheias, e três estrelas a dois metros viram "algumas".
    fn info(&self) -> Option<InfoDoRodape> {
        let foto = self.foto.as_ref()?;
        let nota = foto.rating.clamp(0, 5) as usize;
        Some(InfoDoRodape {
            nome: foto.name.clone().into(),
            cheias: "★".repeat(nota).into(),
            vazias: "★".repeat(5 - nota).into(),
            posicao: self
                .posicao
                .map(|(i, total)| format!("{i} / {total}").into()),
            // `comprada` no view model é **levada no balcão** (ver o campo): é a
            // foto que o cliente já disse que leva.
            escolhida: foto.comprada,
        })
    }
}

/// O que o rodapé da segunda tela mostra.
struct InfoDoRodape {
    nome: SharedString,
    cheias: SharedString,
    vazias: SharedString,
    posicao: Option<SharedString>,
    escolhida: bool,
}

impl Cliente {
    /// Uma foto no ar — a que entra ou a que sai.
    ///
    /// As duas ocupam a janela inteira e se sobrepõem, que é o que permite o
    /// cruzamento. `ObjectFit::Contain` é o padrão do `img`: a foto cabe
    /// inteira, sem corte e sem esticar. Numa apresentação, cobrir a tela
    /// cortaria justamente o enquadramento que se está mostrando.
    ///
    /// ⚠️ **A que sai continua desenhada com opacidade zero** depois do
    /// cruzamento, e é de propósito: tirá-la no mesmo quadro em que a de cima
    /// chega ao fim deixaria uma tarja preta aparecer nas laterais, porque as
    /// duas fotos raramente têm a mesma proporção.
    fn camada(
        id: &'static str,
        troca: usize,
        imagem: Arc<RenderImage>,
        saindo: bool,
    ) -> impl IntoElement {
        div()
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(img(imagem).max_w_full().max_h_full())
            .with_animation(
                (id, troca),
                Animation::new(CRUZAMENTO).with_easing(ease_in_out),
                move |camada, delta| camada.opacity(if saindo { 1.0 - delta } else { delta }),
            )
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
            // ✨ **Uma foto atravessa a outra, e nunca há um quadro sem foto**
            // (pedido do dono, 2026-09-09, na tela do cliente da web; aqui pela
            // regra de paridade entre as duas plataformas). A que sai some
            // enquanto a que entra aparece, no mesmo meio segundo.
            //
            // ⚠️ **Sem o passo à frente que a web tem** (a escala de 1,03 a 1):
            // o GPUI 0.2.2 só oferece `with_transformation` em `svg`, e
            // `div`/`img` não têm escala. O cruzamento — que é o que tira o
            // lampejo preto do meio — sai igual nas duas.
            .children(
                self.saindo
                    .clone()
                    .map(|saindo| Self::camada("cliente-saindo", self.troca, saindo, true)),
            )
            .children(
                self.imagem
                    .clone()
                    .map(|imagem| Self::camada("cliente-entrando", self.troca, imagem, false)),
            )
            .children(info.map(|info| {
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
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .children(info.posicao.map(|posicao| {
                                div()
                                    .text_sm()
                                    .text_color(gpui::rgb(0x9a9a9a))
                                    .child(posicao)
                            }))
                            .child(div().text_sm().text_color(gpui::white()).child(info.nome)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xfbbf24))
                                    .child(info.cheias),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgb(0x4a4a4a))
                                    .child(info.vazias),
                            )
                            .when(info.escolhida, |linha| {
                                linha.child(
                                    div()
                                        .px(px(6.))
                                        .rounded(px(999.))
                                        .bg(gpui::rgb(0xfbbf24))
                                        .text_xs()
                                        .text_color(gpui::black())
                                        .child("Escolhida"),
                                )
                            }),
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

    /// O rodapé conta onde estamos, o nome e a nota — as cinco estrelas.
    ///
    /// 🔑 **As vazias entram**, e é a diferença que a web trouxe: a nota se lê
    /// de longe sem contar o que não está lá.
    #[gpui::test]
    fn o_rodape_mostra_a_posicao_o_nome_e_as_cinco_estrelas(cx: &mut TestAppContext) {
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("retrato.jpg", 3)), None, Some((7, 25)), cx);
                let info = cliente.info().expect("há foto");
                assert_eq!(info.nome, "retrato.jpg");
                assert_eq!(info.cheias, "★★★");
                assert_eq!(info.vazias, "★★", "as vazias completam cinco");
                assert_eq!(info.posicao.as_ref().map(|p| p.as_ref()), Some("7 / 25"));

                cliente.mostrar(Some(foto("crua.NEF", 0)), None, None, cx);
                let info = cliente.info().expect("há foto");
                assert_eq!(info.cheias, "");
                assert_eq!(info.vazias, "★★★★★", "sem nota, cinco vazias");
                assert!(info.posicao.is_none(), "sem posição, o rodapé não inventa");

                cliente.mostrar(None, None, None, cx);
                assert!(cliente.info().is_none(), "sem foto não há rodapé");
            })
            .expect("a janela deve estar aberta");
    }

    /// A foto que o cliente já disse que leva aparece marcada.
    #[gpui::test]
    fn a_levada_no_balcao_aparece_como_escolhida(cx: &mut TestAppContext) {
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("a.jpg", 5)), None, None, cx);
                assert!(!cliente.info().expect("há foto").escolhida);

                let levada = PhotoViewModel {
                    comprada: true,
                    ..foto("b.jpg", 5)
                };
                cliente.mostrar(Some(levada), None, None, cx);
                assert!(
                    cliente.info().expect("há foto").escolhida,
                    "`comprada` no view model é levada no balcão — ver o campo"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Um pixel qualquer, só para haver duas imagens distinguíveis.
    fn imagem() -> Arc<RenderImage> {
        use image::{Frame, RgbaImage};
        use smallvec::SmallVec;
        Arc::new(RenderImage::new(SmallVec::from_elem(
            Frame::new(RgbaImage::new(1, 1)),
            1,
        )))
    }

    /// ✨ Trocar de foto **guarda a anterior**: é ela que sai enquanto a nova entra.
    #[gpui::test]
    fn a_foto_que_sai_fica_para_o_cruzamento(cx: &mut TestAppContext) {
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, cx| {
                let primeira = imagem();
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(primeira.clone()), None, cx);
                assert!(
                    cliente.saindo.is_none(),
                    "a primeira foto não tem de quem sair"
                );
                let troca_da_primeira = cliente.troca;

                cliente.mostrar(Some(foto("b.jpg", 4)), Some(imagem()), None, cx);
                assert!(
                    cliente
                        .saindo
                        .as_ref()
                        .is_some_and(|s| Arc::ptr_eq(s, &primeira)),
                    "a que estava na tela é a que sai"
                );
                assert_ne!(
                    cliente.troca, troca_da_primeira,
                    "e a contagem anda — sem id novo o GPUI daria o cruzamento por terminado"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Continuar na mesma foto troca os pixels **sem** cruzamento.
    ///
    /// 🚨 O caso que faz a guarda existir: `imagem::para_gpui` devolve um `Arc`
    /// novo a cada chamada, então "é a mesma imagem?" não pode ser perguntado ao
    /// ponteiro. Se fosse, toda republicação contaria como troca e a foto
    /// piscaria — com o teste passando, porque o teste também usaria `Arc`s
    /// diferentes.
    #[gpui::test]
    fn continuar_na_mesma_foto_nao_cruza_nada(cx: &mut TestAppContext) {
        let janela = cx.add_window(Cliente::novo);

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(imagem()), None, cx);
                let troca = cliente.troca;

                // Cada chamada traz um `Arc` diferente, como a raiz faz de verdade.
                let revelada = imagem();
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(revelada.clone()), None, cx);
                assert_eq!(
                    cliente.troca, troca,
                    "mesma foto, nenhum cruzamento — senão ela pisca a cada repintura"
                );
                assert!(
                    cliente
                        .imagem
                        .as_ref()
                        .is_some_and(|i| Arc::ptr_eq(i, &revelada)),
                    "mas os pixels novos entram no lugar: revelar precisa aparecer lá"
                );
                assert!(
                    cliente.saindo.is_none(),
                    "e nada foi para a camada de saída"
                );
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
