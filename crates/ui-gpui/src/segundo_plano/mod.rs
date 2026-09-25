//! O trabalho que continua quando ninguém está olhando, e a bandeja que o
//! mostra — o equivalente do service worker da web (dono, 2026-09-17).
//!
//! # O que roda em segundo plano aqui
//!
//! | Atividade | Web | Aqui |
//! |---|---|---|
//! | Subir e tirar fotos do site, salvar revelação | `sw.js` + `envios-pendentes.js` | `Aplicativo::sincronias_pendentes` |
//! | O que o site recusou, com o motivo | descartado | `Aplicativo::recusas` |
//! | Refazer miniaturas | "Em segundo plano N" (fila de fundo) | reposições do disco |
//! | Fotos importadas esperando nota | — | locais da sessão sem nota |
//!
//! # Minimizar e fechar
//!
//! - **Minimizar leva o app para a bandeja**: o ícone aparece na barra de menus
//!   (macOS) ou na área de notificação (Windows e Linux), e no macOS o app sai
//!   do Dock. O trabalho continua: ele nunca dependeu da janela à vista.
//! - **Fechar também leva para a bandeja, sempre** (dono, 2026-09-21: *"quero
//!   que o sistema fique na bandeja ao fechar, assim podemos continuar com os
//!   processos em segundo plano"*). Nada se interrompe e o app não termina
//!   sozinho; sair de verdade é o "Sair" da bandeja (ou `⌘Q`).
//! - **"Abrir o VintageLightbox"** (ou o ícone do Dock) traz a janela de volta.
//!
//! 🔑 **A regra mora aqui, sem GPUI**: `frases` diz o que cada linha da bandeja
//! mostra, e `vigia` decide quando ir para a bandeja, voltar e sair. O laço
//! (`laco`) só junta as duas coisas com a janela e o ícone de verdade.

pub mod frases;
pub mod janela;
mod laco;
pub mod vigia;

#[cfg(test)]
pub(crate) use laco::estado_para_teste;
pub use laco::{
    ao_reabrir, fechar_pelo_botao, gesto_de_roteiro, ligar, sair_agora,
    sair_quando_a_fila_esvaziar, trazer_para_a_frente,
};

/// Liga o [`ao_reabrir`] na aplicação, sem quebrar a corrente de `main.rs`.
pub trait ReabrirDaBandeja {
    fn reabrir_da_bandeja(self) -> Self;
}

impl ReabrirDaBandeja for gpui::Application {
    fn reabrir_da_bandeja(self) -> Self {
        self.on_reopen(ao_reabrir);
        self
    }
}
