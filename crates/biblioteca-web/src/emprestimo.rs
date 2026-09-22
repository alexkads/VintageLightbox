//! Quem empresta a imagem a um tile enquanto a dele não chega.
//!
//! 🚨 **É o que impede o tile vazio quando a foto sobe** (dono, 21/set/2026:
//! *"a foto fica preta e depois aparece novamente"*). Ao subir, a foto troca
//! `local:…` pelo id do servidor, e a miniatura troca de URL; sem empréstimo, o
//! tile mostrava só o fundo até o `fetch` e a decodificação da nova terminarem.
//!
//! 🔑 **A ponte é o nome do arquivo, e não a posição.** A primeira versão
//! herdava de quem estava no mesmo lugar, e rodando o app de desktop contra a
//! pilha local (40 fotos subindo, a janela fotografada a cada 400 ms) isso não
//! pegou: a do servidor entra pela `ordem` dele e as locais andam uma casa —
//! quem saiu do lugar não é quem entrou nele. O nome é o que as duas têm em
//! comum; só a que **saiu da lista** empresta, e duas fotos de mesmo nome à
//! vista ao mesmo tempo não trocam de imagem.
//!
//! Fora do `#[cfg(wasm32)]` de propósito: é conta pura, e é aqui que ela se
//! prova sem navegador. A grade só troca o id devolvido pela textura.

use std::collections::{HashMap, HashSet};

/// A base do nome do arquivo, sem extensão e sem caixa: `DSC_2700.JPG`,
/// `DSC_2700.NEF` e `DSC_2700.jpg` dão o mesmo `dsc_2700`.
///
/// 🚨 **Comparar o nome inteiro não pega a foto que sobe** (dono, 21/set/2026:
/// o pisca voltou com as fotos da câmera dele). A subida troca a extensão por
/// `.jpg` (`nome_para_o_site`): a local `DSC_2700.JPG` vira `DSC_2700.jpg` no
/// site. O roteiro de teste usava `IMG_0001.jpg`, já minúsculo, e passava.
pub fn base_do_nome(arquivo: &str) -> String {
    let arquivo = arquivo.rsplit(['/', '\\']).next().unwrap_or(arquivo);
    arquivo
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(arquivo)
        .to_lowercase()
}

/// Um tile da lista nova.
pub struct Tile<'a> {
    pub id: &'a str,
    pub url: &'a str,
    pub arquivo: Option<&'a str>,
}

/// Para cada tile de `agora` sem textura, o id **de antes** cuja imagem ele
/// segura: o dele mesmo, se a foto já estava na lista (a revelação salva
/// troca o `?v=`); senão, o da foto de mesmo arquivo que sumiu da lista — a
/// local que subiu e virou esta.
pub fn quem_empresta(
    antes: &HashMap<String, String>,
    arquivos_antes: &HashMap<String, String>,
    agora: &[Tile],
    tem_textura: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let ids_de_agora: HashSet<&str> = agora.iter().map(|t| t.id).collect();
    // Os que saíram, pelo nome — o mesmo nome duas vezes não empresta.
    let mut saiu_por_arquivo: HashMap<String, Option<&str>> = HashMap::new();
    for (id, arquivo) in arquivos_antes {
        if !ids_de_agora.contains(id.as_str()) {
            saiu_por_arquivo
                .entry(base_do_nome(arquivo))
                .and_modify(|v| *v = None)
                .or_insert(Some(id.as_str()));
        }
    }
    agora
        .iter()
        .filter(|t| !tem_textura(t.url))
        .filter_map(|t| {
            let de_quem = if antes.contains_key(t.id) {
                t.id.to_string()
            } else {
                saiu_por_arquivo
                    .get(&base_do_nome(t.arquivo?))
                    .copied()
                    .flatten()?
                    .to_string()
            };
            Some((t.id.to_string(), de_quem))
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

    fn tile<'a>(id: &'a str, url: &'a str, arquivo: &'a str) -> Tile<'a> {
        Tile {
            id,
            url,
            arquivo: Some(arquivo),
        }
    }

    /// 🚨 A do servidor entra **antes** da local e as posições andam: é o
    /// nome, e não o lugar, que diz de quem é a imagem.
    #[test]
    fn a_foto_que_sobe_segura_a_imagem_da_local_de_mesmo_nome() {
        let antes = mapa(&[("local:1", "blob:1"), ("local:2", "blob:2")]);
        let arquivos = mapa(&[("local:1", "A.jpg"), ("local:2", "B.jpg")]);
        let agora = [
            tile("s2", "/m/s2", "B.jpg"),
            tile("local:1", "blob:1", "A.jpg"),
        ];
        let tem = |url: &str| url.starts_with("blob:");
        assert_eq!(
            quem_empresta(&antes, &arquivos, &agora, tem),
            vec![("s2".to_string(), "local:2".to_string())]
        );
    }

    /// A mesma foto com URL nova segura a própria imagem de antes.
    #[test]
    fn a_url_nova_da_mesma_foto_segura_a_de_antes() {
        let antes = mapa(&[("s1", "/m/s1?v=1")]);
        let agora = [tile("s1", "/m/s1?v=2", "A.jpg")];
        assert_eq!(
            quem_empresta(&antes, &mapa(&[("s1", "A.jpg")]), &agora, |u| u
                == "/m/s1?v=1"),
            vec![("s1".to_string(), "s1".to_string())]
        );
    }

    /// 🚨 **Foto nova sem par que saiu não herda nada** — nem a vizinha, nem
    /// uma de mesmo nome que continua na lista.
    #[test]
    fn a_foto_nova_sem_par_nao_herda() {
        let antes = mapa(&[("s0", "/m/s0")]);
        let arquivos = mapa(&[("s0", "A.jpg")]);
        let agora = [
            tile("s0", "/m/s0", "A.jpg"),
            tile("nova", "/m/nova", "A.jpg"),
        ];
        assert!(quem_empresta(&antes, &arquivos, &agora, |u| u != "/m/nova").is_empty());
    }

    /// 🚨 A local da câmera é `.JPG` (ou `.NEF`) e a do servidor é `.jpg`: a
    /// extensão e a caixa não separam as duas.
    #[test]
    fn a_extensao_trocada_na_subida_nao_separa_as_duas() {
        let antes = mapa(&[("local:1", "blob:1")]);
        let arquivos = mapa(&[("local:1", "DSC_2700.JPG")]);
        let agora = [tile("s1", "/m/s1", "DSC_2700.jpg")];
        assert_eq!(
            quem_empresta(&antes, &arquivos, &agora, |_| false),
            vec![("s1".to_string(), "local:1".to_string())]
        );
        assert_eq!(base_do_nome("fotos/DSC_2700.NEF"), "dsc_2700");
    }

    /// Duas que saíram com o mesmo nome: nenhuma empresta, para não mostrar a
    /// foto errada.
    #[test]
    fn nome_repetido_entre_as_que_sairam_nao_empresta() {
        let antes = mapa(&[("local:1", "blob:1"), ("local:2", "blob:2")]);
        let arquivos = mapa(&[("local:1", "A.jpg"), ("local:2", "A.jpg")]);
        let agora = [tile("s1", "/m/s1", "A.jpg")];
        assert!(quem_empresta(&antes, &arquivos, &agora, |_| false).is_empty());
    }
}
