//! O bruto em resolução cheia no zoom — o `detalhe` do `editor.tsx`.
//!
//! 🔑 **A cópia de trabalho abre rápido; conferir foco é olhar o arquivo.**
//! Quando o zoom passa de um pixel da cópia por pixel da tela
//! ([`zoom::precisa_do_bruto`]), o palco troca a cópia pelo bruto, 250 ms depois
//! (andar pela tira em 1:1 não baixa um bruto por foto que só passou). Voltando
//! ao encaixe, a cópia volta 1,5 s depois, e os sliders ficam leves de novo.
//!
//! A troca é **só do que a GPU desenha**: receita, histórico, tira e depósito
//! não mudam. O "1:1" já conta em pixels do bruto desde a abertura, porque o
//! lado dele é medido antes ([`Revelacao::definir_lado_do_bruto`]).

use std::time::Duration;

use gpui_kit::{Context, Task};

use super::{Origem, PedidoDaRevelacao, Revelacao};
use crate::revelacao::zoom;

const ESPERA_PARA_BUSCAR: Duration = Duration::from_millis(250);
const ESPERA_PARA_LARGAR: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Fase {
    #[default]
    Copia,
    Carregando,
    NaTela,
    Falhou,
}

/// O estado do bruto da foto aberta.
#[derive(Default)]
pub(super) struct Resolucao {
    /// O maior lado do bruto, em pixels, e de qual foto ele é.
    lado: Option<(String, f32)>,
    fase: Fase,
    /// A cópia de trabalho guardada enquanto o bruto está na tela.
    copia: Option<Origem>,
    /// A espera de 250 ms ou de 1,5 s. Trocar a tarefa cancela a anterior.
    _espera: Option<Task<()>>,
    /// Para que lado a espera em curso vai: `true` = buscar.
    esperando: Option<bool>,
}

impl Resolucao {
    /// Pixels novos da cópia chegaram: o bruto que estava na tela não vale
    /// mais, mas o lado medido continua.
    pub(super) fn recomecar_na_copia(&mut self) {
        let lado = self.lado.take();
        *self = Self {
            lado,
            ..Self::default()
        };
    }
}

impl Revelacao {
    /// A raiz mediu o bruto desta foto.
    pub fn definir_lado_do_bruto(&mut self, foto_id: &str, lado: f32, cx: &mut Context<Self>) {
        if self.foto_aberta().is_some_and(|f| f.id == foto_id) {
            self.resolucao.lado = Some((foto_id.to_string(), lado));
            cx.notify();
        }
    }

    /// Foto nova: tudo recomeça na cópia.
    pub(super) fn esquecer_a_resolucao(&mut self) {
        self.resolucao = Resolucao::default();
    }

    /// Os pixels da cópia de trabalho: os da origem, ou os guardados enquanto o
    /// bruto está na tela.
    pub(super) fn tamanho_da_copia(&self) -> Option<(f32, f32)> {
        if let Some(copia) = &self.resolucao.copia {
            return Some((copia.largura as f32, copia.altura as f32));
        }
        let origem = self.aberta.as_ref()?.origem.as_ref()?;
        Some((origem.largura as f32, origem.altura as f32))
    }

    /// Quantos pixels do bruto cabem num pixel da cópia (≥ 1).
    pub(super) fn fator_do_bruto(&self) -> f32 {
        let aberta = self.aberta.as_ref().map(|a| a.foto.id.as_str());
        let Some((id, lado)) = &self.resolucao.lado else {
            return 1.;
        };
        if Some(id.as_str()) != aberta {
            return 1.;
        }
        match self.tamanho_da_copia() {
            Some((l, a)) if l.max(a) > 0. => (lado / l.max(a)).max(1.),
            _ => 1.,
        }
    }

    /// "resolução cheia…" no controle do zoom.
    pub fn carregando_o_bruto(&self) -> bool {
        self.resolucao.fase == Fase::Carregando
    }

    pub fn bruto_na_tela(&self) -> bool {
        self.resolucao.fase == Fase::NaTela
    }

    fn quer_o_bruto(&self) -> bool {
        self.vista().is_some_and(|(cena, vista)| {
            zoom::precisa_do_bruto(&self.navegacao.zoom, &vista, &cena)
        })
    }

    /// Chamado a cada quadro: decide se é hora de buscar ou de largar o bruto.
    pub(super) fn acompanhar_a_resolucao(&mut self, cx: &mut Context<Self>) {
        let quer = self.quer_o_bruto();
        let fase = self.resolucao.fase;
        let acao = match (quer, fase) {
            (true, Fase::Copia) => Some(true),
            (false, Fase::NaTela) => Some(false),
            _ => None,
        };
        if acao == self.resolucao.esperando {
            return;
        }
        self.resolucao.esperando = acao;
        let Some(buscar) = acao else {
            self.resolucao._espera = None;
            return;
        };
        let espera = if buscar {
            ESPERA_PARA_BUSCAR
        } else {
            ESPERA_PARA_LARGAR
        };
        self.resolucao._espera = Some(cx.spawn(async move |tela, cx| {
            cx.background_executor().timer(espera).await;
            let _ = tela.update(cx, |tela, cx| {
                tela.resolucao.esperando = None;
                // 🛡️ O zoom pode ter voltado durante a espera.
                if tela.quer_o_bruto() != buscar {
                    return;
                }
                if buscar {
                    tela.pedir_o_bruto(cx);
                } else {
                    tela.voltar_para_a_copia(cx);
                }
            });
        }));
    }

    fn pedir_o_bruto(&mut self, cx: &mut Context<Self>) {
        if self.resolucao.fase != Fase::Copia {
            return;
        }
        if self.foto_aberta().is_none() {
            return;
        }
        self.resolucao.fase = Fase::Carregando;
        cx.emit(PedidoDaRevelacao::QueroOBruto);
        cx.notify();
    }

    /// A raiz não conseguiu o bruto: a tela fica na cópia.
    pub fn bruto_indisponivel(&mut self, foto_id: &str, cx: &mut Context<Self>) {
        if self.foto_aberta().is_some_and(|f| f.id == foto_id)
            && self.resolucao.fase == Fase::Carregando
        {
            self.resolucao.fase = Fase::Falhou;
            cx.notify();
        }
    }

    /// O bruto decodificado chegou.
    pub fn receber_bruto(
        &mut self,
        foto_id: &str,
        imagem: image::DynamicImage,
        cx: &mut Context<Self>,
    ) {
        if self.resolucao.fase != Fase::Carregando || !self.quer_o_bruto() {
            if self.resolucao.fase == Fase::Carregando {
                self.resolucao.fase = Fase::Copia;
            }
            return;
        }
        let Some(aberta) = self.aberta.as_mut() else {
            return;
        };
        if aberta.foto.id != foto_id {
            return;
        }
        let rgba = imagem.to_rgba8();
        let bruto = Origem {
            largura: rgba.width(),
            altura: rgba.height(),
            pixels: std::sync::Arc::new(rgba.into_raw()),
        };
        self.resolucao.copia = aberta.origem.replace(bruto);
        aberta.bruta = Some(imagem);
        self.resolucao.fase = Fase::NaTela;
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// De volta à cópia de trabalho.
    fn voltar_para_a_copia(&mut self, cx: &mut Context<Self>) {
        let Some(copia) = self.resolucao.copia.take() else {
            self.resolucao.fase = Fase::Copia;
            return;
        };
        if let Some(aberta) = self.aberta.as_mut() {
            let rgba =
                image::RgbaImage::from_raw(copia.largura, copia.altura, (*copia.pixels).clone());
            aberta.bruta = rgba.map(image::DynamicImage::ImageRgba8);
            aberta.origem = Some(copia);
        }
        self.resolucao.fase = Fase::Copia;
        self.pedir_revelacao(cx);
        cx.notify();
    }
}
