//! ✋ **O gesto dado na foto que ainda sobe, guardado na hora** (#44, C20).
//!
//! O `5` e o `B` na foto local vão ao catálogo por uma tarefa da Biblioteca, e
//! a grade só os vê na releitura seguinte. A conciliação
//! (`Aplicativo::conciliar_o_que_subiu`) roda na releitura em que a foto ganha
//! o id do site, e essa releitura pode ter sido lida **antes** de a gravação do
//! gesto terminar. Ela via "nada mudou", apagava a pendência, e o gesto que
//! chegava ao catálogo logo depois não ia mais ao site: o cupom do caixa
//! cobrava menos fotos do que o cliente levava (e2e
//! `vender_com_as_fotos_ainda_subindo`, 3 de 3 na fedora em 19/11/2026).
//!
//! 🔑 Por isso o gesto é guardado aqui no instante da tecla, e na conciliação
//! ele vence a leitura do catálogo. O catálogo continua sendo a verdade para o
//! resto: este registro só cobre a janela entre a tecla e a releitura.

use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Gesto {
    /// `Some(None)` = a nota foi tirada (`0`).
    nota: Option<Option<u8>>,
    levada: Option<bool>,
}

#[derive(Debug, Default)]
pub(super) struct GestosNaSubida(HashMap<String, Gesto>);

impl GestosNaSubida {
    /// A tecla `B` (ou a que desfaz) numa foto que está subindo.
    pub(super) fn levar(&mut self, id: &str, levada: bool) {
        self.0.entry(id.to_string()).or_default().levada = Some(levada);
    }

    /// A nota (`0` tira) numa foto que está subindo.
    pub(super) fn classificar(&mut self, id: &str, nota: i32) {
        let nota = u8::try_from(nota).ok().filter(|n| (1..=5).contains(n));
        self.0.entry(id.to_string()).or_default().nota = Some(nota);
    }

    /// A foto saiu da subida sem conciliar (excluída, rejeitada).
    pub(super) fn esquecer(&mut self, id: &str) {
        self.0.remove(id);
    }

    /// A nota e a levada que valem na conciliação: o gesto guardado, se
    /// houver, e senão o que a releitura leu. Consome o registro da foto.
    ///
    /// A levada sem nota continua subindo à venda: o servidor recusaria a
    /// foto inteira.
    pub(super) fn conciliar(
        &mut self,
        id: &str,
        nota_lida: Option<u8>,
        comprada_lida: bool,
    ) -> (Option<u8>, bool) {
        let gesto = self.0.remove(id).unwrap_or_default();
        let nota = gesto.nota.unwrap_or(nota_lida);
        let comprada = gesto.levada.unwrap_or(comprada_lida);
        (nota, comprada && nota.is_some())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_releitura_atrasada_nao_apaga_o_b_dado_na_subida() {
        let mut g = GestosNaSubida::default();
        g.classificar("f3", 5);
        g.levar("f3", true);
        // A releitura foi lida antes de o catálogo gravar os dois gestos.
        assert_eq!(g.conciliar("f3", None, false), (Some(5), true));
    }

    #[test]
    fn sem_gesto_vale_o_que_a_releitura_leu() {
        let mut g = GestosNaSubida::default();
        assert_eq!(g.conciliar("f1", Some(4), true), (Some(4), true));
        assert_eq!(g.conciliar("f2", None, true), (None, false));
    }

    #[test]
    fn o_b_sem_nota_sobe_a_venda() {
        let mut g = GestosNaSubida::default();
        g.levar("f1", true);
        assert_eq!(g.conciliar("f1", None, false), (None, false));
    }

    #[test]
    fn o_zero_tira_a_nota_e_com_ela_a_levada() {
        let mut g = GestosNaSubida::default();
        g.classificar("f1", 0);
        assert_eq!(g.conciliar("f1", Some(5), true), (None, false));
    }

    #[test]
    fn o_ultimo_gesto_vence_e_a_conciliacao_consome_o_registro() {
        let mut g = GestosNaSubida::default();
        g.levar("f1", true);
        g.levar("f1", false);
        g.classificar("f1", 3);
        assert_eq!(g.conciliar("f1", None, true), (Some(3), false));
        assert_eq!(g.conciliar("f1", None, false), (None, false));
    }

    #[test]
    fn a_foto_esquecida_nao_leva_gesto_nenhum() {
        let mut g = GestosNaSubida::default();
        g.levar("f1", true);
        g.classificar("f1", 5);
        g.esquecer("f1");
        assert_eq!(g.conciliar("f1", None, false), (None, false));
    }
}
