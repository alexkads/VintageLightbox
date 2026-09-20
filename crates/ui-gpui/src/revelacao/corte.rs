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

/// Gira um quarto de volta no sentido horário, e **leva o retângulo junto**
/// (`girar` do site).
///
/// 🔑 O retângulo mora no espaço já girado (ver `revelacao_core::transformacao`):
/// girar o espaço sem girar o retângulo o deixaria descrevendo outra região da
/// foto. A conta é a rotação do retângulo unitário, `(x, y) → (1 − y − altura, x)`,
/// com os lados trocados.
pub fn girar(corte: &CropSettings) -> CropSettings {
    CropSettings::new(
        1.0 - corte.crop_y() - corte.crop_height(),
        corte.crop_x(),
        corte.crop_height(),
        corte.crop_width(),
        (corte.rotation_90() + 1).rem_euclid(4),
        corte.angle(),
        corte.flip_horizontal(),
        corte.flip_vertical(),
    )
}

/// Três quartos de volta: o "girar à esquerda" do painel.
pub fn girar_a_esquerda(corte: &CropSettings) -> CropSettings {
    girar(&girar(&girar(corte)))
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

/// O maior ângulo do endireitamento, como no site (`ANGULO_MAXIMO`).
pub const ANGULO_MAXIMO: f32 = 45.;

/// Um retângulo em pixels do espaço girado (depois de espelhos e giro de 90°).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// O retângulo do corte em pixels do espaço (`retanguloDe` do site).
pub fn retangulo_de(corte: &CropSettings, espaco: (f32, f32)) -> Retangulo {
    Retangulo {
        x: corte.crop_x() * espaco.0,
        y: corte.crop_y() * espaco.1,
        w: corte.crop_width() * espaco.0,
        h: corte.crop_height() * espaco.1,
    }
}

/// O caminho inverso (`comRetangulo`).
pub fn com_retangulo(corte: &CropSettings, r: Retangulo, espaco: (f32, f32)) -> CropSettings {
    CropSettings::new(
        r.x / espaco.0,
        r.y / espaco.1,
        r.w / espaco.0,
        r.h / espaco.1,
        corte.rotation_90(),
        corte.angle(),
        corte.flip_horizontal(),
        corte.flip_vertical(),
    )
}

/// O ponto está dentro da foto depois de o ângulo girá-la? A foto gira em
/// torno do centro do espaço, e basta desfazer o giro no ponto.
fn dentro_da_foto_girada(px: f32, py: f32, espaco: (f32, f32), angulo: f32) -> bool {
    let (cx, cy) = (espaco.0 / 2., espaco.1 / 2.);
    let (sin, cos) = (-angulo.to_radians()).sin_cos();
    let (dx, dy) = (px - cx, py - cy);
    let x = cx + dx * cos - dy * sin;
    let y = cy + dx * sin + dy * cos;
    let folga = 0.5;
    x >= -folga && y >= -folga && x <= espaco.0 + folga && y <= espaco.1 + folga
}

/// O retângulo cabe inteiro na foto girada — sem canto vazio?
pub fn cabe_na_foto_girada(r: Retangulo, espaco: (f32, f32), angulo: f32) -> bool {
    if r.x < -0.5 || r.y < -0.5 {
        return false;
    }
    if r.x + r.w > espaco.0 + 0.5 || r.y + r.h > espaco.1 + 0.5 {
        return false;
    }
    if angulo == 0. {
        return true;
    }
    [
        (r.x, r.y),
        (r.x + r.w, r.y),
        (r.x, r.y + r.h),
        (r.x + r.w, r.y + r.h),
    ]
    .iter()
    .all(|(x, y)| dentro_da_foto_girada(*x, *y, espaco, angulo))
}

fn escalar_no_centro(r: Retangulo, s: f32, cx: f32, cy: f32) -> Retangulo {
    let (w, h) = (r.w * s, r.h * s);
    Retangulo {
        x: cx - w / 2.,
        y: cy - h / 2.,
        w,
        h,
    }
}

/// O maior retângulo com a mesma proporção e o mesmo centro que cabe na foto
/// girada — o "zoom do endireitar" (dono, 2026-09-05: *"o endireitar precisa
/// dar zoom para preencher os espaços não preenchidos"*).
pub fn encolher_para_caber(r: Retangulo, espaco: (f32, f32), angulo: f32) -> Retangulo {
    // Arredondar para dentro, nunca para fora: um pixel a mais é o canto vazio
    // de volta.
    let arredondar = |v: Retangulo| {
        let x = (v.x - 1e-3).ceil().max(0.);
        let y = (v.y - 1e-3).ceil().max(0.);
        Retangulo {
            x,
            y,
            w: ((v.x + v.w + 1e-3).floor() - x).max(1.),
            h: ((v.y + v.h + 1e-3).floor() - y).max(1.),
        }
    };
    if cabe_na_foto_girada(r, espaco, angulo) {
        return arredondar(r);
    }
    let (mut cx, mut cy) = (r.x + r.w / 2., r.y + r.h / 2.);
    if !dentro_da_foto_girada(cx, cy, espaco, angulo) {
        cx = espaco.0 / 2.;
        cy = espaco.1 / 2.;
    }
    let (mut cabe, mut nao_cabe) = (0f32, 1f32);
    for _ in 0..40 {
        let meio = (cabe + nao_cabe) / 2.;
        if cabe_na_foto_girada(escalar_no_centro(r, meio, cx, cy), espaco, angulo) {
            cabe = meio;
        } else {
            nao_cabe = meio;
        }
    }
    arredondar(escalar_no_centro(r, cabe, cx, cy))
}

/// Endireita, e traz o retângulo para dentro da foto (`endireitar` do site).
///
/// `desejado` é o retângulo que o operador pediu antes de qualquer
/// encolhimento: é o que faz o retângulo **crescer de volta** quando o ângulo
/// volta a zero.
pub fn endireitar(
    corte: &CropSettings,
    angulo: f32,
    espaco: (f32, f32),
    desejado: Option<Retangulo>,
) -> CropSettings {
    let limitado = angulo.clamp(-ANGULO_MAXIMO, ANGULO_MAXIMO);
    let alvo = desejado.unwrap_or_else(|| retangulo_de(corte, espaco));
    let cabendo = encolher_para_caber(alvo, espaco, limitado);
    let com_angulo = CropSettings::new(
        corte.crop_x(),
        corte.crop_y(),
        corte.crop_width(),
        corte.crop_height(),
        corte.rotation_90(),
        limitado,
        corte.flip_horizontal(),
        corte.flip_vertical(),
    );
    com_retangulo(&com_angulo, cabendo, espaco)
}

/// O retângulo remodelado para uma proporção, **na hora**
/// (`comProporcaoNoCentro` do site).
///
/// A **área é preservada**, e não um dos lados: trocar 3:2 por 2:3 mantendo a
/// largura daria um retângulo altíssimo que a foto corta em seguida. Depois
/// disso, cabe no espaço e cabe na foto girada.
pub fn com_proporcao_no_centro(
    r: Retangulo,
    proporcao: f32,
    espaco: (f32, f32),
    angulo: f32,
) -> Retangulo {
    let area = (r.w * r.h).max(1.);
    let mut w = (area * proporcao).sqrt();
    let mut h = w / proporcao;
    let couber = 1f32.min(espaco.0 / w).min(espaco.1 / h);
    w *= couber;
    h *= couber;
    let (cx, cy) = (r.x + r.w / 2., r.y + r.h / 2.);
    let x = (cx - w / 2.).min(espaco.0 - w).max(0.);
    let y = (cy - h / 2.).min(espaco.1 - h).max(0.);
    encolher_para_caber(Retangulo { x, y, w, h }, espaco, angulo)
}

/// As proporções que o painel oferece, na ordem do site. `None` é livre.
pub const PROPORCOES: [(&str, Option<f32>); 7] = [
    ("Livre", None),
    ("1:1", Some(1.)),
    ("3:2", Some(3. / 2.)),
    ("2:3", Some(2. / 3.)),
    ("4:3", Some(4. / 3.)),
    ("3:4", Some(3. / 4.)),
    ("16:9", Some(16. / 9.)),
];

/// O retângulo depois de um arrasto, **em pixels do espaço girado**
/// (`arrastar` do site).
///
/// `alca` `None` é o meio: só move. Com proporção, a borda que o dedo move
/// manda e a outra acompanha, a borda oposta fica parada, e o resultado é
/// limitado à imagem — encolhendo, nunca estourando.
pub fn arrastar_em_pixels(
    inicial: Retangulo,
    alca: Option<Alca>,
    dx: f32,
    dy: f32,
    limite: (f32, f32),
    proporcao: Option<f32>,
) -> Retangulo {
    let minimo = (LADO_MINIMO * limite.0.min(limite.1)).round().max(1.);

    let Some(alca) = alca else {
        return Retangulo {
            x: (inicial.x + dx).min(limite.0 - inicial.w).max(0.),
            y: (inicial.y + dy).min(limite.1 - inicial.h).max(0.),
            ..inicial
        };
    };

    let mut esquerda = inicial.x;
    let mut topo = inicial.y;
    let mut direita = inicial.x + inicial.w;
    let mut base = inicial.y + inicial.h;

    if alca.move_a_esquerda() {
        esquerda = (esquerda + dx).max(0.).min(direita - minimo);
    }
    if alca.move_a_direita() {
        direita = (direita + dx).min(limite.0).max(esquerda + minimo);
    }
    if alca.move_o_topo() {
        topo = (topo + dy).max(0.).min(base - minimo);
    }
    if alca.move_a_base() {
        base = (base + dy).min(limite.1).max(topo + minimo);
    }

    if let Some(proporcao) = proporcao {
        let mut w = direita - esquerda;
        let mut h = base - topo;
        // Quem manda é o eixo que a alça move: numa borda horizontal, a
        // altura; num canto ou borda vertical, a largura.
        if matches!(alca, Alca::Superior | Alca::Inferior) {
            w = h * proporcao;
        } else {
            h = w / proporcao;
        }
        w = w.min(limite.0);
        h = h.min(limite.1);
        if w / h > proporcao {
            w = h * proporcao;
        } else {
            h = w / proporcao;
        }

        // A borda oposta à que se moveu é a âncora.
        if alca.move_a_esquerda() {
            esquerda = direita - w;
        } else {
            direita = esquerda + w;
        }
        if alca.move_o_topo() {
            topo = base - h;
        } else {
            base = topo + h;
        }

        // Se a âncora empurrou para fora, traz de volta sem mudar o tamanho.
        if esquerda < 0. {
            direita -= esquerda;
            esquerda = 0.;
        }
        if topo < 0. {
            base -= topo;
            topo = 0.;
        }
        if direita > limite.0 {
            esquerda -= direita - limite.0;
            direita = limite.0;
        }
        if base > limite.1 {
            topo -= base - limite.1;
            base = limite.1;
        }
        esquerda = esquerda.max(0.);
        topo = topo.max(0.);
    }

    // `round` do JavaScript: meio sobe.
    let arredondar = |v: f32| (v + 0.5).floor();
    Retangulo {
        x: arredondar(esquerda),
        y: arredondar(topo),
        w: arredondar((direita - esquerda).max(minimo)),
        h: arredondar((base - topo).max(minimo)),
    }
}

/// O espaço girado de uma foto: os lados trocam com giro ímpar.
pub fn espaco_de(corte: &CropSettings, foto: (f32, f32)) -> (f32, f32) {
    if corte.rotation_90().rem_euclid(4) % 2 == 1 {
        (foto.1, foto.0)
    } else {
        foto
    }
}

/// O enquadramento não muda nada? A folga é a do `ehCorteInteiro` do site.
pub fn e_inteiro(corte: &CropSettings) -> bool {
    const FOLGA: f32 = 1e-4;
    corte.rotation_90().rem_euclid(4) == 0
        && corte.angle() == 0.
        && !corte.flip_horizontal()
        && !corte.flip_vertical()
        && corte.crop_x().abs() < FOLGA
        && corte.crop_y().abs() < FOLGA
        && (corte.crop_width() - 1.).abs() < FOLGA
        && (corte.crop_height() - 1.).abs() < FOLGA
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
        }
        // O retângulo gira junto, e quatro quartos o devolvem ao lugar.
        let (a, b) = (quatro(&girado), quatro(&antes));
        assert!(perto(a.0, b.0) && perto(a.1, b.1) && perto(a.2, b.2) && perto(a.3, b.3));
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

    fn r(x: f32, y: f32, w: f32, h: f32) -> Retangulo {
        Retangulo { x, y, w, h }
    }

    const ESPACO: (f32, f32) = (1000., 800.);

    /// O retângulo mora no espaço girado: girar sem levá-lo junto faria o
    /// enquadramento saltar para outra região da foto.
    #[test]
    fn girar_leva_o_retangulo_junto() {
        let c = CropSettings::new(0., 0., 0.5, 0.25, 0, 0., false, false);
        let girado = girar(&c);
        assert_eq!(girado.rotation_90(), 1);
        assert!(perto(girado.crop_x(), 0.75));
        assert!(perto(girado.crop_y(), 0.));
        assert!(perto(girado.crop_width(), 0.25));
        assert!(perto(girado.crop_height(), 0.5));
        let volta = girar_a_esquerda(&girado);
        assert_eq!(volta.rotation_90(), 0);
        assert!(perto(volta.crop_x(), 0.) && perto(volta.crop_width(), 0.5));
        assert!(e_inteiro(&girar(&girar(&girar(&girar(&foto_inteira()))))));
        assert!(!e_inteiro(&girar(&foto_inteira())));
    }

    #[test]
    fn arrastar_em_pixels_como_no_site() {
        let meio = r(200., 160., 400., 320.);
        assert_eq!(
            arrastar_em_pixels(meio, None, 50., -30., ESPACO, None),
            r(250., 130., 400., 320.)
        );
        assert_eq!(
            arrastar_em_pixels(meio, None, -9999., -9999., ESPACO, None),
            r(0., 0., 400., 320.)
        );
        assert_eq!(
            arrastar_em_pixels(meio, None, 9999., 9999., ESPACO, None),
            r(600., 480., 400., 320.)
        );
        assert_eq!(
            arrastar_em_pixels(meio, Some(Alca::Esquerda), 100., 0., ESPACO, None),
            r(300., 160., 300., 320.)
        );
        assert_eq!(
            arrastar_em_pixels(meio, Some(Alca::Inferior), 0., 100., ESPACO, None),
            r(200., 160., 400., 420.)
        );
        assert_eq!(
            arrastar_em_pixels(meio, Some(Alca::SuperiorDireita), 50., 50., ESPACO, None),
            r(200., 210., 450., 270.)
        );
        for alca in Alca::TODAS {
            for (dx, dy) in [(9999., 9999.), (-9999., -9999.)] {
                let v = arrastar_em_pixels(meio, Some(alca), dx, dy, ESPACO, None);
                assert!(v.x >= 0. && v.y >= 0., "{alca:?}");
                assert!(v.x + v.w <= ESPACO.0 && v.y + v.h <= ESPACO.1, "{alca:?}");
                assert!(v.w >= 8. && v.h >= 8., "{alca:?}");
            }
            for (rotulo, valor) in PROPORCOES {
                let Some(valor) = valor else { continue };
                let v = arrastar_em_pixels(meio, Some(alca), 70., 70., ESPACO, Some(valor));
                assert!((v.w / v.h - valor).abs() < 0.05, "{rotulo} em {alca:?}");
                assert!(v.x + v.w <= ESPACO.0 + 1., "{rotulo} em {alca:?}");
                assert!(v.y + v.h <= ESPACO.1 + 1., "{rotulo} em {alca:?}");
            }
        }
        let v = arrastar_em_pixels(
            r(0., 0., 100., 100.),
            Some(Alca::InferiorDireita),
            9999.,
            9999.,
            ESPACO,
            Some(1.),
        );
        assert_eq!(v.w, v.h);
        assert!(v.w <= ESPACO.1);
    }

    #[test]
    fn a_proporcao_remodela_na_hora_mantendo_a_area() {
        let espaco = (1000., 600.);
        let base = r(200., 100., 600., 400.);
        let q = com_proporcao_no_centro(base, 1., espaco, 0.);
        assert_eq!(q.w, q.h);
        assert!((q.x + q.w / 2. - 500.).abs() <= 1.);
        assert!((q.y + q.h / 2. - 300.).abs() <= 1.);

        let deitado = r(100., 100., 300., 200.);
        let empe = com_proporcao_no_centro(deitado, 2. / 3., espaco, 0.);
        assert!((empe.w / empe.h - 2. / 3.).abs() < 0.01);
        assert!((empe.w * empe.h - 60_000.).abs() < 1000.);

        let inteiro = r(0., 0., 1000., 600.);
        let largo = com_proporcao_no_centro(inteiro, 16. / 9., espaco, 0.);
        assert!(largo.x >= 0. && largo.y >= 0.);
        assert!(largo.x + largo.w <= 1001. && largo.y + largo.h <= 601.);

        let girado = com_proporcao_no_centro(inteiro, 1., espaco, -18.);
        assert!(cabe_na_foto_girada(girado, espaco, -18.));
        assert!((girado.w / girado.h - 1.).abs() < 0.05);
    }

    #[test]
    fn o_espaco_troca_os_lados_com_giro_impar() {
        let c = girar(&foto_inteira());
        assert_eq!(espaco_de(&c, (300., 200.)), (200., 300.));
        assert_eq!(espaco_de(&foto_inteira(), (300., 200.)), (300., 200.));
    }

    #[test]
    fn endireitar_encolhe_para_caber_e_cresce_de_volta() {
        let espaco = (3000., 2000.);
        let inteiro = foto_inteira();
        let desejado = retangulo_de(&inteiro, espaco);
        let girado = endireitar(&inteiro, 10., espaco, Some(desejado));
        assert_eq!(girado.angle(), 10.);
        assert!(
            girado.crop_width() < 1.,
            "encolheu para não deixar canto vazio"
        );
        let r = retangulo_de(&girado, espaco);
        assert!(cabe_na_foto_girada(r, espaco, 10.));
        // A proporção se mantém.
        assert!((r.w / r.h - 1.5).abs() < 0.01, "{r:?}");
        let de_volta = endireitar(&girado, 0., espaco, Some(desejado));
        assert!(
            (de_volta.crop_width() - 1.).abs() < 1e-3,
            "voltou à foto inteira"
        );
        assert_eq!(endireitar(&inteiro, 80., espaco, None).angle(), 45.);
    }
}
