//! O que o operador escolheu para os avisos sonoros, guardado ao lado do
//! catálogo (`sons.json`), como o `atendimento.json` e o `tema.json`.
//!
//! 🔑 **Evento que falta no arquivo vem do padrão dele.** Uma versão nova que
//! acrescenta um tipo de aviso não pode desligar os outros nem estragar o
//! arquivo de quem já configurou: por isso o mapa é por **chave de texto**, e
//! uma chave que esta versão não conhece fica guardada sem uso.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Evento, Modo};

/// A escolha de um tipo de aviso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escolha {
    pub ligado: bool,
    #[serde(default)]
    pub modo: Modo,
    /// `sino`, `campainha`… (os de fábrica) ou `meu:<arquivo>` (um som próprio).
    pub som: String,
    /// O texto falado, com `{titulo}` e `{detalhe}`.
    #[serde(default)]
    pub texto: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferencias {
    /// A chave geral: desligada, nenhum aviso toca nem fala.
    pub ligados: bool,
    /// 0,0 a 1,0.
    pub volume: f32,
    /// O id da voz do sistema; `None` é a primeira voz em português.
    pub voz: Option<String>,
    /// 0,5 (devagar) a 2,0 (depressa); 1,0 é o ritmo normal da voz.
    pub ritmo: f32,
    pub por_evento: BTreeMap<String, Escolha>,
}

impl Default for Preferencias {
    fn default() -> Self {
        Self {
            ligados: true,
            volume: 0.7,
            voz: None,
            ritmo: 1.0,
            por_evento: BTreeMap::new(),
        }
    }
}

impl Preferencias {
    /// A escolha do evento: a guardada, ou o padrão dele.
    pub fn escolha(&self, evento: Evento) -> Escolha {
        self.por_evento
            .get(evento.chave())
            .cloned()
            .unwrap_or_else(|| evento.padrao())
    }

    pub fn escolha_mut(&mut self, evento: Evento) -> &mut Escolha {
        self.por_evento
            .entry(evento.chave().to_string())
            .or_insert_with(|| evento.padrao())
    }

    /// Volume e ritmo dentro da faixa — o arquivo é editável à mão.
    fn aparado(mut self) -> Self {
        self.volume = if self.volume.is_finite() {
            self.volume.clamp(0.0, 1.0)
        } else {
            0.7
        };
        self.ritmo = if self.ritmo.is_finite() {
            self.ritmo.clamp(0.5, 2.0)
        } else {
            1.0
        };
        self
    }
}

#[cfg(not(test))]
pub fn arquivo() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("sons.json")
}

/// Nos testes, um arquivo temporário por chamada — teste não grava na
/// preferência de quem roda a suíte.
#[cfg(test)]
pub fn arquivo() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-sons-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Arquivo ausente ou estragado é o padrão.
pub fn ler(arquivo: &Path) -> Preferencias {
    std::fs::read(arquivo)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Preferencias>(&bytes).ok())
        .unwrap_or_default()
        .aparado()
}

/// Falhar em guardar só faz a próxima abertura voltar ao padrão.
pub fn guardar(arquivo: &Path, preferencias: &Preferencias) {
    if let Some(pai) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    if let Ok(texto) = serde_json::to_vec_pretty(preferencias) {
        let _ = std::fs::write(arquivo, texto);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nasce_ligado_e_guarda_o_que_mudou() {
        let caminho = arquivo();
        let lidas = ler(&caminho);
        assert_eq!(lidas, Preferencias::default());
        assert!(lidas.ligados);
        assert_eq!(lidas.escolha(Evento::Falha), Evento::Falha.padrao());

        let mut mudadas = Preferencias {
            ligados: false,
            volume: 0.3,
            voz: Some("Luciana".into()),
            ritmo: 1.2,
            ..Preferencias::default()
        };
        mudadas.escolha_mut(Evento::NovoAgendamento).modo = Modo::SomEVoz;
        guardar(&caminho, &mudadas);
        assert_eq!(ler(&caminho), mudadas);
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn arquivo_estragado_e_o_padrao() {
        let caminho = arquivo();
        std::fs::write(&caminho, b"{estragado").unwrap();
        assert_eq!(ler(&caminho), Preferencias::default());
        let _ = std::fs::remove_file(caminho);
    }

    /// 🔑 O arquivo de uma versão com outro tipo de aviso continua valendo: o
    /// que falta vem do padrão, e a chave desconhecida não derruba o resto.
    #[test]
    fn evento_que_falta_vem_do_padrao_e_o_desconhecido_nao_estraga() {
        let caminho = arquivo();
        std::fs::write(
            &caminho,
            br#"{"volume": 0.4, "por_evento": {
                "falha": {"ligado": false, "som": "sino"},
                "evento_do_futuro": {"ligado": true, "modo": "voz", "som": "x", "texto": "y"}
            }}"#,
        )
        .unwrap();
        let lidas = ler(&caminho);
        assert!(lidas.ligados, "campo que falta vem do padrão");
        assert_eq!(lidas.volume, 0.4);
        let falha = lidas.escolha(Evento::Falha);
        assert!(!falha.ligado);
        assert_eq!(falha.modo, Modo::Som);
        assert_eq!(
            lidas.escolha(Evento::NovaVersao),
            Evento::NovaVersao.padrao()
        );
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn volume_e_ritmo_fora_da_faixa_sao_aparados() {
        let caminho = arquivo();
        std::fs::write(&caminho, br#"{"volume": 7, "ritmo": 0.01}"#).unwrap();
        let lidas = ler(&caminho);
        assert_eq!(lidas.volume, 1.0);
        assert_eq!(lidas.ritmo, 0.5);
        let _ = std::fs::remove_file(caminho);
    }
}
