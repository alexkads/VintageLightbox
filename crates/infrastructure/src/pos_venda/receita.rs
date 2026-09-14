//! A receita da revelação como o site a guarda: os ajustes e o enquadramento
//! por nome, num objeto JSON — a coluna `ajustes` (JSONB) de `pos_venda_fotos`.
//!
//! 🔑 **Em JSON puro, sem compressão** (decisão do dono, 14/set/2026). O site e
//! este app gravam e leem o mesmo objeto.

use domain::value_objects::CropSettings;

use crate::gpu_adjustments::Ajustes;

/// Os ajustes e o enquadramento como o site os grava: um objeto só, por nome.
///
/// 🔑 **O corte entra com o prefixo `corte_`**, que é o que `corteParaJson` do
/// editor faz — e é assim que a web lê de volta. Um segundo formato aqui faria
/// a foto revelada no app abrir sem enquadramento no navegador.
pub fn ajustes_em_json(ajustes: &Ajustes, corte: &CropSettings) -> serde_json::Value {
    let mut json = serde_json::to_value(ajustes).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(objeto) = json.as_object_mut() {
        objeto.insert("corte_x".into(), corte.crop_x().into());
        objeto.insert("corte_y".into(), corte.crop_y().into());
        objeto.insert("corte_largura".into(), corte.crop_width().into());
        objeto.insert("corte_altura".into(), corte.crop_height().into());
        objeto.insert("corte_giro90".into(), corte.rotation_90().into());
        objeto.insert("corte_angulo".into(), corte.angle().into());
        objeto.insert(
            "corte_espelho_h".into(),
            i32::from(corte.flip_horizontal()).into(),
        );
        objeto.insert(
            "corte_espelho_v".into(),
            i32::from(corte.flip_vertical()).into(),
        );
    }
    json
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_corte_entra_por_nome_ao_lado_dos_ajustes() {
        let receita = ajustes_em_json(&Ajustes::default(), &CropSettings::default());
        let objeto = receita.as_object().expect("um objeto por nome");
        for campo in [
            "corte_x",
            "corte_y",
            "corte_largura",
            "corte_altura",
            "corte_giro90",
            "corte_angulo",
            "corte_espelho_h",
            "corte_espelho_v",
        ] {
            assert!(objeto.contains_key(campo), "{campo}");
        }
        assert!(objeto.contains_key("exposure"));
    }
}
