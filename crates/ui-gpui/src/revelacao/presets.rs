//! Presets: um punhado de ajustes com nome, aplicados de uma vez.
//!
//! A entidade, os cinco de sistema e a gravação já existem nas camadas internas
//! (`domain::entities::Preset`, `use_cases::presets`), intactas — aqui é só a
//! ponte entre [`PresetAdjustments`] e os [`Ajustes`] que o motor usa.
//!
//! ## ⚠️ Um preset move 15 dos 46 ajustes
//!
//! `PresetAdjustments` tem os 11 do Básico e os 4 da curva de tons. HSL, lente,
//! ruído e nitidez não estão lá — aplicar um preset **não zera** o que eles têm
//! hoje, e é o comportamento do legado (`dock_viewer::apply_preset` só escreve o
//! que o preset traz, campo a campo, e só quando é `Some`).
//!
//! 🔑 É a mesma contagem de 15 do exportador, e não é coincidência: as duas
//! listas foram escritas quando o app só tinha esses ajustes, e nenhuma das duas
//! cresceu junto com o shader.

use std::sync::Arc;

use adapters::controllers::PresetController;
use domain::entities::preset::PresetAdjustments;
use domain::entities::Preset;

use super::processador::Ajustes;

/// Quem sabe guardar um preset novo.
///
/// A mesma forma da porta de gravação de revelação
/// ([`super::persistencia::Gravador`]) e pelas mesmas duas razões: o controller é
/// `async` do tokio, e os testes de tela precisam afirmar **o que foi salvo**.
pub trait GuardaDePresets: Send + Sync + 'static {
    fn salvar(&self, nome: String, ajustes: PresetAdjustments);
}

/// A guarda de verdade: entrega ao `PresetController`, numa tarefa do tokio.
pub struct GuardaDoBanco {
    presets: Arc<PresetController>,
    tokio: tokio::runtime::Handle,
}

impl GuardaDoBanco {
    pub fn nova(presets: Arc<PresetController>, tokio: tokio::runtime::Handle) -> Self {
        Self { presets, tokio }
    }
}

impl GuardaDePresets for GuardaDoBanco {
    fn salvar(&self, nome: String, ajustes: PresetAdjustments) {
        let presets = self.presets.clone();
        let rotulo = nome.clone();

        self.tokio.spawn(async move {
            if let Err(erro) = presets.save_preset(nome, ajustes).await {
                eprintln!("⚠️ [Presets] \"{rotulo}\" não foi salvo: {erro}");
            }
        });
    }
}

/// Aplica o preset sobre os ajustes de agora.
///
/// Campo `None` **não** é tocado: o preset diz o que muda, e o que ele não
/// menciona continua como está. É o que permite aplicar "Warm" sobre uma foto já
/// revelada sem perder o resto do trabalho.
pub fn aplicar(ajustes: &mut Ajustes, preset: &PresetAdjustments) {
    macro_rules! escrever {
        ($($campo:ident),* $(,)?) => {
            $(if let Some(valor) = preset.$campo {
                ajustes.$campo = valor;
            })*
        };
    }

    escrever!(
        exposure,
        contrast,
        temperature,
        tint,
        highlights,
        shadows,
        whites,
        blacks,
        clarity,
        vibrance,
        saturation,
        tone_curve_shadows,
        tone_curve_darks,
        tone_curve_lights,
        tone_curve_highlights,
    );
}

/// O que virar preset a partir do que está na tela.
///
/// Todos os 15 como `Some`, e não só o que difere do neutro: um preset que
/// gravasse apenas o alterado se comportaria diferente conforme a foto em que
/// foi criado — "Warm" feito numa foto contrastada carregaria o contraste dela;
/// feito numa foto neutra, não. O legado grava a partir de `&Photo`, que também
/// leva os campos todos.
pub fn dos_ajustes(ajustes: &Ajustes) -> PresetAdjustments {
    PresetAdjustments {
        exposure: Some(ajustes.exposure),
        contrast: Some(ajustes.contrast),
        temperature: Some(ajustes.temperature),
        tint: Some(ajustes.tint),
        highlights: Some(ajustes.highlights),
        shadows: Some(ajustes.shadows),
        whites: Some(ajustes.whites),
        blacks: Some(ajustes.blacks),
        clarity: Some(ajustes.clarity),
        vibrance: Some(ajustes.vibrance),
        saturation: Some(ajustes.saturation),
        tone_curve_shadows: Some(ajustes.tone_curve_shadows),
        tone_curve_darks: Some(ajustes.tone_curve_darks),
        tone_curve_lights: Some(ajustes.tone_curve_lights),
        tone_curve_highlights: Some(ajustes.tone_curve_highlights),
    }
}

/// Separa os de sistema dos do usuário, mantendo a ordem de cada grupo.
///
/// Duas listas na tela, como no legado — e a divisão é por `is_system`, que vem
/// do use case: os cinco de sistema são construídos ali a cada listagem, e os do
/// usuário saem da tabela `presets`.
pub fn separar(presets: &[Preset]) -> (Vec<&Preset>, Vec<&Preset>) {
    presets.iter().partition(|preset| preset.is_system)
}

/// Uma guarda que só anota o que recebeu.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::{GuardaDePresets, PresetAdjustments};

    #[derive(Default)]
    pub struct GuardaDeMentira {
        salvos: Mutex<Vec<(String, PresetAdjustments)>>,
    }

    impl GuardaDeMentira {
        pub fn salvos(&self) -> Vec<(String, PresetAdjustments)> {
            self.salvos.lock().expect("o registro de presets").clone()
        }
    }

    impl GuardaDePresets for GuardaDeMentira {
        fn salvar(&self, nome: String, ajustes: PresetAdjustments) {
            self.salvos
                .lock()
                .expect("o registro de presets")
                .push((nome, ajustes));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_preset_escreve_so_o_que_traz() {
        let mut ajustes = Ajustes {
            exposure: 1.0,
            saturation: 0.5,
            hsl_blue_lum: -30.0,
            ..Default::default()
        };

        aplicar(
            &mut ajustes,
            &PresetAdjustments {
                temperature: Some(15.0),
                ..Default::default()
            },
        );

        assert_eq!(ajustes.temperature, 15.0);
        assert_eq!(ajustes.exposure, 1.0, "o que o preset não menciona fica");
        assert_eq!(ajustes.saturation, 0.5);
        assert_eq!(
            ajustes.hsl_blue_lum, -30.0,
            "HSL não está em PresetAdjustments — nem para escrever, nem para zerar"
        );
    }

    /// 🔑 Ida e volta: o que vira preset volta igual.
    #[test]
    fn os_quinze_campos_atravessam_a_ida_e_a_volta() {
        let original = Ajustes {
            exposure: 1.25,
            contrast: 1.4,
            temperature: -3.0,
            tint: 2.0,
            highlights: -40.0,
            shadows: 30.0,
            whites: 10.0,
            blacks: -10.0,
            clarity: 0.3,
            vibrance: 0.2,
            saturation: -0.5,
            tone_curve_shadows: 5.0,
            tone_curve_darks: -5.0,
            tone_curve_lights: 8.0,
            tone_curve_highlights: -8.0,
            // Um dos 31 que o preset não carrega, para provar que ele não volta.
            hsl_red_sat: 60.0,
            ..Default::default()
        };

        let preset = dos_ajustes(&original);
        let mut destino = Ajustes::default();
        aplicar(&mut destino, &preset);

        assert_eq!(destino.exposure, 1.25);
        assert_eq!(destino.contrast, 1.4);
        assert_eq!(destino.saturation, -0.5);
        assert_eq!(destino.tone_curve_highlights, -8.0);
        assert_eq!(
            destino.hsl_red_sat, 0.0,
            "os 31 de fora do preset não viajam — inclusive quando a origem os tinha"
        );
    }

    /// ⚠️ Aplicar preset **por cima** de outro não acumula: o segundo manda nos
    /// campos que ele traz.
    #[test]
    fn o_segundo_preset_manda_nos_campos_que_traz() {
        let mut ajustes = Ajustes::default();

        aplicar(
            &mut ajustes,
            &PresetAdjustments {
                temperature: Some(15.0),
                saturation: Some(-1.0),
                ..Default::default()
            },
        );
        aplicar(
            &mut ajustes,
            &PresetAdjustments {
                temperature: Some(-15.0),
                ..Default::default()
            },
        );

        assert_eq!(ajustes.temperature, -15.0);
        assert_eq!(
            ajustes.saturation, -1.0,
            "o que o segundo não traz sobrevive"
        );
    }

    #[test]
    fn separar_respeita_a_ordem_de_cada_grupo() {
        let presets = vec![
            Preset::system("Warm", PresetAdjustments::default()),
            Preset::user("Meu".into(), PresetAdjustments::default()),
            Preset::system("Cool", PresetAdjustments::default()),
        ];

        let (sistema, usuario) = separar(&presets);
        assert_eq!(
            sistema.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Warm", "Cool"]
        );
        assert_eq!(
            usuario.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Meu"]
        );
    }
}
