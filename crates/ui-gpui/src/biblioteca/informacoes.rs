//! O que o acervo tem dentro: a contagem por nota e as câmeras mais usadas.
//!
//! É a metade sem tela do painel de informações — o `Metadata` do dock do legado
//! (`dock_viewer.rs`, `metadata_charts.rs`). Contar é conta, e conta se confere
//! sem abrir janela.
//!
//! ## ⚠️ Isto lê o acervo inteiro, e não a lista filtrada
//!
//! É o que o legado faz (`MetadataCharts::show(ui, &self.context.state.photos)`),
//! e é o que faz sentido: a distribuição por nota existe para responder "como
//! está o acervo", não "como está o filtro". Filtrando por ★★★★, um gráfico
//! sobre o filtrado responderia sempre a mesma coisa — uma barra só.

use adapters::view_models::PhotoViewModel;

/// Quantas câmeras entram na lista. As mesmas cinco do legado.
const CAMERAS_LISTADAS: usize = 5;

/// O retrato do acervo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Estatisticas {
    /// Quantas fotos em cada nota, de 0 a 5.
    pub por_nota: [usize; 6],
    pub total: usize,
    /// As câmeras mais usadas, da mais para a menos.
    pub cameras: Vec<(String, usize)>,
}

pub fn estatisticas(fotos: &[PhotoViewModel]) -> Estatisticas {
    let mut por_nota = [0usize; 6];
    let mut contagem: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for foto in fotos {
        // A nota vem do banco como `i32` e nada impede um 7 ou um -1 ali. O
        // legado também prende (`clamp(0, 5)`), e sem isso a contagem sairia do
        // vetor — o único jeito de este módulo derrubar o app.
        por_nota[foto.rating.clamp(0, 5) as usize] += 1;

        // "Unknown" é o que o extrator de EXIF grava quando não achou a câmera,
        // e contá-lo faria "Unknown" ser a câmera mais usada de quase todo
        // acervo — uma estatística que responde sobre o extrator, não sobre o
        // fotógrafo. O legado exclui os dois casos.
        if !foto.camera.is_empty() && foto.camera != "Unknown" {
            *contagem.entry(foto.camera.as_str()).or_insert(0) += 1;
        }
    }

    let mut cameras: Vec<(String, usize)> = contagem
        .into_iter()
        .map(|(nome, quantas)| (nome.to_string(), quantas))
        .collect();

    // 🔑 **Desempate por nome, e o legado não tem.** Lá a ordenação é só por
    // contagem, sobre um `HashMap`: com duas câmeras empatadas, qual delas
    // aparece — e qual fica de fora das cinco — muda a cada execução. Um painel
    // de estatística que troca de resposta sem o acervo mudar não é conferível.
    cameras.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    cameras.truncate(CAMERAS_LISTADAS);

    Estatisticas {
        por_nota,
        total: fotos.len(),
        cameras,
    }
}

/// A nota em estrelas, como o resto do app a mostra.
pub fn estrelas(nota: i32) -> String {
    "★".repeat(nota.clamp(0, 5) as usize)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto(camera: &str, nota: i32) -> PhotoViewModel {
        PhotoViewModel {
            camera: camera.to_string(),
            rating: nota,
            ..Default::default()
        }
    }

    #[test]
    fn a_distribuicao_conta_cada_nota() {
        let acervo = vec![
            foto("Nikon Z6", 5),
            foto("Nikon Z6", 5),
            foto("Nikon Z6", 3),
            foto("Canon R6", 0),
        ];

        let estatisticas = estatisticas(&acervo);

        assert_eq!(estatisticas.por_nota, [1, 0, 0, 1, 0, 2]);
        assert_eq!(estatisticas.total, 4);
    }

    /// 🚨 Nota fora da faixa não estoura o vetor.
    ///
    /// O campo é `i32` e vem do banco: um `7` ali — ou um `-1` — indexaria fora
    /// dos seis lugares. É a única forma de este módulo derrubar o app, e ela
    /// não passa por nenhuma tela.
    #[test]
    fn nota_fora_da_faixa_e_presa_em_vez_de_estourar() {
        let acervo = vec![foto("Nikon Z6", 9), foto("Nikon Z6", -3)];

        let estatisticas = estatisticas(&acervo);

        assert_eq!(estatisticas.por_nota[5], 1);
        assert_eq!(estatisticas.por_nota[0], 1);
    }

    /// "Unknown" e vazio não são câmeras.
    ///
    /// É o que o extrator grava quando não achou nada, e contá-lo faria
    /// "Unknown" ser a câmera mais usada de quase todo acervo — uma resposta
    /// sobre o extrator, e não sobre quem fotografou.
    #[test]
    fn camera_desconhecida_fica_de_fora() {
        let acervo = vec![foto("Unknown", 0), foto("", 0), foto("Leica M11", 0)];

        let estatisticas = estatisticas(&acervo);

        assert_eq!(estatisticas.cameras, vec![("Leica M11".to_string(), 1)]);
    }

    /// 🚨 Empate na contagem sai sempre na mesma ordem.
    ///
    /// No legado a ordenação é só por contagem, sobre um `HashMap`: duas câmeras
    /// com o mesmo número de fotos trocam de lugar entre execuções, e a sexta
    /// colocada entra ou não na lista de cinco por sorteio. Um painel de
    /// estatística que muda de resposta sem o acervo mudar não é conferível.
    #[test]
    fn o_empate_e_desempatado_pelo_nome() {
        let acervo = vec![foto("Sony A7", 0), foto("Canon R6", 0), foto("Nikon Z6", 0)];

        // Três execuções: com `HashMap` sem desempate, a ordem varia entre elas.
        for _ in 0..3 {
            let estatisticas = estatisticas(&acervo);
            let nomes: Vec<&str> = estatisticas
                .cameras
                .iter()
                .map(|(nome, _)| nome.as_str())
                .collect();
            assert_eq!(nomes, vec!["Canon R6", "Nikon Z6", "Sony A7"]);
        }
    }

    /// A lista tem teto de cinco, como a do legado.
    #[test]
    fn a_lista_de_cameras_para_em_cinco() {
        let acervo: Vec<PhotoViewModel> = (0..8).map(|i| foto(&format!("Camera {i}"), 0)).collect();

        assert_eq!(estatisticas(&acervo).cameras.len(), 5);
    }

    /// Acervo vazio responde vazio, e não um `None` que a tela teria de tratar.
    #[test]
    fn acervo_vazio_nao_tem_nada_a_dizer() {
        let estatisticas = estatisticas(&[]);

        assert_eq!(estatisticas.total, 0);
        assert_eq!(estatisticas.por_nota, [0; 6]);
        assert!(estatisticas.cameras.is_empty());
    }

    #[test]
    fn a_nota_vira_estrelas() {
        assert_eq!(estrelas(0), "");
        assert_eq!(estrelas(3), "★★★");
        assert_eq!(estrelas(9), "★★★★★", "e nota fora da faixa não vira dez");
    }
}
