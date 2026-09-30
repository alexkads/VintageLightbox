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
//! ## 🌹 Âmbar Rouge — o tema fixo claro
//!
//! Pedido do dono em 2026-09-28 (*"Crie um tema feminino chamado Ambar
//! Rouge"*). Fundo rosa-pó `#fff7f5`, letra vinho `#3b1624`, a marca e os
//! botões em rouge `#9e2a4a`, e o que acende (filtro, seleção, foco) em âmbar
//! dourado `#e8a33d` — daí o nome. É o único tema fixo **claro**: por isso o
//! modo não se deduz mais do índice da paleta (`ESCURO_AGORA`), e o claro
//! também escolhe a sua (`Escolha::paleta_clara`).
//!
//! ## 🎞️ Lightroom — o terceiro tema fixo
//!
//! Pedido do dono em 2026-09-28 (*"Eu quero um novo tema inspirado no
//! Lightroom"*), aprovado numa POC em WASM antes de entrar aqui. Os cinzas do
//! Lightroom Classic: painéis `#2b2b2b`, janela `#1e1e1e`, o palco cinza médio
//! `#5a5a5a` atrás da foto, o cabeçalho da sanfona `#353535` e um único azul,
//! o do foco. A cor fica só onde ela diz algo — o trilho dos sliders.
//!
//! 🎚️ **Os componentes eram quase monocromáticos**, e só a paleta não
//! bastava: o `Slider` do gpui-kit pinta uma cor e preenche da esquerda. Por
//! isso o app desenha o slider com as peças do `gpui_kit::base`
//! (`slider_da_casa`): o trilho de cada controle (`controles::Trilho`) vai do
//! azul ao amarelo na Temperatura, na cor da faixa no HSL, e o preenchimento
//! parte do neutro. Isso vale **em todos os temas**; no Claro e no Escuro o
//! trilho liso continua o âmbar do kit (`o_slider_dos_temas_de_antes_e_o_de_sempre`),
//! e no Matrix e no Cyberpunk é a marca de cada um.
//!
//! 🔥 **Desde 2026-09-28 os dois não têm âmbar.** O que acende (filtro,
//! seleção, slider), a estrela e a família quente (levada, editada, não salva,
//! o aviso do gpui-kit) são a marca de cada um — `quente_da_marca`. Antes o
//! âmbar do site valia em todo tema, e no Matrix ele era a única cor fora do
//! verde (dono: *"No tema Matrix existe cor âmbar?"*). O verde de "deu certo",
//! as etiquetas e os trilhos coloridos dos sliders continuam: dizem a cor da
//! foto.
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

pub mod fontes;
pub mod letra;
pub mod medidas;
pub mod preset;
pub mod tokens;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;

use medidas::Medidas;
use preset::Preset;

use gpui_kit::component::button::ButtonCustomVariant;
use gpui_kit::component::{Theme, ThemeConfig, ThemeMode};
use gpui_kit::{App, Window, WindowAppearance};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// As paletas, em `0xrrggbb`: as do template (Claro e Escuro), as dos temas
/// fixos (Matrix e Cyberpunk) e as cores fixas da fotografia.
mod paleta {
    use super::preset::Preset;
    use super::tokens::{Rgba, Tokens};

    /// Os tokens do shadcn, num modo.
    #[derive(Debug, Clone, PartialEq)]
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
        /// O `secondary` do shadcn — ver `cores`: é o do botão fantasma.
        pub secundaria: u32,
        pub sobre_secundaria: u32,
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
        /// 🎚️ O trilho da barra dos sliders. `None` é o do gpui-kit: o
        /// preenchimento a 20% sobre o que estiver atrás.
        pub trilho: Option<u32>,
        /// O preenchimento do slider, do neutro ao valor.
        pub preenchimento: u32,
        /// O punho do slider.
        pub punho: u32,
        /// Barra fina e punho pequeno, como no Lightroom — ou as medidas do
        /// `Slider` do gpui-kit.
        pub slider_fino: bool,
        /// O fundo do cabeçalho de cada painel sanfonado. `None`: nenhum.
        pub sanfona: Option<u32>,
        /// ✨ **O que está aceso**: o filtro escolhido, a foto marcada, o
        /// botão ligado, o trilho do zoom. Nos temas do site é o âmbar; no
        /// Lightroom, o cinza-claro. Não confundir com [`super::cores::quente`],
        /// que é **situação** da foto (levada, editada, não salva) e continua
        /// âmbar em todo tema.
        pub aceso: u32,
        pub sobre_aceso: u32,
        pub aceso_pairando: u32,
        pub aceso_ativo: u32,
        /// ⭐ A estrela acesa da nota.
        pub estrela: u32,
        /// 🎞️ A grade de fotos em células, como a Biblioteca do Lightroom.
        /// `None`: a foto solta sobre o fundo, como no site.
        pub celulas: Option<Celulas>,
        /// Cantos retos (o Lightroom quase não arredonda nada).
        pub cantos_retos: bool,
        /// Painéis da coluna corridos, sem moldura, com o título à direita.
        pub paineis_corridos: bool,
        /// 🔥 **A família quente é a marca do tema**, e não o âmbar do site:
        /// a situação da foto (levada, editada, não salva), o selo de atenção,
        /// o aviso do gpui-kit (`warning`). No Matrix tudo é verde fósforo,
        /// no Cyberpunk o amarelo neon — o âmbar ali era um segundo amarelo,
        /// ou uma cor que o tema não tem (dono, 28/09: *"No tema Matrix
        /// existe cor âmbar?"*).
        pub quente_da_marca: bool,
    }

    /// 🎞️ As cores da grade em células.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Celulas {
        /// O vão entre as células.
        pub grade: u32,
        pub celula: u32,
        /// A que está na seleção, sem ser a do foco.
        pub marcada: u32,
        /// A do foco — clara, e o texto nela escuro.
        pub foco: u32,
    }

    /// A paleta de um modo, montada dos tokens do template.
    ///
    /// | Campo | Token do shadcn |
    /// |---|---|
    /// | `primaria_pairando` | `bg-primary/80` sobre o fundo |
    /// | `borda`, `campo` | `border`, `input` sobre o fundo |
    /// | `borda_lateral` | `sidebar-border` sobre o menu lateral |
    /// | `poco` | `muted` no claro, `card` no escuro |
    /// | `rolagem` | o matiz da borda na luz 0,87 (claro) ou 0,371 (escuro) — o `neutral-300` e o `neutral-700` do Tailwind quando a base é `neutral` |
    ///
    /// Com `cor_do_menu = "inverted"`, o menu lateral usa os tokens do escuro
    /// também no claro (ver `template.toml`).
    pub fn montar(preset: &Preset, escuro: bool) -> Paleta {
        let t = Tokens::do_preset(preset, escuro);
        let fundo = t.hex("background");
        let primaria = t.cor("primary");
        let invertido = !escuro && preset.cor_do_menu.starts_with("inverted");
        let lado = Tokens::do_preset(preset, escuro || invertido);
        let lateral = lado.hex("sidebar");
        Paleta {
            fundo,
            texto: t.hex("foreground"),
            cartao: t.hex("card"),
            primaria: primaria.hex(),
            primaria_pairando: Rgba { a: 0.8, ..primaria }.sobre(fundo),
            sobre_primaria: t.hex("primary-foreground"),
            apagado: t.hex("muted"),
            texto_apagado: t.hex("muted-foreground"),
            acento: t.hex("accent"),
            sobre_acento: t.hex("accent-foreground"),
            secundaria: t.hex("secondary"),
            sobre_secundaria: t.hex("secondary-foreground"),
            destrutiva: t.hex("destructive"),
            borda: t.sobre("border", fundo),
            campo: t.sobre("input", fundo),
            anel: t.hex("ring"),
            lateral,
            texto_lateral: lado.hex("sidebar-foreground"),
            marca: lado.hex("sidebar-primary"),
            sobre_marca: lado.hex("sidebar-primary-foreground"),
            acento_lateral: lado.hex("sidebar-accent"),
            borda_lateral: lado.sobre("sidebar-border", lateral),
            poco: if escuro {
                t.hex("card")
            } else {
                t.hex("muted")
            },
            rolagem: t.com_luz("border", if escuro { 0.371 } else { 0.87 }),
            trilho: None,
            preenchimento: AMBAR_400,
            punho: 0xffffff,
            slider_fino: false,
            sanfona: None,
            aceso: AMBAR_400,
            sobre_aceso: SOBRE_CLARO,
            aceso_pairando: AMBAR_300,
            aceso_ativo: AMBAR_500,
            estrela: NOTA,
            celulas: None,
            cantos_retos: false,
            paineis_corridos: false,
            quente_da_marca: false,
        }
    }

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
        secundaria: 0x0d2e18,
        sobre_secundaria: 0x8dffb0,
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
        trilho: None,
        preenchimento: 0x00ff41,
        punho: 0x8dffb0,
        slider_fino: false,
        sanfona: None,
        // 🟩 Tudo o que acende é o verde fósforo (ver `quente_da_marca`).
        aceso: 0x00ff41,
        sobre_aceso: 0x021a08,
        aceso_pairando: 0x5cff8f,
        aceso_ativo: 0x01cf35,
        estrela: 0x00ff41,
        celulas: None,
        cantos_retos: false,
        paineis_corridos: false,
        quente_da_marca: true,
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
        secundaria: 0x24245a,
        sobre_secundaria: 0x05d9e8,
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
        trilho: None,
        preenchimento: 0xfcee0a,
        // O punho no ciano do foco: as três cores do neon numa barra só.
        punho: 0x05d9e8,
        slider_fino: false,
        sanfona: None,
        // 🌆 Tudo o que acende é o amarelo neon da marca, e não um segundo
        // amarelo (o âmbar), com o texto azul-noite por cima.
        aceso: 0xfcee0a,
        sobre_aceso: 0x0a0a1f,
        aceso_pairando: 0xfdf35c,
        aceso_ativo: 0xccc00e,
        estrela: 0xfcee0a,
        celulas: None,
        cantos_retos: false,
        paineis_corridos: false,
        quente_da_marca: true,
    };

    /// 🎞️ Lightroom: cinzas graduados, o palco cinza médio atrás da foto, e a
    /// cor só onde ela diz algo (os trilhos dos sliders, o histograma).
    ///
    /// 📏 **Os cinzas são medidos** nas capturas do Lightroom Classic que o
    /// dono mandou em 29/09 (*"quero que fique o mais fiel possível"*) — a
    /// cor mais frequente de cada região, e não de memória:
    ///
    /// | Região do Lightroom | Medido | Campo |
    /// |---|---|---|
    /// | Faixa do título do painel, moldura | `#333333` | `fundo`, `sanfona` |
    /// | Corpo do painel | `#505050` | `lateral` (e a sanfona aberta) |
    /// | Linha entre seções | `#1e1e1e` | `borda` |
    /// | Diálogo de importação | `#424242` | `cartao` |
    /// | Botão em segmento aceso | `#858585` | `acento` |
    /// | Grade: vão / célula / marcada / em foco | `#424242` / `#6c6c6c` / `#858585` / `#a9a9a9`–`#cbcbcb` | `celulas` |
    ///
    /// ⚠️ A célula fica um pouco abaixo da medida (`#5a5a5a`): em `#6c6c6c` o
    /// nome da foto, em cinza-claro, não chegava a 4,5 de contraste.
    ///
    /// A primeira versão (28/09) era um Lightroom lembrado, e saiu escura
    /// demais: painéis `#2b2b2b` sobre `#1e1e1e`.
    pub const LIGHTROOM: Paleta = Paleta {
        fundo: 0x333333,
        texto: 0xd4d4d4,
        cartao: 0x424242,
        primaria: 0xcfcfcf,
        // `bg-primary/80` sobre o fundo.
        primaria_pairando: 0xb0b0b0,
        sobre_primaria: 0x1a1a1a,
        apagado: 0x3d3d3d,
        texto_apagado: 0xa8a8a8,
        // O segmento aceso e a linha sob o ponteiro, um degrau acima do painel.
        acento: 0x626262,
        sobre_acento: 0xf2f2f2,
        secundaria: 0x484848,
        sobre_secundaria: 0xd8d8d8,
        destrutiva: 0xe34850,
        // A linha quase preta que separa as seções.
        borda: 0x1e1e1e,
        // Os campos do Lightroom são afundados: moldura escura.
        campo: 0x262626,
        // O único azul: o foco, como o da Adobe.
        anel: 0x378ef0,
        // O corpo do painel, o cinza médio das colunas do Lightroom.
        lateral: 0x505050,
        texto_lateral: 0xd4d4d4,
        // O quadrado do "Lr".
        marca: 0x31a8ff,
        sobre_marca: 0x001e36,
        acento_lateral: 0x626262,
        borda_lateral: 0x2a2a2a,
        // O palco: o cinza médio que o Lightroom põe atrás da foto.
        poco: 0x5a5a5a,
        // A barra de rolagem do Lightroom é clara sobre o cinza.
        rolagem: 0x8a8a8a,
        // O sulco escuro do slider, sobre o corpo do painel (#505050).
        trilho: Some(0x1f1f1f),
        preenchimento: 0xa8a8a8,
        punho: 0xd0d0d0,
        slider_fino: true,
        sanfona: Some(0x333333),
        // O aceso do Lightroom: cinza-claro com letra escura.
        aceso: 0xcbcbcb,
        sobre_aceso: 0x1a1a1a,
        aceso_pairando: 0xdedede,
        aceso_ativo: 0xb4b4b4,
        // As estrelas do Lightroom são claras, e não âmbar.
        estrela: 0xe6e6e6,
        // A Biblioteca: células cinza sobre o vão escuro, a marcada um
        // degrau acima, e a em foco quase branca, como no Lightroom.
        celulas: Some(Celulas {
            grade: 0x424242,
            celula: 0x5a5a5a,
            marcada: 0x858585,
            foco: 0xb4b4b4,
        }),
        cantos_retos: true,
        paineis_corridos: true,
        // A situação da foto continua âmbar, como as etiquetas do Lightroom.
        quente_da_marca: false,
    };

    /// 🌹 Âmbar Rouge: o tema claro e feminino — rosa-pó, letra vinho, a marca
    /// em rouge e o que acende em âmbar dourado (dono, 28/09: *"Crie um tema
    /// feminino chamado Ambar Rouge"*).
    pub const AMBAR_ROUGE: Paleta = Paleta {
        fundo: 0xfff7f5,
        texto: 0x3b1624,
        cartao: 0xffffff,
        primaria: 0x9e2a4a,
        // `bg-primary/80` sobre o fundo.
        primaria_pairando: 0xb1536c,
        sobre_primaria: 0xfff7f5,
        apagado: 0xfbecef,
        texto_apagado: 0x87566a,
        acento: 0xf9dde3,
        sobre_acento: 0x5c1a33,
        secundaria: 0xf9e1e6,
        sobre_secundaria: 0x5c1a33,
        destrutiva: 0xc0262d,
        borda: 0xf0d3d9,
        campo: 0xe6c0c9,
        // O foco em âmbar dourado.
        anel: 0xd99a3a,
        lateral: 0xfbeef0,
        texto_lateral: 0x3b1624,
        marca: 0x9e2a4a,
        sobre_marca: 0xfff7f5,
        acento_lateral: 0xf6dde3,
        borda_lateral: 0xefd0d7,
        // Atrás da foto, um rosado quase neutro: não pode tingir o julgamento
        // da cor.
        poco: 0xf2e9ea,
        rolagem: 0xe0b8c1,
        // O slider em rouge, com o trilho a 20% dele e o punho branco.
        trilho: None,
        preenchimento: 0x9e2a4a,
        punho: 0xffffff,
        slider_fino: false,
        sanfona: None,
        // O que acende é o âmbar dourado, com a letra vinho.
        aceso: 0xe8a33d,
        sobre_aceso: 0x3b1624,
        aceso_pairando: 0xf0bb63,
        aceso_ativo: 0xcc8a26,
        // A estrela em âmbar mel, escura o bastante para o fundo claro.
        estrela: 0xb8741a,
        celulas: None,
        cantos_retos: false,
        paineis_corridos: false,
        // A situação da foto no âmbar do site: combina com o nome.
        quente_da_marca: false,
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
    /// Texto escuro sobre cinza-claro, sem o tom quente do [`SOBRE_CLARO`].
    pub const SOBRE_ESCURO_NEUTRO: u32 = 0x1a1a1a;
}

use paleta::*;

/// 🎨 O template deste binário — o `template.toml` que o `build.rs` leu (ou o
/// `VLB_TEMPLATE` de quem compilou), gravado em `VLB_TEMPLATE_RESOLVIDO`.
///
/// Fixo na compilação de propósito: as fontes e os ícones do template são
/// embutidos pelo `build.rs`, e um template trocado depois não os teria.
pub fn template() -> &'static Preset {
    static TEMPLATE: OnceLock<Preset> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        preset::dos_parametros(env!("VLB_TEMPLATE_RESOLVIDO"))
            .expect("o build.rs só grava template que ele mesmo leu")
    })
}

/// As medidas do estilo do template: alturas, respiros, cantos e letra.
pub fn medidas() -> &'static Medidas {
    static MEDIDAS: OnceLock<Medidas> = OnceLock::new();
    MEDIDAS.get_or_init(|| Medidas::do_preset(template()))
}

/// 🔑 **O canto de uma tela, no raio do template.** As telas foram desenhadas
/// sobre o `--radius` de 10 px do site: `tema::canto(8.)` é o `rounded-md`
/// dali, e continua 8 px no visual da casa. Com `raio = "large"` vira 11,2; com
/// `none`, ou num estilo quadrado (`lyra`, `sera`), some.
pub fn canto(px_com_raio_10: f32) -> gpui_kit::Pixels {
    no_tema(medidas().canto_da_tela(px_com_raio_10))
}

/// 🎞️ **Um canto já medido, levado ao tema**: o Lightroom quase não
/// arredonda — no máximo 2 px. Nos outros temas, o canto passa como veio.
pub fn no_tema(canto: gpui_kit::Pixels) -> gpui_kit::Pixels {
    let maximo = gpui_kit::px(2.);
    if paleta_atual().cantos_retos && canto > maximo {
        maximo
    } else {
        canto
    }
}

/// Se o tema pede cantos retos — a pílula (`rounded_full`) vira retângulo.
pub fn cantos_retos() -> bool {
    paleta_atual().cantos_retos
}

/// As duas paletas do template: (claro, escuro).
fn paletas_do_template() -> &'static (paleta::Paleta, paleta::Paleta) {
    static PALETAS: OnceLock<(paleta::Paleta, paleta::Paleta)> = OnceLock::new();
    PALETAS.get_or_init(|| {
        (
            paleta::montar(template(), false),
            paleta::montar(template(), true),
        )
    })
}

/// Qual paleta está na tela: a de [`Escolha::paleta_escura`] no escuro, a de
/// [`Escolha::paleta_clara`] no claro. Guardada como o índice de [`paletas`].
static PALETA_AGORA: AtomicU8 = AtomicU8::new(1);

/// Se a tela está no escuro agora. As cores sem `cx` ([`cores`]) leem daqui.
///
/// 🚨 **Não se deduz do índice da paleta**: até o Âmbar Rouge todo tema fixo
/// era escuro, e "índice diferente de 0" queria dizer escuro. Um tema fixo
/// claro pintaria os selos e o texto âmbar na versão do escuro.
static ESCURO_AGORA: AtomicBool = AtomicBool::new(true);

/// As paletas, na ordem do índice de [`PALETA_AGORA`]: as duas do template e
/// os dois temas fixos.
fn paletas() -> [&'static paleta::Paleta; 6] {
    let (claro, escuro) = paletas_do_template();
    [
        claro,
        escuro,
        &paleta::MATRIX,
        &paleta::CYBERPUNK,
        &paleta::LIGHTROOM,
        &paleta::AMBAR_ROUGE,
    ]
}

fn paleta_atual() -> &'static paleta::Paleta {
    let todas = paletas();
    todas[PALETA_AGORA.load(Ordering::Relaxed) as usize % todas.len()]
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
    /// 🎞️ Os cinzas do Lightroom, com os trilhos coloridos.
    Lightroom,
    /// 🌹 Rosa-pó, vinho, rouge e âmbar dourado — o único tema fixo claro.
    #[serde(rename = "ambar_rouge")]
    AmbarRouge,
}

impl Escolha {
    pub fn do_nome(nome: &str) -> Option<Self> {
        match nome {
            "claro" => Some(Self::Claro),
            "escuro" => Some(Self::Escuro),
            "sistema" => Some(Self::Sistema),
            "matrix" => Some(Self::Matrix),
            "cyberpunk" => Some(Self::Cyberpunk),
            "lightroom" => Some(Self::Lightroom),
            "ambar_rouge" | "ambar-rouge" => Some(Self::AmbarRouge),
            _ => None,
        }
    }

    /// O modo que vale com esta escolha e esta aparência do sistema.
    pub fn modo(self, aparencia: WindowAppearance) -> ThemeMode {
        match self {
            Self::Claro | Self::AmbarRouge => ThemeMode::Light,
            Self::Escuro | Self::Matrix | Self::Cyberpunk | Self::Lightroom => ThemeMode::Dark,
            Self::Sistema => match aparencia {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
                WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
            },
        }
    }

    /// O índice em [`paletas`] da paleta que esta escolha pinta no escuro.
    fn paleta_escura(self) -> u8 {
        match self {
            Self::Matrix => 2,
            Self::Cyberpunk => 3,
            Self::Lightroom => 4,
            Self::Claro | Self::Escuro | Self::Sistema | Self::AmbarRouge => 1,
        }
    }

    /// O índice em [`paletas`] da paleta que esta escolha pinta no claro.
    fn paleta_clara(self) -> u8 {
        match self {
            Self::AmbarRouge => 5,
            _ => 0,
        }
    }

    fn nome_do_claro(self) -> &'static str {
        match self {
            Self::AmbarRouge => "Âmbar Rouge",
            _ => "RecordarFotos Claro",
        }
    }

    fn nome_do_escuro(self) -> &'static str {
        match self {
            Self::Matrix => "Matrix",
            Self::Cyberpunk => "Cyberpunk",
            Self::Lightroom => "Lightroom",
            Self::Claro | Self::Escuro | Self::Sistema | Self::AmbarRouge => "RecordarFotos Escuro",
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
    let clara = escolha.paleta_clara();
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
    // 🎨 A fonte do template (a do sistema, no visual da casa).
    let do_template = fontes::familia(template().fonte)
        .map(gpui_kit::SharedString::from)
        .unwrap_or_else(|| do_sistema.clone());
    let letra = if escolha == Escolha::Matrix {
        Theme::global(cx).mono_font_family.clone()
    } else {
        do_template.clone()
    };
    let tema = Theme::global_mut(cx);
    let todas = paletas();
    tema.apply_config(&Rc::new(tema_da_paleta(
        escolha.nome_do_claro(),
        ThemeMode::Light,
        todas[clara as usize],
        Some(do_template),
    )));
    tema.apply_config(&Rc::new(tema_da_paleta(
        escolha.nome_do_escuro(),
        ThemeMode::Dark,
        todas[escura as usize],
        Some(letra),
    )));
    // 📣 Os toasts do canto de baixo (chatbot e agenda) sobem acima do
    // rodapé da janela, em vez de cair em cima da versão.
    tema.notification.margins.bottom = gpui_kit::px(16. + crate::app::rodape::ALTURA_DO_RODAPE);
    PALETA_AGORA.store(
        if modo.is_dark() { escura } else { clara },
        Ordering::Relaxed,
    );
    ESCURO_AGORA.store(modo.is_dark(), Ordering::Relaxed);
    Theme::change(modo, window, cx);
}

/// As cores que a tela pede pelo nome, e o tema do `gpui-component` não tem
/// nome para. Seguem o modo da tela.
pub mod cores {
    use super::{paleta, paleta_atual, ESCURO_AGORA};
    use domain::value_objects::ColorLabel;
    use gpui_kit::Hsla;
    use std::sync::atomic::Ordering;

    fn cor(rgb: u32) -> Hsla {
        gpui_kit::rgb(rgb).into()
    }

    fn escuro() -> bool {
        ESCURO_AGORA.load(Ordering::Relaxed)
    }

    /// O fundo de tudo que encosta numa imagem.
    pub fn poco() -> Hsla {
        cor(paleta_atual().poco)
    }

    /// 🎚️ O slider do tema: (trilho, preenchimento, punho, fino).
    pub fn slider() -> (Hsla, Hsla, Hsla, bool) {
        let p = paleta_atual();
        let preenchimento = cor(p.preenchimento);
        let trilho = p
            .trilho
            .map(cor)
            .unwrap_or_else(|| preenchimento.opacity(0.2));
        (trilho, preenchimento, cor(p.punho), p.slider_fino)
    }

    /// O fundo do cabeçalho de um painel sanfonado, se o tema tiver um.
    pub fn sanfona() -> Option<Hsla> {
        paleta_atual().sanfona.map(cor)
    }

    /// ✨ O que está aceso (seleção, filtro escolhido, botão ligado).
    pub fn aceso() -> Hsla {
        cor(paleta_atual().aceso)
    }

    /// O texto legível sobre o [`aceso`].
    pub fn sobre_aceso() -> Hsla {
        cor(paleta_atual().sobre_aceso)
    }

    /// 🎞️ O vão da grade em células, se o tema desenhar assim.
    pub fn grade_em_celulas() -> Option<Hsla> {
        paleta_atual().celulas.map(|c| cor(c.grade))
    }

    /// 🎞️ O fundo de uma célula da grade e a tinta do texto sobre ela, se o
    /// tema desenhar células. A tinta é o texto do tema enquanto ele se lê
    /// (4,5); na marcada e na em foco, claras como no Lightroom, é escura.
    pub fn celula(marcada: bool, em_foco: bool) -> Option<(Hsla, Hsla)> {
        let p = paleta_atual();
        let c = p.celulas?;
        let fundo = cor(if em_foco {
            c.foco
        } else if marcada {
            c.marcada
        } else {
            c.celula
        });
        let texto = cor(p.texto);
        let tinta = if contraste(fundo, texto) >= 4.5 {
            texto
        } else {
            cor(paleta::SOBRE_ESCURO_NEUTRO)
        };
        Some((fundo, tinta))
    }

    /// Painéis corridos, sem moldura e com o título à direita (Lightroom).
    pub fn paineis_corridos() -> bool {
        paleta_atual().paineis_corridos
    }

    /// Âmbar: o recorte aceso da galeria, sessão e balcão.
    pub fn quente() -> Hsla {
        match marca_quente() {
            Some((marca, _)) => marca,
            None => cor(paleta::AMBAR_400),
        }
    }

    /// A marca e o texto sobre ela, quando o tema faz da marca a família
    /// quente (Matrix, Cyberpunk).
    fn marca_quente() -> Option<(Hsla, Hsla)> {
        let p = paleta_atual();
        p.quente_da_marca
            .then(|| (cor(p.primaria), cor(p.sobre_primaria)))
    }

    /// Âmbar para texto: `text-amber-700 dark:text-amber-400`.
    pub fn quente_clara() -> Hsla {
        if let Some((marca, _)) = marca_quente() {
            return marca;
        }
        cor(if escuro() {
            paleta::AMBAR_400
        } else {
            paleta::AMBAR_700
        })
    }

    /// O texto que fica legível sobre o âmbar.
    pub fn sobre_quente() -> Hsla {
        match marca_quente() {
            Some((_, sobre)) => sobre,
            None => cor(paleta::SOBRE_CLARO),
        }
    }

    /// O selo âmbar: `border-amber-300 bg-amber-50 text-amber-800`, e no escuro
    /// `bg-amber-950/40 text-amber-300`. Devolve (fundo, borda, texto).
    pub fn selo_ambar() -> (Hsla, Hsla, Hsla) {
        if let Some((marca, _)) = marca_quente() {
            return (marca.opacity(0.12), marca, marca);
        }
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
        match marca_quente() {
            Some((marca, _)) => marca,
            None => cor(paleta::AMBAR_500),
        }
    }

    /// O véu atrás de um diálogo: `bg-black/50` do site.
    pub fn veu() -> Hsla {
        gpui_kit::rgba(0x00000080).into()
    }

    /// A estrela acesa — âmbar, ou a branca do Lightroom.
    pub fn nota() -> Hsla {
        cor(paleta_atual().estrela)
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

/// O botão aceso: o recorte escolhido da galeria, o alternador ligado — o
/// âmbar do site, ou o cinza-claro do Lightroom.
pub fn botao_aceso(cx: &App) -> ButtonCustomVariant {
    let p = paleta_atual();
    ButtonCustomVariant::new(cx)
        .color(cores::aceso())
        .foreground(cores::sobre_aceso())
        .hover(gpui_kit::rgb(p.aceso_pairando).into())
        .active(gpui_kit::rgb(p.aceso_ativo).into())
}

/// O tema do site num modo — o que o app sempre instalou.
#[cfg(test)]
fn tema_do_site(modo: ThemeMode) -> ThemeConfig {
    match modo {
        ThemeMode::Light => tema_da_paleta("RecordarFotos Claro", modo, paletas()[0], None),
        ThemeMode::Dark => tema_da_paleta("RecordarFotos Escuro", modo, paletas()[1], None),
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
    // 🎨 Os cantos e a letra do estilo do template: `radius` é o dos botões e
    // campos do gpui-kit, `radius.lg` o dos menus, cartões e diálogos dele.
    let m = medidas();
    // 🎞️ Com cantos retos, os do gpui-kit também: 2 nos controles, 3 nos
    // menus, cartões e diálogos.
    let (raio, raio_grande) = if p.cantos_retos {
        (2, 3)
    } else {
        (
            m.canto(m.campo.canto).round() as usize,
            m.canto(m.cartao.canto).round() as usize,
        )
    };
    config.insert("radius".into(), Value::from(raio));
    config.insert("radius.lg".into(), Value::from(raio_grande));
    // 🔠 A letra do template, com o ajuste do `Cmd +`/`Cmd −` (`letra`): trocar
    // de tema não desfaz o tamanho que o operador escolheu.
    config.insert("font.size".into(), Value::from(letra::tamanho()));
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
        // 🚨 `secondary` é o do shadcn, e não o `accent`: o botão fantasma do
        // gpui-kit escreve com `secondary.foreground`, e com
        // `destaque_do_menu = "bold"` o `accent-foreground` é o texto claro de
        // cima da primária — o "Depois" da faixa de atualização sumia.
        ("secondary.background", p.secundaria),
        ("secondary.hover.background", p.secundaria),
        ("secondary.active.background", p.secundaria),
        ("secondary.foreground", p.sobre_secundaria),
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
        // 🎞️ O corpo do painel sanfonado: nos painéis corridos (Lightroom) é
        // o cinza médio da coluna, e não o fundo da janela.
        (
            "accordion.background",
            if p.paineis_corridos {
                p.lateral
            } else {
                p.fundo
            },
        ),
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
        // O âmbar dos controles deslizantes do site (revelação e zoom) —
        // no Lightroom, o cinza. Quem desenha é o `slider_da_casa`.
        ("slider.background", p.preenchimento),
        ("slider.thumb.background", p.punho),
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
        // 🔥 O aviso do kit é a família quente: âmbar, ou a marca do tema.
        (
            "warning.background",
            if p.quente_da_marca {
                p.primaria
            } else {
                AMBAR_400
            },
        ),
        (
            "warning.hover.background",
            if p.quente_da_marca {
                p.aceso_pairando
            } else {
                AMBAR_300
            },
        ),
        (
            "warning.active.background",
            if p.quente_da_marca {
                p.primaria_pairando
            } else {
                AMBAR_500
            },
        ),
        (
            "warning.foreground",
            if p.quente_da_marca {
                p.sobre_primaria
            } else {
                SOBRE_CLARO
            },
        ),
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
            paletas()[1]
        } else {
            paletas()[0]
        }
    }

    #[test]
    fn tema_e_legivel() {
        for modo in MODOS {
            let config = tema_do_site(modo);
            assert_eq!(config.mode, modo);
            // Os cantos e a letra saem do estilo do template: no visual da
            // casa (o `nova` do site), 10 nos controles, 14 nos cartões e
            // diálogos, e o `text-sm`.
            let m = medidas();
            assert_eq!(config.radius, Some(m.canto(m.campo.canto).round() as usize));
            assert_eq!(
                config.radius_lg,
                Some(m.canto(m.cartao.canto).round() as usize)
            );
            assert_eq!(config.font_size, Some(m.letra));
        }
    }

    /// 🔑 **O template da casa pinta o que o app pintava antes de haver
    /// template.** As duas tabelas são as constantes escritas à mão até
    /// 2026-09-26 — os tokens do `globals.css` do site convertidos de oklch.
    /// Um canal pode diferir em 1 (arredondamento da conversão).
    #[test]
    fn a_casa_pinta_como_antes_do_template() {
        let antes_claro: [(&str, u32); 22] = [
            ("fundo", 0xffffff),
            ("texto", 0x0a0a0a),
            ("cartao", 0xffffff),
            ("primaria", 0x445566),
            ("primaria_pairando", 0x6a7785),
            ("sobre_primaria", 0xfafafa),
            ("apagado", 0xf5f5f5),
            ("texto_apagado", 0x737373),
            ("acento", 0xf5f5f5),
            ("sobre_acento", 0x171717),
            ("destrutiva", 0xe7000b),
            ("borda", 0xe5e5e5),
            ("campo", 0xe5e5e5),
            ("anel", 0xa1a1a1),
            ("lateral", 0xfafafa),
            ("texto_lateral", 0x0a0a0a),
            ("marca", 0x171717),
            ("sobre_marca", 0xfafafa),
            ("acento_lateral", 0xf0f0f0),
            ("borda_lateral", 0xe5e5e5),
            ("poco", 0xf5f5f5),
            ("rolagem", 0xd4d4d4),
        ];
        let antes_escuro: [(&str, u32); 22] = [
            ("fundo", 0x0a0a0a),
            ("texto", 0xfafafa),
            ("cartao", 0x171717),
            // Escrito à mão como #748699; a conta `bg-primary/80` sobre o
            // fundo dá #74889c, que é o que o template calcula.
            ("primaria", 0x8fa8c0),
            ("primaria_pairando", 0x74889c),
            ("sobre_primaria", 0x171717),
            ("apagado", 0x262626),
            ("texto_apagado", 0xa1a1a1),
            ("acento", 0x262626),
            ("sobre_acento", 0xfafafa),
            ("destrutiva", 0xff6467),
            ("borda", 0x232323),
            ("campo", 0x2f2f2f),
            ("anel", 0x737373),
            ("lateral", 0x171717),
            ("texto_lateral", 0xfafafa),
            ("marca", 0x1447e6),
            ("sobre_marca", 0xfafafa),
            ("acento_lateral", 0x262626),
            ("borda_lateral", 0x2e2e2e),
            ("poco", 0x171717),
            ("rolagem", 0x404040),
        ];
        for (modo, antes) in [
            (ThemeMode::Light, antes_claro),
            (ThemeMode::Dark, antes_escuro),
        ] {
            let p = paleta_de(modo);
            let agora = [
                p.fundo,
                p.texto,
                p.cartao,
                p.primaria,
                p.primaria_pairando,
                p.sobre_primaria,
                p.apagado,
                p.texto_apagado,
                p.acento,
                p.sobre_acento,
                p.destrutiva,
                p.borda,
                p.campo,
                p.anel,
                p.lateral,
                p.texto_lateral,
                p.marca,
                p.sobre_marca,
                p.acento_lateral,
                p.borda_lateral,
                p.poco,
                p.rolagem,
            ];
            let mut diferentes = Vec::new();
            for ((nome, era), e) in antes.iter().zip(agora) {
                let perto = [16, 8, 0]
                    .iter()
                    .all(|d| ((era >> d) & 0xff).abs_diff((e >> d) & 0xff) <= 1);
                if !perto {
                    diferentes.push(format!("{nome}: era #{era:06x}, agora #{e:06x}"));
                }
            }
            assert!(diferentes.is_empty(), "{modo:?}: {diferentes:#?}");
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
        for (chave, _) in cores(paletas()[1]) {
            assert!(
                vistas.insert(chave),
                "`{chave}` aparece duas vezes na lista"
            );
        }
    }

    /// Os valores são os do `globals.css` do site, convertidos de oklch.
    #[test]
    fn as_cores_sao_as_do_site() {
        assert_eq!(paletas()[0].primaria, 0x445566, "a marca do legado");
        assert_eq!(paletas()[1].fundo, 0x0a0a0a, "oklch(0.145 0 0)");
        assert_eq!(paletas()[1].cartao, 0x171717, "oklch(0.205 0 0)");
        assert_eq!(paletas()[1].primaria, 0x8fa8c0, "oklch(0.72 0.045 248.63)");
        assert_eq!(paletas()[1].marca, 0x1447e6, "oklch(0.488 0.243 264.376)");
    }

    /// 🟩🌆🎞️ Matrix, Cyberpunk e Lightroom: toda cor é `#rrggbb`, toda chave
    /// existe no esquema, o texto é legível (WCAG, 4,5:1) e o `bg-primary/80`
    /// é a conta.
    #[test]
    fn matrix_e_cyberpunk_sao_legiveis() {
        for (nome, p) in [
            ("Matrix", &paleta::MATRIX),
            ("Cyberpunk", &paleta::CYBERPUNK),
            ("Lightroom", &paleta::LIGHTROOM),
            ("Âmbar Rouge", &paleta::AMBAR_ROUGE),
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
            ("lightroom", Escolha::Lightroom),
        ] {
            assert_eq!(Escolha::do_nome(nome), Some(escolha));
            assert_eq!(escolha.modo(WindowAppearance::Light), ThemeMode::Dark);
            let pasta = tempfile::tempdir().unwrap();
            let arquivo = pasta.path().join("tema.json");
            guardar_escolha(&arquivo, escolha);
            assert_eq!(escolha_guardada(&arquivo), escolha);
        }
        assert_eq!(
            paletas()[Escolha::Matrix.paleta_escura() as usize],
            &paleta::MATRIX
        );
        assert_eq!(
            paletas()[Escolha::Cyberpunk.paleta_escura() as usize],
            &paleta::CYBERPUNK
        );
        assert_eq!(
            paletas()[Escolha::Escuro.paleta_escura() as usize],
            paletas()[1]
        );
        assert_eq!(
            paletas()[Escolha::Lightroom.paleta_escura() as usize],
            &paleta::LIGHTROOM
        );
    }

    /// 🎚️ **O slider dos temas que já existiam não muda de cor**: o âmbar
    /// cheio, o trilho a 20% dele (o do gpui-kit), o punho branco, a barra
    /// na medida do kit e o cabeçalho da sanfona sem fundo. Só o Lightroom
    /// traz trilho, punho e sanfona próprios.
    #[test]
    fn o_slider_dos_temas_de_antes_e_o_de_sempre() {
        // Os dois do site (Claro e Escuro): o âmbar do shadcn.
        for (i, p) in paletas().iter().enumerate().take(2) {
            // O aceso, a estrela e a grade de sempre.
            assert_eq!(p.aceso, AMBAR_400, "paleta {i}");
            assert_eq!(p.sobre_aceso, SOBRE_CLARO, "paleta {i}");
            assert_eq!(
                (p.aceso_pairando, p.aceso_ativo),
                (AMBAR_300, AMBAR_500),
                "paleta {i}"
            );
            assert_eq!(p.estrela, NOTA, "paleta {i}");
            assert_eq!(p.celulas, None, "paleta {i}");
            assert!(!p.cantos_retos && !p.paineis_corridos, "paleta {i}");
            assert_eq!(p.trilho, None, "paleta {i}");
            assert_eq!(p.preenchimento, AMBAR_400, "paleta {i}");
            assert_eq!(p.punho, 0xffffff, "paleta {i}");
            assert!(!p.slider_fino, "paleta {i}");
            assert_eq!(p.sanfona, None, "paleta {i}");
        }
        let lr = &paleta::LIGHTROOM;
        assert!(lr.slider_fino);
        assert!(lr.trilho.is_some() && lr.sanfona.is_some());
        // O punho claro precisa aparecer sobre o trilho, e o trilho sobre o
        // corpo da sanfona (a primeira versão, #1a1a1a sobre #1e1e1e, sumia).
        assert!(contraste(lr.trilho.unwrap(), lr.punho) >= 4.5);
        assert!(contraste(lr.lateral, lr.trilho.unwrap()) >= 2.0);
    }

    /// 🌹 **O Âmbar Rouge é o tema fixo claro**: abre no claro mesmo com o
    /// sistema no escuro, fica lembrado, pinta a própria paleta no claro, e o
    /// que acende (âmbar dourado) e a estrela continuam legíveis sobre o
    /// rosa-pó.
    #[test]
    fn o_ambar_rouge_e_claro_e_fica_lembrado() {
        let escolha = Escolha::AmbarRouge;
        assert_eq!(Escolha::do_nome("ambar_rouge"), Some(escolha));
        assert_eq!(escolha.modo(WindowAppearance::Dark), ThemeMode::Light);
        assert_eq!(
            paletas()[escolha.paleta_clara() as usize],
            &paleta::AMBAR_ROUGE
        );
        // Os outros continuam com a paleta clara do template.
        assert_eq!(Escolha::Claro.paleta_clara(), 0);
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = pasta.path().join("tema.json");
        guardar_escolha(&arquivo, escolha);
        assert_eq!(escolha_guardada(&arquivo), escolha);
        assert_eq!(
            std::fs::read_to_string(&arquivo).unwrap(),
            "\"ambar_rouge\""
        );
        let p = &paleta::AMBAR_ROUGE;
        assert!(contraste(p.aceso, p.sobre_aceso) >= 4.5, "o aceso");
        assert!(
            contraste(p.fundo, p.estrela) >= 3.0,
            "a estrela (objeto gráfico)"
        );
        assert!(contraste(p.cartao, p.texto) >= 4.5, "o texto no cartão");
        assert!(!p.quente_da_marca && !p.cantos_retos && p.celulas.is_none());
    }

    /// 🟩🌆 **Matrix e Cyberpunk não têm âmbar** (dono, 28/09: *"No tema
    /// Matrix existe cor âmbar?"*): o que acende, a estrela, o slider e a
    /// família quente são a marca de cada um — e o texto por cima continua
    /// legível.
    #[test]
    fn matrix_e_cyberpunk_acendem_na_propria_marca() {
        let ambares = [AMBAR_300, AMBAR_400, AMBAR_500, NOTA];
        for (nome, p) in [
            ("Matrix", &paleta::MATRIX),
            ("Cyberpunk", &paleta::CYBERPUNK),
        ] {
            assert!(p.quente_da_marca, "{nome}");
            for (campo, c) in [
                ("aceso", p.aceso),
                ("aceso_pairando", p.aceso_pairando),
                ("aceso_ativo", p.aceso_ativo),
                ("estrela", p.estrela),
                ("preenchimento", p.preenchimento),
            ] {
                assert!(!ambares.contains(&c), "{nome}: `{campo}` ainda é âmbar");
            }
            assert_eq!(p.aceso, p.primaria, "{nome}: o aceso é a marca");
            assert!(contraste(p.aceso, p.sobre_aceso) >= 4.5, "{nome}");
            assert!(contraste(p.fundo, p.estrela) >= 4.5, "{nome}");
            // O aviso do gpui-kit também é a marca.
            let config = tema_da_paleta(nome, ThemeMode::Dark, p, None);
            let de_volta = serde_json::to_value(&config.colors).unwrap();
            assert_eq!(
                de_volta.get("warning.background").and_then(|v| v.as_str()),
                Some(hex(p.primaria).as_str()),
                "{nome}"
            );
        }
    }

    /// 🎞️ O pente fino do Lightroom continua legível: o texto sobre o aceso,
    /// a estrela sobre o fundo, e o nome da foto na célula em foco.
    #[test]
    fn o_lightroom_acende_em_cinza_e_continua_legivel() {
        let lr = &paleta::LIGHTROOM;
        assert!(contraste(lr.aceso, lr.sobre_aceso) >= 4.5);
        assert!(contraste(lr.fundo, lr.estrela) >= 4.5);
        let c = lr.celulas.expect("o Lightroom desenha células");
        let fundos = [c.grade, c.celula, c.marcada, c.foco];
        for (i, a) in fundos.iter().enumerate() {
            for b in &fundos[i + 1..] {
                assert!(a != b, "as quatro se distinguem");
            }
        }
        assert!(contraste(c.celula, lr.texto) >= 4.5, "o nome na célula");
        // Na marcada e na em foco, claras, o nome é escuro.
        for fundo in [c.marcada, c.foco] {
            assert!(
                contraste(fundo, lr.texto) >= 4.5 || contraste(fundo, SOBRE_ESCURO_NEUTRO) >= 4.5,
                "o nome sobre #{fundo:06x}"
            );
        }
        // O texto do painel sobre o corpo do painel, e o apagado sobre a moldura.
        assert!(contraste(lr.lateral, lr.texto_lateral) >= 4.5);
        assert!(contraste(lr.fundo, lr.texto_apagado) >= 4.5);
        assert!(lr.cantos_retos && lr.paineis_corridos);
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
                paletas()[0].sobre_primaria,
            ),
            (
                "primária escura",
                paletas()[1].primaria,
                paletas()[1].sobre_primaria,
            ),
            ("âmbar", AMBAR_400, SOBRE_CLARO),
            ("texto no claro", paletas()[0].fundo, paletas()[0].texto),
            (
                "apagado no claro",
                paletas()[0].fundo,
                paletas()[0].texto_apagado,
            ),
            ("texto no escuro", paletas()[1].fundo, paletas()[1].texto),
            (
                "apagado no escuro",
                paletas()[1].fundo,
                paletas()[1].texto_apagado,
            ),
            ("marca escura", paletas()[1].marca, paletas()[1].sobre_marca),
        ];
        for (nome, fundo, frente) in pares {
            let razao = contraste(fundo, frente);
            assert!(razao >= 4.5, "`{nome}`: {razao:.2}:1 — abaixo de 4,5:1");
        }
    }

    /// A marca do legado — a primária do claro no visual da casa.
    const CLARO_PRIMARIA: u32 = 0x445566;

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
