//! A ponte entre a tela e o `PosVendaController` — a mesma forma das outras
//! portas: o controller é `async` do tokio, o GPUI não roda futuros dele, e o
//! `Handle` é capturado no `main` antes de `Application::run` tomar a thread.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::{PosVendaController, RecusaDoFimDaSessao};
use domain::services::pos_venda::{
    EstadoNoBalcao, Estudio, Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto,
    MudancaDaGaleria, NovaGaleria, Produto, Sessao,
};

/// 🔚 Os dois gestos do fim da sessão — os que só saem com o contato do cliente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestoDoFim {
    Link,
    Avisar,
}
use domain::value_objects::CropSettings;
use infrastructure::gpu_adjustments::Ajustes;
use infrastructure::ImageExporterImpl;

/// O que volta pelo canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recado {
    Entrou(Sessao),
    Produtos(Vec<Produto>),
    /// Os estúdios ativos — a escolha obrigatória ao abrir sessão.
    Estudios(Vec<Estudio>),
    /// O link que entra sem senha, pronto para ir ao cliente.
    Link(LinkDeAcesso),
    /// 🔚 O site recusou o link ou o aviso porque a sessão não tem e-mail
    /// (`422`) — sem contato nenhum, ou só com WhatsApp. **Não é falha**: a
    /// tela pede o e-mail e segue com o gesto (2026-09-13).
    FaltaEmail {
        gesto: GestoDoFim,
        frase: String,
    },
    /// Título ou contato gravados na sessão (`PATCH /galerias/{id}`).
    GaleriaAtualizada,
    /// O site recusou a edição. Separado de [`Recado::Falhou`] para a frase ir
    /// ao formulário, e não ao erro geral — onde uma falha de outro gesto no
    /// mesmo instante seria atribuída a ele.
    GaleriaNaoAtualizada(String),
    /// As sessões fotográficas que já existem.
    Galerias(Vec<GaleriaDoPainel>),
    /// Uma sessão recém-aberta, ainda sem foto nenhuma.
    Criada(Galeria),
    /// A sessão em que se entrou: a galeria e as fotos que estão nela.
    Aberta(Box<GaleriaAberta>),
    /// A capa de um estúdio, baixada do cadastro. Ela é **pública** — o R2
    /// serve `studios/` sem autenticação —, então o que viaja é a URL.
    CapaDoEstudio {
        estudio_id: String,
        bytes: std::sync::Arc<Vec<u8>>,
    },
    /// A miniatura de uma foto da sessão.
    Miniatura {
        foto_id: String,
        bytes: Vec<u8>,
    },
    /// O bruto de uma foto do site, como está no storage — para medir o lado
    /// dele e, no zoom, para a tela em resolução cheia.
    Original {
        foto_no_site: String,
        bytes: std::sync::Arc<Vec<u8>>,
    },
    /// O bruto não veio. Não é recusa de gesto: a tela fica na cópia.
    OriginalIndisponivel {
        foto_no_site: String,
    },
    /// "Baixar JPEG": a foto aberta revelada em resolução cheia, com o que
    /// está na tela — o `revelarIntegral` do site.
    JpegRevelado {
        foto_no_site: String,
        bytes: Vec<u8>,
    },
    /// Os pixels de uma foto do site — o passo 11.
    ///
    /// Leva o id da foto junto: um download que volta depois de a seta ter
    /// andado não pode pintar a foto errada, e quem confere isso é a tela.
    Pixels {
        foto_id: String,
        bytes: Vec<u8>,
    },
    /// O revelado entrou no lugar do original — "Salvar na galeria e sair".
    ///
    /// 🔑 **Leva o id da foto no site**, e não é enfeite: é ele que tira a
    /// receita do depósito local (`Gravador::esquecer_do_site`). A partir daqui
    /// a verdade daquela foto é o servidor, e guardar as duas abriria a
    /// pergunta de qual vale.
    RevelacaoSalva {
        foto_no_site: String,
    },
    /// **Uma foto classificada subiu** — o passo 3, com o nome de quem subiu.
    ///
    /// 🔑 **Por que não basta o [`Recado::Sincronizou`]**: ele é o recado de
    /// "algo mudou no catálogo", e sai também de gestos que nunca passaram pela
    /// esteira (negociar, tirar do site). A esteira precisa saber **qual**
    /// trabalho dela terminou — senão não sabe qual vaga abrir nem o que
    /// repetir (dono, 18/set/2026: *"essa rotina precisa ser um tanque de
    /// guerra!"*).
    ClassificadaSubiu {
        foto_id: String,
        /// A receita que subiu junto, em JSON — `None` é a foto no neutro.
        ///
        /// 🚨 **É a da leitura do começo da subida.** O ajuste feito enquanto a
        /// foto subia não está nela, e quem recebe o recado compara com o
        /// catálogo (`conciliar_o_que_subiu`).
        receita: Option<String>,
    },
    /// **Um envio de foto falhou, e com o nome de quem falhou.**
    ///
    /// 🚨 **A frase sozinha não dá para repetir.** Até 18/set/2026 toda recusa
    /// virava [`Recado::Falhou`], que leva só o texto: o lote seguia sem a foto,
    /// e o operador só descobria no canto dos envios — se olhasse. Com o alvo, a
    /// esteira tenta de novo e, se desistir, diz **qual arquivo** ficou para
    /// trás.
    EnvioFalhou {
        alvo: String,
        frase: String,
    },
    /// O passo 3 terminou para uma foto: ela subiu, ou saiu do storage.
    ///
    /// 🔑 **Notifica, não descreve.** Quem escuta só precisa saber que o
    /// catálogo mudou, para reler — os números estão no banco.
    Sincronizou,
    /// O `PATCH` de **uma** foto voltou — com a frase, se o site recusou.
    ///
    /// 🔑 **Por que não o `Sincronizou`**: ele não diz de quem é, e a tela da
    /// sessão manda as mudanças **foto a foto, em série só dentro da mesma
    /// foto**. Sem o id, a única série possível era a da tela inteira — e era
    /// ela que engolia a tecla seguinte enquanto a anterior não voltava, que é
    /// o que acontece o tempo todo com o ensaio subindo ao R2 e a rede cheia.
    Negociou {
        foto_id: String,
        erro: Option<String>,
    },
    /// Não há sessão guardada (ou ela venceu): a tela mostra o convite a
    /// autorizar. **Não é falha** — é o estado normal da primeira abertura, e
    /// tratá-lo como erro pintaria de vermelho um app recém-instalado.
    SemSessao,
    /// Uma frase para a tela — autorização recusada, rede caída, galeria
    /// recusada.
    Falhou(String),
    /// A resposta de [`Publicador::pedir_json`]. O `rotulo` é o que a tela
    /// escolheu para saber a qual pedido ela responde.
    Json {
        rotulo: &'static str,
        resultado: Result<serde_json::Value, String>,
    },
}

impl Recado {
    /// O [`Recado::Negociou`] para quem só **conta** respostas — o balcão e a
    /// conciliação da raiz: `Sincronizou` se passou, `Falhou` se não.
    pub fn sem_o_id(self) -> Recado {
        match self {
            Recado::Negociou { erro: None, .. } => Recado::Sincronizou,
            Recado::Negociou {
                erro: Some(erro), ..
            } => Recado::Falhou(erro),
            outro => outro,
        }
    }
}

/// Um pedido JSON à API, com o rótulo que volta na resposta.
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoJson {
    pub rotulo: &'static str,
    pub metodo: &'static str,
    pub caminho: String,
    pub corpo: Option<serde_json::Value>,
}

impl PedidoJson {
    pub fn ler(rotulo: &'static str, caminho: impl Into<String>) -> Self {
        Self {
            rotulo,
            metodo: "GET",
            caminho: caminho.into(),
            corpo: None,
        }
    }

    pub fn gravar(
        rotulo: &'static str,
        metodo: &'static str,
        caminho: impl Into<String>,
        corpo: serde_json::Value,
    ) -> Self {
        Self {
            rotulo,
            metodo,
            caminho: caminho.into(),
            corpo: Some(corpo),
        }
    }
}

/// O que subir sobre **uma** foto — os quatro campos que a descrevem.
///
/// 🔑 **Eles andam juntos porque descrevem a mesma coisa**, e não para encurtar
/// a assinatura: a foto, onde ela entra na sessão, em que leva está e que nota
/// levou. Soltos, viravam quatro posições seguidas em que trocar duas de lugar
/// compila — `ordem` e `nota` são ambos numéricos e opcionais o bastante para
/// isso passar batido numa revisão.
#[derive(Debug, Clone)]
pub struct FotoClassificada {
    pub foto_id: String,
    pub ordem: u32,
    /// A leva escolhida antes dos arquivos, na tela da sessão. `None` cai na
    /// marcação da tecla `B`, que é o caminho do passo 3 (classificar sobe).
    pub estado: Option<EstadoNoBalcao>,
    /// A nota **que acabou de ser dada**, e não a que o banco tem: a gravação
    /// dela é outra tarefa do tokio, e ninguém a espera — quem lê o banco aqui
    /// corre com ela e pode subir o valor anterior.
    pub nota: Option<u8>,
    /// A **faixa** escolhida na barra de envio, quando há uma. `None` segue a
    /// faixa da galeria — o que o site chama de "Padrão da galeria".
    pub produto_id: Option<String>,
}

/// Os pedidos de foto que a tela faz ao site, pelo que eles fazem lá.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedidoDeFoto {
    SubirClassificada,
    TirarDoSite,
    SalvarRevelacao,
    Negociar,
    EnviarArquivo,
    CopiaDeTrabalho,
    Miniatura,
    Original,
    RevelarIntegral,
}

/// Grava no site, ou só lê dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Natureza {
    /// Muda o que o cliente vê: conta em "Subindo", no canto dos envios e no
    /// G9 (fechar com envio pendente só esconde a janela).
    Envio,
    /// Só traz bytes para esta máquina. Fechar o app perde o download, e nada
    /// mais: a próxima abertura pede de novo.
    Leitura,
}

impl PedidoDeFoto {
    /// 🚨 **Baixar não é enviar.** Até 17/set/2026 a cópia de trabalho da
    /// Revelação entrava na mesma conta que salvar a revelação: a bandeja dizia
    /// "Subindo: 3 fotos" enquanto a tira baixava, e fechar a janela nesse
    /// instante a escondia em vez de sair (achado pelo estresse). No site,
    /// "Subindo" conta só a fila de envios.
    ///
    /// `SalvarRevelacao` baixa o original antes de subir o JPEG, e é envio
    /// assim mesmo: o que importa é o que ele deixa no site.
    pub const fn natureza(self) -> Natureza {
        match self {
            Self::SubirClassificada
            | Self::TirarDoSite
            | Self::SalvarRevelacao
            | Self::Negociar
            | Self::EnviarArquivo => Natureza::Envio,
            Self::CopiaDeTrabalho | Self::Miniatura | Self::Original | Self::RevelarIntegral => {
                Natureza::Leitura
            }
        }
    }
}

pub trait Publicador: Send + Sync + 'static {
    /// Abre o navegador para o operador autorizar este computador.
    ///
    /// Todas devolvem na hora; a resposta vem pelo canal — e esta demora o que o
    /// operador demorar na outra janela, que é justamente por isso que ela não
    /// pode bloquear a interface.
    fn autorizar(&self, canal: Sender<Recado>);

    /// A sessão de ontem, do chaveiro. Responde `Entrou` se ainda valer, e
    /// `SemSessao` se for preciso autorizar.
    fn retomar(&self, canal: Sender<Recado>);

    /// Um pedido JSON em nome da conta (`caminho` relativo a `/api/v2`), para
    /// as telas que só repassam JSON, como no site: a conta, o caixa e a
    /// retenção. Responde [`Recado::Json`] com o mesmo `rotulo`.
    fn pedir_json(&self, sessao: Sessao, pedido: PedidoJson, canal: Sender<Recado>) {
        let _ = sessao;
        let _ = canal.send(Recado::Json {
            rotulo: pedido.rotulo,
            resultado: Err("este publicador não atende pedido JSON".into()),
        });
    }

    /// Esquece a sessão — o "sair" da tela.
    fn sair(&self, canal: Sender<Recado>);
    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>);
    /// Os estúdios ativos. Responde `Estudios`.
    fn estudios(&self, sessao: Sessao, canal: Sender<Recado>);
    /// O passo 7 do fluxo: o link do cliente.
    ///
    /// ⚠️ **Pedido ao site, nunca montado aqui.** O endereço da galeria exige
    /// sessão e o cliente não tem conta — ele saiu do estúdio, não do site.
    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// Muda título, e-mail ou WhatsApp da sessão — o "Editar" e o pedido de
    /// contato do fim da sessão. Responde `GaleriaAtualizada` ou
    /// `GaleriaNaoAtualizada`.
    fn atualizar_galeria(
        &self,
        sessao: Sessao,
        galeria_id: String,
        mudanca: MudancaDaGaleria,
        canal: Sender<Recado>,
    );
    /// A lista de sessões fotográficas.
    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>);
    /// Abre uma sessão vazia — as fotos vêm depois.
    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>);
    /// Sobe uma foto para uma sessão que já existe. Ver [`FotoClassificada`].
    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto: FotoClassificada,
        canal: Sender<Recado>,
    );
    /// O passo 3 ao contrário: a classificação foi zerada, a foto sai do storage.
    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// ❌ Rejeita a foto que tem cópia neste catálogo (`foto_id` é o id
    /// **local**): ela sai da nuvem e fica aqui, marcada. Volta como
    /// `Sincronizou` ou `Falhou`.
    fn rejeitar_tirando_da_nuvem(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// Tira do site pelo id **de lá** — o último passo do resgate da foto que
    /// não tinha cópia aqui.
    fn remover_remoto(&self, sessao: Sessao, no_site: String, canal: Sender<Recado>);
    /// O passo 6: o que o cliente acertou no balcão, gravado na foto do site.
    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    );
    /// Sobe um arquivo do disco para a sessão — o envio da web.
    fn enviar_arquivo(
        &self,
        sessao: Sessao,
        galeria_id: String,
        caminho: String,
        ordem: u32,
        estado: EstadoNoBalcao,
        canal: Sender<Recado>,
    );
    /// Entrar numa sessão — o mesmo gesto que abre a rota `[id]` na web.
    fn abrir_galeria(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// A miniatura de uma foto da sessão, para a grade.
    fn miniatura(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// **Salvar na galeria**: o revelado entra no lugar do original.
    ///
    /// 🔑 É o botão do editor, e não uma publicação: a foto já é da galeria
    /// aberta, e nada aqui cria galeria nem pergunta pelo cliente.
    ///
    /// 🚨 **O original é baixado e revelado aqui, e não na tela.** A tela tem a
    /// cópia de trabalho de 2048 px — é dela que os sliders andam —, e subir
    /// aquilo entregaria ao cliente uma foto de 2048 px no lugar do original.
    /// É o mesmo caminho do `revelarIntegral` do site, e ele mora nesta porta
    /// porque é aqui que existe tokio para a rede e o motor de GPU para revelar,
    /// os dois fora da thread que desenha.
    fn salvar_revelacao(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    );
    /// Manda ao cliente o e-mail "suas fotos estão prontas".
    fn avisar(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// Baixa a capa de um estúdio pela URL do cadastro. Volta como
    /// [`Recado::CapaDoEstudio`].
    ///
    /// 🔑 **Sem sessão**: a foto do estúdio é pública (é a mesma que o site
    /// mostra no agendamento). O padrão não faz nada — quem não sabe baixar
    /// deixa a tela com a inicial do nome, que é o desenho de "cadastro sem
    /// foto".
    fn capa_do_estudio(&self, _estudio_id: String, _url: String, _canal: Sender<Recado>) {}

    /// Baixa o bruto de uma foto do site. Volta como [`Recado::Original`].
    fn original(&self, _sessao: Sessao, foto_no_site: String, canal: Sender<Recado>) {
        let _ = canal.send(Recado::OriginalIndisponivel { foto_no_site });
    }
    /// Baixa o original e o revela com a receita dada, **sem subir nada** —
    /// o "Baixar JPEG" do editor. Volta como [`Recado::JpegRevelado`].
    fn revelar_integral(
        &self,
        _sessao: Sessao,
        _foto_no_site: String,
        _ajustes: Ajustes,
        _corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        let _ = canal.send(Recado::Falhou(
            "este publicador não sabe revelar em resolução cheia".into(),
        ));
    }
    /// O passo 11: os pixels da foto que só existe no storage.
    ///
    /// `foto_local` é o id **do catálogo**, e volta no recado: é por ele que a
    /// tela confere se a foto na frente ainda é a mesma.
    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    );
}

pub struct PublicadorDaApi {
    controlador: Arc<PosVendaController>,
    /// O mesmo motor da exportação e da impressão — um só, e não um por gesto.
    /// Abrir um `Motor` custa um dispositivo wgpu; três caminhos com três
    /// dispositivos dariam três respostas possíveis para a mesma foto.
    exportador: Arc<ImageExporterImpl>,
    tokio: tokio::runtime::Handle,
}

impl PublicadorDaApi {
    pub fn novo(
        controlador: Arc<PosVendaController>,
        exportador: Arc<ImageExporterImpl>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            controlador,
            exportador,
            tokio,
        }
    }
}

/// A qualidade do JPEG que vai para a galeria.
///
/// 🔑 **92, o mesmo do editor do site** (`QUALIDADE` em `editor.tsx`). O que o
/// cliente baixa não pode depender de por qual das duas telas a foto passou.
const QUALIDADE: u8 = 92;

/// Baixa o original, revela com o que está na tela e sobe no lugar dele.
///
/// ⚠️ **Revelar é síncrono e come CPU e GPU**, então ele corre num
/// `spawn_blocking`: dentro de uma tarefa do tokio ele seguraria a thread do
/// executor, e com ela toda a rede do app — inclusive o upload que vem logo
/// depois.
async fn revelar_e_salvar(
    controlador: &PosVendaController,
    exportador: &Arc<ImageExporterImpl>,
    sessao: &Sessao,
    foto_no_site: &str,
    ajustes: Ajustes,
    corte: CropSettings,
) -> Result<(), String> {
    // 🔑 **Zerou tudo: o bruto volta ao lugar dele, e nada sobe.** Pelo caminho
    // de baixo isto seria baixar o original, revelá-lo com os ajustes neutros e
    // subir o resultado — entregando ao cliente uma geração a mais de JPEG no
    // lugar do arquivo que ele deveria receber. É o mesmo atalho que o editor
    // do site faz (`semRevelacao` em `editor.tsx`); o gesto é o mesmo nos dois,
    // e o resultado tem de ser também.
    if ajustes == Ajustes::default() && corte == CropSettings::default() {
        return controlador.restaurar_original(sessao, foto_no_site).await;
    }

    let original = controlador.original(sessao, foto_no_site).await?;

    let exportador = exportador.clone();
    let para_revelar = corte.clone();
    let jpeg = tokio::task::spawn_blocking(move || {
        exportador.renderizar_bytes(&original, &ajustes, &para_revelar, QUALIDADE)
    })
    .await
    .map_err(|e| format!("a revelação não terminou: {e}"))?
    .map_err(|e| e.to_string())?;

    controlador
        .salvar_revelacao(
            sessao,
            foto_no_site,
            jpeg,
            ajustes_em_json(&ajustes, &corte),
        )
        .await
}

/// A receita como o site a grava — mora no `infrastructure`, junto com a
/// compressão com que ela sobe. Ver `infrastructure::pos_venda::receita`.
pub(crate) use infrastructure::pos_venda::receita::ajustes_em_json;

impl Publicador for PublicadorDaApi {
    fn autorizar(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.autorizar().await {
                Ok(sessao) => Recado::Entrou(sessao),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn retomar(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.retomar().await {
                Ok(Some(sessao)) => Recado::Entrou(sessao),
                Ok(None) => Recado::SemSessao,
                // Falha ao ler o chaveiro não é motivo para assustar ninguém: o
                // desfecho é o mesmo de não haver sessão — autorizar.
                Err(_) => Recado::SemSessao,
            };
            let _ = canal.send(recado);
        });
    }

    fn sair(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            controlador.sair().await;
            let _ = canal.send(Recado::SemSessao);
        });
    }

    fn pedir_json(&self, sessao: Sessao, pedido: PedidoJson, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let resultado = controlador
                .pedir_json(&sessao, pedido.metodo, &pedido.caminho, pedido.corpo)
                .await;
            let _ = canal.send(Recado::Json {
                rotulo: pedido.rotulo,
                resultado,
            });
        });
    }

    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.produtos(&sessao).await {
                Ok(produtos) => Recado::Produtos(produtos),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn estudios(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.estudios(&sessao).await {
                Ok(estudios) => Recado::Estudios(estudios),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn salvar_revelacao(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        let exportador = self.exportador.clone();
        self.tokio.spawn(async move {
            let recado = match revelar_e_salvar(
                &controlador,
                &exportador,
                &sessao,
                &foto_no_site,
                ajustes,
                corte,
            )
            .await
            {
                Ok(()) => Recado::RevelacaoSalva {
                    foto_no_site: foto_no_site.clone(),
                },
                Err(frase) => Recado::EnvioFalhou {
                    alvo: foto_no_site.clone(),
                    frase,
                },
            };
            let _ = canal.send(recado);
        });
    }

    fn original(&self, sessao: Sessao, foto_no_site: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.original(&sessao, &foto_no_site).await {
                Ok(bytes) => Recado::Original {
                    foto_no_site,
                    bytes: std::sync::Arc::new(bytes),
                },
                Err(_) => Recado::OriginalIndisponivel { foto_no_site },
            };
            let _ = canal.send(recado);
        });
    }

    fn revelar_integral(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        let exportador = self.exportador.clone();
        self.tokio.spawn(async move {
            let revelado = async {
                let original = controlador.original(&sessao, &foto_no_site).await?;
                tokio::task::spawn_blocking(move || {
                    exportador.renderizar_bytes(&original, &ajustes, &corte, QUALIDADE)
                })
                .await
                .map_err(|e| format!("a revelação não terminou: {e}"))?
                .map_err(|e| e.to_string())
            };
            let recado = match revelado.await {
                Ok(bytes) => Recado::JpegRevelado {
                    foto_no_site,
                    bytes,
                },
                Err(erro) => Recado::Falhou(format!("Não foi possível gerar o JPEG: {erro}")),
            };
            let _ = canal.send(recado);
        });
    }

    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.link_da_galeria(&sessao, &galeria_id).await {
                Ok(link) => Recado::Link(link),
                Err(RecusaDoFimDaSessao::FaltaEmail(frase)) => Recado::FaltaEmail {
                    gesto: GestoDoFim::Link,
                    frase,
                },
                Err(RecusaDoFimDaSessao::Outra(erro)) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn atualizar_galeria(
        &self,
        sessao: Sessao,
        galeria_id: String,
        mudanca: MudancaDaGaleria,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .atualizar_galeria(&sessao, &galeria_id, &mudanca)
                .await
            {
                Ok(()) => Recado::GaleriaAtualizada,
                Err(erro) => Recado::GaleriaNaoAtualizada(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.galerias(&sessao).await {
                Ok(lista) => Recado::Galerias(lista),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.criar_galeria(&sessao, &nova).await {
                Ok(galeria) => Recado::Criada(galeria),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto: FotoClassificada,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let FotoClassificada {
                foto_id,
                ordem,
                estado,
                nota,
                produto_id,
            } = foto;
            let recado = match controlador
                .enviar_uma(
                    &sessao,
                    &galeria_id,
                    &foto_id,
                    ordem,
                    estado,
                    nota,
                    produto_id,
                )
                .await
            {
                Ok(subida) => Recado::ClassificadaSubiu {
                    foto_id: foto_id.clone(),
                    receita: subida.receita.map(|r| r.to_string()),
                },
                Err(frase) => Recado::EnvioFalhou {
                    alvo: foto_id.clone(),
                    frase,
                },
            };
            let _ = canal.send(recado);
        });
    }

    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.remover_do_site(&sessao, &foto_id).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn rejeitar_tirando_da_nuvem(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .rejeitar_tirando_da_nuvem(&sessao, &foto_id)
                .await
            {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn remover_remoto(&self, sessao: Sessao, no_site: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.remover_remoto(&sessao, &no_site).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let erro = controlador
                .mudar_foto(&sessao, &foto_id, &mudanca)
                .await
                .err();
            let recado = Recado::Negociou { foto_id, erro };
            let _ = canal.send(recado);
        });
    }

    fn enviar_arquivo(
        &self,
        sessao: Sessao,
        galeria_id: String,
        caminho: String,
        ordem: u32,
        estado: EstadoNoBalcao,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .enviar_arquivo(&sessao, &galeria_id, &caminho, ordem, estado)
                .await
            {
                Ok(_) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn abrir_galeria(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.abrir_galeria(&sessao, &galeria_id).await {
                Ok(aberta) => Recado::Aberta(Box::new(aberta)),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn miniatura(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.miniatura(&sessao, &foto_id).await {
                Ok(bytes) => Recado::Miniatura { foto_id, bytes },
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn capa_do_estudio(&self, estudio_id: String, url: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            // ⚠️ **Capa que não vem é capa que não aparece**, e a tela já sabe
            // desenhar o estúdio sem ela — nada de recado de falha para um
            // enfeite.
            if let Ok(bytes) = controlador.arquivo_publico(&url).await {
                let _ = canal.send(Recado::CapaDoEstudio {
                    estudio_id,
                    bytes: std::sync::Arc::new(bytes),
                });
            }
        });
    }

    fn avisar(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.avisar(&sessao, &galeria_id).await {
                Ok(()) => Recado::Sincronizou,
                Err(RecusaDoFimDaSessao::FaltaEmail(frase)) => Recado::FaltaEmail {
                    gesto: GestoDoFim::Avisar,
                    frase,
                },
                Err(RecusaDoFimDaSessao::Outra(erro)) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.copia_de_trabalho(&sessao, &foto_no_site).await {
                Ok(bytes) => Recado::Pixels {
                    foto_id: foto_local,
                    bytes,
                },
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }
}

#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Entra com qualquer senha, lista os produtos que recebeu, e "publica"
    /// respondendo o andamento inteiro na hora.
    #[derive(Default)]
    pub struct PublicadorDeMentira {
        pub produtos: Vec<Produto>,
        /// Os estúdios que a lista vai encontrar.
        pub estudios: Vec<Estudio>,
        /// Liga a recusa do site — para a tela poder ser testada com "não
        /// autorizado". Invertido de propósito: o `Default` do teste é o caminho
        /// que dá certo.
        pub recusa_autorizacao: bool,
        /// Quantas vezes o navegador foi aberto.
        pub autorizacoes: Mutex<Vec<()>>,
        /// O que o chaveiro devolve na retomada.
        pub sessao_guardada: Mutex<Option<Sessao>>,
        /// De quais galerias o link foi pedido.
        pub links: Mutex<Vec<String>>,
        /// As sessões que a listagem vai encontrar.
        pub galerias: Mutex<Vec<GaleriaDoPainel>>,
        /// As que foram abertas por esta tela.
        pub criadas: Mutex<Vec<NovaGaleria>>,
        /// `(galeria, foto, ordem)` de cada classificada que subiu.
        pub subidas: Mutex<Vec<(String, String, u32)>>,
        /// O que o servidor de verdade faz ao receber a foto, antes de
        /// responder: ela entra na galeria e ganha id remoto no catálogo.
        ///
        /// 🚨 **Sem isto a subida não tinha consequência**, e a grade da sessão
        /// que esvaziava a cada foto enviada passou por todos os cenários: a
        /// local saía das locais e a do site nunca entrava (21/set/2026).
        /// O que o catálogo sofre quando a rejeição tira a foto da nuvem — a
        /// marca e o id remoto, como o use case de verdade.
        #[allow(clippy::type_complexity)]
        pub ao_rejeitar: Mutex<Option<Box<dyn Fn(&PublicadorDeMentira, &str) + Send>>>,
        /// Os ids **locais** das rejeitadas que saíram da nuvem.
        pub rejeitadas_na_nuvem: Mutex<Vec<String>>,
        #[allow(clippy::type_complexity)]
        pub ao_subir: Mutex<Option<Box<dyn Fn(&PublicadorDeMentira, &str, u32) + Send>>>,
        /// A receita que o catálogo tem da foto **no instante em que a subida
        /// a lê** — é ela que vai junto, como no `subir` de verdade. Sem gancho,
        /// a foto sobe no neutro.
        #[allow(clippy::type_complexity)]
        pub receita_do_catalogo: Mutex<Option<Box<dyn Fn(&str) -> Option<String> + Send>>>,
        /// As fotos tiradas do storage.
        pub tiradas: Mutex<Vec<String>>,
        /// O que foi negociado, por foto.
        pub negociadas: Mutex<Vec<(String, MudancaDaFoto)>>,
        /// Os ids no site cujos pixels foram pedidos.
        pub baixadas: Mutex<Vec<String>>,
        /// As **miniaturas** pedidas, na ordem — uma por foto, por chamada. É
        /// por elas que o estresse sabe se a grade está sendo re-baixada
        /// inteira a cada releitura, ou só a foto que mudou.
        pub miniaturas_pedidas: Mutex<Vec<String>>,
        /// As sessões em que se entrou.
        pub abertas: Mutex<Vec<String>>,
        /// As galerias cujo cliente foi avisado.
        pub avisadas: Mutex<Vec<String>>,
        /// O estado pedido em cada subida — `None` é "o da tecla B".
        pub estados_pedidos: Mutex<Vec<Option<EstadoNoBalcao>>>,
        /// A faixa pedida em cada subida — `None` é "a da galeria".
        pub faixas_pedidas: Mutex<Vec<Option<String>>>,
        /// A nota que acompanhou cada subida — é o que prende a corrida entre a
        /// gravação da nota e o envio.
        pub notas_pedidas: Mutex<Vec<Option<u8>>>,
        /// O que a sessão aberta vai mostrar.
        pub fotos_da_sessao: Mutex<Vec<domain::services::pos_venda::FotoDaGaleria>>,
        /// `(galeria, caminho, ordem, estado)` de cada arquivo do disco enviado.
        pub arquivos_enviados: Mutex<Vec<(String, String, u32, EstadoNoBalcao)>>,
        /// `(foto no site, ajustes, corte)` de cada revelação salva.
        pub reveladas: Mutex<Vec<(String, Ajustes, CropSettings)>>,
        /// Segura as respostas em vez de mandá-las na hora — o que a rede faz.
        ///
        /// 🚨 **O `Default` responde no mesmo instante, e isso escondia um
        /// defeito**: a colheita da tela podia desistir antes da resposta e
        /// nenhum teste reclamava, porque a mentira nunca chegava atrasada. Com
        /// isto ligado, o recado só sai em [`PublicadorDeMentira::responder`].
        pub demorada: bool,
        /// Os recados presos, esperando `responder()`.
        pub guardados: Mutex<Vec<(Sender<Recado>, Recado)>>,
        /// `(galeria, mudança)` de cada `PATCH` dos dados do cliente.
        pub atualizacoes: Mutex<Vec<(String, MudancaDaGaleria)>>,
        /// Os pedidos JSON feitos, na ordem.
        pub pedidos_json: Mutex<Vec<PedidoJson>>,
        /// A resposta de cada rótulo; sem resposta, o pedido falha com "sem rede".
        pub respostas_json:
            Mutex<std::collections::HashMap<&'static str, Result<serde_json::Value, String>>>,
        /// O bruto que `original` devolve — `None` é "indisponível", como o
        /// padrão da porta.
        pub bruto: Mutex<Option<Vec<u8>>>,
        /// Os ids no site cujo bruto foi pedido.
        pub originais: Mutex<Vec<String>>,
        /// Os estúdios cuja capa foi pedida.
        pub capas_pedidas: Mutex<Vec<String>>,
        /// `(foto no site, ajustes, corte)` de cada "Baixar JPEG".
        pub integrais: Mutex<Vec<(String, Ajustes, CropSettings)>>,
        /// Liga a recusa do site ao "Salvar na galeria", com esta frase.
        pub salvar_falha: Option<String>,
        /// **Quantas vezes cada foto ainda vai falhar** antes de passar — a
        /// rede ruim do balcão, reproduzida. O envio decrementa a conta: `2`
        /// falha duas vezes e sobe na terceira; `u8::MAX` nunca sobe.
        pub falhas_por_foto: Mutex<std::collections::HashMap<String, u8>>,
        /// Segura também a cópia de trabalho (o passo 11) até `responder()` —
        /// para provar que um download no ar não conta como envio.
        pub copia_demorada: bool,
        /// Segura a resposta do `PATCH` até `responder()` — a rede cheia com o
        /// ensaio subindo ao R2, que é quando a tecla seguinte chega antes.
        pub negociacao_demorada: bool,
        /// Com isto, a cópia de trabalho falha em vez de chegar.
        pub copia_falha: Option<String>,
    }

    /// Um JPEG 1×1 cinza, codificado de verdade.
    fn jpeg_de_um_pixel() -> Vec<u8> {
        let mut bytes = Vec::new();
        let imagem = image::RgbImage::from_pixel(1, 1, image::Rgb([128, 128, 128]));
        image::DynamicImage::ImageRgb8(imagem)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("codificar um pixel");
        bytes
    }

    impl PublicadorDeMentira {
        pub fn responder_json(
            &self,
            rotulo: &'static str,
            resposta: Result<serde_json::Value, String>,
        ) {
            self.respostas_json
                .lock()
                .expect("as respostas")
                .insert(rotulo, resposta);
        }

        pub fn pedidos_json(&self) -> Vec<PedidoJson> {
            self.pedidos_json.lock().expect("os pedidos").clone()
        }

        pub fn links(&self) -> Vec<String> {
            self.links.lock().expect("os links").clone()
        }

        pub fn criadas(&self) -> Vec<NovaGaleria> {
            self.criadas.lock().expect("as criadas").clone()
        }

        pub fn subidas(&self) -> Vec<(String, String, u32)> {
            self.subidas.lock().expect("as subidas").clone()
        }

        pub fn faixas_pedidas(&self) -> Vec<Option<String>> {
            self.faixas_pedidas.lock().expect("as faixas").clone()
        }

        pub fn tiradas(&self) -> Vec<String> {
            self.tiradas.lock().expect("as tiradas").clone()
        }

        pub fn negociadas(&self) -> Vec<(String, MudancaDaFoto)> {
            self.negociadas.lock().expect("as negociadas").clone()
        }

        pub fn baixadas(&self) -> Vec<String> {
            self.baixadas.lock().expect("as baixadas").clone()
        }

        /// Esta foto falha **desta vez**? Gasta uma das falhas programadas em
        /// `falhas_por_foto` e devolve a frase que o site devolveria.
        fn vai_falhar(&self, alvo: &str) -> Option<String> {
            let mut falhas = self.falhas_por_foto.lock().expect("as falhas");
            let restam = falhas.get_mut(alvo)?;
            if *restam == 0 {
                return None;
            }
            // `u8::MAX` é "falha sempre": não decrementa, e a esteira desiste.
            if *restam != u8::MAX {
                *restam -= 1;
            }
            Some(format!(
                "erro de infraestrutura: o site não respondeu ({alvo})"
            ))
        }

        pub fn miniaturas_pedidas(&self) -> Vec<String> {
            self.miniaturas_pedidas
                .lock()
                .expect("as miniaturas pedidas")
                .clone()
        }

        pub fn abertas(&self) -> Vec<String> {
            self.abertas.lock().expect("as abertas").clone()
        }

        pub fn arquivos_enviados(&self) -> Vec<(String, String, u32, EstadoNoBalcao)> {
            self.arquivos_enviados.lock().expect("os arquivos").clone()
        }

        pub fn estados_pedidos(&self) -> Vec<Option<EstadoNoBalcao>> {
            self.estados_pedidos.lock().expect("os estados").clone()
        }

        pub fn notas_pedidas(&self) -> Vec<Option<u8>> {
            self.notas_pedidas.lock().expect("as notas").clone()
        }

        pub fn reveladas(&self) -> Vec<(String, Ajustes, CropSettings)> {
            self.reveladas.lock().expect("as reveladas").clone()
        }

        /// O bruto que o site devolve quando alguém pede o original — sem
        /// ele, a resposta é `OriginalIndisponivel`, que é o caso da foto cujo
        /// arquivo já não está no storage.
        pub fn definir_bruto(&mut self, bytes: Vec<u8>) {
            *self.bruto.lock().expect("o bruto") = Some(bytes);
        }

        pub fn originais(&self) -> Vec<String> {
            self.originais.lock().expect("os originais").clone()
        }

        pub fn integrais(&self) -> Vec<(String, Ajustes, CropSettings)> {
            self.integrais.lock().expect("os integrais").clone()
        }

        pub fn avisadas(&self) -> Vec<String> {
            self.avisadas.lock().expect("as avisadas").clone()
        }

        pub fn atualizacoes(&self) -> Vec<(String, MudancaDaGaleria)> {
            self.atualizacoes.lock().expect("as atualizacoes").clone()
        }

        /// Um JPEG de verdade, `lado`×`lado`, para o bruto de `original`.
        pub fn jpeg(lado: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            let imagem = image::RgbImage::from_pixel(lado, lado, image::Rgb([90, 90, 90]));
            image::DynamicImage::ImageRgb8(imagem)
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Jpeg,
                )
                .expect("codificar o bruto");
            bytes
        }

        /// Manda **uma** das guardadas, a mais antiga.
        ///
        /// 🔑 Existe para provar o que `responder` não prova: num lote, as
        /// respostas chegam **espalhadas no tempo**, uma por foto. Soltar todas
        /// de uma vez esconderia exatamente o defeito de quem para de escutar
        /// na primeira.
        pub fn responder_uma(&self) {
            let mut guardados = self.guardados.lock().expect("os guardados");
            if guardados.is_empty() {
                return;
            }
            let (canal, recado) = guardados.remove(0);
            let _ = canal.send(recado);
        }

        /// Manda a resposta guardada de **uma revelação**, a mais antiga.
        ///
        /// 🔑 **`responder_uma` não serve para contar o lote.** Com `demorada`,
        /// tudo o que passa por `responder_ou_guardar` entra na mesma fila —
        /// inclusive os `pedir_json` da galeria viva, que chegam sozinhos. Tirar
        /// o primeiro da fila solta qualquer um deles, e o teste que quer ver o
        /// botão andar de "Salvando 1/2…" para "Salvando 2/2…" mediria outra
        /// coisa.
        pub fn responder_uma_revelacao(&self) {
            let mut guardados = self.guardados.lock().expect("os guardados");
            let Some(posicao) = guardados
                .iter()
                .position(|(_, recado)| matches!(recado, Recado::RevelacaoSalva { .. }))
            else {
                return;
            };
            let (canal, recado) = guardados.remove(posicao);
            let _ = canal.send(recado);
        }

        /// Manda o que `demorada` segurou — a rede respondendo, enfim.
        pub fn responder(&self) {
            for (canal, recado) in self.guardados.lock().expect("os guardados").drain(..) {
                let _ = canal.send(recado);
            }
        }

        /// 🔚 A galeria existe e não tem e-mail — quando o site responde `422`
        /// ao link e ao aviso, com WhatsApp ou sem.
        fn sem_email(&self, galeria_id: &str) -> bool {
            self.galerias
                .lock()
                .expect("as galerias")
                .iter()
                .any(|g| g.id == galeria_id && g.email.is_none())
        }

        fn falta_email(gesto: GestoDoFim) -> Recado {
            Recado::FaltaEmail {
                gesto,
                frase: "informe o e-mail do cliente antes de gerar o link".into(),
            }
        }

        fn responder_ou_guardar(&self, canal: Sender<Recado>, recado: Recado) {
            if self.demorada {
                self.guardados
                    .lock()
                    .expect("os guardados")
                    .push((canal, recado));
            } else {
                let _ = canal.send(recado);
            }
        }
    }

    impl Publicador for PublicadorDeMentira {
        fn pedir_json(&self, _sessao: Sessao, pedido: PedidoJson, canal: Sender<Recado>) {
            let resultado = self
                .respostas_json
                .lock()
                .expect("as respostas")
                .get(pedido.rotulo)
                .cloned()
                .unwrap_or_else(|| Err("sem rede".into()));
            let rotulo = pedido.rotulo;
            self.pedidos_json.lock().expect("os pedidos").push(pedido);
            let recado = Recado::Json { rotulo, resultado };
            self.responder_ou_guardar(canal, recado);
        }

        fn autorizar(&self, canal: Sender<Recado>) {
            self.autorizacoes.lock().expect("as autorizacoes").push(());
            // 🔑 **Respeita o `demorada`.** A autorização é a única espera que o
            // operador *vê* — são os minutos dele no navegador —, e sem isto
            // nenhum teste conseguia parar a tela nesse estado para afirmar o
            // que ela oferece ali.
            let recado = if !self.recusa_autorizacao {
                Recado::Entrou(Sessao {
                    access_token: "tok-de-mentira".into(),
                    refresh_token: "ref-de-mentira".into(),
                    access_vence_em: i64::MAX,
                    refresh_vence_em: i64::MAX,
                })
            } else {
                Recado::Falhou("o site recusou a autorização".into())
            };
            self.responder_ou_guardar(canal, recado);
        }

        fn retomar(&self, canal: Sender<Recado>) {
            let _ = canal.send(
                match self.sessao_guardada.lock().expect("a guardada").clone() {
                    Some(sessao) => Recado::Entrou(sessao),
                    None => Recado::SemSessao,
                },
            );
        }

        fn sair(&self, canal: Sender<Recado>) {
            *self.sessao_guardada.lock().expect("a guardada") = None;
            let _ = canal.send(Recado::SemSessao);
        }

        fn produtos(&self, _sessao: Sessao, canal: Sender<Recado>) {
            self.responder_ou_guardar(canal, Recado::Produtos(self.produtos.clone()));
        }

        fn estudios(&self, _sessao: Sessao, canal: Sender<Recado>) {
            self.responder_ou_guardar(canal, Recado::Estudios(self.estudios.clone()));
        }

        fn salvar_revelacao(
            &self,
            _sessao: Sessao,
            foto_no_site: String,
            ajustes: Ajustes,
            corte: CropSettings,
            canal: Sender<Recado>,
        ) {
            self.reveladas.lock().expect("as reveladas").push((
                foto_no_site.clone(),
                ajustes,
                corte.clone(),
            ));
            let recado = match self
                .salvar_falha
                .clone()
                .or_else(|| self.vai_falhar(&foto_no_site))
            {
                Some(frase) => Recado::EnvioFalhou {
                    alvo: foto_no_site,
                    frase,
                },
                None => {
                    // 🔑 **A receita nova fica na linha da foto**, como no
                    // servidor: é dela que a galeria relida reabre a foto
                    // depois que o depósito daqui se esvazia.
                    if let Some(foto) = self
                        .fotos_da_sessao
                        .lock()
                        .expect("as fotos")
                        .iter_mut()
                        .find(|f| f.id == foto_no_site)
                    {
                        foto.ajustes = Some(ajustes_em_json(&ajustes, &corte));
                        foto.revelada = true;
                    }
                    Recado::RevelacaoSalva { foto_no_site }
                }
            };
            self.responder_ou_guardar(canal, recado);
        }

        fn original(&self, _sessao: Sessao, foto_no_site: String, canal: Sender<Recado>) {
            self.originais
                .lock()
                .expect("os originais")
                .push(foto_no_site.clone());
            let recado = match self.bruto.lock().expect("o bruto").as_ref() {
                Some(bytes) => Recado::Original {
                    foto_no_site,
                    bytes: Arc::new(bytes.clone()),
                },
                None => Recado::OriginalIndisponivel { foto_no_site },
            };
            self.responder_ou_guardar(canal, recado);
        }

        /// Um JPEG de um pixel no lugar da revelação em resolução cheia — o
        /// que importa aqui é **o que foi pedido**, e que a resposta chega.
        fn revelar_integral(
            &self,
            _sessao: Sessao,
            foto_no_site: String,
            ajustes: Ajustes,
            corte: CropSettings,
            canal: Sender<Recado>,
        ) {
            self.integrais.lock().expect("os integrais").push((
                foto_no_site.clone(),
                ajustes,
                corte,
            ));
            self.responder_ou_guardar(
                canal,
                Recado::JpegRevelado {
                    foto_no_site,
                    bytes: jpeg_de_um_pixel(),
                },
            );
        }

        fn link(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            // 🔚 Como o site: sem contato não há link, e nada se registra.
            if self.sem_email(&galeria_id) {
                self.responder_ou_guardar(canal, Self::falta_email(GestoDoFim::Link));
                return;
            }
            self.links
                .lock()
                .expect("os links")
                .push(galeria_id.clone());
            self.responder_ou_guardar(
                canal,
                Recado::Link(LinkDeAcesso {
                    url: format!("https://recordarfotos.com.br/entrar?t={galeria_id}"),
                    // Como o site responde desde 2026-09-20: sem prazo.
                    validade_em_segundos: None,
                }),
            );
        }

        fn subir_classificada(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            foto: FotoClassificada,
            canal: Sender<Recado>,
        ) {
            let foto_id = foto.foto_id.clone();
            let ordem = foto.ordem;
            self.subidas
                .lock()
                .expect("as subidas")
                .push((galeria_id, foto.foto_id, foto.ordem));
            self.estados_pedidos
                .lock()
                .expect("os estados")
                .push(foto.estado);
            self.notas_pedidas.lock().expect("as notas").push(foto.nota);
            self.faixas_pedidas
                .lock()
                .expect("as faixas")
                .push(foto.produto_id);
            let recado = match self.vai_falhar(&foto_id) {
                Some(frase) => Recado::EnvioFalhou {
                    alvo: foto_id,
                    frase,
                },
                None => {
                    let receita = self
                        .receita_do_catalogo
                        .lock()
                        .expect("o gancho")
                        .as_ref()
                        .and_then(|ler| ler(&foto_id));
                    if let Some(consequencia) = self.ao_subir.lock().expect("o gancho").as_ref() {
                        consequencia(self, &foto_id, ordem);
                    }
                    Recado::ClassificadaSubiu { foto_id, receita }
                }
            };
            self.responder_ou_guardar(canal, recado);
        }

        fn tirar_do_site(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
            self.tiradas.lock().expect("as tiradas").push(foto_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn rejeitar_tirando_da_nuvem(
            &self,
            _sessao: Sessao,
            foto_id: String,
            canal: Sender<Recado>,
        ) {
            if let Some(consequencia) = self.ao_rejeitar.lock().expect("o gancho").as_ref() {
                consequencia(self, &foto_id);
            }
            self.rejeitadas_na_nuvem
                .lock()
                .expect("as rejeitadas")
                .push(foto_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn remover_remoto(&self, _sessao: Sessao, no_site: String, canal: Sender<Recado>) {
            self.tiradas.lock().expect("as tiradas").push(no_site);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn enviar_arquivo(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            caminho: String,
            ordem: u32,
            estado: EstadoNoBalcao,
            canal: Sender<Recado>,
        ) {
            self.arquivos_enviados
                .lock()
                .expect("os arquivos")
                .push((galeria_id, caminho, ordem, estado));
            // 🔑 **Respeita o `demorada`** — desde 8/set/2026. Uma importação
            // que responde no mesmo instante em que é pedida não tem "meio", e
            // é justamente no meio dela que o operador precisa continuar
            // classificando e negociando. Sem isto não há como afirmar sobre
            // esse estado: o lote nasce e morre dentro da mesma linha do teste.
            self.responder_ou_guardar(canal, Recado::Sincronizou);
        }

        fn abrir_galeria(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            self.abertas
                .lock()
                .expect("as abertas")
                .push(galeria_id.clone());
            let galeria = self
                .galerias
                .lock()
                .expect("as galerias")
                .iter()
                .find(|g| g.id == galeria_id)
                .cloned();
            let Some(galeria) = galeria else {
                let _ = canal.send(Recado::Falhou("essa sessão não existe".into()));
                return;
            };
            let _ = canal.send(Recado::Aberta(Box::new(GaleriaAberta {
                galeria,
                fotos: self.fotos_da_sessao.lock().expect("as fotos").clone(),
                vence_venda: None,
                vence_download: None,
                resumos: Default::default(),
            })));
        }

        fn avisar(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            // 🔚 Como o site: sem contato não há aviso, e nada se registra.
            if self.sem_email(&galeria_id) {
                let _ = canal.send(Self::falta_email(GestoDoFim::Avisar));
                return;
            }
            self.avisadas.lock().expect("as avisadas").push(galeria_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        /// Grava na galeria guardada — a próxima abertura e o próximo link a
        /// veem como o site veria.
        fn atualizar_galeria(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            mudanca: MudancaDaGaleria,
            canal: Sender<Recado>,
        ) {
            if let Some(galeria) = self
                .galerias
                .lock()
                .expect("as galerias")
                .iter_mut()
                .find(|g| g.id == galeria_id)
            {
                if let Some(titulo) = &mudanca.titulo {
                    galeria.titulo = titulo.clone();
                }
                if let Some(email) = &mudanca.email {
                    galeria.email = email.clone();
                }
                if let Some(whatsapp) = &mudanca.whatsapp {
                    galeria.whatsapp = whatsapp.clone();
                }
            }
            self.atualizacoes
                .lock()
                .expect("as atualizacoes")
                .push((galeria_id, mudanca));
            self.responder_ou_guardar(canal, Recado::GaleriaAtualizada);
        }

        fn capa_do_estudio(&self, estudio_id: String, _url: String, canal: Sender<Recado>) {
            self.capas_pedidas
                .lock()
                .expect("as capas")
                .push(estudio_id.clone());
            let _ = canal.send(Recado::CapaDoEstudio {
                estudio_id,
                bytes: Arc::new(Self::jpeg(64)),
            });
        }

        fn miniatura(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
            self.miniaturas_pedidas
                .lock()
                .expect("as miniaturas pedidas")
                .push(foto_id.clone());
            let _ = canal.send(Recado::Miniatura {
                foto_id,
                bytes: jpeg_de_um_pixel(),
            });
        }

        fn copia_de_trabalho(
            &self,
            _sessao: Sessao,
            foto_local: String,
            foto_no_site: String,
            canal: Sender<Recado>,
        ) {
            self.baixadas
                .lock()
                .expect("as baixadas")
                .push(foto_no_site);
            // Um JPEG 1×1 de verdade: a tela decodifica o que chega, e um vetor
            // de lixo faria o teste passar por um caminho que a produção não tem.
            let recado = match &self.copia_falha {
                Some(erro) => Recado::Falhou(erro.clone()),
                None => Recado::Pixels {
                    foto_id: foto_local,
                    bytes: jpeg_de_um_pixel(),
                },
            };
            if self.copia_demorada {
                self.guardados
                    .lock()
                    .expect("os guardados")
                    .push((canal, recado));
            } else {
                let _ = canal.send(recado);
            }
        }

        fn negociar(
            &self,
            _sessao: Sessao,
            foto_id: String,
            mudanca: MudancaDaFoto,
            canal: Sender<Recado>,
        ) {
            // 🔑 **Como o site de verdade: o `PATCH` fica na foto**, e a
            // releitura seguinte o devolve. Um site que responde "ok" e não
            // grava faria a grade desfazer o gesto a cada releitura — e os
            // testes passariam por um caminho que a produção não tem.
            if let Some(foto) = self
                .fotos_da_sessao
                .lock()
                .expect("as fotos")
                .iter_mut()
                .find(|f| f.id == foto_id)
            {
                if let Some(nota) = mudanca.nota {
                    foto.nota = nota.and_then(|n| u8::try_from(n).ok());
                }
                if let Some(rejeitada) = mudanca.rejeitada {
                    foto.rejeitada = rejeitada;
                }
                if let Some(preco) = mudanca.preco_negociado.clone() {
                    foto.preco_negociado = preco;
                }
                if let Some(observacao) = mudanca.observacao_da_negociacao.clone() {
                    foto.observacao_da_negociacao = observacao;
                }
                if let Some(estado) = mudanca.estado {
                    foto.estado = match estado {
                        EstadoNoBalcao::LevadaNoBalcao => {
                            domain::services::pos_venda::EstadoDaFotoNoSite::LevadaNoBalcao
                        }
                        EstadoNoBalcao::Disponivel => {
                            domain::services::pos_venda::EstadoDaFotoNoSite::Disponivel
                        }
                    };
                }
            }
            self.negociadas
                .lock()
                .expect("as negociadas")
                .push((foto_id.clone(), mudanca));
            let recado = Recado::Negociou {
                foto_id,
                erro: None,
            };
            if self.negociacao_demorada {
                self.guardados
                    .lock()
                    .expect("os guardados")
                    .push((canal, recado));
            } else {
                let _ = canal.send(recado);
            }
        }

        fn galerias(&self, _sessao: Sessao, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Galerias(
                self.galerias.lock().expect("as galerias").clone(),
            ));
        }

        fn criar_galeria(&self, _sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
            let id = format!("g{}", self.criadas.lock().expect("as criadas").len() + 1);
            self.criadas.lock().expect("as criadas").push(nova.clone());
            // A criada entra na lista, como entraria no site.
            self.galerias
                .lock()
                .expect("as galerias")
                .push(GaleriaDoPainel {
                    id: id.clone(),
                    titulo: nova.titulo.clone(),
                    email: nova.email.clone(),
                    whatsapp: nova.whatsapp.clone(),
                    produto_id: nova.produto_id.clone(),
                    user_id: None,
                    criada_em_iso: "2026-09-06".into(),
                    expira_em: None,
                    fotos: domain::services::pos_venda::ContagemDeFotos::default(),
                    totais: None,
                    ..Default::default()
                });
            let _ = canal.send(Recado::Criada(Galeria {
                id,
                titulo: nova.titulo,
            }));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::{Natureza, PedidoDeFoto};

    /// A tabela inteira: o que grava no site é envio, o que só traz bytes é
    /// leitura. Um pedido novo sem lugar aqui não compila (o `match` é
    /// exaustivo), e um mal classificado quebra este teste.
    #[test]
    fn baixar_nao_e_enviar() {
        use PedidoDeFoto::*;
        for envio in [
            SubirClassificada,
            TirarDoSite,
            SalvarRevelacao,
            Negociar,
            EnviarArquivo,
        ] {
            assert_eq!(envio.natureza(), Natureza::Envio, "{envio:?}");
        }
        for leitura in [CopiaDeTrabalho, Miniatura, Original, RevelarIntegral] {
            assert_eq!(leitura.natureza(), Natureza::Leitura, "{leitura:?}");
        }
    }
}
