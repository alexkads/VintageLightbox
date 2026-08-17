//! Configurações: o que o cache tem dentro, e como esvaziá-lo.
//!
//! É o `settings_dialog.rs` do legado, sem a seção de leiaute — ela existe lá
//! para desfazer um arranjo de painéis que este app não tem
//! (docs/10-MIGRACAO-GPUI.md, fase 4).
//!
//! ## ⚠️ Limpar cache é seguro, e o texto tem de dizer isso
//!
//! Apagar previews **não toca nos arquivos originais**: é trabalho refeito, não
//! trabalho perdido. Sem essa frase à vista, três botões chamados "limpar" ao
//! lado de um número em gigabytes são um convite a não clicar em nenhum — e o
//! cache continua crescendo.

use std::sync::Arc;

use gpui::{div, prelude::*, px, App, Context, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Sizable};
use infrastructure::cache::preview_manager::{CacheStats, PreviewManager};

/// Tamanho em bytes, do jeito que se lê.
///
/// As mesmas quatro faixas do legado (`format_bytes`), com as mesmas casas
/// decimais: GB com duas, MB e KB com uma, e bytes inteiros. ⚠️ **A vírgula é a
/// decimal**, porque o resto do app está em português e um "1.5 GB" no meio de
/// "1.234 miniaturas" faz o ponto significar duas coisas na mesma tela.
pub fn formatar_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    let texto = if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    };

    texto.replace('.', ",")
}

pub struct Configuracoes {
    previews: Arc<PreviewManager>,
    /// O retrato do cache, lido ao abrir e depois de cada limpeza.
    ///
    /// ⚠️ **Não é lido a cada quadro**: `get_stats` são três consultas ao SQLite,
    /// e responder a mesma coisa 60 vezes por segundo é trabalho que ninguém
    /// pediu — no meio de uma limpeza, é trabalho que disputa o mesmo `Mutex`.
    estatisticas: Option<CacheStats>,
    /// O que a última limpeza fez, para a tela poder dizer.
    ultima_limpeza: Option<SharedString>,
}

impl Configuracoes {
    pub fn nova(previews: Arc<PreviewManager>) -> Self {
        Self {
            previews,
            estatisticas: None,
            ultima_limpeza: None,
        }
    }

    /// Relê o cache. Chamado ao abrir e depois de limpar.
    pub fn atualizar(&mut self, cx: &mut Context<Self>) {
        self.estatisticas = Some(self.previews.get_stats());
        cx.notify();
    }

    pub fn estatisticas(&self) -> Option<&CacheStats> {
        self.estatisticas.as_ref()
    }

    pub fn ultima_limpeza(&self) -> Option<&str> {
        self.ultima_limpeza.as_ref().map(|texto| texto.as_ref())
    }

    fn limpar(
        &mut self,
        o_que: &'static str,
        acao: impl FnOnce(&PreviewManager) -> Result<u64, String>,
        cx: &mut Context<Self>,
    ) {
        self.ultima_limpeza = Some(match acao(&self.previews) {
            Ok(quantos) => format!("{quantos} {o_que} apagad{}", plural(quantos)).into(),
            // 🚨 A falha aparece na tela, e não só no terminal. Limpar cache é
            // uma ação que o fotógrafo pede olhando um número de gigabytes: se
            // ela falha em silêncio, o número não muda e a conclusão é que o
            // botão não funciona.
            Err(erro) => format!("Não foi possível limpar: {erro}").into(),
        });
        self.atualizar(cx);
    }

    pub fn limpar_miniaturas(&mut self, cx: &mut Context<Self>) {
        self.limpar("miniaturas", |cache| cache.clear_thumbnails(), cx);
    }

    pub fn limpar_previews(&mut self, cx: &mut Context<Self>) {
        self.limpar("previews", |cache| cache.clear_previews(), cx);
    }

    pub fn limpar_tudo(&mut self, cx: &mut Context<Self>) {
        self.limpar("itens", |cache| cache.clear_all(), cx);
    }

    fn linha(&self, rotulo: &'static str, valor: String, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .gap(px(12.))
            .text_xs()
            .child(div().text_color(cx.theme().muted_foreground).child(rotulo))
            .child(div().truncate().child(SharedString::from(valor)))
    }
}

impl Render for Configuracoes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let estatisticas = self.estatisticas.clone();
        // ⚠️ Os botões **desligam quando não há o que limpar**, como no legado.
        // Um "Limpar miniaturas" aceso sobre um cache vazio responde com "0
        // apagadas", e quem clicou fica sem saber se apagou nada ou se falhou.
        let tem_miniaturas = estatisticas.as_ref().is_some_and(|e| e.thumbnail_count > 0);
        let tem_previews = estatisticas
            .as_ref()
            .is_some_and(|e| e.large_preview_count > 0);

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(14.))
            .size_full()
            .child(div().text_xs().child("Cache de previews"))
            .child(match &estatisticas {
                Some(numeros) => div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(self.linha("miniaturas", numeros.thumbnail_count.to_string(), cx))
                    .child(self.linha(
                        "previews grandes",
                        numeros.large_preview_count.to_string(),
                        cx,
                    ))
                    .child(self.linha("tamanho", formatar_bytes(numeros.total_size_bytes), cx))
                    .child(self.linha("arquivo", numeros.db_path.to_string_lossy().to_string(), cx))
                    .into_any_element(),
                None => div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("lendo o cache…")
                    .into_any_element(),
            })
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .pt(px(4.))
                    .child(
                        Button::new("limpar-miniaturas")
                            .label("Limpar miniaturas")
                            .xsmall()
                            .disabled(!tem_miniaturas)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.limpar_miniaturas(cx);
                            })),
                    )
                    .child(
                        Button::new("limpar-previews")
                            .label("Limpar previews")
                            .xsmall()
                            .disabled(!tem_previews)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.limpar_previews(cx);
                            })),
                    )
                    .child(
                        Button::new("limpar-tudo")
                            .label("Limpar tudo")
                            .xsmall()
                            .danger()
                            .disabled(!tem_miniaturas && !tem_previews)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.limpar_tudo(cx);
                            })),
                    ),
            )
            .children(self.ultima_limpeza.clone().map(|texto| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(texto)
            }))
            .child(
                // ⚠️ A frase que faz os três botões serem clicáveis: limpar cache
                // é trabalho refeito, não trabalho perdido.
                div()
                    .mt(px(4.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Limpar o cache não toca nas fotos originais. As previews são geradas de \
                         novo quando a foto for aberta.",
                    ),
            )
    }
}

fn plural(quantos: u64) -> &'static str {
    if quantos == 1 {
        "o"
    } else {
        "os"
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    fn foto() -> DynamicImage {
        let mut img = RgbaImage::new(4, 4);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([10, 20, 30, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn cache_com(miniaturas: usize, previews: usize) -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("diretório temporário");
        let cache = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));

        for i in 0..miniaturas {
            cache
                .save_thumbnail(&format!("mini-{i}"), &foto())
                .expect("gravar miniatura");
        }
        for i in 0..previews {
            cache
                .save_preview(&format!("prev-{i}"), &foto())
                .expect("gravar preview");
        }

        (cache, dir)
    }

    /// As quatro faixas do legado, com a vírgula decimal do resto do app.
    #[test]
    fn o_tamanho_e_escrito_do_jeito_que_se_le() {
        assert_eq!(formatar_bytes(0), "0 bytes");
        assert_eq!(formatar_bytes(512), "512 bytes");
        assert_eq!(formatar_bytes(2048), "2,0 KB");
        assert_eq!(formatar_bytes(5 * 1024 * 1024), "5,0 MB");
        assert_eq!(formatar_bytes(3 * 1024 * 1024 * 1024), "3,00 GB");
    }

    /// 🚨 Cada botão apaga **o que o nome dele diz**, e não o cache inteiro.
    ///
    /// São três `DELETE` diferentes no mesmo `Mutex`, e trocar dois de lugar
    /// compila: "Limpar miniaturas" levaria junto os previews grandes, que são
    /// os caros de refazer (cada um é um RAW aberto de novo). Nada falharia — o
    /// número simplesmente cairia mais do que devia.
    #[gpui::test]
    fn cada_botao_apaga_so_a_sua_parte(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(3, 2);
        // O tema global do `gpui-component` — o `render` o lê, e sem ele a
        // janela morre no primeiro quadro.
        cx.update(gpui_component::init);
        let janela = cx.add_window(|_window, cx| {
            let mut tela = Configuracoes::nova(cache);
            tela.atualizar(cx);
            tela
        });

        janela
            .update(cx, |tela, _window, cx| {
                let numeros = tela.estatisticas().expect("já leu o cache");
                assert_eq!(numeros.thumbnail_count, 3);
                assert_eq!(numeros.large_preview_count, 2);

                tela.limpar_miniaturas(cx);

                let numeros = tela.estatisticas().expect("releu");
                assert_eq!(numeros.thumbnail_count, 0);
                assert_eq!(
                    numeros.large_preview_count, 2,
                    "limpar miniatura não pode levar preview junto"
                );
                assert_eq!(tela.ultima_limpeza(), Some("3 miniaturas apagados"));

                tela.limpar_previews(cx);
                assert_eq!(tela.estatisticas().expect("releu").large_preview_count, 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// "Limpar tudo" leva os dois.
    #[gpui::test]
    fn limpar_tudo_esvazia_o_cache(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(2, 2);
        cx.update(gpui_component::init);
        let janela = cx.add_window(|_window, cx| {
            let mut tela = Configuracoes::nova(cache);
            tela.atualizar(cx);
            tela
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.limpar_tudo(cx);

                let numeros = tela.estatisticas().expect("releu");
                assert_eq!(numeros.thumbnail_count, 0);
                assert_eq!(numeros.large_preview_count, 0);
                assert_eq!(numeros.total_size_bytes, 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O número na tela é relido **depois** de limpar.
    ///
    /// Sem a releitura, os três botões apagam de verdade e a tela continua
    /// mostrando o tamanho de antes — e a conclusão de quem olha é que o botão
    /// não funcionou. Clicar de novo (e de novo) é o desfecho.
    #[gpui::test]
    fn o_numero_e_relido_depois_de_limpar(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(4, 0);
        cx.update(gpui_component::init);
        let janela = cx.add_window(|_window, cx| {
            let mut tela = Configuracoes::nova(cache);
            tela.atualizar(cx);
            tela
        });

        janela
            .update(cx, |tela, _window, cx| {
                let antes = tela.estatisticas().expect("leu").total_size_bytes;
                assert!(antes > 0);

                tela.limpar_tudo(cx);

                assert_eq!(tela.estatisticas().expect("releu").total_size_bytes, 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// O singular concorda com o número.
    #[test]
    fn uma_miniatura_apagada_nao_vira_apagados() {
        assert_eq!(plural(1), "o");
        assert_eq!(plural(0), "os");
        assert_eq!(plural(9), "os");
    }
}
