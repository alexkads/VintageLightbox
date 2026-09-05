//! A superfície do canvas e o renderizador do egui sobre o wgpu.
//!
//! O egui entrega a cada quadro uma lista de malhas (`ClippedPrimitive`) e as
//! texturas que mudaram; o `egui_wgpu::Renderer` desenha isso. Não há pipeline
//! nosso: a grade de fotos, a barra, os painéis e os diálogos são todos malhas
//! do egui, e as miniaturas são texturas que o egui gerencia.
//!
//! O que sobra de "nosso" aqui é o que já existia no motor de revelação: abrir
//! WebGPU antes de tocar no canvas, e cair para WebGL2 sem consumi-lo
//! (armadilha nº 56 do projeto).

use egui_wgpu::ScreenDescriptor;

pub struct Superficie {
    dispositivo: wgpu::Device,
    fila: wgpu::Queue,
    superficie: wgpu::Surface<'static>,
    formato: wgpu::TextureFormat,
    renderer: egui_wgpu::Renderer,
    largura: u32,
    altura: u32,
    backend: &'static str,
}

fn erro(mensagem: impl Into<String>) -> String {
    mensagem.into()
}

impl Superficie {
    pub async fn abrir(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String> {
        let sem_superficie = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        };

        // 1. WebGPU: a pergunta **não** toca no canvas, e por isso vem antes.
        let instancia = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let webgpu = instancia.request_adapter(&sem_superficie).await;

        let (adaptador, superficie, backend) = match webgpu {
            Some(adaptador) => {
                let superficie = instancia
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                    .map_err(|e| erro(format!("o canvas não virou superfície (webgpu): {e}")))?;
                (adaptador, superficie, "webgpu")
            }
            // 2. Não respondeu: instância nova, só GL — e aqui a surface vem
            //    antes, porque o backend WebGL2 nasce de um canvas.
            None => {
                let instancia = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::GL,
                    ..Default::default()
                });
                let superficie = instancia
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                    .map_err(|e| erro(format!("o canvas não virou superfície (webgl): {e}")))?;
                let adaptador = instancia
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&superficie),
                        ..sem_superficie
                    })
                    .await
                    .ok_or_else(|| erro("nenhum adaptador de GPU: nem WebGPU nem WebGL2"))?;
                (adaptador, superficie, "webgl")
            }
        };

        let info = adaptador.get_info();
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
            "[Biblioteca] adaptador: {} ({backend}, {:?})",
            info.name, info.device_type
        )));

        let (dispositivo, fila) = adaptador
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("biblioteca"),
                    required_features: wgpu::Features::empty(),
                    required_limits: adaptador.limits(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| {
                erro(format!(
                    "o adaptador respondeu, mas o dispositivo não abriu: {e}"
                ))
            })?;

        let capacidades = superficie.get_capabilities(&adaptador);
        // Sem sRGB, como o motor de revelação: o byte na tela é o byte da
        // miniatura, que já chega no espaço em que será mostrada.
        let formato = capacidades
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| capacidades.formats.first().copied())
            .ok_or_else(|| erro("a superfície não ofereceu formato nenhum"))?;

        let renderer = egui_wgpu::Renderer::new(&dispositivo, formato, None, 1, false);

        Ok(Self {
            dispositivo,
            fila,
            superficie,
            formato,
            renderer,
            largura: 0,
            altura: 0,
            backend,
        })
    }

    pub fn backend(&self) -> &'static str {
        self.backend
    }

    pub fn redimensionar(&mut self, largura: u32, altura: u32) {
        if largura == 0 || altura == 0 || (largura == self.largura && altura == self.altura) {
            return;
        }
        self.largura = largura;
        self.altura = altura;
        self.configurar();
    }

    fn configurar(&mut self) {
        self.superficie.configure(
            &self.dispositivo,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.formato,
                width: self.largura,
                height: self.altura,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );
    }

    /// Desenha um quadro do egui. `false` quando a superfície não está pronta
    /// (tamanho zero, ou perdida ao voltar de segundo plano).
    pub fn desenhar(
        &mut self,
        ctx: &egui::Context,
        saida: egui::FullOutput,
        fundo: egui::Color32,
    ) -> bool {
        if self.largura == 0 || self.altura == 0 {
            return false;
        }
        let quadro = match self.superficie.get_current_texture() {
            Ok(q) => q,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.configurar();
                return false;
            }
            Err(_) => return false,
        };
        let vista = quadro
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let ppp = saida.pixels_per_point;
        let tela = ScreenDescriptor {
            size_in_pixels: [self.largura, self.altura],
            pixels_per_point: ppp,
        };

        for (id, delta) in &saida.textures_delta.set {
            self.renderer
                .update_texture(&self.dispositivo, &self.fila, *id, delta);
        }

        let malhas = ctx.tessellate(saida.shapes, ppp);

        let mut codificador =
            self.dispositivo
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui"),
                });
        let extras = self.renderer.update_buffers(
            &self.dispositivo,
            &self.fila,
            &mut codificador,
            &malhas,
            &tela,
        );
        {
            let mut passo = codificador
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &vista,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(fundo.r()) / 255.0,
                                g: f64::from(fundo.g()) / 255.0,
                                b: f64::from(fundo.b()) / 255.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut passo, &malhas, &tela);
        }

        self.fila
            .submit(extras.into_iter().chain(Some(codificador.finish())));
        quadro.present();

        for id in &saida.textures_delta.free {
            self.renderer.free_texture(id);
        }
        true
    }
}
