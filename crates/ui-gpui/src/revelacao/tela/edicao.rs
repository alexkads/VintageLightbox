//! A Revelação e o editor em camadas: o pedido "Editar Foto" e a troca da fonte
//! quando uma edição é salva (`docs/editor-em-camadas/02-CONTRATO.md`).
//!
//! 🔑 **A janela do editor nunca escreve aqui.** Ela anuncia a versão salva à
//! raiz, e a raiz chama [`Revelacao::fonte_mudou`] — que troca **só a entrada**
//! da foto. Ajustes, corte, máscaras, histórico, lote e foco ficam como estavam:
//! a revelação é reaplicada sobre a imagem nova.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui_kit::{Context, Window};

use super::super::fonte;
use super::super::persistencia;
use super::{tira, Origem, PedidoDaRevelacao, Revelacao};
use crate::editor::porta::Edicoes;

impl Revelacao {
    /// Liga a Revelação às edições em camadas (montagem no `main`).
    pub fn definir_edicoes(&mut self, edicoes: Arc<dyn Edicoes>) {
        self.edicoes = Some(edicoes);
    }

    /// As edições em camadas, para quem abre o editor.
    pub fn edicoes(&self) -> Option<Arc<dyn Edicoes>> {
        self.edicoes.clone()
    }

    /// A foto pode ir ao editor? A comprada e a apagada não se revelam — e não
    /// se editam —, e sem arquivo nem id no site não há bruto de onde partir.
    pub fn pode_editar(foto: &PhotoViewModel) -> bool {
        tira::classificacao(foto).editavel()
            && (!foto.path.is_empty() || foto.pos_venda_foto_id.is_some())
    }

    /// "Editar Foto" do menu da tira: **a foto da posição clicada**, e só ela —
    /// mesmo com várias marcadas. Lote, posição e foco não mudam.
    pub(super) fn pedir_edicao(&mut self, posicao: usize, cx: &mut Context<Self>) {
        let Some(foto) = self.acervo.get(posicao).cloned() else {
            return;
        };
        if !Self::pode_editar(&foto) {
            return;
        }
        self.tira.a_editar = Some(foto);
        cx.emit(PedidoDaRevelacao::EditarFoto);
    }

    /// 🧪 O "Editar Foto" do menu, para os testes da raiz.
    #[cfg(test)]
    pub fn pedir_edicao_para_teste(&mut self, posicao: usize, cx: &mut Context<Self>) {
        self.pedir_edicao(posicao, cx);
    }

    /// A foto tem edição do editor **em vigor** — o selo "Editada".
    pub fn tem_edicao(&self, foto: &PhotoViewModel) -> bool {
        self.edicoes
            .as_deref()
            .is_some_and(|e| crate::editor::porta::versao_da_foto(e, foto).is_some())
    }

    /// A foto tem projeto no editor (mesmo sem efeito) — o "Excluir a edição".
    pub fn tem_projeto_no_editor(&self, foto: &PhotoViewModel) -> bool {
        self.edicoes
            .as_deref()
            .is_some_and(|e| e.tem_projeto(&foto.id, foto.pos_venda_foto_id.as_deref()))
    }

    /// "Excluir a edição": pergunta, e só então pede à raiz.
    ///
    /// 🔑 **A revelação fica.** Excluir a edição é tirar os pixels pintados; os
    /// ajustes, o corte e as máscaras da Revelação são outra coisa, e
    /// continuam — agora aplicados sobre o bruto.
    pub(super) fn pedir_exclusao(
        &mut self,
        posicao: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::component::button::ButtonVariant;
        use gpui_kit::component::dialog::DialogButtonProps;
        use gpui_kit::component::WindowExt;
        use gpui_kit::{div, prelude::*, SharedString};
        let Some(foto) = self.acervo.get(posicao).cloned() else {
            return;
        };
        if !self.tem_projeto_no_editor(&foto) {
            return;
        }
        let esta = cx.entity();
        let nome = foto.name.clone();
        window.open_alert_dialog(cx, move |dialogo, _window, _cx| {
            let para_ok = esta.clone();
            let foto = foto.clone();
            dialogo
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Excluir a edição")
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text("Cancelar")
                        .show_cancel(true),
                )
                .title(SharedString::from(format!("Excluir a edição de {nome}?")))
                .child(div().text_sm().child(
                    "O que foi pintado no editor é apagado e a foto volta ao arquivo bruto. \
                     A revelação (ajustes, corte e máscaras) continua, agora sobre o \
                     bruto. Não dá para desfazer.",
                ))
                .on_ok(move |_ev, window, cx| {
                    para_ok.update(cx, |tela, cx| {
                        tela.tira.a_excluir = Some(foto.clone());
                        cx.emit(PedidoDaRevelacao::ExcluirEdicao);
                    });
                    window.close_dialog(cx);
                    false
                })
                .on_cancel(|_ev, window, cx| {
                    window.close_dialog(cx);
                    false
                })
        });
    }

    /// 🧪 O "Excluir a edição" já confirmado, para os testes da raiz.
    #[cfg(test)]
    pub fn excluir_edicao_para_teste(&mut self, posicao: usize, cx: &mut Context<Self>) {
        if let Some(foto) = self.acervo.get(posicao).cloned() {
            self.tira.a_excluir = Some(foto);
            cx.emit(PedidoDaRevelacao::ExcluirEdicao);
        }
    }

    /// A foto que o menu mandou excluir — a raiz a leva uma vez.
    pub fn levar_a_excluir(&mut self) -> Option<PhotoViewModel> {
        self.tira.a_excluir.take()
    }

    /// A foto que o menu mandou editar — a raiz a leva uma vez.
    pub fn levar_a_editar(&mut self) -> Option<PhotoViewModel> {
        self.tira.a_editar.take()
    }

    /// A revisão da fonte da foto aberta (0 = o bruto).
    pub fn revisao_da_aberta(&self) -> u64 {
        self.aberta.as_ref().map_or(fonte::DO_BRUTO, |a| a.fonte)
    }

    /// Uma edição desta foto foi salva: a entrada dela mudou.
    ///
    /// Esquece **só o que é da foto** — as revelações guardadas, a miniatura da
    /// tira, a prévia revelada da grade e a cópia da revisão anterior — e, se
    /// ela está no palco, troca a origem e reaplica a revelação atual.
    pub fn fonte_mudou(
        &mut self,
        foto_id: &str,
        pos_venda_foto_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let da_foto = |f: &PhotoViewModel| {
            f.id == foto_id
                || pos_venda_foto_id.is_some_and(|pv| f.pos_venda_foto_id.as_deref() == Some(pv))
        };
        let ids: Vec<String> = self
            .acervo
            .iter()
            .filter(|f| da_foto(f))
            .map(|f| f.id.clone())
            .chain(std::iter::once(foto_id.to_string()))
            .collect();
        for id in &ids {
            self.reveladas.esquecer(id);
            self.miniaturas_da_tira.esquecer(id);
            self.previews.apagar(&persistencia::chave_da_revelada(id));
        }

        let Some(aberta) = self.aberta.as_ref().filter(|a| da_foto(&a.foto)) else {
            self.carregar_a_tira(cx);
            cx.notify();
            return;
        };
        let foto = aberta.foto.clone();
        let antiga = aberta.fonte;
        let copia = fonte::copia_de_trabalho(&self.previews, self.edicoes.as_deref(), &foto);
        let neutra = self.sem_revelacao();
        let Some(aberta) = self.aberta.as_mut() else {
            return;
        };
        match copia {
            Some(copia) => {
                if antiga != fonte::DO_BRUTO && antiga != copia.revisao {
                    self.previews
                        .apagar(&editor_core::contrato::chave_da_copia(&foto.id, antiga));
                }
                let rgba = copia.imagem.to_rgba8();
                aberta.origem = Some(Origem {
                    largura: rgba.width(),
                    altura: rgba.height(),
                    pixels: Arc::new(rgba.into_raw()),
                });
                aberta.fonte = copia.revisao;
                aberta.bruta = Some(copia.imagem.clone());
                aberta.revelada = neutra.then_some(copia.imagem);
                aberta.desenhada = None;
                self.resolucao.recomecar_na_copia();
                self.atualizar_exibicao();
                self.pedir_revelacao(cx);
            }
            // A edição saiu (ficou sem efeito) e o bruto desta foto do site
            // ainda não está no disco: a tela fica sem origem e a raiz vai
            // buscá-la, como numa foto recém-aberta.
            None => {
                aberta.origem = None;
                aberta.fonte = fonte::DO_BRUTO;
                cx.emit(PedidoDaRevelacao::AbriuOutraFoto);
            }
        }
        self.carregar_a_tira(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod testes {
    use std::sync::Arc;

    use adapters::view_models::PhotoViewModel;
    use editor_core::{BaseRef, Documento, Historico, Sessao};
    use gpui_kit::TestAppContext;
    use image::{DynamicImage, RgbImage};
    use infrastructure::cache::preview_manager::PreviewManager;

    use super::super::super::cache;
    use super::super::super::fonte;
    use super::super::super::lightroom::mentira::EscolhaDeMentira;
    use super::super::super::persistencia::{self, mentira::GravadorDeMentira};
    use super::super::super::presets::mentira::GuardaDeMentira;
    use super::super::super::processador::Ajustes;
    use super::super::Revelacao;
    use crate::editor::porta::mentira::EdicoesDeMentira;
    use crate::editor::porta::{Edicoes, FotoDoEditor};

    /// Uma foto de 64×48 no disco, com o preview dela no cache — a base.
    fn base_da(i: u32) -> RgbImage {
        RgbImage::from_fn(64, 48, move |x, y| {
            image::Rgb([(x * 3 + i * 20) as u8, (y * 4) as u8, 90])
        })
    }

    struct Palco {
        janela: gpui_kit::WindowHandle<Revelacao>,
        previews: Arc<PreviewManager>,
        edicoes: Arc<EdicoesDeMentira>,
        acervo: Vec<PhotoViewModel>,
        _dir: tempfile::TempDir,
    }

    fn palco(cx: &mut TestAppContext) -> Palco {
        let dir = tempfile::tempdir().unwrap();
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().join("cache")));
        let acervo: Vec<PhotoViewModel> = (0..4)
            .map(|i| {
                let caminho = dir.path().join(format!("f{i}.png"));
                base_da(i).save(&caminho).unwrap();
                let id = format!("id-{i}");
                let imagem = DynamicImage::ImageRgb8(base_da(i));
                previews.save_preview(&id, &imagem).unwrap();
                previews.save_thumbnail(&id, &imagem).unwrap();
                PhotoViewModel {
                    id,
                    name: format!("f{i}.png"),
                    path: caminho.to_string_lossy().into_owned(),
                    ..Default::default()
                }
            })
            .collect();
        let edicoes = Arc::new(EdicoesDeMentira::default());
        cx.update(gpui_kit::init);
        let janela = cx.add_window({
            let previews = previews.clone();
            move |window, cx| {
                Revelacao::nova(
                    previews,
                    Arc::new(GravadorDeMentira::default()),
                    Arc::new(GuardaDeMentira::default()),
                    Arc::new(EscolhaDeMentira::default()),
                    Vec::new(),
                    window,
                    cx,
                )
            }
        });
        let para_abrir = acervo.clone();
        let para_a_tela: Arc<dyn Edicoes> = edicoes.clone();
        janela
            .update(cx, |tela, window, cx| {
                tela.definir_edicoes(para_a_tela);
                tela.abrir_no_acervo(para_abrir, 0, window, cx);
            })
            .unwrap();
        Palco {
            janela,
            previews,
            edicoes,
            acervo,
            _dir: dir,
        }
    }

    /// Pinta um traço vermelho no meio da foto `i` e salva pela porta — o
    /// que a janela do editor faz.
    fn editar_e_salvar(p: &Palco, i: usize) -> RgbImage {
        let foto = &p.acervo[i];
        let base = Arc::new(
            infrastructure::base_neutra::base_neutra(std::path::Path::new(&foto.path))
                .unwrap()
                .to_rgb8(),
        );
        let doc = Documento::novo(BaseRef::da_imagem(&base));
        let mut s = Sessao::nova(base.clone(), doc, Historico::novo(), 64);
        s.pincel.cor = [255, 0, 0];
        s.pincel.raio = 4.0;
        s.pincel.dureza = 1.0;
        s.apertar(10.0, 24.0);
        s.arrastar(54.0, 24.0);
        s.soltar();
        let (doc, hist) = s.instantaneo();
        p.edicoes
            .salvar(&FotoDoEditor::da(foto), &base, &doc, &hist)
            .unwrap()
            .expect("com traço há versão");
        editor_core::composicao::compor(&base, &doc)
    }

    fn origem_rgb(tela: &Revelacao) -> RgbImage {
        let aberta = tela.aberta.as_ref().unwrap();
        let o = aberta.origem.as_ref().unwrap();
        DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(o.largura, o.altura, (*o.pixels).clone()).unwrap(),
        )
        .to_rgb8()
    }

    #[gpui_kit::test]
    fn editar_foto_leva_a_clicada_e_nao_mexe_no_lote(cx: &mut TestAppContext) {
        let p = palco(cx);
        p.janela
            .update(cx, |tela, window, cx| {
                tela.seguir_o_roteiro_da_tira("marcar 1", window, cx);
                tela.seguir_o_roteiro_da_tira("marcar 2", window, cx);
                let lote = tela.marcadas().clone();
                assert_eq!(lote.len(), 3, "0, 1 e 2 marcadas");

                // Fora do lote: só ela.
                tela.pedir_edicao(3, cx);
                assert_eq!(tela.levar_a_editar().map(|f| f.id), Some("id-3".into()));
                // Dentro do lote: ainda só ela, e não o lote.
                tela.pedir_edicao(1, cx);
                assert_eq!(tela.levar_a_editar().map(|f| f.id), Some("id-1".into()));
                assert!(tela.levar_a_editar().is_none(), "levada uma vez só");

                assert_eq!(tela.marcadas(), &lote, "o lote continua o mesmo");
                assert_eq!(tela.posicao(), 0, "a aberta continua a mesma");
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn a_comprada_nao_vai_ao_editor(cx: &mut TestAppContext) {
        let mut comprada = PhotoViewModel {
            path: "/x.jpg".into(),
            ..Default::default()
        };
        assert!(Revelacao::pode_editar(&comprada));
        comprada.comprada = true;
        comprada.revelacao_travada = true;
        assert!(!Revelacao::pode_editar(&comprada));
        let sem_arquivo = PhotoViewModel::default();
        assert!(!Revelacao::pode_editar(&sem_arquivo));
        let _ = cx;
    }

    /// 🔑 **Salvar troca a entrada e mantém a revelação** — sliders, corte,
    /// máscaras e histórico —, e a entrada é a base mais o traço, **sem** a
    /// revelação (nada de efeito duplicado).
    #[gpui_kit::test]
    fn salvar_troca_a_fonte_e_mantem_parametros_corte_mascaras_e_historico(
        cx: &mut TestAppContext,
    ) {
        let p = palco(cx);
        let (ajustes, corte, locais, desfazer, refazer) = p
            .janela
            .update(cx, |tela, _w, cx| {
                // Uma revelação nada neutra aberta.
                tela.aplicar_para_teste(0, 1.2, cx);
                tela.aplicar_para_teste(5, 40.0, cx);
                assert_ne!(tela.ajustes(), Ajustes::default());
                assert_eq!(tela.revisao_da_aberta(), fonte::DO_BRUTO);
                (
                    tela.ajustes(),
                    tela.corte(),
                    tela.locais(),
                    tela.pode_desfazer(),
                    tela.pode_refazer(),
                )
            })
            .unwrap();

        let composta = editar_e_salvar(&p, 0);
        p.janela
            .update(cx, |tela, _w, cx| {
                tela.fonte_mudou("id-0", None, cx);
                assert_eq!(tela.revisao_da_aberta(), 1);
                // A entrada é a imagem editada **inteira e sem revelação**.
                let origem = origem_rgb(tela);
                assert_eq!(origem.as_raw(), composta.as_raw());
                assert_eq!(origem.get_pixel(32, 24).0, [255, 0, 0]);
                assert_eq!(origem.get_pixel(32, 5).0, base_da(0).get_pixel(32, 5).0);
                // A revelação não mudou.
                assert_eq!(tela.ajustes(), ajustes);
                assert_eq!(tela.corte(), corte);
                assert_eq!(tela.locais(), locais);
                assert_eq!(tela.pode_desfazer(), desfazer);
                assert_eq!(tela.pode_refazer(), refazer);
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn salvar_invalida_so_os_caches_daquela_foto(cx: &mut TestAppContext) {
        let p = palco(cx);
        let imagem = DynamicImage::ImageRgb8(RgbImage::new(4, 4));
        for id in ["id-0", "id-1"] {
            p.previews
                .save_preview(&persistencia::chave_da_revelada(id), &imagem)
                .unwrap();
        }
        p.janela
            .update(cx, |tela, _w, cx| {
                let chave = |id: &str| {
                    cache::Chave::nova(
                        id,
                        (64, 48),
                        &Ajustes::default(),
                        &cache::tests_corte(),
                        &Default::default(),
                    )
                };
                tela.reveladas.guardar(chave("id-0"), &imagem);
                tela.reveladas.guardar(chave("id-1"), &imagem);
                editar_e_salvar(&p, 0);
                tela.fonte_mudou("id-0", None, cx);
                assert!(tela.reveladas.buscar(&chave("id-0")).is_none());
                assert!(
                    tela.reveladas.buscar(&chave("id-1")).is_some(),
                    "a outra fica"
                );
            })
            .unwrap();
        assert!(p
            .previews
            .get_preview(&persistencia::chave_da_revelada("id-0"))
            .is_none());
        assert!(p
            .previews
            .get_preview(&persistencia::chave_da_revelada("id-1"))
            .is_some());
    }

    /// A cópia do bruto que chega depois (o download da foto do site) não
    /// passa por cima da edição.
    #[gpui_kit::test]
    fn a_copia_do_bruto_que_chega_nao_apaga_a_edicao(cx: &mut TestAppContext) {
        let p = palco(cx);
        editar_e_salvar(&p, 0);
        p.janela
            .update(cx, |tela, _w, cx| {
                tela.fonte_mudou("id-0", None, cx);
                let usou = tela.receber_pixels("id-0", DynamicImage::ImageRgb8(base_da(0)), cx);
                assert!(!usou);
                assert_eq!(origem_rgb(tela).get_pixel(32, 24).0, [255, 0, 0]);
            })
            .unwrap();
    }

    /// Reabrir a foto (a seta vai e volta) também parte da edição.
    #[gpui_kit::test]
    fn a_foto_editada_abre_da_imagem_editada(cx: &mut TestAppContext) {
        let p = palco(cx);
        let composta = editar_e_salvar(&p, 1);
        p.janela
            .update(cx, |tela, window, cx| {
                tela.ir_para(1, window, cx);
                assert_eq!(tela.revisao_da_aberta(), 1);
                assert_eq!(origem_rgb(tela).as_raw(), composta.as_raw());
                tela.ir_para(2, window, cx);
                assert_eq!(tela.revisao_da_aberta(), fonte::DO_BRUTO);
            })
            .unwrap();
    }

    /// 🔑 **Excluir a edição volta ao bruto e deixa a revelação.** Edição e
    /// revelação são coisas diferentes: tirar os pixels pintados não mexe nos
    /// ajustes, no corte nem nas máscaras.
    #[gpui_kit::test]
    fn excluir_a_edicao_volta_ao_bruto_e_mantem_os_parametros(cx: &mut TestAppContext) {
        let p = palco(cx);
        editar_e_salvar(&p, 0);
        let ajustes = p
            .janela
            .update(cx, |tela, _w, cx| {
                tela.aplicar_para_teste(0, 1.2, cx);
                tela.fonte_mudou("id-0", None, cx);
                assert_eq!(tela.revisao_da_aberta(), 1);
                assert!(tela.tem_edicao(&p.acervo[0]), "o selo acende");
                assert!(!tela.tem_edicao(&p.acervo[1]), "só na editada");
                assert!(tela.tem_projeto_no_editor(&p.acervo[0]));
                tela.ajustes()
            })
            .unwrap();
        assert!(p.edicoes.excluir(&FotoDoEditor::da(&p.acervo[0])).unwrap());
        p.janela
            .update(cx, |tela, _w, cx| {
                tela.fonte_mudou("id-0", None, cx);
                assert_eq!(tela.revisao_da_aberta(), fonte::DO_BRUTO);
                // A prévia do bruto (JPEG, no cache), como antes da edição.
                let bruto = p.previews.get_preview("id-0").unwrap().to_rgb8();
                assert_eq!(
                    origem_rgb(tela).as_raw(),
                    bruto.as_raw(),
                    "de volta ao bruto"
                );
                assert_eq!(tela.ajustes(), ajustes, "a revelação ficou");
                assert!(!tela.tem_edicao(&p.acervo[0]));
                assert!(!tela.tem_projeto_no_editor(&p.acervo[0]));
            })
            .unwrap();
    }

    /// 🔑 **Sem edição e sem revelação, aparece o arquivo bruto** (contrato
    /// "revelar e editar"): excluir a edição e zerar a revelação deixam no
    /// palco exatamente a prévia do bruto.
    #[gpui_kit::test]
    fn sem_edicao_e_sem_revelacao_aparece_o_bruto(cx: &mut TestAppContext) {
        let p = palco(cx);
        editar_e_salvar(&p, 0);
        p.janela
            .update(cx, |tela, window, cx| {
                tela.aplicar_para_teste(0, 1.2, cx);
                tela.fonte_mudou("id-0", None, cx);
                assert_eq!(tela.revisao_da_aberta(), 1);
                assert!(p.edicoes.excluir(&FotoDoEditor::da(&p.acervo[0])).unwrap());
                tela.fonte_mudou("id-0", None, cx);
                tela.redefinir_ajustes(window, cx);
                // Zerar põe no processo do Lightroom (`redefinir_ajustes`), que
                // no neutro dá a mesma foto: o resto é o neutro.
                assert!(tela.ajustes().sem_efeito(), "a revelação zerou");
                assert_eq!(tela.ajustes().processo, 1.0);
                // O que o motor recebe: a entrada é o bruto, e os parâmetros
                // estão todos no neutro — o que sai dele é o próprio bruto.
                assert!(tela.sem_revelacao(), "nenhum parâmetro da revelação");
                assert!(!tela.enquadrada(), "sem corte");
                assert_eq!(tela.revisao_da_aberta(), fonte::DO_BRUTO, "nenhuma edição");
                let bruto = p.previews.get_preview("id-0").unwrap().to_rgb8();
                assert_eq!(
                    origem_rgb(tela).as_raw(),
                    bruto.as_raw(),
                    "a entrada é o arquivo bruto"
                );
            })
            .unwrap();
    }

    /// C31: uma versão cuja proporção não bate com a do bruto é recusada — a
    /// Revelação volta ao bruto, e corte e máscaras não vão para o lugar errado.
    #[gpui_kit::test]
    fn versao_com_outra_proporcao_e_recusada(cx: &mut TestAppContext) {
        let p = palco(cx);
        // Uma "edição" feita sobre uma base de outra proporção (48×64).
        let deitada = Arc::new(RgbImage::from_pixel(48, 64, image::Rgb([9, 9, 9])));
        let doc = Documento::novo(BaseRef::da_imagem(&deitada));
        let mut s = Sessao::nova(deitada.clone(), doc, Historico::novo(), 64);
        s.apertar(10.0, 10.0);
        s.soltar();
        let (doc, hist) = s.instantaneo();
        p.edicoes
            .salvar(&FotoDoEditor::da(&p.acervo[0]), &deitada, &doc, &hist)
            .unwrap();
        let copia =
            fonte::copia_de_trabalho(&p.previews, Some(p.edicoes.as_ref()), &p.acervo[0]).unwrap();
        assert_eq!(copia.revisao, fonte::DO_BRUTO);
        assert_eq!((copia.imagem.width(), copia.imagem.height()), (64, 48));
    }
}
