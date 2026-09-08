//! Desfazer e refazer: a pilha de estados por que a revelação passou.
//!
//! Guarda [`Ajustes`] inteiros, e não "o que mudou". São 46 `f32` — 184 bytes
//! por passo, 3,6 KB no histórico cheio. Guardar diferença economizaria isso e
//! custaria o problema de verdade: aplicar diferença ao contrário, na ordem
//! certa, sem acumular erro de ponto flutuante.
//!
//! ## 🚨 Duas diferenças de propósito em relação ao `crates/ui`
//!
//! **1. Um passo por gesto, e não por quadro.** O legado empurra um snapshot a
//! cada quadro em que algum valor difere do anterior (`app.rs`, dentro do
//! `update`), o que faz um arrasto de meio segundo virar ~30 passos. Com o teto
//! de 20, o `Cmd+Z` de lá desfaz um milímetro por vez e o resto do histórico já
//! foi embora. Pior: **o número de passos depende da taxa de quadros** — a 120fps
//! ele grava o dobro. Comportamento que muda com o monitor não é paridade
//! conferível; é o mesmo recurso em duas máquinas diferentes.
//!
//! Aqui o passo é registrado no **fim do gesto**, o mesmo instante em que a
//! gravação acontece (`tela.rs`). Um arrasto = um `Cmd+Z`.
//!
//! **2. A primeira edição é desfazível.** No legado, `push_edit_snapshot` só
//! roda quando algo mudou, então o primeiro snapshot já é o estado **depois** da
//! mudança — e `undo` faz `if index > 0`, ou seja, não faz nada. A primeira coisa
//! que se faz numa foto lá **não tem volta**. Aqui o estado da abertura entra no
//! histórico como passo zero, e o primeiro `Cmd+Z` devolve a foto ao que estava
//! gravado.
//!
//! **3. O passo é a revelação inteira, e não só os sliders.** Foi o item 12 da
//! fila, e a ideia veio do darktable: lá a pilha de histórico é a lista de
//! *módulos aplicados*, e o corte é um módulo como qualquer outro — não há o que
//! esquecer, porque nada tem lugar privilegiado. Aqui o equivalente é
//! [`Estado`]: os 46 ajustes **e** os oito campos do corte num tipo só.
//!
//! 🚨 Até 6/set/2026 a pilha guardava só `Ajustes`, e `Cmd+Z` depois de cortar
//! voltava tudo menos o enquadramento. O legado erra a mesma coisa de um jeito
//! pior: o `EditSnapshot` de lá **tem** o campo `crop_settings`, guarda o corte
//! nele, e nem `undo` nem `redo` o leem de volta — quem lê o struct conclui que
//! funciona.
//!
//! 🔑 **E o formato é o que segura o próximo**: quando existirem ajustes locais
//! ou máscaras, eles entram no [`Estado`] e o desfazer os alcança sem que ninguém
//! precise lembrar de mexer aqui.

use super::persistencia::Corte;
use super::processador::Ajustes;

/// Como a foto está revelada, inteira — é o que um passo do histórico guarda.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Estado {
    pub ajustes: Ajustes,
    pub corte: Corte,
}

/// Quantos passos cabem.
///
/// Os mesmos 20 do legado. Lá são 20 quadros de arrasto; aqui, 20 gestos — a
/// mesma constante comprando bem mais.
const TETO: usize = 20;

pub struct Historico {
    /// Os estados, do mais antigo ao mais novo. Nunca vazio: nasce com o estado
    /// da abertura.
    passos: Vec<Estado>,
    /// Onde estamos. Desfazer anda para trás, refazer para a frente.
    atual: usize,
}

impl Historico {
    /// Começa no estado em que a foto abriu — o que está gravado no banco.
    pub fn novo(inicial: Estado) -> Self {
        Self {
            passos: vec![inicial],
            atual: 0,
        }
    }

    /// Registra um estado novo, se ele for diferente do atual.
    ///
    /// ⚠️ **Registrar o que não mudou encheria o histórico de passos idênticos**,
    /// e o `Cmd+Z` pareceria não fazer nada por várias teclas seguidas. Acontece
    /// de verdade: soltar o slider exatamente onde ele estava é um gesto completo,
    /// com fim de gesto e tudo.
    pub fn registrar(&mut self, estado: Estado) {
        if self.passos[self.atual] == estado {
            return;
        }

        // O futuro morre aqui: desfazer três vezes e mexer num slider apaga o que
        // havia para refazer. É o que todo editor faz, e o que o legado também
        // faz (`truncate(index + 1)`).
        self.passos.truncate(self.atual + 1);
        self.passos.push(estado);

        if self.passos.len() > TETO {
            // O mais antigo sai. `remove(0)` num Vec de 20 é cópia de 19
            // elementos — irrelevante num gesto humano, e mais simples de ler do
            // que um anel.
            self.passos.remove(0);
        }

        self.atual = self.passos.len() - 1;
    }

    pub fn pode_desfazer(&self) -> bool {
        self.atual > 0
    }

    pub fn pode_refazer(&self) -> bool {
        self.atual + 1 < self.passos.len()
    }

    /// Volta um passo. `None` quando já está no começo.
    pub fn desfazer(&mut self) -> Option<Estado> {
        if !self.pode_desfazer() {
            return None;
        }
        self.atual -= 1;
        Some(self.passos[self.atual])
    }

    /// Avança um passo. `None` quando já está no fim.
    pub fn refazer(&mut self) -> Option<Estado> {
        if !self.pode_refazer() {
            return None;
        }
        self.atual += 1;
        Some(self.passos[self.atual])
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_exposicao(valor: f32) -> Estado {
        Estado {
            ajustes: Ajustes {
                exposure: valor,
                ..Default::default()
            },
            corte: Corte::default(),
        }
    }

    fn cortada(largura: f32) -> Estado {
        Estado {
            corte: Corte {
                largura: Some(largura),
                ..Corte::default()
            },
            ..Estado::default()
        }
    }

    /// 🚨 A primeira edição volta — é a diferença 2 do topo do arquivo.
    ///
    /// No legado este teste falharia: lá o histórico começa **depois** da
    /// primeira mudança, e `undo` com índice 0 devolve `false`. A primeira coisa
    /// que se faz numa foto não tem volta.
    #[test]
    fn a_primeira_edicao_e_desfazivel() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        assert!(
            !historico.pode_desfazer(),
            "sem edição não há o que desfazer"
        );

        historico.registrar(com_exposicao(1.0));

        assert_eq!(historico.desfazer(), Some(com_exposicao(0.0)));
        assert!(!historico.pode_desfazer());
    }

    #[test]
    fn desfazer_e_refazer_andam_nos_dois_sentidos() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));

        assert_eq!(historico.desfazer(), Some(com_exposicao(1.0)));
        assert_eq!(historico.desfazer(), Some(com_exposicao(0.0)));
        assert_eq!(historico.desfazer(), None, "chegou ao começo");

        assert_eq!(historico.refazer(), Some(com_exposicao(1.0)));
        assert_eq!(historico.refazer(), Some(com_exposicao(2.0)));
        assert_eq!(historico.refazer(), None, "chegou ao fim");
    }

    /// 🚨 Editar depois de desfazer apaga o que havia para refazer.
    ///
    /// Sem o corte, o `Cmd+Shift+Z` levaria a um estado que não é continuação
    /// nenhuma do que está na tela — um ramo abandonado do histórico,
    /// apresentado como se fosse o próximo passo.
    #[test]
    fn editar_depois_de_desfazer_mata_o_futuro() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));

        historico.desfazer();
        assert!(historico.pode_refazer());

        historico.registrar(com_exposicao(5.0));

        assert!(!historico.pode_refazer(), "o 2.0 não é mais alcançável");
        assert_eq!(historico.desfazer(), Some(com_exposicao(1.0)));
    }

    /// ⚠️ Gesto que não muda nada não vira passo.
    ///
    /// Soltar o slider onde ele já estava é um gesto completo — com fim de gesto,
    /// gravação e tudo. Se ele virasse passo, o `Cmd+Z` pareceria não fazer nada
    /// por várias teclas seguidas.
    #[test]
    fn gesto_que_nao_muda_nada_nao_vira_passo() {
        let mut historico = Historico::novo(com_exposicao(0.0));

        historico.registrar(com_exposicao(0.0));
        assert!(!historico.pode_desfazer());

        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(1.0));
        historico.desfazer();
        assert_eq!(
            historico.desfazer(),
            None,
            "os dois 1.0 viraram um passo só"
        );
    }

    /// O teto descarta o mais antigo, e o presente continua sendo o presente.
    ///
    /// 🔑 O erro fácil aqui é esquecer de corrigir o índice ao remover da frente:
    /// `atual` continuaria apontando uma posição além do fim, e o próximo
    /// `desfazer` devolveria o passo errado — ou entraria em pânico.
    #[test]
    fn o_teto_descarta_o_mais_antigo() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        for i in 1..=30 {
            historico.registrar(com_exposicao(i as f32));
        }

        assert_eq!(historico.passos.len(), TETO);
        assert_eq!(
            historico.passos[historico.atual],
            com_exposicao(30.0),
            "o presente é o último gesto"
        );

        // Desfaz até o fundo: sobram 19 passos para trás, e não 30.
        let mut voltas = 0;
        while historico.desfazer().is_some() {
            voltas += 1;
        }
        assert_eq!(voltas, TETO - 1);
        assert_eq!(historico.passos[historico.atual], com_exposicao(11.0));
    }

    /// O histórico guarda os 46 campos, e não só o que a tela mostra.
    #[test]
    fn o_passo_guarda_os_ajustes_inteiros() {
        let cheio = Estado {
            ajustes: Ajustes {
                exposure: 1.0,
                hsl_blue_lum: -40.0,
                sharpen_amount: 60.0,
                ..Default::default()
            },
            corte: Corte::default(),
        };

        let mut historico = Historico::novo(Estado::default());
        historico.registrar(cheio);
        historico.registrar(Estado::default());

        assert_eq!(historico.desfazer(), Some(cheio));
    }

    /// 🚨 O corte é passo de histórico como qualquer outro — o item 12.
    ///
    /// Antes de 6/set/2026 a pilha era de `Ajustes`, e cortar não deixava
    /// marca nenhuma nela: `Cmd+Z` depois de cortar voltava a exposição e
    /// mantinha o enquadramento novo, como se cortar não fosse uma edição.
    #[test]
    fn cortar_e_um_passo_que_o_desfazer_alcanca() {
        let mut historico = Historico::novo(Estado::default());

        historico.registrar(cortada(0.5));
        assert!(historico.pode_desfazer());

        assert_eq!(
            historico.desfazer(),
            Some(Estado::default()),
            "desfazer devolve a foto inteira"
        );
        assert_eq!(historico.refazer(), Some(cortada(0.5)));
    }

    /// Dois cortes seguidos são dois passos, e não um.
    #[test]
    fn cada_corte_e_o_seu_proprio_passo() {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar(cortada(0.8));
        historico.registrar(cortada(0.5));

        assert_eq!(historico.desfazer(), Some(cortada(0.8)));
        assert_eq!(historico.desfazer(), Some(Estado::default()));
    }

    /// ⚠️ Reenquadrar para o mesmo lugar não vira passo — a mesma regra do
    /// slider solto onde já estava.
    #[test]
    fn corte_igual_ao_atual_nao_vira_passo() {
        let mut historico = Historico::novo(cortada(0.5));
        historico.registrar(cortada(0.5));
        assert!(!historico.pode_desfazer());
    }
}
