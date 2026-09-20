//! A ponte entre o `image` e o GPUI.
//!
//! É a fronteira técnica nº 1 da migração (docs/10-MIGRACAO-GPUI.md §3.1), e a
//! única sem a qual a Biblioteca não mostra miniatura nenhuma.
//!
//! O caminho fica **mais curto** que o de hoje:
//!
//! ```text
//! egui:  DynamicImage → ColorImage → egui::TextureHandle
//! gpui:  DynamicImage → RgbaImage → Frame → RenderImage
//! ```
//!
//! Some a conversão para `ColorImage`, que era cópia pura de bytes.

use std::sync::Arc;

use gpui::{px, size, DevicePixels, Pixels, RenderImage, Size};
// `Frame` vem do **crate `image`**, e não do `gpui`: lá ele é reexportado de
// forma privada. É a mesma peça que a `infrastructure` já usa, e é por isso que
// as duas versões do `image` precisam ser a mesma na árvore (§3.2).
use image::{DynamicImage, Frame};
use smallvec::SmallVec;

/// Converte a imagem decodificada no que o GPUI desenha.
///
/// # 🚨 O GPUI quer BGRA, e o crate `image` produz RGBA
///
/// `RenderImage` é documentado como *"a cached and processed image, in BGRA
/// format"*, e o renderizador Metal cria as texturas com `BGRA8Unorm`. O
/// `image::Frame`, por outro lado, carrega um `RgbaImage` — o tipo **não** diz
/// qual das duas ordens está lá dentro.
///
/// Entregar RGBA a quem espera BGRA não falha, não avisa e não quebra: troca
/// vermelho por azul em **toda** foto. Num programa de revelação, esse é o pior
/// desfecho possível — a imagem aparece, parece uma decisão de cor, e quem
/// olha conclui que o motor de cor está errado.
///
/// A troca aqui é a mesma que o próprio GPUI faz ao carregar imagem da área de
/// transferência (`platform.rs`, `frames_for_image`): `into_rgba8()` e depois
/// `swap(0, 2)` em cada pixel. Copiar a regra de quem já fala com essa API é
/// mais barato do que descobri-la olhando a foto ficar azul.
///
/// # Uma passada, e não duas
///
/// 🚨 **O que chega do cache é Rgb8, e não Rgba8** — JPEG não tem canal alfa, e
/// tanto a miniatura quanto o preview grande são JPEG. A versão anterior dizia
/// o contrário e contava com `into_rgba8()` reaproveitar o buffer; na prática
/// ele **alocava 17,5 MB e expandia** os 13 MB de RGB, e só então uma segunda
/// varredura trocava R por B. Duas passadas completas sobre a imagem, uma vez
/// por miniatura da grade e **uma vez por quadro** enquanto um slider é
/// arrastado — 4,32 ms de um orçamento de 16,7 ms, medido em 8/set/2026
/// (`medir-revelacao`).
///
/// Expandir e trocar são a mesma passada: lê três bytes, escreve quatro na
/// ordem que o Metal quer. O caminho de baixo continua existindo para o que
/// realmente chega em RGBA (PNG com alfa, imagem sintética dos testes).
pub fn para_gpui(imagem: DynamicImage) -> Arc<RenderImage> {
    let bytes = match imagem {
        DynamicImage::ImageRgb8(rgb) => bgra_de_rgb8(rgb),
        outra => {
            let mut bytes = outra.into_rgba8();
            // `as_chunks_mut::<4>()` e não um laço por (x, y): o buffer é
            // contíguo, e o acesso por coordenada refaria a multiplicação a
            // cada pixel.
            for pixel in bytes.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            bytes
        }
    };

    Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(bytes), 1)))
}

/// O tamanho com que a foto **cabe inteira** numa moldura — o `object-fit:
/// contain` da web, feito na mão.
///
/// # 🚨 Por que não `ObjectFit::Contain`
///
/// O `Img` do GPUI escreve `style.aspect_ratio` com a proporção da imagem em
/// **todo** `request_layout` (`elements/img.rs`). Com `size_full()` os dois
/// lados pedem 100%, o taffy tira a altura da largura e o elemento fica maior
/// que a moldura; o `Contain` então cabe direitinho **nesse** retângulo, que
/// já está fora da tela. Foi o que cortou a tela do cliente (dono,
/// 17/set/2026): uma foto em pé de 747×1370 numa janela de 1920×1080 recebia
/// 1920×3522 e aparecia ampliada 2,6×.
///
/// Onde a moldura tem tamanho conhecido, `max_w_*`/`max_h_*` já resolvem — o
/// máximo limita os dois lados e a proporção do `Img` faz o resto. Esta conta
/// é para quem precisa do número: uma animação de escala, uma moldura própria.
pub fn cabe_em(moldura: Size<Pixels>, imagem: Size<DevicePixels>) -> Size<Pixels> {
    escalar(moldura, imagem, |largura, altura| largura.min(altura))
}

/// O tamanho com que a foto **cobre** uma moldura — o `object-fit: cover`.
///
/// Sobra foto para fora dos dois lados, e quem corta é o `overflow_hidden` da
/// moldura. Mesma razão de [`cabe_em`] para existir.
pub fn cobre(moldura: Size<Pixels>, imagem: Size<DevicePixels>) -> Size<Pixels> {
    escalar(moldura, imagem, |largura, altura| largura.max(altura))
}

/// O que as duas têm em comum: a escala que cada lado pediria, e a escolha
/// entre elas.
///
/// ⚠️ **Moldura ainda sem medida** (o primeiro quadro de uma janela) devolve a
/// foto no tamanho dela: some é pior que grande demais, e o quadro seguinte já
/// traz a medida.
fn escalar(
    moldura: Size<Pixels>,
    imagem: Size<DevicePixels>,
    escolher: fn(f32, f32) -> f32,
) -> Size<Pixels> {
    let largura = u32::from(imagem.width).max(1) as f32;
    let altura = u32::from(imagem.height).max(1) as f32;
    let escala = escolher(
        f32::from(moldura.width) / largura,
        f32::from(moldura.height) / altura,
    );
    let escala = if escala.is_finite() && escala > 0.0 {
        escala
    } else {
        1.0
    };
    size(px(largura * escala), px(altura * escala))
}

/// RGB de três bytes vira BGRA de quatro, numa passada só.
///
/// `chunks_exact(3)` deixa o compilador saber o tamanho do passo; o alfa é
/// opaco porque a origem não tem transparência para preservar — é a mesma
/// decisão que o cache já toma ao gravar em JPEG.
fn bgra_de_rgb8(rgb: image::RgbImage) -> image::RgbaImage {
    let (largura, altura) = rgb.dimensions();
    let origem = rgb.into_raw();

    // 🔑 **Buffer do tamanho final, e escrita por fatia.** `Vec::push` /
    // `extend_from_slice` conferem capacidade a cada pixel e impedem o
    // compilador de vetorizar; com o destino já dimensionado e o alfa já opaco,
    // o laço vira três escritas em posições conhecidas — e o `zip` de dois
    // `as_chunks` dá ao compilador os dois passos como constantes.
    //
    // ⚠️ **`as_chunks`, e não `chunks_exact`**: com o tamanho do passo escrito
    // no tipo (`[u8; 4]`, `[u8; 3]`), o índice é conferido na compilação e não
    // a cada pixel. É o mesmo par de linhas da função acima.
    let mut destino = vec![255u8; origem.len() / 3 * 4];
    for (saida, pixel) in destino
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(origem.as_chunks::<3>().0)
    {
        saida[0] = pixel[2];
        saida[1] = pixel[1];
        saida[2] = pixel[0];
    }

    image::RgbaImage::from_raw(largura, altura, destino)
        .expect("o tamanho do destino vem do da origem")
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{Rgba, RgbaImage};

    /// Uma imagem 1×1 da cor pedida, em RGBA.
    fn um_pixel(r: u8, g: u8, b: u8, a: u8) -> DynamicImage {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([r, g, b, a]));
        DynamicImage::ImageRgba8(img)
    }

    /// Os bytes que o GPUI vai mandar para a GPU.
    fn bytes_de(imagem: DynamicImage) -> Vec<u8> {
        let render = para_gpui(imagem);
        render
            .as_bytes(0)
            .expect("o quadro 0 tem de existir")
            .to_vec()
    }

    #[test]
    fn vermelho_puro_chega_como_bgra() {
        // Se este teste falhar com [255, 0, 0, 255], a troca sumiu — e é o
        // defeito que faz toda foto do acervo ficar azul.
        let bytes = bytes_de(um_pixel(255, 0, 0, 255));

        assert_eq!(
            bytes,
            vec![0, 0, 255, 255],
            "vermelho em RGBA vira B=0 G=0 R=255 em BGRA"
        );
    }

    #[test]
    fn azul_puro_chega_como_bgra() {
        let bytes = bytes_de(um_pixel(0, 0, 255, 255));

        assert_eq!(bytes, vec![255, 0, 0, 255]);
    }

    #[test]
    fn verde_e_alfa_nao_sao_tocados() {
        // A troca é só entre os canais 0 e 2. Verde no meio e alfa no fim
        // ficam onde estão — trocar quatro canais em vez de dois é o erro
        // gêmeo, e ele escurece a imagem em vez de mudar a cor.
        let bytes = bytes_de(um_pixel(10, 200, 30, 128));

        assert_eq!(bytes, vec![30, 200, 10, 128]);
    }

    #[test]
    fn imagem_rgb_sem_alfa_ganha_alfa_opaco() {
        // JPEG não tem alfa, e é o formato de toda miniatura do cache. O
        // `into_rgba8` preenche com 255 — se preenchesse com 0, a grade
        // inteira ficaria invisível.
        let rgb =
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0])));

        assert_eq!(bytes_de(rgb), vec![0, 0, 255, 255]);
    }

    #[test]
    fn o_tamanho_sobrevive_a_conversao() {
        let render = para_gpui(DynamicImage::ImageRgba8(RgbaImage::new(320, 240)));
        let tamanho = render.size(0);

        assert_eq!(u32::from(tamanho.width), 320);
        assert_eq!(u32::from(tamanho.height), 240);
    }

    /// 🔑 A foto em pé cabe pela **altura**, e sobra tarja dos lados — que é o
    /// que a tela do cliente mostra desde que a conta passou a ser esta.
    #[test]
    fn a_foto_em_pe_cabe_pela_altura() {
        let cabe = cabe_em(size(px(1920.), px(1080.)), moldura_de(747, 1370));

        assert_eq!(f32::from(cabe.height).round(), 1080.0);
        assert_eq!(f32::from(cabe.width).round(), 589.0);
    }

    /// A deitada de 3:2 numa janela 16:9 cabe pela **altura** também — e é o
    /// que deixa tarja dos lados em vez de cortar as bonecas do rodapé da foto,
    /// que era o desfecho antigo.
    #[test]
    fn a_foto_deitada_de_3_por_2_cabe_pela_altura_numa_janela_16_por_9() {
        let cabe = cabe_em(size(px(1920.), px(1080.)), moldura_de(2048, 1365));

        assert_eq!(f32::from(cabe.height).round(), 1080.0);
        assert_eq!(f32::from(cabe.width).round(), 1620.0);
    }

    /// Mais larga que a moldura: aí sim cabe pela largura.
    #[test]
    fn a_foto_panoramica_cabe_pela_largura() {
        let cabe = cabe_em(size(px(1000.), px(1000.)), moldura_de(2000, 500));

        assert_eq!(f32::from(cabe.width).round(), 1000.0);
        assert_eq!(f32::from(cabe.height).round(), 250.0);
    }

    /// `cobre` é o contrário: o lado que sobra passa da moldura, e quem corta é
    /// o `overflow_hidden`.
    #[test]
    fn cobrir_estoura_o_lado_que_sobra() {
        let cobre = cobre(size(px(72.), px(72.)), moldura_de(3000, 2000));

        assert_eq!(f32::from(cobre.height).round(), 72.0);
        assert_eq!(f32::from(cobre.width).round(), 108.0);
    }

    /// ⚠️ Moldura de tamanho zero (primeiro quadro) não some com a foto.
    #[test]
    fn sem_moldura_medida_a_foto_fica_no_tamanho_dela() {
        let cabe = cabe_em(size(px(0.), px(0.)), moldura_de(100, 50));

        assert_eq!(f32::from(cabe.width), 100.0);
        assert_eq!(f32::from(cabe.height), 50.0);
    }

    /// O tamanho de uma imagem, como o `RenderImage` o devolve.
    fn moldura_de(largura: u32, altura: u32) -> Size<DevicePixels> {
        para_gpui(DynamicImage::ImageRgba8(RgbaImage::new(largura, altura))).size(0)
    }

    #[test]
    fn cada_imagem_tem_identidade_propria() {
        // O `RenderImage` compara por `id`, e é por ele que o GPUI decide se
        // pode reaproveitar a textura já enviada à GPU. Dois ids iguais para
        // fotos diferentes mostrariam a foto errada na grade.
        let a = para_gpui(um_pixel(1, 2, 3, 255));
        let b = para_gpui(um_pixel(1, 2, 3, 255));

        assert_ne!(a.id, b.id);
    }
}

/// 🚨 **A armadilha do `size_full()` no `img`, vigiada no código-fonte.**
///
/// Ela já cortou a foto três vezes — a tela do cliente e a grade da sessão
/// (17 e 18/set/2026) e as duas tiras (18/set/2026, *"as fotos no modo retrato
/// está ficando cortada no filmstrip"*) —, e some do olho porque **só o
/// retrato aparece errado**: numa moldura paisagem a foto deitada cabe por
/// acaso, e a em pé estoura para fora e o `overflow_hidden` corta.
///
/// Nenhum teste de comportamento a pega: o layout errado só existe dentro do
/// taffy, com uma janela no ar e uma foto em pé carregada. O que sobra é olhar
/// para o próprio código — e é o que este teste faz.
///
/// A regra: `img(...).size_full()` só vale quando **cortar é o pedido**
/// (`ObjectFit::Cover`), quando esticar é (`Fill`), ou quando a moldura já
/// nasce com a proporção da foto (a lista abaixo, com o porquê de cada uma).
/// Em todo o resto é `max_w_full()`/`max_h_full()`, que limitam os dois lados e
/// deixam a proporção do `Img` fazer o resto.
#[cfg(test)]
mod a_moldura_manda {
    /// Onde `size_full()` é legítimo, e por quê.
    ///
    /// Sair daqui pede uma linha nova **com o motivo** — e o motivo tem de ser
    /// "a moldura já tem a proporção da foto". "Ficou bom na minha tela" é como
    /// a armadilha voltou das outras vezes.
    const COM_MOTIVO: &[(&str, &str)] = &[
        (
            "cliente.rs",
            "a moldura é `cabe_em(janela, foto)` — já tem a proporção da foto",
        ),
        (
            "impressao/tela.rs",
            "a moldura é o retângulo que `encaixar` calculou para o papel",
        ),
        (
            "backup/tela.rs",
            "a prévia mede a moldura com `cabe_em(janela, foto) * zoom` — ela já \
             tem a proporção da foto, como em `cliente.rs`",
        ),
    ];

    /// Tira os comentários, para que o texto que **explica** a armadilha não
    /// seja confundido com ela.
    fn sem_comentarios(fonte: &str) -> String {
        fonte
            .lines()
            .map(|linha| match linha.find("//") {
                Some(i) => &linha[..i],
                None => linha,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Os `.rs` de `ui-gpui`, com o caminho relativo a `src/`.
    fn fontes() -> Vec<(String, String)> {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut achados = Vec::new();
        let mut pilha = vec![raiz.clone()];
        while let Some(pasta) = pilha.pop() {
            for entrada in std::fs::read_dir(&pasta)
                .expect("a pasta src existe")
                .flatten()
            {
                let caminho = entrada.path();
                if caminho.is_dir() {
                    pilha.push(caminho);
                } else if caminho.extension().is_some_and(|e| e == "rs") {
                    let nome = caminho
                        .strip_prefix(&raiz)
                        .expect("veio de dentro de src")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let texto = std::fs::read_to_string(&caminho).expect("o arquivo se lê");
                    achados.push((nome, sem_comentarios(&texto)));
                }
            }
        }
        achados
    }

    /// A cadeia de métodos que sai de **um** `img(...)`, em texto.
    ///
    /// ⚠️ **Contando parênteses, e não cortando por tamanho.** Um `.child(...)`
    /// ou um `.when(_, |i| ...)` no meio da cadeia carrega código de outros
    /// elementos dentro; procurar `size_full()` por perto acusaria o `div()`
    /// vizinho. Aqui só conta o que está no **nível da cadeia**: o argumento
    /// de cada método é pulado inteiro.
    ///
    /// Devolve `(usa_size_full, cadeia)` — a cadeia inteira ainda serve para
    /// ver se havia um `object_fit` deliberado.
    fn cadeia_do_img(fonte: &str, inicio: usize) -> (bool, String) {
        let bytes = fonte.as_bytes();
        let mut i = inicio;
        // 1. Pular o argumento do próprio `img(`.
        let mut profundidade = 1usize;
        while i < bytes.len() && profundidade > 0 {
            match bytes[i] {
                b'(' => profundidade += 1,
                b')' => profundidade -= 1,
                _ => {}
            }
            i += 1;
        }
        // 2. Andar pelos `.metodo(...)` seguintes, pulando cada argumento.
        let mut usa_size_full = false;
        let cadeia_de = i;
        loop {
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] != b'.' {
                break;
            }
            let nome_de = i + 1;
            let mut fim = nome_de;
            while fim < bytes.len() && (bytes[fim].is_ascii_alphanumeric() || bytes[fim] == b'_') {
                fim += 1;
            }
            if fim == nome_de || fim >= bytes.len() || bytes[fim] != b'(' {
                break;
            }
            if &fonte[nome_de..fim] == "size_full" {
                usa_size_full = true;
            }
            i = fim + 1;
            let mut profundidade = 1usize;
            while i < bytes.len() && profundidade > 0 {
                match bytes[i] {
                    b'(' => profundidade += 1,
                    b')' => profundidade -= 1,
                    _ => {}
                }
                i += 1;
            }
        }
        (
            usa_size_full,
            fonte[cadeia_de..i.min(fonte.len())].to_string(),
        )
    }

    #[test]
    fn nenhum_img_novo_usa_size_full() {
        let mut culpados = Vec::new();
        for (arquivo, fonte) in fontes() {
            // Este arquivo é conta, e não elemento: o único `img(` aqui é o
            // texto que o próprio teste procura.
            if arquivo == "imagem.rs" || COM_MOTIVO.iter().any(|(qual, _)| *qual == arquivo) {
                continue;
            }
            for (i, _) in fonte.match_indices("img(") {
                // `img(` precedido de letra é outra coisa (`self.img(`, um
                // nome que termina em "img"), e não o construtor do GPUI.
                if i > 0 && {
                    let anterior = fonte.as_bytes()[i - 1];
                    anterior.is_ascii_alphanumeric() || anterior == b'_'
                } {
                    continue;
                }
                let (usa_size_full, cadeia) = cadeia_do_img(&fonte, i + 4);
                if !usa_size_full {
                    continue;
                }
                let deliberado = cadeia.contains("ObjectFit::Cover")
                    || cadeia.contains("ObjectFit::Fill")
                    || cadeia.contains("ObjectFit::None");
                if !deliberado {
                    culpados.push(arquivo.clone());
                }
            }
        }
        culpados.sort();
        culpados.dedup();
        assert!(
            culpados.is_empty(),
            "`img(...).size_full()` sem `Cover`/`Fill` em {culpados:?}: o `Img` do GPUI impõe a \
             proporção da foto no layout, o elemento passa da moldura e o retrato aparece \
             cortado. Use `max_w_full()`/`max_h_full()` — ou, se a moldura já tiver a proporção \
             da foto, acrescente o arquivo a `COM_MOTIVO` com o motivo."
        );
    }

    /// A lista de exceções não pode virar depósito: cada uma some no dia em que
    /// o arquivo deixar de usar `size_full()`.
    #[test]
    fn a_lista_de_excecoes_nao_guarda_nome_morto() {
        let fontes = fontes();
        for (arquivo, motivo) in COM_MOTIVO {
            let ainda_usa = fontes
                .iter()
                .any(|(nome, fonte)| nome == arquivo && fonte.contains(".size_full()"));
            assert!(
                ainda_usa,
                "{arquivo} está em COM_MOTIVO ({motivo}) e não usa mais `size_full()` — tire-o da lista."
            );
        }
    }
}

/// 🚨 **A armadilha, medida.** O teste acima vigia o código-fonte; este mede o
/// layout de verdade, numa janela de teste, e diz **por que** a regra existe.
///
/// Se um dia o GPUI deixar de impor a proporção da foto, é aqui que se descobre
/// — `o_size_full_estoura_a_moldura` passa a falhar, e aí o contorno pode cair.
#[cfg(test)]
mod a_moldura_medida {
    use std::sync::{Arc, Mutex};

    use gpui::{
        div, img, prelude::*, px, AnyElement, App, Bounds, Context, ObjectFit, Pixels, Render,
        RenderImage, TestAppContext, Window,
    };
    use image::{DynamicImage, RgbaImage};

    use super::para_gpui;
    use crate::sessoes::altura_da_tira;

    /// A miniatura da tira, com os números dela — **paisagem**, sempre
    /// (`altura_da_tira::PROPORCAO`). É a moldura que esconde o defeito.
    fn moldura() -> (f32, f32) {
        let lado = altura_da_tira::lado_da_miniatura(altura_da_tira::ALTURA_PADRAO);
        ((lado * altura_da_tira::PROPORCAO).round(), lado)
    }

    /// Como a foto é pendurada na moldura.
    #[derive(Clone, Copy)]
    enum Forma {
        /// O que estava nas duas tiras até 18/set/2026.
        SizeFull,
        /// O que está lá agora.
        MaxFull,
    }

    struct Moldura {
        imagem: Arc<RenderImage>,
        forma: Forma,
        medida: Arc<Mutex<Option<Bounds<Pixels>>>>,
    }

    impl Render for Moldura {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let medida = self.medida.clone();
            let imagem = self.imagem.clone();
            let dentro: AnyElement = match self.forma {
                Forma::SizeFull => img(imagem)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                Forma::MaxFull => img(imagem)
                    .max_w_full()
                    .max_h_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
            };
            let (largura, altura) = moldura();
            let moldura = div()
                .w(px(largura))
                .h(px(altura))
                .overflow_hidden()
                .on_children_prepainted(move |limites: Vec<Bounds<Pixels>>, _w, _cx: &mut App| {
                    *medida.lock().expect("o cadeado do teste") = limites.first().copied();
                })
                .child(dentro);
            // A moldura fica centrada, como na faixa da tira.
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(match self.forma {
                    Forma::SizeFull => moldura,
                    Forma::MaxFull => moldura.flex().items_center().justify_center(),
                })
        }
    }

    /// Desenha uma foto de `largura`×`altura` na moldura e devolve o retângulo
    /// que o elemento da foto ocupou.
    fn medir(cx: &mut TestAppContext, forma: Forma, largura: u32, altura: u32) -> Bounds<Pixels> {
        let medida = Arc::new(Mutex::new(None));
        let imagem = para_gpui(DynamicImage::ImageRgba8(RgbaImage::new(largura, altura)));
        let guardada = medida.clone();
        let janela = cx.add_window(move |_window, _cx| Moldura {
            imagem,
            forma,
            medida: guardada,
        });
        gpui::VisualTestContext::from_window(janela.into(), cx).run_until_parked();
        let lida = *medida.lock().expect("o cadeado do teste");
        lida.expect("a moldura desenhou a foto")
    }

    /// 🚨 **O defeito, reproduzido.** Com os dois lados em 100%, o `Img` do GPUI
    /// impõe a proporção da foto e o elemento fica **mais alto que a moldura** —
    /// e o `overflow_hidden` da tira transforma isso em corte.
    ///
    /// ⚠️ Este teste afirma o **errado de propósito**: é ele que prova que o
    /// contorno ainda é necessário. No dia em que falhar, o GPUI mudou e
    /// `max_w_full`/`max_h_full` podem voltar a ser `size_full`.
    #[gpui::test]
    fn o_size_full_estoura_a_moldura(cx: &mut TestAppContext) {
        let (_, altura_da_moldura) = moldura();
        let retrato = medir(cx, Forma::SizeFull, 240, 320);
        let excesso = f32::from(retrato.size.height) / altura_da_moldura;

        assert!(
            excesso > 1.5,
            "a foto em pé deveria estourar muito a moldura de altura {altura_da_moldura} com \
             size_full, e mediu {:?} ({excesso:.2}×)",
            retrato.size
        );
    }

    /// 🔑 **Por que ninguém via: a foto deitada quase não sofre.**
    ///
    /// Com o mesmo layout errado, a paisagem passa da moldura em **menos de
    /// 5%** — um fio de nada, invisível a olho nu — enquanto a em pé passa de
    /// 80%. O defeito não se escondia por sorte: ele é proporcional à diferença
    /// entre a proporção da foto e a da moldura, e o catálogo de medição só
    /// tinha fotos na proporção da moldura. É por isso que `semear-catalogo`
    /// passou a gerar uma em cada três em pé.
    #[gpui::test]
    fn a_paisagem_quase_nao_sofre_e_por_isso_ninguem_via(cx: &mut TestAppContext) {
        let (_, altura_da_moldura) = moldura();
        let paisagem = medir(cx, Forma::SizeFull, 320, 240);
        let retrato = medir(cx, Forma::SizeFull, 240, 320);

        let excesso_deitada = f32::from(paisagem.size.height) / altura_da_moldura;
        let excesso_em_pe = f32::from(retrato.size.height) / altura_da_moldura;

        assert!(
            excesso_deitada < 1.05,
            "a deitada deveria passar de raspão, e passou {excesso_deitada:.2}×"
        );
        // O que se compara é o **excesso**, e não o tamanho: 1,81× contra 1,02×
        // é 81% de sobra contra 2% — quarenta vezes mais, e não o dobro.
        assert!(
            excesso_em_pe - 1.0 > (excesso_deitada - 1.0) * 10.0,
            "a em pé ({excesso_em_pe:.2}×) tem de sobrar muito mais que a deitada \
             ({excesso_deitada:.2}×) — é essa diferença que escondia o defeito"
        );
    }

    /// ✅ **A correção.** Com `max_w_full`/`max_h_full` o elemento nunca passa
    /// da moldura — nem em pé, nem deitado.
    #[gpui::test]
    fn com_max_full_nada_passa_da_moldura(cx: &mut TestAppContext) {
        for (largura, altura, qual) in [
            (240, 320, "em pé"),
            (320, 240, "deitada"),
            (400, 400, "quadrada"),
        ] {
            let dentro = medir(cx, Forma::MaxFull, largura, altura);

            let (largura_da_moldura, altura_da_moldura) = moldura();
            assert!(
                f32::from(dentro.size.width) <= largura_da_moldura + 1.0
                    && f32::from(dentro.size.height) <= altura_da_moldura + 1.0,
                "a foto {qual} ({largura}×{altura}) passou da moldura de \
                 {largura_da_moldura}×{altura_da_moldura}: {:?}",
                dentro.size
            );
        }
    }
}
