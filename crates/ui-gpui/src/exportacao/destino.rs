//! Que nome cada foto exportada recebe — e por que dois arquivos nunca colidem.
//!
//! Sem tela e sem disco: é só a regra, e é ela que erra em silêncio.
//!
//! ## 🚨 A armadilha que este módulo existe para evitar
//!
//! Exportar 40 fotos para uma pasta é 40 nomes escolhidos, e o nome sai do
//! arquivo de origem. Duas fotos de cartões diferentes chamadas `DSC_0001.NEF`
//! viram `DSC_0001.jpg` **duas vezes**: a segunda grava por cima da primeira, o
//! rodapé diz "40 exportadas", e a pasta tem 39.
//!
//! 🔑 **Este defeito já aconteceu neste repositório, na importação** (`0bcac7e`:
//! *"importar perdia foto — o nome do destino era escolhido sem reserva"*).
//! Escolher o nome consultando o disco não basta: entre a consulta e a gravação
//! cabe outra foto do mesmo lote. O nome tem de ser **reservado** ao ser
//! escolhido, e é por isso que [`Destinos`] guarda estado em vez de ser uma
//! função pura.

use std::collections::HashSet;
use std::path::PathBuf;

/// Reserva de nomes para um lote de exportação.
pub struct Destinos {
    pasta: PathBuf,
    /// Os nomes já entregues neste lote. **A reserva é o ponto do tipo**: sem
    /// ela, duas fotos com o mesmo nome de origem recebem o mesmo destino e uma
    /// sobrescreve a outra sem erro nenhum.
    tomados: HashSet<String>,
}

impl Destinos {
    pub fn na_pasta(pasta: impl Into<PathBuf>) -> Self {
        Self {
            pasta: pasta.into(),
            tomados: HashSet::new(),
        }
    }

    /// O caminho de saída para uma foto, reservando o nome.
    ///
    /// A extensão é a do formato de saída, e não a da origem: exportar um `.NEF`
    /// produz um `.jpg`, que é o ponto de exportar.
    pub fn para(&mut self, origem: &str, extensao: &str) -> PathBuf {
        let base = base_do_caminho(origem);

        let mut nome = format!("{base}.{extensao}");
        let mut n = 2;
        while !self.tomados.insert(nome.to_lowercase()) {
            // `-2`, `-3`… é a convenção do próprio Lightroom para colisão dentro
            // de um lote.
            nome = format!("{base}-{n}.{extensao}");
            n += 1;
        }

        self.pasta.join(nome)
    }
}

/// O nome do arquivo, sem pasta e sem extensão.
///
/// ⚠️ **O corte é por texto, aceitando `/` e `\`.** `Path::file_name` no macOS
/// não reconhece `\`, e um caminho gravado a partir de um cartão formatado no
/// Windows (`C:\Fotos\2024\DSC_1.NEF`) viraria um "nome de arquivo" com o
/// caminho inteiro dentro. É a mesma armadilha que a árvore de pastas e a prévia
/// da importação já encontraram, do outro lado do app.
fn base_do_caminho(caminho: &str) -> String {
    let arquivo = caminho.rsplit(['/', '\\']).next().unwrap_or(caminho).trim();

    let sem_extensao = match arquivo.rsplit_once('.') {
        // Um arquivo chamado `.perfil` não tem extensão: tem nome começando com
        // ponto. Cortar ali deixaria o nome vazio.
        Some((base, _)) if !base.is_empty() => base,
        _ => arquivo,
    };

    if sem_extensao.is_empty() {
        "foto".to_string()
    } else {
        sem_extensao.to_string()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::Path;

    #[test]
    fn o_nome_vem_da_origem_com_a_extensao_do_formato() {
        let mut destinos = Destinos::na_pasta("/saida");
        assert_eq!(
            destinos.para("/cartao/DSC_0001.NEF", "jpg"),
            Path::new("/saida/DSC_0001.jpg")
        );
    }

    /// 🚨 Duas fotos com o mesmo nome de origem **não** podem virar o mesmo
    /// arquivo.
    ///
    /// É o defeito que a importação teve (`0bcac7e`): o rodapé conta 40 e a
    /// pasta tem 39. Nada falha — a segunda gravação sobrescreve a primeira sem
    /// erro, e a foto perdida só aparece se alguém conferir a pasta na mão.
    #[test]
    fn duas_fotos_com_o_mesmo_nome_nao_colidem() {
        let mut destinos = Destinos::na_pasta("/saida");

        assert_eq!(
            destinos.para("/cartao-a/DSC_0001.NEF", "jpg"),
            Path::new("/saida/DSC_0001.jpg")
        );
        assert_eq!(
            destinos.para("/cartao-b/DSC_0001.NEF", "jpg"),
            Path::new("/saida/DSC_0001-2.jpg")
        );
        assert_eq!(
            destinos.para("/cartao-c/DSC_0001.CR2", "jpg"),
            Path::new("/saida/DSC_0001-3.jpg")
        );
    }

    /// ⚠️ A colisão é conferida **sem diferenciar maiúscula de minúscula**.
    ///
    /// O disco do macOS não diferencia por padrão: `DSC_1.jpg` e `dsc_1.jpg` são
    /// o mesmo arquivo lá, e uma reserva que os tratasse como dois nomes
    /// deixaria a colisão passar — no sistema de arquivos em que o app roda.
    #[test]
    fn a_reserva_ignora_caixa_porque_o_disco_ignora() {
        let mut destinos = Destinos::na_pasta("/saida");

        destinos.para("/a/Retrato.jpg", "jpg");
        assert_eq!(
            destinos.para("/b/retrato.jpg", "jpg"),
            Path::new("/saida/retrato-2.jpg")
        );
    }

    /// ⚠️ `Path::parent`/`file_name` não servem para caminho vindo do banco.
    ///
    /// No macOS a barra invertida não é separador, então um caminho de cartão
    /// formatado no Windows viraria um nome de arquivo com o caminho inteiro
    /// dentro — e o arquivo exportado se chamaria `C:\Fotos\2024\DSC_1.jpg`.
    #[test]
    fn caminho_do_windows_tambem_e_cortado() {
        let mut destinos = Destinos::na_pasta("/saida");
        assert_eq!(
            destinos.para(r"C:\Fotos\2024\DSC_1.NEF", "jpg"),
            Path::new("/saida/DSC_1.jpg")
        );
    }

    /// Um nome que começa com ponto não tem extensão — tem nome.
    #[test]
    fn arquivo_oculto_nao_perde_o_nome() {
        let mut destinos = Destinos::na_pasta("/saida");
        assert_eq!(
            destinos.para("/a/.perfil", "jpg"),
            Path::new("/saida/.perfil.jpg")
        );
    }

    /// Origem sem nenhum nome aproveitável ainda produz um arquivo válido —
    /// gravar em `/saida/.jpg` seria criar um arquivo oculto sem nome.
    #[test]
    fn origem_sem_nome_ainda_da_um_arquivo() {
        let mut destinos = Destinos::na_pasta("/saida");
        assert_eq!(destinos.para("/a/", "jpg"), Path::new("/saida/foto.jpg"));
    }
}
