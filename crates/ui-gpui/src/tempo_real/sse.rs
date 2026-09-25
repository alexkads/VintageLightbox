//! O leitor de SSE: bytes que chegam em pedaços viram eventos inteiros.
//!
//! O servidor (`tempo_real.rs` do backend, `fluxo_sse`) fala três coisas:
//!
//! | O que chega | O que é |
//! |---|---|
//! | `event: pronto` / `data: 1` | o fluxo abriu — tudo o que veio antes pode ter se perdido |
//! | `data: {json}` (sem `event:`) | um evento do domínio |
//! | `event: sincronizar` / `data: <n>` | o painel ficou para trás e perdeu `n` eventos |
//! | `:keep-alive` | comentário a cada 15 s; só prova que o cano está vivo |
//!
//! 🔑 **O pedaço de rede não respeita o evento.** Um `data:` pode chegar partido
//! ao meio, e dois eventos podem chegar no mesmo pedaço. Por isso o leitor
//! guarda o resto e só entrega o que terminou na linha em branco.

/// Um evento completo do fluxo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoSse {
    /// O `event:`, ou `message` quando ele não veio (o padrão do SSE).
    pub nome: String,
    /// As linhas `data:` juntas por `\n`.
    pub dados: String,
}

#[derive(Debug, Default)]
pub struct LeitorSse {
    resto: Vec<u8>,
}

impl LeitorSse {
    /// Acrescenta um pedaço e devolve os eventos que ele terminou.
    pub fn ler(&mut self, pedaco: &[u8]) -> Vec<EventoSse> {
        self.resto.extend_from_slice(pedaco);
        let mut eventos = Vec::new();
        while let Some((fim, tamanho)) = fim_do_bloco(&self.resto) {
            let bloco: Vec<u8> = self.resto.drain(..fim + tamanho).take(fim).collect();
            if let Some(evento) = interpretar(&String::from_utf8_lossy(&bloco)) {
                eventos.push(evento);
            }
        }
        eventos
    }
}

/// Onde termina o primeiro bloco (a linha em branco), e quantos bytes tem o
/// separador — `\n\n` ou `\r\n\r\n`.
fn fim_do_bloco(bytes: &[u8]) -> Option<(usize, usize)> {
    (0..bytes.len()).find_map(|i| {
        if bytes[i..].starts_with(b"\r\n\r\n") {
            Some((i, 4))
        } else if bytes[i..].starts_with(b"\n\n") {
            Some((i, 2))
        } else {
            None
        }
    })
}

/// Um bloco vira evento. Bloco só de comentário (o `keep-alive`) não é
/// evento.
fn interpretar(bloco: &str) -> Option<EventoSse> {
    let mut nome = None;
    let mut dados: Vec<&str> = Vec::new();
    for linha in bloco.lines() {
        if linha.starts_with(':') {
            continue;
        }
        let (campo, valor) = linha.split_once(':').unwrap_or((linha, ""));
        let valor = valor.strip_prefix(' ').unwrap_or(valor);
        match campo {
            "event" => nome = Some(valor.to_string()),
            "data" => dados.push(valor),
            _ => {}
        }
    }
    if nome.is_none() && dados.is_empty() {
        return None;
    }
    Some(EventoSse {
        nome: nome.unwrap_or_else(|| "message".into()),
        dados: dados.join("\n"),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_tres_tipos_do_servidor_e_o_keep_alive_calado() {
        let mut leitor = LeitorSse::default();
        let eventos = leitor.ler(
            b"event: pronto\ndata: 1\n\n:keep-alive\n\ndata: {\"tipo\":\"x\"}\n\nevent: sincronizar\ndata: 3\n\n",
        );
        assert_eq!(
            eventos,
            vec![
                EventoSse {
                    nome: "pronto".into(),
                    dados: "1".into()
                },
                EventoSse {
                    nome: "message".into(),
                    dados: "{\"tipo\":\"x\"}".into()
                },
                EventoSse {
                    nome: "sincronizar".into(),
                    dados: "3".into()
                },
            ]
        );
    }

    #[test]
    fn o_evento_partido_entre_dois_pedacos_so_sai_inteiro() {
        let mut leitor = LeitorSse::default();
        assert!(leitor.ler(b"data: {\"tipo\":").is_empty());
        assert!(leitor.ler(b"\"mensagem_recebida\"}\n").is_empty());
        let eventos = leitor.ler(b"\ndata: 2");
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].dados, "{\"tipo\":\"mensagem_recebida\"}");
        assert_eq!(leitor.ler(b"\r\n\r\n")[0].dados, "2", "CRLF também fecha");
    }

    #[test]
    fn varias_linhas_de_dados_se_juntam() {
        let mut leitor = LeitorSse::default();
        let eventos = leitor.ler(b"data:a\ndata: b\n\n");
        assert_eq!(eventos[0].dados, "a\nb");
    }
}
