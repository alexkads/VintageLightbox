//! A perspectiva guiada como a foto a guarda: as guias que o operador traçou e
//! a correção que elas deram.
//!
//! A conta mora em `revelacao-core::perspectiva` (o domínio não conhece o
//! motor); aqui estão os **parâmetros** — no sentido do contrato da foto, a
//! fonte de verdade — e o jeito de levá-los ao JSON da revelação e trazer de
//! volta.
//!
//! ## 🔑 Guardar a correção, e não só as guias
//!
//! O que se aplica na foto é `rotacao` + `foco` + o ajuste manual. As guias
//! ficam para o operador **reabrir e continuar** de onde parou. Guardar os dois
//! custa uns números a mais e dá uma garantia: a foto revelada hoje sai igual
//! daqui a um ano, mesmo que o resolvedor mude — o site, a exportação e a tela
//! do cliente aplicam a correção sem resolver nada.
//!
//! ## No JSON: achatado, só números, e só quando existe
//!
//! A API guarda a revelação como objeto plano de números (`conferir_forma` no
//! backend: até 384 chaves e 16 KiB). As chaves vão com o prefixo `corte_`,
//! como o resto do enquadramento, e **só aparecem quando há o que dizer**: uma
//! foto sem perspectiva tem a mesma revelação de antes, byte a byte — senão toda
//! foto já enviada pareceria "mudada" e subiria de novo.

use serde_json::{Map, Value};

/// Quantas guias a foto guarda — as quatro do Lightroom.
pub const MAXIMO_DE_GUIAS: usize = 4;
/// O foco padrão do motor (`revelacao_core::perspectiva::F0`), em meias
/// diagonais. Repetido aqui porque o domínio não depende do motor; um teste do
/// `infrastructure` prende os dois.
pub const FOCO_PADRAO: f32 = 1.6;

/// Em que eixo a guia deve ficar — **nos eixos da foto de pé**, antes do giro
/// de 90°. A tela mostra o rótulo trocado quando o giro é ímpar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EixoDaGuia {
    #[default]
    Vertical,
    Horizontal,
}

/// Uma reta de referência, normalizada (0–1) na foto de pé — o espaço das
/// máscaras: girar ou espelhar depois não a tira de cima do batente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuiaDePerspectiva {
    pub de: [f32; 2],
    pub ate: [f32; 2],
    pub eixo: EixoDaGuia,
}

/// As guias e a correção.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerspectivaGuiada {
    pub guias: [Option<GuiaDePerspectiva>; MAXIMO_DE_GUIAS],
    /// O que as guias resolveram: vetor de rotação em graus, na foto de pé.
    pub rotacao: [f32; 3],
    pub foco: f32,
    /// O ajuste fino à mão, em graus, nos eixos da tela.
    pub vertical: f32,
    pub horizontal: f32,
}

impl Default for PerspectivaGuiada {
    fn default() -> Self {
        Self {
            guias: [None; MAXIMO_DE_GUIAS],
            rotacao: [0.0; 3],
            foco: FOCO_PADRAO,
            vertical: 0.0,
            horizontal: 0.0,
        }
    }
}

impl PerspectivaGuiada {
    /// A correção não muda a foto (guias podem existir — uma só não corrige).
    pub fn corrige(&self) -> bool {
        self.rotacao.iter().any(|v| v.abs() >= 1e-6)
            || self.vertical.abs() >= 1e-6
            || self.horizontal.abs() >= 1e-6
    }

    /// Nada a guardar: sem guia e sem correção.
    pub fn e_neutra(&self) -> bool {
        !self.corrige() && self.guias.iter().all(Option::is_none)
    }

    pub fn quantas_guias(&self) -> usize {
        self.guias.iter().flatten().count()
    }

    /// As chaves `corte_persp_*` e `corte_guiaN_*` — só as que existem.
    pub fn em_json(&self, objeto: &mut Map<String, Value>) {
        if self.corrige() {
            let [rx, ry, rz] = self.rotacao;
            for (chave, valor) in [
                ("corte_persp_rx", rx),
                ("corte_persp_ry", ry),
                ("corte_persp_rz", rz),
                ("corte_persp_foco", self.foco),
                ("corte_persp_vertical", self.vertical),
                ("corte_persp_horizontal", self.horizontal),
            ] {
                objeto.insert(chave.into(), valor.into());
            }
        }
        for (i, guia) in self.guias.iter().enumerate() {
            let Some(g) = guia else { continue };
            let n = i + 1;
            for (sufixo, valor) in [
                ("x1", g.de[0]),
                ("y1", g.de[1]),
                ("x2", g.ate[0]),
                ("y2", g.ate[1]),
            ] {
                objeto.insert(format!("corte_guia{n}_{sufixo}"), valor.into());
            }
            let eixo = match g.eixo {
                EixoDaGuia::Vertical => 0,
                EixoDaGuia::Horizontal => 1,
            };
            objeto.insert(format!("corte_guia{n}_eixo"), eixo.into());
        }
    }

    /// O caminho de volta. Ausente é o neutro; número que não é finito também
    /// (um `NaN` apagaria a foto). Guia incompleta é descartada inteira.
    pub fn de_json(json: &Value) -> Self {
        let numero = |chave: &str| {
            json.get(chave)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .map(|v| v as f32)
        };
        let mut p = Self {
            rotacao: [
                numero("corte_persp_rx").unwrap_or(0.0),
                numero("corte_persp_ry").unwrap_or(0.0),
                numero("corte_persp_rz").unwrap_or(0.0),
            ],
            foco: numero("corte_persp_foco")
                .filter(|f| *f > 0.0)
                .unwrap_or(FOCO_PADRAO),
            vertical: numero("corte_persp_vertical").unwrap_or(0.0),
            horizontal: numero("corte_persp_horizontal").unwrap_or(0.0),
            ..Self::default()
        };
        for (i, guia) in p.guias.iter_mut().enumerate() {
            let n = i + 1;
            let campo = |s: &str| numero(&format!("corte_guia{n}_{s}"));
            if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(eixo)) = (
                campo("x1"),
                campo("y1"),
                campo("x2"),
                campo("y2"),
                campo("eixo"),
            ) {
                *guia = Some(GuiaDePerspectiva {
                    de: [x1, y1],
                    ate: [x2, y2],
                    eixo: if eixo.round() == 1.0 {
                        EixoDaGuia::Horizontal
                    } else {
                        EixoDaGuia::Vertical
                    },
                });
            }
        }
        p
    }
}

/// "Restringir ao conteúdo" no JSON: só escrito quando desligado — ligado é o
/// padrão, e o comportamento de sempre do endireitar.
pub fn restringir_em_json(restringir: bool, objeto: &mut Map<String, Value>) {
    if !restringir {
        objeto.insert("corte_restringir".into(), 0.into());
    }
}

pub fn restringir_de_json(json: &Value) -> bool {
    json.get("corte_restringir")
        .and_then(Value::as_f64)
        .is_none_or(|v| v != 0.0)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_duas_guias() -> PerspectivaGuiada {
        let mut p = PerspectivaGuiada {
            rotacao: [11.5, -2.0, 0.25],
            foco: 1.3,
            vertical: 3.0,
            horizontal: -1.5,
            ..Default::default()
        };
        p.guias[0] = Some(GuiaDePerspectiva {
            de: [0.2, 0.1],
            ate: [0.25, 0.9],
            eixo: EixoDaGuia::Vertical,
        });
        p.guias[2] = Some(GuiaDePerspectiva {
            de: [0.1, 0.3],
            ate: [0.9, 0.28],
            eixo: EixoDaGuia::Horizontal,
        });
        p
    }

    #[test]
    fn ida_e_volta_pelo_json() {
        let p = com_duas_guias();
        let mut objeto = Map::new();
        p.em_json(&mut objeto);
        let de_volta = PerspectivaGuiada::de_json(&Value::Object(objeto));
        assert_eq!(de_volta, p);
    }

    #[test]
    fn neutra_nao_escreve_nada() {
        let mut objeto = Map::new();
        PerspectivaGuiada::default().em_json(&mut objeto);
        restringir_em_json(true, &mut objeto);
        assert!(objeto.is_empty(), "{objeto:?}");
        assert_eq!(
            PerspectivaGuiada::de_json(&serde_json::json!({})),
            PerspectivaGuiada::default()
        );
        assert!(restringir_de_json(&serde_json::json!({})));
    }

    #[test]
    fn so_numeros_no_objeto_plano() {
        let mut objeto = Map::new();
        com_duas_guias().em_json(&mut objeto);
        restringir_em_json(false, &mut objeto);
        assert!(objeto.values().all(Value::is_number));
        assert!(objeto.keys().all(|k| k.starts_with("corte_")));
        assert!(!restringir_de_json(&Value::Object(objeto)));
    }

    #[test]
    fn guia_incompleta_ou_invalida_fica_de_fora() {
        let json = serde_json::json!({
            "corte_guia1_x1": 0.1, "corte_guia1_y1": 0.1, "corte_guia1_x2": 0.1,
            "corte_guia1_eixo": 0,
            "corte_persp_rx": "dez",
            "corte_persp_foco": -3.0
        });
        let p = PerspectivaGuiada::de_json(&json);
        assert_eq!(p.quantas_guias(), 0);
        assert_eq!(p.rotacao, [0.0; 3]);
        assert_eq!(p.foco, FOCO_PADRAO);
    }

    #[test]
    fn guias_sem_correcao_ainda_sao_guardadas() {
        let mut p = PerspectivaGuiada::default();
        p.guias[1] = Some(GuiaDePerspectiva {
            de: [0.5, 0.1],
            ate: [0.5, 0.9],
            eixo: EixoDaGuia::Vertical,
        });
        assert!(!p.corrige() && !p.e_neutra());
        let mut objeto = Map::new();
        p.em_json(&mut objeto);
        assert!(!objeto.contains_key("corte_persp_rx"));
        assert_eq!(PerspectivaGuiada::de_json(&Value::Object(objeto)), p);
    }
}
