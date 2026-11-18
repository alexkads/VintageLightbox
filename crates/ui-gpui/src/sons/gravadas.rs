//! 🎙️ As vozes gravadas: uma frase por aviso, na voz `pm_santa` do Kokoro 82M,
//! gravadas uma vez e embutidas no app (241 KB em Ogg Vorbis mono a 24 kHz).
//!
//! Por que gravado, e não falado na hora: o Kokoro precisa de 329 MB de modelo
//! e 1,7 GB de memória para falar; gravado, nenhum balcão baixa nada nem
//! carrega modelo, e a frase soa igual no Mac, no Windows e no Linux — inclusive
//! o "servidor não está respondendo", que precisa tocar justamente quando a rede
//! falhou. Os textos estão em `assets/vozes/frases.tsv`; para regravar, o
//! roteiro está na issue do app (gravador com o `ravenrs-ai` do RavenRS,
//! qualidade Vorbis −0,1).
//!
//! O `afplay` do macOS toca Ogg Vorbis pela metade: confira pelo `rodio`, que é
//! o que o app usa.

macro_rules! gravadas {
    ($($nome:ident => $arquivo:literal),+ $(,)?) => {
        /// Uma frase gravada.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Gravada {
            $($nome),+
        }

        impl Gravada {
            pub const TODAS: &'static [Gravada] = &[$(Gravada::$nome),+];

            /// O arquivo, como está em `assets/vozes/`.
            pub fn arquivo(self) -> &'static str {
                match self {
                    $(Gravada::$nome => $arquivo),+
                }
            }

            /// Os bytes do Ogg Vorbis, embutidos no binário.
            pub fn bytes(self) -> &'static [u8] {
                match self {
                    $(Gravada::$nome => include_bytes!(concat!("../../assets/vozes/", $arquivo))),+
                }
            }
        }
    };
}

gravadas! {
    MensagemNoChatbot => "mensagem-no-chatbot.ogg",
    ClienteNoQr => "cliente-no-qr.ogg",
    NovoAgendamento => "novo-agendamento.ogg",
    AgendamentoCancelado => "agendamento-cancelado.ogg",
    Falha => "falha.ogg",
    FotosSalvas => "fotos-salvas.ogg",
    ExportacaoConcluida => "exportacao-concluida.ogg",
    NovaVersao => "nova-versao.ogg",
    CartaoEjetado => "cartao-ejetado.ogg",
    PediuAtendente => "pediu-atendente.ogg",
    AlertaFalha => "alerta-falha.ogg",
    AlertaMemoria => "alerta-memoria.ogg",
    AlertaNormal => "alerta-normal.ogg",
    ServidorOffline => "servidor-offline.ogg",
    SemInternet => "sem-internet.ogg",
    ConexaoVoltou => "conexao-voltou.ogg",
    EnvioConcluido => "envio-concluido.ogg",
}

#[cfg(test)]
mod testes {
    use super::*;
    use rodio::Source;

    /// Toda gravação abre pelo mesmo `rodio` que toca, e inteira: mais de um
    /// segundo e menos de dez (o teto de um aviso, `MAXIMO_DO_SOM`).
    #[test]
    fn toda_gravacao_abre_pelo_rodio_e_cabe_no_teto() {
        for gravada in Gravada::TODAS {
            let decodificado = rodio::Decoder::new(std::io::Cursor::new(gravada.bytes()))
                .unwrap_or_else(|e| panic!("{}: {e}", gravada.arquivo()));
            let taxa = decodificado.sample_rate().get() as f32;
            let segundos = decodificado.count() as f32 / taxa;
            assert!(
                (1.0..10.0).contains(&segundos),
                "{}: {segundos:.2} s",
                gravada.arquivo()
            );
        }
    }

    #[test]
    fn cada_gravacao_tem_a_sua_linha_no_roteiro() {
        let roteiro = include_str!("../../assets/vozes/frases.tsv");
        for gravada in Gravada::TODAS {
            let nome = gravada.arquivo().trim_end_matches(".ogg");
            assert!(
                roteiro.lines().any(|l| l.split('\t').next() == Some(nome)),
                "{nome} sem texto em frases.tsv"
            );
        }
    }
}
