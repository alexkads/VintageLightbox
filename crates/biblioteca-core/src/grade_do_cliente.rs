//! A grade da **tela do cliente**: várias fotos por página, cada uma com o
//! número dela embaixo — funções puras, sem janela.
//!
//! # O número é a identidade
//!
//! 🔑 **O número de uma foto é a posição dela na sessão inteira, contando de
//! 1** — o que o recorte "Todas" mostra. O cliente diz *"quero a 12"* e o
//! balconista procura a 12 do lado dele: as duas telas tiram o número da mesma
//! conta, e por isso não discordam.
//!
//! 🚨 **O recorte não renumera** (dono, 11/out/2026: *"o número não pode mudar
//! em função do filtro, pois deixa o cliente confuso no momento da escolha"*).
//! Até ali o número era a posição no recorte: o operador trocava de "Todas"
//! para "Sinalizadas" e a "4" do cliente virava a "2". Agora o filtro só
//! esconde, e os números ficam vagos (1, 4, 5, 6, 10).
//!
//! 🚨 **Foto que sai da grade do cliente não renumera ninguém.** A rejeitada e
//! a apagada somem daqui, e o número delas fica vago (1, 2, 4, 5…). Fechar o
//! buraco faria a "12" do cliente ser a "13" do operador.
//!
//! # A grade pagina, não rola
//!
//! Quem olha a tela não tem como rolar. Cabem linhas inteiras, e a página no
//! ar é a que tem a foto em foco do operador.
//!
//! # O QR tem coluna própria
//!
//! 🚨 **O QR do bot nunca fica por cima de uma foto** (dono, 11/out/2026:
//! *"QRCode não pode atrapalhar a visualização da grade de fotos"*). Ligado,
//! ele reserva uma coluna à direita e a conta das células é feita sobre o que
//! sobrou.

use crate::grade::Retangulo;

/// O mínimo, o máximo e o padrão de fotos por linha que o operador escolhe.
pub const MENOS_POR_LINHA: usize = 2;
pub const MAIS_POR_LINHA: usize = 8;
pub const POR_LINHA_PADRAO: usize = 4;

/// O respiro entre as células e entre elas e a borda.
pub const VAO: f32 = 12.0;

/// A largura da coluna do QR: o cartão de 256 da tela do cliente e o respiro.
pub const COLUNA_DO_QR: f32 = 288.0;

/// Abaixo desta largura a coluna do QR levaria mais de um terço da tela: ele
/// não entra, em vez de espremer as fotos.
pub const LARGURA_MINIMA_COM_QR: f32 = COLUNA_DO_QR * 3.0;

/// A altura da foto, em relação à largura da célula, que decide **quantas
/// linhas cabem**. A sobra de altura é dividida entre as linhas depois.
const RAZAO_DA_FOTO: f32 = 0.75;

/// O tamanho do número, como fração da largura da célula, e os limites dele.
const NUMERO_POR_LADO: f32 = 0.11;
const NUMERO_MINIMO: f32 = 20.0;
const NUMERO_MAXIMO: f32 = 72.0;

/// A linha do número é a letra e o respiro dela.
const LINHA_POR_LETRA: f32 = 1.35;

/// O que o operador escolheu, dentro da faixa.
pub fn por_linha_valido(pedido: usize) -> usize {
    pedido.clamp(MENOS_POR_LINHA, MAIS_POR_LINHA)
}

/// Onde cada coisa fica numa página da grade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arranjo {
    pub por_linha: usize,
    pub linhas: usize,
    /// A largura da área das fotos — a tela menos a coluna do QR.
    pub largura_das_fotos: f32,
    /// A largura da coluna do QR. Zero quando ele está desligado ou não cabe.
    pub coluna_do_qr: f32,
    pub largura_da_celula: f32,
    pub altura_da_celula: f32,
    /// A parte de cima da célula, onde a foto cabe inteira.
    pub altura_da_foto: f32,
    /// A linha de baixo, reservada ao número — existe com ou sem foto.
    pub altura_do_numero: f32,
    /// O tamanho da letra do número.
    pub letra_do_numero: f32,
}

impl Arranjo {
    pub fn por_pagina(&self) -> usize {
        self.por_linha * self.linhas
    }

    /// A célula de índice `i` **na página**, em coordenadas da área das fotos.
    pub fn celula(&self, i: usize) -> Retangulo {
        let coluna = (i % self.por_linha) as f32;
        let linha = (i / self.por_linha) as f32;
        Retangulo {
            x: VAO + coluna * (self.largura_da_celula + VAO),
            y: VAO + linha * (self.altura_da_celula + VAO),
            w: self.largura_da_celula,
            h: self.altura_da_celula,
        }
    }
}

/// A conta de uma página: a tela, quantas por linha e se o QR está ligado.
///
/// `escala_do_numero` é o gosto do operador sobre o tamanho do número (1 é o
/// padrão); os limites continuam valendo.
///
/// ⚠️ **Sempre cabe ao menos uma linha**, por menor que seja a janela: a
/// altura da célula sai da altura da tela, e não o contrário.
pub fn arranjo(
    largura: f32,
    altura: f32,
    por_linha: usize,
    com_qr: bool,
    escala_do_numero: f32,
) -> Arranjo {
    let largura = if largura.is_finite() {
        largura.max(0.0)
    } else {
        0.0
    };
    let altura = if altura.is_finite() {
        altura.max(0.0)
    } else {
        0.0
    };
    let por_linha = por_linha_valido(por_linha);
    let coluna_do_qr = if com_qr && largura >= LARGURA_MINIMA_COM_QR {
        COLUNA_DO_QR
    } else {
        0.0
    };
    let largura_das_fotos = largura - coluna_do_qr;
    let n = por_linha as f32;
    let largura_da_celula = ((largura_das_fotos - VAO * (n + 1.0)) / n).max(0.0);

    let escala = if escala_do_numero.is_finite() {
        escala_do_numero.clamp(0.5, 2.0)
    } else {
        1.0
    };
    let letra = (largura_da_celula * NUMERO_POR_LADO * escala).clamp(NUMERO_MINIMO, NUMERO_MAXIMO);
    let numero = letra * LINHA_POR_LETRA;

    // Quantas linhas cabem com a foto na proporção de referência…
    let ideal = largura_da_celula * RAZAO_DA_FOTO + numero;
    let linhas = (((altura - VAO) / (ideal + VAO)).floor() as usize).max(1);
    // …e a sobra de altura vai para as fotos, dividida entre as linhas.
    let l = linhas as f32;
    let altura_da_celula = ((altura - VAO * (l + 1.0)) / l).max(0.0);
    // Numa janela minúscula o número não pode comer a célula inteira.
    let altura_do_numero = numero.min(altura_da_celula * 0.5);
    let letra_do_numero = letra.min(altura_do_numero / LINHA_POR_LETRA);

    Arranjo {
        por_linha,
        linhas,
        largura_das_fotos,
        coluna_do_qr,
        largura_da_celula,
        altura_da_celula,
        altura_da_foto: altura_da_celula - altura_do_numero,
        altura_do_numero,
        letra_do_numero,
    }
}

/// Uma foto na grade do cliente: onde ela está no recorte do operador e o
/// número que aparece embaixo dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// A posição no recorte do operador, contando de zero.
    pub posicao: usize,
    /// O número na tela: a posição na sessão inteira, mais um — a mesma em
    /// qualquer recorte.
    pub numero: usize,
}

/// As fotos que o cliente vê, na ordem do operador, com a sessão inteira no
/// recorte. `oculta[p]` diz se a foto da posição `p` fica de fora (rejeitada,
/// apagada).
pub fn itens(oculta: &[bool]) -> Vec<Item> {
    let recorte: Vec<(usize, bool)> = oculta.iter().copied().enumerate().collect();
    itens_do_recorte(&recorte)
}

/// As fotos que o cliente vê quando o operador filtrou. Cada entrada é uma
/// foto do recorte: `(posição dela na sessão inteira, fica de fora?)`.
///
/// 🔑 **O número sai da posição na sessão, e não da posição no recorte**: a
/// mesma foto tem o mesmo número em "Todas", em "Sinalizadas" e em "À venda".
pub fn itens_do_recorte(recorte: &[(usize, bool)]) -> Vec<Item> {
    recorte
        .iter()
        .enumerate()
        .filter(|(_, (_, oculta))| !*oculta)
        .map(|(posicao, (na_sessao, _))| Item {
            posicao,
            numero: na_sessao + 1,
        })
        .collect()
}

/// O índice, entre os itens, da foto em foco do operador.
///
/// ⚠️ **O foco pode estar numa foto que o cliente não vê** (o operador parou
/// numa rejeitada): vale a última visível antes dela, e a primeira se não
/// houver nenhuma — a página não pula para longe por causa disso.
pub fn indice_do_foco(itens: &[Item], foco: usize) -> Option<usize> {
    if itens.is_empty() {
        return None;
    }
    Some(
        itens
            .iter()
            .rposition(|item| item.posicao <= foco)
            .unwrap_or(0),
    )
}

/// Quantas páginas a grade tem. Sem foto nenhuma, é uma página vazia.
pub fn paginas(total: usize, por_pagina: usize) -> usize {
    total.div_ceil(por_pagina.max(1)).max(1)
}

/// A página (de zero) onde está o item de índice `indice`.
pub fn pagina_de(indice: usize, por_pagina: usize) -> usize {
    indice / por_pagina.max(1)
}

/// Os índices dos itens de uma página.
pub fn faixa_da_pagina(pagina: usize, por_pagina: usize, total: usize) -> std::ops::Range<usize> {
    let por_pagina = por_pagina.max(1);
    let de = (pagina * por_pagina).min(total);
    de..(de + por_pagina).min(total)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// As janelas da varredura: da prévia mínima lembrada (320×200) a um 8K,
    /// passando pelos monitores de balcão.
    fn janelas() -> Vec<(f32, f32)> {
        let larguras = [
            320., 640., 800., 863., 864., 1024., 1280., 1366., 1920., 2560., 3840., 7680.,
        ];
        let alturas = [200., 480., 600., 768., 800., 1080., 1440., 2160., 4320.];
        let mut todas = Vec::new();
        for l in larguras {
            for a in alturas {
                todas.push((l, a));
            }
        }
        // E em pé: o monitor do cliente pode estar girado.
        todas.extend([(1080., 1920.), (1440., 2560.), (768., 1366.)]);
        todas
    }

    fn todos_os_arranjos() -> Vec<((f32, f32), Arranjo)> {
        let mut todos = Vec::new();
        for (l, a) in janelas() {
            for por_linha in MENOS_POR_LINHA..=MAIS_POR_LINHA {
                for com_qr in [false, true] {
                    for escala in [0.75, 1.0, 1.4] {
                        todos.push(((l, a), arranjo(l, a, por_linha, com_qr, escala)));
                    }
                }
            }
        }
        todos
    }

    /// 🚨 Nenhuma célula passa da área das fotos — nem para a direita (onde
    /// mora o QR), nem para baixo.
    #[test]
    fn nenhuma_celula_sai_da_area_das_fotos() {
        for ((largura, altura), a) in todos_os_arranjos() {
            for i in 0..a.por_pagina() {
                let c = a.celula(i);
                assert!(c.x >= 0.0 && c.y >= 0.0, "{a:?} célula {i}: {c:?}");
                assert!(
                    c.x + c.w <= a.largura_das_fotos + 0.01,
                    "{largura}×{altura} {a:?}: a célula {i} passa da área das fotos: {c:?}"
                );
                assert!(
                    c.y + c.h <= altura + 0.01,
                    "{largura}×{altura} {a:?}: a célula {i} passa do pé da tela: {c:?}"
                );
            }
        }
    }

    /// 🚨 O pedido do dono: com o QR ligado, nenhuma célula encosta na coluna
    /// dele, e a coluna cabe inteira na tela.
    #[test]
    fn o_qr_nunca_fica_sobre_uma_foto() {
        for (largura, altura) in janelas() {
            for por_linha in MENOS_POR_LINHA..=MAIS_POR_LINHA {
                let a = arranjo(largura, altura, por_linha, true, 1.0);
                assert!(
                    (a.largura_das_fotos + a.coluna_do_qr - largura).abs() < 0.01,
                    "as fotos e o QR somam a tela: {a:?}"
                );
                if largura >= LARGURA_MINIMA_COM_QR {
                    assert_eq!(a.coluna_do_qr, COLUNA_DO_QR, "{largura}: o QR tem a coluna");
                } else {
                    assert_eq!(a.coluna_do_qr, 0.0, "{largura}: estreita demais para o QR");
                }
                let inicio_do_qr = largura - a.coluna_do_qr;
                for i in 0..a.por_pagina() {
                    let c = a.celula(i);
                    assert!(
                        c.x + c.w <= inicio_do_qr + 0.01,
                        "{largura}×{altura}, {por_linha} por linha: a célula {i} invade o QR"
                    );
                }
            }
        }
    }

    /// Sem o QR, as fotos têm a tela inteira.
    #[test]
    fn sem_qr_as_fotos_tem_a_tela_inteira() {
        let a = arranjo(1920., 1080., 4, false, 1.0);
        assert_eq!(a.coluna_do_qr, 0.0);
        assert_eq!(a.largura_das_fotos, 1920.);
    }

    /// As células de uma página não se sobrepõem, e há o vão entre elas.
    #[test]
    fn as_celulas_nao_se_sobrepoem() {
        for (_, a) in todos_os_arranjos() {
            if a.largura_da_celula == 0.0 || a.altura_da_celula == 0.0 {
                continue;
            }
            let celulas: Vec<Retangulo> = (0..a.por_pagina()).map(|i| a.celula(i)).collect();
            for (i, x) in celulas.iter().enumerate() {
                for y in &celulas[i + 1..] {
                    let separadas = x.x + x.w <= y.x + 0.01
                        || y.x + y.w <= x.x + 0.01
                        || x.y + x.h <= y.y + 0.01
                        || y.y + y.h <= x.y + 0.01;
                    assert!(separadas, "{a:?}: {x:?} e {y:?} se sobrepõem");
                }
            }
        }
    }

    /// Sempre há ao menos uma linha, as colunas são as pedidas, e a célula é
    /// foto em cima e número embaixo — nada negativo, nada sobrando.
    #[test]
    fn a_celula_e_a_foto_mais_o_numero() {
        for ((largura, altura), a) in todos_os_arranjos() {
            assert!(a.linhas >= 1, "{largura}×{altura}: {a:?}");
            assert!((MENOS_POR_LINHA..=MAIS_POR_LINHA).contains(&a.por_linha));
            assert!(
                a.altura_da_foto >= 0.0 && a.altura_do_numero >= 0.0,
                "{a:?}"
            );
            assert!(
                (a.altura_da_foto + a.altura_do_numero - a.altura_da_celula).abs() < 0.01,
                "{a:?}"
            );
            assert!(
                a.letra_do_numero <= a.altura_do_numero + 0.01,
                "a letra cabe na linha dela: {a:?}"
            );
            assert!(a.letra_do_numero <= NUMERO_MAXIMO + 0.01);
        }
    }

    /// Nos monitores de balcão o número se lê: nunca abaixo do mínimo.
    #[test]
    fn no_monitor_de_balcao_o_numero_tem_tamanho_de_leitura() {
        for (largura, altura) in [
            (1366., 768.),
            (1920., 1080.),
            (2560., 1440.),
            (3840., 2160.),
        ] {
            for por_linha in MENOS_POR_LINHA..=MAIS_POR_LINHA {
                for com_qr in [false, true] {
                    let a = arranjo(largura, altura, por_linha, com_qr, 1.0);
                    assert!(
                        a.letra_do_numero >= NUMERO_MINIMO - 0.01,
                        "{largura}×{altura}, {por_linha} por linha: letra de {}",
                        a.letra_do_numero
                    );
                    // E a foto fica com a maior parte da célula.
                    assert!(a.altura_da_foto > a.altura_do_numero * 2.0, "{a:?}");
                }
            }
        }
    }

    /// Fora da faixa, o pedido vira o limite; entrada sem sentido não derruba.
    #[test]
    fn pedido_fora_da_faixa_vira_o_limite() {
        assert_eq!(por_linha_valido(0), MENOS_POR_LINHA);
        assert_eq!(por_linha_valido(1), MENOS_POR_LINHA);
        assert_eq!(por_linha_valido(4), 4);
        assert_eq!(por_linha_valido(99), MAIS_POR_LINHA);
        for (l, a) in [(0., 0.), (-10., 500.), (f32::NAN, f32::INFINITY), (1., 1.)] {
            let arr = arranjo(l, a, 4, true, f32::NAN);
            assert!(arr.linhas >= 1);
            assert!(arr.largura_da_celula >= 0.0 && arr.altura_da_celula >= 0.0);
            assert!(arr.altura_da_foto >= 0.0);
        }
    }

    /// Mais fotos por linha, células menores — e nunca menos fotos por página.
    #[test]
    fn mais_por_linha_cabe_mais_por_pagina() {
        for (largura, altura) in janelas() {
            let mut antes: Option<Arranjo> = None;
            for por_linha in MENOS_POR_LINHA..=MAIS_POR_LINHA {
                let a = arranjo(largura, altura, por_linha, false, 1.0);
                if let Some(antes) = antes {
                    assert!(a.largura_da_celula <= antes.largura_da_celula);
                    assert!(
                        a.por_pagina() >= antes.por_pagina(),
                        "{largura}×{altura}: {por_linha} por linha mostra menos que {}",
                        antes.por_linha
                    );
                }
                antes = Some(a);
            }
        }
    }

    /// 🔑 O número é a posição no recorte mais um, e não se repete.
    #[test]
    fn o_numero_e_a_posicao_no_recorte() {
        let todas = itens(&[false; 25]);
        assert_eq!(todas.len(), 25);
        for (i, item) in todas.iter().enumerate() {
            assert_eq!(item.posicao, i);
            assert_eq!(item.numero, i + 1);
        }
        assert!(itens(&[]).is_empty());
    }

    /// 🚨 Quem sai da grade deixa o número vago: ninguém é renumerado.
    #[test]
    fn a_foto_oculta_some_e_nao_renumera_ninguem() {
        let mut oculta = vec![false; 10];
        let antes = itens(&oculta);
        oculta[2] = true;
        oculta[7] = true;
        let depois = itens(&oculta);

        let numeros: Vec<usize> = depois.iter().map(|i| i.numero).collect();
        assert_eq!(numeros, [1, 2, 4, 5, 6, 7, 9, 10]);
        for item in &depois {
            let era = antes.iter().find(|a| a.posicao == item.posicao).unwrap();
            assert_eq!(era.numero, item.numero, "a foto {item:?} mudou de número");
        }
        assert!(itens(&[true; 5]).is_empty(), "todas ocultas: grade vazia");
    }

    /// 🚨 **O filtro não renumera** (dono, 11/out/2026). Onze fotos; o
    /// operador passa de "Todas" para "Sinalizadas" (1, 4, 5, 6, 10): cada
    /// foto continua com o número que o cliente já tinha visto.
    #[test]
    fn o_recorte_do_operador_nao_muda_o_numero_de_ninguem() {
        let todas = itens(&[false; 11]);
        let sinalizadas = [0usize, 3, 4, 5, 9];
        let recorte: Vec<(usize, bool)> = sinalizadas.iter().map(|p| (*p, false)).collect();
        let filtradas = itens_do_recorte(&recorte);

        let numeros: Vec<usize> = filtradas.iter().map(|i| i.numero).collect();
        assert_eq!(numeros, [1, 4, 5, 6, 10], "e não 1, 2, 3, 4, 5");
        for (item, na_sessao) in filtradas.iter().zip(sinalizadas) {
            assert_eq!(item.numero, todas[na_sessao].numero);
        }
        // A posição continua sendo a do recorte: é por ela que o foco do
        // operador acha a página.
        let posicoes: Vec<usize> = filtradas.iter().map(|i| i.posicao).collect();
        assert_eq!(posicoes, [0, 1, 2, 3, 4]);
        assert_eq!(indice_do_foco(&filtradas, 3), Some(3));

        // Oculta dentro do recorte: some, e as outras ficam como estavam.
        let com_rejeitada = itens_do_recorte(&[(0, false), (3, true), (4, false)]);
        let numeros: Vec<usize> = com_rejeitada.iter().map(|i| i.numero).collect();
        assert_eq!(numeros, [1, 5]);
    }

    /// Varredura: qualquer combinação de ocultas dá números únicos, crescentes
    /// e iguais à posição mais um.
    #[test]
    fn os_numeros_sao_unicos_e_crescentes_em_qualquer_recorte() {
        // Um gerador determinístico, para não trazer dependência ao crate.
        let mut semente: u32 = 0x9E37_79B9;
        let mut sorteio = move || {
            semente = semente.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            semente >> 16
        };
        for total in [0usize, 1, 2, 7, 8, 9, 63, 64, 65, 500, 3000] {
            for _ in 0..20 {
                let oculta: Vec<bool> = (0..total).map(|_| sorteio() % 4 == 0).collect();
                let lista = itens(&oculta);
                assert_eq!(lista.len(), oculta.iter().filter(|o| !**o).count());
                for par in lista.windows(2) {
                    assert!(par[0].numero < par[1].numero);
                }
                for item in &lista {
                    assert_eq!(item.numero, item.posicao + 1);
                    assert!(!oculta[item.posicao]);
                }
            }
        }
    }

    /// As páginas cobrem todos os itens, uma vez cada, na ordem.
    #[test]
    fn as_paginas_cobrem_todas_as_fotos_uma_vez() {
        for total in [0usize, 1, 5, 11, 12, 13, 24, 25, 100, 2999] {
            for por_pagina in [1usize, 2, 6, 8, 12, 40] {
                let quantas = paginas(total, por_pagina);
                assert!(quantas >= 1);
                let mut vistos = Vec::new();
                for pagina in 0..quantas {
                    let faixa = faixa_da_pagina(pagina, por_pagina, total);
                    assert!(faixa.len() <= por_pagina);
                    if total > 0 {
                        assert!(!faixa.is_empty(), "página {pagina} vazia com {total} fotos");
                    }
                    vistos.extend(faixa);
                }
                assert_eq!(vistos, (0..total).collect::<Vec<_>>());
                // Depois da última não há nada.
                assert!(faixa_da_pagina(quantas, por_pagina, total).is_empty());
            }
        }
        // Zero por página não divide por zero.
        assert_eq!(paginas(10, 0), 10);
        assert_eq!(pagina_de(3, 0), 3);
    }

    /// 🔑 A foto em foco está sempre na página que está no ar.
    #[test]
    fn a_foto_em_foco_esta_sempre_na_pagina_no_ar() {
        for total in [1usize, 5, 12, 13, 100] {
            let lista = itens(&vec![false; total]);
            for por_pagina in [2usize, 6, 8, 12] {
                for foco in 0..total {
                    let indice = indice_do_foco(&lista, foco).unwrap();
                    let pagina = pagina_de(indice, por_pagina);
                    let faixa = faixa_da_pagina(pagina, por_pagina, lista.len());
                    assert!(faixa.contains(&indice));
                    assert_eq!(lista[indice].posicao, foco);
                }
            }
        }
    }

    /// O foco numa foto que o cliente não vê fica na última visível antes dela
    /// — e na primeira, se não houver.
    #[test]
    fn o_foco_numa_foto_oculta_fica_na_vizinha_de_antes() {
        let lista = itens(&[true, false, false, true, true, false]);
        // posições visíveis: 1, 2, 5
        assert_eq!(
            indice_do_foco(&lista, 0),
            Some(0),
            "antes de todas: a primeira"
        );
        assert_eq!(indice_do_foco(&lista, 1), Some(0));
        assert_eq!(indice_do_foco(&lista, 2), Some(1));
        assert_eq!(indice_do_foco(&lista, 3), Some(1), "oculta: a de antes");
        assert_eq!(indice_do_foco(&lista, 4), Some(1));
        assert_eq!(indice_do_foco(&lista, 5), Some(2));
        assert_eq!(indice_do_foco(&lista, 99), Some(2), "além do fim: a última");
        assert_eq!(indice_do_foco(&[], 3), None, "grade vazia não tem foco");
    }

    /// 🔑 Trocar as fotos por linha muda a página, nunca o número.
    #[test]
    fn trocar_as_fotos_por_linha_nao_muda_o_numero() {
        let oculta: Vec<bool> = (0..57).map(|p| p % 9 == 4).collect();
        let lista = itens(&oculta);
        for por_linha in MENOS_POR_LINHA..=MAIS_POR_LINHA {
            let a = arranjo(1920., 1080., por_linha, true, 1.0);
            let mut numeros = Vec::new();
            for pagina in 0..paginas(lista.len(), a.por_pagina()) {
                for i in faixa_da_pagina(pagina, a.por_pagina(), lista.len()) {
                    numeros.push(lista[i].numero);
                }
            }
            let esperados: Vec<usize> = lista.iter().map(|i| i.numero).collect();
            assert_eq!(numeros, esperados, "{por_linha} por linha");
        }
    }

    /// Os números de referência do balcão: Full HD, quatro por linha.
    #[test]
    fn o_full_hd_com_quatro_por_linha() {
        let a = arranjo(1920., 1080., 4, false, 1.0);
        assert_eq!(a.por_linha, 4);
        assert_eq!(a.linhas, 2);
        assert_eq!(a.por_pagina(), 8);
        assert!((a.largura_da_celula - 465.0).abs() < 0.01);
        assert!((a.altura_da_celula - 522.0).abs() < 0.01);

        let com_qr = arranjo(1920., 1080., 4, true, 1.0);
        assert_eq!(com_qr.coluna_do_qr, COLUNA_DO_QR);
        assert!((com_qr.largura_da_celula - 393.0).abs() < 0.01);
    }
}
