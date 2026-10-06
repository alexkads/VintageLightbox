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

        // E a opção logo abaixo continua lá: ↓ ×4 é "Escolher também".
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
        visual.simulate_keystrokes("down down down down enter");
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
        // Z tocado volta ao encaixe e, de novo, ao 1:1.
        ve.simulate_keystrokes("z");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            Nivel::Encaixar
        );
        ve.simulate_keystrokes("z");
        ve.run_until_parked();
        assert_eq!(
            editor.read_with(&ve, |ed, _| ed.nivel_do_zoom()),
            Nivel::Razao(1.0)
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
                s.base().get_pixel(o.0 as u32, o.1 as u32).0,
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
        editor.update(&mut ve, |ed, cx| ed.escolher_camada(0, cx));
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
        assert_eq!(depois, passos + 1);
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

        ve.simulate_keystrokes("shift-r");
        ve.run_until_parked();
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
        clicar_no_editor(&mut ve, "editor-aba-historico");
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
}
