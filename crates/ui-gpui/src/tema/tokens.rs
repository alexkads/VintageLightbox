//! As cores do template: os tokens do shadcn, em oklch, somados na ordem do
//! `/create` e convertidos para `0xrrggbb`.
//!
//! # De onde vêm
//!
//! `template/cores.json` é o `THEMES` de `apps/v4/registry/themes.ts` do
//! shadcn (commit `98a1fe6`), como está lá, mais a entrada `recordarfotos`:
//! o `--primary` do `frontend/src/app/globals.css` do site, nos dois modos, e o
//! `sidebar-accent` do claro um tom abaixo do site (`#f0f0f0`), que é o que o
//! menu lateral do app sempre acendeu.
//!
//! # Como se somam (`buildRegistryTheme`, `apps/v4/registry/config.ts`)
//!
//! 1. Os tokens da cor base (`neutral`, `stone`…).
//! 2. Por cima, os do tema (`blue`, `emerald`… ou a própria cor base).
//! 3. Com `destaque_do_menu = "bold"`, `accent` passa a ser o `primary`.
//!
//! 🎨 A cor dos gráficos (`chart-*`) é lida e ignorada: o app não desenha
//! gráfico nenhum.
//!
//! # ⚠️ Cor com transparência vira opaca aqui
//!
//! O `gpui-component` lê a cor como está, e a borda do escuro do shadcn é
//! "branco a 10%". Aqui ela é somada à superfície em que fica — o fundo para
//! `border` e `input`, o menu lateral para `sidebar-border` —, que é o que o
//! navegador faz na hora de pintar.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::preset::Preset;

#[derive(Deserialize)]
struct Modos {
    light: HashMap<String, String>,
    dark: HashMap<String, String>,
}

fn temas() -> &'static HashMap<String, Modos> {
    static TEMAS: OnceLock<HashMap<String, Modos>> = OnceLock::new();
    TEMAS.get_or_init(|| {
        serde_json::from_str(include_str!("../../template/cores.json"))
            .expect("template/cores.json é legível — `todo_tema_do_preset_existe` confere")
    })
}

/// Uma cor em sRGB, com opacidade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub fn opaca(rgb: u32) -> Self {
        let canal = |d: u32| ((rgb >> d) & 0xff) as f32 / 255.;
        Self {
            r: canal(16),
            g: canal(8),
            b: canal(0),
            a: 1.,
        }
    }

    /// Esta cor pintada sobre `fundo`, como o navegador faz.
    pub fn sobre(self, fundo: u32) -> u32 {
        let f = Self::opaca(fundo);
        let mistura = |c: f32, f: f32| c * self.a + f * (1. - self.a);
        Self {
            r: mistura(self.r, f.r),
            g: mistura(self.g, f.g),
            b: mistura(self.b, f.b),
            a: 1.,
        }
        .hex()
    }

    pub fn hex(self) -> u32 {
        let canal = |c: f32| (c.clamp(0., 1.) * 255.).round() as u32;
        canal(self.r) << 16 | canal(self.g) << 8 | canal(self.b)
    }
}

/// `oklch(L C H)` ou `oklch(L C H / 10%)` → sRGB. Fora do gamute, corta o canal.
pub fn oklch(texto: &str) -> Option<Rgba> {
    let dentro = texto.trim().strip_prefix("oklch(")?.strip_suffix(')')?;
    let (cor, alfa) = match dentro.split_once('/') {
        Some((cor, alfa)) => (cor, Some(alfa.trim())),
        None => (dentro, None),
    };
    let numeros: Vec<f32> = cor
        .split_whitespace()
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    let [l, c, h] = numeros[..] else {
        return None;
    };
    let a = match alfa {
        None => 1.,
        Some(p) if p.ends_with('%') => p.trim_end_matches('%').parse::<f32>().ok()? / 100.,
        Some(p) => p.parse().ok()?,
    };
    let (sen, cos) = h.to_radians().sin_cos();
    let (la, lb) = (c * cos, c * sen);
    // Oklab → LMS → sRGB linear (Björn Ottosson).
    let l_ = (l + 0.396_337_78 * la + 0.215_803_76 * lb).powi(3);
    let m_ = (l - 0.105_561_346 * la - 0.063_854_17 * lb).powi(3);
    let s_ = (l - 0.089_484_18 * la - 1.291_485_5 * lb).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let b = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    let gama = |c: f32| {
        let c = c.clamp(0., 1.);
        if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1. / 2.4) - 0.055
        }
    };
    Some(Rgba {
        r: gama(r),
        g: gama(g),
        b: gama(b),
        a,
    })
}

/// Os tokens de um modo, já somados: nome do CSS (`primary`) → oklch.
pub struct Tokens(HashMap<String, String>);

impl Tokens {
    pub fn do_preset(preset: &Preset, escuro: bool) -> Self {
        let modo = |nome: &str| -> HashMap<String, String> {
            let Some(modos) = temas().get(nome) else {
                // Só `gray`: o shadcn o reserva no código e não lhe dá tokens.
                eprintln!("🎨 [Template] `{nome}` não tem cores no shadcn; fica o `neutral`");
                return HashMap::new();
            };
            if escuro {
                modos.dark.clone()
            } else {
                modos.light.clone()
            }
        };
        let mut tokens = modo("neutral");
        tokens.extend(modo(preset.cor_base));
        tokens.extend(modo(preset.tema));
        if preset.destaque_do_menu == "bold" {
            for (destino, origem) in [
                ("accent", "primary"),
                ("accent-foreground", "primary-foreground"),
            ] {
                let cor = tokens[origem].clone();
                tokens.insert(destino.into(), cor);
            }
        }
        Self(tokens)
    }

    pub fn cor(&self, nome: &str) -> Rgba {
        let texto = self
            .0
            .get(nome)
            .unwrap_or_else(|| panic!("o shadcn sempre define `{nome}`"));
        oklch(texto).unwrap_or_else(|| panic!("`{nome}` = {texto:?} não é oklch"))
    }

    /// O token opaco, em `0xrrggbb`.
    pub fn hex(&self, nome: &str) -> u32 {
        self.cor(nome).hex()
    }

    /// O token pintado sobre uma superfície.
    pub fn sobre(&self, nome: &str, fundo: u32) -> u32 {
        self.cor(nome).sobre(fundo)
    }

    /// Mesma cor e mesmo matiz do token, com outra luminosidade.
    pub fn com_luz(&self, nome: &str, luz: f32) -> u32 {
        let texto = &self.0[nome];
        let dentro = texto
            .trim()
            .trim_start_matches("oklch(")
            .trim_end_matches(')');
        let dentro = dentro.split('/').next().unwrap_or(dentro);
        let mut partes = dentro.split_whitespace().skip(1);
        let (c, h) = (partes.next().unwrap_or("0"), partes.next().unwrap_or("0"));
        oklch(&format!("oklch({luz} {c} {h})"))
            .expect("montado de um oklch válido")
            .hex()
    }
}

/// Os nomes de tema que o preset pode pedir e que `cores.json` precisa ter.
#[cfg(test)]
pub fn nomes() -> Vec<&'static str> {
    temas().keys().map(|k| k.as_str()).collect()
}
