//! O roteiro de depuração (`VLB_ROTEIRO`), dado pela raiz.
//!
//! Os passos são lidos em [`crate::depuracao`]; aqui cada um vira o gesto que
//! a tela faria. Só existe em build de depuração.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{px, size, Context, Window};

use super::{Aplicativo, Tela};
use crate::depuracao::{self, Passo};

impl Aplicativo {
    /// Lê `VLB_ROTEIRO` e começa a segui-lo depois de a conta entrar.
    pub(super) fn ligar_o_roteiro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(debug_assertions) {
            return;
        }
        let Some(caminho) = std::env::var_os("VLB_ROTEIRO") else {
            return;
        };
        let passos = match std::fs::read_to_string(&caminho)
            .map_err(|e| e.to_string())
            .and_then(|texto| depuracao::ler_roteiro(&texto))
        {
            Ok(passos) => passos,
            Err(erro) => {
                eprintln!("[roteiro] {}: {erro}", PathBuf::from(&caminho).display());
                return;
            }
        };
        let pasta = std::env::var_os("VLB_FOTOS").map(PathBuf::from);
        // 🐕 O vigia de travamento (`VLB_VIGIA=1`): a batida da interface, a
        // cada 16 ms, numa tarefa que só anda se a thread estiver livre.
        depuracao::vigia::ligar();
        if depuracao::vigia::ligado() {
            cx.spawn(async move |_, cx| loop {
                depuracao::vigia::bater();
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            })
            .detach();
        }
        self._roteiro = Some(cx.spawn_in(window, async move |raiz, cx| {
            // A capa de entrada também se fotografa: os passos antes de a conta
            // entrar rodam com ela na tela.
            for passo in passos {
                if let Passo::Esperar(tempo) = passo {
                    cx.background_executor().timer(tempo).await;
                    continue;
                }
                // 🔁 A rajada: o mesmo passo, sem o respiro entre um e outro.
                if let Passo::Rajada {
                    vezes,
                    intervalo,
                    passo: dentro,
                } = &passo
                {
                    depuracao::vigia::passo(&format!("{passo:?}"));
                    eprintln!("[roteiro] rajada de {vezes}: {dentro:?}");
                    for _ in 0..*vezes {
                        let pasta = pasta.clone();
                        let seguiu = raiz.update_in(cx, |raiz, window, cx| {
                            raiz.dar_o_passo(dentro, pasta.as_deref(), window, cx)
                        });
                        if !matches!(seguiu, Ok(true)) {
                            return;
                        }
                        cx.background_executor().timer(*intervalo).await;
                    }
                    continue;
                }
                depuracao::vigia::passo(&format!("{passo:?}"));
                let pasta = pasta.clone();
                let seguiu = raiz.update_in(cx, |raiz, window, cx| {
                    raiz.dar_o_passo(&passo, pasta.as_deref(), window, cx)
                });
                if !matches!(seguiu, Ok(true)) {
                    return;
                }
                // Um quadro para a tela assentar antes do passo seguinte.
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
            }
            eprintln!("[roteiro] fim");
            depuracao::vigia::relatar();
        }));
    }

    /// Um passo. Devolve se o roteiro continua.
    fn dar_o_passo(
        &mut self,
        passo: &Passo,
        pasta: Option<&std::path::Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        eprintln!("[roteiro] {passo:?}");
        match passo {
            // Quem desenrola a rajada é o laço do roteiro.
            Passo::Esperar(_) | Passo::Rajada { .. } => {}
            Passo::Foto(nome) => {
                let Some(pasta) = pasta else {
                    eprintln!("[roteiro] VLB_FOTOS não definida: a foto {nome} não sai");
                    return true;
                };
                let destino = pasta.join(format!("{nome}.png"));
                match depuracao::fotografar(window, &destino) {
                    Ok(()) => eprintln!("[foto] {}", destino.display()),
                    Err(erro) => eprintln!("[foto] {}: {erro}", destino.display()),
                }
            }
            Passo::Tamanho(largura, altura) => {
                window.resize(size(px(*largura), px(*altura)));
            }
            Passo::Ir(destino) => match destino.as_str() {
                "sessoes" => self.ir_para(Tela::Sessoes, window, cx),
                "galeria" if self.sessao_aberta.is_some() => self.ir_para(Tela::Sessao, window, cx),
                "caixa" => self.ir_para(Tela::Caixa, window, cx),
                "retencao" => self.ir_para(Tela::Retencao, window, cx),
                "nova" => self.ir_para(Tela::NovaSessao, window, cx),
                outro => eprintln!("[roteiro] não sei ir para '{outro}'"),
            },
            Passo::AbrirSessao(posicao) => match self.sessoes.read(cx).id_na_posicao(*posicao) {
                Some(id) => self.entrar_na_sessao(id, cx),
                None => eprintln!("[roteiro] a lista não tem a sessão {posicao}"),
            },
            Passo::Detalhes => self
                .detalhe
                .update(cx, |tela, cx| tela.alternar_detalhes(cx)),
            Passo::Foco(posicao) => {
                let posicao = posicao.saturating_sub(1);
                self.detalhe.update(cx, |tela, cx| {
                    tela.clicar(posicao, Default::default(), cx);
                });
            }
            Passo::Atendimento => self
                .detalhe
                .update(cx, |tela, cx| tela.alternar_atendimento(cx)),
            Passo::Menu => self.alternar_menu_lateral(cx),
            Passo::MenuDoUsuario => self.alternar_menu_da_conta(cx),
            Passo::Tema(nome) => match crate::tema::Escolha::do_nome(nome) {
                Some(escolha) => self.escolher_tema(escolha, window, cx),
                None => eprintln!("[roteiro] tema desconhecido: '{nome}'"),
            },
            Passo::Revelar => {
                self.detalhe.update(cx, |tela, cx| tela.revelar_todas(cx));
            }
            Passo::TelaDoCliente => self.alternar_cliente(cx),
            Passo::FotoDoCliente(nome) => {
                let (Some(pasta), Some(janela)) = (pasta, self.cliente) else {
                    eprintln!("[roteiro] sem VLB_FOTOS ou sem a tela do cliente aberta");
                    return true;
                };
                let destino = pasta.join(format!("{nome}.png"));
                let resultado = janela
                    .update(cx, |_cliente, janela, _cx| {
                        depuracao::fotografar(janela, &destino)
                    })
                    .map_err(|e| e.to_string())
                    .and_then(|r| r);
                match resultado {
                    Ok(()) => eprintln!("[foto] {}", destino.display()),
                    Err(erro) => eprintln!("[foto] {}: {erro}", destino.display()),
                }
            }
            Passo::SairDaConta => self.sair_da_conta(cx),
            Passo::AlternarCaixa => {
                self.caixa_flutuante
                    .update(cx, |caixa, cx| caixa.alternar_painel(cx));
            }
            Passo::TeclaDoCaixa(tecla) => {
                let tecla = *tecla;
                self.caixa_flutuante
                    .update(cx, |caixa, cx| caixa.teclar_f(tecla, window, cx));
            }
            Passo::Painel(pedido) => {
                let pedido = pedido.clone();
                self.revelacao
                    .update(cx, |tela, cx| tela.seguir_o_roteiro_do_painel(&pedido, cx));
            }
            Passo::Revelacao(gesto) => {
                let gesto = gesto.clone();
                self.revelacao
                    .update(cx, |tela, cx| tela.seguir_o_roteiro(&gesto, window, cx));
            }
            Passo::ImportarModal => {
                self.detalhe
                    .update(cx, |tela, cx| tela.abrir_a_importacao(cx));
            }
            Passo::Guias(gesto) => {
                let gesto = gesto.clone();
                self.seguir_o_roteiro_das_guias(&gesto, window, cx);
            }
            Passo::Tira(gesto) => {
                let gesto = gesto.clone();
                self.revelacao.update(cx, |tela, cx| {
                    tela.seguir_o_roteiro_da_tira(&gesto, window, cx)
                });
            }
            Passo::Predefinicoes(gesto) => {
                let gesto = gesto.clone();
                self.revelacao.update(cx, |tela, cx| {
                    tela.seguir_o_roteiro_das_predefinicoes(&gesto, window, cx)
                });
            }
            Passo::Nova(gesto) => {
                use crate::sessoes::nova::tela::{Confirmacao, TipoDeBusca};
                let partes: Vec<&str> = gesto.split_whitespace().collect();
                self.nova_sessao.update(cx, |tela, cx| match partes[..] {
                    ["etapa", n] => tela.ir(n.parse().unwrap_or(1), window, cx),
                    ["buscar", qual] => {
                        let tipo = match qual {
                            "voucher" => TipoDeBusca::Voucher,
                            "compra" => TipoDeBusca::Compra,
                            "parceiro" => TipoDeBusca::Parceiro,
                            _ => TipoDeBusca::Agendamento,
                        };
                        tela.abrir_busca(tipo, window, cx);
                    }
                    ["descartar"] => tela.pedir_confirmacao(Confirmacao::Descartar, cx),
                    ["importar", pasta] => {
                        let fotos = crate::sessoes::arquivos::so_as_fotos(&[pasta.into()]);
                        tela.importar_arquivos(fotos, window, cx);
                    }
                    ["conheceu", valor] => tela.escolher_como_conheceu(valor, window, cx),
                    ["preset", n] => {
                        let n: isize = n.parse().unwrap_or(0);
                        tela.mover_foco_do_preset(-10_000, cx);
                        tela.mover_foco_do_preset(n, cx);
                        tela.marcar_preset_em_foco(cx);
                    }
                    ["proporcao", valor] => {
                        let valor = (valor != "sem").then(|| valor.to_string());
                        tela.escolher_proporcao(valor, cx);
                    }
                    ["titulo", ..] => {
                        let titulo = gesto.trim_start_matches("titulo").trim().to_string();
                        tela.digitar(&titulo, "", "", window, cx);
                    }
                    ["produto", id] => tela.escolher_produto(id, window, cx),
                    ["estudio", id] => tela.escolher_estudio(id, window, cx),
                    ["criar"] => tela.criar(window, cx),
                    ["retomar"] => tela.retomar(window, cx),
                    _ => eprintln!("[roteiro] gesto desconhecido da nova sessão: '{gesto}'"),
                });
            }
            Passo::Janela(gesto) => match gesto.split_whitespace().collect::<Vec<_>>()[..] {
                ["minimizar"] => window.minimize_window(),
                ["fingir_envio", n] => self.sincronias_pendentes = n.parse().unwrap_or(0),
                // 🚨 Fora deste `update`: fechar pergunta à própria janela se
                // pode, e a janela está emprestada ao passo agora.
                [gesto @ ("fechar" | "abrir")] => {
                    let gesto = gesto.to_string();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                        let _ = cx.update(|cx| crate::segundo_plano::gesto_de_roteiro(&gesto, cx));
                    })
                    .detach();
                }
                _ => eprintln!("[roteiro] gesto de janela desconhecido: '{gesto}'"),
            },
            Passo::Importar(pasta) => {
                let fotos = crate::sessoes::arquivos::so_as_fotos(&[pasta.into()]);
                self.detalhe
                    .update(cx, |tela, cx| tela.enviar_arquivos(fotos, cx));
            }
            Passo::Tecla(tecla) => match gpui::Keystroke::parse(tecla) {
                // 🚨 **Adiada**: o despacho chega a esta mesma raiz, que está
                // emprestada ao passo agora.
                Ok(tecla) => window.defer(cx, move |window, cx| {
                    let tratou = window.dispatch_keystroke(tecla.clone(), cx);
                    eprintln!("[roteiro] tecla {tecla:?} tratada: {tratou}");
                }),
                Err(erro) => eprintln!("[roteiro] tecla inválida '{tecla}': {erro}"),
            },
            Passo::Fim => {
                depuracao::vigia::relatar();
                cx.quit();
                return false;
            }
        }
        cx.notify();
        true
    }
}
