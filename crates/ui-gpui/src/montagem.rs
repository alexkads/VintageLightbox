//! 🔩 **A montagem do app**: o catálogo, os controllers e as portas, como o
//! balcão os liga.
//!
//! 🔑 **Existe para ser uma só.** O `main.rs` a chama ao abrir, e o e2e do ciclo
//! de vida (`e2e::ciclo_de_vida`) chama a mesma função contra a API de teste. É o
//! passo 5 do CLAUDE.md, que "some em silêncio": um teste com uma montagem própria
//! provaria a montagem dele, e não a que o operador abre.
//!
//! Fica de fora o que é do processo, e não do app: a trava de uma cópia só, o
//! gancho de panic, a telemetria e o painel de desempenho — o `main.rs` os liga.

use std::path::PathBuf;
use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use infrastructure::cache::preview_manager::PreviewManager;
use sqlx::SqlitePool;

use crate::app::Portas;
use crate::biblioteca::acervo::{Acervo, AcervoDoBanco};
use crate::biblioteca::colecoes::{Colecoes, ColecoesDoBanco};
use crate::biblioteca::marcacao::{Marcador, MarcadorDoBanco};
use crate::exportacao::porta::{Exportador, ExportadorDoBanco};
use crate::importacao::explorador::{
    Explorador, ExploradorDoDisco, GeradorDeMiniaturas, GeradorDoDisco, Importador,
    ImportadorDoDisco, SeletorDePasta, SeletorNativo,
};
use crate::pos_venda::porta::{Publicador, PublicadorDaApi};
use crate::revelacao::persistencia::{Gravador, GravadorDoBanco};
use crate::revelacao::presets::{GuardaDePresets, GuardaDoBanco};

/// Onde o app guarda o que é dele, e com quem ele fala.
pub struct Ambiente {
    /// A pasta do catálogo (`AppPaths::catalog_root()` no balcão).
    pub catalogo: PathBuf,
    /// O SQLite do catálogo (`AppPaths::main_db_path()`).
    pub banco: PathBuf,
    /// O cache das prévias (`AppPaths::preview_cache_dir()`).
    pub previews: PathBuf,
    /// 🔑 **Um cliente só da API**, que nasce fora: o `main.rs` o liga também à
    /// telemetria, e o e2e o aponta para a API de teste.
    pub api: Arc<infrastructure::PosVendaApiHttp>,
}

/// O app montado: o que a janela recebe ao abrir.
pub struct Montagem {
    pub pool: SqlitePool,
    pub fotos: Vec<PhotoViewModel>,
    pub presets: Vec<Preset>,
    pub previews: Arc<PreviewManager>,
    pub edicoes: Arc<crate::editor::porta::EdicoesDoCatalogo>,
    pub recuperador: Arc<dyn crate::recuperacao::porta::Recuperador>,
    pub portas: Portas,
}

/// Abre o catálogo (com as migrations) e liga cada porta à peça de verdade.
pub async fn montar(ambiente: Ambiente) -> Montagem {
    let catalogo = ambiente.catalogo.clone();
    let api_do_site = ambiente.api.clone();
    let db_path = ambiente.banco.clone();
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    let pool = infrastructure::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    infrastructure::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    // As quatro camadas internas, intactas — este app fala com elas do mesmo
    // jeito que o de egui falava. É o que torna trocar de interface barato:
    // nada precisa virar comando serializável.
    let repositorio_de_fotos = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let biblioteca = Arc::new(adapters::controllers::LibraryController::new(
        repositorio_de_fotos.clone(),
    ));
    // 🔑 **Duas metades da mesma porta.** `SavePhotoEditsUseCase` grava a
    // revelação da foto **deste disco**, em `photos`; `RevelacoesLocaisUseCase`
    // guarda a da foto que **só existe no site**, que não tem linha lá — e sem
    // ela cada gesto numa foto do pós-venda se perdia calado (8/set/2026).
    let editor = Arc::new(adapters::controllers::EditorController::new(
        Arc::new(use_cases::SavePhotoEditsUseCase::new(
            repositorio_de_fotos.clone(),
        )),
        Arc::new(use_cases::pos_venda::RevelacoesLocaisUseCase::new(
            Arc::new(infrastructure::SqliteRevelacoesDoSite::new(pool.clone())),
        )),
    ));

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
    let cache_de_previews = Arc::new(PreviewManager::new_with_path(ambiente.previews.clone()));

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
        Arc::new(
            use_cases::ImportWithOptionsUseCase::new(
                repositorio_de_fotos.clone(),
                extrator.clone(),
                miniaturas,
                cache_de_previews.clone(),
                organizador,
            )
            // 🎞️ O DNG revelado no Lightroom (ou o NEF com `.xmp` ao lado) entra
            // com a revelação nos parâmetros da foto.
            .com_revelacao_do_arquivo(Arc::new(
                infrastructure::revelacao_do_arquivo::LeitorDoLightroom,
            )),
        ),
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
    // 📸 **O que ficou por subir, lido uma vez** — como os presets, e pelo mesmo
    // motivo: quem consulta é a grade, no meio de um quadro. São as revelações
    // de fotos que só existem no site e que o operador ainda não salvou na
    // galeria; sem elas, reabrir o app mostrava a foto com a revelação do
    // servidor e o trabalho da véspera sumia.
    let guardadas = editor.revelacoes_do_site().await.unwrap_or_else(|erro| {
        // Sem o depósito o app abre igual, e o que se perde é a revelação não
        // enviada — não a foto. Um `expect` aqui impediria de revelar.
        eprintln!("⚠️ [Revelação] o depósito das fotos do site não abriu: {erro}");
        Vec::new()
    });
    // A Revelação local das fotos do site, lida uma vez como as guardadas.
    let locais_do_site = editor.locais_do_site().await.unwrap_or_else(|erro| {
        eprintln!("⚠️ [Revelação local] o depósito das fotos do site não abriu: {erro}");
        Vec::new()
    });
    // 🖌️ **As edições em camadas** (docs/editor-em-camadas/): o catálogo lido
    // uma vez, com a reconciliação das gravações interrompidas. A mesma porta
    // responde à Revelação, à exportação, à impressão e ao pós-venda — a
    // imagem editada é a entrada de todos eles (C32).
    let edicoes = Arc::new(
        crate::editor::porta::EdicoesDoCatalogo::carregar(
            catalogo.clone(),
            infrastructure::database::CatalogoDeEdicoes::new(pool.clone()),
            tokio::runtime::Handle::current(),
        )
        .await,
    );
    crate::editor::porta::definir_as_do_app(edicoes.clone());
    let editada_da_foto: infrastructure::image_exporter::ImagemEditadaDe = {
        use crate::editor::porta::Edicoes;
        let edicoes = edicoes.clone();
        Arc::new(move |foto: &domain::entities::Photo| {
            edicoes
                .versao_de(&foto.id().to_string(), foto.id_no_site())
                .map(|v| v.arquivo)
        })
    };
    let exportador_de_fotos =
        || infrastructure::ImageExporterImpl::new().com_editadas(editada_da_foto.clone());
    let gravador: Arc<dyn Gravador> = Arc::new(
        GravadorDoBanco::novo(editor, tokio::runtime::Handle::current(), guardadas)
            .com_locais_do_site(locais_do_site)
            .com_historico(infrastructure::database::CatalogoDoHistorico::new(
                pool.clone(),
            )),
    );
    // 🚨 A releitura do catálogo, pelo mesmo `LibraryController` que leu a lista
    // acima. Sem ela a importação grava no banco e a grade continua com a lista
    // lida antes de a janela existir — as fotos só apareciam ao reabrir o app.
    let acervo: Arc<dyn Acervo> = Arc::new(AcervoDoBanco::novo(
        biblioteca.clone(),
        tokio::runtime::Handle::current(),
    ));
    // 📸 O pós-venda do site — o vão que o projeto existe para fechar. O mesmo
    // exportador da exportação, em memória: o que sobe é o que a tela mostra,
    // e o site gera a prévia marcada a partir dele.
    let publicador_da_api = PublicadorDaApi::novo(
        Arc::new({
            let api = api_do_site.clone();
            adapters::controllers::PosVendaController::new(
                api.clone(),
                Arc::new(use_cases::pos_venda::PublicarNoPosVendaUseCase::new(
                    repositorio_de_fotos.clone(),
                    Arc::new(exportador_de_fotos()),
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
        Arc::new(exportador_de_fotos()),
        tokio::runtime::Handle::current(),
    )
    // A revelação local das fotos do site sobe e revela junto: a porta a lê do
    // mesmo depósito que a tela grava.
    .com_locais({
        let gravador = gravador.clone();
        Arc::new(move |foto_no_site: &str| gravador.locais_do_site(foto_no_site))
    })
    // 🖌️ E a imagem editada, quando houver: é ela que se revela (C32).
    .com_editadas({
        use crate::editor::porta::Edicoes;
        let edicoes = edicoes.clone();
        Arc::new(move |foto_no_site: &str| {
            edicoes.versao_de("", Some(foto_no_site)).map(|v| v.arquivo)
        })
    });
    // 🚨 **A montagem que nunca existiu.** `ExportPhotoUseCase`,
    // `ExportController` e `ImageExporterImpl` estavam escritos e testados desde
    // antes da migração, e este `Arc::new` nunca foi escrito: o app não tinha
    // caminho nenhum até um arquivo no disco. Camada pronta não é funcionalidade
    // entregue — a pergunta é sempre "que clique chega até aqui?".
    //
    // 📸 E a foto da sessão, que só existe no site, sai pela **mesma**
    // revelação do "Baixar JPEG" (`RevelacaoDoSite`) — sem ela, exportar uma
    // sessão dava "Photo ID inválido" em todas (03/10/2026).
    let exportador: Arc<dyn Exportador> = Arc::new(ExportadorDoBanco::novo(
        Arc::new(crate::exportacao::porta::CatalogoDoBanco(Arc::new(
            adapters::controllers::ExportController::new(Arc::new(
                use_cases::ExportPhotoUseCase::new(
                    repositorio_de_fotos.clone(),
                    Arc::new(exportador_de_fotos()),
                ),
            )),
        ))),
        Arc::new(publicador_da_api.revelacao_do_site()),
        tokio::runtime::Handle::current(),
    ));
    let publicador: Arc<dyn Publicador> = Arc::new(publicador_da_api);

    // 📦 O acervo de arquivos de `/dashboard/backup`: lista pastas, pede a URL
    // assinada e sobe o arquivo direto no R2. Mesmo cliente, mesmo token.
    let acervo_de_arquivos: Arc<dyn crate::backup::Acervo> = Arc::new(
        crate::backup::AcervoHttp::novo(api_do_site.clone(), tokio::runtime::Handle::current()),
    );

    // 📡 O tempo real do chatbot: os fluxos SSE da API, com o mesmo cliente —
    // e o mesmo token — das outras telas.
    let escuta: Arc<dyn crate::tempo_real::Escuta> = Arc::new(crate::tempo_real::EscutaHttp::nova(
        api_do_site.clone(),
        tokio::runtime::Handle::current(),
    ));
    // 🔔 O aviso do sistema: D-Bus no Linux (funciona sem a bandeja do GNOME),
    // toast no Windows, Central de Notificações no macOS.
    let avisador: Arc<dyn crate::tempo_real::Avisador> =
        Arc::new(crate::tempo_real::AvisoDoSistema::default());

    // 🔑 *"Tinha que ter opção sem arrastar e soltar"* (dono, 2026-09-19): a
    // janela do sistema para escolher pasta ou arquivos do backup.
    let escolha_do_backup: Arc<dyn crate::backup::EscolhaDoBackup> = Arc::new(
        crate::backup::EscolhaNativa::nova(tokio::runtime::Handle::current()),
    );

    // A folha de impressão em PDF. 🔑 Ela reusa o **mesmo** exportador da
    // exportação: a folha tem de sair com a foto revelada e enquadrada, e
    // imprimir o arquivo original seria o defeito que a exportação teve até
    // 17/ago — a tela mostrando uma coisa e o papel saindo outra.
    let folha: Arc<dyn crate::impressao::porta::Folha> =
        Arc::new(crate::impressao::porta::FolhaDoDisco::nova(
            repositorio_de_fotos.clone(),
            Arc::new(exportador_de_fotos()),
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
    let escolha_de_presets: Arc<dyn crate::revelacao::lightroom::EscolhaDePresets> = Arc::new(
        crate::revelacao::lightroom::EscolhaNativa::nova(tokio::runtime::Handle::current()),
    );
    let explorador: Arc<dyn Explorador> = Arc::new(ExploradorDoDisco::novo(
        importacao.clone(),
        tokio::runtime::Handle::current(),
    ));
    let importador: Arc<dyn Importador> = Arc::new(ImportadorDoDisco::novo(
        importacao,
        tokio::runtime::Handle::current(),
    ));
    let seletor: Arc<dyn SeletorDePasta> = Arc::new(SeletorNativo::novo());
    let recuperador: Arc<dyn crate::recuperacao::porta::Recuperador> =
        Arc::new(crate::recuperacao::porta::RecuperadorDoDisco::novo(
            Arc::new(infrastructure::devices::brutos::CartoesDoSistema),
        ));
    let seletor_de_fotos: Arc<dyn crate::sessoes::arquivos::SeletorDeFotos> = Arc::new(
        crate::sessoes::arquivos::SeletorDeFotosNativo::novo(tokio::runtime::Handle::current()),
    );
    // 🔑 **A versão que ele compara é a do próprio binário** (`CARGO_PKG_VERSION`,
    // que vem do `[workspace.package]`). Passá-la por outro caminho — um
    // arquivo, uma constante escrita à mão — é como um app acaba se achando
    // desatualizado para sempre, ou nunca.
    //
    // ⚠️ Sem `Handle` do tokio, e é a única porta assim: `check_update` é
    // bloqueante e monta um runtime próprio por dentro. O motivo está em
    // `atualizacao::porta`.
    let atualizador: Arc<dyn crate::atualizacao::porta::Atualizador> = Arc::new(
        crate::atualizacao::porta::AtualizadorDaWeb::novo(env!("CARGO_PKG_VERSION")),
    );
    let gerador: Arc<dyn GeradorDeMiniaturas> = Arc::new(GeradorDoDisco::novo(
        gerador_de_miniaturas.clone(),
        previews.clone(),
        tokio::runtime::Handle::current(),
    ));
    // 🔑 **O mesmo gerador, outra chave.** O de cima grava sob
    // `import::<caminho>`, para arquivo que ainda não está no catálogo; este
    // grava sob o id da foto, refazendo o que o cache perdeu.
    let repositor: Arc<dyn crate::revelacao::reposicao::Repositor> =
        Arc::new(crate::revelacao::reposicao::RepositorDoDisco::novo(
            gerador_de_miniaturas,
            previews.clone(),
            tokio::runtime::Handle::current(),
        ));

    Montagem {
        pool,
        fotos,
        presets,
        previews,
        edicoes,
        recuperador,
        portas: Portas {
            gravador,
            acervo,
            exportador,
            publicador,
            colecoes,
            folha,
            marcador,
            gerador,
            repositor,
            guarda_de_presets,
            escolha_de_presets,
            explorador,
            importador,
            seletor,
            seletor_de_fotos,
            atualizador,
            acervo_de_arquivos,
            escolha_do_backup,
            escuta,
            avisador,
        },
    }
}
