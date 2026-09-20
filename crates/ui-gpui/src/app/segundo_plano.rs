//! O que a raiz conta à bandeja (`crate::segundo_plano`), e o que a bandeja
//! pede a ela.

use gpui::{App, Context};

use super::Aplicativo;
use crate::segundo_plano::frases::Retrato;

impl Aplicativo {
    /// O retrato do trabalho em segundo plano, lido a cada volta da bandeja.
    ///
    /// ⚠️ Sem o que mora fora da raiz (tamanho do catálogo, pilha local, a
    /// hora): isso é o laço quem sabe.
    pub(crate) fn retrato_do_segundo_plano(&self, cx: &App) -> Retrato {
        // 🔑 **"Esperando nota" é a área temporária**: foto importada numa
        // sessão, que ainda não está no site e só sobe quando ganhar nota.
        let sem_nota = if self.conta.is_some() {
            self.biblioteca.read(cx).contar_fotos(|f| {
                f.sessao_id.is_some() && f.pos_venda_foto_id.is_none() && f.rating == 0
            })
        } else {
            0
        };
        Retrato {
            subindo: self.sincronias_pendentes,
            sem_nota,
            recusadas: self.recusas.len(),
            refazendo: self.reposicoes_pendentes,
            conta: self.conta.as_ref().map(|c| c.email.clone()),
            ultimo_envio: self.ultimo_envio,
            ..Default::default()
        }
    }

    /// Fecha a segunda tela, se aberta. A janela principal foi para a bandeja,
    /// e o monitor do cliente não pode ficar com a tela sozinha.
    pub(crate) fn fechar_tela_do_cliente(&mut self, cx: &mut Context<Self>) {
        if let Some(janela) = self.cliente.take() {
            let _ = janela.update(cx, |_cliente, window, _cx| window.remove_window());
            self.detalhe
                .update(cx, |tela, cx| tela.definir_cliente_aberta(false, cx));
        }
    }
}

#[cfg(test)]
mod testes {
    use gpui::TestAppContext;

    use crate::app::testes::{acervo, portas, previews_descartaveis};
    use crate::app::Aplicativo;

    #[gpui::test]
    fn o_retrato_conta_o_que_a_raiz_espera_do_site(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
        });
        janela
            .update(cx, |app, _window, cx| {
                let r = app.retrato_do_segundo_plano(cx);
                assert_eq!(r.subindo, 0);
                assert_eq!(r.recusadas, 0);
                assert!(!r.ha_envio_pendente());

                app.sincronias_pendentes = 2;
                app.recusas.push("410: foto apagada".into());
                app.ultimo_envio = Some(123);
                let r = app.retrato_do_segundo_plano(cx);
                assert_eq!(r.subindo, 2);
                assert_eq!(r.recusadas, 1);
                assert_eq!(r.ultimo_envio, Some(123));
                assert!(r.ha_envio_pendente(), "fechar agora esconderia a janela");
            })
            .expect("a janela deve estar aberta");
    }
}
