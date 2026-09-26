//! O Comparar da Revelação — duas fotos lado a lado, no `⇧C`.
//!
//! # Para que serve
//!
//! No balcão, o cliente olha o segundo monitor e escolhe **qual das duas**
//! levar (dono, 2026-09-26, olhando o `C` do Lightroom). O `⇧C` põe a foto
//! aberta e a candidata lado a lado, aqui e na tela do cliente. A regra (quem é
//! a candidata, para onde as setas levam, o que abre ao sair) é a do
//! `biblioteca_core::comparar`, a mesma que o editor do site repete.
//!
//! 🔑 **A foto aberta não muda enquanto ele dura.** O motor, o histórico e a
//! gravação continuam nela; o que muda é o palco, que mostra as duas. Ao sair,
//! a Revelação abre a escolhida.
//!
//! # De onde vêm os pixels da outra
//!
//! A aberta já está desenhada (`Aberta::desenhada`). A outra é revelada pelo
//! mesmo caminho da revelação antecipada (`antecipar_a_proxima`): a prévia do
//! disco, a receita do catálogo, uma ida ao motor e o cache de reveladas.
//!
//! 🚨 **Um pedido do Comparar por vez, e nunca por cima do da aberta.** O
//! `Processador` só atende o pedido mais novo: dois seguidos perdem o primeiro.
//! Por isso as fotos a revelar esperam numa fila e saem uma a uma, quando a GPU
//! está livre.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use biblioteca_core::comparar::{self, Comparacao, Lado};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, img, prelude::*, px, Context, ObjectFit, RenderImage, SharedString, Window};
use infrastructure::transformacao;

use super::super::cache;
use super::super::persistencia;
use super::super::processador::{Ajustes, Pedido};
use super::{tira, Destino, Pendente, Revelacao};
use crate::imagem::para_gpui;
use crate::tema;

/// O Comparar em curso.
pub(super) struct EmComparacao {
    /// As duas, por posição no acervo.
    pub par: Comparacao<usize>,
    /// A tira de quando ele abriu.
    ///
    /// 🔑 **Congelada**: uma nota ou um `X` dados aqui podem tirar a foto do
    /// recorte, e as setas não podem mudar de trilho no meio da comparação.
    pub tira: Vec<usize>,
    /// O que já está pronto para o palco, da foto que não é a aberta.
    pub(super) imagens: HashMap<usize, Arc<RenderImage>>,
    /// As que esperam a vez do motor — ver o 🚨 do módulo.
    pub(super) fila: VecDeque<usize>,
    /// A que está no motor agora (ou lendo a prévia do disco).
    pub(super) em_voo: Option<usize>,
}

impl Revelacao {
    /// Há duas fotos no palco?
    pub fn comparando(&self) -> bool {
        self.comparacao.is_some()
    }

    /// A foto escolhida no Comparar, com o id da grade da sessão — é nela que a
    /// nota, o `P` e o `X` caem.
    pub fn escolhida_no_comparar(&self) -> Option<String> {
        let c = self.comparacao.as_ref()?;
        self.acervo.get(*c.par.ativa()).map(tira::id_na_grade)
    }

    /// As duas do Comparar, esquerda e direita, e qual é a escolhida — o que
    /// a tela do cliente precisa saber.
    pub fn fotos_do_comparar(&self) -> Option<(usize, usize, Lado)> {
        let c = self.comparacao.as_ref()?;
        Some((c.par.esquerda, c.par.direita, c.par.ativa))
    }

    /// A outra foto do Comparar — o destaque de candidata na tira.
    pub(super) fn candidata_no_comparar(&self) -> Option<usize> {
        self.comparacao.as_ref().map(|c| *c.par.candidata())
    }

    /// O `⇧C`: entra com a aberta e a candidata, ou sai.
    pub fn alternar_comparacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.comparacao.is_some() {
            self.sair_do_comparar(window, cx);
            return;
        }
        // No Enquadrar não entra: comparar um recorte pela metade não responde
        // nada, e sair dele é o `R` ou o `Enter`.
        if self.edicao.is_some() || self.acervo.is_empty() {
            return;
        }
        let tira = self.na_tira();
        let marcadas: Vec<usize> = self.marcadas.iter().copied().collect();
        let Ok(par) = comparar::entrar(&tira, &self.posicao, &marcadas) else {
            return;
        };
        self.mostrando_original = false;
        self.previa = None;
        self.comparacao = Some(EmComparacao {
            par,
            tira,
            imagens: HashMap::new(),
            fila: VecDeque::new(),
            em_voo: None,
        });
        self.preparar_o_par(cx);
        cx.notify();
    }

    /// `Esc` ou `⇧C` de novo: sai abrindo a escolhida.
    pub fn sair_do_comparar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.comparacao.take() else {
            return;
        };
        let alvo = *c.par.ao_sair();
        if alvo != self.posicao {
            self.ir_para(alvo, window, cx);
        }
        cx.notify();
    }

    /// O clique numa metade: ela passa a ser a escolhida.
    pub fn ativar_no_comparar(&mut self, lado: Lado, cx: &mut Context<Self>) {
        if let Some(c) = self.comparacao.as_mut() {
            if c.par.ativa != lado {
                c.par.ativar(lado);
                cx.notify();
            }
        }
    }

    /// As setas no Comparar: trocam a outra foto, pulando a escolhida.
    pub(super) fn andar_no_comparar(&mut self, passo: i32, cx: &mut Context<Self>) {
        let andou = self.comparacao.as_mut().is_some_and(|c| {
            let tira = c.tira.clone();
            c.par.andar(&tira, passo)
        });
        if andou {
            self.preparar_o_par(cx);
            cx.notify();
        }
    }

    /// O clique simples na tira, no Comparar: a foto vira a candidata.
    pub(super) fn escolher_no_comparar(&mut self, posicao: usize, cx: &mut Context<Self>) {
        let trocou = self
            .comparacao
            .as_mut()
            .is_some_and(|c| c.par.escolher(posicao));
        if trocou {
            self.preparar_o_par(cx);
            cx.notify();
        }
    }

    /// Põe na fila as fotos do par que ainda não têm imagem, e solta a próxima.
    fn preparar_o_par(&mut self, cx: &mut Context<Self>) {
        let aberta = self.posicao;
        let Some(c) = self.comparacao.as_mut() else {
            return;
        };
        for posicao in [c.par.esquerda, c.par.direita] {
            let falta = posicao != aberta
                && !c.imagens.contains_key(&posicao)
                && c.em_voo != Some(posicao)
                && !c.fila.contains(&posicao);
            if falta {
                c.fila.push_back(posicao);
            }
        }
        // A que saiu do par não precisa mais: sem isto, andar depressa pelas
        // setas revelaria cada foto que passou.
        let par = [c.par.esquerda, c.par.direita];
        c.fila.retain(|p| par.contains(p));
        self.soltar_a_proxima_do_comparar(cx);
    }

    /// Manda ao motor a próxima da fila, se a GPU estiver livre.
    pub(super) fn soltar_a_proxima_do_comparar(&mut self, cx: &mut Context<Self>) {
        // A aberta tem prioridade: furar o pedido dela o descartaria.
        if self.aguardando.is_some() {
            return;
        }
        let Some(c) = self.comparacao.as_mut() else {
            return;
        };
        if c.em_voo.is_some() {
            return;
        }
        let Some(posicao) = c.fila.pop_front() else {
            return;
        };
        let Some(foto) = self.acervo.get(posicao).cloned() else {
            return;
        };
        c.em_voo = Some(posicao);

        let ajustes = persistencia::da_foto(&foto);
        let crop = persistencia::para_crop_settings(&persistencia::corte_da_foto(&foto));
        let no_cache = if persistencia::so_existe_no_site(&foto) {
            persistencia::chave_do_trabalho(&foto.id)
        } else {
            foto.id.clone()
        };
        let da_galeria = foto.id.clone();
        let previews = self.previews.clone();
        cx.spawn(async move |esta, cx| {
            // A decodificação vai para o executor de fundo, como na antecipação.
            let origem = cx
                .background_executor()
                .spawn(async move {
                    // ⚠️ A foto do site que a Revelação ainda não abriu pode não
                    // ter a cópia de trabalho no disco: aqui, no palco do
                    // operador, a prévia da galeria serve até lá. A segunda tela
                    // não passa por aqui — ela pede a cópia sem marca.
                    let imagem = previews
                        .get_preview(&no_cache)
                        .or_else(|| previews.get_preview(&da_galeria))?;
                    let rgba = imagem.to_rgba8();
                    let (largura, altura) = (rgba.width(), rgba.height());
                    Some((largura, altura, Arc::new(rgba.into_raw())))
                })
                .await;
            let _ = esta.update(cx, |tela, cx| match origem {
                Some((largura, altura, pixels)) => tela.revelar_no_comparar(
                    posicao, foto.id, largura, altura, pixels, ajustes, crop, cx,
                ),
                None => tela.largar_do_comparar(posicao, cx),
            });
        })
        .detach();
    }

    /// Os pixels da prévia chegaram: do cache, direto (no neutro) ou do motor.
    #[allow(clippy::too_many_arguments)]
    fn revelar_no_comparar(
        &mut self,
        posicao: usize,
        foto_id: String,
        largura: u32,
        altura: u32,
        pixels: Arc<Vec<u8>>,
        ajustes: Ajustes,
        crop: domain::value_objects::CropSettings,
        cx: &mut Context<Self>,
    ) {
        // O operador pode ter saído, ou trocado esta foto, enquanto o disco lia.
        let ainda = self
            .comparacao
            .as_ref()
            .is_some_and(|c| c.em_voo == Some(posicao));
        if !ainda {
            return;
        }
        let corte = transformacao::corte(&crop);
        let chave = cache::Chave::nova(&foto_id, (largura, altura), &ajustes, &corte);
        let sem_gpu = self.processador.disponivel() == Some(false);
        let pronta = if ajustes == Ajustes::default() || sem_gpu {
            // Sem receita não há o que revelar; sem GPU, a crua é o que há.
            image::RgbaImage::from_raw(largura, altura, pixels.to_vec())
                .map(image::DynamicImage::ImageRgba8)
        } else {
            self.reveladas.buscar(&chave)
        };
        if let Some(imagem) = pronta {
            self.entregar_ao_comparar(posicao, &imagem, &crop, cx);
            return;
        }

        let id = self.processador.proximo_id();
        self.processador.pedir(Pedido {
            id,
            pixels,
            largura,
            altura,
            ajustes,
            corte,
        });
        self.pedidos.insert(
            id,
            Pendente {
                chave,
                destino: Destino::Comparar { posicao, crop },
            },
        );
        // O pedido novo passou à frente da antecipação: a thread a larga.
        self.antecipando = None;
        self.pedido_do_comparar = Some(id);
        self.acompanhar(cx);
    }

    /// A revelação da outra foto está pronta: vai ao palco do Comparar.
    pub(super) fn entregar_ao_comparar(
        &mut self,
        posicao: usize,
        imagem: &image::DynamicImage,
        crop: &domain::value_objects::CropSettings,
        cx: &mut Context<Self>,
    ) {
        if let Some(c) = self.comparacao.as_mut() {
            if c.em_voo == Some(posicao) {
                c.em_voo = None;
            }
            if [c.par.esquerda, c.par.direita].contains(&posicao) {
                c.imagens.insert(
                    posicao,
                    para_gpui(transformacao::aplicar(imagem, crop, true)),
                );
            }
        }
        self.soltar_a_proxima_do_comparar(cx);
        cx.notify();
    }

    /// O motor largou o pedido do Comparar: a foto volta ao começo da fila.
    pub(super) fn devolver_a_fila_do_comparar(&mut self) {
        if let Some(c) = self.comparacao.as_mut() {
            if let Some(posicao) = c.em_voo.take() {
                c.fila.push_front(posicao);
            }
        }
    }

    /// A prévia não existe no disco: a metade fica com o aviso, e a fila anda.
    fn largar_do_comparar(&mut self, posicao: usize, cx: &mut Context<Self>) {
        if let Some(c) = self.comparacao.as_mut() {
            if c.em_voo == Some(posicao) {
                c.em_voo = None;
            }
        }
        self.soltar_a_proxima_do_comparar(cx);
    }

    /// O palco dividido.
    ///
    /// 🔑 **O clique escolhe a metade avaliada** — a de borda âmbar, que recebe
    /// a nota, o `P` e o `X`. As fotos não trocam de lado. Sem zoom e sem
    /// gestos: aqui se escolhe, não se ajusta.
    pub(super) fn palco_do_comparar(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(c) = self.comparacao.as_ref() else {
            return div().into_any_element();
        };
        let total = c.tira.len();
        // A metade é medida pelo palco, que o `canvas` do modo normal mediu:
        // o respiro de 24px dos lados e o vão de 12px entre as duas.
        let largura = ((f32::from(self.palco.size.width) - 12.) / 2.).max(1.);
        let altura = f32::from(self.palco.size.height).max(1.);

        let metade = |lado: Lado, cx: &mut Context<Self>| {
            let posicao = *c.par.foto(lado);
            let ativa = c.par.ativa == lado;
            let foto = self.acervo.get(posicao);
            let imagem = if posicao == self.posicao {
                self.aberta.as_ref().and_then(|a| a.desenhada.clone())
            } else {
                c.imagens.get(&posicao).cloned()
            };
            let na_tira = c
                .tira
                .iter()
                .position(|p| *p == posicao)
                .map_or(0, |i| i + 1);
            let nome = foto.map(|f| f.name.clone()).unwrap_or_default();
            let nota = foto.map_or(0, |f| f.rating.clamp(0, 5) as usize);
            let conteudo = match imagem {
                Some(imagem) => {
                    let tamanho = imagem.size(0);
                    let (x, y, w, h) = crate::revelacao::corte::area_da_foto(
                        (largura, altura),
                        (tamanho.width.0 as f32, tamanho.height.0 as f32),
                    );
                    img(imagem)
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(w))
                        .h(px(h))
                        .object_fit(ObjectFit::Fill)
                        .into_any_element()
                }
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(format!("Preparando {nome}…")))
                    .into_any_element(),
            };
            div()
                .id(SharedString::from(format!("comparar-{lado:?}")))
                .relative()
                .flex_1()
                .h_full()
                .min_w(px(0.))
                .overflow_hidden()
                .border_4()
                .rounded(px(4.))
                .border_color(if ativa {
                    gpui_kit::rgb(0xfbbf24).into()
                } else {
                    gpui_kit::transparent_black()
                })
                .cursor_pointer()
                .on_click(cx.listener(move |tela, _, _w, cx| tela.ativar_no_comparar(lado, cx)))
                .child(conteudo)
                // 🚨 **Sobre a foto, fixo nos dois temas**: o texto se lê
                // contra a foto, e não contra o tema.
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_between()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(6.))
                        .bg(gpui_kit::rgba(0x000000b3))
                        .text_xs()
                        .text_color(gpui_kit::rgb(0xe5e5e5))
                        .child(SharedString::from(format!("{na_tira} / {total}  {nome}")))
                        .child(div().text_color(gpui_kit::rgb(0xfbbf24)).child(
                            SharedString::from(format!(
                                "{}{}",
                                "★".repeat(nota),
                                "☆".repeat(5 - nota)
                            )),
                        )),
                )
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .gap(px(12.))
            .p(px(24.))
            .bg(tema::cores::poco())
            .child(metade(Lado::Esquerda, cx))
            .child(metade(Lado::Direita, cx))
            .into_any_element()
    }
}
