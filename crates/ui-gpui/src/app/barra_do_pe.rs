//! A barra fina no pé da janela enquanto a importação anda.
//!
//! # Por que ela é assim, e não um aviso
//!
//! O operador importa e **continua trabalhando**: classifica, sinaliza e revela
//! com o cliente olhando a foto no segundo monitor (dono, 2026-09-24 — *"nunca
//! atrapalhe o usuário"*). Então ela:
//!
//! - não pega clique nem foco, e não tem texto: três pixels no pé da janela não
//!   tapam a tira nem um botão;
//! - aparece em **toda** tela da janela principal, inclusive a Revelação, que
//!   não tem menu nem cabeçalho — o canto dos envios some lá;
//! - **nunca vai à tela do cliente**: aquela é outra janela, com o `Cliente`
//!   como raiz, e nada que a raiz daqui desenha chega lá.
//!
//! É a mesma barra do site (`importacao/barra-da-leva.tsx`), e o detalhe
//! ("importando 12 de 30", "N envios na fila") continua onde já estava.
//!
//! # A leva é cópia **e** subida
//!
//! No app a importação tem duas metades: copiar para o disco e subir para o
//! site (C20 — o ensaio inteiro sobe em segundo plano). Uma barra que medisse
//! só a cópia chegaria ao fim com nada no site; uma que medisse as duas em
//! sequência voltaria para trás no instante em que a cópia termina e os
//! envios entram. Então o total é `cópia + subida`, com a subida contada como
//! **pelo menos** o tamanho da cópia — cada foto copiada vai subir.

use std::time::{Duration, Instant};

use crate::envios::Progresso;

/// Quanto a barra cheia fica à vista depois de a leva acabar.
pub const FIM_A_VISTA: Duration = Duration::from_millis(2_500);

/// O que desenhar neste quadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quadro {
    /// De 0 a 1.
    pub fracao: f32,
    /// Nada andando: a barra fica cheia um instante e some.
    pub terminou: bool,
    /// Alguma foto falhou na leva — o fim fica vermelho, e não verde.
    pub houve_falha: bool,
}

/// A leva em curso, lembrada entre um quadro e outro.
///
/// 🔑 **Lembrar é o que a torna uma leva.** A cópia da sessão some quando
/// termina e a esteira zera quando esvazia; sem guardar o último número de
/// cada uma, a barra pularia de volta ao zero entre as duas metades.
#[derive(Debug, Default)]
pub struct BarraDoPe {
    /// `(total, prontas)` da cópia.
    copia: (usize, usize),
    /// `(total, respondidos)` da subida.
    subida: (usize, usize),
    houve_falha: bool,
    cheia_desde: Option<Instant>,
}

impl BarraDoPe {
    /// Alimenta com o que anda agora e devolve o que desenhar (`None`: nada).
    ///
    /// `copia` é `(total, prontas)` das cópias em curso, ou `None` se nenhuma
    /// anda. O segundo valor devolvido é `true` no quadro em que a leva acabou:
    /// quem desenha agenda um redesenho para depois de [`FIM_A_VISTA`].
    pub fn quadro(
        &mut self,
        copia: Option<(usize, usize)>,
        subida: Progresso,
        agora: Instant,
    ) -> (Option<Quadro>, bool) {
        match copia {
            Some(c) => self.copia = c,
            // 🔑 **A cópia que sumiu terminou.** O último quadro dela pode ter
            // sido 29/30 — a resposta final e o fim do lote chegam juntos —, e
            // lembrar esse número deixaria a barra devendo uma foto para sempre.
            None => self.copia.1 = self.copia.0,
        }
        if subida.total > 0 {
            self.subida = (subida.total, subida.respondidos);
            self.houve_falha |= subida.houve_falha;
        }

        let andando = copia.is_some_and(|(t, p)| p < t) || subida.andando();
        if self.copia == (0, 0) && self.subida == (0, 0) {
            return (None, false);
        }
        if andando {
            self.cheia_desde = None;
            let (ct, cp) = self.copia;
            let (st, sr) = self.subida;
            let total = ct + st.max(ct);
            let feitas = (cp + sr).min(total);
            let fracao = if total == 0 {
                0.
            } else {
                feitas as f32 / total as f32
            };
            let quadro = Quadro {
                fracao,
                terminou: false,
                houve_falha: self.houve_falha,
            };
            return (Some(quadro), false);
        }

        let acabou_agora = self.cheia_desde.is_none();
        let desde = *self.cheia_desde.get_or_insert(agora);
        if agora.duration_since(desde) >= FIM_A_VISTA {
            *self = Self::default();
            return (None, false);
        }
        let quadro = Quadro {
            fracao: 1.,
            terminou: true,
            houve_falha: self.houve_falha,
        };
        (Some(quadro), acabou_agora)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn subida(total: usize, respondidos: usize) -> Progresso {
        Progresso {
            total,
            respondidos,
            no_ar: 0,
            houve_falha: false,
        }
    }

    #[test]
    fn sem_leva_nao_ha_barra() {
        let mut b = BarraDoPe::default();
        assert_eq!(
            b.quadro(None, Progresso::default(), Instant::now()),
            (None, false)
        );
    }

    #[test]
    fn a_barra_nao_volta_quando_a_copia_termina_e_os_envios_entram() {
        let mut b = BarraDoPe::default();
        let t = Instant::now();
        // 30 fotos: metade copiada, nada no site ainda.
        let (q, _) = b.quadro(Some((30, 15)), Progresso::default(), t);
        let meio_da_copia = q.unwrap().fracao;
        assert!((meio_da_copia - 0.25).abs() < 1e-6);
        // A cópia terminou e sumiu; os 30 envios entraram na esteira.
        let (q, _) = b.quadro(None, subida(30, 0), t);
        let fim_da_copia = q.unwrap().fracao;
        assert!(fim_da_copia >= meio_da_copia, "a barra voltou para trás");
        assert!((fim_da_copia - 0.5).abs() < 1e-6);
        let (q, _) = b.quadro(None, subida(30, 15), t);
        assert!((q.unwrap().fracao - 0.75).abs() < 1e-6);
    }

    #[test]
    fn o_fim_fica_cheio_um_instante_e_some() {
        let mut b = BarraDoPe::default();
        let t = Instant::now();
        b.quadro(None, subida(3, 1), t);
        // A esteira esvaziou e zerou.
        let (q, acabou) = b.quadro(None, Progresso::default(), t);
        assert!(acabou, "o quadro do fim pede o redesenho");
        assert_eq!(q.unwrap().fracao, 1.);
        assert!(q.unwrap().terminou);
        let (_, de_novo) = b.quadro(None, Progresso::default(), t);
        assert!(!de_novo, "o redesenho é pedido uma vez só");
        assert_eq!(
            b.quadro(None, Progresso::default(), t + FIM_A_VISTA),
            (None, false)
        );
    }

    #[test]
    fn a_falha_da_leva_chega_ao_fim() {
        let mut b = BarraDoPe::default();
        let t = Instant::now();
        let falhou = Progresso {
            houve_falha: true,
            ..subida(2, 1)
        };
        b.quadro(None, falhou, t);
        let (q, _) = b.quadro(None, Progresso::default(), t);
        assert!(q.unwrap().houve_falha);
    }

    #[test]
    fn copia_parada_em_ponto_morto_nao_conta_como_andando() {
        // Um lote de cópia com todas prontas mas ainda reportado: terminou.
        let mut b = BarraDoPe::default();
        let (q, _) = b.quadro(Some((5, 5)), Progresso::default(), Instant::now());
        assert!(q.unwrap().terminou);
    }
}
