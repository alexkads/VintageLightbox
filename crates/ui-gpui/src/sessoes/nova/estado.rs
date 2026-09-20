//! As regras do assistente da nova sessão — puras, para serem provadas sem
//! janela. É o `nova/estado.ts` do site, portado regra por regra.
//!
//! # O pedido (dono, 2026-09-13, na web; 2026-09-17, no desktop)
//!
//! *"A ideia é entrar dentro da sessão com praticamente tudo que precisamos
//! para o atendimento."* Sete etapas, **nenhuma trava a outra**. O obrigatório
//! é título, preço por foto e estúdio, mais uma coerência: "conheceu por
//! parceiro" exige o parceiro, que o backend recusaria em `400`.
//!
//! 🔚 **O contato não é obrigatório para criar**: quem o pede é o fim da
//! sessão ("Copiar link" e "Avisar"). O e-mail **preenchido** pela metade
//! continua sendo pendência.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::associacoes::{
    eh_como_conheceu, AgendamentoEscolhido, CompraEscolhida, ParceiroEscolhido, VoucherEscolhido,
};

/// O `galeriaId` das fotos locais até a sessão nascer.
pub const PREFIXO_DE_RASCUNHO: &str = "rascunho:";

/// As proporções que a sessão pode ter como corte padrão, na ordem da tela.
/// `None` é "Sem corte".
pub const PROPORCOES_PADRAO: [&str; 7] = ["livre", "1:1", "3:2", "2:3", "4:3", "3:4", "16:9"];

pub fn rotulo_da_proporcao(p: &str) -> &str {
    if p == "livre" {
        "Livre"
    } else {
        p
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Formulario {
    pub titulo: String,
    pub email: String,
    pub whatsapp: String,
    /// A faixa — o "preço por foto".
    pub produto_id: String,
    pub estudio_id: String,
    /// Id do preset do banco, ou `sistema:<chave>` do que vem com o editor.
    pub preset_id: Option<String>,
    pub proporcao: Option<String>,
    pub agendamento: Option<AgendamentoEscolhido>,
    pub voucher: Option<VoucherEscolhido>,
    pub compra: Option<CompraEscolhida>,
    pub como_conheceu: Option<String>,
    pub como_conheceu_detalhe: String,
    pub parceiro: Option<ParceiroEscolhido>,
}

pub const ETAPAS: [&str; 7] = [
    "Aplicativo",
    "Fotos e receita",
    "Cliente e preço",
    "Agendamento",
    "Voucher",
    "Como conheceu",
    "Compra antecipada",
];

pub const TOTAL_DE_ETAPAS: usize = ETAPAS.len();

pub fn titulo_da_etapa(etapa: usize) -> &'static str {
    ETAPAS.get(etapa.wrapping_sub(1)).copied().unwrap_or("")
}

/// As frases de pendência, que também dizem **onde** a pendência se resolve.
pub const FALTA_TITULO: &str = "Informe o título.";
pub const FALTA_PRECO: &str = "Escolha o preço por foto.";
pub const FALTA_ESTUDIO: &str = "Escolha o estúdio.";
pub const EMAIL_INCOMPLETO: &str = "O e-mail não parece completo.";
pub const FALTA_PARCEIRO: &str = "Escolha o parceiro que indicou o cliente.";

/// O campo onde cada pendência se resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Titulo,
    Produto,
    Estudio,
    Email,
    Parceiro,
}

pub fn campo_da_pendencia(mensagem: &str) -> Option<Campo> {
    match mensagem {
        FALTA_PRECO => Some(Campo::Produto),
        FALTA_ESTUDIO => Some(Campo::Estudio),
        FALTA_TITULO => Some(Campo::Titulo),
        EMAIL_INCOMPLETO => Some(Campo::Email),
        FALTA_PARCEIRO => Some(Campo::Parceiro),
        _ => None,
    }
}

/// O que falta **nesta** etapa para a sessão poder ser criada.
pub fn pendencias_da_etapa(etapa: usize, f: &Formulario) -> Vec<&'static str> {
    match etapa {
        3 => {
            let mut falta = Vec::new();
            if f.titulo.trim().is_empty() {
                falta.push(FALTA_TITULO);
            }
            if f.produto_id.trim().is_empty() {
                falta.push(FALTA_PRECO);
            }
            if f.estudio_id.trim().is_empty() {
                falta.push(FALTA_ESTUDIO);
            }
            if !f.email.trim().is_empty()
                && !biblioteca_core::dados_do_cliente::email_plausivel(f.email.trim())
            {
                falta.push(EMAIL_INCOMPLETO);
            }
            falta
        }
        6 if f.como_conheceu.as_deref() == Some("parceiro") && f.parceiro.is_none() => {
            vec![FALTA_PARCEIRO]
        }
        _ => Vec::new(),
    }
}

/// As etapas que não pedem nada: o rodapé oferece "Pular".
pub fn etapa_opcional(etapa: usize) -> bool {
    (4..=7).contains(&etapa)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDaEtapa {
    Pendente,
    Preenchida,
    Vazia,
}

/// O selo de cada etapa no passo a passo.
pub fn estado_da_etapa(
    etapa: usize,
    f: &Formulario,
    fotos: usize,
    como_app: bool,
) -> EstadoDaEtapa {
    if !pendencias_da_etapa(etapa, f).is_empty() {
        return EstadoDaEtapa::Pendente;
    }
    let preenchida = match etapa {
        1 => como_app,
        2 => fotos > 0,
        3 => true,
        4 => f.agendamento.is_some(),
        5 => f.voucher.is_some(),
        6 => f.como_conheceu.is_some(),
        7 => f.compra.is_some(),
        _ => false,
    };
    if preenchida {
        EstadoDaEtapa::Preenchida
    } else {
        EstadoDaEtapa::Vazia
    }
}

/// A legenda embaixo do título da etapa, na lateral (`passos.tsx`).
pub fn legenda(etapa: usize, estado: EstadoDaEtapa, atual: bool) -> String {
    let base = match estado {
        EstadoDaEtapa::Pendente => "falta o obrigatório",
        EstadoDaEtapa::Preenchida => "feita",
        EstadoDaEtapa::Vazia if etapa_opcional(etapa) => "opcional",
        EstadoDaEtapa::Vazia if etapa == 1 => "recomendado",
        EstadoDaEtapa::Vazia if etapa == 2 => "sem fotos",
        EstadoDaEtapa::Vazia => "a preencher",
    };
    if atual {
        format!("atual · {base}")
    } else {
        base.to_string()
    }
}

/// Tudo o que impede criar, com a etapa onde se resolve.
pub fn pendencias_para_criar(f: &Formulario) -> Vec<(usize, &'static str)> {
    (1..=TOTAL_DE_ETAPAS)
        .flat_map(|etapa| {
            pendencias_da_etapa(etapa, f)
                .into_iter()
                .map(move |m| (etapa, m))
        })
        .collect()
}

pub fn pode_criar(f: &Formulario) -> bool {
    pendencias_para_criar(f).is_empty()
}

/// "Maria Souza — 13/09" a partir do agendamento; vazio sem nome.
pub fn titulo_do_agendamento(ag: &AgendamentoEscolhido) -> String {
    let Some(nome) = ag.nome.as_deref().map(str::trim).filter(|n| !n.is_empty()) else {
        return String::new();
    };
    match ag.quando.as_deref().and_then(|q| q.get(0..5)) {
        Some(dia) if dia.as_bytes()[2] == b'/' => format!("{nome} — {dia}"),
        _ => nome.to_string(),
    }
}

/// Leva o contato de uma associação para os campos **vazios**: o que o
/// operador digitou vale mais.
pub fn preencher_contato_vazio(f: &mut Formulario, whatsapp: Option<&str>, email: Option<&str>) {
    if f.whatsapp.trim().is_empty() {
        if let Some(w) = whatsapp.map(str::trim) {
            f.whatsapp = w.to_string();
        }
    }
    if f.email.trim().is_empty() {
        if let Some(e) = email.map(str::trim) {
            f.email = e.to_string();
        }
    }
}

pub fn associar_agendamento(f: &mut Formulario, ag: Option<AgendamentoEscolhido>) {
    let Some(ag) = ag else {
        f.agendamento = None;
        return;
    };
    preencher_contato_vazio(f, ag.whatsapp.as_deref(), ag.email.as_deref());
    if f.titulo.trim().is_empty() {
        let sugerido = titulo_do_agendamento(&ag);
        if !sugerido.is_empty() {
            f.titulo = sugerido;
        }
    }
    if f.estudio_id.is_empty() {
        f.estudio_id = ag.estudio_id.clone().unwrap_or_default();
    }
    f.agendamento = Some(ag);
}

pub fn associar_voucher(f: &mut Formulario, v: Option<VoucherEscolhido>) {
    if let Some(v) = &v {
        preencher_contato_vazio(f, v.whatsapp.as_deref(), v.email.as_deref());
    }
    f.voucher = v;
}

pub fn associar_compra(f: &mut Formulario, c: Option<CompraEscolhida>) {
    if let Some(c) = &c {
        preencher_contato_vazio(f, c.whatsapp.as_deref(), c.comprador_email.as_deref());
    }
    f.compra = c;
}

/// Trocar a resposta limpa o que só valia para a anterior — o backend recusa
/// parceiro sem `como_conheceu = parceiro`.
pub fn escolher_como_conheceu(f: &mut Formulario, valor: Option<String>) {
    if valor.as_deref() != Some("parceiro") {
        f.parceiro = None;
    }
    if valor.as_deref() != Some("outro") {
        f.como_conheceu_detalhe.clear();
    }
    f.como_conheceu = valor;
}

/// O corpo do `POST /pos-venda/galerias`.
pub fn para_envio(f: &Formulario) -> Value {
    let ou = |v: &str| {
        let v = v.trim();
        (!v.is_empty()).then(|| v.to_string())
    };
    let como = f.como_conheceu.as_deref();
    json!({
        "titulo": f.titulo.trim(),
        "email": ou(&f.email),
        "whatsapp": ou(&f.whatsapp),
        "produto_id": f.produto_id,
        "estudio_id": ou(&f.estudio_id),
        "ensaio_id": f.agendamento.as_ref().map(|a| &a.id),
        "voucher_id": f.voucher.as_ref().map(|v| &v.id),
        "pedido_id": f.compra.as_ref().map(|c| &c.id),
        "como_conheceu": f.como_conheceu,
        "como_conheceu_detalhe": if como == Some("outro") { ou(&f.como_conheceu_detalhe) } else { None },
        "parceiro_id": if como == Some("parceiro") { f.parceiro.as_ref().map(|p| p.id.clone()) } else { None },
        "preset_padrao_id": f.preset_id,
        "proporcao_padrao": f.proporcao,
    })
}

fn sem_acento(texto: &str) -> String {
    biblioteca_core::sessoes::normalizar(texto)
}

/// Em que etapa o erro do backend se resolve. O assunto mais específico vem
/// antes: "parceiro do voucher" é do voucher.
pub fn etapa_do_erro(status: Option<u16>, mensagem: &str) -> Option<usize> {
    if !matches!(status, Some(400 | 404 | 409)) {
        return None;
    }
    let m = sem_acento(mensagem);
    let tem = |palavras: &[&str]| palavras.iter().any(|p| m.contains(p));
    if tem(&["voucher"]) {
        Some(5)
    } else if tem(&["pedido", "compra"]) {
        Some(7)
    } else if tem(&["parceiro", "como_conheceu", "como conheceu"]) {
        Some(6)
    } else if tem(&["agendamento", "ensaio"]) {
        Some(4)
    } else if tem(&["preset", "predefinic", "proporc", "corte"]) {
        Some(2)
    } else if tem(&[
        "produto", "faixa", "preco", "estudio", "titulo", "e-mail", "email", "whatsapp", "contato",
    ]) {
        Some(3)
    } else {
        None
    }
}

/// O status e a frase de um erro do `pedir_json` ("o site respondeu 409: …").
pub fn ler_erro_do_site(erro: &str) -> (Option<u16>, String) {
    if let Some(resto) = erro.split("respondeu ").nth(1) {
        if let Some((codigo, frase)) = resto.split_once(": ") {
            if let Ok(status) = codigo.trim().parse::<u16>() {
                return (Some(status), frase.trim().to_string());
            }
        }
    }
    (None, erro.to_string())
}

/// As frases da action do site para o erro da criação.
pub fn mensagem_da_criacao(erro: &str) -> (Option<u16>, String) {
    let (status, frase) = ler_erro_do_site(erro);
    let mensagem = match status {
        Some(403) => "Sua conta não pode criar sessões.".to_string(),
        Some(400 | 404 | 409) => frase,
        Some(_) => "Não foi possível criar a sessão.".to_string(),
        None => "Não foi possível criar a sessão. Tente de novo.".to_string(),
    };
    (status, mensagem)
}

// ── O rascunho nesta máquina ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rascunho {
    pub versao: u32,
    /// `rascunho:<uuid>` — o `sessao_id` das fotos locais até a sessão nascer.
    pub id_provisorio: String,
    pub etapa: usize,
    pub formulario: Formulario,
    /// 🚨 **O id que o servidor devolveu, antes de as fotos mudarem de dono.**
    /// Se a troca falhar (ou o app fechar no meio), o próximo "Criar sessão"
    /// retoma a troca em vez de criar uma segunda galeria.
    pub criada_id: Option<String>,
    /// Segundos desde a época.
    pub atualizado_em: i64,
}

impl Rascunho {
    pub fn novo(uuid: &str, formulario: Formulario, agora: i64) -> Self {
        Self {
            versao: 1,
            id_provisorio: format!("{PREFIXO_DE_RASCUNHO}{uuid}"),
            etapa: 1,
            formulario,
            criada_id: None,
            atualizado_em: agora,
        }
    }
}

pub fn eh_rascunho(sessao_id: &str) -> bool {
    sessao_id.starts_with(PREFIXO_DE_RASCUNHO)
}

#[cfg(not(test))]
pub fn caminho_do_rascunho() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("nova-sessao-rascunho.json")
}

/// 🚨 Nos testes, um arquivo por tela — nunca o catálogo do fotógrafo.
#[cfg(test)]
pub fn caminho_do_rascunho() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-nova-sessao-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Lê o rascunho guardado — e desconfia dele: campo com forma errada vira o
/// vazio daquele campo, e sem `id_provisorio` de rascunho não há como achar as
/// fotos.
pub fn ler_rascunho(caminho: &Path) -> Option<Rascunho> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    let bruto: Value = serde_json::from_str(&texto).ok()?;
    if bruto.get("versao").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let id = bruto.get("id_provisorio")?.as_str()?;
    if !eh_rascunho(id) {
        return None;
    }
    let formulario: Formulario = bruto
        .get("formulario")
        .cloned()
        .and_then(|f| serde_json::from_value(f).ok())
        .unwrap_or_default();
    let mut formulario = formulario;
    if formulario
        .proporcao
        .as_deref()
        .is_some_and(|p| !PROPORCOES_PADRAO.contains(&p))
    {
        formulario.proporcao = None;
    }
    if formulario
        .como_conheceu
        .as_deref()
        .is_some_and(|c| !eh_como_conheceu(c))
    {
        formulario.como_conheceu = None;
    }
    let etapa = bruto
        .get("etapa")
        .and_then(Value::as_u64)
        .map(|e| e as usize)
        .filter(|e| (1..=TOTAL_DE_ETAPAS).contains(e))
        .unwrap_or(1);
    Some(Rascunho {
        versao: 1,
        id_provisorio: id.to_string(),
        etapa,
        formulario,
        criada_id: bruto
            .get("criada_id")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .map(str::to_string),
        atualizado_em: bruto
            .get("atualizado_em")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    })
}

/// Grava; sem onde gravar, o assistente segue — só não retoma depois.
pub fn guardar_rascunho(caminho: &Path, rascunho: &Rascunho) -> bool {
    let Ok(texto) = serde_json::to_string_pretty(rascunho) else {
        return false;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    std::fs::write(caminho, texto).is_ok()
}

pub fn apagar_rascunho(caminho: &Path) {
    let _ = std::fs::remove_file(caminho);
}

/// Há algo que valha perguntar "retomar ou descartar"?
pub fn rascunho_tem_conteudo(r: &Rascunho, fotos: usize) -> bool {
    if fotos > 0 || r.criada_id.is_some() {
        return true;
    }
    let f = &r.formulario;
    !f.titulo.trim().is_empty()
        || !f.email.trim().is_empty()
        || !f.whatsapp.trim().is_empty()
        || f.agendamento.is_some()
        || f.voucher.is_some()
        || f.compra.is_some()
        || f.como_conheceu.is_some()
        || f.preset_id.is_some()
        || f.proporcao.is_some()
}

/// "1 foto" / "3 fotos" e os outros plurais das frases do assistente.
pub fn plural(n: usize, singular: &str, plural: &str) -> String {
    format!("{n} {}", if n == 1 { singular } else { plural })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn completo() -> Formulario {
        Formulario {
            titulo: "Ensaio da Maria".into(),
            produto_id: "p1".into(),
            estudio_id: "e1".into(),
            ..Default::default()
        }
    }

    fn agendamento() -> AgendamentoEscolhido {
        AgendamentoEscolhido {
            id: "ag".into(),
            nome: Some("Maria Souza".into()),
            whatsapp: Some("47999".into()),
            email: Some("maria@x.com".into()),
            inicio_iso: None,
            quando: Some("13/09/2026 14:00".into()),
            estudio_id: Some("e9".into()),
            estudio_nome: None,
            status: "Confirmed".into(),
        }
    }

    #[test]
    fn a_etapa_3_pede_titulo_preco_e_estudio() {
        let f = Formulario::default();
        assert_eq!(
            pendencias_da_etapa(3, &f),
            [FALTA_TITULO, FALTA_PRECO, FALTA_ESTUDIO]
        );
        assert!(pendencias_da_etapa(3, &completo()).is_empty());
        let meio_email = Formulario {
            email: "maria@".into(),
            ..completo()
        };
        assert_eq!(pendencias_da_etapa(3, &meio_email), [EMAIL_INCOMPLETO]);
    }

    #[test]
    fn parceiro_escolhido_na_6_e_obrigatorio() {
        let mut f = completo();
        escolher_como_conheceu(&mut f, Some("parceiro".into()));
        assert_eq!(pendencias_para_criar(&f), [(6, FALTA_PARCEIRO)]);
        assert_eq!(estado_da_etapa(6, &f, 0, true), EstadoDaEtapa::Pendente);
        escolher_como_conheceu(&mut f, Some("instagram".into()));
        assert!(pode_criar(&f));
    }

    #[test]
    fn toda_pendencia_tem_campo() {
        for m in [
            FALTA_TITULO,
            FALTA_PRECO,
            FALTA_ESTUDIO,
            EMAIL_INCOMPLETO,
            FALTA_PARCEIRO,
        ] {
            assert!(campo_da_pendencia(m).is_some(), "{m}");
        }
    }

    #[test]
    fn estado_e_legenda_das_etapas() {
        let f = completo();
        assert_eq!(estado_da_etapa(1, &f, 0, true), EstadoDaEtapa::Preenchida);
        assert_eq!(estado_da_etapa(2, &f, 0, true), EstadoDaEtapa::Vazia);
        assert_eq!(estado_da_etapa(2, &f, 3, true), EstadoDaEtapa::Preenchida);
        assert_eq!(estado_da_etapa(4, &f, 0, true), EstadoDaEtapa::Vazia);
        assert_eq!(legenda(4, EstadoDaEtapa::Vazia, false), "opcional");
        assert_eq!(legenda(2, EstadoDaEtapa::Vazia, false), "sem fotos");
        assert_eq!(legenda(1, EstadoDaEtapa::Vazia, false), "recomendado");
        assert_eq!(legenda(2, EstadoDaEtapa::Preenchida, true), "atual · feita");
        assert_eq!(
            legenda(3, EstadoDaEtapa::Pendente, false),
            "falta o obrigatório"
        );
    }

    #[test]
    fn o_agendamento_so_preenche_o_vazio() {
        let mut f = Formulario {
            email: "digitado@x.com".into(),
            ..Default::default()
        };
        associar_agendamento(&mut f, Some(agendamento()));
        assert_eq!(f.titulo, "Maria Souza — 13/09");
        assert_eq!(f.email, "digitado@x.com");
        assert_eq!(f.whatsapp, "47999");
        assert_eq!(f.estudio_id, "e9");

        let mut com_estudio = completo();
        associar_agendamento(&mut com_estudio, Some(agendamento()));
        assert_eq!(com_estudio.estudio_id, "e1");
        assert_eq!(com_estudio.titulo, "Ensaio da Maria");
        associar_agendamento(&mut com_estudio, None);
        assert!(com_estudio.agendamento.is_none());
    }

    #[test]
    fn trocar_como_conheceu_limpa_parceiro_e_detalhe() {
        let mut f = completo();
        escolher_como_conheceu(&mut f, Some("outro".into()));
        f.como_conheceu_detalhe = "Rádio".into();
        escolher_como_conheceu(&mut f, Some("google".into()));
        assert!(f.como_conheceu_detalhe.is_empty());
    }

    #[test]
    fn o_corpo_do_envio() {
        let mut f = Formulario {
            email: "  ".into(),
            whatsapp: " 47 ".into(),
            preset_id: Some("sistema:sepia".into()),
            proporcao: Some("3:2".into()),
            ..completo()
        };
        escolher_como_conheceu(&mut f, Some("outro".into()));
        f.como_conheceu_detalhe = " feira ".into();
        let corpo = para_envio(&f);
        assert_eq!(corpo["email"], Value::Null);
        assert_eq!(corpo["whatsapp"], "47");
        assert_eq!(corpo["estudio_id"], "e1");
        assert_eq!(corpo["como_conheceu_detalhe"], "feira");
        assert_eq!(corpo["parceiro_id"], Value::Null);
        assert_eq!(corpo["preset_padrao_id"], "sistema:sepia");
        assert_eq!(corpo["proporcao_padrao"], "3:2");
    }

    #[test]
    fn a_etapa_do_erro_pelo_assunto() {
        assert_eq!(etapa_do_erro(Some(404), "Voucher não encontrado"), Some(5));
        assert_eq!(etapa_do_erro(Some(400), "o parceiro do voucher"), Some(5));
        assert_eq!(etapa_do_erro(Some(409), "Pedido já associado"), Some(7));
        assert_eq!(etapa_do_erro(Some(400), "Estúdio inválido"), Some(3));
        assert_eq!(etapa_do_erro(Some(500), "Estúdio inválido"), None);
        assert_eq!(etapa_do_erro(Some(400), "nada a ver"), None);
    }

    #[test]
    fn o_erro_do_site_vira_frase_da_tela() {
        assert_eq!(
            mensagem_da_criacao("o site respondeu 409: Voucher já usado"),
            (Some(409), "Voucher já usado".into())
        );
        assert_eq!(
            mensagem_da_criacao("erro de infraestrutura: o site respondeu 403: nope").1,
            "Sua conta não pode criar sessões."
        );
        assert_eq!(
            mensagem_da_criacao("sem rede").1,
            "Não foi possível criar a sessão. Tente de novo."
        );
    }

    #[test]
    fn o_rascunho_ida_e_volta_e_desconfiado() {
        let caminho = caminho_do_rascunho();
        let mut r = Rascunho::novo("abc", completo(), 10);
        r.formulario.agendamento = Some(agendamento());
        r.etapa = 4;
        assert!(guardar_rascunho(&caminho, &r));
        assert_eq!(ler_rascunho(&caminho), Some(r));

        std::fs::write(
            &caminho,
            r#"{"versao":1,"id_provisorio":"rascunho:x","etapa":99,
                "formulario":{"titulo":"T","proporcao":"5:1","como_conheceu":"radio"}}"#,
        )
        .unwrap();
        let lido = ler_rascunho(&caminho).unwrap();
        assert_eq!(lido.etapa, 1);
        assert_eq!(lido.formulario.titulo, "T");
        assert_eq!(lido.formulario.proporcao, None);
        assert_eq!(lido.formulario.como_conheceu, None);

        std::fs::write(&caminho, r#"{"versao":1,"id_provisorio":"g1"}"#).unwrap();
        assert!(ler_rascunho(&caminho).is_none());
        apagar_rascunho(&caminho);
        assert!(ler_rascunho(&caminho).is_none());
    }

    #[test]
    fn rascunho_vazio_nao_pergunta_nada() {
        let r = Rascunho::novo("a", Formulario::default(), 0);
        assert!(!rascunho_tem_conteudo(&r, 0));
        assert!(rascunho_tem_conteudo(&r, 1));
    }
}
