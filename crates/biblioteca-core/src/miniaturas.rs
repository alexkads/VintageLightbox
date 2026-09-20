use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

/// Quem entra, quem espera e quem sai da memória de miniaturas — **a decisão,
/// não o carregamento**.
///
/// # Por que a política é separada de quem busca
///
/// Buscar uma miniatura é coisa de cada lado: no navegador é uma `Image` que
/// decodifica sozinha, no desktop é o `thumbnail_generator` lendo do disco. O
/// que **não** muda entre os dois é a parte que erra: quantas buscar ao mesmo
/// tempo, em que ordem, quantas guardar e qual descartar quando estoura.
///
/// Este módulo é essa parte, e ela cabe num teste que roda em milissegundos.
///
/// # As três regras que já custaram caro
///
/// 1. 🚨 **Paralelismo curto.** Sem teto, o primeiro quadro de uma grade de 200
///    fotos dispara 200 requisições — e as 30 que estão na tela chegam por
///    último, porque entraram na fila junto com as outras 170.
/// 2. 🚨 **A fila é substituída, não acumulada.** Quem rola rápido pede faixas
///    novas a cada quadro; guardar tudo faria a grade carregar o que ficou para
///    trás antes do que está na frente dos olhos.
/// 3. 🚨 **Falha é resposta.** Uma miniatura que não carrega fica registrada
///    como falha; sem isso a tela tenta de novo a cada quadro, para sempre.
#[derive(Debug, Clone, Copy)]
pub struct Politica {
    /// Quantas miniaturas ficam na memória.
    pub teto: usize,
    /// Quantas buscas simultâneas.
    pub paralelo: usize,
}

impl Default for Politica {
    fn default() -> Self {
        Self {
            teto: 240,
            paralelo: 6,
        }
    }
}

/// O que aconteceu com uma miniatura que terminou de carregar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desfecho {
    Chegou,
    Falhou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    /// Guardada e utilizável, com o instante lógico do último uso.
    Viva(u64),
    /// Tentou e não veio. Fica registrada para não ser pedida de novo.
    Falhou,
}

/// A política aplicada a um conjunto de chaves.
///
/// A chave é de quem usa: no site é a **URL** da prévia (e não o id — a URL
/// muda quando o estado da foto muda, e a miniatura velha tem de cair junto);
/// no desktop é o id da foto no catálogo.
#[derive(Debug)]
pub struct Cache<K: Eq + Hash + Clone> {
    politica: Politica,
    estados: HashMap<K, Estado>,
    em_voo: HashSet<K>,
    fila: VecDeque<K>,
    /// As que a tela quer agora — protegidas da poda enquanto estiverem à vista.
    desejadas: HashSet<K>,
    relogio: u64,
}

impl<K: Eq + Hash + Clone> Cache<K> {
    pub fn nova(politica: Politica) -> Self {
        Self {
            politica,
            estados: HashMap::new(),
            em_voo: HashSet::new(),
            fila: VecDeque::new(),
            desejadas: HashSet::new(),
            relogio: 0,
        }
    }

    pub fn tem(&self, chave: &K) -> bool {
        matches!(self.estados.get(chave), Some(Estado::Viva(_)))
    }

    pub fn falhou(&self, chave: &K) -> bool {
        matches!(self.estados.get(chave), Some(Estado::Falhou))
    }

    pub fn guardadas(&self) -> usize {
        self.estados
            .values()
            .filter(|e| matches!(e, Estado::Viva(_)))
            .count()
    }

    pub fn carregando(&self) -> usize {
        self.em_voo.len()
    }

    /// Marca uso — é o que decide quem sobrevive à poda.
    pub fn usar(&mut self, chave: &K) {
        self.relogio += 1;
        if let Some(Estado::Viva(usada_em)) = self.estados.get_mut(chave) {
            *usada_em = self.relogio;
        }
    }

    /// O que a tela quer agora, em ordem de prioridade. Devolve o que deve
    /// **começar a carregar neste instante** — o resto fica na fila.
    pub fn pedir(&mut self, desejadas: &[K]) -> Vec<K> {
        self.desejadas = desejadas.iter().cloned().collect();
        self.fila = desejadas
            .iter()
            .filter(|k| !self.estados.contains_key(k) && !self.em_voo.contains(k))
            .cloned()
            .collect();
        self.bombear()
    }

    /// Registra o fim de uma busca e devolve **o que sair da memória** por
    /// causa dela, mais o que começa a carregar em seguida.
    pub fn concluir(&mut self, chave: &K, desfecho: Desfecho) -> Concluido<K> {
        self.em_voo.remove(chave);
        self.relogio += 1;
        self.estados.insert(
            chave.clone(),
            match desfecho {
                Desfecho::Chegou => Estado::Viva(self.relogio),
                Desfecho::Falhou => Estado::Falhou,
            },
        );
        Concluido {
            podar: self.podar(),
            comecar: self.bombear(),
        }
    }

    /// Esquece tudo — troca de galeria, de filtro, de pasta.
    pub fn limpar(&mut self) {
        self.estados.clear();
        self.em_voo.clear();
        self.fila.clear();
        self.desejadas.clear();
    }

    fn bombear(&mut self) -> Vec<K> {
        let mut comecar = Vec::new();
        while self.em_voo.len() < self.politica.paralelo {
            let Some(proxima) = self.fila.pop_front() else {
                break;
            };
            if self.estados.contains_key(&proxima) || self.em_voo.contains(&proxima) {
                continue;
            }
            self.em_voo.insert(proxima.clone());
            comecar.push(proxima);
        }
        comecar
    }

    /// A poda: as menos usadas saem quando passam do teto.
    ///
    /// ⚠️ **O que está à vista é o último a sair.** Podar uma miniatura visível
    /// devolveria a mesma chave para a fila no quadro seguinte — a grade
    /// carregaria em laço justamente enquanto alguém a olha. Elas só entram na
    /// conta quando a tela sozinha já passa do teto, e aí não há escolha boa.
    fn podar(&mut self) -> Vec<K> {
        if self.guardadas() <= self.politica.teto {
            return Vec::new();
        }

        let mut vivas: Vec<(K, u64, bool)> = self
            .estados
            .iter()
            .filter_map(|(k, e)| match e {
                Estado::Viva(usada_em) => Some((k.clone(), *usada_em, self.desejadas.contains(k))),
                Estado::Falhou => None,
            })
            .collect();

        // Primeiro as que não estão à vista, e entre elas as mais antigas.
        vivas.sort_by(|a, b| a.2.cmp(&b.2).then(a.1.cmp(&b.1)));

        let excedente = self.guardadas() - self.politica.teto;
        let podadas: Vec<K> = vivas
            .into_iter()
            .take(excedente)
            .map(|(k, _, _)| k)
            .collect();
        for chave in &podadas {
            self.estados.remove(chave);
        }
        podadas
    }
}

/// O efeito de uma busca que terminou.
#[derive(Debug, PartialEq, Eq)]
pub struct Concluido<K> {
    /// O que sai da memória agora — quem desenha destrói a textura destas.
    pub podar: Vec<K>,
    /// O que começa a carregar em seguida.
    pub comecar: Vec<K>,
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cache(teto: usize, paralelo: usize) -> Cache<&'static str> {
        Cache::nova(Politica { teto, paralelo })
    }

    #[test]
    fn comeca_so_o_paralelo_e_guarda_o_resto_na_fila() {
        let mut c = cache(100, 2);
        let comecar = c.pedir(&["a", "b", "c", "d"]);
        assert_eq!(comecar, vec!["a", "b"], "duas de cada vez, na ordem pedida");
        assert_eq!(c.carregando(), 2);
    }

    #[test]
    fn cada_uma_que_chega_puxa_a_proxima_da_fila() {
        let mut c = cache(100, 2);
        c.pedir(&["a", "b", "c", "d"]);
        assert_eq!(c.concluir(&"a", Desfecho::Chegou).comecar, vec!["c"]);
        assert_eq!(c.concluir(&"b", Desfecho::Chegou).comecar, vec!["d"]);
        assert_eq!(
            c.concluir(&"c", Desfecho::Chegou).comecar,
            Vec::<&str>::new()
        );
    }

    /// 🚨 A fila é substituída: quem rola rápido não faz a grade carregar o que
    /// já saiu da tela antes do que está nela.
    #[test]
    fn pedir_de_novo_troca_a_fila_em_vez_de_acumular() {
        let mut c = cache(100, 1);
        c.pedir(&["a", "b", "c"]);
        // Rolou: agora o que importa é outro pedaço da grade.
        let comecar = c.pedir(&["x", "y"]);
        assert!(comecar.is_empty(), "o paralelo já está ocupado com 'a'");
        assert_eq!(
            c.concluir(&"a", Desfecho::Chegou).comecar,
            vec!["x"],
            "a próxima é a nova, não a 'b' que saiu da tela"
        );
    }

    #[test]
    fn o_que_ja_esta_guardado_nao_e_pedido_de_novo() {
        let mut c = cache(100, 4);
        c.pedir(&["a"]);
        c.concluir(&"a", Desfecho::Chegou);
        assert!(c.tem(&"a"));
        assert_eq!(c.pedir(&["a", "b"]), vec!["b"]);
    }

    /// 🚨 Sem isto a tela tenta de novo a cada quadro, para sempre.
    #[test]
    fn o_que_falhou_fica_registrado_e_nao_e_tentado_de_novo() {
        let mut c = cache(100, 4);
        c.pedir(&["a"]);
        c.concluir(&"a", Desfecho::Falhou);
        assert!(c.falhou(&"a"));
        assert!(!c.tem(&"a"));
        assert!(c.pedir(&["a"]).is_empty());
    }

    #[test]
    fn acima_do_teto_saem_as_menos_usadas() {
        let mut c = cache(2, 4);
        for chave in ["a", "b", "c"] {
            c.pedir(&[chave]);
            c.concluir(&chave, Desfecho::Chegou);
        }
        // Nada está à vista, e "a" foi a mais antiga.
        assert_eq!(c.guardadas(), 2);
        assert!(!c.tem(&"a"));
        assert!(c.tem(&"b") && c.tem(&"c"));
    }

    #[test]
    fn usar_uma_miniatura_a_salva_da_poda() {
        let mut c = cache(2, 4);
        for chave in ["a", "b"] {
            c.pedir(&[chave]);
            c.concluir(&chave, Desfecho::Chegou);
        }
        c.usar(&"a"); // "a" volta a ser a mais recente
        c.pedir(&["c"]);
        c.concluir(&"c", Desfecho::Chegou);
        assert!(c.tem(&"a"), "usada há pouco, fica");
        assert!(!c.tem(&"b"), "a mais antiga é a que sai");
    }

    /// ⚠️ Podar o que está na tela devolveria a chave para a fila no quadro
    /// seguinte — a grade carregaria em laço enquanto alguém a olha.
    #[test]
    fn o_que_esta_a_vista_e_o_ultimo_a_sair() {
        let mut c = cache(2, 4);
        for chave in ["a", "b"] {
            c.pedir(&[chave]);
            c.concluir(&chave, Desfecho::Chegou);
        }
        // "a" é a mais antiga, mas está à vista; "b" não está.
        c.pedir(&["a", "c"]);
        let efeito = c.concluir(&"c", Desfecho::Chegou);
        assert_eq!(efeito.podar, vec!["b"]);
        assert!(c.tem(&"a"));
    }

    #[test]
    fn limpar_esquece_tudo() {
        let mut c = cache(100, 4);
        c.pedir(&["a", "b"]);
        c.concluir(&"a", Desfecho::Chegou);
        c.limpar();
        assert_eq!(c.guardadas(), 0);
        assert_eq!(c.carregando(), 0);
        assert_eq!(c.pedir(&["a"]), vec!["a"], "depois de limpar, pede de novo");
    }
}
