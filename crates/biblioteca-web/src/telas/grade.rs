//! A grade — desenhada com o pintor do egui, decidida pelo core.
//!
//! Só os tiles visíveis existem em cada quadro: o `ScrollArea` dá o recorte,
//! o core (`Layout::intervalo_visivel`) diz quais índices caem nele, e o resto
//! da altura é espaço alocado e vazio. Uma sessão de 2.000 fotos desenha as 30
//! da tela.
//!
//! Toda decisão de seleção é do core: clique, Shift, Ctrl, arrasto e teclado
//! chegam aqui como gestos do egui e saem como chamadas a `Selecao`.

use biblioteca_core::grade::{recorte_cobrir, Direcao, Retangulo};
use biblioteca_core::negociacao;
use biblioteca_core::selecao::Modificadores;
use egui::{Color32, Key, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

use crate::tema;

use crate::app::{App, Comando, Pedido};

const RAIO: f32 = tema::RAIO_DO_TILE;
const CAIXA: f32 = 22.0;
const MARGEM: f32 = 8.0;

impl App {
    pub(crate) fn ui_grade(&mut self, ui: &mut egui::Ui, comandos: &mut Vec<Comando>) {
        let largura = ui.available_width();
        let layout = self.layout_da_grade(largura);
        self.grade.layout = Some(layout);

        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(largura, layout.altura_total.max(1.0)),
            Sense::click_and_drag(),
        );
        let id_da_grade = resp.id;
        let visivel = ui.clip_rect().intersect(rect);
        let deslocamento = (visivel.top() - rect.top()).max(0.0);
        let (inicio, fim) = layout.intervalo_visivel(deslocamento, visivel.height());

        // As miniaturas do que está à vista, com uma folga de duas linhas.
        let folga = layout.colunas * 2;
        let desejadas: Vec<String> = (inicio.saturating_sub(folga)
            ..(fim + folga).min(layout.total))
            .filter_map(|n| self.foto_visivel(n).map(|f| f.miniatura.clone()))
            .collect();
        self.pedir_miniaturas(desejadas);

        // ---- gestos ----
        let mods = ui.input(|i| i.modifiers);
        let modificadores = Modificadores {
            aditivo: mods.command,
            faixa: mods.shift,
        };
        let para_conteudo = |p: Pos2| (p.x - rect.left(), p.y - rect.top());

        if let Some(p) = resp.hover_pos() {
            let (x, y) = para_conteudo(p);
            self.grade.hover = layout.indice_em(x, y);
        } else {
            self.grade.hover = None;
        }

        if resp.clicked() {
            resp.request_focus();
            if let Some(p) = resp.interact_pointer_pos() {
                let (x, y) = para_conteudo(p);
                match layout.indice_em(x, y) {
                    Some(n) => {
                        let tile = layout.posicao_do(n);
                        let na_caixa = dentro(x, y, area_da_caixa(tile));
                        self.selecao.clicar(n, na_caixa, modificadores);
                    }
                    None => self.selecao.clicar_no_vazio(modificadores),
                }
            }
        }
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let (x, y) = para_conteudo(p);
                if let Some(f) = layout.indice_em(x, y).and_then(|n| self.foto_visivel(n)) {
                    comandos.push(Comando::Pedido(Pedido::AbrirUrl {
                        url: f.previa.clone(),
                    }));
                }
            }
        }
        if resp.drag_started() {
            resp.request_focus();
            self.grade.arrasto_base = Some(if modificadores.aditivo || modificadores.faixa {
                self.selecao.instantaneo()
            } else {
                Default::default()
            });
        }
        if resp.dragged() {
            if let (Some(base), Some(origem), Some(atual)) = (
                self.grade.arrasto_base.clone(),
                ui.input(|i| i.pointer.press_origin()),
                resp.interact_pointer_pos(),
            ) {
                let (x0, y0) = para_conteudo(origem);
                let (x1, y1) = para_conteudo(atual);
                let r = Retangulo::entre(x0, y0, x1, y1);
                let dentro_do_retangulo = layout.indices_no_retangulo(r);
                self.selecao.arrastar(&base, &dentro_do_retangulo);
                ui.painter().rect(
                    Rect::from_min_size(
                        Pos2::new(rect.left() + r.x, rect.top() + r.y),
                        Vec2::new(r.w, r.h),
                    ),
                    0.0,
                    Color32::from_rgba_unmultiplied(0xfb, 0xbf, 0x24, 40),
                    Stroke::new(1.0_f32, tema::AMBAR),
                );
            }
        }
        if resp.drag_stopped() {
            self.grade.arrasto_base = None;
        }

        // ---- teclado, quando a grade tem o foco ----
        if ui.memory(|m| m.has_focus(id_da_grade)) {
            self.ui_teclado_da_grade(ui, comandos, modificadores);
        }

        // Ctrl + roda = zoom: o egui converte em `zoom_delta`.
        if resp.hovered() {
            let z = ui.input(|i| i.zoom_delta());
            if (z - 1.0).abs() > 0.001 {
                comandos.push(Comando::Zoom(self.zoom * z));
            }
        }

        // Rolar até o tile que o teclado alcançou.
        if let Some(n) = self.grade.rolar_para.take() {
            let t = layout.posicao_do(n);
            let alvo = Rect::from_min_size(
                Pos2::new(rect.left() + t.x, rect.top() + t.y),
                Vec2::new(t.w, t.h),
            );
            ui.scroll_to_rect(alvo, None);
        }

        // ---- desenho ----
        let pintor = ui.painter_at(visivel);
        // As cores da tira do editor: fundo neutral-800, borda âmbar na
        // escolhida, texto neutral-300/400.
        let cor_tile = tema::N800;
        let cor_texto = tema::N300;
        let cor_suave = tema::N400;
        let cor_selecao = tema::AMBAR;
        let cor_foco = tema::N100;
        let fonte = egui::FontId::proportional(12.0);
        let fonte_pequena = egui::FontId::proportional(10.5);

        for n in inicio..fim {
            let Some(foto) = self.foto_visivel(n).cloned() else {
                continue;
            };
            let t = layout.posicao_do(n);
            let tile = Rect::from_min_size(
                Pos2::new(rect.left() + t.x, rect.top() + t.y),
                Vec2::new(t.w, t.h),
            );
            let imagem = Rect::from_min_size(tile.min, Vec2::new(t.w, layout.altura_imagem));

            pintor.rect_filled(imagem, Rounding::same(RAIO), cor_tile);

            if let Some(tex) = self.texturas.get(&foto.miniatura) {
                let tam = tex.size_vec2();
                let uv = recorte_cobrir(tam.x, tam.y, imagem.width(), imagem.height());
                let uv = Rect::from_min_size(Pos2::new(uv.x, uv.y), Vec2::new(uv.w, uv.h));
                let mut img =
                    egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), tam))
                        .uv(uv)
                        .rounding(Rounding::same(RAIO));
                if foto.apagada_em.is_some() {
                    img = img.tint(Color32::from_gray(90));
                }
                img.paint_at(ui, imagem);
            }

            let selecionada = self.selecao.tem(n);
            let em_foco = self.selecao.foco() == Some(n);
            if selecionada {
                pintor.rect_stroke(
                    imagem,
                    Rounding::same(RAIO),
                    Stroke::new(2.0_f32, cor_selecao),
                );
            }
            if em_foco {
                pintor.rect_stroke(
                    imagem.expand(2.0),
                    Rounding::same(RAIO + 2.0),
                    Stroke::new(1.5_f32, cor_foco),
                );
            }
            if selecionada || self.grade.hover == Some(n) {
                let caixa = area_da_caixa(t);
                let caixa = Rect::from_min_size(
                    Pos2::new(rect.left() + caixa.x, rect.top() + caixa.y),
                    Vec2::new(caixa.w, caixa.h),
                );
                pintor.rect(
                    caixa,
                    5.0,
                    if selecionada {
                        cor_selecao
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 110)
                    },
                    Stroke::new(
                        1.0_f32,
                        if selecionada {
                            cor_selecao
                        } else {
                            Color32::WHITE
                        },
                    ),
                );
                if selecionada {
                    // `bg-amber-400 text-black`, como o botão ligado do editor.
                    pintor.text(
                        caixa.center(),
                        egui::Align2::CENTER_CENTER,
                        "✔",
                        egui::FontId::proportional(14.0),
                        Color32::BLACK,
                    );
                }
            }

            if layout.altura_rodape > 0.0 {
                let rodape = Rect::from_min_size(
                    Pos2::new(tile.left(), imagem.bottom()),
                    Vec2::new(t.w, layout.altura_rodape),
                );
                let x = rodape.left() + 4.0;
                let largura_texto = rodape.width() - 8.0;

                // Linha 1: nome e situação.
                let situacao = foto.rotulo_do_estado();
                let galley_situacao = ui.fonts(|f| {
                    f.layout_no_wrap(situacao.to_string(), fonte_pequena.clone(), cor_suave)
                });
                let largura_nome = (largura_texto - galley_situacao.size().x - 6.0).max(20.0);
                let nome = truncado(ui, &foto.arquivo, fonte.clone(), cor_texto, largura_nome);
                pintor.galley(Pos2::new(x, rodape.top() + 4.0), nome.clone(), cor_texto);
                pintor.galley(
                    Pos2::new(x + nome.size().x + 6.0, rodape.top() + 6.0),
                    galley_situacao,
                    cor_suave,
                );

                // Linha 2: faixa, preço fixado, extra.
                let mut partes = vec![self.estado.nome_da_faixa(&foto.produto_efetivo)];
                if let Some(p) = foto.preco_de_venda {
                    partes.push(format!(
                        "vende por {}",
                        biblioteca_core::dinheiro::formatar(p)
                    ));
                }
                if let Some(a) = &foto.apagada_em {
                    partes.push(format!("apagada em {a}"));
                } else if foto.estado == "comprada" {
                    if let Some(p) = &foto.pedido_id {
                        partes.push(format!("pedido {}", &p[..p.len().min(8)]));
                    }
                } else if foto.estado == "levada_no_balcao" {
                    partes.push(format!("{} download(s)", foto.downloads));
                }
                if let Some(e) = self.edicao_local(&foto.id) {
                    partes.push(format!("editada {} · não salva", e.atualizada_em));
                }
                if foto.preco_negociado.is_some()
                    || foto.observacao.as_deref().is_some_and(|o| !o.is_empty())
                {
                    let e = negociacao::descrever(foto.preco_negociado, foto.observacao.as_deref());
                    partes.push(e.titulo.clone());
                }
                let linha2 = truncado(
                    ui,
                    &partes.join(" · "),
                    fonte_pequena.clone(),
                    cor_suave,
                    largura_texto,
                );
                pintor.galley(Pos2::new(x, rodape.top() + 21.0), linha2, cor_suave);
            }
        }

        if self.grade.hover.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    fn ui_teclado_da_grade(
        &mut self,
        ui: &mut egui::Ui,
        comandos: &mut Vec<Comando>,
        modificadores: Modificadores,
    ) {
        let Some(layout) = self.grade.layout else {
            return;
        };
        let setas = [
            (Key::ArrowLeft, Direcao::Esquerda),
            (Key::ArrowRight, Direcao::Direita),
            (Key::ArrowUp, Direcao::Cima),
            (Key::ArrowDown, Direcao::Baixo),
            (Key::Home, Direcao::Inicio),
            (Key::End, Direcao::Fim),
        ];
        for (tecla, direcao) in setas {
            if ui.input(|i| i.key_pressed(tecla)) {
                self.grade.rolar_para = self.selecao.mover(direcao, &layout, modificadores);
            }
        }
        let altura_visivel = ui.clip_rect().height();
        let linhas =
            ((altura_visivel / (layout.altura_tile + layout.espaco)).floor() as usize).max(1);
        if ui.input(|i| i.key_pressed(Key::PageDown)) {
            self.grade.rolar_para = self.selecao.saltar(linhas, true, &layout, modificadores);
        }
        if ui.input(|i| i.key_pressed(Key::PageUp)) {
            self.grade.rolar_para = self.selecao.saltar(linhas, false, &layout, modificadores);
        }
        if ui.input(|i| i.key_pressed(Key::Space)) {
            self.selecao.alternar_foco();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.selecao.desmarcar();
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::A)) {
            self.selecao.marcar_todas(layout.total);
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(f) = self.id_em_foco().and_then(|id| self.foto(&id)) {
                comandos.push(Comando::Pedido(Pedido::AbrirUrl {
                    url: f.previa.clone(),
                }));
            }
        }
    }
}

fn area_da_caixa(t: Retangulo) -> Retangulo {
    Retangulo {
        x: t.x + t.w - MARGEM - CAIXA,
        y: t.y + MARGEM,
        w: CAIXA,
        h: CAIXA,
    }
}

fn dentro(x: f32, y: f32, r: Retangulo) -> bool {
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
}

/// Um texto numa linha só, cortado com reticências para caber.
fn truncado(
    ui: &egui::Ui,
    texto: &str,
    fonte: egui::FontId,
    cor: Color32,
    largura: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(texto.to_string(), fonte, cor);
    job.wrap = egui::text::TextWrapping {
        max_width: largura,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.fonts(|f| f.layout_job(job))
}
