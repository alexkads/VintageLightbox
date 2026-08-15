//! O tema **Vintage Dark**, dito na linguagem do `gpui-component`.
//!
//! `gpui_component::init` não é neutro: ele instala o tema do shadcn — fundo
//! `#0a0a0a`, cantos de 6px — e **sincroniza claro/escuro com o sistema**. Os
//! dois são errados aqui, por razões diferentes:
//!
//! - o app de egui desenha `#1a1a1a` com cantos de 3px, e a fase 2 se confere
//!   por paridade (docs/10-MIGRACAO-GPUI.md §7.1);
//! - um programa de revelação **não pode ficar branco** porque o macOS amanheceu
//!   no modo claro. O entorno é parte da medição de cor — é a mesma razão pela
//!   qual a moldura da foto selecionada é borda, e não fundo colorido.
//!
//! Por isso, depois do `init`, este módulo troca o tema escuro pelo nosso e
//! **trava o modo em escuro**.
//!
//! ## Por que JSON, e não um literal de struct
//!
//! `ThemeConfigColors` tem doze campos privados (as cores base), e isso torna
//! `..Default::default()` proibido de fora do crate — um literal de struct não
//! compila. O JSON é o formato nativo do `gpui-component` (é o que ele carrega
//! de `~/.config/.../themes`), então escrever nele não é contorno: é usar a
//! porta da frente. As chaves são as do
//! [esquema oficial](https://github.com/longbridge/gpui-component), e é por isso
//! que aparecem com ponto (`accent.background`) em vez do nome do campo Rust.
//!
//! ## 🚨 Os dois modos de errar aqui são silenciosos
//!
//! 1. **Cor ilegível não falha: ela some.** O `apply_config` tenta ler cada
//!    hexadecimal e, quando não consegue, usa o padrão dele **sem dizer nada** —
//!    um `#1a1a1` de um dígito a menos devolveria o shadcn num campo só.
//! 2. **Chave errada não falha: ela é ignorada.** O `serde` do `ThemeConfig` não
//!    recusa campo desconhecido, então `acent.background` seria lido, descartado
//!    e o tema abriria com aquele token no valor de fábrica.
//!
//! Nenhum dos dois dá erro, log ou tela quebrada — dão *uma cor diferente*, que
//! é exatamente o tipo de coisa que se atribui ao framework três meses depois.
//! Os testes no fim do arquivo existem para pegar os dois.

use std::rc::Rc;

use gpui::App;
use gpui_component::{Theme, ThemeConfig, ThemeMode};
use serde_json::{json, Value};

/// A paleta de `crates/ui/src/design_system/theme.rs`, em hexadecimal.
///
/// Repetida, e não importada: `crates/ui` é o crate do egui, e depender dele
/// aqui traria o eframe inteiro junto — o oposto do que a migração faz. A cópia
/// não corre risco de divergir porque `crates/ui` está congelado até a fase 5
/// (§7.1: nenhuma feature nova), e some junto com ele.
mod paleta {
    pub const BG_APP: &str = "#1a1a1a";
    pub const BG_SURFACE: &str = "#252525";
    pub const BG_ELEVATED: &str = "#2d2d2d";
    pub const BG_HOVER: &str = "#353535";
    pub const BG_ACTIVE: &str = "#3a3a3a";

    pub const TEXT_PRIMARY: &str = "#e0e0e0";
    pub const TEXT_SECONDARY: &str = "#b0b0b0";
    pub const TEXT_MUTED: &str = "#808080";
    pub const TEXT_DISABLED: &str = "#606060";

    pub const ACCENT_PRIMARY: &str = "#4a9eff";
    pub const ACCENT_HOVER: &str = "#5aafff";
    pub const ACCENT_PRESSED: &str = "#3a8ee6";
    pub const ACCENT_WARNING: &str = "#ffd700";
    pub const ACCENT_SUCCESS: &str = "#4ade80";
    pub const ACCENT_ERROR: &str = "#ef4444";

    pub const BORDER_DEFAULT: &str = "#333333";
    pub const BORDER_HOVER: &str = "#404040";

    /// Texto sobre o azul — o único fundo do tema claro o bastante para o
    /// `TEXT_PRIMARY` perder contraste em cima.
    pub const SOBRE_ACENTO: &str = "#ffffff";
    /// Texto sobre o amarelo e o verde de aviso, que são claros de verdade.
    pub const SOBRE_CLARO: &str = "#1a1a1a";
}

use paleta::*;

/// `RADIUS_SM` do design system: é o que os cantos desenham hoje, na grade e nos
/// botões de filtro. O padrão do `gpui-component` é 6px, que arredonda um botão
/// de 20px de altura até ele virar pílula.
const RAIO: usize = 3;
/// `RADIUS_XL` — diálogo e aviso, peças grandes o bastante para o canto pequeno
/// sumir nelas.
const RAIO_GRANDE: usize = 8;

/// Instala o Vintage Dark e trava o modo em escuro.
///
/// Chamar **depois** de `gpui_component::init`, que é quem cria o `Theme`
/// global — antes dele isto não teria o que trocar.
pub fn aplicar(cx: &mut App) {
    let vintage = Rc::new(vintage_dark());

    // `apply_config` guarda a configuração como "o tema escuro" e já pinta as
    // cores dela.
    Theme::global_mut(cx).apply_config(&vintage);
    // ...mas quem fixa o **modo** é o `change`. Sem esta linha o `mode` continua
    // sendo o que o sistema respondeu lá no `init`: numa máquina em modo claro,
    // o tema guardado seria o nosso e a tela continuaria branca.
    Theme::change(ThemeMode::Dark, None, cx);
}

fn vintage_dark() -> ThemeConfig {
    serde_json::from_value(json!({
        "is_default": true,
        "name": "Vintage Dark",
        "mode": "dark",
        "radius": RAIO,
        "radius.lg": RAIO_GRANDE,
        // Sombra desligada, como no egui ("Flat shadows for minimalist look").
        // Aqui isso é mais que estilo: sombra em cima de miniatura é gradiente
        // escuro encostando na foto, e quem julga exposição olhando a grade
        // passa a julgar a sombra junto.
        "shadow": false,
        "colors": cores(),
    }))
    .expect("o Vintage Dark tem de ser legível — `tema_e_legivel` confere isso")
}

/// Só o que o design system nomeia — chave do esquema à esquerda, cor à direita.
///
/// O que ele não nomeia fica de fora de propósito, e o `gpui-component` resolve
/// com o escuro padrão dele. Inventar valor para os ~40 tokens restantes seria
/// criar decisão de cor que ninguém tomou — e que a comparação de paridade com o
/// app de egui não teria como conferir.
///
/// Uma lista de pares, e não um `json!`: com este tamanho o `json!` estoura o
/// limite de recursão de macro do rustc, e subir o limite do crate inteiro para
/// escrever um objeto plano é pagar caro por açúcar.
const CORES: &[(&str, &str)] = &[
    ("background", BG_APP),
    ("foreground", TEXT_PRIMARY),
    ("border", BORDER_DEFAULT),
    ("muted.background", BG_SURFACE),
    ("muted.foreground", TEXT_MUTED),
    // O secundário é o botão comum — o de filtro da barra.
    ("secondary.background", BG_ELEVATED),
    ("secondary.hover.background", BG_HOVER),
    ("secondary.active.background", BG_ACTIVE),
    ("secondary.foreground", TEXT_SECONDARY),
    // O acento é o realce do item sob o ponteiro, em menu e em lista.
    ("accent.background", BG_HOVER),
    ("accent.foreground", TEXT_PRIMARY),
    ("primary.background", ACCENT_PRIMARY),
    ("primary.hover.background", ACCENT_HOVER),
    ("primary.active.background", ACCENT_PRESSED),
    ("primary.foreground", SOBRE_ACENTO),
    ("ring", ACCENT_PRIMARY),
    ("selection.background", ACCENT_PRIMARY),
    ("caret", ACCENT_PRIMARY),
    ("input.border", BORDER_DEFAULT),
    ("popover.background", BG_ELEVATED),
    ("popover.foreground", TEXT_PRIMARY),
    ("sidebar.background", BG_SURFACE),
    ("sidebar.foreground", TEXT_PRIMARY),
    ("sidebar.border", BORDER_DEFAULT),
    ("sidebar.accent.background", BG_HOVER),
    ("sidebar.accent.foreground", TEXT_PRIMARY),
    ("sidebar.primary.background", ACCENT_PRIMARY),
    ("sidebar.primary.foreground", SOBRE_ACENTO),
    ("list.background", BG_SURFACE),
    ("list.hover.background", BG_HOVER),
    ("list.active.background", BG_ACTIVE),
    ("list.active.border", ACCENT_PRIMARY),
    // Os dois que a fase 2 vai gastar mais: a barra preenchida do slider e o
    // punho. O punho é claro, e não azul, para continuar visível quando ~50
    // ajustes estiverem empilhados na mesma coluna.
    ("slider.background", ACCENT_PRIMARY),
    ("slider.thumb.background", TEXT_PRIMARY),
    ("scrollbar.thumb.background", BORDER_HOVER),
    ("scrollbar.thumb.hover.background", TEXT_DISABLED),
    ("title_bar.background", BG_SURFACE),
    ("title_bar.border", BORDER_DEFAULT),
    ("danger.background", ACCENT_ERROR),
    ("danger.foreground", SOBRE_ACENTO),
    ("warning.background", ACCENT_WARNING),
    ("warning.foreground", SOBRE_CLARO),
    ("success.background", ACCENT_SUCCESS),
    ("success.foreground", SOBRE_CLARO),
    ("link", ACCENT_PRIMARY),
    ("link.hover", ACCENT_HOVER),
    ("link.active", ACCENT_PRESSED),
];

fn cores() -> Value {
    Value::Object(
        CORES
            .iter()
            .map(|(chave, cor)| (chave.to_string(), Value::String(cor.to_string())))
            .collect(),
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A configuração inteira é legível — é o `expect` de `vintage_dark` virando
    /// teste em vez de virar pânico na abertura do app.
    #[test]
    fn tema_e_legivel() {
        let config = vintage_dark();
        assert_eq!(config.mode, ThemeMode::Dark);
        assert!(config.is_default);
        assert_eq!(config.radius, Some(RAIO));
        assert_eq!(config.shadow, Some(false));
    }

    /// 🚨 Todo hexadecimal está na forma que o `gpui-component` aceita.
    ///
    /// Ele não devolve erro quando não consegue ler uma cor: usa o padrão dele,
    /// calado. Um dígito a menos daria um app que abre, roda, e está com uma cor
    /// de outro tema no meio.
    ///
    /// Confere a forma `#rrggbb` aqui, sem abrir janela, porque o
    /// `try_parse_color` do crate é privado.
    #[test]
    fn toda_cor_e_hexadecimal_de_seis_digitos() {
        assert!(
            !CORES.is_empty(),
            "nenhuma cor declarada — o tema não estaria fazendo nada"
        );

        for (chave, hex) in CORES {
            assert!(
                hex.len() == 7
                    && hex.starts_with('#')
                    && hex[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "`{chave}` = {hex:?} não é #rrggbb — o gpui-component cairia no tema dele, sem avisar"
            );
        }
    }

    /// 🚨 Toda chave declarada chegou de fato ao tema.
    ///
    /// O `ThemeConfig` não recusa campo desconhecido: `acent.background` seria
    /// lido, descartado, e o token ficaria no valor de fábrica sem uma linha de
    /// aviso. O jeito de flagrar isso sem depender da lista de ~90 nomes é ir e
    /// voltar — serializar o que foi lido e cobrar cada chave escrita aqui.
    #[test]
    fn nenhuma_chave_e_ignorada() {
        let config = vintage_dark();
        let de_volta = serde_json::to_value(&config.colors).expect("as cores devem serializar");

        for (chave, hex) in CORES {
            assert_eq!(
                de_volta.get(chave).and_then(|v| v.as_str()),
                Some(*hex),
                "`{chave}` não sobreviveu à leitura — nome fora do esquema do gpui-component"
            );
        }
    }
}
