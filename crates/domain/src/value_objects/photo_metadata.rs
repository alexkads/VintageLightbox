//! Photo Metadata Value Object
//!
//! Contém metadados técnicos da fotografia (EXIF).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PhotoMetadata {
    /// Modelo da câmera
    pub camera_model: Option<String>,
    /// Fabricante da câmera
    pub camera_make: Option<String>,
    /// Data/hora **do disparo** (`DateTimeOriginal`), no formato EXIF
    /// `"YYYY:MM:DD HH:MM:SS"`.
    ///
    /// 🚨 **É o disparo, e não o último retoque** (20/set/2026). Até esta data o
    /// leitor preenchia isto com a tag `DateTime` do IFD0 — que é *quando o
    /// arquivo foi escrito pela última vez*. Num ensaio exportado do Lightroom
    /// todas as fotos saem com a mesma `DateTime` (a da exportação), e a grade
    /// ordenada por "Hora de captura" via um empate geral: a ordem do ensaio
    /// virava a ordem do desempate. Ver [`Self::chave_de_captura`].
    pub date_time: Option<String>,
    /// O subsegundo do disparo (`SubSecTimeOriginal`), quando a câmera o grava.
    ///
    /// 🔑 **É o que desempata rajada.** Seis fotos disparadas no mesmo segundo
    /// têm `date_time` idêntica; sem isto, a sequência delas passa a depender do
    /// nome do arquivo — que mente quando o contador da câmera vira.
    #[serde(default)]
    pub sub_sec: Option<String>,
    /// ISO
    pub iso: Option<u32>,
    /// Abertura (f-number)
    pub aperture: Option<f64>,
    /// Velocidade do obturador
    pub shutter_speed: Option<String>,
    /// Distância focal
    pub focal_length: Option<f64>,
    /// Largura da imagem
    pub width: Option<u32>,
    /// Altura da imagem
    pub height: Option<u32>,
}

impl PhotoMetadata {
    /// A chave que põe as fotos na ordem em que foram fotografadas.
    ///
    /// Devolve `"YYYY:MM:DD HH:MM:SS.NNN"` — a data do disparo com o subsegundo
    /// **normalizado em três dígitos**, e `None` quando não há data.
    ///
    /// 🚨 **A normalização é a razão de isto ser uma função.** O subsegundo do
    /// EXIF é uma *fração decimal*: `"5"` é meio segundo e `"45"` é 45
    /// centésimos, então comparar os textos crus poria `"45"` antes de `"5"` —
    /// invertendo duas fotos de uma rajada. Completado à direita com zeros,
    /// `"500"` e `"450"` comparam como números, e a comparação continua sendo a
    /// de texto, que é o que o SQLite e a grade já sabem fazer.
    pub fn chave_de_captura(&self) -> Option<String> {
        let data = self.date_time.as_deref()?.trim();
        if data.is_empty() {
            return None;
        }
        let digitos: String = self
            .sub_sec
            .as_deref()
            .unwrap_or("")
            .chars()
            .filter(char::is_ascii_digit)
            .take(3)
            .collect();
        Some(format!("{data}.{:0<3}", digitos))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com(data: &str, sub: Option<&str>) -> PhotoMetadata {
        PhotoMetadata {
            date_time: Some(data.to_string()),
            sub_sec: sub.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn sem_data_nao_ha_chave() {
        assert_eq!(PhotoMetadata::default().chave_de_captura(), None);
        assert_eq!(com("   ", None).chave_de_captura(), None);
    }

    #[test]
    fn sem_subsegundo_a_chave_termina_em_zeros() {
        assert_eq!(
            com("2026:09:20 14:03:07", None).chave_de_captura().unwrap(),
            "2026:09:20 14:03:07.000"
        );
    }

    /// 🚨 O caso que a normalização existe para resolver: `"5"` é meio segundo,
    /// e vem **depois** de `"45"`, que é quarenta e cinco centésimos.
    #[test]
    fn o_subsegundo_compara_como_fracao_e_nao_como_texto() {
        let meio = com("2026:09:20 14:03:07", Some("5"))
            .chave_de_captura()
            .unwrap();
        let quase_meio = com("2026:09:20 14:03:07", Some("45"))
            .chave_de_captura()
            .unwrap();
        assert_eq!(meio, "2026:09:20 14:03:07.500");
        assert_eq!(quase_meio, "2026:09:20 14:03:07.450");
        assert!(quase_meio < meio, "45 centésimos vêm antes de meio segundo");
    }

    #[test]
    fn a_rajada_fica_em_ordem() {
        let mut chaves: Vec<String> = ["07", "45", "5", "123"]
            .iter()
            .map(|s| {
                com("2026:09:20 14:03:07", Some(s))
                    .chave_de_captura()
                    .unwrap()
            })
            .collect();
        chaves.sort();
        assert_eq!(
            chaves,
            vec![
                "2026:09:20 14:03:07.070",
                "2026:09:20 14:03:07.123",
                "2026:09:20 14:03:07.450",
                "2026:09:20 14:03:07.500",
            ]
        );
    }

    /// O que a câmera grava com lixo junto não derruba a chave.
    #[test]
    fn o_subsegundo_sujo_vira_digitos() {
        assert_eq!(
            com("2026:09:20 14:03:07", Some(" 07 "))
                .chave_de_captura()
                .unwrap(),
            "2026:09:20 14:03:07.070"
        );
    }

    /// 🔑 Um `PhotoMetadata` gravado antes do campo `sub_sec` continua sendo
    /// lido: o catálogo tem milhares deles em JSON.
    #[test]
    fn o_json_antigo_ainda_desserializa() {
        let antigo = r#"{"camera_model":"Canon R6","camera_make":null,"date_time":"2026:09:20 14:03:07","iso":100,"aperture":null,"shutter_speed":null,"focal_length":null,"width":null,"height":null}"#;
        let lido: PhotoMetadata = serde_json::from_str(antigo).unwrap();
        assert_eq!(lido.sub_sec, None);
        assert_eq!(lido.chave_de_captura().unwrap(), "2026:09:20 14:03:07.000");
    }
}
