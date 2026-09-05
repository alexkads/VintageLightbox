//! O estado da tela e o quadro — o que junta core, rede, texturas e egui.
//!
//! # O desenho
//!
//! O egui é **modo imediato**: a cada quadro a tela inteira é descrita de novo
//! a partir do estado. Não há componente com ciclo de vida, não há efeito, não
//! há sincronização de props — há `self` e o que ele diz. É por isso que tudo
//! o que a tela sabe mora aqui, num struct só, e é por isso que as telas
//! (`telas/`) são `impl App` espalhados em arquivos, e não objetos.
//!
//! # Comandos, não mutações no meio do desenho
//!
//! 🔑 Enquanto desenha, uma tela empurra **comandos** numa lista
//! (`Comando`), e o quadro os executa depois que o desenho acaba. Sem isso,
//! o botão "Apagar" precisaria de `&mut self.selecao` e `&self.estado` ao mesmo
//! tempo — e o borrow checker tem razão em recusar: apagar no meio do desenho
//! deixaria a metade de baixo da tela desenhando fotos que a de cima acabou de
//! remover.

use std::collections::{BTreeSet, HashMap};

use biblioteca_core::acervo::{Acervo, Filtro};
use biblioteca_core::grade::{Layout, Opcoes, ZOOM_MAX, ZOOM_MIN, ZOOM_PADRAO};
use biblioteca_core::miniaturas::{Cache, Desfecho, Politica};
use biblioteca_core::negociacao;
use biblioteca_core::selecao::Selecao;
use egui::TextureHandle;
use serde::{Deserialize, Serialize};

use crate::modelo::{EstadoDaGaleria, Foto, ItemDaImportacao, Pagamento, RevelacaoLocal};
use crate::rede::{self, Caixa, Chegada};

/// Um aviso passageiro no canto da tela — o `toast` do site.
pub struct Toast {
    pub texto: String,
    pub erro: bool,
    pub ate: f64,
}

/// O que a tela pede ao hospedeiro (o `app.tsx`) — o que só o DOM faz.
///
/// A lista é curta de propósito, e é a fronteira inteira entre o wasm e o
/// site: escolher arquivo é um `<input type=file>`; copiar e abrir aba exigem
/// gesto do usuário que só o DOM tem; a fila de importação vive num Worker que
/// é do site; e o editor de revelação ainda é o React (até ser portado).
#[derive(Debug, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Pedido {
    EscolherArquivos,
    Copiar { texto: String },
    AbrirUrl { url: String },
    Revelar { foto_id: Option<String> },
    Importacao { acao: String, id: Option<String> },
    Cursor { cursor: String },
}

/// A leva que o operador está montando: o que os próximos arquivos viram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leva {
    pub estado: String,
    pub produto_id: String,
    pub parametros: Parametros,
}

/// Os parâmetros da compressão — o mesmo JSON que o Worker do site lê do
/// `localStorage` (`parametros.ts`), com as mesmas faixas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parametros {
    pub ligada: bool,
    pub lado_maximo: u32,
    pub qualidade: u32,
    pub simultaneos: u32,
}

impl Default for Parametros {
    fn default() -> Self {
        Self {
            ligada: true,
            lado_maximo: 4000,
            qualidade: 90,
            simultaneos: 3,
        }
    }
}

impl Parametros {
    pub fn normalizar(&mut self) {
        self.lado_maximo = self.lado_maximo.clamp(1200, 12000);
        self.qualidade = self.qualidade.clamp(60, 100);
        self.simultaneos = self.simultaneos.clamp(1, 6);
    }
}

const CHAVE_PARAMETROS: &str = "recordarfotos:importacao:compressao";
const CHAVE_ZOOM: &str = "sessoes-fotograficas:zoom";

/// O recibo: os pagamentos, ou o motivo (e o detalhe) de não haver nenhum.
pub type ResultadoDoRecibo = Result<Vec<Pagamento>, (String, Option<String>)>;

/// Os diálogos — um de cada vez, por cima de tudo.
pub enum Dialogo {
    Negociacao {
        titulo: String,
        ids: Vec<String>,
        n: negociacao::Negociacao,
        preco_texto: String,
        preco_da_faixa: Option<i64>,
        existente: bool,
        erro: Option<String>,
    },
    ConfirmarApagar {
        ids: Vec<String>,
    },
    Recibo {
        arquivo: String,
        liberada_em: Option<String>,
        downloads: u32,
        fotos_no_pedido: usize,
        resultado: Option<ResultadoDoRecibo>,
    },
    Link {
        url: String,
        aviso: Option<String>,
    },
}

/// O que as telas pedem ao quadro.
pub enum Comando {
    Acao(Acao),
    Dialogo(Dialogo),
    FecharDialogo,
    Pedido(Pedido),
    Filtrar(Filtro),
    Zoom(f32),
    LimparSelecao,
    AlternarTodas,
    GuardarLeva,
}

/// Uma ação que grava — sempre pelo site, nunca direto na API.
#[derive(Debug, Serialize)]
#[serde(tag = "acao", rename_all = "snake_case")]
pub enum Acao {
    Estado {
        ids: Vec<String>,
        estado: String,
    },
    Produto {
        ids: Vec<String>,
        produto_id: Option<String>,
    },
    Remover {
        ids: Vec<String>,
    },
    Negociacao {
        ids: Vec<String>,
        valor: Option<NegociacaoJson>,
    },
    PrecoDeVenda {
        ids: Vec<String>,
        preco: Option<i64>,
    },
    Estudio {
        estudio_id: Option<String>,
    },
    Avisar,
    Link,
    Recibo {
        pedido_id: String,
    },
}

#[derive(Debug, Serialize)]
pub struct NegociacaoJson {
    pub preco_negociado: Option<i64>,
    pub observacao: Option<String>,
}

impl Acao {
    /// O verbo que o aviso usa quando dá certo.
    fn verbo(&self) -> &'static str {
        match self {
            Acao::Estado { estado, .. } if estado == "levada_no_balcao" => {
                "marcada(s) como levada(s)"
            }
            Acao::Estado { .. } => "posta(s) à venda",
            Acao::Produto { .. } => "com a faixa alterada",
            Acao::Remover { .. } => "apagada(s)",
            Acao::Negociacao { valor: Some(_), .. } => "com a negociação registrada",
            Acao::Negociacao { .. } => "com a negociação removida",
            Acao::PrecoDeVenda { preco: Some(_), .. } => "com o preço de venda fixado",
            Acao::PrecoDeVenda { .. } => "de volta ao preço da faixa",
            Acao::Estudio { .. } => "estúdio",
            Acao::Avisar => "avisar",
            Acao::Link => "link",
            Acao::Recibo { .. } => "recibo",
        }
    }
}

/// O que o site responde a uma ação.
#[derive(Debug, Deserialize, Default)]
pub struct Resultado {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub feitas: usize,
    #[serde(default)]
    pub falhas: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub pagamentos: Option<Vec<Pagamento>>,
    #[serde(default)]
    pub motivo: Option<String>,
    #[serde(default)]
    pub detalhe: Option<String>,
}

/// O estado da grade que não é do core: o gesto em curso e o ponteiro.
#[derive(Default)]
pub struct EstadoDaGrade {
    pub arrasto_base: Option<BTreeSet<usize>>,
    pub hover: Option<usize>,
    pub rolar_para: Option<usize>,
    pub layout: Option<Layout>,
}

/// Os campos de texto do painel — o que o operador está digitando.
#[derive(Default)]
pub struct Painel {
    pub preco_texto: String,
    pub preco_erro: Option<String>,
    /// Para qual foto o texto acima foi preenchido; trocar de foto zera.
    pub preco_de: Option<String>,
    pub lote_preco_texto: String,
    pub lote_preco_erro: Option<String>,
}

pub struct App {
    pub ctx: egui::Context,
    pub base: String,
    pub estado: EstadoDaGaleria,
    pub acervo: Acervo,
    pub selecao: Selecao,
    pub cache: Cache<String>,
    pub texturas: HashMap<String, TextureHandle>,
    pub filtro: Filtro,
    pub zoom: f32,
    pub caixa: Caixa,
    /// Quantas ações estão em voo — a tela desliga os botões enquanto houver.
    pub pendentes: usize,
    pub toasts: Vec<Toast>,
    pub dialogo: Option<Dialogo>,
    pub leva: Leva,
    pub ajustes_abertos: bool,
    pub importacao: Vec<ItemDaImportacao>,
    pub arrastando_arquivos: bool,
    pub pedidos: Vec<Pedido>,
    pub painel: Painel,
    pub grade: EstadoDaGrade,
    pub estudio_escolhido: String,
    pub agora: f64,
    pub backend: String,
    /// O que o editor de revelação gravou localmente, por id de foto — a
    /// biblioteca marca "editada · não salva" no tile e no painel.
    pub revelacoes_locais: HashMap<String, RevelacaoLocal>,
    /// Uma releitura pedida e ainda não feita: espera as ações em voo
    /// terminarem, para N ações renderem **uma** ida à API, e não N.
    pub releitura_pendente: bool,
    pub galeria_id: String,
}

impl App {
    pub fn novo(galeria_id: String, estado_json: &str, backend: &str) -> Result<Self, String> {
        let estado: EstadoDaGaleria =
            serde_json::from_str(estado_json).map_err(|e| format!("estado inválido: {e}"))?;

        let parametros = rede::lembrar(CHAVE_PARAMETROS)
            .and_then(|t| serde_json::from_str::<Parametros>(&t).ok())
            .map(|mut p| {
                p.normalizar();
                p
            })
            .unwrap_or_default();
        let zoom = rede::lembrar(CHAVE_ZOOM)
            .and_then(|t| t.parse::<f32>().ok())
            .filter(|z| (ZOOM_MIN..=ZOOM_MAX).contains(z))
            .unwrap_or(ZOOM_PADRAO);

        let mut app = Self {
            ctx: egui::Context::default(),
            base: format!("/dashboard/sessoes-fotograficas/{}", galeria_id),
            estudio_escolhido: estado.galeria.estudio_id.clone().unwrap_or_default(),
            revelacoes_locais: HashMap::new(),
            releitura_pendente: false,
            galeria_id: galeria_id.clone(),
            estado: EstadoDaGaleria::default(),
            acervo: Acervo::novo(),
            selecao: Selecao::nova(),
            cache: Cache::nova(Politica::default()),
            texturas: HashMap::new(),
            filtro: Filtro::Todas,
            zoom,
            caixa: Caixa::default(),
            pendentes: 0,
            toasts: Vec::new(),
            dialogo: None,
            leva: Leva {
                estado: "disponivel".into(),
                produto_id: String::new(),
                parametros,
            },
            ajustes_abertos: false,
            importacao: Vec::new(),
            arrastando_arquivos: false,
            pedidos: Vec::new(),
            painel: Painel::default(),
            grade: EstadoDaGrade::default(),
            agora: 0.0,
            backend: backend.to_string(),
        };
        app.definir_estado(estado)?;
        app.caixa.listar_revelacoes_locais();
        Ok(app)
    }

    /// Troca o acervo inteiro — o que acontece a cada gravação.
    ///
    /// ⚠️ **A seleção é preservada por id**, e não por posição: uma ação em
    /// lote muda cinco fotos e o acervo volta reordenado; se a seleção fosse
    /// por posição, o clique seguinte agiria sobre as erradas.
    pub fn definir_estado(&mut self, estado: EstadoDaGaleria) -> Result<(), String> {
        let marcados: Vec<String> = self.ids_selecionados();
        let foco = self.id_em_foco();

        let mut fotos = Vec::with_capacity(estado.fotos.len());
        for f in &estado.fotos {
            fotos.push(f.para_core()?);
        }
        self.estudio_escolhido = estado.galeria.estudio_id.clone().unwrap_or_default();
        self.estado = estado;
        self.acervo.definir(fotos);
        self.acervo.filtrar(self.filtro);

        self.selecao.limpar_tudo();
        for id in &marcados {
            if let Some(n) = self.acervo.posicao_de(id) {
                self.selecao.marcar(n);
            }
        }
        self.selecao
            .focar(foco.and_then(|id| self.acervo.posicao_de(&id)));

        // O retrato para a próxima abertura — e para o dia em que a tela abrir
        // do depósito antes de ir à rede.
        if let Ok(json) = serde_json::to_string(&self.estado) {
            self.caixa.guardar_galeria(self.galeria_id.clone(), json);
        }
        Ok(())
    }

    /// A edição local desta foto, se o editor deixou alguma **ainda não salva**.
    pub fn edicao_local(&self, foto_id: &str) -> Option<&RevelacaoLocal> {
        self.revelacoes_locais
            .get(foto_id)
            .filter(|r| !r.sincronizada)
    }

    pub fn foto(&self, id: &str) -> Option<&Foto> {
        self.estado.fotos.iter().find(|f| f.id == id)
    }

    /// A foto na posição `n` da grade filtrada.
    pub fn foto_visivel(&self, n: usize) -> Option<&Foto> {
        self.acervo.visivel(n).and_then(|f| self.foto(&f.id))
    }

    pub fn ids_selecionados(&self) -> Vec<String> {
        self.selecao
            .marcadas()
            .filter_map(|n| self.acervo.visivel(n))
            .map(|f| f.id.clone())
            .collect()
    }

    pub fn id_em_foco(&self) -> Option<String> {
        self.selecao
            .foco()
            .and_then(|n| self.acervo.visivel(n))
            .map(|f| f.id.clone())
    }

    pub fn avisar(&mut self, texto: impl Into<String>, erro: bool) {
        self.toasts.push(Toast {
            texto: texto.into(),
            erro,
            ate: self.agora + if erro { 8.0 } else { 4.0 },
        });
    }

    // ----- o que chega da rede -----

    /// Esvazia a caixa de correio. Chamado no começo de cada quadro.
    pub fn receber(&mut self) {
        for chegada in self.caixa.esvaziar() {
            match chegada {
                Chegada::Estado(Ok(json)) => match serde_json::from_str::<EstadoDaGaleria>(&json) {
                    Ok(estado) => {
                        if let Err(e) = self.definir_estado(estado) {
                            self.avisar(e, true);
                        }
                    }
                    Err(e) => self.avisar(format!("a galeria voltou ilegível: {e}"), true),
                },
                Chegada::Estado(Err(e)) => {
                    self.avisar(format!("não foi possível reler a galeria: {e}"), true)
                }
                Chegada::Acao { rotulo, resultado } => {
                    self.pendentes = self.pendentes.saturating_sub(1);
                    self.receber_acao(&rotulo, resultado);
                }
                Chegada::Miniatura { url, bytes } => self.receber_miniatura(url, bytes),
                Chegada::RevelacoesLocais(Ok(lista)) => {
                    self.revelacoes_locais = lista
                        .into_iter()
                        .filter_map(|(id, json)| {
                            serde_json::from_str::<RevelacaoLocal>(&json)
                                .ok()
                                .map(|r| (id, r))
                        })
                        .collect();
                }
                Chegada::RevelacoesLocais(Err(_)) => {
                    // Sem IndexedDB (aba privada de alguns navegadores): a
                    // biblioteca só deixa de mostrar o "não salva".
                }
            }
            self.ctx.request_repaint();
        }
        // 🔑 Uma releitura para N ações: só quando a última em voo voltou.
        if self.releitura_pendente && self.pendentes == 0 {
            self.releitura_pendente = false;
            self.caixa
                .buscar_estado(format!("{}/api/estado", self.base));
        }
    }

    fn receber_acao(&mut self, rotulo: &str, resultado: Result<String, String>) {
        let r: Resultado = match resultado {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(e) => {
                self.avisar(e, true);
                return;
            }
        };

        match rotulo {
            "link" => {
                if let Some(url) = r.link {
                    self.pedidos.push(Pedido::Copiar { texto: url.clone() });
                    self.dialogo = Some(Dialogo::Link { url, aviso: None });
                } else {
                    self.avisar(
                        r.message
                            .unwrap_or_else(|| "Não foi possível gerar o link.".into()),
                        true,
                    );
                }
                return;
            }
            "recibo" => {
                if let Some(Dialogo::Recibo { resultado, .. }) = &mut self.dialogo {
                    *resultado = Some(match r.pagamentos {
                        Some(p) if r.ok => Ok(p),
                        _ => Err((
                            r.motivo.unwrap_or_else(|| "gateway_indisponivel".into()),
                            r.detalhe,
                        )),
                    });
                }
                return;
            }
            "estúdio" => {
                if r.ok {
                    self.avisar("Estúdio gravado.", false);
                    self.reler();
                } else {
                    self.avisar(
                        r.message
                            .unwrap_or_else(|| "Não foi possível gravar o estúdio.".into()),
                        true,
                    );
                    self.estudio_escolhido =
                        self.estado.galeria.estudio_id.clone().unwrap_or_default();
                }
                return;
            }
            "avisar" => {
                if r.ok {
                    let email = self.estado.galeria.email.clone().unwrap_or_default();
                    self.avisar(format!("Aviso enviado para {email}."), false);
                    self.reler();
                } else {
                    self.avisar(
                        r.message
                            .unwrap_or_else(|| "Não foi possível enviar o aviso.".into()),
                        true,
                    );
                }
                return;
            }
            _ => {}
        }

        if let Some(m) = r.message {
            self.avisar(m, true);
            return;
        }
        if r.feitas > 0 {
            self.avisar(format!("{} foto(s) {rotulo}.", r.feitas), false);
        }
        if !r.falhas.is_empty() {
            self.avisar(
                format!("{} não mudaram: {}", r.falhas.len(), r.falhas.join(" / ")),
                true,
            );
        }
        if r.feitas > 0 {
            self.selecao.desmarcar();
            self.reler();
        }
    }

    fn receber_miniatura(&mut self, url: String, bytes: Result<Vec<u8>, String>) {
        let textura = bytes.ok().and_then(|b| decodificar(&b));
        let desfecho = match textura {
            Some((largura, altura, rgba)) => {
                let imagem = egui::ColorImage::from_rgba_unmultiplied([largura, altura], &rgba);
                let handle =
                    self.ctx
                        .load_texture(url.clone(), imagem, egui::TextureOptions::LINEAR);
                self.texturas.insert(url.clone(), handle);
                Desfecho::Chegou
            }
            None => Desfecho::Falhou,
        };
        let efeito = self.cache.concluir(&url, desfecho);
        for podada in efeito.podar {
            self.texturas.remove(&podada);
        }
        for comecar in efeito.comecar {
            self.caixa.buscar_miniatura(comecar);
        }
    }

    /// Pede as miniaturas do que está à vista — chamado pela grade, a cada
    /// quadro em que o visível muda. Quem decide quantas e em que ordem é o
    /// core; quem busca é o `fetch`; quem decodifica é o `image`, aqui.
    pub fn pedir_miniaturas(&mut self, desejadas: Vec<String>) {
        for chave in &desejadas {
            self.cache.usar(chave);
        }
        for url in self.cache.pedir(&desejadas) {
            self.caixa.buscar_miniatura(url);
        }
    }

    /// Pede uma releitura da galeria — que acontece quando nada mais está em
    /// voo. Cinco ações seguidas viram **uma** ida à API, e não cinco.
    pub fn reler(&mut self) {
        self.releitura_pendente = true;
        self.ctx.request_repaint();
    }

    /// O editor gravou algo no depósito local: reler a lista.
    pub fn revelacoes_mudaram(&mut self) {
        self.caixa.listar_revelacoes_locais();
    }

    // ----- comandos -----

    pub fn executar(&mut self, comandos: Vec<Comando>) {
        for c in comandos {
            match c {
                Comando::Acao(acao) => self.disparar(acao),
                Comando::Dialogo(d) => self.dialogo = Some(d),
                Comando::FecharDialogo => self.dialogo = None,
                Comando::Pedido(p) => self.pedidos.push(p),
                Comando::Filtrar(f) => {
                    self.filtro = f;
                    self.acervo.filtrar(f);
                    self.selecao.limpar_tudo();
                    self.grade.hover = None;
                }
                Comando::Zoom(z) => {
                    self.zoom = z.clamp(ZOOM_MIN, ZOOM_MAX).round();
                    rede::guardar(CHAVE_ZOOM, &self.zoom.to_string());
                }
                Comando::LimparSelecao => self.selecao.desmarcar(),
                Comando::AlternarTodas => self.selecao.alternar_todas(self.acervo.total_visivel()),
                Comando::GuardarLeva => {
                    self.leva.parametros.normalizar();
                    if let Ok(json) = serde_json::to_string(&self.leva.parametros) {
                        rede::guardar(CHAVE_PARAMETROS, &json);
                    }
                }
            }
        }
    }

    fn disparar(&mut self, acao: Acao) {
        let rotulo = match &acao {
            Acao::Link => "link".to_string(),
            Acao::Recibo { .. } => "recibo".to_string(),
            Acao::Estudio { .. } => "estúdio".to_string(),
            Acao::Avisar => "avisar".to_string(),
            outra => outra.verbo().to_string(),
        };
        match serde_json::to_string(&acao) {
            Ok(corpo) => {
                self.pendentes += 1;
                self.caixa
                    .enviar_acao(format!("{}/api/acao", self.base), corpo, rotulo);
            }
            Err(e) => self.avisar(format!("ação inválida: {e}"), true),
        }
    }

    pub fn layout_da_grade(&self, largura: f32) -> Layout {
        Layout::calcular(
            largura,
            self.zoom,
            self.acervo.total_visivel(),
            Opcoes::default(),
        )
    }

    /// O quadro inteiro: recebe a rede, desenha as telas, expira os avisos.
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.agora = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.ate > self.agora);

        let mut comandos = Vec::new();
        self.ui_cabecalho(ctx, &mut comandos);
        self.ui_painel(ctx, &mut comandos);
        self.ui_central(ctx, &mut comandos);
        self.ui_dialogos(ctx, &mut comandos);
        self.ui_toasts(ctx);
        self.executar(comandos);

        if !self.toasts.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui_toasts(&self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let borda = if t.erro {
                        crate::tema::VERMELHO
                    } else {
                        crate::tema::N700
                    };
                    egui::Frame::popup(ui.style())
                        .fill(crate::tema::N800)
                        .stroke(egui::Stroke::new(1.0_f32, borda))
                        .rounding(crate::tema::RAIO)
                        .show(ui, |ui| {
                            ui.set_max_width(360.0);
                            ui.colored_label(crate::tema::N100, &t.texto);
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

/// JPEG/PNG → RGBA, reduzido ao lado da textura. Milissegundos por miniatura.
fn decodificar(bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    const LADO: u32 = 512;
    let imagem = image::load_from_memory(bytes).ok()?;
    let imagem = if imagem.width() > LADO || imagem.height() > LADO {
        imagem.thumbnail(LADO, LADO)
    } else {
        imagem
    };
    let rgba = imagem.to_rgba8();
    let (l, a) = rgba.dimensions();
    Some((l as usize, a as usize, rgba.into_raw()))
}
