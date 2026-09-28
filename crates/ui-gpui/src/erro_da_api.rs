//! A frase que o operador lê quando o site recusa um pedido ou não responde.
//!
//! 🔑 **O erro chega cru até a tela.** O cliente HTTP escreve
//! `o site respondeu 500 Internal Server Error: {"error":…}`, o domínio põe
//! `Erro de infraestrutura:` na frente, e era isso que aparecia no aviso — com
//! o corpo HTML do proxy inteiro, às vezes.
//!
//! ⚠️ **A tradução é feita aqui, na hora de mostrar, e não no controlador**:
//! quem reage a um status (a lista de sessões com o `403` e o `410`, o caixa, o
//! chatbot, a retenção) lê o número dessa mesma frase.

/// Onde começa a parte técnica do erro, do mais largo ao mais estreito.
const MARCAS: [&str; 5] = [
    "Erro de infraestrutura: ",
    "o site respondeu ",
    "a API respondeu ",
    "o servidor respondeu ",
    "sem resposta do site",
];

/// O texto com a parte técnica trocada por uma frase — ou ele mesmo, se não
/// tiver nenhuma.
///
/// O que vem antes da parte técnica fica (`Não foi possível salvar: …`): é o
/// que diz **qual** gesto falhou.
pub fn legivel(texto: &str) -> String {
    let Some(inicio) = MARCAS.iter().filter_map(|m| texto.find(m)).min() else {
        return texto.to_string();
    };
    let (antes, tecnico) = texto.split_at(inicio);
    let tecnico = tecnico
        .strip_prefix("Erro de infraestrutura: ")
        .unwrap_or(tecnico);
    let frase = traduzir(tecnico);
    if antes.trim().is_empty() {
        maiuscula(&frase)
    } else {
        format!("{antes}{frase}")
    }
}

/// A parte técnica, já sem o `Erro de infraestrutura:`.
fn traduzir(tecnico: &str) -> String {
    if tecnico.starts_with("sem resposta do site") {
        return "o site não respondeu. Confira a internet e tente de novo.".into();
    }
    if tecnico.starts_with("resposta ilegível") {
        return "o site respondeu algo que o app não entendeu. Tente de novo.".into();
    }
    let Some((status, mensagem)) = status_e_mensagem(tecnico) else {
        return tecnico.to_string();
    };
    let dita = mensagem.filter(|m| se_le(m));
    match status {
        401 => "a sua entrada no site venceu. Entre de novo.".into(),
        403 => "a sua conta não tem permissão para isso.".into(),
        413 => "o arquivo é grande demais para o site.".into(),
        429 => "o site pediu uma pausa. Tente de novo em instantes.".into(),
        // O site já escreve a frase da recusa (`409` com as fotos pagas, `400`
        // da validação): ela vai sem o número na frente.
        400..=499 => match dita {
            Some(frase) => frase.to_string(),
            None if status == 404 => "o site não encontrou o que foi pedido.".into(),
            None => format!("o site recusou o pedido ({status})."),
        },
        500..=599 => "o site teve um problema e não concluiu. Tente de novo em instantes.".into(),
        _ => format!("o site respondeu {status}."),
    }
}

/// `o site respondeu 409 Conflict: a frase` → `(409, Some("a frase"))`.
///
/// ⚠️ **O status vem em dois formatos**: `409: …` (o `pedir_json`) e
/// `409 Conflict: …` (o `StatusCode` do reqwest). Ler só os três dígitos serve
/// aos dois.
fn status_e_mensagem(tecnico: &str) -> Option<(u16, Option<&str>)> {
    let (_, depois) = tecnico.split_once("respondeu ")?;
    let status = depois.get(..3)?.parse::<u16>().ok()?;
    let mensagem = depois.split_once(": ").map(|(_, m)| m.trim());
    Some((status, mensagem))
}

/// Se a mensagem do site é uma frase — e não um corpo HTML ou JSON que o
/// envelope de erro não soube abrir.
fn se_le(mensagem: &str) -> bool {
    !mensagem.is_empty()
        && !mensagem.starts_with(['<', '{', '['])
        && mensagem.chars().count() <= 200
}

fn maiuscula(frase: &str) -> String {
    let mut letras = frase.chars();
    match letras.next() {
        Some(primeira) => primeira.to_uppercase().chain(letras).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod testes {
    use super::legivel;

    #[test]
    fn o_erro_do_servidor_vira_uma_frase_sem_o_corpo() {
        assert_eq!(
            legivel(
                "Erro de infraestrutura: o site respondeu 500 Internal Server Error: \
                 <html><body>Bad gateway</body></html>"
            ),
            "O site teve um problema e não concluiu. Tente de novo em instantes."
        );
    }

    #[test]
    fn a_recusa_com_frase_do_site_fica_com_a_frase() {
        assert_eq!(
            legivel("Erro de infraestrutura: o site respondeu 409 Conflict: A galeria tem 2 fotos pagas"),
            "A galeria tem 2 fotos pagas"
        );
        assert_eq!(
            legivel("o site respondeu 400: título obrigatório"),
            "Título obrigatório"
        );
    }

    #[test]
    fn o_gesto_que_falhou_continua_na_frente() {
        assert_eq!(
            legivel("Não foi possível salvar: Erro de infraestrutura: o site respondeu 403 Forbidden: nope"),
            "Não foi possível salvar: a sua conta não tem permissão para isso."
        );
    }

    #[test]
    fn sem_rede_e_sem_resposta_legivel() {
        assert_eq!(
            legivel("Erro de infraestrutura: sem resposta do site: error sending request for url"),
            "O site não respondeu. Confira a internet e tente de novo."
        );
        assert_eq!(
            legivel("Erro de infraestrutura: resposta ilegível: expected value at line 1"),
            "O site respondeu algo que o app não entendeu. Tente de novo."
        );
        assert_eq!(
            legivel("a API respondeu 404: {\"error\":{}}"),
            "O site não encontrou o que foi pedido."
        );
    }

    #[test]
    fn o_que_nao_e_do_site_passa_como_esta() {
        assert_eq!(legivel("cartão removido"), "cartão removido");
        assert_eq!(
            legivel("Erro de infraestrutura: disco cheio"),
            "Disco cheio"
        );
        // Já traduzido: traduzir de novo não muda nada.
        let frase = "O site teve um problema e não concluiu. Tente de novo em instantes.";
        assert_eq!(legivel(frase), frase);
    }
}
