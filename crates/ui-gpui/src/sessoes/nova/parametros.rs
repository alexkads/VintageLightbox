//! A revelação padrão da nova sessão: o preset e o corte que a etapa 2 aplica
//! sozinha às fotos do rascunho (`revelacao-padrao/parametros.ts` e
//! `nova/presets-da-sessao.ts` do site).
//!
//! # 🔑 O id é o do site
//!
//! `sistema:<chave>` para as predefinições que vêm com o editor, e o id do
//! banco para as do servidor (`GET /revelacao/presets`). É o que vai em
//! `preset_padrao_id`: a sessão criada aqui abre com o mesmo preset padrão no
//! navegador.

use domain::entities::preset::PresetAdjustments;
use domain::entities::Preset;
use serde_json::Value;

use crate::revelacao::corte;
use crate::revelacao::persistencia::Corte;
use crate::revelacao::presets;
use infrastructure::gpu_adjustments::Ajustes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Sistema,
    Minhas,
}

#[derive(Debug, Clone)]
pub struct PresetDaSessao {
    pub id: String,
    pub nome: String,
    pub grupo: Grupo,
    pub preset: Preset,
}

/// A chave de cada predefinição do sistema no site
/// (`[id]/revelacao/presets-do-sistema.ts`), pelo nome — que é igual nos dois.
pub fn id_do_sistema(nome: &str) -> Option<String> {
    crate::revelacao::presets::ordem::id_do_site(nome).map(|chave| format!("sistema:{chave}"))
}

/// 💛 As favoritas do coração da Revelação primeiro, na ordem delas; o resto
/// como veio — o `SeletorDePreset` do site. É o mesmo operador escolhendo, e
/// ele procura as mesmas.
pub fn favoritas_primeiro(lista: Vec<PresetDaSessao>, favoritas: &[String]) -> Vec<PresetDaSessao> {
    let (mut primeiro, resto): (Vec<_>, Vec<_>) =
        lista.into_iter().partition(|p| favoritas.contains(&p.id));
    primeiro.sort_by_key(|p| favoritas.iter().position(|f| *f == p.id));
    primeiro.extend(resto);
    primeiro
}

/// As predefinições do servidor, como `GET /revelacao/presets` as devolve.
pub fn presets_do_servidor(v: &Value) -> Vec<PresetDaSessao> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let nome = p.get("nome")?.as_str()?.to_string();
            let ajustes = p
                .get("ajustes")
                .and_then(Value::as_object)
                .map(|mapa| {
                    mapa.iter()
                        .filter_map(|(campo, valor)| Some((campo.clone(), valor.as_f64()? as f32)))
                        .fold(PresetAdjustments::vazia(), |acc, (c, v)| acc.com(c, v))
                })
                .unwrap_or_else(PresetAdjustments::vazia);
            Some(PresetDaSessao {
                id,
                preset: Preset::user(nome.clone(), ajustes),
                nome,
                grupo: Grupo::Minhas,
            })
        })
        .collect()
}

/// As do sistema (as do editor, com a chave do site) e depois as do servidor.
pub fn presets_da_sessao(
    do_sistema: &[Preset],
    do_servidor: Vec<PresetDaSessao>,
) -> Vec<PresetDaSessao> {
    do_sistema
        .iter()
        .filter(|p| p.is_system)
        .filter_map(|p| {
            // 🎞️ As da pasta "LRs" têm id pelo nome do arquivo: "RecordarFotos
            // P&B" existe nas duas pastas, e a tabela por nome daria a do
            // do sistema (a refeita do darktable) no lugar da do Lightroom.
            use crate::revelacao::presets::ordem;
            let id = if ordem::Grupo::de(p) == ordem::Grupo::Lrs {
                format!("sistema:{}", ordem::id_do_lightroom(&p.name))
            } else {
                id_do_sistema(&p.name)?
            };
            Some(PresetDaSessao {
                id,
                nome: p.name.clone(),
                grupo: Grupo::Sistema,
                preset: p.clone(),
            })
        })
        .chain(do_servidor)
        .collect()
}

/// O nome do preset guardado — ou uma marca, quando ele saiu da lista.
pub fn nome_do_preset(lista: &[PresetDaSessao], id: Option<&str>) -> Option<String> {
    let id = id?;
    Some(
        lista
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.nome.clone())
            .unwrap_or_else(|| {
                let curto: String = id.chars().take(8).collect();
                format!("Predefinição removida ({curto}…)")
            }),
    )
}

/// Os ajustes da revelação padrão. 🚨 **Sempre a partir do neutro**, mesmo para
/// o preset que soma: trocar o preset da sessão troca o efeito, não empilha.
pub fn ajustes_dos_parametros(preset: Option<&Preset>) -> Ajustes {
    let mut ajustes = Ajustes::default();
    if let Some(preset) = preset {
        presets::aplicar(&mut ajustes, &preset.adjustments);
    }
    ajustes
}

/// "3:2" → 1,5; "livre" e ausente não recortam.
pub fn valor_da_proporcao(p: Option<&str>) -> Option<f32> {
    let rotulo = p?;
    corte::PROPORCOES
        .iter()
        .find(|(r, _)| *r == rotulo)
        .and_then(|(_, v)| *v)
}

/// A conta mora no corte, que é quem também trava a proporção no Enquadrar.
pub use crate::revelacao::corte::proporcao_na_orientacao;

/// O corte centralizado desta foto: o maior retângulo na proporção, no meio.
/// Sem proporção (ou sem tamanho conhecido), a foto inteira.
pub fn corte_centralizado(proporcao: Option<&str>, largura: u32, altura: u32) -> Corte {
    let (l, a) = (largura as f32, altura as f32);
    if l <= 0. || a <= 0. {
        return Corte::default();
    }
    let Some(p) = proporcao_na_orientacao(valor_da_proporcao(proporcao), l, a) else {
        return Corte::default();
    };
    let inteira = corte::foto_inteira();
    let espaco = (l, a);
    let r = corte::com_proporcao_no_centro(corte::retangulo_de(&inteira, espaco), p, espaco, 0.);
    let recortado = corte::com_retangulo(&inteira, r, espaco);
    if corte::e_inteiro(&recortado) {
        return Corte::default();
    }
    Corte {
        x: Some(recortado.crop_x()),
        y: Some(recortado.crop_y()),
        largura: Some(recortado.crop_width()),
        altura: Some(recortado.crop_height()),
        rotacao: Some(recortado.rotation_90()),
        angulo: Some(recortado.angle()),
        espelho_h: Some(recortado.flip_horizontal()),
        espelho_v: Some(recortado.flip_vertical()),
        perspectiva: None,
        restringir: None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn todo_preset_do_sistema_tem_chave_do_site() {
        // As da pasta "LRs" vão com `id_do_lightroom` (`presets_da_sessao`).
        for preset in use_cases::presets::presets_de_sistema()
            .into_iter()
            .filter(|p| p.grupo.is_none())
        {
            assert!(
                id_do_sistema(&preset.name).is_some(),
                "`{}` não tem chave em `ordem::IDS_DO_SISTEMA` — a sessão iria sem preset padrão",
                preset.name
            );
        }
    }

    #[test]
    fn as_favoritas_vem_primeiro_na_ordem_do_coracao() {
        let lista = presets_da_sessao(&use_cases::presets::presets_de_sistema(), Vec::new());
        let favoritas = vec![
            "sistema:vintage-portra".to_string(),
            "sistema:apagada".to_string(),
            "sistema:sepia".to_string(),
        ];
        let ordenada = favoritas_primeiro(lista.clone(), &favoritas);
        let ids: Vec<&str> = ordenada.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            &ids[..3],
            [
                "sistema:vintage-portra",
                "sistema:sepia",
                "sistema:pb-classico"
            ]
        );
        assert_eq!(ordenada.len(), lista.len(), "nenhuma some nem se repete");
    }

    #[test]
    fn a_lista_da_sessao_poe_o_sistema_antes() {
        let servidor = presets_do_servidor(&json!([
            {"id": "u1", "nome": "Meu", "ajustes": {"exposure": 0.5, "texto": "x"}}
        ]));
        let lista = presets_da_sessao(&use_cases::presets::presets_de_sistema(), servidor);
        assert_eq!(lista.first().map(|p| p.grupo), Some(Grupo::Sistema));
        let meu = lista.last().unwrap();
        assert_eq!((meu.id.as_str(), meu.grupo), ("u1", Grupo::Minhas));
        assert_eq!(meu.preset.adjustments.get("exposure"), Some(0.5));
        assert_eq!(nome_do_preset(&lista, Some("u1")).as_deref(), Some("Meu"));
        assert_eq!(
            nome_do_preset(&lista, Some("0123456789")).as_deref(),
            Some("Predefinição removida (01234567…)")
        );
    }

    #[test]
    fn os_parametros_parte_do_neutro() {
        let soma = Preset::system("x", PresetAdjustments::vazia().com("exposure", 0.3));
        let a = ajustes_dos_parametros(Some(&soma));
        assert_eq!(a.exposure, 0.3);
        assert_eq!(a.contrast, Ajustes::default().contrast);
        assert_eq!(ajustes_dos_parametros(None), Ajustes::default());
    }

    #[test]
    fn o_corte_segue_a_orientacao() {
        assert_eq!(proporcao_na_orientacao(Some(1.5), 600., 400.), Some(1.5));
        assert_eq!(
            proporcao_na_orientacao(Some(1.5), 400., 600.),
            Some(1. / 1.5)
        );
        assert_eq!(proporcao_na_orientacao(None, 400., 600.), None);

        let deitada = corte_centralizado(Some("1:1"), 600, 400);
        let (w, h) = (
            deitada.largura.unwrap() * 600.,
            deitada.altura.unwrap() * 400.,
        );
        assert!((w - h).abs() < 1., "{w}×{h}");
        assert_eq!(
            corte_centralizado(Some("livre"), 600, 400),
            Corte::default()
        );
        assert_eq!(corte_centralizado(None, 600, 400), Corte::default());
        assert_eq!(corte_centralizado(Some("3:2"), 600, 400), Corte::default());
        assert_eq!(corte_centralizado(Some("1:1"), 0, 0), Corte::default());
    }
}
