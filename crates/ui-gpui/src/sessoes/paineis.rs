//! A coluna da direita da galeria: aberta ou recolhida, e o que dentro dela
//! fica aberto — lembrado entre aberturas, como o `PainelColapsavel` do site.
//!
//! 🗂️ **A coluna fica sempre reservada** (dono, 24/set/2026: *"deixa a área
//! aonde ficam as informações da foto e todos os dados pertinentes à
//! negociação reservada e sempre aberta, com opção de minimizá-la — esse
//! padrão é o da indústria de software"*). Ela só aparecia com uma foto em
//! foco, e a grade mudava de largura a cada clique: as colunas pulavam e a foto
//! que se ia clicar saía de baixo do ponteiro. Sem foto, ela mostra os
//! **Atalhos**, como o site; e quem quer a tela inteira para a grade recolhe a
//! coluna numa faixa estreita, e a escolha fica lembrada.
//!
//! ⚠️ É preferência **desta máquina**, num JSON ao lado do catálogo — o mesmo
//! raciocínio da arrumação dos painéis da Revelação (`revelacao-paineis.json`)
//! e do `localStorage` do site: dois balcões arrumam a tela de jeitos
//! diferentes.

use std::collections::HashMap;
use std::path::PathBuf;

/// A coluna aberta: o painel de 300px mais o `gap` do corpo.
pub const LARGURA_ABERTA: f32 = 300.0 + 8.0;
/// A coluna recolhida: a faixa com o botão de abrir, mais o `gap`.
pub const LARGURA_RECOLHIDA: f32 = 36.0 + 8.0;

/// A coluna inteira recolhida?
const CHAVE_DA_COLUNA: &str = "galeria:coluna-recolhida";
/// A seção "Atalhos" — a mesma chave do site (`galeria:atalhos`), fechada por
/// padrão como lá.
const CHAVE_DOS_ATALHOS: &str = "galeria:atalhos";

#[derive(Debug)]
pub struct PaineisDaGaleria {
    abertos: HashMap<String, bool>,
    /// Onde a lembrança mora. `None` nos testes da tela, que nunca tocam o
    /// arquivo de quem trabalha.
    arquivo: Option<PathBuf>,
}

impl Default for PaineisDaGaleria {
    fn default() -> Self {
        Self::do_arquivo(arquivo_da_lembranca())
    }
}

#[cfg(not(test))]
fn arquivo_da_lembranca() -> Option<PathBuf> {
    Some(infrastructure::paths::AppPaths::catalog_root().join("galeria-paineis.json"))
}

#[cfg(test)]
fn arquivo_da_lembranca() -> Option<PathBuf> {
    None
}

impl PaineisDaGaleria {
    fn do_arquivo(arquivo: Option<PathBuf>) -> Self {
        let abertos = arquivo
            .as_ref()
            .and_then(|caminho| std::fs::read_to_string(caminho).ok())
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();
        Self { abertos, arquivo }
    }

    pub fn coluna_recolhida(&self) -> bool {
        self.valor(CHAVE_DA_COLUNA, false)
    }

    pub fn atalhos_abertos(&self) -> bool {
        self.valor(CHAVE_DOS_ATALHOS, false)
    }

    pub fn alternar_coluna(&mut self) {
        self.definir(CHAVE_DA_COLUNA, !self.coluna_recolhida());
    }

    pub fn alternar_atalhos(&mut self) {
        self.definir(CHAVE_DOS_ATALHOS, !self.atalhos_abertos());
    }

    /// O que a coluna tira da largura da grade.
    ///
    /// 🔑 **Não depende de haver foto em foco** — é esse o ponto: focar uma
    /// foto não pode mudar o número de colunas da grade.
    pub fn largura(&self) -> f32 {
        if self.coluna_recolhida() {
            LARGURA_RECOLHIDA
        } else {
            LARGURA_ABERTA
        }
    }

    fn valor(&self, chave: &str, padrao: bool) -> bool {
        self.abertos.get(chave).copied().unwrap_or(padrao)
    }

    /// Guarda. Não poder lembrar não pode impedir de recolher.
    fn definir(&mut self, chave: &str, valor: bool) {
        self.abertos.insert(chave.to_string(), valor);
        let Some(caminho) = self.arquivo.as_ref() else {
            return;
        };
        let Ok(texto) = serde_json::to_string_pretty(&self.abertos) else {
            return;
        };
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        if let Err(erro) = std::fs::write(caminho, texto) {
            crate::telemetria::avisar!("⚠️ [Sessão] a arrumação da coluna não foi gravada: {erro}");
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nasce_com_a_coluna_aberta_e_os_atalhos_fechados_como_no_site() {
        let paineis = PaineisDaGaleria::do_arquivo(None);
        assert!(!paineis.coluna_recolhida());
        assert!(!paineis.atalhos_abertos());
        assert_eq!(paineis.largura(), LARGURA_ABERTA);
    }

    #[test]
    fn recolher_encolhe_a_coluna_e_fica_lembrado() {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let arquivo = pasta.path().join("galeria-paineis.json");
        let mut paineis = PaineisDaGaleria::do_arquivo(Some(arquivo.clone()));
        paineis.alternar_coluna();
        paineis.alternar_atalhos();
        assert_eq!(paineis.largura(), LARGURA_RECOLHIDA);

        let de_novo = PaineisDaGaleria::do_arquivo(Some(arquivo));
        assert!(de_novo.coluna_recolhida(), "a próxima abertura lembra");
        assert!(de_novo.atalhos_abertos());
    }

    #[test]
    fn arquivo_estragado_vale_o_padrao() {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let arquivo = pasta.path().join("galeria-paineis.json");
        std::fs::write(&arquivo, "{ não é json").expect("gravar");
        let paineis = PaineisDaGaleria::do_arquivo(Some(arquivo));
        assert!(!paineis.coluna_recolhida());
    }
}
