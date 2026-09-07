use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PresetId(Uuid);

impl PresetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PresetId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PresetId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl std::fmt::Display for PresetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// O que uma predefinição escreve: **alguns** ajustes, e não todos.
///
/// # 🔑 Um mapa esparso, e não uma lista de campos
///
/// Eram 15 `Option<f32>` nomeados — os 11 do Básico e os 4 da curva de tons —,
/// e a lista não cresceu junto com o motor: quando o `Ajustes` chegou a 53
/// campos, uma predefinição continuou incapaz de guardar HSL, nitidez, ruído,
/// lente, tonalização ou grão. **"Sépia à moda antiga" não existia aqui por
/// isso**: a sépia se faz com tonalização, e não havia campo.
///
/// Agora é um mapa `nome → valor`, com os nomes de
/// `revelacao_core::Ajustes::NOMES` — os mesmos que o shader lê por posição, os
/// mesmos que o site manda no `nomes.json`. Um campo que o motor não conhece é
/// **ignorado** na aplicação, e não é erro: é como o site trata o que vem de um
/// `.xmp` do Lightroom com recurso que este motor não tem.
///
/// ⚠️ **O `domain` não conhece os 53 nomes, e é de propósito.** Ele não depende
/// do `revelacao-core` (a regra da camada de dentro), então aqui nome é texto.
/// Quem confere é a borda que aplica — `ui-gpui/src/revelacao/presets.rs` —, e
/// há teste lá de que nenhuma predefinição de sistema escreve num nome que o
/// motor não tenha.
///
/// # ⚠️ Ausente não é neutro
///
/// Campo fora do mapa **não é tocado** ao aplicar: a predefinição diz o que
/// muda, e o resto continua como está. É o que permite somar uma de cor com uma
/// de nitidez. Uma predefinição que queira ser um visual inteiro guarda os 53 —
/// inclusive os que estão no neutro —, e aí aplicar apaga o que havia antes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct PresetAdjustments(BTreeMap<String, f32>);

impl PresetAdjustments {
    /// Uma predefinição que não escreve nada.
    pub fn vazia() -> Self {
        Self::default()
    }

    /// O valor de um campo, se a predefinição o traz.
    pub fn get(&self, campo: &str) -> Option<f32> {
        self.0.get(campo).copied()
    }

    /// Acrescenta ou substitui um campo.
    pub fn com(mut self, campo: impl Into<String>, valor: f32) -> Self {
        self.0.insert(campo.into(), valor);
        self
    }

    /// Quantos campos ela escreve — o número que a lista mostra ao lado do nome.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Os campos e os valores, em ordem de nome.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.0.iter().map(|(nome, valor)| (nome.as_str(), *valor))
    }

    /// Só os nomes.
    pub fn campos(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

impl<N: Into<String>> FromIterator<(N, f32)> for PresetAdjustments {
    fn from_iter<T: IntoIterator<Item = (N, f32)>>(itens: T) -> Self {
        Self(
            itens
                .into_iter()
                .map(|(nome, valor)| (nome.into(), valor))
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: PresetId,
    pub name: String,
    pub adjustments: PresetAdjustments,
    pub is_system: bool,
}

impl Preset {
    pub fn new(name: String, adjustments: PresetAdjustments, is_system: bool) -> Self {
        Self {
            id: PresetId::new(),
            name,
            adjustments,
            is_system,
        }
    }

    pub fn system(name: &str, adjustments: PresetAdjustments) -> Self {
        Self::new(name.to_string(), adjustments, true)
    }

    pub fn user(name: String, adjustments: PresetAdjustments) -> Self {
        Self::new(name, adjustments, false)
    }
}
