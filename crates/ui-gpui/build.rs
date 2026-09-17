//! Embute os ícones e as imagens da interface no binário (`src/recursos.rs`).
//!
//! Uma tabela gerada, e não uma lista escrita à mão: um ícone novo em
//! `icones/` passa a existir sem mais nada, e um nome errado no código aparece
//! como ícone vazio na tela — o que o teste `todo_icone_do_app_existe` cobra.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut tabela = String::from("&[\n");
    for (pasta, prefixo) in [("icones", "icons/"), ("imagens", "imagens/")] {
        let dir = raiz.join(pasta);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut arquivos: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("ler {}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("svg" | "png" | "jpeg" | "jpg")
                )
            })
            .collect();
        arquivos.sort();
        for arquivo in arquivos {
            let nome = arquivo.file_name().unwrap().to_string_lossy();
            writeln!(
                tabela,
                "    ({:?}, include_bytes!({:?}) as &[u8]),",
                format!("{prefixo}{nome}"),
                arquivo.display().to_string()
            )
            .unwrap();
        }
    }
    tabela.push(']');
    let saida = Path::new(&std::env::var("OUT_DIR").unwrap()).join("recursos.rs");
    std::fs::write(saida, tabela).unwrap();
}
