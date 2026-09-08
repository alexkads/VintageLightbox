//! Nota, cor e sinalizador: o que as treze teclas do legado escrevem.
//!
//! `0`–`5` dão nota, `6`–`9` dão cor, `P`/`X`/`U` sinalizam. São os atalhos mais
//! usados de um programa de seleção — quem tria 800 fotos de um casamento passa
//! por eles algumas centenas de vezes, e cada um é um `UPDATE` no catálogo.
//!
//! ## 🔑 As três regras de alternância moram aqui, e não na tela
//!
//! A nota é **absoluta** (`3` põe 3, `0` tira), mas a cor e o sinalizador
//! **alternam**: teclar `7` numa foto já amarela tira o amarelo, e `P` numa foto
//! já escolhida a desmarca. É a diferença que faz a mesma tecla ser "marcar" e
//! "desmarcar" sem precisar de uma segunda — e é fácil de errar sem perceber,
//! porque errar aqui não falha: só deixa de desmarcar.

use std::sync::Arc;

use adapters::controllers::PhotoController;

/// O que uma tecla de marcação faz com a foto.
#[derive(Debug, Clone, PartialEq)]
pub enum Marca {
    Nota(i32),
    /// `None` é "sem cor" — o que teclar a mesma cor duas vezes produz.
    Cor(Option<String>),
    /// Os códigos do legado: `1` escolhida, `-1` rejeitada, `0` sem marca.
    Sinalizador(i32),
    /// Levada no balcão (`true`) ou deixada para trás (`false`) — a decisão
    /// que o pós-venda do site consome. Tecla `B`, e alterna como `P`.
    Comprada(bool),
}

/// Quem sabe gravar uma marcação.
///
/// 🔑 **É uma porta, e não o `PhotoController` direto**, pelas mesmas duas razões
/// do [`crate::revelacao::persistencia::Gravador`]: o controller é `async` do
/// tokio e o GPUI não roda futuros dele, e os testes de tela precisam afirmar
/// **o que foi gravado** — "apertei `3` duas vezes e o banco recebeu 3 uma vez
/// só" é uma linha com o marcador de mentira e um banco inteiro sem ele.
///
/// Não devolve `Result`, como a outra porta: a tela já mostrou a nota nova, e
/// não há o que ela faça com a falha depois disso além de registrar.
pub trait Marcador: Send + Sync + 'static {
    fn marcar(&self, id: String, marca: Marca);

    /// Tira a foto do catálogo.
    ///
    /// 🚨 **Não apaga o arquivo do disco.** `DeletePhotoUseCase` remove só a
    /// linha do banco — é o "Remove from Catalog" do Lightroom, e é o padrão
    /// seguro. Apagar do disco é operação de outra natureza: precisa de use case
    /// próprio, de um segundo passo no aviso, e de uma decisão que ninguém toma
    /// por engano com a tecla `Delete`.
    ///
    /// ⚠️ **A revelação vai junto.** Os 46 ajustes moram na linha da foto; tirar
    /// a foto do catálogo joga fora o trabalho de revelação dela. Reimportar
    /// devolve o arquivo, não a revelação — é por isso que isto pede confirmação.
    fn apagar(&self, id: String);
}

/// O marcador de verdade: entrega ao `PhotoController`, numa tarefa do tokio.
///
/// ⚠️ O `Handle` é capturado no `main`, **antes** de `Application::run` tomar a
/// thread — a mesma armadilha do gravador da Revelação: um `tokio::spawn` de
/// dentro do GPUI entra em pânico com *there is no reactor running*, e aqui isso
/// aconteceria no meio de uma triagem, ao apertar uma tecla.
pub struct MarcadorDoBanco {
    fotos: Arc<PhotoController>,
    tokio: tokio::runtime::Handle,
}

impl MarcadorDoBanco {
    pub fn novo(fotos: Arc<PhotoController>, tokio: tokio::runtime::Handle) -> Self {
        Self { fotos, tokio }
    }
}

impl Marcador for MarcadorDoBanco {
    fn marcar(&self, id: String, marca: Marca) {
        let fotos = self.fotos.clone();
        let nome = id.clone();

        self.tokio.spawn(async move {
            let resultado = match marca {
                Marca::Nota(nota) => fotos.rate_photo(&id, nota).await,
                // ⚠️ "Sem cor" é **string vazia** no controller, e não um
                // `Option::None`: é a interface que o `set_color_label` oferece,
                // e o legado passa `""` pelo mesmo motivo.
                Marca::Cor(cor) => {
                    fotos
                        .set_color_label(&id, cor.as_deref().unwrap_or(""))
                        .await
                }
                Marca::Sinalizador(codigo) => fotos.set_flag(&id, codigo).await,
                Marca::Comprada(sim) => fotos.set_comprada(&id, sim).await,
            };

            if let Err(erro) = resultado {
                eprintln!("⚠️  Falhou ao marcar {nome}: {erro}");
            }
        });
    }

    fn apagar(&self, id: String) {
        let fotos = self.fotos.clone();
        let nome = id.clone();
        self.tokio.spawn(async move {
            if let Err(erro) = fotos.delete_photo(&id).await {
                eprintln!("⚠️  Falhou ao apagar {nome} do catálogo: {erro}");
            }
        });
    }
}

/// A cor que a tecla produz, dada a que a foto já tem.
///
/// 🔑 **Teclar a mesma cor de novo tira a cor** — é como se desmarca sem uma
/// segunda tecla, e é o que o legado faz. A comparação ignora caixa porque o
/// banco guarda o rótulo como texto livre (`"Red"`, `"red"`), e uma diferença de
/// maiúscula faria a segunda tecla marcar de novo em vez de desmarcar.
pub fn cor_ao_teclar(atual: Option<&str>, pedida: &str) -> Option<String> {
    match atual {
        Some(cor) if cor.eq_ignore_ascii_case(pedida) => None,
        _ => Some(pedida.to_string()),
    }
}

/// O sinalizador que a tecla produz, dado o que a foto já tem.
///
/// `P` e `X` alternam; `U` desmarca sempre — pedir "sem marca" numa foto sem
/// marca não pode "alternar" de volta para coisa nenhuma.
pub fn sinalizador_ao_teclar(atual: Option<i32>, pedido: i32) -> i32 {
    if pedido != 0 && atual.unwrap_or(0) == pedido {
        0
    } else {
        pedido
    }
}

#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::*;

    /// Guarda o que foi mandado gravar, na ordem.
    #[derive(Default)]
    pub struct MarcadorDeMentira {
        marcado: Mutex<Vec<(String, Marca)>>,
        apagados: Mutex<Vec<String>>,
    }

    impl MarcadorDeMentira {
        pub fn apagados(&self) -> Vec<String> {
            self.apagados
                .lock()
                .expect("o marcador de mentira não deve estar envenenado")
                .clone()
        }

        pub fn marcado(&self) -> Vec<(String, Marca)> {
            self.marcado
                .lock()
                .expect("o marcador de mentira não deve estar envenenado")
                .clone()
        }
    }

    impl Marcador for MarcadorDeMentira {
        fn marcar(&self, id: String, marca: Marca) {
            self.marcado
                .lock()
                .expect("o marcador de mentira não deve estar envenenado")
                .push((id, marca));
        }

        fn apagar(&self, id: String) {
            self.apagados
                .lock()
                .expect("o marcador de mentira não deve estar envenenado")
                .push(id);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 A mesma cor duas vezes desmarca.
    ///
    /// Sem isto a tecla é só "marcar", e tirar a cor de uma foto passa a exigir
    /// o menu de contexto — que é o caminho que o atalho existe para evitar.
    #[test]
    fn teclar_a_cor_que_a_foto_ja_tem_tira_a_cor() {
        assert_eq!(cor_ao_teclar(None, "Yellow"), Some("Yellow".to_string()));
        assert_eq!(cor_ao_teclar(Some("Yellow"), "Yellow"), None);
        assert_eq!(
            cor_ao_teclar(Some("Red"), "Yellow"),
            Some("Yellow".to_string()),
            "cor diferente troca, não desmarca"
        );
    }

    /// ⚠️ E a comparação ignora caixa, porque o banco guarda texto livre.
    ///
    /// Com comparação sensível, uma foto gravada como `"red"` receberia
    /// `"Red"` de volta ao teclar `6` — a tecla marcaria de novo em vez de
    /// desmarcar, e a foto pareceria "presa" na cor.
    #[test]
    fn a_cor_gravada_em_minuscula_ainda_desmarca() {
        assert_eq!(cor_ao_teclar(Some("red"), "Red"), None);
    }

    /// `P` numa foto já escolhida a desmarca; `X` idem.
    #[test]
    fn o_sinalizador_repetido_desmarca() {
        assert_eq!(sinalizador_ao_teclar(None, 1), 1);
        assert_eq!(sinalizador_ao_teclar(Some(1), 1), 0);
        assert_eq!(sinalizador_ao_teclar(Some(1), -1), -1, "trocar de lado");
        assert_eq!(sinalizador_ao_teclar(Some(-1), -1), 0);
    }

    /// 🚨 `U` desmarca sempre — inclusive o que já está desmarcado.
    ///
    /// Se `U` alternasse como as outras duas, apertá-lo numa foto sem marca
    /// devolveria... "sem marca" de novo, que é o mesmo. O risco real é o
    /// contrário: escrever a regra como "alterna" e ver `U` marcar algo.
    #[test]
    fn desmarcar_e_absoluto() {
        assert_eq!(sinalizador_ao_teclar(None, 0), 0);
        assert_eq!(sinalizador_ao_teclar(Some(1), 0), 0);
        assert_eq!(sinalizador_ao_teclar(Some(-1), 0), 0);
    }
}
