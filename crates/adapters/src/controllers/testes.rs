//! Testes de caracterização dos controllers: fixam o que cada um faz **hoje**
//! (tradução de entrada da tela, escolha do use case, mensagem de erro), com
//! repositórios em memória e o estado final como prova.

use super::apoio_de_teste::*;
use super::*;
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Collection, Photo, Preset};
use domain::repositories::{CollectionRepository, PhotoRepository, PresetRepository};
use domain::value_objects::{
    CollectionId, ColorLabel, ExportOptions, FilePath, Flag, PhotoId, Rating,
};
use domain::DomainResult;
use std::sync::{Arc, Mutex};
use use_cases::presets::{
    DeletePresetUseCase, ListPresetsUseCase, RenamePresetUseCase, SavePresetUseCase,
};
use use_cases::*;

fn foto(nome: &str) -> Photo {
    Photo::new(FilePath::new(format!("/fotos/{nome}.jpg")).unwrap())
}

// ============================================================
// PhotoController
// ============================================================

fn controller_de_fotos(repo: &Arc<FotosEmMemoria>) -> PhotoController {
    let r: Arc<dyn PhotoRepository> = repo.clone();
    PhotoController::new(
        Arc::new(RatePhotoUseCase::new(r.clone())),
        Arc::new(SetColorLabelUseCase::new(r.clone())),
        Arc::new(SetFlagUseCase::new(r.clone())),
        Arc::new(DeletePhotoUseCase::new(r.clone())),
        Arc::new(MarcarCompradaUseCase::new(r)),
    )
}

#[tokio::test]
async fn avaliar_grava_a_nota_e_zero_tira_a_nota() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);

    c.rate_photo(&id.to_string(), 4).await.unwrap();
    assert_eq!(
        repo.ler(&id).unwrap().rating(),
        Some(Rating::new(4).unwrap())
    );

    c.rate_photo(&id.to_string(), 0).await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().rating(), None);
}

#[tokio::test]
async fn avaliar_fora_de_1_a_5_e_recusado_e_nao_mexe_na_foto() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);

    c.rate_photo(&id.to_string(), 3).await.unwrap();
    let erro = c.rate_photo(&id.to_string(), 6).await.unwrap_err();
    assert!(erro.starts_with("Invalid rating"), "{erro}");
    assert_eq!(
        repo.ler(&id).unwrap().rating(),
        Some(Rating::new(3).unwrap())
    );
}

#[tokio::test]
async fn id_que_nao_e_id_e_recusado_em_todas_as_operacoes_da_foto() {
    let repo = Arc::new(FotosEmMemoria::default());
    let c = controller_de_fotos(&repo);

    for erro in [
        c.rate_photo("lixo", 3).await.unwrap_err(),
        c.set_color_label("lixo", "red").await.unwrap_err(),
        c.set_flag("lixo", 1).await.unwrap_err(),
        c.delete_photo("lixo").await.unwrap_err(),
        c.set_comprada("lixo", true).await.unwrap_err(),
    ] {
        assert!(erro.starts_with("Invalid photo ID"), "{erro}");
    }
}

#[tokio::test]
async fn avaliar_foto_inexistente_devolve_erro_traduzido() {
    let repo = Arc::new(FotosEmMemoria::default());
    let c = controller_de_fotos(&repo);
    let erro = c
        .rate_photo(&PhotoId::new().to_string(), 3)
        .await
        .unwrap_err();
    assert!(erro.starts_with("Failed to rate photo"), "{erro}");
}

#[tokio::test]
async fn etiqueta_de_cor_poe_troca_e_limpa_com_vazio_ou_none() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);
    let sid = id.to_string();

    c.set_color_label(&sid, "Red").await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().color_label(), Some(ColorLabel::Red));

    c.set_color_label(&sid, "").await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().color_label(), None);

    c.set_color_label(&sid, "blue").await.unwrap();
    c.set_color_label(&sid, "NONE").await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().color_label(), None);
}

#[tokio::test]
async fn etiqueta_desconhecida_e_recusada_e_a_anterior_fica() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);

    c.set_color_label(&id.to_string(), "green").await.unwrap();
    let erro = c
        .set_color_label(&id.to_string(), "rosa")
        .await
        .unwrap_err();
    assert!(erro.starts_with("Invalid color label 'rosa'"), "{erro}");
    assert_eq!(
        repo.ler(&id).unwrap().color_label(),
        Some(ColorLabel::Green)
    );
}

#[tokio::test]
async fn sinalizador_1_pick_menos1_reject_zero_tira() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);
    let sid = id.to_string();

    c.set_flag(&sid, 1).await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().flag(), Some(Flag::Pick));
    c.set_flag(&sid, -1).await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().flag(), Some(Flag::Reject));
    c.set_flag(&sid, 0).await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().flag(), None);

    let erro = c.set_flag(&sid, 2).await.unwrap_err();
    assert_eq!(erro, "Invalid flag code '2'");
}

#[tokio::test]
async fn excluir_tira_a_foto_do_catalogo() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    controller_de_fotos(&repo)
        .delete_photo(&id.to_string())
        .await
        .unwrap();
    assert!(repo.ler(&id).is_none());
}

#[tokio::test]
async fn levada_no_balcao_marca_e_desmarca_e_marcar_de_novo_nao_muda_a_data() {
    let f = foto("a");
    let id = f.id();
    let repo = Arc::new(FotosEmMemoria::com([f]));
    let c = controller_de_fotos(&repo);
    let sid = id.to_string();

    c.set_comprada(&sid, true).await.unwrap();
    let primeira = repo.ler(&id).unwrap();
    assert!(primeira.comprada());

    c.set_comprada(&sid, true).await.unwrap();
    assert_eq!(repo.ler(&id).unwrap().comprada_em(), primeira.comprada_em());

    c.set_comprada(&sid, false).await.unwrap();
    assert!(!repo.ler(&id).unwrap().comprada());
}

#[tokio::test]
async fn marcar_comprada_foto_inexistente_devolve_erro_traduzido() {
    let repo = Arc::new(FotosEmMemoria::default());
    let erro = controller_de_fotos(&repo)
        .set_comprada(&PhotoId::new().to_string(), true)
        .await
        .unwrap_err();
    assert!(erro.starts_with("Failed to mark as purchased"), "{erro}");
}

// ============================================================
// CollectionController
// ============================================================

fn controller_de_colecoes(
    fotos: &Arc<FotosEmMemoria>,
    colecoes: &Arc<ColecoesEmMemoria>,
) -> CollectionController {
    let cr: Arc<dyn CollectionRepository> = colecoes.clone();
    let pr: Arc<dyn PhotoRepository> = fotos.clone();
    CollectionController::new(
        cr.clone(),
        Arc::new(CreateCollectionUseCase::new(cr.clone())),
        Arc::new(AddPhotoToCollectionUseCase::new(cr.clone(), pr)),
        Arc::new(RemovePhotoFromCollectionUseCase::new(cr)),
    )
}

#[tokio::test]
async fn criar_colecao_devolve_o_modelo_da_tela_e_grava() {
    let fotos = Arc::new(FotosEmMemoria::default());
    let colecoes = Arc::new(ColecoesEmMemoria::default());
    let c = controller_de_colecoes(&fotos, &colecoes);

    let vm = c.create("Ensaio Ana".into()).await.unwrap();
    assert_eq!(vm.name, "Ensaio Ana");
    assert_eq!(vm.photo_count, 0);

    let id = CollectionId::from_string(&vm.id).unwrap();
    assert_eq!(colecoes.ler(&id).unwrap().name(), "Ensaio Ana");
}

#[tokio::test]
async fn listar_leva_a_contagem_e_nao_os_ids() {
    let a = foto("a");
    let mut col = Collection::new("Ensaio");
    col.add_photo(a.id());
    let cid = col.id().to_string();
    let fotos = Arc::new(FotosEmMemoria::com([a]));
    let colecoes = Arc::new(ColecoesEmMemoria::com([col]));

    let lista = controller_de_colecoes(&fotos, &colecoes)
        .list()
        .await
        .unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].id, cid);
    assert_eq!(lista[0].photo_count, 1);
}

#[tokio::test]
async fn acrescentar_em_lote_conta_e_repetida_nao_e_erro() {
    let (a, b) = (foto("a"), foto("b"));
    let (ia, ib) = (a.id(), b.id());
    let col = Collection::new("Ensaio");
    let cid = *col.id();
    let fotos = Arc::new(FotosEmMemoria::com([a, b]));
    let colecoes = Arc::new(ColecoesEmMemoria::com([col]));
    let c = controller_de_colecoes(&fotos, &colecoes);

    let n = c
        .add_photos(cid.to_string(), vec![ia.to_string(), ib.to_string()])
        .await
        .unwrap();
    assert_eq!(n, 2);
    // A mesma de novo: segue e conta, sem duplicar.
    let n = c
        .add_photos(cid.to_string(), vec![ia.to_string()])
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(colecoes.ler(&cid).unwrap().photo_count(), 2);

    let mut ids = c.photo_ids(cid.to_string()).await.unwrap();
    ids.sort();
    let mut esperado = vec![ia.to_string(), ib.to_string()];
    esperado.sort();
    assert_eq!(ids, esperado);
}

#[tokio::test]
async fn acrescentar_foto_que_nao_existe_para_o_lote_no_ponto_do_erro() {
    let a = foto("a");
    let ia = a.id();
    let col = Collection::new("Ensaio");
    let cid = *col.id();
    let fotos = Arc::new(FotosEmMemoria::com([a]));
    let colecoes = Arc::new(ColecoesEmMemoria::com([col]));
    let c = controller_de_colecoes(&fotos, &colecoes);

    let resultado = c
        .add_photos(
            cid.to_string(),
            vec![ia.to_string(), PhotoId::new().to_string()],
        )
        .await;
    assert!(resultado.is_err());
    // O que veio antes do erro já foi gravado (comportamento atual).
    assert_eq!(colecoes.ler(&cid).unwrap().photo_count(), 1);
}

#[tokio::test]
async fn ids_invalidos_sao_recusados() {
    let fotos = Arc::new(FotosEmMemoria::default());
    let colecoes = Arc::new(ColecoesEmMemoria::default());
    let c = controller_de_colecoes(&fotos, &colecoes);

    assert!(c.photo_ids("lixo".into()).await.is_err());
    assert!(c.add_photos("lixo".into(), vec![]).await.is_err());
    assert!(c.remove_photos("lixo".into(), vec![]).await.is_err());

    let col = Collection::new("x");
    let cid = *col.id();
    let colecoes = Arc::new(ColecoesEmMemoria::com([col]));
    let c = controller_de_colecoes(&fotos, &colecoes);
    assert!(c
        .add_photos(cid.to_string(), vec!["lixo".into()])
        .await
        .is_err());
}

#[tokio::test]
async fn ids_de_colecao_inexistente_diz_nao_encontrada() {
    let fotos = Arc::new(FotosEmMemoria::default());
    let colecoes = Arc::new(ColecoesEmMemoria::default());
    let erro = controller_de_colecoes(&fotos, &colecoes)
        .photo_ids(CollectionId::new().to_string())
        .await
        .unwrap_err();
    assert_eq!(erro, "coleção não encontrada");
}

#[tokio::test]
async fn remover_conta_so_as_que_estavam_e_ausente_nao_e_erro() {
    let (a, b) = (foto("a"), foto("b"));
    let (ia, ib) = (a.id(), b.id());
    let mut col = Collection::new("Ensaio");
    col.add_photo(ia);
    let cid = *col.id();
    let fotos = Arc::new(FotosEmMemoria::com([a, b]));
    let colecoes = Arc::new(ColecoesEmMemoria::com([col]));
    let c = controller_de_colecoes(&fotos, &colecoes);

    let n = c
        .remove_photos(cid.to_string(), vec![ia.to_string(), ib.to_string()])
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert!(colecoes.ler(&cid).unwrap().is_empty());
}

// ============================================================
// ExportController
// ============================================================

#[tokio::test]
async fn exportar_repassa_foto_destino_e_opcoes_ao_exportador() {
    let f = foto("a");
    let id = f.id();
    let repo: Arc<dyn PhotoRepository> = Arc::new(FotosEmMemoria::com([f]));
    let exportador = Arc::new(ExportadorQueAnota::default());
    let c = ExportController::new(Arc::new(ExportPhotoUseCase::new(repo, exportador.clone())));

    let opcoes = ExportOptions::default();
    c.export_photo(id.to_string(), "/saida/a.jpg".into(), &opcoes)
        .await
        .unwrap();

    let pedidos = exportador.pedidos.lock().unwrap();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(pedidos[0].0, id);
    assert_eq!(pedidos[0].1, "/saida/a.jpg");
    assert_eq!(pedidos[0].2, opcoes.quality());
}

#[tokio::test]
async fn exportar_recusa_id_ruim_foto_ausente_e_falha_do_exportador() {
    let f = foto("a");
    let id = f.id();
    let repo: Arc<dyn PhotoRepository> = Arc::new(FotosEmMemoria::com([f]));
    let opcoes = ExportOptions::default();

    let bom = Arc::new(ExportadorQueAnota::default());
    let c = ExportController::new(Arc::new(ExportPhotoUseCase::new(repo.clone(), bom.clone())));
    assert!(c
        .export_photo("lixo".into(), "/s/a.jpg".into(), &opcoes)
        .await
        .is_err());
    assert!(c
        .export_photo(PhotoId::new().to_string(), "/s/a.jpg".into(), &opcoes)
        .await
        .is_err());
    assert!(bom.pedidos.lock().unwrap().is_empty());

    let ruim = Arc::new(ExportadorQueAnota {
        falha: true,
        ..Default::default()
    });
    let c = ExportController::new(Arc::new(ExportPhotoUseCase::new(repo, ruim)));
    let erro = c
        .export_photo(id.to_string(), "/s/a.jpg".into(), &opcoes)
        .await
        .unwrap_err();
    assert!(erro.contains("disco cheio"), "{erro}");
}

// ============================================================
// PresetController
// ============================================================

#[derive(Default)]
struct PresetsEmMemoria(Mutex<Vec<Preset>>);

#[async_trait::async_trait]
impl PresetRepository for PresetsEmMemoria {
    async fn save(&self, p: &Preset) -> DomainResult<()> {
        let mut v = self.0.lock().unwrap();
        v.retain(|x| x.id != p.id);
        v.push(p.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: &domain::entities::PresetId) -> DomainResult<Option<Preset>> {
        Ok(self.0.lock().unwrap().iter().find(|p| &p.id == id).cloned())
    }
    async fn find_all(&self) -> DomainResult<Vec<Preset>> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn delete(&self, id: &domain::entities::PresetId) -> DomainResult<()> {
        self.0.lock().unwrap().retain(|p| &p.id != id);
        Ok(())
    }
}

fn controller_de_presets(repo: &Arc<PresetsEmMemoria>) -> PresetController {
    let r: Arc<dyn PresetRepository> = repo.clone();
    PresetController::new(
        Arc::new(ListPresetsUseCase::new(r.clone())),
        Arc::new(SavePresetUseCase::new(r.clone())),
        Arc::new(RenamePresetUseCase::new(r.clone())),
        Arc::new(DeletePresetUseCase::new(r)),
    )
}

#[tokio::test]
async fn presets_salvar_listar_renomear_e_apagar_de_ponta_a_ponta() {
    let repo = Arc::new(PresetsEmMemoria::default());
    let c = controller_de_presets(&repo);

    let sistema = c.list_presets().await.unwrap();
    assert!(!sistema.is_empty() && sistema.iter().all(|p| p.is_system));

    let meu = Preset::user(
        "Meu".into(),
        PresetAdjustments::vazia().com("exposure", 0.5),
    );
    c.save_preset(&meu).await.unwrap();
    let lista = c.list_presets().await.unwrap();
    assert_eq!(lista.len(), sistema.len() + 1);

    c.rename_preset(&meu.id, "Renomeado".into()).await.unwrap();
    let lista = c.list_presets().await.unwrap();
    let achado = lista.iter().find(|p| p.id == meu.id).unwrap();
    assert_eq!(achado.name, "Renomeado");
    assert_eq!(achado.adjustments.get("exposure"), Some(0.5));
    // Mesma linha: não sobrou uma cópia com o nome antigo.
    assert_eq!(lista.iter().filter(|p| p.name == "Meu").count(), 0);

    c.delete_preset(&meu.id).await.unwrap();
    assert_eq!(c.list_presets().await.unwrap().len(), sistema.len());
}

#[tokio::test]
async fn renomear_devolve_a_recusa_do_use_case_como_texto() {
    let repo = Arc::new(PresetsEmMemoria::default());
    let c = controller_de_presets(&repo);
    let sistema = Preset::system("De sistema", PresetAdjustments::vazia());
    repo.save(&sistema).await.unwrap();

    let e1 = c
        .rename_preset(&sistema.id, "Outro".into())
        .await
        .unwrap_err();
    assert!(e1.contains("sistema"), "{e1}");
    let e2 = c
        .rename_preset(&domain::entities::PresetId::new(), "X".into())
        .await
        .unwrap_err();
    assert!(e2.contains("não existe"), "{e2}");
    let e3 = c.rename_preset(&sistema.id, "  ".into()).await.unwrap_err();
    assert!(e3.contains("sem nome"), "{e3}");
}
