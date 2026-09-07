//! O app em GPUI, ao lado do de egui.
//!
//! `cargo run -p ui-gpui` — e `cargo run -p ui` continua abrindo o de sempre.
//! Os dois leem o mesmo catálogo, ou catálogos separados via `VLB_CATALOG`
//! enquanto este aqui está em obras.

use std::sync::Arc;

use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
use gpui_component::Root;
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;

use ui_gpui::app::{Aplicativo, Portas};
use ui_gpui::biblioteca::acervo::{Acervo, AcervoDoBanco};
use ui_gpui::biblioteca::colecoes::{Colecoes, ColecoesDoBanco};
use ui_gpui::biblioteca::marcacao::{Marcador, MarcadorDoBanco};
use ui_gpui::exportacao::porta::{Exportador, ExportadorDoBanco};
use ui_gpui::importacao::explorador::{
    Explorador, ExploradorDoDisco, GeradorDeMiniaturas, GeradorDoDisco, Importador,
    ImportadorDoDisco, SeletorDePasta, SeletorNativo,
};
use ui_gpui::pos_venda::porta::{Publicador, PublicadorDaApi};
use ui_gpui::revelacao::persistencia::{Gravador, GravadorDoBanco};
use ui_gpui::revelacao::presets::{GuardaDePresets, GuardaDoBanco};
use ui_gpui::tema;

#[tokio::main]
async fn main() {
    // O mesmo preâmbulo do `crates/ui`, e de propósito: os dois apps abrem o
    // mesmo catálogo e rodam as mesmas migrations. Se este divergisse daquele,
    // a comparação de paridade da fase 5 mediria dois bancos diferentes.
    let catalogo = AppPaths::catalog_root();
    if !catalogo.exists() {
        std::fs::create_dir_all(&catalogo).expect("Failed to create catalog directory");
    }

    let db_path = AppPaths::main_db_path();
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    let pool = infrastructure::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    infrastructure::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    // As quatro camadas internas, intactas — este app fala com elas do mesmo
    // jeito que o de egui fala. É o que tornou o GPUI mais barato que o Tauri:
    // nada precisou virar comando serializável.
    let repositorio_de_fotos = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let biblioteca = Arc::new(adapters::controllers::LibraryController::new(
        repositorio_de_fotos.clone(),
    ));
    let editor = Arc::new(adapters::controllers::EditorController::new(Arc::new(
        use_cases::SavePhotoEditsUseCase::new(repositorio_de_fotos.clone()),
    )));

    // As fotos são carregadas **antes** da janela, e isso é provisório: num
    // acervo grande a abertura fica esperando o banco. A fase 1 termina com
    // isso assíncrono — mas o carregamento síncrono é o que permite medir os
    // 60fps da rolagem sem confundir com o tempo de abertura.
    let fotos = biblioteca
        .get_all_photos()
        .await
        .expect("ler as fotos do catálogo");

    // Os presets, uma vez só: são cinco de sistema construídos no use case mais
    // os do usuário na tabela `presets`. Carregar aqui, junto com as fotos, é o
    // mesmo caminho do legado (`load_presets` no primeiro quadro) — e evita que a
    // Revelação precise saber falar com o banco.
    let presets_repo = Arc::new(infrastructure::SqlitePresetRepository::new(pool.clone()));
    let controlador_de_presets = Arc::new(adapters::controllers::PresetController::new(
        Arc::new(use_cases::presets::ListPresetsUseCase::new(
            presets_repo.clone(),
        )),
        Arc::new(use_cases::presets::SavePresetUseCase::new(
            presets_repo.clone(),
        )),
        Arc::new(use_cases::presets::RenamePresetUseCase::new(
            presets_repo.clone(),
        )),
        Arc::new(use_cases::presets::DeletePresetUseCase::new(presets_repo)),
    ));
    let presets = controlador_de_presets
        .list_presets()
        .await
        .unwrap_or_else(|erro| {
            // Sem presets o app abre igual; com um `expect` aqui, um banco velho
            // impediria de revelar qualquer foto.
            eprintln!("⚠️ [Presets] não foi possível carregar: {erro}");
            Vec::new()
        });

    // A importação: seis use cases, um controller. É a mesma montagem do
    // `crates/ui`, linha por linha — nada aqui é novo, só está sendo ligado do
    // outro lado.
    let extrator = Arc::new(infrastructure::ExifReader);
    let miniaturas = Arc::new(infrastructure::ThumbnailGeneratorImpl::new());
    let gerador_de_miniaturas = miniaturas.clone();
    let organizador = Arc::new(infrastructure::FileOrganizerImpl::new(catalogo.clone()));
    let dispositivos =
        Arc::new(infrastructure::devices::repository::InfrastructureDeviceRepository::new());
    let cache_de_previews = Arc::new(PreviewManager::new());

    let importacao = Arc::new(adapters::controllers::ImportController::new(
        Arc::new(use_cases::ImportPhotoUseCase::new(
            repositorio_de_fotos.clone(),
            extrator.clone(),
            miniaturas.clone(),
            cache_de_previews.clone(),
        )),
        Arc::new(use_cases::CheckDuplicatesUseCase::new(
            repositorio_de_fotos.clone(),
        )),
        Arc::new(use_cases::ImportWithOptionsUseCase::new(
            repositorio_de_fotos.clone(),
            extrator.clone(),
            miniaturas,
            cache_de_previews.clone(),
            organizador,
        )),
        Arc::new(use_cases::GetImportSourcesUseCase::new(dispositivos)),
        Arc::new(use_cases::ScanSourceUseCase::new(Arc::new(
            infrastructure::SourceScannerImpl::new(),
        ))),
        Arc::new(use_cases::DescribeCandidatesUseCase::new(extrator)),
    ));

    // O mesmo cache que a importação usa: duas instâncias apontando para o mesmo
    // diretório seriam dois caches do mesmo arquivo.
    let previews = cache_de_previews;

    // 🚨 O `Handle` é pego **aqui**, e não lá dentro. `Application::run` toma esta
    // thread e o que roda depois está fora do contexto do runtime: um
    // `tokio::spawn` lá dentro entraria em pânico com "there is no reactor
    // running" — no meio de um arrasto de slider, sem relação visível com o que o
    // dedo estava fazendo. Com o `Handle` clonado, as tarefas de gravação vão
    // para as threads do tokio, que continuam vivas.
    let gravador: Arc<dyn Gravador> = Arc::new(GravadorDoBanco::novo(
        editor,
        tokio::runtime::Handle::current(),
    ));
    // 🚨 A releitura do catálogo, pelo mesmo `LibraryController` que leu a lista
    // acima. Sem ela a importação grava no banco e a grade continua com a lista
    // lida antes de a janela existir — as fotos só apareciam ao reabrir o app.
    let acervo: Arc<dyn Acervo> = Arc::new(AcervoDoBanco::novo(
        biblioteca.clone(),
        tokio::runtime::Handle::current(),
    ));
    // 🚨 **A montagem que nunca existiu.** `ExportPhotoUseCase`,
    // `ExportController` e `ImageExporterImpl` estavam escritos e testados desde
    // antes da migração, e este `Arc::new` nunca foi escrito: o app não tinha
    // caminho nenhum até um arquivo no disco. Camada pronta não é funcionalidade
    // entregue — a pergunta é sempre "que clique chega até aqui?".
    let exportador: Arc<dyn Exportador> = Arc::new(ExportadorDoBanco::novo(
        Arc::new(adapters::controllers::ExportController::new(Arc::new(
            use_cases::ExportPhotoUseCase::new(
                repositorio_de_fotos.clone(),
                Arc::new(infrastructure::ImageExporterImpl::new()),
            ),
        ))),
        tokio::runtime::Handle::current(),
    ));

    // 📸 O pós-venda do site — o vão que o projeto existe para fechar. O mesmo
    // exportador da exportação, em memória: o que sobe é o que a tela mostra,
    // e o site gera a prévia marcada a partir dele.
    let publicador: Arc<dyn Publicador> = Arc::new(PublicadorDaApi::novo(
        Arc::new({
            // 🔑 O chaveiro do sistema é o que faz a sessão sobreviver ao
            // fechamento do app: sem ele, o refresh de quinze dias morreria com
            // o processo e o operador reautorizaria toda manhã.
            let api = Arc::new(
                infrastructure::PosVendaApiHttp::nova(ui_gpui::pos_venda::config::ler().base_url)
                    .com_cofre(Arc::new(infrastructure::CofreDoSistema::novo())),
            );
            adapters::controllers::PosVendaController::new(
                api.clone(),
                Arc::new(use_cases::pos_venda::PublicarNoPosVendaUseCase::new(
                    repositorio_de_fotos.clone(),
                    Arc::new(infrastructure::ImageExporterImpl::new()),
                    // O mesmo gerador das miniaturas prepara o que sobe: ele já
                    // abre RAW, TIFF e HEIC, e já reduz para um lado máximo.
                    Arc::new(infrastructure::ThumbnailGeneratorImpl::new()),
                    api,
                )),
            )
        }),
        // 🔑 O mesmo exportador da exportação e da impressão: é ele que revela
        // o original quando o editor salva na galeria, e a foto do cliente não
        // pode depender de qual dos caminhos a produziu.
        Arc::new(infrastructure::ImageExporterImpl::new()),
        tokio::runtime::Handle::current(),
    ));

    // A folha de impressão em PDF. 🔑 Ela reusa o **mesmo** exportador da
    // exportação: a folha tem de sair com a foto revelada e enquadrada, e
    // imprimir o arquivo original seria o defeito que a exportação teve até
    // 17/ago — a tela mostrando uma coisa e o papel saindo outra.
    let folha: Arc<dyn ui_gpui::impressao::porta::Folha> =
        Arc::new(ui_gpui::impressao::porta::FolhaDoDisco::nova(
            repositorio_de_fotos.clone(),
            Arc::new(infrastructure::ImageExporterImpl::new()),
            tokio::runtime::Handle::current(),
        ));

    // As coleções. 🔑 O ensaio de um cliente **é** uma coleção, e é dela que a
    // galeria do site vai sair — por isso ela não é "mais uma forma de
    // organizar": é a estrutura em que a mesma foto pertence a vários lugares
    // sem ser copiada, que é o que pasta não faz.
    let repositorio_de_colecoes =
        Arc::new(infrastructure::CollectionRepositoryImpl::new(pool.clone()));
    let colecoes: Arc<dyn Colecoes> = Arc::new(ColecoesDoBanco::novo(
        Arc::new(adapters::controllers::CollectionController::new(
            repositorio_de_colecoes.clone(),
            Arc::new(use_cases::CreateCollectionUseCase::new(
                repositorio_de_colecoes.clone(),
            )),
            Arc::new(use_cases::AddPhotoToCollectionUseCase::new(
                repositorio_de_colecoes.clone(),
                repositorio_de_fotos.clone(),
            )),
            Arc::new(use_cases::RemovePhotoFromCollectionUseCase::new(
                repositorio_de_colecoes,
            )),
        )),
        tokio::runtime::Handle::current(),
    ));

    // As treze teclas de triagem da Biblioteca. O `PhotoController` junta os
    // quatro use cases de marcação — e o de apagar, que **não** tem tecla aqui:
    // `Delete` existe no legado e leva um caminho próprio (confirmação e
    // remoção do arquivo), que é trabalho próprio e não um atalho a mais.
    let marcador: Arc<dyn Marcador> = Arc::new(MarcadorDoBanco::novo(
        Arc::new(adapters::controllers::PhotoController::new(
            Arc::new(use_cases::RatePhotoUseCase::new(
                repositorio_de_fotos.clone(),
            )),
            Arc::new(use_cases::SetColorLabelUseCase::new(
                repositorio_de_fotos.clone(),
            )),
            Arc::new(use_cases::SetFlagUseCase::new(repositorio_de_fotos.clone())),
            Arc::new(use_cases::DeletePhotoUseCase::new(
                repositorio_de_fotos.clone(),
            )),
            Arc::new(use_cases::MarcarCompradaUseCase::new(
                repositorio_de_fotos.clone(),
            )),
        )),
        tokio::runtime::Handle::current(),
    ));
    let guarda_de_presets: Arc<dyn GuardaDePresets> = Arc::new(GuardaDoBanco::nova(
        controlador_de_presets,
        tokio::runtime::Handle::current(),
    ));
    // A janela do sistema para escolher `.lrtemplate` e `.xmp`. Porta própria,
    // e não um método da guarda: guardar é banco, escolher arquivo é sistema
    // operacional.
    let escolha_de_presets: Arc<dyn ui_gpui::revelacao::lightroom::EscolhaDePresets> = Arc::new(
        ui_gpui::revelacao::lightroom::EscolhaNativa::nova(tokio::runtime::Handle::current()),
    );
    let explorador: Arc<dyn Explorador> = Arc::new(ExploradorDoDisco::novo(
        importacao.clone(),
        tokio::runtime::Handle::current(),
    ));
    let importador: Arc<dyn Importador> = Arc::new(ImportadorDoDisco::novo(
        importacao,
        tokio::runtime::Handle::current(),
    ));
    let seletor: Arc<dyn SeletorDePasta> =
        Arc::new(SeletorNativo::novo(tokio::runtime::Handle::current()));
    let seletor_de_fotos: Arc<dyn ui_gpui::sessoes::arquivos::SeletorDeFotos> = Arc::new(
        ui_gpui::sessoes::arquivos::SeletorDeFotosNativo::novo(tokio::runtime::Handle::current()),
    );
    // 🔑 **A versão que ele compara é a do próprio binário** (`CARGO_PKG_VERSION`,
    // que vem do `[workspace.package]`). Passá-la por outro caminho — um
    // arquivo, uma constante escrita à mão — é como um app acaba se achando
    // desatualizado para sempre, ou nunca.
    //
    // ⚠️ Sem `Handle` do tokio, e é a única porta assim: `check_update` é
    // bloqueante e monta um runtime próprio por dentro. O motivo está em
    // `atualizacao::porta`.
    let atualizador: Arc<dyn ui_gpui::atualizacao::porta::Atualizador> = Arc::new(
        ui_gpui::atualizacao::porta::AtualizadorDaWeb::novo(env!("CARGO_PKG_VERSION")),
    );
    let gerador: Arc<dyn GeradorDeMiniaturas> = Arc::new(GeradorDoDisco::novo(
        gerador_de_miniaturas,
        previews.clone(),
        tokio::runtime::Handle::current(),
    ));

    Application::new().run(move |cx: &mut App| {
        // Antes de qualquer janela: é o `init` que cria o `Theme` global, o
        // registro de temas e os estados globais de campo de texto, menu,
        // diálogo e lista. Sem ele, o primeiro componente do `gpui-component`
        // que a tela usar entra num `cx.global::<...>()` que não existe.
        gpui_component::init(cx);
        // E logo em seguida o nosso tema, porque o `init` deixa o do shadcn
        // ligado e sincronizado com o claro/escuro do sistema.
        tema::aplicar(cx);
        // As teclas da raiz — hoje só o `Esc` que sai da Revelação.
        ui_gpui::app::init(cx);
        // As cinco teclas do modal de importação — em contexto próprio, para não
        // roubarem Enter e espaço de quem estiver atrás.
        ui_gpui::importacao::tela::init(cx);
        // E as duas da segunda tela — em contexto próprio, senão o `Esc` dela
        // e o da janela principal seriam a mesma ligação em janelas diferentes.
        ui_gpui::cliente::init(cx);

        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let aplicativo = cx.new(|cx| {
                    Aplicativo::novo(
                        fotos.clone(),
                        previews.clone(),
                        presets.clone(),
                        Portas {
                            gravador: gravador.clone(),
                            acervo: acervo.clone(),
                            exportador: exportador.clone(),
                            publicador: publicador.clone(),
                            colecoes: colecoes.clone(),
                            folha: folha.clone(),
                            marcador: marcador.clone(),
                            gerador: gerador.clone(),
                            guarda_de_presets: guarda_de_presets.clone(),
                            escolha_de_presets: escolha_de_presets.clone(),
                            explorador: explorador.clone(),
                            importador: importador.clone(),
                            seletor: seletor.clone(),
                            // A janela **do sistema** para escolher as fotos da
                            // sessão: filtro de imagem e seleção múltipla.
                            seletor_de_fotos: seletor_de_fotos.clone(),
                            atualizador: atualizador.clone(),
                        },
                        window,
                        cx,
                    )
                });
                // A primeira camada da janela **tem** de ser o `Root`: é ele
                // que hospeda diálogo, gaveta e aviso, e quem sabe qual campo
                // de texto está com o foco. O `gpui-component` procura por ele
                // com um `expect` — sem o `Root`, abrir um diálogo derruba o
                // app em vez de mostrar o diálogo.
                cx.new(|cx| Root::new(aplicativo, window, cx))
            },
        )
        .expect("abrir a janela");
        cx.activate(true);
    });
}
