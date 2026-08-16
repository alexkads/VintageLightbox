//! A geometria do corte: onde ficam as alças e o que cada arrasto faz.
//!
//! Só matemática, sem tela. É a metade do crop overlay que erra em silêncio — um
//! retângulo que anda quando devia encolher, uma alça que ignora o limite da foto
//! — e a única que dá para conferir sem olhar.
//!
//! O tipo que vai e volta é o [`CropSettings`] do `domain`, intacto: ele já
//! guarda as coordenadas normalizadas (0..1), o giro de 90°, o ângulo livre e os
//! dois espelhamentos, e já recusa valor fora de faixa no construtor.
//!
//! ## 🔑 Tudo aqui é normalizado, e o delta também
//!
//! Quem chama converte o movimento do ponteiro em fração da imagem **exibida**
//! (`dx = pixels / largura_na_tela`). Assim a mesma foto cortada numa janela
//! grande e numa pequena dá o mesmo corte — e é o que o legado faz, dividindo o
//! delta pelo `image_size` antes de aplicar.

use domain::value_objects::{AspectRatio, CropSettings};

/// O menor lado que um corte pode ter, em fração da imagem.
///
/// Os mesmos 1% do `CropSettings::MIN_CROP_SIZE` (que é privado lá). Sem um piso,
/// arrastar uma alça até o outro lado deixaria um retângulo de área zero — e sair
/// dele exigiria acertar um alvo de zero pixel.
const LADO_MINIMO: f32 = 0.01;

/// As oito alças, na ordem do legado: começa no canto superior esquerdo e gira no
/// sentido horário.
///
/// 🔑 A ordem importa porque é a mesma do índice que o legado usa
/// (`get_handle_positions`), e manter as duas iguais é o que permite comparar o
/// comportamento alça a alça durante a paridade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alca {
    SuperiorEsquerda,
    Superior,
    SuperiorDireita,
    Direita,
    InferiorDireita,
    Inferior,
    InferiorEsquerda,
    Esquerda,
}

impl Alca {
    pub const TODAS: [Alca; 8] = [
        Alca::SuperiorEsquerda,
        Alca::Superior,
        Alca::SuperiorDireita,
        Alca::Direita,
        Alca::InferiorDireita,
        Alca::Inferior,
        Alca::InferiorEsquerda,
        Alca::Esquerda,
    ];

    /// Onde ela fica, em fração do **retângulo de corte** (0..1 nos dois eixos).
    pub fn posicao(&self) -> (f32, f32) {
        match self {
            Alca::SuperiorEsquerda => (0.0, 0.0),
            Alca::Superior => (0.5, 0.0),
            Alca::SuperiorDireita => (1.0, 0.0),
            Alca::Direita => (1.0, 0.5),
            Alca::InferiorDireita => (1.0, 1.0),
            Alca::Inferior => (0.5, 1.0),
            Alca::InferiorEsquerda => (0.0, 1.0),
            Alca::Esquerda => (0.0, 0.5),
        }
    }

    /// Se ela move a borda esquerda (e portanto muda `x` junto com a largura).
    fn move_a_esquerda(&self) -> bool {
        matches!(
            self,
            Alca::SuperiorEsquerda | Alca::InferiorEsquerda | Alca::Esquerda
        )
    }

    fn move_a_direita(&self) -> bool {
        matches!(
            self,
            Alca::SuperiorDireita | Alca::InferiorDireita | Alca::Direita
        )
    }

    fn move_o_topo(&self) -> bool {
        matches!(
            self,
            Alca::SuperiorEsquerda | Alca::Superior | Alca::SuperiorDireita
        )
    }

    fn move_a_base(&self) -> bool {
        matches!(
            self,
            Alca::InferiorEsquerda | Alca::Inferior | Alca::InferiorDireita
        )
    }

    fn e_canto(&self) -> bool {
        matches!(
            self,
            Alca::SuperiorEsquerda
                | Alca::SuperiorDireita
                | Alca::InferiorDireita
                | Alca::InferiorEsquerda
        )
    }
}

/// Um retângulo normalizado, como o `CropSettings` guarda.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Caixa {
    x: f32,
    y: f32,
    largura: f32,
    altura: f32,
}

impl Caixa {
    fn de(corte: &CropSettings) -> Self {
        Self {
            x: corte.crop_x(),
            y: corte.crop_y(),
            largura: corte.crop_width(),
            altura: corte.crop_height(),
        }
    }

    /// Vira `CropSettings` **preservando** giro, ângulo e espelhamentos.
    ///
    /// 🚨 É o ponto em que se perde o resto do corte sem perceber: `CropSettings`
    /// tem oito campos, e reconstruí-lo com quatro deles zerados devolveria a foto
    /// à orientação original no meio de um arrasto de alça.
    fn para(self, original: &CropSettings) -> CropSettings {
        CropSettings::new(
            self.x,
            self.y,
            self.largura,
            self.altura,
            original.rotation_90(),
            original.angle(),
            original.flip_horizontal(),
            original.flip_vertical(),
        )
    }
}

/// Move uma alça, em fração da imagem exibida.
///
/// `proporcao` é o `largura / altura` a manter, em pixels — `None` para corte
/// livre. Ela vem de [`proporcao_de`], que sabe traduzir `AspectRatio::Original`.
pub fn mover_alca(
    corte: &CropSettings,
    alca: Alca,
    dx: f32,
    dy: f32,
    imagem: (f32, f32),
    proporcao: Option<f32>,
) -> CropSettings {
    let antes = Caixa::de(corte);
    let mut caixa = antes;

    // 🚨 Alça da esquerda move `x` **e** encolhe a largura na mesma medida. Só
    // mover `x` faria a caixa inteira deslizar em vez de redimensionar — é o erro
    // mais comum aqui, e ele parece "o corte anda sozinho".
    if alca.move_a_esquerda() {
        caixa.x += dx;
        caixa.largura -= dx;
    }
    if alca.move_a_direita() {
        caixa.largura += dx;
    }
    if alca.move_o_topo() {
        caixa.y += dy;
        caixa.altura -= dy;
    }
    if alca.move_a_base() {
        caixa.altura += dy;
    }

    if let Some(proporcao) = proporcao {
        caixa = ajustar_a_proporcao(caixa, antes, alca, dx, dy, imagem, proporcao);
    }

    caixa = respeitar_o_minimo(caixa, antes, alca);
    caixa = caber_na_imagem(caixa);

    // Coube na imagem à custa da proporção? Encolhe o lado que sobrou, em vez de
    // devolver um retângulo com a forma errada. É o que o legado faz no fim de
    // `update_crop_handle`, e sem isso arrastar até a borda com 16:9 travado
    // entrega um 16:10 silencioso.
    if let Some(proporcao) = proporcao {
        caixa = encolher_para_a_proporcao(caixa, imagem, proporcao);
    }

    caixa.para(corte)
}

/// Arrasta o retângulo inteiro, sem mudar o tamanho.
pub fn arrastar(corte: &CropSettings, dx: f32, dy: f32) -> CropSettings {
    let mut caixa = Caixa::de(corte);
    caixa.x += dx;
    caixa.y += dy;

    // 🔑 Aqui o limite **empurra**, e não encolhe: quem arrasta o retândulo quer
    // movê-lo, e chegar na borda tem de parar o movimento, não comer o corte.
    caixa.x = caixa.x.clamp(0.0, 1.0 - caixa.largura);
    caixa.y = caixa.y.clamp(0.0, 1.0 - caixa.altura);

    caixa.para(corte)
}

/// Gira 90° no sentido horário, como o botão do legado.
///
/// ⚠️ **O retângulo de corte não é girado junto.** Ele é guardado no espaço da
/// imagem original, e é o `CropSettings::to_visual_space` (no `domain`) que o
/// traduz para o que aparece na tela. Girar as coordenadas aqui aplicaria a
/// rotação duas vezes.
pub fn girar(corte: &CropSettings) -> CropSettings {
    let voltas = (corte.rotation_90() + 1).rem_euclid(4);
    CropSettings::new(
        corte.crop_x(),
        corte.crop_y(),
        corte.crop_width(),
        corte.crop_height(),
        voltas,
        corte.angle(),
        corte.flip_horizontal(),
        corte.flip_vertical(),
    )
}

pub fn espelhar_horizontal(corte: &CropSettings) -> CropSettings {
    CropSettings::new(
        corte.crop_x(),
        corte.crop_y(),
        corte.crop_width(),
        corte.crop_height(),
        corte.rotation_90(),
        corte.angle(),
        !corte.flip_horizontal(),
        corte.flip_vertical(),
    )
}

pub fn espelhar_vertical(corte: &CropSettings) -> CropSettings {
    CropSettings::new(
        corte.crop_x(),
        corte.crop_y(),
        corte.crop_width(),
        corte.crop_height(),
        corte.rotation_90(),
        corte.angle(),
        corte.flip_horizontal(),
        !corte.flip_vertical(),
    )
}

/// Muda o ângulo livre, em graus. `CropSettings` limita a ±45.
pub fn inclinar(corte: &CropSettings, graus: f32) -> CropSettings {
    CropSettings::new(
        corte.crop_x(),
        corte.crop_y(),
        corte.crop_width(),
        corte.crop_height(),
        corte.rotation_90(),
        corte.angle() + graus,
        corte.flip_horizontal(),
        corte.flip_vertical(),
    )
}

/// O `largura / altura` que a proporção escolhida exige, em pixels.
///
/// `Free` não exige nada; `Original` depende do tamanho da foto, e por isso não
/// dá para tirar do enum sozinho.
pub fn proporcao_de(escolha: &AspectRatio, imagem: (f32, f32)) -> Option<f32> {
    match escolha {
        AspectRatio::Free => None,
        AspectRatio::Original => Some(imagem.0 / imagem.1),
        outra => Some(outra.value()),
    }
}

/// Um corte que ocupa a foto inteira — o estado ao entrar no modo de corte numa
/// foto que nunca foi cortada.
pub fn foto_inteira() -> CropSettings {
    CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 0.0, false, false)
}

/// Onde a foto fica dentro do palco, em pixels: `(x, y, largura, altura)`.
///
/// 🚨 **O overlay precisa deste retângulo, e não do palco.** A foto é desenhada
/// com `ObjectFit::Contain` — cabe inteira, deixando faixa vazia num dos eixos.
/// Um overlay do tamanho do palco poria o retângulo de corte sobre a faixa vazia,
/// e o corte sairia deslocado em relação ao que se vê: em foto deitada numa
/// janela alta, a diferença é a altura das duas tarjas.
///
/// Palco ou foto com lado zero devolve um retângulo vazio na origem — não há
/// divisão por zero, e o overlay simplesmente não aparece.
pub fn area_da_foto(palco: (f32, f32), foto: (f32, f32)) -> (f32, f32, f32, f32) {
    if palco.0 <= 0.0 || palco.1 <= 0.0 || foto.0 <= 0.0 || foto.1 <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }

    let escala = (palco.0 / foto.0).min(palco.1 / foto.1);
    let largura = foto.0 * escala;
    let altura = foto.1 * escala;

    (
        (palco.0 - largura) / 2.0,
        (palco.1 - altura) / 2.0,
        largura,
        altura,
    )
}

/// Ajusta a dimensão secundária para manter a proporção.
///
/// Em alça de canto, quem manda é o eixo que mais se moveu — arrastar na
/// diagonal precisa escolher um dos dois, e escolher o menor faria o retângulo
/// parecer preso. Em alça de aresta, quem manda é o eixo dela.
fn ajustar_a_proporcao(
    mut caixa: Caixa,
    antes: Caixa,
    alca: Alca,
    dx: f32,
    dy: f32,
    imagem: (f32, f32),
    proporcao: f32,
) -> Caixa {
    let horizontal_manda = if alca.e_canto() {
        dx.abs() >= dy.abs()
    } else {
        alca.move_a_esquerda() || alca.move_a_direita()
    };

    if horizontal_manda {
        let altura_em_pixels = (caixa.largura * imagem.0) / proporcao;
        let nova_altura = altura_em_pixels / imagem.1;
        // A borda que **não** está sendo arrastada fica parada: mexer na alça de
        // cima não pode mover a de baixo.
        if alca.move_o_topo() {
            caixa.y = antes.y + antes.altura - nova_altura;
        } else if !alca.move_a_base() {
            // Aresta lateral: cresce para os dois lados, mantendo o centro.
            caixa.y = antes.y + (antes.altura - nova_altura) / 2.0;
        }
        caixa.altura = nova_altura;
    } else {
        let largura_em_pixels = (caixa.altura * imagem.1) * proporcao;
        let nova_largura = largura_em_pixels / imagem.0;
        if alca.move_a_esquerda() {
            caixa.x = antes.x + antes.largura - nova_largura;
        } else if !alca.move_a_direita() {
            caixa.x = antes.x + (antes.largura - nova_largura) / 2.0;
        }
        caixa.largura = nova_largura;
    }

    caixa
}

/// Nenhum lado abaixo de [`LADO_MINIMO`], e a borda que não se move fica parada.
fn respeitar_o_minimo(mut caixa: Caixa, antes: Caixa, alca: Alca) -> Caixa {
    if caixa.largura < LADO_MINIMO {
        if alca.move_a_esquerda() {
            // Encolheu puxando a borda esquerda para a direita: o mínimo tem de
            // ficar encostado na borda **direita**, que não se moveu.
            caixa.x = antes.x + antes.largura - LADO_MINIMO;
        }
        caixa.largura = LADO_MINIMO;
    }
    if caixa.altura < LADO_MINIMO {
        if alca.move_o_topo() {
            caixa.y = antes.y + antes.altura - LADO_MINIMO;
        }
        caixa.altura = LADO_MINIMO;
    }
    caixa
}

/// Empurra e encolhe o que passou de 0..1.
///
/// ⚠️ Passar da borda **encolhe**, e não desloca: arrastar a alça esquerda para
/// fora da foto não pode empurrar a direita para dentro.
fn caber_na_imagem(mut caixa: Caixa) -> Caixa {
    if caixa.x < 0.0 {
        caixa.largura += caixa.x;
        caixa.x = 0.0;
    }
    if caixa.y < 0.0 {
        caixa.altura += caixa.y;
        caixa.y = 0.0;
    }
    caixa.largura = caixa.largura.min(1.0 - caixa.x).max(LADO_MINIMO);
    caixa.altura = caixa.altura.min(1.0 - caixa.y).max(LADO_MINIMO);
    caixa
}

fn encolher_para_a_proporcao(mut caixa: Caixa, imagem: (f32, f32), proporcao: f32) -> Caixa {
    let largura_em_pixels = caixa.largura * imagem.0;
    let altura_em_pixels = caixa.altura * imagem.1;
    let atual = largura_em_pixels / altura_em_pixels;

    if (atual - proporcao).abs() <= 0.01 {
        return caixa;
    }

    if atual > proporcao {
        caixa.largura = (altura_em_pixels * proporcao) / imagem.0;
    } else {
        caixa.altura = (largura_em_pixels / proporcao) / imagem.1;
    }
    caixa
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Uma foto quadrada de 1000×1000 deixa a conta de proporção legível: fração
    /// e pixel viram o mesmo número.
    const QUADRADA: (f32, f32) = (1000.0, 1000.0);

    fn meio() -> CropSettings {
        CropSettings::new(0.25, 0.25, 0.5, 0.5, 0, 0.0, false, false)
    }

    fn quatro(corte: &CropSettings) -> (f32, f32, f32, f32) {
        (
            corte.crop_x(),
            corte.crop_y(),
            corte.crop_width(),
            corte.crop_height(),
        )
    }

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// 🚨 A alça da esquerda encolhe, e a borda direita fica onde estava.
    ///
    /// Este é o erro clássico do corte: mover `x` sem descontar da largura faz o
    /// retângulo inteiro deslizar. Na tela isso parece "o corte anda sozinho
    /// quando eu tento apertá-lo", e ninguém associa a causa.
    #[test]
    fn a_alca_esquerda_encolhe_sem_mover_a_direita() {
        let antes = meio();
        let direita_antes = antes.crop_x() + antes.crop_width();

        let depois = mover_alca(&antes, Alca::Esquerda, 0.1, 0.0, QUADRADA, None);

        assert!(perto(depois.crop_x(), 0.35));
        assert!(perto(depois.crop_width(), 0.4));
        assert!(
            perto(depois.crop_x() + depois.crop_width(), direita_antes),
            "a borda direita não pode se mexer"
        );
        assert!(perto(depois.crop_y(), antes.crop_y()), "e nem a vertical");
        assert!(perto(depois.crop_height(), antes.crop_height()));
    }

    /// Cada alça mexe só nas bordas que ela toca.
    #[test]
    fn cada_alca_move_as_bordas_que_lhe_cabem() {
        let antes = meio();
        let (x0, y0, w0, h0) = quatro(&antes);
        let (d, dy) = (0.05, 0.05);

        for alca in Alca::TODAS {
            let depois = mover_alca(&antes, alca, d, dy, QUADRADA, None);
            let (x, y, w, h) = quatro(&depois);

            let esquerda_mudou = !perto(x, x0);
            let direita_mudou = !perto(x + w, x0 + w0);
            let topo_mudou = !perto(y, y0);
            let base_mudou = !perto(y + h, y0 + h0);

            assert_eq!(
                esquerda_mudou,
                alca.move_a_esquerda(),
                "{alca:?} e a borda esquerda"
            );
            assert_eq!(
                direita_mudou,
                alca.move_a_direita(),
                "{alca:?} e a borda direita"
            );
            assert_eq!(topo_mudou, alca.move_o_topo(), "{alca:?} e o topo");
            assert_eq!(base_mudou, alca.move_a_base(), "{alca:?} e a base");
        }
    }

    /// 🚨 Nenhum arrasto de alça perde giro, ângulo ou espelhamento.
    ///
    /// `CropSettings` tem oito campos e a geometria só mexe em quatro. Reconstruir
    /// o valor esquecendo os outros devolveria a foto à orientação original no
    /// meio de um arrasto — sem erro, e parecendo que a tela "pulou".
    #[test]
    fn o_arrasto_preserva_giro_angulo_e_espelhos() {
        let antes = CropSettings::new(0.2, 0.2, 0.5, 0.5, 3, -12.5, true, true);

        for alca in Alca::TODAS {
            let depois = mover_alca(&antes, alca, 0.05, -0.03, QUADRADA, None);
            assert_eq!(depois.rotation_90(), 3, "{alca:?} perdeu o giro");
            assert!(perto(depois.angle(), -12.5), "{alca:?} perdeu o ângulo");
            assert!(depois.flip_horizontal(), "{alca:?} perdeu o espelho h");
            assert!(depois.flip_vertical(), "{alca:?} perdeu o espelho v");
        }

        let arrastado = arrastar(&antes, 0.1, 0.1);
        assert_eq!(arrastado.rotation_90(), 3);
        assert!(perto(arrastado.angle(), -12.5));
    }

    /// 🚨 Arrastar o retângulo **empurra** no limite; não encolhe.
    ///
    /// Quem arrasta quer mover. Se a borda comesse o corte, chegar na margem
    /// mudaria o enquadramento sem ninguém ter puxado alça nenhuma.
    #[test]
    fn arrastar_ate_a_borda_para_sem_encolher() {
        let antes = meio();
        let depois = arrastar(&antes, 10.0, 10.0);

        assert!(perto(depois.crop_width(), 0.5), "o tamanho é intocado");
        assert!(perto(depois.crop_height(), 0.5));
        assert!(perto(depois.crop_x(), 0.5), "encostou na borda direita");
        assert!(perto(depois.crop_y(), 0.5));
    }

    /// ⚠️ Puxar uma alça para fora da foto **encolhe** — e não empurra o outro
    /// lado.
    #[test]
    fn alca_puxada_para_fora_encolhe_em_vez_de_deslocar() {
        let antes = meio();
        let direita_antes = antes.crop_x() + antes.crop_width();

        let depois = mover_alca(&antes, Alca::Esquerda, -10.0, 0.0, QUADRADA, None);

        assert!(perto(depois.crop_x(), 0.0), "parou na borda da foto");
        assert!(
            perto(depois.crop_x() + depois.crop_width(), direita_antes),
            "a borda direita continua parada"
        );
    }

    /// 🚨 O lado mínimo encosta na borda que **não** está sendo arrastada.
    ///
    /// Apertar a alça esquerda até o fim tem de deixar a fita de 1% colada na
    /// borda direita. Se ela ficasse na esquerda, o retângulo teria saltado a
    /// largura inteira no último milímetro de arrasto.
    #[test]
    fn o_minimo_fica_colado_na_borda_parada() {
        let antes = meio();
        let direita = antes.crop_x() + antes.crop_width();

        let depois = mover_alca(&antes, Alca::Esquerda, 10.0, 0.0, QUADRADA, None);

        assert!(perto(depois.crop_width(), LADO_MINIMO));
        assert!(
            perto(depois.crop_x(), direita - LADO_MINIMO),
            "o mínimo tem de nascer encostado na direita, e não na esquerda"
        );
    }

    /// A proporção travada mantém a forma quando se arrasta um canto.
    #[test]
    fn a_proporcao_travada_segue_o_eixo_que_mais_se_moveu() {
        let antes = CropSettings::new(0.1, 0.1, 0.4, 0.4, 0, 0.0, false, false);

        // Arrasto quase horizontal no canto inferior direito, com 1:1 travado.
        let depois = mover_alca(
            &antes,
            Alca::InferiorDireita,
            0.2,
            0.01,
            QUADRADA,
            Some(1.0),
        );

        assert!(
            perto(depois.crop_width(), depois.crop_height()),
            "1:1 numa foto quadrada tem de sair quadrado: {}×{}",
            depois.crop_width(),
            depois.crop_height()
        );
        assert!(depois.crop_width() > antes.crop_width());
    }

    /// 🚨 Chegar na borda com proporção travada **encolhe o outro lado**.
    ///
    /// Sem isto, o clamp entrega um retângulo com a forma errada — 16:9 pedido,
    /// 16:10 entregue — e o desvio só aparece na exportação.
    #[test]
    fn a_proporcao_sobrevive_ao_limite_da_foto() {
        let antes = CropSettings::new(0.6, 0.6, 0.3, 0.3, 0, 0.0, false, false);

        let depois = mover_alca(&antes, Alca::InferiorDireita, 0.5, 0.5, QUADRADA, Some(1.0));

        assert!(
            perto(depois.crop_width(), depois.crop_height()),
            "encostou na borda e continuou quadrado: {}×{}",
            depois.crop_width(),
            depois.crop_height()
        );
        assert!(depois.crop_x() + depois.crop_width() <= 1.0 + 1e-4);
        assert!(depois.crop_y() + depois.crop_height() <= 1.0 + 1e-4);
    }

    /// ⚠️ `Original` depende do tamanho da foto; `Free` não trava nada.
    #[test]
    fn a_proporcao_original_vem_da_foto_e_a_livre_de_lugar_nenhum() {
        assert_eq!(proporcao_de(&AspectRatio::Free, (3000.0, 2000.0)), None);
        assert_eq!(
            proporcao_de(&AspectRatio::Original, (3000.0, 2000.0)),
            Some(1.5)
        );
        assert_eq!(
            proporcao_de(&AspectRatio::Square, (3000.0, 2000.0)),
            Some(1.0)
        );
    }

    /// Girar quatro vezes volta ao começo, e o retângulo não se mexe.
    ///
    /// 🚨 O corte é guardado no espaço da imagem **original** — é o
    /// `to_visual_space` do `domain` que o traduz para a tela. Girar as
    /// coordenadas aqui aplicaria a rotação duas vezes, e o enquadramento saltaria
    /// para outro canto a cada clique.
    #[test]
    fn girar_quatro_vezes_volta_ao_comeco() {
        let antes = meio();
        let mut girado = antes.clone();

        for volta in 1..=4 {
            girado = girar(&girado);
            assert_eq!(girado.rotation_90(), volta % 4);
            assert_eq!(
                quatro(&girado),
                quatro(&antes),
                "o retângulo não gira junto"
            );
        }
    }

    #[test]
    fn espelhar_duas_vezes_desfaz() {
        let antes = meio();
        assert!(espelhar_horizontal(&antes).flip_horizontal());
        assert!(!espelhar_horizontal(&espelhar_horizontal(&antes)).flip_horizontal());
        assert!(espelhar_vertical(&antes).flip_vertical());
        assert!(!espelhar_vertical(&espelhar_vertical(&antes)).flip_vertical());
    }

    /// O ângulo livre para em ±45°, como o `CropSettings` define.
    #[test]
    fn o_angulo_livre_para_nos_quarenta_e_cinco() {
        let antes = meio();
        assert!(perto(inclinar(&antes, 10.0).angle(), 10.0));
        assert!(perto(inclinar(&antes, 90.0).angle(), 45.0));
        assert!(perto(inclinar(&antes, -90.0).angle(), -45.0));
    }

    /// As oito alças ficam nos oito lugares certos, sem repetir.
    #[test]
    fn as_oito_alcas_ocupam_oito_lugares() {
        let mut vistas = Vec::new();
        for alca in Alca::TODAS {
            let posicao = alca.posicao();
            assert!(
                !vistas.contains(&posicao),
                "{alca:?} repete a posição de outra"
            );
            vistas.push(posicao);
        }
        assert_eq!(vistas.len(), 8);
    }

    /// 🚨 A foto deitada num palco alto deixa faixa em cima e embaixo — e o
    /// overlay tem de ficar sobre a foto, não sobre a faixa.
    #[test]
    fn a_area_da_foto_centraliza_e_cabe_inteira() {
        // Foto 2:1 num palco quadrado: a largura enche, sobra metade da altura.
        let (x, y, w, h) = area_da_foto((800.0, 800.0), (2000.0, 1000.0));
        assert!(perto(x, 0.0));
        assert!(perto(w, 800.0));
        assert!(perto(h, 400.0));
        assert!(perto(y, 200.0), "as duas tarjas têm a mesma altura");

        // Foto em pé num palco largo: agora sobra dos lados.
        let (x, y, w, h) = area_da_foto((800.0, 800.0), (1000.0, 2000.0));
        assert!(perto(y, 0.0));
        assert!(perto(h, 800.0));
        assert!(perto(w, 400.0));
        assert!(perto(x, 200.0));
    }

    /// ⚠️ Palco ou foto de lado zero não divide por zero: some.
    ///
    /// Acontece de verdade — no primeiro quadro, antes do layout, o palco mede
    /// zero. Um `NaN` ali viraria um retângulo em lugar nenhum, e o GPUI desenha
    /// `NaN` como coisa nenhuma, sem reclamar.
    #[test]
    fn palco_ou_foto_sem_tamanho_nao_gera_nan() {
        for caso in [
            ((0.0, 600.0), (100.0, 100.0)),
            ((800.0, 0.0), (100.0, 100.0)),
            ((800.0, 600.0), (0.0, 100.0)),
            ((800.0, 600.0), (100.0, 0.0)),
        ] {
            let (x, y, w, h) = area_da_foto(caso.0, caso.1);
            assert_eq!((x, y, w, h), (0.0, 0.0, 0.0, 0.0), "caso {caso:?}");
        }
    }

    /// Entrar no modo de corte numa foto sem corte pega a foto inteira.
    #[test]
    fn a_foto_inteira_e_o_ponto_de_partida() {
        let inteira = foto_inteira();
        assert_eq!(quatro(&inteira), (0.0, 0.0, 1.0, 1.0));
        assert!(!inteira.is_cropped(), "foto inteira não é corte");
    }
}
