//! As rodas da Correção de cores — o painel "Color Grading" do Lightroom.
//!
//! 🔑 **Conta, e não tela.** Tudo o que a roda decide — onde o puck fica para
//! um matiz e uma saturação, que matiz e saturação um ponto da roda quer dizer,
//! o que Shift e Cmd fazem com o arrasto, quais campos cada faixa move — mora
//! aqui, testável sem janela. A coluna da direita (`tela/painel.rs`) só desenha
//! e encaminha o ponteiro.
//!
//! A disposição é a do Lightroom (dono, 2026-09-30, com o print do painel):
//! matiz 0° à direita, crescendo no sentido anti-horário, e a saturação é a
//! distância ao centro. A cor de cada ângulo é a da roda do Lightroom
//! ([`cor_da_roda_do_lightroom`]), a mesma que o motor aplica — o puck aponta
//! para a cor que a foto vai ganhar.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::RenderImage;
use revelacao_core::ajustes::cor_da_roda_do_lightroom;

use super::processador::Ajustes;

/// Uma das quatro faixas da Correção de cores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Faixa {
    Sombras,
    TonsMedios,
    Realces,
    Global,
}

impl Faixa {
    pub const TODAS: [Faixa; 4] = [
        Faixa::Sombras,
        Faixa::TonsMedios,
        Faixa::Realces,
        Faixa::Global,
    ];

    pub fn rotulo(&self) -> &'static str {
        match self {
            Faixa::Sombras => "Sombras",
            Faixa::TonsMedios => "Tons médios",
            Faixa::Realces => "Realces",
            Faixa::Global => "Global",
        }
    }

    /// O pedaço do `debug_selector` — sem acento, para o roteiro.
    pub fn chave(&self) -> &'static str {
        match self {
            Faixa::Sombras => "sombras",
            Faixa::TonsMedios => "medios",
            Faixa::Realces => "realces",
            Faixa::Global => "global",
        }
    }

    /// Os rótulos dos três controles da faixa na tabela (`controles.rs`),
    /// por onde a tela acha o slider de cada um.
    ///
    /// ⚠️ **Sempre junto com `Secao::Tonalizacao`**: "Sombras — matiz" também
    /// é a tinta das sombras da Calibração, e "Realces — matiz" é do Color
    /// balance da aba RGB.
    pub fn rotulo_do_matiz(&self) -> &'static str {
        match self {
            Faixa::Sombras => "Sombras — matiz",
            Faixa::TonsMedios => "Tons médios — matiz",
            Faixa::Realces => "Realces — matiz",
            Faixa::Global => "Global — matiz",
        }
    }

    pub fn rotulo_da_saturacao(&self) -> &'static str {
        match self {
            Faixa::Sombras => "Sombras — saturação",
            Faixa::TonsMedios => "Tons médios — saturação",
            Faixa::Realces => "Realces — saturação",
            Faixa::Global => "Global — saturação",
        }
    }

    pub fn rotulo_da_luminancia(&self) -> &'static str {
        match self {
            Faixa::Sombras => "Sombras — luminância",
            Faixa::TonsMedios => "Tons médios — luminância",
            Faixa::Realces => "Realces — luminância",
            Faixa::Global => "Global — luminância",
        }
    }

    /// `(matiz, saturação, luminância)`.
    pub fn ler(&self, a: &Ajustes) -> (f32, f32, f32) {
        match self {
            Faixa::Sombras => (a.split_shadow_hue, a.split_shadow_sat, a.split_shadow_lum),
            Faixa::TonsMedios => (
                a.split_midtone_hue,
                a.split_midtone_sat,
                a.split_midtone_lum,
            ),
            Faixa::Realces => (
                a.split_highlight_hue,
                a.split_highlight_sat,
                a.split_highlight_lum,
            ),
            Faixa::Global => (a.split_global_hue, a.split_global_sat, a.split_global_lum),
        }
    }

    /// Escreve matiz e saturação — o que a roda move. A luminância é do slider.
    pub fn definir_cor(&self, a: &mut Ajustes, matiz: f32, saturacao: f32) {
        let (h, s) = match self {
            Faixa::Sombras => (&mut a.split_shadow_hue, &mut a.split_shadow_sat),
            Faixa::TonsMedios => (&mut a.split_midtone_hue, &mut a.split_midtone_sat),
            Faixa::Realces => (&mut a.split_highlight_hue, &mut a.split_highlight_sat),
            Faixa::Global => (&mut a.split_global_hue, &mut a.split_global_sat),
        };
        *h = matiz;
        *s = saturacao;
    }

    /// A faixa sem efeito: matiz, saturação e luminância no neutro. É o que o
    /// olho mostra enquanto está apertado.
    pub fn neutralizar(&self, a: &mut Ajustes) {
        let neutro = Ajustes::default();
        let (h, s, l) = self.ler(&neutro);
        self.definir_cor(a, h, s);
        match self {
            Faixa::Sombras => a.split_shadow_lum = l,
            Faixa::TonsMedios => a.split_midtone_lum = l,
            Faixa::Realces => a.split_highlight_lum = l,
            Faixa::Global => a.split_global_lum = l,
        }
    }

    /// Se a faixa saiu do neutro.
    pub fn alterada(&self, a: &Ajustes) -> bool {
        self.ler(a) != self.ler(&Ajustes::default())
    }
}

/// O painel inteiro sem efeito: as quatro faixas, o Equilíbrio e a Mesclagem.
/// É o olho do cabeçalho.
pub fn sem_correcao_de_cores(a: &Ajustes) -> Ajustes {
    let mut sem = *a;
    for faixa in Faixa::TODAS {
        faixa.neutralizar(&mut sem);
    }
    let neutro = Ajustes::default();
    sem.split_balance = neutro.split_balance;
    sem.split_blending = neutro.split_blending;
    sem
}

/// O que o seletor "Ajustar" mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vista {
    /// Tons médios em cima, Sombras e Realces embaixo — a abertura do
    /// Lightroom. O Global só aparece na vista dele.
    #[default]
    TresRodas,
    Uma(Faixa),
}

impl Vista {
    /// Os cinco botões do "Ajustar", na ordem do Lightroom.
    pub const TODAS: [Vista; 5] = [
        Vista::TresRodas,
        Vista::Uma(Faixa::Sombras),
        Vista::Uma(Faixa::TonsMedios),
        Vista::Uma(Faixa::Realces),
        Vista::Uma(Faixa::Global),
    ];

    pub fn dica(&self) -> &'static str {
        match self {
            Vista::TresRodas => "3 rodas",
            Vista::Uma(f) => f.rotulo(),
        }
    }

    pub fn chave(&self) -> &'static str {
        match self {
            Vista::TresRodas => "tres",
            Vista::Uma(f) => f.chave(),
        }
    }

    /// As faixas que esta vista cobre — o ponto embaixo do botão.
    pub fn faixas(&self) -> &'static [Faixa] {
        match self {
            Vista::TresRodas => &[Faixa::Sombras, Faixa::TonsMedios, Faixa::Realces],
            Vista::Uma(Faixa::Sombras) => &[Faixa::Sombras],
            Vista::Uma(Faixa::TonsMedios) => &[Faixa::TonsMedios],
            Vista::Uma(Faixa::Realces) => &[Faixa::Realces],
            Vista::Uma(Faixa::Global) => &[Faixa::Global],
        }
    }
}

// ------------------------------------------------------------------ geometria

/// O ponto da roda (raio 1, y para baixo, como na tela) de um matiz e uma
/// saturação.
pub fn ponto_da_roda(matiz: f32, saturacao: f32) -> (f32, f32) {
    let raio = (saturacao / 100.0).clamp(0.0, 1.0);
    let angulo = matiz.to_radians();
    (angulo.cos() * raio, -angulo.sin() * raio)
}

/// O matiz (0–359) e a saturação (0–100) de um ponto da roda, recolhidos e
/// arredondados como os sliders. Fora da roda vale a borda.
pub fn cor_do_ponto(dx: f32, dy: f32) -> (f32, f32) {
    let raio = dx.hypot(dy);
    let saturacao = (raio.min(1.0) * 100.0).round();
    (matiz_do_ponto(dx, dy), saturacao)
}

/// O ângulo de um ponto, 0–359, inteiro.
pub fn matiz_do_ponto(dx: f32, dy: f32) -> f32 {
    let graus = (-dy).atan2(dx).to_degrees().rem_euclid(360.0).round();
    if graus >= 360.0 {
        0.0
    } else {
        graus
    }
}

/// O raio da roda dentro de um quadro de `lado`: sobra em volta para a alça,
/// que fica do lado de fora.
pub fn raio_no_quadro(lado: f32) -> f32 {
    (lado / 2.0 - FOLGA_DA_ALCA).max(1.0)
}

/// O espaço entre a borda da roda e a do quadro.
pub const FOLGA_DA_ALCA: f32 = 9.0;
/// Onde a alça fica: um pouco além da borda.
pub const RAIO_DA_ALCA: f32 = 1.0 + 5.0 / 60.0;
/// Até onde o clique ainda pega a alça, em pixels de tela.
const ALCANCE_DA_ALCA: f32 = 8.0;

/// O que o ponteiro pegou ao apertar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pegada {
    /// O disco: move matiz e saturação.
    Puck,
    /// A bolinha da borda: só o matiz.
    Alca,
}

/// Onde o ponteiro caiu, em pixels relativos ao centro da roda.
pub fn pegar(dx: f32, dy: f32, raio: f32, matiz: f32) -> Option<Pegada> {
    let (ax, ay) = ponto_da_roda(matiz, 100.0);
    let (ax, ay) = (ax * raio * RAIO_DA_ALCA, ay * raio * RAIO_DA_ALCA);
    if (dx - ax).hypot(dy - ay) <= ALCANCE_DA_ALCA {
        return Some(Pegada::Alca);
    }
    (dx.hypot(dy) <= raio + 3.0).then_some(Pegada::Puck)
}

/// Um arrasto em curso numa roda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrasto {
    pub faixa: Faixa,
    pub pegada: Pegada,
    /// Onde o ponteiro estava ao apertar (relativo ao centro, em pixels).
    pub ponteiro_inicial: (f32, f32),
    /// Matiz e saturação ao apertar.
    pub cor_inicial: (f32, f32),
}

/// Quanto o modo fino (Cmd/Ctrl) reduz o movimento — o do Lightroom.
pub const FATOR_FINO: f32 = 0.25;

impl Arrasto {
    /// Matiz e saturação para o ponteiro em `(dx, dy)` (pixels, relativo ao
    /// centro), numa roda de `raio`.
    ///
    /// - **Puck**: o ponto vai para debaixo do ponteiro. No modo fino ele anda
    ///   um quarto do que o ponteiro andou, a partir de onde estava.
    /// - **Shift** trava o matiz: só a saturação muda, pela projeção do
    ///   ponteiro na direção do matiz de partida.
    /// - **Alça**: só o matiz, pelo ângulo do ponteiro; a saturação fica.
    pub fn cor(&self, dx: f32, dy: f32, raio: f32, fino: bool, travar_matiz: bool) -> (f32, f32) {
        let (matiz0, sat0) = self.cor_inicial;
        let raio = raio.max(1.0);
        let alvo = if fino {
            let (px0, py0) = ponto_da_roda(matiz0, sat0);
            (
                px0 + (dx - self.ponteiro_inicial.0) / raio * FATOR_FINO,
                py0 + (dy - self.ponteiro_inicial.1) / raio * FATOR_FINO,
            )
        } else {
            (dx / raio, dy / raio)
        };
        match self.pegada {
            Pegada::Alca => (matiz_do_ponto(alvo.0, alvo.1), sat0),
            Pegada::Puck if travar_matiz => {
                let (ux, uy) = ponto_da_roda(matiz0, 100.0);
                let projecao = (alvo.0 * ux + alvo.1 * uy).clamp(0.0, 1.0);
                (matiz0, (projecao * 100.0).round())
            }
            Pegada::Puck => {
                let (matiz, sat) = cor_do_ponto(alvo.0, alvo.1);
                // No centro exato o ângulo não diz nada: guardar o matiz de
                // antes evita a roda "esquecer" a cor ao passar pelo meio.
                if sat == 0.0 {
                    (matiz0, 0.0)
                } else {
                    (matiz, sat)
                }
            }
        }
    }
}

// --------------------------------------------------------------------- imagem

/// O cinza do centro: o L* 60 da roda do Lightroom, sem croma.
const CINZA_DO_CENTRO: f32 = 0.57;
/// A borda um pouco mais viva que o meio-tom do motor, como a roda do
/// Lightroom aparece — sem isso o disco fica lavado no tema escuro.
const VIVACIDADE_DA_BORDA: f32 = 1.35;

/// A cor de um ponto do disco, sRGB 0–1.
pub fn cor_do_disco(matiz: f32, raio: f32) -> [f32; 3] {
    let cor = cor_da_roda_do_lightroom(matiz);
    let t = raio.clamp(0.0, 1.0) * VIVACIDADE_DA_BORDA;
    cor.map(|c| (CINZA_DO_CENTRO + (c - CINZA_DO_CENTRO) * t).clamp(0.0, 1.0))
}

/// Os bytes BGRA do disco num quadrado de `lado` pixels, com a borda suave e
/// transparente fora.
pub fn bgra_do_disco(lado: u32) -> Vec<u8> {
    let lado_f = lado as f32;
    let centro = lado_f / 2.0;
    let raio = centro;
    let mut bytes = vec![0u8; (lado * lado * 4) as usize];
    for y in 0..lado {
        for x in 0..lado {
            let dx = x as f32 + 0.5 - centro;
            let dy = y as f32 + 0.5 - centro;
            let d = dx.hypot(dy);
            let alfa = (raio - d + 0.5).clamp(0.0, 1.0);
            if alfa <= 0.0 {
                continue;
            }
            let [r, g, b] = cor_do_disco(matiz_do_ponto(dx, dy), d / raio);
            let i = ((y * lado + x) * 4) as usize;
            // Alfa pré-multiplicado não: o GPUI mistura com alfa direto.
            bytes[i] = (b * 255.0).round() as u8;
            bytes[i + 1] = (g * 255.0).round() as u8;
            bytes[i + 2] = (r * 255.0).round() as u8;
            bytes[i + 3] = (alfa * 255.0).round() as u8;
        }
    }
    bytes
}

thread_local! {
    /// Um disco por tamanho. São três tamanhos na vida do app (as duas
    /// pequenas, a do meio e a grande), e refazer a cada quadro seria varrer
    /// ~150 mil pixels por roda enquanto o puck anda.
    static DISCOS: RefCell<HashMap<u32, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

/// O disco de `lado` pixels **físicos**, feito uma vez.
pub fn imagem_do_disco(lado: u32) -> Option<Arc<RenderImage>> {
    let lado = lado.clamp(8, 1024);
    DISCOS.with(|discos| {
        if let Some(pronta) = discos.borrow().get(&lado) {
            return Some(pronta.clone());
        }
        let imagem = crate::imagem::de_bgra(lado, lado, bgra_do_disco(lado))?;
        discos.borrow_mut().insert(lado, imagem.clone());
        Some(imagem)
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::revelacao::controles::{campo_do_controle, Secao, CONTROLES};

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// 🔑 0° à direita, 90° em cima — o anti-horário do Lightroom, com y da
    /// tela para baixo.
    #[test]
    fn o_vermelho_fica_a_direita_e_o_matiz_cresce_para_cima() {
        let (x, y) = ponto_da_roda(0.0, 100.0);
        assert!(perto(x, 1.0) && perto(y, 0.0));
        let (x, y) = ponto_da_roda(90.0, 100.0);
        assert!(perto(x, 0.0) && perto(y, -1.0), "90° em cima: ({x}, {y})");
        let (x, y) = ponto_da_roda(180.0, 50.0);
        assert!(perto(x, -0.5) && perto(y, 0.0));
    }

    #[test]
    fn ponto_e_cor_vao_e_voltam() {
        for matiz in [0.0, 1.0, 45.0, 120.0, 210.0, 359.0] {
            for sat in [1.0, 30.0, 100.0] {
                let (x, y) = ponto_da_roda(matiz, sat);
                assert_eq!(cor_do_ponto(x, y), (matiz, sat), "{matiz}° {sat}");
            }
        }
    }

    #[test]
    fn fora_da_roda_vale_a_borda() {
        assert_eq!(cor_do_ponto(3.0, 0.0), (0.0, 100.0));
        assert_eq!(
            matiz_do_ponto(1.0, -0.00001),
            0.0,
            "359,99° arredonda para 0, e não 360"
        );
    }

    fn arrasto(pegada: Pegada, cor: (f32, f32), ponteiro: (f32, f32)) -> Arrasto {
        Arrasto {
            faixa: Faixa::Sombras,
            pegada,
            ponteiro_inicial: ponteiro,
            cor_inicial: cor,
        }
    }

    #[test]
    fn o_puck_vai_para_debaixo_do_ponteiro() {
        let a = arrasto(Pegada::Puck, (0.0, 0.0), (0.0, 0.0));
        // 50 px para cima numa roda de 100: 90°, saturação 50.
        assert_eq!(a.cor(0.0, -50.0, 100.0, false, false), (90.0, 50.0));
    }

    #[test]
    fn a_alca_so_gira_o_matiz() {
        let a = arrasto(Pegada::Alca, (40.0, 25.0), (0.0, 0.0));
        assert_eq!(a.cor(-120.0, 0.0, 100.0, false, false), (180.0, 25.0));
    }

    #[test]
    fn shift_trava_o_matiz() {
        let a = arrasto(Pegada::Puck, (0.0, 30.0), (30.0, 0.0));
        // O ponteiro sobe e anda para a direita: só a projeção no 0° conta.
        assert_eq!(a.cor(70.0, -40.0, 100.0, false, true), (0.0, 70.0));
        // Para o lado oposto, a saturação para no zero, e não vira o matiz.
        assert_eq!(a.cor(-70.0, 0.0, 100.0, false, true), (0.0, 0.0));
    }

    #[test]
    fn cmd_anda_um_quarto() {
        let a = arrasto(Pegada::Puck, (0.0, 20.0), (20.0, 0.0));
        // 40 px para a direita numa roda de 100 = 0,4; um quarto = 0,1.
        assert_eq!(a.cor(60.0, 0.0, 100.0, true, false), (0.0, 30.0));
    }

    #[test]
    fn passar_pelo_centro_nao_esquece_o_matiz() {
        let a = arrasto(Pegada::Puck, (210.0, 40.0), (0.0, 0.0));
        assert_eq!(a.cor(0.0, 0.0, 100.0, false, false), (210.0, 0.0));
    }

    #[test]
    fn o_clique_na_alca_pega_a_alca_e_no_disco_pega_o_puck() {
        let raio = 60.0;
        let (ax, ay) = ponto_da_roda(45.0, 100.0);
        let (ax, ay) = (ax * raio * RAIO_DA_ALCA, ay * raio * RAIO_DA_ALCA);
        assert_eq!(pegar(ax, ay, raio, 45.0), Some(Pegada::Alca));
        assert_eq!(pegar(10.0, 10.0, raio, 45.0), Some(Pegada::Puck));
        assert_eq!(pegar(raio + 20.0, raio + 20.0, raio, 45.0), None);
    }

    /// Cada faixa acha os seus três controles na tabela, e cada um lê o campo
    /// que a faixa diz.
    #[test]
    fn cada_faixa_acha_os_seus_controles() {
        for faixa in Faixa::TODAS {
            for (rotulo, esperado) in [
                (faixa.rotulo_do_matiz(), "hue"),
                (faixa.rotulo_da_saturacao(), "sat"),
                (faixa.rotulo_da_luminancia(), "lum"),
            ] {
                let def = CONTROLES
                    .iter()
                    .find(|d| d.secao == Secao::Tonalizacao && d.rotulo == rotulo)
                    .unwrap_or_else(|| panic!("`{rotulo}` não está na tabela"));
                let campo = campo_do_controle(def);
                let prefixo = match faixa {
                    Faixa::Sombras => "split_shadow_",
                    Faixa::TonsMedios => "split_midtone_",
                    Faixa::Realces => "split_highlight_",
                    Faixa::Global => "split_global_",
                };
                assert_eq!(campo, format!("{prefixo}{esperado}"), "`{rotulo}`");
            }
        }
    }

    #[test]
    fn definir_e_neutralizar_mexem_so_na_faixa() {
        let mut a = Ajustes::default();
        Faixa::Realces.definir_cor(&mut a, 210.0, 35.0);
        a.split_highlight_lum = 12.0;
        a.split_shadow_sat = 20.0;
        assert_eq!(Faixa::Realces.ler(&a), (210.0, 35.0, 12.0));
        assert!(Faixa::Realces.alterada(&a));
        let mut sem = a;
        Faixa::Realces.neutralizar(&mut sem);
        assert!(!Faixa::Realces.alterada(&sem));
        assert_eq!(sem.split_shadow_sat, 20.0, "as outras faixas ficam");
    }

    #[test]
    fn sem_correcao_de_cores_e_o_neutro_do_painel() {
        let a = Ajustes {
            split_midtone_sat: 50.0,
            split_global_lum: -20.0,
            split_balance: 30.0,
            split_blending: 80.0,
            exposure: 0.5,
            ..Default::default()
        };
        let sem = sem_correcao_de_cores(&a);
        let neutro = Ajustes::default();
        assert_eq!(sem.split_blending, neutro.split_blending);
        assert_eq!(sem.split_balance, 0.0);
        assert!(Faixa::TODAS.iter().all(|f| !f.alterada(&sem)));
        assert_eq!(sem.exposure, 0.5, "o resto da revelação fica");
    }

    /// O disco é cinza no centro, colorido na borda, transparente fora.
    #[test]
    fn o_disco_e_cinza_no_meio_e_some_fora() {
        let lado = 64;
        let bytes = bgra_do_disco(lado);
        let em = |x: u32, y: u32| {
            let i = ((y * lado + x) * 4) as usize;
            [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]
        };
        let [b, g, r, a] = em(32, 32);
        assert_eq!(a, 255);
        assert!(
            r.abs_diff(g) < 12 && g.abs_diff(b) < 12,
            "centro cinza: {r} {g} {b}"
        );
        assert_eq!(em(0, 0)[3], 0, "o canto fica de fora");
        // Na borda direita (0°), a cor da roda do Lightroom no vermelho.
        let [b, g, r, _] = em(62, 32);
        assert!(r > g && r > b, "0° é vermelho: {r} {g} {b}");
    }
}
