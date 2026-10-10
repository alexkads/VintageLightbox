//! ✍️ A formatação de mensagem do WhatsApp — `*negrito*`, `_itálico_`,
//! `~riscado~`, `` `código` ``, o bloco de três crases, lista, citação e link.
//!
//! Pedido do dono em 2026-10-10: *"o WhatsApp bot não obedece o padrão de
//! formatação das mensagens como negrito, itálico, sublinhado, links"*. O
//! painel mostrava a marcação crua — `👥 *Até 2 pessoas:*`, com os asteriscos —,
//! e o cliente, no telefone, via em negrito. Quem atende lia outra mensagem.
//!
//! No site a mesma regra mora em `chatbot/formatacao.ts`, com os mesmos casos
//! de teste: mudar um lado é mudar o outro.
//!
//! # As regras, que são as do WhatsApp
//!
//! - A marca **abre** depois de começo de linha, espaço ou pontuação, e colada
//!   no texto; **fecha** colada no texto e antes de fim de linha, espaço ou
//!   pontuação. É o que deixa `foto_do_cliente` e `2 * 3 * 4` em paz.
//! - Marca não atravessa linha, e uma cabe dentro da outra (`*_assim_*`).
//! - Código (uma crase) e bloco (três) são literais: nada dentro é formatado.
//! - `* item` e `- item` no começo da linha são lista; `> texto`, citação.
//! - **Sublinhado não existe no WhatsApp** — não há marca para ele.
//!
//! # O link não é formatado por dentro
//!
//! `https://x.com/_abc_/` tem dois sublinhados que abririam e fechariam um
//! itálico: a marca sumiria e o endereço deixaria de abrir. O link é reconhecido
//! **antes** das marcas e sai inteiro.
//!
//! 🔑 **Nada aqui sabe de tela**, como o resto do modelo: devolve o texto sem
//! as marcas e as faixas (em bytes) de cada estilo.

use std::ops::Range;

/// Os estilos de um trecho. Somam-se: `*_isto_*` é negrito e itálico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Estilo {
    pub negrito: bool,
    pub italico: bool,
    pub riscado: bool,
    /// Código e bloco de código.
    pub mono: bool,
    /// A linha é uma citação (`> texto`).
    pub citacao: bool,
}

impl Estilo {
    fn com(mut self, marca: char) -> Self {
        match marca {
            '*' => self.negrito = true,
            '_' => self.italico = true,
            '~' => self.riscado = true,
            _ => self.mono = true,
        }
        self
    }
}

/// Um trecho com estilo, com link, ou com os dois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trecho {
    /// Em bytes, dentro de [`Formatado::texto`].
    pub faixa: Range<usize>,
    pub estilo: Estilo,
    /// O endereço que o clique abre.
    pub link: Option<String>,
}

/// A mensagem pronta para desenhar: o texto **sem as marcas** e os trechos que
/// têm estilo. O que não está em trecho nenhum é texto comum.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formatado {
    pub texto: String,
    pub trechos: Vec<Trecho>,
}

impl Formatado {
    fn empurrar(&mut self, texto: &str, estilo: Estilo, link: Option<String>) {
        if texto.is_empty() {
            return;
        }
        let inicio = self.texto.len();
        self.texto.push_str(texto);
        if estilo != Estilo::default() || link.is_some() {
            self.trechos.push(Trecho {
                faixa: inicio..self.texto.len(),
                estilo,
                link,
            });
        }
    }
}

/// As marcas de uma linha, na ordem em que o WhatsApp as lê.
const MARCAS: [char; 4] = ['*', '_', '~', '`'];

/// O que marca o começo de uma lista e o de uma citação, na tela.
const MARCADOR_DE_LISTA: &str = "• ";

/// Formata a mensagem.
///
/// `marcas` liga as do WhatsApp; desligado, só os links são reconhecidos — é o
/// que os outros canais usam, que não têm essa marcação.
pub fn formatar(texto: &str, marcas: bool) -> Formatado {
    let mut saida = Formatado::default();
    if !marcas {
        em_linha(texto, Estilo::default(), false, &mut saida);
        return saida;
    }

    // Os blocos de três crases vêm primeiro: atravessam linhas, e nada dentro
    // deles é marca.
    let mut resto = texto;
    while let Some(abre) = resto.find("```") {
        let depois = &resto[abre + 3..];
        let Some(fecha) = depois.find("```") else {
            break;
        };
        let dentro = depois[..fecha].trim_matches('\n');
        if dentro.trim().is_empty() {
            // Seis crases sem nada dentro são seis crases.
            linhas(&resto[..abre + 6 + fecha], &mut saida);
        } else {
            linhas(&resto[..abre], &mut saida);
            saida.empurrar(
                dentro,
                Estilo {
                    mono: true,
                    ..Estilo::default()
                },
                None,
            );
        }
        resto = &depois[fecha + 3..];
    }
    linhas(resto, &mut saida);
    saida
}

/// O texto sem as marcas, numa linha só — a prévia da lista de conversas.
pub fn sem_marcas(texto: &str) -> String {
    formatar(texto, true)
        .texto
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn linhas(texto: &str, saida: &mut Formatado) {
    for (n, linha) in texto.split('\n').enumerate() {
        if n > 0 {
            saida.texto.push('\n');
        }
        let mut estilo = Estilo::default();
        let corpo = if let Some(item) = linha
            .strip_prefix("* ")
            .or_else(|| linha.strip_prefix("- "))
            .filter(|item| !item.trim().is_empty())
        {
            saida.texto.push_str(MARCADOR_DE_LISTA);
            item
        } else if let Some(citado) = linha
            .strip_prefix("> ")
            .filter(|citado| !citado.trim().is_empty())
        {
            estilo.citacao = true;
            citado
        } else {
            linha
        };
        em_linha(corpo, estilo, true, saida);
    }
}

/// Onde termina o link que começa em `texto` — ou `None` se ali não começa um.
///
/// A mesma regra do site (`partirEmLinks`): a pontuação que termina frase fica
/// de fora, senão "abra aqui: https://x/y." vira um endereço que dá 404.
fn fim_do_link(texto: &str) -> Option<usize> {
    let esquema = ["https://", "http://"]
        .into_iter()
        .find(|esquema| texto.starts_with(esquema))?;
    let proibido = |c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'');
    let fim = texto.find(proibido).unwrap_or(texto.len());
    let link = texto[..fim].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}']);
    (link.len() > esquema.len()).then_some(link.len())
}

/// A marca em `i` pode abrir: vem depois de começo, espaço ou pontuação, e
/// está colada no que formata.
fn abre(texto: &str, i: usize, marca: char) -> bool {
    let antes = texto[..i].chars().next_back();
    let depois = texto[i + marca.len_utf8()..].chars().next();
    antes.is_none_or(|c| !c.is_alphanumeric() && c != marca)
        && depois.is_some_and(|c| !c.is_whitespace() && c != marca)
}

/// Onde a marca aberta em `i` fecha: colada no texto, e antes de fim, espaço
/// ou pontuação. O código (crase) fecha na primeira crase seguinte.
fn fecho(texto: &str, i: usize, marca: char) -> Option<usize> {
    let comeco = i + marca.len_utf8();
    texto[comeco..]
        .match_indices(marca)
        .map(|(j, _)| comeco + j)
        .find(|&j| {
            if j == comeco {
                return false;
            }
            if marca == '`' {
                return true;
            }
            let antes = texto[..j].chars().next_back();
            let depois = texto[j + marca.len_utf8()..].chars().next();
            antes.is_some_and(|c| !c.is_whitespace()) && depois.is_none_or(|c| !c.is_alphanumeric())
        })
}

fn em_linha(texto: &str, base: Estilo, marcas: bool, saida: &mut Formatado) {
    let mut comum = 0;
    let mut i = 0;
    while i < texto.len() {
        let Some(c) = texto[i..].chars().next() else {
            break;
        };
        // O link primeiro, e inteiro: marca dentro de endereço não é marca.
        if c == 'h' {
            if let Some(tamanho) = fim_do_link(&texto[i..]) {
                saida.empurrar(&texto[comum..i], base, None);
                let link = &texto[i..i + tamanho];
                saida.empurrar(link, base, Some(link.to_string()));
                i += tamanho;
                comum = i;
                continue;
            }
        }
        if marcas && MARCAS.contains(&c) && abre(texto, i, c) {
            if let Some(j) = fecho(texto, i, c) {
                saida.empurrar(&texto[comum..i], base, None);
                let dentro = &texto[i + 1..j];
                if c == '`' {
                    saida.empurrar(dentro, base.com(c), None);
                } else {
                    em_linha(dentro, base.com(c), true, saida);
                }
                i = j + 1;
                comum = i;
                continue;
            }
        }
        i += c.len_utf8();
    }
    saida.empurrar(&texto[comum..], base, None);
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O texto de cada trecho com os estilos por extenso — o que o teste lê.
    fn lido(texto: &str) -> (String, Vec<(String, String)>) {
        let f = formatar(texto, true);
        let trechos = f
            .trechos
            .iter()
            .map(|t| {
                let mut estilos = Vec::new();
                if t.estilo.negrito {
                    estilos.push("negrito");
                }
                if t.estilo.italico {
                    estilos.push("italico");
                }
                if t.estilo.riscado {
                    estilos.push("riscado");
                }
                if t.estilo.mono {
                    estilos.push("mono");
                }
                if t.estilo.citacao {
                    estilos.push("citacao");
                }
                if t.link.is_some() {
                    estilos.push("link");
                }
                (f.texto[t.faixa.clone()].to_string(), estilos.join("+"))
            })
            .collect();
        (f.texto, trechos)
    }

    fn par(texto: &str, estilos: &str) -> (String, String) {
        (texto.to_string(), estilos.to_string())
    }

    /// 🔑 A mensagem de preço do bot, como ela é gravada — o caso do pedido.
    #[test]
    fn a_mensagem_de_preco_do_bot_sai_sem_asterisco_e_em_negrito() {
        let (texto, trechos) = lido("👥 *Até 2 pessoas:*\n💵 *R$ 25,00*\n📸 *digital*");
        assert_eq!(texto, "👥 Até 2 pessoas:\n💵 R$ 25,00\n📸 digital");
        assert_eq!(
            trechos,
            [
                par("Até 2 pessoas:", "negrito"),
                par("R$ 25,00", "negrito"),
                par("digital", "negrito"),
            ]
        );
    }

    #[test]
    fn os_quatro_estilos_e_um_dentro_do_outro() {
        let (texto, trechos) = lido("_itálico_ ~riscado~ `código` e *_os dois_*");
        assert_eq!(texto, "itálico riscado código e os dois");
        assert_eq!(
            trechos,
            [
                par("itálico", "italico"),
                par("riscado", "riscado"),
                par("código", "mono"),
                par("os dois", "negrito+italico"),
            ]
        );

        // Negrito com um pedaço em itálico no meio: três trechos.
        let (texto, trechos) = lido("*antes _meio_ depois*");
        assert_eq!(texto, "antes meio depois");
        assert_eq!(
            trechos,
            [
                par("antes ", "negrito"),
                par("meio", "negrito+italico"),
                par(" depois", "negrito"),
            ]
        );
    }

    /// O que **não** é marca fica como foi escrito.
    #[test]
    fn marca_no_meio_de_palavra_ou_solta_nao_formata() {
        for cru in [
            "foto_do_cliente.jpg",
            "2 * 3 * 4 = 24",
            "preço: R$ 10 ~ R$ 20",
            "* sozinho",
            "*sem fechar",
            "fecha sem abrir*",
            "* com espaço *",
            "a*b*c",
            "**",
            "__",
        ] {
            let (texto, trechos) = lido(cru);
            assert_eq!(trechos, [], "trechos de {cru:?}");
            // A lista troca o marcador; o resto sai igual.
            if !cru.starts_with("* ") {
                assert_eq!(texto, cru);
            }
        }
    }

    #[test]
    fn marca_nao_atravessa_linha() {
        let (texto, trechos) = lido("*começa\ntermina*");
        assert_eq!(texto, "*começa\ntermina*");
        assert_eq!(trechos, []);
    }

    #[test]
    fn pontuacao_em_volta_nao_impede_a_marca() {
        let (texto, trechos) = lido("(*atenção*), veja: _isto_!");
        assert_eq!(texto, "(atenção), veja: isto!");
        assert_eq!(trechos, [par("atenção", "negrito"), par("isto", "italico")]);
    }

    #[test]
    fn o_bloco_de_tres_crases_atravessa_linhas_e_e_literal() {
        let (texto, trechos) = lido("código:\n```\nlinha *um*\nlinha _dois_\n```\nfim");
        assert_eq!(texto, "código:\nlinha *um*\nlinha _dois_\nfim");
        assert_eq!(trechos, [par("linha *um*\nlinha _dois_", "mono")]);

        // Código de uma crase também é literal.
        let (texto, trechos) = lido("use `*assim*` no texto");
        assert_eq!(texto, "use *assim* no texto");
        assert_eq!(trechos, [par("*assim*", "mono")]);

        // Sem fechar, as crases ficam.
        assert_eq!(lido("``` sem fim").0, "``` sem fim");
    }

    #[test]
    fn lista_e_citacao_no_comeco_da_linha() {
        let (texto, trechos) = lido("Leve:\n* documento\n- *comprovante*\n> como combinado");
        assert_eq!(texto, "Leve:\n• documento\n• comprovante\ncomo combinado");
        assert_eq!(
            trechos,
            [
                par("comprovante", "negrito"),
                par("como combinado", "citacao"),
            ]
        );
    }

    /// 🚨 O link sai inteiro e clicável — marca dentro do endereço não é marca.
    #[test]
    fn o_link_sai_inteiro_e_nao_e_formatado_por_dentro() {
        let (texto, trechos) = lido("veja https://x.com/_abc_/foto_1.jpg. e *isto*");
        assert_eq!(texto, "veja https://x.com/_abc_/foto_1.jpg. e isto");
        assert_eq!(
            trechos,
            [
                par("https://x.com/_abc_/foto_1.jpg", "link"),
                par("isto", "negrito"),
            ]
        );
        assert_eq!(
            formatar("veja https://x.com/a. ok", true).trechos[0]
                .link
                .as_deref(),
            Some("https://x.com/a"),
            "a pontuação que termina a frase fica fora do endereço"
        );

        // O link dentro de um negrito leva os dois.
        let (texto, trechos) = lido("*abra https://x.com/v agora*");
        assert_eq!(texto, "abra https://x.com/v agora");
        assert_eq!(
            trechos,
            [
                par("abra ", "negrito"),
                par("https://x.com/v", "negrito+link"),
                par(" agora", "negrito"),
            ]
        );

        // "http" solto não é link.
        assert_eq!(lido("http e https://").1, []);
    }

    /// Os outros canais não têm a marcação: só o link é reconhecido.
    #[test]
    fn sem_as_marcas_so_o_link_e_reconhecido() {
        let f = formatar("*oi* veja https://x.com/a", false);
        assert_eq!(f.texto, "*oi* veja https://x.com/a");
        assert_eq!(f.trechos.len(), 1);
        assert_eq!(f.trechos[0].link.as_deref(), Some("https://x.com/a"));
    }

    #[test]
    fn a_previa_da_lista_sai_sem_marca_e_numa_linha() {
        assert_eq!(
            sem_marcas("👥 *Até 2 pessoas:*\n💵 *R$ 25,00*"),
            "👥 Até 2 pessoas: 💵 R$ 25,00"
        );
        assert_eq!(sem_marcas("📷 Foto"), "📷 Foto");
    }

    #[test]
    fn as_faixas_caem_em_fronteira_de_caractere() {
        // Acento e emoji antes e dentro da marca: a faixa é em bytes, e um
        // corte no meio de um caractere derrubaria o desenho.
        let f = formatar("ação 👍 *você* é _ótimo_", true);
        for trecho in &f.trechos {
            assert!(f.texto.is_char_boundary(trecho.faixa.start));
            assert!(f.texto.is_char_boundary(trecho.faixa.end));
        }
        assert_eq!(&f.texto[f.trechos[0].faixa.clone()], "você");
        assert_eq!(f.texto, "ação 👍 você é ótimo");

        // Colada num emoji, a marca abre: emoji não é letra.
        assert_eq!(lido("👍*sim*").1, [par("sim", "negrito")]);
    }
}
