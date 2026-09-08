//! O caso de uso que fecha o vão: da triagem à galeria do cliente.
//!
//! Para cada foto do lote: lê do catálogo, revela e enquadra **em memória**
//! (o mesmo `.wgsl` da tela e da exportação), e sobe o JPEG com o estado que a
//! tecla `B` decidiu. A falha de uma foto não interrompe o lote — mas não some:
//! ela chega pelo canal, e o resumo diz "27 de 28".
//!
//! 🔑 **O original sobe sem marca, sempre** (`ExportOptions::default()`). É o
//! site que marca a prévia; a decisão de quem vê o quê é o `estado`.

use std::sync::Arc;

use domain::repositories::PhotoRepository;
use domain::services::pos_venda::{EstadoNoBalcao, FotoParaEnviar, PosVendaApi, Sessao};
use domain::services::ImageExporter;
use domain::services::ThumbnailGenerator;
use domain::value_objects::{ExportOptions, FilePath, PhotoId};
use domain::DomainError;

pub struct PublicarNoPosVendaUseCase {
    fotos: Arc<dyn PhotoRepository>,
    exportador: Arc<dyn ImageExporter>,
    /// Quem transforma um arquivo do disco na cópia que sobe.
    ///
    /// 🔑 **É o mesmo gerador das miniaturas**, e não um segundo caminho: ele já
    /// sabe abrir RAW, JPEG, TIFF e HEIC, e já reduz para um lado máximo. Um
    /// redimensionador próprio aqui seria uma segunda resposta para a mesma
    /// pergunta — e as duas divergiriam no primeiro formato novo.
    preparador: Arc<dyn ThumbnailGenerator>,
    api: Arc<dyn PosVendaApi>,
}

/// O lado maior do que sobe, em pixels.
///
/// É o padrão da web (`importacao/parametros.ts`): grande o bastante para
/// impressão de balcão, pequeno o bastante para uma sessão de duzentas não virar
/// gigabytes. O original **não** sobe daqui — quem quer o arquivo cheio exporta.
pub const LADO_DO_ENVIO: u32 = 4000;

impl PublicarNoPosVendaUseCase {
    pub fn new(
        fotos: Arc<dyn PhotoRepository>,
        exportador: Arc<dyn ImageExporter>,
        preparador: Arc<dyn ThumbnailGenerator>,
        api: Arc<dyn PosVendaApi>,
    ) -> Self {
        Self {
            fotos,
            exportador,
            preparador,
            api,
        }
    }

    /// Sobe um **arquivo do disco** para uma sessão — sem passar pelo catálogo.
    ///
    /// 🔑 **É o envio da web trazido para cá**: o operador exporta do Lightroom
    /// para uma pasta e arrasta a pasta para a sessão. A foto entregue ao cliente
    /// não precisa estar catalogada aqui — o catálogo é da triagem em RAW, e são
    /// dois trabalhos diferentes.
    ///
    /// ⚠️ **O nome que o cliente vê é o do arquivo**, com `.jpg`: o que sobe é
    /// sempre JPEG, e `DSC_001.NEF` viraria um download `.NEF` contendo um JPEG.
    pub async fn enviar_arquivo(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        caminho: &str,
        ordem: u32,
        estado: EstadoNoBalcao,
    ) -> Result<String, String> {
        let nome = nome_para_o_site(caminho);
        let arquivo = FilePath::new(caminho).map_err(|e| format!("{nome}: {e}"))?;

        let jpeg = self
            .preparador
            .generate(&arquivo, LADO_DO_ENVIO)
            .await
            .map_err(|e| format!("{nome}: {e}"))?;

        self.api
            .enviar_foto(
                sessao,
                galeria_id,
                FotoParaEnviar {
                    nome: nome.clone(),
                    jpeg,
                    estado,
                    ordem,
                    // ⚠️ **Sem nota e sem chave, e é o que este caminho é.** Ele
                    // sobe um arquivo do disco que não passou pelo catálogo:
                    // não há linha de onde tirar a classificação, nem id que
                    // sobreviva a uma segunda tentativa. O site vai recusá-lo
                    // enquanto exigir foto classificada — e isso é correto: o
                    // que autoriza uma foto a subir é o passo 3.
                    nota: None,
                    chave_do_cliente: None,
                },
            )
            .await
            .map_err(|e| format!("{nome}: {e}"))?;

        Ok(nome)
    }

    /// Sobe **uma** foto para uma galeria que já existe.
    ///
    /// 🔑 É o passo 3 do fluxo do dono: classificar autoriza a foto a subir, e a
    /// galeria já foi escolhida antes. O mesmo caminho de [`Self::execute`], sem
    /// a criação da galeria e sem o aviso ao cliente — que sai no fim do lote,
    /// e não a cada estrela.
    /// 🚨 **`nota` vem de quem classificou, e não do banco.** Quem sobe a foto é
    /// a travessia do zero, que acontece na mesma tecla que grava a nota — e a
    /// gravação é uma tarefa do tokio que ninguém espera. Ler o banco aqui é uma
    /// corrida com ela: quando a leitura chega primeiro, o que sobe é o valor
    /// **anterior**, e o site devolve `400` — *"nota invalida: 0 (use de 1 a
    /// 5)"* quando o anterior era zero, *"informe a nota de 1 a 5"* quando era
    /// ausente. Nos dois casos a foto não sobe e a mensagem fala de uma nota que
    /// o app tinha na mão.
    ///
    /// `None` mantém o comportamento de ler do banco, que é o certo para quem
    /// publica em lote uma foto já classificada há tempos.
    pub async fn enviar_uma(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        id: &PhotoId,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
        nota: Option<u8>,
    ) -> Result<String, String> {
        self.subir(sessao, galeria_id, id, ordem, estado, nota)
            .await
            .map(|(nome, _)| nome)
            .map_err(|(nome, erro)| format!("{nome}: {erro}"))
    }

    /// **Salvar na galeria**: o JPEG revelado entra no lugar do original.
    ///
    /// 🔑 É o botão do editor, e o caminho é o mesmo do site: um bilhete de uma
    /// hora emitido para *esta* foto, e o envio por ele. A foto já é da galeria
    /// — nada aqui cria galeria, escolhe produto ou pergunta pelo cliente, que
    /// é o que o editor da web também não faz.
    ///
    /// ⚠️ **Os pixels vêm de fora, prontos.** Quem revela é a tela, com o mesmo
    /// motor que desenhou o que o operador está vendo; revelar de novo aqui, do
    /// arquivo, entregaria ao cliente uma imagem que ninguém conferiu.
    pub async fn salvar_revelacao(
        &self,
        sessao: &Sessao,
        foto_no_site: &str,
        jpeg: Vec<u8>,
        ajustes: serde_json::Value,
    ) -> Result<(), String> {
        let bilhete = self
            .api
            .bilhete_de_revelacao(sessao, foto_no_site)
            .await
            .map_err(|e| e.to_string())?;
        self.api
            .salvar_revelacao(&bilhete, jpeg, ajustes)
            .await
            .map_err(|e| e.to_string())
    }

    /// Devolve a foto do site ao **original**: o bruto volta ao lugar dele.
    ///
    /// 🔑 É o "Zerar tudo" salvo, e o par de [`Self::salvar_revelacao`]. Sem
    /// bilhete e sem pixels: nada sobe. Subir o bruto revelado com os ajustes
    /// neutros entregaria ao cliente uma geração a mais de JPEG no lugar do
    /// arquivo dele — que é justamente o que "voltar ao original" quer evitar.
    pub async fn restaurar_original(
        &self,
        sessao: &Sessao,
        foto_no_site: &str,
    ) -> Result<(), String> {
        self.api
            .restaurar_original(sessao, foto_no_site)
            .await
            .map_err(|e| e.to_string())
    }

    /// Tira a foto do storage — o que zerar a classificação faz.
    ///
    /// 🚨 **O `None` local sai mesmo quando o site diz que não achou.** Um id que
    /// não existe mais lá significa que alguém já removeu a foto por outra tela;
    /// insistir em guardá-lo aqui deixaria o catálogo apontando para o vazio e
    /// tentando remover de novo a cada estrela apagada.
    ///
    /// Foto que nunca subiu não é erro: é o caso normal de quem tira a nota de
    /// uma foto que nunca a teve.
    pub async fn remover_do_site(&self, sessao: &Sessao, id: &PhotoId) -> Result<(), String> {
        let Some(mut photo) = self.fotos.find_by_id(id).await.map_err(|e| e.to_string())? else {
            return Err("foto não está mais no catálogo".into());
        };
        let Some(remoto) = photo.id_no_site().map(str::to_string) else {
            return Ok(());
        };

        let recusa = self.api.remover_foto(sessao, &remoto).await.err();

        photo.definir_id_no_site(None);
        self.fotos.update(&photo).await.map_err(|e| e.to_string())?;

        match recusa {
            // 404 é "já não está lá", e isso é o desfecho desejado.
            Some(DomainError::NaoEncontradoNoSite(_)) | None => Ok(()),
            Some(erro) => Err(erro.to_string()),
        }
    }

    /// `estado` manda quando vem preenchido.
    ///
    /// 🔑 **É a leva escolhida antes dos arquivos**, como na tela da sessão do
    /// site: "sobe estas como levadas" é uma decisão sobre o lote, e não sobre
    /// cada foto. `None` cai na marcação da tecla `B` de cada uma — que é o que
    /// a publicação em lote e a classificação usam.
    async fn subir(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        id: &PhotoId,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
        nota: Option<u8>,
    ) -> Result<(String, EstadoNoBalcao), (String, String)> {
        let mut photo = match self.fotos.find_by_id(id).await {
            Ok(Some(p)) => p,
            Ok(None) => return Err((id.to_string(), "foto não está mais no catálogo".into())),
            Err(e) => return Err((id.to_string(), e.to_string())),
        };

        // 🚨 **`file_name()`, e não o caminho.** Desde a migration 022 o arquivo
        // no disco se chama `<uuid>.jpg`: tirar o nome do caminho entregaria ao
        // cliente uma galeria de UUIDs. `file_name()` devolve o nome de origem
        // quando ele existe, e cai no caminho quando não — que é o que mantém
        // de pé toda foto importada antes desta coluna.
        let nome = nome_para_o_site(photo.file_name().unwrap_or_default());
        let estado = estado.unwrap_or_else(|| EstadoNoBalcao::da_foto(&photo));

        // 🔑 **A de quem classificou vence a do banco.** O `photo` acima foi
        // lido enquanto a gravação da nota ainda corria noutra tarefa; sem este
        // `or_else` o que sobe é o valor anterior. Ver `enviar_uma`.
        //
        // ⚠️ **E não se recusa aqui o que o site recusa.** Uma guarda local
        // "só sobe de 1 a 5" pareceria melhora e derrubaria a publicação em
        // lote, que sobe foto do ensaio inteiro sem passar pela travessia do
        // zero. Quem decide o que o acervo aceita é o site.
        let nota = nota.or_else(|| photo.rating().map(|r| r.value()));

        let jpeg = self
            .exportador
            .renderizar_jpeg(&photo, &ExportOptions::default())
            .await
            .map_err(|e| (nome.clone(), e.to_string()))?;

        let enviada = self
            .api
            .enviar_foto(
                sessao,
                galeria_id,
                FotoParaEnviar {
                    nome: nome.clone(),
                    jpeg,
                    estado,
                    ordem,
                    // 🚨 **A nota vai junto do arquivo.** É ela que autoriza a
                    // foto a subir (passo 3), e o site recusa o envio sem ela —
                    // *"a foto sobe classificada: informe a nota de 1 a 5"*.
                    // Ela faltava, e com ela faltando **toda** classificação
                    // voltava `400`.
                    //
                    nota,
                    // 🔑 **A chave é o id da foto no catálogo local**, e não um
                    // valor novo por tentativa: é isso que faz o reenvio ser
                    // reconhecido em vez de virar uma segunda cópia na galeria
                    // do cliente. Ver `docs/11-OFFLINE-E-SINCRONIZACAO.md`.
                    chave_do_cliente: Some(id.to_string()),
                },
            )
            .await
            .map_err(|e| (nome.clone(), e.to_string()))?;

        // 🔑 **O id remoto volta para o catálogo.** Sem ele a publicação seria
        // um caminho de mão única: dava para subir e não para desfazer — nem
        // tirar a foto do storage quando a classificação é zerada, nem registrar
        // a negociação do balcão nela.
        //
        // ⚠️ **Falhar aqui não desfaz o envio**: a foto está no site, e dizer
        // que ela falhou faria o operador subir de novo, criando duplicata. O
        // preço de não gravar é perder o id — que se recupera relendo a galeria.
        photo.definir_id_no_site(Some(enviada.id));
        if let Err(erro) = self.fotos.update(&photo).await {
            eprintln!("⚠️ [Pós-venda] {nome} subiu, mas o id do site não foi gravado: {erro}");
        }

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
    use domain::services::pos_venda::{FotoEnviada, Galeria, NovaGaleria};
    use domain::value_objects::FilePath;
    use domain::DomainResult;
    use mockall::mock;
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

    mock! {
        pub ThumbnailGen {}

        #[async_trait::async_trait]
        impl ThumbnailGenerator for ThumbnailGen {
            async fn generate(&self, path: &FilePath, max_size: u32) -> DomainResult<Vec<u8>>;
            async fn generate_set(&self, path: &FilePath, max_sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>>;
        }
    }

    /// A API de mentira: registra o que subiu, com o estado de cada foto.
    #[derive(Default)]
    struct ApiDeMentira {
        enviadas: Mutex<Vec<(String, EstadoNoBalcao, u32)>>,
        /// 🔑 **O envio inteiro**, para o teste poder cobrar o que a tupla acima
        /// não carrega — a nota (sem ela o site devolve `400`) e a chave de
        /// idempotência. A tupla continua porque outros testes afirmam ordem e
        /// estado por ela, e trocá-la mudaria testes que não têm nada a ver.
        recebidas: Mutex<Vec<FotoParaEnviar>>,
        galerias: Mutex<Vec<NovaGaleria>>,
        /// Nomes que devem falhar ao subir.
        falham: Vec<String>,
        avisadas: Mutex<Vec<String>>,
        removidas: Mutex<Vec<String>>,
        /// Ids que o site responde `404` ao remover — alguém já os tirou de lá.
        some_do_site: Vec<String>,
        /// `(bilhete, tamanho do JPEG, ajustes)` de cada revelação salva.
        reveladas: Mutex<Vec<(String, usize, serde_json::Value)>>,
        /// As fotos que voltaram ao original — o "Zerar tudo" salvo.
        restauradas: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl PosVendaApi for ApiDeMentira {
        async fn autorizar_pelo_navegador(&self) -> DomainResult<Sessao> {
            unreachable!("o caso de uso não autoriza; recebe a sessão pronta")
        }
        async fn retomar_sessao(&self) -> DomainResult<Option<Sessao>> {
            Ok(None)
        }
        async fn sair(&self) {}
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
                .push((foto.nome.clone(), foto.estado, foto.ordem));
            self.recebidas.lock().unwrap().push(foto);
            Ok(FotoEnviada { id: "f".into() })
        }
        async fn avisar_fotos_prontas(&self, _: &Sessao, galeria_id: &str) -> DomainResult<()> {
            self.avisadas.lock().unwrap().push(galeria_id.to_string());
            Ok(())
        }

        // Os quatro do balcão não entram na publicação: ela cria a galeria e
        // sobe as fotos, e o que vem depois — negociar, remover, gerar link —
        // é gesto de outra tela. `unreachable!` em vez de `Ok(())` para que um
        // caso de uso que passe a usá-los acuse aqui, e não em produção.
        async fn galerias(
            &self,
            _: &Sessao,
        ) -> DomainResult<Vec<domain::services::pos_venda::GaleriaDoPainel>> {
            unreachable!("a publicação cria a galeria; não lista as que existem")
        }
        async fn mudar_foto(
            &self,
            _: &Sessao,
            _: &str,
            _: &domain::services::pos_venda::MudancaDaFoto,
        ) -> DomainResult<()> {
            unreachable!("a publicação sobe a foto já com o estado; não a muda depois")
        }
        async fn remover_foto(&self, _: &Sessao, id: &str) -> DomainResult<()> {
            self.removidas.lock().unwrap().push(id.to_string());
            if self.some_do_site.contains(&id.to_string()) {
                return Err(DomainError::NaoEncontradoNoSite("foto".into()));
            }
            Ok(())
        }
        async fn copia_de_trabalho(&self, _: &Sessao, _: &str) -> DomainResult<Vec<u8>> {
            unreachable!("a publicação sobe pixels; não os busca de volta")
        }
        async fn miniatura(&self, _: &Sessao, _: &str) -> DomainResult<Vec<u8>> {
            unreachable!("quem desenha a grade da sessão é a tela, não a publicação")
        }
        async fn abrir_galeria(
            &self,
            _: &Sessao,
            _: &str,
        ) -> DomainResult<domain::services::pos_venda::GaleriaAberta> {
            unreachable!("a publicação cria a galeria; não a abre")
        }
        async fn link_da_galeria(
            &self,
            _: &Sessao,
            _: &str,
        ) -> DomainResult<domain::services::pos_venda::LinkDeAcesso> {
            unreachable!("o link é pedido pela tela, depois de publicar")
        }
        async fn original(&self, _: &Sessao, _: &str) -> DomainResult<Vec<u8>> {
            unreachable!("quem baixa o original é a porta do app, que tem o motor de GPU")
        }
        async fn bilhete_de_revelacao(&self, _: &Sessao, foto_id: &str) -> DomainResult<String> {
            Ok(format!("bilhete-de-{foto_id}"))
        }
        async fn salvar_revelacao(
            &self,
            bilhete: &str,
            jpeg: Vec<u8>,
            ajustes: serde_json::Value,
        ) -> DomainResult<()> {
            self.reveladas
                .lock()
                .unwrap()
                .push((bilhete.to_string(), jpeg.len(), ajustes));
            Ok(())
        }
        async fn restaurar_original(&self, _: &Sessao, foto_id: &str) -> DomainResult<()> {
            self.restauradas.lock().unwrap().push(foto_id.to_string());
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

    /// 🔑 A tecla `B` é a única coisa que decide o estado no site.
    ///
    /// 📌 Este teste subia um lote inteiro por `execute`, que criava a galeria
    /// e mandava as fotos de uma vez. Aquele caminho saiu com o modal em
    /// 7/set/2026 — a web não cria galeria a partir de uma seleção de fotos —,
    /// e o que ele prendia continua valendo em `enviar_uma`: o estado sai da
    /// marcação da foto, e a ordem é a que o chamador pediu.
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
        // 🔑 O id que o site devolveu volta para o catálogo — é o que permite
        // desfazer depois (tirar do storage, registrar a negociação).
        repo.expect_update()
            .times(2)
            .withf(|foto| foto.id_no_site() == Some("f"))
            .returning(|_| Ok(()));
        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .withf(|_, opcoes| opcoes.watermark().is_none() && opcoes.longest_edge().is_none())
            .returning(|_, _| Ok(vec![1, 2, 3]));
        let api = Arc::new(ApiDeMentira::default());

        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );
        for (ordem, foto) in [&levada, &ficou].iter().enumerate() {
            caso.enviar_uma(&sessao(), "g1", &foto.id(), ordem as u32, None, None)
                .await
                .unwrap();
        }

        let enviadas = api.enviadas.lock().unwrap().clone();
        assert_eq!(
            enviadas,
            vec![
                ("DSC_001.jpg".to_string(), EstadoNoBalcao::LevadaNoBalcao, 0),
                ("DSC_002.jpg".to_string(), EstadoNoBalcao::Disponivel, 1),
            ]
        );
    }

    #[test]
    fn o_nome_no_site_e_o_da_origem_com_jpg() {
        assert_eq!(nome_para_o_site("/ensaio/DSC_001.NEF"), "DSC_001.jpg");
        assert_eq!(nome_para_o_site("C:\\fotos\\IMG.jpeg"), "IMG.jpg");
        assert_eq!(nome_para_o_site("semextensao"), "semextensao.jpg");
    }

    /// 🚨 Zerar a classificação tira a foto do storage **e** limpa o id local.
    ///
    /// É o passo 3 do fluxo do dono ao contrário: foi a classificação que
    /// autorizou a foto a subir, então tirar a nota é tirá-la de lá. O site
    /// recusa `nota: null` justamente porque foto do acervo sem nota não existe.
    #[tokio::test]
    async fn remover_do_site_tira_a_foto_e_esquece_o_id() {
        let mut photo = foto("/ensaio/a.NEF", false);
        photo.definir_id_no_site(Some("remota-1".into()));
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        // O id sai do catálogo: a foto voltou a ser só local.
        repo.expect_update()
            .times(1)
            .withf(|f| f.id_no_site().is_none())
            .returning(|_| Ok(()));

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(MockExportador::new()),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        caso.remover_do_site(&sessao(), &id).await.unwrap();
        assert_eq!(*api.removidas.lock().unwrap(), vec!["remota-1".to_string()]);
    }

    /// 🔑 O `404` do site é o desfecho desejado, e não uma falha.
    ///
    /// Um id que não existe mais lá significa que alguém já removeu a foto por
    /// outra tela. Tratar isso como erro faria o app **insistir** — tentando
    /// remover, a cada estrela apagada, algo que já saiu.
    #[tokio::test]
    async fn ja_removida_no_site_nao_e_erro_e_o_id_sai_daqui() {
        let mut photo = foto("/ensaio/b.NEF", false);
        photo.definir_id_no_site(Some("sumida".into()));
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update()
            .times(1)
            .withf(|f| f.id_no_site().is_none())
            .returning(|_| Ok(()));

        let api = Arc::new(ApiDeMentira {
            some_do_site: vec!["sumida".into()],
            ..Default::default()
        });
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(MockExportador::new()),
            Arc::new(MockThumbnailGen::new()),
            api,
        );

        caso.remover_do_site(&sessao(), &id)
            .await
            .expect("já não estar lá é o que se queria");
    }

    /// ⚠️ Tirar a nota de uma foto que nunca subiu não fala com o site.
    ///
    /// É o caso comum — a maioria das fotos de uma triagem nunca é classificada.
    /// Uma ida à rede por estrela apagada seria ruído puro.
    #[tokio::test]
    async fn foto_que_nunca_subiu_nao_gasta_uma_ida_a_rede() {
        let photo = foto("/ensaio/c.NEF", false);
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        // Nenhum `expect_update`: nada muda, então nada é gravado.

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(MockExportador::new()),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        caso.remover_do_site(&sessao(), &id).await.unwrap();
        assert!(api.removidas.lock().unwrap().is_empty());
    }

    /// 📸 **"Salvar na galeria e sair"**: o revelado entra no lugar do original.
    ///
    /// 🔑 O que este teste prende é que o caminho é o do site — bilhete emitido
    /// para *aquela* foto, e o JPEG por ele — e que **nada mais acontece**: nem
    /// galeria criada, nem catálogo lido, nem produto escolhido. Os dois mocks
    /// vazios são a afirmação: se o caso de uso passar a tocar o repositório ou
    /// o exportador, o `mockall` acusa aqui.
    #[tokio::test]
    async fn salvar_revelacao_sobe_o_jpeg_por_um_bilhete_daquela_foto() {
        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(MockPhotoRepo::new()),
            Arc::new(MockExportador::new()),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        let ajustes = serde_json::json!({ "exposure": 0.5, "corte_x": 0.1 });
        caso.salvar_revelacao(&sessao(), "foto-do-site", vec![9; 42], ajustes.clone())
            .await
            .unwrap();

        let reveladas = api.reveladas.lock().unwrap();
        assert_eq!(
            &*reveladas,
            &[("bilhete-de-foto-do-site".to_string(), 42, ajustes)],
            "o bilhete é o daquela foto, e os ajustes vão inteiros"
        );
    }

    /// 📸 **"Zerar tudo" salvo**: o bruto volta ao lugar, e nada sobe.
    ///
    /// 🔑 O que este teste prende é a ausência: nenhum bilhete emitido, nenhum
    /// JPEG enviado. O caminho pelo bilhete entregaria ao cliente uma geração a
    /// mais de perda no lugar do arquivo dele — que é o oposto de "voltar ao
    /// original".
    #[tokio::test]
    async fn restaurar_original_nao_pede_bilhete_nem_sobe_arquivo() {
        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(MockPhotoRepo::new()),
            Arc::new(MockExportador::new()),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        caso.restaurar_original(&sessao(), "foto-do-site")
            .await
            .unwrap();

        assert_eq!(&*api.restauradas.lock().unwrap(), &["foto-do-site"]);
        assert!(
            api.reveladas.lock().unwrap().is_empty(),
            "restaurar não sobe JPEG nenhum"
        );
    }

    /// 🚨 **O cliente não pode receber uma galeria de UUIDs.**
    ///
    /// Desde a migration 022 o arquivo no disco se chama `<uuid>.jpg`, e o nome
    /// que a câmera deu mora em `photos.nome_original`. Este é o ponto em que a
    /// diferença chega ao cliente: `subir` tirava o nome do **caminho**, e com o
    /// arquivo renomeado isso entregaria `0f8c….jpg` na galeria dele — no lugar
    /// de `DSC_2571.JPG`.
    ///
    /// ⚠️ **Não falha em lugar nenhum**: o envio dá certo, o id volta, a foto
    /// aparece. Só o nome é outro — e quem descobre é o cliente, olhando a
    /// galeria que ele pagou.
    #[tokio::test]
    async fn o_cliente_recebe_o_nome_da_camera_e_nao_o_uuid_do_disco() {
        let mut photo = foto(
            "/Ensaios/Teste - g1/0f8c1a2b-3c4d-5e6f-7a8b-9c0d1e2f3a4b.jpg",
            false,
        );
        photo.definir_nome_original(Some("DSC_2571.JPG".into()));
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update().returning(|_| Ok(()));

        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        let nome = caso
            .enviar_uma(&sessao(), "g1", &id, 0, None, None)
            .await
            .expect("subiu");
        assert_eq!(
            nome, "DSC_2571.jpg",
            "o cliente recebeu o nome do disco em vez do nome da câmera"
        );
    }

    /// ⚠️ **A foto de antes da migration 022 sobe com o nome do caminho** — que
    /// é o único que ela tem. É a mesma queda de `Photo::file_name()`, e sem ela
    /// toda foto do catálogo antigo subiria sem nome nenhum.
    #[tokio::test]
    async fn a_foto_sem_nome_guardado_sobe_com_o_nome_do_caminho() {
        let photo = foto("/Pictures/Catalog/2026/09/08/DSC_0001.NEF", false);
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update().returning(|_| Ok(()));

        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));

        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            Arc::new(ApiDeMentira::default()),
        );

        let nome = caso
            .enviar_uma(&sessao(), "g1", &id, 0, None, None)
            .await
            .expect("subiu");
        assert_eq!(nome, "DSC_0001.jpg");
    }

    /// 🚨 **A nota e a chave sobem junto do arquivo — as duas faltavam.**
    ///
    /// A **nota** é o que autoriza a foto a subir (passo 3), e o site recusa o
    /// envio sem ela: *"a foto sobe classificada: informe a nota de 1 a 5"*.
    /// `FotoParaEnviar` não tinha o campo, então **toda** classificação feita no
    /// app voltava `400` — e a mensagem falava de uma nota que o app tinha na
    /// mão e não mandava.
    ///
    /// A **chave** é o id da foto no catálogo, e é o que faz reenviar ser
    /// reconhecido em vez de virar uma segunda cópia na galeria de quem pagou.
    /// Ela precisa ser a mesma em toda tentativa — um valor novo por envio
    /// seria idempotência nenhuma.
    #[tokio::test]
    async fn a_nota_e_a_chave_sobem_junto_do_arquivo() {
        let mut photo = foto("/Ensaios/Teste - g1/uuid.jpg", false);
        photo.definir_nome_original(Some("DSC_2571.JPG".into()));
        photo
            .rate(domain::value_objects::Rating::new(4).unwrap())
            .unwrap();
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update().returning(|_| Ok(()));

        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        caso.enviar_uma(&sessao(), "g1", &id, 0, None, None)
            .await
            .expect("subiu");

        let recebidas = api.recebidas.lock().unwrap();
        let foto = recebidas.first().expect("uma foto subiu");
        assert_eq!(foto.nota, Some(4), "sem a nota o site devolve 400");
        assert_eq!(
            foto.chave_do_cliente.as_deref(),
            Some(id.to_string().as_str()),
            "a chave tem de ser o id do catálogo, estável entre tentativas"
        );
    }

    /// 🚨 **A nota de quem classificou vence a do banco — e o motivo é uma
    /// corrida.**
    ///
    /// Gravar a nota é uma tarefa do tokio que ninguém espera, e o envio lê a
    /// foto do repositório na mesma tecla. Quando a leitura chega primeiro, o
    /// que sobe é o valor **anterior**: o site devolve `400` — *"nota invalida:
    /// 0 (use de 1 a 5)"* — e a mensagem fala de uma nota que o app tinha na
    /// mão. Foi o que o dono viu ao classificar em 8/set/2026.
    ///
    /// Aqui o repositório devolve de propósito a foto **como ela era antes** da
    /// gravação; o que tem de subir é o 5 que veio pelo parâmetro.
    #[tokio::test]
    async fn a_nota_recem_dada_vence_a_que_o_banco_ainda_nao_gravou() {
        // O banco ainda tem o valor anterior: nota zero.
        let mut photo = foto("/Ensaios/Teste - g1/uuid.jpg", false);
        photo
            .rate(domain::value_objects::Rating::new(0).unwrap())
            .unwrap();
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update().returning(|_| Ok(()));

        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        caso.enviar_uma(&sessao(), "g1", &id, 0, None, Some(5))
            .await
            .expect("subiu");

        let recebidas = api.recebidas.lock().unwrap();
        assert_eq!(
            recebidas.first().expect("uma foto subiu").nota,
            Some(5),
            "subiu a nota velha do banco: é o 400 de 8/set"
        );
    }

    /// 📸 Classificar sobe a foto para a galeria que já está aberta.
    #[tokio::test]
    async fn enviar_uma_sobe_para_a_galeria_aberta_e_guarda_o_id() {
        let photo = foto("/ensaio/DSC_009.NEF", false);
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(photo.clone())));
        repo.expect_update()
            .times(1)
            .withf(|f| f.id_no_site() == Some("f"))
            .returning(|_| Ok(()));

        let mut exportador = MockExportador::new();
        exportador
            .expect_renderizar_jpeg()
            .returning(|_, _| Ok(vec![1]));

        let api = Arc::new(ApiDeMentira::default());
        let caso = PublicarNoPosVendaUseCase::new(
            Arc::new(repo),
            Arc::new(exportador),
            Arc::new(MockThumbnailGen::new()),
            api.clone(),
        );

        let nome = caso
            .enviar_uma(&sessao(), "g1", &id, 0, None, None)
            .await
            .expect("subiu");
        assert_eq!(nome, "DSC_009.jpg");
        // 🔑 Não cria galeria e não avisa o cliente: o aviso sai no fim do lote,
        // e não a cada estrela.
        assert!(api.galerias.lock().unwrap().is_empty());
        assert!(api.avisadas.lock().unwrap().is_empty());
    }

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            // Prazos folgados: o que estes testes exercem é a tela, não a
            // renovação — que tem teste próprio em `pos_venda/http.rs`.
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }
}
