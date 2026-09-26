//! O tema do site, dito na linguagem do `gpui-component`.
//!
//! # As cores são as do `recordarfotos.com.br`, nos dois modos
//!
//! Até 2026-09-17 este app tinha um tema próprio, o "Vintage Dark", travado no
//! escuro, com um azul de ação (`#4a9eff`) que o site não usa. O dono pediu que
//! o GPUI ficasse igual ao site (*"deixar o crates/ui-gpui lindo!"*). Por isso
//! as duas paletas abaixo são os tokens do shadcn do site
//! (`frontend/src/app/globals.css`), convertidos de oklch para hexadecimal, e o
//! operador escolhe **Claro, Escuro ou Sistema** no menu da conta, como no
//! site.
//!
//! ## 🟩 Matrix e 🌆 Cyberpunk — os dois temas que não vêm do site
//!
//! Pedido do dono em 2026-09-26 (*"Eu preciso de dois temas, um Matrix e outro
//! Cyberpunk"*). São escuros e ficam no mesmo menu, lembrados na máquina como
//! os outros:
//!
//! | Tema | Fundo | Marca | Letra |
//! |---|---|---|---|
//! | Matrix | preto esverdeado `#030d06` | o verde fósforo `#00ff41` | a mono do sistema |
//! | Cyberpunk | azul-noite `#0a0a1f` | amarelo neon `#fcee0a`, rosa `#ff2a6d` no menu lateral, ciano `#05d9e8` no foco | a do sistema |
//!
//! O âmbar da nota, o verde de "deu certo" e as etiquetas continuam os de
//! sempre: são o significado da foto, e não decoração do tema.
//!
//! | Token do site | Claro | Escuro |
//! |---|---|---|
//! | `--background` | `#ffffff` | `#0a0a0a` |
//! | `--card`, `--popover` | `#ffffff` | `#171717` |
//! | `--primary` (a marca, `#445566` do legado) | `#445566` | `#8fa8c0` |
//! | `--muted`, `--accent`, `--secondary` | `#f5f5f5` | `#262626` |
//! | `--muted-foreground` | `#737373` | `#a1a1a1` |
//! | `--border` | `#e5e5e5` | branco a 10% |
//! | `--sidebar` | `#fafafa` | `#171717` |
//! | `--sidebar-primary` (o quadrado da marca) | `#171717` | `#1447e6` |
//!
//! ⚠️ **A borda do escuro é branco a 10% no site**, e aqui é opaca: o
//! `gpui-component` lê a cor como está. O valor é o branco a 10% já somado ao
//! fundo de cada superfície (`#232323` no fundo, `#2e2e2e` no menu lateral).
//!
//! ## O âmbar e o verde continuam, com os tons do Tailwind
//!
//! O site marca o que é do cliente com âmbar (os recortes acesos da galeria,
//! "Sem fotos") e o que deu certo com esmeralda ("Cliente já abriu", "Pago no
//! pós-venda"). Os tons são os do Tailwind 4, que é o que o site usa.
//!
//! ## 🚨 Os dois modos de errar aqui são silenciosos
//!
//! 1. **Cor ilegível não falha: ela some.** O `apply_config` tenta ler cada
//!    hexadecimal e, quando não consegue, usa o padrão dele **sem dizer nada**.
//!    `hex` sempre produz `#rrggbb`, e o teste `todo_hexadecimal_tem_seis_digitos`
//!    é quem mantém assim.
//! 2. **Chave errada não falha: ela é ignorada.** O `serde` do `ThemeConfig` não
//!    recusa campo desconhecido. `nenhuma_chave_e_ignorada` pega isso.
//!
//! ## 🚨 `apply_config` pinta, mesmo quando o modo é o outro
//!
//! Aplicar a configuração escura guarda "o tema escuro" **e** troca as cores da
//! tela por ela. Por isso [`aplicar`] instala as duas e só então chama
//! `Theme::change` com o modo escolhido: é ele que decide qual das duas vale.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use gpui_kit::component::button::ButtonCustomVariant;
use gpui_kit::component::{Theme, ThemeConfig, ThemeMode};
use gpui_kit::{App, Window, WindowAppearance};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// As duas paletas, em `0xrrggbb`.
mod paleta {
    /// Os tokens do shadcn do site, num modo.
    #[derive(Debug, PartialEq)]
    pub struct Paleta {
        pub fundo: u32,
        pub texto: u32,
        pub cartao: u32,
        pub primaria: u32,
        pub primaria_pairando: u32,
        pub sobre_primaria: u32,
        pub apagado: u32,
        pub texto_apagado: u32,
        pub acento: u32,
        pub sobre_acento: u32,
        pub destrutiva: u32,
        pub borda: u32,
        pub campo: u32,
        pub anel: u32,
        pub lateral: u32,
        pub texto_lateral: u32,
        pub marca: u32,
        pub sobre_marca: u32,
        pub acento_lateral: u32,
        pub borda_lateral: u32,
        /// Atrás de uma foto.
        pub poco: u32,
        pub rolagem: u32,
    }

    pub const CLARO: Paleta = Paleta {
        fundo: 0xffffff,
        texto: 0x0a0a0a,
        cartao: 0xffffff,
        primaria: 0x445566,
        // `bg-primary/80` sobre o branco.
        primaria_pairando: 0x6a7785,
        sobre_primaria: 0xfafafa,
        apagado: 0xf5f5f5,
        texto_apagado: 0x737373,
        acento: 0xf5f5f5,
        sobre_acento: 0x171717,
        destrutiva: 0xe7000b,
        borda: 0xe5e5e5,
        campo: 0xe5e5e5,
        anel: 0xa1a1a1,
        lateral: 0xfafafa,
        texto_lateral: 0x0a0a0a,
        marca: 0x171717,
        sobre_marca: 0xfafafa,
        acento_lateral: 0xf0f0f0,
        borda_lateral: 0xe5e5e5,
        poco: 0xf5f5f5,
        rolagem: 0xd4d4d4,
    };

    pub const ESCURO: Paleta = Paleta {
        fundo: 0x0a0a0a,
        texto: 0xfafafa,
        cartao: 0x171717,
        primaria: 0x8fa8c0,
        // `bg-primary/80` sobre o fundo escuro.
        primaria_pairando: 0x748699,
        sobre_primaria: 0x171717,
        apagado: 0x262626,
        texto_apagado: 0xa1a1a1,
        acento: 0x262626,
        sobre_acento: 0xfafafa,
        destrutiva: 0xff6467,
        borda: 0x232323,
        campo: 0x2f2f2f,
        anel: 0x737373,
        lateral: 0x171717,
        texto_lateral: 0xfafafa,
        marca: 0x1447e6,
        sobre_marca: 0xfafafa,
        acento_lateral: 0x262626,
        borda_lateral: 0x2e2e2e,
        poco: 0x171717,
        rolagem: 0x404040,
    };

    /// 🟩 Matrix: o verde fósforo sobre preto.
    pub const MATRIX: Paleta = Paleta {
        fundo: 0x030d06,
        texto: 0x5cff8f,
        cartao: 0x071a0d,
        primaria: 0x00ff41,
        // `bg-primary/80` sobre o fundo.
        primaria_pairando: 0x01cf35,
        sobre_primaria: 0x021a08,
        apagado: 0x0b2413,
        texto_apagado: 0x34b865,
        acento: 0x0d2e18,
        sobre_acento: 0x8dffb0,
        destrutiva: 0xff4d4d,
        borda: 0x114225,
        campo: 0x17592f,
        anel: 0x00ff41,
        lateral: 0x020805,
        texto_lateral: 0x5cff8f,
        marca: 0x00ff41,
        sobre_marca: 0x021a08,
        acento_lateral: 0x0d2e18,
        borda_lateral: 0x114225,
        poco: 0x000000,
        rolagem: 0x17592f,
    };

    /// 🌆 Cyberpunk: neon amarelo, rosa e ciano sobre azul-noite.
    pub const CYBERPUNK: Paleta = Paleta {
        fundo: 0x0a0a1f,
        texto: 0xeae8ff,
        cartao: 0x141438,
        primaria: 0xfcee0a,
        // `bg-primary/80` sobre o fundo.
        primaria_pairando: 0xccc00e,
        sobre_primaria: 0x0a0a1f,
        apagado: 0x1c1c47,
        texto_apagado: 0xa3a1dc,
        acento: 0x24245a,
        sobre_acento: 0x05d9e8,
        destrutiva: 0xff2a6d,
        borda: 0x2c2c66,
        campo: 0x3a3a80,
        anel: 0x05d9e8,
        lateral: 0x07071a,
        texto_lateral: 0xeae8ff,
        marca: 0xff2a6d,
        sobre_marca: 0x0a0a1f,
        acento_lateral: 0x24245a,
        borda_lateral: 0x2c2c66,
        poco: 0x05050f,
        rolagem: 0x3a3a80,
    };

    // ── Tailwind 4, as famílias que o site usa por nome ────────────────────
    pub const AMBAR_50: u32 = 0xfffbeb;
    pub const AMBAR_300: u32 = 0xffd230;
    pub const AMBAR_400: u32 = 0xffb900;
    pub const AMBAR_500: u32 = 0xfe9a00;
    pub const AMBAR_700: u32 = 0xbb4d00;
    pub const AMBAR_800: u32 = 0x973c00;
    pub const AMBAR_950: u32 = 0x461901;
    pub const ESMERALDA_50: u32 = 0xecfdf5;
    pub const ESMERALDA_300: u32 = 0x5ee9b5;
    pub const ESMERALDA_400: u32 = 0x00d492;
    pub const ESMERALDA_700: u32 = 0x007a55;
    pub const ESMERALDA_800: u32 = 0x006045;
    pub const ESMERALDA_900: u32 = 0x004f3b;
    pub const ESMERALDA_950: u32 = 0x002c22;
    pub const AZUL_500: u32 = 0x2b7fff;
    pub const CEU_50: u32 = 0xf0f9ff;
    pub const CEU_300: u32 = 0x74d4ff;
    pub const CEU_800: u32 = 0x00598a;
    pub const CEU_950: u32 = 0x052f4a;

    // ── Semânticas da fotografia ──────────────────────────────────────────
    /// A estrela acesa: o `text-amber-400` das estrelas da galeria.
    pub const NOTA: u32 = AMBAR_400;
    pub const ESCOLHIDA: u32 = 0x4ade80;
    pub const REJEITADA: u32 = 0xef4444;

    // ── As cinco etiquetas do `ColorLabel` ────────────────────────────────
    pub const ETIQUETA_VERMELHA: u32 = 0xef5d62;
    pub const ETIQUETA_AMARELA: u32 = 0xf2c336;
    pub const ETIQUETA_VERDE: u32 = 0x46b95c;
    pub const ETIQUETA_AZUL: u32 = 0x7b86ff;
    pub const ETIQUETA_ROXA: u32 = 0xb073ea;

    /// Texto escuro sobre âmbar, amarelo e verde.
    pub const SOBRE_CLARO: u32 = 0x1a1206;
}

use paleta::*;

/// O canto padrão: `rounded-md` do site (0,8 × 10 px).
const RAIO: usize = 8;
/// `rounded-lg` do site: cartões, menus e diálogos.
const RAIO_GRANDE: usize = 10;
/// `text-sm` do site: é o tamanho de quase todo texto do painel.
const LETRA: f32 = 14.;

/// Se a tela está no escuro agora. As cores sem `cx` ([`cores`]) leem daqui.
/// Qual paleta está na tela: a de [`Escolha::paleta_escura`] no escuro, a
/// clara no claro. Guardada como o índice de [`PALETAS`].
static PALETA_AGORA: AtomicU8 = AtomicU8::new(1);

/// As paletas, na ordem do índice de [`PALETA_AGORA`].
const PALETAS: [&paleta::Paleta; 4] = [
    &paleta::CLARO,
    &paleta::ESCURO,
    &paleta::MATRIX,
    &paleta::CYBERPUNK,
];

fn paleta_atual() -> &'static paleta::Paleta {
    PALETAS[PALETA_AGORA.load(Ordering::Relaxed) as usize % PALETAS.len()]
}

/// O que o operador escolheu no menu da conta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Escolha {
    Claro,
    Escuro,
    /// O padrão, como no site: segue o sistema.
    #[default]
    Sistema,
    /// 🟩 Verde fósforo sobre preto, com a letra mono.
    Matrix,
    /// 🌆 Neon sobre azul-noite.
    Cyberpunk,
}

impl Escolha {
    pub fn do_nome(nome: &str) -> Option<Self> {
        match nome {
            "claro" => Some(Self::Claro),
            "escuro" => Some(Self::Escuro),
            "sistema" => Some(Self::Sistema),
            "matrix" => Some(Self::Matrix),
            "cyberpunk" => Some(Self::Cyberpunk),
            _ => None,
        }
    }

    /// O modo que vale com esta escolha e esta aparência do sistema.
    pub fn modo(self, aparencia: WindowAppearance) -> ThemeMode {
        match self {
            Self::Claro => ThemeMode::Light,
            Self::Escuro | Self::Matrix | Self::Cyberpunk => ThemeMode::Dark,
            Self::Sistema => match aparencia {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
                WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
            },
        }
    }

    /// O índice em [`PALETAS`] da paleta que esta escolha pinta no escuro.
    fn paleta_escura(self) -> u8 {
        match self {
            Self::Matrix => 2,
            Self::Cyberpunk => 3,
            Self::Claro | Self::Escuro | Self::Sistema => 1,
        }
    }

    fn nome_do_escuro(self) -> &'static str {
        match self {
            Self::Matrix => "Matrix",
            Self::Cyberpunk => "Cyberpunk",
            Self::Claro | Self::Escuro | Self::Sistema => "RecordarFotos Escuro",
        }
    }
}

/// Onde a escolha fica lembrada nesta máquina.
#[cfg(not(test))]
pub fn arquivo_da_escolha() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("tema.json")
}

/// 🚨 **Nos testes, um arquivo temporário por janela.** Até 2026-09-17 o
/// "Claro/Escuro/Sistema" de um teste gravava no `tema.json` do catálogo de
/// quem roda a suíte — e o app dele abria no tema que o teste escolheu.
#[cfg(test)]
pub fn arquivo_da_escolha() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-tema-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Lê a escolha guardada. Arquivo ausente ou estragado é "Sistema".
pub fn escolha_guardada(arquivo: &Path) -> Escolha {
    std::fs::read(arquivo)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Guarda a escolha. Falhar só faz a próxima abertura seguir o sistema.
pub fn guardar_escolha(arquivo: &Path, escolha: Escolha) {
    if let Some(pai) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    if let Ok(texto) = serde_json::to_vec(&escolha) {
        let _ = std::fs::write(arquivo, texto);
    }
}

/// Instala os dois temas do site e liga o modo da escolha.
///
/// Chamar **depois** de `gpui_kit::init`, que é quem cria o `Theme`
/// global.
pub fn aplicar(escolha: Escolha, window: Option<&mut Window>, cx: &mut App) {
    let aparencia = window
        .as_ref()
        .map(|w| w.appearance())
        .unwrap_or_else(|| cx.window_appearance());
    let modo = escolha.modo(aparencia);
    let escura = escolha.paleta_escura();
    // 🟩 O Matrix escreve com a mono que o próprio gpui-kit escolheu para este
    // sistema (Menlo, Consolas…) — nome de fonte escrito à mão faltaria em
    // algum balcão.
    //
    // 🚨 **E os outros devolvem a letra do sistema, dita por extenso.** O
    // `apply_config` só troca o que vem nele: sem `font.family`, quem saía do
    // Matrix para o Cyberpunk ficava com a mono (as fotos do roteiro pegaram).
    // A letra do sistema é a que o tema tinha antes da primeira aplicação.
    static LETRA_DO_SISTEMA: OnceLock<gpui_kit::SharedString> = OnceLock::new();
    let do_sistema = LETRA_DO_SISTEMA
        .get_or_init(|| Theme::global(cx).font_family.clone())
        .clone();
    let letra = if escolha == Escolha::Matrix {
        Theme::global(cx).mono_font_family.clone()
    } else {
        do_sistema.clone()
    };
    let tema = Theme::global_mut(cx);
    tema.apply_config(&Rc::new(tema_da_paleta(
        "RecordarFotos Claro",
        ThemeMode::Light,
        PALETAS[0],
        Some(do_sistema),
    )));
    tema.apply_config(&Rc::new(tema_da_paleta(
        escolha.nome_do_escuro(),
        ThemeMode::Dark,
        PALETAS[escura as usize],
        Some(letra),
    )));
    PALETA_AGORA.store(if modo.is_dark() { escura } else { 0 }, Ordering::Relaxed);
    Theme::change(modo, window, cx);
}

/// As cores que a tela pede pelo nome, e o tema do `gpui-component` não tem
/// nome para. Seguem o modo da tela.
pub mod cores {
    use super::{paleta, paleta_atual, PALETA_AGORA};
    use domain::value_objects::ColorLabel;
    use gpui_kit::Hsla;
    use std::sync::atomic::Ordering;

    fn cor(rgb: u32) -> Hsla {
        gpui_kit::rgb(rgb).into()
    }

    fn escuro() -> bool {
        PALETA_AGORA.load(Ordering::Relaxed) != 0
    }

    /// O fundo de tudo que encosta numa imagem.
    pub fn poco() -> Hsla {
        cor(paleta_atual().poco)
    }

    /// Âmbar: o recorte aceso da galeria, sessão e balcão.
    pub fn quente() -> Hsla {
        cor(paleta::AMBAR_400)
    }

    /// Âmbar para texto: `text-amber-700 dark:text-amber-400`.
    pub fn quente_clara() -> Hsla {
        cor(if escuro() {
            paleta::AMBAR_400
        } else {
            paleta::AMBAR_700
        })
    }

    /// O texto que fica legível sobre o âmbar.
    pub fn sobre_quente() -> Hsla {
        cor(paleta::SOBRE_CLARO)
    }

    /// O selo âmbar: `border-amber-300 bg-amber-50 text-amber-800`, e no escuro
    /// `bg-amber-950/40 text-amber-300`. Devolve (fundo, borda, texto).
    pub fn selo_ambar() -> (Hsla, Hsla, Hsla) {
        if escuro() {
            (
                cor(paleta::AMBAR_950).opacity(0.4),
                cor(paleta::AMBAR_300),
                cor(paleta::AMBAR_300),
            )
        } else {
            (
                cor(paleta::AMBAR_50),
                cor(paleta::AMBAR_300),
                cor(paleta::AMBAR_800),
            )
        }
    }

    /// O selo esmeralda ("Cliente já abriu"). Devolve (fundo, borda, texto).
    pub fn selo_esmeralda() -> (Hsla, Hsla, Hsla) {
        if escuro() {
            (
                cor(paleta::ESMERALDA_950).opacity(0.4),
                cor(paleta::ESMERALDA_300),
                cor(paleta::ESMERALDA_300),
            )
        } else {
            (
                cor(paleta::ESMERALDA_50),
                cor(paleta::ESMERALDA_300),
                cor(paleta::ESMERALDA_800),
            )
        }
    }

    /// O selo céu ("Aguardando o cliente"). Devolve (fundo, borda, texto).
    pub fn selo_ceu() -> (Hsla, Hsla, Hsla) {
        if escuro() {
            (
                cor(paleta::CEU_950).opacity(0.4),
                cor(paleta::CEU_300),
                cor(paleta::CEU_300),
            )
        } else {
            (
                cor(paleta::CEU_50),
                cor(paleta::CEU_300),
                cor(paleta::CEU_800),
            )
        }
    }

    /// O cartão em destaque ("Pago no pós-venda"). Devolve (fundo, borda, texto).
    pub fn destaque_esmeralda() -> (Hsla, Hsla, Hsla) {
        if escuro() {
            (
                cor(paleta::ESMERALDA_950).opacity(0.2),
                cor(paleta::ESMERALDA_900),
                cor(paleta::ESMERALDA_400),
            )
        } else {
            (
                cor(paleta::ESMERALDA_50).opacity(0.5),
                cor(paleta::ESMERALDA_300),
                cor(paleta::ESMERALDA_700),
            )
        }
    }

    /// Verde de "deu certo" em texto.
    pub fn sucesso() -> Hsla {
        cor(if escuro() {
            paleta::ESMERALDA_400
        } else {
            paleta::ESMERALDA_700
        })
    }

    /// ☁️ O selo "na nuvem" sobre a foto: `text-emerald-400/80` do site —
    /// discreto de propósito, porque é o normal.
    pub fn nuvem() -> Hsla {
        cor(paleta::ESMERALDA_400).opacity(0.8)
    }

    /// O azul da seleção na grade.
    pub fn selecao() -> Hsla {
        cor(paleta::AZUL_500)
    }

    /// O âmbar de "atenção" mais forte (`amber-500`).
    pub fn atencao() -> Hsla {
        cor(paleta::AMBAR_500)
    }

    /// O véu atrás de um diálogo: `bg-black/50` do site.
    pub fn veu() -> Hsla {
        gpui_kit::rgba(0x00000080).into()
    }

    /// A estrela acesa.
    pub fn nota() -> Hsla {
        cor(paleta::NOTA)
    }

    /// O sinalizador de escolhida.
    pub fn escolhida() -> Hsla {
        cor(paleta::ESCOLHIDA)
    }

    /// O sinalizador de rejeitada.
    pub fn rejeitada() -> Hsla {
        cor(paleta::REJEITADA)
    }

    /// Claro ou escuro sobre uma cor — o que tiver mais contraste (WCAG).
    pub fn texto_sobre(fundo: Hsla) -> Hsla {
        let claro: Hsla = gpui_kit::rgb(0xffffff).into();
        let escuro: Hsla = cor(paleta::SOBRE_CLARO);
        if contraste(fundo, escuro) >= contraste(fundo, claro) {
            escuro
        } else {
            claro
        }
    }

    fn contraste(a: Hsla, b: Hsla) -> f32 {
        let (la, lb) = (luminancia(a), luminancia(b));
        let (mais, menos) = if la > lb { (la, lb) } else { (lb, la) };
        (mais + 0.05) / (menos + 0.05)
    }

    fn luminancia(cor: Hsla) -> f32 {
        let rgba = gpui_kit::Rgba::from(cor);
        let linear = |c: f32| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(rgba.r) + 0.7152 * linear(rgba.g) + 0.0722 * linear(rgba.b)
    }

    /// A cor de uma das cinco etiquetas, pelo nome que o banco guarda.
    pub fn etiqueta(nome: &str) -> Option<Hsla> {
        Some(match ColorLabel::from_name(nome).ok()? {
            ColorLabel::Red => cor(paleta::ETIQUETA_VERMELHA),
            ColorLabel::Yellow => cor(paleta::ETIQUETA_AMARELA),
            ColorLabel::Green => cor(paleta::ETIQUETA_VERDE),
            ColorLabel::Blue => cor(paleta::ETIQUETA_AZUL),
            ColorLabel::Purple => cor(paleta::ETIQUETA_ROXA),
        })
    }
}

/// O botão da família âmbar: o recorte aceso da galeria.
pub fn botao_quente(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(cores::quente())
        .foreground(cores::sobre_quente())
        .hover(gpui_kit::rgb(AMBAR_300).into())
        .active(gpui_kit::rgb(AMBAR_500).into())
}

/// O tema do site num modo — o que o app sempre instalou.
#[cfg(test)]
fn tema_do_site(modo: ThemeMode) -> ThemeConfig {
    match modo {
        ThemeMode::Light => tema_da_paleta("RecordarFotos Claro", modo, &paleta::CLARO, None),
        ThemeMode::Dark => tema_da_paleta("RecordarFotos Escuro", modo, &paleta::ESCURO, None),
    }
}

fn tema_da_paleta(
    nome: &str,
    modo: ThemeMode,
    p: &paleta::Paleta,
    letra: Option<gpui_kit::SharedString>,
) -> ThemeConfig {
    let mut config = serde_json::Map::new();
    config.insert("is_default".into(), Value::Bool(true));
    config.insert("name".into(), Value::String(nome.into()));
    config.insert(
        "mode".into(),
        Value::String(if modo.is_dark() { "dark" } else { "light" }.into()),
    );
    config.insert("radius".into(), Value::from(RAIO));
    config.insert("radius.lg".into(), Value::from(RAIO_GRANDE));
    config.insert("font.size".into(), Value::from(LETRA));
    // O `shadow-xs` dos botões e campos do shadcn.
    config.insert("shadow".into(), Value::Bool(true));
    if let Some(letra) = letra {
        config.insert("font.family".into(), Value::String(letra.to_string()));
    }
    let mut cores = cores_do_esquema(p);
    // 🪟 O véu atrás do `Sheet` e do `Dialog` do gpui-kit — o mesmo
    // `bg-black/50` do véu que o app desenha ([`cores::veu`]). Fora da lista
    // de [`cores`] porque tem transparência, e aquela é só `#rrggbb`. Sem ele,
    // valia o do gpui-kit, quase transparente: a gaveta do atendimento abria
    // sem escurecer a grade atrás.
    if let Value::Object(mapa) = &mut cores {
        mapa.insert("overlay".into(), Value::String(VEU.into()));
    }
    config.insert("colors".into(), cores);
    serde_json::from_value(Value::Object(config))
        .expect("o tema do site tem de ser legível — `tema_e_legivel` confere isso")
}

/// Chave do esquema oficial à esquerda, cor da paleta à direita.
fn cores(p: &paleta::Paleta) -> Vec<(&'static str, u32)> {
    vec![
        ("background", p.fundo),
        ("foreground", p.texto),
        ("border", p.borda),
        ("window.border", p.borda),
        ("muted.background", p.apagado),
        ("muted.foreground", p.texto_apagado),
        ("secondary.background", p.acento),
        ("secondary.hover.background", p.acento),
        ("secondary.active.background", p.acento),
        ("secondary.foreground", p.sobre_acento),
        ("accent.background", p.acento),
        ("accent.foreground", p.sobre_acento),
        ("primary.background", p.primaria),
        ("primary.hover.background", p.primaria_pairando),
        ("primary.active.background", p.primaria_pairando),
        ("primary.foreground", p.sobre_primaria),
        ("ring", p.anel),
        ("selection.background", AZUL_500),
        ("caret", p.texto),
        ("input.border", p.campo),
        ("popover.background", p.cartao),
        ("popover.foreground", p.texto),
        ("sidebar.background", p.lateral),
        ("sidebar.foreground", p.texto_lateral),
        ("sidebar.border", p.borda_lateral),
        ("sidebar.accent.background", p.acento_lateral),
        ("sidebar.accent.foreground", p.texto_lateral),
        ("sidebar.primary.background", p.marca),
        ("sidebar.primary.foreground", p.sobre_marca),
        ("list.background", p.fundo),
        ("list.hover.background", p.acento),
        ("list.active.background", p.acento),
        ("list.active.border", p.anel),
        ("list.even.background", p.fundo),
        ("list.head.background", p.fundo),
        ("tab_bar.background", p.fundo),
        ("tab_bar.segmented.background", p.apagado),
        ("tab.background", p.fundo),
        ("tab.foreground", p.texto_apagado),
        ("tab.active.background", p.cartao),
        ("tab.active.foreground", p.texto),
        ("drag.border", AZUL_500),
        ("drop_target.background", AZUL_500),
        ("accordion.background", p.fundo),
        ("group_box.background", p.cartao),
        ("group_box.foreground", p.texto),
        ("group_box.title.foreground", p.texto_apagado),
        ("table.background", p.fundo),
        ("table.head.background", p.fundo),
        ("table.head.foreground", p.texto),
        ("table.hover.background", p.acento),
        ("table.active.background", p.acento),
        ("table.active.border", p.anel),
        ("table.even.background", p.fundo),
        ("table.row.border", p.borda),
        // O âmbar dos controles deslizantes do site (revelação e zoom).
        ("slider.background", AMBAR_400),
        ("slider.thumb.background", 0xffffff),
        ("progress.bar.background", p.primaria),
        ("switch.background", p.primaria),
        ("switch.thumb.background", p.fundo),
        ("skeleton.background", p.apagado),
        ("scrollbar.background", p.fundo),
        ("scrollbar.thumb.background", p.rolagem),
        ("scrollbar.thumb.hover.background", p.anel),
        ("title_bar.background", p.fundo),
        ("title_bar.border", p.borda),
        ("danger.background", p.destrutiva),
        ("danger.hover.background", p.destrutiva),
        ("danger.active.background", p.destrutiva),
        ("danger.foreground", 0xffffff),
        ("warning.background", AMBAR_400),
        ("warning.hover.background", AMBAR_300),
        ("warning.active.background", AMBAR_500),
        ("warning.foreground", SOBRE_CLARO),
        ("success.background", ESMERALDA_400),
        ("success.hover.background", ESMERALDA_300),
        ("success.active.background", ESMERALDA_700),
        ("success.foreground", SOBRE_CLARO),
        ("info.background", p.primaria),
        ("info.hover.background", p.primaria_pairando),
        ("info.active.background", p.primaria_pairando),
        ("info.foreground", p.sobre_primaria),
        ("link", p.texto),
        ("link.hover", p.texto_apagado),
        ("link.active", p.texto),
        ("base.red", ETIQUETA_VERMELHA),
        ("base.yellow", ETIQUETA_AMARELA),
        ("base.green", ETIQUETA_VERDE),
        ("base.blue", ETIQUETA_AZUL),
        ("base.magenta", ETIQUETA_ROXA),
    ]
}

/// O véu dos modais, em `#rrggbbaa`: preto a 50%.
const VEU: &str = "#00000080";

/// `0x1a2b3c` → `"#1a2b3c"`. Sempre seis dígitos.
fn hex(cor: u32) -> String {
    format!("#{cor:06x}")
}

fn cores_do_esquema(p: &paleta::Paleta) -> Value {
    Value::Object(
        cores(p)
            .into_iter()
            .map(|(chave, cor)| (chave.to_string(), Value::String(hex(cor))))
            .collect(),
    )
}

/// Como **escrever** a tecla modificadora dos atalhos.
pub fn modificador() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const MODOS: [ThemeMode; 2] = [ThemeMode::Light, ThemeMode::Dark];

    fn paleta_de(modo: ThemeMode) -> &'static paleta::Paleta {
        if modo.is_dark() {
            &paleta::ESCURO
        } else {
            &paleta::CLARO
        }
    }

    #[test]
    fn tema_e_legivel() {
        for modo in MODOS {
            let config = tema_do_site(modo);
            assert_eq!(config.mode, modo);
            assert_eq!(config.radius, Some(RAIO));
            assert_eq!(config.font_size, Some(LETRA));
        }
    }

    #[test]
    fn todo_hexadecimal_tem_seis_digitos() {
        for modo in MODOS {
            for (chave, cor) in cores(paleta_de(modo)) {
                let escrito = hex(cor);
                assert!(
                    escrito.len() == 7 && escrito[1..].chars().all(|c| c.is_ascii_hexdigit()),
                    "`{chave}` = {escrito:?} não é #rrggbb"
                );
            }
        }
    }

    #[test]
    fn nenhuma_chave_e_ignorada() {
        for modo in MODOS {
            let config = tema_do_site(modo);
            let de_volta = serde_json::to_value(&config.colors).expect("as cores devem serializar");
            for (chave, cor) in cores(paleta_de(modo)) {
                assert_eq!(
                    de_volta.get(chave).and_then(|v| v.as_str()),
                    Some(hex(cor).as_str()),
                    "`{chave}` não sobreviveu à leitura — nome fora do esquema do gpui-component"
                );
            }
        }
    }

    /// O véu do `Sheet` e do `Dialog` é o do app, e o gpui-kit o lê.
    #[test]
    fn o_veu_dos_modais_e_o_do_app() {
        let config = tema_do_site(ThemeMode::Light);
        let de_volta = serde_json::to_value(&config.colors).expect("as cores devem serializar");
        assert_eq!(de_volta.get("overlay").and_then(|v| v.as_str()), Some(VEU));
        let veu = gpui_kit::Rgba::from(cores::veu());
        assert!((veu.a - 0.5).abs() < 0.01, "o véu do app mudou: {veu:?}");
    }

    #[test]
    fn nenhuma_chave_repetida() {
        let mut vistas = std::collections::HashSet::new();
        for (chave, _) in cores(&paleta::ESCURO) {
            assert!(
                vistas.insert(chave),
                "`{chave}` aparece duas vezes na lista"
            );
        }
    }

    /// Os valores são os do `globals.css` do site, convertidos de oklch.
    #[test]
    fn as_cores_sao_as_do_site() {
        assert_eq!(paleta::CLARO.primaria, 0x445566, "a marca do legado");
        assert_eq!(paleta::ESCURO.fundo, 0x0a0a0a, "oklch(0.145 0 0)");
        assert_eq!(paleta::ESCURO.cartao, 0x171717, "oklch(0.205 0 0)");
        assert_eq!(
            paleta::ESCURO.primaria,
            0x8fa8c0,
            "oklch(0.72 0.045 248.63)"
        );
        assert_eq!(paleta::ESCURO.marca, 0x1447e6, "oklch(0.488 0.243 264.376)");
    }

    /// 🟩🌆 Matrix e Cyberpunk: toda cor é `#rrggbb`, toda chave existe no
    /// esquema, o texto é legível (WCAG, 4,5:1) e o `bg-primary/80` é a conta.
    #[test]
    fn matrix_e_cyberpunk_sao_legiveis() {
        for (nome, p) in [
            ("Matrix", &paleta::MATRIX),
            ("Cyberpunk", &paleta::CYBERPUNK),
        ] {
            let config = tema_da_paleta(nome, ThemeMode::Dark, p, Some("Menlo".into()));
            assert_eq!(config.font_family.as_deref(), Some("Menlo"), "{nome}");
            let de_volta = serde_json::to_value(&config.colors).expect("as cores devem serializar");
            for (chave, cor) in cores(p) {
                assert_eq!(
                    de_volta.get(chave).and_then(|v| v.as_str()),
                    Some(hex(cor).as_str()),
                    "{nome}: `{chave}` fora do esquema"
                );
            }
            for (par, fundo, frente) in [
                ("texto", p.fundo, p.texto),
                ("apagado", p.fundo, p.texto_apagado),
                ("texto no cartão", p.cartao, p.texto),
                ("primária", p.primaria, p.sobre_primaria),
                ("acento", p.acento, p.sobre_acento),
                ("menu lateral", p.lateral, p.texto_lateral),
                ("marca", p.marca, p.sobre_marca),
            ] {
                let razao = contraste(fundo, frente);
                assert!(
                    razao >= 4.5,
                    "{nome}, `{par}`: {razao:.2}:1 — abaixo de 4,5:1"
                );
            }
            let canal = |c: u32, d: u32| ((c >> d) & 0xff) as f32;
            for d in [16, 8, 0] {
                let conta = canal(p.primaria, d) * 0.8 + canal(p.fundo, d) * 0.2;
                assert!(
                    (canal(p.primaria_pairando, d) - conta).abs() <= 1.,
                    "{nome}: `primaria_pairando` não é o primary/80"
                );
            }
        }
    }

    #[test]
    fn matrix_e_cyberpunk_sao_escuros_e_ficam_lembrados() {
        for (nome, escolha) in [
            ("matrix", Escolha::Matrix),
            ("cyberpunk", Escolha::Cyberpunk),
        ] {
            assert_eq!(Escolha::do_nome(nome), Some(escolha));
            assert_eq!(escolha.modo(WindowAppearance::Light), ThemeMode::Dark);
            let pasta = tempfile::tempdir().unwrap();
            let arquivo = pasta.path().join("tema.json");
            guardar_escolha(&arquivo, escolha);
            assert_eq!(escolha_guardada(&arquivo), escolha);
        }
        assert_eq!(
            PALETAS[Escolha::Matrix.paleta_escura() as usize],
            &paleta::MATRIX
        );
        assert_eq!(
            PALETAS[Escolha::Cyberpunk.paleta_escura() as usize],
            &paleta::CYBERPUNK
        );
        assert_eq!(
            PALETAS[Escolha::Escuro.paleta_escura() as usize],
            &paleta::ESCURO
        );
    }

    #[test]
    fn a_escolha_vira_modo_e_sobrevive_ao_arquivo() {
        assert_eq!(
            Escolha::Sistema.modo(WindowAppearance::Light),
            ThemeMode::Light
        );
        assert_eq!(
            Escolha::Sistema.modo(WindowAppearance::VibrantDark),
            ThemeMode::Dark
        );
        assert_eq!(
            Escolha::Claro.modo(WindowAppearance::Dark),
            ThemeMode::Light
        );
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = pasta.path().join("sub").join("tema.json");
        assert_eq!(escolha_guardada(&arquivo), Escolha::Sistema);
        guardar_escolha(&arquivo, Escolha::Escuro);
        assert_eq!(escolha_guardada(&arquivo), Escolha::Escuro);
        std::fs::write(&arquivo, b"lixo").unwrap();
        assert_eq!(escolha_guardada(&arquivo), Escolha::Sistema);
    }

    /// Texto sobre as cores de destaque continua legível (WCAG, 4,5:1).
    #[test]
    fn contraste_do_texto_sobre_cor() {
        let pares = [
            (
                "primária clara",
                CLARO_PRIMARIA,
                paleta::CLARO.sobre_primaria,
            ),
            (
                "primária escura",
                paleta::ESCURO.primaria,
                paleta::ESCURO.sobre_primaria,
            ),
            ("âmbar", AMBAR_400, SOBRE_CLARO),
            ("texto no claro", paleta::CLARO.fundo, paleta::CLARO.texto),
            (
                "apagado no claro",
                paleta::CLARO.fundo,
                paleta::CLARO.texto_apagado,
            ),
            (
                "texto no escuro",
                paleta::ESCURO.fundo,
                paleta::ESCURO.texto,
            ),
            (
                "apagado no escuro",
                paleta::ESCURO.fundo,
                paleta::ESCURO.texto_apagado,
            ),
            (
                "marca escura",
                paleta::ESCURO.marca,
                paleta::ESCURO.sobre_marca,
            ),
        ];
        for (nome, fundo, frente) in pares {
            let razao = contraste(fundo, frente);
            assert!(razao >= 4.5, "`{nome}`: {razao:.2}:1 — abaixo de 4,5:1");
        }
    }

    const CLARO_PRIMARIA: u32 = paleta::CLARO.primaria;

    #[test]
    fn toda_etiqueta_do_dominio_tem_cor() {
        for etiqueta in domain::value_objects::ColorLabel::all() {
            let nome = etiqueta.name();
            assert!(cores::etiqueta(nome).is_some(), "`{nome}` não tem cor");
            let como_no_banco = format!("{}{}", nome[..1].to_uppercase(), &nome[1..]);
            assert_eq!(cores::etiqueta(&como_no_banco), cores::etiqueta(nome));
        }
        assert!(cores::etiqueta("laranja").is_none());
    }

    #[test]
    fn o_texto_escolhido_pela_luminancia_e_legivel() {
        for (nome, fundo) in [
            ("vermelha", ETIQUETA_VERMELHA),
            ("amarela", ETIQUETA_AMARELA),
            ("verde", ETIQUETA_VERDE),
            ("azul", ETIQUETA_AZUL),
            ("roxa", ETIQUETA_ROXA),
            ("âmbar", AMBAR_400),
            ("nota", NOTA),
        ] {
            let texto = cores::texto_sobre(gpui_kit::rgb(fundo).into());
            let rgba = gpui_kit::Rgba::from(texto);
            let como_u32 = ((rgba.r * 255.).round() as u32) << 16
                | ((rgba.g * 255.).round() as u32) << 8
                | (rgba.b * 255.).round() as u32;
            let razao = contraste(fundo, como_u32);
            assert!(
                razao >= 4.5,
                "sobre a etiqueta {nome} o texto dá {razao:.2}:1"
            );
        }
    }

    fn luminancia(cor: u32) -> f32 {
        let linear = |c: f32| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let canal = |deslocamento: u32| ((cor >> deslocamento) & 0xff) as f32 / 255.;
        0.2126 * linear(canal(16)) + 0.7152 * linear(canal(8)) + 0.0722 * linear(canal(0))
    }

    fn contraste(a: u32, b: u32) -> f32 {
        let (la, lb) = (luminancia(a), luminancia(b));
        let (mais, menos) = if la > lb { (la, lb) } else { (lb, la) };
        (mais + 0.05) / (menos + 0.05)
    }
}
