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
    let (l, a) = (destino.largura().max(1), destino.altura().max(1));
    let escala = (l.max(a) as f32 / lado as f32).max(1.0);
    let (w, h) = (
        ((l as f32 / escala).round() as u32).max(1),
        ((a as f32 / escala).round() as u32).max(1),
    );
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let fy = ((y as f32 + 0.5) * a as f32 / h as f32) as u32;
        for x in 0..w {
            let fx = ((x as f32 + 0.5) * l as f32 / w as f32) as u32;
            let k = ((y * w + x) * 4) as usize;
            if destino.valor(fx, fy) > 0 {
                bgra[k..k + 4].copy_from_slice(&[40, 40, 230, 110]);
            } else if amostragem.valor(fx, fy) >= 128 {
                bgra[k..k + 4].copy_from_slice(&[80, 200, 60, 70]);
            }
        }
    }
    (w, h, bgra)
}

/// A prévia, em BGRA com o peso de aplicação (o valor da seleção no meio de
/// cada pixel) como alfa — o que a camada vai mostrar.
pub fn previa_bgra(r: &Resultado, destino: &Selecao) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(r.rgba.len());
    for y in 0..r.altura {
        for x in 0..r.largura {
            let k = ((y * r.largura + x) * 4) as usize;
            let (fx, fy) = (
                r.ret.x + x * r.fator + r.fator / 2,
                r.ret.y + y * r.fator + r.fator / 2,
            );
            let peso = destino.valor(fx, fy);
            bgra.extend_from_slice(&[r.rgba[k + 2], r.rgba[k + 1], r.rgba[k], peso]);
        }
    }
    bgra
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
        let final_ = calcular(&foto, &destino, &amostragem, &pm, 0.5, 1, 9, &controle).unwrap();
        assert_eq!(final_.ret, Retangulo::novo(176, 116, 58, 58));
        assert_eq!(final_.fator, 1);
        let vermelhos = final_
            .rgba
            .chunks(4)
            .filter(|p| p[0] > 200 && p[1] < 60)
            .count();
        assert_eq!(vermelhos, 0, "nada da mala");
        // A prévia provisória, reduzida, cobre o mesmo lugar.
        let previa = calcular(&foto, &destino, &amostragem, &pm, 0.5, 4, 9, &controle).unwrap();
        assert_eq!(previa.fator, 4);
        assert!(previa.ret.x <= 176 && previa.ret.direita() >= 234);
        assert!(previa.largura < final_.largura);
        assert_eq!(previa_bgra(&final_, &destino).len(), final_.rgba.len());
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
        let r = calcular(&foto, &destino, &nada, &pm, 0.5, 1, 9, &controle);
        assert_eq!(r, Err(Erro::SemFontes));
        assert!(Erro::SemFontes.mensagem().contains("verde"));
        // Amostragem só do lado esquerdo: o remendo usa só o de lá.
        let mut esquerda = Selecao::vazia(400, 300);
        esquerda.pintar_disco(60.0, 150.0, 60.0, true);
        let r = calcular(&foto, &destino, &esquerda, &pm, 0.5, 1, 9, &controle).unwrap();
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
