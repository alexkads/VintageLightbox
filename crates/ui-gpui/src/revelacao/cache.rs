//! O cache de fotos reveladas, e a chave que diz quando ele serve.
//!
//! # Por que existe
//!
//! *"Essa tela de revelação precisa guardar um cache e fazer uma aplicação
//! antecipada na próxima foto pra garantir mais velocidade"* (dono,
//! 17/set/2026). Desde que o palco passou a esperar o motor para desenhar
//! (`tela.rs`, `mostrar`), toda foto com receita custa uma ida à GPU antes de
//! aparecer — e voltar uma seta custava a mesma ida de novo, para produzir
//! exatamente a mesma imagem.
//!
//! São duas coisas, e só a primeira mora aqui:
//!
//! 1. **guardar o que já foi revelado**, para a volta ser instantânea;
//! 2. **revelar a próxima antes de a seta chegar nela** (`tela.rs`,
//!    `antecipar_a_proxima`), que é o que faz a ida sumir também na primeira vez.
//!
//! # 🔑 A chave é a foto **mais** a receita
//!
//! Guardar por `foto.id` sozinho seria pior que não guardar: mexer num slider e
//! voltar à foto traria a revelação de antes do gesto, sem erro nenhum e sem
//! pista — o mesmo desfecho do cache de prévia servindo a foto com marca d'água
//! ao balcão. A receita entra inteira ([`Ajustes::como_vetor`] e os oito campos
//! do corte), pelos **bits** dos `f32`: comparar `f32` por igualdade é o que se
//! quer aqui, porque a pergunta não é "são parecidos?", é "é a mesma receita?".
//!
//! ⚠️ **O corte entra na chave** porque entra no pedido: as duas vinhetas são
//! medidas no recorte (`Motor::definir_corte`), então mudar o enquadramento muda
//! o pixel que o shader devolve.

use image::DynamicImage;
use lru::LruCache;

use super::processador::{Ajustes, Corte};

/// O teto do cache, **em pixels** — e não em número de revelações.
///
/// 🚨 **Contar entradas aqui seria contar coisas de tamanhos muito diferentes.**
/// O `PreviewManager` entrega prévia de até 2560 px: uma revelação dessas são
/// 4,4 MP, ~17 MB em RGBA. Trinta e duas delas seriam meio gigabyte parado — e
/// numa máquina cuja prévia fosse pequena, trinta e duas seriam pouco demais. O
/// mesmo teto responde pelos dois casos.
///
/// 48 MP ≈ 190 MB: cabem onze prévias de 2560 px, ou a sessão inteira de 30
/// fotos se a prévia for menor. É a ordem de grandeza do LRU de prévias ao lado
/// (quinze de 2560 px, ~400 MB no pior caso), e não a dobra.
const TETO_DE_PIXELS: u64 = 48_000_000;

/// 🚨 **Acima disto não guarda, nem desconta do teto.** A troca para a
/// resolução cheia (`resolucao.rs`) revela o bruto de 24 MP: uma só ocuparia
/// metade do teto e despejaria a sessão inteira para ser usada uma vez. O que
/// o cache existe para poupar é a ida à GPU da foto de tela, que é a que o
/// operador atravessa com a seta; a resolução cheia é um pedido por vez,
/// deliberado, e não se repete a cada passo.
///
/// 12 MP separa os dois sem ambiguidade: a prévia de 2560 px tem 4,4 e o bruto
/// de uma câmera atual tem 24 ou mais.
const MAIOR_PARA_GUARDAR: u64 = 12_000_000;

/// Quantos pixels uma revelação ocupa.
fn custo(imagem: &DynamicImage) -> u64 {
    u64::from(imagem.width()) * u64::from(imagem.height())
}

/// A identidade de uma revelação: a foto e a receita com que ela foi feita.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chave {
    foto: String,
    /// 🚨 **O tamanho da origem entra na chave.** A mesma foto é revelada a
    /// partir de duas imagens diferentes sob o mesmo id: a cópia de trabalho e,
    /// no zoom, o bruto em resolução cheia (`resolucao.rs`). Sem isto, voltar do
    /// zoom devolveria a revelação da cópia com a cara da resolução cheia — e é
    /// pelo tamanho que as duas se distinguem sem carregar pixel nenhum.
    origem: (u32, u32),
    /// Os `f32` da receita e do corte pelos **bits**. Ver o cabeçalho.
    receita: Box<[u32]>,
}

impl Chave {
    pub fn nova(foto_id: &str, origem: (u32, u32), ajustes: &Ajustes, corte: &Corte) -> Self {
        let mut receita: Vec<u32> = ajustes.como_vetor().iter().map(|v| v.to_bits()).collect();
        receita.extend(
            [
                corte.x(),
                corte.y(),
                corte.largura(),
                corte.altura(),
                corte.angulo(),
            ]
            .iter()
            .map(|v| v.to_bits()),
        );
        receita.push(corte.giro_90() as u32);
        receita.push(u32::from(corte.espelho_h()));
        receita.push(u32::from(corte.espelho_v()));
        Self {
            foto: foto_id.to_string(),
            origem,
            receita: receita.into_boxed_slice(),
        }
    }

    pub fn e_da_foto(&self, foto_id: &str) -> bool {
        self.foto == foto_id
    }
}

/// As revelações guardadas, as mais recentes por último a sair.
pub struct CacheDeReveladas {
    guardadas: LruCache<Chave, DynamicImage>,
    /// Quantos pixels há guardados agora. Somado e subtraído a cada entrada e
    /// saída, para o despejo não ter de varrer o cache inteiro.
    pixels: u64,
    teto: u64,
}

impl Default for CacheDeReveladas {
    fn default() -> Self {
        Self::nova(TETO_DE_PIXELS)
    }
}

impl CacheDeReveladas {
    /// `teto` é em **pixels** — ver [`TETO_DE_PIXELS`].
    pub fn nova(teto: u64) -> Self {
        Self {
            guardadas: LruCache::unbounded(),
            pixels: 0,
            teto,
        }
    }

    /// A revelação desta foto com esta receita, se estiver guardada.
    ///
    /// Clona: o palco precisa da imagem e o cache precisa continuar com ela.
    /// É uma cópia de prévia, não da foto inteira.
    pub fn buscar(&mut self, chave: &Chave) -> Option<DynamicImage> {
        self.guardadas.get(chave).cloned()
    }

    /// Guarda, despejando as mais velhas até caber no teto. Devolve se guardou.
    pub fn guardar(&mut self, chave: Chave, imagem: &DynamicImage) -> bool {
        if custo(imagem) > MAIOR_PARA_GUARDAR {
            return false;
        }
        if let Some(velha) = self.guardadas.put(chave, imagem.clone()) {
            self.pixels = self.pixels.saturating_sub(custo(&velha));
        }
        self.pixels += custo(imagem);

        // ⚠️ **Nunca despeja a última.** Uma revelação sozinha maior que o teto
        // ainda é a foto que o operador está vendo; jogá-la fora seria guardar
        // para esquecer no mesmo instante.
        while self.pixels > self.teto && self.guardadas.len() > 1 {
            match self.guardadas.pop_lru() {
                Some((_, saiu)) => self.pixels = self.pixels.saturating_sub(custo(&saiu)),
                None => break,
            }
        }
        true
    }

    /// Esquece tudo o que é desta foto.
    ///
    /// 🚨 **Quem grava uma receita nova não invalida nada** — a chave já leva a
    /// receita, e a revelação antiga simplesmente deixa de ser procurada. Isto
    /// aqui é para quando os **pixels de origem** mudam sob o mesmo id: a cópia
    /// de trabalho que chega do site e a troca para a resolução cheia. Aí a
    /// mesma chave passaria a mentir, e mentir com a cara certa.
    pub fn esquecer(&mut self, foto_id: &str) {
        let alvos: Vec<Chave> = self
            .guardadas
            .iter()
            .map(|(chave, _)| chave)
            .filter(|chave| chave.e_da_foto(foto_id))
            .cloned()
            .collect();
        for chave in alvos {
            if let Some(saiu) = self.guardadas.pop(&chave) {
                self.pixels = self.pixels.saturating_sub(custo(&saiu));
            }
        }
    }

    #[cfg(test)]
    pub fn quantas(&self) -> usize {
        self.guardadas.len()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use image::{DynamicImage, RgbImage};

    /// O tamanho da origem, igual em todos os casos que não são sobre ele.
    const ORIGEM: (u32, u32) = (640, 427);

    fn corte() -> Corte {
        Corte::novo(0.0, 0.0, 1.0, 1.0, 0, 0.0, false, false)
    }

    fn imagem(largura: u32, altura: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::new(largura, altura))
    }

    /// 🚨 A receita faz parte da chave — senão o cache devolve a revelação de
    /// antes do gesto, com a cara de certa.
    #[test]
    fn a_mesma_foto_com_receitas_diferentes_tem_chaves_diferentes() {
        let mut ajustes = Ajustes::default();
        let neutra = Chave::nova("id-1", ORIGEM, &ajustes, &corte());
        ajustes.exposure = 1.5;
        let mexida = Chave::nova("id-1", ORIGEM, &ajustes, &corte());

        assert_ne!(neutra, mexida);
        assert_eq!(
            neutra,
            Chave::nova("id-1", ORIGEM, &Ajustes::default(), &corte())
        );
    }

    /// ⚠️ O corte também: as vinhetas são medidas no recorte, então enquadrar
    /// muda o pixel que o shader devolve.
    #[test]
    fn o_corte_entra_na_chave() {
        let ajustes = Ajustes::default();
        let inteira = Chave::nova("id-1", ORIGEM, &ajustes, &corte());
        let recortada = Chave::nova(
            "id-1",
            ORIGEM,
            &ajustes,
            &Corte::novo(0.1, 0.1, 0.5, 0.5, 0, 0.0, false, false),
        );
        let girada = Chave::nova(
            "id-1",
            ORIGEM,
            &ajustes,
            &Corte::novo(0.0, 0.0, 1.0, 1.0, 1, 0.0, false, false),
        );
        let espelhada = Chave::nova(
            "id-1",
            ORIGEM,
            &ajustes,
            &Corte::novo(0.0, 0.0, 1.0, 1.0, 0, 0.0, true, false),
        );

        assert_ne!(inteira, recortada);
        assert_ne!(inteira, girada);
        assert_ne!(inteira, espelhada);
    }

    /// 🚨 A cópia de trabalho e o bruto em resolução cheia são a mesma foto com
    /// a mesma receita — e não podem compartilhar a chave.
    #[test]
    fn o_tamanho_da_origem_entra_na_chave() {
        let ajustes = Ajustes::default();
        assert_ne!(
            Chave::nova("id-1", (2048, 1365), &ajustes, &corte()),
            Chave::nova("id-1", (6000, 4000), &ajustes, &corte())
        );
    }

    /// Duas fotos com a mesma receita não se confundem.
    #[test]
    fn a_foto_faz_parte_da_chave() {
        let ajustes = Ajustes::default();
        assert_ne!(
            Chave::nova("id-1", ORIGEM, &ajustes, &corte()),
            Chave::nova("id-2", ORIGEM, &ajustes, &corte())
        );
    }

    #[test]
    fn guarda_e_devolve_a_mesma_revelacao() {
        let mut cache = CacheDeReveladas::default();
        let chave = Chave::nova("id-1", ORIGEM, &Ajustes::default(), &corte());

        assert!(cache.buscar(&chave).is_none(), "nasce vazio");
        assert!(cache.guardar(chave.clone(), &imagem(64, 48)));

        let guardada = cache.buscar(&chave).expect("guardada");
        assert_eq!((guardada.width(), guardada.height()), (64, 48));
        assert!(
            cache.buscar(&chave).is_some(),
            "buscar não consome — o palco leva uma cópia e o cache fica com a dele"
        );
    }

    /// 🚨 A resolução cheia não entra: uma só ocuparia metade do teto e
    /// despejaria a sessão inteira para ser usada uma vez.
    #[test]
    fn a_revelacao_grande_demais_nao_e_guardada() {
        let mut cache = CacheDeReveladas::default();
        let chave = Chave::nova("id-1", (6000, 4000), &Ajustes::default(), &corte());

        assert!(
            !cache.guardar(chave.clone(), &imagem(6000, 4000)),
            "24 MP passa do que se guarda"
        );
        assert!(cache.buscar(&chave).is_none());
        assert_eq!(cache.quantas(), 0);
    }

    /// ⚠️ A prévia de 2560 px **entra** — é a que o palco usa, e o caso de
    /// sempre. O teto anterior contava entradas e recusava justamente esta.
    #[test]
    fn a_previa_de_2560_cabe() {
        let mut cache = CacheDeReveladas::default();
        let chave = Chave::nova("id-1", (2560, 1707), &Ajustes::default(), &corte());

        assert!(cache.guardar(chave.clone(), &imagem(2560, 1707)));
        assert!(cache.buscar(&chave).is_some());
    }

    /// ⚠️ Os pixels de origem podem mudar sob o mesmo id — e aí a chave mente.
    #[test]
    fn esquecer_tira_tudo_o_que_e_daquela_foto() {
        let mut cache = CacheDeReveladas::default();
        let mut ajustes = Ajustes::default();
        let uma = Chave::nova("id-1", ORIGEM, &ajustes, &corte());
        ajustes.exposure = 1.0;
        let outra_receita = Chave::nova("id-1", ORIGEM, &ajustes, &corte());
        let outra_foto = Chave::nova("id-2", ORIGEM, &ajustes, &corte());

        cache.guardar(uma.clone(), &imagem(8, 8));
        cache.guardar(outra_receita.clone(), &imagem(8, 8));
        cache.guardar(outra_foto.clone(), &imagem(8, 8));

        cache.esquecer("id-1");

        assert!(cache.buscar(&uma).is_none());
        assert!(
            cache.buscar(&outra_receita).is_none(),
            "as duas receitas da mesma foto saem juntas"
        );
        assert!(cache.buscar(&outra_foto).is_some(), "a outra foto fica");
    }

    /// O mais velho sai quando o teto estoura — e o que acabou de ser usado
    /// fica.
    #[test]
    fn o_mais_velho_sai_primeiro() {
        // Teto de duas imagens de 8x8.
        let mut cache = CacheDeReveladas::nova(8 * 8 * 2);
        let a = Chave::nova("id-a", ORIGEM, &Ajustes::default(), &corte());
        let b = Chave::nova("id-b", ORIGEM, &Ajustes::default(), &corte());
        let c = Chave::nova("id-c", ORIGEM, &Ajustes::default(), &corte());

        cache.guardar(a.clone(), &imagem(8, 8));
        cache.guardar(b.clone(), &imagem(8, 8));
        // Tocar em `a` a torna a mais recente: quem sai é `b`.
        assert!(cache.buscar(&a).is_some());
        cache.guardar(c.clone(), &imagem(8, 8));

        assert!(cache.buscar(&a).is_some());
        assert!(cache.buscar(&c).is_some());
        assert!(cache.buscar(&b).is_none());
    }
}
