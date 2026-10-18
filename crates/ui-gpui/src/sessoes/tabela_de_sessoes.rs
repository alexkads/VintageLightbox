//! 📋 A lista de sessões no `DataTable` do gpui-kit.
//!
//! # Por que o `DataTable`
//!
//! Até a 0.1.168 a lista era desenhada à mão: duas metades lado a lado (Galeria
//! e Contato congeladas, o resto rolando na horizontal) presas ao mesmo
//! `ScrollHandle`, para caber uma linha de filtros sob o cabeçalho. O dono
//! trocou a linha de filtros pelo componente do kit (18/out/2026: *"mesmo que
//! eu perca os filtros"*). Do kit vêm as colunas congeladas
//! (`Column::fixed_left`), a largura ajustável, a ordenação pelo cabeçalho e a
//! rolagem virtual.
//!
//! # O que é daqui
//!
//! As células — os mesmos selos, números e caminhos da lista de antes — e a
//! ordem escolhida no cabeçalho, que sobrevive a cada lista nova que chega do
//! site ([`SessoesNaTabela::trocar_linhas`]).
//!
//! ⚠️ **A linha abre a sessão pelo clique, e não pela seleção do kit**: as
//! setas do teclado também selecionam, e abrir a galeria a cada seta seria
//! entrar sem querer. Por isso a seleção de linha fica desligada, e a linha
//! aberta acende pelo [`TableDelegate::render_tr`].

use std::cmp::Ordering;

use biblioteca_core::dinheiro;
use biblioteca_core::sessoes::{primeiro_envio_do_fotolivro, SessaoFotografica};
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    div, prelude::*, px, App, Context, Div, IntoElement, SharedString, Stateful, WeakEntity, Window,
};

use super::tela::Sessoes;
use super::tela::{celula_do_caixa, celula_do_fotolivro, data_hora_br, lixeira, selo_da_situacao};

/// A altura da linha: o contato vem em duas linhas de `text_xs`.
pub const ALTURA_DA_LINHA: f32 = 52.;

/// As colunas, na ordem da tela — a do site, sem as de "máquinas".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Col {
    Galeria,
    Contato,
    Situacao,
    Levadas,
    AVenda,
    Compradas,
    Balcao,
    Caixa,
    PosVenda,
    Fotolivro,
    FotolivroEnviadoEm,
    Criada,
    /// 🗑️ Só para o SuperAdmin, como o `podeExcluir` do site.
    Lixeira,
}

impl Col {
    const TODAS: [Col; 13] = [
        Col::Galeria,
        Col::Contato,
        Col::Situacao,
        Col::Levadas,
        Col::AVenda,
        Col::Compradas,
        Col::Balcao,
        Col::Caixa,
        Col::PosVenda,
        Col::Fotolivro,
        Col::FotolivroEnviadoEm,
        Col::Criada,
        Col::Lixeira,
    ];

    fn titulo(self) -> &'static str {
        match self {
            Col::Galeria => "Galeria",
            Col::Contato => "Contato",
            Col::Situacao => "Situação",
            Col::Levadas => "Levadas",
            Col::AVenda => "À venda",
            Col::Compradas => "Compradas",
            Col::Balcao => "Balcão",
            Col::Caixa => "Caixa (PDV)",
            Col::PosVenda => "Pós-venda",
            Col::Fotolivro => "Fotolivro",
            Col::FotolivroEnviadoEm => "Fotolivro enviado em",
            Col::Criada => "Criada",
            Col::Lixeira => "",
        }
    }

    /// 💵 A do caixa é mais larga: ela pode trazer o "Fechar venda" em vez de
    /// um número (dono, 20/set/2026).
    fn largura(self) -> f32 {
        match self {
            Col::Galeria => 216.,
            Col::Contato => 236.,
            Col::Situacao => 200.,
            Col::Levadas | Col::AVenda => 100.,
            Col::Compradas => 120.,
            Col::Balcao | Col::PosVenda => 128.,
            Col::Caixa => 140.,
            Col::Fotolivro | Col::FotolivroEnviadoEm => 166.,
            Col::Criada => 152.,
            Col::Lixeira => 44.,
        }
    }

    /// Números e datas encostam à direita, como no site.
    fn a_direita(self) -> bool {
        !matches!(
            self,
            Col::Galeria | Col::Contato | Col::Situacao | Col::Lixeira
        )
    }

    /// O caixa e o fotolivro são estados, e não números: não há ordem que o
    /// operador reconheça neles.
    fn ordenavel(self) -> bool {
        !matches!(
            self,
            Col::Contato | Col::Caixa | Col::Fotolivro | Col::Lixeira
        )
    }

    fn comparar(self, a: &SessaoFotografica, b: &SessaoFotografica, agora: i64) -> Ordering {
        let balcao = |s: &SessaoFotografica| s.totais.map(|t| t.balcao);
        let pos_venda = |s: &SessaoFotografica| s.totais.map(|t| t.pos_venda);
        let enviado = |s: &SessaoFotografica| {
            primeiro_envio_do_fotolivro(s.fotolivro.as_ref()).map(str::to_string)
        };
        match self {
            Col::Galeria => a.titulo.to_lowercase().cmp(&b.titulo.to_lowercase()),
            Col::Situacao => a.situacao(agora).cmp(&b.situacao(agora)),
            Col::Levadas => a.fotos.levadas_no_balcao.cmp(&b.fotos.levadas_no_balcao),
            Col::AVenda => a.fotos.disponiveis.cmp(&b.fotos.disponiveis),
            Col::Compradas => a.fotos.compradas.cmp(&b.fotos.compradas),
            Col::Balcao => balcao(a).cmp(&balcao(b)),
            Col::PosVenda => pos_venda(a).cmp(&pos_venda(b)),
            Col::FotolivroEnviadoEm => enviado(a).cmp(&enviado(b)),
            Col::Criada => a.criada_em_iso.cmp(&b.criada_em_iso),
            Col::Contato | Col::Caixa | Col::Fotolivro | Col::Lixeira => Ordering::Equal,
        }
    }
}

/// O que o `DataTable` da lista de sessões desenha.
pub struct SessoesNaTabela {
    tela: WeakEntity<Sessoes>,
    /// As sessões visíveis, na ordem em que o site as manda.
    originais: Vec<SessaoFotografica>,
    /// As mesmas, na ordem do cabeçalho.
    linhas: Vec<SessaoFotografica>,
    agora: i64,
    com_lixeira: bool,
    aberta: Option<String>,
    /// A coluna escolhida no cabeçalho e o sentido. `None` = a ordem do site.
    ordem: Option<(Col, ColumnSort)>,
}

impl SessoesNaTabela {
    pub fn nova(tela: WeakEntity<Sessoes>) -> Self {
        Self {
            tela,
            originais: Vec::new(),
            linhas: Vec::new(),
            agora: 0,
            com_lixeira: false,
            aberta: None,
            ordem: None,
        }
    }

    fn colunas(&self) -> impl Iterator<Item = Col> + '_ {
        Col::TODAS
            .into_iter()
            .filter(|c| *c != Col::Lixeira || self.com_lixeira)
    }

    fn coluna(&self, col_ix: usize) -> Col {
        self.colunas().nth(col_ix).unwrap_or(Col::Galeria)
    }

    /// A lista nova que a tela recortou. Devolve `true` quando algo mudou —
    /// e `colunas_mudaram` quando a lixeira entrou ou saiu.
    pub fn trocar_linhas(
        &mut self,
        visiveis: &[&SessaoFotografica],
        agora: i64,
        com_lixeira: bool,
        aberta: Option<&str>,
    ) -> (bool, bool) {
        let colunas_mudaram = self.com_lixeira != com_lixeira;
        let mesmas = self.originais.len() == visiveis.len()
            && self.originais.iter().zip(visiveis).all(|(a, b)| a == *b);
        let aberta_mudou = self.aberta.as_deref() != aberta;
        // ⏱️ O `agora` anda a cada quadro; ele só muda a situação (a que
        // vence), e a tela redesenha a cada segundo de qualquer jeito.
        self.agora = agora;
        if mesmas && !colunas_mudaram && !aberta_mudou {
            return (false, false);
        }
        self.com_lixeira = com_lixeira;
        self.aberta = aberta.map(str::to_string);
        if !mesmas {
            self.originais = visiveis.iter().map(|s| (*s).clone()).collect();
            self.ordenar();
        }
        (true, colunas_mudaram)
    }

    fn ordenar(&mut self) {
        self.linhas = self.originais.clone();
        let Some((col, sentido)) = self.ordem else {
            return;
        };
        let agora = self.agora;
        // 🔑 Ordenação estável: empatados ficam na ordem do site.
        self.linhas.sort_by(|a, b| {
            let ordem = col.comparar(a, b, agora);
            match sentido {
                ColumnSort::Descending => ordem.reverse(),
                _ => ordem,
            }
        });
    }

    /// 🧪 Os títulos, na ordem da tabela.
    #[cfg(test)]
    pub(crate) fn titulos(&self) -> Vec<String> {
        self.linhas.iter().map(|s| s.titulo.clone()).collect()
    }
}

impl TableDelegate for SessoesNaTabela {
    fn columns_count(&self, _cx: &App) -> usize {
        self.colunas().count()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.linhas.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        let col = self.coluna(col_ix);
        let coluna = Column::new(SharedString::from(format!("{col:?}")), col.titulo())
            .width(px(col.largura()))
            .min_width(px(col.largura().min(80.)));
        let coluna = match col {
            // 🧊 **Galeria e Contato ficam congeladas** (dono, 03/out/2026):
            // com o grid rolando na horizontal, o nome da sessão não pode sair
            // da tela.
            Col::Galeria | Col::Contato => coluna.fixed_left(),
            Col::Lixeira => coluna.resizable(false).selectable(false),
            _ => coluna,
        };
        let coluna = if col.a_direita() {
            coluna.text_right()
        } else {
            coluna
        };
        if !col.ordenavel() {
            return coluna;
        }
        // 🔁 O `refresh` relê as colunas: a ordem escolhida volta com elas.
        match self.ordem {
            Some((escolhida, sentido)) if escolhida == col => coluna.sort(sentido),
            _ => coluna.sortable(),
        }
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) {
        let col = self.coluna(col_ix);
        self.ordem = (sort != ColumnSort::Default).then_some((col, sort));
        self.ordenar();
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let col = self.coluna(col_ix);
        div()
            // 🧪 O teste de janela acha o ícone de ordenar ao lado do título.
            .debug_selector(move || format!("cabecalho-{col:?}"))
            .size_full()
            .flex()
            .items_center()
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .when(col.a_direita(), |d| d.justify_end())
            .child(col.titulo())
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let linha = div().id(("sessao", row_ix));
        let Some(sessao) = self.linhas.get(row_ix) else {
            return linha;
        };
        let id = sessao.id.clone();
        let aberta = self.aberta.as_deref() == Some(id.as_str());
        let realce = cx.theme().muted;
        let tela = self.tela.clone();
        linha
            // O e2e do ciclo de vida abre a sessão pela linha, como o operador.
            .debug_selector({
                let id = id.clone();
                move || format!("sessao-{id}")
            })
            .cursor_pointer()
            .when(aberta, |l| l.bg(realce.opacity(0.5)))
            .on_click(move |_ev, _window, cx| {
                let _ = tela.update(cx, |tela, cx| tela.abrir(id.clone(), cx));
            })
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(sessao) = self.linhas.get(row_ix) else {
            return div().into_any_element();
        };
        let col = self.coluna(col_ix);
        let apagado = cx.theme().muted_foreground;
        let valor_ou_traco = |centavos: Option<i64>| match centavos {
            Some(c) if c > 0 => dinheiro::formatar(c),
            _ => "—".to_string(),
        };
        let celula = div()
            .size_full()
            .flex()
            .items_center()
            .text_sm()
            .when(col.a_direita(), |d| d.justify_end());
        match col {
            Col::Galeria => celula
                // 🧪 Para o teste de janela medir a coluna congelada.
                .debug_selector({
                    let id = sessao.id.clone();
                    move || format!("sessao-galeria-{id}")
                })
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(div().w_full().truncate().child(sessao.titulo.clone()))
                .into_any_element(),
            Col::Contato => celula
                .flex_col()
                .items_start()
                .justify_center()
                .text_xs()
                .text_color(apagado)
                .when(sessao.email.is_none() && sessao.whatsapp.is_none(), |d| {
                    d.child("—")
                })
                .children(
                    sessao
                        .email
                        .clone()
                        .map(|e| div().w_full().truncate().child(e)),
                )
                .children(
                    sessao
                        .whatsapp
                        .clone()
                        .map(|w| div().w_full().truncate().child(w)),
                )
                .into_any_element(),
            Col::Situacao => celula
                // 🧪 A primeira coluna que rola, para o teste de janela.
                .debug_selector({
                    let id = sessao.id.clone();
                    move || format!("sessao-situacao-{id}")
                })
                .gap(px(6.))
                .child(selo_da_situacao(sessao.situacao(self.agora), cx))
                // 🏢 **A sessão sem estúdio é marcada aqui** (dono,
                // 18/set/2026: *"talvez alguma indicação no grid quando a
                // sessão estiver com esse problema, pois atrapalha até o
                // fechamento de caixa"*). Ela não entra em caixa nenhum
                // enquanto ninguém a corrigir, e é o cabeçalho da galeria que
                // corrige.
                .when(sessao.sem_estudio, |c| {
                    c.child(
                        crate::estilo::selo_colorido(crate::tema::cores::selo_ambar())
                            .child("Sem estúdio"),
                    )
                })
                .into_any_element(),
            Col::Levadas => celula
                .child(sessao.fotos.levadas_no_balcao.to_string())
                .into_any_element(),
            Col::AVenda => celula
                .child(sessao.fotos.disponiveis.to_string())
                .into_any_element(),
            Col::Compradas => celula
                .child(sessao.fotos.compradas.to_string())
                .into_any_element(),
            Col::Balcao => celula
                .child(valor_ou_traco(sessao.totais.map(|t| t.balcao)))
                .into_any_element(),
            Col::Caixa => celula
                .child(celula_do_caixa(
                    sessao.caixa.as_ref(),
                    sessao.fotos.levadas_no_balcao,
                    sessao.id.clone(),
                    apagado,
                    self.tela.clone(),
                ))
                .into_any_element(),
            Col::PosVenda => celula
                .child(valor_ou_traco(sessao.totais.map(|t| t.pos_venda)))
                .into_any_element(),
            Col::Fotolivro => celula
                .child(celula_do_fotolivro(
                    sessao.fotolivro.as_ref(),
                    sessao.caixa.as_ref(),
                    apagado,
                ))
                .into_any_element(),
            // 📖 Quando o livro foi ao cliente: o primeiro envio.
            Col::FotolivroEnviadoEm => celula
                .text_xs()
                .text_color(apagado)
                .child(
                    primeiro_envio_do_fotolivro(sessao.fotolivro.as_ref())
                        .map(data_hora_br)
                        .unwrap_or_else(|| "—".into()),
                )
                .into_any_element(),
            Col::Criada => celula
                .text_xs()
                .text_color(apagado)
                .child(data_hora_br(&sessao.criada_em_iso))
                .into_any_element(),
            Col::Lixeira => celula
                .justify_center()
                .child(lixeira(sessao, apagado, self.tela.clone(), cx))
                .into_any_element(),
        }
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.))
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Nenhuma sessão com esse nome ou nessa situação.")
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _cx: &App) -> String {
        let Some(sessao) = self.linhas.get(row_ix) else {
            return String::new();
        };
        match self.coluna(col_ix) {
            Col::Galeria => sessao.titulo.clone(),
            Col::Levadas => sessao.fotos.levadas_no_balcao.to_string(),
            Col::AVenda => sessao.fotos.disponiveis.to_string(),
            Col::Compradas => sessao.fotos.compradas.to_string(),
            Col::Criada => sessao.criada_em_iso.clone(),
            _ => String::new(),
        }
    }
}
