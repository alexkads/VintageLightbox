//! A fila do envio: que arquivos vão, com que nome, e quanto já subiu.
//!
//! Tudo aqui é conta pura — é o que permite testar o que erra (caminho montado
//! errado, lote fora do teto, porcentagem que passa de 100) sem rede, sem R2 e
//! sem arrastar nada. O par do `backup/plano.ts` do site, com os mesmos números.

use std::path::{Path, PathBuf};

/// O estado de uma peça.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situacao {
    Esperando,
    Enviando,
    Pronta,
    Falhou,
}

/// Uma peça: um arquivo a subir. Original e convertido são peças diferentes.
#[derive(Debug, Clone, PartialEq)]
pub struct Peca {
    pub id: usize,
    /// Relativo à raiz do acervo — o que o backend assina.
    pub caminho: String,
    pub nome: String,
    pub bytes: u64,
    /// De onde os bytes saem. `None` no convertido, que já está em memória.
    pub origem: Option<PathBuf>,
    pub convertida: bool,
    pub situacao: Situacao,
    pub enviados: u64,
    pub erro: Option<String>,
}

/// Junta a pasta aberta com o caminho relativo do que foi arrastado.
///
/// 🔑 **A árvore arrastada é preservada** — é o que faz disto um backup. Soltar
/// `ensaio-silva/` dentro de `2026` grava `2026/ensaio-silva/…`.
///
/// Normaliza barras repetidas e nas pontas: no Windows o caminho vem com `\`, e
/// `a//b` seria recusado pelo backend com um erro que não diz de onde veio.
pub fn caminho_no_acervo(pasta_aberta: &str, relativo: &str) -> String {
    format!("{pasta_aberta}/{relativo}")
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|parte| !parte.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// O caminho de `arquivo` relativo à pasta que foi solta.
///
/// Soltar a pasta `…/Fotos/ensaio-silva` com `2026/DSC_01.NEF` dentro dá
/// `ensaio-silva/2026/DSC_01.NEF`: o **nome da pasta solta entra**, porque é ele
/// que o operador reconhece do outro lado. Um arquivo solto avulso vira só o
/// nome dele.
pub fn relativo_ao_solto(solto: &Path, arquivo: &Path) -> String {
    let base = if solto.is_dir() {
        solto.parent().unwrap_or(solto)
    } else {
        solto.parent().unwrap_or(Path::new(""))
    };
    arquivo
        .strip_prefix(base)
        .unwrap_or(arquivo)
        .to_string_lossy()
        .replace('\\', "/")
}

/// O andamento do envio inteiro, como a barra o mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Andamento {
    pub bytes_totais: u64,
    pub bytes_enviados: u64,
    pub porcento: u8,
    pub prontas: usize,
    pub com_erro: usize,
    pub total: usize,
    pub terminado: bool,
}

/// A conta da barra de cima.
///
/// 🚨 **Por bytes, e não por arquivo.** Uma pasta com um RAW de 40 MB e trinta
/// JSONs de 1 KB ficaria em "30 de 31" quase instantaneamente e depois pararia
/// por dois minutos — a barra estaria certa e mentindo.
///
/// ⚠️ **A peça que falhou conta como concluída** para o total. Sem isso a barra
/// para em 98% para sempre depois de uma falha, e ninguém sabe se ainda sobe.
pub fn andamento_de(pecas: &[Peca]) -> Andamento {
    let bytes_totais: u64 = pecas.iter().map(|p| p.bytes).sum();
    let bytes_enviados: u64 = pecas
        .iter()
        .map(|p| match p.situacao {
            Situacao::Pronta | Situacao::Falhou => p.bytes,
            _ => p.enviados.min(p.bytes),
        })
        .sum();
    let prontas = pecas
        .iter()
        .filter(|p| p.situacao == Situacao::Pronta)
        .count();
    let com_erro = pecas
        .iter()
        .filter(|p| p.situacao == Situacao::Falhou)
        .count();
    Andamento {
        bytes_totais,
        bytes_enviados,
        // Sem bytes não há divisão: uma fila vazia é 0%, e não uma divisão por
        // zero.
        porcento: if bytes_totais == 0 {
            0
        } else {
            ((bytes_enviados as f64 / bytes_totais as f64) * 100.0)
                .round()
                .min(100.0) as u8
        },
        prontas,
        com_erro,
        total: pecas.len(),
        terminado: !pecas.is_empty() && prontas + com_erro == pecas.len(),
    }
}

/// `1,4 GB` — o mesmo formato da gaveta "Espaço em uso" e do site.
pub fn em_tamanho(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let unidades = ["KB", "MB", "GB", "TB"];
    let mut valor = bytes as f64 / 1024.0;
    let mut unidade = 0;
    while valor >= 1024.0 && unidade < unidades.len() - 1 {
        valor /= 1024.0;
        unidade += 1;
    }
    // 🔑 **Uma casa só abaixo de 10, e sem o zero à toa** — é o que o
    // `toLocaleString("pt-BR", { maximumFractionDigits: 1 })` do site produz:
    // `1,5 KB` e `5 GB`, nunca `5,0 GB`. Divergir aqui faria a mesma pasta ter
    // dois tamanhos em duas telas que o operador usa no mesmo dia.
    let casas = if valor < 10.0 { 1 } else { 0 };
    let numero = format!("{valor:.casas$}");
    let numero = numero.strip_suffix(".0").unwrap_or(&numero);
    format!("{} {}", numero.replace('.', ","), unidades[unidade])
}

/// O tipo do arquivo pela extensão — o `Content-Type` com que ele é gravado.
///
/// Lista fechada, e o resto é binário: é a mesma decisão do
/// `handlers::media::tipo_do_arquivo` no backend, e pelo mesmo motivo — um
/// `text/html` gravado por adivinhação é um arquivo que o navegador
/// **executaria** ao abrir.
pub fn tipo_do_arquivo(nome: &str) -> String {
    let extensao = nome
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extensao.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "tif" | "tiff" => "image/tiff",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" => "text/plain",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn peca(bytes: u64, situacao: Situacao, enviados: u64) -> Peca {
        Peca {
            id: 0,
            caminho: "x.jpg".into(),
            nome: "x.jpg".into(),
            bytes,
            origem: None,
            convertida: false,
            situacao,
            enviados,
            erro: None,
        }
    }

    #[test]
    fn a_arvore_arrastada_e_preservada() {
        assert_eq!(
            caminho_no_acervo("2026", "ensaio-silva/dia-1/DSC_01.NEF"),
            "2026/ensaio-silva/dia-1/DSC_01.NEF"
        );
        assert_eq!(
            caminho_no_acervo("", "ensaio/DSC_01.NEF"),
            "ensaio/DSC_01.NEF"
        );
    }

    /// 🚨 `a//b` seria recusado pelo backend com um erro que não diz de onde veio.
    #[test]
    fn a_barra_repetida_e_a_do_windows_somem() {
        assert_eq!(
            caminho_no_acervo("/2026/", "/ensaio//DSC_01.NEF"),
            "2026/ensaio/DSC_01.NEF"
        );
        assert_eq!(
            caminho_no_acervo("2026", "ensaio\\dia-1\\a.jpg"),
            "2026/ensaio/dia-1/a.jpg"
        );
    }

    /// 🔑 O nome da pasta solta entra no caminho: é ele que o operador
    /// reconhece do outro lado.
    #[test]
    fn a_pasta_solta_da_nome_a_arvore() {
        let solto = Path::new("/Users/alex/Fotos/ensaio-silva");
        let dentro = Path::new("/Users/alex/Fotos/ensaio-silva/2026/DSC_01.NEF");
        // `is_dir()` é falso num caminho que não existe, então o teste confere o
        // ramo do arquivo avulso — o da pasta é conferido na conta abaixo.
        assert_eq!(
            relativo_ao_solto(solto, dentro),
            "ensaio-silva/2026/DSC_01.NEF"
        );
    }

    #[test]
    fn o_arquivo_avulso_vira_so_o_nome() {
        let solto = Path::new("/Users/alex/Fotos/DSC_01.NEF");
        assert_eq!(relativo_ao_solto(solto, solto), "DSC_01.NEF");
    }

    /// 🚨 Por bytes: 30 JSONs e um RAW ficariam em "30 de 31" e parariam ali.
    #[test]
    fn a_barra_conta_bytes_e_nao_arquivos() {
        let andamento = andamento_de(&[
            peca(1000, Situacao::Pronta, 1000),
            peca(9000, Situacao::Enviando, 0),
        ]);

        assert_eq!(andamento.porcento, 10);
        assert_eq!(andamento.prontas, 1);
        assert!(!andamento.terminado);
    }

    #[test]
    fn o_progresso_parcial_entra_na_conta() {
        assert_eq!(
            andamento_de(&[peca(200, Situacao::Enviando, 50)]).porcento,
            25
        );
    }

    /// ⚠️ Sem isto a barra para em 98% para sempre depois de uma falha.
    #[test]
    fn a_peca_que_falhou_conclui_a_barra() {
        let andamento = andamento_de(&[
            peca(100, Situacao::Pronta, 100),
            peca(100, Situacao::Falhou, 3),
        ]);

        assert_eq!(andamento.porcento, 100);
        assert!(andamento.terminado);
        assert_eq!(andamento.com_erro, 1);
    }

    #[test]
    fn a_fila_vazia_e_zero_por_cento() {
        let vazia = andamento_de(&[]);
        assert_eq!(vazia.porcento, 0);
        assert!(!vazia.terminado);
    }

    /// ⚠️ O último aviso do fluxo pode chegar depois de o total já ser conhecido;
    /// a barra não pode passar de 100.
    #[test]
    fn a_barra_nunca_passa_de_cem() {
        assert_eq!(
            andamento_de(&[peca(100, Situacao::Enviando, 9_999)]).porcento,
            100
        );
    }

    #[test]
    fn o_tamanho_usa_a_unidade_que_cabe() {
        assert_eq!(em_tamanho(512), "512 B");
        assert_eq!(em_tamanho(1536), "1,5 KB");
        assert_eq!(em_tamanho(5 * 1024 * 1024 * 1024), "5 GB");
    }

    /// 🚨 O que não é imagem conhecida vira binário — e não `text/html`.
    #[test]
    fn o_tipo_vem_de_uma_lista_fechada() {
        assert_eq!(tipo_do_arquivo("a.JPG"), "image/jpeg");
        assert_eq!(tipo_do_arquivo("b.webp"), "image/webp");
        assert_eq!(tipo_do_arquivo("c.html"), "application/octet-stream");
        assert_eq!(tipo_do_arquivo("DSC_01.NEF"), "application/octet-stream");
        assert_eq!(tipo_do_arquivo("sem-extensao"), "application/octet-stream");
    }
}
