//! A linha de filtros **por coluna** da lista de sessões — o botão "Filtros"
//! e os campos sob o cabeçalho, como a `linha-de-filtros.tsx` do site.
//!
//! 🔑 **A conta não mora aqui**: o que cada campo quer dizer e quem passa é do
//! `biblioteca_core::filtro_de_coluna`. Esta peça só guarda os campos e diz,
//! a cada quadro, quais estão preenchidos.
//!
//! ⚠️ **A linha só aparece quando o operador a liga** — ou enquanto houver
//! filtro valendo, para ninguém ficar com a lista recortada sem ver por quê.
//! Nove campos vazios sob o cabeçalho seriam nove alvos de clique acidental.

use crate::campo::TrocarValor as _;
use std::collections::HashMap;

use biblioteca_core::filtro_de_coluna::{
    ler_dia, ler_numero, Coluna, FiltroDeColuna, Operador, Tipo,
};
use biblioteca_core::sessoes::Situacao;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{SearchableVec, Select, SelectItem, SelectState};
use gpui_kit::component::Sizable;
use gpui_kit::{prelude::*, px, App, Entity, SharedString, Window};

/// Uma opção de lista: o valor que se lê e o rótulo que se vê.
#[derive(Debug, Clone)]
pub struct Opcao {
    valor: String,
    titulo: SharedString,
}

impl SelectItem for Opcao {
    type Value = String;

    fn title(&self) -> SharedString {
        self.titulo.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.valor
    }

    fn display_title(&self) -> Option<gpui_kit::AnyElement> {
        None
    }
}

type Lista = Entity<SelectState<SearchableVec<Opcao>>>;

pub struct FiltrosDaLista {
    ligada: bool,
    textos: HashMap<Coluna, Entity<InputState>>,
    operadores: HashMap<Coluna, Lista>,
    situacao: Lista,
    de: Entity<InputState>,
    ate: Entity<InputState>,
}

fn simbolo(operador: Operador) -> String {
    operador.simbolo().to_string()
}

impl FiltrosDaLista {
    pub fn novos<T: 'static>(window: &mut Window, cx: &mut gpui_kit::Context<T>) -> Self {
        let mut textos = HashMap::new();
        let mut operadores = HashMap::new();
        for coluna in Coluna::TODAS {
            match coluna.tipo() {
                Tipo::Texto => {
                    textos.insert(
                        coluna,
                        cx.new(|cx| InputState::new(window, cx).placeholder("contém…")),
                    );
                }
                Tipo::Numero | Tipo::Dinheiro => {
                    textos.insert(coluna, cx.new(|cx| InputState::new(window, cx)));
                    let opcoes: Vec<Opcao> = Operador::TODOS
                        .iter()
                        .map(|o| Opcao {
                            valor: simbolo(*o),
                            titulo: o.simbolo().into(),
                        })
                        .collect();
                    let lista =
                        cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
                    lista.update(cx, |estado, cx| {
                        estado.set_selected_value(&simbolo(Operador::MaiorIgual), window, cx)
                    });
                    operadores.insert(coluna, lista);
                }
                Tipo::Data | Tipo::Escolha => {}
            }
        }
        let mut opcoes = vec![Opcao {
            valor: String::new(),
            titulo: "todas".into(),
        }];
        opcoes.extend(Situacao::TODAS.iter().map(|s| Opcao {
            valor: s.como_texto().to_string(),
            titulo: s.rotulo().into(),
        }));
        let situacao = cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
        situacao.update(cx, |estado, cx| {
            estado.set_selected_value(&String::new(), window, cx)
        });
        Self {
            ligada: false,
            textos,
            operadores,
            situacao,
            de: cx.new(|cx| InputState::new(window, cx).placeholder("de dd/mm/aaaa")),
            ate: cx.new(|cx| InputState::new(window, cx).placeholder("até dd/mm/aaaa")),
        }
    }

    pub fn alternar(&mut self) {
        self.ligada = !self.ligada;
    }

    /// A linha aparece? Ligada, ou com filtro valendo.
    pub fn visivel(&self, cx: &App) -> bool {
        self.ligada || !self.ativos(cx).is_empty()
    }

    pub fn ligada(&self) -> bool {
        self.ligada
    }

    /// Os filtros preenchidos agora. Campo em branco — ou que não se lê como
    /// número ou data — não filtra.
    pub fn ativos(&self, cx: &App) -> Vec<(Coluna, FiltroDeColuna)> {
        let texto = |coluna: &Coluna| {
            self.textos
                .get(coluna)
                .map(|campo| campo.read(cx).value().to_string())
                .unwrap_or_default()
        };
        Coluna::TODAS
            .into_iter()
            .filter_map(|coluna| {
                let filtro = match coluna.tipo() {
                    Tipo::Texto => {
                        let t = texto(&coluna);
                        (!t.trim().is_empty()).then_some(FiltroDeColuna::Texto(t))?
                    }
                    Tipo::Numero | Tipo::Dinheiro => {
                        let valor = ler_numero(&texto(&coluna))?;
                        let escolhido = self
                            .operadores
                            .get(&coluna)
                            .and_then(|l| l.read(cx).selected_value().cloned())
                            .unwrap_or_default();
                        let operador = Operador::TODOS
                            .into_iter()
                            .find(|o| simbolo(*o) == escolhido)
                            .unwrap_or_default();
                        FiltroDeColuna::Numero { operador, valor }
                    }
                    Tipo::Data => {
                        let de = ler_dia(self.de.read(cx).value().as_ref());
                        let ate = ler_dia(self.ate.read(cx).value().as_ref());
                        if de.is_none() && ate.is_none() {
                            return None;
                        }
                        FiltroDeColuna::Data { de, ate }
                    }
                    Tipo::Escolha => {
                        let escolhida = self.situacao.read(cx).selected_value().cloned()?;
                        let situacao = Situacao::TODAS
                            .into_iter()
                            .find(|s| s.como_texto() == escolhida)?;
                        FiltroDeColuna::Situacao(situacao)
                    }
                };
                Some((coluna, filtro))
            })
            .collect()
    }

    /// O × ao lado de "Filtros": todos os campos em branco.
    pub fn limpar<T: 'static>(&self, window: &mut Window, cx: &mut gpui_kit::Context<T>) {
        for campo in self.textos.values().chain([&self.de, &self.ate]) {
            campo.update(cx, |estado, cx| estado.trocar_valor("", window, cx));
        }
        for lista in self.operadores.values() {
            lista.update(cx, |estado, cx| {
                estado.set_selected_value(&simbolo(Operador::MaiorIgual), window, cx)
            });
        }
        self.situacao.update(cx, |estado, cx| {
            estado.set_selected_value(&String::new(), window, cx)
        });
    }

    /// O campo da coluna, para a linha sob o cabeçalho.
    pub fn campo(&self, coluna: Coluna) -> gpui_kit::AnyElement {
        let seletor = move || format!("filtro-{coluna:?}");
        match coluna.tipo() {
            Tipo::Texto => gpui_kit::div()
                .w_full()
                .debug_selector(seletor)
                .child(Input::new(&self.textos[&coluna]).xsmall())
                .into_any_element(),
            Tipo::Numero | Tipo::Dinheiro => gpui_kit::div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(2.))
                .debug_selector(seletor)
                .child(
                    gpui_kit::div()
                        .w(px(44.))
                        .flex_none()
                        .child(Select::new(&self.operadores[&coluna]).xsmall()),
                )
                .child(
                    gpui_kit::div()
                        .flex_1()
                        .min_w(px(0.))
                        .child(Input::new(&self.textos[&coluna]).xsmall()),
                )
                .into_any_element(),
            Tipo::Data => gpui_kit::div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(2.))
                .debug_selector(seletor)
                .child(Input::new(&self.de).xsmall())
                .child(Input::new(&self.ate).xsmall())
                .into_any_element(),
            Tipo::Escolha => gpui_kit::div()
                .w_full()
                .debug_selector(seletor)
                .child(Select::new(&self.situacao).xsmall())
                .into_any_element(),
        }
    }

    /// 🧪 Escreve no campo da coluna, como quem digita.
    #[cfg(test)]
    pub fn escrever<T: 'static>(
        &self,
        coluna: Coluna,
        texto: &str,
        window: &mut Window,
        cx: &mut gpui_kit::Context<T>,
    ) {
        let texto = texto.to_string();
        let campo = match coluna.tipo() {
            Tipo::Data => &self.de,
            _ => &self.textos[&coluna],
        };
        campo.update(cx, |estado, cx| estado.trocar_valor(texto, window, cx));
    }

    /// 🧪 Escolhe o operador da coluna numérica.
    #[cfg(test)]
    pub fn escolher_operador<T: 'static>(
        &self,
        coluna: Coluna,
        operador: Operador,
        window: &mut Window,
        cx: &mut gpui_kit::Context<T>,
    ) {
        self.operadores[&coluna].update(cx, |estado, cx| {
            estado.set_selected_value(&simbolo(operador), window, cx)
        });
    }
}
