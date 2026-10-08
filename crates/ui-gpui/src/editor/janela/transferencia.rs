//! 📋 A área de transferência do editor (etapa 16): ⌘C, ⇧⌘C (copiar
//! mesclado), ⌘X, ⌘V (no meio da vista, ou da seleção), ⇧⌘V (no lugar), o
//! "Carimbar visível" (⇧⌥⌘E) e o "Importar imagem como camada…".
//!
//! 🔑 **Duas áreas, como no Photoshop**: a do editor guarda os pixels com o
//! lugar de onde saíram (o "Colar no lugar" precisa dele), e a do sistema
//! recebe um PNG, para colar em outro programa. Ao colar, uma imagem do
//! sistema que **não** é a que o editor pôs lá (outro programa copiou depois)
//! vence — e entra no meio da vista.

use editor_core::transformar::Conteudo;
use gpui_kit::{ClipboardEntry, ClipboardItem, Context, Image, ImageFormat, SharedString};

use super::EditorDeFoto;

/// O que o editor copiou por último.
pub(super) struct Copiado {
    conteudo: Conteudo,
    /// O `id` da imagem que foi para a área do sistema (`None` enquanto o PNG
    /// é feito em segundo plano).
    no_sistema: Option<u64>,
}

impl Copiado {
    /// A caixa e se já foi para o sistema — o estado do roteiro.
    pub(super) fn descricao(&self) -> String {
        let c = self.conteudo.caixa;
        format!(
            "({}, {}) {}×{} sistema={}",
            c.x,
            c.y,
            c.largura,
            c.altura,
            self.no_sistema.is_some()
        )
    }
}

/// O maior lado de uma imagem colada ou importada maior que a foto: ela é
/// reduzida para caber, como o "Colocar" do Photoshop.
fn caber(imagem: image::RgbaImage, largura: u32, altura: u32) -> image::RgbaImage {
    let (l, a) = imagem.dimensions();
    if l <= largura && a <= altura {
        return imagem;
    }
    let escala = (largura as f32 / l as f32).min(altura as f32 / a as f32);
    let (nl, na) = (
        ((l as f32 * escala).round() as u32).max(1),
        ((a as f32 * escala).round() as u32).max(1),
    );
    image::imageops::resize(&imagem, nl, na, image::imageops::FilterType::Lanczos3)
}

impl EditorDeFoto {
    /// ⌘C (ou ⇧⌘C, `mesclado`).
    pub fn copiar(&mut self, mesclado: bool, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        match s.copiar(mesclado) {
            Some(conteudo) => {
                let texto = if mesclado {
                    "Copiado (mesclado)"
                } else {
                    "Copiado"
                };
                self.guardar_copiado(conteudo, cx);
                self.aviso = Some((texto.into(), false));
            }
            None => {
                self.aviso = Some((
                    "Nada para copiar: a seleção está vazia, ou a camada não tem pixels".into(),
                    true,
                ))
            }
        }
        cx.notify();
    }

    /// ⌘X: copia e apaga.
    pub fn recortar(&mut self, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let bloqueada = s.pixels_bloqueados();
        match s.recortar() {
            Some(conteudo) => {
                self.guardar_copiado(conteudo, cx);
                self.aviso = None;
            }
            None if bloqueada => self.avisar_cadeado("recortar", cx),
            None => {
                self.aviso = Some((
                    "Nada para recortar: a seleção está vazia, ou a camada não tem pixels".into(),
                    true,
                ))
            }
        }
        cx.notify();
    }

    fn guardar_copiado(&mut self, conteudo: Conteudo, cx: &mut Context<Self>) {
        let imagem = conteudo.imagem();
        self.copiado = Some(Copiado {
            conteudo,
            no_sistema: None,
        });
        // O PNG para os outros programas sai em segundo plano: uma camada
        // inteira de 24 MP leva mais de um segundo.
        let trabalho = cx.background_executor().spawn(async move {
            let mut png = Vec::new();
            let codificador = image::codecs::png::PngEncoder::new_with_quality(
                &mut png,
                image::codecs::png::CompressionType::Fast,
                image::codecs::png::FilterType::Adaptive,
            );
            image::ImageEncoder::write_image(
                codificador,
                imagem.as_raw(),
                imagem.width(),
                imagem.height(),
                image::ExtendedColorType::Rgba8,
            )
            .ok()
            .map(|_| png)
        });
        self._tarefa_da_transferencia = Some(cx.spawn(async move |esta, cx| {
            let Some(png) = trabalho.await else {
                return;
            };
            let _ = esta.update(cx, |ed, cx| {
                let imagem = Image::from_bytes(ImageFormat::Png, png);
                let id = imagem.id();
                cx.write_to_clipboard(ClipboardItem::new_image(&imagem));
                if let Some(c) = ed.copiado.as_mut() {
                    c.no_sistema = Some(id);
                }
            });
        }));
    }

    /// ⌘V (`no_lugar = false`) e ⇧⌘V.
    pub fn colar(&mut self, no_lugar: bool, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() || self.sessao().is_none() {
            return;
        }
        let de_fora = cx.read_from_clipboard().and_then(|item| {
            item.entries().iter().find_map(|e| match e {
                ClipboardEntry::Image(imagem) => Some(imagem.clone()),
                _ => None,
            })
        });
        let pendente = self
            .copiado
            .as_ref()
            .is_some_and(|c| c.no_sistema.is_none());
        let nosso = self.copiado.as_ref().and_then(|c| c.no_sistema);
        match de_fora {
            Some(imagem) if !pendente && Some(imagem.id()) != nosso => {
                self.colar_de_fora(imagem, cx);
            }
            _ => {
                let Some(conteudo) = self.copiado.as_ref().map(|c| c.conteudo.clone()) else {
                    self.aviso = Some(("Nada para colar".into(), true));
                    cx.notify();
                    return;
                };
                let centro = (!no_lugar).then(|| self.centro_para_colar()).flatten();
                self.na_sessao(cx, |s| {
                    s.colar(&conteudo, centro);
                });
            }
        }
    }

    /// Onde o ⌘V põe o meio do que cola: o meio da seleção, ou o da vista.
    fn centro_para_colar(&self) -> Option<(f32, f32)> {
        let s = self.sessao()?;
        if let Some(sel) = s.selecao() {
            let c = sel.caixa_justa();
            if !c.vazio() {
                return Some((
                    c.x as f32 + c.largura as f32 / 2.0,
                    c.y as f32 + c.altura as f32 / 2.0,
                ));
            }
        }
        self.na_foto_sem_limite(self.palco.center())
    }

    /// Uma imagem de outro programa: decodificada em segundo plano, reduzida
    /// se for maior que a foto, e colada no meio da vista.
    fn colar_de_fora(&mut self, imagem: Image, cx: &mut Context<Self>) {
        let Some((largura, altura)) = self
            .sessao()
            .map(|s| (s.documento().largura(), s.documento().altura()))
        else {
            return;
        };
        self.aviso = Some(("Colando a imagem…".into(), false));
        cx.notify();
        let trabalho = cx.background_executor().spawn(async move {
            image::load_from_memory(imagem.bytes())
                .map(|i| caber(i.to_rgba8(), largura, altura))
                .map_err(|e| e.to_string())
        });
        self._tarefa_da_transferencia = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| match resultado {
                Ok(img) => {
                    let conteudo = Conteudo::da_imagem(&img, 0, 0);
                    let centro = ed.centro_para_colar();
                    ed.na_sessao(cx, |s| {
                        s.colar(&conteudo, centro);
                    });
                }
                Err(erro) => {
                    ed.aviso = Some((format!("Não deu para colar a imagem: {erro}").into(), true));
                    cx.notify();
                }
            });
        }));
    }

    /// ⇧⌥⌘E: a foto como aparece numa camada nova — composta em segundo
    /// plano, a partir do documento de agora.
    pub fn carimbar_visivel(&mut self, cx: &mut Context<Self>) {
        if self.carimbando || self.area_do_preenchimento.is_some() {
            return;
        }
        let Some((versao, base, doc)) = self.sessao().map(|s| s.pedido_de_carimbo()) else {
            return;
        };
        self.carimbando = true;
        self.aviso = Some(("Carimbando o visível…".into(), false));
        cx.notify();
        let trabalho = cx.background_executor().spawn(async move {
            let foto = editor_core::composicao::compor(&base, &doc);
            editor_core::CamadaDePixels::da_imagem(&foto)
        });
        self._tarefa_do_carimbo = Some(cx.spawn(async move |esta, cx| {
            let pixels = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.carimbando = false;
                let feito = ed
                    .sessao_mut()
                    .is_some_and(|s| s.carimbar_visivel(versao, pixels));
                ed.aviso = (!feito).then(|| {
                    (
                        SharedString::from(
                            "A foto mudou enquanto o carimbo era feito — repita ⇧⌥⌘E",
                        ),
                        true,
                    )
                });
                cx.notify();
            });
        }));
    }

    pub fn carimbando(&self) -> bool {
        self.carimbando
    }

    /// "Importar imagem como camada…": o arquivo escolhido entra numa camada
    /// nova com o nome dele, no meio da vista (reduzido se for maior que a
    /// foto).
    pub fn importar_imagem(&mut self, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() || self.sessao().is_none() {
            return;
        }
        let escolha = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar como camada".into()),
        });
        self._tarefa_da_transferencia = Some(cx.spawn(async move |esta, cx| {
            let Ok(Ok(Some(caminhos))) = escolha.await else {
                return;
            };
            let Some(caminho) = caminhos.into_iter().next() else {
                return;
            };
            let _ = esta.update(cx, |ed, cx| ed.importar_arquivo(caminho, cx));
        }));
    }

    /// O arquivo `caminho` como camada nova (o passo do roteiro também).
    pub fn importar_arquivo(&mut self, caminho: std::path::PathBuf, cx: &mut Context<Self>) {
        let Some((largura, altura)) = self
            .sessao()
            .map(|s| (s.documento().largura(), s.documento().altura()))
        else {
            return;
        };
        let nome = caminho
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imagem".into());
        self.aviso = Some((format!("Importando {nome}…").into(), false));
        cx.notify();
        let trabalho = cx.background_executor().spawn(async move {
            image::open(&caminho)
                .map(|i| caber(i.to_rgba8(), largura, altura))
                .map_err(|e| e.to_string())
        });
        self._tarefa_da_transferencia = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| match resultado {
                Ok(img) => {
                    let conteudo = Conteudo::da_imagem(&img, 0, 0);
                    let centro = ed.na_foto_sem_limite(ed.palco.center());
                    ed.na_sessao(cx, |s| {
                        s.colar_com_nome(&conteudo, centro, &nome, "Importar imagem");
                    });
                }
                Err(erro) => {
                    ed.aviso = Some((format!("Não deu para abrir a imagem: {erro}").into(), true));
                    cx.notify();
                }
            });
        }));
    }
}

#[cfg(test)]
mod testes {
    use super::caber;

    #[test]
    fn a_imagem_maior_que_a_foto_cabe_nela_sem_mudar_a_proporcao() {
        let grande = image::RgbaImage::new(4000, 1000);
        let cabida = caber(grande, 800, 600);
        assert_eq!(cabida.dimensions(), (800, 200));
        let pequena = image::RgbaImage::new(100, 50);
        assert_eq!(caber(pequena, 800, 600).dimensions(), (100, 50));
    }
}
