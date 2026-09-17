//! A política de retenção do pós-venda: as faixas de cada prazo e o que o
//! formulário recusa antes de mandar ao servidor.
//!
//! É o `configuracaoDeRetencaoFormSchema` do site
//! (`frontend/src/lib/schemas/pos-venda.ts`), com as mesmas frases. O servidor
//! valida de novo (os `CHECK` da migration); isto aqui só poupa a viagem.

/// Os quatro prazos, na ordem em que o formulário os mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prazo {
    DiasAVenda,
    DiasLiberadas,
    DiasDeAviso,
    ProrrogacaoSemLeitura,
}

impl Prazo {
    pub const TODOS: [Prazo; 4] = [
        Prazo::DiasAVenda,
        Prazo::DiasLiberadas,
        Prazo::DiasDeAviso,
        Prazo::ProrrogacaoSemLeitura,
    ];

    /// O nome do campo na API.
    pub fn campo(self) -> &'static str {
        match self {
            Prazo::DiasAVenda => "dias_a_venda",
            Prazo::DiasLiberadas => "dias_liberadas",
            Prazo::DiasDeAviso => "dias_de_aviso",
            Prazo::ProrrogacaoSemLeitura => "prorrogacao_sem_leitura_dias",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Prazo::DiasAVenda => "Fotos não adquiridas ficam à venda por (dias)",
            Prazo::DiasLiberadas => "Fotos adquiridas ficam para download por (dias)",
            Prazo::DiasDeAviso => "Avisar o cliente por e-mail (dias antes de vencer)",
            Prazo::ProrrogacaoSemLeitura => "Se o aviso não foi lido, adiar a exclusão por (dias)",
        }
    }

    pub fn ajuda(self) -> &'static str {
        match self {
            Prazo::DiasAVenda => "Contados da criação da galeria. Depois disso a retenção apaga o original e a prévia — a linha fica, para a conta de vendas.",
            Prazo::DiasLiberadas => "Contados da liberação: no balcão, da entrada; compradas, do pagamento. A foto adquirida que o cliente nunca baixou não é apagada (veja abaixo).",
            Prazo::DiasDeAviso => "Um e-mail por fase: um para as fotos à venda, outro para as adquiridas. O link do e-mail entra sem senha e vale 7 dias.",
            Prazo::ProrrogacaoSemLeitura => "A leitura vem do MailerSend (abertura ou clique). Adia uma vez só, a galeria inteira; depois disso apaga mesmo sem leitura. Zero desliga.",
        }
    }

    /// `(mínimo, máximo)`, as faixas que o backend valida.
    pub fn faixa(self) -> (i64, i64) {
        match self {
            Prazo::DiasAVenda | Prazo::DiasLiberadas => (7, 3650),
            Prazo::DiasDeAviso => (1, 90),
            Prazo::ProrrogacaoSemLeitura => (0, 365),
        }
    }

    fn abaixo(self) -> &'static str {
        match self {
            Prazo::DiasAVenda => "Menos de uma semana não dá tempo de comprar",
            Prazo::DiasLiberadas => "Menos de uma semana não dá tempo de baixar",
            Prazo::DiasDeAviso => "Ao menos 1 dia antes",
            Prazo::ProrrogacaoSemLeitura => "Zero desliga a prorrogação",
        }
    }

    fn acima(self) -> &'static str {
        match self {
            Prazo::DiasAVenda | Prazo::DiasLiberadas => "Acima de 10 anos é guardar para sempre",
            Prazo::DiasDeAviso => "Acima de 90 dias o aviso sai junto com as fotos",
            Prazo::ProrrogacaoSemLeitura => "Acima de um ano não é prorrogação",
        }
    }
}

/// O que o formulário manda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Politica {
    pub dias_a_venda: i64,
    pub dias_liberadas: i64,
    pub dias_de_aviso: i64,
    pub prorrogacao_sem_leitura_dias: i64,
    pub apagar_liberada_sem_download: bool,
    pub apagar_automaticamente: bool,
}

/// Os quatro campos como o operador os digitou.
pub type Digitado<'a> = [&'a str; 4];

/// Confere o formulário. Devolve a política, ou a frase de cada prazo errado
/// (na ordem de [`Prazo::TODOS`]).
pub fn conferir(
    digitado: Digitado<'_>,
    apagar_liberada_sem_download: bool,
    apagar_automaticamente: bool,
) -> Result<Politica, [Option<&'static str>; 4]> {
    let mut erros = [None; 4];
    let mut valores = [0i64; 4];
    for (i, prazo) in Prazo::TODOS.iter().enumerate() {
        let texto = digitado[i].trim();
        match texto.parse::<i64>() {
            Err(_) => erros[i] = Some("Use um número inteiro de dias"),
            Ok(n) => {
                let (min, max) = prazo.faixa();
                if n < min {
                    erros[i] = Some(prazo.abaixo());
                } else if n > max {
                    erros[i] = Some(prazo.acima());
                }
                valores[i] = n;
            }
        }
    }
    // O aviso precisa sair antes dos dois vencimentos.
    if erros.iter().all(Option::is_none) && valores[2] >= valores[0].min(valores[1]) {
        erros[2] =
            Some("O aviso precisa sair antes do vencimento — menos dias que os dois prazos.");
    }
    if erros.iter().any(Option::is_some) {
        return Err(erros);
    }
    Ok(Politica {
        dias_a_venda: valores[0],
        dias_liberadas: valores[1],
        dias_de_aviso: valores[2],
        prorrogacao_sem_leitura_dias: valores[3],
        apagar_liberada_sem_download,
        apagar_automaticamente,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_politica_do_site_passa() {
        let politica = conferir(["90", "365", "20", "30"], false, true).unwrap();
        assert_eq!(politica.dias_a_venda, 90);
        assert_eq!(politica.prorrogacao_sem_leitura_dias, 30);
        assert!(politica.apagar_automaticamente);
    }

    #[test]
    fn cada_prazo_fora_da_faixa_diz_o_porque() {
        let erros = conferir(["6", "3651", "0", "366"], false, false).unwrap_err();
        assert_eq!(
            erros[0],
            Some("Menos de uma semana não dá tempo de comprar")
        );
        assert_eq!(erros[1], Some("Acima de 10 anos é guardar para sempre"));
        assert_eq!(erros[2], Some("Ao menos 1 dia antes"));
        assert_eq!(erros[3], Some("Acima de um ano não é prorrogação"));
        let erros = conferir(["90", "sete", "20", "0"], false, false).unwrap_err();
        assert_eq!(erros[1], Some("Use um número inteiro de dias"));
        assert_eq!(erros[3], None, "zero desliga a prorrogação, e vale");
    }

    #[test]
    fn o_aviso_sai_antes_dos_dois_vencimentos() {
        let erros = conferir(["30", "365", "30", "0"], false, false).unwrap_err();
        assert!(erros[2].unwrap().starts_with("O aviso precisa sair antes"));
        assert!(conferir(["30", "365", "29", "0"], false, false).is_ok());
    }
}
