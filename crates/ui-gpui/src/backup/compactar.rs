//! O zip de "baixar a pasta compactada".
//!
//! # Por que o zip é montado **aqui**, e não na API
//!
//! Pedido do dono (2026-09-19: *"baixar as pastas compactadas"*). Montá-lo no
//! servidor custaria três coisas: a máquina do Fly leria cada arquivo (um RAW
//! de 80 MB na memória de uma máquina pequena), pagaria a saída de uma pasta de
//! 2 GB — que pelo R2 é grátis, e é o que o próprio painel de governança
//! recomenda evitar — e seguraria a conexão aberta o download inteiro.
//!
//! Aqui o app busca cada arquivo **direto do R2** pela URL assinada e vai
//! escrevendo o zip no disco conforme eles chegam. É a mesma decisão do site,
//! que faz o mesmo com o `client-zip`.
//!
//! # Sem compressão, de propósito
//!
//! O método é *store*. JPEG, WebP e RAW já são dados comprimidos, e passar
//! deflate neles gasta CPU para ganhar 1–2%. O zip aqui serve para **juntar**.

use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Escreve o zip, um arquivo por vez.
///
/// 🔑 **Escreve direto no arquivo de destino**, e não num buffer: uma pasta de
/// 20 GB não cabe na memória, e guardar tudo para gravar no fim é a diferença
/// entre funcionar e derrubar o app na pasta grande.
pub struct Compactador<W: Write + Seek> {
    zip: ZipWriter<W>,
    opcoes: SimpleFileOptions,
}

impl Compactador<File> {
    /// Abre o zip no destino escolhido.
    pub fn no_arquivo(destino: &Path) -> std::io::Result<Self> {
        Ok(Self::novo(File::create(destino)?))
    }
}

impl<W: Write + Seek> Compactador<W> {
    pub fn novo(saida: W) -> Self {
        Self {
            zip: ZipWriter::new(saida),
            // `Stored`: ver o porquê no módulo.
            //
            // ⚠️ **`large_file(true)` sempre.** Sem ele, o `zip` escreve o
            // cabeçalho sem ZIP64 e **falha** no primeiro arquivo acima de
            // 4 GB — que num acervo de vídeo de ensaio existe. Ligar por
            // arquivo exigiria saber o tamanho antes, e o que se tem aqui é um
            // fluxo.
            opcoes: SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .large_file(true),
        }
    }

    /// Põe um arquivo no zip, no caminho que ele terá dentro dele.
    pub fn por(&mut self, caminho: &str, bytes: &[u8]) -> std::io::Result<()> {
        self.zip.start_file(caminho, self.opcoes)?;
        self.zip.write_all(bytes)?;
        Ok(())
    }

    /// Fecha o zip. **Sem isto o arquivo não abre**: o índice central só é
    /// escrito no fim.
    pub fn fechar(self) -> std::io::Result<W> {
        Ok(self.zip.finish()?)
    }
}

/// O que dá para ver na prévia, pela extensão.
///
/// Lista fechada e igual à do site (`baixar.ts`): o que aparece numa interface
/// tem de aparecer na outra, ou o mesmo acervo teria duas caras.
pub fn eh_visualizavel(nome: &str) -> bool {
    let extensao = nome
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extensao.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif" | "bmp"
    )
}

/// O nome do zip de uma pasta: `ensaio-silva.zip`. A raiz vira `acervo.zip`.
pub fn nome_do_zip(caminho: &str) -> String {
    let nome = caminho
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    if nome.is_empty() {
        "acervo.zip".to_string()
    } else {
        format!("{nome}.zip")
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn o_zip_guarda_os_caminhos_e_o_conteudo() {
        let mut compactador = Compactador::novo(Cursor::new(Vec::new()));
        compactador
            .por("dia-1/DSC_01.NEF", b"cru")
            .expect("entrada");
        compactador.por("capa.jpg", b"foto").expect("entrada");
        let bytes = compactador.fechar().expect("fechar").into_inner();

        let mut lido = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip legível");
        assert_eq!(lido.len(), 2);
        let nomes: Vec<String> = lido.file_names().map(str::to_string).collect();
        assert!(nomes.contains(&"dia-1/DSC_01.NEF".to_string()));

        let mut dentro = lido.by_name("capa.jpg").expect("a foto está lá");
        let mut conteudo = Vec::new();
        std::io::Read::read_to_end(&mut dentro, &mut conteudo).expect("ler");
        assert_eq!(conteudo, b"foto");
    }

    /// 🚨 *Store*, e não deflate: o zip junta, não encolhe.
    #[test]
    fn nada_e_comprimido() {
        let mut compactador = Compactador::novo(Cursor::new(Vec::new()));
        compactador.por("a.bin", &vec![0u8; 4096]).expect("entrada");
        let bytes = compactador.fechar().expect("fechar").into_inner();

        let mut lido = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip legível");
        let entrada = lido.index_for_name("a.bin").expect("entrada");
        let dados = lido.by_index_raw(entrada).expect("crua");
        assert_eq!(dados.compression(), zip::CompressionMethod::Stored);
        // 4096 zeros comprimidos por deflate dariam dezenas de bytes; aqui não.
        assert_eq!(dados.size(), 4096);
    }

    #[test]
    fn o_zip_leva_o_nome_da_pasta() {
        assert_eq!(nome_do_zip("2026/ensaio-silva"), "ensaio-silva.zip");
        assert_eq!(nome_do_zip("2026/ensaio-silva/"), "ensaio-silva.zip");
        assert_eq!(nome_do_zip(""), "acervo.zip");
    }
}
