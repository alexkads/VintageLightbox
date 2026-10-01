//! A faixa que avisa da versão nova — e o diálogo das novidades.
//!
//! 🔑 **Faixa, e não modal.** Quem está triando 200 fotos não pode ser
//! interrompido por uma janela que exige clique para sumir — o desfecho
//! conhecido disso é o operador aprender a fechar sem ler, e a próxima que
//! importar de verdade some junto. A faixa fica no rodapé, ocupa uma linha e
//! espera. As novidades abrem num diálogo **só quando pedidas**.
//!
//! 🔑 **A faixa mora dentro do rodapé** (`app/rodape.rs`) desde 27/set/2026 —
//! antes ela se sobrepunha ao pé da tela e sumia quando não havia o que
//! dizer. O rodapé fica sempre, com a versão à esquerda; a faixa ocupa o meio
//! dele quando há aviso, e o mesmo diálogo mostra as novidades **desta**
//! versão quando o operador clica na versão.
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
use gpui_kit::component::list::ListItem;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Sizable};
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
    /// O diálogo das novidades **desta** versão (clique na versão do rodapé)
    /// está aberto.
    pub desta_versao_aberta: bool,
    /// 📚 O histórico do `main`, quando já chegou — é dele que vêm as versões
    /// entre a aberta e a anunciada, que o binário não conhece.
    pub historico_do_main: Vec<novidades::Novidades>,
    /// A busca do histórico do `main` está no ar.
    pub buscando_historico: bool,
    /// A versão que o operador escolheu na lista do diálogo. `None` é a de
    /// partida: a anunciada, ou a aberta (rodapé).
    pub em_vista: Option<String>,
    /// O operador pediu "Verificar atualizações" e a resposta não chegou.
    pub verificando: bool,
    /// A versão cuja faixa já foi contada ao servidor como `exibida` — a
    /// mesma faixa volta a cada procura, e contar de novo inflaria o painel.
    pub relatada: Option<String>,
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

    /// A versão que veio só do anúncio do servidor não tem a lista: o botão
    /// das novidades não abre um painel vazio.
    fn tem_novidades(&self) -> bool {
        self.versao
            .as_ref()
            .and_then(|v| v.novidades.as_ref())
            .is_some_and(|n| !n.novidades.is_empty())
    }

    /// 📚 As versões que o diálogo lista, da mais nova para a mais antiga.
    ///
    /// A da versão aberta é o histórico do próprio binário; a da versão nova
    /// junta a anunciada, o histórico do `main` (as do meio) e o do binário.
    pub fn linha_do_tempo(&self) -> Vec<novidades::Novidades> {
        let desta = novidades::desta_versao()
            .into_iter()
            .chain(novidades::historico_desta_versao().iter().cloned());
        if self.desta_versao_aberta {
            return novidades::juntar(desta);
        }
        let anunciada = self.versao.as_ref().map(|v| {
            v.novidades.clone().unwrap_or_else(|| {
                novidades::Novidades::do_anuncio(
                    v.versao.clone(),
                    v.notas.clone().unwrap_or_default(),
                    false,
                )
            })
        });
        novidades::juntar(
            anunciada
                .into_iter()
                .chain(self.historico_do_main.iter().cloned())
                .chain(desta),
        )
    }

    /// Onde a lista está: a versão escolhida ou, sem escolha, a de partida.
    pub fn indice_em_vista(&self, linha: &[novidades::Novidades]) -> usize {
        let partida = if self.desta_versao_aberta {
            Some(env!("CARGO_PKG_VERSION"))
        } else {
            self.versao.as_ref().map(|v| v.versao.as_str())
        };
        self.em_vista
            .as_deref()
            .or(partida)
            .and_then(|procurada| {
                linha.iter().position(|n| {
                    novidades::comparar(&n.versao, procurada) == std::cmp::Ordering::Equal
                })
            })
            .unwrap_or(0)
    }

    /// O clique numa versão da lista (ou em "Mais recente"/"Mais antiga").
    pub fn ver_versao(&mut self, indice: usize) {
        if let Some(n) = self.linha_do_tempo().get(indice) {
            self.em_vista = Some(n.versao.clone());
        }
    }

    /// Vale buscar o histórico do `main`: há versão anunciada que ele ainda
    /// não traz, e nenhuma busca no ar.
    pub fn precisa_do_historico(&self) -> bool {
        !self.buscando_historico
            && self.versao.as_ref().is_some_and(|v| {
                !self
                    .historico_do_main
                    .iter()
                    .any(|n| novidades::comparar(&n.versao, &v.versao) == std::cmp::Ordering::Equal)
            })
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
    /// A versão no rodapé: o que esta versão trouxe.
    VerEstaVersao,
    /// Fecha o diálogo das novidades, seja de qual versão for.
    FecharNovidades,
    /// 📚 Mostra no diálogo a versão desta posição da
    /// [`Estado::linha_do_tempo`].
    VerVersao(usize),
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

/// Se a faixa pede destaque: versão nova importante, ainda por instalar.
pub fn em_destaque(estado: &Estado) -> bool {
    estado.importante() && matches!(estado.visivel(), Some(Aviso::Disponivel { .. }))
}

/// Desenha a faixa **dentro do rodapé**: a frase e os botões, numa linha que
/// encolhe. `agir` recebe o [`Pedido`] de cada botão.
pub fn desenhar(estado: &Estado, cx: &gpui_kit::App, agir: Agir) -> Option<AnyElement> {
    let texto: SharedString = estado.texto()?.into();
    let tema = cx.theme();
    let destaque = em_destaque(estado);
    let mut linha = h_flex().flex_none().items_center().gap(px(4.));
    for (id, rotulo, primario, pedido) in botoes(estado) {
        linha = linha.child(botao(id, rotulo, primario, pedido, &agir));
    }
    Some(
        h_flex()
            .id("faixa-de-atualizacao")
            .debug_selector(|| "faixa-de-atualizacao".into())
            .min_w(px(0.))
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_xs()
                    .text_color(if destaque {
                        tema.foreground
                    } else {
                        tema.muted_foreground
                    })
                    .when(destaque, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(texto.clone()),
            )
            .child(linha)
            // A frase inteira, quando o rodapé estreito a cortou.
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(texto.clone()).build(window, cx)
            })
            .into_any_element(),
    )
}

/// O diálogo "Novidades da versão X": a lista das versões à esquerda e, à
/// direita, o que a escolhida mudou — e, se for mais nova que a aberta, como
/// atualizar.
///
/// 📚 Abre na versão anunciada (ou na aberta, pelo rodapé), e a lista desce
/// por todo o histórico (dono, 01/out/2026: *"pra gente conseguir navegar
/// pelo histórico de novidades anteriores"*).
pub fn desenhar_novidades(
    estado: &Estado,
    rolagem: &gpui_kit::ScrollHandle,
    cx: &gpui_kit::App,
    agir: Agir,
) -> Option<AnyElement> {
    if !estado.desta_versao_aberta && (!estado.novidades_abertas || estado.versao.is_none()) {
        return None;
    }
    let linha = estado.linha_do_tempo();
    let em_vista = estado.indice_em_vista(&linha);
    let n = linha.get(em_vista)?;
    let instalada = env!("CARGO_PKG_VERSION");
    let nova = novidades::mais_nova(&n.versao, instalada);
    let tema = cx.theme();
    let (apagado, borda, aviso) = (tema.muted_foreground, tema.border, tema.warning);

    let mut corpo = v_flex().gap(px(12.));
    if let Some(data) = n.data.as_deref().and_then(data_por_extenso) {
        corpo = corpo.child(
            div()
                .text_xs()
                .text_color(apagado)
                .child(format!("Lançada em {data}")),
        );
    }
    let por_que = if nova {
        "Por que atualizar"
    } else {
        "Por que ela importa"
    };
    corpo = corpo.child(o_que_mudou(n, por_que, aviso));

    let como = estado
        .versao
        .as_ref()
        .filter(|_| estado.novidades_abertas && nova)
        .map(|v| novidades::como_atualizar(v.jeito));
    if let Some(como) = &como {
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
                        .rounded(crate::tema::canto(6.))
                        .border_1()
                        .border_color(borda)
                        .text_xs()
                        .font_family("monospace")
                        .child(comando),
                );
        }
        if como.baixar.is_some() {
            corpo = corpo.child(div().text_xs().text_color(apagado).child(
                "Se a atualização pelo app falhar, feche o app e rode o instalador baixado.",
            ));
        }
    }

    // ← e → andam uma versão; nas pontas o botão apaga.
    let mut navegar = h_flex().gap(px(4.));
    for (id, rotulo, alvo) in [
        (
            "novidades-mais-recente",
            "Mais recente",
            em_vista.checked_sub(1),
        ),
        (
            "novidades-mais-antiga",
            "Mais antiga",
            Some(em_vista + 1).filter(|i| *i < linha.len()),
        ),
    ] {
        let agir = agir.clone();
        navegar = navegar.child(
            Button::new(id)
                .label(rotulo)
                .xsmall()
                .ghost()
                .disabled(alvo.is_none())
                .on_click(move |_ev, w, cx| {
                    if let Some(i) = alvo {
                        agir(Pedido::VerVersao(i), w, cx)
                    }
                }),
        );
    }

    let mut acoes = h_flex().gap(px(6.));
    if let Some(como) = &como {
        if como.comando.is_some() {
            acoes = acoes.child(botao(
                "novidades-copiar",
                "Copiar comando",
                false,
                Pedido::CopiarComando,
                &agir,
            ));
        }
        if como.baixar.is_some() {
            acoes = acoes.child(botao(
                "novidades-baixar",
                "Baixar o instalador",
                false,
                Pedido::BaixarInstalador,
                &agir,
            ));
        }
    }
    if estado.novidades_abertas
        && matches!(estado.visivel(), Some(Aviso::Disponivel { .. }))
        && !estado.instalando
    {
        acoes = acoes.child(botao(
            "novidades-atualizar",
            "Atualizar",
            true,
            Pedido::Instalar,
            &agir,
        ));
    }
    acoes = acoes.child(botao(
        "novidades-fechar",
        "Fechar",
        false,
        Pedido::FecharNovidades,
        &agir,
    ));
    let rodape = h_flex()
        .justify_between()
        .gap(px(6.))
        .child(navegar)
        .child(acoes);

    let anunciada = estado
        .novidades_abertas
        .then(|| estado.versao.as_ref().map(|v| v.versao.clone()))
        .flatten();
    let versoes = lista_de_versoes(&linha, em_vista, anunciada.as_deref(), rolagem, cx, &agir);

    Some(moldura(
        format!("Novidades da versão {}", n.versao),
        versoes,
        corpo.into_any_element(),
        rodape,
        cx,
        agir,
    ))
}

/// A coluna das versões: número, data e o selo da instalada e das novas.
fn lista_de_versoes(
    linha: &[novidades::Novidades],
    em_vista: usize,
    anunciada: Option<&str>,
    rolagem: &gpui_kit::ScrollHandle,
    cx: &gpui_kit::App,
    agir: &Agir,
) -> AnyElement {
    let instalada = env!("CARGO_PKG_VERSION");
    v_flex()
        .id("novidades-versoes")
        .debug_selector(|| "novidades-versoes".into())
        .w(px(220.))
        .flex_none()
        .overflow_y_scroll()
        .track_scroll(rolagem)
        .gap(px(2.))
        .pr(px(8.))
        .children(linha.iter().enumerate().map(|(i, n)| {
            let agir = agir.clone();
            let selo: Option<(&'static str, bool)> = if n.versao == instalada {
                Some(("Instalada", false))
            } else if anunciada.is_some() && novidades::mais_nova(&n.versao, instalada) {
                Some(("Nova", true))
            } else {
                None
            };
            let data = n.data.as_deref().and_then(data_curta);
            // O selo vai na linha do número, e não no `suffix` do `ListItem`:
            // ali a coluna estreita o cortava ("Insta").
            let selo = selo.map(|(rotulo, nova)| {
                if nova {
                    crate::estilo::selo_colorido(crate::tema::cores::selo_esmeralda())
                } else {
                    crate::estilo::selo_contorno(cx)
                }
                .child(rotulo)
            });
            ListItem::new(("novidades-versao", i))
                .debug_selector(move || format!("novidades-versao-{i}"))
                .selected(i == em_vista)
                .rounded(crate::tema::canto(6.))
                .px(px(8.))
                .py(px(4.))
                .child(
                    v_flex()
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(format!("Versão {}", n.versao)),
                                )
                                .children(selo),
                        )
                        .when_some(data, |c, data| {
                            c.child(div().text_xs().opacity(0.7).child(data))
                        }),
                )
                .on_click(move |_ev, w, cx| agir(Pedido::VerVersao(i), w, cx))
        }))
        .into_any_element()
}

const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

fn dia_mes_ano(data: &str) -> Option<(u32, usize, u32)> {
    let mut partes = data.split('-').map(|p| p.parse::<u32>().ok());
    let (ano, mes, dia) = (partes.next()??, partes.next()??, partes.next()??);
    ((1..=12).contains(&mes) && (1..=31).contains(&dia)).then_some((dia, mes as usize - 1, ano))
}

/// `2026-10-01` → `1 de outubro de 2026`.
fn data_por_extenso(data: &str) -> Option<String> {
    let (dia, mes, ano) = dia_mes_ano(data)?;
    Some(format!("{dia} de {} de {ano}", MESES[mes]))
}

/// `2026-10-01` → `1 de out. de 2026`.
fn data_curta(data: &str) -> Option<String> {
    let (dia, mes, ano) = dia_mes_ano(data)?;
    let mes = MESES[mes];
    let curto = if mes.chars().count() <= 5 {
        mes.to_string()
    } else {
        format!("{}.", mes.chars().take(3).collect::<String>())
    };
    Some(format!("{dia} de {curto} de {ano}"))
}

fn secao(titulo: &'static str) -> gpui_kit::Div {
    div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .child(titulo)
}

/// O título, o que mudou e o porquê — igual para a versão nova e a aberta.
fn o_que_mudou(
    n: &novidades::Novidades,
    por_que: &'static str,
    aviso: gpui_kit::Hsla,
) -> AnyElement {
    v_flex()
        .gap(px(12.))
        .when(n.importante, |corpo| {
            corpo.child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(crate::tema::canto(6.))
                    .bg(aviso.opacity(0.15))
                    .text_color(aviso)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Atualização importante"),
            )
        })
        .child(div().text_sm().child(n.titulo.clone()))
        // A versão que só o anúncio do servidor trouxe não tem a lista.
        .when(!n.novidades.is_empty(), |corpo| {
            corpo
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
                                // `min_w(0)`: sem ele o item não quebra a linha e
                                // passa da borda do diálogo.
                                .child(div().flex_1().min_w(px(0.)).child(item.clone()))
                        })),
                )
        })
        .when(!n.por_que_atualizar.trim().is_empty(), |corpo| {
            corpo
                .child(secao(por_que))
                .child(div().text_sm().child(n.por_que_atualizar.clone()))
        })
        .into_any_element()
}

/// O véu escuro e o cartão do diálogo: a lista das versões e o texto, cada
/// um rolando por si. Clicar fora fecha.
fn moldura(
    titulo: String,
    versoes: AnyElement,
    corpo: AnyElement,
    rodape: gpui_kit::Div,
    cx: &gpui_kit::App,
    agir: Agir,
) -> AnyElement {
    let tema = cx.theme();
    let (borda, fundo) = (tema.border, tema.popover);
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
            agir(Pedido::FecharNovidades, w, cx)
        })
        .child(
            v_flex()
                .id("novidades-da-versao")
                .debug_selector(|| "novidades-da-versao".into())
                .w(px(780.))
                .h(px(620.))
                .max_w(gpui_kit::relative(0.94))
                .max_h(gpui_kit::relative(0.9))
                .p(px(16.))
                .gap(px(16.))
                .rounded(crate::tema::canto(12.))
                .border_1()
                .border_color(borda)
                .bg(fundo)
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex_none()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(titulo),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h(px(0.))
                        .items_start()
                        .gap(px(16.))
                        .child(
                            div()
                                .h_full()
                                .flex()
                                .border_r_1()
                                .border_color(borda)
                                .child(versoes),
                        )
                        .child(
                            div()
                                .id("novidades-texto")
                                .debug_selector(|| "novidades-texto".into())
                                .h_full()
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_y_scroll()
                                .pr(px(8.))
                                .child(corpo),
                        ),
                )
                .child(rodape.flex_none()),
        )
        .into_any_element()
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
                    data: None,
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
