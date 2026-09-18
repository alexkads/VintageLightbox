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

    icone_do_executavel(raiz);
}

/// 🪟 **O ícone do `.exe` no Windows** (dono, 18/set/2026: *"no Windows não
/// aparece a logo"*).
///
/// O GPUI desenha a janela com o **recurso de ícone de id 1 do próprio
/// executável** — `LoadImageW(modulo, PCWSTR(1), IMAGE_ICON, …)`, em
/// `gpui/src/platform/windows/platform.rs`. Sem recurso nenhum a chamada falha e
/// ele cai num ícone vazio: barra de título e barra de tarefas sem logo. No
/// macOS o ícone vem do `.app` e no Linux do `.desktop`, e por isso lá não
/// faltava.
///
/// 🔑 **O mesmo `.ico` do instalador** (`empacotamento/icones/icone.ico`, que o
/// `packager.toml` já lista): dois arquivos para a mesma logo é a receita de um
/// deles envelhecer sem ninguém notar.
fn icone_do_executavel(raiz: &Path) {
    // ⚠️ **O alvo, e não o host**: `cfg!(windows)` aqui falaria da máquina que
    // compila, e o pacote do Windows sai de uma máquina Windows hoje, mas não
    // por obrigação.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = raiz.join("../../empacotamento/icones/icone.ico");
    if !ico.exists() {
        // Sem o arquivo, seguir sem logo é melhor do que não compilar o app.
        println!("cargo:warning=ícone não encontrado em {}", ico.display());
        return;
    }
    println!("cargo:rerun-if-changed={}", ico.display());
    let rc = Path::new(&std::env::var("OUT_DIR").unwrap()).join("icone.rc");
    // O `.rc` quer o caminho absoluto, com a barra invertida escapada.
    let caminho = ico.display().to_string().replace('\\', "\\\\");
    std::fs::write(&rc, format!("1 ICON \"{caminho}\"\n")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .expect("embutir o ícone no executável");
}
