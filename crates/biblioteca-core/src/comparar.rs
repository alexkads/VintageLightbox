//! Comparar duas fotos lado a lado — o `C` do Lightroom, aqui no `⇧C`.
//!
//! # Para que serve
//!
//! No balcão, o cliente olha o segundo monitor e escolhe **qual das duas**
//! levar. Revelar em série não responde isso: a foto anterior some quando a
//! próxima entra. O Comparar põe as duas juntas, uma de cada lado, e deixa o
//! operador classificar a escolhida sem sair dali.
//!
//! # As duas fotos têm papéis diferentes
//!
//! - **A ativa** é a que está sendo avaliada: a borda âmbar, a que recebe a
//!   nota, o `P` e o `X`, e a que abre no editor ao sair.
//! - **A candidata** é a outra: as setas a trocam pela vizinha na tira, para
//!   o cliente ir comparando a ativa com as próximas.
//!
//! 🔑 **As fotos não trocam de lado.** Clicar numa metade só muda qual é a
//! ativa; as setas só trocam a foto da metade que não é a ativa. O cliente
//! nunca vê a foto que está escolhendo pular de um lado para o outro.
//!
//! # Ids, e não posições
//!
//! Uma nota ou um `X` dados dentro do modo podem mudar o recorte da tira (a
//! rejeitada sai do filtro "sem rejeitadas"). Posições apontariam para outra
//! foto no meio da comparação. Por isso quem chama **congela a tira em ids ao
//! entrar** e passa sempre a mesma lista.
//!
//! ⚠️ O editor do site repete esta regra em TypeScript
//! (`revelacao/comparar.ts`, no `recordarfotos-e-commerce`), com os mesmos
//! casos de teste. O editor de lá não carrega o wasm da biblioteca, e não
//! vale carregá-lo por quarenta linhas. Mudou aqui, muda lá.

/// Uma das duas metades da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Esquerda,
    Direita,
}

impl Lado {
    /// A outra metade.
    pub fn outro(self) -> Self {
        match self {
            Lado::Esquerda => Lado::Direita,
            Lado::Direita => Lado::Esquerda,
        }
    }
}

/// Por que o Comparar não abriu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recusa {
    /// A tira tem uma foto só: não há com quem comparar.
    TiraCurta,
    /// A foto aberta não está na tira — não há de onde partir.
    AbertaForaDaTira,
}

/// As duas fotos na tela, e qual delas está sendo avaliada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparacao<T> {
    pub esquerda: T,
    pub direita: T,
    pub ativa: Lado,
}

/// Abre o Comparar a partir da foto aberta no editor.
///
/// A aberta vai para a esquerda e começa ativa. A candidata é **a primeira
/// marcada depois da aberta, na ordem da tira, dando a volta** — com duas
/// marcadas, é a outra. Marcadas que não estão na tira não contam.
///
/// 🔑 **Só a aberta marcada compara com a vizinha seguinte** (a anterior, se a
/// aberta for a última), como o Lightroom faz com uma foto selecionada. É a
/// pergunta mais comum do balcão — "esta ou a próxima?" —, e errar não custa
/// nada: o Comparar não grava coisa nenhuma, e o `Esc` desfaz.
pub fn entrar<T: PartialEq + Clone>(
    tira: &[T],
    aberta: &T,
    marcadas: &[T],
) -> Result<Comparacao<T>, Recusa> {
    let Some(posicao) = tira.iter().position(|f| f == aberta) else {
        return Err(Recusa::AbertaForaDaTira);
    };
    if tira.len() < 2 {
        return Err(Recusa::TiraCurta);
    }
    let marcada_seguinte = (1..tira.len())
        .map(|k| (posicao + k) % tira.len())
        .find(|&i| marcadas.contains(&tira[i]));
    let candidata = marcada_seguinte.unwrap_or(if posicao + 1 < tira.len() {
        posicao + 1
    } else {
        posicao - 1
    });
    Ok(Comparacao {
        esquerda: aberta.clone(),
        direita: tira[candidata].clone(),
        ativa: Lado::Esquerda,
    })
}

impl<T: PartialEq + Clone> Comparacao<T> {
    /// A foto de um lado.
    pub fn foto(&self, lado: Lado) -> &T {
        match lado {
            Lado::Esquerda => &self.esquerda,
            Lado::Direita => &self.direita,
        }
    }

    /// A foto sendo avaliada.
    pub fn ativa(&self) -> &T {
        self.foto(self.ativa)
    }

    /// A outra.
    pub fn candidata(&self) -> &T {
        self.foto(self.ativa.outro())
    }

    /// O clique numa metade: ela passa a ser a avaliada.
    pub fn ativar(&mut self, lado: Lado) {
        self.ativa = lado;
    }

    /// As setas: troca a candidata pela vizinha na tira, **pulando a ativa** —
    /// comparar uma foto com ela mesma não responde nada. Não dá a volta, como
    /// as setas da tira fora do modo. Devolve se andou.
    pub fn andar(&mut self, tira: &[T], passo: i32) -> bool {
        let Some(de) = tira.iter().position(|f| f == self.candidata()) else {
            return false;
        };
        let direcao = passo.signum() as isize;
        if direcao == 0 {
            return false;
        }
        let mut ate = de as isize;
        for _ in 0..passo.unsigned_abs() {
            ate += direcao;
            if tira.get(ate as usize).is_some_and(|f| f == self.ativa()) {
                ate += direcao;
            }
        }
        if ate < 0 || ate as usize >= tira.len() {
            return false;
        }
        self.trocar_a_candidata(tira[ate as usize].clone());
        true
    }

    /// O clique numa foto da tira: ela vira a candidata. A ativa e a própria
    /// candidata não mudam nada. Devolve se trocou.
    pub fn escolher(&mut self, foto: T) -> bool {
        if &foto == self.ativa() || &foto == self.candidata() {
            return false;
        }
        self.trocar_a_candidata(foto);
        true
    }

    /// A foto que o editor abre ao sair: a avaliada.
    pub fn ao_sair(&self) -> &T {
        self.ativa()
    }

    fn trocar_a_candidata(&mut self, foto: T) {
        match self.ativa {
            Lado::Esquerda => self.direita = foto,
            Lado::Direita => self.esquerda = foto,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const TIRA: [&str; 5] = ["a", "b", "c", "d", "e"];

    fn par(esquerda: &'static str, direita: &'static str, ativa: Lado) -> Comparacao<&'static str> {
        Comparacao {
            esquerda,
            direita,
            ativa,
        }
    }

    #[test]
    fn duas_marcadas_comparam_uma_com_a_outra() {
        assert_eq!(
            entrar(&TIRA, &"b", &["b", "d"]),
            Ok(par("b", "d", Lado::Esquerda))
        );
    }

    #[test]
    fn so_a_aberta_marcada_compara_com_a_seguinte() {
        assert_eq!(
            entrar(&TIRA, &"b", &["b"]),
            Ok(par("b", "c", Lado::Esquerda))
        );
        assert_eq!(entrar(&TIRA, &"b", &[]), Ok(par("b", "c", Lado::Esquerda)));
    }

    #[test]
    fn a_aberta_na_ultima_compara_com_a_anterior() {
        assert_eq!(
            entrar(&TIRA, &"e", &["e"]),
            Ok(par("e", "d", Lado::Esquerda))
        );
    }

    #[test]
    fn tira_de_uma_foto_recusa() {
        assert_eq!(entrar(&["a"], &"a", &["a"]), Err(Recusa::TiraCurta));
    }

    #[test]
    fn aberta_fora_da_tira_recusa() {
        assert_eq!(entrar(&TIRA, &"z", &["a"]), Err(Recusa::AbertaForaDaTira));
    }

    #[test]
    fn com_tres_marcadas_pega_a_proxima_dando_a_volta() {
        assert_eq!(
            entrar(&TIRA, &"d", &["a", "b", "d"]),
            Ok(par("d", "a", Lado::Esquerda))
        );
    }

    #[test]
    fn marcada_fora_da_tira_nao_conta() {
        assert_eq!(
            entrar(&TIRA, &"b", &["z", "b"]),
            Ok(par("b", "c", Lado::Esquerda))
        );
    }

    #[test]
    fn ativar_a_direita_faz_as_setas_trocarem_a_esquerda() {
        let mut c = par("b", "d", Lado::Esquerda);
        c.ativar(Lado::Direita);
        assert!(c.andar(&TIRA, 1));
        assert_eq!(c, par("c", "d", Lado::Direita));
    }

    #[test]
    fn andar_pula_a_ativa() {
        let mut c = par("b", "a", Lado::Esquerda);
        assert!(c.andar(&TIRA, 1));
        assert_eq!(c.candidata(), &"c");
        assert!(c.andar(&TIRA, -1));
        assert_eq!(c.candidata(), &"a");
    }

    #[test]
    fn andar_na_ponta_nao_da_a_volta() {
        let mut c = par("b", "e", Lado::Esquerda);
        assert!(!c.andar(&TIRA, 1));
        assert_eq!(c.candidata(), &"e");
        // A ativa na ponta de lá também é ponta: pular ela cairia fora.
        let mut c = par("a", "b", Lado::Esquerda);
        assert!(!c.andar(&TIRA, -1));
        assert_eq!(c.candidata(), &"b");
    }

    #[test]
    fn escolher_na_tira_troca_a_candidata_mas_nao_a_ativa() {
        let mut c = par("b", "c", Lado::Esquerda);
        assert!(!c.escolher("b"));
        assert!(!c.escolher("c"));
        assert!(c.escolher("e"));
        assert_eq!(c, par("b", "e", Lado::Esquerda));
    }

    #[test]
    fn ao_sair_abre_a_ativa() {
        let mut c = par("b", "d", Lado::Esquerda);
        assert_eq!(c.ao_sair(), &"b");
        c.ativar(Lado::Direita);
        assert_eq!(c.ao_sair(), &"d");
    }
}
