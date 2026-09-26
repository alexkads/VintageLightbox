//! O texto dos detalhes da galeria — o `DetalhesDaGaleria` do site
//! (`[id]/tela-da-galeria.tsx`), linha por linha.
//!
//! 🔑 **As palavras e os destaques moram aqui, e a tela só pinta.** Cada linha
//! vira um [`Texto`]: a frase inteira e os trechos que o site põe em `<strong>`,
//! em `text-muted-foreground` ou num `<Link>`. Assim a frase se confere num
//! teste, e não numa foto da janela.

use std::ops::Range;

use domain::services::pos_venda::AvisoDaGaleria;

/// Como um trecho se destaca do resto da linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tom {
    /// O `<strong>` do site: os números e o preço.
    Forte,
    /// O `text-muted-foreground`: o nome da faixa, o "por fulano".
    Apagado,
    /// O `<Link>` para a política de retenção.
    Link,
}

/// Uma frase e os trechos dela que não têm o tom da linha.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texto {
    pub texto: String,
    pub trechos: Vec<(Range<usize>, Tom)>,
}

impl Texto {
    fn com(mut self, pedaco: &str, tom: Option<Tom>) -> Self {
        let inicio = self.texto.len();
        self.texto.push_str(pedaco);
        if let Some(tom) = tom {
            self.trechos.push((inicio..self.texto.len(), tom));
        }
        self
    }

    /// Os intervalos com este tom — o que a tela torna clicável.
    pub fn de(&self, tom: Tom) -> Vec<Range<usize>> {
        self.trechos
            .iter()
            .filter(|(_, t)| *t == tom)
            .map(|(r, _)| r.clone())
            .collect()
    }
}

/// "**5** levadas · **13** à venda · **0** compradas", e as apagadas só quando há.
pub fn fotos(levadas: usize, a_venda: usize, compradas: usize, apagadas: usize) -> Texto {
    let forte = Some(Tom::Forte);
    let texto = Texto::default()
        .com(&levadas.to_string(), forte)
        .com(" levadas · ", None)
        .com(&a_venda.to_string(), forte)
        .com(" à venda · ", None)
        .com(&compradas.to_string(), forte)
        .com(" compradas", None);
    match apagadas {
        0 => texto,
        n => texto.com(&format!(" · {n} apagadas"), Some(Tom::Apagado)),
    }
}

/// "**R$ 25,00** (Até 2 Pessoas)", e "· sessão mista, N faixas" quando há mais
/// de uma em uso.
pub fn preco_padrao(preco: &str, nome: &str, faixas: usize) -> Texto {
    let texto = Texto::default()
        .com(preco, Some(Tom::Forte))
        .com(" ", None)
        .com(&format!("({nome})"), Some(Tom::Apagado));
    if faixas > 1 {
        texto.com(
            &format!(" · sessão mista, {faixas} faixas"),
            Some(Tom::Apagado),
        )
    } else {
        texto
    }
}

/// "19/09/2026 por fulano" — o "por" apagado, e some quando ninguém consta.
pub fn criada(dia: &str, por: Option<&str>) -> Texto {
    let texto = Texto::default().com(dia, None);
    match por.map(str::trim).filter(|q| !q.is_empty()) {
        Some(quem) => texto
            .com(" ", None)
            .com(&format!("por {quem}"), Some(Tom::Apagado)),
        None => texto,
    }
}

/// O nome que o site dá a cada e-mail (`TIPO_DE_AVISO`).
pub fn tipo_de_aviso(tipo: &str) -> &str {
    match tipo {
        "fotos_prontas" => "Fotos prontas",
        "vencimento_venda" => "Vencimento da venda",
        "vencimento_download" => "Vencimento do download",
        outro => outro,
    }
}

/// `dd/mm/aaaa` no fuso do estúdio.
pub fn dia_br(segundos: i64) -> Option<String> {
    Some(no_estudio(segundos)?.format("%d/%m/%Y").to_string())
}

/// `dd/mm/aaaa, hh:mm` no fuso do estúdio — o `formatarDataHoraBR` do site.
pub fn dia_e_hora_br(segundos: i64) -> Option<String> {
    Some(no_estudio(segundos)?.format("%d/%m/%Y, %H:%M").to_string())
}

fn no_estudio(segundos: i64) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600)?;
    Some(chrono::DateTime::from_timestamp(segundos, 0)?.with_timezone(&brasilia))
}

const POLITICA: &str = "política de retenção";

/// O rodapé: o último e-mail e o que o provedor disse dele, e o link da
/// política de retenção. Um [`Texto`] por linha, na ordem do site — o "e mais
/// N" é um bloco, então o link desce para a linha de baixo quando ele existe.
pub fn rodape(avisos: &[AvisoDaGaleria], tem_email: bool) -> Vec<Texto> {
    let link = |t: Texto| t.com(" · ", None).com(POLITICA, Some(Tom::Link));
    let Some(ultimo) = avisos.first() else {
        let frase = if tem_email {
            "Nenhum aviso enviado ainda. O e-mail “fotos prontas” leva os prazos e um link que entra sem senha e não perde a validade; os avisos de vencimento saem sozinhos."
        } else {
            "Sem e-mail do cliente: o link e o aviso pedem o e-mail antes de sair. Até lá, a retenção trata a galeria como “não lida”."
        };
        return vec![link(Texto::default().com(frase, None))];
    };
    let quando = |s: i64| dia_e_hora_br(s).unwrap_or_default();
    let retorno = match (ultimo.lido_em, ultimo.entregue_em) {
        (Some(lido), _) => format!(" · lido {}", quando(lido)),
        (None, Some(_)) => " · entregue, sem leitura".to_string(),
        (None, None) => " · sem retorno do provedor".to_string(),
    };
    let principal = Texto::default().com(
        &format!(
            "Último aviso: {} em {} para {}{retorno}",
            tipo_de_aviso(&ultimo.tipo),
            quando(ultimo.enviado_em),
            ultimo.destino
        ),
        None,
    );
    match avisos.len() - 1 {
        0 => vec![link(principal)],
        antes => vec![
            principal,
            Texto::default().com(&format!("e mais {antes} aviso(s) antes deste."), None),
            Texto::default()
                .com("· ", None)
                .com(POLITICA, Some(Tom::Link)),
        ],
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 2026-09-19 17:51 no estúdio.
    const ENVIO: i64 = 1_789_851_060;

    fn aviso(lido: Option<i64>, entregue: Option<i64>) -> AvisoDaGaleria {
        AvisoDaGaleria {
            tipo: "fotos_prontas".into(),
            destino: "paula@exemplo.com".into(),
            enviado_em: ENVIO,
            entregue_em: entregue,
            lido_em: lido,
        }
    }

    fn trecho(t: &Texto, tom: Tom) -> Vec<&str> {
        t.de(tom).into_iter().map(|r| &t.texto[r]).collect()
    }

    #[test]
    fn os_numeros_das_fotos_saem_em_negrito_e_as_apagadas_so_quando_ha() {
        let t = fotos(5, 13, 0, 0);
        assert_eq!(t.texto, "5 levadas · 13 à venda · 0 compradas");
        assert_eq!(trecho(&t, Tom::Forte), ["5", "13", "0"]);

        let t = fotos(5, 13, 0, 2);
        assert_eq!(t.texto, "5 levadas · 13 à venda · 0 compradas · 2 apagadas");
        assert_eq!(trecho(&t, Tom::Apagado), [" · 2 apagadas"]);
    }

    #[test]
    fn o_preco_padrao_e_o_valor_em_negrito_com_a_faixa_entre_parenteses() {
        let t = preco_padrao("R$ 25,00", "Até 2 Pessoas", 1);
        assert_eq!(t.texto, "R$ 25,00 (Até 2 Pessoas)");
        assert_eq!(trecho(&t, Tom::Forte), ["R$ 25,00"]);

        let t = preco_padrao("R$ 25,00", "Até 2 Pessoas", 3);
        assert_eq!(t.texto, "R$ 25,00 (Até 2 Pessoas) · sessão mista, 3 faixas");
    }

    #[test]
    fn criada_traz_quem_criou_e_nao_inventa_quando_nao_consta() {
        let t = criada("19/09/2026", Some("ana@exemplo.com"));
        assert_eq!(t.texto, "19/09/2026 por ana@exemplo.com");
        assert_eq!(trecho(&t, Tom::Apagado), ["por ana@exemplo.com"]);
        assert_eq!(criada("19/09/2026", Some("  ")).texto, "19/09/2026");
    }

    #[test]
    fn o_ultimo_aviso_conta_leitura_entrega_ou_silencio_do_provedor() {
        let lido = rodape(&[aviso(Some(ENVIO + 60), Some(ENVIO))], true);
        assert_eq!(
            lido[0].texto,
            "Último aviso: Fotos prontas em 19/09/2026, 17:51 para paula@exemplo.com \
             · lido 19/09/2026, 17:52 · política de retenção"
        );
        assert_eq!(trecho(&lido[0], Tom::Link), ["política de retenção"]);

        let entregue = &rodape(&[aviso(None, Some(ENVIO))], true)[0].texto;
        assert!(
            entregue.contains(" · entregue, sem leitura · "),
            "{entregue}"
        );
        let mudo = &rodape(&[aviso(None, None)], true)[0].texto;
        assert!(mudo.contains(" · sem retorno do provedor · "), "{mudo}");
    }

    /// Como no site: o "e mais N" é um bloco, e o link desce para baixo dele.
    #[test]
    fn com_avisos_anteriores_o_link_desce_para_a_ultima_linha() {
        let linhas = rodape(&[aviso(None, None), aviso(None, None)], true);
        let textos: Vec<_> = linhas.iter().map(|l| l.texto.as_str()).collect();
        assert_eq!(textos.len(), 3);
        assert!(textos[0].starts_with("Último aviso: "));
        assert!(!textos[0].contains("política"));
        assert_eq!(textos[1], "e mais 1 aviso(s) antes deste.");
        assert_eq!(textos[2], "· política de retenção");
    }

    #[test]
    fn sem_aviso_a_frase_depende_de_haver_e_mail() {
        assert!(rodape(&[], true)[0]
            .texto
            .starts_with("Nenhum aviso enviado ainda."));
        assert!(rodape(&[], false)[0]
            .texto
            .starts_with("Sem e-mail do cliente"));
        assert_eq!(
            trecho(&rodape(&[], false)[0], Tom::Link),
            ["política de retenção"]
        );
    }
}
