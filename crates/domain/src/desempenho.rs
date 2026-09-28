//! ⏱️ Uma sessão de medição de desempenho, como ela é guardada.
//!
//! A ferramenta "Desempenho" do rodapé (dono, 2026-09-27) mede quadros e etapas
//! enquanto o operador trabalha, e a sessão vai para o banco local para ser
//! comparada depois — a mesma operação no Windows e no Linux, antes e depois
//! de uma correção.
//!
//! 🔒 **Nada de pixel nem de conteúdo de foto**: da imagem só vão as
//! características técnicas (tamanho, formato, quantas máscaras).
//!
//! Os textos das operações e etapas são os nomes estáveis de
//! `ui-gpui/src/desempenho` (`arrasto_de_slider`, `conversao_para_exibicao`…):
//! o banco guarda o nome, e não a posição num `enum`, para uma sessão gravada
//! numa versão continuar legível na seguinte.

use serde::{Deserialize, Serialize};

/// O cabeçalho: quando, onde e em quê.
///
/// `#[serde(default)]`: uma sessão que volta do servidor, gravada por outra
/// versão do app, pode não ter um campo novo — e não pode sumir da lista por
/// isso.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CabecalhoDaSessao {
    pub id: String,
    /// RFC 3339.
    pub iniciada_em: String,
    pub terminada_em: String,
    pub duracao_ms: f64,
    /// `manual` (Iniciar/Parar) ou `travamento` (o vigia guardou sozinho).
    pub origem: String,
    pub versao_do_app: String,
    /// `release`, `carga`, `debug`… — medida em `debug` não vale.
    pub perfil_de_build: String,
    /// `windows`, `macos` ou `linux`.
    pub sistema: String,
    pub sistema_versao: String,
    pub arquitetura: String,
    pub cpu: String,
    pub gpu_nome: String,
    pub gpu_backend: String,
    pub gpu_driver: String,
    /// A GPU do motor carimba passadas (tempo de GPU medido).
    pub gpu_carimbos: bool,
    pub janela_largura_px: u32,
    pub janela_altura_px: u32,
    pub escala: f32,
    pub taxa_do_monitor_hz: f32,
    /// De onde veio a taxa: `sistema`, `estimada` ou `padrao`.
    pub taxa_origem: String,
    /// As telas por onde a captura passou, separadas por vírgula.
    pub telas: String,
    /// Números do quadro, na sessão inteira durante interação.
    pub quadros: u64,
    pub fps_interacao: f32,
    pub mediana_ms: f32,
    pub p95_ms: f32,
    pub pior_ms: f32,
    pub acima_do_orcamento: u64,
    pub travamentos: u32,
    /// O diagnóstico em uma frase por operação.
    pub diagnostico: String,
    /// A máquina inteira (CPU, memória, todas as GPUs, drivers, monitores,
    /// energia) — JSON, para o formato poder crescer sem migração.
    pub maquina_json: String,
    /// As imagens reveladas: tamanho, formato, máscaras — JSON.
    pub imagens_json: String,
    /// Uma amostra das revelações do motor, etapa por etapa — JSON.
    pub revelacoes_json: String,
    /// Os travamentos da interface — JSON.
    pub travamentos_json: String,
}

/// Uma distribuição agregada: operação × etapa.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricaDeDesempenho {
    pub operacao: String,
    pub etapa: String,
    /// `quadro`, `cpu_interface`, `cpu_fundo`, `gpu` ou `ponta_a_ponta`.
    pub onde: String,
    pub amostras: u64,
    pub mediana_ms: f32,
    pub p95_ms: f32,
    pub pior_ms: f32,
    pub media_ms: f32,
    pub soma_ms: f64,
}

/// Um quadro individual da amostra guardada.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuadroGravado {
    pub indice: u64,
    /// Desde o começo da sessão.
    pub em_ms: f64,
    /// Desde o quadro anterior (0 quando o anterior foi antes da interação).
    pub intervalo_ms: f32,
    /// `render` da raiz → fim da pintura.
    pub montagem_ms: f32,
    /// Fim da pintura → primeira tarefa depois do `present` (aproximado).
    pub apresentacao_ms: f32,
    pub operacao: String,
    pub tela: String,
    pub lento: bool,
    /// As etapas da thread da interface entre este quadro e o anterior,
    /// `{"histograma": 3.2, …}`.
    pub etapas_json: String,
}

/// Uma sessão inteira.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessaoDeDesempenho {
    pub cabecalho: CabecalhoDaSessao,
    pub metricas: Vec<MetricaDeDesempenho>,
    pub quadros: Vec<QuadroGravado>,
}

/// Uma linha para comparar a mesma operação entre sessões e sistemas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinhaDeComparacao {
    pub sessao_id: String,
    pub iniciada_em: String,
    pub sistema: String,
    pub gpu_nome: String,
    pub gpu_backend: String,
    pub taxa_do_monitor_hz: f32,
    pub metrica: MetricaDeDesempenho,
}
