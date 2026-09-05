//! O desenho da grade — retângulos e texturas, e nada de texto.
//!
//! Por tile à vista: o fundo (enquanto a miniatura não chega), a miniatura em
//! *cover* com cantos arredondados, o véu da apagada, o anel âmbar da
//! selecionada, o anel do foco (só quando o canvas tem o teclado), a caixinha
//! de marcar no hover e na selecionada — e o laço do arrasto por cima de tudo.
//!
//! O nome, a situação, a faixa e o "editada · não salva" **não são pintados
//! aqui**: são DOM do site, posicionados por `visiveis_json`. É por isso que o
//! egui entra sem fontes.
//!
//! Coordenadas: o core fala em conteúdo; o canvas mostra a janela que começa
//! em `deslocamento`. Subtrair é tudo o que este arquivo faz de geometria.

use biblioteca_core::grade::recorte_cobrir;
use egui::epaint::RectShape;
use egui::{Color32, Pos2, Rect, Rounding, Shape, Stroke, Vec2};

use crate::grade::{area_da_caixa, Grade};
use crate::tema;

/// `rounded-lg` — o tile da foto.
const RAIO: f32 = 8.0;

pub fn pintar(ctx: &egui::Context, g: &Grade) {
    let p = ctx.layer_painter(egui::LayerId::background());
    let d = g.deslocamento;
    let cores = g.cores;
    let (inicio, fim) = g.intervalo;

    for n in inicio..fim {
        let Some(foto) = g.acervo.visivel(n) else {
            continue;
        };
        let t = g.layout.posicao_do(n);
        let imagem = Rect::from_min_size(
            Pos2::new(t.x, t.y - d),
            Vec2::new(t.w, g.layout.altura_imagem),
        );
        // Fora da janela (a folga do intervalo): não custa desenhar.
        if imagem.bottom() < 0.0 || imagem.top() > g.altura_visivel {
            continue;
        }

        p.rect_filled(imagem, Rounding::same(RAIO), cores.fundo_do_tile);

        if let Some(tex) = g.miniaturas.get(&foto.id).and_then(|u| g.texturas.get(u)) {
            let tam = tex.size_vec2();
            let uv = recorte_cobrir(tam.x, tam.y, imagem.width(), imagem.height());
            let mut forma = RectShape::filled(imagem, Rounding::same(RAIO), Color32::WHITE);
            forma.fill_texture_id = tex.id();
            forma.uv = Rect::from_min_size(Pos2::new(uv.x, uv.y), Vec2::new(uv.w, uv.h));
            p.add(Shape::Rect(forma));
        }
        if foto.apagada {
            p.rect_filled(imagem, Rounding::same(RAIO), cores.veu);
        }

        let selecionada = g.selecao.tem(n);
        let em_foco = g.tem_foco && g.selecao.foco() == Some(n);
        if selecionada {
            p.rect_stroke(
                imagem,
                Rounding::same(RAIO),
                Stroke::new(2.0_f32, tema::AMBAR),
            );
        }
        if em_foco {
            p.rect_stroke(
                imagem.expand(2.0),
                Rounding::same(RAIO + 2.0),
                Stroke::new(1.5_f32, cores.foco),
            );
        }

        if selecionada || g.hover == Some(n) {
            let c = area_da_caixa(t);
            let caixa = Rect::from_min_size(Pos2::new(c.x, c.y - d), Vec2::new(c.w, c.h));
            let (fundo, borda) = if selecionada {
                (tema::AMBAR, tema::AMBAR)
            } else {
                (cores.caixa, cores.borda_da_caixa)
            };
            p.rect(
                caixa,
                Rounding::same(5.0),
                fundo,
                Stroke::new(1.0_f32, borda),
            );
            if selecionada {
                // O ✔ em preto sobre âmbar — `bg-amber-400 text-black`, como o
                // botão ligado do editor. Duas linhas, porque não há fonte.
                let o = caixa.min;
                let traco = Stroke::new(2.0_f32, Color32::BLACK);
                p.line_segment([o + Vec2::new(6.0, 11.5), o + Vec2::new(9.5, 15.0)], traco);
                p.line_segment([o + Vec2::new(9.5, 15.0), o + Vec2::new(16.0, 7.5)], traco);
            }
        }
    }

    if let Some(a) = g.arrasto.as_ref().filter(|a| a.ativo) {
        let r = a.retangulo();
        p.rect(
            Rect::from_min_size(Pos2::new(r.x, r.y - d), Vec2::new(r.w, r.h)),
            0.0,
            Color32::from_rgba_unmultiplied(0xfb, 0xbf, 0x24, 40),
            Stroke::new(1.0_f32, tema::AMBAR),
        );
    }
}
