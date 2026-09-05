//! A fronteira com o JavaScript — e ela é curta de propósito.
//!
//! ## O que fica de cada lado
//!
//! | lado | faz |
//! |---|---|
//! | JavaScript (`app.tsx`) | monta o `<canvas>`, entrega os eventos do DOM, roda o laço de quadros, escolhe arquivo, copia, abre aba, fala com o Worker da importação |
//! | aqui | **a tela inteira**: cabeçalho, envio, barra, grade, painel, diálogos — decididos pelo `biblioteca-core` e desenhados pelo egui sobre o wgpu |
//!
//! 🔑 **O JavaScript não desenha nada e não decide nada.** Ele é o sistema
//! operacional desta tela: fornece entrada, saída e as duas ou três coisas que
//! só o DOM faz (`<input type=file>`, área de transferência, `window.open`).
//! Foi o que o dono pediu em 2026-09-05 — *"todas as funcionalidades dentro do
//! wasm, não uma coisa híbrida"* — e é o que permite ao VintageLightbox usar as
//! mesmas telas.

use wasm_bindgen::prelude::*;

use crate::app::{App as Tela, Pedido};
use crate::entrada::Entrada;
use crate::render::Superficie;

fn erro(mensagem: impl Into<String>) -> JsValue {
    JsValue::from_str(&mensagem.into())
}

/// A galeria do pós-venda aberta sobre um `<canvas>`.
#[wasm_bindgen]
pub struct App {
    tela: Tela,
    entrada: Entrada,
    superficie: Superficie,
}

/// O esquema do depósito local (nome, versão, lojas) — **a fonte única** que o
/// lado TypeScript lê antes de abrir o IndexedDB. Ver `local.rs`.
#[wasm_bindgen]
pub fn esquema_local_json() -> String {
    serde_json::to_string(&crate::local::esquema()).unwrap_or_else(|_| "{}".into())
}

/// Abre a tela sobre o canvas: WebGPU se houver, senão WebGL2.
///
/// `estado_json` é a galeria inteira como o site a monta
/// (`estado-da-galeria.ts`); depois disso a tela relê sozinha, pela rota
/// `/api/estado`, a cada gravação.
#[wasm_bindgen]
pub async fn abrir_app(
    canvas: web_sys::HtmlCanvasElement,
    galeria_id: String,
    estado_json: String,
    tema_escuro: bool,
) -> Result<App, JsValue> {
    console_error_panic_hook::set_once();

    let superficie = Superficie::abrir(canvas).await.map_err(erro)?;
    let tela = Tela::novo(galeria_id, &estado_json, superficie.backend()).map_err(erro)?;

    // O visual é o do editor de revelação, sempre escuro — ver `tema.rs`.
    // `tema_escuro` fica na assinatura para o hospedeiro não mudar; o valor
    // não muda nada, como o editor também não muda com o tema do site.
    let _ = tema_escuro;
    crate::tema::aplicar(&tela.ctx);

    Ok(App {
        tela,
        entrada: Entrada::default(),
        superficie,
    })
}

#[wasm_bindgen]
impl App {
    /// `"webgpu"` ou `"webgl"` — o que respondeu.
    pub fn backend(&self) -> String {
        self.tela.backend.clone()
    }

    /// O tamanho do canvas em pixels de CSS e a razão de pixels do dispositivo.
    pub fn redimensionar(&mut self, largura: f32, altura: f32, dpr: f32) {
        self.entrada.redimensionar(largura, altura, dpr);
        let dpr = self.entrada.dpr();
        self.superficie.redimensionar(
            (largura * dpr).round().max(1.0) as u32,
            (altura * dpr).round().max(1.0) as u32,
        );
        self.tela.ctx.request_repaint();
    }

    /// O tema do site mudou. A biblioteca segue o editor de revelação, que é
    /// escuro em qualquer tema — então nada muda aqui (ver `tema.rs`).
    pub fn definir_tema(&mut self, escuro: bool) {
        let _ = escuro;
    }

    /// A função que o hospedeiro quer que seja chamada quando uma resposta
    /// chega (rede, miniatura) — ele agenda um quadro nela.
    pub fn definir_despertador(&mut self, f: js_sys::Function) {
        self.tela.caixa.definir_despertador(f);
    }

    // ----- entrada -----

    #[allow(clippy::too_many_arguments)]
    pub fn ponteiro_moveu(
        &mut self,
        x: f32,
        y: f32,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) {
        self.entrada.ponteiro_moveu(x, y, ctrl, shift, alt, meta);
        self.tela.ctx.request_repaint();
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
        alt: bool,
        meta: bool,
    ) {
        self.entrada
            .ponteiro_botao(x, y, botao, apertado, ctrl, shift, alt, meta);
        self.tela.ctx.request_repaint();
    }

    pub fn ponteiro_saiu(&mut self) {
        self.entrada.ponteiro_saiu();
        self.tela.ctx.request_repaint();
    }

    pub fn roda(&mut self, dx: f32, dy: f32, ctrl: bool, shift: bool, alt: bool, meta: bool) {
        self.entrada.roda(dx, dy, ctrl, shift, alt, meta);
        self.tela.ctx.request_repaint();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tecla(
        &mut self,
        nome: &str,
        apertada: bool,
        repetida: bool,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) {
        self.entrada
            .tecla(nome, apertada, repetida, ctrl, shift, alt, meta);
        self.tela.ctx.request_repaint();
    }

    pub fn colar(&mut self, texto: &str) {
        self.entrada.colar(texto);
        self.tela.ctx.request_repaint();
    }

    pub fn foco(&mut self, tem: bool) {
        self.entrada.foco(tem);
    }

    /// O egui quer texto do teclado? (Um campo está com o cursor.) O
    /// hospedeiro usa isto para não roubar as teclas de atalho da página.
    pub fn quer_teclado(&self) -> bool {
        self.tela.ctx.wants_keyboard_input()
    }

    // ----- o que o site sabe e a tela mostra -----

    /// A fila de importação, como o Worker do site a vê (`ItemVisivel[]`).
    pub fn definir_importacao(&mut self, json: &str) {
        match serde_json::from_str(json) {
            Ok(itens) => {
                self.tela.importacao = itens;
                self.tela.ctx.request_repaint();
            }
            Err(e) => self
                .tela
                .avisar(format!("a fila voltou ilegível: {e}"), true),
        }
    }

    pub fn arrastando_arquivos(&mut self, arrastando: bool) {
        if self.tela.arrastando_arquivos != arrastando {
            self.tela.arrastando_arquivos = arrastando;
            self.tela.ctx.request_repaint();
        }
    }

    /// O que os próximos arquivos viram: estado, faixa e parâmetros. É o que
    /// o hospedeiro passa ao `enfileirar` quando o operador solta a leva.
    pub fn leva_json(&self) -> String {
        serde_json::to_string(&self.tela.leva).unwrap_or_else(|_| "{}".into())
    }

    /// A galeria mudou por fora (a importação subiu uma foto): reler.
    pub fn reler(&mut self) {
        self.tela.reler();
    }

    /// O editor de revelação gravou no depósito local: a biblioteca relê o
    /// que está "editada · não salva".
    pub fn revelacoes_mudaram(&mut self) {
        self.tela.revelacoes_mudaram();
    }

    // ----- o quadro -----

    /// Desenha um quadro. Devolve `true` quando a tela quer outro em seguida
    /// (animação, aviso expirando, resposta a caminho) — o hospedeiro decide
    /// se agenda o próximo `requestAnimationFrame` ou dorme até o próximo
    /// evento.
    pub fn quadro(&mut self, agora_ms: f64) -> bool {
        self.tela.receber();
        let raw = self.entrada.quadro(agora_ms);
        let ctx = self.tela.ctx.clone();
        let saida = ctx.run(raw, |c| self.tela.ui(c));

        let plataforma = &saida.platform_output;
        if let Some(url) = &plataforma.open_url {
            self.tela.pedidos.push(Pedido::AbrirUrl {
                url: url.url.clone(),
            });
        }
        if !plataforma.copied_text.is_empty() {
            self.tela.pedidos.push(Pedido::Copiar {
                texto: plataforma.copied_text.clone(),
            });
        }
        self.tela.pedidos.push(Pedido::Cursor {
            cursor: cursor_css(plataforma.cursor_icon).to_string(),
        });

        let fundo = ctx.style().visuals.panel_fill;
        self.superficie.desenhar(&ctx, saida, fundo);
        ctx.has_requested_repaint()
    }

    /// O que a tela pede ao hospedeiro desde o último quadro — e esvazia.
    pub fn pedidos_json(&mut self) -> String {
        let pedidos = std::mem::take(&mut self.tela.pedidos);
        serde_json::to_string(&pedidos).unwrap_or_else(|_| "[]".into())
    }
}

fn cursor_css(icone: egui::CursorIcon) -> &'static str {
    use egui::CursorIcon as C;
    match icone {
        C::Default => "default",
        C::PointingHand => "pointer",
        C::Text => "text",
        C::Grab => "grab",
        C::Grabbing => "grabbing",
        C::Crosshair => "crosshair",
        C::Move => "move",
        C::ResizeHorizontal => "ew-resize",
        C::ResizeVertical => "ns-resize",
        C::ResizeNeSw => "nesw-resize",
        C::ResizeNwSe => "nwse-resize",
        C::NotAllowed => "not-allowed",
        C::Wait => "wait",
        C::Progress => "progress",
        C::Help => "help",
        C::None => "none",
        _ => "default",
    }
}
