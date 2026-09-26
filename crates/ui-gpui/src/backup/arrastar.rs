//! O que foi arrastado — **inclusive pasta**.
//!
//! O par nativo de `backup/arrastar.ts` do site. Lá é preciso percorrer a API de
//! *entries* do navegador; aqui o `gpui_kit::ExternalPaths` já traz caminhos de
//! verdade, e percorrer é ler o disco.
//!
//! 🔑 **É a diferença que o dono pediu** (2026-09-18: *"para arrastar pastas os
//! arquivos"*), e a única parte desta tela que o legado não tinha.

use std::path::{Path, PathBuf};

use super::fila::relativo_ao_solto;

/// Quantos níveis de pasta se desce.
///
/// ⚠️ **Teto existe porque a árvore pode não ter fim.** Um *symlink* circular
/// vira uma árvore infinita, e a varredura roda até o app parar de responder —
/// sem erro, sem fim. Vinte níveis é mais fundo que qualquer organização de
/// estúdio, e é o mesmo teto do site.
const PROFUNDIDADE_MAXIMA: usize = 20;

/// Quantos arquivos se aceita de uma vez.
///
/// ⚠️ Não é limite de produto: é o que impede uma pasta solta por engano (a raiz
/// do disco) de virar uma leitura de horas antes de a tela poder dizer qualquer
/// coisa. Quem precisa de mais solta em duas vezes.
const ARQUIVOS_MAXIMOS: usize = 20_000;

/// Um arquivo pronto para subir, com o lugar dele na árvore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArquivoArrastado {
    pub origem: PathBuf,
    /// Relativo ao que foi solto: `ensaio-silva/2026/DSC_01.NEF`.
    pub relativo: String,
    pub bytes: u64,
}

/// Tudo o que foi solto, achatado, com os caminhos relativos preservados.
///
/// **Arquivo ilegível some da lista em vez de derrubar o arrasto inteiro** — um
/// `.DS_Store` sem permissão não pode impedir uma pasta de 300 fotos de subir.
pub fn arquivos_soltos(soltos: &[PathBuf]) -> Vec<ArquivoArrastado> {
    let mut achados = Vec::new();
    for solto in soltos {
        percorrer(solto, solto, 0, &mut achados);
        if achados.len() >= ARQUIVOS_MAXIMOS {
            break;
        }
    }
    achados
}

fn percorrer(solto: &Path, atual: &Path, nivel: usize, achados: &mut Vec<ArquivoArrastado>) {
    if achados.len() >= ARQUIVOS_MAXIMOS {
        return;
    }
    // `symlink_metadata`: seguir o link é como a árvore circular nasce.
    let Ok(dados) = std::fs::symlink_metadata(atual) else {
        return;
    };
    if dados.is_symlink() {
        return;
    }
    if dados.is_file() {
        // O oculto do sistema não é acervo de ninguém — a mesma regra do disco
        // local no backend.
        if atual
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            return;
        }
        achados.push(ArquivoArrastado {
            origem: atual.to_path_buf(),
            relativo: relativo_ao_solto(solto, atual),
            bytes: dados.len(),
        });
        return;
    }
    if !dados.is_dir() || nivel >= PROFUNDIDADE_MAXIMA {
        return;
    }
    let Ok(leitura) = std::fs::read_dir(atual) else {
        return;
    };
    // Ordenado: a mesma pasta arrastada duas vezes produz a mesma fila, e o
    // operador reconhece onde parou. `read_dir` não promete ordem nenhuma.
    let mut filhos: Vec<PathBuf> = leitura.flatten().map(|e| e.path()).collect();
    filhos.sort();
    for filho in filhos {
        percorrer(solto, &filho, nivel + 1, achados);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_de_teste() -> tempfile::TempDir {
        let raiz = tempfile::tempdir().expect("pasta temporária");
        let ensaio = raiz.path().join("ensaio-silva");
        std::fs::create_dir_all(ensaio.join("2026")).expect("subpasta");
        std::fs::write(ensaio.join("capa.jpg"), b"capa").expect("arquivo");
        std::fs::write(ensaio.join("2026").join("DSC_01.NEF"), b"raw!").expect("arquivo");
        std::fs::write(ensaio.join(".DS_Store"), b"lixo").expect("arquivo");
        raiz
    }

    /// 🔑 A árvore inteira vem, e o nome da pasta solta entra no caminho.
    #[test]
    fn a_pasta_arrastada_vira_a_arvore_inteira() {
        let raiz = pasta_de_teste();
        let achados = arquivos_soltos(&[raiz.path().join("ensaio-silva")]);

        let mut caminhos: Vec<&str> = achados.iter().map(|a| a.relativo.as_str()).collect();
        caminhos.sort_unstable();
        assert_eq!(
            caminhos,
            ["ensaio-silva/2026/DSC_01.NEF", "ensaio-silva/capa.jpg"]
        );
    }

    /// O oculto do sistema não sobe.
    #[test]
    fn o_ds_store_fica_de_fora() {
        let raiz = pasta_de_teste();
        let achados = arquivos_soltos(&[raiz.path().join("ensaio-silva")]);

        assert!(!achados.iter().any(|a| a.relativo.contains(".DS_Store")));
    }

    #[test]
    fn o_arquivo_avulso_sobe_com_o_proprio_nome() {
        let raiz = pasta_de_teste();
        let achados = arquivos_soltos(&[raiz.path().join("ensaio-silva").join("capa.jpg")]);

        assert_eq!(achados.len(), 1);
        assert_eq!(achados[0].relativo, "capa.jpg");
        assert_eq!(achados[0].bytes, 4);
    }

    /// 🚨 O caso que o teto existe para impedir: um link que aponta para cima
    /// faria a varredura não terminar.
    #[cfg(unix)]
    #[test]
    fn o_link_circular_nao_prende_a_varredura() {
        let raiz = pasta_de_teste();
        let ensaio = raiz.path().join("ensaio-silva");
        std::os::unix::fs::symlink(&ensaio, ensaio.join("eu-mesmo")).expect("link");

        let achados = arquivos_soltos(&[ensaio]);

        assert_eq!(achados.len(), 2, "só os dois arquivos de verdade");
    }

    #[test]
    fn o_que_nao_existe_nao_e_erro() {
        assert!(arquivos_soltos(&[PathBuf::from("/nao/existe/mesmo")]).is_empty());
    }
}
