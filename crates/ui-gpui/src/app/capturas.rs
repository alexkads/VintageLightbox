//! 📷 Os dados da captura (ISO, distância focal, abertura, velocidade) entre
//! os balcões, pelo site.
//!
//! O EXIF não sobe com a foto: o que vai ao site é um JPEG regravado, e só o
//! catálogo do computador que importou guarda esses quatro. Para a linha sob o
//! histograma da Revelação aparecer em qualquer balcão (dono, 12/out/2026):
//!
//! 1. ao abrir a sessão, lê-se o que o site já tem (`GET
//!    /pos-venda/galerias/{id}/capturas`);
//! 2. o que este catálogo sabe e o site não — as fotos importadas aqui, as de
//!    sessões antigas inclusive — sobe em um pedido só (`PUT`, o mesmo
//!    caminho);
//! 3. as fotos que vêm do site ganham a captura lida.
//!
//! A leitura se repete quando a sessão muda ou quando a lista de fotos do site
//! cresce (um envio que acabou): não a cada evento da galeria.

use std::collections::HashMap;

use adapters::view_models::{Captura, PhotoViewModel};
use gpui_kit::Context;
use serde_json::{json, Value};

use super::Aplicativo;
use crate::pos_venda::porta::{PedidoJson, Recado};
use crate::revelacao::persistencia;

const LIDAS: &str = "capturas-lidas";
const GRAVADAS: &str = "capturas-gravadas";

fn caminho(galeria: &str) -> String {
    format!("/pos-venda/galerias/{galeria}/capturas")
}

/// O que se sabe das capturas da sessão aberta.
#[derive(Default)]
pub(super) struct CapturasDoSite {
    /// De qual sessão é o mapa abaixo.
    galeria: Option<String>,
    /// Por id da foto **no site**.
    pub(super) por_foto: HashMap<String, Captura>,
    /// Quantas fotos o site tinha quando se leu pela última vez.
    lidas_com: usize,
    /// O site já respondeu o que tem desta sessão? Só depois disso se sabe o
    /// que falta subir.
    lida: bool,
}

/// A resposta do site, foto a foto. O que não tem id ou dado é ignorado.
fn capturas_de(valor: &Value) -> HashMap<String, Captura> {
    valor
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|linha| {
            let id = linha.get("foto_id")?.as_str()?.to_string();
            let captura: Captura = serde_json::from_value(linha.clone()).ok()?;
            (!captura.rotulos().is_empty()).then_some((id, captura))
        })
        .collect()
}

/// O que este catálogo tem e o site não (ou tem diferente), no corpo do `PUT`.
fn a_enviar(daqui: &HashMap<String, Captura>, do_site: &HashMap<String, Captura>) -> Option<Value> {
    let mut linhas: Vec<(&String, &Captura)> = daqui
        .iter()
        .filter(|(id, captura)| do_site.get(*id) != Some(*captura))
        .collect();
    if linhas.is_empty() {
        return None;
    }
    linhas.sort_by(|a, b| a.0.cmp(b.0));
    Some(Value::Array(
        linhas
            .into_iter()
            .map(|(id, c)| {
                json!({
                    "foto_id": id,
                    "iso": c.iso,
                    "focal": c.focal,
                    "abertura": c.abertura,
                    "velocidade": c.velocidade,
                })
            })
            .collect(),
    ))
}

impl Aplicativo {
    /// As fotos do site chegaram: é hora de ler as capturas, se a sessão é
    /// outra ou se a lista cresceu.
    pub(super) fn sincronizar_capturas(&mut self, cx: &mut Context<Self>) {
        let (Some(sessao), Some(galeria)) = (self.sessao.clone(), self.sessao_aberta.clone())
        else {
            return;
        };
        let quantas = self.fotos_do_site.len();
        let outra = self.capturas.galeria.as_deref() != Some(galeria.as_str());
        if outra {
            self.capturas = CapturasDoSite {
                galeria: Some(galeria.clone()),
                ..Default::default()
            };
        } else if quantas <= self.capturas.lidas_com {
            // A mesma sessão, sem foto nova: o que já veio continua valendo.
            self.aplicar_capturas(cx);
            return;
        }
        self.capturas.lidas_com = quantas;
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler(LIDAS, caminho(&galeria)),
            self.recados_das_capturas.0.clone(),
        );
        self.esperar_capturas(cx);
    }

    /// As capturas que **este catálogo** tem das fotos da sessão aberta, por
    /// id do site.
    fn capturas_daqui(&self, catalogo: &[PhotoViewModel]) -> HashMap<String, Captura> {
        let do_site: std::collections::HashSet<&str> = self
            .fotos_do_site
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.as_deref())
            .collect();
        catalogo
            .iter()
            // Só a linha do catálogo: a cópia do site está na mesma lista.
            .filter(|f| !f.path.is_empty() && !f.captura.rotulos().is_empty())
            .filter_map(|f| {
                let id = f.pos_venda_foto_id.as_deref()?;
                do_site
                    .contains(id)
                    .then(|| (id.to_string(), f.captura.clone()))
            })
            .collect()
    }

    /// O catálogo foi relido: o que ele sabe e o site ainda não tem sobe.
    ///
    /// 🔑 **É aqui que o envio acontece de verdade.** Quando as fotos do site
    /// chegam, as linhas do catálogo saem da lista da grade até a releitura
    /// (`absorver_as_do_site`); medir só naquele instante não acharia nada.
    pub(super) fn capturas_do_catalogo_relido(
        &mut self,
        catalogo: &[PhotoViewModel],
        cx: &mut Context<Self>,
    ) {
        if !self.capturas.lida {
            return;
        }
        let daqui = self.capturas_daqui(catalogo);
        self.subir_as_que_faltam(daqui, cx);
    }

    /// Manda ao site o que este catálogo tem e ele não, e já passa a valer aqui.
    fn subir_as_que_faltam(&mut self, daqui: HashMap<String, Captura>, cx: &mut Context<Self>) {
        let corpo = a_enviar(&daqui, &self.capturas.por_foto);
        self.capturas.por_foto.extend(daqui);
        self.aplicar_capturas(cx);
        let (Some(corpo), Some(sessao), Some(galeria)) =
            (corpo, self.sessao.clone(), self.capturas.galeria.clone())
        else {
            return;
        };
        self.publicador.pedir_json(
            sessao,
            PedidoJson::gravar(GRAVADAS, "PUT", caminho(&galeria), corpo),
            self.recados_das_capturas.0.clone(),
        );
        self.esperar_capturas(cx);
    }

    /// Põe as capturas lidas nas fotos do site e na foto aberta na Revelação.
    fn aplicar_capturas(&mut self, cx: &mut Context<Self>) {
        if self.capturas.por_foto.is_empty() {
            return;
        }
        for foto in &mut self.fotos_do_site {
            let Some(id) = foto.pos_venda_foto_id.as_deref() else {
                continue;
            };
            if let Some(captura) = self.capturas.por_foto.get(id) {
                if foto.captura.rotulos().is_empty() {
                    foto.captura = captura.clone();
                }
            }
        }
        let aberta = self
            .revelacao
            .read(cx)
            .foto_aberta()
            .filter(|f| persistencia::so_existe_no_site(f))
            .and_then(|f| Some((f.id.clone(), f.pos_venda_foto_id.clone()?)));
        if let Some((id, no_site)) = aberta {
            if let Some(captura) = self.capturas.por_foto.get(&no_site).cloned() {
                self.revelacao
                    .update(cx, |tela, cx| tela.definir_captura(&id, captura, cx));
            }
        }
    }

    /// A captura que o site tem de uma foto — para a Revelação que abre.
    pub(super) fn captura_do_site(&self, no_site: &str) -> Option<&Captura> {
        self.capturas.por_foto.get(no_site)
    }

    /// Colhe as respostas enquanto houver alguma a caminho (até 30 s).
    fn esperar_capturas(&mut self, cx: &mut Context<Self>) {
        self.capturas_a_caminho += 1;
        if self._capturas.is_some() {
            return;
        }
        self._capturas = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..300 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(acabou) = raiz.update(cx, |raiz, cx| raiz.colher_capturas(cx)) else {
                    return;
                };
                if acabou {
                    break;
                }
            }
            let _ = raiz.update(cx, |raiz, _| {
                raiz._capturas = None;
                raiz.capturas_a_caminho = 0;
            });
        }));
    }

    /// Trata o que chegou. `true` quando não há mais nada a caminho.
    pub(crate) fn colher_capturas(&mut self, cx: &mut Context<Self>) -> bool {
        while let Ok(recado) = self.recados_das_capturas.1.try_recv() {
            let Recado::Json { rotulo, resultado } = recado else {
                continue;
            };
            self.capturas_a_caminho = self.capturas_a_caminho.saturating_sub(1);
            match (rotulo, resultado) {
                (LIDAS, Ok(valor)) => {
                    self.capturas.por_foto = capturas_de(&valor);
                    self.capturas.lida = true;
                    let catalogo = self.biblioteca.read(cx).todas_as_fotos();
                    let daqui = self.capturas_daqui(&catalogo);
                    self.subir_as_que_faltam(daqui, cx);
                }
                // Acessório: sem rede, ou num site mais velho que o app, a
                // linha sob o histograma só fica sem os dados.
                (LIDAS | GRAVADAS, Err(erro)) => {
                    // A próxima chegada de fotos tenta de novo.
                    self.capturas.lidas_com = 0;
                    self.capturas.lida = false;
                    crate::telemetria::avisar!("⚠️ [Capturas] os dados da câmera: {erro}");
                }
                _ => {}
            }
        }
        self.capturas_a_caminho == 0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn captura(iso: u32) -> Captura {
        Captura {
            iso: Some(iso),
            focal: Some(27.0),
            abertura: Some(11.0),
            velocidade: Some("1/160".into()),
        }
    }

    #[test]
    fn le_as_capturas_do_site_e_ignora_o_que_nao_tem_dado() {
        let lidas = capturas_de(&json!([
            { "foto_id": "a", "iso": 1000, "focal": 27.0, "abertura": 11.0, "velocidade": "1/160" },
            { "foto_id": "b", "iso": null, "focal": null, "abertura": null, "velocidade": null },
            { "iso": 400 },
        ]));
        assert_eq!(lidas.len(), 1);
        assert_eq!(lidas["a"], captura(1000));
        assert!(capturas_de(&json!({ "erro": "não é lista" })).is_empty());
    }

    #[test]
    fn so_sobe_o_que_o_site_nao_tem_ou_tem_diferente() {
        let daqui = HashMap::from([
            ("a".to_string(), captura(1000)),
            ("b".to_string(), captura(200)),
            ("c".to_string(), captura(800)),
        ]);
        let do_site = HashMap::from([
            ("a".to_string(), captura(1000)),
            ("b".to_string(), captura(100)),
        ]);
        let corpo = a_enviar(&daqui, &do_site).expect("há o que subir");
        let ids: Vec<&str> = corpo
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["foto_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["b", "c"]);
        assert_eq!(corpo[0]["iso"], 200);
        // Tudo igual: nenhum pedido.
        assert!(a_enviar(&do_site, &do_site).is_none());
    }
}
