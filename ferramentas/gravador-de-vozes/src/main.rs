//! Grava as frases de alerta da API do RecordarFotos com o Kokoro do RavenRS
//! (voz pm_santa). Uso: gravador-de-vozes <modelos> <saída> <frases.tsv>, ou
//! gravador-de-vozes <arquivo.ogg> para decodificá-lo em <arquivo.ogg>.ouvir.wav.
//! Quem chama é o `regravar.sh` (skill `vozes-gravadas`).

use std::num::{NonZeroU32, NonZeroU8};
use std::time::Instant;

use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

use ravenrs_ai::{model_by_id, ModelFolder, SpeechEngine, SpeechSettings, SpeechVoice, SPEECH_RATE};

/// As frases vêm de um arquivo: uma por linha, `nome<TAB>texto`.
fn frases(arquivo: &str) -> Vec<(String, String)> {
    std::fs::read_to_string(arquivo)
        .expect("arquivo de frases")
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(n, t)| (n.trim().to_string(), t.trim().to_string()))
        .collect()
}

fn wav(som: &[f32]) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + som.len() * 2);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + som.len() as u32 * 2).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16_u32.to_le_bytes());
    w.extend_from_slice(&1_u16.to_le_bytes());
    w.extend_from_slice(&1_u16.to_le_bytes());
    w.extend_from_slice(&SPEECH_RATE.to_le_bytes());
    w.extend_from_slice(&(SPEECH_RATE * 2).to_le_bytes());
    w.extend_from_slice(&2_u16.to_le_bytes());
    w.extend_from_slice(&16_u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(som.len() as u32 * 2).to_le_bytes());
    for a in som {
        w.extend_from_slice(&((a.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    w
}

/// Ogg Vorbis mono, na qualidade pedida (-0.2 a 1). A série do fluxo vem do
/// nome: a mesma frase gera o mesmo arquivo, byte a byte.
fn ogg(som: &[f32], nome: &str, qualidade: f32) -> Vec<u8> {
    let serie = nome.bytes().fold(0x5eed_u32, |h, b| h.rotate_left(5) ^ u32::from(b));
    let mut codificador = VorbisEncoderBuilder::new_with_serial(
        NonZeroU32::new(SPEECH_RATE).expect("taxa"),
        NonZeroU8::new(1).expect("canal"),
        Vec::new(),
        serie as i32,
    )
    .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
        target_quality: qualidade,
    })
    .build()
    .expect("codificador");
    for bloco in som.chunks(4096) {
        codificador.encode_audio_block([bloco]).expect("codificar");
    }
    codificador.finish().expect("fechar")
}

/// Quantas amostras o `.ogg` devolve ao ser lido de novo.
fn conferir(ogg: &[u8]) -> usize {
    let mut leitor = vorbis_rs::VorbisDecoder::new(std::io::Cursor::new(ogg.to_vec())).expect("ler ogg");
    let mut total = 0;
    while let Some(bloco) = leitor.decode_audio_block().expect("decodificar") {
        total += bloco.samples()[0].len();
    }
    total
}

fn main() {
    if let Some(arquivo) = std::env::args().nth(1).filter(|a| a.ends_with(".ogg")) {
        let bytes = std::fs::read(&arquivo).expect("arquivo");
        let mut leitor = vorbis_rs::VorbisDecoder::new(std::io::Cursor::new(bytes)).expect("ler ogg");
        let mut som = Vec::new();
        while let Some(bloco) = leitor.decode_audio_block().expect("decodificar") {
            som.extend_from_slice(bloco.samples()[0]);
        }
        // O som como o app vai tocar, já com a perda da compressão.
        std::fs::write(format!("{arquivo}.ouvir.wav"), wav(&som)).expect("gravar");
        println!("{arquivo}: {} amostras = {:.2} s", som.len(), som.len() as f32 / SPEECH_RATE as f32);
        return;
    }
    let mut args = std::env::args().skip(1);
    let modelos = args.next().expect("pasta dos modelos");
    let saida = std::path::PathBuf::from(args.next().expect("pasta de saída"));
    let lista = frases(&args.next().expect("arquivo de frases"));
    std::fs::create_dir_all(&saida).expect("pasta de saída");
    let modelo = model_by_id("kokoro-82m").expect("kokoro no catálogo");
    let t = Instant::now();
    let mut motor = SpeechEngine::load(&ModelFolder::new(modelos), &modelo).expect("o modelo");
    eprintln!("modelo carregado em {:.1} s", t.elapsed().as_secs_f32());
    let ajustes = SpeechSettings {
        voice: SpeechVoice::Male,
        speaker: Some("pm_santa".to_owned()),
        ..SpeechSettings::default()
    };
    // Como a locução dos treinamentos: as peças que o motor sugere, com 0,25 s
    // de silêncio entre elas.
    let silencio = vec![0.0_f32; SPEECH_RATE as usize / 4];
    for (nome, texto) in &lista {
        let t = Instant::now();
        let mut som = Vec::new();
        for (i, peca) in motor.pieces(texto).iter().enumerate() {
            if i > 0 {
                som.extend_from_slice(&silencio);
            }
            som.extend(motor.speak(peca, &ajustes, &mut |_| true).expect("a fala"));
        }
        let pico = som.iter().fold(0.0_f32, |p, a| p.max(a.abs()));
        let destino = saida.join(format!("{nome}.wav"));
        std::fs::write(&destino, wav(&som)).expect("gravar");
        let qualidade: f32 = std::env::var("QUALIDADE").ok().and_then(|q| q.parse().ok()).unwrap_or(0.0);
        let comprimido = ogg(&som, nome, qualidade);
        std::fs::write(destino.with_extension("ogg"), &comprimido).expect("gravar ogg");
        eprintln!("   ogg {} KB (q {qualidade})", comprimido.len() / 1024);
        eprintln!(
            "{nome}: {:.1} s de fala em {:.1} s (pico {pico:.2}) → {}",
            som.len() as f32 / SPEECH_RATE as f32,
            t.elapsed().as_secs_f32(),
            destino.display()
        );
    }
}
