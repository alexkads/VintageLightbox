//! Revelar uma foto no `darktable-cli` com um `.dtstyle`, sem passar pelo banco
//! do darktable.
//!
//! 🚨 **`darktable-cli --style` só acha estilo que já está no banco** do
//! darktable (`data.db`), e um `--configdir` novo começa vazio. O caminho que
//! funciona é o segundo argumento do `darktable-cli`, um XMP de histórico: aqui
//! ele é gerado do `.dtstyle`, módulo a módulo, com os mesmos `op_params` e a
//! mesma ordem (`iop_list`) — o resultado é o do estilo aplicado.
//!
//! O `--configdir` vai para uma pasta própria, para não tocar a configuração do
//! darktable de quem usa a máquina.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// Onde costuma estar o `darktable-cli` (ou o que está no PATH).
pub fn achar() -> PathBuf {
    [
        "C:/Program Files/darktable/bin/darktable-cli.exe",
        "/Applications/darktable.app/Contents/MacOS/darktable-cli",
        "/usr/bin/darktable-cli",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
    .unwrap_or_else(|| PathBuf::from("darktable-cli"))
}

/// O XMP de histórico com os módulos do `.dtstyle`.
pub fn xmp_do_estilo(dtstyle: &str) -> String {
    let doc = roxmltree::Document::parse(dtstyle).expect("o .dtstyle é XML");
    let texto = |no: roxmltree::Node, nome: &str| -> String {
        no.children()
            .find(|c| c.has_tag_name(nome))
            .and_then(|c| c.text())
            .unwrap_or("")
            .to_string()
    };
    let raiz = doc.root_element();
    let iop_list = raiz
        .descendants()
        .find(|n| n.has_tag_name("iop_list"))
        .and_then(|n| n.text())
        .unwrap_or("");
    let escapar = |s: String| {
        s.replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
    };
    let mut lis = Vec::new();
    for (i, p) in raiz
        .descendants()
        .filter(|n| n.has_tag_name("plugin"))
        .enumerate()
    {
        lis.push(format!(
            "     <rdf:li darktable:num=\"{i}\" darktable:operation=\"{}\" darktable:enabled=\"{}\" \
             darktable:modversion=\"{}\" darktable:params=\"{}\" darktable:multi_name=\"{}\" \
             darktable:multi_name_hand_edited=\"0\" darktable:multi_priority=\"{}\" \
             darktable:blendop_version=\"{}\" darktable:blendop_params=\"{}\"/>",
            texto(p, "operation"),
            texto(p, "enabled"),
            texto(p, "module"),
            texto(p, "op_params"),
            escapar(texto(p, "multi_name")),
            texto(p, "multi_priority"),
            texto(p, "blendop_version"),
            texto(p, "blendop_params"),
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"XMP Core 4.4.0-Exiv2\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         <rdf:Description rdf:about=\"\" xmlns:darktable=\"http://darktable.sf.net/\" \
         darktable:xmp_version=\"5\" darktable:auto_presets_applied=\"1\" \
         darktable:history_end=\"{}\" darktable:iop_order_version=\"0\" \
         darktable:iop_order_list=\"{iop_list}\">\n\
         <darktable:history>\n<rdf:Seq>\n{}\n</rdf:Seq>\n</darktable:history>\n\
         </rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n",
        lis.len(),
        lis.join("\n")
    )
}

/// Revela `foto` com o estilo em `saida` (JPEG), se ela ainda não existe.
/// Devolve `false` se o `darktable-cli` falhou.
pub fn revelar(
    cli: &Path,
    dtstyle: &Path,
    foto: &Path,
    saida: &Path,
    pasta_de_trabalho: &Path,
) -> bool {
    if saida.exists() {
        return true;
    }
    std::fs::create_dir_all(pasta_de_trabalho).unwrap();
    let xmp = pasta_de_trabalho.join("estilo.xmp");
    let estilo = std::fs::read_to_string(dtstyle).expect("ler o .dtstyle");
    std::fs::write(&xmp, xmp_do_estilo(&estilo)).unwrap();
    // O darktable-cli no Windows quer barras normais.
    let barra = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let status = Command::new(cli)
        .arg(barra(foto))
        .arg(barra(&xmp))
        .arg(barra(saida))
        .args(["--hq", "true", "--core", "--configdir"])
        .arg(barra(&pasta_de_trabalho.join("configuracao")))
        .output();
    match status {
        Ok(s) if s.status.success() && saida.exists() => true,
        Ok(s) => {
            eprintln!(
                "darktable-cli falhou em {}: {}",
                foto.display(),
                String::from_utf8_lossy(&s.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("não consegui rodar {}: {e}", cli.display());
            false
        }
    }
}
