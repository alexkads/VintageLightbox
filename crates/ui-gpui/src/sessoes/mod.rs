//! As sessões fotográficas — a mesma tela que o site tem em
//! `/dashboard/sessoes-fotograficas`: listar, buscar, filtrar por situação,
//! somar o que entrou e abrir uma nova.
//!
//! 🔑 **As contas não moram aqui.** Situação, busca, contagens, soma e o
//! agrupamento do gráfico são de `biblioteca_core::sessoes`, que é o que o site
//! também vai consumir — a mesma pergunta ("cadê a galeria da Maria?") não pode
//! ter duas respostas conforme o app.

pub mod altura_da_tira;
pub mod arquivos;
pub mod detalhe;
/// O assistente de sete etapas da nova sessão (a rota `nova` do site).
pub mod nova;
/// A receita padrão revelada em segundo plano (o `receita-padrao/` do site).
pub mod periodo;
/// O quadro "arraste ou escolha" das duas portas de importar.
pub mod quadro_de_importacao;
pub mod receita_padrao;
/// A política de retenção do pós-venda (a rota `configuracoes` do site).
pub mod retencao;
pub mod tela;
