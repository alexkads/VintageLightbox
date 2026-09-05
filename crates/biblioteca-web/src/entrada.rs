//! Do evento do navegador ao `RawInput` do egui.
//!
//! O egui não sabe o que é um `<canvas>`: a cada quadro ele recebe a lista de
//! eventos desde o quadro anterior, o tamanho da tela e o relógio. Quem ouve o
//! DOM é o hospedeiro (`app.tsx`), que chama os métodos de `App` — e eles
//! caem aqui, numa fila que o próximo quadro consome.
//!
//! 🔑 **Coordenadas em pontos de CSS**, não em pixels de dispositivo: o egui
//! trabalha em pontos e multiplica pelo `pixels_per_point` na hora de
//! desenhar. O hospedeiro manda `clientX/clientY` relativos ao canvas, e é isso.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

#[derive(Default)]
pub struct Entrada {
    eventos: Vec<Event>,
    modificadores: Modifiers,
    tamanho: Vec2,
    dpr: f32,
    inicio: Option<f64>,
    tempo: f64,
}

impl Entrada {
    pub fn redimensionar(&mut self, largura: f32, altura: f32, dpr: f32) {
        self.tamanho = Vec2::new(largura, altura);
        self.dpr = dpr;
    }

    pub fn dpr(&self) -> f32 {
        if self.dpr > 0.0 {
            self.dpr
        } else {
            1.0
        }
    }

    fn modificadores(&mut self, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Modifiers {
        let m = Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd: meta,
            command: ctrl || meta,
        };
        self.modificadores = m;
        m
    }

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
        self.modificadores(ctrl, shift, alt, meta);
        self.eventos.push(Event::PointerMoved(Pos2::new(x, y)));
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
        let modifiers = self.modificadores(ctrl, shift, alt, meta);
        let button = match botao {
            1 => PointerButton::Middle,
            2 => PointerButton::Secondary,
            _ => PointerButton::Primary,
        };
        self.eventos.push(Event::PointerButton {
            pos: Pos2::new(x, y),
            button,
            pressed: apertado,
            modifiers,
        });
    }

    pub fn ponteiro_saiu(&mut self) {
        self.eventos.push(Event::PointerGone);
    }

    /// A roda do mouse. `dx`/`dy` em pixels de CSS, no sentido do DOM (positivo
    /// = o conteúdo sobe); o egui quer o contrário.
    pub fn roda(&mut self, dx: f32, dy: f32, ctrl: bool, shift: bool, alt: bool, meta: bool) {
        let modifiers = self.modificadores(ctrl, shift, alt, meta);
        self.eventos.push(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(-dx, -dy),
            modifiers,
        });
    }

    /// Uma tecla. `nome` é o `KeyboardEvent.key`; o que não é tecla nomeada e
    /// tem um caractere só vira texto digitado.
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
        let modifiers = self.modificadores(ctrl, shift, alt, meta);

        // Copiar/colar/recortar são eventos próprios do egui, e chegam aqui
        // porque o hospedeiro não consegue ler a área de transferência de
        // dentro de um `keydown` sem gesto — quem cola é o próprio `paste`.
        if apertada && modifiers.command {
            match nome {
                "c" | "C" => {
                    self.eventos.push(Event::Copy);
                    return;
                }
                "x" | "X" => {
                    self.eventos.push(Event::Cut);
                    return;
                }
                _ => {}
            }
        }

        if let Some(key) = tecla_do_dom(nome) {
            self.eventos.push(Event::Key {
                key,
                physical_key: None,
                pressed: apertada,
                repeat: repetida,
                modifiers,
            });
        }

        // Texto: um caractere imprimível, sem Ctrl/⌘ segurado.
        if apertada && !modifiers.command && !modifiers.alt {
            let mut chars = nome.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if !c.is_control() {
                    self.eventos.push(Event::Text(c.to_string()));
                }
            }
        }
    }

    pub fn colar(&mut self, texto: &str) {
        self.eventos.push(Event::Paste(texto.to_string()));
    }

    pub fn foco(&mut self, tem: bool) {
        self.eventos.push(Event::WindowFocused(tem));
    }

    /// Monta o `RawInput` do quadro e esvazia a fila.
    pub fn quadro(&mut self, agora_ms: f64) -> RawInput {
        let inicio = *self.inicio.get_or_insert(agora_ms);
        let anterior = self.tempo;
        self.tempo = (agora_ms - inicio) / 1000.0;
        let dt = (self.tempo - anterior).clamp(1.0 / 240.0, 1.0) as f32;

        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.tamanho)),
            time: Some(self.tempo),
            predicted_dt: dt,
            modifiers: self.modificadores,
            events: std::mem::take(&mut self.eventos),
            ..Default::default()
        };
        raw.viewports
            .entry(raw.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(self.dpr());
        raw
    }
}

/// `KeyboardEvent.key` → `egui::Key`. O egui reconhece os nomes do DOM para
/// as teclas nomeadas; as letras e dígitos vêm em maiúsculas por convenção
/// dele.
fn tecla_do_dom(nome: &str) -> Option<Key> {
    match nome {
        " " => Some(Key::Space),
        "Esc" => Some(Key::Escape),
        "Del" => Some(Key::Delete),
        _ => {
            if let Some(k) = Key::from_name(nome) {
                return Some(k);
            }
            let mut chars = nome.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => {
                    Key::from_name(&c.to_ascii_uppercase().to_string())
                }
                _ => None,
            }
        }
    }
}
