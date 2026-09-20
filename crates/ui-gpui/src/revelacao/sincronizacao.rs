//! Sincronizar: os ajustes da foto aberta vão para as outras marcadas na tira.
//!
//! É o "Synchronize Settings" do Lightroom, e o desenho é o do site
//! (`revelacao/sincronizacao.ts`, 2026-09-05, pedido do dono: *"coloque flags,
//! escolhe tudo e desmarcar algumas coisas para sincronizar"*). Este módulo é
//! a parte **pura**: o que está marcado na tira, o que viaja e o que fica, e a
//! escolha guardada entre aberturas. Quem grava e sobe é a raiz.
//!
//! 🔑 **Os grupos são os do site**, não uma divisão nova. O operador acabou de
//! mexer em "Básico" e "HSL"; a caixa que pergunta o que sincronizar tem de usar
//! os mesmos nomes, na mesma ordem, senão ele traduz mentalmente a cada uso. O
//! enquadramento anda separado porque não é ajuste — e porque é o único que
//! costuma estar errado no destino.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};

use super::controles::Painel;
use super::processador::Ajustes;

// ------------------------------------------------------------- a marcação

/// `Ctrl` no clique: entra ou sai da marcação.
///
/// 🔑 **A foto aberta não sai** — ela é a fonte da sincronização, e um lote sem
/// fonte não tem o que copiar. É a regra do site (`escolherNaTira`).
pub fn alternar(marcadas: &mut BTreeSet<usize>, aberta: usize, posicao: usize) {
    marcadas.insert(aberta);
    if posicao == aberta || !marcadas.remove(&posicao) {
        marcadas.insert(posicao);
    }
}

/// `Shift` no clique: a faixa entre a aberta e a clicada, **substituindo** o que
/// estava marcado — como no site, e ao contrário da grade da Biblioteca, que
/// soma.
pub fn faixa(aberta: usize, posicao: usize) -> BTreeSet<usize> {
    let (de, ate) = if aberta <= posicao {
        (aberta, posicao)
    } else {
        (posicao, aberta)
    };
    (de..=ate).collect()
}

/// `Cmd+A`: a tira inteira.
pub fn todas(total: usize) -> BTreeSet<usize> {
    (0..total).collect()
}

/// `Cmd+D`, e também o que sobra ao trocar de foto: só a aberta.
///
/// Trocar de foto recomeça a marcação de propósito: o lote é montado com Ctrl
/// ou Shift, e herdar o anterior sincronizaria fotos que o operador já tinha
/// esquecido de ter marcado.
pub fn so(aberta: usize) -> BTreeSet<usize> {
    BTreeSet::from([aberta])
}

// -------------------------------------------------------------- a escolha

/// Um grupo da caixa de sincronizar — os do site, na ordem do site.
///
/// ⚠️ **Desde 2026-09-17 a coluna tem painel próprio para a curva por ponto,
/// o P&B, a calibração e os cinco módulos RGB**, mas a caixa continua com os
/// doze grupos do site: `Grupo::Painel` de um desses é sinônimo do grupo
/// dedicado, e `TODOS` só usa o dedicado.
///
/// 🚨 **Não são só os sete painéis da coluna.** O site sincroniza doze grupos
/// (`GRUPOS_DA_SINCRONIZACAO`, em `revelacao/sincronizacao.ts`): os sete painéis,
/// os Controles RGB, a curva por ponto, o preto e branco, a calibração e o
/// enquadramento. Até 2026-09-13 esta lista tinha só os painéis, e o mapa por
/// nome mandava ao Básico todo ajuste que não reconhecia — 113 dos 171, entre
/// eles o estilo inteiro dos Controles RGB. Sincronizar só o Básico levava o
/// estilo junto; desmarcar o Básico o deixava para trás.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    ControlesRgb,
    Painel(Painel),
    CurvaPorPonto,
    PretoEBranco,
    Calibracao,
    Enquadramento,
}

impl Grupo {
    pub const TODOS: [Grupo; 12] = [
        Grupo::ControlesRgb,
        Grupo::Painel(Painel::Basico),
        Grupo::Painel(Painel::CurvaDeTons),
        Grupo::CurvaPorPonto,
        Grupo::Painel(Painel::Hsl),
        Grupo::PretoEBranco,
        Grupo::Painel(Painel::Detalhe),
        Grupo::Painel(Painel::Lente),
        Grupo::Calibracao,
        Grupo::Painel(Painel::Tonalizacao),
        Grupo::Painel(Painel::Efeitos),
        Grupo::Enquadramento,
    ];

    /// O grupo da caixa que leva os ajustes deste painel da coluna.
    pub fn do_painel(painel: Painel) -> Grupo {
        match painel {
            Painel::CurvaPorPonto => Grupo::CurvaPorPonto,
            Painel::PretoEBranco => Grupo::PretoEBranco,
            Painel::Calibracao => Grupo::Calibracao,
            p if p.no_rgb() => Grupo::ControlesRgb,
            p => Grupo::Painel(p),
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Grupo::ControlesRgb
            | Grupo::Painel(
                Painel::RgbExposicao
                | Painel::RgbSombrasERealces
                | Painel::RgbMonocromatico
                | Painel::RgbVinhetagem
                | Painel::RgbColorBalance,
            ) => "Controles RGB",
            Grupo::Painel(painel) => painel.rotulo(),
            Grupo::CurvaPorPonto => "Curva por ponto",
            Grupo::PretoEBranco => "Preto e branco",
            Grupo::Calibracao => "Calibração",
            Grupo::Enquadramento => "Enquadramento",
        }
    }

    /// O que ele descreve, em uma linha — some quando for óbvio.
    pub fn detalhe(self) -> Option<&'static str> {
        match self {
            Grupo::ControlesRgb
            | Grupo::Painel(
                Painel::RgbExposicao
                | Painel::RgbSombrasERealces
                | Painel::RgbMonocromatico
                | Painel::RgbVinhetagem
                | Painel::RgbColorBalance,
            ) => Some("exposição, sombras e realces, monocromático, vinhetagem e color balance"),
            Grupo::CurvaPorPonto | Grupo::Painel(Painel::CurvaPorPonto) => Some("os quatro canais"),
            Grupo::Painel(Painel::Hsl) => Some("cor, luminância e matiz das oito faixas"),
            Grupo::PretoEBranco | Grupo::Painel(Painel::PretoEBranco) => {
                Some("conversão e mixer por cor")
            }
            Grupo::Painel(Painel::Detalhe) => Some("ruído e nitidez"),
            Grupo::Painel(Painel::Lente) => Some("distorção e vinheta"),
            Grupo::Calibracao | Grupo::Painel(Painel::Calibracao) => Some("os primários da câmera"),
            Grupo::Painel(Painel::Tonalizacao) => Some("a cor das sombras e a das altas luzes"),
            Grupo::Painel(Painel::Efeitos) => Some("grão"),
            Grupo::Enquadramento => Some("giro, espelho, endireitar e recorte"),
            Grupo::Painel(Painel::Basico) | Grupo::Painel(Painel::CurvaDeTons) => None,
        }
    }

    /// Um id estável para o elemento da caixa.
    pub fn chave(self) -> &'static str {
        match self {
            Grupo::ControlesRgb
            | Grupo::Painel(
                Painel::RgbExposicao
                | Painel::RgbSombrasERealces
                | Painel::RgbMonocromatico
                | Painel::RgbVinhetagem
                | Painel::RgbColorBalance,
            ) => "controles-rgb",
            Grupo::Painel(Painel::Basico) => "basico",
            Grupo::Painel(Painel::CurvaDeTons) => "curva",
            Grupo::CurvaPorPonto | Grupo::Painel(Painel::CurvaPorPonto) => "curva-por-ponto",
            Grupo::Painel(Painel::Hsl) => "hsl",
            Grupo::PretoEBranco | Grupo::Painel(Painel::PretoEBranco) => "preto-e-branco",
            Grupo::Painel(Painel::Detalhe) => "detalhe",
            Grupo::Painel(Painel::Lente) => "lente",
            Grupo::Calibracao | Grupo::Painel(Painel::Calibracao) => "calibracao",
            Grupo::Painel(Painel::Tonalizacao) => "tonalizacao",
            Grupo::Painel(Painel::Efeitos) => "efeitos",
            Grupo::Enquadramento => "enquadramento",
        }
    }
}

/// O que viaja da foto aberta para as outras. Um `bool` por grupo.
///
/// 🚨 **Nasce com tudo marcado, menos o enquadramento.** O retângulo que
/// endireita o horizonte de uma foto corta a cabeça de outra. Ele existe na
/// lista porque às vezes é justamente o que se quer repetir — uma sequência do
/// mesmo enquadramento, uma leva torta pelo mesmo ângulo —, mas quem o liga tem
/// de escolher ligar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Escolha {
    pub controles_rgb: bool,
    pub basico: bool,
    pub curva: bool,
    pub curva_por_ponto: bool,
    pub hsl: bool,
    pub preto_e_branco: bool,
    pub detalhe: bool,
    pub lente: bool,
    pub calibracao: bool,
    pub tonalizacao: bool,
    pub efeitos: bool,
    pub enquadramento: bool,
}

impl Default for Escolha {
    /// 🔑 O `#[serde(default)]` da struct usa isto campo a campo: uma escolha
    /// gravada antes de um grupo existir abre com ele **marcado**, como o site.
    fn default() -> Self {
        Self {
            controles_rgb: true,
            basico: true,
            curva: true,
            curva_por_ponto: true,
            hsl: true,
            preto_e_branco: true,
            detalhe: true,
            lente: true,
            calibracao: true,
            tonalizacao: true,
            efeitos: true,
            enquadramento: false,
        }
    }
}

impl Escolha {
    fn campo(&mut self, grupo: Grupo) -> &mut bool {
        match grupo {
            Grupo::ControlesRgb
            | Grupo::Painel(
                Painel::RgbExposicao
                | Painel::RgbSombrasERealces
                | Painel::RgbMonocromatico
                | Painel::RgbVinhetagem
                | Painel::RgbColorBalance,
            ) => &mut self.controles_rgb,
            Grupo::Painel(Painel::Basico) => &mut self.basico,
            Grupo::Painel(Painel::CurvaDeTons) => &mut self.curva,
            Grupo::CurvaPorPonto | Grupo::Painel(Painel::CurvaPorPonto) => {
                &mut self.curva_por_ponto
            }
            Grupo::Painel(Painel::Hsl) => &mut self.hsl,
            Grupo::PretoEBranco | Grupo::Painel(Painel::PretoEBranco) => &mut self.preto_e_branco,
            Grupo::Painel(Painel::Detalhe) => &mut self.detalhe,
            Grupo::Painel(Painel::Lente) => &mut self.lente,
            Grupo::Calibracao | Grupo::Painel(Painel::Calibracao) => &mut self.calibracao,
            Grupo::Painel(Painel::Tonalizacao) => &mut self.tonalizacao,
            Grupo::Painel(Painel::Efeitos) => &mut self.efeitos,
            Grupo::Enquadramento => &mut self.enquadramento,
        }
    }

    pub fn ligado(&self, grupo: Grupo) -> bool {
        let mut copia = *self;
        *copia.campo(grupo)
    }

    pub fn alternar(&mut self, grupo: Grupo) {
        let campo = self.campo(grupo);
        *campo = !*campo;
    }

    pub fn marcar_tudo(&mut self, valor: bool) {
        for grupo in Grupo::TODOS {
            *self.campo(grupo) = valor;
        }
    }

    /// Todos ligados — o rótulo do atalho da caixa vira "Desmarcar tudo".
    pub fn tudo(&self) -> bool {
        Grupo::TODOS.into_iter().all(|g| self.ligado(g))
    }

    /// Alguma coisa a fazer? Sem nada marcado o botão de confirmar desliga.
    pub fn tem_algo(&self) -> bool {
        Grupo::TODOS.into_iter().any(|g| self.ligado(g))
    }
}

/// Em que grupo da sincronização um ajuste viaja, pelo nome do `uniform`.
///
/// 🔑 **Pelo nome, e não pela tabela de controles**: `CONTROLES` descreve os
/// sliders do desktop, e boa parte dos ajustes ainda só tem slider no site. Os
/// nomes são o contrato com o shader (`Ajustes::NOMES`), e os prefixos são os
/// grupos do site.
///
/// 🚨 **Sem "o resto vai para o Básico".** Era assim até 2026-09-13, e quando o
/// motor ganhou calibração, P&B, curva por ponto e os Controles RGB, todos eles
/// passaram a viajar como se fossem exposição. O Básico é a lista dos onze; nome
/// que nenhum grupo reconhece devolve `None` e não viaja — e
/// `cada_ajuste_viaja_em_um_grupo` falha antes de isso chegar a uma sessão.
pub fn grupo_do_ajuste(nome: &str) -> Option<Grupo> {
    const BASICO: [&str; 11] = [
        "exposure",
        "contrast",
        "temperature",
        "tint",
        "highlights",
        "shadows",
        "whites",
        "blacks",
        "clarity",
        "vibrance",
        "saturation",
    ];
    let com = |prefixo: &str| nome.starts_with(prefixo);
    let grupo = if BASICO.contains(&nome) {
        Grupo::Painel(Painel::Basico)
    } else if com("tone_curve_") {
        Grupo::Painel(Painel::CurvaDeTons)
    } else if com("curva_") {
        Grupo::CurvaPorPonto
    } else if com("hsl_") {
        Grupo::Painel(Painel::Hsl)
    } else if com("bw_") {
        Grupo::PretoEBranco
    } else if com("nr_") || com("sharpen_") {
        Grupo::Painel(Painel::Detalhe)
    } else if com("lens_") {
        Grupo::Painel(Painel::Lente)
    } else if com("calib_") {
        Grupo::Calibracao
    } else if com("split_") {
        Grupo::Painel(Painel::Tonalizacao)
    } else if com("grain_") {
        Grupo::Painel(Painel::Efeitos)
    } else if com("dt_") {
        Grupo::ControlesRgb
    } else {
        return None;
    };
    Some(grupo)
}

/// Os ajustes do destino com os grupos escolhidos trocados pelos da origem.
///
/// 🔑 **O que não foi escolhido fica como estava na foto de destino** — não
/// volta ao neutro. É a diferença entre "sincronizar" e "substituir": desmarcar
/// "Detalhe" tem de deixar a nitidez que aquela foto já tinha, e não zerá-la.
pub fn mesclar(destino: Ajustes, origem: Ajustes, escolha: &Escolha) -> Ajustes {
    let mut saida = destino.como_vetor();
    let de = origem.como_vetor();
    for (posicao, nome) in Ajustes::NOMES.iter().enumerate() {
        if grupo_do_ajuste(nome).is_some_and(|grupo| escolha.ligado(grupo)) {
            saida[posicao] = de[posicao];
        }
    }
    Ajustes::de_vetor(&saida).expect("o vetor saiu de `como_vetor`, tem o tamanho certo")
}

// ------------------------------------------------- a escolha guardada

/// Onde a escolha mora: ao lado do catálogo, como o arranjo dos painéis.
///
/// Quem sincroniza costuma sincronizar sempre do mesmo jeito, e remarcar seis
/// caixas por leva é trabalho que a tela pode poupar. É preferência de quem
/// opera, não dado do ensaio — e por isso não vai para o site.
pub fn caminho() -> PathBuf {
    AppPaths::catalog_root().join("sincronizacao.json")
}

/// Ler nunca derruba: qualquer problema devolve o padrão.
///
/// ⚠️ **Nos testes não toca o disco** (a mesma regra de `presets::ordem`): a
/// caixa do "Sincronizar N" lia e regravava o `sincronizacao.json` de quem
/// roda a suíte.
pub fn ler() -> Escolha {
    if cfg!(test) {
        return Escolha::default();
    }
    ler_de(&caminho()).unwrap_or_default()
}

pub fn ler_de(caminho: &Path) -> Option<Escolha> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Gravar falha só imprime: não poder lembrar não pode impedir de sincronizar.
pub fn gravar(escolha: &Escolha) {
    if cfg!(test) {
        return;
    }
    gravar_em(&caminho(), escolha);
}

pub fn gravar_em(caminho: &Path, escolha: &Escolha) {
    let Ok(texto) = serde_json::to_string_pretty(escolha) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(caminho, texto) {
        eprintln!("⚠️ [Revelação] a escolha de sincronizar não foi gravada: {erro}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_aberta_nunca_sai_da_marcacao() {
        let mut marcadas = so(2);
        alternar(&mut marcadas, 2, 2);
        assert_eq!(marcadas, BTreeSet::from([2]), "Ctrl na aberta não a tira");

        alternar(&mut marcadas, 2, 5);
        assert_eq!(marcadas, BTreeSet::from([2, 5]));
        alternar(&mut marcadas, 2, 5);
        assert_eq!(marcadas, BTreeSet::from([2]), "Ctrl de novo tira a outra");
    }

    #[test]
    fn shift_substitui_pela_faixa_nos_dois_sentidos() {
        assert_eq!(faixa(2, 5), BTreeSet::from([2, 3, 4, 5]));
        assert_eq!(faixa(5, 2), BTreeSet::from([2, 3, 4, 5]));
        assert_eq!(faixa(3, 3), BTreeSet::from([3]));
    }

    #[test]
    fn a_escolha_nasce_com_tudo_menos_o_enquadramento() {
        let escolha = Escolha::default();
        for grupo in Grupo::TODOS {
            assert_eq!(
                escolha.ligado(grupo),
                grupo != Grupo::Enquadramento,
                "{}",
                grupo.rotulo()
            );
        }
        assert!(escolha.tem_algo());
        assert!(!escolha.tudo());

        let mut tudo = escolha;
        tudo.marcar_tudo(true);
        assert!(tudo.tudo());
        tudo.marcar_tudo(false);
        assert!(!tudo.tem_algo());
    }

    /// 🚨 **Todo ajuste viaja em exatamente um grupo**, e nenhum por sobra.
    ///
    /// Este teste contava 53 e travava as contagens dos sete painéis — e quebrou
    /// quando o motor passou a 171, com o Básico contando 124. A fonte agora é
    /// `Ajustes::NOMES`: ajuste novo sem grupo falha aqui, com o nome, antes de
    /// ser levado (ou esquecido) por uma sincronização.
    #[test]
    fn cada_ajuste_viaja_em_um_grupo() {
        for nome in Ajustes::NOMES {
            let grupo = grupo_do_ajuste(nome);
            assert!(grupo.is_some(), "`{nome}` não viaja em grupo nenhum");
            assert_ne!(
                grupo,
                Some(Grupo::Enquadramento),
                "`{nome}`: o enquadramento não é ajuste"
            );
        }
        // Toda caixa de ajuste leva alguma coisa: caixa que não leva nada é
        // mentira na tela.
        for grupo in Grupo::TODOS
            .into_iter()
            .filter(|g| *g != Grupo::Enquadramento)
        {
            assert!(
                Ajustes::NOMES
                    .iter()
                    .any(|n| grupo_do_ajuste(n) == Some(grupo)),
                "`{}` não leva ajuste nenhum",
                grupo.rotulo()
            );
        }
        // O estilo dos Controles RGB viaja inteiro, e em grupo próprio.
        for nome in Ajustes::NOMES.iter().filter(|n| n.starts_with("dt_")) {
            assert_eq!(grupo_do_ajuste(nome), Some(Grupo::ControlesRgb), "`{nome}`");
        }
    }

    /// 🔑 **O grupo de um ajuste é o painel onde o slider dele mora.**
    ///
    /// Sem isto, a caixa diria "Detalhe" e levaria um slider que o operador vê
    /// em "Lente". A conferência é contra a tabela de controles, pelo campo que
    /// cada um lê (`campo_do_controle`), e não contra uma lista escrita aqui.
    #[test]
    fn o_grupo_de_cada_controle_e_o_painel_dele() {
        use crate::revelacao::controles::{campo_do_controle, CONTROLES};
        for def in CONTROLES.iter() {
            let nome = campo_do_controle(def);
            assert_eq!(
                grupo_do_ajuste(nome),
                Some(Grupo::do_painel(def.secao.painel())),
                "`{}` ({nome}) mora em `{}`",
                def.rotulo,
                def.secao.painel().rotulo()
            );
        }
    }

    /// 🚨 **Sincronizar só o Básico não leva o estilo, e desmarcar o Básico não
    /// o deixa para trás.** Era o defeito de o mapa mandar ao Básico todo ajuste
    /// que não reconhecia.
    #[test]
    fn o_estilo_e_a_calibracao_viajam_nos_proprios_grupos() {
        let origem = Ajustes {
            exposure: 1.5,
            dt_vignette_ativo: 1.0,
            dt_vignette_brightness: 0.9,
            calib_red_hue: 20.0,
            bw_ativo: 1.0,
            curva_m4: 150.0,
            ..Ajustes::default()
        };
        let destino = Ajustes::default();

        let mut so_basico = Escolha::default();
        so_basico.marcar_tudo(false);
        so_basico.basico = true;
        let final_ = mesclar(destino, origem, &so_basico);
        assert_eq!(final_.exposure, 1.5, "o Básico viaja");
        assert_eq!(
            final_.dt_vignette_ativo, 0.0,
            "o estilo não anda com o Básico"
        );
        assert_eq!(
            final_.calib_red_hue, 0.0,
            "a calibração não anda com o Básico"
        );
        assert_eq!(final_.bw_ativo, 0.0, "o P&B não anda com o Básico");
        assert_eq!(
            final_.curva_m4, destino.curva_m4,
            "a curva por ponto não anda com o Básico"
        );

        let sem_basico = Escolha {
            basico: false,
            ..Escolha::default()
        };
        let final_ = mesclar(destino, origem, &sem_basico);
        assert_eq!(
            final_.exposure, 0.0,
            "o Básico desmarcado fica o do destino"
        );
        assert_eq!(final_.dt_vignette_ativo, 1.0, "o estilo viaja sem o Básico");
        assert_eq!(final_.dt_vignette_brightness, 0.9);
        assert_eq!(final_.calib_red_hue, 20.0);
        assert_eq!(final_.bw_ativo, 1.0);
        assert_eq!(final_.curva_m4, 150.0);
    }

    /// 🔑 Sincronizar não é substituir: o grupo desmarcado fica como estava no
    /// destino.
    #[test]
    fn mesclar_troca_so_os_grupos_escolhidos() {
        let origem = Ajustes {
            exposure: 1.5,
            sharpen_amount: 80.0,
            tone_curve_darks: -20.0,
            ..Ajustes::default()
        };
        let destino = Ajustes {
            exposure: -0.5,
            sharpen_amount: 30.0,
            hsl_red_sat: 40.0,
            ..Ajustes::default()
        };

        let escolha = Escolha {
            detalhe: false,
            ..Escolha::default()
        };
        let final_ = mesclar(destino, origem, &escolha);
        assert_eq!(final_.exposure, 1.5, "Básico viaja");
        assert_eq!(final_.tone_curve_darks, -20.0, "a curva viaja");
        assert_eq!(
            final_.sharpen_amount, 30.0,
            "Detalhe desmarcado fica o do destino"
        );
        assert_eq!(
            final_.hsl_red_sat, 0.0,
            "HSL marcado leva o neutro da origem"
        );
    }

    #[test]
    fn a_escolha_sobrevive_ao_disco_e_o_lixo_vira_padrao() {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let caminho = dir.path().join("sincronizacao.json");
        assert_eq!(ler_de(&caminho), None, "sem arquivo, sem escolha");

        let escolha = Escolha {
            hsl: false,
            enquadramento: true,
            ..Escolha::default()
        };
        gravar_em(&caminho, &escolha);
        assert_eq!(ler_de(&caminho), Some(escolha));

        std::fs::write(&caminho, "{ isto não é json").unwrap();
        assert_eq!(ler_de(&caminho), None);

        // Campo que ainda não existia quando o arquivo foi escrito: o padrão.
        std::fs::write(&caminho, r#"{"basico": false}"#).unwrap();
        let lida = ler_de(&caminho).unwrap();
        assert!(!lida.basico);
        assert!(lida.curva && !lida.enquadramento);
        // E os grupos que vieram depois nascem marcados, como no site.
        assert!(
            lida.controles_rgb && lida.curva_por_ponto && lida.preto_e_branco && lida.calibracao
        );
    }
}
