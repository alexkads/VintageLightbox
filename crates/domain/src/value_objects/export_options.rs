//! O que decidir na hora de exportar: tamanho, qualidade e marca d'água.
//!
//! ## 🔑 Por que a marca d'água mora no domínio, e não na tela
//!
//! Ela não é enfeite: é a regra de negócio que separa **entregar** de
//! **mostrar**. No fluxo do estúdio, a foto que o cliente comprou vai inteira, e
//! a que ficou para trás vai marcada — é o que permite exibi-la na galeria sem
//! entregá-la. Sem marca d'água não existe upsell; existe distribuição.
//!
//! ⚠️ **E é por isso que os dois casos são o mesmo tipo com valores diferentes**,
//! e não dois caminhos de código: "entrega final" e "prévia para a galeria" têm
//! de sair pelo mesmo exportador, senão a diferença entre eles vira um `if` em
//! algum lugar que ninguém revisa.

use crate::value_objects::FilePath;

/// Onde a marca d'água fica sobre a foto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatermarkPosition {
    /// O padrão, e é decisão: no canto ela é recortável, e a foto marcada de
    /// uma galeria é justamente a que alguém tem motivo para recortar.
    #[default]
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Uma marca d'água de imagem — o logotipo do estúdio, com transparência.
///
/// 🔑 **É imagem, e não texto, de propósito.** Texto exigiria fonte embutida,
/// medição de glifo e uma decisão de tipografia por exportação; um PNG com alfa
/// é o que o estúdio já tem em disco e o que ele quer ver na galeria.
#[derive(Debug, Clone, PartialEq)]
pub struct Watermark {
    file: FilePath,
    position: WatermarkPosition,
    /// Largura da marca como fração da largura da foto, entre 1% e 100%.
    scale: f32,
    /// 0.0 é invisível, 1.0 é opaca.
    opacity: f32,
    /// Distância da borda, como fração da menor dimensão da foto.
    margin: f32,
}

impl Watermark {
    const MIN_SCALE: f32 = 0.01;
    const MAX_SCALE: f32 = 1.0;

    pub fn new(file: FilePath, position: WatermarkPosition, scale: f32, opacity: f32) -> Self {
        Self {
            file,
            position,
            scale: scale.clamp(Self::MIN_SCALE, Self::MAX_SCALE),
            opacity: opacity.clamp(0.0, 1.0),
            margin: 0.03,
        }
    }

    pub fn file(&self) -> &FilePath {
        &self.file
    }
    pub fn position(&self) -> WatermarkPosition {
        self.position
    }
    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn opacity(&self) -> f32 {
        self.opacity
    }
    pub fn margin(&self) -> f32 {
        self.margin
    }
}

/// As decisões de uma exportação.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    /// Qualidade JPEG, de 1 a 100.
    quality: u8,
    /// Limite do lado maior, em pixels. `None` mantém o tamanho original.
    longest_edge: Option<u32>,
    watermark: Option<Watermark>,
}

impl Default for ExportOptions {
    /// O padrão é **a entrega final**: tamanho original, sem marca.
    ///
    /// ⚠️ **É o padrão certo porque o errado é irreversível na direção errada.**
    /// Exportar sem marca o que devia ir marcado entrega a foto que não foi
    /// comprada; exportar marcado o que devia ir limpo é um reexport. Só que o
    /// padrão seguro nesse raciocínio seria *marcar sempre* — e aí toda entrega
    /// de cliente sairia com logotipo, o que ninguém deixaria passar. O padrão é
    /// o caso comum, e quem exporta prévia **escolhe**.
    fn default() -> Self {
        Self {
            quality: 90,
            longest_edge: None,
            watermark: None,
        }
    }
}

impl ExportOptions {
    pub fn quality(&self) -> u8 {
        self.quality
    }
    pub fn longest_edge(&self) -> Option<u32> {
        self.longest_edge
    }
    pub fn watermark(&self) -> Option<&Watermark> {
        self.watermark.as_ref()
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(1, 100);
        self
    }

    /// Limita o lado maior. ⚠️ **Nunca amplia** — ver
    /// [`Self::longest_edge`] e o exportador.
    pub fn with_longest_edge(mut self, pixels: u32) -> Self {
        self.longest_edge = Some(pixels.max(1));
        self
    }

    pub fn with_watermark(mut self, watermark: Watermark) -> Self {
        self.watermark = Some(watermark);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arquivo() -> FilePath {
        FilePath::new("/logos/marca.png").unwrap()
    }

    /// O padrão é a entrega final: original, sem marca.
    #[test]
    fn o_padrao_e_a_entrega_final() {
        let opcoes = ExportOptions::default();
        assert_eq!(opcoes.quality(), 90);
        assert_eq!(opcoes.longest_edge(), None);
        assert!(opcoes.watermark().is_none());
    }

    /// ⚠️ **Qualidade 0 não é uma qualidade.** O encoder JPEG a aceita e devolve
    /// uma imagem que não serve para nada; um lote inteiro sairia ilegível sem
    /// erro nenhum.
    #[test]
    fn a_qualidade_e_limitada_a_faixa_util() {
        assert_eq!(ExportOptions::default().with_quality(0).quality(), 1);
        assert_eq!(ExportOptions::default().with_quality(255).quality(), 100);
    }

    /// ⚠️ **Lado maior zero apagaria a foto.** `resize(0, 0)` devolve uma imagem
    /// vazia, e o arquivo gravado teria zero pixel — sem erro em lugar nenhum.
    #[test]
    fn o_lado_maior_nunca_e_zero() {
        assert_eq!(
            ExportOptions::default().with_longest_edge(0).longest_edge(),
            Some(1)
        );
    }

    /// 🚨 **Escala zero some com a marca; opacidade fora da faixa a inverte.**
    ///
    /// Os dois são o mesmo tipo de defeito e o pior possível aqui: a exportação
    /// termina, o arquivo existe, o rodapé diz "40 exportadas" — e as fotos que
    /// deviam ir protegidas foram para a galeria **sem proteção**.
    #[test]
    fn a_marca_dagua_recusa_valores_que_a_fariam_sumir() {
        let some = Watermark::new(arquivo(), WatermarkPosition::Center, 0.0, 0.5);
        assert!(
            some.scale() >= 0.01,
            "escala zero deixaria a marca invisível"
        );

        let invertida = Watermark::new(arquivo(), WatermarkPosition::Center, 0.3, -1.0);
        assert_eq!(invertida.opacity(), 0.0);

        let estourada = Watermark::new(arquivo(), WatermarkPosition::Center, 0.3, 5.0);
        assert_eq!(estourada.opacity(), 1.0);
    }

    /// A marca não pode ser maior que a foto.
    #[test]
    fn a_escala_nao_passa_da_foto_inteira() {
        let gigante = Watermark::new(arquivo(), WatermarkPosition::Center, 3.0, 1.0);
        assert_eq!(gigante.scale(), 1.0);
    }

    /// 🔑 O centro é o padrão, e é decisão: no canto a marca é recortável, e a
    /// foto marcada de uma galeria é justamente a que alguém tem motivo para
    /// recortar.
    #[test]
    fn a_posicao_padrao_e_o_centro() {
        assert_eq!(WatermarkPosition::default(), WatermarkPosition::Center);
    }
}
