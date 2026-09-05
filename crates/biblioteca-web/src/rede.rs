//! O `fetch` do navegador, visto de dentro do wasm.
//!
//! # Por que o wasm fala com o **site**, e não com a API
//!
//! O token da API mora num cookie `httpOnly`, que nenhum JavaScript lê — e
//! isso é uma decisão, não um acidente. As chamadas daqui vão para rotas do
//! próprio site (`/dashboard/sessoes-fotograficas/{id}/api/...`), que repassam
//! à API com o token do cookie. É o mesmo desenho do bilhete de envio. O
//! `credentials: same-origin` é o que faz o cookie viajar junto.
//!
//! # Como a resposta chega à tela
//!
//! O egui é modo imediato: não há `await` dentro de um quadro. A chamada é
//! disparada com `spawn_local`, e o resultado cai numa **caixa de correio**
//! (`Caixa`) que a tela esvazia no começo de cada quadro. Nada aqui bloqueia.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestCredentials, RequestInit, Response};

/// O que voltou de uma chamada.
pub enum Chegada {
    /// O JSON da galeria inteira, para trocar o acervo.
    Estado(Result<String, String>),
    /// O resultado de uma ação (lote, estúdio, aviso, link, recibo).
    Acao {
        rotulo: String,
        resultado: Result<String, String>,
    },
    /// Os bytes de uma miniatura.
    Miniatura {
        url: String,
        bytes: Result<Vec<u8>, String>,
    },
    /// O que há no depósito local sobre revelações: `(foto_id, json)`.
    RevelacoesLocais(Result<Vec<(String, String)>, String>),
}

#[derive(Clone, Default)]
pub struct Caixa {
    chegadas: Rc<RefCell<Vec<Chegada>>>,
    /// Quem acordar quando algo chega — o hospedeiro agenda um quadro.
    ///
    /// 🚨 Sem isto a resposta ficava na caixa até o próximo evento do mouse:
    /// o laço de quadros dorme quando a tela está parada (custo zero), e uma
    /// miniatura que chega com a tela parada não tem quem a desenhe.
    despertador: Rc<RefCell<Option<js_sys::Function>>>,
}

impl Caixa {
    pub fn esvaziar(&self) -> Vec<Chegada> {
        std::mem::take(&mut *self.chegadas.borrow_mut())
    }

    pub fn definir_despertador(&self, f: js_sys::Function) {
        *self.despertador.borrow_mut() = Some(f);
    }

    fn depositar(&self, chegada: Chegada) {
        self.chegadas.borrow_mut().push(chegada);
        if let Some(f) = self.despertador.borrow().as_ref() {
            let _ = f.call0(&wasm_bindgen::JsValue::NULL);
        }
    }

    pub fn buscar_estado(&self, url: String) {
        let caixa = self.clone();
        spawn_local(async move {
            caixa.depositar(Chegada::Estado(texto(&url, None).await));
        });
    }

    pub fn enviar_acao(&self, url: String, corpo: String, rotulo: String) {
        let caixa = self.clone();
        spawn_local(async move {
            let resultado = texto(&url, Some(corpo)).await;
            caixa.depositar(Chegada::Acao { rotulo, resultado });
        });
    }

    /// Relê do depósito local o que o editor de revelação gravou.
    pub fn listar_revelacoes_locais(&self) {
        let caixa = self.clone();
        spawn_local(async move {
            let lista = crate::local::listar(crate::local::LOJA_REVELACOES).await;
            caixa.depositar(Chegada::RevelacoesLocais(lista));
        });
    }

    /// Guarda a galeria no depósito local — sem esperar, sem avisar: é o
    /// retrato para a próxima abertura, e falhar aqui não muda a tela.
    pub fn guardar_galeria(&self, id: String, json: String) {
        spawn_local(async move {
            let _ = crate::local::gravar(crate::local::LOJA_GALERIAS, &id, &json).await;
        });
    }

    pub fn buscar_miniatura(&self, url: String) {
        let caixa = self.clone();
        spawn_local(async move {
            let bytes = bytes(&url).await;
            caixa.depositar(Chegada::Miniatura { url, bytes });
        });
    }
}

fn erro_js(valor: wasm_bindgen::JsValue) -> String {
    valor
        .as_string()
        .or_else(|| js_sys::Error::from(valor).message().as_string())
        .unwrap_or_else(|| "falha de rede".to_string())
}

async fn responder(url: &str, corpo_json: Option<String>) -> Result<Response, String> {
    let init = RequestInit::new();
    init.set_credentials(RequestCredentials::SameOrigin);
    if let Some(corpo) = corpo_json {
        init.set_method("POST");
        let cabecalhos = web_sys::Headers::new().map_err(erro_js)?;
        cabecalhos
            .set("content-type", "application/json")
            .map_err(erro_js)?;
        init.set_headers(&cabecalhos);
        init.set_body(&wasm_bindgen::JsValue::from_str(&corpo));
    } else {
        init.set_method("GET");
    }
    let pedido = Request::new_with_str_and_init(url, &init).map_err(erro_js)?;
    let janela = web_sys::window().ok_or("sem janela")?;
    let resposta = JsFuture::from(janela.fetch_with_request(&pedido))
        .await
        .map_err(erro_js)?;
    resposta
        .dyn_into::<Response>()
        .map_err(|_| "a resposta não é uma Response".to_string())
}

/// O corpo como texto. Um status fora de 2xx vira erro **com o corpo**, porque
/// é lá que o site escreve a frase que diz o que corrigir.
async fn texto(url: &str, corpo_json: Option<String>) -> Result<String, String> {
    let resposta = responder(url, corpo_json).await?;
    let corpo = JsFuture::from(resposta.text().map_err(erro_js)?)
        .await
        .map_err(erro_js)?
        .as_string()
        .unwrap_or_default();
    if resposta.ok() {
        Ok(corpo)
    } else {
        let mensagem = serde_json::from_str::<serde_json::Value>(&corpo)
            .ok()
            .and_then(|v| {
                v.get("message")
                    .and_then(|m| m.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {}", resposta.status()));
        Err(mensagem)
    }
}

async fn bytes(url: &str) -> Result<Vec<u8>, String> {
    let resposta = responder(url, None).await?;
    if !resposta.ok() {
        return Err(format!("HTTP {}", resposta.status()));
    }
    let buffer = JsFuture::from(resposta.array_buffer().map_err(erro_js)?)
        .await
        .map_err(erro_js)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// O `localStorage`, quando existe — para o zoom e os parâmetros da leva.
pub fn guardar(chave: &str, valor: &str) {
    if let Some(armazem) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = armazem.set_item(chave, valor);
    }
}

pub fn lembrar(chave: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|a| a.get_item(chave).ok().flatten())
}
