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
/// Os campos de associação (agendamento, voucher, compra, parceiro) — o
/// `associacoes/` do site.
pub mod associacao;
/// A gaveta do atendimento da sessão aberta.
pub mod atendimento;
pub mod detalhe;
/// O texto dos detalhes da galeria (o `DetalhesDaGaleria` do site).
pub mod detalhes_da_galeria;
/// O "Filtros" da lista: um campo por coluna, sob o cabeçalho.
pub mod filtros_da_lista;
/// O assistente de sete etapas da nova sessão (a rota `nova` do site).
pub mod nova;
/// "Do cartão ou pasta…": o menu e a janela de escolher, das duas portas.
pub mod origem_das_fotos;
/// A coluna da direita da galeria: aberta ou recolhida, lembrado.
pub mod paineis;
/// A receita padrão revelada em segundo plano (o `receita-padrao/` do site).
pub mod periodo;
/// O quadro "arraste ou escolha" das duas portas de importar.
pub mod quadro_de_importacao;
pub mod receita_padrao;
/// A política de retenção do pós-venda (a rota `configuracoes` do site).
pub mod retencao;
pub mod tela;
