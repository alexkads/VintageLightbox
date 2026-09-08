//! A porta para o pós-venda do `recordarfotos.com.br`.
//!
//! É o vão que o projeto existe para fechar ([`docs/00-OBJETIVO.md`]): a decisão
//! que o fotógrafo toma na triagem — esta foi levada, esta ficou — vira galeria
//! no site sem passo manual.
//!
//! # O fluxo inteiro, e não só o envio
//!
//! Até 6/set/2026 a porta sabia quatro coisas: entrar, listar o produto que dá o
//! preço, criar a galeria e subir uma foto. Dava para publicar — e não dava para
//! **acompanhar** o passo seguinte do balcão, que é onde o dinheiro entra.
//!
//! Os quatro métodos que entraram vêm do fluxo do dono, passo a passo, e nenhum
//! deles pediu rota nova: o backend já expunha as quatro.
//!
//! | Passo do fluxo | O que faltava aqui |
//! |---|---|
//! | classificar sobe a foto | [`PosVendaApi::galerias`] — para subir **numa galeria que já existe**, em vez de criar uma por leva |
//! | zerar a nota tira do storage | [`PosVendaApi::remover_foto`] |
//! | o cliente paga no balcão | [`PosVendaApi::mudar_foto`] — a negociação e o estado |
//! | gerar o link do cliente | [`PosVendaApi::link_da_galeria`] |
//!
//! 🚨 **`nota: null` é recusado pelo site, e isso não é limitação: é a regra.**
//! Foi a classificação que autorizou a foto a subir, então uma foto do acervo
//! sem nota não existe. Zerar a nota é [`PosVendaApi::remover_foto`] — a foto sai
//! do storage e volta a ser só local, que é o mesmo ciclo da área temporária do
//! navegador.
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
use serde_json::Value;

use crate::entities::Photo;
use crate::DomainResult;

/// O que fica de uma autorização: o par de tokens e quando o de acesso vence.
///
/// # Por que o de renovação passou a caber aqui
///
/// Antes só havia o de acesso, e a nota dizia que guardar o de renovação seria
/// "deixar a credencial do estúdio num JSON ao lado do catálogo". A objeção
/// continua certa — o que mudou é **onde** ele é guardado: no chaveiro do
/// sistema (`CofreDeSessao`), nunca em arquivo do app. Sem ele, a sessão morria
/// aos quinze minutos do token de acesso e o operador voltava ao login no meio
/// do balcão.
///
/// 🚨 **Isto não vai para `pos-venda.json`.** O teste
/// `o_arquivo_nao_tem_onde_guardar_senha` continua de pé, e é o que impede a
/// volta do atalho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessao {
    pub access_token: String,
    /// O que renova o de acesso sem novo login. Vale quinze dias para o app —
    /// o site emite sete para o navegador (`ClasseDeCliente`, no backend).
    pub refresh_token: String,
    /// Quando o **de acesso** vence, em segundos desde a época.
    ///
    /// Absoluto, e não "faltam N segundos": a sessão é guardada e relida depois
    /// de o app ter ficado fechado a noite inteira, e um prazo relativo gravado
    /// ontem diria que ainda faltam quinze minutos.
    pub access_vence_em: i64,
    /// Quando o **de renovação** vence. Passado ele, não há o que renovar: é
    /// autorizar de novo.
    pub refresh_vence_em: i64,
}

impl Sessao {
    /// Margem antes do vencimento, em segundos.
    ///
    /// Renovar só depois de vencer deixa uma janela em que a chamada sai com um
    /// token que expira no caminho — e o operador vê "sessão recusada" no meio
    /// de uma subida de trinta fotos. É o mesmo minuto que o site usa no proxy.
    pub const MARGEM: i64 = 60;

    /// O de acesso ainda serve para a chamada que vai sair agora?
    pub fn acesso_utilizavel(&self, agora: i64) -> bool {
        self.access_vence_em - Self::MARGEM > agora
    }

    /// Ainda dá para renovar? `false` é autorizar de novo, do zero.
    pub fn renovavel(&self, agora: i64) -> bool {
        self.refresh_vence_em - Self::MARGEM > agora
    }
}

/// Onde o par de tokens dorme entre uma abertura do app e a seguinte.
///
/// # Por que uma porta, e não `std::fs` direto
///
/// Porque a implementação certa é o **chaveiro do sistema** (Keychain no macOS),
/// e chaveiro não existe em teste: o teste não pode pedir a senha do usuário nem
/// sujar o chaveiro da máquina de quem roda `cargo test`. Com a porta, o teste
/// usa um cofre em memória e a produção usa o do sistema.
///
/// ⚠️ **Nenhum método devolve `Result`.** Guardar a sessão é acessório: se o
/// chaveiro recusar, o operador perde a comodidade de não reautorizar amanhã —
/// não perde o dia de trabalho. Um `?` aqui faria a falha do acessório derrubar
/// o principal, que é o mesmo motivo pelo qual as portas de evento do site não
/// devolvem `Result`.
pub trait CofreDeSessao: Send + Sync {
    fn guardar(&self, sessao: &Sessao);
    fn ler(&self) -> Option<Sessao>;
    fn esquecer(&self);
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

/// Uma galeria que **já existe**, como o painel do site a lista.
///
/// 🔑 Traz o contato porque é ele que identifica o cliente no balcão: duas
/// galerias com o mesmo título e clientes diferentes são o caso comum de um
/// estúdio, e escolher a errada manda as fotos de um cliente para outro.
///
/// 🔑 **E traz o que a lista precisa para decidir a situação sozinha** —
/// contagem por estado, prazo, se o cliente já entrou. Quem calcula a situação
/// é `biblioteca_core::sessoes`, a mesma conta da lista do site; o que chega
/// aqui é o dado cru para ela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaleriaDoPainel {
    pub id: String,
    pub titulo: String,
    pub email: Option<String>,
    pub whatsapp: Option<String>,
    pub produto_id: String,
    /// Preenchido quando o cliente já criou conta pelo link — é o que separa
    /// "aguardando o cliente" de "cliente já abriu".
    pub user_id: Option<String>,
    /// `"2026-09-03"`, já reduzida ao dia: é o carimbo do eixo do gráfico.
    pub criada_em_iso: String,
    /// Segundos desde a época. `None` = não expira.
    pub expira_em: Option<i64>,
    pub fotos: ContagemDeFotos,
    /// `None` na galeria de uma API anterior ao campo — e isso é dito na soma,
    /// em vez de virar zero calado.
    pub totais: Option<TotaisDaGaleria>,
}

/// Quantas fotos a galeria tem, por estado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContagemDeFotos {
    pub levadas_no_balcao: u32,
    pub disponiveis: u32,
    pub compradas: u32,
    /// Apagadas pela retenção: a linha ficou, o arquivo não.
    pub apagadas: u32,
}

/// Quanto a galeria já rendeu, por porta.
///
/// 🔑 **Decimal em texto** (`"150.00"`), como [`Produto::preco`] — é como o site
/// devolve, e converter aqui exigiria uma escolha de arredondamento que o
/// `domain` não tem por que tomar. Quem lê para centavos é a tela, com
/// `biblioteca_core::dinheiro`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotaisDaGaleria {
    pub balcao: String,
    pub pos_venda: String,
}

/// O link que abre a galeria **sem senha**.
///
/// ⚠️ **Não é o endereço da galeria.** `/meus-ensaios/{id}` exige sessão, e o
/// cliente não tem conta — ele saiu do estúdio, não do site. Este link é
/// assinado pelo backend, cria a conta no primeiro clique e é o mesmo que vai
/// no e-mail de "fotos prontas".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDeAcesso {
    pub url: String,
    pub validade_em_segundos: i64,
}

/// Como o **site** vê o estado de uma foto.
///
/// 🚨 **São três, e o [`EstadoNoBalcao`] tem dois.** A diferença não é
/// descuido: `Comprada` nasce de um pedido pago no site e **nunca** sai daqui —
/// mas volta de lá, e a grade da sessão precisa saber desenhá-la. Um enum só
/// para os dois sentidos deixaria "comprada" representável na escrita, que é
/// exatamente o que não pode acontecer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDaFotoNoSite {
    LevadaNoBalcao,
    Disponivel,
    /// Pagou depois, pela galeria — há um pedido por trás.
    Comprada,
}

impl EstadoDaFotoNoSite {
    pub fn do_texto(texto: &str) -> Self {
        match texto {
            "levada_no_balcao" => Self::LevadaNoBalcao,
            "comprada" => Self::Comprada,
            // ⚠️ O desconhecido cai em "disponível", e não em pânico: um estado
            // novo no site não pode impedir a grade inteira de desenhar.
            _ => Self::Disponivel,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::LevadaNoBalcao => "levada",
            Self::Disponivel => "à venda",
            Self::Comprada => "comprada",
        }
    }
}

/// Uma foto como ela está no site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoDaGaleria {
    pub id: String,
    /// O nome que o cliente vê.
    pub arquivo: String,
    pub estado: EstadoDaFotoNoSite,
    pub ordem: i32,
    /// Quanto entrou de verdade no balcão, decimal em texto. `None` = a faixa.
    pub preco_negociado: Option<String>,
    pub observacao_da_negociacao: Option<String>,
    /// A retenção apagou os arquivos: a linha ficou para a conta de vendas, e
    /// não há imagem para mostrar.
    pub apagada: bool,
    /// A nota de 1 a 5 do fotógrafo. `None` = **não classificada**.
    ///
    /// 🚨 **Sem ela a barra da grade mente.** Os recortes por situação exigem
    /// classificação (`biblioteca_core::acervo::Filtro`), então uma foto que
    /// chegasse sempre sem nota cairia toda no recorte "Sem nota" e "À venda"
    /// mostraria zero numa galeria cheia.
    pub nota: Option<u8>,
    /// A faixa que **vale** para esta foto: a dela, ou a da galeria.
    pub produto_efetivo: String,
    /// O preço fixado para a compra online, decimal em texto. `None` = a faixa.
    pub preco_de_venda: Option<String>,
    /// O pedido que a comprou, quando houve um.
    pub pedido_id: Option<String>,
    /// Quantas vezes o cliente baixou o original.
    pub downloads: u32,
    /// Se ela já foi revelada no navegador — os 46 ajustes estão gravados.
    pub revelada: bool,
    /// A receita da revelação, como o site a guarda: os ajustes por nome e o
    /// enquadramento com prefixo `corte_`. `None` = nunca revelada.
    ///
    /// 🚨 **Era só o `bool` acima, e a receita ficava pelo caminho.** Uma foto já
    /// revelada abria no app com os sliders no neutro e a miniatura revelada
    /// como se fosse o bruto — e "sincronizar" a partir dela mandava o neutro às
    /// outras. É o `ajustes` que reabre o editor do site com os sliders no
    /// lugar (`completar(foto.ajustes)`), e tem de reabrir o daqui também.
    pub ajustes: Option<serde_json::Value>,
}

/// A sessão aberta — o que a tela de uma sessão precisa saber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaleriaAberta {
    pub galeria: GaleriaDoPainel,
    pub fotos: Vec<FotoDaGaleria>,
    /// Até quando as não adquiridas ficam à venda.
    pub vence_venda: Option<i64>,
    /// Até quando as adquiridas ficam para download.
    pub vence_download: Option<i64>,
}

/// O que muda numa foto que **já está** no site.
///
/// 🔑 **Cada campo tem três estados, e os três importam**: `None` não mexe,
/// `Some(None)` apaga, `Some(Some(v))` grava. É o `Option<Option<_>>` que o
/// `PATCH` do backend fala — sem ele, "não mexer no preço" e "voltar ao preço
/// da faixa" seriam a mesma requisição.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MudancaDaFoto {
    /// `comprada` é recusado pelo site: ele nasce de pedido pago, nunca daqui.
    pub estado: Option<EstadoNoBalcao>,
    /// Quanto entrou de verdade no balcão — cortesia, desconto, site parceiro.
    /// Decimal em texto (`"15.00"`), como o preço do produto.
    ///
    /// ⚠️ **Não é o preço de venda**: não muda o que a compra online cobra.
    pub preco_negociado: Option<Option<String>>,
    /// O porquê do `preco_negociado`, no formato que o painel lê de volta:
    /// `"Cortesia — aniversário"`, `"TchêOfertas — cupom 123"`.
    pub observacao_da_negociacao: Option<Option<String>>,
    /// A nota de 1 a 5. 🚨 **`Some(None)` é recusado pelo site** — ver o topo
    /// do módulo; tirar a nota de uma foto do acervo é removê-la.
    pub nota: Option<Option<i16>>,
}

impl MudancaDaFoto {
    /// Se não há nada a mudar. O site recusa um `PATCH` vazio, e mandar um
    /// seria gastar uma ida à rede para levar um erro de volta.
    pub fn vazia(&self) -> bool {
        self.estado.is_none()
            && self.preco_negociado.is_none()
            && self.observacao_da_negociacao.is_none()
            && self.nota.is_none()
    }
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
    /// Autoriza este computador **pelo navegador**, e devolve a sessão.
    ///
    /// # Por que não há mais e-mail e senha aqui
    ///
    /// Porque a senha do estúdio não precisa passar por um aplicativo desktop
    /// para o aplicativo ter acesso. Quem autentica é o site, no navegador, com
    /// o que o operador já usa lá — inclusive o Google, que pela janela do app
    /// era impossível. O app recebe de volta um código de dois minutos e o troca
    /// por uma sessão de quinze dias, provando com um segredo que nunca saiu
    /// desta máquina (PKCE).
    ///
    /// Bloqueia enquanto o operador decide na outra janela, e desiste sozinha se
    /// ele fechar o navegador e ir embora — ver a implementação para o prazo.
    ///
    /// Recusa do operador (ou de quem não opera o estúdio) é
    /// [`crate::DomainError::AcessoRecusado`], e não erro de infraestrutura: a
    /// tela precisa dizer "não autorizado" e não "sem rede".
    async fn autorizar_pelo_navegador(&self) -> DomainResult<Sessao>;

    /// A sessão de ontem, se o chaveiro ainda a tiver e ela ainda valer.
    ///
    /// `None` é "nunca autorizou aqui" ou "passou dos quinze dias" — os dois
    /// levam ao mesmo lugar, que é autorizar de novo.
    async fn retomar_sessao(&self) -> DomainResult<Option<Sessao>>;

    /// Esquece a sessão: apaga o que está no chaveiro e larga o que está na
    /// memória. Depois disto, entrar é autorizar de novo.
    async fn sair(&self);

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

    /// As galerias que já existem — para subir numa delas em vez de criar uma
    /// por leva de fotos.
    async fn galerias(&self, sessao: &Sessao) -> DomainResult<Vec<GaleriaDoPainel>>;

    /// Muda uma foto que já está no site: o estado do balcão, a negociação, a
    /// nota. Ver [`MudancaDaFoto`] para os três estados de cada campo.
    async fn mudar_foto(
        &self,
        sessao: &Sessao,
        foto_id: &str,
        mudanca: &MudancaDaFoto,
    ) -> DomainResult<()>;

    /// Tira a foto do storage. É o que zerar a classificação faz: ela volta a
    /// ser só local, pronta para subir de novo quando for classificada.
    async fn remover_foto(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<()>;

    /// O link que entra sem senha — o passo "gero o link para o cliente".
    async fn link_da_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> DomainResult<LinkDeAcesso>;

    /// A sessão aberta: a galeria e as fotos que estão nela.
    ///
    /// 🔑 É o "entrar na sessão" — o mesmo gesto que na web abre
    /// `/dashboard/sessoes-fotograficas/{id}`. Sem ele o desktop só sabia
    /// **mandar** fotos para uma galeria, e nunca ver o que já estava nela.
    async fn abrir_galeria(&self, sessao: &Sessao, id: &str) -> DomainResult<GaleriaAberta>;

    /// A miniatura de uma foto do site, para desenhar a grade da sessão.
    async fn miniatura(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>>;

    /// Os bytes da **cópia de trabalho** de uma foto que está no site.
    ///
    /// 🔑 É o passo 11 do fluxo: a Revelação revela as da base local **e** as do
    /// storage. A foto que subiu e cujo arquivo não está nesta máquina — outro
    /// computador do estúdio, cache limpo, foto enviada pelo próprio cliente —
    /// abre por aqui.
    ///
    /// ⚠️ **A cópia de trabalho, e não o original.** São 2048 px, cerca de 1/20
    /// do arquivo: é o que os sliders animam. O original só faz diferença na
    /// exportação, e baixá-lo a cada foto da tira seria trafegar dezenas de MB
    /// para jogar fora antes do primeiro slider se mexer — a mesma decisão que a
    /// web tomou em `revelacao/fonte.ts`.
    async fn copia_de_trabalho(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>>;

    /// Os bytes do **original** — o arquivo cheio que o cliente baixa.
    ///
    /// ⚠️ **Não é a cópia de trabalho.** Ela é 2048 px, e serve para os sliders
    /// andarem; salvar a revelação a partir dela entregaria ao cliente uma foto
    /// de 2048 px no lugar do original — uma perda que ninguém veria acontecer.
    /// É a mesma separação que o editor do site faz entre `fonte` e
    /// `baixarOriginal`.
    async fn original(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>>;

    /// O bilhete que autoriza **substituir o original** desta foto pelo revelado.
    ///
    /// 🔑 É a porta de saída do editor, e ela é a mesma do site: o painel emite
    /// um bilhete de uma hora para a foto, e o envio do JPEG vai por ele. Vale
    /// uma foto só, e o site recusa a que o cliente já comprou — ele pode ter
    /// baixado o original.
    async fn bilhete_de_revelacao(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<String>;

    /// Sobe o JPEG revelado no lugar do original, com o bilhete na mão.
    ///
    /// `ajustes` é o JSON **por nome** — os mesmos campos que o editor do site
    /// grava, mais os `corte_*` do enquadramento. É o que faz a foto voltar a
    /// abrir revelada, aqui e lá.
    ///
    /// ⚠️ **Sem sessão de propósito.** A rota é pública e o bilhete é a
    /// credencial: é assim que o navegador do site sobe, e ter dois caminhos
    /// para o mesmo envio seria a segunda resposta que diverge na primeira
    /// mudança.
    async fn salvar_revelacao(
        &self,
        bilhete: &str,
        jpeg: Vec<u8>,
        ajustes: Value,
    ) -> DomainResult<()>;
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
