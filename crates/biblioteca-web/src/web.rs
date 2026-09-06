//! A fronteira com o JavaScript — e ela é curta de propósito.
//!
//! ## O que fica de cada lado
//!
//! | lado | faz |
//! |---|---|
//! | React (`grade-wasm.tsx`) | cabeçalho, envio, barra, painel, diálogos, o texto sob cada foto, as Server Actions; entrega ponteiro e teclado; rola a página; agenda os quadros |
//! | aqui | a grade: geometria, seleção, contagens, miniaturas (busca, decodificação, textura) e o desenho dos tiles na GPU |
//!
//! É o mesmo desenho do `revelacao-web` (`abrir(canvas) → Motor`): o wasm é
//! motor, o site é a tela. **Já foi o contrário** — em 2026-09-05, a pedido
//! do dono, este crate desenhou a tela inteira em egui; no mesmo dia, vendo o
//! resultado ao lado do editor, ele reverteu: *"a biblioteca-web deveria usar
//! as tecnologias da revelacao-web"*. O registro completo, com o que não
//! refazer, está em `docs/BIBLIOTECA_NO_NAVEGADOR.md` do site.
//!
//! ## O protocolo
//!
//! Entra JSON pequeno (`definir_fotos`: id, miniatura, estado, apagada, ordem)
//! e eventos crus. Sai um **bitset** por chamada dizendo o que mudou (ver
//! [`crate::grade::mudou`]), e getters JSON para cada fatia — o React relê só
//! a fatia cujo bit acendeu.

use biblioteca_core::grade::{ZOOM_MAX, ZOOM_MIN, ZOOM_PADRAO};
use biblioteca_core::selecao::Modificadores;
use wasm_bindgen::prelude::*;

use crate::grade::{mudou, Grade as Estado};
use crate::render::Superficie;

fn erro(mensagem: impl Into<String>) -> JsValue {
    JsValue::from_str(&mensagem.into())
}

/// O esquema do depósito local (nome, versão, lojas) — **a fonte única** que o
/// lado TypeScript lê antes de abrir o IndexedDB. Ver `local.rs`.
#[wasm_bindgen]
pub fn esquema_local_json() -> String {
    serde_json::to_string(&crate::local::esquema()).unwrap_or_else(|_| "{}".into())
}

/// Os limites do zoom, do core — para o slider do site não repetir os números.
#[wasm_bindgen]
pub fn limites_de_zoom_json() -> String {
    format!("{{\"min\":{ZOOM_MIN},\"max\":{ZOOM_MAX},\"padrao\":{ZOOM_PADRAO}}}")
}

/// Os bits de mudança, por nome — lidos uma vez pelo `motor.ts`.
#[wasm_bindgen]
pub fn bits_de_mudanca_json() -> String {
    serde_json::to_string(&mudou::tabela()).unwrap_or_else(|_| "{}".into())
}

/// A grade aberta sobre um `<canvas>`.
#[wasm_bindgen]
pub struct Grade {
    estado: Estado,
    superficie: Superficie,
    inicio: Option<f64>,
}

/// Abre a grade sobre o canvas: WebGPU se houver, senão WebGL2. Rejeita
/// quando nenhum dos dois responde — o site mostra o aviso e para.
#[wasm_bindgen]
pub async fn abrir(canvas: web_sys::HtmlCanvasElement, escuro: bool) -> Result<Grade, JsValue> {
    console_error_panic_hook::set_once();
    let superficie = Superficie::abrir(canvas).await.map_err(erro)?;
    let estado = Estado::nova(superficie.backend(), escuro);
    Ok(Grade {
        estado,
        superficie,
        inicio: None,
    })
}

fn modificadores(ctrl: bool, shift: bool, meta: bool) -> Modificadores {
    Modificadores {
        aditivo: ctrl || meta,
        faixa: shift,
    }
}

#[wasm_bindgen]
impl Grade {
    /// `"webgpu"` ou `"webgl"` — o que respondeu.
    pub fn backend(&self) -> String {
        self.estado.backend.clone()
    }

    /// A função que o hospedeiro quer que seja chamada quando uma miniatura
    /// chega — ele agenda um quadro nela.
    pub fn definir_despertador(&mut self, f: js_sys::Function) {
        self.estado.caixa.definir_despertador(f);
    }

    pub fn definir_tema(&mut self, escuro: bool) -> u32 {
        self.estado.definir_tema(escuro)
    }

    // ----- dados -----

    /// A lista de fotos **já na ordem da grade** (o recorte é feito aqui).
    pub fn definir_fotos(&mut self, json: &str) -> Result<u32, JsValue> {
        self.estado.definir_fotos(json).map_err(erro)
    }

    /// `todas` | `levada_no_balcao` | `disponivel` | `comprada` | `apagadas`.
    pub fn definir_filtro(&mut self, filtro: &str) -> Result<u32, JsValue> {
        self.estado.definir_filtro(filtro).map_err(erro)
    }

    pub fn definir_zoom(&mut self, zoom: f32) -> u32 {
        self.estado.definir_zoom(zoom)
    }

    /// O maior zoom em que todas as fotos do recorte cabem em
    /// `altura_disponivel` (a da **área**, não a do canvas), ou `-1` quando não
    /// cabem nem no menor tile.
    pub fn zoom_para_caber(&self, altura_disponivel: f32) -> f32 {
        self.estado
            .zoom_para_caber(altura_disponivel)
            .unwrap_or(-1.0)
    }

    // ----- janela -----

    /// O tamanho do canvas em pixels de CSS (a janela visível) e a razão de
    /// pixels do dispositivo.
    pub fn redimensionar(&mut self, largura: f32, altura_visivel: f32, dpr: f32) -> u32 {
        let bits = self.estado.redimensionar(largura, altura_visivel, dpr);
        let dpr = self.estado.dpr;
        self.superficie.redimensionar(
            (largura * dpr).round().max(1.0) as u32,
            (altura_visivel * dpr).round().max(1.0) as u32,
        );
        bits
    }

    /// O topo da janela visível, em pixels de conteúdo — a página rolou.
    pub fn rolar(&mut self, deslocamento: f32) -> u32 {
        self.estado.rolar(deslocamento)
    }

    // ----- entrada (coordenadas relativas ao canvas, em px de CSS) -----

    pub fn ponteiro_moveu(&mut self, x: f32, y: f32) -> u32 {
        self.estado.ponteiro_moveu(x, y)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ponteiro_botao(
        &mut self,
        x: f32,
        y: f32,
        botao: u8,
        apertado: bool,
        ctrl: bool,
        shift: bool,
        meta: bool,
    ) -> u32 {
        self.estado
            .ponteiro_botao(x, y, botao, apertado, modificadores(ctrl, shift, meta))
    }

    pub fn ponteiro_saiu(&mut self) -> u32 {
        self.estado.ponteiro_saiu()
    }

    /// `KeyboardEvent.key`. Devolve `CONSUMIDA` quando a tecla era da grade.
    pub fn tecla(&mut self, nome: &str, ctrl: bool, shift: bool, meta: bool) -> u32 {
        self.estado
            .tecla(nome, modificadores(ctrl, shift, meta), ctrl || meta)
    }

    pub fn foco(&mut self, tem: bool) -> u32 {
        self.estado.foco(tem)
    }

    pub fn alternar_visiveis(&mut self) -> u32 {
        self.estado.alternar_visiveis()
    }

    pub fn limpar_selecao(&mut self) -> u32 {
        self.estado.limpar_selecao()
    }

    /// A tira do site clicou nesta foto: foca e seleciona, com os mesmos
    /// modificadores do canvas (Ctrl acrescenta, Shift estende).
    pub fn focar_id(&mut self, id: &str, aditivo: bool, faixa: bool) -> u32 {
        self.estado.focar_id(id, aditivo, faixa)
    }

    /// Os ids do recorte em vigor, na ordem da grade — o que a tira percorre.
    pub fn ids_visiveis_json(&self) -> String {
        self.estado.ids_visiveis_json()
    }

    // ----- leitura -----

    /// A foto sob um ponto do canvas; `-1` no vazio.
    pub fn indice_em(&self, x: f32, y: f32) -> i32 {
        self.estado.indice_em(x, y).map_or(-1, |n| n as i32)
    }

    pub fn sob_ponteiro(&self) -> i32 {
        self.estado.hover.map_or(-1, |n| n as i32)
    }

    pub fn altura_total(&self) -> f32 {
        self.estado.layout.altura_total
    }

    pub fn layout_json(&self) -> String {
        self.estado.layout_json()
    }

    pub fn contagens_json(&self) -> String {
        self.estado.contagens_json()
    }

    pub fn selecao_json(&self) -> String {
        self.estado.selecao_json()
    }

    pub fn visiveis_json(&self) -> String {
        self.estado.visiveis_json()
    }

    /// `[y, h]` do tile a garantir visível (o teclado moveu o foco); vazio
    /// quando não há. Esvazia ao ler.
    pub fn alvo_de_rolagem(&mut self) -> Vec<f32> {
        self.estado.alvo_de_rolagem()
    }

    // ----- o quadro -----

    /// Desenha um quadro. Devolve `true` quando quer outro em seguida (uma
    /// miniatura chegou, um arrasto está em curso) — o hospedeiro decide se
    /// agenda o próximo `requestAnimationFrame` ou dorme até o próximo evento.
    pub fn quadro(&mut self, agora_ms: f64) -> bool {
        self.estado.receber();

        let inicio = *self.inicio.get_or_insert(agora_ms);
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(self.estado.largura, self.estado.altura_visivel),
            )),
            time: Some((agora_ms - inicio) / 1000.0),
            ..Default::default()
        };
        raw.viewports
            .entry(raw.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(self.estado.dpr);

        let ctx = self.estado.ctx.clone();
        let estado = &self.estado;
        let saida = ctx.run(raw, |c| crate::pintor::pintar(c, estado));
        self.superficie.desenhar(&ctx, saida, estado.cores.fundo);
        ctx.has_requested_repaint()
    }
}
