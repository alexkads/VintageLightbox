//! Ferramentas de depuração: fotografar a janela e seguir um roteiro.
//!
//! Duas, para o desenho da janela poder ser conferido sem ninguém na frente
//! da tela:
//!
//! - **`VLB_FOTOS=pasta`** grava `<pasta>/<nome>.png` com o que a janela mostra,
//!   a cada passo `foto <nome>` do roteiro. O `screencapture` não tem permissão
//!   nesta máquina, mas um processo pode fotografar **a própria** janela
//!   (`CGWindowListCreateImage`, conferido no macOS 26 em 2026-09-17);
//! - **`VLB_ROTEIRO=arquivo`** segue uma lista de passos, um por linha, depois
//!   de a janela abrir (ver [`Passo`]).
//!
//! 🔒 **Só existe em build de depuração.** O binário do balcão não lê nenhuma
//! das duas variáveis (`app.rs`, `ligar_o_roteiro`).

use std::path::Path;
use std::time::Duration;

/// Um passo do roteiro. Linha vazia e linha começando com `#` são ignoradas.
#[derive(Debug, Clone, PartialEq)]
pub enum Passo {
    /// `esperar 1500` — em milissegundos.
    Esperar(Duration),
    /// `foto 01-lista` — letras, números, `-` e `_`.
    Foto(String),
    /// `ir sessoes` · `ir caixa` · `ir retencao` · `ir galeria` · `ir nova`
    Ir(String),
    /// `abrir_sessao 1` — a N-ésima da lista, contando de 1.
    AbrirSessao(usize),
    /// `atendimento` — abre ou fecha a gaveta do atendimento da sessão aberta.
    Atendimento,
    /// `detalhes` — abre ou fecha os números e prazos da sessão aberta.
    Detalhes,
    /// `foco 1` — põe o foco na N-ésima foto da grade da sessão, contando de 1.
    /// É o clique da grade, e é ele que faz o painel da direita aparecer.
    Foco(usize),
    /// `menu` — abre ou recolhe o menu lateral.
    Menu,
    /// `menu_usuario` — abre ou fecha o menu da conta.
    MenuDoUsuario,
    /// `tema claro` · `tema escuro` · `tema sistema`
    Tema(String),
    /// `tamanho 1920 1050` — a janela, em pontos.
    Tamanho(f32, f32),
    /// `revelar` — o botão "Revelar" da galeria aberta (a sessão inteira).
    Revelar,
    /// `tela_do_cliente` — o botão "Tela do cliente" (abre ou fecha).
    TelaDoCliente,
    /// `foto_do_cliente 11-cliente` — fotografa a janela da tela do cliente.
    FotoDoCliente(String),
    /// `sair_da_conta` — o mesmo "Sair" do menu da conta.
    SairDaConta,
    /// `alternar_caixa` — o F9 do caixa flutuante (abre ou minimiza).
    AlternarCaixa,
    /// `tecla_do_caixa 1` — uma tecla F do caixa flutuante. Só para abrir
    /// diálogo e fotografar: nenhum passo confirma nada.
    TeclaDoCaixa(u8),
    /// `painel rgb` · `painel srgb` · `painel abrir Curva por ponto` ·
    /// `painel rolar 600` · `painel rolar fim` — a coluna de ajustes da
    /// Revelação aberta.
    Painel(String),
    /// `revelacao enquadrar` · `revelacao angulo 5` · `revelacao proporcao 1`
    /// · `revelacao girar` · `revelacao zoom` · `revelacao ajuda` — gestos na
    /// Revelação aberta, para fotografar.
    Revelacao(String),
    /// `predefinicoes criar` · `predefinicoes zerar` · `predefinicoes renomear` ·
    /// `predefinicoes arrastar` · `predefinicoes ordem` · `predefinicoes
    /// importar <arquivo>` · `predefinicoes prever <n>` — a coluna das
    /// predefinições, para fotografar. Grava só no catálogo local.
    Predefinicoes(String),
    /// `tira recorte classificadas` · `tira altura 200` · `tira marcar 2` ·
    /// `tira faixa 4` · `tira abrir 3` · `tira rolar 300` · `tira menu 2` — a
    /// tira da Revelação aberta (a posição é a da tira, contando de 0). O
    /// `menu` dá um botão direito de verdade sobre a miniatura.
    Tira(String),
    /// `janela minimizar` · `janela fechar` · `janela abrir` · `janela
    /// fingir_envio 2` — a bandeja (`crate::segundo_plano`). `fechar` é o botão
    /// vermelho de verdade; `abrir` é o "Abrir o VintageLightbox"; `fingir_envio`
    /// só mexe na conta de pedidos pendentes, sem mandar nada ao site.
    Janela(String),
    /// `nova etapa 3` · `nova buscar agendamento|voucher|compra|parceiro` ·
    /// `nova descartar` · `nova importar <pasta>` · `nova conheceu parceiro` ·
    /// `nova preset <n>` · `nova proporcao 3:2` · `nova criar` — o assistente
    /// da nova sessão, para fotografar.
    Nova(String),
    /// `fim` — fecha o app.
    Fim,
}

/// Lê o roteiro. Uma linha que não se entende vira erro com o número dela:
/// passo ignorado em silêncio é foto do lugar errado.
pub fn ler_roteiro(texto: &str) -> Result<Vec<Passo>, String> {
    let mut passos = Vec::new();
    for (i, linha) in texto.lines().enumerate() {
        let linha = linha.trim();
        if linha.is_empty() || linha.starts_with('#') {
            continue;
        }
        let mut partes = linha.split_whitespace();
        let comando = partes.next().unwrap_or_default();
        let argumentos: Vec<&str> = partes.collect();
        let numero = |n: usize| -> Result<f32, String> {
            argumentos
                .get(n)
                .and_then(|a| a.parse::<f32>().ok())
                .ok_or_else(|| format!("linha {}: '{linha}' precisa de um número", i + 1))
        };
        let passo = match comando {
            "esperar" => Passo::Esperar(Duration::from_millis(numero(0)? as u64)),
            "foto" => {
                let nome = argumentos.first().copied().unwrap_or_default();
                if !nome_valido(nome) {
                    return Err(format!("linha {}: nome de foto inválido: '{nome}'", i + 1));
                }
                Passo::Foto(nome.to_string())
            }
            "ir" => Passo::Ir(argumentos.first().copied().unwrap_or_default().to_string()),
            "abrir_sessao" => Passo::AbrirSessao(numero(0)?.max(1.0) as usize),
            "atendimento" => Passo::Atendimento,
            "detalhes" => Passo::Detalhes,
            "foco" => Passo::Foco(numero(0)?.max(1.0) as usize),
            "menu" => Passo::Menu,
            "menu_usuario" => Passo::MenuDoUsuario,
            "tema" => Passo::Tema(argumentos.first().copied().unwrap_or_default().to_string()),
            "tamanho" => Passo::Tamanho(numero(0)?, numero(1)?),
            "revelar" => Passo::Revelar,
            "tela_do_cliente" => Passo::TelaDoCliente,
            "foto_do_cliente" => {
                let nome = argumentos.first().copied().unwrap_or_default();
                if !nome_valido(nome) {
                    return Err(format!("linha {}: nome de foto inválido: '{nome}'", i + 1));
                }
                Passo::FotoDoCliente(nome.to_string())
            }
            "sair_da_conta" => Passo::SairDaConta,
            "alternar_caixa" => Passo::AlternarCaixa,
            "tecla_do_caixa" => Passo::TeclaDoCaixa(numero(0)? as u8),
            "painel" => Passo::Painel(argumentos.join(" ")),
            "revelacao" => Passo::Revelacao(argumentos.join(" ")),
            "predefinicoes" => Passo::Predefinicoes(argumentos.join(" ")),
            "tira" => Passo::Tira(argumentos.join(" ")),
            "janela" => Passo::Janela(argumentos.join(" ")),
            "nova" => Passo::Nova(argumentos.join(" ")),
            "fim" => Passo::Fim,
            outro => return Err(format!("linha {}: passo desconhecido: '{outro}'", i + 1)),
        };
        passos.push(passo);
    }
    Ok(passos)
}

fn nome_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Grava em `destino` (PNG) o que a janela mostra agora.
#[cfg(target_os = "macos")]
pub fn fotografar(window: &gpui::Window, destino: &Path) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let alca = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(appkit) = alca.as_raw() else {
        return Err("a janela não é do AppKit".into());
    };
    let numero = unsafe { mac::numero_da_janela(appkit.ns_view.as_ptr()) };
    let (largura, altura, rgba) = unsafe { mac::capturar(numero)? };
    let imagem = image::RgbaImage::from_raw(largura, altura, rgba)
        .ok_or("a captura veio com tamanho incoerente")?;
    if let Some(pai) = destino.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    imagem.save(destino).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn fotografar(_window: &gpui::Window, _destino: &Path) -> Result<(), String> {
    Err("a foto da janela só existe no macOS".into())
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void};

    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        largura: f64,
        altura: f64,
    }

    /// `CGWindowListCreateImage(rect, listOption, windowID, imageOption)`.
    type Capturar = unsafe extern "C" fn(CGRect, u32, u32, u32) -> *mut c_void;

    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    const INCLUINDO_A_JANELA: u32 = 1 << 3;
    const SEM_MOLDURA: u32 = 1 << 0;
    const PRIMEIRO_O_ALFA: u32 = 2; // kCGImageAlphaPremultipliedFirst
    const ALFA_MASCARA: u32 = 0x1f;
    const ORDEM_MASCARA: u32 = 0x7000;
    const PEQUENA: u32 = 2 << 12; // kCGBitmapByteOrder32Little

    extern "C" {
        fn dlsym(alca: *mut c_void, nome: *const c_char) -> *mut c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(imagem: *mut c_void) -> usize;
        fn CGImageGetHeight(imagem: *mut c_void) -> usize;
        fn CGImageGetBytesPerRow(imagem: *mut c_void) -> usize;
        fn CGImageGetBitsPerPixel(imagem: *mut c_void) -> usize;
        fn CGImageGetBitmapInfo(imagem: *mut c_void) -> u32;
        fn CGImageGetDataProvider(imagem: *mut c_void) -> *mut c_void;
        fn CGDataProviderCopyData(provedor: *mut c_void) -> *mut c_void;
        fn CGImageRelease(imagem: *mut c_void);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDataGetBytePtr(dados: *mut c_void) -> *const u8;
        fn CFDataGetLength(dados: *mut c_void) -> isize;
        fn CFRelease(objeto: *mut c_void);
    }

    /// O número da janela no WindowServer, a partir da `NSView` do GPUI.
    pub unsafe fn numero_da_janela(vista: *mut c_void) -> u32 {
        let vista = vista as *mut AnyObject;
        let janela: *mut AnyObject = msg_send![vista, window];
        if janela.is_null() {
            return 0;
        }
        let numero: isize = msg_send![janela, windowNumber];
        numero as u32
    }

    /// A janela em RGBA8. A função é declarada obsoleta no SDK do macOS 15,
    /// mas continua no CoreGraphics; por isso é procurada pelo nome.
    pub unsafe fn capturar(numero: u32) -> Result<(u32, u32, Vec<u8>), String> {
        let simbolo = dlsym(RTLD_DEFAULT, c"CGWindowListCreateImage".as_ptr());
        if simbolo.is_null() {
            return Err("CGWindowListCreateImage não existe neste macOS".into());
        }
        let capturar: Capturar = std::mem::transmute(simbolo);
        let nulo = CGRect {
            x: f64::INFINITY,
            y: f64::INFINITY,
            largura: 0.0,
            altura: 0.0,
        };
        let imagem = capturar(nulo, INCLUINDO_A_JANELA, numero, SEM_MOLDURA);
        if imagem.is_null() {
            return Err("o WindowServer não devolveu a imagem".into());
        }
        let largura = CGImageGetWidth(imagem);
        let altura = CGImageGetHeight(imagem);
        let por_linha = CGImageGetBytesPerRow(imagem);
        let bits = CGImageGetBitsPerPixel(imagem);
        let info = CGImageGetBitmapInfo(imagem);
        let dados = CGDataProviderCopyData(CGImageGetDataProvider(imagem));
        CGImageRelease(imagem);
        if dados.is_null() || bits != 32 {
            if !dados.is_null() {
                CFRelease(dados);
            }
            return Err(format!("formato inesperado: {bits} bits por pixel"));
        }
        let bytes =
            std::slice::from_raw_parts(CFDataGetBytePtr(dados), CFDataGetLength(dados) as usize);
        // O WindowServer entrega BGRA com o alfa primeiro e ordem pequena, que
        // na memória é B, G, R, A.
        let bgra = info & ORDEM_MASCARA == PEQUENA && info & ALFA_MASCARA == PRIMEIRO_O_ALFA;
        let mut rgba = Vec::with_capacity(largura * altura * 4);
        for y in 0..altura {
            let linha = &bytes[y * por_linha..y * por_linha + largura * 4];
            for &[a, b, c, d] in linha.as_chunks::<4>().0 {
                if bgra {
                    rgba.extend_from_slice(&[c, b, a, 255]);
                } else {
                    rgba.extend_from_slice(&[b, c, d, 255]);
                }
            }
        }
        CFRelease(dados);
        Ok((largura as u32, altura as u32, rgba))
    }
}

/// A sessão num arquivo, para a rodada de depuração não depender do chaveiro.
///
/// 🔧 Cada recompilação é outro binário para o macOS, que pergunta de novo se
/// libera o item do chaveiro — e um roteiro não tem quem clique. Com
/// `VLB_SESSAO_EM_ARQUIVO=1` (só em depuração), a sessão fica em
/// `sessao-dev.json`, com permissão só do usuário.
pub struct CofreEmArquivo {
    pub arquivo: std::path::PathBuf,
}

impl CofreEmArquivo {
    /// O arquivo ao lado dos dados do app.
    ///
    /// 🔧 **`VLB_SESSAO_ARQUIVO=<caminho>` troca o arquivo**: é como se abre uma
    /// sessão de teste (a da pilha local, por exemplo) sem passar por cima da
    /// que já está guardada.
    pub fn padrao() -> Self {
        if let Some(caminho) = std::env::var_os("VLB_SESSAO_ARQUIVO") {
            return Self {
                arquivo: std::path::PathBuf::from(caminho),
            };
        }
        let dados = infrastructure::paths::AppPaths::home_dir()
            .join("Library")
            .join("Application Support");
        Self {
            arquivo: dados.join("VintageLightbox").join("sessao-dev.json"),
        }
    }

    fn gravar(&self, bytes: &[u8]) {
        if let Some(pai) = self.arquivo.parent() {
            let _ = std::fs::create_dir_all(pai);
        }
        let _ = std::fs::write(&self.arquivo, bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.arquivo, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SessaoEmArquivo {
    access_token: String,
    refresh_token: String,
    access_vence_em: i64,
    refresh_vence_em: i64,
}

impl domain::services::pos_venda::CofreDeSessao for CofreEmArquivo {
    fn guardar(&self, sessao: &domain::services::pos_venda::Sessao) {
        let guardada = SessaoEmArquivo {
            access_token: sessao.access_token.clone(),
            refresh_token: sessao.refresh_token.clone(),
            access_vence_em: sessao.access_vence_em,
            refresh_vence_em: sessao.refresh_vence_em,
        };
        if let Ok(texto) = serde_json::to_vec(&guardada) {
            self.gravar(&texto);
        }
    }

    fn ler(&self) -> Option<domain::services::pos_venda::Sessao> {
        let bytes = std::fs::read(&self.arquivo).ok()?;
        let g: SessaoEmArquivo = serde_json::from_slice(&bytes).ok()?;
        Some(domain::services::pos_venda::Sessao {
            access_token: g.access_token,
            refresh_token: g.refresh_token,
            access_vence_em: g.access_vence_em,
            refresh_vence_em: g.refresh_vence_em,
        })
    }

    fn esquecer(&self) {
        let _ = std::fs::remove_file(&self.arquivo);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use domain::services::pos_venda::{CofreDeSessao, Sessao};

    #[test]
    fn o_cofre_de_arquivo_guarda_le_e_esquece() {
        let pasta = tempfile::tempdir().unwrap();
        let cofre = CofreEmArquivo {
            arquivo: pasta.path().join("gpui").join("sessao-dev.json"),
        };
        assert!(cofre.ler().is_none(), "sem arquivo não há sessão");
        cofre.guardar(&Sessao {
            access_token: "b".into(),
            refresh_token: "s".into(),
            access_vence_em: 3,
            refresh_vence_em: 4,
        });
        assert!(
            cofre.arquivo.exists(),
            "o arquivo nasce na primeira gravação"
        );
        assert_eq!(cofre.ler().unwrap().access_token, "b");
        assert_eq!(cofre.ler().unwrap().refresh_token, "s");
        cofre.esquecer();
        assert!(!cofre.arquivo.exists());
    }

    #[test]
    fn o_roteiro_le_cada_passo_e_ignora_comentario() {
        let passos = ler_roteiro(
            "# abrir e fotografar\n\
             tamanho 1920 1050\n\
             esperar 1500\n\
             foto 01-lista\n\
             \n\
             ir caixa\n\
             abrir_sessao 2\n\
             menu\n\
             menu_usuario\n\
             tema claro\n\
             revelar\n\
             sair_da_conta\n\
             fim\n",
        )
        .unwrap();
        assert_eq!(
            passos,
            vec![
                Passo::Tamanho(1920.0, 1050.0),
                Passo::Esperar(Duration::from_millis(1500)),
                Passo::Foto("01-lista".into()),
                Passo::Ir("caixa".into()),
                Passo::AbrirSessao(2),
                Passo::Menu,
                Passo::MenuDoUsuario,
                Passo::Tema("claro".into()),
                Passo::Revelar,
                Passo::SairDaConta,
                Passo::Fim,
            ]
        );
    }

    #[test]
    fn linha_que_nao_se_entende_diz_qual_e() {
        let erro = ler_roteiro("esperar 10\nvoar alto\n").unwrap_err();
        assert!(erro.contains("linha 2"), "{erro}");
        assert!(ler_roteiro("foto ../fora").is_err());
        assert!(ler_roteiro("esperar muito").is_err());
    }
}
