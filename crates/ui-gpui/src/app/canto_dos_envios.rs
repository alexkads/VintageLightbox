//! O canto dos envios sai da frente: arrasta-se pela alça, como o caixa.
//!
//! # Por que ele anda
//!
//! O dono, 24/set/2026: *"esse status de 'envios na fila' tinha que conseguir
//! movimentá-lo de lugar, semelhante ao que foi feito o modal do caixa, pois
//! em alguns momentos esse status pode ficar por cima do filmstrip"*. O canto
//! nasce no pé da janela, ao lado do menu — exatamente onde a tira da Revelação
//! e a da sessão desenham as primeiras fotos. Durante uma importação ele fica
//! à vista por minutos, e a foto de baixo dele não se alcança.
//!
//! # O mesmo gesto do caixa
//!
//! A alça (⋮⋮) começa o arrasto, o ponteiro leva, e o lugar em que ele foi
//! largado é lembrado num arquivo ao lado do catálogo — um só para todas as
//! sessões, porque o lugar é da mesa de quem opera, e não do ensaio. Com a
//! janela menor, ele volta para dentro, como o caixa (`dentro_dos_limites`).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

/// A folga entre o canto e a borda da área, dos dois lados.
pub(super) const MARGEM: f32 = 16.;

/// Onde o canto está, e o arrasto em curso.
pub(super) struct CantoDosEnvios {
    /// O deslocamento a partir do canto inferior esquerdo da área (à direita
    /// do menu): `(0, 0)` é o lugar de nascença, positivo leva para a direita e
    /// negativo para cima.
    pub posicao: (f32, f32),
    /// Onde o arrasto começou: o ponteiro e a posição do canto ali.
    arrasto: Option<((f32, f32), (f32, f32))>,
    /// O tamanho do canto no último quadro — o limite do arrasto.
    pub tamanho: Rc<Cell<(f32, f32)>>,
    lembranca: PathBuf,
}

/// O que vai para o disco.
#[derive(Serialize, Deserialize, Default)]
struct Guardado {
    x: f32,
    y: f32,
}

#[cfg(not(test))]
fn caminho_do_guardado() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("canto-dos-envios.json")
}

#[cfg(test)]
fn caminho_do_guardado() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-canto-dos-envios-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Os limites do deslocamento: da nascença até a outra ponta da área, com a
/// folga dos dois lados, sem inverter quando o canto é maior que a área.
pub(super) fn dentro_dos_limites(
    posicao: (f32, f32),
    canto: (f32, f32),
    area: (f32, f32),
) -> (f32, f32) {
    let direita = (area.0 - canto.0 - 2. * MARGEM).max(0.);
    let topo = (-(area.1 - canto.1 - 2. * MARGEM)).min(0.);
    (posicao.0.clamp(0., direita), posicao.1.clamp(topo, 0.))
}

impl CantoDosEnvios {
    /// O canto onde foi largado da última vez — ou na nascença.
    pub fn lembrado() -> Self {
        let lembranca = caminho_do_guardado();
        let guardado: Guardado = std::fs::read_to_string(&lembranca)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            posicao: (guardado.x.max(0.), guardado.y.min(0.)),
            arrasto: None,
            tamanho: Rc::new(Cell::new((0., 0.))),
            lembranca,
        }
    }

    pub fn arrastando(&self) -> bool {
        self.arrasto.is_some()
    }

    /// A alça foi apertada em `ponto`.
    pub fn comecar(&mut self, ponto: (f32, f32)) {
        self.arrasto = Some((ponto, self.posicao));
    }

    /// O ponteiro andou até `ponto`. O `y` da tela cresce para baixo, e o do
    /// deslocamento também: subir o ponteiro leva o canto para cima.
    pub fn arrastar(&mut self, ponto: (f32, f32), area: (f32, f32)) -> bool {
        let Some((inicio, de)) = self.arrasto else {
            return false;
        };
        let nova = (de.0 + ponto.0 - inicio.0, de.1 + ponto.1 - inicio.1);
        self.posicao = dentro_dos_limites(nova, self.tamanho.get(), area);
        true
    }

    /// O botão soltou: o lugar fica lembrado.
    pub fn soltar(&mut self) -> bool {
        if self.arrasto.take().is_none() {
            return false;
        }
        let guardado = Guardado {
            x: self.posicao.0,
            y: self.posicao.1,
        };
        // Não poder lembrar não impede de usar: o canto só nasce no lugar de
        // sempre na próxima abertura.
        if let Ok(texto) = serde_json::to_string(&guardado) {
            if let Err(erro) = std::fs::write(&self.lembranca, texto) {
                eprintln!("⚠️ [Canto dos envios] não deu para lembrar o lugar: {erro}");
            }
        }
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const AREA: (f32, f32) = (1000., 700.);
    const CANTO: (f32, f32) = (200., 40.);

    #[test]
    fn nasce_no_pe_e_nao_passa_para_a_esquerda_nem_para_baixo() {
        assert_eq!(dentro_dos_limites((0., 0.), CANTO, AREA), (0., 0.));
        assert_eq!(
            dentro_dos_limites((-50., 30.), CANTO, AREA),
            (0., 0.),
            "à esquerda está o menu, e abaixo, a borda"
        );
    }

    #[test]
    fn vai_ate_a_outra_ponta_com_a_folga() {
        assert_eq!(
            dentro_dos_limites((5000., -5000.), CANTO, AREA),
            (1000. - 200. - 32., -(700. - 40. - 32.)),
        );
    }

    #[test]
    fn maior_que_a_area_fica_na_nascenca() {
        assert_eq!(
            dentro_dos_limites((10., -10.), (2000., 900.), AREA),
            (0., 0.)
        );
    }

    #[test]
    fn o_arrasto_leva_e_o_lugar_fica_lembrado() {
        let mut canto = CantoDosEnvios::lembrado();
        canto.tamanho.set(CANTO);
        assert!(
            !canto.arrastar((10., 10.), AREA),
            "sem alça apertada, não anda"
        );

        canto.comecar((100., 600.));
        assert!(canto.arrastando());
        assert!(canto.arrastar((400., 300.), AREA));
        assert_eq!(canto.posicao, (300., -300.), "para a direita e para cima");
        assert!(canto.soltar());
        assert!(!canto.arrastando());

        let lembrado = std::fs::read_to_string(&canto.lembranca).expect("gravado");
        let guardado: Guardado = serde_json::from_str(&lembrado).expect("JSON");
        assert_eq!((guardado.x, guardado.y), (300., -300.));
        let _ = std::fs::remove_file(&canto.lembranca);
    }
}
