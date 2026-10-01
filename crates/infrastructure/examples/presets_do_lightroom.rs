//! Gera o grupo "LRs" das predefinições do sistema a partir de uma pasta de
//! `.xmp` do Lightroom — o JSON que o desktop (`use-cases`) e o site
//! (`presets-lr.json`) leem.
//!
//! ```bash
//! cargo run --release -p infrastructure --example presets_do_lightroom -- \
//!     "<pasta com os .xmp>" > crates/use-cases/src/presets/lightroom.json
//! ```
//!
//! 🔑 **Quem só mexe na vinheta pós-corte soma; o resto recomeça do neutro.**
//! "Vinheta Borda" vai por cima do visual que já está na foto, como no
//! Lightroom; "RecordarFotos Sépia" é o visual inteiro, e somado a outro daria
//! uma mistura dos dois. A vinheta que soma leva os seus campos mesmo no
//! neutro: "Vinheta Nenhuma" é justamente `PostCropVignetteAmount = 0`.

use std::collections::BTreeMap;

use infrastructure::lightroom::{self, Valor};

/// Chaves do XMP que não mexem na imagem — descrição do preset.
fn eh_descricao(chave: &str) -> bool {
    chave.starts_with("Supports")
        || matches!(
            chave,
            "UUID"
                | "Version"
                | "PresetType"
                | "Cluster"
                | "Copyright"
                | "ContactInfo"
                | "CameraModelRestriction"
                | "HasSettings"
                | "RequiresRGBTables"
                | "ShowInPresets"
                | "ShowInQuickActions"
                | "Name"
                | "ShortName"
                | "SortName"
                | "Group"
                | "Description"
                | "OverrideLookVignette"
                | "ProcessVersion"
        )
}

fn main() {
    let pasta = std::env::args().nth(1).expect("a pasta dos .xmp");
    let mut arquivos: Vec<_> = std::fs::read_dir(&pasta)
        .expect("ler a pasta")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("xmp")))
        .collect();
    arquivos.sort();

    let mut saida = Vec::new();
    for caminho in arquivos {
        let nome = caminho.file_stem().unwrap().to_string_lossy().trim().to_string();
        let texto = std::fs::read_to_string(&caminho).expect("ler o .xmp");
        let bruto = lightroom::ler_xmp(&texto, &nome).expect("é .xmp do Lightroom");
        let traduzido = lightroom::traduzir(&bruto);

        let so_vinheta = bruto
            .ajustes
            .keys()
            .all(|k| eh_descricao(k) || k.starts_with("PostCropVignette"));

        let mut ajustes: BTreeMap<String, f32> = traduzido
            .ajustes
            .iter()
            .map(|(c, v)| (c.to_string(), v))
            .collect();
        if so_vinheta {
            // Os campos da vinheta entram inteiros, neutros inclusive.
            let numero = |k: &str| bruto.ajustes.get(k).and_then(|v| match v {
                Valor::Numero(n) => Some(*n),
                _ => None,
            });
            let quantidade = numero("PostCropVignetteAmount").unwrap_or(0.0);
            ajustes.insert("pcv_amount".into(), quantidade);
            if quantidade != 0.0 {
                for (chave, campo, neutro) in [
                    ("PostCropVignetteMidpoint", "pcv_midpoint", 50.0),
                    ("PostCropVignetteRoundness", "pcv_roundness", 0.0),
                    ("PostCropVignetteFeather", "pcv_feather", 50.0),
                    ("PostCropVignetteHighlightContrast", "pcv_highlights", 0.0),
                ] {
                    ajustes.insert(campo.into(), numero(chave).unwrap_or(neutro));
                }
                let estilo = numero("PostCropVignetteStyle").unwrap_or(1.0);
                ajustes.insert("pcv_style".into(), (estilo - 1.0).round().clamp(0.0, 2.0));
            }
        }

        // O perfil: os de base da Adobe (Color, Monochrome) são o ponto de
        // partida do motor; os criativos têm tabela de cor da Adobe, que não
        // vem no preset.
        let mut avisos = traduzido.ignorados.clone();
        let look = Look::do_xmp(&texto);
        if let Some(look) = &look {
            let pb = look.nome.starts_with("B&W") || look.nome.contains("Monochrome");
            if pb {
                ajustes.insert("bw_ativo".into(), 1.0);
                ajustes.insert("saturation".into(), -1.0);
            }
            if !matches!(look.nome.as_str(), "Adobe Color" | "Adobe Monochrome") {
                avisos.push(format!(
                    "perfil criativo \"{}\" (a tabela de cor é da Adobe)",
                    look.nome
                ));
            }
        }
        let mascaras = texto.matches("crs:What=\"Correction\"").count();
        if mascaras > 0 {
            avisos.push(format!("{mascaras} ajuste(s) local(is) com máscara"));
        }

        // Quatro casas, como o tradutor grava no banco — e sem o ruído do f32.
        let ajustes: BTreeMap<String, f64> = ajustes
            .into_iter()
            .map(|(c, v)| (c, (v as f64 * 10_000.0).round() / 10_000.0))
            .collect();
        saida.push(serde_json::json!({
            "nome": nome,
            "recomeca": !so_vinheta,
            "ajustes": ajustes,
            "avisos": avisos,
        }));
    }
    println!("{}", serde_json::to_string_pretty(&saida).unwrap());
}

struct Look {
    nome: String,
}

impl Look {
    fn do_xmp(texto: &str) -> Option<Look> {
        let bloco = texto.split("<crs:Look>").nth(1)?.split("</crs:Look>").next()?;
        let depois = bloco.split("crs:Name=\"").nth(1)?;
        let nome = depois.split('"').next()?.replace("&amp;", "&");
        Some(Look { nome })
    }
}
