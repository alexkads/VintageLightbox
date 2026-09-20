//! `/dashboard/backup` — o acervo de arquivos no R2.
//!
//! O porte do `dashboard.file-manager` do legado (`paridade-paginas.tsv`, linha
//! `falta-storage`), com o nome que o dono pediu e com o que lá não havia:
//! **arrastar pasta**.
//!
//! 🚨 **Tudo cai sob `arquivos/` no bucket das fotos**, e isso é trava do
//! backend, não escolha daqui (dono, 2026-09-18: *"não quero cair nessa regra
//! da limpeza"* — o bucket de backup apaga o que tem sete dias). Ver
//! `application::arquivos` no backend.
//!
//! A tela é a mesma da web, gesto por gesto (FLUXO_UNICO_DAS_TRES_INTERFACES: a
//! web é a referência). O que muda é o ambiente: aqui a pasta arrastada chega
//! como caminho de disco, e a conversão para WebP é feita pelo `image` em vez do
//! `canvas`.

pub mod arrastar;
pub mod compactar;
pub mod conversao;
pub mod escolha;
pub mod fila;
pub mod porta;
pub mod tela;

pub use escolha::{EscolhaDoBackup, EscolhaNativa};
pub use porta::{Acervo, AcervoHttp};
pub use tela::Backup;
