//! 📴 **O espelho do site** — o que o balcão leu da API pela última vez, guardado
//! em disco para a hora em que a internet cai (20/10/2026).
//!
//! # Por que existe
//!
//! > *"Hoje ficamos sem internet no estúdio, com isso descobrimos o que já era
//! > certo, o sistema não trabalha offline."* — dono, 20/10/2026.
//!
//! Toda tela de trabalho do app lia direto da API: sem rede, a lista de sessões
//! vinha vazia, a sessão aberta não abria e o caixa não mostrava nada. O espelho
//! guarda **a resposta crua** de cada leitura (`GET`), por rota, e o cliente HTTP
//! a devolve quando o site não responde. A tela não sabe a diferença — e é isso
//! que mantém o trabalho de pé.
//!
//! # O que ele guarda, e o que não
//!
//! - 🔑 **Bytes crus, e não os tipos do domínio.** O que volta passa pelo mesmo
//!   leitor da resposta de verdade: uma tradução só, sem uma segunda que pudesse
//!   divergir da primeira.
//! - 🔑 **Só serve quando a rede falha.** Com o site respondendo, a leitura é
//!   sempre a do site — o espelho só é regravado.
//! - ⚠️ **O original de 30 MB fica de fora** (`espelhavel`): é o arquivo cheio
//!   do zoom e da revelação final, e duzentas fotos dele encheriam o disco do
//!   balcão. A miniatura e a cópia de trabalho bastam para triar e negociar.
//! - ⚠️ **O espelho não é a verdade.** O que mudou no site depois da última
//!   leitura não está aqui; o que mudou **aqui** sem rede está na fila de
//!   sincronização do app, que a tela soma por cima.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

/// Uma leitura guardada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lembranca {
    pub bytes: Vec<u8>,
    pub tipo: Option<String>,
    /// Segundos desde a época — quando o site mandou isto.
    pub visto_em: i64,
}

/// Onde as leituras ficam.
///
/// 🔑 **Sem `Result`**, como as outras portas da casa: guardar é acessório, e
/// uma falha do disco não pode derrubar a leitura que deu certo.
pub trait Espelho: Send + Sync {
    fn guardar(&self, chave: &str, bytes: &[u8], tipo: Option<&str>);
    fn ler(&self, chave: &str) -> Option<Lembranca>;
}

/// A rota que vale guardar. O original e a fonte da revelação são o arquivo
/// cheio; o fluxo de eventos não é leitura que se repita.
pub fn espelhavel(caminho: &str) -> bool {
    let rota = caminho.split('?').next().unwrap_or(caminho);
    !(rota.ends_with("/original")
        || rota.ends_with("/fonte-para-revelar")
        || rota.contains("/eventos")
        || rota.starts_with("/auth/"))
}

fn agora() -> i64 {
    chrono::Utc::now().timestamp()
}

/// O espelho em disco: um arquivo por rota, com o nome pelo SHA-256 dela.
pub struct EspelhoEmDisco {
    pasta: PathBuf,
}

impl EspelhoEmDisco {
    pub fn novo(pasta: impl Into<PathBuf>) -> Self {
        let pasta = pasta.into();
        let _ = std::fs::create_dir_all(&pasta);
        Self { pasta }
    }

    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    fn nome(&self, chave: &str) -> PathBuf {
        let hash = Sha256::digest(chave.as_bytes());
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        self.pasta.join(hex)
    }

    /// Apaga o que não foi relido há mais de `dias`. Roda na abertura do app:
    /// uma sessão de três meses atrás não vai ser aberta sem rede, e as
    /// miniaturas dela ocupariam o disco do balcão para sempre.
    pub fn faxina(&self, dias: i64) -> usize {
        let limite = std::time::SystemTime::now()
            - std::time::Duration::from_secs((dias.max(1) as u64) * 86_400);
        let Ok(entradas) = std::fs::read_dir(&self.pasta) else {
            return 0;
        };
        let mut apagados = 0;
        for entrada in entradas.flatten() {
            let velho = entrada
                .metadata()
                .and_then(|m| m.modified())
                .map(|m| m < limite)
                .unwrap_or(false);
            if velho && std::fs::remove_file(entrada.path()).is_ok() {
                apagados += 1;
            }
        }
        apagados
    }
}

/// Grava por cima com troca atômica: um app fechado no meio da gravação deixa
/// o arquivo anterior, nunca metade do novo.
///
/// 🚨 **O provisório tem nome próprio por gravação.** Duas leituras da mesma
/// rota ao mesmo tempo (o caixa e a sessão pedem o catálogo juntos) usavam o
/// mesmo provisório, e a troca de uma levava o arquivo da outra: "No such file
/// or directory" no log do e2e (20/10/2026).
fn gravar_atomico(destino: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let provisorio = destino.with_extension(format!("{}.gravando", uuid::Uuid::new_v4().simple()));
    std::fs::write(&provisorio, bytes)?;
    std::fs::rename(&provisorio, destino)
}

impl Espelho for EspelhoEmDisco {
    fn guardar(&self, chave: &str, bytes: &[u8], tipo: Option<&str>) {
        let base = self.nome(chave);
        // Os dois arquivos: o corpo e o cabeçalho (`visto_em` e o tipo). O
        // cabeçalho vai por último — sem ele a leitura não acontece, e um corpo
        // novo com o cabeçalho velho não se mistura.
        let cabecalho = format!("{}\n{}", agora(), tipo.unwrap_or(""));
        // A pasta pode ter sumido com o app aberto (a faxina do disco, um
        // catálogo recriado): ela volta antes de gravar.
        let _ = std::fs::create_dir_all(&self.pasta);
        let corpo = base.with_extension("corpo");
        let cab = base.with_extension("cab");
        if let Err(erro) =
            gravar_atomico(&corpo, bytes).and_then(|_| gravar_atomico(&cab, cabecalho.as_bytes()))
        {
            eprintln!("⚠️ [Espelho] não guardou {chave}: {erro}");
        }
    }

    fn ler(&self, chave: &str) -> Option<Lembranca> {
        let base = self.nome(chave);
        let cabecalho = std::fs::read_to_string(base.with_extension("cab")).ok()?;
        let bytes = std::fs::read(base.with_extension("corpo")).ok()?;
        let mut linhas = cabecalho.splitn(2, '\n');
        let visto_em = linhas.next()?.trim().parse().ok()?;
        let tipo = linhas
            .next()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        Some(Lembranca {
            bytes,
            tipo,
            visto_em,
        })
    }
}

/// O espelho de quem não tem disco — o padrão do cliente HTTP, e o dos testes.
#[derive(Default)]
pub struct EspelhoEmMemoria {
    leituras: Mutex<HashMap<String, Lembranca>>,
}

impl Espelho for EspelhoEmMemoria {
    fn guardar(&self, chave: &str, bytes: &[u8], tipo: Option<&str>) {
        if let Ok(mut leituras) = self.leituras.lock() {
            leituras.insert(
                chave.to_string(),
                Lembranca {
                    bytes: bytes.to_vec(),
                    tipo: tipo.map(str::to_string),
                    visto_em: agora(),
                },
            );
        }
    }

    fn ler(&self, chave: &str) -> Option<Lembranca> {
        self.leituras.lock().ok()?.get(chave).cloned()
    }
}

/// 📶 **Se o site está respondendo** — o que o selo do rodapé mostra e o que a
/// fila de sincronização consulta antes de tentar.
///
/// Quem muda o estado é o cliente HTTP: qualquer resposta do site (mesmo uma
/// recusa) é rede; "sem resposta" ou `5xx` é falta dela.
#[derive(Debug, Default)]
pub struct EstadoDaRede {
    /// `0` = com rede; senão, desde quando (segundos desde a época) não há.
    sem_rede_desde: AtomicI64,
    /// Quando o espelho respondeu no lugar do site pela última vez.
    ultima_lembranca: AtomicI64,
}

impl EstadoDaRede {
    pub fn com_rede(&self) -> bool {
        self.sem_rede_desde.load(Ordering::Relaxed) == 0
    }

    /// Desde quando o site não responde. `None` = está respondendo.
    pub fn sem_rede_desde(&self) -> Option<i64> {
        match self.sem_rede_desde.load(Ordering::Relaxed) {
            0 => None,
            desde => Some(desde),
        }
    }

    pub fn marcar_sem_rede(&self) {
        let _ = self.sem_rede_desde.compare_exchange(
            0,
            agora().max(1),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    pub fn marcar_com_rede(&self) {
        self.sem_rede_desde.store(0, Ordering::Relaxed);
    }

    /// O espelho respondeu no lugar do site — a tela pode estar vendo dados de
    /// antes. Guardado para o selo dizer "visto às 14h".
    pub fn lembrou(&self, visto_em: i64) {
        self.ultima_lembranca.store(visto_em, Ordering::Relaxed);
    }

    pub fn ultima_lembranca(&self) -> Option<i64> {
        match self.ultima_lembranca.load(Ordering::Relaxed) {
            0 => None,
            v => Some(v),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_disco_guarda_e_devolve_bytes_e_tipo() {
        let pasta = tempfile::tempdir().unwrap();
        let espelho = EspelhoEmDisco::novo(pasta.path());
        assert_eq!(espelho.ler("/pos-venda/galerias"), None);

        espelho.guardar("/pos-venda/galerias", b"[1,2]", Some("application/json"));
        let lido = espelho.ler("/pos-venda/galerias").unwrap();
        assert_eq!(lido.bytes, b"[1,2]");
        assert_eq!(lido.tipo.as_deref(), Some("application/json"));
        assert!(lido.visto_em > 0);

        // Por cima: vale a última.
        espelho.guardar("/pos-venda/galerias", b"[3]", None);
        let lido = espelho.ler("/pos-venda/galerias").unwrap();
        assert_eq!(lido.bytes, b"[3]");
        assert_eq!(lido.tipo, None);

        // Outra rota é outro arquivo — reabrir a pasta lê o mesmo.
        let de_novo = EspelhoEmDisco::novo(pasta.path());
        assert_eq!(de_novo.ler("/pos-venda/galerias").unwrap().bytes, b"[3]");
        assert_eq!(de_novo.ler("/pos-venda/galerias/1"), None);
    }

    /// Muitas gravações da mesma rota ao mesmo tempo: nenhuma falha, e o que
    /// fica é uma delas inteira.
    #[test]
    fn gravacoes_ao_mesmo_tempo_nao_se_atropelam() {
        let pasta = tempfile::tempdir().unwrap();
        let espelho = std::sync::Arc::new(EspelhoEmDisco::novo(pasta.path()));
        let fios: Vec<_> = (0..16u8)
            .map(|n| {
                let espelho = espelho.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        espelho.guardar("/products/admin", &[n; 64], None);
                    }
                })
            })
            .collect();
        for fio in fios {
            fio.join().unwrap();
        }
        let lido = espelho.ler("/products/admin").unwrap();
        assert_eq!(lido.bytes.len(), 64);
        assert!(lido.bytes.iter().all(|b| *b == lido.bytes[0]));
        let sobras = std::fs::read_dir(pasta.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .to_string_lossy()
                    .ends_with(".gravando")
            })
            .count();
        assert_eq!(sobras, 0, "nenhum provisório fica para trás");
    }

    #[test]
    fn o_original_e_o_fluxo_nao_entram() {
        assert!(espelhavel("/pos-venda/galerias"));
        assert!(espelhavel("/pos-venda/fotos/a/miniatura"));
        assert!(espelhavel("/pos-venda/caixa?estudio_id=1&local=ESTUDIO"));
        assert!(!espelhavel("/pos-venda/fotos/a/original"));
        assert!(!espelhavel("/pos-venda/fotos/a/fonte-para-revelar"));
        assert!(!espelhavel("/auth/me"));
    }

    #[test]
    fn a_rede_lembra_desde_quando_caiu() {
        let rede = EstadoDaRede::default();
        assert!(rede.com_rede());
        rede.marcar_sem_rede();
        let desde = rede.sem_rede_desde().unwrap();
        // Cair de novo não muda o "desde".
        rede.marcar_sem_rede();
        assert_eq!(rede.sem_rede_desde(), Some(desde));
        rede.marcar_com_rede();
        assert!(rede.com_rede());
    }
}
