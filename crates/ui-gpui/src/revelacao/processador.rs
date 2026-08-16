//! O motor de revelação: wgpu, numa thread só dele.
//!
//! É a peça que o plano prometeu que atravessaria inteira
//! (docs/10-MIGRACAO-GPUI.md §2.2), e a promessa se confirmou ao abrir o
//! arquivo: `gpu_processor.rs` cria a **própria** `wgpu::Instance` e o próprio
//! dispositivo, numa thread de fundo. Ele nunca soube que existia eframe. A
//! única referência a egui nas 578 linhas era o **tipo de saída**.
//!
//! Aqui esse tipo sumiu: o resultado carrega um `DynamicImage` e quem desenha
//! converte com [`crate::imagem::para_gpui`]. Some uma cópia de bytes por
//! quadro — a mesma economia que a ponte de miniatura já tinha dado.
//!
//! ## Por que continua sendo thread + canal
//!
//! Não é herança: é o que a tela precisa. Arrastar um slider gera dezenas de
//! pedidos por segundo, e cada um custa upload de textura, dispatch e leitura de
//! volta. Fazer isso no `render` congelaria a janela no arrasto — que é
//! exatamente o momento em que ela precisa responder.
//!
//! O descarte de pedido velho (`id < atual`) é a outra metade: sem ele, soltar o
//! slider deixaria uma fila de quadros intermediários para desenhar, e a imagem
//! chegaria ao valor final segundos depois do dedo.
//!
//! ## 🚨 O que **não** veio junto: o caminho de CPU
//!
//! O original cai para `ImageProcessor::process_image` quando não há adaptador —
//! uma **segunda implementação** da mesma matemática, com resultado diferente do
//! shader. Numa migração cujo critério é igualdade de pixel, carregar isso seria
//! carregar o defeito para dentro da régua.
//!
//! Sem adaptador, aqui, [`Processador::disponivel`] responde `false` e a
//! Revelação mostra a foto sem ajuste — o que é honesto e visível, em vez de
//! silenciosamente certo-por-outro-caminho. O produto declara macOS e Windows
//! ([README](../../../../README.md)), onde adaptador sempre existe.

use std::num::NonZeroUsize;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use image::DynamicImage;
use lru::LruCache;
use parking_lot::Mutex;

/// Os ajustes, no layout que o WGSL espera.
///
/// 🚨 **Os nomes e a ordem são os do shader, e ficam em inglês de propósito.**
/// `repr(C)` + `bytemuck` mandam esta struct para a GPU como bytes crus, campo a
/// campo, por **posição**. Um campo fora de lugar aqui não é erro de compilação,
/// é a foto saindo com o ajuste errado aplicado — e poder ler os dois lados um ao
/// lado do outro é a única defesa que existe.
///
/// 🚨 **E ela já falhou: o `uniform` do outro lado declara 28 campos, não 46.**
/// Do campo 23 em diante o shader lê o do vizinho (o matiz do vermelho vira
/// redução de ruído) e do 28 em diante não lê nada — os 4 controles de Detalhe e
/// os 3 de Lente não fazem efeito nenhum. É defeito herdado do `crates/ui`, que
/// manda a mesma struct para o mesmo shader; está preso em
/// `o_wgsl_declara_28_campos_para_os_46_que_o_rust_manda`, com a tabela inteira,
/// e registrado em `docs/10-MIGRACAO-GPUI.md`.
///
/// Os 46 campos ficam aqui assim mesmo: encolher a struct para 28 mudaria o que
/// a GPU recebe, e a fase 2 se mede por igualdade de pixel com o app de egui.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Ajustes {
    pub exposure: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub clarity: f32,
    pub vibrance: f32,
    pub saturation: f32,
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    pub nr_luminance: f32,
    pub nr_color: f32,
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
}

impl Default for Ajustes {
    /// O neutro, conferido campo a campo contra o `crates/ui`.
    ///
    /// ⚠️ **Nem todo neutro é zero**, e é por isso que este `Default` é escrito e
    /// não derivado: `contrast` neutro é `1.0` (é um multiplicador) e
    /// `sharpen_radius` é `1.0` (raio zero seria não ter pixel de vizinhança).
    /// Derivar daria zero nos dois, e a foto abriria já alterada — sem ninguém
    /// ter tocado em nada.
    ///
    /// 🚨 **`lens_vignette_midpoint` era `50.0` aqui, e estava errado.** O 50 veio
    /// de `GpuEditParams::default` do `crates/ui` — que o app de lá **nunca
    /// chama**: o único chamador em todo o repositório é um teste que confere só
    /// os 11 campos do Básico. O que o legado de fato usa é
    /// `AppState::new`/`reset_edits`, e nos dois o meio da vinheta é **`0.0`**.
    /// Copiar a `impl Default` em vez do caminho vivo fazia o slider "Meio da
    /// vinheta" abrir em 50 aqui e em 0 lá, na mesma foto.
    ///
    /// 🔑 A pergunta que separa os dois: não é "qual é o padrão declarado", é
    /// **"qual valor a foto recebe quando ninguém mexeu em nada"**. `impl Default`
    /// responde a primeira, e ela pode ser código morto.
    ///
    /// ⚠️ **E a resposta certa, para foto de verdade, é um quarto lugar: o
    /// schema.** `014_add_hsl_lens_fields.sql` cria a coluna com
    /// `DEFAULT 50.0`, então toda foto importada volta do banco com o meio da
    /// vinheta preenchido — e é esse 50 que os dois apps mostram no slider. O que
    /// este `Default` decide é o resto: o estado antes de abrir qualquer foto, o
    /// futuro "redefinir", e o campo que vier `NULL`. Nos três o legado diz 0.0.
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            clarity: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
            tone_curve_shadows: 0.0,
            tone_curve_darks: 0.0,
            tone_curve_lights: 0.0,
            tone_curve_highlights: 0.0,
            hsl_red_sat: 0.0,
            hsl_orange_sat: 0.0,
            hsl_yellow_sat: 0.0,
            hsl_green_sat: 0.0,
            hsl_aqua_sat: 0.0,
            hsl_blue_sat: 0.0,
            hsl_purple_sat: 0.0,
            hsl_magenta_sat: 0.0,
            hsl_red_hue: 0.0,
            hsl_orange_hue: 0.0,
            hsl_yellow_hue: 0.0,
            hsl_green_hue: 0.0,
            hsl_aqua_hue: 0.0,
            hsl_blue_hue: 0.0,
            hsl_purple_hue: 0.0,
            hsl_magenta_hue: 0.0,
            hsl_red_lum: 0.0,
            hsl_orange_lum: 0.0,
            hsl_yellow_lum: 0.0,
            hsl_green_lum: 0.0,
            hsl_aqua_lum: 0.0,
            hsl_blue_lum: 0.0,
            hsl_purple_lum: 0.0,
            hsl_magenta_lum: 0.0,
            lens_distortion: 0.0,
            lens_vignette_amount: 0.0,
            lens_vignette_midpoint: 0.0,
            nr_luminance: 0.0,
            nr_color: 0.0,
            sharpen_amount: 0.0,
            sharpen_radius: 1.0,
        }
    }
}

/// Um pedido de revelação.
pub struct Pedido {
    pub id: u64,
    /// `Arc` para o pixel não ser copiado a cada arrasto — e, do outro lado, é a
    /// **identidade** do `Arc` que decide se a textura precisa subir de novo.
    /// Mover 24 MB para a GPU a cada milímetro de slider é o que separa arrastar
    /// liso de arrastar aos trancos.
    pub pixels: Arc<Vec<u8>>,
    pub largura: u32,
    pub altura: u32,
    pub ajustes: Ajustes,
}

/// O que volta.
pub struct Resultado {
    pub id: u64,
    pub imagem: DynamicImage,
    pub duracao_ms: f32,
}

pub struct Processador {
    pedidos: Sender<Pedido>,
    resultados: Receiver<Resultado>,
    /// O id do pedido mais recente. A thread compara contra ele para largar o
    /// que já não interessa, e por isso ele é compartilhado, não copiado.
    id_atual: Arc<Mutex<u64>>,
    disponivel: Arc<Mutex<Option<bool>>>,
}

impl Processador {
    pub fn novo() -> Self {
        let (envia_pedido, recebe_pedido) = channel::<Pedido>();
        let (envia_resultado, recebe_resultado) = channel::<Resultado>();
        let id_atual = Arc::new(Mutex::new(0u64));
        let disponivel = Arc::new(Mutex::new(None));

        {
            let id_atual = id_atual.clone();
            let disponivel = disponivel.clone();
            std::thread::spawn(move || {
                laco(recebe_pedido, envia_resultado, id_atual, disponivel);
            });
        }

        Self {
            pedidos: envia_pedido,
            resultados: recebe_resultado,
            id_atual,
            disponivel,
        }
    }

    /// `None` enquanto a thread ainda está abrindo o dispositivo.
    ///
    /// Três estados, e não dois: "ainda não sei" é diferente de "não tem GPU", e
    /// tratá-los igual faria a tela anunciar ausência de placa durante os
    /// milissegundos de abertura — em toda abertura.
    pub fn disponivel(&self) -> Option<bool> {
        *self.disponivel.lock()
    }

    pub fn proximo_id(&self) -> u64 {
        let mut guarda = self.id_atual.lock();
        *guarda += 1;
        *guarda
    }

    /// Enfileira, e marca este como o pedido que interessa.
    pub fn pedir(&self, pedido: Pedido) -> u64 {
        let id = pedido.id;
        *self.id_atual.lock() = id;
        let _ = self.pedidos.send(pedido);
        id
    }

    /// O resultado mais recente que chegou, descartando os atrasados.
    ///
    /// Drena a fila inteira em vez de devolver o primeiro: durante um arrasto
    /// chegam vários, e desenhar os intermediários é gastar quadro para mostrar
    /// estado que já passou.
    pub fn colher(&self) -> Option<Resultado> {
        let mut ultimo: Option<Resultado> = None;
        while let Ok(resultado) = self.resultados.try_recv() {
            if ultimo.as_ref().is_none_or(|u| resultado.id > u.id) {
                ultimo = Some(resultado);
            }
        }
        ultimo
    }
}

impl Default for Processador {
    fn default() -> Self {
        Self::novo()
    }
}

/// Recursos por tamanho de imagem.
///
/// Trocar de foto no mesmo tamanho reaproveita textura e buffer; trocar de
/// tamanho cria de novo. O cache guarda cinco tamanhos porque uma sessão de
/// revelação alterna entre poucas resoluções (o preview, o full, o recorte), e
/// recriar textura a cada troca aparece como engasgo.
struct Recursos {
    textura_entrada: wgpu::Texture,
    textura_saida: wgpu::Texture,
    buffer_ajustes: wgpu::Buffer,
    buffer_saida: wgpu::Buffer,
    grupo: wgpu::BindGroup,
    bytes_por_linha_alinhado: u32,
    bytes_por_linha: u32,
    /// Os pixels que já estão na textura de entrada — por identidade de `Arc`,
    /// não por conteúdo. Comparar 24 MB byte a byte para decidir se vale subir
    /// 24 MB custaria quase o mesmo que subir.
    ultimos_pixels: Option<Arc<Vec<u8>>>,
}

/// O laço da thread: abre o dispositivo e atende pedidos até o canal fechar.
fn laco(
    pedidos: Receiver<Pedido>,
    resultados: Sender<Resultado>,
    id_atual: Arc<Mutex<u64>>,
    disponivel: Arc<Mutex<Option<bool>>>,
) {
    let instancia = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adaptador = pollster::block_on(instancia.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));

    let Some(adaptador) = adaptador else {
        *disponivel.lock() = Some(false);
        return;
    };

    let dispositivo = pollster::block_on(adaptador.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("VintageLightbox GPU"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ));

    let Ok((dispositivo, fila)) = dispositivo else {
        *disponivel.lock() = Some(false);
        return;
    };

    // 🚨 O **mesmo** WGSL do `crates/ui`, e há teste conferindo byte a byte
    // (`o_shader_e_o_mesmo_do_crates_ui`). O critério de saída da fase 2 é
    // igualdade de pixel: dois shaders parecidos dariam imagens parecidas, e
    // "parecida" é justamente o que ninguém consegue julgar olhando.
    let modulo = dispositivo.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Image Adjustments Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/image_adjustments.wgsl").into()),
    });

    let pipeline = dispositivo.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Image Processing Pipeline"),
        layout: None,
        module: &modulo,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    *disponivel.lock() = Some(true);

    let mut cache: LruCache<(u32, u32), Recursos> =
        LruCache::new(NonZeroUsize::new(5).expect("5 não é zero"));

    while let Ok(pedido) = pedidos.recv() {
        // Pedido velho é largado sem processar: durante um arrasto a fila enche,
        // e o que interessa é sempre o último.
        if pedido.id < *id_atual.lock() {
            continue;
        }

        let comeco = std::time::Instant::now();
        if let Some(imagem) = revelar(&dispositivo, &fila, &pipeline, &pedido, &mut cache) {
            let _ = resultados.send(Resultado {
                id: pedido.id,
                imagem,
                duracao_ms: comeco.elapsed().as_secs_f32() * 1000.0,
            });
        }
    }
}

/// Uma passada: sobe o que mudou, despacha o compute, lê de volta.
fn revelar(
    dispositivo: &wgpu::Device,
    fila: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    pedido: &Pedido,
    cache: &mut LruCache<(u32, u32), Recursos>,
) -> Option<DynamicImage> {
    let (largura, altura) = (pedido.largura, pedido.altura);

    if !cache.contains(&(largura, altura)) {
        cache.put(
            (largura, altura),
            criar_recursos(dispositivo, pipeline, largura, altura),
        );
    }
    let recursos = cache.get_mut(&(largura, altura))?;

    let precisa_subir = match &recursos.ultimos_pixels {
        Some(ultimos) => !Arc::ptr_eq(ultimos, &pedido.pixels),
        None => true,
    };

    if precisa_subir {
        fila.write_texture(
            wgpu::ImageCopyTexture {
                texture: &recursos.textura_entrada,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pedido.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(recursos.bytes_por_linha),
                rows_per_image: Some(altura),
            },
            wgpu::Extent3d {
                width: largura,
                height: altura,
                depth_or_array_layers: 1,
            },
        );
        recursos.ultimos_pixels = Some(pedido.pixels.clone());
    }

    fila.write_buffer(
        &recursos.buffer_ajustes,
        0,
        bytemuck::bytes_of(&pedido.ajustes),
    );

    let mut encoder = dispositivo.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Compute Encoder"),
    });

    {
        let mut passe = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Image Processing Pass"),
            timestamp_writes: None,
        });
        passe.set_pipeline(pipeline);
        passe.set_bind_group(0, &recursos.grupo, &[]);
        // Grupos de 16×16, como o `@workgroup_size` do WGSL declara. A divisão
        // arredonda para cima: com 17 pixels de largura o último grupo trabalha
        // pela metade, e é o shader que descarta quem cair fora.
        passe.dispatch_workgroups(largura.div_ceil(16), altura.div_ceil(16), 1);
    }

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &recursos.textura_saida,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &recursos.buffer_saida,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(recursos.bytes_por_linha_alinhado),
                rows_per_image: Some(altura),
            },
        },
        wgpu::Extent3d {
            width: largura,
            height: altura,
            depth_or_array_layers: 1,
        },
    );

    fila.submit(std::iter::once(encoder.finish()));

    let fatia = recursos.buffer_saida.slice(..);
    let (avisa, espera) = channel();
    fatia.map_async(wgpu::MapMode::Read, move |r| {
        let _ = avisa.send(r);
    });
    dispositivo.poll(wgpu::Maintain::Wait);
    espera.recv().ok()?.ok()?;

    let dados = fatia.get_mapped_range();
    // A GPU devolve cada linha alinhada em 256 bytes; a imagem não tem esse
    // enchimento. Copiar o buffer inteiro daria uma foto com listras
    // deslocadas — e quanto mais estreita, mais torta.
    let mut pixels = Vec::with_capacity((recursos.bytes_por_linha * altura) as usize);
    for y in 0..altura {
        let inicio = (y * recursos.bytes_por_linha_alinhado) as usize;
        pixels.extend_from_slice(&dados[inicio..inicio + recursos.bytes_por_linha as usize]);
    }
    drop(dados);
    recursos.buffer_saida.unmap();

    Some(DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
        largura, altura, pixels,
    )?))
}

fn criar_recursos(
    dispositivo: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    largura: u32,
    altura: u32,
) -> Recursos {
    /// Exigência do wgpu ao copiar textura para buffer.
    const ALINHAMENTO: u32 = 256;
    let bytes_por_linha = 4 * largura;
    let bytes_por_linha_alinhado = bytes_por_linha.div_ceil(ALINHAMENTO) * ALINHAMENTO;

    let descritor = |rotulo, uso| wgpu::TextureDescriptor {
        label: Some(rotulo),
        size: wgpu::Extent3d {
            width: largura,
            height: altura,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: uso,
        view_formats: &[],
    };

    let textura_entrada = dispositivo.create_texture(&descritor(
        "Input Texture",
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    ));
    let textura_saida = dispositivo.create_texture(&descritor(
        "Output Texture",
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
    ));

    let buffer_ajustes = dispositivo.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Params Buffer"),
        size: std::mem::size_of::<Ajustes>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let grupo = dispositivo.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Compute Bind Group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &textura_entrada.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    &textura_saida.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffer_ajustes.as_entire_binding(),
            },
        ],
    });

    let buffer_saida = dispositivo.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (bytes_por_linha_alinhado * altura) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    Recursos {
        textura_entrada,
        textura_saida,
        buffer_ajustes,
        buffer_saida,
        grupo,
        bytes_por_linha_alinhado,
        bytes_por_linha,
        ultimos_pixels: None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 O shader é o **mesmo arquivo** do `crates/ui`, byte a byte.
    ///
    /// O critério de saída da fase 2 é igualdade de pixel. Dois shaders
    /// "equivalentes" dariam imagens "parecidas" — e parecida é exatamente o que
    /// ninguém julga olhando: um `1e-3` a mais numa constante de vinheta some
    /// numa foto e aparece em outra, meses depois.
    ///
    /// Cópia, e não `include_str!` cruzando os crates, porque o `crates/ui` sai
    /// do workspace na fase 5 e levaria o caminho junto. Este teste é a trava
    /// enquanto os dois existem, e some com ele.
    #[test]
    fn o_shader_e_o_mesmo_do_crates_ui() {
        let aqui = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/shaders/image_adjustments.wgsl"
        );
        let la = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ui/src/shaders/image_adjustments.wgsl"
        );

        let nosso = std::fs::read(aqui).expect("ler o shader do ui-gpui");
        let Ok(deles) = std::fs::read(la) else {
            // Depois da fase 5 o `crates/ui` não existe mais, e aí não há o que
            // comparar. Falhar seria transformar o fim da migração em suíte
            // vermelha.
            return;
        };

        assert_eq!(
            nosso.len(),
            deles.len(),
            "o shader divergiu em tamanho — alguém editou um lado só"
        );
        assert!(
            nosso == deles,
            "o shader divergiu em conteúdo — a igualdade de pixel da fase 2 depende dele"
        );
    }

    /// ⚠️ O neutro **não é zero** em dois campos — e num terceiro parecia não ser.
    ///
    /// `contrast` é multiplicador e `sharpen_radius` zero seria não ter pixel de
    /// vizinhança: um `#[derive(Default)]` daria zero nos dois e a foto abriria
    /// alterada sem ninguém ter tocado em nada.
    ///
    /// 🚨 `lens_vignette_midpoint` está aqui pelo motivo oposto: ele **é** zero, e
    /// já esteve em 50 porque foi copiado de `GpuEditParams::default`, uma `impl`
    /// que o app do legado nunca chama. O que o legado usa — `AppState::new` e
    /// `reset_edits` — diz `0.0`. O teste cobra o valor do caminho vivo, e não o
    /// do padrão declarado.
    #[test]
    fn o_neutro_e_o_do_caminho_vivo_do_legado() {
        let neutro = Ajustes::default();
        assert_eq!(neutro.contrast, 1.0);
        assert_eq!(neutro.sharpen_radius, 1.0);
        assert_eq!(
            neutro.lens_vignette_midpoint, 0.0,
            "é o que `AppState::new` e `reset_edits` põem no slider; \
             o 50 de `GpuEditParams::default` é código morto"
        );
        assert_eq!(neutro.exposure, 0.0);
        assert_eq!(neutro.saturation, 0.0);
    }

    /// Espera a thread abrir o dispositivo, e falha se não houver GPU.
    ///
    /// ⚠️ **Falha, e não pula.** O produto declara macOS e Windows, onde sempre
    /// há adaptador; uma máquina rodando esta suíte sem GPU precisa saber que
    /// não está conferindo o motor de revelação. Teste que se cala quando não
    /// pode rodar é teste que some do placar sem ninguém notar.
    fn processador_pronto() -> Processador {
        let processador = Processador::novo();
        for _ in 0..300 {
            match processador.disponivel() {
                Some(true) => return processador,
                Some(false) => {
                    panic!("nenhum adaptador de GPU — o motor de revelação não roda aqui")
                }
                None => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        panic!("a GPU não respondeu em 6s");
    }

    fn cinza(lado: u32, valor: u8) -> Arc<Vec<u8>> {
        Arc::new(
            std::iter::repeat_n([valor, valor, valor, 255], (lado * lado) as usize)
                .flatten()
                .collect(),
        )
    }

    fn revelar_e_colher(
        processador: &Processador,
        pixels: Arc<Vec<u8>>,
        ajustes: Ajustes,
    ) -> Vec<u8> {
        const LADO: u32 = 16;
        let id = processador.proximo_id();
        processador.pedir(Pedido {
            id,
            pixels,
            largura: LADO,
            altura: LADO,
            ajustes,
        });

        for _ in 0..300 {
            if let Some(resultado) = processador.colher() {
                assert_eq!(resultado.id, id);
                return resultado.imagem.into_rgba8().into_raw();
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("o processador não devolveu resultado em 6s");
    }

    /// 🚨 O neutro tem de sair **igual** ao que entrou.
    ///
    /// É o teste que vale mais nesta fase, e não é sobre a GPU: é sobre os 46
    /// valores de `Ajustes::default`. Um só deles fora do neutro faz toda foto
    /// abrir alterada — sem erro, sem aviso, e parecendo decisão de cor de quem
    /// escreveu o shader. O sintoma seria descoberto na comparação de pixel da
    /// fase 5, com três meses de trabalho em cima.
    #[test]
    fn o_neutro_devolve_o_pixel_intacto() {
        let processador = processador_pronto();
        let entrada = cinza(16, 100);
        let saida = revelar_e_colher(&processador, entrada.clone(), Ajustes::default());

        assert_eq!(
            saida.as_slice(),
            entrada.as_slice(),
            "algum campo de `Ajustes::default` não é neutro"
        );
    }

    /// A exposição atravessou: `+1` dobra o valor, como `pow(2, exposure)` manda.
    ///
    /// Prova que o caminho inteiro está de pé — textura sobe, `uniform` chega no
    /// campo certo, compute despacha, buffer volta sem o enchimento de 256 bytes
    /// por linha. Se o `Ajustes` estivesse deslocado de um campo, aqui apareceria
    /// como exposição que não faz nada.
    #[test]
    fn exposicao_de_um_ponto_dobra_o_valor() {
        let processador = processador_pronto();
        let saida = revelar_e_colher(
            &processador,
            cinza(16, 100),
            Ajustes {
                exposure: 1.0,
                ..Default::default()
            },
        );

        assert_eq!(&saida[0..4], &[200, 200, 200, 255]);
        // O último pixel também: linha final é onde o desalinhamento de 256
        // bytes apareceria primeiro.
        assert_eq!(&saida[saida.len() - 4..], &[200, 200, 200, 255]);
    }

    const NOMES: [&str; 46] = [
        "exposure",
        "contrast",
        "temperature",
        "tint",
        "highlights",
        "shadows",
        "whites",
        "blacks",
        "clarity",
        "vibrance",
        "saturation",
        "tone_curve_shadows",
        "tone_curve_darks",
        "tone_curve_lights",
        "tone_curve_highlights",
        "hsl_red_sat",
        "hsl_orange_sat",
        "hsl_yellow_sat",
        "hsl_green_sat",
        "hsl_aqua_sat",
        "hsl_blue_sat",
        "hsl_purple_sat",
        "hsl_magenta_sat",
        "hsl_red_hue",
        "hsl_orange_hue",
        "hsl_yellow_hue",
        "hsl_green_hue",
        "hsl_aqua_hue",
        "hsl_blue_hue",
        "hsl_purple_hue",
        "hsl_magenta_hue",
        "hsl_red_lum",
        "hsl_orange_lum",
        "hsl_yellow_lum",
        "hsl_green_lum",
        "hsl_aqua_lum",
        "hsl_blue_lum",
        "hsl_purple_lum",
        "hsl_magenta_lum",
        "lens_distortion",
        "lens_vignette_amount",
        "lens_vignette_midpoint",
        "nr_luminance",
        "nr_color",
        "sharpen_amount",
        "sharpen_radius",
    ];

    /// Uma amostra 16×16 desenhada para que **todo** ajuste do shader tenha onde
    /// agir: rampa de cinza de 0 a 255 (altas luzes, sombras, brancos, pretos e
    /// as quatro zonas da curva), as 8 cores do HSL saturadas, e as mesmas 8
    /// esmaecidas (`vibrance` só age onde `max_diff < 64`). Tudo em xadrez de
    /// 1px, para haver borda dura em toda parte — sem borda, nitidez e redução de
    /// ruído não teriam o que fazer.
    fn amostra() -> Arc<Vec<u8>> {
        const CORES: [[u8; 3]; 8] = [
            [220, 40, 40],
            [230, 140, 30],
            [230, 220, 40],
            [40, 200, 60],
            [40, 210, 200],
            [50, 80, 220],
            [140, 50, 210],
            [220, 50, 180],
        ];
        let mut pixels = Vec::with_capacity(16 * 16 * 4);
        for y in 0u32..16 {
            for x in 0u32..16 {
                let cor = match y {
                    // Rampa de cinza, com um degrau por coluna.
                    0..=3 => [(x * 17) as u8; 3],
                    // As 8 cores, uma por linha, em xadrez com cinza médio.
                    4..=11 if (x + y) % 2 == 0 => CORES[(y - 4) as usize],
                    4..=11 => [128, 128, 128],
                    // As mesmas, puxadas para perto do cinza.
                    _ if (x + y) % 2 == 0 => {
                        let base = CORES[(y - 12) as usize];
                        [
                            (128 + (base[0] as i32 - 128) / 4) as u8,
                            (128 + (base[1] as i32 - 128) / 4) as u8,
                            (128 + (base[2] as i32 - 128) / 4) as u8,
                        ]
                    }
                    _ => [128, 128, 128],
                };
                pixels.extend_from_slice(&[cor[0], cor[1], cor[2], 255]);
            }
        }
        Arc::new(pixels)
    }

    /// Escreve **por posição**, e não por nome: é assim que o `uniform` chega à
    /// GPU, e é a única forma de perguntar "o que o shader faz com o campo *n*"
    /// sem depender de qual nome o Rust deu a ele.
    fn com_campos(alterados: &[(usize, f32)]) -> Ajustes {
        let neutro = Ajustes::default();
        let mut bytes = bytemuck::bytes_of(&neutro).to_vec();
        let campos: &mut [f32] = bytemuck::cast_slice_mut(&mut bytes);
        for (indice, valor) in alterados {
            campos[*indice] = *valor;
        }
        *bytemuck::from_bytes(&bytes)
    }

    fn com_campo(indice: usize, valor: f32) -> Ajustes {
        com_campos(&[(indice, valor)])
    }

    /// Os nomes do `struct Params` do WGSL, na ordem em que ele os declara.
    fn campos_do_wgsl() -> Vec<String> {
        let shader = include_str!("../shaders/image_adjustments.wgsl");
        shader
            .split("struct Params {")
            .nth(1)
            .and_then(|resto| resto.split('}').next())
            .expect("o shader tem de declarar `struct Params`")
            .lines()
            .filter_map(|linha| linha.split(':').next())
            .map(str::trim)
            .filter(|nome| !nome.is_empty() && !nome.starts_with("//"))
            .map(str::to_string)
            .collect()
    }

    /// Soma das diferenças entre vizinhos horizontais.
    ///
    /// Cai quando a imagem borra, sobe quando ela é afiada — é o que separa
    /// "mudou alguma coisa" de "virou exatamente o ajuste do vizinho".
    fn contraste_local(pixels: &[u8]) -> u64 {
        let mut soma = 0u64;
        for y in 0..16usize {
            for x in 1..16usize {
                for canal in 0..3usize {
                    let atual = pixels[(y * 16 + x) * 4 + canal] as i32;
                    let anterior = pixels[(y * 16 + x - 1) * 4 + canal] as i32;
                    soma += atual.abs_diff(anterior) as u64;
                }
            }
        }
        soma
    }

    /// 🚨 **O `struct Params` do WGSL não é o `Ajustes` do Rust.** Ele declara 28
    /// campos para os 46 que a CPU manda, e a divergência começa no 23.
    ///
    /// O `uniform` chega à GPU como bytes crus, **por posição**. Enquanto os
    /// nomes batem, cada slider move o que promete; a partir do 23 o shader lê o
    /// campo do vizinho:
    ///
    /// | posição | o Rust manda | o shader lê como |
    /// |--------:|--------------|------------------|
    /// | 23 | `hsl_red_hue` | `nr_luminance` |
    /// | 24 | `hsl_orange_hue` | `nr_luminance` **de novo** — declarado duas vezes |
    /// | 25 | `hsl_yellow_hue` | `nr_color` |
    /// | 26 | `hsl_green_hue` | `sharpen_amount` |
    /// | 27 | `hsl_aqua_hue` | `sharpen_radius` |
    /// | 28–45 | matiz, luminância, lente, ruído, nitidez | **nada** — fora do `uniform` |
    ///
    /// Nada disso falha em lugar nenhum: o buffer é maior que o mínimo que o
    /// binding exige, então o wgpu aceita e ignora a sobra.
    ///
    /// ⚠️ **Este teste prende um defeito de propósito** (regra §7.3: portar é
    /// reescrever com a regra entendida, defeito preservado fica registrado em
    /// teste). Ele **tem de falhar** no dia em que o WGSL for consertado — e aí a
    /// tabela acima, o `docs/10-MIGRACAO-GPUI.md` e o `crates/ui` mudam juntos,
    /// porque o shader é o mesmo arquivo nos dois apps.
    #[test]
    fn o_wgsl_declara_28_campos_para_os_46_que_o_rust_manda() {
        let wgsl = campos_do_wgsl();

        assert_eq!(wgsl.len(), 28, "o `struct Params` do WGSL mudou de tamanho");
        assert_eq!(NOMES.len(), 46, "o `Ajustes` do Rust mudou de tamanho");

        assert_eq!(
            wgsl[..23],
            NOMES[..23],
            "até o campo 22 os dois lados batem — é o que faz o Básico e o HSL/cor funcionarem"
        );
        assert_eq!(
            wgsl[23..],
            [
                "nr_luminance",
                "nr_luminance",
                "nr_color",
                "sharpen_amount",
                "sharpen_radius"
            ],
            "a partir do 23 o shader lê o campo do vizinho — e `nr_luminance` está declarado duas vezes"
        );
    }

    /// 🚨 Só os 23 primeiros ajustes chegam à GPU. Os 18 últimos não chegam.
    ///
    /// A contraprova do teste acima, medida na imagem em vez de lida no arquivo:
    /// mexer em cada um dos 46 campos, um por vez, e ver quais mudam algum pixel.
    #[test]
    fn os_ajustes_a_partir_do_campo_28_nao_mudam_nenhum_pixel() {
        let processador = processador_pronto();
        let entrada = amostra();
        let neutro = revelar_e_colher(&processador, entrada.clone(), Ajustes::default());

        for (i, nome) in NOMES.iter().enumerate().take(23) {
            let saida = revelar_e_colher(&processador, entrada.clone(), com_campo(i, 60.0));
            assert_ne!(
                saida, neutro,
                "`{nome}` (campo {i}) devia chegar ao shader e não mudou nada"
            );
        }

        for (i, nome) in NOMES.iter().enumerate().skip(28) {
            let saida = revelar_e_colher(&processador, entrada.clone(), com_campo(i, 60.0));
            assert_eq!(
                saida, neutro,
                "`{nome}` (campo {i}) mudou a foto — o `struct Params` do WGSL cresceu?"
            );
        }
    }

    /// 🚨 O slider "HSL / matiz — Vermelho" **borra a foto**.
    ///
    /// É o campo 23, que o shader lê como `nr_luminance`. Um ajuste de matiz gira
    /// a cor e não pode mexer no contraste entre vizinhos; redução de ruído faz
    /// exatamente o contrário. O contraste local caindo é a assinatura de um
    /// borrão, e nenhum giro de matiz produziria isso.
    ///
    /// Para quem usa o app, o sintoma é o pior tipo: o controle responde, a foto
    /// muda, e o que mudou não tem nada a ver com o rótulo.
    #[test]
    fn o_matiz_do_vermelho_borra_a_foto_em_vez_de_girar_a_cor() {
        let processador = processador_pronto();
        let entrada = amostra();

        let neutro = contraste_local(&revelar_e_colher(
            &processador,
            entrada.clone(),
            Ajustes::default(),
        ));
        let com_matiz = contraste_local(&revelar_e_colher(
            &processador,
            entrada.clone(),
            com_campo(23, 60.0),
        ));

        assert!(
            com_matiz < neutro,
            "o campo 23 devia borrar (é lido como `nr_luminance`): contraste local {com_matiz} vs {neutro} no neutro"
        );
    }

    /// 🚨 O preset de sistema **"B&W" não deixa a foto em preto e branco**.
    ///
    /// `ListPresetsUseCase` constrói os cinco presets de sistema, e o "B&W" pede
    /// `saturation: Some(-100.0)`. Mas a saturação do shader é um fator, não uma
    /// porcentagem:
    ///
    /// ```wgsl
    /// let factor = 1.0 + params.saturation;
    /// r = lum2 + (r - lum2) * factor;
    /// ```
    ///
    /// Cinza é `factor == 0`, ou seja **`-1.0`** — e é por isso que o slider de
    /// saturação vai de -1 a 1 (`controles.rs`, lido de `dock_viewer.rs`). Com
    /// `-100`, o fator é `-99`: cada canal é jogado 99 vezes para o **lado oposto**
    /// do cinza. Não é ausência de cor, é cor invertida e estourada.
    ///
    /// Este teste mede os dois: o `-1.0` deixa os três canais iguais; o `-100.0`
    /// não. Vale para os dois apps — o preset vem do mesmo use case.
    #[test]
    fn o_preset_bw_do_legado_nao_da_preto_e_branco() {
        let processador = processador_pronto();
        let entrada = amostra();

        let cinza_de_verdade = revelar_e_colher(
            &processador,
            entrada.clone(),
            Ajustes {
                saturation: -1.0,
                ..Default::default()
            },
        );
        for pixel in cinza_de_verdade.chunks_exact(4) {
            assert_eq!(
                (pixel[0], pixel[1]),
                (pixel[1], pixel[2]),
                "saturação -1.0 é o fator zero: os três canais têm de virar o mesmo valor"
            );
        }

        let como_o_preset_pede = revelar_e_colher(
            &processador,
            entrada,
            Ajustes {
                saturation: -100.0,
                ..Default::default()
            },
        );
        assert!(
            como_o_preset_pede
                .chunks_exact(4)
                .any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2]),
            "se isto passar a dar cinza, o shader ou o preset mudaram — e o defeito acabou"
        );
    }

    /// 🚨 Os 4 controles de Detalhe e os 3 de Lente não fazem **nada**.
    ///
    /// Sete sliders que o painel oferece, arrastam, mostram número — e a foto não
    /// muda, porque os campos deles ficam além do que o `uniform` do shader
    /// declara. Nitidez é testada com raio junto: raio sozinho nunca faria efeito,
    /// e o teste passaria por engano.
    #[test]
    fn detalhe_e_lente_nao_chegam_ao_shader() {
        let processador = processador_pronto();
        let entrada = amostra();
        let neutro = revelar_e_colher(&processador, entrada.clone(), Ajustes::default());

        let casos: [(&str, &[(usize, f32)]); 5] = [
            ("Detalhe — Ruído (luminância)", &[(42, 60.0)]),
            ("Detalhe — Ruído (cor)", &[(43, 60.0)]),
            ("Detalhe — Nitidez (com raio)", &[(44, 80.0), (45, 2.0)]),
            ("Lente — Distorção", &[(39, 60.0)]),
            ("Lente — Vinheta (com meio)", &[(40, 80.0), (41, 30.0)]),
        ];

        for (rotulo, campos) in casos {
            let saida = revelar_e_colher(&processador, entrada.clone(), com_campos(campos));
            assert_eq!(saida, neutro, "`{rotulo}` passou a fazer efeito");
        }
    }

    /// O layout que vai para a GPU tem os 46 campos que o `crates/ui` manda.
    ///
    /// ⚠️ Este teste dizia "os 46 campos **que o WGSL declara**" — e o WGSL
    /// declara 28. Ele nunca conferiu isso: `size_of` não sabe do shader. Era uma
    /// afirmação escrita ao lado de um teste que não a mediaria nunca, e foi assim
    /// que o desalinhamento sobreviveu à leitura de todo mundo. Quem confere o
    /// outro lado é `o_wgsl_declara_28_campos_para_os_46_que_o_rust_manda`.
    ///
    /// O que ele confere de fato: que ninguém acrescentou ou tirou um campo aqui.
    /// Campo a mais desloca **todos** os seguintes na leitura do shader, e o
    /// sintoma é a saturação virando nitidez.
    #[test]
    fn o_layout_tem_46_campos_de_quatro_bytes() {
        assert_eq!(std::mem::size_of::<Ajustes>(), 46 * 4);
    }
}
