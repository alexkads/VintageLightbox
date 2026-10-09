//! O Preenchimento sensível ao conteúdo do editor — a parte sem janela.
//!
//! O fluxo do Photoshop: a seleção é o **destino** (o que será refeito, com o
//! peso dela na borda), a **amostragem** é de onde os pedaços podem vir
//! (automática em volta do destino, ajustada com um pincel de incluir e
//! excluir), e o resultado é visto antes — numa prévia provisória reduzida e
//! depois no resultado final, na resolução da edição salva — e só então
//! confirmado, numa camada de retoque nova.
//!
//! 🔑 **Três coisas que não se confundem:** o destino e a amostragem viram as
//! duas máscaras do motor (`revelacao_core::preenchimento::Pedido`); o peso
//! de aplicação é o valor da seleção, usado só ao colar. Nenhuma delas é a
//! máscara de camada.
//!
//! A fonte é o instantâneo da composição até a camada escolhida, tirado ao
//! abrir: os retoques de antes contam, a prévia nunca (ela não está no
//! documento).

use editor_core::{Retangulo, Selecao};
use image::RgbImage;
use preenchimento::{Controle, Entrada, Erro, Metodo, Motor};

/// O maior lado do destino na prévia provisória, em pixels.
pub const LADO_DA_PREVIA: u32 = 160;

/// O teto da região de trabalho, em pixels (depois da redução): a síntese
/// guarda ~10 bytes por pixel, mais a pirâmide. 24 MP é uma foto de balcão
/// inteira.
pub const TETO_DA_REGIAO: u64 = 24_000_000;

/// Um remendo calculado. Com `fator` > 1 (a prévia provisória), `rgba` é
/// reduzido: cada pixel dele cobre `fator × fator` da foto.
#[derive(Clone, Debug, PartialEq)]
pub struct Resultado {
    /// Onde ele vai na foto (a caixa do destino), em pixels da foto.
    pub ret: Retangulo,
    pub largura: u32,
    pub altura: u32,
    pub rgba: Vec<u8>,
    pub fator: u32,
    /// A redução que o **motor** fez para caber na resolução dele (a LaMa,
    /// acima de 512 px) — 1 = a resolução da foto.
    pub reducao_do_motor: f32,
    pub executado_em: String,
}

/// A amostragem automática: a vizinhança do destino, com a margem que o
/// motor usa no caminho rápido (¾ do tamanho do destino, pelo menos 24 px).
/// O destino em si fica de fora na hora de calcular.
pub fn amostragem_automatica(destino: &Selecao) -> Selecao {
    let l = destino.caixa_justa();
    let margem =
        revelacao_core::preenchimento::margem_de_trabalho((l.x, l.y, l.direita(), l.baixo()), 0.0);
    destino.expandida(margem as i32)
}

/// "Opções da área de amostragem" do Photoshop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpcaoDeAmostragem {
    /// Em volta da área, pelo formato dela (a de sempre).
    #[default]
    Automatica,
    /// O retângulo em volta da área, com a mesma margem.
    Retangular,
    /// Começa vazia: o operador pinta de onde amostrar.
    Personalizada,
}

impl OpcaoDeAmostragem {
    pub const TODAS: [OpcaoDeAmostragem; 3] = [
        OpcaoDeAmostragem::Automatica,
        OpcaoDeAmostragem::Retangular,
        OpcaoDeAmostragem::Personalizada,
    ];

    pub fn chave(self) -> &'static str {
        match self {
            OpcaoDeAmostragem::Automatica => "automatica",
            OpcaoDeAmostragem::Retangular => "retangular",
            OpcaoDeAmostragem::Personalizada => "personalizada",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|o| o.chave() == chave)
    }

    pub fn nome(self) -> &'static str {
        match self {
            OpcaoDeAmostragem::Automatica => "Automática",
            OpcaoDeAmostragem::Retangular => "Retangular",
            OpcaoDeAmostragem::Personalizada => "Personalizada",
        }
    }
}

/// A amostragem de uma opção: a automática segue o formato da área; a
/// retangular é a caixa em volta dela com a mesma margem; a personalizada
/// começa vazia.
pub fn amostragem_de(opcao: OpcaoDeAmostragem, destino: &Selecao) -> Selecao {
    match opcao {
        OpcaoDeAmostragem::Automatica => amostragem_automatica(destino),
        OpcaoDeAmostragem::Personalizada => Selecao::vazia(destino.largura(), destino.altura()),
        OpcaoDeAmostragem::Retangular => {
            let l = destino.caixa_justa();
            let margem = revelacao_core::preenchimento::margem_de_trabalho(
                (l.x, l.y, l.direita(), l.baixo()),
                0.0,
            ) as i64;
            let (largura, altura) = (destino.largura() as i64, destino.altura() as i64);
            let (x0, y0) = ((l.x as i64 - margem).max(0), (l.y as i64 - margem).max(0));
            let (x1, y1) = (
                (l.direita() as i64 + margem).min(largura),
                (l.baixo() as i64 + margem).min(altura),
            );
            let caixa = Retangulo::novo(
                x0 as u32,
                y0 as u32,
                (x1 - x0).max(0) as u32,
                (y1 - y0).max(0) as u32,
            );
            Selecao::da_forma(
                destino.largura(),
                destino.altura(),
                &editor_core::Forma::Retangulo(caixa),
            )
        }
    }
}

/// Como a área de amostragem aparece no palco ("Sobreposição da área de
/// amostragem" do Photoshop): à vista ou não, a opacidade, a cor e se ela
/// pinta a área de onde se amostra ou a excluída.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sobreposicao {
    pub mostrar: bool,
    pub opacidade: f32,
    pub cor: [u8; 3],
    pub indica_excluida: bool,
}

impl Default for Sobreposicao {
    fn default() -> Self {
        Self {
            mostrar: true,
            opacidade: 0.5,
            cor: CORES_DA_SOBREPOSICAO[0].1,
            indica_excluida: false,
        }
    }
}

/// As cores da sobreposição: (nome, RGB).
pub const CORES_DA_SOBREPOSICAO: [(&str, [u8; 3]); 4] = [
    ("Verde", [60, 200, 80]),
    ("Vermelho", [230, 40, 40]),
    ("Azul", [40, 120, 230]),
    ("Amarelo", [240, 210, 40]),
];

/// O fator da prévia provisória: o destino com no máximo [`LADO_DA_PREVIA`].
pub fn fator_da_previa(destino: &Selecao) -> u32 {
    let l = destino.caixa_justa();
    l.largura.max(l.altura).div_ceil(LADO_DA_PREVIA).max(1)
}

/// O destino da síntese e o peso de aplicação, a partir da seleção e da
/// suavização da borda (`raio` px): o peso é a seleção difundida, e o destino
/// cobre toda a rampa dela — a mistura na borda é entre a foto e o
/// reconstruído, e não entre a foto e o objeto.
pub fn destino_e_peso(selecao: &Selecao, raio: u32) -> (Selecao, Selecao) {
    if raio == 0 {
        return (selecao.clone(), selecao.clone());
    }
    let peso = selecao.difusa(raio);
    (selecao.expandida(raio as i32 + 1), peso)
}

/// A região de trabalho: o que o motor precisa ver. Para quem amostra (o
/// PatchMatch), o destino e a amostragem; para a LaMa, o destino com a margem
/// de contexto de cada lado.
pub fn regiao_de_trabalho(
    destino: &Selecao,
    amostragem: &Selecao,
    metodo: Metodo,
    contexto: f32,
) -> Retangulo {
    let (l, a) = (destino.largura(), destino.altura());
    let caixa = destino.caixa_justa();
    let r = if metodo.capacidades().amostragem {
        caixa.uniao(&amostragem.caixa_justa())
    } else {
        let lado = caixa.largura.max(caixa.altura) as f32;
        let m = (lado * contexto.clamp(0.0, 3.0)).round() as u32 + 16;
        Retangulo::novo(
            caixa.x.saturating_sub(m),
            caixa.y.saturating_sub(m),
            caixa.largura + 2 * m,
            caixa.altura + 2 * m,
        )
    };
    Retangulo::novo(
        r.x.saturating_sub(4),
        r.y.saturating_sub(4),
        r.largura + 8,
        r.altura + 8,
    )
    .limitado(l, a)
}

/// Calcula o remendo com o motor, reduzido por `fator` (1 = a resolução
/// cheia). A foto é o instantâneo tirado ao abrir.
#[allow(clippy::too_many_arguments)]
pub fn calcular(
    foto: &RgbImage,
    destino: &Selecao,
    amostragem: &Selecao,
    motor: &dyn Motor,
    contexto: f32,
    adaptar_cor: bool,
    fator: u32,
    semente: u64,
    controle: &Controle,
) -> Result<Resultado, Erro> {
    let f = fator.max(1);
    let metodo = motor.metodo();
    let regiao = regiao_de_trabalho(destino, amostragem, metodo, contexto);
    if regiao.vazio() || destino.caixa_justa().vazio() {
        return Err(Erro::SemDestino);
    }
    let (w, h) = (regiao.largura.div_ceil(f), regiao.altura.div_ceil(f));
    if w as u64 * h as u64 > TETO_DA_REGIAO {
        return Err(Erro::Processamento(
            "a região é grande demais: reduza a área verde ou a seleção".into(),
        ));
    }
    let n = (w * h) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    let mut alvo = Vec::with_capacity(n);
    let mut fonte = Vec::with_capacity(n);
    let amostra = metodo.capacidades().amostragem;
    for y in 0..h {
        for x in 0..w {
            let (x0, y0) = (regiao.x + x * f, regiao.y + y * f);
            let (x1, y1) = ((x0 + f).min(regiao.direita()), (y0 + f).min(regiao.baixo()));
            let mut soma = [0u32; 3];
            let mut conta = 0u32;
            let mut no_destino = false;
            let mut toda_amostrada = true;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let p = foto.get_pixel(xx, yy).0;
                    for c in 0..3 {
                        soma[c] += p[c] as u32;
                    }
                    conta += 1;
                    no_destino |= destino.valor(xx, yy) > 0;
                    if amostra {
                        toda_amostrada &= amostragem.valor(xx, yy) >= 128;
                    }
                }
            }
            let conta = conta.max(1);
            rgba.extend_from_slice(&[
                ((soma[0] + conta / 2) / conta) as u8,
                ((soma[1] + conta / 2) / conta) as u8,
                ((soma[2] + conta / 2) / conta) as u8,
                255,
            ]);
            alvo.push(no_destino);
            fonte.push(toda_amostrada && !no_destino);
        }
    }
    let entrada = Entrada {
        rgba: &rgba,
        largura: w,
        altura: h,
        destino: &alvo,
        amostragem: amostra.then_some(fonte.as_slice()),
        semente,
        contexto,
        adaptar_cor,
    };
    let r = motor.preencher(&entrada, controle)?;
    let ret = Retangulo::novo(
        regiao.x + r.x0 * f,
        regiao.y + r.y0 * f,
        r.largura * f,
        r.altura * f,
    )
    .limitado(foto.width(), foto.height());
    Ok(Resultado {
        ret,
        largura: r.largura,
        altura: r.altura,
        rgba: r.rgba,
        fator: f,
        reducao_do_motor: r.reducao,
        executado_em: r.executado_em,
    })
}

/// A sobreposição do palco, em BGRA com alfa, de no máximo `lado` no maior
/// lado: vermelho onde será refeito, verde de onde os pedaços podem vir.
pub fn sobreposicao(destino: &Selecao, amostragem: &Selecao, lado: u32) -> (u32, u32, Vec<u8>) {
    sobreposicao_com(destino, Some(amostragem), &Sobreposicao::default(), lado)
}

/// A sobreposição com as opções do painel. A área a refazer fica sempre em
/// vermelho (é o que vai mudar); a amostragem segue a cor, a opacidade e o
/// "indica" (amostrada ou excluída) — e some com `mostrar` desligado ou sem
/// amostragem (a IA não amostra).
pub fn sobreposicao_com(
    destino: &Selecao,
    amostragem: Option<&Selecao>,
    opcoes: &Sobreposicao,
    lado: u32,
) -> (u32, u32, Vec<u8>) {
    let (l, a) = (destino.largura().max(1), destino.altura().max(1));
    let escala = (l.max(a) as f32 / lado as f32).max(1.0);
    let (w, h) = (
        ((l as f32 / escala).round() as u32).max(1),
        ((a as f32 / escala).round() as u32).max(1),
    );
    let alfa = (opcoes.opacidade.clamp(0.0, 1.0) * 160.0).round() as u8;
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let fy = ((y as f32 + 0.5) * a as f32 / h as f32) as u32;
        for x in 0..w {
            let fx = ((x as f32 + 0.5) * l as f32 / w as f32) as u32;
            let k = ((y * w + x) * 4) as usize;
            if destino.valor(fx, fy) > 0 {
                bgra[k..k + 4].copy_from_slice(&[40, 40, 230, 110]);
            } else if let Some(amostragem) = amostragem.filter(|_| opcoes.mostrar) {
                if (amostragem.valor(fx, fy) >= 128) != opcoes.indica_excluida {
                    let [r, g, b] = opcoes.cor;
                    bgra[k..k + 4].copy_from_slice(&[b, g, r, alfa]);
                }
            }
        }
    }
    (w, h, bgra)
}

/// A janela da Visualização (o painel do meio, como no Photoshop): o recorte
/// da foto em volta do destino — meia área de margem, pelo menos 32 px —, com
/// o resultado por cima no peso da aplicação quando há um. Em BGRA opaco, no
/// máximo `lado_maximo` de lado (amostra por vizinho mais próximo).
pub fn visualizacao(
    foto: &RgbImage,
    destino: &Selecao,
    peso: &Selecao,
    resultado: Option<&Resultado>,
    lado_maximo: u32,
) -> (u32, u32, Vec<u8>) {
    let (fl, fa) = foto.dimensions();
    let caixa = destino.caixa_justa();
    let recorte = if caixa.largura == 0 || caixa.altura == 0 {
        Retangulo::inteiro(fl, fa)
    } else {
        let margem = caixa.largura.max(caixa.altura) / 2 + 32;
        let x0 = caixa.x.saturating_sub(margem);
        let y0 = caixa.y.saturating_sub(margem);
        let x1 = (caixa.x + caixa.largura + margem).min(fl);
        let y1 = (caixa.y + caixa.altura + margem).min(fa);
        Retangulo::novo(x0, y0, x1 - x0, y1 - y0)
    };
    let passo = recorte
        .largura
        .max(recorte.altura)
        .div_ceil(lado_maximo.max(1))
        .max(1);
    let (w, h) = (
        recorte.largura.div_ceil(passo),
        recorte.altura.div_ceil(passo),
    );
    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for j in 0..h {
        for i in 0..w {
            let (x, y) = (recorte.x + i * passo, recorte.y + j * passo);
            let base = foto.get_pixel(x, y).0;
            let mut cor = [base[0] as f32, base[1] as f32, base[2] as f32];
            if let Some(r) = resultado {
                let dentro = x >= r.ret.x && y >= r.ret.y;
                let (rx, ry) = (
                    (x.wrapping_sub(r.ret.x)) / r.fator.max(1),
                    (y.wrapping_sub(r.ret.y)) / r.fator.max(1),
                );
                if dentro && rx < r.largura && ry < r.altura {
                    let k = ((ry * r.largura + rx) * 4) as usize;
                    let a = peso.valor(x, y) as f32 / 255.0;
                    for (c, v) in cor.iter_mut().zip(&r.rgba[k..k + 3]) {
                        *c += (*v as f32 - *c) * a;
                    }
                }
            }
            bgra.extend_from_slice(&[
                cor[2].round() as u8,
                cor[1].round() as u8,
                cor[0].round() as u8,
                255,
            ]);
        }
    }
    (w, h, bgra)
}

#[cfg(test)]
mod testes {
    use super::*;
    use editor_core::{Forma, Operacao};

    /// Um piso listrado com uma "mala" vermelha no meio.
    fn piso() -> RgbImage {
        RgbImage::from_fn(400, 300, |x, y| {
            if (180..230).contains(&x) && (120..170).contains(&y) {
                image::Rgb([220, 30, 30])
            } else {
                let v = if (x / 8) % 2 == 0 { 70 } else { 160 } + ((x * 7 + y * 3) % 9) as u8;
                image::Rgb([v, v - 10, v - 30])
            }
        })
    }

    fn mala() -> Selecao {
        let mut s = Selecao::da_forma(
            400,
            300,
            &Forma::Retangulo(Retangulo::novo(176, 116, 58, 58)),
        );
        s.combinar(&Selecao::vazia(400, 300), Operacao::Somar);
        s
    }

    #[test]
    fn a_visualizacao_recorta_em_volta_e_mostra_o_resultado_so_com_ele() {
        let foto = piso();
        let destino = mala();
        // Sem resultado: a mala à vista, num recorte com margem em volta dela.
        let (w, h, antes) = visualizacao(&foto, &destino, &destino, None, 4096);
        // 58 de lado + 2 × (29 + 32) de margem.
        assert_eq!((w, h), (180, 180));
        let vermelho = |b: &[u8]| b.chunks(4).filter(|p| p[2] > 200 && p[1] < 60).count();
        assert_eq!(vermelho(&antes), 50 * 50, "a mala inteira");
        // Com o resultado (todo cinza), nenhum vermelho sobra.
        let r = Resultado {
            ret: Retangulo::novo(176, 116, 58, 58),
            largura: 58,
            altura: 58,
            rgba: [90, 90, 90, 255].repeat(58 * 58),
            fator: 1,
            reducao_do_motor: 1.0,
            executado_em: String::new(),
        };
        let (_, _, depois) = visualizacao(&foto, &destino, &destino, Some(&r), 4096);
        assert_eq!(vermelho(&depois), 0);
        // E reduz para caber no lado pedido.
        let (w, h, _) = visualizacao(&foto, &destino, &destino, None, 60);
        assert!(w <= 60 && h <= 60, "{w}×{h}");
    }

    #[test]
    fn a_mala_sai_e_o_piso_continua_na_previa_e_no_final() {
        let foto = piso();
        let destino = mala();
        let amostragem = amostragem_automatica(&destino);
        let parado = std::sync::atomic::AtomicBool::new(false);
        let controle = Controle {
            cancelado: &parado,
            progresso: &|_| {},
        };
        let pm = preenchimento::patchmatch::PatchMatch;
        let final_ = calcular(
            &foto,
            &destino,
            &amostragem,
            &pm,
            0.5,
            true,
            1,
            9,
            &controle,
        )
        .unwrap();
        assert_eq!(final_.ret, Retangulo::novo(176, 116, 58, 58));
        assert_eq!(final_.fator, 1);
        let vermelhos = final_
            .rgba
            .chunks(4)
            .filter(|p| p[0] > 200 && p[1] < 60)
            .count();
        assert_eq!(vermelhos, 0, "nada da mala");
        // A prévia provisória, reduzida, cobre o mesmo lugar.
        let previa = calcular(
            &foto,
            &destino,
            &amostragem,
            &pm,
            0.5,
            true,
            4,
            9,
            &controle,
        )
        .unwrap();
        assert_eq!(previa.fator, 4);
        assert!(previa.ret.x <= 176 && previa.ret.direita() >= 234);
        assert!(previa.largura < final_.largura);
    }

    #[test]
    fn amostragem_vazia_pede_para_ampliar_e_o_destino_fora_nao_conta() {
        let foto = piso();
        let destino = mala();
        let nada = Selecao::vazia(400, 300);
        let parado = std::sync::atomic::AtomicBool::new(false);
        let controle = Controle {
            cancelado: &parado,
            progresso: &|_| {},
        };
        let pm = preenchimento::patchmatch::PatchMatch;
        let r = calcular(&foto, &destino, &nada, &pm, 0.5, true, 1, 9, &controle);
        assert_eq!(r, Err(Erro::SemFontes));
        assert!(Erro::SemFontes.mensagem().contains("verde"));
        // Amostragem só do lado esquerdo: o remendo usa só o de lá.
        let mut esquerda = Selecao::vazia(400, 300);
        esquerda.pintar_disco(60.0, 150.0, 60.0, true);
        let r = calcular(&foto, &destino, &esquerda, &pm, 0.5, true, 1, 9, &controle).unwrap();
        assert_eq!(r.ret, Retangulo::novo(176, 116, 58, 58));
        assert_eq!(
            regiao_de_trabalho(&destino, &esquerda, Metodo::PatchMatch, 0.5).x,
            0
        );
        // A LaMa não amostra: a região é o destino com a margem de contexto.
        let lama = regiao_de_trabalho(&destino, &esquerda, Metodo::LaMa, 0.5);
        assert_eq!(
            lama,
            Retangulo::novo(
                176 - 29 - 16 - 4,
                116 - 29 - 16 - 4,
                58 + 2 * 45 + 8,
                58 + 2 * 45 + 8
            )
        );
    }

    #[test]
    fn a_sobreposicao_mostra_destino_e_amostragem() {
        let destino = mala();
        let amostragem = amostragem_automatica(&destino);
        let (w, h, bgra) = sobreposicao(&destino, &amostragem, 200);
        assert_eq!((w, h), (200, 150));
        let em = |x: u32, y: u32| &bgra[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize];
        assert_eq!(em(100, 70)[2], 230, "vermelho no destino");
        assert_eq!(em(100, 40)[1], 200, "verde em volta");
        assert_eq!(em(2, 2)[3], 0, "longe, nada");
    }

    /// As três opções da área de amostragem do Photoshop.
    #[test]
    fn as_opcoes_da_amostragem() {
        let destino =
            Selecao::da_forma(400, 300, &Forma::Elipse(Retangulo::novo(150, 100, 100, 80)));
        let auto = amostragem_de(OpcaoDeAmostragem::Automatica, &destino);
        let ret = amostragem_de(OpcaoDeAmostragem::Retangular, &destino);
        let nada = amostragem_de(OpcaoDeAmostragem::Personalizada, &destino);
        assert!(nada.caixa_justa().vazio(), "a personalizada começa vazia");
        assert_eq!(ret.caixa_justa(), auto.caixa_justa(), "a mesma margem");
        // Num canto da caixa, a retangular amostra e a automática (que segue
        // o formato) não.
        let c = ret.caixa_justa();
        assert_eq!(ret.valor(c.x, c.y), 255);
        assert!(auto.valor(c.x, c.y) < 128);
    }

    /// A sobreposição segue as opções: cor, "indica a excluída" e esconder.
    #[test]
    fn a_sobreposicao_segue_as_opcoes() {
        let destino = mala();
        let amostragem = amostragem_automatica(&destino);
        let em = |b: &[u8], w: u32, x: u32, y: u32| b[((y * w + x) * 4) as usize..][..4].to_vec();
        let azul = Sobreposicao {
            cor: [40, 120, 230],
            ..Sobreposicao::default()
        };
        let (w, _, b) = sobreposicao_com(&destino, Some(&amostragem), &azul, 200);
        assert_eq!(
            em(&b, w, 100, 40)[..3],
            [230, 120, 40],
            "azul (BGRA) na amostragem"
        );
        let excluida = Sobreposicao {
            indica_excluida: true,
            ..Sobreposicao::default()
        };
        let (w, _, b) = sobreposicao_com(&destino, Some(&amostragem), &excluida, 200);
        assert_eq!(em(&b, w, 100, 40)[3], 0, "a amostrada fica limpa");
        assert!(em(&b, w, 2, 2)[3] > 0, "a excluída pintada");
        let escondida = Sobreposicao {
            mostrar: false,
            ..Sobreposicao::default()
        };
        let (w, _, b) = sobreposicao_com(&destino, Some(&amostragem), &escondida, 200);
        assert_eq!(em(&b, w, 100, 40)[3], 0);
        assert_eq!(em(&b, w, 100, 70)[2], 230, "a área a refazer continua");
    }

    #[test]
    fn o_fator_da_previa_limita_o_destino() {
        let grande = Selecao::da_forma(
            4000,
            3000,
            &Forma::Retangulo(Retangulo::novo(100, 100, 1600, 900)),
        );
        assert_eq!(fator_da_previa(&grande), 10);
        assert_eq!(fator_da_previa(&mala()), 1);
    }
}
