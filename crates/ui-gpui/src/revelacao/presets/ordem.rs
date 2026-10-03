//! A ordem das predefinições na coluna, escolhida por quem opera — o
//! `ordem-dos-presets.ts` do site (dono, 2026-09-12: *"quero conseguir
//! reordenar os presets"*).
//!
//! # Onde ela mora
//!
//! 🔑 **Neste computador, uma lista por grupo** — "Do sistema", "Minhas" e
//! "LRs" —, num JSON ao lado do catálogo, como a escolha da sincronização. No
//! site é o `localStorage` da máquina, pelo mesmo raciocínio: é arrumação de
//! quem está no balcão, não dado da galeria.
//!
//! 💛 **As "Favoritas" são do perfil do usuário, na API** (dono, 2026-10-02:
//! *"A favoritação de presets precisa gravar no banco de dados no perfil do
//! usuário da API e não só localmente"*): `GET`/`PUT /revelacao/favoritas`,
//! a mesma lista do site. O JSON daqui guarda a última que veio — a coluna abre
//! com ela antes de a conta entrar, e a Nova sessão a lê sem rede — e quem fala
//! com a API é a raiz (`app/favoritas.rs`).
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

/// Os grupos da coluna. Cada um reordena só dentro dele.
///
/// 💛 **"Favoritas" é um grupo como os outros** (dono, 2026-09-29: *"clicar num
/// coraçãozinho deixando os preferidos primeiro"*): a lista guardada dele é ao
/// mesmo tempo **quais** são as favoritas e **em que ordem**. O coração põe no
/// fim; o arrasto reordena dentro dela. Uma favorita sai do grupo de origem
/// enquanto estiver lá — o `ordem-dos-presets.ts` do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Favoritas,
    Sistema,
    Minhas,
    /// 🎞️ As predefinições do Lightroom do estúdio (`presets_do_lightroom`).
    Lrs,
}

impl Grupo {
    /// O nome com que o grupo recolhido fica guardado.
    pub fn chave(self) -> &'static str {
        match self {
            Grupo::Favoritas => "favoritas",
            Grupo::Sistema => "sistema",
            Grupo::Minhas => "minhas",
            Grupo::Lrs => "lrs",
        }
    }

    /// O grupo de uma predefinição.
    pub fn de(preset: &Preset) -> Self {
        if !preset.is_system {
            Grupo::Minhas
        } else if preset.grupo.as_deref() == Some(use_cases::presets::list_presets::GRUPO_LRS) {
            Grupo::Lrs
        } else {
            Grupo::Sistema
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
    ("Vintage · Foto envelhecida", "vintage-envelhecida"),
    ("Vintage · Polaroid antiga", "vintage-polaroid"),
    ("Vintage · Anos passados", "vintage-anos-passados"),
    ("Vintage · Processo cruzado", "vintage-processo-cruzado"),
    ("Vintage · Cianótipo", "vintage-cianotipo"),
    ("Vintage · Cinza antigo", "vintage-cinza-antigo"),
    ("Vintage · Bleach bypass", "vintage-bleach-bypass"),
    ("Vintage · Positivo direto", "vintage-positivo-direto"),
    ("Vintage · Kodachrome", "vintage-kodachrome"),
    ("Vintage · Portra 400", "vintage-portra"),
    ("Vintage · Ektachrome anos 70", "vintage-ektachrome"),
    ("Vintage · Desbotado anos 70", "vintage-desbotado-70"),
    ("Nitidez para impressão", "para-impressao"),
    ("Cinematográfico P&B", "cinematografico-pb"),
];

/// O id do site de uma predefinição da pasta "LRs": `lr-` e o nome sem acento,
/// em minúsculas, com hífen — o `idDoLightroom` de `presets-do-sistema.ts`.
///
/// 🔑 **Pelo nome do arquivo, e não pela tabela acima**: "RecordarFotos P&B"
/// existe nas duas pastas (a refeita do darktable no sistema, a do Lightroom nas LRs),
/// e o nome sozinho as confundiria.
pub fn id_do_lightroom(nome: &str) -> String {
    let mut id = String::from("lr-");
    let mut hifen = false;
    for c in nome.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            if hifen && id.len() > 3 {
                id.push('-');
            }
            hifen = false;
            id.push(c);
        } else {
            hifen = true;
        }
    }
    id
}

/// O id que o site dá a uma predefinição do sistema, pelo nome — `sepia` para
/// "Sépia à moda antiga". É a **única** tabela do app: a Nova sessão também
/// lê daqui (era uma cópia com oito linhas, e as doze Vintage teriam ficado de
/// fora do preset padrão sem aviso nenhum).
pub fn id_do_site(nome: &str) -> Option<&'static str> {
    IDS_DO_SISTEMA
        .iter()
        .find(|(n, _)| *n == nome)
        .map(|(_, id)| *id)
}

/// A chave com que a ordem guarda uma predefinição.
pub fn chave(preset: &Preset) -> String {
    if !preset.is_system {
        return preset.id.to_string();
    }
    if Grupo::de(preset) == Grupo::Lrs {
        return format!("sistema:{}", id_do_lightroom(&preset.name));
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

/// A chave do Navegador entre os recolhidos — ao lado das dos grupos.
const NAVEGADOR: &str = "navegador";
/// A chave do painel Histórico, no pé da coluna.
const HISTORICO: &str = "historico";

/// As listas guardadas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ordem {
    #[serde(default)]
    pub sistema: Vec<String>,
    #[serde(default)]
    pub minhas: Vec<String>,
    #[serde(default)]
    pub lrs: Vec<String>,
    /// As chaves com o coração aceso, na ordem em que aparecem no topo.
    #[serde(default)]
    pub favoritas: Vec<String>,
    /// 📁 Os grupos fechados na coluna — a pasta recolhida do Lightroom
    /// (dono, 2026-09-30: *"essa listagem de preset tá ruim de usar"*) —, e o
    /// Navegador, se estiver recolhido.
    #[serde(default)]
    pub recolhidos: Vec<String>,
    /// 💛 As favoritas desta máquina já estão no perfil da API. Antes disso,
    /// a lista daqui sobe uma vez (a de quem favoritava antes de a API
    /// guardar); depois, quem manda é a API — "tirei todas no site" não pode
    /// trazer de volta a lista velha deste computador.
    #[serde(default)]
    pub favoritas_no_perfil: bool,
}

/// O que fazer com a lista de favoritas que veio do perfil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FavoritasDoPerfil {
    /// O perfil está vazio e esta máquina tem a lista de antes: ela sobe.
    Subir(Vec<String>),
    /// A do perfil vale, aqui e na tela.
    Adotar(Vec<String>),
}

impl Ordem {
    /// 💛 A lista do perfil contra a desta máquina — a migração de uma vez só.
    pub fn favoritas_do_perfil(&self, do_perfil: Vec<String>) -> FavoritasDoPerfil {
        if do_perfil.is_empty() && !self.favoritas.is_empty() && !self.favoritas_no_perfil {
            FavoritasDoPerfil::Subir(self.favoritas.clone())
        } else {
            FavoritasDoPerfil::Adotar(do_perfil)
        }
    }
}

impl Ordem {
    pub fn do_grupo(&self, grupo: Grupo) -> &[String] {
        match grupo {
            Grupo::Favoritas => &self.favoritas,
            Grupo::Sistema => &self.sistema,
            Grupo::Minhas => &self.minhas,
            Grupo::Lrs => &self.lrs,
        }
    }

    /// Troca a lista de um grupo. `None` (ou lista vazia) volta à ordem padrão
    /// — e, nas favoritas, a nenhuma.
    pub fn definir(&mut self, grupo: Grupo, ids: Option<Vec<String>>) {
        let ids = ids.unwrap_or_default();
        match grupo {
            Grupo::Favoritas => self.favoritas = ids,
            Grupo::Sistema => self.sistema = ids,
            Grupo::Minhas => self.minhas = ids,
            Grupo::Lrs => self.lrs = ids,
        }
    }

    pub fn eh_favorita(&self, chave: &str) -> bool {
        self.favoritas.iter().any(|c| c == chave)
    }

    pub fn recolhido(&self, grupo: Grupo) -> bool {
        self.recolhido_por_chave(grupo.chave())
    }

    /// Abre ou fecha a pasta de um grupo.
    pub fn alternar_recolhido(&mut self, grupo: Grupo) {
        self.alternar_por_chave(grupo.chave());
    }

    /// 🔍 O Navegador recolhido no topo da coluna (dono, 2026-09-30: *"dê a
    /// opção de recolher o painel do zoom"*). Mora na mesma lista das pastas:
    /// é a mesma arrumação da coluna, deste computador.
    pub fn navegador_recolhido(&self) -> bool {
        self.recolhido_por_chave(NAVEGADOR)
    }

    pub fn alternar_navegador(&mut self) {
        self.alternar_por_chave(NAVEGADOR);
    }

    /// 📜 O painel Histórico recolhido no pé da coluna — a mesma arrumação
    /// deste computador que o Navegador.
    pub fn historico_recolhido(&self) -> bool {
        self.recolhido_por_chave(HISTORICO)
    }

    pub fn alternar_historico(&mut self) {
        self.alternar_por_chave(HISTORICO);
    }

    fn recolhido_por_chave(&self, chave: &str) -> bool {
        self.recolhidos.iter().any(|c| c == chave)
    }

    fn alternar_por_chave(&mut self, chave: &str) {
        if self.recolhido_por_chave(chave) {
            self.recolhidos.retain(|c| c != chave);
        } else {
            self.recolhidos.push(chave.to_string());
        }
    }

    /// Liga ou desliga o coração: entra no fim das favoritas, ou sai delas.
    pub fn alternar_favorita(&mut self, chave: &str) {
        if self.eh_favorita(chave) {
            self.favoritas.retain(|c| c != chave);
        } else {
            self.favoritas.push(chave.to_string());
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
    if *ordem == Ordem::default() {
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
        crate::telemetria::avisar!("⚠️ [Presets] a ordem não foi guardada: {erro}");
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

    // ----------------------------------------------------- favoritas (site)

    #[test]
    fn o_coracao_poe_no_fim_das_favoritas_e_o_segundo_clique_tira() {
        let mut ordem = Ordem::default();
        ordem.alternar_favorita("c");
        ordem.alternar_favorita("a");
        assert_eq!(ordem.favoritas, lista(&["c", "a"]));
        ordem.alternar_favorita("c");
        assert_eq!(ordem.favoritas, lista(&["a"]));
        assert!(ordem.eh_favorita("a") && !ordem.eh_favorita("c"));
    }

    /// 💛 A lista de antes sobe uma vez; depois, o perfil manda — mesmo
    /// vazio.
    #[test]
    fn a_lista_desta_maquina_sobe_uma_vez_e_depois_o_perfil_manda() {
        let mut ordem = Ordem::default();
        ordem.alternar_favorita("sistema:sepia");
        assert_eq!(
            ordem.favoritas_do_perfil(vec![]),
            FavoritasDoPerfil::Subir(lista(&["sistema:sepia"]))
        );
        assert_eq!(
            ordem.favoritas_do_perfil(lista(&["sistema:pb-classico"])),
            FavoritasDoPerfil::Adotar(lista(&["sistema:pb-classico"])),
            "o perfil com lista ganha da máquina"
        );
        ordem.favoritas_no_perfil = true;
        assert_eq!(
            ordem.favoritas_do_perfil(vec![]),
            FavoritasDoPerfil::Adotar(vec![]),
            "tirou todas em outro lugar: aqui também"
        );
        assert_eq!(
            Ordem::default().favoritas_do_perfil(vec![]),
            FavoritasDoPerfil::Adotar(vec![])
        );
    }

    /// Um arquivo gravado antes do coração não tem `favoritas`: abre sem
    /// nenhuma, e sem perder a ordem que já tinha.
    #[test]
    fn a_ordem_de_antes_do_coracao_abre_sem_favoritas() {
        let ordem: Ordem = serde_json::from_str(r#"{"sistema":["sistema:sepia"],"minhas":[]}"#)
            .expect("o formato de antes");
        assert_eq!(ordem.sistema, lista(&["sistema:sepia"]));
        assert!(ordem.favoritas.is_empty());
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
        let lr = primeira
            .iter()
            .find(|p| p.name == "RecordarFotos P&B" && Grupo::de(p) == Grupo::Lrs)
            .expect("a do Lightroom");
        assert_eq!(
            chave(lr),
            "sistema:lr-recordarfotos-p-b",
            "não é a do darktable"
        );
        // Toda do sistema tem id do site: um nome novo sem linha na tabela
        // guardaria a ordem pelo nome, e renomeá-la a perderia.
        // As da pasta "LRs" têm id pelo nome do arquivo (`id_do_lightroom`).
        for preset in primeira.iter().filter(|p| Grupo::de(p) == Grupo::Sistema) {
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
