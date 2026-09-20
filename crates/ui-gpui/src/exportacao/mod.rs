//! A exportação — o caminho da tela até o arquivo no disco.
//!
//! 🚨 **Ela não existia.** `ExportPhotoUseCase`, `ExportController` e
//! `ImageExporterImpl` estavam escritos e testados desde antes da migração, e
//! **nunca eram construídos no `main.rs`**: as seis ocorrências de "export" em
//! `crates/ui-gpui/src` eram todas comentário. O app importava, organizava,
//! triava, revelava, imprimia a prévia e mostrava ao cliente — e não produzia um
//! arquivo.
//!
//! ⚠️ **O app de egui também não tinha**, e é o que explica a migração não ter
//! acusado: `docs/PARIDADE-UI.md` não menciona exportação em linha nenhuma, e
//! paridade com quem não exporta é não exportar.

pub mod destino;
pub mod porta;
pub mod tela;
