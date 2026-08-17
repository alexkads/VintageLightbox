//! Os cinco painéis da Revelação, para o dock poder movê-los.
//!
//! O mesmo desenho dos painéis da Biblioteca ([`crate::biblioteca::paineis`]):
//! views finas por cima do **mesmo estado**, apontando de volta com
//! `WeakEntity` para não fechar um ciclo de contagem de referência.
//!
//! ## ⚠️ O corte de painéis é o do legado, com uma junção
//!
//! O `create_develop_layout` de lá tem presets à esquerda, a foto no centro,
//! histograma e ajustes à direita, filmstrip embaixo. Aqui é igual, exceto que
//! **histograma e curva de tons ficam no mesmo painel**: os dois são desenho da
//! mesma medida (a foto que está na tela), nenhum tem controle, e separá-los
//! daria uma aba de 120px de altura para um gráfico só.

use gpui::{
    div, prelude::*, App, Context, EventEmitter, FocusHandle, Focusable, Render, SharedString,
    WeakEntity, Window,
};
use gpui_component::dock::{Panel, PanelEvent};

use super::tela::Revelacao;

/// Qual dos cinco é este painel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qual {
    Palco,
    Ajustes,
    Graficos,
    Presets,
    Filmstrip,
}

impl Qual {
    pub const TODOS: [Qual; 5] = [
        Qual::Palco,
        Qual::Ajustes,
        Qual::Graficos,
        Qual::Presets,
        Qual::Filmstrip,
    ];

    /// 🚨 **O nome é o que o arranjo gravado guarda**, e mudá-lo faz um leiaute
    /// salvo apontar para um painel que não existe — que o `gpui-component`
    /// devolve como `InvalidPanel`, um retângulo com o nome escrito dentro no
    /// lugar da foto.
    pub fn nome(&self) -> &'static str {
        match self {
            Qual::Palco => "revelacao:palco",
            Qual::Ajustes => "revelacao:ajustes",
            Qual::Graficos => "revelacao:graficos",
            Qual::Presets => "revelacao:presets",
            Qual::Filmstrip => "revelacao:filmstrip",
        }
    }

    fn titulo(&self) -> &'static str {
        match self {
            Qual::Palco => "Foto",
            Qual::Ajustes => "Ajustes",
            Qual::Graficos => "Histograma",
            Qual::Presets => "Presets",
            Qual::Filmstrip => "Filmstrip",
        }
    }
}

pub struct PainelDaRevelacao {
    qual: Qual,
    revelacao: WeakEntity<Revelacao>,
    foco: FocusHandle,
}

impl PainelDaRevelacao {
    pub fn novo(qual: Qual, revelacao: WeakEntity<Revelacao>, cx: &mut Context<Self>) -> Self {
        Self {
            qual,
            revelacao,
            foco: cx.focus_handle(),
        }
    }
}

impl Render for PainelDaRevelacao {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let qual = self.qual;
        let Some(revelacao) = self.revelacao.upgrade() else {
            // A janela está fechando e o estado já foi embora — desenhar vazio é
            // melhor do que o `expect` que derrubaria o app no último quadro.
            return div().into_any_element();
        };

        revelacao.update(cx, |tela, cx| tela.desenhar_painel(qual, window, cx))
    }
}

impl Focusable for PainelDaRevelacao {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl EventEmitter<PanelEvent> for PainelDaRevelacao {}

impl Panel for PainelDaRevelacao {
    fn panel_name(&self) -> &'static str {
        self.qual.nome()
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SharedString::from(self.qual.titulo())
    }

    /// ⚠️ **Nenhum painel fecha** — a mesma decisão da Biblioteca. No legado
    /// fechar "AllAdjustments" tira os 42 controles, e a volta é o "Reset
    /// Docking Layout", que joga fora o arranjo das duas telas de uma vez.
    fn closable(&self, _cx: &App) -> bool {
        false
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 Os nomes são o que o arranjo gravado guarda — mudá-los quebra o
    /// leiaute de quem já arrumou a tela.
    #[test]
    fn os_nomes_dos_paineis_nao_mudam() {
        assert_eq!(Qual::Palco.nome(), "revelacao:palco");
        assert_eq!(Qual::Ajustes.nome(), "revelacao:ajustes");
        assert_eq!(Qual::Graficos.nome(), "revelacao:graficos");
        assert_eq!(Qual::Presets.nome(), "revelacao:presets");
        assert_eq!(Qual::Filmstrip.nome(), "revelacao:filmstrip");
    }

    /// E não colidem com os da Biblioteca: os dois docks dividem o mesmo
    /// registro global, e dois painéis com o mesmo nome fariam um restaurar no
    /// lugar do outro.
    #[test]
    fn os_nomes_nao_colidem_com_os_da_biblioteca() {
        use crate::biblioteca::paineis::Qual as DaBiblioteca;

        let daqui: std::collections::HashSet<&str> =
            Qual::TODOS.iter().map(|qual| qual.nome()).collect();
        let de_la: std::collections::HashSet<&str> = [
            DaBiblioteca::Pastas,
            DaBiblioteca::Grade,
            DaBiblioteca::Informacoes,
            DaBiblioteca::Filmstrip,
        ]
        .iter()
        .map(|qual| qual.nome())
        .collect();

        assert!(
            daqui.is_disjoint(&de_la),
            "os dois docks dividem o mesmo registro global de painéis"
        );
        assert_eq!(daqui.len(), Qual::TODOS.len(), "e nenhum se repete aqui");
    }
}
