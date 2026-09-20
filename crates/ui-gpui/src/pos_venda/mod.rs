//! O pós-venda do `recordarfotos.com.br` visto do estúdio: a porta que fala com
//! a API v2 e a configuração de onde ela mora.
//!
//! 🚨 **Não há tela aqui, e é de propósito.** Até 7/set/2026 existia o modal
//! "Publicar no pós-venda", que criava galeria nova a partir de uma seleção:
//! título, e-mail, WhatsApp e produto, num formulário só. Ele saiu porque a web
//! não tem esse gesto em lugar nenhum — lá a galeria nasce na **lista** de
//! sessões (`nova-galeria.tsx`, com os mesmos campos), as fotos entram **dentro**
//! da sessão, e o editor salva o revelado na foto que já é dela. Quem entrava na
//! revelação a partir de uma sessão e apertava "salvar" levava aquele formulário
//! na cara, pedindo os dados de uma galeria que já existia.
//!
//! As três telas que usam esta porta são as da web, uma para cada gesto:
//! `sessoes/tela.rs` (a lista, e criar), `sessoes/detalhe.rs` (dentro da sessão)
//! e `revelacao/` pela raiz (salvar na galeria).

pub mod config;
pub mod porta;
