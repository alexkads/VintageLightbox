//! As amostras do seletor de preset: a mesma foto revelada com cada preset
//! (`amostra-de-presets/` do site; dono, 2026-09-13: *"os presets deveriam ter
//! uma amostra"*).
//!
//! A foto é a primeira do rascunho que tem prévia; sem nenhuma, a capa do
//! estúdio que vem com o app. A revelação é do motor, numa thread só dela,
//! em miniatura — o quadro do cartão recorta no centro, na proporção do corte
//! escolhido, que é o mesmo retângulo que o corte centralizado grava.
//!
//! 🔑 **Sem GPU, a amostra é a foto como veio**: o cartão continua mostrando
//! uma imagem, e o nome diz qual é o preset.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use gpui::RenderImage;
use image::DynamicImage;
use infrastructure::gpu_adjustments::Ajustes;

use crate::imagem::para_gpui;

/// O lado maior da foto revelada nas amostras.
const LADO: u32 = 256;

struct Pedido {
    chave: String,
    base: Arc<Base>,
    ajustes: Ajustes,
}

struct Base {
    pixels: Arc<Vec<u8>>,
    largura: u32,
    altura: u32,
}

/// O canal por onde o motor devolve `(chave, imagem)`.
type Respostas = (
    Sender<(String, DynamicImage)>,
    Receiver<(String, DynamicImage)>,
);

pub struct Amostras {
    /// De onde a base saiu (`None` = a capa embutida).
    origem: Option<Option<String>>,
    base: Option<Arc<Base>>,
    prontas: HashMap<String, Arc<RenderImage>>,
    pedidas: std::collections::HashSet<String>,
    pedidos: Option<Sender<Pedido>>,
    respostas: Respostas,
}

impl Default for Amostras {
    fn default() -> Self {
        Self {
            origem: None,
            base: None,
            prontas: HashMap::new(),
            pedidas: Default::default(),
            pedidos: None,
            respostas: channel(),
        }
    }
}

impl Amostras {
    /// Troca a foto das amostras quando a primeira do rascunho muda.
    pub fn definir_base(
        &mut self,
        origem: Option<String>,
        imagem: impl FnOnce() -> Option<DynamicImage>,
    ) {
        if self.origem.as_ref() == Some(&origem) {
            return;
        }
        let imagem = imagem().or_else(|| {
            crate::recursos::imagem("capa-canela.jpeg")
                .and_then(|bytes| image::load_from_memory(bytes).ok())
        });
        let Some(imagem) = imagem else {
            return;
        };
        let pequena = imagem.thumbnail(LADO, LADO).to_rgba8();
        let (largura, altura) = pequena.dimensions();
        self.origem = Some(origem);
        self.base = Some(Arc::new(Base {
            pixels: Arc::new(pequena.into_raw()),
            largura,
            altura,
        }));
        self.prontas.clear();
        self.pedidas.clear();
    }

    /// A amostra deste preset — pedida ao motor na primeira vez.
    pub fn obter(&mut self, chave: &str, ajustes: Ajustes) -> Option<Arc<RenderImage>> {
        self.colher();
        if let Some(pronta) = self.prontas.get(chave) {
            return Some(pronta.clone());
        }
        let base = self.base.clone()?;
        if self.pedidas.insert(chave.to_string()) {
            if ajustes == Ajustes::default() {
                // "Nenhum" é a foto como veio: não precisa do motor.
                let imagem =
                    image::RgbaImage::from_raw(base.largura, base.altura, (*base.pixels).clone())
                        .map(DynamicImage::ImageRgba8)?;
                let pronta = para_gpui(imagem);
                self.prontas.insert(chave.to_string(), pronta.clone());
                return Some(pronta);
            }
            let pedido = Pedido {
                chave: chave.to_string(),
                base,
                ajustes,
            };
            let _ = self.motor().send(pedido);
        }
        None
    }

    /// Se ainda há amostra a caminho (a tela continua acordando).
    pub fn esperando(&self) -> bool {
        self.pedidas.len() > self.prontas.len()
    }

    /// Recolhe o que o motor terminou. Devolve se chegou alguma.
    pub fn colher(&mut self) -> bool {
        let mut chegou = false;
        while let Ok((chave, imagem)) = self.respostas.1.try_recv() {
            self.prontas.insert(chave, para_gpui(imagem));
            chegou = true;
        }
        chegou
    }

    fn motor(&mut self) -> &Sender<Pedido> {
        self.pedidos.get_or_insert_with(|| {
            let (envia, recebe) = channel::<Pedido>();
            let respostas = self.respostas.0.clone();
            std::thread::Builder::new()
                .name("amostras-de-preset".into())
                .spawn(move || laco(recebe, respostas))
                .expect("abrir a thread das amostras");
            envia
        })
    }
}

fn laco(pedidos: Receiver<Pedido>, respostas: Sender<(String, DynamicImage)>) {
    #[cfg(not(test))]
    let mut motor = infrastructure::gpu_adjustments::Motor::abrir();
    #[cfg(test)]
    let mut motor: Option<infrastructure::gpu_adjustments::Motor> = None;
    while let Ok(pedido) = pedidos.recv() {
        let revelada = motor.as_mut().and_then(|m| {
            m.revelar(
                &pedido.base.pixels,
                pedido.base.largura,
                pedido.base.altura,
                &pedido.ajustes,
            )
        });
        let imagem = revelada.or_else(|| {
            image::RgbaImage::from_raw(
                pedido.base.largura,
                pedido.base.altura,
                (*pedido.base.pixels).clone(),
            )
            .map(DynamicImage::ImageRgba8)
        });
        if let Some(imagem) = imagem {
            if respostas.send((pedido.chave, imagem)).is_err() {
                return;
            }
        }
    }
}
