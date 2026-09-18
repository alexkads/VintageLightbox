//! A lista de sessões fotográficas, no mesmo desenho da rota do site.
//!
//! # O que é daqui e o que é do core
//!
//! Daqui é o desenho: a barra, os cartões de total, a tabela, o formulário. As
//! **contas** — quem está em que situação, o que a busca acha, quanto o recorte
//! somou — são de [`biblioteca_core::sessoes`], porque são as mesmas do site.
//!
//! # Duas armadilhas que a tela do site já evitou, e que valem aqui
//!
//! 1. 💰 **Os totais são do recorte visível, e a tela diz isso.** Somar o
//!    conjunto e mostrar ao lado de uma lista filtrada é um número que responde
//!    outra pergunta.
//! 2. ⚠️ **A galeria sem totais é contada à parte.** Ela vem de uma API anterior
//!    ao campo; tratada como zero, faria o rodapé anunciar menos do que o real
//!    sem nada dizendo por quê.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::dados_do_cliente;
use biblioteca_core::dinheiro;
use biblioteca_core::sessoes::{
    self, ContagemDeFotos, Criterio, FaixaDeDatas, SessaoFotografica, Situacao, Totais,
};
use domain::services::pos_venda::{Estudio, GaleriaDoPainel, NovaGaleria, Produto, Sessao};

use super::periodo;
use gpui::{div, prelude::*, px, App, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::pos_venda::porta::{Publicador, Recado};

/// De quanto em quanto a tela pergunta se o site respondeu.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// Hoje, **no fuso do estúdio** — a data que o balcão chama de hoje.
///
/// 🔑 O mesmo fuso do resto do app (`caixa::dados::no_estudio`): uma sessão
/// criada às 21h de Brasília é de hoje aqui, e do dia seguinte em UTC.
fn hoje_no_estudio() -> chrono::NaiveDate {
    chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::west_opt(3 * 3600).expect("o fuso do estúdio"))
        .date_naive()
}

/// O lado da capa do estúdio no diálogo, em pontos — o avatar do shadcn.
const LADO_DA_CAPA: u32 = 48;

/// A sessão fotográfica escolhida para receber as fotos.
///
/// Quem escuta é a raiz: é ela que leva o id para o pós-venda e — quando o
/// passo 3 do fluxo entrar — para a classificação, que sobe a foto.
pub struct Escolhida(pub String);

impl EventEmitter<Escolhida> for Sessoes {}

/// "Nova sessão": a raiz abre o assistente de sete etapas, como a rota
/// `/nova` do site.
pub struct NovaPedida;

impl EventEmitter<NovaPedida> for Sessoes {}

pub struct Sessoes {
    publicador: Arc<dyn Publicador>,
    /// `None` antes de a porta responder: a tela existe, e diz que precisa de
    /// conta.
    sessao: Option<Sessao>,
    galerias: Vec<GaleriaDoPainel>,
    produtos: Vec<Produto>,
    /// Os estúdios ativos — a escolha obrigatória ao abrir sessão.
    estudios: Vec<Estudio>,
    /// Onde o último estúdio escolhido fica lembrado nesta máquina.
    lembranca: PathBuf,
    /// 📅 O calendário do período está aberto? (`Some` = o popover na tela.)
    ///
    /// 🚨 **É o nosso, e não o do `gpui-component`** — o de lá não fala
    /// português e o dicionário dele é compilado dentro do crate. Ver
    /// `sessoes::periodo`.
    calendario: Option<periodo::EstadoDoPeriodo>,
    /// O período escolhido. `None` é "todo o período" — o arquivo inteiro.
    faixa: Option<FaixaDeDatas>,
    /// As capas dos estúdios, já decodificadas — por id.
    ///
    /// 🔑 **A tela guarda a imagem pronta**, e não os bytes: decodificar a cada
    /// quadro é o defeito que a grade da sessão já pagou duas vezes
    /// (`docs/08-CACHE-ARCHITECTURE.md`).
    capas: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
    /// Os estúdios cuja capa já foi pedida — uma vez cada.
    capas_pedidas: std::collections::HashSet<String>,
    /// A pergunta da entrada está na tela? (`Some` enquanto ela espera resposta.)
    ///
    /// 🚨 **A rota das sessões exige um estúdio antes de qualquer coisa** (dono,
    /// 2026-09-18), como no site: sem a resposta, cada tela adiante adivinha —
    /// e adivinhava pela última sessão criada, que é a de qualquer balcão.
    escolhendo_estudio: bool,
    /// Qual sessão está aberta para receber fotos.
    aberta: Option<String>,
    busca: gpui::Entity<InputState>,
    situacao: Option<Situacao>,
    carregando: bool,
    erro: Option<SharedString>,
    /// O formulário de abrir sessão, quando aparece.
    nova: Option<Nova>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

/// Os campos de abrir uma sessão nova.
struct Nova {
    titulo: gpui::Entity<InputState>,
    email: gpui::Entity<InputState>,
    whatsapp: gpui::Entity<InputState>,
    produto_id: Option<String>,
    estudio_id: Option<String>,
    enviando: bool,
}

/// 🎯 O que esta máquina lembra de uma sessão nova para a próxima.
///
/// *"Grave o último preset selecionado e o corte utilizado e o estúdio."* —
/// dono, 2026-09-13. A criação do desktop não escolhe preset nem corte (eles
/// nascem na revelação), então aqui fica **só o estúdio**: um balcão atende
/// no mesmo estúdio o dia inteiro.
///
/// Mesmo molde de `altura_da_tira` e `pos_venda::config`: um arquivo ao lado do
/// catálogo; ler nunca derruba, gravar que falha só não lembra.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Lembranca {
    #[serde(default)]
    estudio_id: Option<String>,
}

#[cfg(not(test))]
fn caminho_da_lembranca() -> PathBuf {
    AppPaths::catalog_root().join("sessao-nova.json")
}

/// 🚨 Nos testes, um arquivo temporário **por tela**: gravar no catálogo do
/// fotógrafo durante o `cargo test` já aconteceu neste repositório, e um
/// arquivo por processo faria os testes em paralelo lembrarem uns dos outros.
#[cfg(test)]
fn caminho_da_lembranca() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMA: AtomicUsize = AtomicUsize::new(0);
    let _ = AppPaths::catalog_root;
    std::env::temp_dir().join(format!(
        "vlb-sessao-nova-teste-{}-{}.json",
        std::process::id(),
        PROXIMA.fetch_add(1, Ordering::SeqCst)
    ))
}

fn estudio_lembrado(caminho: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str::<Lembranca>(&texto)
        .ok()?
        .estudio_id
        .filter(|id| !id.trim().is_empty())
}

/// O estúdio que **esta máquina** escolheu na entrada das sessões, como está
/// guardado — sem conferir contra a lista de agora.
///
/// 🔑 **Quem confere é quem tem a lista** (a tela das sessões, o caixa): aqui
/// só se lê o arquivo. É por esta função que o caixa sabe em que estúdio abrir
/// quando a sessão não diz (dono, 2026-09-18) — o equivalente ao cookie que a
/// web grava para o servidor ler.
pub fn estudio_de_trabalho_guardado() -> Option<String> {
    estudio_lembrado(&caminho_da_lembranca())
}

fn lembrar_estudio(caminho: &Path, estudio_id: &str) {
    let lembranca = Lembranca {
        estudio_id: Some(estudio_id.to_string()),
    };
    let Ok(texto) = serde_json::to_string_pretty(&lembranca) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    let _ = std::fs::write(caminho, texto);
}

impl Sessoes {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nome, e-mail ou WhatsApp…"));
        // 📅 **A lista abre em hoje** (dono, 2026-09-18), como no site: o balcão
        // trabalha o dia, e a lista inteira é o arquivo.
        let hoje = hoje_no_estudio();
        Self {
            publicador,
            calendario: None,
            faixa: Some(FaixaDeDatas::no_dia(hoje.format("%Y-%m-%d").to_string())),
            sessao: None,
            galerias: Vec::new(),
            produtos: Vec::new(),
            estudios: Vec::new(),
            lembranca: caminho_da_lembranca(),
            escolhendo_estudio: false,
            capas: std::collections::HashMap::new(),
            capas_pedidas: std::collections::HashSet::new(),
            aberta: None,
            busca,
            situacao: None,
            carregando: false,
            erro: None,
            nova: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    /// A sessão do site chegou da porta do app: dá para listar.
    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        self.sessao = Some(sessao);
        self.recarregar(cx);
    }

    pub fn recarregar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.carregando = true;
        self.erro = None;
        self.publicador
            .galerias(sessao.clone(), self.recados.0.clone());
        self.publicador
            .estudios(sessao.clone(), self.recados.0.clone());
        self.publicador.produtos(sessao, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// As sessões como o core as entende — a tradução acontece num lugar só.
    /// As galerias que a lista mostra — **as do estúdio desta máquina**.
    ///
    /// 🚨 **O filtro é o estúdio escolhido na entrada** (dono, 18/set/2026: *"as
    /// sessões do grid precisam ser filtradas de acordo com o estúdio
    /// selecionado"*). Cada balcão vê o que é dele: a lista de Canela com as
    /// sessões de Gramado no meio é onde o operador clica na galeria errada.
    ///
    /// ⚠️ **A sessão sem estúdio aparece em todas.** São as de antes de o campo
    /// existir e as que nasceram por outro caminho; escondê-las de todo mundo
    /// seria perdê-las de vista — e é justamente nelas que o seletor do
    /// cabeçalho da galeria serve para dizer de quem são.
    fn para_o_core(&self) -> Vec<SessaoFotografica> {
        let meu = self.estudio_de_trabalho().map(|e| e.id.clone());
        self.galerias
            .iter()
            .filter(|g| match (&meu, &g.estudio_id) {
                (Some(meu), Some(dela)) => meu == dela,
                _ => true,
            })
            .map(|g| SessaoFotografica {
                id: g.id.clone(),
                titulo: g.titulo.clone(),
                email: g.email.clone(),
                whatsapp: g.whatsapp.clone(),
                criada_em_iso: g.criada_em_iso.clone(),
                expira_em: g.expira_em,
                user_id: g.user_id.clone(),
                fotos: ContagemDeFotos {
                    levadas_no_balcao: g.fotos.levadas_no_balcao,
                    disponiveis: g.fotos.disponiveis,
                    compradas: g.fotos.compradas,
                    apagadas: g.fotos.apagadas,
                },
                // 🔑 O decimal em texto do site vira centavos aqui, com o mesmo
                // leitor que o campo de preço usa. `None` continua `None`: a
                // galeria sem totais tem de chegar ao rodapé como "não sei".
                totais: g.totais.as_ref().map(|t| Totais {
                    balcao: dinheiro::ler_campo(&t.balcao).unwrap_or(0),
                    pos_venda: dinheiro::ler_campo(&t.pos_venda).unwrap_or(0),
                }),
            })
            .collect()
    }

    fn criterio(&self, cx: &Context<Self>) -> Criterio {
        Criterio {
            busca: self.busca.read(cx).value().to_string(),
            situacao: self.situacao,
            periodo: self.faixa.clone(),
        }
    }

    /// Se há busca ou filtro — é o que faz os totais falarem em "recorte".
    pub fn filtrando(&self, cx: &Context<Self>) -> bool {
        // 📅 **O período conta como recorte**: a lista abre em hoje, e somar o
        // arquivo inteiro debaixo de uma lista de um dia diria um número que a
        // tela não reproduz.
        !self.busca.read(cx).value().trim().is_empty()
            || self.situacao.is_some()
            || self.faixa.is_some()
    }

    /// Abre ou fecha o calendário do período.
    pub fn alternar_calendario(&mut self, cx: &mut Context<Self>) {
        self.calendario = match self.calendario {
            Some(_) => None,
            None => Some(periodo::EstadoDoPeriodo::no_mes_de(
                self.faixa
                    .as_ref()
                    .and_then(|f| chrono::NaiveDate::parse_from_str(f.de.as_str(), "%Y-%m-%d").ok())
                    .unwrap_or_else(hoje_no_estudio),
            )),
        };
        cx.notify();
    }

    /// Um atalho do calendário — `None` é "Tudo".
    pub fn escolher_periodo(&mut self, faixa: Option<FaixaDeDatas>, cx: &mut Context<Self>) {
        self.faixa = faixa;
        self.calendario = None;
        cx.notify();
    }

    /// O clique num dia do calendário.
    ///
    /// ⚠️ **O primeiro clique já filtra** (um dia só): esperar o segundo
    /// deixaria a lista parada no meio do gesto. O segundo fecha o intervalo e
    /// o calendário.
    pub fn clicar_no_dia(&mut self, dia: chrono::NaiveDate, cx: &mut Context<Self>) {
        let Some(estado) = self.calendario.as_mut() else {
            return;
        };
        let faixa = periodo::clicar_no_dia(estado, dia);
        let fechou = estado.comecando.is_none();
        self.faixa = Some(faixa);
        if fechou {
            self.calendario = None;
        }
        cx.notify();
    }

    /// As setas do calendário.
    pub fn andar_no_mes(&mut self, passo: i32, cx: &mut Context<Self>) {
        if let Some(estado) = self.calendario.as_mut() {
            estado.andar_mes(passo);
            cx.notify();
        }
    }

    /// 🧪 A faixa de datas que a lista está mostrando.
    #[cfg(test)]
    pub(crate) fn faixa_para_teste(&self) -> Option<FaixaDeDatas> {
        self.faixa.clone()
    }

    /// 🧪 Troca o período sem passar pelo calendário — `None` é "todo o
    /// período", o "Tudo" do seletor.
    #[cfg(test)]
    pub(crate) fn escolher_periodo_para_teste(
        &mut self,
        faixa: Option<FaixaDeDatas>,
        cx: &mut Context<Self>,
    ) {
        self.faixa = faixa;
        cx.notify();
    }

    pub fn limpar_filtros(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        self.situacao = None;
        // 📅 **O "limpar" volta ao padrão, e não ao arquivo inteiro**: a lista
        // abre em hoje, e é para hoje que ela volta. Ver o arquivo é uma
        // escolha, e ela tem o "Tudo" do seletor.
        let hoje = hoje_no_estudio();
        self.faixa = Some(FaixaDeDatas::no_dia(hoje.format("%Y-%m-%d").to_string()));
        self.calendario = None;
        cx.notify();
    }

    pub fn filtrar_por(&mut self, situacao: Option<Situacao>, cx: &mut Context<Self>) {
        self.situacao = situacao;
        cx.notify();
    }

    /// Abre a sessão para receber fotos. É o "editar" desta tela: o site edita
    /// a galeria entrando nela, e aqui entrar é escolhê-la como destino.
    ///
    /// ⚠️ **Título e contato não se editam por aqui**: editam-se **dentro** da
    /// sessão (`Detalhe`, "Editar"), pelo `PATCH /galerias/{id}` — o mesmo gesto
    /// da web (dono, 2026-09-13).
    pub fn abrir(&mut self, id: String, cx: &mut Context<Self>) {
        self.aberta = Some(id.clone());
        cx.emit(Escolhida(id));
        cx.notify();
    }

    pub fn aberta(&self) -> Option<&str> {
        self.aberta.as_deref()
    }

    pub fn quantas(&self) -> usize {
        self.galerias.len()
    }

    /// O id da N-ésima sessão, na ordem em que o site a lista (de 1).
    pub fn id_na_posicao(&self, posicao: usize) -> Option<String> {
        self.galerias
            .get(posicao.checked_sub(1)?)
            .map(|g| g.id.clone())
    }

    /// Abre o formulário de sessão nova, com o produto da última já escolhido.
    ///
    /// 🔑 **A sugestão é o produto da sessão mais recente**, como no site: um
    /// estúdio cobra a mesma faixa a semana inteira, e a alternativa seria
    /// escolher de novo a cada cliente.
    ///
    /// 🎯 **O estúdio sugerido é o último escolhido nesta máquina** — e só se
    /// ele ainda estiver entre os ativos; senão fica sem escolha, porque
    /// escolher é obrigatório (dono, 2026-09-13).
    pub fn comecar_nova(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sugerido = self
            .galerias
            .first()
            .map(|g| g.produto_id.clone())
            .or_else(|| self.produtos.first().map(|p| p.id.clone()));
        let estudio = estudio_lembrado(&self.lembranca)
            .filter(|id| self.estudios.iter().any(|e| &e.id == id));

        self.nova = Some(Nova {
            titulo: cx.new(|cx| InputState::new(window, cx).placeholder("Ensaio da Maria")),
            email: cx.new(|cx| InputState::new(window, cx).placeholder("cliente@exemplo.com")),
            whatsapp: cx.new(|cx| InputState::new(window, cx).placeholder("(47) 99999-8888")),
            produto_id: sugerido,
            estudio_id: estudio,
            enviando: false,
        });
        cx.notify();
    }

    /// O preço por foto da sessão nova — a ficha clicada.
    pub fn escolher_faixa(&mut self, produto_id: String, cx: &mut Context<Self>) {
        if let Some(nova) = self.nova.as_mut() {
            nova.produto_id = Some(produto_id);
            cx.notify();
        }
    }

    /// O estúdio da sessão nova — a ficha clicada.
    pub fn escolher_estudio(&mut self, estudio_id: String, cx: &mut Context<Self>) {
        if let Some(nova) = self.nova.as_mut() {
            nova.estudio_id = Some(estudio_id);
            cx.notify();
        }
    }

    pub fn cancelar_nova(&mut self, cx: &mut Context<Self>) {
        self.nova = None;
        cx.notify();
    }

    pub fn abrindo_nova(&self) -> bool {
        self.nova.is_some()
    }

    /// Manda abrir a sessão no site.
    ///
    /// 🔚 **Só o título é obrigatório** (dono, 2026-09-13: *"Não tem que
    /// obrigar o email e whatsapp para criar a sessão. Essas informações são
    /// obrigatórias no final da sessão."*). O contato é pedido pelo "Copiar
    /// link" e pelo "Avisar", dentro da sessão. O e-mail **preenchido** pela
    /// metade continua recusado — a mesma conferência do site
    /// (`biblioteca_core::dados_do_cliente`).
    pub fn criar(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(nova)) = (self.sessao.clone(), self.nova.as_mut()) else {
            return;
        };
        if nova.enviando {
            return;
        }

        let titulo = nova.titulo.read(cx).value().trim().to_string();
        let email = nao_vazio(&nova.email.read(cx).value());
        let whatsapp = nao_vazio(&nova.whatsapp.read(cx).value());
        // 🎯 Preço por foto e estúdio são obrigatórios (dono, 2026-09-13).
        let Some(produto_id) = nova.produto_id.clone() else {
            self.erro = Some("escolha o preço por foto da sessão".into());
            cx.notify();
            return;
        };
        let Some(estudio_id) = nova.estudio_id.clone() else {
            self.erro = Some("escolha o estúdio da sessão".into());
            cx.notify();
            return;
        };
        if titulo.is_empty() {
            self.erro = Some("dê um título à sessão — o contato pode ficar para o fim".into());
            cx.notify();
            return;
        }
        if email
            .as_deref()
            .is_some_and(|e| !dados_do_cliente::email_plausivel(e))
        {
            self.erro = Some("o e-mail do cliente não parece completo".into());
            cx.notify();
            return;
        }

        nova.enviando = true;
        self.erro = None;
        self.publicador.criar_galeria(
            sessao,
            NovaGaleria {
                titulo,
                email,
                whatsapp,
                produto_id,
                estudio_id: Some(estudio_id),
            },
            self.recados.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// Preenche o formulário de sessão nova — só para os testes da raiz, que
    /// não alcançam os campos privados.
    #[cfg(test)]
    pub fn preencher_para_teste(
        &mut self,
        titulo: &str,
        email: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A faixa sugerida vem da sessão mais recente; sem nenhuma, do catálogo.
        if let Some(nova) = self.nova.as_mut() {
            if nova.produto_id.is_none() {
                nova.produto_id = Some("p1".into());
            }
            if nova.estudio_id.is_none() {
                nova.estudio_id = Some("s1".into());
            }
        }
        let Some(nova) = self.nova.as_ref() else {
            return;
        };
        let (titulo, email) = (titulo.to_string(), email.to_string());
        nova.titulo
            .update(cx, |estado, cx| estado.set_value(titulo, window, cx));
        nova.email
            .update(cx, |estado, cx| estado.set_value(email, window, cx));
    }

    /// 🧪 Digita na busca, como quem escreve no campo.
    #[cfg(test)]
    pub(crate) fn buscar(&mut self, texto: &str, window: &mut Window, cx: &mut Context<Self>) {
        let texto = texto.to_string();
        self.busca
            .update(cx, |estado, cx| estado.set_value(texto, window, cx));
        cx.notify();
    }

    /// 🧪 Os títulos que a lista mostra agora, com a busca e o recorte — a
    /// mesma conta do `render`.
    #[cfg(test)]
    pub(crate) fn titulos_visiveis(&self, cx: &Context<Self>) -> Vec<String> {
        sessoes::filtrar(&self.para_o_core(), &self.criterio(cx), agora_em_segundos())
            .iter()
            .map(|s| s.titulo.clone())
            .collect()
    }

    /// 🧪 A frase de erro da tela.
    #[cfg(test)]
    pub(crate) fn erro_para_teste(&self) -> Option<String> {
        self.erro.as_ref().map(|e| e.to_string())
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Galerias(lista) => {
                    self.carregando = false;
                    self.galerias = lista;
                }
                Recado::Produtos(lista) => self.produtos = lista,
                Recado::Estudios(lista) => {
                    self.estudios = lista;
                    // 🖼️ **A capa do cadastro** (dono, 18/set/2026): a mesma
                    // foto que o site mostra no agendamento, para o operador
                    // reconhecer o estúdio pela imagem e não pelo nome.
                    self.pedir_as_capas();
                    // 🏢 **A lista chegou: ou a lembrança vale, ou a pergunta
                    // aparece.** É o `precisaEscolher` do site, e o mesmo
                    // cuidado: sem estúdio cadastrado não se pergunta nada — um
                    // diálogo sem opção e sem saída é pior que a ausência dele.
                    if self.estudio_de_trabalho().is_none() && !self.estudios.is_empty() {
                        self.escolhendo_estudio = true;
                    }
                }
                Recado::CapaDoEstudio { estudio_id, bytes } => {
                    // ⚠️ Imagem que não decodifica é capa que não aparece: o
                    // estúdio fica com a inicial, como o cadastro sem foto.
                    if let Ok(imagem) = image::load_from_memory(&bytes) {
                        let miniatura = imagem.thumbnail(LADO_DA_CAPA * 2, LADO_DA_CAPA * 2);
                        self.capas
                            .insert(estudio_id, crate::imagem::para_gpui(miniatura));
                    }
                }
                Recado::Criada(galeria) => {
                    // 🎯 O estúdio fica lembrado **depois** de a sessão existir:
                    // lembrar uma escolha que o site recusou sugeriria o erro.
                    if let Some(estudio) = self.nova.as_ref().and_then(|n| n.estudio_id.clone()) {
                        lembrar_estudio(&self.lembranca, &estudio);
                    }
                    self.nova = None;
                    // A sessão recém-aberta já fica escolhida: quem a criou vai
                    // subir foto nela agora, e não daqui a três telas.
                    self.aberta = Some(galeria.id.clone());
                    cx.emit(Escolhida(galeria.id));
                    self.recarregar(cx);
                }
                Recado::Falhou(erro) => {
                    self.carregando = false;
                    if let Some(nova) = self.nova.as_mut() {
                        nova.enviando = false;
                    }
                    self.erro = Some(erro.into());
                }
                // Entrar, publicar e link são de outras telas.
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.carregando || self.nova.as_ref().is_some_and(|n| n.enviando);
        if !continua {
            self.colhendo = false;
        }
        continua
    }
}

fn nao_vazio(texto: &str) -> Option<String> {
    let limpo = texto.trim();
    (!limpo.is_empty()).then(|| limpo.to_string())
}

impl Render for Sessoes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let agora = agora_em_segundos();
        let todas = self.para_o_core();
        let contagens = sessoes::contar_por_situacao(&todas, agora);
        let visiveis = sessoes::filtrar(&todas, &self.criterio(cx), agora);
        let soma = sessoes::somar_totais(&visiveis);
        let filtrando = self.filtrando(cx);
        let apagado = cx.theme().muted_foreground;
        let rodape = format!("{} de {} sessões", visiveis.len(), todas.len());

        // 📐 O esqueleto da página do site (`Pagina alturaCheia`): 24 px de
        // respiro, e só a tabela rola.
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(16.))
            .size_full()
            .p(px(24.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.barra(&contagens, cx))
            .child(self.indicadores(&soma, visiveis.len(), filtrando, cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(crate::estilo::aviso(erro, true, cx))
            })
            .when(self.sessao.is_none(), |tela| {
                tela.child(crate::estilo::aviso(
                    "Esta tela precisa da conta do site. Feche e abra o app para entrar.",
                    false,
                    cx,
                ))
            })
            .children(self.formulario(cx))
            .child(self.tabela(&visiveis, agora, cx))
            .child(div().text_xs().text_color(apagado).child(rodape))
            .children(self.dialogo_do_estudio(cx))
            .children(self.popover_do_periodo(cx))
    }
}

/// O selo de situação, com as cores de `COR_DA_SITUACAO` do site.
fn selo_da_situacao(situacao: Situacao, cx: &App) -> impl IntoElement {
    use crate::tema::cores;
    let tema = cx.theme();
    let cores = match situacao {
        Situacao::SemFotos => cores::selo_ambar(),
        Situacao::AguardandoCliente => cores::selo_ceu(),
        Situacao::AbertaPeloCliente => cores::selo_esmeralda(),
        Situacao::Vencida => (tema.muted, tema.border, tema.muted_foreground),
    };
    crate::estilo::selo_colorido(cores).child(situacao.rotulo())
}

/// `2026-09-16T13:04:00Z` → `16/09/2026`, no fuso do estúdio.
fn data_br(iso: &str) -> String {
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600).expect("fuso fixo");
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&brasilia).format("%d/%m/%Y").to_string())
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(&iso[..iso.len().min(10)], "%Y-%m-%d")
                .map(|d| d.format("%d/%m/%Y").to_string())
        })
        .unwrap_or_else(|_| iso.to_string())
}

impl Sessoes {
    /// Pede a capa de cada estúdio que ainda não tem uma — uma vez por id.
    fn pedir_as_capas(&mut self) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let _ = sessao;
        for estudio in &self.estudios {
            let Some(url) = estudio.foto.clone() else {
                continue;
            };
            if !self.capas_pedidas.insert(estudio.id.clone()) {
                continue;
            }
            self.publicador
                .capa_do_estudio(estudio.id.clone(), url, self.recados.0.clone());
        }
    }

    /// A miniatura do estúdio — a capa do cadastro, ou a inicial do nome.
    ///
    /// 🎨 **É o avatar do shadcn**: quadrado de canto arredondado, a imagem
    /// cobrindo (`object-cover`), e a inicial no fundo `muted` quando não há
    /// foto. O site faz o mesmo no agendamento, com um gradiente no lugar da
    /// inicial.
    fn capa_do_estudio(&self, estudio: &Estudio, lado: f32, cx: &App) -> gpui::AnyElement {
        let tema = cx.theme();
        let moldura = div()
            .flex_none()
            .size(px(lado))
            .rounded(px(8.))
            .overflow_hidden()
            .bg(tema.muted);
        match self.capas.get(&estudio.id) {
            Some(imagem) => moldura
                .child(
                    gpui::img(imagem.clone())
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                )
                .into_any_element(),
            None => moldura
                .flex()
                .items_center()
                .justify_center()
                .text_color(tema.muted_foreground)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(SharedString::from(
                    estudio
                        .nome
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                ))
                .into_any_element(),
        }
    }

    /// O estúdio em que **esta máquina** está trabalhando — conferido contra a
    /// lista de agora.
    ///
    /// 🔑 **A lista manda.** O nome lembrado pode estar velho (o estúdio foi
    /// renomeado), e um desativado não continua valendo como padrão só porque
    /// alguém o escolheu na semana passada. É o mesmo de
    /// `estudio-de-trabalho.ts`, no site.
    pub fn estudio_de_trabalho(&self) -> Option<&Estudio> {
        let lembrado = estudio_lembrado(&self.lembranca)?;
        self.estudios.iter().find(|e| e.id == lembrado)
    }

    /// Abre a pergunta — é o clique no chip do estúdio.
    pub fn trocar_de_estudio(&mut self, cx: &mut Context<Self>) {
        if self.estudios.is_empty() {
            return;
        }
        self.escolhendo_estudio = true;
        cx.notify();
    }

    /// A resposta: fica lembrada nesta máquina e vale para todas as telas.
    pub fn escolher_estudio_de_trabalho(&mut self, id: &str, cx: &mut Context<Self>) {
        lembrar_estudio(&self.lembranca, id);
        self.escolhendo_estudio = false;
        // 🔑 **A sessão nova em curso acompanha.** Trocar de estúdio com o
        // formulário aberto e deixá-lo no anterior seria a tela contradizendo a
        // resposta que acabou de receber.
        if let Some(nova) = self.nova.as_mut() {
            nova.estudio_id = Some(id.to_string());
        }
        cx.notify();
    }

    /// 🧪 A pergunta está na tela?
    #[cfg(test)]
    pub(crate) fn perguntando_o_estudio(&self) -> bool {
        self.escolhendo_estudio
    }

    /// 📅 O calendário do período — os atalhos à esquerda, o mês à direita.
    ///
    /// 🎨 **É o `DatePicker` do shadcn, desenhado aqui**: o mesmo arranjo do
    /// site (`filtro-de-periodo.tsx`), com as peças de `estilo.rs`. O véu
    /// fecha ao clique fora, como um popover.
    fn popover_do_periodo(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        use crate::recursos::Icone;
        use gpui_component::Icon;

        let estado = self.calendario.as_ref()?;
        let hoje = hoje_no_estudio();
        let tema = cx.theme().clone();
        let faixa = self.faixa.clone();
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .id("periodo-veu")
                .occlude()
                .on_click(cx.listener(|tela, _ev, _w, cx| tela.alternar_calendario(cx)))
                .child(
                    div()
                        // Ancorado sob a barra, à esquerda — onde o botão está.
                        .absolute()
                        .top(px(112.))
                        .left(px(360.))
                        .flex()
                        .gap(px(12.))
                        .p(px(12.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(tema.border)
                        .bg(tema.popover)
                        .text_color(tema.popover_foreground)
                        .shadow_lg()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(gpui_component::v_flex().gap(px(2.)).w(px(120.)).children(
                            periodo::atalhos(hoje).into_iter().map(|(rotulo, faixa)| {
                                crate::estilo::botao_fantasma(
                                    SharedString::from(format!("periodo-{rotulo}")),
                                    cx,
                                )
                                .w_full()
                                .justify_start()
                                .child(rotulo)
                                .on_click(cx.listener(
                                    move |tela, _ev, _w, cx| {
                                        tela.escolher_periodo(faixa.clone(), cx)
                                    },
                                ))
                            }),
                        ))
                        .child(
                            gpui_component::v_flex()
                                .gap(px(8.))
                                .child(
                                    gpui_component::h_flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            crate::estilo::botao_fantasma(
                                                "periodo-mes-anterior",
                                                cx,
                                            )
                                            .child(Icon::new(Icone::ChevronLeft).size(px(14.)))
                                            .on_click(
                                                cx.listener(|tela, _ev, _w, cx| {
                                                    tela.andar_no_mes(-1, cx)
                                                }),
                                            ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(SharedString::from(periodo::titulo_do_mes(
                                                    estado.mes,
                                                ))),
                                        )
                                        .child(
                                            crate::estilo::botao_fantasma(
                                                "periodo-mes-proximo",
                                                cx,
                                            )
                                            .child(Icon::new(Icone::ChevronRight).size(px(14.)))
                                            .on_click(
                                                cx.listener(|tela, _ev, _w, cx| {
                                                    tela.andar_no_mes(1, cx)
                                                }),
                                            ),
                                        ),
                                )
                                .child(periodo::calendario(
                                    estado,
                                    faixa.as_ref(),
                                    hoje,
                                    cx,
                                    |tela, dia, _window, cx| tela.clicar_no_dia(dia, cx),
                                )),
                        ),
                ),
        )
    }

    /// O diálogo **sem saída** da entrada: "Em qual estúdio você está?".
    ///
    /// 🚨 **Sem "cancelar", como no site.** A única saída é escolher — e é isso
    /// que impede a tela seguinte de adivinhar.
    fn dialogo_do_estudio(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.escolhendo_estudio {
            return None;
        }
        let apagado = cx.theme().muted_foreground;
        let primaria = cx.theme().primary;
        let atual = self.estudio_de_trabalho().map(|e| e.id.clone());
        Some(
            crate::estilo::veu_do_dialogo().child(
                crate::estilo::caixa_do_dialogo(cx)
                    .child(crate::estilo::cabecalho_do_dialogo(
                        "Em qual estúdio você está?",
                        "A escolha fica guardada nesta máquina e vale para todas as telas: as \
                         sessões que você criar e o caixa saem daqui. Dá para trocar a qualquer \
                         momento na barra.",
                        Some(crate::recursos::Icone::Building2),
                        cx,
                    ))
                    .child(gpui_component::v_flex().gap(px(8.)).children(
                        self.estudios.iter().map(|estudio| {
                            let id = estudio.id.clone();
                            let escolhido = atual.as_deref() == Some(estudio.id.as_str());
                            crate::estilo::opcao_do_dialogo(
                                SharedString::from(format!("estudio-de-trabalho-{}", estudio.id)),
                                cx,
                            )
                            .when(escolhido, |o| o.border_color(primaria))
                            // 🖼️ Capa à esquerda, nome e cidade à direita — o
                            // item de lista com avatar do shadcn.
                            .flex_row()
                            .items_center()
                            .gap(px(12.))
                            .child(self.capa_do_estudio(estudio, LADO_DA_CAPA as f32, cx))
                            .child(
                                gpui_component::v_flex()
                                    .gap(px(2.))
                                    .items_start()
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(SharedString::from(estudio.nome.clone())),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(apagado)
                                            .child(SharedString::from(estudio.cidade.clone())),
                                    ),
                            )
                            .on_click(cx.listener(
                                move |tela, _ev, _window, cx| {
                                    tela.escolher_estudio_de_trabalho(&id, cx)
                                },
                            ))
                        }),
                    )),
            ),
        )
    }

    fn barra(&self, contagens: &sessoes::Contagens, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::estilo;
        use crate::recursos::Icone;
        use gpui_component::Icon;

        let filtrando = self.filtrando(cx);
        let ativa = self.situacao;
        let tema = cx.theme();
        let (borda, texto, fundo, apagado, acento) = (
            tema.border,
            tema.foreground,
            tema.background,
            tema.muted_foreground,
            tema.accent,
        );
        // Os recortes do site: pílulas, a escolhida em cores invertidas.
        let pilula = move |id: SharedString, rotulo: &'static str, quantas: usize, acesa: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .px(px(10.))
                .rounded_full()
                .border_1()
                .border_color(borda)
                .text_xs()
                .cursor_pointer()
                .when(acesa, |p| p.bg(texto).text_color(fundo).border_color(texto))
                .when(!acesa, |p| p.hover(move |s| s.bg(acento)))
                .child(rotulo)
                .child(
                    div()
                        .when(!acesa, |d| d.text_color(apagado))
                        .when(acesa, |d| d.opacity(0.7))
                        .child(quantas.to_string()),
                )
        };

        let sem_conta = self.sessao.is_none() || self.abrindo_nova();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(estilo::desligado(
                estilo::botao_primario("sessoes-nova", cx)
                    .child(Icon::new(Icone::Plus).size(px(16.)))
                    .child("Nova sessão")
                    .when(!sem_conta, |b| {
                        b.on_click(cx.listener(|_tela, _ev, _window, cx| cx.emit(NovaPedida)))
                    }),
                sem_conta,
            ))
            .child(
                div().w(px(320.)).child(
                    Input::new(&self.busca)
                        .prefix(Icon::new(Icone::Search).size(px(16.)).text_color(apagado))
                        .cleanable(true),
                ),
            )
            // 📅 **O período, e ele abre em hoje** (dono, 2026-09-18). O
            // calendário é o nosso (`sessoes::periodo`), em português — o do
            // `gpui-component` só fala inglês, chinês e italiano.
            .child(
                estilo::botao_contorno("sessoes-periodo", cx)
                    .child(Icon::new(Icone::CalendarCheck).size(px(14.)))
                    .child(SharedString::from(periodo::rotulo(
                        self.faixa.as_ref(),
                        hoje_no_estudio(),
                    )))
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.alternar_calendario(cx))),
            )
            .child(
                pilula(
                    "sessoes-todas".into(),
                    "Todas",
                    contagens.todas,
                    ativa.is_none(),
                )
                .on_click(cx.listener(|tela, _ev, _window, cx| tela.filtrar_por(None, cx))),
            )
            .children(Situacao::TODAS.into_iter().filter_map(|situacao| {
                let quantas = contagens.de(situacao);
                // 🔑 A situação vazia some — **menos** quando é a escolhida:
                // sumir o filtro ativo tiraria o caminho de volta.
                if quantas == 0 && ativa != Some(situacao) {
                    return None;
                }
                Some(
                    pilula(
                        SharedString::from(format!("sessoes-{}", situacao.como_texto())),
                        situacao.rotulo(),
                        quantas,
                        ativa == Some(situacao),
                    )
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.filtrar_por(Some(situacao), cx)
                    })),
                )
            }))
            .when(filtrando, |barra| {
                barra.child(
                    estilo::botao_fantasma("sessoes-limpar", cx)
                        .text_color(apagado)
                        .child("Limpar")
                        .on_click(
                            cx.listener(|tela, _ev, window, cx| tela.limpar_filtros(window, cx)),
                        ),
                )
            })
            .child(div().flex_1())
            // 🏢 **O estúdio desta máquina, e o clique que o troca.** Sem um
            // lugar visível para trocar, a escolha do primeiro dia viraria
            // definitiva — e o jeito de desfazê-la seria apagar o arquivo.
            .when_some(
                self.estudio_de_trabalho().map(|e| e.nome.clone()),
                |barra, nome| {
                    barra.child(
                        estilo::botao_contorno("sessoes-estudio-de-trabalho", cx)
                            .child(Icon::new(Icone::Building2).size(px(14.)))
                            .child(SharedString::from(nome))
                            .on_click(
                                cx.listener(|tela, _ev, _window, cx| tela.trocar_de_estudio(cx)),
                            ),
                    )
                },
            )
            .child(estilo::desligado(
                estilo::botao_contorno("sessoes-recarregar", cx)
                    .child(if self.carregando {
                        "Lendo…"
                    } else {
                        "Recarregar"
                    })
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.recarregar(cx))),
                self.sessao.is_none() || self.carregando,
            ))
    }

    fn indicadores(
        &self,
        soma: &sessoes::Soma,
        quantas: usize,
        filtrando: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, apagado) = (tema.border, tema.muted_foreground);
        let (fundo_bom, borda_boa, texto_bom) = crate::tema::cores::destaque_esmeralda();
        let cartao = move |rotulo: String, valor: String, nota: &'static str, destaque: bool| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(if destaque { borda_boa } else { borda })
                .when(destaque, |c| c.bg(fundo_bom))
                .child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child(rotulo.to_uppercase()),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .when(destaque, |d| d.text_color(texto_bom))
                        .child(valor),
                )
                .child(div().text_xs().text_color(apagado).child(nota))
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(cartao(
                        if filtrando {
                            "Balcão, no recorte".into()
                        } else {
                            "Pago no balcão".to_string()
                        },
                        dinheiro::formatar(soma.balcao),
                        "fotos levadas na hora, pelo preço cheio ou pelo que foi negociado",
                        false,
                    ))
                    .child(cartao(
                        if filtrando {
                            "Pós-venda, no recorte".into()
                        } else {
                            "Pago no pós-venda".to_string()
                        },
                        dinheiro::formatar(soma.pos_venda),
                        "fotos que o cliente voltou e comprou pela galeria",
                        true,
                    )),
            )
            // 💰 A tela dizendo de qual conjunto o número é.
            .when(filtrando || soma.sem_totais > 0, |bloco| {
                let mut frase = String::new();
                if filtrando {
                    frase.push_str(&format!(
                        "Somando as {quantas} sessões deste recorte, não o total. "
                    ));
                }
                if soma.sem_totais > 0 {
                    frase.push_str(&format!(
                        "{} sessão(ões) ainda sem os totais na API — o número está menor que o real.",
                        soma.sem_totais
                    ));
                }
                bloco.child(div().text_xs().text_color(apagado).child(frase))
            })
    }

    /// 🎯 Preço por foto e estúdio, em fichas — os dois obrigatórios (dono,
    /// 2026-09-13). Fichas, e não um `Select`, pelo mesmo motivo da barra: são
    /// poucas opções, e a escolhida tem de se ver sem abrir nada.
    fn escolhas_da_nova(&self, nova: &Nova, cx: &mut Context<Self>) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        let rotulo =
            |texto: &'static str| div().w(px(110.)).text_xs().text_color(apagado).child(texto);

        let faixas = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .child(rotulo("preço por foto *"))
            .when(self.produtos.is_empty(), |linha| {
                linha.child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child("carregando o catálogo…"),
                )
            })
            .children(self.produtos.iter().map(|produto| {
                let id = produto.id.clone();
                Button::new(SharedString::from(format!("sessoes-faixa-{}", produto.id)))
                    .label(format!(
                        "{} — R$ {}",
                        produto.nome,
                        produto.preco.replace('.', ",")
                    ))
                    .xsmall()
                    .selected(nova.produto_id.as_deref() == Some(produto.id.as_str()))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.escolher_faixa(id.clone(), cx)
                    }))
            }));

        let estudios =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.))
                .child(rotulo("estúdio *"))
                .when(self.estudios.is_empty(), |linha| {
                    linha.child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child("nenhum estúdio ativo no site"),
                    )
                })
                .children(self.estudios.iter().map(|estudio| {
                    let id = estudio.id.clone();
                    let nome = if estudio.cidade.trim().is_empty() {
                        estudio.nome.clone()
                    } else {
                        format!("{} — {}", estudio.nome, estudio.cidade)
                    };
                    Button::new(SharedString::from(format!(
                        "sessoes-estudio-{}",
                        estudio.id
                    )))
                    .label(nome)
                    .xsmall()
                    .selected(nova.estudio_id.as_deref() == Some(estudio.id.as_str()))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.escolher_estudio(id.clone(), cx)
                    }))
                }));

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(faixas)
            .child(estudios)
    }

    fn formulario(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let nova = self.nova.as_ref()?;
        let campo = |rotulo: &str, estado: &gpui::Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(rotulo.to_string()),
                )
                .child(Input::new(estado).xsmall())
        };

        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .child(div().text_xs().child("Nova sessão fotográfica"))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(campo("título", &nova.titulo))
                        .child(campo("e-mail do cliente", &nova.email))
                        .child(campo("WhatsApp", &nova.whatsapp)),
                )
                .child(self.escolhas_da_nova(nova, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "O contato pode ficar para o fim: o link e o aviso pedem o e-mail antes de sair.",
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(6.))
                        .child(
                            Button::new("sessoes-cancelar")
                                .label("Cancelar")
                                .xsmall()
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.cancelar_nova(cx)),
                                ),
                        )
                        .child(
                            Button::new("sessoes-criar")
                                .label(if nova.enviando {
                                    "Abrindo…"
                                } else {
                                    "Abrir sessão"
                                })
                                .xsmall()
                                .primary()
                                .disabled(nova.enviando)
                                .on_click(cx.listener(|tela, _ev, _window, cx| tela.criar(cx))),
                        ),
                ),
        )
    }

    fn tabela(
        &self,
        visiveis: &[&SessaoFotografica],
        agora: i64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, apagado, realce, fundo) = (
            tema.border,
            tema.muted_foreground,
            tema.muted,
            tema.background,
        );
        if visiveis.is_empty() {
            let frase = if self.galerias.is_empty() {
                "Nenhuma galeria ainda. A primeira nasce no botão \"Nova sessão\"."
            } else {
                "Nenhuma sessão com esse nome ou nessa situação."
            };
            return crate::estilo::cartao(cx)
                .flex()
                .items_center()
                .justify_center()
                .p(px(32.))
                .text_sm()
                .text_color(apagado)
                .child(frase)
                .into_any_element();
        }

        // As colunas do site que o app tem (as de "máquinas" são do relato do
        // navegador, e não existem aqui). Largura fixa nos números, alinhados
        // à direita como no site.
        const LARGURAS: [f32; 6] = [84., 84., 104., 112., 112., 104.];
        let numero = |largura: f32| div().w(px(largura)).flex_none().flex().justify_end();
        let titulo_da_coluna = |texto: &'static str, largura: Option<f32>| match largura {
            Some(l) => numero(l).child(texto).into_any_element(),
            None => div().flex_1().min_w(px(0.)).child(texto).into_any_element(),
        };
        let valor_ou_traco = |centavos: Option<i64>| match centavos {
            Some(c) if c > 0 => dinheiro::formatar(c),
            _ => "—".to_string(),
        };

        let cabecalho = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .h(px(40.))
            .px(px(8.))
            .border_b_1()
            .border_color(borda)
            .text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(titulo_da_coluna("Galeria", None))
            .child(titulo_da_coluna("Contato", None))
            .child(div().w(px(160.)).flex_none().child("Situação"))
            .child(titulo_da_coluna("Levadas", Some(LARGURAS[0])))
            .child(titulo_da_coluna("À venda", Some(LARGURAS[1])))
            .child(titulo_da_coluna("Compradas", Some(LARGURAS[2])))
            .child(titulo_da_coluna("Balcão", Some(LARGURAS[3])))
            .child(titulo_da_coluna("Pós-venda", Some(LARGURAS[4])))
            .child(titulo_da_coluna("Criada", Some(LARGURAS[5])));

        let linhas = visiveis.iter().map(|sessao| {
            let id = sessao.id.clone();
            let e_a_aberta = self.aberta.as_deref() == Some(sessao.id.as_str());
            let (balcao, pos_venda) = match &sessao.totais {
                Some(t) => (Some(t.balcao), Some(t.pos_venda)),
                None => (None, None),
            };
            div()
                .id(SharedString::from(format!("sessao-{}", sessao.id)))
                .flex()
                .items_center()
                .gap(px(16.))
                .min_h(px(52.))
                .px(px(8.))
                .py(px(6.))
                .border_b_1()
                .border_color(borda)
                .text_sm()
                .cursor_pointer()
                .hover(move |s| s.bg(realce.opacity(0.5)))
                .when(e_a_aberta, |linha| linha.bg(realce.opacity(0.5)))
                .on_click(cx.listener(move |tela, _ev, _window, cx| tela.abrir(id.clone(), cx)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .truncate()
                        .child(sessao.titulo.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .text_xs()
                        .text_color(apagado)
                        .when(sessao.email.is_none() && sessao.whatsapp.is_none(), |d| {
                            d.child("—")
                        })
                        .children(sessao.email.clone().map(|e| div().truncate().child(e)))
                        .children(sessao.whatsapp.clone().map(|w| div().truncate().child(w))),
                )
                .child(
                    div()
                        .w(px(160.))
                        .flex_none()
                        .flex()
                        .child(selo_da_situacao(sessao.situacao(agora), cx)),
                )
                .child(numero(LARGURAS[0]).child(sessao.fotos.levadas_no_balcao.to_string()))
                .child(numero(LARGURAS[1]).child(sessao.fotos.disponiveis.to_string()))
                .child(numero(LARGURAS[2]).child(sessao.fotos.compradas.to_string()))
                .child(numero(LARGURAS[3]).child(valor_ou_traco(balcao)))
                .child(numero(LARGURAS[4]).child(valor_ou_traco(pos_venda)))
                .child(
                    numero(LARGURAS[5])
                        .text_xs()
                        .text_color(apagado)
                        .child(data_br(&sessao.criada_em_iso)),
                )
        });

        crate::estilo::cartao(cx)
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .bg(fundo)
            .child(cabecalho)
            .child(
                div()
                    .id("sessoes-linhas")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(linhas),
            )
            .into_any_element()
    }
}

/// Agora, em segundos desde a época — o que a conta da situação compara.
fn agora_em_segundos() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use domain::services::pos_venda::ContagemDeFotos as ContagemDaApi;
    use gpui::TestAppContext;
    use std::sync::Mutex;

    fn galeria(id: &str, titulo: &str, email: Option<&str>) -> GaleriaDoPainel {
        GaleriaDoPainel {
            id: id.into(),
            titulo: titulo.into(),
            email: email.map(str::to_string),
            whatsapp: None,
            produto_id: "p1".into(),
            user_id: None,
            criada_em_iso: "2026-09-06".into(),
            expira_em: None,
            fotos: ContagemDaApi {
                disponiveis: 3,
                ..ContagemDaApi::default()
            },
            totais: None,
            ..Default::default()
        }
    }

    fn estudio() -> Estudio {
        Estudio {
            id: "s1".into(),
            nome: "Gramado".into(),
            cidade: "Gramado".into(),
            foto: None,
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        publicador: Arc<PublicadorDeMentira>,
    ) -> gpui::WindowHandle<Sessoes> {
        cx.update(gpui_component::init);
        cx.add_window(move |window, cx| Sessoes::nova(publicador, window, cx))
    }

    fn colher(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Sessoes>) {
        for _ in 0..10 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
    }

    fn com_sessao(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Sessoes>) {
        janela
            .update(cx, |tela, _window, cx| {
                tela.definir_sessao(
                    Sessao {
                        access_token: "tok".into(),
                        refresh_token: "ref".into(),
                        access_vence_em: i64::MAX,
                        refresh_vence_em: i64::MAX,
                    },
                    cx,
                );
            })
            .expect("a janela deve estar aberta");
        colher(cx, janela);
    }

    /// 🚨 A lista chega e a busca acha — pela **mesma conta do site**.
    ///
    /// O que este teste prende não é a busca em si (ela tem 12 testes em
    /// `biblioteca_core::sessoes`), é a **tradução**: a galeria da API vira
    /// `SessaoFotografica` com os campos certos nos lugares certos. Trocar
    /// `disponiveis` por `compradas` na cópia não faria nada falhar — daria a
    /// situação certa com o número errado.
    #[gpui::test]
    fn a_lista_chega_e_a_busca_acha_como_no_site(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira {
            galerias: Mutex::new(vec![
                galeria("g1", "Ensaio do João", Some("joao@exemplo.com")),
                galeria("g2", "Casamento da Maria", Some("maria@outro.com")),
            ]),
            ..Default::default()
        });
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(tela.quantas(), 2);
                // 📅 A lista abre em hoje; estas são de outro dia, e o que este
                // teste mede é a busca — "Tudo" no seletor tira o período do
                // caminho.
                tela.escolher_periodo_para_teste(None, cx);
                let todas = tela.para_o_core();
                assert_eq!(todas[0].fotos.disponiveis, 3, "a contagem atravessou");
                assert_eq!(todas[0].situacao(0), Situacao::AguardandoCliente);

                // "joao" acha "João" — a normalização é a do core.
                tela.busca
                    .update(cx, |estado, cx| estado.set_value("joao", window, cx));
                let achadas = sessoes::filtrar(&todas, &tela.criterio(cx), 0);
                assert_eq!(achadas.len(), 1);
                assert_eq!(achadas[0].titulo, "Ensaio do João");
                assert!(tela.filtrando(cx), "com busca, os totais falam em recorte");

                tela.limpar_filtros(window, cx);
                tela.escolher_periodo_para_teste(None, cx);
                assert!(!tela.filtrando(cx));
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔚 **Só o título é obrigatório** (dono, 2026-09-13): sem contato a
    /// sessão sai, e o contato é pedido no fim. Sem título, ou com o e-mail
    /// escrito pela metade, não sai — o site recusaria, e deixar sair daqui
    /// gastaria uma ida à rede para trazer de volta um erro que a tela já sabia.
    #[gpui::test]
    fn abrir_sessao_exige_so_o_titulo_e_recusa_email_pela_metade(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador.clone());
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.produtos = vec![Produto {
                    id: "p1".into(),
                    nome: "Foto avulsa".into(),
                    preco: "29.90".into(),
                    inativo: false,
                }];
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                assert_eq!(
                    nova.produto_id.as_deref(),
                    Some("p1"),
                    "a faixa vem sugerida"
                );
                assert_eq!(nova.estudio_id, None, "sem lembrança, o estúdio é escolha");

                // Vazio: não sai.
                tela.criar(cx);
                assert!(tela.erro.is_some());

                // Com título e o e-mail pela metade: também não.
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                nova.email
                    .update(cx, |estado, cx| estado.set_value("ana@", window, cx));
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "e-mail pela metade é recusado");
            })
            .expect("a janela deve estar aberta");
        assert!(
            publicador.criadas().is_empty(),
            "sem título ou com e-mail torto a sessão não sai"
        );

        // 🎯 Sem contato, com título e faixa, mas **sem estúdio**: não sai; e
        // sem faixa também não (dono, 2026-09-13).
        janela
            .update(cx, |tela, window, cx| {
                let nova = tela.nova.as_ref().expect("o formulário continua aberto");
                nova.email
                    .update(cx, |estado, cx| estado.set_value("", window, cx));
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "sem estúdio não sai");

                tela.escolher_estudio("s1".into(), cx);
                if let Some(nova) = tela.nova.as_mut() {
                    nova.produto_id = None;
                }
                tela.erro = None;
                tela.criar(cx);
                assert!(tela.erro.is_some(), "sem preço por foto não sai");
            })
            .expect("a janela deve estar aberta");
        assert!(publicador.criadas().is_empty());

        // Título, faixa e estúdio, sem contato nenhum: sai.
        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_faixa("p1".into(), cx);
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let criadas = publicador.criadas();
        assert_eq!(criadas.len(), 1);
        assert_eq!(criadas[0].titulo, "Ensaio da Ana");
        assert_eq!(criadas[0].email, None);
        assert_eq!(criadas[0].whatsapp, None);
        assert_eq!(criadas[0].produto_id, "p1");
        assert_eq!(criadas[0].estudio_id.as_deref(), Some("s1"));
    }

    /// 🎯 *"Grave o último preset selecionado e o corte utilizado e o
    /// estúdio."* — dono, 2026-09-13. A criação do desktop não tem preset nem
    /// corte; o estúdio escolhido volta sugerido na próxima sessão — e some da
    /// sugestão se deixar de estar ativo.
    #[gpui::test]
    fn o_ultimo_estudio_volta_sugerido_na_proxima_sessao(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.produtos = vec![Produto {
                    id: "p1".into(),
                    nome: "Foto avulsa".into(),
                    preco: "29.90".into(),
                    inativo: false,
                }];
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                tela.escolher_estudio("s1".into(), cx);
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                assert_eq!(
                    tela.nova.as_ref().and_then(|n| n.estudio_id.as_deref()),
                    Some("s1"),
                    "o estúdio da última sessão volta sugerido"
                );

                tela.estudios = vec![];
                tela.comecar_nova(window, cx);
                assert_eq!(
                    tela.nova.as_ref().and_then(|n| n.estudio_id.clone()),
                    None,
                    "estúdio que saiu da lista não é sugerido"
                );
                let _ = std::fs::remove_file(&tela.lembranca);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A rota das sessões pergunta o estúdio antes de qualquer coisa** —
    /// e a resposta fica nesta máquina (dono, 2026-09-18).
    ///
    /// É a mesma pergunta do site (`estudio-de-trabalho.ts`): sem ela, a tela
    /// seguinte adivinha o estúdio pela última sessão criada, que é a de
    /// qualquer balcão — e o operador de Canela cria sessão em Gramado sem
    /// perceber.
    #[gpui::test]
    fn a_entrada_exige_o_estudio_e_a_escolha_fica(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira {
            estudios: vec![estudio()],
            ..Default::default()
        });
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                assert!(
                    tela.perguntando_o_estudio(),
                    "a lista de estúdios chegou e não há escolha guardada"
                );
                assert!(tela.estudio_de_trabalho().is_none());

                tela.escolher_estudio_de_trabalho("s1", cx);
                assert!(!tela.perguntando_o_estudio(), "respondida, ela sai da tela");
                assert_eq!(
                    tela.estudio_de_trabalho().map(|e| e.id.as_str()),
                    Some("s1")
                );

                // 🏢 E a sessão nova nasce nele — sem passar pela sugestão do
                // servidor nem pela lembrança antiga.
                tela.comecar_nova(window, cx);
                assert_eq!(
                    tela.nova.as_ref().and_then(|n| n.estudio_id.as_deref()),
                    Some("s1")
                );

                // O chip abre a mesma pergunta de novo — é a saída da decisão.
                tela.trocar_de_estudio(cx);
                assert!(tela.perguntando_o_estudio());
                let _ = std::fs::remove_file(&tela.lembranca);
            })
            .expect("a janela deve estar aberta");
    }

    /// 📅 **A lista abre em hoje, e o seletor muda o período** (dono,
    /// 18/set/2026: *"na listagem de sessões por padrão deve estar filtrado
    /// como hoje, mas com opção de selecionar o dia ou mesmo range de um
    /// período"*).
    #[gpui::test]
    fn a_lista_abre_em_hoje_e_o_periodo_recorta(cx: &mut TestAppContext) {
        let hoje = super::hoje_no_estudio();
        let ontem = hoje - chrono::Duration::days(1);
        let iso = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
        let de_hoje = |id: &str, dia: chrono::NaiveDate| {
            let mut g = galeria(id, "Ensaio", None);
            g.criada_em_iso = format!("{}T12:00:00Z", iso(dia));
            g
        };
        let publicador = Arc::new(PublicadorDeMentira {
            galerias: Mutex::new(vec![de_hoje("g1", hoje), de_hoje("g2", ontem)]),
            ..Default::default()
        });
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                // 1 · Abre em hoje: a de ontem fica no arquivo.
                assert_eq!(
                    tela.faixa_para_teste(),
                    Some(FaixaDeDatas::no_dia(iso(hoje)))
                );
                let visiveis: Vec<String> =
                    sessoes::filtrar(&tela.para_o_core(), &tela.criterio(cx), 0)
                        .iter()
                        .map(|s| s.id.clone())
                        .collect();
                assert_eq!(visiveis, vec!["g1".to_string()]);
                assert!(tela.filtrando(cx), "hoje é um recorte, e os totais dizem");

                // 2 · Um intervalo de dois dias traz as duas.
                tela.escolher_periodo(
                    Some(FaixaDeDatas {
                        de: iso(ontem),
                        ate: iso(hoje),
                    }),
                    cx,
                );
                assert_eq!(
                    sessoes::filtrar(&tela.para_o_core(), &tela.criterio(cx), 0).len(),
                    2
                );

                // 3 · "Tudo" (o seletor limpo) é o arquivo inteiro.
                tela.escolher_periodo(None, cx);
                assert_eq!(tela.faixa_para_teste(), None);
                assert_eq!(
                    sessoes::filtrar(&tela.para_o_core(), &tela.criterio(cx), 0).len(),
                    2
                );

                // 4 · E o "limpar" volta ao padrão — hoje, e não o arquivo.
                tela.limpar_filtros(window, cx);
                assert_eq!(
                    tela.faixa_para_teste(),
                    Some(FaixaDeDatas::no_dia(iso(hoje)))
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 **A lista mostra as sessões do estúdio escolhido** (dono,
    /// 18/set/2026: *"as sessões do grid precisam ser filtradas de acordo com o
    /// estúdio selecionado"*).
    ///
    /// A sessão **sem** estúdio aparece em todas: são as de antes de o campo
    /// existir, e escondê-las de todo mundo seria perdê-las de vista.
    #[gpui::test]
    fn a_lista_mostra_so_as_sessoes_do_estudio_escolhido(cx: &mut TestAppContext) {
        let de_gramado = |id: &str, estudio: Option<&str>| {
            let mut g = galeria(id, "Ensaio", None);
            g.estudio_id = estudio.map(|e| e.to_string());
            g
        };
        let publicador = Arc::new(PublicadorDeMentira {
            galerias: Mutex::new(vec![
                de_gramado("g1", Some("s1")),
                de_gramado("g2", Some("s2")),
                de_gramado("g3", None),
            ]),
            estudios: vec![
                estudio(),
                Estudio {
                    id: "s2".into(),
                    nome: "Canela".into(),
                    cidade: "Canela".into(),
                    foto: None,
                },
            ],
            ..Default::default()
        });
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                // Sem escolha ainda: a lista é inteira — e a pergunta está na tela.
                assert_eq!(tela.para_o_core().len(), 3);

                tela.escolher_estudio_de_trabalho("s1", cx);
                let ids: Vec<String> = tela.para_o_core().into_iter().map(|s| s.id).collect();
                assert_eq!(
                    ids,
                    vec!["g1".to_string(), "g3".to_string()],
                    "as de Gramado e a sem estúdio"
                );

                tela.escolher_estudio_de_trabalho("s2", cx);
                let ids: Vec<String> = tela.para_o_core().into_iter().map(|s| s.id).collect();
                assert_eq!(ids, vec!["g2".to_string(), "g3".to_string()]);
                let _ = std::fs::remove_file(&tela.lembranca);
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ **Sem estúdio cadastrado não se pergunta nada**: um diálogo sem opção
    /// e sem saída é pior do que a ausência dele — e quem está começando ainda
    /// não cadastrou estúdio nenhum.
    #[gpui::test]
    fn sem_estudio_cadastrado_a_entrada_nao_pergunta(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert!(tela.estudios.is_empty());
                assert!(!tela.perguntando_o_estudio());
                tela.trocar_de_estudio(cx);
                assert!(
                    !tela.perguntando_o_estudio(),
                    "nem pelo chip: não há o que escolher"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 A sessão recém-aberta **já fica escolhida**.
    ///
    /// Quem acabou de cadastrar o cliente vai subir foto nele agora, e não daqui
    /// a três telas. É o mesmo encadeamento que "criar coleção leva a seleção
    /// junto" tem na Biblioteca.
    #[gpui::test]
    fn a_sessao_recem_aberta_ja_fica_escolhida(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = janela(cx, publicador);
        com_sessao(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.produtos = vec![Produto {
                    id: "p1".into(),
                    nome: "Foto avulsa".into(),
                    preco: "29.90".into(),
                    inativo: false,
                }];
                tela.estudios = vec![estudio()];
                tela.comecar_nova(window, cx);
                let nova = tela.nova.as_ref().expect("o formulário está aberto");
                nova.titulo.update(cx, |estado, cx| {
                    estado.set_value("Ensaio da Ana", window, cx)
                });
                nova.email.update(cx, |estado, cx| {
                    estado.set_value("ana@exemplo.com", window, cx)
                });
                tela.escolher_estudio("s1".into(), cx);
                tela.criar(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.aberta(), Some("g1"));
                assert!(!tela.abrindo_nova(), "o formulário fechou sozinho");
                assert_eq!(tela.quantas(), 1, "e a lista já a inclui");
            })
            .expect("a janela deve estar aberta");
    }
}
