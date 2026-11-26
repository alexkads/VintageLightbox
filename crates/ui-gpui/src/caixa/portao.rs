//! ⏰ **O portão do caixa do dia** (#52; dono, 25/11/2026: *"Não deve ser
//! possível começar a trabalhar no sistema sem antes abrir o caixa do dia"* e
//! *"às 00:00 o caixa precisa ser fechado!"*).
//!
//! Logo depois da conta, quem opera o caixa cai aqui antes de qualquer outra
//! tela — a prancheta **Main** do canvas "VintageLightbox · Caixa do dia":
//!
//! | O caixa | O portão |
//! |---|---|
//! | aberto e dentro do horário | livre: a raiz solta as telas |
//! | aberto e passado do corte | **Fechar o caixa de DD/MM**, com o motivo do atraso |
//! | nenhum aberto | **Abrir o caixa de hoje** |
//!
//! 🔑 **O portão é a rota do caixa, e não uma tela nova.** O fechamento e a
//! abertura são os diálogos de sempre (`dialogos.rs`), com a mesma gravação,
//! a mesma chave de idempotência e a mesma fila sem internet; aqui fica só a
//! página que explica o passo e o motivo do atraso, que segue para o
//! fechamento. Quem segura as outras telas é a raiz (`app.rs`), que ouve o
//! [`PedidoDoCaixa::Portao`] de cada carga.
//!
//! O horário de corte vem da API (`fechar_ate`, gravado na abertura): com o app
//! aberto na virada, [`Caixa::vigiar_a_virada`] relê o caixa e o portão volta.

use biblioteca_core::caixa::{self as regras, Portao};
use biblioteca_core::dinheiro;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, prelude::*, px, AnyElement, ClickEvent, Context, Div, Entity, FontWeight, Window,
};

use super::dados;
use super::dialogos::{campo, erro_do_form, rotulado, valor_de, TipoDeDialogo};
use super::tela::{com_tecla, Caixa, PedidoDoCaixa, TeclaF7, TeclaF8, Vista, CONTEXTO};
use crate::estilo;

/// O que o portão guarda entre um desenho e outro.
#[derive(Default)]
pub(crate) struct EstadoDoPortao {
    /// A conta passa pelo portão (`regras::passa_pelo_portao`).
    pub ligado: bool,
    /// O último portão contado à raiz.
    pub contado: Option<Portao>,
    /// O caixa vencido já fechou nesta passagem: o passo 1 fica feito.
    pub fechou_o_vencido: bool,
    /// O motivo do atraso, escrito na página antes de contar.
    pub motivo: Option<Entity<InputState>>,
    pub erro: Option<String>,
    /// 🏢 O estúdio escolhido no próprio portão, nesta abertura do app (o
    /// arquivo da máquina também o guarda; a memória basta para não perguntar
    /// de novo enquanto ele relê).
    pub estudio_escolhido: Option<String>,
    /// O dia do caixa vencido (`"24/11"`), para o passo "Fechar o caixa de
    /// DD/MM" seguir com a data depois que ele fecha e sai da vista.
    pub dia_do_vencido: Option<String>,
}

impl Caixa {
    /// 🔐 A raiz soube quem entrou. Ligado, o caixa é lido já, sem esperar a
    /// rota aparecer: é a carga que decide o portão.
    pub fn ligar_o_portao(&mut self, ligado: bool, cx: &mut Context<Self>) {
        self.portao.ligado = ligado;
        self.portao.contado = None;
        self.portao.fechou_o_vencido = false;
        if ligado {
            self.recarregar(cx);
        } else {
            cx.emit(PedidoDoCaixa::Portao(Portao::Livre));
        }
        cx.notify();
    }

    /// 🧾 Uma cobrança no ar ou um diálogo aberto: o portão espera.
    pub fn ocupado(&self) -> bool {
        self.gravando > 0 || self.dialogo.is_some()
    }

    /// O portão pela vista de agora. Sem vista ainda, livre: quem segura o
    /// começo é a raiz, que espera a primeira conta.
    pub fn portao_agora(&self) -> Portao {
        if !self.portao.ligado || self.flutuante() {
            return Portao::Livre;
        }
        self.vista
            .as_ref()
            .map_or(Portao::Livre, |v| regras::portao(true, v.situacao()))
    }

    /// Conta à raiz o portão de agora, quando ele muda — a cada carga.
    pub(super) fn contar_o_portao(&mut self, cx: &mut Context<Self>) {
        if !self.portao.ligado || self.flutuante() {
            return;
        }
        let agora = self.portao_agora();
        // O dia do vencido acompanha cada carga: trocar de estúdio no portão
        // troca de caixa vencido sem o portão mudar de estado.
        if agora == Portao::FecharOVencido {
            self.portao.dia_do_vencido = self
                .vista
                .as_ref()
                .and_then(|v| v.caixa.as_ref())
                .map(dados::CaixaDoBalcao::dia);
        }
        if self.portao.contado == Some(agora) {
            return;
        }
        if self.portao.contado == Some(Portao::FecharOVencido) && agora == Portao::AbrirODeHoje {
            self.portao.fechou_o_vencido = true;
        }
        if agora != Portao::FecharOVencido {
            self.portao.erro = None;
        }
        // 🪟 A página do portão muda e o campo do motivo some: o diálogo aberto
        // por cima (o resumo do fechamento) devolve o foco à página, e não a
        // um campo que não existe mais (achado pelo e2e do ciclo, 26/11).
        self.foco_antes.esquecer();
        self.portao.contado = Some(agora);
        cx.emit(PedidoDoCaixa::Portao(agora));
    }

    /// ⏰ **A virada com o app aberto** — a raiz chama uma vez por segundo. O
    /// caixa da última carga passou do corte e a última conta dizia livre:
    /// relê, e a carga conta o portão de novo. Relê, e não decide pela vista,
    /// porque o caixa pode ter fechado no painel da galeria nesse meio-tempo.
    pub fn vigiar_a_virada(&mut self, cx: &mut Context<Self>) {
        if !self.portao.ligado
            || self.carregando()
            || self.gravando > 0
            || self.portao.contado != Some(Portao::Livre)
        {
            return;
        }
        if self.vista.as_ref().is_some_and(Vista::caixa_vencido) {
            self.recarregar(cx);
        }
    }

    /// 🏢 **O estúdio vem antes do caixa.** Máquina sem estúdio escolhido (o
    /// catálogo novo, ou quem respondeu "Todos os estúdios") e mais de um
    /// estúdio ativo: o portão pergunta em qual estúdio fica este caixa, em vez
    /// de abrir o caixa do primeiro da lista — que seria o de outra cidade
    /// metade das vezes (o mesmo cuidado de 18/09 no caixa).
    pub(super) fn estudio_a_escolher(&self) -> bool {
        let Some(v) = self.vista.as_ref() else {
            return false;
        };
        v.estudios.len() > 1
            && self.estudio_pedido.is_none()
            && self.portao.estudio_escolhido.is_none()
            && crate::sessoes::tela::estudio_de_trabalho_guardado().is_none()
    }

    /// O estúdio escolhido no portão: fica sendo o desta máquina, e o caixa é
    /// relido nele.
    fn escolher_o_estudio_do_balcao(&mut self, estudio: String, cx: &mut Context<Self>) {
        crate::sessoes::tela::guardar_estudio_de_trabalho(&estudio);
        self.portao.estudio_escolhido = Some(estudio.clone());
        self.navegar(Some(estudio), None, cx);
    }

    /// F8 no portão: com o caixa vencido, o motivo vem antes da contagem.
    fn f8_no_portao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogo.is_some() || self.estudio_a_escolher() {
            return;
        }
        match self.portao_agora() {
            Portao::FecharOVencido => self.contar_o_vencido(window, cx),
            Portao::AbrirODeHoje => self.abrir_dialogo(TipoDeDialogo::Abrir, window, cx),
            Portao::Livre => {}
        }
    }

    /// "Contar o caixa de DD/MM": sem o motivo, a página pede; com ele, o
    /// fechamento abre com o motivo já escrito (a API o grava como atraso).
    pub(super) fn contar_o_vencido(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.portao.motivo.is_none() {
            self.portao.motivo = Some(campo(window, cx, ""));
        }
        let motivo = self
            .portao
            .motivo
            .as_ref()
            .map(|m| valor_de(m, cx))
            .unwrap_or_default()
            .trim()
            .to_string();
        if motivo.is_empty() {
            self.portao.erro = Some(regras::SEM_MOTIVO_DO_ATRASO.into());
            self.foco_pendente = self.portao.motivo.clone();
            cx.notify();
            return;
        }
        self.portao.erro = None;
        self.abrir_dialogo(TipoDeDialogo::Fechar, window, cx);
        self.escrever_o_motivo_do_fechamento(motivo, window, cx);
    }

    /// A página do portão, no lugar das colunas do PDV.
    pub(super) fn render_portao(
        &mut self,
        portao: Portao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.portao.motivo.is_none() {
            self.portao.motivo = Some(campo(
                window,
                cx,
                "Ex.: a última sessão acabou tarde e o caixa ficou para depois",
            ));
        }
        let (fundo, texto) = (cx.theme().background, cx.theme().foreground);
        let escolher = self.estudio_a_escolher();
        let principal = match portao {
            _ if escolher => self.pagina_do_estudio(cx),
            Portao::FecharOVencido => self.pagina_do_vencido(cx),
            _ => self.pagina_da_abertura(cx),
        };
        let lateral = match portao {
            Portao::FecharOVencido => vec![
                quadro(
                    "Por que fechar primeiro",
                    &[
                        "Cada caixa é a conta de um dia só. Se a venda de hoje cair no caixa de ontem, a diferença aparece e ninguém sabe de qual dia ela é.",
                        "O fechamento é cego: você conta a gaveta e os comprovantes primeiro, e o app só mostra o esperado depois.",
                        "O dono recebe o resumo com a hora em que o caixa foi fechado e o motivo do atraso.",
                    ],
                    cx,
                ),
                quadro("Enquanto o caixa não abre", &[ENQUANTO_ESPERA], cx),
            ],
            _ => vec![
                quadro(
                    "Como o dia funciona",
                    &[
                        "Vender: escolha a sessão e finalize o pagamento (F4).",
                        "Sangria e suprimento: todo dinheiro que sai ou entra na gaveta fora de uma venda (F6), sempre com o motivo.",
                        "Vendas e estorno: confira os lançamentos e estorne o que foi cobrado errado (F7).",
                        "Conferir e fechar: no fim do dia, conte a gaveta e os comprovantes às cegas (F8).",
                    ],
                    cx,
                ),
                quadro(
                    "O horário de fechamento",
                    &["O caixa fecha no horário de corte do estúdio (à meia-noite, se nada foi mudado). Depois dele o app pede o fechamento antes da próxima venda, e o dono recebe o resumo de cada caixa."],
                    cx,
                ),
                quadro("Enquanto o caixa não abre", &[ENQUANTO_ESPERA], cx),
            ],
        };
        let dialogo = self.render_dialogo(window, cx);
        let recados = self.render_recados(window, cx);
        v_flex()
            .id("caixa-portao")
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(|t, _: &TeclaF7, w, cx| t.tecla(7, w, cx)))
            .on_action(cx.listener(|t, _: &TeclaF8, w, cx| t.f8_no_portao(w, cx)))
            .relative()
            .size_full()
            .min_h(px(0.))
            .bg(fundo)
            .text_color(texto)
            .text_sm()
            .child(
                h_flex()
                    .id("caixa-portao-conteudo")
                    .flex_1()
                    .min_h(px(0.))
                    .items_start()
                    .gap(px(24.))
                    .px(px(32.))
                    .py(px(24.))
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .gap(px(16.))
                            .child(self.passos(portao, cx))
                            .child(principal),
                    )
                    .child(
                        v_flex()
                            .flex_none()
                            .w(px(360.))
                            .gap(px(16.))
                            .children(lateral),
                    ),
            )
            .children(dialogo)
            .children(recados)
            .into_any_element()
    }

    /// 🏢 Em qual estúdio fica este caixa: um botão por estúdio ativo.
    fn pagina_do_estudio(&self, cx: &mut Context<Self>) -> Div {
        let apagado = cx.theme().muted_foreground;
        let estudios: Vec<(String, String)> = self
            .vista
            .as_ref()
            .map(|v| {
                v.estudios
                    .iter()
                    .map(|e| (e.id.clone(), e.nome.clone()))
                    .collect()
            })
            .unwrap_or_default();
        v_flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Em qual estúdio fica este caixa?"),
                    )
                    .child(div().text_color(apagado).child(
                        "Este computador ainda não tem estúdio escolhido. O caixa do dia é de um estúdio só: escolha onde fica esta gaveta. A escolha fica guardada neste computador e vale também para as sessões.",
                    )),
            )
            .child(
                v_flex()
                    .max_w(px(640.))
                    .gap(px(8.))
                    .children(estudios.into_iter().map(|(id, nome)| {
                        let escolhido = id.clone();
                        estilo::botao_contorno(format!("caixa-portao-estudio-{id}"), cx)
                            .w_full()
                            .justify_start()
                            .child(nome)
                            .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                t.escolher_o_estudio_do_balcao(escolhido.clone(), cx)
                            }))
                    })),
            )
    }

    /// 1 Fechar o caixa de DD/MM — 2 Abrir o caixa de hoje — 3 Começar o dia.
    fn passos(&self, portao: Portao, cx: &Context<Self>) -> gpui_kit::Stateful<Div> {
        let dia = self
            .vista
            .as_ref()
            .and_then(|v| v.caixa.as_ref())
            .filter(|_| portao == Portao::FecharOVencido)
            .map(dados::CaixaDoBalcao::dia)
            .or_else(|| self.portao.dia_do_vencido.clone());
        let fechar = (portao == Portao::FecharOVencido || self.portao.fechou_o_vencido)
            .then(|| format!("Fechar o caixa de {}", dia.unwrap_or_default()));
        let mut itens: Vec<(String, bool, bool)> = Vec::new();
        let escolher = self.estudio_a_escolher();
        if escolher || self.portao.estudio_escolhido.is_some() {
            itens.push(("Escolher o estúdio".into(), escolher, !escolher));
        }
        if let Some(f) = fechar {
            itens.push((
                f,
                !escolher && portao == Portao::FecharOVencido,
                self.portao.fechou_o_vencido,
            ));
        }
        itens.push((
            "Abrir o caixa de hoje".into(),
            !escolher && portao == Portao::AbrirODeHoje,
            false,
        ));
        itens.push(("Começar o dia".into(), false, false));
        let (apagado, borda) = (cx.theme().muted_foreground, cx.theme().border);
        let mut fileira = h_flex().id("caixa-portao-passos").gap(px(12.));
        let total = itens.len();
        for (i, (rotulo, agora, feito)) in itens.into_iter().enumerate() {
            fileira = fileira.child(
                h_flex()
                    .gap(px(8.))
                    .text_color(if agora {
                        cx.theme().foreground
                    } else {
                        apagado
                    })
                    .when(agora, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(estilo::selo_do_passo(i + 1, feito))
                    .child(rotulo),
            );
            if i + 1 < total {
                fileira = fileira.child(div().w(px(32.)).h(px(1.)).bg(borda));
            }
        }
        fileira
    }

    /// O caixa vencido: o resumo dele, o motivo e a contagem.
    fn pagina_do_vencido(&self, cx: &mut Context<Self>) -> Div {
        let apagado = cx.theme().muted_foreground;
        let Some(c) = self.vista.as_ref().and_then(|v| v.caixa.as_ref()) else {
            return div();
        };
        let dia = c.dia();
        let sangrias = c.movimentos.len();
        let linhas = [
            (
                "Aberto".to_string(),
                format!("{}, por {}", dados::hora_br(&c.aberto_em), c.operador_email),
            ),
            (
                "Fundo de troco na abertura".into(),
                dinheiro::formatar(c.fundo_de_troco),
            ),
            ("Vendas".into(), c.vendas.len().to_string()),
            ("Sangrias e suprimentos".into(), sangrias.to_string()),
            (
                "Devia fechar às".into(),
                format!("{} de {}", c.hora_do_corte(), fechar_em(c)),
            ),
        ];
        let motivo = self.portao.motivo.clone();
        v_flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Antes de começar, feche o caixa que ficou aberto"),
                    )
                    .child(div().text_color(apagado).child(format!(
                        "O caixa de {dia} passou do horário de fechamento e continua aberto. A gaveta ainda tem o dinheiro dele: feche-o agora, antes da primeira venda de hoje."
                    ))),
            )
            .child(cartao_de_linhas(
                &format!(
                    "Caixa de {dia}{}",
                    c.ponto_de_caixa
                        .as_deref()
                        .map(|p| format!(" · {p}"))
                        .unwrap_or_default()
                ),
                &linhas,
                cx,
            ))
            .child(
                v_flex()
                    .max_w(px(640.))
                    .gap(px(6.))
                    .children(motivo.map(|m| {
                        rotulado(
                            "Por que o caixa não foi fechado no horário?",
                            estilo::campo(Input::new(&m)),
                        )
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child("Obrigatório. Vai no resumo do dono junto com o fechamento."),
                    )
                    .children(self.portao.erro.clone().map(|e| erro_do_form(e, cx))),
            )
            .child(
                h_flex()
                    .max_w(px(640.))
                    .gap(px(8.))
                    .child(
                        com_tecla(
                            estilo::botao_fantasma("caixa-portao-lancamentos", cx),
                            &format!("Ver os lançamentos de {dia}"),
                            "F7",
                        )
                        .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.tecla(7, w, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        com_tecla(
                            estilo::botao_primario("caixa-portao-contar", cx),
                            &format!("Contar o caixa de {dia}"),
                            "F8",
                        )
                        .on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| t.contar_o_vencido(w, cx)),
                        ),
                    ),
            )
    }

    /// Nenhum caixa aberto: o que ter à mão e o botão de abrir.
    fn pagina_da_abertura(&self, cx: &mut Context<Self>) -> Div {
        let apagado = cx.theme().muted_foreground;
        let estudio = self.vista.as_ref().and_then(|v| {
            let id = v.estudio_id.as_deref()?;
            v.estudios
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.nome.clone())
        });
        let linhas = [
            (
                "A gaveta".to_string(),
                "com o fundo de troco que ficou no último fechamento".to_string(),
            ),
            ("A maquininha".into(), "com o lote de ontem fechado".into()),
            (
                "O extrato do PIX".into(),
                "para conferir no fim do dia".into(),
            ),
        ];
        v_flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Abra o caixa de hoje para começar"),
                    )
                    .child(div().text_color(apagado).child(format!(
                        "Cada dia tem a sua conta{}. Conte o dinheiro da gaveta e abra o caixa com esse fundo de troco: as sessões, a importação e o resto do app esperam por ele.",
                        estudio.map(|e| format!(" em {e}")).unwrap_or_default()
                    ))),
            )
            .child(cartao_de_linhas("Antes de abrir, tenha à mão", &linhas, cx))
            .child(
                h_flex().max_w(px(640.)).child(div().flex_1()).child(
                    com_tecla(
                        estilo::botao_primario("caixa-portao-abrir", cx),
                        "Abrir o caixa de hoje",
                        "F8",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.abrir_dialogo(TipoDeDialogo::Abrir, w, cx)
                    })),
                ),
            )
    }
}

/// O que continua à mão enquanto o portão está na tela.
const ENQUANTO_ESPERA: &str = "Sessões, importação, Biblioteca e o resto do app esperam o caixa de hoje. A conta, a troca de estúdio e a atualização do app continuam à mão.";

/// `"25/11"` — o dia do horário de corte vencido.
fn fechar_em(c: &dados::CaixaDoBalcao) -> String {
    c.fechar_ate
        .as_deref()
        .map(dados::data_br)
        .map(|d| d.get(..5).unwrap_or(&d).to_string())
        .unwrap_or_default()
}

/// Um cartão com título e linhas "rótulo · valor" (a `cd-propriedade` da prancheta).
fn cartao_de_linhas(titulo: &str, linhas: &[(String, String)], cx: &Context<Caixa>) -> Div {
    let tema = cx.theme();
    let (apagado, borda) = (tema.muted_foreground, tema.border);
    let total = linhas.len();
    estilo::cartao(cx).max_w(px(640.)).child(
        v_flex()
            .p(px(16.))
            .gap(px(8.))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(titulo.to_string()),
            )
            .children(linhas.iter().enumerate().map(|(i, (rotulo, valor))| {
                h_flex()
                    .min_h(px(32.))
                    .gap(px(12.))
                    .when(i + 1 < total, |d| d.border_b_1().border_color(borda))
                    .child(div().flex_1().text_color(apagado).child(rotulo.clone()))
                    .child(div().child(valor.clone()))
            })),
    )
}

/// Um quadro da coluna da direita: título e parágrafos curtos.
fn quadro(titulo: &str, paragrafos: &[&str], cx: &Context<Caixa>) -> AnyElement {
    let apagado = cx.theme().muted_foreground;
    estilo::cartao(cx)
        .child(
            v_flex()
                .p(px(16.))
                .gap(px(8.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(titulo.to_string()),
                )
                .children(
                    paragrafos
                        .iter()
                        .map(|p| div().text_color(apagado).child(p.to_string())),
                ),
        )
        .into_any_element()
}
