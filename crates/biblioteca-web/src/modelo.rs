//! O que o site manda para a tela — e só o que a tela precisa.
//!
//! O JSON é montado no servidor do site (`estado-da-galeria.ts`), com datas
//! **já formatadas** em português e preços em centavos: formatar data exige
//! fuso e idioma, que o site já resolve; formatar dinheiro é do core
//! (`biblioteca_core::dinheiro`), porque é regra do balcão.
//!
//! ⚠️ **Campo que falta não derruba a tela.** Todo campo novo é `#[serde(default)]`:
//! a Vercel e o Fly sobem separados, e por alguns minutos o JSON pode vir de
//! uma versão sem ele (armadilha nº 33 do projeto).

use biblioteca_core::acervo::{Estado as EstadoDaFoto, Foto as FotoDoCore};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Galeria {
    pub id: String,
    pub titulo: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub whatsapp: Option<String>,
    #[serde(default)]
    pub cliente_ja_abriu: bool,
    #[serde(default)]
    pub estudio_id: Option<String>,
    #[serde(default)]
    pub criada_por: String,
    #[serde(default)]
    pub criada_em: String,
    #[serde(default)]
    pub expira_em: Option<String>,
    #[serde(default)]
    pub prorrogada_ate: Option<String>,
    #[serde(default)]
    pub vence_venda: Option<String>,
    #[serde(default)]
    pub vence_download: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Faixa {
    pub id: String,
    pub nome: String,
    /// O preço **de balcão** (cheio), em centavos.
    pub preco: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Estudio {
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub cidade: String,
}

/// Um e-mail que saiu para o cliente, já em texto — o site sabe formatar data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Aviso {
    pub tipo: String,
    pub enviado_em: String,
    pub destino: String,
    /// "lido …", "entregue, sem leitura" ou "sem retorno do provedor".
    #[serde(default)]
    pub situacao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Foto {
    pub id: String,
    pub arquivo: String,
    pub estado: String,
    #[serde(default)]
    pub apagada_em: Option<String>,
    #[serde(default)]
    pub tamanho_mb: String,
    #[serde(default)]
    pub downloads: u32,
    #[serde(default)]
    pub pedido_id: Option<String>,
    #[serde(default)]
    pub liberada_em: Option<String>,
    /// O que está gravado nesta foto. `None` = segue o padrão da galeria.
    #[serde(default)]
    pub produto_id: Option<String>,
    /// O que **vale** — resolvido pelo backend, nunca aqui.
    #[serde(default)]
    pub produto_efetivo: String,
    #[serde(default)]
    pub preco_negociado: Option<i64>,
    #[serde(default)]
    pub observacao: Option<String>,
    #[serde(default)]
    pub preco_de_venda: Option<i64>,
    #[serde(default)]
    pub ordem: i64,
    #[serde(default)]
    pub revelada_em: Option<String>,
    /// A prévia de 1400 px — o que abre no duplo clique.
    #[serde(default)]
    pub previa: String,
    /// A miniatura de 640 px — o que a grade desenha, e a chave da textura.
    #[serde(default)]
    pub miniatura: String,
}

impl Foto {
    pub fn para_core(&self) -> Result<FotoDoCore, String> {
        let estado = EstadoDaFoto::do_texto(&self.estado)
            .ok_or_else(|| format!("estado desconhecido: {}", self.estado))?;
        Ok(FotoDoCore {
            id: self.id.clone(),
            arquivo: self.arquivo.clone(),
            estado,
            apagada: self.apagada_em.is_some(),
            produto_efetivo: self.produto_efetivo.clone(),
            preco_negociado: self.preco_negociado,
            tem_observacao: self.observacao.as_deref().is_some_and(|o| !o.is_empty()),
            preco_de_venda: self.preco_de_venda,
            pedido_id: self.pedido_id.clone(),
            downloads: self.downloads,
            revelada: self.revelada_em.is_some(),
            ordem: self.ordem,
        })
    }

    pub fn editavel(&self) -> bool {
        self.estado != "comprada" && self.apagada_em.is_none()
    }

    pub fn rotulo_do_estado(&self) -> &'static str {
        if self.apagada_em.is_some() {
            return "Apagada";
        }
        EstadoDaFoto::do_texto(&self.estado).map_or("?", EstadoDaFoto::rotulo)
    }
}

/// O estado inteiro da galeria, como o site o monta.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EstadoDaGaleria {
    pub galeria: Galeria,
    /// O produto padrão: o que vale para a foto que ninguém marcou.
    pub produto: Faixa,
    /// O catálogo mais as faixas em uso — onde a tela acha nome e preço.
    #[serde(default)]
    pub faixas: Vec<Faixa>,
    /// O que a compra online cobra por produto — a referência do preço fixado.
    #[serde(default)]
    pub precos_online: std::collections::HashMap<String, i64>,
    #[serde(default)]
    pub estudios: Vec<Estudio>,
    #[serde(default)]
    pub avisos: Vec<Aviso>,
    #[serde(default)]
    pub ja_avisado: bool,
    #[serde(default)]
    pub fotos: Vec<Foto>,
}

impl EstadoDaGaleria {
    pub fn faixa(&self, id: &str) -> Option<&Faixa> {
        self.faixas.iter().find(|f| f.id == id)
    }

    /// O preço de balcão da faixa que vale para a foto.
    pub fn preco_da_faixa(&self, foto: &Foto) -> i64 {
        self.faixa(&foto.produto_efetivo)
            .map_or(self.produto.preco, |f| f.preco)
    }

    /// O que o cliente paga online sem valor fixado: a loja, não o balcão.
    pub fn preco_online_da_faixa(&self, foto: &Foto) -> i64 {
        self.precos_online
            .get(&foto.produto_efetivo)
            .or_else(|| self.precos_online.get(&self.produto.id))
            .copied()
            .unwrap_or_else(|| self.preco_da_faixa(foto))
    }

    pub fn nome_da_faixa(&self, id: &str) -> String {
        self.faixa(id).map_or_else(
            || "faixa fora do catálogo".to_string(),
            |f| {
                format!(
                    "{} — {}",
                    f.nome,
                    biblioteca_core::dinheiro::formatar(f.preco)
                )
            },
        )
    }

    /// Quantas fotos desta galeria estão no mesmo pedido.
    pub fn contar_no_pedido(&self, pedido_id: &str) -> usize {
        self.fotos
            .iter()
            .filter(|f| f.pedido_id.as_deref() == Some(pedido_id))
            .count()
    }
}

/// O que o site diz sobre a fila de importação — o Worker é dele.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ItemDaImportacao {
    pub id: String,
    #[serde(default)]
    pub nome_original: String,
    pub estado: String,
    #[serde(default)]
    pub progresso: u32,
    #[serde(default)]
    pub bytes_originais: u64,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub erro: Option<String>,
    #[serde(default)]
    pub aviso: Option<String>,
}

/// O que o editor de revelação deixou no depósito local sobre uma foto.
///
/// Só o que a biblioteca mostra: **quando** e **se já subiu**. Os 46 ajustes
/// e o enquadramento ficam no mesmo registro, mas são do editor.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RevelacaoLocal {
    #[serde(default)]
    pub atualizada_em: String,
    #[serde(default)]
    pub sincronizada: bool,
}

/// O recibo do MercadoPago de uma foto comprada, como o site o devolve.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Pagamento {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub meio: String,
    /// ⚠️ Em **reais**, como o gateway devolve.
    #[serde(default)]
    pub valor: f64,
    #[serde(default)]
    pub liquido: Option<f64>,
    #[serde(default)]
    pub quando: String,
    #[serde(default)]
    pub pagador: Option<String>,
}
