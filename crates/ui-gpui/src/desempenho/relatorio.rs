//! O relatório: a sessão como ela vai para o banco, o diagnóstico em frases e
//! o texto do "Copiar relatório".
//!
//! # O diagnóstico separa o app da máquina
//!
//! 🔑 O gargalo sozinho não diz de quem é a culpa. Quem diz é **onde** ele roda
//! e **em quê**:
//!
//! - na **thread da interface** (conversão, histograma, recorte, decodificação
//!   síncrona): é o app — trabalho que prende o quadro e pode sair dali;
//! - na **montagem/apresentação**: é o desenho do GPUI (layout, pintura,
//!   `present`) — o app pode desenhar menos, mas o custo é do renderizador;
//! - na **GPU** ou na **espera pela GPU**: é a placa — a menos que o motor
//!   tenha aberto na placa errada (integrada com dedicada ao lado, OpenGL,
//!   driver genérico da Microsoft), e aí volta a ser o app;
//! - em segundo plano (redução, preparo): atrasa a foto, não o quadro.
//!
//! As observações de máquina (bateria, build de depuração, taxa estimada) vêm
//! junto porque mudam a leitura de todos os números.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::time::Duration;

use domain::desempenho::{
    CabecalhoDaSessao, MetricaDeDesempenho, QuadroGravado, SessaoDeDesempenho,
};

use super::coletor::{Coletor, Quadro, Resumo, ResumoDaOperacao};
use super::maquina::Maquina;
use super::{Contexto, Etapa, Onde, Operacao, ETAPAS_DA_INTERFACE};

/// Quadros normais guardados por sessão (os lentos vão todos).
pub const AMOSTRA_DE_QUADROS_NORMAIS: usize = 2_000;
/// Revelações do motor guardadas por sessão.
pub const AMOSTRA_DE_REVELACOES: usize = 300;

/// O que o vigia grava quando a interface trava: a sessão até ali.
pub type Fotografia = SessaoDeDesempenho;

pub fn fotografar(
    id: &str,
    iniciada_em: chrono::DateTime<chrono::Utc>,
    duracao: Duration,
    coletor: &Coletor,
    contexto: &Contexto,
) -> Fotografia {
    montar_sessao(
        id,
        iniciada_em,
        duracao,
        coletor,
        contexto,
        "travamento",
        super::maquina::pronta().as_deref(),
    )
}

fn ms(x: f32) -> String {
    if x >= 100.0 {
        format!("{x:.0} ms")
    } else {
        format!("{x:.1} ms")
    }
}

// ── O diagnóstico ──────────────────────────────────────────────────────────

/// A frase do gargalo de uma operação: "Maior gargalo no arrasto de slider:
/// conversão da imagem para exibição (CPU, thread da interface) — p95 de 22 ms".
pub fn frase_do_gargalo(op: &ResumoDaOperacao) -> Option<String> {
    let (etapa, s) = op.gargalo?;
    Some(format!(
        "Maior gargalo no {}: {} ({}) — p95 de {}",
        op.operacao.rotulo(),
        etapa.rotulo(),
        etapa.onde().rotulo(),
        ms(s.p95_ms)
    ))
}

/// De quem é o gargalo — ver o [módulo](self).
pub fn leitura_do_gargalo(
    etapa: Etapa,
    motor: Option<&revelacao_core::InfoDoAdaptador>,
    maquina: Option<&Maquina>,
) -> String {
    match etapa.onde() {
        Onde::CpuInterface => {
            "prende a thread da interface: é do app (trabalho que pode sair do caminho do quadro)"
                .into()
        }
        Onde::Quadro => "é o desenho da janela (layout, pintura e present do GPUI)".into(),
        Onde::CpuFundo if etapa != Etapa::EsperaPelaGpu => {
            "roda em segundo plano: atrasa a foto, mas não prende o quadro".into()
        }
        _ => match motor_em_placa_errada(motor, maquina) {
            Some(motivo) => format!("o motor está numa GPU que não é a melhor: {motivo}"),
            None => match motor.map(|m| m.tipo) {
                Some("integrada") => "é a GPU (integrada): provável limite do hardware".into(),
                _ => "é a GPU: compare com outra máquina antes de culpar o app".into(),
            },
        },
    }
}

/// O motor abriu num adaptador pior que um disponível.
pub fn motor_em_placa_errada(
    motor: Option<&revelacao_core::InfoDoAdaptador>,
    maquina: Option<&Maquina>,
) -> Option<String> {
    let motor = motor?;
    if motor.backend == "OpenGL" {
        return Some("o motor caiu no OpenGL (o mais lento dos backends)".into());
    }
    if motor.tipo == "cpu" || motor.fabricante_id == 0x1414 {
        return Some(format!(
            "o motor está num adaptador de software ({}): falta o driver da placa",
            motor.nome
        ));
    }
    let maquina = maquina?;
    if motor.tipo == "integrada" {
        if let Some(dedicada) = maquina.gpus.iter().find(|g| g.tipo == "dedicada") {
            return Some(format!(
                "o motor abriu na integrada ({}) com uma dedicada disponível ({})",
                motor.nome, dedicada.nome
            ));
        }
    }
    None
}

/// As observações que valem para a sessão inteira.
pub fn observacoes(resumo: &Resumo, contexto: &Contexto, maquina: Option<&Maquina>) -> Vec<String> {
    let mut o = Vec::new();
    if cfg!(debug_assertions) {
        o.push(
            "⚠️ Build de depuração: os números valem até 57× mais que no release. Meça em --release."
                .into(),
        );
    }
    match contexto.hz_origem {
        "sistema" => {}
        "estimada" => o.push(format!(
            "A taxa do monitor ({:.0} Hz) foi estimada pelos quadros; o sistema não informou.",
            contexto.hz
        )),
        _ => o.push("A taxa do monitor não foi lida: orçamento calculado para 60 Hz.".into()),
    }
    match &contexto.motor {
        None => o.push("O motor da Revelação não rodou nesta captura (sem tempos de GPU).".into()),
        Some(m) if !m.carimbos => o.push(format!(
            "A GPU do motor ({} · {}) não oferece timestamp query: tempo de GPU indisponível; \
             a \"espera pela GPU\" inclui fila e driver.",
            m.nome, m.backend
        )),
        Some(_) if !resumo.gpu_medida => {
            o.push("A GPU oferece timestamp query, mas nenhuma revelação foi medida.".into())
        }
        Some(_) => {}
    }
    if let Some(motivo) = motor_em_placa_errada(contexto.motor.as_ref(), maquina) {
        o.push(format!("🚨 {motivo}."));
    }
    if let Some(m) = maquina {
        if m.energia.as_deref().is_some_and(|e| e.contains("bateria")) {
            o.push("O computador está na bateria: CPU e GPU podem estar em economia.".into());
        }
        // No macOS a memória "livre" é pouca de propósito (vira cache): o
        // aviso só vale nos outros.
        if !cfg!(target_os = "macos") && m.memoria_livre_mb > 0 && m.memoria_livre_mb < 1024 {
            o.push(format!(
                "Pouca memória livre ao coletar ({} MB): o sistema pode estar trocando com o disco.",
                m.memoria_livre_mb
            ));
        }
        if m.gpus.iter().any(|g| g.fabricante_id == "0x1414") {
            o.push(
                "Há um \"Microsoft Basic Render Driver\": a placa pode estar sem driver instalado."
                    .into(),
            );
        }
    }
    if !resumo.travamentos.is_empty() {
        let pior = resumo
            .travamentos
            .iter()
            .map(|t| t.duracao_ms)
            .fold(0.0, f32::max);
        o.push(format!(
            "A interface travou {} vez(es), a pior por {}.",
            resumo.travamentos.len(),
            ms(pior)
        ));
    }
    o
}

/// O diagnóstico inteiro, uma frase por linha.
pub fn diagnostico(resumo: &Resumo, contexto: &Contexto, maquina: Option<&Maquina>) -> Vec<String> {
    let mut linhas: Vec<String> = resumo
        .por_operacao
        .iter()
        .filter_map(|op| {
            let frase = frase_do_gargalo(op)?;
            let (etapa, _) = op.gargalo?;
            Some(format!(
                "{frase}. {}.",
                leitura_do_gargalo(etapa, contexto.motor.as_ref(), maquina)
            ))
        })
        .collect();
    linhas.extend(observacoes(resumo, contexto, maquina));
    linhas
}

// ── A sessão para o banco ──────────────────────────────────────────────────

fn etapas_do_quadro(q: &Quadro) -> String {
    let mut mapa = serde_json::Map::new();
    for (i, e) in ETAPAS_DA_INTERFACE.iter().enumerate() {
        if q.etapas[i] > 0 {
            mapa.insert(e.nome().into(), serde_json::json!(q.etapa_ms(i)));
        }
    }
    serde_json::Value::Object(mapa).to_string()
}

fn gravado(q: &Quadro, telas: &[&'static str]) -> QuadroGravado {
    QuadroGravado {
        indice: q.seq,
        em_ms: q.em_us as f64 / 1000.0,
        intervalo_ms: q.intervalo_ms(),
        montagem_ms: q.montagem_us as f32 / 1000.0,
        apresentacao_ms: q.apresentacao_us as f32 / 1000.0,
        operacao: q.operacao.nome().into(),
        tela: telas.get(q.tela as usize).copied().unwrap_or("?").into(),
        lento: q.lento,
        etapas_json: etapas_do_quadro(q),
    }
}

/// Todos os lentos guardados (inclusive os piores, que nunca saem) e uma
/// fração uniforme dos outros quadros de interação.
pub fn amostra_de_quadros(c: &Coletor) -> Vec<QuadroGravado> {
    let mut vistos = HashSet::new();
    let mut saida: Vec<QuadroGravado> = Vec::new();
    for q in c.piores().iter().chain(c.lentos().iter()) {
        if vistos.insert(q.seq) {
            saida.push(gravado(q, &c.telas));
        }
    }
    let normais: Vec<&Quadro> = c
        .quadros()
        .iter()
        .filter(|q| !q.lento && q.operacao != Operacao::Nenhuma)
        .collect();
    let passo = normais.len().div_ceil(AMOSTRA_DE_QUADROS_NORMAIS).max(1);
    for q in normais.into_iter().step_by(passo) {
        if vistos.insert(q.seq) {
            saida.push(gravado(q, &c.telas));
        }
    }
    saida.sort_by_key(|q| q.indice);
    saida
}

fn revelacoes_json(c: &Coletor) -> String {
    let lista: Vec<serde_json::Value> = c
        .revelacoes()
        .iter()
        .rev()
        .take(AMOSTRA_DE_REVELACOES)
        .rev()
        .map(|r| {
            let g = r.tempos.gpu;
            serde_json::json!({
                "em_ms": r.em_us as f64 / 1000.0,
                "operacao": r.operacao.nome(),
                "largura": r.largura,
                "altura": r.altura,
                "rascunho": r.rascunho,
                "reducao_ms": r.reducao_ms,
                "preparo_ms": r.tempos.preparo_ms,
                "gravacao_ms": r.tempos.gravacao_ms,
                "espera_ms": r.tempos.espera_ms,
                "leitura_ms": r.tempos.leitura_ms,
                "subiu_textura": r.tempos.subiu_textura,
                "total_ms": r.total_ms,
                "gpu_mascaras_ms": g.and_then(|g| g.mascaras_ms),
                "gpu_retoques_ms": g.and_then(|g| g.retoques_ms),
                "gpu_revelacao_ms": g.and_then(|g| g.revelacao_ms),
                "gpu_total_ms": g.and_then(|g| g.total_ms),
            })
        })
        .collect();
    serde_json::Value::Array(lista).to_string()
}

pub fn metricas(c: &Coletor) -> Vec<MetricaDeDesempenho> {
    let mut lista: Vec<MetricaDeDesempenho> = c
        .distribuicoes()
        .iter()
        .map(|((op, etapa), d)| {
            let s = d.resumo();
            MetricaDeDesempenho {
                operacao: op.nome().into(),
                etapa: etapa.nome().into(),
                onde: etapa.onde().nome().into(),
                amostras: s.amostras,
                mediana_ms: s.mediana_ms,
                p95_ms: s.p95_ms,
                pior_ms: s.pior_ms,
                media_ms: s.media_ms,
                soma_ms: s.soma_ms,
            }
        })
        .collect();
    lista.sort_by(|a, b| (&a.operacao, &a.etapa).cmp(&(&b.operacao, &b.etapa)));
    lista
}

pub fn montar_sessao(
    id: &str,
    iniciada_em: chrono::DateTime<chrono::Utc>,
    duracao: Duration,
    c: &Coletor,
    contexto: &Contexto,
    origem: &str,
    maquina: Option<&Maquina>,
) -> SessaoDeDesempenho {
    let t = duracao.as_micros() as u64;
    let r = c.resumo(t);
    let motor = contexto.motor.clone().unwrap_or_default();
    let so = crate::telemetria::maquina::deste();
    let travamentos: Vec<serde_json::Value> = r
        .travamentos
        .iter()
        .map(|x| {
            serde_json::json!({
                "em_ms": x.em_us as f64 / 1000.0,
                "duracao_ms": x.duracao_ms,
                "operacao": x.operacao.nome(),
            })
        })
        .collect();
    SessaoDeDesempenho {
        cabecalho: CabecalhoDaSessao {
            id: id.into(),
            iniciada_em: iniciada_em.to_rfc3339(),
            terminada_em: (iniciada_em + chrono::Duration::from_std(duracao).unwrap_or_default())
                .to_rfc3339(),
            duracao_ms: duracao.as_secs_f64() * 1000.0,
            origem: origem.into(),
            versao_do_app: env!("CARGO_PKG_VERSION").into(),
            perfil_de_build: super::perfil_de_build().into(),
            sistema: so.sistema.clone(),
            sistema_versao: maquina
                .map(|m| m.sistema_versao.clone())
                .or_else(|| so.distribuicao.clone())
                .unwrap_or_default(),
            arquitetura: std::env::consts::ARCH.into(),
            cpu: maquina.map(Maquina::cpu_em_uma_linha).unwrap_or_default(),
            gpu_nome: motor.nome.clone(),
            gpu_backend: motor.backend.into(),
            gpu_driver: [motor.driver.as_str(), motor.driver_info.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            gpu_carimbos: motor.carimbos,
            janela_largura_px: contexto.janela_px.0,
            janela_altura_px: contexto.janela_px.1,
            escala: contexto.escala,
            taxa_do_monitor_hz: contexto.hz,
            taxa_origem: contexto.hz_origem.into(),
            telas: r
                .telas
                .iter()
                .filter(|t| **t != "?")
                .copied()
                .collect::<Vec<_>>()
                .join(","),
            quadros: r.quadros_interacao,
            fps_interacao: r.fps_interacao,
            mediana_ms: r.mediana_ms,
            p95_ms: r.p95_ms,
            pior_ms: r.pior_ms,
            acima_do_orcamento: r.acima_do_orcamento,
            travamentos: r.travamentos.len() as u32,
            diagnostico: diagnostico(&r, contexto, maquina).join("\n"),
            maquina_json: maquina
                .and_then(|m| serde_json::to_string(m).ok())
                .unwrap_or_else(|| "{}".into()),
            imagens_json: serde_json::to_string(c.imagens()).unwrap_or_else(|_| "[]".into()),
            revelacoes_json: revelacoes_json(c),
            travamentos_json: serde_json::Value::Array(travamentos).to_string(),
        },
        metricas: metricas(c),
        quadros: amostra_de_quadros(c),
    }
}

// ── O texto ────────────────────────────────────────────────────────────────

/// O "Copiar relatório": texto para colar numa conversa ou num chamado.
pub fn texto(sessao: &SessaoDeDesempenho) -> String {
    let c = &sessao.cabecalho;
    let mut t = String::new();
    let _ = writeln!(
        t,
        "VintageLightbox {} · relatório de desempenho",
        c.versao_do_app
    );
    let _ = writeln!(
        t,
        "Sessão {} · {} · {:.1} s · origem {} · build {}",
        &c.id[..c.id.len().min(8)],
        c.iniciada_em,
        c.duracao_ms / 1000.0,
        c.origem,
        c.perfil_de_build
    );
    let _ = writeln!(t);
    let _ = writeln!(t, "== Quadros (durante interação) ==");
    let _ = writeln!(
        t,
        "{} quadros · {:.1} FPS · mediana {} · p95 {} · pior {} · {} acima do orçamento ({:.0} Hz, taxa {})",
        c.quadros,
        c.fps_interacao,
        ms(c.mediana_ms),
        ms(c.p95_ms),
        ms(c.pior_ms),
        c.acima_do_orcamento,
        c.taxa_do_monitor_hz,
        c.taxa_origem
    );
    let _ = writeln!(
        t,
        "Janela {}×{} px · escala {} · telas: {}",
        c.janela_largura_px, c.janela_altura_px, c.escala, c.telas
    );
    let _ = writeln!(t);
    let _ = writeln!(t, "== Diagnóstico ==");
    for linha in c.diagnostico.lines() {
        let _ = writeln!(t, "- {linha}");
    }
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "== Por operação e etapa (mediana / p95 / pior, amostras) =="
    );
    let mut operacao_atual = "";
    for m in &sessao.metricas {
        if m.operacao == "nenhuma" {
            continue;
        }
        if m.operacao != operacao_atual {
            operacao_atual = &m.operacao;
            let _ = writeln!(t, "[{}]", Operacao::do_nome(&m.operacao).rotulo());
        }
        let etapa = Etapa::TODAS
            .into_iter()
            .find(|e| e.nome() == m.etapa)
            .map_or(m.etapa.as_str(), |e| e.rotulo());
        let _ = writeln!(
            t,
            "  {:<52} {:>9} {:>9} {:>9}  ({})  {}",
            etapa,
            ms(m.mediana_ms),
            ms(m.p95_ms),
            ms(m.pior_ms),
            m.amostras,
            m.onde
        );
    }
    let _ = writeln!(t);
    let _ = writeln!(t, "== Máquina ==");
    t.push_str(&texto_da_maquina(&c.maquina_json, c));
    let _ = writeln!(t);
    let _ = writeln!(t, "== Indisponível nesta medição ==");
    let _ = writeln!(
        t,
        "- tempo de GPU do renderizador do GPUI e subida das imagens ao atlas (sem instrumentação exposta)"
    );
    let _ = writeln!(
        t,
        "- fim exato do present: a apresentação é medida até a primeira tarefa seguinte (teto)"
    );
    if !c.gpu_carimbos {
        let _ = writeln!(
            t,
            "- tempo de GPU do motor: o adaptador não oferece timestamp query"
        );
    }
    let lentos = sessao.quadros.iter().filter(|q| q.lento).count();
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "{} quadros guardados ({} lentos) · imagens: {}",
        sessao.quadros.len(),
        lentos,
        c.imagens_json
    );
    t
}

fn texto_da_maquina(json: &str, c: &CabecalhoDaSessao) -> String {
    let mut t = String::new();
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let _ = writeln!(
        t,
        "Sistema: {} ({}) · kernel {} · {}{}",
        if s("sistema_versao").is_empty() {
            c.sistema_versao.clone()
        } else {
            s("sistema_versao")
        },
        c.sistema,
        s("kernel"),
        c.arquitetura,
        v["area_de_trabalho"]
            .as_str()
            .map(|a| format!(" · {a}"))
            .unwrap_or_default()
    );
    let _ = writeln!(
        t,
        "CPU: {} · {} lógicos / {} físicos · {} MHz",
        s("cpu"),
        v["nucleos_logicos"],
        v["nucleos_fisicos"],
        v["cpu_mhz"]
    );
    let _ = writeln!(
        t,
        "Memória: {} MB no total, {} MB livres ao medir",
        v["memoria_total_mb"], v["memoria_livre_mb"]
    );
    let _ = writeln!(
        t,
        "Motor da Revelação: {} · {} · driver {} · timestamps {}",
        c.gpu_nome,
        c.gpu_backend,
        if c.gpu_driver.is_empty() {
            "?"
        } else {
            &c.gpu_driver
        },
        if c.gpu_carimbos { "sim" } else { "não" }
    );
    let _ = writeln!(t, "Interface: {}", s("renderizador_da_interface"));
    for gpu in v["gpus"].as_array().into_iter().flatten() {
        let g = |k: &str| gpu[k].as_str().unwrap_or_default().to_string();
        let _ = writeln!(
            t,
            "GPU (wgpu): {} · {} {}:{} · {} · {} · driver {} {} · timestamps {}",
            g("nome"),
            g("fabricante"),
            g("fabricante_id"),
            g("placa_id"),
            g("tipo"),
            g("backend"),
            g("driver"),
            g("driver_info"),
            if gpu["carimbos"].as_bool() == Some(true) {
                "sim"
            } else {
                "não"
            }
        );
    }
    for p in v["placas_no_sistema"].as_array().into_iter().flatten() {
        let _ = writeln!(
            t,
            "Placa (sistema): {} · driver {} {} {}{}",
            p["nome"].as_str().unwrap_or_default(),
            p["driver_versao"].as_str().unwrap_or_default(),
            p["driver_data"].as_str().unwrap_or_default(),
            p["detalhe"].as_str().unwrap_or_default(),
            p["memoria_mb"]
                .as_u64()
                .map(|mb| format!(" · ≥{mb} MB"))
                .unwrap_or_default()
        );
    }
    for m in v["monitores"].as_array().into_iter().flatten() {
        let _ = writeln!(
            t,
            "Monitor: {} · {} · {}{}",
            m["nome"].as_str().unwrap_or_default(),
            m["resolucao"].as_str().unwrap_or_default(),
            m["hz"]
                .as_f64()
                .map(|h| format!("{h:.0} Hz"))
                .unwrap_or_else(|| "Hz ?".into()),
            if m["principal"].as_bool() == Some(true) {
                " (principal)"
            } else {
                ""
            }
        );
    }
    if let Some(e) = v["energia"].as_str() {
        let _ = writeln!(t, "Energia: {e}");
    }
    for f in v["faltas"].as_array().into_iter().flatten() {
        let _ = writeln!(t, "Não lido: {}", f.as_str().unwrap_or_default());
    }
    if v.as_object().is_none_or(|o| o.is_empty()) {
        let _ = writeln!(t, "(a coleta da máquina não terminou a tempo)");
    }
    t
}

#[cfg(test)]
mod testes {
    use super::*;

    const MS: u64 = 1000;

    fn coletor_com_arrasto() -> Coletor {
        let mut c = Coletor::novo(60.0);
        c.tela("revelacao");
        c.operacao(Operacao::ArrastoDeSlider, 0);
        let mut t = 0;
        for i in 0..100 {
            t += if i % 10 == 0 { 45 * MS } else { 16_667 };
            c.operacao(Operacao::ArrastoDeSlider, t - 1);
            c.etapa(Etapa::ConversaoParaExibicao, 22.0, true, t - 2);
            c.etapa(Etapa::Histograma, 2.0, true, t - 2);
            c.quadro_comecou(t - 3 * MS);
            let s = c.quadro_pintado(t);
            c.quadro_apresentado(s, t + MS);
        }
        c
    }

    #[test]
    fn o_diagnostico_diz_o_gargalo_e_de_quem_e() {
        let c = coletor_com_arrasto();
        let r = c.resumo(2_000 * MS);
        let contexto = Contexto {
            hz: 60.0,
            hz_origem: "sistema",
            ..Default::default()
        };
        let linhas = diagnostico(&r, &contexto, None);
        assert!(
            linhas[0].starts_with(
                "Maior gargalo no arrasto de slider: conversão da imagem para exibição \
                 (CPU, thread da interface) — p95 de 22"
            ),
            "{linhas:?}"
        );
        assert!(linhas[0].contains("é do app"), "{linhas:?}");
    }

    #[test]
    fn o_motor_na_integrada_com_dedicada_ao_lado_e_do_app() {
        let motor = revelacao_core::InfoDoAdaptador {
            nome: "Intel UHD 630".into(),
            backend: "DirectX 12",
            tipo: "integrada",
            ..Default::default()
        };
        let maquina = Maquina {
            gpus: vec![super::super::maquina::Gpu {
                nome: "NVIDIA RTX 3060".into(),
                tipo: "dedicada".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let leitura = leitura_do_gargalo(Etapa::GpuRevelacao, Some(&motor), Some(&maquina));
        assert!(leitura.contains("dedicada disponível"), "{leitura}");
        let sozinha = leitura_do_gargalo(Etapa::GpuRevelacao, Some(&motor), None);
        assert!(sozinha.contains("limite do hardware"), "{sozinha}");
    }

    #[test]
    fn a_sessao_leva_os_lentos_e_uma_amostra_e_vira_texto() {
        let c = coletor_com_arrasto();
        let contexto = Contexto {
            janela_px: (2000, 1200),
            escala: 2.0,
            hz: 60.0,
            hz_origem: "sistema",
            motor: None,
        };
        let s = montar_sessao(
            "abcdef123",
            chrono::Utc::now(),
            Duration::from_secs(2),
            &c,
            &contexto,
            "manual",
            None,
        );
        let lentos = s.quadros.iter().filter(|q| q.lento).count();
        assert_eq!(lentos as u64, s.cabecalho.acima_do_orcamento);
        assert!(lentos >= 9, "{lentos}");
        assert!(s
            .quadros
            .iter()
            .any(|q| q.etapas_json.contains("histograma")));
        assert!(s
            .metricas
            .iter()
            .any(|m| m.etapa == "conversao_para_exibicao" && m.onde == "cpu_interface"));
        let texto = texto(&s);
        assert!(texto.contains("== Máquina =="));
        assert!(texto.contains("Maior gargalo no arrasto de slider"));
        assert!(texto.contains("Indisponível"));
    }
}
