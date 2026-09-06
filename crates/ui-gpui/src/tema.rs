//! O tema **Vintage Dark**, dito na linguagem do `gpui-component`.
//!
//! `gpui_component::init` não é neutro: ele instala o tema do shadcn — fundo
//! `#0a0a0a`, cantos de 6px — e **sincroniza claro/escuro com o sistema**. Os
//! dois são errados aqui, por razões diferentes:
//!
//! - o app é escuro por decisão, e o cinza dele é escolhido (§ "A escada");
//! - um programa de revelação **não pode ficar branco** porque o macOS amanheceu
//!   no modo claro. O entorno é parte da medição de cor — é a mesma razão pela
//!   qual a moldura da foto selecionada é borda, e não fundo colorido.
//!
//! Por isso, depois do `init`, este módulo troca o tema escuro pelo nosso e
//! **trava o modo em escuro**.
//!
//! ## A escada de cinzas, e por que ela tem sete degraus
//!
//! Até 6/set/2026 o tema tinha cinco fundos, e a tela usava **três**: o fundo do
//! app, a borda e o texto apagado. Tudo o mais era o azul de acento. O resultado
//! é o que o dono descreveu — "apenas 3 cores (P&B) + azul" —, e a causa não é o
//! framework: é que profundidade em interface escura se faz com **degraus de
//! luminância**, e não havia degrau suficiente para hierarquia nenhuma.
//!
//! A escada agora vai do **poço** (o que fica atrás de uma foto) até a borda
//! forte, e cada degrau tem um papel:
//!
//! | Degrau | Papel | Onde |
//! |---|---|---|
//! | `POCO` | atrás de imagem | moldura da miniatura, visor, filmstrip |
//! | `FUNDO` | o fundo do app | a raiz de cada tela |
//! | `SUPERFICIE` | painel encostado no fundo | barra, laterais, cabeçalho de sessão |
//! | `ELEVADO` | o que flutua | diálogo, popover, cartão, campo |
//! | `PAIRANDO` / `ATIVO` | resposta ao ponteiro | botão, linha de lista |
//! | `LINHA_FORTE` | separação que precisa ser vista | punho de rolagem, divisor |
//!
//! 🔑 **O poço é mais escuro que o app, e isso é a favor da foto**: quanto mais
//! escuro o que encosta na imagem, menos a vizinhança empurra a percepção de
//! exposição. É por isso que a escada desce em vez de só subir.
//!
//! ## Duas famílias de acento, e o que cada uma quer dizer
//!
//! - **Azul** — ação e seleção. É o mesmo azul do `recordarfotos.com.br`
//!   (paridade com o site: o mesmo gesto, a mesma cor).
//! - **Âmbar** — sessão, balcão e pós-venda: tudo que atravessa para o site e
//!   vira dinheiro. É a família que separa "estou mexendo na foto" de "estou
//!   mexendo no ensaio do cliente", e o motivo de ela existir é que essas duas
//!   coisas custavam a mesma cor.
//!
//! ⚠️ **Nenhuma das duas encosta na foto.** A moldura da selecionada continua
//! sendo borda azul fina, e o entorno da imagem continua cinza puro: cor
//! saturada ao redor de uma foto muda como a foto é percebida.
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
//! 🔑 **As cores são `u32`, e o hexadecimal é gerado.** Antes eram literais
//! `&str`, e a mesma cor precisava existir duas vezes — uma para o JSON do tema,
//! outra para o código da tela, que quer `Hsla`. Duas escritas da mesma decisão
//! divergem; esta escreve uma vez e formata (`hex`) ou converte (`cor`).
//!
//! ## 🚨 Os dois modos de errar aqui são silenciosos
//!
//! 1. **Cor ilegível não falha: ela some.** O `apply_config` tenta ler cada
//!    hexadecimal e, quando não consegue, usa o padrão dele **sem dizer nada**.
//!    Isto hoje é impossível por construção — `hex` sempre produz `#rrggbb` —, e
//!    o teste `todo_hexadecimal_tem_seis_digitos` é quem mantém assim.
//! 2. **Chave errada não falha: ela é ignorada.** O `serde` do `ThemeConfig` não
//!    recusa campo desconhecido, então `acent.background` seria lido, descartado
//!    e o tema abriria com aquele token no valor de fábrica.
//!
//! Nenhum dos dois dá erro, log ou tela quebrada — dão *uma cor diferente*, que
//! é exatamente o tipo de coisa que se atribui ao framework três meses depois.
//! Os testes no fim do arquivo existem para pegar os dois.

use std::rc::Rc;

use gpui::App;
use gpui_component::button::ButtonCustomVariant;
use gpui_component::{Theme, ThemeConfig, ThemeMode};
use serde_json::Value;

/// A paleta, em `0xrrggbb`.
///
/// 🔑 **Um número por decisão de cor.** O JSON quer texto e a tela quer `Hsla`;
/// os dois saem daqui, por `hex` e por `cor`.
mod paleta {
    // ── A escada de cinzas ────────────────────────────────────────────────
    /// Atrás de imagem. Mais escuro que o app de propósito.
    pub const POCO: u32 = 0x121212;
    /// O fundo do app.
    pub const FUNDO: u32 = 0x1a1a1a;
    /// Painel encostado no fundo: barra, laterais, cabeçalho.
    pub const SUPERFICIE: u32 = 0x212121;
    /// O que flutua: diálogo, popover, cartão, campo.
    pub const ELEVADO: u32 = 0x282828;
    /// Sob o ponteiro.
    pub const PAIRANDO: u32 = 0x303030;
    /// Apertado, ou aceso.
    pub const ATIVO: u32 = 0x3a3a3a;
    /// Separação que precisa ser vista: punho de rolagem, divisor forte.
    pub const LINHA_FORTE: u32 = 0x454545;

    // ── Texto ─────────────────────────────────────────────────────────────
    pub const TEXTO: u32 = 0xe8e8e8;
    pub const TEXTO_SECUNDARIO: u32 = 0xb8b8b8;
    pub const TEXTO_APAGADO: u32 = 0x8c8c8c;
    pub const TEXTO_DESLIGADO: u32 = 0x5e5e5e;

    // ── Linhas ────────────────────────────────────────────────────────────
    pub const BORDA: u32 = 0x323232;
    pub const BORDA_CLARA: u32 = 0x3f3f3f;

    // ── Azul: ação e seleção ──────────────────────────────────────────────
    pub const AZUL: u32 = 0x4a9eff;
    pub const AZUL_CLARO: u32 = 0x6fb2ff;
    pub const AZUL_ESCURO: u32 = 0x3a8ee6;

    // ── Âmbar: sessão, balcão, pós-venda ──────────────────────────────────
    pub const AMBAR: u32 = 0xd99a4e;
    pub const AMBAR_CLARO: u32 = 0xe8ab5f;
    pub const AMBAR_ESCURO: u32 = 0xbf8440;

    // ── Semânticas ────────────────────────────────────────────────────────
    /// A estrela acesa. Ouro, e não o azul de ação: nota é julgamento sobre a
    /// foto, não um controle da interface.
    pub const NOTA: u32 = 0xf0b429;
    /// O sinalizador de escolhida (`P`).
    pub const ESCOLHIDA: u32 = 0x4ade80;
    /// O sinalizador de rejeitada (`X`), desenhado como marca sobre a foto.
    pub const REJEITADA: u32 = 0xef4444;
    /// 🔑 **O vermelho de botão é mais escuro que o de marca, e é de propósito.**
    /// Os dois têm leitores diferentes: a marca é tinta sobre o poço (5,0:1 lá),
    /// e o botão carrega texto branco em cima — `#ef4444` daria 3,8:1, que é
    /// legível de dia e some no resto. Um vermelho só serviria mal aos dois.
    pub const ERRO_FUNDO: u32 = 0xdc2626;
    pub const ERRO_CLARO: u32 = 0xef4444;
    pub const ERRO_ESCURO: u32 = 0xb91c1c;

    // ── As cinco etiquetas do `ColorLabel` ────────────────────────────────
    /// 🔑 **As cinco são claras**, e não os vermelhos e roxos cheios que a
    /// paleta de marca pediria. Elas têm dois trabalhos incompatíveis: ponto de
    /// 8px sobre o poço (precisa de luminância para aparecer) e fundo de botão
    /// de filtro com o rótulo escrito em cima (precisa de luminância para o
    /// texto escuro caber). Um roxo `#8e4ec6` bonito falha nos dois.
    pub const ETIQUETA_VERMELHA: u32 = 0xef5d62;
    pub const ETIQUETA_AMARELA: u32 = 0xf2c336;
    pub const ETIQUETA_VERDE: u32 = 0x46b95c;
    /// ⚠️ Puxada para o índigo, e não para o azul de ação: uma etiqueta azul
    /// parecida com a moldura da seleção faria a foto etiquetada parecer
    /// selecionada.
    pub const ETIQUETA_AZUL: u32 = 0x7b86ff;
    pub const ETIQUETA_ROXA: u32 = 0xb073ea;

    // ── Texto sobre fundo colorido ────────────────────────────────────────
    /// 🔑 **Escuro sobre o azul, e não branco.** Branco sobre `#4a9eff` dá 2,8:1
    /// — abaixo do mínimo de qualquer régua. O escuro dá mais de 7:1, e
    /// `contraste_do_texto_sobre_cor` não deixa isso regredir.
    pub const SOBRE_AZUL: u32 = 0x0e1420;
    /// Sobre âmbar, amarelo e verde, que são claros de verdade.
    pub const SOBRE_CLARO: u32 = 0x1a1206;
}

use paleta::*;

/// O canto padrão. Eram 3px por paridade com o app de egui, que saiu em
/// ago/2026; 4px é o mesmo desenho um pouco menos duro, e ainda longe de virar
/// pílula num botão de 20px de altura (o padrão do `gpui-component` é 6).
const RAIO: usize = 4;
/// Diálogo e aviso, peças grandes o bastante para o canto pequeno sumir nelas.
const RAIO_GRANDE: usize = 10;

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

/// O que a tela pede pelo nome, e o tema do `gpui-component` não tem nome para.
///
/// 🔑 **Só entra aqui o que carrega significado da fotografia** — nota,
/// sinalizador, etiqueta — ou o que a escada nomeia e o esquema oficial não
/// (o poço). Cor de botão, de borda e de painel continua vindo do
/// `cx.theme()`: duas fontes para a mesma decisão é como um tema deixa de valer.
pub mod cores {
    use super::paleta;
    use domain::value_objects::ColorLabel;
    use gpui::Hsla;

    fn cor(rgb: u32) -> Hsla {
        gpui::rgb(rgb).into()
    }

    /// O fundo de tudo que encosta numa imagem.
    pub fn poco() -> Hsla {
        cor(paleta::POCO)
    }

    /// Âmbar — sessão, balcão, pós-venda.
    pub fn quente() -> Hsla {
        cor(paleta::AMBAR)
    }

    /// Âmbar mais claro, para texto sobre fundo escuro.
    pub fn quente_clara() -> Hsla {
        cor(paleta::AMBAR_CLARO)
    }

    /// O texto que fica legível sobre o âmbar.
    pub fn sobre_quente() -> Hsla {
        cor(paleta::SOBRE_CLARO)
    }

    /// O véu preto atrás de um diálogo.
    ///
    /// 🔑 **Um valor só, e não seis.** Estava escrito à mão em cada modal —
    /// `0x99` nas Configurações, `0xaa` no aviso, `0xcc` na importação —, e a
    /// diferença não era decisão: era ordem de escrita. Modal mais escuro que o
    /// outro faz o app parecer que muda de tema ao abrir um deles.
    ///
    /// ⚠️ **Não sai do `cx.theme()`**: o `overlay` do `gpui-component` é o véu
    /// sutil dele (5% de preto no tema claro), e não o fundo de diálogo. Pedir
    /// aquele token aqui daria um véu que não esconde nada.
    pub fn veu() -> Hsla {
        gpui::rgba(0x000000a6).into()
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

    /// Claro ou escuro sobre uma cor — o que tiver mais contraste.
    ///
    /// 🔑 **Medido, e não escolhido a olho.** É a conta da WCAG, a mesma que
    /// `contraste_do_texto_sobre_cor` cobra da paleta: sobre o amarelo da
    /// etiqueta, branco dá 1,7:1 e some; sobre um roxo cheio, escuro dá 3,2:1 e
    /// some do mesmo jeito. Nenhum dos dois serve sempre — e comparar as duas
    /// razões custa o mesmo que chutar um limiar.
    pub fn texto_sobre(fundo: Hsla) -> Hsla {
        let claro: Hsla = gpui::rgb(0xffffff).into();
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

    /// Luminância relativa da WCAG 2.1, com a linearização do sRGB.
    fn luminancia(cor: Hsla) -> f32 {
        let rgba = gpui::Rgba::from(cor);
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
    ///
    /// ⚠️ **Sem maiúscula fixa**: o banco guarda `"Yellow"`, o domínio nomeia
    /// `"yellow"` e o legado gravava as duas formas. `ColorLabel::from_name` já
    /// resolve isso, e esta função existe para não haver uma segunda tabela de
    /// nomes de cor no crate da interface.
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

/// O botão da família âmbar — balcão, pós-venda, sessão.
///
/// 🔑 **É variante de botão, e não uma cor solta**: `ButtonCustomVariant` já
/// sabe pintar o repouso, o ponteiro e o apertado, e é o que mantém o âmbar se
/// comportando como os outros botões em vez de virar um `div` colorido que
/// parece um botão.
pub fn botao_quente(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(cores::quente())
        .foreground(cores::sobre_quente())
        .border(cores::quente())
        .hover(gpui::rgb(AMBAR_CLARO).into())
        .active(gpui::rgb(AMBAR_ESCURO).into())
}

fn vintage_dark() -> ThemeConfig {
    let mut config = serde_json::Map::new();
    config.insert("is_default".into(), Value::Bool(true));
    config.insert("name".into(), Value::String("Vintage Dark".into()));
    config.insert("mode".into(), Value::String("dark".into()));
    config.insert("radius".into(), Value::from(RAIO));
    config.insert("radius.lg".into(), Value::from(RAIO_GRANDE));
    // Sombra desligada, como no egui ("Flat shadows for minimalist look").
    // Aqui isso é mais que estilo: sombra em cima de miniatura é gradiente
    // escuro encostando na foto, e quem julga exposição olhando a grade
    // passa a julgar a sombra junto. A profundidade vem da escada de cinzas.
    config.insert("shadow".into(), Value::Bool(false));
    config.insert("colors".into(), cores_do_esquema());

    serde_json::from_value(Value::Object(config))
        .expect("o Vintage Dark tem de ser legível — `tema_e_legivel` confere isso")
}

/// Chave do esquema oficial à esquerda, cor da paleta à direita.
///
/// Uma lista de pares, e não um `json!`: com este tamanho o `json!` estoura o
/// limite de recursão de macro do rustc, e subir o limite do crate inteiro para
/// escrever um objeto plano é pagar caro por açúcar.
const CORES: &[(&str, u32)] = &[
    ("background", FUNDO),
    ("foreground", TEXTO),
    ("border", BORDA),
    ("window.border", BORDA),
    // O `muted` é o fundo de tudo que encosta em imagem — moldura da miniatura,
    // do filmstrip, do visor e da folha de impressão. É o poço, e não um cinza
    // intermediário: ver a escada, no topo do arquivo.
    ("muted.background", POCO),
    ("muted.foreground", TEXTO_APAGADO),
    // O secundário é o botão comum — o de filtro da barra e quase toda a barra
    // de navegação.
    ("secondary.background", ELEVADO),
    ("secondary.hover.background", PAIRANDO),
    ("secondary.active.background", ATIVO),
    ("secondary.foreground", TEXTO_SECUNDARIO),
    // O acento é o realce do item sob o ponteiro, em menu e em lista.
    ("accent.background", PAIRANDO),
    ("accent.foreground", TEXTO),
    ("primary.background", AZUL),
    ("primary.hover.background", AZUL_CLARO),
    ("primary.active.background", AZUL_ESCURO),
    ("primary.foreground", SOBRE_AZUL),
    ("ring", AZUL),
    ("selection.background", AZUL),
    ("caret", AZUL),
    ("input.border", BORDA_CLARA),
    ("popover.background", ELEVADO),
    ("popover.foreground", TEXTO),
    ("sidebar.background", SUPERFICIE),
    ("sidebar.foreground", TEXTO),
    ("sidebar.border", BORDA),
    ("sidebar.accent.background", PAIRANDO),
    ("sidebar.accent.foreground", TEXTO),
    ("sidebar.primary.background", AZUL),
    ("sidebar.primary.foreground", SOBRE_AZUL),
    ("list.background", SUPERFICIE),
    ("list.hover.background", PAIRANDO),
    ("list.active.background", ATIVO),
    ("list.active.border", AZUL),
    ("list.even.background", FUNDO),
    ("list.head.background", SUPERFICIE),
    // 🚨 As abas do dock — os painéis das duas telas grandes vivem nelas, e até
    // 6/set/2026 eram as únicas peças da tela ainda pintadas pelo shadcn:
    // `#0a0a0a` de fábrica dentro de um app `#1a1a1a`.
    // 🔑 A barra de abas fica no fundo do app e a aba acesa **sobe** para o
    // cinza do painel (`sidebar`), de modo que ela e o conteúdo dela são a mesma
    // superfície. O contrário — aba mais escura que o próprio painel — faz a
    // acesa parecer a desligada.
    ("tab_bar.background", FUNDO),
    ("tab_bar.segmented.background", ELEVADO),
    ("tab.background", FUNDO),
    ("tab.foreground", TEXTO_APAGADO),
    ("tab.active.background", SUPERFICIE),
    ("tab.active.foreground", TEXTO),
    // Arrastar painel do dock: a linha que diz "solta aqui".
    ("drag.border", AZUL),
    ("drop_target.background", AZUL),
    ("accordion.background", SUPERFICIE),
    ("accordion.hover.background", PAIRANDO),
    ("group_box.background", SUPERFICIE),
    ("group_box.foreground", TEXTO),
    ("group_box.title.foreground", TEXTO_SECUNDARIO),
    ("table.background", FUNDO),
    ("table.head.background", SUPERFICIE),
    ("table.head.foreground", TEXTO_APAGADO),
    ("table.hover.background", PAIRANDO),
    ("table.active.background", ATIVO),
    ("table.active.border", AZUL),
    ("table.even.background", SUPERFICIE),
    ("table.row.border", BORDA),
    // Os dois que a Revelação mais gasta: a barra preenchida do slider e o
    // punho. O punho é claro, e não azul, para continuar visível quando ~50
    // ajustes estiverem empilhados na mesma coluna.
    ("slider.background", AZUL),
    ("slider.thumb.background", TEXTO),
    ("progress.bar.background", AZUL),
    ("switch.background", ATIVO),
    ("switch.thumb.background", TEXTO),
    ("skeleton.background", ELEVADO),
    ("scrollbar.background", FUNDO),
    ("scrollbar.thumb.background", LINHA_FORTE),
    ("scrollbar.thumb.hover.background", TEXTO_DESLIGADO),
    ("title_bar.background", SUPERFICIE),
    ("title_bar.border", BORDA),
    ("tiles.background", POCO),
    ("danger.background", ERRO_FUNDO),
    ("danger.hover.background", ERRO_CLARO),
    ("danger.active.background", ERRO_ESCURO),
    ("danger.foreground", 0xffffff),
    // 🔑 O aviso é o ouro da nota, e não um amarelo à parte: as duas coisas
    // querem dizer "olhe para isto" e uma segunda tonalidade só dividiria a
    // atenção.
    ("warning.background", NOTA),
    ("warning.hover.background", 0xf6c24a),
    ("warning.active.background", 0xd79f1f),
    ("warning.foreground", SOBRE_CLARO),
    ("success.background", ESCOLHIDA),
    ("success.hover.background", 0x63e79a),
    ("success.active.background", 0x3bc46b),
    ("success.foreground", SOBRE_CLARO),
    ("info.background", AZUL),
    ("info.hover.background", AZUL_CLARO),
    ("info.active.background", AZUL_ESCURO),
    ("info.foreground", SOBRE_AZUL),
    ("link", AZUL),
    ("link.hover", AZUL_CLARO),
    ("link.active", AZUL_ESCURO),
    // As cores base do `Tag::color(…)` e do que o crate desenha por nome.
    ("base.red", ETIQUETA_VERMELHA),
    ("base.yellow", ETIQUETA_AMARELA),
    ("base.green", ETIQUETA_VERDE),
    ("base.blue", ETIQUETA_AZUL),
    ("base.magenta", ETIQUETA_ROXA),
];

/// `0x1a2b3c` → `"#1a2b3c"`. Sempre seis dígitos — é o que o `gpui-component`
/// consegue ler, e o que ele não lê ele troca pelo tema dele sem avisar.
fn hex(cor: u32) -> String {
    format!("#{cor:06x}")
}

fn cores_do_esquema() -> Value {
    Value::Object(
        CORES
            .iter()
            .map(|(chave, cor)| (chave.to_string(), Value::String(hex(*cor))))
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
    #[test]
    fn todo_hexadecimal_tem_seis_digitos() {
        assert!(
            !CORES.is_empty(),
            "nenhuma cor declarada — o tema não estaria fazendo nada"
        );

        for (chave, cor) in CORES {
            let escrito = hex(*cor);
            assert!(
                escrito.len() == 7
                    && escrito.starts_with('#')
                    && escrito[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "`{chave}` = {escrito:?} não é #rrggbb — o gpui-component cairia no tema dele, sem avisar"
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

        for (chave, cor) in CORES {
            assert_eq!(
                de_volta.get(chave).and_then(|v| v.as_str()),
                Some(hex(*cor).as_str()),
                "`{chave}` não sobreviveu à leitura — nome fora do esquema do gpui-component"
            );
        }
    }

    /// Nenhuma chave escrita duas vezes com valores diferentes.
    ///
    /// A lista é plana e cresce por baixo; um par repetido não falha em lugar
    /// nenhum — o último vence, e a decisão que se lê no arquivo não é a que
    /// está na tela.
    #[test]
    fn nenhuma_chave_repetida() {
        let mut vistas = std::collections::HashSet::new();
        for (chave, _) in CORES {
            assert!(
                vistas.insert(*chave),
                "`{chave}` aparece duas vezes na lista"
            );
        }
    }

    /// A escada sobe: cada degrau é mais claro que o anterior.
    ///
    /// 🔑 É o que faz a profundidade existir. Dois degraus na mesma luminância
    /// compilam, abrem e desenham uma tela chapada — o defeito que este tema
    /// veio corrigir.
    #[test]
    fn a_escada_sobe_degrau_a_degrau() {
        let escada = [
            ("POCO", POCO),
            ("FUNDO", FUNDO),
            ("SUPERFICIE", SUPERFICIE),
            ("ELEVADO", ELEVADO),
            ("PAIRANDO", PAIRANDO),
            ("ATIVO", ATIVO),
            ("LINHA_FORTE", LINHA_FORTE),
        ];

        for par in escada.windows(2) {
            let (nome_baixo, baixo) = par[0];
            let (nome_alto, alto) = par[1];
            assert!(
                luminancia(alto) > luminancia(baixo),
                "`{nome_alto}` não é mais claro que `{nome_baixo}` — o degrau não existe na tela"
            );
        }
    }

    /// 🚨 Texto sobre fundo colorido continua legível.
    ///
    /// O erro clássico é branco sobre um azul claro de acento: dá 2,8:1, some
    /// no sol e ninguém chama de defeito porque "aparece". A régua é a da WCAG
    /// para texto pequeno, 4,5:1.
    #[test]
    fn contraste_do_texto_sobre_cor() {
        let pares = [
            ("azul", AZUL, SOBRE_AZUL),
            ("azul claro", AZUL_CLARO, SOBRE_AZUL),
            ("âmbar", AMBAR, SOBRE_CLARO),
            ("nota / aviso", NOTA, SOBRE_CLARO),
            ("escolhida / sucesso", ESCOLHIDA, SOBRE_CLARO),
            ("erro", ERRO_FUNDO, 0xffffff),
            // A marca de rejeitada não carrega texto: ela é tinta sobre o
            // poço, e é contra ele que precisa aparecer.
            ("rejeitada sobre o poço", POCO, REJEITADA),
            ("texto no app", FUNDO, TEXTO),
            ("texto apagado no app", FUNDO, TEXTO_APAGADO),
        ];

        for (nome, fundo, frente) in pares {
            let razao = contraste(fundo, frente);
            assert!(
                razao >= 4.5,
                "`{nome}`: {razao:.2}:1 entre {} e {} — abaixo de 4,5:1",
                hex(fundo),
                hex(frente)
            );
        }
    }

    /// As cinco etiquetas do domínio têm cor, em qualquer grafia.
    #[test]
    fn toda_etiqueta_do_dominio_tem_cor() {
        for etiqueta in domain::value_objects::ColorLabel::all() {
            let nome = etiqueta.name();
            assert!(
                cores::etiqueta(nome).is_some(),
                "`{nome}` não tem cor — a grade desenharia a etiqueta invisível"
            );
            // A grafia do banco é com maiúscula (`"Yellow"`), a do domínio é
            // sem. As duas precisam achar a mesma cor.
            let como_no_banco = format!("{}{}", nome[..1].to_uppercase(), &nome[1..]);
            assert_eq!(
                cores::etiqueta(&como_no_banco),
                cores::etiqueta(nome),
                "`{como_no_banco}` e `{nome}` deram cores diferentes"
            );
        }

        assert!(
            cores::etiqueta("laranja").is_none(),
            "cor inventada devolveu tinta — a grade mostraria etiqueta que o domínio não tem"
        );
    }

    /// 🚨 O texto que `texto_sobre` escolhe é legível sobre toda cor da paleta.
    ///
    /// A régua é a mesma de `contraste_do_texto_sobre_cor`: 4,5:1. Aqui ela vale
    /// para as cores em que o texto **não** foi escolhido à mão — as cinco
    /// etiquetas, que carregam rótulo quando viram botão de filtro aceso.
    #[test]
    fn o_texto_escolhido_pela_luminancia_e_legivel() {
        let coloridas = [
            ("vermelha", ETIQUETA_VERMELHA),
            ("amarela", ETIQUETA_AMARELA),
            ("verde", ETIQUETA_VERDE),
            ("azul", ETIQUETA_AZUL),
            ("roxa", ETIQUETA_ROXA),
            ("âmbar", AMBAR),
            ("azul de ação", AZUL),
            ("nota", NOTA),
        ];

        for (nome, fundo) in coloridas {
            let texto = cores::texto_sobre(gpui::rgb(fundo).into());
            let como_u32 = {
                let rgba = gpui::Rgba::from(texto);
                ((rgba.r * 255.).round() as u32) << 16
                    | ((rgba.g * 255.).round() as u32) << 8
                    | (rgba.b * 255.).round() as u32
            };
            let razao = contraste(fundo, como_u32);
            assert!(
                razao >= 4.5,
                "sobre a etiqueta {nome} ({}) o texto escolhido dá {razao:.2}:1",
                hex(fundo)
            );
        }
    }

    /// As cinco etiquetas são distinguíveis entre si.
    #[test]
    fn nenhuma_etiqueta_repete_a_outra() {
        let cores_usadas = [
            ETIQUETA_VERMELHA,
            ETIQUETA_AMARELA,
            ETIQUETA_VERDE,
            ETIQUETA_AZUL,
            ETIQUETA_ROXA,
        ];
        let distintas: std::collections::HashSet<_> = cores_usadas.iter().collect();
        assert_eq!(
            distintas.len(),
            cores_usadas.len(),
            "duas etiquetas com a mesma cor — a triagem por cor deixaria de separar"
        );
    }

    // ── As contas da WCAG, para os testes acima ───────────────────────────

    fn canais(cor: u32) -> [f32; 3] {
        [
            ((cor >> 16) & 0xff) as f32 / 255.,
            ((cor >> 8) & 0xff) as f32 / 255.,
            (cor & 0xff) as f32 / 255.,
        ]
    }

    /// Luminância relativa da WCAG 2.1 — com a linearização do sRGB, que é o
    /// que separa "mais claro no número" de "mais claro no olho".
    fn luminancia(cor: u32) -> f32 {
        let linear = |c: f32| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = canais(cor);
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn contraste(a: u32, b: u32) -> f32 {
        let (mais, menos) = {
            let (la, lb) = (luminancia(a), luminancia(b));
            if la > lb {
                (la, lb)
            } else {
                (lb, la)
            }
        };
        (mais + 0.05) / (menos + 0.05)
    }
}
