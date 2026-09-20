//! A Impressão — a fase 4 da migração (docs/10-MIGRACAO-GPUI.md §5).
//!
//! O módulo de impressão do legado (`print_view.rs`, 1.132 LOC) é um **layout de
//! papel**: escolher modelo, papel, margens e ver onde as fotos caem.
//!
//! ✅ **E desde 17/ago/2026 ele imprime.** Até então "Print" e "Export PDF"
//! mostravam um aviso de *"coming soon"* — nos dois apps. A folha vira PDF
//! ([`pdf`]) com as fotos **reveladas e enquadradas**, pelo mesmo caminho da
//! exportação, e o PDF vai para um arquivo ou para o diálogo de impressão do
//! sistema ([`porta`]).
//!
//! Como no corte da fase 2, a geometria vem primeiro e sozinha: ela é a metade
//! que erra em silêncio (um papel na proporção errada, uma margem que não é a
//! que o campo diz) e a única que dá para conferir sem abrir a tela.

pub mod pagina;
pub mod pdf;
pub mod porta;
pub mod tela;
