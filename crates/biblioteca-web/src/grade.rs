//! O estado da grade — o que junta core, miniaturas e gestos.
//!
//! # O que mora aqui, e o que não mora
//!
//! Aqui: o acervo filtrado (`biblioteca_core::Acervo`), a seleção
//! (`Selecao`), a geometria (`Layout`), a política e as texturas das
//! miniaturas, o ponteiro e o arrasto. É o que precisa ser **uma conta só**
//! entre o site e o desktop, mais o que é pesado demais para o JavaScript
//! (decodificar e subir 30 miniaturas por quadro).
//!
//! Não mora aqui: nome de arquivo, faixa, preço, negociação, datas — nada que
//! vire texto. O site guarda a foto inteira e escreve isso em DOM sobre os
//! tiles, a partir de [`Grade::visiveis_json`].
//!
//! # Os bits de mudança
//!
//! 🔑 Toda chamada que muda alguma coisa devolve um [`mudou`] — um `u32` com
//! um bit por fatia de estado que o React precisa reler (`selecao_json`,
//! `visiveis_json`, ...). Sem isso o hospedeiro teria de reler tudo a cada
//! evento, ou adivinhar; com isso ele relê só o que mudou, na hora, sem esperar
//! o próximo quadro. Os valores vivem aqui e são exportados por
//! `bits_de_mudanca_json` — o TypeScript não os repete (armadilha nº 8).

use std::collections::{BTreeSet, HashMap};

use biblioteca_core::acervo::{Acervo, Estado, Filtro};
use biblioteca_core::grade::{
    zoom_que_cabe, Direcao, Layout, Opcoes, Retangulo, ZOOM_MAX, ZOOM_MIN, ZOOM_PADRAO,
};
use biblioteca_core::miniaturas::{Cache, Desfecho, Politica};
use biblioteca_core::selecao::{Modificadores, Selecao};
use egui::TextureHandle;
use serde::Serialize;

use crate::modelo::FotoJson;
use crate::rede::Caixa;
use crate::tema::Cores;

/// Um bit por fatia de estado que o hospedeiro relê quando ele acende.
pub mod mudou {
    /// `selecao_json()` mudou (ids marcados ou foco).
    pub const SELECAO: u32 = 1;
    /// `visiveis_json()` mudou — a lista de tiles à vista, para os rodapés em DOM.
    pub const VISIVEIS: u32 = 2;
    /// `layout_json()`/`altura_total()` mudaram — o contêiner alto e as colunas.
    pub const LAYOUT: u32 = 4;
    /// Há um `alvo_de_rolagem()` a garantir visível (o teclado moveu o foco).
    pub const ROLAR: u32 = 8;
    /// `sob_ponteiro()` mudou — cursor e `title` do canvas.
    pub const CURSOR: u32 = 16;
    /// A tecla foi da grade: o hospedeiro faz `preventDefault`.
    pub const CONSUMIDA: u32 = 32;
    /// `contagens_json()` mudou — os números da barra.
    pub const CONTAGENS: u32 = 64;

    #[derive(serde::Serialize)]
    pub struct Tabela {
        pub selecao: u32,
        pub visiveis: u32,
        pub layout: u32,
        pub rolar: u32,
        pub cursor: u32,
        pub consumida: u32,
        pub contagens: u32,
    }

    pub fn tabela() -> Tabela {
        Tabela {
            selecao: SELECAO,
            visiveis: VISIVEIS,
            layout: LAYOUT,
            rolar: ROLAR,
            cursor: CURSOR,
            consumida: CONSUMIDA,
            contagens: CONTAGENS,
        }
    }
}

/// A partir de quantos pixels um botão apertado vira arrasto, e não clique.
const LIMIAR_DO_ARRASTO: f32 = 4.0;
/// A caixinha de marcar no canto do tile.
pub const CAIXA: f32 = 22.0;
pub const MARGEM_DA_CAIXA: f32 = 8.0;

/// Um arrasto de seleção em curso — em coordenadas **de conteúdo**.
pub struct Arrasto {
    pub origem: (f32, f32),
    pub atual: (f32, f32),
    /// A seleção de quando começou (só quando o gesto era aditivo).
    pub base: BTreeSet<usize>,
    /// Passou do limiar: é laço, não clique.
    pub ativo: bool,
}

impl Arrasto {
    pub fn retangulo(&self) -> Retangulo {
        Retangulo::entre(self.origem.0, self.origem.1, self.atual.0, self.atual.1)
    }
}

pub struct Grade {
    pub ctx: egui::Context,
    pub acervo: Acervo,
    pub selecao: Selecao,
    pub cache: Cache<String>,
    pub texturas: HashMap<String, TextureHandle>,
    pub caixa: Caixa,
    /// A URL da miniatura por id — a chave da textura.
    pub miniaturas: HashMap<String, String>,
    pub cores: Cores,
    pub layout: Layout,
    pub hover: Option<usize>,
    pub tem_foco: bool,
    pub arrasto: Option<Arrasto>,
    /// `[início, fim)` dos tiles que a janela mostra, com uma linha de folga.
    pub intervalo: (usize, usize),
    /// O deslocamento da página: o topo da janela visível, em px de conteúdo.
    pub deslocamento: f32,
    pub altura_visivel: f32,
    pub largura: f32,
    pub dpr: f32,
    pub backend: String,
    zoom: f32,
    rolar_para: Option<usize>,
}

impl Grade {
    pub fn nova(backend: &str, escuro: bool) -> Self {
        Self {
            ctx: egui::Context::default(),
            acervo: Acervo::novo(),
            selecao: Selecao::nova(),
            cache: Cache::nova(Politica::default()),
            texturas: HashMap::new(),
            caixa: Caixa::default(),
            miniaturas: HashMap::new(),
            cores: Cores::do_tema(escuro),
            layout: Layout::calcular(0.0, ZOOM_PADRAO, 0, Opcoes::default()),
            hover: None,
            tem_foco: false,
            arrasto: None,
            intervalo: (0, 0),
            deslocamento: 0.0,
            altura_visivel: 0.0,
            largura: 0.0,
            dpr: 1.0,
            backend: backend.to_string(),
            zoom: ZOOM_PADRAO,
            rolar_para: None,
        }
    }

    // ----- dados -----

    /// Troca o acervo inteiro — é o que a página faz a cada revalidação.
    ///
    /// ⚠️ **A seleção é preservada por id**, e não por posição: uma ação em
    /// lote muda cinco fotos e o acervo volta reordenado; se a seleção fosse
    /// por posição, o clique seguinte agiria sobre as erradas. O que sumiu do
    /// recorte simplesmente não volta.
    pub fn definir_fotos(&mut self, json: &str) -> Result<u32, String> {
        let lista: Vec<FotoJson> =
            serde_json::from_str(json).map_err(|e| format!("fotos ilegíveis: {e}"))?;
        let marcados = self.ids_selecionados();
        let foco = self.id_em_foco();

        let mut fotos = Vec::with_capacity(lista.len());
        let mut miniaturas = HashMap::with_capacity(lista.len());
        for f in &lista {
            fotos.push(f.para_core()?);
            miniaturas.insert(f.id.clone(), f.miniatura.clone());
        }
        self.acervo.definir(fotos);
        self.miniaturas = miniaturas;

        self.selecao.limpar_tudo();
        for id in &marcados {
            if let Some(n) = self.acervo.posicao_de(id) {
                self.selecao.marcar(n);
            }
        }
        self.selecao
            .focar(foco.and_then(|id| self.acervo.posicao_de(&id)));
        self.hover = None;

        Ok(mudou::SELECAO | mudou::CONTAGENS | self.recalcular())
    }

    /// O recorte da barra. **Trocar limpa a seleção**: as posições passam a
    /// apontar para outras fotos, e o que se vê é o que se opera.
    pub fn definir_filtro(&mut self, filtro: &str) -> Result<u32, String> {
        let f = match filtro {
            "todas" => Filtro::Todas,
            "apagadas" => Filtro::Apagadas,
            outro => Estado::do_texto(outro)
                .map(Filtro::Situacao)
                .ok_or_else(|| format!("filtro desconhecido: {outro}"))?,
        };
        self.acervo.filtrar(f);
        self.selecao.limpar_tudo();
        self.hover = None;
        Ok(mudou::SELECAO | mudou::CONTAGENS | mudou::CURSOR | self.recalcular())
    }

    pub fn definir_zoom(&mut self, zoom: f32) -> u32 {
        let z = if zoom.is_finite() {
            zoom.clamp(ZOOM_MIN, ZOOM_MAX).round()
        } else {
            ZOOM_PADRAO
        };
        if (z - self.zoom).abs() < f32::EPSILON {
            return 0;
        }
        self.zoom = z;
        self.recalcular()
    }

    /// O maior zoom em que **todas** as fotos do recorte cabem na área
    /// visível — o "ajustar à janela". `None` quando não cabe nem no mínimo.
    pub fn zoom_para_caber(&self) -> Option<f32> {
        let z = zoom_que_cabe(
            self.largura,
            self.altura_visivel,
            self.acervo.total_visivel(),
            Opcoes::default(),
        );
        let cabe = Layout::calcular(
            self.largura,
            z,
            self.acervo.total_visivel(),
            Opcoes::default(),
        )
        .altura_total
            <= self.altura_visivel;
        cabe.then_some(z)
    }

    pub fn definir_tema(&mut self, escuro: bool) -> u32 {
        self.cores = Cores::do_tema(escuro);
        self.ctx.request_repaint();
        0
    }

    // ----- janela -----

    pub fn redimensionar(&mut self, largura: f32, altura_visivel: f32, dpr: f32) -> u32 {
        self.largura = if largura.is_finite() {
            largura.max(0.0)
        } else {
            0.0
        };
        self.altura_visivel = if altura_visivel.is_finite() {
            altura_visivel.max(0.0)
        } else {
            0.0
        };
        self.dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        self.recalcular()
    }

    pub fn rolar(&mut self, deslocamento: f32) -> u32 {
        let d = if deslocamento.is_finite() {
            deslocamento.max(0.0)
        } else {
            0.0
        };
        if (d - self.deslocamento).abs() < 0.5 {
            return 0;
        }
        self.deslocamento = d;
        self.ctx.request_repaint();
        self.atualizar_visiveis()
    }

    fn recalcular(&mut self) -> u32 {
        let novo = Layout::calcular(
            self.largura,
            self.zoom,
            self.acervo.total_visivel(),
            Opcoes::default(),
        );
        let bits = if novo == self.layout {
            0
        } else {
            mudou::LAYOUT
        };
        self.layout = novo;
        self.ctx.request_repaint();
        bits | self.atualizar_visiveis()
    }

    /// Recalcula o que está à vista e pede as miniaturas dele. Chamado a cada
    /// rolagem, redimensionamento e troca de acervo.
    fn atualizar_visiveis(&mut self) -> u32 {
        let (inicio, fim) = self
            .layout
            .intervalo_visivel(self.deslocamento, self.altura_visivel);
        let total = self.layout.total;
        let colunas = self.layout.colunas.max(1);

        // As miniaturas do que está à vista, com duas linhas de folga.
        let folga = colunas * 2;
        let desejadas: Vec<String> = (inicio.saturating_sub(folga)..(fim + folga).min(total))
            .filter_map(|n| self.url_da(n))
            .collect();
        self.pedir_miniaturas(desejadas);

        // Os rodapés em DOM, com uma linha de folga — para não piscar na borda.
        let novo = (inicio.saturating_sub(colunas), (fim + colunas).min(total));
        if novo == self.intervalo {
            0
        } else {
            self.intervalo = novo;
            mudou::VISIVEIS
        }
    }

    // ----- ponteiro e teclado -----

    /// Do canvas (janela) para o conteúdo: a página rolou `deslocamento`.
    fn conteudo(&self, x: f32, y: f32) -> (f32, f32) {
        (x, y + self.deslocamento)
    }

    pub fn ponteiro_moveu(&mut self, x: f32, y: f32) -> u32 {
        let (cx, cy) = self.conteudo(x, y);
        if let Some(a) = &mut self.arrasto {
            a.atual = (cx, cy);
            if !a.ativo {
                let dx = cx - a.origem.0;
                let dy = cy - a.origem.1;
                a.ativo = (dx * dx + dy * dy).sqrt() > LIMIAR_DO_ARRASTO;
            }
            if a.ativo {
                let dentro = self.layout.indices_no_retangulo(a.retangulo());
                let base = a.base.clone();
                self.selecao.arrastar(&base, &dentro);
                self.ctx.request_repaint();
                return mudou::SELECAO;
            }
            return 0;
        }
        let novo = self.layout.indice_em(cx, cy);
        if novo == self.hover {
            return 0;
        }
        self.hover = novo;
        self.ctx.request_repaint();
        mudou::CURSOR
    }

    pub fn ponteiro_botao(
        &mut self,
        x: f32,
        y: f32,
        botao: u8,
        apertado: bool,
        modificadores: Modificadores,
    ) -> u32 {
        if botao != 0 {
            return 0;
        }
        let (cx, cy) = self.conteudo(x, y);
        if apertado {
            self.arrasto = Some(Arrasto {
                origem: (cx, cy),
                atual: (cx, cy),
                base: if modificadores.aditivo || modificadores.faixa {
                    self.selecao.instantaneo()
                } else {
                    BTreeSet::new()
                },
                ativo: false,
            });
            return 0;
        }
        let Some(arrasto) = self.arrasto.take() else {
            return 0;
        };
        self.ctx.request_repaint();
        if arrasto.ativo {
            // O laço some; a seleção já foi sendo aplicada a cada movimento.
            return mudou::SELECAO;
        }
        match self.layout.indice_em(cx, cy) {
            Some(n) => {
                let na_caixa = dentro(cx, cy, area_da_caixa(self.layout.posicao_do(n)));
                self.selecao.clicar(n, na_caixa, modificadores);
            }
            None => self.selecao.clicar_no_vazio(modificadores),
        }
        mudou::SELECAO
    }

    pub fn ponteiro_saiu(&mut self) -> u32 {
        if self.hover.is_none() {
            return 0;
        }
        self.hover = None;
        self.ctx.request_repaint();
        mudou::CURSOR
    }

    /// As teclas que são da grade. Devolve 0 para as que não são — o
    /// hospedeiro deixa a página tratá-las.
    pub fn tecla(&mut self, nome: &str, modificadores: Modificadores, comando: bool) -> u32 {
        let total = self.layout.total;
        let direcao = match nome {
            "ArrowLeft" => Some(Direcao::Esquerda),
            "ArrowRight" => Some(Direcao::Direita),
            "ArrowUp" => Some(Direcao::Cima),
            "ArrowDown" => Some(Direcao::Baixo),
            "Home" => Some(Direcao::Inicio),
            "End" => Some(Direcao::Fim),
            _ => None,
        };
        let bits = if let Some(d) = direcao {
            self.rolar_para = self.selecao.mover(d, &self.layout, modificadores);
            mudou::SELECAO | mudou::ROLAR
        } else {
            match nome {
                "PageDown" | "PageUp" => {
                    let passo = self.layout.altura_tile + self.layout.espaco;
                    let linhas = ((self.altura_visivel / passo.max(1.0)).floor() as usize).max(1);
                    self.rolar_para = self.selecao.saltar(
                        linhas,
                        nome == "PageDown",
                        &self.layout,
                        modificadores,
                    );
                    mudou::SELECAO | mudou::ROLAR
                }
                " " => {
                    self.selecao.alternar_foco();
                    mudou::SELECAO
                }
                "Escape" => {
                    self.selecao.desmarcar();
                    mudou::SELECAO
                }
                "a" | "A" if comando => {
                    self.selecao.marcar_todas(total);
                    mudou::SELECAO
                }
                _ => return 0,
            }
        };
        self.ctx.request_repaint();
        bits | mudou::CONSUMIDA
    }

    /// O canvas ganhou ou perdeu o foco do teclado. Chegando pelo Tab sem
    /// nada em foco, o cursor vai para a primeira — senão não há o que mover.
    pub fn foco(&mut self, tem: bool) -> u32 {
        self.tem_foco = tem;
        self.ctx.request_repaint();
        if !tem {
            return 0;
        }
        let antes = self.selecao.foco();
        self.selecao.focar_primeira_se_vazio(self.layout.total);
        if self.selecao.foco() == antes {
            0
        } else {
            mudou::SELECAO
        }
    }

    /// O botão "Selecionar as N visíveis" — que também desmarca quando todas
    /// já estão marcadas, porque é o mesmo botão.
    pub fn alternar_visiveis(&mut self) -> u32 {
        self.selecao.alternar_todas(self.layout.total);
        self.ctx.request_repaint();
        mudou::SELECAO
    }

    pub fn limpar_selecao(&mut self) -> u32 {
        self.selecao.desmarcar();
        self.ctx.request_repaint();
        mudou::SELECAO
    }

    /// Um clique vindo de fora do canvas — a tira de miniaturas do site: foca
    /// e seleciona só esta foto, como o clique simples na grade. Devolve 0 se
    /// o id não está no recorte.
    pub fn focar_id(&mut self, id: &str) -> u32 {
        let Some(n) = self.acervo.posicao_de(id) else {
            return 0;
        };
        self.selecao.clicar(n, false, Modificadores::default());
        self.rolar_para = Some(n);
        self.ctx.request_repaint();
        mudou::SELECAO | mudou::ROLAR
    }

    /// Os ids do recorte em vigor, na ordem da grade — a lista que a tira do
    /// site percorre. É a mesma conta do filtro, lida daqui em vez de refeita
    /// em TypeScript (armadilha nº 8).
    pub fn ids_visiveis_json(&self) -> String {
        let ids: Vec<&str> = self.acervo.visiveis().map(|f| f.id.as_str()).collect();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())
    }

    // ----- leitura -----

    pub fn indice_em(&self, x: f32, y: f32) -> Option<usize> {
        let (cx, cy) = self.conteudo(x, y);
        self.layout.indice_em(cx, cy)
    }

    pub fn url_da(&self, n: usize) -> Option<String> {
        self.acervo
            .visivel(n)
            .and_then(|f| self.miniaturas.get(&f.id))
            .cloned()
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

    pub fn selecao_json(&self) -> String {
        #[derive(Serialize)]
        struct S {
            ids: Vec<String>,
            foco: Option<String>,
        }
        serde_json::to_string(&S {
            ids: self.ids_selecionados(),
            foco: self.id_em_foco(),
        })
        .unwrap_or_else(|_| "{\"ids\":[],\"foco\":null}".into())
    }

    pub fn contagens_json(&self) -> String {
        #[derive(Serialize)]
        struct C {
            todas: usize,
            levadas: usize,
            a_venda: usize,
            compradas: usize,
            apagadas: usize,
            visiveis: usize,
        }
        let c = self.acervo.contagens();
        serde_json::to_string(&C {
            todas: c.todas,
            levadas: c.levadas,
            a_venda: c.a_venda,
            compradas: c.compradas,
            apagadas: c.apagadas,
            visiveis: self.acervo.total_visivel(),
        })
        .unwrap_or_else(|_| "{}".into())
    }

    pub fn layout_json(&self) -> String {
        #[derive(Serialize)]
        struct L {
            colunas: usize,
            lado: f32,
            altura_imagem: f32,
            altura_rodape: f32,
            altura_tile: f32,
            espaco: f32,
            total: usize,
            altura_total: f32,
        }
        let l = &self.layout;
        serde_json::to_string(&L {
            colunas: l.colunas,
            lado: l.lado,
            altura_imagem: l.altura_imagem,
            altura_rodape: l.altura_rodape,
            altura_tile: l.altura_tile,
            espaco: l.espaco,
            total: l.total,
            altura_total: l.altura_total,
        })
        .unwrap_or_else(|_| "{}".into())
    }

    /// Os tiles à vista, em coordenadas **de conteúdo** — é onde o site põe os
    /// rodapés em DOM. Só muda quando o intervalo ou o layout mudam.
    pub fn visiveis_json(&self) -> String {
        #[derive(Serialize)]
        struct V {
            i: usize,
            id: String,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
        }
        let (inicio, fim) = self.intervalo;
        let lista: Vec<V> = (inicio..fim)
            .filter_map(|n| {
                let f = self.acervo.visivel(n)?;
                let t = self.layout.posicao_do(n);
                Some(V {
                    i: n,
                    id: f.id.clone(),
                    x: t.x,
                    y: t.y,
                    w: t.w,
                    h: t.h,
                })
            })
            .collect();
        serde_json::to_string(&lista).unwrap_or_else(|_| "[]".into())
    }

    /// `[y, h]` (em conteúdo) do tile que o teclado alcançou, uma vez.
    pub fn alvo_de_rolagem(&mut self) -> Vec<f32> {
        match self.rolar_para.take() {
            Some(n) if n < self.layout.total => {
                let t = self.layout.posicao_do(n);
                vec![t.y, t.h]
            }
            _ => Vec::new(),
        }
    }

    // ----- miniaturas -----

    /// Esvazia a caixa de correio. Chamado no começo de cada quadro.
    pub fn receber(&mut self) {
        for m in self.caixa.esvaziar() {
            self.receber_miniatura(m.url, m.bytes);
            self.ctx.request_repaint();
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

    /// Quem decide quantas e em que ordem é o core; quem busca é o `fetch`;
    /// quem decodifica é o `image`, aqui.
    fn pedir_miniaturas(&mut self, desejadas: Vec<String>) {
        for chave in &desejadas {
            self.cache.usar(chave);
        }
        for url in self.cache.pedir(&desejadas) {
            self.caixa.buscar_miniatura(url);
        }
    }
}

/// Onde fica a caixinha de marcar dentro de um tile.
pub fn area_da_caixa(t: Retangulo) -> Retangulo {
    Retangulo {
        x: t.x + t.w - MARGEM_DA_CAIXA - CAIXA,
        y: t.y + MARGEM_DA_CAIXA,
        w: CAIXA,
        h: CAIXA,
    }
}

fn dentro(x: f32, y: f32, r: Retangulo) -> bool {
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
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
