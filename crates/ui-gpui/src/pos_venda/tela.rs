//! O modal "Publicar no pós-venda": entrar, escolher o produto e o cliente,
//! e subir a seleção com o que a tecla `B` decidiu.
//!
//! Mesma mecânica de `exportacao/tela.rs`: as fotos são copiadas ao abrir, o
//! trabalho corre numa porta, o andamento chega por canal e a tela o colhe a
//! cada 100 ms.
//!
//! ## 🔑 Nada aqui escolhe o estado da foto
//!
//! O par "levada no balcão / à venda" vem da marcação `B` de cada foto, feita
//! na triagem. Este modal só **mostra a conta** ("3 levadas · 17 à venda") e
//! manda o lote. Uma caixa de opção aqui seria uma segunda lista para a mesma
//! decisão — e no dia em que as duas discordassem, a foto errada iria à venda.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use domain::services::pos_venda::{LinkDeAcesso, NovaGaleria, Produto, Sessao};
use gpui::{div, prelude::*, px, Context, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use use_cases::pos_venda::Progresso;

use super::config::{self, Configuracao};
use super::porta::{Pedido, Publicador, Recado};

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Andamento {
    pub total: usize,
    pub feitas: usize,
    pub falhas: usize,
    pub galeria_id: Option<String>,
    pub terminou: bool,
}

pub struct PosVenda {
    publicador: Arc<dyn Publicador>,
    config: Configuracao,
    /// Copiadas da Biblioteca ao abrir — ver `Exportacao::fotos`.
    fotos: Vec<PhotoViewModel>,
    sessao: Option<Sessao>,
    produtos: Vec<Produto>,
    produto_id: Option<String>,
    titulo: gpui::Entity<InputState>,
    email_do_cliente: gpui::Entity<InputState>,
    whatsapp_do_cliente: gpui::Entity<InputState>,
    entrando: bool,
    andamento: Option<Andamento>,
    ultimo: Option<SharedString>,
    aviso: Option<SharedString>,
    /// O link do cliente, depois de pedido. `None` é "ainda não pedi".
    link: Option<LinkDeAcesso>,
    /// Se o pedido do link está no ar — para o botão não ser apertado duas vezes.
    pedindo_link: bool,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl PosVenda {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        config: Configuracao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let titulo = cx
            .new(|cx| InputState::new(window, cx).placeholder("título da galeria — o cliente lê"));
        let email_do_cliente =
            cx.new(|cx| InputState::new(window, cx).placeholder("e-mail do cliente"));
        let whatsapp_do_cliente =
            cx.new(|cx| InputState::new(window, cx).placeholder("WhatsApp do cliente (opcional)"));

        Self {
            publicador,
            produto_id: config.produto_id.clone(),
            config,
            fotos: Vec::new(),
            sessao: None,
            produtos: Vec::new(),
            titulo,
            email_do_cliente,
            whatsapp_do_cliente,
            entrando: false,
            andamento: None,
            ultimo: None,
            aviso: None,
            recados: channel(),
            link: None,
            pedindo_link: false,
            colhendo: false,
            _colheita: None,
        }
    }

    /// Abre para uma seleção. O andamento do lote anterior é jogado fora aqui,
    /// e não ao fechar — pelo mesmo motivo da exportação.
    pub fn abrir_para(&mut self, fotos: Vec<PhotoViewModel>, cx: &mut Context<Self>) {
        self.fotos = fotos;
        self.andamento = None;
        self.ultimo = None;
        self.aviso = None;
        cx.notify();
    }

    pub fn quantas(&self) -> usize {
        self.fotos.len()
    }

    pub fn quantas_levadas(&self) -> usize {
        self.fotos.iter().filter(|f| f.comprada).count()
    }

    pub fn quantas_a_venda(&self) -> usize {
        self.fotos.len() - self.quantas_levadas()
    }

    pub fn sessao(&self) -> Option<&Sessao> {
        self.sessao.as_ref()
    }

    pub fn produtos(&self) -> &[Produto] {
        &self.produtos
    }

    pub fn produto_id(&self) -> Option<&str> {
        self.produto_id.as_deref()
    }

    pub fn andamento(&self) -> Option<&Andamento> {
        self.andamento.as_ref()
    }

    /// Autoriza pelo navegador — o mesmo gesto da porta do app.
    ///
    /// Aqui ele é a exceção: quem chega neste modal já entrou na abertura, e a
    /// sessão só some se os quinze dias vencerem com o app aberto. O caminho
    /// continua existindo para esse caso, em vez de o modal ficar inútil até o
    /// operador descobrir que precisa reiniciar o app.
    pub fn entrar(&mut self, cx: &mut Context<Self>) {
        if self.entrando {
            return;
        }
        self.entrando = true;
        self.aviso = None;
        self.publicador.autorizar(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// A sessão sem passar pelo login — para o teste da tela.
    #[cfg(test)]
    pub fn entrar_para_teste(&mut self, cx: &mut Context<Self>) {
        self.sessao = Some(Sessao {
            access_token: "tok-de-teste".into(),
            refresh_token: "ref-de-teste".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        });
        self.publicador
            .produtos(self.sessao.clone().unwrap(), self.recados.0.clone());
        self.colher(cx);
    }

    #[cfg(test)]
    pub fn preencher_para_teste(
        &mut self,
        titulo: &str,
        email_do_cliente: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.titulo.update(cx, |campo, cx| {
            campo.set_value(titulo.to_string(), window, cx)
        });
        self.email_do_cliente.update(cx, |campo, cx| {
            campo.set_value(email_do_cliente.to_string(), window, cx)
        });
    }

    pub fn escolher_produto(&mut self, id: String, cx: &mut Context<Self>) {
        self.produto_id = Some(id);
        cx.notify();
    }

    /// A galeria como o formulário a descreve — `None` enquanto falta algo.
    ///
    /// 🚨 **Sem produto não sai**: é ele que dá o preço, e o site recusaria a
    /// galeria — mas recusaria depois de o operador ter preenchido tudo.
    pub fn galeria(&self, cx: &Context<Self>) -> Option<NovaGaleria> {
        let produto_id = self.produto_id.clone()?;
        let titulo = self.titulo.read(cx).value().trim().to_string();
        if titulo.is_empty() {
            return None;
        }
        let email = self.email_do_cliente.read(cx).value().trim().to_string();
        let whatsapp = self.whatsapp_do_cliente.read(cx).value().trim().to_string();
        if email.is_empty() && whatsapp.is_empty() {
            return None;
        }
        Some(NovaGaleria {
            titulo,
            email: (!email.is_empty()).then_some(email),
            whatsapp: (!whatsapp.is_empty()).then_some(whatsapp),
            produto_id,
        })
    }

    pub fn publicar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(galeria) = self.galeria(cx) else {
            return;
        };
        if self.fotos.is_empty() || self.correndo() {
            return;
        }

        // O produto escolhido fica lembrado: o estúdio tem um, e escolhê-lo a
        // cada galeria é atrito puro.
        self.config.produto_id = galeria.produto_id.clone().into();
        config::gravar(&self.config);

        let fotos: Vec<String> = self.fotos.iter().map(|f| f.id.clone()).collect();
        self.andamento = Some(Andamento {
            total: fotos.len(),
            ..Default::default()
        });
        self.ultimo = None;
        self.aviso = None;

        self.publicador.publicar(
            Pedido {
                sessao,
                galeria,
                fotos,
            },
            self.recados.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn correndo(&self) -> bool {
        self.andamento.as_ref().is_some_and(|a| !a.terminou)
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
                Recado::Entrou(sessao) => {
                    self.entrando = false;
                    self.publicador
                        .produtos(sessao.clone(), self.recados.0.clone());
                    self.sessao = Some(sessao);
                }
                // A sessão acabou com o app aberto: o modal volta a mostrar o
                // convite a autorizar, em vez de tentar publicar sem token.
                Recado::SemSessao => {
                    self.entrando = false;
                    self.sessao = None;
                }
                Recado::Produtos(produtos) => {
                    // O produto lembrado vale se ainda existe; senão o primeiro.
                    let lembrado = self
                        .produto_id
                        .as_ref()
                        .filter(|id| produtos.iter().any(|p| &p.id == *id))
                        .cloned();
                    self.produto_id = lembrado.or_else(|| produtos.first().map(|p| p.id.clone()));
                    self.produtos = produtos;
                }
                Recado::Link(link) => {
                    self.pedindo_link = false;
                    // 🔑 **Copia na hora, e mostra assim mesmo.** A área de
                    // transferência é a razão de o botão existir; o texto na
                    // tela é o plano B de quando algo a engoliu — perder o link
                    // é pior que copiá-lo à mão.
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.url.clone()));
                    let dias = link.validade_em_segundos / 86_400;
                    self.ultimo = Some(format!("link copiado ({dias} dias): {}", link.url).into());
                    self.link = Some(link);
                }
                Recado::Andamento(progresso) => self.anotar(progresso),
                // Listar e abrir sessão são gestos da tela de Sessões
                // Fotográficas; este modal só publica. Se chegarem aqui é
                // porque alguém compartilhou o canal por engano.
                Recado::Galerias(_)
                | Recado::Criada(_)
                | Recado::Sincronizou
                | Recado::Pixels { .. }
                | Recado::Aberta(_)
                | Recado::Miniatura { .. } => {}
                Recado::Falhou(erro) => {
                    self.entrando = false;
                    self.pedindo_link = false;
                    if let Some(a) = self.andamento.as_mut() {
                        a.terminou = true;
                    }
                    self.aviso = Some(erro.into());
                }
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.entrando || self.correndo();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn anotar(&mut self, progresso: Progresso) {
        let Some(andamento) = self.andamento.as_mut() else {
            return;
        };
        match progresso {
            Progresso::Comecou { total } => andamento.total = total,
            Progresso::GaleriaCriada { id } => andamento.galeria_id = Some(id),
            Progresso::Enviada { nome, .. } => {
                andamento.feitas += 1;
                self.ultimo = Some(nome.into());
            }
            Progresso::Falhou { nome, erro } => {
                andamento.falhas += 1;
                self.aviso = Some(format!("{nome}: {erro}").into());
            }
            Progresso::ClienteAvisado { falha: None } => {
                self.ultimo = Some("cliente avisado por e-mail".into());
            }
            Progresso::ClienteAvisado {
                falha: Some(motivo),
            } => {
                self.aviso = Some(
                    format!("fotos no ar, mas o aviso não saiu: {motivo} — reenvie pelo painel")
                        .into(),
                );
            }
            Progresso::Terminou {
                galeria_id,
                sucesso,
                falhas,
            } => {
                andamento.galeria_id = Some(galeria_id);
                andamento.feitas = sucesso;
                andamento.falhas = falhas;
                andamento.terminou = true;
            }
        }
    }

    /// A sessão vem da porta do app, e não de um login próprio.
    ///
    /// 🔑 O formulário de entrada daqui continua existindo para o caso de a
    /// sessão morrer com o app aberto — mas com sessão já dada ele não aparece,
    /// e ninguém entra duas vezes na mesma conta na mesma abertura.
    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        self.publicador
            .produtos(sessao.clone(), self.recados.0.clone());
        self.sessao = Some(sessao);
        self.acompanhar(cx);
        cx.notify();
    }

    /// O passo 7 do fluxo: pede o link do cliente e o copia.
    ///
    /// ⚠️ **Só existe depois de a galeria existir.** Antes de publicar não há
    /// o que linkar, e um botão ligado sobre o nada só teria como resposta um
    /// erro do site.
    pub fn pedir_o_link(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria_id)) = (self.sessao.clone(), self.galeria_publicada())
        else {
            return;
        };
        if self.pedindo_link {
            return;
        }
        self.pedindo_link = true;
        self.publicador
            .link(sessao, galeria_id, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// O id da galeria que **esta** publicação criou, se ela terminou.
    pub fn galeria_publicada(&self) -> Option<String> {
        self.andamento
            .as_ref()
            .filter(|a| a.terminou)
            .and_then(|a| a.galeria_id.clone())
    }

    pub fn link(&self) -> Option<&LinkDeAcesso> {
        self.link.as_ref()
    }

    pub fn resumo(&self) -> String {
        match &self.andamento {
            Some(a) if a.terminou && a.falhas > 0 => {
                format!("{} publicadas · {} falharam", a.feitas, a.falhas)
            }
            Some(a) if a.terminou => format!("{} publicadas — galeria no ar", a.feitas),
            Some(a) => format!("{} de {}…", a.feitas + a.falhas, a.total),
            None if self.fotos.is_empty() => "nenhuma foto selecionada".to_string(),
            None => format!(
                "{} levada(s) no balcão · {} à venda",
                self.quantas_levadas(),
                self.quantas_a_venda()
            ),
        }
    }

    fn campo(
        rotulo: &'static str,
        campo: &gpui::Entity<InputState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .w(px(120.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(rotulo),
            )
            .child(div().flex_1().child(Input::new(campo)))
    }

    fn formulario_de_entrada(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "A sessão do estúdio venceu. O navegador vai abrir para \
                         você entrar em {} de novo.",
                        self.config.site()
                    )),
            )
            .child(
                div().flex().justify_end().child(
                    Button::new("pos-venda-entrar")
                        .label(if self.entrando {
                            "Aguardando o navegador…"
                        } else {
                            "Entrar pelo navegador"
                        })
                        .xsmall()
                        .primary()
                        .disabled(self.entrando)
                        .on_click(cx.listener(|tela, _ev, _window, cx| tela.entrar(cx))),
                ),
            )
    }

    fn formulario_da_galeria(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut produtos = Vec::new();
        for produto in &self.produtos {
            let id = produto.id.clone();
            let aceso = self.produto_id.as_deref() == Some(id.as_str());
            let rotulo = format!(
                "{} — R$ {}{}",
                produto.nome,
                produto.preco,
                if produto.inativo {
                    " (fora da vitrine)"
                } else {
                    ""
                }
            );
            produtos.push(
                Button::new(SharedString::from(format!("pos-venda-produto-{id}")))
                    .label(rotulo)
                    .xsmall()
                    .when(aceso, |b| b.primary())
                    .selected(aceso)
                    .disabled(self.correndo())
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.escolher_produto(id.clone(), cx);
                    })),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Produto que dá o preço de cada foto à venda"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .when(self.produtos.is_empty(), |d| {
                        d.child(div().text_xs().text_color(cx.theme().danger).child(
                            "nenhum produto no catálogo — cadastre um em /dashboard/produtos",
                        ))
                    })
                    .children(produtos),
            )
            .child(Self::campo("título", &self.titulo, cx))
            .child(Self::campo("e-mail do cliente", &self.email_do_cliente, cx))
            .child(Self::campo("WhatsApp", &self.whatsapp_do_cliente, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Ao menos um contato. É por ele que a galeria aparece para o cliente — \
                         prefira o e-mail: é por ele que o site manda o link de acesso.",
                    ),
            )
    }
}

impl gpui::Render for PosVenda {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pronto = self.sessao.is_some()
            && !self.fotos.is_empty()
            && !self.correndo()
            && self.galeria(cx).is_some();

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .min_w(px(520.))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} foto(s): {} levada(s) no balcão (download liberado) · {} à venda (com marca d'água). \
                         O original sobe sem marca; o site gera a prévia.",
                        self.quantas(),
                        self.quantas_levadas(),
                        self.quantas_a_venda()
                    )),
            )
            .when(self.sessao.is_none(), |raiz| raiz.child(self.formulario_de_entrada(cx)))
            .when(self.sessao.is_some(), |raiz| raiz.child(self.formulario_da_galeria(cx)))
            .when_some(self.aviso.clone(), |raiz, aviso| {
                raiz.child(div().text_xs().text_color(cx.theme().danger).child(aviso))
            })
            .when_some(self.ultimo.clone(), |raiz, ultimo| {
                raiz.child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(cx.theme().muted_foreground)
                        .child(ultimo),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.resumo()),
                    )
                    // O link só aparece quando há galeria — e aí ele é o
                    // gesto seguinte, no lugar de "Publicar".
                    .when_some(self.galeria_publicada(), |linha, _| {
                        linha.child(
                            Button::new("pos-venda-link")
                                .label(if self.link.is_some() {
                                    "Copiar de novo"
                                } else {
                                    "Copiar link do cliente"
                                })
                                .xsmall()
                                .disabled(self.pedindo_link)
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.pedir_o_link(cx)),
                                ),
                        )
                    })
                    .child(
                        Button::new("pos-venda-publicar")
                            .label(format!("Publicar {}", self.fotos.len()))
                            .xsmall()
                            .primary()
                            .disabled(!pronto)
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.publicar(cx))),
                    ),
            )
    }
}
