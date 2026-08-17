//! A fila de pedidos de revelação: uma thread, um canal, e o descarte do que já
//! não interessa.
//!
//! 🔑 **O motor não está mais aqui.** O wgpu, o WGSL e os 46 ajustes foram para
//! [`infrastructure::gpu_adjustments`] em 17/ago/2026, porque a exportação
//! precisava do **mesmo** shader que a tela — e ela não pode depender do crate
//! de interface. O que sobrou neste arquivo é o que só a tela precisa.
//!
//! ## Por que thread + canal
//!
//! Não é herança: é o que a tela precisa. Arrastar um slider gera dezenas de
//! pedidos por segundo, e cada um custa upload de textura, dispatch e leitura de
//! volta. Fazer isso no `render` congelaria a janela no arrasto — que é
//! exatamente o momento em que ela precisa responder.
//!
//! O descarte de pedido velho (`id < atual`) é a outra metade: sem ele, soltar o
//! slider deixaria uma fila de quadros intermediários para desenhar, e a imagem
//! chegaria ao valor final segundos depois do dedo.
//!
//! ⚠️ **Sem adaptador de GPU, [`Processador::disponivel`] responde `false`** e a
//! Revelação mostra a foto sem ajuste — honesto e visível, em vez de
//! silenciosamente certo-por-outro-caminho. O caminho de CPU do `crates/ui` era
//! uma segunda implementação da mesma matemática, com resultado diferente do
//! shader, e não veio junto de propósito.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use image::DynamicImage;
use parking_lot::Mutex;

pub use infrastructure::gpu_adjustments::Ajustes;
use infrastructure::gpu_adjustments::Motor;

/// Um pedido de revelação.
pub struct Pedido {
    pub id: u64,
    /// `Arc` para o pixel não ser copiado a cada arrasto — e, do outro lado, é a
    /// **identidade** do `Arc` que decide se a textura precisa subir de novo.
    /// Mover 24 MB para a GPU a cada milímetro de slider é o que separa arrastar
    /// liso de arrastar aos trancos.
    pub pixels: Arc<Vec<u8>>,
    pub largura: u32,
    pub altura: u32,
    pub ajustes: Ajustes,
}

/// O que volta.
pub struct Resultado {
    pub id: u64,
    pub imagem: DynamicImage,
    pub duracao_ms: f32,
}

pub struct Processador {
    pedidos: Sender<Pedido>,
    resultados: Receiver<Resultado>,
    /// O id do pedido mais recente. A thread compara contra ele para largar o
    /// que já não interessa, e por isso ele é compartilhado, não copiado.
    id_atual: Arc<Mutex<u64>>,
    disponivel: Arc<Mutex<Option<bool>>>,
}

impl Processador {
    pub fn novo() -> Self {
        let (envia_pedido, recebe_pedido) = channel::<Pedido>();
        let (envia_resultado, recebe_resultado) = channel::<Resultado>();
        let id_atual = Arc::new(Mutex::new(0u64));
        let disponivel = Arc::new(Mutex::new(None));

        {
            let id_atual = id_atual.clone();
            let disponivel = disponivel.clone();
            std::thread::spawn(move || {
                laco(recebe_pedido, envia_resultado, id_atual, disponivel);
            });
        }

        Self {
            pedidos: envia_pedido,
            resultados: recebe_resultado,
            id_atual,
            disponivel,
        }
    }

    /// `None` enquanto a thread ainda está abrindo o dispositivo.
    ///
    /// Três estados, e não dois: "ainda não sei" é diferente de "não tem GPU", e
    /// tratá-los igual faria a tela anunciar ausência de placa durante os
    /// milissegundos de abertura — em toda abertura.
    pub fn disponivel(&self) -> Option<bool> {
        *self.disponivel.lock()
    }

    pub fn proximo_id(&self) -> u64 {
        let mut guarda = self.id_atual.lock();
        *guarda += 1;
        *guarda
    }

    /// Enfileira, e marca este como o pedido que interessa.
    pub fn pedir(&self, pedido: Pedido) -> u64 {
        let id = pedido.id;
        *self.id_atual.lock() = id;
        let _ = self.pedidos.send(pedido);
        id
    }

    /// O resultado mais recente que chegou, descartando os atrasados.
    ///
    /// Drena a fila inteira em vez de devolver o primeiro: durante um arrasto
    /// chegam vários, e desenhar os intermediários é gastar quadro para mostrar
    /// estado que já passou.
    pub fn colher(&self) -> Option<Resultado> {
        let mut ultimo: Option<Resultado> = None;
        while let Ok(resultado) = self.resultados.try_recv() {
            if ultimo.as_ref().is_none_or(|u| resultado.id > u.id) {
                ultimo = Some(resultado);
            }
        }
        ultimo
    }
}

impl Default for Processador {
    fn default() -> Self {
        Self::novo()
    }
}
/// O laço da thread: abre o motor e atende pedidos até o canal fechar.
fn laco(
    pedidos: Receiver<Pedido>,
    resultados: Sender<Resultado>,
    id_atual: Arc<Mutex<u64>>,
    disponivel: Arc<Mutex<Option<bool>>>,
) {
    let Some(mut motor) = Motor::abrir() else {
        *disponivel.lock() = Some(false);
        return;
    };
    *disponivel.lock() = Some(true);

    while let Ok(pedido) = pedidos.recv() {
        // Pedido velho é largado sem processar: durante um arrasto a fila enche,
        // e o que interessa é sempre o último.
        if pedido.id < *id_atual.lock() {
            continue;
        }

        let comeco = std::time::Instant::now();
        if let Some(imagem) = motor.revelar(
            &pedido.pixels,
            pedido.largura,
            pedido.altura,
            &pedido.ajustes,
        ) {
            let _ = resultados.send(Resultado {
                id: pedido.id,
                imagem,
                duracao_ms: comeco.elapsed().as_secs_f32() * 1000.0,
            });
        }
    }
}
