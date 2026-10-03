//! 🔁 **Voltar onde estávamos** — depois de atualizar e depois de fechar.
//!
//! *"Ao aceitar uma atualização de versão e reinicializar a aplicação, volte ao
//! mesmo estado da aplicação, ou seja onde parou/onde estávamos. O mesmo vale
//! quando fechamos a aplicação. Faça isso quando possível sem termos efeitos
//! colaterais."* (dono, 03/out/2026)
//!
//! Até aqui o app reabria sempre na lista de sessões: o "Reabrir agora" da
//! atualização sai com `exit(0)` e o operador, no meio de um atendimento,
//! perdia a sessão da frente e a foto que estava revelando. É o que o
//! Lightroom faz ao reabrir — volta no módulo e na foto em que ficou.
//!
//! # O que volta, e o que não volta
//!
//! | Volta | Não volta |
//! |---|---|
//! | a tela (sessão, Revelação, caixa, agenda, chatbot, backup…) | os modais (importação, exportação, balcão, configurações): reabrir um deles retomaria uma ação |
//! | a guia da frente | o assistente da nova sessão e a impressão: o estado deles não se guarda, e eles caem na lista e na sessão |
//! | a foto da Revelação, na guia da frente **e** nas de trás | o editor em camadas, o zoom e o desfazer |
//! | o menu lateral aberto ou recolhido | o recorte da tira |
//! | a **tela do cliente**, só depois de **atualizar** (até 15 min) | a tela do cliente depois de **fechar**: quem estiver na frente do monitor na próxima abertura pode ser outro cliente, e veria as fotos do anterior |
//!
//! 🔑 **Gravado enquanto se trabalha, e não só ao sair.** O retrato vai para o
//! disco a cada troca de tela, de sessão ou de foto — é pequeno, e só se grava
//! quando muda. Assim ele sobrevive ao `exit(0)` da atualização, a uma queda e
//! a um `kill`; o fim do app só carimba o motivo ([`Motivo`]).
//!
//! 🚨 **Sair também não pode perder o último gesto.** O ajuste da Revelação vai
//! ao banco numa tarefa do tokio, e o processo que acaba a leva junto. Antes de
//! sair, o gesto em curso é fechado e o app espera as gravações no ar
//! ([`Aplicativo::preparar_para_sair`]).
//!
//! ⚠️ **Nunca puxa a tela de quem já começou.** A retomada só acontece se,
//! quando o `/auth/me` responde, o operador ainda está na lista de sessões.
//! O arquivo é preso à conta (outra conta não herda) e some no "Sair" da conta.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, Context, Window};
use serde::{Deserialize, Serialize};

use super::{Aplicativo, Tela};
use crate::revelacao::persistencia;

/// Até quando, depois de uma atualização, a tela do cliente volta sozinha. Mais
/// que isso, o atendimento de antes pode ter acabado.
const CLIENTE_DEPOIS_DE_ATUALIZAR: i64 = 15 * 60;

/// Quanto o app espera as gravações no ar antes de sair.
const PRAZO_DAS_GRAVACOES: Duration = Duration::from_secs(2);

/// Quanto tempo a tela do cliente espera ter uma foto para mostrar.
const PRAZO_DO_CLIENTE: Duration = Duration::from_secs(60);

/// Por que o retrato foi gravado pela última vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// No meio do trabalho — se o app parou depois disso, foi queda ou `kill`.
    #[default]
    Trabalhando,
    /// O operador fechou o app.
    Fechou,
    /// O "Reabrir agora" da atualização.
    Atualizou,
}

/// Onde o operador está — o que muda com o trabalho, sem o carimbo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Retrato {
    /// O nome estável da tela ([`Tela::nome_para_desempenho`]).
    pub tela: String,
    /// A galeria da guia da frente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessao: Option<String>,
    /// A foto aberta na Revelação, por galeria (id da grade da sessão) — a da
    /// frente e as das guias de trás que ficaram na Revelação.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub revelando: BTreeMap<String, String>,
    #[serde(default)]
    pub menu_aberto: bool,
    #[serde(default)]
    pub tela_do_cliente: bool,
}

/// O `onde-estavamos.json`: o retrato, de quem é e quando foi tirado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OndeEstavamos {
    pub conta: String,
    /// A versão que gravou — para o rastro da atualização.
    pub versao: String,
    /// Segundos unix.
    pub gravado_em: i64,
    #[serde(default)]
    pub motivo: Motivo,
    #[serde(flatten)]
    pub retrato: Retrato,
}

/// Para onde a abertura leva, já com os rebaixamentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    pub tela: Tela,
    pub sessao: Option<String>,
    /// A foto a reabrir na Revelação da sessão da frente.
    pub foto: Option<String>,
    /// As guias de trás que estavam na Revelação: `galeria → foto`.
    pub outras: HashMap<String, String>,
    pub menu_aberto: bool,
    pub cliente: bool,
}

/// Lê o arquivo, **só se for desta conta**.
pub fn de_json(json: &str, conta: &str) -> Option<OndeEstavamos> {
    serde_json::from_str::<OndeEstavamos>(json)
        .ok()
        .filter(|onde| onde.conta == conta)
}

/// Para onde voltar.
///
/// - a nova sessão e a impressão não guardam o que estava nelas: a primeira
///   volta à lista, a segunda à sessão de onde saiu;
/// - a Revelação sem foto lembrada volta à grade da sessão;
/// - dentro de sessão, sem a sessão, é a lista;
/// - a tela do cliente só volta depois de uma **atualização** recente.
pub fn destino(onde: &OndeEstavamos, agora: i64) -> Destino {
    let retrato = &onde.retrato;
    let lembrada = Tela::do_nome(&retrato.tela).unwrap_or(Tela::Sessoes);
    let da_sessao = matches!(
        lembrada,
        Tela::Sessao | Tela::Revelacao | Tela::Impressao | Tela::Biblioteca
    );
    let sessao = retrato.sessao.clone().filter(|_| da_sessao);
    let foto = sessao
        .as_ref()
        .filter(|_| lembrada == Tela::Revelacao)
        .and_then(|id| retrato.revelando.get(id).cloned());
    let tela = match (lembrada, &sessao) {
        (Tela::NovaSessao, _) => Tela::Sessoes,
        (_, Some(_)) if da_sessao => {
            if foto.is_some() {
                Tela::Revelacao
            } else {
                Tela::Sessao
            }
        }
        _ if da_sessao => Tela::Sessoes,
        (outra, _) => outra,
    };
    let outras = retrato
        .revelando
        .iter()
        .filter(|(galeria, _)| sessao.as_ref() != Some(*galeria))
        .map(|(g, f)| (g.clone(), f.clone()))
        .collect();
    let cliente = retrato.tela_do_cliente
        && sessao.is_some()
        && onde.motivo == Motivo::Atualizou
        && (0..=CLIENTE_DEPOIS_DE_ATUALIZAR).contains(&(agora - onde.gravado_em));
    Destino {
        tela,
        sessao,
        foto,
        outras,
        menu_aberto: retrato.menu_aberto,
        cliente,
    }
}

/// Onde o retrato fica entre aberturas, ao lado do catálogo. Os testes nunca
/// tocam o arquivo de quem trabalha.
pub(super) fn arquivo() -> Option<PathBuf> {
    (!cfg!(test))
        .then(|| infrastructure::paths::AppPaths::catalog_root().join("onde-estavamos.json"))
}

/// Grava por um arquivo ao lado e troca: um app que morre no meio da escrita
/// não deixa meio JSON para a próxima abertura.
fn gravar(arquivo: &Path, onde: &OndeEstavamos) {
    let Ok(json) = serde_json::to_string_pretty(onde) else {
        return;
    };
    let provisorio = arquivo.with_extension("json.novo");
    if std::fs::write(&provisorio, json).is_ok() {
        let _ = std::fs::rename(&provisorio, arquivo);
    }
}

fn agora() -> i64 {
    chrono::Utc::now().timestamp()
}

/// O id da foto na grade da sessão: o do site sem o prefixo, ou o local.
fn id_na_grade(id: &str) -> String {
    persistencia::id_no_site(id).unwrap_or(id).to_string()
}

/// O estado da retomada dentro da raiz.
#[derive(Default)]
pub(super) struct Retomada {
    pub(super) arquivo: Option<PathBuf>,
    /// A janela principal — é por ela que a retomada sai assim que a conta
    /// chega, sem esperar o próximo desenho.
    janela: Option<AnyWindowHandle>,
    /// O arquivo já foi lido nesta entrada da conta.
    decidida: bool,
    /// O destino lido, esperando a janela para ser executado.
    pendente: Option<Destino>,
    /// O destino saiu para o `defer` e ainda não foi executado.
    em_curso: bool,
    /// O último retrato gravado — só se grava o que mudou.
    ultimo: Option<Retrato>,
    /// As guias de trás que voltam na Revelação quando o operador for a elas.
    pub(super) revelar_ao_voltar: HashMap<String, String>,
    /// A tela do cliente voltando: tenta até haver foto para mostrar.
    _cliente: Option<gpui_kit::Task<()>>,
    /// O app está saindo: o retrato com o motivo já foi gravado.
    saindo: bool,
}

impl Retomada {
    pub(super) fn nova(arquivo: Option<PathBuf>, janela: Option<AnyWindowHandle>) -> Self {
        Self {
            arquivo,
            janela,
            ..Self::default()
        }
    }
}

impl Aplicativo {
    /// O retrato de agora.
    fn retrato_de_agora(&self, cx: &Context<Self>) -> Retrato {
        let mut revelando: BTreeMap<String, String> = self
            .retomada
            .revelar_ao_voltar
            .iter()
            .map(|(g, f)| (g.clone(), f.clone()))
            .collect();
        for (galeria, foto) in self.guias.fotos_reveladas() {
            revelando.insert(galeria, id_na_grade(&foto));
        }
        let mut tela = self.tela;
        if let Some(sessao) = &self.sessao_aberta {
            match self.tela {
                Tela::Revelacao => {
                    if let Some(foto) = self.revelacao.read(cx).foto_aberta() {
                        revelando.insert(sessao.clone(), id_na_grade(&foto.id));
                    }
                }
                // A Revelação lembrada ainda esperando a galeria chegar: se o
                // app fechar agora, a próxima abertura continua querendo voltar
                // nela.
                Tela::Sessao => {
                    if let Some(foto) = self.detalhe.read(cx).revelacao_a_retomar() {
                        tela = Tela::Revelacao;
                        revelando.insert(sessao.clone(), foto.to_string());
                    }
                }
                _ => {}
            }
        }
        Retrato {
            tela: tela.nome_para_desempenho().to_string(),
            sessao: self.sessao_aberta.clone(),
            revelando,
            menu_aberto: self.menu_aberto,
            tela_do_cliente: self.cliente.is_some(),
        }
    }

    fn gravar_o_retrato(&mut self, retrato: Retrato, motivo: Motivo) {
        let (Some(arquivo), Some(conta)) = (&self.retomada.arquivo, &self.conta) else {
            return;
        };
        gravar(
            arquivo,
            &OndeEstavamos {
                conta: conta.email.clone(),
                versao: env!("CARGO_PKG_VERSION").to_string(),
                gravado_em: agora(),
                motivo,
                retrato: retrato.clone(),
            },
        );
        self.retomada.ultimo = Some(retrato);
    }

    /// Grava onde o operador está, se mudou desde a última vez.
    ///
    /// 🚨 **Só depois de a retomada ser decidida e executada.** Antes disso a
    /// tela é a lista de sessões da abertura, e gravá-la apagaria justamente o
    /// que a abertura ia ler.
    pub(super) fn lembrar_onde_estamos(&mut self, cx: &mut Context<Self>) {
        let r = &self.retomada;
        if r.saindo || !r.decidida || r.pendente.is_some() || r.em_curso || self.conta.is_none() {
            return;
        }
        let retrato = self.retrato_de_agora(cx);
        if self.retomada.ultimo.as_ref() != Some(&retrato) {
            self.gravar_o_retrato(retrato, Motivo::Trabalhando);
        }
    }

    /// A conta chegou: lê onde ela estava, uma vez por entrada.
    pub(super) fn decidir_a_retomada(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.retomada.decidida, true) {
            return;
        }
        let (Some(arquivo), Some(conta)) = (&self.retomada.arquivo, &self.conta) else {
            return;
        };
        let Some(onde) = std::fs::read_to_string(arquivo)
            .ok()
            .and_then(|json| de_json(&json, &conta.email))
        else {
            return;
        };
        if self.tela != Tela::Sessoes || self.sessao_aberta.is_some() {
            eprintln!("🔁 [Retomada] o operador já saiu da lista: fica onde está");
            return;
        }
        let destino = destino(&onde, agora());
        eprintln!(
            "🔁 [Retomada] {:?} na {}: {} · sessão {:?} · foto {:?} · {} guia(s) na Revelação · cliente {}",
            onde.motivo,
            onde.versao,
            destino.tela.nome_para_desempenho(),
            destino.sessao,
            destino.foto,
            destino.outras.len(),
            destino.cliente,
        );
        self.retomada.pendente = Some(destino);
        // 🚨 **Sai já, pela janela, e não no próximo desenho.** Esperar o
        // `render` deixava a retomada presa enquanto a janela não desenha —
        // tela bloqueada, janela tapada ou minimizada —, e a primeira tecla do
        // operador chegava à grade antes de a Revelação abrir (visto no app
        // real, com o roteiro, em 03/out/2026). O `render` fica como reserva.
        if let Some(janela) = self.retomada.janela {
            let raiz = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = janela.update(cx, |_, window, cx| {
                    let _ = raiz.update(cx, |raiz, cx| {
                        if let Some(destino) = raiz.retomada.pendente.take() {
                            raiz.executar_a_retomada(destino, window, cx);
                        }
                    });
                });
            });
        }
        cx.notify();
    }

    /// Chamada pelo `render` da raiz, que tem a janela: o destino lido vai para
    /// o `defer`, e o retrato de agora para o disco.
    pub(super) fn cuidar_da_retomada(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(destino) = self.retomada.pendente.take() {
            self.retomada.em_curso = true;
            cx.defer_in(window, move |raiz, window, cx| {
                raiz.executar_a_retomada(destino, window, cx)
            });
            return;
        }
        self.lembrar_onde_estamos(cx);
    }

    fn executar_a_retomada(
        &mut self,
        destino: Destino,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.retomada.em_curso = false;
        // O operador foi mais rápido que o `defer`.
        if self.tela != Tela::Sessoes || self.sessao_aberta.is_some() {
            return;
        }
        self.menu_aberto = destino.menu_aberto;
        self.retomada.revelar_ao_voltar = destino.outras;
        match (destino.tela, destino.sessao) {
            (Tela::Sessao | Tela::Revelacao, Some(sessao)) => {
                self.entrar_na_sessao(sessao, cx);
                if let Some(foto) = destino.foto {
                    self.detalhe
                        .update(cx, |tela, cx| tela.revelar_ao_carregar(foto, cx));
                }
                if destino.cliente {
                    self.retomada._cliente = Some(Self::reabrir_o_cliente(cx));
                }
            }
            (Tela::Sessoes, _) => {}
            (outra, _) => self.ir_para(outra, window, cx),
        }
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// A tela do cliente volta assim que houver foto para ela — a galeria e a
    /// Revelação chegam depois da sessão. Por relógio, e não pelo desenho: a
    /// janela que não desenha (tapada, tela bloqueada) não segura a volta.
    fn reabrir_o_cliente(cx: &mut Context<Self>) -> gpui_kit::Task<()> {
        cx.spawn(async move |raiz, cx| {
            let ate = Instant::now() + PRAZO_DO_CLIENTE;
            while Instant::now() < ate {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let Ok(feito) = raiz.update(cx, |raiz, cx| {
                    if raiz.cliente.is_some() {
                        return true;
                    }
                    if raiz.foto_para_o_cliente(cx).is_none() {
                        return false;
                    }
                    // O monitor da última vez, sem perguntar — ver `reabrir_cliente`.
                    raiz.reabrir_cliente(cx);
                    eprintln!("🔁 [Retomada] a tela do cliente voltou");
                    true
                }) else {
                    return;
                };
                if feito {
                    return;
                }
            }
        })
    }

    /// O app vai sair — fechado ou reaberto pela atualização.
    ///
    /// 1. o retrato vai para o disco com o motivo, **antes** de tudo;
    /// 2. o gesto em curso da Revelação fecha e vai ao banco, sem trocar de
    ///    tela (o que `sair_da_revelacao` grava);
    /// 3. o app espera as gravações no ar.
    pub(super) fn preparar_para_sair(&mut self, motivo: Motivo, cx: &mut Context<Self>) {
        if self.retomada.saindo {
            return;
        }
        // Com a retomada ainda por executar, o arquivo continua o de antes, e
        // é ele que vale para a próxima abertura.
        let r = &self.retomada;
        if r.decidida && r.pendente.is_none() && !r.em_curso && self.conta.is_some() {
            let retrato = self.retrato_de_agora(cx);
            self.gravar_o_retrato(retrato, motivo);
        }
        self.retomada.saindo = true;
        if self.tela == Tela::Revelacao {
            self.revelacao.update(cx, |tela, _cx| {
                tela.gravar_o_que_estiver_pendente();
                tela.guardar_a_revelada_no_cache();
            });
            self.guardar_os_parametros_do_site(cx);
        }
        self.gravador.esperar_as_gravacoes(PRAZO_DAS_GRAVACOES);
    }

    /// A saída não aconteceu (o "Reabrir agora" não achou o app instalado): o
    /// retrato volta a acompanhar o trabalho.
    pub(super) fn voltar_a_lembrar(&mut self) {
        self.retomada.saindo = false;
    }

    /// O "Sair" da conta: o retrato era dela, e a próxima entrada começa do
    /// zero.
    pub(super) fn esquecer_onde_estavamos(&mut self) {
        if let Some(arquivo) = &self.retomada.arquivo {
            let _ = std::fs::remove_file(arquivo);
        }
        let (arquivo, janela) = (self.retomada.arquivo.take(), self.retomada.janela);
        self.retomada = Retomada::nova(arquivo, janela);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const TODAS: [Tela; 12] = [
        Tela::Biblioteca,
        Tela::Revelacao,
        Tela::Impressao,
        Tela::Sessoes,
        Tela::Sessao,
        Tela::Caixa,
        Tela::Retencao,
        Tela::NovaSessao,
        Tela::Backup,
        Tela::Chatbot,
        Tela::Agenda,
        Tela::Recuperacao,
    ];

    fn onde(tela: Tela, sessao: Option<&str>, revelando: &[(&str, &str)]) -> OndeEstavamos {
        OndeEstavamos {
            conta: "balcao@recordarfotos.com.br".into(),
            versao: "0.1.78".into(),
            gravado_em: 1_000,
            motivo: Motivo::Fechou,
            retrato: Retrato {
                tela: tela.nome_para_desempenho().into(),
                sessao: sessao.map(str::to_string),
                revelando: revelando
                    .iter()
                    .map(|(g, f)| (g.to_string(), f.to_string()))
                    .collect(),
                menu_aberto: false,
                tela_do_cliente: false,
            },
        }
    }

    #[test]
    fn o_nome_da_tela_vai_e_volta() {
        for tela in TODAS {
            assert_eq!(Tela::do_nome(tela.nome_para_desempenho()), Some(tela));
        }
        assert_eq!(Tela::do_nome("modulo-que-nao-existe"), None);
    }

    #[test]
    fn o_arquivo_de_outra_conta_nao_volta() {
        let json = serde_json::to_string(&onde(Tela::Sessao, Some("g1"), &[])).unwrap();
        assert!(de_json(&json, "outra@recordarfotos.com.br").is_none());
        assert!(de_json(&json, "balcao@recordarfotos.com.br").is_some());
        assert!(de_json("{ meio json", "balcao@recordarfotos.com.br").is_none());
    }

    #[test]
    fn a_revelacao_volta_na_foto_da_sessao_da_frente() {
        let d = destino(
            &onde(Tela::Revelacao, Some("g1"), &[("g1", "f7"), ("g2", "f3")]),
            1_000,
        );
        assert_eq!(d.tela, Tela::Revelacao);
        assert_eq!(d.sessao.as_deref(), Some("g1"));
        assert_eq!(d.foto.as_deref(), Some("f7"));
        // A guia de trás volta na Revelação quando o operador for a ela.
        assert_eq!(
            d.outras,
            HashMap::from([("g2".to_string(), "f3".to_string())])
        );
    }

    #[test]
    fn a_revelacao_sem_foto_lembrada_volta_na_grade() {
        let d = destino(&onde(Tela::Revelacao, Some("g1"), &[("g2", "f3")]), 1_000);
        assert_eq!(d.tela, Tela::Sessao);
        assert_eq!(d.foto, None);
    }

    #[test]
    fn a_impressao_volta_na_sessao_e_a_nova_sessao_na_lista() {
        let impressao = destino(&onde(Tela::Impressao, Some("g1"), &[]), 1_000);
        assert_eq!(
            (impressao.tela, impressao.sessao.as_deref()),
            (Tela::Sessao, Some("g1"))
        );
        let nova = destino(&onde(Tela::NovaSessao, None, &[]), 1_000);
        assert_eq!(nova.tela, Tela::Sessoes);
    }

    #[test]
    fn dentro_de_sessao_sem_a_sessao_e_a_lista() {
        for tela in [Tela::Sessao, Tela::Revelacao, Tela::Biblioteca] {
            assert_eq!(destino(&onde(tela, None, &[]), 1_000).tela, Tela::Sessoes);
        }
    }

    #[test]
    fn as_telas_de_secao_voltam_como_estavam() {
        for tela in [
            Tela::Caixa,
            Tela::Agenda,
            Tela::Chatbot,
            Tela::Backup,
            Tela::Retencao,
            Tela::Recuperacao,
            Tela::Sessoes,
        ] {
            let d = destino(&onde(tela, None, &[]), 1_000);
            assert_eq!((d.tela, d.sessao), (tela, None));
        }
    }

    #[test]
    fn a_tela_do_cliente_so_volta_depois_de_atualizar_ha_pouco() {
        let mut o = onde(Tela::Revelacao, Some("g1"), &[("g1", "f1")]);
        o.retrato.tela_do_cliente = true;

        o.motivo = Motivo::Fechou;
        assert!(
            !destino(&o, 1_010).cliente,
            "fechar não traz o cliente de volta"
        );
        o.motivo = Motivo::Trabalhando;
        assert!(!destino(&o, 1_010).cliente, "nem a queda");

        o.motivo = Motivo::Atualizou;
        assert!(destino(&o, 1_010).cliente);
        assert!(destino(&o, 1_000 + CLIENTE_DEPOIS_DE_ATUALIZAR).cliente);
        assert!(!destino(&o, 1_001 + CLIENTE_DEPOIS_DE_ATUALIZAR).cliente);
    }

    #[test]
    fn o_arquivo_e_trocado_inteiro() {
        let pasta = std::env::temp_dir().join(format!("vlb-retomada-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("onde-estavamos.json");
        let o = onde(Tela::Revelacao, Some("g1"), &[("g1", "f1")]);
        gravar(&arquivo, &o);
        let lido = de_json(
            &std::fs::read_to_string(&arquivo).unwrap(),
            "balcao@recordarfotos.com.br",
        );
        assert_eq!(lido, Some(o));
        assert!(!arquivo.with_extension("json.novo").exists());
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
