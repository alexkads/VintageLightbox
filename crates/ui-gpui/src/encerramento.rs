//! Quando o app acaba.
//!
//! No macOS, fechar a última janela **não** encerra o processo: o ícone fica na
//! Dock e o app segue vivo. Num editor com documentos isso faz sentido — clicar
//! no ícone reabre a janela. Aqui não: a janela principal é o app inteiro, não
//! há menu de aplicativo (nunca chamamos `set_menus`), e sem ela sobra um ícone
//! que não responde a nada e um Cmd+Q que não existe. Quem fechou a janela
//! precisou matar o processo — foi o defeito relatado em 7/set/2026.
//!
//! A segunda tela (`cliente`) não conta como "o app está aberto": ela é a
//! apresentação do que está na janela principal, em tela cheia, sem barra de
//! título e sem botão de fechar. Se a principal fosse embora e ela ficasse, o
//! operador teria uma tela preta ocupando o monitor e nenhum jeito de sair.
//! Por isso o critério é a janela principal, e não "sobrou alguma janela".

/// Verdadeiro quando a janela principal não está mais entre as abertas.
///
/// Genérica no tipo do identificador para poder ser conferida sem subir um
/// `App`: o `AnyWindowHandle` do GPUI só entra em `main.rs`.
pub fn deve_encerrar<J: PartialEq>(principal: &J, abertas: &[J]) -> bool {
    !abertas.contains(principal)
}

#[cfg(test)]
mod testes {
    use super::deve_encerrar;

    #[test]
    fn segue_vivo_enquanto_a_principal_esta_aberta() {
        assert!(!deve_encerrar(&1, &[1]));
    }

    #[test]
    fn a_segunda_tela_nao_segura_o_app() {
        // A principal saiu, a do cliente ficou: encerra assim mesmo, senão o
        // monitor do cliente fica preto e ninguém tem como fechá-lo.
        assert!(deve_encerrar(&1, &[2]));
    }

    #[test]
    fn a_principal_aberta_com_a_do_cliente_junto_nao_encerra() {
        assert!(!deve_encerrar(&1, &[1, 2]));
    }

    #[test]
    fn sem_janela_nenhuma_encerra() {
        assert!(deve_encerrar(&1, &[]));
    }
}
