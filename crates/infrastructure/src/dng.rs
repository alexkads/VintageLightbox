//! Por que um DNG não abre — quando a resposta é "não é o arquivo".
//!
//! ## 🚨 O caso que este módulo existe para nomear
//!
//! Um `.dng` do acervo do dono falhou na importação com
//! `LibRaw failed to open: FileUnsupported` — uma frase que aponta para o
//! arquivo. Medido em 17/ago/2026, o arquivo está perfeito: é um DNG 1.4 de uma
//! Nikon D7200, 2560×1707, gravado com **compressão com perdas**
//! (`Compression = 34892`).
//!
//! Nenhum dos dois decodificadores do projeto o abre, e os dois dizem por quê:
//!
//! | | resposta |
//! |---|---|
//! | `rsraw` (LibRaw) | `FileUnsupported` |
//! | `rawloader` | *"Don't know how to read DNGs with compression 34892"* |
//! | `image` | nem tenta — não reconhece a extensão |
//!
//! 🔑 **A causa é de compilação, não de formato.** O `build.rs` do `rsraw-sys`
//! compila a LibRaw **sem `USE_JPEG`**, e o caminho de DNG com perdas é
//! justamente o que precisa de libjpeg: os blocos de imagem lá dentro são JPEG
//! de linha de base. A LibRaw sabe ler isto — a que está no binário, não.
//!
//! ## Por que este módulo só **nomeia** o problema, em vez de resolvê-lo
//!
//! Três saídas foram pesadas:
//!
//! 1. **Decodificar aqui.** Os blocos são JPEG e a imagem já vem demosaicada
//!    (`PhotometricInterpretation = LinearRaw`), então extraí-los é viável — mas
//!    o resultado é linear de cena, e transformá-lo em cor exige a matriz de cor,
//!    o `AsShotNeutral` e a `ForwardMatrix` do DNG. Isso é escrever um conversor
//!    de RAW, e o resultado errado seria **cor plausível e errada** — o pior
//!    desfecho possível num programa de revelação.
//! 2. **Cair na prévia embutida.** Este arquivo carrega **uma só**, de 256×171
//!    (medido). Importar isso como se fosse a foto trocaria uma falha visível por
//!    uma invisível: o catálogo teria uma entrada de aparência normal, e revelar
//!    ou exportar devolveria 256 px sem ninguém ser avisado.
//! 3. **Dizer o que é.** É o que está aqui. `FileUnsupported` faz quem importa
//!    procurar defeito no próprio arquivo; a mensagem certa diz que o arquivo
//!    está bom, o que falta no app, e o que dá para fazer hoje.
//!
//! ## ✅ E em 18/ago apareceu uma quarta saída, medida
//!
//! A LibRaw **do sistema** já está instalada nesta máquina (Homebrew) e é
//! compilada **com** libjpeg:
//!
//! ```text
//! otool -L /opt/homebrew/opt/libraw/lib/libraw.dylib | grep jpeg
//!     /opt/homebrew/opt/jpeg-turbo/lib/libjpeg.8.dylib
//! ```
//!
//! E ela abre o arquivo: `dcraw_emu -w -T -Z -` devolve 13 MB de TIFF pelo
//! stdout, sem escrever nada na pasta de quem importa.
//!
//! 🔑 **Isso troca "vendorizar a LibRaw no repositório" por "usar a que já
//! existe"** — e é o que [`decodificar_com_a_libraw_do_sistema`] faz, como
//! **reserva**: só quando o `rsraw` recusa, e só para o caso com perdas.
//!
//! ⚠️ **A reserva não substitui o caminho normal, e é de propósito.** São duas
//! invocações diferentes da LibRaw, com padrões diferentes de revelação: usá-la
//! para todo RAW mudaria a cor de todos os arquivos que hoje abrem — em silêncio.
//! Ela entra onde a alternativa é não abrir.

/// A compressão com perdas do DNG 1.4, na tabela do TIFF.
const LOSSY_JPEG: u16 = 34892;

/// Tags do TIFF que interessam aqui.
const COMPRESSION: u16 = 0x0103;
const SUB_IFDS: u16 = 0x014a;

/// Um leitor de TIFF do tamanho da pergunta: só o suficiente para achar a
/// compressão do bloco de imagem principal.
struct Tiff<'a> {
    bytes: &'a [u8],
    little: bool,
}

impl<'a> Tiff<'a> {
    fn novo(bytes: &'a [u8]) -> Option<Self> {
        let little = match bytes.get(..4)? {
            [0x49, 0x49, 0x2a, 0x00] => true,
            [0x4d, 0x4d, 0x00, 0x2a] => false,
            _ => return None,
        };
        Some(Self { bytes, little })
    }

    fn u16(&self, em: usize) -> Option<u16> {
        let b: [u8; 2] = self.bytes.get(em..em + 2)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }

    fn u32(&self, em: usize) -> Option<u32> {
        let b: [u8; 4] = self.bytes.get(em..em + 4)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }

    /// As compressões declaradas neste IFD e nos SubIFDs dele.
    ///
    /// ⚠️ **Não desce recursivamente sem limite.** Um TIFF pode apontar um SubIFD
    /// para si mesmo, e um leitor ingênuo entra em laço — dentro de uma
    /// importação de 2.000 arquivos, isso é o app parecendo travado.
    fn compressoes(&self, ifd: usize, fundo: u8, saida: &mut Vec<u16>) {
        if fundo > 4 {
            return;
        }
        let Some(quantas) = self.u16(ifd) else {
            return;
        };

        for i in 0..quantas as usize {
            let entrada = ifd + 2 + i * 12;
            let (Some(tag), Some(tipo), Some(conta)) = (
                self.u16(entrada),
                self.u16(entrada + 2),
                self.u32(entrada + 4),
            ) else {
                return;
            };

            match tag {
                COMPRESSION if tipo == 3 => {
                    if let Some(valor) = self.u16(entrada + 8) {
                        saida.push(valor);
                    }
                }
                SUB_IFDS => {
                    // Um só cabe no campo; vários moram fora, e o campo vira
                    // ponteiro para a lista.
                    let offsets: Vec<u32> = if conta == 1 {
                        self.u32(entrada + 8).into_iter().collect()
                    } else {
                        let base = match self.u32(entrada + 8) {
                            Some(b) => b as usize,
                            None => continue,
                        };
                        (0..conta as usize)
                            .filter_map(|n| self.u32(base + n * 4))
                            .collect()
                    };
                    for off in offsets {
                        self.compressoes(off as usize, fundo + 1, saida);
                    }
                }
                _ => {}
            }
        }
    }
}

/// O arquivo é um DNG (ou TIFF) com compressão **com perdas**?
///
/// 🔑 **Responde pelo conteúdo, e não pela extensão.** Um `.dng` sem perdas abre
/// normalmente, e um arquivo renomeado não vira DNG — a pergunta é sobre o que
/// está gravado dentro.
pub fn tem_compressao_com_perdas(bytes: &[u8]) -> bool {
    let Some(tiff) = Tiff::novo(bytes) else {
        return false;
    };
    let Some(primeiro) = tiff.u32(4) else {
        return false;
    };

    let mut compressoes = Vec::new();
    tiff.compressoes(primeiro as usize, 0, &mut compressoes);
    compressoes.contains(&LOSSY_JPEG)
}

/// A explicação, quando um RAW não abre.
///
/// ⚠️ **Ela vale mais que a causa técnica.** `FileUnsupported` manda quem importa
/// procurar defeito no próprio arquivo — e o arquivo está bom. A mensagem diz o
/// que é, o que falta no app, e o que dá para fazer hoje.
pub fn explicar_falha(bytes: &[u8], erro_original: &str) -> String {
    if tem_compressao_com_perdas(bytes) {
        return "DNG com compressão *lossy* (DNG 1.4). O arquivo está íntegro — \
                falta suporte no app: a LibRaw embutida aqui foi compilada sem \
                libjpeg, que é o que esse formato exige. Por enquanto, importe o \
                RAW original da câmera, ou converta este DNG sem compressão com \
                perdas."
            .to_string();
    }
    erro_original.to_string()
}

/// Onde procurar a LibRaw do sistema.
///
/// ⚠️ **Caminhos conhecidos primeiro, e o `PATH` depois.** O Homebrew instala em
/// `/opt/homebrew` (Apple Silicon) ou `/usr/local` (Intel), e um app aberto pelo
/// Finder herda o `PATH` mínimo do `launchd` — não o do shell. Procurar só no
/// `PATH` faria a reserva funcionar no terminal e não no app, que é a pior forma
/// de uma funcionalidade existir.
///
/// 🚨 **E isto não tem teste que o prove.** Tentei quebrar de propósito, tirando
/// os dois caminhos absolutos, e o teste continuou passando — porque nesta
/// máquina o `dcraw_emu` **está** no `PATH` do shell, então a terceira entrada
/// cobre. A afirmação sobre o Finder vem do comportamento conhecido do `launchd`,
/// e não de medida feita aqui; está escrita assim para quem vier depois não
/// confundir as duas coisas.
const ONDE_PROCURAR: [&str; 3] = [
    "/opt/homebrew/opt/libraw/bin/dcraw_emu",
    "/usr/local/opt/libraw/bin/dcraw_emu",
    "dcraw_emu",
];

fn caminho_do_decodificador() -> Option<std::path::PathBuf> {
    for candidato in ONDE_PROCURAR {
        let caminho = std::path::PathBuf::from(candidato);
        if caminho.is_absolute() {
            if caminho.exists() {
                return Some(caminho);
            }
        } else if std::process::Command::new(&caminho)
            .arg("-h")
            .output()
            .is_ok()
        {
            return Some(caminho);
        }
    }
    None
}

/// Decodifica pela LibRaw **do sistema**, quando ela existe.
///
/// 🔑 **É reserva, e só para o que o caminho normal recusa.** Ver o topo do
/// arquivo: são duas invocações diferentes da LibRaw, e usar esta para todo RAW
/// mudaria a cor de todos os arquivos que já abrem.
///
/// ⚠️ **A saída vai para o stdout** (`-Z -`), e não para um arquivo ao lado do
/// original. O `simple_dcraw` grava `<nome>.tiff` na pasta de origem — e escrever
/// no cartão de alguém durante uma importação é o tipo de efeito que ninguém
/// pede e que ninguém desfaz.
pub fn decodificar_com_a_libraw_do_sistema(caminho: &str) -> Result<image::DynamicImage, String> {
    let programa = caminho_do_decodificador()
        .ok_or_else(|| "a LibRaw do sistema não está instalada".to_string())?;

    let saida = std::process::Command::new(&programa)
        // `-w` usa o balanço de branco da câmera, que é o que o caminho normal
        // (`rsraw::process`) também faz — sem ele a foto sai esverdeada.
        .args(["-w", "-T", "-Z", "-"])
        .arg(caminho)
        .output()
        .map_err(|e| format!("não foi possível executar a LibRaw do sistema: {e}"))?;

    if !saida.status.success() {
        let erro = String::from_utf8_lossy(&saida.stderr);
        return Err(format!(
            "a LibRaw do sistema recusou o arquivo: {}",
            erro.trim()
        ));
    }
    if saida.stdout.is_empty() {
        return Err("a LibRaw do sistema não devolveu imagem".to_string());
    }

    image::load_from_memory_with_format(&saida.stdout, image::ImageFormat::Tiff)
        .map_err(|e| format!("o TIFF devolvido pela LibRaw do sistema não abriu: {e}"))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Monta um TIFF pequeno: IFD0 com uma compressão e, opcionalmente, um
    /// SubIFD com outra.
    fn tiff(compressao_do_ifd0: u16, compressao_do_sub: Option<u16>) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&[0x49, 0x49, 0x2a, 0x00]); // II*\0
        b.extend_from_slice(&8u32.to_le_bytes()); // IFD0 em 8

        let entradas: u16 = if compressao_do_sub.is_some() { 2 } else { 1 };
        b.extend_from_slice(&entradas.to_le_bytes());

        // Compression (SHORT)
        b.extend_from_slice(&COMPRESSION.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&compressao_do_ifd0.to_le_bytes());
        b.extend_from_slice(&[0, 0]);

        if let Some(compressao_do_sub) = compressao_do_sub {
            // SubIFDs (LONG, 1) — o offset é preenchido depois.
            b.extend_from_slice(&SUB_IFDS.to_le_bytes());
            b.extend_from_slice(&4u16.to_le_bytes());
            b.extend_from_slice(&1u32.to_le_bytes());
            let marca = b.len();
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes()); // próximo IFD: nenhum

            let onde = b.len() as u32;
            b[marca..marca + 4].copy_from_slice(&onde.to_le_bytes());

            b.extend_from_slice(&1u16.to_le_bytes()); // uma entrada
            b.extend_from_slice(&COMPRESSION.to_le_bytes());
            b.extend_from_slice(&3u16.to_le_bytes());
            b.extend_from_slice(&1u32.to_le_bytes());
            b.extend_from_slice(&compressao_do_sub.to_le_bytes());
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(&0u32.to_le_bytes());
        } else {
            b.extend_from_slice(&0u32.to_le_bytes());
        }
        b
    }

    /// 🚨 **A compressão com perdas mora no SubIFD, e não no IFD0.**
    ///
    /// É a forma do arquivo de verdade: o IFD0 guarda a prévia (JPEG comum,
    /// compressão 7) e o bloco grande fica num SubIFD. Um leitor que olhasse só
    /// o IFD0 diria "compressão 7, tudo bem" e a explicação nunca apareceria.
    #[test]
    fn acha_a_compressao_com_perdas_no_subifd() {
        let arquivo = tiff(7, Some(LOSSY_JPEG));
        assert!(tem_compressao_com_perdas(&arquivo));
    }

    /// Um DNG comum não é acusado.
    #[test]
    fn dng_sem_perdas_nao_e_acusado() {
        assert!(!tem_compressao_com_perdas(&tiff(7, Some(1))));
        assert!(!tem_compressao_com_perdas(&tiff(1, None)));
    }

    /// ⚠️ Arquivo que não é TIFF não pode derrubar nem mentir.
    #[test]
    fn arquivo_que_nao_e_tiff_responde_nao() {
        assert!(!tem_compressao_com_perdas(b"nao sou um tiff"));
        assert!(!tem_compressao_com_perdas(&[]));
        assert!(!tem_compressao_com_perdas(&[0x49, 0x49]));
    }

    /// 🚨 **Um SubIFD que aponta para si mesmo não pode travar o app.**
    ///
    /// Dentro de uma importação de 2.000 arquivos, um laço aqui é o app parecendo
    /// congelado — sem erro, sem log, e sem relação óbvia com o arquivo que o
    /// causou.
    #[test]
    fn subifd_circular_nao_trava() {
        let mut b = Vec::new();
        b.extend_from_slice(&[0x49, 0x49, 0x2a, 0x00]);
        b.extend_from_slice(&8u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&SUB_IFDS.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&8u32.to_le_bytes()); // aponta para o próprio IFD0
        b.extend_from_slice(&0u32.to_le_bytes());

        assert!(!tem_compressao_com_perdas(&b));
    }

    /// A explicação troca a causa técnica pelo que dá para fazer.
    #[test]
    fn a_explicacao_diz_que_o_arquivo_esta_bom() {
        let com_perdas = tiff(7, Some(LOSSY_JPEG));
        let texto = explicar_falha(&com_perdas, "LibRaw failed to open: FileUnsupported");

        assert!(texto.contains("lossy"));
        assert!(
            texto.contains("íntegro"),
            "quem importa precisa saber que o arquivo não é o problema"
        );
        assert!(
            texto.contains("RAW original") || texto.contains("converta"),
            "uma explicação sem saída é só uma recusa mais longa"
        );
    }

    /// ⚠️ Para qualquer outra falha, a causa original passa intacta.
    #[test]
    fn outra_falha_mantem_a_mensagem_original() {
        let comum = tiff(7, Some(1));
        assert_eq!(
            explicar_falha(&comum, "arquivo corrompido"),
            "arquivo corrompido"
        );
    }
}
