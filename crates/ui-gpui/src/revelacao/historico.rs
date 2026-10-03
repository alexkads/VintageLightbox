//! O histórico da revelação: desfazer, refazer e o painel **Histórico** do
//! Lightroom — a lista de passos com nome, variação e valor ("Contraste −1 55").
//!
//! Guarda o [`Estado`] inteiro em cada passo, e não "o que mudou". Guardar
//! diferença economizaria memória e custaria o problema de verdade: aplicar
//! diferença ao contrário, na ordem certa, sem acumular erro de ponto
//! flutuante. O **nome** do passo, ao contrário, sai da diferença
//! ([`rotular`]): é ela que diz o que o gesto fez.
//!
//! ## 🚨 Três diferenças de propósito em relação ao `crates/ui`
//!
//! **1. Um passo por gesto, e não por quadro.** O legado empurra um snapshot a
//! cada quadro em que algum valor difere do anterior (`app.rs`, dentro do
//! `update`), o que faz um arrasto de meio segundo virar ~30 passos. Pior: **o
//! número de passos depende da taxa de quadros** — a 120fps ele grava o dobro.
//! Comportamento que muda com o monitor não é paridade conferível; é o mesmo
//! recurso em duas máquinas diferentes.
//!
//! Aqui o passo é registrado no **fim do gesto**, o mesmo instante em que a
//! gravação acontece (`tela.rs`). Um arrasto = um `Cmd+Z` = uma linha no painel.
//!
//! **2. A primeira edição é desfazível.** No legado, `push_edit_snapshot` só
//! roda quando algo mudou, então o primeiro snapshot já é o estado **depois** da
//! mudança — e `undo` faz `if index > 0`, ou seja, não faz nada. Aqui o estado
//! da abertura entra no histórico como passo zero ("Início"), e o primeiro
//! `Cmd+Z` devolve a foto ao que estava gravado.
//!
//! **3. O passo é a revelação inteira, e não só os sliders.** Foi o item 12 da
//! fila, e a ideia veio do darktable: lá a pilha de histórico é a lista de
//! *módulos aplicados*, e o corte é um módulo como qualquer outro. Aqui o
//! equivalente é [`Estado`]: os ajustes, o corte e a Revelação local num tipo
//! só. 🚨 Até 6/set/2026 a pilha guardava só `Ajustes`, e `Cmd+Z` depois de
//! cortar voltava tudo menos o enquadramento.
//!
//! ## 📜 O histórico fica com a foto
//!
//! Como no Lightroom, ele não morre ao trocar de foto nem ao fechar o app: vai
//! para o catálogo em JSON ([`Historico::em_json`], tabela
//! `historico_da_revelacao`) e volta na abertura ([`Historico::retomar`]). O
//! que mudou a foto **fora** da Revelação nesse meio-tempo (colar na
//! Biblioteca, sincronizar, a revelação padrão) entra como um passo próprio,
//! [`DE_FORA`] — sem ele, o `Cmd+Z` levaria a foto a um estado que nunca
//! esteve na tela.

use std::sync::Arc;

use domain::value_objects::PerspectivaGuiada;
use infrastructure::gpu_adjustments::ParametrosLocais;
use revelacao_core::locais::{Camada, Retoque};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::controles::{Definicao, Secao, CONTROLES};
use super::persistencia::Corte;
use super::processador::Ajustes;

/// Como a foto está revelada, inteira — é o que um passo do histórico guarda.
///
/// 🔑 **A Revelação local entrou aqui como o cabeçalho previa**: máscaras e
/// retoques são parte do estado, e o `Cmd+Z` os alcança sem ninguém lembrar.
/// Por `Arc`: um passo custa um ponteiro, e a comparação continua por valor.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Estado {
    pub ajustes: Ajustes,
    pub corte: Corte,
    pub locais: Arc<ParametrosLocais>,
}

/// O que a linha do painel mostra: o nome do gesto e, quando ele mexeu num
/// controle só, quanto andou e onde parou — "Contraste −1 55".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rotulo {
    pub nome: String,
    /// Quanto o controle andou, com sinal. `None` quando não é número
    /// (interruptor, lista, máscara).
    pub variacao: Option<String>,
    /// Onde ele parou — o número, "Sim"/"Não" ou o nome da opção.
    pub valor: Option<String>,
}

impl Rotulo {
    /// Só o nome — o gesto discreto ("Predefinição: Kodak") e o que não é número.
    pub fn so(nome: impl Into<String>) -> Self {
        Self {
            nome: nome.into(),
            variacao: None,
            valor: None,
        }
    }
}

/// Uma linha do painel: o estado e o nome do gesto que levou a ele.
#[derive(Debug, Clone, PartialEq)]
pub struct Passo {
    pub estado: Estado,
    pub rotulo: Rotulo,
}

/// O nome do passo zero — o estado em que a foto abriu, ou o que ficou depois
/// de limpar o histórico.
pub const INICIO: &str = "Início";
/// O passo que a retomada acrescenta quando a foto mudou fora da Revelação.
pub const DE_FORA: &str = "Mudou fora da Revelação";
/// Mais de um controle, de seções diferentes, no mesmo gesto.
pub const VARIAS: &str = "Várias configurações";

/// Quantos passos cabem por foto.
///
/// Eram os 20 do legado enquanto o histórico morria ao trocar de foto. Com o
/// painel e o histórico guardado com a foto, 20 é pouco para uma revelação de
/// verdade (dez sliders e duas máscaras já passam disso); 100 cabem folgados
/// no catálogo, porque o JSON guarda só o que difere do neutro e cada
/// Revelação local repetida uma vez só.
pub const TETO: usize = 100;

/// A versão do JSON gravado. Maior que esta é recusada (app mais velho).
const VERSAO: u32 = 1;

pub struct Historico {
    /// Os passos, do mais antigo ao mais novo. Nunca vazio: nasce com o estado
    /// da abertura.
    passos: Vec<Passo>,
    /// Onde estamos. Desfazer anda para trás, refazer para a frente.
    atual: usize,
}

impl Historico {
    /// Começa no estado em que a foto abriu — o que está gravado no banco.
    pub fn novo(inicial: Estado) -> Self {
        Self {
            passos: vec![Passo {
                estado: inicial,
                rotulo: Rotulo::so(INICIO),
            }],
            atual: 0,
        }
    }

    /// Registra um estado novo, com o nome tirado da diferença para o atual.
    /// Devolve se virou passo.
    ///
    /// ⚠️ **Registrar o que não mudou encheria o histórico de passos idênticos**,
    /// e o `Cmd+Z` pareceria não fazer nada por várias teclas seguidas. Acontece
    /// de verdade: soltar o slider exatamente onde ele estava é um gesto completo,
    /// com fim de gesto e tudo.
    pub fn registrar(&mut self, estado: Estado) -> bool {
        let rotulo = rotular(&self.passos[self.atual].estado, &estado);
        self.empilhar(estado, rotulo)
    }

    /// Registra um gesto discreto com o nome dele — "Predefinição: Kodak",
    /// "Tom automático". A diferença diria só "Várias configurações".
    pub fn registrar_como(&mut self, estado: Estado, nome: impl Into<String>) -> bool {
        self.empilhar(estado, Rotulo::so(nome))
    }

    fn empilhar(&mut self, estado: Estado, rotulo: Rotulo) -> bool {
        if self.passos[self.atual].estado == estado {
            return false;
        }

        // O futuro morre aqui: desfazer três vezes e mexer num slider apaga o que
        // havia para refazer. É o que todo editor faz, e o que o Lightroom faz
        // quando se clica num passo antigo e se mexe na foto.
        self.passos.truncate(self.atual + 1);
        self.passos.push(Passo { estado, rotulo });

        if self.passos.len() > TETO {
            // O mais antigo sai. `remove(0)` num Vec de 100 é cópia de 99
            // ponteiros e números — irrelevante num gesto humano, e mais simples
            // de ler do que um anel.
            self.passos.remove(0);
        }

        self.atual = self.passos.len() - 1;
        true
    }

    pub fn pode_desfazer(&self) -> bool {
        self.atual > 0
    }

    pub fn pode_refazer(&self) -> bool {
        self.atual + 1 < self.passos.len()
    }

    /// Volta um passo. `None` quando já está no começo.
    pub fn desfazer(&mut self) -> Option<Estado> {
        if !self.pode_desfazer() {
            return None;
        }
        self.atual -= 1;
        Some(self.passos[self.atual].estado.clone())
    }

    /// Avança um passo. `None` quando já está no fim.
    pub fn refazer(&mut self) -> Option<Estado> {
        if !self.pode_refazer() {
            return None;
        }
        self.atual += 1;
        Some(self.passos[self.atual].estado.clone())
    }

    /// Vai direto a um passo — o clique numa linha do painel. `None` quando o
    /// passo não existe ou já é o atual.
    ///
    /// 🔑 **Não apaga nada**: os passos depois dele continuam lá, como no
    /// Lightroom, e o `Cmd+Shift+Z` volta a eles. Só o próximo gesto os apaga.
    pub fn ir_para(&mut self, indice: usize) -> Option<Estado> {
        if indice >= self.passos.len() || indice == self.atual {
            return None;
        }
        self.atual = indice;
        Some(self.passos[indice].estado.clone())
    }

    /// O ✕ do painel: fica só o estado de agora, como passo zero.
    pub fn limpar(&mut self) {
        let estado = self.passos[self.atual].estado.clone();
        *self = Self::novo(estado);
    }

    /// Os passos, do mais antigo ao mais novo.
    pub fn passos(&self) -> &[Passo] {
        &self.passos
    }

    /// O índice do passo que está na tela.
    pub fn atual(&self) -> usize {
        self.atual
    }

    /// Refaz cada passo sobre uma revelação que mudou por fora.
    ///
    /// 🔑 Os passos foram dados sobre a revelação de antes: sem isto, o `Cmd+Z`
    /// levaria a foto de volta a um estado sem a revelação que chegou — e
    /// gravaria isso (ver `Revelacao::parametros_mudaram_por_fora`). Os nomes
    /// ficam: o gesto continua sendo o que foi.
    pub fn rebasear(&mut self, refazer: impl Fn(Estado) -> Estado) {
        for passo in &mut self.passos {
            passo.estado = refazer(passo.estado.clone());
        }
    }

    // ------------------------------------------------------------ gravação

    /// O histórico como o catálogo o guarda.
    ///
    /// Os ajustes vão só com o que difere do neutro (`serde(default)` os
    /// completa na volta), e cada Revelação local diferente vai uma vez só,
    /// numa lista que os passos apontam por índice — uma máscara pintada com
    /// 4.000 pontos, repetida em 60 passos de slider, seria 60 cópias.
    pub fn em_json(&self) -> String {
        let neutro = serde_json::to_value(Ajustes::default()).unwrap_or_default();
        let mut locais: Vec<Arc<ParametrosLocais>> = Vec::new();
        let passos = self
            .passos
            .iter()
            .map(|passo| {
                let indice = match locais.iter().position(|l| {
                    Arc::ptr_eq(l, &passo.estado.locais) || **l == *passo.estado.locais
                }) {
                    Some(i) => i,
                    None => {
                        locais.push(passo.estado.locais.clone());
                        locais.len() - 1
                    }
                };
                PassoGravado {
                    nome: passo.rotulo.nome.clone(),
                    variacao: passo.rotulo.variacao.clone(),
                    valor: passo.rotulo.valor.clone(),
                    ajustes: fora_do_neutro(&passo.estado.ajustes, &neutro),
                    corte: CorteGravado::de(&passo.estado.corte),
                    locais: indice,
                }
            })
            .collect();
        let gravado = Gravado {
            versao: VERSAO,
            atual: self.atual,
            locais: locais
                .iter()
                .map(|l| serde_json::to_value(&**l).unwrap_or_default())
                .collect(),
            passos,
        };
        serde_json::to_string(&gravado).unwrap_or_default()
    }

    /// O histórico gravado, de volta — e conferido contra a foto que abriu.
    ///
    /// 🚨 **O passo atual gravado tem de ser a foto de agora.** Se não for, a
    /// foto mudou fora da Revelação (colada na Biblioteca, sincronizada, a
    /// revelação padrão do ensaio), e o histórico ganha um passo [`DE_FORA`]
    /// com o estado de agora. Adotar o gravado como estava faria a tela mostrar
    /// uma coisa e o painel marcar outra.
    pub fn retomar(json: &str, aberto: Estado) -> Result<Self, String> {
        let gravado: Gravado =
            serde_json::from_str(json).map_err(|e| format!("o histórico não é legível: {e}"))?;
        if gravado.versao > VERSAO {
            return Err(format!(
                "o histórico é da versão {} (este app lê até a {VERSAO})",
                gravado.versao
            ));
        }
        let locais = gravado
            .locais
            .iter()
            .map(|v| {
                ParametrosLocais::de_json(&v.to_string())
                    .map(Arc::new)
                    .map_err(|e| format!("a Revelação local de um passo não é legível: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let passos = gravado
            .passos
            .into_iter()
            .map(|p| {
                let ajustes = serde_json::from_value::<Ajustes>(Value::Object(p.ajustes))
                    .map_err(|e| format!("os ajustes de um passo não são legíveis: {e}"))?;
                let locais = locais
                    .get(p.locais)
                    .cloned()
                    .ok_or("um passo aponta uma Revelação local que não existe")?;
                Ok(Passo {
                    estado: Estado {
                        ajustes,
                        corte: p.corte.para_corte(),
                        locais,
                    },
                    rotulo: Rotulo {
                        nome: p.nome,
                        variacao: p.variacao,
                        valor: p.valor,
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if passos.is_empty() || gravado.atual >= passos.len() {
            return Err("o histórico gravado está vazio ou aponta além do fim".into());
        }
        let mut historico = Self {
            passos,
            atual: gravado.atual,
        };
        // ⚠️ **A mesma foto por extenso, e não campo a campo.** A foto do site
        // grava o corte pelo `CropSettings` e reabre com a foto inteira escrita
        // (`Some(0.0)`, `Some(1.0)`) onde o passo tinha `None`: comparar o
        // `Corte` cru poria um "Mudou fora" a cada reabertura dela.
        let presente = &historico.passos[historico.atual].estado;
        let a_mesma = super::persistencia::mesmos_parametros(
            (presente.ajustes, presente.corte),
            (aberto.ajustes, aberto.corte),
        ) && presente.locais == aberto.locais;
        if a_mesma {
            // O passo fica com a foto como ela abriu: é contra ela que o
            // próximo gesto se compara.
            historico.passos[historico.atual].estado = aberto;
        } else {
            historico.registrar_como(aberto, DE_FORA);
        }
        Ok(historico)
    }

    /// O gravado, com os passos desta abertura por cima — a leitura do disco
    /// chegou depois de o operador já ter mexido na foto.
    ///
    /// 🔑 O passo zero desta abertura é a foto como abriu: é contra ele que o
    /// gravado se confere ([`Self::retomar`]), e os gestos de agora vêm depois.
    pub fn retomar_sob(json: &str, recente: &Historico) -> Result<Self, String> {
        let mut historico = Self::retomar(json, recente.passos[0].estado.clone())?;
        for passo in &recente.passos[1..] {
            historico.empilhar(passo.estado.clone(), passo.rotulo.clone());
        }
        // 🚨 **Sem gesto desta abertura, o atual é o gravado.** A conta abaixo
        // levava o atual ao fim mesmo assim: a foto reaberta com passos
        // desfeitos aparecia marcada no último, e o gesto seguinte se
        // comparava com ele — "Endireitar" virou "Enquadrar" (visto no app,
        // 03/10/2026).
        if recente.passos.len() > 1 {
            // O que estava desfeito nesta abertura continua desfeito.
            let desfeitos = recente.passos.len() - 1 - recente.atual;
            historico.atual =
                historico.passos.len() - 1 - desfeitos.min(historico.passos.len() - 1);
        }
        Ok(historico)
    }
}

/// O JSON da tabela `historico_da_revelacao`.
#[derive(Serialize, Deserialize)]
struct Gravado {
    versao: u32,
    atual: usize,
    #[serde(default)]
    locais: Vec<Value>,
    passos: Vec<PassoGravado>,
}

#[derive(Serialize, Deserialize)]
struct PassoGravado {
    nome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variacao: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    valor: Option<String>,
    #[serde(default)]
    ajustes: Map<String, Value>,
    #[serde(default)]
    corte: CorteGravado,
    #[serde(default)]
    locais: usize,
}

/// Os campos dos ajustes que não estão no neutro.
fn fora_do_neutro(ajustes: &Ajustes, neutro: &Value) -> Map<String, Value> {
    let Ok(Value::Object(mut campos)) = serde_json::to_value(ajustes) else {
        return Map::new();
    };
    campos.retain(|chave, valor| neutro.get(chave) != Some(valor));
    campos
}

/// O [`Corte`] do jeito que o histórico o guarda — **campo a campo, com o
/// `None`**.
///
/// 🚨 **Não é o `ajustes_em_json` do site.** Aquele passa pelo `CropSettings`,
/// que troca o "sem corte" (`None`) pela foto inteira (`Some(0.0)`, `Some(1.0)`):
/// o passo voltaria diferente do que a foto abre, e toda foto reaberta ganharia
/// um "Mudou fora da Revelação" sem ninguém ter mudado nada.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct CorteGravado {
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    largura: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    altura: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotacao: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    angulo: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    espelho_h: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    espelho_v: Option<bool>,
    /// As chaves `corte_persp_*`/`corte_guiaN_*` do domínio.
    #[serde(skip_serializing_if = "Option::is_none")]
    perspectiva: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restringir: Option<bool>,
}

impl CorteGravado {
    fn de(corte: &Corte) -> Self {
        Self {
            x: corte.x,
            y: corte.y,
            largura: corte.largura,
            altura: corte.altura,
            rotacao: corte.rotacao,
            angulo: corte.angulo,
            espelho_h: corte.espelho_h,
            espelho_v: corte.espelho_v,
            perspectiva: corte.perspectiva.map(|p| {
                let mut objeto = Map::new();
                p.em_json(&mut objeto);
                objeto
            }),
            restringir: corte.restringir,
        }
    }

    fn para_corte(self) -> Corte {
        Corte {
            x: self.x,
            y: self.y,
            largura: self.largura,
            altura: self.altura,
            rotacao: self.rotacao,
            angulo: self.angulo,
            espelho_h: self.espelho_h,
            espelho_v: self.espelho_v,
            perspectiva: self
                .perspectiva
                .map(|objeto| PerspectivaGuiada::de_json(&Value::Object(objeto))),
            restringir: self.restringir,
        }
    }
}

// ------------------------------------------------------------ nomes

/// O nome do gesto que leva de `antes` a `depois` — a linha do painel.
pub fn rotular(antes: &Estado, depois: &Estado) -> Rotulo {
    let mut partes = Vec::new();
    if antes.ajustes != depois.ajustes {
        partes.push(rotulo_dos_ajustes(&antes.ajustes, &depois.ajustes));
    }
    if antes.corte != depois.corte {
        partes.push(rotulo_do_corte(&antes.corte, &depois.corte));
    }
    if antes.locais != depois.locais {
        partes.push(rotulo_dos_locais(&antes.locais, &depois.locais));
    }
    match partes.len() {
        1 => partes.remove(0),
        _ => Rotulo::so(VARIAS),
    }
}

fn rotulo_dos_ajustes(antes: &Ajustes, depois: &Ajustes) -> Rotulo {
    // Um controle pode aparecer em dois lugares (o P&B do Básico e o da
    // Mistura): o nome conta uma vez só.
    let mut mudados: Vec<&Definicao> = Vec::new();
    for definicao in CONTROLES {
        if (definicao.ler)(antes) != (definicao.ler)(depois)
            && !mudados.iter().any(|d| d.rotulo == definicao.rotulo)
        {
            mudados.push(definicao);
        }
    }
    match mudados.as_slice() {
        [] => Rotulo::so(VARIAS),
        // A curva por ponto: arrastar um ponto mexe nele e nos vizinhos, e
        // "RGB — ponto 3" não diz nada a quem olha a lista.
        [d, ..] if mudados.iter().all(|m| m.secao == Secao::CurvaPorPonto) => {
            Rotulo::so(d.secao.rotulo())
        }
        [d] => rotulo_do_controle(d, antes, depois),
        [d, ..] if mudados.iter().all(|m| m.secao == d.secao) => Rotulo::so(d.secao.rotulo()),
        _ => Rotulo::so(VARIAS),
    }
}

/// "Contraste −1 55", "Remover desvio cromático Sim", "Estilo Realces".
fn rotulo_do_controle(d: &Definicao, antes: &Ajustes, depois: &Ajustes) -> Rotulo {
    let (de, para) = ((d.ler)(antes), (d.ler)(depois));
    let nome = d.rotulo.to_string();
    if d.opcoes.is_some() {
        return Rotulo {
            nome,
            variacao: None,
            valor: Some(d.formatar(para)),
        };
    }
    if d.discreto && d.minimo == 0.0 && d.maximo == 1.0 {
        return Rotulo {
            nome,
            variacao: None,
            valor: Some(if para >= 0.5 { "Sim" } else { "Não" }.to_string()),
        };
    }
    Rotulo {
        nome,
        variacao: Some(com_sinal(para - de, d.casas)),
        valor: Some(d.formatar(para)),
    }
}

/// A variação sempre com sinal, nas casas do controle: "+0,30", "-1".
fn com_sinal(valor: f32, casas: usize) -> String {
    // Arredonda antes de decidir o sinal: −0,001 com zero casas é "0", e não
    // "-0".
    let fator = 10f32.powi(casas as i32);
    let arredondado = (valor * fator).round() / fator;
    let texto = format!("{:.*}", casas, arredondado.abs()).replace('.', ",");
    if arredondado > 0.0 {
        format!("+{texto}")
    } else if arredondado < 0.0 {
        format!("-{texto}")
    } else {
        texto
    }
}

fn rotulo_do_corte(antes: &Corte, depois: &Corte) -> Rotulo {
    // O retângulo por extenso: `None` é a foto inteira. Girar escreve os
    // quatro (`corte_de`), e "nada" → "a foto inteira" não é cortar.
    let retangulo = |c: &Corte| {
        (
            c.x.unwrap_or(0.0),
            c.y.unwrap_or(0.0),
            c.largura.unwrap_or(1.0),
            c.altura.unwrap_or(1.0),
        )
    };
    let enquadrou = retangulo(antes) != retangulo(depois);
    let endireitou = antes.angulo.unwrap_or(0.0) != depois.angulo.unwrap_or(0.0);
    let girou = antes.rotacao.unwrap_or(0) != depois.rotacao.unwrap_or(0);
    let espelhou = antes.espelho_h.unwrap_or(false) != depois.espelho_h.unwrap_or(false)
        || antes.espelho_v.unwrap_or(false) != depois.espelho_v.unwrap_or(false);
    let perspectiva =
        antes.perspectiva.unwrap_or_default() != depois.perspectiva.unwrap_or_default();
    let restringiu = antes.restringir.unwrap_or(true) != depois.restringir.unwrap_or(true);

    // 🔑 **O gesto manda no nome; o retângulo é consequência.** Girar troca
    // largura e altura, endireitar encolhe o retângulo para caber: contá-los
    // como um segundo gesto dava "Enquadrar" a um simples `]` (visto no app,
    // 03/10/2026).
    let gestos = [
        (girou, "Girar"),
        (espelhou, "Espelhar"),
        (perspectiva, "Perspectiva"),
        (restringiu, "Restringir ao conteúdo"),
        (endireitou, "Endireitar"),
    ];
    let feitos: Vec<&str> = gestos.iter().filter(|(f, _)| *f).map(|(_, n)| *n).collect();
    match feitos.as_slice() {
        ["Endireitar"] => {
            let graus = depois.angulo.unwrap_or(0.0);
            Rotulo {
                nome: "Endireitar".into(),
                variacao: Some(com_sinal(graus - antes.angulo.unwrap_or(0.0), 1)),
                valor: Some(format!("{}°", format!("{graus:.1}").replace('.', ","))),
            }
        }
        ["Restringir ao conteúdo"] => Rotulo {
            nome: "Restringir ao conteúdo".into(),
            variacao: None,
            valor: Some(
                if depois.restringir.unwrap_or(true) {
                    "Sim"
                } else {
                    "Não"
                }
                .into(),
            ),
        },
        [um] => Rotulo::so(*um),
        [] if enquadrou => Rotulo::so("Cortar"),
        _ => Rotulo::so("Enquadrar"),
    }
}

fn rotulo_dos_locais(antes: &ParametrosLocais, depois: &ParametrosLocais) -> Rotulo {
    let camadas = antes.camadas != depois.camadas;
    let retoques = antes.retoques != depois.retoques;
    if camadas && retoques {
        return Rotulo::so("Revelação local");
    }
    if camadas {
        let (de, para) = (antes.camadas.len(), depois.camadas.len());
        if para > de {
            return Rotulo::so("Adicionar máscara");
        }
        if para < de {
            return Rotulo::so("Excluir máscara");
        }
        // O olho da lista: uma máscara só, e só a visibilidade mudou.
        let mudadas: Vec<_> = antes
            .camadas
            .iter()
            .zip(&depois.camadas)
            .filter(|(a, b)| a != b)
            .collect();
        if let [(a, b)] = mudadas.as_slice() {
            let so_o_olho = Camada {
                visivel: a.visivel,
                ..(*b).clone()
            } == **a;
            if so_o_olho {
                return Rotulo::so(if b.visivel {
                    "Mostrar máscara"
                } else {
                    "Ocultar máscara"
                });
            }
        }
        return Rotulo::so("Atualizar máscara");
    }
    let (de, para) = (antes.retoques.len(), depois.retoques.len());
    if para > de {
        let nome = match depois.retoques.last() {
            Some(Retoque::Clone(_)) => "Adicionar carimbo",
            Some(Retoque::Heal(_)) => "Adicionar band-aid",
            Some(Retoque::Preencher(_)) => "Adicionar Content-Aware",
            None => "Adicionar retoque",
        };
        return Rotulo::so(nome);
    }
    Rotulo::so(if para < de {
        "Excluir retoque"
    } else {
        "Atualizar retoque"
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use revelacao_core::locais::{AjustesLocais, Carimbo};

    fn com_exposicao(valor: f32) -> Estado {
        Estado {
            ajustes: Ajustes {
                exposure: valor,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn cortada(largura: f32) -> Estado {
        Estado {
            corte: Corte {
                largura: Some(largura),
                ..Corte::default()
            },
            ..Estado::default()
        }
    }

    fn com_mascaras(n: usize) -> Estado {
        Estado {
            locais: Arc::new(ParametrosLocais {
                camadas: (0..n)
                    .map(|i| Camada {
                        nome: format!("Máscara {}", i + 1),
                        ajustes: AjustesLocais { exposicao_ev: 0.5 },
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Estado::default()
        }
    }

    fn atual(historico: &Historico) -> &Estado {
        &historico.passos[historico.atual].estado
    }

    fn nomes(historico: &Historico) -> Vec<&str> {
        historico
            .passos
            .iter()
            .map(|p| p.rotulo.nome.as_str())
            .collect()
    }

    /// 🚨 A primeira edição volta — é a diferença 2 do topo do arquivo.
    #[test]
    fn a_primeira_edicao_e_desfazivel() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        assert!(
            !historico.pode_desfazer(),
            "sem edição não há o que desfazer"
        );

        historico.registrar(com_exposicao(1.0));

        assert_eq!(historico.desfazer(), Some(com_exposicao(0.0)));
        assert!(!historico.pode_desfazer());
    }

    #[test]
    fn desfazer_e_refazer_andam_nos_dois_sentidos() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));

        assert_eq!(historico.desfazer(), Some(com_exposicao(1.0)));
        assert_eq!(historico.desfazer(), Some(com_exposicao(0.0)));
        assert_eq!(historico.desfazer(), None, "chegou ao começo");

        assert_eq!(historico.refazer(), Some(com_exposicao(1.0)));
        assert_eq!(historico.refazer(), Some(com_exposicao(2.0)));
        assert_eq!(historico.refazer(), None, "chegou ao fim");
    }

    /// 🚨 Editar depois de desfazer apaga o que havia para refazer.
    #[test]
    fn editar_depois_de_desfazer_mata_o_futuro() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));

        historico.desfazer();
        assert!(historico.pode_refazer());

        historico.registrar(com_exposicao(5.0));

        assert!(!historico.pode_refazer(), "o 2.0 não é mais alcançável");
        assert_eq!(historico.desfazer(), Some(com_exposicao(1.0)));
    }

    /// ⚠️ Gesto que não muda nada não vira passo.
    #[test]
    fn gesto_que_nao_muda_nada_nao_vira_passo() {
        let mut historico = Historico::novo(com_exposicao(0.0));

        assert!(!historico.registrar(com_exposicao(0.0)));
        assert!(!historico.pode_desfazer());

        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(1.0));
        historico.desfazer();
        assert_eq!(
            historico.desfazer(),
            None,
            "os dois 1.0 viraram um passo só"
        );
    }

    /// O teto descarta o mais antigo, e o presente continua sendo o presente.
    ///
    /// 🔑 O erro fácil aqui é esquecer de corrigir o índice ao remover da frente:
    /// `atual` continuaria apontando uma posição além do fim.
    #[test]
    fn o_teto_descarta_o_mais_antigo() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        let gestos = TETO + 10;
        for i in 1..=gestos {
            historico.registrar(com_exposicao(i as f32 / 100.0));
        }

        assert_eq!(historico.passos.len(), TETO);
        assert_eq!(
            atual(&historico),
            &com_exposicao(gestos as f32 / 100.0),
            "o presente é o último gesto"
        );

        let mut voltas = 0;
        while historico.desfazer().is_some() {
            voltas += 1;
        }
        assert_eq!(voltas, TETO - 1);
        assert_eq!(
            atual(&historico),
            &com_exposicao((gestos - TETO + 1) as f32 / 100.0)
        );
    }

    #[test]
    fn o_passo_guarda_os_ajustes_inteiros() {
        let cheio = Estado {
            ajustes: Ajustes {
                exposure: 1.0,
                hsl_blue_lum: -40.0,
                sharpen_amount: 60.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut historico = Historico::novo(Estado::default());
        historico.registrar(cheio.clone());
        historico.registrar(Estado::default());

        assert_eq!(historico.desfazer(), Some(cheio));
    }

    /// 🚨 O corte é passo de histórico como qualquer outro — o item 12.
    #[test]
    fn cortar_e_um_passo_que_o_desfazer_alcanca() {
        let mut historico = Historico::novo(Estado::default());

        historico.registrar(cortada(0.5));
        assert!(historico.pode_desfazer());

        assert_eq!(historico.desfazer(), Some(Estado::default()));
        assert_eq!(historico.refazer(), Some(cortada(0.5)));
    }

    #[test]
    fn cada_corte_e_o_seu_proprio_passo() {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar(cortada(0.8));
        historico.registrar(cortada(0.5));

        assert_eq!(historico.desfazer(), Some(cortada(0.8)));
        assert_eq!(historico.desfazer(), Some(Estado::default()));
    }

    #[test]
    fn corte_igual_ao_atual_nao_vira_passo() {
        let mut historico = Historico::novo(cortada(0.5));
        historico.registrar(cortada(0.5));
        assert!(!historico.pode_desfazer());
    }

    // ------------------------------------------------------------ nomes

    /// 📜 A linha do Lightroom: nome, quanto andou, onde parou.
    #[test]
    fn um_slider_vira_nome_variacao_e_valor() {
        let mut historico = Historico::novo(com_exposicao(0.2));
        historico.registrar(com_exposicao(0.5));

        assert_eq!(
            historico.passos[1].rotulo,
            Rotulo {
                nome: "Exposição".into(),
                variacao: Some("+0,30".into()),
                valor: Some("+0,50".into()),
            }
        );
        assert_eq!(historico.passos[0].rotulo, Rotulo::so(INICIO));
    }

    /// O Contraste mostra a escala do Lightroom (−100..100), e não o
    /// multiplicador do motor: "−1 55", como no print do dono.
    #[test]
    fn o_contraste_fala_na_escala_do_lightroom() {
        let de = Estado {
            ajustes: Ajustes {
                contrast: 1.56,
                ..Default::default()
            },
            ..Default::default()
        };
        let para = Estado {
            ajustes: Ajustes {
                contrast: 1.55,
                ..Default::default()
            },
            ..Default::default()
        };
        let rotulo = rotular(&de, &para);
        assert_eq!(rotulo.nome, "Contraste");
        assert_eq!(rotulo.variacao.as_deref(), Some("-1"));
        assert_eq!(rotulo.valor.as_deref(), Some("+55"));
    }

    #[test]
    fn interruptor_diz_sim_ou_nao() {
        let pb = Estado {
            ajustes: Ajustes {
                bw_ativo: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let rotulo = rotular(&Estado::default(), &pb);
        assert_eq!(rotulo.nome, "P&B");
        assert_eq!(rotulo.variacao, None);
        assert_eq!(rotulo.valor.as_deref(), Some("Sim"));
        assert_eq!(
            rotular(&pb, &Estado::default()).valor.as_deref(),
            Some("Não")
        );
    }

    #[test]
    fn lista_diz_o_nome_da_opcao() {
        let estilo = Estado {
            ajustes: Ajustes {
                pcv_style: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let rotulo = rotular(&Estado::default(), &estilo);
        assert_eq!(rotulo.nome, "Estilo");
        assert_eq!(rotulo.valor.as_deref(), Some("Prioridade de cores"));
    }

    /// Dois controles da mesma seção (a roda de cor mexe em matiz e saturação
    /// juntos) levam o nome da seção; de seções diferentes, "Várias".
    #[test]
    fn varios_controles_viram_secao_ou_varias() {
        let roda = Estado {
            ajustes: Ajustes {
                split_shadow_hue: 200.0,
                split_shadow_sat: 30.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(rotular(&Estado::default(), &roda).nome, "Tonalização");

        let misturado = Estado {
            ajustes: Ajustes {
                exposure: 1.0,
                sharpen_amount: 60.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(rotular(&Estado::default(), &misturado).nome, VARIAS);
    }

    #[test]
    fn o_corte_tem_os_nomes_da_ferramenta() {
        assert_eq!(rotular(&Estado::default(), &cortada(0.5)).nome, "Cortar");

        let endireitada = Estado {
            corte: Corte {
                angulo: Some(2.5),
                largura: Some(0.9),
                ..Corte::default()
            },
            ..Estado::default()
        };
        let rotulo = rotular(&Estado::default(), &endireitada);
        assert_eq!(rotulo.nome, "Endireitar");
        assert_eq!(rotulo.valor.as_deref(), Some("2,5°"));

        // Girar troca largura e altura e escreve o retângulo inteiro: o nome
        // continua sendo o giro (visto no app, 03/10/2026).
        let girada = Estado {
            corte: Corte {
                x: Some(0.1),
                y: Some(0.0),
                largura: Some(0.8),
                altura: Some(1.0),
                rotacao: Some(1),
                ..Corte::default()
            },
            ..Estado::default()
        };
        assert_eq!(rotular(&Estado::default(), &girada).nome, "Girar");

        // "Nada" → "a foto inteira escrita" não é corte nenhum.
        let inteira = Estado {
            corte: Corte {
                x: Some(0.0),
                y: Some(0.0),
                largura: Some(1.0),
                altura: Some(1.0),
                rotacao: Some(0),
                ..Corte::default()
            },
            ..Estado::default()
        };
        assert_eq!(rotular(&Estado::default(), &inteira).nome, "Enquadrar");
    }

    #[test]
    fn mascaras_adicionar_atualizar_ocultar_excluir() {
        let uma = com_mascaras(1);
        let duas = com_mascaras(2);
        assert_eq!(rotular(&uma, &duas).nome, "Adicionar máscara");
        assert_eq!(rotular(&duas, &uma).nome, "Excluir máscara");

        let mut mais_clara = (*uma.locais).clone();
        mais_clara.camadas[0].ajustes.exposicao_ev = 1.0;
        let mais_clara = Estado {
            locais: Arc::new(mais_clara),
            ..Estado::default()
        };
        assert_eq!(rotular(&uma, &mais_clara).nome, "Atualizar máscara");

        let mut oculta = (*uma.locais).clone();
        oculta.camadas[0].visivel = false;
        let oculta = Estado {
            locais: Arc::new(oculta),
            ..Estado::default()
        };
        assert_eq!(rotular(&uma, &oculta).nome, "Ocultar máscara");
        assert_eq!(rotular(&oculta, &uma).nome, "Mostrar máscara");
    }

    #[test]
    fn retoque_adicionado_diz_a_ferramenta() {
        let carimbo = Estado {
            locais: Arc::new(ParametrosLocais {
                retoques: vec![Retoque::Heal(Carimbo {
                    origem: [0.2, 0.2],
                    destino_inicial: [0.5, 0.5],
                    caminho: vec![[0.5, 0.5]],
                    raio: 0.02,
                    feather: 0.5,
                    opacidade: 1.0,
                })],
                ..Default::default()
            }),
            ..Estado::default()
        };
        assert_eq!(
            rotular(&Estado::default(), &carimbo).nome,
            "Adicionar band-aid"
        );
        assert_eq!(
            rotular(&carimbo, &Estado::default()).nome,
            "Excluir retoque"
        );
    }

    #[test]
    fn gesto_discreto_leva_o_proprio_nome() {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar_como(com_exposicao(0.4), "Predefinição: Kodak");
        assert_eq!(nomes(&historico), [INICIO, "Predefinição: Kodak"]);
        assert_eq!(historico.passos[1].rotulo.valor, None);
    }

    // ------------------------------------------------------------ painel

    /// 📜 O clique na linha: vai ao passo e **não apaga** os de depois — só o
    /// próximo gesto apaga, como no Lightroom.
    #[test]
    fn ir_para_um_passo_guarda_o_futuro_ate_o_proximo_gesto() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));
        historico.registrar(com_exposicao(3.0));

        assert_eq!(historico.ir_para(1), Some(com_exposicao(1.0)));
        assert_eq!(historico.atual(), 1);
        assert_eq!(historico.passos().len(), 4, "os de depois continuam");
        assert_eq!(historico.ir_para(1), None, "já está nele");
        assert_eq!(historico.ir_para(9), None, "não existe");

        assert_eq!(historico.refazer(), Some(com_exposicao(2.0)));
        historico.ir_para(1);
        historico.registrar(com_exposicao(0.5));
        assert_eq!(historico.passos().len(), 3, "o gesto apagou o 2 e o 3");
    }

    /// O ✕: fica só a foto de agora, como passo zero.
    #[test]
    fn limpar_deixa_so_o_estado_de_agora() {
        let mut historico = Historico::novo(com_exposicao(0.0));
        historico.registrar(com_exposicao(1.0));
        historico.registrar(com_exposicao(2.0));
        historico.desfazer();

        historico.limpar();

        assert_eq!(nomes(&historico), [INICIO]);
        assert_eq!(atual(&historico), &com_exposicao(1.0));
        assert!(!historico.pode_desfazer() && !historico.pode_refazer());
    }

    // ------------------------------------------------------------ gravação

    fn cheio() -> Historico {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar(com_exposicao(0.5));
        let mut com_tudo = com_mascaras(2);
        com_tudo.ajustes.exposure = 0.5;
        com_tudo.corte = Corte {
            largura: Some(0.8),
            angulo: Some(-1.5),
            perspectiva: Some(PerspectivaGuiada {
                vertical: 3.0,
                ..Default::default()
            }),
            restringir: Some(false),
            ..Corte::default()
        };
        historico.registrar(com_tudo.clone());
        // Vários passos com a mesma Revelação local.
        for i in 1..=5 {
            let mut passo = com_tudo.clone();
            passo.ajustes.contrast = 1.0 + i as f32 / 100.0;
            historico.registrar(passo);
        }
        historico.desfazer();
        historico
    }

    #[test]
    fn o_json_vai_e_volta_igual() {
        let historico = cheio();
        let json = historico.em_json();

        let aberto = atual(&historico).clone();
        let volta = Historico::retomar(&json, aberto).expect("legível");

        assert_eq!(volta.passos, historico.passos);
        assert_eq!(volta.atual, historico.atual, "o passo atual também volta");
        assert!(volta.pode_refazer(), "o que estava desfeito continua lá");
    }

    /// Uma máscara repetida em vários passos é gravada uma vez só, e os
    /// ajustes só com o que difere do neutro.
    #[test]
    fn o_json_nao_repete_a_revelacao_local() {
        let json: Value = serde_json::from_str(&cheio().em_json()).expect("JSON");
        assert_eq!(
            json["locais"].as_array().map(Vec::len),
            Some(2),
            "o vazio e a de duas máscaras"
        );
        assert_eq!(
            json["passos"][1]["ajustes"],
            serde_json::json!({ "exposure": 0.5 })
        );
    }

    /// 🚨 A foto mudou fora da Revelação (colada na Biblioteca): o histórico
    /// ganha o passo de fora, e o `Cmd+Z` volta ao que estava.
    #[test]
    fn retomar_com_a_foto_mudada_por_fora_acrescenta_um_passo() {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar(com_exposicao(0.5));

        let mut volta =
            Historico::retomar(&historico.em_json(), com_exposicao(1.2)).expect("legível");

        assert_eq!(nomes(&volta), [INICIO, "Exposição", DE_FORA]);
        assert_eq!(atual(&volta), &com_exposicao(1.2));
        assert_eq!(volta.desfazer(), Some(com_exposicao(0.5)));
    }

    /// A leitura chegou depois do primeiro gesto: o gravado vem antes, os
    /// gestos de agora depois, e o desfeito continua desfeito.
    #[test]
    fn retomar_sob_poe_os_gestos_de_agora_por_cima() {
        let mut gravado = Historico::novo(Estado::default());
        gravado.registrar(com_exposicao(0.5));

        let mut agora = Historico::novo(com_exposicao(0.5));
        agora.registrar(com_exposicao(0.7));
        agora.registrar(com_exposicao(0.9));
        agora.desfazer();

        let junto = Historico::retomar_sob(&gravado.em_json(), &agora).expect("legível");

        assert_eq!(
            nomes(&junto),
            [INICIO, "Exposição", "Exposição", "Exposição"]
        );
        assert_eq!(atual(&junto), &com_exposicao(0.7));
        assert!(junto.pode_refazer());
    }

    /// ⚠️ O corte "foto inteira" escrito por extenso é o mesmo que nenhum
    /// corte — a foto do site reabre assim.
    #[test]
    fn retomar_nao_confunde_corte_inteiro_com_mudanca() {
        let mut historico = Historico::novo(Estado::default());
        historico.registrar(com_exposicao(0.5));
        let mut aberta = com_exposicao(0.5);
        aberta.corte = Corte {
            x: Some(0.0),
            y: Some(0.0),
            largura: Some(1.0),
            altura: Some(1.0),
            rotacao: Some(0),
            angulo: Some(0.0),
            espelho_h: Some(false),
            espelho_v: Some(false),
            ..Corte::default()
        };

        let volta = Historico::retomar(&historico.em_json(), aberta.clone()).expect("legível");

        assert_eq!(nomes(&volta), [INICIO, "Exposição"]);
        assert_eq!(atual(&volta), &aberta);
    }

    /// ⏱️ O histórico cheio vai ao disco a cada gesto: o JSON de 100 passos
    /// tem de custar bem menos que um quadro.
    #[test]
    fn o_json_do_historico_cheio_custa_pouco() {
        let mut historico = Historico::novo(com_mascaras(3));
        for i in 1..=TETO {
            let mut passo = com_mascaras(3);
            passo.ajustes.exposure = i as f32 / 100.0;
            passo.ajustes.contrast = 1.2;
            historico.registrar(passo);
        }
        let inicio = std::time::Instant::now();
        let mut tamanho = 0;
        for _ in 0..10 {
            tamanho = historico.em_json().len();
        }
        let cada = inicio.elapsed() / 10;
        eprintln!("⏱️ JSON de {TETO} passos: {cada:?}, {tamanho} bytes");
        assert!(cada < std::time::Duration::from_millis(16), "{cada:?}");
        assert!(tamanho < 64 * 1024, "{tamanho} bytes");
    }

    /// 🚨 Reabrir a foto com um passo desfeito: o atual continua o gravado,
    /// e o próximo gesto parte dele (visto no app, 03/10/2026).
    #[test]
    fn reabrir_com_passo_desfeito_mantem_o_atual() {
        let mut gravado = Historico::novo(Estado::default());
        gravado.registrar(com_exposicao(0.5));
        gravado.registrar(cortada(0.5));
        gravado.desfazer();

        let agora = Historico::novo(com_exposicao(0.5));
        let mut junto = Historico::retomar_sob(&gravado.em_json(), &agora).expect("legível");

        assert_eq!(junto.atual(), 1, "a foto está no passo da exposição");
        assert!(junto.pode_refazer(), "o corte desfeito continua lá");
        let mut endireitada = com_exposicao(0.5);
        endireitada.corte.angulo = Some(5.0);
        junto.registrar(endireitada);
        assert_eq!(nomes(&junto), [INICIO, "Exposição", "Endireitar"]);
    }

    #[test]
    fn retomar_recusa_o_ilegivel_e_o_mais_novo() {
        assert!(Historico::retomar("não é json", Estado::default()).is_err());
        let futuro = r#"{"versao":99,"atual":0,"passos":[{"nome":"Início"}]}"#;
        assert!(Historico::retomar(futuro, Estado::default()).is_err());
        let alem = r#"{"versao":1,"atual":3,"passos":[{"nome":"Início"}]}"#;
        assert!(Historico::retomar(alem, Estado::default()).is_err());
    }
}
