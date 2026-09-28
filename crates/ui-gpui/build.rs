//! Embute os ícones e as imagens da interface no binário (`src/recursos.rs`).
//!
//! Uma tabela gerada, e não uma lista escrita à mão: um ícone novo em
//! `icones/` passa a existir sem mais nada, e um nome errado no código aparece
//! como ícone vazio na tela — o que o teste `todo_icone_do_app_existe` cobra.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// O leitor do template, o mesmo que o app usa — ver `src/tema/preset.rs`.
#[path = "src/tema/preset.rs"]
mod preset;

fn main() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let template = template(raiz);
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
            let nome = arquivo.file_name().unwrap().to_string_lossy().to_string();
            // 🎨 O ícone da biblioteca do template, quando ela tem este.
            let arquivo = if pasta == "icones" {
                icone_do_template(raiz, template.icones, &nome).unwrap_or(arquivo)
            } else {
                arquivo
            };
            writeln!(
                tabela,
                "    ({:?}, include_bytes!({:?}) as &[u8]),",
                format!("{prefixo}{nome}"),
                arquivo.display().to_string()
            )
            .unwrap();
        }
    }
    for arquivo in fontes_do_template(raiz, &template) {
        let nome = arquivo.strip_prefix(raiz).unwrap().display().to_string();
        writeln!(
            tabela,
            "    ({:?}, include_bytes!({:?}) as &[u8]),",
            nome.replace('\\', "/"),
            arquivo.display().to_string()
        )
        .unwrap();
    }
    tabela.push(']');
    let saida = Path::new(&std::env::var("OUT_DIR").unwrap()).join("recursos.rs");
    std::fs::write(saida, tabela).unwrap();

    icone_do_executavel(raiz);
    jeito_de_instalar();
}

/// 🎨 **O template deste binário** — `crates/ui-gpui/template.toml`, ou o
/// `VLB_TEMPLATE` de quem compila, que vale por cima do arquivo para
/// experimentar sem editá-lo:
///
/// ```sh
/// VLB_TEMPLATE=bIkeymG cargo run -p ui-gpui          # o código do /create
/// VLB_TEMPLATE=lyra cargo run -p ui-gpui             # um preset com nome
/// VLB_TEMPLATE=outro-template.toml cargo run -p ui-gpui
/// ```
///
/// O resultado vai para `VLB_TEMPLATE_RESOLVIDO`, que `tema::template()` lê.
/// Template ilegível **para a compilação** com a linha e o que vale: um erro
/// aqui chega a todo balcão, e o lugar de vê-lo é antes do `make producao`.
fn template(raiz: &Path) -> preset::Preset {
    let arquivo = raiz.join("template.toml");
    println!("cargo:rerun-if-changed={}", arquivo.display());
    println!("cargo:rerun-if-env-changed=VLB_TEMPLATE");
    let (origem, texto) = match std::env::var("VLB_TEMPLATE") {
        Ok(valor) if Path::new(&valor).is_file() => {
            println!("cargo:rerun-if-changed={valor}");
            (valor.clone(), std::fs::read_to_string(&valor).unwrap())
        }
        Ok(valor) if !valor.trim().is_empty() => (
            "VLB_TEMPLATE".to_string(),
            format!("preset = \"{}\"", valor.trim()),
        ),
        _ => (
            arquivo.display().to_string(),
            std::fs::read_to_string(&arquivo).unwrap_or_default(),
        ),
    };
    let template = preset::ler(&texto).unwrap_or_else(|erro| panic!("\n🎨 {origem}: {erro}\n"));
    println!(
        "cargo:rustc-env=VLB_TEMPLATE_RESOLVIDO={}",
        template.parametros()
    );
    template
}

/// O SVG de `icones-<biblioteca>/` com o mesmo nome do lucide, se houver.
/// O que a biblioteca não tem continua lucide (`template/icones.json`).
fn icone_do_template(raiz: &Path, biblioteca: &str, nome: &str) -> Option<PathBuf> {
    if biblioteca == "lucide" {
        return None;
    }
    let pasta = raiz.join(format!("icones-{biblioteca}"));
    println!("cargo:rerun-if-changed={}", pasta.display());
    if !pasta.is_dir() {
        panic!(
            "\n🎨 O template pede os ícones `{biblioteca}`, e `{}` não existe.\n   \
             python3 scripts/baixar-do-template.py --icones {biblioteca}\n",
            pasta.display()
        );
    }
    let arquivo = pasta.join(nome);
    arquivo.is_file().then_some(arquivo)
}

/// Os `.ttf` da fonte do corpo e da dos títulos. `sistema` não embute nada.
fn fontes_do_template(raiz: &Path, template: &preset::Preset) -> Vec<PathBuf> {
    let mut slugs = vec![template.fonte, template.fonte_dos_titulos];
    slugs.retain(|s| *s != preset::FONTE_DO_SISTEMA && *s != preset::HERDAR);
    slugs.dedup();
    let mut arquivos = Vec::new();
    for slug in slugs {
        let pasta = raiz.join("fontes").join(slug);
        println!("cargo:rerun-if-changed={}", pasta.display());
        let mut da_familia: Vec<_> = std::fs::read_dir(&pasta)
            .map(|dir| {
                dir.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ttf"))
                    .collect()
            })
            .unwrap_or_default();
        if da_familia.is_empty() {
            panic!(
                "\n🎨 O template pede a fonte `{slug}`, e `{}` não tem nenhum .ttf.\n   \
                 python3 scripts/baixar-do-template.py --fonte {slug}\n",
                pasta.display()
            );
        }
        da_familia.sort();
        arquivos.extend(da_familia);
    }
    arquivos
}

/// 🔄 **Como este binário vai ser atualizado** — gravado nele, em
/// `VLB_JEITO_DE_INSTALAR`, e lido por `atualizacao::porta::jeito_desta_instalacao`.
///
/// | Perfil | Quem compila | Como atualiza |
/// |---|---|---|
/// | `instalador` | `scripts/instalar-vintagelightbox-gpui.cmd`, no balcão | repetindo o instalador (compila o `main`) |
/// | `release` | `scripts/empacotar.sh`, o pacote assinado | a faixa baixa e instala sozinha |
/// | `debug` | quem desenvolve | `git pull` |
///
/// 🔑 **Pelo diretório de saída, e não por variável do instalador.** O
/// balcão roda a cópia do instalador que baixou um dia, e uma variável nova
/// nele só valeria para quem baixasse de novo. O perfil `instalador` já existe
/// em todas as cópias — e o cargo põe o nome do perfil no caminho do `OUT_DIR`.
fn jeito_de_instalar() {
    let saida = std::env::var("OUT_DIR").unwrap_or_default();
    let perfis: Vec<&str> = std::path::Path::new(&saida)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let jeito = if perfis.contains(&"instalador") {
        "compilado"
    } else if perfis.contains(&"release") {
        "pacote"
    } else {
        "desenvolvimento"
    };
    println!("cargo:rustc-env=VLB_JEITO_DE_INSTALAR={jeito}");
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
/// `packager.toml` já lista): dois arquivos para a mesma logo é a revelação de um
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
