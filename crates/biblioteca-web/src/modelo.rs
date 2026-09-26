//! O que o site manda para a grade — e só o que a grade precisa.
//!
//! O site guarda a foto inteira (nome, faixa, negociação, preço, datas) e é ele
//! quem escreve isso na tela, em DOM. Para cá vem o mínimo que filtrar, contar,
//! selecionar e desenhar exigem: quem é, qual miniatura, em que situação está.
//!
//! ⚠️ **Campo que falta não derruba a grade.** Todo campo é `#[serde(default)]`
//! menos o `id`: a Vercel sobe o site e o wasm juntos, mas um JSON de uma
//! versão sem o campo não pode virar tela vazia (armadilha nº 33 do projeto).

use biblioteca_core::acervo::{Estado, Foto};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FotoJson {
    pub id: String,
    /// A miniatura de 640 px — o que a grade desenha, e a chave da textura.
    #[serde(default)]
    pub miniatura: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub apagada: bool,
    /// A nota de 1 a 5; ausente ou nula = não classificada.
    #[serde(default)]
    pub nota: Option<u8>,
    /// Rejeitada com a tecla `X` (C21). Ausente = não rejeitada, que é o que
    /// um site sem o campo quer dizer.
    #[serde(default)]
    pub rejeitada: bool,
    #[serde(default)]
    pub ordem: i64,
    /// O nome do arquivo — o que a local e a do servidor têm em comum quando
    /// ela sobe (ver `emprestimo`). Ausente = nenhuma herança por nome.
    #[serde(default)]
    pub arquivo: String,
    /// A prévia grande (1400 px) — o que a grade desenha quando o tile passa
    /// do tamanho da miniatura (a "uma por linha" do zoom). Ausente = a grade
    /// fica na miniatura.
    #[serde(default)]
    pub previa: String,
}

impl FotoJson {
    /// ⚠️ Situação desconhecida é erro, não "a mais parecida" (armadilha nº 6):
    /// tratar `comprada` como `disponivel` poria à venda o que já foi vendido.
    pub fn para_core(&self) -> Result<Foto, String> {
        let estado = Estado::do_texto(&self.estado)
            .ok_or_else(|| format!("estado desconhecido: {}", self.estado))?;
        Ok(Foto {
            id: self.id.clone(),
            arquivo: String::new(),
            estado,
            apagada: self.apagada,
            produto_efetivo: String::new(),
            preco_negociado: None,
            observacao: None,
            preco_de_venda: None,
            pedido_id: None,
            downloads: 0,
            revelada: false,
            nota: self.nota,
            // 🚨 **Faltava**: o core ganhou o campo em 2026-09-20 e esta ponte
            // não o passava — a grade do site nunca soube que uma foto estava
            // rejeitada, e a contava "à venda" (D21).
            rejeitada: self.rejeitada,
            ordem: self.ordem,
        })
    }
}
