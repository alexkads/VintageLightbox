//! O motor de revelação: wgpu, o WGSL, e os 46 ajustes que viajam para ele.
//!
//! 🔑 **Ele mora aqui, e não no crate de interface, porque wgpu é detalhe
//! técnico** — que é exatamente o que esta camada guarda. Estava em
//! `ui-gpui/revelacao/processador.rs` por herança: veio do `gpu_processor.rs`
//! do `crates/ui`, onde nasceu colado na tela.
//!
//! O que o descolou foi a exportação. `ImageExporterImpl` tinha a **própria**
//! implementação dos ajustes, na CPU, com 15 dos 46 e uma matemática que já
//! divergia nos que aplicava: o ruído do shader é bilateral e o de lá era um
//! `blur` gaussiano; a nitidez entra antes dos tons no shader e entrava depois
//! no exportador. Duas respostas para a mesma pergunta, e quem revelava via uma
//! na tela e recebia a outra no arquivo.
//!
//! Com o motor aqui, a tela e o arquivo atravessam o **mesmo** `.wgsl`.
//!
//! ## O que ficou do outro lado
//!
//! A fila de pedidos — thread, canal, descarte do pedido velho durante um
//! arrasto — continua em `ui-gpui`: é resposta a um dedo arrastando um slider,
//! e não tem o que fazer numa exportação, que roda uma vez e espera.

use std::num::NonZeroUsize;
use std::sync::mpsc::channel;
use std::sync::Arc;

use image::DynamicImage;
use lru::LruCache;

/// Os ajustes, no layout que o WGSL espera.
///
/// 🚨 **Os nomes e a ordem são os do shader, e ficam em inglês de propósito.**
/// `repr(C)` + `bytemuck` mandam esta struct para a GPU como bytes crus, campo a
/// campo, por **posição**. Um campo fora de lugar aqui não é erro de compilação,
/// é a foto saindo com o ajuste errado aplicado — e poder ler os dois lados um ao
/// lado do outro é a única defesa que existe.
///
/// 🚨 **E ela já falhou uma vez: o `uniform` do outro lado declarava 28 campos.**
/// Do campo 23 em diante o shader lia o do vizinho (o matiz do vermelho virava
/// redução de ruído) e do 28 em diante não lia nada — os 4 controles de Detalhe e
/// os 3 de Lente não faziam efeito nenhum. Defeito herdado do `crates/ui`, que
/// mandava a mesma struct para o mesmo shader, e **consertado em 17/ago/2026**,
/// depois que a fase 5 tirou o outro app do caminho: o `struct Params` passou a
/// declarar os 46 na mesma ordem, e quem prende isso é
/// `o_wgsl_declara_os_mesmos_46_campos_na_mesma_ordem`.
///
/// ⚠️ **Declarado não é aplicado.** Chegar ao shader é a primeira metade; ter
/// código que os use é a segunda. O matiz e a luminância do HSL ganharam a sua em
/// 17/ago/2026 (16 sliders); **a Lente ainda não tem**
/// (`a_lente_ainda_nao_tem_codigo_no_shader`).
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

/// O tamanho do buffer de `uniform`, arredondado para múltiplo de 16 bytes.
///
/// 🚨 **Não é `size_of::<Ajustes>()`, e a diferença é uma regra do WGSL.** No
/// endereço `uniform` o alinhamento de uma struct é `roundUp(16, …)`, então os
/// 46 `f32` (184 bytes) ocupam 192 do ponto de vista do shader — e o `bind
/// group` recusa um buffer menor que isso. Enquanto o `struct Params` declarava
/// 28 campos (112 bytes, múltiplo de 16) ninguém precisava saber disto.
///
/// Os 8 bytes de sobra nunca são escritos nem lidos: a CPU manda os 184 do
/// `bytemuck::bytes_of`, e o shader não tem campo além do 45.
const TAMANHO_DO_UNIFORM: wgpu::BufferAddress = {
    let bytes = std::mem::size_of::<Ajustes>() as wgpu::BufferAddress;
    bytes.next_multiple_of(16)
};

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

impl Ajustes {
    /// A revelação que a foto tem gravada.
    ///
    /// 🔑 **O padrão de campo ausente vem de [`Ajustes::default`], campo a
    /// campo, e não de 46 números digitados aqui.** O legado escrevia
    /// `photo.edit_exposure().unwrap_or(0.0)` quarenta e seis vezes, com o
    /// neutro repetido em cada linha — e dois deles não são zero (`contrast` e
    /// `sharpen_radius`). Um `unwrap_or` errado no contraste achataria a foto
    /// inteira em cinza, e a suspeita cairia no motor de cor, não na leitura.
    ///
    /// ⚠️ **`Some(0.0)` no contraste é o fotógrafo tendo arrastado até o fim**, e
    /// não pode ser confundido com ausência — que é por que isto é `if let
    /// Some`, e não `unwrap_or_default`.
    pub fn da_entidade(foto: &domain::entities::Photo) -> Self {
        let mut ajustes = Self::default();

        macro_rules! ler {
            ($($campo:ident <- $salvo:ident),* $(,)?) => {
                $(if let Some(valor) = foto.$salvo() {
                    ajustes.$campo = valor;
                })*
            };
        }

        ler! {
            exposure <- edit_exposure,
            contrast <- edit_contrast,
            temperature <- edit_temperature,
            tint <- edit_tint,
            highlights <- edit_highlights,
            shadows <- edit_shadows,
            whites <- edit_whites,
            blacks <- edit_blacks,
            clarity <- edit_clarity,
            vibrance <- edit_vibrance,
            saturation <- edit_saturation,
            tone_curve_shadows <- edit_tone_curve_shadows,
            tone_curve_darks <- edit_tone_curve_darks,
            tone_curve_lights <- edit_tone_curve_lights,
            tone_curve_highlights <- edit_tone_curve_highlights,
            hsl_red_sat <- edit_hsl_red_sat,
            hsl_orange_sat <- edit_hsl_orange_sat,
            hsl_yellow_sat <- edit_hsl_yellow_sat,
            hsl_green_sat <- edit_hsl_green_sat,
            hsl_aqua_sat <- edit_hsl_aqua_sat,
            hsl_blue_sat <- edit_hsl_blue_sat,
            hsl_purple_sat <- edit_hsl_purple_sat,
            hsl_magenta_sat <- edit_hsl_magenta_sat,
            hsl_red_hue <- edit_hsl_red_hue,
            hsl_orange_hue <- edit_hsl_orange_hue,
            hsl_yellow_hue <- edit_hsl_yellow_hue,
            hsl_green_hue <- edit_hsl_green_hue,
            hsl_aqua_hue <- edit_hsl_aqua_hue,
            hsl_blue_hue <- edit_hsl_blue_hue,
            hsl_purple_hue <- edit_hsl_purple_hue,
            hsl_magenta_hue <- edit_hsl_magenta_hue,
            hsl_red_lum <- edit_hsl_red_lum,
            hsl_orange_lum <- edit_hsl_orange_lum,
            hsl_yellow_lum <- edit_hsl_yellow_lum,
            hsl_green_lum <- edit_hsl_green_lum,
            hsl_aqua_lum <- edit_hsl_aqua_lum,
            hsl_blue_lum <- edit_hsl_blue_lum,
            hsl_purple_lum <- edit_hsl_purple_lum,
            hsl_magenta_lum <- edit_hsl_magenta_lum,
            lens_distortion <- edit_lens_distortion,
            lens_vignette_amount <- edit_lens_vignette_amount,
            lens_vignette_midpoint <- edit_lens_vignette_midpoint,
            nr_luminance <- edit_nr_luminance,
            nr_color <- edit_nr_color,
            sharpen_amount <- edit_sharpen_amount,
            sharpen_radius <- edit_sharpen_radius,
        }

        ajustes
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
        size: TAMANHO_DO_UNIFORM,
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
/// O dispositivo, o pipeline e o cache de recursos — abertos uma vez.
///
/// ⚠️ **Abrir custa**: `request_adapter` e `request_device` são assíncronos e
/// levam dezenas de milissegundos. Quem revela abre um na thread de fundo e o
/// mantém pela sessão inteira; quem exporta abre um por lote, não por foto.
pub struct Motor {
    dispositivo: wgpu::Device,
    fila: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    /// Recursos por tamanho de imagem — ver [`Recursos`].
    cache: LruCache<(u32, u32), Recursos>,
}

impl Motor {
    /// `None` quando não há adaptador de GPU.
    ///
    /// 🚨 **E aí não há caminho de CPU para cair.** O `crates/ui` tinha um —
    /// uma segunda implementação da mesma matemática, com resultado diferente
    /// do shader — e ele não veio junto de propósito: um motor que responde
    /// "certo por outro caminho" é pior que um que responde "não sei".
    ///
    /// Quem chama decide o que fazer com o `None`: a Revelação mostra a foto sem
    /// ajuste, e a exportação **falha**, porque gravar arquivo com os ajustes
    /// descartados em silêncio é o defeito que ela acabou de deixar de ter.
    pub fn abrir() -> Option<Self> {
        let instancia = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adaptador =
            pollster::block_on(instancia.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))?;

        let (dispositivo, fila) = pollster::block_on(adaptador.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("VintageLightbox GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;

        // 🚨 O `struct Params` do WGSL tem de casar com o `Ajustes`, campo a
        // campo: o `uniform` viaja como bytes crus e liga por **posição**, não
        // por nome. Quem prende isso é
        // `o_wgsl_declara_os_mesmos_46_campos_na_mesma_ordem`.
        let modulo = dispositivo.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Adjustments Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image_adjustments.wgsl").into()),
        });

        let pipeline = dispositivo.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Image Processing Pipeline"),
            layout: None,
            module: &modulo,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Some(Self {
            dispositivo,
            fila,
            pipeline,
            cache: LruCache::new(NonZeroUsize::new(5).expect("5 não é zero")),
        })
    }

    /// Uma passada: sobe o que mudou, despacha o compute, lê de volta.
    pub fn revelar(
        &mut self,
        pixels: &Arc<Vec<u8>>,
        largura: u32,
        altura: u32,
        ajustes: &Ajustes,
    ) -> Option<DynamicImage> {
        let (dispositivo, fila, pipeline) = (&self.dispositivo, &self.fila, &self.pipeline);
        let cache = &mut self.cache;

        if !cache.contains(&(largura, altura)) {
            cache.put(
                (largura, altura),
                criar_recursos(dispositivo, pipeline, largura, altura),
            );
        }
        let recursos = cache.get_mut(&(largura, altura))?;

        let precisa_subir = match &recursos.ultimos_pixels {
            Some(ultimos) => !Arc::ptr_eq(ultimos, pixels),
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
                pixels,
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
            recursos.ultimos_pixels = Some(pixels.clone());
        }

        fila.write_buffer(&recursos.buffer_ajustes, 0, bytemuck::bytes_of(ajustes));

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
        let mut saida = Vec::with_capacity((recursos.bytes_por_linha * altura) as usize);
        for y in 0..altura {
            let inicio = (y * recursos.bytes_por_linha_alinhado) as usize;
            saida.extend_from_slice(&dados[inicio..inicio + recursos.bytes_por_linha as usize]);
        }
        drop(dados);
        recursos.buffer_saida.unmap();

        Some(DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
            largura, altura, saida,
        )?))
    }
}
#[cfg(test)]
mod testes {
    use super::*;

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
    fn motor_pronto() -> Motor {
        Motor::abrir().expect("nenhum adaptador de GPU — o motor de revelação não roda aqui")
    }

    fn cinza(lado: u32, valor: u8) -> Arc<Vec<u8>> {
        Arc::new(
            std::iter::repeat_n([valor, valor, valor, 255], (lado * lado) as usize)
                .flatten()
                .collect(),
        )
    }

    fn revelar_e_colher(motor: &mut Motor, entrada: Arc<Vec<u8>>, ajustes: Ajustes) -> Vec<u8> {
        motor
            .revelar(&entrada, 16, 16, &ajustes)
            .expect("o motor não devolveu imagem")
            .into_rgba8()
            .into_raw()
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
        let mut motor = motor_pronto();
        let entrada = cinza(16, 100);
        let saida = revelar_e_colher(&mut motor, entrada.clone(), Ajustes::default());

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
        let mut motor = motor_pronto();
        let saida = revelar_e_colher(
            &mut motor,
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
        let shader = include_str!("shaders/image_adjustments.wgsl");
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

    /// O `struct Params` do WGSL declara os mesmos 46 campos do `Ajustes`, na
    /// mesma ordem.
    ///
    /// 🚨 **Este teste substitui um que prendia o defeito oposto.** Até 17/ago o
    /// WGSL declarava **28** campos para os 46 que a CPU manda, e como o
    /// `uniform` chega por **posição** e não por nome, a partir do 23 o shader
    /// lia o campo do vizinho:
    ///
    /// | posição | o Rust mandava | o shader lia como | o usuário via |
    /// |--------:|----------------|-------------------|---------------|
    /// | 23 | `hsl_red_hue` | `nr_luminance` | a foto **borrava** |
    /// | 24 | `hsl_orange_hue` | `nr_luminance` de novo — declarado duas vezes | nada |
    /// | 25 | `hsl_yellow_hue` | `nr_color` | tirava ruído de cor |
    /// | 26 | `hsl_green_hue` | `sharpen_amount` | afiava |
    /// | 27 | `hsl_aqua_hue` | `sharpen_radius` | nada sozinho |
    /// | 28–45 | matiz (3), luminância (8), lente (3), Detalhe (4) | **nada** | nada |
    ///
    /// Nada disso falhava: o buffer é maior que o mínimo que o binding exige,
    /// então o wgpu aceita e ignora a sobra, e a duplicata de `nr_luminance` no
    /// WGSL o naga também aceita. Não havia erro, log nem tela quebrada — havia
    /// um controle que responde e uma foto que muda pelo motivo errado.
    ///
    /// 🔑 **A conferência é por leitura do arquivo, e não por medida na imagem**,
    /// porque campo declarado e campo aplicado são coisas diferentes: quem mede
    /// a segunda é `os_dezenove_ajustes_sem_codigo_no_shader_nao_mudam_nenhum_pixel`.
    #[test]
    fn o_wgsl_declara_os_mesmos_46_campos_na_mesma_ordem() {
        assert_eq!(
            campos_do_wgsl(),
            NOMES,
            "o `struct Params` do WGSL divergiu do `Ajustes` — e o `uniform` casa por posição"
        );
    }

    /// Os 23 primeiros ajustes e os 4 de Detalhe mudam a foto.
    ///
    /// ✅ **O Detalhe é o que o alinhamento de 17/ago devolveu**: `nr_luminance`,
    /// `nr_color` e `sharpen_amount` sempre tiveram código no corpo do shader —
    /// o que faltava era chegarem lá. Eram quatro sliders que arrastavam,
    /// mostravam número e não moviam um pixel.
    ///
    /// ⚠️ **`sharpen_radius` só conta com `sharpen_amount` junto**: raio sozinho
    /// nunca mudaria nada (`do_sharpen` é `amount > 0.0`), e o teste passaria por
    /// engano ao afirmar o contrário.
    #[test]
    fn o_basico_e_o_detalhe_chegam_ao_shader() {
        let mut motor = motor_pronto();
        let entrada = amostra();
        let neutro = revelar_e_colher(&mut motor, entrada.clone(), Ajustes::default());

        for (i, nome) in NOMES.iter().enumerate().take(23) {
            let saida = revelar_e_colher(&mut motor, entrada.clone(), com_campo(i, 60.0));
            assert_ne!(
                saida, neutro,
                "`{nome}` (campo {i}) devia chegar ao shader e não mudou nada"
            );
        }

        let detalhe: [(&str, &[(usize, f32)]); 3] = [
            ("Ruído (luminância)", &[(42, 60.0)]),
            ("Ruído (cor)", &[(43, 60.0)]),
            ("Nitidez (com raio)", &[(44, 80.0), (45, 2.0)]),
        ];
        for (rotulo, campos) in detalhe {
            let saida = revelar_e_colher(&mut motor, entrada.clone(), com_campos(campos));
            assert_ne!(saida, neutro, "Detalhe — `{rotulo}` não fez efeito nenhum");
        }
    }

    /// ⚠️ **Três ajustes chegam ao shader e não têm código que os use** — a
    /// Lente: distorção, vinheta e o meio dela.
    ///
    /// 🔑 **Eram dezenove.** O matiz (8) e a luminância (8) do HSL entraram em
    /// 17/ago/2026; a Lente ficou porque não é o mesmo trabalho: matiz e
    /// luminância entram no bloco de HSL que já existia, e a distorção precisa
    /// **reamostrar coordenada**, que muda como o shader lê a textura.
    ///
    /// Este teste **tem de falhar** no dia em que a Lente entrar, e some com ele.
    #[test]
    fn a_lente_ainda_nao_tem_codigo_no_shader() {
        let mut motor = motor_pronto();
        let entrada = amostra();
        let neutro = revelar_e_colher(&mut motor, entrada.clone(), Ajustes::default());

        let saida = revelar_e_colher(&mut motor, entrada.clone(), com_campo(39, 60.0));
        assert_eq!(saida, neutro, "Lente — a distorção passou a fazer efeito");

        // A vinheta com o meio junto: o meio sozinho nunca faria efeito, e o
        // teste passaria por engano.
        let saida = revelar_e_colher(&mut motor, entrada, com_campos(&[(40, 80.0), (41, 30.0)]));
        assert_eq!(saida, neutro, "Lente — a vinheta passou a fazer efeito");
    }

    /// ✅ **Os 16 sliders de matiz e luminância do HSL movem a foto.**
    ///
    /// Eles existiam no painel desde sempre e nunca aplicaram nada: primeiro
    /// porque o `uniform` do shader declarava 28 campos e eles ficavam de fora,
    /// depois — já alinhados — porque o corpo do shader não os mencionava.
    ///
    /// A amostra tem as oito cores do HSL, então cada canal tem onde agir.
    #[test]
    fn o_matiz_e_a_luminancia_do_hsl_movem_a_foto() {
        let mut motor = motor_pronto();
        let entrada = amostra();
        let neutro = revelar_e_colher(&mut motor, entrada.clone(), Ajustes::default());

        // 23–30 é matiz, 31–38 é luminância — um canal por posição.
        for (i, nome) in NOMES.iter().enumerate().take(39).skip(23) {
            let saida = revelar_e_colher(&mut motor, entrada.clone(), com_campo(i, 60.0));
            assert_ne!(
                saida, neutro,
                "`{nome}` (campo {i}) chega ao shader e não moveu um pixel"
            );
        }
    }

    /// 🚨 **A luminância do HSL não pode clarear cinza.**
    ///
    /// Um pixel cinza não tem matiz: `delta == 0` dá `hue == 0`, que cai na
    /// faixa do **vermelho** com peso 1.0. Sem o portão da saturação, arrastar
    /// "HSL / luminância — Vermelho" clarearia **toda** área neutra da foto — e
    /// o slider viraria, calado, um controle global de brilho.
    ///
    /// Nada falharia: a foto muda, o controle responde, e o que mudou não tem
    /// relação com o rótulo. É a mesma família do matiz que borrava.
    #[test]
    fn a_luminancia_do_vermelho_nao_mexe_no_cinza() {
        let mut motor = motor_pronto();
        let cinza_puro = cinza(16, 128);

        let neutro = revelar_e_colher(&mut motor, cinza_puro.clone(), Ajustes::default());
        let com_lum = revelar_e_colher(
            &mut motor,
            cinza_puro,
            Ajustes {
                hsl_red_lum: 100.0,
                ..Default::default()
            },
        );

        assert_eq!(
            com_lum, neutro,
            "o cinza não tem matiz — a luminância do vermelho não pode tocá-lo"
        );
    }

    /// 🔑 **O matiz gira a cor; ele não mexe no contraste entre vizinhos.**
    ///
    /// É a mesma medida que acusou o defeito antigo, agora do lado certo: quando
    /// o campo 23 era lido como `nr_luminance`, o contraste local **caía** —
    /// assinatura de borrão. Girando de verdade, ele fica onde estava.
    #[test]
    fn o_matiz_gira_a_cor_sem_borrar() {
        let mut motor = motor_pronto();
        let entrada = amostra();

        let neutro = revelar_e_colher(&mut motor, entrada.clone(), Ajustes::default());
        let girado = revelar_e_colher(
            &mut motor,
            entrada,
            Ajustes {
                hsl_red_hue: 120.0,
                ..Default::default()
            },
        );

        assert_ne!(girado, neutro, "o matiz do vermelho não moveu nada");
        let (antes, depois) = (contraste_local(&neutro), contraste_local(&girado));
        // Girar matiz mexe em cor, não em detalhe: a soma de diferenças entre
        // vizinhos fica na mesma ordem de grandeza. Um borrão a derrubaria.
        assert!(
            depois > antes / 2,
            "o contraste local caiu de {antes} para {depois} — isso é borrão, não giro de matiz"
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
        let mut motor = motor_pronto();
        let entrada = amostra();

        let cinza_de_verdade = revelar_e_colher(
            &mut motor,
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
            &mut motor,
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

    /// O layout que vai para a GPU tem os 46 campos que o `crates/ui` mandava.
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
