//! A Impressão — a fase 4 da migração (docs/10-MIGRACAO-GPUI.md §5).
//!
//! O módulo de impressão do legado (`print_view.rs`, 1.132 LOC) é um **layout de
//! papel**: escolher modelo, papel, margens e ver onde as fotos caem. Imprimir
//! mesmo ele não faz — o botão "Print" mostra um aviso de *"coming soon"*, e o
//! "Export PDF" também. Portar é portar a prévia.
//!
//! Como no corte da fase 2, a geometria vem primeiro e sozinha: ela é a metade
//! que erra em silêncio (um papel na proporção errada, uma margem que não é a
//! que o campo diz) e a única que dá para conferir sem abrir a tela.

pub mod pagina;
