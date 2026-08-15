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
/// campo, por **posição**. O `uniform` do outro lado
/// (`shaders/image_adjustments.wgsl`) declara os mesmos 46 na mesma ordem, e a
/// única defesa contra os dois divergirem é poder ler um ao lado do outro.
/// Traduzir os nomes tiraria essa leitura e não daria nada em troca — um campo
/// fora de lugar aqui não é erro de compilação, é a foto saindo com o ajuste
/// errado aplicado.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
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
    /// O neutro, conferido campo a campo contra o `GpuEditParams::default` do
    /// `crates/ui`.
    ///
    /// ⚠️ **Nem todo neutro é zero**, e é por isso que este `Default` é escrito e
    /// não derivado: `contrast` neutro é `1.0` (é um multiplicador),
    /// `lens_vignette_midpoint` é `50.0` (é o meio de uma escala de 0 a 100) e
    /// `sharpen_radius` é `1.0` (raio zero seria não ter pixel). Derivar daria
    /// zero nos três, e a foto abriria já alterada — sem ninguém ter tocado em
    /// nada.
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
            lens_vignette_midpoint: 50.0,
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

    /// ⚠️ O neutro **não é zero** em três campos.
    ///
    /// `contrast` é multiplicador, `lens_vignette_midpoint` é o meio de uma
    /// escala de 0 a 100 e `sharpen_radius` zero seria não ter pixel. Um
    /// `#[derive(Default)]` daria zero nos três e a foto abriria alterada sem
    /// ninguém ter tocado em nada — sem erro, e parecendo decisão de cor.
    #[test]
    fn o_neutro_nao_e_tudo_zero() {
        let neutro = Ajustes::default();
        assert_eq!(neutro.contrast, 1.0);
        assert_eq!(neutro.lens_vignette_midpoint, 50.0);
        assert_eq!(neutro.sharpen_radius, 1.0);
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

    /// O layout que vai para a GPU tem os 46 campos que o WGSL declara.
    ///
    /// Não confere nome nem ordem — confere que ninguém acrescentou ou tirou um
    /// campo de um lado só. Campo a mais aqui desloca **todos** os seguintes na
    /// leitura do shader, e o sintoma é a saturação virando nitidez.
    #[test]
    fn o_layout_tem_46_campos_de_quatro_bytes() {
        assert_eq!(std::mem::size_of::<Ajustes>(), 46 * 4);
    }
}
