//! A aba "Avisos sonoros" das Configurações: a chave geral, o volume, a voz,
//! e uma linha por tipo de aviso — liga/desliga, modo, som, texto falado e
//! "Ouvir". Embaixo, os sons próprios.
//!
//! 🔑 **Toda mudança grava na hora** (`Sons::mudar`), como o tema: não há
//! "Salvar" para esquecer de apertar antes de fechar.

use std::path::PathBuf;
use std::sync::mpsc::{channel, TryRecvError};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::{
    div, prelude::*, px, App, Context, Entity, SharedString, Subscription, Task, Window,
};

use super::{Embutido, Evento, Modo, Preferencias, Sons};
use crate::sessoes::filtros_da_lista::Opcao;

type Lista = SelectState<Vec<Opcao>>;

/// Um tipo de aviso na tela.
struct Linha {
    evento: Evento,
    modo: Entity<Lista>,
    som: Entity<Lista>,
    texto: Entity<InputState>,
}

pub struct AvisosSonoros {
    volume: Entity<SliderState>,
    ritmo: Entity<SliderState>,
    voz: Entity<Lista>,
    /// Quantas vozes a lista mostra: a leitura das vozes do sistema termina
    /// depois da tela abrir, e a lista se refaz quando o número muda.
    vozes_na_lista: usize,
    linhas: Vec<Linha>,
    /// Os sons próprios, relidos ao abrir, ao adicionar e ao remover — não a
    /// cada quadro.
    meus: Vec<String>,
    erro: Option<SharedString>,
    erro_visto: Option<SharedString>,
    _assinaturas: Vec<Subscription>,
    _tarefas: Vec<Task<()>>,
}

fn preferencias(cx: &App) -> Preferencias {
    cx.try_global::<Sons>()
        .map(|sons| sons.preferencias().clone())
        .unwrap_or_default()
}

fn mudar(cx: &mut App, mudanca: impl FnOnce(&mut Preferencias)) {
    if cx.has_global::<Sons>() {
        cx.global_mut::<Sons>().mudar(mudanca);
    }
}

fn opcoes_de_modo() -> Vec<Opcao> {
    Modo::TODOS
        .into_iter()
        .map(|modo| Opcao::nova(modo.chave(), modo.rotulo()))
        .collect()
}

fn opcoes_de_som(meus: &[String]) -> Vec<Opcao> {
    Embutido::TODOS
        .into_iter()
        .map(|som| Opcao::nova(som.chave(), som.rotulo()))
        .chain(
            meus.iter()
                .map(|nome| Opcao::nova(super::chave_do_meu(nome), format!("Meu: {nome}"))),
        )
        .collect()
}

fn opcoes_de_voz(cx: &App) -> Vec<Opcao> {
    let vozes = cx
        .try_global::<Sons>()
        .map(|sons| sons.portas().alto_falante.vozes())
        .unwrap_or_default();
    std::iter::once(Opcao::nova("", "Padrão (a primeira em português)"))
        .chain(
            vozes
                .into_iter()
                .map(|voz| Opcao::nova(voz.id, format!("{} ({})", voz.nome, voz.idioma))),
        )
        .collect()
}

impl AvisosSonoros {
    pub fn nova(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lidas = preferencias(cx);
        let meus = cx
            .try_global::<Sons>()
            .map(Sons::meus_sons)
            .unwrap_or_default();
        let mut assinaturas = Vec::new();

        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(5.0)
                .default_value(lidas.volume * 100.0)
        });
        // Ao soltar, grava e toca um sino no volume novo: é assim que se acha
        // o volume certo.
        assinaturas.push(cx.subscribe(&volume, |_, _, evento: &SliderEvent, cx| {
            if let SliderEvent::Release(valor) = evento {
                let volume = (valor.start() / 100.0).clamp(0.0, 1.0);
                mudar(cx, |p| p.volume = volume);
                if let Some(sons) = cx.try_global::<Sons>() {
                    sons.ouvir_som(Embutido::Sino.chave());
                }
            }
        }));

        let ritmo = cx.new(|_| {
            SliderState::new()
                .min(0.5)
                .max(2.0)
                .step(0.1)
                .default_value(lidas.ritmo)
        });
        assinaturas.push(cx.subscribe(&ritmo, |_, _, evento: &SliderEvent, cx| {
            if let SliderEvent::Release(valor) = evento {
                let ritmo = valor.start().clamp(0.5, 2.0);
                mudar(cx, |p| p.ritmo = ritmo);
                // Ao soltar, a voz fala no ritmo novo.
                if let Some(sons) = cx.try_global::<Sons>() {
                    sons.ouvir_voz();
                }
            }
        }));

        let vozes = opcoes_de_voz(cx);
        let vozes_na_lista = vozes.len();
        let voz = cx.new(|cx| SelectState::new(vozes, None, window, cx));
        voz.update(cx, |estado, cx| {
            estado.set_selected_value(&lidas.voz.clone().unwrap_or_default(), window, cx)
        });
        assinaturas.push(cx.subscribe_in(
            &voz,
            window,
            |_, _, evento: &SelectEvent<Vec<Opcao>>, _, cx| {
                let SelectEvent::Confirm(Some(id)) = evento else {
                    return;
                };
                let voz = (!id.is_empty()).then(|| id.clone());
                mudar(cx, |p| p.voz = voz);
                // Escolher é ouvir, como nos sons.
                if let Some(sons) = cx.try_global::<Sons>() {
                    sons.ouvir_voz();
                }
            },
        ));

        let mut linhas = Vec::new();
        for evento in Evento::TODOS {
            let escolha = lidas.escolha(evento);

            let modo = cx.new(|cx| SelectState::new(opcoes_de_modo(), None, window, cx));
            modo.update(cx, |estado, cx| {
                estado.set_selected_value(&escolha.modo.chave().to_string(), window, cx)
            });
            assinaturas.push(cx.subscribe_in(
                &modo,
                window,
                move |_, _, e: &SelectEvent<Vec<Opcao>>, _, cx| {
                    let SelectEvent::Confirm(Some(chave)) = e else {
                        return;
                    };
                    if let Some(modo) = Modo::da_chave(chave) {
                        mudar(cx, |p| p.escolha_mut(evento).modo = modo);
                        cx.notify();
                    }
                },
            ));

            let som = cx.new(|cx| SelectState::new(opcoes_de_som(&meus), None, window, cx));
            som.update(cx, |estado, cx| {
                estado.set_selected_value(&escolha.som, window, cx)
            });
            assinaturas.push(cx.subscribe_in(
                &som,
                window,
                move |_, _, e: &SelectEvent<Vec<Opcao>>, _, cx| {
                    let SelectEvent::Confirm(Some(chave)) = e else {
                        return;
                    };
                    mudar(cx, |p| p.escolha_mut(evento).som = chave.clone());
                    // Escolher é ouvir: ninguém decora o nome de um som.
                    if let Some(sons) = cx.try_global::<Sons>() {
                        sons.ouvir_som(chave);
                    }
                },
            ));

            let texto = cx.new(|cx| {
                let mut estado = InputState::new(window, cx).placeholder("O que a voz fala…");
                estado.set_value(escolha.texto.clone(), window, cx);
                estado
            });
            assinaturas.push(cx.subscribe_in(
                &texto,
                window,
                move |_, campo, e: &InputEvent, _, cx| {
                    if let InputEvent::Change = e {
                        let novo: String = campo.read(cx).value().chars().take(200).collect();
                        mudar(cx, |p| p.escolha_mut(evento).texto = novo);
                    }
                },
            ));

            linhas.push(Linha {
                evento,
                modo,
                som,
                texto,
            });
        }

        Self {
            volume,
            ritmo,
            voz,
            vozes_na_lista,
            linhas,
            meus,
            erro: None,
            erro_visto: None,
            _assinaturas: assinaturas,
            _tarefas: Vec::new(),
        }
    }

    /// Os sons próprios mudaram: a lista e as escolhas de cada linha seguem.
    fn reler_meus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.meus = cx
            .try_global::<Sons>()
            .map(Sons::meus_sons)
            .unwrap_or_default();
        let lidas = preferencias(cx);
        for linha in &self.linhas {
            let escolhido = lidas.escolha(linha.evento).som;
            let opcoes = opcoes_de_som(&self.meus);
            linha.som.update(cx, |estado, cx| {
                estado.set_items(opcoes, window, cx);
                estado.set_selected_value(&escolhido, window, cx);
            });
        }
        cx.notify();
    }

    /// "Adicionar som…": a janela do sistema, e o arquivo escolhido vira uma
    /// cópia na pasta dos sons.
    pub fn adicionar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(escolha) = cx
            .try_global::<Sons>()
            .map(|sons| sons.portas().escolha.clone())
        else {
            return;
        };
        let (daqui, dali) = channel();
        escolha.escolher(daqui);
        // O laço acaba na resposta — a regra da casa para drenar canal.
        let tarefa = cx.spawn_in(window, async move |esta, cx| loop {
            match dali.try_recv() {
                Ok(resposta) => {
                    let _ =
                        esta.update_in(cx, |tela, window, cx| tela.recebeu(resposta, window, cx));
                    return;
                }
                Err(TryRecvError::Empty) => {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
                Err(TryRecvError::Disconnected) => return,
            }
        });
        self._tarefas.push(tarefa);
    }

    fn recebeu(&mut self, arquivo: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        // Fechar a janela do sistema é desistir, e não erro.
        let Some(arquivo) = arquivo else {
            return;
        };
        if !cx.has_global::<Sons>() {
            return;
        }
        match cx.global_mut::<Sons>().adicionar(&arquivo) {
            Ok(chave) => {
                self.erro = None;
                self.reler_meus(window, cx);
                cx.global::<Sons>().ouvir_som(&chave);
            }
            Err(erro) => self.erro = Some(erro.into()),
        }
        cx.notify();
    }

    fn remover(&mut self, nome: &str, window: &mut Window, cx: &mut Context<Self>) {
        if cx.has_global::<Sons>() {
            cx.global_mut::<Sons>().remover(nome);
        }
        self.reler_meus(window, cx);
    }

    /// A lista de vozes chega depois da tela: refaz quando o número muda.
    fn acompanhar_as_vozes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let vozes = opcoes_de_voz(cx);
        if vozes.len() == self.vozes_na_lista {
            return;
        }
        self.vozes_na_lista = vozes.len();
        let escolhida = preferencias(cx).voz.unwrap_or_default();
        self.voz.update(cx, |estado, cx| {
            estado.set_items(vozes, window, cx);
            estado.set_selected_value(&escolhida, window, cx);
        });
    }

    fn linha(
        &self,
        linha: &Linha,
        lidas: &Preferencias,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let evento = linha.evento;
        let escolha = lidas.escolha(evento);
        let chave = evento.chave();
        h_flex()
            .gap(px(8.))
            .items_center()
            .child(
                div()
                    .debug_selector(move || format!("som-ligado-{chave}"))
                    .child(
                        Switch::new(SharedString::from(format!("som-ligado-{chave}")))
                            .small()
                            .checked(escolha.ligado)
                            .accessibility_label(evento.rotulo())
                            .on_change(cx.listener(move |_, ligar: &bool, _, cx| {
                                let ligar = *ligar;
                                mudar(cx, |p| p.escolha_mut(evento).ligado = ligar);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().w(px(210.)).text_sm().child(evento.rotulo()))
            .child(div().w(px(112.)).child(crate::estilo::campo_pequeno(
                Select::new(&linha.modo).xsmall(),
            )))
            .child(
                div().w(px(150.)).child(crate::estilo::campo_pequeno(
                    Select::new(&linha.som)
                        .xsmall()
                        .disabled(escolha.modo == Modo::Voz),
                )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(120.))
                    .child(crate::estilo::campo_pequeno(
                        Input::new(&linha.texto)
                            .xsmall()
                            .disabled(escolha.modo == Modo::Som),
                    )),
            )
            .child(
                div()
                    .debug_selector(move || format!("ouvir-{chave}"))
                    .child(
                        crate::estilo::botao_contorno_pequeno(
                            SharedString::from(format!("ouvir-{chave}")),
                            cx,
                        )
                        .label("Ouvir")
                        .on_click(move |_, _, cx| {
                            if let Some(sons) = cx.try_global::<Sons>() {
                                sons.ouvir(evento);
                            }
                        }),
                    ),
            )
    }
}

impl Render for AvisosSonoros {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.acompanhar_as_vozes(window, cx);
        crate::estilo::toast_quando_mudar(
            &mut self.erro_visto,
            self.erro.clone(),
            crate::estilo::Toast::Erro,
            window,
            cx,
        );
        let lidas = preferencias(cx);
        let fala = cx
            .try_global::<Sons>()
            .is_some_and(|sons| sons.portas().alto_falante.fala());
        let muted = cx.theme().muted_foreground;

        let linhas: Vec<_> = self
            .linhas
            .iter()
            .map(|linha| self.linha(linha, &lidas, cx).into_any_element())
            .collect();

        let meus: Vec<_> = self
            .meus
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, nome)| {
                let para_ouvir = super::chave_do_meu(&nome);
                let para_remover = nome.clone();
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(div().flex_1().truncate().text_sm().child(nome))
                    .child(
                        crate::estilo::botao_contorno_pequeno(
                            SharedString::from(format!("ouvir-meu-{i}")),
                            cx,
                        )
                        .label("Ouvir")
                        .on_click(move |_, _, cx| {
                            if let Some(sons) = cx.try_global::<Sons>() {
                                sons.ouvir_som(&para_ouvir);
                            }
                        }),
                    )
                    .child(
                        crate::estilo::botao_fantasma_pequeno(
                            SharedString::from(format!("remover-meu-{i}")),
                            cx,
                        )
                        .label("Remover")
                        .on_click(cx.listener(
                            move |tela, _, window, cx| {
                                tela.remover(&para_remover, window, cx);
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();

        v_flex()
            .id("avisos-sonoros")
            .gap(px(14.))
            .max_h(px(560.))
            .overflow_y_scroll()
            .pr(px(4.))
            .child(
                h_flex()
                    .gap(px(16.))
                    .items_center()
                    .child(
                        Switch::new("sons-ligados")
                            .checked(lidas.ligados)
                            .label("Tocar avisos sonoros")
                            .on_change(cx.listener(|_, ligar: &bool, _, cx| {
                                let ligar = *ligar;
                                mudar(cx, |p| p.ligados = ligar);
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(div().text_sm().text_color(muted).child("Volume"))
                    .child(div().w(px(160.)).child(crate::estilo::slider(&self.volume))),
            )
            .child(if fala {
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(div().text_sm().text_color(muted).child("Voz"))
                    .child(div().w(px(300.)).child(crate::estilo::campo_pequeno(
                        Select::new(&self.voz).xsmall(),
                    )))
                    // 🗣️ O exemplo da voz (dono, 03/out/2026: *"Tinha que ter um
                    // botão pra gente ouvir o exemplo da voz"*).
                    .child(
                        div().debug_selector(|| "ouvir-voz".into()).child(
                            crate::estilo::botao_contorno_pequeno("ouvir-voz", cx)
                                .label("Ouvir")
                                .on_click(|_, _, cx| {
                                    if let Some(sons) = cx.try_global::<Sons>() {
                                        sons.ouvir_voz();
                                    }
                                }),
                        ),
                    )
                    .child(div().flex_1())
                    .child(div().text_sm().text_color(muted).child("Ritmo da fala"))
                    .child(div().w(px(160.)).child(crate::estilo::slider(&self.ritmo)))
                    .into_any_element()
            } else {
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(
                        "Este computador não tem voz instalada: os avisos só tocam som. \
                         No Linux, instale o speech-dispatcher ou o espeak-ng.",
                    )
                    .into_any_element()
            })
            .child(
                v_flex()
                    .gap(px(6.))
                    .when(!lidas.ligados, |lista| lista.opacity(0.5))
                    .children(linhas),
            )
            .child(div().text_xs().text_color(muted).child(
                "No texto falado, {titulo} e {detalhe} viram o que o aviso diz: o nome de \
                     quem escreveu e a mensagem, o cliente e o horário do agendamento, o motivo \
                     da falha.",
            ))
            .child(
                v_flex()
                    .gap(px(6.))
                    .pt(px(4.))
                    .child(div().text_sm().child("Meus sons"))
                    .children(meus)
                    .when(self.meus.is_empty(), |bloco| {
                        bloco.child(div().text_xs().text_color(muted).child(
                            "Nenhum ainda. WAV, MP3, OGG ou FLAC — o arquivo é copiado para \
                             o app, então pode apagar o original.",
                        ))
                    })
                    .child(
                        h_flex().debug_selector(|| "adicionar-som".into()).child(
                            crate::estilo::botao_contorno_pequeno("adicionar-som", cx)
                                .label("Adicionar som…")
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    tela.adicionar(window, cx);
                                })),
                        ),
                    ),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sons::porta::mentira::{AltoFalanteDeMentira, EscolhaDeMentira};
    use crate::sons::{PortasDoSom, Som};
    use gpui_kit::{TestAppContext, VisualTestContext};
    use std::sync::Arc;

    struct Bancada {
        alto_falante: Arc<AltoFalanteDeMentira>,
        escolha: Arc<EscolhaDeMentira>,
        arquivo: PathBuf,
        pasta: tempfile::TempDir,
    }

    fn montar(cx: &mut TestAppContext) -> (Entity<AvisosSonoros>, VisualTestContext, Bancada) {
        let pasta = tempfile::TempDir::new().unwrap();
        let alto_falante = Arc::new(AltoFalanteDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::default());
        let arquivo = pasta.path().join("sons.json");
        let portas = PortasDoSom {
            alto_falante: alto_falante.clone(),
            escolha: escolha.clone(),
        };
        let para_o_global = arquivo.clone();
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(Sons::novo(portas, para_o_global));
        });
        let mut guardada: Option<Entity<AvisosSonoros>> = None;
        let janela = cx.add_window({
            let guardada = &mut guardada;
            move |window, cx| {
                let aba = cx.new(|cx| AvisosSonoros::nova(window, cx));
                *guardada = Some(aba.clone());
                gpui_kit::component::Root::new(aba, window, cx)
            }
        });
        let visual = VisualTestContext::from_window(janela.into(), cx);
        (
            guardada.expect("a aba foi construída"),
            visual,
            Bancada {
                alto_falante,
                escolha,
                arquivo,
                pasta,
            },
        )
    }

    fn clicar(visual: &mut VisualTestContext, alvo: &'static str) {
        visual.run_until_parked();
        let onde = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{alvo} não está desenhado"));
        // O primeiro evento de ponteiro da janela se perde no harness.
        visual.simulate_mouse_move(onde.center(), None, gpui_kit::Modifiers::none());
        visual.simulate_click(onde.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
    }

    /// 🚨 Desligar um tipo pelo interruptor grava no `sons.json` — e o aviso
    /// daquele tipo para de tocar.
    #[gpui_kit::test]
    fn desligar_um_tipo_grava_e_silencia(cx: &mut TestAppContext) {
        let (_aba, mut visual, bancada) = montar(cx);
        clicar(&mut visual, "som-ligado-falha");

        let lidas = crate::sons::preferencias::ler(&bancada.arquivo);
        assert!(!lidas.escolha(Evento::Falha).ligado);
        visual.update(|_, cx| crate::sons::soar(Evento::Falha, "", "", cx));
        assert!(bancada.alto_falante.pedidos().is_empty());
    }

    /// O "Ouvir" ao lado da voz fala o exemplo, sem som antes.
    #[gpui_kit::test]
    fn ouvir_a_voz_fala_o_exemplo(cx: &mut TestAppContext) {
        let (_aba, mut visual, bancada) = montar(cx);
        clicar(&mut visual, "ouvir-voz");
        let pedidos = bancada.alto_falante.pedidos();
        assert_eq!(pedidos.len(), 1);
        assert_eq!(pedidos[0].som, None);
        assert_eq!(
            pedidos[0].fala.as_ref().map(|f| f.texto.as_str()),
            Some(crate::sons::EXEMPLO_DA_VOZ)
        );
    }

    #[gpui_kit::test]
    fn ouvir_toca_o_som_do_tipo(cx: &mut TestAppContext) {
        let (_aba, mut visual, bancada) = montar(cx);
        clicar(&mut visual, "ouvir-cliente_no_qr");
        assert_eq!(
            bancada.alto_falante.pedidos()[0].som,
            Some(Som::Embutido(Embutido::Campainha))
        );
    }

    /// O arquivo escolhido entra na lista, vira opção de toda linha e toca.
    #[gpui_kit::test]
    fn adicionar_um_som_proprio(cx: &mut TestAppContext) {
        let (aba, mut visual, bancada) = montar(cx);
        let origem = bancada.pasta.path().join("buzina.wav");
        std::fs::write(&origem, crate::sons::porta::wav_de_teste(800)).unwrap();
        *bancada.escolha.resposta.lock().unwrap() = Some(origem);

        clicar(&mut visual, "adicionar-som");

        visual.update(|_, cx| {
            let aba = aba.read(cx);
            assert_eq!(aba.meus, vec!["buzina.wav".to_string()]);
            assert_eq!(aba.erro, None);
        });
        assert!(matches!(
            bancada
                .alto_falante
                .pedidos()
                .last()
                .and_then(|p| p.som.clone()),
            Some(Som::Proprio(_))
        ));
    }

    #[gpui_kit::test]
    fn arquivo_que_nao_e_som_vira_erro_na_tela(cx: &mut TestAppContext) {
        let (aba, mut visual, bancada) = montar(cx);
        let origem = bancada.pasta.path().join("nota.mp3");
        std::fs::write(&origem, b"nao sou som").unwrap();
        *bancada.escolha.resposta.lock().unwrap() = Some(origem);

        clicar(&mut visual, "adicionar-som");

        visual.update(|_, cx| {
            let aba = aba.read(cx);
            assert!(aba.meus.is_empty());
            assert!(aba.erro.as_ref().is_some_and(|e| e.contains("nota.mp3")));
        });
    }
}
