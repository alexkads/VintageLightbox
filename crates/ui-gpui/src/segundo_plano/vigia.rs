//! Quando o app vai para a bandeja, quando volta e quando termina — a regra
//! sozinha, sem janela nem ícone, para ser conferida em teste.

use std::time::{Duration, Instant};

/// Depois de trazer a janela de volta, a vigia espera ela terminar de voltar
/// antes de perguntar de novo — senão a animação de desminimizar ainda diz
/// "minimizada" e o app voltaria direto para a bandeja.
pub const ESPERA_AO_VOLTAR: Duration = Duration::from_secs(2);

/// O que fazer com o pedido de fechar a janela principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AoFechar {
    /// Nada pendente: fecha, e o app termina (`encerramento`).
    Fechar,
    /// Envio na fila (G9): a janela some, a bandeja fica, e o app termina
    /// quando a fila esvaziar.
    Esconder,
}

/// O que uma volta do laço pede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passo {
    Nada,
    /// A janela foi minimizada: mostrar a bandeja (e, no macOS, sair do Dock).
    ParaABandeja,
    /// A janela voltou por outro caminho (a barra de tarefas do Windows): a
    /// bandeja some, como no "Abrir".
    Voltou,
}

#[derive(Debug, Default)]
pub struct Vigia {
    na_bandeja: bool,
    /// Fechada com envio (G9), e não só minimizada: ela não "volta sozinha".
    escondida: bool,
    fechar_ao_esvaziar: bool,
    restaurada_em: Option<Instant>,
}

impl Vigia {
    pub fn na_bandeja(&self) -> bool {
        self.na_bandeja
    }

    /// O operador pediu para fechar a janela principal.
    pub fn ao_fechar(&mut self, ha_envio_pendente: bool) -> AoFechar {
        if !ha_envio_pendente {
            return AoFechar::Fechar;
        }
        self.na_bandeja = true;
        self.escondida = true;
        self.fechar_ao_esvaziar = true;
        AoFechar::Esconder
    }

    /// Uma volta do laço, com o que a janela diz agora.
    pub fn volta(&mut self, minimizada: bool, agora: Instant) -> Passo {
        let voltando = self
            .restaurada_em
            .is_some_and(|quando| agora.duration_since(quando) < ESPERA_AO_VOLTAR);
        if voltando {
            return Passo::Nada;
        }
        match (self.na_bandeja, minimizada) {
            (false, true) => {
                self.na_bandeja = true;
                Passo::ParaABandeja
            }
            (true, false) if !self.escondida => {
                self.restaurar(agora);
                Passo::Voltou
            }
            _ => Passo::Nada,
        }
    }

    /// A janela volta ("Abrir", ou o ícone do Dock). Quem a trouxe está de
    /// volta: o app deixa de terminar sozinho quando a fila esvaziar.
    pub fn restaurar(&mut self, agora: Instant) {
        self.na_bandeja = false;
        self.escondida = false;
        self.fechar_ao_esvaziar = false;
        self.restaurada_em = Some(agora);
    }

    /// A janela foi fechada com envio, e a fila acabou de esvaziar.
    pub fn deve_sair(&self, ha_envio_pendente: bool) -> bool {
        self.fechar_ao_esvaziar && !ha_envio_pendente
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn minimizar_leva_para_a_bandeja_uma_vez_so() {
        let mut v = Vigia::default();
        let t = Instant::now();
        assert_eq!(v.volta(false, t), Passo::Nada);
        assert_eq!(v.volta(true, t), Passo::ParaABandeja);
        assert!(v.na_bandeja());
        assert_eq!(v.volta(true, t), Passo::Nada, "já está na bandeja");
        assert!(!v.deve_sair(false), "minimizar não encerra o app");
    }

    #[test]
    fn abrir_tira_a_bandeja_e_espera_a_janela_terminar_de_voltar() {
        let mut v = Vigia::default();
        let t = Instant::now();
        v.volta(true, t);
        v.restaurar(t);
        assert!(!v.na_bandeja());
        // A animação ainda diz "minimizada": não volta para a bandeja.
        assert_eq!(v.volta(true, t + Duration::from_millis(700)), Passo::Nada);
        // Passada a espera, minimizar de novo vale de novo.
        assert_eq!(
            v.volta(true, t + ESPERA_AO_VOLTAR + Duration::from_millis(1)),
            Passo::ParaABandeja
        );
    }

    #[test]
    fn a_janela_que_volta_pela_barra_de_tarefas_tira_a_bandeja() {
        let mut v = Vigia::default();
        let t = Instant::now();
        v.volta(true, t);
        assert_eq!(v.volta(false, t), Passo::Voltou);
        assert!(!v.na_bandeja());
    }

    #[test]
    fn fechar_sem_envio_fecha() {
        let mut v = Vigia::default();
        assert_eq!(v.ao_fechar(false), AoFechar::Fechar);
        assert!(!v.na_bandeja());
        assert!(!v.deve_sair(false));
    }

    #[test]
    fn fechar_com_envio_esconde_e_sai_quando_a_fila_esvazia() {
        let mut v = Vigia::default();
        let t = Instant::now();
        assert_eq!(v.ao_fechar(true), AoFechar::Esconder);
        assert!(v.na_bandeja());
        // Escondida não é "desminimizada": a vigia não a dá por voltada.
        assert_eq!(v.volta(false, t), Passo::Nada);
        assert!(!v.deve_sair(true), "ainda subindo");
        assert!(v.deve_sair(false), "a fila esvaziou");
    }

    #[test]
    fn abrir_de_novo_antes_de_esvaziar_mantem_o_app_aberto() {
        let mut v = Vigia::default();
        v.ao_fechar(true);
        v.restaurar(Instant::now());
        assert!(!v.deve_sair(false), "quem fechou está de volta");
    }
}
