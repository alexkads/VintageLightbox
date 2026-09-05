//! O depósito local — o IndexedDB do navegador, visto de dentro do wasm.
//!
//! # Por que existe (dono, 2026-09-05)
//!
//! *"A biblioteca e a revelação devem compartilhar a mesma informação local, e
//! a edição das fotos não precisa depender do botão salvar na galeria para
//! persistir."* Um depósito só, com um esquema só, escrito por quem grava
//! (hoje o editor de revelação, ainda em React; amanhã o próprio wasm) e lido
//! por quem mostra (a biblioteca). O que o operador ajusta numa foto fica aqui
//! a cada gesto; "Salvar na galeria" vira o passo que **sincroniza** com o
//! backend — e é o único que custa API.
//!
//! # O esquema mora aqui, e só aqui
//!
//! 🔑 [`esquema_json`] é o que o lado TypeScript lê antes de abrir o banco:
//! nome, versão e lojas vêm de uma fonte só. Dois esquemas para o mesmo banco
//! seriam a armadilha nº 8 no lugar em que ela apaga dado de cliente — um
//! `onupgradeneeded` de cada lado, com listas diferentes, e o banco fica sem a
//! loja que o outro esperava.
//!
//! # Como uma `IDBRequest` vira `await`
//!
//! O IndexedDB avisa por `onsuccess`/`onerror`. Cada pedido é embrulhado numa
//! `Promise` cujos `resolve`/`reject` são presos a esses dois — e a `JsFuture`
//! faz o resto. Os fechos são `once_into_js`: vivem até serem chamados e depois
//! o JavaScript os coleta.

use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

pub const NOME: &str = "recordarfotos-biblioteca";
pub const VERSAO: u32 = 1;
/// As lojas: cada uma guarda JSON em texto, com a chave fora do valor.
pub const LOJAS: [&str; 3] = [LOJA_REVELACOES, LOJA_GALERIAS, LOJA_PARAMETROS];
/// Por foto: os ajustes e o enquadramento, e se já subiram.
pub const LOJA_REVELACOES: &str = "revelacoes";
/// Por galeria: o último estado lido, para abrir sem esperar a rede.
pub const LOJA_GALERIAS: &str = "galerias";
/// Preferências que valem para toda galeria (zoom, leva).
pub const LOJA_PARAMETROS: &str = "parametros";

#[derive(Serialize)]
pub struct Esquema {
    pub nome: &'static str,
    pub versao: u32,
    pub lojas: [&'static str; 3],
}

pub fn esquema() -> Esquema {
    Esquema {
        nome: NOME,
        versao: VERSAO,
        lojas: LOJAS,
    }
}

fn erro(v: JsValue) -> String {
    v.as_string()
        .or_else(|| js_sys::Error::from(v.clone()).message().as_string())
        .unwrap_or_else(|| "falha no depósito local".into())
}

/// Espera uma `IDBRequest` terminar e devolve o `result`.
async fn aguardar(pedido: &IdbRequest) -> Result<JsValue, String> {
    let promessa = js_sys::Promise::new(&mut |resolve, reject| {
        let p = pedido.clone();
        let ok = Closure::once_into_js(move |_: web_sys::Event| {
            let _ = resolve.call1(&JsValue::NULL, &p.result().unwrap_or(JsValue::UNDEFINED));
        });
        let falha = Closure::once_into_js(move |e: web_sys::Event| {
            let mensagem = e
                .target()
                .and_then(|t| t.dyn_into::<IdbRequest>().ok())
                .and_then(|r| r.error().ok().flatten())
                .map(|d| d.message())
                .unwrap_or_else(|| "IndexedDB recusou".into());
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&mensagem));
        });
        pedido.set_onsuccess(Some(ok.unchecked_ref()));
        pedido.set_onerror(Some(falha.unchecked_ref()));
    });
    JsFuture::from(promessa).await.map_err(erro)
}

/// Abre (ou cria) o banco. Cria as lojas que faltarem — o mesmo laço que o
/// lado TypeScript faz com o mesmo esquema.
pub async fn abrir() -> Result<IdbDatabase, String> {
    let janela = web_sys::window().ok_or("sem janela")?;
    let fabrica = janela
        .indexed_db()
        .map_err(erro)?
        .ok_or("este navegador não tem IndexedDB")?;
    let pedido = fabrica.open_with_u32(NOME, VERSAO).map_err(erro)?;

    let ao_atualizar = Closure::once_into_js(move |e: web_sys::IdbVersionChangeEvent| {
        let Some(banco) = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::IdbOpenDbRequest>().ok())
            .and_then(|r| r.result().ok())
            .and_then(|r| r.dyn_into::<IdbDatabase>().ok())
        else {
            return;
        };
        let existentes = banco.object_store_names();
        for loja in LOJAS {
            let ja_tem = (0..existentes.length())
                .filter_map(|i| existentes.get(i))
                .any(|n| n == loja);
            if !ja_tem {
                let _ = banco.create_object_store(loja);
            }
        }
    });
    pedido.set_onupgradeneeded(Some(ao_atualizar.unchecked_ref()));

    let resultado = aguardar(pedido.unchecked_ref::<IdbRequest>()).await?;
    resultado
        .dyn_into::<IdbDatabase>()
        .map_err(|_| "o IndexedDB não devolveu um banco".to_string())
}

/// Ainda sem chamador aqui: é o que o editor de revelação vai usar quando for
/// portado para este wasm — hoje quem lê por foto é o `local.ts` do site.
#[allow(dead_code)]
pub async fn ler(loja: &str, chave: &str) -> Result<Option<String>, String> {
    let banco = abrir().await?;
    let transacao = banco
        .transaction_with_str_and_mode(loja, IdbTransactionMode::Readonly)
        .map_err(erro)?;
    let objetos = transacao.object_store(loja).map_err(erro)?;
    let pedido = objetos.get(&JsValue::from_str(chave)).map_err(erro)?;
    let valor = aguardar(&pedido).await?;
    Ok(valor.as_string())
}

pub async fn gravar(loja: &str, chave: &str, valor_json: &str) -> Result<(), String> {
    let banco = abrir().await?;
    let transacao = banco
        .transaction_with_str_and_mode(loja, IdbTransactionMode::Readwrite)
        .map_err(erro)?;
    let objetos = transacao.object_store(loja).map_err(erro)?;
    let pedido = objetos
        .put_with_key(&JsValue::from_str(valor_json), &JsValue::from_str(chave))
        .map_err(erro)?;
    aguardar(&pedido).await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn apagar(loja: &str, chave: &str) -> Result<(), String> {
    let banco = abrir().await?;
    let transacao = banco
        .transaction_with_str_and_mode(loja, IdbTransactionMode::Readwrite)
        .map_err(erro)?;
    let objetos = transacao.object_store(loja).map_err(erro)?;
    let pedido = objetos.delete(&JsValue::from_str(chave)).map_err(erro)?;
    aguardar(&pedido).await?;
    Ok(())
}

/// Tudo o que há numa loja, como `(chave, json)`.
pub async fn listar(loja: &str) -> Result<Vec<(String, String)>, String> {
    let banco = abrir().await?;
    let transacao = banco
        .transaction_with_str_and_mode(loja, IdbTransactionMode::Readonly)
        .map_err(erro)?;
    let objetos = transacao.object_store(loja).map_err(erro)?;
    let chaves = aguardar(&objetos.get_all_keys().map_err(erro)?).await?;
    let valores = aguardar(&objetos.get_all().map_err(erro)?).await?;
    let chaves = js_sys::Array::from(&chaves);
    let valores = js_sys::Array::from(&valores);
    Ok(chaves
        .iter()
        .zip(valores.iter())
        .filter_map(|(c, v)| Some((c.as_string()?, v.as_string()?)))
        .collect())
}
