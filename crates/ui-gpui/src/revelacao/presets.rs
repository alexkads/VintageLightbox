//! Presets: um punhado de ajustes com nome, aplicados de uma vez.
//!
//! A entidade, os sete de sistema e a gravação existem nas camadas internas
//! (`domain::entities::Preset`, `use_cases::presets`) — aqui é a ponte entre o
//! mapa `nome → valor` que elas guardam e os [`Ajustes`] que o motor usa.
//!
//! ## ✅ Um preset move qualquer um dos 53 — desde 7/set/2026
//!
//! 🚨 **Eram 15**: os 11 do Básico e os 4 da curva de tons, uma coluna por
//! campo numa tabela escrita quando o motor só tinha esses ajustes. Ela não
//! cresceu junto com o shader, e o efeito era mudo — salvar uma predefinição
//! com HSL, nitidez ou tonalização gravava o nome e **descartava** os campos
//! que não tinham coluna. É por isso que "Sépia à moda antiga" não existia
//! aqui: a sépia se faz com tonalização.
//!
//! Agora é um mapa esparso (`PresetAdjustments`), com os nomes de
//! [`Ajustes::NOMES`] — os mesmos que o shader lê por posição e que o site
//! manda em `nomes.json`. **Nome desconhecido é ignorado**, e campo ausente
//! continua não sendo tocado.

use std::sync::Arc;

use adapters::controllers::PresetController;
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Preset, PresetId};

use super::processador::Ajustes;

/// Quem sabe guardar um preset novo.
///
/// A mesma forma da porta de gravação de revelação
/// ([`super::persistencia::Gravador`]) e pelas mesmas duas razões: o controller é
/// `async` do tokio, e os testes de tela precisam afirmar **o que foi salvo**.
pub trait GuardaDePresets: Send + Sync + 'static {
    /// 🚨 **Recebe o `Preset` inteiro, com o id que a tela pôs na lista.** Era
    /// nome e ajustes, e o id nascia lá dentro: a lista da tela e a tabela
    /// falavam de linhas diferentes, e renomear ou apagar mandava o comando
    /// para um id que não existe.
    fn salvar(&self, preset: Preset);
    /// Troca o nome de uma que já existe — a mesma linha, e não uma cópia.
    fn renomear(&self, id: PresetId, nome: String);
    /// Apaga uma do fotógrafo. As de sistema nascem em código e não têm linha.
    fn apagar(&self, id: PresetId);
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
    fn salvar(&self, preset: Preset) {
        let presets = self.presets.clone();
        let rotulo = preset.name.clone();

        self.tokio.spawn(async move {
            if let Err(erro) = presets.save_preset(&preset).await {
                eprintln!("⚠️ [Presets] \"{rotulo}\" não foi salvo: {erro}");
            }
        });
    }

    fn renomear(&self, id: PresetId, nome: String) {
        let presets = self.presets.clone();
        let rotulo = nome.clone();

        self.tokio.spawn(async move {
            if let Err(erro) = presets.rename_preset(&id, nome).await {
                eprintln!("⚠️ [Presets] o nome \"{rotulo}\" não foi gravado: {erro}");
            }
        });
    }

    fn apagar(&self, id: PresetId) {
        let presets = self.presets.clone();

        self.tokio.spawn(async move {
            if let Err(erro) = presets.delete_preset(&id).await {
                eprintln!("⚠️ [Presets] a predefinição {id} não foi apagada: {erro}");
            }
        });
    }
}

/// Aplica o preset sobre os ajustes de agora.
///
/// Campo ausente **não** é tocado: o preset diz o que muda, e o que ele não
/// menciona continua como está. É o que permite aplicar "Hora dourada" sobre uma
/// foto já revelada sem perder o resto do trabalho — e é o que o site faz.
///
/// ⚠️ **Nome que o motor não conhece é ignorado, e não é erro.** Um preset
/// gravado por uma versão mais nova (ou traduzido de um `.xmp` do Lightroom com
/// recurso que este motor não tem) aplica o que dá e deixa o resto quieto. O
/// contrário — recusar o preset inteiro — perderia oito ajustes bons por causa
/// de um nome desconhecido.
pub fn aplicar(ajustes: &mut Ajustes, preset: &PresetAdjustments) {
    let mut vetor = ajustes.como_vetor();
    for (campo, valor) in preset.iter() {
        if let Some(posicao) = posicao_de(campo) {
            vetor[posicao] = valor;
        }
    }
    *ajustes =
        Ajustes::de_vetor(&vetor).expect("o vetor saiu de `como_vetor`, tem o tamanho certo");
}

/// A posição de um campo no vetor do shader, pelo nome.
///
/// 🔑 **É a única ponte entre o texto que o banco guarda e o `f32` que a GPU
/// recebe.** `Ajustes::NOMES` é a mesma lista que o `struct Params` do WGSL
/// declara por posição e que o site recebe em `nomes.json`.
fn posicao_de(campo: &str) -> Option<usize> {
    Ajustes::NOMES.iter().position(|nome| *nome == campo)
}

/// O que virar preset a partir do que está na tela.
///
/// 🔑 **Só o que saiu do neutro**, como no site (`soOsAlterados`). Um preset é
/// "o que eu mexi", e não "o estado desta foto": guardar os 53 faria a segunda
/// predefinição aplicada apagar a primeira, e uma de nitidez por cima de uma de
/// cor devolveria a cor ao neutro sem dizer nada.
///
/// ⚠️ **`inteiro` é o contrário disso, e é escolha de quem salva.** É a caixa
/// "Zerar os outros ajustes ao aplicar" do site: a predefinição que é um visual
/// completo guarda os 53 — inclusive os neutros — e aplicar devolve ao neutro o
/// que ela não menciona. Não precisa de campo novo: guardar tudo já é isso.
pub fn dos_ajustes(ajustes: &Ajustes, inteiro: bool) -> PresetAdjustments {
    let neutro = Ajustes::default().como_vetor();
    let valores = ajustes.como_vetor();

    Ajustes::NOMES
        .iter()
        .enumerate()
        .filter(|(i, _)| inteiro || valores[*i] != neutro[*i])
        .map(|(i, nome)| (*nome, valores[i]))
        .collect()
}

/// Quantos dos 53 este preset escreve — o número que a lista mostra ao lado do
/// nome, como no site.
pub fn quantos_campos(preset: &Preset) -> usize {
    preset.adjustments.len()
}

/// Separa os de sistema dos do usuário, mantendo a ordem de cada grupo.
///
/// Duas listas na tela, como no site — e a divisão é por `is_system`, que vem do
/// use case: os sete de sistema são construídos ali a cada listagem, e os do
/// usuário saem da tabela `presets`.
pub fn separar(presets: &[Preset]) -> (Vec<&Preset>, Vec<&Preset>) {
    presets.iter().partition(|preset| preset.is_system)
}

/// O mesmo, deixando de fora o que a busca não alcança.
///
/// Busca vazia é "mostre tudo", e não "não mostre nada". A comparação é por
/// pedaço do nome, sem caixa — o mesmo `toLocaleLowerCase().includes()` do site.
///
/// ⚠️ **Sem dobra de acento, como no site.** "Sepia" não acha "Sépia à moda
/// antiga", e é uma diferença conhecida: fazer diferente aqui daria dois
/// resultados para a mesma digitação, dependendo da tela.
pub fn separar_filtrando<'a>(
    presets: &'a [Preset],
    busca: &str,
) -> (Vec<&'a Preset>, Vec<&'a Preset>) {
    let alvo = busca.trim().to_lowercase();
    presets
        .iter()
        .filter(|preset| alvo.is_empty() || preset.name.to_lowercase().contains(&alvo))
        .partition(|preset| preset.is_system)
}

/// Se o fotógrafo ainda não salvou nenhuma — o que decide entre "não achei" e
/// "não existe".
pub fn nenhuma_do_usuario(presets: &[Preset]) -> bool {
    !presets.iter().any(|preset| !preset.is_system)
}

/// Uma guarda que só anota o que recebeu.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::{GuardaDePresets, Preset, PresetId};

    #[derive(Default)]
    pub struct GuardaDeMentira {
        salvos: Mutex<Vec<Preset>>,
        renomeados: Mutex<Vec<(PresetId, String)>>,
        apagados: Mutex<Vec<PresetId>>,
    }

    impl GuardaDeMentira {
        pub fn salvos(&self) -> Vec<Preset> {
            self.salvos.lock().expect("o registro de presets").clone()
        }

        pub fn renomeados(&self) -> Vec<(PresetId, String)> {
            self.renomeados
                .lock()
                .expect("o registro de presets")
                .clone()
        }

        pub fn apagados(&self) -> Vec<PresetId> {
            self.apagados.lock().expect("o registro de presets").clone()
        }
    }

    impl GuardaDePresets for GuardaDeMentira {
        fn salvar(&self, preset: Preset) {
            self.salvos
                .lock()
                .expect("o registro de presets")
                .push(preset);
        }

        fn renomear(&self, id: PresetId, nome: String) {
            self.renomeados
                .lock()
                .expect("o registro de presets")
                .push((id, nome));
        }

        fn apagar(&self, id: PresetId) {
            self.apagados
                .lock()
                .expect("o registro de presets")
                .push(id);
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
            &PresetAdjustments::vazia().com("temperature", 5.0),
        );

        assert_eq!(ajustes.temperature, 5.0);
        assert_eq!(ajustes.exposure, 1.0, "o que o preset não menciona fica");
        assert_eq!(ajustes.saturation, 0.5);
        assert_eq!(ajustes.hsl_blue_lum, -30.0);
    }

    /// ✅ **Agora um preset alcança HSL, nitidez, lente, tonalização e grão.**
    ///
    /// 🚨 Este teste era `os_quinze_campos_atravessam_a_ida_e_a_volta`, e a
    /// última asserção dele afirmava o defeito: *"os 31 de fora do preset não
    /// viajam"*. Viajavam para lugar nenhum porque a tabela não tinha coluna
    /// para eles — salvar era perder, calado.
    #[test]
    fn os_53_campos_atravessam_a_ida_e_a_volta() {
        let original = Ajustes {
            exposure: 1.25,
            contrast: 1.4,
            temperature: -3.0,
            hsl_red_sat: 60.0,
            hsl_blue_lum: -20.0,
            sharpen_amount: 40.0,
            lens_vignette_amount: -25.0,
            split_shadow_hue: 35.0,
            split_shadow_sat: 45.0,
            grain_amount: 30.0,
            ..Default::default()
        };

        let preset = dos_ajustes(&original, false);
        let mut destino = Ajustes::default();
        aplicar(&mut destino, &preset);

        assert_eq!(destino, original, "os 53, campo a campo");
    }

    /// ⚠️ **Só o que saiu do neutro vira preset** — é o que o site guarda.
    ///
    /// Guardar os 53 sempre faria a segunda predefinição aplicada apagar a
    /// primeira: uma de nitidez por cima de uma de cor devolveria a cor ao
    /// neutro, sem que nada acusasse.
    #[test]
    fn o_preset_guarda_so_o_que_saiu_do_neutro() {
        let ajustes = Ajustes {
            exposure: 1.25,
            hsl_red_sat: 60.0,
            ..Default::default()
        };

        let preset = dos_ajustes(&ajustes, false);
        assert_eq!(preset.len(), 2);
        assert_eq!(preset.get("exposure"), Some(1.25));
        assert_eq!(preset.get("hsl_red_sat"), Some(60.0));
        assert_eq!(
            preset.get("contrast"),
            None,
            "o contraste está no neutro (1,0), e neutro não é alteração"
        );
    }

    /// E o preset "inteiro" guarda os 53 — a caixa "Zerar os outros ajustes ao
    /// aplicar" do site. Aplicar um destes devolve ao neutro o que ele não
    /// menciona, porque ele menciona tudo.
    #[test]
    fn o_preset_inteiro_guarda_os_53_e_apaga_o_que_havia() {
        let visual = dos_ajustes(
            &Ajustes {
                saturation: -1.0,
                ..Default::default()
            },
            true,
        );
        assert_eq!(visual.len(), 53);

        let mut destino = Ajustes {
            exposure: 2.0,
            hsl_red_sat: 60.0,
            ..Default::default()
        };
        aplicar(&mut destino, &visual);

        assert_eq!(destino.saturation, -1.0);
        assert_eq!(destino.exposure, 0.0, "o que ele não pediu volta ao neutro");
        assert_eq!(destino.hsl_red_sat, 0.0);
        assert_eq!(destino.contrast, 1.0, "e neutro é 1,0, não zero");
    }

    /// ⚠️ Aplicar preset **por cima** de outro não acumula: o segundo manda nos
    /// campos que ele traz.
    #[test]
    fn o_segundo_preset_manda_nos_campos_que_traz() {
        let mut ajustes = Ajustes::default();

        aplicar(
            &mut ajustes,
            &PresetAdjustments::vazia()
                .com("temperature", 5.0)
                .com("saturation", -1.0),
        );
        aplicar(
            &mut ajustes,
            &PresetAdjustments::vazia().com("temperature", -5.0),
        );

        assert_eq!(ajustes.temperature, -5.0);
        assert_eq!(
            ajustes.saturation, -1.0,
            "o que o segundo não traz sobrevive"
        );
    }

    /// 🚨 **Nome que o motor não conhece não derruba o preset.**
    ///
    /// O banco guarda texto, e nada impede um campo escrito por uma versão mais
    /// nova — ou traduzido de um `.xmp` do Lightroom com recurso que este motor
    /// não tem. Recusar o preset inteiro por causa de um nome perderia os oito
    /// ajustes bons junto com o desconhecido.
    #[test]
    fn campo_desconhecido_e_ignorado_sem_levar_os_outros() {
        let mut ajustes = Ajustes::default();

        aplicar(
            &mut ajustes,
            &PresetAdjustments::vazia()
                .com("dehaze", 40.0)
                .com("exposure", 1.0),
        );

        assert_eq!(ajustes.exposure, 1.0);
    }

    /// 🔑 **Toda predefinição de sistema escreve num campo que o motor tem.**
    ///
    /// Os sete nascem no `use-cases`, que não conhece o `revelacao-core` — o
    /// nome de campo lá é texto solto, e um erro de digitação em
    /// `"split_shadow_hue"` daria uma predefinição que aplica **quase** tudo,
    /// sem erro nenhum. Este é o único lugar do workspace que vê as duas listas.
    #[test]
    fn nenhuma_predefinicao_de_sistema_escreve_em_campo_inventado() {
        for preset in use_cases::presets::presets_de_sistema() {
            for campo in preset.adjustments.campos() {
                assert!(
                    posicao_de(campo).is_some(),
                    "\"{}\" escreve em `{campo}`, que não está em `Ajustes::NOMES`",
                    preset.name
                );
            }
        }
    }

    /// ⚠️ **Busca vazia mostra tudo.** O engano fácil é filtrar por
    /// `contains("")` num caminho e por igualdade noutro: com a lista vazia ao
    /// abrir o painel, o operador conclui que perdeu as predefinições.
    #[test]
    fn a_busca_vazia_mostra_as_duas_listas_inteiras() {
        let presets = vec![
            Preset::system("Sépia à moda antiga", PresetAdjustments::vazia()),
            Preset::user("Meu retrato".into(), PresetAdjustments::vazia()),
        ];

        let (sistema, usuario) = separar_filtrando(&presets, "   ");
        assert_eq!(sistema.len(), 1);
        assert_eq!(usuario.len(), 1);
    }

    /// E ela acha por pedaço do nome, sem caixa, nos dois grupos ao mesmo
    /// tempo.
    #[test]
    fn a_busca_acha_por_pedaco_do_nome_nos_dois_grupos() {
        let presets = vec![
            Preset::system("Preto e branco clássico", PresetAdjustments::vazia()),
            Preset::system("Hora dourada", PresetAdjustments::vazia()),
            Preset::user("Dourado meu".into(), PresetAdjustments::vazia()),
        ];

        let (sistema, usuario) = separar_filtrando(&presets, "DOURAD");
        assert_eq!(
            sistema.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Hora dourada"]
        );
        assert_eq!(
            usuario.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Dourado meu"]
        );
    }

    /// 🔑 "Não achei" e "não tenho nenhuma" são mensagens diferentes, e a
    /// segunda explica como criar a primeira.
    #[test]
    fn nenhuma_do_usuario_olha_a_lista_inteira_e_nao_a_filtrada() {
        let so_de_sistema = vec![Preset::system("Hora dourada", PresetAdjustments::vazia())];
        assert!(nenhuma_do_usuario(&so_de_sistema));

        let com_uma_minha = vec![
            Preset::system("Hora dourada", PresetAdjustments::vazia()),
            Preset::user("Meu".into(), PresetAdjustments::vazia()),
        ];
        assert!(!nenhuma_do_usuario(&com_uma_minha));
    }

    #[test]
    fn separar_respeita_a_ordem_de_cada_grupo() {
        let presets = vec![
            Preset::system("Warm", PresetAdjustments::vazia()),
            Preset::user("Meu".into(), PresetAdjustments::vazia()),
            Preset::system("Cool", PresetAdjustments::vazia()),
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
