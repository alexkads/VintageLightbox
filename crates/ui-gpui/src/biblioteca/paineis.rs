//! Os quatro painéis da Biblioteca, para o dock poder movê-los.
//!
//! No legado a tela inteira é um `DockArea` do `egui_dock` com abas
//! arrastáveis; aqui cada painel é uma `Entity` fina por cima do **mesmo
//! estado** — a `Biblioteca`, que continua guardando fotos, filtros, seleção e
//! cache.
//!
//! ## 🔑 Por que os painéis não têm estado próprio
//!
//! O desenho de cada um já existe como método da `Biblioteca`, e os `cx.listener`
//! de lá esperam `Context<Biblioteca>`. Mover o desenho para dentro de views
//! novas trocaria **todos** eles por `entidade.update(…)`: umas 600 linhas
//! reescritas para mudar de lugar, com os testes por baixo. Assim o dock custou
//! um arquivo pequeno, e o que já estava conferido não se mexeu.
//!
//! ## ⚠️ A referência de volta é fraca
//!
//! A `Biblioteca` guarda o `DockArea`, o dock guarda os painéis, e os painéis
//! apontam para a `Biblioteca`. Com `Entity` nos dois sentidos isso é um ciclo
//! de contagem de referência — memória que não volta ao fechar a janela. Com
//! `WeakEntity` na volta, o ciclo se abre; quando ela não puder ser lida (a
//! janela fechando), o painel desenha vazio em vez de derrubar o app.

use gpui::{
    div, prelude::*, AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, Render,
    SharedString, WeakEntity, Window,
};
use gpui_component::dock::{Panel, PanelEvent};

use super::tela::Biblioteca;

/// Qual dos quatro é este painel.
///
/// Um enum, e não quatro structs iguais: o que muda entre eles é **o nome e qual
/// método de desenho chamar** — quatro tipos com o mesmo corpo seriam a mesma
/// coisa escrita quatro vezes, que é o que a tabela de controles da Revelação já
/// ensinou a não fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qual {
    Pastas,
    Grade,
    Informacoes,
    Filmstrip,
}

impl Qual {
    /// 🚨 **O nome é o que o arranjo gravado guarda**, e mudá-lo depois faz o
    /// leiaute salvo apontar para um painel que não existe mais. O
    /// `gpui-component` avisa disso na própria `trait` ("once you have defined a
    /// panel name, this must not be changed") — e o legado tem o mesmo contrato
    /// com o `serde` do `DockTab`.
    pub fn nome(&self) -> &'static str {
        match self {
            Qual::Pastas => "biblioteca:pastas",
            Qual::Grade => "biblioteca:grade",
            Qual::Informacoes => "biblioteca:informacoes",
            Qual::Filmstrip => "biblioteca:filmstrip",
        }
    }

    fn titulo(&self) -> &'static str {
        match self {
            Qual::Pastas => "Pastas",
            Qual::Grade => "Grade",
            Qual::Informacoes => "Informações",
            Qual::Filmstrip => "Filmstrip",
        }
    }
}

pub struct PainelDaBiblioteca {
    qual: Qual,
    acervo: WeakEntity<Biblioteca>,
    foco: FocusHandle,
}

impl PainelDaBiblioteca {
    pub fn novo(qual: Qual, acervo: WeakEntity<Biblioteca>, cx: &mut Context<Self>) -> Self {
        Self {
            qual,
            acervo,
            foco: cx.focus_handle(),
        }
    }

    pub fn qual(&self) -> Qual {
        self.qual
    }
}

impl Render for PainelDaBiblioteca {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let qual = self.qual;
        let Some(acervo) = self.acervo.upgrade() else {
            // A janela está fechando e o estado já foi embora. Desenhar vazio é
            // o que resta — e é melhor do que o `expect` que derrubaria o app no
            // último quadro da sessão.
            return div().into_any_element();
        };

        acervo.update(cx, |biblioteca, cx| match qual {
            Qual::Pastas => biblioteca.painel_das_pastas(cx),
            Qual::Grade => biblioteca.painel_da_grade(window, cx),
            Qual::Informacoes => biblioteca.painel_das_informacoes(cx),
            Qual::Filmstrip => biblioteca.painel_do_filmstrip(cx),
        })
    }
}

impl Focusable for PainelDaBiblioteca {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl EventEmitter<PanelEvent> for PainelDaBiblioteca {}

impl Panel for PainelDaBiblioteca {
    fn panel_name(&self) -> &'static str {
        self.qual.nome()
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SharedString::from(self.qual.titulo())
    }

    /// ⚠️ **Nenhum painel fecha.**
    ///
    /// No legado eles fecham, e voltar exige o "Reset Docking Layout", que joga
    /// fora o arranjo das duas telas de uma vez — fechar a grade por engano custa
    /// tudo o que se arrumou. Aqui eles se movem e se redimensionam; some a
    /// única forma de perder um painel sem querer.
    fn closable(&self, _cx: &App) -> bool {
        false
    }
}

/// O desenho de um painel, para quem só quer o elemento.
///
/// Existe para o dia em que a Biblioteca precisar desenhar um painel fora do
/// dock (o modo de tela cheia de um painel, por exemplo) sem duplicar o `match`.
pub fn desenhar(
    qual: Qual,
    biblioteca: &mut Biblioteca,
    window: &mut Window,
    cx: &mut Context<Biblioteca>,
) -> AnyElement {
    match qual {
        Qual::Pastas => biblioteca.painel_das_pastas(cx),
        Qual::Grade => biblioteca.painel_da_grade(window, cx),
        Qual::Informacoes => biblioteca.painel_das_informacoes(cx),
        Qual::Filmstrip => biblioteca.painel_do_filmstrip(cx),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 Os nomes dos painéis são o que o arranjo gravado guarda.
    ///
    /// Mudar um deles faz o leiaute salvo apontar para um painel que não existe
    /// — e o que se perde é o arranjo de quem já tinha arrumado a tela. Este
    /// teste existe para a mudança **falhar aqui**, e não no dia seguinte, na
    /// máquina de quem usa.
    #[test]
    fn os_nomes_dos_paineis_nao_mudam() {
        assert_eq!(Qual::Pastas.nome(), "biblioteca:pastas");
        assert_eq!(Qual::Grade.nome(), "biblioteca:grade");
        assert_eq!(Qual::Informacoes.nome(), "biblioteca:informacoes");
        assert_eq!(Qual::Filmstrip.nome(), "biblioteca:filmstrip");
    }

    /// E são quatro nomes diferentes — dois iguais fariam o dock restaurar o
    /// mesmo painel duas vezes.
    #[test]
    fn cada_painel_tem_um_nome_so_dele() {
        let nomes = [
            Qual::Pastas.nome(),
            Qual::Grade.nome(),
            Qual::Informacoes.nome(),
            Qual::Filmstrip.nome(),
        ];
        let unicos: std::collections::HashSet<&str> = nomes.iter().copied().collect();

        assert_eq!(unicos.len(), nomes.len());
    }
}
