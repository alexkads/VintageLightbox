//! A segunda tela: a foto grande, em fundo preto, no outro monitor.
//!
//! É o "Client View" do legado (`components/secondary_window.rs`) — a janela que
//! o fotógrafo vira para o cliente enquanto tria: sem barra de título, sem
//! controle nenhum, só a foto que está selecionada na janela principal.
//!
//! ## 🔑 O que muda do egui para o GPUI
//!
//! Lá é um `ViewportDeferred` dentro do mesmo `Context`, e a comunicação entre
//! as duas telas passa por `ctx.data_mut` com chaves de texto
//! (`"secondary_window_close_req"`) — memória global compartilhada, porque o
//! closure do viewport não alcança o estado do app. O `Esc` de lá **não fecha a
//! janela**: ele grava um sinalizador que o quadro seguinte da janela principal
//! lê e consome.
//!
//! Aqui é uma janela de verdade, com entidade própria: fechar é fechar, e a foto
//! chega por chamada de método. O que some junto é a necessidade de o app
//! principal repintar a 60fps só para manter a segunda tela em dia — o legado
//! faz `ctx.request_repaint()` incondicional nos dois lados enquanto ela estiver
//! aberta.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::comparar::Lado;
use biblioteca_core::grade_do_cliente as conta;
use domain::value_objects::CropSettings;
use gpui_kit::component::InteractiveElementExt as _;
use gpui_kit::{
    actions, div, ease_in_out, img, point, prelude::*, px, size, Animation, AnimationExt, Bounds,
    Context, FocusHandle, Pixels, RenderImage, SharedString, Size, Task, Window,
};
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::transformacao;

use crate::revelacao::processador::{Ajustes, ParaTela, Pedido, Processador};

pub mod nome_do_monitor;

/// O que a raiz manda revelar na segunda tela: a foto **sem marca** e a
/// revelação dela.
///
/// 🔑 **É o desenho do site** (`FotoNaTelaDoCliente`): a imagem é a cópia de
/// trabalho, sem marca d'água, e a janela do cliente tem o próprio motor, que
/// aplica os ajustes. Até 2026-09-17 ia a prévia do cache, que para foto do
/// site é a da galeria — com a marca d'água que o cliente não deveria ver no
/// balcão (dono: *"mostrou com marca d'agua"*).
pub struct ParaRevelar {
    pub foto: PhotoViewModel,
    pub posicao: Option<(usize, usize)>,
    /// Os pixels sem marca, em RGBA8. `Arc` porque a identidade dele diz ao
    /// motor se a textura precisa subir de novo.
    pub pixels: Arc<Vec<u8>>,
    pub largura: u32,
    pub altura: u32,
    pub ajustes: Ajustes,
    pub corte: CropSettings,
    /// A Revelação local: sem ela o cliente veria outra foto.
    pub locais: Arc<infrastructure::gpu_adjustments::ParametrosLocais>,
}

/// O pedido no motor: o id dele, a foto, onde ela está na sequência e o
/// enquadramento a aplicar no resultado.
type EmRevelacao = (u64, PhotoViewModel, Option<(usize, usize)>, CropSettings);

/// Uma metade do Comparar (`⇧C`) na segunda tela.
struct MetadeNoCliente {
    foto: PhotoViewModel,
    posicao: Option<(usize, usize)>,
    /// `None` enquanto o motor revela: a metade mostra o aviso, e não a crua.
    imagem: Option<Arc<RenderImage>>,
    /// A revelação com que a imagem acima foi revelada — o que decide se vale
    /// revelar de novo quando a raiz reenvia o par (uma nota, a escolhida).
    parametros: (Ajustes, CropSettings),
}

/// As duas fotos do Comparar, e qual o operador está avaliando.
struct ParNoCliente {
    esquerda: MetadeNoCliente,
    direita: MetadeNoCliente,
    ativa: Lado,
}

/// Uma foto na grade da segunda tela.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemDaGrade {
    /// O número embaixo da foto: a posição dela no recorte do operador,
    /// contando de 1 — o mesmo "7 / 25" do rodapé da foto única.
    pub numero: usize,
    /// A chave da imagem no cache de previews — a mesma que a galeria do
    /// operador desenha (a prévia local não salva vence a do site).
    pub chave: String,
    /// O cliente já disse que leva esta.
    pub escolhida: bool,
    /// ⭐ A nota de 1 a 5 — zero é sem nota. Vai em estrelas sobre a foto
    /// (dono, 12/out/2026: *"preciso que apareça as estrelinhas da
    /// classificação"*).
    pub nota: u8,
}

/// A grade que a raiz manda à segunda tela: as fotos do recorte do operador
/// que o cliente vê, qual está em foco e quantas vão por linha.
///
/// 🔑 **O número vem pronto da raiz**, e não é contado aqui: a rejeitada e a
/// apagada ficam de fora e o número delas fica vago — a "12" do cliente é a
/// "12" do balconista (`biblioteca_core::grade_do_cliente`).
#[derive(Debug, Clone, PartialEq)]
pub struct GradeParaOCliente {
    pub itens: Vec<ItemDaGrade>,
    /// O índice, em `itens`, da foto em foco do operador.
    pub foco: Option<usize>,
    pub por_linha: usize,
}

/// A faixa do pé da grade: quais fotos e qual página estão no ar.
const PE_DA_GRADE: f32 = 30.;

/// A borda da foto em foco na grade — a âmbar do Comparar.
const BORDA_DA_GRADE: f32 = 4.;

/// Acima deste lado, em pixels do dispositivo, a célula pede a prévia grande:
/// é o lado da miniatura do cache (`sessoes::detalhe::LADO_DA_MINIATURA`).
const LADO_DA_MINIATURA: f32 = 320.;

/// Quantas prévias grandes a grade decodifica por quadro. As outras entram
/// com a miniatura e trocam no quadro seguinte.
const GRANDES_POR_QUADRO: usize = 4;

/// Quanto dura a entrada de uma página da grade.
const ENTRADA_DA_PAGINA: Duration = Duration::from_millis(350);

/// Quanto dura o cruzamento entre uma foto e a seguinte.
///
/// 🔑 **Um segundo, como na web desde 2026-09-11.** Meio segundo era o número
/// herdado da galeria, onde o operador atravessa a tira com a seta e a
/// transição vira melado — mas esta é a tela do **cliente**: quem olha não está
/// navegando, está decidindo se leva a foto, e o que ele viu foi *"tá muito
/// rápido"* (dono). O `ease_in_out` da animação é o que faz o tempo maior ser
/// percebido como apresentação e não como lentidão: o `ease_out` sai do zero a
/// toda velocidade e a foto aparece de estalo.
const CRUZAMENTO: Duration = Duration::from_millis(1000);

actions!(
    vintagelightbox,
    [
        FecharCliente,
        AlternarInfoDoCliente,
        AlternarTelaCheiaDoCliente
    ]
);

/// O contexto de teclado da segunda tela.
///
/// Próprio, e não o da raiz: as duas janelas existem ao mesmo tempo, e `Esc` na
/// principal volta para a Biblioteca enquanto `Esc` aqui fecha esta janela. Com
/// um contexto só, a tecla faria as duas coisas em janelas diferentes conforme
/// quem estivesse com o foco — que é o pior desfecho possível para uma tecla que
/// fecha coisas.
const CONTEXTO: &str = "Cliente";

/// O atalho da tela cheia: `⌘⇧F` no Mac, `Ctrl+Shift+F` no resto. Vale
/// dentro da tela do cliente e na janela principal (`app::TelaCheiaDoCliente`).
pub const ATALHO_DA_TELA_CHEIA: &str = "secondary-shift-f";

/// O atalho como aparece nos botões — `⌘⇧F` no Mac, `Ctrl+Shift+F` no resto.
pub fn tecla_da_tela_cheia() -> gpui_kit::component::kbd::Kbd {
    gpui_kit::component::kbd::Kbd::new(
        gpui_kit::Keystroke::parse(ATALHO_DA_TELA_CHEIA).expect("o atalho é válido"),
    )
}

/// O mesmo atalho, em texto.
pub fn texto_da_tela_cheia() -> String {
    gpui_kit::component::kbd::Kbd::format(
        &gpui_kit::Keystroke::parse(ATALHO_DA_TELA_CHEIA).expect("o atalho é válido"),
    )
}

pub fn init(cx: &mut gpui_kit::App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("escape", FecharCliente, Some(CONTEXTO)),
        // `I` liga e desliga o rodapé com nome e nota, como no legado.
        gpui_kit::KeyBinding::new("i", AlternarInfoDoCliente, Some(CONTEXTO)),
        // 🔑 **`F` (ou `⌘⇧F`/`Ctrl+Shift+F`) põe e tira a tela cheia, na
        // própria janela** —
        // o gesto do darktable (dono, 22/set/2026). Arrastar até o monitor do
        // cliente e apertar `F` é o caminho que funciona até no GNOME, onde o
        // app não escolhe monitor nenhum. `J` era a troca de modo antiga e
        // continua valendo, para não trair a mão de quem já a usava.
        gpui_kit::KeyBinding::new("f", AlternarTelaCheiaDoCliente, Some(CONTEXTO)),
        // Sem `F11`: o teclado do Mac do dono não tem F1–F12 (22/set/2026).
        gpui_kit::KeyBinding::new(
            ATALHO_DA_TELA_CHEIA,
            AlternarTelaCheiaDoCliente,
            Some(CONTEXTO),
        ),
        gpui_kit::KeyBinding::new("j", AlternarTelaCheiaDoCliente, Some(CONTEXTO)),
    ]);
}

/// Em qual monitor a segunda tela abre.
///
/// A regra é a do legado: **o primeiro que não for o principal**, e o principal
/// se não houver outro. Com uma tela só ela abre por cima da janela do app, que
/// é o que permite conferir a funcionalidade sem um segundo monitor plugado —
/// e é o que o `MonitorDetector::get_secondary_monitor` de lá faz.
///
/// 🔑 **Recebe os ids em vez de ler o sistema**, e é genérica no tipo deles:
/// o `DisplayId` do GPUI **não pode ser construído de fora do crate** (o campo é
/// `pub(crate)`), então uma assinatura concreta só poderia ser conferida numa
/// máquina com dois monitores plugados — que é o mesmo que não conferir.
pub fn monitor_do_cliente<T: Copy + PartialEq>(todos: &[T], principal: Option<T>) -> Option<T> {
    todos
        .iter()
        .find(|id| Some(**id) != principal)
        .or_else(|| todos.first())
        .copied()
}

/// O tamanho da janela quando **não** há segundo monitor.
///
/// 🔑 **É o mesmo tamanho que o pop-up da tela do cliente tem no site** (dono,
/// 2026-09-17). Um número igual dos dois lados é o que permite conferir por
/// foto das duas janelas lado a lado.
const PREVIA: (f32, f32) = (1280., 800.);

/// Onde a segunda tela nasce — e **nunca em tela cheia**.
///
/// 🚨 **Era `WindowBounds::Fullscreen`, e isso foi defeito** (dono,
/// 17/set/2026). Tela cheia no macOS não é uma janela grande: é um *Space*
/// próprio, que o sistema tira do monitor onde a janela ia nascer e joga por
/// cima de tudo. Num Mac com um monitor só — que é como esta tela se confere
/// (D10) — o resultado era o app sumir atrás dela.
///
/// Aqui a janela é sempre `Windowed`, e o que muda é o tamanho:
///
/// - **Com monitor próprio**, ela recebe os limites dele inteiros, origem
///   incluída. A origem é o que a coloca no monitor certo: os `Bounds` do GPUI
///   são globais, e um `display_id` sem origem deixa o sistema escolher.
/// - **Sem monitor próprio**, ela é uma janela comum de [`PREVIA`], centrada e
///   arrastável. Quem está olhando é o operador, e o que ele precisa é
///   comparar com o app, não perdê-lo de vista.
///
/// Numa tela menor que [`PREVIA`] a janela não estoura o monitor: o tamanho é
/// limitado ao que existe. Sem isso a origem centrada ficaria **negativa** e a
/// janela nasceria com o canto fora da tela.
pub fn area_do_cliente(tela: Bounds<Pixels>, monitor_proprio: bool) -> Bounds<Pixels> {
    if monitor_proprio {
        return tela;
    }

    let largura = px(PREVIA.0).min(tela.size.width);
    let altura = px(PREVIA.1).min(tela.size.height);
    Bounds {
        origin: point(
            tela.origin.x + (tela.size.width - largura) / 2.,
            tela.origin.y + (tela.size.height - altura) / 2.,
        ),
        size: size(largura, altura),
    }
}

/// O menor tamanho que uma janela lembrada pode ter ao voltar. Abaixo disso a
/// lembrança é descartada: uma tela do cliente de 40 px não é escolha, é
/// resto de um arrasto que deu errado.
const MENOR_JANELA: (f32, f32) = (320., 200.);

/// A altura da barra que a tela do cliente desenha em janela no GNOME.
const ALTURA_DA_BARRA: f32 = 32.;

/// Se esta janela desenha a própria barra de título.
///
/// 🚨 **No GNOME não há barra do sistema** (dono, 22/set/2026: *"quando
/// estiver no modo janela precisa ter a barra de movimentação no GNome"*). O
/// compositor não implementa `xdg-decoration`, e o `titlebar` do
/// `WindowOptions` é ignorado — a janela nascia sem por onde pegar, e o modo
/// janela, que existe justamente para arrastar a tela até o monitor certo,
/// não arrastava. `crate::janela` explica o porquê inteiro.
///
/// No macOS, no Windows e no KDE a barra do sistema existe, e uma segunda
/// seria duplicata — quem decide é [`crate::janela::app_desenha_a_barra`].
/// 🚨 **Em tela cheia a barra fica** (dono, 25/set/2026: *"mesmo que estiver
/// com tela cheia precisa ter o minimizar, restaurar e fechar, sempre precisa
/// ter como arrastar essa tela para outro monitor, assim como é feito no
/// Darktable"*). Até aqui ela sumia, e em tela cheia **nenhum** sistema mostra
/// a dele: a tela ficava presa no monitor, sem botão e sem por onde pegar. Por
/// isso a tela cheia liga a barra em qualquer sistema, e não só no GNOME.
fn tem_barra_propria(window: &Window) -> bool {
    window.is_fullscreen() || crate::janela::app_desenha_a_barra(window)
}

/// Como a janela estava quando saiu: é como ela volta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    Janela,
    Maximizada,
    TelaCheia,
}

/// Se o gesto de tela cheia vira tela cheia **de verdade**, ou maximizar.
///
/// 🚨 **No Mac, no monitor principal, maximiza** (dono, 17/set/2026). Tela
/// cheia no macOS é um *Space* próprio: num Mac de um monitor só ele tomava o
/// lugar do app, e o operador perdia a triagem de vista. Maximizar ali usa a
/// área visível da tela e deixa o app a um clique. Num segundo monitor o
/// *Space* é daquele monitor, e a tela cheia é a de sempre. No Linux e no
/// Windows, tela cheia é só uma janela sem moldura — vale em qualquer monitor.
pub fn tela_cheia_de_verdade(e_mac: bool, no_monitor_principal: bool) -> bool {
    !(e_mac && no_monitor_principal)
}

/// O estado com que ela abre.
///
/// - **Lembrado**, o lembrado — com a mesma troca de tela cheia por
///   maximizar que [`tela_cheia_de_verdade`] faz no Mac.
/// - **Sem lembrança**, tela cheia se ela tem um monitor só para ela (o que a
///   regra do legado sempre fez) e janela quando divide a tela com o app.
pub fn estado_ao_abrir(
    lembrado: Option<Estado>,
    monitor_proprio: bool,
    e_mac: bool,
    no_monitor_principal: bool,
) -> Estado {
    let pedido = lembrado.unwrap_or(if monitor_proprio {
        Estado::TelaCheia
    } else {
        Estado::Janela
    });
    if pedido == Estado::TelaCheia && !tela_cheia_de_verdade(e_mac, no_monitor_principal) {
        Estado::Maximizada
    } else {
        pedido
    }
}

/// O que a tela do cliente lembra entre uma abertura e outra.
///
/// 🔑 **O monitor e a janela de onde ela saiu** (dono, 22/set/2026: *"precisa
/// memorizar a última posição e monitor que estava antes de ser fechado"*).
/// Quem arrastou a tela até o monitor virado para o cliente não quer
/// arrastá-la de novo a cada abertura.
///
/// O monitor é guardado pelo `uuid` do sistema, e não pelo `DisplayId`: este é
/// só a posição na lista, e muda quando um monitor é plugado ou tirado.
///
/// ⚠️ O arquivo de antes guardava só `em_janela`: o campo é ignorado, e a
/// primeira abertura depois da troca segue a regra sem lembrança.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lembranca {
    /// Janela, maximizada ou tela cheia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<Estado>,
    /// O `uuid` do monitor onde ela estava.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<String>,
    /// Posição e tamanho no modo janela: `[x, y, largura, altura]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub janela: Option<[f32; 4]>,
    /// A tela mostra a grade (várias fotos numeradas), e não a foto em foco.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grade: bool,
    /// Quantas fotos por linha na grade. `None` é o padrão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub por_linha: Option<usize>,
}

impl Lembranca {
    /// ⚠️ Arquivo ausente ou ilegível é a lembrança vazia: o padrão de sempre.
    pub fn ler(arquivo: &std::path::Path) -> Self {
        std::fs::read_to_string(arquivo)
            .ok()
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default()
    }

    /// ⚠️ Falha de gravação não é fim de fluxo: vale nesta abertura, e a
    /// próxima começa no padrão.
    pub fn gravar(&self, arquivo: &std::path::Path) {
        if let Some(pai) = arquivo.parent() {
            let _ = std::fs::create_dir_all(pai);
        }
        if let Ok(texto) = serde_json::to_string(self) {
            let _ = std::fs::write(arquivo, texto);
        }
    }

    pub fn limites(&self) -> Option<Bounds<Pixels>> {
        let [x, y, largura, altura] = self.janela?;
        (largura >= MENOR_JANELA.0 && altura >= MENOR_JANELA.1).then(|| Bounds {
            origin: point(px(x), px(y)),
            size: size(px(largura), px(altura)),
        })
    }

    /// As fotos por linha da grade, dentro da faixa — arquivo mexido à mão
    /// não põe 40 por linha.
    pub fn fotos_por_linha(&self) -> usize {
        conta::por_linha_valido(self.por_linha.unwrap_or(conta::POR_LINHA_PADRAO))
    }

    pub fn lembrar_limites(&mut self, limites: Bounds<Pixels>) {
        self.janela = Some([
            f32::from(limites.origin.x),
            f32::from(limites.origin.y),
            f32::from(limites.size.width),
            f32::from(limites.size.height),
        ]);
    }
}

/// O monitor lembrado, **se ele ainda estiver plugado**.
///
/// Genérica pelo mesmo motivo de [`monitor_do_cliente`]: o `DisplayId` não se
/// constrói fora do GPUI. Sem o lembrado, quem decide é a regra de sempre.
pub fn monitor_lembrado<T: Copy>(
    todos: &[(T, Option<String>)],
    lembrado: Option<&str>,
) -> Option<T> {
    let lembrado = lembrado?;
    todos
        .iter()
        .find(|(_, uuid)| uuid.as_deref() == Some(lembrado))
        .map(|(id, _)| *id)
}

/// Um monitor como a escolha da tela do cliente o mostra.
///
/// Genérico pelo mesmo motivo de [`monitor_do_cliente`]: o `DisplayId` não se
/// constrói fora do GPUI.
#[derive(Debug, Clone, PartialEq)]
pub struct Monitor<T> {
    pub id: T,
    /// O `uuid` do sistema — o que a [`Lembranca`] guarda.
    pub uuid: Option<String>,
    /// Como o sistema o chama (`DELL U2720Q`), se disser — ver
    /// [`nome_do_monitor`].
    pub nome: Option<String>,
    pub limites: Bounds<Pixels>,
}

/// Os monitores da esquerda para a direita (e, na mesma coluna, de cima para
/// baixo): "Monitor 1" é o da esquerda, como o operador os vê na mesa.
///
/// 🚨 **A ordem do GPUI não serve**: no Wayland a lista sai de um `HashMap`, e o
/// "Monitor 1" de uma abertura seria o "Monitor 2" da seguinte.
pub fn em_ordem<T>(mut todos: Vec<Monitor<T>>) -> Vec<Monitor<T>> {
    todos.sort_by(|a, b| {
        let lugar = |m: &Monitor<T>| (f32::from(m.limites.origin.x), f32::from(m.limites.origin.y));
        lugar(a)
            .partial_cmp(&lugar(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    todos
}

/// O monitor que já vem marcado na escolha: o da última vez, se ainda estiver
/// plugado; senão o primeiro que não é o do app.
///
/// 🔑 **"O do app", e não o principal**: no Wayland o GPUI não sabe qual é o
/// principal (`primary_display()` é `None`), e quem chama passa o monitor onde
/// a janela principal está.
pub fn monitor_sugerido<T: Copy + PartialEq>(
    todos: &[Monitor<T>],
    lembrado: Option<&str>,
    do_app: Option<T>,
) -> Option<T> {
    let pares: Vec<(T, Option<String>)> = todos.iter().map(|m| (m.id, m.uuid.clone())).collect();
    let ids: Vec<T> = todos.iter().map(|m| m.id).collect();
    monitor_lembrado(&pares, lembrado).or_else(|| monitor_do_cliente(&ids, do_app))
}

/// O texto do botão: `Monitor 2 · DELL U2720Q · 3840 × 2160 · onde está o app`.
///
/// O nome é o que distingue dois monitores do mesmo tamanho; sem ele (o
/// sistema não disse) o botão fica só com o número e o tamanho.
///
/// O tamanho é o que o sistema informa — em pontos, e não em pixels do
/// dispositivo: um 4K em escala 2 aparece como 1920 × 1080, que é o mesmo
/// número que as configurações de tela do sistema mostram.
pub fn rotulo<T: PartialEq>(numero: usize, monitor: &Monitor<T>, do_app: Option<T>) -> String {
    let tamanho = monitor.limites.size;
    let mut texto = format!("Monitor {numero}");
    if let Some(nome) = &monitor.nome {
        texto.push_str(&format!(" · {nome}"));
    }
    texto.push_str(&format!(
        " · {} × {}",
        f32::from(tamanho.width).round() as i32,
        f32::from(tamanho.height).round() as i32,
    ));
    if do_app.as_ref() == Some(&monitor.id) {
        texto.push_str(" · onde está o app");
    }
    texto
}

/// Com que estado a janela nasce.
///
/// 🚨 **No Wayland, com monitor próprio, é sempre tela cheia** (Fedora/GNOME,
/// 03/out/2026). O xdg-shell não deixa o app posicionar janela: `display_id` e
/// a origem dos limites são ignorados. O único pedido de lugar que o GNOME
/// atende é `set_fullscreen(monitor)` — o patch em `vendor/gpui-pre-linux` o
/// liga ao `display_id`.
///
/// ⚠️ **E ela não volta sozinha à janela.** A primeira versão nascia em tela
/// cheia e desfazia no primeiro quadro, para respeitar a lembrança: no Mutter
/// (medido com dois monitores virtuais) sair da tela cheia **devolve a janela
/// ao monitor principal**, maximizando antes ou depois. Era o defeito que o
/// dono viu: escolhia o monitor 2 e ela aparecia no do app.
pub fn ao_nascer(pedido: Estado, monitor_proprio: bool, sem_posicao: bool) -> Estado {
    if sem_posicao && monitor_proprio {
        Estado::TelaCheia
    } else {
        pedido
    }
}

/// Onde a janela arrastável nasce: onde estava, se ainda couber.
///
/// - **Sem lembrança**, é a prévia centrada de [`area_do_cliente`].
/// - **Lembrada e ainda tocando o monitor**, volta ali — empurrada para
///   dentro dele se sobrava para fora, e nunca maior que ele.
/// - **Lembrada e fora do monitor** (a resolução mudou, o monitor foi trocado
///   de lado), volta no tamanho que tinha, centrada nele. Nascer fora de
///   qualquer tela é o que deixaria a janela sem como ser alcançada.
pub fn area_da_janela(tela: Bounds<Pixels>, lembrada: Option<Bounds<Pixels>>) -> Bounds<Pixels> {
    let Some(lembrada) = lembrada else {
        return area_do_cliente(tela, false);
    };
    let largura = lembrada.size.width.min(tela.size.width);
    let altura = lembrada.size.height.min(tela.size.height);
    let origem = if lembrada.intersects(&tela) {
        point(
            lembrada
                .origin
                .x
                .max(tela.origin.x)
                .min(tela.origin.x + tela.size.width - largura),
            lembrada
                .origin
                .y
                .max(tela.origin.y)
                .min(tela.origin.y + tela.size.height - altura),
        )
    } else {
        point(
            tela.origin.x + (tela.size.width - largura) / 2.,
            tela.origin.y + (tela.size.height - altura) / 2.,
        )
    };
    Bounds {
        origin: origem,
        size: size(largura, altura),
    }
}

pub struct Cliente {
    foto: Option<PhotoViewModel>,
    imagem: Option<Arc<RenderImage>>,
    /// A foto que está **saindo** — ela some enquanto a nova aparece.
    ///
    /// 🔑 **Ela fica montada com opacidade zero depois do cruzamento**, e não é
    /// limpa por um temporizador. Custa uma textura, nunca duas: a próxima
    /// troca a substitui pela que estiver saindo então. Um agendamento só para
    /// devolver essa textura seria mais código do que o que ele economiza.
    saindo: Option<Arc<RenderImage>>,
    /// Quantas trocas já houve.
    ///
    /// 🔑 **É o que reinicia a animação.** O GPUI guarda o instante inicial por
    /// `ElementId` (`AnimationState`, em `elements/animation.rs`): com um id
    /// fixo, a segunda troca nasceria com o `delta` da primeira já em 1 e a
    /// foto apareceria de uma vez. É o mesmo papel da `chave` das camadas na
    /// web.
    troca: usize,
    /// Onde a foto está na sequência do operador, e de quantas. Ver `info`.
    posicao: Option<(usize, usize)>,
    mostrar_info: bool,
    /// 📸 O QR do bot da sessão, no canto inferior direito (dono,
    /// 30/set/2026). A raiz manda o da sessão aberta; o `I` o esconde junto
    /// com o rodapé.
    convite: Option<crate::sessoes::qr_do_bot::ConviteNaTela>,
    /// O motor desta janela, aberto na primeira foto.
    processador: Option<Processador>,
    /// O maior lado do monitor desta janela, em pixels do dispositivo — o
    /// tamanho em que a foto é revelada aqui. Medido a cada `render`.
    ///
    /// 🪟 **O monitor, e não a janela**: ela pode passar a tela cheia sem foto
    /// nova, e a foto revelada para a prévia pequena ficaria borrada no cliente.
    /// Revelar os 2560 px da cópia para um monitor de 1920 era pagar a mais, a
    /// cada passo de slider, na GPU e na subida para a janela — o que no
    /// Windows, com duas APIs gráficas, pesava (dono, 27/set/2026).
    lado_do_monitor: Option<u32>,
    /// O pedido que está no motor, e o que fazer com o resultado.
    revelando: Option<EmRevelacao>,
    /// O Comparar (`⇧C`): duas fotos lado a lado, enquanto o operador compara.
    par: Option<ParNoCliente>,
    /// Para qual metade é o pedido em `revelando` — `None` é a foto única.
    revelando_lado: Option<Lado>,
    /// As metades que esperam a vez do motor.
    ///
    /// 🚨 **Uma no motor por vez**: ele só atende o pedido mais novo, e dois
    /// seguidos perderiam o primeiro — a metade esquerda nunca chegaria.
    fila_do_par: VecDeque<(Lado, ParaRevelar)>,
    _colheita: Option<Task<()>>,
    /// A janela precisa de foco próprio para `Esc` e `I` chegarem — a mesma
    /// lição que custou dois commits na Revelação: `track_focus` rastreia o
    /// foco, não o concede.
    foco: FocusHandle,
    /// Onde a lembrança desta tela é gravada. `None` nos testes que não a
    /// conferem.
    arquivo: Option<std::path::PathBuf>,
    _limites: Option<gpui_kit::Subscription>,
    /// A grade (várias fotos, cada uma com o número embaixo), quando é ela
    /// que está no ar. `None` é a foto única ou o Comparar.
    grade: Option<GradeParaOCliente>,
    /// O cache de previews do app, de onde saem as imagens da grade. `None`
    /// nos testes que não a desenham.
    previews: Option<Arc<PreviewManager>>,
    /// As imagens da página no ar, por chave — e se é a prévia grande.
    imagens_da_grade: std::collections::HashMap<String, (bool, Arc<RenderImage>)>,
    /// As chaves que o cache não tinha, para não perguntar ao disco a cada
    /// quadro. Esvazia quando a raiz reenvia a grade ou avisa de imagem nova.
    sem_imagem: std::collections::HashSet<String>,
    /// A raiz avisou que alguma imagem mudou: a página é relida por cima do
    /// que está na tela, sem um quadro vazio no meio.
    reler_a_grade: bool,
    /// 🎛️ **A célula que mostra a revelação acontecendo** (dono, 12/out/2026):
    /// com a grade no ar e o operador na Revelação, a foto aberta aparece na
    /// célula dela como está na tela dele, a cada gesto — a chave do item e a
    /// imagem que o motor desta janela devolveu por último.
    ao_vivo: Option<(String, Arc<RenderImage>)>,
    /// A chave da célula a que pertence o pedido que está no motor.
    revelando_na_grade: Option<String>,
}

impl Cliente {
    pub fn novo(
        arquivo: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let foco = cx.focus_handle();
        window.focus(&foco, cx);
        // 🔑 Mover ou redimensionar é o que grava: fechar pelo `X`, pelo `Esc`
        // ou pelo botão do app não passa todos pelo mesmo lugar, e o último
        // lugar onde ela esteve é o último movimento.
        let limites = arquivo.is_some().then(|| {
            cx.observe_window_bounds(window, |cliente: &mut Self, window, cx| {
                cliente.lembrar(window, cx)
            })
        });

        Self {
            foto: None,
            imagem: None,
            saindo: None,
            troca: 0,
            posicao: None,
            // Nasce ligado, como no legado — quem mostra ao cliente costuma
            // querer o nome do arquivo à vista, e desligar é uma tecla.
            mostrar_info: true,
            convite: None,
            processador: None,
            lado_do_monitor: None,
            revelando: None,
            par: None,
            revelando_lado: None,
            fila_do_par: VecDeque::new(),
            _colheita: None,
            foco,
            arquivo,
            _limites: limites,
            grade: None,
            previews: None,
            imagens_da_grade: std::collections::HashMap::new(),
            sem_imagem: std::collections::HashSet::new(),
            reler_a_grade: false,
            ao_vivo: None,
            revelando_na_grade: None,
        }
    }

    /// Com o cache de previews do app — é dele que a grade tira as imagens.
    pub fn com_previews(mut self, previews: Arc<PreviewManager>) -> Self {
        self.previews = Some(previews);
        self
    }

    /// Grava o monitor onde ela está, o estado e, em janela, a posição e o
    /// tamanho — o `_darkroom_ui_second_window_write_config` do darktable.
    ///
    /// ⚠️ **Maximizada não grava os limites**: seriam os do monitor inteiro, e
    /// a próxima abertura em janela nasceria do tamanho da tela. O que vale é
    /// a janela que o operador desenhou.
    ///
    /// ⚠️ **No GNOME (Wayland) a posição não existe para o app**: o protocolo
    /// não conta onde a janela está, e o GPUI devolve a origem em zero. O
    /// monitor e o tamanho são lembrados; o lugar exato, quem decide é o
    /// compositor.
    fn lembrar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(arquivo) = &self.arquivo else {
            return;
        };
        let antes = Lembranca::ler(arquivo);
        let mut agora = antes.clone();
        if let Some(uuid) = window.display(cx).and_then(|tela| tela.uuid().ok()) {
            agora.monitor = Some(uuid.to_string());
        }
        agora.estado = Some(match window.window_bounds() {
            gpui_kit::WindowBounds::Windowed(limites) => {
                agora.lembrar_limites(limites);
                Estado::Janela
            }
            gpui_kit::WindowBounds::Maximized(_) => Estado::Maximizada,
            gpui_kit::WindowBounds::Fullscreen(_) => Estado::TelaCheia,
        });
        if agora != antes {
            agora.gravar(arquivo);
        }
    }

    /// Revela esta foto com a revelação dela e a mostra quando o motor responder.
    ///
    /// 🔑 **A foto anterior fica na tela até a nova estar pronta**, e só então
    /// cruza: nunca aparece um quadro preto nem a foto crua no meio.
    pub fn revelar(&mut self, pedido: ParaRevelar, cx: &mut Context<Self>) {
        // Uma foto de novo: o Comparar acabou.
        self.par = None;
        self.fila_do_par.clear();
        self.revelando_na_grade = None;
        self.revelar_para(pedido, None, cx);
    }

    /// Revela esta foto **para a célula dela na grade** — a de chave `chave`.
    ///
    /// 🔑 A grade continua no ar: o resultado do motor troca só a imagem
    /// daquela célula, e as outras ficam com a do cache.
    pub fn revelar_na_grade(&mut self, pedido: ParaRevelar, chave: String, cx: &mut Context<Self>) {
        self.par = None;
        self.fila_do_par.clear();
        self.revelando_na_grade = Some(chave);
        self.revelar_para(pedido, None, cx);
    }

    /// Duas fotos lado a lado — o Comparar (`⇧C`) do operador.
    ///
    /// 🔑 **Só revela a metade que mudou.** A raiz reenvia o par a cada mudança
    /// (a outra foto trocou, a escolhida mudou, uma nota): a metade com a mesma
    /// foto e a mesma revelação fica como está, e a que já estava na tela como
    /// foto única vira a esquerda sem piscar — o motor a refaz por baixo.
    pub fn comparar(
        &mut self,
        esquerda: ParaRevelar,
        direita: ParaRevelar,
        ativa: Lado,
        cx: &mut Context<Self>,
    ) {
        self.grade = None;
        let anterior = self.par.take();
        let unica = self
            .foto
            .as_ref()
            .map(|f| f.id.clone())
            .zip(self.imagem.clone());
        let em_voo = self
            .revelando
            .as_ref()
            .map(|(_, foto, _, _)| foto.id.clone())
            .zip(self.revelando_lado);
        self.fila_do_par.clear();
        let metade = |lado: Lado, pedido: ParaRevelar, fila: &mut VecDeque<(Lado, ParaRevelar)>| {
            let parametros = (pedido.ajustes, pedido.corte.clone());
            let ja = anterior.as_ref().and_then(|p| {
                [&p.esquerda, &p.direita]
                    .into_iter()
                    .find(|m| m.foto.id == pedido.foto.id)
                    .map(|m| (m.imagem.clone(), Some(m.parametros.clone())))
            });
            let ja = ja.or_else(|| {
                unica
                    .as_ref()
                    .filter(|(id, _)| *id == pedido.foto.id)
                    .map(|(_, imagem)| (Some(imagem.clone()), None))
            });
            let (imagem, parametros_da_imagem) = ja.unwrap_or((None, None));
            let revelada_assim =
                imagem.is_some() && parametros_da_imagem.as_ref() == Some(&parametros);
            let no_motor = em_voo.as_ref() == Some(&(pedido.foto.id.clone(), lado));
            let m = MetadeNoCliente {
                foto: pedido.foto.clone(),
                posicao: pedido.posicao,
                imagem,
                parametros,
            };
            if !revelada_assim && !no_motor {
                fila.push_back((lado, pedido));
            }
            m
        };
        let esquerda = metade(Lado::Esquerda, esquerda, &mut self.fila_do_par);
        let direita = metade(Lado::Direita, direita, &mut self.fila_do_par);
        self.par = Some(ParNoCliente {
            esquerda,
            direita,
            ativa,
        });
        self.soltar_a_proxima_do_par(cx);
        cx.notify();
    }

    /// Manda ao motor a próxima metade da fila, se ele estiver livre.
    fn soltar_a_proxima_do_par(&mut self, cx: &mut Context<Self>) {
        if self.revelando.is_some() {
            return;
        }
        if let Some((lado, pedido)) = self.fila_do_par.pop_front() {
            self.revelar_para(pedido, Some(lado), cx);
        }
    }

    fn revelar_para(&mut self, pedido: ParaRevelar, lado: Option<Lado>, cx: &mut Context<Self>) {
        self.revelando_lado = lado;
        let processador = self.processador.get_or_insert_with(Processador::novo);
        let id = processador.proximo_id();
        processador.pedir(Pedido {
            id,
            pixels: pedido.pixels,
            largura: pedido.largura,
            altura: pedido.altura,
            ajustes: pedido.ajustes,
            corte: transformacao::corte(&pedido.corte),
            locais: pedido.locais,
            lado_na_tela: self.lado_do_monitor,
            // Enquadrada e em BGRA pela GPU, no tamanho do monitor: a thread
            // da interface não recorta nem converte — e a Tela do cliente fica
            // aberta o expediente inteiro.
            tela: Some(ParaTela {
                recortar: true,
                inteira: false,
            }),
        });
        self.revelando = Some((id, pedido.foto, pedido.posicao, pedido.corte));
        if self._colheita.is_none() {
            self._colheita = Some(cx.spawn(async move |tela, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let Ok(continua) = tela.update(cx, |tela, cx| tela.colher(cx)) else {
                    break;
                };
                if !continua {
                    break;
                }
            }));
        }
    }

    /// Pega o resultado mais novo do motor. Devolve se ainda há o que esperar.
    fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(processador) = self.processador.as_ref() else {
            self._colheita = None;
            return false;
        };
        if let Some(resultado) = processador.colher() {
            if let Some((id, foto, posicao, corte)) = self.revelando.take() {
                if id == resultado.id {
                    let inicio = std::time::Instant::now();
                    // A pronta já vem enquadrada pela GPU; sem ela (uma GPU
                    // que recusou a passada), o caminho de sempre na CPU.
                    let imagem = match (resultado.pronta, resultado.imagem) {
                        (Some(pronta), _) => pronta.imagem,
                        (None, Some(inteira)) => {
                            let exibida = transformacao::aplicar(&inteira, &corte, true);
                            crate::imagem::para_gpui(exibida)
                        }
                        (None, None) => return true,
                    };
                    crate::depuracao::vigia::cronometrar(
                        "tela do cliente: enquadrar + converter",
                        inicio,
                    );
                    if crate::depuracao::vigia::ligado() {
                        eprintln!("[cliente] mostrou {}", foto.id);
                    }
                    match self.revelando_lado.take() {
                        // Uma metade do Comparar: vai para ela, se ela ainda
                        // for desta foto, e a próxima da fila entra no motor.
                        Some(lado) => {
                            if let Some(par) = self.par.as_mut() {
                                let metade = match lado {
                                    Lado::Esquerda => &mut par.esquerda,
                                    Lado::Direita => &mut par.direita,
                                };
                                if metade.foto.id == foto.id {
                                    metade.imagem = Some(imagem);
                                }
                            }
                            self.soltar_a_proxima_do_par(cx);
                            cx.notify();
                        }
                        // 🔑 A grade só sai quando a foto está pronta: até
                        // lá ela fica no ar, e não a foto de antes dela.
                        None => match self.revelando_na_grade.take() {
                            // 🎛️ Para a célula da grade: ela fica no ar, e
                            // só aquela foto muda.
                            Some(chave) if self.grade.is_some() => {
                                self.ao_vivo = Some((chave, imagem));
                                cx.notify();
                            }
                            _ => {
                                self.grade = None;
                                self.ao_vivo = None;
                                self.mostrar(Some(foto), Some(imagem), posicao, cx)
                            }
                        },
                    }
                } else {
                    if crate::depuracao::vigia::ligado() {
                        eprintln!("[cliente] resultado velho {} (espera {id})", resultado.id);
                    }
                    // Chegou um resultado velho: o pedido novo continua na fila.
                    self.revelando = Some((id, foto, posicao, corte));
                }
            }
        }
        if self.revelando.is_none() {
            self._colheita = None;
            return false;
        }
        true
    }

    /// Troca a foto que está na tela — **uma atravessando a outra**.
    ///
    /// ⚠️ **Sem imagem, a foto some da segunda tela.** Chega assim quando a foto
    /// não tem preview no cache, e mostrar o nome sobre o preto seria pior do que
    /// mostrar o preto: o cliente veria uma tela escura com um nome de arquivo.
    ///
    /// ⚠️ **Continuar na mesma foto troca os pixels sem cruzamento nenhum.** A
    /// raiz chama isto em situações que não são troca de foto — o operador
    /// acabou de revelar e a imagem é outra, a nota mudou —, e cruzar a foto
    /// com ela mesma faria a tela piscar na cara do cliente. O conteúdo
    /// atualiza no lugar; a animação é só para quando a foto muda.
    ///
    /// 🚨 **Quem decide é o id da foto, nunca o `Arc` da imagem.**
    /// `imagem::para_gpui` decodifica e **embrulha num `Arc` novo a cada
    /// chamada**: um `Arc::ptr_eq` aqui seria falso sempre, a guarda não
    /// guardaria nada, e o defeito seria invisível — a tela pisca, e o código
    /// diz que não pisca.
    pub fn mostrar(
        &mut self,
        foto: Option<PhotoViewModel>,
        imagem: Option<Arc<RenderImage>>,
        posicao: Option<(usize, usize)>,
        cx: &mut Context<Self>,
    ) {
        self.posicao = posicao;
        if self.e_a_mesma_foto(foto.as_ref()) {
            self.foto = foto;
            self.imagem = imagem;
            cx.notify();
            return;
        }
        self.saindo = self.imagem.take();
        self.troca = self.troca.wrapping_add(1);
        self.foto = foto;
        self.imagem = imagem;
        cx.notify();
    }

    /// Continuamos na mesma foto? (Sem foto dos dois lados também conta.)
    fn e_a_mesma_foto(&self, nova: Option<&PhotoViewModel>) -> bool {
        match (self.foto.as_ref(), nova) {
            (Some(atual), Some(nova)) => atual.id == nova.id,
            (None, None) => true,
            _ => false,
        }
    }

    /// O nome da foto que está na tela — ou que o motor está revelando para
    /// entrar nela. É para a raiz poder afirmar sobre ela.
    pub fn foto_mostrada(&self) -> Option<String> {
        self.revelando
            .as_ref()
            .map(|(_, foto, _, _)| foto)
            .or(self.foto.as_ref())
            .map(|foto| foto.name.clone())
    }

    /// Os nomes das duas fotos do Comparar, e se as duas já têm imagem.
    pub fn fotos_comparadas(&self) -> Option<(String, String, bool)> {
        let par = self.par.as_ref()?;
        Some((
            par.esquerda.foto.name.clone(),
            par.direita.foto.name.clone(),
            par.esquerda.imagem.is_some() && par.direita.imagem.is_some(),
        ))
    }

    pub fn mostrando_info(&self) -> bool {
        self.mostrar_info
    }

    /// 📸 O QR da sessão — `None` tira.
    pub fn definir_convite(
        &mut self,
        convite: Option<crate::sessoes::qr_do_bot::ConviteNaTela>,
        cx: &mut Context<Self>,
    ) {
        if self.convite != convite {
            self.convite = convite;
            cx.notify();
        }
    }

    pub fn convite(&self) -> Option<&crate::sessoes::qr_do_bot::ConviteNaTela> {
        self.convite.as_ref()
    }

    /// 📸 O QR para o cliente ler com a câmera — grande, legível do outro lado
    /// do balcão. Depois da primeira resposta vira a confirmação de que deu
    /// certo e o resto é no celular. O mesmo desenho da tela do cliente do site.
    fn qr_na_tela(&self) -> Option<gpui_kit::AnyElement> {
        Some(
            self.cartao_do_qr()?
                .absolute()
                .right(px(24.))
                .bottom(px(24.))
                .into_any_element(),
        )
    }

    /// O cartão do QR, sem lugar: por cima do palco (foto única e Comparar) ou
    /// na coluna dele (grade).
    fn cartao_do_qr(&self) -> Option<gpui_kit::Div> {
        use crate::sessoes::qr_do_bot::EstadoDoBot;
        // 🔑 **Só o balconista tira o QR daqui** (o botão "QR do bot" e o `I`
        // da sessão): nem o `I` desta janela, que é do rodapé, nem o cadastro
        // concluído — outra pessoa da mesma sessão ainda pode querer ler.
        let convite = self.convite.as_ref()?;
        // 📐 Cada texto vai **na largura do cartão** (`w_full`), e não na dele:
        // numa coluna centralizada o GPUI dá ao texto a largura do conteúdo, e
        // a frase não quebrava — vazava pelos dois lados do cartão (dono,
        // 30/set/2026, com a foto da tela do cliente).
        let andamento = (convite.estado != EstadoDoBot::Aguardando).then(|| {
            div()
                .w_full()
                .flex()
                .justify_center()
                .items_start()
                .gap(px(6.))
                .child(
                    gpui_kit::component::Icon::new(crate::recursos::Icone::CircleCheck)
                        .size(px(14.))
                        .text_color(gpui_kit::rgb(0x34d399))
                        .flex_none()
                        .mt(px(1.)),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_xs()
                        .text_color(gpui_kit::rgb(0xd4d4d4))
                        .child(match &convite.quem {
                            Some(quem) => format!("{quem} — conversa iniciada"),
                            None => "Conversa iniciada".to_string(),
                        }),
                )
        });
        let corpo = div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .p(px(8.))
                    .bg(gpui_kit::white())
                    .rounded(crate::tema::canto(8.))
                    .child(crate::sessoes::qr_do_bot::desenho_do_qr(
                        convite.qr.clone(),
                        208.,
                    )),
            )
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .text_color(gpui_kit::rgb(0xf5f5f5))
                    .child("Aponte a câmera do celular para receber suas fotos"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(gpui_kit::rgb(0x9a9a9a))
                    .child(convite.codigo.clone()),
            )
            .children(andamento);
        Some(
            div()
                .w(px(256.))
                .p(px(16.))
                .rounded(crate::tema::canto(12.))
                .bg(gpui_kit::rgba(0x000000c0))
                .flex()
                .flex_col()
                .items_center()
                .child(corpo),
        )
    }

    pub fn alternar_info(&mut self, cx: &mut Context<Self>) {
        self.mostrar_info = !self.mostrar_info;
        cx.notify();
    }

    /// `Esc` fecha esta janela — e só esta.
    ///
    /// 🔑 **É a janela que se remove**, e não um recado para a principal ler
    /// depois. No legado o `Esc` do viewport grava `secondary_window_close_req`
    /// na memória global do egui, e a janela principal fecha a segunda no quadro
    /// seguinte: um caminho a mais para o mesmo fim, que existe só porque o
    /// closure do viewport não alcança o estado do app.
    fn ao_fechar(&mut self, _acao: &FecharCliente, window: &mut Window, _cx: &mut Context<Self>) {
        window.remove_window();
    }

    fn ao_alternar_info(
        &mut self,
        _acao: &AlternarInfoDoCliente,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.alternar_info(cx);
    }

    /// `F`: tela cheia neste monitor, ou de volta à janela.
    ///
    /// 🔑 **É a mesma janela**, e não uma nova: o sistema a põe em tela cheia
    /// no monitor onde ela está. Até 22/set/2026 o `J` fechava e reabria em
    /// outro modo, com o monitor escolhido pelo app — e no GNOME o app não
    /// escolhe monitor nenhum, então a tela nascia onde o compositor queria e,
    /// sem barra, não saía de lá.
    pub fn alternar_tela_cheia(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
            return;
        }
        let principal = cx.primary_display().map(|tela| tela.id());
        let aqui = window.display(cx).map(|tela| tela.id());
        if tela_cheia_de_verdade(cfg!(target_os = "macos"), aqui == principal) {
            window.toggle_fullscreen();
        } else {
            window.zoom_window();
        }
    }

    fn ao_alternar_tela_cheia(
        &mut self,
        _acao: &AlternarTelaCheiaDoCliente,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.alternar_tela_cheia(window, cx);
    }

    /// O que o rodapé conta: onde estamos, o nome, a nota e se o cliente já
    /// levou esta foto.
    ///
    /// 🔑 **Os três dados respondem a perguntas que o cliente faz em voz alta**
    /// — *"em qual estamos?"*, *"esta é das boas?"*, *"essa eu já levo?"* —, e
    /// entraram na web em 2026-09-11. Sem eles, quem está do outro lado
    /// acompanha o operador no escuro e pergunta a cada foto.
    ///
    /// ⚠️ **As cinco estrelas sempre aparecem**, as vazias em cinza: é o que
    /// deixa a nota ser lida de longe, sem contar o que não está lá. O legado
    /// mostrava só as cheias, e três estrelas a dois metros viram "algumas".
    fn info(&self) -> Option<InfoDoRodape> {
        let foto = self.foto.as_ref()?;
        let nota = foto.rating.clamp(0, 5) as usize;
        Some(InfoDoRodape {
            nome: foto.name.clone().into(),
            cheias: "★".repeat(nota).into(),
            vazias: "★".repeat(5 - nota).into(),
            posicao: self
                .posicao
                .map(|(i, total)| format!("{i} / {total}").into()),
            // `comprada` no view model é **levada no balcão** (ver o campo): é a
            // foto que o cliente já disse que leva.
            escolhida: foto.comprada,
        })
    }
}

/// O tamanho da camada que entra, como fração da janela, ao longo do cruzamento.
///
/// 🚨 **Ela vai de 0,97 a 1 — nunca acima** (dono, 17/set/2026: *"a foto tá
/// ficando cortada"*). Era de 1,03 a 1, e o zoom começando **acima** da moldura
/// deixava a camada maior que a janela durante o cruzamento inteiro: o
/// `ObjectFit::Contain` de dentro fazia a foto caber na camada, e a camada não
/// cabia na tela. O movimento percebido é o mesmo vindo de baixo, e assim a foto
/// nunca passa da tela.
fn escala_de_entrada(delta: f32) -> f32 {
    0.97 + 0.03 * delta
}

/// O que o rodapé da segunda tela mostra.
struct InfoDoRodape {
    nome: SharedString,
    cheias: SharedString,
    vazias: SharedString,
    posicao: Option<SharedString>,
    escolhida: bool,
}

impl Cliente {
    /// Uma foto no ar — a que entra ou a que sai.
    ///
    /// As duas ocupam a janela inteira e se sobrepõem, que é o que permite o
    /// cruzamento. `ObjectFit::Contain` é o padrão do `img`: a foto cabe
    /// inteira, sem corte e sem esticar. Numa apresentação, cobrir a tela
    /// cortaria justamente o enquadramento que se está mostrando.
    ///
    /// 🚨 **`ObjectFit::Contain` sozinho não contém nada aqui** (dono,
    /// 17/set/2026: *"a tela do cliente tá cortando a foto"*). O `Img` do GPUI
    /// escreve `style.aspect_ratio` com a proporção da imagem em **todo**
    /// `request_layout` (`elements/img.rs`); com `size_full()`, o taffy tira a
    /// altura da largura e o elemento fica **maior que a janela** — uma foto em
    /// pé de 747×1370 numa janela de 1920×1080 recebia 1920×3522, e o `Contain`
    /// preenchia esse retângulo direitinho, para fora da tela. O que se via era
    /// a foto ampliada 2,6× e cortada, que é o oposto do que `Contain`
    /// promete.
    ///
    /// A conta agora é [`crate::imagem::cabe_em`]: a mesma da tela do cliente da
    /// web (`tela-do-cliente-web`, `escala = min(jw/sw, jh/sh) * zoom`). O
    /// elemento nasce **do tamanho da foto**, e nada precisa ser contido.
    ///
    /// ⚠️ **A que sai continua desenhada com opacidade zero** depois do
    /// cruzamento, e é de propósito: tirá-la no mesmo quadro em que a de cima
    /// chega ao fim deixaria uma tarja preta aparecer nas laterais, porque as
    /// duas fotos raramente têm a mesma proporção.
    fn camada(
        id: &'static str,
        troca: usize,
        imagem: Arc<RenderImage>,
        saindo: bool,
        janela: Size<Pixels>,
    ) -> impl IntoElement {
        let base = crate::imagem::cabe_em(janela, imagem.size(0));
        div()
            .absolute()
            .size_full()
            // A foto fica no meio da tela; quem a dimensiona é `contain`.
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(base.width)
                    .h(base.height)
                    // `size_full` **dentro de uma moldura da proporção da
                    // foto**: aqui a altura que o `aspect_ratio` do `Img`
                    // calcula é a que a moldura já tem, e não sobra nada para
                    // fora.
                    .child(img(imagem).size_full())
                    .with_animation(
                        (id, troca),
                        Animation::new(CRUZAMENTO).with_easing(ease_in_out),
                        move |foto, delta| {
                            // ✨ O passo à frente do site: a que entra cresce até o
                            // tamanho da moldura; a que sai fica onde está e some.
                            let escala = if saindo {
                                1.0
                            } else {
                                escala_de_entrada(delta)
                            };
                            foto.w(base.width * escala)
                                .h(base.height * escala)
                                .opacity(if saindo { 1.0 - delta } else { delta })
                        },
                    ),
            )
    }
}

impl Render for Cliente {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _t = crate::regua::trecho("tela do cliente: render");
        self.lado_do_monitor = window.display(cx).map(|monitor| {
            let tamanho = monitor.bounds().size;
            let lado = f32::from(tamanho.width).max(f32::from(tamanho.height));
            (lado * window.scale_factor().max(1.0)).ceil() as u32
        });
        let info = self.mostrar_info.then(|| self.info()).flatten();
        let barra = tem_barra_propria(window);
        let mut janela = window.viewport_size();
        if barra {
            janela.height = (janela.height - px(ALTURA_DA_BARRA)).max(px(0.));
        }

        if self.grade.is_some() {
            if self.preparar_a_grade(janela, window.scale_factor()) {
                window.request_animation_frame();
            }
            let grade = self.palco_da_grade(janela, cx);
            return self.moldura(grade, barra, false, window, cx);
        }

        if let Some(par) = self.par.as_ref() {
            let comparacao = Self::palco_do_comparar(par, self.mostrar_info, janela);
            return self.moldura(comparacao.into_any_element(), barra, true, window, cx);
        }

        let palco =
            div()
                .id("palco-do-cliente")
                // Dois cliques na foto: tela cheia e volta, como o `F`.
                .on_double_click(
                    cx.listener(|cliente, _, window, cx| cliente.alternar_tela_cheia(window, cx)),
                )
                .relative()
                .flex_1()
                .w_full()
                // 🔑 **Preto, e não o fundo do tema.** A segunda tela é onde a cor da
                // foto é julgada por quem paga por ela; qualquer entorno claro muda
                // como a foto é percebida. É a mesma razão de o tema do app inteiro
                // não poder acompanhar o claro/escuro do sistema.
                .bg(gpui_kit::black())
                .flex()
                .items_center()
                .justify_center()
                // ✨ **Uma foto atravessa a outra, e nunca há um quadro sem foto**
                // (pedido do dono, 2026-09-09, na tela do cliente da web; aqui pela
                // regra de paridade entre as duas plataformas). A que sai some
                // enquanto a que entra aparece, no mesmo meio segundo.
                //
                // O passo à frente da web (a escala de 1,03 a 1) sai pelo tamanho
                // da camada: o GPUI 0.2.2 não tem escala em `div`/`img`.
                .children(
                    self.saindo.clone().map(|saindo| {
                        Self::camada("cliente-saindo", self.troca, saindo, true, janela)
                    }),
                )
                .children(self.imagem.clone().map(|imagem| {
                    Self::camada("cliente-entrando", self.troca, imagem, false, janela)
                }))
                .children(info.map(|info| {
                    div()
                        .absolute()
                        .left(px(20.))
                        .bottom(px(20.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(crate::tema::canto(4.))
                        .bg(gpui_kit::rgba(0x000000b4))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(8.))
                                .children(info.posicao.map(|posicao| {
                                    div()
                                        .text_sm()
                                        .text_color(gpui_kit::rgb(0x9a9a9a))
                                        .child(posicao)
                                }))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(gpui_kit::white())
                                        .child(info.nome),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(gpui_kit::rgb(0xfbbf24))
                                        .child(info.cheias),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(gpui_kit::rgb(0x4a4a4a))
                                        .child(info.vazias),
                                )
                                .when(info.escolhida, |linha| {
                                    linha.child(
                                        div()
                                            .px(px(6.))
                                            .rounded(px(999.))
                                            .bg(gpui_kit::rgb(0xfbbf24))
                                            .text_xs()
                                            .text_color(gpui_kit::black())
                                            .child("Escolhida"),
                                    )
                                }),
                        )
                }))
                .child(
                    // As instruções, como no legado. Elas **não** dependem do `I`:
                    // desligar o rodapé e perder junto a única pista de como fechar
                    // a janela deixaria uma tela preta sem saída visível, num monitor
                    // que muitas vezes está de costas para quem a abriu.
                    div()
                        .absolute()
                        .right(px(20.))
                        .top(px(20.))
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(crate::tema::canto(4.))
                        .bg(gpui_kit::rgba(0x00000078))
                        .text_xs()
                        .text_color(gpui_kit::rgb(0xb4b4b4))
                        // 🔑 **O `F` aparece aqui porque é a saída de um problema
                        // que não se vê**: a tela no monitor errado. Em janela,
                        // arrasta-se até o certo e aperta-se `F` lá.
                        .child(SharedString::from(if window.is_fullscreen() {
                            format!(
                                "Esc fecha • I mostra o nome • F ou {} sai da tela cheia",
                                texto_da_tela_cheia()
                            )
                        } else {
                            format!(
                                "Esc fecha • I mostra o nome • F ou {} tela cheia neste monitor",
                                texto_da_tela_cheia()
                            )
                        })),
                );

        self.moldura(palco.into_any_element(), barra, true, window, cx)
    }
}

impl Cliente {
    /// A janela em volta do palco: o foco, as teclas e a barra própria.
    fn moldura(
        &self,
        palco: gpui_kit::AnyElement,
        barra: bool,
        qr_por_cima: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        div()
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(Self::ao_alternar_info))
            .on_action(cx.listener(Self::ao_alternar_tela_cheia))
            .on_action(cx.listener(Self::ao_fechar))
            .size_full()
            .bg(gpui_kit::black())
            .flex()
            .flex_col()
            .when(barra, |coluna| {
                coluna.child(
                    crate::janela::como_barra_de_titulo(div(), "barra-do-cliente", window, cx)
                        .flex_none()
                        .h(px(ALTURA_DA_BARRA))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pl(px(12.))
                        .pr(px(4.))
                        .bg(gpui_kit::rgb(0x161616))
                        .text_xs()
                        .text_color(gpui_kit::rgb(0xb4b4b4))
                        .child("Tela do cliente")
                        .child(crate::janela::controles_mesmo_em_tela_cheia(
                            "janela-cliente",
                            gpui_kit::rgb(0xb4b4b4).into(),
                            window,
                            cx,
                        )),
                )
            })
            .child(
                // 📸 O QR fica sobre o palco, no canto — nos dois modos (uma
                // foto e o Comparar).
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .child(palco)
                    // 🚨 Na grade o QR tem coluna própria, dentro do palco:
                    // por cima, ele taparia fotos (dono, 11/out/2026).
                    .children(qr_por_cima.then(|| self.qr_na_tela()).flatten()),
            )
            .into_any_element()
    }

    /// As duas fotos do Comparar, uma de cada lado, sobre preto.
    ///
    /// 🔑 **A borda âmbar é a mesma da tela do operador**: o cliente sabe de
    /// qual das duas se está falando. As fotos não trocam de lado.
    fn palco_do_comparar(par: &ParNoCliente, info: bool, janela: Size<Pixels>) -> impl IntoElement {
        const VAO: f32 = 8.;
        let meia = size(
            ((janela.width - px(VAO * 3.)) / 2.).max(px(1.)),
            (janela.height - px(VAO * 2.)).max(px(1.)),
        );
        let metade = |metade: &MetadeNoCliente, ativa: bool| {
            let nota = metade.foto.rating.clamp(0, 5) as usize;
            div()
                .relative()
                .flex_1()
                .h_full()
                .min_w(px(0.))
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .border_4()
                .rounded(crate::tema::canto(4.))
                .border_color(if ativa {
                    gpui_kit::rgb(0xfbbf24).into()
                } else {
                    gpui_kit::transparent_black()
                })
                .children(metade.imagem.clone().map(|imagem| {
                    let base = crate::imagem::cabe_em(meia, imagem.size(0));
                    div()
                        .w(base.width)
                        .h(base.height)
                        .child(img(imagem).size_full())
                }))
                .when(info, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(12.))
                            .bottom(px(12.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(crate::tema::canto(4.))
                            .bg(gpui_kit::rgba(0x000000b4))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_sm()
                            .children(metade.posicao.map(|(i, total)| {
                                div()
                                    .text_color(gpui_kit::rgb(0x9a9a9a))
                                    .child(SharedString::from(format!("{i} / {total}")))
                            }))
                            .child(
                                div()
                                    .text_color(gpui_kit::white())
                                    .child(SharedString::from(metade.foto.name.clone())),
                            )
                            .child(
                                div()
                                    .text_color(gpui_kit::rgb(0xfbbf24))
                                    .child(SharedString::from("★".repeat(nota))),
                            )
                            .when(metade.foto.comprada, |linha| {
                                linha.child(
                                    div()
                                        .px(px(6.))
                                        .rounded(px(999.))
                                        .bg(gpui_kit::rgb(0xfbbf24))
                                        .text_xs()
                                        .text_color(gpui_kit::black())
                                        .child("Escolhida"),
                                )
                            }),
                    )
                })
        };
        div()
            .id("palco-do-cliente")
            .flex_1()
            .w_full()
            .bg(gpui_kit::black())
            .flex()
            .gap(px(VAO))
            .p(px(VAO))
            .child(metade(&par.esquerda, par.ativa == Lado::Esquerda))
            .child(metade(&par.direita, par.ativa == Lado::Direita))
    }
}

/// O que a grade desenha agora: o arranjo, a página e os itens dela.
pub struct DesenhoDaGrade {
    pub arranjo: conta::Arranjo,
    /// A página no ar, contando de zero, e quantas há.
    pub pagina: usize,
    pub paginas: usize,
    /// Os índices, em `GradeParaOCliente::itens`, das fotos desta página.
    pub faixa: std::ops::Range<usize>,
}

impl Cliente {
    /// Põe a grade no ar: várias fotos por página, cada uma com o número dela.
    ///
    /// 🔑 **A raiz reenvia a cada mudança** (o foco andou, uma foto foi levada
    /// ou rejeitada, o recorte mudou, as fotos por linha mudaram): aqui só se
    /// guarda e se desenha. As imagens já lidas ficam — a chave é a mesma.
    pub fn mostrar_grade(&mut self, grade: GradeParaOCliente, cx: &mut Context<Self>) {
        self.mostrar_grade_com(grade, None, cx)
    }

    /// A grade com uma célula ao vivo — a de chave `ao_vivo`, a foto que o
    /// operador está revelando. Sem ela (a galeria), toda célula vem do cache.
    ///
    /// A imagem ao vivo que já está na tela fica enquanto a célula for a
    /// mesma: a grade é reenviada a cada mudança, e a foto não pode piscar.
    pub fn mostrar_grade_com(
        &mut self,
        grade: GradeParaOCliente,
        ao_vivo: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if self.ao_vivo.as_ref().map(|(chave, _)| chave.as_str()) != ao_vivo {
            self.ao_vivo = None;
        }
        if self.revelando_na_grade.as_deref() != ao_vivo {
            self.revelando_na_grade = None;
        }
        self.par = None;
        self.fila_do_par.clear();
        // Uma chave que o cache não tinha pode ter chegado desde então.
        self.sem_imagem.clear();
        self.grade = Some(grade);
        cx.notify();
    }

    /// A chave da célula que está (ou vai ficar) ao vivo — só os testes.
    #[cfg(test)]
    pub fn celula_ao_vivo(&self) -> Option<&str> {
        self.ao_vivo
            .as_ref()
            .map(|(chave, _)| chave.as_str())
            .or(self.revelando_na_grade.as_deref())
    }

    /// O motor já devolveu a imagem da célula ao vivo? — só os testes.
    #[cfg(test)]
    pub fn imagem_ao_vivo_chegou(&self) -> bool {
        self.ao_vivo.is_some()
    }

    /// A grade que está no ar, se é ela.
    pub fn grade_mostrada(&self) -> Option<&GradeParaOCliente> {
        self.grade.as_ref()
    }

    /// Alguma imagem do cache mudou (a miniatura chegou, a revelada foi
    /// regravada): a página é relida no próximo quadro.
    pub fn reler_as_imagens_da_grade(&mut self, cx: &mut Context<Self>) {
        if self.grade.is_some() {
            self.reler_a_grade = true;
            self.sem_imagem.clear();
            cx.notify();
        }
    }

    /// A conta da grade para este tamanho de palco.
    ///
    /// 🚨 **O QR ligado reserva a coluna dele** — é a conta que garante que
    /// nenhuma célula fica debaixo do cartão.
    pub fn desenho_da_grade(&self, palco: Size<Pixels>) -> Option<DesenhoDaGrade> {
        let grade = self.grade.as_ref()?;
        let arranjo = conta::arranjo(
            f32::from(palco.width),
            (f32::from(palco.height) - PE_DA_GRADE).max(0.),
            grade.por_linha,
            self.convite.is_some(),
            1.0,
        );
        let por_pagina = arranjo.por_pagina();
        let pagina = grade
            .foco
            .map_or(0, |foco| conta::pagina_de(foco, por_pagina));
        let paginas = conta::paginas(grade.itens.len(), por_pagina);
        // O foco além do fim (a lista encolheu) cai na última página.
        let pagina = pagina.min(paginas - 1);
        Some(DesenhoDaGrade {
            arranjo,
            pagina,
            paginas,
            faixa: conta::faixa_da_pagina(pagina, por_pagina, grade.itens.len()),
        })
    }

    /// Lê do cache as imagens da página no ar. Devolve se ficou alguma prévia
    /// grande para o próximo quadro.
    ///
    /// 🔑 **Antes do `render`, e não dentro da célula** — a lição da grade da
    /// sessão: decodificar por célula, por quadro, é o que trava a tela.
    fn preparar_a_grade(&mut self, palco: Size<Pixels>, escala: f32) -> bool {
        let Some(desenho) = self.desenho_da_grade(palco) else {
            return false;
        };
        let (Some(grade), Some(previews)) = (self.grade.as_ref(), self.previews.clone()) else {
            return false;
        };
        let a = desenho.arranjo;
        let quer_grande =
            a.largura_da_celula.max(a.altura_da_foto) * escala.max(1.0) > LADO_DA_MINIATURA;
        let reler = std::mem::take(&mut self.reler_a_grade);
        let chaves: Vec<String> = grade.itens[desenho.faixa.clone()]
            .iter()
            .map(|item| item.chave.clone())
            .collect();
        let mut grandes = 0;
        let mut faltou = false;
        for chave in &chaves {
            let tem = self.imagens_da_grade.get(chave).map(|(grande, _)| *grande);
            if !reler && (tem == Some(quer_grande) || tem == Some(true)) {
                continue;
            }
            if self.sem_imagem.contains(chave) {
                continue;
            }
            let pode_grande = quer_grande && (reler || grandes < GRANDES_POR_QUADRO);
            let lida = if pode_grande {
                grandes += 1;
                previews
                    .get_preview(chave)
                    .map(|imagem| (true, imagem))
                    .or_else(|| previews.get_thumbnail(chave).map(|i| (true, i)))
            } else if tem.is_some() && !reler {
                // Já tem a miniatura; a grande fica para o próximo quadro.
                faltou = true;
                continue;
            } else {
                faltou |= quer_grande;
                previews
                    .get_thumbnail(chave)
                    .map(|imagem| (false, imagem))
                    .or_else(|| previews.get_preview(chave).map(|i| (true, i)))
            };
            match lida {
                Some((grande, imagem)) => {
                    self.imagens_da_grade
                        .insert(chave.clone(), (grande, crate::imagem::para_gpui(imagem)));
                }
                None => {
                    // Sem nada no cache: se havia imagem, ela fica.
                    self.sem_imagem.insert(chave.clone());
                }
            }
        }
        // Só a página no ar fica na memória.
        self.imagens_da_grade
            .retain(|chave, _| chaves.iter().any(|c| c == chave));
        faltou
    }

    /// Uma célula da grade: a foto inteira em cima, o número embaixo.
    fn celula_da_grade(
        &self,
        item: &ItemDaGrade,
        i: usize,
        em_foco: bool,
        a: &conta::Arranjo,
    ) -> gpui_kit::AnyElement {
        let lugar = a.celula(i);
        let ambar = gpui_kit::rgb(0xfbbf24);
        let caixa = size(
            px((lugar.w - 2. * BORDA_DA_GRADE).max(1.)),
            px((a.altura_da_foto - 2. * BORDA_DA_GRADE).max(1.)),
        );
        // 🎛️ A célula ao vivo mostra o que o motor devolveu; as outras, o cache.
        let imagem = self
            .ao_vivo
            .as_ref()
            .filter(|(chave, _)| *chave == item.chave)
            .map(|(_, imagem)| imagem.clone())
            .or_else(|| {
                self.imagens_da_grade
                    .get(&item.chave)
                    .map(|(_, i)| i.clone())
            });
        // 🔑 A moldura nasce do tamanho da foto ([`crate::imagem::cabe_em`]):
        // `size_full` numa caixa de outra proporção corta a foto — a armadilha
        // do `Img` que já cortou esta tela (17/set/2026).
        let base = match &imagem {
            Some(imagem) => crate::imagem::cabe_em(caixa, imagem.size(0)),
            // Ainda sem imagem: o lugar dela, para a página não dançar.
            None => caixa,
        };
        let selo = (a.letra_do_numero * 0.5).clamp(11., 22.);

        div()
            .absolute()
            .left(px(lugar.x))
            .top(px(lugar.y))
            .w(px(lugar.w))
            .h(px(lugar.h))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(a.altura_da_foto))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // A borda e o selo são **da foto**, e não da caixa: em
                        // volta da caixa, a foto em pé ganharia duas tarjas
                        // pretas dentro da moldura do foco.
                        div()
                            .relative()
                            .w(base.width + px(2. * BORDA_DA_GRADE))
                            .h(base.height + px(2. * BORDA_DA_GRADE))
                            .rounded(crate::tema::canto(6.))
                            .border(px(BORDA_DA_GRADE))
                            .border_color(if em_foco {
                                ambar.into()
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(imagem.is_none(), |quadro| {
                                quadro.bg(gpui_kit::rgb(0x141414))
                            })
                            .children(imagem.map(|imagem| img(imagem).size_full()))
                            // ⭐ A nota, no canto de cima à esquerda — o
                            // mesmo lugar das estrelas na tira do operador.
                            // Sobre um fundo escuro: a foto por baixo pode
                            // ser clara, e a estrela âmbar sumiria nela.
                            .when(item.nota > 0, |quadro| {
                                quadro.child(
                                    div()
                                        .debug_selector(|| "grade-do-cliente-nota".into())
                                        .absolute()
                                        .top(px(6.))
                                        .left(px(6.))
                                        .px(px(selo * 0.4))
                                        .rounded(px(999.))
                                        .bg(gpui_kit::black().opacity(0.55))
                                        .text_size(px(selo))
                                        .text_color(ambar)
                                        .child(SharedString::from(
                                            "★".repeat(usize::from(item.nota.min(5))),
                                        )),
                                )
                            })
                            .when(item.escolhida, |quadro| {
                                quadro.child(
                                    div()
                                        .absolute()
                                        .top(px(6.))
                                        .right(px(6.))
                                        .px(px(selo * 0.6))
                                        .rounded(px(999.))
                                        .bg(ambar)
                                        .text_size(px(selo))
                                        .text_color(gpui_kit::black())
                                        .child("Escolhida"),
                                )
                            }),
                    ),
            )
            // 🔑 A linha do número é reservada: do mesmo tamanho em toda
            // célula, com ou sem foto, e a foto nunca desce sobre ela.
            .child(
                div()
                    .w_full()
                    .h(px(a.altura_do_numero))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(a.letra_do_numero))
                    .line_height(px(a.altura_do_numero))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(if em_foco {
                        ambar
                    } else {
                        gpui_kit::rgb(0xf5f5f5)
                    })
                    .child(SharedString::from(item.numero.to_string())),
            )
            .into_any_element()
    }

    /// A grade: as células da página, o pé com a página e, se o QR está
    /// ligado, a coluna dele ao lado — nunca por cima.
    fn palco_da_grade(&self, palco: Size<Pixels>, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let (Some(grade), Some(desenho)) = (self.grade.as_ref(), self.desenho_da_grade(palco))
        else {
            return div().into_any_element();
        };
        let a = desenho.arranjo;
        let itens = &grade.itens[desenho.faixa.clone()];
        let pe = match (itens.first(), itens.last()) {
            (Some(de), Some(ate)) => format!(
                "Fotos {} a {}  ·  página {} de {}",
                de.numero,
                ate.numero,
                desenho.pagina + 1,
                desenho.paginas
            ),
            _ => "Nenhuma foto para mostrar".to_string(),
        };
        let celulas: Vec<gpui_kit::AnyElement> = itens
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let em_foco = grade.foco == Some(desenho.faixa.start + i);
                self.celula_da_grade(item, i, em_foco, &a)
            })
            .collect();

        div()
            .id("palco-do-cliente")
            .on_double_click(
                cx.listener(|cliente, _, window, cx| cliente.alternar_tela_cheia(window, cx)),
            )
            .flex_1()
            .w_full()
            .bg(gpui_kit::black())
            .flex()
            .child(
                div()
                    .w(px(a.largura_das_fotos))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        // A página que entra aparece suave; o id com a página
                        // é o que reinicia a animação a cada troca.
                        div()
                            .relative()
                            .flex_1()
                            .w_full()
                            .children(celulas)
                            .with_animation(
                                ("pagina-da-grade", desenho.pagina),
                                Animation::new(ENTRADA_DA_PAGINA).with_easing(ease_in_out),
                                |pagina, delta| pagina.opacity(delta),
                            ),
                    )
                    .child(
                        div()
                            .h(px(PE_DA_GRADE))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(gpui_kit::rgb(0x8a8a8a))
                            .child(SharedString::from(pe)),
                    ),
            )
            .when(a.coluna_do_qr > 0., |tela| {
                tela.child(
                    div()
                        .w(px(a.coluna_do_qr))
                        .h_full()
                        .flex()
                        .items_end()
                        .justify_center()
                        .pb(px(24.))
                        .children(self.cartao_do_qr()),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui_kit::TestAppContext;

    fn foto(nome: &str, nota: i32) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            rating: nota,
            ..Default::default()
        }
    }

    /// A regra do legado: o primeiro que não for o principal.
    #[test]
    fn o_cliente_abre_no_monitor_que_nao_e_o_principal() {
        let (principal, outro) = (1u32, 2u32);

        assert_eq!(
            monitor_do_cliente(&[principal, outro], Some(principal)),
            Some(outro)
        );
        assert_eq!(
            monitor_do_cliente(&[outro, principal], Some(principal)),
            Some(outro),
            "a ordem da lista não decide — quem decide é não ser o principal"
        );
    }

    /// ⚠️ Com um monitor só, ela abre nele mesmo.
    ///
    /// É o `or_else` do legado, e é o que permite conferir a segunda tela sem um
    /// segundo monitor plugado. Devolver `None` faria o botão não fazer nada na
    /// máquina de quem está desenvolvendo — e "não faz nada" é indistinguível de
    /// defeito.
    /// 🔑 O monitor lembrado ganha da regra — desde que ainda esteja plugado.
    /// 🔑 O atalho existe e é uma combinação — nunca uma tecla F (o Mac do
    /// dono não as tem).
    #[test]
    fn o_atalho_da_tela_cheia_e_uma_combinacao() {
        let tecla = gpui_kit::Keystroke::parse(ATALHO_DA_TELA_CHEIA).expect("válido");

        assert_eq!(tecla.key, "f");
        assert!(tecla.modifiers.shift);
        assert!(tecla.modifiers.platform || tecla.modifiers.control);
        assert!(!texto_da_tela_cheia().is_empty());
    }

    #[test]
    fn o_monitor_lembrado_volta_se_ainda_existir() {
        let telas = [
            (1, Some("principal".to_string())),
            (2, Some("cliente".to_string())),
        ];

        assert_eq!(monitor_lembrado(&telas, Some("cliente")), Some(2));
        assert_eq!(monitor_lembrado(&telas, Some("principal")), Some(1));
        assert_eq!(
            monitor_lembrado(&telas, Some("desplugado")),
            None,
            "monitor que saiu não é escolha: vale a regra de sempre"
        );
        assert_eq!(monitor_lembrado(&telas, None), None);
    }

    fn limites(x: f32, y: f32, largura: f32, altura: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(largura), px(altura)),
        }
    }

    /// A janela volta onde estava e do tamanho que tinha.
    #[test]
    fn a_janela_lembrada_volta_onde_estava() {
        let tela = segundo_monitor();
        let lembrada = limites(2100., 200., 900., 600.);

        assert_eq!(area_da_janela(tela, Some(lembrada)), lembrada);
    }

    /// Sobrando para fora, é empurrada para dentro — nunca maior que a tela.
    #[test]
    fn a_janela_que_sobrava_para_fora_volta_para_dentro() {
        let tela = segundo_monitor();
        let area = area_da_janela(tela, Some(limites(4000., 1200., 900., 600.)));

        assert_eq!(area.size, size(px(900.), px(600.)));
        assert!(area.origin.x + area.size.width <= tela.origin.x + tela.size.width);
        assert!(area.origin.y + area.size.height <= tela.origin.y + tela.size.height);

        let enorme = area_da_janela(tela, Some(limites(1920., 0., 9000., 9000.)));
        assert_eq!(enorme, tela, "maior que a tela, ela cabe na tela");
    }

    /// Fora do monitor inteiro (o GNOME devolve a origem em zero), volta no
    /// tamanho lembrado, centrada nele.
    #[test]
    fn a_janela_lembrada_fora_do_monitor_volta_centrada() {
        let tela = segundo_monitor();
        let area = area_da_janela(tela, Some(limites(0., 0., 800., 500.)));

        assert_eq!(area.size, size(px(800.), px(500.)));
        assert_eq!(area.origin, point(px(1920. + 880.), px(470.)));
    }

    /// Sem lembrança, a prévia centrada de sempre.
    #[test]
    fn sem_lembranca_a_janela_e_a_previa() {
        let tela = segundo_monitor();

        assert_eq!(area_da_janela(tela, None), area_do_cliente(tela, false));
    }

    /// O arquivo antigo, só com o modo, continua sendo lido — e a lembrança
    /// grava e relê os três campos.
    #[test]
    fn a_lembranca_le_o_arquivo_antigo_e_guarda_os_tres_campos() {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let arquivo = pasta.path().join("tela-do-cliente.json");
        std::fs::write(&arquivo, r#"{"em_janela":true}"#).expect("gravar");

        let mut lembranca = Lembranca::ler(&arquivo);
        assert_eq!(lembranca.estado, None, "o campo antigo não vira estado");
        assert_eq!(lembranca.limites(), None);

        lembranca.estado = Some(Estado::TelaCheia);

        lembranca.monitor = Some("cliente".into());
        lembranca.lembrar_limites(limites(2100., 200., 900., 600.));
        lembranca.gravar(&arquivo);

        let relida = Lembranca::ler(&arquivo);
        assert_eq!(relida, lembranca);
        assert_eq!(relida.limites(), Some(limites(2100., 200., 900., 600.)));
    }

    /// 🔑 Sem lembrança, tela cheia com monitor próprio e janela dividindo a
    /// tela; lembrado, o lembrado.
    #[test]
    fn o_estado_ao_abrir_segue_a_lembranca_e_depois_o_monitor() {
        assert_eq!(estado_ao_abrir(None, true, false, false), Estado::TelaCheia);
        assert_eq!(estado_ao_abrir(None, false, false, true), Estado::Janela);
        assert_eq!(
            estado_ao_abrir(Some(Estado::Janela), true, false, false),
            Estado::Janela,
            "quem deixou em janela reabre em janela, mesmo com monitor próprio"
        );
        assert_eq!(
            estado_ao_abrir(Some(Estado::Maximizada), false, false, true),
            Estado::Maximizada
        );
    }

    /// 🚨 No Mac, tela cheia no monitor principal vira maximizar: o *Space*
    /// dela escondia o app num Mac de um monitor só.
    #[test]
    fn no_mac_a_tela_cheia_no_monitor_principal_maximiza() {
        assert!(!tela_cheia_de_verdade(true, true));
        assert!(
            tela_cheia_de_verdade(true, false),
            "no segundo monitor é tela cheia"
        );
        assert!(
            tela_cheia_de_verdade(false, true),
            "no Linux e no Windows sempre"
        );
        assert_eq!(
            estado_ao_abrir(Some(Estado::TelaCheia), false, true, true),
            Estado::Maximizada
        );
        assert_eq!(
            estado_ao_abrir(Some(Estado::TelaCheia), true, true, false),
            Estado::TelaCheia
        );
    }

    /// Resto de arrasto não é tamanho de janela.
    #[test]
    fn janela_minuscula_nao_e_lembrada() {
        let mut lembranca = Lembranca::default();
        lembranca.lembrar_limites(limites(0., 0., 40., 30.));

        assert_eq!(lembranca.limites(), None);
    }

    #[test]
    fn com_um_monitor_so_ela_abre_nele() {
        let unico = 1u32;
        assert_eq!(monitor_do_cliente(&[unico], Some(unico)), Some(unico));
    }

    /// Sem monitor nenhum não há onde abrir — e quem chama não abre janela.
    #[test]
    fn sem_monitor_nao_ha_onde_abrir() {
        assert_eq!(monitor_do_cliente::<u32>(&[], None), None);
    }

    fn monitor(id: u32, uuid: &str, x: f32, largura: f32, altura: f32) -> Monitor<u32> {
        Monitor {
            id,
            uuid: Some(uuid.into()),
            nome: None,
            limites: Bounds {
                origin: point(px(x), px(0.)),
                size: size(px(largura), px(altura)),
            },
        }
    }

    /// "Monitor 1" é o da esquerda, venha a lista na ordem que vier — inclusive
    /// o monitor à esquerda do principal, de origem negativa.
    #[test]
    fn os_monitores_vao_da_esquerda_para_a_direita() {
        let ordem = em_ordem(vec![
            monitor(7, "hdmi", 1920., 3840., 2160.),
            monitor(3, "dp", -1280., 1280., 1024.),
            monitor(5, "edp", 0., 1920., 1080.),
        ]);
        let ids: Vec<u32> = ordem.iter().map(|m| m.id).collect();
        assert_eq!(ids, [3, 5, 7]);
    }

    /// O da última vez vem marcado, se ainda estiver plugado — mesmo que seja o
    /// monitor do app.
    #[test]
    fn a_escolha_vem_marcada_no_monitor_da_ultima_vez() {
        let todos = [
            monitor(1, "edp", 0., 1920., 1080.),
            monitor(2, "hdmi", 1920., 3840., 2160.),
        ];
        assert_eq!(monitor_sugerido(&todos, Some("hdmi"), Some(1)), Some(2));
        assert_eq!(monitor_sugerido(&todos, Some("edp"), Some(1)), Some(1));
    }

    /// Sem lembrança (ou com o lembrado desplugado), o primeiro que não é o do
    /// app. 🚨 No Wayland não há principal: sem o "do app", a regra antiga caía
    /// no primeiro da lista, que podia ser o próprio monitor do operador.
    #[test]
    fn sem_lembranca_vem_marcado_o_monitor_que_nao_e_o_do_app() {
        let todos = [
            monitor(1, "edp", 0., 1920., 1080.),
            monitor(2, "hdmi", 1920., 3840., 2160.),
        ];
        assert_eq!(monitor_sugerido(&todos, None, Some(1)), Some(2));
        assert_eq!(
            monitor_sugerido(&todos, Some("dp-tirado"), Some(2)),
            Some(1)
        );
    }

    #[test]
    fn o_botao_diz_o_tamanho_e_onde_esta_o_app() {
        let tela = monitor(2, "hdmi", 1920., 3840., 2160.);
        assert_eq!(rotulo(2, &tela, Some(1)), "Monitor 2 · 3840 × 2160");
        assert_eq!(
            rotulo(2, &tela, Some(2)),
            "Monitor 2 · 3840 × 2160 · onde está o app"
        );
    }

    /// Dois monitores do mesmo tamanho só se distinguem pelo nome (dono,
    /// 11/out/2026).
    #[test]
    fn o_botao_diz_o_nome_do_monitor_quando_o_sistema_o_da() {
        let tela = Monitor {
            nome: Some("DELL U2720Q".into()),
            ..monitor(2, "hdmi", 1920., 3840., 2160.)
        };
        assert_eq!(
            rotulo(2, &tela, Some(1)),
            "Monitor 2 · DELL U2720Q · 3840 × 2160"
        );
        assert_eq!(
            rotulo(2, &tela, Some(2)),
            "Monitor 2 · DELL U2720Q · 3840 × 2160 · onde está o app"
        );
    }

    /// No Wayland, com monitor próprio, ela nasce **e fica** em tela cheia — o
    /// único estado que cai no monitor escolhido; sair dela devolve a janela
    /// ao monitor principal. Fora do Wayland, ou dividindo o monitor com o
    /// app, nasce como foi pedida.
    #[test]
    fn no_wayland_com_monitor_proprio_ela_fica_em_tela_cheia() {
        use Estado::*;
        assert_eq!(ao_nascer(Janela, true, true), TelaCheia);
        assert_eq!(ao_nascer(Maximizada, true, true), TelaCheia);
        assert_eq!(ao_nascer(TelaCheia, true, true), TelaCheia);
        assert_eq!(ao_nascer(Janela, false, true), Janela);
        assert_eq!(ao_nascer(Janela, true, false), Janela);
    }

    /// Uma tela de 2560x1440 com origem em (1920, 0): o segundo monitor à
    /// direita do principal.
    fn segundo_monitor() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(1920.), px(0.)),
            size: size(px(2560.), px(1440.)),
        }
    }

    /// Com monitor próprio, a janela toma o monitor inteiro — **origem
    /// incluída**.
    ///
    /// 🔑 A origem é o que a coloca lá. Os `Bounds` do GPUI são globais; devolver
    /// só o tamanho abriria uma janela do tamanho certo no monitor errado, que é
    /// exatamente a queixa de quem plugou o segundo monitor e não viu nada nele.
    #[test]
    fn com_monitor_proprio_ela_toma_o_monitor_inteiro() {
        let tela = segundo_monitor();
        assert_eq!(area_do_cliente(tela, true), tela);
    }

    /// Sem monitor próprio ela é prévia: centrada, e **não engole a tela**.
    ///
    /// 🚨 É a queixa do dono de 17/set/2026 — em tela cheia, num Mac de um
    /// monitor só, a segunda tela cobria o app e não havia mais triagem atrás
    /// dela.
    #[test]
    fn sem_monitor_proprio_ela_e_uma_previa_centrada() {
        let tela = segundo_monitor();
        let area = area_do_cliente(tela, false);

        assert_eq!(
            (f32::from(area.size.width), f32::from(area.size.height)),
            PREVIA,
            "o tamanho é o do pop-up do site, para as janelas serem comparáveis"
        );
        assert!(
            area.size.width < tela.size.width && area.size.height < tela.size.height,
            "a prévia não pode ocupar a tela toda: {area:?}"
        );
        assert!(
            area.origin.x > tela.origin.x && area.origin.y > tela.origin.y,
            "ela nasce dentro da tela, não colada no canto: {area:?}"
        );

        // Centrada: a sobra de cada lado é a mesma. Meio pixel de tolerância
        // porque a conta é em `f32` — exigir igualdade exata aqui seria um teste
        // sobre aritmética de ponto flutuante, e não sobre onde a janela nasce.
        let centrada =
            |antes: Pixels, depois: Pixels| (f32::from(antes) - f32::from(depois)).abs() < 0.5;
        assert!(
            centrada(
                area.origin.x - tela.origin.x,
                (tela.origin.x + tela.size.width) - (area.origin.x + area.size.width)
            ),
            "sobra desigual na horizontal: {area:?}"
        );
        assert!(
            centrada(
                area.origin.y - tela.origin.y,
                (tela.origin.y + tela.size.height) - (area.origin.y + area.size.height)
            ),
            "sobra desigual na vertical: {area:?}"
        );
    }

    /// Numa tela menor que a janela, ela encolhe em vez de sair pela borda.
    ///
    /// ⚠️ Sem o limite, a conta da centralização daria origem **negativa** e o
    /// canto da janela nasceria fora da tela — sem barra de título à vista para
    /// trazê-la de volta.
    #[test]
    fn numa_tela_pequena_a_janela_cabe_nela() {
        let pequena = Bounds {
            origin: point(px(0.), px(0.)),
            size: size(px(1024.), px(640.)),
        };
        let area = area_do_cliente(pequena, false);

        assert_eq!(area.size, pequena.size);
        assert_eq!(area.origin, pequena.origin);
    }

    /// A escala do cruzamento **nunca passa de 1** — senão a moldura é maior que
    /// a janela e a foto entra cortada (dono, 17/set/2026).
    #[test]
    fn a_foto_que_entra_nunca_e_maior_que_a_tela() {
        for passo in 0..=100 {
            let delta = passo as f32 / 100.0;
            let escala = escala_de_entrada(delta);
            assert!(
                escala <= 1.0,
                "com delta {delta} a camada ficou em {escala}, maior que a tela"
            );
            assert!(escala > 0.0);
        }
        // E ela termina exatamente na moldura: parar antes deixaria uma tarja
        // preta permanente em volta da foto.
        assert_eq!(escala_de_entrada(1.0), 1.0);
    }

    /// O rodapé conta onde estamos, o nome e a nota — as cinco estrelas.
    ///
    /// 🔑 **As vazias entram**, e é a diferença que a web trouxe: a nota se lê
    /// de longe sem contar o que não está lá.
    #[gpui_kit::test]
    fn o_rodape_mostra_a_posicao_o_nome_e_as_cinco_estrelas(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("retrato.jpg", 3)), None, Some((7, 25)), cx);
                let info = cliente.info().expect("há foto");
                assert_eq!(info.nome, "retrato.jpg");
                assert_eq!(info.cheias, "★★★");
                assert_eq!(info.vazias, "★★", "as vazias completam cinco");
                assert_eq!(info.posicao.as_ref().map(|p| p.as_ref()), Some("7 / 25"));

                cliente.mostrar(Some(foto("crua.NEF", 0)), None, None, cx);
                let info = cliente.info().expect("há foto");
                assert_eq!(info.cheias, "");
                assert_eq!(info.vazias, "★★★★★", "sem nota, cinco vazias");
                assert!(info.posicao.is_none(), "sem posição, o rodapé não inventa");

                cliente.mostrar(None, None, None, cx);
                assert!(cliente.info().is_none(), "sem foto não há rodapé");
            })
            .expect("a janela deve estar aberta");
    }

    /// A foto que o cliente já disse que leva aparece marcada.
    #[gpui_kit::test]
    fn a_levada_no_balcao_aparece_como_escolhida(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("a.jpg", 5)), None, None, cx);
                assert!(!cliente.info().expect("há foto").escolhida);

                let levada = PhotoViewModel {
                    comprada: true,
                    ..foto("b.jpg", 5)
                };
                cliente.mostrar(Some(levada), None, None, cx);
                assert!(
                    cliente.info().expect("há foto").escolhida,
                    "`comprada` no view model é levada no balcão — ver o campo"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// Um pixel qualquer, só para haver duas imagens distinguíveis.
    fn imagem() -> Arc<RenderImage> {
        use image::{Frame, RgbaImage};
        use smallvec::SmallVec;
        Arc::new(RenderImage::new(SmallVec::from_elem(
            Frame::new(RgbaImage::new(1, 1)),
            1,
        )))
    }

    /// ✨ Trocar de foto **guarda a anterior**: é ela que sai enquanto a nova entra.
    #[gpui_kit::test]
    fn a_foto_que_sai_fica_para_o_cruzamento(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));

        janela
            .update(cx, |cliente, _window, cx| {
                let primeira = imagem();
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(primeira.clone()), None, cx);
                assert!(
                    cliente.saindo.is_none(),
                    "a primeira foto não tem de quem sair"
                );
                let troca_da_primeira = cliente.troca;

                cliente.mostrar(Some(foto("b.jpg", 4)), Some(imagem()), None, cx);
                assert!(
                    cliente
                        .saindo
                        .as_ref()
                        .is_some_and(|s| Arc::ptr_eq(s, &primeira)),
                    "a que estava na tela é a que sai"
                );
                assert_ne!(
                    cliente.troca, troca_da_primeira,
                    "e a contagem anda — sem id novo o GPUI daria o cruzamento por terminado"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Continuar na mesma foto troca os pixels **sem** cruzamento.
    ///
    /// 🚨 O caso que faz a guarda existir: `imagem::para_gpui` devolve um `Arc`
    /// novo a cada chamada, então "é a mesma imagem?" não pode ser perguntado ao
    /// ponteiro. Se fosse, toda republicação contaria como troca e a foto
    /// piscaria — com o teste passando, porque o teste também usaria `Arc`s
    /// diferentes.
    #[gpui_kit::test]
    fn continuar_na_mesma_foto_nao_cruza_nada(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));

        janela
            .update(cx, |cliente, _window, cx| {
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(imagem()), None, cx);
                let troca = cliente.troca;

                // Cada chamada traz um `Arc` diferente, como a raiz faz de verdade.
                let revelada = imagem();
                cliente.mostrar(Some(foto("a.jpg", 3)), Some(revelada.clone()), None, cx);
                assert_eq!(
                    cliente.troca, troca,
                    "mesma foto, nenhum cruzamento — senão ela pisca a cada repintura"
                );
                assert!(
                    cliente
                        .imagem
                        .as_ref()
                        .is_some_and(|i| Arc::ptr_eq(i, &revelada)),
                    "mas os pixels novos entram no lugar: revelar precisa aparecer lá"
                );
                assert!(
                    cliente.saindo.is_none(),
                    "e nada foi para a camada de saída"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla `I` chega mesmo à segunda tela.
    ///
    /// Ela tem contexto e foco próprios — e ligação que não casa **não falha**,
    /// ela só não faz nada. É o defeito que custou dois commits na Revelação.
    /// 📸 O QR do bot na segunda tela: aparece quando a sessão manda, e só sai
    /// quando o balconista desliga — nem o `I` desta janela nem o cadastro
    /// concluído o tiram.
    #[gpui_kit::test]
    fn o_qr_do_bot_aparece_e_o_i_esconde(cx: &mut TestAppContext) {
        use crate::sessoes::qr_do_bot::{ConviteNaTela, EstadoDoBot};
        cx.update(init);
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));
        let convite = |estado| ConviteNaTela {
            codigo: "RF-ABC234".into(),
            qr: Arc::new(vec![vec![true, false], vec![false, true]]),
            estado,
            quem: None,
        };
        janela
            .update(cx, |cliente, _w, cx| {
                assert!(cliente.qr_na_tela().is_none(), "sem sessão, sem QR");
                cliente.definir_convite(Some(convite(EstadoDoBot::Aguardando)), cx);
                assert!(cliente.qr_na_tela().is_some());
            })
            .unwrap();

        // O `I` desta janela é do rodapé: o QR fica — quem o tira é o balconista.
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("i");
        janela
            .update(cx, |cliente, _w, cx| {
                assert!(!cliente.mostrando_info());
                assert!(
                    cliente.qr_na_tela().is_some(),
                    "o I da tela do cliente não tira o QR"
                );
                cliente.definir_convite(Some(convite(EstadoDoBot::Concluido)), cx);
                assert!(
                    cliente.qr_na_tela().is_some(),
                    "o cadastro concluído não tira o QR"
                );
                cliente.definir_convite(None, cx);
                assert!(cliente.qr_na_tela().is_none(), "o balconista desligou");
            })
            .unwrap();
    }

    fn grade_de(quantas: usize, foco: Option<usize>, por_linha: usize) -> GradeParaOCliente {
        GradeParaOCliente {
            itens: (0..quantas)
                .map(|i| ItemDaGrade {
                    numero: i + 1,
                    chave: format!("site:f{i}"),
                    escolhida: i == 2,
                    nota: 0,
                })
                .collect(),
            foco,
            por_linha,
        }
    }

    fn full_hd() -> Size<Pixels> {
        size(px(1920.), px(1080.))
    }

    fn convite_de_teste() -> crate::sessoes::qr_do_bot::ConviteNaTela {
        crate::sessoes::qr_do_bot::ConviteNaTela {
            codigo: "RF-ABC234".into(),
            qr: Arc::new(vec![vec![true, false], vec![false, true]]),
            estado: crate::sessoes::qr_do_bot::EstadoDoBot::Aguardando,
            quem: None,
        }
    }

    /// 🖼️ A grade entra no ar, e a página é a que tem a foto em foco.
    #[gpui_kit::test]
    fn na_grade_a_pagina_e_a_da_foto_em_foco(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));
        janela
            .update(cx, |cliente, _w, cx| {
                assert!(cliente.grade_mostrada().is_none());
                assert!(cliente.desenho_da_grade(full_hd()).is_none());

                cliente.mostrar_grade(grade_de(25, Some(0), 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!(d.arranjo.por_pagina(), 8, "Full HD, quatro por linha");
                assert_eq!((d.pagina, d.paginas, d.faixa), (0, 4, 0..8));

                cliente.mostrar_grade(grade_de(25, Some(10), 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!((d.pagina, d.faixa), (1, 8..16), "a 11 está na 2ª página");

                cliente.mostrar_grade(grade_de(25, Some(24), 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!((d.pagina, d.faixa), (3, 24..25), "a última página, com uma");

                // Mais por linha: outra página, os mesmos números.
                cliente.mostrar_grade(grade_de(25, Some(10), 8), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!(d.arranjo.por_linha, 8);
                assert!(d.faixa.contains(&10));
                let numeros: Vec<usize> = cliente.grade_mostrada().unwrap().itens[d.faixa]
                    .iter()
                    .map(|i| i.numero)
                    .collect();
                assert!(numeros.contains(&11));
            })
            .expect("a janela deve estar aberta");
    }

    /// Grade vazia, sem foco, ou com o foco além do fim: nada derruba, e a
    /// página é uma que existe.
    #[gpui_kit::test]
    fn a_grade_vazia_ou_com_foco_perdido_nao_derruba(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));
        janela
            .update(cx, |cliente, _w, cx| {
                cliente.mostrar_grade(grade_de(0, None, 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!((d.pagina, d.paginas, d.faixa), (0, 1, 0..0));

                cliente.mostrar_grade(grade_de(5, None, 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!(
                    (d.pagina, d.faixa),
                    (0, 0..5),
                    "sem foco, a primeira página"
                );

                cliente.mostrar_grade(grade_de(5, Some(400), 4), cx);
                let d = cliente.desenho_da_grade(full_hd()).expect("a grade");
                assert_eq!((d.pagina, d.faixa), (0, 0..5), "foco além do fim: a última");

                // Janela de tamanho zero (o primeiro quadro) e fotos por linha
                // fora da faixa.
                cliente.mostrar_grade(grade_de(5, Some(1), 0), cx);
                let d = cliente
                    .desenho_da_grade(size(px(0.), px(0.)))
                    .expect("a grade");
                assert_eq!(d.arranjo.por_linha, conta::MENOS_POR_LINHA);
                assert!(d.arranjo.linhas >= 1);
            })
            .expect("a janela deve estar aberta");
        // E desenha assim mesmo.
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("i");
    }

    /// 🚨 O pedido do dono (11/out/2026): na grade o QR tem coluna própria, e
    /// nenhuma célula fica debaixo dele. Fora da grade ele continua por cima.
    #[gpui_kit::test]
    fn na_grade_o_qr_fica_em_coluna_propria(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));
        janela
            .update(cx, |cliente, _w, cx| {
                cliente.mostrar_grade(grade_de(25, Some(0), 4), cx);
                let sem = cliente
                    .desenho_da_grade(full_hd())
                    .expect("a grade")
                    .arranjo;
                assert_eq!(sem.coluna_do_qr, 0., "sem QR, a tela é das fotos");
                assert_eq!(sem.largura_das_fotos, 1920.);

                cliente.definir_convite(Some(convite_de_teste()), cx);
                for por_linha in conta::MENOS_POR_LINHA..=conta::MAIS_POR_LINHA {
                    cliente.mostrar_grade(grade_de(25, Some(0), por_linha), cx);
                    let a = cliente
                        .desenho_da_grade(full_hd())
                        .expect("a grade")
                        .arranjo;
                    assert_eq!(a.coluna_do_qr, conta::COLUNA_DO_QR);
                    for i in 0..a.por_pagina() {
                        let c = a.celula(i);
                        assert!(
                            c.x + c.w <= 1920. - conta::COLUNA_DO_QR + 0.01,
                            "{por_linha} por linha: a célula {i} está debaixo do QR"
                        );
                    }
                }
                assert!(
                    cliente.cartao_do_qr().is_some(),
                    "o cartão existe para a coluna"
                );

                // Janela estreita demais: o QR sai, em vez de espremer as fotos.
                let estreita = cliente
                    .desenho_da_grade(size(px(700.), px(500.)))
                    .expect("a grade")
                    .arranjo;
                assert_eq!(estreita.coluna_do_qr, 0.);
            })
            .expect("a janela deve estar aberta");
        // Desenha com o QR na coluna.
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("i");
    }

    /// As imagens da grade vêm do cache de previews, só as da página no ar; o
    /// que o cache não tem vira célula vazia com o número — e não derruba.
    #[gpui_kit::test]
    fn a_grade_le_do_cache_so_a_pagina_no_ar(cx: &mut TestAppContext) {
        use image::{DynamicImage, RgbaImage};
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let previews = Arc::new(PreviewManager::new_with_path(pasta.path().to_path_buf()));
        let foto = |l, a| DynamicImage::ImageRgba8(RgbaImage::new(l, a));
        // 25 fotos; a `f3` não tem imagem nenhuma, a `f5` só a grande.
        for i in (0..25).filter(|i| *i != 3) {
            let chave = format!("site:f{i}");
            if i != 5 {
                previews
                    .save_thumbnail(&chave, &foto(32, 24))
                    .expect("gravar a miniatura");
            }
            previews
                .save_preview(&chave, &foto(64, 48))
                .expect("gravar a prévia");
        }
        cx.update(init);
        let janela =
            cx.add_window(move |window, cx| Cliente::novo(None, window, cx).com_previews(previews));
        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);

        janela
            .update(&mut visual, |cliente, _w, cx| {
                cliente.mostrar_grade(grade_de(25, Some(0), 8), cx)
            })
            .unwrap();
        visual.simulate_keystrokes("i");
        visual.run_until_parked();
        let pagina = janela
            .update(&mut visual, |cliente, window, _cx| {
                let d = cliente
                    .desenho_da_grade(window.viewport_size())
                    .expect("a grade");
                for i in d.faixa.clone() {
                    let chave = format!("site:f{i}");
                    if i == 3 {
                        assert!(cliente.sem_imagem.contains(&chave), "a f3 não tem imagem");
                        assert!(!cliente.imagens_da_grade.contains_key(&chave));
                    } else {
                        assert!(
                            cliente.imagens_da_grade.contains_key(&chave),
                            "a foto {i} da página foi lida"
                        );
                    }
                }
                assert!(
                    cliente.imagens_da_grade.len() <= d.faixa.len(),
                    "só a página no ar fica na memória"
                );
                d.faixa
            })
            .unwrap();

        // O foco vai para a última foto: a página muda, e a memória com ela.
        if pagina.end < 25 {
            janela
                .update(&mut visual, |cliente, _w, cx| {
                    cliente.mostrar_grade(grade_de(25, Some(24), 8), cx)
                })
                .unwrap();
            visual.simulate_keystrokes("i");
            visual.run_until_parked();
            janela
                .update(&mut visual, |cliente, window, _cx| {
                    let d = cliente
                        .desenho_da_grade(window.viewport_size())
                        .expect("a grade");
                    assert!(d.faixa.contains(&24));
                    assert!(cliente.imagens_da_grade.contains_key("site:f24"));
                    assert!(
                        !cliente.imagens_da_grade.contains_key("site:f0"),
                        "a página que saiu não fica na memória"
                    );
                })
                .unwrap();
        }

        // A raiz avisa de imagem nova: relê no quadro seguinte, e a que
        // faltava é procurada de novo.
        janela
            .update(&mut visual, |cliente, _w, cx| {
                cliente.mostrar_grade(grade_de(25, Some(0), 8), cx);
                cliente.reler_as_imagens_da_grade(cx);
                assert!(cliente.reler_a_grade);
                assert!(cliente.sem_imagem.is_empty());
            })
            .unwrap();
        visual.simulate_keystrokes("i");
        visual.run_until_parked();
        janela
            .update(&mut visual, |cliente, _w, _cx| {
                assert!(!cliente.reler_a_grade, "a releitura foi feita");
            })
            .unwrap();
    }

    /// Fora da grade, `reler` não liga nada — a foto única não tem página.
    #[gpui_kit::test]
    fn reler_sem_grade_nao_faz_nada(cx: &mut TestAppContext) {
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));
        janela
            .update(cx, |cliente, _w, cx| {
                cliente.reler_as_imagens_da_grade(cx);
                assert!(!cliente.reler_a_grade);
            })
            .unwrap();
    }

    /// A lembrança guarda o modo e as fotos por linha — e o arquivo de antes,
    /// sem os campos, é a foto única com o padrão.
    #[test]
    fn a_lembranca_guarda_a_grade_e_as_fotos_por_linha() {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let arquivo = pasta.path().join("tela-do-cliente.json");
        std::fs::write(&arquivo, r#"{"estado":"janela","monitor":"hdmi"}"#).expect("gravar");

        let mut lembranca = Lembranca::ler(&arquivo);
        assert!(!lembranca.grade, "o arquivo de antes é a foto única");
        assert_eq!(lembranca.fotos_por_linha(), conta::POR_LINHA_PADRAO);

        lembranca.grade = true;
        lembranca.por_linha = Some(6);
        lembranca.gravar(&arquivo);
        let relida = Lembranca::ler(&arquivo);
        assert_eq!(relida, lembranca);
        assert_eq!(relida.monitor.as_deref(), Some("hdmi"), "o resto fica");
        assert_eq!(relida.fotos_por_linha(), 6);

        // Arquivo mexido à mão não põe 40 por linha, nem zero.
        lembranca.por_linha = Some(40);
        assert_eq!(lembranca.fotos_por_linha(), conta::MAIS_POR_LINHA);
        lembranca.por_linha = Some(0);
        assert_eq!(lembranca.fotos_por_linha(), conta::MENOS_POR_LINHA);
    }

    #[gpui_kit::test]
    fn a_tecla_i_liga_e_desliga_o_rodape(cx: &mut TestAppContext) {
        cx.update(init);
        let janela = cx.add_window(|window, cx| Cliente::novo(None, window, cx));

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(cliente.mostrando_info(), "nasce ligado, como no legado");
            })
            .expect("a janela deve estar aberta");

        let mut visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_keystrokes("i");

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(!cliente.mostrando_info());
            })
            .expect("a janela deve estar aberta");

        visual.simulate_keystrokes("i");

        janela
            .update(cx, |cliente, _window, _cx| {
                assert!(cliente.mostrando_info(), "e volta");
            })
            .expect("a janela deve estar aberta");
    }
}
