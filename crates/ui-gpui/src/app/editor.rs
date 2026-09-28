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
use gpui_kit::{AppContext, Context, Entity, Subscription};

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
    abertas: HashMap<String, (gpui_kit::AnyWindowHandle, Entity<EditorDeFoto>)>,
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
            .insert(chave.clone(), (janela.into(), editor));
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
        self.revelacao.update(cx, |tela, cx| {
            tela.fonte_mudou(&foto.id, foto.pos_venda_foto_id.as_deref(), cx)
        });
        if let Some(no_site) = foto.pos_venda_foto_id.clone() {
            let (ajustes, corte) = self.receita_para_subir(foto, cx);
            if let Some(ja) = self.a_subir.iter_mut().find(|(id, _, _)| *id == no_site) {
                *ja = (no_site, ajustes, corte);
            } else {
                self.a_subir.push((no_site, ajustes, corte));
            }
            self.recontar_o_que_falta_subir(cx);
        }
        cx.notify();
    }

    /// A receita **atual** da foto: a da tela, se ela está no palco; senão a
    /// que a foto guarda.
    fn receita_para_subir(
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

    /// As janelas do editor abertas (testes e roteiro).
    pub fn editores_abertos(&self) -> Vec<Entity<EditorDeFoto>> {
        self.editores
            .abertas
            .values()
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// A porta das edições, para quem monta o app.
    pub fn definir_edicoes(&mut self, edicoes: Arc<dyn Edicoes>, cx: &mut Context<Self>) {
        self.revelacao
            .update(cx, |tela, _| tela.definir_edicoes(edicoes));
    }
}
