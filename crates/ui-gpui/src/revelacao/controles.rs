//! Quais controles existem, em que painel, e o que cada um move.
//!
//! Uma tabela, e não 171 blocos de interface iguais. O `crates/ui` gastou 290
//! linhas para *um* slider (`components/advanced_slider.rs`) e depois repetiu o
//! bloco 42 vezes espalhado por `dock_viewer.rs`; aqui o HSL inteiro — 24
//! controles — são 24 linhas de dados.
//!
//! ## ✅ Desde 2026-09-17 são os 171 do motor, um por campo
//!
//! A tabela é o porte de `revelacao/ajustes.ts` do site, grupo a grupo, com os
//! mesmos rótulos, faixas e casas: Básico, Curva de tons, Curva por ponto, HSL,
//! Preto e branco, Detalhe, Lente, Calibração, Tonalização e Efeitos na aba
//! **sRGB**, e os cinco módulos em RGB linear (Exposição, Sombras e realces,
//! Monocromático, Vinhetagem e Color balance) na aba **RGB**. Até aqui eram 53,
//! e os outros 118 só chegavam por preset, pela receita do site ou por
//! sincronização — sem como vê-los nem desfazê-los um a um.
//!
//! ⚠️ **Os 36 da curva por ponto estão na tabela, mas não viram slider.** Eles
//! existem aqui para o invariante "um controle por campo" continuar valendo
//! (`todo_ajuste_tem_um_controle`); quem os desenha é o editor de curva
//! (`tela/painel.rs`), como no site.
//!
//! 🚧 **Divergência D7 do contrato da foto**: numa foto do catálogo local os 118
//! novos ainda não têm coluna (`persistencia::SEM_COLUNA_NO_BANCO_LOCAL`) e
//! somem ao reabrir. Na foto do site eles viajam inteiros pela receita.

use super::processador::Ajustes;

/// A família de um controle — o que ele move, e não onde ele é desenhado.
///
/// ⚠️ **Seção não é painel.** As três famílias de HSL continuam separadas aqui
/// (um controle sabe se move saturação, luminância ou matiz), mas na tela elas
/// dividem **um** painel com três abas — o desenho do site, e o do Lightroom.
/// Quem decide o que aparece é [`Painel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Secao {
    Basico,
    CurvaDeTons,
    CurvaPorPonto,
    HslCor,
    HslLuminancia,
    HslMatiz,
    PretoEBranco,
    Detalhe,
    Lente,
    Calibracao,
    Tonalizacao,
    Efeitos,
    RgbExposicao,
    RgbSombrasERealces,
    RgbMonocromatico,
    RgbVinhetagem,
    RgbColorBalance,
}

impl Secao {
    pub const TODAS: [Secao; 17] = [
        Secao::Basico,
        Secao::CurvaDeTons,
        Secao::CurvaPorPonto,
        Secao::HslCor,
        Secao::HslLuminancia,
        Secao::HslMatiz,
        Secao::PretoEBranco,
        Secao::Detalhe,
        Secao::Lente,
        Secao::Calibracao,
        Secao::Tonalizacao,
        Secao::Efeitos,
        Secao::RgbExposicao,
        Secao::RgbSombrasERealces,
        Secao::RgbMonocromatico,
        Secao::RgbVinhetagem,
        Secao::RgbColorBalance,
    ];

    /// O nome inteiro, para diagnóstico de teste e para o `id` da aba.
    pub fn rotulo(&self) -> &'static str {
        match self {
            Secao::Basico => "Básico",
            Secao::CurvaDeTons => "Curva de tons",
            Secao::CurvaPorPonto => "Curva por ponto",
            Secao::HslCor => "HSL / cor",
            Secao::HslLuminancia => "HSL / luminância",
            Secao::HslMatiz => "HSL / matiz",
            Secao::PretoEBranco => "Preto e branco",
            Secao::Detalhe => "Detalhe",
            Secao::Lente => "Lente",
            Secao::Calibracao => "Calibração",
            Secao::Tonalizacao => "Tonalização",
            Secao::Efeitos => "Efeitos",
            Secao::RgbExposicao => "RGB / Exposição",
            Secao::RgbSombrasERealces => "RGB / Sombras e realces",
            Secao::RgbMonocromatico => "RGB / Monocromático",
            Secao::RgbVinhetagem => "RGB / Vinhetagem",
            Secao::RgbColorBalance => "RGB / Color balance",
        }
    }

    /// Em que painel ela é desenhada.
    pub fn painel(&self) -> Painel {
        match self {
            Secao::Basico => Painel::Basico,
            Secao::CurvaDeTons => Painel::CurvaDeTons,
            Secao::CurvaPorPonto => Painel::CurvaPorPonto,
            Secao::HslCor | Secao::HslLuminancia | Secao::HslMatiz => Painel::Hsl,
            Secao::PretoEBranco => Painel::PretoEBranco,
            Secao::Detalhe => Painel::Detalhe,
            Secao::Lente => Painel::Lente,
            Secao::Calibracao => Painel::Calibracao,
            Secao::Tonalizacao => Painel::Tonalizacao,
            Secao::Efeitos => Painel::Efeitos,
            Secao::RgbExposicao => Painel::RgbExposicao,
            Secao::RgbSombrasERealces => Painel::RgbSombrasERealces,
            Secao::RgbMonocromatico => Painel::RgbMonocromatico,
            Secao::RgbVinhetagem => Painel::RgbVinhetagem,
            Secao::RgbColorBalance => Painel::RgbColorBalance,
        }
    }
}

/// Um painel sanfonado da coluna da direita.
///
/// 🔑 **São os do site** (`revelacao/paineis.tsx`), nas duas abas: dez na sRGB e
/// cinco na RGB, cada aba na ordem em que o shader aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Painel {
    Basico,
    CurvaDeTons,
    CurvaPorPonto,
    Hsl,
    PretoEBranco,
    Detalhe,
    Lente,
    Calibracao,
    Tonalizacao,
    Efeitos,
    RgbExposicao,
    RgbSombrasERealces,
    RgbMonocromatico,
    RgbVinhetagem,
    RgbColorBalance,
}

impl Painel {
    /// Todos, na ordem da tabela: a aba sRGB e depois a RGB.
    pub const TODOS: [Painel; 15] = [
        Painel::Basico,
        Painel::CurvaDeTons,
        Painel::CurvaPorPonto,
        Painel::Hsl,
        Painel::PretoEBranco,
        Painel::Detalhe,
        Painel::Lente,
        Painel::Calibracao,
        Painel::Tonalizacao,
        Painel::Efeitos,
        Painel::RgbExposicao,
        Painel::RgbSombrasERealces,
        Painel::RgbMonocromatico,
        Painel::RgbVinhetagem,
        Painel::RgbColorBalance,
    ];

    /// A aba sRGB — os controles de sempre, sobre a foto com gama.
    ///
    /// 🔑 **A ordem é a do pipeline** (`paineis.tsx:159-181`): a curva por ponto
    /// logo depois da paramétrica, o mixer de P&B depois do HSL, a calibração
    /// antes da tonalização, e o virador e o grão por último.
    pub const SRGB: [Painel; 10] = [
        Painel::Basico,
        Painel::CurvaDeTons,
        Painel::CurvaPorPonto,
        Painel::Hsl,
        Painel::PretoEBranco,
        Painel::Detalhe,
        Painel::Lente,
        Painel::Calibracao,
        Painel::Tonalizacao,
        Painel::Efeitos,
    ];

    /// A aba RGB — os módulos em RGB linear, na ordem do pipeline do darktable.
    ///
    /// ⚠️ **Na tela eles não levam o nome "darktable"** (dono, 2026-09-12:
    /// *"esse nome darktable suja os controles"*).
    pub const RGB: [Painel; 5] = [
        Painel::RgbExposicao,
        Painel::RgbSombrasERealces,
        Painel::RgbMonocromatico,
        Painel::RgbVinhetagem,
        Painel::RgbColorBalance,
    ];

    pub fn rotulo(&self) -> &'static str {
        match self {
            Painel::Basico => "Básico",
            Painel::CurvaDeTons => "Curva de tons",
            Painel::CurvaPorPonto => "Curva por ponto",
            Painel::Hsl => "HSL",
            Painel::PretoEBranco => "Preto e branco",
            Painel::Detalhe => "Detalhe",
            Painel::Lente => "Lente",
            Painel::Calibracao => "Calibração",
            Painel::Tonalizacao => "Tonalização",
            Painel::Efeitos => "Efeitos",
            Painel::RgbExposicao => "Exposição",
            Painel::RgbSombrasERealces => "Sombras e realces",
            Painel::RgbMonocromatico => "Monocromático",
            Painel::RgbVinhetagem => "Vinhetagem",
            Painel::RgbColorBalance => "Color balance",
        }
    }

    /// Onde a abertura deste painel fica lembrada — a chave do site
    /// (`revelacao:<título>`, e `revelacao:curva-por-ponto`).
    pub fn chave(&self) -> String {
        match self {
            Painel::CurvaPorPonto => "revelacao:curva-por-ponto".to_string(),
            outro => format!("revelacao:{}", outro.rotulo()),
        }
    }

    /// Se ele mora na aba RGB.
    pub fn no_rgb(&self) -> bool {
        Painel::RGB.contains(self)
    }

    /// As famílias que ele desenha. Uma só, menos o HSL — que tem as três, e é
    /// por isso que ele tem abas.
    pub fn secoes(&self) -> &'static [Secao] {
        match self {
            Painel::Basico => &[Secao::Basico],
            Painel::CurvaDeTons => &[Secao::CurvaDeTons],
            Painel::CurvaPorPonto => &[Secao::CurvaPorPonto],
            Painel::Hsl => &[Secao::HslCor, Secao::HslLuminancia, Secao::HslMatiz],
            Painel::PretoEBranco => &[Secao::PretoEBranco],
            Painel::Detalhe => &[Secao::Detalhe],
            Painel::Lente => &[Secao::Lente],
            Painel::Calibracao => &[Secao::Calibracao],
            Painel::Tonalizacao => &[Secao::Tonalizacao],
            Painel::Efeitos => &[Secao::Efeitos],
            Painel::RgbExposicao => &[Secao::RgbExposicao],
            Painel::RgbSombrasERealces => &[Secao::RgbSombrasERealces],
            Painel::RgbMonocromatico => &[Secao::RgbMonocromatico],
            Painel::RgbVinhetagem => &[Secao::RgbVinhetagem],
            Painel::RgbColorBalance => &[Secao::RgbColorBalance],
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
    /// no primeiro).
    pub fn nasce_aberto(&self) -> bool {
        matches!(self, Painel::Basico)
    }
}

/// Um controle: onde ele mora, o rótulo, a faixa e por onde ele escreve.
#[derive(Clone, Copy)]
pub struct Definicao {
    pub secao: Secao,
    pub rotulo: &'static str,
    pub minimo: f32,
    pub maximo: f32,
    /// Quantas casas mostrar ao lado do rótulo — as do site.
    pub casas: usize,
    /// Se o valor positivo leva `+`. Exposição `+0,30` diz "clareou";
    /// contraste `1,30` não é "mais 1,30", é um multiplicador.
    pub com_sinal: bool,
    /// Se o controle só aceita inteiros — os interruptores (`bw_ativo`, os
    /// "Ligar" da aba RGB). Com o passo fino dos outros, um interruptor
    /// pararia em `0,37`, que o motor lê como desligado sem ninguém saber.
    pub discreto: bool,
    pub aplicar: fn(&mut Ajustes, f32),
    pub ler: fn(&Ajustes) -> f32,
}

/// Quantas posições distintas toda barra contínua precisa oferecer.
///
/// 🔑 **É cerca de uma por pixel de barra.** O painel tem 320px e a barra ocupa
/// pouco mais de 200 deles; abaixo disso o punho pula pixels visivelmente, e o
/// que se sente não é "grosso", é **lento** — foi como o defeito chegou
/// (*"os controles não estão fluidos"*).
const POSICOES_MINIMAS: f32 = 200.0;

impl Definicao {
    /// Onde o slider nasce.
    ///
    /// 🔑 Vem de [`Ajustes::default`], e **não** de um número escrito aqui. Os
    /// neutros não são todos zero (contraste, raio de nitidez, mistura, a curva
    /// por ponto e metade da aba RGB), e ter dois lugares dizendo qual é o
    /// neutro é ter um deles errado mais cedo ou mais tarde.
    pub fn neutro(&self) -> f32 {
        (self.ler)(&Ajustes::default())
    }

    /// Se este controle está fora do neutro — a mesma comparação exata do site
    /// (`ajustes[c.campo] !== PADRAO[c.campo]`).
    pub fn alterado(&self, ajustes: &Ajustes) -> bool {
        (self.ler)(ajustes) != self.neutro()
    }

    /// O menor movimento que este controle aceita.
    ///
    /// 🚨 **O padrão do `Slider` é 1,0, e ele arredonda o valor ao passo.** Com
    /// ele, a exposição tinha onze posições na barra inteira (*"ele faz de 1 em
    /// 1 e não quebra 1.01"*).
    ///
    /// 🔑 **O rótulo é um piso, e não a resposta**: nenhum controle contínuo
    /// tem menos de [`POSICOES_MINIMAS`] posições, que é o que faz o arrasto
    /// ser contínuo aos olhos. Os discretos andam de um em um.
    pub fn passo(&self) -> f32 {
        if self.discreto {
            return 1.0;
        }
        let pelo_rotulo = 10f32.powi(-(self.casas as i32));
        let pela_barra = (self.maximo - self.minimo) / POSICOES_MINIMAS;
        pelo_rotulo.min(pela_barra)
    }

    /// O valor como o site o escreve (`formatarValor`): vírgula decimal, e o
    /// `+` só no positivo — o neutro sai `0,00`, e não `+0,00`.
    pub fn formatar(&self, valor: f32) -> String {
        let texto = format!("{:.*}", self.casas, valor).replace('.', ",");
        if self.com_sinal && valor > 0.0 {
            format!("+{texto}")
        } else {
            texto
        }
    }
}

/// Um controle qualquer: família, rótulo, campo, faixa, casas e sinal.
macro_rules! def {
    ($secao:expr, $rotulo:literal, $campo:ident, $minimo:expr, $maximo:expr, $casas:expr, $sinal:expr) => {
        Definicao {
            secao: $secao,
            rotulo: $rotulo,
            minimo: $minimo,
            maximo: $maximo,
            casas: $casas,
            com_sinal: $sinal,
            discreto: false,
            aplicar: |a, v| a.$campo = v,
            ler: |a| a.$campo,
        }
    };
}

/// A faixa −100..100 inteira com sinal — o `cem` do site.
macro_rules! cem {
    ($secao:expr, $rotulo:literal, $campo:ident) => {
        def!($secao, $rotulo, $campo, -100.0, 100.0, 0, true)
    };
}

/// A faixa −1..1 com duas casas — o `unitario` do site.
macro_rules! unitario {
    ($rotulo:literal, $campo:ident) => {
        def!(Secao::Basico, $rotulo, $campo, -1.0, 1.0, 2, true)
    };
}

/// Uma escolha de cor na roda inteira (0–360°) — o `matiz` do site.
///
/// 🔑 **Não é o desvio do HSL.** Os oito matizes do HSL vão de −180 a 180
/// porque giram a cor que o pixel já tem; estes **escolhem** a cor que vai
/// entrar — 35° é o âmbar da sépia.
macro_rules! matiz {
    ($secao:expr, $rotulo:literal, $campo:ident) => {
        def!($secao, $rotulo, $campo, 0.0, 360.0, 0, false)
    };
}

/// A quantidade 0..100 sem sinal.
macro_rules! cento {
    ($secao:expr, $rotulo:literal, $campo:ident) => {
        def!($secao, $rotulo, $campo, 0.0, 100.0, 0, false)
    };
}

/// Uma faixa do darktable — o `faixa` do site: sinal quando a faixa desce
/// abaixo de zero.
///
/// 🚨 **As faixas são as `$MIN`/`$MAX` do darktable 5.6.1, e não as do slider
/// dele.** Um estilo gravado com exposição +5 não cabe em −3..+4; um valor fora
/// da faixa encostaria no limite sem erro nenhum.
macro_rules! faixa {
    ($secao:expr, $rotulo:literal, $campo:ident, $minimo:expr, $maximo:expr, $casas:expr) => {
        def!(
            $secao,
            $rotulo,
            $campo,
            $minimo,
            $maximo,
            $casas,
            ($minimo as f32) < 0.0
        )
    };
}

/// Um liga/desliga: o módulo só age com ele em 1.
macro_rules! interruptor {
    ($secao:expr, $rotulo:literal, $campo:ident) => {
        Definicao {
            discreto: true,
            ..def!($secao, $rotulo, $campo, 0.0, 1.0, 0, false)
        }
    };
}

/// As oito cores de uma família do HSL.
macro_rules! hsl {
    ($secao:expr, $min:expr, $max:expr, $r:ident, $o:ident, $y:ident, $g:ident, $a:ident, $b:ident, $p:ident, $m:ident) => {
        [
            def!($secao, "Vermelho", $r, $min, $max, 0, true),
            def!($secao, "Laranja", $o, $min, $max, 0, true),
            def!($secao, "Amarelo", $y, $min, $max, 0, true),
            def!($secao, "Verde", $g, $min, $max, 0, true),
            def!($secao, "Água", $a, $min, $max, 0, true),
            def!($secao, "Azul", $b, $min, $max, 0, true),
            def!($secao, "Roxo", $p, $min, $max, 0, true),
            def!($secao, "Magenta", $m, $min, $max, 0, true),
        ]
    };
}

/// Um ponto da curva por ponto: 0–255, inteiro.
macro_rules! ponto {
    ($rotulo:literal, $campo:ident) => {
        def!(Secao::CurvaPorPonto, $rotulo, $campo, 0.0, 255.0, 0, false)
    };
}

const HSL_COR: [Definicao; 8] = hsl!(
    Secao::HslCor,
    -100.0,
    100.0,
    hsl_red_sat,
    hsl_orange_sat,
    hsl_yellow_sat,
    hsl_green_sat,
    hsl_aqua_sat,
    hsl_blue_sat,
    hsl_purple_sat,
    hsl_magenta_sat
);
const HSL_LUMINANCIA: [Definicao; 8] = hsl!(
    Secao::HslLuminancia,
    -100.0,
    100.0,
    hsl_red_lum,
    hsl_orange_lum,
    hsl_yellow_lum,
    hsl_green_lum,
    hsl_aqua_lum,
    hsl_blue_lum,
    hsl_purple_lum,
    hsl_magenta_lum
);
// −180 a 180 porque matiz é um círculo: o dobro da faixa das outras duas.
const HSL_MATIZ: [Definicao; 8] = hsl!(
    Secao::HslMatiz,
    -180.0,
    180.0,
    hsl_red_hue,
    hsl_orange_hue,
    hsl_yellow_hue,
    hsl_green_hue,
    hsl_aqua_hue,
    hsl_blue_hue,
    hsl_purple_hue,
    hsl_magenta_hue
);

use Secao as S;

/// Os 171 controles, na ordem em que a coluna da direita os desenha: a aba
/// sRGB (a ordem de `paineis.tsx`) e depois a RGB.
///
/// ⚠️ **O Básico é o primeiro**, e os testes da tela contam com isso
/// (`controles[0]` é a exposição).
pub const CONTROLES: &[Definicao] = &[
    // ---------------------------------------------------------------- Básico
    def!(S::Basico, "Exposição", exposure, -5.0, 5.0, 2, true),
    def!(S::Basico, "Contraste", contrast, 0.0, 2.0, 2, false),
    def!(S::Basico, "Temperatura", temperature, -10.0, 10.0, 1, true),
    def!(S::Basico, "Matiz", tint, -10.0, 10.0, 1, true),
    cem!(S::Basico, "Altas luzes", highlights),
    cem!(S::Basico, "Sombras", shadows),
    cem!(S::Basico, "Brancos", whites),
    cem!(S::Basico, "Pretos", blacks),
    unitario!("Textura", clarity),
    unitario!("Intensidade", vibrance),
    unitario!("Saturação", saturation),
    // --------------------------------------------------------- Curva de tons
    cem!(S::CurvaDeTons, "Sombras", tone_curve_shadows),
    cem!(S::CurvaDeTons, "Escuros", tone_curve_darks),
    cem!(S::CurvaDeTons, "Claros", tone_curve_lights),
    cem!(S::CurvaDeTons, "Altas luzes", tone_curve_highlights),
    // ------------------------------------------------------- Curva por ponto
    ponto!("RGB — ponto 1", curva_m0),
    ponto!("RGB — ponto 2", curva_m1),
    ponto!("RGB — ponto 3", curva_m2),
    ponto!("RGB — ponto 4", curva_m3),
    ponto!("RGB — ponto 5", curva_m4),
    ponto!("RGB — ponto 6", curva_m5),
    ponto!("RGB — ponto 7", curva_m6),
    ponto!("RGB — ponto 8", curva_m7),
    ponto!("RGB — ponto 9", curva_m8),
    ponto!("Vermelho — ponto 1", curva_r0),
    ponto!("Vermelho — ponto 2", curva_r1),
    ponto!("Vermelho — ponto 3", curva_r2),
    ponto!("Vermelho — ponto 4", curva_r3),
    ponto!("Vermelho — ponto 5", curva_r4),
    ponto!("Vermelho — ponto 6", curva_r5),
    ponto!("Vermelho — ponto 7", curva_r6),
    ponto!("Vermelho — ponto 8", curva_r7),
    ponto!("Vermelho — ponto 9", curva_r8),
    ponto!("Verde — ponto 1", curva_g0),
    ponto!("Verde — ponto 2", curva_g1),
    ponto!("Verde — ponto 3", curva_g2),
    ponto!("Verde — ponto 4", curva_g3),
    ponto!("Verde — ponto 5", curva_g4),
    ponto!("Verde — ponto 6", curva_g5),
    ponto!("Verde — ponto 7", curva_g6),
    ponto!("Verde — ponto 8", curva_g7),
    ponto!("Verde — ponto 9", curva_g8),
    ponto!("Azul — ponto 1", curva_b0),
    ponto!("Azul — ponto 2", curva_b1),
    ponto!("Azul — ponto 3", curva_b2),
    ponto!("Azul — ponto 4", curva_b3),
    ponto!("Azul — ponto 5", curva_b4),
    ponto!("Azul — ponto 6", curva_b5),
    ponto!("Azul — ponto 7", curva_b6),
    ponto!("Azul — ponto 8", curva_b7),
    ponto!("Azul — ponto 9", curva_b8),
    // ------------------------------------------------------------------- HSL
    HSL_COR[0],
    HSL_COR[1],
    HSL_COR[2],
    HSL_COR[3],
    HSL_COR[4],
    HSL_COR[5],
    HSL_COR[6],
    HSL_COR[7],
    HSL_LUMINANCIA[0],
    HSL_LUMINANCIA[1],
    HSL_LUMINANCIA[2],
    HSL_LUMINANCIA[3],
    HSL_LUMINANCIA[4],
    HSL_LUMINANCIA[5],
    HSL_LUMINANCIA[6],
    HSL_LUMINANCIA[7],
    HSL_MATIZ[0],
    HSL_MATIZ[1],
    HSL_MATIZ[2],
    HSL_MATIZ[3],
    HSL_MATIZ[4],
    HSL_MATIZ[5],
    HSL_MATIZ[6],
    HSL_MATIZ[7],
    // -------------------------------------------------------- Preto e branco
    // 🔑 O `bw_ativo` é o interruptor, e os oito dormem sem ele — um preset de
    // cor que traga `GrayMixer` dentro não dessatura a foto sozinho.
    interruptor!(S::PretoEBranco, "Converter para P&B", bw_ativo),
    cem!(S::PretoEBranco, "Vermelhos", bw_red),
    cem!(S::PretoEBranco, "Laranjas", bw_orange),
    cem!(S::PretoEBranco, "Amarelos", bw_yellow),
    cem!(S::PretoEBranco, "Verdes", bw_green),
    cem!(S::PretoEBranco, "Águas", bw_aqua),
    cem!(S::PretoEBranco, "Azuis", bw_blue),
    cem!(S::PretoEBranco, "Roxos", bw_purple),
    cem!(S::PretoEBranco, "Magentas", bw_magenta),
    // --------------------------------------------------------------- Detalhe
    cento!(S::Detalhe, "Ruído (luminância)", nr_luminance),
    cento!(S::Detalhe, "Ruído (cor)", nr_color),
    cento!(S::Detalhe, "Nitidez", sharpen_amount),
    // Começa em 0,5: raio zero seria não ter pixel de vizinhança.
    def!(
        S::Detalhe,
        "Raio da nitidez",
        sharpen_radius,
        0.5,
        3.0,
        1,
        false
    ),
    // ----------------------------------------------------------------- Lente
    cem!(S::Lente, "Distorção", lens_distortion),
    cem!(S::Lente, "Vinheta", lens_vignette_amount),
    cento!(S::Lente, "Meio da vinheta", lens_vignette_midpoint),
    // ------------------------------------------------------------ Calibração
    cem!(S::Calibracao, "Sombras — matiz", calib_shadow_tint),
    matiz!(S::Calibracao, "Vermelho — matiz", calib_red_hue),
    cem!(S::Calibracao, "Vermelho — saturação", calib_red_sat),
    matiz!(S::Calibracao, "Verde — matiz", calib_green_hue),
    cem!(S::Calibracao, "Verde — saturação", calib_green_sat),
    matiz!(S::Calibracao, "Azul — matiz", calib_blue_hue),
    cem!(S::Calibracao, "Azul — saturação", calib_blue_sat),
    // ----------------------------------------------------------- Tonalização
    // As três faixas e o global do Color Grading; a Mistura abre em 50.
    matiz!(S::Tonalizacao, "Sombras — matiz", split_shadow_hue),
    cento!(S::Tonalizacao, "Sombras — saturação", split_shadow_sat),
    matiz!(S::Tonalizacao, "Tons médios — matiz", split_midtone_hue),
    cento!(S::Tonalizacao, "Tons médios — saturação", split_midtone_sat),
    matiz!(S::Tonalizacao, "Altas luzes — matiz", split_highlight_hue),
    cento!(
        S::Tonalizacao,
        "Altas luzes — saturação",
        split_highlight_sat
    ),
    matiz!(S::Tonalizacao, "Global — matiz", split_global_hue),
    cento!(S::Tonalizacao, "Global — saturação", split_global_sat),
    cem!(S::Tonalizacao, "Balanço", split_balance),
    cento!(S::Tonalizacao, "Mistura", split_blending),
    // --------------------------------------------------------------- Efeitos
    cento!(S::Efeitos, "Grão", grain_amount),
    cento!(S::Efeitos, "Tamanho do grão", grain_size),
    // ============================================================ aba RGB
    // ------------------------------------------------------------- Exposição
    interruptor!(S::RgbExposicao, "Ligar", dt_exposure_ativo),
    faixa!(
        S::RgbExposicao,
        "Exposição (EV)",
        dt_exposure_exposure,
        -18.0,
        18.0,
        3
    ),
    faixa!(
        S::RgbExposicao,
        "Correção do nível de preto",
        dt_exposure_black,
        -1.0,
        1.0,
        4
    ),
    // ----------------------------------------------------- Sombras e realces
    interruptor!(S::RgbSombrasERealces, "Ligar", dt_shadhi_ativo),
    faixa!(
        S::RgbSombrasERealces,
        "Sombras",
        dt_shadhi_shadows,
        -100.0,
        100.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Realces",
        dt_shadhi_highlights,
        -100.0,
        100.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Ajuste do ponto branco",
        dt_shadhi_whitepoint,
        -10.0,
        10.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Raio (px da foto original)",
        dt_shadhi_radius,
        0.1,
        500.0,
        1
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Compressão",
        dt_shadhi_compress,
        0.0,
        100.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Cor das sombras",
        dt_shadhi_shadows_ccorrect,
        0.0,
        100.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Cor dos realces",
        dt_shadhi_highlights_ccorrect,
        0.0,
        100.0,
        2
    ),
    faixa!(
        S::RgbSombrasERealces,
        "Limites (flags UNBOUND)",
        dt_shadhi_flags,
        0.0,
        255.0,
        0
    ),
    // --------------------------------------------------------- Monocromático
    interruptor!(S::RgbMonocromatico, "Ligar", dt_monochrome_ativo),
    faixa!(
        S::RgbMonocromatico,
        "Filtro — a (verde ↔ magenta)",
        dt_monochrome_a,
        -128.0,
        128.0,
        2
    ),
    faixa!(
        S::RgbMonocromatico,
        "Filtro — b (azul ↔ amarelo)",
        dt_monochrome_b,
        -128.0,
        128.0,
        2
    ),
    faixa!(
        S::RgbMonocromatico,
        "Largura do filtro",
        dt_monochrome_size,
        0.1,
        10.0,
        2
    ),
    faixa!(
        S::RgbMonocromatico,
        "Preservar realces",
        dt_monochrome_highlights,
        0.0,
        1.0,
        3
    ),
    // ------------------------------------------------------------ Vinhetagem
    interruptor!(S::RgbVinhetagem, "Ligar", dt_vignette_ativo),
    faixa!(
        S::RgbVinhetagem,
        "Início da queda (%)",
        dt_vignette_scale,
        0.0,
        200.0,
        2
    ),
    faixa!(
        S::RgbVinhetagem,
        "Raio da queda (%)",
        dt_vignette_falloff_scale,
        0.0,
        200.0,
        2
    ),
    faixa!(
        S::RgbVinhetagem,
        "Brilho",
        dt_vignette_brightness,
        -1.0,
        1.0,
        3
    ),
    faixa!(
        S::RgbVinhetagem,
        "Saturação",
        dt_vignette_saturation,
        -1.0,
        1.0,
        3
    ),
    faixa!(
        S::RgbVinhetagem,
        "Centro — horizontal",
        dt_vignette_center_x,
        -1.0,
        1.0,
        3
    ),
    faixa!(
        S::RgbVinhetagem,
        "Centro — vertical",
        dt_vignette_center_y,
        -1.0,
        1.0,
        3
    ),
    interruptor!(
        S::RgbVinhetagem,
        "Proporção automática",
        dt_vignette_autoratio
    ),
    faixa!(
        S::RgbVinhetagem,
        "Proporção largura/altura",
        dt_vignette_whratio,
        0.0,
        2.0,
        3
    ),
    faixa!(S::RgbVinhetagem, "Forma", dt_vignette_shape, 0.0, 5.0, 3),
    interruptor!(
        S::RgbVinhetagem,
        "Sem recorte de valores",
        dt_vignette_unbound
    ),
    // --------------------------------------------------------- Color balance
    // ⚠️ Croma, saturação e brilho são frações, como o darktable grava.
    interruptor!(S::RgbColorBalance, "Ligar", dt_cb_ativo),
    faixa!(
        S::RgbColorBalance,
        "Sombras — luminância",
        dt_cb_shadows_y,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Sombras — croma",
        dt_cb_shadows_c,
        0.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Sombras — matiz",
        dt_cb_shadows_h,
        0.0,
        360.0,
        2
    ),
    faixa!(
        S::RgbColorBalance,
        "Meios-tons — luminância",
        dt_cb_midtones_y,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Meios-tons — croma",
        dt_cb_midtones_c,
        0.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Meios-tons — matiz",
        dt_cb_midtones_h,
        0.0,
        360.0,
        2
    ),
    faixa!(
        S::RgbColorBalance,
        "Realces — luminância",
        dt_cb_highlights_y,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Realces — croma",
        dt_cb_highlights_c,
        0.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Realces — matiz",
        dt_cb_highlights_h,
        0.0,
        360.0,
        2
    ),
    faixa!(
        S::RgbColorBalance,
        "Global — luminância",
        dt_cb_global_y,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Global — croma",
        dt_cb_global_c,
        0.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Global — matiz",
        dt_cb_global_h,
        0.0,
        360.0,
        2
    ),
    faixa!(
        S::RgbColorBalance,
        "Deslocamento de matiz",
        dt_cb_hue_angle,
        -180.0,
        180.0,
        2
    ),
    faixa!(
        S::RgbColorBalance,
        "Vibração global",
        dt_cb_vibrance,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Contraste",
        dt_cb_contrast,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Croma — global",
        dt_cb_chroma_global,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Croma — sombras",
        dt_cb_chroma_shadows,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Croma — meios-tons",
        dt_cb_chroma_midtones,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Croma — realces",
        dt_cb_chroma_highlights,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Saturação — global",
        dt_cb_saturation_global,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Saturação — sombras",
        dt_cb_saturation_shadows,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Saturação — meios-tons",
        dt_cb_saturation_midtones,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Saturação — realces",
        dt_cb_saturation_highlights,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Brilho — global",
        dt_cb_brilliance_global,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Brilho — sombras",
        dt_cb_brilliance_shadows,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Brilho — meios-tons",
        dt_cb_brilliance_midtones,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Brilho — realces",
        dt_cb_brilliance_highlights,
        -1.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Máscara — queda das sombras",
        dt_cb_shadows_weight,
        0.0,
        3.0,
        3
    ),
    faixa!(
        S::RgbColorBalance,
        "Máscara — queda dos realces",
        dt_cb_highlights_weight,
        0.0,
        3.0,
        3
    ),
    faixa!(
        S::RgbColorBalance,
        "Máscara — fulcro branco (EV)",
        dt_cb_white_fulcrum,
        -16.0,
        16.0,
        3
    ),
    faixa!(
        S::RgbColorBalance,
        "Máscara — fulcro cinza",
        dt_cb_mask_grey_fulcrum,
        0.0,
        1.0,
        4
    ),
    faixa!(
        S::RgbColorBalance,
        "Fulcro cinza do contraste",
        dt_cb_grey_fulcrum,
        0.0,
        1.0,
        4
    ),
];

/// Quantos campos do motor estão fora do neutro — **todos**, e não só os que
/// têm slider. É a conta do cabeçalho do site
/// (`NOMES_DOS_AJUSTES.filter(n => ajustes[n] !== PADRAO[n])`).
pub fn quantos_fora_do_neutro(ajustes: &Ajustes) -> usize {
    let neutro = Ajustes::default().como_vetor();
    ajustes
        .como_vetor()
        .iter()
        .zip(neutro.iter())
        .filter(|(a, n)| a != n)
        .count()
}

/// Se algum controle desta família saiu do neutro.
pub fn secao_alterada(ajustes: &Ajustes, secao: Secao) -> bool {
    CONTROLES
        .iter()
        .filter(|d| d.secao == secao)
        .any(|d| d.alterado(ajustes))
}

/// Se algum controle deste painel saiu do neutro — o ponto âmbar.
pub fn painel_alterado(ajustes: &Ajustes, painel: Painel) -> bool {
    painel
        .secoes()
        .iter()
        .any(|secao| secao_alterada(ajustes, *secao))
}

/// Se a aba (RGB ou sRGB) tem algum ajuste — o ponto âmbar da aba.
pub fn aba_alterada(ajustes: &Ajustes, rgb: bool) -> bool {
    let paineis: &[Painel] = if rgb { &Painel::RGB } else { &Painel::SRGB };
    paineis.iter().any(|p| painel_alterado(ajustes, *p))
}

/// Em que campo do [`Ajustes`] este controle lê — pelo que ele devolve, e não
/// por um nome escrito à mão.
#[cfg(test)]
pub(crate) fn campo_do_controle(def: &Definicao) -> &'static str {
    const BASE: f32 = 10_000.0;
    let vetor: Vec<f32> = (0..Ajustes::NOMES.len()).map(|i| BASE + i as f32).collect();
    let ajustes = Ajustes::de_vetor(&vetor).expect("o vetor tem o tamanho de `NOMES`");
    let lido = (def.ler)(&ajustes);
    let posicao = (lido - BASE) as usize;
    assert!(
        vetor.get(posicao) == Some(&lido),
        "`{}` não lê campo nenhum do `Ajustes`",
        def.rotulo
    );
    Ajustes::NOMES[posicao]
}

#[cfg(test)]
mod passo_dos_controles {
    use super::*;

    /// 🚨 **O Lightroom chega a `+1,55` na exposição, e o app não chegava.**
    #[test]
    fn a_exposicao_alcanca_um_virgula_cinco_cinco() {
        let exposicao = CONTROLES
            .iter()
            .find(|d| d.rotulo == "Exposição" && d.secao == Secao::Basico)
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

    /// ⚠️ **Barra que pula pixel não parece grossa, parece lenta** — menos nos
    /// interruptores, que são dois estados e não uma faixa.
    #[test]
    fn toda_barra_continua_tem_uma_posicao_por_pixel() {
        for definicao in CONTROLES.iter().filter(|d| !d.discreto) {
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

    /// 🚨 **Interruptor anda de um em um.** Com passo fino ele pararia em
    /// `0,37`, que o motor lê como desligado sem ninguém saber.
    #[test]
    fn o_interruptor_so_tem_dois_estados() {
        let interruptores: Vec<_> = CONTROLES.iter().filter(|d| d.discreto).collect();
        assert_eq!(interruptores.len(), 8, "bw_ativo, 5 módulos e 2 da vinheta");
        for d in interruptores {
            assert_eq!(
                (d.minimo, d.maximo, d.passo()),
                (0.0, 1.0, 1.0),
                "{}",
                d.rotulo
            );
        }
    }

    /// O número sai como o site o escreve: vírgula, e `+` só no positivo.
    #[test]
    fn o_valor_sai_como_no_site() {
        let exposicao = &CONTROLES[0];
        assert_eq!(exposicao.formatar(0.0), "0,00");
        assert_eq!(exposicao.formatar(1.5), "+1,50");
        assert_eq!(exposicao.formatar(-0.3), "-0,30");
        let contraste = &CONTROLES[1];
        assert_eq!(contraste.formatar(1.3), "1,30");
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 Todo controle nasce no neutro **e** dentro da própria faixa.
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

    /// 🔑 Cada controle escreve num campo **diferente**, e lê de volta o que
    /// escreveu.
    #[test]
    fn cada_controle_move_um_campo_proprio() {
        for (i, def) in CONTROLES.iter().enumerate() {
            let mut ajustes = Ajustes::default();
            let marca = def.minimo + (def.maximo - def.minimo) * 0.25 + 0.125;
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

    /// ✅ **Todo ajuste do motor tem exatamente um controle** — os 171, como no
    /// site (`TODOS_OS_CONTROLES`).
    ///
    /// 🚨 **Até 2026-09-17 eram 53**, e os outros 118 moravam numa lista
    /// `AINDA_SO_NO_SITE`. A fonte é `Ajustes::NOMES`, nome por nome: ajuste novo
    /// no motor sem controle aqui falha com o nome.
    #[test]
    fn todo_ajuste_tem_um_controle() {
        let mut com_controle = std::collections::BTreeMap::new();
        for def in CONTROLES.iter() {
            let nome = campo_do_controle(def);
            if let Some(outro) = com_controle.insert(nome, def.rotulo) {
                panic!("`{nome}` tem dois controles: `{outro}` e `{}`", def.rotulo);
            }
        }
        for nome in Ajustes::NOMES {
            assert!(
                com_controle.contains_key(nome),
                "`{nome}` não tem controle no desktop"
            );
        }
        assert_eq!(CONTROLES.len(), Ajustes::NOMES.len());
    }

    /// Toda seção declarada tem pelo menos um controle.
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
    }

    /// As duas abas somam todos os painéis, sem repetir nenhum.
    #[test]
    fn as_duas_abas_cobrem_todos_os_paineis() {
        let juntas: Vec<Painel> = Painel::SRGB.into_iter().chain(Painel::RGB).collect();
        assert_eq!(juntas, Painel::TODOS.to_vec());
        for p in Painel::RGB {
            assert!(p.no_rgb());
        }
        for p in Painel::SRGB {
            assert!(!p.no_rgb());
        }
    }

    /// 🚨 **A ordem dos painéis é a ordem da tabela**, e é a do site.
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

    /// As chaves da lembrança são as do site.
    #[test]
    fn as_chaves_sao_as_do_site() {
        assert_eq!(Painel::Basico.chave(), "revelacao:Básico");
        assert_eq!(Painel::Hsl.chave(), "revelacao:HSL");
        assert_eq!(Painel::CurvaPorPonto.chave(), "revelacao:curva-por-ponto");
        assert_eq!(Painel::RgbExposicao.chave(), "revelacao:Exposição");
    }

    /// 🚨 **O cabeçalho conta os 171**, e o neutro não conta nada — nem o
    /// cinza do darktable, nem a identidade da curva.
    #[test]
    fn o_cabecalho_conta_todos_os_campos() {
        assert_eq!(quantos_fora_do_neutro(&Ajustes::default()), 0);
        let ajustes = Ajustes {
            exposure: 1.0,
            dt_cb_ativo: 1.0,
            curva_b3: 10.0,
            ..Ajustes::default()
        };
        assert_eq!(quantos_fora_do_neutro(&ajustes), 3);
        assert!(aba_alterada(&ajustes, true));
        assert!(aba_alterada(&ajustes, false));
        assert!(painel_alterado(&ajustes, Painel::CurvaPorPonto));
        assert!(!painel_alterado(&ajustes, Painel::Hsl));
    }
}
