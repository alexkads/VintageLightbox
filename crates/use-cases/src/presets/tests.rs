//! Unit tests for Preset Use Cases
//!
//! Tests using mockall for PresetRepository mock.

use crate::presets::{
    presets_de_sistema, DeletePresetUseCase, ListPresetsUseCase, SavePresetUseCase,
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

    // Os sete de sistema mais o do usuário. Eram quatro até 7/set/2026, com os
    // nomes em inglês do app antigo; agora são os do site, e o que a lista
    // devolve tem de trazer os dois grupos.
    assert_eq!(presets.len(), 8);

    let system_names: Vec<_> = presets
        .iter()
        .filter(|p| p.is_system)
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
            "Nitidez para impressão",
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
        | "grain_size" => (0.0, 100.0),
        // A roda de cor inteira: estes **escolhem** a cor que entra.
        "split_shadow_hue" | "split_highlight_hue" => (0.0, 360.0),
        "split_shadow_sat" | "split_highlight_sat" => (0.0, 100.0),
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
/// São sete nomes escritos em dois lugares (aqui e em
/// `revelacao/presets-do-sistema.ts`), e nada liga um ao outro em tempo de
/// compilação. Um nome trocado aqui não quebra nada: só faz o fotógrafo
/// procurar no app a predefinição que ele usou no navegador.
#[test]
fn os_sete_do_sistema_sao_os_do_site() {
    let nomes: Vec<String> = presets_de_sistema()
        .into_iter()
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
            "Nitidez para impressão",
        ]
    );
}

/// E cada uma escreve a mesma quantidade de campos que a do site — o número que
/// a lista mostra ao lado do nome, nas duas telas.
#[test]
fn cada_uma_escreve_a_mesma_quantidade_de_campos_do_site() {
    let quantos: Vec<usize> = presets_de_sistema()
        .into_iter()
        .map(|preset| preset.adjustments.len())
        .collect();

    assert_eq!(quantos, vec![5, 8, 9, 6, 7, 7, 4]);
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
