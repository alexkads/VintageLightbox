//! 🧪 O caixa visto de fora do módulo — o que os cenários de ponta a ponta
//! (`crate::e2e`) precisam ler e fazer no painel da galeria.
//!
//! 🔑 **As teclas passam pelo mesmo `teclar_no_painel`** que o interceptador
//! do teclado chama, e o pagamento pelo mesmo diálogo; nada aqui escreve no
//! estado do caixa por fora.

use biblioteca_core::caixa::{FormaDePagamento, Pessoas};
use gpui_kit::{Context, Window};

use super::tela::Caixa;

impl Caixa {
    /// Uma tecla com o foco no cupom do painel (a lista de itens).
    pub(crate) fn tecla_no_cupom(
        &mut self,
        tecla: &str,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.teclar_no_painel(tecla, shift, false, None, true, false, window, cx)
    }

    /// O diálogo aberto, pelo nome do contexto (`Pagamento`, `Negociacao`…).
    pub(crate) fn dialogo_do_caixa(&self) -> Option<&'static str> {
        self.dialogo_aberto()
    }

    /// Os ids das fotos no cupom e o total, em centavos.
    pub(crate) fn cupom_para_teste(&self) -> (Vec<String>, i64) {
        self.vista.as_ref().map_or((Vec::new(), 0), |v| {
            (
                v.cupom.itens.iter().map(|i| i.foto_id.clone()).collect(),
                v.cupom.total,
            )
        })
    }

    /// O caixa do estúdio está aberto?
    pub(crate) fn caixa_do_estudio_aberto(&self) -> bool {
        self.vista.as_ref().is_some_and(|v| v.caixa.is_some())
    }

    /// Fotógrafo, atendente e auxiliar — o diálogo F3, já respondido.
    pub(crate) fn escolher_as_pessoas(&mut self, funcionario: &str) {
        self.pessoas = Pessoas {
            fotografo: Some(funcionario.into()),
            atendente: Some(funcionario.into()),
            auxiliar: Some(funcionario.into()),
        };
    }

    /// No diálogo de pagamento: a forma da tecla `n` (1 = dinheiro, 2 = Pix…),
    /// o valor digitado e "Lançar".
    pub(crate) fn lancar_pagamento(
        &mut self,
        tecla_da_forma: usize,
        valor: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(forma) = FormaDePagamento::da_tecla(tecla_da_forma) {
            self.escolher_forma_de_pagamento(forma, window, cx);
        }
        self.preencher("valor", valor, window, cx);
        self.lancar(window, cx);
    }

    /// Enter no diálogo aberto.
    pub(crate) fn confirmar_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirmar(window, cx);
    }

    /// Quantos pagamentos o diálogo tem lançados.
    pub(crate) fn pagamentos_lancados(&self) -> usize {
        self.lancados().len()
    }

    /// O número da última venda registrada.
    pub(crate) fn ultima_venda_para_teste(&self) -> Option<i64> {
        self.ultima_venda.as_ref().map(|v| v.numero)
    }

    /// O "×" do diálogo aberto.
    pub(crate) fn fechar_dialogo_do_caixa(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fechar_dialogo(window, cx);
    }

    /// As vendas da sessão, como o diálogo F7 as lista: número, total, o que
    /// já foi estornado e as fotos que continuam vendidas.
    pub(crate) fn vendas_da_sessao_para_teste(&self) -> Vec<(i64, i64, i64, Vec<String>)> {
        self.vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| {
                s.vendas
                    .iter()
                    .map(|v| (v.numero, v.total, v.estornado, v.fotos_vendidas.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A contagem cega foi conferida e o "Fechar" está à vista.
    pub(crate) fn contagem_conferida(&self) -> bool {
        self.fechamento_conferido()
    }

    /// O diálogo de fechamento já mostra o caixa fechado (o resumo final).
    pub(crate) fn caixa_fechado_no_dialogo(&self) -> bool {
        self.fechamento_concluido()
    }

    /// Fotógrafo, atendente e auxiliar gravados pelo diálogo F3.
    pub(crate) fn pessoas_para_teste(&self) -> [Option<String>; 3] {
        [
            self.pessoas.fotografo.clone(),
            self.pessoas.atendente.clone(),
            self.pessoas.auxiliar.clone(),
        ]
    }
}
