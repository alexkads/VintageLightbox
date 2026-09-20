//! A geometria da página: onde cada foto cai no papel.
//!
//! Só matemática, sem tela. É a parte da impressão que erra sem avisar — um
//! papel desenhado na proporção errada não quebra nada, só responde a pergunta
//! errada — e a única que dá para conferir com `assert`.
//!
//! ## 🔑 Tudo aqui é em milímetro, e é de propósito
//!
//! Margem e espaçamento são digitados em milímetro pelo fotógrafo, o papel é
//! definido em milímetro, e a folha impressa existe em milímetro. A tela pede
//! uma escala ([`Leiaute::escala_para`]) e multiplica; nada aqui sabe o que é
//! pixel.
//!
//! Guardar as células em **fração do papel**, que era a alternativa óbvia,
//! traria junto uma armadilha: fração não é isotrópica. Numa A4 retrato, `0,5`
//! na horizontal são 105 mm e `0,5` na vertical são 148,5 mm — e encaixar foto
//! sem esticar, que é conta de proporção, sairia deformada em silêncio.

/// O papel, e as medidas que ele tem de verdade.
///
/// Os mesmos cinco do legado (`print_view.rs`), com os mesmos milímetros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Papel {
    A4,
    A3,
    Carta,
    Oficio,
    Tabloide,
}

impl Papel {
    pub const TODOS: [Papel; 5] = [
        Papel::A4,
        Papel::A3,
        Papel::Carta,
        Papel::Oficio,
        Papel::Tabloide,
    ];

    pub fn nome(&self) -> &'static str {
        match self {
            Papel::A4 => "A4 (210 × 297 mm)",
            Papel::A3 => "A3 (297 × 420 mm)",
            Papel::Carta => "Carta (216 × 279 mm)",
            Papel::Oficio => "Ofício (216 × 356 mm)",
            Papel::Tabloide => "Tabloide (279 × 432 mm)",
        }
    }

    /// Largura e altura **em retrato**, em milímetros.
    pub fn milimetros(&self) -> (f32, f32) {
        match self {
            Papel::A4 => (210.0, 297.0),
            Papel::A3 => (297.0, 420.0),
            Papel::Carta => (215.9, 279.4),
            Papel::Oficio => (215.9, 355.6),
            Papel::Tabloide => (279.4, 431.8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientacao {
    Retrato,
    Paisagem,
}

impl Orientacao {
    pub fn nome(&self) -> &'static str {
        match self {
            Orientacao::Retrato => "Retrato",
            Orientacao::Paisagem => "Paisagem",
        }
    }
}

/// O modelo de grade, que é o que o legado chama de *template*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modelo {
    Unica,
    Grade2x2,
    Grade3x3,
    Grade4x4,
    FolhaDeContato,
    Personalizada,
}

impl Modelo {
    pub const TODOS: [Modelo; 6] = [
        Modelo::Unica,
        Modelo::Grade2x2,
        Modelo::Grade3x3,
        Modelo::Grade4x4,
        Modelo::FolhaDeContato,
        Modelo::Personalizada,
    ];

    pub fn nome(&self) -> &'static str {
        match self {
            Modelo::Unica => "Uma foto",
            Modelo::Grade2x2 => "2 × 2",
            Modelo::Grade3x3 => "3 × 3",
            Modelo::Grade4x4 => "4 × 4",
            Modelo::FolhaDeContato => "Folha de contato",
            Modelo::Personalizada => "Personalizada",
        }
    }
}

/// Os limites da grade personalizada, os mesmos do legado (`DragValue::range`).
///
/// Eles moram aqui, e não na tela, porque quem responde `fotos_por_pagina` é
/// esta função: um `12` digitado num campo sem limite viraria 96 células de
/// meio milímetro, e a conta de páginas concordaria com elas.
const COLUNAS: std::ops::RangeInclusive<u8> = 1..=6;
const LINHAS: std::ops::RangeInclusive<u8> = 1..=8;

/// O quanto a foto recua dentro da célula.
///
/// Os mesmos 95% do legado: a moldura da célula continua visível em volta da
/// foto, que é o que separa "duas fotos coladas" de "duas fotos numa grade".
const RESPIRO: f32 = 0.95;

/// Um retângulo em milímetros, a contar do canto superior esquerdo do papel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Celula {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

/// As escolhas que decidem a página.
///
/// É o `PrintViewState` do legado sem a parte que é estado de tela (as fotos
/// escolhidas, o arrasto de cada célula): aqui fica só o que a conta precisa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leiaute {
    pub modelo: Modelo,
    pub papel: Papel,
    pub orientacao: Orientacao,
    pub margem_mm: f32,
    pub espaco_mm: f32,
    /// Só valem com [`Modelo::Personalizada`].
    pub colunas: u8,
    pub linhas: u8,
}

impl Default for Leiaute {
    /// ⚠️ **Escrito à mão, e não derivado.** `#[derive(Default)]` daria margem
    /// zero, espaçamento zero e uma grade de zero por zero coluna — a página
    /// abriria com as fotos sangrando na borda e sem nenhuma célula, sem erro
    /// nenhum. É a mesma armadilha do `Ajustes::default` da fase 2, onde
    /// `contrast` neutro é 1.0.
    ///
    /// Os valores são os do legado: A4 retrato, 10 mm de margem, 5 mm entre
    /// células, e a grade personalizada começando em 2 × 2.
    fn default() -> Self {
        Self {
            modelo: Modelo::Unica,
            papel: Papel::A4,
            orientacao: Orientacao::Retrato,
            margem_mm: 10.0,
            espaco_mm: 5.0,
            colunas: 2,
            linhas: 2,
        }
    }
}

impl Leiaute {
    /// Largura e altura do papel, **já orientado**, em milímetros.
    ///
    /// 🚨 **No legado esta conta não existe, e é o defeito central da tela de
    /// lá**: o papel é desenhado como `0,8 × 0,9` do espaço disponível, então a
    /// folha tem a proporção da **janela**. Trocar A4 por Tabloide não muda
    /// nada; girar para paisagem não muda nada. Os dois controles existem, são
    /// gravados no estado e nunca chegam a um pixel.
    ///
    /// Aqui a proporção é a do papel — e a divergência é decisão, pelo mesmo
    /// motivo que o histórico da fase 2 não copiou o passo por quadro: **prévia
    /// que muda de forma junto com a janela não é conferível**, e uma prévia de
    /// impressão que não mostra o papel não responde a única pergunta que ela
    /// existe para responder.
    pub fn papel_mm(&self) -> (f32, f32) {
        let (largura, altura) = self.papel.milimetros();
        match self.orientacao {
            Orientacao::Retrato => (largura, altura),
            Orientacao::Paisagem => (altura, largura),
        }
    }

    /// Colunas e linhas da grade.
    ///
    /// 🚨 **O `photos_per_page` do legado responde 4 para qualquer grade
    /// personalizada**, porque o `grid_dimensions` dela devolve o `(2, 2)` que é
    /// só o valor inicial dos dois campos. A prévia de lá desenha `custom_cols ×
    /// custom_rows` células, e o rodapé conta páginas de 4 fotos: numa grade 6 ×
    /// 8, a tela mostra 48 fotos numa folha e diz que são 12 páginas. Nenhum dos
    /// dois números avisa que discorda do outro.
    pub fn grade(&self) -> (u8, u8) {
        match self.modelo {
            Modelo::Unica => (1, 1),
            Modelo::Grade2x2 => (2, 2),
            Modelo::Grade3x3 => (3, 3),
            Modelo::Grade4x4 => (4, 4),
            // 4 colunas por 6 linhas — as 24 fotos por folha do legado.
            Modelo::FolhaDeContato => (4, 6),
            Modelo::Personalizada => (
                self.colunas.clamp(*COLUNAS.start(), *COLUNAS.end()),
                self.linhas.clamp(*LINHAS.start(), *LINHAS.end()),
            ),
        }
    }

    pub fn fotos_por_pagina(&self) -> usize {
        let (colunas, linhas) = self.grade();
        colunas as usize * linhas as usize
    }

    /// Quantas folhas um acervo de `total` fotos ocupa.
    ///
    /// Zero foto é zero página — e não uma folha em branco, que é o que a tela
    /// desenharia se este número fosse `max(1)`. Quem mostra "página 1 de 1" com
    /// nada dentro está dizendo que há algo para imprimir.
    pub fn paginas(&self, total: usize) -> usize {
        if total == 0 {
            return 0;
        }
        total.div_ceil(self.fotos_por_pagina())
    }

    /// As fotos que caem na folha `pagina` (contada de zero).
    ///
    /// Fora da faixa devolve fatia vazia em vez de estourar: a página que está
    /// na tela e o tamanho da lista mudam por caminhos diferentes — trocar de
    /// modelo encolhe o número de páginas debaixo de quem está olhando a última.
    pub fn fotos_da_pagina<'a, T>(&self, fotos: &'a [T], pagina: usize) -> &'a [T] {
        let por_pagina = self.fotos_por_pagina();
        let inicio = pagina.saturating_mul(por_pagina);
        if inicio >= fotos.len() {
            return &[];
        }
        let fim = (inicio + por_pagina).min(fotos.len());
        &fotos[inicio..fim]
    }

    /// Quantos pixels vale um milímetro, para o papel inteiro caber no espaço
    /// dado — a única ponte entre este módulo e a tela.
    pub fn escala_para(&self, disponivel: (f32, f32)) -> f32 {
        let (largura, altura) = self.papel_mm();
        let escala = (disponivel.0 / largura).min(disponivel.1 / altura);
        // Janela recém-aberta mede zero, e um quadro com escala negativa
        // desenharia retângulos invertidos antes do primeiro layout.
        if escala.is_finite() && escala > 0.0 {
            escala
        } else {
            0.0
        }
    }

    /// Onde fica cada célula da folha, em milímetros, na ordem da leitura
    /// (esquerda para a direita, de cima para baixo).
    ///
    /// ⚠️ **A margem é a mesma distância nos quatro lados, e é a que o campo
    /// diz.** No legado ela é `margem_mm / 297`, aplicada como fração **do mesmo
    /// jeito nos dois eixos e sempre dividida pela altura da A4**: numa A4
    /// retrato, uma margem pedida de 10 mm sai com 7,1 mm nas laterais, e num
    /// Tabloide sai com 9,4 mm nas laterais e 14,5 mm em cima. O número digitado
    /// não descreve nenhuma das duas distâncias.
    pub fn celulas(&self) -> Vec<Celula> {
        let (papel_largura, papel_altura) = self.papel_mm();
        let (colunas, linhas) = self.grade();

        // A margem não pode comer o papel inteiro: com 50 mm (o teto do campo no
        // legado) numa A4, sobram 110 mm de largura — mas numa margem maior que
        // a metade a área útil vira negativa, e cada célula sairia com largura
        // negativa. Retângulo invertido não falha: ele some, e a folha aparece
        // vazia como se não houvesse foto escolhida.
        let margem = self.margem_mm.max(0.0);
        let espaco = self.espaco_mm.max(0.0);
        let util_largura = (papel_largura - 2.0 * margem).max(0.0);
        let util_altura = (papel_altura - 2.0 * margem).max(0.0);

        let vaos_horizontais = (colunas as f32 - 1.0) * espaco;
        let vaos_verticais = (linhas as f32 - 1.0) * espaco;
        let largura = ((util_largura - vaos_horizontais) / colunas as f32).max(0.0);
        let altura = ((util_altura - vaos_verticais) / linhas as f32).max(0.0);

        let mut celulas = Vec::with_capacity(colunas as usize * linhas as usize);
        for linha in 0..linhas {
            for coluna in 0..colunas {
                celulas.push(Celula {
                    x: margem + coluna as f32 * (largura + espaco),
                    y: margem + linha as f32 * (altura + espaco),
                    largura,
                    altura,
                });
            }
        }
        celulas
    }
}

/// A foto dentro da célula: inteira, centralizada e sem esticar.
///
/// `aspecto` é largura ÷ altura da foto. Uma foto mais larga que a célula
/// encosta nas laterais e sobra em cima e embaixo; mais alta, o contrário — que
/// é o que a grade de contato faz com retrato e paisagem misturados.
pub fn encaixar(celula: &Celula, aspecto: f32) -> Celula {
    if !aspecto.is_finite() || aspecto <= 0.0 || celula.largura <= 0.0 || celula.altura <= 0.0 {
        return Celula {
            x: celula.x + celula.largura / 2.0,
            y: celula.y + celula.altura / 2.0,
            largura: 0.0,
            altura: 0.0,
        };
    }

    let disponivel_largura = celula.largura * RESPIRO;
    let disponivel_altura = celula.altura * RESPIRO;

    let (largura, altura) = if disponivel_largura / disponivel_altura > aspecto {
        // A célula é mais larga que a foto: quem manda é a altura.
        (disponivel_altura * aspecto, disponivel_altura)
    } else {
        (disponivel_largura, disponivel_largura / aspecto)
    };

    Celula {
        x: celula.x + (celula.largura - largura) / 2.0,
        y: celula.y + (celula.altura - altura) / 2.0,
        largura,
        altura,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    /// 🚨 O papel decide a proporção da folha — a janela não opina.
    ///
    /// No legado a folha é `0,8 × 0,9` do espaço disponível, então ela tem a
    /// forma da janela: A4 e Tabloide desenham o mesmo retângulo, e maximizar a
    /// janela muda o formato do papel. Aqui A4 é A4.
    #[test]
    fn o_papel_decide_a_proporcao_e_nao_a_janela() {
        let a4 = Leiaute::default();
        let tabloide = Leiaute {
            papel: Papel::Tabloide,
            ..Leiaute::default()
        };

        let quadrado = (1000.0, 1000.0);
        let forma = |l: &Leiaute| {
            let escala = l.escala_para(quadrado);
            let (largura, altura) = l.papel_mm();
            (largura * escala) / (altura * escala)
        };

        assert!(perto(forma(&a4), 210.0 / 297.0));
        assert!(perto(forma(&tabloide), 279.4 / 431.8));
        assert!(
            !perto(forma(&a4), forma(&tabloide)),
            "trocar de papel tem de mudar a folha na tela"
        );
    }

    /// Paisagem troca os lados — e é a outra metade do mesmo defeito do legado,
    /// onde o botão existe e não chega a nenhum pixel.
    #[test]
    fn a_paisagem_troca_os_lados_do_papel() {
        let retrato = Leiaute::default();
        let paisagem = Leiaute {
            orientacao: Orientacao::Paisagem,
            ..Leiaute::default()
        };

        assert_eq!(retrato.papel_mm(), (210.0, 297.0));
        assert_eq!(paisagem.papel_mm(), (297.0, 210.0));
    }

    /// A folha inteira cabe no espaço, e sobra numa direção só.
    #[test]
    fn a_folha_cabe_inteira_no_espaco_disponivel() {
        let leiaute = Leiaute::default();
        let (largura, altura) = leiaute.papel_mm();

        for espaco in [(1000.0, 1000.0), (300.0, 900.0), (900.0, 300.0)] {
            let escala = leiaute.escala_para(espaco);
            assert!(largura * escala <= espaco.0 + 1e-3);
            assert!(altura * escala <= espaco.1 + 1e-3);
            // E encosta em pelo menos um dos dois lados: menos que isso é folha
            // pequena no meio de espaço vazio.
            assert!(
                perto(largura * escala, espaco.0) || perto(altura * escala, espaco.1),
                "a folha tem de crescer até o limite de um dos eixos"
            );
        }
    }

    /// Janela ainda sem medida não gera escala negativa nem `NaN`.
    #[test]
    fn espaco_zerado_nao_gera_escala_invalida() {
        let leiaute = Leiaute::default();
        assert_eq!(leiaute.escala_para((0.0, 0.0)), 0.0);
        assert_eq!(leiaute.escala_para((-10.0, 500.0)), 0.0);
    }

    /// 🚨 A margem é a distância que o campo diz, nos quatro lados.
    ///
    /// No legado ela é `margem_mm / 297` como fração dos dois eixos: pedir 10 mm
    /// numa A4 dá 7,1 mm nas laterais e 10 mm em cima. O campo diz "Margins
    /// (mm)" e não descreve nenhuma das duas distâncias.
    #[test]
    fn a_margem_e_a_mesma_distancia_nos_quatro_lados() {
        let leiaute = Leiaute {
            margem_mm: 10.0,
            ..Leiaute::default()
        };
        let (papel_largura, papel_altura) = leiaute.papel_mm();
        let celulas = leiaute.celulas();
        let unica = celulas[0];

        assert!(perto(unica.x, 10.0), "esquerda");
        assert!(perto(unica.y, 10.0), "topo");
        assert!(
            perto(papel_largura - (unica.x + unica.largura), 10.0),
            "direita"
        );
        assert!(perto(papel_altura - (unica.y + unica.altura), 10.0), "base");
    }

    /// As células cobrem a área útil sem sobrar nem se sobrepor.
    #[test]
    fn as_celulas_preenchem_a_area_util_com_o_espaco_pedido() {
        let leiaute = Leiaute {
            modelo: Modelo::Grade3x3,
            margem_mm: 12.0,
            espaco_mm: 4.0,
            ..Leiaute::default()
        };
        let (papel_largura, papel_altura) = leiaute.papel_mm();
        let celulas = leiaute.celulas();

        assert_eq!(celulas.len(), 9);

        let primeira = celulas[0];
        let ultima = celulas[8];
        assert!(perto(primeira.x, 12.0));
        assert!(perto(papel_largura - (ultima.x + ultima.largura), 12.0));
        assert!(perto(papel_altura - (ultima.y + ultima.altura), 12.0));

        // O vão entre duas colunas vizinhas é exatamente o espaçamento pedido.
        let vao = celulas[1].x - (celulas[0].x + celulas[0].largura);
        assert!(perto(vao, 4.0));
        let vao_vertical = celulas[3].y - (celulas[0].y + celulas[0].altura);
        assert!(perto(vao_vertical, 4.0), "e é o mesmo nos dois eixos");
    }

    /// A ordem é a da leitura: a segunda célula está à direita da primeira, não
    /// embaixo dela. É o que casa o índice da célula com o índice da foto.
    #[test]
    fn as_celulas_saem_na_ordem_da_leitura() {
        let leiaute = Leiaute {
            modelo: Modelo::FolhaDeContato,
            ..Leiaute::default()
        };
        let celulas = leiaute.celulas();

        assert_eq!(celulas.len(), 24, "4 colunas × 6 linhas");
        assert!(celulas[1].x > celulas[0].x);
        assert!(perto(celulas[1].y, celulas[0].y));
        assert!(
            celulas[4].y > celulas[0].y,
            "a quinta começa a segunda linha"
        );
        assert!(perto(celulas[4].x, celulas[0].x));
    }

    /// 🚨 A grade personalizada conta as próprias células.
    ///
    /// No legado, `photos_per_page` responde 4 para qualquer combinação — a
    /// prévia desenha 48 células numa grade 6 × 8 e o rodapé promete 12 páginas
    /// para as mesmas 48 fotos.
    #[test]
    fn a_grade_personalizada_conta_as_proprias_celulas() {
        let leiaute = Leiaute {
            modelo: Modelo::Personalizada,
            colunas: 6,
            linhas: 8,
            ..Leiaute::default()
        };

        assert_eq!(leiaute.grade(), (6, 8));
        assert_eq!(leiaute.fotos_por_pagina(), 48);
        assert_eq!(leiaute.celulas().len(), 48);
        assert_eq!(leiaute.paginas(48), 1, "48 fotos cabem numa folha só");
    }

    /// E ela respeita os limites dos campos do legado, mesmo que alguém escreva
    /// fora deles: zero coluna daria divisão por zero na conta de páginas.
    #[test]
    fn a_grade_personalizada_nunca_e_zero() {
        let leiaute = Leiaute {
            modelo: Modelo::Personalizada,
            colunas: 0,
            linhas: 99,
            ..Leiaute::default()
        };

        assert_eq!(leiaute.grade(), (1, 8));
        assert_eq!(leiaute.paginas(3), 1);
    }

    #[test]
    fn a_conta_de_paginas_segue_o_modelo() {
        let uma = Leiaute::default();
        assert_eq!(uma.paginas(0), 0, "sem foto não há folha em branco");
        assert_eq!(uma.paginas(3), 3);

        let quatro = Leiaute {
            modelo: Modelo::Grade2x2,
            ..Leiaute::default()
        };
        assert_eq!(quatro.paginas(4), 1, "exatamente cheia é uma folha");
        assert_eq!(quatro.paginas(5), 2);

        let contato = Leiaute {
            modelo: Modelo::FolhaDeContato,
            ..Leiaute::default()
        };
        assert_eq!(contato.paginas(25), 2);
    }

    /// A página dois começa onde a um parou.
    ///
    /// ⚠️ **O legado desenha a página 1 e só ela**: as células são preenchidas a
    /// partir do índice 0 da lista, o rodapé diz "Page 1 of 7" e não há como
    /// chegar às outras seis. A conta existe aqui porque a folha é que é a
    /// unidade da impressão; a tela é que decide o que oferece.
    #[test]
    fn a_pagina_seguinte_comeca_onde_a_anterior_parou() {
        let leiaute = Leiaute {
            modelo: Modelo::Grade2x2,
            ..Leiaute::default()
        };
        let fotos: Vec<usize> = (0..5).collect();

        assert_eq!(leiaute.fotos_da_pagina(&fotos, 0), &[0, 1, 2, 3]);
        assert_eq!(leiaute.fotos_da_pagina(&fotos, 1), &[4], "a última sobra");
        assert!(
            leiaute.fotos_da_pagina(&fotos, 2).is_empty(),
            "página que não existe é fatia vazia, e não pânico"
        );
    }

    /// 🚨 Margem grande demais zera a célula — nunca a inverte.
    ///
    /// Com 50 mm (o teto do campo) numa A4 sobram 110 × 197 mm; com mais que a
    /// metade do papel a área útil vira negativa. No legado a conta é a mesma e
    /// não tem piso: a célula sai com largura negativa, o retângulo se inverte e
    /// a folha aparece vazia — o mesmo desenho de "nenhuma foto escolhida".
    #[test]
    fn margem_maior_que_o_papel_nao_gera_celula_negativa() {
        let leiaute = Leiaute {
            margem_mm: 200.0,
            ..Leiaute::default()
        };

        for celula in leiaute.celulas() {
            assert!(celula.largura >= 0.0, "largura negativa vira folha vazia");
            assert!(celula.altura >= 0.0);
        }
    }

    /// O mesmo vale para o espaçamento entre células, que também soma.
    #[test]
    fn espaco_maior_que_a_area_util_nao_gera_celula_negativa() {
        let leiaute = Leiaute {
            modelo: Modelo::Grade4x4,
            espaco_mm: 90.0,
            ..Leiaute::default()
        };

        for celula in leiaute.celulas() {
            assert!(celula.largura >= 0.0);
            assert!(celula.altura >= 0.0);
        }
    }

    /// A foto cabe inteira na célula, centralizada, e não é esticada.
    #[test]
    fn a_foto_cabe_inteira_na_celula_sem_esticar() {
        let celula = Celula {
            x: 10.0,
            y: 20.0,
            largura: 100.0,
            altura: 50.0,
        };

        // Uma foto 3:2 é mais "quadrada" que a célula 2:1 — quem manda é a altura.
        let deitada = encaixar(&celula, 3.0 / 2.0);
        assert!(perto(deitada.largura / deitada.altura, 3.0 / 2.0));
        assert!(deitada.altura <= celula.altura);
        assert!(deitada.largura <= celula.largura);
        // Centralizada nos dois eixos.
        assert!(perto(
            deitada.x + deitada.largura / 2.0,
            celula.x + celula.largura / 2.0
        ));
        assert!(perto(
            deitada.y + deitada.altura / 2.0,
            celula.y + celula.altura / 2.0
        ));

        // Um retrato 2:3 na mesma célula continua com a proporção dele.
        let em_pe = encaixar(&celula, 2.0 / 3.0);
        assert!(perto(em_pe.largura / em_pe.altura, 2.0 / 3.0));
        assert!(em_pe.altura <= celula.altura);
        assert!(
            em_pe.largura < deitada.largura,
            "retrato e paisagem não podem sair do mesmo tamanho"
        );
    }

    /// ⚠️ Foto sem proporção conhecida não vira `NaN` no meio do layout.
    ///
    /// A miniatura chega depois da célula, e enquanto ela não chegou o aspecto é
    /// um `0.0` ou um `0/0`. Um `NaN` num `div` do GPUI não falha: ele some da
    /// tela, e a célula parece não ter foto.
    #[test]
    fn aspecto_invalido_devolve_retangulo_vazio_e_nao_nan() {
        let celula = Celula {
            x: 0.0,
            y: 0.0,
            largura: 100.0,
            altura: 100.0,
        };

        for aspecto in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let foto = encaixar(&celula, aspecto);
            assert!(foto.largura.is_finite() && foto.altura.is_finite());
            assert_eq!((foto.largura, foto.altura), (0.0, 0.0));
        }
    }

    /// O respiro é o do legado: a foto ocupa 95% da célula, e a moldura continua
    /// aparecendo em volta.
    #[test]
    fn a_foto_deixa_o_respiro_da_celula_a_mostra() {
        let celula = Celula {
            x: 0.0,
            y: 0.0,
            largura: 100.0,
            altura: 100.0,
        };
        let foto = encaixar(&celula, 1.0);

        assert!(perto(foto.largura, 95.0));
        assert!(perto(foto.altura, 95.0));
    }
}
