//! Quais sliders existem, em que seção, e o que cada um move.
//!
//! Uma tabela, e não 42 blocos de interface iguais. O `crates/ui` gastou 290
//! linhas para *um* slider (`components/advanced_slider.rs`) e depois repetiu o
//! bloco 42 vezes espalhado por `dock_viewer.rs`; aqui o HSL inteiro — 24
//! controles — são 24 linhas de dados.
//!
//! ## ✅ E desde 2026-09-06 há Tonalização e Efeitos
//!
//! Sete controles novos, e eles não vieram do app antigo: o motor ganhou
//! tonalização (a cor das sombras e a das altas luzes, separadas) e grão de
//! filme porque **não havia como fazer sépia** — temperatura e matiz agem antes
//! da saturação, e numa foto em preto e branco a cor que eles pintam é apagada
//! pelo passo seguinte. O pedido veio do site, que usa o mesmo `revelacao-core`;
//! aqui eles entram pelo mesmo motivo de sempre: um fotógrafo faz isto no
//! Lightroom, e a foto revelada nos dois lugares tem de sair igual.
//!
//! ## 🚨 A curva de tons não está aqui, e é de propósito
//!
//! `Ajustes` tem `tone_curve_shadows`, `_darks`, `_lights` e `_highlights`, e o
//! shader os aplica. Mas **no legado nenhum controle os escreve**: os únicos
//! escritores são o reset, o undo/redo, a carga do banco e a aplicação de
//! preset (conferido em 15/ago/2026, `grep -rn "active_tone_curve"`). A seção
//! "Tone Curve" de lá desenha um gráfico calculado a partir de exposição,
//! contraste, altas luzes, sombras, brancos e pretos — ela não toca nos quatro
//! parâmetros que levam o nome dela.
//!
//! Dar slider a eles seria **feature nova**, que a regra §7.1 proíbe: com ela,
//! qualquer diferença entre os dois apps deixa de ser conferível — não dá para
//! saber se é defeito de porte ou escopo que só um dos lados tem.
//!
//! ## 🚨 E 18 destes 42 não movem a foto — nem aqui, nem no `crates/ui`
//!
//! O `struct Params` do WGSL declara 28 campos para os 46 que a CPU manda, e o
//! `uniform` casa por **posição**. O efeito na tabela abaixo:
//!
//! - **Básico** (11) e **HSL / cor** (8): chegam certos.
//! - **HSL / matiz**: vermelho borra (o shader lê aquele campo como
//!   `nr_luminance`), amarelo e verde aplicam ruído de cor e nitidez, e os outros
//!   cinco não fazem nada.
//! - **HSL / luminância** (8), **Detalhe** (4) e **Lente** (3): nada.
//!
//! A tabela posição a posição está em
//! [`super::processador`], presa por
//! `o_wgsl_declara_28_campos_para_os_46_que_o_rust_manda`. **Não é para
//! consertar aqui**: o shader é o mesmo arquivo dos dois apps e a fase 2 se mede
//! por igualdade de pixel com o de egui — conserto é trabalho próprio, nos dois
//! lados, com o critério da fase ajustado junto.

use super::processador::Ajustes;

/// A família de um controle — o que ele move, e não onde ele é desenhado.
///
/// ⚠️ **Seção não é painel desde 7/set/2026.** As três famílias de HSL
/// continuam separadas aqui (um controle sabe se move saturação, luminância ou
/// matiz), mas na tela elas dividem **um** painel com três abas — o desenho do
/// site, e o do Lightroom. Quem decide o que aparece é [`Painel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Secao {
    Basico,
    CurvaDeTons,
    HslCor,
    HslLuminancia,
    HslMatiz,
    Detalhe,
    Lente,
    Tonalizacao,
    Efeitos,
}

impl Secao {
    pub const TODAS: [Secao; 9] = [
        Secao::Basico,
        Secao::CurvaDeTons,
        Secao::HslCor,
        Secao::HslLuminancia,
        Secao::HslMatiz,
        Secao::Detalhe,
        Secao::Lente,
        Secao::Tonalizacao,
        Secao::Efeitos,
    ];

    /// O nome inteiro, para diagnóstico de teste e para o `title` da aba.
    pub fn rotulo(&self) -> &'static str {
        match self {
            Secao::Basico => "Básico",
            Secao::CurvaDeTons => "Curva de tons",
            Secao::HslCor => "HSL / cor",
            Secao::HslLuminancia => "HSL / luminância",
            Secao::HslMatiz => "HSL / matiz",
            Secao::Detalhe => "Detalhe",
            Secao::Lente => "Lente",
            Secao::Tonalizacao => "Tonalização",
            Secao::Efeitos => "Efeitos",
        }
    }

    /// Em que painel ela é desenhada.
    pub fn painel(&self) -> Painel {
        match self {
            Secao::Basico => Painel::Basico,
            Secao::CurvaDeTons => Painel::CurvaDeTons,
            Secao::HslCor | Secao::HslLuminancia | Secao::HslMatiz => Painel::Hsl,
            Secao::Detalhe => Painel::Detalhe,
            Secao::Lente => Painel::Lente,
            Secao::Tonalizacao => Painel::Tonalizacao,
            Secao::Efeitos => Painel::Efeitos,
        }
    }
}

/// Um painel sanfonado da coluna da direita, na ordem em que ele aparece.
///
/// 🔑 **São os sete do site**, na ordem do site
/// (`revelacao/paineis.tsx`): Básico, Curva de tons, HSL, Detalhe, Lente,
/// Tonalização e Efeitos. Aqui eram nove, com HSL ocupando três — e a coluna
/// de 280px ficava com três cabeçalhos quase iguais em sequência, que é
/// exatamente o que as abas resolvem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Painel {
    Basico,
    CurvaDeTons,
    Hsl,
    Detalhe,
    Lente,
    Tonalizacao,
    Efeitos,
}

impl Painel {
    pub const TODOS: [Painel; 7] = [
        Painel::Basico,
        Painel::CurvaDeTons,
        Painel::Hsl,
        Painel::Detalhe,
        Painel::Lente,
        Painel::Tonalizacao,
        Painel::Efeitos,
    ];

    pub fn rotulo(&self) -> &'static str {
        match self {
            Painel::Basico => "Básico",
            Painel::CurvaDeTons => "Curva de tons",
            Painel::Hsl => "HSL",
            Painel::Detalhe => "Detalhe",
            Painel::Lente => "Lente",
            Painel::Tonalizacao => "Tonalização",
            Painel::Efeitos => "Efeitos",
        }
    }

    /// As famílias que ele desenha. Uma só, menos o HSL — que tem as três, e é
    /// por isso que ele tem abas.
    pub fn secoes(&self) -> &'static [Secao] {
        match self {
            Painel::Basico => &[Secao::Basico],
            Painel::CurvaDeTons => &[Secao::CurvaDeTons],
            Painel::Hsl => &[Secao::HslCor, Secao::HslLuminancia, Secao::HslMatiz],
            Painel::Detalhe => &[Secao::Detalhe],
            Painel::Lente => &[Secao::Lente],
            Painel::Tonalizacao => &[Secao::Tonalizacao],
            Painel::Efeitos => &[Secao::Efeitos],
        }
    }

    /// O rótulo da aba, quando o painel tem mais de uma família.
    ///
    /// São os do site: "Cor", "Luminância" e "Matiz" — e não "HSL / cor", que
    /// repetiria o nome do painel dentro dele três vezes.
    pub fn aba(secao: Secao) -> &'static str {
        match secao {
            Secao::HslCor => "Cor",
            Secao::HslLuminancia => "Luminância",
            Secao::HslMatiz => "Matiz",
            outra => outra.rotulo(),
        }
    }

    /// Só o Básico nasce aberto — é o que o site faz (`<Secao … aberta />` só
    /// no primeiro) e o que o legado fazia. Com 53 controles, abrir tudo daria
    /// uma coluna de dois metros e nenhum deles seria encontrado.
    pub fn nasce_aberto(&self) -> bool {
        matches!(self, Painel::Basico)
    }
}

/// Um controle: onde ele mora, o rótulo, a faixa e por onde ele escreve.
pub struct Definicao {
    pub secao: Secao,
    pub rotulo: &'static str,
    pub minimo: f32,
    pub maximo: f32,
    /// Quantas casas mostrar ao lado do rótulo — o app de egui usa `{:+.2}` na
    /// exposição, `{:.2}` no contraste e `{:+.0}` em tudo que vai de -100 a 100.
    pub casas: usize,
    /// Se o valor merece sinal explícito. Exposição `+0,30` diz "clareou";
    /// contraste `1,30` não é "mais 1,30", é um multiplicador.
    pub com_sinal: bool,
    pub aplicar: fn(&mut Ajustes, f32),
    pub ler: fn(&Ajustes) -> f32,
}

/// Quantas posições distintas toda barra precisa oferecer.
///
/// 🔑 **É cerca de uma por pixel de barra.** O painel tem 320px e a barra ocupa
/// pouco mais de 200 deles; abaixo disso o punho pula pixels visivelmente, e o
/// que se sente não é "grosso", é **lento** — foi como o defeito chegou
/// (*"os controles não estão fluidos"*), depois de o quadro já estar medido em
/// 3,94 ms.
const POSICOES_MINIMAS: f32 = 200.0;

impl Definicao {
    /// Onde o slider nasce.
    ///
    /// 🔑 Vem de [`Ajustes::default`], e **não** de um número escrito aqui. Os
    /// neutros não são todos zero (contraste é 1.0, raio de nitidez é 1.0), e
    /// ter dois lugares dizendo qual é o neutro é ter um deles errado mais cedo
    /// ou mais tarde — com o sintoma de a foto abrir alterada e o slider parado
    /// no meio, parecendo certo.
    pub fn neutro(&self) -> f32 {
        (self.ler)(&Ajustes::default())
    }

    /// O menor movimento que este controle aceita.
    ///
    /// 🚨 **O padrão do `Slider` é 1,0, e ele arredonda o valor ao passo**
    /// (`(valor / passo).round() * passo`, `gpui-component`). Com ele, a
    /// exposição — que vai de −5 a +5 — tinha **onze posições na barra inteira**,
    /// e o contraste, que vive entre 0 e 2 em torno de 1,0, tinha três. O painel
    /// mostrava `+0.00` com duas casas e não havia gesto capaz de produzir
    /// `+1.55`: arrastar dava saltos, e a sensação era de controle emperrado —
    /// que foi como o dono descreveu (*"ele faz de 1 em 1 e não quebra 1.01"*).
    ///
    /// 🔑 **Sai de [`Self::casas`], e não de uma tabela nova.** O número de casas
    /// já é a decisão de quanto este controle distingue: se a tela mostra duas,
    /// o passo não pode ser mais grosso que 0,01. Uma segunda tabela seria um
    /// segundo lugar dizendo a mesma coisa, com os dois divergindo no dia em que
    /// alguém mexesse só num.
    ///
    /// 🚨 **Mas o rótulo é um piso, e não a resposta.** No Lightroom o arrasto é
    /// contínuo e o número ao lado é a **leitura arredondada** dele — não o
    /// contrário. Derivar o passo só das casas deixava o raio da nitidez com 25
    /// posições em toda a barra (0,5 a 3,0 de 0,1 em 0,1): o punho anda aos
    /// saltos, e saltar parece travar. Daí o segundo termo: nenhum controle tem
    /// menos de [`POSICOES_MINIMAS`] posições, que é cerca de uma por pixel de
    /// barra — o que faz o arrasto ser contínuo aos olhos.
    ///
    /// O resultado é o do Lightroom: exposição andando fino e escrita `+1,55`,
    /// altas luzes andando de um em um e escritas `−41`.
    pub fn passo(&self) -> f32 {
        let pelo_rotulo = 10f32.powi(-(self.casas as i32));
        let pela_barra = (self.maximo - self.minimo) / POSICOES_MINIMAS;
        pelo_rotulo.min(pela_barra)
    }

    pub fn formatar(&self, valor: f32) -> String {
        if self.com_sinal {
            format!("{:+.*}", self.casas, valor)
        } else {
            format!("{:.*}", self.casas, valor)
        }
    }
}

/// Atalho para as três famílias de HSL, que só diferem no campo e na faixa.
macro_rules! hsl {
    ($secao:expr, $rotulo:literal, $campo:ident, $minimo:literal, $maximo:literal) => {
        Definicao {
            secao: $secao,
            rotulo: $rotulo,
            minimo: $minimo,
            maximo: $maximo,
            casas: 0,
            com_sinal: true,
            aplicar: |a, v| a.$campo = v,
            ler: |a| a.$campo,
        }
    };
}

/// As quatro zonas da curva de tons.
///
/// A faixa é -100 a 100, como no Lightroom, e é a mesma que o shader espera: ele
/// multiplica por `0.01` para virar fração.
macro_rules! curva {
    ($rotulo:literal, $campo:ident) => {
        Definicao {
            secao: Secao::CurvaDeTons,
            rotulo: $rotulo,
            minimo: -100.0,
            maximo: 100.0,
            casas: 0,
            com_sinal: true,
            aplicar: |a, v| a.$campo = v,
            ler: |a| a.$campo,
        }
    };
}

/// Os 53 controles, na ordem em que a coluna da direita os desenha.
///
/// 🔑 **A ordem é a do site** (`ajustes.ts`, `TODOS_OS_CONTROLES`): Básico,
/// Curva de tons, HSL nas três famílias, Detalhe, Lente, Tonalização e
/// Efeitos. Aqui o Detalhe vinha antes do HSL, e a coluna da direita saía com
/// os painéis em ordem diferente da do site — mesmo trabalho, dois desenhos.
///
/// ⚠️ **As faixas são as do `crates/ui`**, lidas uma a uma de
/// `docking/dock_viewer.rs`. Não são arredondamentos bonitos: contraste vai de 0
/// a 2 porque é multiplicador, temperatura de -10 a 10 porque é a escala do
/// shader, matiz de -180 a 180 porque é um círculo de cor, e o raio de nitidez
/// começa em 0,5 porque raio zero não tem pixel. Mudar qualquer uma faria o
/// mesmo arrasto dar resultado diferente nos dois apps — que é exatamente o que
/// a paridade da fase 2 mede.
pub const CONTROLES: &[Definicao] = &[
    // ---------------------------------------------------------------- Básico
    Definicao {
        secao: Secao::Basico,
        rotulo: "Exposição",
        minimo: -5.0,
        maximo: 5.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.exposure = v,
        ler: |a| a.exposure,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Contraste",
        minimo: 0.0,
        maximo: 2.0,
        casas: 2,
        com_sinal: false,
        aplicar: |a, v| a.contrast = v,
        ler: |a| a.contrast,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Temperatura",
        minimo: -10.0,
        maximo: 10.0,
        casas: 1,
        com_sinal: true,
        aplicar: |a, v| a.temperature = v,
        ler: |a| a.temperature,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Matiz",
        minimo: -10.0,
        maximo: 10.0,
        casas: 1,
        com_sinal: true,
        aplicar: |a, v| a.tint = v,
        ler: |a| a.tint,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Altas luzes",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.highlights = v,
        ler: |a| a.highlights,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Sombras",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.shadows = v,
        ler: |a| a.shadows,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Brancos",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.whites = v,
        ler: |a| a.whites,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Pretos",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.blacks = v,
        ler: |a| a.blacks,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Textura",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.clarity = v,
        ler: |a| a.clarity,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Intensidade",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.vibrance = v,
        ler: |a| a.vibrance,
    },
    Definicao {
        secao: Secao::Basico,
        rotulo: "Saturação",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.saturation = v,
        ler: |a| a.saturation,
    },
    // -------------------------------------------------- Curva de tons
    // As quatro zonas paramétricas do Lightroom, e as quatro que o shader já
    // aplicava sozinho: sombras (centro 0,125), escuros (0,375), claros (0,625)
    // e altas luzes (0,875), cada uma com meia-largura de 0,25.
    //
    // 🚨 **Elas existiam no `Ajustes` e no shader desde sempre, e nenhum controle
    // as escrevia** — nem aqui, nem no app de egui, cuja seção "Tone Curve"
    // desenhava um gráfico a partir dos ajustes do Básico e não tocava nos
    // parâmetros que levam o nome dela.
    curva!("Sombras", tone_curve_shadows),
    curva!("Escuros", tone_curve_darks),
    curva!("Claros", tone_curve_lights),
    curva!("Altas luzes", tone_curve_highlights),
    // -------------------------------------------------------------- HSL / cor
    hsl!(Secao::HslCor, "Vermelho", hsl_red_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Laranja", hsl_orange_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Amarelo", hsl_yellow_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Verde", hsl_green_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Água", hsl_aqua_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Azul", hsl_blue_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Roxo", hsl_purple_sat, -100.0, 100.0),
    hsl!(Secao::HslCor, "Magenta", hsl_magenta_sat, -100.0, 100.0),
    // ------------------------------------------------------- HSL / luminância
    hsl!(Secao::HslLuminancia, "Vermelho", hsl_red_lum, -100.0, 100.0),
    hsl!(
        Secao::HslLuminancia,
        "Laranja",
        hsl_orange_lum,
        -100.0,
        100.0
    ),
    hsl!(
        Secao::HslLuminancia,
        "Amarelo",
        hsl_yellow_lum,
        -100.0,
        100.0
    ),
    hsl!(Secao::HslLuminancia, "Verde", hsl_green_lum, -100.0, 100.0),
    hsl!(Secao::HslLuminancia, "Água", hsl_aqua_lum, -100.0, 100.0),
    hsl!(Secao::HslLuminancia, "Azul", hsl_blue_lum, -100.0, 100.0),
    hsl!(Secao::HslLuminancia, "Roxo", hsl_purple_lum, -100.0, 100.0),
    hsl!(
        Secao::HslLuminancia,
        "Magenta",
        hsl_magenta_lum,
        -100.0,
        100.0
    ),
    // ------------------------------------------------------------ HSL / matiz
    // -180 a 180 porque matiz é um círculo: o dobro da faixa das outras duas
    // famílias, e copiar -100..100 aqui limitaria o giro a pouco mais da metade.
    hsl!(Secao::HslMatiz, "Vermelho", hsl_red_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Laranja", hsl_orange_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Amarelo", hsl_yellow_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Verde", hsl_green_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Água", hsl_aqua_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Azul", hsl_blue_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Roxo", hsl_purple_hue, -180.0, 180.0),
    hsl!(Secao::HslMatiz, "Magenta", hsl_magenta_hue, -180.0, 180.0),
    // --------------------------------------------------------------- Detalhe
    Definicao {
        secao: Secao::Detalhe,
        rotulo: "Ruído (luminância)",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.nr_luminance = v,
        ler: |a| a.nr_luminance,
    },
    Definicao {
        secao: Secao::Detalhe,
        rotulo: "Ruído (cor)",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.nr_color = v,
        ler: |a| a.nr_color,
    },
    Definicao {
        secao: Secao::Detalhe,
        rotulo: "Nitidez",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.sharpen_amount = v,
        ler: |a| a.sharpen_amount,
    },
    // Começa em 0,5 e não em 0: raio zero seria não ter pixel de vizinhança para
    // comparar, e a nitidez não faria nada com o controle no mínimo.
    Definicao {
        secao: Secao::Detalhe,
        rotulo: "Raio da nitidez",
        minimo: 0.5,
        maximo: 3.0,
        casas: 1,
        com_sinal: false,
        aplicar: |a, v| a.sharpen_radius = v,
        ler: |a| a.sharpen_radius,
    },
    // ----------------------------------------------------------------- Lente
    Definicao {
        secao: Secao::Lente,
        rotulo: "Distorção",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.lens_distortion = v,
        ler: |a| a.lens_distortion,
    },
    Definicao {
        secao: Secao::Lente,
        rotulo: "Vinheta",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.lens_vignette_amount = v,
        ler: |a| a.lens_vignette_amount,
    },
    Definicao {
        secao: Secao::Lente,
        rotulo: "Meio da vinheta",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.lens_vignette_midpoint = v,
        ler: |a| a.lens_vignette_midpoint,
    },
    // ---------------------------------------------------------- Tonalização
    //
    // 🔑 **O matiz aqui é a roda de cor inteira (0–360°), e não o desvio do
    // HSL.** Os oito matizes do HSL vão de -180 a 180 porque giram a cor que o
    // pixel já tem; estes dois **escolhem** a cor que vai entrar — 35° é o
    // âmbar da sépia, 210° o azul das sombras frias. Copiar a faixa do vizinho
    // tiraria metade da roda do alcance de quem arrasta, e o slider ainda
    // andaria: ninguém repararia.
    Definicao {
        secao: Secao::Tonalizacao,
        rotulo: "Sombras — matiz",
        minimo: 0.0,
        maximo: 360.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.split_shadow_hue = v,
        ler: |a| a.split_shadow_hue,
    },
    Definicao {
        secao: Secao::Tonalizacao,
        rotulo: "Sombras — saturação",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.split_shadow_sat = v,
        ler: |a| a.split_shadow_sat,
    },
    Definicao {
        secao: Secao::Tonalizacao,
        rotulo: "Altas luzes — matiz",
        minimo: 0.0,
        maximo: 360.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.split_highlight_hue = v,
        ler: |a| a.split_highlight_hue,
    },
    Definicao {
        secao: Secao::Tonalizacao,
        rotulo: "Altas luzes — saturação",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.split_highlight_sat = v,
        ler: |a| a.split_highlight_sat,
    },
    Definicao {
        secao: Secao::Tonalizacao,
        rotulo: "Balanço",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.split_balance = v,
        ler: |a| a.split_balance,
    },
    // --------------------------------------------------------------- Efeitos
    Definicao {
        secao: Secao::Efeitos,
        rotulo: "Grão",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.grain_amount = v,
        ler: |a| a.grain_amount,
    },
    Definicao {
        secao: Secao::Efeitos,
        rotulo: "Tamanho do grão",
        minimo: 0.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: false,
        aplicar: |a, v| a.grain_size = v,
        ler: |a| a.grain_size,
    },
];

#[cfg(test)]
mod passo_dos_controles {
    use super::*;

    /// 🚨 **O Lightroom chega a `+1,55` na exposição, e o app não chegava.**
    ///
    /// O `Slider` do `gpui-component` nasce com passo 1,0 e arredonda o valor a
    /// ele. A exposição vai de −5 a +5: eram **onze posições na barra inteira**,
    /// e o painel mostrando `+0.00` prometia duas casas que gesto nenhum
    /// produzia. Foi o que o dono descreveu como controle não fluido — *"ele faz
    /// de 1 em 1 e não quebra 1.01"*.
    #[test]
    fn a_exposicao_alcanca_um_virgula_cinco_cinco() {
        let exposicao = CONTROLES
            .iter()
            .find(|d| d.rotulo == "Exposição")
            .expect("a exposição está na tabela");

        let passo = exposicao.passo();
        let alcancado = (1.55 / passo).round() * passo;
        assert!(
            (alcancado - 1.55).abs() < 1e-4,
            "com passo {passo} o mais perto de +1,55 é {alcancado}"
        );
    }

    /// ⚠️ **O passo nunca pode ser mais grosso que a precisão que o rótulo
    /// mostra.**
    ///
    /// Mostrar duas casas e andar de um em um é prometer uma precisão que o
    /// gesto não entrega. O contrário — passo mais fino que o rótulo — é o
    /// desenho do Lightroom, e é o que faz o arrasto ser contínuo com um número
    /// legível ao lado.
    #[test]
    fn o_passo_nunca_e_mais_grosso_que_o_rotulo() {
        for definicao in CONTROLES {
            let do_rotulo = 10f32.powi(-(definicao.casas as i32));
            assert!(
                definicao.passo() <= do_rotulo + 1e-6,
                "{}: passo {} para um rótulo de {} casas",
                definicao.rotulo,
                definicao.passo(),
                definicao.casas
            );
        }
    }

    /// ⚠️ **Barra que pula pixel não parece grossa, parece lenta.**
    ///
    /// Três posições no contraste (0, 1, 2) era o caso extremo: o neutro é 1,0,
    /// e o único movimento possível era dobrar ou zerar. Vinte e cinco no raio
    /// da nitidez era o caso silencioso — andava, mas aos saltos.
    #[test]
    fn toda_barra_tem_uma_posicao_por_pixel() {
        for definicao in CONTROLES {
            let posicoes = (definicao.maximo - definicao.minimo) / definicao.passo();
            assert!(
                posicoes >= POSICOES_MINIMAS - 1.0,
                "{}: {posicoes:.0} posições de {} a {}",
                definicao.rotulo,
                definicao.minimo,
                definicao.maximo
            );
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 Todo controle nasce no neutro **e** dentro da própria faixa.
    ///
    /// Dois casos pegam: contraste, neutro `1.0` numa faixa de `0..2`, e o raio
    /// da nitidez, neutro `1.0` numa faixa que começa em `0,5`. Se alguém copiar
    /// a faixa do vizinho (`-100..100`), o slider nasceria fora do lugar e a foto
    /// abriria com o ajuste no extremo — sem erro, parecendo escolha de quem
    /// desenhou a tela.
    #[test]
    fn todo_neutro_cabe_na_faixa() {
        for def in CONTROLES {
            let neutro = def.neutro();
            assert!(
                neutro >= def.minimo && neutro <= def.maximo,
                "`{}` ({}) nasce em {neutro}, fora de {}..{}",
                def.rotulo,
                def.secao.rotulo(),
                def.minimo,
                def.maximo
            );
        }
    }

    #[test]
    fn nenhuma_faixa_esta_invertida() {
        for def in CONTROLES {
            assert!(
                def.minimo < def.maximo,
                "`{}` tem faixa invertida",
                def.rotulo
            );
        }
    }

    /// 🔑 Cada controle escreve num campo **diferente**.
    ///
    /// Dois `aplicar` apontando para o mesmo campo é o erro de copiar-e-colar
    /// desta tabela, e ele não falha: um slider simplesmente deixa de fazer
    /// efeito e o outro passa a responder por dois. Com 24 linhas de HSL geradas
    /// por macro, a chance de trocar `hsl_blue_lum` por `hsl_blue_sat` é alta e a
    /// de perceber olhando é baixa.
    #[test]
    fn cada_controle_move_um_campo_proprio() {
        for (i, def) in CONTROLES.iter().enumerate() {
            let mut ajustes = Ajustes::default();
            let marca = def.minimo + (def.maximo - def.minimo) * 0.25;
            (def.aplicar)(&mut ajustes, marca);

            for (j, outro) in CONTROLES.iter().enumerate() {
                if i == j {
                    assert_eq!(
                        (def.ler)(&ajustes),
                        marca,
                        "`{}` não lê de volta o que escreveu",
                        def.rotulo
                    );
                } else {
                    assert_eq!(
                        (outro.ler)(&ajustes),
                        outro.neutro(),
                        "mexer em `{} / {}` mexeu em `{} / {}` — os dois escrevem no mesmo campo",
                        def.secao.rotulo(),
                        def.rotulo,
                        outro.secao.rotulo(),
                        outro.rotulo
                    );
                }
            }
        }
    }

    /// ✅ **Um controle por ajuste: 53 e 53.**
    ///
    /// 🚨 **Eram 42 para 46 até 17/ago/2026**, e a diferença era a curva de tons:
    /// os quatro `tone_curve_*` existiam no `Ajustes`, o shader os aplicava, e
    /// **nada os escrevia** — nem aqui, nem no app de egui, cuja seção "Tone
    /// Curve" desenhava um gráfico a partir dos ajustes do Básico sem tocar nos
    /// parâmetros que levam o nome dela.
    ///
    /// O teste antigo travava o número em 42 de propósito, contra o impulso de
    /// "completar a tabela": enquanto o alvo era o app antigo, dar slider a eles
    /// era feature nova, e feature nova tornava impossível separar defeito de
    /// porte de escopo divergente. O alvo passou a ser o Lightroom, que **tem**
    /// esses quatro controles, e a trava virou o contrário: agora ela cobra que
    /// nenhum ajuste fique sem quem o escreva.
    #[test]
    fn todo_ajuste_tem_um_controle() {
        assert_eq!(CONTROLES.len(), 53);
        assert_eq!(
            std::mem::size_of::<Ajustes>() / 4,
            CONTROLES.len(),
            "há ajuste sem controle, ou controle a mais"
        );
    }

    /// Toda seção declarada tem pelo menos um controle, e todo controle está
    /// numa seção declarada.
    #[test]
    fn as_secoes_e_os_controles_se_cobrem() {
        for secao in Secao::TODAS {
            assert!(
                CONTROLES.iter().any(|d| d.secao == secao),
                "a seção `{}` não tem controle nenhum — apareceria como cabeçalho vazio",
                secao.rotulo()
            );
        }
    }

    /// 🔑 Cada seção mora em **um** painel, e nenhum painel promete seção que
    /// não existe.
    ///
    /// Um painel sem seção seria um cabeçalho vazio; uma seção em dois painéis
    /// desenharia os mesmos oito sliders duas vezes, com um dos dois grupos
    /// respondendo — o defeito mais caro de achar olhando, porque os dois
    /// parecem certos.
    #[test]
    fn cada_secao_mora_em_um_painel_so() {
        for secao in Secao::TODAS {
            let donos: Vec<Painel> = Painel::TODOS
                .into_iter()
                .filter(|painel| painel.secoes().contains(&secao))
                .collect();
            assert_eq!(
                donos.len(),
                1,
                "`{}` aparece em {} painéis",
                secao.rotulo(),
                donos.len()
            );
            assert_eq!(
                donos[0],
                secao.painel(),
                "`{}` diz um e é desenhada noutro",
                secao.rotulo()
            );
        }

        for painel in Painel::TODOS {
            assert!(
                !painel.secoes().is_empty(),
                "o painel `{}` não desenha nada",
                painel.rotulo()
            );
        }
    }

    /// 🚨 **A ordem dos painéis é a ordem da tabela**, e é a do site.
    ///
    /// A coluna desenha `Painel::TODOS` em sequência e cada painel filtra a
    /// tabela. Se as duas ordens divergirem, nada falha: a tela sai com os
    /// painéis numa ordem e o site na outra, e a diferença só aparece com as
    /// duas telas lado a lado.
    #[test]
    fn os_paineis_seguem_a_ordem_da_tabela() {
        let mut vistos: Vec<Painel> = Vec::new();
        for def in CONTROLES {
            let painel = def.secao.painel();
            if vistos.last() != Some(&painel) && !vistos.contains(&painel) {
                vistos.push(painel);
            }
        }
        assert_eq!(vistos, Painel::TODOS.to_vec());
    }

    /// Os controles estão **agrupados** na tabela, e não intercalados.
    ///
    /// A tela desenha em ordem e abre uma seção nova a cada troca. Uma linha
    /// fora de lugar criaria um segundo cabeçalho "HSL / cor" mais abaixo, com
    /// um controle solto dentro — feio, e difícil de atribuir à tabela.
    #[test]
    fn cada_secao_aparece_uma_vez_so() {
        let mut vistas: Vec<Secao> = Vec::new();
        for def in CONTROLES {
            if vistas.last() != Some(&def.secao) {
                assert!(
                    !vistas.contains(&def.secao),
                    "a seção `{}` aparece em dois blocos",
                    def.secao.rotulo()
                );
                vistas.push(def.secao);
            }
        }
    }
}
