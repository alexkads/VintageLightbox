//! As regras de recuperar um cartão formatado.
//!
//! Duas, e as duas protegem alguma coisa que não tem volta:
//!
//! 1. **O disco do sistema nunca é oferecido.** A lista vem de "o que é
//!    removível", e isso mente: um leitor de cartão interno às vezes se diz
//!    fixo, e um HD externo com o sistema se diz removível. O filtro final é
//!    pelo que está montado: um dispositivo com `/` ou `C:\` dentro não é
//!    cartão de câmera.
//! 2. **O destino nunca é o próprio cartão.** As fotos apagadas continuam nos
//!    setores livres do cartão. Gravar lá o que se recupera escreveria por
//!    cima do que ainda falta recuperar.

use std::path::Path;
use std::sync::Arc;

use domain::recuperacao::{CartaoBruto, CartoesBrutos};

/// O maior cartão SD que existe (SDXC/SDUC chegam a 2 TB). Acima disso é disco,
/// e não cartão de câmera.
pub const MAIOR_CARTAO: u64 = 2 * 1024 * 1024 * 1024 * 1024;

/// Por que um destino foi recusado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recusa {
    /// O destino fica dentro do cartão que se quer recuperar.
    DentroDoCartao,
    /// Não há destino.
    SemDestino,
}

impl Recusa {
    pub fn texto(&self) -> &'static str {
        match self {
            Recusa::DentroDoCartao => {
                "O destino está no próprio cartão. Gravar nele apagaria as fotos que \
                 ainda faltam recuperar: escolha uma pasta no computador."
            }
            Recusa::SemDestino => "Escolha a pasta onde as fotos recuperadas vão ficar.",
        }
    }
}

/// Os cartões que podem ser recuperados, na ordem da lista.
pub fn cartoes_recuperaveis(
    candidatos: Vec<CartaoBruto>,
    raizes_do_sistema: &[String],
) -> Vec<CartaoBruto> {
    candidatos
        .into_iter()
        .filter(|c| c.tamanho > 0 && c.tamanho <= MAIOR_CARTAO)
        .filter(|c| {
            !c.montagens
                .iter()
                .any(|m| raizes_do_sistema.iter().any(|r| mesmo_caminho(m, r)))
        })
        .collect()
}

/// Confere o destino contra o cartão.
///
/// `destino` deve chegar já resolvido (sem `..` nem link simbólico): quem
/// chama é quem tem acesso ao disco para isso.
pub fn conferir_destino(destino: &str, cartao: &CartaoBruto) -> Result<(), Recusa> {
    if destino.trim().is_empty() {
        return Err(Recusa::SemDestino);
    }
    let dentro = cartao.montagens.iter().any(|m| dentro_de(destino, m))
        || dentro_de(destino, &cartao.dispositivo);
    if dentro {
        return Err(Recusa::DentroDoCartao);
    }
    Ok(())
}

/// Letra de unidade do Windows não diferencia maiúscula: `e:\` é `E:\`.
fn normalizar(caminho: &str) -> String {
    let b = caminho.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        caminho.replace('/', "\\").to_ascii_lowercase()
    } else {
        caminho.to_string()
    }
}

fn mesmo_caminho(a: &str, b: &str) -> bool {
    let (a, b) = (normalizar(a), normalizar(b));
    Path::new(&a) == Path::new(&b)
}

fn dentro_de(caminho: &str, raiz: &str) -> bool {
    let (c, r) = (normalizar(caminho), normalizar(raiz));
    if r.is_empty() {
        return false;
    }
    // `Path::starts_with` compara por componente: `/media/ana/CARTAO2` não está
    // dentro de `/media/ana/CARTAO`. No Linux ele não entende `E:\`, então a
    // letra de unidade é comparada como texto.
    if r.len() >= 2 && r.as_bytes()[1] == b':' {
        let r = r.trim_end_matches('\\');
        return c == r || c.starts_with(&format!("{r}\\"));
    }
    Path::new(&c).starts_with(Path::new(&r))
}

/// A lista de cartões que a tela oferece.
pub struct ListarCartoesUseCase {
    cartoes: Arc<dyn CartoesBrutos>,
}

impl ListarCartoesUseCase {
    pub fn new(cartoes: Arc<dyn CartoesBrutos>) -> Self {
        Self { cartoes }
    }

    pub fn execute(&self) -> Vec<CartaoBruto> {
        cartoes_recuperaveis(self.cartoes.listar(), &self.cartoes.raizes_do_sistema())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cartao(dispositivo: &str, tamanho: u64, montagens: &[&str]) -> CartaoBruto {
        CartaoBruto {
            dispositivo: dispositivo.into(),
            nome: "Cartão".into(),
            tamanho,
            montagens: montagens.iter().map(|m| m.to_string()).collect(),
        }
    }

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn o_disco_do_sistema_nunca_aparece() {
        let lista = vec![
            cartao("/dev/sda", 500 * GB, &["/", "/boot/efi"]),
            cartao("/dev/mmcblk0", 64 * GB, &["/media/ana/EOS_DIGITAL"]),
            cartao("\\\\.\\PhysicalDrive0", 256 * GB, &["C:\\"]),
        ];
        let raizes = vec!["/".to_string(), "c:\\".to_string()];
        let sobram = cartoes_recuperaveis(lista, &raizes);
        assert_eq!(sobram.len(), 1);
        assert_eq!(sobram[0].dispositivo, "/dev/mmcblk0");
    }

    #[test]
    fn leitor_vazio_e_disco_grande_ficam_de_fora() {
        let lista = vec![
            cartao("/dev/sdb", 0, &[]),
            cartao("/dev/sdc", 4 * 1024 * GB, &[]),
            cartao("/dev/sdd", 32 * GB, &[]),
        ];
        let sobram = cartoes_recuperaveis(lista, &[]);
        assert_eq!(sobram.len(), 1);
        assert_eq!(sobram[0].dispositivo, "/dev/sdd");
    }

    #[test]
    fn o_destino_no_proprio_cartao_e_recusado() {
        let c = cartao("/dev/mmcblk0", 64 * GB, &["/media/ana/EOS_DIGITAL"]);
        assert_eq!(
            conferir_destino("/media/ana/EOS_DIGITAL/recuperadas", &c),
            Err(Recusa::DentroDoCartao)
        );
        assert_eq!(
            conferir_destino("/media/ana/EOS_DIGITAL", &c),
            Err(Recusa::DentroDoCartao)
        );
        // Outro cartão com nome parecido não é este.
        assert_eq!(conferir_destino("/media/ana/EOS_DIGITAL2/x", &c), Ok(()));
        assert_eq!(conferir_destino("/home/ana/Recuperadas", &c), Ok(()));
    }

    #[test]
    fn no_windows_a_letra_nao_diferencia_maiuscula() {
        let c = cartao("\\\\.\\PhysicalDrive2", 64 * GB, &["E:\\"]);
        assert_eq!(
            conferir_destino("e:\\Fotos", &c),
            Err(Recusa::DentroDoCartao)
        );
        assert_eq!(conferir_destino("E:", &c), Err(Recusa::DentroDoCartao));
        assert_eq!(conferir_destino("C:\\Users\\ana\\Fotos", &c), Ok(()));
    }

    #[test]
    fn sem_destino_e_recusado() {
        let c = cartao("/dev/sdb", 64 * GB, &[]);
        assert_eq!(conferir_destino("  ", &c), Err(Recusa::SemDestino));
    }

    struct DeMentira;
    impl CartoesBrutos for DeMentira {
        fn listar(&self) -> Vec<CartaoBruto> {
            vec![
                cartao("/dev/sda", 500 * GB, &["/"]),
                cartao("/dev/sdb", 32 * GB, &[]),
            ]
        }
        fn raizes_do_sistema(&self) -> Vec<String> {
            vec!["/".into()]
        }
    }

    #[test]
    fn o_caso_de_uso_aplica_o_filtro() {
        let lista = ListarCartoesUseCase::new(Arc::new(DeMentira)).execute();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].dispositivo, "/dev/sdb");
    }
}
