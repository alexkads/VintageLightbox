//! O que a Biblioteca mostra, dado o que foi filtrado.
//!
//! Fora do componente pelo mesmo motivo da grade: é a parte que um teste
//! alcança, e é onde o porte pode divergir do `crates/ui` sem aparecer.

use adapters::view_models::PhotoViewModel;

/// Nota mínima exigida.
///
/// **Mínima, e não exata** — é assim no `crates/ui` e no Lightroom: quem filtra
/// por 3 estrelas quer ver as de 4 e 5 também. Filtrar por igualdade esconderia
/// justamente as melhores fotos do acervo, e o sintoma seria "sumiram as que eu
/// mais gosto".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotaMinima(pub u8);

/// O sinalizador que se quer ver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroDeSinalizador {
    #[default]
    Qualquer,
    /// Só as marcadas como boas (`flag = 1`).
    Escolhidas,
    /// Só as rejeitadas (`flag = -1`).
    ///
    /// ⚠️ `-1`, e não `2`: a documentação de `Flag::as_code` afirmava `2` e o
    /// código sempre gravou `-1`. Um filtro escrito a partir do texto não
    /// acharia rejeitada nenhuma — sem erro, só uma lista vazia parecendo
    /// "não há".
    Rejeitadas,
    /// Só as que não têm sinalizador nenhum (`NULL` na coluna).
    SemSinalizador,
}

/// Tudo que a barra de filtros decide.
#[derive(Debug, Clone, Default)]
pub struct Filtros {
    pub nota_minima: NotaMinima,
    pub sinalizador: FiltroDeSinalizador,
    /// Cor exata, como o legado guarda (`"Red"`, `"Blue"`…). `None` é "qualquer".
    pub cor: Option<String>,
    /// Trecho do nome do arquivo, sem diferenciar maiúsculas.
    pub busca: String,
    /// Diretório escolhido na árvore lateral. `None` é "todas as pastas".
    pub pasta: Option<String>,
}

impl Filtros {
    /// Se nada foi escolhido, a grade mostra o acervo inteiro — e é isso que
    /// permite pular o trabalho de filtrar.
    pub fn vazio(&self) -> bool {
        self.nota_minima.0 == 0
            && self.sinalizador == FiltroDeSinalizador::Qualquer
            && self.cor.is_none()
            && self.busca.trim().is_empty()
            && self.pasta.is_none()
    }

    fn aceita(&self, foto: &PhotoViewModel) -> bool {
        if (foto.rating.max(0) as u8) < self.nota_minima.0 {
            return false;
        }

        let passa_sinalizador = match self.sinalizador {
            FiltroDeSinalizador::Qualquer => true,
            FiltroDeSinalizador::Escolhidas => foto.flag == Some(1),
            FiltroDeSinalizador::Rejeitadas => foto.flag == Some(-1),
            FiltroDeSinalizador::SemSinalizador => foto.flag.is_none(),
        };
        if !passa_sinalizador {
            return false;
        }

        if let Some(cor) = &self.cor {
            if foto.color_label.as_deref() != Some(cor.as_str()) {
                return false;
            }
        }

        if let Some(pasta) = &self.pasta {
            if !crate::biblioteca::pastas::na_pasta(foto, pasta) {
                return false;
            }
        }

        let termo = self.busca.trim().to_lowercase();
        if !termo.is_empty() && !foto.name.to_lowercase().contains(&termo) {
            return false;
        }

        true
    }
}

/// Os índices das fotos que passam — e **não** as fotos.
///
/// A grade guarda o acervo inteiro num `Arc<Vec<_>>` e o `uniform_list` pede
/// linhas por índice. Devolver `Vec<PhotoViewModel>` clonaria o acervo a cada
/// digitação na busca; devolver índices custa 8 bytes por foto e não toca no
/// original.
pub fn indices_visiveis(fotos: &[PhotoViewModel], filtros: &Filtros) -> Vec<usize> {
    if filtros.vazio() {
        return (0..fotos.len()).collect();
    }

    fotos
        .iter()
        .enumerate()
        .filter(|(_, foto)| filtros.aceita(foto))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foto(nome: &str, nota: i32, flag: Option<i32>, cor: Option<&str>) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            path: format!("/fotos/{nome}"),
            thumbnail_path: None,
            date: String::new(),
            camera: String::new(),
            exposure: String::new(),
            rating: nota,
            color_label: cor.map(|c| c.to_string()),
            flag,
            width: None,
            height: None,
            ..Default::default()
        }
    }

    fn acervo() -> Vec<PhotoViewModel> {
        vec![
            foto("DSC_001.NEF", 0, None, None),
            foto("DSC_002.NEF", 3, Some(1), Some("Red")),
            foto("DSC_003.NEF", 5, Some(-1), Some("Blue")),
            foto("retrato.jpg", 4, None, Some("Red")),
        ]
    }

    fn nomes(fotos: &[PhotoViewModel], filtros: &Filtros) -> Vec<String> {
        indices_visiveis(fotos, filtros)
            .into_iter()
            .map(|i| fotos[i].name.clone())
            .collect()
    }

    #[test]
    fn sem_filtro_mostra_o_acervo_inteiro() {
        let fotos = acervo();
        assert_eq!(indices_visiveis(&fotos, &Filtros::default()).len(), 4);
    }

    #[test]
    fn a_nota_e_minima_e_nao_exata() {
        // Quem filtra por 3 estrelas quer ver as de 4 e 5 também. Por
        // igualdade, as melhores fotos do acervo sumiriam — e o sintoma seria
        // "sumiram justo as que eu mais gosto".
        let fotos = acervo();
        let filtros = Filtros {
            nota_minima: NotaMinima(3),
            ..Default::default()
        };

        assert_eq!(
            nomes(&fotos, &filtros),
            vec!["DSC_002.NEF", "DSC_003.NEF", "retrato.jpg"]
        );
    }

    #[test]
    fn rejeitada_e_menos_um_e_nao_dois() {
        // A documentação de `Flag::as_code` afirmava `2`. Um filtro escrito a
        // partir dela devolveria lista vazia — sem erro, parecendo "não há
        // nenhuma rejeitada".
        let fotos = acervo();
        let filtros = Filtros {
            sinalizador: FiltroDeSinalizador::Rejeitadas,
            ..Default::default()
        };

        assert_eq!(nomes(&fotos, &filtros), vec!["DSC_003.NEF"]);
    }

    #[test]
    fn escolhidas_e_sem_sinalizador_sao_conjuntos_diferentes() {
        let fotos = acervo();

        let escolhidas = Filtros {
            sinalizador: FiltroDeSinalizador::Escolhidas,
            ..Default::default()
        };
        assert_eq!(nomes(&fotos, &escolhidas), vec!["DSC_002.NEF"]);

        // Sem sinalizador é `NULL` na coluna — não é "nem escolhida nem
        // rejeitada por acaso", é a ausência explícita.
        let sem = Filtros {
            sinalizador: FiltroDeSinalizador::SemSinalizador,
            ..Default::default()
        };
        assert_eq!(nomes(&fotos, &sem), vec!["DSC_001.NEF", "retrato.jpg"]);
    }

    #[test]
    fn a_cor_e_exata() {
        let fotos = acervo();
        let filtros = Filtros {
            cor: Some("Red".to_string()),
            ..Default::default()
        };

        assert_eq!(nomes(&fotos, &filtros), vec!["DSC_002.NEF", "retrato.jpg"]);
    }

    #[test]
    fn a_busca_ignora_maiusculas_e_casa_por_trecho() {
        let fotos = acervo();
        let filtros = Filtros {
            busca: "RETRATO".to_string(),
            ..Default::default()
        };

        assert_eq!(nomes(&fotos, &filtros), vec!["retrato.jpg"]);
    }

    #[test]
    fn os_filtros_se_combinam() {
        let fotos = acervo();
        let filtros = Filtros {
            nota_minima: NotaMinima(4),
            cor: Some("Red".to_string()),
            ..Default::default()
        };

        // A de 5 estrelas é azul, e a vermelha de 3 não alcança a nota.
        assert_eq!(nomes(&fotos, &filtros), vec!["retrato.jpg"]);
    }

    #[test]
    fn nota_negativa_no_banco_conta_como_zero() {
        // `rating` é `i32` e a coluna é nullable — o adaptador manda `0` para
        // nulo, mas nada impede um `-1` gravado por outra versão. Sem o
        // `max(0)`, o `as u8` daria 255 e a foto passaria por **qualquer**
        // filtro de nota.
        let fotos = vec![foto("estranha.jpg", -1, None, None)];
        let filtros = Filtros {
            nota_minima: NotaMinima(1),
            ..Default::default()
        };

        assert!(indices_visiveis(&fotos, &filtros).is_empty());
    }

    #[test]
    fn os_indices_apontam_para_o_acervo_original() {
        // A grade indexa o `Arc<Vec<_>>` inteiro com o que sai daqui. Índice
        // deslocado mostraria a foto errada na célula — o defeito que não
        // parece defeito, porque a grade continua bonita.
        let fotos = acervo();
        let filtros = Filtros {
            cor: Some("Blue".to_string()),
            ..Default::default()
        };

        let indices = indices_visiveis(&fotos, &filtros);
        assert_eq!(indices, vec![2]);
        assert_eq!(fotos[indices[0]].name, "DSC_003.NEF");
    }
}
