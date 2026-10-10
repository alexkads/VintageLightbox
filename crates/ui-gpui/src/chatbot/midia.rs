//! 📎 A mídia das conversas do WhatsApp — o que o cliente manda (foto,
//! documento, áudio) e o anexo que o operador manda daqui.
//!
//! Dois pedidos do dono em 2026-10-10: *"quando o cliente envia uma imagem ou
//! documento e não consigo visualizar pelo bot do whatsapp"* e *"também preciso
//! ter a opção de enviar anexo para o cliente pelo bot do whatsapp respeitando
//! a minha janela"* — e, no mesmo dia, o áudio e a transcrição dele.
//!
//! | Aqui | No site |
//! |---|---|
//! | [`Midia`] | `midiaDaMensagemSchema` + `MidiaDoBalao` (`modelo.ts`) |
//! | [`recusa_do_anexo`], [`texto_do_anexo_em_voo`] | `anexo.ts` |
//! | [`multipart`] | o `FormData` de `enviarAnexoManual` |
//!
//! 🔑 **Nada aqui sabe de tela nem de rede**, como o resto do modelo.
//!
//! # O que difere do site, e por quê
//!
//! - **O teto do anexo é o da API** (5 MB a foto, 16 MB o resto), e não os
//!   4 MB do site: aquele número é da Vercel, por onde o arquivo do site passa
//!   e o do app não.
//! - **O app não grava áudio pelo microfone** — manda arquivo de áudio. O
//!   gravador do site usa o do navegador; aqui pediria um codificador nativo
//!   nos três sistemas, que não cabe numa entrega só.

use domain::services::pos_venda::CorpoDoPedido;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDeMidia {
    Imagem,
    Documento,
    Audio,
    Video,
    Figurinha,
}

impl TipoDeMidia {
    /// Os nomes da API (`domain::whatsapp::TipoDeMidia`, em minúsculas).
    fn da_api(tipo: &str) -> Option<Self> {
        match tipo {
            "image" => Some(Self::Imagem),
            "document" => Some(Self::Documento),
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            "sticker" => Some(Self::Figurinha),
            _ => None,
        }
    }

    /// O que o cartão do arquivo diz quando a mídia não tem nome.
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Imagem => "Foto",
            Self::Documento => "Documento",
            Self::Audio => "Áudio",
            Self::Video => "Vídeo",
            Self::Figurinha => "Figurinha",
        }
    }

    /// A foto e a figurinha aparecem **dentro** do balão; o resto é um cartão
    /// que abre no programa do sistema.
    pub fn aparece_no_balao(self) -> bool {
        matches!(self, Self::Imagem | Self::Figurinha)
    }
}

/// A mídia de uma mensagem, como a API a descreve. O arquivo em si se busca
/// por [`caminho_da_midia`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Midia {
    pub tipo: TipoDeMidia,
    pub mime: Option<String>,
    pub nome: Option<String>,
    pub tamanho: Option<u64>,
    /// O texto da mensagem é legenda de verdade, e não o rótulo ("📷 Foto")
    /// posto no lugar do texto que faltava.
    pub com_legenda: bool,
    /// 🎤 `None` é "ainda não transcrito"; `Some("")` é áudio sem fala.
    pub transcricao: Option<String>,
}

impl Midia {
    /// O campo `midia` de uma mensagem. `None` para a mensagem só de texto — e
    /// para um tipo que este app ainda não conhece, que segue como texto.
    pub fn da_mensagem(mensagem: &Value) -> Option<Self> {
        let midia = mensagem.get("midia").filter(|m| m.is_object())?;
        let texto = |campo: &str| midia.get(campo).and_then(Value::as_str).map(str::to_string);
        Some(Self {
            tipo: TipoDeMidia::da_api(midia.get("tipo")?.as_str()?)?,
            mime: texto("mime"),
            nome: texto("nome").filter(|n| !n.trim().is_empty()),
            tamanho: midia.get("tamanho").and_then(Value::as_u64),
            com_legenda: midia
                .get("com_legenda")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            transcricao: texto("transcricao"),
        })
    }

    /// O título do cartão: o nome do arquivo, ou o tipo.
    pub fn titulo(&self) -> String {
        self.nome
            .clone()
            .unwrap_or_else(|| self.tipo.rotulo().to_string())
    }

    /// A segunda linha do cartão: `"1,2 MB · abrir"`.
    pub fn detalhe(&self) -> String {
        match self.tamanho {
            Some(tamanho) => format!("{} · abrir", formatar_tamanho(tamanho)),
            None => "abrir".to_string(),
        }
    }

    /// O nome com que o arquivo é gravado antes de abrir no programa do
    /// sistema — é a **extensão** que escolhe o programa.
    ///
    /// O nome que o cliente deu vale quando é só um nome; senão, um montado
    /// com a extensão do tipo que a API declarou na resposta.
    pub fn nome_para_abrir(&self, mensagem_id: &str, tipo_da_resposta: Option<&str>) -> String {
        if let Some(nome) = self.nome.as_deref().map(nome_seguro) {
            if nome.contains('.') {
                return nome;
            }
        }
        let cauda: String = mensagem_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let tipo = tipo_da_resposta.or(self.mime.as_deref());
        format!(
            "{}-{cauda}.{}",
            self.tipo
                .rotulo()
                .to_lowercase()
                .replace('á', "a")
                .replace('í', "i"),
            extensao_do_tipo(tipo)
        )
    }
}

/// O nome sem caminho e sem o que um sistema de arquivos recusa.
fn nome_seguro(nome: &str) -> String {
    let sem_caminho = nome.rsplit(['/', '\\']).next().unwrap_or_default();
    let limpo: String = sem_caminho
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    limpo.trim().trim_matches('.').to_string()
}

/// A extensão de um `Content-Type` — os que a API serve
/// (`application::whatsapp::midia::EM_LINHA`). Desconhecido vira `bin`.
fn extensao_do_tipo(tipo: Option<&str>) -> &'static str {
    let puro = tipo
        .and_then(|t| t.split(';').next())
        .map(|t| t.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match puro.as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "application/pdf" => "pdf",
        "audio/ogg" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/aac" => "aac",
        "audio/amr" => "amr",
        "video/mp4" => "mp4",
        "video/3gpp" => "3gp",
        _ => "bin",
    }
}

/// `GET /whatsapp/messages/{id}/midia` — o arquivo da mensagem.
///
/// 🚨 Montado pelo **id da mensagem**, nunca por endereço que veio no
/// conteúdo: quem manda mensagem para o número não escolhe o que o app busca.
pub fn caminho_da_midia(mensagem_id: &str) -> String {
    format!(
        "/whatsapp/messages/{}/midia",
        super::pedidos::codificar(mensagem_id)
    )
}

/// `"1,2 MB"`, `"340 kB"` — como no site (`formatarTamanho`).
pub fn formatar_tamanho(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MB {
        // Uma casa, com vírgula; `2,0` vira `2`, como o `toLocaleString`.
        let decimos = (bytes as f64 / MB * 10.0).round() as u64;
        return if decimos.is_multiple_of(10) {
            format!("{} MB", decimos / 10)
        } else {
            format!("{},{} MB", decimos / 10, decimos % 10)
        };
    }
    format!("{} kB", ((bytes as f64 / 1024.0).round() as u64).max(1))
}

// ── O anexo que sai daqui ──────────────────────────────────────────────────

/// A maior foto: o teto da Cloud API para imagem.
pub const TETO_DA_FOTO: u64 = 5 * 1024 * 1024;
/// O maior áudio ou documento: o teto do backend (`TETO_DO_DOCUMENTO`).
pub const TETO_DO_ARQUIVO: u64 = 16 * 1024 * 1024;

/// Como um anexo sai: foto vira imagem na conversa do cliente, áudio toca lá,
/// e o resto chega como documento, com o nome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseDoAnexo {
    Foto,
    Audio,
    Documento,
}

/// A extensão, o tipo declarado no envio e a classe — a mesma lista de
/// `ANEXOS` no backend, menos o `webm` (que só o gravador do site produz).
const ANEXOS: &[(&str, &str, ClasseDoAnexo)] = &[
    ("jpg", "image/jpeg", ClasseDoAnexo::Foto),
    ("jpeg", "image/jpeg", ClasseDoAnexo::Foto),
    ("png", "image/png", ClasseDoAnexo::Foto),
    ("ogg", "audio/ogg", ClasseDoAnexo::Audio),
    ("opus", "audio/ogg", ClasseDoAnexo::Audio),
    ("mp3", "audio/mpeg", ClasseDoAnexo::Audio),
    ("m4a", "audio/mp4", ClasseDoAnexo::Audio),
    ("aac", "audio/aac", ClasseDoAnexo::Audio),
    ("amr", "audio/amr", ClasseDoAnexo::Audio),
    ("pdf", "application/pdf", ClasseDoAnexo::Documento),
    ("txt", "text/plain", ClasseDoAnexo::Documento),
    ("doc", "application/msword", ClasseDoAnexo::Documento),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ClasseDoAnexo::Documento,
    ),
    ("xls", "application/vnd.ms-excel", ClasseDoAnexo::Documento),
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ClasseDoAnexo::Documento,
    ),
    (
        "ppt",
        "application/vnd.ms-powerpoint",
        ClasseDoAnexo::Documento,
    ),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ClasseDoAnexo::Documento,
    ),
];

/// As extensões que o seletor de arquivo oferece.
pub fn extensoes_aceitas() -> Vec<&'static str> {
    ANEXOS.iter().map(|(extensao, _, _)| *extensao).collect()
}

fn do_anexo(nome: &str) -> Option<&'static (&'static str, &'static str, ClasseDoAnexo)> {
    let extensao = nome.rsplit_once('.')?.1.to_ascii_lowercase();
    ANEXOS.iter().find(|(ext, _, _)| *ext == extensao)
}

/// Foto, áudio ou documento — `None` para o que o WhatsApp não leva.
pub fn classe_do_anexo(nome: &str) -> Option<ClasseDoAnexo> {
    do_anexo(nome).map(|(_, _, classe)| *classe)
}

/// O que impede o envio, ou `None` quando o arquivo serve. As frases são as
/// do site, com os tetos daqui.
pub fn recusa_do_anexo(nome: &str, tamanho: u64) -> Option<String> {
    let Some(classe) = classe_do_anexo(nome) else {
        return Some(
            "Este tipo de arquivo não vai pelo WhatsApp. Anexe foto (JPG ou PNG), áudio, PDF, \
             Word, Excel, PowerPoint ou texto."
                .into(),
        );
    };
    if tamanho == 0 {
        return Some("O arquivo está vazio.".into());
    }
    let (teto, o_que) = match classe {
        ClasseDoAnexo::Foto => (TETO_DA_FOTO, "A foto"),
        ClasseDoAnexo::Audio => (TETO_DO_ARQUIVO, "O áudio"),
        ClasseDoAnexo::Documento => (TETO_DO_ARQUIVO, "O arquivo"),
    };
    (tamanho > teto).then(|| {
        format!(
            "{o_que} tem {}; o limite é {}.",
            formatar_tamanho(tamanho),
            formatar_tamanho(teto)
        )
    })
}

/// O que o balão "enviando" mostra. O áudio mostra o rótulo que o servidor
/// grava — ele não leva legenda —, e o resto, a legenda ou o nome do arquivo.
pub fn texto_do_anexo_em_voo(nome: &str, legenda: &str) -> String {
    if classe_do_anexo(nome) == Some(ClasseDoAnexo::Audio) {
        return "🎤 Áudio".into();
    }
    let legenda = legenda.trim();
    if legenda.is_empty() {
        format!("📎 {nome}")
    } else {
        legenda.to_string()
    }
}

/// O corpo de `POST /whatsapp/messages/send-media`: `contact_id`, `caption`
/// (quando há) e `file`.
///
/// `limite` vem de fora para o teste conferir o corpo byte a byte; quem chama
/// passa um que não aparece no arquivo ([`limite_do_multipart`]).
pub fn multipart(
    contato: &str,
    legenda: &str,
    nome: &str,
    bytes: &[u8],
    limite: &str,
) -> CorpoDoPedido {
    let tipo = do_anexo(nome).map_or("application/octet-stream", |(_, tipo, _)| tipo);
    // Aspas e quebras de linha no nome quebrariam o cabeçalho da parte.
    let nome: String = nome
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .collect();

    let mut corpo = Vec::with_capacity(bytes.len() + 512);
    let mut campo = |nome_do_campo: &str, valor: &str| {
        corpo.extend_from_slice(
            format!(
                "--{limite}\r\nContent-Disposition: form-data; name=\"{nome_do_campo}\"\r\n\r\n{valor}\r\n"
            )
            .as_bytes(),
        );
    };
    campo("contact_id", contato.trim());
    let legenda = legenda.trim();
    if !legenda.is_empty() {
        campo("caption", legenda);
    }
    corpo.extend_from_slice(
        format!(
            "--{limite}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{nome}\"\r\n\
             Content-Type: {tipo}\r\n\r\n"
        )
        .as_bytes(),
    );
    corpo.extend_from_slice(bytes);
    corpo.extend_from_slice(format!("\r\n--{limite}--\r\n").as_bytes());

    CorpoDoPedido {
        tipo: format!("multipart/form-data; boundary={limite}"),
        bytes: corpo,
    }
}

/// Um limite de multipart que não aparece no arquivo. O relógio dá a
/// variação; a conferência contra os bytes é o que garante.
pub fn limite_do_multipart(bytes: &[u8]) -> String {
    let mut semente = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    loop {
        let limite = format!("----vintagelightbox-{semente:032x}");
        let procurado = limite.as_bytes();
        if !bytes
            .windows(procurado.len())
            .any(|janela| janela == procurado)
        {
            return limite;
        }
        semente = semente.wrapping_add(1);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_midia_sai_do_json_da_api() {
        let foto = json!({ "id": "m1", "midia": {
            "tipo": "image", "id_na_meta": "111", "mime": "image/jpeg", "nome": null,
            "tamanho": 120_000, "chave": "whatsapp/midia/x/m1.jpg", "com_legenda": false
        }});
        let midia = Midia::da_mensagem(&foto).expect("tem mídia");
        assert_eq!(midia.tipo, TipoDeMidia::Imagem);
        assert_eq!(midia.tamanho, Some(120_000));
        assert!(!midia.com_legenda);
        assert_eq!(midia.transcricao, None);
        assert_eq!(midia.titulo(), "Foto");
        assert_eq!(midia.detalhe(), "117 kB · abrir");

        let audio = json!({ "midia": {
            "tipo": "audio", "mime": "audio/ogg; codecs=opus", "transcricao": ""
        }});
        // Vazio é resposta ("áudio sem fala"), diferente de ausente.
        assert_eq!(
            Midia::da_mensagem(&audio).unwrap().transcricao.as_deref(),
            Some("")
        );
    }

    #[test]
    fn mensagem_de_texto_e_tipo_desconhecido_nao_tem_midia() {
        assert_eq!(Midia::da_mensagem(&json!({ "id": "m" })), None);
        assert_eq!(Midia::da_mensagem(&json!({ "midia": null })), None);
        // Um tipo que a API ganhe depois não derruba a leitura: segue como texto.
        assert_eq!(
            Midia::da_mensagem(&json!({ "midia": { "tipo": "holograma" } })),
            None
        );
    }

    #[test]
    fn so_foto_e_figurinha_aparecem_dentro_do_balao() {
        assert!(TipoDeMidia::Imagem.aparece_no_balao());
        assert!(TipoDeMidia::Figurinha.aparece_no_balao());
        for tipo in [
            TipoDeMidia::Documento,
            TipoDeMidia::Audio,
            TipoDeMidia::Video,
        ] {
            assert!(!tipo.aparece_no_balao());
        }
    }

    #[test]
    fn o_nome_para_abrir_carrega_a_extensao_que_escolhe_o_programa() {
        let mut documento = Midia {
            tipo: TipoDeMidia::Documento,
            mime: Some("application/pdf".into()),
            nome: Some("comprovante do pix.pdf".into()),
            tamanho: None,
            com_legenda: false,
            transcricao: None,
        };
        assert_eq!(
            documento.nome_para_abrir("0199aaaa-bbbb", None),
            "comprovante do pix.pdf"
        );

        // Nome hostil não vira caminho.
        documento.nome = Some("../../etc/passwd".into());
        assert_eq!(
            documento.nome_para_abrir("0199aaaa-bbbb", Some("application/pdf")),
            "documento-0199aaaa.pdf"
        );

        // Sem nome: o tipo que a API declarou na resposta manda.
        let audio = Midia {
            tipo: TipoDeMidia::Audio,
            mime: None,
            nome: None,
            tamanho: None,
            com_legenda: false,
            transcricao: None,
        };
        assert_eq!(
            audio.nome_para_abrir("0199aaaa-bbbb", Some("audio/ogg; codecs=opus")),
            "audio-0199aaaa.ogg"
        );
        assert_eq!(audio.nome_para_abrir("x", None), "audio-x.bin");
    }

    #[test]
    fn o_caminho_sai_do_id_da_mensagem() {
        assert_eq!(
            caminho_da_midia("abc-123"),
            "/whatsapp/messages/abc-123/midia"
        );
        assert_eq!(
            caminho_da_midia("a/b?c"),
            "/whatsapp/messages/a%2Fb%3Fc/midia"
        );
    }

    #[test]
    fn o_tamanho_sai_como_no_site() {
        assert_eq!(formatar_tamanho(340 * 1024), "340 kB");
        assert_eq!(formatar_tamanho(1_310_720), "1,3 MB");
        assert_eq!(formatar_tamanho(2 * 1024 * 1024), "2 MB");
        assert_eq!(formatar_tamanho(10), "1 kB");
    }

    #[test]
    fn a_recusa_do_anexo_diz_o_motivo() {
        for nome in [
            "a.jpg", "A.JPEG", "b.png", "c.pdf", "d.docx", "voz.ogg", "r.mp3",
        ] {
            assert_eq!(recusa_do_anexo(nome, 1000), None, "{nome}");
        }
        for nome in [
            "a.webp",
            "b.html",
            "c.zip",
            "d.mp4",
            "sem-extensao",
            "voz.webm",
        ] {
            assert!(
                recusa_do_anexo(nome, 1000)
                    .unwrap()
                    .contains("não vai pelo WhatsApp"),
                "{nome}"
            );
        }
        assert_eq!(
            recusa_do_anexo("a.pdf", 0).as_deref(),
            Some("O arquivo está vazio.")
        );
        // 🔑 Os tetos são os da API, e não os 4 MB do site (que são da Vercel).
        assert_eq!(recusa_do_anexo("contrato.pdf", 10 * 1024 * 1024), None);
        assert_eq!(
            recusa_do_anexo("foto.jpg", TETO_DA_FOTO + 1).as_deref(),
            Some("A foto tem 5 MB; o limite é 5 MB.")
        );
        assert!(
            recusa_do_anexo("contrato.pdf", TETO_DO_ARQUIVO + 1024 * 1024)
                .unwrap()
                .starts_with("O arquivo tem 17 MB")
        );
    }

    #[test]
    fn o_balao_em_voo_mostra_a_legenda_o_nome_ou_o_rotulo_do_audio() {
        assert_eq!(texto_do_anexo_em_voo("a.pdf", "  "), "📎 a.pdf");
        assert_eq!(texto_do_anexo_em_voo("a.pdf", " segue "), "segue");
        // Áudio não leva legenda: o balão mostra o que o servidor grava.
        assert_eq!(texto_do_anexo_em_voo("voz.ogg", "ouve aí"), "🎤 Áudio");
        assert_eq!(classe_do_anexo("voz.OGG"), Some(ClasseDoAnexo::Audio));
        assert_eq!(classe_do_anexo("foto.png"), Some(ClasseDoAnexo::Foto));
    }

    #[test]
    fn o_multipart_leva_o_contato_a_legenda_e_o_arquivo() {
        let corpo = multipart(" 5554999 ", " segue ", "or\"ça.pdf", b"%PDF", "LIM");
        assert_eq!(corpo.tipo, "multipart/form-data; boundary=LIM");
        assert_eq!(
            String::from_utf8(corpo.bytes).unwrap(),
            "--LIM\r\nContent-Disposition: form-data; name=\"contact_id\"\r\n\r\n5554999\r\n\
             --LIM\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nsegue\r\n\
             --LIM\r\nContent-Disposition: form-data; name=\"file\"; filename=\"orça.pdf\"\r\n\
             Content-Type: application/pdf\r\n\r\n%PDF\r\n--LIM--\r\n"
        );

        // Sem legenda o campo não vai: vazio seria uma legenda vazia.
        let sem = multipart("5554999", "", "foto.jpg", b"x", "LIM");
        let texto = String::from_utf8(sem.bytes).unwrap();
        assert!(!texto.contains("caption"));
        assert!(texto.contains("Content-Type: image/jpeg"));
    }

    #[test]
    fn o_limite_nao_aparece_no_arquivo() {
        let bytes = b"----vintagelightbox-".to_vec();
        let limite = limite_do_multipart(&bytes);
        assert!(limite.starts_with("----vintagelightbox-"));
        assert!(!bytes
            .windows(limite.len())
            .any(|janela| janela == limite.as_bytes()));
    }
}
