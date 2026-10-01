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

/// O roteiro e o cofre em arquivo valem: em `debug`, ou num binário otimizado
/// compilado com a feature `roteiro` — o de medir desempenho.
pub const fn ferramentas_ligadas() -> bool {
    cfg!(debug_assertions) || cfg!(feature = "roteiro")
}

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
    /// `conferir_conta` — falha se a autorização local ainda não terminou.
    ConferirConta,
    /// `conferir_selecao 2` — falha se a janela real não tiver N fotos marcadas.
    ConferirSelecao(usize),
    /// `conversa 1` — abre a N-ésima conversa da lista do chatbot.
    Conversa(usize),
    /// `conferir_chatbot 1` — falha se a lista do chatbot tiver menos de N
    /// conversas lidas da API.
    ConferirChatbot(usize),
    /// `conferir_novidades 1` — falha se o chatbot não tiver ao menos N
    /// conversas com mensagem nova (o tempo real chegou até a tela).
    ConferirNovidades(usize),
    /// `conferir_agenda 1` — falha se a agenda tiver menos de N agendamentos
    /// lidos da API no período.
    ConferirAgenda(usize),
    /// `menu` — abre ou recolhe o menu lateral.
    Menu,
    /// Liga ou desliga a linha de filtros por coluna da lista de sessões.
    Filtros,
    /// `graficos` — abre o diálogo dos gráficos da lista de sessões.
    Graficos,
    /// `formas_do_caixa` — abre ou fecha as formas no cartão do caixa.
    FormasDoCaixa,
    /// `buscar <texto>` — escreve na busca da lista de sessões.
    Buscar(String),
    /// `periodo <de> <ate>` (ou `periodo tudo`) — o período da lista.
    Periodo(Option<(String, String)>),
    /// Abre o modal "Dados do cliente" da sessão aberta.
    DadosDoCliente,
    /// `negociar` — o "Negociação…" do painel na foto em foco. Depois,
    /// `negociar parceiro` troca o tipo (`cortesia`, `desconto`, `parceiro`,
    /// `outro`) e falha se o diálogo não tiver aberto. Não salva nada.
    Negociar(String),
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
    /// `painel rgb` · `painel srgb` · `painel abrir Curva de tons` ·
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
    /// 🖌️ A janela do editor em camadas aberta (`tira editar N` a abre):
    /// `editor mouse apertar|arrastar|soltar fx fy` (fração da foto, evento
    /// **real** do AppKit na janela do editor) · `editor tecla <keyCode> [mods]`
    /// · `editor foto <nome>` · `editor estado` (uma linha no stderr).
    Editor(String),
    /// `guias menu 1` · `guias fechar_menu` · `guias renomear 1` · `guias nome 1
    /// Prova` · `guias cor 1 purple` · `guias mover 1 0` — a faixa das guias
    /// (posição contando de 0). O `menu` desenha o mesmo menu do botão direito
    /// sobre a guia.
    Guias(String),
    /// `importar_modal` — o botão "Importar fotos" da sessão aberta: abre o
    /// modal do quadro, sem abrir a janela do sistema.
    ImportarModal,
    /// `importar_origem` — o mesmo modal, já com o menu do "Do cartão ou
    /// pasta…" aberto: os cartões montados e o "Escolher pasta…".
    ImportarOrigem,
    /// `janela minimizar` · `janela fechar` · `janela abrir` · `janela
    /// fingir_envio 2` — a bandeja (`crate::segundo_plano`). `fechar` é o botão
    /// vermelho de verdade; `abrir` é o "Abrir o VintageLightbox"; `fingir_envio`
    /// só mexe na conta de pedidos pendentes, sem mandar nada ao site;
    /// `janela frente` traz o app para o foco; `janela fingir_recusa <motivo>`
    /// põe uma recusa de mentira na lista local, e `janela recusas` abre a
    /// lista (o clique nos recusados do rodapé).
    Janela(String),
    /// `nova etapa 3` · `nova buscar agendamento|voucher|compra|parceiro` ·
    /// `nova descartar` · `nova importar <pasta>` · `nova conheceu parceiro` ·
    /// `nova preset <n>` · `nova proporcao 3:2` · `nova titulo <texto>` ·
    /// `nova produto <id>` · `nova estudio <id>` · `nova criar` — o assistente
    /// da nova sessão, para fotografar.
    Nova(String),
    /// `importar <pasta>` — as fotos da pasta entram na sessão aberta, como se
    /// soltas na grade.
    Importar(String),
    /// `tecla 3` · `tecla x` · `tecla cmd-a` — uma tecla de verdade na janela,
    /// pelo mesmo despacho do teclado: passa pelo foco e pelos contextos, que é
    /// onde um atalho morto se esconde.
    Tecla(String),
    /// `tecla_real 33` · `tecla_real 30 shift` — a tecla **física** (o
    /// `keyCode` do macOS), num `NSEvent` entregue à fila do próprio app. Passa
    /// pela tradução do layout (ABNT2, Brazilian Pro…) e pelo foco reais, que
    /// o `tecla` pula. Não precisa da permissão de acessibilidade.
    TeclaReal { codigo: u16, modificadores: usize },
    /// `mouse_real apertar 0.5 0.5` · `arrastar` · `soltar` · `mover` ·
    /// `duplo` — o botão esquerdo de verdade (`NSEvent`), numa fração do palco
    /// da foto aberta na Revelação.
    ///
    /// ⚠️ **O `mover` sem botão não dá hover** a um app aberto pelo terminal:
    /// no macOS o GPUI só considera sob o mouse a janela **ativa**
    /// (`is_window_hovered` = `is_window_active`), e o sistema não deixa um
    /// processo em segundo plano se ativar. Apertar, arrastar e soltar chegam.
    ///
    /// `mouse_janela clicar 120 340` — o mesmo, em pontos da janela (o painel,
    /// a barra). Os dois aceitam modificadores no fim: `alt`, `shift`, `cmd`,
    /// `ctrl` (e `tecla_real` também: `tecla_real 6 cmd shift`).
    MouseReal {
        tipo: String,
        x: f32,
        y: f32,
        modificadores: usize,
        na_janela: bool,
    },
    /// `pinca 0.05 0.5 0.5` — a pinça do trackpad (o `magnify` do AppKit), com
    /// o quanto a escala cresce (0,05 = 5%; negativo afasta), numa fração do
    /// palco. O AppKit não deixa criar esse `NSEvent`: ela entra pelo
    /// `dispatch_event` da janela, o mesmo caminho que o evento nativo
    /// percorre depois de traduzido.
    Pinca { delta: f32, x: f32, y: f32 },
    /// `rajada 200 25 tecla right` — o passo do fim da linha, N vezes, com o
    /// intervalo em milissegundos, **sem** o respiro de 120 ms entre passos: é
    /// a carga do teste de estresse (dono, 2026-09-22: *"a aplicação parou de
    /// responder durante o uso"*).
    Rajada {
        vezes: usize,
        intervalo: Duration,
        passo: Box<Passo>,
    },
    /// `varrer 60 16 520 700 760 700` — um arrasto com o botão esquerdo, em
    /// pontos da janela: aperta no primeiro ponto, anda N vezes até o segundo
    /// (um movimento a cada `ms`) e solta. Entra pelo `dispatch_event` do
    /// GPUI, e por isso vale no **Windows e no Linux** também — é o cenário
    /// reproduzível da ferramenta de desempenho (um slider, um pincel).
    Varrer {
        vezes: usize,
        intervalo: Duration,
        de: (f32, f32),
        ate: (f32, f32),
    },
    /// `rolar 40 16 800 500 -60` — N giros da roda em `(x, y)` da janela, de
    /// `dy` pixels cada, com o intervalo em milissegundos.
    Rolar {
        vezes: usize,
        intervalo: Duration,
        em: (f32, f32),
        dy: f32,
    },
    /// `desempenho iniciar` · `parar` · `salvar` · `abrir` · `fechar` ·
    /// `relatorio` (imprime o texto do "Copiar relatório" no terminal).
    Desempenho(String),
    /// `aviso ok <texto>` · `aviso erro <texto>` — o toast da raiz, para
    /// conferir na tela o que o operador vê (o erro passa pela tradução do
    /// `erro_da_api`).
    Aviso { erro: bool, texto: String },
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
            "conferir_conta" => Passo::ConferirConta,
            "conferir_selecao" => Passo::ConferirSelecao(numero(0)? as usize),
            "conversa" => Passo::Conversa(numero(0)? as usize),
            "conferir_chatbot" => Passo::ConferirChatbot(numero(0)? as usize),
            "conferir_novidades" => Passo::ConferirNovidades(numero(0)? as usize),
            "conferir_agenda" => Passo::ConferirAgenda(numero(0)? as usize),
            "menu" => Passo::Menu,
            "filtros" => Passo::Filtros,
            "graficos" => Passo::Graficos,
            "formas_do_caixa" => Passo::FormasDoCaixa,
            "buscar" => Passo::Buscar(argumentos.join(" ")),
            "periodo" => match argumentos.as_slice() {
                [de, ate] => Passo::Periodo(Some((de.to_string(), ate.to_string()))),
                _ => Passo::Periodo(None),
            },
            "dados_do_cliente" => Passo::DadosDoCliente,
            "negociar" => Passo::Negociar(argumentos.join(" ")),
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
            "editor" => Passo::Editor(argumentos.join(" ")),
            "guias" => Passo::Guias(argumentos.join(" ")),
            "importar_modal" => Passo::ImportarModal,
            "importar_origem" => Passo::ImportarOrigem,
            "janela" => Passo::Janela(argumentos.join(" ")),
            "nova" => Passo::Nova(argumentos.join(" ")),
            "importar" => Passo::Importar(argumentos.join(" ")),
            "tecla_real" => Passo::TeclaReal {
                codigo: numero(0)? as u16,
                modificadores: modificadores(argumentos.get(1..).unwrap_or_default()),
            },
            "mouse_real" | "mouse_janela" => Passo::MouseReal {
                tipo: argumentos.first().copied().unwrap_or_default().to_string(),
                x: numero(1)?,
                y: numero(2)?,
                modificadores: modificadores(argumentos.get(3..).unwrap_or_default()),
                na_janela: comando == "mouse_janela",
            },
            "tecla" => Passo::Tecla(argumentos.first().copied().unwrap_or_default().to_string()),
            "pinca" => Passo::Pinca {
                delta: numero(0)?,
                x: numero(1)?,
                y: numero(2)?,
            },
            "rajada" => {
                let vezes = numero(0)? as usize;
                let intervalo = Duration::from_millis(numero(1)? as u64);
                let resto = argumentos.get(2..).unwrap_or_default().join(" ");
                let mut dentro =
                    ler_roteiro(&resto).map_err(|e| format!("linha {}: rajada: {e}", i + 1))?;
                let Some(passo) = dentro.pop().filter(|_| dentro.is_empty()) else {
                    return Err(format!("linha {}: rajada precisa de um passo", i + 1));
                };
                Passo::Rajada {
                    vezes,
                    intervalo,
                    passo: Box::new(passo),
                }
            }
            "varrer" => Passo::Varrer {
                vezes: numero(0)? as usize,
                intervalo: Duration::from_millis(numero(1)? as u64),
                de: (numero(2)?, numero(3)?),
                ate: (numero(4)?, numero(5)?),
            },
            "rolar" => Passo::Rolar {
                vezes: numero(0)? as usize,
                intervalo: Duration::from_millis(numero(1)? as u64),
                em: (numero(2)?, numero(3)?),
                dy: numero(4)?,
            },
            "desempenho" => Passo::Desempenho(argumentos.join(" ")),
            "aviso" => Passo::Aviso {
                erro: argumentos.first() == Some(&"erro"),
                texto: argumentos.get(1..).unwrap_or_default().join(" "),
            },
            "fim" => Passo::Fim,
            outro => return Err(format!("linha {}: passo desconhecido: '{outro}'", i + 1)),
        };
        passos.push(passo);
    }
    Ok(passos)
}

/// Os `NSEventModifierFlags` pelos nomes do roteiro.
fn modificadores(nomes: &[&str]) -> usize {
    nomes
        .iter()
        .map(|n| match *n {
            "shift" => 1 << 17,
            "ctrl" => 1 << 18,
            "alt" => 1 << 19,
            "cmd" => 1 << 20,
            _ => 0,
        })
        .fold(0, |a, b| a | b)
}

fn nome_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Grava em `destino` (PNG) o que a janela mostra agora.
#[cfg(target_os = "macos")]
pub fn fotografar(window: &gpui_kit::Window, destino: &Path) -> Result<(), String> {
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

/// O `NSView` do GPUI desta janela.
#[cfg(target_os = "macos")]
fn vista_da(window: &gpui_kit::Window) -> Result<*mut std::ffi::c_void, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let alca = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(appkit) = alca.as_raw() else {
        return Err("a janela não é do AppKit".into());
    };
    Ok(appkit.ns_view.as_ptr())
}

/// Põe na fila do app uma tecla física (desce e sobe) — ver [`Passo::TeclaReal`].
#[cfg(target_os = "macos")]
pub fn tecla_nativa(window: &gpui_kit::Window, codigo: u16, mods: usize) -> Result<(), String> {
    let vista = vista_da(window)?;
    unsafe { mac::tecla(vista, codigo, mods) }
}

/// Põe na fila do app um evento do botão esquerdo em `(x, y)`, em pontos da
/// janela com a origem no alto — ver [`Passo::MouseReal`].
#[cfg(target_os = "macos")]
pub fn mouse_nativo(
    window: &gpui_kit::Window,
    tipo: &str,
    x: f32,
    y: f32,
    mods: usize,
) -> Result<(), String> {
    let vista = vista_da(window)?;
    unsafe { mac::mouse(vista, tipo, x as f64, y as f64, mods) }
}

#[cfg(not(target_os = "macos"))]
pub fn tecla_nativa(_w: &gpui_kit::Window, _c: u16, _m: usize) -> Result<(), String> {
    Err("só no macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn mouse_nativo(
    _w: &gpui_kit::Window,
    _t: &str,
    _x: f32,
    _y: f32,
    _m: usize,
) -> Result<(), String> {
    Err("só no macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn fotografar(_window: &gpui_kit::Window, _destino: &Path) -> Result<(), String> {
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

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }

    unsafe impl objc2::encode::Encode for NSPoint {
        const ENCODING: objc2::encode::Encoding =
            objc2::encode::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }

    use objc2::runtime::AnyClass;

    const TECLA_DESCE: usize = 10;
    const TECLA_SOBE: usize = 11;

    unsafe fn classe(nome: &std::ffi::CStr) -> Result<&'static AnyClass, String> {
        AnyClass::get(nome).ok_or_else(|| format!("classe {nome:?} ausente"))
    }

    unsafe fn texto(s: &str) -> Result<*mut AnyObject, String> {
        let c = std::ffi::CString::new(s).map_err(|e| e.to_string())?;
        let t: *mut AnyObject = msg_send![classe(c"NSString")?, stringWithUTF8String: c.as_ptr()];
        Ok(t)
    }

    unsafe fn postar(evento: *mut AnyObject) -> Result<(), String> {
        if evento.is_null() {
            return Err("o NSEvent não foi criado".into());
        }
        let app: *mut AnyObject = msg_send![classe(c"NSApplication")?, sharedApplication];
        let _: () = msg_send![app, postEvent: evento, atStart: false];
        Ok(())
    }

    pub unsafe fn tecla(vista: *mut c_void, codigo: u16, flags: usize) -> Result<(), String> {
        let vista = vista as *mut AnyObject;
        let janela: *mut AnyObject = msg_send![vista, window];
        let numero: isize = msg_send![janela, windowNumber];
        // Os caracteres do evento não importam ao GPUI: ele traduz o keyCode
        // pelo layout ativo (`chars_for_modified_key`). Vão vazios de propósito.
        let vazio = texto("")?;
        for tipo in [TECLA_DESCE, TECLA_SOBE] {
            let evento: *mut AnyObject = msg_send![
                classe(c"NSEvent")?,
                keyEventWithType: tipo,
                location: NSPoint { x: 0.0, y: 0.0 },
                modifierFlags: flags,
                timestamp: 0.0f64,
                windowNumber: numero,
                context: std::ptr::null_mut::<AnyObject>(),
                characters: vazio,
                charactersIgnoringModifiers: vazio,
                isARepeat: false,
                keyCode: codigo
            ];
            postar(evento)?;
        }
        Ok(())
    }

    pub unsafe fn mouse(
        vista: *mut c_void,
        tipo: &str,
        x: f64,
        y: f64,
        flags: usize,
    ) -> Result<(), String> {
        let vista = vista as *mut AnyObject;
        let janela: *mut AnyObject = msg_send![vista, window];
        let numero: isize = msg_send![janela, windowNumber];
        let conteudo: *mut AnyObject = msg_send![janela, contentView];
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSRect {
            origem: NSPoint,
            tamanho: NSPoint,
        }
        unsafe impl objc2::encode::Encode for NSRect {
            const ENCODING: objc2::encode::Encoding = objc2::encode::Encoding::Struct(
                "CGRect",
                &[
                    NSPoint::ENCODING,
                    objc2::encode::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]),
                ],
            );
        }
        let quadro: NSRect = msg_send![conteudo, frame];

        // O AppKit conta y de baixo para cima.
        let ponto = NSPoint {
            x,
            y: quadro.tamanho.y - y,
        };
        let tipos: &[usize] = match tipo {
            "apertar" => &[1],
            "soltar" => &[2],
            "arrastar" => &[6],
            "mover" => &[5],
            "clicar" => &[1, 2],
            "duplo" => &[1, 2, 1, 2],
            // O botão direito: `NSEventTypeRightMouseDown` e `…Up`.
            "direito" => &[3, 4],
            outro => return Err(format!("mouse_real: tipo desconhecido '{outro}'")),
        };
        for (i, t) in tipos.iter().enumerate() {
            let cliques: isize = if tipo == "duplo" && i >= 2 { 2 } else { 1 };
            let evento: *mut AnyObject = msg_send![
                classe(c"NSEvent")?,
                mouseEventWithType: *t,
                location: ponto,
                modifierFlags: flags,
                timestamp: 0.0f64,
                windowNumber: numero,
                context: std::ptr::null_mut::<AnyObject>(),
                eventNumber: 0isize,
                clickCount: cliques,
                pressure: if *t == 2 || *t == 4 || *t == 5 { 0.0f32 } else { 1.0f32 }
            ];
            // 🔑 O hover não passa pela fila: a janela normal do GPUI desliga
            // `acceptsMouseMovedEvents` e ouve a área de rastreamento, que só
            // reage ao ponteiro físico. O evento vai direto ao `mouseMoved:` da
            // view — o mesmo método que a área de rastreamento chama.
            if *t == 5 {
                if evento.is_null() {
                    return Err("o NSEvent não foi criado".into());
                }
                // No macOS o GPUI só dá hover à janela ativa
                // (`is_window_hovered` = `is_window_active`).
                let app: *mut AnyObject = msg_send![classe(c"NSApplication")?, sharedApplication];
                let _: () = msg_send![app, activateIgnoringOtherApps: true];
                let _: () =
                    msg_send![janela, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
                let _: () = msg_send![vista, mouseMoved: evento];
                continue;
            }
            postar(evento)?;
        }
        Ok(())
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

    /// A rajada é o passo do fim da linha, N vezes com o intervalo.
    #[test]
    fn a_rajada_repete_o_passo_do_fim_da_linha() {
        let passos =
            ler_roteiro("rajada 200 25 tecla right\nrajada 3 16 revelacao varrer 0").unwrap();
        assert_eq!(
            passos,
            vec![
                Passo::Rajada {
                    vezes: 200,
                    intervalo: Duration::from_millis(25),
                    passo: Box::new(Passo::Tecla("right".into())),
                },
                Passo::Rajada {
                    vezes: 3,
                    intervalo: Duration::from_millis(16),
                    passo: Box::new(Passo::Revelacao("varrer 0".into())),
                },
            ]
        );
        assert!(ler_roteiro("rajada 2 10").is_err(), "sem passo é erro");
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
             importar_origem\n\
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
                Passo::ImportarOrigem,
                Passo::Fim,
            ]
        );
        assert_eq!(
            ler_roteiro("conferir_selecao 2").unwrap(),
            vec![Passo::ConferirSelecao(2)]
        );
        assert_eq!(
            ler_roteiro("conferir_conta").unwrap(),
            vec![Passo::ConferirConta]
        );
        assert_eq!(
            ler_roteiro(
                "ir chatbot\nconferir_chatbot 1\nconversa 2\nconferir_novidades 1\nir agenda\nconferir_agenda 3"
            )
            .unwrap(),
            vec![
                Passo::Ir("chatbot".into()),
                Passo::ConferirChatbot(1),
                Passo::Conversa(2),
                Passo::ConferirNovidades(1),
                Passo::Ir("agenda".into()),
                Passo::ConferirAgenda(3),
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

/// 🐕 **O vigia de travamento** (`VLB_VIGIA=1`) — mede quanto tempo a thread
/// da interface fica sem responder (dono, 2026-09-22: *"a aplicação parou de
/// responder durante o uso, então precisamos simular uma carga mais intensa"*).
///
/// A thread da interface bate a cada 16 ms ([`bater`]); uma thread à parte olha
/// a cada 10 ms e, quando a batida some por mais de [`LIMIAR`], anota a trava —
/// a duração e o passo do roteiro em que ela aconteceu. [`relatar`] imprime o
/// resumo. Não custa nada com a variável desligada.
pub mod vigia {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Acima disto, a interface "travou" — o operador já sente.
    pub const LIMIAR: Duration = Duration::from_millis(150);

    static LIGADO: AtomicBool = AtomicBool::new(false);
    static BATIDA: AtomicU64 = AtomicU64::new(0);
    static PASSO: Mutex<String> = Mutex::new(String::new());
    static TRAVAS: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());
    static COMECO: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

    fn agora_ms() -> u64 {
        COMECO.get_or_init(Instant::now).elapsed().as_millis() as u64
    }

    pub fn ligado() -> bool {
        LIGADO.load(Ordering::Relaxed)
    }

    /// Liga, se `VLB_VIGIA` estiver definida. Chamar uma vez.
    pub fn ligar() {
        if std::env::var_os("VLB_VIGIA").is_none() || LIGADO.swap(true, Ordering::SeqCst) {
            return;
        }
        BATIDA.store(agora_ms(), Ordering::SeqCst);
        std::thread::Builder::new()
            .name("vigia".into())
            .spawn(|| {
                let mut travada_desde: Option<u64> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(10));
                    let agora = agora_ms();
                    let ultima = BATIDA.load(Ordering::SeqCst);
                    let parada = agora.saturating_sub(ultima);
                    match travada_desde {
                        None if parada > LIMIAR.as_millis() as u64 => travada_desde = Some(ultima),
                        Some(desde) if parada < 40 => {
                            let durou = ultima.saturating_sub(desde);
                            let passo = PASSO.lock().map(|p| p.clone()).unwrap_or_default();
                            eprintln!("[vigia] TRAVOU {durou} ms (em {desde} ms) — passo: {passo}");
                            if let Ok(mut t) = TRAVAS.lock() {
                                t.push((durou, passo));
                            }
                            travada_desde = None;
                        }
                        _ => {}
                    }
                }
            })
            .expect("abrir a thread do vigia");
    }

    /// A batida da thread da interface.
    pub fn bater() {
        BATIDA.store(agora_ms(), Ordering::SeqCst);
    }

    /// O passo do roteiro que está rodando — para dizer onde travou.
    pub fn passo(nome: &str) {
        if let Ok(mut p) = PASSO.lock() {
            *p = nome.to_string();
        }
    }

    /// Um trecho que passou do orçamento de um quadro, com nome.
    pub fn cronometrar(nome: &str, inicio: Instant) {
        if !ligado() {
            return;
        }
        let gasto = inicio.elapsed();
        if gasto > Duration::from_millis(16) {
            eprintln!("[vigia] lento: {nome} levou {} ms", gasto.as_millis());
        }
    }

    /// O resumo: quantas travas, a pior, o total e as dez maiores.
    pub fn relatar() {
        if !ligado() {
            return;
        }
        let mut t = TRAVAS.lock().map(|t| t.clone()).unwrap_or_default();
        t.sort_by_key(|(ms, _)| std::cmp::Reverse(*ms));
        let total: u64 = t.iter().map(|(ms, _)| ms).sum();
        eprintln!(
            "[vigia] RESUMO: {} travas acima de {} ms, a pior {} ms, {} ms parado no total",
            t.len(),
            LIMIAR.as_millis(),
            t.first().map_or(0, |(ms, _)| *ms),
            total
        );
        for (ms, passo) in t.iter().take(10) {
            eprintln!("[vigia]   {ms:>6} ms — {passo}");
        }
    }
}
