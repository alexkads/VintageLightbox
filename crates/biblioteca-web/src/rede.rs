//! O `fetch` do navegador, visto de dentro do wasm — **só para miniaturas**.
//!
//! # O wasm não chama API nenhuma
//!
//! A única coisa que sai daqui é `GET` da miniatura, pelo proxy do próprio
//! site (`/dashboard/sessoes-fotograficas/fotos/{id}/miniatura`), com
//! `credentials: same-origin` para o cookie viajar. Gravar (estado, faixa,
//! negociação, preço, apagar) é Server Action chamada pelo React, como em
//! qualquer outra tela do painel. Já foi diferente — em 2026-09-05 o wasm
//! chamava rotas-proxy `api/acao` e `api/estado` — e a decisão do dono no
//! mesmo dia foi voltar: ver `docs/BIBLIOTECA_NO_NAVEGADOR.md` do site.
//!
//! # Como a resposta chega à grade
//!
//! Não há `await` dentro de um quadro. A busca é disparada com `spawn_local`,
//! e os bytes caem numa **caixa de correio** (`Caixa`) que a grade esvazia no
//! começo de cada quadro. Nada aqui bloqueia.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestCredentials, RequestInit, Response};

/// Os bytes de uma miniatura, ou o motivo de não terem vindo.
pub struct Miniatura {
    pub url: String,
    pub bytes: Result<Vec<u8>, String>,
}

#[derive(Clone, Default)]
pub struct Caixa {
    chegadas: Rc<RefCell<Vec<Miniatura>>>,
    /// Quem acordar quando algo chega — o hospedeiro agenda um quadro.
    ///
    /// 🚨 Sem isto a miniatura ficava na caixa até o próximo evento do mouse:
    /// o laço de quadros dorme quando a tela está parada (custo zero), e uma
    /// miniatura que chega com a tela parada não tem quem a desenhe.
    despertador: Rc<RefCell<Option<js_sys::Function>>>,
}

impl Caixa {
    pub fn esvaziar(&self) -> Vec<Miniatura> {
        std::mem::take(&mut *self.chegadas.borrow_mut())
    }

    pub fn definir_despertador(&self, f: js_sys::Function) {
        *self.despertador.borrow_mut() = Some(f);
    }

    fn depositar(&self, chegada: Miniatura) {
        self.chegadas.borrow_mut().push(chegada);
        if let Some(f) = self.despertador.borrow().as_ref() {
            let _ = f.call0(&wasm_bindgen::JsValue::NULL);
        }
    }

    pub fn buscar_miniatura(&self, url: String) {
        let caixa = self.clone();
        spawn_local(async move {
            let bytes = bytes(&url).await;
            caixa.depositar(Miniatura { url, bytes });
        });
    }
}

fn erro_js(valor: wasm_bindgen::JsValue) -> String {
    valor
        .as_string()
        .or_else(|| js_sys::Error::from(valor).message().as_string())
        .unwrap_or_else(|| "falha de rede".to_string())
}

async fn bytes(url: &str) -> Result<Vec<u8>, String> {
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_credentials(RequestCredentials::SameOrigin);
    let pedido = Request::new_with_str_and_init(url, &init).map_err(erro_js)?;
    let janela = web_sys::window().ok_or("sem janela")?;
    let resposta = JsFuture::from(janela.fetch_with_request(&pedido))
        .await
        .map_err(erro_js)?;
    let resposta = resposta
        .dyn_into::<Response>()
        .map_err(|_| "a resposta não é uma Response".to_string())?;
    if !resposta.ok() {
        return Err(format!("HTTP {}", resposta.status()));
    }
    let buffer = JsFuture::from(resposta.array_buffer().map_err(erro_js)?)
        .await
        .map_err(erro_js)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
