//! Os selos da célula: nota, etiqueta, sinalizador e balcão.
//!
//! # Por que existe
//!
//! Até 6/set/2026 a célula da grade mostrava **o nome do arquivo**, e nada mais.
//! Nota, etiqueta de cor e sinalizador — as três marcas que a triagem inteira
//! produz, e as três que têm tecla dedicada — só apareciam no painel da direita,
//! uma foto por vez. Quem tria olha a grade: marcar 200 fotos e não ver nenhuma
//! marca é fazer o trabalho às cegas e conferir no varejo.
//!
//! É também de onde vem a cor da tela. O tema tem duas famílias de acento
//! ([`crate::tema`]), e é aqui que a segunda ganha sentido: o azul é da
//! interface, e o **âmbar é do balcão** — a foto que o cliente levou.
//!
//! # As regras que o desenho segue
//!
//! 1. 🔑 **Nada por cima da foto.** Todo selo fica no rodapé da célula. Selo
//!    sobre a imagem cobre justamente o que está sendo avaliado — é a mesma
//!    razão de a seleção ser borda e não fundo colorido.
//! 2. **A fileira de estrelas existe sempre**, acesa ou apagada. Some com a nota
//!    zero, a célula mudaria de altura — e o `uniform_list` mede uma linha e
//!    repete a medida, então a grade inteira sairia do lugar.
//! 3. **Sinalizador e etiqueta só aparecem quando existem.** São exceção; a nota
//!    é a régua comum.

use adapters::view_models::PhotoViewModel;
use biblioteca_core::acervo::Estado;
use biblioteca_core::sessoes::Situacao;
use gpui::{div, prelude::*, px, App, Hsla};
use gpui_component::ActiveTheme;

use crate::tema::cores;

/// O que a célula tem para mostrar, já traduzido do view model.
///
/// 🔑 **Separado do desenho de propósito**: é a parte que se confere sem abrir
/// janela, e onde mora a regra ("−1 é rejeitada", "etiqueta que o domínio não
/// conhece não vira tinta").
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Selos {
    /// De 0 a 5, já limitada.
    pub nota: i32,
    /// A cor da etiqueta, ou nada quando a foto não tem — ou quando tem um nome
    /// que o `ColorLabel` não reconhece.
    pub etiqueta: Option<Hsla>,
    pub sinalizador: Sinalizador,
    /// Levada no balcão.
    pub comprada: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sinalizador {
    #[default]
    Nenhum,
    Escolhida,
    Rejeitada,
}

impl Sinalizador {
    /// O que o banco guarda em `flag`: `1`, `-1`, `0` ou nada.
    ///
    /// ⚠️ **Qualquer outro número é "nenhum"**, e não um caso de pânico: a
    /// coluna é um inteiro livre, e uma linha estranha vinda de importação
    /// antiga não pode derrubar a grade.
    fn do_banco(codigo: Option<i32>) -> Self {
        match codigo {
            Some(1) => Sinalizador::Escolhida,
            Some(-1) => Sinalizador::Rejeitada,
            _ => Sinalizador::Nenhum,
        }
    }

    fn marca(self) -> &'static str {
        match self {
            Sinalizador::Nenhum => "",
            // ⚑ escolhida, ⚐ rejeitada: a mesma bandeira cheia e vazia, como no
            // Lightroom. A cor separa as duas; a forma separa quem enxerga mal
            // vermelho e verde.
            Sinalizador::Escolhida => "⚑",
            Sinalizador::Rejeitada => "⚐",
        }
    }

    fn cor(self) -> Hsla {
        match self {
            Sinalizador::Nenhum => cores::escolhida(),
            Sinalizador::Escolhida => cores::escolhida(),
            Sinalizador::Rejeitada => cores::rejeitada(),
        }
    }
}

impl Selos {
    pub fn da_foto(foto: &PhotoViewModel) -> Self {
        Selos {
            nota: foto.rating.clamp(0, 5),
            etiqueta: foto
                .color_label
                .as_deref()
                .filter(|nome| !nome.trim().is_empty())
                .and_then(cores::etiqueta),
            sinalizador: Sinalizador::do_banco(foto.flag),
            comprada: foto.comprada,
        }
    }
}

/// A fileira do rodapé da célula: estrelas à esquerda, exceções à direita.
pub fn faixa(selos: &Selos, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .h(px(14.))
        .child(estrelas(selos.nota, cx))
        .child(div().flex_1())
        .when(selos.sinalizador != Sinalizador::Nenhum, |linha| {
            linha.child(
                div()
                    .text_xs()
                    .text_color(selos.sinalizador.cor())
                    .child(selos.sinalizador.marca()),
            )
        })
        .when(selos.comprada, |linha| linha.child(selo_do_balcao(cx)))
        .when_some(selos.etiqueta, |linha, cor| {
            linha.child(
                // Um ponto, e não uma barra colorida: a barra competiria com a
                // moldura da seleção, que é a outra coisa colorida da célula.
                div()
                    .size(px(8.))
                    .rounded_full()
                    .bg(cor)
                    .border_1()
                    .border_color(cx.theme().background),
            )
        })
}

/// As cinco estrelas — as acesas em ouro, as apagadas quase invisíveis.
///
/// 🔑 **As apagadas ficam, e é o que faz a nota ser legível de relance**: com
/// só as acesas desenhadas, distinguir 3 de 4 exige contar; com a régua inteira,
/// é a proporção que se lê.
pub fn estrelas(nota: i32, cx: &App) -> impl IntoElement {
    let nota = nota.clamp(0, 5);
    div()
        .flex()
        .gap(px(1.))
        .text_xs()
        .children((1..=5).map(move |estrela| {
            let acesa = estrela <= nota;
            div()
                .text_color(if acesa {
                    cores::nota()
                } else {
                    cx.theme().muted_foreground.opacity(0.25)
                })
                .child("★")
        }))
}

/// "levada" — o selo do balcão, na família âmbar.
///
/// 🔑 **Não é o azul de ação**, e era antes: azul é o que a interface faz
/// (selecionar, abrir, aplicar), âmbar é o que o cliente comprou. Com uma cor
/// só, "esta foto está selecionada" e "esta foto foi vendida" chegavam ao olho
/// pelo mesmo caminho.
pub fn selo_do_balcao(cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .px(px(4.))
        .rounded(cx.theme().radius)
        .bg(cores::quente())
        .text_color(cores::sobre_quente())
        .child("levada")
}

/// As cinco etiquetas: **a grafia que o banco guarda** e o nome em português.
///
/// 🔑 **Uma lista só.** A barra de filtros escrevia as cinco à mão e o painel de
/// informações mostrava o valor cru (`"Yellow"`) — duas listas para a mesma
/// decisão, que é a armadilha nº 8 do projeto. A grafia com maiúscula é a que o
/// legado gravou na coluna; comparar com outra não casaria com nada.
pub const ETIQUETAS: [(&str, &str); 5] = [
    ("Red", "vermelho"),
    ("Yellow", "amarelo"),
    ("Green", "verde"),
    ("Blue", "azul"),
    ("Purple", "roxo"),
];

/// O nome em português de uma etiqueta, em qualquer grafia.
pub fn nome_da_etiqueta(valor: &str) -> Option<&'static str> {
    ETIQUETAS
        .iter()
        .find(|(guardado, _)| guardado.eq_ignore_ascii_case(valor))
        .map(|(_, nome)| *nome)
}

/// Que peso um selo tem na tela.
///
/// 🔑 **O comum é cinza, e só a exceção tem cor.** "Disponível" é o estado de
/// quase toda foto de um ensaio recém-publicado, e "aguardando o cliente" é a
/// situação de quase toda sessão: pintar o comum faria a tela inteira acender e
/// não sobraria contraste para o que mudou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tom {
    /// O estado comum. Sem cor.
    Neutro,
    /// Sem arquivo, ou sem nada dentro — desbotado.
    Apagado,
    /// O balcão: o cliente está do outro lado disto.
    Quente,
    /// Acabou bem: vendida, aberta pelo cliente.
    Bom,
    /// Passou da hora.
    Ruim,
}

/// ⚠️ **A retenção vence o estado**: uma foto comprada e depois apagada não é
/// "comprada" para quem olha — o arquivo não existe mais.
pub fn tom_do_estado(estado: Estado, apagada: bool) -> Tom {
    if apagada {
        return Tom::Apagado;
    }
    match estado {
        Estado::Disponivel => Tom::Neutro,
        Estado::LevadaNoBalcao => Tom::Quente,
        Estado::Comprada => Tom::Bom,
    }
}

/// A situação de uma sessão, na lista de sessões.
pub fn tom_da_situacao(situacao: Situacao) -> Tom {
    match situacao {
        // Uma sessão sem foto ainda não começou: ela é um cadastro.
        Situacao::SemFotos => Tom::Apagado,
        // 🔑 O comum de uma sessão publicada — e por isso sem cor.
        Situacao::AguardandoCliente => Tom::Neutro,
        // O cliente abriu: é o sinal que o fotógrafo espera para cobrar.
        Situacao::AbertaPeloCliente => Tom::Bom,
        Situacao::Vencida => Tom::Ruim,
    }
}

/// Fundo e texto de cada tom.
fn cores_do_tom(tom: Tom, cx: &App) -> (Hsla, Hsla) {
    match tom {
        Tom::Neutro => (cx.theme().secondary, cx.theme().secondary_foreground),
        Tom::Apagado => (cx.theme().muted, cx.theme().muted_foreground),
        Tom::Quente => (cores::quente(), cores::sobre_quente()),
        Tom::Bom => (cx.theme().success, cx.theme().success_foreground),
        Tom::Ruim => (cx.theme().danger, cx.theme().danger_foreground),
    }
}

/// Um selo: uma palavra, o tom dela, e nada mais.
pub fn selo(tom: Tom, texto: impl Into<gpui::SharedString>, cx: &App) -> impl IntoElement {
    let (fundo, frente) = cores_do_tom(tom, cx);
    div()
        .flex_none()
        .px(px(5.))
        .py(px(1.))
        .rounded(cx.theme().radius)
        .bg(fundo)
        .text_color(frente)
        .text_xs()
        .child(texto.into())
}

/// O selo do estado de uma foto do site, como a grade do ensaio o desenha.
pub fn selo_do_estado(estado: Estado, apagada: bool, cx: &App) -> impl IntoElement {
    let texto = if apagada {
        "Apagada".to_string()
    } else {
        estado.rotulo().to_string()
    };
    selo(tom_do_estado(estado, apagada), texto, cx)
}

/// O selo da foto **que só existe no disco** — a importada, antes do passo 3.
///
/// 🚨 **Ela não é "à venda", e dizer que é seria mentir na cor certa.** O
/// `acervo::Estado` só sabe falar do que existe no site (levada · à venda ·
/// comprada), e a importada entra no menos errado dos três — mas o cliente não a
/// vê, não pode comprá-la, e ela nem chegou ao storage. Quem olha a grade
/// precisa saber a diferença: é ela que diz o que ainda falta fazer.
pub fn selo_de_so_no_disco(cx: &App) -> impl IntoElement {
    selo(Tom::Neutro, "No disco", cx)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto() -> PhotoViewModel {
        PhotoViewModel::default()
    }

    #[test]
    fn a_nota_e_limitada_as_cinco_estrelas() {
        let mut acima = foto();
        acima.rating = 9;
        assert_eq!(Selos::da_foto(&acima).nota, 5);

        let mut abaixo = foto();
        abaixo.rating = -3;
        assert_eq!(Selos::da_foto(&abaixo).nota, 0);
    }

    #[test]
    fn o_sinalizador_vem_do_codigo_do_banco() {
        for (codigo, esperado) in [
            (Some(1), Sinalizador::Escolhida),
            (Some(-1), Sinalizador::Rejeitada),
            (Some(0), Sinalizador::Nenhum),
            (None, Sinalizador::Nenhum),
            // 🚨 Lixo na coluna não derruba a grade.
            (Some(7), Sinalizador::Nenhum),
        ] {
            let mut foto = foto();
            foto.flag = codigo;
            assert_eq!(
                Selos::da_foto(&foto).sinalizador,
                esperado,
                "flag {codigo:?} virou o sinalizador errado"
            );
        }
    }

    /// As duas bandeiras têm formas diferentes, e não só cores diferentes.
    #[test]
    fn escolhida_e_rejeitada_se_distinguem_sem_cor() {
        assert_ne!(
            Sinalizador::Escolhida.marca(),
            Sinalizador::Rejeitada.marca()
        );
    }

    #[test]
    fn a_etiqueta_vira_tinta_na_grafia_do_banco() {
        let mut amarela = foto();
        amarela.color_label = Some("Yellow".into());
        assert_eq!(
            Selos::da_foto(&amarela).etiqueta,
            cores::etiqueta("yellow"),
            "a grafia com maiúscula é a que o banco guarda"
        );
    }

    /// 🚨 Etiqueta vazia ou desconhecida não vira ponto de cor.
    ///
    /// A coluna é texto livre: `Some("")` chega de linha antiga, e um nome que o
    /// domínio não conhece chegaria de qualquer importador. Nos dois casos o
    /// certo é não desenhar nada — desenhar cinza diria "tem etiqueta" para uma
    /// foto que não tem.
    #[test]
    fn etiqueta_vazia_ou_desconhecida_nao_desenha() {
        for nome in ["", "   ", "laranja"] {
            let mut foto = foto();
            foto.color_label = Some(nome.into());
            assert_eq!(
                Selos::da_foto(&foto).etiqueta,
                None,
                "{nome:?} virou cor na grade"
            );
        }
    }

    /// A lista de etiquetas cobre as cinco do domínio, e todas têm cor.
    #[test]
    fn a_lista_de_etiquetas_e_a_do_dominio() {
        use domain::value_objects::ColorLabel;

        assert_eq!(ETIQUETAS.len(), ColorLabel::all().len());
        for (valor, nome) in ETIQUETAS {
            assert!(
                cores::etiqueta(valor).is_some(),
                "`{valor}` está na barra de filtros e não tem cor"
            );
            assert_eq!(nome_da_etiqueta(&valor.to_lowercase()), Some(nome));
        }
        assert_eq!(nome_da_etiqueta("laranja"), None);
    }

    /// Cada estado do acervo do site tem um tom só seu, e a retenção vence.
    #[test]
    fn o_tom_do_selo_separa_os_estados() {
        assert_eq!(tom_do_estado(Estado::Disponivel, false), Tom::Neutro);
        assert_eq!(tom_do_estado(Estado::LevadaNoBalcao, false), Tom::Quente);
        assert_eq!(tom_do_estado(Estado::Comprada, false), Tom::Bom);

        // 🚨 Comprada **e** apagada aparece como apagada: o cliente já não tem
        // o que baixar, e dizer "comprada" mandaria o operador procurar um
        // arquivo que a retenção levou.
        assert_eq!(tom_do_estado(Estado::Comprada, true), Tom::Apagado);
    }

    /// A situação da sessão: só o que exige ação ganha cor.
    #[test]
    fn o_tom_da_situacao_reserva_a_cor_para_a_excecao() {
        assert_eq!(tom_da_situacao(Situacao::AguardandoCliente), Tom::Neutro);
        assert_eq!(tom_da_situacao(Situacao::SemFotos), Tom::Apagado);
        assert_eq!(tom_da_situacao(Situacao::AbertaPeloCliente), Tom::Bom);
        assert_eq!(tom_da_situacao(Situacao::Vencida), Tom::Ruim);

        // 🔑 As quatro do core estão cobertas — uma situação nova sem tom cairia
        // no `match` e o compilador acusa, mas só se este teste as percorrer.
        for situacao in Situacao::TODAS {
            let _ = tom_da_situacao(situacao);
        }
    }

    #[test]
    fn foto_sem_marca_nenhuma_nao_tem_selo() {
        let selos = Selos::da_foto(&foto());
        assert_eq!(selos.nota, 0);
        assert_eq!(selos.etiqueta, None);
        assert_eq!(selos.sinalizador, Sinalizador::Nenhum);
        assert!(!selos.comprada);
    }
}
