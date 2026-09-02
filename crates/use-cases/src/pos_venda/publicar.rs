//! O caso de uso que fecha o vão: da triagem à galeria do cliente.
//!
//! Para cada foto do lote: lê do catálogo, revela e enquadra **em memória**
//! (o mesmo `.wgsl` da tela e da exportação), e sobe o JPEG com o estado que a
//! tecla `B` decidiu. A falha de uma foto não interrompe o lote — mas não some:
//! ela chega pelo canal, e o resumo diz "27 de 28".
//!
//! 🔑 **O original sobe sem marca, sempre** (`ExportOptions::default()`). É o
//! site que marca a prévia; a decisão de quem vê o quê é o `estado`.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use domain::repositories::PhotoRepository;
use domain::services::pos_venda::{
    EstadoNoBalcao, FotoParaEnviar, Galeria, NovaGaleria, PosVendaApi, Sessao,
};
use domain::services::ImageExporter;
use domain::value_objects::{ExportOptions, PhotoId};
use domain::{DomainError, DomainResult};

/// O que a tela pede.
#[derive(Debug, Clone)]
pub struct Pedido {
    pub sessao: Sessao,
    pub galeria: NovaGaleria,
    /// Na ordem em que devem aparecer no site.
    pub fotos: Vec<PhotoId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progresso {
    Comecou {
        total: usize,
    },
    GaleriaCriada {
        id: String,
    },
    Enviada {
        nome: String,
        estado: EstadoNoBalcao,
    },
    /// Uma foto ficou de fora. O lote continua.
    Falhou {
        nome: String,
        erro: String,
    },
    /// O e-mail "suas fotos estão prontas" saiu (ou não) — depois do lote.
    ClienteAvisado {
        /// `None` = avisado; `Some(motivo)` = o site recusou (sem e-mail, etc.).
        falha: Option<String>,
    },
    Terminou {
        galeria_id: String,
        sucesso: usize,
        falhas: usize,
    },
}

pub struct PublicarNoPosVendaUseCase {
    fotos: Arc<dyn PhotoRepository>,
    exportador: Arc<dyn ImageExporter>,
    api: Arc<dyn PosVendaApi>,
}

impl PublicarNoPosVendaUseCase {
    pub fn new(
        fotos: Arc<dyn PhotoRepository>,
        exportador: Arc<dyn ImageExporter>,
        api: Arc<dyn PosVendaApi>,
    ) -> Self {
        Self {
            fotos,
            exportador,
            api,
        }
    }

    /// Cria a galeria e sobe as fotos, uma a uma, na ordem do pedido.
    ///
    /// Devolve a galeria criada mesmo que fotos tenham falhado — ela existe no
    /// site, e o operador precisa do id para completar pelo painel. Só a
    /// criação da galeria é fatal: sem ela não há onde subir.
    pub async fn execute(&self, pedido: Pedido, canal: Sender<Progresso>) -> DomainResult<Galeria> {
        if pedido.fotos.is_empty() {
            return Err(DomainError::InvalidOperation(
                "nenhuma foto para publicar".to_string(),
            ));
        }

        let _ = canal.send(Progresso::Comecou {
            total: pedido.fotos.len(),
        });

        let galeria = self
            .api
            .criar_galeria(&pedido.sessao, &pedido.galeria)
            .await?;
        let _ = canal.send(Progresso::GaleriaCriada {
            id: galeria.id.clone(),
        });

        let (mut sucesso, mut falhas) = (0usize, 0usize);
        for (ordem, id) in pedido.fotos.iter().enumerate() {
            match self
                .subir(&pedido.sessao, &galeria.id, id, ordem as u32)
                .await
            {
                Ok((nome, estado)) => {
                    sucesso += 1;
                    let _ = canal.send(Progresso::Enviada { nome, estado });
                }
                Err((nome, erro)) => {
                    falhas += 1;
                    let _ = canal.send(Progresso::Falhou { nome, erro });
                }
            }
        }

        // 📧 O aviso sai **depois** do lote inteiro, e só se alguma foto subiu:
        // um e-mail dizendo "suas fotos estão prontas" para uma galeria vazia
        // seria a primeira impressão errada. A falha do aviso não é falha da
        // publicação — as fotos estão no ar; o operador reenvia pelo painel.
        if sucesso > 0 {
            let falha = self
                .api
                .avisar_fotos_prontas(&pedido.sessao, &galeria.id)
                .await
                .err()
                .map(|e| e.to_string());
            let _ = canal.send(Progresso::ClienteAvisado { falha });
        }

        let _ = canal.send(Progresso::Terminou {
            galeria_id: galeria.id.clone(),
            sucesso,
            falhas,
        });
        Ok(galeria)
    }

    async fn subir(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        id: &PhotoId,
        ordem: u32,
    ) -> Result<(String, EstadoNoBalcao), (String, String)> {
        let photo = match self.fotos.find_by_id(id).await {
            Ok(Some(p)) => p,
            Ok(None) => return Err((id.to_string(), "foto não está mais no catálogo".into())),
            Err(e) => return Err((id.to_string(), e.to_string())),
        };

        let nome = nome_para_o_site(&photo.file_path().to_string_lossy());
        let estado = EstadoNoBalcao::da_foto(&photo);

        let jpeg = self
            .exportador
            .renderizar_jpeg(&photo, &ExportOptions::default())
            .await
            .map_err(|e| (nome.clone(), e.to_string()))?;

        self.api
            .enviar_foto(
                sessao,
                galeria_id,
                FotoParaEnviar {
                    nome: nome.clone(),
                    jpeg,
                    estado,
                    ordem,
                },
            )
            .await
            .map_err(|e| (nome.clone(), e.to_string()))?;

        Ok((nome, estado))
    }
}

/// O nome que o cliente vê no site: o do arquivo de origem, com `.jpg`,
/// porque o que sobe é sempre o JPEG revelado — `DSC_001.NEF` viraria um
/// download chamado `.NEF` contendo um JPEG.
pub fn nome_para_o_site(caminho: &str) -> String {
    let arquivo = caminho.rsplit(['/', '\\']).next().unwrap_or(caminho);
    let base = arquivo.rsplit_once('.').map(|(b, _)| b).unwrap_or(arquivo);
    let base = if base.is_empty() { "foto" } else { base };
    format!("{base}.jpg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::entities::Photo;
    use domain::services::pos_venda::FotoEnviada;
    use domain::value_objects::FilePath;
    use mockall::mock;
    use std::sync::mpsc::channel;
    use std::sync::Mutex;

    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
        }
    }

    mock! {
        pub Exportador {}

        #[async_trait::async_trait]
        impl ImageExporter for Exportador {
            async fn export(&self, photo: &Photo, output_path: &FilePath, options: &ExportOptions) -> DomainResult<()>;
            async fn renderizar_jpeg(&self, photo: &Photo, options: &ExportOptions) -> DomainResult<Vec<u8>>;
        }
    }

    /// A API de mentira: registra o que subiu, com o estado de cada foto.
    #[derive(Default)]
    struct ApiDeMentira {
        enviadas: Mutex<Vec<(String, EstadoNoBalcao, u32)>>,
        galerias: Mutex<Vec<NovaGaleria>>,
        /// Nomes que devem falhar ao subir.
        falham: Vec<String>,
        avisadas: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl PosVendaApi for ApiDeMentira {
        async fn entrar(&self, _: &str, _: &str) -> DomainResult<Sessao> {
            unreachable!("o caso de uso não entra; recebe a sessão pronta")
        }
        async fn produtos(
            &self,
            _: &Sessao,
        ) -> DomainResult<Vec<domain::services::pos_venda::Produto>> {
            Ok(vec![])
        }
        async fn criar_galeria(&self, _: &Sessao, nova: &NovaGaleria) -> DomainResult<Galeria> {
            self.galerias.lock().unwrap().push(nova.clone());
            Ok(Galeria {
                id: "g1".into(),
                titulo: nova.titulo.clone(),
            })
        }
        async fn enviar_foto(
            &self,
            _: &Sessao,
            _: &str,
            foto: FotoParaEnviar,
        ) -> DomainResult<FotoEnviada> {
            if self.falham.contains(&foto.nome) {
                return Err(DomainError::InfrastructureError("caiu".into()));
            }
            self.enviadas
                .lock()
                .unwrap()
                .push((foto.nome, foto.estado, foto.ordem));
            Ok(FotoEnviada { id: "f".into() })
        }
        async fn avisar_fotos_prontas(&self, _: &Sessao, galeria_id: &str) -> DomainResult<()> {
            self.avisadas.lock().unwrap().push(galeria_id.to_string());
            Ok(())
        }
    }

    fn foto(caminho: &str, comprada: bool) -> Photo {
        let mut p = Photo::new(FilePath::new(caminho).unwrap());
        if comprada {
            p.marcar_comprada();
        }
        p
    }

    fn pedido(fotos: &[&Photo]) -> Pedido {
        Pedido {
            sessao: Sessao {
                access_token: "tok".into(),
            },
            galeria: NovaGaleria {
                titulo: "Ensaio".into(),
                email: Some("maria@x.com".into()),
                whatsapp: None,
                produto_id: "p1".into(),
            },
            fotos: fotos.iter().map(|f| f.id()).collect(),
        }
    }

    /// 🔑 A tecla `B` é a única coisa que decide o estado no site.
    #[tokio::test]
    async fn a_levada_sobe_como_levada_e_a_outra_como_disponivel_na_ordem() {
        let levada = foto("/ensaio/DSC_001.NEF", true);
        let ficou = foto("/ensaio/DSC_002.NEF", false);

        let mut repo = MockPhotoRepo::new();
        for f in [&levada, &ficou] {
            let devolvida = f.clone();
            repo.expect_find_by_id()
                .withf(move |id| *id == devolvida.id())
                .returning({
                    let f = f.clone();
                    move |_| Ok(Some(f.clone()))
                });
        }
        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .withf(|_, opcoes| opcoes.watermark().is_none() && opcoes.longest_edge().is_none())
            .returning(|_, _| Ok(vec![1, 2, 3]));
        let api = Arc::new(ApiDeMentira::default());

        let (tx, rx) = channel();
        let galeria =
            PublicarNoPosVendaUseCase::new(Arc::new(repo), Arc::new(exportador), api.clone())
                .execute(pedido(&[&levada, &ficou]), tx)
                .await
                .unwrap();
        assert_eq!(galeria.id, "g1");

        let enviadas = api.enviadas.lock().unwrap().clone();
        assert_eq!(
            enviadas,
            vec![
                ("DSC_001.jpg".to_string(), EstadoNoBalcao::LevadaNoBalcao, 0),
                ("DSC_002.jpg".to_string(), EstadoNoBalcao::Disponivel, 1),
            ]
        );

        let eventos: Vec<Progresso> = rx.try_iter().collect();
        assert!(matches!(
            eventos.last(),
            Some(Progresso::Terminou {
                sucesso: 2,
                falhas: 0,
                ..
            })
        ));
        // 📧 E o cliente é avisado uma vez, depois do lote.
        assert_eq!(api.avisadas.lock().unwrap().as_slice(), ["g1"]);
        assert!(eventos
            .iter()
            .any(|e| matches!(e, Progresso::ClienteAvisado { falha: None })));
    }

    /// A falha de uma foto não interrompe o lote, e o resumo a conta.
    #[tokio::test]
    async fn uma_foto_que_falha_nao_derruba_as_outras() {
        let a = foto("/ensaio/a.NEF", false);
        let b = foto("/ensaio/b.NEF", false);
        let mut repo = MockPhotoRepo::new();
        for f in [&a, &b] {
            let f = f.clone();
            let id = f.id();
            repo.expect_find_by_id()
                .withf(move |x| *x == id)
                .returning(move |_| Ok(Some(f.clone())));
        }
        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));
        let api = Arc::new(ApiDeMentira {
            falham: vec!["a.jpg".into()],
            ..Default::default()
        });

        let (tx, rx) = channel();
        PublicarNoPosVendaUseCase::new(Arc::new(repo), Arc::new(exportador), api.clone())
            .execute(pedido(&[&a, &b]), tx)
            .await
            .unwrap();

        assert_eq!(api.enviadas.lock().unwrap().len(), 1);
        let eventos: Vec<Progresso> = rx.try_iter().collect();
        assert!(eventos
            .iter()
            .any(|e| matches!(e, Progresso::Falhou { nome, .. } if nome == "a.jpg")));
        assert!(matches!(
            eventos.last(),
            Some(Progresso::Terminou {
                sucesso: 1,
                falhas: 1,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn sem_fotos_nao_cria_galeria() {
        let api = Arc::new(ApiDeMentira::default());
        let (tx, _rx) = channel();
        let erro = PublicarNoPosVendaUseCase::new(
            Arc::new(MockPhotoRepo::new()),
            Arc::new(MockExportador::new()),
            api.clone(),
        )
        .execute(pedido(&[]), tx)
        .await
        .unwrap_err();
        assert!(matches!(erro, DomainError::InvalidOperation(_)));
        assert!(api.galerias.lock().unwrap().is_empty());
    }

    #[test]
    fn o_nome_no_site_e_o_da_origem_com_jpg() {
        assert_eq!(nome_para_o_site("/ensaio/DSC_001.NEF"), "DSC_001.jpg");
        assert_eq!(nome_para_o_site("C:\\fotos\\IMG.jpeg"), "IMG.jpg");
        assert_eq!(nome_para_o_site("semextensao"), "semextensao.jpg");
    }
}
