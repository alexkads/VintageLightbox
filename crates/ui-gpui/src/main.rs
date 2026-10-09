// 🚨 **No Windows, um app de janela não abre console** (dono, 18/set/2026:
// *"quando abro a aplicação no Windows, também abre um terminal, e quando fecho
// o terminal a aplicação fecha"*). O padrão do `rustc` é o subsistema
// `console`: o Windows cria uma janela de terminal para o processo, ela fica
// atrás do app, e fechá-la mata o processo inteiro — o operador do balcão
// perde a sessão por ter fechado o que parecia uma janela solta.
//
// ⚠️ **Só fora do `debug_assertions`.** Em desenvolvimento o console é onde
// saem os `eprintln!` do roteiro, da sonda e do barramento; sem ele, conferir
// qualquer coisa na máquina de trabalho passaria a exigir arquivo de log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! O app em GPUI, ao lado do de egui.
//!
//! `cargo run -p ui-gpui` — e `cargo run -p ui` continua abrindo o de sempre.
//! Os dois leem o mesmo catálogo, ou catálogos separados via `VLB_CATALOG`
//! enquanto este aqui está em obras.

use std::sync::Arc;

use gpui_kit::component::Root;
use gpui_kit::{px, size, AnyWindowHandle, App, AppContext, Bounds, WindowBounds, WindowOptions};
use infrastructure::paths::AppPaths;

use ui_gpui::app::Aplicativo;
use ui_gpui::segundo_plano::ReabrirDaBandeja;
use ui_gpui::tema;

/// Onde a sessão do site dorme: o chaveiro do sistema.
///
/// 🔧 Em depuração, `VLB_SESSAO_EM_ARQUIVO=1` troca o chaveiro por um arquivo
/// (`ui_gpui::depuracao::CofreEmArquivo`), para um roteiro não parar no diálogo
/// do macOS a cada recompilação. O binário do balcão nem olha a variável.
fn cofre_da_sessao(pilha_local: bool) -> Arc<dyn domain::services::pos_venda::CofreDeSessao> {
    if ui_gpui::depuracao::ferramentas_ligadas()
        && std::env::var_os("VLB_SESSAO_EM_ARQUIVO").is_some()
    {
        return Arc::new(ui_gpui::depuracao::CofreEmArquivo::padrao());
    }
    // 🚨 **A pilha local nunca usa o item de produção do chaveiro.** Eram o
    // mesmo item, e autorizar contra `localhost` gravava o token local em cima
    // da sessão do estúdio — o operador voltava a produção deslogado, sem
    // relação visível com o que tinha feito. A pilha local tem a própria
    // sessão, e é por isso que o serviço do chaveiro é outro.
    if pilha_local {
        return Arc::new(infrastructure::CofreDoSistema::com_servico(
            "br.com.recordarfotos.vintagelightbox.local",
        ));
    }
    Arc::new(infrastructure::CofreDoSistema::novo())
}

#[tokio::main]
async fn main() {
    // 🔄 `--versao`: responde a versão e sai, **antes** de janela, catálogo,
    // rede ou a trava de uma cópia só. É a prova de vida que o instalador pede
    // ao binário novo antes de trocar o instalado, e a conferência que a
    // atualização automática faz no fim (`atualizacao::compilar`). Um binário
    // que não chega aqui não substitui o que funciona.
    if std::env::args().skip(1).any(|a| a == "--versao") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // 💾 `--recuperar <dispositivo> <destino> [tamanho]`: a varredura do
    // cartão formatado, no processo que o `pkexec`/`osascript`/UAC abriu como
    // administrador (`ui_gpui::recuperacao::elevar`). Sai antes de tudo pelo
    // mesmo motivo do `--versao`, e por mais um: este processo roda como root,
    // e nada do catálogo, da janela ou do chaveiro pode nascer de root.
    {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--recuperar") {
            std::process::exit(infrastructure::recuperacao::rodar_pela_linha_de_comando(
                &args,
            ));
        }
    }
    // 📊 `--comparar-apis <arquivo>`: a comparação das APIs gráficas que a
    // janela Desempenho pede (`desempenho::comparacao`). Num processo à parte
    // porque abre drivers que o app não usa, e um driver ruim derruba quem o
    // carregou; sai antes da janela e do catálogo, como o `--recuperar`.
    {
        let args: Vec<String> = std::env::args().collect();
        if args
            .iter()
            .any(|a| a == ui_gpui::desempenho::comparacao::ARGUMENTO)
        {
            std::process::exit(ui_gpui::desempenho::comparacao::rodar_pela_linha_de_comando(&args));
        }
    }
    // 🧯 O panic vai para o depósito **antes** de qualquer coisa — janela,
    // catálogo, rede. Um panic na abertura também fica registrado, e sobe
    // quando a conta entrar (`telemetria`).
    ui_gpui::telemetria::instalar_o_gancho_de_panico();
    // Onde está rodando — no Linux, a área de trabalho muda quem desenha a
    // barra das janelas (`janela::app_desenha_a_barra`).
    if cfg!(target_os = "linux") {
        eprintln!(
            "🖥️  Área de trabalho: {}",
            ui_gpui::janela::AreaDeTrabalho::atual().descricao()
        );
    }

    // O mesmo preâmbulo do `crates/ui`, e de propósito: os dois apps abrem o
    // mesmo catálogo e rodam as mesmas migrations. Se este divergisse daquele,
    // a comparação de paridade da fase 5 mediria dois bancos diferentes.
    let catalogo = AppPaths::catalog_root();
    if !catalogo.exists() {
        std::fs::create_dir_all(&catalogo).expect("Failed to create catalog directory");
    }

    // 🔒 **Uma cópia só por catálogo** (`ui_gpui::copia_unica`), antes do
    // banco: a segunda abertura acorda a janela da primeira e sai sem tocar no
    // SQLite. Em desenvolvimento, `VLB_VARIAS_COPIAS=1` libera.
    let mut trava = match ui_gpui::copia_unica::abrir(&catalogo) {
        ui_gpui::copia_unica::Abertura::Dona(trava) => Some(trava),
        ui_gpui::copia_unica::Abertura::SemTrava => None,
        ui_gpui::copia_unica::Abertura::JaAberta => {
            eprintln!("🔒 O VintageLightbox já está aberto: a janela dele vem para a frente.");
            return;
        }
    };

    // 🔑 O chaveiro do sistema é o que faz a sessão sobreviver ao
    // fechamento do app: sem ele, o refresh de quinze dias morreria com
    // o processo e o operador reautorizaria toda manhã.
    // 🚨 **Sem `com_site` a autorização ia sempre para produção.**
    // `PosVendaApiHttp::nova` cai em `SITE_PADRAO` quando ninguém lhe
    // diz o site, então com a API em `localhost` o navegador abria
    // `recordarfotos.com.br/autorizar-app` — que é exatamente a
    // armadilha descrita em `pos_venda/config.rs`: o código é assinado
    // pelo segredo de um servidor e apresentado a outro. `config.site()`
    // já resolve a precedência (`VLB_SITE_URL`, o JSON, a dedução).
    //
    // 🔑 **Um cliente só, e por isso ele nasce aqui fora.** O acervo do backup
    // fala com a mesma API e precisa do mesmo token; dois clientes seriam dois
    // lugares onde a base e a renovação podem divergir — e o sintoma seria uma
    // das telas deslogando sozinha.
    let api_do_site = {
        let config = ui_gpui::pos_venda::config::ler();
        let local =
            config.base_url.contains("://localhost") || config.base_url.contains("://127.0.0.1");
        Arc::new(
            infrastructure::PosVendaApiHttp::nova(config.base_url.clone())
                .com_site(config.site())
                .com_cofre(cofre_da_sessao(local)),
        )
    };
    let ui_gpui::montagem::Montagem {
        pool,
        fotos,
        presets,
        previews,
        edicoes,
        recuperador,
        portas,
    } = ui_gpui::montagem::montar(ui_gpui::montagem::Ambiente {
        catalogo: catalogo.clone(),
        banco: AppPaths::main_db_path(),
        previews: AppPaths::preview_cache_dir(),
        api: api_do_site.clone(),
    })
    .await;
    // 📡 O app falando de si com o servidor: o aviso de versão por SSE e os
    // relatos de panic e erro. Mesmo cliente, mesmo token.
    ui_gpui::telemetria::ligar(api_do_site.clone(), tokio::runtime::Handle::current());
    // ⏱️ A ferramenta de desempenho grava as sessões no mesmo SQLite do
    // catálogo, em segundo plano (`desempenho::porta`).
    ui_gpui::desempenho::painel::instalar(Arc::new(
        ui_gpui::desempenho::porta::DepositoNoBanco::novo(
            Arc::new(infrastructure::SqliteDesempenho::new(pool.clone())),
            tokio::runtime::Handle::current(),
        ),
    ));

    gpui_kit::application()
        .with_assets(ui_gpui::recursos::Recursos)
        // Com o app na bandeja, o ícone do Dock (ou abri-lo de novo) traz a janela.
        .reabrir_da_bandeja()
        .run(move |cx: &mut App| {
            // Antes de qualquer janela: é o `init` que cria o `Theme` global, o
            // registro de temas e os estados globais de campo de texto, menu,
            // diálogo e lista. Sem ele, o primeiro componente do `gpui-component`
            // que a tela usar entra num `cx.global::<...>()` que não existe.
            gpui_kit::init(cx);
            // Devolve à GPU as texturas das imagens que saíram de uso — sem
            // isso a memória de vídeo só cresce (`imagem::coleta`).
            ui_gpui::imagem::coleta::ligar(cx);
            // 🎨 As fontes do template, antes do tema que as nomeia.
            tema::fontes::registrar(cx);
            // E logo em seguida o tema do site, no modo que o operador escolheu
            // (Claro, Escuro ou Sistema, no menu da conta).
            // 🔠 O tamanho da letra do `Cmd +`/`Cmd −`, antes do tema que o usa.
            tema::letra::carregar();
            tema::aplicar(
                tema::escolha_guardada(&tema::arquivo_da_escolha()),
                None,
                cx,
            );
            // As teclas da raiz — hoje só o `Esc` que sai da Revelação.
            ui_gpui::app::init(cx);
            // As cinco teclas do modal de importação — em contexto próprio, para não
            // roubarem Enter e espaço de quem estiver atrás.
            ui_gpui::importacao::tela::init(cx);
            // E as duas da segunda tela — em contexto próprio, senão o `Esc` dela
            // e o da janela principal seriam a mesma ligação em janelas diferentes.
            ui_gpui::cliente::init(cx);
            // O menu do app no macOS.
            ui_gpui::menu::instalar(cx);

            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            let principal = cx
                .open_window(
                    WindowOptions {
                        app_id: Some(ui_gpui::menu::APP_ID.into()),
                        // 🖥️ **Abre maximizada** (dono, 22/set/2026: *"o gnome
                        // ainda não tem barra pra fazer isso"*). `bounds` fica
                        // como o tamanho de restaurar. Só o Windows atende este
                        // pedido no GPUI 0.2.2; os outros são maximizados logo
                        // abaixo, depois de a janela existir.
                        window_bounds: Some(WindowBounds::Maximized(bounds)),
                        titlebar: Some(gpui_kit::TitlebarOptions {
                            title: Some(ui_gpui::menu::NOME.into()),
                            ..Default::default()
                        }),
                        is_resizable: true,
                        // No Linux, pede a barra desenhada pelo app — sem
                        // isto o GNOME fica sem barra nenhuma
                        // (`janela::decoracoes_ao_abrir`).
                        window_decorations: ui_gpui::janela::decoracoes_ao_abrir(),
                        ..Default::default()
                    },
                    |window, cx| {
                        let aplicativo = cx.new(|cx| {
                            Aplicativo::novo(
                                fotos.clone(),
                                previews.clone(),
                                presets.clone(),
                                portas.clone(),
                                window,
                                cx,
                            )
                        });
                        // 🖌️ A Revelação resolve a imagem editada pela mesma porta.
                        aplicativo.update(cx, |app, cx| app.definir_edicoes(edicoes.clone(), cx));
                        aplicativo.update(cx, |app, cx| {
                            app.ligar_recuperacao(recuperador.clone(), portas.seletor.clone(), cx)
                        });
                        // Minimizar leva à bandeja; fechar com envio na fila só
                        // esconde (G9) — `ui_gpui::segundo_plano`.
                        ui_gpui::segundo_plano::ligar(aplicativo.downgrade(), window, cx);
                        // E a segunda cópia, quando abrirem de novo, também.
                        if let Some(trava) = trava.take() {
                            ui_gpui::copia_unica::ligar(trava, cx);
                        }
                        // No Linux (Wayland e X11) e no macOS o GPUI ignora o
                        // `Maximized` da abertura e só o informa depois: quem
                        // maximiza é o app. No Wayland o pedido vai antes do
                        // primeiro quadro, e a janela já nasce do tamanho da tela.
                        if !window.is_maximized() {
                            window.zoom_window();
                        }
                        // A primeira camada da janela **tem** de ser o `Root`: é ele
                        // que hospeda diálogo, gaveta e aviso, e quem sabe qual campo
                        // de texto está com o foco. O `gpui-component` procura por ele
                        // com um `expect` — sem o `Root`, abrir um diálogo derruba o
                        // app em vez de mostrar o diálogo.
                        cx.new(|cx| Root::new(aplicativo, window, cx))
                    },
                )
                .expect("abrir a janela");

            // Fechar a janela principal encerra o app. Sem isto o macOS mantém o
            // processo vivo com o ícone na Dock e nenhuma janela — e como não
            // registramos menu de aplicativo, não sobra nem Cmd+Q: só resta matar o
            // processo. O porquê do critério ser a janela principal, e não "sobrou
            // alguma janela", está em `ui_gpui::encerramento`.
            let principal = AnyWindowHandle::from(principal);
            cx.on_window_closed(move |cx, _janela| {
                if ui_gpui::encerramento::deve_encerrar(&principal, &cx.windows()) {
                    cx.quit();
                }
            })
            .detach();

            cx.activate(true);
        });
}
