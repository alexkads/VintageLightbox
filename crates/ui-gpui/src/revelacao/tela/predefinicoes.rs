//! A coluna das predefinições — o `painel-presets.tsx` do site, com a prévia e
//! o aplicar do `editor.tsx`.
//!
//! | Gesto | Faz |
//! |---|---|
//! | passar o mouse numa linha | mostra na foto — partindo do neutro quando ela substitui |
//! | clicar no nome | aplica: um passo de histórico, gravado na hora |
//! | `+` | abre o formulário embutido: Enter salva, Esc cancela |
//! | ícone de envio | importa `.lrtemplate` e `.xmp` do Lightroom |
//! | lápis / lixeira (ao passar o mouse) | renomeia no lugar (Enter/Esc/✓/✕) · apaga depois de perguntar |
//! | botão direito → "Atualizar com os ajustes atuais" | a do operador passa a guardar o que está na foto |
//! | botão direito → "Duplicar" | uma cópia em "Minhas" — também das do sistema, que viram editáveis |
//! | botão direito → "Exportar…" | grava a predefinição num `.rfpreset` |
//! | botão direito na pasta → "Exportar o grupo…" | grava o grupo num `.zip` |
//! | ícone de baixar · botão direito → "Exportar todas…" | todas num `.zip`, uma pasta por grupo |
//! | arrastar a linha | reordena dentro do grupo; ↑ ↓ com a alça focada; "ordem padrão" desfaz |
//!
//! 🔑 **Reordenar fica desligado durante a busca**, como no site: a lista
//! filtrada esconde os vizinhos, e soltar "entre" duas que aparecem juntas poria
//! a predefinição no meio de outras cinco.
//!
//! A regra de negócio — somar ou substituir, o resumo, a ordem, as traduções —
//! mora em [`crate::revelacao::presets`] e [`crate::revelacao::lightroom`], sem
//! tela; aqui é só o desenho e os gestos.

use crate::campo::TrocarValor as _;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

#[cfg(test)]
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Preset, PresetId};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::{
    actions, anchored, canvas, deferred, div, point, prelude::*, px, relative, rgb, Anchor,
    AnchoredPositionMode, AnyElement, App, Context, CursorStyle, DragMoveEvent, Entity,
    FocusHandle, FontWeight, KeyBinding, MouseButton, Pixels, SharedString, Size, Subscription,
    Window,
};

use super::{PedidoDaRevelacao, Revelacao};
use crate::modal::Modal;
use crate::recursos::Icone;
use crate::revelacao::lightroom::{self, Arquivo, Gravacao};
use crate::revelacao::presets::arquivo::{self as rfpreset, ArquivoDePredefinicao, ItemDoPacote};
use crate::revelacao::presets::{self, ordem, ordem::Grupo};
use crate::revelacao::processador::Ajustes;
use crate::tema::cores;

actions!(
    predefinicoes,
    [
        SubirPredefinicao,
        DescerPredefinicao,
        ConfirmarPergunta,
        CancelarPergunta
    ]
);

/// O contexto de teclado da pergunta de apagar: Enter confirma, Esc cancela —
/// e o Esc não chega à raiz, onde ele sairia da Revelação.
const CONTEXTO_DA_PERGUNTA: &str = "PerguntaDaPredefinicao";

/// Quanto um aviso fica no canto — o padrão do `sonner` do site.
const DURACAO_DO_AVISO: Duration = Duration::from_secs(4);

/// O contexto de teclado da alça de uma linha. ↑ e ↓ aqui dentro reordenam, e
/// ganham das setas da Revelação por ser o contexto mais fundo.
const CONTEXTO_DA_ALCA: &str = "AlcaDaPredefinicao";

/// Marca que as teclas da alça já foram ligadas neste app.
struct AtalhosLigados;
impl gpui_kit::Global for AtalhosLigados {}

/// Liga ↑ e ↓ na alça — uma vez por app.
///
/// ⚠️ **Aqui, e não no `init` da raiz**: a alça é desta coluna, e o `main.rs`
/// não precisa saber que ela existe.
pub(super) fn ligar_atalhos(cx: &mut App) {
    if cx.has_global::<AtalhosLigados>() {
        return;
    }
    cx.set_global(AtalhosLigados);
    cx.bind_keys([
        KeyBinding::new("up", SubirPredefinicao, Some(CONTEXTO_DA_ALCA)),
        KeyBinding::new("down", DescerPredefinicao, Some(CONTEXTO_DA_ALCA)),
        KeyBinding::new("enter", ConfirmarPergunta, Some(CONTEXTO_DA_PERGUNTA)),
        KeyBinding::new("escape", CancelarPergunta, Some(CONTEXTO_DA_PERGUNTA)),
    ]);
}

/// O que a coluna guarda além do que já estava na tela.
pub(super) struct Predefinicoes {
    /// O formulário de "salvar como predefinição". Os três que tomam o foco
    /// para um campo moram no [`Modal`]: fechar é devolver.
    pub criando: Modal<()>,
    /// A linha que virou campo de nome.
    pub renomeando: Modal<PresetId>,
    /// A ordem escolhida neste computador.
    pub ordem: ordem::Ordem,
    /// O arrasto em curso: de onde saiu, e sobre qual linha (antes ou depois).
    pub arrasto: Option<ArrastoEmCurso>,
    /// O foco da alça de cada linha, pela chave da ordem. `RefCell` porque as
    /// alças nascem no desenho, que só tem `&self`.
    focos: RefCell<HashMap<String, FocusHandle>>,
    /// A linha (ou a pasta) do último botão direito — o menu da lista a lê e
    /// esvazia.
    pub alvo_do_menu: Option<AlvoDoMenu>,
    /// A exportação esperando a janela do sistema: o que dizer quando ela
    /// gravar. `Some` também desliga um segundo "Exportar" no meio.
    pub exportando: Option<String>,
    exportacao: (
        std::sync::mpsc::Sender<Gravacao>,
        std::sync::mpsc::Receiver<Gravacao>,
    ),
    /// A predefinição que espera o "Apagar" ou o "Cancelar".
    pub pergunta: Modal<(PresetId, String)>,
    foco_da_pergunta: Option<FocusHandle>,
    /// Os avisos no canto — o `toast` do site.
    pub avisos: Vec<Aviso>,
    proximo_aviso: u64,
    /// O tamanho da janela no último quadro, para a pergunta cobrir a tela.
    ///
    /// ⚠️ **A pergunta e os avisos são desenhados aqui**, e não pelo `Root` do
    /// `gpui-component`: o app não desenha a camada de diálogos nem a de avisos
    /// dele, e `open_dialog` não aparecia — a lixeira não fazia nada visível.
    janela: Rc<Cell<Size<Pixels>>>,
}

/// Um aviso no canto.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Aviso {
    pub texto: String,
    pub erro: bool,
    numero: u64,
}

impl Default for Predefinicoes {
    fn default() -> Self {
        Self {
            criando: Modal::default(),
            renomeando: Modal::default(),
            ordem: ordem::ler(),
            arrasto: None,
            alvo_do_menu: None,
            exportando: None,
            exportacao: std::sync::mpsc::channel(),
            focos: RefCell::new(HashMap::new()),
            pergunta: Modal::default(),
            foco_da_pergunta: None,
            avisos: Vec::new(),
            proximo_aviso: 0,
            janela: Rc::new(Cell::new(Size::default())),
        }
    }
}

/// O nome da cópia: "X (cópia)", e "X (cópia 2)" em diante quando já existe.
fn nome_da_copia(nome: &str, presets: &[Preset]) -> String {
    let existe = |candidato: &str| presets.iter().any(|p| p.name == candidato);
    let primeiro = format!("{nome} (cópia)");
    if !existe(&primeiro) {
        return primeiro;
    }
    (2..)
        .map(|n| format!("{nome} (cópia {n})"))
        .find(|candidato| !existe(candidato))
        .expect("a sequência não acaba")
}

/// Onde foi o botão direito.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlvoDoMenu {
    Preset(PresetId),
    /// O título de uma pasta.
    Grupo(Grupo),
}

/// O que exportar — os três gestos do pedido do dono.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Exportacao {
    /// Uma, num `.rfpreset`.
    Uma(PresetId),
    /// Um grupo da coluna, num `.zip`.
    Grupo(Grupo),
    /// Todas, num `.zip` com uma pasta por grupo.
    Todas,
}

/// O nome do grupo como a pasta do pacote e o campo `grupo` do arquivo o
/// escrevem — o da coluna, sem as maiúsculas do título.
pub fn rotulo_do_grupo(grupo: Grupo) -> &'static str {
    match grupo {
        Grupo::Favoritas => "Favoritas",
        Grupo::Sistema => "Do sistema",
        Grupo::Minhas => "Minhas",
        Grupo::Lrs => "LRs",
    }
}

/// O arquivo pronto para gravar: o nome sugerido, os bytes e quantas
/// predefinições vão nele.
#[derive(Debug, Clone, PartialEq)]
pub struct Exportada {
    pub nome: String,
    pub conteudo: Vec<u8>,
    pub quantas: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ArrastoEmCurso {
    pub grupo: Grupo,
    pub chave: String,
    pub sobre: Option<String>,
    pub depois: bool,
}

/// O valor que viaja com o arrasto de uma linha.
#[derive(Debug, Clone)]
pub(super) struct ArrastoDePreset {
    grupo: Grupo,
    nome: SharedString,
}

/// O que segue o ponteiro durante o arrasto: o nome da predefinição.
struct FantasmaDoPreset {
    nome: SharedString,
}

impl Render for FantasmaDoPreset {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(6.))
            .py(px(4.))
            .rounded(crate::tema::canto(4.))
            .bg(cx.theme().muted)
            .opacity(0.85)
            .text_size(crate::tema::letra::em(12.))
            .line_height(px(16.))
            .text_color(cx.theme().foreground)
            .child(self.nome.clone())
    }
}

/// Os campos de nome respondem ao Enter.
pub(super) fn assinar(
    nome: &Entity<InputState>,
    renome: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Revelacao>,
) -> Vec<Subscription> {
    vec![
        cx.subscribe_in(
            nome,
            window,
            |tela: &mut Revelacao, _campo, evento: &InputEvent, window, cx| match evento {
                InputEvent::PressEnter { .. } => tela.salvar_preset(window, cx),
                // O "Salvar" liga e desliga com o nome.
                InputEvent::Change => cx.notify(),
                _ => {}
            },
        ),
        cx.subscribe_in(
            renome,
            window,
            |tela: &mut Revelacao, campo, evento: &InputEvent, _window, cx| {
                if let (InputEvent::PressEnter { .. }, Some(&id)) =
                    (evento, tela.predefinicoes.renomeando.aberto())
                {
                    let nome = campo.read(cx).value().to_string();
                    tela.renomear_preset(id, nome, cx);
                }
            },
        ),
    ]
}

impl Revelacao {
    // ------------------------------------------------------------ prévia

    /// O que a GPU desenha: os ajustes de verdade, ou eles com a predefinição
    /// sob o ponteiro.
    ///
    /// 🔑 **A prévia não entra em `self.ajustes`**, e é o que a mantém
    /// reversível de graça. 🚨 **E é a mesma conta do clique**
    /// ([`presets::aplicado`]): a que substitui parte do neutro também aqui.
    pub(super) fn ajustes_na_tela(&self) -> Ajustes {
        // 📜 O passo do histórico sob o ponteiro: a foto daquele passo.
        if let Some(passo) = self.passo_em_previa() {
            return self.sem_o_que_o_olho_esconde(passo.estado.ajustes);
        }
        let ajustes = match &self.previa {
            Some(preset) => presets::aplicado(&self.ajustes, preset),
            None => self.ajustes,
        };
        // O olho da Correção de cores apertado: a prévia sem aquela faixa.
        self.sem_o_que_o_olho_esconde(ajustes)
    }

    /// Mostra (ou tira) a prévia de uma predefinição.
    ///
    /// Sem mudança não há o que redesenhar — pedir à GPU o mesmo quadro a cada
    /// movimento do ponteiro sobre a mesma linha seria trabalho por nada.
    pub fn prever(&mut self, preset: Option<&Preset>, cx: &mut Context<Self>) {
        // `None` também tira a prévia de um passo do histórico: a coluna que
        // some leva as duas.
        if self.previa.as_ref() == preset && self.previa_do_passo.is_none() {
            return;
        }
        self.previa = preset.cloned();
        self.previa_do_passo = None;
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// A predefinição que está sendo prevista, para os testes.
    #[cfg(test)]
    pub fn previa(&self) -> Option<&PresetAdjustments> {
        self.previa.as_ref().map(|preset| &preset.adjustments)
    }

    /// Aplica uma predefinição.
    ///
    /// É um gesto discreto, como o `Cmd+Z` — vira passo de histórico e vai para
    /// o banco **na hora**, sem a espera de 500 ms dos arrastos.
    ///
    /// 🔑 **Somar ou substituir, e quem decide é a predefinição**
    /// ([`presets::substitui`]). ⚠️ O enquadramento nunca entra: recortar é
    /// outra decisão, e é a mesma regra do "Zerar tudo".
    pub fn aplicar_preset(&mut self, preset: &Preset, window: &mut Window, cx: &mut Context<Self>) {
        // O que estiver a meio caminho fecha primeiro, pelo mesmo motivo do
        // desfazer: senão a espera pendente grava por cima do preset.
        self.gravar_o_que_estiver_pendente();

        self.ajustes = presets::aplicado(&self.ajustes, preset);
        self.espalhar_nos_sliders(window, cx);
        self.pedir_revelacao_cruzando(cx);
        self.historico
            .registrar_como(self.estado(), format!("Predefinição: {}", preset.name));
        self.gravar();
        cx.notify();
    }

    /// A coluna está travada: sem foto pronta, ou com a revelação da foto
    /// travada no site (comprada ou apagada) — o
    /// `desabilitado={!pronto || !podeRevelar}` do site. A levada no balcão
    /// continua revelável.
    pub(super) fn predefinicoes_desligadas(&self) -> bool {
        !self.tem_pixels() || !self.pode_revelar()
    }

    /// Travada, ou esperando o seletor de arquivos — o `travado` do site.
    fn predefinicoes_travadas(&self) -> bool {
        self.predefinicoes_desligadas() || self.escolhendo_arquivos
    }

    // --------------------------------------------------------- criar

    /// Abre (ou fecha) o formulário de "salvar como predefinição".
    ///
    /// O campo começa vazio a cada abertura: o nome anterior sugerido convida a
    /// salvar dois com o mesmo nome.
    pub fn alternar_formulario_de_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.nome_do_preset
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        if self.predefinicoes.criando.esta_aberto() {
            self.predefinicoes.criando.fechar(window, cx);
        } else {
            self.predefinicoes.criando.abrir((), window, cx);
            self.nome_do_preset
                .update(cx, |campo, cx| campo.focus(window, cx));
        }
        cx.notify();
    }

    pub fn cancelar_formulario_de_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.predefinicoes.criando.fechar(window, cx);
        cx.notify();
    }

    /// A caixa "Zerar os outros ajustes ao aplicar".
    pub fn alternar_preset_inteiro(&mut self, cx: &mut Context<Self>) {
        self.preset_inteiro = !self.preset_inteiro;
        cx.notify();
    }

    /// Guarda os ajustes de agora como predefinição do operador.
    ///
    /// 🚨 **Nome vazio não salva, e nada fora do neutro também não** — a menos
    /// que a caixa de zerar esteja marcada, que é a predefinição que devolve a
    /// foto ao original. O diálogo antigo tinha o OK sempre ligado e gravava uma
    /// predefinição vazia.
    ///
    /// 🔑 **O `Preset` nasce aqui, com o id que vai para os dois lados**: a
    /// lista da tela e a tabela do banco falam da mesma linha.
    pub fn salvar_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nome = self.nome_do_preset.read(cx).value().trim().to_string();
        if nome.is_empty() {
            return;
        }
        if presets::dos_ajustes(&self.ajustes, false).is_empty() && !self.preset_inteiro {
            self.avisar_na_coluna(
                "Não há nenhum ajuste fora do neutro para guardar.",
                true,
                cx,
            );
            return;
        }

        // Zerar o resto ao aplicar é guardar todos: o neutro dos que não foram
        // mexidos vai junto, e escrever tudo é o que apaga o que estava antes.
        let novo = Preset::user(
            nome,
            presets::dos_ajustes(&self.ajustes, self.preset_inteiro),
        );
        self.guarda_de_presets.salvar(novo.clone());
        self.avisar_na_coluna(
            format!("\"{}\" entrou nas predefinições.", novo.name),
            false,
            cx,
        );
        self.presets.push(novo);

        self.predefinicoes.criando.fechar(window, cx);
        self.preset_inteiro = false;
        self.nome_do_preset
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        cx.notify();
    }

    // ------------------------------------------------------ importar

    /// Abre o seletor do sistema para importar do Lightroom.
    ///
    /// 🚨 **O laço de colheita sobe antes da resposta**, e não depois: o seletor
    /// é uma janela do sistema e pode voltar a qualquer momento.
    pub fn importar_do_lightroom(&mut self, cx: &mut Context<Self>) {
        if self.escolhendo_arquivos {
            return;
        }
        self.escolhendo_arquivos = true;
        self.relatorio = None;
        self.escolha_de_presets.escolher(self.arquivos.0.clone());
        self.esperar_arquivos(cx);
        cx.notify();
    }

    /// Acorda a tela de tempos em tempos enquanto o seletor está aberto.
    fn esperar_arquivos(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |tela, cx| loop {
            // ⚠️ Uma espera bem mais longa que a da GPU: do outro lado há uma
            // pessoa procurando arquivo numa janela do sistema.
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let continua = tela
                .update(cx, |tela, cx| tela.colher_arquivos(cx))
                .unwrap_or(false);
            if !continua {
                break;
            }
        })
        .detach();
    }

    /// Drena o que o seletor mandou. Devolve se vale continuar acordando.
    fn colher_arquivos(&mut self, cx: &mut Context<Self>) -> bool {
        let mut chegou = false;
        while let Ok(arquivos) = self.arquivos.1.try_recv() {
            chegou = true;
            self.importar(arquivos, cx);
        }
        if chegou {
            self.escolhendo_arquivos = false;
            cx.notify();
        }
        self.escolhendo_arquivos
    }

    /// Traduz o que foi lido e guarda o que virou predefinição.
    ///
    /// ⚠️ **A lista da tela recebe as novas na hora**: quem acabou de importar
    /// quer aplicá-las agora.
    pub fn importar(&mut self, arquivos: Vec<Arquivo>, cx: &mut Context<Self>) {
        let nomes: Vec<String> = self.presets.iter().map(|p| p.name.clone()).collect();
        let (novas, relatorio) = lightroom::preparar(&arquivos, &nomes);

        for traduzida in novas {
            let preset = Preset::user(traduzida.nome, traduzida.ajustes);
            self.guarda_de_presets.salvar(preset.clone());
            self.presets.push(preset);
        }

        // Nada escolhido não é resultado: quem desiste do seletor não precisa
        // ler "0 arquivos lidos".
        self.relatorio = (relatorio.arquivos > 0).then_some(relatorio);
        cx.notify();
    }

    /// Fecha o resultado da última importação.
    pub fn fechar_relatorio(&mut self, cx: &mut Context<Self>) {
        self.relatorio = None;
        cx.notify();
    }

    // ------------------------------------------------------ exportar

    /// O arquivo de uma exportação, montado sem tocar em disco.
    ///
    /// `Ok(None)` quando não há o que exportar (grupo vazio, predefinição que
    /// sumiu). 🔑 **A coluna inteira, sem a busca**: quem exporta "LRs" com
    /// "vinheta" digitado quer o grupo, e não as cinco que a busca mostra.
    pub fn arquivo_da_exportacao(&self, alvo: Exportacao) -> Result<Option<Exportada>, String> {
        let erro = |e: std::io::Error| format!("não deu para montar o pacote: {e}");
        match alvo {
            Exportacao::Uma(id) => {
                let Some(preset) = self.presets.iter().find(|p| p.id == id) else {
                    return Ok(None);
                };
                let grupo = rotulo_do_grupo(Grupo::de(preset));
                Ok(Some(Exportada {
                    nome: rfpreset::nome_de_arquivo(&preset.name, rfpreset::EXTENSAO),
                    conteudo: ArquivoDePredefinicao::do_preset(preset, grupo)
                        .texto()
                        .into_bytes(),
                    quantas: 1,
                }))
            }
            Exportacao::Grupo(grupo) => {
                let coluna = presets::da_coluna(&self.presets, "", &self.predefinicoes.ordem);
                let lista = match grupo {
                    Grupo::Favoritas => coluna.favoritas,
                    Grupo::Sistema => coluna.sistema,
                    Grupo::Minhas => coluna.minhas,
                    Grupo::Lrs => coluna.lrs,
                };
                if lista.is_empty() {
                    return Ok(None);
                }
                let itens: Vec<ItemDoPacote<'_>> = lista
                    .iter()
                    .map(|preset| ItemDoPacote {
                        preset,
                        grupo: rotulo_do_grupo(Grupo::de(preset)),
                        pasta: None,
                    })
                    .collect();
                Ok(Some(Exportada {
                    nome: rfpreset::nome_de_arquivo(rotulo_do_grupo(grupo), "zip"),
                    conteudo: rfpreset::pacote(&itens).map_err(erro)?,
                    quantas: itens.len(),
                }))
            }
            Exportacao::Todas => {
                // As favoritas voltam ao grupo de origem: no pacote, cada
                // predefinição aparece uma vez só, na pasta dela.
                let ordem = ordem::Ordem {
                    favoritas: Vec::new(),
                    ..self.predefinicoes.ordem.clone()
                };
                let coluna = presets::da_coluna(&self.presets, "", &ordem);
                let itens: Vec<ItemDoPacote<'_>> = [
                    (Grupo::Minhas, coluna.minhas),
                    (Grupo::Sistema, coluna.sistema),
                    (Grupo::Lrs, coluna.lrs),
                ]
                .into_iter()
                .flat_map(|(grupo, lista)| {
                    let rotulo = rotulo_do_grupo(grupo);
                    lista.into_iter().map(move |preset| ItemDoPacote {
                        preset,
                        grupo: rotulo,
                        pasta: Some(rotulo),
                    })
                })
                .collect();
                if itens.is_empty() {
                    return Ok(None);
                }
                Ok(Some(Exportada {
                    nome: "Predefinições.zip".into(),
                    conteudo: rfpreset::pacote(&itens).map_err(erro)?,
                    quantas: itens.len(),
                }))
            }
        }
    }

    /// Exporta: monta o arquivo e pergunta ao sistema onde gravar — o
    /// "Exportar…" do Lightroom.
    pub fn exportar_predefinicoes(&mut self, alvo: Exportacao, cx: &mut Context<Self>) {
        if self.predefinicoes.exportando.is_some() {
            return;
        }
        let exportada = match self.arquivo_da_exportacao(alvo) {
            Ok(Some(exportada)) => exportada,
            Ok(None) => {
                self.avisar_na_coluna("Não há predefinição para exportar aqui.", true, cx);
                return;
            }
            Err(erro) => {
                self.avisar_na_coluna(format!("A exportação falhou: {erro}"), true, cx);
                return;
            }
        };
        let feito = match alvo {
            Exportacao::Uma(id) => {
                let nome = self
                    .presets
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str());
                format!("\"{}\" exportada", nome.unwrap_or_default())
            }
            Exportacao::Grupo(grupo) => format!(
                "{} de \"{}\" exportada{}",
                quantas_predefinicoes(exportada.quantas),
                rotulo_do_grupo(grupo),
                if exportada.quantas == 1 { "" } else { "s" },
            ),
            Exportacao::Todas => format!(
                "{} exportada{}",
                quantas_predefinicoes(exportada.quantas),
                if exportada.quantas == 1 { "" } else { "s" },
            ),
        };
        self.predefinicoes.exportando = Some(feito);
        self.escolha_de_presets.gravar(
            exportada.nome,
            exportada.conteudo,
            self.predefinicoes.exportacao.0.clone(),
        );
        // A mesma espera da importação: do outro lado há uma janela do
        // sistema, que volta quando a pessoa escolher.
        cx.spawn(async move |tela, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let continua = tela
                .update(cx, |tela, cx| tela.colher_exportacao(cx))
                .unwrap_or(false);
            if !continua {
                break;
            }
        })
        .detach();
        cx.notify();
    }

    /// Lê a resposta da janela de gravar. Devolve se vale continuar acordando.
    fn colher_exportacao(&mut self, cx: &mut Context<Self>) -> bool {
        let Ok(resposta) = self.predefinicoes.exportacao.1.try_recv() else {
            return self.predefinicoes.exportando.is_some();
        };
        let feito = self.predefinicoes.exportando.take().unwrap_or_default();
        match resposta {
            // Desistir da janela não é resultado — o mesmo do importar.
            Ok(None) => {}
            Ok(Some(caminho)) => {
                let arquivo = caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.avisar_na_coluna(format!("{feito} em {arquivo}."), false, cx);
            }
            Err(erro) => self.avisar_na_coluna(format!("Não deu para gravar: {erro}"), true, cx),
        }
        cx.notify();
        false
    }

    // ---------------------------------------------- renomear e apagar

    /// A linha vira campo, começando com o nome de agora.
    pub fn comecar_a_renomear(
        &mut self,
        id: PresetId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preset) = self.presets.iter().find(|p| p.id == id && !p.is_system) else {
            return;
        };
        let nome = preset.name.clone();
        self.predefinicoes.renomeando.abrir(id, window, cx);
        self.renome_do_preset.update(cx, |campo, cx| {
            campo.trocar_valor(nome, window, cx);
            campo.focus(window, cx);
        });
        cx.notify();
    }

    pub fn cancelar_renome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.predefinicoes.renomeando.fechar(window, cx);
        cx.notify();
    }

    /// Troca o nome de uma predefinição do operador.
    ///
    /// 🔑 **A lista da tela muda junto com o banco**. Nome em branco não vale —
    /// e o campo continua aberto, como no site.
    pub fn renomear_preset(&mut self, id: PresetId, nome: String, cx: &mut Context<Self>) {
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            return;
        }

        let Some(preset) = self
            .presets
            .iter_mut()
            .find(|preset| preset.id == id && !preset.is_system)
        else {
            return;
        };
        preset.name = nome.clone();
        if self.predefinicoes.renomeando.aberto() == Some(&id) {
            // Sem janela à mão: quem renomeia é o Enter do campo e os testes.
            self.predefinicoes.renomeando.fechar_depois(cx);
        }

        self.guarda_de_presets.renomear(id, nome);
        cx.notify();
    }

    /// Grava numa predefinição do operador os ajustes que estão na foto — o
    /// "Atualizar com as configurações atuais" do Lightroom.
    ///
    /// 🔑 **A mesma linha, e no mesmo modo em que nasceu**: o id, o nome e o
    /// lugar na lista e nas favoritas ficam; a que guardava os 53 (a caixa
    /// "Zerar os outros ajustes ao aplicar") continua inteira, e a outra guarda
    /// só o que saiu do neutro — a regra do [`Self::salvar_preset`]. As do
    /// sistema nascem no código e não têm linha no banco: não se atualizam.
    pub fn atualizar_preset(&mut self, id: PresetId, cx: &mut Context<Self>) {
        let ajustes = self.ajustes;
        let Some(preset) = self
            .presets
            .iter_mut()
            .find(|preset| preset.id == id && !preset.is_system)
        else {
            return;
        };
        let inteiro = preset.adjustments.len() == Ajustes::NOMES.len();
        let novos = presets::dos_ajustes(&ajustes, inteiro);
        if novos.is_empty() {
            self.avisar_na_coluna(
                "Não há nenhum ajuste fora do neutro para guardar.",
                true,
                cx,
            );
            return;
        }
        preset.adjustments = novos;
        let nome = preset.name.clone();
        self.guarda_de_presets.salvar(preset.clone());
        self.avisar_na_coluna(
            format!("\"{nome}\" agora guarda os ajustes desta foto."),
            false,
            cx,
        );
        cx.notify();
    }

    /// Uma cópia em "Minhas", com " (cópia)" no nome — o caminho para mexer
    /// numa do sistema, que não se renomeia nem se atualiza.
    ///
    /// 🚨 **A que substitui continua substituindo.** A do operador não guarda
    /// a marca ([`Preset::replaces`] não tem coluna no banco), então a cópia
    /// guarda os 53 já partindo do neutro — a "Zerar os outros ajustes ao
    /// aplicar" do [`Self::salvar_preset`]. Sem isso, a cópia do RecordarFotos
    /// P&B sobre uma sépia somaria e sairia âmbar.
    pub fn duplicar_preset(&mut self, id: PresetId, cx: &mut Context<Self>) {
        let Some(original) = self.presets.iter().find(|p| p.id == id) else {
            return;
        };
        let ajustes = if presets::substitui(original) {
            presets::dos_ajustes(&presets::aplicado(&Ajustes::default(), original), true)
        } else {
            original.adjustments.clone()
        };
        let nome = nome_da_copia(&original.name, &self.presets);
        let copia = Preset::user(nome, ajustes);
        self.guarda_de_presets.salvar(copia.clone());
        self.avisar_na_coluna(format!("\"{}\" entrou em Minhas.", copia.name), false, cx);
        self.presets.push(copia);
        // A cópia escondida numa pasta fechada pareceria não ter saído.
        if self.predefinicoes.ordem.recolhido(Grupo::Minhas) {
            self.alternar_grupo(Grupo::Minhas, cx);
        }
        cx.notify();
    }

    /// A pergunta do site antes de apagar.
    ///
    /// 🚨 **É o único gesto desta coluna que não se desfaz**: a predefinição
    /// não está em foto nenhuma, e nem o `Cmd+Z` nem reabrir a trazem de volta.
    pub(super) fn pedir_para_apagar(
        &mut self,
        id: PresetId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(nome) = self
            .presets
            .iter()
            .find(|p| p.id == id && !p.is_system)
            .map(|p| p.name.clone())
        else {
            return;
        };
        self.predefinicoes.pergunta.abrir((id, nome), window, cx);
        let foco = self
            .predefinicoes
            .foco_da_pergunta
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        window.focus(&foco, cx);
        cx.notify();
    }

    /// "Apagar" (`true`) ou "Cancelar" (`false`).
    pub fn responder_pergunta(
        &mut self,
        apagar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, _)) = self.predefinicoes.pergunta.fechar(window, cx) else {
            return;
        };
        if apagar {
            self.apagar_preset(id, cx);
        }
        cx.notify();
    }

    /// Um aviso no canto, que some sozinho.
    pub(super) fn avisar_na_coluna(
        &mut self,
        texto: impl Into<String>,
        erro: bool,
        cx: &mut Context<Self>,
    ) {
        let numero = self.predefinicoes.proximo_aviso;
        self.predefinicoes.proximo_aviso += 1;
        self.predefinicoes.avisos.push(Aviso {
            texto: texto.into(),
            erro,
            numero,
        });
        cx.spawn(async move |tela, cx| {
            cx.background_executor().timer(DURACAO_DO_AVISO).await;
            tela.update(cx, |tela, cx| {
                tela.predefinicoes.avisos.retain(|a| a.numero != numero);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Apaga uma predefinição do operador.
    ///
    /// ⚠️ **As de sistema não se apagam** — nascem em código a cada listagem, e
    /// apagar mandaria um `DELETE` para um id que a tabela não tem.
    pub fn apagar_preset(&mut self, id: PresetId, cx: &mut Context<Self>) {
        if !self
            .presets
            .iter()
            .any(|preset| preset.id == id && !preset.is_system)
        {
            return;
        }

        self.presets.retain(|preset| preset.id != id);
        // A prévia pode ser justamente a que sumiu.
        self.prever(None, cx);
        self.guarda_de_presets.apagar(id);
        cx.notify();
    }

    // ------------------------------------------------------ reordenar

    /// Os grupos como a coluna os mostra agora.
    pub(super) fn grupos_da_coluna(&self, cx: &App) -> presets::Coluna<'_> {
        let busca = self.busca_de_presets.read(cx).value().to_string();
        presets::da_coluna(&self.presets, &busca, &self.predefinicoes.ordem)
    }

    /// As chaves de um grupo, na ordem da tela.
    ///
    /// 💛 As favoritas reordenam sobre a lista **guardada**: uma chave que a
    /// tela não mostra (predefinição que ainda não carregou) não pode perder o
    /// coração num arrasto — o `reordenacao` do site.
    fn chaves_do_grupo(&self, grupo: Grupo, cx: &App) -> Vec<String> {
        let coluna = self.grupos_da_coluna(cx);
        let lista = match grupo {
            Grupo::Favoritas => return self.predefinicoes.ordem.favoritas.clone(),
            Grupo::Sistema => coluna.sistema,
            Grupo::Minhas => coluna.minhas,
            Grupo::Lrs => coluna.lrs,
        };
        lista.into_iter().map(ordem::chave).collect()
    }

    /// 💛 O coração da linha: põe no fim das favoritas, ou tira delas — e a
    /// raiz leva a lista ao perfil na API.
    pub fn alternar_favorita(&mut self, chave: &str, cx: &mut Context<Self>) {
        self.predefinicoes.ordem.alternar_favorita(chave);
        ordem::gravar(&self.predefinicoes.ordem);
        cx.emit(PedidoDaRevelacao::GuardarFavoritas);
        cx.notify();
    }

    /// 💛 As favoritas na ordem do topo — o que a raiz manda ao perfil.
    pub fn favoritas(&self) -> Vec<String> {
        self.predefinicoes.ordem.favoritas.clone()
    }

    /// 💛 A lista que veio do perfil (ou que acabou de subir para ele): a
    /// coluna passa a mostrá-la, e o JSON daqui a guarda para a próxima
    /// abertura.
    pub fn receber_favoritas(&mut self, chaves: Vec<String>, cx: &mut Context<Self>) {
        let ordem = &mut self.predefinicoes.ordem;
        ordem.definir(Grupo::Favoritas, Some(chaves));
        ordem.favoritas_no_perfil = true;
        ordem::gravar(ordem);
        cx.notify();
    }

    /// 📁 Abre ou fecha a pasta de um grupo, e lembra neste computador.
    pub fn alternar_grupo(&mut self, grupo: Grupo, cx: &mut Context<Self>) {
        self.predefinicoes.ordem.alternar_recolhido(grupo);
        ordem::gravar(&self.predefinicoes.ordem);
        cx.notify();
    }

    /// O grupo está fechado na tela? Com a busca, todos abrem: um resultado
    /// escondido numa pasta fechada pareceria "nenhuma com esse nome".
    pub(super) fn grupo_fechado(&self, grupo: Grupo, cx: &App) -> bool {
        self.predefinicoes.ordem.recolhido(grupo)
            && self.busca_de_presets.read(cx).value().trim().is_empty()
    }

    /// ✔️ A predefinição é o que está na foto: aplicá-la de novo não mudaria
    /// nada. Mexeu num controle que ela define, a marca sai — a foto já não é
    /// "esta predefinição".
    pub(super) fn em_uso(&self, preset: &Preset) -> bool {
        !preset.adjustments.is_empty() && presets::aplicado(&self.ajustes, preset) == self.ajustes
    }

    /// Se as linhas podem ser arrastadas agora.
    pub(super) fn reordenar_ligado(&self, cx: &App) -> bool {
        !self.predefinicoes_travadas() && self.busca_de_presets.read(cx).value().trim().is_empty()
    }

    /// Troca a ordem de um grupo e a guarda. `None` volta à padrão.
    pub fn definir_ordem_dos_presets(
        &mut self,
        grupo: Grupo,
        ids: Option<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        self.predefinicoes.ordem.definir(grupo, ids);
        ordem::gravar(&self.predefinicoes.ordem);
        if grupo == Grupo::Favoritas {
            cx.emit(PedidoDaRevelacao::GuardarFavoritas);
        }
        cx.notify();
    }

    /// ↑ (−1) ou ↓ (+1) com a alça focada.
    pub fn deslocar_preset(
        &mut self,
        grupo: Grupo,
        chave: &str,
        passo: i32,
        cx: &mut Context<Self>,
    ) {
        if !self.reordenar_ligado(cx) {
            return;
        }
        let ids = ordem::deslocar(&self.chaves_do_grupo(grupo, cx), chave, passo);
        self.definir_ordem_dos_presets(grupo, Some(ids), cx);
    }

    pub(super) fn comecar_arrasto_de_preset(
        &mut self,
        grupo: Grupo,
        chave: String,
        cx: &mut Context<Self>,
    ) {
        self.prever(None, cx);
        self.predefinicoes.arrasto = Some(ArrastoEmCurso {
            grupo,
            chave,
            sobre: None,
            depois: false,
        });
        cx.notify();
    }

    /// O arrasto passou sobre uma linha do mesmo grupo.
    pub fn passar_sobre_preset(&mut self, chave: &str, depois: bool, cx: &mut Context<Self>) {
        let Some(arrasto) = self.predefinicoes.arrasto.as_mut() else {
            return;
        };
        if arrasto.sobre.as_deref() != Some(chave) || arrasto.depois != depois {
            arrasto.sobre = Some(chave.to_string());
            arrasto.depois = depois;
            cx.notify();
        }
    }

    /// Solta: a arrastada vai para antes ou depois da linha sob o ponteiro.
    pub fn soltar_preset(&mut self, cx: &mut Context<Self>) {
        let Some(arrasto) = self.predefinicoes.arrasto.take() else {
            return;
        };
        if let Some(alvo) = &arrasto.sobre {
            let ids = ordem::mover_para(
                &self.chaves_do_grupo(arrasto.grupo, cx),
                &arrasto.chave,
                alvo,
                arrasto.depois,
            );
            self.definir_ordem_dos_presets(arrasto.grupo, Some(ids), cx);
        }
        cx.notify();
    }

    /// O arrasto terminou fora de qualquer linha.
    fn terminar_arrasto_de_preset(&mut self, cx: &mut Context<Self>) {
        if self.predefinicoes.arrasto.take().is_some() {
            cx.notify();
        }
    }

    fn foco_da_alca_do_preset(&self, chave: &str, cx: &mut App) -> FocusHandle {
        self.predefinicoes
            .focos
            .borrow_mut()
            .entry(chave.to_string())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// Um gesto do roteiro de depuração (`predefinicoes …`), para fotografar
    /// os estados da coluna. Só grava no catálogo local.
    pub fn seguir_o_roteiro_das_predefinicoes(
        &mut self,
        gesto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (comando, argumento) = gesto.split_once(' ').unwrap_or((gesto, ""));
        let primeira_minha = self.grupos_da_coluna(cx).minhas.first().map(|p| p.id);
        match comando {
            "criar" => self.alternar_formulario_de_preset(window, cx),
            "nome" => self.nome_do_preset.update(cx, |campo, cx| {
                campo.trocar_valor(argumento.to_string(), window, cx)
            }),
            "zerar" => self.alternar_preset_inteiro(cx),
            "renomear" => {
                if let Some(id) = primeira_minha {
                    self.comecar_a_renomear(id, window, cx);
                }
            }
            "apagar" => {
                if let Some(id) = primeira_minha {
                    self.pedir_para_apagar(id, window, cx);
                }
            }
            "arrastar" => {
                let chaves = self.chaves_do_grupo(Grupo::Sistema, cx);
                if let [primeira, segunda, ..] = chaves.as_slice() {
                    let (primeira, segunda) = (primeira.clone(), segunda.clone());
                    self.comecar_arrasto_de_preset(Grupo::Sistema, segunda, cx);
                    self.passar_sobre_preset(&primeira, false, cx);
                }
            }
            "soltar" => self.soltar_preset(cx),
            "fechar" => self.fechar_relatorio(cx),
            // 🧪 Aplica a N-ésima predefinição do sistema à foto aberta (0 é a
            // primeira) — o clique na linha da coluna. Com um nome em vez do
            // número (`predefinicoes aplicar RecordarFotos P&B`), a desse nome:
            // a ordem da coluna muda com as favoritas. O nome procura também nas
            // pastas, como a "LRs" (`predefinicoes aplicar Vinheta Nenhuma`).
            "aplicar" => {
                let sistema = self.grupos_da_coluna(cx).sistema;
                let escolhida = match argumento.parse::<usize>() {
                    Ok(n) => sistema.get(n),
                    Err(_) => sistema.iter().find(|p| p.name == argumento),
                }
                .map(|p| (*p).clone())
                .or_else(|| self.presets.iter().find(|p| p.name == argumento).cloned());
                match escolhida {
                    Some(preset) => self.aplicar_preset(&preset, window, cx),
                    None => eprintln!("[roteiro] predefinicoes: não achei '{argumento}'"),
                }
            }
            // 🧪 O "Atualizar com os ajustes atuais" da do operador com esse nome.
            "atualizar" => {
                let id = self
                    .presets
                    .iter()
                    .find(|p| !p.is_system && p.name == argumento)
                    .map(|p| p.id);
                match id {
                    Some(id) => self.atualizar_preset(id, cx),
                    None => eprintln!("[roteiro] predefinicoes: não achei '{argumento}'"),
                }
            }
            // 🧪 O "Duplicar" da predefinição com esse nome.
            "duplicar" => {
                let id = self
                    .presets
                    .iter()
                    .find(|p| p.name == argumento)
                    .map(|p| p.id);
                match id {
                    Some(id) => self.duplicar_preset(id, cx),
                    None => eprintln!("[roteiro] predefinicoes: não achei '{argumento}'"),
                }
            }
            "responder" => self.responder_pergunta(argumento == "sim", window, cx),
            "ordem" => self.definir_ordem_dos_presets(Grupo::Sistema, None, cx),
            // 💛 O coração da N-ésima do sistema (0 é a primeira).
            "favoritar" => {
                let n: usize = argumento.parse().unwrap_or(0);
                if let Some(chave) = self
                    .grupos_da_coluna(cx)
                    .sistema
                    .get(n)
                    .map(|p| ordem::chave(p))
                {
                    self.alternar_favorita(&chave, cx);
                }
            }
            // O `.zip` entra como no seletor: o que tem dentro.
            "importar" => {
                let arquivos = rfpreset::ler_do_disco(std::path::Path::new(argumento));
                self.importar(arquivos, cx);
            }
            // 📦 `exportar <destino> todas|grupo:<lrs|sistema|minhas|favoritas>|uma:<nome>`
            // grava direto no destino, sem a janela do sistema.
            "exportar" => {
                let (destino, alvo) = argumento.split_once(' ').unwrap_or((argumento, "todas"));
                let alvo = match alvo.split_once(':') {
                    Some(("uma", nome)) => self
                        .presets
                        .iter()
                        .find(|p| p.name == nome)
                        .map(|p| Exportacao::Uma(p.id)),
                    Some(("grupo", grupo)) => {
                        [Grupo::Favoritas, Grupo::Sistema, Grupo::Minhas, Grupo::Lrs]
                            .into_iter()
                            .find(|g| g.chave() == grupo)
                            .map(Exportacao::Grupo)
                    }
                    _ => Some(Exportacao::Todas),
                };
                match alvo.map(|alvo| self.arquivo_da_exportacao(alvo)) {
                    Some(Ok(Some(exportada))) => {
                        match std::fs::write(destino, &exportada.conteudo) {
                            Ok(()) => eprintln!(
                                "[roteiro] predefinicoes: {} em {destino}",
                                exportada.quantas
                            ),
                            Err(erro) => eprintln!("[roteiro] predefinicoes: {erro}"),
                        }
                    }
                    outro => eprintln!("[roteiro] predefinicoes: nada a exportar ({outro:?})"),
                }
            }
            "prever" => {
                let indice: usize = argumento.parse().unwrap_or(0);
                let preset = self
                    .grupos_da_coluna(cx)
                    .sistema
                    .get(indice)
                    .map(|p| (*p).clone());
                self.prever(preset.as_ref(), cx);
            }
            outro => eprintln!("[roteiro] predefinicoes: não sei '{outro}'"),
        }
        cx.notify();
    }

    // ------------------------------------------------------------ desenho

    /// A lista de predefinições — o desenho do site (`painel-presets.tsx`).
    pub(super) fn presets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let presets::Coluna {
            favoritas,
            sistema: do_sistema,
            minhas,
            lrs,
        } = self.grupos_da_coluna(cx);
        let nenhuma =
            favoritas.is_empty() && do_sistema.is_empty() && minhas.is_empty() && lrs.is_empty();
        let buscando = !self.busca_de_presets.read(cx).value().trim().is_empty();
        let apagado = cx.theme().muted_foreground;
        let fraco = apagado.opacity(0.6);
        let sem_minhas = presets::nenhuma_do_usuario(&self.presets);

        let grupos: AnyElement = if nenhuma {
            // ⚠️ **"Nenhuma com esse nome" e "nenhuma ainda" são coisas
            // diferentes**: sem a distinção, quem digitasse errado iria criar a
            // que já existe.
            div()
                .py(px(16.))
                .text_size(crate::tema::letra::em(11.))
                .line_height(relative(1.375))
                .text_color(apagado)
                .child(if buscando {
                    "Nenhuma predefinição com esse nome."
                } else {
                    "Nenhuma predefinição ainda."
                })
                .into_any_element()
        } else {
            // 🔑 **Uma lista só, que rola inteira, com pastas** (dono,
            // 2026-09-30: *"essa listagem de preset tá ruim de usar e muito
            // pouco intuitivo"*). Eram três listas rolando cada uma no seu
            // teto: "Do sistema" mostrava duas das dezoito, sem barra, e nada
            // dizia que havia mais. Agora é o painel do Lightroom — uma rolagem,
            // com barra, e cada grupo uma pasta que abre e fecha.
            //
            // O navegador e a busca continuam fora da rolagem (dono,
            // 2026-09-29), e "Minhas" vem antes das vinte do sistema: as do
            // fotógrafo não somem abaixo da dobra.
            let lista = v_flex()
                .id("lista-de-predefinicoes")
                .gap(px(8.))
                .pr(px(8.))
                .when(!favoritas.is_empty(), |d| {
                    d.child(self.grupo_de_presets(Grupo::Favoritas, &favoritas, None, cx))
                })
                .child(self.grupo_de_presets(
                    Grupo::Minhas,
                    &minhas,
                    sem_minhas.then_some(
                        "Ajuste uma foto e use o + para guardar, ou importe do Lightroom pelo ícone ao lado.",
                    ),
                    cx,
                ))
                .child(self.grupo_de_presets(Grupo::Sistema, &do_sistema, None, cx))
                .when(!lrs.is_empty(), |d| {
                    d.child(self.grupo_de_presets(Grupo::Lrs, &lrs, None, cx))
                })
                .overflow_y_scrollbar();
            // O invólucro é a janela da rolagem: é ele que o teste mede.
            div()
                .id("janela-das-predefinicoes")
                .debug_selector(|| "lista-de-predefinicoes".into())
                .flex_1()
                .min_h(px(0.))
                .mr(px(-8.))
                .child(lista)
                .context_menu(self.menu_da_lista(cx))
                .into_any_element()
        };

        // Uma linha só (dono, 2026-09-30): numa coluna de 224 px o texto de
        // antes eram quatro linhas — quatro predefinições a menos à vista. A
        // regra de cada uma (recomeça do neutro ou soma) está na dica da linha,
        // onde vale para aquela predefinição, e não para "as que definem o
        // visual" em geral.
        const RODAPE: &str = "Mouse em cima mostra; clique aplica; botão direito tem o resto.";

        v_flex()
            .id("predefinicoes")
            .flex_1()
            .min_h(px(0.))
            .gap(px(8.))
            // Soltar fora de uma linha (no vão entre os grupos) só termina.
            .on_drop(cx.listener(|tela, _: &ArrastoDePreset, _window, cx| {
                tela.terminar_arrasto_de_preset(cx);
            }))
            .child(self.barra_das_predefinicoes(cx))
            .children(self.formulario_de_preset(cx))
            .children(self.resultado_da_importacao(cx))
            .child(grupos)
            .child(
                div()
                    .flex_none()
                    .text_size(crate::tema::letra::em(11.))
                    .line_height(relative(1.375))
                    .text_color(fraco)
                    .child(RODAPE),
            )
            // Mede a janela a cada quadro: a pergunta cobre a tela inteira.
            .child({
                let janela = self.predefinicoes.janela.clone();
                canvas(
                    move |_, window, _| janela.set(window.viewport_size()),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_0()
            })
            .children(self.avisos_da_coluna(cx))
    }

    /// "Apagar "X"?" — o `useConfirmacao` do site, no `Dialog` do gpui-kit.
    ///
    /// Desenhada pelo `render` da Revelação (a coluna não tem a `Window`). O
    /// clique fora cancela, como o `onOpenChange` do site; Enter e Esc são do
    /// contexto da pergunta, para o Esc não chegar à raiz e sair da Revelação.
    pub(super) fn pergunta_de_apagar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (_, nome) = self.predefinicoes.pergunta.aberto()?;
        let foco = self.predefinicoes.foco_da_pergunta.clone()?;
        let miolo = div()
            .id("pergunta-da-predefinicao")
            .track_focus(&foco)
            .key_context(CONTEXTO_DA_PERGUNTA)
            .on_action(cx.listener(|tela, _: &ConfirmarPergunta, window, cx| {
                tela.responder_pergunta(true, window, cx);
            }))
            .on_action(cx.listener(|tela, _: &CancelarPergunta, window, cx| {
                tela.responder_pergunta(false, window, cx);
            }))
            .child(crate::dialogo::miolo_da_pergunta(
                "cartao-da-pergunta",
                format!("Apagar \"{nome}\"?"),
                "A predefinição sai da lista.",
                cx,
            ))
            .into_any_element();
        let rodape = crate::dialogo::rodape_da_pergunta(cx)
            .child(
                crate::estilo::botao_contorno("cancelar-apagar", cx)
                    .child("Cancelar")
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.responder_pergunta(false, window, cx);
                    })),
            )
            .child(
                crate::estilo::botao_perigo("confirmar-apagar", cx)
                    .debug_selector(|| "confirmar-apagar".into())
                    .child("Apagar")
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.responder_pergunta(true, window, cx);
                    })),
            )
            .into_any_element();
        crate::dialogo::desenhar_conteudo(
            Some(miolo),
            Some(rodape),
            crate::dialogo::Jeito {
                largura: 448.,
                esc: false,
                veu: true,
                x: false,
            },
            |tela, window, cx| tela.responder_pergunta(false, window, cx),
            window,
            cx,
        )
    }

    /// Os avisos no canto de baixo — o `toast` do site.
    fn avisos_da_coluna(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.predefinicoes.avisos.is_empty() {
            return None;
        }
        let tamanho = self.predefinicoes.janela.get();
        let tema = cx.theme();
        let (cartao, frente, borda, perigo) =
            (tema.popover, tema.foreground, tema.border, tema.danger);
        Some(
            deferred(
                anchored()
                    .anchor(Anchor::BottomRight)
                    .position(point(tamanho.width - px(16.), tamanho.height - px(16.)))
                    .position_mode(AnchoredPositionMode::Window)
                    .child(v_flex().gap(px(8.)).w(px(356.)).children(
                        self.predefinicoes.avisos.iter().map(|aviso| {
                            let (icone, cor) = if aviso.erro {
                                (Icone::CircleAlert, perigo)
                            } else {
                                (Icone::Check, cores::sucesso())
                            };
                            h_flex()
                                .items_start()
                                .gap(px(8.))
                                .p(px(16.))
                                .rounded(crate::tema::canto(8.))
                                .border_1()
                                .border_color(borda)
                                .bg(cartao)
                                .shadow_lg()
                                .text_size(crate::tema::letra::em(13.))
                                .child(Icon::new(icone).size(px(16.)).text_color(cor))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_color(frente)
                                        .child(aviso.texto.clone()),
                                )
                        }),
                    )),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// A busca, o `+` e a importação.
    fn barra_das_predefinicoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let desligada = self.predefinicoes_desligadas();
        let travada = self.predefinicoes_travadas();
        let exportando = self.predefinicoes.exportando.is_some();
        let tema = cx.theme();
        h_flex()
            .items_center()
            .gap(px(4.))
            .child(
                div().flex_1().min_w(px(0.)).child(
                    Styled::h(Input::new(&self.busca_de_presets).xsmall(), px(28.))
                        .text_size(crate::tema::letra::em(12.))
                        .px(px(8.))
                        .rounded(crate::tema::canto(4.))
                        .bg(tema.popover),
                ),
            )
            .child(
                icone_de_botao(
                    "salvar-preset",
                    Icone::Plus,
                    14.,
                    "Salvar os ajustes atuais como predefinição",
                    desligada,
                    cx,
                )
                .when(!desligada, |b| {
                    b.on_click(cx.listener(|tela, _ev, window, cx| {
                        tela.alternar_formulario_de_preset(window, cx);
                    }))
                }),
            )
            .child(
                icone_de_botao(
                    "importar-do-lightroom",
                    Icone::Upload,
                    14.,
                    "Importar predefinições (.rfpreset, .zip, e .xmp e .lrtemplate do Lightroom)",
                    travada,
                    cx,
                )
                .when(!travada, |b| {
                    b.on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.importar_do_lightroom(cx);
                    }))
                }),
            )
            // 📦 Exportar não muda a foto nem a lista: fica ligado mesmo com a
            // coluna travada. Uma ou um grupo, pelo botão direito.
            .child(
                icone_de_botao(
                    "exportar-predefinicoes",
                    Icone::Download,
                    14.,
                    "Exportar todas as predefinições (.zip) — uma ou um grupo, pelo botão direito",
                    exportando,
                    cx,
                )
                .when(!exportando, |b| {
                    b.on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.exportar_predefinicoes(Exportacao::Todas, cx);
                    }))
                }),
            )
    }

    /// O formulário embutido de "salvar como predefinição".
    ///
    /// 🔑 **Embutido, e não diálogo**, como no site: o operador vê o resumo do
    /// que vai guardar enquanto a foto continua à vista.
    fn formulario_de_preset(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.predefinicoes.criando.esta_aberto() {
            return None;
        }
        let tema = cx.theme();
        let (apagado, frente, borda) = (tema.muted_foreground, tema.foreground, tema.border);
        let (cartao, fundo, realce) = (tema.popover, tema.background, tema.muted);
        let marca = if tema.mode.is_dark() {
            cores::aceso()
        } else {
            rgb(0xe17100).into()
        };
        let inteiro = self.preset_inteiro;
        let alterados = presets::dos_ajustes(&self.ajustes, false);
        let nome = self.nome_do_preset.read(cx).value().to_string();
        let pode =
            !self.predefinicoes_travadas() && presets::pode_salvar(&nome, alterados.len(), inteiro);

        Some(
            div()
                .rounded(crate::tema::canto(4.))
                .border_1()
                .border_color(borda)
                .bg(cartao)
                .p(px(8.))
                .on_action(cx.listener(|tela, _: &Escape, window, cx| {
                    tela.cancelar_formulario_de_preset(window, cx);
                }))
                .child(
                    Styled::h(Input::new(&self.nome_do_preset).xsmall(), px(28.))
                        .text_size(crate::tema::letra::em(12.))
                        .px(px(8.))
                        .rounded(crate::tema::canto(4.))
                        .bg(fundo),
                )
                .child(
                    div()
                        .id("preset-inteiro")
                        .mt(px(8.))
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .rounded(crate::tema::canto(4.))
                        .px(px(2.))
                        .py(px(4.))
                        .text_size(crate::tema::letra::em(11.))
                        .line_height(relative(1.375))
                        .text_color(frente.opacity(0.9))
                        .cursor_pointer()
                        .hover(move |s| s.bg(realce))
                        .child(
                            div().mt(px(1.)).flex_none().child(
                                Icon::new(if inteiro {
                                    Icone::SquareCheck
                                } else {
                                    Icone::Square
                                })
                                .size(px(14.))
                                .text_color(if inteiro { marca } else { apagado }),
                            ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .child("Zerar os outros ajustes ao aplicar")
                                .child(div().text_color(apagado).child(
                                    "Para um visual inteiro. Sem isto, ela soma com o que já estiver na foto.",
                                )),
                        )
                        .on_click(cx.listener(|tela, _ev, _window, cx| {
                            tela.alternar_preset_inteiro(cx);
                        })),
                )
                .child(
                    div()
                        .mt(px(6.))
                        .text_size(crate::tema::letra::em(11.))
                        .line_height(relative(1.375))
                        .text_color(apagado)
                        .child(presets::o_que_guarda(&alterados, inteiro)),
                )
                .child(
                    h_flex()
                        .mt(px(8.))
                        .justify_end()
                        .gap(px(4.))
                        .child(
                            botao_pequeno("cancelar-preset", "Cancelar", false, false, cx).on_click(
                                cx.listener(|tela, _ev, window, cx| {
                                    tela.cancelar_formulario_de_preset(window, cx);
                                }),
                            ),
                        )
                        .child(
                            botao_pequeno("confirmar-preset", "Salvar", true, !pode, cx).when(
                                pode,
                                |b| {
                                    b.on_click(cx.listener(|tela, _ev, window, cx| {
                                        tela.salvar_preset(window, cx);
                                    }))
                                },
                            ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// O que a última importação aproveitou — e o que não.
    ///
    /// 🔑 **Fica na coluna, e não num aviso que se fecha sozinho**: a lista de
    /// recursos ignorados é a resposta para "por que este preset mudou tão
    /// pouco?" — pergunta que só aparece depois de aplicar o primeiro.
    fn resultado_da_importacao(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let relatorio = self.relatorio.as_ref()?;
        let tema = cx.theme();
        let (apagado, frente, borda, cartao) = (
            tema.muted_foreground,
            tema.foreground,
            tema.border,
            tema.popover,
        );
        let ignorados = relatorio.linhas_dos_ignorados();

        Some(
            div()
                .rounded(crate::tema::canto(4.))
                .border_1()
                .border_color(borda)
                .bg(cartao)
                .p(px(8.))
                .text_size(crate::tema::letra::em(11.))
                .line_height(relative(1.375))
                .text_color(apagado)
                .child(
                    h_flex()
                        .items_start()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_color(frente)
                                .child(relatorio.resumo()),
                        )
                        .child(
                            div()
                                .id("fechar-relatorio")
                                .cursor_pointer()
                                .child(Icon::new(Icone::X).size(px(12.)).text_color(apagado))
                                .tooltip(|window, cx| {
                                    Tooltip::new("Fechar o resultado").build(window, cx)
                                })
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.fechar_relatorio(cx);
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .mt(px(4.))
                        .gap(px(2.))
                        .children(relatorio.linhas().into_iter().map(|l| div().child(l))),
                )
                .when(!ignorados.is_empty(), |d| {
                    d.child(
                        div()
                            .mt(px(6.))
                            .border_t_1()
                            .border_color(borda)
                            .pt(px(6.))
                            .child(
                                "Recursos que este motor ainda não tem, e quantos arquivos usavam:",
                            )
                            .child(
                                v_flex()
                                    .mt(px(2.))
                                    .children(ignorados.into_iter().map(|l| div().child(l))),
                            ),
                    )
                })
                .into_any_element(),
        )
    }

    /// Um bloco da lista: a pasta (título e contagem, clicável para abrir e
    /// fechar), o "ordem padrão" e as linhas.
    fn grupo_de_presets(
        &self,
        grupo: Grupo,
        lista: &[&Preset],
        vazio: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);
        let fraco = apagado.opacity(0.6);
        let titulo = match grupo {
            Grupo::Favoritas => "FAVORITAS",
            Grupo::Sistema => "DO SISTEMA",
            Grupo::Minhas => "MINHAS",
            Grupo::Lrs => "LRS",
        };
        let fechado = self.grupo_fechado(grupo, cx);
        // As favoritas não têm "ordem padrão": a lista guardada é a escolha.
        let reordenada = !fechado
            && grupo != Grupo::Favoritas
            && !self.predefinicoes.ordem.do_grupo(grupo).is_empty();

        // 📁 A pasta: o título inteiro é o alvo do clique, com a seta de quem
        // abre e fecha — o triângulo do painel de predefinições do Lightroom.
        let pasta = h_flex()
            .id(SharedString::from(format!("pasta-{titulo}")))
            .debug_selector(move || format!("pasta-{titulo}"))
            .flex_1()
            .min_w(px(0.))
            .items_center()
            .gap(px(4.))
            .py(px(2.))
            .rounded(crate::tema::canto(4.))
            .cursor_pointer()
            .hover(move |s| s.text_color(frente))
            .child(
                Icon::new(if fechado {
                    Icone::ChevronRight
                } else {
                    Icone::ChevronDown
                })
                .size(px(12.)),
            )
            .child(div().flex_none().child(titulo))
            .child(
                div()
                    .flex_none()
                    .text_color(fraco)
                    .child(SharedString::from(lista.len().to_string())),
            )
            .tooltip(move |window, cx| {
                Tooltip::new(if fechado {
                    "Abrir o grupo"
                } else {
                    "Fechar o grupo"
                })
                .build(window, cx)
            })
            .on_click(
                cx.listener(move |tela, ev: &gpui_kit::ClickEvent, _window, cx| {
                    if ev.standard_click() {
                        tela.alternar_grupo(grupo, cx);
                    }
                }),
            )
            // O botão direito na pasta: o "Exportar grupo…" do Lightroom.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |tela, _ev, _window, _cx| {
                    tela.predefinicoes.alvo_do_menu = Some(AlvoDoMenu::Grupo(grupo));
                }),
            );

        let cabeca = h_flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .text_size(crate::tema::letra::em(10.))
            .line_height(px(15.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(apagado)
            .child(pasta)
            .when(reordenada, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("ordem-padrao-{titulo}")))
                        .flex_none()
                        .px(px(4.))
                        .rounded(crate::tema::canto(4.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(fraco)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(frente).bg(realce))
                        .child("ordem padrão")
                        .tooltip(|window, cx| {
                            Tooltip::new("Desfaz a reordenação deste grupo").build(window, cx)
                        })
                        .on_click(cx.listener(move |tela, _ev, _window, cx| {
                            tela.definir_ordem_dos_presets(grupo, None, cx);
                        })),
                )
            });

        let corpo: Option<AnyElement> = match (fechado, lista.is_empty(), vazio) {
            (true, _, _) => None,
            (false, true, Some(texto)) => Some(
                div()
                    .px(px(4.))
                    .pb(px(4.))
                    .text_size(crate::tema::letra::em(11.))
                    .line_height(relative(1.375))
                    .text_color(fraco)
                    .child(texto)
                    .into_any_element(),
            ),
            _ => Some(
                v_flex()
                    .gap(px(1.))
                    .children(lista.iter().map(|preset| {
                        if self.predefinicoes.renomeando.aberto() == Some(&preset.id) {
                            self.nome_em_edicao(preset.id, cx)
                        } else {
                            self.linha_de_preset(preset, grupo, cx)
                        }
                    }))
                    .into_any_element(),
            ),
        };

        v_flex()
            .debug_selector(move || format!("grupo-{titulo}"))
            .flex_none()
            .gap(px(2.))
            .child(cabeca)
            .children(corpo)
            .into_any_element()
    }

    /// Uma predefinição na lista.
    ///
    /// 🔑 O id do elemento é o **id do preset**, e não a posição: dois com o
    /// mesmo nome são possíveis, e id por posição faria o GPUI confundir o
    /// estado de dois botões quando a lista mudasse.
    ///
    /// 🚨 **O clique mora no nome, e não na linha**: com o `on_click` na linha
    /// inteira, clicar na lixeira aplicaria a predefinição antes da pergunta.
    ///
    /// ✔️ A que está na foto fica acesa, e o botão direito abre o menu da
    /// linha — aplicar, favoritar, renomear, apagar —, como no Lightroom: os
    /// ícones que só aparecem no passar do mouse ninguém descobria.
    fn linha_de_preset(&self, preset: &Preset, grupo: Grupo, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);
        let fraco = apagado.opacity(0.6);
        let travada = self.predefinicoes_travadas();
        let chave = ordem::chave(preset);
        let favorita = self.predefinicoes.ordem.eh_favorita(&chave);
        let reordena = self.reordenar_ligado(cx);
        let arrasto = self.predefinicoes.arrasto.as_ref();
        let arrastando = arrasto.is_some_and(|a| a.chave == chave);
        let alvo = arrasto
            .filter(|a| a.chave != chave && a.sobre.as_deref() == Some(chave.as_str()))
            .map(|a| a.depois);
        let marca_do_grupo = SharedString::from(format!("linha-{chave}"));
        let nome = SharedString::from(preset.name.clone());
        let coracao = self.coracao(chave.clone(), favorita, &nome, &marca_do_grupo, cx);
        let em_uso = self.em_uso(preset);
        // O número de controles saiu da linha (era um "24" sem legenda ao lado
        // do nome) e ficou aqui, com a regra e o que ela move.
        let dica = SharedString::from(format!(
            "{}{}\n{}\n{} controles",
            if em_uso { "✓ Na foto agora · " } else { "" },
            presets::dica_do_nome(preset),
            presets::regra_da_linha(preset),
            presets::quantos_campos(preset),
        ));
        let para_prever = preset.clone();
        let para_aplicar = preset.clone();
        let id = preset.id;

        let mut linha = div()
            .id(SharedString::from(format!("preset-{id}")))
            .group(marca_do_grupo.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(crate::tema::canto(4.))
            .px(px(4.))
            .when(em_uso, |d| d.bg(realce))
            .hover(move |s| s.bg(realce))
            .when(arrastando, |d| d.opacity(0.4))
            .children(em_uso.then(|| {
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(5.))
                    .bottom(px(5.))
                    .w(px(2.))
                    .rounded(crate::tema::canto(1.))
                    .bg(cores::aceso())
            }))
            .on_hover(cx.listener(move |tela, sobre: &bool, _window, cx| {
                if *sobre && !tela.predefinicoes_travadas() {
                    tela.prever(Some(&para_prever), cx);
                } else if !*sobre {
                    tela.prever(None, cx);
                }
            }))
            .children(alvo.map(|depois| {
                div()
                    .absolute()
                    .left(px(4.))
                    .right(px(4.))
                    .h(px(2.))
                    .rounded(crate::tema::canto(1.))
                    .bg(cores::aceso())
                    .when(depois, |d| d.bottom(px(-1.)))
                    .when(!depois, |d| d.top(px(-1.)))
            }));

        if reordena {
            let esta = cx.entity().downgrade();
            let chave_do_arrasto = chave.clone();
            let chave_de_cima = chave.clone();
            let chave_de_soltar = chave.clone();
            linha = linha
                .on_drag(
                    ArrastoDePreset {
                        grupo,
                        nome: nome.clone(),
                    },
                    move |valor, _posicao, _window, cx| {
                        let chave = chave_do_arrasto.clone();
                        esta.update(cx, |tela, cx| {
                            tela.comecar_arrasto_de_preset(valor.grupo, chave, cx)
                        })
                        .ok();
                        let nome = valor.nome.clone();
                        cx.new(|_| FantasmaDoPreset { nome })
                    },
                )
                .on_drag_move(cx.listener(
                    move |tela, evento: &DragMoveEvent<ArrastoDePreset>, _window, cx| {
                        if evento.drag(cx).grupo != grupo
                            || !evento.bounds.contains(&evento.event.position)
                        {
                            return;
                        }
                        let depois = evento.event.position.y > evento.bounds.center().y;
                        tela.passar_sobre_preset(&chave_de_cima, depois, cx);
                    },
                ))
                .on_drop(
                    cx.listener(move |tela, valor: &ArrastoDePreset, _window, cx| {
                        if valor.grupo == grupo {
                            // O soltar vale onde o ponteiro está, mesmo que o último
                            // movimento não tenha sido registrado nesta linha.
                            let depois = tela
                                .predefinicoes
                                .arrasto
                                .as_ref()
                                .filter(|a| a.sobre.as_deref() == Some(chave_de_soltar.as_str()))
                                .is_some_and(|a| a.depois);
                            tela.passar_sobre_preset(&chave_de_soltar, depois, cx);
                            tela.soltar_preset(cx);
                        } else {
                            tela.terminar_arrasto_de_preset(cx);
                        }
                    }),
                )
                // Soltar fora da coluna não chega a `on_drop` nenhum.
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|tela, _ev, _window, cx| {
                        if tela.predefinicoes.arrasto.is_some() {
                            let esta = cx.entity();
                            cx.defer(move |cx| {
                                esta.update(cx, |tela, cx| tela.terminar_arrasto_de_preset(cx));
                            });
                        }
                    }),
                );

            let foco = self.foco_da_alca_do_preset(&chave, cx);
            let (chave_sobe, chave_desce) = (chave.clone(), chave.clone());
            linha = linha.child(
                div()
                    .id(SharedString::from(format!("alca-{chave}")))
                    .track_focus(&foco)
                    .key_context(CONTEXTO_DA_ALCA)
                    .ml(px(-2.))
                    .flex_none()
                    .cursor(CursorStyle::OpenHand)
                    .text_color(fraco)
                    .opacity(0.)
                    .group_hover(marca_do_grupo.clone(), |s| s.opacity(1.))
                    .focus(|s| s.opacity(1.))
                    .hover(move |s| s.text_color(frente))
                    .child(Icon::new(Icone::GripVertical).size(px(12.)))
                    .tooltip(|window, cx| {
                        Tooltip::new("Arraste para reordenar — ou use ↑ e ↓").build(window, cx)
                    })
                    .on_action(
                        cx.listener(move |tela, _: &SubirPredefinicao, _window, cx| {
                            tela.deslocar_preset(grupo, &chave_sobe, -1, cx);
                        }),
                    )
                    .on_action(
                        cx.listener(move |tela, _: &DescerPredefinicao, _window, cx| {
                            tela.deslocar_preset(grupo, &chave_desce, 1, cx);
                        }),
                    ),
            );
        }

        linha
            .child(
                div()
                    .id(SharedString::from(format!("aplicar-{id}")))
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .py(px(6.))
                    .text_size(crate::tema::letra::em(12.))
                    .line_height(px(16.))
                    .text_color(if travada { fraco } else { frente })
                    .when(em_uso, |d| d.font_weight(FontWeight::MEDIUM))
                    .when(!travada, |d| d.cursor_pointer())
                    .child(nome.clone())
                    .tooltip(move |window, cx| Tooltip::new(dica.clone()).build(window, cx))
                    .when(!travada, |d| {
                        d.on_click(cx.listener(
                            move |tela, ev: &gpui_kit::ClickEvent, window, cx| {
                                // 🚨 O GPUI chama o `on_click` também no botão
                                // direito: sem isto, abrir o menu da linha
                                // aplicava a predefinição.
                                if !ev.standard_click() {
                                    return;
                                }
                                // 🚨 A prévia sai **antes** de aplicar: se ficasse, o
                                // resultado seria o preset por cima dele mesmo.
                                tela.prever(None, cx);
                                tela.aplicar_preset(&para_aplicar, window, cx);
                            },
                        ))
                    }),
            )
            // O coração é sempre o último: a coluna dele fica alinhada entre
            // as do sistema e as minhas, que têm lápis e lixeira antes.
            .children(self.acoes_do_preset(preset, &marca_do_grupo, travada, cx))
            .child(coracao)
            // O botão direito anota a linha; o menu da lista inteira lê.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |tela, _ev, _window, _cx| {
                    tela.predefinicoes.alvo_do_menu = Some(AlvoDoMenu::Preset(id));
                }),
            )
            .into_any_element()
    }

    /// O menu do botão direito da lista.
    ///
    /// 🔑 **Um menu para a lista inteira**, como na tira e nas guias: a linha
    /// do botão direito anota quem foi clicada ([`Predefinicoes::alvo_do_menu`])
    /// e o menu, montado depois do evento, lê. Preso a cada linha dentro da
    /// rolagem, o menu do kit era montado e não ficava aberto.
    fn menu_da_lista(
        &self,
        cx: &mut Context<Self>,
    ) -> impl Fn(
        gpui_kit::component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui_kit::component::menu::PopupMenu>,
    ) -> gpui_kit::component::menu::PopupMenu
           + 'static {
        let esta = cx.entity().downgrade();
        move |menu, window, cx| {
            // 🚨 O foco volta a quem o tinha quando o menu fechar — o mesmo
            // cuidado do menu da tira (sem ele, as setas da Revelação param).
            let menu = match window.focused(cx) {
                Some(antes) => menu.action_context(antes),
                None => menu,
            };
            let Ok((alvo, exportando)) = esta.update(cx, |tela, _cx| {
                (
                    tela.predefinicoes.alvo_do_menu.take(),
                    tela.predefinicoes.exportando.is_some(),
                )
            }) else {
                return menu;
            };
            let id = match alvo {
                Some(AlvoDoMenu::Preset(id)) => id,
                Some(AlvoDoMenu::Grupo(grupo)) => {
                    return Self::menu_do_grupo(menu, grupo, exportando, esta.clone(), cx)
                }
                // 📦 No vão da lista, o gesto que vale para a coluna inteira.
                None => {
                    let para_exportar = esta.clone();
                    return menu.item(
                        PopupMenuItem::new("Exportar todas as predefinições…")
                            .disabled(exportando)
                            .on_click(move |_ev, _window, cx| {
                                let _ = para_exportar.update(cx, |tela, cx| {
                                    tela.exportar_predefinicoes(Exportacao::Todas, cx);
                                });
                            }),
                    );
                }
            };
            let Some((preset, favorita, travada)) = esta
                .update(cx, |tela, _cx| {
                    let preset = tela.presets.iter().find(|p| p.id == id)?.clone();
                    let favorita = tela.predefinicoes.ordem.eh_favorita(&ordem::chave(&preset));
                    Some((preset, favorita, tela.predefinicoes_travadas()))
                })
                .ok()
                .flatten()
            else {
                return menu;
            };
            let (para_aplicar, para_atualizar) = (esta.clone(), esta.clone());
            let (para_duplicar, para_favoritar) = (esta.clone(), esta.clone());
            let (para_exportar, para_exportar_o_grupo) = (esta.clone(), esta.clone());
            let origem = Grupo::de(&preset);
            let (para_renomear, para_apagar) = (esta.clone(), esta.clone());
            let chave = ordem::chave(&preset);
            let id = preset.id;
            let sistema = preset.is_system;
            let menu = menu
                .label(preset.name.clone())
                .separator()
                .item(
                    PopupMenuItem::new("Aplicar nesta foto")
                        .disabled(travada)
                        .on_click(move |_ev, window, cx| {
                            let _ = para_aplicar.update(cx, |tela, cx| {
                                tela.prever(None, cx);
                                tela.aplicar_preset(&preset, window, cx);
                            });
                        }),
                )
                // As do sistema nascem no código: à vista, mas desligada, como
                // no Lightroom — some, e o operador procuraria onde ela foi.
                .item(
                    PopupMenuItem::new("Atualizar com os ajustes atuais")
                        .disabled(travada || sistema)
                        .on_click(move |_ev, _window, cx| {
                            let _ = para_atualizar.update(cx, |tela, cx| {
                                tela.prever(None, cx);
                                tela.atualizar_preset(id, cx);
                            });
                        }),
                )
                // Não depende da foto: duplica mesmo com a coluna travada.
                .item(
                    PopupMenuItem::new("Duplicar").on_click(move |_ev, _window, cx| {
                        let _ = para_duplicar.update(cx, |tela, cx| {
                            tela.duplicar_preset(id, cx);
                        });
                    }),
                )
                .item(
                    PopupMenuItem::new(if favorita {
                        "Tirar das favoritas"
                    } else {
                        "Pôr nas favoritas"
                    })
                    .on_click(move |_ev, _window, cx| {
                        let _ = para_favoritar.update(cx, |tela, cx| {
                            tela.alternar_favorita(&chave, cx);
                        });
                    }),
                )
                // 📦 Exportar vale para todas, inclusive as do sistema: é o
                // jeito de levar uma delas para outro balcão.
                .separator()
                .item(
                    PopupMenuItem::new("Exportar…")
                        .disabled(exportando)
                        .on_click(move |_ev, _window, cx| {
                            let _ = para_exportar.update(cx, |tela, cx| {
                                tela.exportar_predefinicoes(Exportacao::Uma(id), cx);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new(format!(
                        "Exportar o grupo \"{}\"…",
                        rotulo_do_grupo(origem)
                    ))
                    .disabled(exportando)
                    .on_click(move |_ev, _window, cx| {
                        let _ = para_exportar_o_grupo.update(cx, |tela, cx| {
                            tela.exportar_predefinicoes(Exportacao::Grupo(origem), cx);
                        });
                    }),
                );
            // As do sistema não têm linha no banco: nem renomear nem apagar.
            if sistema {
                return menu;
            }
            menu.separator()
                .item(PopupMenuItem::new("Renomear…").disabled(travada).on_click(
                    move |_ev, window, cx| {
                        let _ = para_renomear.update(cx, |tela, cx| {
                            tela.comecar_a_renomear(id, window, cx);
                        });
                    },
                ))
                .item(PopupMenuItem::new("Apagar…").disabled(travada).on_click(
                    move |_ev, window, cx| {
                        let _ = para_apagar.update(cx, |tela, cx| {
                            tela.pedir_para_apagar(id, window, cx);
                        });
                    },
                ))
        }
    }

    /// O menu do botão direito numa pasta — o "Exportar grupo…" do Lightroom.
    fn menu_do_grupo(
        menu: gpui_kit::component::menu::PopupMenu,
        grupo: Grupo,
        exportando: bool,
        esta: gpui_kit::WeakEntity<Self>,
        cx: &mut App,
    ) -> gpui_kit::component::menu::PopupMenu {
        let (quantas, fechado) = esta
            .update(cx, |tela, cx| {
                let coluna = presets::da_coluna(&tela.presets, "", &tela.predefinicoes.ordem);
                let quantas = match grupo {
                    Grupo::Favoritas => coluna.favoritas.len(),
                    Grupo::Sistema => coluna.sistema.len(),
                    Grupo::Minhas => coluna.minhas.len(),
                    Grupo::Lrs => coluna.lrs.len(),
                };
                (quantas, tela.grupo_fechado(grupo, cx))
            })
            .unwrap_or((0, false));
        let (para_exportar, para_todas, para_abrir) = (esta.clone(), esta.clone(), esta);
        menu.label(format!(
            "{} · {}",
            rotulo_do_grupo(grupo),
            quantas_predefinicoes(quantas)
        ))
        .separator()
        .item(
            PopupMenuItem::new("Exportar o grupo…")
                .disabled(exportando || quantas == 0)
                .on_click(move |_ev, _window, cx| {
                    let _ = para_exportar.update(cx, |tela, cx| {
                        tela.exportar_predefinicoes(Exportacao::Grupo(grupo), cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Exportar todas as predefinições…")
                .disabled(exportando)
                .on_click(move |_ev, _window, cx| {
                    let _ = para_todas.update(cx, |tela, cx| {
                        tela.exportar_predefinicoes(Exportacao::Todas, cx);
                    });
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(if fechado {
                "Abrir o grupo"
            } else {
                "Fechar o grupo"
            })
            .on_click(move |_ev, _window, cx| {
                let _ = para_abrir.update(cx, |tela, cx| tela.alternar_grupo(grupo, cx));
            }),
        )
    }

    /// 💛 O coração da linha — o `Heart` do site: aceso fica sempre à vista;
    /// apagado, só no passar do mouse (vinte corações vazios encheriam a coluna).
    fn coracao(
        &self,
        chave: String,
        favorita: bool,
        nome: &SharedString,
        linha: &SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);
        let dica = SharedString::from(if favorita {
            format!("Tirar {nome} das favoritas")
        } else {
            format!("Pôr {nome} nas favoritas — elas ficam no topo")
        });
        let seletor = format!("favorita-{chave}");
        div()
            .id(SharedString::from(seletor.clone()))
            .debug_selector(move || seletor)
            .flex_none()
            .p(px(4.))
            .rounded(crate::tema::canto(4.))
            .cursor_pointer()
            // Um `hover` só: o GPUI recusa o segundo ("hover style already set").
            .hover(move |s| {
                let s = s.bg(realce);
                if favorita {
                    s
                } else {
                    s.text_color(frente)
                }
            })
            .when(favorita, |d| {
                d.text_color(rgb(0xf43f5e))
                    .child(Icon::new(Icone::HeartCheio).size(px(12.)))
            })
            .when(!favorita, |d| {
                d.text_color(apagado)
                    .opacity(0.)
                    .group_hover(linha.clone(), |s| s.opacity(1.))
                    .child(Icon::new(Icone::Heart).size(px(12.)))
            })
            .tooltip(move |window, cx| Tooltip::new(dica.clone()).build(window, cx))
            .on_click(
                cx.listener(move |tela, ev: &gpui_kit::ClickEvent, _window, cx| {
                    if ev.standard_click() {
                        tela.alternar_favorita(&chave, cx);
                    }
                }),
            )
            .into_any_element()
    }

    /// Renomear e apagar — só para as do operador, e só ao passar o mouse.
    ///
    /// Uma do sistema não tem linha no banco para apagar: sumiria da tela e
    /// voltaria na abertura seguinte.
    fn acoes_do_preset(
        &self,
        preset: &Preset,
        grupo: &SharedString,
        travada: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if preset.is_system {
            return Vec::new();
        }
        let id = preset.id;
        let discreto = |botao: Button| {
            botao.opacity(0.).group_hover(grupo.clone(), move |s| {
                s.opacity(if travada { 0.4 } else { 1. })
            })
        };

        vec![
            discreto(icone_de_botao(
                SharedString::from(format!("renomear-{id}")),
                Icone::Pencil,
                12.,
                format!("Renomear {}", preset.name),
                false,
                cx,
            ))
            .when(!travada, |b| {
                b.on_click(cx.listener(move |tela, _ev, window, cx| {
                    tela.comecar_a_renomear(id, window, cx);
                }))
            })
            .into_any_element(),
            discreto(icone_de_botao(
                SharedString::from(format!("apagar-{id}")),
                Icone::Trash2,
                12.,
                format!("Apagar {}", preset.name),
                false,
                cx,
            ))
            .debug_selector({
                let nome = preset.name.clone();
                move || format!("lixeira-{nome}")
            })
            .when(!travada, |b| {
                b.on_click(cx.listener(move |tela, _ev, window, cx| {
                    tela.pedir_para_apagar(id, window, cx);
                }))
            })
            .into_any_element(),
        ]
    }

    /// A linha em edição: o campo, ✓ e ✕. Enter confirma, Esc cancela.
    fn nome_em_edicao(&self, id: PresetId, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        h_flex()
            .gap(px(4.))
            .px(px(4.))
            .py(px(2.))
            .on_action(cx.listener(|tela, _: &Escape, window, cx| tela.cancelar_renome(window, cx)))
            .child(
                div().flex_1().min_w(px(0.)).child(
                    Styled::h(Input::new(&self.renome_do_preset).xsmall(), px(28.))
                        .text_size(crate::tema::letra::em(12.))
                        .px(px(8.))
                        .rounded(crate::tema::canto(4.))
                        .bg(tema.background),
                ),
            )
            .child(
                icone_de_botao(
                    "confirmar-nome",
                    Icone::Check,
                    14.,
                    "Confirmar o nome",
                    false,
                    cx,
                )
                .on_click(cx.listener(move |tela, _ev, _window, cx| {
                    let nome = tela.renome_do_preset.read(cx).value().to_string();
                    tela.renomear_preset(id, nome, cx);
                })),
            )
            .child(
                icone_de_botao("cancelar-nome", Icone::X, 14., "Cancelar", false, cx).on_click(
                    cx.listener(|tela, _ev, window, cx| tela.cancelar_renome(window, cx)),
                ),
            )
            .into_any_element()
    }
}

/// O `IconeBotao` do site (`rounded p-1`, apagado): o `Button` fantasma do
/// kit só com o ícone, com a dica e o desligado que **não clica**.
fn icone_de_botao(
    id: impl Into<SharedString>,
    icone: Icone,
    lado: f32,
    rotulo: impl Into<SharedString>,
    desligado: bool,
    cx: &App,
) -> Button {
    Button::new(id.into())
        .ghost()
        .xsmall()
        .flex_none()
        .size(px(lado + 8.))
        .px(px(0.))
        .rounded(crate::tema::canto(4.))
        .text_color(cx.theme().muted_foreground)
        .child(Icon::new(icone).size(px(lado)))
        .tooltip(rotulo.into())
        .disabled(desligado)
}

/// O `BotaoPequeno` do site: "Cancelar" apagado, "Salvar" em âmbar — o
/// `Button` do kit em `text-xs`, com a variante âmbar do tema.
fn botao_pequeno(
    id: &'static str,
    rotulo: &'static str,
    destaque: bool,
    desligado: bool,
    cx: &App,
) -> Button {
    Button::new(id)
        .xsmall()
        .map(|b| {
            if destaque {
                b.custom(crate::tema::botao_aceso(cx))
            } else {
                b.custom(
                    ButtonCustomVariant::new(cx)
                        .color(cx.theme().muted)
                        .foreground(cx.theme().foreground.opacity(0.9))
                        .hover(cx.theme().muted.opacity(0.7))
                        .active(cx.theme().muted.opacity(0.7)),
                )
            }
        })
        .rounded(crate::tema::canto(4.))
        .px(px(8.))
        .text_size(crate::tema::letra::em(12.))
        .child(rotulo)
        .disabled(desligado)
}

/// "1 predefinição", "26 predefinições".
fn quantas_predefinicoes(n: usize) -> String {
    if n == 1 {
        "1 predefinição".into()
    } else {
        format!("{n} predefinições")
    }
}
