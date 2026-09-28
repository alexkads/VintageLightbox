//! A janela do editor em camadas — uma por foto.
//!
//! ```text
//! ┌ ✎ img0042.jpg  • Alterações não salvas        ↶  ↷   [Salvar]  [Fechar] ┐
//! ├───────────────────────────────────────────────────────────┬────────────┤
//! │                                                           │ Ferramenta │
//! │                  a foto (vista em ladrilhos)              │ Tamanho …  │
//! │                                                           │ Camada 👁  │
//! └───────────────────────────────────────────────────────────┴────────────┘
//! ```
//!
//! 🔑 **A janela não sabe da Revelação.** Ela carrega a base neutra, pinta,
//! salva pela porta e anuncia [`EventoDoEditor::Salva`]; quem troca a fonte da
//! Revelação é a raiz (`docs/editor-em-camadas/02-CONTRATO.md`).
//!
//! 🔑 **A foto pinta em resolução cheia e a tela vê a vista reduzida**, em
//! ladrilhos de 256 px: cada gesto só reenvia à GPU os ladrilhos que sujou
//! (`editor-core/src/vista.rs`) — é o que deixa o pincel acompanhar a mão numa
//! foto de 24 MP.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use editor_core::{BaseRef, Documento, Ferramenta, Historico, Sessao, VersaoEditada};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Disableable};
use gpui_kit::{
    canvas, div, img, prelude::*, px, Bounds, Context, Entity, EventEmitter, FocusHandle,
    Focusable, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point,
    RenderImage, SharedString, Subscription, Task, Window,
};
use image::DynamicImage;

use super::porta::{Abertura, Edicoes, FotoDoEditor};
use super::{
    AlternarCamada, DesfazerNoEditor, FecharEditor, PincelMaior, PincelMenor, RefazerNoEditor,
    SalvarNoEditor, UsarBorracha, UsarPincel, CONTEXTO,
};
use crate::tema;

/// O maior lado da vista reduzida, em pixels: o dobro de uma tela de ~1000 pt,
/// que é o que a janela mostra numa tela retina.
pub const LADO_DA_VISTA: u32 = 2048;

/// As cores das amostras do pincel.
const AMOSTRAS: [[u8; 3]; 10] = [
    [0, 0, 0],
    [255, 255, 255],
    [128, 128, 128],
    [200, 30, 30],
    [240, 140, 20],
    [240, 220, 40],
    [40, 170, 70],
    [40, 120, 220],
    [140, 60, 200],
    [120, 80, 50],
];

/// O que a janela anuncia.
#[derive(Clone, Debug)]
pub enum EventoDoEditor {
    /// A edição foi salva e confirmada no catálogo. `versao` é `None` quando o
    /// projeto não muda a foto (C30): a Revelação volta ao bruto.
    Salva {
        foto: FotoDoEditor,
        versao: Option<VersaoEditada>,
    },
}

impl EventEmitter<EventoDoEditor> for EditorDeFoto {}

enum Fase {
    Carregando,
    Falhou(String),
    Pronta(Box<Sessao>),
}

/// O que a medida da responsividade guarda (`medir-editor` e o roteiro).
#[derive(Default, Clone, Copy, Debug)]
pub struct Medidas {
    /// O último gesto do pincel: da chegada do evento à vista refeita.
    pub ultimo_gesto: Option<std::time::Duration>,
    /// Quantos ladrilhos subiram para a GPU no último quadro.
    pub ladrilhos_no_quadro: usize,
    /// O último salvamento inteiro (compor, codificar, gravar, confirmar).
    pub ultimo_salvamento: Option<std::time::Duration>,
}

pub struct EditorDeFoto {
    foto: FotoDoEditor,
    edicoes: Arc<dyn Edicoes>,
    fase: Fase,
    ladrilhos: HashMap<(u32, u32), Arc<RenderImage>>,
    /// Onde a foto está desenhada, medida no quadro anterior.
    palco: Bounds<Pixels>,
    ponteiro: Option<Point<Pixels>>,
    pintando: bool,
    salvando: bool,
    /// O aviso da barra: o texto, e se é erro.
    aviso: Option<(SharedString, bool)>,
    /// "Fechar" com alterações pendentes: a pergunta está aberta.
    perguntando: bool,
    /// Fechar sem perguntar (depois de Salvar ou Descartar na pergunta).
    liberada: bool,
    foco: FocusHandle,
    tamanho: Entity<SliderState>,
    dureza: Entity<SliderState>,
    opacidade: Entity<SliderState>,
    opacidade_da_camada: Entity<SliderState>,
    medidas: Medidas,
    _assinaturas: Vec<Subscription>,
    _tarefa: Option<Task<()>>,
}

fn slider(
    min: f32,
    max: f32,
    passo: f32,
    valor: f32,
    cx: &mut Context<EditorDeFoto>,
) -> Entity<SliderState> {
    cx.new(|_| {
        SliderState::new()
            .min(min)
            .max(max)
            .step(passo)
            .default_value(valor)
    })
}

fn valor(evento: &SliderEvent) -> (f32, bool) {
    match evento {
        SliderEvent::Change(v) => (v.start(), false),
        SliderEvent::Release(v) => (v.start(), true),
    }
}

impl EditorDeFoto {
    /// Abre a janela e começa a carregar a base (`carregar_base`, no executor
    /// de fundo) e o projeto salvo, se houver.
    pub fn novo(
        foto: FotoDoEditor,
        edicoes: Arc<dyn Edicoes>,
        carregar_base: impl FnOnce() -> Result<DynamicImage, String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pincel = editor_core::Pincel::default();
        let tamanho = slider(1.0, 800.0, 1.0, pincel.raio, cx);
        let dureza = slider(0.0, 100.0, 1.0, pincel.dureza * 100.0, cx);
        let opacidade = slider(1.0, 100.0, 1.0, pincel.opacidade * 100.0, cx);
        let opacidade_da_camada = slider(0.0, 100.0, 1.0, 100.0, cx);

        let mut assinaturas = Vec::new();
        for (estado, qual) in [(&tamanho, 0u8), (&dureza, 1), (&opacidade, 2)] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                    let (v, _) = valor(evento);
                    if let Some(s) = ed.sessao_mut() {
                        match qual {
                            0 => s.pincel.raio = v.max(1.0),
                            1 => s.pincel.dureza = v / 100.0,
                            _ => s.pincel.opacidade = v / 100.0,
                        }
                    }
                    cx.notify();
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &opacidade_da_camada,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = valor(evento);
                ed.mover_opacidade_da_camada(v / 100.0, soltou, cx);
            },
        ));

        let foco = cx.focus_handle();
        window.focus(&foco, cx);

        // 🚨 Fechar pelo X da janela com alterações pergunta antes, como o
        // botão "Fechar".
        let fraca = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            fraca
                .update(cx, |ed, cx| ed.pode_fechar(window, cx))
                .unwrap_or(true)
        });

        let mut editor = Self {
            foto,
            edicoes,
            fase: Fase::Carregando,
            ladrilhos: HashMap::new(),
            palco: Bounds::default(),
            ponteiro: None,
            pintando: false,
            salvando: false,
            aviso: None,
            perguntando: false,
            liberada: false,
            foco,
            tamanho,
            dureza,
            opacidade,
            opacidade_da_camada,
            medidas: Medidas::default(),
            _assinaturas: assinaturas,
            _tarefa: None,
        };
        editor.carregar(carregar_base, cx);
        editor
    }

    fn carregar(
        &mut self,
        carregar_base: impl FnOnce() -> Result<DynamicImage, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let edicoes = self.edicoes.clone();
        let foto = self.foto.clone();
        let trabalho = cx.background_executor().spawn(async move {
            // C28: a base é a base neutra do bruto — nunca uma prévia.
            let base = Arc::new(carregar_base()?.to_rgb8());
            let (documento, historico) = match edicoes.abrir(&foto, &base)? {
                Abertura::Nova => (
                    Documento::novo(BaseRef::da_imagem(&base)),
                    Historico::novo(),
                ),
                Abertura::Existente {
                    documento,
                    historico,
                } => (documento, historico),
            };
            Ok::<_, String>(Sessao::nova(base, documento, historico, LADO_DA_VISTA))
        });
        self._tarefa = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.fase = match resultado {
                    Ok(sessao) => Fase::Pronta(Box::new(sessao)),
                    Err(erro) => Fase::Falhou(erro),
                };
                ed.sincronizar_os_sliders_da_camada(cx);
                cx.notify();
            });
        }));
    }

    // ------------------------------------------------------------ consultas

    pub fn foto(&self) -> &FotoDoEditor {
        &self.foto
    }

    pub fn sessao(&self) -> Option<&Sessao> {
        match &self.fase {
            Fase::Pronta(s) => Some(s),
            _ => None,
        }
    }

    fn sessao_mut(&mut self) -> Option<&mut Sessao> {
        match &mut self.fase {
            Fase::Pronta(s) => Some(s),
            _ => None,
        }
    }

    /// Onde a foto está desenhada, em pontos da janela — o roteiro mira nela.
    pub fn area_na_janela(&self) -> Option<Bounds<Pixels>> {
        area_da_foto(self.palco, self.sessao()?).map(|(area, _)| area)
    }

    pub fn pronta(&self) -> bool {
        self.sessao().is_some()
    }

    pub fn falha(&self) -> Option<&str> {
        match &self.fase {
            Fase::Falhou(e) => Some(e),
            _ => None,
        }
    }

    /// Há alterações que não foram salvas.
    pub fn alterado(&self) -> bool {
        self.sessao().is_some_and(Sessao::alterado)
    }

    pub fn salvando(&self) -> bool {
        self.salvando
    }

    pub fn aviso(&self) -> Option<(&str, bool)> {
        self.aviso.as_ref().map(|(t, e)| (t.as_ref(), *e))
    }

    pub fn perguntando(&self) -> bool {
        self.perguntando
    }

    pub fn medidas(&self) -> Medidas {
        self.medidas
    }

    /// O título da janela: o arquivo e o ponto das alterações.
    pub fn titulo(&self) -> String {
        format!(
            "Editar — {}{}",
            self.foto.nome,
            if self.alterado() { " •" } else { "" }
        )
    }

    // ------------------------------------------------------------ o pincel

    /// Converte um ponto da janela em pixel da foto, se ele cai na foto.
    fn na_foto(&self, ponto: Point<Pixels>) -> Option<(f32, f32)> {
        let sessao = self.sessao()?;
        let (area, escala) = area_da_foto(self.palco, sessao)?;
        let x = f32::from(ponto.x - area.origin.x) / escala;
        let y = f32::from(ponto.y - area.origin.y) / escala;
        let fator = sessao.vista().fator() as f32;
        Some((x * fator, y * fator))
    }

    /// O ponteiro desceu na foto.
    pub fn apertar(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((x, y)) = self.na_foto(ponto) else {
            return;
        };
        let inicio = Instant::now();
        if let Some(s) = self.sessao_mut() {
            s.apertar(x, y);
            self.pintando = true;
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    /// O ponteiro andou — pinta se estiver apertado; sempre move o círculo.
    pub fn mover(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        self.ponteiro = Some(ponto);
        if self.pintando {
            // Fora da foto o traço continua (a conta dá coordenada negativa ou
            // além da borda, e o pincel corta) — como no Photoshop.
            if let (Some(sessao), Some((area, escala))) = (
                self.sessao(),
                self.sessao().and_then(|s| area_da_foto(self.palco, s)),
            ) {
                let fator = sessao.vista().fator() as f32;
                let x = f32::from(ponto.x - area.origin.x) / escala * fator;
                let y = f32::from(ponto.y - area.origin.y) / escala * fator;
                let inicio = Instant::now();
                if let Some(s) = self.sessao_mut() {
                    s.arrastar(x, y);
                }
                self.medidas.ultimo_gesto = Some(inicio.elapsed());
            }
        }
        cx.notify();
    }

    /// O ponteiro subiu: o traço vira um passo do desfazer.
    pub fn soltar(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pintando) {
            return;
        }
        if let Some(s) = self.sessao_mut() {
            s.soltar();
        }
        self.aviso = None;
        cx.notify();
    }

    pub fn ferramenta(&self) -> Option<Ferramenta> {
        self.sessao().map(|s| s.pincel.ferramenta)
    }

    pub fn usar(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.pincel.ferramenta = ferramenta;
        }
        cx.notify();
    }

    pub fn escolher_cor(&mut self, cor: [u8; 3], cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.pincel.cor = cor;
            s.pincel.ferramenta = Ferramenta::Pincel;
        }
        cx.notify();
    }

    fn mudar_tamanho(&mut self, fator: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let novo = (s.pincel.raio * fator).clamp(1.0, 800.0).round();
        s.pincel.raio = novo;
        self.tamanho
            .update(cx, |estado, cx| estado.set_value(novo, window, cx));
        cx.notify();
    }

    // ----------------------------------------------------------- a camada

    pub fn alternar_visibilidade(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.alternar_visibilidade();
        }
        cx.notify();
    }

    pub fn mover_opacidade_da_camada(&mut self, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.mover_opacidade(valor);
            if soltou {
                s.confirmar_opacidade();
            }
        }
        cx.notify();
    }

    fn sincronizar_os_sliders_da_camada(&mut self, cx: &mut Context<Self>) {
        // Sem janela aqui: o valor entra no próximo quadro (ver `render`).
        let _ = cx;
    }

    pub fn camada_visivel(&self) -> bool {
        self.sessao()
            .is_some_and(|s| s.documento().camadas[0].visivel)
    }

    pub fn opacidade_da_camada(&self) -> f32 {
        self.sessao()
            .map_or(1.0, |s| s.documento().camadas[0].opacidade)
    }

    // ---------------------------------------------------------- desfazer

    pub fn desfazer(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.desfazer();
        }
        cx.notify();
    }

    pub fn refazer(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.refazer();
        }
        cx.notify();
    }

    // ------------------------------------------------------------ salvar

    /// Salva em segundo plano. `e_fechar`: fecha a janela quando der certo (o
    /// "Salvar" da pergunta de fechar).
    pub fn salvar(&mut self, e_fechar: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.salvando {
            return;
        }
        let Some(sessao) = self.sessao_mut() else {
            return;
        };
        let (documento, historico) = sessao.instantaneo();
        let base = sessao.base().clone();
        let edicoes = self.edicoes.clone();
        let foto = self.foto.clone();
        self.salvando = true;
        self.aviso = Some(("Salvando…".into(), false));
        cx.notify();

        let inicio = Instant::now();
        let para_gravar = (foto.clone(), documento, historico.clone());
        let trabalho = cx.background_executor().spawn(async move {
            let (foto, documento, historico) = para_gravar;
            edicoes.salvar(&foto, &base, &documento, &historico)
        });
        let janela = window.window_handle();
        self._tarefa = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.salvando = false;
                ed.medidas.ultimo_salvamento = Some(inicio.elapsed());
                match resultado {
                    Ok(versao) => {
                        if let Some(s) = ed.sessao_mut() {
                            s.salvo(&historico);
                        }
                        ed.aviso = Some((
                            match &versao {
                                Some(v) => format!("Salvo — revisão {}", v.revisao),
                                None => "Salvo — sem efeito, a foto fica como o bruto".into(),
                            }
                            .into(),
                            false,
                        ));
                        cx.emit(EventoDoEditor::Salva { foto, versao });
                        if e_fechar {
                            ed.liberada = true;
                            let _ = janela.update(cx, |_, window, _| window.remove_window());
                        }
                    }
                    Err(erro) => {
                        ed.aviso = Some((format!("Não foi possível salvar: {erro}").into(), true));
                    }
                }
                cx.notify();
            });
        }));
    }

    // ------------------------------------------------------------ fechar

    /// O X da janela, o `Cmd+W` e o botão "Fechar": com alterações, pergunta.
    pub fn pode_fechar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.liberada || !self.alterado() {
            return true;
        }
        self.perguntando = true;
        cx.notify();
        false
    }

    pub fn fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_fechar(window, cx) {
            window.remove_window();
        }
    }

    /// "Descartar" na pergunta: fecha sem salvar — o projeto salvo continua
    /// como estava.
    pub fn descartar_e_fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.liberada = true;
        self.perguntando = false;
        cx.notify();
        window.remove_window();
    }

    pub fn cancelar_fechar(&mut self, cx: &mut Context<Self>) {
        self.perguntando = false;
        cx.notify();
    }

    /// Um gesto do roteiro de depuração nesta janela — o do app inteiro
    /// (`editor …` depois de `tira editar N`) e o do editor avulso (`bin/editor.rs`).
    ///
    /// - `mouse apertar|arrastar|soltar|clicar fx fy` — evento **real** do
    ///   AppKit, numa fração da foto;
    /// - `tecla <keyCode> [shift|ctrl|alt|cmd…]` — tecla física;
    /// - `foto <nome>` — a janela em PNG, em `pasta`;
    /// - `estado` — uma linha no stderr.
    pub fn seguir_o_roteiro(
        &mut self,
        gesto: &str,
        pasta: Option<&std::path::Path>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let partes: Vec<&str> = gesto.split_whitespace().collect();
        let numero = |i: usize| {
            partes
                .get(i)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0)
        };
        match partes.first().copied().unwrap_or_default() {
            "mouse" => {
                let tipo = partes.get(1).copied().unwrap_or("clicar");
                let Some(area) = self.area_na_janela() else {
                    eprintln!("[roteiro] editor: a foto ainda não está desenhada");
                    return;
                };
                let x = f32::from(area.origin.x) + f32::from(area.size.width) * numero(2);
                let y = f32::from(area.origin.y) + f32::from(area.size.height) * numero(3);
                let r = crate::depuracao::mouse_nativo(window, tipo, x, y, 0);
                eprintln!("[roteiro] editor mouse {tipo} ({x:.0}, {y:.0}): {r:?}");
            }
            "tecla" => {
                let codigo = numero(1) as u16;
                let mods = partes
                    .iter()
                    .skip(2)
                    .map(|n| match *n {
                        "shift" => 1 << 17,
                        "ctrl" => 1 << 18,
                        "alt" => 1 << 19,
                        "cmd" => 1 << 20,
                        _ => 0,
                    })
                    .fold(0, |a, b| a | b);
                let r = crate::depuracao::tecla_nativa(window, codigo, mods);
                eprintln!("[roteiro] editor tecla {codigo} mods={mods:#x}: {r:?}");
            }
            "foto" => {
                let Some(pasta) = pasta else {
                    eprintln!("[roteiro] editor foto: sem VLB_FOTOS");
                    return;
                };
                let destino = pasta.join(format!("{}.png", partes.get(1).unwrap_or(&"editor")));
                let r = crate::depuracao::fotografar(window, &destino);
                eprintln!("[foto] {}: {r:?}", destino.display());
            }
            "estado" => eprintln!(
                "[roteiro] editor: foto={} pronta={} falha={:?} alterado={} salvando={} aviso={:?} passos={} medidas={:?}",
                self.foto.id,
                self.pronta(),
                self.falha(),
                self.alterado(),
                self.salvando,
                self.aviso(),
                self.sessao().map_or(0, |s| s.historico().posicao()),
                self.medidas,
            ),
            outro => eprintln!("[roteiro] gesto do editor desconhecido: {outro}"),
        }
    }

    /// 🧪 Um traço de `de` a `ate`, em pixels da foto — o que o ponteiro faz,
    /// sem precisar do palco medido.
    #[cfg(test)]
    pub fn tracar_para_teste(&mut self, de: (f32, f32), ate: (f32, f32), cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.pincel.dureza = 1.0;
            s.pincel.raio = 4.0;
            s.apertar(de.0, de.1);
            s.arrastar(ate.0, ate.1);
            s.soltar();
        }
        cx.notify();
    }

    // ------------------------------------------------------------ desenho

    /// Sobe para a GPU os ladrilhos que o último gesto sujou.
    fn subir_os_ladrilhos(&mut self) {
        let Fase::Pronta(sessao) = &mut self.fase else {
            return;
        };
        let sujos = sessao.vista_mut().levar_os_sujos();
        self.medidas.ladrilhos_no_quadro = sujos.len();
        for ladrilho in sujos {
            let (l, a, bytes) = sessao.vista().ladrilho_bgra(ladrilho);
            if let Some(imagem) = crate::imagem::de_bgra(l, a, bytes) {
                self.ladrilhos.insert(ladrilho, imagem);
            }
        }
    }

    fn palco(&self, window: &Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let fator_da_tela = window.scale_factor().max(1.0);
        let medidor = cx.entity();
        let ouvinte = cx.entity();
        let pintando = self.pintando;
        let medida = canvas(
            move |bounds, _window, cx| {
                medidor.update(cx, |ed, _cx| {
                    if ed.palco != bounds {
                        ed.palco = bounds;
                    }
                });
            },
            move |_bounds, _prepaint, window, _cx| {
                // 🔑 O arrasto é ouvido na janela: o traço continua quando o
                // ponteiro sai da foto, e o soltar fora dela ainda fecha o traço.
                if !pintando {
                    return;
                }
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &MouseMoveEvent, fase, _window, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.mover(evento.position, cx));
                        }
                    }
                });
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, _window, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.soltar(cx));
                        }
                    }
                });
            },
        )
        .absolute()
        .size_full();

        let mut palco = div()
            .id("palco-do-editor")
            .debug_selector(|| "palco-do-editor".into())
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(tema::cores::poco())
            .child(medida);

        match &self.fase {
            Fase::Carregando => {
                palco = palco.child(aviso_centrado("Abrindo a foto em resolução cheia…", cx));
            }
            Fase::Falhou(erro) => {
                palco = palco.child(aviso_centrado(&format!("A foto não abriu: {erro}"), cx));
            }
            Fase::Pronta(sessao) => {
                if let Some((area, escala)) = area_da_foto(self.palco, sessao) {
                    let origem = area.origin - self.palco.origin;
                    let vista = sessao.vista();
                    for ly in 0..vista.linhas() {
                        for lx in 0..vista.colunas() {
                            let Some(imagem) = self.ladrilhos.get(&(lx, ly)) else {
                                continue;
                            };
                            let r = vista.retangulo_do_ladrilho((lx, ly));
                            // 🚨 **Alinhado aos pixels da tela, e sobrando um.**
                            // Com a posição fracionária, a borda de dois
                            // ladrilhos vizinhos caía no meio de um pixel e
                            // abria uma fresta escura (visto no app real,
                            // 27/set/2026). Início para baixo, fim para cima e
                            // um pixel do dispositivo a mais: o vizinho cobre.
                            let alinhar = |v: f32, cima: bool| {
                                let d = v * fator_da_tela;
                                (if cima { d.ceil() } else { d.floor() }) / fator_da_tela
                            };
                            let x0 = f32::from(origem.x) + r.x as f32 * escala;
                            let y0 = f32::from(origem.y) + r.y as f32 * escala;
                            let (esq, topo) = (alinhar(x0, false), alinhar(y0, false));
                            let dir =
                                alinhar(x0 + r.largura as f32 * escala, true) + 1.0 / fator_da_tela;
                            let baixo =
                                alinhar(y0 + r.altura as f32 * escala, true) + 1.0 / fator_da_tela;
                            palco = palco.child(
                                img(imagem.clone())
                                    .object_fit(ObjectFit::Fill)
                                    .absolute()
                                    .left(px(esq))
                                    .top(px(topo))
                                    .w(px(dir - esq))
                                    .h(px(baixo - topo)),
                            );
                        }
                    }
                    // O círculo do pincel, do tamanho que ele pinta.
                    if let Some(ponteiro) = self.ponteiro.filter(|p| area.contains(p)) {
                        let raio = sessao.pincel.raio / vista.fator() as f32 * escala;
                        let centro = ponteiro - self.palco.origin;
                        palco = palco.child(
                            div()
                                .absolute()
                                .left(centro.x - px(raio))
                                .top(centro.y - px(raio))
                                .size(px(raio * 2.0))
                                .rounded_full()
                                .border_1()
                                .border_color(gpui_kit::white().opacity(0.85)),
                        );
                    }
                }
                palco = palco
                    .cursor(gpui_kit::CursorStyle::Crosshair)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|ed, evento: &MouseDownEvent, window, cx| {
                            window.focus(&ed.foco, cx);
                            ed.apertar(evento.position, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|ed, evento: &MouseMoveEvent, _w, cx| {
                        if !ed.pintando {
                            ed.mover(evento.position, cx);
                        }
                    }));
            }
        }
        palco.into_any_element()
    }

    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let pronta = self.pronta();
        let (pode_desfazer, pode_refazer) = self
            .sessao()
            .map(|s| (s.historico().pode_desfazer(), s.historico().pode_refazer()))
            .unwrap_or((false, false));
        let dica = |passo: Option<&editor_core::Comando>, verbo: &str| -> SharedString {
            match (self.sessao(), passo) {
                (Some(s), Some(p)) => format!("{verbo} {}", p.descricao(s.documento())).into(),
                _ => verbo.to_string().into(),
            }
        };
        let dica_desfazer = dica(
            self.sessao().and_then(|s| s.historico().a_desfazer()),
            "Desfazer",
        );
        let dica_refazer = dica(
            self.sessao().and_then(|s| s.historico().a_refazer()),
            "Refazer",
        );
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(48.))
            .px(px(12.))
            .border_b_1()
            .border_color(tema.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(format!("Editar — {}", self.foto.nome)),
            )
            .when(self.alterado(), |barra| {
                barra.child(
                    div()
                        .id("editor-alterado")
                        .debug_selector(|| "editor-alterado".into())
                        .text_xs()
                        .text_color(tema::cores::quente())
                        .child("• Alterações não salvas"),
                )
            })
            .when_some(self.aviso.clone(), |barra, (texto, erro)| {
                barra.child(
                    div()
                        .text_xs()
                        .text_color(if erro {
                            tema.danger
                        } else {
                            tema.muted_foreground
                        })
                        .child(texto),
                )
            })
            .child(div().flex_1())
            .child(
                crate::estilo::botao_fantasma("editor-desfazer", cx)
                    .debug_selector(|| "editor-desfazer".into())
                    .child("↶")
                    .tooltip(dica_desfazer)
                    .disabled(!pode_desfazer)
                    .on_click(cx.listener(|ed, _, _, cx| ed.desfazer(cx))),
            )
            .child(
                crate::estilo::botao_fantasma("editor-refazer", cx)
                    .debug_selector(|| "editor-refazer".into())
                    .child("↷")
                    .tooltip(dica_refazer)
                    .disabled(!pode_refazer)
                    .on_click(cx.listener(|ed, _, _, cx| ed.refazer(cx))),
            )
            .child(
                crate::estilo::botao_primario("editor-salvar", cx)
                    .debug_selector(|| "editor-salvar".into())
                    .child(if self.salvando {
                        "Salvando…"
                    } else {
                        "Salvar"
                    })
                    .disabled(!pronta || self.salvando)
                    .on_click(cx.listener(|ed, _, window, cx| ed.salvar(false, window, cx))),
            )
            .child(
                crate::estilo::botao_contorno("editor-fechar", cx)
                    .debug_selector(|| "editor-fechar".into())
                    .child("Fechar")
                    .on_click(cx.listener(|ed, _, window, cx| ed.fechar(window, cx))),
            )
    }

    fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let ferramenta = self.ferramenta();
        let cor_atual = self.sessao().map(|s| s.pincel.cor);
        let rotulo = |texto: &'static str| {
            div()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(texto)
        };
        let botao_ferramenta =
            |id: &'static str, nome: &'static str, qual: Ferramenta, cx: &mut Context<Self>| {
                let ativa = ferramenta == Some(qual);
                let botao = if ativa {
                    crate::estilo::botao_primario(id, cx)
                } else {
                    crate::estilo::botao_contorno(id, cx)
                };
                botao
                    .debug_selector(move || id.into())
                    .child(nome)
                    .on_click(cx.listener(move |ed, _, _, cx| ed.usar(qual, cx)))
            };
        let visivel = self.camada_visivel();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .w(px(240.))
            .h_full()
            .p(px(12.))
            .border_l_1()
            .border_color(tema.border)
            .child(rotulo("Ferramenta"))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(botao_ferramenta(
                        "editor-pincel",
                        "Pincel (B)",
                        Ferramenta::Pincel,
                        cx,
                    ))
                    .child(botao_ferramenta(
                        "editor-borracha",
                        "Borracha (E)",
                        Ferramenta::Borracha,
                        cx,
                    )),
            )
            .child(rotulo("Tamanho  [  ]"))
            .child(div().h(px(20.)).child(crate::estilo::slider(&self.tamanho)))
            .child(rotulo("Dureza"))
            .child(div().h(px(20.)).child(crate::estilo::slider(&self.dureza)))
            .child(rotulo("Opacidade do pincel"))
            .child(
                div()
                    .h(px(20.))
                    .child(crate::estilo::slider(&self.opacidade)),
            )
            .child(rotulo("Cor"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(AMOSTRAS.iter().enumerate().map(|(i, cor)| {
                        let cor = *cor;
                        let escolhida = cor_atual == Some(cor);
                        div()
                            .id(("editor-cor", i))
                            .size(px(22.))
                            .rounded(crate::tema::canto(4.))
                            .border_2()
                            .border_color(if escolhida { tema.ring } else { tema.border })
                            .bg(gpui_kit::rgb(
                                (cor[0] as u32) << 16 | (cor[1] as u32) << 8 | cor[2] as u32,
                            ))
                            .cursor_pointer()
                            .on_click(cx.listener(move |ed, _, _, cx| ed.escolher_cor(cor, cx)))
                    })),
            )
            .child(div().h(px(1.)).bg(tema.border))
            .child(rotulo("Camada"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        crate::estilo::botao_fantasma("editor-olho", cx)
                            .debug_selector(|| "editor-olho".into())
                            .child(if visivel { "👁" } else { "—" })
                            .tooltip(if visivel {
                                "Esconder a camada (H)"
                            } else {
                                "Mostrar a camada (H)"
                            })
                            .on_click(cx.listener(|ed, _, _, cx| ed.alternar_visibilidade(cx))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(editor_core::documento::NOME_DA_PRIMEIRA),
                    ),
            )
            .child(rotulo("Opacidade da camada"))
            .child(
                div()
                    .h(px(20.))
                    .child(crate::estilo::slider(&self.opacidade_da_camada)),
            )
    }

    fn pergunta_de_fechar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        Some(
            gpui_kit::component::v_flex()
                .gap(px(16.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(crate::estilo::cabecalho_do_dialogo(
                    "Salvar a edição antes de fechar?",
                    SharedString::from(format!(
                        "{} tem alterações que ainda não foram salvas. Descartar fecha a janela e mantém a última versão salva.",
                        self.foto.nome
                    )),
                    None,
                    cx,
                ))
                .child(
                    crate::estilo::rodape_do_dialogo()
                        .child(
                            crate::estilo::botao_contorno("editor-cancelar-fechar", cx)
                                .debug_selector(|| "editor-cancelar-fechar".into())
                                .child("Cancelar")
                                .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_fechar(cx))),
                        )
                        .child(
                            crate::estilo::botao_perigo("editor-descartar", cx)
                                .debug_selector(|| "editor-descartar".into())
                                .child("Descartar")
                                .on_click(cx.listener(|ed, _, window, cx| ed.descartar_e_fechar(window, cx))),
                        )
                        .child(
                            crate::estilo::botao_primario("editor-salvar-e-fechar", cx)
                                .debug_selector(|| "editor-salvar-e-fechar".into())
                                .child("Salvar")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.perguntando = false;
                                    ed.salvar(true, window, cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// O retângulo da janela onde a foto inteira cabe, e a escala da vista para
/// ele (pixels da janela por pixel da vista).
fn area_da_foto(palco: Bounds<Pixels>, sessao: &Sessao) -> Option<(Bounds<Pixels>, f32)> {
    let imagem = sessao.vista().imagem();
    let (vl, va) = (imagem.width() as f32, imagem.height() as f32);
    let (pl, pa) = (f32::from(palco.size.width), f32::from(palco.size.height));
    if vl <= 0.0 || va <= 0.0 || pl <= 1.0 || pa <= 1.0 {
        return None;
    }
    let margem = 16.0;
    let escala = ((pl - 2.0 * margem) / vl)
        .min((pa - 2.0 * margem) / va)
        .max(0.01);
    let (l, a) = (vl * escala, va * escala);
    let origem = palco.origin + gpui_kit::point(px((pl - l) / 2.0), px((pa - a) / 2.0));
    Some((Bounds::new(origem, gpui_kit::size(px(l), px(a))), escala))
}

fn aviso_centrado(texto: &str, cx: &mut Context<EditorDeFoto>) -> impl IntoElement {
    div()
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(texto.to_string())
}

impl Focusable for EditorDeFoto {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.foco.clone()
    }
}

impl Render for EditorDeFoto {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.subir_os_ladrilhos();
        window.set_window_title(&self.titulo());
        // O slider da camada acompanha o desfazer e a reabertura.
        let opacidade = self.opacidade_da_camada() * 100.0;
        if (self.opacidade_da_camada.read(cx).value().start() - opacidade).abs() > 0.5 {
            self.opacidade_da_camada
                .update(cx, |s, cx| s.set_value(opacidade, window, cx));
        }
        let pergunta = {
            let quer = self.perguntando;
            crate::dialogo::desenhar(
                self,
                quer,
                crate::dialogo::Jeito::alerta(480.),
                Self::pergunta_de_fechar,
                |ed, _, cx| ed.cancelar_fechar(cx),
                window,
                cx,
            )
        };
        let tema = cx.theme().clone();
        div()
            .id("editor-de-foto")
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .size_full()
            .flex()
            .flex_col()
            .bg(tema.background)
            .text_color(tema.foreground)
            .on_action(cx.listener(|ed, _: &DesfazerNoEditor, _, cx| ed.desfazer(cx)))
            .on_action(cx.listener(|ed, _: &RefazerNoEditor, _, cx| ed.refazer(cx)))
            .on_action(
                cx.listener(|ed, _: &SalvarNoEditor, window, cx| ed.salvar(false, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &FecharEditor, window, cx| ed.fechar(window, cx)))
            .on_action(cx.listener(|ed, _: &UsarPincel, _, cx| ed.usar(Ferramenta::Pincel, cx)))
            .on_action(cx.listener(|ed, _: &UsarBorracha, _, cx| ed.usar(Ferramenta::Borracha, cx)))
            .on_action(cx.listener(|ed, _: &PincelMenor, window, cx| {
                ed.mudar_tamanho(1.0 / 1.25, window, cx)
            }))
            .on_action(
                cx.listener(|ed, _: &PincelMaior, window, cx| ed.mudar_tamanho(1.25, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &AlternarCamada, _, cx| ed.alternar_visibilidade(cx)))
            .child(self.barra(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.palco(window, cx))
                    .child(self.painel(cx)),
            )
            .children(pergunta)
    }
}
