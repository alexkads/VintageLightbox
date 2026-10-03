//! Unit tests for Preset Use Cases
//!
//! Tests using mockall for PresetRepository mock.

use crate::presets::{
    presets_de_sistema, DeletePresetUseCase, ListPresetsUseCase, RenamePresetUseCase,
    SavePresetUseCase,
};
use domain::{
    entities::{preset::PresetAdjustments, Preset, PresetId},
    repositories::PresetRepository,
    DomainError, DomainResult,
};
use mockall::mock;
use std::sync::Arc;

// Mock PresetRepository
mock! {
    pub PresetRepo {}

    #[async_trait::async_trait]
    impl PresetRepository for PresetRepo {
        async fn save(&self, preset: &Preset) -> DomainResult<()>;
        async fn find_by_id(&self, id: &PresetId) -> DomainResult<Option<Preset>>;
        async fn find_all(&self) -> DomainResult<Vec<Preset>>;
        async fn delete(&self, id: &PresetId) -> DomainResult<()>;
    }
}

// ============================================
// SavePresetUseCase Tests
// ============================================

#[tokio::test]
async fn test_save_preset_success() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo.expect_save().times(1).returning(|_| Ok(()));

    let use_case = SavePresetUseCase::new(Arc::new(mock_repo));

    let preset = Preset::user(
        "My Preset".to_string(),
        PresetAdjustments::vazia()
            .com("exposure", 1.5)
            .com("contrast", 1.2),
    );

    // Act
    let result = use_case.execute(&preset).await;

    // Assert
    assert!(result.is_ok());
    // 🔑 A identidade é de quem criou o `Preset`, e não do use case: é o que
    // faz a lista da tela e a linha do banco serem a mesma coisa.
    assert!(!preset.is_system);
    assert_eq!(preset.adjustments.get("exposure"), Some(1.5));
}

#[tokio::test]
async fn test_save_preset_repository_error() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_save()
        .times(1)
        .returning(|_| Err(DomainError::InfrastructureError("DB error".to_string())));

    let use_case = SavePresetUseCase::new(Arc::new(mock_repo));

    let preset = Preset::user("Test".to_string(), PresetAdjustments::vazia());

    // Act
    let result = use_case.execute(&preset).await;

    // Assert
    assert!(result.is_err());
}

// ============================================
// ListPresetsUseCase Tests
// ============================================

#[tokio::test]
async fn test_list_presets_returns_system_and_user() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();

    // Repository returns user presets (system presets are added by the use case)
    let user_preset = Preset::user(
        "User Preset".to_string(),
        PresetAdjustments::vazia().com("exposure", 0.5),
    );
    mock_repo
        .expect_find_all()
        .times(1)
        .returning(move || Ok(vec![user_preset.clone()]));

    let use_case = ListPresetsUseCase::new(Arc::new(mock_repo));

    // Act
    let result = use_case.execute().await;

    // Assert
    assert!(result.is_ok());
    let presets = result.unwrap();

    // Os vinte de sistema mais o do usuário. Eram quatro até 7/set/2026, com os
    // nomes em inglês do app antigo; agora são os do site — com o estilo do
    // estúdio no darktable desde 17/set/2026 e os doze "Vintage ·" desde
    // 28/set/2026 —, e o que a lista devolve tem de trazer os dois grupos.
    assert_eq!(presets.len(), presets_de_sistema().len() + 1);

    let system_names: Vec<_> = presets
        .iter()
        .filter(|p| p.is_system && p.grupo.is_none())
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        system_names,
        [
            "Preto e branco clássico",
            "Sépia à moda antiga",
            "Retrato suave",
            "Luz de estúdio",
            "Hora dourada",
            "Alta-chave",
            "RecordarFotos P&B",
            "Vintage · Foto envelhecida",
            "Vintage · Polaroid antiga",
            "Vintage · Anos passados",
            "Vintage · Processo cruzado",
            "Vintage · Cianótipo",
            "Vintage · Cinza antigo",
            "Vintage · Bleach bypass",
            "Vintage · Positivo direto",
            "Vintage · Kodachrome",
            "Vintage · Portra 400",
            "Vintage · Ektachrome anos 70",
            "Vintage · Desbotado anos 70",
            "Nitidez para impressão",
            "Cinematográfico P&B",
        ]
    );

    // Check user preset exists
    let user_presets: Vec<_> = presets.iter().filter(|p| !p.is_system).collect();
    assert_eq!(user_presets.len(), 1);
    assert_eq!(user_presets[0].name, "User Preset");
}

#[tokio::test]
async fn test_list_presets_empty_user_presets() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_find_all()
        .times(1)
        .returning(|| Ok(vec![]));

    let use_case = ListPresetsUseCase::new(Arc::new(mock_repo));

    // Act
    let result = use_case.execute().await;

    // Assert
    assert!(result.is_ok());
    let presets = result.unwrap();

    // Sem nada salvo, sobram só os de sistema.
    assert_eq!(presets.len(), presets_de_sistema().len());
    assert!(presets.iter().all(|p| p.is_system));
}

/// 🚨 Preset de sistema fora da faixa dos sliders **destrói a foto**.
///
/// Este teste não existia, e por isso os quatro antigos passaram meses pedindo
/// números de outra escala: `saturation: -100` numa escala que vai de -1 a 1,
/// `contrast: 50` num multiplicador de 0 a 2, `temperature: ±15` numa faixa de
/// -10 a 10. Nenhuma camada reclamava — o valor chega ao shader como `f32` e o
/// shader faz a conta com o que recebe.
///
/// As faixas aqui são as mesmas de `CONTROLES`
/// (`ui-gpui/src/revelacao/controles.rs`), que é quem as oferece ao arrasto.
/// Repeti-las é de propósito: o `use-cases` não pode depender da interface, e o
/// que este teste afirma é que **nenhum preset pede o que nenhum slider
/// consegue pedir**.
#[test]
fn os_presets_de_sistema_ficam_dentro_da_escala_do_motor() {
    for preset in presets_de_sistema() {
        let nome = &preset.name;
        for (campo, valor) in preset.adjustments.iter() {
            let (minimo, maximo) = faixa_do_slider(campo);
            assert!(
                (minimo..=maximo).contains(&valor),
                "o preset \"{nome}\" pede {campo} = {valor}, e o slider vai de {minimo} a {maximo}"
            );
        }
    }
}

/// A faixa que o slider oferece, por nome de campo.
///
/// 🔑 **Por família, e não campo a campo.** Os 24 do HSL são três faixas, e
/// escrever 53 linhas convidaria a copiar a do vizinho — que é exatamente o
/// engano que o teste existe para pegar (matiz de HSL vai de -180 a 180, e o da
/// tonalização é a roda inteira, de 0 a 360).
fn faixa_do_slider(campo: &str) -> (f32, f32) {
    match campo {
        "exposure" => (-5.0, 5.0),
        // Multiplicador em volta de 128, com neutro em 1,0.
        "contrast" => (0.0, 2.0),
        "temperature" | "tint" => (-10.0, 10.0),
        "clarity" | "vibrance" | "saturation" => (-1.0, 1.0),
        // Raio zero não teria pixel de vizinhança para comparar.
        "sharpen_radius" => (0.5, 3.0),
        "nr_luminance"
        | "nr_color"
        | "sharpen_amount"
        | "lens_vignette_midpoint"
        | "grain_amount"
        | "grain_size"
        | "grain_roughness"
        | "sharpen_detail"
        | "sharpen_masking"
        | "nr_luminance_detail"
        | "nr_luminance_contrast"
        | "nr_color_detail"
        | "nr_color_smoothness"
        | "pcv_midpoint"
        | "pcv_feather"
        | "pcv_highlights"
        | "split_blending" => (0.0, 100.0),
        campo if campo.starts_with("tone_curve_split_") => (0.0, 100.0),
        "pcv_style" => (0.0, 2.0),
        // A vinheta do darktable: escala e decaimento em % do raio, o centro
        // e as proporções nas faixas do módulo.
        "darktable_vignette_scale" | "darktable_vignette_falloff_scale" => (0.0, 200.0),
        "darktable_vignette_brightness"
        | "darktable_vignette_saturation"
        | "darktable_vignette_center_x"
        | "darktable_vignette_center_y" => (-1.0, 1.0),
        "darktable_vignette_ativo" | "darktable_vignette_autoratio" => (0.0, 1.0),
        "darktable_vignette_dithering" => (0.0, 2.0),
        "darktable_vignette_whratio" => (0.0, 2.0),
        "darktable_vignette_shape" => (0.0, 5.0),
        // A roda de cor inteira: estes **escolhem** a cor que entra.
        campo if campo.starts_with("split_") && campo.ends_with("_hue") => (0.0, 360.0),
        campo if campo.starts_with("split_") && campo.ends_with("_sat") => (0.0, 100.0),
        campo if campo.starts_with("calib_") && campo.ends_with("_hue") => (-100.0, 100.0),
        // A curva por ponto guarda a altura de cada um dos nove pontos, em
        // níveis de 0 a 255 — a faixa do `CURVA_POR_PONTO` do site.
        campo if campo.starts_with("curva_") => (0.0, 255.0),
        // Interruptor do preto e branco.
        "bw_ativo" => (0.0, 1.0),
        // Os oito do HSL giram a cor que o pixel já tem: é um desvio.
        campo if campo.ends_with("_hue") => (-180.0, 180.0),
        _ => (-100.0, 100.0),
    }
}

/// O que cada preset de sistema faz, em números.
///
/// Um preset que não move nada é tão defeito quanto um que move demais — foi o
/// caso do "Auto", que pedia `exposure: Some(0.0)` sobre o neutro `0.0`.
///
/// 🔑 **Os nomes conferidos são os do site** (`presets-do-sistema.ts`): a lista
/// é a mesma nos dois, e um nome que só existe de um lado é a divergência que
/// este teste existe para pegar.
#[test]
fn cada_preset_de_sistema_move_alguma_coisa() {
    let por_nome = |nome: &str| {
        presets_de_sistema()
            .into_iter()
            .find(|p| p.name == nome)
            .unwrap_or_else(|| panic!("o preset de sistema \"{nome}\" sumiu da lista"))
    };

    // Cinza é o fator zero: `1.0 + (-1.0)`.
    assert_eq!(
        por_nome("Preto e branco clássico")
            .adjustments
            .get("saturation"),
        Some(-1.0)
    );
    // 🚨 A sépia se faz com tonalização, e não com temperatura: temperatura age
    // antes da saturação, e numa foto em cinza a cor dela é apagada em seguida.
    let sepia = por_nome("Sépia à moda antiga");
    assert_eq!(sepia.adjustments.get("saturation"), Some(-1.0));
    assert_eq!(sepia.adjustments.get("split_shadow_hue"), Some(35.0));
    assert_eq!(sepia.adjustments.get("split_shadow_sat"), Some(45.0));
    assert_eq!(
        sepia.adjustments.get("temperature"),
        None,
        "a sépia não passa pela temperatura — ela seria apagada pela saturação"
    );
    // O shader faz `r += t*10` em 0..255 — 25 níveis para cada lado.
    assert_eq!(
        por_nome("Hora dourada").adjustments.get("temperature"),
        Some(2.5)
    );

    for preset in presets_de_sistema() {
        assert!(
            !preset.adjustments.is_empty(),
            "o preset de sistema \"{}\" não mexe em nada",
            preset.name
        );
    }
}

/// ⚠️ **A lista é a mesma do site, e na mesma ordem.**
///
/// São vinte nomes escritos em dois lugares (aqui e em
/// `revelacao/presets-do-sistema.ts`), e nada liga um ao outro em tempo de
/// compilação. Um nome trocado aqui não quebra nada: só faz o fotógrafo
/// procurar no app a predefinição que ele usou no navegador.
#[test]
fn os_vinte_do_sistema_sao_os_do_site() {
    // A pasta "Do sistema"; as "LRs" vêm de `lightroom.json`, o mesmo arquivo
    // nos dois lados.
    let nomes: Vec<String> = presets_de_sistema()
        .into_iter()
        .filter(|preset| preset.grupo.is_none())
        .map(|preset| preset.name)
        .collect();

    assert_eq!(
        nomes,
        vec![
            "Preto e branco clássico",
            "Sépia à moda antiga",
            "Retrato suave",
            "Luz de estúdio",
            "Hora dourada",
            "Alta-chave",
            "RecordarFotos P&B",
            "Vintage · Foto envelhecida",
            "Vintage · Polaroid antiga",
            "Vintage · Anos passados",
            "Vintage · Processo cruzado",
            "Vintage · Cianótipo",
            "Vintage · Cinza antigo",
            "Vintage · Bleach bypass",
            "Vintage · Positivo direto",
            "Vintage · Kodachrome",
            "Vintage · Portra 400",
            "Vintage · Ektachrome anos 70",
            "Vintage · Desbotado anos 70",
            "Nitidez para impressão",
            "Cinematográfico P&B",
        ]
    );
}

/// E cada uma escreve a mesma quantidade de campos que a do site — o número que
/// a lista mostra ao lado do nome, nas duas telas.
#[test]
fn cada_uma_escreve_a_mesma_quantidade_de_campos_do_site() {
    let quantos: Vec<usize> = presets_de_sistema()
        .into_iter()
        .filter(|preset| preset.grupo.is_none())
        .map(|preset| preset.adjustments.len())
        .collect();

    assert_eq!(
        quantos,
        // O RecordarFotos P&B (7º) tem 42 desde 3/out/2026: os do arquivo que o
        // dono exportou — a vinheta do darktable desligada (seis números
        // guardados) e as cinco da pós-corte.
        vec![5, 8, 9, 6, 7, 7, 42, 13, 13, 10, 12, 7, 11, 6, 10, 16, 17, 13, 13, 4, 35]
    );
}

/// A predefinição do operador importada de um `.dtstyle` (só campos `dt_*`)
/// vira o RecordarFotos P&B de hoje, e recomeça do neutro — em vez de virar
/// uma predefinição vazia que não faz nada.
#[tokio::test]
async fn a_predefinicao_importada_do_darktable_vira_o_recordarfotos_pb() {
    let mut mock_repo = MockPresetRepo::new();
    let do_darktable = Preset::user(
        "PB Gramado".to_string(),
        PresetAdjustments::vazia()
            .com("dt_monochrome_ativo", 1.0)
            .com("dt_shadhi_shadows", 65.38),
    );
    let do_lightroom = Preset::user(
        "Minha".to_string(),
        PresetAdjustments::vazia().com("exposure", 0.5),
    );
    mock_repo
        .expect_find_all()
        .times(1)
        .returning(move || Ok(vec![do_darktable.clone(), do_lightroom.clone()]));

    let presets = ListPresetsUseCase::new(Arc::new(mock_repo))
        .execute()
        .await
        .expect("lista");
    let pb = presets
        .iter()
        .find(|p| p.name == "PB Gramado")
        .expect("a do operador");
    assert!(pb.replaces, "um visual inteiro recomeça do neutro");
    assert!(pb.adjustments.campos().all(|c| !c.starts_with("dt_")));
    assert_eq!(pb.adjustments.get("bw_ativo"), Some(1.0));
    let minha = presets.iter().find(|p| p.name == "Minha").expect("a outra");
    assert!(!minha.replaces, "a do Lightroom continua somando");
    assert_eq!(minha.adjustments.get("exposure"), Some(0.5));
}

/// A foto guardada com o RecordarFotos P&B do darktable reabre P&B, com os
/// valores de hoje — e uma foto sem ele só perde os `dt_*` que não fazem nada.
#[test]
fn a_receita_do_darktable_vira_o_recordarfotos_pb_de_hoje() {
    use crate::presets::{migrar_do_darktable, RECORDARFOTOS_PB};
    let mut antiga = serde_json::json!({
        "dt_monochrome_ativo": 1.0,
        "dt_shadhi_shadows": 65.38,
        "exposure": 0.4,
        "corte_ativo": 1.0,
    });
    let receita = antiga.as_object_mut().unwrap();
    migrar_do_darktable(receita);
    assert!(receita.keys().all(|k| !k.starts_with("dt_")));
    for (nome, valor) in RECORDARFOTOS_PB {
        assert_eq!(receita[*nome].as_f64().unwrap() as f32, *valor, "{nome}");
    }
    assert_eq!(receita["corte_ativo"], 1.0, "o corte da foto fica");

    // A vinheta do P&B de hoje é a pós-corte do arquivo do dono (3/out/2026);
    // a do darktable não liga.
    assert_eq!(receita["pcv_amount"], 78.0);
    assert!(!receita.contains_key("darktable_vignette_ativo"));
    assert!(
        !receita.contains_key("lens_vignette_amount"),
        "a aproximação saiu"
    );

    let mut colorida = serde_json::json!({
        "dt_vignette_ativo": 1.0,
        "dt_vignette_brightness": -0.25,
        "dt_vignette_shape": 2.0,
        "dt_vignette_unbound": 1.0,
        "exposure": 0.4,
    });
    let receita = colorida.as_object_mut().unwrap();
    migrar_do_darktable(receita);
    assert!(receita.keys().all(|k| !k.starts_with("dt_")));
    assert_eq!(receita["exposure"], 0.4);
    assert_eq!(receita["darktable_vignette_ativo"], 1.0);
    assert_eq!(receita["darktable_vignette_brightness"], -0.25);
    assert_eq!(receita["darktable_vignette_shape"], 2.0);
    // O que não estava gravado fica no neutro, o padrão do darktable.
    assert_eq!(receita.len(), 1 + 3);

    let mut desligada =
        serde_json::json!({ "dt_vignette_ativo": 0.0, "dt_vignette_brightness": 1.0 });
    let receita = desligada.as_object_mut().unwrap();
    migrar_do_darktable(receita);
    assert!(
        receita.is_empty(),
        "vinheta desligada não volta: {receita:?}"
    );
}

// ============================================
// DeletePresetUseCase Tests
// ============================================

#[tokio::test]
async fn test_delete_preset_success() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo.expect_delete().times(1).returning(|_| Ok(()));

    let use_case = DeletePresetUseCase::new(Arc::new(mock_repo));
    let preset_id = PresetId::default();

    // Act
    let result = use_case.execute(&preset_id).await;

    // Assert
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_preset_repository_error() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_delete()
        .times(1)
        .returning(|_| Err(DomainError::InfrastructureError("Not found".to_string())));

    let use_case = DeletePresetUseCase::new(Arc::new(mock_repo));
    let preset_id = PresetId::default();

    // Act
    let result = use_case.execute(&preset_id).await;

    // Assert
    assert!(result.is_err());
}

/// 🚨 O caso que deu origem à marca: "Preto e branco clássico" aplicado sobre
/// "Sépia à moda antiga" não ficava preto e branco — a dessaturação era
/// escrita, a tonalização âmbar da sépia **não** era desfeita, e saía uma foto
/// âmbar com nome de preto e branco (dono, 2026-09-11, na web).
#[test]
fn as_que_definem_o_look_substituem_e_a_nitidez_soma() {
    let sistema = presets_de_sistema();
    let por_nome = |nome: &str| {
        sistema
            .iter()
            .find(|p| p.name == nome)
            .unwrap_or_else(|| panic!("{nome} tem que existir"))
    };

    for nome in [
        "Preto e branco clássico",
        "Sépia à moda antiga",
        "Retrato suave",
        "Luz de estúdio",
        "Hora dourada",
        "Alta-chave",
    ] {
        assert!(por_nome(nome).replaces, "{nome} define o visual");
    }

    assert!(
        !por_nome("Nitidez para impressão").replaces,
        "nitidez se aplica depois de qualquer look — zerar o look para acrescentá-la \
         seria o contrário do que o gesto quer dizer"
    );
}

/// A predefinição que o operador salva **soma**, como sempre somou.
#[test]
fn a_do_operador_nao_substitui() {
    let dele = Preset::user("Meu jeito".into(), PresetAdjustments::vazia());
    assert!(
        !dele.replaces,
        "mudar isso reescreveria o que ele ja salvou"
    );
}

// ============================================
// RenamePresetUseCase Tests
// ============================================

#[tokio::test]
async fn renomear_regrava_a_mesma_linha_com_o_nome_novo() {
    let original = Preset::user(
        "Antigo".to_string(),
        PresetAdjustments::vazia().com("exposure", 0.5),
    );
    let id = original.id;

    let mut repo = MockPresetRepo::new();
    let achado = original.clone();
    repo.expect_find_by_id()
        .times(1)
        .returning(move |_| Ok(Some(achado.clone())));
    // 🔑 Renomear não cria id novo nem mexe nos ajustes: é a mesma linha.
    let esperado = original.clone();
    repo.expect_save()
        .times(1)
        .withf(move |p| {
            p.id == esperado.id
                && p.name == "Novo"
                && p.adjustments == esperado.adjustments
                && !p.is_system
        })
        .returning(|_| Ok(()));

    let uso = RenamePresetUseCase::new(Arc::new(repo));
    assert!(uso.execute(&id, "Novo".to_string()).await.is_ok());
}

#[tokio::test]
async fn renomear_aparo_o_nome_antes_de_gravar() {
    let original = Preset::user("Antigo".to_string(), PresetAdjustments::vazia());
    let id = original.id;

    let mut repo = MockPresetRepo::new();
    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(original.clone())));
    repo.expect_save()
        .times(1)
        .withf(|p| p.name == "Com espaços")
        .returning(|_| Ok(()));

    let uso = RenamePresetUseCase::new(Arc::new(repo));
    assert!(uso
        .execute(&id, "  Com espaços \n".to_string())
        .await
        .is_ok());
}

#[tokio::test]
async fn renomear_para_nome_vazio_e_recusado_sem_tocar_no_banco() {
    // Sem `expect_*`: qualquer chamada ao repositório derruba o teste.
    let uso = RenamePresetUseCase::new(Arc::new(MockPresetRepo::new()));
    let id = PresetId::new();

    for nome in ["", "   ", "\t\n"] {
        let erro = uso.execute(&id, nome.to_string()).await.unwrap_err();
        assert!(matches!(erro, DomainError::InvalidOperation(_)), "{nome:?}");
    }
}

#[tokio::test]
async fn renomear_o_que_nao_existe_e_recusado_e_nao_grava() {
    let mut repo = MockPresetRepo::new();
    repo.expect_find_by_id().times(1).returning(|_| Ok(None));
    repo.expect_save().never();

    let uso = RenamePresetUseCase::new(Arc::new(repo));
    let erro = uso
        .execute(&PresetId::new(), "Qualquer".to_string())
        .await
        .unwrap_err();
    assert!(matches!(erro, DomainError::InvalidOperation(_)));
}

#[tokio::test]
async fn predefinicao_de_sistema_nao_se_renomeia() {
    let sistema = presets_de_sistema().remove(0);
    let id = sistema.id;

    let mut repo = MockPresetRepo::new();
    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(sistema.clone())));
    // Gravar criaria uma cópia ao lado da original na próxima listagem.
    repo.expect_save().never();

    let uso = RenamePresetUseCase::new(Arc::new(repo));
    let erro = uso.execute(&id, "Outro".to_string()).await.unwrap_err();
    assert!(matches!(erro, DomainError::InvalidOperation(_)));
}

#[tokio::test]
async fn renomear_propaga_o_erro_do_banco() {
    let original = Preset::user("Antigo".to_string(), PresetAdjustments::vazia());
    let id = original.id;

    let mut repo = MockPresetRepo::new();
    repo.expect_find_by_id()
        .returning(move |_| Ok(Some(original.clone())));
    repo.expect_save()
        .returning(|_| Err(DomainError::InfrastructureError("DB error".into())));

    let uso = RenamePresetUseCase::new(Arc::new(repo));
    assert!(matches!(
        uso.execute(&id, "Novo".to_string()).await,
        Err(DomainError::InfrastructureError(_))
    ));
}

/// 🎞️ A pasta "LRs": as 26 do Lightroom do estúdio, as de vinheta somando e as
/// de visual recomeçando do neutro. A "Predefinição sem título" e a
/// "RecordarFotos Bem Velhão" saíram (dono, 2/out/2026).
#[test]
fn as_do_lightroom_vem_na_pasta_lrs_e_so_as_vinhetas_somam() {
    use crate::presets::list_presets::{presets_de_sistema, presets_do_lightroom, GRUPO_LRS};
    let lrs = presets_do_lightroom();
    assert_eq!(lrs.len(), 26);
    for p in &lrs {
        assert!(p.is_system, "{}", p.name);
        assert_eq!(p.grupo.as_deref(), Some(GRUPO_LRS), "{}", p.name);
        // A versão de processo vai junto da vinheta: no processo 1 ela é a
        // medida no Lightroom (`revelacao_core::lightroom`). E a vinheta do
        // darktable desliga: uma vinheta por vez.
        let so_vinheta = p
            .adjustments
            .campos()
            .all(|c| c.starts_with("pcv_") || c == "processo" || c == "darktable_vignette_ativo");
        if so_vinheta {
            assert_eq!(
                p.adjustments.get("darktable_vignette_ativo"),
                Some(0.0),
                "{} deixa a vinheta do darktable ligada",
                p.name
            );
        }
        assert_eq!(
            p.replaces, !so_vinheta,
            "{}: recomeça só quem não é vinheta",
            p.name
        );
    }
    let nomes: Vec<&str> = lrs.iter().map(|p| p.name.as_str()).collect();
    for nome in [
        "Vinheta Borda",
        "Vinheta Nenhuma",
        "RecordarFotos P&B Cinematografico",
    ] {
        assert!(nomes.contains(&nome), "{nome}");
    }
    for fora in ["Predefinição sem título", "RecordarFotos Bem Velhão"] {
        assert!(!nomes.contains(&fora), "{fora} saiu das LRs");
    }
    // A "Vinheta Nenhuma" é justamente o zero: somada, tira a vinheta.
    let nenhuma = lrs.iter().find(|p| p.name == "Vinheta Nenhuma").unwrap();
    assert_eq!(nenhuma.adjustments.get("pcv_amount"), Some(0.0));
    assert!(!nenhuma.replaces);

    let todas = presets_de_sistema();
    let cine = todas
        .iter()
        .find(|p| p.name == "Cinematográfico P&B")
        .expect("o padrão do DNG do Estúdio Canela");
    assert!(cine.replaces && cine.grupo.is_none());
    assert_eq!(cine.adjustments.get("pcv_style"), Some(2.0));
}
