//! **Comparar APIs gráficas** — o botão da janela Desempenho.
//!
//! *"Não seria interessante criar um teste de desempenho de DX11, DX12 e
//! Vulkan?"* e, escolhido entre as opções, *"Dentro do app"* — dono,
//! 9/out/2026. A mesma revelação em cada API da máquina, e no Windows a subida
//! ao DirectX 11 da janela; o que se mede e por quê está em
//! `revelacao_core::comparacao_de_apis`.
//!
//! # Num processo filho
//!
//! 🚨 **O app chama a si mesmo com `--comparar-apis <arquivo>`**, como o
//! `--recuperar`. Abrir o motor numa API que o app não usa carrega o driver
//! dela, e um driver ruim derruba o processo dentro dele — o Vulkan da Intel
//! UHD 630 no Windows fez isso com o LightCraft (issue 136), e um crash nativo
//! não se apanha. No filho, quem cai é ele: o app fica de pé e conta onde foi.
//!
//! O filho regrava o arquivo antes de cada passo e depois de cada medida. O
//! app lê o mesmo arquivo para mostrar o progresso, e no fim — terminado ou
//! não — monta uma sessão de Desempenho com `origem = comparar_apis` e a manda
//! ao banco pelo caminho de sempre (`porta`): computador e servidor.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use domain::desempenho::{CabecalhoDaSessao, MetricaDeDesempenho, SessaoDeDesempenho};
use revelacao_core::comparacao_de_apis::{self, ComparacaoDeApis};

use super::maquina::Maquina;

/// O argumento do processo filho.
pub const ARGUMENTO: &str = "--comparar-apis";

/// A `origem` da sessão no banco.
pub const ORIGEM: &str = "comparar_apis";

/// O processo filho: compara e grava o arquivo a cada passo. Devolve o
/// código de saída.
pub fn rodar_pela_linha_de_comando(args: &[String]) -> i32 {
    let Some(arquivo) = args
        .iter()
        .position(|a| a == ARGUMENTO)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
    else {
        eprintln!("uso: {ARGUMENTO} <arquivo.json>");
        return 2;
    };
    let foto = comparacao_de_apis::foto_sintetica();
    comparacao_de_apis::comparar(
        &foto,
        "sintética",
        |passo| eprintln!("… {passo}"),
        |parcial| gravar(&arquivo, parcial),
    );
    0
}

/// Grava num arquivo ao lado e troca: quem lê nunca vê um JSON pela metade.
fn gravar(arquivo: &Path, resultado: &ComparacaoDeApis) {
    let Ok(bytes) = serde_json::to_vec(resultado) else {
        return;
    };
    let provisorio = arquivo.with_extension("json.parcial");
    if std::fs::write(&provisorio, bytes).is_ok() {
        let _ = std::fs::rename(&provisorio, arquivo);
    }
}

fn ler(arquivo: &Path) -> Option<ComparacaoDeApis> {
    std::fs::read(arquivo)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

/// Roda o filho e espera, mandando o passo em andamento a `progresso`. Bloqueia:
/// chame numa thread de fundo.
///
/// `Err` só quando nem o filho começou nem o arquivo apareceu; um filho que
/// caiu no meio devolve `Ok` com o que mediu e a ocorrência.
pub fn rodar_no_filho(mut progresso: impl FnMut(&str)) -> Result<ComparacaoDeApis, String> {
    let pasta = crate::atualizacao::compilar::casa()
        .map(|c| c.join("desempenho"))
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&pasta).map_err(|e| format!("sem pasta para o resultado ({e})"))?;
    let arquivo = pasta.join(format!("comparacao-{}.json", uuid::Uuid::new_v4()));
    let exe = std::env::current_exe().map_err(|e| format!("não achei o app ({e})"))?;
    let mut filho = std::process::Command::new(exe)
        .arg(ARGUMENTO)
        .arg(&arquivo)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("o processo de medição não começou ({e})"))?;

    // A exportação de 24 MP no OpenGL de uma placa fraca leva minutos; mais
    // que isso é um driver preso.
    let limite = Instant::now() + Duration::from_secs(15 * 60);
    let mut ultimo = String::new();
    let status = loop {
        if let Some(parcial) = ler(&arquivo) {
            if parcial.passo != ultimo && !parcial.passo.is_empty() {
                ultimo = parcial.passo.clone();
                progresso(&ultimo);
            }
        }
        match filho.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() > limite => {
                let _ = filho.kill();
                let _ = filho.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(_) => break None,
        }
    };

    let resultado = ler(&arquivo);
    let _ = std::fs::remove_file(&arquivo);
    let mut resultado = resultado.ok_or_else(|| match status {
        Some(s) => format!("o processo de medição saiu ({s}) sem resultado"),
        None => "o processo de medição passou de 15 min e foi encerrado".to_string(),
    })?;
    if !resultado.terminou {
        resultado.ocorrencias.push(match status {
            Some(s) => format!(
                "o processo de medição caiu ({s}) em \"{}\" — provável defeito do driver dessa API",
                resultado.passo
            ),
            None => format!(
                "o processo de medição passou de 15 min em \"{}\" e foi encerrado",
                resultado.passo
            ),
        });
    }
    Ok(resultado)
}

/// A comparação como uma sessão de Desempenho, pronta para o banco.
///
/// As métricas são uma por API × placa × tamanho (`operacao` =
/// `revelacao_<tamanho>`, `etapa` = `<API> · <placa>`) e uma por subida ao
/// DX11 (`subida_dx11_<tamanho>`). O resultado inteiro vai em
/// `revelacoes_json`, e é dele que o relatório se refaz.
pub fn sessao(
    resultado: &ComparacaoDeApis,
    iniciada_em: chrono::DateTime<chrono::Utc>,
    duracao: Duration,
    maquina: Option<&Maquina>,
) -> SessaoDeDesempenho {
    let so = crate::telemetria::maquina::deste();
    let mut placas: Vec<String> = Vec::new();
    let mut apis: Vec<String> = Vec::new();
    for m in &resultado.medidas {
        if !placas.contains(&m.placa) {
            placas.push(m.placa.clone());
        }
        if !apis.contains(&m.api) {
            apis.push(m.api.clone());
        }
    }
    let mut metricas: Vec<MetricaDeDesempenho> = resultado
        .medidas
        .iter()
        .map(|m| MetricaDeDesempenho {
            operacao: format!("revelacao_{}", m.tamanho),
            etapa: format!("{} · {}", m.api, m.placa),
            onde: "gpu".into(),
            amostras: 20,
            mediana_ms: m.mediana_ms as f32,
            p95_ms: m.p95_ms as f32,
            pior_ms: m.p95_ms.max(m.fria_ms) as f32,
            media_ms: m.mediana_ms as f32,
            soma_ms: m.mediana_ms * 20.0,
        })
        .collect();
    for s in resultado.subidas.iter().flatten() {
        metricas.push(MetricaDeDesempenho {
            operacao: format!("subida_dx11_{}", s.tamanho),
            etapa: "DirectX 11 · janela".into(),
            onde: "cpu_interface".into(),
            amostras: 20,
            mediana_ms: s.mediana_ms as f32,
            p95_ms: s.p95_ms as f32,
            pior_ms: s.p95_ms as f32,
            media_ms: s.mediana_ms as f32,
            soma_ms: s.mediana_ms * 20.0,
        });
    }
    let mut diagnostico = resultado.resumo();
    diagnostico.extend(resultado.ocorrencias.iter().cloned());
    SessaoDeDesempenho {
        cabecalho: CabecalhoDaSessao {
            id: uuid::Uuid::new_v4().to_string(),
            iniciada_em: iniciada_em.to_rfc3339(),
            terminada_em: (iniciada_em + chrono::Duration::from_std(duracao).unwrap_or_default())
                .to_rfc3339(),
            duracao_ms: duracao.as_secs_f64() * 1000.0,
            origem: ORIGEM.into(),
            versao_do_app: env!("CARGO_PKG_VERSION").into(),
            perfil_de_build: super::perfil_de_build().into(),
            sistema: so.sistema.clone(),
            sistema_versao: maquina
                .map(|m| m.sistema_versao.clone())
                .or_else(|| so.distribuicao.clone())
                .unwrap_or_default(),
            arquitetura: std::env::consts::ARCH.into(),
            cpu: maquina.map(Maquina::cpu_em_uma_linha).unwrap_or_default(),
            gpu_nome: placas.join(" | "),
            gpu_backend: apis.join(", "),
            telas: "comparar_apis".into(),
            diagnostico: diagnostico.join("\n"),
            maquina_json: maquina
                .and_then(|m| serde_json::to_string(m).ok())
                .unwrap_or_else(|| "{}".into()),
            imagens_json: serde_json::json!([{
                "largura": resultado.largura,
                "altura": resultado.altura,
                "formato": resultado.foto,
            }])
            .to_string(),
            revelacoes_json: serde_json::to_string(resultado).unwrap_or_else(|_| "{}".into()),
            ..Default::default()
        },
        metricas,
        quadros: Vec::new(),
    }
}

/// O relatório de uma sessão de comparação, refeito do `revelacoes_json`.
/// `None` quando a sessão não é de comparação (ou o JSON não se lê).
pub fn texto(sessao: &SessaoDeDesempenho) -> Option<String> {
    let c = &sessao.cabecalho;
    if c.origem != ORIGEM {
        return None;
    }
    let resultado: ComparacaoDeApis = serde_json::from_str(&c.revelacoes_json).ok()?;
    Some(format!(
        "VintageLightbox {} · comparação das APIs gráficas\n\
         Sessão {} · {} · {:.0} s · build {}\n\
         {} {} · {} · {}\n\n{}",
        c.versao_do_app,
        &c.id[..c.id.len().min(8)],
        c.iniciada_em,
        c.duracao_ms / 1000.0,
        c.perfil_de_build,
        c.sistema,
        c.sistema_versao,
        c.arquitetura,
        c.cpu,
        resultado.texto()
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use revelacao_core::comparacao_de_apis::{MedidaDaApi, SubidaAJanela};

    fn exemplo(terminou: bool) -> ComparacaoDeApis {
        ComparacaoDeApis {
            foto: "sintética".into(),
            largura: 6000,
            altura: 4000,
            medidas: vec![MedidaDaApi {
                api: "DX12".into(),
                placa: "Intel UHD 630".into(),
                tipo: "integrada".into(),
                tamanho: "arrasto".into(),
                largura: 1600,
                altura: 1067,
                fria_ms: 80.0,
                mediana_ms: 9.0,
                p95_ms: 12.0,
                bgra_ms: 1.0,
                ..Default::default()
            }],
            subidas: Some(vec![SubidaAJanela {
                tamanho: "arrasto".into(),
                largura: 1600,
                altura: 1067,
                mediana_ms: 4.0,
                p95_ms: 5.0,
            }]),
            ocorrencias: vec!["Vulkan: nenhuma placa responde nesta API".into()],
            terminou,
            passo: if terminou {
                String::new()
            } else {
                "Vulkan: procurando placas".into()
            },
        }
    }

    #[test]
    fn a_sessao_leva_uma_metrica_por_medida_e_por_subida() {
        let s = sessao(
            &exemplo(true),
            chrono::Utc::now(),
            Duration::from_secs(60),
            None,
        );
        assert_eq!(s.cabecalho.origem, ORIGEM);
        assert_eq!(s.cabecalho.gpu_backend, "DX12");
        let nomes: Vec<_> = s
            .metricas
            .iter()
            .map(|m| (m.operacao.as_str(), m.etapa.as_str()))
            .collect();
        assert_eq!(
            nomes,
            [
                ("revelacao_arrasto", "DX12 · Intel UHD 630"),
                ("subida_dx11_arrasto", "DirectX 11 · janela")
            ]
        );
        assert!(
            s.cabecalho.diagnostico.contains("= 14.0 ms"),
            "motor 9 + BGRA 1 + subida 4: {}",
            s.cabecalho.diagnostico
        );
    }

    #[test]
    fn o_relatorio_se_refaz_da_sessao_gravada() {
        let s = sessao(
            &exemplo(true),
            chrono::Utc::now(),
            Duration::from_secs(60),
            None,
        );
        // Ida e volta pelo JSON, como do banco.
        let lida: SessaoDeDesempenho =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        let texto = texto(&lida).expect("é uma comparação");
        assert!(texto.contains("DX12 · Intel UHD 630"));
        assert!(texto.contains("subida à janela (DirectX 11"));
        assert!(texto.contains("Vulkan: nenhuma placa responde"));
    }

    #[test]
    fn a_comparacao_que_caiu_diz_onde() {
        let texto = exemplo(false).texto();
        assert!(
            texto.contains("o processo caiu em \"Vulkan: procurando placas\""),
            "{texto}"
        );
    }

    #[test]
    fn o_filho_grava_um_json_que_o_app_le() {
        let pasta = tempfile::tempdir().unwrap();
        let arquivo = pasta.path().join("c.json");
        gravar(&arquivo, &exemplo(false));
        assert_eq!(ler(&arquivo), Some(exemplo(false)));
        assert!(!arquivo.with_extension("json.parcial").exists());
    }
}
