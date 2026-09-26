//! 🧪 Os gestos da Revelação que os cenários de ponta a ponta (`crate::e2e`)
//! precisam fazer **pelo caminho do ponteiro**, e que não têm tecla.
//!
//! 🔑 **Nada aqui é atalho para o estado.** Cada gancho faz o que o clique ou o
//! arrasto faz: o slider recebe o `Change` que o `Slider` emite (e não um
//! `set_value`, que não emite), a alça do Enquadrar passa pelo começar, mover e
//! soltar, e a predefinição arrastada passa pelo mesmo `soltar`. Um gancho que
//! escrevesse no campo daria um teste que passa com a inscrição morta.

use gpui::{px, Bounds, Context, Window};
use gpui_component::slider::{SliderEvent, SliderValue};

use super::Revelacao;
use crate::revelacao::controles::Definicao;
use crate::revelacao::corte::Alca;
use crate::revelacao::presets::ordem::{self, Grupo};
use domain::entities::Preset;
use domain::value_objects::CropSettings;

impl Revelacao {
    /// O palco com este tamanho, como o `canvas` o mediria no quadro.
    pub(crate) fn medir_o_palco(&mut self, largura: f32, altura: f32) {
        self.palco = Bounds::new(
            gpui::point(px(0.), px(0.)),
            gpui::size(px(largura), px(altura)),
        );
    }

    /// O índice do primeiro controle que obedece a `filtro` — para achar um
    /// slider da aba RGB sem escrever a posição na tabela.
    pub(crate) fn controle_onde(&self, filtro: impl Fn(&Definicao) -> bool) -> Option<usize> {
        self.controles.iter().position(|c| filtro(c.definicao))
    }

    /// Um arrasto de slider: o `Change` que o componente emite.
    pub(crate) fn arrastar_slider(&mut self, controle: usize, valor: f32, cx: &mut Context<Self>) {
        let estado = self.controles[controle].estado.clone();
        estado.update(cx, |_, cx| {
            cx.emit(SliderEvent::Change(SliderValue::Single(valor)))
        });
    }

    /// Onde o slider está desenhado.
    pub(crate) fn valor_do_slider(&self, controle: usize, cx: &gpui::App) -> f32 {
        self.controles[controle].estado.read(cx).value().start()
    }

    /// O endireitar pelo slider do painel de corte.
    pub(crate) fn arrastar_angulo(&mut self, graus: f32, cx: &mut Context<Self>) {
        let estado = self.angulo.clone();
        estado.update(cx, |_, cx| {
            cx.emit(SliderEvent::Change(SliderValue::Single(graus)))
        });
    }

    /// O enquadramento **na tela** — o do retângulo, com o gesto em curso.
    pub(crate) fn corte_na_ferramenta(&self) -> CropSettings {
        self.corte_atual()
    }

    /// Pega uma alça (ou o miolo, com `None`) e a arrasta `dx`×`dy` pixels
    /// **da foto** (do espaço girado), pelo palco desenhado — começar, mover e
    /// soltar, como o ponteiro.
    pub(crate) fn arrastar_no_corte(
        &mut self,
        alca: Option<Alca>,
        dx: f32,
        dy: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.area_da_foto().is_none() {
            // Sem quadro desenhado ainda: o palco de uma janela comum.
            self.medir_o_palco(800., 600.);
        }
        let (Some(espaco), Some(area)) = (
            self.tamanho_da_foto()
                .map(|foto| crate::revelacao::corte::espaco_de(&self.corte_atual(), foto)),
            self.area_da_foto(),
        ) else {
            return;
        };
        let escala = area.2 / espaco.0;
        let (x0, y0) = (area.0 + area.2 / 2., area.1 + area.3 / 2.);
        self.comecar_arrasto(alca, gpui::point(px(x0), px(y0)), cx);
        self.mover_no_corte(
            gpui::point(px(x0 + dx * escala), px(y0 + dy * escala)),
            window,
            cx,
        );
        self.soltar_no_corte(cx);
    }

    /// A proporção travada no Enquadrar.
    pub(crate) fn proporcao_travada(&self) -> Option<f32> {
        self.edicao.as_ref().and_then(|e| e.proporcao)
    }

    /// A linha que responde ao "Apagar" — a lixeira da coluna.
    pub(crate) fn clicar_na_lixeira(
        &mut self,
        preset: &Preset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pedir_para_apagar(preset.id, window, cx);
    }

    /// A pergunta de apagar, aberta: o nome da predefinição.
    pub(crate) fn nome_na_pergunta_de_apagar(&self) -> Option<String> {
        self.predefinicoes
            .pergunta
            .aberto()
            .map(|(_, nome)| nome.clone())
    }

    /// As predefinições que a coluna mostra: `(do sistema, minhas)`, em nome.
    pub(crate) fn coluna_de_predefinicoes(&self, cx: &gpui::App) -> (Vec<String>, Vec<String>) {
        let (sistema, minhas) = self.grupos_da_coluna(cx);
        (
            sistema.iter().map(|p| p.name.clone()).collect(),
            minhas.iter().map(|p| p.name.clone()).collect(),
        )
    }

    /// A predefinição com este nome, como a lista a tem.
    pub(crate) fn predefinicao(&self, nome: &str) -> Option<Preset> {
        self.presets.iter().find(|p| p.name == nome).cloned()
    }

    /// O arrasto de uma linha do grupo do sistema: pega a `de`-ésima, passa
    /// sobre a `sobre`-ésima e solta antes dela.
    pub(crate) fn arrastar_predefinicao_do_sistema(
        &mut self,
        de: usize,
        sobre: usize,
        cx: &mut Context<Self>,
    ) {
        let chaves: Vec<String> = self
            .grupos_da_coluna(cx)
            .0
            .into_iter()
            .map(ordem::chave)
            .collect();
        let (Some(pega), Some(alvo)) = (chaves.get(de).cloned(), chaves.get(sobre).cloned()) else {
            return;
        };
        self.comecar_arrasto_de_preset(Grupo::Sistema, pega, cx);
        self.passar_sobre_preset(&alvo, false, cx);
        self.soltar_preset(cx);
    }

    /// Digita o nome no formulário de "Salvar como predefinição".
    pub(crate) fn digitar_nome_da_predefinicao(
        &mut self,
        nome: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nome = nome.to_string();
        self.nome_do_preset
            .update(cx, |campo, cx| campo.set_value(nome, window, cx));
    }

    pub(crate) fn formulario_de_predefinicao_aberto(&self) -> bool {
        self.predefinicoes.criando.esta_aberto()
    }

    /// O resultado da última importação: `(arquivos lidos, predefinições novas)`.
    pub(crate) fn relatorio_da_importacao(&self) -> Option<(usize, usize)> {
        self.relatorio.as_ref().map(|r| (r.arquivos, r.criadas))
    }

    /// Quantos pixels do bruto cabem num da cópia — 1 até a raiz medir o
    /// bruto da foto aberta.
    pub(crate) fn fator_do_bruto_medido(&self) -> f32 {
        self.fator_do_bruto()
    }

    /// Onde o zoom está centrado, em fração da foto.
    pub(crate) fn centro_do_zoom(&self) -> (f32, f32) {
        let centro = self.navegacao.zoom.centro;
        (centro.x, centro.y)
    }
}
