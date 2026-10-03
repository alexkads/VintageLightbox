//! 🔊 As portas do som: o alto-falante (som e voz) e a janela do sistema que
//! escolhe um arquivo de som.
//!
//! 🔑 **Nenhuma devolve `Result` para a tela.** Som é acessório: um balcão sem
//! placa de áudio, ou sem voz instalada, segue trabalhando mudo, e a falha vai
//! uma vez para a telemetria.
//!
//! O alto-falante é **uma thread só, com fila**: o som toca e a fala vem
//! depois dele, uma fala por vez — dois avisos seguidos não falam por cima um
//! do outro.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::embutidos::Embutido;
use super::voz::{self, Sistema};

/// O que tocar.
#[derive(Debug, Clone, PartialEq)]
pub enum Som {
    Embutido(Embutido),
    /// Um arquivo da pasta dos sons próprios.
    Proprio(PathBuf),
}

/// O que falar, e com que voz.
#[derive(Debug, Clone, PartialEq)]
pub struct Fala {
    pub texto: String,
    /// `None` é a primeira voz em português do sistema.
    pub voz: Option<String>,
    pub ritmo: f32,
}

/// Um aviso para o alto-falante: o som, depois a fala.
#[derive(Debug, Clone, PartialEq)]
pub struct Pedido {
    pub som: Option<Som>,
    pub fala: Option<Fala>,
    /// 0,0 a 1,0.
    pub volume: f32,
}

/// Uma voz do sistema. `id` é o que vai para o programa que fala.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voz {
    pub id: String,
    pub nome: String,
    pub idioma: String,
}

pub trait AltoFalante: Send + Sync + 'static {
    /// Põe o pedido na fila e devolve na hora.
    fn tocar(&self, pedido: Pedido);
    /// As vozes do sistema — vazia enquanto a leitura não terminou, ou quando
    /// não há voz.
    fn vozes(&self) -> Vec<Voz>;
    /// Se este computador sabe falar.
    fn fala(&self) -> bool;
}

/// A janela do sistema para escolher um arquivo de som. Desistir é `None`.
pub trait EscolhaDeSom: Send + Sync + 'static {
    fn escolher(&self, canal: Sender<Option<PathBuf>>);
}

/// As duas portas juntas — o que a montagem entrega ao app.
#[derive(Clone)]
pub struct PortasDoSom {
    pub alto_falante: Arc<dyn AltoFalante>,
    pub escolha: Arc<dyn EscolhaDeSom>,
}

/// Um aviso não toca mais que isto: um som próprio comprido (uma música
/// escolhida por engano) seguraria a fila inteira.
const MAXIMO_DO_SOM: Duration = Duration::from_secs(10);

/// Se o arquivo é um som que o app sabe tocar — decodificando de verdade, e
/// não pela extensão.
pub fn decodifica(caminho: &Path) -> Result<(), String> {
    let arquivo = std::fs::File::open(caminho).map_err(|erro| erro.to_string())?;
    let leitor = std::io::BufReader::new(arquivo);
    let mut decodificador = rodio::Decoder::new(leitor)
        .map_err(|_| "o formato não é WAV, MP3, OGG nem FLAC".to_string())?;
    // Um cabeçalho válido sobre dados vazios também "decodifica": é preciso
    // tirar ao menos uma amostra.
    decodificador
        .next()
        .map(|_| ())
        .ok_or_else(|| "o arquivo não tem som".to_string())
}

/// O alto-falante de verdade: `rodio` para o som, o programa do sistema para
/// a voz.
pub struct AltoFalanteDoSistema {
    fila: Mutex<Sender<Pedido>>,
    vozes: Arc<Mutex<Vec<Voz>>>,
    sistema: Option<Sistema>,
}

impl AltoFalanteDoSistema {
    pub fn novo() -> Self {
        let sistema = Sistema::deste();
        let vozes = Arc::new(Mutex::new(Vec::new()));
        // As vozes se leem fora da fila: o primeiro aviso não espera o
        // `say -v ?` terminar.
        if let Some(sistema) = sistema {
            let vozes = vozes.clone();
            let _ = std::thread::Builder::new()
                .name("vozes-do-sistema".into())
                .spawn(move || {
                    *vozes.lock().unwrap() = voz::listar(sistema);
                });
        }
        let (fila, pedidos) = channel::<Pedido>();
        let vozes_da_fila = vozes.clone();
        let _ = std::thread::Builder::new()
            .name("alto-falante".into())
            .spawn(move || {
                let mut saida: Option<rodio::MixerDeviceSink> = None;
                let mut sem_audio_avisado = false;
                let mut sem_voz_avisada = false;
                for pedido in pedidos {
                    if let Some(som) = &pedido.som {
                        if saida.is_none() {
                            match rodio::DeviceSinkBuilder::open_default_sink() {
                                Ok(mut aberta) => {
                                    aberta.log_on_drop(false);
                                    saida = Some(aberta);
                                }
                                Err(erro) => {
                                    if !sem_audio_avisado {
                                        crate::telemetria::avisar!(
                                            "⚠️ [Sons] sem saída de áudio: {erro}"
                                        );
                                        sem_audio_avisado = true;
                                    }
                                }
                            }
                        }
                        if let Some(saida) = &saida {
                            if let Err(erro) = tocar_ate_o_fim(saida, som, pedido.volume) {
                                crate::telemetria::avisar!("⚠️ [Sons] não tocou {som:?}: {erro}");
                            }
                        }
                    }
                    if let Some(fala) = &pedido.fala {
                        let Some(sistema) = sistema else {
                            if !sem_voz_avisada {
                                crate::telemetria::avisar!(
                                    "⚠️ [Sons] este computador não tem voz instalada"
                                );
                                sem_voz_avisada = true;
                            }
                            continue;
                        };
                        let mut fala = fala.clone();
                        if fala.voz.is_none() {
                            fala.voz = voz::padrao(&vozes_da_fila.lock().unwrap());
                        }
                        if let Err(erro) =
                            voz::comando_de_fala(sistema, &fala, pedido.volume).rodar()
                        {
                            crate::telemetria::avisar!("⚠️ [Sons] a voz não falou: {erro}");
                        }
                    }
                }
            });
        Self {
            fila: Mutex::new(fila),
            vozes,
            sistema,
        }
    }
}

/// Toca e espera acabar — a fala, se houver, vem depois.
fn tocar_ate_o_fim(saida: &rodio::MixerDeviceSink, som: &Som, volume: f32) -> Result<(), String> {
    let tocador = rodio::Player::connect_new(saida.mixer());
    match som {
        Som::Embutido(embutido) => {
            let amostras = embutido.amostras();
            tocador.append(rodio::buffer::SamplesBuffer::new(
                std::num::NonZero::new(1).unwrap(),
                std::num::NonZero::new(super::embutidos::TAXA).unwrap(),
                amostras,
            ));
        }
        Som::Proprio(caminho) => {
            let arquivo = std::fs::File::open(caminho).map_err(|erro| erro.to_string())?;
            let decodificado = rodio::Decoder::new(std::io::BufReader::new(arquivo))
                .map_err(|erro| erro.to_string())?;
            tocador.append(decodificado);
        }
    }
    tocador.set_volume(volume.clamp(0.0, 1.0));
    let inicio = Instant::now();
    while !tocador.empty() && inicio.elapsed() < MAXIMO_DO_SOM {
        std::thread::sleep(Duration::from_millis(30));
    }
    tocador.stop();
    crate::telemetria::avisar!("🔊 [Sons] tocou {som:?} a {:.0}%", volume * 100.0);
    Ok(())
}

impl AltoFalante for AltoFalanteDoSistema {
    fn tocar(&self, pedido: Pedido) {
        if let Some(fala) = &pedido.fala {
            crate::telemetria::avisar!("🗣️ [Sons] falar: {}", fala.texto);
        }
        let _ = self.fila.lock().unwrap().send(pedido);
    }

    fn vozes(&self) -> Vec<Voz> {
        self.vozes.lock().unwrap().clone()
    }

    fn fala(&self) -> bool {
        self.sistema.is_some()
    }
}

/// A janela do sistema, pelo `rfd`, no tokio do app — como a do backup.
pub struct EscolhaNativa {
    tokio: tokio::runtime::Handle,
}

impl EscolhaNativa {
    pub fn nova(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }
}

impl EscolhaDeSom for EscolhaNativa {
    fn escolher(&self, canal: Sender<Option<PathBuf>>) {
        self.tokio.spawn(async move {
            let escolhido = rfd::AsyncFileDialog::new()
                .set_title("Escolher um som")
                .add_filter("Som", &["wav", "mp3", "ogg", "oga", "flac"])
                .pick_file()
                .await
                .map(|arquivo| arquivo.path().to_path_buf());
            let _ = canal.send(escolhido);
        });
    }
}

#[cfg(test)]
pub mod mentira {
    use super::*;

    /// Guarda o que pediram para tocar, sem som nenhum.
    #[derive(Default)]
    pub struct AltoFalanteDeMentira {
        pub pedidos: Mutex<Vec<Pedido>>,
        pub vozes: Vec<Voz>,
        pub sem_voz: bool,
    }

    impl AltoFalanteDeMentira {
        pub fn pedidos(&self) -> Vec<Pedido> {
            self.pedidos.lock().unwrap().clone()
        }
    }

    impl AltoFalante for AltoFalanteDeMentira {
        fn tocar(&self, pedido: Pedido) {
            self.pedidos.lock().unwrap().push(pedido);
        }
        fn vozes(&self) -> Vec<Voz> {
            self.vozes.clone()
        }
        fn fala(&self) -> bool {
            !self.sem_voz
        }
    }

    /// Responde o arquivo que lhe mandarem responder.
    #[derive(Default)]
    pub struct EscolhaDeMentira {
        pub resposta: Mutex<Option<PathBuf>>,
    }

    impl EscolhaDeSom for EscolhaDeMentira {
        fn escolher(&self, canal: Sender<Option<PathBuf>>) {
            let _ = canal.send(self.resposta.lock().unwrap().clone());
        }
    }

    impl PortasDoSom {
        /// As portas de mentira, com o alto-falante à mão para afirmar.
        pub fn de_mentira() -> (Self, Arc<AltoFalanteDeMentira>) {
            let alto_falante = Arc::new(AltoFalanteDeMentira::default());
            (
                Self {
                    alto_falante: alto_falante.clone(),
                    escolha: Arc::new(EscolhaDeMentira::default()),
                },
                alto_falante,
            )
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um WAV de verdade, mono, 16 bits, com `amostras` amostras.
    pub fn wav(amostras: usize) -> Vec<u8> {
        let dados = (amostras * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + dados).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&16000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&dados.to_le_bytes());
        for i in 0..amostras {
            bytes.extend_from_slice(&(((i % 40) as i16 - 20) * 500).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn so_aceita_o_que_decodifica() {
        let pasta = tempfile::TempDir::new().unwrap();
        let bom = pasta.path().join("bom.wav");
        std::fs::write(&bom, wav(800)).unwrap();
        assert_eq!(decodifica(&bom), Ok(()));

        let texto = pasta.path().join("finge.mp3");
        std::fs::write(&texto, b"isto nao e som").unwrap();
        assert!(decodifica(&texto).is_err());

        let vazio = pasta.path().join("vazio.wav");
        std::fs::write(&vazio, wav(0)).unwrap();
        assert!(
            decodifica(&vazio).is_err(),
            "cabeçalho sem amostra não é som"
        );

        assert!(decodifica(&pasta.path().join("sumiu.wav")).is_err());
    }
}

#[cfg(test)]
pub use testes::wav as wav_de_teste;
