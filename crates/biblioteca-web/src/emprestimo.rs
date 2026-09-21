//! Quem empresta a imagem a um tile enquanto a dele não chega.
//!
//! 🚨 **É o que impede o tile vazio quando a foto sobe** (dono, 21/set/2026:
//! *"a foto fica preta e depois aparece novamente"*). Ao subir, a foto troca
//! `local:…` pelo id do servidor, e a miniatura troca de URL; sem empréstimo, o
//! tile mostrava só o fundo até o `fetch` e a decodificação da nova terminarem.
//!
//! Fora do `#[cfg(wasm32)]` de propósito: é conta pura, e é aqui que ela se
//! prova sem navegador. A grade só troca o id devolvido pela textura.

use std::collections::HashMap;

/// Para cada tile de `agora` (id, URL) sem textura, o id **de antes** cuja
/// imagem ele segura: o dele mesmo, se a foto já estava na lista (a revelação
/// salva troca o `?v=`); senão, o de quem estava na mesma posição e sumiu — a
/// local que subiu e virou esta. A ordem da grade é a da fotografia, e a do
/// servidor chega no lugar da local.
pub fn quem_empresta(
    antes: &HashMap<String, String>,
    antes_por_posicao: &[String],
    agora: &[(&str, &str)],
    tem_textura: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let ids_de_agora: std::collections::HashSet<&str> = agora.iter().map(|(id, _)| *id).collect();
    agora
        .iter()
        .enumerate()
        .filter(|(_, (_, url))| !tem_textura(url))
        .filter_map(|(n, (id, _))| {
            let de_quem = if antes.contains_key(*id) {
                id.to_string()
            } else {
                antes_por_posicao
                    .get(n)
                    .filter(|velho| !ids_de_agora.contains(velho.as_str()))?
                    .clone()
            };
            Some((id.to_string(), de_quem))
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn mapa(pares: &[(&str, &str)]) -> HashMap<String, String> {
        pares
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    /// A local que subiu vira a do servidor no mesmo lugar: esta segura a
    /// imagem daquela. Quem não mudou não pede nada.
    #[test]
    fn a_foto_que_sobe_segura_a_imagem_da_local() {
        let antes = mapa(&[("s0", "/m/s0"), ("local:1", "blob:1")]);
        let posicoes = vec!["s0".to_string(), "local:1".to_string()];
        let agora = [("s0", "/m/s0"), ("s1", "/m/s1")];
        let tem = |url: &str| url == "/m/s0" || url == "blob:1";
        assert_eq!(
            quem_empresta(&antes, &posicoes, &agora, tem),
            vec![("s1".to_string(), "local:1".to_string())]
        );
    }

    /// A mesma foto com URL nova segura a própria imagem de antes.
    #[test]
    fn a_url_nova_da_mesma_foto_segura_a_de_antes() {
        let antes = mapa(&[("s1", "/m/s1?v=1")]);
        let agora = [("s1", "/m/s1?v=2")];
        assert_eq!(
            quem_empresta(&antes, &["s1".to_string()], &agora, |u| u == "/m/s1?v=1"),
            vec![("s1".to_string(), "s1".to_string())]
        );
    }

    /// 🚨 **Foto nova num lugar de quem continua na lista não herda nada** —
    /// uma importação que entra no meio empurra as outras, e herdar a vizinha
    /// mostraria a foto errada até a certa chegar.
    #[test]
    fn a_foto_que_so_empurrou_as_outras_nao_herda_a_vizinha() {
        let antes = mapa(&[("s0", "/m/s0"), ("s1", "/m/s1")]);
        let posicoes = vec!["s0".to_string(), "s1".to_string()];
        let agora = [("s0", "/m/s0"), ("nova", "/m/nova"), ("s1", "/m/s1")];
        let tem = |url: &str| url != "/m/nova";
        assert!(quem_empresta(&antes, &posicoes, &agora, tem).is_empty());
    }
}
