//! A coluna das predefinições — o `painel-presets.tsx` do site, com a prévia e
//! o aplicar do `editor.tsx`.
//!
//! | Gesto | Faz |
//! |---|---|
//! | passar o mouse numa linha | mostra na foto — partindo do neutro quando ela substitui |
//! | clicar no nome | aplica: um passo de histórico, gravado na hora |
//! | `+` | abre o formulário embutido: Enter salva, Esc cancela |
//! | ícone de envio | importa `.lrtemplate`, `.xmp` (Lightroom e darktable) e `.dtstyle` |
//! | lápis / lixeira (ao passar o mouse) | renomeia no lugar (Enter/Esc/✓/✕) · apaga depois de perguntar |
//! | arrastar a linha | reordena dentro do grupo; ↑ ↓ com a alça focada; "ordem padrão" desfaz |
//!
//! 🔑 **Reordenar fica desligado durante a busca**, como no site: a lista
//! filtrada esconde os vizinhos, e soltar "entre" duas que aparecem juntas poria
//! a predefinição no meio de outras cinco.
//!
//! A regra de negócio — somar ou substituir, o resumo, a ordem, as traduções —
//! mora em [`crate::revelacao::presets`] e [`crate::revelacao::lightroom`], sem
//! tela; aqui é só o desenho e os gestos.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

#[cfg(test)]
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Preset, PresetId};
use gpui::Div;
use gpui::{
    actions, anchored, canvas, deferred, div, point, prelude::*, px, relative, rgb,
    AnchoredPositionMode, AnyElement, App, Context, Corner, CursorStyle, DragMoveEvent, Entity,
    FocusHandle, FontWeight, HighlightStyle, Hsla, KeyBinding, MouseButton, Pixels, SharedString,
    Size, Stateful, StyledText, Subscription, Window,
};
use gpui_component::input::{Escape, Input, InputEvent, InputState};
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, Sizable};

use super::Revelacao;
use crate::recursos::Icone;
use crate::revelacao::lightroom::{self, Arquivo};
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
impl gpui::Global for AtalhosLigados {}

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
    /// O formulário de "salvar como predefinição" está aberto.
    pub criando: bool,
    /// A linha que virou campo de nome.
    pub renomeando: Option<PresetId>,
    /// A ordem escolhida neste computador.
    pub ordem: ordem::Ordem,
    /// O arrasto em curso: de onde saiu, e sobre qual linha (antes ou depois).
    pub arrasto: Option<ArrastoEmCurso>,
    /// O foco da alça de cada linha, pela chave da ordem. `RefCell` porque as
    /// alças nascem no desenho, que só tem `&self`.
    focos: RefCell<HashMap<String, FocusHandle>>,
    /// A predefinição que espera o "Apagar" ou o "Cancelar".
    pub pergunta: Option<(PresetId, String)>,
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
            criando: false,
            renomeando: None,
            ordem: ordem::ler(),
            arrasto: None,
            focos: RefCell::new(HashMap::new()),
            pergunta: None,
            foco_da_pergunta: None,
            avisos: Vec::new(),
            proximo_aviso: 0,
            janela: Rc::new(Cell::new(Size::default())),
        }
    }
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
            .rounded(px(4.))
            .bg(cx.theme().muted)
            .opacity(0.85)
            .text_size(px(12.))
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
                if let (InputEvent::PressEnter { .. }, Some(id)) =
                    (evento, tela.predefinicoes.renomeando)
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
        match &self.previa {
            Some(preset) => presets::aplicado(&self.ajustes, preset),
            None => self.ajustes,
        }
    }

    /// Mostra (ou tira) a prévia de uma predefinição.
    ///
    /// Sem mudança não há o que redesenhar — pedir à GPU o mesmo quadro a cada
    /// movimento do ponteiro sobre a mesma linha seria trabalho por nada.
    pub fn prever(&mut self, preset: Option<&Preset>, cx: &mut Context<Self>) {
        if self.previa.as_ref() == preset {
            return;
        }
        self.previa = preset.cloned();
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
        self.historico.registrar(self.estado());
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
        self.predefinicoes.criando = !self.predefinicoes.criando;
        self.nome_do_preset
            .update(cx, |campo, cx| campo.set_value("", window, cx));
        if self.predefinicoes.criando {
            self.nome_do_preset
                .update(cx, |campo, cx| campo.focus(window, cx));
        }
        cx.notify();
    }

    pub fn cancelar_formulario_de_preset(&mut self, cx: &mut Context<Self>) {
        self.predefinicoes.criando = false;
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

        self.predefinicoes.criando = false;
        self.preset_inteiro = false;
        self.nome_do_preset
            .update(cx, |campo, cx| campo.set_value("", window, cx));
        cx.notify();
    }

    // ------------------------------------------------------ importar

    /// Abre o seletor do sistema para importar do Lightroom ou do darktable.
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
        self.predefinicoes.renomeando = Some(id);
        self.renome_do_preset.update(cx, |campo, cx| {
            campo.set_value(nome, window, cx);
            campo.focus(window, cx);
        });
        cx.notify();
    }

    pub fn cancelar_renome(&mut self, cx: &mut Context<Self>) {
        self.predefinicoes.renomeando = None;
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
        if self.predefinicoes.renomeando == Some(id) {
            self.predefinicoes.renomeando = None;
        }

        self.guarda_de_presets.renomear(id, nome);
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
        self.predefinicoes.pergunta = Some((id, nome));
        let foco = self
            .predefinicoes
            .foco_da_pergunta
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        window.focus(&foco);
        cx.notify();
    }

    /// "Apagar" (`true`) ou "Cancelar" (`false`).
    pub fn responder_pergunta(&mut self, apagar: bool, cx: &mut Context<Self>) {
        let Some((id, _)) = self.predefinicoes.pergunta.take() else {
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

    /// Os dois grupos como a coluna os mostra agora.
    pub(super) fn grupos_da_coluna(&self, cx: &App) -> (Vec<&Preset>, Vec<&Preset>) {
        let busca = self.busca_de_presets.read(cx).value().to_string();
        presets::da_coluna(&self.presets, &busca, &self.predefinicoes.ordem)
    }

    /// As chaves de um grupo, na ordem da tela.
    fn chaves_do_grupo(&self, grupo: Grupo, cx: &App) -> Vec<String> {
        let (sistema, minhas) = self.grupos_da_coluna(cx);
        let lista = match grupo {
            Grupo::Sistema => sistema,
            Grupo::Minhas => minhas,
        };
        lista.into_iter().map(ordem::chave).collect()
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
        let primeira_minha = self.grupos_da_coluna(cx).1.first().map(|p| p.id);
        match comando {
            "criar" => self.alternar_formulario_de_preset(window, cx),
            "nome" => self.nome_do_preset.update(cx, |campo, cx| {
                campo.set_value(argumento.to_string(), window, cx)
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
            // primeira) — o clique na linha da coluna.
            "aplicar" => {
                let n: usize = argumento.parse().unwrap_or(0);
                if let Some(preset) = self.grupos_da_coluna(cx).0.get(n).map(|p| (*p).clone()) {
                    self.aplicar_preset(&preset, window, cx);
                }
            }
            "responder" => self.responder_pergunta(argumento == "sim", cx),
            "ordem" => self.definir_ordem_dos_presets(Grupo::Sistema, None, cx),
            "importar" => {
                let caminho = std::path::Path::new(argumento);
                let arquivo = Arquivo {
                    nome: caminho
                        .file_name()
                        .map_or(argumento.to_string(), |n| n.to_string_lossy().to_string()),
                    texto: std::fs::read_to_string(caminho).ok(),
                };
                self.importar(vec![arquivo], cx);
            }
            "prever" => {
                let indice: usize = argumento.parse().unwrap_or(0);
                let preset = self
                    .grupos_da_coluna(cx)
                    .0
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
        let (do_sistema, minhas) = self.grupos_da_coluna(cx);
        let nenhuma = do_sistema.is_empty() && minhas.is_empty();
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
                .text_size(px(11.))
                .line_height(relative(1.375))
                .text_color(apagado)
                .child(if buscando {
                    "Nenhuma predefinição com esse nome."
                } else {
                    "Nenhuma predefinição ainda."
                })
                .into_any_element()
        } else {
            v_flex()
                .gap(px(12.))
                .child(self.grupo_de_presets(Grupo::Sistema, &do_sistema, None, cx))
                .child(self.grupo_de_presets(
                    Grupo::Minhas,
                    &minhas,
                    sem_minhas.then_some(
                        "Ajuste uma foto e use o + para guardar, ou importe do Lightroom ou do darktable pelo ícone ao lado.",
                    ),
                    cx,
                ))
                .into_any_element()
        };

        // ⚠️ **O texto conta as duas regras**, e não só a de somar: enquanto ele
        // prometia "os outros ficam como estão", quem aplicava "Preto e branco"
        // sobre uma sépia via a tonalização de pé e não entendia por quê.
        const RODAPE: &str = "Passe o mouse para ver na foto, clique para aplicar. As que definem o visual recomeçam do neutro; as que só acrescentam — como a nitidez — somam ao que já está. O enquadramento nunca muda. Arraste pela alça para reordenar: a ordem fica guardada neste computador.";
        const DESTAQUE: &str = "recomeçam do neutro";
        let inicio = RODAPE.find(DESTAQUE).unwrap_or(0);
        let rodape = StyledText::new(RODAPE).with_highlights([(
            inicio..inicio + DESTAQUE.len(),
            HighlightStyle {
                color: Some(apagado),
                ..Default::default()
            },
        )]);

        v_flex()
            .id("predefinicoes")
            .gap(px(12.))
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
                    .text_size(px(11.))
                    .line_height(relative(1.375))
                    .text_color(fraco)
                    .child(rodape),
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
            .children(self.pergunta_de_apagar(cx))
            .children(self.avisos_da_coluna(cx))
    }

    /// "Apagar "X"?" — o `useConfirmacao` do site, sobre a tela inteira.
    fn pergunta_de_apagar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, nome) = self.predefinicoes.pergunta.as_ref()?;
        let foco = self.predefinicoes.foco_da_pergunta.clone()?;
        let tamanho = self.predefinicoes.janela.get();
        let tema = cx.theme();
        let (cartao, frente, apagado, borda, realce, perigo, fundo) = (
            tema.popover,
            tema.foreground,
            tema.muted_foreground,
            tema.border,
            tema.muted,
            tema.danger,
            tema.background,
        );
        let largura = px(448.).min(tamanho.width - px(32.));

        Some(
            deferred(
                anchored()
                    .position(point(px(0.), px(0.)))
                    .position_mode(AnchoredPositionMode::Window)
                    .child(
                        div()
                            .id("pergunta-da-predefinicao")
                            .occlude()
                            .w(tamanho.width)
                            .h(tamanho.height)
                            .bg(gpui::black().opacity(0.1))
                            .flex()
                            .items_center()
                            .justify_center()
                            // Clicar fora cancela, como o `onOpenChange` do site.
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.responder_pergunta(false, cx);
                            }))
                            .child(
                                v_flex()
                                    .id("cartao-da-pergunta")
                                    .track_focus(&foco)
                                    .key_context(CONTEXTO_DA_PERGUNTA)
                                    .on_action(cx.listener(
                                        |tela, _: &ConfirmarPergunta, _w, cx| {
                                            tela.responder_pergunta(true, cx);
                                        },
                                    ))
                                    .on_action(cx.listener(|tela, _: &CancelarPergunta, _w, cx| {
                                        tela.responder_pergunta(false, cx);
                                    }))
                                    // O clique no cartão não é "fora".
                                    .on_click(|_, _, cx| cx.stop_propagation())
                                    .w(largura)
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(frente.opacity(0.1))
                                    .bg(cartao)
                                    .text_color(frente)
                                    .shadow_lg()
                                    .overflow_hidden()
                                    .child(
                                        v_flex()
                                            .p(px(16.))
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .text_size(px(16.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(format!("Apagar \"{nome}\"?")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .text_color(apagado)
                                                    .child("A predefinição sai da lista."),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .justify_end()
                                            .gap(px(8.))
                                            .p(px(16.))
                                            .border_t_1()
                                            .border_color(borda)
                                            .bg(realce.opacity(0.5))
                                            .child(
                                                botao_da_pergunta(
                                                    "cancelar-apagar",
                                                    "Cancelar",
                                                    cx,
                                                )
                                                .border_1()
                                                .border_color(borda)
                                                .bg(fundo)
                                                .hover(move |s| s.bg(realce))
                                                .on_click(cx.listener(|tela, _ev, _w, cx| {
                                                    tela.responder_pergunta(false, cx);
                                                })),
                                            )
                                            .child(
                                                botao_da_pergunta("confirmar-apagar", "Apagar", cx)
                                                    .bg(perigo)
                                                    .text_color(gpui::white())
                                                    .hover(move |s| s.bg(perigo.opacity(0.9)))
                                                    .on_click(cx.listener(|tela, _ev, _w, cx| {
                                                        tela.responder_pergunta(true, cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
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
                    .anchor(Corner::BottomRight)
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
                                .rounded(px(8.))
                                .border_1()
                                .border_color(borda)
                                .bg(cartao)
                                .shadow_lg()
                                .text_size(px(13.))
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
        let tema = cx.theme();
        h_flex()
            .items_center()
            .gap(px(4.))
            .child(
                div().flex_1().min_w(px(0.)).child(
                    Styled::h(Input::new(&self.busca_de_presets).xsmall(), px(28.))
                        .text_size(px(12.))
                        .px(px(8.))
                        .rounded(px(4.))
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
                    "Importar do Lightroom ou do darktable (.lrtemplate, .xmp, .dtstyle)",
                    travada,
                    cx,
                )
                .when(!travada, |b| {
                    b.on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.importar_do_lightroom(cx);
                    }))
                }),
            )
    }

    /// O formulário embutido de "salvar como predefinição".
    ///
    /// 🔑 **Embutido, e não diálogo**, como no site: o operador vê o resumo do
    /// que vai guardar enquanto a foto continua à vista.
    fn formulario_de_preset(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.predefinicoes.criando {
            return None;
        }
        let tema = cx.theme();
        let (apagado, frente, borda) = (tema.muted_foreground, tema.foreground, tema.border);
        let (cartao, fundo, realce) = (tema.popover, tema.background, tema.muted);
        let marca = if tema.mode.is_dark() {
            cores::quente()
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
                .rounded(px(4.))
                .border_1()
                .border_color(borda)
                .bg(cartao)
                .p(px(8.))
                .on_action(cx.listener(|tela, _: &Escape, _window, cx| {
                    tela.cancelar_formulario_de_preset(cx);
                }))
                .child(
                    Styled::h(Input::new(&self.nome_do_preset).xsmall(), px(28.))
                        .text_size(px(12.))
                        .px(px(8.))
                        .rounded(px(4.))
                        .bg(fundo),
                )
                .child(
                    div()
                        .id("preset-inteiro")
                        .mt(px(8.))
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .rounded(px(4.))
                        .px(px(2.))
                        .py(px(4.))
                        .text_size(px(11.))
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
                        .text_size(px(11.))
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
                                cx.listener(|tela, _ev, _window, cx| {
                                    tela.cancelar_formulario_de_preset(cx);
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
                .rounded(px(4.))
                .border_1()
                .border_color(borda)
                .bg(cartao)
                .p(px(8.))
                .text_size(px(11.))
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

    /// Um bloco da lista, com o título, a contagem e o "ordem padrão".
    fn grupo_de_presets(
        &self,
        grupo: Grupo,
        lista: &[&Preset],
        vazio: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente) = (tema.muted_foreground, tema.foreground);
        let fraco = apagado.opacity(0.6);
        let titulo = match grupo {
            Grupo::Sistema => "DO SISTEMA",
            Grupo::Minhas => "MINHAS",
        };
        let reordenada = !self.predefinicoes.ordem.do_grupo(grupo).is_empty();

        let cabeca = h_flex()
            .mb(px(4.))
            .items_center()
            .gap(px(6.))
            .text_size(px(10.))
            .line_height(px(15.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(apagado)
            .child(div().flex_none().child(titulo))
            .child(
                div()
                    .flex_none()
                    .text_color(fraco)
                    .child(SharedString::from(lista.len().to_string())),
            )
            .when(reordenada, |d| {
                // ⚠️ Num `flex_1` que empurra para a direita, e não `ml_auto`:
                // com a margem automática o Taffy engolia o espaço entre o
                // título e a contagem ("DO SISTEMA8").
                d.child(
                    div().flex_1().flex().justify_end().child(
                        div()
                            .id(SharedString::from(format!("ordem-padrao-{titulo}")))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(fraco)
                            .cursor_pointer()
                            .hover(move |s| s.text_color(frente))
                            .child("ordem padrão")
                            .tooltip(|window, cx| {
                                Tooltip::new("Desfaz a reordenação deste grupo").build(window, cx)
                            })
                            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                tela.definir_ordem_dos_presets(grupo, None, cx);
                            })),
                    ),
                )
            });

        let corpo: AnyElement = match (lista.is_empty(), vazio) {
            (true, Some(texto)) => div()
                .px(px(4.))
                .pb(px(4.))
                .text_size(px(11.))
                .line_height(relative(1.375))
                .text_color(fraco)
                .child(texto)
                .into_any_element(),
            _ => v_flex()
                .gap(px(2.))
                .children(lista.iter().map(|preset| {
                    if self.predefinicoes.renomeando == Some(preset.id) {
                        self.nome_em_edicao(preset.id, cx)
                    } else {
                        self.linha_de_preset(preset, cx)
                    }
                }))
                .into_any_element(),
        };

        div().child(cabeca).child(corpo).into_any_element()
    }

    /// Uma predefinição na lista.
    ///
    /// 🔑 O id do elemento é o **id do preset**, e não a posição: dois com o
    /// mesmo nome são possíveis, e id por posição faria o GPUI confundir o
    /// estado de dois botões quando a lista mudasse.
    ///
    /// 🚨 **O clique mora no nome, e não na linha**: com o `on_click` na linha
    /// inteira, clicar na lixeira aplicaria a predefinição antes da pergunta.
    fn linha_de_preset(&self, preset: &Preset, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);
        let fraco = apagado.opacity(0.6);
        let travada = self.predefinicoes_travadas();
        let grupo = Grupo::de(preset);
        let chave = ordem::chave(preset);
        let reordena = self.reordenar_ligado(cx);
        let arrasto = self.predefinicoes.arrasto.as_ref();
        let arrastando = arrasto.is_some_and(|a| a.chave == chave);
        let alvo = arrasto
            .filter(|a| a.chave != chave && a.sobre.as_deref() == Some(chave.as_str()))
            .map(|a| a.depois);
        let marca_do_grupo = SharedString::from(format!("linha-{chave}"));
        let nome = SharedString::from(preset.name.clone());
        let quantos = presets::quantos_campos(preset);
        let dica = SharedString::from(format!(
            "{}\n{}",
            presets::dica_do_nome(preset),
            presets::regra_da_linha(preset)
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
            .rounded(px(4.))
            .px(px(4.))
            .hover(move |s| s.bg(realce))
            .when(arrastando, |d| d.opacity(0.4))
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
                    .rounded(px(1.))
                    .bg(cores::quente())
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
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(if travada { fraco } else { frente })
                    .when(!travada, |d| d.cursor_pointer())
                    .child(nome)
                    .tooltip(move |window, cx| Tooltip::new(dica.clone()).build(window, cx))
                    .when(!travada, |d| {
                        d.on_click(cx.listener(move |tela, _ev, window, cx| {
                            // 🚨 A prévia sai **antes** de aplicar: se ficasse, o
                            // resultado seria o preset por cima dele mesmo.
                            tela.prever(None, cx);
                            tela.aplicar_preset(&para_aplicar, window, cx);
                        }))
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("campos-{id}")))
                    .flex_none()
                    .text_size(px(10.))
                    .text_color(fraco)
                    .child(SharedString::from(quantos.to_string()))
                    .tooltip(move |window, cx| {
                        Tooltip::new(format!("{quantos} controles")).build(window, cx)
                    }),
            )
            .children(self.acoes_do_preset(preset, &marca_do_grupo, travada, cx))
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
        let discreto = |botao: Stateful<Div>| {
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
            .on_action(cx.listener(|tela, _: &Escape, _window, cx| tela.cancelar_renome(cx)))
            .child(
                div().flex_1().min_w(px(0.)).child(
                    Styled::h(Input::new(&self.renome_do_preset).xsmall(), px(28.))
                        .text_size(px(12.))
                        .px(px(8.))
                        .rounded(px(4.))
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
                icone_de_botao("cancelar-nome", Icone::X, 14., "Cancelar", false, cx)
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.cancelar_renome(cx))),
            )
            .into_any_element()
    }
}

/// O `IconeBotao` do site: `rounded p-1`, apagado, acende ao passar o mouse.
fn icone_de_botao(
    id: impl Into<SharedString>,
    icone: Icone,
    lado: f32,
    rotulo: impl Into<SharedString>,
    desligado: bool,
    cx: &App,
) -> Stateful<Div> {
    let tema = cx.theme();
    let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);
    let rotulo: SharedString = rotulo.into();
    div()
        .id(id.into())
        .flex_none()
        .rounded(px(4.))
        .p(px(4.))
        .text_color(apagado)
        .child(Icon::new(icone).size(px(lado)))
        .tooltip(move |window, cx| Tooltip::new(rotulo.clone()).build(window, cx))
        .when(desligado, |b| b.opacity(0.4))
        .when(!desligado, |b| {
            b.cursor_pointer()
                .hover(move |s| s.bg(realce).text_color(frente))
        })
}

/// Um botão do rodapé da pergunta: `h-8 px-2.5 rounded-lg text-sm`.
fn botao_da_pergunta(id: &'static str, rotulo: &'static str, _cx: &App) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(32.))
        .px(px(10.))
        .rounded(px(8.))
        .justify_center()
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(rotulo)
}

/// O `BotaoPequeno` do site: "Cancelar" apagado, "Salvar" em âmbar.
fn botao_pequeno(
    id: &'static str,
    rotulo: &'static str,
    destaque: bool,
    desligado: bool,
    cx: &App,
) -> Stateful<Div> {
    let tema = cx.theme();
    let (fundo, texto, pairando): (Hsla, Hsla, Hsla) = if destaque {
        (cores::quente(), gpui::black(), rgb(0xffd230).into())
    } else {
        (
            tema.muted,
            tema.foreground.opacity(0.9),
            tema.muted.opacity(0.7),
        )
    };
    div()
        .id(id)
        .rounded(px(4.))
        .px(px(8.))
        .py(px(4.))
        .text_size(px(12.))
        .line_height(px(16.))
        .bg(fundo)
        .text_color(texto)
        .child(rotulo)
        .when(desligado, |b| b.opacity(0.4))
        .when(!desligado, |b| {
            b.cursor_pointer().hover(move |s| s.bg(pairando))
        })
}
