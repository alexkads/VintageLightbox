//! O rodapé da janela: a versão, os envios, a faixa de atualização, a tela
//! aberta e o servidor em que o app está.
//!
//! # Por que um rodapé fixo
//!
//! O app se comunica com o operador sobre **ele mesmo** — versão nova,
//! atualização compilando, "reabra para usar" — e até 27/set/2026 isso só
//! aparecia numa faixa que surgia sobre o pé da tela e sumia. Quem queria saber
//! que versão tinha nas mãos precisava abrir o menu da conta. O dono pediu o
//! rodapé *"para comunicação de versão e afins"*: uma linha que está sempre
//! lá, como a barra de status dos editores. No mesmo dia pediu nele os
//! **envios** e a **tela atual**.
//!
//! | Trecho | O que diz | Clique |
//! |---|---|---|
//! | versão | `VintageLightbox` e `versão 0.1.22` | as novidades desta versão |
//! | envios | subindo, recusados, ou "tudo no site" | os recusados abrem os motivos |
//! | meio | a faixa de atualização, quando há aviso ([`faixa`]) | os botões dela |
//! | tela | o recorte, quantas fotos e quantas selecionadas | — |
//! | servidor | o site em que o app está; âmbar fora da produção | abre o site |
//!
//! 🔑 **Nunca atrapalha**: 28 px, uma linha, sem foco. O destaque da versão
//! importante pinta só o trecho do meio — o rodapé não vira um aviso que grita
//! a sessão inteira.
//!
//! 🚨 **O servidor aparece porque já enganou gente**: com só a API apontada
//! para a pilha local, a autorização abria em produção e o operador entrava na
//! conta real achando que estava local (17/set/2026).
//!
//! 🚨 **Os recusados abrem a lista dos motivos.** O "N envios recusados" do
//! canto dos envios ligava uma marca (`vendo_recusas`) que nenhuma tela
//! desenhava: o clique não fazia nada. A lista é [`Aplicativo::lista_das_recusas`],
//! e os dois cliques (o do canto e o do rodapé) a abrem.
//!
//! O rodapé é da janela principal: a tela do cliente é outra janela, com
//! outra raiz, e nada daqui chega lá.

use std::sync::{Arc, OnceLock};

use biblioteca_core::acervo::{Estado, Filtro};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Selectable};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, FontWeight, SharedString};

use super::{Aplicativo, PedidoDeAtualizacao, Tela};
use crate::atualizacao::faixa;
use crate::estilo;
use crate::pos_venda::config::SITE_PADRAO;
use crate::recursos::Icone;
use crate::segundo_plano::frases::{ha_quanto, plural};

/// A altura do rodapé. Os toasts do canto de baixo sobem esta medida
/// (`tema::aplicar`) para não caírem em cima dele.
pub const ALTURA_DO_RODAPE: f32 = 28.;

/// Quantas recusas a lista mostra — as mais novas.
const RECUSAS_A_VISTA: usize = 100;

/// O site desta abertura, e se ele é a produção.
///
/// 🔑 Lido **uma vez**: o endereço vem do `pos-venda.json` e das variáveis de
/// ambiente, e nenhum dos dois muda com o app aberto — reler o disco a cada
/// quadro seria custo sem ganho.
fn servidor() -> &'static (String, bool) {
    static SERVIDOR: OnceLock<(String, bool)> = OnceLock::new();
    SERVIDOR.get_or_init(|| {
        let site = crate::pos_venda::config::ler().site();
        let producao = site.trim_end_matches('/') == SITE_PADRAO;
        (site, producao)
    })
}

/// O endereço como o rodapé o escreve: sem o esquema.
pub fn endereco_curto(site: &str) -> &str {
    let sem_esquema = site.split_once("://").map_or(site, |(_, resto)| resto);
    sem_esquema.trim_end_matches('/')
}

/// O que o trecho dos envios diz. Subindo e recusados podem aparecer juntos;
/// "calmo" só quando não há nenhum dos dois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envios {
    pub subindo: Option<String>,
    pub recusados: Option<String>,
    pub calmo: Option<String>,
}

/// As frases dos envios — as mesmas contas da bandeja (`frases::linhas`).
pub fn envios(subindo: usize, recusados: usize, ultimo: Option<i64>, agora: i64) -> Envios {
    let calmo = (subindo == 0 && recusados == 0).then(|| match ultimo {
        Some(quando) => format!("Tudo no site · último envio {}", ha_quanto(agora - quando)),
        None => "Nada na fila de envio".into(),
    });
    Envios {
        subindo: (subindo > 0).then(|| format!("Subindo {}", plural(subindo, "foto", "fotos"))),
        recusados: (recusados > 0).then(|| plural(recusados, "envio recusado", "envios recusados")),
        calmo,
    }
}

/// O nome do recorte como os chips da galeria e da tira o escrevem.
pub fn nome_do_recorte(filtro: Filtro) -> &'static str {
    match filtro {
        Filtro::Todas => "Todas",
        Filtro::Classificadas => "Classificadas",
        // "Sinalizada" é a levada no balcão — o que a tecla P marca.
        Filtro::Situacao(Estado::LevadaNoBalcao) => "Sinalizadas",
        Filtro::Situacao(Estado::Disponivel) => "À venda",
        Filtro::Situacao(Estado::Comprada) => "Compradas",
        Filtro::Apagadas => "Apagadas",
        Filtro::Rejeitadas => "Rejeitadas",
        Filtro::SemNota => "Sem nota",
    }
}

/// A tela aberta numa linha: "Sem nota: 41 de 47 fotos · 3 selecionadas".
///
/// 🔑 **O recorte vem com o total da galeria** — "41 fotos" sozinho pareceria
/// a galeria inteira, a armadilha do número parcial que se apresenta como
/// total. Em "Todas" o número já é o total.
pub fn resumo_da_tela(filtro: Filtro, visiveis: usize, total: usize, marcadas: usize) -> String {
    let mut partes = vec![if filtro == Filtro::Todas {
        plural(total, "foto", "fotos")
    } else {
        format!(
            "{}: {visiveis} de {}",
            nome_do_recorte(filtro),
            plural(total, "foto", "fotos")
        )
    }];
    if marcadas > 0 {
        partes.push(plural(marcadas, "selecionada", "selecionadas"));
    }
    partes.join(" · ")
}

impl Aplicativo {
    /// O resumo da tela aberta — só as telas de fotos têm um.
    fn resumo_da_tela_aberta(&self, cx: &gpui_kit::App) -> Option<String> {
        match self.tela {
            Tela::Sessao => {
                let detalhe = self.detalhe.read(cx);
                let (filtro, contagens) = (detalhe.filtro(), detalhe.contagens());
                (contagens.todas > 0).then(|| {
                    resumo_da_tela(
                        filtro,
                        contagens.de(filtro),
                        contagens.todas,
                        detalhe.quantas_marcadas(),
                    )
                })
            }
            Tela::Revelacao => {
                let revelacao = self.revelacao.read(cx);
                let total = revelacao.acervo().len();
                (total > 0).then(|| {
                    resumo_da_tela(
                        revelacao.recorte(),
                        revelacao.posicao_na_tira().1,
                        total,
                        revelacao.marcadas().len(),
                    )
                })
            }
            _ => None,
        }
    }

    /// O rodapé inteiro. Ver o [módulo](self).
    pub(super) fn rodape(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, fundo, frente, apagado, aviso, perigo) = (
            tema.border,
            tema.background,
            tema.foreground,
            tema.muted_foreground,
            tema.warning,
            tema.danger,
        );
        let destaque = faixa::em_destaque(&self.atualizacao);
        let item = |id: &'static str| {
            h_flex()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .h(px(20.))
                .px(px(6.))
                .gap(px(6.))
                .rounded(crate::tema::canto(4.))
                .items_center()
                .text_xs()
        };
        let botao = |id: &'static str| {
            estilo::botao_raso(id)
                .debug_selector(move || id.into())
                .flex_none()
        };
        let separador = || div().flex_none().w(px(1.)).h(px(12.)).bg(borda);

        // ── A versão ──────────────────────────────────────────────────────
        let versao = botao("rodape-versao")
            .text_color(apagado)
            .child(Icon::new(Icone::Sparkles).size(px(12.)))
            .child("VintageLightbox")
            .child(
                div()
                    .px(px(6.))
                    .rounded(crate::tema::canto(4.))
                    .border_1()
                    .border_color(borda)
                    .text_color(frente)
                    .font_weight(FontWeight::MEDIUM)
                    .child(concat!("versão ", env!("CARGO_PKG_VERSION"))),
            )
            .tooltip("Ver as novidades desta versão")
            .on_click(cx.listener(|raiz, _, _window, cx| {
                raiz.atender(PedidoDeAtualizacao::VerEstaVersao, cx);
            }));

        // ── Os envios ─────────────────────────────────────────────────────
        let frases = envios(
            self.sincronias_pendentes,
            self.recusas.len(),
            self.ultimo_envio,
            chrono::Utc::now().timestamp(),
        );
        let mut trecho_dos_envios = h_flex().flex_none().items_center().gap(px(2.));
        if let Some(texto) = frases.subindo {
            trecho_dos_envios = trecho_dos_envios.child(
                item("rodape-subindo")
                    .text_color(aviso)
                    .font_weight(FontWeight::MEDIUM)
                    .child(Icon::new(Icone::Upload).size(px(12.)))
                    .child(texto),
            );
        }
        if let Some(texto) = frases.recusados {
            trecho_dos_envios = trecho_dos_envios.child(
                // 🪟 A lista abre no `Popover` do gpui-kit, por cima do rodapé,
                // e fecha no clique fora e no Esc. Controlado por
                // `vendo_recusas`, porque o canto dos envios e o roteiro também
                // a abrem.
                {
                    let raiz = cx.entity().downgrade();
                    let do_conteudo = raiz.clone();
                    Popover::new("rodape-recusas")
                        .anchor(gpui_kit::Anchor::BottomLeft)
                        .appearance(false)
                        .open(self.vendo_recusas && !self.recusas.is_empty())
                        .on_open_change(move |aberto, _window, cx| {
                            let aberto = *aberto;
                            let _ = raiz.update(cx, |raiz, cx| {
                                raiz.vendo_recusas = aberto;
                                cx.notify();
                            });
                        })
                        .content(move |_, _window, cx| {
                            do_conteudo
                                .upgrade()
                                .and_then(|raiz| {
                                    raiz.update(cx, |raiz, cx| raiz.lista_das_recusas(cx))
                                })
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .trigger(
                            botao("rodape-recusados")
                                .text_color(perigo)
                                .font_weight(FontWeight::MEDIUM)
                                .child(Icon::new(Icone::TriangleAlert).size(px(12.)))
                                .child(texto)
                                .tooltip("Ver o que o site recusou"),
                        )
                },
            );
        }
        if let Some(texto) = frases.calmo {
            trecho_dos_envios = trecho_dos_envios.child(
                item("rodape-envios")
                    .text_color(apagado)
                    .child(Icon::new(Icone::Cloud).size(px(12.)))
                    .child(texto),
            );
        }

        // ── A faixa de atualização ────────────────────────────────────────
        let agir = cx.listener(|raiz, pedido: &PedidoDeAtualizacao, _window, cx| {
            raiz.atender(*pedido, cx);
        });
        let meio: Option<AnyElement> = faixa::desenhar(
            &self.atualizacao,
            cx,
            Arc::new(move |pedido, window, app| agir(&pedido, window, app)),
        );

        // ── A tela aberta ─────────────────────────────────────────────────
        let tela = self.resumo_da_tela_aberta(cx).map(|resumo| {
            item("rodape-tela")
                .text_color(apagado)
                .child(Icon::new(Icone::Layers).size(px(12.)))
                .child(resumo)
        });

        // ── Desempenho ────────────────────────────────────────────────────
        // ⏱️ Abre o painel de medição. Vermelho, com o ponto, enquanto a
        // captura grava — mesmo com o painel fechado.
        let medindo = crate::desempenho::ativa();
        let painel_aberto = self.desempenho.read(cx).aberto();
        let desempenho = botao("rodape-desempenho")
            .selected(painel_aberto && !medindo)
            .map(|d| {
                if medindo {
                    d.text_color(perigo)
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(perigo.opacity(0.12))
                } else if painel_aberto {
                    d.text_color(frente)
                } else {
                    d.text_color(apagado)
                }
            })
            .child(Icon::new(Icone::ChartColumn).size(px(12.)))
            .when(medindo, |d| d.child("● Medindo"))
            .when(!medindo, |d| d.child("Desempenho"))
            .tooltip(if medindo {
                "A captura de desempenho está gravando — clique para ver"
            } else {
                "Medir quadros, etapas de CPU e GPU e a máquina"
            })
            .on_click(cx.listener(|raiz, _, _window, cx| {
                raiz.desempenho.update(cx, |p, cx| p.alternar(cx));
                cx.notify();
            }));

        // ── O servidor ────────────────────────────────────────────────────
        let (site, producao) = servidor();
        let producao = *producao;
        let endereco: SharedString = endereco_curto(site).to_string().into();
        let para_abrir = site.clone();
        let servidor = botao("rodape-servidor")
            .map(|d| {
                if producao {
                    d.text_color(apagado)
                } else {
                    d.text_color(aviso).font_weight(FontWeight::MEDIUM)
                }
            })
            .child(Icon::new(Icone::Globe).size(px(12.)))
            .when(!producao, |d| d.child("Fora da produção ·"))
            .child(endereco)
            .tooltip(if producao {
                "O site em que as fotos e as vendas ficam"
            } else {
                "Este app não está falando com o site de produção"
            })
            .on_click(move |_, _window, cx| cx.open_url(&para_abrir));

        h_flex()
            .id("rodape")
            .flex_none()
            .h(px(ALTURA_DO_RODAPE))
            .px(px(6.))
            .gap(px(6.))
            .items_center()
            .border_t_1()
            .border_color(borda)
            .bg(fundo)
            .child(versao)
            .child(separador())
            .child(trecho_dos_envios)
            .child(
                // O meio cresce e encolhe: a frase da faixa se corta antes de
                // empurrar os trechos das pontas para fora.
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .px(px(6.))
                    .items_center()
                    .when(destaque, |d| {
                        d.bg(aviso.opacity(0.15))
                            .border_x_1()
                            .border_color(aviso.opacity(0.4))
                    })
                    .children(meio),
            )
            .when_some(tela, |rodape, tela| rodape.child(tela).child(separador()))
            .child(desempenho)
            .child(separador())
            .child(servidor)
    }

    /// A lista do que o site recusou nesta abertura, sobre o rodapé — aberta
    /// pelo "N envios recusados" do rodapé ou do canto dos envios.
    /// A lista do que o site recusou — o conteúdo do `Popover` do rodapé.
    fn lista_das_recusas(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.recusas.is_empty() {
            return None;
        }
        let tema = cx.theme();
        let (borda, fundo, apagado, perigo) = (
            tema.border,
            tema.popover,
            tema.muted_foreground,
            tema.danger,
        );
        let total = self.recusas.len();
        let linhas = self
            .recusas
            .iter()
            .rev()
            .take(RECUSAS_A_VISTA)
            .map(|motivo| {
                h_flex()
                    .items_start()
                    .gap(px(8.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(borda)
                    .text_xs()
                    .child(
                        Icon::new(Icone::TriangleAlert)
                            .size(px(12.))
                            .text_color(perigo),
                    )
                    .child(div().flex_1().min_w(px(0.)).child(motivo.clone()))
            });
        let botao = |id: &'static str, rotulo: &'static str| {
            gpui_kit::component::button::Button::new(id)
                .label(rotulo)
                .xsmall()
        };
        use gpui_kit::component::button::ButtonVariants as _;
        use gpui_kit::component::Sizable as _;
        Some(
            v_flex()
                .id("lista-das-recusas")
                .debug_selector(|| "lista-das-recusas".into())
                .mb(px(6.))
                .w(px(440.))
                .max_h(px(360.))
                .p(px(12.))
                .gap(px(8.))
                .rounded(crate::tema::canto(10.))
                .border_1()
                .border_color(borda)
                .bg(fundo)
                .shadow_lg()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!(
                            "O site recusou {}",
                            plural(total, "envio", "envios")
                        )),
                )
                .child(div().text_xs().text_color(apagado).child(
                    "O motivo é a frase do site. A foto continua neste computador; \
                     salve ou importe de novo depois de resolver.",
                ))
                .child(
                    v_flex()
                        .id("lista-das-recusas-linhas")
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scroll()
                        .children(linhas),
                )
                .child(
                    h_flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(botao("recusas-limpar", "Limpar a lista").ghost().on_click(
                            cx.listener(|raiz, _, _window, cx| {
                                raiz.recusas.clear();
                                raiz.vendo_recusas = false;
                                cx.notify();
                            }),
                        ))
                        .child(botao("recusas-fechar", "Fechar").on_click(cx.listener(
                            |raiz, _, _window, cx| {
                                raiz.vendo_recusas = false;
                                cx.notify();
                            },
                        ))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_endereco_sai_sem_esquema_e_sem_barra() {
        assert_eq!(
            endereco_curto("https://recordarfotos.com.br/"),
            "recordarfotos.com.br"
        );
        assert_eq!(endereco_curto("http://localhost:8001"), "localhost:8001");
        assert_eq!(
            endereco_curto("recordarfotos.com.br"),
            "recordarfotos.com.br"
        );
    }

    #[test]
    fn sem_fila_os_envios_dizem_quando_foi_o_ultimo() {
        let nunca = envios(0, 0, None, 1_000);
        assert_eq!(nunca.calmo.as_deref(), Some("Nada na fila de envio"));
        assert_eq!((nunca.subindo, nunca.recusados), (None, None));
        let ha_pouco = envios(0, 0, Some(1_000 - 300), 1_000);
        assert_eq!(
            ha_pouco.calmo.as_deref(),
            Some("Tudo no site · último envio há 5 min")
        );
    }

    /// Subindo e recusados aparecem juntos, e "tudo no site" some — ele
    /// mentiria com uma recusa na lista.
    #[test]
    fn subindo_e_recusados_aparecem_juntos_e_calam_o_tudo_no_site() {
        let e = envios(3, 1, Some(0), 1_000);
        assert_eq!(e.subindo.as_deref(), Some("Subindo 3 fotos"));
        assert_eq!(e.recusados.as_deref(), Some("1 envio recusado"));
        assert_eq!(e.calmo, None);
        let so_recusa = envios(0, 2, Some(0), 1_000);
        assert_eq!(so_recusa.recusados.as_deref(), Some("2 envios recusados"));
        assert_eq!(so_recusa.calmo, None);
        assert_eq!(
            envios(1, 0, None, 0).subindo.as_deref(),
            Some("Subindo 1 foto")
        );
    }

    #[test]
    fn o_resumo_da_tela_traz_o_recorte_com_o_total_da_galeria() {
        assert_eq!(resumo_da_tela(Filtro::Todas, 47, 47, 0), "47 fotos");
        assert_eq!(
            resumo_da_tela(Filtro::SemNota, 41, 47, 3),
            "Sem nota: 41 de 47 fotos · 3 selecionadas"
        );
        assert_eq!(
            resumo_da_tela(Filtro::Situacao(Estado::LevadaNoBalcao), 1, 47, 1),
            "Sinalizadas: 1 de 47 fotos · 1 selecionada"
        );
        assert_eq!(resumo_da_tela(Filtro::Todas, 1, 1, 0), "1 foto");
    }

    /// Os nomes batem com os chips da galeria e da tira — o rodapé não pode
    /// chamar de outro jeito o recorte que o operador acabou de clicar.
    #[test]
    fn o_nome_do_recorte_e_o_do_chip() {
        for (rotulo, filtro) in crate::revelacao::tela::FILTROS_DA_TIRA {
            assert_eq!(nome_do_recorte(filtro), rotulo);
        }
        for (rotulo, filtro) in crate::sessoes::detalhe::FILTROS {
            assert_eq!(nome_do_recorte(filtro), rotulo);
        }
    }
}
