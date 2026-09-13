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

use biblioteca_core::dados_do_cliente;
use biblioteca_core::dinheiro;
use biblioteca_core::sessoes::{
    self, ContagemDeFotos, Criterio, SessaoFotografica, Situacao, Totais,
};
use domain::services::pos_venda::{Estudio, GaleriaDoPainel, NovaGaleria, Produto, Sessao};
use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::pos_venda::porta::{Publicador, Recado};
use crate::selos;

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
    /// `None` antes de a porta responder: a tela existe, e diz que precisa de
    /// conta.
    sessao: Option<Sessao>,
    galerias: Vec<GaleriaDoPainel>,
    produtos: Vec<Produto>,
    /// Os estúdios ativos — a escolha obrigatória ao abrir sessão.
    estudios: Vec<Estudio>,
    /// Onde o último estúdio escolhido fica lembrado nesta máquina.
    lembranca: PathBuf,
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
    estudio_id: Option<String>,
    enviando: bool,
}

/// 🎯 O que esta máquina lembra de uma sessão nova para a próxima.
///
/// *"Grave o último preset selecionado e o corte utilizado e o estúdio."* —
/// dono, 2026-09-13. A criação do desktop não escolhe preset nem corte (eles
/// nascem na revelação), então aqui fica **só o estúdio**: um balcão atende
/// no mesmo estúdio o dia inteiro.
///
/// Mesmo molde de `altura_da_tira` e `pos_venda::config`: um arquivo ao lado do
/// catálogo; ler nunca derruba, gravar que falha só não lembra.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Lembranca {
    #[serde(default)]
    estudio_id: Option<String>,
}

#[cfg(not(test))]
fn caminho_da_lembranca() -> PathBuf {
    AppPaths::catalog_root().join("sessao-nova.json")
}

/// 🚨 Nos testes, um arquivo temporário **por tela**: gravar no catálogo do
/// fotógrafo durante o `cargo test` já aconteceu neste repositório, e um
/// arquivo por processo faria os testes em paralelo lembrarem uns dos outros.
#[cfg(test)]
fn caminho_da_lembranca() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMA: AtomicUsize = AtomicUsize::new(0);
    let _ = AppPaths::catalog_root;
    std::env::temp_dir().join(format!(
        "vlb-sessao-nova-teste-{}-{}.json",
        std::process::id(),
        PROXIMA.fetch_add(1, Ordering::SeqCst)
    ))
}

fn estudio_lembrado(caminho: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str::<Lembranca>(&texto)
        .ok()?
        .estudio_id
        .filter(|id| !id.trim().is_empty())
}

fn lembrar_estudio(caminho: &Path, estudio_id: &str) {
    let lembranca = Lembranca {
        estudio_id: Some(estudio_id.to_string()),
    };
    let Ok(texto) = serde_json::to_string_pretty(&lembranca) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    let _ = std::fs::write(caminho, texto);
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
            estudios: Vec::new(),
            lembranca: caminho_da_lembranca(),
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
        self.publicador
            .estudios(sessao.clone(), self.recados.0.clone());
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
    /// ⚠️ **Título e contato não se editam por aqui**: editam-se **dentro** da
    /// sessão (`Detalhe`, "Editar"), pelo `PATCH /galerias/{id}` — o mesmo gesto
    /// da web (dono, 2026-09-13).
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
    ///
    /// 🎯 **O estúdio sugerido é o último escolhido nesta máquina** — e só se
    /// ele ainda estiver entre os ativos; senão fica sem escolha, porque
    /// escolher é obrigatório (dono, 2026-09-13).
    pub fn comecar_nova(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sugerido = self
            .galerias
            .first()
            .map(|g| g.produto_id.clone())
            .or_else(|| self.produtos.first().map(|p| p.id.clone()));
        let estudio = estudio_lembrado(&self.lembranca)
            .filter(|id| self.estudios.iter().any(|e| &e.id == id));

        self.nova = Some(Nova {
            titulo: cx.new(|cx| InputState::new(window, cx).placeholder("Ensaio da Maria")),
            email: cx.new(|cx| InputState::new(window, cx).placeholder("cliente@exemplo.com")),
            whatsapp: cx.new(|cx| InputState::new(window, cx).placeholder("(47) 99999-8888")),
            produto_id: sugerido,
            estudio_id: estudio,
            enviando: false,
        });
        cx.notify();
    }

    /// O preço por foto da sessão nova — a ficha clicada.
    pub fn escolher_faixa(&mut self, produto_id: String, cx: &mut Context<Self>) {
        if let Some(nova) = self.nova.as_mut() {
            nova.produto_id = Some(produto_id);
            cx.notify();
        }
    }

    /// O estúdio da sessão nova — a ficha clicada.
    pub fn escolher_estudio(&mut self, estudio_id: String, cx: &mut Context<Self>) {
        if let Some(nova) = self.nova.as_mut() {
            nova.estudio_id = Some(estudio_id);
            cx.notify();
        }
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
    /// 🔚 **Só o título é obrigatório** (dono, 2026-09-13: *"Não tem que
    /// obrigar o email e whatsapp para criar a sessão. Essas informações são
    /// obrigatórias no final da sessão."*). O contato é pedido pelo "Copiar
    /// link" e pelo "Avisar", dentro da sessão. O e-mail **preenchido** pela
    /// metade continua recusado — a mesma conferência do site
    /// (`biblioteca_core::dados_do_cliente`).
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
        // 🎯 Preço por foto e estúdio são obrigatórios (dono, 2026-09-13).
        let Some(produto_id) = nova.produto_id.clone() else {
            self.erro = Some("escolha o preço por foto da sessão".into());
            cx.notify();
            return;
        };
        let Some(estudio_id) = nova.estudio_id.clone() else {
            self.erro = Some("escolha o estúdio da sessão".into());
            cx.notify();
            return;
        };
        if titulo.is_empty() {
            self.erro = Some("dê um título à sessão — o contato pode ficar para o fim".into());
            cx.notify();
            return;
        }
        if email
            .as_deref()
            .is_some_and(|e| !dados_do_cliente::email_plausivel(e))
        {
            self.erro = Some("o e-mail do cliente não parece completo".into());
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
                estudio_id: Some(estudio_id),
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
            if nova.estudio_id.is_none() {
                nova.estudio_id = Some("s1".into());
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
                Recado::Estudios(lista) => self.estudios = lista,
                Recado::Criada(galeria) => {
                    // 🎯 O estúdio fica lembrado **depois** de a sessão existir:
                    // lembrar uma escolha que o site recusou sugeriria o erro.
                    if let Some(estudio) = self.nova.as_ref().and_then(|n| n.estudio_id.clone()) {
                        lembrar_estudio(&self.lembranca, &estudio);
                    }
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

    /// 🎯 Preço por foto e estúdio, em fichas — os dois obrigatórios (dono,
    /// 2026-09-13). Fichas, e não um `Select`, pelo mesmo motivo da barra: são
    /// poucas opções, e a escolhida tem de se ver sem abrir nada.
    fn escolhas_da_nova(&self, nova: &Nova, cx: &mut Context<Self>) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        let rotulo =
            |texto: &'static str| div().w(px(110.)).text_xs().text_color(apagado).child(texto);

        let faixas = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .child(rotulo("preço por foto *"))
            .when(self.produtos.is_empty(), |linha| {
                linha.child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child("carregando o catálogo…"),
                )
            })
            .children(self.produtos.iter().map(|produto| {
                let id = produto.id.clone();
                Button::new(SharedString::from(format!("sessoes-faixa-{}", produto.id)))
                    .label(format!(
                        "{} — R$ {}",
                        produto.nome,
                        produto.preco.replace('.', ",")
                    ))
                    .xsmall()
                    .selected(nova.produto_id.as_deref() == Some(produto.id.as_str()))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.escolher_faixa(id.clone(), cx)
                    }))
            }));

        let estudios =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.))
                .child(rotulo("estúdio *"))
                .when(self.estudios.is_empty(), |linha| {
                    linha.child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child("nenhum estúdio ativo no site"),
                    )
                })
                .children(self.estudios.iter().map(|estudio| {
                    let id = estudio.id.clone();
                    let nome = if estudio.cidade.trim().is_empty() {
                        estudio.nome.clone()
                    } else {
                        format!("{} — {}", estudio.nome, estudio.cidade)
                    };
                    Button::new(SharedString::from(format!(
                        "sessoes-estudio-{}",
                        estudio.id
                    )))
                    .label(nome)
                    .xsmall()
                    .selected(nova.estudio_id.as_deref() == Some(estudio.id.as_str()))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.escolher_estudio(id.clone(), cx)
                    }))
                }));

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(faixas)
            .child(estudios)
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
                .child(self.escolhas_da_nova(nova, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "O contato pode ficar para o fim: o link e o aviso pedem o e-mail antes de sair.",
                        ),
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
                    // O mesmo filete das linhas, sem cor: é o que mantém as
                    // colunas do cabeçalho alinhadas com as de baixo.
                    .child(div().w(px(2.)).flex_none())
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
                    // A sessão aberta é a linha acesa — no degrau de "ativo",
                    // e não no poço: `muted` virou o fundo de trás de foto
                    // (`crate::tema`), e uma linha mais escura que a tabela
                    // pareceria desligada em vez de escolhida.
                    .when(e_a_aberta, |linha| linha.bg(cx.theme().list_active))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| tela.abrir(id.clone(), cx)))
                    // 🔑 A marca da aberta é um filete âmbar **dentro** da linha,
                    // e não uma borda esquerda: `border_color` no GPUI pinta os
                    // quatro lados de uma vez (a linha de baixo viraria âmbar
                    // junto), e uma borda que só existe na escolhida empurraria
                    // o texto dela 2px para o lado.
                    .child(
                        div()
                            .w(px(2.))
                            .flex_none()
                            .rounded(px(1.))
                            .when(e_a_aberta, |marca| marca.bg(crate::tema::cores::quente())),
                    )
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
                    .child({
                        // 🔑 A situação é a coluna que se lê varrendo a lista de
                        // cima a baixo — "qual delas precisa de mim hoje" —, e
                        // era texto do mesmo cinza de todo o resto.
                        let situacao = sessao.situacao(agora);
                        div().flex_1().flex().child(selos::selo(
                            selos::tom_da_situacao(situacao),
                            situacao.rotulo(),
                            cx,
                        ))
                    })
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

    fn estudio() -> Estudio {
        Estudio {
            id: "s1".into(),
            nome: "Gramado".into(),
            cidade: "Gramado".into(),
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
                        refresh_token: "ref".into(),
                        access_vence_em: i64::MAX,
                        refresh_vence_em: i64::MAX,
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

    /// 🔚 **Só o título é obrigatório** (dono, 2026-09-13): sem contato a
    /// sessão sai, e o contato é pedido no fim. Sem título, ou com o e-mail
    /// escrito pela metade, não sai — o site recusaria, e deixar sair daqui
    /// gastaria uma ida à rede para trazer de volta um erro que a tela já sabia.
    #[gpui::test]
    fn abrir_sessao_exige_so_o_titulo_e_recusa_email_pela_metade(cx: &mut TestAppContext) {
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
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                assert_eq!(
                    nova.produto_id.as_deref(),
                    Some("p1"),
                    "a faixa vem sugerida"
                );
                assert_eq!(nova.estudio_id, None, "sem lembrança, o estúdio é escolha");

                // Vazio: não sai.
                tela.criar(cx);
                assert!(tela.erro.is_some());

                // Com título e o e-mail pela metade: também não.
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                nova.email
                    .update(cx, |estado, cx| estado.set_value("ana@", window, cx));
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "e-mail pela metade é recusado");
            })
            .expect("a janela deve estar aberta");
        assert!(
            publicador.criadas().is_empty(),
            "sem título ou com e-mail torto a sessão não sai"
        );

        // 🎯 Sem contato, com título e faixa, mas **sem estúdio**: não sai; e
        // sem faixa também não (dono, 2026-09-13).
        janela
            .update(cx, |tela, window, cx| {
                let nova = tela.nova.as_ref().expect("o formulário continua aberto");
                nova.email
                    .update(cx, |estado, cx| estado.set_value("", window, cx));
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "sem estúdio não sai");

                tela.escolher_estudio("s1".into(), cx);
                if let Some(nova) = tela.nova.as_mut() {
                    nova.produto_id = None;
                }
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "sem preço por foto não sai");
            })
            .expect("a janela deve estar aberta");
        assert!(publicador.criadas().is_empty());

        // Título, faixa e estúdio, sem contato nenhum: sai.
        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_faixa("p1".into(), cx);
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let criadas = publicador.criadas();
        assert_eq!(criadas.len(), 1);
        assert_eq!(criadas[0].titulo, "Ensaio da Ana");
        assert_eq!(criadas[0].email, None);
        assert_eq!(criadas[0].whatsapp, None);
        assert_eq!(criadas[0].produto_id, "p1");
        assert_eq!(criadas[0].estudio_id.as_deref(), Some("s1"));
    }

    /// 🎯 *"Grave o último preset selecionado e o corte utilizado e o
    /// estúdio."* — dono, 2026-09-13. A criação do desktop não tem preset nem
    /// corte; o estúdio escolhido volta sugerido na próxima sessão — e some da
    /// sugestão se deixar de estar ativo.
    #[gpui::test]
    fn o_ultimo_estudio_volta_sugerido_na_proxima_sessao(cx: &mut TestAppContext) {
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
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                tela.escolher_estudio("s1".into(), cx);
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                assert_eq!(
                    tela.nova.as_ref().and_then(|n| n.estudio_id.as_deref()),
                    Some("s1"),
                    "o estúdio da última sessão volta sugerido"
                );

                tela.estudios = vec![];
                tela.comecar_nova(window, cx);
                assert_eq!(
                    tela.nova.as_ref().and_then(|n| n.estudio_id.clone()),
                    None,
                    "estúdio que saiu da lista não é sugerido"
                );
                let _ = std::fs::remove_file(&tela.lembranca);
            })
            .expect("a janela deve estar aberta");
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
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                nova.email.update(cx, |estado, cx| {
                    estado.set_value("ana@exemplo.com", window, cx)
                });
                tela.escolher_estudio("s1".into(), cx);
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
