//! Para onde a foto vai: a conta que responde antes de o botão ser apertado.
//!
//! Modo, destino, organização e renomeação são quatro escolhas abstratas. Juntas
//! elas decidem um caminho — e **mostrar esse caminho** é o que transforma quatro
//! combos numa decisão conferível. É o que o legado faz ("A primeira foto vai
//! para…"), e é a parte que mais vale portar da tela dele.
//!
//! ⚠️ **A conta aqui é uma previsão, não a execução.** Quem organiza de verdade é
//! o `FileOrganizerImpl`, na `infrastructure`, e ele é quem manda. Esta função
//! existe para a tela poder dizer o que vai acontecer; se as duas divergirem, é
//! esta que está errada.

use domain::value_objects::{ImportMode, OrganizationStrategy, RenamePattern};

use super::estado::{Candidato, Estado};

/// O caminho previsto da primeira foto marcada, relativo ao destino.
///
/// `None` quando não há foto marcada ou quando o modo é `Add` — que não move
/// nada, e portanto não tem destino a prever.
pub fn previa(estado: &Estado) -> Option<String> {
    if estado.opcoes.mode == ImportMode::Add {
        return None;
    }

    let candidato = estado.candidatos.iter().find(|c| c.marcado)?;
    let pasta = pasta_de(estado, candidato)?;
    let nome = nome_de(candidato, &estado.opcoes.rename_pattern);

    Some(format!("{pasta}{nome}"))
}

fn pasta_de(estado: &Estado, candidato: &Candidato) -> Option<String> {
    match estado.opcoes.organization {
        OrganizationStrategy::ByDate => Some(match partes_da_data(&candidato.data) {
            Some((ano, mes, dia)) => format!("{ano}/{mes}/{dia}/"),
            // ⚠️ Sem EXIF, o placeholder — e não uma data inventada. Uma pasta
            // "1970/01/01" pareceria informação verdadeira.
            None => "AAAA/MM/DD/".to_string(),
        }),
        OrganizationStrategy::PreserveStructure => {
            let raiz = estado.origem.as_deref()?;
            // 🚨 O corte é por **texto**, com os dois separadores, e não por
            // `Path::parent`: um caminho de cartão gravado no Windows chega com
            // `\`, e no macOS o `Path` não o reconhece — a hierarquia inteira
            // viraria uma pasta só. É a mesma armadilha que a árvore de pastas da
            // fase 1 encontrou.
            let relativo = candidato.caminho.strip_prefix(raiz)?;
            let sem_barra = relativo.trim_start_matches(['/', '\\']);
            let ate_a_pasta = sem_barra.rfind(['/', '\\']).map(|i| &sem_barra[..i]);

            Some(match ate_a_pasta {
                Some(pasta) if !pasta.is_empty() => format!("{}/", pasta.replace('\\', "/")),
                _ => String::new(),
            })
        }
        OrganizationStrategy::IntoOneFolder => Some(String::new()),
    }
}

fn nome_de(candidato: &Candidato, padrao: &RenamePattern) -> String {
    match padrao {
        RenamePattern::KeepOriginal => candidato.nome.clone(),
        _ => {
            let extensao = candidato
                .nome
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_lowercase())
                .unwrap_or_else(|| "jpg".to_string());

            match partes_da_data(&candidato.data) {
                Some((ano, mes, dia)) => format!("photo-{ano}-{mes}-{dia}-001.{extensao}"),
                None => format!("photo-AAAA-MM-DD-001.{extensao}"),
            }
        }
    }
}

/// Ano, mês e dia de uma data EXIF (`"YYYY:MM:DD HH:MM:SS"`).
fn partes_da_data(data: &str) -> Option<(&str, &str, &str)> {
    let dia_inteiro = data.split(' ').next()?;
    let mut partes = dia_inteiro.split(':');
    let (ano, mes, dia) = (partes.next()?, partes.next()?, partes.next()?);

    // Uma data pela metade não vira pasta: `"2026:08"` daria `2026/08//`, com uma
    // barra dupla que só apareceria no disco.
    (!ano.is_empty() && !mes.is_empty() && !dia.is_empty()).then_some((ano, mes, dia))
}

#[cfg(test)]
mod testes {
    use super::*;

    use super::super::estado::Candidato;

    fn estado_com(caminho: &str, data: &str) -> Estado {
        let mut candidato = Candidato::do_caminho(caminho.to_string());
        candidato.data = data.to_string();

        Estado {
            origem: Some("/cartao".into()),
            candidatos: vec![candidato],
            ..Default::default()
        }
    }

    /// O padrão: copiar, por data, renomeando.
    #[test]
    fn o_padrao_organiza_por_data_e_renomeia() {
        let estado = estado_com("/cartao/DSC_0001.NEF", "2026:08:16 10:00:00");

        assert_eq!(
            previa(&estado).as_deref(),
            Some("2026/08/16/photo-2026-08-16-001.nef")
        );
    }

    /// 🚨 `Add` não tem destino — ele cataloga onde está.
    ///
    /// Mostrar um caminho previsto no modo que não move arquivo nenhum seria
    /// prometer uma cópia que não vai acontecer.
    #[test]
    fn o_modo_add_nao_preve_destino() {
        let mut estado = estado_com("/cartao/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.opcoes.mode = ImportMode::Add;

        assert_eq!(previa(&estado), None);
    }

    /// Sem foto marcada não há o que prever.
    #[test]
    fn sem_marcacao_nao_ha_previa() {
        let mut estado = estado_com("/cartao/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.marcar_todos(false);

        assert_eq!(previa(&estado), None);
    }

    /// ⚠️ Sem EXIF, o placeholder — e não uma data inventada.
    ///
    /// Uma pasta `1970/01/01` pareceria informação verdadeira, e o fotógrafo só
    /// descobriria o contrário depois de a importação terminar.
    #[test]
    fn foto_sem_data_mostra_o_lugar_da_data() {
        let estado = estado_com("/cartao/DSC_0001.NEF", "");

        assert_eq!(
            previa(&estado).as_deref(),
            Some("AAAA/MM/DD/photo-AAAA-MM-DD-001.nef")
        );
    }

    /// Data pela metade também não vira pasta.
    #[test]
    fn data_incompleta_nao_vira_barra_dupla() {
        let estado = estado_com("/cartao/a.jpg", "2026:08");

        let previsto = previa(&estado).expect("há previsão");
        assert!(!previsto.contains("//"), "{previsto}");
    }

    /// Manter o nome original preserva maiúsculas e extensão.
    #[test]
    fn manter_o_nome_original_preserva_o_arquivo() {
        let mut estado = estado_com("/cartao/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.opcoes.rename_pattern = RenamePattern::KeepOriginal;

        assert_eq!(previa(&estado).as_deref(), Some("2026/08/16/DSC_0001.NEF"));
    }

    /// Numa pasta só: nada de hierarquia.
    #[test]
    fn numa_pasta_so_nao_cria_subpasta() {
        let mut estado = estado_com("/cartao/DCIM/100ND/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.opcoes.organization = OrganizationStrategy::IntoOneFolder;

        assert_eq!(previa(&estado).as_deref(), Some("photo-2026-08-16-001.nef"));
    }

    /// Preservar estrutura repete as subpastas da origem.
    #[test]
    fn preservar_estrutura_repete_as_subpastas() {
        let mut estado = estado_com("/cartao/DCIM/100ND/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.opcoes.organization = OrganizationStrategy::PreserveStructure;
        estado.opcoes.rename_pattern = RenamePattern::KeepOriginal;

        assert_eq!(previa(&estado).as_deref(), Some("DCIM/100ND/DSC_0001.NEF"));
    }

    /// Arquivo na raiz da origem não ganha subpasta.
    #[test]
    fn arquivo_na_raiz_nao_ganha_subpasta() {
        let mut estado = estado_com("/cartao/DSC_0001.NEF", "2026:08:16 10:00:00");
        estado.opcoes.organization = OrganizationStrategy::PreserveStructure;
        estado.opcoes.rename_pattern = RenamePattern::KeepOriginal;

        assert_eq!(previa(&estado).as_deref(), Some("DSC_0001.NEF"));
    }

    /// 🚨 Caminho gravado com `\` (cartão formatado no Windows) preserva a
    /// hierarquia.
    ///
    /// `Path::parent` no macOS não reconhece `\`, e a hierarquia inteira viraria
    /// uma pasta só — a mesma armadilha que a árvore de pastas da fase 1
    /// encontrou, do outro lado do app.
    #[test]
    fn caminho_com_barra_invertida_preserva_a_hierarquia() {
        let mut candidato = Candidato::do_caminho("C:\\Fotos\\2024\\DSC_1.NEF".to_string());
        candidato.data = "2026:08:16 10:00:00".into();

        let estado = Estado {
            origem: Some("C:\\Fotos".into()),
            candidatos: vec![candidato],
            opcoes: domain::value_objects::ImportOptions {
                organization: OrganizationStrategy::PreserveStructure,
                rename_pattern: RenamePattern::KeepOriginal,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(previa(&estado).as_deref(), Some("2024/DSC_1.NEF"));
    }

    /// Foto fora da origem declarada não tem estrutura a preservar.
    #[test]
    fn foto_fora_da_origem_nao_preve_nada() {
        let mut estado = estado_com("/outro/lugar/a.NEF", "2026:08:16 10:00:00");
        estado.opcoes.organization = OrganizationStrategy::PreserveStructure;

        assert_eq!(previa(&estado), None);
    }
}
