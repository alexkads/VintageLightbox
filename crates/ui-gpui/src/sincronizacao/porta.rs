//! 📴 A porta local-primeiro: o mesmo [`Publicador`] que a tela já usa, em
//! volta do que fala com a API.
//!
//! 🔑 **Um decorador, e não um `if` em cada tela.** As telas continuam pedindo
//! e recebendo os mesmos [`Recado`]s; é aqui que se decide se o gesto vai ao
//! site agora ou se fica na fila. Com rede e fila vazia, nada muda: o gesto vai
//! direto, e a recusa de regra chega na hora, como sempre chegou.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use domain::services::pos_venda::{
    EstadoDaFotoNoSite, EstadoNoBalcao, Galeria, MudancaDaFoto, MudancaDaGaleria, NovaGaleria,
    Sessao,
};
use domain::value_objects::CropSettings;
use infrastructure::gpu_adjustments::Ajustes;

use super::{e_falta_de_rede, e_passageira, Gesto, Sincronizacao};
use crate::pos_venda::porta::{FotoClassificada, PedidoCru, PedidoJson, Publicador, Recado};

/// A frase dos gestos que precisam do site na hora.
pub const PRECISA_DE_INTERNET: &str =
    "precisa de internet — tente quando a rede voltar. O resto do trabalho segue guardado.";

pub struct PublicadorLocalPrimeiro {
    dentro: Arc<dyn Publicador>,
    sinc: Arc<Sincronizacao>,
    /// A conta de agora — o trabalhador da fila chama o site em nome dela.
    sessao: Arc<Mutex<Option<Sessao>>>,
    /// As sessões já pré-carregadas nesta abertura do app.
    pre_carregadas: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// Quantas sessões recentes a pré-carga leva ao espelho.
const SESSOES_PRE_CARREGADAS: usize = 6;
/// O teto de miniaturas por sessão na pré-carga.
const MINIATURAS_POR_SESSAO: usize = 400;
/// O teto de cópias de trabalho por sessão na pré-carga — cada uma tem uns
/// poucos MB.
const COPIAS_POR_SESSAO: usize = 80;

impl PublicadorLocalPrimeiro {
    pub fn novo(
        dentro: Arc<dyn Publicador>,
        sinc: Arc<Sincronizacao>,
        sessao: Arc<Mutex<Option<Sessao>>>,
    ) -> Self {
        Self {
            dentro,
            sinc,
            sessao,
            pre_carregadas: Arc::default(),
        }
    }

    /// 📥 **A pré-carga**: com a lista de sessões em mãos e a rede de pé, as
    /// mais recentes são abertas por trás — a galeria e as miniaturas —, e o
    /// espelho as guarda. É o que deixa a sessão de hoje abrir com a grade
    /// cheia depois de a internet cair, mesmo que ninguém a tenha aberto
    /// neste computador.
    ///
    /// ⚠️ Uma por vez, numa tarefa só, e cada sessão uma vez por abertura do
    /// app: o balcão não pode disputar a rede com a subida das fotos.
    fn pre_carregar(&self, sessao: Sessao, ids: Vec<String>) {
        let novas: Vec<String> = {
            let Ok(mut feitas) = self.pre_carregadas.lock() else {
                return;
            };
            ids.into_iter()
                .filter(|id| feitas.insert(id.clone()))
                .take(SESSOES_PRE_CARREGADAS)
                .collect()
        };
        if novas.is_empty() {
            return;
        }
        let dentro = self.dentro.clone();
        let rede = self.sinc.rede().clone();
        self.sinc.tokio().spawn_blocking(move || {
            for id in novas {
                if !rede.com_rede() {
                    return;
                }
                let (canal, resposta) = std::sync::mpsc::channel();
                dentro.abrir_galeria(sessao.clone(), id, canal);
                let Ok(Recado::Aberta(aberta)) =
                    resposta.recv_timeout(std::time::Duration::from_secs(120))
                else {
                    continue;
                };
                // 📸 O QR do bot da sessão também: o cliente o lê com o celular
                // dele, e o bot funciona sem a internet do estúdio.
                let (canal, resposta) = std::sync::mpsc::channel();
                dentro.pedir_json(
                    sessao.clone(),
                    PedidoJson::ler(
                        "pre-carga",
                        crate::sessoes::qr_do_bot::caminho_da_situacao(&aberta.galeria.id),
                    ),
                    canal,
                );
                let _ = resposta.recv_timeout(std::time::Duration::from_secs(60));
                for foto in aberta
                    .fotos
                    .iter()
                    .filter(|f| !f.apagada)
                    .take(MINIATURAS_POR_SESSAO)
                {
                    if !rede.com_rede() {
                        return;
                    }
                    let (canal, resposta) = std::sync::mpsc::channel();
                    dentro.miniatura(sessao.clone(), foto.id.clone(), canal);
                    let _ = resposta.recv_timeout(std::time::Duration::from_secs(60));
                }
                // 🎞️ E a cópia de trabalho das levadas e compradas: é delas que
                // a Revelação parte, e são elas que o balcão revela no dia.
                for foto in aberta
                    .fotos
                    .iter()
                    .filter(|f| !f.apagada && f.estado != EstadoDaFotoNoSite::Disponivel)
                    .take(COPIAS_POR_SESSAO)
                {
                    if !rede.com_rede() {
                        return;
                    }
                    let (canal, resposta) = std::sync::mpsc::channel();
                    dentro.copia_de_trabalho(
                        sessao.clone(),
                        format!("site:{}", foto.id),
                        foto.id.clone(),
                        canal,
                    );
                    let _ = resposta.recv_timeout(std::time::Duration::from_secs(120));
                }
            }
        });
    }

    fn lembrar(&self, sessao: &Sessao) {
        if let Ok(mut viva) = self.sessao.lock() {
            *viva = Some(sessao.clone());
        }
    }

    /// Vai já, ou guarda? Guarda quando a rede está caída e quando já há fila
    /// — o gesto novo não fura a vez dos de antes.
    fn guardar_ja(&self) -> bool {
        !self.sinc.rede().com_rede() || self.sinc.tem_pendente()
    }

    /// 🔑 **O gesto que grava**: vai direto se pode; se o site não responder,
    /// ou se não pode ir, fica na fila e a tela recebe `guardado`.
    ///
    /// A resposta do site passa por aqui antes de chegar à tela: a falha de
    /// rede é trocada pelo `guardado`, e qualquer outra resposta segue igual.
    fn gravar(
        &self,
        gesto: Gesto,
        canal: Sender<Recado>,
        guardado: Recado,
        chamar: impl FnOnce(Sender<Recado>),
    ) {
        if self.guardar_ja() {
            let sem_internet = !self.sinc.rede().com_rede();
            self.sinc.guardar(gesto);
            let _ = canal.send(com_o_motivo(guardado, sem_internet));
            return;
        }
        let (para_ca, daqui) = std::sync::mpsc::channel();
        chamar(para_ca);
        let sinc = self.sinc.clone();
        self.sinc.tokio().spawn_blocking(move || {
            let mut gesto = Some(gesto);
            for recado in daqui {
                if let Some(frase) = frase_de_falha(&recado).filter(|f| e_passageira(f)) {
                    if let Some(gesto) = gesto.take() {
                        let sem_internet = e_falta_de_rede(frase);
                        if sem_internet {
                            sinc.rede().marcar_sem_rede();
                        }
                        sinc.guardar(gesto);
                        let _ = canal.send(com_o_motivo(guardado.clone(), sem_internet));
                        continue;
                    }
                }
                let _ = canal.send(recado);
            }
        });
    }

    /// 📶 O gesto que só existe com o site (apagar, tirar da nuvem): vai
    /// direto, como sempre — e a falha de rede marca a rede como caída, para o
    /// gesto seguinte já ir à fila e o selo acender (revisão de 20/10/2026).
    fn repassar(&self, canal: Sender<Recado>, chamar: impl FnOnce(Sender<Recado>)) {
        self.ler(canal, chamar, |sinc, recado| {
            if frase_de_falha(&recado).is_some_and(e_falta_de_rede) {
                sinc.rede().marcar_sem_rede();
            }
            recado
        });
    }

    /// A leitura: o que voltar passa pela projeção da fila.
    fn ler(
        &self,
        canal: Sender<Recado>,
        chamar: impl FnOnce(Sender<Recado>),
        projetar: impl Fn(&Sincronizacao, Recado) -> Recado + Send + 'static,
    ) {
        let (para_ca, daqui) = std::sync::mpsc::channel();
        chamar(para_ca);
        let sinc = self.sinc.clone();
        self.sinc.tokio().spawn_blocking(move || {
            for recado in daqui {
                let _ = canal.send(projetar(&sinc, recado));
            }
        });
    }
}

/// O que a pré-carga precisa fora da porta (ela roda numa thread à parte).
struct PreCarga {
    porta_pre_carregadas: Arc<Mutex<std::collections::HashSet<String>>>,
    dentro: Arc<dyn Publicador>,
    sinc: Arc<Sincronizacao>,
    sessao: Sessao,
}

impl PreCarga {
    fn disparar(&self, ids: Vec<String>) {
        PublicadorLocalPrimeiro {
            dentro: self.dentro.clone(),
            sinc: self.sinc.clone(),
            sessao: Arc::default(),
            pre_carregadas: self.porta_pre_carregadas.clone(),
        }
        .pre_carregar(self.sessao.clone(), ids);
    }
}

/// `PATCH /pos-venda/fotos/{id}` → o id da foto.
fn foto_do_patch(metodo: &str, caminho: &str) -> Option<String> {
    if !metodo.eq_ignore_ascii_case("PATCH") {
        return None;
    }
    let id = caminho.strip_prefix("/pos-venda/fotos/")?;
    (!id.is_empty() && !id.contains('/') && !id.contains('?')).then(|| id.to_string())
}

/// `GET /pos-venda/galerias/{id}` → o id da sessão.
fn galeria_do_caminho(caminho: &str) -> Option<&str> {
    let id = caminho.strip_prefix("/pos-venda/galerias/")?;
    (!id.is_empty() && !id.contains('/') && !id.contains('?')).then_some(id)
}

/// 📴 O `guardado` com o porquê: sem internet, ou guardado com a rede de pé
/// (o soluço do site, a vez atrás da fila). A frase da tela fala de internet
/// só no primeiro caso — o caixa lê `sem_internet` na resposta guardada.
fn com_o_motivo(guardado: Recado, sem_internet: bool) -> Recado {
    match guardado {
        Recado::GuardadoParaDepois { alvo, .. } => {
            Recado::GuardadoParaDepois { alvo, sem_internet }
        }
        Recado::Json {
            rotulo,
            resultado: Ok(mut valor),
        } if valor.get("guardada_sem_rede").is_some() => {
            valor["sem_internet"] = sem_internet.into();
            Recado::Json {
                rotulo,
                resultado: Ok(valor),
            }
        }
        outro => outro,
    }
}

/// A frase da falha que o recado traz, quando ele é uma falha.
pub fn frase_de_falha(recado: &Recado) -> Option<&str> {
    match recado {
        Recado::Falhou(frase)
        | Recado::GaleriaNaoAtualizada(frase)
        | Recado::EnvioFalhou { frase, .. }
        | Recado::Negociou {
            erro: Some(frase), ..
        }
        | Recado::Json {
            resultado: Err(frase),
            ..
        } => Some(frase),
        _ => None,
    }
}

/// O recado é "o site não respondeu"?
pub fn falta_de_rede(recado: &Recado) -> bool {
    frase_de_falha(recado).is_some_and(e_falta_de_rede)
}

impl Publicador for PublicadorLocalPrimeiro {
    fn edicao_excluida(&self, foto_no_site: &str) {
        self.dentro.edicao_excluida(foto_no_site);
    }

    fn autorizar(&self, canal: Sender<Recado>) {
        self.dentro.autorizar(canal);
    }

    fn retomar(&self, canal: Sender<Recado>) {
        self.dentro.retomar(canal);
    }

    fn entrar_com_senha(&self, email: String, senha: String, canal: Sender<Recado>) {
        self.dentro.entrar_com_senha(email, senha, canal);
    }

    fn entrar_com_autenticador(&self, email: String, codigo: String, canal: Sender<Recado>) {
        self.dentro.entrar_com_autenticador(email, codigo, canal);
    }

    fn entrar_pelo_link(
        &self,
        email: String,
        cancelado: Arc<std::sync::atomic::AtomicBool>,
        canal: Sender<Recado>,
    ) {
        self.dentro.entrar_pelo_link(email, cancelado, canal);
    }

    fn pedir_troca_de_senha(&self, email: String, canal: Sender<Recado>) {
        self.dentro.pedir_troca_de_senha(email, canal);
    }

    /// O pedido JSON das telas que só repassam JSON — o caixa, a retenção, o
    /// atendimento.
    ///
    /// - **Leitura**: passa pela projeção da fila (o caixa aberto sem rede, a
    ///   venda guardada).
    /// - **As três gravações do caixa** (abrir, vender, movimentar): vão já ou
    ///   ficam na fila, como a nota.
    /// - **O resto que grava**: sem rede, a frase de que precisa de internet —
    ///   e não o erro cru da conexão.
    fn pedir_json(&self, sessao: Sessao, pedido: PedidoJson, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        // O `PATCH` de foto pelo pedido cru (o lote de faixa e preço do
        // caixa) é a mesma mudança da nota: vai pela mesma fila.
        if let Some(foto_id) = foto_do_patch(pedido.metodo, &pedido.caminho) {
            let mudanca =
                MudancaDaFoto::do_corpo(pedido.corpo.as_ref().unwrap_or(&serde_json::Value::Null));
            let guardado = Recado::Json {
                rotulo: pedido.rotulo,
                resultado: Ok(serde_json::json!({ "guardada_sem_rede": true })),
            };
            let gesto = Gesto::MudarFoto { foto_id, mudanca };
            self.gravar(gesto, canal, guardado, move |c| {
                dentro.pedir_json(sessao, pedido, c)
            });
            return;
        }
        // A sessão criada pelo assistente (o atendimento vai junto): o id
        // nasce aqui, e sem rede ela fica na fila como a da porta tipada.
        if pedido.metodo.eq_ignore_ascii_case("POST") && pedido.caminho == "/pos-venda/galerias" {
            let mut corpo = pedido
                .corpo
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            if corpo
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_none()
            {
                corpo["id"] = uuid::Uuid::new_v4().to_string().into();
            }
            let mut eco = corpo.clone();
            eco["guardada_sem_rede"] = true.into();
            let guardado = Recado::Json {
                rotulo: pedido.rotulo,
                resultado: Ok(eco),
            };
            let gesto = Gesto::PedidoJson {
                metodo: "POST".into(),
                caminho: pedido.caminho.clone(),
                corpo: corpo.clone(),
            };
            let pedido = PedidoJson {
                corpo: Some(corpo),
                ..pedido
            };
            self.gravar(gesto, canal, guardado, move |c| {
                dentro.pedir_json(sessao, pedido, c)
            });
            return;
        }
        // A sessão criada sem rede, lida pelo pedido cru: a daqui, vazia.
        if pedido.metodo.eq_ignore_ascii_case("GET") {
            if let Some(json) = galeria_do_caminho(&pedido.caminho)
                .and_then(|id| self.sinc.galeria_criada_aqui_em_json(id))
            {
                let _ = canal.send(Recado::Json {
                    rotulo: pedido.rotulo,
                    resultado: Ok(json),
                });
                return;
            }
        }
        if pedido.metodo.eq_ignore_ascii_case("GET") {
            let caminho = pedido.caminho.clone();
            self.ler(
                canal,
                move |c| dentro.pedir_json(sessao, pedido, c),
                move |sinc, recado| match recado {
                    Recado::Json {
                        rotulo,
                        resultado: Ok(mut valor),
                    } => {
                        if galeria_do_caminho(&caminho).is_some() {
                            sinc.projetar_galeria_json(&mut valor);
                        }
                        Recado::Json {
                            rotulo,
                            resultado: Ok(sinc
                                .projetar_json(&caminho, Some(valor))
                                .unwrap_or(serde_json::Value::Null)),
                        }
                    }
                    Recado::Json {
                        rotulo,
                        resultado: Err(frase),
                    } if e_passageira(&frase) => Recado::Json {
                        rotulo,
                        resultado: sinc.projetar_json(&caminho, None).ok_or(frase),
                    },
                    outro => outro,
                },
            );
            return;
        }
        if let Some(tipo) = super::caixa::guardavel(pedido.metodo, &pedido.caminho) {
            // 🕐 **A hora do balcão vai só com o que fica na fila.** A venda
            // das 14h guardada sem rede entra às 14h — e o site a aceita mesmo
            // que a foto tenha sido vendida no meio-tempo (o dinheiro já está
            // na gaveta), marcando a duplicidade. A venda que vai **agora**
            // não leva a hora: o site tem de poder recusar a foto já vendida
            // antes de o operador cobrar (verificação de 21/10/2026).
            let mut guardado_corpo = pedido
                .corpo
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            guardado_corpo["ocorrido_em"] = chrono::Local::now().to_rfc3339().into();
            let guardado = Recado::Json {
                rotulo: pedido.rotulo,
                resultado: Ok(super::caixa::resposta_guardada(tipo, &guardado_corpo)),
            };
            let gesto = Gesto::PedidoJson {
                metodo: pedido.metodo.to_string(),
                caminho: pedido.caminho.clone(),
                corpo: guardado_corpo,
            };
            self.gravar(gesto, canal, guardado, move |c| {
                dentro.pedir_json(sessao, pedido, c)
            });
            return;
        }
        // O resto que grava segue para o site como sempre: sem rede, a falha
        // de rede de sempre, que as telas e os relatos automáticos já sabem
        // ler (`erro_passageiro`).
        self.dentro.pedir_json(sessao, pedido, canal);
    }

    fn pedir_cru(&self, sessao: Sessao, pedido: PedidoCru, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        self.dentro.pedir_cru(sessao, pedido, canal);
    }

    fn sair(&self, canal: Sender<Recado>) {
        if let Ok(mut viva) = self.sessao.lock() {
            *viva = None;
        }
        self.dentro.sair(canal);
    }

    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        self.dentro.produtos(sessao, canal);
    }

    fn estudios(&self, sessao: Sessao, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        self.dentro.estudios(sessao, canal);
    }

    /// O link é o site que assina: sem rede, não há link para mostrar.
    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        if self.sinc.aberta_criada_aqui(&galeria_id).is_some() {
            let _ = canal.send(Recado::Falhou(format!(
                "a sessão ainda não chegou ao site — o link sai depois que ela subir. {PRECISA_DE_INTERNET}"
            )));
            return;
        }
        // Tenta sempre — a bandeira da rede pode estar atrasada em relação à
        // rede de verdade —, e a falha de rede vira a frase de que o link
        // precisa de internet.
        let dentro = self.dentro.clone();
        self.ler(
            canal,
            move |c| dentro.link(sessao, galeria_id, c),
            |sinc, recado| match frase_de_falha(&recado) {
                Some(frase) if e_falta_de_rede(frase) => {
                    sinc.rede().marcar_sem_rede();
                    Recado::Falhou(format!("O link do cliente {PRECISA_DE_INTERNET}"))
                }
                _ => recado,
            },
        );
    }

    fn atualizar_galeria(
        &self,
        sessao: Sessao,
        galeria_id: String,
        mudanca: MudancaDaGaleria,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        let gesto = Gesto::AtualizarGaleria {
            galeria_id: galeria_id.clone(),
            mudanca: mudanca.clone(),
        };
        let dentro = self.dentro.clone();
        // A sessão criada aqui ainda não existe lá: a edição espera atrás dela.
        if self.sinc.aberta_criada_aqui(&galeria_id).is_some() {
            self.sinc.guardar(gesto);
            let _ = canal.send(Recado::GaleriaAtualizada);
            return;
        }
        self.gravar(gesto, canal, Recado::GaleriaAtualizada, move |c| {
            dentro.atualizar_galeria(sessao, galeria_id, mudanca, c)
        });
    }

    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        // A lista passa pela pré-carga antes da projeção: as mais recentes
        // (a lista vem da mais nova para a mais velha) vão ao espelho.
        let (para_a_tela, da_lista) = std::sync::mpsc::channel();
        let pre = PreCarga {
            porta_pre_carregadas: self.pre_carregadas.clone(),
            dentro: self.dentro.clone(),
            sinc: self.sinc.clone(),
            sessao: sessao.clone(),
        };
        let canal_final = canal;
        self.sinc.tokio().spawn_blocking(move || {
            for recado in da_lista {
                if let (Recado::Galerias(lista), true) = (&recado, pre.sinc.rede().com_rede()) {
                    // As de ontem e de hoje: é o que o balcão atende agora.
                    let desde = (chrono::Local::now() - chrono::Duration::days(1))
                        .format("%Y-%m-%d")
                        .to_string();
                    pre.disparar(
                        lista
                            .iter()
                            .filter(|g| g.criada_em_iso >= desde)
                            .map(|g| g.id.clone())
                            .collect(),
                    );
                }
                let _ = canal_final.send(recado);
            }
        });
        let canal = para_a_tela;
        self.ler(
            canal,
            move |c| dentro.galerias(sessao, c),
            |sinc, recado| match recado {
                Recado::Galerias(mut lista) => {
                    sinc.projetar_lista(&mut lista);
                    Recado::Galerias(lista)
                }
                // Sem rede e sem espelho (a primeira abertura do dia, offline):
                // as sessões criadas aqui ainda aparecem.
                Recado::Falhou(frase) if e_passageira(&frase) => {
                    let mut lista = Vec::new();
                    sinc.projetar_lista(&mut lista);
                    if lista.is_empty() {
                        Recado::Falhou(frase)
                    } else {
                        Recado::Galerias(lista)
                    }
                }
                outro => outro,
            },
        );
    }

    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        // Sem id escolhido aqui não há como a sessão existir antes do site.
        let Some(id) = nova.id.clone() else {
            self.dentro.criar_galeria(sessao, nova, canal);
            return;
        };
        let guardado = Recado::Criada(Galeria {
            id,
            titulo: nova.titulo.clone(),
        });
        let dentro = self.dentro.clone();
        let gesto = Gesto::CriarGaleria(nova.clone());
        self.gravar(gesto, canal, guardado, move |c| {
            dentro.criar_galeria(sessao, nova, c)
        });
    }

    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto: FotoClassificada,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        let guardado = Recado::GuardadoParaDepois {
            alvo: foto.foto_id.clone(),
            sem_internet: true,
        };
        let gesto = Gesto::SubirClassificada {
            galeria_id: galeria_id.clone(),
            foto: foto.clone(),
        };
        let dentro = self.dentro.clone();
        self.gravar(gesto, canal, guardado, move |c| {
            dentro.subir_classificada(sessao, galeria_id, foto, c)
        });
    }

    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        self.repassar(canal, move |c| dentro.tirar_do_site(sessao, foto_id, c));
    }

    fn rejeitar_tirando_da_nuvem(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        self.repassar(canal, move |c| {
            dentro.rejeitar_tirando_da_nuvem(sessao, foto_id, c)
        });
    }

    fn remover_remoto(&self, sessao: Sessao, no_site: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        self.repassar(canal, move |c| dentro.remover_remoto(sessao, no_site, c));
    }

    fn excluir_da_sessao(
        &self,
        sessao: Sessao,
        no_site: Option<String>,
        local: Option<String>,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        let dentro = self.dentro.clone();
        self.repassar(canal, move |c| {
            dentro.excluir_da_sessao(sessao, no_site, local, c)
        });
    }

    /// A nota, o `B`, o `X` e a negociação: o `PATCH` da foto.
    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        let guardado = Recado::Negociou {
            foto_id: foto_id.clone(),
            erro: None,
        };
        let gesto = Gesto::MudarFoto {
            foto_id: foto_id.clone(),
            mudanca: mudanca.clone(),
        };
        let dentro = self.dentro.clone();
        self.gravar(gesto, canal, guardado, move |c| {
            dentro.negociar(sessao, foto_id, mudanca, c)
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
        self.lembrar(&sessao);
        self.dentro
            .enviar_arquivo(sessao, galeria_id, caminho, ordem, estado, canal);
    }

    fn abrir_galeria(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        // A sessão criada sem rede ainda não está no site: abre a daqui.
        if let Some(aberta) = self.sinc.aberta_criada_aqui(&galeria_id) {
            let _ = canal.send(Recado::Aberta(Box::new(aberta)));
            return;
        }
        let dentro = self.dentro.clone();
        self.ler(
            canal,
            move |c| dentro.abrir_galeria(sessao, galeria_id, c),
            |sinc, recado| match recado {
                Recado::Aberta(mut aberta) => {
                    sinc.projetar_aberta(&mut aberta);
                    Recado::Aberta(aberta)
                }
                outro => outro,
            },
        );
    }

    fn miniatura(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        self.dentro.miniatura(sessao, foto_id, canal);
    }

    fn salvar_revelacao(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        let guardado = Recado::GuardadoParaDepois {
            alvo: foto_no_site.clone(),
            sem_internet: true,
        };
        let gesto = Gesto::SalvarRevelacao {
            foto_no_site: foto_no_site.clone(),
            parametros: crate::pos_venda::porta::ajustes_em_json(&ajustes, &corte),
        };
        let dentro = self.dentro.clone();
        self.gravar(gesto, canal, guardado, move |c| {
            dentro.salvar_revelacao(sessao, foto_no_site, ajustes, corte, c)
        });
    }

    fn avisar(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        let guardado = Recado::GuardadoParaDepois {
            alvo: galeria_id.clone(),
            sem_internet: true,
        };
        let gesto = Gesto::Avisar {
            galeria_id: galeria_id.clone(),
        };
        let dentro = self.dentro.clone();
        self.gravar(gesto, canal, guardado, move |c| {
            dentro.avisar(sessao, galeria_id, c)
        });
    }

    fn capa_do_estudio(&self, estudio_id: String, url: String, canal: Sender<Recado>) {
        self.dentro.capa_do_estudio(estudio_id, url, canal);
    }

    fn original(&self, sessao: Sessao, foto_no_site: String, canal: Sender<Recado>) {
        self.lembrar(&sessao);
        self.dentro.original(sessao, foto_no_site, canal);
    }

    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    ) {
        self.lembrar(&sessao);
        self.dentro
            .copia_de_trabalho(sessao, foto_local, foto_no_site, canal);
    }
}
