//! O painel de Desempenho: aberto pelo botão do rodapé **numa janela própria**,
//! que vai para qualquer monitor, sem
//! tirar o operador do que ele está medindo.
//!
//! # Por que uma janela à parte
//!
//! 🔑 **O que se mede é a janela principal.** Uma tela de diagnóstico dentro
//! dela esconderia a rolagem, o slider e o pincel que o operador precisa fazer
//! enquanto a captura corre — e a primeira versão, um painel flutuando sobre a
//! Revelação, tapava metade do palco. O dono perguntou se dava para levá-la a
//! outro monitor (27/09): agora ela é uma janela como a Tela do cliente. Fecha
//! sem parar a captura, e o botão do rodapé fica vermelho enquanto ela grava.
//!
//! O sentinela continua só na janela principal: os quadros desta janela não
//! entram na conta (mas o tempo que ela gasta desenhando é da mesma thread da
//! interface — por isso ela só se redesenha duas vezes por segundo).
//!
//! # O painel também desenha
//!
//! ⚠️ Aberto com a captura ligada, ele se atualiza duas vezes por segundo — e
//! esses quadros entram na medição como qualquer outro (fora de operação, não
//! entram no FPS de interação). Fechado, ele não desenha nada: sobra só a
//! batida do vigia, que acorda a thread sem pedir quadro.
//!
//! # As três abas
//!
//! - **Ao vivo**: os números da captura em curso (ou da última parada), a linha
//!   do tempo dos quadros lentos, o diagnóstico por etapa, o que não foi
//!   possível medir e a máquina;
//! - **Sessões**: as guardadas no banco, com o relatório de cada uma;
//! - **Comparar**: a mesma operação em todas as sessões guardadas, por sistema
//!   (Windows × Linux × macOS) — inclusive as importadas de outra máquina.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use domain::desempenho::{CabecalhoDaSessao, LinhaDeComparacao, SessaoDeDesempenho};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable as _, Icon};
use gpui_kit::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, ClipboardItem, Context, Entity,
    EventEmitter, FontWeight, Hsla, SharedString, Subscription, Task, Window,
};

use super::coletor::{Estatistica, Resumo};
use super::porta::DepositoDeDesempenho;
use super::{maquina, relatorio, Contexto, Etapa, Operacao};
use crate::recursos::Icone;

static DEPOSITO: OnceLock<Arc<dyn DepositoDeDesempenho>> = OnceLock::new();

/// Liga o painel ao banco. Chamado uma vez, no `main`.
pub fn instalar(deposito: Arc<dyn DepositoDeDesempenho>) {
    let _ = DEPOSITO.set(deposito);
}

fn deposito() -> Option<Arc<dyn DepositoDeDesempenho>> {
    DEPOSITO.get().cloned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aba {
    AoVivo,
    Sessoes,
    Comparar,
}

/// A janela abriu ou fechou, ou a captura começou ou parou: o rodapé da
/// janela principal se redesenha. (Não é um `observe`: o painel se atualiza
/// duas vezes por segundo, e cada aviso redesenharia a janela medida.)
pub struct MudouOEstado;

impl EventEmitter<MudouOEstado> for PainelDeDesempenho {}

pub struct PainelDeDesempenho {
    janela: Option<AnyWindowHandle>,
    _fechou: Option<Subscription>,
    aba: Aba,
    resumo: Option<Resumo>,
    contexto: Option<Contexto>,
    /// A última captura encerrada, montada como vai ao banco.
    sessao: Option<Arc<SessaoDeDesempenho>>,
    salva: bool,
    montando: bool,
    aviso: Option<(String, bool)>,
    salvas: Vec<CabecalhoDaSessao>,
    /// A lista veio do servidor (todas as máquinas), ou só deste computador.
    lista_do_servidor: bool,
    vendo: Option<Arc<SessaoDeDesempenho>>,
    comparar: Operacao,
    comparacao: Vec<LinhaDeComparacao>,
    ja_importou: bool,
    tique: Option<Task<()>>,
}

impl Default for PainelDeDesempenho {
    fn default() -> Self {
        Self::novo()
    }
}

impl PainelDeDesempenho {
    pub fn novo() -> Self {
        Self {
            janela: None,
            _fechou: None,
            aba: Aba::AoVivo,
            resumo: None,
            contexto: None,
            sessao: None,
            salva: false,
            montando: false,
            aviso: None,
            salvas: Vec::new(),
            lista_do_servidor: false,
            vendo: None,
            comparar: Operacao::ArrastoDeSlider,
            comparacao: Vec::new(),
            ja_importou: false,
            tique: None,
        }
    }

    pub fn aberto(&self) -> bool {
        self.janela.is_some()
    }

    /// A janela do painel, quando aberta (o roteiro a fotografa).
    pub fn janela(&self) -> Option<AnyWindowHandle> {
        self.janela
    }

    /// O botão do rodapé: abre a janela, ou a fecha.
    pub fn alternar(&mut self, cx: &mut Context<Self>) {
        if let Some(janela) = self.janela.take() {
            // Adiado: quem chama está no meio do `update` da janela principal.
            cx.defer(move |cx| {
                let _ = janela.update(cx, |_, window, _| window.remove_window());
            });
            cx.emit(MudouOEstado);
            cx.notify();
            return;
        }
        maquina::coletar_em_fundo();
        if !self.ja_importou {
            self.ja_importou = true;
            self.importar(super::porta::travamentos_pendentes(), true, cx);
            self.sincronizar(cx);
        }
        self.recarregar_lista(cx);
        if super::ativa() {
            self.resumo = super::resumo();
            self.contexto = super::contexto();
        }
        // 🚨 Adiado também: abrir a janela desenha a raiz dela — esta entidade
        // —, e ela está emprestada a este `update`.
        let entidade = cx.entity();
        cx.defer(move |cx| abrir_a_janela(entidade, cx));
    }

    fn avisar(&mut self, texto: impl Into<String>, erro: bool, cx: &mut Context<Self>) {
        let texto = texto.into();
        if crate::depuracao::ferramentas_ligadas() {
            eprintln!("[desempenho] {texto}");
        }
        self.aviso = Some((texto, erro));
        cx.notify();
    }

    // ── Iniciar e parar ──

    /// Liga a captura. O tamanho e a escala são da **janela principal** —
    /// quem os preenche é o `render` dela (`desempenho::janela_principal`).
    pub fn iniciar(&mut self, cx: &mut Context<Self>) {
        if super::ativa() {
            return;
        }
        maquina::coletar_em_fundo();
        let (hz, origem) = match maquina::pronta().and_then(|m| m.hz_do_principal()) {
            Some(hz) => (hz, "sistema"),
            None => (60.0, "padrao"),
        };
        super::iniciar(Contexto {
            janela_px: (0, 0),
            escala: 0.0,
            hz,
            hz_origem: origem,
            motor: None,
        });
        self.sessao = None;
        self.salva = false;
        self.aviso = None;
        self.aba = Aba::AoVivo;
        self.resumo = super::resumo();
        self.contexto = super::contexto();
        cx.emit(MudouOEstado);
        // 🔑 A batida e a atualização do painel: uma tarefa da thread da
        // interface. Ela acorda a thread a cada 16 ms (sem pedir quadro — é a
        // prova de que a thread está livre, para o coletor e para o vigia), e
        // só redesenha o painel, com ele aberto, a cada ~500 ms.
        self.tique = Some(cx.spawn(async move |painel, cx| {
            let mut volta = 0u32;
            loop {
                cx.background_executor()
                    .timer(Duration::from_micros(super::coletor::BATIDA_US))
                    .await;
                if !super::ativa() {
                    break;
                }
                super::batida();
                volta += 1;
                if !volta.is_multiple_of(31) {
                    continue;
                }
                if let Some(hz) = super::estimar_hz() {
                    super::atualizar_contexto(|c| {
                        if c.hz_origem != "sistema" {
                            c.hz = hz;
                            c.hz_origem = "estimada";
                        }
                    });
                }
                let seguiu = painel.update(cx, |p, cx| {
                    if p.janela.is_some() {
                        p.resumo = super::resumo();
                        p.contexto = super::contexto();
                        cx.notify();
                    }
                });
                if seguiu.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub fn parar(&mut self, cx: &mut Context<Self>) {
        let Some(encerrada) = super::parar() else {
            return;
        };
        self.tique = None;
        cx.emit(MudouOEstado);
        self.resumo = Some(
            encerrada
                .coletor
                .resumo(encerrada.duracao.as_micros() as u64),
        );
        self.contexto = Some(encerrada.contexto.clone());
        self.montando = true;
        cx.notify();
        // A montagem (amostra, métricas, texto) e a espera pela coleta da
        // máquina vão para o segundo plano.
        let tarefa = cx.background_executor().spawn(async move {
            let maquina = maquina::esperar(Duration::from_secs(4));
            relatorio::montar_sessao(
                &encerrada.id,
                encerrada.iniciada_em,
                encerrada.duracao,
                &encerrada.coletor,
                &encerrada.contexto,
                encerrada.origem,
                maquina.as_deref(),
            )
        });
        cx.spawn(async move |painel, cx| {
            let sessao = tarefa.await;
            let _ = painel.update(cx, |p, cx| {
                p.sessao = Some(Arc::new(sessao));
                p.montando = false;
                cx.notify();
            });
        })
        .detach();
    }

    // ── Banco ──

    fn salvar(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(deposito)) = (self.sessao.clone(), deposito()) else {
            return;
        };
        let recebe = deposito.salvar(sessao.clone());
        self.avisar("Salvando no banco…", false, cx);
        cx.spawn(async move |painel, cx| {
            let resposta = recebe.await.unwrap_or_else(|_| Err("sem resposta".into()));
            let _ = painel.update(cx, |p, cx| {
                match resposta {
                    Ok(destino) => {
                        p.salva = true;
                        // A fotografia do vigia desta sessão virou redundante.
                        if let Some(pasta) = super::vigia::pasta_dos_pendentes() {
                            let _ = std::fs::remove_file(
                                pasta.join(format!("{}.json", sessao.cabecalho.id)),
                            );
                        }
                        let onde = match destino {
                            super::porta::Destino::Servidor => {
                                "neste computador e no servidor".to_string()
                            }
                            super::porta::Destino::SoNesteComputador(motivo) => {
                                format!("neste computador; sobe ao servidor quando der ({motivo})")
                            }
                        };
                        p.avisar(
                            format!(
                                "Sessão salva {onde} — {} quadros, {} métricas.",
                                sessao.quadros.len(),
                                sessao.metricas.len()
                            ),
                            false,
                            cx,
                        );
                    }
                    Err(e) => p.avisar(format!("Não salvou: {e}"), true, cx),
                }
                p.recarregar_lista(cx);
            });
        })
        .detach();
    }

    /// Manda ao servidor a fila e as sessões locais que ele não tem, e
    /// relê a lista.
    fn sincronizar(&mut self, cx: &mut Context<Self>) {
        let Some(deposito) = deposito() else {
            return;
        };
        let recebe = deposito.sincronizar();
        cx.spawn(async move |painel, cx| {
            let subiram = recebe.await.unwrap_or(0);
            let _ = painel.update(cx, |p, cx| {
                if subiram > 0 {
                    p.avisar(
                        format!("{subiram} sessão(ões) deste computador subiram ao servidor."),
                        false,
                        cx,
                    );
                }
                p.recarregar_lista(cx);
            });
        })
        .detach();
    }

    fn recarregar_lista(&mut self, cx: &mut Context<Self>) {
        let Some(deposito) = deposito() else {
            return;
        };
        let recebe = deposito.listar();
        cx.spawn(async move |painel, cx| {
            let lista = recebe.await.unwrap_or_default();
            let _ = painel.update(cx, |p, cx| {
                p.salvas = lista.sessoes;
                p.lista_do_servidor = lista.do_servidor;
                cx.notify();
            });
        })
        .detach();
    }

    fn ver(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(deposito) = deposito() else {
            return;
        };
        let recebe = deposito.carregar(id);
        cx.spawn(async move |painel, cx| {
            let sessao = recebe.await.ok().flatten();
            let _ = painel.update(cx, |p, cx| {
                p.vendo = sessao.map(Arc::new);
                cx.notify();
            });
        })
        .detach();
    }

    fn apagar(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(deposito) = deposito() else {
            return;
        };
        let recebe = deposito.apagar(id);
        cx.spawn(async move |painel, cx| {
            let _ = recebe.await;
            let _ = painel.update(cx, |p, cx| {
                p.vendo = None;
                p.recarregar_lista(cx);
            });
        })
        .detach();
    }

    fn comparar(&mut self, operacao: Operacao, cx: &mut Context<Self>) {
        self.comparar = operacao;
        let Some(deposito) = deposito() else {
            return;
        };
        let recebe = deposito.comparar(operacao.nome().into());
        cx.spawn(async move |painel, cx| {
            let linhas = recebe.await.unwrap_or_default();
            let _ = painel.update(cx, |p, cx| {
                p.comparacao = linhas;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn importar(
        &mut self,
        arquivos: Vec<std::path::PathBuf>,
        apagar: bool,
        cx: &mut Context<Self>,
    ) {
        let (false, Some(deposito)) = (arquivos.is_empty(), deposito()) else {
            return;
        };
        let recebe = deposito.importar(arquivos, apagar);
        cx.spawn(async move |painel, cx| {
            let n = recebe.await.unwrap_or(0);
            let _ = painel.update(cx, |p, cx| {
                if n > 0 {
                    p.avisar(
                        if apagar {
                            format!("{n} sessão(ões) guardada(s) por travamento entraram no banco.")
                        } else {
                            format!("{n} sessão(ões) importada(s).")
                        },
                        false,
                        cx,
                    );
                }
                p.recarregar_lista(cx);
            });
        })
        .detach();
    }

    fn escolher_para_importar(&mut self, cx: &mut Context<Self>) {
        let escolha = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Importar sessões de desempenho".into()),
        });
        cx.spawn(async move |painel, cx| {
            let Ok(Ok(Some(arquivos))) = escolha.await else {
                return;
            };
            let _ = painel.update(cx, |p, cx| p.importar(arquivos, false, cx));
        })
        .detach();
    }

    /// Exporta **todas** as sessões guardadas num arquivo: é como a sessão do
    /// Windows chega ao Linux (e vice-versa) para a aba Comparar.
    fn exportar(&mut self, cx: &mut Context<Self>) {
        let Some(deposito) = deposito() else {
            return;
        };
        let pasta = directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(|d| d.to_path_buf()))
            .unwrap_or_else(std::env::temp_dir);
        let sistema = &crate::telemetria::maquina::deste().sistema;
        let nome = format!(
            "desempenho-{sistema}-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M")
        );
        let destino = cx.prompt_for_new_path(&pasta, Some(&nome));
        let ids: Vec<String> = self.salvas.iter().map(|c| c.id.clone()).collect();
        cx.spawn(async move |painel, cx| {
            let Ok(Ok(Some(caminho))) = destino.await else {
                return;
            };
            let mut sessoes = Vec::new();
            for id in ids {
                if let Ok(Some(s)) = deposito.carregar(id).await {
                    sessoes.push(s);
                }
            }
            let n = sessoes.len();
            let gravou = serde_json::to_vec(&sessoes)
                .map_err(|e| e.to_string())
                .and_then(|b| std::fs::write(&caminho, b).map_err(|e| e.to_string()));
            let _ = painel.update(cx, |p, cx| match gravou {
                Ok(()) => p.avisar(
                    format!("{n} sessão(ões) exportada(s) para {}", caminho.display()),
                    false,
                    cx,
                ),
                Err(e) => p.avisar(format!("Não exportou: {e}"), true, cx),
            });
        })
        .detach();
    }

    /// O "Salvar no banco" do roteiro: espera a montagem, se ela ainda anda.
    pub fn salvar_pelo_roteiro(&mut self, cx: &mut Context<Self>) {
        if self.sessao.is_some() {
            self.salvar(cx);
        } else {
            eprintln!("[desempenho] nada para salvar (a montagem terminou?)");
        }
    }

    /// O texto do "Copiar relatório" da última sessão montada.
    pub fn relatorio_em_texto(&self) -> Option<String> {
        self.sessao.as_ref().map(|s| relatorio::texto(s))
    }

    fn copiar(&mut self, cx: &mut Context<Self>) {
        let sessao = if super::ativa() {
            super::sessao_ate_agora().map(Arc::new)
        } else {
            self.sessao.clone()
        };
        match sessao {
            Some(s) => {
                cx.write_to_clipboard(ClipboardItem::new_string(relatorio::texto(&s)));
                self.avisar("Relatório copiado.", false, cx);
            }
            None => self.avisar(
                "Nada para copiar ainda: inicie e pare uma captura.",
                true,
                cx,
            ),
        }
    }
}

// ── O desenho ──────────────────────────────────────────────────────────────

fn ms(x: f32) -> String {
    if x >= 100.0 {
        format!("{x:.0} ms")
    } else {
        format!("{x:.1} ms")
    }
}

/// A cor de cada operação, na linha do tempo e no selo "agora".
fn cor_da_operacao(op: Operacao, cx: &App) -> Hsla {
    let t = cx.theme();
    match op {
        Operacao::Nenhuma => t.muted_foreground,
        Operacao::Rolagem => t.blue,
        Operacao::TrocaDeFoto => t.cyan,
        Operacao::AberturaDaRevelacao => t.magenta,
        Operacao::ArrastoDeSlider => t.yellow,
        Operacao::Pincel | Operacao::Laco => t.green,
        Operacao::Gradiente => t.chart_3,
        Operacao::Clone | Operacao::Heal | Operacao::Preencher => t.red,
    }
}

fn duracao_legivel(ms: f64) -> String {
    let s = (ms / 1000.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl PainelDeDesempenho {
    fn cartao(&self, rotulo: &str, valor: String, detalhe: Option<String>, cx: &App) -> AnyElement {
        let t = cx.theme();
        v_flex()
            .flex_1()
            .min_w(px(110.))
            .p(px(10.))
            .gap(px(2.))
            .rounded(crate::tema::canto(8.))
            .border_1()
            .border_color(t.border)
            .child(
                div()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(rotulo.to_string()),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(valor),
            )
            .children(detalhe.map(|d| div().text_xs().text_color(t.muted_foreground).child(d)))
            .into_any_element()
    }

    fn titulo(texto: &str) -> AnyElement {
        div()
            .pt(px(6.))
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .child(texto.to_string())
            .into_any_element()
    }

    fn linha_do_tempo(&self, r: &Resumo, cx: &App) -> AnyElement {
        let t = cx.theme();
        const LARGURA: f32 = 712.;
        const ALTURA: f32 = 64.;
        let total = (r.duracao_ms as f32).max(1.0);
        // A escala vertical: até 4× o orçamento, o teto é 100 ms.
        let teto = (r.orcamento_ms * 6.0).max(50.0);
        let limite_y = ALTURA - (r.limite_de_lento_ms / teto * ALTURA).min(ALTURA);
        let barras = r.lentos_recentes.iter().map(|q| {
            let x = (q.em_us as f32 / 1000.0 / total * LARGURA).clamp(0.0, LARGURA - 3.0);
            let valor = q.intervalo_ms().max(q.duracao_ms());
            let h = (valor / teto * ALTURA).clamp(3.0, ALTURA);
            div()
                .absolute()
                .left(px(x))
                .bottom(px(0.))
                .w(px(3.))
                .h(px(h))
                .rounded(crate::tema::canto(1.))
                .bg(cor_da_operacao(q.operacao, cx))
        });
        let travas = r.travamentos.iter().map(|tr| {
            let x = (tr.em_us as f32 / 1000.0 / total * LARGURA).clamp(0.0, LARGURA - 3.0);
            div()
                .absolute()
                .left(px(x))
                .top(px(0.))
                .w(px(3.))
                .h(px(ALTURA))
                .bg(t.danger)
        });
        v_flex()
            .gap(px(4.))
            .child(
                div()
                    .relative()
                    .w(px(LARGURA))
                    .h(px(ALTURA))
                    .rounded(crate::tema::canto(6.))
                    .bg(t.muted)
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(limite_y))
                            .w_full()
                            .h(px(1.))
                            .bg(t.border),
                    )
                    .children(barras)
                    .children(travas),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child(format!(
                        "{} quadros lentos à vista · linha = {} (1,5 × {}) · teto {}",
                        r.lentos_recentes.len(),
                        ms(r.limite_de_lento_ms),
                        ms(r.orcamento_ms),
                        ms(teto)
                    ))
                    .children(Operacao::TODAS.into_iter().map(|op| {
                        h_flex()
                            .gap(px(4.))
                            .items_center()
                            .child(
                                div()
                                    .size(px(8.))
                                    .rounded(crate::tema::canto(2.))
                                    .bg(cor_da_operacao(op, cx)),
                            )
                            .child(op.rotulo())
                    })),
            )
            .into_any_element()
    }

    fn tabela_de_etapas(
        &self,
        etapas: &[(Etapa, Estatistica)],
        gargalo: Option<Etapa>,
        cx: &App,
    ) -> AnyElement {
        let t = cx.theme();
        let mut ordenadas: Vec<&(Etapa, Estatistica)> =
            etapas.iter().filter(|(_, s)| s.amostras > 0).collect();
        ordenadas.sort_by(|a, b| b.1.p95_ms.total_cmp(&a.1.p95_ms));
        let cabeca = h_flex()
            .text_xs()
            .text_color(t.muted_foreground)
            .child(div().w(px(330.)).child("etapa"))
            .child(div().w(px(150.)).child("onde"))
            .child(div().w(px(64.)).child("mediana"))
            .child(div().w(px(64.)).child("p95"))
            .child(div().w(px(64.)).child("pior"))
            .child(div().w(px(50.)).child("n"));
        let linhas = ordenadas.into_iter().map(|(e, s)| {
            let destaque = Some(*e) == gargalo;
            h_flex()
                .text_xs()
                .py(px(2.))
                .when(destaque, |d| {
                    d.bg(t.warning.opacity(0.15))
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .child(div().w(px(330.)).child(e.rotulo()))
                .child(
                    div()
                        .w(px(150.))
                        .text_color(t.muted_foreground)
                        .child(e.onde().rotulo()),
                )
                .child(div().w(px(64.)).child(ms(s.mediana_ms)))
                .child(div().w(px(64.)).child(ms(s.p95_ms)))
                .child(div().w(px(64.)).child(ms(s.pior_ms)))
                .child(
                    div()
                        .w(px(50.))
                        .text_color(t.muted_foreground)
                        .child(s.amostras.to_string()),
                )
        });
        v_flex().child(cabeca).children(linhas).into_any_element()
    }

    fn ao_vivo(&self, cx: &App) -> AnyElement {
        let t = cx.theme();
        let Some(r) = &self.resumo else {
            return v_flex()
                .gap(px(8.))
                .text_sm()
                .text_color(t.muted_foreground)
                .child("Nenhuma captura ainda.")
                .child(
                    "Aperte Iniciar, repita a ação lenta (rolar a galeria, trocar de foto, \
                     arrastar um slider, pintar uma máscara) e aperte Parar. Meça em --release, \
                     com a mesma foto e a mesma sequência nas máquinas que vai comparar.",
                )
                .child(self.maquina(cx))
                .into_any_element();
        };
        let contexto = self.contexto.clone().unwrap_or_default();
        let maq = maquina::pronta();
        let diagnostico = relatorio::diagnostico(r, &contexto, maq.as_deref());
        let agora = if super::ativa() {
            Some(r.operacao_atual)
        } else {
            None
        };
        let mut v = v_flex().gap(px(8.));
        v = v.child(
            h_flex()
                .gap(px(8.))
                .child(self.cartao(
                    "FPS durante a interação",
                    format!("{:.1}", r.fps_interacao),
                    Some(format!(
                        "{} quadros de {}",
                        r.quadros_interacao, r.quadros_total
                    )),
                    cx,
                ))
                .child(self.cartao("Mediana por quadro", ms(r.mediana_ms), None, cx))
                .child(self.cartao("p95", ms(r.p95_ms), None, cx))
                .child(self.cartao("Pior quadro", ms(r.pior_ms), None, cx))
                .child(self.cartao(
                    "Acima do orçamento",
                    r.acima_do_orcamento.to_string(),
                    Some(format!(
                        "orçamento {} · {:.0} Hz ({})",
                        ms(r.orcamento_ms),
                        1000.0 / r.orcamento_ms,
                        match contexto.hz_origem {
                            "sistema" => "do sistema",
                            "estimada" => "estimada",
                            _ => "padrão",
                        }
                    )),
                    cx,
                )),
        );
        if let Some(op) = agora {
            v = v.child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .text_sm()
                    .child("Agora:")
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(crate::tema::canto(10.))
                            .bg(cor_da_operacao(op, cx).opacity(0.25))
                            .border_1()
                            .border_color(cor_da_operacao(op, cx))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(op.rotulo()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.muted_foreground)
                            .child(format!(
                                "memória da captura: {:.1} MB",
                                r.memoria_bytes as f64 / 1_048_576.0
                            )),
                    ),
            );
        }
        v = v
            .child(Self::titulo("Quadros lentos na linha do tempo"))
            .child(self.linha_do_tempo(r, cx))
            .child(Self::titulo("Diagnóstico"));
        if diagnostico.is_empty() {
            v = v.child(
                div()
                    .text_xs()
                    .text_color(t.muted_foreground)
                    .child("Ainda sem operação medida."),
            );
        }
        for linha in diagnostico {
            v = v.child(div().text_sm().child(linha));
        }
        for op in &r.por_operacao {
            v = v.child(Self::titulo(&format!(
                "{} — {} quadros · {:.1} FPS · intervalo p95 {} · pior {} · {} acima",
                op.operacao.rotulo(),
                op.quadros,
                op.fps,
                ms(op.intervalo.p95_ms),
                ms(op.intervalo.pior_ms),
                op.acima_do_orcamento
            )));
            v = v.child(self.tabela_de_etapas(&op.etapas, op.gargalo.map(|g| g.0), cx));
        }
        v = v.child(Self::titulo("Indisponível nesta medição")).child(
            v_flex()
                .text_xs()
                .text_color(t.muted_foreground)
                .child("• Tempo de GPU do renderizador do GPUI e subida das imagens ao atlas: o GPUI não expõe instrumentação.")
                .child("• Fim exato do present: a apresentação é medida até a primeira tarefa seguinte (um teto).")
                .children(match &contexto.motor {
                    Some(m) if !m.carimbos => Some(format!(
                        "• Tempo de GPU do motor: {} ({}) não oferece timestamp query — sobram as etapas de CPU e a espera pela GPU.",
                        m.nome, m.backend
                    )),
                    None => Some("• Tempo de GPU do motor: a Revelação ainda não revelou nesta captura.".into()),
                    _ => None,
                }),
        );
        v.child(self.maquina(cx)).into_any_element()
    }

    fn maquina(&self, cx: &App) -> AnyElement {
        let t = cx.theme();
        let mut v = v_flex().gap(px(2.)).child(Self::titulo("Máquina"));
        let Some(m) = maquina::pronta() else {
            return v
                .child(
                    div()
                        .text_xs()
                        .text_color(t.muted_foreground)
                        .child("Coletando hardware, sistema e drivers…"),
                )
                .into_any_element();
        };
        let linha = |texto: String| div().text_xs().child(texto);
        v = v
            .child(linha(format!(
                "Sistema: {} · kernel {} · {}{}",
                m.sistema_versao,
                m.kernel,
                m.arquitetura,
                m.area_de_trabalho
                    .as_ref()
                    .map(|a| format!(" · {a}"))
                    .unwrap_or_default()
            )))
            .child(linha(format!(
                "CPU: {} · {} MHz",
                m.cpu_em_uma_linha(),
                m.cpu_mhz
            )))
            .child(linha(format!(
                "Memória livre ao coletar: {} MB · app {} · build {}",
                m.memoria_livre_mb, m.versao_do_app, m.perfil_de_build
            )))
            .child(linha(format!("Interface: {}", m.renderizador_da_interface)));
        if let Some(motor) = self.contexto.as_ref().and_then(|c| c.motor.clone()) {
            v = v.child(linha(format!(
                "Motor da Revelação: {} · {} · {} · driver {} {} · timestamps {}",
                motor.nome,
                motor.backend,
                motor.tipo,
                motor.driver,
                motor.driver_info,
                if motor.carimbos { "sim" } else { "não" }
            )));
        }
        for g in &m.gpus {
            v = v.child(linha(format!(
                "GPU: {} · {} {}:{} · {} · {} · driver {} {}",
                g.nome,
                g.fabricante,
                g.fabricante_id,
                g.placa_id,
                g.tipo,
                g.backend,
                g.driver,
                g.driver_info
            )));
        }
        for p in &m.placas_no_sistema {
            v = v.child(linha(format!(
                "Placa no sistema: {} · driver {} {} {}",
                p.nome, p.driver_versao, p.driver_data, p.detalhe
            )));
        }
        for mon in &m.monitores {
            v = v.child(linha(format!(
                "Monitor: {} · {} · {}{}",
                mon.nome,
                mon.resolucao,
                mon.hz
                    .map(|h| format!("{h:.0} Hz"))
                    .unwrap_or_else(|| "Hz ?".into()),
                if mon.principal { " (principal)" } else { "" }
            )));
        }
        if let Some(e) = &m.energia {
            v = v.child(linha(format!("Energia: {e}")));
        }
        for f in &m.faltas {
            v = v.child(
                div()
                    .text_xs()
                    .text_color(t.warning)
                    .child(format!("Não lido: {f}")),
            );
        }
        v.into_any_element()
    }

    fn sessoes(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.theme().clone();
        let mut v =
            v_flex()
                .gap(px(4.))
                .child(div().text_xs().text_color(t.muted_foreground).child(
                if self.lista_do_servidor {
                    "Sessões do servidor: todos os computadores (e as deste que ainda não subiram)."
                } else {
                    "Sem o servidor agora: só as sessões deste computador."
                },
            ));
        if self.salvas.is_empty() {
            v = v.child(
                div()
                    .text_sm()
                    .text_color(t.muted_foreground)
                    .child("Nenhuma sessão no banco. Pare uma captura e aperte Salvar no banco."),
            );
        }
        for (i, c) in self.salvas.iter().enumerate() {
            let id = c.id.clone();
            let selecionada = self.vendo.as_ref().is_some_and(|v| v.cabecalho.id == c.id);
            let data = chrono::DateTime::parse_from_rfc3339(&c.iniciada_em)
                .map(|d| {
                    d.with_timezone(&chrono::Local)
                        .format("%d/%m %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|_| c.iniciada_em.clone());
            v = v.child(
                h_flex()
                    .id(SharedString::from(format!("desempenho-sessao-{i}")))
                    .gap(px(10.))
                    .px(px(6.))
                    .py(px(4.))
                    .rounded(crate::tema::canto(6.))
                    .text_xs()
                    .cursor_pointer()
                    .when(selecionada, |d| d.bg(t.accent))
                    .hover(|d| d.bg(t.accent))
                    .child(div().w(px(80.)).child(data))
                    .child(div().w(px(60.)).child(c.sistema.clone()))
                    .child(
                        div()
                            .w(px(200.))
                            .overflow_hidden()
                            .child(format!("{} · {}", c.gpu_nome, c.gpu_backend)),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .child(format!("{:.1} FPS", c.fps_interacao)),
                    )
                    .child(div().w(px(80.)).child(format!("p95 {}", ms(c.p95_ms))))
                    .child(
                        div()
                            .w(px(70.))
                            .child(format!("{} acima", c.acima_do_orcamento)),
                    )
                    .child(
                        div()
                            .text_color(t.muted_foreground)
                            .child(format!("{} · {}", c.origem, c.perfil_de_build)),
                    )
                    .on_click(cx.listener(move |p, _, _, cx| p.ver(id.clone(), cx))),
            );
        }
        if let Some(s) = self.vendo.clone() {
            let id = s.cabecalho.id.clone();
            let texto = relatorio::texto(&s);
            let para_copiar = texto.clone();
            v = v
                .child(
                    h_flex()
                        .gap(px(6.))
                        .pt(px(8.))
                        .child(
                            crate::estilo::botao_contorno("desempenho-copiar-vista", cx)
                                .label("Copiar este relatório")
                                .on_click(cx.listener(move |p, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        para_copiar.clone(),
                                    ));
                                    p.avisar("Relatório copiado.", false, cx);
                                })),
                        )
                        .child(
                            crate::estilo::botao_perigo("desempenho-apagar", cx)
                                .label("Apagar sessão")
                                .on_click(cx.listener(move |p, _, _, cx| p.apagar(id.clone(), cx))),
                        ),
                )
                .child(
                    div()
                        .p(px(8.))
                        .rounded(crate::tema::canto(6.))
                        .bg(t.muted)
                        .text_xs()
                        .font_family("monospace")
                        .children(texto.lines().map(|l| div().child(l.to_string()))),
                );
        }
        v.into_any_element()
    }

    fn comparacao(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.theme().clone();
        let chips = h_flex()
            .flex_wrap()
            .gap(px(4.))
            .children(Operacao::TODAS.into_iter().map(|op| {
                let ativo = op == self.comparar;
                let botao = if ativo {
                    crate::estilo::botao_primario(
                        SharedString::from(format!("comparar-{}", op.nome())),
                        cx,
                    )
                } else {
                    crate::estilo::botao_contorno(
                        SharedString::from(format!("comparar-{}", op.nome())),
                        cx,
                    )
                };
                botao
                    .label(op.rotulo())
                    .on_click(cx.listener(move |p, _, _, cx| p.comparar(op, cx)))
            }));
        let mut v = v_flex().gap(px(8.)).child(chips);
        let linhas = &self.comparacao;
        if linhas.is_empty() {
            return v
                .child(
                    div()
                        .text_sm()
                        .text_color(t.muted_foreground)
                        .child("Sem sessões com esta operação. Para comparar sistemas, exporte as sessões numa máquina e importe na outra."),
                )
                .into_any_element();
        }
        let mut sistemas: Vec<&str> = linhas.iter().map(|l| l.sistema.as_str()).collect();
        sistemas.sort();
        sistemas.dedup();
        for sistema in sistemas {
            let deste: Vec<&LinhaDeComparacao> =
                linhas.iter().filter(|l| l.sistema == sistema).collect();
            let mut sessoes: Vec<&str> = deste.iter().map(|l| l.sessao_id.as_str()).collect();
            sessoes.sort();
            sessoes.dedup();
            let intervalos: Vec<&LinhaDeComparacao> = deste
                .iter()
                .copied()
                .filter(|l| l.metrica.etapa == Etapa::IntervaloDoQuadro.nome())
                .collect();
            let mediana = |xs: &mut Vec<f32>| -> f32 {
                if xs.is_empty() {
                    return 0.0;
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                xs[xs.len() / 2]
            };
            let p95 = mediana(&mut intervalos.iter().map(|l| l.metrica.p95_ms).collect());
            let media = mediana(&mut intervalos.iter().map(|l| l.metrica.media_ms).collect());
            // O maior gargalo do sistema: a etapa candidata com maior p95 médio.
            let mut por_etapa: std::collections::HashMap<&str, Vec<f32>> = Default::default();
            for l in &deste {
                let e = Etapa::TODAS
                    .into_iter()
                    .find(|e| e.nome() == l.metrica.etapa);
                if e.is_some_and(|e| e.candidata_a_gargalo()) {
                    por_etapa
                        .entry(l.metrica.etapa.as_str())
                        .or_default()
                        .push(l.metrica.p95_ms);
                }
            }
            let gargalo = por_etapa
                .iter()
                .map(|(e, xs)| (*e, xs.iter().sum::<f32>() / xs.len() as f32))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            v = v.child(
                v_flex()
                    .p(px(8.))
                    .gap(px(2.))
                    .rounded(crate::tema::canto(8.))
                    .border_1()
                    .border_color(t.border)
                    .child(
                        div().text_sm().font_weight(FontWeight::SEMIBOLD).child(format!(
                            "{sistema} — {} sessão(ões) · intervalo p95 {} (mediana entre sessões) · ~{:.0} FPS",
                            sessoes.len(),
                            ms(p95),
                            if media > 0.0 { 1000.0 / media } else { 0.0 }
                        )),
                    )
                    .children(gargalo.map(|(e, p)| {
                        let rotulo = Etapa::TODAS
                            .into_iter()
                            .find(|x| x.nome() == e)
                            .map_or(e, |x| x.rotulo());
                        div().text_xs().child(format!("Maior gargalo: {rotulo} — p95 médio {}", ms(p)))
                    }))
                    .children(intervalos.iter().map(|l| {
                        div().text_xs().text_color(t.muted_foreground).child(format!(
                            "{} · {} · {} · {:.0} Hz · p95 {} · pior {} · {} quadros",
                            l.iniciada_em.get(..16).unwrap_or(&l.iniciada_em),
                            l.gpu_nome,
                            l.gpu_backend,
                            l.taxa_do_monitor_hz,
                            ms(l.metrica.p95_ms),
                            ms(l.metrica.pior_ms),
                            l.metrica.amostras
                        ))
                    })),
            );
        }
        v.into_any_element()
    }
}

impl Render for PainelDeDesempenho {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let ativa = super::ativa();
        // No Linux (GNOME, KDE…) o app desenha a barra da janela: sem ela não
        // haveria onde pegar para arrastá-la a outro monitor.
        let barra = crate::janela::app_desenha_a_barra(window).then(|| {
            crate::janela::como_barra_de_titulo(div(), "barra-do-desempenho", window, cx)
                .flex_none()
                .h(px(34.))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .pl(px(12.))
                .pr(px(4.))
                .bg(t.title_bar)
                .text_xs()
                .text_color(t.muted_foreground)
                .child("Desempenho — VintageLightbox")
                .child(crate::janela::controles(
                    "janela-desempenho",
                    t.muted_foreground,
                    window,
                    cx,
                ))
        });

        let estado = if ativa {
            format!(
                "● Gravando {}",
                duracao_legivel(super::duracao().map_or(0.0, |d| d.as_secs_f64() * 1000.0))
            )
        } else if self.montando {
            "Montando o relatório…".into()
        } else if self.sessao.is_some() {
            if self.salva {
                "Parada · salva"
            } else {
                "Parada · não salva"
            }
            .into()
        } else {
            "Sem captura".into()
        };

        let aba = |id: &'static str, rotulo: &'static str, qual: Aba, cx: &mut Context<Self>| {
            let b = if self.aba == qual {
                crate::estilo::botao_primario(id, cx)
            } else {
                crate::estilo::botao_fantasma(id, cx)
            };
            b.label(rotulo).on_click(cx.listener(move |p, _, _, cx| {
                p.aba = qual;
                if qual == Aba::Comparar {
                    p.comparar(p.comparar, cx);
                } else if qual == Aba::Sessoes {
                    p.recarregar_lista(cx);
                }
                cx.notify();
            }))
        };

        let acoes = h_flex()
            .gap(px(6.))
            .flex_wrap()
            .child(
                crate::estilo::botao_primario("desempenho-iniciar", cx)
                    .child(Icon::new(Icone::Zap).size(px(14.)))
                    .label("Iniciar")
                    .disabled(ativa)
                    .on_click(cx.listener(|p, _, _, cx| p.iniciar(cx))),
            )
            .child(
                crate::estilo::botao_perigo("desempenho-parar", cx)
                    .child(Icon::new(Icone::Square).size(px(14.)))
                    .label("Parar")
                    .disabled(!ativa)
                    .on_click(cx.listener(|p, _, _, cx| p.parar(cx))),
            )
            .child(
                crate::estilo::botao_contorno("desempenho-salvar", cx)
                    .child(Icon::new(Icone::Save).size(px(14.)))
                    .label("Salvar no banco")
                    .disabled(self.sessao.is_none() || self.salva || deposito().is_none())
                    .on_click(cx.listener(|p, _, _, cx| p.salvar(cx))),
            )
            .child(
                crate::estilo::botao_contorno("desempenho-copiar", cx)
                    .child(Icon::new(Icone::Copy).size(px(14.)))
                    .label("Copiar relatório")
                    .disabled(!ativa && self.sessao.is_none())
                    .on_click(cx.listener(|p, _, _, cx| p.copiar(cx))),
            )
            .child(
                crate::estilo::botao_fantasma("desempenho-exportar", cx)
                    .child(Icon::new(Icone::Download).size(px(14.)))
                    .label("Exportar")
                    .disabled(self.salvas.is_empty())
                    .on_click(cx.listener(|p, _, _, cx| p.exportar(cx))),
            )
            .child(
                crate::estilo::botao_fantasma("desempenho-importar", cx)
                    .child(Icon::new(Icone::Upload).size(px(14.)))
                    .label("Importar")
                    .on_click(cx.listener(|p, _, _, cx| p.escolher_para_importar(cx))),
            );

        let conteudo = match self.aba {
            Aba::AoVivo => self.ao_vivo(cx),
            Aba::Sessoes => self.sessoes(cx),
            Aba::Comparar => self.comparacao(cx),
        };

        let corpo = v_flex()
            .id("painel-de-desempenho")
            .debug_selector(|| "painel-de-desempenho".into())
            .flex_1()
            .min_h(px(0.))
            .p(px(12.))
            .gap(px(8.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(Icon::new(Icone::ChartColumn).size(px(16.)))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Desempenho"),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(1.))
                            .rounded(crate::tema::canto(10.))
                            .text_xs()
                            .when(ativa, |d| d.bg(t.danger.opacity(0.2)).text_color(t.danger))
                            .when(!ativa, |d| d.bg(t.muted).text_color(t.muted_foreground))
                            .child(estado),
                    )
                    .child(div().flex_1())
                    .child(aba("desempenho-aba-vivo", "Ao vivo", Aba::AoVivo, cx))
                    .child(aba("desempenho-aba-sessoes", "Sessões", Aba::Sessoes, cx))
                    .child(aba(
                        "desempenho-aba-comparar",
                        "Comparar",
                        Aba::Comparar,
                        cx,
                    ))
                    .child(
                        crate::estilo::botao_fantasma("desempenho-fechar", cx)
                            .child(Icon::new(Icone::JanelaFechar).size(px(14.)))
                            .tooltip("Fechar a janela (a captura continua)")
                            .on_click(cx.listener(|p, _, _, cx| p.alternar(cx))),
                    ),
            )
            .child(acoes)
            .children(self.aviso.as_ref().map(|(texto, erro)| {
                div()
                    .text_xs()
                    .text_color(if *erro { t.danger } else { t.muted_foreground })
                    .child(texto.clone())
            }))
            .child(
                div()
                    .id("desempenho-conteudo")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(conteudo),
            );
        v_flex()
            .size_full()
            .bg(t.background)
            .text_color(t.foreground)
            .children(barra)
            .child(corpo)
            .into_any_element()
    }
}

/// Abre a janela do painel, do tamanho de um painel e no meio da tela — o
/// operador a arrasta para o monitor que quiser.
fn abrir_a_janela(entidade: Entity<PainelDeDesempenho>, cx: &mut App) {
    let area = gpui_kit::Bounds::centered(None, gpui_kit::size(px(820.), px(780.)), cx);
    let opcoes = gpui_kit::WindowOptions {
        app_id: Some(crate::menu::APP_ID.into()),
        window_bounds: Some(gpui_kit::WindowBounds::Windowed(area)),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some("Desempenho — VintageLightbox".into()),
            ..Default::default()
        }),
        is_movable: true,
        is_resizable: true,
        is_minimizable: true,
        window_background: gpui_kit::WindowBackgroundAppearance::Opaque,
        window_decorations: crate::janela::decoracoes_ao_abrir(),
        // 🚨 **Sem roubar o foco.** Com a janela principal inativa, o macOS não
        // considera nada dela "sob o mouse", e os sliders ignoram o arrasto
        // até um clique a reativar — o operador abriria o painel e o gesto que
        // queria medir não responderia (achado medindo, 27/09).
        focus: false,
        show: true,
        ..Default::default()
    };
    let raiz = entidade.clone();
    match cx.open_window(opcoes, move |_, _| raiz) {
        Ok(janela) => {
            let id = janela.window_id();
            let fraca = entidade.downgrade();
            entidade.update(cx, |p, cx| {
                p.janela = Some(janela.into());
                // 🚨 O `X` da barra fecha a janela sem passar pelo botão: o
                // rodapé tem de saber, senão o clique seguinte "fecha" de novo.
                p._fechou = Some(cx.on_window_closed(move |cx, fechada| {
                    if fechada == id {
                        let _ = fraca.update(cx, |p, cx| {
                            p.janela = None;
                            cx.emit(MudouOEstado);
                            cx.notify();
                        });
                    }
                }));
                cx.emit(MudouOEstado);
                cx.notify();
            });
        }
        Err(erro) => {
            crate::telemetria::avisar!("⚠️ [Desempenho] não foi possível abrir a janela: {erro}")
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_duracao_sai_em_minutos_e_segundos() {
        assert_eq!(duracao_legivel(0.0), "00:00");
        assert_eq!(duracao_legivel(125_400.0), "02:05");
    }
}
