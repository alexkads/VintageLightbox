//! A tira (filmstrip) da Revelação — o porte de `revelacao/tira.tsx` e do
//! pedaço do `editor.tsx` que a monta: os recortes, o puxador, o menu do botão
//! direito e as marcas de cada miniatura.
//!
//! 🚨 *"Filmstrip do modo revelação tá diferente"* — dono, 2026-09-17: *"mesmos
//! atalhos, funcionalidades e mesmo visual — não abro mão de nada"*. Tudo o que
//! está aqui tem um par no site, e o comentário diz qual.
//!
//! ```text
//! ┌ Todas 29 · Classificadas 29 · Sinalizadas 1 · À venda 28 ───────────┐  recortes
//! ├──────────────────────────────── ═ ─────────────────────────────────┤  puxador
//! │ ‹ [★★★  •] [★★★   ] [★★★ ●] …                                  › │  miniaturas
//! └────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use adapters::view_models::PhotoViewModel;
use biblioteca_core::acervo::{self, Contagens, Estado, Filtro};
use biblioteca_core::selecao::Modificadores;
use domain::services::PreviewType;
use gpui::{
    canvas, div, img, prelude::*, px, AnyElement, Context, MouseButton, Pixels, SharedString,
    Window,
};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme, Icon, WindowExt};

use crate::biblioteca::miniaturas::Miniatura;
use crate::imagem::para_gpui;
use crate::recursos::Icone;
use crate::sessoes::altura_da_tira;
use crate::tema;

use super::super::persistencia;
use super::super::sincronizacao;
use super::{Aberta, PedidoDaRevelacao, Revelacao, MINIATURAS_DA_TIRA};

/// O vão entre duas miniaturas, e entre a primeira e a borda — o `gap-2 px-2`.
const VAO: f32 = 8.0;

/// Onde a altura fica guardada — o `qual` do site.
const QUAL: &str = "revelacao";

/// A altura com que a tira nasce: a do site (`alturaGuardada("revelacao", 110)`).
pub const ALTURA_PADRAO: f32 = 110.0;

/// Os recortes da tira, na ordem do site (`FILTROS_DA_TIRA`, `recorte.ts`).
///
/// 🔑 **Cinco, e não os sete da galeria.** A comprada não se revela e a apagada
/// nem chega aqui; se um desses vier da sessão, o chip dele aparece aceso
/// (ver [`Revelacao::recortes_da_barra`]).
pub const FILTROS_DA_TIRA: [(&str, Filtro); 5] = [
    ("Todas", Filtro::Todas),
    ("Classificadas", Filtro::Classificadas),
    // "Sinalizada" é a levada no balcão — o que a tecla P marca.
    ("Sinalizadas", Filtro::Situacao(Estado::LevadaNoBalcao)),
    ("À venda", Filtro::Situacao(Estado::Disponivel)),
    // 🚨 Último de propósito, como na galeria: é recorte de exceção.
    ("Sem nota", Filtro::SemNota),
];

/// O que a tira lembra entre um quadro e outro.
pub(super) struct EstadoDaTira {
    /// O recorte em vigor — o `filtro` que o site divide com a galeria.
    recorte: Filtro,
    /// A altura da faixa, que **é** o zoom das miniaturas.
    altura: f32,
    /// O arrasto do puxador: onde o ponteiro desceu e a altura de então.
    arrasto: Option<(Pixels, f32)>,
    /// A foto do último botão direito — o alvo do menu que vai abrir.
    menu: Option<usize>,
    /// O "Zerar N fotos" do menu, enquanto a raiz o atende.
    ///
    /// 🔑 O alvo do menu não é sempre "as marcadas": é a seleção quando a foto
    /// clicada faz parte dela, e só ela quando não faz. A raiz pergunta por
    /// [`Revelacao::outras_a_zerar`], e é isto que responde por ela.
    zerar_do_menu: Option<Vec<String>>,
    /// O "Baixar como… (N)" do menu, até a raiz abrir a exportação.
    a_baixar: Vec<PhotoViewModel>,
    /// Quantos quadros ainda tentam centralizar a aberta.
    ///
    /// ⚠️ **Dois**, porque a conta usa o leiaute do quadro anterior: no primeiro
    /// depois de trocar o recorte as posições ainda são as velhas.
    centrar: u8,
    /// As fotos (id no site) com receita que a galeria ainda não recebeu.
    pendentes: BTreeSet<String>,
    /// O primeiro item da tira que virou elemento no último quadro — os que
    /// vêm antes são um espaçador (ver [`faixa_desenhada`]).
    primeira_desenhada: usize,
    /// O menu aberto pelo roteiro de depuração, e onde.
    menu_do_roteiro: Option<(
        gpui::Entity<gpui_component::menu::PopupMenu>,
        gpui::Point<Pixels>,
    )>,
}

impl EstadoDaTira {
    pub(super) fn novo() -> Self {
        Self {
            recorte: Filtro::Todas,
            // Os testes não leem a arrumação de quem trabalha.
            altura: if cfg!(test) {
                ALTURA_PADRAO
            } else {
                altura_da_tira::guardada_ou(QUAL, ALTURA_PADRAO)
            },
            arrasto: None,
            menu: None,
            zerar_do_menu: None,
            a_baixar: Vec::new(),
            centrar: 0,
            pendentes: BTreeSet::new(),
            primeira_desenhada: 0,
            menu_do_roteiro: None,
        }
    }
}

// ------------------------------------------------------------ regras puras

/// A foto como o recorte a enxerga — só nota, situação e apagada.
///
/// 🔑 **A regra é a do core** ([`Filtro::bate`]), a mesma da galeria; aqui só
/// se traduz o `PhotoViewModel`, que guarda a situação em dois campos:
///
/// | `comprada` | `revelacao_travada` | situação |
/// |---|---|---|
/// | não | não | à venda |
/// | sim | não | levada no balcão |
/// | sim | sim | comprada |
/// | não | sim | apagada |
///
/// ⚠️ `id` e `arquivo` ficam vazios: quem pergunta é o filtro, e isto roda por
/// foto a cada quadro.
pub fn classificacao(foto: &PhotoViewModel) -> acervo::Foto {
    let (estado, apagada) = match (foto.comprada, foto.revelacao_travada) {
        (true, true) => (Estado::Comprada, false),
        (true, false) => (Estado::LevadaNoBalcao, false),
        (false, true) => (Estado::Disponivel, true),
        (false, false) => (Estado::Disponivel, false),
    };
    acervo::Foto {
        id: String::new(),
        arquivo: String::new(),
        estado,
        apagada,
        produto_efetivo: String::new(),
        preco_negociado: None,
        tem_observacao: false,
        preco_de_venda: None,
        pedido_id: None,
        downloads: 0,
        revelada: false,
        nota: (1..=5).contains(&foto.rating).then_some(foto.rating as u8),
        ordem: 0,
    }
}

/// As posições do acervo que a tira mostra: o recorte, **mais a aberta**.
///
/// 🔑 A foto do palco nunca some da tira (`fotosDoRecorte`): filtrar "Sem nota"
/// com uma classificada aberta tiraria a referência de onde se está.
pub fn na_tira(acervo: &[PhotoViewModel], recorte: Filtro, aberta: usize) -> Vec<usize> {
    acervo
        .iter()
        .enumerate()
        .filter(|(i, f)| *i == aberta || recorte.bate(&classificacao(f)))
        .map(|(i, _)| i)
        .collect()
}

/// Quantas fotos cada recorte tem — o número de cada chip.
pub fn contar(acervo: &[PhotoViewModel]) -> Contagens {
    let mut contagem = acervo::Acervo::novo();
    contagem.definir(acervo.iter().map(classificacao).collect());
    contagem.contagens()
}

/// A vizinha na tira, sem dar a volta — as setas andam sobre o recorte.
pub fn vizinha(na_tira: &[usize], aberta: usize, passo: i32) -> Option<usize> {
    let i = na_tira.iter().position(|p| *p == aberta)?;
    let j = if passo > 0 {
        i.checked_add(1)?
    } else {
        i.checked_sub(1)?
    };
    na_tira.get(j).copied()
}

/// O lote depois do clique simples (`selecaoAoTrocar`): trocar **dentro** dele o
/// mantém; trocar para fora recomeça na nova.
pub fn ao_trocar(marcadas: &BTreeSet<usize>, para: usize) -> BTreeSet<usize> {
    if marcadas.contains(&para) {
        marcadas.clone()
    } else {
        sincronizacao::so(para)
    }
}

/// `Shift` no clique: a faixa **da tira** entre a aberta e a clicada, mais a
/// aberta — as que o recorte esconde não entram.
pub fn faixa_na_tira(na_tira: &[usize], aberta: usize, clicada: usize) -> Option<BTreeSet<usize>> {
    let de = na_tira.iter().position(|p| *p == aberta)?;
    let ate = na_tira.iter().position(|p| *p == clicada)?;
    let mut faixa: BTreeSet<usize> = na_tira[de.min(ate)..=de.max(ate)].iter().copied().collect();
    faixa.insert(aberta);
    Some(faixa)
}

/// `Cmd+A` e "Escolher todas": **a tira**, somada ao que já estava marcado.
pub fn marcar_a_tira(marcadas: &BTreeSet<usize>, na_tira: &[usize]) -> BTreeSet<usize> {
    marcadas.iter().chain(na_tira).copied().collect()
}

/// Sobre quem o menu age (`alvosDoMenu`): a seleção quando a clicada faz parte
/// dela, e só a clicada quando não faz.
pub fn alvos_do_menu(marcadas: &BTreeSet<usize>, na_tira: &[usize], clicada: usize) -> Vec<usize> {
    if marcadas.contains(&clicada) {
        na_tira
            .iter()
            .copied()
            .filter(|p| marcadas.contains(p))
            .collect()
    } else {
        vec![clicada]
    }
}

/// Com quais marcadas a Revelação abre (`selecaoAoAbrir`): as da sessão, se
/// incluírem a aberta; senão, só ela.
pub fn selecao_ao_abrir(
    acervo: &[PhotoViewModel],
    aberta: usize,
    da_sessao: &[String],
) -> BTreeSet<usize> {
    let lote: BTreeSet<usize> = acervo
        .iter()
        .enumerate()
        .filter(|(_, f)| da_sessao.contains(&id_na_grade(f)))
        .map(|(i, _)| i)
        .collect();
    if lote.contains(&aberta) {
        lote
    } else {
        sincronizacao::so(aberta)
    }
}

/// Quais itens da tira viram elemento neste quadro: `[de, ate)`.
///
/// 🚨 **A tira desenhava o acervo inteiro a cada quadro**, e cada miniatura
/// custa ~30 µs para montar (id, dica, marcas, ouvintes): 7,9 ms com 125
/// fotos, 22 ms com 500 e 65 ms com 2.000 (medido pelo estresse em
/// 17/set/2026, perfil de teste). Um ensaio de casamento já passava do quadro
/// de 60 fps — e todo arrasto de slider redesenha a tela.
///
/// 🔑 Os itens têm largura fixa, então a posição de cada um é conta: só o que
/// está à vista, mais **uma tela para cada lado**, é montado; o resto vira dois
/// espaçadores do mesmo tamanho, e a rolagem não percebe a diferença.
pub fn faixa_desenhada(total: usize, passo: f32, deslocamento: f32, vista: f32) -> (usize, usize) {
    if total == 0
        || passo.is_nan()
        || passo <= 0.
        || !vista.is_finite()
        || !deslocamento.is_finite()
    {
        return (0, total);
    }
    let vista = vista.max(0.);
    let de = ((deslocamento - VAO - vista) / passo).floor().max(0.) as usize;
    let ate = ((deslocamento + 2. * vista) / passo).ceil().max(0.) as usize + 1;
    let de = de.min(total);
    (de, ate.clamp(de, total))
}

/// O id com que a grade da sessão conhece a foto: o do site, ou o do catálogo.
pub fn id_na_grade(foto: &PhotoViewModel) -> String {
    foto.pos_venda_foto_id
        .clone()
        .unwrap_or_else(|| foto.id.clone())
}

/// A frase do `title` do site: as marcas pequenas viram texto.
fn dica(
    foto: &PhotoViewModel,
    numero: usize,
    escolhida: bool,
    marcada: bool,
    nao_salva: bool,
) -> String {
    let c = classificacao(foto);
    let nota = match c.nota {
        Some(1) => "1 estrela".to_string(),
        Some(n) => format!("{n} estrelas"),
        None => "sem nota".to_string(),
    };
    let situacao = if c.apagada {
        "apagada"
    } else {
        match c.estado {
            Estado::LevadaNoBalcao => "levada no balcão",
            Estado::Comprada => "comprada, não se revela",
            Estado::Disponivel => "à venda",
        }
    };
    let mut partes = vec![
        format!("{numero}. {}", foto.name),
        nota,
        situacao.to_string(),
    ];
    if nao_salva {
        partes.push("editada aqui, não salva".into());
    }
    if marcada && !escolhida {
        partes.push("escolhida para sincronizar".into());
    }
    partes.join(" — ")
}

/// O que o menu do botão direito precisa saber, lido uma vez ao abrir.
struct Menu {
    clicada: usize,
    arquivo: String,
    e_a_aberta: bool,
    na_escolha: bool,
    alvos: Vec<PhotoViewModel>,
    quantas_zeram: usize,
    marcadas: usize,
    pode_sincronizar: bool,
}

// ------------------------------------------------------------- a tela

impl Revelacao {
    /// As posições do acervo que a tira mostra agora.
    pub fn na_tira(&self) -> Vec<usize> {
        if self.acervo.is_empty() {
            return Vec::new();
        }
        na_tira(&self.acervo, self.tira.recorte, self.posicao)
    }

    /// 🧪 Os ids das fotos da tira, na ordem — o que o cenário e2e afirma.
    ///
    /// 🔑 **Id da grade**, e não o do catálogo: é ele que liga a tira à foto da
    /// sessão (`site:<id>` quando ela já subiu).
    #[cfg(test)]
    pub(crate) fn ids_na_tira(&self) -> Vec<String> {
        self.na_tira()
            .into_iter()
            .filter_map(|p| self.acervo.get(p).map(id_na_grade))
            .collect()
    }

    /// Onde a aberta está **na tira**, e quantas a tira tem — o "3/29" do site
    /// conta sobre o recorte.
    pub fn posicao_na_tira(&self) -> (usize, usize) {
        let tira = self.na_tira();
        let i = tira.iter().position(|p| *p == self.posicao).unwrap_or(0);
        (i, tira.len())
    }

    /// O recorte em vigor.
    pub fn recorte(&self) -> Filtro {
        self.tira.recorte
    }

    /// Troca o recorte (o `aoMudarFiltro` do site). A seleção fica.
    pub fn recortar(&mut self, recorte: Filtro, cx: &mut Context<Self>) {
        if self.tira.recorte != recorte {
            self.tira.recorte = recorte;
            // A aberta pode ter mudado de lugar na faixa: centraliza de novo.
            self.ultima_na_tira = None;
            cx.notify();
        }
    }

    /// Recorte e marcadas vindos da grade da sessão — o estado é um só,
    /// como no site.
    pub fn herdar_da_sessao(
        &mut self,
        recorte: Filtro,
        marcadas: &[String],
        cx: &mut Context<Self>,
    ) {
        self.tira.recorte = recorte;
        if !self.acervo.is_empty() {
            self.marcadas = selecao_ao_abrir(&self.acervo, self.posicao, marcadas);
        }
        self.ultima_na_tira = None;
        cx.notify();
    }

    /// As marcadas, pelo id da grade da sessão — o caminho de volta.
    pub fn marcadas_na_grade(&self) -> Vec<String> {
        self.marcadas
            .iter()
            .filter_map(|p| self.acervo.get(*p))
            .map(id_na_grade)
            .collect()
    }

    /// A raiz diz quais fotos (id no site) têm receita que não subiu.
    pub fn definir_pendentes(&mut self, pendentes: BTreeSet<String>, cx: &mut Context<Self>) {
        if self.tira.pendentes != pendentes {
            self.tira.pendentes = pendentes;
            cx.notify();
        }
    }

    /// O que o "Baixar como…" do menu escolheu. A raiz leva e esvazia.
    pub fn levar_a_baixar(&mut self) -> Vec<PhotoViewModel> {
        std::mem::take(&mut self.tira.a_baixar)
    }

    /// O "Zerar N fotos" do menu, enquanto a raiz o atende.
    pub(super) fn zerar_do_menu(&self) -> Option<&[String]> {
        self.tira.zerar_do_menu.as_deref()
    }

    /// O clique na tira: sozinho troca de foto, com Ctrl marca, com Shift marca
    /// a faixa — como no Lightroom e no site.
    ///
    /// ⚠️ **Ctrl e Shift não trocam a foto aberta.** Trocar grava a anterior;
    /// montar um lote de dez fotos trocando dez vezes deixaria o operador
    /// sempre com uma só marcada. Quem manda no canvas é o clique simples.
    pub fn clicar_na_tira(
        &mut self,
        posicao: usize,
        modificadores: Modificadores,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if posicao >= self.acervo.len() {
            return;
        }
        if modificadores.aditivo {
            sincronizacao::alternar(&mut self.marcadas, self.posicao, posicao);
            cx.notify();
        } else if modificadores.faixa {
            if let Some(faixa) = faixa_na_tira(&self.na_tira(), self.posicao, posicao) {
                self.marcadas = faixa;
                cx.notify();
            }
        } else {
            self.ir_para(posicao, window, cx);
        }
    }

    /// `Cmd+A`: **a tira** — o recorte, e não o acervo inteiro (`marcar-a-tira`).
    pub fn marcar_todas(&mut self, cx: &mut Context<Self>) {
        if self.acervo.is_empty() {
            return;
        }
        self.marcadas = marcar_a_tira(&self.marcadas, &self.na_tira());
        cx.notify();
    }

    /// `Cmd+D`: só a aberta.
    pub fn desmarcar(&mut self, cx: &mut Context<Self>) {
        if self.acervo.is_empty() {
            return;
        }
        self.marcadas = sincronizacao::so(self.posicao);
        cx.notify();
    }

    /// Carrega as miniaturas da tira **fora da thread que desenha**.
    ///
    /// 🚨 **Era síncrono, dentro do render, e o ensaio inteiro de uma vez.**
    /// Entrar na Revelação de um ensaio de 125 fotos lia e convertia as 125
    /// miniaturas antes do primeiro quadro: 49,77 ms de um gesto de 68,37 ms
    /// (medido em 8/set/2026, `medir-revelacao`).
    ///
    /// 🔑 **O caro vai para o executor de fundo, uma foto por vez**, e a tira
    /// aparece enchendo do palco para fora. O cache mora na tela, e só ela o
    /// toca.
    pub(super) fn carregar_a_tira(&mut self, cx: &mut Context<Self>) {
        if self.acervo.len() < 2 {
            return;
        }
        self.segurar_as_vizinhas();
        // 🚨 **A peneira é aqui, e não lá dentro**: perguntar de dentro da
        // tarefa custava um `esta.update` por foto — 125 saltos por seta com a
        // tira já carregada. Nada faltando, tarefa nenhuma.
        let faltando = self.miniaturas_faltando();
        if faltando.is_empty() {
            return;
        }
        let previews = self.previews.clone();

        self._tira = Some(cx.spawn(async move |esta, cx| {
            for id in faltando {
                let pronta = {
                    let previews = previews.clone();
                    let id = id.clone();
                    cx.background_executor()
                        .spawn(async move {
                            // 🚨 **A prévia revelada local vem antes da do
                            // servidor.** Enquanto a foto não sobe, o JPEG de lá
                            // é o de antes do "Sincronizar"; quem tem o efeito é
                            // esta chave. É o mesmo que a grade da sessão faz, e
                            // o que a web faz em `usar-previas-reveladas.ts`.
                            let revelada = persistencia::chave_da_revelada(&id);
                            let chave = if previews.tem(&revelada, PreviewType::Thumbnail) {
                                revelada
                            } else {
                                id
                            };
                            previews.get_thumbnail(&chave).map(para_gpui)
                        })
                        .await
                };
                // `update` falha quando a tela morreu.
                let atualizou = esta.update(cx, |tela, cx| {
                    tela.miniaturas_da_tira.guardar(
                        &id,
                        match pronta {
                            Some(imagem) => Miniatura::Pronta(imagem),
                            None => Miniatura::Ausente,
                        },
                    );
                    cx.notify();
                });
                if atualizou.is_err() {
                    return;
                }
            }
        }));
    }

    /// As vizinhas do palco que já estão no cache passam à frente na fila de
    /// descarte — da mais longe para a mais perto, que fica por último.
    ///
    /// 🚨 **Com o acervo maior que o cache, o LRU descartava justo o palco.**
    /// O carregamento guarda do palco para fora; as primeiras guardadas eram as
    /// primeiras a sair quando as de longe chegavam, e ao fim de cada seta o
    /// cache tinha as 512 **mais distantes** — a tira em volta da foto aberta
    /// ficava preta, e a seta seguinte lia tudo de novo (achado pelo estresse,
    /// 17/set/2026, com 10.000 fotos).
    fn segurar_as_vizinhas(&mut self) {
        let perto: Vec<usize> = self
            .da_posicao_para_fora()
            .take(MINIATURAS_DA_TIRA)
            .collect();
        for i in perto.into_iter().rev() {
            if let Some(foto) = self.acervo.get(i) {
                self.miniaturas_da_tira.tocar(&foto.id);
            }
        }
    }

    /// Quais miniaturas da tira ainda não foram lidas — na ordem de urgência.
    ///
    /// `espiar` devolvendo `Some` inclui o `Ausente`: já perguntada é já
    /// perguntada, e é isso que impede de repetir a leitura a cada troca.
    ///
    /// ⚠️ **Só as que cabem no cache**, a partir do palco: pedir mais que isso
    /// é ler do disco o que o LRU vai jogar fora na mesma volta.
    pub(crate) fn miniaturas_faltando(&self) -> Vec<String> {
        self.da_posicao_para_fora()
            .take(MINIATURAS_DA_TIRA)
            .filter_map(|i| self.acervo.get(i))
            .filter(|foto| self.miniaturas_da_tira.espiar(&foto.id).is_none())
            .map(|foto| foto.id.clone())
            .collect()
    }

    /// Esquece a miniatura desta foto na memória da tira — a próxima passada
    /// relê do cache. É o que faz o "Zerar tudo" aparecer na tira.
    pub(crate) fn esquecer_a_miniatura_da_tira(&mut self, foto_id: &str) {
        self.miniaturas_da_tira.esquecer(foto_id);
    }

    /// A miniatura desta foto voltou ao cache: a célula da tira pode reler.
    pub fn miniatura_reposta(&mut self, foto_id: &str, cx: &mut Context<Self>) {
        self.miniaturas_da_tira.esquecer(foto_id);
        cx.notify();
    }

    /// Põe na tira a foto **como ela está sendo revelada**.
    ///
    /// ⚠️ **Só a memória da tira, e não o cache em disco**: gravar a revelada
    /// lá faria a próxima abertura aplicar a receita duas vezes (7/set).
    pub(super) fn atualizar_a_tira_com_o_revelado(&mut self) {
        let Some(Aberta {
            foto,
            revelada: Some(revelada),
            ..
        }) = self.aberta.as_ref()
        else {
            return;
        };
        // Convertida no tamanho em que se desenha (o dobro, pela tela retina).
        let lado = altura_da_tira::lado_da_miniatura(self.tira.altura);
        let largura = (lado * altura_da_tira::PROPORCAO * 2.0) as u32;
        let pequena = revelada.thumbnail(largura, (lado * 2.0) as u32);
        let id = foto.id.clone();
        self.miniaturas_da_tira
            .guardar(&id, Miniatura::Pronta(para_gpui(pequena)));
    }

    /// O menu da foto `clicada`, com os números já contados.
    fn menu_da_tira(&self, clicada: usize) -> Option<Menu> {
        let foto = self.acervo.get(clicada)?;
        let tira = self.na_tira();
        let alvos: Vec<PhotoViewModel> = alvos_do_menu(&self.marcadas, &tira, clicada)
            .into_iter()
            .filter_map(|p| self.acervo.get(p).cloned())
            .collect();
        let aberta_id = self.foto_aberta().map(|f| f.id.clone());
        // A aberta conta se tem o que zerar; as outras, se são reveláveis e
        // saíram do neutro — a mesma conta do "Zerar tudo" do painel.
        let aberta_zera = alvos.iter().any(|f| Some(&f.id) == aberta_id.as_ref())
            && self.tem_pixels()
            && self.pode_revelar()
            && (self.quantos_alterados() > 0 || self.enquadrada());
        let outras = self.outras_do_menu(&alvos).len();
        Some(Menu {
            clicada,
            arquivo: foto.name.clone(),
            e_a_aberta: clicada == self.posicao,
            na_escolha: self.marcadas.contains(&clicada),
            quantas_zeram: outras + usize::from(aberta_zera),
            alvos,
            marcadas: self.marcadas.len(),
            pode_sincronizar: self.marcadas.len() >= 2 && self.tem_pixels() && self.pode_revelar(),
        })
    }

    /// Dos alvos do menu, as **outras** que têm o que zerar (`asQueZeram`).
    fn outras_do_menu(&self, alvos: &[PhotoViewModel]) -> Vec<String> {
        let aberta = self.foto_aberta().map(|f| f.id.clone());
        alvos
            .iter()
            .filter(|f| Some(&f.id) != aberta.as_ref())
            .filter(|f| !f.revelacao_travada)
            .filter(|f| persistencia::ja_revelada(f))
            .map(|f| f.id.clone())
            .collect()
    }

    /// "Zerar N fotos" do menu: a aberta pelo histórico, as outras pela raiz.
    fn zerar_pelo_menu(
        &mut self,
        alvos: Vec<PhotoViewModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let aberta = self.foto_aberta().map(|f| f.id.clone());
        if alvos.iter().any(|f| Some(&f.id) == aberta.as_ref())
            && self.tem_pixels()
            && self.pode_revelar()
            && (self.quantos_alterados() > 0 || self.enquadrada())
        {
            self.redefinir_ajustes(window, cx);
        }
        let outras = self.outras_do_menu(&alvos);
        if outras.is_empty() {
            return;
        }
        self.tira.zerar_do_menu = Some(outras);
        cx.emit(PedidoDaRevelacao::ZerarAsMarcadas);
        // 🔑 O aviso é atendido antes do `defer`: a fila de efeitos anda em
        // ordem. Depois dele a raiz volta a perguntar pelas marcadas.
        let esta = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = esta.update(cx, |tela, _cx| tela.tira.zerar_do_menu = None);
        });
    }

    /// "Baixar como… (N)": a exportação com os alvos do menu.
    fn baixar_pelo_menu(
        &mut self,
        alvos: Vec<PhotoViewModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pendentes = alvos
            .iter()
            .filter(|f| {
                f.pos_venda_foto_id
                    .as_ref()
                    .is_some_and(|id| self.tira.pendentes.contains(id))
            })
            .count();
        if pendentes > 0 {
            window.push_notification(
                format!("{pendentes} têm edição não salva: o arquivo sai como está na galeria."),
                cx,
            );
        }
        self.tira.a_baixar = alvos;
        cx.emit(PedidoDaRevelacao::BaixarComo);
    }

    /// A foto tem receita que a galeria ainda não recebeu — o ponto oco.
    fn nao_salva(&self, posicao: usize, foto: &PhotoViewModel) -> bool {
        if foto.revelacao_travada {
            return false;
        }
        if posicao == self.posicao {
            return self.aberta_a_salvar();
        }
        foto.pos_venda_foto_id
            .as_ref()
            .is_some_and(|id| self.tira.pendentes.contains(id))
    }

    /// Os chips da barra: o recorte ativo sempre aparece, mesmo sem chip próprio.
    fn recortes_da_barra(&self) -> Vec<(&'static str, Filtro)> {
        let mut chips: Vec<(&'static str, Filtro)> = FILTROS_DA_TIRA.to_vec();
        if !chips.iter().any(|(_, f)| *f == self.tira.recorte) {
            let rotulo = match self.tira.recorte {
                Filtro::Situacao(Estado::Comprada) => "Compradas",
                _ => "Apagadas",
            };
            chips.push((rotulo, self.tira.recorte));
        }
        chips
    }

    /// A barra dos recortes, colada na tira (`editor.tsx`).
    ///
    /// O chip com zero some, como na galeria — menos o aceso, que é o caminho
    /// de volta.
    fn barra_de_recortes(&self, cx: &mut Context<Self>) -> AnyElement {
        let contagens = contar(&self.acervo);
        let tema = cx.theme();
        let (borda, apagado, texto, muted) = (
            tema.border,
            tema.muted_foreground,
            tema.foreground,
            tema.muted,
        );
        let ativo = self.tira.recorte;

        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .pt(px(6.))
            .border_t_1()
            .border_color(borda)
            .children(
                self.recortes_da_barra()
                    .into_iter()
                    .filter_map(|(rotulo, filtro)| {
                        let quantas = contagens.de(filtro);
                        let aceso = filtro == ativo;
                        if filtro != Filtro::Todas && quantas == 0 && !aceso {
                            return None;
                        }
                        // O que veio da galeria sem chip aqui: o clique volta a "Todas".
                        let destino = if FILTROS_DA_TIRA.iter().any(|(_, f)| *f == filtro) {
                            filtro
                        } else {
                            Filtro::Todas
                        };
                        Some(
                            div()
                                .id(SharedString::from(format!("tira-recorte-{rotulo}")))
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(px(6.))
                                .h(px(24.))
                                .px(px(8.))
                                .rounded_full()
                                .border_1()
                                .text_size(px(11.))
                                .cursor_pointer()
                                .when(aceso, |chip| {
                                    chip.border_color(tema::cores::quente())
                                        .bg(tema::cores::quente())
                                        .text_color(gpui::black())
                                })
                                .when(!aceso, |chip| {
                                    chip.border_color(borda)
                                        .text_color(apagado)
                                        .hover(move |s| s.bg(muted).text_color(texto))
                                })
                                .child(rotulo)
                                .child(
                                    div()
                                        .opacity(0.7)
                                        .child(SharedString::from(quantas.to_string())),
                                )
                                .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                    tela.recortar(destino, cx)
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    /// A barra que arrasta a altura da tira — **e a altura é o zoom**.
    ///
    /// 🚨 **O arrasto é escutado na janela**, como o `setPointerCapture` do
    /// site: numa barra de 6px o primeiro movimento rápido sai dela.
    fn puxador_da_tira(&self, cx: &mut Context<Self>) -> AnyElement {
        let ouvinte = cx.entity();
        let arrastando = self.tira.arrasto.is_some();
        let tema = cx.theme();
        let (fundo, hover, traco) = (tema.background, tema.muted, tema.muted_foreground);

        div()
            .id("tira-da-revelacao-puxador")
            .group("puxador-da-revelacao")
            .h(px(6.))
            .w_full()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .border_t_1()
            .border_color(tema.border)
            .bg(fundo)
            .hover(move |s| s.bg(hover))
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .tooltip(|window, cx| {
                Tooltip::new("Arraste para mudar o tamanho das miniaturas").build(window, cx)
            })
            .child(
                div()
                    .h(px(2.))
                    .w(px(32.))
                    .rounded_full()
                    .bg(traco.opacity(0.4))
                    .group_hover("puxador-da-revelacao", move |s| s.bg(traco)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|tela, evento: &gpui::MouseDownEvent, _w, cx| {
                    tela.tira.arrasto = Some((evento.position.y, tela.tira.altura));
                    cx.notify();
                }),
            )
            .child(canvas(
                |_bounds, _window, _cx| {},
                move |_bounds, _prepaint, window, _cx| {
                    if !arrastando {
                        return;
                    }
                    window.on_mouse_event({
                        let esta = ouvinte.clone();
                        move |evento: &gpui::MouseMoveEvent, fase, _window, cx| {
                            if !fase.bubble() {
                                return;
                            }
                            esta.update(cx, |tela, cx| {
                                let Some((y, altura)) = tela.tira.arrasto else {
                                    return;
                                };
                                // Para cima é maior: a tira cresce contra o palco.
                                let nova = altura_da_tira::limitar(
                                    altura + f32::from(y - evento.position.y),
                                );
                                if nova != tela.tira.altura {
                                    tela.tira.altura = nova;
                                    cx.notify();
                                }
                            });
                        }
                    });
                    window.on_mouse_event({
                        let esta = ouvinte.clone();
                        move |_evento: &gpui::MouseUpEvent, fase, _window, cx| {
                            if !fase.bubble() {
                                return;
                            }
                            esta.update(cx, |tela, cx| {
                                if tela.tira.arrasto.take().is_some() {
                                    altura_da_tira::guardar(QUAL, tela.tira.altura);
                                    // A miniatura da aberta volta no tamanho novo.
                                    tela.miniaturas_da_tira.esquecer(
                                        &tela
                                            .foto_aberta()
                                            .map(|f| f.id.clone())
                                            .unwrap_or_default(),
                                    );
                                    tela.carregar_a_tira(cx);
                                    cx.notify();
                                }
                            });
                        }
                    });
                },
            ))
            .into_any_element()
    }

    /// Uma foto na tira — o `Tile` do site.
    fn miniatura_na_tira(
        &self,
        posicao: usize,
        numero: usize,
        lado: f32,
        largura: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let foto = &self.acervo[posicao];
        let escolhida = posicao == self.posicao;
        let marcada = self.marcadas.contains(&posicao);
        let nao_salva = self.nao_salva(posicao, foto);
        let c = classificacao(foto);
        let nota = c.nota.unwrap_or(0) as usize;
        let levada = c.estado == Estado::LevadaNoBalcao && !c.apagada;
        let editavel = c.editavel();
        let revelada = !nao_salva && persistencia::ja_revelada(foto);
        let miniatura = match self.miniaturas_da_tira.espiar(&foto.id) {
            Some(Miniatura::Pronta(imagem)) => Some(imagem),
            _ => None,
        };
        let texto_da_dica = SharedString::from(dica(foto, numero, escolhida, marcada, nao_salva));

        let tema = cx.theme();
        // `border-amber-500 dark:border-amber-400`, e a marcada a 40%.
        let ambar = if tema.mode.is_dark() {
            tema::cores::quente()
        } else {
            tema::cores::atencao()
        };
        let (fundo, borda, apagado) = (tema.muted, tema.border, tema.muted_foreground);

        div()
            .id(SharedString::from(format!("tira-revelacao-{}", foto.id)))
            .relative()
            .w(px(largura))
            .h(px(lado))
            .flex_none()
            .overflow_hidden()
            .rounded(px(4.))
            .bg(fundo)
            .border_2()
            .cursor_pointer()
            .when(escolhida, |t| t.border_color(ambar))
            .when(!escolhida && marcada, |t| {
                t.border_color(ambar.opacity(0.4))
            })
            .when(!escolhida && !marcada, |t| {
                t.border_color(gpui::transparent_black())
                    .hover(move |s| s.border_color(borda))
            })
            .tooltip(move |window, cx| Tooltip::new(texto_da_dica.clone()).build(window, cx))
            .map(|t| match miniatura {
                Some(imagem) => t.child(
                    img(imagem)
                        .size_full()
                        // `object-contain`: o recorte da foto enquadrada fica inteiro.
                        .object_fit(gpui::ObjectFit::Contain)
                        // A apagada desbota, como na tira da galeria.
                        .when(c.apagada, |i| i.opacity(0.4)),
                ),
                None => t.child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.))
                        .text_color(apagado.opacity(0.6))
                        .child("sem prévia"),
                ),
            })
            // 🔑 Em cima, o que o cliente decidiu (nota e sinalização); embaixo,
            // o que o operador fez aqui (revelada, não salva).
            .when(nota > 0, |t| {
                t.child(
                    div()
                        .absolute()
                        .top(px(2.))
                        .left(px(4.))
                        .text_size(px(10.))
                        .line_height(px(10.))
                        .text_color(tema::cores::nota())
                        .child(SharedString::from("★".repeat(nota.min(5)))),
                )
            })
            .when(levada, |t| {
                t.child(
                    div()
                        .absolute()
                        .top(px(4.))
                        .right(px(4.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(gpui::rgb(0x00d492))
                        .border_1()
                        .border_color(gpui::black().opacity(0.4)),
                )
            })
            .when(nao_salva, |t| {
                t.child(
                    div()
                        .absolute()
                        .bottom(px(4.))
                        .right(px(4.))
                        .size(px(8.))
                        .rounded_full()
                        .border_2()
                        .border_color(tema::cores::quente()),
                )
            })
            .when(revelada, |t| {
                t.child(
                    div()
                        .absolute()
                        .bottom(px(4.))
                        .right(px(4.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(tema::cores::quente()),
                )
            })
            // 🚨 Fixa nos dois temas: a faixa se lê contra a foto.
            .when(!editavel, |t| {
                t.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .py(px(2.))
                        .text_center()
                        .text_size(px(10.))
                        .bg(gpui::black().opacity(0.7))
                        .text_color(gpui::rgb(0xd4d4d4))
                        .child(if c.apagada { "apagada" } else { "comprada" }),
                )
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |tela, _ev: &gpui::MouseDownEvent, _w, _cx| {
                    tela.tira.menu = Some(posicao);
                }),
            )
            .on_click(
                cx.listener(move |tela, evento: &gpui::ClickEvent, window, cx| {
                    let m = evento.modifiers();
                    // 🚨 Ctrl **e** Cmd acrescentam, como na web (`ctrlKey ||
                    // metaKey`): no macOS o Ctrl+clique chega com `control`.
                    tela.clicar_na_tira(
                        posicao,
                        Modificadores {
                            aditivo: m.secondary() || m.control,
                            faixa: m.shift,
                        },
                        window,
                        cx,
                    );
                }),
            )
            .into_any_element()
    }

    /// Rola a faixa um passo de tela (`andar` do site): 80% da largura, ou 200.
    fn rolar_a_tira(&mut self, direcao: f32, cx: &mut Context<Self>) {
        let largura = f32::from(self.rolagem_da_tira.bounds().size.width);
        let passo = (largura * 0.8).max(200.0);
        let maximo = self.rolagem_da_tira.max_offset().width;
        let atual = self.rolagem_da_tira.offset();
        let x = (atual.x - px(direcao * passo)).min(px(0.)).max(-maximo);
        self.rolagem_da_tira.set_offset(gpui::point(x, atual.y));
        cx.notify();
    }

    /// Uma seta da ponta — só quando há o que rolar para aquele lado.
    fn seta_da_tira(&self, esquerda: bool, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (fundo, texto) = (tema.background, tema.foreground);
        div()
            .id(if esquerda {
                "tira-revelacao-seta-esquerda"
            } else {
                "tira-revelacao-seta-direita"
            })
            .absolute()
            .top_0()
            .bottom_0()
            .w(px(40.))
            .when(esquerda, |s| s.left_0())
            .when(!esquerda, |s| s.right_0())
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(gpui::linear_gradient(
                if esquerda { 90.0 } else { 270.0 },
                gpui::linear_color_stop(fundo, 0.0),
                gpui::linear_color_stop(fundo.opacity(0.0), 1.0),
            ))
            .text_color(texto.opacity(0.9))
            .hover(move |s| s.text_color(texto))
            .tooltip(move |window, cx| {
                Tooltip::new(if esquerda {
                    "Rolar a tira para a esquerda"
                } else {
                    "Rolar a tira para a direita"
                })
                .build(window, cx)
            })
            .child(
                Icon::new(if esquerda {
                    Icone::ChevronLeft
                } else {
                    Icone::ChevronRight
                })
                .size(px(20.)),
            )
            .on_click(cx.listener(move |tela, _ev, _window, cx| {
                tela.rolar_a_tira(if esquerda { -1.0 } else { 1.0 }, cx)
            }))
            .into_any_element()
    }

    /// Traz a aberta **para o meio** da faixa (`inline: "center"` do site).
    fn centralizar_a_aberta(
        &mut self,
        tira: &[usize],
        largura: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ultima_na_tira != Some(self.posicao) {
            self.ultima_na_tira = Some(self.posicao);
            self.tira.centrar = 2;
        }
        if self.tira.centrar == 0 {
            return;
        }
        let Some(indice) = tira.iter().position(|p| *p == self.posicao) else {
            self.tira.centrar = 0;
            return;
        };
        let caixa = self.rolagem_da_tira.bounds();
        if caixa.size.width > px(0.) {
            // 🔑 **Por conta, e não pelo leiaute**: o item pode nem ter virado
            // elemento (ver `faixa_desenhada`). A largura é fixa, e o centro
            // dele fica a `VAO + i·passo + largura/2` do começo da faixa.
            let centro = VAO + indice as f32 * (largura + VAO) + largura / 2.;
            let alvo = px(f32::from(caixa.size.width) / 2. - centro);
            let maximo = self.rolagem_da_tira.max_offset().width;
            let x = alvo.min(px(0.)).max(-maximo);
            let atual = self.rolagem_da_tira.offset();
            self.rolagem_da_tira.set_offset(gpui::point(x, atual.y));
        }
        self.tira.centrar -= 1;
        cx.on_next_frame(window, |_tela, _window, cx| cx.notify());
    }

    /// A faixa do rodapé: recortes, puxador e miniaturas.
    ///
    /// ⚠️ **Some com um acervo de uma foto**: uma faixa com um item só ocupa
    /// espaço da foto para não dizer nada.
    pub(super) fn filmstrip(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.acervo.len() < 2 {
            return None;
        }
        self.miniaturas_da_tira.ajustar_capacidade(
            NonZeroUsize::new(self.acervo.len().clamp(1, MINIATURAS_DA_TIRA))
                .expect("o piso 1 garante que não é zero"),
        );
        // 🔑 **O quadro só lê.** Quem carrega é [`Self::carregar_a_tira`].

        let tira = self.na_tira();
        let lado = altura_da_tira::lado_da_miniatura(self.tira.altura);
        let largura = (lado * altura_da_tira::PROPORCAO).round();
        self.centralizar_a_aberta(&tira, largura, window, cx);

        // Só o pedaço à vista vira elemento (`faixa_desenhada`). No primeiro
        // quadro a faixa ainda não tem medida, e a janela é o teto dela.
        let passo = largura + VAO;
        let vista = match f32::from(self.rolagem_da_tira.bounds().size.width) {
            v if v > 0. => v,
            _ => f32::from(window.viewport_size().width),
        };
        let (de, ate) = faixa_desenhada(
            tira.len(),
            passo,
            -f32::from(self.rolagem_da_tira.offset().x),
            vista,
        );
        self.tira.primeira_desenhada = de;
        let espacador = |itens: usize| {
            div()
                .flex_none()
                .w(px(itens as f32 * passo - VAO))
                .h(px(1.))
                .into_any_element()
        };
        let mut itens: Vec<AnyElement> = Vec::with_capacity(ate - de + 2);
        if de > 0 {
            itens.push(espacador(de));
        }
        itens.extend(
            tira[de..ate].iter().enumerate().map(|(i, posicao)| {
                self.miniatura_na_tira(*posicao, de + i + 1, lado, largura, cx)
            }),
        );
        if ate < tira.len() {
            itens.push(espacador(tira.len() - ate));
        }

        let deslocamento = -self.rolagem_da_tira.offset().x;
        let maximo = self.rolagem_da_tira.max_offset().width;
        let tem_antes = deslocamento > px(4.);
        let tem_depois = maximo > px(4.) && deslocamento < maximo - px(4.);

        let esta = cx.entity().downgrade();
        let faixa = div()
            .relative()
            .flex_none()
            .child(
                div()
                    .id("faixa-da-revelacao")
                    .track_scroll(&self.rolagem_da_tira)
                    .flex()
                    .items_center()
                    .gap(px(VAO))
                    .px(px(VAO))
                    .h(px(lado + 12.0))
                    .overflow_x_scroll()
                    // 🔑 A roda vertical rola na horizontal — o `wheel` com
                    // `passive: false` do site.
                    .on_scroll_wheel(cx.listener(
                        |tela, evento: &gpui::ScrollWheelEvent, window, cx| {
                            let delta = evento.delta.pixel_delta(window.line_height());
                            if delta.y.abs() <= delta.x.abs() {
                                return;
                            }
                            let maximo = tela.rolagem_da_tira.max_offset().width;
                            let atual = tela.rolagem_da_tira.offset();
                            let x = (atual.x + delta.y).min(px(0.)).max(-maximo);
                            tela.rolagem_da_tira.set_offset(gpui::point(x, atual.y));
                            cx.stop_propagation();
                            cx.notify();
                        },
                    ))
                    .children(itens),
            )
            .when(tem_antes, |f| f.child(self.seta_da_tira(true, cx)))
            .when(tem_depois, |f| f.child(self.seta_da_tira(false, cx)))
            // 🔑 **Um menu para a faixa inteira.** A miniatura do botão direito
            // anota quem foi clicada; o menu é montado depois do evento, e lê.
            // No vão entre duas, não há alvo e o menu não abre.
            .context_menu(move |menu, _window, cx| {
                let Some(dados) = esta
                    .update(cx, |tela, _cx| {
                        tela.tira.menu.take().and_then(|p| tela.menu_da_tira(p))
                    })
                    .ok()
                    .flatten()
                else {
                    return menu;
                };
                montar_o_menu(menu, dados, esta.clone())
            });

        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .child(self.barra_de_recortes(cx))
                .child(self.puxador_da_tira(cx))
                .child(faixa)
                .children(self.tira.menu_do_roteiro.as_ref().map(|(menu, ponto)| {
                    gpui::deferred(gpui::anchored().position(*ponto).child(menu.clone()))
                        .with_priority(1)
                }))
                .into_any_element(),
        )
    }
}

impl Revelacao {
    /// Um gesto do roteiro de depuração (`tira …`). Devolve onde dar o botão
    /// direito, quando o gesto é `menu`.
    pub fn seguir_o_roteiro_da_tira(
        &mut self,
        gesto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut partes = gesto.split_whitespace();
        let nome = partes.next().unwrap_or_default();
        let argumento = partes.next().unwrap_or_default();
        let numero = argumento.parse::<f32>().unwrap_or(0.0);
        let tira = self.na_tira();
        let na_posicao = tira.get(numero as usize).copied();
        let ctrl = Modificadores {
            aditivo: true,
            faixa: false,
        };
        let shift = Modificadores {
            aditivo: false,
            faixa: true,
        };
        match nome {
            "recorte" => {
                let filtro = match argumento {
                    "classificadas" => Filtro::Classificadas,
                    "sinalizadas" => Filtro::Situacao(Estado::LevadaNoBalcao),
                    "avenda" => Filtro::Situacao(Estado::Disponivel),
                    "semnota" => Filtro::SemNota,
                    "compradas" => Filtro::Situacao(Estado::Comprada),
                    _ => Filtro::Todas,
                };
                self.recortar(filtro, cx);
            }
            "altura" => {
                self.tira.altura = altura_da_tira::limitar(numero);
                cx.notify();
            }
            "marcar" | "faixa" | "abrir" => {
                let modificadores = match nome {
                    "marcar" => ctrl,
                    "faixa" => shift,
                    _ => Modificadores::default(),
                };
                if let Some(posicao) = na_posicao {
                    self.clicar_na_tira(posicao, modificadores, window, cx);
                }
            }
            "rolar" => {
                let maximo = self.rolagem_da_tira.max_offset().width;
                let atual = self.rolagem_da_tira.offset();
                let x = px(-numero).min(px(0.)).max(-maximo);
                self.rolagem_da_tira.set_offset(gpui::point(x, atual.y));
                cx.notify();
            }
            // ⚠️ O `ContextMenu` não abre por fora, e o `dispatch_event` da
            // janela não é público: o roteiro monta **o mesmo** menu e o
            // desenha sobre a miniatura.
            "menu" => {
                // O item é o `indice` da tira, mas o elemento conta a partir do
                // primeiro desenhado — com o espaçador na frente, se houver.
                let primeira = self.tira.primeira_desenhada;
                let elemento = (numero as usize)
                    .checked_sub(primeira)
                    .map(|k| k + usize::from(primeira > 0));
                let (Some(posicao), Some(item)) = (
                    na_posicao,
                    elemento.and_then(|k| self.rolagem_da_tira.bounds_for_item(k)),
                ) else {
                    return;
                };
                let Some(dados) = self.menu_da_tira(posicao) else {
                    return;
                };
                let esta = cx.entity().downgrade();
                let menu =
                    gpui_component::menu::PopupMenu::build(window, cx, move |menu, _w, _cx| {
                        montar_o_menu(menu, dados, esta)
                    });
                let ponto = item.center() + self.rolagem_da_tira.offset();
                self.tira.menu_do_roteiro = Some((menu, ponto));
                cx.notify();
            }
            "fechar" => {
                self.tira.menu_do_roteiro = None;
                cx.notify();
            }
            outro => eprintln!("[roteiro] gesto da tira desconhecido: {outro}"),
        }
    }
}

/// O menu do botão direito (`menuDaFoto` do `editor.tsx`).
///
/// 🔑 Curto de propósito: as ações de acervo (nota, levada, apagar) são da
/// galeria; aqui o assunto é o lote que vai receber os ajustes.
fn montar_o_menu(
    menu: gpui_component::menu::PopupMenu,
    dados: Menu,
    esta: gpui::WeakEntity<Revelacao>,
) -> gpui_component::menu::PopupMenu {
    let clicada = dados.clicada;
    let n_alvos = dados.alvos.len();
    let com = |f: fn(&mut Revelacao, &mut Window, &mut Context<Revelacao>)| {
        let esta = esta.clone();
        move |_ev: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
            let _ = esta.update(cx, |tela, cx| f(tela, window, cx));
        }
    };
    let alvos_zerar = dados.alvos.clone();
    let alvos_baixar = dados.alvos;
    let (para_zerar, para_baixar) = (esta.clone(), esta.clone());
    let (para_abrir, para_escolher) = (esta.clone(), esta.clone());

    menu.label(dados.arquivo)
        .separator()
        .item(
            PopupMenuItem::new("Abrir esta foto")
                .disabled(dados.e_a_aberta)
                .on_click(move |_ev, window, cx| {
                    let _ = para_abrir.update(cx, |tela, cx| tela.ir_para(clicada, window, cx));
                }),
        )
        .item(
            PopupMenuItem::new(if dados.na_escolha && !dados.e_a_aberta {
                "Tirar da escolha"
            } else {
                "Escolher também"
            })
            .on_click(move |_ev, window, cx| {
                let _ = para_escolher.update(cx, |tela, cx| {
                    tela.clicar_na_tira(
                        clicada,
                        Modificadores {
                            aditivo: true,
                            faixa: false,
                        },
                        window,
                        cx,
                    )
                });
            }),
        )
        .item(
            PopupMenuItem::new("Escolher todas")
                .on_click(com(|tela, _window, cx| tela.marcar_todas(cx))),
        )
        .item(
            PopupMenuItem::new("Desmarcar todas")
                .on_click(com(|tela, _window, cx| tela.desmarcar(cx))),
        )
        .separator()
        .item(
            PopupMenuItem::new(if n_alvos > 1 {
                format!("Baixar como… ({n_alvos})")
            } else {
                "Baixar como…".to_string()
            })
            .disabled(n_alvos == 0)
            .on_click(move |_ev, window, cx| {
                let alvos = alvos_baixar.clone();
                let _ = para_baixar.update(cx, |tela, cx| tela.baixar_pelo_menu(alvos, window, cx));
            }),
        )
        .separator()
        .item(
            PopupMenuItem::new(format!("Sincronizar {} com esta", dados.marcadas))
                .disabled(!dados.pode_sincronizar)
                .on_click(com(|tela, window, cx| tela.abrir_sincronizacao(window, cx))),
        )
        .item(
            PopupMenuItem::new(if dados.quantas_zeram > 1 {
                format!("Zerar {} fotos", dados.quantas_zeram)
            } else {
                "Zerar tudo".to_string()
            })
            .disabled(dados.quantas_zeram == 0)
            .on_click(move |_ev, window, cx| {
                let alvos = alvos_zerar.clone();
                let _ = para_zerar.update(cx, |tela, cx| tela.zerar_pelo_menu(alvos, window, cx));
            }),
        )
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto(nome: &str, nota: i32, comprada: bool, travada: bool) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("site:{nome}"),
            name: nome.into(),
            rating: nota,
            comprada,
            revelacao_travada: travada,
            pos_venda_foto_id: Some(nome.into()),
            ..Default::default()
        }
    }

    /// a: à venda ★3 · b: sem nota · c: levada ★5 · d: comprada ★2
    fn acervo() -> Vec<PhotoViewModel> {
        vec![
            foto("a", 3, false, false),
            foto("b", 0, false, false),
            foto("c", 5, true, false),
            foto("d", 2, true, true),
        ]
    }

    #[test]
    fn a_faixa_desenhada_cobre_a_vista_e_uma_tela_de_cada_lado() {
        // 10.000 itens de 100 px (passo 108), vista de 1.000 px.
        let passo = 108.;
        assert_eq!(faixa_desenhada(10_000, passo, 0., 1000.), (0, 20));
        let (de, ate) = faixa_desenhada(10_000, passo, 540_000., 1000.);
        assert!(de * 108 <= 540_000 - 1000, "uma tela antes");
        assert!(ate as f32 * passo >= 540_000. + 2000., "uma tela depois");
        assert!(ate - de < 40, "{de}..{ate}: só o que se vê");
        // No fim, não passa do total.
        assert_eq!(faixa_desenhada(10_000, passo, 1e9, 1000.).1, 10_000);
        // Sem medida, tudo (o caminho seguro).
        assert_eq!(faixa_desenhada(50, 0., 0., 1000.), (0, 50));
        assert_eq!(faixa_desenhada(50, passo, f32::NAN, 1000.), (0, 50));
        assert_eq!(faixa_desenhada(0, passo, 0., 1000.), (0, 0));
    }

    #[test]
    fn a_situacao_sai_dos_dois_campos_do_view_model() {
        let a = acervo();
        assert_eq!(classificacao(&a[0]).estado, Estado::Disponivel);
        assert_eq!(classificacao(&a[1]).nota, None);
        assert_eq!(classificacao(&a[2]).estado, Estado::LevadaNoBalcao);
        assert_eq!(classificacao(&a[3]).estado, Estado::Comprada);
        assert!(!classificacao(&a[3]).editavel());
        let apagada = foto("e", 4, false, true);
        assert!(classificacao(&apagada).apagada);
    }

    /// Os números dos chips são os do core — "À venda" não conta a sem nota.
    #[test]
    fn os_chips_contam_como_a_galeria() {
        let c = contar(&acervo());
        assert_eq!(c.de(Filtro::Todas), 4);
        assert_eq!(c.de(Filtro::Classificadas), 3);
        assert_eq!(c.de(Filtro::Situacao(Estado::LevadaNoBalcao)), 1);
        assert_eq!(c.de(Filtro::Situacao(Estado::Disponivel)), 1);
        assert_eq!(c.de(Filtro::SemNota), 1);
    }

    /// 🔑 A aberta nunca some da tira (`fotosDoRecorte`).
    #[test]
    fn o_recorte_mantem_a_aberta() {
        let a = acervo();
        assert_eq!(na_tira(&a, Filtro::SemNota, 0), vec![0, 1]);
        assert_eq!(na_tira(&a, Filtro::SemNota, 1), vec![1]);
        assert_eq!(na_tira(&a, Filtro::Todas, 0), vec![0, 1, 2, 3]);
        assert_eq!(na_tira(&a, Filtro::Classificadas, 1), vec![0, 1, 2, 3]);
    }

    #[test]
    fn as_setas_andam_sobre_o_recorte() {
        let tira = vec![0, 2, 3];
        assert_eq!(vizinha(&tira, 0, 1), Some(2));
        assert_eq!(vizinha(&tira, 2, -1), Some(0));
        assert_eq!(vizinha(&tira, 0, -1), None, "sem dar a volta");
        assert_eq!(vizinha(&tira, 3, 1), None);
        assert_eq!(vizinha(&tira, 1, 1), None, "fora da tira não anda");
    }

    /// `selecaoAoTrocar`: dentro do lote ele fica; fora, recomeça.
    #[test]
    fn o_lote_fica_ao_andar_dentro_dele() {
        let lote = BTreeSet::from([1, 3, 4]);
        assert_eq!(ao_trocar(&lote, 3), lote);
        assert_eq!(ao_trocar(&lote, 2), BTreeSet::from([2]));
    }

    #[test]
    fn o_shift_so_pega_o_que_a_tira_mostra() {
        let tira = vec![0, 2, 5, 7];
        assert_eq!(faixa_na_tira(&tira, 2, 7), Some(BTreeSet::from([2, 5, 7])));
        assert_eq!(
            faixa_na_tira(&tira, 7, 0),
            Some(BTreeSet::from([0, 2, 5, 7]))
        );
        assert_eq!(faixa_na_tira(&tira, 2, 3), None);
    }

    #[test]
    fn escolher_todas_soma_a_tira_ao_lote() {
        let lote = BTreeSet::from([9]);
        assert_eq!(marcar_a_tira(&lote, &[0, 2]), BTreeSet::from([0, 2, 9]));
    }

    /// O alvo do menu é a seleção quando a clicada faz parte dela.
    #[test]
    fn o_menu_age_sobre_a_selecao_ou_so_na_clicada() {
        let lote = BTreeSet::from([0, 2, 3]);
        let tira = vec![0, 1, 2, 3];
        assert_eq!(alvos_do_menu(&lote, &tira, 2), vec![0, 2, 3]);
        assert_eq!(alvos_do_menu(&lote, &tira, 1), vec![1]);
        // A marcada que o recorte esconde não vai junto.
        assert_eq!(alvos_do_menu(&lote, &[0, 2], 0), vec![0, 2]);
    }

    /// `selecaoAoAbrir`: a seleção da sessão vale se inclui a aberta.
    #[test]
    fn a_selecao_da_sessao_entra_se_incluir_a_aberta() {
        let a = acervo();
        let da_sessao = vec!["a".to_string(), "c".to_string(), "zz".to_string()];
        assert_eq!(selecao_ao_abrir(&a, 2, &da_sessao), BTreeSet::from([0, 2]));
        assert_eq!(selecao_ao_abrir(&a, 1, &da_sessao), BTreeSet::from([1]));
        assert_eq!(selecao_ao_abrir(&a, 1, &[]), BTreeSet::from([1]));
    }

    #[test]
    fn a_dica_diz_em_palavras_o_que_as_marcas_dizem() {
        let a = acervo();
        assert_eq!(
            dica(&a[2], 3, false, true, true),
            "3. c — 5 estrelas — levada no balcão — editada aqui, não salva — escolhida para sincronizar"
        );
        assert_eq!(
            dica(&a[1], 2, true, true, false),
            "2. b — sem nota — à venda"
        );
        assert_eq!(
            dica(&a[3], 4, false, false, false),
            "4. d — 2 estrelas — comprada, não se revela"
        );
    }
}
