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
//! | zerar a nota tira do storage | 🔄 **caiu em 2026-09-20** — ver abaixo |
//! | o cliente paga no balcão | [`PosVendaApi::mudar_foto`] — a negociação e o estado |
//! | gerar o link do cliente | [`PosVendaApi::link_da_galeria`] |
//!
//! 🔄 **A regra mudou em 2026-09-20 — e este parágrafo dizia o contrário.**
//!
//! Aqui se lia: *"`nota: null` é recusado pelo site, e isso não é limitação: é a
//! regra. Foi a classificação que autorizou a foto a subir, então uma foto do
//! acervo sem nota não existe. Zerar a nota é [`PosVendaApi::remover_foto`]"*.
//!
//! O dono trocou a regra (contrato da foto, C20–C22): o ensaio inteiro sobe em
//! segundo plano **durante** a classificação, classificado ou não. `nota: null`
//! passou a ser aceito e significa só "sem curadoria" — não move arquivo nenhum.
//! Quem impede uma foto de subir é a **rejeição** (a tecla `X`), e ela **marca
//! sem apagar**: a foto que já subiu fica onde está.
//!
//! 🚨 **Nenhum gesto de classificação apaga arquivo da nuvem.** Some com isso a
//! janela entre "baixei o bruto de volta" e "o servidor apagou a foto", que
//! custou duas perdas registradas no site. [`PosVendaApi::remover_foto`]
//! continua existindo para o gesto explícito de apagar, que é outra coisa.
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

/// Um estúdio do site — onde a sessão foi feita (`studios`).
///
/// *"Deve ser obrigatório informar o Preço por Foto e o Estúdio."* — dono,
/// 2026-09-13. Só os ativos chegam aqui: estúdio desativado não recebe sessão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estudio {
    pub id: String,
    pub nome: String,
    /// Pode vir vazia: o cadastro do site não a exige.
    pub cidade: String,
    /// A **primeira** foto do cadastro (`studios."fotosUrls"`), que é a mesma
    /// que o site usa como capa do estúdio no agendamento.
    ///
    /// 🔑 **Pública, e por isso basta a URL**: o R2 serve `studios/` sem
    /// autenticação (`/api/v2/public/media/arquivos/`), ao contrário das fotos
    /// de cliente. `None` é cadastro sem foto — e aí quem desenha mostra a
    /// inicial, como a web faz com o gradiente.
    pub foto: Option<String>,
}

/// A galeria do cliente, como o balcão a descreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaGaleria {
    pub titulo: String,
    /// Opcional, como o WhatsApp (dono, 2026-09-13): a sessão nasce sem
    /// contato, e o link e o aviso o pedem no fim. Mal formado o site recusa.
    pub email: Option<String>,
    pub whatsapp: Option<String>,
    pub produto_id: String,
    /// Em qual estúdio a sessão foi feita (`studios.id`). A tela do desktop o
    /// exige (dono, 2026-09-13); a API o aceita ausente por compatibilidade.
    pub estudio_id: Option<String>,
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
/// `Default` existe para os cenários de teste: a galeria tem quinze campos, e
/// exigir os quinze em cada literal faz cada campo novo virar uma varredura por
/// arquivos de teste — que foi como este ficou sem `preset_padrao_id`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// Quem criou a sessão — o e-mail do operador, como no "detalhes" do site.
    pub criada_por: Option<String>,
    /// Segundos desde a época. `None` = não expira.
    pub expira_em: Option<i64>,
    pub fotos: ContagemDeFotos,
    /// `None` na galeria de uma API anterior ao campo — e isso é dito na soma,
    /// em vez de virar zero calado.
    pub totais: Option<TotaisDaGaleria>,
    /// O que o caixa cobrou desta sessão (2026-09-20). `None` = a API não
    /// soube dizer; `Some` com `vendas: 0` = não passou pelo caixa.
    pub caixa: Option<PagoNoCaixa>,
    /// A **receita padrão** da sessão: a predefinição escolhida na etapa 2 do
    /// assistente (`sistema:<chave>` ou o id do banco).
    ///
    /// 🚨 **Ela não é enfeite da lista**: é o que faz a foto importada **dentro**
    /// da sessão nascer com o mesmo visual das que entraram pelo assistente. Sem
    /// ela aqui, a sessão aplicava a receita só às fotos do rascunho, e as
    /// importadas depois ficavam cruas — na web o agendador as pega pelas duas
    /// portas (`receita-padrao/agendador.ts` lê a receita **da galeria**).
    pub preset_padrao_id: Option<String>,
    /// A proporção do corte padrão (`"3:2"`, `"livre"`…), pelo mesmo motivo.
    pub proporcao_padrao: Option<String>,
    /// O estúdio da sessão — o seletor do cabeçalho.
    pub estudio_id: Option<String>,
    /// O que o assistente associou: agendamento, voucher e compra antecipada.
    ///
    /// 🔑 **Ids, e não resumos.** É o que a API devolve sempre; o resumo (nome,
    /// data, total) vem quando ela o tem, e a gaveta do atendimento mostra
    /// "associado" quando só há o id — esconder diria ao operador que **não há**
    /// associação, e o próximo gesto dele seria associar outra por cima.
    pub ensaio_id: Option<String>,
    pub voucher_id: Option<String>,
    pub pedido_id: Option<String>,
    /// Como o cliente conheceu o estúdio (`instagram`, `parceiro`…).
    pub como_conheceu: Option<String>,
    pub como_conheceu_detalhe: Option<String>,
    pub parceiro_id: Option<String>,
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

/// Quanto o **caixa do balcão** cobrou desta sessão — o `PagoNoCaixa` do site.
///
/// ⚠️ **Centavos inteiros aqui, e decimal em texto no vizinho**
/// ([`TotaisDaGaleria`]): é assim que a API devolve os dois, porque o caixa
/// fala centavos em toda parte e os totais vêm de `Decimal`. Converter um dos
/// dois no caminho esconderia a diferença até alguém somá-los.
///
/// 🔑 **`vendas: 0` é um fato**: a sessão não passou pelo caixa, e é dele que
/// sai o "Fechar venda" da lista. Quem não sabe é o `Option` de fora.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PagoNoCaixa {
    pub vendas: u32,
    pub bruto_centavos: i64,
    pub estornado_centavos: i64,
    pub liquido_centavos: i64,
}

/// O link que abre a galeria **sem senha**.
///
/// ⚠️ **Não é o endereço da galeria.** `/meus-ensaios/{id}` exige sessão, e o
/// cliente não tem conta — ele saiu do estúdio, não do site. Este link é
/// assinado pelo backend, cria a conta no primeiro clique e é o mesmo que vai
/// no e-mail de "fotos prontas".
///
/// 🚨 **Ele não expira** desde 2026-09-20 (decisão do dono: *"jamais poderá
/// expirar se o usuário nunca logar-se com o e-mail dele"*). A regra e o preço
/// dela estão no backend, em `application::ports::token::destino_perpetuo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDeAcesso {
    pub url: String,
    /// **`None` = não expira**, e é o que o site sempre devolve desde
    /// 2026-09-20.
    ///
    /// `Option`, e não um número grande: um prazo absurdo continuaria sendo um
    /// prazo, e voltaria a trancar o cliente do lado de fora no dia em que
    /// chegasse. Quem lê isto para mostrar ao operador diz "não expira" no
    /// `None` — nunca um traço mudo.
    pub validade_em_segundos: Option<i64>,
}

/// Como o **site** vê o estado de uma foto.
///
/// 🚨 **São três, e o [`EstadoNoBalcao`] tem dois.** A diferença não é
/// descuido: `Comprada` nasce de um pedido pago no site e **nunca** sai daqui —
/// mas volta de lá, e a grade da sessão precisa saber desenhá-la. Um enum só
/// para os dois sentidos deixaria "comprada" representável na escrita, que é
/// exatamente o que não pode acontecer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EstadoDaFotoNoSite {
    LevadaNoBalcao,
    /// O padrão: é o que o site grava quando o envio não diz o estado, e o que
    /// o `Default` de [`FotoDaGaleria`] usa nos cenários de teste.
    #[default]
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
#[derive(Default, Debug, Clone, PartialEq, Eq)]
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
    /// A foto foi **rejeitada** — a tecla `X` (contrato C21).
    ///
    /// 🚨 **Rejeitar marca, e nunca apaga.** A rejeitada continua inteira no
    /// acervo: o que ela perde é a vista do cliente, o balcão e a venda.
    ///
    /// ⚠️ **Não é `nota == None`**, que é "ainda não passou pela curadoria" e
    /// sobe e vende normalmente (C20, C22).
    pub rejeitada: bool,
    /// A nota de 1 a 5 do fotógrafo. `None` = **não classificada**.
    ///
    /// 🚨 **Sem ela a barra da grade mente.** Os recortes por situação exigem
    /// classificação (`biblioteca_core::acervo::Filtro`), então uma foto que
    /// chegasse sempre sem nota cairia toda no recorte "Sem nota" e "À venda"
    /// mostraria zero numa galeria cheia.
    pub nota: Option<u8>,
    /// A faixa que **vale** para esta foto: a dela, ou a da galeria.
    pub produto_efetivo: String,
    /// A faixa **fixada nesta foto**, quando há uma. `None` = ela segue a
    /// galeria.
    ///
    /// 🔑 **Não é o mesmo que `produto_efetivo`**, e o painel precisa dos dois:
    /// o seletor marca "Padrão da galeria" quando este é `None`, e mostrar o
    /// efetivo ali diria que a foto tem faixa própria quando ela não tem.
    pub produto_id: Option<String>,
    /// O tamanho do arquivo no acervo, em bytes — o "Tamanho" do painel.
    pub tamanho_bytes: Option<u64>,
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
    /// As faixas em uso na galeria, a padrão incluída, **a preço de balcão**
    /// (o cheio). É daqui que o "Preço padrão" dos detalhes sai, como no site:
    /// o catálogo pode não ter uma faixa que a galeria ainda usa.
    pub faixas: Vec<FaixaDaGaleria>,
    /// Os e-mails que saíram para o cliente, mais recente primeiro.
    pub avisos: Vec<AvisoDaGaleria>,
    /// Os resumos do que o assistente associou — o que a gaveta do atendimento
    /// mostra além do id.
    ///
    /// ⚠️ **`None` não é "não há associação"**: o id na galeria é que responde
    /// isso. O resumo pode faltar (a API no meio de um deploy, o voucher
    /// apagado), e aí a gaveta diz "associado" — a mesma regra do site.
    pub resumos: ResumosDoAtendimento,
}

/// Uma faixa de preço em uso na galeria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaixaDaGaleria {
    pub id: String,
    pub nome: String,
    /// O preço **cheio**, decimal em texto (`"25.00"`): no balcão a negociação
    /// parte dele, e o desconto do cadastro é da compra antecipada pelo site.
    pub preco: String,
}

/// Um e-mail que saiu para o cliente, e o que o provedor contou dele depois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvisoDaGaleria {
    /// `fotos_prontas`, `vencimento_venda` ou `vencimento_download`.
    pub tipo: String,
    pub destino: String,
    pub enviado_em: i64,
    pub entregue_em: Option<i64>,
    /// Abertura ou clique — o que a retenção lê para decidir prorrogar.
    pub lido_em: Option<i64>,
}

/// O que a API devolve sobre cada associação, para a gaveta não mostrar só ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumosDoAtendimento {
    /// `(nome, quando, estúdio)` do agendamento.
    pub agendamento: Option<ResumoSimples>,
    /// `(número, nome do cliente, parceiro)` do voucher.
    pub voucher: Option<ResumoSimples>,
    /// `(total, comprador, pago em)` da compra antecipada.
    pub pedido: Option<ResumoSimples>,
    /// `(nome, tipo)` do parceiro.
    pub parceiro: Option<ResumoSimples>,
}

/// Um resumo como a tela o mostra: o título e uma linha de detalhe.
///
/// 🔑 **Texto pronto, e não campos.** Cada resumo da API tem uma forma
/// diferente (data, total, slug), e montá-los aqui deixa a tela com uma regra
/// só: mostrar o que veio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumoSimples {
    pub titulo: String,
    pub detalhe: String,
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
    /// A nota de 1 a 5, ou `Some(None)` para tirá-la.
    ///
    /// 🔄 **`Some(None)` passou a ser aceito em 2026-09-20** (contrato C22).
    /// Aqui se lia que ele era recusado, porque tirar a nota significava remover
    /// a foto do acervo. Não significa mais: a nota virou curadoria, e tirá-la
    /// não move arquivo nenhum.
    pub nota: Option<Option<i16>>,
    /// A **rejeição** — a tecla `X` (contrato C21). `Some(true)` rejeita,
    /// `Some(false)` desfaz, `None` não mexe.
    ///
    /// 🚨 **Rejeitar marca, e nunca apaga.** A rejeitada não sobe, não aparece
    /// ao cliente e não é comprável; a que já tiver subido fica exatamente onde
    /// está. É o gesto que substituiu a desclassificação destrutiva.
    ///
    /// ⚠️ **Não é "sem nota".** Sem nota é foto que ainda não passou pela
    /// curadoria, e que sobe e vende normalmente.
    pub rejeitada: Option<bool>,
    /// A **faixa** desta foto — o "tipo de ensaio", que dá o preço. `Some(None)`
    /// devolve a foto ao produto padrão da galeria.
    ///
    /// 🔑 É o `produto_id` do `PATCH /pos-venda/fotos/{id}`, o mesmo campo que o
    /// painel do site muda em "Faixa": a sessão mista tem fotos de faixas
    /// diferentes, e corrigir uma leva inteira foto a foto é o que o balcão faz.
    pub produto_id: Option<Option<String>>,
    /// **O preço** desta foto na galeria do cliente e no pedido, quando difere
    /// da faixa. Decimal em texto (`"19.90"`); `Some(None)` volta ao da faixa.
    ///
    /// ⚠️ **Não confundir com `preco_negociado`**: aquele é registro do balcão e
    /// não entra na compra online; este é o que o cliente paga. Zero é recusado
    /// pelo site (`400`) — zero é cortesia, e cortesia é negociação.
    pub preco_de_venda: Option<Option<String>>,
}

impl MudancaDaFoto {
    /// Se não há nada a mudar. O site recusa um `PATCH` vazio, e mandar um
    /// seria gastar uma ida à rede para levar um erro de volta.
    pub fn vazia(&self) -> bool {
        self.estado.is_none()
            && self.preco_negociado.is_none()
            && self.observacao_da_negociacao.is_none()
            && self.nota.is_none()
            && self.produto_id.is_none()
            && self.preco_de_venda.is_none()
            // 🚨 Sem esta linha, um `X` sozinho seria "mudança vazia" e nunca
            // chegaria à rede — o gesto morreria calado no cliente HTTP.
            && self.rejeitada.is_none()
    }
}

/// O que muda nos dados do cliente de uma sessão que já existe — o
/// `PATCH /galerias/{id}` do site.
///
/// *"Dentro da sessão precisa ser possível mudar o Título, email e o
/// whatsapp."* — dono, 2026-09-13.
///
/// O mesmo `Option<Option<_>>` de [`MudancaDaFoto`]: `None` não mexe,
/// `Some(None)` apaga. `titulo` é `Option` simples porque não se apaga (o site
/// responde `400`). E-mail e WhatsApp **se apagam, inclusive o último**: o
/// contato é exigido no fim da sessão, não na edição.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MudancaDaGaleria {
    pub titulo: Option<String>,
    pub email: Option<Option<String>>,
    pub whatsapp: Option<Option<String>>,
    /// O estúdio da sessão. `Some(None)` a deixa sem estúdio.
    pub estudio_id: Option<Option<String>>,
    /// As associações do atendimento — o que a gaveta corrige sem recriar a
    /// sessão (site: `atendimento-da-sessao.tsx`).
    pub ensaio_id: Option<Option<String>>,
    pub voucher_id: Option<Option<String>>,
    pub pedido_id: Option<Option<String>>,
    /// 🚨 **Os três do "como conheceu" andam juntos.** O site recusa parceiro
    /// sem `como_conheceu = parceiro` (`400`), e a resposta trocada sem limpar o
    /// parceiro é exatamente esse caso.
    pub como_conheceu: Option<Option<String>>,
    pub como_conheceu_detalhe: Option<Option<String>>,
    pub parceiro_id: Option<Option<String>>,
    /// A receita padrão da sessão — muda o que as próximas fotos recebem.
    pub preset_padrao_id: Option<Option<String>>,
    pub proporcao_padrao: Option<Option<String>>,
}

impl MudancaDaGaleria {
    /// Nada a mudar — não se gasta uma ida à rede.
    pub fn vazia(&self) -> bool {
        self.titulo.is_none()
            && self.email.is_none()
            && self.whatsapp.is_none()
            && self.estudio_id.is_none()
            && self.ensaio_id.is_none()
            && self.voucher_id.is_none()
            && self.pedido_id.is_none()
            && self.como_conheceu.is_none()
            && self.como_conheceu_detalhe.is_none()
            && self.parceiro_id.is_none()
            && self.preset_padrao_id.is_none()
            && self.proporcao_padrao.is_none()
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
    /// A mesma foto **sem revelação nenhuma**, quando `jpeg` já vem tratado.
    ///
    /// 🚨 **É o que torna a revelação reversível no site.** O envio do desktop
    /// renderiza com os ajustes do catálogo: o que sobe já é a foto tratada, e
    /// sem esta segunda cópia o servidor nunca teve o arquivo como ele entrou.
    /// "Zerar tudo" na web não teria para onde voltar — o mesmo buraco que a
    /// área temporária do navegador tinha, e que o dono encontrou em
    /// 11/set/2026.
    ///
    /// `None` quando a foto está no neutro: aí `jpeg` **é** o bruto dela, e
    /// mandar duas cópias iguais é banda e armazenamento por nada.
    pub bruto: Option<Vec<u8>>,
    /// A receita com que `jpeg` foi revelado a partir do `bruto`, no formato do
    /// site (sobe em JSON, para a coluna `ajustes`). Só vai com o bruto: sem ele, `jpeg` **é** a foto
    /// como entrou. Ver `ImageExporter::receita_para_o_site`.
    pub ajustes: Option<serde_json::Value>,
    pub estado: EstadoNoBalcao,
    /// A **faixa** desta foto, quando a leva escolheu uma. `None` = a da
    /// galeria.
    ///
    /// 🔑 É a primeira das duas escolhas antes dos arquivos, no site
    /// (`envio.tsx`): a sessão mista sobe a mãe sozinha numa faixa e a família
    /// em outra, e sem este campo a leva inteira nascia no padrão da galeria.
    pub produto_id: Option<String>,
    pub ordem: u32,
    /// A nota de 1 a 5. **O site recusa envio sem ela.**
    ///
    /// 🚨 *"a foto sobe classificada: informe a nota de 1 a 5"* — é a regra do
    /// dono de 2026-09-05, e o backend a aplica na primeira linha do envio. Este
    /// campo faltava, e com ele faltando **o passo 3 do desktop devolvia `400`
    /// em toda foto**: classificar não subia nada, e o erro falava de uma nota
    /// que o app tinha e não mandava.
    pub nota: Option<u8>,
    /// A chave de idempotência desta foto — o id dela no catálogo local.
    ///
    /// 🔑 **É o que faz reenviar ser seguro.** A mesma chave na mesma galeria
    /// devolve a foto que já está lá, em vez de uma segunda cópia na galeria de
    /// quem pagou. Ver `docs/11-OFFLINE-E-SINCRONIZACAO.md`.
    pub chave_do_cliente: Option<String>,
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

    /// Um pedido JSON em nome da conta (`caminho` relativo a `/api/v2`), com a
    /// resposta crua em JSON.
    ///
    /// 🔑 É a porta das telas do painel que só leem e gravam JSON e não têm
    /// regra própria no app: a conta (`/auth/me`), o caixa e a retenção. Elas
    /// são as mesmas do site, que também só repassa o JSON (2026-09-17).
    ///
    /// Uma resposta fora de `2xx` vira erro, com a frase do envelope do site.
    async fn pedir_json(
        &self,
        sessao: &Sessao,
        metodo: &str,
        caminho: &str,
        corpo: Option<serde_json::Value>,
    ) -> DomainResult<serde_json::Value> {
        let _ = (sessao, metodo, caminho, corpo);
        Err(crate::DomainError::InvalidOperation(
            "esta API não atende pedido JSON".into(),
        ))
    }

    /// O catálogo administrativo — inclusive inativos.
    async fn produtos(&self, sessao: &Sessao) -> DomainResult<Vec<Produto>>;

    /// Os estúdios **ativos** — onde a sessão pode ser feita.
    async fn estudios(&self, sessao: &Sessao) -> DomainResult<Vec<Estudio>>;

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
    ///
    /// 🔚 Galeria sem e-mail volta como
    /// [`crate::DomainError::FaltaEmail`] (`422`).
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
    ///
    /// 🔚 Galeria sem e-mail volta como
    /// [`crate::DomainError::FaltaEmail`] (`422`).
    async fn link_da_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> DomainResult<LinkDeAcesso>;

    /// Muda título, e-mail ou WhatsApp da sessão — só o que veio. Ver
    /// [`MudancaDaGaleria`].
    async fn atualizar_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        mudanca: &MudancaDaGaleria,
    ) -> DomainResult<()>;

    /// A sessão aberta: a galeria e as fotos que estão nela.
    ///
    /// 🔑 É o "entrar na sessão" — o mesmo gesto que na web abre
    /// `/dashboard/sessoes-fotograficas/{id}`. Sem ele o desktop só sabia
    /// **mandar** fotos para uma galeria, e nunca ver o que já estava nela.
    async fn abrir_galeria(&self, sessao: &Sessao, id: &str) -> DomainResult<GaleriaAberta>;

    /// A miniatura de uma foto do site, para desenhar a grade da sessão.
    async fn miniatura(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>>;

    /// Os bytes de um arquivo **público** do site — a capa de um estúdio.
    ///
    /// 🔑 **Sem sessão, e por isso fora do resto**: o R2 serve `studios/` e
    /// `blog/` sem autenticação, e é a mesma URL que o site usa no
    /// agendamento. O padrão recusa: quem não sabe baixar deixa a tela com a
    /// inicial do nome, que é o desenho de "cadastro sem foto".
    async fn arquivo_publico(&self, _url: &str) -> DomainResult<Vec<u8>> {
        Err(crate::DomainError::InvalidOperation(
            "esta API não baixa arquivo público".into(),
        ))
    }

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

    /// Desfaz a revelação: o **bruto** guardado no site volta a ser o original.
    ///
    /// 🔑 É o "Zerar tudo" salvo, e ele não sobe arquivo nenhum. Revelar o
    /// bruto com os ajustes neutros e substituir daria ao cliente uma geração a
    /// mais de JPEG no lugar do arquivo que ele deveria receber — e ainda
    /// pagaria o download do original e o upload do resultado para chegar lá.
    ///
    /// Vai com sessão, e não com bilhete: o bilhete autoriza *substituir*, e
    /// aqui nada é enviado. Foto que nunca foi revelada responde certo — zerar
    /// o que já estava zerado não é erro.
    async fn restaurar_original(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<()>;
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
