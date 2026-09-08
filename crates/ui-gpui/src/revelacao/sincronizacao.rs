//! Sincronizar: os ajustes da foto aberta vão para as outras marcadas na tira.
//!
//! É o "Synchronize Settings" do Lightroom, e o desenho é o do site
//! (`revelacao/sincronizacao.ts`, 2026-09-05, pedido do dono: *"coloque flags,
//! escolhe tudo e desmarcar algumas coisas para sincronizar"*). Este módulo é
//! a parte **pura**: o que está marcado na tira, o que viaja e o que fica, e a
//! escolha guardada entre aberturas. Quem grava e sobe é a raiz.
//!
//! 🔑 **Os grupos são os painéis**, não uma divisão nova. O operador acabou de
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

/// Um grupo da caixa de sincronizar: os sete painéis e o enquadramento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Painel(Painel),
    Enquadramento,
}

impl Grupo {
    pub const TODOS: [Grupo; 8] = [
        Grupo::Painel(Painel::Basico),
        Grupo::Painel(Painel::CurvaDeTons),
        Grupo::Painel(Painel::Hsl),
        Grupo::Painel(Painel::Detalhe),
        Grupo::Painel(Painel::Lente),
        Grupo::Painel(Painel::Tonalizacao),
        Grupo::Painel(Painel::Efeitos),
        Grupo::Enquadramento,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Grupo::Painel(painel) => painel.rotulo(),
            Grupo::Enquadramento => "Enquadramento",
        }
    }

    /// O que ele descreve, em uma linha — some quando for óbvio.
    pub fn detalhe(self) -> Option<&'static str> {
        match self {
            Grupo::Painel(Painel::Hsl) => Some("cor, luminância e matiz das oito faixas"),
            Grupo::Painel(Painel::Detalhe) => Some("ruído e nitidez"),
            Grupo::Painel(Painel::Lente) => Some("distorção e vinheta"),
            Grupo::Painel(Painel::Tonalizacao) => Some("a cor das sombras e a das altas luzes"),
            Grupo::Painel(Painel::Efeitos) => Some("grão"),
            Grupo::Enquadramento => Some("giro, espelho, endireitar e recorte"),
            _ => None,
        }
    }

    /// Um id estável para o elemento da caixa.
    pub fn chave(self) -> &'static str {
        match self {
            Grupo::Painel(Painel::Basico) => "basico",
            Grupo::Painel(Painel::CurvaDeTons) => "curva",
            Grupo::Painel(Painel::Hsl) => "hsl",
            Grupo::Painel(Painel::Detalhe) => "detalhe",
            Grupo::Painel(Painel::Lente) => "lente",
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
    pub basico: bool,
    pub curva: bool,
    pub hsl: bool,
    pub detalhe: bool,
    pub lente: bool,
    pub tonalizacao: bool,
    pub efeitos: bool,
    pub enquadramento: bool,
}

impl Default for Escolha {
    fn default() -> Self {
        Self {
            basico: true,
            curva: true,
            hsl: true,
            detalhe: true,
            lente: true,
            tonalizacao: true,
            efeitos: true,
            enquadramento: false,
        }
    }
}

impl Escolha {
    fn campo(&mut self, grupo: Grupo) -> &mut bool {
        match grupo {
            Grupo::Painel(Painel::Basico) => &mut self.basico,
            Grupo::Painel(Painel::CurvaDeTons) => &mut self.curva,
            Grupo::Painel(Painel::Hsl) => &mut self.hsl,
            Grupo::Painel(Painel::Detalhe) => &mut self.detalhe,
            Grupo::Painel(Painel::Lente) => &mut self.lente,
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

    /// Os oito ligados — o rótulo do atalho da caixa vira "Desmarcar tudo".
    pub fn tudo(&self) -> bool {
        Grupo::TODOS.into_iter().all(|g| self.ligado(g))
    }

    /// Alguma coisa a fazer? Sem nada marcado o botão de confirmar desliga.
    pub fn tem_algo(&self) -> bool {
        Grupo::TODOS.into_iter().any(|g| self.ligado(g))
    }

    pub fn leva(&self, painel: Painel) -> bool {
        self.ligado(Grupo::Painel(painel))
    }
}

/// Em que painel um ajuste mora, pelo nome do `uniform`.
///
/// 🔑 **Pelo nome, e não pela tabela de controles**: `CONTROLES` descreve
/// sliders, e a curva de tons tem widget próprio. Os nomes são o contrato com o
/// shader (`Ajustes::NOMES`) e cobrem os 53 — o teste abaixo conta.
pub fn painel_do_ajuste(nome: &str) -> Painel {
    if nome.starts_with("tone_curve_") {
        Painel::CurvaDeTons
    } else if nome.starts_with("hsl_") {
        Painel::Hsl
    } else if nome.starts_with("nr_") || nome.starts_with("sharpen_") {
        Painel::Detalhe
    } else if nome.starts_with("lens_") {
        Painel::Lente
    } else if nome.starts_with("split_") {
        Painel::Tonalizacao
    } else if nome.starts_with("grain_") {
        Painel::Efeitos
    } else {
        Painel::Basico
    }
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
        if escolha.leva(painel_do_ajuste(nome)) {
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
pub fn ler() -> Escolha {
    ler_de(&caminho()).unwrap_or_default()
}

pub fn ler_de(caminho: &Path) -> Option<Escolha> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Gravar falha só imprime: não poder lembrar não pode impedir de sincronizar.
pub fn gravar(escolha: &Escolha) {
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

    /// 🚨 O mapa por nome cobre os 53, com as contagens dos painéis do site.
    #[test]
    fn cada_um_dos_53_ajustes_mora_em_um_painel() {
        let conta = |painel: Painel| {
            Ajustes::NOMES
                .iter()
                .filter(|nome| painel_do_ajuste(nome) == painel)
                .count()
        };
        assert_eq!(conta(Painel::Basico), 11);
        assert_eq!(conta(Painel::CurvaDeTons), 4);
        assert_eq!(conta(Painel::Hsl), 24);
        assert_eq!(conta(Painel::Detalhe), 4);
        assert_eq!(conta(Painel::Lente), 3);
        assert_eq!(conta(Painel::Tonalizacao), 5);
        assert_eq!(conta(Painel::Efeitos), 2);
        assert_eq!(Ajustes::NOMES.len(), 53);
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
    }
}
