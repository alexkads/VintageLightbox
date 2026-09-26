//! A faixa que avisa da versão nova — e o diálogo das novidades.
//!
//! 🔑 **Faixa, e não modal.** Quem está triando 200 fotos não pode ser
//! interrompido por uma janela que exige clique para sumir — o desfecho
//! conhecido disso é o operador aprender a fechar sem ler, e a próxima que
//! importar de verdade some junto. A faixa fica no rodapé, ocupa uma linha e
//! espera. As novidades abrem num diálogo **só quando pedidas**.
//!
//! | Estado | A faixa diz | Botões |
//! |---|---|---|
//! | há versão (pacote) | "Versão X disponível — …" | Atualizar · Ver novidades · Depois |
//! | há versão (compila) | o mesmo, e a compilação já começou sozinha | Ver novidades |
//! | compilando | "Atualizando para a versão X em segundo plano — etapa" | Ver novidades |
//! | instalada | "Versão X instalada…" | Reabrir agora |
//! | falhou (compila) | "… a versão atual continua funcionando" | Tentar de novo · Como atualizar · Fechar |
//! | verificando (pedido) | "Procurando versão nova…" | — |
//! | em dia (pedido) | "Você está na versão mais recente (X)." | Fechar |
//! | sem resposta (pedido) | "Não consegui verificar se há versão nova: …" | Tentar de novo · Fechar |
//!
//! ⚠️ **O "Depois" guarda a versão só nesta sessão** — uma correção que o
//! fotógrafo dispensou uma vez precisa voltar a aparecer.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::{div, prelude::*, px, AnyElement, FontWeight, MouseButton, SharedString, Window};

use super::novidades;
use super::porta::{Aviso, JeitoDeAtualizar, VersaoNova};

/// O que a raiz guarda enquanto a faixa está no ar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Estado {
    /// O aviso corrente, se houver.
    pub aviso: Option<Aviso>,
    /// Verdadeiro enquanto o pacote baixa ou a compilação roda.
    pub instalando: bool,
    /// A versão que o operador mandou esperar **nesta sessão**.
    pub dispensada: Option<String>,
    /// A última versão anunciada — o diálogo, o "Reabrir" e o "Tentar de
    /// novo" precisam dela depois que o aviso virou `Instalada` ou `Falhou`.
    pub versao: Option<VersaoNova>,
    /// A etapa que o instalador anunciou por último.
    pub etapa: Option<String>,
    /// O diálogo das novidades está aberto.
    pub novidades_abertas: bool,
    /// O operador pediu "Verificar atualizações" e a resposta não chegou.
    pub verificando: bool,
}

impl Estado {
    /// Um aviso chegou da porta. Devolve `true` quando a compilação deve
    /// começar sozinha.
    pub fn receber(&mut self, aviso: Aviso) -> bool {
        if !matches!(aviso, Aviso::Progresso(_)) {
            self.verificando = false;
        }
        match aviso {
            Aviso::Progresso(etapa) => {
                self.etapa = Some(etapa);
                false
            }
            Aviso::Disponivel { versao, automatico } => {
                let compilar = automatico && versao.jeito == JeitoDeAtualizar::Compilar;
                self.versao = Some(versao.clone());
                self.aviso = Some(Aviso::Disponivel { versao, automatico });
                self.instalando = compilar;
                self.etapa = None;
                compilar
            }
            fim @ (Aviso::Instalada(_) | Aviso::Falhou(_)) => {
                self.instalando = false;
                self.aviso = Some(fim);
                false
            }
            resposta @ (Aviso::EmDia(_) | Aviso::SemResposta(_)) => {
                self.aviso = Some(resposta);
                false
            }
        }
    }

    fn jeito(&self) -> JeitoDeAtualizar {
        self.versao
            .as_ref()
            .map(|v| v.jeito)
            .unwrap_or(JeitoDeAtualizar::Pacote)
    }

    fn importante(&self) -> bool {
        self.versao
            .as_ref()
            .and_then(|v| v.novidades.as_ref())
            .is_some_and(|n| n.importante)
    }

    fn tem_novidades(&self) -> bool {
        self.versao.as_ref().is_some_and(|v| v.novidades.is_some())
    }

    /// O que a faixa mostra agora — `None` quando não há nada a dizer.
    pub fn visivel(&self) -> Option<&Aviso> {
        match self.aviso.as_ref()? {
            // Dispensada nesta abertura: some da vista, volta na próxima.
            Aviso::Disponivel { versao, .. }
                if !self.instalando && self.dispensada.as_deref() == Some(&versao.versao) =>
            {
                None
            }
            aviso => Some(aviso),
        }
    }

    /// O que a faixa diz, em uma linha.
    pub fn texto(&self) -> Option<String> {
        if self.verificando {
            return Some("Procurando versão nova…".into());
        }
        Some(match self.visivel()? {
            Aviso::Disponivel { versao, .. }
                if self.instalando && versao.jeito == JeitoDeAtualizar::Compilar =>
            {
                format!(
                    "Atualizando para a versão {} em segundo plano — {}. O app continua funcionando.",
                    versao.versao,
                    self.etapa.as_deref().unwrap_or("começando")
                )
            }
            Aviso::Disponivel { versao, .. } if self.instalando => {
                format!("Baixando a versão {}…", versao.versao)
            }
            Aviso::Disponivel { versao, .. } => {
                let abertura = if self.importante() {
                    format!("Atualização importante: versão {}", versao.versao)
                } else {
                    format!("Versão {} disponível", versao.versao)
                };
                match &versao.notas {
                    Some(notas) if !notas.trim().is_empty() => {
                        format!("{abertura} — {}", notas.trim())
                    }
                    _ => abertura,
                }
            }
            Aviso::Instalada(versao) if self.jeito() == JeitoDeAtualizar::Compilar => format!(
                "Versão {versao} instalada. Ela entra na próxima vez que o app abrir — ou reabra agora."
            ),
            Aviso::Instalada(versao) => {
                format!("Versão {versao} instalada. Reabra para usá-la.")
            }
            Aviso::Falhou(motivo) if self.jeito() == JeitoDeAtualizar::Compilar => format!(
                "A atualização não deu certo, e a versão {} continua funcionando. {motivo}",
                env!("CARGO_PKG_VERSION")
            ),
            Aviso::Falhou(motivo) => format!("Não consegui atualizar: {motivo}"),
            Aviso::EmDia(versao) => format!("Você está na versão mais recente ({versao})."),
            Aviso::SemResposta(motivo) => {
                format!("Não consegui verificar se há versão nova: {motivo}")
            }
            Aviso::Progresso(_) => return None,
        })
    }
}

/// O que os botões pedem à raiz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedido {
    /// "Atualizar": o pacote, ou a compilação que não começou sozinha.
    Instalar,
    Reabrir,
    Dispensar,
    VerNovidades,
    FecharNovidades,
    /// "Tentar de novo", depois de a compilação falhar.
    TentarDeNovo,
    CopiarComando,
    BaixarInstalador,
    /// "Verificar atualizações" (menu da conta) e o "Tentar de novo" dela.
    Verificar,
}

/// Quem atende os botões da faixa.
///
/// 🔑 Um `Arc`, e não um genérico `Clone`: o que a raiz passa aqui é o
/// `cx.listener` do GPUI, que **não é `Clone`** — e cada botão precisa da sua
/// cópia. O `Arc` resolve os dois de uma vez.
pub type Agir = Arc<dyn Fn(Pedido, &mut Window, &mut gpui_kit::App)>;

fn botao(
    id: &'static str,
    rotulo: &'static str,
    primario: bool,
    pedido: Pedido,
    agir: &Agir,
) -> Button {
    let agir = agir.clone();
    let b = Button::new(id)
        .label(rotulo)
        .xsmall()
        .on_click(move |_ev, w, cx| agir(pedido, w, cx));
    if primario {
        b.primary()
    } else {
        b.ghost()
    }
}

/// Os botões que cada estado oferece — `(id, rótulo, primário, pedido)`.
pub fn botoes(estado: &Estado) -> Vec<(&'static str, &'static str, bool, Pedido)> {
    if estado.verificando {
        return Vec::new();
    }
    let Some(aviso) = estado.visivel() else {
        return Vec::new();
    };
    let novidades = (
        "atualizar-novidades",
        "Ver novidades",
        false,
        Pedido::VerNovidades,
    );
    let mut lista = Vec::new();
    match aviso {
        Aviso::Disponivel { .. } if estado.instalando => {
            if estado.tem_novidades() {
                lista.push(novidades);
            }
        }
        Aviso::Disponivel { .. } => {
            lista.push(("atualizar-agora", "Atualizar", true, Pedido::Instalar));
            if estado.tem_novidades() {
                lista.push(novidades);
            }
            lista.push(("atualizar-depois", "Depois", false, Pedido::Dispensar));
        }
        Aviso::Instalada(_) => {
            lista.push(("atualizar-reabrir", "Reabrir agora", true, Pedido::Reabrir));
            if estado.tem_novidades() {
                lista.push(novidades);
            }
        }
        Aviso::Falhou(_) if estado.jeito() == JeitoDeAtualizar::Compilar => {
            lista.push((
                "atualizar-de-novo",
                "Tentar de novo",
                true,
                Pedido::TentarDeNovo,
            ));
            lista.push((
                "atualizar-como",
                "Como atualizar",
                false,
                Pedido::VerNovidades,
            ));
            lista.push(("atualizar-fechar", "Fechar", false, Pedido::Dispensar));
        }
        Aviso::Falhou(_) | Aviso::EmDia(_) => {
            lista.push(("atualizar-fechar", "Fechar", false, Pedido::Dispensar));
        }
        Aviso::SemResposta(_) => {
            lista.push((
                "atualizar-verificar",
                "Tentar de novo",
                true,
                Pedido::Verificar,
            ));
            lista.push(("atualizar-fechar", "Fechar", false, Pedido::Dispensar));
        }
        Aviso::Progresso(_) => {}
    }
    lista
}

/// Desenha a faixa. `agir` recebe o [`Pedido`] de cada botão.
pub fn desenhar(estado: &Estado, cx: &gpui_kit::App, agir: Agir) -> Option<AnyElement> {
    let texto: SharedString = estado.texto()?.into();
    let tema = cx.theme();
    let destaque =
        estado.importante() && matches!(estado.visivel(), Some(Aviso::Disponivel { .. }));
    let mut linha = h_flex().items_center().gap(px(6.));
    for (id, rotulo, primario, pedido) in botoes(estado) {
        linha = linha.child(botao(id, rotulo, primario, pedido, &agir));
    }
    Some(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(12.))
            .py(px(6.))
            .bg(if destaque {
                tema.warning.opacity(0.15)
            } else {
                tema.background
            })
            .border_t_1()
            .border_color(if destaque { tema.warning } else { tema.border })
            .child(
                div()
                    .text_xs()
                    .when(destaque, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(texto),
            )
            .child(linha)
            .into_any_element(),
    )
}

/// O diálogo "Novidades da versão X": o que mudou, por que atualizar e como.
pub fn desenhar_novidades(estado: &Estado, cx: &gpui_kit::App, agir: Agir) -> Option<AnyElement> {
    if !estado.novidades_abertas {
        return None;
    }
    let versao = estado.versao.as_ref()?;
    let tema = cx.theme();
    let (apagado, borda, fundo, aviso) = (
        tema.muted_foreground,
        tema.border,
        tema.popover,
        tema.warning,
    );
    let como = novidades::como_atualizar(versao.jeito);
    let secao = |titulo: &'static str| {
        div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .child(titulo)
    };

    let mut corpo = v_flex().gap(px(12.));
    if let Some(n) = &versao.novidades {
        if n.importante {
            corpo = corpo.child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .bg(aviso.opacity(0.15))
                    .text_color(aviso)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Atualização importante"),
            );
        }
        corpo = corpo
            .child(div().text_sm().child(n.titulo.clone()))
            .child(secao("O que mudou"))
            .child(
                v_flex()
                    .gap(px(4.))
                    .children(n.novidades.iter().map(|item| {
                        h_flex()
                            .items_start()
                            .gap(px(6.))
                            .text_sm()
                            .child("•")
                            .child(div().flex_1().child(item.clone()))
                    })),
            )
            .child(secao("Por que atualizar"))
            .child(div().text_sm().child(n.por_que_atualizar.clone()));
    }
    corpo = corpo.child(secao("Como atualizar")).child(
        v_flex().gap(px(4.)).children(
            como.passos
                .iter()
                .enumerate()
                .map(|(i, passo)| div().text_sm().child(format!("{}. {passo}", i + 1))),
        ),
    );
    if let Some(comando) = como.comando {
        corpo = corpo
            .child(
                div()
                    .text_xs()
                    .text_color(apagado)
                    .child("Se a atualização pelo app falhar, feche o app e rode no Terminal:"),
            )
            .child(
                div()
                    .p(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(borda)
                    .text_xs()
                    .font_family("monospace")
                    .child(comando),
            );
    }
    if como.baixar.is_some() {
        corpo =
            corpo.child(div().text_xs().text_color(apagado).child(
                "Se a atualização pelo app falhar, feche o app e rode o instalador baixado.",
            ));
    }

    let mut rodape = h_flex().justify_end().gap(px(6.));
    if como.comando.is_some() {
        rodape = rodape.child(botao(
            "novidades-copiar",
            "Copiar comando",
            false,
            Pedido::CopiarComando,
            &agir,
        ));
    }
    if como.baixar.is_some() {
        rodape = rodape.child(botao(
            "novidades-baixar",
            "Baixar o instalador",
            false,
            Pedido::BaixarInstalador,
            &agir,
        ));
    }
    if matches!(estado.visivel(), Some(Aviso::Disponivel { .. })) && !estado.instalando {
        rodape = rodape.child(botao(
            "novidades-atualizar",
            "Atualizar",
            true,
            Pedido::Instalar,
            &agir,
        ));
    }
    rodape = rodape.child(botao(
        "novidades-fechar",
        "Fechar",
        false,
        Pedido::FecharNovidades,
        &agir,
    ));

    let fechar = agir.clone();
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .occlude()
            .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                fechar(Pedido::FecharNovidades, w, cx)
            })
            .child(
                v_flex()
                    .id("novidades-da-versao")
                    .w(px(520.))
                    .max_h(px(620.))
                    .overflow_y_scroll()
                    .p(px(16.))
                    .gap(px(16.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(borda)
                    .bg(fundo)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Novidades da versão {}", versao.versao)),
                    )
                    .child(corpo)
                    .child(rodape),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::atualizacao::novidades::Novidades;

    fn nova(versao: &str) -> Aviso {
        Aviso::Disponivel {
            versao: VersaoNova {
                versao: versao.into(),
                notas: None,
                jeito: JeitoDeAtualizar::Pacote,
                novidades: None,
            },
            automatico: false,
        }
    }

    fn para_compilar(versao: &str, importante: bool, automatico: bool) -> Aviso {
        Aviso::Disponivel {
            versao: VersaoNova {
                versao: versao.into(),
                notas: Some("Chatbot e Agendamentos".into()),
                jeito: JeitoDeAtualizar::Compilar,
                novidades: Some(Novidades {
                    versao: versao.into(),
                    titulo: "Chatbot e Agendamentos".into(),
                    importante,
                    novidades: vec!["o chatbot".into()],
                    por_que_atualizar: "para não perder cliente".into(),
                }),
            },
            automatico,
        }
    }

    fn com(aviso: Aviso) -> Estado {
        let mut e = Estado::default();
        e.receber(aviso);
        e
    }

    fn rotulos(e: &Estado) -> Vec<&'static str> {
        botoes(e).into_iter().map(|(_, r, _, _)| r).collect()
    }

    #[test]
    fn sem_aviso_a_faixa_nao_diz_nada() {
        assert_eq!(Estado::default().texto(), None);
        assert!(botoes(&Estado::default()).is_empty());
    }

    #[test]
    fn a_versao_dispensada_some_da_vista() {
        let mut estado = com(nova("0.2.0"));
        estado.dispensada = Some("0.2.0".into());
        assert_eq!(estado.texto(), None);
    }

    /// 🔑 Dispensar a 0.2.0 não pode calar a 0.3.0 — senão uma correção
    /// urgente fica invisível para quem clicou "Depois" uma vez.
    #[test]
    fn dispensar_uma_versao_nao_cala_a_proxima() {
        let mut estado = com(nova("0.3.0"));
        estado.dispensada = Some("0.2.0".into());
        assert_eq!(estado.texto().as_deref(), Some("Versão 0.3.0 disponível"));
        assert_eq!(
            rotulos(&estado),
            ["Atualizar", "Depois"],
            "sem novidades, sem o botão"
        );
    }

    #[test]
    fn as_notas_entram_na_linha_quando_existem() {
        let estado = com(Aviso::Disponivel {
            versao: VersaoNova {
                versao: "0.2.0".into(),
                notas: Some("  corrige o magenta da tonalização  ".into()),
                jeito: JeitoDeAtualizar::Pacote,
                novidades: None,
            },
            automatico: false,
        });
        assert_eq!(
            estado.texto().as_deref(),
            Some("Versão 0.2.0 disponível — corrige o magenta da tonalização")
        );
    }

    /// Notas vazias não viram um travessão solto no fim da frase.
    #[test]
    fn notas_em_branco_nao_deixam_travessao() {
        let estado = com(Aviso::Disponivel {
            versao: VersaoNova {
                versao: "0.2.0".into(),
                notas: Some("   ".into()),
                jeito: JeitoDeAtualizar::Pacote,
                novidades: None,
            },
            automatico: false,
        });
        assert_eq!(estado.texto().as_deref(), Some("Versão 0.2.0 disponível"));
    }

    #[test]
    fn instalando_o_pacote_troca_a_frase_e_tira_os_botoes() {
        let mut estado = com(nova("0.2.0"));
        estado.instalando = true;
        assert_eq!(estado.texto().as_deref(), Some("Baixando a versão 0.2.0…"));
        assert!(botoes(&estado).is_empty());
    }

    #[test]
    fn instalada_pelo_pacote_pede_para_reabrir() {
        let estado = com(Aviso::Instalada("0.2.0".into()));
        assert_eq!(
            estado.texto().as_deref(),
            Some("Versão 0.2.0 instalada. Reabra para usá-la.")
        );
        assert_eq!(rotulos(&estado), ["Reabrir agora"]);
    }

    /// A falha do pacote aparece: houve um clique esperando resposta.
    #[test]
    fn a_falha_do_pacote_aparece_na_faixa() {
        let estado = com(Aviso::Falhou("assinatura inválida".into()));
        assert_eq!(
            estado.texto().as_deref(),
            Some("Não consegui atualizar: assinatura inválida")
        );
        assert_eq!(rotulos(&estado), ["Fechar"]);
    }

    /// 🔑 **Tudo automático**: a versão que compila começa sozinha, e a faixa
    /// conta a etapa que o instalador anunciou.
    #[test]
    fn a_compilacao_automatica_comeca_sozinha_e_conta_a_etapa() {
        let mut estado = Estado::default();
        assert!(
            estado.receber(para_compilar("0.1.13", true, true)),
            "começa sozinha"
        );
        assert!(estado.instalando);
        assert_eq!(
            estado.texto().as_deref(),
            Some("Atualizando para a versão 0.1.13 em segundo plano — começando. O app continua funcionando.")
        );
        assert!(!estado.receber(Aviso::Progresso("compilando".into())));
        assert_eq!(
            estado.texto().as_deref(),
            Some("Atualizando para a versão 0.1.13 em segundo plano — compilando. O app continua funcionando.")
        );
        assert_eq!(
            rotulos(&estado),
            ["Ver novidades"],
            "nada a decidir enquanto compila"
        );
        // "Depois" não esconde uma compilação em andamento.
        estado.dispensada = Some("0.1.13".into());
        assert!(estado.texto().is_some());
    }

    #[test]
    fn a_compilacao_que_nao_comeca_sozinha_espera_o_clique() {
        let mut estado = Estado::default();
        assert!(!estado.receber(para_compilar("0.1.13", true, false)));
        assert!(!estado.instalando);
        assert_eq!(
            estado.texto().as_deref(),
            Some("Atualização importante: versão 0.1.13 — Chatbot e Agendamentos")
        );
        assert_eq!(rotulos(&estado), ["Atualizar", "Ver novidades", "Depois"]);
        let comum = com(para_compilar("0.1.13", false, false));
        assert_eq!(
            comum.texto().as_deref(),
            Some("Versão 0.1.13 disponível — Chatbot e Agendamentos")
        );
    }

    #[test]
    fn instalada_pela_compilacao_diz_que_entra_na_proxima_abertura() {
        let mut estado = com(para_compilar("0.1.13", true, true));
        estado.receber(Aviso::Instalada("0.1.13".into()));
        assert!(!estado.instalando);
        assert_eq!(
            estado.texto().as_deref(),
            Some("Versão 0.1.13 instalada. Ela entra na próxima vez que o app abrir — ou reabra agora.")
        );
        assert_eq!(rotulos(&estado), ["Reabrir agora", "Ver novidades"]);
    }

    /// 🔑 **A falha diz que nada se perdeu**: a versão aberta continua, e o
    /// operador pode tentar de novo ou ver como atualizar à mão.
    #[test]
    fn a_falha_da_compilacao_diz_que_a_versao_atual_continua() {
        let mut estado = com(para_compilar("0.1.13", true, true));
        estado.receber(Aviso::Falhou("o instalador parou".into()));
        let texto = estado.texto().unwrap();
        assert!(
            texto.starts_with("A atualização não deu certo, e a versão "),
            "{texto}"
        );
        assert!(texto.contains(env!("CARGO_PKG_VERSION")));
        assert!(texto.contains("continua funcionando"));
        assert!(texto.ends_with("o instalador parou"));
        assert_eq!(
            rotulos(&estado),
            ["Tentar de novo", "Como atualizar", "Fechar"]
        );
    }

    /// 🔑 **A verificação pedida sempre responde**: "procurando" enquanto
    /// espera, e depois "está em dia" — nunca o silêncio da abertura.
    #[test]
    fn a_verificacao_pedida_diz_que_esta_em_dia() {
        let mut estado = Estado {
            verificando: true,
            ..Default::default()
        };
        assert_eq!(estado.texto().as_deref(), Some("Procurando versão nova…"));
        assert!(botoes(&estado).is_empty());
        estado.receber(Aviso::EmDia("0.1.13".into()));
        assert!(!estado.verificando);
        assert_eq!(
            estado.texto().as_deref(),
            Some("Você está na versão mais recente (0.1.13).")
        );
        assert_eq!(rotulos(&estado), ["Fechar"]);
    }

    #[test]
    fn a_verificacao_sem_resposta_oferece_tentar_de_novo() {
        let estado = com(Aviso::SemResposta("sem internet".into()));
        assert_eq!(
            estado.texto().as_deref(),
            Some("Não consegui verificar se há versão nova: sem internet")
        );
        assert_eq!(
            botoes(&estado)
                .into_iter()
                .map(|(_, r, _, p)| (r, p))
                .collect::<Vec<_>>(),
            [
                ("Tentar de novo", Pedido::Verificar),
                ("Fechar", Pedido::Dispensar)
            ]
        );
    }

    /// A versão nova achada pela verificação pedida aparece como a da abertura.
    #[test]
    fn a_verificacao_que_acha_versao_mostra_a_faixa_de_sempre() {
        let mut estado = Estado {
            verificando: true,
            ..Default::default()
        };
        estado.receber(nova("0.2.0"));
        assert_eq!(estado.texto().as_deref(), Some("Versão 0.2.0 disponível"));
    }

    #[test]
    fn o_progresso_sozinho_nao_vira_faixa() {
        let mut estado = Estado::default();
        estado.receber(Aviso::Progresso("compilando".into()));
        assert_eq!(estado.texto(), None);
    }
}
