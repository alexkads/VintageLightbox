//! A porta para o pós-venda do `recordarfotos.com.br`.
//!
//! É o vão que o projeto existe para fechar ([`docs/00-OBJETIVO.md`]): a decisão
//! que o fotógrafo toma na triagem — esta foi levada, esta ficou — vira galeria
//! no site sem passo manual. A porta sabe **quatro coisas** e nada mais: entrar,
//! listar o produto que dá o preço, criar a galeria do cliente, subir uma foto
//! com o estado dela.
//!
//! 🔑 **O original sobe sem marca, sempre.** É o site que gera a prévia marcada
//! a partir dele (uma vez, no upload) e que decide, pelo `estado`, se o cliente
//! baixa o original ou vê a prévia. Subir a "prévia da galeria" da exportação
//! local seria mandar uma foto reduzida e marcada como se fosse o produto — e o
//! cliente que pagasse receberia isso.
//!
//! O `domain` não sabe HTTP: quem fala com a rede é a `infrastructure`, e os
//! testes do caso de uso rodam com um dublê desta trait.

use async_trait::async_trait;

use crate::entities::Photo;
use crate::DomainResult;

/// O que fica de um login: o token que as outras chamadas carregam.
///
/// Só o de acesso. O de renovação não entra aqui de propósito — a sessão vive
/// enquanto o app está aberto, e um token de renovação guardado em disco ao
/// lado do catálogo seria a credencial do estúdio num JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessao {
    pub access_token: String,
}

/// Um produto do catálogo do site — o que dá o preço de cada foto à venda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Produto {
    pub id: String,
    pub nome: String,
    /// Como o site devolve: decimal em texto (`"29.90"`), sem conversão aqui.
    pub preco: String,
    /// Fora da vitrine — e é o caso esperado do produto "foto avulsa".
    pub inativo: bool,
}

/// A galeria do cliente, como o balcão a descreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaGaleria {
    pub titulo: String,
    /// Ao menos um dos dois; quem confere é o site, e a recusa volta como erro.
    pub email: Option<String>,
    pub whatsapp: Option<String>,
    pub produto_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Galeria {
    pub id: String,
    pub titulo: String,
}

/// O que o cliente decidiu no balcão — os dois estados que uma foto pode ter
/// ao entrar no site. `comprada` nasce lá, de pedido pago, nunca daqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoNoBalcao {
    LevadaNoBalcao,
    Disponivel,
}

impl EstadoNoBalcao {
    /// O texto que a API recebe no campo `estado`.
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::LevadaNoBalcao => "levada_no_balcao",
            Self::Disponivel => "disponivel",
        }
    }

    /// 🔑 **A única tradução de `Photo::comprada` para o site.** É a marcação
    /// da tecla `B`; não existe um segundo lugar onde essa decisão seja tomada.
    pub fn da_foto(photo: &Photo) -> Self {
        if photo.comprada() {
            Self::LevadaNoBalcao
        } else {
            Self::Disponivel
        }
    }
}

/// Uma foto pronta para subir: o JPEG já revelado e enquadrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoParaEnviar {
    /// O nome que o cliente vê no site — o do arquivo de origem, com `.jpg`.
    pub nome: String,
    pub jpeg: Vec<u8>,
    pub estado: EstadoNoBalcao,
    pub ordem: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoEnviada {
    pub id: String,
}

#[async_trait]
pub trait PosVendaApi: Send + Sync {
    /// E-mail e senha do operador. Credencial recusada é
    /// [`crate::DomainError::AcessoRecusado`], e não erro de infraestrutura:
    /// a tela precisa dizer "senha errada" e não "sem rede".
    async fn entrar(&self, email: &str, senha: &str) -> DomainResult<Sessao>;

    /// O catálogo administrativo — inclusive inativos.
    async fn produtos(&self, sessao: &Sessao) -> DomainResult<Vec<Produto>>;

    async fn criar_galeria(&self, sessao: &Sessao, nova: &NovaGaleria) -> DomainResult<Galeria>;

    async fn enviar_foto(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        foto: FotoParaEnviar,
    ) -> DomainResult<FotoEnviada>;

    /// Manda ao cliente o e-mail "suas fotos estão prontas" — com os prazos de
    /// download e de venda e um link que entra sem senha. É o site quem
    /// escreve e manda; o app só pede.
    async fn avisar_fotos_prontas(&self, sessao: &Sessao, galeria_id: &str) -> DomainResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_estado_sai_da_marcacao_do_balcao() {
        let mut foto = Photo::new_test();
        assert_eq!(EstadoNoBalcao::da_foto(&foto), EstadoNoBalcao::Disponivel);
        foto.marcar_comprada();
        assert_eq!(
            EstadoNoBalcao::da_foto(&foto),
            EstadoNoBalcao::LevadaNoBalcao
        );
    }

    /// Os dois textos são os que o CHECK da migration do site aceita.
    #[test]
    fn os_textos_sao_os_da_api() {
        assert_eq!(
            EstadoNoBalcao::LevadaNoBalcao.como_texto(),
            "levada_no_balcao"
        );
        assert_eq!(EstadoNoBalcao::Disponivel.como_texto(), "disponivel");
    }
}
