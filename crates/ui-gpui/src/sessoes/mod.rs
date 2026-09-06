//! As sessões fotográficas — a mesma tela que o site tem em
//! `/dashboard/sessoes-fotograficas`: listar, buscar, filtrar por situação,
//! somar o que entrou e abrir uma nova.
//!
//! 🔑 **As contas não moram aqui.** Situação, busca, contagens, soma e o
//! agrupamento do gráfico são de `biblioteca_core::sessoes`, que é o que o site
//! também vai consumir — a mesma pergunta ("cadê a galeria da Maria?") não pode
//! ter duas respostas conforme o app.

pub mod tela;
