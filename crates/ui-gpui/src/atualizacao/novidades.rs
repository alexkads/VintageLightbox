//! 📣 As novidades da versão nova — e o aviso para quem instalou **compilando**.
//!
//! ## O buraco que isto fecha
//!
//! A faixa de atualização só enxergava o manifesto dos **pacotes**
//! (`latest.json`), e em 25/set/2026 ele só tinha o macOS. A maior parte dos
//! balcões — Windows e Linux — instalou pelo `instalar-vintagelightbox-gpui.cmd`,
//! que **compila o `main`**: para eles não havia pacote, e portanto nunca havia
//! aviso. O Chatbot e os Agendamentos chegaram ao `main` sem que esses balcões
//! tivessem como saber (dono, 25/set: *"vai ter que implementar essa
//! comunicação no app dizendo que tem uma versão nova, e mostrar quais são as
//! novidades e por que ela é tão importante"*).
//!
//! ## O que é "versão nova" para cada instalação
//!
//! 🔑 **A versão nova é a do `main`, para todo mundo.** Lançar pacote exige a
//! máquina de cada sistema; esperar por isso deixava o balcão sem saber
//! (dono: *"não posso ficar dependendo de lembrar de compilar e colocar lá"*).
//!
//! | Instalou por | Há pacote da versão no `latest.json`? | "Atualizar" faz |
//! |---|---|---|
//! | pacote (`release`) | sim | baixa e instala o pacote assinado |
//! | pacote (`release`) | não | roda o instalador, que compila o `main` |
//! | instalador que compila (`instalador`) | — | roda o instalador de novo |
//!
//! A instalação compilada **nunca** recebe pacote: ele cairia ao lado da
//! compilada (Windows) ou por cima dela (Linux), o caso que o README do
//! empacotamento proíbe.
//!
//! 🔑 **O `novidades.json` anda com o `Cargo.toml`.** Um teste prende a versão
//! dele à `CARGO_PKG_VERSION`: subir a versão sem escrever as novidades (ou o
//! contrário) não passa. É o mesmo commit de "Versão 0.1.N".
//!
//! ## 📚 O histórico
//!
//! O `novidades.json` só fala da última versão. O diálogo das novidades
//! navega por todas (dono, 01/out/2026: *"pra gente conseguir navegar pelo
//! histórico de novidades anteriores"*), e elas moram no
//! [`ARQUIVO_DO_HISTORICO`]: uma lista, da mais nova para a mais antiga, que
//! **começa pela mesma entrada do `novidades.json`** — o teste
//! `o_historico_comeca_pelas_novidades_desta_versao` prende os dois. Lançar é
//! acrescentar a entrada nova no topo dele também.
//!
//! O app traz o histórico do próprio commit e, quando há versão nova, busca o
//! do `main`: é dele que vêm as versões entre a instalada e a anunciada.

use serde::Deserialize;

use super::porta::{JeitoDeAtualizar, VersaoNova};

/// O arquivo no repositório. Serve às duas instalações: a compilada o lê no
/// `main`, e a do pacote o usa para mostrar o que mudou.
pub const ARQUIVO: &str = "docs/novidades.json";

/// Onde o app lê as novidades, em ordem.
///
/// - o `raw` do GitHub vale no instante em que o `main` anda (o `make mains`);
/// - o GitHub Pages é a cópia, para quando o `raw` não responder.
///
/// ⚠️ Como os endereços de atualização, **só se acrescenta**: o app instalado
/// só conhece os endereços com que foi compilado.
pub const ENDERECOS: [&str; 2] = [
    "https://raw.githubusercontent.com/alexkads/VintageLightbox/main/docs/novidades.json",
    "https://alexkads.github.io/VintageLightbox/novidades.json",
];

/// O que o `novidades.json` diz, escrito para o operador do balcão.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Novidades {
    pub versao: String,
    /// O dia do lançamento (`2026-10-01`) — só o histórico o tem.
    #[serde(default)]
    pub data: Option<String>,
    /// Uma linha: o que o operador vai notar primeiro.
    pub titulo: String,
    /// Atualização que não deve esperar — a faixa fica em destaque.
    #[serde(default)]
    pub importante: bool,
    /// O que mudou, um item por linha.
    pub novidades: Vec<String>,
    /// Por que vale atualizar agora.
    pub por_que_atualizar: String,
}

/// Lê o arquivo. Torto, sem versão ou sem título é `None`: melhor não avisar
/// do que avisar em branco.
pub fn ler(texto: &str) -> Option<Novidades> {
    let novidades: Novidades = serde_json::from_str(texto).ok()?;
    valida(&novidades).then_some(novidades)
}

fn valida(novidades: &Novidades) -> bool {
    versao_em_numeros(&novidades.versao).is_some() && !novidades.titulo.trim().is_empty()
}

/// 📚 O arquivo com todas as versões, da mais nova para a mais antiga.
pub const ARQUIVO_DO_HISTORICO: &str = "docs/historico-de-novidades.json";

/// Onde o app busca o histórico do `main` — os mesmos lugares de
/// [`ENDERECOS`], e com a mesma regra: **só se acrescenta**.
pub const ENDERECOS_DO_HISTORICO: [&str; 2] = [
    "https://raw.githubusercontent.com/alexkads/VintageLightbox/main/docs/historico-de-novidades.json",
    "https://alexkads.github.io/VintageLightbox/historico-de-novidades.json",
];

/// Lê o histórico. Uma entrada torta fica de fora sem levar as outras junto.
pub fn ler_historico(texto: &str) -> Vec<Novidades> {
    let itens: Vec<serde_json::Value> = serde_json::from_str(texto).unwrap_or_default();
    juntar(
        itens
            .into_iter()
            .filter_map(|item| serde_json::from_value::<Novidades>(item).ok())
            .filter(valida),
    )
}

/// O histórico **até esta versão**, gravado no binário na compilação — lido
/// uma vez só.
pub fn historico_desta_versao() -> &'static [Novidades] {
    static HISTORICO: std::sync::OnceLock<Vec<Novidades>> = std::sync::OnceLock::new();
    HISTORICO
        .get_or_init(|| ler_historico(include_str!("../../../../docs/historico-de-novidades.json")))
}

/// Junta versões de vários lugares: uma por número, da mais nova para a mais
/// antiga. Repetida, fica a que tem a lista — a do anúncio do servidor vem
/// sem ela.
pub fn juntar(itens: impl IntoIterator<Item = Novidades>) -> Vec<Novidades> {
    let mut todas: Vec<Novidades> = Vec::new();
    for n in itens {
        match todas
            .iter_mut()
            .find(|t| comparar(&t.versao, &n.versao) == std::cmp::Ordering::Equal)
        {
            Some(t) => {
                if t.novidades.is_empty() && !n.novidades.is_empty() {
                    let data = t.data.take();
                    *t = n;
                    t.data = t.data.take().or(data);
                } else if t.data.is_none() {
                    t.data = n.data;
                }
            }
            None => todas.push(n),
        }
    }
    todas.sort_by(|a, b| comparar(&b.versao, &a.versao));
    todas
}

/// As novidades **desta** versão, gravadas no binário na compilação.
///
/// 🔑 **É o arquivo do próprio commit**, e não o do `main`: o rodapé mostra o
/// que o operador tem nas mãos agora, e o teste
/// `o_novidades_json_anda_com_a_versao_do_cargo` garante que ele fala da
/// `CARGO_PKG_VERSION`.
pub fn desta_versao() -> Option<Novidades> {
    ler(include_str!("../../../../docs/novidades.json"))
}

pub fn versao_em_numeros(versao: &str) -> Option<Vec<u64>> {
    let limpa = versao.trim().trim_start_matches('v');
    let numeros: Option<Vec<u64>> = limpa
        .split('.')
        .map(|parte| parte.parse::<u64>().ok())
        .collect();
    numeros.filter(|n| !n.is_empty())
}

/// A `candidata` é maior que a `atual`? `0.1.13 > 0.1.9` (número a número,
/// e não como texto). Ilegível nunca é "mais nova".
pub fn mais_nova(candidata: &str, atual: &str) -> bool {
    versao_em_numeros(candidata).is_some()
        && versao_em_numeros(atual).is_some()
        && comparar(candidata, atual) == std::cmp::Ordering::Greater
}

/// Ordena duas versões número a número (`0.1` é `0.1.0`). Ilegível fica
/// abaixo de tudo.
pub fn comparar(a: &str, b: &str) -> std::cmp::Ordering {
    match (versao_em_numeros(a), versao_em_numeros(b)) {
        (Some(mut a), Some(mut b)) => {
            let tamanho = a.len().max(b.len());
            a.resize(tamanho, 0);
            b.resize(tamanho, 0);
            a.cmp(&b)
        }
        (a, b) => a.is_some().cmp(&b.is_some()),
    }
}

/// Como este binário foi instalado — gravado nele pelo `build.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JeitoDaInstalacao {
    /// O instalador do balcão, que compila o `main` (perfil `instalador`).
    Compilado,
    /// O pacote assinado (perfil `release`).
    Pacote,
    /// Quem desenvolve (`debug`).
    Desenvolvimento,
}

impl JeitoDaInstalacao {
    pub fn do_texto(texto: &str) -> JeitoDaInstalacao {
        match texto {
            "compilado" => JeitoDaInstalacao::Compilado,
            "pacote" => JeitoDaInstalacao::Pacote,
            _ => JeitoDaInstalacao::Desenvolvimento,
        }
    }
}

pub fn jeito_desta_instalacao() -> JeitoDaInstalacao {
    JeitoDaInstalacao::do_texto(env!("VLB_JEITO_DE_INSTALAR"))
}

impl Novidades {
    /// 📡 A versão que o servidor anunciou pelo SSE: número, título e
    /// destaque, sem a lista (que só o arquivo tem).
    pub fn do_anuncio(versao: String, titulo: String, importante: bool) -> Self {
        Self {
            versao,
            data: None,
            titulo,
            importante,
            novidades: Vec::new(),
            por_que_atualizar: String::new(),
        }
    }
}

/// 📡 **O anúncio do servidor vale por si** — como o Zed, que pergunta ao
/// próprio servidor (`/releases/stable/latest`) e não a um arquivo em CDN.
///
/// 🚨 O servidor e o app leem o `novidades.json` em pontas diferentes do
/// cache do `raw` do GitHub (5 minutos cada). Em 29/09 o servidor, nos EUA,
/// viu a 0.1.45 e anunciou às 21:52; o Mac do dono, perguntando em São Paulo
/// segundos depois, recebeu a 0.1.44 e ficou em silêncio. As máquinas que
/// reconectaram o fluxo três minutos depois receberam o anúncio de novo, já
/// com o cache em dia — e foi só por isso que a rotina "sempre funcionou".
///
/// Fica a mais nova das duas; empatadas, a do arquivo, que tem a lista.
pub fn a_mais_nova(arquivo: Option<Novidades>, anunciada: Option<Novidades>) -> Option<Novidades> {
    match (arquivo, anunciada) {
        (Some(a), Some(n)) if mais_nova(&n.versao, &a.versao) => Some(n),
        (Some(a), _) => Some(a),
        (None, n) => n,
    }
}

/// O aviso para quem instalou compilando: há versão nova no `main`?
pub fn aviso_do_main(atual: &str, novidades: Option<Novidades>) -> Option<VersaoNova> {
    let novidades = novidades?;
    if !mais_nova(&novidades.versao, atual) {
        return None;
    }
    Some(VersaoNova {
        versao: novidades.versao.clone(),
        notas: Some(novidades.titulo.clone()),
        jeito: JeitoDeAtualizar::Compilar,
        novidades: Some(novidades),
    })
}

/// As novidades só acompanham o pacote da **mesma** versão: as de outra
/// versão contariam o que o pacote não traz.
pub fn para_o_pacote(versao_do_pacote: &str, novidades: Option<Novidades>) -> Option<Novidades> {
    novidades.filter(|n| {
        versao_em_numeros(&n.versao).is_some()
            && versao_em_numeros(&n.versao) == versao_em_numeros(versao_do_pacote)
    })
}

/// Busca o arquivo nos [`ENDERECOS`], em ordem. **Bloqueia**: chamar fora da
/// thread da interface (a procura já roda numa thread própria).
pub fn buscar() -> Option<Novidades> {
    buscar_com_rastro().0
}

/// 🔎 [`buscar`], contando o que cada endereço respondeu e em quanto tempo —
/// o que vai no `procurou` para o servidor (`raw 0.1.44 em 180 ms`).
pub fn buscar_com_rastro() -> (Option<Novidades>, Vec<String>) {
    let mut rastro = Vec::new();
    let Ok(cliente) = cargo_packager_updater::reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    else {
        rastro.push("sem cliente HTTP".into());
        return (None, rastro);
    };
    for endereco in ENDERECOS {
        let nome = if endereco.contains("raw.githubusercontent") {
            "raw"
        } else {
            "pages"
        };
        let inicio = std::time::Instant::now();
        let resposta = cliente.get(endereco).send();
        let ms = inicio.elapsed().as_millis();
        match resposta {
            Err(erro) => rastro.push(format!("{nome}: sem resposta em {ms} ms ({erro})")),
            Ok(r) if !r.status().is_success() => {
                eprintln!("[novidades] {endereco} respondeu {}", r.status());
                rastro.push(format!("{nome}: HTTP {} em {ms} ms", r.status().as_u16()));
            }
            Ok(r) => {
                // O `raw` do GitHub conta a idade da cópia em `source-age`.
                let idade = ["source-age", "age"]
                    .iter()
                    .find_map(|c| r.headers().get(*c))
                    .and_then(|v| v.to_str().ok())
                    .map(|v| format!(", cache de {v} s"))
                    .unwrap_or_default();
                match r.text().ok().as_deref().and_then(ler) {
                    Some(n) => {
                        rastro.push(format!("{nome} {} em {ms} ms{idade}", n.versao));
                        return (Some(n), rastro);
                    }
                    None => rastro.push(format!("{nome}: arquivo ilegível em {ms} ms")),
                }
            }
        }
    }
    (None, rastro)
}

/// 📚 Busca o histórico do `main` nos [`ENDERECOS_DO_HISTORICO`], em ordem.
/// Vazio quando nenhum respondeu. **Bloqueia**, como [`buscar`].
pub fn buscar_historico() -> Vec<Novidades> {
    let Ok(cliente) = cargo_packager_updater::reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    else {
        return Vec::new();
    };
    for endereco in ENDERECOS_DO_HISTORICO {
        let historico = cliente
            .get(endereco)
            .send()
            .ok()
            .filter(|r| r.status().is_success())
            .and_then(|r| r.text().ok())
            .map(|texto| ler_historico(&texto))
            .unwrap_or_default();
        if !historico.is_empty() {
            return historico;
        }
        eprintln!("[novidades] o histórico não veio de {endereco}");
    }
    Vec::new()
}

/// O que o operador faz para atualizar, neste sistema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComoAtualizar {
    pub passos: Vec<&'static str>,
    /// Se a compilação pelo app falhar: a linha para colar no Terminal (Linux
    /// e macOS).
    pub comando: Option<&'static str>,
    /// Se a compilação pelo app falhar: de onde baixar o instalador (Windows).
    pub baixar: Option<&'static str>,
}

/// A linha de instalação do Linux e do macOS — a mesma do site de documentação.
pub const COMANDO_DO_INSTALADOR: &str = "curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/main/scripts/instalar-vintagelightbox-gpui.cmd | sh";

/// O instalador do Windows — o mesmo link do site de documentação.
pub const INSTALADOR_DO_WINDOWS: &str = "https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-gpui.cmd";

pub fn como_atualizar(jeito: JeitoDeAtualizar) -> ComoAtualizar {
    match jeito {
        JeitoDeAtualizar::Pacote => ComoAtualizar {
            passos: vec![
                "Clique em Atualizar: o app baixa a versão nova e confere a assinatura antes de instalar.",
                "No fim, clique em Reabrir agora.",
            ],
            comando: None,
            baixar: None,
        },
        // O app roda o instalador sozinho; o comando e o link ficam como o
        // caminho à mão, para quando a compilação pelo app falhar.
        JeitoDeAtualizar::Compilar => ComoAtualizar {
            passos: vec![
                "Clique em Atualizar: o app baixa o código da versão nova e compila nesta máquina, com o mesmo instalador de sempre.",
                "Leva alguns minutos. Você pode continuar trabalhando enquanto isso.",
                "No fim, clique em Reabrir agora. O catálogo e as fotos continuam lá.",
            ],
            comando: (!cfg!(target_os = "windows")).then_some(COMANDO_DO_INSTALADOR),
            baixar: cfg!(target_os = "windows").then_some(INSTALADOR_DO_WINDOWS),
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const EXEMPLO: &str = r#"{
        "versao": "0.1.13",
        "titulo": "Chatbot e Agendamentos",
        "importante": true,
        "novidades": ["O chatbot no app", "A agenda no app"],
        "por_que_atualizar": "Para não perder cliente."
    }"#;

    #[test]
    fn le_o_arquivo_e_recusa_o_torto() {
        let n = ler(EXEMPLO).unwrap();
        assert_eq!(n.versao, "0.1.13");
        assert!(n.importante);
        assert_eq!(n.novidades.len(), 2);
        assert!(ler("{").is_none());
        assert!(
            ler(r#"{"versao":"x","titulo":"t","novidades":[],"por_que_atualizar":""}"#).is_none()
        );
        assert!(
            ler(r#"{"versao":"1.0","titulo":"  ","novidades":[],"por_que_atualizar":""}"#)
                .is_none()
        );
        let sem_importante =
            ler(r#"{"versao":"1.0","titulo":"t","novidades":[],"por_que_atualizar":"p"}"#).unwrap();
        assert!(!sem_importante.importante, "ausente é não importante");
    }

    #[test]
    fn compara_versao_numero_a_numero() {
        assert!(mais_nova("0.1.13", "0.1.9"), "13 > 9, e não como texto");
        assert!(mais_nova("0.2.0", "0.1.99"));
        assert!(mais_nova("1.0", "0.9.9"));
        assert!(mais_nova("v0.1.13", "0.1.12"));
        assert!(!mais_nova("0.1.12", "0.1.12"));
        assert!(!mais_nova("0.1.12", "0.1.13"));
        assert!(!mais_nova("0.1", "0.1.0"), "0.1 é 0.1.0");
        assert!(!mais_nova("lixo", "0.1.0"));
        assert!(!mais_nova("0.2.0", "lixo"));
    }

    #[test]
    fn o_jeito_de_instalar_vem_do_perfil() {
        assert_eq!(
            JeitoDaInstalacao::do_texto("compilado"),
            JeitoDaInstalacao::Compilado
        );
        assert_eq!(
            JeitoDaInstalacao::do_texto("pacote"),
            JeitoDaInstalacao::Pacote
        );
        assert_eq!(
            JeitoDaInstalacao::do_texto("desenvolvimento"),
            JeitoDaInstalacao::Desenvolvimento
        );
        // Os testes rodam no perfil de desenvolvimento.
        assert_eq!(jeito_desta_instalacao(), JeitoDaInstalacao::Desenvolvimento);
    }

    #[test]
    fn a_instalacao_compilada_e_avisada_da_versao_do_main() {
        let aviso = aviso_do_main("0.1.12", ler(EXEMPLO)).expect("há versão nova");
        assert_eq!(aviso.versao, "0.1.13");
        assert_eq!(aviso.jeito, JeitoDeAtualizar::Compilar);
        assert_eq!(aviso.notas.as_deref(), Some("Chatbot e Agendamentos"));
        assert!(aviso.novidades.unwrap().importante);
        assert!(
            aviso_do_main("0.1.13", ler(EXEMPLO)).is_none(),
            "já está nela"
        );
        assert!(
            aviso_do_main("0.1.14", ler(EXEMPLO)).is_none(),
            "está à frente"
        );
        assert!(
            aviso_do_main("0.1.12", None).is_none(),
            "sem arquivo, sem aviso"
        );
    }

    #[test]
    fn as_novidades_so_acompanham_o_pacote_da_mesma_versao() {
        assert!(para_o_pacote("0.1.13", ler(EXEMPLO)).is_some());
        assert!(para_o_pacote("v0.1.13", ler(EXEMPLO)).is_some());
        assert!(para_o_pacote("0.1.14", ler(EXEMPLO)).is_none());
        assert!(para_o_pacote("0.1.13", None).is_none());
    }

    #[test]
    fn como_atualizar_em_cada_jeito() {
        let pacote = como_atualizar(JeitoDeAtualizar::Pacote);
        assert!(pacote.comando.is_none() && pacote.baixar.is_none());
        let compilado = como_atualizar(JeitoDeAtualizar::Compilar);
        assert_eq!(compilado.passos.len(), 3);
        assert!(compilado.passos[0].contains("compila nesta máquina"));
        if cfg!(target_os = "windows") {
            assert_eq!(compilado.baixar, Some(INSTALADOR_DO_WINDOWS));
        } else {
            assert_eq!(compilado.comando, Some(COMANDO_DO_INSTALADOR));
        }
    }

    /// 🔑 **O `novidades.json` do repositório é da versão do `Cargo.toml`.**
    /// Subir a versão sem escrever as novidades (ou o contrário) faz os balcões
    /// ou não serem avisados, ou serem avisados do que não existe.
    #[test]
    fn o_anuncio_vence_o_cache_velho_do_github() {
        let arquivo = |v: &str| {
            let mut n = Novidades::do_anuncio(v.into(), "do arquivo".into(), false);
            n.novidades = vec!["item".into()];
            n
        };
        let anuncio = |v: &str| Novidades::do_anuncio(v.into(), "do anúncio".into(), false);
        // 29/09: o cache devolveu a 0.1.44 e o servidor anunciou a 0.1.45.
        let fica = a_mais_nova(Some(arquivo("0.1.44")), Some(anuncio("0.1.45"))).unwrap();
        assert_eq!(fica.versao, "0.1.45");
        assert!(
            aviso_do_main("0.1.44", Some(fica)).is_some(),
            "a faixa aparece"
        );
        // Empatadas, fica o arquivo, que tem a lista.
        let fica = a_mais_nova(Some(arquivo("0.1.45")), Some(anuncio("0.1.45"))).unwrap();
        assert_eq!(fica.titulo, "do arquivo");
        // O GitHub fora do ar: vale o anúncio.
        assert_eq!(
            a_mais_nova(None, Some(anuncio("0.1.45"))).unwrap().versao,
            "0.1.45"
        );
        // Sem anúncio, o de sempre.
        assert_eq!(
            a_mais_nova(Some(arquivo("0.1.45")), None).unwrap().versao,
            "0.1.45"
        );
    }

    /// Contra a rede de verdade: o rastro diz qual endereço respondeu, a
    /// versão e o tempo. `cargo test -p ui-gpui --lib o_rastro_da_busca -- --ignored`
    #[test]
    #[ignore]
    fn o_rastro_da_busca_de_verdade() {
        let (achou, rastro) = buscar_com_rastro();
        eprintln!("{rastro:?}");
        let achou = achou.expect("o GitHub respondeu");
        assert!(rastro
            .last()
            .unwrap()
            .starts_with(&format!("raw {}", achou.versao)));
        assert!(rastro.last().unwrap().contains(" ms"));
    }

    #[test]
    fn o_novidades_json_anda_com_a_versao_do_cargo() {
        let n = desta_versao().expect("docs/novidades.json legível, com versão e título");
        assert_eq!(
            n.versao,
            env!("CARGO_PKG_VERSION"),
            "docs/novidades.json e o [workspace.package] version do Cargo.toml andam juntos"
        );
        assert!(!n.novidades.is_empty(), "novidade sem item não diz nada");
        assert!(!n.por_que_atualizar.trim().is_empty());
    }

    /// 🔑 **O histórico começa pelo `novidades.json`**, e desce sem repetir
    /// versão: quem lança acrescenta a entrada nova no topo dos dois.
    #[test]
    fn o_historico_comeca_pelas_novidades_desta_versao() {
        let atual = desta_versao().expect("docs/novidades.json legível");
        let texto = include_str!("../../../../docs/historico-de-novidades.json");
        let cru: Vec<serde_json::Value> = serde_json::from_str(texto).expect("histórico legível");
        let historico = historico_desta_versao();
        assert_eq!(
            cru.len(),
            historico.len(),
            "nenhuma entrada torta ou repetida"
        );
        let topo = &historico[0];
        assert_eq!(
            (
                &topo.versao,
                &topo.titulo,
                topo.importante,
                &topo.novidades,
                &topo.por_que_atualizar
            ),
            (
                &atual.versao,
                &atual.titulo,
                atual.importante,
                &atual.novidades,
                &atual.por_que_atualizar
            ),
            "a primeira entrada de docs/historico-de-novidades.json é a de docs/novidades.json"
        );
        for (n, antes) in historico.iter().zip(cru.iter()) {
            assert_eq!(
                antes["versao"].as_str(),
                Some(n.versao.as_str()),
                "da mais nova para a mais antiga"
            );
            assert!(!n.novidades.is_empty(), "{} sem item", n.versao);
            let data = n.data.as_deref().unwrap_or_default();
            assert!(
                data.len() == 10 && data.starts_with("20"),
                "{}: data no formato 2026-10-01, e não {data:?}",
                n.versao
            );
        }
    }

    #[test]
    fn juntar_fica_com_a_versao_que_tem_a_lista() {
        let com_lista = |v: &str, data: Option<&str>| {
            let mut n = Novidades::do_anuncio(v.into(), "do arquivo".into(), false);
            n.novidades = vec!["item".into()];
            n.data = data.map(Into::into);
            n
        };
        let anuncio = Novidades::do_anuncio("0.1.62".into(), "do anúncio".into(), true);
        let juntas = juntar([
            anuncio,
            com_lista("0.1.9", Some("2026-09-20")),
            com_lista("0.1.62", None),
            com_lista("0.1.10", Some("2026-09-21")),
            com_lista("0.1.9", None),
        ]);
        let versoes: Vec<&str> = juntas.iter().map(|n| n.versao.as_str()).collect();
        assert_eq!(
            versoes,
            ["0.1.62", "0.1.10", "0.1.9"],
            "uma por número, 10 > 9"
        );
        assert_eq!(
            juntas[0].titulo, "do arquivo",
            "a do anúncio não tem a lista"
        );
        assert_eq!(juntas[2].data.as_deref(), Some("2026-09-20"));
        assert!(ler_historico("lixo").is_empty());
        assert_eq!(
            ler_historico(
                r#"[{"versao":"x","titulo":"t","novidades":[],"por_que_atualizar":""},
                {"versao":"0.1.2","titulo":"t","novidades":["a"],"por_que_atualizar":"p"}]"#
            )
            .len(),
            1,
            "a entrada torta fica de fora sozinha"
        );
    }

    /// Os endereços são https e terminam no arquivo — e o `raw` do `main` vem
    /// primeiro, porque vale no instante do `make mains`.
    #[test]
    fn os_enderecos_das_novidades() {
        for e in ENDERECOS_DO_HISTORICO {
            assert!(
                e.starts_with("https://") && e.ends_with("/historico-de-novidades.json"),
                "{e}"
            );
        }
        for e in ENDERECOS {
            assert!(
                e.starts_with("https://") && e.ends_with("/novidades.json"),
                "{e}"
            );
        }
        assert!(
            ENDERECOS[0].contains("raw.githubusercontent.com") && ENDERECOS[0].contains("/main/")
        );
    }
}
