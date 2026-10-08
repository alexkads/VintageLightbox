//! A raiz e o editor em camadas: abrir a janela da foto clicada e levar à
//! Revelação a versão que ela salvar (`docs/editor-em-camadas/02-CONTRATO.md`).
//!
//! 🔑 **A raiz é a ponte, e a única.** A janela do editor não conhece a
//! Revelação, e a Revelação não conhece a janela: a janela anuncia
//! `EventoDoEditor::Salva`, e daqui sai `Revelacao::fonte_mudou` — mais a foto
//! do site no depósito do "Salvar na galeria", porque a revelada que o cliente
//! vê precisa ser refeita com a edição.

use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use gpui_kit::{AppContext, Context, Entity, Subscription, WeakEntity};

use super::Aplicativo;
use crate::editor::porta::{Edicoes, FotoDoEditor};
use crate::editor::{EditorDeFoto, EventoDoEditor};
use crate::pos_venda::porta::Recado;
use crate::revelacao::persistencia;

/// Quanto a janela espera o bruto do site antes de desistir.
const ESPERA_DO_BRUTO: Duration = Duration::from_secs(180);

/// As janelas do editor abertas, uma por foto.
#[derive(Default)]
pub(super) struct Editores {
    /// 🚨 **Referência fraca.** A janela é dona do editor; guardar a entidade
    /// forte aqui a manteria viva depois de a janela fechar — e o aviso de que
    /// ela fechou (`observe_release`) nunca viria.
    abertas: HashMap<String, (gpui_kit::AnyWindowHandle, WeakEntity<EditorDeFoto>)>,
    assinaturas: HashMap<String, Vec<Subscription>>,
}

impl Aplicativo {
    /// "Editar Foto": a foto que o menu da tira mandou — a clicada.
    pub(super) fn editar_a_foto_pedida(&mut self, cx: &mut Context<Self>) {
        let Some(foto) = self.revelacao.update(cx, |tela, _| tela.levar_a_editar()) else {
            return;
        };
        self.abrir_o_editor(foto, cx);
    }

    /// Abre a janela do editor para `foto` — ou traz para a frente a que já
    /// está aberta para ela.
    pub fn abrir_o_editor(&mut self, foto: PhotoViewModel, cx: &mut Context<Self>) {
        let Some(edicoes) = self.revelacao.read(cx).edicoes() else {
            crate::telemetria::avisar!("⚠️ [Editor] sem catálogo de edições: o editor não abre");
            return;
        };
        let alvo = FotoDoEditor::da(&foto);
        let chave = alvo.chave();
        if let Some((janela, _)) = self.editores.abertas.get(&chave) {
            if janela
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
            self.editores.abertas.remove(&chave);
        }

        let carregar = self.carregador_da_base(&foto);
        let opcoes = gpui_kit::WindowOptions {
            app_id: Some(crate::menu::APP_ID.into()),
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(
                gpui_kit::Bounds::centered(
                    None,
                    gpui_kit::size(gpui_kit::px(1280.), gpui_kit::px(860.)),
                    cx,
                ),
            )),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some(format!("Editar — {}", foto.name).into()),
                ..Default::default()
            }),
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            window_background: gpui_kit::WindowBackgroundAppearance::Opaque,
            window_decorations: crate::janela::decoracoes_ao_abrir(),
            ..Default::default()
        };
        let para_a_janela = alvo.clone();
        let mut entidade = None;
        let aberta = cx.open_window(opcoes, |window, cx| {
            let editor =
                cx.new(|cx| EditorDeFoto::novo(para_a_janela, edicoes, carregar, window, cx));
            entidade = Some(editor.clone());
            cx.new(|cx| gpui_kit::component::Root::new(editor, window, cx))
        });
        let (Ok(janela), Some(editor)) = (aberta, entidade) else {
            crate::telemetria::avisar!("⚠️ [Editor] o sistema não abriu a janela do editor");
            return;
        };
        let fechou = {
            let chave = chave.clone();
            cx.observe_release(&editor, move |raiz, _ed, _cx| {
                raiz.editores.abertas.remove(&chave);
                raiz.editores.assinaturas.remove(&chave);
            })
        };
        let salvou = cx.subscribe(&editor, |raiz, _ed, evento: &EventoDoEditor, cx| {
            raiz.edicao_salva(evento, cx)
        });
        self.editores
            .abertas
            .insert(chave.clone(), (janela.into(), editor.downgrade()));
        self.editores
            .assinaturas
            .insert(chave, vec![fechou, salvou]);
        cx.notify();
    }

    /// Quem carrega a base neutra da foto (C28), no executor de fundo.
    ///
    /// - foto com arquivo neste disco: `base_neutra` do arquivo (LibRaw para
    ///   RAW);
    /// - foto do site: o bruto, pela mesma porta do zoom em resolução cheia —
    ///   reaproveitando o que já está em mãos.
    fn carregador_da_base(
        &mut self,
        foto: &PhotoViewModel,
    ) -> Box<dyn FnOnce() -> Result<image::DynamicImage, String> + Send> {
        if !foto.path.is_empty() {
            let caminho = std::path::PathBuf::from(&foto.path);
            return Box::new(move || infrastructure::base_neutra::base_neutra(&caminho));
        }
        let Some(no_site) = foto.pos_venda_foto_id.clone() else {
            return Box::new(|| Err("a foto não tem arquivo neste computador nem no site".into()));
        };
        if let Some(bytes) = self.bruto_em_maos(&no_site) {
            return Box::new(move || infrastructure::base_neutra::base_neutra_de_bytes(&bytes));
        }
        let Some(sessao) = self.sessao().cloned() else {
            return Box::new(|| Err("entre na sessão do site para editar esta foto".into()));
        };
        let (canal, recebe) = channel();
        self.publicador.original(sessao, no_site, canal);
        Box::new(move || match recebe.recv_timeout(ESPERA_DO_BRUTO) {
            Ok(Recado::Original { bytes, .. }) => {
                infrastructure::base_neutra::base_neutra_de_bytes(&bytes)
            }
            Ok(Recado::OriginalIndisponivel { .. }) => {
                Err("o site não entregou o arquivo original desta foto".into())
            }
            Ok(_) => Err("resposta inesperada do site".into()),
            Err(_) => Err("o arquivo original não chegou do site".into()),
        })
    }

    /// A edição foi salva: a Revelação troca a fonte, e a foto do site vai
    /// para o depósito do "Salvar na galeria".
    fn edicao_salva(&mut self, evento: &EventoDoEditor, cx: &mut Context<Self>) {
        let EventoDoEditor::Salva { foto, versao } = evento;
        crate::telemetria::avisar!(
            "🖌️ [Editor] {} salva ({})",
            foto.nome,
            versao
                .as_ref()
                .map_or("sem efeito — volta ao bruto".to_string(), |v| format!(
                    "revisão {}",
                    v.revisao
                ))
        );
        self.a_fonte_da_foto_mudou(foto, cx);
    }

    /// A entrada da foto mudou (edição salva ou excluída): a Revelação troca a
    /// fonte, e a foto do site vai para o depósito do "Salvar na galeria" — a
    /// revelada que o cliente vê precisa ser refeita.
    fn a_fonte_da_foto_mudou(&mut self, foto: &FotoDoEditor, cx: &mut Context<Self>) {
        self.revelacao.update(cx, |tela, cx| {
            tela.fonte_mudou(&foto.id, foto.pos_venda_foto_id.as_deref(), cx)
        });
        if let Some(no_site) = foto.pos_venda_foto_id.clone() {
            let (ajustes, corte) = self.parametros_para_subir(foto, cx);
            if let Some(ja) = self.a_subir.iter_mut().find(|(id, _, _)| *id == no_site) {
                *ja = (no_site, ajustes, corte);
            } else {
                self.a_subir.push((no_site, ajustes, corte));
            }
            self.recontar_o_que_falta_subir(cx);
        }
        cx.notify();
    }

    /// "Excluir a edição" do menu da tira, já confirmado.
    pub(super) fn excluir_a_edicao_pedida(&mut self, cx: &mut Context<Self>) {
        let Some(foto) = self.revelacao.update(cx, |tela, _| tela.levar_a_excluir()) else {
            return;
        };
        let Some(edicoes) = self.revelacao.read(cx).edicoes() else {
            return;
        };
        let alvo = FotoDoEditor::da(&foto);
        // A janela do editor desta foto fecha sem perguntar: o operador acabou
        // de confirmar que a edição sai.
        if let Some((janela, editor)) = self
            .editores
            .abertas
            .get(&alvo.chave())
            .and_then(|(j, e)| Some((*j, e.upgrade()?)))
        {
            let _ = janela.update(cx, |_, window, cx| {
                editor.update(cx, |ed, cx| ed.descartar_e_fechar(window, cx))
            });
        }
        let para_excluir = alvo.clone();
        let trabalho = cx
            .background_executor()
            .spawn(async move { edicoes.excluir(&para_excluir) });
        cx.spawn(async move |raiz, cx| {
            let resultado = trabalho.await;
            let _ = raiz.update(cx, |raiz, cx| match resultado {
                Ok(_) => {
                    crate::telemetria::avisar!("🖌️ [Editor] edição de {} excluída", alvo.nome);
                    // ✂️ O site deixa de revelar o retoque no próximo envio desta
                    // foto (D23).
                    if let Some(no_site) = &alvo.pos_venda_foto_id {
                        raiz.publicador.edicao_excluida(no_site);
                    }
                    raiz.a_fonte_da_foto_mudou(&alvo, cx);
                    raiz.avisar_em_toast(
                        format!(
                            "Edição de {} excluída — a foto volta ao arquivo bruto",
                            alvo.nome
                        ),
                        false,
                        cx,
                    );
                }
                Err(erro) => raiz.avisar_em_toast(
                    format!("Não foi possível excluir a edição de {}: {erro}", alvo.nome),
                    true,
                    cx,
                ),
            });
        })
        .detach();
    }

    /// A revelação **atual** da foto: a da tela, se ela está no palco; senão a
    /// que a foto guarda.
    fn parametros_para_subir(
        &self,
        foto: &FotoDoEditor,
        cx: &Context<Self>,
    ) -> (
        crate::revelacao::processador::Ajustes,
        domain::value_objects::CropSettings,
    ) {
        let revelacao = self.revelacao.read(cx);
        if revelacao.foto_aberta().is_some_and(|f| {
            f.id == foto.id
                || (f.pos_venda_foto_id.is_some() && f.pos_venda_foto_id == foto.pos_venda_foto_id)
        }) {
            return (revelacao.ajustes(), revelacao.enquadramento());
        }
        match revelacao.acervo().iter().find(|f| f.id == foto.id) {
            Some(f) => (
                persistencia::da_foto(f),
                persistencia::para_crop_settings(&persistencia::corte_da_foto(f)),
            ),
            None => Default::default(),
        }
    }

    /// Um gesto do roteiro de depuração na janela do editor aberta — os
    /// gestos são da janela (`EditorDeFoto::seguir_o_roteiro`), e o `editor
    /// estado` ganha aqui o que a Revelação diz.
    pub(super) fn seguir_o_roteiro_do_editor(
        &mut self,
        gesto: &str,
        pasta: Option<&std::path::Path>,
        cx: &mut Context<Self>,
    ) {
        let Some((janela, editor)) = self.editores_abertos().into_iter().next() else {
            eprintln!("[roteiro] editor: nenhuma janela do editor aberta");
            return;
        };
        if gesto.trim() == "estado" {
            let revelacao = self.revelacao.read(cx);
            eprintln!(
                "[roteiro] revelação: revisão={} marcadas={:?} posição={}",
                revelacao.revisao_da_aberta(),
                revelacao.marcadas(),
                revelacao.posicao(),
            );
        }
        let pasta = pasta.map(|p| p.to_path_buf());
        let gesto = gesto.to_string();
        let _ = janela.update(cx, |_, window, cx| {
            editor.update(cx, |ed, cx| {
                ed.seguir_o_roteiro(&gesto, pasta.as_deref(), window, cx)
            })
        });
    }

    /// As janelas do editor abertas (testes e roteiro).
    pub fn editores_abertos(&self) -> Vec<(gpui_kit::AnyWindowHandle, Entity<EditorDeFoto>)> {
        self.editores
            .abertas
            .values()
            .filter_map(|(janela, e)| Some((*janela, e.upgrade()?)))
            .collect()
    }

    /// A porta das edições, para quem monta o app.
    pub fn definir_edicoes(&mut self, edicoes: Arc<dyn Edicoes>, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _| tela.definir_edicoes(edicoes));
    }
}

#[cfg(test)]
mod testes {
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use adapters::view_models::PhotoViewModel;
    use gpui_kit::{AppContext, Focusable, TestAppContext, VisualTestContext};
    use image::{DynamicImage, RgbImage};
    use infrastructure::cache::preview_manager::PreviewManager;

    use super::super::testes::portas;
    use super::super::Aplicativo;
    use crate::editor::porta::mentira::EdicoesDeMentira;
    use crate::editor::EditorDeFoto;

    fn base_da(i: u32) -> RgbImage {
        RgbImage::from_fn(64, 48, move |x, y| {
            image::Rgb([(x * 3 + i * 20) as u8, (y * 4) as u8, 90])
        })
    }

    struct Montado {
        janela: gpui_kit::WindowHandle<Aplicativo>,
        edicoes: Arc<EdicoesDeMentira>,
        _dir: tempfile::TempDir,
    }

    /// Quatro fotos no disco, a Revelação aberta na primeira, a tira desenhada.
    fn montar(cx: &mut TestAppContext) -> (Montado, VisualTestContext) {
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
        cx.update(super::super::init);
        let janela = cx.add_window({
            let previews = previews.clone();
            move |window, cx| {
                Aplicativo::ja_dentro(acervo, previews, Vec::new(), portas(), window, cx)
            }
        });
        let para_o_app = edicoes.clone();
        janela
            .update(cx, |app, window, cx| {
                app.definir_edicoes(para_o_app, cx);
                app.biblioteca
                    .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
                app.revelar(window, cx);
            })
            .unwrap();
        let visual = VisualTestContext::from_window(janela.into(), cx);
        visual.run_until_parked();
        (
            Montado {
                janela,
                edicoes,
                _dir: dir,
            },
            visual,
        )
    }

    fn o_editor(
        m: &Montado,
        cx: &mut TestAppContext,
    ) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<EditorDeFoto>) {
        let abertos = m
            .janela
            .update(cx, |app, _w, _cx| app.editores_abertos())
            .unwrap();
        assert_eq!(abertos.len(), 1, "uma janela de editor");
        abertos[0].clone()
    }

    /// 🔑 **O alvo é a foto do botão direito**, mesmo com três marcadas — pelo
    /// mesmo menu que o clique abre, dirigido pelo teclado (↓ ↓ ↓ Enter:
    /// título, "Abrir esta foto", "Editar Foto"). Lote, posição e as outras
    /// opções do menu ficam onde estavam.
    #[gpui_kit::test]
    fn editar_foto_pelo_menu_abre_a_janela_da_clicada(cx: &mut TestAppContext) {
        let (m, mut visual) = montar(cx);
        let janelas_antes = cx.windows().len();
        m.janela
            .update(cx, |app, window, cx| {
                app.revelacao.update(cx, |tela, cx| {
                    tela.seguir_o_roteiro_da_tira("marcar 1", window, cx);
                    tela.seguir_o_roteiro_da_tira("marcar 2", window, cx);
                    tela.seguir_o_roteiro_da_tira("menu 3", window, cx);
                });
                let menu = app
                    .revelacao
                    .read(cx)
                    .menu_do_roteiro()
                    .expect("o menu da miniatura 3");
                window.focus(&menu.focus_handle(cx), cx);
            })
            .unwrap();
        visual.run_until_parked();
        visual.simulate_keystrokes("down down down enter");
        visual.run_until_parked();

        let (_, editor) = o_editor(&m, cx);
        assert_eq!(editor.read_with(cx, |ed, _| ed.foto().id.clone()), "id-3");
        assert_eq!(cx.windows().len(), janelas_antes + 1);
        m.janela
            .update(cx, |app, _w, cx| {
                let tela = app.revelacao.read(cx);
                assert_eq!(
                    tela.marcadas().iter().copied().collect::<Vec<_>>(),
                    vec![0, 1, 2]
                );
                assert_eq!(tela.posicao(), 0);
            })
            .unwrap();

        // E o grupo da escolha vem depois de "Baixar como…": ↓ ×5 é
        // "Escolher também".
        m.janela
            .update(cx, |app, window, cx| {
                app.revelacao.update(cx, |tela, cx| {
                    tela.seguir_o_roteiro_da_tira("menu 3", window, cx)
                });
                let menu = app.revelacao.read(cx).menu_do_roteiro().unwrap();
                window.focus(&menu.focus_handle(cx), cx);
            })
            .unwrap();
        visual.run_until_parked();
        visual.simulate_keystrokes("down down down down down enter");
        visual.run_until_parked();
        m.janela
            .update(cx, |app, _w, cx| {
                let marcadas: Vec<usize> =
                    app.revelacao.read(cx).marcadas().iter().copied().collect();
                assert_eq!(
                    marcadas,
                    vec![0, 1, 2, 3],
                    "\"Escolher também\" pôs a 3 no lote"
                );
            })
            .unwrap();
    }

    /// Pedir de novo a mesma foto traz a janela que já está aberta.
    #[gpui_kit::test]
    fn a_mesma_foto_nao_abre_duas_janelas(cx: &mut TestAppContext) {
        let (m, visual) = montar(cx);
        let antes = cx.windows().len();
        for _ in 0..2 {
            m.janela
                .update(cx, |app, _w, cx| {
                    let foto = app.revelacao.read(cx).acervo()[2].clone();
                    app.abrir_o_editor(foto, cx);
                })
                .unwrap();
            visual.run_until_parked();
        }
        assert_eq!(cx.windows().len(), antes + 1);
    }

    /// 🚨 **Abrir o editor não tira as setas da Revelação**: a janela
    /// principal continua com o foco dela.
    #[gpui_kit::test]
    fn as_setas_andam_depois_de_abrir_o_editor(cx: &mut TestAppContext) {
        let (m, mut visual) = montar(cx);
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(2, cx));
            })
            .unwrap();
        visual.run_until_parked();
        o_editor(&m, cx);
        visual.simulate_keystrokes("right");
        visual.run_until_parked();
        let posicao = m
            .janela
            .update(cx, |app, _w, cx| app.revelacao.read(cx).posicao())
            .unwrap();
        assert_eq!(posicao, 1, "a seta andou na Revelação");
    }

    /// Pintar, salvar, e a Revelação usar a versão salva — e a falha de
    /// gravação deixando a anterior de pé e as alterações marcadas.
    #[gpui_kit::test]
    fn pintar_salvar_e_a_revelacao_usar_a_versao(cx: &mut TestAppContext) {
        let (m, visual) = montar(cx);
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(0, cx));
            })
            .unwrap();
        visual.run_until_parked();
        let (janela_do_editor, editor) = o_editor(&m, cx);
        assert!(editor.read_with(cx, |ed, _| ed.pronta()), "a base carregou");
        assert!(!editor.read_with(cx, |ed, _| ed.alterado()));

        editor.update(cx, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        assert!(editor.read_with(cx, |ed, _| ed.alterado()));
        assert!(editor.read_with(cx, |ed, _| ed.titulo().ends_with('•')));

        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.salvar(false, window, cx))
        })
        .unwrap();
        visual.run_until_parked();
        assert_eq!(m.edicoes.salvamentos.load(Ordering::SeqCst), 1);
        assert!(!editor.read_with(cx, |ed, _| ed.alterado()), "salvo");
        let revisao = m
            .janela
            .update(cx, |app, _w, cx| app.revelacao.read(cx).revisao_da_aberta())
            .unwrap();
        assert_eq!(revisao, 1, "a Revelação passou a usar a revisão 1");

        // O disco falha na segunda: a Revelação continua na 1, o editor avisa
        // e continua "alterado".
        editor.update(cx, |ed, cx| {
            ed.tracar_para_teste((10., 10.), (54., 10.), cx)
        });
        m.edicoes.falhar.store(true, Ordering::SeqCst);
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.salvar(false, window, cx))
        })
        .unwrap();
        visual.run_until_parked();
        let (aviso, erro) = editor.read_with(cx, |ed, _| {
            let (t, e) = ed.aviso().unwrap();
            (t.to_string(), e)
        });
        assert!(erro, "{aviso}");
        assert!(
            aviso.contains("versão anterior continua valendo"),
            "{aviso}"
        );
        assert!(editor.read_with(cx, |ed, _| ed.alterado()));
        let revisao = m
            .janela
            .update(cx, |app, _w, cx| app.revelacao.read(cx).revisao_da_aberta())
            .unwrap();
        assert_eq!(revisao, 1);
    }

    /// "Excluir a edição" pela tira: a janela do editor daquela foto fecha, o
    /// projeto sai e a Revelação volta ao bruto.
    #[gpui_kit::test]
    fn excluir_a_edicao_fecha_o_editor_e_volta_ao_bruto(cx: &mut TestAppContext) {
        let (m, visual) = montar(cx);
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(0, cx));
            })
            .unwrap();
        visual.run_until_parked();
        let (janela_do_editor, editor) = o_editor(&m, cx);
        editor.update(cx, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.salvar(false, window, cx))
        })
        .unwrap();
        visual.run_until_parked();
        drop(editor);
        let revisao = m
            .janela
            .update(cx, |app, _w, cx| app.revelacao.read(cx).revisao_da_aberta())
            .unwrap();
        assert_eq!(revisao, 1);

        m.janela
            .update(cx, |app, window, cx| {
                app.revelacao.update(cx, |tela, cx| {
                    tela.seguir_o_roteiro_da_tira("excluir 0", window, cx)
                });
            })
            .unwrap();
        visual.run_until_parked();
        let (abertos, revisao) = m
            .janela
            .update(cx, |app, _w, cx| {
                (
                    app.editores_abertos().len(),
                    app.revelacao.read(cx).revisao_da_aberta(),
                )
            })
            .unwrap();
        assert_eq!(abertos, 0, "a janela do editor fechou");
        assert_eq!(revisao, 0, "a Revelação voltou ao bruto");
        use crate::editor::porta::Edicoes;
        assert!(!m.edicoes.tem_projeto("id-0", None));
    }

    /// Fechar com alterações pergunta; "Descartar" fecha e a raiz esquece a
    /// janela. Reabrir traz o que foi **salvo**.
    #[gpui_kit::test]
    fn fechar_com_alteracoes_pergunta_e_reabrir_traz_o_salvo(cx: &mut TestAppContext) {
        let (m, visual) = montar(cx);
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(1, cx));
            })
            .unwrap();
        visual.run_until_parked();
        let (janela_do_editor, editor) = o_editor(&m, cx);
        editor.update(cx, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.salvar(false, window, cx))
        })
        .unwrap();
        visual.run_until_parked();
        let salvo = editor.read_with(cx, |ed, _| ed.sessao().unwrap().documento().clone());

        // Mais um traço, sem salvar, e o Fechar pergunta.
        editor.update(cx, |ed, cx| ed.tracar_para_teste((5., 5.), (60., 40.), cx));
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.fechar(window, cx))
        })
        .unwrap();
        assert!(editor.read_with(cx, |ed, _| ed.perguntando()));
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.descartar_e_fechar(window, cx))
        })
        .unwrap();
        drop(editor);
        visual.run_until_parked();
        let abertos = m
            .janela
            .update(cx, |app, _w, _cx| app.editores_abertos().len())
            .unwrap();
        assert_eq!(abertos, 0, "a raiz esqueceu a janela fechada");

        // Reabrir: o projeto salvo, intacto — sem o traço descartado.
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(1, cx));
            })
            .unwrap();
        visual.run_until_parked();
        let (_, reaberto) = o_editor(&m, cx);
        let doc = reaberto.read_with(cx, |ed, _| ed.sessao().unwrap().documento().clone());
        assert_eq!(doc, salvo);
        assert!(!reaberto.read_with(cx, |ed, _| ed.alterado()));
        assert!(reaberto.read_with(cx, |ed, _| ed.sessao().unwrap().historico().pode_desfazer()));
    }

    /// Abre o editor da primeira foto e devolve a janela dele com o contexto
    /// visual — onde as teclas e os cliques do editor acontecem.
    fn editor_aberto(
        cx: &mut TestAppContext,
    ) -> (Montado, gpui_kit::Entity<EditorDeFoto>, VisualTestContext) {
        let (m, visual) = montar(cx);
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(0, cx));
            })
            .unwrap();
        visual.run_until_parked();
        let (janela_do_editor, editor) = o_editor(&m, cx);
        let mut ve = VisualTestContext::from_window(janela_do_editor, cx);
        ve.run_until_parked();
        // 🧪 O primeiro evento de ponteiro de uma janela do harness se perde.
        ve.simulate_mouse_move(
            gpui_kit::point(gpui_kit::px(1.), gpui_kit::px(1.)),
            None,
            gpui_kit::Modifiers::none(),
        );
        assert!(editor.read_with(&ve, |ed, _| ed.pronta()));
        (m, editor, ve)
    }

    fn clicar_no_editor(ve: &mut VisualTestContext, alvo: &'static str) {
        let caixa = ve
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{alvo} não está desenhado"));
        ve.simulate_click(caixa.center(), gpui_kit::Modifiers::none());
        ve.run_until_parked();
    }

    /// Um item de um menu do editor (Arquivo, Editar…): abre o menu e clica.
    fn pelo_menu(ve: &mut VisualTestContext, menu: &'static str, item: &'static str) {
        clicar_no_editor(ve, menu);
        clicar_no_editor(ve, item);
    }

    /// Uma ferramenta escondida no grupo da barra: o botão direito no grupo
    /// abre o flyout, e o clique na linha a escolhe.
    fn pelo_flyout(ve: &mut VisualTestContext, ferramenta: &'static str) {
        let d = crate::editor::janela::FERRAMENTAS
            .iter()
            .find(|d| d.id == ferramenta)
            .unwrap_or_else(|| panic!("{ferramenta} não está na tabela"));
        let grupo: &'static str = Box::leak(format!("editor-grupo-{}", d.grupo).into_boxed_str());
        let caixa = ve.debug_bounds(grupo).expect("o grupo na barra");
        ve.simulate_mouse_down(
            caixa.center(),
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        ve.simulate_mouse_up(
            caixa.center(),
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        let linha: &'static str = Box::leak(format!("editor-flyout-{ferramenta}").into_boxed_str());
        clicar_no_editor(ve, linha);
    }

    /// 🪟 No GNOME o sistema não decora a janela do editor: a barra dele tem
    /// minimizar, maximizar e fechar no alto, à direita (dono, 07/out/2026:
    /// *"a janela do editor de fotos no Fedora 44 Gnome não tem barra"*). E o
    /// fechar dela é o do editor — com alterações, pergunta, e a janela fica.
    #[gpui_kit::test]
    fn no_gnome_a_barra_do_editor_tem_os_botoes_e_o_fechar_pergunta(cx: &mut TestAppContext) {
        crate::janela::teste::forcar_barra_do_app();
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.refresh());
        ve.run_until_parked();
        let largura = ve.update(|window, _| window.viewport_size().width);
        for botao in ["janela-minimizar", "janela-maximizar", "janela-fechar"] {
            let b = ve
                .debug_bounds(botao)
                .unwrap_or_else(|| panic!("{botao} não está na barra do editor"));
            assert!(
                b.bottom() <= gpui_kit::px(48.),
                "{botao} fora do alto ({b:?})"
            );
            assert!(
                b.right() >= largura - gpui_kit::px(140.),
                "{botao} fora do canto direito ({b:?})"
            );
        }

        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        ve.run_until_parked();
        clicar_no_editor(&mut ve, "janela-fechar");
        assert!(
            editor.read_with(&ve, |ed, _| ed.perguntando()),
            "o fechar da barra pergunta antes de descartar"
        );
        assert!(
            editor.read_with(&ve, |ed, _| ed.alterado()),
            "a edição continua lá"
        );
    }

    fn camadas(
        editor: &gpui_kit::Entity<EditorDeFoto>,
        ve: &VisualTestContext,
    ) -> (Vec<String>, usize) {
        editor.read_with(ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (
                s.documento()
                    .camadas
                    .iter()
                    .map(|c| c.nome.clone())
                    .collect(),
                s.ativa(),
            )
        })
    }

    /// 🎨 As camadas pelas teclas do Photoshop e pelos botões do painel, e o
    /// desfazer devolvendo a pilha.
    #[gpui_kit::test]
    fn camadas_pelas_teclas_e_pelo_painel(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        assert_eq!(camadas(&editor, &ve), (vec!["Pintura".to_string()], 0));

        ve.simulate_keystrokes("cmd-shift-n");
        ve.run_until_parked();
        assert_eq!(
            camadas(&editor, &ve),
            (vec!["Pintura".to_string(), "Camada 1".to_string()], 1)
        );
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        let pintadas = editor.read_with(&ve, |ed, _| {
            ed.sessao()
                .unwrap()
                .documento()
                .camadas
                .iter()
                .map(|c| !c.pixels.vazia())
                .collect::<Vec<_>>()
        });
        assert_eq!(pintadas, vec![false, true], "o pincel pintou na escolhida");

        ve.simulate_keystrokes("cmd-j");
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).0[2], "Camada 1 cópia");
        assert_eq!(camadas(&editor, &ve).1, 2);
        ve.simulate_keystrokes("cmd-[");
        ve.run_until_parked();
        assert_eq!(
            camadas(&editor, &ve),
            (
                vec![
                    "Pintura".to_string(),
                    "Camada 1 cópia".to_string(),
                    "Camada 1".to_string()
                ],
                1
            )
        );
        ve.simulate_keystrokes("alt-]");
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).1, 2, "⌥] escolhe a de cima");

        // O olho esconde sem escolher; a linha escolhe.
        clicar_no_editor(&mut ve, "editor-olho-0");
        assert!(
            !editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .visivel)
        );
        assert_eq!(camadas(&editor, &ve).1, 2);
        clicar_no_editor(&mut ve, "editor-camada-0");
        assert_eq!(camadas(&editor, &ve).1, 0);
        assert!(!editor.read_with(&ve, |ed, _| ed.camada_visivel()));

        clicar_no_editor(&mut ve, "editor-camada-excluir");
        assert_eq!(camadas(&editor, &ve).0.len(), 2);
        clicar_no_editor(&mut ve, "editor-camada-nova");
        assert_eq!(camadas(&editor, &ve).0.len(), 3);
        assert_eq!(camadas(&editor, &ve).0[1], "Camada 2", "o número não volta");

        for _ in 0..2 {
            ve.simulate_keystrokes("cmd-z");
        }
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).0[0], "Pintura", "a excluída voltou");
        assert_eq!(camadas(&editor, &ve).0.len(), 3);
    }

    /// ✏️ Renomear pelo duplo clique: as letras vão para o nome, e não viram
    /// atalho (B é o pincel, E a borracha, Z o zoom).
    #[gpui_kit::test]
    fn renomear_a_camada_nao_dispara_as_teclas_soltas(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        let caixa = ve.debug_bounds("editor-camada-0").unwrap();
        ve.simulate_event(gpui_kit::MouseDownEvent {
            button: gpui_kit::MouseButton::Left,
            position: caixa.center(),
            modifiers: gpui_kit::Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        ve.simulate_event(gpui_kit::MouseUpEvent {
            button: gpui_kit::MouseButton::Left,
            position: caixa.center(),
            modifiers: gpui_kit::Modifiers::none(),
            click_count: 2,
        });
        ve.run_until_parked();
        ve.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        ve.run_until_parked();
        let nivel_antes = editor.read_with(&ve, |ed, _| ed.nivel_do_zoom());
        ve.simulate_input("Bez fundo");
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).0[0], "Bez fundo");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Pincel)
        );
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            nivel_antes
        );
        // O foco voltou ao editor: a tecla solta vale de novo.
        ve.simulate_keystrokes("e");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Borracha)
        );
    }

    /// 🔍 O zoom pelas teclas da Revelação, e a roda movendo a foto ampliada.
    #[gpui_kit::test]
    fn zoom_pelas_teclas_e_a_roda(cx: &mut TestAppContext) {
        use crate::revelacao::zoom::Nivel;
        let (_m, editor, mut ve) = editor_aberto(cx);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            Nivel::Encaixar
        );
        let encaixada = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());

        ve.simulate_keystrokes("cmd-=");
        ve.run_until_parked();
        let ampliada = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert!(ampliada.size.width > encaixada.size.width, "⌘= ampliou");

        // A roda move a foto ampliada.
        let palco = ve.debug_bounds("palco-do-editor").unwrap();
        ve.simulate_event(gpui_kit::ScrollWheelEvent {
            position: palco.center(),
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                gpui_kit::px(40.),
                gpui_kit::px(30.),
            )),
            modifiers: gpui_kit::Modifiers::none(),
            touch_phase: gpui_kit::TouchPhase::Moved,
        });
        ve.run_until_parked();
        let movida = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert_ne!(movida.origin, ampliada.origin, "a roda moveu");

        ve.simulate_keystrokes("cmd-0");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            Nivel::Encaixar
        );
        ve.simulate_keystrokes("cmd-alt-0");
        ve.run_until_parked();
        let razao = editor.read_with(&ve, |ed, _| ed.razao_do_zoom().unwrap());
        assert!(
            (razao - 1.0).abs() < 0.01,
            "1:1 é um pixel por pixel ({razao})"
        );
        // Z é a Lupa, como no Photoshop — e não mexe no zoom sozinho.
        ve.simulate_keystrokes("z");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(crate::editor::janela::Auxiliar::Zoom)
        );
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            Nivel::Razao(1.0),
            "a tecla escolhe a ferramenta, o zoom fica"
        );
    }

    /// 🖐️ Espaço segurado + arrastar move a foto ampliada e não pinta.
    #[gpui_kit::test]
    fn o_espaco_segurado_e_a_mao(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        for _ in 0..3 {
            ve.simulate_keystrokes("cmd-=");
        }
        ve.run_until_parked();
        let antes = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        // A janela do editor na frente: sem foco, o editor larga o Espaço (a
        // tecla solta não chegaria).
        ve.update(|window, _| window.activate_window());
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| ed.espaco_apertado(cx));
        assert!(editor.read_with(&ve, |ed, _| ed.espaco_segurado()));
        let palco = ve.debug_bounds("palco-do-editor").unwrap();
        let inicio = palco.center();
        let fim = inicio + gpui_kit::point(gpui_kit::px(-60.), gpui_kit::px(-40.));
        ve.simulate_mouse_down(
            inicio,
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        ve.simulate_mouse_move(
            fim,
            Some(gpui_kit::MouseButton::Left),
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        ve.simulate_mouse_up(
            fim,
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| ed.espaco_solto(cx));
        let depois = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert_ne!(depois.origin, antes.origin, "a mão moveu a foto");
        assert!(
            !editor.read_with(&ve, |ed, _| ed.alterado()),
            "e não pintou nada"
        );
    }

    /// 🧰 A barra de ferramentas à esquerda: cada botão escolhe a ferramenta e
    /// fica aceso; a lupa amplia com um clique e afasta com ⌥ + clique; a mão
    /// arrasta a foto ampliada sem pintar.
    #[gpui_kit::test]
    fn a_barra_de_ferramentas_escolhe_e_a_lupa_e_a_mao_funcionam(cx: &mut TestAppContext) {
        use crate::editor::janela::{Auxiliar, TipoDeSelecao};
        use editor_core::Ferramenta;
        let (_m, editor, mut ve) = editor_aberto(cx);
        let barra = ve.debug_bounds("editor-barra-de-ferramentas").unwrap();
        let palco = ve.debug_bounds("palco-do-editor").unwrap();
        assert!(
            barra.origin.x < palco.origin.x && barra.size.width < gpui_kit::px(60.),
            "a barra é estreita e fica à esquerda do palco"
        );

        clicar_no_editor(&mut ve, "editor-borracha");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Borracha)
        );
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nome_da_ferramenta()),
            "Borracha (E)"
        );
        clicar_no_editor(&mut ve, "editor-selecao-laco");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.selecionando()),
            Some(TipoDeSelecao::Laco)
        );
        clicar_no_editor(&mut ve, "editor-subexposicao");
        assert!(matches!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Subexposicao(_))
        ));
        assert_eq!(editor.read_with(&ve, |ed, _| ed.selecionando()), None);
        assert!(
            ve.debug_bounds("editor-faixa").is_some(),
            "a faixa aparece nas opções"
        );
        clicar_no_editor(&mut ve, "editor-pincel");
        assert!(ve.debug_bounds("editor-faixa").is_none());

        // A lupa: clique amplia em torno do ponto, ⌥ + clique afasta.
        clicar_no_editor(&mut ve, "editor-lupa");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(Auxiliar::Zoom)
        );
        let encaixada = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        let alvo = ponto_do_palco(&mut ve, (0.5, 0.5));
        ve.simulate_click(alvo, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let ampliada = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert!(
            ampliada.size.width > encaixada.size.width * 1.5,
            "o clique ampliou"
        );
        ve.simulate_click(alvo, gpui_kit::Modifiers::alt());
        ve.run_until_parked();
        let afastada = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert!(
            afastada.size.width < ampliada.size.width,
            "⌥ + clique afastou"
        );
        for _ in 0..2 {
            ve.simulate_click(alvo, gpui_kit::Modifiers::none());
        }
        ve.run_until_parked();

        // A mão: arrastar move a foto ampliada e não pinta.
        clicar_no_editor(&mut ve, "editor-mao");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(Auxiliar::Mao)
        );
        assert!(
            ve.debug_bounds("editor-tamanho").is_none(),
            "a mão não tem tamanho de pincel"
        );
        let antes = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        arrastar_no_palco(&mut ve, (0.5, 0.5), (0.3, 0.35));
        let depois = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        assert_ne!(depois.origin, antes.origin, "a mão moveu a foto");
        assert!(
            !editor.read_with(&ve, |ed, _| ed.alterado()),
            "e não pintou nada"
        );

        // A tecla da ferramenta volta ao pincel e apaga a mão da barra.
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("b");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.auxiliar()), None);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Pincel)
        );
    }

    /// 🎭 A máscara de camada pela tela: o botão (e com ⌥), as miniaturas que
    /// escolhem onde o pincel pinta, ⇧ + clique que desliga, o degradê (G) e a
    /// lata (⇧G) nela, e a lixeira que a exclui.
    #[gpui_kit::test]
    fn a_mascara_de_camada_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        // Uma camada vermelha cheia por cima da foto.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.cor = [255, 0, 0];
                s.selecionar_tudo();
                s.preencher_selecao();
                s.desmarcar();
            })
        });
        let cor = |ve: &mut VisualTestContext, x: f32, y: f32| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().cor_em(x, y).unwrap())
        };
        let mascara = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().documento().camadas[0]
                    .mascara
                    .as_ref()
                    .map(|m| (m.fundo, m.ativa))
            })
        };
        let na_mascara = |ve: &mut VisualTestContext| editor.read_with(ve, |ed, _| ed.na_mascara());

        clicar_no_editor(&mut ve, "editor-camada-mascara");
        assert_eq!(mascara(&mut ve), Some((255, true)));
        assert!(na_mascara(&mut ve), "o pincel vai para a máscara nova");
        assert!(
            ve.debug_bounds("editor-na-mascara").is_some(),
            "o painel avisa"
        );
        assert!(
            ve.debug_bounds("editor-mascara-0").is_some(),
            "a miniatura aparece"
        );

        // A miniatura da camada (a linha) volta aos pixels; a da máscara, a ela.
        clicar_no_editor(&mut ve, "editor-miniatura-0");
        assert!(!na_mascara(&mut ve));
        clicar_no_editor(&mut ve, "editor-mascara-0");
        assert!(na_mascara(&mut ve));

        // G e um arrasto da esquerda para a direita: preto → branco na máscara.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.pincel.cor = [0; 3])
        });
        ve.simulate_keystrokes("g");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(crate::editor::janela::Auxiliar::Degrade)
        );
        let base = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().base().clone());
        // Em frações da foto (o palco é mais largo que ela).
        let foto = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        let na_foto = |fx: f32, fy: f32| {
            foto.origin + gpui_kit::point(foto.size.width * fx, foto.size.height * fy)
        };
        let (de, ate) = (na_foto(0.1, 0.5), na_foto(0.9, 0.5));
        ve.simulate_mouse_down(de, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
        ve.simulate_mouse_move(
            ate,
            Some(gpui_kit::MouseButton::Left),
            gpui_kit::Modifiers::none(),
        );
        ve.simulate_mouse_up(
            ate,
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        let (l, a) = (base.width() as f32, base.height() as f32);
        assert_eq!(
            cor(&mut ve, 1.0, a / 2.0),
            base.get_pixel(1, (a / 2.0) as u32).0,
            "a esquerda some"
        );
        assert_eq!(
            cor(&mut ve, l - 1.0, a / 2.0),
            [255, 0, 0],
            "a direita fica"
        );
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed
                .sessao()
                .unwrap()
                .historico()
                .a_desfazer()
                .map(|p| p.descricao(ed.sessao().unwrap().documento()))),
            Some("Pincel na máscara".to_string())
        );

        // ⇧ + clique na miniatura desliga: a camada inteira de novo.
        let miniatura = ve.debug_bounds("editor-mascara-0").unwrap();
        ve.simulate_click(miniatura.center(), gpui_kit::Modifiers::shift());
        ve.run_until_parked();
        assert_eq!(mascara(&mut ve), Some((255, false)));
        assert_eq!(cor(&mut ve, 1.0, a / 2.0), [255, 0, 0]);
        ve.simulate_click(miniatura.center(), gpui_kit::Modifiers::shift());
        ve.run_until_parked();
        assert_eq!(mascara(&mut ve), Some((255, true)));

        // ⇧G e um clique na parte preta: a lata (com branco) revela de volta.
        ve.simulate_keystrokes("shift-g");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.pincel.cor = [255; 3])
        });
        let ponto = na_foto(0.02, 0.5);
        ve.simulate_click(ponto, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        assert_eq!(cor(&mut ve, 1.0, a / 2.0), [255, 0, 0], "a lata revelou");

        // A lixeira com a máscara escolhida exclui a máscara, não a camada.
        clicar_no_editor(&mut ve, "editor-mascara-0");
        clicar_no_editor(&mut ve, "editor-camada-excluir");
        assert_eq!(mascara(&mut ve), None);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas.len()),
            1
        );

        // ⌥ + o botão: uma máscara que esconde tudo.
        let botao = ve.debug_bounds("editor-camada-mascara").unwrap();
        ve.simulate_click(botao.center(), gpui_kit::Modifiers::alt());
        ve.run_until_parked();
        assert_eq!(mascara(&mut ve), Some((0, true)));
        assert_eq!(
            cor(&mut ve, l / 2.0, a / 2.0),
            base.get_pixel((l / 2.0) as u32, (a / 2.0) as u32).0
        );
    }

    /// 🎚️ A camada de ajuste pela tela: o menu do rodapé, as Propriedades com
    /// os sliders, um passo só por arrasto, o ícone na linha e o ⌘E.
    #[gpui_kit::test]
    fn a_camada_de_ajuste_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.cor = [200, 100, 50];
                s.selecionar_tudo();
                s.preencher_selecao();
                s.desmarcar();
            })
        });
        assert!(ve.debug_bounds("editor-propriedades").is_none());
        clicar_no_editor(&mut ve, "editor-camada-ajuste");
        clicar_no_editor(&mut ve, "editor-ajuste-novo-niveis");
        let (nomes, ativa) = camadas(&editor, &ve);
        assert_eq!(nomes, vec!["Pintura".to_string(), "Níveis 1".to_string()]);
        assert_eq!(ativa, 1);
        assert!(
            ve.debug_bounds("editor-propriedades").is_some(),
            "as Propriedades aparecem"
        );
        assert!(ve.debug_bounds("editor-ajuste-branco").is_some());
        assert!(
            ve.debug_bounds("editor-ajuste-brilho").is_none(),
            "só os do ajuste escolhido"
        );

        // Um arrasto do slider do branco: a foto muda a cada valor, e o
        // desfazer volta tudo de uma vez.
        let passos = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().passos().len());
        for (v, soltou) in [(230.0, false), (210.0, false), (200.0, true)] {
            editor.update(&mut ve, |ed, cx| {
                ed.mover_parametro_do_ajuste(4, v, soltou, cx)
            });
        }
        ve.run_until_parked();
        let cor = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().cor_em(5.0, 5.0).unwrap())
        };
        assert_eq!(cor(&mut ve), [255, 128, 64], "200 vira o branco");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().passos().len()),
            passos + 1
        );
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(cor(&mut ve), [200, 100, 50]);
        ve.simulate_keystrokes("cmd-shift-z");
        ve.run_until_parked();
        assert_eq!(cor(&mut ve), [255, 128, 64]);

        // ⌘E: o ajuste entra nos pixels da Pintura.
        ve.simulate_keystrokes("cmd-e");
        ve.run_until_parked();
        let (nomes, _) = camadas(&editor, &ve);
        assert_eq!(nomes, vec!["Pintura".to_string()]);
        assert_eq!(cor(&mut ve), [255, 128, 64]);
        assert!(ve.debug_bounds("editor-propriedades").is_none());
    }

    /// 🪄 A varinha (W) e os modificadores, a tolerância e o contíguo nas
    /// opções, o ⌘ + clique na miniatura, e Difundir/Expandir/Contrair.
    #[gpui_kit::test]
    fn a_varinha_e_os_comandos_de_selecao_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        // Um retângulo azul pintado na camada: a varinha o pega inteiro.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(10, 10, 20, 20)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [0, 0, 255];
                s.preencher_selecao();
                s.desmarcar();
            })
        });
        ve.simulate_keystrokes("w");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(crate::editor::janela::Auxiliar::Varinha)
        );
        // Sem antisserrilhado: o ⌥ abaixo tira tudo, sem a rampa da borda.
        editor.update(&mut ve, |ed, _| ed.opcoes_da_varinha_mut().suavizar = false);
        assert!(ve.debug_bounds("editor-tolerancia").is_some());
        assert!(ve.debug_bounds("editor-contigua").is_some());
        let foto = editor.read_with(&ve, |ed, _| ed.area_na_janela().unwrap());
        let (l, a) = editor.read_with(&ve, |ed, _| {
            let b = ed.sessao().unwrap().base();
            (b.width() as f32, b.height() as f32)
        });
        let na_foto = |x: f32, y: f32| {
            foto.origin + gpui_kit::point(foto.size.width * (x / l), foto.size.height * (y / a))
        };
        ve.simulate_click(na_foto(20.0, 20.0), gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let limites = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().selecao().map(|s| s.limites())
            })
        };
        let valor = |ve: &mut VisualTestContext, x: u32, y: u32| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().selecao().map_or(0, |s| s.valor(x, y))
            })
        };
        assert_eq!((valor(&mut ve, 15, 15), valor(&mut ve, 5, 5)), (255, 0));
        assert!(limites(&mut ve).is_some());
        // ⌥ + clique no mesmo azul tira tudo.
        ve.simulate_click(na_foto(20.0, 20.0), gpui_kit::Modifiers::alt());
        ve.run_until_parked();
        assert!(limites(&mut ve).is_none());

        // ⌘ + clique na miniatura: o alfa da camada, sem trocar a escolhida.
        let miniatura = ve.debug_bounds("editor-miniatura-0").unwrap();
        ve.simulate_click(miniatura.center(), gpui_kit::Modifiers::secondary_key());
        ve.run_until_parked();
        assert_eq!((valor(&mut ve, 15, 15), valor(&mut ve, 5, 5)), (255, 0));

        // "Modificar seleção ▾": o menu tem os três comandos e, separado,
        // Transformar seleção; cada comando pede o valor em pixels.
        use crate::editor::janela::Modificacao;
        clicar_no_editor(&mut ve, "editor-modificar-selecao");
        for item in [
            "editor-modificar-difundir",
            "editor-modificar-expandir",
            "editor-modificar-contrair",
            "editor-transformar-selecao",
        ] {
            assert!(ve.debug_bounds(item).is_some(), "{item} no menu");
        }
        clicar_no_editor(&mut ve, "editor-modificar-expandir");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.modificando()),
            Some(Modificacao::Expandir)
        );
        assert!(ve.debug_bounds("editor-valor-da-modificacao").is_some());
        assert!(ve.debug_bounds("editor-confirmar-modificacao").is_some());
        // O foco está no campo: Enter confirma (o OK faz o mesmo).
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert_eq!(valor(&mut ve, 6, 15), 255, "expandiu 5 px");
        ve.update(|window, cx| {
            editor.update(cx, |ed, cx| {
                ed.abrir_modificacao(Modificacao::Contrair, window, cx)
            })
        });
        ve.run_until_parked();
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert_eq!(valor(&mut ve, 6, 15), 0);
        assert_eq!(valor(&mut ve, 10, 15), 255);
        // ⇧F6 abre o Difundir; Enter no campo confirma.
        ve.simulate_keystrokes("shift-f6");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.modificando()),
            Some(Modificacao::Difundir)
        );
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.modificando()).is_none());
        let borda = valor(&mut ve, 10, 15);
        assert!(borda > 0 && borda < 255, "a difusão fez rampa ({borda})");
        // Esc no diálogo cancela sem passo.
        let passos = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().passos().len());
        ve.simulate_keystrokes("shift-f6");
        ve.run_until_parked();
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.modificando()).is_none());
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().passos().len()),
            passos
        );
    }

    /// 🎭 O fluxo do Photoshop na janela, com a foto ampliada e movida: a
    /// máscara pelo botão, D (preto), um clique que revela a base **no ponto
    /// exato** da foto, X (branco) que restaura, ⌘Z/⇧⌘Z, e a miniatura da
    /// camada que devolve o pincel aos pixels.
    #[gpui_kit::test]
    fn pintar_de_preto_na_mascara_revela_o_de_baixo_com_zoom_e_mao(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.cor = [255, 0, 0];
                s.selecionar_tudo();
                s.preencher_selecao();
                s.desmarcar();
                s.pincel.raio = 1.5;
                s.pincel.dureza = 1.0;
            })
        });
        let base = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().base().clone());
        let cor = |ve: &mut VisualTestContext, x: u32, y: u32| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().cor_em(x as f32, y as f32).unwrap()
            })
        };
        let pixels_da_camada = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().documento().camadas[0].pixels.clone()
        });

        clicar_no_editor(&mut ve, "editor-camada-mascara");
        assert!(editor.read_with(&ve, |ed, _| ed.na_mascara()));
        assert!(ve.debug_bounds("editor-na-mascara").is_some());
        ve.simulate_keystrokes("d");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().pincel.cor),
            [0; 3]
        );

        // Ampliada três passos e movida com a mão.
        for _ in 0..3 {
            ve.simulate_keystrokes("cmd-=");
        }
        ve.run_until_parked();
        clicar_no_editor(&mut ve, "editor-mao");
        arrastar_no_palco(&mut ve, (0.5, 0.5), (0.4, 0.45));
        clicar_no_editor(&mut ve, "editor-pincel");

        // Um clique no centro do pixel (40, 20) da foto, achado pela área da
        // foto na tela de agora.
        let (alvo_x, alvo_y) = (40u32, 20u32);
        let clicar_na_foto = |ve: &mut VisualTestContext| {
            let (area, (l, a)) = editor.read_with(ve, |ed, _| {
                let b = ed.sessao().unwrap().base();
                (
                    ed.area_na_janela().unwrap(),
                    (b.width() as f32, b.height() as f32),
                )
            });
            let ponto = area.origin
                + gpui_kit::point(
                    area.size.width * ((alvo_x as f32 + 0.5) / l),
                    area.size.height * ((alvo_y as f32 + 0.5) / a),
                );
            assert!(
                ve.debug_bounds("palco-do-editor").unwrap().contains(&ponto),
                "o alvo está à vista"
            );
            ve.simulate_click(ponto, gpui_kit::Modifiers::none());
            ve.run_until_parked();
        };
        clicar_na_foto(&mut ve);
        assert_eq!(
            cor(&mut ve, alvo_x, alvo_y),
            base.get_pixel(alvo_x, alvo_y).0,
            "revelou a base ali"
        );
        assert_eq!(cor(&mut ve, alvo_x + 4, alvo_y), [255, 0, 0], "e só ali");
        let valor = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().documento().camadas[0]
                    .mascara
                    .as_ref()
                    .unwrap()
                    .valor(alvo_x, alvo_y)
            })
        };
        assert_eq!(valor(&mut ve), 0);

        // X: branco na frente — restaura.
        ve.simulate_keystrokes("x");
        ve.run_until_parked();
        clicar_na_foto(&mut ve);
        assert_eq!(cor(&mut ve, alvo_x, alvo_y), [255, 0, 0]);
        // ⌘Z volta o furo; ⇧⌘Z o fecha de novo.
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(valor(&mut ve), 0);
        ve.simulate_keystrokes("cmd-shift-z");
        ve.run_until_parked();
        assert_eq!(valor(&mut ve), 255);
        // Os pixels da camada nunca mudaram.
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .clone()),
            pixels_da_camada
        );

        // A miniatura do conteúdo: o pincel volta aos pixels (e pinta neles).
        clicar_no_editor(&mut ve, "editor-miniatura-0");
        assert!(!editor.read_with(&ve, |ed, _| ed.na_mascara()));
        ve.simulate_keystrokes("x");
        ve.run_until_parked();
        clicar_na_foto(&mut ve);
        assert_eq!(
            cor(&mut ve, alvo_x, alvo_y),
            [0, 0, 0],
            "pintou preto nos pixels"
        );
        assert_ne!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .clone()),
            pixels_da_camada
        );
    }

    /// 🪄 O Preenchimento sensível ao conteúdo pela tela, com o PatchMatch: o
    /// espaço modal abre **sem calcular** (dono, 07/out/2026), Visualizar
    /// calcula, mexer num ajuste derruba a prévia e o Aplicar; a
    /// prévia não mexe no documento, Esc cancela sem rastro, Enter aplica numa
    /// camada nova num passo só (⌘Z desfaz), um resultado de pedido antigo é
    /// descartado, e um documento mudado depois do instantâneo recusa.
    #[gpui_kit::test]
    fn o_preenchimento_sensivel_ao_conteudo_pela_tela(cx: &mut TestAppContext) {
        use crate::editor::janela::EstadoDoCalculo;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        // Um "objeto" vermelho num fundo listrado (pintado na camada).
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(24, 16, 12, 10)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [230, 20, 20];
                s.preencher_selecao();
            })
        });
        let fotografia = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().compor())
        };
        let antes = fotografia(&mut ve);
        let passos = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().historico().passos().len())
        };
        let n_passos = passos(&mut ve);
        let estado = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.espaco_do_preenchimento().map(|e| e.estado.clone())
            })
        };

        pelo_menu(&mut ve, "editor-menu-editar", "editor-abrir-preenchimento");
        ve.run_until_parked();
        assert!(
            ve.debug_bounds("editor-preenchimento").is_some(),
            "o painel abre"
        );
        assert!(
            ve.debug_bounds("editor-visualizacao").is_some(),
            "a janela da Visualização, ao lado da foto"
        );
        for fora in [
            "editor-varinha",
            "editor-salvar",
            "editor-abrir-preenchimento",
        ] {
            assert!(
                ve.debug_bounds(fora).is_none(),
                "{fora}: o resto do editor sai de cena (é modal)"
            );
        }
        assert_eq!(
            estado(&mut ve),
            Some(EstadoDoCalculo::Ocioso),
            "abrir não calcula"
        );
        assert!(ve.debug_bounds("editor-visualizacao-depois").is_none());
        clicar_no_editor(&mut ve, "editor-preenchimento-visualizar");
        assert_eq!(
            estado(&mut ve),
            Some(EstadoDoCalculo::Pronto { final_: true })
        );
        assert!(
            ve.debug_bounds("editor-visualizacao-depois").is_some(),
            "o resultado na Visualização"
        );
        // Um ajuste mudou: a prévia sai, e o Enter visualiza de novo.
        editor.update(&mut ve, |ed, cx| ed.redefinir_amostragem(cx));
        ve.run_until_parked();
        assert_eq!(estado(&mut ve), Some(EstadoDoCalculo::Ocioso));
        assert!(ve.debug_bounds("editor-visualizacao-depois").is_none());
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert_eq!(
            estado(&mut ve),
            Some(EstadoDoCalculo::Pronto { final_: true }),
            "Enter sem prévia visualiza"
        );
        assert_eq!(
            fotografia(&mut ve).as_raw(),
            antes.as_raw(),
            "a prévia não mexe no documento"
        );
        assert_eq!(passos(&mut ve), n_passos);

        // Esc: fecha sem rastro.
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert!(estado(&mut ve).is_none());
        assert_eq!(fotografia(&mut ve).as_raw(), antes.as_raw());

        // De novo, e um resultado de um pedido antigo é descartado.
        pelo_menu(&mut ve, "editor-menu-editar", "editor-abrir-preenchimento");
        ve.run_until_parked();
        clicar_no_editor(&mut ve, "editor-preenchimento-visualizar");
        let atual = editor.read_with(&ve, |ed, _| {
            ed.espaco_do_preenchimento()
                .unwrap()
                .resultado
                .clone()
                .unwrap()
        });
        let pedido = editor.read_with(&ve, |ed, _| ed.espaco_do_preenchimento().unwrap().pedido);
        let mut velho = atual.clone();
        velho.rgba.iter_mut().for_each(|v| *v = 7);
        let aceito = editor.update(&mut ve, |ed, cx| {
            ed.receber_para_teste(pedido - 1, velho, cx)
        });
        assert!(!aceito);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed
                .espaco_do_preenchimento()
                .unwrap()
                .resultado
                .clone()
                .unwrap()),
            atual,
            "o antigo não entrou"
        );

        // Enter: a camada nova, num passo só, sem nada do vermelho.
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(estado(&mut ve).is_none(), "o painel fecha");
        let (nomes, _) = camadas(&editor, &ve);
        assert_eq!(nomes.last().map(String::as_str), Some("Preenchimento 1"));
        let descr = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            s.historico()
                .passos()
                .iter()
                .map(|p| p.descricao(s.documento()))
                .collect::<Vec<_>>()
        });
        assert_eq!(passos(&mut ve), n_passos + 1, "{descr:?}");
        let depois = fotografia(&mut ve);
        let vermelhos = (16..26)
            .flat_map(|y| (24..36).map(move |x| (x, y)))
            .filter(|&(x, y)| depois.get_pixel(x, y).0 == [230, 20, 20])
            .count();
        assert_eq!(vermelhos, 0, "o objeto sumiu");
        for (x, y) in [(2, 2), (60, 40), (10, 30)] {
            assert_eq!(
                depois.get_pixel(x, y),
                antes.get_pixel(x, y),
                "fora da seleção, intacto"
            );
        }
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(
            fotografia(&mut ve).as_raw(),
            antes.as_raw(),
            "⌘Z volta tudo"
        );

        // O documento muda depois do instantâneo: aplicar recusa.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(24, 16, 12, 10)),
                    editor_core::Operacao::Nova,
                );
            })
        });
        pelo_menu(&mut ve, "editor-menu-editar", "editor-abrir-preenchimento");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.mover_opacidade(0.5))
        });
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.confirmar_opacidade())
        });
        let camadas_antes = camadas(&editor, &ve).0.len();
        // O primeiro Enter visualiza; o segundo aplica — e recusa.
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(matches!(estado(&mut ve), Some(EstadoDoCalculo::Falhou(m)) if m.contains("mudou")));
        assert_eq!(
            camadas(&editor, &ve).0.len(),
            camadas_antes,
            "nada aplicado"
        );
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();

        // Na máscara de camada não abre: o preenchimento refaz a foto.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.adicionar_mascara(false);
            })
        });
        pelo_menu(&mut ve, "editor-menu-editar", "editor-abrir-preenchimento");
        assert!(estado(&mut ve).is_none());
    }

    fn arrastar_no_palco(ve: &mut VisualTestContext, de: (f32, f32), ate: (f32, f32)) {
        let palco = ve.debug_bounds("palco-do-editor").unwrap();
        let ponto = |f: (f32, f32)| {
            palco.origin + gpui_kit::point(palco.size.width * f.0, palco.size.height * f.1)
        };
        let nada = gpui_kit::Modifiers::none();
        ve.simulate_mouse_down(ponto(de), gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
        ve.simulate_mouse_move(ponto(ate), Some(gpui_kit::MouseButton::Left), nada);
        ve.run_until_parked();
        ve.simulate_mouse_up(ponto(ate), gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
    }

    /// ⬚ A seleção pelo palco (M + arrastar), o pincel preso nela, e ⌘D ⌘A ⇧⌘I.
    #[gpui_kit::test]
    fn a_selecao_pelo_palco_prende_o_pincel(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.selecionando()),
            Some(crate::editor::janela::TipoDeSelecao::Retangulo)
        );
        // A foto encaixada (64×48) ocupa o palco inteiro na largura: o meio
        // da largura é o meio da foto.
        arrastar_no_palco(&mut ve, (0.25, 0.5), (0.5, 0.6));
        let limites = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().selecao().map(|s| s.limites())
        });
        assert!(limites.is_some(), "o arrasto selecionou");
        let (dentro, fora) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap().selecao().unwrap();
            (s.valor(24, 24), s.valor(60, 24))
        });
        assert_eq!((dentro, fora), (255, 0));

        ve.simulate_keystrokes("b");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.selecionando()), None);
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((2., 24.), (62., 24.), cx)
        });
        let (pintado, nao) = editor.read_with(&ve, |ed, _| {
            let c = &ed.sessao().unwrap().documento().camadas[0].pixels;
            (c.pixel(24, 24)[3], c.pixel(60, 24)[3])
        });
        assert!(pintado > 0 && nao == 0, "dentro {pintado}, fora {nao}");

        ve.simulate_keystrokes("cmd-d");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.sessao().unwrap().selecao().is_none()));
        ve.simulate_keystrokes("cmd-a");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.sessao().unwrap().selecao().is_some()));
        ve.simulate_keystrokes("cmd-shift-i");
        ve.run_until_parked();
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().selecao().is_none()),
            "tudo invertido é nada"
        );
    }

    /// ⌫ ⌥⌫ ⌘E e as miniaturas do painel.
    #[gpui_kit::test]
    fn apagar_preencher_e_mesclar_pelas_teclas(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.simulate_keystrokes("cmd-shift-n");
        ve.simulate_keystrokes("cmd-a");
        ve.simulate_keystrokes("alt-backspace");
        ve.run_until_parked();
        let alfa = |ed: &EditorDeFoto| {
            ed.sessao().unwrap().documento().camadas[1]
                .pixels
                .pixel(30, 30)[3]
        };
        assert_eq!(editor.read_with(&ve, |ed, _| alfa(ed)), 255, "⌥⌫ preencheu");
        ve.simulate_keystrokes("backspace");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| alfa(ed)), 0, "⌫ apagou");
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| alfa(ed)), 255);
        assert_eq!(editor.read_with(&ve, |ed, _| ed.quantas_miniaturas()), 2);

        ve.simulate_keystrokes("cmd-e");
        ve.run_until_parked();
        let (quantas, de_baixo) = editor.read_with(&ve, |ed, _| {
            let d = ed.sessao().unwrap().documento();
            (d.camadas.len(), d.camadas[0].pixels.pixel(30, 30)[3])
        });
        assert_eq!(
            (quantas, de_baixo),
            (1, 255),
            "⌘E levou o preenchido para a de baixo"
        );
        assert_eq!(editor.read_with(&ve, |ed, _| ed.quantas_miniaturas()), 1);
        ve.simulate_keystrokes("cmd-e");
        ve.run_until_parked();
        assert!(
            editor.read_with(&ve, |ed, _| ed.aviso().is_some_and(|(_, erro)| erro)),
            "na última não há o que mesclar"
        );
    }

    fn ponto_do_palco(
        ve: &mut VisualTestContext,
        f: (f32, f32),
    ) -> gpui_kit::Point<gpui_kit::Pixels> {
        let palco = ve.debug_bounds("palco-do-editor").unwrap();
        palco.origin + gpui_kit::point(palco.size.width * f.0, palco.size.height * f.1)
    }

    /// 🖃 O carimbo: S, ⌥ + clique na origem, e o traço copia a foto deslocada
    /// numa camada vazia.
    #[gpui_kit::test]
    fn o_carimbo_copia_da_origem_escolhida_com_alt(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("cmd-shift-n s");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Carimbo)
        );
        // Sem origem: avisa e não pinta.
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((30., 30.), (34., 30.), cx)
        });
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[1]
                .pixels
                .vazia())
        );

        let origem = ponto_do_palco(&mut ve, (0.25, 0.5));
        let alt = gpui_kit::Modifiers {
            alt: true,
            ..Default::default()
        };
        ve.simulate_mouse_down(origem, gpui_kit::MouseButton::Left, alt);
        ve.simulate_mouse_up(origem, gpui_kit::MouseButton::Left, alt);
        ve.run_until_parked();
        let o = editor
            .read_with(&ve, |ed, _| ed.sessao().unwrap().origem())
            .expect("a origem");
        assert!(
            (o.0 - 16.0).abs() < 1.0,
            "um quarto da largura de 64: {o:?}"
        );

        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((48., 24.), (48., 24.), cx)
        });
        let (camada, base) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (
                s.documento().camadas[1].pixels.pixel(48, 24),
                // O carimbo arredonda o deslocamento (`carimbo.rs`).
                s.base().get_pixel(o.0.round() as u32, o.1.round() as u32).0,
            )
        });
        assert_eq!([camada[0], camada[1], camada[2]], base, "copiou a origem");
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .vazia())
        );
    }

    /// 💧 I e um clique: a cor da foto vai para o pincel.
    #[gpui_kit::test]
    fn o_conta_gotas_pega_a_cor_da_foto(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("i");
        ve.run_until_parked();
        let p = ponto_do_palco(&mut ve, (0.5, 0.5));
        ve.simulate_click(p, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let (cor, esperada) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (s.pincel.cor, s.base().get_pixel(32, 24).0)
        });
        for i in 0..3 {
            assert!(
                (cor[i] as i32 - esperada[i] as i32).abs() <= 4,
                "{cor:?} × {esperada:?}"
            );
        }
    }

    /// ✥ V e arrastar: a camada anda, e um ⌘Z a devolve.
    #[gpui_kit::test]
    fn o_mover_arrasta_a_camada_e_se_desfaz(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 10.), (10., 10.), cx)
        });
        ve.simulate_keystrokes("v");
        ve.run_until_parked();
        arrastar_no_palco(&mut ve, (0.2, 0.2), (0.45, 0.2));
        let (antes, depois) = editor.read_with(&ve, |ed, _| {
            let c = &ed.sessao().unwrap().documento().camadas[0].pixels;
            (c.pixel(10, 10)[3], c.pixel(26, 10)[3])
        });
        assert_eq!(antes, 0, "saiu de lá");
        assert!(depois > 0, "andou 16 px (um quarto de 64)");
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .pixel(10, 10)[3])
                > 0
        );
    }

    /// ⌘T: arrastar dentro da caixa move; Enter aplica num passo só; Esc
    /// cancela.
    #[gpui_kit::test]
    fn a_transformacao_livre_move_aplica_e_cancela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((20., 20.), (24., 20.), cx)
        });
        let passos = |ed: &EditorDeFoto| ed.sessao().unwrap().historico().posicao();
        let antes = editor.read_with(&ve, |ed, _| passos(ed));

        ve.simulate_keystrokes("cmd-t");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.transformando()));
        // O traço está em (20..24, 20): 31% e 42% do palco de 64×48.
        arrastar_no_palco(
            &mut ve,
            (22.0 / 64.0, 20.0 / 48.0),
            (38.0 / 64.0, 20.0 / 48.0),
        );
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.transformando()));
        let (velho, novo) = editor.read_with(&ve, |ed, _| {
            let c = &ed.sessao().unwrap().documento().camadas[0].pixels;
            (c.pixel(20, 20)[3], c.pixel(36, 20)[3])
        });
        assert_eq!(velho, 0);
        assert!(novo > 0, "andou 16 px");
        assert_eq!(editor.read_with(&ve, |ed, _| passos(ed)), antes + 1);

        ve.simulate_keystrokes("cmd-t");
        ve.run_until_parked();
        arrastar_no_palco(
            &mut ve,
            (38.0 / 64.0, 20.0 / 48.0),
            (10.0 / 64.0, 40.0 / 48.0),
        );
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .pixel(36, 20)[3])
                > 0,
            "Esc devolveu"
        );
        assert_eq!(editor.read_with(&ve, |ed, _| passos(ed)), antes + 1);
    }

    /// ⌘J com seleção copia só o pedaço para uma camada nova; ⇧⌘J recorta.
    #[gpui_kit::test]
    fn camada_via_copia_e_via_recorte_pelas_teclas(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.simulate_keystrokes("cmd-a alt-backspace");
        ve.run_until_parked();
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        ve.update(|window, _| window.activate_window());
        arrastar_no_palco(&mut ve, (0.0, 0.0), (0.5, 0.5));
        ve.simulate_keystrokes("cmd-j");
        ve.run_until_parked();
        let (quantas, dentro, fora) = editor.read_with(&ve, |ed, _| {
            let d = ed.sessao().unwrap().documento();
            (
                d.camadas.len(),
                d.camadas[1].pixels.pixel(10, 10)[3],
                d.camadas[1].pixels.pixel(50, 40)[3],
            )
        });
        assert_eq!((quantas, dentro, fora), (2, 255, 0));
        // Como no Photoshop, a seleção sai com a cópia (o ⌘J seguinte
        // duplicaria a camada nova exata).
        assert!(selecao_de(&editor, &ve).is_none());
        editor.update(&mut ve, |ed, cx| ed.escolher_camada(0, cx));
        arrastar_no_palco(&mut ve, (0.0, 0.0), (0.5, 0.5));
        ve.simulate_keystrokes("cmd-shift-j");
        ve.run_until_parked();
        let (quantas, recortado, resto) = editor.read_with(&ve, |ed, _| {
            let d = ed.sessao().unwrap().documento();
            (
                d.camadas.len(),
                d.camadas[0].pixels.pixel(10, 10)[3],
                d.camadas[0].pixels.pixel(50, 40)[3],
            )
        });
        assert_eq!((quantas, recortado, resto), (3, 0, 255));
    }

    /// ✨ ⇧⌫ refaz a seleção pelo conteúdo em volta, numa camada vazia por
    /// cima, num passo só; o J faz o mesmo no traço.
    #[gpui_kit::test]
    fn preencher_pelo_conteudo_e_o_pincel_de_correcao(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("cmd-shift-n m");
        ve.run_until_parked();
        arrastar_no_palco(&mut ve, (0.4, 0.4), (0.55, 0.6));
        ve.simulate_keystrokes("shift-backspace");
        ve.run_until_parked();
        let (preenchido, fora, passos) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            let c = &s.documento().camadas[1].pixels;
            (
                c.pixel(30, 24)[3],
                c.pixel(5, 5)[3],
                s.historico().posicao(),
            )
        });
        assert!(!editor.read_with(&ve, |ed, _| ed.preenchendo()));
        assert_eq!(
            (preenchido, fora),
            (255, 0),
            "só a seleção, na camada de cima"
        );
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .vazia())
        );

        ve.simulate_keystrokes("cmd-d j");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(crate::editor::janela::Auxiliar::Correcao)
        );
        arrastar_no_palco(&mut ve, (0.1, 0.8), (0.2, 0.8));
        ve.run_until_parked();
        let (corrigido, depois) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (
                s.documento().camadas[1].pixels.pixel(9, 38)[3],
                s.historico().posicao(),
            )
        });
        assert!(corrigido > 0, "o traço foi refeito");
        // ⌘D (a seleção entra no desfazer desde a etapa 13) e o remendo.
        assert_eq!(depois, passos + 2);
    }

    /// ☀️ O e R escolhem as ferramentas de tom e de foco; a subexposição
    /// clareia numa camada vazia; um clique no Histórico volta um passo.
    #[gpui_kit::test]
    fn tom_foco_e_o_historico(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.simulate_keystrokes("cmd-shift-n o");
        ve.run_until_parked();
        assert!(matches!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Subexposicao(_))
        ));
        assert!(ve.debug_bounds("editor-faixa").is_some(), "a faixa aparece");
        let antes = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().compor().get_pixel(30, 24).0
        });
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((26., 24.), (34., 24.), cx)
        });
        let depois = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().compor().get_pixel(30, 24).0
        });
        assert!(depois[1] > antes[1], "clareou: {antes:?} → {depois:?}");
        assert!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .vazia())
        );

        // Nitidez não tem letra (como no Photoshop): pela barra.
        pelo_flyout(&mut ve, "editor-nitidez");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Nitidez)
        );
        assert!(ve.debug_bounds("editor-faixa").is_none());

        // Histórico: Abertura, Criar Camada 1, Pincel. Clicar na 1 volta.
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().posicao()),
            2
        );
        pelo_menu(&mut ve, "editor-menu-janela", "editor-janela-historico");
        clicar_no_editor(&mut ve, "editor-historico-1");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().posicao()),
            1
        );
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed
                .sessao()
                .unwrap()
                .compor()
                .get_pixel(30, 24)
                .0),
            antes
        );
        clicar_no_editor(&mut ve, "editor-historico-2");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().historico().posicao()),
            2
        );
    }

    // -------------------------------------------- etapa 13: os controles

    /// Um ponto da janela pela fração da **foto** (e não do palco).
    fn ponto_da_foto(
        ve: &mut VisualTestContext,
        editor: &gpui_kit::Entity<EditorDeFoto>,
        f: (f32, f32),
    ) -> gpui_kit::Point<gpui_kit::Pixels> {
        let area = editor.read_with(ve, |ed, _| ed.area_na_janela().unwrap());
        area.origin + gpui_kit::point(area.size.width * f.0, area.size.height * f.1)
    }

    /// Um arrasto com `ao_apertar` no clique e `no_arrasto` durante o
    /// movimento e no soltar — no Photoshop, ⇧⌥ no clique cruza a seleção, e
    /// segurados no arrasto fazem quadrado a partir do centro.
    fn arrastar_com(
        ve: &mut VisualTestContext,
        a: gpui_kit::Point<gpui_kit::Pixels>,
        b: gpui_kit::Point<gpui_kit::Pixels>,
        ao_apertar: gpui_kit::Modifiers,
        no_arrasto: gpui_kit::Modifiers,
    ) {
        ve.simulate_mouse_down(a, gpui_kit::MouseButton::Left, ao_apertar);
        ve.run_until_parked();
        // Em dois passos: o meio do caminho e o fim.
        let meio = gpui_kit::point((a.x + b.x) / 2., (a.y + b.y) / 2.);
        ve.simulate_mouse_move(meio, Some(gpui_kit::MouseButton::Left), no_arrasto);
        ve.simulate_mouse_move(b, Some(gpui_kit::MouseButton::Left), no_arrasto);
        ve.run_until_parked();
        ve.simulate_mouse_up(b, gpui_kit::MouseButton::Left, no_arrasto);
        ve.run_until_parked();
    }

    /// 🔤 As letras do Photoshop (tabela oficial da Adobe): H é a Mão, R a
    /// Girar vista, ⇧ + letra passa para a seguinte do grupo, e a letra volta
    /// à última usada nele. O H deixou de esconder a camada — agora é ⌘,.
    #[gpui_kit::test]
    fn as_letras_do_photoshop_escolhem_e_alternam_no_grupo(cx: &mut TestAppContext) {
        use crate::editor::janela::{Auxiliar, Item, TipoDeSelecao};
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let tecla = |ve: &mut VisualTestContext, t: &str| {
            ve.simulate_keystrokes(t);
            ve.run_until_parked();
            editor.read_with(ve, |ed, _| ed.item_atual())
        };
        assert_eq!(tecla(&mut ve, "h"), Some(Item::A(Auxiliar::Mao)));
        assert!(
            editor.read_with(&ve, |ed, _| ed.camada_visivel()),
            "o H não esconde mais a camada"
        );
        assert_eq!(tecla(&mut ve, "r"), Some(Item::A(Auxiliar::GirarVista)));
        assert_eq!(tecla(&mut ve, "m"), Some(Item::S(TipoDeSelecao::Retangulo)));
        assert_eq!(
            tecla(&mut ve, "shift-m"),
            Some(Item::S(TipoDeSelecao::Elipse))
        );
        assert_eq!(
            tecla(&mut ve, "b"),
            Some(Item::F(editor_core::Ferramenta::Pincel))
        );
        assert_eq!(
            tecla(&mut ve, "m"),
            Some(Item::S(TipoDeSelecao::Elipse)),
            "a letra volta à última do grupo"
        );
        assert_eq!(
            tecla(&mut ve, "shift-m"),
            Some(Item::S(TipoDeSelecao::Retangulo))
        );
        assert!(matches!(
            tecla(&mut ve, "o"),
            Some(Item::F(editor_core::Ferramenta::Subexposicao(_)))
        ));
        assert!(matches!(
            tecla(&mut ve, "shift-o"),
            Some(Item::F(editor_core::Ferramenta::Superexposicao(_)))
        ));
        assert_eq!(tecla(&mut ve, "g"), Some(Item::A(Auxiliar::Degrade)));
        assert_eq!(tecla(&mut ve, "shift-g"), Some(Item::A(Auxiliar::Lata)));
        assert_eq!(tecla(&mut ve, "shift-g"), Some(Item::A(Auxiliar::Degrade)));
        // ⌘, esconde e mostra a escolhida.
        tecla(&mut ve, "cmd-,");
        assert!(!editor.read_with(&ve, |ed, _| ed.camada_visivel()));
        tecla(&mut ve, "cmd-,");
        assert!(editor.read_with(&ve, |ed, _| ed.camada_visivel()));
    }

    /// 🔄 R + arrastar gira **só a vista**: nenhum pixel, dimensão ou passo do
    /// histórico muda; o clique na vista girada pinta onde o ponteiro aponta na
    /// foto; Esc volta a 0°.
    #[gpui_kit::test]
    fn girar_a_vista_nao_muda_a_foto_e_o_pincel_segue_o_giro(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let (doc, passos) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (s.documento().clone(), s.historico().passos().len())
        });
        ve.simulate_keystrokes("r");
        ve.run_until_parked();
        // Da direita do centro para baixo do centro: +90° (horário), com ⇧
        // preso no múltiplo de 15°.
        let shift = gpui_kit::Modifiers {
            shift: true,
            ..Default::default()
        };
        let (a, b) = (
            ponto_do_palco(&mut ve, (0.8, 0.5)),
            ponto_do_palco(&mut ve, (0.52, 0.95)),
        );
        arrastar_com(&mut ve, a, b, gpui_kit::Modifiers::none(), shift);
        let graus = editor.read_with(&ve, |ed, _| ed.giro_da_vista().to_degrees());
        assert!((graus - 90.0).abs() < 0.01, "girou {graus}°");
        ve.update(|window, _| window.refresh());
        ve.run_until_parked();
        assert!(
            editor.read_with(&ve, |ed, _| ed.ladrilhos_do_palco_girado()) > 0,
            "o palco girado foi montado"
        );
        editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            assert_eq!(s.documento(), &doc, "nenhum pixel mudou");
            assert_eq!((s.base().width(), s.base().height()), (64, 48));
            assert_eq!(s.historico().passos().len(), passos, "girar não é passo");
            assert!(!ed.alterado());
        });

        // O pincel na vista girada: abaixo do centro na tela é à direita do
        // centro na foto.
        ve.simulate_keystrokes("b");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.raio = 2.0;
                s.pincel.dureza = 1.0;
                s.pincel.suavizacao = 0.0;
                s.pincel.cor = [255, 0, 0];
            })
        });
        let largura = ve.debug_bounds("palco-do-editor").unwrap().size.width;
        let alvo =
            ponto_do_palco(&mut ve, (0.5, 0.5)) + gpui_kit::point(gpui_kit::px(0.), largura * 0.2);
        ve.simulate_click(alvo, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let (x, y) = editor.read_with(&ve, |ed, _| {
            let c = &ed.sessao().unwrap().documento().camadas[0].pixels;
            let mut achado = (0, 0);
            for y in 0..48 {
                for x in 0..64 {
                    if c.pixel(x, y)[3] > 200 {
                        achado = (x, y);
                    }
                }
            }
            achado
        });
        assert!(x > 38 && (y as i32 - 24).abs() <= 2, "pintou em ({x}, {y})");

        ve.simulate_keystrokes("r escape");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.giro_da_vista()), 0.0);
    }

    /// 🔢 Os números dão a opacidade (4 e 5 em seguida = 45%), ⇧ + números o
    /// fluxo, e `{` `}` a dureza em passos de 25%.
    #[gpui_kit::test]
    fn numeros_e_chaves_ajustam_o_pincel(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let pincel =
            |ve: &mut VisualTestContext| editor.read_with(ve, |ed, _| ed.sessao().unwrap().pincel);
        ve.simulate_keystrokes("b 4 5");
        ve.run_until_parked();
        assert!((pincel(&mut ve).opacidade - 0.45).abs() < 1e-4);
        ve.simulate_keystrokes("shift-3");
        ve.run_until_parked();
        assert!(
            (pincel(&mut ve).fluxo - 0.3).abs() < 1e-4,
            "⇧ + número é o fluxo"
        );
        assert!(
            (pincel(&mut ve).opacidade - 0.45).abs() < 1e-4,
            "a opacidade ficou"
        );
        ve.simulate_keystrokes("0");
        ve.run_until_parked();
        assert_eq!(pincel(&mut ve).opacidade, 1.0);
        let dureza = pincel(&mut ve).dureza;
        ve.simulate_keystrokes("shift-[");
        ve.run_until_parked();
        let menos = pincel(&mut ve).dureza;
        assert!(
            menos < dureza && (menos / 0.25).fract().abs() < 1e-4,
            "{dureza} → {menos}"
        );
        ve.simulate_keystrokes("shift-] shift-] shift-]");
        ve.run_until_parked();
        assert_eq!(pincel(&mut ve).dureza, 1.0);
        // Os sliders acompanham.
        ve.update(|window, _| window.refresh());
        ve.run_until_parked();
        assert!(ve.debug_bounds("editor-fluxo").is_some());
        assert!(ve.debug_bounds("editor-suavizacao").is_some());
        // O espaçamento (avançado) mora no painel Pincel.
        clicar_no_editor(&mut ve, "editor-abrir-painel-pincel");
        assert!(ve.debug_bounds("editor-espacamento").is_some());
    }

    /// ／ ⇧ + clique liga com uma reta desde o fim do traço anterior, num passo
    /// do desfazer próprio.
    #[gpui_kit::test]
    fn shift_clique_no_palco_traca_uma_reta(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("b");
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.raio = 1.5;
                s.pincel.dureza = 1.0;
                s.pincel.suavizacao = 0.0;
                s.pincel.cor = [0, 0, 255];
            })
        });
        let a = ponto_da_foto(&mut ve, &editor, (0.1, 0.5));
        let b = ponto_da_foto(&mut ve, &editor, (0.9, 0.5));
        ve.simulate_click(a, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let shift = gpui_kit::Modifiers {
            shift: true,
            ..Default::default()
        };
        ve.simulate_click(b, shift);
        ve.run_until_parked();
        let (meio, passos) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            let fim = s.fim_do_ultimo_traco().unwrap();
            (
                s.documento().camadas[0].pixels.pixel(32, fim.1 as u32)[3],
                s.historico().passos().len(),
            )
        });
        assert_eq!(meio, 255, "o meio da reta foi pintado");
        assert_eq!(passos, 2);
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        let meio = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .pixel(32, 24)[3]
        });
        assert_eq!(meio, 0, "desfazer tira só a reta");
    }

    /// ⬚ ⇧⌥ + arrasto cruza a seleção, o desfazer volta a anterior, e
    /// arrastar por dentro (sem modificador) move só o contorno.
    #[gpui_kit::test]
    fn a_intersecao_e_o_contorno_pelo_palco(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.1, 0.1)),
            ponto_da_foto(&mut ve, &editor, (0.6, 0.6)),
        );
        arrastar_com(
            &mut ve,
            a,
            b,
            gpui_kit::Modifiers::none(),
            gpui_kit::Modifiers::none(),
        );
        let shift_alt = gpui_kit::Modifiers {
            shift: true,
            alt: true,
            ..Default::default()
        };
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.4, 0.05)),
            ponto_da_foto(&mut ve, &editor, (0.95, 0.95)),
        );
        arrastar_com(&mut ve, a, b, shift_alt, gpui_kit::Modifiers::none());
        let valor = |ve: &mut VisualTestContext, x: u32, y: u32| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().selecao().map_or(0, |s| s.valor(x, y))
            })
        };
        // A foto (64×48) encaixada: 50% da largura é x = 32.
        assert_eq!(valor(&mut ve, 32, 20), 255, "no cruzamento");
        assert_eq!(valor(&mut ve, 10, 20), 0, "só na primeira");
        assert_eq!(valor(&mut ve, 50, 40), 0, "só na segunda");
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(valor(&mut ve, 10, 20), 255, "o desfazer volta a primeira");

        // Arrastar por dentro move o contorno; os pixels ficam.
        let doc = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().clone());
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.3, 0.3)),
            ponto_da_foto(&mut ve, &editor, (0.5, 0.3)),
        );
        arrastar_com(
            &mut ve,
            a,
            b,
            gpui_kit::Modifiers::none(),
            gpui_kit::Modifiers::none(),
        );
        assert_eq!(valor(&mut ve, 8, 20), 0, "o contorno saiu daqui");
        assert_eq!(valor(&mut ve, 45, 20), 255, "e veio para cá");
        editor.read_with(&ve, |ed, _| {
            assert_eq!(ed.sessao().unwrap().documento(), &doc, "nenhum pixel andou")
        });
    }

    /// ⇧ no arrasto faz quadrado; ⌥ desenha a partir do centro.
    #[gpui_kit::test]
    fn shift_e_alt_no_arrasto_fazem_quadrado_e_centro(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        let shift = gpui_kit::Modifiers {
            shift: true,
            ..Default::default()
        };
        let nada = gpui_kit::Modifiers::none();
        let limites = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().selecao().unwrap().caixa_justa()
            })
        };
        // De (10%, 10%) até (60%, 30%) da foto 64×48: sem ⇧ seria 32×10; com
        // ⇧, o maior lado nos dois.
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.1, 0.1)),
            ponto_da_foto(&mut ve, &editor, (0.6, 0.3)),
        );
        arrastar_com(&mut ve, a, b, nada, shift);
        let r = limites(&mut ve);
        assert!(
            (r.largura as i32 - r.altura as i32).abs() <= 1,
            "quadrado: {r:?}"
        );
        assert!(r.largura >= 30, "{r:?}");

        // ⌥: o ponto do clique é o centro.
        let alt = gpui_kit::Modifiers {
            alt: true,
            ..Default::default()
        };
        ve.simulate_keystrokes("cmd-d");
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.5, 0.5)),
            ponto_da_foto(&mut ve, &editor, (0.6, 0.6)),
        );
        arrastar_com(&mut ve, a, b, nada, alt);
        let r = limites(&mut ve);
        let centro = (
            r.x as f32 + r.largura as f32 / 2.0,
            r.y as f32 + r.altura as f32 / 2.0,
        );
        assert!(
            (centro.0 - 32.0).abs() <= 1.0 && (centro.1 - 24.0).abs() <= 1.0,
            "{r:?}"
        );
        assert!(r.largura >= 11, "o dobro da distância: {r:?}");
    }

    // ------------------------------------------- opções das ferramentas de seleção

    fn selecao_de(
        editor: &gpui_kit::Entity<EditorDeFoto>,
        ve: &VisualTestContext,
    ) -> Option<editor_core::Selecao> {
        editor.read_with(ve, |ed, _| ed.sessao().unwrap().selecao().cloned())
    }

    fn passos_de(editor: &gpui_kit::Entity<EditorDeFoto>, ve: &VisualTestContext) -> usize {
        editor.read_with(ve, |ed, _| ed.sessao().unwrap().historico().passos().len())
    }

    /// ⬚ A barra de opções: os quatro modos com o ativo realçado; o modo da
    /// barra vale sem modificador, e ⇧/⌥ trocam só durante o gesto.
    #[gpui_kit::test]
    fn os_modos_da_barra_e_os_modificadores_so_no_gesto(cx: &mut TestAppContext) {
        use editor_core::Operacao;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        assert!(ve.debug_bounds("editor-opcoes-da-selecao").is_none());
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        for id in [
            "editor-opcoes-da-selecao",
            "editor-modo-nova",
            "editor-modo-adicionar",
            "editor-modo-subtrair",
            "editor-modo-intersectar",
            "editor-difusao",
            "editor-estilo",
        ] {
            assert!(ve.debug_bounds(id).is_some(), "{id} na barra");
        }
        let nada = gpui_kit::Modifiers::none();
        // Nova: a esquerda inteira da foto.
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.0, 0.0)),
            ponto_da_foto(&mut ve, &editor, (0.5, 1.0)),
        );
        arrastar_com(&mut ve, a, b, nada, nada);
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(10, 10), s.valor(40, 10)), (255, 0));
        // Subtrair pela barra, sem modificador: tira o alto.
        clicar_no_editor(&mut ve, "editor-modo-subtrair");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.modo_de_selecao()),
            Operacao::Subtrair
        );
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.0, 0.0)),
            ponto_da_foto(&mut ve, &editor, (1.0, 0.25)),
        );
        arrastar_com(&mut ve, a, b, nada, nada);
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(10, 5), s.valor(10, 30)), (0, 255));
        // ⇧ no clique soma, só neste gesto: a direita de baixo entra…
        let shift = gpui_kit::Modifiers::shift();
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.75, 0.75)),
            ponto_da_foto(&mut ve, &editor, (1.0, 1.0)),
        );
        arrastar_com(&mut ve, a, b, shift, nada);
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!(s.valor(56, 42), 255);
        // …e a barra continua em Subtrair.
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.modo_de_selecao()),
            Operacao::Subtrair
        );
        // Intersectar pela barra.
        clicar_no_editor(&mut ve, "editor-modo-intersectar");
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (0.0, 0.5)),
            ponto_da_foto(&mut ve, &editor, (1.0, 1.0)),
        );
        arrastar_com(&mut ve, a, b, nada, nada);
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!(
            (s.valor(10, 20), s.valor(10, 40), s.valor(56, 42)),
            (0, 255, 255)
        );
        assert_eq!(passos_de(&editor, &ve), 4);
        assert!(
            !editor.read_with(&ve, |ed, _| ed.alterado()),
            "só seleção: nada a salvar"
        );
        // Mudar opção não é passo e não mexe na seleção de agora.
        let antes = selecao_de(&editor, &ve);
        editor.update(&mut ve, |ed, cx| {
            ed.opcoes_da_forma_mut(crate::editor::janela::TipoDeSelecao::Retangulo)
                .acabamento
                .difusao = 10;
            ed.escolher_modo_de_selecao(Operacao::Nova, cx);
        });
        ve.run_until_parked();
        assert_eq!(selecao_de(&editor, &ve), antes);
        assert_eq!(passos_de(&editor, &ve), 4);
    }

    /// 📐 Proporção fixa e tamanho fixo pelo palco, com a medida em pixels
    /// do documento durante o gesto — a mesma com zoom e com a vista girada.
    #[gpui_kit::test]
    fn proporcao_e_tamanho_fixos_nao_dependem_do_zoom_nem_do_giro(cx: &mut TestAppContext) {
        use crate::editor::janela::{TipoDeEstilo, TipoDeSelecao};
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        // Proporção 2:1: um arrasto alto vira a caixa 2:1 que cobre o ponteiro.
        editor.update(&mut ve, |ed, cx| {
            ed.escolher_estilo(TipoDeEstilo::Proporcao, cx);
            ed.opcoes_da_forma_mut(TipoDeSelecao::Retangulo).proporcao = (2.0, 1.0);
        });
        ve.run_until_parked();
        assert!(ve.debug_bounds("editor-estilo-largura").is_some());
        assert!(ve.debug_bounds("editor-trocar-medidas").is_some());
        let nada = gpui_kit::Modifiers::none();
        let a = ponto_da_foto(&mut ve, &editor, (0.0, 0.0));
        let b = ponto_da_foto(&mut ve, &editor, (10.0 / 64.0, 12.0 / 48.0));
        ve.simulate_mouse_down(a, gpui_kit::MouseButton::Left, nada);
        ve.simulate_mouse_move(b, Some(gpui_kit::MouseButton::Left), nada);
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.medida_do_gesto()),
            Some((24, 12))
        );
        assert!(ve.debug_bounds("editor-medida-da-selecao").is_some());
        ve.simulate_mouse_up(b, gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
        let c = selecao_de(&editor, &ve).unwrap().caixa_justa();
        assert_eq!((c.largura, c.altura), (24, 12));
        // ⇄ troca: 1:2.
        clicar_no_editor(&mut ve, "editor-trocar-medidas");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed
                .opcoes_da_forma(TipoDeSelecao::Retangulo)
                .proporcao),
            (1.0, 2.0)
        );

        // Tamanho fixo 20×10, com zoom de 4× e a vista girada 30°: o clique
        // dá 20×10 pixels do documento.
        editor.update(&mut ve, |ed, cx| {
            ed.escolher_estilo(TipoDeEstilo::Tamanho, cx);
            ed.opcoes_da_forma_mut(TipoDeSelecao::Retangulo).tamanho = (20, 10);
            ed.ir_para_nivel(crate::revelacao::zoom::Nivel::Razao(4.0), None, cx);
            ed.girar_a_vista(30f32.to_radians(), cx);
        });
        ve.run_until_parked();
        let p = ponto_do_palco(&mut ve, (0.5, 0.5));
        ve.simulate_mouse_down(p, gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.medida_do_gesto()),
            Some((20, 10))
        );
        ve.simulate_mouse_up(p, gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
        let c = selecao_de(&editor, &ve).unwrap().caixa_justa();
        assert_eq!((c.largura, c.altura), (20, 10));
        assert!(!editor.read_with(&ve, |ed, _| ed.alterado()));
    }

    /// ⬠ O laço poligonal (⇧L): clique a clique, ⌫ tira o último vértice,
    /// fechar no primeiro vira um passo só; Esc cancela e a seleção de antes
    /// fica; o duplo clique também fecha.
    #[gpui_kit::test]
    fn o_laco_poligonal_clique_a_clique(cx: &mut TestAppContext) {
        use crate::editor::janela::{Item, TipoDeSelecao};
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("l shift-l");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.item_atual()),
            Some(Item::S(TipoDeSelecao::LacoPoligonal))
        );
        let nada = gpui_kit::Modifiers::none();
        let clique = |ve: &mut VisualTestContext, f: (f32, f32)| {
            let p = ponto_da_foto(ve, &editor, f);
            ve.simulate_click(p, gpui_kit::Modifiers::none());
            ve.run_until_parked();
        };
        clique(&mut ve, (0.1, 0.1));
        clique(&mut ve, (0.9, 0.1));
        clique(&mut ve, (0.5, 0.5)); // um vértice errado…
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.vertices_do_poligono().len()),
            3
        );
        ve.simulate_keystrokes("backspace"); // …tirado com ⌫
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.vertices_do_poligono().len()),
            2
        );
        assert!(
            selecao_de(&editor, &ve).is_none(),
            "⌫ não apagou pixel nem seleção"
        );
        // A prévia segue o ponteiro sem botão.
        let p = ponto_da_foto(&mut ve, &editor, (0.9, 0.9));
        ve.simulate_mouse_move(p, None, nada);
        ve.run_until_parked();
        let (px_, py_) = editor
            .read_with(&ve, |ed, _| ed.proximo_do_poligono())
            .expect("a prévia segue o ponteiro");
        assert!(
            (px_ - 57.6).abs() < 1.0 && (py_ - 43.2).abs() < 1.0,
            "({px_}, {py_})"
        );
        clique(&mut ve, (0.9, 0.9));
        clique(&mut ve, (0.1, 0.9));
        assert!(selecao_de(&editor, &ve).is_none(), "ainda aberto");
        clique(&mut ve, (0.1, 0.1)); // no primeiro: fecha
        assert!(!editor.read_with(&ve, |ed, _| ed.poligono_aberto()));
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(32, 24), s.valor(2, 2)), (255, 0));
        assert_eq!(passos_de(&editor, &ve), 1);

        // Esc no meio: a seleção de antes fica, sem passo.
        clique(&mut ve, (0.05, 0.05));
        clique(&mut ve, (0.3, 0.05));
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.poligono_aberto()));
        assert_eq!(selecao_de(&editor, &ve), Some(s));
        assert_eq!(passos_de(&editor, &ve), 1);

        // Duplo clique fecha (Nova: troca a de antes).
        clique(&mut ve, (0.6, 0.6));
        clique(&mut ve, (0.95, 0.6));
        let p = ponto_da_foto(&mut ve, &editor, (0.95, 0.95));
        ve.simulate_click(p, nada);
        ve.simulate_event(gpui_kit::MouseDownEvent {
            position: p,
            button: gpui_kit::MouseButton::Left,
            modifiers: nada,
            click_count: 2,
            first_mouse: false,
        });
        ve.simulate_event(gpui_kit::MouseUpEvent {
            position: p,
            button: gpui_kit::MouseButton::Left,
            modifiers: nada,
            click_count: 2,
        });
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.poligono_aberto()));
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(58, 38), s.valor(32, 24)), (255, 0));
        assert_eq!(passos_de(&editor, &ve), 2);
    }

    /// 🪄 A varinha pela barra: "Camada atual" lê só a camada (o transparente
    /// conta), "Todas as camadas" a foto como aparece.
    #[gpui_kit::test]
    fn a_varinha_amostra_a_camada_ou_todas(cx: &mut TestAppContext) {
        use editor_core::AmostraDaVarinha;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("w");
        ve.run_until_parked();
        for id in [
            "editor-tolerancia",
            "editor-contigua",
            "editor-suavizar",
            "editor-amostra",
        ] {
            assert!(ve.debug_bounds(id).is_some(), "{id} na barra");
        }
        // Na camada atual (vazia), o clique pega a foto inteira: tudo é
        // transparente nela.
        editor.update(&mut ve, |ed, _| {
            ed.opcoes_da_varinha_mut().amostra = AmostraDaVarinha::CamadaAtual;
            ed.opcoes_da_varinha_mut().tolerancia = 0;
        });
        let p = ponto_da_foto(&mut ve, &editor, (0.5, 0.5));
        ve.simulate_click(p, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(0, 0), s.valor(63, 47)), (255, 255));
        // Todas as camadas, tolerância 0: só o que tem a cor da base ali.
        editor.update(&mut ve, |ed, _| {
            ed.opcoes_da_varinha_mut().amostra = AmostraDaVarinha::Todas;
        });
        ve.simulate_click(p, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let todas = selecao_de(&editor, &ve);
        assert_ne!(
            todas,
            Some(s),
            "a base entra em Todas e não em Camada atual"
        );
    }

    /// ↔️ "Transformar seleção" pelo menu: só o contorno anda; Enter aplica
    /// num passo de seleção; os pixels ficam.
    #[gpui_kit::test]
    fn transformar_a_selecao_pelo_menu(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(4, 4, 10, 10)),
                    editor_core::Operacao::Nova,
                );
            })
        });
        ve.simulate_keystrokes("m");
        ve.run_until_parked();
        let doc = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().clone());
        clicar_no_editor(&mut ve, "editor-modificar-selecao");
        clicar_no_editor(&mut ve, "editor-transformar-selecao");
        assert!(editor.read_with(&ve, |ed, _| ed.transformando()));
        assert!(ve.debug_bounds("editor-dica-da-transformacao").is_some());
        // Arrastar por dentro da caixa leva o contorno.
        let (a, b) = (
            ponto_da_foto(&mut ve, &editor, (8.0 / 64.0, 8.0 / 48.0)),
            ponto_da_foto(&mut ve, &editor, (28.0 / 64.0, 8.0 / 48.0)),
        );
        arrastar_com(
            &mut ve,
            a,
            b,
            gpui_kit::Modifiers::none(),
            gpui_kit::Modifiers::none(),
        );
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.transformando()));
        let s = selecao_de(&editor, &ve).unwrap();
        assert_eq!((s.valor(26, 8), s.valor(6, 8)), (255, 0));
        editor.read_with(&ve, |ed, _| {
            assert_eq!(ed.sessao().unwrap().documento(), &doc, "pixels intactos");
            assert!(!ed.alterado());
        });
        assert_eq!(passos_de(&editor, &ve), 2);
        assert_eq!(
            editor.read_with(&ve, |ed, _| {
                let s = ed.sessao().unwrap();
                s.historico()
                    .a_desfazer()
                    .map(|p| p.descricao(s.documento()))
            }),
            Some("Transformar seleção".to_string()),
            "o arrasto foi da caixa, e não um mover do contorno"
        );
    }

    // ------------------------------------------- etapa 14: ⌘T e carimbo

    fn transformacao_de(
        editor: &gpui_kit::Entity<EditorDeFoto>,
        ve: &VisualTestContext,
    ) -> (editor_core::Caixa, editor_core::Transformacao) {
        editor.read_with(ve, |ed, _| ed.sessao().unwrap().transformacao().unwrap())
    }

    /// ↔️ ⌘T com oito alças: o meio do lado muda só a largura com o outro lado
    /// parado; ⌥ no canto escala em volta do ponto de referência; o ponto de
    /// referência anda e o giro passa a ser em volta dele; os números da barra
    /// mudam a caixa; Enter é um passo só.
    #[gpui_kit::test]
    fn a_transformacao_tem_oito_alcas_e_ponto_de_referencia(cx: &mut TestAppContext) {
        use crate::editor::janela::ParteDaCaixa;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(16, 12, 32, 24)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [255, 0, 0];
                s.preencher_selecao();
                s.desmarcar();
            });
            ed.transformar(cx);
        });
        ve.run_until_parked();
        let passos = passos_de(&editor, &ve);
        for id in [
            "editor-opcoes-da-transformacao",
            "editor-transformacao-x",
            "editor-transformacao-l",
            "editor-transformacao-angulo",
            "editor-referencia-da-caixa",
        ] {
            assert!(ve.debug_bounds(id).is_some(), "{id}");
        }
        let (caixa, _) = transformacao_de(&editor, &ve);
        assert_eq!(caixa, editor_core::Caixa::nova(16, 12, 32, 24));
        let foto = |ve: &mut VisualTestContext, x: f32, y: f32| {
            ponto_da_foto(ve, &editor, (x / 64.0, y / 48.0))
        };
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.parte_da_caixa(48.0, 24.0)),
            Some(ParteDaCaixa::Alca(3))
        );
        // O meio do lado direito (48, 24) até (56, 24): só a largura, 40/32.
        let nada = gpui_kit::Modifiers::none();
        let (a, b) = (foto(&mut ve, 48.0, 24.0), foto(&mut ve, 56.0, 24.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        let (caixa, t) = transformacao_de(&editor, &ve);
        assert!(
            (t.escala_x - 1.25).abs() < 0.05 && (t.escala_y - 1.0).abs() < 1e-4,
            "{t:?}"
        );
        let esquerda = t.aplicar(&caixa, 16.0, 24.0);
        assert!(
            (esquerda.0 - 16.0).abs() < 0.5,
            "o lado esquerdo ficou: {esquerda:?}"
        );

        // Esc e de novo; ⌥ no canto ↘: o centro (a referência) fica parado.
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| ed.transformar(cx));
        ve.run_until_parked();
        let alt = gpui_kit::Modifiers::alt();
        let (a, b) = (foto(&mut ve, 48.0, 36.0), foto(&mut ve, 56.0, 42.0));
        arrastar_com(&mut ve, a, b, alt, alt);
        let (caixa, t) = transformacao_de(&editor, &ve);
        let c = t.aplicar(&caixa, 32.0, 24.0);
        assert!(
            (c.0 - 32.0).abs() < 0.5 && (c.1 - 24.0).abs() < 0.5,
            "o centro ficou: {c:?}"
        );
        assert!(t.escala_x > 1.2, "{t:?}");

        // A referência levada ao canto ↖ (16, 12), e o giro em volta dela.
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| ed.transformar(cx));
        ve.run_until_parked();
        let (a, b) = (foto(&mut ve, 32.0, 24.0), foto(&mut ve, 20.0, 14.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        let r = editor
            .read_with(&ve, |ed, _| ed.referencia_da_caixa())
            .unwrap();
        assert!(
            (r.0 - 20.0).abs() < 1.0 && (r.1 - 14.0).abs() < 1.0,
            "{r:?}"
        );
        let shift = gpui_kit::Modifiers::shift();
        let (a, b) = (foto(&mut ve, 60.0, 14.0), foto(&mut ve, 20.0, 46.0));
        arrastar_com(&mut ve, a, b, nada, shift);
        let (caixa, t) = transformacao_de(&editor, &ve);
        assert!(
            (t.angulo.to_degrees() - 90.0).abs() < 0.01,
            "{}",
            t.angulo.to_degrees()
        );
        let q = t.aplicar(&caixa, r.0, r.1);
        assert!(
            (q.0 - r.0).abs() < 0.5 && (q.1 - r.1).abs() < 0.5,
            "a referência ficou: {q:?}"
        );

        // Os números: largura a 50% em volta da referência, ângulo 0.
        editor.update(&mut ve, |ed, cx| {
            ed.numero_da_transformacao_mudou(4, 0.0, cx);
            ed.numero_da_transformacao_mudou(2, 50.0, cx);
        });
        ve.run_until_parked();
        let (caixa, t) = transformacao_de(&editor, &ve);
        assert!((t.escala_x - 0.5).abs() < 1e-4 && t.angulo.abs() < 1e-4);
        let q = t.aplicar(&caixa, r.0, r.1);
        assert!((q.0 - r.0).abs() < 0.5, "{q:?}");
        let (x, _, l, _, _) = editor
            .read_with(&ve, |ed, _| ed.numeros_da_transformacao())
            .unwrap();
        assert!((l - 50.0).abs() < 1e-3 && (x - r.0).abs() < 0.5);
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.transformando()));
        assert_eq!(passos_de(&editor, &ve), passos + 1, "Enter é um passo só");
        assert_eq!(
            editor.read_with(&ve, |ed, _| {
                let s = ed.sessao().unwrap();
                s.historico()
                    .a_desfazer()
                    .map(|p| p.descricao(s.documento()))
            }),
            Some("Transformação livre".to_string())
        );
    }

    /// 🖃 O carimbo pela tela: as opções no painel (modo, amostra, alinhado,
    /// a origem no pincel), Alinhado desligado pelo clique e a prévia da
    /// origem no círculo.
    #[gpui_kit::test]
    fn as_opcoes_do_carimbo_e_a_previa_da_origem(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("s");
        ve.run_until_parked();
        for id in [
            "editor-modo-da-ferramenta",
            "editor-amostra-do-carimbo",
            "editor-carimbo-alinhado",
            "editor-carimbo-sobreposicao",
        ] {
            assert!(ve.debug_bounds(id).is_some(), "{id} no painel");
        }
        clicar_no_editor(&mut ve, "editor-carimbo-alinhado");
        assert!(!editor.read_with(&ve, |ed, _| ed.sessao().unwrap().carimbo.alinhado));
        // ⌥ + clique escolhe a origem; o ponteiro em outro lugar mostra a
        // prévia dentro do círculo.
        let origem = ponto_da_foto(&mut ve, &editor, (0.25, 0.25));
        ve.simulate_click(origem, gpui_kit::Modifiers::alt());
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.sessao().unwrap().origem().is_some()));
        let p = ponto_da_foto(&mut ve, &editor, (0.7, 0.6));
        ve.simulate_mouse_move(p, None, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        ve.update(|window, _| window.refresh());
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.tem_previa_do_carimbo()));
        assert!(ve.debug_bounds("editor-previa-do-carimbo").is_some());
        // Desligada, some.
        clicar_no_editor(&mut ve, "editor-carimbo-sobreposicao");
        ve.simulate_mouse_move(p, None, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.tem_previa_do_carimbo()));
    }

    /// 🧱 O Mover pelo palco leva o conteúdo para fora da foto e traz de
    /// volta inteiro (etapa 14, parte 2).
    #[gpui_kit::test]
    fn o_mover_leva_para_fora_e_traz_de_volta(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(4, 4, 12, 12)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [255, 0, 0];
                s.preencher_selecao();
                s.desmarcar();
            })
        });
        ve.simulate_keystrokes("v");
        ve.run_until_parked();
        let nada = gpui_kit::Modifiers::none();
        let foto = |ve: &mut VisualTestContext, x: f32, y: f32| {
            ponto_da_foto(ve, &editor, (x / 64.0, y / 48.0))
        };
        // Para a esquerda, 30 px: o quadrado (4..16) sai inteiro da foto.
        let (a, b) = (foto(&mut ve, 10.0, 10.0), foto(&mut ve, -20.0, 10.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        let pixel = |ve: &mut VisualTestContext, x: i64, y: i64| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().documento().camadas[0]
                    .pixels
                    .pixel_em(x, y)
            })
        };
        assert_eq!(pixel(&mut ve, 10, 10)[3], 0, "saiu da foto");
        assert_eq!(
            pixel(&mut ve, -20, 10),
            [255, 0, 0, 255],
            "mas ficou guardado"
        );
        // De volta, num arrasto novo.
        let (a, b) = (foto(&mut ve, 30.0, 30.0), foto(&mut ve, 60.0, 30.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        assert_eq!(pixel(&mut ve, 4, 4), [255, 0, 0, 255]);
        assert_eq!(pixel(&mut ve, 15, 15), [255, 0, 0, 255], "inteiro");
    }

    /// ✂️ O retoque de queixo e pescoço do Photoshop, de ponta a ponta pela
    /// tela: camada da fotografia, laço poligonal, difusão, duas camadas via
    /// cópia, máscara de corte (⌥ + clique na divisa), Deformar (cancelar e
    /// confirmar), recuperação com origem manual, mesclar e salvar/reabrir.
    #[gpui_kit::test]
    fn o_retoque_do_queixo_pela_tela(cx: &mut TestAppContext) {
        use crate::editor::janela::Modificacao;
        let (m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let nada = gpui_kit::Modifiers::none();
        let alt = gpui_kit::Modifiers {
            alt: true,
            ..Default::default()
        };
        let doc = |ve: &VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().documento().clone())
        };

        // 1. A camada da fotografia, pelo menu ⋯ (montada em segundo plano).
        clicar_no_editor(&mut ve, "editor-camada-mais");
        clicar_no_editor(&mut ve, "editor-camada-fotografia");
        ve.run_until_parked();
        let (nomes, ativa) = camadas(&editor, &ve);
        assert_eq!(nomes, vec!["Fotografia".to_string(), "Pintura".to_string()]);
        assert_eq!(ativa, 0);
        let base = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().base().clone());
        assert_eq!(doc(&ve).camadas[0].pixels.pixel(10, 20), {
            let b = base.get_pixel(10, 20).0;
            [b[0], b[1], b[2], 255]
        });

        // 2. O laço poligonal em volta do "queixo".
        ve.simulate_keystrokes("l shift-l");
        ve.run_until_parked();
        for f in [
            (0.25, 0.4),
            (0.75, 0.4),
            (0.75, 0.85),
            (0.25, 0.85),
            (0.25, 0.4),
        ] {
            let p = ponto_da_foto(&mut ve, &editor, f);
            ve.simulate_click(p, nada);
            ve.run_until_parked();
        }
        assert!(selecao_de(&editor, &ve).is_some(), "o polígono fechou");

        // 3. Difusão de 2 px pelo menu de contexto do palco.
        let p = ponto_da_foto(&mut ve, &editor, (0.5, 0.6));
        ve.simulate_mouse_down(p, gpui_kit::MouseButton::Right, nada);
        ve.simulate_mouse_up(p, gpui_kit::MouseButton::Right, nada);
        ve.run_until_parked();
        assert!(
            ve.debug_bounds("editor-contexto-via-copia").is_some(),
            "o menu da seleção"
        );
        clicar_no_editor(&mut ve, "editor-contexto-difundir");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.modificando()),
            Some(Modificacao::Difundir)
        );
        ve.update(|window, cx| {
            editor.update(cx, |ed, cx| {
                ed.cancelar_modificacao(window, cx);
                ed.modificar_selecao(Modificacao::Difundir, 2, cx);
            })
        });
        ve.run_until_parked();
        let borda = selecao_de(&editor, &ve).unwrap().valor(16, 30);
        assert!((1..255).contains(&borda), "borda difusa: {borda}");

        // 4. ⌘J duas vezes: o trecho e a cópia exata dele.
        ve.simulate_keystrokes("cmd-j");
        ve.run_until_parked();
        assert!(
            selecao_de(&editor, &ve).is_none(),
            "a seleção sai com a cópia"
        );
        ve.simulate_keystrokes("cmd-j");
        ve.run_until_parked();
        let d = doc(&ve);
        assert_eq!(d.camadas.len(), 4);
        assert_eq!(d.camadas[2].pixels, d.camadas[1].pixels, "duas iguais");
        assert_eq!(d.camadas[1].pixels.pixel(16, 30)[3], borda);

        // 5. ⌥ + clique na divisa: a de cima recortada pela de baixo.
        let divisa = ve.debug_bounds("editor-divisa-2").expect("a divisa");
        ve.simulate_click(divisa.center(), alt);
        ve.run_until_parked();
        assert!(doc(&ve).camadas[2].recortada);
        assert!(
            ve.debug_bounds("editor-recorte-2").is_some(),
            "o ↳ no painel"
        );

        // 6. Deformar pelo menu Transformar ▾: Esc devolve tudo.
        let antes = doc(&ve);
        let passos_antes = passos_de(&editor, &ve);
        pelo_menu(&mut ve, "editor-menu-editar", "editor-transformar-deformar");
        assert!(editor.read_with(&ve, |ed, _| ed.deformando()));
        assert!(ve.debug_bounds("editor-opcoes-do-deformar").is_some());
        let arrastar_o_ponto = |ve: &mut VisualTestContext| {
            // Um quadro novo: a barra de opções trocou e o palco andou.
            ve.update(|window, _| window.refresh());
            ve.run_until_parked();
            let (_, m) = editor
                .read_with(ve, |ed, _| ed.sessao().unwrap().malha())
                .unwrap();
            let (x, y) = m.pontos[3][1];
            let a = ponto_da_foto(ve, &editor, (x / 64.0, y / 48.0));
            let desenhado = ve
                .debug_bounds("editor-malha-3-1")
                .expect("o ponto")
                .center();
            assert!(
                (desenhado.x - a.x).abs() < gpui_kit::px(2.)
                    && (desenhado.y - a.y).abs() < gpui_kit::px(2.),
                "o ponto desenhado ({desenhado:?}) está onde ele pega ({a:?})"
            );
            let b = a - gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(60.));
            arrastar_com(
                ve,
                a,
                b,
                gpui_kit::Modifiers::none(),
                gpui_kit::Modifiers::none(),
            );
        };
        let malha = |ve: &VisualTestContext| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().malha().map(|(_, m)| m.pontos)
            })
        };
        let m0 = malha(&ve);
        arrastar_o_ponto(&mut ve);
        assert_ne!(malha(&ve), m0, "o ponto andou");
        assert_ne!(
            doc(&ve).camadas[2].pixels,
            antes.camadas[2].pixels,
            "a prévia"
        );
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert_eq!(doc(&ve), antes, "Esc restaura");
        assert_eq!(passos_de(&editor, &ve), passos_antes);

        // 7. De novo, pelo ⌘T e o menu de contexto, e Enter: um passo.
        ve.simulate_keystrokes("cmd-t");
        ve.run_until_parked();
        let p = ponto_da_foto(&mut ve, &editor, (0.5, 0.6));
        ve.simulate_mouse_down(p, gpui_kit::MouseButton::Right, nada);
        ve.simulate_mouse_up(p, gpui_kit::MouseButton::Right, nada);
        ve.run_until_parked();
        clicar_no_editor(&mut ve, "editor-contexto-deformar");
        assert!(editor.read_with(&ve, |ed, _| ed.deformando()));
        arrastar_o_ponto(&mut ve);
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.transformando()));
        assert_eq!(passos_de(&editor, &ve), passos_antes + 1);
        let deformado = doc(&ve);
        assert_ne!(deformado.camadas[2].pixels, antes.camadas[2].pixels);
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(doc(&ve), antes);
        ve.simulate_keystrokes("cmd-shift-z");
        ve.run_until_parked();
        assert_eq!(doc(&ve), deformado);

        // 8. A recuperação (J, ⇧J) numa camada vazia por cima, com a origem
        // escolhida com ⌥ + clique.
        ve.simulate_keystrokes("cmd-shift-n j shift-j");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(editor_core::Ferramenta::Recuperacao)
        );
        let origem = ponto_da_foto(&mut ve, &editor, (0.15, 0.2));
        ve.simulate_mouse_down(origem, gpui_kit::MouseButton::Left, alt);
        ve.simulate_mouse_up(origem, gpui_kit::MouseButton::Left, alt);
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.sessao().unwrap().origem().is_some()));
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.pincel.raio = 4.0);
            ed.tracar_para_teste((40., 30.), (44., 34.), cx)
        });
        let d = doc(&ve);
        assert_eq!(d.camadas.len(), 5);
        // A nova entrou logo acima da escolhida (a recortada), fora do
        // conjunto — acima dela não há recortada.
        assert!(!d.camadas[3].recortada);
        assert!(
            !d.camadas[3].pixels.vazia(),
            "a recuperação pintou na camada nova"
        );
        assert_eq!(
            d.camadas[2].pixels, deformado.camadas[2].pixels,
            "o deformado ficou"
        );

        // 9. Salvar com as camadas separadas, fechar e reabrir.
        let (janela_do_editor, _) = o_editor(&m, cx);
        let salvo = doc(&ve);
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.salvar(false, window, cx))
        })
        .unwrap();
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.alterado()), "salvo");
        let revisao = m
            .janela
            .update(cx, |app, _w, cx| app.revelacao.read(cx).revisao_da_aberta())
            .unwrap();
        assert_eq!(revisao, 1, "a Revelação usa a imagem editada");
        cx.update_window(janela_do_editor, |_, window, cx| {
            editor.update(cx, |ed, cx| ed.fechar(window, cx))
        })
        .unwrap();
        drop(editor);
        drop(ve);
        cx.run_until_parked();
        m.janela
            .update(cx, |app, _w, cx| {
                app.revelacao
                    .update(cx, |tela, cx| tela.pedir_edicao_para_teste(0, cx));
            })
            .unwrap();
        cx.run_until_parked();
        let (_, reaberto) = o_editor(&m, cx);
        let d = reaberto.read_with(cx, |ed, _| ed.sessao().unwrap().documento().clone());
        assert_eq!(d, salvo, "camadas, recorte e pixels voltam");
        assert!(d.camadas[2].recortada);

        // 10. Mesclar a recortada na base: a foto não muda.
        let foto = reaberto.read_with(cx, |ed, _| ed.sessao().unwrap().compor());
        reaberto.update(cx, |ed, cx| {
            ed.escolher_camada(2, cx);
            ed.mesclar_para_baixo(cx);
        });
        let (foto2, quantas) = reaberto.read_with(cx, |ed, _| {
            let s = ed.sessao().unwrap();
            (s.compor(), s.documento().camadas.len())
        });
        assert_eq!(quantas, 4);
        assert_eq!(foto2, foto, "mesclar não muda a foto");
    }

    /// 🎭 Etapa 16 pela tela: a máscara pelo botão, as Propriedades dela
    /// (inverter, ver sozinha, rubi), a corrente, os cadeados (clique e `/`),
    /// ⌘I, ⌘C ⌘V ⇧⌘V, ⇧⌥⌘E e arrastar a camada no painel.
    #[gpui_kit::test]
    fn a_mascara_os_cadeados_e_a_area_de_transferencia_pela_tela(cx: &mut TestAppContext) {
        use editor_core::Exibicao;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let doc = |ve: &VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().documento().clone())
        };
        let redesenhar = |ve: &mut VisualTestContext| {
            ve.update(|window, _| window.refresh());
            ve.run_until_parked();
        };

        // Uma camada pintada inteira e a máscara pelo botão.
        ve.simulate_keystrokes("cmd-a alt-backspace cmd-d");
        ve.run_until_parked();
        assert_eq!(doc(&ve).camadas[0].pixels.pixel(5, 5)[3], 255);
        clicar_no_editor(&mut ve, "editor-camada-mascara");
        redesenhar(&mut ve);
        assert!(doc(&ve).camadas[0].mascara.is_some());
        assert!(
            ve.debug_bounds("editor-propriedades-da-mascara").is_some(),
            "as Propriedades da máscara no painel"
        );

        // Inverter pelo botão; ⌥ + clique na miniatura mostra só a máscara;
        // `\` troca para o rubi.
        clicar_no_editor(&mut ve, "editor-mascara-inverter");
        assert_eq!(doc(&ve).camadas[0].mascara.as_ref().unwrap().fundo, 0);
        let miniatura = ve.debug_bounds("editor-mascara-0").expect("a miniatura");
        ve.simulate_click(
            miniatura.center(),
            gpui_kit::Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.exibicao()),
            Exibicao::SoAMascara(0)
        );
        ve.simulate_keystrokes("\\");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.exibicao()),
            Exibicao::Rubi(0)
        );
        ve.simulate_keystrokes("\\");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.exibicao()), Exibicao::Foto);
        // ⌘I na máscara escolhida volta a revelar.
        ve.simulate_keystrokes("cmd-i");
        ve.run_until_parked();
        assert_eq!(doc(&ve).camadas[0].mascara.as_ref().unwrap().fundo, 255);

        // A corrente entre as miniaturas.
        redesenhar(&mut ve);
        clicar_no_editor(&mut ve, "editor-corrente-0");
        assert!(!doc(&ve).camadas[0].mascara.as_ref().unwrap().vinculada);

        // Os cadeados: o dos pixels pelo clique (o pincel recusa e avisa),
        // e `/` para a transparência.
        let miniatura = ve.debug_bounds("editor-miniatura-0").expect("a camada");
        ve.simulate_click(miniatura.center(), gpui_kit::Modifiers::none());
        ve.run_until_parked();
        clicar_no_editor(&mut ve, "editor-cadeado-pixels");
        redesenhar(&mut ve);
        assert!(doc(&ve).camadas[0].bloqueio.pixels);
        assert!(ve.debug_bounds("editor-cadeado-da-camada-0").is_some());
        ve.simulate_keystrokes("b");
        let p = ponto_da_foto(&mut ve, &editor, (0.5, 0.5));
        ve.simulate_click(p, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let aviso = editor.read_with(&ve, |ed, _| ed.aviso().map(|(t, e)| (t.to_string(), e)));
        assert!(
            aviso
                .as_ref()
                .is_some_and(|(t, erro)| *erro && t.contains("pixels da camada estão bloqueados")),
            "{aviso:?}"
        );
        ve.simulate_keystrokes("/");
        ve.run_until_parked();
        assert!(doc(&ve).camadas[0].bloqueio.transparencia);
        clicar_no_editor(&mut ve, "editor-cadeado-tudo");
        clicar_no_editor(&mut ve, "editor-cadeado-tudo");
        assert!(
            !doc(&ve).camadas[0].bloqueio.algum(),
            "tudo e de novo solta"
        );

        // ⌘C ⌘V: a cópia numa camada nova acima; ⇧⌘V também.
        ve.simulate_keystrokes("cmd-a cmd-c");
        ve.run_until_parked();
        ve.simulate_keystrokes("cmd-v");
        ve.run_until_parked();
        let (nomes, ativa) = camadas(&editor, &ve);
        assert_eq!(nomes, vec!["Pintura".to_string(), "Camada 1".to_string()]);
        assert_eq!(ativa, 1);
        assert_eq!(
            doc(&ve).camadas[1].pixels.pixel(3, 3),
            doc(&ve).camadas[0].pixels.pixel(3, 3)
        );
        ve.simulate_keystrokes("cmd-shift-v");
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).0.len(), 3);

        // ⇧⌥⌘E: o visível numa camada nova (composta em segundo plano).
        let foto = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().compor());
        ve.simulate_keystrokes("cmd-shift-alt-e");
        ve.run_until_parked();
        assert_eq!(camadas(&editor, &ve).0.len(), 4);
        let d = doc(&ve);
        let carimbo = &d.camadas[3].pixels;
        assert_eq!(carimbo.pixel(7, 9)[..3], foto.get_pixel(7, 9).0);

        // Arrastar a de cima para o fundo da pilha.
        redesenhar(&mut ve);
        let de = ve
            .debug_bounds("editor-camada-3")
            .expect("a linha de cima")
            .center();
        let para = ve
            .debug_bounds("editor-camada-0")
            .expect("a linha de baixo")
            .center();
        ve.simulate_mouse_down(de, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
        ve.run_until_parked();
        let meio = gpui_kit::point(de.x, (de.y + para.y) / 2.);
        ve.simulate_mouse_move(
            meio,
            Some(gpui_kit::MouseButton::Left),
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        ve.simulate_mouse_move(
            para,
            Some(gpui_kit::MouseButton::Left),
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        ve.simulate_mouse_up(
            para,
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        let (nomes, ativa) = camadas(&editor, &ve);
        assert_eq!(nomes[0], "Camada 3", "{nomes:?}");
        assert_eq!(ativa, 0);
        let passos = passos_de(&editor, &ve);
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(
            passos_de(&editor, &ve),
            passos,
            "o desfazer não apaga passos"
        );
        assert_eq!(
            camadas(&editor, &ve).0[3],
            "Camada 3",
            "um desfazer volta tudo"
        );
    }

    /// 📈 Curvas pela tela: o menu de ajustes cria, o clique no gráfico põe
    /// um ponto, o arrasto escurece os meios-tons num passo só, arrastar
    /// para fora tira o ponto, e os canais têm curva própria.
    #[gpui_kit::test]
    fn as_curvas_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        clicar_no_editor(&mut ve, "editor-camada-ajuste");
        clicar_no_editor(&mut ve, "editor-ajuste-novo-curvas");
        let (nomes, _) = camadas(&editor, &ve);
        assert_eq!(nomes.last().map(String::as_str), Some("Curvas 1"));
        let painel = caixa(&mut ve, "editor-painel-propriedades");
        let grafico = ve.debug_bounds("editor-curva").expect("o gráfico");
        assert!(
            grafico.bottom() <= painel.bottom() + gpui_kit::px(1.),
            "o gráfico inteiro à vista no grupo padrão: {grafico:?} em {painel:?}"
        );
        let no_grafico = |x: f32, y: f32| {
            grafico.origin
                + gpui_kit::point(
                    grafico.size.width * (x / 255.0),
                    grafico.size.height * (1.0 - y / 255.0),
                )
        };
        let cor = |ve: &mut VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().cor_em(42.0, 32.0).unwrap())
        };
        let antes = cor(&mut ve); // (126, 128, 90)
        let passos = passos_de(&editor, &ve);
        let nada = gpui_kit::Modifiers::none();
        arrastar_com(
            &mut ve,
            no_grafico(128.0, 128.0),
            no_grafico(128.0, 80.0),
            nada,
            nada,
        );
        let depois = cor(&mut ve);
        assert!(
            depois[1] < antes[1] - 30,
            "os meios-tons escureceram: {antes:?} → {depois:?}"
        );
        assert_eq!(passos_de(&editor, &ve), passos + 1, "um passo pelo arrasto");
        assert!(ve.debug_bounds("editor-curva-ponto-1").is_some());
        // Arrastar para fora tira o ponto: a curva volta à reta.
        let p = no_grafico(128.0, 80.0);
        let fora = gpui_kit::point(p.x, grafico.origin.y - gpui_kit::px(80.));
        arrastar_com(&mut ve, p, fora, nada, nada);
        assert_eq!(cor(&mut ve), antes);
        assert!(ve.debug_bounds("editor-curva-ponto-2").is_none());
        // O canal vermelho tem a curva dele.
        assert!(ve.debug_bounds("editor-curva-canal").is_some());
        editor.update(&mut ve, |ed, cx| ed.escolher_canal_da_curva(1, cx));
        ve.run_until_parked();
        arrastar_com(
            &mut ve,
            no_grafico(128.0, 128.0),
            no_grafico(128.0, 200.0),
            nada,
            nada,
        );
        let vermelho = cor(&mut ve);
        assert!(
            vermelho[0] > antes[0] + 30 && vermelho[1] == antes[1],
            "{vermelho:?}"
        );
        // Desfazer volta passo a passo.
        ve.simulate_keystrokes("cmd-z");
        ve.run_until_parked();
        assert_eq!(cor(&mut ve), antes);
    }

    /// 🔁 Antes/Depois: Y (e o botão) mostra a foto como abriu, sem passo no
    /// Histórico nem "alterado" novo; uma pincelada volta ao depois.
    #[gpui_kit::test]
    fn o_antes_depois_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| s.pincel.cor = [255, 0, 0]);
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        let passos = passos_de(&editor, &ve);
        let vista = |ve: &VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().vista().imagem().clone())
        };
        let depois = vista(&ve);
        ve.simulate_keystrokes("y");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.mostrando_antes()));
        assert!(ve.debug_bounds("editor-selo-antes").is_some());
        assert_ne!(vista(&ve), depois, "a tela mostra o antes");
        assert_eq!(passos_de(&editor, &ve), passos, "nada no Histórico");
        pelo_menu(&mut ve, "editor-menu-visualizar", "editor-antes-depois");
        assert!(!editor.read_with(&ve, |ed, _| ed.mostrando_antes()));
        assert_eq!(vista(&ve), depois);
        // Pintar com o antes ligado volta ao depois.
        ve.simulate_keystrokes("y");
        ve.run_until_parked();
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 10.), (20., 10.), cx)
        });
        assert!(!editor.read_with(&ve, |ed, _| ed.mostrando_antes()));
    }

    /// 🫧 Liquidificar pela tela: ⇧⌘X, arrastar empurra a camada, Esc devolve,
    /// de novo e Enter num passo "Liquidificar".
    #[gpui_kit::test]
    fn o_liquidificar_pela_tela(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        // Uma faixa escura vertical em x = 30..33 sobre fundo claro.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.pincel.cor = [230, 230, 230];
                s.selecionar_tudo();
                s.preencher_selecao();
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(30, 0, 3, 48)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [10, 10, 10];
                s.preencher_selecao();
                s.desmarcar();
                s.pincel.raio = 8.0;
                s.forca_do_liquido = 1.0;
            })
        });
        let pixel = |ve: &VisualTestContext, x: u32, y: u32| {
            editor.read_with(ve, |ed, _| {
                ed.sessao().unwrap().documento().camadas[0]
                    .pixels
                    .pixel(x, y)
            })
        };
        let original = editor.read_with(&ve, |ed, _| {
            ed.sessao().unwrap().documento().camadas[0].pixels.clone()
        });
        let passos = passos_de(&editor, &ve);
        ve.simulate_keystrokes("cmd-shift-x");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.liquidificando()));
        assert!(ve.debug_bounds("editor-opcoes-do-liquidificar").is_some());
        let nada = gpui_kit::Modifiers::none();
        let a = ponto_da_foto(&mut ve, &editor, (31.5 / 64.0, 24.0 / 48.0));
        let b = ponto_da_foto(&mut ve, &editor, (39.5 / 64.0, 24.0 / 48.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        assert!(
            pixel(&ve, 38, 24)[0] < 100,
            "a faixa andou: {:?}",
            pixel(&ve, 38, 24)
        );
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.liquidificando()));
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.sessao().unwrap().documento().camadas[0]
                .pixels
                .clone()),
            original,
            "Esc devolve"
        );
        ve.simulate_keystrokes("cmd-shift-x");
        ve.run_until_parked();
        arrastar_com(&mut ve, a, b, nada, nada);
        ve.simulate_keystrokes("enter");
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.liquidificando()));
        assert_eq!(passos_de(&editor, &ve), passos + 1, "um passo");
        assert!(pixel(&ve, 38, 24)[0] < 100);
    }

    /// 🩹 O Remendo pela tela: J/⇧J até ele, o laço à mão sobre a mancha, e
    /// arrastar a seleção até a pele limpa — um passo "Remendo".
    #[gpui_kit::test]
    fn o_remendo_pela_tela(cx: &mut TestAppContext) {
        use crate::editor::janela::{Auxiliar, Item};
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        // Uma mancha escura na foto (na Pintura), e uma camada vazia por cima.
        editor.update(&mut ve, |ed, cx| {
            ed.na_sessao_para_teste(cx, |s| {
                s.selecionar(
                    &editor_core::Forma::Retangulo(editor_core::Retangulo::novo(40, 20, 6, 6)),
                    editor_core::Operacao::Nova,
                );
                s.pincel.cor = [0, 0, 0];
                s.preencher_selecao();
                s.desmarcar();
                s.nova_camada();
            })
        });
        ve.simulate_keystrokes("j shift-j shift-j");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.item_atual()),
            Some(Item::A(Auxiliar::Remendo))
        );
        let nada = gpui_kit::Modifiers::none();
        // O laço à mão em volta da mancha.
        let pts: Vec<_> = [(38.0, 18.0), (48.0, 18.0), (48.0, 28.0), (38.0, 28.0)]
            .iter()
            .map(|&(x, y)| ponto_da_foto(&mut ve, &editor, (x / 64.0, y / 48.0)))
            .collect();
        ve.simulate_mouse_down(pts[0], gpui_kit::MouseButton::Left, nada);
        for p in &pts[1..] {
            ve.simulate_mouse_move(*p, Some(gpui_kit::MouseButton::Left), nada);
        }
        ve.simulate_mouse_up(pts[3], gpui_kit::MouseButton::Left, nada);
        ve.run_until_parked();
        assert!(selecao_de(&editor, &ve).is_some(), "o laço do Remendo");
        let passos = passos_de(&editor, &ve);
        // Arrastar de dentro da seleção 20 px para a esquerda: a origem limpa.
        let a = ponto_da_foto(&mut ve, &editor, (43.0 / 64.0, 23.0 / 48.0));
        let b = ponto_da_foto(&mut ve, &editor, (23.0 / 64.0, 23.0 / 48.0));
        arrastar_com(&mut ve, a, b, nada, nada);
        assert_eq!(passos_de(&editor, &ve), passos + 1);
        let (cor, nova) = editor.read_with(&ve, |ed, _| {
            let s = ed.sessao().unwrap();
            (
                s.cor_em(42.0, 22.0).unwrap(),
                s.documento().camadas[1].pixels.vazia(),
            )
        });
        assert!(cor[0] > 60, "a mancha preta sumiu: {cor:?}");
        assert!(!nova, "pintou na camada de cima");
    }

    // ------------------------------------------------- a área de trabalho

    fn caixa(ve: &mut VisualTestContext, alvo: &'static str) -> gpui_kit::Bounds<gpui_kit::Pixels> {
        ve.debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{alvo} não está desenhado"))
    }

    /// 🧭 As regiões do Photoshop, cada uma no lugar: menus em cima, a barra
    /// de opções embaixo deles, ferramentas à esquerda, a aba do documento em
    /// cima do palco, os painéis à direita e o status embaixo. E a barra de
    /// opções não muda de altura com a ferramenta: o palco fica parado.
    #[gpui_kit::test]
    fn as_regioes_da_area_de_trabalho_e_o_palco_parado(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let menus = caixa(&mut ve, "editor-menu-arquivo");
        let opcoes = caixa(&mut ve, "editor-barra-de-opcoes");
        let barra = caixa(&mut ve, "editor-barra-de-ferramentas");
        let aba = caixa(&mut ve, "editor-aba-do-documento");
        let palco = caixa(&mut ve, "palco-do-editor");
        let paineis = caixa(&mut ve, "editor-coluna-de-paineis");
        let status = caixa(&mut ve, "editor-barra-de-status");
        assert!(
            menus.bottom() <= opcoes.top() + gpui_kit::px(1.),
            "menus em cima das opções"
        );
        assert!(
            opcoes.bottom() <= barra.top() + gpui_kit::px(1.),
            "opções em cima das ferramentas"
        );
        assert!(
            barra.right() <= palco.left() + gpui_kit::px(1.),
            "ferramentas à esquerda"
        );
        assert!(
            aba.bottom() <= palco.top() + gpui_kit::px(1.),
            "a aba em cima do palco"
        );
        assert!(
            palco.right() <= paineis.left() + gpui_kit::px(1.),
            "painéis à direita"
        );
        assert!(
            status.top() >= palco.bottom() - gpui_kit::px(1.),
            "status embaixo"
        );
        assert!((opcoes.size.height - gpui_kit::px(36.)).abs() < gpui_kit::px(1.));
        // Pincel, laço, varinha, mão e lupa: o palco não anda.
        for tecla in ["l", "w", "h", "z", "j", "b"] {
            ve.simulate_keystrokes(tecla);
            ve.run_until_parked();
            assert_eq!(caixa(&mut ve, "palco-do-editor"), palco, "com {tecla}");
        }
        // Nem com a transformação aberta (Cancelar e Aplicar à direita).
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        ve.simulate_keystrokes("cmd-t");
        ve.run_until_parked();
        assert!(editor.read_with(&ve, |ed, _| ed.transformando()));
        assert_eq!(caixa(&mut ve, "palco-do-editor"), palco, "com ⌘T");
        let aplicar = caixa(&mut ve, "editor-aplicar-transformacao");
        assert!(aplicar.right() <= opcoes.right() + gpui_kit::px(1.));
        clicar_no_editor(&mut ve, "editor-aplicar-transformacao");
        assert!(!editor.read_with(&ve, |ed, _| ed.transformando()));
    }

    /// 🧰 Os grupos da barra: uma ferramenta por grupo; o botão direito abre o
    /// flyout dentro da janela; a escolhida vira o ícone do grupo; a letra
    /// volta a ela e ⇧ + letra anda; Esc fecha; ↑ ↓ Enter escolhem; o aperto
    /// longo também abre e não usa a ferramenta.
    #[gpui_kit::test]
    fn os_grupos_da_barra_e_o_flyout(cx: &mut TestAppContext) {
        use crate::editor::janela::{Auxiliar, Item};
        use editor_core::Ferramenta;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        assert!(
            ve.debug_bounds("editor-correcao").is_some(),
            "o J mostra a correção"
        );
        assert!(
            ve.debug_bounds("editor-recuperacao").is_none(),
            "e esconde as outras"
        );

        pelo_flyout(&mut ve, "editor-recuperacao");
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Recuperacao)
        );
        assert_eq!(editor.read_with(&ve, |ed, _| ed.flyout_aberto()), None);
        assert!(
            ve.debug_bounds("editor-recuperacao").is_some(),
            "o grupo mostra a escolhida"
        );
        assert!(ve.debug_bounds("editor-correcao").is_none());
        // A letra volta à última do grupo; ⇧ + letra anda.
        ve.simulate_keystrokes("b j");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Recuperacao)
        );
        ve.simulate_keystrokes("shift-j");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.item_atual()),
            Some(Item::A(Auxiliar::Remendo))
        );
        assert!(ve.debug_bounds("editor-remendo").is_some());

        // O flyout fica dentro da janela, e Esc fecha sem trocar nada.
        let grupo = caixa(&mut ve, "editor-grupo-12");
        ve.simulate_mouse_down(
            grupo.center(),
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        ve.simulate_mouse_up(
            grupo.center(),
            gpui_kit::MouseButton::Right,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.flyout_aberto()), Some(12));
        let flyout = caixa(&mut ve, "editor-flyout");
        let janela = ve.update(|window, _| window.viewport_size());
        assert!(flyout.left() >= grupo.right(), "ao lado do botão");
        assert!(flyout.right() <= janela.width && flyout.bottom() <= janela.height);
        ve.simulate_keystrokes("escape");
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.flyout_aberto()), None);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.item_atual()),
            Some(Item::A(Auxiliar::Remendo)),
            "Esc não troca a ferramenta"
        );
        // Pelo teclado: ↓ e Enter escolhem a segunda (Girar vista).
        editor.update(&mut ve, |ed, cx| ed.abrir_flyout(12, cx));
        ve.simulate_keystrokes("down enter");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(Auxiliar::GirarVista)
        );
        // O aperto longo no grupo do laço abre o flyout, e o soltar não usa
        // a ferramenta.
        let laco = caixa(&mut ve, "editor-grupo-2");
        ve.simulate_mouse_down(
            laco.center(),
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.executor()
            .advance_clock(std::time::Duration::from_millis(500));
        ve.run_until_parked();
        assert_eq!(editor.read_with(&ve, |ed, _| ed.flyout_aberto()), Some(2));
        ve.simulate_mouse_up(
            laco.center(),
            gpui_kit::MouseButton::Left,
            gpui_kit::Modifiers::none(),
        );
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.auxiliar()),
            Some(Auxiliar::GirarVista),
            "o aperto longo não escolheu o laço"
        );
        assert!(
            !editor.read_with(&ve, |ed, _| ed.alterado()),
            "nada foi pintado pela barra"
        );
    }

    /// ⇥ Tab esconde ferramentas, opções e painéis (o palco cresce); ⇧Tab só
    /// os painéis. Não grava a arrumação, e não reenvia ladrilhos à GPU.
    #[gpui_kit::test]
    fn tab_e_shift_tab_escondem_sem_gravar_nem_reenviar(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        let palco = caixa(&mut ve, "palco-do-editor");
        let gravacoes = editor.read_with(&ve, |ed, _| ed.gravacoes_da_area_de_trabalho());
        ve.simulate_keystrokes("tab");
        ve.run_until_parked();
        for fora in [
            "editor-barra-de-ferramentas",
            "editor-barra-de-opcoes",
            "editor-coluna-de-paineis",
        ] {
            assert!(ve.debug_bounds(fora).is_none(), "{fora} some com o Tab");
        }
        let grande = caixa(&mut ve, "palco-do-editor");
        assert!(grande.size.width > palco.size.width && grande.size.height > palco.size.height);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.medidas().ladrilhos_no_quadro),
            0,
            "esconder não reenvia a foto"
        );
        ve.simulate_keystrokes("tab");
        ve.run_until_parked();
        assert_eq!(caixa(&mut ve, "palco-do-editor"), palco, "volta tudo");
        ve.simulate_keystrokes("shift-tab");
        ve.run_until_parked();
        assert!(ve.debug_bounds("editor-coluna-de-paineis").is_none());
        assert!(ve.debug_bounds("editor-barra-de-ferramentas").is_some());
        assert!(ve.debug_bounds("editor-barra-de-opcoes").is_some());
        ve.simulate_keystrokes("shift-tab");
        ve.run_until_parked();
        assert!(ve.debug_bounds("editor-coluna-de-paineis").is_some());
        ve.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.gravacoes_da_area_de_trabalho()),
            gravacoes,
            "esconder é passageiro: nada gravado"
        );
        assert!(editor.read_with(&ve, |ed, _| ed.arranjo().ocultos.is_empty()));
    }

    /// 🖐️ O Espaço segurado é a Mão por um instante; ao soltar, a ferramenta
    /// é a mesma, e tocar sem arrastar não mexe no zoom.
    #[gpui_kit::test]
    fn o_espaco_e_a_mao_temporaria(cx: &mut TestAppContext) {
        use crate::revelacao::zoom::Nivel;
        use editor_core::Ferramenta;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        ve.simulate_keystrokes("e");
        ve.run_until_parked();
        let nivel = editor.read_with(&ve, |ed, _| ed.nivel_do_zoom());
        editor.update(&mut ve, |ed, cx| ed.espaco_apertado(cx));
        assert!(editor.read_with(&ve, |ed, _| ed.espaco_segurado()));
        editor.update(&mut ve, |ed, cx| ed.espaco_solto(cx));
        ve.run_until_parked();
        assert!(!editor.read_with(&ve, |ed, _| ed.espaco_segurado()));
        assert_eq!(editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()), nivel);
        assert_eq!(nivel, Nivel::Encaixar);
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.ferramenta()),
            Some(Ferramenta::Borracha)
        );
    }

    /// 🗂️ Os painéis: o menu Janela esconde e traz de volta; recolher vira
    /// faixa de ícones e o ícone abre a coluna no painel; restaurar volta ao
    /// padrão. Tudo pelo menu (os mesmos métodos das teclas).
    #[gpui_kit::test]
    fn os_paineis_pelo_menu_janela(cx: &mut TestAppContext) {
        use crate::editor::janela::QualPainel;
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        assert_eq!(
            editor.read_with(&ve, |ed, cx| ed.paineis_no_dock(cx)).len(),
            6,
            "os seis painéis no dock"
        );
        assert!(ve.debug_bounds("editor-painel-camadas").is_some());
        assert!(ve.debug_bounds("editor-painel-propriedades").is_some());
        let camadas = caixa(&mut ve, "editor-painel-camadas");
        let propriedades = caixa(&mut ve, "editor-painel-propriedades");
        assert!(camadas.top() > propriedades.top(), "Camadas embaixo");
        assert!(camadas.size.height > propriedades.size.height * 0.6);

        pelo_menu(&mut ve, "editor-menu-janela", "editor-janela-camadas");
        assert!(
            ve.debug_bounds("editor-painel-camadas").is_none(),
            "escondido"
        );
        assert!(editor.read_with(&ve, |ed, _| !ed.painel_visivel(QualPainel::Camadas)));
        pelo_menu(&mut ve, "editor-menu-janela", "editor-janela-camadas");
        assert!(
            ve.debug_bounds("editor-painel-camadas").is_some(),
            "de volta"
        );

        // O Histórico, atrás de Propriedades, vem para a frente.
        assert!(ve.debug_bounds("editor-painel-historico").is_none());
        pelo_menu(&mut ve, "editor-menu-janela", "editor-janela-historico");
        assert!(ve.debug_bounds("editor-painel-historico").is_some());

        pelo_menu(&mut ve, "editor-menu-janela", "editor-janela-recolher");
        assert!(ve.debug_bounds("editor-faixa-de-icones").is_some());
        assert!(ve.debug_bounds("editor-painel-camadas").is_none());
        clicar_no_editor(&mut ve, "editor-icone-camadas");
        assert!(ve.debug_bounds("editor-faixa-de-icones").is_none());
        assert!(ve.debug_bounds("editor-painel-camadas").is_some());

        editor.update(&mut ve, |ed, cx| {
            ed.definir_painel_oculto(QualPainel::Cor, true, cx)
        });
        pelo_menu(&mut ve, "editor-menu-janela", "editor-restaurar-area");
        assert!(editor.read_with(&ve, |ed, _| ed.painel_visivel(QualPainel::Cor)));
        assert!(!editor.read_with(&ve, |ed, _| ed.arranjo().recolhido));
    }

    /// 💾 Uma arrumação gravada com painel desconhecido, repetido e grupo
    /// vazio: o que serve fica, o que falta ganha lugar — nenhum painel some.
    #[gpui_kit::test]
    fn a_arrumacao_estragada_nao_perde_painel(cx: &mut TestAppContext) {
        use gpui_kit::component::dock::{PanelInfo, PanelState};
        let (_m, editor, mut ve) = editor_aberto(cx);
        let abas = |nomes: &[&str]| PanelState {
            panel_name: String::new(),
            children: nomes.iter().map(|n| PanelState::new(*n)).collect(),
            info: PanelInfo::tabs(9),
        };
        let estragada = PanelState {
            panel_name: String::new(),
            children: vec![
                abas(&["editor:camadas", "editor:sumiu", "editor:camadas"]),
                abas(&[]),
                abas(&["editor:historico"]),
            ],
            info: PanelInfo::Stack {
                sizes: vec![
                    gpui_kit::px(-5.),
                    gpui_kit::px(f32::NAN),
                    gpui_kit::px(100.),
                ],
                axis: 1,
            },
        };
        editor.update(&mut ve, |ed, cx| {
            ed.remontar_a_area_com(Some(estragada), cx)
        });
        ve.update(|window, _| window.refresh());
        ve.run_until_parked();
        let mut no_dock = editor.read_with(&ve, |ed, cx| ed.paineis_no_dock(cx));
        no_dock.sort_by_key(|q| q.nome());
        no_dock.dedup();
        assert_eq!(no_dock.len(), 6, "os seis, uma vez cada: {no_dock:?}");
        assert!(ve.debug_bounds("editor-painel-camadas").is_some());
    }

    /// 📋 Os menus chamam o mesmo que as teclas: Editar › Desfazer desfaz o
    /// traço, e fica apagado sem nada para desfazer; o atalho escrito ao lado
    /// é o da plataforma.
    #[gpui_kit::test]
    fn os_menus_chamam_as_mesmas_acoes(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| {
            ed.tracar_para_teste((10., 24.), (54., 24.), cx)
        });
        ve.run_until_parked();
        let posicao = |ve: &VisualTestContext| {
            editor.read_with(ve, |ed, _| ed.sessao().unwrap().historico().posicao())
        };
        assert_eq!(posicao(&ve), 1);
        pelo_menu(&mut ve, "editor-menu-editar", "editor-menu-desfazer");
        assert_eq!(posicao(&ve), 0, "desfez pelo menu");
        pelo_menu(&mut ve, "editor-menu-editar", "editor-menu-refazer");
        assert_eq!(posicao(&ve), 1, "refez pelo menu");
        let foco = editor.read_with(&ve, |ed, cx| ed.focus_handle(cx));
        let atalho = ve.update(|window, _| {
            crate::editor::janela::menus_atalho(&crate::editor::DesfazerNoEditor, &foco, window)
        });
        assert_eq!(
            atalho.as_deref(),
            Some(if cfg!(target_os = "macos") {
                "⌘Z"
            } else {
                "Ctrl+Z"
            })
        );
        // Z é a Lupa também pelo menu da Ajuda (a tabela é a mesma).
        pelo_menu(&mut ve, "editor-menu-ajuda", "editor-ajuda-atalhos");
        assert!(ve.debug_bounds("editor-dialogo-atalhos").is_some());
    }

    /// 🎨 As duas cores na barra: ⇄ troca (como o X) e o quadradinho volta a
    /// preto e branco (como o D).
    #[gpui_kit::test]
    fn as_cores_da_barra_trocam_e_voltam(cx: &mut TestAppContext) {
        let (_m, editor, mut ve) = editor_aberto(cx);
        ve.update(|window, _| window.activate_window());
        editor.update(&mut ve, |ed, cx| ed.escolher_cor([200, 30, 30], cx));
        clicar_no_editor(&mut ve, "editor-trocar-cores");
        let (frente, fundo) = editor.read_with(&ve, |ed, _| {
            let p = ed.sessao().unwrap().pincel;
            (p.cor, p.cor_de_fundo)
        });
        assert_eq!((frente, fundo), ([255; 3], [200, 30, 30]));
        clicar_no_editor(&mut ve, "editor-cores-padrao");
        let p = editor.read_with(&ve, |ed, _| ed.sessao().unwrap().pincel);
        assert_eq!((p.cor, p.cor_de_fundo), ([0; 3], [255; 3]));
    }
}
