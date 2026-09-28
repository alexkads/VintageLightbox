//! O template do app, na língua do <https://ui.shadcn.com/create>.
//!
//! # 🎨 Como o visual é escolhido
//!
//! O dono monta o visual no `/create` do shadcn, copia o código do preset
//! (`b2fA`, `bIkeymG`…) ou o endereço da página, e cola em
//! `crates/ui-gpui/template.toml`. A compilação lê esse arquivo, e **todo
//! balcão** abre com o mesmo visual — o operador só escolhe claro ou escuro.
//!
//! Os eixos e os valores são os do shadcn, com os mesmos nomes, na mesma ordem
//! (`packages/shadcn/src/preset/preset.ts`, commit `98a1fe6` de 2026-09-21).
//! Dois valores são **da casa** e não existem lá:
//!
//! | Eixo | Da casa | Por quê |
//! |---|---|---|
//! | `tema` | `recordarfotos` | o primário `#445566` da marca, que o site usa |
//! | `fonte` | `sistema` | a letra do sistema operacional, que o app usa desde sempre |
//!
//! Um preset com valor da casa não tem código do `/create` — [`Preset::codigo`]
//! devolve `None`.
//!
//! # 🔑 Este arquivo compila em dois lugares
//!
//! Aqui, e dentro do `build.rs` (`#[path]`), que decide quais fontes e quais
//! ícones embutir. Por isso ele **não usa nada fora da biblioteca padrão**: sem
//! `serde`, sem `toml`, sem `gpui`. O `template.toml` é lido por [`ler`], que
//! entende o pedaço de TOML que o arquivo precisa — `chave = "valor"` e
//! comentário.

#![allow(dead_code)]

use std::fmt;

// ── Os valores, na ordem do shadcn ─────────────────────────────────────────
// 🚨 A ordem é o código: o índice de cada valor é o que vai nos bits do preset.
// Nunca reordenar; o shadcn só acrescenta no fim.

pub const ESTILOS: &[&str] = &[
    "nova", "vega", "maia", "lyra", "mira", "luma", "sera", "rhea",
];

pub const CORES_BASE: &[&str] = &[
    "neutral", "stone", "zinc", "gray", "mauve", "olive", "mist", "taupe",
];

pub const TEMAS: &[&str] = &[
    "neutral", "stone", "zinc", "gray", "amber", "blue", "cyan", "emerald", "fuchsia", "green",
    "indigo", "lime", "orange", "pink", "purple", "red", "rose", "sky", "teal", "violet", "yellow",
    "mauve", "olive", "mist", "taupe",
];

pub const ICONES: &[&str] = &["lucide", "hugeicons", "tabler", "phosphor", "remixicon"];

pub const FONTES: &[&str] = &[
    "inter",
    "noto-sans",
    "nunito-sans",
    "figtree",
    "roboto",
    "raleway",
    "dm-sans",
    "public-sans",
    "outfit",
    "jetbrains-mono",
    "geist",
    "geist-mono",
    "lora",
    "merriweather",
    "playfair-display",
    "noto-serif",
    "roboto-slab",
    "oxanium",
    "manrope",
    "space-grotesk",
    "montserrat",
    "ibm-plex-sans",
    "source-sans-3",
    "instrument-sans",
    "eb-garamond",
    "instrument-serif",
];

/// `inherit` na frente das fontes: o título usa a letra do corpo.
pub const HERDAR: &str = "inherit";

pub const RAIOS: &[&str] = &["default", "none", "small", "medium", "large"];

pub const DESTAQUES_DO_MENU: &[&str] = &["subtle", "bold"];

pub const CORES_DO_MENU: &[&str] = &[
    "default",
    "inverted",
    "default-translucent",
    "inverted-translucent",
];

/// O primário da marca (`--primary` do site).
pub const TEMA_DA_CASA: &str = "recordarfotos";
/// A letra do sistema (`.SystemUIFont` no macOS).
pub const FONTE_DO_SISTEMA: &str = "sistema";

/// A escolha inteira — um valor por eixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub estilo: &'static str,
    pub cor_base: &'static str,
    pub tema: &'static str,
    pub cor_dos_graficos: &'static str,
    pub icones: &'static str,
    pub fonte: &'static str,
    pub fonte_dos_titulos: &'static str,
    pub raio: &'static str,
    pub destaque_do_menu: &'static str,
    pub cor_do_menu: &'static str,
}

/// Os campos do código, do bit menos significativo para o mais.
/// `(nome no /create, bits)`.
const CAMPOS: &[(&str, u32)] = &[
    ("menuColor", 3),
    ("menuAccent", 3),
    ("radius", 4),
    ("font", 6),
    ("iconLibrary", 6),
    ("theme", 6),
    ("baseColor", 6),
    ("style", 6),
    // Só na versão `b`:
    ("chartColor", 6),
    ("fontHeading", 5),
];

const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// O que deu errado ao ler o template. Aparece na compilação, com a linha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erro(pub String);

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn erro(texto: impl Into<String>) -> Erro {
    Erro(texto.into())
}

impl Default for Preset {
    /// O `b0` do shadcn: o índice 0 de todo eixo.
    fn default() -> Self {
        Self {
            estilo: ESTILOS[0],
            cor_base: CORES_BASE[0],
            tema: TEMAS[0],
            cor_dos_graficos: TEMAS[0],
            icones: ICONES[0],
            fonte: FONTES[0],
            fonte_dos_titulos: HERDAR,
            raio: RAIOS[0],
            destaque_do_menu: DESTAQUES_DO_MENU[0],
            cor_do_menu: CORES_DO_MENU[0],
        }
    }
}

impl Preset {
    /// O visual do app **antes** de haver template: o do site
    /// (`frontend/components.json` = `base-nova`, `neutral`, `lucide`), com o
    /// primário da marca e a letra do sistema.
    pub fn da_casa() -> Self {
        Self {
            estilo: "nova",
            tema: TEMA_DA_CASA,
            cor_dos_graficos: "neutral",
            fonte: FONTE_DO_SISTEMA,
            ..Self::default()
        }
    }

    /// Os oito presets com nome do shadcn (`DEFAULT_PRESETS`).
    pub fn com_nome(nome: &str) -> Option<Self> {
        let estilo = *ESTILOS.iter().find(|e| **e == nome)?;
        let (fonte, icones) = match estilo {
            "nova" => ("geist", "lucide"),
            "maia" => ("figtree", "hugeicons"),
            "lyra" => ("jetbrains-mono", "phosphor"),
            "mira" => ("inter", "hugeicons"),
            "sera" => ("noto-sans", "lucide"),
            _ => ("inter", "lucide"),
        };
        let mut preset = Self {
            estilo,
            fonte,
            icones,
            ..Self::default()
        };
        if estilo == "sera" {
            preset.cor_base = "taupe";
            preset.tema = "taupe";
            preset.cor_dos_graficos = "taupe";
            preset.fonte_dos_titulos = "playfair-display";
        }
        Some(preset)
    }

    /// Lê o código do `/create` (`b…`, ou o antigo `a…`).
    pub fn do_codigo(codigo: &str) -> Result<Self, Erro> {
        let codigo = codigo.trim();
        let versao = codigo.chars().next().ok_or_else(|| erro("código vazio"))?;
        let campos = match versao {
            'a' => &CAMPOS[..8],
            'b' => CAMPOS,
            _ => {
                return Err(erro(format!(
                    "`{codigo}` não é código do /create: começa com `a` ou `b`"
                )))
            }
        };
        if codigo.len() < 2 || codigo.len() > 10 {
            return Err(erro(format!("`{codigo}` não tem o tamanho de um código")));
        }
        let mut bits: u64 = 0;
        for c in codigo[1..].bytes() {
            let valor = BASE62
                .iter()
                .position(|b| *b == c)
                .ok_or_else(|| erro(format!("`{codigo}` tem `{}` fora da base 62", c as char)))?;
            bits = bits
                .checked_mul(62)
                .and_then(|b| b.checked_add(valor as u64))
                .ok_or_else(|| erro(format!("`{codigo}` é grande demais")))?;
        }
        let mut preset = Self::default();
        let mut deslocamento = 0;
        for (campo, largura) in campos {
            let indice = ((bits >> deslocamento) & ((1 << largura) - 1)) as usize;
            deslocamento += largura;
            // Índice fora da lista é o padrão, como no `decodePreset`.
            let de = |lista: &'static [&'static str]| *lista.get(indice).unwrap_or(&lista[0]);
            match *campo {
                "menuColor" => preset.cor_do_menu = de(CORES_DO_MENU),
                "menuAccent" => preset.destaque_do_menu = de(DESTAQUES_DO_MENU),
                "radius" => preset.raio = de(RAIOS),
                "font" => preset.fonte = de(FONTES),
                "iconLibrary" => preset.icones = de(ICONES),
                "theme" => preset.tema = de(TEMAS),
                "baseColor" => preset.cor_base = de(CORES_BASE),
                "style" => preset.estilo = de(ESTILOS),
                "chartColor" => preset.cor_dos_graficos = de(TEMAS),
                "fontHeading" => {
                    preset.fonte_dos_titulos = if indice == 0 {
                        HERDAR
                    } else {
                        *FONTES.get(indice - 1).unwrap_or(&HERDAR)
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(preset)
    }

    /// O código do `/create` deste preset — `None` se ele usa valor da casa.
    pub fn codigo(&self) -> Option<String> {
        let mut bits: u64 = 0;
        let mut deslocamento = 0;
        for (campo, largura) in CAMPOS {
            let indice = match *campo {
                "menuColor" => indice(CORES_DO_MENU, self.cor_do_menu)?,
                "menuAccent" => indice(DESTAQUES_DO_MENU, self.destaque_do_menu)?,
                "radius" => indice(RAIOS, self.raio)?,
                "font" => indice(FONTES, self.fonte)?,
                "iconLibrary" => indice(ICONES, self.icones)?,
                "theme" => indice(TEMAS, self.tema)?,
                "baseColor" => indice(CORES_BASE, self.cor_base)?,
                "style" => indice(ESTILOS, self.estilo)?,
                "chartColor" => indice(TEMAS, self.cor_dos_graficos)?,
                "fontHeading" if self.fonte_dos_titulos == HERDAR => 0,
                "fontHeading" => indice(FONTES, self.fonte_dos_titulos)? + 1,
                _ => unreachable!(),
            };
            bits |= (indice as u64) << deslocamento;
            deslocamento += largura;
        }
        let mut texto = Vec::new();
        loop {
            texto.push(BASE62[(bits % 62) as usize]);
            bits /= 62;
            if bits == 0 {
                break;
            }
        }
        texto.push(b'b');
        texto.reverse();
        Some(String::from_utf8(texto).expect("base 62 é ASCII"))
    }

    /// As travas do `/create`: `lyra` e `sera` não têm canto, e `rhea` não
    /// aceita o raio grande (`radius-picker.tsx`, `design-system-provider.tsx`).
    pub fn com_travas(mut self) -> Self {
        match self.estilo {
            "lyra" | "sera" => self.raio = "none",
            "rhea" if self.raio == "large" => self.raio = "default",
            _ => {}
        }
        self
    }

    /// A forma que atravessa do `build.rs` para o app: os parâmetros de
    /// endereço do `/create`, que também servem para abri-lo.
    pub fn parametros(&self) -> String {
        format!(
            "style={}&baseColor={}&theme={}&chartColor={}&iconLibrary={}&font={}&fontHeading={}&radius={}&menuAccent={}&menuColor={}",
            self.estilo,
            self.cor_base,
            self.tema,
            self.cor_dos_graficos,
            self.icones,
            self.fonte,
            self.fonte_dos_titulos,
            self.raio,
            self.destaque_do_menu,
            self.cor_do_menu,
        )
    }

    /// Aplica um parâmetro do endereço do `/create` (`style=nova`) ou uma
    /// chave do `template.toml` (`estilo = "nova"`) — os dois nomes valem.
    fn definir(&mut self, chave: &str, valor: &str) -> Result<(), Erro> {
        let valor = valor.trim();
        let (campo, lista, extra): (&mut &'static str, &'static [&'static str], &[&'static str]) =
            match chave {
                "style" | "estilo" => (&mut self.estilo, ESTILOS, &[]),
                "baseColor" | "cor_base" => (&mut self.cor_base, CORES_BASE, &[]),
                "theme" | "tema" => (&mut self.tema, TEMAS, &[TEMA_DA_CASA]),
                "chartColor" | "cor_dos_graficos" => {
                    (&mut self.cor_dos_graficos, TEMAS, &[TEMA_DA_CASA])
                }
                "iconLibrary" | "icones" => (&mut self.icones, ICONES, &[]),
                "font" | "fonte" => (&mut self.fonte, FONTES, &[FONTE_DO_SISTEMA]),
                "fontHeading" | "fonte_dos_titulos" => (
                    &mut self.fonte_dos_titulos,
                    FONTES,
                    &[HERDAR, FONTE_DO_SISTEMA],
                ),
                "radius" | "raio" => (&mut self.raio, RAIOS, &[]),
                "menuAccent" | "destaque_do_menu" => {
                    (&mut self.destaque_do_menu, DESTAQUES_DO_MENU, &[])
                }
                "menuColor" | "cor_do_menu" => (&mut self.cor_do_menu, CORES_DO_MENU, &[]),
                // O que o `/create` põe no endereço e não é visual.
                "base" | "template" | "rtl" | "pointer" | "item" | "size" | "custom" => {
                    return Ok(())
                }
                _ => return Err(erro(format!("`{chave}` não é eixo do template"))),
            };
        *campo = lista
            .iter()
            .chain(extra)
            .find(|v| **v == valor)
            .copied()
            .ok_or_else(|| {
                erro(format!(
                    "`{chave}` não aceita `{valor}` — vale um destes: {}",
                    lista
                        .iter()
                        .chain(extra)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
        Ok(())
    }

    /// Lê o que o `/create` entrega: o código, o nome de um preset, o comando
    /// (`npx shadcn init --preset b…`) ou o endereço da página.
    pub fn do_create(texto: &str) -> Result<Self, Erro> {
        let texto = texto.trim();
        let texto = texto
            .rsplit_once("--preset")
            .map(|(_, depois)| depois.split_whitespace().next().unwrap_or(""))
            .unwrap_or(texto);
        if let Some((_, consulta)) = texto.split_once('?') {
            let consulta = consulta.split('#').next().unwrap_or("");
            let pares: Vec<(&str, &str)> = consulta
                .split('&')
                .filter_map(|par| par.split_once('='))
                .collect();
            // O código primeiro; os parâmetros soltos por cima dele, como na página.
            let mut preset = match pares.iter().find(|(chave, _)| *chave == "preset") {
                Some((_, codigo)) => Self::do_create(codigo)?,
                None => Self::default(),
            };
            for (chave, valor) in pares {
                if chave != "preset" {
                    preset.definir(chave, valor)?;
                }
            }
            return Ok(preset);
        }
        if let Some(preset) = Self::com_nome(texto) {
            return Ok(preset);
        }
        Self::do_codigo(texto)
    }
}

fn indice(lista: &[&str], valor: &str) -> Option<usize> {
    lista.iter().position(|v| *v == valor)
}

/// Lê o `template.toml`: `preset = "…"` (o que o `/create` entrega) e, por cima
/// dele, um eixo por linha. Sem `preset`, parte do [`Preset::da_casa`].
pub fn ler(texto: &str) -> Result<Preset, Erro> {
    let mut linhas = Vec::new();
    for (numero, linha) in texto.lines().enumerate() {
        let linha = linha.split('#').next().unwrap_or("").trim();
        if linha.is_empty() {
            continue;
        }
        let (chave, valor) = linha
            .split_once('=')
            .ok_or_else(|| erro(format!("linha {}: falta `=`", numero + 1)))?;
        let valor = valor.trim();
        let valor = valor
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| {
                erro(format!(
                    "linha {}: o valor vai entre aspas (`{} = \"{}\"`)",
                    numero + 1,
                    chave.trim(),
                    valor
                ))
            })?;
        linhas.push((numero + 1, chave.trim(), valor));
    }
    let mut preset = match linhas.iter().find(|(_, chave, _)| *chave == "preset") {
        Some((numero, _, valor)) => {
            Preset::do_create(valor).map_err(|e| erro(format!("linha {numero}: {e}")))?
        }
        None => Preset::da_casa(),
    };
    for (numero, chave, valor) in linhas {
        if chave != "preset" {
            preset
                .definir(chave, valor)
                .map_err(|e| erro(format!("linha {numero}: {e}")))?;
        }
    }
    Ok(preset.com_travas())
}

/// Lê a forma de [`Preset::parametros`] — a que o `build.rs` grava no binário.
pub fn dos_parametros(texto: &str) -> Result<Preset, Erro> {
    Preset::do_create(&format!("?{texto}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Os códigos que o encoder do próprio shadcn calcula para os presets com
    /// nome (`scratchpad/shadcn-create/codes.ts`, `encodePreset` de verdade).
    const DO_SHADCN: &[(&str, &str)] = &[
        ("nova", "b2fA"),
        ("vega", "bIkeymG"),
        ("maia", "bbVKFP6"),
        ("lyra", "buFznsW"),
        ("mira", "b1D0eCA4"),
        ("luma", "b1VlIttI"),
        ("rhea", "b27GcrRo"),
        ("sera", "b4xFeBLg4O"),
    ];

    #[test]
    fn os_codigos_batem_com_os_do_shadcn() {
        for (nome, codigo) in DO_SHADCN {
            let preset = Preset::com_nome(nome).unwrap();
            assert_eq!(preset.codigo().as_deref(), Some(*codigo), "{nome}");
            assert_eq!(Preset::do_codigo(codigo).unwrap(), preset, "{nome}");
        }
    }

    #[test]
    fn b0_e_o_padrao() {
        assert_eq!(Preset::do_codigo("b0").unwrap(), Preset::default());
        assert_eq!(Preset::default().codigo().as_deref(), Some("b0"));
    }

    #[test]
    fn o_codigo_a_nao_tem_graficos_nem_titulo() {
        let preset = Preset::do_codigo("a0").unwrap();
        assert_eq!(preset.fonte_dos_titulos, HERDAR);
        assert_eq!(preset.cor_dos_graficos, "neutral");
    }

    #[test]
    fn le_o_que_o_create_entrega() {
        let vega = Preset::com_nome("vega").unwrap();
        for forma in [
            "bIkeymG",
            "vega",
            "npx shadcn@latest init --preset bIkeymG --template next",
            "https://ui.shadcn.com/create?preset=bIkeymG&item=preview",
            "https://ui.shadcn.com/create?style=vega&font=inter",
        ] {
            assert_eq!(Preset::do_create(forma).unwrap(), vega, "{forma}");
        }
        let por_cima =
            Preset::do_create("https://ui.shadcn.com/create?preset=bIkeymG&theme=emerald").unwrap();
        assert_eq!(por_cima.tema, "emerald");
        assert_eq!(por_cima.estilo, "vega");
    }

    #[test]
    fn o_template_vazio_e_o_visual_da_casa() {
        assert_eq!(ler("# nada\n\n").unwrap(), Preset::da_casa());
        assert_eq!(Preset::da_casa().codigo(), None, "tem valor da casa");
    }

    #[test]
    fn eixo_do_arquivo_vale_por_cima_do_preset() {
        let preset =
            ler("preset = \"bIkeymG\" # vega\ntema = \"recordarfotos\"\nfonte = \"sistema\"\n")
                .unwrap();
        assert_eq!(preset.estilo, "vega");
        assert_eq!(preset.tema, TEMA_DA_CASA);
        assert_eq!(preset.fonte, FONTE_DO_SISTEMA);
    }

    #[test]
    fn erro_diz_a_linha_e_o_que_vale() {
        let Erro(texto) = ler("estilo = \"nova\"\nraio = \"enorme\"").unwrap_err();
        assert!(texto.starts_with("linha 2:"), "{texto}");
        assert!(texto.contains("large"), "{texto}");
        assert!(ler("cor = \"azul\"").is_err(), "eixo que não existe");
        assert!(ler("estilo = nova").is_err(), "sem aspas");
        assert!(ler("preset = \"x9\"").is_err(), "código de outra versão");
    }

    #[test]
    fn as_travas_do_create() {
        assert_eq!(
            ler("estilo = \"lyra\"\nraio = \"large\"").unwrap().raio,
            "none"
        );
        assert_eq!(ler("estilo = \"sera\"").unwrap().raio, "none");
        assert_eq!(
            ler("estilo = \"rhea\"\nraio = \"large\"").unwrap().raio,
            "default"
        );
        assert_eq!(
            ler("estilo = \"maia\"\nraio = \"large\"").unwrap().raio,
            "large"
        );
    }

    #[test]
    fn os_parametros_voltam_ao_mesmo_preset() {
        for preset in [
            Preset::da_casa(),
            Preset::com_nome("sera").unwrap(),
            Preset::do_codigo("bbVKFP6").unwrap(),
        ] {
            assert_eq!(dos_parametros(&preset.parametros()).unwrap(), preset);
        }
    }
}
