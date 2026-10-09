//! 🧰 A barra de ferramentas do Photoshop: grupos, flyout e as duas cores.
//!
//! 🔑 **Uma tabela só** ([`FERRAMENTAS`]): o grupo da barra, a letra, o
//! ícone, o nome e a dica de cada ferramenta saem daqui — a barra, o flyout,
//! as letras do teclado, o título das opções e a Ajuda não têm cópia própria.
//!
//! Como no Photoshop:
//!
//! - cada grupo mostra **uma** ferramenta (a última usada nele), com o
//!   triângulo no canto quando há variantes;
//! - clique simples usa a mostrada; aperto prolongado ou botão direito abrem
//!   o flyout (ícone, nome, letra e a escolhida marcada; ↑ ↓ Enter, Esc
//!   fecha), que fica dentro da janela;
//! - a letra volta à última do grupo de letra; ⇧ + letra passa para a
//!   seguinte (Mão e Girar vista dividem o grupo da barra com letras
//!   próprias, H e R, como lá);
//! - embaixo, frente e fundo sobrepostos: clicar em cada um edita aquela cor,
//!   ⇄ troca (X) e o quadradinho volta a preto e branco (D).

use std::time::Duration;

use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, prelude::*, px, AnyElement, Bounds, Context, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString,
};

use super::aparencia::{self, medida};
use super::{Auxiliar, EditorDeFoto, Item, TipoDeSelecao};
use crate::recursos::Icone;
use editor_core::pincel::Faixa;
use editor_core::vetor::caneta::FerramentaVetorial as FV;
use editor_core::Ferramenta;

/// Uma ferramenta da barra.
#[derive(Clone, Copy, Debug)]
pub struct DefDeFerramenta {
    pub item: Item,
    /// O grupo da barra ([`GRUPOS_DA_BARRA`]).
    pub grupo: usize,
    /// A letra do teclado (`None`: desfoque e nitidez, como no Photoshop).
    pub letra: Option<char>,
    pub icone: Icone,
    /// O id do botão (e o seletor dos testes).
    pub id: &'static str,
    pub nome: &'static str,
    /// O que a dica acrescenta ao nome.
    pub dica: &'static str,
}

const MEIOS: Faixa = Faixa::MeiosTons;

const fn def(
    item: Item,
    grupo: usize,
    letra: Option<char>,
    icone: Icone,
    id: &'static str,
    nome: &'static str,
    dica: &'static str,
) -> DefDeFerramenta {
    DefDeFerramenta {
        item,
        grupo,
        letra,
        icone,
        id,
        nome,
        dica,
    }
}

/// As ferramentas, na ordem da barra do Photoshop; dentro do grupo, a
/// primeira é a que aparece de saída.
pub const FERRAMENTAS: &[DefDeFerramenta] = &[
    def(
        Item::A(Auxiliar::Mover),
        0,
        Some('v'),
        Icone::Move,
        "editor-mover",
        "Mover",
        "Arrasta o conteúdo da camada escolhida; com seleção, leva a seleção",
    ),
    def(
        Item::S(TipoDeSelecao::Retangulo),
        1,
        Some('m'),
        Icone::Square,
        "editor-selecao-retangulo",
        "Seleção retangular",
        "⇧ soma, ⌥ tira, ⇧⌥ cruza; ⇧ no arrasto faz quadrado, ⌥ desenha do centro; arrastar dentro move só o contorno",
    ),
    def(
        Item::S(TipoDeSelecao::Elipse),
        1,
        Some('m'),
        Icone::CircleDashed,
        "editor-selecao-elipse",
        "Seleção elíptica",
        "⇧ soma, ⌥ tira, ⇧⌥ cruza; ⇧ no arrasto faz círculo",
    ),
    def(
        Item::S(TipoDeSelecao::Laco),
        2,
        Some('l'),
        Icone::Lasso,
        "editor-selecao-laco",
        "Laço",
        "Desenhe a seleção à mão; ⇧ soma, ⌥ tira",
    ),
    def(
        Item::S(TipoDeSelecao::LacoPoligonal),
        2,
        Some('l'),
        Icone::Pentagon,
        "editor-selecao-poligonal",
        "Laço poligonal",
        "Clique a clique; fecha no primeiro vértice, duplo clique ou Enter; ⌫ tira o último, Esc cancela",
    ),
    def(
        Item::A(Auxiliar::Varinha),
        3,
        Some('w'),
        Icone::WandSparkles,
        "editor-varinha",
        "Varinha mágica",
        "Clique na cor; ⇧ soma, ⌥ tira, ⇧⌥ cruza",
    ),
    def(
        Item::A(Auxiliar::ContaGotas),
        4,
        Some('i'),
        Icone::Pipette,
        "editor-conta-gotas",
        "Conta-gotas",
        "A cor da foto vira a de frente; com o pincel, ⌥ + clique",
    ),
    def(
        Item::A(Auxiliar::Correcao),
        5,
        Some('j'),
        Icone::Bandage,
        "editor-correcao",
        "Pincel de correção para manchas",
        "Refaz a mancha pelo que está em volta, sem origem",
    ),
    def(
        Item::F(Ferramenta::Recuperacao),
        5,
        Some('j'),
        Icone::Sparkles,
        "editor-recuperacao",
        "Pincel de recuperação",
        "⌥ + clique escolhe a origem; a textura vem dela e a cor se adapta ao destino",
    ),
    def(
        Item::A(Auxiliar::Remendo),
        5,
        Some('j'),
        Icone::Scan,
        "editor-remendo",
        "Remendo",
        "Contorne a área e arraste-a até a pele limpa: a textura vem de lá, a cor fica a daqui",
    ),
    def(
        Item::F(Ferramenta::Pincel),
        6,
        Some('b'),
        Icone::Paintbrush,
        "editor-pincel",
        "Pincel",
        "⇧ + clique liga com uma reta · [ ] tamanho · { } dureza · números: opacidade, ⇧ + números: fluxo",
    ),
    def(
        Item::F(Ferramenta::Misturador),
        6,
        Some('b'),
        Icone::Droplet,
        "editor-misturador",
        "Pincel misturador",
        "Mistura a tinta carregada com a cor da tela · umidade, carga e mistura na barra",
    ),
    def(
        Item::F(Ferramenta::Carimbo),
        7,
        Some('s'),
        Icone::Stamp,
        "editor-carimbo",
        "Carimbo",
        "⌥ + clique escolhe a origem",
    ),
    def(
        Item::F(Ferramenta::Borracha),
        8,
        Some('e'),
        Icone::Eraser,
        "editor-borracha",
        "Borracha",
        "⇧ + clique liga com uma reta; na máscara pinta a cor de fundo",
    ),
    def(
        Item::A(Auxiliar::Degrade),
        9,
        Some('g'),
        Icone::Gradient,
        "editor-degrade",
        "Degradê",
        "Arraste do começo ao fim; ⇧ prende em 45°. Na máscara, frente → fundo",
    ),
    def(
        Item::A(Auxiliar::Lata),
        9,
        Some('g'),
        Icone::PaintBucket,
        "editor-lata",
        "Lata de tinta",
        "Pinta com a cor de frente a área parecida em volta do clique",
    ),
    def(
        Item::F(Ferramenta::Desfoque),
        10,
        None,
        Icone::Droplet,
        "editor-desfoque",
        "Desfoque",
        "Suaviza onde passa (sem letra, como no Photoshop)",
    ),
    def(
        Item::F(Ferramenta::Nitidez),
        10,
        None,
        Icone::Triangle,
        "editor-nitidez",
        "Nitidez",
        "Realça os detalhes onde passa (sem letra, como no Photoshop)",
    ),
    def(
        Item::F(Ferramenta::Subexposicao(MEIOS)),
        11,
        Some('o'),
        Icone::Sun,
        "editor-subexposicao",
        "Subexposição",
        "Clareia a faixa de tons escolhida",
    ),
    def(
        Item::F(Ferramenta::Superexposicao(MEIOS)),
        11,
        Some('o'),
        Icone::Moon,
        "editor-superexposicao",
        "Superexposição",
        "Escurece a faixa de tons escolhida",
    ),
    // ✒️ Desenho: a Caneta (P) com Adicionar, Excluir e Converter ponto (sem
    // letra, como no Photoshop); Seleção de caminho e Seleção direta (A).
    def(
        Item::P(FV::Caneta),
        12,
        Some('p'),
        Icone::PenTool,
        "editor-caneta",
        "Caneta",
        "Clique faz canto, arraste faz curva; clique no primeiro ponto fecha. ⌘ seleção direta, ⌥ converte, ⇧ 45°; Enter termina aberto",
    ),
    def(
        Item::P(FV::Curvatura),
        12,
        Some('p'),
        Icone::PenLine,
        "editor-caneta-de-curvatura",
        "Caneta de curvatura",
        "Clique põe um ponto por onde a curva passa lisa; duplo clique ou ⌥ faz canto; arraste um ponto para movê-lo; clique no primeiro fecha",
    ),
    def(
        Item::P(FV::AdicionarPonto),
        12,
        None,
        Icone::CirclePlus,
        "editor-adicionar-ponto",
        "Adicionar ponto de ancoragem",
        "Clique num segmento: a âncora entra sem mudar a curva",
    ),
    def(
        Item::P(FV::ExcluirPonto),
        12,
        None,
        Icone::CircleMinus,
        "editor-excluir-ponto",
        "Excluir ponto de ancoragem",
        "Clique numa âncora: as vizinhas se ligam com as alças delas",
    ),
    def(
        Item::P(FV::ConverterPonto),
        12,
        None,
        Icone::Spline,
        "editor-converter-ponto",
        "Converter ponto",
        "Arraste numa âncora para puxar alças; clique para fazer canto; arraste uma alça para soltá-la da outra",
    ),
    def(
        Item::P(FV::SelecaoDeCaminho),
        13,
        Some('a'),
        Icone::MousePointer,
        "editor-selecao-de-caminho",
        "Seleção de caminho",
        "Escolhe e move componentes inteiros; ⇧ soma, ⌥ + arrasto duplica, Delete exclui",
    ),
    def(
        Item::P(FV::SelecaoDireta),
        13,
        Some('a'),
        Icone::MousePointer2,
        "editor-selecao-direta",
        "Seleção direta",
        "Escolhe e move âncoras e alças; ⇧ soma, arraste no vazio para o retângulo, setas empurram",
    ),
    def(
        Item::A(Auxiliar::Mao),
        14,
        Some('h'),
        Icone::Hand,
        "editor-mao",
        "Mão",
        "Arrasta a foto ampliada; ou segure o Espaço com qualquer ferramenta",
    ),
    def(
        Item::A(Auxiliar::GirarVista),
        14,
        Some('r'),
        Icone::RotateCw,
        "editor-girar-vista",
        "Girar vista",
        "Gira só a tela (⇧ de 15° em 15°); Esc volta a 0°. Nenhum pixel muda",
    ),
    def(
        Item::A(Auxiliar::Zoom),
        15,
        Some('z'),
        Icone::ZoomIn,
        "editor-lupa",
        "Lupa",
        "Clique amplia; ⌥ + clique reduz",
    ),
];

/// Quantos grupos a barra tem.
pub const QUANTOS_GRUPOS: usize = 16;

/// Depois de que grupos vem um separador (as seções da barra do Photoshop:
/// mover e seleção; medida; retoque e pintura; desenho; navegação).
pub const SEPARADOR_DEPOIS: &[usize] = &[0, 4, 11, 13];

/// A definição de uma ferramenta (a subexposição vale com qualquer faixa).
pub fn def_de(item: &Item) -> Option<&'static DefDeFerramenta> {
    FERRAMENTAS.iter().find(|d| d.item.mesma(item))
}

/// As ferramentas de um grupo da barra, na ordem.
pub fn do_grupo(grupo: usize) -> impl Iterator<Item = &'static DefDeFerramenta> {
    FERRAMENTAS.iter().filter(move |d| d.grupo == grupo)
}

/// As ferramentas de uma letra, na ordem (⇧ + letra anda nelas).
pub fn da_letra(letra: char) -> impl Iterator<Item = &'static DefDeFerramenta> {
    FERRAMENTAS.iter().filter(move |d| d.letra == Some(letra))
}

/// A letra de uma ferramenta (`None`: desfoque e nitidez).
pub fn letra_de(item: &Item) -> Option<char> {
    def_de(item).and_then(|d| d.letra)
}

/// O nome com a letra, como no Photoshop ("Pincel (B)").
pub fn nome_com_letra(d: &DefDeFerramenta) -> String {
    match d.letra {
        Some(l) => format!("{} ({})", d.nome, l.to_ascii_uppercase()),
        None => d.nome.to_string(),
    }
}

/// O texto com os símbolos de tecla da plataforma: no Windows e no Linux o
/// ⌘ é Ctrl, o ⌥ é Alt e o ⇧ é Shift — nenhum símbolo do macOS fora dele.
pub fn na_plataforma(texto: &str) -> String {
    if cfg!(target_os = "macos") {
        return texto.to_string();
    }
    let mut s = texto.to_string();
    for (simbolo, nome) in [("⌃", "Ctrl"), ("⇧", "Shift"), ("⌥", "Alt"), ("⌘", "Ctrl")] {
        s = s.replace(&format!("{simbolo} "), &format!("{nome} "));
        s = s.replace(simbolo, &format!("{nome}+"));
    }
    s.replace('⌫', "Backspace").replace('⏎', "Enter")
}

/// A dica de um botão da barra: nome, letra, o ⇧ + letra quando há
/// variantes na mesma letra, e o que a ferramenta faz.
pub fn dica_da_ferramenta(d: &DefDeFerramenta) -> String {
    let mut dica = nome_com_letra(d);
    if let Some(l) = d.letra {
        if da_letra(l).count() > 1 {
            dica.push_str(&format!(" — ⇧{} alterna no grupo", l.to_ascii_uppercase()));
        }
    }
    if !d.dica.is_empty() {
        dica.push('\n');
        dica.push_str(d.dica);
    }
    if do_grupo(d.grupo).count() > 1 {
        dica.push_str("\nBotão direito ou aperto longo: as outras do grupo");
    }
    na_plataforma(&dica)
}

/// O flyout aberto: o grupo, a linha realçada pelo teclado e onde ele abre.
#[derive(Clone, Copy, Debug)]
pub struct Flyout {
    pub grupo: usize,
    pub destaque: usize,
    pub onde: Point<Pixels>,
}

impl EditorDeFoto {
    /// A ferramenta que o grupo mostra: a última usada nele, ou a primeira.
    pub fn mostrada_no_grupo(&self, grupo: usize) -> &'static DefDeFerramenta {
        self.mostrada_na_barra
            .get(grupo)
            .copied()
            .flatten()
            .and_then(|item| def_de(&item))
            .or_else(|| do_grupo(grupo).next())
            .expect("todo grupo tem ferramenta")
    }

    /// O grupo da ferramenta na mão.
    fn grupo_atual(&self) -> Option<usize> {
        self.item_atual().and_then(|i| def_de(&i)).map(|d| d.grupo)
    }

    /// O grupo passa a mostrar esta ferramenta (e a letra volta a ela).
    pub(super) fn lembrar_na_barra(&mut self, item: Item) {
        if let Some(d) = def_de(&item) {
            if let Some(slot) = self.mostrada_na_barra.get_mut(d.grupo) {
                *slot = Some(d.item);
            }
            if let Some(letra) = d.letra {
                self.ultima_do_grupo.insert(letra, d.item);
            }
        }
    }

    /// A subexposição e a superexposição levam a faixa escolhida.
    fn com_a_faixa(&self, item: Item) -> Item {
        match item {
            Item::F(Ferramenta::Subexposicao(_)) => Item::F(Ferramenta::Subexposicao(self.faixa)),
            Item::F(Ferramenta::Superexposicao(_)) => {
                Item::F(Ferramenta::Superexposicao(self.faixa))
            }
            outro => outro,
        }
    }

    /// A letra de um grupo: a última ferramenta usada nele; com `proxima`
    /// (⇧ + letra), a seguinte, em volta.
    pub fn pela_letra(&mut self, letra: char, proxima: bool, cx: &mut Context<Self>) {
        // No Preenchimento sensível ao conteúdo, as letras são as do espaço
        // dele (B, L, H, Z), como no Photoshop.
        if self.area_do_preenchimento.is_some() {
            if let Some(f) =
                super::painel_do_preenchimento::FerramentaDoPreenchimento::da_letra(letra)
            {
                self.escolher_ferramenta_do_preenchimento(f, cx);
            }
            return;
        }
        let itens: Vec<Item> = da_letra(letra).map(|d| d.item).collect();
        let Some(&primeira) = itens.first() else {
            return;
        };
        self.fechar_flyout(cx);
        let ultima = self.ultima_do_grupo.get(&letra).copied();
        let atual = self.item_atual().filter(|i| letra_de(i) == Some(letra));
        let mut item = ultima.unwrap_or(primeira);
        if proxima {
            let de = atual.or(ultima).unwrap_or(primeira);
            let i = itens.iter().position(|x| x.mesma(&de)).unwrap_or(0);
            item = itens[(i + 1) % itens.len()];
        }
        let item = self.com_a_faixa(item);
        self.usar_item(item, cx);
    }

    /// Clique simples num grupo: a ferramenta que ele mostra.
    fn usar_o_grupo(&mut self, grupo: usize, cx: &mut Context<Self>) {
        let item = self.com_a_faixa(self.mostrada_no_grupo(grupo).item);
        self.usar_item(item, cx);
    }

    // ---------------------------------------------------------- o flyout

    /// Abre o flyout do grupo ao lado do botão (o realce começa na mostrada).
    pub fn abrir_flyout(&mut self, grupo: usize, cx: &mut Context<Self>) {
        let mostrada = self.mostrada_no_grupo(grupo);
        let destaque = do_grupo(grupo)
            .position(|d| d.item.mesma(&mostrada.item))
            .unwrap_or(0);
        let botao = self.caixas_da_barra.get(grupo).copied().unwrap_or_default();
        let onde = gpui_kit::point(botao.right() + px(4.), botao.top());
        self.flyout = Some(Flyout {
            grupo,
            destaque,
            onde,
        });
        cx.notify();
    }

    pub fn fechar_flyout(&mut self, cx: &mut Context<Self>) {
        self.aperto_na_barra = None;
        if self.flyout.take().is_some() {
            cx.notify();
        }
    }

    pub fn flyout_aberto(&self) -> Option<usize> {
        self.flyout.map(|f| f.grupo)
    }

    /// A linha `indice` do flyout aberto vira a ferramenta (e o ícone do
    /// grupo).
    pub fn escolher_no_flyout(&mut self, indice: usize, cx: &mut Context<Self>) {
        let Some(f) = self.flyout else {
            return;
        };
        let Some(d) = do_grupo(f.grupo).nth(indice) else {
            return;
        };
        self.flyout = None;
        let item = self.com_a_faixa(d.item);
        self.usar_item(item, cx);
    }

    /// ↑ ↓ Enter e Esc com o flyout aberto. Devolve se a tecla era dele.
    pub(super) fn tecla_no_flyout(&mut self, tecla: &str, cx: &mut Context<Self>) -> bool {
        let Some(mut f) = self.flyout else {
            return false;
        };
        let quantas = do_grupo(f.grupo).count().max(1);
        match tecla {
            "down" => f.destaque = (f.destaque + 1) % quantas,
            "up" => f.destaque = (f.destaque + quantas - 1) % quantas,
            "enter" => {
                self.escolher_no_flyout(f.destaque, cx);
                return true;
            }
            "escape" => {
                self.fechar_flyout(cx);
                return true;
            }
            _ => return false,
        }
        self.flyout = Some(f);
        cx.notify();
        true
    }

    /// O botão do grupo foi apertado: depois do aperto longo, o flyout.
    fn apertar_no_grupo(&mut self, grupo: usize, cx: &mut Context<Self>) {
        self.aperto_na_barra = Some(grupo);
        self.abriu_pelo_aperto = false;
        if do_grupo(grupo).count() < 2 {
            return;
        }
        self._tarefa_do_aperto = Some(cx.spawn(async move |ed, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(medida::APERTO_PROLONGADO_MS))
                .await;
            let _ = ed.update(cx, |ed, cx| {
                if ed.aperto_na_barra == Some(grupo) {
                    ed.aperto_na_barra = None;
                    ed.abriu_pelo_aperto = true;
                    ed.abrir_flyout(grupo, cx);
                }
            });
        }));
    }

    /// Antes de mudar de ferramenta pela barra: o nome sendo editado termina,
    /// e o foco volta ao palco (as letras e os números seguem valendo).
    fn foco_de_volta(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        if self.renomeando.is_some() {
            self.terminar_de_renomear(true, window, cx);
        }
        window.focus(&self.foco, cx);
    }

    /// A barra de ferramentas vertical, à esquerda do palco.
    pub(super) fn barra_de_ferramentas(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let duas = self.barra_em_duas_colunas;
        let largura = if duas {
            medida::BARRA_DUAS_COLUNAS
        } else {
            medida::BARRA_UMA_COLUNA
        };
        let atual = self.grupo_atual();
        let botao_do_grupo = |grupo: usize, cx: &mut Context<Self>| -> AnyElement {
            let d = self.mostrada_no_grupo(grupo);
            let variantes = do_grupo(grupo).count() > 1;
            let ativa = atual == Some(grupo);
            let medidor = cx.entity();
            let botao = crate::estilo::botao_icone(
                d.id,
                d.icone,
                medida::BOTAO_DA_FERRAMENTA,
                medida::ICONE_DA_FERRAMENTA,
            )
            .rounded(px(medida::CANTO))
            .tooltip(dica_da_ferramenta(d))
            .on_click(cx.listener(move |ed, _, window, cx| {
                ed.aperto_na_barra = None;
                if std::mem::take(&mut ed.abriu_pelo_aperto) {
                    return;
                }
                ed.foco_de_volta(window, cx);
                ed.fechar_flyout(cx);
                ed.usar_o_grupo(grupo, cx);
            }));
            let botao = if ativa { botao.primary() } else { botao };
            div()
                .id(("editor-grupo", grupo))
                .debug_selector(move || format!("editor-grupo-{grupo}"))
                .relative()
                .size(px(medida::BOTAO_DA_FERRAMENTA))
                .child(
                    // A caixa do botão, para o flyout abrir ao lado dele.
                    gpui_kit::canvas(
                        move |caixa, _, cx| {
                            medidor.update(cx, |ed, _| {
                                if let Some(c) = ed.caixas_da_barra.get_mut(grupo) {
                                    *c = caixa;
                                }
                            })
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .child(botao)
                .when(variantes, |d| {
                    // O triângulo do canto: há outras ferramentas no grupo.
                    d.child(
                        div()
                            .absolute()
                            .right(px(2.))
                            .bottom(px(1.))
                            .text_size(crate::tema::letra::em(7.))
                            .line_height(px(7.))
                            .text_color(if ativa { c.ativo_texto } else { c.apagado })
                            .child("◢"),
                    )
                })
                .capture_any_mouse_down(cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                    match e.button {
                        MouseButton::Left => ed.apertar_no_grupo(grupo, cx),
                        MouseButton::Right => {
                            ed.foco_de_volta(window, cx);
                            if ed.flyout_aberto() == Some(grupo) {
                                ed.fechar_flyout(cx);
                            } else {
                                ed.abrir_flyout(grupo, cx);
                            }
                        }
                        _ => {}
                    }
                }))
                .capture_any_mouse_up(cx.listener(|ed, _: &gpui_kit::MouseUpEvent, _, _| {
                    ed.aperto_na_barra = None;
                }))
                .into_any_element()
        };

        let mut grupos = div()
            .flex()
            .when(duas, |d| d.flex_wrap().justify_center().px(px(3.)))
            .when(!duas, |d| d.flex_col().items_center())
            .gap(px(2.));
        for grupo in 0..QUANTOS_GRUPOS {
            grupos = grupos.child(botao_do_grupo(grupo, cx));
            if SEPARADOR_DEPOIS.contains(&grupo) {
                grupos = grupos.child(div().my(px(3.)).h(px(1.)).w(px(largura - 14.)).bg(c.borda));
            }
        }

        let alternar = crate::estilo::botao_icone(
            "editor-barra-colunas",
            if duas {
                Icone::ChevronsLeft
            } else {
                Icone::ChevronsRight
            },
            18.,
            12.,
        )
        .tooltip(if duas {
            "Ferramentas em uma coluna"
        } else {
            "Ferramentas em duas colunas"
        })
        .on_click(cx.listener(|ed, _, window, cx| {
            ed.foco_de_volta(window, cx);
            ed.alternar_colunas_da_barra(cx);
        }));

        div()
            .id("editor-barra-de-ferramentas")
            .debug_selector(|| "editor-barra-de-ferramentas".into())
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(largura))
            .h_full()
            .bg(c.cromo)
            .border_r_1()
            .border_color(c.borda)
            .child(
                div()
                    .flex()
                    .justify_center()
                    .h(px(20.))
                    .items_center()
                    .child(alternar),
            )
            // 🔑 Numa janela baixa a lista rola, e as cores ficam embaixo,
            // sempre à vista.
            .child(
                div()
                    .id("editor-lista-de-ferramentas")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .py(px(2.))
                    .child(grupos),
            )
            .child(self.cores_da_barra(cx))
            .into_any_element()
    }

    /// As cores de frente e de fundo, sobrepostas, com trocar e
    /// preto-e-branco.
    fn cores_da_barra(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let lado = medida::AMOSTRA_DA_COR;
        let passo = medida::DESLOCAMENTO_DO_FUNDO;
        let caixa = lado + passo;
        let miuda = |id: &'static str, icone: Icone, dica: &str| {
            crate::estilo::botao_icone(id, icone, 14., 10.).tooltip(na_plataforma(dica))
        };
        div()
            .flex()
            .justify_center()
            .py(px(8.))
            .border_t_1()
            .border_color(c.borda)
            .child(
                div()
                    .relative()
                    .size(px(caixa + 8.))
                    // O fundo, embaixo e à direita — desenhado antes, fica
                    // por baixo da frente.
                    .child(
                        div()
                            .absolute()
                            .left(px(passo))
                            .top(px(passo))
                            .debug_selector(|| "editor-cor-de-fundo".into())
                            .child(
                                ColorPicker::new(&self.seletor_de_fundo)
                                    .with_size(px(lado))
                                    .featured_colors(super::amostras_em_hsla()),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(0.))
                            .debug_selector(|| "editor-seletor-de-cor".into())
                            .child(
                                ColorPicker::new(&self.seletor_de_cor)
                                    .with_size(px(lado))
                                    .featured_colors(super::amostras_em_hsla()),
                            ),
                    )
                    .child(
                        div().absolute().right(px(-2.)).top(px(-4.)).child(
                            miuda(
                                "editor-trocar-cores",
                                Icone::ArrowLeftRight,
                                "Trocar frente e fundo (X)",
                            )
                            .on_click(cx.listener(
                                |ed, _, window, cx| {
                                    window.focus(&ed.foco, cx);
                                    ed.trocar_cores(cx)
                                },
                            )),
                        ),
                    )
                    .child(
                        div()
                            .id("editor-cores-padrao")
                            .debug_selector(|| "editor-cores-padrao".into())
                            .absolute()
                            .left(px(-2.))
                            .bottom(px(-4.))
                            .size(px(12.))
                            .cursor_pointer()
                            .tooltip(|window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(
                                    "Frente preta e fundo branco (D)",
                                )
                                .build(window, cx)
                            })
                            .on_click(cx.listener(|ed, _, window, cx| {
                                window.focus(&ed.foco, cx);
                                ed.cores_padrao(cx)
                            }))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(4.))
                                    .top(px(4.))
                                    .size(px(7.))
                                    .bg(gpui_kit::white())
                                    .border_1()
                                    .border_color(c.apagado),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(px(0.))
                                    .top(px(0.))
                                    .size(px(7.))
                                    .bg(gpui_kit::black())
                                    .border_1()
                                    .border_color(c.apagado),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// O flyout aberto, por cima de tudo e dentro da janela.
    pub(super) fn flyout_da_barra(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.flyout?;
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let mostrada = self.mostrada_no_grupo(f.grupo);
        let linhas = do_grupo(f.grupo).enumerate().map(|(i, d)| {
            let escolhida = d.item.mesma(&mostrada.item);
            let realcada = i == f.destaque;
            let seletor: SharedString = format!("editor-flyout-{}", d.id).into();
            div()
                .id(("editor-flyout-linha", i))
                .debug_selector(move || seletor.to_string())
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(medida::LINHA_DO_FLYOUT))
                .px(px(8.))
                .text_size(crate::tema::letra::em(aparencia::LETRA))
                .cursor_pointer()
                .when(realcada, |d| d.bg(c.realce))
                .hover(|d| d.bg(c.realce))
                .child(div().w(px(10.)).text_color(c.alvo).child(if escolhida {
                    "■"
                } else {
                    ""
                }))
                .child(
                    gpui_kit::component::Icon::new(d.icone)
                        .size(px(16.))
                        .text_color(c.texto),
                )
                .child(div().flex_1().child(d.nome))
                .child(
                    div().text_color(c.apagado).child(
                        d.letra
                            .map(|l| l.to_ascii_uppercase().to_string())
                            .unwrap_or_default(),
                    ),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ed, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        window.focus(&ed.foco, cx);
                        ed.escolher_no_flyout(i, cx);
                    }),
                )
        });
        Some(
            deferred(
                anchored()
                    .position(f.onde)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("editor-flyout")
                            .debug_selector(|| "editor-flyout".into())
                            .occlude()
                            .w(px(medida::LARGURA_DO_FLYOUT))
                            .py(px(4.))
                            .bg(tema.popover)
                            .text_color(tema.popover_foreground)
                            .border_1()
                            .border_color(c.borda)
                            .rounded(px(medida::CANTO))
                            .shadow_md()
                            .on_mouse_down_out(cx.listener(|ed, _, _, cx| ed.fechar_flyout(cx)))
                            .children(linhas),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// Uma coluna ↔ duas colunas (gravado com a área de trabalho).
    pub fn alternar_colunas_da_barra(&mut self, cx: &mut Context<Self>) {
        self.barra_em_duas_colunas = !self.barra_em_duas_colunas;
        self.fechar_flyout(cx);
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// As caixas dos botões da barra (do último quadro) — para os testes.
    pub fn caixa_do_grupo(&self, grupo: usize) -> Option<Bounds<Pixels>> {
        self.caixas_da_barra.get(grupo).copied()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Toda ferramenta da tabela tem grupo válido e id único, e todo grupo
    /// tem pelo menos uma — a barra, as letras e as dicas saem desta tabela.
    #[test]
    fn a_tabela_da_barra_e_coerente() {
        let mut ids = std::collections::HashSet::new();
        for d in FERRAMENTAS {
            assert!(d.grupo < QUANTOS_GRUPOS, "{} fora dos grupos", d.id);
            assert!(ids.insert(d.id), "{} repetido", d.id);
            assert_eq!(def_de(&d.item).map(|x| x.id), Some(d.id));
        }
        for g in 0..QUANTOS_GRUPOS {
            assert!(do_grupo(g).count() >= 1, "grupo {g} vazio");
        }
    }

    /// As letras do Photoshop: Z é a Lupa, H a Mão e R o Girar vista (no
    /// mesmo grupo da barra), J anda nas três de correção, desfoque e nitidez
    /// sem letra.
    #[test]
    fn as_letras_sao_as_do_photoshop() {
        assert_eq!(letra_de(&Item::A(Auxiliar::Zoom)), Some('z'));
        assert_eq!(letra_de(&Item::A(Auxiliar::Mao)), Some('h'));
        assert_eq!(letra_de(&Item::A(Auxiliar::GirarVista)), Some('r'));
        assert_eq!(
            def_de(&Item::A(Auxiliar::Mao)).map(|d| d.grupo),
            def_de(&Item::A(Auxiliar::GirarVista)).map(|d| d.grupo)
        );
        assert_eq!(da_letra('j').count(), 3);
        // P anda na Caneta e na de curvatura (as de ponto não têm letra); A
        // anda nas duas setas de caminho.
        assert_eq!(da_letra('p').count(), 2);
        assert_eq!(letra_de(&Item::P(FV::AdicionarPonto)), None);
        assert_eq!(da_letra('a').count(), 2);
        assert_eq!(do_grupo(12).count(), 5);
        assert_eq!(letra_de(&Item::F(Ferramenta::Desfoque)), None);
        assert_eq!(
            letra_de(&Item::F(Ferramenta::Subexposicao(Faixa::Realces))),
            Some('o'),
            "a faixa não muda a ferramenta"
        );
    }

    #[test]
    fn a_dica_fala_a_lingua_da_plataforma() {
        let d = def_de(&Item::F(Ferramenta::Carimbo)).unwrap();
        let dica = dica_da_ferramenta(d);
        assert!(dica.starts_with("Carimbo (S)"));
        if !cfg!(target_os = "macos") {
            assert!(!dica.contains('⌥') && dica.contains("Alt"), "{dica}");
        }
        assert_eq!(
            if cfg!(target_os = "macos") {
                na_plataforma("⇧⌥ cruza")
            } else {
                na_plataforma("⇧⌥ cruza").replace("Shift+Alt", "⇧⌥")
            },
            "⇧⌥ cruza"
        );
    }
}
