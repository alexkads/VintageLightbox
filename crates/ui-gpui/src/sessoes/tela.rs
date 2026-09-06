//! A lista de sessões fotográficas, no mesmo desenho da rota do site.
//!
//! # O que é daqui e o que é do core
//!
//! Daqui é o desenho: a barra, os cartões de total, a tabela, o formulário. As
//! **contas** — quem está em que situação, o que a busca acha, quanto o recorte
//! somou — são de [`biblioteca_core::sessoes`], porque são as mesmas do site.
//!
//! # Duas armadilhas que a tela do site já evitou, e que valem aqui
//!
//! 1. 💰 **Os totais são do recorte visível, e a tela diz isso.** Somar o
//!    conjunto e mostrar ao lado de uma lista filtrada é um número que responde
//!    outra pergunta.
//! 2. ⚠️ **A galeria sem totais é contada à parte.** Ela vem de uma API anterior
//!    ao campo; tratada como zero, faria o rodapé anunciar menos do que o real
//!    sem nada dizendo por quê.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::dinheiro;
use biblioteca_core::sessoes::{
    self, ContagemDeFotos, Criterio, SessaoFotografica, Situacao, Totais,
};
use domain::services::pos_venda::{GaleriaDoPainel, NovaGaleria, Produto, Sessao};
use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};

use crate::pos_venda::porta::{Publicador, Recado};

/// De quanto em quanto a tela pergunta se o site respondeu.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// A sessão fotográfica escolhida para receber as fotos.
///
/// Quem escuta é a raiz: é ela que leva o id para o pós-venda e — quando o
/// passo 3 do fluxo entrar — para a classificação, que sobe a foto.
pub struct Escolhida(pub String);

impl EventEmitter<Escolhida> for Sessoes {}

pub struct Sessoes {
    publicador: Arc<dyn Publicador>,
    /// `None` no modo offline: a tela existe, e diz que precisa de conta.
    sessao: Option<Sessao>,
    galerias: Vec<GaleriaDoPainel>,
    produtos: Vec<Produto>,
    /// Qual sessão está aberta para receber fotos.
    aberta: Option<String>,
    busca: gpui::Entity<InputState>,
    situacao: Option<Situacao>,
    carregando: bool,
    erro: Option<SharedString>,
    /// O formulário de abrir sessão, quando aparece.
    nova: Option<Nova>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

/// Os campos de abrir uma sessão nova.
struct Nova {
    titulo: gpui::Entity<InputState>,
    email: gpui::Entity<InputState>,
    whatsapp: gpui::Entity<InputState>,
    produto_id: Option<String>,
    enviando: bool,
}

impl Sessoes {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nome, e-mail ou WhatsApp…"));
        Self {
            publicador,
            sessao: None,
            galerias: Vec::new(),
            produtos: Vec::new(),
            aberta: None,
            busca,
            situacao: None,
            carregando: false,
            erro: None,
            nova: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    /// A sessão do site chegou da porta do app: dá para listar.
    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        self.sessao = Some(sessao);
        self.recarregar(cx);
    }

    pub fn recarregar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.carregando = true;
        self.erro = None;
        self.publicador
            .galerias(sessao.clone(), self.recados.0.clone());
        self.publicador.produtos(sessao, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// As sessões como o core as entende — a tradução acontece num lugar só.
    fn para_o_core(&self) -> Vec<SessaoFotografica> {
        self.galerias
            .iter()
            .map(|g| SessaoFotografica {
                id: g.id.clone(),
                titulo: g.titulo.clone(),
                email: g.email.clone(),
                whatsapp: g.whatsapp.clone(),
                criada_em_iso: g.criada_em_iso.clone(),
                expira_em: g.expira_em,
                user_id: g.user_id.clone(),
                fotos: ContagemDeFotos {
                    levadas_no_balcao: g.fotos.levadas_no_balcao,
                    disponiveis: g.fotos.disponiveis,
                    compradas: g.fotos.compradas,
                    apagadas: g.fotos.apagadas,
                },
                // 🔑 O decimal em texto do site vira centavos aqui, com o mesmo
                // leitor que o campo de preço usa. `None` continua `None`: a
                // galeria sem totais tem de chegar ao rodapé como "não sei".
                totais: g.totais.as_ref().map(|t| Totais {
                    balcao: dinheiro::ler_campo(&t.balcao).unwrap_or(0),
                    pos_venda: dinheiro::ler_campo(&t.pos_venda).unwrap_or(0),
                }),
            })
            .collect()
    }

    fn criterio(&self, cx: &Context<Self>) -> Criterio {
        Criterio {
            busca: self.busca.read(cx).value().to_string(),
            situacao: self.situacao,
        }
    }

    /// Se há busca ou filtro — é o que faz os totais falarem em "recorte".
    pub fn filtrando(&self, cx: &Context<Self>) -> bool {
        !self.busca.read(cx).value().trim().is_empty() || self.situacao.is_some()
    }

    pub fn limpar_filtros(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        self.situacao = None;
        cx.notify();
    }

    pub fn filtrar_por(&mut self, situacao: Option<Situacao>, cx: &mut Context<Self>) {
        self.situacao = situacao;
        cx.notify();
    }

    /// Abre a sessão para receber fotos. É o "editar" desta tela: o site edita
    /// a galeria entrando nela, e aqui entrar é escolhê-la como destino.
    ///
    /// ⚠️ **Título e contato não se editam por aqui**, e não é esquecimento: a
    /// API não expõe `PATCH /galerias/{id}` para eles. O que ela expõe é o
    /// estúdio, e isso é escolha de outra tela.
    pub fn abrir(&mut self, id: String, cx: &mut Context<Self>) {
        self.aberta = Some(id.clone());
        cx.emit(Escolhida(id));
        cx.notify();
    }

    pub fn aberta(&self) -> Option<&str> {
        self.aberta.as_deref()
    }

    pub fn quantas(&self) -> usize {
        self.galerias.len()
    }

    /// Abre o formulário de sessão nova, com o produto da última já escolhido.
    ///
    /// 🔑 **A sugestão é o produto da sessão mais recente**, como no site: um
    /// estúdio cobra a mesma faixa a semana inteira, e a alternativa seria
    /// escolher de novo a cada cliente.
    pub fn comecar_nova(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sugerido = self
            .galerias
            .first()
            .map(|g| g.produto_id.clone())
            .or_else(|| self.produtos.first().map(|p| p.id.clone()));

        self.nova = Some(Nova {
            titulo: cx.new(|cx| InputState::new(window, cx).placeholder("Ensaio da Maria")),
            email: cx.new(|cx| InputState::new(window, cx).placeholder("cliente@exemplo.com")),
            whatsapp: cx.new(|cx| InputState::new(window, cx).placeholder("(47) 99999-8888")),
            produto_id: sugerido,
            enviando: false,
        });
        cx.notify();
    }

    pub fn cancelar_nova(&mut self, cx: &mut Context<Self>) {
        self.nova = None;
        cx.notify();
    }

    pub fn abrindo_nova(&self) -> bool {
        self.nova.is_some()
    }

    /// Manda abrir a sessão no site.
    ///
    /// ⚠️ **Ao menos um contato**, e quem confere de verdade é o site: sem
    /// e-mail nem WhatsApp não há como o cliente receber o link, e a galeria
    /// nasceria inalcançável.
    pub fn criar(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(nova)) = (self.sessao.clone(), self.nova.as_mut()) else {
            return;
        };
        if nova.enviando {
            return;
        }

        let titulo = nova.titulo.read(cx).value().trim().to_string();
        let email = nao_vazio(&nova.email.read(cx).value());
        let whatsapp = nao_vazio(&nova.whatsapp.read(cx).value());
        let Some(produto_id) = nova.produto_id.clone() else {
            self.erro = Some("nenhuma faixa de preço disponível — confira o catálogo".into());
            cx.notify();
            return;
        };
        if titulo.is_empty() || (email.is_none() && whatsapp.is_none()) {
            self.erro = Some("dê um título e ao menos um contato do cliente".into());
            cx.notify();
            return;
        }

        nova.enviando = true;
        self.erro = None;
        self.publicador.criar_galeria(
            sessao,
            NovaGaleria {
                titulo,
                email,
                whatsapp,
                produto_id,
            },
            self.recados.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// Preenche o formulário de sessão nova — só para os testes da raiz, que
    /// não alcançam os campos privados.
    #[cfg(test)]
    pub fn preencher_para_teste(
        &mut self,
        titulo: &str,
        email: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A faixa sugerida vem da sessão mais recente; sem nenhuma, do catálogo.
        if let Some(nova) = self.nova.as_mut() {
            if nova.produto_id.is_none() {
                nova.produto_id = Some("p1".into());
            }
        }
        let Some(nova) = self.nova.as_ref() else {
            return;
        };
        let (titulo, email) = (titulo.to_string(), email.to_string());
        nova.titulo
            .update(cx, |estado, cx| estado.set_value(titulo, window, cx));
        nova.email
            .update(cx, |estado, cx| estado.set_value(email, window, cx));
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Galerias(lista) => {
                    self.carregando = false;
                    self.galerias = lista;
                }
                Recado::Produtos(lista) => self.produtos = lista,
                Recado::Criada(galeria) => {
                    self.nova = None;
                    // A sessão recém-aberta já fica escolhida: quem a criou vai
                    // subir foto nela agora, e não daqui a três telas.
                    self.aberta = Some(galeria.id.clone());
                    cx.emit(Escolhida(galeria.id));
                    self.recarregar(cx);
                }
                Recado::Falhou(erro) => {
                    self.carregando = false;
                    if let Some(nova) = self.nova.as_mut() {
                        nova.enviando = false;
                    }
                    self.erro = Some(erro.into());
                }
                // Entrar, publicar e link são de outras telas.
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.carregando || self.nova.as_ref().is_some_and(|n| n.enviando);
        if !continua {
            self.colhendo = false;
        }
        continua
    }
}

fn nao_vazio(texto: &str) -> Option<String> {
    let limpo = texto.trim();
    (!limpo.is_empty()).then(|| limpo.to_string())
}

impl Render for Sessoes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let agora = agora_em_segundos();
        let todas = self.para_o_core();
        let contagens = sessoes::contar_por_situacao(&todas, agora);
        let visiveis = sessoes::filtrar(&todas, &self.criterio(cx), agora);
        let soma = sessoes::somar_totais(&visiveis);
        let filtrando = self.filtrando(cx);

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .size_full()
            .p(px(12.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.barra(&contagens, cx))
            .child(self.indicadores(&soma, visiveis.len(), filtrando, cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(div().text_xs().text_color(cx.theme().danger).child(erro))
            })
            .when(self.sessao.is_none(), |tela| {
                tela.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "Esta tela precisa da conta do site. Feche e abra o app para entrar.",
                        ),
                )
            })
            .children(self.formulario(cx))
            .child(self.tabela(&visiveis, agora, cx))
    }
}

impl Sessoes {
    fn barra(&self, contagens: &sessoes::Contagens, cx: &mut Context<Self>) -> impl IntoElement {
        let filtrando = self.filtrando(cx);
        let ativa = self.situacao;

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .child(div().w(px(260.)).child(Input::new(&self.busca).xsmall()))
            .child(
                Button::new("sessoes-todas")
                    .label(format!("Todas {}", contagens.todas))
                    .xsmall()
                    .selected(ativa.is_none())
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.filtrar_por(None, cx))),
            )
            .children(Situacao::TODAS.into_iter().filter_map(|situacao| {
                let quantas = contagens.de(situacao);
                // 🔑 A situação vazia some — **menos** quando é a escolhida:
                // sumir o filtro ativo tiraria o caminho de volta.
                if quantas == 0 && ativa != Some(situacao) {
                    return None;
                }
                Some(
                    Button::new(SharedString::from(format!(
                        "sessoes-{}",
                        situacao.como_texto()
                    )))
                    .label(format!("{} {quantas}", situacao.rotulo()))
                    .xsmall()
                    .selected(ativa == Some(situacao))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.filtrar_por(Some(situacao), cx)
                    })),
                )
            }))
            .when(filtrando, |barra| {
                barra.child(
                    Button::new("sessoes-limpar")
                        .label("limpar")
                        .xsmall()
                        .ghost()
                        .on_click(
                            cx.listener(|tela, _ev, window, cx| tela.limpar_filtros(window, cx)),
                        ),
                )
            })
            .child(div().flex_1())
            .child(
                Button::new("sessoes-nova")
                    .label("Nova sessão")
                    .xsmall()
                    .primary()
                    .disabled(self.sessao.is_none() || self.abrindo_nova())
                    .on_click(cx.listener(|tela, _ev, window, cx| tela.comecar_nova(window, cx))),
            )
            .child(
                Button::new("sessoes-recarregar")
                    .label(if self.carregando {
                        "Lendo…"
                    } else {
                        "Recarregar"
                    })
                    .xsmall()
                    .disabled(self.sessao.is_none() || self.carregando)
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.recarregar(cx))),
            )
    }

    fn indicadores(
        &self,
        soma: &sessoes::Soma,
        quantas: usize,
        filtrando: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let cartao = |rotulo: String, valor: String, nota: &'static str, cx: &mut Context<Self>| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(10.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(rotulo),
                )
                .child(div().text_lg().child(valor))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(nota),
                )
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(cartao(
                        if filtrando {
                            "Balcão, no recorte".into()
                        } else {
                            "Pago no balcão".to_string()
                        },
                        dinheiro::formatar(soma.balcao),
                        "fotos levadas na hora, pelo preço cheio ou pelo negociado",
                        cx,
                    ))
                    .child(cartao(
                        if filtrando {
                            "Pós-venda, no recorte".into()
                        } else {
                            "Pago no pós-venda".to_string()
                        },
                        dinheiro::formatar(soma.pos_venda),
                        "fotos que o cliente voltou e comprou pela galeria",
                        cx,
                    )),
            )
            // 💰 A tela dizendo de qual conjunto o número é. Sem esta linha, um
            // total de recorte ao lado de uma lista filtrada responde outra
            // pergunta — e ninguém tem como saber qual.
            .when(filtrando || soma.sem_totais > 0, |bloco| {
                let mut frase = String::new();
                if filtrando {
                    frase.push_str(&format!(
                        "Somando as {quantas} sessões deste recorte, não o total. "
                    ));
                }
                if soma.sem_totais > 0 {
                    frase.push_str(&format!(
                        "{} sessão(ões) ainda sem os totais na API — o número está menor que o real.",
                        soma.sem_totais
                    ));
                }
                bloco.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(frase),
                )
            })
    }

    fn formulario(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let nova = self.nova.as_ref()?;
        let campo = |rotulo: &str, estado: &gpui::Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(rotulo.to_string()),
                )
                .child(Input::new(estado).xsmall())
        };

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .child(div().text_xs().child("Nova sessão fotográfica"))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(campo("título", &nova.titulo))
                        .child(campo("e-mail do cliente", &nova.email))
                        .child(campo("WhatsApp", &nova.whatsapp)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Ao menos um contato: é por ele que o cliente recebe o link."),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(
                            Button::new("sessoes-cancelar")
                                .label("Cancelar")
                                .xsmall()
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.cancelar_nova(cx)),
                                ),
                        )
                        .child(
                            Button::new("sessoes-criar")
                                .label(if nova.enviando {
                                    "Abrindo…"
                                } else {
                                    "Abrir sessão"
                                })
                                .xsmall()
                                .primary()
                                .disabled(nova.enviando)
                                .on_click(cx.listener(|tela, _ev, _window, cx| tela.criar(cx))),
                        ),
                ),
        )
    }

    fn tabela(
        &self,
        visiveis: &[&SessaoFotografica],
        agora: i64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if visiveis.is_empty() {
            let frase = if self.galerias.is_empty() {
                "Nenhuma sessão ainda. Abra a primeira em \"Nova sessão\"."
            } else {
                "Nenhuma sessão bate com a busca."
            };
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(frase)
                .into_any_element();
        }

        let cabecalho = |texto: &'static str| {
            div()
                .flex_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(texto)
        };

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .pb(px(4.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(cabecalho("Sessão"))
                    .child(cabecalho("Contato"))
                    .child(cabecalho("Situação"))
                    .child(cabecalho("Levadas"))
                    .child(cabecalho("À venda"))
                    .child(cabecalho("Compradas"))
                    .child(cabecalho("Criada")),
            )
            .children(visiveis.iter().map(|sessao| {
                let id = sessao.id.clone();
                let e_a_aberta = self.aberta.as_deref() == Some(sessao.id.as_str());
                div()
                    .id(SharedString::from(format!("sessao-{}", sessao.id)))
                    .flex()
                    .gap(px(8.))
                    .py(px(6.))
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .when(e_a_aberta, |linha| linha.bg(cx.theme().muted))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| tela.abrir(id.clone(), cx)))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .child(sessao.titulo.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                sessao
                                    .email
                                    .clone()
                                    .or_else(|| sessao.whatsapp.clone())
                                    .unwrap_or_else(|| "—".into()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .child(sessao.situacao(agora).rotulo()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .child(sessao.fotos.levadas_no_balcao.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .child(sessao.fotos.disponiveis.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .child(sessao.fotos.compradas.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(sessao.criada_em_iso.clone()),
                    )
            }))
            .into_any_element()
    }
}

/// Agora, em segundos desde a época — o que a conta da situação compara.
fn agora_em_segundos() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use domain::services::pos_venda::ContagemDeFotos as ContagemDaApi;
    use gpui::TestAppContext;
    use std::sync::Mutex;

    fn galeria(id: &str, titulo: &str, email: Option<&str>) -> GaleriaDoPainel {
        GaleriaDoPainel {
            id: id.into(),
            titulo: titulo.into(),
            email: email.map(str::to_string),
            whatsapp: None,
            produto_id: "p1".into(),
            user_id: None,
            criada_em_iso: "2026-09-06".into(),
            expira_em: None,
            fotos: ContagemDaApi {
                disponiveis: 3,
                ..ContagemDaApi::default()
            },
            totais: None,
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
    ) -> gpui::WindowHandle<Sessoes> {
        cx.update(gpui_component::init);
        cx.add_window(move |window, cx| Sessoes::nova(publicador, window, cx))
    }

    fn colher(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Sessoes>) {
        for _ in 0..10 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
    }

    fn com_sessao(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Sessoes>) {
        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_sessao(
                    Sessao {
                        access_token: "tok".into(),
                    },
                    cx,
                );
            })
            .expect("a janela deve estar aberta");
        colher(cx, janela);
    }

    /// 🚨 A lista chega e a busca acha — pela **mesma conta do site**.
    ///
    /// O que este teste prende não é a busca em si (ela tem 12 testes em
    /// `biblioteca_core::sessoes`), é a **tradução**: a galeria da API vira
    /// `SessaoFotografica` com os campos certos nos lugares certos. Trocar
    /// `disponiveis` por `compradas` na cópia não faria nada falhar — daria a
    /// situação certa com o número errado.
    #[gpui::test]
    fn a_lista_chega_e_a_busca_acha_como_no_site(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira {
            galerias: Mutex::new(vec![
                galeria("g1", "Ensaio do João", Some("joao@exemplo.com")),
                galeria("g2", "Casamento da Maria", Some("maria@outro.com")),
            ]),
            ..Default::default()
        });
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.quantas(), 2);
                let todas = tela.para_o_core();
                assert_eq!(todas[0].fotos.disponiveis, 3, "a contagem atravessou");
                assert_eq!(todas[0].situacao(0), Situacao::AguardandoCliente);

                // "joao" acha "João" — a normalização é a do core.
                tela.busca
                    .update(cx, |estado, cx| estado.set_value("joao", window, cx));
                let achadas = sessoes::filtrar(&todas, &tela.criterio(cx), 0);
                assert_eq!(achadas.len(), 1);
                assert_eq!(achadas[0].titulo, "Ensaio do João");
                assert!(tela.filtrando(cx), "com busca, os totais falam em recorte");

                tela.limpar_filtros(window, cx);
                assert!(!tela.filtrando(cx));
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Sem título ou sem contato nenhum, a sessão não sai.
    ///
    /// Quem confere de verdade é o site — mas deixar sair daqui gastaria uma
    /// ida à rede para trazer de volta um erro que a tela já sabia.
    #[gpui::test]
    fn abrir_sessao_exige_titulo_e_ao_menos_um_contato(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.produtos = vec![Produto {
                    id: "p1".into(),
                    nome: "Foto avulsa".into(),
                    preco: "29.90".into(),
                    inativo: false,
                }];
                tela.comecar_nova(window, cx);

                // Vazio: não sai.
                tela.criar(cx);
                assert!(tela.erro.is_some());

                // Só título, sem contato: também não.
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        assert!(
            publicador.criadas().is_empty(),
            "sem contato o cliente não teria como receber o link"
        );

        janela
            .update(cx, |tela, window, cx| {
                let nova = tela.nova.as_ref().expect("o formulário continua aberto");
                nova.email.update(cx, |estado, cx| {
                    estado.set_value("ana@exemplo.com", window, cx)
                });
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let criadas = publicador.criadas();
        assert_eq!(criadas.len(), 1);
        assert_eq!(criadas[0].titulo, "Ensaio da Ana");
        assert_eq!(criadas[0].email.as_deref(), Some("ana@exemplo.com"));
        assert_eq!(criadas[0].produto_id, "p1", "a faixa sugerida foi junto");
    }

    /// 🔑 A sessão recém-aberta **já fica escolhida**.
    ///
    /// Quem acabou de cadastrar o cliente vai subir foto nele agora, e não daqui
    /// a três telas. É o mesmo encadeamento que "criar coleção leva a seleção
    /// junto" tem na Biblioteca.
    #[gpui::test]
    fn a_sessao_recem_aberta_ja_fica_escolhida(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.produtos = vec![Produto {
                    id: "p1".into(),
                    nome: "Foto avulsa".into(),
                    preco: "29.90".into(),
                    inativo: false,
                }];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                nova.email.update(cx, |estado, cx| {
                    estado.set_value("ana@exemplo.com", window, cx)
                });
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.aberta(), Some("g1"));
                assert!(!tela.abrindo_nova(), "o formulário fechou sozinho");
                assert_eq!(tela.quantas(), 1, "e a lista já a inclui");
            })
            .expect("a janela deve estar aberta");
    }
}
