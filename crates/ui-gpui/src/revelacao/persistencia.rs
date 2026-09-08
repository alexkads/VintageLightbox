//! O que a foto já traz do banco, virando [`Ajustes`].
//!
//! Sem isto a Revelação abre toda foto no neutro — inclusive as que já foram
//! reveladas. O sintoma não é "faltou uma tela": é o trabalho do fotógrafo
//! sumindo da vista, com a foto na frente dele mostrando o arquivo cru.
//!
//! ## 🔑 O neutro vem de `Ajustes::default`, não de números escritos aqui
//!
//! O legado faz `photo.edit_exposure.unwrap_or(0.0)` campo a campo — 46
//! `unwrap_or` com o padrão digitado em cada um. Dois deles não são zero
//! (`contrast` e `sharpen_radius`), e num terceiro o legado se contradiz: o meio
//! da vinheta tem **quatro** declarações de neutro espalhadas, contando a coluna
//! do banco. Aqui o padrão é sempre o campo correspondente de
//! [`Ajustes::default`]: um lugar só dizendo qual é o neutro, que é a mesma regra
//! que [`super::controles::Definicao::neutro`] segue para os sliders.

use std::sync::Arc;

use adapters::controllers::EditorController;
use adapters::view_models::PhotoViewModel;
use domain::value_objects::CropSettings;

use super::processador::Ajustes;

/// Os oito campos de corte, do jeito que o banco os guarda.
///
/// 🚨 **Eles existem aqui porque gravar ajuste apaga corte.**
/// `SavePhotoEditsUseCase::execute` recebe os oito como `Option` e a entidade faz
/// `self.edit_crop_x = crop_x` — atribuição direta, sem mesclar. Salvar uma
/// exposição passando `None` neles **apaga o corte** que a foto tinha.
///
/// A Revelação em GPUI ainda não tem crop overlay (é o que falta da fase 2), o
/// que torna o risco pior, e não melhor: sem tela de corte, ninguém aqui sabe que
/// existe corte — e mexer num slider apagaria, calado, um enquadramento feito no
/// app de egui. Por isso o corte é **lido da foto e devolvido igual**.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corte {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub largura: Option<f32>,
    pub altura: Option<f32>,
    pub rotacao: Option<i32>,
    pub angulo: Option<f32>,
    pub espelho_h: Option<bool>,
    pub espelho_v: Option<bool>,
}

/// O corte que a foto já tinha, para ser devolvido intacto na gravação.
pub fn corte_da_foto(foto: &PhotoViewModel) -> Corte {
    Corte {
        x: foto.edit_crop_x,
        y: foto.edit_crop_y,
        largura: foto.edit_crop_width,
        altura: foto.edit_crop_height,
        rotacao: foto.edit_crop_rotation,
        angulo: foto.edit_crop_angle,
        espelho_h: foto.edit_crop_flip_h,
        espelho_v: foto.edit_crop_flip_v,
    }
}

/// Quem sabe gravar uma revelação.
///
/// 🔑 **É uma porta, e não o controller direto**, por dois motivos que se somam:
/// a tela não precisa saber que existe tokio (o `EditorController` é `async` e o
/// GPUI não roda futuros de tokio), e os testes de tela precisam afirmar **o que
/// foi gravado** — inclusive que o corte voltou intacto —, o que com banco de
/// verdade seria caro e com este `trait` é uma linha.
///
/// Não devolve `Result` de propósito: gravar acontece 500 ms depois do arrasto,
/// longe de quem arrastou, e não há o que a tela faça com a falha naquele
/// momento. O que existe é registro no terminal — o legado nem isso tem
/// (`let _ = controller.save_edits(...)` nos quatro pontos que chamam).
pub trait Gravador: Send + Sync + 'static {
    fn gravar(&self, id: String, ajustes: Ajustes, corte: Corte);
}

/// O gravador de verdade: entrega ao `EditorController`, numa tarefa do tokio.
///
/// ⚠️ O `Handle` é capturado no `main`, **antes** de `Application::run` tomar a
/// thread. Sem ele, `tokio::spawn` aqui dentro entra em pânico: o GPUI roda fora
/// do contexto do runtime, e "não há reator" é o erro que aparece — no meio de um
/// arrasto, sem relação visível com o que o dedo estava fazendo.
pub struct GravadorDoBanco {
    editor: Arc<EditorController>,
    tokio: tokio::runtime::Handle,
}

impl GravadorDoBanco {
    pub fn novo(editor: Arc<EditorController>, tokio: tokio::runtime::Handle) -> Self {
        Self { editor, tokio }
    }
}

impl Gravador for GravadorDoBanco {
    fn gravar(&self, id: String, ajustes: Ajustes, corte: Corte) {
        let editor = self.editor.clone();
        let nome = id.clone();

        self.tokio.spawn(async move {
            let resultado = editor
                .save_edits(
                    id,
                    ajustes.exposure,
                    ajustes.contrast,
                    ajustes.temperature,
                    ajustes.tint,
                    ajustes.highlights,
                    ajustes.shadows,
                    ajustes.whites,
                    ajustes.blacks,
                    ajustes.clarity,
                    ajustes.vibrance,
                    ajustes.saturation,
                    ajustes.tone_curve_shadows,
                    ajustes.tone_curve_darks,
                    ajustes.tone_curve_lights,
                    ajustes.tone_curve_highlights,
                    ajustes.hsl_red_sat,
                    ajustes.hsl_orange_sat,
                    ajustes.hsl_yellow_sat,
                    ajustes.hsl_green_sat,
                    ajustes.hsl_aqua_sat,
                    ajustes.hsl_blue_sat,
                    ajustes.hsl_purple_sat,
                    ajustes.hsl_magenta_sat,
                    ajustes.hsl_red_hue,
                    ajustes.hsl_orange_hue,
                    ajustes.hsl_yellow_hue,
                    ajustes.hsl_green_hue,
                    ajustes.hsl_aqua_hue,
                    ajustes.hsl_blue_hue,
                    ajustes.hsl_purple_hue,
                    ajustes.hsl_magenta_hue,
                    ajustes.hsl_red_lum,
                    ajustes.hsl_orange_lum,
                    ajustes.hsl_yellow_lum,
                    ajustes.hsl_green_lum,
                    ajustes.hsl_aqua_lum,
                    ajustes.hsl_blue_lum,
                    ajustes.hsl_purple_lum,
                    ajustes.hsl_magenta_lum,
                    ajustes.lens_distortion,
                    ajustes.lens_vignette_amount,
                    ajustes.lens_vignette_midpoint,
                    ajustes.nr_luminance,
                    ajustes.nr_color,
                    ajustes.sharpen_amount,
                    ajustes.sharpen_radius,
                    ajustes.split_shadow_hue,
                    ajustes.split_shadow_sat,
                    ajustes.split_highlight_hue,
                    ajustes.split_highlight_sat,
                    ajustes.split_balance,
                    ajustes.grain_amount,
                    ajustes.grain_size,
                    corte.x,
                    corte.y,
                    corte.largura,
                    corte.altura,
                    corte.rotacao,
                    corte.angulo,
                    corte.espelho_h,
                    corte.espelho_v,
                )
                .await;

            if let Err(erro) = resultado {
                eprintln!("⚠️ [Revelação] a revelação de {nome} não foi gravada: {erro}");
            }
        });
    }
}

/// A tabela ajuste ↔ coluna da foto, escrita **uma** vez.
///
/// 🔑 Duas listas (uma para ler, outra para escrever) seriam duas listas para
/// esquecer um campo — e o sintoma é mudo: o ajuste some entre a Revelação e a
/// grade sem erro nenhum. `$m` recebe a lista inteira e decide a direção.
macro_rules! com_os_campos {
    ($m:ident) => {
        $m! {
        exposure <- edit_exposure,
        contrast <- edit_contrast,
        temperature <- edit_temperature,
        tint <- edit_tint,
        highlights <- edit_highlights,
        shadows <- edit_shadows,
        whites <- edit_whites,
        blacks <- edit_blacks,
        clarity <- edit_clarity,
        vibrance <- edit_vibrance,
        saturation <- edit_saturation,
        tone_curve_shadows <- edit_tone_curve_shadows,
        tone_curve_darks <- edit_tone_curve_darks,
        tone_curve_lights <- edit_tone_curve_lights,
        tone_curve_highlights <- edit_tone_curve_highlights,
        hsl_red_sat <- edit_hsl_red_sat,
        hsl_orange_sat <- edit_hsl_orange_sat,
        hsl_yellow_sat <- edit_hsl_yellow_sat,
        hsl_green_sat <- edit_hsl_green_sat,
        hsl_aqua_sat <- edit_hsl_aqua_sat,
        hsl_blue_sat <- edit_hsl_blue_sat,
        hsl_purple_sat <- edit_hsl_purple_sat,
        hsl_magenta_sat <- edit_hsl_magenta_sat,
        hsl_red_hue <- edit_hsl_red_hue,
        hsl_orange_hue <- edit_hsl_orange_hue,
        hsl_yellow_hue <- edit_hsl_yellow_hue,
        hsl_green_hue <- edit_hsl_green_hue,
        hsl_aqua_hue <- edit_hsl_aqua_hue,
        hsl_blue_hue <- edit_hsl_blue_hue,
        hsl_purple_hue <- edit_hsl_purple_hue,
        hsl_magenta_hue <- edit_hsl_magenta_hue,
        hsl_red_lum <- edit_hsl_red_lum,
        hsl_orange_lum <- edit_hsl_orange_lum,
        hsl_yellow_lum <- edit_hsl_yellow_lum,
        hsl_green_lum <- edit_hsl_green_lum,
        hsl_aqua_lum <- edit_hsl_aqua_lum,
        hsl_blue_lum <- edit_hsl_blue_lum,
        hsl_purple_lum <- edit_hsl_purple_lum,
        hsl_magenta_lum <- edit_hsl_magenta_lum,
        lens_distortion <- edit_lens_distortion,
        lens_vignette_amount <- edit_lens_vignette_amount,
        lens_vignette_midpoint <- edit_lens_vignette_midpoint,
        nr_luminance <- edit_nr_luminance,
        nr_color <- edit_nr_color,
        sharpen_amount <- edit_sharpen_amount,
        sharpen_radius <- edit_sharpen_radius,
        split_shadow_hue <- edit_split_shadow_hue,
        split_shadow_sat <- edit_split_shadow_sat,
        split_highlight_hue <- edit_split_highlight_hue,
        split_highlight_sat <- edit_split_highlight_sat,
        split_balance <- edit_split_balance,
        grain_amount <- edit_grain_amount,
        grain_size <- edit_grain_size,
        }
    };
}

/// Lê os ajustes gravados na foto. Campo ausente fica no neutro.
///
/// ⚠️ **Ausente não é zero, e não é "nunca revelada".** O legado grava os 46 de
/// uma vez, então uma foto ou tem todos ou não tem nenhum — mas ler campo a campo
/// é o que sobrevive a um `NULL` solto no banco, que nenhum dos dois apps sabe
/// produzir hoje e o SQLite aceita sem reclamar.
pub fn da_foto(foto: &PhotoViewModel) -> Ajustes {
    let mut ajustes = Ajustes::default();

    macro_rules! ler {
        ($($campo:ident <- $salvo:ident),* $(,)?) => {
            $(if let Some(valor) = foto.$salvo {
                ajustes.$campo = valor;
            })*
        };
    }

    com_os_campos!(ler);

    ajustes
}

/// O contrário de [`da_foto`]: escreve ajustes e corte **na** foto da grade.
///
/// Existe para a sincronização: a Revelação grava a receita nas marcadas e
/// precisa que as cópias que ela tem em memória digam o mesmo que o banco —
/// senão a seta seguinte abriria a foto recém-sincronizada com os sliders de
/// antes.
pub fn na_foto(foto: &mut PhotoViewModel, ajustes: Ajustes, corte: Corte) {
    macro_rules! escrever {
        ($($campo:ident <- $salvo:ident),* $(,)?) => {
            $(foto.$salvo = Some(ajustes.$campo);)*
        };
    }
    com_os_campos!(escrever);

    foto.edit_crop_x = corte.x;
    foto.edit_crop_y = corte.y;
    foto.edit_crop_width = corte.largura;
    foto.edit_crop_height = corte.altura;
    foto.edit_crop_rotation = corte.rotacao;
    foto.edit_crop_angle = corte.angulo;
    foto.edit_crop_flip_h = corte.espelho_h;
    foto.edit_crop_flip_v = corte.espelho_v;
}

/// O corte como o domínio o entende: campo ausente é a foto inteira.
pub fn para_crop_settings(corte: &Corte) -> CropSettings {
    CropSettings::new(
        corte.x.unwrap_or(0.0),
        corte.y.unwrap_or(0.0),
        corte.largura.unwrap_or(1.0),
        corte.altura.unwrap_or(1.0),
        corte.rotacao.unwrap_or(0),
        corte.angulo.unwrap_or(0.0),
        corte.espelho_h.unwrap_or(false),
        corte.espelho_v.unwrap_or(false),
    )
}

/// Se esta foto já foi revelada — algum ajuste fora do neutro, ou algum
/// enquadramento.
///
/// 🔑 **É o ponto âmbar da tira**, o mesmo do site (`reveladaEm`). Numa sessão
/// de duzentas fotos, "quais já passaram" não tem outra resposta senão abrir
/// uma a uma — e o operador que volta do café não sabe onde parou.
///
/// ⚠️ **Contra o neutro, e não contra zero.** Contraste e raio da nitidez têm
/// neutro 1,0: comparar com zero acenderia o ponto em toda foto do acervo.
pub fn ja_revelada(foto: &PhotoViewModel) -> bool {
    da_foto(foto) != Ajustes::default() || corte_da_foto(foto) != Corte::default()
}

/// Um gravador que só anota o que recebeu.
///
/// Existe para os testes de tela poderem afirmar **o que foi gravado** — que o
/// arrasto virou uma gravação só, que a foto certa foi gravada na troca, e que o
/// corte voltou intacto. Com banco de verdade, cada uma dessas seria um teste
/// caro; aqui são três linhas.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::{Ajustes, Corte, Gravador};

    #[derive(Default)]
    pub struct GravadorDeMentira {
        gravado: Mutex<Vec<(String, Ajustes, Corte)>>,
    }

    impl GravadorDeMentira {
        pub fn gravado(&self) -> Vec<(String, Ajustes, Corte)> {
            self.gravado
                .lock()
                .expect("o registro de gravações")
                .clone()
        }
    }

    impl Gravador for GravadorDeMentira {
        fn gravar(&self, id: String, ajustes: Ajustes, corte: Corte) {
            self.gravado
                .lock()
                .expect("o registro de gravações")
                .push((id, ajustes, corte));
        }
    }
}

#[cfg(test)]
mod testes {

    /// 🔑 Escrever e ler são a mesma tabela: o que entra por `na_foto` sai
    /// igual por `da_foto`, nos 53 e no corte.
    #[test]
    fn na_foto_e_da_foto_sao_inversos() {
        let ajustes = Ajustes {
            exposure: 0.7,
            hsl_magenta_lum: -12.0,
            grain_size: 33.0,
            split_balance: 15.0,
            ..Ajustes::default()
        };
        let corte = Corte {
            x: Some(0.1),
            largura: Some(0.5),
            angulo: Some(2.5),
            espelho_h: Some(true),
            ..Corte::default()
        };

        let mut foto = PhotoViewModel::default();
        assert!(!ja_revelada(&foto));
        na_foto(&mut foto, ajustes, corte);

        assert_eq!(da_foto(&foto), ajustes);
        assert_eq!(corte_da_foto(&foto), corte);
        assert!(ja_revelada(&foto));

        let dominio = para_crop_settings(&corte);
        assert_eq!(dominio.crop_x(), 0.1);
        assert_eq!(dominio.crop_width(), 0.5);
        assert_eq!(dominio.crop_height(), 1.0, "ausente é a foto inteira");
        assert!(dominio.flip_horizontal());
    }
    use super::*;

    fn foto() -> PhotoViewModel {
        PhotoViewModel {
            id: "id-retrato.jpg".into(),
            name: "retrato.jpg".into(),
            ..Default::default()
        }
    }

    /// 🚨 **Contra o neutro, e não contra zero.**
    ///
    /// Contraste e raio da nitidez têm neutro 1,0. Um `ja_revelada` que
    /// perguntasse "tem algum campo diferente de zero" acenderia o ponto âmbar
    /// em **toda** foto do acervo — inclusive nas que ninguém abriu —, e o
    /// sinal de "onde eu parei" viraria ruído no primeiro uso.
    #[test]
    fn crua_nao_conta_como_revelada() {
        assert!(!ja_revelada(&foto()));
        assert!(
            !ja_revelada(&PhotoViewModel {
                edit_contrast: Some(1.0),
                edit_sharpen_radius: Some(1.0),
                ..foto()
            }),
            "os dois neutros que não são zero"
        );
    }

    /// E um ajuste, ou só um enquadramento, já a marca: recortar é revelar
    /// tanto quanto mover a exposição, e uma foto cortada que aparecesse como
    /// intocada mandaria o operador refazer o corte.
    #[test]
    fn ajuste_ou_enquadramento_marcam_a_foto() {
        assert!(ja_revelada(&PhotoViewModel {
            edit_exposure: Some(0.5),
            ..foto()
        }));
        assert!(ja_revelada(&PhotoViewModel {
            edit_crop_width: Some(0.5),
            ..foto()
        }));
    }

    #[test]
    fn foto_sem_edicao_nenhuma_da_o_neutro() {
        assert_eq!(da_foto(&foto()), Ajustes::default());
    }

    #[test]
    fn o_que_esta_gravado_chega_aos_ajustes() {
        let salva = PhotoViewModel {
            edit_exposure: Some(1.5),
            edit_contrast: Some(1.2),
            edit_hsl_blue_lum: Some(-40.0),
            ..foto()
        };

        let ajustes = da_foto(&salva);
        assert_eq!(ajustes.exposure, 1.5);
        assert_eq!(ajustes.contrast, 1.2);
        assert_eq!(ajustes.hsl_blue_lum, -40.0);
        assert_eq!(
            ajustes.saturation, 0.0,
            "o que não foi gravado fica no neutro"
        );
    }

    /// 🚨 Campo ausente cai no **neutro do campo**, e não em zero.
    ///
    /// O contraste é o caso que pega: `None` virando `0.0` é contraste zero, que
    /// achata a foto inteira em cinza. Numa foto que tenha exposição gravada e
    /// contraste não — o que acontece se um dia algum caminho gravar parcialmente
    /// — a foto abriria cinza, e a suspeita cairia no motor de cor.
    #[test]
    fn campo_ausente_cai_no_neutro_dele_e_nao_em_zero() {
        let ajustes = da_foto(&PhotoViewModel {
            edit_exposure: Some(0.5),
            ..foto()
        });

        assert_eq!(ajustes.contrast, 1.0);
        assert_eq!(ajustes.sharpen_radius, 1.0);
    }

    /// 🔑 Um valor gravado que **é** o zero não pode ser confundido com ausência.
    ///
    /// `Some(0.0)` num campo cujo neutro é 1.0 tem de virar 0.0 — é o fotógrafo
    /// tendo arrastado o contraste até o fim, e não o banco sem resposta. Um
    /// `unwrap_or_default` fora de lugar, ou um `if valor != 0.0`, apagaria isso.
    /// 🚨 O corte da foto volta inteiro, para poder ser devolvido na gravação.
    ///
    /// Os oito campos, e não os quatro do retângulo: rotação, ângulo e os dois
    /// espelhamentos também se perdem se não forem reenviados, e a foto voltaria
    /// à orientação original sem ninguém ter pedido.
    #[test]
    fn o_corte_da_foto_volta_com_os_oito_campos() {
        let corte = corte_da_foto(&PhotoViewModel {
            edit_crop_x: Some(0.1),
            edit_crop_y: Some(0.2),
            edit_crop_width: Some(0.7),
            edit_crop_height: Some(0.6),
            edit_crop_rotation: Some(90),
            edit_crop_angle: Some(-2.5),
            edit_crop_flip_h: Some(true),
            edit_crop_flip_v: Some(false),
            ..foto()
        });

        assert_eq!(
            corte,
            Corte {
                x: Some(0.1),
                y: Some(0.2),
                largura: Some(0.7),
                altura: Some(0.6),
                rotacao: Some(90),
                angulo: Some(-2.5),
                espelho_h: Some(true),
                espelho_v: Some(false),
            }
        );
    }

    /// Foto sem corte dá corte vazio — e vazio aqui significa "não mexa".
    #[test]
    fn foto_sem_corte_da_corte_vazio() {
        assert_eq!(corte_da_foto(&foto()), Corte::default());
    }

    #[test]
    fn zero_gravado_e_diferente_de_campo_ausente() {
        let ajustes = da_foto(&PhotoViewModel {
            edit_contrast: Some(0.0),
            ..foto()
        });

        assert_eq!(ajustes.contrast, 0.0);
    }

    /// Os 53 campos estão na macro — nenhum ficou de fora na cópia.
    ///
    /// 🚨 Um campo esquecido não falha: ele simplesmente nunca volta do banco, e
    /// a foto abre com aquele ajuste no neutro. Com 53 nomes parecidos
    /// (`hsl_blue_lum` e `hsl_blue_sat` diferem em três letras), esquecer um é o
    /// erro provável — e o sintoma seria "o app novo perdeu meu HSL", meses
    /// depois.
    #[test]
    fn todos_os_campos_voltam_do_banco() {
        // Uma foto com **tudo** gravado num valor que não é o neutro de nenhum
        // campo, e a conferência de que nenhum sobrou no neutro.
        const MARCA: f32 = 7.25;
        let salva = PhotoViewModel {
            edit_exposure: Some(MARCA),
            edit_contrast: Some(MARCA),
            edit_temperature: Some(MARCA),
            edit_tint: Some(MARCA),
            edit_highlights: Some(MARCA),
            edit_shadows: Some(MARCA),
            edit_whites: Some(MARCA),
            edit_blacks: Some(MARCA),
            edit_clarity: Some(MARCA),
            edit_vibrance: Some(MARCA),
            edit_saturation: Some(MARCA),
            edit_tone_curve_shadows: Some(MARCA),
            edit_tone_curve_darks: Some(MARCA),
            edit_tone_curve_lights: Some(MARCA),
            edit_tone_curve_highlights: Some(MARCA),
            edit_hsl_red_sat: Some(MARCA),
            edit_hsl_orange_sat: Some(MARCA),
            edit_hsl_yellow_sat: Some(MARCA),
            edit_hsl_green_sat: Some(MARCA),
            edit_hsl_aqua_sat: Some(MARCA),
            edit_hsl_blue_sat: Some(MARCA),
            edit_hsl_purple_sat: Some(MARCA),
            edit_hsl_magenta_sat: Some(MARCA),
            edit_hsl_red_hue: Some(MARCA),
            edit_hsl_orange_hue: Some(MARCA),
            edit_hsl_yellow_hue: Some(MARCA),
            edit_hsl_green_hue: Some(MARCA),
            edit_hsl_aqua_hue: Some(MARCA),
            edit_hsl_blue_hue: Some(MARCA),
            edit_hsl_purple_hue: Some(MARCA),
            edit_hsl_magenta_hue: Some(MARCA),
            edit_hsl_red_lum: Some(MARCA),
            edit_hsl_orange_lum: Some(MARCA),
            edit_hsl_yellow_lum: Some(MARCA),
            edit_hsl_green_lum: Some(MARCA),
            edit_hsl_aqua_lum: Some(MARCA),
            edit_hsl_blue_lum: Some(MARCA),
            edit_hsl_purple_lum: Some(MARCA),
            edit_hsl_magenta_lum: Some(MARCA),
            edit_lens_distortion: Some(MARCA),
            edit_lens_vignette_amount: Some(MARCA),
            edit_lens_vignette_midpoint: Some(MARCA),
            edit_nr_luminance: Some(MARCA),
            edit_nr_color: Some(MARCA),
            edit_sharpen_amount: Some(MARCA),
            edit_sharpen_radius: Some(MARCA),
            edit_split_shadow_hue: Some(MARCA),
            edit_split_shadow_sat: Some(MARCA),
            edit_split_highlight_hue: Some(MARCA),
            edit_split_highlight_sat: Some(MARCA),
            edit_split_balance: Some(MARCA),
            edit_grain_amount: Some(MARCA),
            edit_grain_size: Some(MARCA),
            ..foto()
        };

        let ajustes = da_foto(&salva);
        let campos: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&ajustes));

        assert_eq!(campos.len(), std::mem::size_of::<Ajustes>() / 4);
        for (i, valor) in campos.iter().enumerate() {
            assert_eq!(
                *valor, MARCA,
                "o campo {i} não foi lido do banco — falta uma linha na macro `ler!`"
            );
        }
    }
}
