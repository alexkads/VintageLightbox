//! O depósito das revelações de fotos do site que ainda não subiram.

use std::sync::Arc;

use domain::repositories::RevelacoesDoSiteRepository;
use domain::DomainResult;

/// Guarda, lê e esquece a receita de uma foto que **só existe no site**.
///
/// # Por que existe
///
/// A foto aberta a partir de uma sessão do pós-venda não é linha do catálogo
/// (o id dela é `site:<uuid>`), então `SavePhotoEditsUseCase` não tem onde
/// escrever: até 8/set/2026 revelá-la não gravava nada, e o dono viu isso como
/// *"os parâmetros de edição não estão sendo gravados"*. Este é o lugar onde a
/// receita dela espera — o mesmo papel do depósito local do site.
///
/// # Três métodos, um caso de uso
///
/// 🔑 São as três pontas do **mesmo** ciclo, e separá-las em três tipos daria
/// três construções no `main.rs` para um único fluxo. O ciclo é: o gesto grava,
/// a abertura lê, e o envio à galeria esquece.
///
/// ⚠️ **`ajustes` é opaco aqui**, como no backend do site: é o JSON que sobe
/// para a API e volta dela. Quem conhece os nomes é o motor.
pub struct RevelacoesLocaisUseCase {
    repositorio: Arc<dyn RevelacoesDoSiteRepository>,
}

impl RevelacoesLocaisUseCase {
    pub fn new(repositorio: Arc<dyn RevelacoesDoSiteRepository>) -> Self {
        Self { repositorio }
    }

    /// O gesto acabou de acontecer: a receita desta foto passa a ser esta.
    pub async fn guardar(&self, foto_no_site: &str, ajustes: &str) -> DomainResult<()> {
        self.repositorio.guardar(foto_no_site, ajustes).await
    }

    /// Tudo o que ficou por subir, para a abertura do app carregar de uma vez.
    pub async fn todas(&self) -> DomainResult<Vec<(String, String)>> {
        self.repositorio.todas().await
    }

    /// A revelação subiu: o servidor passa a ser a verdade desta foto.
    pub async fn esquecer(&self, foto_no_site: &str) -> DomainResult<()> {
        self.repositorio.esquecer(foto_no_site).await
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Mutex;

    /// Um depósito de mentira: um mapa, e o registro do que foi pedido.
    #[derive(Default)]
    struct DepositoDeMentira {
        linhas: Mutex<Vec<(String, String)>>,
    }

    #[async_trait::async_trait]
    impl RevelacoesDoSiteRepository for DepositoDeMentira {
        async fn guardar(&self, foto_no_site: &str, ajustes: &str) -> DomainResult<()> {
            let mut linhas = self.linhas.lock().unwrap();
            linhas.retain(|(id, _)| id != foto_no_site);
            linhas.push((foto_no_site.to_string(), ajustes.to_string()));
            Ok(())
        }
        async fn todas(&self) -> DomainResult<Vec<(String, String)>> {
            Ok(self.linhas.lock().unwrap().clone())
        }
        async fn esquecer(&self, foto_no_site: &str) -> DomainResult<()> {
            self.linhas
                .lock()
                .unwrap()
                .retain(|(id, _)| id != foto_no_site);
            Ok(())
        }
    }

    /// 🔑 O ciclo inteiro: o gesto grava, a abertura lê, o envio esquece.
    #[tokio::test]
    async fn o_gesto_grava_a_abertura_le_e_o_envio_esquece() {
        let caso = RevelacoesLocaisUseCase::new(Arc::new(DepositoDeMentira::default()));

        caso.guardar("remota-1", r#"{"exposure":1.25}"#)
            .await
            .unwrap();
        assert_eq!(
            caso.todas().await.unwrap(),
            vec![("remota-1".to_string(), r#"{"exposure":1.25}"#.to_string())]
        );

        // O gesto seguinte substitui: o depósito guarda o estado, não a história.
        caso.guardar("remota-1", r#"{"exposure":0.5}"#)
            .await
            .unwrap();
        assert_eq!(caso.todas().await.unwrap().len(), 1);
        assert_eq!(caso.todas().await.unwrap()[0].1, r#"{"exposure":0.5}"#);

        caso.esquecer("remota-1").await.unwrap();
        assert!(
            caso.todas().await.unwrap().is_empty(),
            "subiu, e saiu daqui"
        );
    }
}
