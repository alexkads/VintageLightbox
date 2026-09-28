//! A revelação da revelação como o site a guarda: os ajustes e o enquadramento
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
        // A perspectiva guiada e o "restringir": só quando existem — a revelação
        // de uma foto sem eles fica a mesma de antes (ver `perspectiva.rs` do
        // domínio).
        corte.perspectiva().em_json(objeto);
        domain::value_objects::perspectiva::restringir_em_json(corte.restringir(), objeto);
    }
    json
}

/// Duas revelações dizem **a mesma coisa**? — o que o site guardou e o que o
/// catálogo tem agora.
///
/// 🚨 **Comparar o JSON por igualdade fazia o ensaio inteiro subir duas vezes.**
/// O corte 3:2 da revelação padrão sai do catálogo como `f32` e, pelo caminho da
/// subida, como `f64`: `corte_altura` virava `0.9988283514976501` de um lado e
/// `0.99882835149765` do outro — 1 ulp. A raiz achava que a revelação tinha mudado
/// enquanto a foto subia e mandava cada uma de novo como revelação (bilhete,
/// JPEG, upload), e o "Subindo" do canto ficava parado em 48↔47 durante o envio
/// inteiro (visto rodando o app com o cartão da D3100, 27/set/2026).
///
/// 🔑 Número é igual quando a diferença cabe na precisão do `f32` que o
/// catálogo guarda (1e-6 relativo); o resto compara como sempre.
pub fn mesmos_parametros(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1.0),
            _ => x == y,
        },
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(chave, vx)| y.get(chave).is_some_and(|vy| mesmos_parametros(vx, vy)))
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(vx, vy)| mesmos_parametros(vx, vy))
        }
        _ => a == b,
    }
}

/// [`mesmos_parametros`] sobre o texto guardado. Texto que não é JSON compara como
/// texto.
pub fn mesmos_parametros_em_texto(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(a),
        serde_json::from_str::<serde_json::Value>(b),
    ) {
        (Ok(a), Ok(b)) => mesmos_parametros(&a, &b),
        _ => a == b,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O caso medido: o corte 3:2 da revelação padrão, pelos dois caminhos.
    #[test]
    fn o_mesmo_corte_em_f32_e_f64_e_a_mesmos_parametros() {
        let agora = serde_json::json!({"saturation": -1.0, "corte_altura": 0.9988283514976501_f64});
        let subiu = serde_json::json!({"saturation": -1.0, "corte_altura": 0.99882835149765_f64});
        assert_ne!(agora, subiu, "por igualdade, eram diferentes");
        assert!(mesmos_parametros(&agora, &subiu));

        let de_f32 = ajustes_em_json(&Ajustes::default(), &CropSettings::default());
        assert!(mesmos_parametros_em_texto(
            &de_f32.to_string(),
            &de_f32.to_string()
        ));
    }

    #[test]
    fn mudanca_de_verdade_continua_diferente() {
        let sepia = serde_json::json!({"contrast": 1.08, "split_shadow_sat": 45.0});
        let pb = serde_json::json!({"contrast": 1.15, "split_shadow_sat": 0.0});
        assert!(!mesmos_parametros(&sepia, &pb));
        assert!(!mesmos_parametros(
            &serde_json::json!({"curva_m4": 127.5}),
            &serde_json::json!({"curva_m4": 127.6})
        ));
        assert!(
            !mesmos_parametros(
                &serde_json::json!({"a": 1.0}),
                &serde_json::json!({"a": 1.0, "b": 0.0})
            ),
            "campo a mais é outra revelação"
        );
    }

    #[test]
    fn o_corte_entra_por_nome_ao_lado_dos_ajustes() {
        let parametros = ajustes_em_json(&Ajustes::default(), &CropSettings::default());
        let objeto = parametros.as_object().expect("um objeto por nome");
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
