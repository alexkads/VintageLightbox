//! A ordem das predefinições na coluna, escolhida por quem opera — o
//! `ordem-dos-presets.ts` do site (dono, 2026-09-12: *"quero conseguir
//! reordenar os presets"*).
//!
//! # Onde ela mora
//!
//! 🔑 **Neste computador, uma lista por grupo** — "Do sistema" e "Minhas" —,
//! num JSON ao lado do catálogo, como a escolha da sincronização. No site é o
//! `localStorage` da máquina, pelo mesmo raciocínio: é arrumação de quem está no
//! balcão, não dado da galeria. Dois operadores em máquinas diferentes põem no
//! topo o que cada um usa.
//!
//! # O que a lista guardada é
//!
//! Só chaves, na ordem escolhida. Ela **não** é a lista de predefinições: uma
//! apagada some da tela mesmo que a chave continue guardada, e uma que chegou
//! depois (importada, criada, ou uma do sistema nova numa versão do app)
//! aparece no fim, na ordem padrão — sem ninguém precisar reordenar tudo.
//!
//! # A chave
//!
//! ⚠️ **As do sistema não têm id estável aqui**: o `use-cases` as monta a cada
//! listagem, com `PresetId` novo. A chave delas é a do site — `sistema:sepia`,
//! `sistema:pb-classico`… —, e a das do operador é o id da linha.

use std::path::{Path, PathBuf};

use domain::entities::Preset;
use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};

/// Os dois grupos da coluna. Cada um reordena só dentro dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Sistema,
    Minhas,
}

impl Grupo {
    /// O grupo de uma predefinição.
    pub fn de(preset: &Preset) -> Self {
        if preset.is_system {
            Grupo::Sistema
        } else {
            Grupo::Minhas
        }
    }
}

/// O id que o site dá a cada predefinição do sistema (`presets-do-sistema.ts`).
const IDS_DO_SISTEMA: &[(&str, &str)] = &[
    ("Preto e branco clássico", "pb-classico"),
    ("Sépia à moda antiga", "sepia"),
    ("Retrato suave", "retrato-suave"),
    ("Luz de estúdio", "luz-de-estudio"),
    ("Hora dourada", "hora-dourada"),
    ("Alta-chave", "alta-chave"),
    ("RecordarFotos P&B", "recordarfotos-pb"),
    ("Nitidez para impressão", "para-impressao"),
];

/// A chave com que a ordem guarda uma predefinição.
pub fn chave(preset: &Preset) -> String {
    if !preset.is_system {
        return preset.id.to_string();
    }
    let id = IDS_DO_SISTEMA
        .iter()
        .find(|(nome, _)| *nome == preset.name)
        .map_or(preset.name.as_str(), |(_, id)| id);
    format!("sistema:{id}")
}

/// A lista na ordem guardada: primeiro as que têm lugar escolhido, depois as
/// que não têm, na ordem em que vieram.
pub fn aplicar_ordem<T>(lista: Vec<T>, ordem: &[String], chave: impl Fn(&T) -> String) -> Vec<T> {
    if ordem.is_empty() {
        return lista;
    }
    let posicao = |item: &T| ordem.iter().position(|c| *c == chave(item));
    let (mut com_lugar, sem_lugar): (Vec<T>, Vec<T>) =
        lista.into_iter().partition(|item| posicao(item).is_some());
    com_lugar.sort_by_key(|item| posicao(item));
    com_lugar.extend(sem_lugar);
    com_lugar
}

/// As chaves com `id` posto antes (ou depois) de `alvo` — o soltar do arrasto.
pub fn mover_para(ids: &[String], id: &str, alvo: &str, depois: bool) -> Vec<String> {
    let tem = |x: &str| ids.iter().any(|i| i == x);
    if id == alvo || !tem(id) || !tem(alvo) {
        return ids.to_vec();
    }
    let mut sem: Vec<String> = ids.iter().filter(|x| *x != id).cloned().collect();
    let indice = sem.iter().position(|x| x == alvo).unwrap_or(0) + usize::from(depois);
    sem.insert(indice, id.to_string());
    sem
}

/// As chaves com `id` uma posição acima (−1) ou abaixo (+1) — o teclado.
pub fn deslocar(ids: &[String], id: &str, passo: i32) -> Vec<String> {
    let mut saida = ids.to_vec();
    let Some(i) = ids.iter().position(|x| x == id) else {
        return saida;
    };
    let j = i as i64 + i64::from(passo);
    if j < 0 || j >= ids.len() as i64 {
        return saida;
    }
    saida.swap(i, j as usize);
    saida
}

/// As duas listas guardadas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ordem {
    #[serde(default)]
    pub sistema: Vec<String>,
    #[serde(default)]
    pub minhas: Vec<String>,
}

impl Ordem {
    pub fn do_grupo(&self, grupo: Grupo) -> &[String] {
        match grupo {
            Grupo::Sistema => &self.sistema,
            Grupo::Minhas => &self.minhas,
        }
    }

    /// Troca a lista de um grupo. `None` (ou lista vazia) volta à ordem padrão.
    pub fn definir(&mut self, grupo: Grupo, ids: Option<Vec<String>>) {
        let ids = ids.unwrap_or_default();
        match grupo {
            Grupo::Sistema => self.sistema = ids,
            Grupo::Minhas => self.minhas = ids,
        }
    }
}

/// Onde a ordem mora: ao lado do catálogo.
pub fn caminho() -> PathBuf {
    AppPaths::catalog_root().join("ordem-dos-presets.json")
}

/// A ordem guardada. JSON estragado ou arquivo ausente valem a ordem padrão.
///
/// ⚠️ **Nos testes não toca o disco**: a suíte não pode ler nem reescrever a
/// arrumação de quem trabalha nesta máquina.
pub fn ler() -> Ordem {
    if cfg!(test) {
        return Ordem::default();
    }
    ler_de(&caminho()).unwrap_or_default()
}

pub fn ler_de(caminho: &Path) -> Option<Ordem> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Grava a ordem. Falhar só custa a arrumação ao reabrir o app.
pub fn gravar(ordem: &Ordem) {
    if cfg!(test) {
        return;
    }
    gravar_em(&caminho(), ordem);
}

pub fn gravar_em(caminho: &Path, ordem: &Ordem) {
    if ordem.sistema.is_empty() && ordem.minhas.is_empty() {
        let _ = std::fs::remove_file(caminho);
        return;
    }
    let Ok(texto) = serde_json::to_string_pretty(ordem) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(caminho, texto) {
        eprintln!("⚠️ [Presets] a ordem não foi guardada: {erro}");
    }
}

#[cfg(test)]
mod testes {
    use domain::entities::preset::PresetAdjustments;

    use super::*;

    fn lista(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn ordenar(itens: &[&str], ordem: &[&str]) -> Vec<String> {
        aplicar_ordem(lista(itens), &lista(ordem), Clone::clone)
    }

    // ------------------------------------------ a ordem guardada (site)

    #[test]
    fn sem_ordem_a_lista_fica_como_veio() {
        assert_eq!(ordenar(&["a", "b", "c"], &[]), lista(&["a", "b", "c"]));
    }

    #[test]
    fn as_que_tem_lugar_escolhido_vem_primeiro_na_ordem_escolhida() {
        assert_eq!(
            ordenar(&["a", "b", "c", "d"], &["c", "a"]),
            lista(&["c", "a", "b", "d"])
        );
    }

    /// 🔑 Importar dez presets não pode obrigar a reordenar a lista inteira, e
    /// apagar um não pode deixar buraco: a ordem guardada é só uma preferência
    /// sobre a lista que existe.
    #[test]
    fn a_que_chegou_depois_vai_para_o_fim_e_a_apagada_nao_volta() {
        assert_eq!(
            ordenar(&["b", "novo", "a"], &["a", "apagada", "b"]),
            lista(&["a", "b", "novo"])
        );
    }

    // -------------------------------------------------------- mover (site)

    #[test]
    fn solta_antes_ou_depois_do_alvo() {
        let ids = lista(&["a", "b", "c", "d"]);
        assert_eq!(
            mover_para(&ids, "d", "b", false),
            lista(&["a", "d", "b", "c"])
        );
        assert_eq!(
            mover_para(&ids, "a", "c", true),
            lista(&["b", "c", "a", "d"])
        );
    }

    #[test]
    fn soltar_sobre_si_mesma_ou_com_id_desconhecido_nao_muda_nada() {
        let ids = lista(&["a", "b"]);
        assert_eq!(mover_para(&ids, "a", "a", true), ids);
        assert_eq!(mover_para(&ids, "x", "a", true), ids);
    }

    #[test]
    fn pelo_teclado_anda_uma_posicao_e_para_nas_pontas() {
        let ids = lista(&["a", "b", "c"]);
        assert_eq!(deslocar(&ids, "b", -1), lista(&["b", "a", "c"]));
        assert_eq!(deslocar(&ids, "b", 1), lista(&["a", "c", "b"]));
        assert_eq!(deslocar(&ids, "a", -1), ids);
        assert_eq!(deslocar(&ids, "c", 1), ids);
    }

    // ------------------------------------------------------- daqui

    /// ⚠️ **A chave das do sistema é a do site, e não o id da entidade** — que
    /// nasce novo a cada listagem. Com o id, a ordem escolhida sumiria ao
    /// reabrir o app.
    #[test]
    fn a_chave_das_do_sistema_sobrevive_a_uma_nova_listagem() {
        let primeira = use_cases::presets::presets_de_sistema();
        let segunda = use_cases::presets::presets_de_sistema();
        for (a, b) in primeira.iter().zip(&segunda) {
            assert_ne!(a.id, b.id, "o id muda a cada listagem");
            assert_eq!(chave(a), chave(b));
            assert!(chave(a).starts_with("sistema:"));
        }
        assert_eq!(chave(&primeira[1]), "sistema:sepia");
        // Toda do sistema tem id do site: um nome novo sem linha na tabela
        // guardaria a ordem pelo nome, e renomeá-la a perderia.
        for preset in &primeira {
            assert!(
                IDS_DO_SISTEMA.iter().any(|(nome, _)| *nome == preset.name),
                "\"{}\" não tem id do site",
                preset.name
            );
        }

        let minha = Preset::user("Minha".into(), PresetAdjustments::vazia());
        assert_eq!(chave(&minha), minha.id.to_string());
    }

    #[test]
    fn a_ordem_sobrevive_ao_disco_e_o_lixo_vira_padrao() {
        let dir = tempfile::tempdir().expect("pasta temporária");
        let arquivo = dir.path().join("ordem-dos-presets.json");
        assert_eq!(ler_de(&arquivo), None);

        let mut ordem = Ordem::default();
        ordem.definir(Grupo::Minhas, Some(lista(&["b", "a"])));
        gravar_em(&arquivo, &ordem);
        assert_eq!(ler_de(&arquivo), Some(ordem.clone()));

        // Voltar à ordem padrão nos dois grupos apaga o arquivo.
        ordem.definir(Grupo::Minhas, None);
        gravar_em(&arquivo, &ordem);
        assert!(!arquivo.exists());

        std::fs::write(&arquivo, "{ isto não é json").expect("gravar");
        assert_eq!(ler_de(&arquivo), None);
    }
}
