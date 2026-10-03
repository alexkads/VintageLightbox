//! Quais controles existem, em que painel, e o que cada um move.
//!
//! Uma tabela, e não 193 blocos de interface iguais. O `crates/ui` gastou 290
//! linhas para *um* slider (`components/advanced_slider.rs`) e depois repetiu o
//! bloco 42 vezes espalhado por `dock_viewer.rs`; aqui o HSL inteiro — 24
//! controles — são 24 linhas de dados.
//!
//! ## ✅ Desde 2026-09-30 são os 193 do motor, um por campo
//!
//! Os 22 do fim são os controles do Lightroom que faltavam (Textura, Remover
//! névoa, vinheta pós-corte, o resto do Detalhe…).
//!
//! ## Desde 2026-09-17 eram os 171 do motor
//!
//! A tabela é o porte de `revelacao/ajustes.ts` do site, grupo a grupo, com os
//! mesmos rótulos, faixas e casas: Básico, Curva de tons, Curva por ponto, HSL,
//! Preto e branco, Detalhe, Lente, Calibração, Tonalização e Efeitos — os do
//! Lightroom. (A aba **RGB**, com os módulos do darktable, saiu em 2/out/2026:
//! um motor só, e o RecordarFotos P&B refeito com estes.) Até aqui eram 53,
//! e os outros 118 só chegavam por preset, pela revelação do site ou por
//! sincronização — sem como vê-los nem desfazê-los um a um.
//!
//! ⚠️ **Os 36 da curva por ponto estão na tabela, mas não viram slider.** Eles
//! existem aqui para o invariante "um controle por campo" continuar valendo
//! (`todo_ajuste_tem_um_controle`); quem os desenha é o editor de curva
//! (`tela/painel.rs`), como no site. O mesmo vale para os três divisores da
//! Curva de tons (`Secao::RegioesDaCurva`), que a barra de pinos desenha.
//!
//! 🚧 **Divergência D7 do contrato da foto**: numa foto do catálogo local os 118
//! novos ainda não têm coluna (`persistencia::SEM_COLUNA_NO_BANCO_LOCAL`) e
//! somem ao reabrir. Na foto do site eles viajam inteiros pela revelação.

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
    /// 🎞️ O tratamento em P&B (`bw_ativo`): desenhado no alto do Básico,
    /// como no Lightroom — o botão "P&B" e o Perfil —, e sincronizado com a
    /// Mistura de preto e branco, como no site.
    Tratamento,
    CurvaDeTons,
    /// Os três pinos da barra de divisão da Curva de tons. Não viram slider:
    /// quem os desenha é a barra (`tela/painel.rs`).
    RegioesDaCurva,
    CurvaPorPonto,
    HslCor,
    HslLuminancia,
    HslMatiz,
    PretoEBranco,
    Detalhe,
    Lente,
    Calibracao,
    Tonalizacao,
    /// A vinheta pós-corte do Lightroom — o primeiro grupo do painel Efeitos.
    Vinheta,
    /// 🎞️ A vinheta do darktable (o `vignette` do `RecordarFotos P&B`) — o
    /// segundo grupo do painel Efeitos, na ordem do shader. O liga/desliga é
    /// a chave do título, como o botão do módulo no darktable.
    VinhetaDarktable,
    /// A segunda metade do módulo, com o subtítulo dele: "posição / forma".
    VinhetaDarktableForma,
    /// O grão — o último grupo do painel Efeitos.
    Grao,
}

impl Secao {
    pub const TODAS: [Secao; 17] = [
        Secao::Basico,
        Secao::Tratamento,
        Secao::CurvaDeTons,
        Secao::RegioesDaCurva,
        Secao::CurvaPorPonto,
        Secao::HslCor,
        Secao::HslLuminancia,
        Secao::HslMatiz,
        Secao::PretoEBranco,
        Secao::Detalhe,
        Secao::Lente,
        Secao::Calibracao,
        Secao::Tonalizacao,
        Secao::Vinheta,
        Secao::VinhetaDarktable,
        Secao::VinhetaDarktableForma,
        Secao::Grao,
    ];

    /// O nome inteiro, para diagnóstico de teste e para o `id` da aba.
    pub fn rotulo(&self) -> &'static str {
        match self {
            Secao::Basico => "Básico",
            Secao::Tratamento => "Tratamento",
            Secao::CurvaDeTons => "Curva de tons",
            Secao::RegioesDaCurva => "Divisão das regiões",
            Secao::CurvaPorPonto => "Curva por ponto",
            Secao::HslCor => "HSL / cor",
            Secao::HslLuminancia => "HSL / luminância",
            Secao::HslMatiz => "HSL / matiz",
            // 🎞️ O título do grupo, como no Lightroom.
            Secao::PretoEBranco => "Mistura de preto e branco",
            Secao::Detalhe => "Detalhe",
            Secao::Lente => "Lente",
            Secao::Calibracao => "Calibração",
            Secao::Tonalizacao => "Tonalização",
            Secao::Vinheta => "Vinheta de corte posterior",
            Secao::VinhetaDarktable => "Vinheta do darktable",
            Secao::VinhetaDarktableForma => "Posição / forma",
            Secao::Grao => "Granulado",
        }
    }

    /// Em que painel ela é desenhada.
    pub fn painel(&self) -> Painel {
        match self {
            Secao::Basico | Secao::Tratamento => Painel::Basico,
            Secao::CurvaDeTons | Secao::RegioesDaCurva | Secao::CurvaPorPonto => {
                Painel::CurvaDeTons
            }
            Secao::HslCor | Secao::HslLuminancia | Secao::HslMatiz => Painel::Hsl,
            Secao::PretoEBranco => Painel::PretoEBranco,
            Secao::Detalhe => Painel::Detalhe,
            Secao::Lente => Painel::Lente,
            Secao::Calibracao => Painel::Calibracao,
            Secao::Tonalizacao => Painel::Tonalizacao,
            Secao::Vinheta
            | Secao::VinhetaDarktable
            | Secao::VinhetaDarktableForma
            | Secao::Grao => Painel::Efeitos,
        }
    }
}

/// Um painel sanfonado da coluna da direita.
///
/// 🔑 **São os do site** (`revelacao/paineis.tsx`), na ordem em que o shader aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Painel {
    Basico,
    CurvaDeTons,
    Hsl,
    PretoEBranco,
    Detalhe,
    Lente,
    Calibracao,
    Tonalizacao,
    Efeitos,
}

impl Painel {
    /// Todos, na ordem da tabela.
    pub const TODOS: [Painel; 9] = [
        Painel::Basico,
        Painel::CurvaDeTons,
        Painel::Hsl,
        Painel::PretoEBranco,
        Painel::Detalhe,
        Painel::Lente,
        Painel::Calibracao,
        Painel::Tonalizacao,
        Painel::Efeitos,
    ];

    /// A aba Adobe RGB — os controles do Lightroom, sobre a foto lida no
    /// espaço que a câmera gravou (dono, 2/out/2026: a aba sRGB vira Adobe RGB,
    /// e o sRGB sai da tela).
    ///
    /// 🔑 **A ordem é a do pipeline** (`paineis.tsx:159-181`): a curva por ponto
    /// logo depois da paramétrica, o mixer de P&B depois do HSL, a calibração
    /// antes da tonalização, e o virador e o grão por último.
    pub const ADOBE_RGB: [Painel; 9] = [
        Painel::Basico,
        Painel::CurvaDeTons,
        Painel::Hsl,
        Painel::PretoEBranco,
        Painel::Detalhe,
        Painel::Lente,
        Painel::Calibracao,
        Painel::Tonalizacao,
        Painel::Efeitos,
    ];

    pub fn rotulo(&self) -> &'static str {
        match self {
            Painel::Basico => "Básico",
            Painel::CurvaDeTons => "Curva de tons",
            Painel::Hsl => "HSL",
            // 🎞️ O "P & B" do Lightroom, que toma o lugar do HSL.
            Painel::PretoEBranco => "P&B",
            Painel::Detalhe => "Detalhe",
            Painel::Lente => "Lente",
            Painel::Calibracao => "Calibração",
            Painel::Tonalizacao => "Correção de cores",
            Painel::Efeitos => "Efeitos",
        }
    }

    /// Onde a abertura deste painel fica lembrada — a chave do site
    /// (`revelacao:<título>`, e `revelacao:curva-por-ponto`).
    pub fn chave(&self) -> String {
        match self {
            // 🔑 A chave é a de antes de o painel virar "Correção de cores"
            // (2026-09-30): trocar o nome não pode fechar o painel de ninguém.
            Painel::Tonalizacao => "revelacao:Tonalização".to_string(),
            // O mesmo para o "Preto e branco", que virou "P&B" em 2026-10-01.
            Painel::PretoEBranco => "revelacao:Preto e branco".to_string(),
            outro => format!("revelacao:{}", outro.rotulo()),
        }
    }

    /// As famílias que ele desenha. Uma só, menos o HSL — que tem as três, e é
    /// por isso que ele tem abas.
    pub fn secoes(&self) -> &'static [Secao] {
        match self {
            Painel::Basico => &[Secao::Basico, Secao::Tratamento],
            // 🎞️ **Um painel só, como no Lightroom** (dono, 30/09): a
            // paramétrica, os pinos da divisão e a curva por ponto. No site
            // ainda são dois.
            Painel::CurvaDeTons => &[
                Secao::CurvaDeTons,
                Secao::RegioesDaCurva,
                Secao::CurvaPorPonto,
            ],
            Painel::Hsl => &[Secao::HslCor, Secao::HslLuminancia, Secao::HslMatiz],
            Painel::PretoEBranco => &[Secao::PretoEBranco],
            Painel::Detalhe => &[Secao::Detalhe],
            Painel::Lente => &[Secao::Lente],
            Painel::Calibracao => &[Secao::Calibracao],
            Painel::Tonalizacao => &[Secao::Tonalizacao],
            // 🎞️ Os dois grupos do Lightroom, cada um com o seu título, e a
            // vinheta do darktable entre eles — a ordem do shader.
            Painel::Efeitos => &[
                Secao::Vinheta,
                Secao::VinhetaDarktable,
                Secao::VinhetaDarktableForma,
                Secao::Grao,
            ],
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
    /// "Ligar" da Calibração). Com o passo fino dos outros, um interruptor
    /// pararia em `0,37`, que o motor lê como desligado sem ninguém saber.
    pub discreto: bool,
    pub aplicar: fn(&mut Ajustes, f32),
    pub ler: fn(&Ajustes) -> f32,
    /// 🎨 O desenho da barra — ver [`Trilho`].
    pub trilho: Trilho,
    /// Os nomes de cada posição, num controle discreto que é escolha e não
    /// número — o Estilo da vinheta pós-corte. O valor mostra o nome, e na
    /// tela a linha é uma lista (`Select`), e não uma barra.
    pub opcoes: Option<&'static [&'static str]>,
    /// 🎞️ **Quando o controle age** — fora disso ele aparece apagado, como no
    /// Lightroom: o Tamanho do grão com a Intensidade em 0 não muda pixel
    /// nenhum, e uma barra acesa que não faz nada parece defeito. `None` é
    /// sempre.
    pub ativo: Option<fn(&Ajustes) -> bool>,
}

/// 🎨 **O que a barra de um controle desenha** — o trilho colorido do
/// Lightroom (dono, 2026-09-28, depois da POC em WASM).
///
/// A cor só entra onde ela **diz** para onde o controle leva a foto: a
/// Temperatura vai do azul ao amarelo, cada faixa do HSL aparece na própria
/// cor. O resto é [`Trilho::Liso`], preenchido a partir do neutro.
///
/// Os matizes estão em graus (0–360); quem vira cor é o
/// `slider_da_casa::paradas`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trilho {
    Liso,
    /// Do azul ao amarelo.
    Temperatura,
    /// Do verde ao magenta.
    VerdeMagenta,
    /// Do cinza à roda de cores (Intensidade, Saturação).
    Saturacao,
    /// A roda inteira, 0–360° (os matizes da Calibração).
    Roda,
    /// A roda da Gradação de cores do Lightroom: o mesmo número dá a cor que
    /// o Lightroom dá (`revelacao_core::ajustes::cor_da_roda_do_lightroom`),
    /// e não a do HSV.
    RodaDoLightroom,
    /// Do escuro ao claro, em cinza — a luminância das rodas da Correção de
    /// cores.
    Luminancia,
    /// HSL · Cor: do cinza à cor da faixa.
    HslSaturacao(f32),
    /// HSL · Luminância: escuro → a cor → claro.
    HslLuminancia(f32),
    /// HSL · Matiz: a vizinha anterior → a cor → a vizinha seguinte.
    HslMatiz(f32),
}

impl Definicao {
    /// Esta definição com outro trilho — `const` para caber na tabela.
    pub const fn com_trilho(mut self, trilho: Trilho) -> Self {
        self.trilho = trilho;
        self
    }

    /// Esta definição só acesa quando `quando` diz — `const` para caber na
    /// tabela.
    pub const fn ativo_quando(mut self, quando: fn(&Ajustes) -> bool) -> Self {
        self.ativo = Some(quando);
        self
    }

    /// Se o controle age com estes ajustes — ver [`Definicao::ativo`].
    pub fn age(&self, ajustes: &Ajustes) -> bool {
        self.ativo.is_none_or(|quando| quando(ajustes))
    }
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
    /// por ponto), e ter dois lugares dizendo qual é o
    /// neutro é ter um deles errado mais cedo ou mais tarde.
    pub fn neutro(&self) -> f32 {
        (self.ler)(&Ajustes::default())
    }

    /// Se este controle está fora do neutro — a mesma comparação exata do site
    /// (`ajustes[c.campo] !== PADRAO[c.campo]`).
    pub fn alterado(&self, ajustes: &Ajustes) -> bool {
        // A versão de processo não é ajuste (`Ajustes::sem_efeito`): a foto
        // nova nasce no processo 1 e não acende o ponto da Calibração.
        let sem_processo = Ajustes {
            processo: 0.0,
            ..*ajustes
        };
        (self.ler)(&sem_processo) != self.neutro()
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
        if let Some(opcoes) = self.opcoes {
            let i = (valor.round().max(0.0) as usize).min(opcoes.len() - 1);
            return opcoes[i].to_string();
        }
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
            trilho: Trilho::Liso,
            opcoes: None,
            ativo: None,
        }
    };
}

/// A faixa −100..100 inteira com sinal — o `cem` do site.
macro_rules! cem {
    ($secao:expr, $rotulo:literal, $campo:ident) => {
        def!($secao, $rotulo, $campo, -100.0, 100.0, 0, true)
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

/// Uma escolha entre nomes: discreta, de 0 a `n − 1`, e desenhada como lista.
macro_rules! escolha {
    ($secao:expr, $rotulo:literal, $campo:ident, $opcoes:expr) => {
        Definicao {
            discreto: true,
            opcoes: Some($opcoes),
            ..def!(
                $secao,
                $rotulo,
                $campo,
                0.0,
                ($opcoes.len() - 1) as f32,
                0,
                false
            )
        }
    };
}

/// Os estilos da vinheta pós-corte, na ordem do `pcv_style` do motor.
///
/// ⚠️ **O Lightroom em português diz "Destacar prioridade"** no primeiro — é
/// *Highlight Priority* traduzido errado. Aqui é o que ele faz: poupa os
/// realces.
pub const ESTILOS_DA_VINHETA: &[&str] = &[
    "Prioridade de realces",
    "Prioridade de cores",
    "Sobreposição de tinta",
];

/// A vinheta age com alguma intensidade.
fn vinheta_ligada(a: &Ajustes) -> bool {
    a.pcv_amount != 0.0
}

/// Os Realces da vinheta só existem escurecendo, e não na Sobreposição de
/// tinta — é o que o shader faz, e o que o Lightroom apaga.
fn realces_da_vinheta(a: &Ajustes) -> bool {
    a.pcv_amount < 0.0 && a.pcv_style.round() != 2.0
}

/// Os nomes da matização do `vignette.c` (`dt_iop_dither_t`), na ordem do
/// `darktable_vignette_dithering`, como o darktable em português os diz.
pub const MATIZACAO_DO_DARKTABLE: &[&str] = &["Desligada", "Saída de 8 bits", "Saída de 16 bits"];

/// A vinheta do darktable está ligada — a chave do título do grupo.
fn vinheta_do_darktable_ligada(a: &Ajustes) -> bool {
    a.vinheta_do_darktable_ligada()
}

/// A relação largura/altura só vale com a proporção automática desligada —
/// ligada, a forma segue o quadro.
fn proporcao_manual(a: &Ajustes) -> bool {
    a.vinheta_do_darktable_ligada() && a.darktable_vignette_autoratio < 0.5
}

/// O grão age com alguma intensidade.
fn grao_ligado(a: &Ajustes) -> bool {
    a.grain_amount != 0.0
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

/// Um controle do Básico na escala do Lightroom (−100..100, inteiro), sobre um
/// campo que o motor guarda noutra escala: `ler` e `aplicar` fazem a conta.
macro_rules! lightroom {
    ($rotulo:literal, $campo:ident, $ler:expr, $aplicar:expr) => {
        Definicao {
            ler: $ler,
            aplicar: $aplicar,
            ..def!(S::Basico, $rotulo, $campo, -100.0, 100.0, 0, true)
        }
    };
}

/// Uma faixa da Mistura de preto e branco, com a cor da faixa no trilho.
macro_rules! mistura {
    ($rotulo:literal, $campo:ident, $faixa:expr) => {
        cem!(S::PretoEBranco, $rotulo, $campo)
            .com_trilho(Trilho::HslLuminancia(MATIZ_DA_FAIXA[$faixa]))
    };
}

/// A foto não está em P&B — quando Vibração e Saturação agem.
fn colorida(a: &Ajustes) -> bool {
    a.bw_ativo == 0.0
}

/// As oito cores de uma família do HSL.
macro_rules! hsl {
    ($secao:expr, $trilho:path, $min:expr, $max:expr, $r:ident, $o:ident, $y:ident, $g:ident, $a:ident, $b:ident, $p:ident, $m:ident) => {
        [
            def!($secao, "Vermelho", $r, $min, $max, 0, true)
                .com_trilho($trilho(MATIZ_DA_FAIXA[0])),
            def!($secao, "Laranja", $o, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[1])),
            def!($secao, "Amarelo", $y, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[2])),
            def!($secao, "Verde", $g, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[3])),
            def!($secao, "Água", $a, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[4])),
            def!($secao, "Azul", $b, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[5])),
            def!($secao, "Roxo", $p, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[6])),
            def!($secao, "Magenta", $m, $min, $max, 0, true).com_trilho($trilho(MATIZ_DA_FAIXA[7])),
        ]
    };
}

/// O matiz (graus) do centro de cada uma das oito faixas do HSL, na ordem
/// Vermelho, Laranja, Amarelo, Verde, Água, Azul, Roxo, Magenta.
pub const MATIZ_DA_FAIXA: [f32; 8] = [0.0, 30.0, 55.0, 120.0, 180.0, 220.0, 275.0, 315.0];

/// Um ponto da curva por ponto: 0–255, inteiro.
macro_rules! ponto {
    ($rotulo:literal, $campo:ident) => {
        def!(Secao::CurvaPorPonto, $rotulo, $campo, 0.0, 255.0, 0, false)
    };
}

const HSL_COR: [Definicao; 8] = hsl!(
    Secao::HslCor,
    Trilho::HslSaturacao,
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
    Trilho::HslLuminancia,
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
    Trilho::HslMatiz,
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

/// Os controles, na ordem em que a coluna da direita os desenha (a de
/// `paineis.tsx`).
///
/// ⚠️ **O Básico é o primeiro**, e os testes da tela contam com isso
/// (`controles[0]` é a exposição).
pub const CONTROLES: &[Definicao] = &[
    // ---------------------------------------------------------------- Básico
    // 🎞️ **Os nomes e os números do painel Básico do Lightroom em português**
    // (dono, 2026-10-01, com o print do painel). O campo do motor guarda a
    // escala dele — contraste é multiplicador, temperatura vai a ±10 —, e a
    // linha mostra a do Lightroom: um preset que diz "Contraste +7" aparece
    // aqui "+7", e não "1,07". Quem desenha a ordem e os grupos (Tom,
    // Presença) é `tela/painel/basico.rs`.
    def!(S::Basico, "Exposição", exposure, -5.0, 5.0, 2, true),
    lightroom!(
        "Contraste",
        contrast,
        |a| (a.contrast - 1.0) * 100.0,
        |a, v| a.contrast = 1.0 + v / 100.0
    ),
    lightroom!(
        "Temperatura",
        temperature,
        |a| a.temperature * 10.0,
        |a, v| a.temperature = v / 10.0
    )
    .com_trilho(Trilho::Temperatura),
    lightroom!("Colorir", tint, |a| a.tint * 10.0, |a, v| a.tint = v / 10.0)
        .com_trilho(Trilho::VerdeMagenta),
    cem!(S::Basico, "Realces", highlights),
    cem!(S::Basico, "Sombras", shadows),
    cem!(S::Basico, "Brancos", whites),
    cem!(S::Basico, "Pretos", blacks),
    // 🚨 **Até 2026-09-30 a Claridade se chamava "Textura" aqui** (e era uma
    // saturação no shader). Agora são as três do Lightroom, na ordem dele.
    cem!(S::Basico, "Textura", texture),
    lightroom!("Claridade", clarity, |a| a.clarity * 100.0, |a, v| a
        .clarity =
        v / 100.0),
    cem!(S::Basico, "Desembaçar", dehaze),
    // No P&B elas guardam o valor e não agem (o shader as pula), e aparecem
    // apagadas — como no Lightroom.
    lightroom!("Vibração", vibrance, |a| a.vibrance * 100.0, |a, v| a
        .vibrance =
        v / 100.0)
    .com_trilho(Trilho::Saturacao)
    .ativo_quando(colorida),
    lightroom!(
        "Saturação",
        saturation,
        |a| a.saturation * 100.0,
        |a, v| a.saturation = v / 100.0
    )
    .com_trilho(Trilho::Saturacao)
    .ativo_quando(colorida),
    // 🎞️ **O "P&B" do alto do Básico**, e o Perfil "Monocromático": no
    // Lightroom o tratamento em preto e branco é do Básico, e o painel HSL
    // vira a Mistura de preto e branco enquanto ele está ligado. Não vira
    // slider: quem o desenha são o botão e a lista do Perfil.
    interruptor!(S::Tratamento, "P&B", bw_ativo),
    // --------------------------------------------------------- Curva de tons
    cem!(S::CurvaDeTons, "Sombras", tone_curve_shadows),
    cem!(S::CurvaDeTons, "Escuros", tone_curve_darks),
    cem!(S::CurvaDeTons, "Claros", tone_curve_lights),
    cem!(S::CurvaDeTons, "Realces", tone_curve_highlights),
    // As três divisões entre as zonas (25/50/75 no Lightroom) — os pinos da
    // barra embaixo do gráfico, e não sliders.
    cento!(
        S::RegioesDaCurva,
        "Divisão das sombras",
        tone_curve_split_shadows
    ),
    cento!(
        S::RegioesDaCurva,
        "Divisão dos meios-tons",
        tone_curve_split_midtones
    ),
    cento!(
        S::RegioesDaCurva,
        "Divisão dos realces",
        tone_curve_split_highlights
    ),
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
    // 🔑 Os oito dormem sem o `bw_ativo` (o "P&B" do Básico) — um preset de
    // cor que traga `GrayMixer` dentro não dessatura a foto sozinho. Os nomes
    // são os da Mistura de preto e branco do Lightroom em português.
    mistura!("Vermelho", bw_red, 0),
    mistura!("Laranja", bw_orange, 1),
    mistura!("Amarelo", bw_yellow, 2),
    mistura!("Verde", bw_green, 3),
    mistura!("Azul-piscina", bw_aqua, 4),
    mistura!("Azul", bw_blue, 5),
    mistura!("Púrpura", bw_purple, 6),
    mistura!("Magenta", bw_magenta, 7),
    // --------------------------------------------------------------- Detalhe
    // A ordem do Lightroom: Nitidez (quantidade, raio, detalhe, máscara) e
    // Redução de ruído (luminância, detalhe, contraste; cor, detalhe,
    // suavidade).
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
    cento!(S::Detalhe, "Detalhe da nitidez", sharpen_detail),
    cento!(S::Detalhe, "Máscara da nitidez", sharpen_masking),
    cento!(S::Detalhe, "Ruído (luminância)", nr_luminance),
    cento!(S::Detalhe, "Detalhe da luminância", nr_luminance_detail),
    cento!(S::Detalhe, "Contraste da luminância", nr_luminance_contrast),
    cento!(S::Detalhe, "Ruído (cor)", nr_color),
    cento!(S::Detalhe, "Detalhe da cor", nr_color_detail),
    cento!(S::Detalhe, "Suavidade da cor", nr_color_smoothness),
    // ----------------------------------------------------------------- Lente
    cem!(S::Lente, "Distorção", lens_distortion),
    cem!(S::Lente, "Vinheta", lens_vignette_amount),
    cento!(S::Lente, "Meio da vinheta", lens_vignette_midpoint),
    // ------------------------------------------------------------ Calibração
    // 🔑 A versão de processo, no lugar onde o Lightroom a mostra: ligado, a
    // Exposição, o Contraste, Realces, Sombras, Brancos, Pretos e a vinheta usam
    // as curvas medidas no Lightroom (`revelacao_core::lightroom`). Foto antiga
    // abre desligado — e é por aqui que o operador a atualiza.
    interruptor!(S::Calibracao, "Processo do Lightroom", processo),
    cem!(S::Calibracao, "Sombras — matiz", calib_shadow_tint).com_trilho(Trilho::VerdeMagenta),
    matiz!(S::Calibracao, "Vermelho — matiz", calib_red_hue).com_trilho(Trilho::Roda),
    cem!(S::Calibracao, "Vermelho — saturação", calib_red_sat)
        .com_trilho(Trilho::HslSaturacao(0.0)),
    matiz!(S::Calibracao, "Verde — matiz", calib_green_hue).com_trilho(Trilho::Roda),
    cem!(S::Calibracao, "Verde — saturação", calib_green_sat)
        .com_trilho(Trilho::HslSaturacao(120.0)),
    matiz!(S::Calibracao, "Azul — matiz", calib_blue_hue).com_trilho(Trilho::Roda),
    cem!(S::Calibracao, "Azul — saturação", calib_blue_sat).com_trilho(Trilho::HslSaturacao(220.0)),
    // ------------------------------------------------------ Correção de cores
    // As três faixas e o global do Color Grading; a Mesclagem abre em 50.
    // 🔑 Os matizes são os da roda do Lightroom desde 2026-09-30: o mesmo
    // número dá a mesma cor que lá (o trilho mostra qual).
    //
    // 🎡 **Na tela, matiz e saturação são as rodas** (`rodas.rs`), e não estas
    // barras — elas só aparecem na vista de uma roda. Continuam aqui porque a
    // contagem, o ponto âmbar, o duplo clique e a sincronização leem a tabela.
    // Os rótulos são os do Lightroom em português: Realces, Equilíbrio e
    // Mesclagem (dono, 2026-09-30, com o print do painel).
    matiz!(S::Tonalizacao, "Sombras — matiz", split_shadow_hue).com_trilho(Trilho::RodaDoLightroom),
    cento!(S::Tonalizacao, "Sombras — saturação", split_shadow_sat),
    cem!(S::Tonalizacao, "Sombras — luminância", split_shadow_lum).com_trilho(Trilho::Luminancia),
    matiz!(S::Tonalizacao, "Tons médios — matiz", split_midtone_hue)
        .com_trilho(Trilho::RodaDoLightroom),
    cento!(S::Tonalizacao, "Tons médios — saturação", split_midtone_sat),
    cem!(
        S::Tonalizacao,
        "Tons médios — luminância",
        split_midtone_lum
    )
    .com_trilho(Trilho::Luminancia),
    matiz!(S::Tonalizacao, "Realces — matiz", split_highlight_hue)
        .com_trilho(Trilho::RodaDoLightroom),
    cento!(S::Tonalizacao, "Realces — saturação", split_highlight_sat),
    cem!(S::Tonalizacao, "Realces — luminância", split_highlight_lum)
        .com_trilho(Trilho::Luminancia),
    matiz!(S::Tonalizacao, "Global — matiz", split_global_hue).com_trilho(Trilho::RodaDoLightroom),
    cento!(S::Tonalizacao, "Global — saturação", split_global_sat),
    cem!(S::Tonalizacao, "Global — luminância", split_global_lum).com_trilho(Trilho::Luminancia),
    cento!(S::Tonalizacao, "Mesclagem", split_blending),
    cem!(S::Tonalizacao, "Equilíbrio", split_balance),
    // --------------------------------------------------------------- Efeitos
    // 🎞️ Os dois grupos do Lightroom, com os nomes dele: o título do grupo
    // diz de quem é a Intensidade.
    escolha!(S::Vinheta, "Estilo", pcv_style, ESTILOS_DA_VINHETA),
    cem!(S::Vinheta, "Intensidade", pcv_amount),
    cento!(S::Vinheta, "Ponto médio", pcv_midpoint).ativo_quando(vinheta_ligada),
    cem!(S::Vinheta, "Arredondamento", pcv_roundness).ativo_quando(vinheta_ligada),
    cento!(S::Vinheta, "Difusão", pcv_feather).ativo_quando(vinheta_ligada),
    cento!(S::Vinheta, "Realces", pcv_highlights).ativo_quando(realces_da_vinheta),
    // 🎞️ A vinheta do darktable — o `gui_init` de `src/iop/vignette.c`: os
    // mesmos controles, na mesma ordem, com as faixas, as casas e os nomes
    // do darktable em português. "Ligar" é a chave do título do grupo.
    interruptor!(S::VinhetaDarktable, "Ligar", darktable_vignette_ativo),
    def!(
        S::VinhetaDarktable,
        "Início do decaimento",
        darktable_vignette_scale,
        0.0,
        200.0,
        2,
        false
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktable,
        "Raio do decaimento",
        darktable_vignette_falloff_scale,
        0.0,
        200.0,
        2,
        false
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktable,
        "Brilho",
        darktable_vignette_brightness,
        -1.0,
        1.0,
        3,
        true
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktable,
        "Saturação",
        darktable_vignette_saturation,
        -1.0,
        1.0,
        3,
        true
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktableForma,
        "Centro horizontal",
        darktable_vignette_center_x,
        -1.0,
        1.0,
        3,
        true
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktableForma,
        "Centro vertical",
        darktable_vignette_center_y,
        -1.0,
        1.0,
        3,
        true
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktableForma,
        "Forma",
        darktable_vignette_shape,
        0.0,
        5.0,
        2,
        false
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    interruptor!(
        S::VinhetaDarktableForma,
        "Proporção automática",
        darktable_vignette_autoratio
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    def!(
        S::VinhetaDarktableForma,
        "Largura/altura",
        darktable_vignette_whratio,
        0.0,
        2.0,
        3,
        false
    )
    .ativo_quando(proporcao_manual),
    escolha!(
        S::VinhetaDarktableForma,
        "Matização",
        darktable_vignette_dithering,
        MATIZACAO_DO_DARKTABLE
    )
    .ativo_quando(vinheta_do_darktable_ligada),
    cento!(S::Grao, "Intensidade", grain_amount),
    cento!(S::Grao, "Tamanho", grain_size).ativo_quando(grao_ligado),
    cento!(S::Grao, "Aspereza", grain_roughness).ativo_quando(grao_ligado),
];

/// Quantos campos do motor estão fora do neutro — **todos**, e não só os que
/// têm slider. É a conta do cabeçalho do site
/// (`NOMES_DOS_AJUSTES.filter(n => ajustes[n] !== PADRAO[n])`).
pub fn quantos_fora_do_neutro(ajustes: &Ajustes) -> usize {
    // A versão de processo não é ajuste (ver `Ajustes::sem_efeito`): a foto
    // nova nasce no processo 1 e não pode abrir dizendo "1 ajuste".
    let ajustes = Ajustes {
        processo: 0.0,
        ..*ajustes
    };
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

/// O que o ponto de um conjunto de controles diz — o `marcaDosCampos` do site.
///
/// 🚨 **O ponto âmbar comparava com o neutro** (dono, 2026-09-27): a foto que
/// chega revelada do servidor, aberta sem mexer, acendia âmbar em toda seção do
/// estilo, com o "Salvar na galeria e sair" apagado ao lado — as duas
/// indicações se desmentiam. Âmbar é "mudou e não salvou", a pergunta do botão;
/// o ajuste que já estava salvo leva um ponto cinza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marca {
    /// Algum controle mudou desde a revelação com que a foto abriu.
    NaoSalvo,
    /// Nada mudou, e a revelação salva tem ajuste fora do neutro.
    Ajustado,
}

fn marca_dos<'a>(
    defs: impl Iterator<Item = &'a Definicao> + Clone,
    ajustes: &Ajustes,
    salvo: &Ajustes,
) -> Option<Marca> {
    if defs.clone().any(|d| (d.ler)(ajustes) != (d.ler)(salvo)) {
        Some(Marca::NaoSalvo)
    } else if defs.into_iter().any(|d| d.alterado(ajustes)) {
        Some(Marca::Ajustado)
    } else {
        None
    }
}

/// O ponto de uma família de controles (uma aba do HSL, por exemplo).
pub fn marca_da_secao(ajustes: &Ajustes, salvo: &Ajustes, secao: Secao) -> Option<Marca> {
    marca_dos(
        CONTROLES.iter().filter(|d| d.secao == secao),
        ajustes,
        salvo,
    )
}

/// O ponto do cabeçalho de um painel.
pub fn marca_do_painel(ajustes: &Ajustes, salvo: &Ajustes, painel: Painel) -> Option<Marca> {
    marca_dos(
        CONTROLES
            .iter()
            .filter(|d| painel.secoes().contains(&d.secao)),
        ajustes,
        salvo,
    )
}

/// Em que campo do [`Ajustes`] este controle lê — pelo que ele devolve, e não
/// por um nome escrito à mão.
#[cfg(test)]
pub(crate) fn campo_do_controle(def: &Definicao) -> &'static str {
    // 🔑 **Pelo campo que muda a leitura**, e não pelo número lido: os do
    // Básico mostram a escala do Lightroom (contraste +7 é o campo 1,07), e
    // o número que eles devolvem não é o do campo.
    let neutro = Ajustes::default().como_vetor();
    let base = (def.ler)(&Ajustes::default());
    let lidos: Vec<usize> = (0..Ajustes::NOMES.len())
        .filter(|&i| {
            let mut vetor = neutro.to_vec();
            vetor[i] += 0.5;
            let ajustes = Ajustes::de_vetor(&vetor).expect("o vetor tem o tamanho de `NOMES`");
            (def.ler)(&ajustes) != base
        })
        .collect();
    assert!(
        lidos.len() == 1,
        "`{}` não lê campo nenhum do `Ajustes` (ou lê {})",
        def.rotulo,
        lidos.len()
    );
    Ajustes::NOMES[lidos[0]]
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
        // O Estilo da vinheta pós-corte também é discreto, mas é escolha de
        // três, com nome em cada posição — não é interruptor.
        let interruptores: Vec<_> = CONTROLES
            .iter()
            .filter(|d| d.discreto && d.opcoes.is_none())
            .collect();
        assert_eq!(
            interruptores.len(),
            4,
            "bw_ativo, o processo do Lightroom e as duas chaves da vinheta do darktable \
             (os 7 do estágio darktable saíram em 2/out/2026)"
        );
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
        // 🎞️ O contraste na escala do Lightroom: inteiro, com sinal.
        let contraste = &CONTROLES[1];
        assert_eq!(contraste.formatar(7.0), "+7");
        assert_eq!(contraste.formatar(-12.0), "-12");
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
                    // Os do Básico convertem para a escala do motor e
                    // voltam: o f32 erra na sétima casa.
                    assert!(
                        ((def.ler)(&ajustes) - marca).abs() < 1e-3,
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
        assert_eq!(Painel::ADOBE_RGB.to_vec(), Painel::TODOS.to_vec());
    }

    /// 🚨 **O cabeçalho conta todos os campos**, e o neutro não conta nada — nem
    /// a identidade da curva.
    #[test]
    fn o_cabecalho_conta_todos_os_campos() {
        assert_eq!(quantos_fora_do_neutro(&Ajustes::default()), 0);
        let ajustes = Ajustes {
            exposure: 1.0,
            pcv_amount: -20.0,
            curva_b3: 10.0,
            ..Ajustes::default()
        };
        assert_eq!(quantos_fora_do_neutro(&ajustes), 3);
        assert!(painel_alterado(&ajustes, Painel::CurvaDeTons));
        assert!(!painel_alterado(&ajustes, Painel::Hsl));
    }
}

/// 🎨 O trilho de cada controle — o desenho do Lightroom.
#[cfg(test)]
mod trilho_dos_controles {
    use super::*;

    fn def(secao: Secao, rotulo: &str) -> &'static Definicao {
        CONTROLES
            .iter()
            .find(|d| d.secao == secao && d.rotulo == rotulo)
            .unwrap_or_else(|| panic!("`{rotulo}` sumiu da tabela"))
    }

    #[test]
    fn o_equilibrio_de_branco_tem_as_cores_do_lightroom() {
        assert_eq!(
            def(Secao::Basico, "Temperatura").trilho,
            Trilho::Temperatura
        );
        assert_eq!(def(Secao::Basico, "Colorir").trilho, Trilho::VerdeMagenta);
        assert_eq!(def(Secao::Basico, "Vibração").trilho, Trilho::Saturacao);
        assert_eq!(def(Secao::Basico, "Saturação").trilho, Trilho::Saturacao);
    }

    /// Cada uma das 24 barras do HSL está na cor da própria faixa, e a família
    /// (Cor, Luminância, Matiz) decide o desenho.
    #[test]
    fn cada_faixa_do_hsl_tem_a_propria_cor() {
        for (secao, familia) in [
            (Secao::HslCor, Trilho::HslSaturacao as fn(f32) -> Trilho),
            (Secao::HslLuminancia, Trilho::HslLuminancia),
            (Secao::HslMatiz, Trilho::HslMatiz),
        ] {
            let da_secao: Vec<_> = CONTROLES.iter().filter(|d| d.secao == secao).collect();
            assert_eq!(da_secao.len(), 8);
            for (d, matiz) in da_secao.iter().zip(MATIZ_DA_FAIXA) {
                assert_eq!(d.trilho, familia(matiz), "{:?} `{}`", secao, d.rotulo);
            }
        }
    }

    /// Todo controle que **escolhe** um matiz (0–360°) mostra a roda inteira —
    /// a do Lightroom na Tonalização, que é a que o motor usa ali.
    #[test]
    fn quem_escolhe_matiz_mostra_a_roda() {
        for d in CONTROLES
            .iter()
            .filter(|d| d.minimo == 0.0 && d.maximo == 360.0)
        {
            let esperado = if d.secao == Secao::Tonalizacao {
                Trilho::RodaDoLightroom
            } else {
                Trilho::Roda
            };
            assert_eq!(d.trilho, esperado, "`{}`", d.rotulo);
        }
    }

    /// O Estilo da vinheta mostra o nome, e não o número.
    #[test]
    fn o_estilo_da_vinheta_mostra_o_nome() {
        let estilo = CONTROLES
            .iter()
            .find(|d| d.secao == Secao::Vinheta && d.rotulo == "Estilo")
            .expect("o controle existe");
        assert_eq!(estilo.passo(), 1.0);
        assert_eq!(
            (estilo.minimo, estilo.maximo, estilo.neutro()),
            (0.0, 2.0, 0.0)
        );
        assert_eq!(estilo.formatar(0.0), "Prioridade de realces");
        assert_eq!(estilo.formatar(2.0), "Sobreposição de tinta");
    }

    /// O resto é liso: a cor só entra onde ela diz algo.
    #[test]
    fn os_de_tom_sao_lisos() {
        for rotulo in [
            "Exposição",
            "Contraste",
            "Realces",
            "Sombras",
            "Brancos",
            "Pretos",
        ] {
            assert_eq!(
                def(Secao::Basico, rotulo).trilho,
                Trilho::Liso,
                "`{rotulo}`"
            );
        }
        assert!(CONTROLES
            .iter()
            .filter(|d| d.discreto)
            .all(|d| d.trilho == Trilho::Liso));
    }
}

/// 🎞️ O painel Efeitos do Lightroom: dois grupos, e o que não age fica apagado.
#[cfg(test)]
mod efeitos_do_lightroom {
    use super::*;

    fn rotulos(secao: Secao) -> Vec<&'static str> {
        CONTROLES
            .iter()
            .filter(|d| d.secao == secao)
            .map(|d| d.rotulo)
            .collect()
    }

    fn controle(secao: Secao, rotulo: &str) -> &'static Definicao {
        CONTROLES
            .iter()
            .find(|d| d.secao == secao && d.rotulo == rotulo)
            .unwrap_or_else(|| panic!("`{rotulo}` não existe"))
    }

    #[test]
    fn a_vinheta_e_o_granulado_na_ordem_do_lightroom() {
        assert_eq!(
            Painel::Efeitos.secoes(),
            &[
                Secao::Vinheta,
                Secao::VinhetaDarktable,
                Secao::VinhetaDarktableForma,
                Secao::Grao
            ]
        );
        // O `gui_init` do `vignette.c`, na ordem dele.
        assert_eq!(
            rotulos(Secao::VinhetaDarktable),
            [
                "Ligar",
                "Início do decaimento",
                "Raio do decaimento",
                "Brilho",
                "Saturação"
            ]
        );
        assert_eq!(
            rotulos(Secao::VinhetaDarktableForma),
            [
                "Centro horizontal",
                "Centro vertical",
                "Forma",
                "Proporção automática",
                "Largura/altura",
                "Matização"
            ]
        );
        assert_eq!(
            rotulos(Secao::Vinheta),
            [
                "Estilo",
                "Intensidade",
                "Ponto médio",
                "Arredondamento",
                "Difusão",
                "Realces"
            ]
        );
        assert_eq!(rotulos(Secao::Grao), ["Intensidade", "Tamanho", "Aspereza"]);
        let estilo = controle(Secao::Vinheta, "Estilo");
        assert_eq!(estilo.opcoes.map(<[_]>::len), Some(3));
    }

    /// O que acende com a intensidade da vinheta em 0, −37 (escurecendo) e +20
    /// (clareando), e no estilo Sobreposição de tinta.
    #[test]
    fn o_que_nao_age_fica_apagado() {
        let acesos = |a: &Ajustes, secao: Secao| -> Vec<&'static str> {
            CONTROLES
                .iter()
                .filter(|d| d.secao == secao && d.age(a))
                .map(|d| d.rotulo)
                .collect()
        };

        let neutro = Ajustes::default();
        assert_eq!(acesos(&neutro, Secao::Vinheta), ["Estilo", "Intensidade"]);
        assert_eq!(acesos(&neutro, Secao::VinhetaDarktable), ["Ligar"]);
        assert!(acesos(&neutro, Secao::VinhetaDarktableForma).is_empty());
        let darktable = Ajustes {
            darktable_vignette_ativo: 1.0,
            ..neutro
        };
        assert_eq!(acesos(&darktable, Secao::VinhetaDarktable).len(), 5);
        assert_eq!(acesos(&darktable, Secao::VinhetaDarktableForma).len(), 6);
        let automatica = Ajustes {
            darktable_vignette_autoratio: 1.0,
            ..darktable
        };
        assert!(!acesos(&automatica, Secao::VinhetaDarktableForma).contains(&"Largura/altura"));
        assert_eq!(acesos(&neutro, Secao::Grao), ["Intensidade"]);

        let escura = Ajustes {
            pcv_amount: -37.0,
            grain_amount: 25.0,
            ..neutro
        };
        assert_eq!(acesos(&escura, Secao::Vinheta).len(), 6);
        assert_eq!(acesos(&escura, Secao::Grao).len(), 3);

        let clara = Ajustes {
            pcv_amount: 20.0,
            ..neutro
        };
        assert!(!acesos(&clara, Secao::Vinheta).contains(&"Realces"));

        let tinta = Ajustes {
            pcv_style: 2.0,
            ..escura
        };
        assert!(!acesos(&tinta, Secao::Vinheta).contains(&"Realces"));
        assert!(acesos(&tinta, Secao::Vinheta).contains(&"Difusão"));
    }
}

/// 🎞️ O Básico e a Mistura de preto e branco do Lightroom em português.
#[cfg(test)]
mod basico_do_lightroom {
    use super::*;

    fn controle(secao: Secao, rotulo: &str) -> &'static Definicao {
        CONTROLES
            .iter()
            .find(|d| d.secao == secao && d.rotulo == rotulo)
            .unwrap_or_else(|| panic!("`{rotulo}` não existe"))
    }

    #[test]
    fn os_nomes_sao_os_do_lightroom() {
        let basico: Vec<_> = CONTROLES
            .iter()
            .filter(|d| d.secao == Secao::Basico)
            .map(|d| d.rotulo)
            .collect();
        for rotulo in [
            "Temperatura",
            "Colorir",
            "Exposição",
            "Contraste",
            "Realces",
            "Sombras",
            "Brancos",
            "Pretos",
            "Textura",
            "Claridade",
            "Desembaçar",
            "Vibração",
            "Saturação",
        ] {
            assert!(basico.contains(&rotulo), "o Básico não tem `{rotulo}`");
        }
        assert_eq!(
            controle(Secao::Tratamento, "P&B").secao.painel(),
            Painel::Basico
        );
        let mistura: Vec<_> = CONTROLES
            .iter()
            .filter(|d| d.secao == Secao::PretoEBranco)
            .map(|d| d.rotulo)
            .collect();
        assert_eq!(
            mistura,
            [
                "Vermelho",
                "Laranja",
                "Amarelo",
                "Verde",
                "Azul-piscina",
                "Azul",
                "Púrpura",
                "Magenta"
            ]
        );
    }

    /// Os números do print do dono: Temperatura +8, Colorir −23, Contraste
    /// +7, Vibração −14, Saturação +1 — na escala do Lightroom, sobre os
    /// campos do motor.
    #[test]
    fn os_numeros_sao_os_do_lightroom() {
        let mut a = Ajustes::default();
        for (rotulo, valor) in [
            ("Temperatura", 8.0),
            ("Colorir", -23.0),
            ("Contraste", 7.0),
            ("Claridade", 25.0),
            ("Vibração", -14.0),
            ("Saturação", 1.0),
        ] {
            let d = controle(Secao::Basico, rotulo);
            (d.aplicar)(&mut a, valor);
            assert!(((d.ler)(&a) - valor).abs() < 1e-3, "`{rotulo}`");
            assert_eq!(
                d.formatar((d.ler)(&a)),
                format!("{valor:+}").replace("+-", "-")
            );
        }
        assert!((a.temperature - 0.8).abs() < 1e-6);
        assert!((a.tint + 2.3).abs() < 1e-6);
        assert!((a.contrast - 1.07).abs() < 1e-6);
        assert!((a.clarity - 0.25).abs() < 1e-6);
        assert!((a.vibrance + 0.14).abs() < 1e-6);
        assert!((a.saturation - 0.01).abs() < 1e-6);
    }

    /// No P&B a Vibração e a Saturação ficam apagadas (o motor as pula).
    #[test]
    fn no_pb_vibracao_e_saturacao_se_apagam() {
        let mut a = Ajustes::default();
        assert!(controle(Secao::Basico, "Vibração").age(&a));
        assert!(controle(Secao::Basico, "Saturação").age(&a));
        a.bw_ativo = 1.0;
        assert!(!controle(Secao::Basico, "Vibração").age(&a));
        assert!(!controle(Secao::Basico, "Saturação").age(&a));
        assert!(controle(Secao::Basico, "Exposição").age(&a));
    }
}
