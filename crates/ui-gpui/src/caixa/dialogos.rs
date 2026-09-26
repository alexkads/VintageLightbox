//! Os diálogos do PDV — um por operação do caixa, cada um inteiro no teclado.
//! É o `pdv-dialogos.tsx` do site, e as gravações do `usar-pdv.tsx`.
//!
//! 🪟 **Uma camada absoluta sobre a tela, e não um `deferred`.** O GPUI recusa
//! `deferred` dentro de `deferred` (entra em pânico), e o campo de texto do
//! `gpui-component` abre o menu do botão direito com um. Um diálogo adiado com
//! campo dentro derrubaria o app no primeiro clique direito — então ele é
//! desenhado por último dentro da própria tela, como os modais da raiz.
//!
//! Quem grava é a tela (`gravar`); o diálogo só lê o teclado, confere o formato
//! com as regras do core e fecha quando a resposta diz que foi.

use std::collections::HashSet;

use biblioteca_core::caixa::{
    self as regras, ExtraDaVenda, FormaDePagamento, ModoDoDesconto, PagamentoLancado, Pessoas,
    TipoDeMovimento, Venda, BANDEIRAS,
};
use biblioteca_core::dinheiro;
use biblioteca_core::negociacao::{self, Negociacao, Tipo, PARCEIROS};
use gpui::{
    div, prelude::*, px, relative, AnyElement, ClickEvent, Context, Div, Entity, FocusHandle,
    Focusable, FontWeight, KeyContext, MouseButton, SharedString, Stateful, Subscription, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, Sizable};
use serde_json::json;

use super::dados::{self, CaixaDoBalcao, Conferencia};
use super::tela::{
    codificar, com_tecla, Caixa, ConcluirVenda, ConfirmarDialogo, DescontoNoTotal, EmPercentual,
    EmReais, FecharDialogo, Forma1, Forma2, Forma3, Forma4, Forma5, Forma6, Forma7, Forma8,
    Sangria, Suprimento, TipoDeRecado, TirarUltimo, DIALOGO,
};
use crate::estilo;
use crate::recursos::Icone;
use crate::tema::cores;

/// Qual diálogo abrir — os `DialogoDoPdv` do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoDeDialogo {
    Atalhos,
    Abrir,
    Desconto,
    Pessoas,
    Pagamento,
    Movimento,
    Vendas,
    Fechar,
}

pub(super) struct FormAbrir {
    fundo: Entity<InputState>,
    erro: Option<String>,
    enviando: bool,
}

pub(super) struct FormDesconto {
    modo: ModoDoDesconto,
    texto: Entity<InputState>,
    motivo: Entity<InputState>,
}

/// Uma opção do `ComboBox` de funcionários.
#[derive(Clone)]
pub(super) struct OpcaoDeFuncionario {
    id: String,
    nome: SharedString,
}

impl SelectItem for OpcaoDeFuncionario {
    type Value = String;

    fn title(&self) -> SharedString {
        self.nome.clone()
    }

    fn value(&self) -> &String {
        &self.id
    }
}

type Escolha = SelectState<SearchableVec<OpcaoDeFuncionario>>;

pub(super) struct FormCadastro {
    papel: usize,
    nome: Entity<InputState>,
    whatsapp: Entity<InputState>,
    email: Entity<InputState>,
    erro: Option<String>,
    enviando: bool,
}

pub(super) struct FormPessoas {
    /// Fotografou, atendeu, auxiliou — nessa ordem.
    escolhas: [Entity<Escolha>; 3],
    valores: [Option<String>; 3],
    erro: Option<String>,
    cadastro: Option<FormCadastro>,
    /// Um cadastrado agora, a mostrar escolhido no próximo quadro (a lista do
    /// `Select` só muda com a janela na mão).
    pendente: Option<(usize, String)>,
    _assinaturas: Vec<Subscription>,
}

pub(super) struct FormPagamento {
    lancados: Vec<PagamentoLancado>,
    forma: Option<FormaDePagamento>,
    valor: Entity<InputState>,
    detalhe: Entity<InputState>,
    bandeira: Option<&'static str>,
    erro: Option<String>,
    enviando: bool,
}

pub(super) struct FormMovimento {
    tipo: TipoDeMovimento,
    valor: Entity<InputState>,
    motivo: Entity<InputState>,
    erro: Option<String>,
    enviando: bool,
}

pub(super) struct FormEstorno {
    venda: Venda,
    escolhidas: HashSet<String>,
    valor: Entity<InputState>,
    /// O que a tela escreveu no valor. Diferente do campo = o operador digitou
    /// outro (o `valorTexto !== null` do site).
    escrito: String,
    forma: FormaDePagamento,
    detalhe: Entity<InputState>,
    motivo: Entity<InputState>,
    des_sinalizar: bool,
    erro: Option<String>,
    enviando: bool,
}

pub(super) struct FormFechamento {
    campos: Vec<(FormaDePagamento, Entity<InputState>)>,
    observacao: Entity<InputState>,
    conferencia: Option<Conferencia>,
    resultado: Option<CaixaDoBalcao>,
    erro: Option<String>,
    enviando: bool,
}

/// A negociação completa (`negociacao-dialog.tsx`), aberta pelo `N` do painel.
pub(super) struct FormNegociacao {
    ids: Vec<String>,
    titulo: String,
    tipo: Tipo,
    parceiro: String,
    cupom: Entity<InputState>,
    preco: Entity<InputState>,
    motivo: Entity<InputState>,
    preco_da_faixa: Option<i64>,
    existente: bool,
    /// "Remover a negociação?" à vista, dentro do diálogo.
    confirmando: bool,
    erro: Option<String>,
    pub(super) enviando: bool,
}

pub enum Dialogo {
    Atalhos,
    Abrir(FormAbrir),
    Desconto(FormDesconto),
    Pessoas(Box<FormPessoas>),
    Pagamento(FormPagamento),
    Movimento(FormMovimento),
    Vendas,
    Estorno(Box<FormEstorno>),
    Fechar(Box<FormFechamento>),
    Negociacao(Box<FormNegociacao>),
}

impl Dialogo {
    fn contexto(&self) -> &'static str {
        match self {
            Dialogo::Atalhos => "Atalhos",
            Dialogo::Abrir(_) => "Abrir",
            Dialogo::Desconto(_) => "Desconto",
            Dialogo::Pessoas(_) => "Pessoas",
            Dialogo::Pagamento(_) => "Pagamento",
            Dialogo::Movimento(_) => "Movimento",
            Dialogo::Vendas => "Vendas",
            Dialogo::Estorno(_) => "Estorno",
            Dialogo::Fechar(_) => "Fechamento",
            Dialogo::Negociacao(_) => "Negociacao",
        }
    }
}

/// O que uma gravação no ar precisa lembrar para dar o recibo.
pub(super) enum EmCurso {
    Abrir {
        fundo: i64,
    },
    Movimento {
        tipo: TipoDeMovimento,
        valor: i64,
    },
    Estorno {
        venda_id: String,
        numero: i64,
        valor: i64,
        fotos: Vec<String>,
        des_sinalizar: bool,
    },
}

/// A volta das fotos estornadas a "à venda", uma a uma.
pub(super) struct DesSinalizacao {
    numero: i64,
    valor: i64,
    faltam: usize,
    feitas: usize,
    falhas: usize,
}

fn campo(window: &mut Window, cx: &mut Context<Caixa>, dica: &str) -> Entity<InputState> {
    let dica = dica.to_string();
    cx.new(|cx| InputState::new(window, cx).placeholder(dica))
}

fn valor_de(campo: &Entity<InputState>, cx: &Context<Caixa>) -> String {
    campo.read(cx).value().to_string()
}

fn escrever(
    campo: &Entity<InputState>,
    texto: String,
    window: &mut Window,
    cx: &mut Context<Caixa>,
) {
    campo.update(cx, |c, cx| c.set_value(texto, window, cx));
}

impl Caixa {
    pub(super) fn abrir_dialogo(
        &mut self,
        tipo: TipoDeDialogo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.depois_das_pessoas = false;
        let (dialogo, foco): (Dialogo, Option<FocusHandle>) = match tipo {
            TipoDeDialogo::Atalhos => (Dialogo::Atalhos, None),
            TipoDeDialogo::Vendas => (Dialogo::Vendas, None),
            TipoDeDialogo::Abrir => {
                let fundo = campo(window, cx, "0,00");
                let foco = fundo.read(cx).focus_handle(cx);
                (
                    Dialogo::Abrir(FormAbrir {
                        fundo,
                        erro: None,
                        enviando: false,
                    }),
                    Some(foco),
                )
            }
            TipoDeDialogo::Desconto => {
                let DescontoNoTotal {
                    modo,
                    texto,
                    motivo,
                } = self.desconto.clone();
                let campo_texto = campo(
                    window,
                    cx,
                    if modo == ModoDoDesconto::Valor {
                        "0,00"
                    } else {
                        "10"
                    },
                );
                escrever(&campo_texto, texto, window, cx);
                let campo_motivo = campo(window, cx, "");
                escrever(&campo_motivo, motivo, window, cx);
                let foco = campo_texto.read(cx).focus_handle(cx);
                (
                    Dialogo::Desconto(FormDesconto {
                        modo,
                        texto: campo_texto,
                        motivo: campo_motivo,
                    }),
                    Some(foco),
                )
            }
            TipoDeDialogo::Pessoas => {
                let form = self.form_de_pessoas(window, cx);
                let foco = form.escolhas[0].read(cx).focus_handle(cx);
                (Dialogo::Pessoas(Box::new(form)), Some(foco))
            }
            TipoDeDialogo::Pagamento => (
                Dialogo::Pagamento(FormPagamento {
                    lancados: Vec::new(),
                    forma: None,
                    valor: campo(window, cx, "0,00"),
                    detalhe: campo(window, cx, ""),
                    bandeira: None,
                    erro: None,
                    enviando: false,
                }),
                None,
            ),
            TipoDeDialogo::Movimento => {
                let valor = campo(window, cx, "0,00");
                let foco = valor.read(cx).focus_handle(cx);
                (
                    Dialogo::Movimento(FormMovimento {
                        tipo: TipoDeMovimento::Sangria,
                        valor,
                        motivo: campo(window, cx, ""),
                        erro: None,
                        enviando: false,
                    }),
                    Some(foco),
                )
            }
            TipoDeDialogo::Fechar => {
                let campos: Vec<_> = FormaDePagamento::TODAS
                    .into_iter()
                    .map(|f| (f, campo(window, cx, "0,00")))
                    .collect();
                let foco = campos[0].1.read(cx).focus_handle(cx);
                (
                    Dialogo::Fechar(Box::new(FormFechamento {
                        campos,
                        observacao: campo(window, cx, ""),
                        conferencia: None,
                        resultado: None,
                        erro: None,
                        enviando: false,
                    })),
                    Some(foco),
                )
            }
        };
        self.lembrar_foco(window, cx);
        self.dialogo = Some(dialogo);
        window.focus(&foco.unwrap_or_else(|| self.foco_do_dialogo.clone()));
        cx.notify();
    }

    /// Quem tinha o foco antes do primeiro diálogo — é para lá que ele volta.
    fn lembrar_foco(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogo.is_none() {
            self.foco_antes.esquecer();
            self.foco_antes.lembrar(window, cx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn abrir_dialogo_de_negociacao(
        &mut self,
        ids: Vec<String>,
        titulo: String,
        inicial: Negociacao,
        preco_da_faixa: Option<i64>,
        existente: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cupom = campo(window, cx, "o código que o cliente trouxe");
        escrever(&cupom, inicial.cupom.clone(), window, cx);
        let preco = campo(window, cx, "0,00");
        if let (Some(p), true) = (inicial.preco, inicial.tipo != Tipo::Cortesia) {
            escrever(&preco, dinheiro::formatar_campo(p), window, cx);
        }
        let motivo = campo(window, cx, "");
        escrever(&motivo, inicial.motivo.clone(), window, cx);
        let foco = match inicial.tipo {
            Tipo::Desconto => preco.read(cx).focus_handle(cx),
            Tipo::Parceiro => cupom.read(cx).focus_handle(cx),
            _ => motivo.read(cx).focus_handle(cx),
        };
        let parceiro = if inicial.parceiro.is_empty() {
            PARCEIROS[0].to_string()
        } else {
            inicial.parceiro.clone()
        };
        self.lembrar_foco(window, cx);
        self.dialogo = Some(Dialogo::Negociacao(Box::new(FormNegociacao {
            ids,
            titulo,
            tipo: inicial.tipo,
            parceiro,
            cupom,
            preco,
            motivo,
            preco_da_faixa,
            existente,
            confirmando: false,
            erro: None,
            enviando: false,
        })));
        window.focus(&foco);
        cx.notify();
    }

    fn salvar_negociacao(&mut self, cx: &mut Context<Self>) {
        let Some(Dialogo::Negociacao(form)) = self.dialogo.as_mut() else {
            return;
        };
        if form.enviando {
            return;
        }
        if form.confirmando {
            form.enviando = true;
            form.confirmando = false;
            let ids = form.ids.clone();
            self.gravar_negociacao_do_dialogo(ids, None, cx);
            cx.notify();
            return;
        }
        let pede_preco = form.tipo != Tipo::Cortesia;
        let texto = valor_de(&form.preco, cx);
        let mut preco = None;
        if pede_preco && !texto.trim().is_empty() {
            match dinheiro::ler_campo(&texto) {
                Some(p) => preco = Some(p),
                None => {
                    form.erro = Some("Valor inválido. Use o formato 15,00.".into());
                    cx.notify();
                    return;
                }
            }
        }
        let n = Negociacao {
            tipo: form.tipo,
            preco: if form.tipo == Tipo::Cortesia {
                Some(0)
            } else {
                preco
            },
            parceiro: form.parceiro.clone(),
            cupom: valor_de(&form.cupom, cx),
            motivo: valor_de(&form.motivo, cx),
        };
        match negociacao::montar(&n) {
            Err(erro) => form.erro = Some(erro),
            Ok(valor) => {
                form.erro = None;
                form.enviando = true;
                let ids = form.ids.clone();
                self.gravar_negociacao_do_dialogo(ids, Some(valor), cx);
            }
        }
        cx.notify();
    }

    fn form_de_pessoas(&mut self, window: &mut Window, cx: &mut Context<Self>) -> FormPessoas {
        let opcoes: Vec<OpcaoDeFuncionario> = self
            .todos_os_funcionarios()
            .into_iter()
            .filter(|f| f.ativo)
            .map(|f| OpcaoDeFuncionario {
                id: f.id,
                nome: f.nome.into(),
            })
            .collect();
        let validas = self.pessoas_validas();
        let valores = [validas.fotografo, validas.atendente, validas.auxiliar];
        let mut assinaturas = Vec::new();
        let escolhas: [Entity<Escolha>; 3] = std::array::from_fn(|papel| {
            let lista = SearchableVec::new(opcoes.clone());
            let valor = valores[papel].clone();
            let escolha = cx.new(|cx| {
                let mut estado = SelectState::new(lista, None, window, cx).searchable(true);
                if let Some(valor) = &valor {
                    estado.set_selected_value(valor, window, cx);
                }
                estado
            });
            assinaturas.push(cx.subscribe_in(
                &escolha,
                window,
                move |tela, _, evento: &SelectEvent<SearchableVec<OpcaoDeFuncionario>>, _, cx| {
                    let SelectEvent::Confirm(valor) = evento;
                    if let Some(Dialogo::Pessoas(form)) = tela.dialogo.as_mut() {
                        form.valores[papel] = valor.clone();
                        form.erro = None;
                        cx.notify();
                    }
                },
            ));
            escolha
        });
        FormPessoas {
            escolhas,
            valores,
            erro: None,
            cadastro: None,
            pendente: None,
            _assinaturas: assinaturas,
        }
    }

    pub(super) fn fechar_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Fechar o cadastro volta às pessoas, como o diálogo de cima do site.
        if let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() {
            if form.cadastro.take().is_some() {
                let foco = form.escolhas[0].read(cx).focus_handle(cx);
                window.focus(&foco);
                cx.notify();
                return;
            }
        }
        self.dialogo = None;
        self.depois_das_pessoas = false;
        if self.foco_antes.guardado() {
            self.foco_antes.devolver(window);
        } else {
            window.focus(&self.foco);
        }
        cx.notify();
    }

    /// O que precisa da janela e nasceu sem ela (numa resposta da rede).
    pub(super) fn aplicar_pendencias_do_dialogo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() else {
            return;
        };
        let Some((papel, id)) = form.pendente.take() else {
            return;
        };
        let opcoes: Vec<OpcaoDeFuncionario> = self
            .todos_os_funcionarios()
            .into_iter()
            .filter(|f| f.ativo)
            .map(|f| OpcaoDeFuncionario {
                id: f.id,
                nome: f.nome.into(),
            })
            .collect();
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() else {
            return;
        };
        for (i, escolha) in form.escolhas.iter().enumerate() {
            let lista = SearchableVec::new(opcoes.clone());
            let valor = if i == papel {
                Some(id.clone())
            } else {
                form.valores[i].clone()
            };
            escolha.update(cx, |estado, cx| {
                estado.set_items(lista, window, cx);
                match &valor {
                    Some(v) => estado.set_selected_value(v, window, cx),
                    None => estado.set_selected_index(None, window, cx),
                }
            });
        }
        form.valores[papel] = Some(id);
        let foco = form.escolhas[papel].read(cx).focus_handle(cx);
        window.focus(&foco);
    }

    /// Escreve num campo do diálogo aberto, pelo nome — só os testes.
    #[cfg(test)]
    pub(super) fn preencher(
        &mut self,
        nome: &str,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let campo = match (self.dialogo.as_ref(), nome) {
            (Some(Dialogo::Abrir(f)), "fundo") => f.fundo.clone(),
            (Some(Dialogo::Desconto(f)), "texto") => f.texto.clone(),
            (Some(Dialogo::Movimento(f)), "valor") => f.valor.clone(),
            (Some(Dialogo::Movimento(f)), "motivo") => f.motivo.clone(),
            (Some(Dialogo::Pagamento(f)), "valor") => f.valor.clone(),
            (Some(Dialogo::Estorno(f)), "motivo") => f.motivo.clone(),
            (Some(Dialogo::Estorno(f)), "valor") => f.valor.clone(),
            (Some(Dialogo::Fechar(f)), forma) => f
                .campos
                .iter()
                .find(|(fp, _)| fp.chave() == forma)
                .expect("a forma do fechamento")
                .1
                .clone(),
            _ => panic!("o diálogo aberto não tem o campo {nome}"),
        };
        escrever(&campo, texto.to_string(), window, cx);
    }

    /// Qual diálogo está aberto — só os testes.
    #[cfg(test)]
    pub(super) fn dialogo_aberto(&self) -> Option<&'static str> {
        self.dialogo.as_ref().map(Dialogo::contexto)
    }

    /// Os pagamentos lançados no diálogo aberto — só os testes.
    #[cfg(test)]
    pub(super) fn lancados(&self) -> Vec<PagamentoLancado> {
        match self.dialogo.as_ref() {
            Some(Dialogo::Pagamento(f)) => f.lancados.clone(),
            _ => Vec::new(),
        }
    }

    // ── O teclado dos diálogos ──────────────────────────────────────────────

    /// `Enter`: o `submit` do formulário aberto.
    pub(super) fn confirmar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.dialogo.as_ref() {
            Some(Dialogo::Abrir(_)) => self.enviar_abertura(cx),
            Some(Dialogo::Desconto(_)) => self.aplicar_desconto(window, cx),
            Some(Dialogo::Pessoas(form)) => {
                if form.cadastro.is_some() {
                    self.cadastrar(cx)
                } else {
                    self.salvar_pessoas(window, cx)
                }
            }
            Some(Dialogo::Pagamento(form)) => {
                if form.forma.is_some() {
                    self.lancar(window, cx);
                } else {
                    self.concluir_venda(window, cx);
                }
            }
            Some(Dialogo::Movimento(_)) => self.registrar_movimento(cx),
            Some(Dialogo::Estorno(_)) => self.estornar(cx),
            Some(Dialogo::Fechar(form)) => {
                if form.resultado.is_some() {
                } else if form.conferencia.is_some() {
                    self.fechar_caixa(cx);
                } else {
                    self.conferir_caixa(cx);
                }
            }
            Some(Dialogo::Negociacao(_)) => self.salvar_negociacao(cx),
            Some(Dialogo::Atalhos) | Some(Dialogo::Vendas) | None => {}
        }
    }

    fn escolher_forma(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(forma) = FormaDePagamento::da_tecla(n) {
            self.escolher_forma_de_pagamento(forma, window, cx);
        }
    }

    pub(super) fn tirar_ultimo(&mut self, cx: &mut Context<Self>) {
        if let Some(Dialogo::Pagamento(form)) = self.dialogo.as_mut() {
            form.lancados.pop();
            cx.notify();
        }
    }

    pub(super) fn mudar_modo(&mut self, modo: ModoDoDesconto, cx: &mut Context<Self>) {
        if let Some(Dialogo::Desconto(form)) = self.dialogo.as_mut() {
            form.modo = modo;
            cx.notify();
        }
    }

    pub(super) fn mudar_tipo(&mut self, tipo: TipoDeMovimento, cx: &mut Context<Self>) {
        if let Some(Dialogo::Movimento(form)) = self.dialogo.as_mut() {
            form.tipo = tipo;
            cx.notify();
        }
    }

    // ── Abrir o caixa ───────────────────────────────────────────────────────

    fn enviar_abertura(&mut self, cx: &mut Context<Self>) {
        let estudio = self.estudio_do_caixa().map(str::to_string);
        let Some(Dialogo::Abrir(form)) = self.dialogo.as_mut() else {
            return;
        };
        if form.enviando {
            return;
        }
        let fundo = match regras::ler_fundo_de_troco(&valor_de(&form.fundo, cx)) {
            Ok(f) => f,
            Err(erro) => {
                form.erro = Some(erro);
                cx.notify();
                return;
            }
        };
        let Some(estudio) = estudio else {
            self.avisar(
                "Escolha o estúdio da sessão antes de abrir o caixa.",
                TipoDeRecado::Erro,
                cx,
            );
            return;
        };
        form.erro = None;
        form.enviando = true;
        self.em_curso.push(EmCurso::Abrir { fundo });
        self.gravar(
            "abrir",
            "POST",
            "/pos-venda/caixa".into(),
            json!({ "estudio_id": estudio, "fundo_de_troco_centavos": fundo }),
            cx,
        );
        cx.notify();
    }

    // ── Desconto ────────────────────────────────────────────────────────────

    fn desconto_do_form(&self, cx: &Context<Self>) -> Option<DescontoNoTotal> {
        let Some(Dialogo::Desconto(form)) = self.dialogo.as_ref() else {
            return None;
        };
        Some(DescontoNoTotal {
            modo: form.modo,
            texto: valor_de(&form.texto, cx),
            motivo: valor_de(&form.motivo, cx),
        })
    }

    fn aplicar_desconto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(desconto) = self.desconto_do_form(cx) else {
            return;
        };
        let base = self.cupom().total;
        if regras::ler_desconto(&desconto.texto, desconto.modo, base).is_err() {
            return;
        }
        self.desconto = desconto;
        self.fechar_dialogo(window, cx);
    }

    // ── Pessoas ─────────────────────────────────────────────────────────────

    fn salvar_pessoas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() else {
            return;
        };
        let [fotografo, atendente, auxiliar] = form.valores.clone();
        let pessoas = Pessoas {
            fotografo,
            atendente,
            auxiliar,
        };
        if let Err(erro) = pessoas.conferir() {
            form.erro = Some(erro);
            cx.notify();
            return;
        }
        let seguir = self.depois_das_pessoas;
        self.guardar_pessoas(pessoas);
        self.fechar_dialogo(window, cx);
        // O F4 que pediu os nomes continua para o pagamento.
        if seguir {
            self.abrir_dialogo(TipoDeDialogo::Pagamento, window, cx);
        }
    }

    fn abrir_cadastro(&mut self, papel: usize, window: &mut Window, cx: &mut Context<Self>) {
        let nome = campo(window, cx, "");
        let whatsapp = campo(window, cx, "");
        let email = campo(window, cx, "");
        let foco = nome.read(cx).focus_handle(cx);
        if let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() {
            form.cadastro = Some(FormCadastro {
                papel,
                nome,
                whatsapp,
                email,
                erro: None,
                enviando: false,
            });
            window.focus(&foco);
            cx.notify();
        }
    }

    fn cadastrar(&mut self, cx: &mut Context<Self>) {
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() else {
            return;
        };
        let Some(cadastro) = form.cadastro.as_mut() else {
            return;
        };
        if cadastro.enviando {
            return;
        }
        let nome = valor_de(&cadastro.nome, cx);
        if nome.trim().is_empty() {
            cadastro.erro = Some("Informe o nome.".into());
            cx.notify();
            return;
        }
        cadastro.erro = None;
        cadastro.enviando = true;
        let corpo = dados::novo_funcionario_json(
            &nome,
            &valor_de(&cadastro.whatsapp, cx),
            &valor_de(&cadastro.email, cx),
        );
        self.gravar(
            "cadastrar",
            "POST",
            "/pos-venda/funcionarios".into(),
            corpo,
            cx,
        );
        cx.notify();
    }

    // ── Pagamento ───────────────────────────────────────────────────────────

    pub(super) fn escolher_forma_de_pagamento(
        &mut self,
        forma: FormaDePagamento,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let total = self.a_receber();
        let Some(Dialogo::Pagamento(form)) = self.dialogo.as_mut() else {
            return;
        };
        let conta = regras::conta_do_pagamento(total, &form.lancados);
        form.erro = None;
        form.forma = Some(forma);
        form.bandeira = None;
        let (valor, detalhe) = (form.valor.clone(), form.detalhe.clone());
        escrever(&detalhe, String::new(), window, cx);
        // A forma escolhida leva o foco ao valor, já com o que falta.
        let falta = if conta.falta > 0 {
            dinheiro::formatar_campo(conta.falta)
        } else {
            String::new()
        };
        escrever(&valor, falta, window, cx);
        let foco = valor.read(cx).focus_handle(cx);
        window.focus(&foco);
        cx.notify();
    }

    pub(super) fn lancar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.a_receber();
        let foco_do_dialogo = self.foco_do_dialogo.clone();
        let Some(Dialogo::Pagamento(form)) = self.dialogo.as_mut() else {
            return;
        };
        let Some(forma) = form.forma else {
            return;
        };
        match regras::lancar(
            forma,
            &valor_de(&form.valor, cx),
            &valor_de(&form.detalhe, cx),
            form.bandeira,
        ) {
            Ok(lancado) => {
                form.lancados.push(lancado);
                form.forma = None;
                form.bandeira = None;
                form.erro = None;
                let (valor, detalhe) = (form.valor.clone(), form.detalhe.clone());
                escrever(&valor, String::new(), window, cx);
                escrever(&detalhe, String::new(), window, cx);
                // Pronto: o `Enter` seguinte conclui. Faltando: `1`–`8`
                // escolhem a próxima forma. Os dois pedem o foco fora do campo.
                let _ = total;
                window.focus(&foco_do_dialogo);
            }
            Err(erro) => form.erro = Some(erro),
        }
        cx.notify();
    }

    pub(super) fn concluir_venda(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let total = self.a_receber();
        let desconto_no_total = self.desconto_no_total();
        let motivo = self.desconto.motivo.clone();
        let pessoas = self.pessoas_validas();
        let cupom = self.cupom();
        let galeria = self
            .vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| s.id.clone());
        let Some(Dialogo::Pagamento(form)) = self.dialogo.as_mut() else {
            return;
        };
        let conta = regras::conta_do_pagamento(total, &form.lancados);
        if !conta.pronto || form.enviando {
            return;
        }
        let Some(galeria) = galeria else {
            self.avisar("Escolha a sessão a cobrar.", TipoDeRecado::Informacao, cx);
            return;
        };
        form.enviando = true;
        let venda = regras::montar_venda(
            &galeria,
            &cupom,
            ExtraDaVenda {
                desconto_no_total,
                motivo_do_desconto: motivo,
                pagamentos: form.lancados.clone(),
                fotografo_id: pessoas.fotografo.unwrap_or_default(),
                atendente_id: pessoas.atendente.unwrap_or_default(),
                auxiliar_id: pessoas.auxiliar.unwrap_or_default(),
            },
        );
        self.gravar(
            "venda",
            "POST",
            "/pos-venda/caixa/vendas".into(),
            dados::nova_venda_json(&venda),
            cx,
        );
        cx.notify();
    }

    // ── Sangria e suprimento ────────────────────────────────────────────────

    fn registrar_movimento(&mut self, cx: &mut Context<Self>) {
        let estudio = self.estudio_do_caixa().map(str::to_string);
        let Some(Dialogo::Movimento(form)) = self.dialogo.as_mut() else {
            return;
        };
        if form.enviando {
            return;
        }
        let motivo = valor_de(&form.motivo, cx);
        let valor = match regras::ler_movimento(&valor_de(&form.valor, cx), &motivo) {
            Ok(v) => v,
            Err(erro) => {
                form.erro = Some(erro);
                cx.notify();
                return;
            }
        };
        let Some(estudio) = estudio else {
            form.erro = Some("Movimento inválido.".into());
            cx.notify();
            return;
        };
        form.erro = None;
        form.enviando = true;
        let tipo = form.tipo;
        self.em_curso.push(EmCurso::Movimento { tipo, valor });
        self.gravar(
            "movimento",
            "POST",
            "/pos-venda/caixa/movimentos".into(),
            json!({
                "estudio_id": estudio,
                "tipo": tipo.chave(),
                "valor_centavos": valor,
                "motivo": motivo,
            }),
            cx,
        );
        cx.notify();
    }

    // ── Estorno ─────────────────────────────────────────────────────────────

    pub(super) fn abrir_estorno(
        &mut self,
        venda: Venda,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let escolhidas: HashSet<String> = venda
            .itens
            .iter()
            .filter(|i| !i.estornada)
            .map(|i| i.foto_id.clone())
            .collect();
        let sugerido = regras::valor_sugerido_do_estorno(&venda, &escolhidas);
        let escrito = dinheiro::formatar_campo(sugerido);
        let valor = campo(window, cx, "0,00");
        escrever(&valor, escrito.clone(), window, cx);
        let forma = venda.forma_principal();
        self.lembrar_foco(window, cx);
        self.dialogo = Some(Dialogo::Estorno(Box::new(FormEstorno {
            venda,
            escolhidas,
            valor,
            escrito,
            forma,
            detalhe: campo(window, cx, "Detalhe (opcional): NSU, chave PIX…"),
            motivo: campo(window, cx, "o cliente desistiu de…"),
            des_sinalizar: true,
            erro: None,
            enviando: false,
        })));
        window.focus(&self.foco_do_dialogo);
        cx.notify();
    }

    /// O valor acompanha a seleção até o operador digitar outro.
    pub(super) fn alternar_foto_do_estorno(
        &mut self,
        foto: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Dialogo::Estorno(form)) = self.dialogo.as_mut() else {
            return;
        };
        let seguia = valor_de(&form.valor, cx) == form.escrito;
        if !form.escolhidas.remove(&foto) {
            form.escolhidas.insert(foto);
        }
        if seguia {
            let sugerido = regras::valor_sugerido_do_estorno(&form.venda, &form.escolhidas);
            form.escrito = dinheiro::formatar_campo(sugerido);
            let (campo_valor, texto) = (form.valor.clone(), form.escrito.clone());
            escrever(&campo_valor, texto, window, cx);
        }
        cx.notify();
    }

    fn voltar_ao_sugerido(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialogo::Estorno(form)) = self.dialogo.as_mut() else {
            return;
        };
        let sugerido = regras::valor_sugerido_do_estorno(&form.venda, &form.escolhidas);
        form.escrito = dinheiro::formatar_campo(sugerido);
        let (campo_valor, texto) = (form.valor.clone(), form.escrito.clone());
        escrever(&campo_valor, texto, window, cx);
        cx.notify();
    }

    fn estornar(&mut self, cx: &mut Context<Self>) {
        let aberto = self.vista.as_ref().is_some_and(|v| v.caixa.is_some());
        let Some(Dialogo::Estorno(form)) = self.dialogo.as_mut() else {
            return;
        };
        if form.enviando {
            return;
        }
        let valor = dinheiro::ler_campo(&valor_de(&form.valor, cx));
        let motivo = valor_de(&form.motivo, cx);
        let valor =
            match regras::conferir_estorno(&form.venda, aberto, &form.escolhidas, valor, &motivo) {
                Ok(v) => v,
                Err(erro) => {
                    form.erro = Some(erro);
                    cx.notify();
                    return;
                }
            };
        form.erro = None;
        form.enviando = true;
        let estorno = regras::montar_estorno(
            &form.venda,
            &form.escolhidas,
            valor,
            &motivo,
            form.forma,
            &valor_de(&form.detalhe, cx),
        );
        let venda = form.venda.clone();
        let des_sinalizar = form.des_sinalizar;
        self.em_curso.push(EmCurso::Estorno {
            venda_id: venda.id.clone(),
            numero: venda.numero,
            valor,
            fotos: estorno.fotos.clone(),
            des_sinalizar,
        });
        self.gravar(
            "estorno",
            "POST",
            format!("/pos-venda/caixa/vendas/{}/estornos", codificar(&venda.id)),
            dados::novo_estorno_json(&estorno),
            cx,
        );
        cx.notify();
    }

    fn terminar_estorno(&mut self, d: DesSinalizacao, des_sinalizou: bool, cx: &mut Context<Self>) {
        let mut frase = format!(
            "Estorno de {} na venda #{}",
            dinheiro::formatar(d.valor),
            d.numero
        );
        if des_sinalizou {
            frase.push_str(&format!(" · {} foto(s) de volta a “à venda”", d.feitas));
        } else {
            frase.push_str(" · as fotos voltam ao cupom");
        }
        self.avisar_por(
            frase,
            TipoDeRecado::Sucesso,
            std::time::Duration::from_secs(8),
            cx,
        );
        if d.falhas > 0 {
            self.avisar_por(
                format!(
                    "{} foto(s) continuam sinalizadas: tire a sinalização (P) na grade.",
                    d.falhas
                ),
                TipoDeRecado::Erro,
                std::time::Duration::from_secs(10),
                cx,
            );
        }
        if matches!(self.dialogo, Some(Dialogo::Estorno(_))) {
            self.fechar_dialogo_depois(cx);
        }
        // As fotos des-sinalizadas mudaram na galeria: ela se relê também.
        if des_sinalizou && d.feitas > 0 {
            self.reler_detalhe(cx);
        }
        self.recarregar(cx);
    }

    // ── Fechamento cego ─────────────────────────────────────────────────────

    fn contagem(&self, cx: &Context<Self>) -> Option<Result<regras::PorForma, String>> {
        let Some(Dialogo::Fechar(form)) = self.dialogo.as_ref() else {
            return None;
        };
        let textos: Vec<(FormaDePagamento, String)> = form
            .campos
            .iter()
            .map(|(f, c)| (*f, valor_de(c, cx)))
            .collect();
        Some(regras::ler_contagem(&textos))
    }

    fn conferir_caixa(&mut self, cx: &mut Context<Self>) {
        let estudio = self.estudio_do_caixa().map(str::to_string);
        let Some(lido) = self.contagem(cx) else {
            return;
        };
        let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() else {
            return;
        };
        if form.enviando {
            return;
        }
        let contado = match lido {
            Ok(c) => c,
            Err(erro) => {
                form.erro = Some(erro);
                cx.notify();
                return;
            }
        };
        form.erro = None;
        let Some(estudio) = estudio else {
            self.avisar("Sessão sem estúdio.", TipoDeRecado::Erro, cx);
            return;
        };
        form.enviando = true;
        self.gravar(
            "conferir",
            "POST",
            "/pos-venda/caixa/conferir".into(),
            json!({ "estudio_id": estudio, "contado": dados::contado_json(&contado) }),
            cx,
        );
        cx.notify();
    }

    fn corrigir_contagem(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() {
            if form.enviando {
                return;
            }
            form.conferencia = None;
            let foco = form.campos[0].1.read(cx).focus_handle(cx);
            window.focus(&foco);
            cx.notify();
        }
    }

    fn fechar_caixa(&mut self, cx: &mut Context<Self>) {
        let estudio = self.estudio_do_caixa().map(str::to_string);
        let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() else {
            return;
        };
        let Some(conferencia) = &form.conferencia else {
            return;
        };
        if form.enviando {
            return;
        }
        let Some(estudio) = estudio else {
            self.avisar("Sessão sem estúdio.", TipoDeRecado::Erro, cx);
            return;
        };
        // O backend recalcula o esperado com o caixa travado: o que fecha é o da hora.
        let observacao = valor_de(&form.observacao, cx);
        let observacao = observacao.trim();
        let corpo = json!({
            "estudio_id": estudio,
            "contado": dados::contado_json(&conferencia.contado),
            "observacao": (!observacao.is_empty()).then_some(observacao),
        });
        form.enviando = true;
        self.gravar(
            "fechar",
            "POST",
            "/pos-venda/caixa/fechar".into(),
            corpo,
            cx,
        );
        cx.notify();
    }

    // ── As respostas ────────────────────────────────────────────────────────

    fn tirar_em_curso(&mut self, qual: impl Fn(&EmCurso) -> bool) -> Option<EmCurso> {
        let posicao = self.em_curso.iter().position(qual)?;
        Some(self.em_curso.remove(posicao))
    }

    pub(super) fn receber_gravacao(
        &mut self,
        rotulo: &str,
        resultado: Result<serde_json::Value, String>,
        cx: &mut Context<Self>,
    ) {
        match rotulo {
            "abrir" => {
                let pedido = self.tirar_em_curso(|e| matches!(e, EmCurso::Abrir { .. }));
                match resultado {
                    Ok(_) => {
                        let fundo = match pedido {
                            Some(EmCurso::Abrir { fundo }) => fundo,
                            _ => 0,
                        };
                        self.avisar(
                            format!(
                                "Caixa aberto com {} de fundo de troco.",
                                dinheiro::formatar(fundo)
                            ),
                            TipoDeRecado::Sucesso,
                            cx,
                        );
                        self.fechar_se(|d| matches!(d, Dialogo::Abrir(_)), cx);
                        self.recarregar(cx);
                    }
                    Err(erro) => {
                        if let Some(Dialogo::Abrir(form)) = self.dialogo.as_mut() {
                            form.enviando = false;
                        }
                        self.avisar(
                            dados::mensagem_do_erro(&erro, "Não foi possível abrir o caixa."),
                            TipoDeRecado::Erro,
                            cx,
                        );
                    }
                }
            }
            "movimento" => {
                let pedido = self.tirar_em_curso(|e| matches!(e, EmCurso::Movimento { .. }));
                match resultado {
                    Ok(_) => {
                        if let Some(EmCurso::Movimento { tipo, valor }) = pedido {
                            self.avisar(
                                format!(
                                    "{} de {} registrado.",
                                    tipo.rotulo(),
                                    dinheiro::formatar(valor)
                                ),
                                TipoDeRecado::Sucesso,
                                cx,
                            );
                        }
                        self.fechar_se(|d| matches!(d, Dialogo::Movimento(_)), cx);
                        self.recarregar(cx);
                    }
                    Err(erro) => {
                        if let Some(Dialogo::Movimento(form)) = self.dialogo.as_mut() {
                            form.enviando = false;
                        }
                        self.avisar(
                            dados::mensagem_do_erro(
                                &erro,
                                "Não foi possível registrar o movimento.",
                            ),
                            TipoDeRecado::Erro,
                            cx,
                        );
                    }
                }
            }
            "venda" => match resultado {
                Ok(valor) => {
                    let oito = std::time::Duration::from_secs(8);
                    match dados::ler_venda(valor) {
                        Ok(venda) => {
                            let mut frase = format!(
                                "Venda #{} registrada · {}",
                                venda.numero,
                                dinheiro::formatar(venda.total)
                            );
                            if venda.troco > 0 {
                                frase.push_str(&format!(
                                    " · troco {}",
                                    dinheiro::formatar(venda.troco)
                                ));
                            }
                            self.ultima_venda = Some(venda);
                            self.avisar_por(frase, TipoDeRecado::Sucesso, oito, cx);
                        }
                        Err(_) => {
                            self.avisar_por("Venda registrada.", TipoDeRecado::Sucesso, oito, cx)
                        }
                    }
                    self.desconto = DescontoNoTotal::default();
                    self.fechar_se(|d| matches!(d, Dialogo::Pagamento(_)), cx);
                    self.recarregar(cx);
                }
                Err(erro) => {
                    if let Some(Dialogo::Pagamento(form)) = self.dialogo.as_mut() {
                        form.enviando = false;
                    }
                    self.avisar_por(
                        dados::mensagem_do_erro(&erro, "Não foi possível registrar a venda."),
                        TipoDeRecado::Erro,
                        std::time::Duration::from_secs(8),
                        cx,
                    );
                }
            },
            "estorno" => {
                let pedido = self.tirar_em_curso(|e| matches!(e, EmCurso::Estorno { .. }));
                let Some(EmCurso::Estorno {
                    venda_id,
                    numero,
                    valor,
                    fotos,
                    des_sinalizar,
                }) = pedido
                else {
                    return;
                };
                match resultado {
                    Ok(_) => {
                        if self.ultima_venda.as_ref().is_some_and(|v| v.id == venda_id) {
                            self.ultima_venda = None;
                        }
                        let mut d = DesSinalizacao {
                            numero,
                            valor,
                            faltam: 0,
                            feitas: 0,
                            falhas: 0,
                        };
                        // As fotos voltam a "à venda" **depois** de o estorno
                        // gravar: o estorno é o que precisa acontecer; a
                        // sinalização é o ajuste que o acompanha.
                        if des_sinalizar && !fotos.is_empty() {
                            for foto in &fotos {
                                if self.gravar(
                                    "des-sinalizar",
                                    "PATCH",
                                    format!("/pos-venda/fotos/{}", codificar(foto)),
                                    json!({ "estado": "disponivel" }),
                                    cx,
                                ) {
                                    d.faltam += 1;
                                } else {
                                    d.falhas += 1;
                                }
                            }
                        }
                        if d.faltam == 0 {
                            self.terminar_estorno(d, des_sinalizar, cx);
                        } else {
                            self.des_sinalizacao = Some(d);
                        }
                    }
                    Err(erro) => {
                        if let Some(Dialogo::Estorno(form)) = self.dialogo.as_mut() {
                            form.enviando = false;
                        }
                        self.avisar_por(
                            dados::mensagem_do_erro(&erro, "Não foi possível estornar."),
                            TipoDeRecado::Erro,
                            std::time::Duration::from_secs(8),
                            cx,
                        );
                    }
                }
            }
            "des-sinalizar" => {
                let Some(d) = self.des_sinalizacao.as_mut() else {
                    return;
                };
                d.faltam = d.faltam.saturating_sub(1);
                if resultado.is_ok() {
                    d.feitas += 1;
                } else {
                    d.falhas += 1;
                }
                if d.faltam == 0 {
                    if let Some(d) = self.des_sinalizacao.take() {
                        self.terminar_estorno(d, true, cx);
                    }
                }
            }
            "conferir" => {
                let lida = resultado.and_then(dados::ler_conferencia);
                let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() else {
                    return;
                };
                form.enviando = false;
                match lida {
                    Ok(conferencia) => {
                        form.conferencia = Some(conferencia);
                        self.foco_pendente = Some(form.observacao.clone());
                    }
                    Err(erro) => self.avisar(
                        dados::mensagem_do_erro(&erro, "Não foi possível conferir o caixa."),
                        TipoDeRecado::Erro,
                        cx,
                    ),
                }
            }
            "fechar" => match resultado {
                Ok(valor) => {
                    self.ultima_venda = None;
                    if let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() {
                        form.enviando = false;
                        match dados::ler_caixa(valor) {
                            Ok(caixa) => form.resultado = Some(caixa),
                            Err(_) => {
                                self.fechar_dialogo_depois(cx);
                                self.avisar("Caixa fechado.", TipoDeRecado::Sucesso, cx);
                            }
                        }
                    }
                    self.recarregar(cx);
                }
                Err(erro) => {
                    if let Some(Dialogo::Fechar(form)) = self.dialogo.as_mut() {
                        form.enviando = false;
                    }
                    self.avisar(
                        dados::mensagem_do_erro(&erro, "Não foi possível fechar o caixa."),
                        TipoDeRecado::Erro,
                        cx,
                    );
                }
            },
            "lote" => self.receber_lote(resultado, cx),
            "cadastrar" => {
                let lido = resultado.and_then(dados::ler_funcionario);
                let Some(Dialogo::Pessoas(form)) = self.dialogo.as_mut() else {
                    return;
                };
                let Some(cadastro) = form.cadastro.as_mut() else {
                    return;
                };
                match lido {
                    Ok(funcionario) => {
                        let papel = cadastro.papel;
                        form.cadastro = None;
                        form.pendente = Some((papel, funcionario.id.clone()));
                        let nome = funcionario.nome.clone();
                        self.cadastrados.retain(|c| c.id != funcionario.id);
                        self.cadastrados.push(funcionario);
                        self.avisar(format!("{nome} cadastrado."), TipoDeRecado::Sucesso, cx);
                    }
                    Err(erro) => {
                        cadastro.enviando = false;
                        cadastro.erro = Some(dados::mensagem_do_erro(
                            &erro,
                            "Não foi possível cadastrar o funcionário.",
                        ));
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn fechar_se(&mut self, qual: impl Fn(&Dialogo) -> bool, cx: &mut gpui::App) {
        if self.dialogo.as_ref().is_some_and(qual) {
            self.fechar_dialogo_depois(cx);
        }
    }

    /// Fecha de onde não há janela — a resposta da rede que conclui o diálogo.
    ///
    /// 🔑 **O foco volta para quem o tinha** ([`crate::modal`]), como no
    /// `fechar_dialogo`. Até 2026-09-26 o sucesso só pedia o foco do caixa, e a
    /// negociação do caixa flutuante nem isso: o foco ficava no campo que
    /// sumiu, e as teclas da galeria morriam.
    pub(super) fn fechar_dialogo_depois(&mut self, cx: &mut gpui::App) {
        self.dialogo = None;
        if self.foco_antes.guardado() {
            self.foco_antes.devolver_depois(cx);
        } else {
            self.pedir_foco = true;
        }
    }

    // ── O desenho ───────────────────────────────────────────────────────────

    pub(super) fn render_dialogo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(foco) = self.foco_pendente.take() {
            let foco = foco.read(cx).focus_handle(cx);
            window.focus(&foco);
        }
        let dialogo = self.dialogo.as_ref()?;
        let contexto = dialogo.contexto();
        let (titulo, descricao, largura, corpo): (String, Option<AnyElement>, f32, AnyElement) =
            match dialogo {
                Dialogo::Atalhos => ("Atalhos do caixa".into(), None, 448., self.corpo_atalhos(cx)),
                Dialogo::Abrir(_) => (
                    "Abrir o caixa".into(),
                    Some(texto("O dinheiro que já está na gaveta para dar troco.")),
                    448.,
                    self.corpo_abrir(cx),
                ),
                Dialogo::Desconto(_) => {
                    let base = self.cupom().total;
                    (
                        "Desconto no total".into(),
                        Some(
                            h_flex()
                                .flex_wrap()
                                .gap(px(4.))
                                .child(format!("Sobre {}.", dinheiro::formatar(base)))
                                .child(estilo::tecla("R"))
                                .child("em reais,")
                                .child(estilo::tecla("%"))
                                .child("em percentual.")
                                .into_any_element(),
                        ),
                        448.,
                        self.corpo_desconto(cx),
                    )
                }
                Dialogo::Pessoas(form) if form.cadastro.is_some() => (
                    "Cadastrar funcionário".into(),
                    Some(texto(
                        "Quem fotografa, atende e auxilia nas vendas do caixa — qualquer um pode fazer os três.",
                    )),
                    448.,
                    self.corpo_cadastro(cx),
                ),
                Dialogo::Pessoas(_) => (
                    "Quem fotografou, quem atendeu e quem auxiliou".into(),
                    Some(texto(
                        "Do cadastro de funcionários — a mesma pessoa pode estar nos três. Vai na venda, e na nota quando ela existir.",
                    )),
                    448.,
                    self.corpo_pessoas(cx),
                ),
                Dialogo::Pagamento(_) => (
                    "Finalizar pagamento".into(),
                    Some(texto(self.resumo_do_pagamento())),
                    672.,
                    self.corpo_pagamento(cx),
                ),
                Dialogo::Movimento(_) => (
                    "Sangria ou suprimento".into(),
                    Some(
                        h_flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .child("Dinheiro que sai (")
                            .child(estilo::tecla("S"))
                            .child("sangria) ou entra (")
                            .child(estilo::tecla("U"))
                            .child("suprimento) na gaveta fora de uma venda.")
                            .into_any_element(),
                    ),
                    448.,
                    self.corpo_movimento(cx),
                ),
                Dialogo::Vendas => (
                    "Vendas".into(),
                    Some(texto(
                        "As desta sessão, com estorno, e o que passou pelo caixa do estúdio.",
                    )),
                    672.,
                    self.corpo_vendas(cx),
                ),
                Dialogo::Estorno(form) => (
                    format!("Estornar a venda #{}", form.venda.numero),
                    Some(texto(
                        "O valor sai do caixa aberto do estúdio agora, pela forma escolhida.",
                    )),
                    576.,
                    self.corpo_estorno(cx),
                ),
                Dialogo::Negociacao(form) => (
                    form.titulo.clone(),
                    Some(texto(
                        "O que foi combinado no balcão. Não muda o preço da compra online.",
                    )),
                    448.,
                    self.corpo_negociacao(cx),
                ),
                Dialogo::Fechar(form) => {
                    let (titulo, descricao) = if form.resultado.is_some() {
                        (
                            "Caixa fechado",
                            "Contado, esperado e diferença por forma de pagamento.",
                        )
                    } else if form.conferencia.is_some() {
                        (
                            "Conferir o fechamento",
                            "O caixa ainda está aberto. Confira a diferença e corrija o que foi digitado errado antes de fechar.",
                        )
                    } else {
                        (
                            "Fechar o caixa",
                            "Conte a gaveta e os comprovantes. O esperado aparece na conferência, antes de fechar.",
                        )
                    };
                    (
                        titulo.into(),
                        Some(texto(descricao)),
                        576.,
                        self.corpo_fechamento(cx),
                    )
                }
            };
        Some(self.casca(contexto, titulo, descricao, largura, corpo, cx))
    }

    fn casca(
        &self,
        contexto: &str,
        titulo: String,
        descricao: Option<AnyElement>,
        largura: f32,
        corpo: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (fundo, borda, apagado, acento) = (
            tema.background,
            tema.border,
            tema.muted_foreground,
            tema.accent,
        );
        // O painel da galeria marca os dele: o interceptador de teclas dele só
        // cuida do que é seu.
        let flutuante = if self.flutuante() { " Flutuante" } else { "" };
        let chave =
            KeyContext::parse(&format!("{DIALOGO} {contexto}{flutuante}")).unwrap_or_default();

        div()
            .id("caixa-veu")
            .absolute()
            .inset_0()
            .occlude()
            .bg(cores::veu())
            .flex()
            .items_center()
            .justify_center()
            .p(px(16.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|t, _, w, cx| t.fechar_dialogo(w, cx)),
            )
            .child(
                v_flex()
                    .id("caixa-dialogo")
                    .key_context(chave)
                    .track_focus(&self.foco_do_dialogo)
                    .on_action(cx.listener(|t, _: &FecharDialogo, w, cx| t.fechar_dialogo(w, cx)))
                    .on_action(cx.listener(|t, _: &ConfirmarDialogo, w, cx| t.confirmar(w, cx)))
                    .on_action(cx.listener(|t, _: &ConcluirVenda, w, cx| {
                        if matches!(t.dialogo, Some(Dialogo::Pagamento(_))) {
                            t.concluir_venda(w, cx)
                        } else {
                            t.confirmar(w, cx)
                        }
                    }))
                    .on_action(cx.listener(|t, _: &Forma1, w, cx| t.escolher_forma(1, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma2, w, cx| t.escolher_forma(2, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma3, w, cx| t.escolher_forma(3, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma4, w, cx| t.escolher_forma(4, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma5, w, cx| t.escolher_forma(5, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma6, w, cx| t.escolher_forma(6, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma7, w, cx| t.escolher_forma(7, w, cx)))
                    .on_action(cx.listener(|t, _: &Forma8, w, cx| t.escolher_forma(8, w, cx)))
                    .on_action(cx.listener(|t, _: &TirarUltimo, _, cx| t.tirar_ultimo(cx)))
                    .on_action(
                        cx.listener(|t, _: &EmReais, _, cx| {
                            t.mudar_modo(ModoDoDesconto::Valor, cx)
                        }),
                    )
                    .on_action(cx.listener(|t, _: &EmPercentual, _, cx| {
                        t.mudar_modo(ModoDoDesconto::Percentual, cx)
                    }))
                    .on_action(cx.listener(|t, _: &Sangria, _, cx| {
                        t.mudar_tipo(TipoDeMovimento::Sangria, cx)
                    }))
                    .on_action(cx.listener(|t, _: &Suprimento, _, cx| {
                        t.mudar_tipo(TipoDeMovimento::Suprimento, cx)
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .relative()
                    .w(px(largura))
                    .max_w_full()
                    .max_h(relative(0.9))
                    .overflow_y_scroll()
                    .gap(px(16.))
                    .p(px(24.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(borda)
                    .bg(fundo)
                    .shadow_lg()
                    .child(
                        v_flex()
                            .gap(px(8.))
                            .pr(px(24.))
                            .child(
                                div()
                                    .text_lg()
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(titulo),
                            )
                            .when_some(descricao, |d, descricao| {
                                d.child(div().text_sm().text_color(apagado).child(descricao))
                            }),
                    )
                    .child(corpo)
                    .child(
                        div()
                            .id("caixa-dialogo-fechar")
                            .absolute()
                            .top(px(16.))
                            .right(px(16.))
                            .size(px(20.))
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.7)
                            .cursor_pointer()
                            .hover(move |s| s.opacity(1.).bg(acento))
                            .child(Icon::new(Icone::X).size(px(16.)))
                            .on_click(
                                cx.listener(|t, _: &ClickEvent, w, cx| t.fechar_dialogo(w, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn corpo_atalhos(&self, cx: &mut Context<Self>) -> AnyElement {
        let apagado = cx.theme().muted_foreground;
        // O F9 e as teclas do cupom são do painel da galeria; a rota não as tem.
        let painel: &[(&str, &str)] = if self.flutuante() {
            &regras::ATALHOS_DO_PAINEL
        } else {
            &[]
        };
        v_flex()
            .gap(px(12.))
            .child(
                v_flex()
                    .gap(px(6.))
                    .children(regras::ATALHOS.iter().chain(painel).map(|(tecla, acao)| {
                        h_flex()
                            .items_start()
                            .gap(px(12.))
                            .child(div().w(px(52.)).flex_none().child(estilo::tecla(*tecla)))
                            .child(div().flex_1().min_w(px(0.)).child(*acao))
                    })),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .text_xs()
                    .text_color(apagado)
                    .child("No pagamento:")
                    .child(estilo::tecla("1"))
                    .child("–")
                    .child(estilo::tecla("8"))
                    .child("escolhe a forma,")
                    .child(estilo::tecla("Enter"))
                    .child("lança,")
                    .child(estilo::tecla("⌫"))
                    .child("tira o último."),
            )
            .into_any_element()
    }

    fn corpo_abrir(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Abrir(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let (fundo, erro, enviando) = (form.fundo.clone(), form.erro.clone(), form.enviando);
        v_flex()
            .gap(px(16.))
            .child(campo_de_valor("Fundo de troco", &fundo, cx))
            .children(erro.map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    com_tecla(
                        estilo::desligado(
                            estilo::botao_primario("caixa-abrir-enviar", cx),
                            enviando,
                        ),
                        if enviando {
                            "Abrindo…"
                        } else {
                            "Abrir caixa"
                        },
                        "Enter",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.enviar_abertura(cx))),
                ),
            )
            .into_any_element()
    }

    fn corpo_desconto(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Desconto(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let (modo, texto_campo, motivo) = (form.modo, form.texto.clone(), form.motivo.clone());
        let base = self.cupom().total;
        let lido = regras::ler_desconto(&valor_de(&texto_campo, cx), modo, base);
        let modos =
            h_flex()
                .gap(px(8.))
                .children(
                    [ModoDoDesconto::Valor, ModoDoDesconto::Percentual].map(|m| {
                        let rotulo = if m == ModoDoDesconto::Valor {
                            "Em reais (R)"
                        } else {
                            "Percentual (%)"
                        };
                        alternavel(
                            SharedString::from(format!("caixa-desconto-{rotulo}")),
                            modo == m,
                            cx,
                        )
                        .child(rotulo)
                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| t.mudar_modo(m, cx)))
                    }),
                );
        let resultado = match &lido {
            Ok(c) => div()
                .font_family(cx.theme().mono_font_family.clone())
                .child(format!(
                    "− {} → total {}",
                    dinheiro::formatar(*c),
                    dinheiro::formatar(base - c)
                ))
                .into_any_element(),
            Err(e) => erro_do_form(e.clone(), cx).into_any_element(),
        };
        let pode = lido.is_ok();
        v_flex()
            .gap(px(16.))
            .child(modos)
            .child(rotulado(
                if modo == ModoDoDesconto::Valor {
                    "Desconto em R$"
                } else {
                    "Desconto em %"
                },
                div()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_lg()
                    .child(Input::new(&texto_campo)),
            ))
            .child(rotulado("Motivo (opcional)", Input::new(&motivo)))
            .child(resultado)
            .child(
                rodape()
                    .child(
                        estilo::botao_fantasma("caixa-desconto-remover", cx)
                            .child("Remover desconto")
                            .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                                t.desconto = DescontoNoTotal::default();
                                t.fechar_dialogo(w, cx);
                            })),
                    )
                    .child(
                        com_tecla(
                            estilo::desligado(
                                estilo::botao_primario("caixa-desconto-aplicar", cx),
                                !pode,
                            ),
                            "Aplicar",
                            "Enter",
                        )
                        .on_click(
                            cx.listener(|t, _: &ClickEvent, w, cx| t.aplicar_desconto(w, cx)),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn corpo_pessoas(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let papeis = [
            ("Quem fotografou", "Buscar quem fotografou…"),
            ("Quem atendeu", "Buscar quem atendeu…"),
            ("Quem auxiliou", "Buscar quem auxiliou…"),
        ];
        let linhas: Vec<AnyElement> = papeis
            .iter()
            .enumerate()
            .map(|(papel, (rotulo, dica))| {
                rotulado(
                    rotulo,
                    h_flex()
                        .gap(px(8.))
                        .child(
                            div().flex_1().min_w(px(0.)).child(
                                Select::new(&form.escolhas[papel])
                                    .placeholder(*dica)
                                    .search_placeholder(*dica)
                                    .cleanable(true)
                                    .empty(
                                        div()
                                            .p(px(8.))
                                            .text_sm()
                                            .child("Ninguém com esse nome. Use “Cadastrar”."),
                                    ),
                            ),
                        )
                        .child(
                            estilo::botao_contorno(
                                SharedString::from(format!("caixa-cadastrar-{papel}")),
                                cx,
                            )
                            .child(Icon::new(Icone::UserPlus).size(px(16.)))
                            .child("Cadastrar")
                            .on_click(cx.listener(
                                move |t, _: &ClickEvent, w, cx| t.abrir_cadastro(papel, w, cx),
                            )),
                        ),
                )
                .into_any_element()
            })
            .collect();
        v_flex()
            .gap(px(16.))
            .children(linhas)
            .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    com_tecla(
                        estilo::botao_primario("caixa-pessoas-confirmar", cx),
                        "Confirmar",
                        "Enter",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.salvar_pessoas(w, cx))),
                ),
            )
            .into_any_element()
    }

    fn corpo_cadastro(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Pessoas(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let Some(c) = form.cadastro.as_ref() else {
            return div().into_any_element();
        };
        v_flex()
            .gap(px(16.))
            .child(rotulado("Nome", Input::new(&c.nome)))
            .child(
                h_flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .child(rotulado("WhatsApp (opcional)", Input::new(&c.whatsapp))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(rotulado("E-mail (opcional)", Input::new(&c.email))),
                    ),
            )
            .children(c.erro.clone().map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    estilo::desligado(
                        estilo::botao_primario("caixa-cadastro-salvar", cx),
                        c.enviando,
                    )
                    .child(if c.enviando {
                        "Salvando…"
                    } else {
                        "Cadastrar"
                    })
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.cadastrar(cx))),
                ),
            )
            .into_any_element()
    }

    fn resumo_do_pagamento(&self) -> String {
        let n = self.cupom().itens.len();
        let p = self.pessoas_validas();
        let nome = |id: &Option<String>| {
            self.nome_do_funcionario(id.as_deref())
                .unwrap_or_else(|| "?".into())
        };
        format!(
            "{} · {} fotografou · {} atendeu · {} auxiliou",
            regras::itens(n),
            nome(&p.fotografo),
            nome(&p.atendente),
            nome(&p.auxiliar)
        )
    }

    fn corpo_pagamento(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Pagamento(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let total = self.a_receber();
        let conta = regras::conta_do_pagamento(total, &form.lancados);
        let tema = cx.theme();
        let (borda, apagado, frente, fundo, primaria, secundario, sobre_secundario, acento) = (
            tema.border,
            tema.muted_foreground,
            tema.foreground,
            tema.background,
            tema.primary,
            tema.secondary,
            tema.secondary_foreground,
            tema.accent,
        );
        let mono = tema.mono_font_family.clone();

        let formas = div().grid().grid_cols(2).gap(px(6.)).children(
            FormaDePagamento::TODAS
                .into_iter()
                .enumerate()
                .map(|(i, f)| {
                    alternavel(
                        SharedString::from(format!("caixa-forma-{}", f.chave())),
                        form.forma == Some(f),
                        cx,
                    )
                    .justify_start()
                    .child(estilo::tecla((i + 1).to_string()))
                    .child(f.rotulo())
                    .on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                        t.escolher_forma_de_pagamento(f, w, cx)
                    }))
                }),
        );

        let painel_da_forma = form.forma.map(|forma| {
            let bandeira = form.bandeira;
            v_flex()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(borda)
                .child(campo_de_valor(
                    &format!("Valor em {}", forma.rotulo()),
                    &form.valor,
                    cx,
                ))
                .when(forma.e_cartao(), |d| {
                    d.child(
                        v_flex()
                            .gap(px(6.))
                            .child(div().font_weight(FontWeight::MEDIUM).child("Bandeira"))
                            .child(div().grid().grid_cols(3).gap(px(6.)).children(
                                BANDEIRAS.iter().map(|(id, nome)| {
                                    let escolhida = bandeira == Some(*id);
                                    let id = *id;
                                    h_flex()
                                        .id(SharedString::from(format!("caixa-bandeira-{id}")))
                                        .gap(px(8.))
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(6.))
                                        .border_1()
                                        .text_xs()
                                        .cursor_pointer()
                                        .map(|d| {
                                            if escolhida {
                                                d.border_color(primaria).bg(primaria.opacity(0.1))
                                            } else {
                                                d.border_color(borda).hover(move |s| s.bg(acento))
                                            }
                                        })
                                        .child(div().truncate().child(*nome))
                                        // Clicar na escolhida desmarca: a bandeira é opcional.
                                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                            if let Some(Dialogo::Pagamento(form)) =
                                                t.dialogo.as_mut()
                                            {
                                                form.bandeira = if form.bandeira == Some(id) {
                                                    None
                                                } else {
                                                    Some(id)
                                                };
                                                cx.notify();
                                            }
                                        }))
                                }),
                            )),
                    )
                })
                .when(forma != FormaDePagamento::Dinheiro, |d| {
                    d.child(rotulado(forma.dica_do_detalhe(), Input::new(&form.detalhe)))
                })
                .child(
                    com_tecla(
                        estilo::botao_primario("caixa-lancar", cx).w_full(),
                        "Lançar",
                        "Enter",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.lancar(w, cx))),
                )
        });

        let lancados = (!form.lancados.is_empty()).then(|| {
            v_flex()
                .rounded(px(10.))
                .border_1()
                .border_color(borda)
                .font_family(mono.clone())
                .children(form.lancados.iter().enumerate().map(|(i, p)| {
                    let detalhe = p.detalhe.trim().to_string();
                    h_flex()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(6.))
                        .when(i > 0, |d| d.border_t_1().border_color(borda))
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .child(p.forma.rotulo())
                                .when(!detalhe.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .truncate()
                                            .text_color(apagado)
                                            .child(format!(" · {detalhe}")),
                                    )
                                }),
                        )
                        .child(dinheiro::formatar(p.valor))
                        .child(
                            div()
                                .id(SharedString::from(format!("caixa-tirar-{i}")))
                                .p(px(2.))
                                .rounded(px(4.))
                                .text_color(apagado)
                                .cursor_pointer()
                                .hover(move |s| s.bg(acento).text_color(frente))
                                .child(Icon::new(Icone::X).size(px(16.)))
                                .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                                    if let Some(Dialogo::Pagamento(form)) = t.dialogo.as_mut() {
                                        if i < form.lancados.len() {
                                            form.lancados.remove(i);
                                        }
                                        cx.notify();
                                    }
                                })),
                        )
                }))
        });

        let dica = h_flex()
            .flex_wrap()
            .gap(px(4.))
            .text_xs()
            .text_color(apagado)
            .child(estilo::tecla("1"))
            .child("–")
            .child(estilo::tecla("8"))
            .child("escolhe a forma ·")
            .child(estilo::tecla("Enter"))
            .child("lança ·")
            .child(estilo::tecla("⌫"))
            .child("tira o último ·")
            .child(estilo::tecla("F4"))
            .child("conclui ·")
            .child(estilo::tecla("Esc"))
            .child("volta");

        let linha = |rotulo: &str, valor: String, destaque: bool| {
            h_flex()
                .items_baseline()
                .justify_between()
                .gap(px(8.))
                .child(div().text_xs().opacity(0.8).child(rotulo.to_uppercase()))
                .child(
                    div()
                        .when(destaque, |d| {
                            d.text_size(px(24.)).font_weight(FontWeight::BOLD)
                        })
                        .when(!destaque, |d| d.text_base())
                        .child(valor),
                )
        };
        let enviando = form.enviando;
        let painel = v_flex()
            .w(px(224.))
            .flex_none()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(10.))
            .bg(frente)
            .text_color(fundo)
            .font_family(mono)
            .child(linha("Total", dinheiro::formatar(total), false))
            .child(linha("Recebido", dinheiro::formatar(conta.recebido), false))
            .when(conta.falta > 0, |d| {
                d.child(linha("Falta", dinheiro::formatar(conta.falta), true))
            })
            .when(conta.excedente_fora_do_dinheiro > 0, |d| {
                d.child(div().text_xs().text_color(cores::quente()).child(format!(
                    "Cartão, PIX e as outras formas não dão troco: passou {} do total.",
                    dinheiro::formatar(conta.excedente_fora_do_dinheiro)
                )))
            })
            .when(conta.pronto, |d| {
                d.child(linha("Troco", dinheiro::formatar(conta.troco), true))
            })
            .child(div().flex_1())
            .child(
                com_tecla(
                    estilo::desligado(
                        botao_secundario("caixa-concluir", secundario, sobre_secundario),
                        !conta.pronto || enviando,
                    ),
                    if enviando {
                        "Registrando…"
                    } else {
                        "Concluir venda"
                    },
                    "F4",
                )
                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.concluir_venda(w, cx))),
            );

        div()
            .flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(12.))
                    .child(formas)
                    .children(painel_da_forma)
                    .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
                    .children(lancados)
                    .child(dica),
            )
            .child(painel)
            .into_any_element()
    }

    fn corpo_movimento(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Movimento(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let tipos = h_flex().gap(px(8.)).children(
            [TipoDeMovimento::Sangria, TipoDeMovimento::Suprimento].map(|t| {
                let rotulo = if t == TipoDeMovimento::Sangria {
                    "Sangria (S)"
                } else {
                    "Suprimento (U)"
                };
                alternavel(
                    SharedString::from(format!("caixa-tipo-{}", t.chave())),
                    form.tipo == t,
                    cx,
                )
                .child(rotulo)
                .on_click(cx.listener(move |tela, _: &ClickEvent, _, cx| tela.mudar_tipo(t, cx)))
            }),
        );
        let enviando = form.enviando;
        v_flex()
            .gap(px(16.))
            .child(tipos)
            .child(campo_de_valor("Valor", &form.valor, cx))
            .child(rotulado("Motivo", Input::new(&form.motivo)))
            .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    com_tecla(
                        estilo::desligado(
                            estilo::botao_primario("caixa-movimento-registrar", cx),
                            enviando,
                        ),
                        if enviando {
                            "Registrando…"
                        } else {
                            "Registrar"
                        },
                        "Enter",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.registrar_movimento(cx))),
                ),
            )
            .into_any_element()
    }

    fn corpo_vendas(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (borda, apagado) = (tema.border, tema.muted_foreground);
        let mono = tema.mono_font_family.clone();
        let vendas = self
            .vista
            .as_ref()
            .and_then(|v| v.sessao.as_ref())
            .map(|s| s.vendas.clone())
            .unwrap_or_default();
        let caixa = self.vista.as_ref().and_then(|v| v.caixa.clone());

        let titulo = |t: &str| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(apagado)
                .child(t.to_uppercase())
        };

        let da_sessao = if vendas.is_empty() {
            div()
                .text_color(apagado)
                .child("Nenhuma venda nesta sessão ainda.")
                .into_any_element()
        } else {
            v_flex()
                .rounded(px(10.))
                .border_1()
                .border_color(borda)
                .children(vendas.into_iter().enumerate().map(|(i, v)| {
                    let mut quem = format!("Fotografou {} · atendeu {}", v.fotografo, v.atendente);
                    if let Some(aux) = &v.auxiliar {
                        quem.push_str(&format!(" · auxiliou {aux}"));
                    }
                    if v.estornado > 0 {
                        quem.push_str(&format!(" · estornado {}", dinheiro::formatar(v.estornado)));
                    }
                    let (_, _, ambar) = cores::selo_ambar();
                    let estornos: Vec<Div> = v
                        .estornos
                        .iter()
                        .map(|e| {
                            let mut frase = format!(
                                "Estorno #{} · {} · {} foto(s) · {}",
                                e.numero,
                                dados::dia_e_hora_br(&e.criado_em),
                                e.fotos.len(),
                                dinheiro::formatar(e.valor)
                            );
                            if !e.pagamentos.is_empty() {
                                frase.push_str(&format!(
                                    " em {}",
                                    regras::formas_juntas(e.pagamentos.iter().map(|p| p.forma))
                                ));
                            }
                            frase.push_str(&format!(" · {}", e.motivo));
                            div().text_xs().text_color(ambar).child(frase)
                        })
                        .collect();
                    let pode_estornar = !v.fotos_vendidas.is_empty();
                    let para_estornar = v.clone();
                    v_flex()
                        .gap(px(4.))
                        .px(px(12.))
                        .py(px(8.))
                        .when(i > 0, |d| d.border_t_1().border_color(borda))
                        .child(
                            h_flex()
                                .gap(px(12.))
                                .font_family(mono.clone())
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("#{}", v.numero)),
                                )
                                .child(
                                    div()
                                        .text_color(apagado)
                                        .child(dados::dia_e_hora_br(&v.criada_em)),
                                )
                                .child(div().flex_1().min_w(px(0.)).truncate().child(format!(
                                    "{} de {} foto(s) · {}",
                                    v.fotos_vendidas.len(),
                                    v.itens.len(),
                                    regras::formas_juntas(v.pagamentos.iter().map(|p| p.forma))
                                )))
                                .child(dinheiro::formatar(v.total)),
                        )
                        .child(div().text_xs().text_color(apagado).child(quem))
                        .children(estornos)
                        .when(pode_estornar, |d| {
                            d.child(
                                div().child(
                                    estilo::botao_contorno(
                                        SharedString::from(format!("caixa-estornar-{}", v.id)),
                                        cx,
                                    )
                                    .child("Estornar")
                                    .on_click(cx.listener(
                                        move |t, _: &ClickEvent, w, cx| {
                                            t.abrir_estorno(para_estornar.clone(), w, cx)
                                        },
                                    )),
                                ),
                            )
                        })
                }))
                .into_any_element()
        };

        let do_estudio = match caixa {
            None => div()
                .text_color(apagado)
                .child("O caixa do estúdio está fechado.")
                .into_any_element(),
            Some(c) => {
                let vendido: i64 = c.vendas.iter().map(|v| v.total).sum();
                let estornado: i64 = c.estornos.iter().map(|e| e.valor).sum();
                let mut frase = format!(
                    "Aberto às {} por {} · {} venda(s) · {} vendidos",
                    dados::hora_br(&c.aberto_em),
                    c.operador_email,
                    c.vendas.len(),
                    dinheiro::formatar(vendido)
                );
                if estornado > 0 {
                    frase.push_str(&format!(" · {} estornados", dinheiro::formatar(estornado)));
                }
                v_flex()
                    .gap(px(8.))
                    .child(div().text_color(apagado).child(frase))
                    .when(!c.movimentos.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .rounded(px(10.))
                                .border_1()
                                .border_color(borda)
                                .font_family(mono.clone())
                                .children(c.movimentos.iter().rev().enumerate().map(|(i, m)| {
                                    h_flex()
                                        .gap(px(12.))
                                        .px(px(12.))
                                        .py(px(8.))
                                        .when(i > 0, |d| d.border_t_1().border_color(borda))
                                        .child(
                                            div()
                                                .text_color(apagado)
                                                .child(dados::hora_br(&m.criado_em)),
                                        )
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .child(m.tipo.chave().to_uppercase())
                                                .child(
                                                    div()
                                                        .truncate()
                                                        .text_color(apagado)
                                                        .child(format!(" · {}", m.motivo)),
                                                ),
                                        )
                                        .child(format!(
                                            "{} {}",
                                            m.tipo.sinal(),
                                            dinheiro::formatar(m.valor)
                                        ))
                                })),
                        )
                    })
                    .into_any_element()
            }
        };

        v_flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(titulo("Desta sessão"))
                    .child(da_sessao),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(titulo("Caixa do estúdio"))
                    .child(do_estudio),
            )
            .into_any_element()
    }

    fn corpo_estorno(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Estorno(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let tema = cx.theme();
        let (borda, apagado, acento) = (tema.border, tema.muted_foreground, tema.accent);
        let mono = tema.mono_font_family.clone();
        let aberto = self.vista.as_ref().is_some_and(|v| v.caixa.is_some());
        let texto_do_valor = valor_de(&form.valor, cx);
        let editado = texto_do_valor != form.escrito;
        let valor = dinheiro::ler_campo(&texto_do_valor);
        let sugerido = regras::valor_sugerido_do_estorno(&form.venda, &form.escolhidas);
        let estornavel = form.venda.estornavel;

        let alerta = (!aberto).then(|| {
            let (fundo, borda_ambar, texto_ambar) = cores::selo_ambar();
            div()
                .px(px(12.))
                .py(px(8.))
                .rounded(px(6.))
                .border_1()
                .border_color(borda_ambar)
                .bg(fundo)
                .text_color(texto_ambar)
                .child(
                    "O caixa do estúdio está fechado. Abra o caixa (F8) para o dinheiro sair dele.",
                )
        });

        let fotos = v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .children(form.venda.itens.iter().enumerate().map(|(i, item)| {
                let marcada = item.estornada || form.escolhidas.contains(&item.foto_id);
                let foto = item.foto_id.clone();
                let estornada = item.estornada;
                h_flex()
                    .id(SharedString::from(format!("caixa-estorno-foto-{i}")))
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(8.))
                    .when(i > 0, |d| d.border_t_1().border_color(borda))
                    .map(|d| {
                        if estornada {
                            d.opacity(0.5)
                        } else {
                            d.cursor_pointer()
                                .hover(move |s| s.bg(acento))
                                .on_click(cx.listener(move |t, _: &ClickEvent, w, cx| {
                                    t.alternar_foto_do_estorno(foto.clone(), w, cx)
                                }))
                        }
                    })
                    .child(caixa_de_marcar(marcada, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_family(mono.clone())
                            .child(item.arquivo.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(mono.clone())
                            .text_xs()
                            .text_color(apagado)
                            .child(if estornada {
                                "já estornada".to_string()
                            } else {
                                dinheiro::formatar(item.valor_sugerido_de_estorno)
                            }),
                    )
            }));

        let devolucao = valor.filter(|v| *v > 0).map(|_| {
            v_flex()
                .gap(px(8.))
                .child(div().font_weight(FontWeight::MEDIUM).child("Devolvido em"))
                .child(div().grid().grid_cols(4).gap(px(6.)).children(
                    FormaDePagamento::TODAS.into_iter().map(|f| {
                        alternavel(
                            SharedString::from(format!("caixa-devolucao-{}", f.chave())),
                            form.forma == f,
                            cx,
                        )
                        .child(div().truncate().child(f.rotulo()))
                        .on_click(cx.listener(
                            move |t, _: &ClickEvent, _, cx| {
                                if let Some(Dialogo::Estorno(form)) = t.dialogo.as_mut() {
                                    form.forma = f;
                                    cx.notify();
                                }
                            },
                        ))
                    }),
                ))
                .child(Input::new(&form.detalhe))
        });

        let des_sinalizar = form.des_sinalizar;
        let enviando = form.enviando;
        v_flex()
            .gap(px(16.))
            .children(alerta)
            .child(rotulado("Fotos de que o cliente desistiu", fotos))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(campo_de_valor(
                        &format!("Valor a devolver (até {})", dinheiro::formatar(estornavel)),
                        &form.valor,
                        cx,
                    ))
                    .when(editado, |d| {
                        d.child(
                            div()
                                .id("caixa-estorno-sugerido")
                                .text_xs()
                                .text_color(apagado)
                                .cursor_pointer()
                                .hover(|s| s.underline())
                                .child(format!(
                                    "Voltar ao sugerido ({})",
                                    dinheiro::formatar(sugerido)
                                ))
                                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| {
                                    t.voltar_ao_sugerido(w, cx)
                                })),
                        )
                    }),
            )
            .children(devolucao)
            .child(rotulado("Motivo", Input::new(&form.motivo)))
            .child(
                h_flex()
                    .id("caixa-estorno-des-sinalizar")
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(caixa_de_marcar(des_sinalizar, cx))
                    .child("Des-sinalizar as fotos estornadas (voltam a “à venda”)")
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        if let Some(Dialogo::Estorno(form)) = t.dialogo.as_mut() {
                            form.des_sinalizar = !form.des_sinalizar;
                            cx.notify();
                        }
                    })),
            )
            .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    estilo::desligado(estilo::botao_perigo("caixa-estornar", cx), enviando)
                        .child(if enviando {
                            "Estornando…".to_string()
                        } else {
                            format!(
                                "Estornar {}",
                                valor.map(dinheiro::formatar).unwrap_or_default()
                            )
                        })
                        .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.estornar(cx))),
                ),
            )
            .into_any_element()
    }

    fn corpo_fechamento(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Fechar(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let apagado = cx.theme().muted_foreground;
        let enviando = form.enviando;

        if let Some(caixa) = &form.resultado {
            let contado = caixa.contado.clone().unwrap_or_default();
            let primeira = caixa.primeira_contagem.clone();
            let corrigida = regras::contagem_corrigida(primeira.as_ref(), &contado);
            return v_flex()
                .gap(px(12.))
                .child(tabela_do_fechamento(
                    &contado,
                    &caixa.esperado.clone().unwrap_or_default(),
                    &caixa.diferenca.clone().unwrap_or_default(),
                    if corrigida { primeira.as_ref() } else { None },
                    cx,
                ))
                .when(corrigida, |d| {
                    d.child(div().text_xs().text_color(apagado).child(
                        "A contagem foi corrigida na conferência. A primeira, feita às cegas, fica guardada no caixa.",
                    ))
                })
                .into_any_element();
        }

        if let Some(conferencia) = &form.conferencia {
            let (frase, alerta) = regras::frase_da_conferencia(&conferencia.diferenca);
            return v_flex()
                .gap(px(16.))
                .child(tabela_do_fechamento(
                    &conferencia.contado,
                    &conferencia.esperado,
                    &conferencia.diferenca,
                    None,
                    cx,
                ))
                .child(
                    div()
                        .text_color(if alerta {
                            cores::quente_clara()
                        } else {
                            apagado
                        })
                        .child(frase),
                )
                .child(rotulado(
                    "Observação (opcional)",
                    Input::new(&form.observacao),
                ))
                .child(
                    rodape()
                        .child(
                            estilo::desligado(
                                estilo::botao_contorno("caixa-corrigir", cx),
                                enviando,
                            )
                            .child("Corrigir contagem")
                            .on_click(
                                cx.listener(|t, _: &ClickEvent, w, cx| t.corrigir_contagem(w, cx)),
                            ),
                        )
                        .child(
                            com_tecla(
                                estilo::desligado(
                                    estilo::botao_perigo("caixa-fechar-caixa", cx),
                                    enviando,
                                ),
                                if enviando {
                                    "Fechando…"
                                } else {
                                    "Fechar caixa"
                                },
                                "Enter",
                            )
                            .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.fechar_caixa(cx))),
                        ),
                )
                .into_any_element();
        }

        v_flex()
            .gap(px(16.))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(12.))
                    .children(form.campos.iter().map(|(f, c)| {
                        campo_de_valor(
                            if *f == FormaDePagamento::Dinheiro {
                                "Dinheiro na gaveta (com o fundo)"
                            } else {
                                f.rotulo()
                            },
                            c,
                            cx,
                        )
                    })),
            )
            .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
            .child(
                rodape().child(
                    com_tecla(
                        estilo::desligado(estilo::botao_primario("caixa-conferir", cx), enviando),
                        if enviando {
                            "Conferindo…"
                        } else {
                            "Conferir contagem"
                        },
                        "Enter",
                    )
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.conferir_caixa(cx))),
                ),
            )
            .into_any_element()
    }
}

impl Caixa {
    fn corpo_negociacao(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Dialogo::Negociacao(form)) = self.dialogo.as_ref() else {
            return div().into_any_element();
        };
        let tema = cx.theme();
        let (primaria, borda, acento, apagado, perigo) = (
            tema.primary,
            tema.border,
            tema.accent,
            tema.muted_foreground,
            tema.danger,
        );
        let tipos = div()
            .grid()
            .grid_cols(2)
            .gap(px(8.))
            .children(Tipo::TODOS.into_iter().map(|t| {
                let ativo = form.tipo == t;
                v_flex()
                    .id(SharedString::from(format!("caixa-negociacao-{t:?}")))
                    .gap(px(2.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .cursor_pointer()
                    .map(|d| {
                        if ativo {
                            d.border_color(primaria).bg(primaria.opacity(0.05))
                        } else {
                            d.border_color(borda).hover(move |s| s.bg(acento))
                        }
                    })
                    .child(div().font_weight(FontWeight::MEDIUM).child(t.rotulo()))
                    .child(div().text_xs().text_color(apagado).child(t.dica()))
                    .on_click(cx.listener(move |tela, _: &ClickEvent, _, cx| {
                        if let Some(Dialogo::Negociacao(form)) = tela.dialogo.as_mut() {
                            form.tipo = t;
                            form.erro = None;
                            cx.notify();
                        }
                    }))
            }));
        let parceiro = (form.tipo == Tipo::Parceiro).then(|| {
            h_flex()
                .items_start()
                .gap(px(12.))
                .child(
                    div().flex_1().child(rotulado(
                        "Site",
                        h_flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(PARCEIROS.iter().map(|p| {
                                let p: &'static str = p;
                                alternavel(
                                    SharedString::from(format!("caixa-negociacao-site-{p}")),
                                    form.parceiro == p,
                                    cx,
                                )
                                .child(p)
                                .on_click(cx.listener(
                                    move |tela, _: &ClickEvent, _, cx| {
                                        if let Some(Dialogo::Negociacao(form)) =
                                            tela.dialogo.as_mut()
                                        {
                                            form.parceiro = p.to_string();
                                            cx.notify();
                                        }
                                    },
                                ))
                            })),
                    )),
                )
                .child(
                    div()
                        .flex_1()
                        .child(rotulado("Cupom", Input::new(&form.cupom))),
                )
        });
        let preco = (form.tipo != Tipo::Cortesia).then(|| {
            let rotulo = match form.tipo {
                Tipo::Desconto => "Quanto foi cobrado",
                Tipo::Parceiro => "Quanto pagou lá (se souber)",
                _ => "Valor cobrado (se houver)",
            };
            rotulado(
                rotulo,
                h_flex()
                    .gap(px(8.))
                    .child(div().text_color(apagado).child("R$"))
                    .child(div().w(px(128.)).child(Input::new(&form.preco)))
                    .children(form.preco_da_faixa.map(|p| {
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child(format!("preço da faixa: {}", dinheiro::formatar(p)))
                    })),
            )
        });
        let motivo = rotulado(
            if form.tipo == Tipo::Outro {
                "O que foi combinado"
            } else {
                "Motivo (opcional)"
            },
            Input::new(&form.motivo),
        );
        let confirmacao = form.confirmando.then(|| {
            v_flex()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(perigo.opacity(0.5))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Remover a negociação?"),
                )
                .child(div().text_sm().text_color(apagado).child(
                    "O registro do que foi combinado no balcão — tipo, valor e motivo — é apagado.",
                ))
                .child(
                    rodape()
                        .child(
                            estilo::botao_contorno("caixa-negociacao-nao-remover", cx)
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| {
                                    if let Some(Dialogo::Negociacao(form)) = tela.dialogo.as_mut() {
                                        form.confirmando = false;
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            estilo::botao_perigo("caixa-negociacao-remover-sim", cx)
                                .child("Remover")
                                .on_click(cx.listener(|tela, _: &ClickEvent, _, cx| {
                                    tela.salvar_negociacao(cx)
                                })),
                        ),
                )
        });
        let enviando = form.enviando;
        let existente = form.existente;
        v_flex()
            .gap(px(16.))
            .child(tipos)
            .children(parceiro)
            .children(preco)
            .child(motivo)
            .children(form.erro.clone().map(|e| erro_do_form(e, cx)))
            .children(confirmacao)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        estilo::desligado(
                            estilo::botao_primario("caixa-negociacao-salvar", cx),
                            enviando,
                        )
                        .child(if enviando { "Salvando…" } else { "Salvar" })
                        .on_click(cx.listener(
                            |tela, _: &ClickEvent, _, cx| {
                                if let Some(Dialogo::Negociacao(form)) = tela.dialogo.as_mut() {
                                    form.confirmando = false;
                                }
                                tela.salvar_negociacao(cx)
                            },
                        )),
                    )
                    .child(
                        estilo::desligado(
                            estilo::botao_fantasma("caixa-negociacao-cancelar", cx),
                            enviando,
                        )
                        .child("Cancelar")
                        .on_click(
                            cx.listener(|tela, _: &ClickEvent, w, cx| tela.fechar_dialogo(w, cx)),
                        ),
                    )
                    .when(existente, |d| {
                        d.child(div().flex_1()).child(
                            estilo::desligado(
                                estilo::botao_fantasma("caixa-negociacao-remover", cx),
                                enviando,
                            )
                            .text_color(perigo)
                            .child("Remover negociação")
                            .on_click(cx.listener(
                                |tela, _: &ClickEvent, _, cx| {
                                    if let Some(Dialogo::Negociacao(form)) = tela.dialogo.as_mut() {
                                        if !form.enviando {
                                            form.confirmando = true;
                                            cx.notify();
                                        }
                                    }
                                },
                            )),
                        )
                    }),
            )
            .into_any_element()
    }
}

// ── As peças ────────────────────────────────────────────────────────────────

fn texto(frase: impl Into<SharedString>) -> AnyElement {
    div().child(frase.into()).into_any_element()
}

/// `DialogFooter`: à direita, com vão.
fn rodape() -> Div {
    h_flex().justify_end().gap(px(8.))
}

/// `Label` em cima, campo embaixo.
fn rotulado(rotulo: &str, campo: impl IntoElement) -> Div {
    v_flex()
        .gap(px(6.))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(rotulo.to_string()),
        )
        .child(campo)
}

/// O `CampoDeValor` do site: rótulo, `R$` ao lado e o número em mono, grande.
fn campo_de_valor(rotulo: &str, campo: &Entity<InputState>, cx: &Context<Caixa>) -> Div {
    let tema = cx.theme();
    let (apagado, mono) = (tema.muted_foreground, tema.mono_font_family.clone());
    rotulado(
        rotulo,
        h_flex()
            .gap(px(8.))
            .child(
                div()
                    .font_family(mono.clone())
                    .text_sm()
                    .text_color(apagado)
                    .child("R$"),
            )
            .child(
                div()
                    .flex_1()
                    .font_family(mono)
                    .text_lg()
                    .child(Input::new(campo).large()),
            ),
    )
}

fn erro_do_form(erro: String, cx: &Context<Caixa>) -> Div {
    div().text_sm().text_color(cx.theme().danger).child(erro)
}

/// Os pares de botão do site: `default` quando escolhido, `outline` quando não.
fn alternavel(id: SharedString, ativo: bool, cx: &Context<Caixa>) -> Stateful<Div> {
    if ativo {
        estilo::botao_primario(id, cx)
    } else {
        estilo::botao_contorno(id, cx)
    }
}

/// `Button variant="secondary"` — o "Concluir venda" sobre o painel escuro.
fn botao_secundario(id: &'static str, fundo: gpui::Hsla, texto: gpui::Hsla) -> Stateful<Div> {
    h_flex()
        .id(id)
        .flex_none()
        .h(px(32.))
        .px(px(10.))
        .gap(px(6.))
        .rounded(px(8.))
        .justify_center()
        .text_sm()
        .whitespace_nowrap()
        .cursor_pointer()
        .bg(fundo)
        .text_color(texto)
        .font_weight(FontWeight::MEDIUM)
        .hover(move |s| s.bg(fundo.opacity(0.8)))
}

/// O quadradinho de marcar (`role="checkbox"` do site).
fn caixa_de_marcar(marcada: bool, cx: &Context<Caixa>) -> Div {
    let tema = cx.theme();
    let (primaria, sobre, borda) = (tema.primary, tema.primary_foreground, tema.border);
    div()
        .flex_none()
        .size(px(16.))
        .rounded(px(4.))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if marcada {
                d.border_color(primaria)
                    .bg(primaria)
                    .text_color(sobre)
                    .child(Icon::new(Icone::Check).size(px(12.)))
            } else {
                d.border_color(borda)
            }
        })
}

/// A tabela do fechamento: contado, esperado e diferença por forma.
fn tabela_do_fechamento(
    contado: &regras::PorForma,
    esperado: &regras::PorForma,
    diferenca: &regras::PorForma,
    primeira: Option<&regras::PorForma>,
    cx: &Context<Caixa>,
) -> Div {
    let tema = cx.theme();
    let (borda, apagado, perigo) = (tema.border, tema.muted_foreground, tema.danger);
    let mono = tema.mono_font_family.clone();
    let formas = regras::formas_da_tabela(contado, esperado, primeira);
    let de = |m: &regras::PorForma, f: &FormaDePagamento| m.get(f).copied().unwrap_or(0);
    let coluna = || div().flex_1().text_right();

    let mut cabeca = h_flex()
        .text_xs()
        .text_color(apagado)
        .child(div().flex_1().child("FORMA"));
    if primeira.is_some() {
        cabeca = cabeca.child(coluna().child("1ª CONTAGEM"));
    }
    cabeca = cabeca
        .child(coluna().child("CONTADO"))
        .child(coluna().child("ESPERADO"))
        .child(coluna().child("DIFERENÇA"));

    v_flex()
        .font_family(mono)
        .child(cabeca)
        .when(formas.is_empty(), |d| {
            d.child(
                div()
                    .py(px(8.))
                    .border_t_1()
                    .border_color(borda)
                    .text_center()
                    .text_color(apagado)
                    .child("Nada contado e nada esperado."),
            )
        })
        .children(formas.iter().map(|f| {
            let d = de(diferenca, f);
            let mut linha = h_flex()
                .py(px(4.))
                .border_t_1()
                .border_color(borda)
                .child(div().flex_1().child(f.rotulo()));
            if let Some(p) = primeira {
                linha = linha.child(
                    coluna()
                        .text_color(apagado)
                        .child(dinheiro::formatar(de(p, f))),
                );
            }
            linha
                .child(coluna().child(dinheiro::formatar(de(contado, f))))
                .child(coluna().child(dinheiro::formatar(de(esperado, f))))
                .child(
                    coluna()
                        .font_weight(FontWeight::SEMIBOLD)
                        .when(d < 0, |c| c.text_color(perigo))
                        .when(d > 0, |c| c.text_color(cores::quente_clara()))
                        .child(regras::diferenca_escrita(d)),
                )
        }))
}
