//! As pastas do acervo, tiradas dos caminhos das fotos.
//!
//! O catálogo não guarda uma tabela de pastas — guarda o caminho de cada foto.
//! A árvore lateral é uma **leitura** desses caminhos, e não um cadastro: é o
//! que o `crates/ui` faz em `folder_tree.rs`, e é o que mantém a lista honesta
//! quando uma importação nova chega.

use std::collections::BTreeMap;

use adapters::view_models::PhotoViewModel;

/// Uma pasta e quantas fotos ela tem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pasta {
    /// Caminho completo do diretório, como está no banco.
    pub caminho: String,
    /// O nome que aparece na tela — o último trecho do caminho.
    pub nome: String,
    /// Quantas fotos estão **nesta** pasta, sem contar subpastas.
    ///
    /// Sem subpastas de propósito: o `crates/ui` lista pastas planas, e somar
    /// as filhas faria o número da mãe discordar do que a grade mostra ao
    /// clicar nela — que é o pior tipo de número, o que parece certo.
    pub quantas: usize,
}

/// O diretório de um caminho de arquivo.
///
/// Feito por texto, e não por `Path::parent()`: o caminho vem do banco e pode
/// ter sido gravado noutro sistema operacional. `Path` no macOS não reconhece
/// `\` como separador, e a pasta inteira viraria um nome só.
fn diretorio_de(caminho: &str) -> Option<&str> {
    let corte = caminho.rfind(['/', '\\'])?;
    if corte == 0 {
        // Arquivo na raiz: o diretório é a própria barra, e não string vazia.
        return Some(&caminho[..1]);
    }
    Some(&caminho[..corte])
}

/// O último trecho de um caminho de diretório.
fn nome_de(caminho: &str) -> &str {
    match caminho.rfind(['/', '\\']) {
        Some(corte) if corte + 1 < caminho.len() => &caminho[corte + 1..],
        // A raiz não tem nome próprio; mostrar o separador é mais honesto que
        // mostrar vazio, que pareceria pasta sem nome.
        _ => caminho,
    }
}

/// As pastas do acervo, em ordem alfabética e com a contagem de cada uma.
///
/// `BTreeMap` e não `HashMap`: a ordem tem de ser estável entre execuções. Com
/// hash, a árvore lateral se reorganizaria a cada abertura do app, e procurar
/// pasta viraria adivinhação.
pub fn pastas_do_acervo(fotos: &[PhotoViewModel]) -> Vec<Pasta> {
    let mut contagem: BTreeMap<&str, usize> = BTreeMap::new();

    for foto in fotos {
        if let Some(dir) = diretorio_de(&foto.path) {
            *contagem.entry(dir).or_insert(0) += 1;
        }
    }

    contagem
        .into_iter()
        .map(|(caminho, quantas)| Pasta {
            nome: nome_de(caminho).to_string(),
            caminho: caminho.to_string(),
            quantas,
        })
        .collect()
}

/// Se a foto está na pasta escolhida.
///
/// Comparação **exata** do diretório, e não `starts_with`: com prefixo,
/// escolher `/fotos/2024` traria também `/fotos/2024-descartes`, que é outra
/// pasta com nome parecido. É o mesmo motivo de a contagem não somar subpastas.
pub fn na_pasta(foto: &PhotoViewModel, pasta: &str) -> bool {
    diretorio_de(&foto.path) == Some(pasta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foto(caminho: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: caminho.to_string(),
            name: caminho.rsplit('/').next().unwrap_or(caminho).to_string(),
            path: caminho.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn agrupa_por_diretorio_e_conta() {
        let fotos = vec![
            foto("/fotos/2024/a.nef"),
            foto("/fotos/2024/b.nef"),
            foto("/fotos/2025/c.nef"),
        ];

        let pastas = pastas_do_acervo(&fotos);

        assert_eq!(pastas.len(), 2);
        assert_eq!(pastas[0].caminho, "/fotos/2024");
        assert_eq!(pastas[0].nome, "2024");
        assert_eq!(pastas[0].quantas, 2);
        assert_eq!(pastas[1].quantas, 1);
    }

    #[test]
    fn a_ordem_e_estavel_entre_execucoes() {
        // Com `HashMap`, a árvore lateral se reorganizaria a cada abertura do
        // app, e procurar pasta viraria adivinhação.
        let fotos = vec![foto("/z/1.nef"), foto("/a/2.nef"), foto("/m/3.nef")];

        let nomes: Vec<_> = pastas_do_acervo(&fotos)
            .into_iter()
            .map(|p| p.caminho)
            .collect();

        assert_eq!(nomes, vec!["/a", "/m", "/z"]);
    }

    #[test]
    fn caminho_do_windows_nao_vira_uma_pasta_so() {
        // O caminho vem do banco e pode ter sido gravado noutro sistema.
        // `Path::parent()` no macOS não reconhece `\`, e a pasta inteira
        // viraria o nome do arquivo.
        let fotos = vec![foto(r"C:\Fotos\2024\a.nef")];

        let pastas = pastas_do_acervo(&fotos);

        assert_eq!(pastas[0].caminho, r"C:\Fotos\2024");
        assert_eq!(pastas[0].nome, "2024");
    }

    #[test]
    fn arquivo_na_raiz_tem_a_barra_como_pasta() {
        let fotos = vec![foto("/solta.nef")];

        let pastas = pastas_do_acervo(&fotos);

        assert_eq!(pastas[0].caminho, "/");
        assert_eq!(pastas[0].nome, "/", "a raiz não pode aparecer sem nome");
    }

    #[test]
    fn caminho_sem_separador_nenhum_e_ignorado() {
        // Não deveria existir, mas o banco é compartilhado com versões antigas
        // do app. Uma linha estranha não pode virar uma pasta sem nome no meio
        // da árvore.
        let fotos = vec![foto("arquivo-solto.nef")];

        assert!(pastas_do_acervo(&fotos).is_empty());
    }

    #[test]
    fn a_contagem_nao_soma_subpastas() {
        // Somar as filhas faria o número da mãe discordar do que a grade mostra
        // ao clicar nela — o pior tipo de número, o que parece certo.
        let fotos = vec![
            foto("/fotos/a.nef"),
            foto("/fotos/2024/b.nef"),
            foto("/fotos/2024/c.nef"),
        ];

        let pastas = pastas_do_acervo(&fotos);
        let raiz = pastas.iter().find(|p| p.caminho == "/fotos").unwrap();

        assert_eq!(raiz.quantas, 1);
    }

    #[test]
    fn escolher_uma_pasta_nao_traz_a_de_nome_parecido() {
        // Com `starts_with`, `/fotos/2024` traria `/fotos/2024-descartes` —
        // que é justamente a pasta que ninguém quer ver junto.
        let dentro = foto("/fotos/2024/a.nef");
        let vizinha = foto("/fotos/2024-descartes/b.nef");

        assert!(na_pasta(&dentro, "/fotos/2024"));
        assert!(!na_pasta(&vizinha, "/fotos/2024"));
    }
}
