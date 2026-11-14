//! Configurações: os avisos sonoros (`crate::sons::tela`), o cache — o que
//! ele tem dentro, e como esvaziá-lo — e a conta (o Google Authenticator,
//! `crate::autenticador_da_conta`, desde 2026-10-18). Uma aba para cada.
//!
//! É o `settings_dialog.rs` do legado, sem a seção de leiaute — ela existe lá
//! para desfazer um arranjo de painéis que este app não tem
//! (docs/10-MIGRACAO-GPUI.md, fase 4).
//!
//! ## ⚠️ Limpar cache é seguro, e o texto tem de dizer isso
//!
//! Apagar previews **não toca nos arquivos originais**: é trabalho refeito, não
//! trabalho perdido. Sem essa frase à vista, botões chamados "limpar" ao
//! lado de um número em gigabytes são um convite a não clicar em nenhum — e o
//! cache continua crescendo.

use std::sync::Arc;

use gpui_kit::component::button::Button;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, FontWeight, SharedString, Window};

use crate::autenticador_da_conta::AutenticadorDaConta;
use crate::pos_venda::porta::Publicador;
use crate::recursos::Icone;
use crate::sons::tela::AvisosSonoros;
use domain::services::pos_venda::Sessao;
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

/// Os períodos da limpeza por idade, em dias (pedido do dono, 14/11/2026).
pub const PERIODOS: [u32; 4] = [15, 30, 60, 90];

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
    /// Se a última limpeza foi uma falha — o toast é vermelho.
    limpeza_falhou: bool,
    /// O período escolhido para a limpeza por idade (um de [`PERIODOS`]).
    periodo: u32,
    /// Quantos itens e quantos bytes estão sem uso há mais de cada período,
    /// lidos junto com as `estatisticas`.
    sem_uso: Vec<(u32, (u64, u64))>,
    /// A última limpeza que já virou toast (`estilo::toast_quando_mudar`).
    limpeza_vista: Option<SharedString>,
    /// A aba aberta: 0 é "Avisos sonoros", 1 é "Cache", 2 é "Conta".
    aba: usize,
    /// A aba dos sons, criada na primeira abertura — os campos dela pedem a
    /// janela, que o `nova` não tem.
    sons: Option<Entity<AvisosSonoros>>,
    /// A aba da conta, criada na primeira abertura com a sessão da vez.
    conta: Option<Entity<AutenticadorDaConta>>,
}

impl Configuracoes {
    pub fn nova(previews: Arc<PreviewManager>) -> Self {
        Self {
            previews,
            estatisticas: None,
            ultima_limpeza: None,
            limpeza_falhou: false,
            periodo: 30,
            sem_uso: Vec::new(),
            limpeza_vista: None,
            aba: 0,
            sons: None,
            conta: None,
        }
    }

    /// Liga a aba da conta à sessão da vez. Chamado ao abrir: quem entrou pode
    /// ter mudado desde a última abertura.
    pub fn ligar_conta(
        &mut self,
        publicador: Arc<dyn Publicador>,
        sessao: Option<Sessao>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.conta {
            Some(conta) => conta.update(cx, |conta, cx| conta.trocar_sessao(sessao, cx)),
            None => {
                self.conta =
                    Some(cx.new(|cx| AutenticadorDaConta::nova(publicador, sessao, window, cx)))
            }
        }
    }

    /// Cria a aba dos sons, se ainda não existe. Chamado ao abrir.
    pub fn preparar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sons.is_none() {
            self.sons = Some(cx.new(|cx| AvisosSonoros::nova(window, cx)));
        }
    }

    pub fn aba(&self) -> usize {
        self.aba
    }

    /// 0 é "Avisos sonoros", 1 é "Cache", 2 é "Conta".
    pub fn ir_para_a_aba(&mut self, aba: usize, cx: &mut Context<Self>) {
        self.aba = aba.min(2);
        cx.notify();
    }

    /// Relê o cache. Chamado ao abrir e depois de limpar.
    pub fn atualizar(&mut self, cx: &mut Context<Self>) {
        self.estatisticas = Some(self.previews.get_stats());
        self.sem_uso = PERIODOS
            .iter()
            .map(|&dias| (dias, self.previews.sem_uso_ha(dias)))
            .collect();
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
        o_que: Nome,
        acao: impl FnOnce(&PreviewManager) -> Result<u64, String>,
        cx: &mut Context<Self>,
    ) {
        let resultado = acao(&self.previews);
        self.limpeza_falhou = resultado.is_err();
        // Uma limpeza nova sempre vira toast, mesmo com a frase igual à
        // anterior.
        self.limpeza_vista = None;
        self.ultima_limpeza = Some(match resultado {
            Ok(quantos) => apagados(quantos, o_que).into(),
            // 🚨 A falha aparece na tela, e não só no terminal. Limpar cache é
            // uma ação que o fotógrafo pede olhando um número de gigabytes: se
            // ela falha em silêncio, o número não muda e a conclusão é que o
            // botão não funciona.
            Err(erro) => format!("Não foi possível limpar: {erro}").into(),
        });
        self.atualizar(cx);
    }

    pub fn limpar_miniaturas(&mut self, cx: &mut Context<Self>) {
        self.limpar(
            ("miniatura", "miniaturas", true),
            |cache| cache.clear_thumbnails(),
            cx,
        );
    }

    pub fn limpar_previews(&mut self, cx: &mut Context<Self>) {
        self.limpar(
            ("preview", "previews", false),
            |cache| cache.clear_previews(),
            cx,
        );
    }

    /// Escolhe o período da limpeza por idade.
    pub fn escolher_periodo(&mut self, dias: u32, cx: &mut Context<Self>) {
        self.periodo = dias;
        cx.notify();
    }

    /// Itens e bytes sem uso há mais do período escolhido.
    pub fn sem_uso_do_periodo(&self) -> (u64, u64) {
        self.sem_uso
            .iter()
            .find(|(dias, _)| *dias == self.periodo)
            .map(|(_, numeros)| *numeros)
            .unwrap_or((0, 0))
    }

    /// Apaga o que está sem uso há mais do período escolhido.
    pub fn limpar_por_periodo(&mut self, cx: &mut Context<Self>) {
        let dias = self.periodo;
        self.limpar(
            ("item", "itens", false),
            move |cache| cache.limpar_sem_uso_ha(dias),
            cx,
        );
    }
}

impl Render for Configuracoes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 🍞 O resultado da limpeza é toast do kit, verde ou vermelho.
        crate::estilo::toast_quando_mudar(
            &mut self.limpeza_vista,
            self.ultima_limpeza.clone(),
            if self.limpeza_falhou {
                crate::estilo::Toast::Erro
            } else {
                crate::estilo::Toast::Sucesso
            },
            window,
            cx,
        );
        let tela = cx.entity().downgrade();
        let abas = TabBar::new("abas-das-configuracoes")
            .segmented()
            .small()
            .selected_index(self.aba)
            .on_click(move |indice, _, cx| {
                let indice = *indice;
                let _ = tela.update(cx, |tela, cx| {
                    tela.aba = indice;
                    cx.notify();
                });
            })
            .child(
                Tab::new()
                    .debug_selector(|| "aba-avisos-sonoros".into())
                    .label("Avisos sonoros"),
            )
            .child(
                Tab::new()
                    .debug_selector(|| "aba-cache".into())
                    .label("Cache"),
            )
            .child(
                Tab::new()
                    .debug_selector(|| "aba-conta".into())
                    .label("Conta"),
            );
        let miolo = match (self.aba, &self.sons) {
            (0, Some(sons)) => sons.clone().into_any_element(),
            (0, None) => div().into_any_element(),
            (2, _) => match &self.conta {
                Some(conta) => conta.clone().into_any_element(),
                None => div().into_any_element(),
            },
            _ => self.cache(cx).into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .size_full()
            .child(div().flex().child(abas))
            .child(miolo)
    }
}

impl Configuracoes {
    /// Um dos três cartões do alto: ícone e nome, o número grande, uma linha
    /// do que ele é e, no pé, o botão da parte dele.
    fn cartao_do_cache(
        &self,
        icone: Icone,
        nome: &'static str,
        numero: SharedString,
        detalhe: &'static str,
        botao: Button,
        cx: &App,
    ) -> impl IntoElement {
        let caixa = crate::tema::medidas().cartao;
        let muted = cx.theme().muted_foreground;
        crate::estilo::cartao(cx)
            .flex_1()
            .min_w_0()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .size_full()
                    .p(px(caixa.respiro))
                    .gap(px(caixa.vao.min(12.)))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .items_center()
                            .text_xs()
                            .text_color(muted)
                            .child(Icon::new(icone).size(px(14.)).text_color(muted))
                            .child(nome),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(numero),
                    )
                    .child(div().flex_1().text_xs().text_color(muted).child(detalhe))
                    .child(div().pt(px(4.)).child(botao.w_full())),
            )
    }

    /// A limpeza por idade: o período (15, 30, 60 ou 90 dias sem uso), o que
    /// ele libera e o botão. O número aparece **antes** do clique, para quem
    /// escolhe saber o que vai sair.
    fn cartao_do_periodo(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let caixa = crate::tema::medidas().cartao;
        let muted = cx.theme().muted_foreground;
        let (itens, bytes) = self.sem_uso_do_periodo();
        let escolhido = PERIODOS
            .iter()
            .position(|&dias| dias == self.periodo)
            .unwrap_or(1);
        let tela = cx.entity().downgrade();
        let periodos = PERIODOS.iter().fold(
            TabBar::new("periodos-do-cache")
                .segmented()
                .small()
                .selected_index(escolhido)
                .on_click(move |indice, _, cx| {
                    let dias = PERIODOS[(*indice).min(PERIODOS.len() - 1)];
                    let _ = tela.update(cx, |tela, cx| tela.escolher_periodo(dias, cx));
                }),
            |barra, dias| {
                barra.child(
                    Tab::new()
                        .debug_selector(move || format!("periodo-{dias}"))
                        .label(format!("> {dias} dias")),
                )
            },
        );
        let resumo: SharedString = if itens == 0 {
            "Nada sem uso há tanto tempo".into()
        } else {
            format!(
                "{} {} · {}",
                milhar(itens),
                if itens == 1 { "item" } else { "itens" },
                formatar_bytes(bytes)
            )
            .into()
        };

        crate::estilo::cartao(cx).bg(cx.theme().background).child(
            v_flex()
                .p(px(caixa.respiro))
                .gap(px(10.))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_xs()
                        .text_color(muted)
                        .child(Icon::new(Icone::Clock).size(px(14.)).text_color(muted))
                        .child(div().flex_1().child("Limpar o que está sem uso há mais de"))
                        .child(
                            div()
                                .flex_none()
                                .text_color(if itens == 0 {
                                    muted
                                } else {
                                    cx.theme().foreground
                                })
                                .child(resumo),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(12.))
                        .items_center()
                        .child(div().flex_none().child(periodos))
                        .child(div().flex_1())
                        .child(
                            crate::estilo::botao_contorno_pequeno("limpar-por-periodo", cx)
                                .label("Limpar")
                                .disabled(itens == 0)
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.limpar_por_periodo(cx);
                                })),
                        ),
                ),
        )
    }

    fn cache(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let estatisticas = self.estatisticas.clone();
        let muted = cx.theme().muted_foreground;
        // ⚠️ Os botões **desligam quando não há o que limpar**, como no legado.
        // Um "Limpar miniaturas" aceso sobre um cache vazio responde com "0
        // apagadas", e quem clicou fica sem saber se apagou nada ou se falhou.
        let tem_miniaturas = estatisticas.as_ref().is_some_and(|e| e.thumbnail_count > 0);
        let tem_previews = estatisticas
            .as_ref()
            .is_some_and(|e| e.large_preview_count > 0);
        // Enquanto o cache não foi lido, o cartão mostra o travessão e não um
        // "0" que pareceria um cache vazio.
        let ou_traco = |numero: Option<String>| -> SharedString {
            numero.unwrap_or_else(|| "—".to_string()).into()
        };
        let tamanho = ou_traco(
            estatisticas
                .as_ref()
                .map(|e| formatar_bytes(e.total_size_bytes)),
        );
        let miniaturas = ou_traco(estatisticas.as_ref().map(|e| milhar(e.thumbnail_count)));
        let previews = ou_traco(estatisticas.as_ref().map(|e| milhar(e.large_preview_count)));
        let caminho = estatisticas.as_ref().map(|e| e.db_path.clone());
        let pasta = caminho
            .as_ref()
            .and_then(|arquivo| arquivo.parent().map(|pasta| pasta.to_path_buf()));

        let abrir_pasta = crate::estilo::botao_contorno_pequeno("abrir-pasta-do-cache", cx)
            .label("Abrir a pasta")
            .disabled(pasta.is_none())
            .on_click(move |_, _, _| {
                if let Some(pasta) = &pasta {
                    crate::bandeja::abrir_pasta(pasta);
                }
            });
        let limpar_miniaturas = crate::estilo::botao_contorno_pequeno("limpar-miniaturas", cx)
            .label("Limpar miniaturas")
            .disabled(!tem_miniaturas)
            .on_click(cx.listener(|tela, _ev, _window, cx| {
                tela.limpar_miniaturas(cx);
            }));
        let limpar_previews = crate::estilo::botao_contorno_pequeno("limpar-previews", cx)
            .label("Limpar previews")
            .disabled(!tem_previews)
            .on_click(cx.listener(|tela, _ev, _window, cx| {
                tela.limpar_previews(cx);
            }));

        v_flex()
            .gap(px(16.))
            .size_full()
            .child(
                v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Cache de previews"),
                    )
                    .child(div().text_xs().text_color(muted).child(
                        "Imagens prontas que fazem a grade e a Revelação abrirem rápido. \
                         O que faltar é refeito sozinho.",
                    )),
            )
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_stretch()
                    .child(self.cartao_do_cache(
                        Icone::HardDrive,
                        "Tamanho no disco",
                        tamanho,
                        "Miniaturas e previews",
                        abrir_pasta,
                        cx,
                    ))
                    .child(self.cartao_do_cache(
                        Icone::Columns3,
                        "Miniaturas",
                        miniaturas,
                        "Da grade e da tira",
                        limpar_miniaturas,
                        cx,
                    ))
                    .child(self.cartao_do_cache(
                        Icone::Maximize,
                        "Previews grandes",
                        previews,
                        "Da Revelação e da lupa",
                        limpar_previews,
                        cx,
                    )),
            )
            .child(self.cartao_do_periodo(cx))
            .children(caminho.map(|caminho| {
                h_flex()
                    .min_w_0()
                    .gap(px(6.))
                    .items_center()
                    .text_xs()
                    .text_color(muted)
                    .child(Icon::new(Icone::File).size(px(12.)).text_color(muted))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .child(SharedString::from(caminho.to_string_lossy().to_string())),
                    )
            }))
            // ⚠️ A frase que faz os botões de limpar serem clicáveis: limpar cache
            // é trabalho refeito, não trabalho perdido.
            .child(crate::estilo::aviso(
                "Limpar o cache não toca nas fotos originais. As previews são geradas de \
                 novo quando a foto for aberta.",
                false,
                cx,
            ))
    }
}

/// Número inteiro com o ponto de milhar: "4.146", e não "4146".
fn milhar(numero: u64) -> String {
    let digitos = numero.to_string();
    let mut texto = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, digito) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            texto.push('.');
        }
        texto.push(digito);
    }
    texto
}

/// "3 miniaturas apagadas", "1 preview apagado": o número, o nome e o
/// particípio concordando com os dois.
fn apagados(quantos: u64, nome: Nome) -> String {
    let (singular, plural_do_nome, feminino) = nome;
    let nome = if quantos == 1 {
        singular
    } else {
        plural_do_nome
    };
    let vogal = if feminino { "a" } else { "o" };
    let fim = if quantos == 1 { "" } else { "s" };
    format!("{} {nome} apagad{vogal}{fim}", milhar(quantos))
}

/// Singular, plural e se é feminino.
type Nome = (&'static str, &'static str, bool);

#[cfg(test)]
mod testes {
    use super::*;

    use gpui_kit::TestAppContext;
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
    #[gpui_kit::test]
    fn cada_botao_apaga_so_a_sua_parte(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(3, 2);
        // O tema global do `gpui-component` — o `render` o lê, e sem ele a
        // janela morre no primeiro quadro.
        cx.update(gpui_kit::init);
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
                assert_eq!(tela.ultima_limpeza(), Some("3 miniaturas apagadas"));

                tela.limpar_previews(cx);
                assert_eq!(tela.estatisticas().expect("releu").large_preview_count, 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 O número na tela é relido **depois** de limpar.
    ///
    /// Sem a releitura, os três botões apagam de verdade e a tela continua
    /// mostrando o tamanho de antes — e a conclusão de quem olha é que o botão
    /// não funcionou. Clicar de novo (e de novo) é o desfecho.
    #[gpui_kit::test]
    fn o_numero_e_relido_depois_de_limpar(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(4, 0);
        // Só miniaturas: limpá-las esvazia o cache.
        cx.update(gpui_kit::init);
        let janela = cx.add_window(|_window, cx| {
            let mut tela = Configuracoes::nova(cache);
            tela.atualizar(cx);
            tela
        });

        janela
            .update(cx, |tela, _window, cx| {
                let antes = tela.estatisticas().expect("leu").total_size_bytes;
                assert!(antes > 0);

                tela.limpar_miniaturas(cx);

                assert_eq!(tela.estatisticas().expect("releu").total_size_bytes, 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// O singular e o feminino concordam com o número e com o nome.
    #[test]
    fn uma_miniatura_apagada_nao_vira_apagados() {
        const MINIATURA: Nome = ("miniatura", "miniaturas", true);
        const PREVIEW: Nome = ("preview", "previews", false);
        assert_eq!(apagados(1, MINIATURA), "1 miniatura apagada");
        assert_eq!(apagados(3, MINIATURA), "3 miniaturas apagadas");
        assert_eq!(apagados(1, PREVIEW), "1 preview apagado");
        assert_eq!(apagados(4758, PREVIEW), "4.758 previews apagados");
    }

    /// 🚨 O período escolhido decide o que sai: o cache recém-gravado não tem
    /// nada sem uso há 15 dias, e "Limpar" por período não leva nada.
    #[gpui_kit::test]
    fn o_periodo_so_leva_o_que_esta_sem_uso(cx: &mut TestAppContext) {
        let (cache, _dir) = cache_com(2, 2);
        cx.update(gpui_kit::init);
        let janela = cx.add_window(|_window, cx| {
            let mut tela = Configuracoes::nova(cache);
            tela.atualizar(cx);
            tela
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_periodo(15, cx);
                assert_eq!(tela.sem_uso_do_periodo(), (0, 0));

                tela.limpar_por_periodo(cx);
                let numeros = tela.estatisticas().expect("releu");
                assert_eq!(
                    (numeros.thumbnail_count, numeros.large_preview_count),
                    (2, 2),
                    "nada estava sem uso"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// O ponto de milhar do resto do app.
    #[test]
    fn o_numero_leva_o_ponto_de_milhar() {
        assert_eq!(milhar(0), "0");
        assert_eq!(milhar(999), "999");
        assert_eq!(milhar(4146), "4.146");
        assert_eq!(milhar(1_234_567), "1.234.567");
    }
}
