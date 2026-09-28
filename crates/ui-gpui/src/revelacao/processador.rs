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
pub use infrastructure::transformacao::Corte;

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
    /// O enquadramento **da tela** — o do modo de corte, se ele estiver aberto.
    ///
    /// 🔑 Os pixels continuam saindo inteiros (quem recorta é `refazer_exibicao`,
    /// depois), mas as duas vinhetas são medidas no recorte: sem o corte, a foto
    /// recortada mostrava a vinheta da foto inteira.
    pub corte: Corte,
    /// A Revelação local: máscaras e retoques, refeitos pelo motor na
    /// resolução desta revelação.
    pub locais: Arc<infrastructure::gpu_adjustments::ReceitaLocal>,
    /// O maior lado, em pixels do dispositivo, da área onde a foto vai
    /// aparecer. `None` revela no tamanho que chegou.
    ///
    /// 🔑 **Revelar mais pixels do que a tela mostra é pagar três vezes por
    /// nada**: na GPU, na volta para a CPU e na subida para a janela. Durante o
    /// arrasto de um slider a Revelação pede no tamanho do palco, e a tela do
    /// cliente sempre no tamanho do monitor — ver [`tamanho_reduzido`].
    pub lado_na_tela: Option<u32>,
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
    /// Qual API gráfica respondeu — o selo da barra, como no site.
    backend: Arc<Mutex<Option<&'static str>>>,
    /// Quem é a GPU do motor — para a ferramenta de desempenho.
    info: Arc<Mutex<Option<revelacao_core::InfoDoAdaptador>>>,
}

impl Processador {
    pub fn novo() -> Self {
        let (envia_pedido, recebe_pedido) = channel::<Pedido>();
        let (envia_resultado, recebe_resultado) = channel::<Resultado>();
        let id_atual = Arc::new(Mutex::new(0u64));
        let disponivel = Arc::new(Mutex::new(None));
        let backend = Arc::new(Mutex::new(None));
        let info = Arc::new(Mutex::new(None));

        {
            let id_atual = id_atual.clone();
            let disponivel = disponivel.clone();
            let backend = backend.clone();
            let info = info.clone();
            std::thread::spawn(move || {
                laco(
                    recebe_pedido,
                    envia_resultado,
                    id_atual,
                    disponivel,
                    backend,
                    info,
                );
            });
        }

        Self {
            pedidos: envia_pedido,
            resultados: recebe_resultado,
            id_atual,
            disponivel,
            backend,
            info,
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

    /// Qual API gráfica respondeu. `None` enquanto a thread abre o dispositivo.
    pub fn backend(&self) -> Option<&'static str> {
        *self.backend.lock()
    }

    /// A GPU do motor, com driver e suporte a timestamp. `None` enquanto abre.
    pub fn info(&self) -> Option<revelacao_core::InfoDoAdaptador> {
        self.info.lock().clone()
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
    backend: Arc<Mutex<Option<&'static str>>>,
    info: Arc<Mutex<Option<revelacao_core::InfoDoAdaptador>>>,
) {
    let Some(mut motor) = Motor::abrir() else {
        *disponivel.lock() = Some(false);
        return;
    };
    *backend.lock() = Some(motor.backend());
    *info.lock() = Some(motor.info().clone());
    *disponivel.lock() = Some(true);
    eprintln!(
        "[Revelação] motor aberto: {} · máscaras {}",
        motor.backend(),
        if motor.mascaras_suportadas() {
            "suportadas"
        } else {
            "SEM SUPORTE"
        }
    );

    let mut reduzida: Option<Reduzida> = None;
    while let Ok(pedido) = pedidos.recv() {
        // Pedido velho é largado sem processar: durante um arrasto a fila enche,
        // e o que interessa é sempre o último.
        if pedido.id < *id_atual.lock() {
            continue;
        }

        let comeco = std::time::Instant::now();
        // ⏱️ A ferramenta de desempenho: carimbar as passadas só enquanto ela
        // grava (desligada, a revelação é a de sempre).
        let medindo = crate::desempenho::ativa();
        motor.definir_medicao(medindo);
        let (pixels, largura, altura) = match pedido
            .lado_na_tela
            .and_then(|lado| tamanho_reduzido(pedido.largura, pedido.altura, &pedido.corte, lado))
        {
            Some((largura, altura)) => (
                reduzir(
                    &mut reduzida,
                    &pedido.pixels,
                    (pedido.largura, pedido.altura),
                    (largura, altura),
                ),
                largura,
                altura,
            ),
            None => (pedido.pixels.clone(), pedido.largura, pedido.altura),
        };
        let reducao = comeco.elapsed();
        // Os módulos locais medem em pixels da foto: na cópia reduzida o raio
        // encolhe junto, e o rascunho mostra o mesmo efeito que a cópia inteira.
        motor.definir_escala_do_original(largura as f32 / pedido.largura.max(1) as f32);
        motor.definir_corte(&pedido.corte);
        // Sem suporte a máscara na GPU a revelação sai sem elas — a tela
        // avisa (`Processador::mascaras_suportadas`), e a exportação falha.
        if let Err(erro) = motor.definir_locais(&pedido.locais) {
            crate::telemetria::avisar!("⚠️ [Revelação] a receita local ficou de fora: {erro:?}");
        }
        if let Some(imagem) = motor.revelar(&pixels, largura, altura, &pedido.ajustes) {
            if medindo {
                crate::desempenho::revelacao_do_motor(
                    crate::desempenho::RevelacaoNoMotor {
                        em_us: 0,
                        operacao: crate::desempenho::Operacao::Nenhuma,
                        largura,
                        altura,
                        rascunho: pedido.lado_na_tela.is_some() && largura < pedido.largura,
                        reducao_ms: reducao.as_secs_f32() * 1000.0,
                        tempos: motor.ultimos_tempos(),
                        total_ms: comeco.elapsed().as_secs_f32() * 1000.0,
                    },
                    Some(motor.info()),
                );
            }
            let _ = resultados.send(Resultado {
                id: pedido.id,
                imagem,
                duracao_ms: comeco.elapsed().as_secs_f32() * 1000.0,
            });
        }
    }
}

/// A última cópia reduzida, e de quais pixels ela saiu.
///
/// 🔑 **Reduz uma vez por foto, e não por quadro.** Durante um arrasto chegam
/// dezenas de pedidos com o mesmo `Arc`; guardar a cópia também mantém a
/// identidade dela, que é o que evita o motor subir a textura de novo.
struct Reduzida {
    de: Arc<Vec<u8>>,
    tamanho: (u32, u32),
    pixels: Arc<Vec<u8>>,
}

fn reduzir(
    guardada: &mut Option<Reduzida>,
    pixels: &Arc<Vec<u8>>,
    (largura, altura): (u32, u32),
    tamanho: (u32, u32),
) -> Arc<Vec<u8>> {
    if let Some(r) = guardada.as_ref() {
        if Arc::ptr_eq(&r.de, pixels) && r.tamanho == tamanho {
            return r.pixels.clone();
        }
    }
    let original =
        image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(largura, altura, &pixels[..]);
    let Some(original) = original else {
        return pixels.clone();
    };
    let copia = Arc::new(image::imageops::thumbnail(&original, tamanho.0, tamanho.1).into_raw());
    *guardada = Some(Reduzida {
        de: pixels.clone(),
        tamanho,
        pixels: copia.clone(),
    });
    copia
}

/// O tamanho da cópia que basta para a tela, ou `None` quando não vale reduzir.
///
/// ⚠️ **O recorte é feito depois da revelação**, então a conta é sobre o
/// pedaço que sobra: um corte que fica com metade da foto precisa do dobro de
/// pixels para encher a mesma tela. O endireitamento amplia a foto mais um
/// pouco, e entra pelo cosseno.
pub fn tamanho_reduzido(largura: u32, altura: u32, corte: &Corte, lado: u32) -> Option<(u32, u32)> {
    let maior = largura.max(altura) as f32;
    if maior < 1.0 || lado == 0 {
        return None;
    }
    let fracao = corte.largura().min(corte.altura()).max(0.01);
    let giro = corte.angulo().to_radians().cos().abs().max(0.5);
    let necessario = lado as f32 / (fracao * giro);
    // Menos de 20% de ganho não paga a redução.
    if necessario >= maior * 0.8 {
        return None;
    }
    let fator = necessario / maior;
    let medir = |lado: u32| ((lado as f32 * fator).round() as u32).max(1);
    Some((medir(largura), medir(altura)))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_copia_cabe_na_tela_e_guarda_a_proporcao() {
        let (l, a) = tamanho_reduzido(2560, 1707, &Corte::inteiro(), 1280).expect("reduz");
        assert_eq!(l, 1280);
        assert!((a as i32 - 854).abs() <= 1, "3:2 continua 3:2: {a}");
    }

    #[test]
    fn tela_do_tamanho_da_foto_nao_reduz() {
        assert_eq!(tamanho_reduzido(2560, 1707, &Corte::inteiro(), 2400), None);
        assert_eq!(tamanho_reduzido(2560, 1707, &Corte::inteiro(), 0), None);
    }

    /// Metade da foto recortada precisa do dobro de pixels na mesma tela.
    #[test]
    fn o_recorte_pede_mais_pixels() {
        let metade = Corte::novo(0.25, 0.25, 0.5, 0.5, 0, 0.0, false, false);
        let (l, _) = tamanho_reduzido(4000, 3000, &metade, 1000).expect("reduz");
        assert_eq!(l, 2000);
        let apertado = Corte::novo(0.4, 0.4, 0.2, 0.2, 0, 0.0, false, false);
        assert_eq!(tamanho_reduzido(4000, 3000, &apertado, 1000), None);
    }

    /// A cópia sai uma vez por foto: o mesmo `Arc` volta enquanto dura o arrasto.
    #[test]
    fn a_reducao_e_feita_uma_vez_por_foto() {
        let pixels = Arc::new(vec![128u8; 40 * 20 * 4]);
        let mut guardada = None;
        let primeira = reduzir(&mut guardada, &pixels, (40, 20), (20, 10));
        assert_eq!(primeira.len(), 20 * 10 * 4);
        let segunda = reduzir(&mut guardada, &pixels, (40, 20), (20, 10));
        assert!(Arc::ptr_eq(&primeira, &segunda));
        let outra = Arc::new(vec![0u8; 40 * 20 * 4]);
        let terceira = reduzir(&mut guardada, &outra, (40, 20), (20, 10));
        assert!(!Arc::ptr_eq(&primeira, &terceira), "foto nova, cópia nova");
    }
}
