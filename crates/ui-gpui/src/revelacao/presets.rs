//! Presets: um punhado de ajustes com nome, aplicados de uma vez.
//!
//! A entidade, os oito de sistema e a gravação existem nas camadas internas
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

use std::cmp::Ordering;
use std::sync::Arc;

use adapters::controllers::PresetController;
use domain::entities::preset::PresetAdjustments;
use domain::entities::{Preset, PresetId};

use super::processador::Ajustes;

/// A ordem escolhida por quem opera (`ordem-dos-presets.ts`).
pub mod ordem;
/// O rótulo de cada ajuste, para o resumo.
mod rotulos;

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

/// A predefinição liga algum módulo do darktable?
///
/// 🚨 **Um estilo do darktable só é fiel partindo do neutro** (dono,
/// 2026-09-12: *"a importação de .dtstyle deveria resetar todo o efeito para
/// ficar fiel"*). O estilo foi medido contra o darktable sobre a foto crua;
/// somado a um contraste ou a uma tonalização que já estivessem na foto, ele
/// vira uma terceira imagem.
///
/// 🔑 **Decidido pelo conteúdo, e não por uma coluna** — o `ehEstiloDoDarktable`
/// do site (`presets.ts`). O banco não guarda `replaces` para as do operador, e
/// todo estilo lido de um `.dtstyle` ou `.xmp` do darktable liga pelo menos um
/// `dt_*_ativo`. Vale também para os já importados, sem migration.
pub fn eh_estilo_do_darktable(ajustes: &PresetAdjustments) -> bool {
    ajustes.iter().any(|(campo, valor)| {
        campo.len() > "dt__ativo".len()
            && campo.starts_with("dt_")
            && campo.ends_with("_ativo")
            && valor != 0.0
    })
}

/// Esta predefinição recomeça do neutro em vez de somar?
///
/// As do sistema trazem a marca no código (`Preset::replaces`); as do operador,
/// pelo conteúdo ([`eh_estilo_do_darktable`]).
pub fn substitui(preset: &Preset) -> bool {
    preset.replaces || eh_estilo_do_darktable(&preset.adjustments)
}

/// Os ajustes com a predefinição aplicada.
///
/// 🚨 **A mesma conta para a prévia e para o clique** — `editor.tsx`:
/// `{...(previa.substitui ? PADRAO : ajustes), ...previa.campos}`. A prévia
/// somava sempre, e o mouse mostrava uma foto que o clique não dava: "Preto e
/// branco" sobre uma sépia aparecia âmbar no ponteiro e cinza depois do clique.
pub fn aplicado(ajustes: &Ajustes, preset: &Preset) -> Ajustes {
    let mut saida = if substitui(preset) {
        Ajustes::default()
    } else {
        *ajustes
    };
    aplicar(&mut saida, &preset.adjustments);
    saida
}

/// Os rótulos dos controles que a predefinição move — o `resumir` do site.
///
/// Até quatro nomes; passando disso, os quatro primeiros e "e mais N". A ordem é
/// a do painel do site ([`rotulos::ROTULOS`]), e campo no neutro não conta.
pub fn resumir(ajustes: &PresetAdjustments) -> String {
    let neutro = Ajustes::default().como_vetor();
    let nomes: Vec<&str> = rotulos::ROTULOS
        .iter()
        .filter(|(campo, _)| match (ajustes.get(campo), posicao_de(campo)) {
            (Some(valor), Some(i)) => valor != neutro[i],
            _ => false,
        })
        .map(|(_, rotulo)| *rotulo)
        .collect();
    match nomes.len() {
        0 => "nada".to_string(),
        1..=4 => nomes.join(", "),
        n => format!("{} e mais {}", nomes[..4].join(", "), n - 4),
    }
}

/// O número que o texto do formulário cita ao guardar "todos".
///
/// ⚠️ **É o do site, e está velho lá**: o motor tem 171 ajustes e o
/// `painel-presets.tsx` ainda escreve "os 46". Fica igual de propósito — a regra
/// é o mesmo texto nas duas plataformas —, e corrigir é mudar os dois lados.
pub const TODOS_NO_TEXTO: usize = 46;

/// A frase sob a caixa "Zerar os outros ajustes ao aplicar": o que a
/// predefinição nova vai guardar.
///
/// `alterados` é o que está fora do neutro na foto ([`dos_ajustes`] sem
/// `inteiro`).
pub fn o_que_guarda(alterados: &PresetAdjustments, inteiro: bool) -> String {
    let quantos = alterados.len();
    match (inteiro, quantos) {
        (true, 0) => format!(
            "Guarda os {TODOS_NO_TEXTO} ajustes no neutro: aplicar devolve a foto ao original."
        ),
        (true, _) => format!(
            "Guarda os {TODOS_NO_TEXTO} ajustes — {} e o neutro do resto.",
            resumir(alterados)
        ),
        (false, 0) => "Nenhum ajuste fora do neutro: não há o que guardar.".to_string(),
        (false, 1) => format!("Guarda 1 ajuste: {}.", resumir(alterados)),
        (false, n) => format!("Guarda {n} ajustes: {}.", resumir(alterados)),
    }
}

/// Se o "Salvar" do formulário está ligado: com nome, e com algo a guardar.
///
/// Sem nada fora do neutro, só a caixa "zerar os outros" dá sentido a salvar —
/// é a predefinição que devolve a foto ao original.
pub fn pode_salvar(nome: &str, quantos_alterados: usize, inteiro: bool) -> bool {
    !nome.trim().is_empty() && (quantos_alterados > 0 || inteiro)
}

/// A dica da linha: o nome e o que ele faz com o que já está na foto.
pub fn regra_da_linha(preset: &Preset) -> String {
    if substitui(preset) {
        format!(
            "{} — recomeça do neutro: substitui o tratamento que está na foto",
            preset.name
        )
    } else {
        format!("{} — soma ao tratamento que já está na foto", preset.name)
    }
}

/// A dica do nome: o nome e os controles que ele move.
pub fn dica_do_nome(preset: &Preset) -> String {
    format!("{} — {}", preset.name, resumir(&preset.adjustments))
}

/// Compara dois nomes como o `localeCompare(…, "pt-BR")` do site: sem caixa e
/// sem acento no primeiro olhar, e só então letra a letra.
///
/// "Árvore" vem antes de "Bosque", e "sépia" junto de "Sepia" — a ordem de uma
/// agenda, e não a da tabela Unicode, que poria todo acento depois do "z".
pub fn comparar_nomes(a: &str, b: &str) -> Ordering {
    let chave = |texto: &str| -> String {
        texto
            .chars()
            .flat_map(char::to_lowercase)
            .map(sem_acento)
            .collect()
    };
    chave(a).cmp(&chave(b)).then_with(|| a.cmp(b))
}

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        outro => outro,
    }
}

/// Os dois grupos da coluna, prontos para desenhar: na ordem guardada e com a
/// busca aplicada.
///
/// A busca é por pedaço do nome, sem caixa — o `toLocaleLowerCase().includes()`
/// do site —, e busca vazia é "mostre tudo". ⚠️ **Sem dobra de acento, como
/// lá**: "Sepia" não acha "Sépia à moda antiga", e fazer diferente aqui daria
/// dois resultados para a mesma digitação, dependendo da tela.
///
/// 🔑 **"Minhas" em ordem alfabética quando não há ordem guardada** — o
/// `ordenar` do site, que a lista recebe a cada criação, importação e troca de
/// nome. A do sistema fica na ordem do código, que é a do site.
pub fn da_coluna<'a>(
    presets: &'a [Preset],
    busca: &str,
    guardada: &ordem::Ordem,
) -> (Vec<&'a Preset>, Vec<&'a Preset>) {
    let (sistema, mut minhas) = separar(presets);
    minhas.sort_by(|a, b| comparar_nomes(&a.name, &b.name));
    let alvo = busca.trim().to_lowercase();
    let filtrar = |lista: Vec<&'a Preset>| -> Vec<&'a Preset> {
        lista
            .into_iter()
            .filter(|p| alvo.is_empty() || p.name.to_lowercase().contains(&alvo))
            .collect()
    };
    let chave = |p: &&Preset| ordem::chave(p);
    (
        filtrar(ordem::aplicar_ordem(
            sistema,
            guardada.do_grupo(ordem::Grupo::Sistema),
            chave,
        )),
        filtrar(ordem::aplicar_ordem(
            minhas,
            guardada.do_grupo(ordem::Grupo::Minhas),
            chave,
        )),
    )
}

/// Separa os de sistema dos do usuário, mantendo a ordem de cada grupo.
///
/// Duas listas na tela, como no site — e a divisão é por `is_system`, que vem do
/// use case: os sete de sistema são construídos ali a cada listagem, e os do
/// usuário saem da tabela `presets`.
pub fn separar(presets: &[Preset]) -> (Vec<&Preset>, Vec<&Preset>) {
    presets.iter().partition(|preset| preset.is_system)
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

    /// ✅ **Um preset alcança todos os ajustes do motor, e não só os do Básico.**
    ///
    /// 🚨 Este teste era `os_quinze_campos_atravessam_a_ida_e_a_volta`, e a
    /// última asserção dele afirmava o defeito: *"os 31 de fora do preset não
    /// viajavam"*. Depois virou "os 53", com dez campos escolhidos à mão — e os
    /// 118 que o motor ganhou depois (calibração, P&B, curva por ponto, Controles
    /// RGB) nunca foram conferidos. Agora a fonte é `Ajustes::NOMES`: cada ajuste
    /// recebe um valor que só ele tem, e tem de voltar com ele.
    #[test]
    fn todos_os_campos_atravessam_a_ida_e_a_volta() {
        let vetor: Vec<f32> = (0..Ajustes::NOMES.len())
            .map(|i| 1_000.0 + i as f32)
            .collect();
        let original = Ajustes::de_vetor(&vetor).expect("o vetor tem o tamanho de `NOMES`");

        let preset = dos_ajustes(&original, false);
        assert_eq!(
            preset.len(),
            Ajustes::NOMES.len(),
            "nenhum estava no neutro"
        );
        let mut destino = Ajustes::default();
        aplicar(&mut destino, &preset);

        let destino = destino.como_vetor();
        for (i, nome) in Ajustes::NOMES.iter().enumerate() {
            assert_eq!(destino[i], vetor[i], "`{nome}` não atravessou o preset");
        }
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

    /// E o preset "inteiro" guarda **todos** os ajustes do motor — a caixa "Zerar
    /// os outros ajustes ao aplicar" do site. Aplicar um destes devolve ao neutro
    /// o que ele não menciona, porque ele menciona tudo.
    ///
    /// ⚠️ A conta era `53`, e quebrou quando o motor passou a 171: o código já
    /// guardava todos, e o número escrito no teste é que tinha ficado para trás.
    #[test]
    fn o_preset_inteiro_guarda_todos_e_apaga_o_que_havia() {
        let visual = dos_ajustes(
            &Ajustes {
                saturation: -1.0,
                ..Default::default()
            },
            true,
        );
        assert_eq!(visual.len(), Ajustes::NOMES.len());
        for nome in Ajustes::NOMES {
            assert!(
                visual.get(nome).is_some(),
                "o preset inteiro não guardou `{nome}`"
            );
        }

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

        let (sistema, usuario) = da_coluna(&presets, "   ", &ordem::Ordem::default());
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

        let (sistema, usuario) = da_coluna(&presets, "DOURAD", &ordem::Ordem::default());
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

    fn sistema(nome: &str) -> Preset {
        use_cases::presets::presets_de_sistema()
            .into_iter()
            .find(|p| p.name == nome)
            .unwrap_or_else(|| panic!("\"{nome}\" sumiu da lista do sistema"))
    }

    /// 🚨 **O estilo do darktable recomeça do neutro** (dono, 2026-09-12), e a
    /// do Lightroom soma — `presets.test.ts`. Um interruptor em zero não é
    /// estilo nenhum.
    #[test]
    fn a_importada_do_darktable_substitui_e_a_do_lightroom_soma() {
        let do_darktable = Preset::user(
            "dt".into(),
            PresetAdjustments::vazia()
                .com("dt_exposure_ativo", 1.0)
                .com("dt_exposure_exposure", 0.163),
        );
        let do_lightroom = Preset::user(
            "lr".into(),
            PresetAdjustments::vazia()
                .com("contrast", 1.2)
                .com("split_shadow_hue", 35.0),
        );
        let desligado = Preset::user(
            "desligado".into(),
            PresetAdjustments::vazia()
                .com("dt_cb_ativo", 0.0)
                .com("exposure", 0.2),
        );

        assert!(substitui(&do_darktable));
        assert!(!substitui(&do_lightroom));
        assert!(!substitui(&desligado));
    }

    /// 🚨 **A prévia e o clique dão a mesma foto.** "Preto e branco" sobre uma
    /// sépia: somando, a tonalização âmbar ficava de pé no ponteiro.
    #[test]
    fn o_que_substitui_parte_do_neutro_e_o_que_soma_parte_da_foto() {
        let sepia = aplicado(&Ajustes::default(), &sistema("Sépia à moda antiga"));
        assert_eq!(sepia.split_shadow_sat, 45.0);

        let pb = aplicado(&sepia, &sistema("Preto e branco clássico"));
        assert_eq!(pb.split_shadow_sat, 0.0, "a tonalização da sépia sai");
        assert_eq!(pb.saturation, -1.0);

        let nitida = aplicado(&sepia, &sistema("Nitidez para impressão"));
        assert_eq!(nitida.split_shadow_sat, 45.0, "a nitidez soma");
        assert_eq!(nitida.sharpen_amount, 55.0);
    }

    // ------------------------------------ presets-do-sistema.test.ts

    #[test]
    fn as_do_sistema_sao_oito_com_nomes_distintos() {
        let lista = use_cases::presets::presets_de_sistema();
        assert_eq!(lista.len(), 8);
        let nomes: std::collections::HashSet<_> = lista.iter().map(|p| &p.name).collect();
        assert_eq!(nomes.len(), lista.len());
        assert!(lista.iter().all(|p| p.is_system));
    }

    /// Gravar um campo já neutro faria a predefinição "reencostar" o controle
    /// em vez de deixá-lo como está — e duas aplicadas em sequência deixariam de
    /// somar.
    #[test]
    fn nenhuma_do_sistema_guarda_campo_no_neutro() {
        let neutro = Ajustes::default().como_vetor();
        for preset in use_cases::presets::presets_de_sistema() {
            for (campo, valor) in preset.adjustments.iter() {
                let i = posicao_de(campo).expect("campo do motor");
                assert_ne!(
                    valor, neutro[i],
                    "\"{}\" guarda `{campo}` no neutro",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn as_que_definem_o_visual_recomecam_do_neutro_e_a_nitidez_soma() {
        for nome in [
            "Preto e branco clássico",
            "Sépia à moda antiga",
            "Retrato suave",
            "Luz de estúdio",
            "Hora dourada",
            "Alta-chave",
            "RecordarFotos P&B",
        ] {
            assert!(substitui(&sistema(nome)), "{nome} define o visual");
        }
        assert!(!substitui(&sistema("Nitidez para impressão")));
    }

    /// Ele conta com o `replaces` para apagar a tonalização da sépia; escrever
    /// `split_*: 0` aqui obrigaria cada predefinição nova a lembrar de zerar
    /// tudo o que as outras escrevem.
    #[test]
    fn o_preto_e_branco_nao_carrega_tonalizacao_nenhuma() {
        let pb = sistema("Preto e branco clássico");
        assert!(pb.adjustments.campos().all(|c| !c.starts_with("split_")));
    }

    // ----------------------------------------- o texto da coluna

    #[test]
    fn o_resumo_diz_ate_quatro_nomes_e_conta_o_resto() {
        assert_eq!(resumir(&PresetAdjustments::vazia()), "nada");
        assert_eq!(
            resumir(&PresetAdjustments::vazia().com("contrast", 1.0)),
            "nada",
            "o neutro não conta"
        );
        assert_eq!(
            resumir(
                &PresetAdjustments::vazia()
                    .com("shadows", 10.0)
                    .com("exposure", 0.5)
            ),
            "Exposição, Sombras",
            "na ordem do painel, e não na do mapa"
        );
        assert_eq!(
            resumir(&sistema("Retrato suave").adjustments),
            "Contraste, Altas luzes, Sombras, Textura e mais 5"
        );
        assert_eq!(
            resumir(&sistema("RecordarFotos P&B").adjustments),
            "Ligar, Exposição (EV), Correção do nível de preto, Ligar e mais 20"
        );
    }

    #[test]
    fn o_formulario_diz_o_que_vai_guardar() {
        let vazio = PresetAdjustments::vazia();
        let um = PresetAdjustments::vazia().com("exposure", 0.5);
        let dois = um.clone().com("shadows", 10.0);

        assert_eq!(
            o_que_guarda(&vazio, false),
            "Nenhum ajuste fora do neutro: não há o que guardar."
        );
        assert_eq!(o_que_guarda(&um, false), "Guarda 1 ajuste: Exposição.");
        assert_eq!(
            o_que_guarda(&dois, false),
            "Guarda 2 ajustes: Exposição, Sombras."
        );
        assert_eq!(
            o_que_guarda(&vazio, true),
            "Guarda os 46 ajustes no neutro: aplicar devolve a foto ao original."
        );
        assert_eq!(
            o_que_guarda(&dois, true),
            "Guarda os 46 ajustes — Exposição, Sombras e o neutro do resto."
        );
    }

    /// 🚨 **Salvar sem nome, ou sem nada a guardar, não liga.** O diálogo
    /// antigo tinha o OK sempre aceso e gravava uma predefinição vazia.
    #[test]
    fn salvar_so_liga_com_nome_e_com_algo_a_guardar() {
        assert!(!pode_salvar("   ", 3, false));
        assert!(!pode_salvar("Quente", 0, false));
        assert!(
            pode_salvar("Quente", 0, true),
            "zerar os outros é algo a guardar"
        );
        assert!(pode_salvar("Quente", 2, false));
    }

    #[test]
    fn a_dica_da_linha_diz_se_recomeca_ou_soma() {
        assert_eq!(
            regra_da_linha(&sistema("Sépia à moda antiga")),
            "Sépia à moda antiga — recomeça do neutro: substitui o tratamento que está na foto"
        );
        assert_eq!(
            regra_da_linha(&sistema("Nitidez para impressão")),
            "Nitidez para impressão — soma ao tratamento que já está na foto"
        );
        assert_eq!(
            dica_do_nome(&sistema("Nitidez para impressão")),
            "Nitidez para impressão — Textura, Ruído (luminância), Nitidez, Raio da nitidez"
        );
    }

    /// "Minhas" vêm em ordem alfabética — sem caixa e sem acento, como o
    /// `localeCompare` do site —, e a ordem guardada manda quando existe.
    #[test]
    fn minhas_em_ordem_alfabetica_e_a_guardada_manda() {
        let presets = vec![
            Preset::system("Hora dourada", PresetAdjustments::vazia()),
            Preset::user("zebra".into(), PresetAdjustments::vazia()),
            Preset::user("Árvore".into(), PresetAdjustments::vazia()),
            Preset::user("bosque".into(), PresetAdjustments::vazia()),
        ];
        let nomes = |lista: Vec<&Preset>| lista.iter().map(|p| p.name.clone()).collect::<Vec<_>>();

        let (_, minhas) = da_coluna(&presets, "", &ordem::Ordem::default());
        assert_eq!(nomes(minhas), ["Árvore", "bosque", "zebra"]);

        let mut guardada = ordem::Ordem::default();
        guardada.definir(ordem::Grupo::Minhas, Some(vec![presets[1].id.to_string()]));
        let (_, minhas) = da_coluna(&presets, "", &guardada);
        assert_eq!(nomes(minhas), ["zebra", "Árvore", "bosque"]);

        // A busca filtra depois de ordenar.
        let (_, minhas) = da_coluna(&presets, "O", &guardada);
        assert_eq!(nomes(minhas), ["Árvore", "bosque"]);
    }
}
