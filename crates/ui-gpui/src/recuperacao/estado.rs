//! O estado do painel de recuperação, sem tela: o que chega, o que muda.

use domain::recuperacao::CartaoBruto;
use use_cases::recuperacao::conferir_destino;

/// O que as portas mandam para a tela.
#[derive(Debug, Clone, PartialEq)]
pub enum Recado {
    Cartoes(Vec<CartaoBruto>),
    /// A pasta escolhida no seletor, ou `None` se o operador desistiu.
    Destino(Option<String>),
    Andamento {
        lidos: u64,
        total: u64,
        achadas: usize,
    },
    Terminou {
        achadas: usize,
        interrompida: bool,
        ilegiveis: u32,
    },
    Falhou(String),
    /// O operador fechou o pedido de senha, ou a senha não passou.
    Negado,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Fase {
    /// Escolhendo cartão e destino.
    #[default]
    Escolhendo,
    /// Pedindo a senha ou já varrendo.
    Recuperando {
        lidos: u64,
        total: u64,
        achadas: usize,
        parando: bool,
    },
    Terminou {
        achadas: usize,
        interrompida: bool,
        ilegiveis: u32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Estado {
    pub cartoes: Vec<CartaoBruto>,
    /// Se a lista de cartões já chegou. Antes disso, "nenhum cartão" seria
    /// mentira.
    pub listou: bool,
    pub escolhido: Option<usize>,
    pub destino: Option<String>,
    pub fase: Fase,
    pub aviso: Option<String>,
}

impl Estado {
    pub fn cartao(&self) -> Option<&CartaoBruto> {
        self.escolhido.and_then(|i| self.cartoes.get(i))
    }

    pub fn recuperando(&self) -> bool {
        matches!(self.fase, Fase::Recuperando { .. })
    }

    pub fn escolher_cartao(&mut self, indice: usize) {
        if self.recuperando() || indice >= self.cartoes.len() {
            return;
        }
        self.escolhido = Some(indice);
        self.aviso = self.recusa();
    }

    /// Por que ainda não dá para começar, em palavras da tela.
    fn recusa(&self) -> Option<String> {
        let (Some(cartao), Some(destino)) = (self.cartao(), self.destino.as_deref()) else {
            return None;
        };
        conferir_destino(destino, cartao)
            .err()
            .map(|r| r.texto().to_string())
    }

    /// O botão "Recuperar" só liga com cartão, destino e destino válido.
    pub fn pode_comecar(&self) -> bool {
        !self.recuperando()
            && self.cartao().is_some()
            && self.destino.is_some()
            && self.recusa().is_none()
    }

    /// Passa à fase de recuperação. Devolve o que a porta precisa, ou `None`
    /// se ainda não dá.
    pub fn comecar(&mut self) -> Option<(CartaoBruto, String)> {
        if !self.pode_comecar() {
            self.aviso = self.recusa();
            return None;
        }
        let cartao = self.cartao()?.clone();
        let destino = self.destino.clone()?;
        self.aviso = None;
        self.fase = Fase::Recuperando {
            lidos: 0,
            total: cartao.tamanho,
            achadas: 0,
            parando: false,
        };
        Some((cartao, destino))
    }

    pub fn pedir_parada(&mut self) -> bool {
        match &mut self.fase {
            Fase::Recuperando { parando, .. } if !*parando => {
                *parando = true;
                true
            }
            _ => false,
        }
    }

    /// Volta ao começo para recuperar outro cartão (ou o mesmo de novo).
    pub fn recomecar(&mut self) {
        if !self.recuperando() {
            self.fase = Fase::Escolhendo;
            self.aviso = None;
        }
    }

    /// Quantas fotos voltaram, quando acabou.
    pub fn recuperadas(&self) -> Option<usize> {
        match self.fase {
            Fase::Terminou { achadas, .. } => Some(achadas),
            _ => None,
        }
    }
}

pub fn aplicar(estado: &mut Estado, recado: Recado) {
    match recado {
        Recado::Cartoes(cartoes) => {
            // O escolhido se mantém se continuar plugado.
            let antes = estado.cartao().map(|c| c.dispositivo.clone());
            estado.cartoes = cartoes;
            estado.listou = true;
            estado.escolhido = antes
                .and_then(|d| estado.cartoes.iter().position(|c| c.dispositivo == d))
                .or_else(|| (estado.cartoes.len() == 1).then_some(0));
            if !estado.recuperando() {
                estado.aviso = estado.recusa();
            }
        }
        Recado::Destino(Some(destino)) => {
            if !estado.recuperando() {
                estado.destino = Some(destino);
                estado.aviso = estado.recusa();
            }
        }
        Recado::Destino(None) => {}
        Recado::Andamento {
            lidos,
            total,
            achadas,
        } => {
            if let Fase::Recuperando {
                lidos: l,
                total: t,
                achadas: a,
                ..
            } = &mut estado.fase
            {
                *l = lidos;
                if total > 0 {
                    *t = total;
                }
                *a = achadas;
            }
        }
        Recado::Terminou {
            achadas,
            interrompida,
            ilegiveis,
        } => {
            estado.fase = Fase::Terminou {
                achadas,
                interrompida,
                ilegiveis,
            };
            estado.aviso = (ilegiveis > 0).then(|| {
                format!(
                    "{ilegiveis} trecho(s) do cartão não deixaram ler. O cartão pode estar \
                     com defeito: não use ele de novo antes de conferir."
                )
            });
        }
        Recado::Falhou(motivo) => {
            estado.fase = Fase::Escolhendo;
            estado.aviso = Some(motivo);
        }
        Recado::Negado => {
            estado.fase = Fase::Escolhendo;
            estado.aviso = Some(
                "A recuperação precisa da senha de administrador para ler o cartão. \
                 Nada foi feito."
                    .into(),
            );
        }
    }
}

/// A frase do andamento.
pub fn frase(fase: &Fase) -> String {
    match fase {
        Fase::Escolhendo => String::new(),
        Fase::Recuperando {
            lidos,
            total,
            achadas,
            parando,
        } => {
            if *parando {
                return format!("Parando… {achadas} fotos até aqui");
            }
            if *lidos == 0 {
                return "Esperando a senha de administrador…".into();
            }
            let pct = if *total > 0 {
                (*lidos as f64 / *total as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
            format!(
                "Lendo o cartão: {pct:.0}% · {achadas} {} até aqui",
                if *achadas == 1 { "foto" } else { "fotos" }
            )
        }
        Fase::Terminou {
            achadas,
            interrompida,
            ..
        } => match (achadas, interrompida) {
            (0, false) => "O cartão foi lido inteiro e nenhuma foto inteira foi achada.".into(),
            (n, false) => format!("Pronto: {n} {}.", recuperadas(*n)),
            (n, true) => format!("Parada a pedido: {n} {} até ali.", recuperadas(*n)),
        },
    }
}

fn recuperadas(n: usize) -> &'static str {
    if n == 1 {
        "foto recuperada"
    } else {
        "fotos recuperadas"
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cartao(dispositivo: &str, montagem: &str) -> CartaoBruto {
        CartaoBruto {
            dispositivo: dispositivo.into(),
            nome: "EOS_DIGITAL".into(),
            tamanho: 1000,
            montagens: vec![montagem.into()],
        }
    }

    #[test]
    fn um_cartao_so_ja_vem_escolhido() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/media/a/X")]),
        );
        assert_eq!(e.escolhido, Some(0));
        assert!(!e.pode_comecar(), "sem destino não começa");
        aplicar(&mut e, Recado::Destino(Some("/home/a/Rec".into())));
        assert!(e.pode_comecar());
    }

    #[test]
    fn o_destino_no_cartao_avisa_e_nao_comeca() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/media/a/X")]),
        );
        aplicar(&mut e, Recado::Destino(Some("/media/a/X/fotos".into())));
        assert!(!e.pode_comecar());
        assert!(e.aviso.as_deref().unwrap().contains("próprio cartão"));
        assert_eq!(e.comecar(), None);
    }

    #[test]
    fn a_fase_anda_do_pedido_ao_fim() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/media/a/X")]),
        );
        aplicar(&mut e, Recado::Destino(Some("/home/a/Rec".into())));
        let (c, d) = e.comecar().unwrap();
        assert_eq!(
            (c.dispositivo.as_str(), d.as_str()),
            ("/dev/sdb", "/home/a/Rec")
        );
        assert_eq!(frase(&e.fase), "Esperando a senha de administrador…");
        aplicar(
            &mut e,
            Recado::Andamento {
                lidos: 500,
                total: 1000,
                achadas: 3,
            },
        );
        assert_eq!(frase(&e.fase), "Lendo o cartão: 50% · 3 fotos até aqui");
        // Enquanto recupera, trocar o destino não vale.
        aplicar(&mut e, Recado::Destino(Some("/outro".into())));
        assert_eq!(e.destino.as_deref(), Some("/home/a/Rec"));
        aplicar(
            &mut e,
            Recado::Terminou {
                achadas: 7,
                interrompida: false,
                ilegiveis: 0,
            },
        );
        assert_eq!(e.recuperadas(), Some(7));
        assert_eq!(frase(&e.fase), "Pronto: 7 fotos recuperadas.");
    }

    #[test]
    fn negar_a_senha_volta_ao_comeco_com_aviso() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/media/a/X")]),
        );
        aplicar(&mut e, Recado::Destino(Some("/home/a/Rec".into())));
        e.comecar().unwrap();
        aplicar(&mut e, Recado::Negado);
        assert_eq!(e.fase, Fase::Escolhendo);
        assert!(e
            .aviso
            .as_deref()
            .unwrap()
            .contains("senha de administrador"));
        assert!(e.pode_comecar(), "dá para tentar de novo");
    }

    #[test]
    fn parar_so_uma_vez() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/media/a/X")]),
        );
        aplicar(&mut e, Recado::Destino(Some("/home/a/Rec".into())));
        e.comecar().unwrap();
        assert!(e.pedir_parada());
        assert!(!e.pedir_parada());
    }

    #[test]
    fn a_lista_nova_mantem_o_escolhido() {
        let mut e = Estado::default();
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdb", "/a"), cartao("/dev/sdc", "/b")]),
        );
        e.escolher_cartao(1);
        aplicar(
            &mut e,
            Recado::Cartoes(vec![cartao("/dev/sdc", "/b"), cartao("/dev/sdd", "/c")]),
        );
        assert_eq!(e.cartao().unwrap().dispositivo, "/dev/sdc");
    }
}
