//! A biblioteca de fotos, sem tela nenhuma em volta.
//!
//! # Por que este crate existe
//!
//! A mesma biblioteca é escrita hoje em três lugares: a grade do painel do
//! pós-venda e a galeria do cliente, no site (`recordarfotos-e-commerce`), e a
//! tela Biblioteca do desktop (`ui-gpui`). Onde a conta é a mesma e o código é
//! diferente, **as três divergem** — e já divergiam: o desktop mostrava uma
//! coluna a menos porque cobrava respiro da última.
//!
//! Aqui mora o que não depende de quem desenha:
//!
//! | módulo | o que decide |
//! |---|---|
//! | [`grade`] | onde cada foto fica, o que está visível, para onde o foco vai |
//! | [`selecao`] | clique, Shift, Ctrl, arrasto e teclado |
//! | [`acervo`] | o recorte da barra, as contagens e o que ainda pode mudar |
//! | [`miniaturas`] | quantas carregar por vez, em que ordem e qual descartar |
//! | [`dinheiro`] | reais em centavos: o que se lê de um campo e o que se escreve na tela |
//! | [`negociacao`] | cortesia, desconto, site parceiro — o registro do balcão |
//! | [`preco_de_venda`] | o preço fixado para a compra online, e o que ele recusa |
//!
//! # O que ele **não** faz, e por quê
//!
//! 🔑 **Não desenha, não busca e não grava.** Desenhar é do wgpu (no navegador)
//! e do GPUI (no desktop); buscar miniatura é `Image` de um lado e disco do
//! outro; gravar é Server Action de um lado e caso de uso do outro. Cada um
//! desses tem uma dependência que não atravessa a fronteira — e é justamente
//! por não tê-las que este crate compila para `wasm32` e roda nos testes em
//! milissegundos.
//!
//! 🔑 **Não sabe desenhar texto.** O nome e a faixa que aparecem sob a foto são
//! escritos por quem tem tipografia: DOM no navegador, GPUI no desktop. Embutir
//! uma fonte aqui pesaria no `.wasm` e daria um texto pior que o dos dois.

pub mod acervo;
pub mod dinheiro;
pub mod grade;
pub mod miniaturas;
pub mod negociacao;
pub mod preco_de_venda;
pub mod selecao;

pub use acervo::{permissoes, Acervo, Contagens, Estado, Filtro, Foto, Permissoes};
pub use grade::{Direcao, Layout, Opcoes, Retangulo};
pub use miniaturas::{Cache as CacheDeMiniaturas, Desfecho, Politica};
pub use selecao::{Modificadores, Selecao};
