//! As miniaturas da grade do assistente, lidas **fora da linha da interface**.
//!
//! # Por que existe (dono, 20/set/2026)
//!
//! *"Não consigo digitar o título como se tivesse um bug no input"* — faltando
//! letras, com a bandeja marcando 54 → 55 → 60 fotos esperando nota. Não havia
//! bug no `Input`: `preparar_miniaturas` rodava **a cada desenho**, na linha da
//! interface, e lia do cache toda foto que ainda não tivesse miniatura em
//! memória. Com uma importação correndo, cada releitura do catálogo traz um
//! lote de fotos novas, e o desenho seguinte decodificava o lote inteiro de uma
//! vez.
//!
//! O preço, medido em `tests/custo_do_quadro_da_nova.rs`:
//!
//! | Parte | Por foto | Leva de 10 |
//! |---|---|---|
//! | Ler do cache e redimensionar | **7,5 ms** | **75 ms** |
//! | `para_gpui` (troca BGRA) | 64 µs | 0,6 ms |
//!
//! 94 ms de linha parada são seis quadros a 60 Hz. A tecla que chega nesse
//! intervalo não é perdida pelo campo — ela é entregue tarde demais para o
//! quadro em que o operador a digitou.
//!
//! 🔑 **Só a leitura muda de linha.** `para_gpui` custa 64 µs e continua onde
//! está, como nas [`super::amostras`]: o que atravessa o canal é a
//! `DynamicImage`, e `RenderImage` nasce na interface, que é de quem ele é.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use gpui::RenderImage;
use image::DynamicImage;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::imagem::para_gpui;
use crate::revelacao::persistencia;

/// O lado da miniatura da grade, quando é preciso reduzir a prévia grande.
const LADO: u32 = 320;

struct Pedido {
    id: String,
    previews: Arc<PreviewManager>,
}

type Respostas = (
    Sender<(String, Option<DynamicImage>)>,
    Receiver<(String, Option<DynamicImage>)>,
);

/// As miniaturas prontas, as que estão a caminho, e a thread que as lê.
pub struct Miniaturas {
    prontas: HashMap<String, Arc<RenderImage>>,
    /// Pedidas e ainda não respondidas — inclusive as que voltaram sem imagem,
    /// para não pedir de novo a cada quadro uma foto que ainda não tem prévia.
    pedidas: HashSet<String>,
    /// As que o leitor respondeu "não tenho": ficam de fora até alguém
    /// esquecê-las (a prévia chega depois, e quem avisa é a releitura).
    vazias: HashSet<String>,
    pedidos: Option<Sender<Pedido>>,
    respostas: Respostas,
}

impl Default for Miniaturas {
    fn default() -> Self {
        Self {
            prontas: HashMap::new(),
            pedidas: HashSet::new(),
            vazias: HashSet::new(),
            pedidos: None,
            respostas: channel(),
        }
    }
}

impl Miniaturas {
    /// A miniatura desta foto, se já chegou.
    pub fn obter(&self, id: &str) -> Option<&Arc<RenderImage>> {
        self.prontas.get(id)
    }

    pub fn tem(&self, id: &str) -> bool {
        self.prontas.contains_key(id)
    }

    /// Pede ao leitor o que falta. **Não lê nada aqui** — só despacha.
    pub fn pedir(&mut self, ids: impl IntoIterator<Item = String>, previews: &Arc<PreviewManager>) {
        for id in ids {
            if self.prontas.contains_key(&id) || self.vazias.contains(&id) {
                continue;
            }
            if !self.pedidas.insert(id.clone()) {
                continue;
            }
            let pedido = Pedido {
                id,
                previews: previews.clone(),
            };
            let _ = self.leitor().send(pedido);
        }
    }

    /// Se ainda há miniatura a caminho — a tela continua acordando por elas.
    pub fn esperando(&self) -> bool {
        self.pedidas.len() > self.prontas.len() + self.vazias.len()
    }

    /// Recolhe o que o leitor terminou. Devolve se chegou alguma.
    ///
    /// 🔑 É aqui que `para_gpui` roda, na linha da interface: 64 µs por foto,
    /// contra os 7,5 ms da leitura que ficou na thread.
    pub fn colher(&mut self) -> bool {
        let mut chegou = false;
        while let Ok((id, imagem)) = self.respostas.1.try_recv() {
            match imagem {
                Some(imagem) => {
                    self.prontas.insert(id, para_gpui(imagem));
                }
                None => {
                    self.vazias.insert(id);
                }
            }
            chegou = true;
        }
        chegou
    }

    /// Esquece esta foto: a próxima passada pede de novo.
    ///
    /// Serve à revelação que acabou de gravar uma versão nova (C17 do Contrato
    /// da Foto: o cache derivado sai e é refeito) e à foto que ainda não tinha
    /// prévia quando foi pedida.
    pub fn esquecer(&mut self, id: &str) {
        self.prontas.remove(id);
        self.pedidas.remove(id);
        self.vazias.remove(id);
    }

    /// Esquece as que voltaram sem imagem: a releitura do catálogo diz que o
    /// trabalhador gravou prévias novas desde a última passada.
    pub fn esquecer_as_vazias(&mut self) {
        for id in self.vazias.drain() {
            self.pedidas.remove(&id);
        }
    }

    fn leitor(&mut self) -> &Sender<Pedido> {
        self.pedidos.get_or_insert_with(|| {
            let (envia, recebe) = channel::<Pedido>();
            let respostas = self.respostas.0.clone();
            std::thread::Builder::new()
                .name("miniaturas-da-nova".into())
                .spawn(move || laco(recebe, respostas))
                .expect("abrir a thread das miniaturas");
            envia
        })
    }
}

/// 🚨 **A ordem de leitura é a mesma de antes**: a versão revelada primeiro, o
/// bruto depois — a grade mostra a foto com a receita, e não como ela veio.
fn laco(pedidos: Receiver<Pedido>, respostas: Sender<(String, Option<DynamicImage>)>) {
    while let Ok(pedido) = pedidos.recv() {
        let previews = &pedido.previews;
        let revelada = persistencia::chave_da_revelada(&pedido.id);
        let imagem = previews
            .get_thumbnail(&revelada)
            .or_else(|| previews.get_thumbnail(&pedido.id))
            .or_else(|| {
                previews
                    .get_preview(&pedido.id)
                    .map(|g| g.thumbnail(LADO, LADO))
            });
        if respostas.send((pedido.id, imagem)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_previa(ids: &[&str]) -> (Arc<PreviewManager>, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
        let imagem = image::DynamicImage::ImageRgba8(image::RgbaImage::new(64, 48));
        for id in ids {
            previews.save_preview(id, &imagem).expect("gravar a prévia");
        }
        (previews, dir)
    }

    /// Espera o leitor responder — ele é uma thread de verdade.
    fn ate_chegar(m: &mut Miniaturas) {
        for _ in 0..200 {
            m.colher();
            if !m.esperando() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("o leitor das miniaturas não respondeu");
    }

    #[test]
    fn a_foto_com_previa_vira_miniatura() {
        let (previews, _dir) = com_previa(&["a", "b"]);
        let mut m = Miniaturas::default();
        m.pedir(["a".to_string(), "b".to_string()], &previews);
        ate_chegar(&mut m);
        assert!(m.tem("a") && m.tem("b"), "as duas tinham prévia no cache");
        assert!(m.obter("a").is_some());
    }

    /// 🚨 A foto pedida antes de o trabalhador gravar a prévia volta vazia — e
    /// **não** é pedida de novo a cada quadro. Quem a libera é a releitura.
    #[test]
    fn a_foto_sem_previa_nao_e_repedida_a_cada_quadro() {
        let (previews, _dir) = com_previa(&["a"]);
        let mut m = Miniaturas::default();
        m.pedir(["a".to_string(), "ainda-nao".to_string()], &previews);
        ate_chegar(&mut m);
        assert!(m.tem("a"));
        assert!(!m.tem("ainda-nao"));

        // Um quadro depois, sem novidade: nada volta para a fila.
        m.pedir(["ainda-nao".to_string()], &previews);
        assert!(!m.esperando(), "a vazia não podia ser pedida de novo");

        // A prévia chega, e a releitura avisa: agora vale pedir.
        previews
            .save_preview(
                "ainda-nao",
                &image::DynamicImage::ImageRgba8(image::RgbaImage::new(64, 48)),
            )
            .expect("gravar a prévia");
        m.esquecer_as_vazias();
        m.pedir(["ainda-nao".to_string()], &previews);
        ate_chegar(&mut m);
        assert!(
            m.tem("ainda-nao"),
            "a prévia chegou: a miniatura tinha de vir"
        );
    }

    /// C17 do Contrato da Foto: a versão revelada troca o cache derivado.
    #[test]
    fn esquecer_faz_a_proxima_passada_ler_de_novo() {
        let (previews, _dir) = com_previa(&["a"]);
        let mut m = Miniaturas::default();
        m.pedir(["a".to_string()], &previews);
        ate_chegar(&mut m);
        assert!(m.tem("a"));

        m.esquecer("a");
        assert!(!m.tem("a"));
        m.pedir(["a".to_string()], &previews);
        ate_chegar(&mut m);
        assert!(m.tem("a"), "depois de esquecida, ela é lida de novo");
    }
}
