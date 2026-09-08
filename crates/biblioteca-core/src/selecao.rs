use std::collections::BTreeSet;

use crate::grade::{faixa_entre, Direcao, Layout};

/// Escolher fotos numa grade — clique, Shift, Ctrl, arrasto e teclado.
///
/// # Por que isto é código de domínio, e não do componente
///
/// A seleção de um gerenciador de fotos tem regras que ninguém percebe até
/// errarem: Shift estende **a partir da âncora**, e a âncora não é o foco; a
/// caixinha do canto alterna sem desmarcar o resto; clicar no vazio limpa,
/// menos se o gesto for aditivo; a seta com Ctrl move o foco sem mexer no que
/// está marcado. Escritas dentro de um `onPointerUp`, essas regras só existem
/// numa tela — e a Biblioteca do desktop teria de reescrevê-las para se parecer
/// com a do site.
///
/// Aqui elas ficam num lugar só, com teste, e as duas telas ficam idênticas de
/// graça.
///
/// # Índices, não ids
///
/// A seleção trabalha por posição **na grade em vigor** (já filtrada). Quem
/// chama traduz para id na hora de agir — e é por isso que trocar o filtro
/// limpa a seleção: as posições passam a apontar para outras fotos.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selecao {
    marcadas: BTreeSet<usize>,
    foco: Option<usize>,
    /// De onde o Shift estende. **Não é o foco**: clicar em A, mover com as
    /// setas até D e apertar Shift+seta estende de A, e não de D.
    ancora: Option<usize>,
}

/// O que o gesto do ponteiro carrega, já lido pela camada de cima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    /// Ctrl (ou ⌘ no Mac): acrescenta em vez de trocar.
    pub aditivo: bool,
    /// Shift: estende a partir da âncora.
    pub faixa: bool,
}

impl Selecao {
    pub fn nova() -> Self {
        Self::default()
    }

    pub fn marcadas(&self) -> impl Iterator<Item = usize> + '_ {
        self.marcadas.iter().copied()
    }

    pub fn quantas(&self) -> usize {
        self.marcadas.len()
    }

    pub fn tem(&self, indice: usize) -> bool {
        self.marcadas.contains(&indice)
    }

    pub fn foco(&self) -> Option<usize> {
        self.foco
    }

    pub fn vazia(&self) -> bool {
        self.marcadas.is_empty()
    }

    /// Trocar o recorte da barra limpa tudo: **o que se vê é o que se opera**.
    pub fn limpar_tudo(&mut self) {
        self.marcadas.clear();
        self.foco = None;
        self.ancora = None;
    }

    /// Esc: desmarca, mas o cursor fica onde está.
    pub fn desmarcar(&mut self) {
        self.marcadas.clear();
    }

    /// O clique terminou sobre a foto `indice`.
    ///
    /// `na_caixa` é o clique na caixinha de marcar do canto do tile: ela alterna
    /// **sem** desfazer o resto da seleção, que é o que a torna útil para
    /// escolher sete fotos espalhadas sem segurar tecla nenhuma.
    pub fn clicar(&mut self, indice: usize, na_caixa: bool, modificadores: Modificadores) {
        if na_caixa || modificadores.aditivo {
            self.alternar(indice);
            self.ancora = Some(indice);
            self.foco = Some(indice);
            return;
        }

        if modificadores.faixa {
            if let Some(ancora) = self.ancora {
                self.marcadas = faixa_entre(ancora, indice).collect();
                self.foco = Some(indice);
                return;
            }
        }

        self.marcadas = std::iter::once(indice).collect();
        self.ancora = Some(indice);
        self.foco = Some(indice);
    }

    /// O clique terminou no vazio — entre tiles, ou depois da última foto.
    ///
    /// Limpa, **menos** quando o gesto era aditivo ou de faixa: quem está com
    /// Ctrl apertado e erra o alvo por dois pixels não quer perder a escolha de
    /// sete fotos.
    pub fn clicar_no_vazio(&mut self, modificadores: Modificadores) {
        if !modificadores.aditivo && !modificadores.faixa {
            self.marcadas.clear();
        }
    }

    /// O arrasto de retângulo, quadro a quadro.
    ///
    /// `base` é a seleção de quando o arrasto começou — só existe se o gesto era
    /// aditivo; senão o arrasto substitui. Recalcular a partir da base a cada
    /// quadro é o que permite **desfazer** ao voltar com o ponteiro, em vez de
    /// ir acumulando o rastro.
    pub fn arrastar(&mut self, base: &BTreeSet<usize>, dentro_do_retangulo: &[usize]) {
        let mut novo = base.clone();
        novo.extend(dentro_do_retangulo.iter().copied());
        self.marcadas = novo;
    }

    /// A seleção de agora, para virar `base` de um arrasto que começa.
    pub fn instantaneo(&self) -> BTreeSet<usize> {
        self.marcadas.clone()
    }

    /// Espaço: alterna a foto em foco.
    pub fn alternar_foco(&mut self) {
        if let Some(foco) = self.foco {
            self.alternar(foco);
            self.ancora = Some(foco);
        }
    }

    /// Ctrl+A: marca a grade inteira, sem mexer no foco.
    pub fn marcar_todas(&mut self, total: usize) {
        self.marcadas = (0..total).collect();
    }

    /// O botão "Selecionar as N visíveis" — que também **desmarca** quando
    /// todas já estão marcadas, porque é o mesmo botão.
    pub fn alternar_todas(&mut self, total: usize) {
        if total > 0 && (0..total).all(|i| self.marcadas.contains(&i)) {
            for i in 0..total {
                self.marcadas.remove(&i);
            }
        } else {
            self.marcadas.extend(0..total);
        }
    }

    /// Marca uma foto sem passar por gesto nenhum.
    ///
    /// É o que reconstrói a escolha depois de o acervo ser trocado: revalidar a
    /// página no meio de uma correção em lote não pode desfazer o que o
    /// operador já tinha marcado. Quem chama traduz os ids de volta para
    /// posições — e o que sumiu do recorte simplesmente não volta.
    pub fn marcar(&mut self, indice: usize) {
        self.marcadas.insert(indice);
    }

    /// Põe o cursor numa posição, sem mexer no que está marcado.
    pub fn focar(&mut self, indice: Option<usize>) {
        self.foco = indice;
        if self.ancora.is_none() {
            self.ancora = indice;
        }
    }

    /// Chegando pelo Tab: o cursor vai para a primeira, senão não há o que mover.
    pub fn focar_primeira_se_vazio(&mut self, total: usize) {
        if self.foco.is_none() && total > 0 {
            self.foco = Some(0);
            self.ancora = Some(0);
        }
    }

    /// As setas do teclado.
    ///
    /// - sozinha: leva o foco e marca só a foto de destino;
    /// - com Shift: estende da âncora até o destino;
    /// - com Ctrl: **só move o cursor**, sem tocar no que está marcado — é como
    ///   se escolhe fotos salteadas sem mouse.
    ///
    /// Devolve o destino, para quem desenha rolar até ele.
    pub fn mover(
        &mut self,
        direcao: Direcao,
        layout: &Layout,
        modificadores: Modificadores,
    ) -> Option<usize> {
        if layout.total == 0 {
            return None;
        }
        let atual = self.foco.unwrap_or(0).min(layout.total - 1);
        let destino = match self.foco {
            // Sem foco ainda (a grade acabou de receber o teclado): a primeira.
            None => 0,
            Some(_) => layout.mover(atual, direcao),
        };
        self.ir_para(destino, modificadores);
        Some(destino)
    }

    /// PageUp/PageDown: um "tela cheia de linhas" de cada vez.
    pub fn saltar(
        &mut self,
        linhas_por_tela: usize,
        para_baixo: bool,
        layout: &Layout,
        modificadores: Modificadores,
    ) -> Option<usize> {
        if layout.total == 0 {
            return None;
        }
        let salto = (linhas_por_tela.max(1) * layout.colunas) as isize;
        let atual = self.foco.unwrap_or(0) as isize;
        let bruto = atual + if para_baixo { salto } else { -salto };
        let destino = bruto.clamp(0, layout.total as isize - 1) as usize;
        self.ir_para(destino, modificadores);
        Some(destino)
    }

    fn ir_para(&mut self, destino: usize, modificadores: Modificadores) {
        if modificadores.faixa {
            let ancora = self.ancora.unwrap_or(self.foco.unwrap_or(destino));
            self.ancora = Some(ancora);
            self.marcadas = faixa_entre(ancora, destino).collect();
        } else if modificadores.aditivo {
            // Só o cursor anda.
        } else {
            self.marcadas = std::iter::once(destino).collect();
            self.ancora = Some(destino);
        }
        self.foco = Some(destino);
    }

    fn alternar(&mut self, indice: usize) {
        if !self.marcadas.insert(indice) {
            self.marcadas.remove(&indice);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::grade::Opcoes;

    fn marcadas(s: &Selecao) -> Vec<usize> {
        s.marcadas().collect()
    }

    fn layout(total: usize) -> Layout {
        // 1000 px, zoom 220 → 4 colunas.
        Layout::calcular(1000.0, 220.0, total, Opcoes::default())
    }

    const SOZINHO: Modificadores = Modificadores {
        aditivo: false,
        faixa: false,
    };
    const CTRL: Modificadores = Modificadores {
        aditivo: true,
        faixa: false,
    };
    const SHIFT: Modificadores = Modificadores {
        aditivo: false,
        faixa: true,
    };

    #[test]
    fn o_clique_simples_troca_a_selecao_inteira() {
        let mut s = Selecao::nova();
        s.clicar(3, false, SOZINHO);
        s.clicar(7, false, SOZINHO);
        assert_eq!(marcadas(&s), vec![7]);
        assert_eq!(s.foco(), Some(7));
    }

    #[test]
    fn ctrl_acrescenta_e_tira() {
        let mut s = Selecao::nova();
        s.clicar(1, false, SOZINHO);
        s.clicar(4, false, CTRL);
        assert_eq!(marcadas(&s), vec![1, 4]);
        s.clicar(1, false, CTRL);
        assert_eq!(marcadas(&s), vec![4], "o segundo Ctrl+clique desmarca");
    }

    /// A caixinha do canto: alterna sem desfazer o resto, e é isso que a torna
    /// útil para escolher fotos espalhadas sem segurar tecla nenhuma.
    #[test]
    fn a_caixinha_alterna_sem_desfazer_o_resto() {
        let mut s = Selecao::nova();
        s.clicar(1, false, SOZINHO);
        s.clicar(5, true, SOZINHO);
        s.clicar(9, true, SOZINHO);
        assert_eq!(marcadas(&s), vec![1, 5, 9]);
    }

    #[test]
    fn shift_estende_a_partir_da_ancora() {
        let mut s = Selecao::nova();
        s.clicar(2, false, SOZINHO);
        s.clicar(5, false, SHIFT);
        assert_eq!(marcadas(&s), vec![2, 3, 4, 5]);
    }

    #[test]
    fn shift_estende_nas_duas_direcoes() {
        let mut s = Selecao::nova();
        s.clicar(5, false, SOZINHO);
        s.clicar(2, false, SHIFT);
        assert_eq!(marcadas(&s), vec![2, 3, 4, 5]);
    }

    /// 🚨 A âncora **não é o foco**: mover o cursor com Ctrl e depois estender
    /// com Shift tem de estender de onde o operador clicou pela última vez.
    #[test]
    fn a_ancora_nao_anda_com_o_cursor_de_ctrl() {
        let l = layout(20);
        let mut s = Selecao::nova();
        s.clicar(2, false, SOZINHO); // âncora = 2
        s.mover(Direcao::Direita, &l, CTRL); // cursor vai a 3, marcação intacta
        s.mover(Direcao::Direita, &l, CTRL); // cursor vai a 4
        assert_eq!(marcadas(&s), vec![2], "Ctrl move o cursor e mais nada");
        s.mover(Direcao::Direita, &l, SHIFT); // estende de 2 até 5
        assert_eq!(marcadas(&s), vec![2, 3, 4, 5]);
    }

    #[test]
    fn shift_sem_ancora_nao_estende_do_nada() {
        let mut s = Selecao::nova();
        s.clicar(4, false, SHIFT);
        assert_eq!(marcadas(&s), vec![4], "vira um clique comum");
    }

    #[test]
    fn clicar_no_vazio_limpa_menos_com_modificador() {
        let mut s = Selecao::nova();
        s.clicar(1, false, SOZINHO);
        s.clicar_no_vazio(CTRL);
        assert_eq!(
            marcadas(&s),
            vec![1],
            "quem errou o alvo com Ctrl não perde"
        );
        s.clicar_no_vazio(SOZINHO);
        assert!(s.vazia());
    }

    /// Recalcular da base a cada quadro é o que permite **desfazer** voltando
    /// com o ponteiro, em vez de acumular o rastro do arrasto.
    #[test]
    fn o_arrasto_recalcula_da_base_e_nao_acumula() {
        let mut s = Selecao::nova();
        let base = s.instantaneo();
        s.arrastar(&base, &[3, 4, 5]);
        assert_eq!(marcadas(&s), vec![3, 4, 5]);
        s.arrastar(&base, &[3]);
        assert_eq!(
            marcadas(&s),
            vec![3],
            "voltou com o ponteiro, soltou as outras"
        );
    }

    #[test]
    fn o_arrasto_aditivo_soma_a_base() {
        let mut s = Selecao::nova();
        s.clicar(9, false, SOZINHO);
        let base = s.instantaneo();
        s.arrastar(&base, &[1, 2]);
        assert_eq!(marcadas(&s), vec![1, 2, 9]);
    }

    #[test]
    fn a_seta_sozinha_leva_o_foco_e_a_marcacao_junto() {
        let l = layout(20);
        let mut s = Selecao::nova();
        s.clicar(0, false, SOZINHO);
        assert_eq!(s.mover(Direcao::Baixo, &l, SOZINHO), Some(4));
        assert_eq!(marcadas(&s), vec![4]);
        assert_eq!(s.foco(), Some(4));
    }

    #[test]
    fn sem_foco_a_primeira_seta_vai_para_a_primeira_foto() {
        let l = layout(20);
        let mut s = Selecao::nova();
        assert_eq!(s.mover(Direcao::Direita, &l, SOZINHO), Some(0));
    }

    #[test]
    fn o_espaco_alterna_a_foto_em_foco() {
        let l = layout(20);
        let mut s = Selecao::nova();
        s.mover(Direcao::Direita, &l, SOZINHO);
        s.mover(Direcao::Direita, &l, CTRL);
        s.alternar_foco();
        assert_eq!(marcadas(&s), vec![0, 1]);
        s.alternar_foco();
        assert_eq!(marcadas(&s), vec![0]);
    }

    #[test]
    fn a_pagina_salta_uma_tela_de_linhas_e_para_nas_pontas() {
        let l = layout(20);
        let mut s = Selecao::nova();
        s.clicar(0, false, SOZINHO);
        assert_eq!(
            s.saltar(2, true, &l, SOZINHO),
            Some(8),
            "2 linhas × 4 colunas"
        );
        assert_eq!(
            s.saltar(99, true, &l, SOZINHO),
            Some(19),
            "não passa do fim"
        );
        assert_eq!(s.saltar(99, false, &l, SOZINHO), Some(0), "nem do começo");
    }

    #[test]
    fn marcar_todas_nao_mexe_no_foco() {
        let mut s = Selecao::nova();
        s.clicar(3, false, SOZINHO);
        s.marcar_todas(6);
        assert_eq!(marcadas(&s), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(s.foco(), Some(3));
    }

    #[test]
    fn o_botao_das_visiveis_marca_e_desmarca_com_o_mesmo_clique() {
        let mut s = Selecao::nova();
        s.alternar_todas(3);
        assert_eq!(marcadas(&s), vec![0, 1, 2]);
        s.alternar_todas(3);
        assert!(s.vazia());
    }

    #[test]
    fn esc_desmarca_mas_o_cursor_fica() {
        let mut s = Selecao::nova();
        s.clicar(2, false, SOZINHO);
        s.desmarcar();
        assert!(s.vazia());
        assert_eq!(s.foco(), Some(2));
    }

    #[test]
    fn trocar_o_recorte_limpa_tudo_inclusive_o_cursor() {
        let mut s = Selecao::nova();
        s.clicar(2, false, SOZINHO);
        s.limpar_tudo();
        assert!(s.vazia());
        assert_eq!(s.foco(), None);
    }

    #[test]
    fn remarcar_reconstroi_a_escolha_sem_gesto_nenhum() {
        let mut s = Selecao::nova();
        s.marcar(1);
        s.marcar(4);
        s.focar(Some(4));
        assert_eq!(marcadas(&s), vec![1, 4]);
        assert_eq!(s.foco(), Some(4));
    }

    #[test]
    fn o_tab_poe_o_cursor_na_primeira_sem_marcar_nada() {
        let mut s = Selecao::nova();
        s.focar_primeira_se_vazio(10);
        assert_eq!(s.foco(), Some(0));
        assert!(s.vazia(), "chegar pelo Tab não escolhe foto");
    }

    #[test]
    fn grade_vazia_nao_derruba_nada() {
        let l = layout(0);
        let mut s = Selecao::nova();
        assert_eq!(s.mover(Direcao::Baixo, &l, SOZINHO), None);
        assert_eq!(s.saltar(3, true, &l, SOZINHO), None);
        s.focar_primeira_se_vazio(0);
        assert_eq!(s.foco(), None);
    }
}
