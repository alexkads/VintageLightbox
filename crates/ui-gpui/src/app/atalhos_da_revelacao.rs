//! Os atalhos da revelação que o site tem e o app ainda não tinha — o
//! `decidirAtalho` de `revelacao/atalhos.ts`, tecla por tecla.
//!
//! 🔑 **Ligados na raiz**, como os outros: quem tem o foco é o [`Aplicativo`],
//! e ação só alcança quem está no caminho dele. Cada um só age na Revelação.
//!
//! | Tecla | Faz |
//! |---|---|
//! | `Enter` | no Enquadrar, confirma e sai |
//! | `[` · `]` | gira 90° à esquerda · à direita |
//! | `⇧H` · `⇧V` | espelha na horizontal · na vertical |
//! | `?` | mostra e esconde a folha de atalhos |
//! | `Z` | alterna o zoom; segurado, espia e volta ao soltar |
//! | Espaço | tocado alterna o zoom; segurado, é a mão |
//! | `\` | segurado, mostra a foto sem ajuste |
//! | `⌘=` · `⌘−` · `⌘0` · `⌘⌥0` | aproxima · afasta · encaixa · 1:1 |
//!
//! 🔠 **Fora da Revelação, `⌘=` · `⌘−` · `⌘0` são a letra do app** — o gesto
//! do Zed (`tema::letra`). Na Revelação a tecla continua sendo da foto, como
//! no Lightroom.
//!
//! | Tecla | Faz |
//! |---|---|
//! | Home · End · PgDn · PgUp | percorrem a foto ampliada |
//! | `⇧C` | o Comparar: duas fotos lado a lado, aqui e na tela do cliente |
//! | `Tab` · `⇧Tab` | esconde e mostra as colunas · as colunas e a tira (também na galeria da sessão) |
//!
//! ⚠️ **Sem o F6/F7/F8 do Lightroom** para cada coluna: no app eles são do
//! caixa flutuante (sangria, vendas), que vale em qualquer tela. As setas das
//! bordas fazem cada coluna uma a uma.
//!
//! 🔑 **No Comparar, as outras ficam mudas** (`na_revelacao`): elas editam a
//! foto aberta, que ali divide o palco com outra. Só o `?` e o próprio `⇧C`
//! passam — é a lista do `decidirAtalho` do site com `comparando`.

use gpui_kit::{actions, prelude::*, Context, Div, KeyBinding, KeyUpEvent, Window};

use super::{Aplicativo, Tela, CONTEXTO, SEM_CAMPO_DE_TEXTO};
use crate::revelacao::tela::{Ferramenta, Revelacao};
use crate::revelacao::zoom::Nivel;

actions!(
    vintagelightbox,
    [
        ConfirmarEnquadramento,
        GirarAEsquerda,
        GirarADireita,
        EspelharNaHorizontal,
        EspelharNaVertical,
        AlternarAjuda,
        AlternarZoom,
        SegurarAMao,
        VerOAntes,
        Aproximar,
        Afastar,
        ZoomEncaixar,
        Zoom1Para1,
        ZoomAoInicio,
        ZoomAoFim,
        ZoomTelaSeguinte,
        ZoomTelaAnterior,
        Comparar,
        FerramentaPincel,
        FerramentaLinear,
        FerramentaRadial,
        FerramentaLaco,
        FerramentaCarimbo,
        FerramentaBandAid,
        FerramentaPreencher,
        AlternarMarcacoes,
        MenosSuavizacao,
        MaisSuavizacao,
        AlternarColunas,
        AlternarPaineis
    ]
);

pub(super) fn ligar(cx: &mut gpui_kit::App) {
    let solta = Some(SEM_CAMPO_DE_TEXTO);
    let com_modificador = Some(CONTEXTO);
    cx.bind_keys([
        KeyBinding::new("enter", ConfirmarEnquadramento, solta),
        KeyBinding::new("[", GirarAEsquerda, solta),
        KeyBinding::new("]", GirarADireita, solta),
        KeyBinding::new("shift-h", EspelharNaHorizontal, solta),
        KeyBinding::new("shift-v", EspelharNaVertical, solta),
        KeyBinding::new("shift-/", AlternarAjuda, solta),
        KeyBinding::new("?", AlternarAjuda, solta),
        KeyBinding::new("z", AlternarZoom, solta),
        KeyBinding::new("space", SegurarAMao, solta),
        // O `\` deixa de alternar: segurar é ver, soltar é voltar.
        KeyBinding::new("\\", VerOAntes, solta),
        KeyBinding::new("cmd-=", Aproximar, com_modificador),
        KeyBinding::new("cmd-+", Aproximar, com_modificador),
        KeyBinding::new("cmd-shift-=", Aproximar, com_modificador),
        KeyBinding::new("cmd--", Afastar, com_modificador),
        KeyBinding::new("cmd-0", ZoomEncaixar, com_modificador),
        KeyBinding::new("cmd-alt-0", Zoom1Para1, com_modificador),
        KeyBinding::new("ctrl-=", Aproximar, com_modificador),
        KeyBinding::new("ctrl-+", Aproximar, com_modificador),
        KeyBinding::new("ctrl-shift-=", Aproximar, com_modificador),
        KeyBinding::new("ctrl--", Afastar, com_modificador),
        KeyBinding::new("ctrl-0", ZoomEncaixar, com_modificador),
        KeyBinding::new("ctrl-alt-0", Zoom1Para1, com_modificador),
        KeyBinding::new("home", ZoomAoInicio, solta),
        KeyBinding::new("end", ZoomAoFim, solta),
        KeyBinding::new("pagedown", ZoomTelaSeguinte, solta),
        KeyBinding::new("pageup", ZoomTelaAnterior, solta),
        // 🔑 **`⇧C`, e não o `C` do Lightroom** (dono, 2026-09-26): o `C` solto
        // já é a Cortesia no caixa. O `cmd-shift-c` (Copiar revelação) é outra
        // tecla e não disputa com este.
        KeyBinding::new("shift-c", Comparar, solta),
        // A Revelação local — as teclas do Lightroom e do Photoshop.
        KeyBinding::new("k", FerramentaPincel, solta),
        KeyBinding::new("m", FerramentaLinear, solta),
        KeyBinding::new("shift-m", FerramentaRadial, solta),
        KeyBinding::new("l", FerramentaLaco, solta),
        KeyBinding::new("s", FerramentaCarimbo, solta),
        KeyBinding::new("j", FerramentaBandAid, solta),
        KeyBinding::new("shift-j", FerramentaPreencher, solta),
        KeyBinding::new("h", AlternarMarcacoes, solta),
        // `{` e `}`: a suavização. Pelo caractere e pela tecla com Shift, para
        // valer no teclado americano e no ABNT2.
        KeyBinding::new("{", MenosSuavizacao, solta),
        KeyBinding::new("}", MaisSuavizacao, solta),
        KeyBinding::new("shift-[", MenosSuavizacao, solta),
        KeyBinding::new("shift-]", MaisSuavizacao, solta),
        // Os painéis, com as teclas do Lightroom.
        KeyBinding::new("tab", AlternarColunas, solta),
        KeyBinding::new("shift-tab", AlternarPaineis, solta),
    ]);
}

/// 🔠 A letra do app um passo maior (`1`), menor (`-1`) ou de volta à do
/// template (`0`), com o toast do kit dizendo o tamanho.
fn mudar_letra(delta: i32, window: &mut Window, cx: &mut Context<Aplicativo>) {
    let tamanho = crate::tema::letra::passo(delta, cx);
    // O `info` do kit, neutro: não deu certo nem errado, só mudou.
    let nota =
        gpui_kit::component::notification::Notification::info(format!("Letra em {tamanho:.0} px"))
            .placement(gpui_kit::Anchor::TopCenter)
            .id::<LetraDoApp>();
    crate::estilo::mostrar_toast(nota, window, cx);
}

/// A chave do toast da letra: um toque novo troca o aviso, em vez de empilhar.
struct LetraDoApp;

impl Aplicativo {
    /// Faz na Revelação, e só nela. Fora dela a tecla segue adiante — Home e
    /// PgDn continuam rolando o que rolariam.
    fn na_revelacao(
        &mut self,
        cx: &mut Context<Self>,
        fazer: impl FnOnce(&mut Revelacao, &mut Context<Revelacao>),
    ) {
        if self.tela != Tela::Revelacao {
            cx.propagate();
            return;
        }
        // No Comparar, a tecla é engolida sem efeito — ver o topo do módulo.
        if self.revelacao.read(cx).comparando() {
            return;
        }
        self.revelacao.update(cx, fazer);
    }

    /// `Tab` e `⇧Tab`: os painéis da Revelação, ou os da galeria da sessão.
    fn alternar_paineis(&mut self, tudo: bool, window: &mut Window, cx: &mut Context<Self>) {
        match self.tela {
            Tela::Revelacao => self
                .revelacao
                .update(cx, |tela, cx| tela.alternar_paineis(tudo, window, cx)),
            Tela::Sessao => self
                .detalhe
                .update(cx, |tela, cx| tela.alternar_paineis(tudo, window, cx)),
            _ => cx.propagate(),
        }
    }

    /// Faz na Revelação mesmo no Comparar — o `?` e o próprio `⇧C`.
    fn na_revelacao_sempre(
        &mut self,
        cx: &mut Context<Self>,
        fazer: impl FnOnce(&mut Revelacao, &mut Context<Revelacao>),
    ) {
        if self.tela != Tela::Revelacao {
            cx.propagate();
            return;
        }
        self.revelacao.update(cx, fazer);
    }

    /// Home, End e as páginas só valem com a foto maior que a área.
    fn na_foto_ampliada(
        &mut self,
        cx: &mut Context<Self>,
        fazer: impl FnOnce(&mut Revelacao, &mut Context<Revelacao>),
    ) {
        let ampliada = self.tela == Tela::Revelacao
            && self.revelacao.read(cx).foto_ampliada()
            && !self.revelacao.read(cx).comparando();
        if !ampliada {
            cx.propagate();
            return;
        }
        self.revelacao.update(cx, fazer);
    }

    /// Soltou a tecla: o `Z` que espiava volta, o Espaço vira clique ou larga
    /// a mão, e o `\` devolve a foto revelada.
    fn tecla_da_revelacao_solta(&mut self, evento: &KeyUpEvent, cx: &mut Context<Self>) {
        if self.tela != Tela::Revelacao {
            return;
        }
        let tecla = evento.keystroke.key.as_str();
        self.revelacao.update(cx, |tela, cx| match tecla {
            "z" => tela.z_solto(cx),
            "space" => tela.espaco_solto(cx),
            "\\" => tela.ver_o_antes(false, cx),
            _ => {}
        });
    }

    /// Os ouvintes, pendurados na raiz.
    pub(super) fn ouvir_atalhos_da_revelacao(
        &self,
        raiz: Div,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        // 🚨 A tecla solta fora da janela nunca chega: sem foco, larga tudo.
        if !window.is_window_active() {
            let revelacao = self.revelacao.clone();
            cx.defer(move |cx| revelacao.update(cx, |tela, cx| tela.soltar_as_teclas(cx)));
        }
        raiz.on_key_up(cx.listener(|este, evento: &KeyUpEvent, _w, cx| {
            este.tecla_da_revelacao_solta(evento, cx)
        }))
        .on_action(cx.listener(|este, _: &ConfirmarEnquadramento, _w, cx| {
            let cortando = este.tela == Tela::Revelacao && este.revelacao.read(cx).cortando();
            if !cortando {
                cx.propagate();
                return;
            }
            este.revelacao.update(cx, |tela, cx| tela.aplicar_corte(cx));
        }))
        // `[` e `]`: com uma ferramenta da Revelação local na mão, o tamanho
        // (como no Lightroom); sem, o giro de 90° (como no site).
        .on_action(cx.listener(|este, _: &GirarAEsquerda, window, cx| {
            este.na_revelacao(cx, |tela, cx| {
                if tela.com_ferramenta_local() {
                    tela.mudar_tamanho_local(false, false, window, cx)
                } else {
                    tela.girar_a_esquerda(cx)
                }
            })
        }))
        .on_action(cx.listener(|este, _: &GirarADireita, window, cx| {
            este.na_revelacao(cx, |tela, cx| {
                if tela.com_ferramenta_local() {
                    tela.mudar_tamanho_local(true, false, window, cx)
                } else {
                    tela.girar(cx)
                }
            })
        }))
        .on_action(cx.listener(|este, _: &MenosSuavizacao, window, cx| {
            este.na_revelacao(cx, |tela, cx| {
                tela.mudar_tamanho_local(false, true, window, cx)
            })
        }))
        .on_action(cx.listener(|este, _: &MaisSuavizacao, window, cx| {
            este.na_revelacao(cx, |tela, cx| {
                tela.mudar_tamanho_local(true, true, window, cx)
            })
        }))
        .on_action(cx.listener(|este, _: &FerramentaPincel, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::Pincel, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaLinear, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::Linear, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaRadial, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::Radial, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaLaco, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::Laco, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaCarimbo, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::Carimbo, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaBandAid, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.usar_ferramenta(Ferramenta::BandAid, cx))
        }))
        .on_action(cx.listener(|este, _: &FerramentaPreencher, _w, cx| {
            este.na_revelacao(cx, |tela, cx| {
                tela.usar_ferramenta(Ferramenta::Preencher, cx)
            })
        }))
        .on_action(cx.listener(|este, _: &AlternarMarcacoes, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.alternar_marcacoes(cx))
        }))
        .on_action(cx.listener(|este, _: &EspelharNaHorizontal, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.espelhar_horizontal(cx))
        }))
        .on_action(cx.listener(|este, _: &EspelharNaVertical, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.espelhar_vertical(cx))
        }))
        .on_action(cx.listener(|este, _: &AlternarAjuda, _w, cx| {
            este.na_revelacao_sempre(cx, |tela, cx| tela.alternar_ajuda(cx))
        }))
        .on_action(cx.listener(|este, _: &Comparar, window, cx| {
            este.na_revelacao_sempre(cx, |tela, cx| tela.alternar_comparacao(window, cx))
        }))
        .on_action(cx.listener(|este, _: &AlternarZoom, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.z_apertado(cx))
        }))
        .on_action(cx.listener(|este, _: &SegurarAMao, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.espaco_apertado(cx))
        }))
        .on_action(cx.listener(|este, _: &VerOAntes, _w, cx| {
            este.na_revelacao(cx, |tela, cx| tela.ver_o_antes(true, cx))
        }))
        .on_action(cx.listener(|este, _: &Aproximar, window, cx| {
            if este.tela != Tela::Revelacao {
                return mudar_letra(1, window, cx);
            }
            este.na_revelacao(cx, |tela, cx| tela.passo_de_zoom(1, cx))
        }))
        .on_action(cx.listener(|este, _: &Afastar, window, cx| {
            if este.tela != Tela::Revelacao {
                return mudar_letra(-1, window, cx);
            }
            este.na_revelacao(cx, |tela, cx| tela.passo_de_zoom(-1, cx))
        }))
        .on_action(cx.listener(|este, _: &ZoomEncaixar, window, cx| {
            if este.tela != Tela::Revelacao {
                return mudar_letra(0, window, cx);
            }
            este.na_revelacao(cx, |tela, cx| {
                if !tela.cortando() {
                    tela.ir_para_nivel(Nivel::Encaixar, None, cx)
                }
            })
        }))
        .on_action(cx.listener(|este, _: &Zoom1Para1, _w, cx| {
            este.na_revelacao(cx, |tela, cx| {
                if !tela.cortando() {
                    tela.ir_para_nivel(Nivel::Razao(1.), None, cx)
                }
            })
        }))
        .on_action(cx.listener(|este, _: &AlternarColunas, window, cx| {
            este.alternar_paineis(false, window, cx)
        }))
        .on_action(cx.listener(|este, _: &AlternarPaineis, window, cx| {
            este.alternar_paineis(true, window, cx)
        }))
        .on_action(cx.listener(|este, _: &ZoomAoInicio, _w, cx| {
            este.na_foto_ampliada(cx, |tela, cx| tela.zoom_ao_inicio(cx))
        }))
        .on_action(cx.listener(|este, _: &ZoomAoFim, _w, cx| {
            este.na_foto_ampliada(cx, |tela, cx| tela.zoom_ao_fim(cx))
        }))
        .on_action(cx.listener(|este, _: &ZoomTelaSeguinte, _w, cx| {
            este.na_foto_ampliada(cx, |tela, cx| tela.zoom_por_tela(1, cx))
        }))
        .on_action(cx.listener(|este, _: &ZoomTelaAnterior, _w, cx| {
            este.na_foto_ampliada(cx, |tela, cx| tela.zoom_por_tela(-1, cx))
        }))
    }
}
