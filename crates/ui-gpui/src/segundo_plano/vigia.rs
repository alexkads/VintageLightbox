//! Quando o app vai para a bandeja e quando volta — a regra
//! sozinha, sem janela nem ícone, para ser conferida em teste.

use std::time::{Duration, Instant};

/// Depois de trazer a janela de volta, a vigia espera ela terminar de voltar
/// antes de perguntar de novo — senão a animação de desminimizar ainda diz
/// "minimizada" e o app voltaria direto para a bandeja.
pub const ESPERA_AO_VOLTAR: Duration = Duration::from_secs(2);

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
    /// Fechada (e não só minimizada): ela não "volta sozinha".
    escondida: bool,
    restaurada_em: Option<Instant>,
}

impl Vigia {
    pub fn na_bandeja(&self) -> bool {
        self.na_bandeja
    }

    /// O operador fechou a janela principal: **ela vai para a bandeja, sempre.**
    ///
    /// 🔄 **Decisão do dono (2026-09-21)**: *"quero que o sistema fique na
    /// bandeja ao fechar, assim podemos continuar com os processos em segundo
    /// plano"*. Até aqui, fechar sem nada na fila encerrava o app, e fechar com
    /// envio avisava e depois encerrava quando a fila esvaziasse (G9 e o aviso
    /// de 2026-09-20). Agora nada se interrompe ao fechar — e, como nada se
    /// interrompe, não há o que perguntar. Sair de verdade é o "Sair" da
    /// bandeja (ou `⌘Q`).
    pub fn ao_fechar(&mut self) {
        self.na_bandeja = true;
        self.escondida = true;
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

    /// A janela volta ("Abrir", ou o ícone do Dock).
    pub fn restaurar(&mut self, agora: Instant) {
        self.na_bandeja = false;
        self.escondida = false;
        self.restaurada_em = Some(agora);
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

    /// 🔄 **Fechar leva para a bandeja, com ou sem trabalho** (dono,
    /// 2026-09-21): o app não termina ao fechar a janela.
    #[test]
    fn fechar_sempre_leva_para_a_bandeja() {
        let mut v = Vigia::default();
        let t = Instant::now();
        v.ao_fechar();
        assert!(v.na_bandeja());
        // Escondida não é "desminimizada": a vigia não a dá por voltada.
        assert_eq!(v.volta(false, t), Passo::Nada);
        // E abrir de novo a traz de volta.
        v.restaurar(t);
        assert!(!v.na_bandeja());
    }
}
