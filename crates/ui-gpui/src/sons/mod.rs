//! 🔔 Os avisos sonoros: um som, uma voz do sistema, ou os dois, nas situações
//! que pedem o operador — e a escolha de cada um nas Configurações.
//!
//! *"Eu preciso de avisos sonoros em situações importantes, mas que tenhamos um
//! lugar para configurar a necessidade ou não dessa notificação, assim como
//! escolha dos tipos e os sons existentes ou customizados"* (dono, 03/out/2026),
//! e na revisão do plano: *"que também seja possível usar algum recurso de voz
//! do sistema operacional, digitando o texto que será falado"*.
//!
//! | Peça | O quê |
//! |---|---|
//! | [`Evento`] | os tipos de aviso, cada um com o padrão dele |
//! | [`preferencias`] | o `sons.json` ao lado do catálogo |
//! | [`embutidos`] | os sons de fábrica, sintetizados |
//! | [`voz`] | a fala, pelo programa do sistema |
//! | [`porta`] | o alto-falante (uma thread, com fila) e a escolha de arquivo |
//! | [`tela`] | a aba "Avisos sonoros" das Configurações |
//!
//! 🔑 **Quem avisa chama [`soar`], e nada mais.** As regras — a chave geral, a
//! do tipo, o modo, o intervalo mínimo — moram aqui, num `Global` do GPUI, para
//! a tela que avisa (o caixa, a agenda, a raiz) não precisar saber de som.
//! Sem o `Global` instalado (um teste de outra tela, um binário avulso), `soar`
//! não faz nada.

pub mod embutidos;
pub mod porta;
pub mod preferencias;
pub mod tela;
pub mod voz;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

pub use embutidos::Embutido;
pub use porta::{AltoFalante, EscolhaDeSom, Fala, Pedido, PortasDoSom, Som, Voz};
pub use preferencias::{Escolha, Preferencias};

/// O mesmo tipo de aviso não toca de novo antes disto: dez mensagens seguidas
/// do chatbot são um sino, e não dez — a mesma ideia da `tag` que faz o aviso
/// do sistema substituir o anterior em vez de empilhar.
pub const INTERVALO_MINIMO: Duration = Duration::from_secs(2);

/// O prefixo dos sons próprios na escolha: `meu:alerta.mp3`.
const MEU: &str = "meu:";

/// Os tipos de aviso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Evento {
    MensagemNoChatbot,
    ClienteNoQr,
    NovoAgendamento,
    AgendamentoCancelado,
    Falha,
    RevelacoesSalvas,
    ExportacaoConcluida,
    NovaVersao,
}

/// Som, voz, ou os dois (o som primeiro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modo {
    #[default]
    Som,
    Voz,
    SomEVoz,
}

impl Modo {
    pub const TODOS: [Modo; 3] = [Modo::Som, Modo::Voz, Modo::SomEVoz];

    pub fn chave(self) -> &'static str {
        match self {
            Modo::Som => "som",
            Modo::Voz => "voz",
            Modo::SomEVoz => "som_e_voz",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Modo::Som => "Som",
            Modo::Voz => "Voz",
            Modo::SomEVoz => "Som e voz",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|m| m.chave() == chave)
    }

    fn toca(self) -> bool {
        matches!(self, Modo::Som | Modo::SomEVoz)
    }

    fn fala(self) -> bool {
        matches!(self, Modo::Voz | Modo::SomEVoz)
    }
}

impl Evento {
    pub const TODOS: [Evento; 8] = [
        Evento::MensagemNoChatbot,
        Evento::ClienteNoQr,
        Evento::NovoAgendamento,
        Evento::AgendamentoCancelado,
        Evento::Falha,
        Evento::RevelacoesSalvas,
        Evento::ExportacaoConcluida,
        Evento::NovaVersao,
    ];

    /// A chave no `sons.json` — nunca muda, ou o arquivo de quem já
    /// configurou perde a escolha.
    pub fn chave(self) -> &'static str {
        match self {
            Evento::MensagemNoChatbot => "mensagem_no_chatbot",
            Evento::ClienteNoQr => "cliente_no_qr",
            Evento::NovoAgendamento => "novo_agendamento",
            Evento::AgendamentoCancelado => "agendamento_cancelado",
            Evento::Falha => "falha",
            Evento::RevelacoesSalvas => "revelacoes_salvas",
            Evento::ExportacaoConcluida => "exportacao_concluida",
            Evento::NovaVersao => "nova_versao",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|e| e.chave() == chave)
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Evento::MensagemNoChatbot => "Mensagem de cliente no chatbot",
            Evento::ClienteNoQr => "Cliente chegou pelo QR da sessão",
            Evento::NovoAgendamento => "Novo agendamento",
            Evento::AgendamentoCancelado => "Agendamento cancelado",
            Evento::Falha => "Falha (o site recusou, erro no caixa)",
            Evento::RevelacoesSalvas => "Fotos salvas na galeria",
            Evento::ExportacaoConcluida => "Exportação concluída",
            Evento::NovaVersao => "Nova versão pronta para reabrir",
        }
    }

    /// O que vem ligado: o que chega de fora e pede alguém no balcão. O que
    /// só confirma um gesto do próprio operador nasce desligado.
    pub fn padrao(self) -> Escolha {
        let (ligado, som, texto) = match self {
            Evento::MensagemNoChatbot => (true, Embutido::Sino, "Mensagem de {titulo}"),
            Evento::ClienteNoQr => (true, Embutido::Campainha, "Cliente chegou pelo QR"),
            Evento::NovoAgendamento => (true, Embutido::DoisToques, "Novo agendamento: {detalhe}"),
            Evento::AgendamentoCancelado => {
                (true, Embutido::Descida, "Agendamento cancelado: {detalhe}")
            }
            Evento::Falha => (true, Embutido::Alerta, "Atenção: {detalhe}"),
            Evento::RevelacoesSalvas => (false, Embutido::Suave, "Fotos salvas na galeria"),
            Evento::ExportacaoConcluida => (false, Embutido::Suave, "Exportação concluída"),
            Evento::NovaVersao => (
                false,
                Embutido::Sino,
                "Nova versão instalada. Reabra o app quando puder",
            ),
        };
        Escolha {
            ligado,
            modo: Modo::Som,
            som: som.chave().into(),
            texto: texto.into(),
        }
    }

    /// O som de fábrica do evento — para onde volta um som próprio sumido.
    fn som_padrao(self) -> Embutido {
        Embutido::da_chave(&self.padrao().som).unwrap_or(Embutido::Sino)
    }

    /// Um aviso de exemplo, para o "Ouvir" das Configurações.
    pub fn exemplo(self) -> (&'static str, &'static str) {
        match self {
            Evento::MensagemNoChatbot => ("Maria Souza", "Bom dia, as fotos já estão prontas?"),
            Evento::ClienteNoQr => ("Maria Souza", "WhatsApp"),
            Evento::NovoAgendamento => ("Novo agendamento", "Maria Souza · 05/10, 14:00"),
            Evento::AgendamentoCancelado => ("Agendamento cancelado", "Maria Souza · 05/10, 14:00"),
            Evento::Falha => ("Falha", "o site recusou a venda"),
            Evento::RevelacoesSalvas => ("Fotos salvas", "12 fotos"),
            Evento::ExportacaoConcluida => ("Exportação concluída", "12 arquivos"),
            Evento::NovaVersao => ("Nova versão", "0.1.90"),
        }
    }
}

/// Troca `{titulo}` e `{detalhe}`, e apara o que sobrou de pontuação quando o
/// detalhe veio vazio ("Atenção: " vira "Atenção").
pub fn preencher(texto: &str, titulo: &str, detalhe: &str) -> String {
    texto
        .replace("{titulo}", titulo.trim())
        .replace("{detalhe}", detalhe.trim())
        .trim()
        .trim_end_matches([':', '·', ',', '-', ' '])
        .trim()
        .to_string()
}

/// O estado dos avisos sonoros do app.
pub struct Sons {
    preferencias: Preferencias,
    portas: PortasDoSom,
    /// O `sons.json`.
    arquivo: PathBuf,
    /// A pasta dos sons próprios — cópias, para o original poder sumir.
    pasta: PathBuf,
    ultimo: HashMap<Evento, Instant>,
}

impl Global for Sons {}

impl Sons {
    pub fn novo(portas: PortasDoSom, arquivo: PathBuf) -> Self {
        let pasta = arquivo
            .parent()
            .map(|pai| pai.join("sons"))
            .unwrap_or_else(|| PathBuf::from("sons"));
        Self {
            preferencias: preferencias::ler(&arquivo),
            portas,
            arquivo,
            pasta,
            ultimo: HashMap::new(),
        }
    }

    pub fn preferencias(&self) -> &Preferencias {
        &self.preferencias
    }

    pub fn portas(&self) -> &PortasDoSom {
        &self.portas
    }

    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Muda e grava na hora — não há botão "Salvar".
    pub fn mudar(&mut self, mudanca: impl FnOnce(&mut Preferencias)) {
        mudanca(&mut self.preferencias);
        preferencias::guardar(&self.arquivo, &self.preferencias);
    }

    /// O que tocar para `evento` — `None` quando nada deve soar.
    fn pedido(
        &self,
        evento: Evento,
        escolha: &Escolha,
        titulo: &str,
        detalhe: &str,
    ) -> Option<Pedido> {
        let som = escolha
            .modo
            .toca()
            .then(|| self.som(&escolha.som, evento.som_padrao()));
        let fala = escolha
            .modo
            .fala()
            .then(|| preencher(&escolha.texto, titulo, detalhe))
            .filter(|texto| !texto.is_empty())
            .map(|texto| Fala {
                texto,
                voz: self.preferencias.voz.clone(),
                ritmo: self.preferencias.ritmo,
            });
        (som.is_some() || fala.is_some()).then_some(Pedido {
            som,
            fala,
            volume: self.preferencias.volume,
        })
    }

    /// A escolha guardada vira o som a tocar. Som próprio que sumiu da pasta
    /// volta ao de fábrica do evento: o aviso nunca fica mudo por isso.
    fn som(&self, chave: &str, padrao: Embutido) -> Som {
        match chave.strip_prefix(MEU) {
            Some(nome) => {
                let caminho = self.pasta.join(nome);
                if caminho.is_file() {
                    Som::Proprio(caminho)
                } else {
                    Som::Embutido(padrao)
                }
            }
            None => Som::Embutido(Embutido::da_chave(chave).unwrap_or(padrao)),
        }
    }

    /// O aviso aconteceu: toca, se as chaves deixarem e o intervalo já passou.
    pub fn soar_em(&mut self, evento: Evento, titulo: &str, detalhe: &str, agora: Instant) {
        if !self.preferencias.ligados {
            return;
        }
        let escolha = self.preferencias.escolha(evento);
        if !escolha.ligado {
            return;
        }
        if let Some(antes) = self.ultimo.get(&evento) {
            if agora.saturating_duration_since(*antes) < INTERVALO_MINIMO {
                return;
            }
        }
        if let Some(pedido) = self.pedido(evento, &escolha, titulo, detalhe) {
            self.ultimo.insert(evento, agora);
            self.portas.alto_falante.tocar(pedido);
        }
    }

    /// O "Ouvir" das Configurações: toca do jeito escolhido, com o aviso de
    /// exemplo — mesmo com o tipo ou a chave geral desligados, que é quando o
    /// operador quer saber como seria.
    pub fn ouvir(&self, evento: Evento) {
        let escolha = self.preferencias.escolha(evento);
        let (titulo, detalhe) = evento.exemplo();
        if let Some(pedido) = self.pedido(evento, &escolha, titulo, detalhe) {
            self.portas.alto_falante.tocar(pedido);
        }
    }

    /// Toca um som só, no volume de agora — o "Ouvir" de um som próprio.
    pub fn ouvir_som(&self, chave: &str) {
        self.portas.alto_falante.tocar(Pedido {
            som: Some(self.som(chave, Embutido::Sino)),
            fala: None,
            volume: self.preferencias.volume,
        });
    }

    /// Os sons próprios guardados, por nome de arquivo.
    pub fn meus_sons(&self) -> Vec<String> {
        let mut nomes: Vec<String> = std::fs::read_dir(&self.pasta)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entrada| entrada.path().is_file())
            .filter_map(|entrada| entrada.file_name().into_string().ok())
            .filter(|nome| !nome.starts_with('.'))
            .collect();
        nomes.sort();
        nomes
    }

    /// Copia o arquivo para a pasta dos sons e devolve a chave dele
    /// (`meu:<nome>`). Recusa o que não decodifica.
    pub fn adicionar(&mut self, origem: &Path) -> Result<String, String> {
        porta::decodifica(origem).map_err(|erro| {
            format!(
                "Não deu para usar {}: {erro}.",
                origem
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        })?;
        let nome = origem
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("O nome do arquivo não é texto.")?;
        std::fs::create_dir_all(&self.pasta)
            .map_err(|erro| format!("Não deu para criar a pasta dos sons: {erro}."))?;
        let nome = nome_livre(&self.pasta, nome);
        std::fs::copy(origem, self.pasta.join(&nome))
            .map_err(|erro| format!("Não deu para copiar o som: {erro}."))?;
        Ok(format!("{MEU}{nome}"))
    }

    /// Apaga a cópia, e quem a usava volta ao som de fábrica.
    pub fn remover(&mut self, nome: &str) {
        let _ = std::fs::remove_file(self.pasta.join(nome));
        let chave = format!("{MEU}{nome}");
        self.mudar(|preferencias| {
            for evento in Evento::TODOS {
                let escolha = preferencias.escolha_mut(evento);
                if escolha.som == chave {
                    escolha.som = evento.padrao().som;
                }
            }
        });
    }
}

/// `alerta.mp3`, e se já existe, `alerta-2.mp3`, `alerta-3.mp3`…
fn nome_livre(pasta: &Path, nome: &str) -> String {
    if !pasta.join(nome).exists() {
        return nome.to_string();
    }
    let (base, extensao) = match nome.rsplit_once('.') {
        Some((base, extensao)) => (base, format!(".{extensao}")),
        None => (nome, String::new()),
    };
    (2..)
        .map(|n| format!("{base}-{n}{extensao}"))
        .find(|candidato| !pasta.join(candidato).exists())
        .expect("algum número está livre")
}

/// A chave da escolha de um som próprio.
pub fn chave_do_meu(nome: &str) -> String {
    format!("{MEU}{nome}")
}

/// 🔔 **O aviso aconteceu.** Quem avisa chama isto, com o que o aviso diz
/// (`titulo` e `detalhe` preenchem o texto falado); as regras são daqui.
pub fn soar(evento: Evento, titulo: &str, detalhe: &str, cx: &mut App) {
    if !cx.has_global::<Sons>() {
        return;
    }
    cx.global_mut::<Sons>()
        .soar_em(evento, titulo, detalhe, Instant::now());
}

#[cfg(test)]
mod testes {
    use super::porta::mentira::AltoFalanteDeMentira;
    use super::*;
    use std::sync::Arc;

    fn sons() -> (Sons, Arc<AltoFalanteDeMentira>, tempfile::TempDir) {
        let pasta = tempfile::TempDir::new().unwrap();
        let (portas, alto_falante) = PortasDoSom::de_mentira();
        (
            Sons::novo(portas, pasta.path().join("sons.json")),
            alto_falante,
            pasta,
        )
    }

    #[test]
    fn o_aviso_toca_o_som_do_evento_no_volume_escolhido() {
        let (mut sons, alto_falante, _pasta) = sons();
        sons.soar_em(
            Evento::NovoAgendamento,
            "Novo agendamento",
            "Ana",
            Instant::now(),
        );
        assert_eq!(
            alto_falante.pedidos(),
            vec![Pedido {
                som: Some(Som::Embutido(Embutido::DoisToques)),
                fala: None,
                volume: 0.7,
            }]
        );
    }

    #[test]
    fn chave_geral_ou_do_tipo_desligada_nao_toca() {
        let (mut sons, alto_falante, _pasta) = sons();
        sons.soar_em(Evento::RevelacoesSalvas, "", "", Instant::now());
        assert!(alto_falante.pedidos().is_empty(), "nasce desligado");

        sons.mudar(|p| p.ligados = false);
        sons.soar_em(Evento::Falha, "", "", Instant::now());
        assert!(alto_falante.pedidos().is_empty(), "chave geral desligada");

        sons.mudar(|p| {
            p.ligados = true;
            p.escolha_mut(Evento::Falha).ligado = false;
        });
        sons.soar_em(Evento::Falha, "", "", Instant::now());
        assert!(alto_falante.pedidos().is_empty(), "o tipo desligado");
    }

    /// Dez mensagens seguidas são um sino; passado o intervalo, toca de novo.
    #[test]
    fn o_mesmo_aviso_nao_toca_duas_vezes_no_intervalo() {
        let (mut sons, alto_falante, _pasta) = sons();
        let agora = Instant::now();
        for _ in 0..10 {
            sons.soar_em(Evento::MensagemNoChatbot, "Ana", "oi", agora);
        }
        sons.soar_em(Evento::Falha, "", "", agora);
        assert_eq!(alto_falante.pedidos().len(), 2, "outro tipo não espera");
        sons.soar_em(
            Evento::MensagemNoChatbot,
            "Ana",
            "oi",
            agora + INTERVALO_MINIMO,
        );
        assert_eq!(alto_falante.pedidos().len(), 3);
    }

    #[test]
    fn a_voz_fala_o_texto_com_o_aviso_dentro() {
        let (mut sons, alto_falante, _pasta) = sons();
        sons.mudar(|p| {
            p.voz = Some("Luciana".into());
            p.ritmo = 1.3;
            let escolha = p.escolha_mut(Evento::NovoAgendamento);
            escolha.modo = Modo::SomEVoz;
            escolha.texto = "Chegou {titulo} de {detalhe}".into();
            p.escolha_mut(Evento::Falha).modo = Modo::Voz;
        });
        sons.soar_em(
            Evento::NovoAgendamento,
            "agendamento",
            "Ana · 05/10",
            Instant::now(),
        );
        sons.soar_em(Evento::Falha, "Falha", "", Instant::now());
        let pedidos = alto_falante.pedidos();
        assert_eq!(pedidos[0].som, Some(Som::Embutido(Embutido::DoisToques)));
        assert_eq!(
            pedidos[0].fala,
            Some(Fala {
                texto: "Chegou agendamento de Ana · 05/10".into(),
                voz: Some("Luciana".into()),
                ritmo: 1.3,
            })
        );
        assert_eq!(pedidos[1].som, None, "só voz não toca som");
        assert_eq!(
            pedidos[1].fala.as_ref().map(|f| f.texto.as_str()),
            Some("Atenção"),
            "o detalhe vazio não deixa ': ' pendurado"
        );
    }

    #[test]
    fn so_voz_com_texto_vazio_nao_pede_nada() {
        let (mut sons, alto_falante, _pasta) = sons();
        sons.mudar(|p| {
            let escolha = p.escolha_mut(Evento::Falha);
            escolha.modo = Modo::Voz;
            escolha.texto = "{detalhe}".into();
        });
        sons.soar_em(Evento::Falha, "", "  ", Instant::now());
        assert!(alto_falante.pedidos().is_empty());
    }

    /// 🔑 O som próprio é uma cópia; apagado, o aviso volta ao de fábrica em
    /// vez de ficar mudo.
    #[test]
    fn som_proprio_copiado_e_quando_some_volta_ao_de_fabrica() {
        let (mut sons, alto_falante, pasta) = sons();
        let original = pasta.path().join("buzina.wav");
        std::fs::write(&original, porta::wav_de_teste(800)).unwrap();

        let chave = sons.adicionar(&original).expect("WAV válido entra");
        assert_eq!(chave, "meu:buzina.wav");
        assert_eq!(sons.adicionar(&original).unwrap(), "meu:buzina-2.wav");
        assert_eq!(sons.meus_sons(), vec!["buzina-2.wav", "buzina.wav"]);
        std::fs::remove_file(&original).unwrap();

        sons.mudar(|p| p.escolha_mut(Evento::Falha).som = chave.clone());
        sons.soar_em(Evento::Falha, "", "", Instant::now());
        assert_eq!(
            alto_falante.pedidos()[0].som,
            Some(Som::Proprio(sons.pasta().join("buzina.wav"))),
            "toca a cópia, e não o original que sumiu"
        );

        std::fs::remove_file(sons.pasta().join("buzina.wav")).unwrap();
        sons.ouvir(Evento::Falha);
        assert_eq!(
            alto_falante.pedidos()[1].som,
            Some(Som::Embutido(Embutido::Alerta))
        );
    }

    #[test]
    fn remover_o_som_devolve_quem_o_usava_ao_padrao() {
        let (mut sons, _alto_falante, pasta) = sons();
        let original = pasta.path().join("apito.wav");
        std::fs::write(&original, porta::wav_de_teste(800)).unwrap();
        let chave = sons.adicionar(&original).unwrap();
        sons.mudar(|p| {
            p.escolha_mut(Evento::ClienteNoQr).som = chave.clone();
            p.escolha_mut(Evento::Falha).som = chave.clone();
        });

        sons.remover("apito.wav");
        assert!(sons.meus_sons().is_empty());
        let lidas = preferencias::ler(&pasta.path().join("sons.json"));
        assert_eq!(lidas.escolha(Evento::ClienteNoQr).som, "campainha");
        assert_eq!(lidas.escolha(Evento::Falha).som, "alerta");
    }

    #[test]
    fn arquivo_que_nao_e_som_nao_entra() {
        let (mut sons, _alto_falante, pasta) = sons();
        let falso = pasta.path().join("planilha.mp3");
        std::fs::write(&falso, b"nome;valor\n").unwrap();
        let erro = sons.adicionar(&falso).unwrap_err();
        assert!(erro.contains("planilha.mp3"), "{erro}");
        assert!(sons.meus_sons().is_empty());
    }

    /// O "Ouvir" toca mesmo com tudo desligado — é quando se quer saber.
    #[test]
    fn ouvir_ignora_as_chaves() {
        let (mut sons, alto_falante, _pasta) = sons();
        sons.mudar(|p| p.ligados = false);
        sons.ouvir(Evento::RevelacoesSalvas);
        assert_eq!(
            alto_falante.pedidos()[0].som,
            Some(Som::Embutido(Embutido::Suave))
        );
    }

    #[test]
    fn as_chaves_voltam_ao_evento_e_ao_modo() {
        for evento in Evento::TODOS {
            assert_eq!(Evento::da_chave(evento.chave()), Some(evento));
            assert!(Embutido::da_chave(&evento.padrao().som).is_some());
        }
        for modo in Modo::TODOS {
            assert_eq!(Modo::da_chave(modo.chave()), Some(modo));
        }
    }
}
