//! Recuperar as fotos de um cartão formatado: o cartão visto como dispositivo.
//!
//! O cartão formatado aparece montado e **vazio** na lista de origens da
//! importação, porque ela lê a tabela de arquivos, e a tabela é justamente o
//! que a formatação zerou. Para recuperar, o cartão tem de ser lido como
//! dispositivo, setor por setor (`recuperacao-core`). Aqui mora o que ele é
//! para quem decide: qual dispositivo, que tamanho e onde está montado.

/// Um cartão visto como dispositivo, e não como pasta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartaoBruto {
    /// O que se abre para ler: `/dev/mmcblk0`, `/dev/rdisk4`,
    /// `\\.\PhysicalDrive2`.
    pub dispositivo: String,
    /// O que o operador lê na tela: o modelo do leitor ou o nome do volume.
    pub nome: String,
    /// Em bytes. Zero é leitor sem cartão.
    pub tamanho: u64,
    /// Onde as partições dele estão montadas agora (`/media/ana/EOS_DIGITAL`,
    /// `E:\`). Vazio quando nada está montado.
    pub montagens: Vec<String>,
}

/// Quem lista os cartões plugados. Uma implementação por sistema, no
/// `infrastructure`.
pub trait CartoesBrutos: Send + Sync {
    /// Os dispositivos removíveis, sem filtro de regra de negócio.
    fn listar(&self) -> Vec<CartaoBruto>;
    /// Onde o sistema está montado (`/`, `/boot`, `C:\`). Um dispositivo com
    /// alguma montagem aqui nunca é oferecido.
    fn raizes_do_sistema(&self) -> Vec<String>;
}

/// O nome da n-ésima foto recuperada (a partir de 1), na ordem do cartão.
///
/// 🔑 A ordem do cartão é a ordem em que a câmera gravou, e o nome guarda essa
/// ordem: `recuperada-00001.cr2` vem antes de `recuperada-00002.jpg`. O nome
/// original morreu com a tabela; a data volta pelo EXIF na importação.
pub fn nome_da_recuperada(n: usize, extensao: &str) -> String {
    format!("recuperada-{n:05}.{extensao}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_nome_guarda_a_ordem() {
        assert_eq!(nome_da_recuperada(1, "cr2"), "recuperada-00001.cr2");
        assert!(nome_da_recuperada(9, "jpg") < nome_da_recuperada(10, "jpg"));
    }
}
