//! A geometria de uma grade de fotos — funções puras, sem GPU e sem janela.
//!
//! Tudo que o desenho e a interação precisam saber sobre *onde* cada foto está
//! mora aqui: quantas colunas cabem, o tamanho do tile, qual foto está sob o
//! ponteiro, quais estão na faixa visível ou dentro de um retângulo arrastado,
//! para onde o foco vai com uma seta.
//!
//! # Por que é aqui, e não em cada tela
//!
//! Três implementações da mesma conta existiam ao mesmo tempo: a do painel do
//! pós-venda e a da galeria do cliente (`grade-layout.ts`, no site) e a da
//! Biblioteca do desktop (`ui-gpui/.../grade.rs`). É a armadilha nº 8 do projeto
//! na forma mais cara — **duas listas para a mesma decisão** —, e ela já tinha
//! divergido: o desktop não descontava o respiro que a última coluna não usa, e
//! por isso mostrava uma coluna a menos em certas larguras de janela.
//!
//! Agora é uma conta só, e quem desenha só desenha.
//!
//! # Coordenadas
//!
//! **De conteúdo**, com origem no canto superior esquerdo da grade inteira. A
//! rolagem é um deslocamento que quem desenha subtrai na hora de desenhar e soma
//! na hora de interpretar o ponteiro — este módulo nunca vê "tela".

/// O tamanho-alvo do tile, em pixels — o controle de zoom escolhe entre os dois.
pub const ZOOM_MIN: f32 = 110.0;
pub const ZOOM_MAX: f32 = 440.0;
pub const ZOOM_PADRAO: f32 = 220.0;

/// O respiro entre tiles.
pub const ESPACO: f32 = 12.0;
/// Abaixo desta largura o tile é só a imagem — não há espaço para ler nome.
pub const RODAPE_A_PARTIR_DE: f32 = 150.0;
/// O rodapé com duas linhas de texto (nome e faixa).
pub const ALTURA_RODAPE: f32 = 40.0;

/// O que muda entre as duas grades do site (e a do desktop).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opcoes {
    /// Altura da imagem em relação à largura do tile — 3/4 no painel, 1 na
    /// galeria do cliente (tile quadrado).
    pub razao: f32,
    /// Altura fixa do rodapé. `None` = a regra do painel: some em tile estreito.
    pub altura_rodape: Option<f32>,
    pub espaco: f32,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            razao: 3.0 / 4.0,
            altura_rodape: None,
            espaco: ESPACO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Retangulo {
    /// O retângulo entre dois cantos, em qualquer ordem — o arrasto de seleção.
    pub fn entre(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
        }
    }

    pub fn toca(&self, outro: &Retangulo) -> bool {
        self.x < outro.x + outro.w
            && self.x + self.w > outro.x
            && self.y < outro.y + outro.h
            && self.y + self.h > outro.y
    }

    /// Interpolação linear entre dois retângulos — a animação de abrir a foto.
    pub fn interpolar(a: Retangulo, b: Retangulo, t: f32) -> Retangulo {
        let k = t.clamp(0.0, 1.0);
        Retangulo {
            x: a.x + (b.x - a.x) * k,
            y: a.y + (b.y - a.y) * k,
            w: a.w + (b.w - a.w) * k,
            h: a.h + (b.h - a.h) * k,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub largura: f32,
    pub colunas: usize,
    /// A largura de um tile.
    pub lado: f32,
    pub altura_imagem: f32,
    pub altura_rodape: f32,
    pub altura_tile: f32,
    pub espaco: f32,
    pub total: usize,
    /// A altura da grade inteira, para o contêiner que a página rola.
    pub altura_total: f32,
}

/// Para onde o foco vai com uma tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direcao {
    Esquerda,
    Direita,
    Cima,
    Baixo,
    Inicio,
    Fim,
}

impl Layout {
    /// A grade que cabe em `largura`, com tiles do tamanho-alvo `zoom`.
    ///
    /// 🔑 **A última coluna não paga respiro à direita.** É a diferença que
    /// fazia o desktop mostrar uma coluna a menos: em 999 px com tile-alvo de
    /// 188 e respiro de 12, cabem **cinco** (5 × 188 + 4 × 12 = 988), e não
    /// quatro.
    ///
    /// **Nunca devolve zero colunas.** Numa janela mais estreita que uma
    /// miniatura a resposta honesta é "uma por linha, ocupando tudo": zero faria
    /// `total / colunas` dividir por zero e a grade sumir no momento em que
    /// alguém arrasta a borda da janela para a esquerda.
    pub fn calcular(largura: f32, zoom: f32, total: usize, opcoes: Opcoes) -> Layout {
        // `NAN` chega aqui quando o layout ainda não mediu o contêiner — no
        // primeiro quadro do desktop, e entre o `resize` e o efeito no site.
        let l = if largura.is_finite() {
            largura.max(0.0)
        } else {
            0.0
        };
        let espaco = if opcoes.espaco.is_finite() {
            opcoes.espaco.max(0.0)
        } else {
            ESPACO
        };
        let alvo = zoom.clamp(ZOOM_MIN, ZOOM_MAX);

        let colunas = colunas_que_cabem(l, alvo, espaco);
        let lado = if colunas == 1 {
            l
        } else {
            (l - espaco * (colunas - 1) as f32) / colunas as f32
        };

        let altura_imagem = (lado * opcoes.razao).round();
        let altura_rodape = opcoes.altura_rodape.unwrap_or({
            if lado >= RODAPE_A_PARTIR_DE {
                ALTURA_RODAPE
            } else {
                0.0
            }
        });
        let altura_tile = altura_imagem + altura_rodape;

        let linhas = linhas_necessarias(total, colunas);
        let altura_total = if linhas == 0 {
            0.0
        } else {
            linhas as f32 * altura_tile + (linhas - 1) as f32 * espaco
        };

        Layout {
            largura: l,
            colunas,
            lado,
            altura_imagem,
            altura_rodape,
            altura_tile,
            espaco,
            total,
            altura_total,
        }
    }

    fn passo_x(&self) -> f32 {
        self.lado + self.espaco
    }

    fn passo_y(&self) -> f32 {
        self.altura_tile + self.espaco
    }

    pub fn posicao_do(&self, indice: usize) -> Retangulo {
        let coluna = indice % self.colunas;
        let linha = indice / self.colunas;
        Retangulo {
            x: coluna as f32 * self.passo_x(),
            y: linha as f32 * self.passo_y(),
            w: self.lado,
            h: self.altura_tile,
        }
    }

    /// A foto sob um ponto — `None` no respiro entre tiles ou fora da grade.
    pub fn indice_em(&self, x: f32, y: f32) -> Option<usize> {
        if x < 0.0 || y < 0.0 || self.total == 0 {
            return None;
        }
        let coluna = (x / self.passo_x()).floor();
        let linha = (y / self.passo_y()).floor();
        if !coluna.is_finite() || !linha.is_finite() || coluna < 0.0 || linha < 0.0 {
            return None;
        }
        let (coluna, linha) = (coluna as usize, linha as usize);
        if coluna >= self.colunas {
            return None;
        }
        // Dentro do passo, mas depois do tile: é respiro, não é foto.
        if x - coluna as f32 * self.passo_x() >= self.lado {
            return None;
        }
        if y - linha as f32 * self.passo_y() >= self.altura_tile {
            return None;
        }
        let i = linha * self.colunas + coluna;
        (i < self.total).then_some(i)
    }

    /// `[início, fim)` das fotos que tocam a faixa `[deslocamento, +altura]`.
    ///
    /// É o que decide quantas miniaturas ficam na memória e quantos tiles são
    /// desenhados: uma grade de 2000 fotos desenha as 30 que estão na tela.
    pub fn intervalo_visivel(&self, deslocamento: f32, altura: f32) -> (usize, usize) {
        if self.total == 0 || altura <= 0.0 {
            return (0, 0);
        }
        let passo = self.passo_y();
        let primeira = (deslocamento / passo).floor().max(0.0) as usize;
        let ultima = ((deslocamento + altura - 1.0) / passo).floor().max(0.0) as usize;
        let inicio = (primeira * self.colunas).min(self.total);
        let fim = ((ultima + 1) * self.colunas).min(self.total);
        (inicio, fim.max(inicio))
    }

    /// Os índices que um retângulo toca, em ordem de leitura.
    pub fn indices_no_retangulo(&self, r: Retangulo) -> Vec<usize> {
        let mut achados = Vec::new();
        if self.total == 0 || r.w < 0.0 || r.h < 0.0 {
            return achados;
        }
        let c0 = (r.x / self.passo_x()).floor().max(0.0) as usize;
        let c1 = ((r.x + r.w) / self.passo_x())
            .floor()
            .max(0.0)
            .min((self.colunas - 1) as f32) as usize;
        let l0 = (r.y / self.passo_y()).floor().max(0.0) as usize;
        let l1 = ((r.y + r.h) / self.passo_y()).floor().max(0.0) as usize;

        for linha in l0..=l1 {
            for coluna in c0..=c1 {
                let i = linha * self.colunas + coluna;
                if i >= self.total {
                    break;
                }
                if r.toca(&self.posicao_do(i)) {
                    achados.push(i);
                }
            }
        }
        achados
    }

    /// Para onde o foco vai com uma seta — preso às bordas, como num
    /// gerenciador de arquivos: "baixo" na última linha não sai do lugar, e
    /// "baixo" numa coluna que não tem tile embaixo vai para a última foto.
    pub fn mover(&self, indice: usize, direcao: Direcao) -> usize {
        if self.total == 0 {
            return 0;
        }
        let ultimo = self.total - 1;
        let atual = indice.min(ultimo);
        match direcao {
            Direcao::Esquerda => atual.saturating_sub(1),
            Direcao::Direita => (atual + 1).min(ultimo),
            Direcao::Cima => atual.checked_sub(self.colunas).unwrap_or(atual),
            Direcao::Baixo => {
                let abaixo = atual + self.colunas;
                if abaixo <= ultimo {
                    abaixo
                } else if atual / self.colunas < ultimo / self.colunas {
                    // Há linha de baixo, só não nesta coluna → a última foto.
                    ultimo
                } else {
                    atual
                }
            }
            Direcao::Inicio => 0,
            Direcao::Fim => ultimo,
        }
    }

    /// A faixa de índices que a linha `indice` mostra — o que o `uniform_list`
    /// do GPUI pede, fechada no início e aberta no fim.
    ///
    /// **Recortada pelo total**: a última linha costuma ser parcial, e pedir
    /// `fotos[24..27]` num acervo de 25 é pânico, não tela vazia.
    pub fn fotos_da_linha(&self, indice: usize) -> std::ops::Range<usize> {
        fotos_da_linha(indice, self.colunas, self.total)
    }

    pub fn linhas(&self) -> usize {
        linhas_necessarias(self.total, self.colunas)
    }
}

/// Quantas colunas de `lado_alvo` cabem em `largura`, com `espaco` entre elas.
///
/// 🔑 **A última coluna não paga respiro à direita** — é a conta que o desktop
/// tinha errada, e que lhe custava uma coluna em certas larguras de janela.
///
/// **Nunca devolve zero**: numa largura menor que um tile a resposta honesta é
/// "uma, ocupando o que houver". Zero faria `total / colunas` dividir por zero
/// e a grade sumir quando alguém arrasta a borda da janela para a esquerda —
/// e `NAN as usize` é 0 em Rust, que é como isso chegava aqui no primeiro
/// quadro, antes de a janela ser medida.
pub fn colunas_que_cabem(largura: f32, lado_alvo: f32, espaco: f32) -> usize {
    if !largura.is_finite() || !lado_alvo.is_finite() || lado_alvo <= 0.0 {
        return 1;
    }
    let espaco = if espaco.is_finite() {
        espaco.max(0.0)
    } else {
        0.0
    };
    let cabem = ((largura + espaco) / (lado_alvo + espaco)).floor();
    if cabem.is_finite() && cabem >= 1.0 {
        cabem as usize
    } else {
        1
    }
}

/// Quantas linhas a grade tem.
///
/// Divisão para cima: 7 fotos em 3 colunas são 3 linhas, e não 2 — a última
/// fica pela metade. Arredondar para baixo esconderia a última linha inteira, e
/// o sintoma seria "as fotos mais recentes sumiram".
pub fn linhas_necessarias(total: usize, colunas: usize) -> usize {
    if colunas == 0 {
        return 0;
    }
    total.div_ceil(colunas)
}

/// A faixa de índices da linha, recortada pelo total.
pub fn fotos_da_linha(indice: usize, colunas: usize, total: usize) -> std::ops::Range<usize> {
    if colunas == 0 {
        return 0..0;
    }
    let inicio = indice.saturating_mul(colunas).min(total);
    let fim = (inicio + colunas).min(total);
    inicio..fim
}

/// Os índices de `a` até `b`, inclusive, em qualquer ordem — a seleção com Shift.
pub fn faixa_entre(a: usize, b: usize) -> std::ops::RangeInclusive<usize> {
    a.min(b)..=a.max(b)
}

/// O recorte de origem que faz a imagem cobrir `w × h` sem deformar
/// (`object-fit: cover`), em coordenadas **normalizadas** (0–1).
///
/// 🔑 Normalizadas, e não em pixels, porque é assim que a GPU as consome: o
/// mesmo número vira `uv` no shader do navegador e recorte no desktop.
pub fn recorte_cobrir(largura_imagem: f32, altura_imagem: f32, w: f32, h: f32) -> Retangulo {
    let inteira = Retangulo {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };
    if largura_imagem <= 0.0 || altura_imagem <= 0.0 || w <= 0.0 || h <= 0.0 {
        return inteira;
    }
    let razao_alvo = w / h;
    let razao_imagem = largura_imagem / altura_imagem;
    if razao_imagem > razao_alvo {
        // Mais larga que o alvo: corta as laterais.
        let largura_util = altura_imagem * razao_alvo / largura_imagem;
        Retangulo {
            x: (1.0 - largura_util) / 2.0,
            y: 0.0,
            w: largura_util,
            h: 1.0,
        }
    } else {
        let altura_util = largura_imagem / razao_alvo / altura_imagem;
        Retangulo {
            x: 0.0,
            y: (1.0 - altura_util) / 2.0,
            w: 1.0,
            h: altura_util,
        }
    }
}

/// O retângulo em que `largura × altura` cabe inteiro dentro de `caixa`,
/// centralizado (`object-fit: contain`) — o visualizador em tela cheia.
pub fn caber_em(largura: f32, altura: f32, caixa: Retangulo) -> Retangulo {
    if largura <= 0.0 || altura <= 0.0 || caixa.w <= 0.0 || caixa.h <= 0.0 {
        return Retangulo {
            x: caixa.x,
            y: caixa.y,
            w: 0.0,
            h: 0.0,
        };
    }
    let escala = (caixa.w / largura).min(caixa.h / altura);
    let w = largura * escala;
    let h = altura * escala;
    Retangulo {
        x: caixa.x + (caixa.w - w) / 2.0,
        y: caixa.y + (caixa.h - h) / 2.0,
        w,
        h,
    }
}

/// Saída suave (cúbica): rápido no começo, assenta no fim.
pub fn suavizar(t: f32) -> f32 {
    let k = t.clamp(0.0, 1.0);
    1.0 - (1.0 - k).powi(3)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn painel(largura: f32, zoom: f32, total: usize) -> Layout {
        Layout::calcular(largura, zoom, total, Opcoes::default())
    }

    #[test]
    fn distribui_a_largura_entre_as_colunas_que_cabem_sem_sobra() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.colunas, 4);
        // 4 tiles + 3 respiros ocupam a largura inteira.
        assert!((l.lado * 4.0 + l.espaco * 3.0 - 1000.0).abs() < 0.001);
        assert_eq!(l.altura_rodape, ALTURA_RODAPE);
    }

    /// 🚨 A conta que o desktop tinha errada: a última coluna não paga respiro.
    #[test]
    fn a_ultima_coluna_nao_paga_respiro_a_direita() {
        // 999 px com tile-alvo de 188 e respiro de 12: cabem cinco.
        let l = Layout::calcular(
            999.0,
            188.0,
            20,
            Opcoes {
                espaco: 12.0,
                ..Default::default()
            },
        );
        assert_eq!(l.colunas, 5);
        assert!(l.lado * 5.0 + 12.0 * 4.0 <= 999.0 + 0.001);
    }

    #[test]
    fn num_conteiner_mais_estreito_que_o_zoom_uma_coluna_ocupa_tudo_e_o_rodape_some() {
        let l = painel(120.0, 220.0, 3);
        assert_eq!(l.colunas, 1);
        assert_eq!(l.lado, 120.0);
        assert_eq!(l.altura_rodape, 0.0, "não há espaço para ler nome");
    }

    #[test]
    fn sem_fotos_a_grade_tem_altura_zero() {
        assert_eq!(painel(1000.0, 220.0, 0).altura_total, 0.0);
    }

    #[test]
    fn a_galeria_do_cliente_pede_tile_quadrado_sem_rodape_e_com_respiro_menor() {
        let l = Layout::calcular(
            600.0,
            180.0,
            9,
            Opcoes {
                razao: 1.0,
                altura_rodape: Some(0.0),
                espaco: 8.0,
            },
        );
        assert_eq!(l.altura_rodape, 0.0);
        assert_eq!(l.altura_imagem, l.lado.round());
        assert_eq!(l.altura_tile, l.altura_imagem);
    }

    /// 🚨 O primeiro quadro mede o contêiner como `NAN` — e a grade não pode
    /// sumir por isso.
    #[test]
    fn largura_absurda_nao_derruba_a_conta() {
        for largura in [f32::NAN, f32::INFINITY, -50.0] {
            let l = painel(largura, 220.0, 10);
            assert_eq!(l.colunas, 1);
            assert!(l.lado >= 0.0);
        }
    }

    #[test]
    fn a_sexta_foto_fica_na_segunda_coluna_da_segunda_linha() {
        let l = painel(1000.0, 220.0, 10);
        let r = l.posicao_do(5);
        assert!((r.x - (l.lado + l.espaco)).abs() < 0.001);
        assert!((r.y - (l.altura_tile + l.espaco)).abs() < 0.001);
    }

    #[test]
    fn o_ponto_dentro_de_um_tile_devolve_o_indice_dele() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.indice_em(10.0, 10.0), Some(0));
        assert_eq!(l.indice_em(l.lado + l.espaco + 5.0, 5.0), Some(1));
    }

    #[test]
    fn o_respiro_entre_tiles_nao_e_foto_nenhuma() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.indice_em(l.lado + 2.0, 10.0), None);
        assert_eq!(l.indice_em(10.0, l.altura_tile + 2.0), None);
    }

    #[test]
    fn uma_posicao_alem_da_ultima_foto_nao_e_foto() {
        let l = painel(1000.0, 220.0, 10);
        // A terceira linha tem só duas fotos (índices 8 e 9).
        let terceira_linha = 2.0 * (l.altura_tile + l.espaco) + 5.0;
        assert_eq!(l.indice_em(5.0, terceira_linha), Some(8));
        assert_eq!(
            l.indice_em(2.0 * (l.lado + l.espaco) + 5.0, terceira_linha),
            None
        );
    }

    #[test]
    fn o_intervalo_visivel_cobre_as_linhas_que_tocam_a_janela() {
        let l = painel(1000.0, 220.0, 40);

        // Uma janela que acaba dentro do respiro ainda vê uma linha só: o que
        // vem depois do tile é espaço vazio, não a linha de baixo.
        assert_eq!(l.intervalo_visivel(0.0, l.altura_tile + 1.0), (0, 4));

        // Passando do respiro, a segunda linha entra inteira.
        assert_eq!(
            l.intervalo_visivel(0.0, l.altura_tile + l.espaco + 1.0),
            (0, 8),
            "duas linhas de quatro"
        );
    }

    #[test]
    fn o_intervalo_visivel_nao_passa_do_total() {
        let l = painel(1000.0, 220.0, 6);
        let (inicio, fim) = l.intervalo_visivel(0.0, 10_000.0);
        assert_eq!((inicio, fim), (0, 6));
    }

    #[test]
    fn um_retangulo_so_no_respiro_nao_pega_nada() {
        let l = painel(1000.0, 220.0, 10);
        let r = Retangulo {
            x: l.lado + 1.0,
            y: 0.0,
            w: 4.0,
            h: 10.0,
        };
        assert!(l.indices_no_retangulo(r).is_empty());
    }

    #[test]
    fn um_retangulo_que_toca_dois_tiles_pega_os_dois() {
        let l = painel(1000.0, 220.0, 10);
        let r = Retangulo {
            x: l.lado - 5.0,
            y: 5.0,
            w: l.espaco + 10.0,
            h: 5.0,
        };
        assert_eq!(l.indices_no_retangulo(r), vec![0, 1]);
    }

    #[test]
    fn atravessando_linhas_pega_em_ordem_de_leitura_e_para_no_total() {
        let l = painel(1000.0, 220.0, 6);
        let r = Retangulo {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 10_000.0,
        };
        assert_eq!(l.indices_no_retangulo(r), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn o_foco_fica_preso_as_bordas_nas_setas_horizontais() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.mover(0, Direcao::Esquerda), 0);
        assert_eq!(l.mover(9, Direcao::Direita), 9);
        assert_eq!(l.mover(3, Direcao::Direita), 4);
    }

    #[test]
    fn para_baixo_numa_coluna_sem_tile_embaixo_vai_para_a_ultima_foto() {
        let l = painel(1000.0, 220.0, 10);
        // Índice 7 (segunda linha, última coluna): abaixo seria 11, que não
        // existe — mas há linha de baixo, então vai para a 9.
        assert_eq!(l.mover(7, Direcao::Baixo), 9);
        // Já na última linha, fica.
        assert_eq!(l.mover(9, Direcao::Baixo), 9);
    }

    #[test]
    fn para_cima_na_primeira_linha_fica_onde_esta() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.mover(2, Direcao::Cima), 2);
        assert_eq!(l.mover(6, Direcao::Cima), 2);
    }

    #[test]
    fn inicio_e_fim() {
        let l = painel(1000.0, 220.0, 10);
        assert_eq!(l.mover(5, Direcao::Inicio), 0);
        assert_eq!(l.mover(5, Direcao::Fim), 9);
    }

    #[test]
    fn a_faixa_vale_nas_duas_direcoes() {
        assert_eq!(faixa_entre(2, 5).collect::<Vec<_>>(), vec![2, 3, 4, 5]);
        assert_eq!(faixa_entre(5, 2).collect::<Vec<_>>(), vec![2, 3, 4, 5]);
    }

    #[test]
    fn o_retangulo_arrastado_de_baixo_para_cima_e_o_mesmo() {
        assert_eq!(
            Retangulo::entre(10.0, 20.0, 30.0, 5.0),
            Retangulo {
                x: 10.0,
                y: 5.0,
                w: 20.0,
                h: 15.0
            }
        );
    }

    #[test]
    fn o_recorte_de_cobrir_corta_as_laterais_da_imagem_mais_larga() {
        let r = recorte_cobrir(2000.0, 1000.0, 100.0, 100.0);
        assert!((r.w - 0.5).abs() < 0.001, "usa metade da largura");
        assert!((r.x - 0.25).abs() < 0.001, "centralizado");
        assert_eq!(r.h, 1.0);
    }

    #[test]
    fn o_recorte_de_cobrir_corta_em_cima_e_embaixo_da_imagem_mais_alta() {
        let r = recorte_cobrir(1000.0, 2000.0, 100.0, 100.0);
        assert_eq!(r.w, 1.0);
        assert!((r.h - 0.5).abs() < 0.001);
        assert!((r.y - 0.25).abs() < 0.001);
    }

    #[test]
    fn recorte_de_imagem_sem_tamanho_e_a_imagem_inteira() {
        let r = recorte_cobrir(0.0, 0.0, 100.0, 100.0);
        assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn caber_em_encosta_no_lado_maior_e_centraliza_no_outro() {
        let caixa = Retangulo {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let larga = caber_em(200.0, 100.0, caixa);
        assert_eq!((larga.w, larga.h), (100.0, 50.0));
        assert_eq!(larga.y, 25.0);

        let alta = caber_em(100.0, 200.0, caixa);
        assert_eq!((alta.w, alta.h), (50.0, 100.0));
        assert_eq!(alta.x, 25.0);
    }

    #[test]
    fn a_interpolacao_fica_presa_nas_pontas() {
        let a = Retangulo {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        let b = Retangulo {
            x: 100.0,
            y: 100.0,
            w: 50.0,
            h: 50.0,
        };
        assert_eq!(Retangulo::interpolar(a, b, -1.0), a);
        assert_eq!(Retangulo::interpolar(a, b, 2.0), b);
        assert_eq!(Retangulo::interpolar(a, b, 0.5).x, 50.0);
    }

    #[test]
    fn a_suavizacao_comeca_rapido_e_assenta_no_fim() {
        assert_eq!(suavizar(0.0), 0.0);
        assert_eq!(suavizar(1.0), 1.0);
        assert!(suavizar(0.5) > 0.5, "já passou da metade no meio do tempo");
    }

    #[test]
    fn a_ultima_linha_parcial_conta_como_linha() {
        assert_eq!(linhas_necessarias(7, 3), 3);
        assert_eq!(linhas_necessarias(0, 3), 0);
        assert_eq!(linhas_necessarias(1, 3), 1);
    }

    #[test]
    fn a_linha_alem_do_fim_e_vazia_e_nao_panico() {
        assert_eq!(fotos_da_linha(99, 4, 10), 10..10);
        assert!(fotos_da_linha(99, 4, 10).is_empty());
    }

    /// A propriedade que importa numa grade virtualizada: percorrer as linhas
    /// reconstrói o acervo inteiro, sem buraco e sem repetição. O sintoma de
    /// errar isto é a foto duplicada na emenda entre duas linhas.
    #[test]
    fn toda_foto_aparece_exatamente_uma_vez() {
        for total in [0usize, 1, 7, 25, 2000] {
            for colunas in 1..=8 {
                let vistas: Vec<usize> = (0..linhas_necessarias(total, colunas))
                    .flat_map(|i| fotos_da_linha(i, colunas, total))
                    .collect();
                assert_eq!(
                    vistas,
                    (0..total).collect::<Vec<_>>(),
                    "total={total} colunas={colunas}"
                );
            }
        }
    }

    /// E a mesma propriedade pelo lado da geometria: todo tile visível tem
    /// índice, e todo índice cai dentro do próprio tile.
    #[test]
    fn o_ponto_no_centro_de_cada_tile_devolve_o_indice_dele() {
        let l = painel(1000.0, 220.0, 37);
        for i in 0..l.total {
            let r = l.posicao_do(i);
            assert_eq!(
                l.indice_em(r.x + r.w / 2.0, r.y + r.h / 2.0),
                Some(i),
                "tile {i}"
            );
        }
    }
}
