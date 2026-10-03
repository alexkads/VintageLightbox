//! O roteiro de depuração (`VLB_ROTEIRO`), dado pela raiz.
//!
//! Os passos são lidos em [`crate::depuracao`]; aqui cada um vira o gesto que
//! a tela faria. Só existe em build de depuração.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{px, size, Context, Window};

use super::{Aplicativo, Tela};
use crate::depuracao::{self, Passo};

impl Aplicativo {
    /// Lê `VLB_ROTEIRO` e começa a segui-lo depois de a conta entrar.
    pub(super) fn ligar_o_roteiro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !depuracao::ferramentas_ligadas() {
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
                // 🖱️ Arrastar e rolar: um evento por volta, com o tempo entre
                // eles — pelo `dispatch_event`, o mesmo caminho do evento
                // nativo depois de traduzido, nos três sistemas.
                if let Some(eventos) = eventos_do_gesto(&passo) {
                    eprintln!("[roteiro] {passo:?}");
                    let intervalo = match &passo {
                        Passo::Varrer { intervalo, .. } | Passo::Rolar { intervalo, .. } => {
                            *intervalo
                        }
                        _ => Duration::ZERO,
                    };
                    for evento in eventos {
                        // 🚨 Adiado, como a pinça: o despacho chega à raiz, que
                        // está emprestada a este `update`.
                        let seguiu = raiz.update_in(cx, |_, window, cx| {
                            window.defer(cx, move |window, cx| {
                                window.dispatch_event(evento, cx);
                            });
                        });
                        if seguiu.is_err() {
                            return;
                        }
                        cx.background_executor().timer(intervalo).await;
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
                "backup" => self.ir_para(Tela::Backup, window, cx),
                "chatbot" => self.ir_para(Tela::Chatbot, window, cx),
                "agenda" => self.ir_para(Tela::Agenda, window, cx),
                "recuperacao" => self.ir_para(Tela::Recuperacao, window, cx),
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
            Passo::Conversa(posicao) => {
                let chave = self
                    .chatbot
                    .read(cx)
                    .visiveis()
                    .get(posicao.saturating_sub(1))
                    .map(|c| c.chave.clone());
                match chave {
                    Some(chave) => self.chatbot.update(cx, |t, cx| t.abrir(chave, cx)),
                    None => eprintln!("[roteiro] o chatbot não tem a conversa {posicao}"),
                }
            }
            Passo::ConferirChatbot(minimo) => {
                let lidas = self.chatbot.read(cx).visiveis().len();
                assert!(
                    lidas >= *minimo,
                    "[roteiro] o chatbot deveria listar ao menos {minimo} conversas, e lista {lidas}"
                );
                eprintln!("[roteiro] chatbot com {lidas} conversas");
            }
            Passo::ConferirNovidades(minimo) => {
                let novidades = self.chatbot.read(cx).quantas_novidades();
                assert!(
                    novidades >= *minimo,
                    "[roteiro] o chatbot deveria ter ao menos {minimo} conversas com mensagem nova, e tem {novidades}"
                );
                eprintln!("[roteiro] chatbot com {novidades} novidades");
            }
            Passo::ConferirAgenda(minimo) => {
                let lidos = self.agenda.read(cx).ensaios.len();
                assert!(
                    lidos >= *minimo,
                    "[roteiro] a agenda deveria ter ao menos {minimo} agendamentos, e tem {lidos}"
                );
                eprintln!("[roteiro] agenda com {lidos} agendamentos");
            }
            Passo::ConferirConta => {
                assert!(
                    self.entrou(),
                    "[roteiro] a conta local ainda não foi autorizada; conclua a entrada antes de testar a galeria"
                );
                eprintln!("[roteiro] conta autorizada");
            }
            Passo::ConferirSelecao(esperadas) => {
                let marcadas = self.detalhe.read(cx).quantas_marcadas();
                assert_eq!(
                    marcadas, *esperadas,
                    "[roteiro] a janela real deveria ter {esperadas} fotos selecionadas"
                );
                eprintln!("[roteiro] seleção confirmada: {marcadas} fotos");
            }
            Passo::Atendimento => self
                .detalhe
                .update(cx, |tela, cx| tela.alternar_atendimento(window, cx)),
            Passo::Menu => self.alternar_menu_lateral(cx),
            Passo::DadosDoCliente => self
                .detalhe
                .update(cx, |tela, cx| tela.editar_dados_do_cliente(cx)),
            // 🔑 Dois passos, e não um: o pedido da sessão chega à raiz como
            // evento, depois que este passo termina — conferir aqui mesmo dava
            // "não abriu" com o diálogo a caminho.
            Passo::Negociar(tipo) if tipo.is_empty() => {
                let foco = self.detalhe.read(cx).em_foco().map(|f| f.id.clone());
                let Some(foco) = foco else {
                    panic!("[roteiro] negociar pede uma foto em foco (`foco 1`)");
                };
                eprintln!("[roteiro] negociar a foto {foco}");
                self.detalhe
                    .update(cx, |tela, cx| tela.pedir_negociacao(vec![foco], true, cx));
            }
            Passo::Negociar(tipo) => {
                assert!(
                    self.no_balcao(),
                    "[roteiro] o diálogo da negociação deveria estar aberto (`negociar` antes)"
                );
                let tipo = match tipo.as_str() {
                    "cortesia" => biblioteca_core::negociacao::Tipo::Cortesia,
                    "desconto" => biblioteca_core::negociacao::Tipo::Desconto,
                    "parceiro" => biblioteca_core::negociacao::Tipo::Parceiro,
                    "outro" => biblioteca_core::negociacao::Tipo::Outro,
                    outro => panic!("[roteiro] tipo de negociação desconhecido: '{outro}'"),
                };
                self.balcao
                    .update(cx, |tela, cx| tela.mudar_tipo(tipo, window, cx));
                eprintln!("[roteiro] negociação: {}", self.balcao.read(cx).titulo());
            }
            Passo::Filtros => self
                .sessoes
                .update(cx, |tela, cx| tela.alternar_filtros(cx)),
            Passo::Graficos => self.sessoes.update(cx, |tela, cx| tela.abrir_graficos(cx)),
            Passo::FormasDoCaixa => self.sessoes.update(cx, |tela, cx| tela.alternar_formas(cx)),
            Passo::Buscar(texto) => self
                .sessoes
                .update(cx, |tela, cx| tela.buscar(texto, window, cx)),
            Passo::Periodo(faixa) => {
                let faixa =
                    faixa
                        .as_ref()
                        .map(|(de, ate)| biblioteca_core::sessoes::FaixaDeDatas {
                            de: de.clone(),
                            ate: ate.clone(),
                        });
                self.sessoes
                    .update(cx, |tela, cx| tela.escolher_periodo_para_teste(faixa, cx));
            }
            Passo::MenuDoUsuario => self.alternar_menu_da_conta(window, cx),
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
            Passo::ImportarOrigem => {
                self.detalhe
                    .update(cx, |tela, cx| tela.abrir_a_origem(window, cx));
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
            Passo::Editor(gesto) => {
                let gesto = gesto.clone();
                self.seguir_o_roteiro_do_editor(&gesto, pasta, cx);
            }
            Passo::Exportacao(gesto) => {
                use crate::exportacao::tela::Modo;
                use domain::value_objects::FormatoDeSaida;
                let partes: Vec<&str> = gesto.splitn(2, ' ').collect();
                match partes[..] {
                    ["abrir"] => self.exportar(cx),
                    ["pasta", pasta] => {
                        let pasta = PathBuf::from(pasta);
                        self.exportacao
                            .update(cx, |tela, cx| tela.escolher_pasta_em(pasta, cx));
                    }
                    ["formato", qual] => {
                        let formato = FormatoDeSaida::TODOS
                            .into_iter()
                            .find(|f| f.extensao() == qual)
                            .unwrap_or_default();
                        self.exportacao
                            .update(cx, |tela, cx| tela.escolher_formato(formato, cx));
                    }
                    ["uso", qual] => {
                        let modo = if qual == "previa" {
                            Modo::Previa
                        } else {
                            Modo::Entrega
                        };
                        self.exportacao
                            .update(cx, |tela, cx| tela.escolher_modo(modo, cx));
                    }
                    ["exportar"] => self.exportacao.update(cx, |tela, cx| tela.exportar(cx)),
                    ["parar"] => self.exportacao.update(cx, |tela, cx| tela.parar(cx)),
                    ["estado"] => {
                        let tela = self.exportacao.read(cx);
                        eprintln!(
                            "[roteiro] exportacao: {} · {:?} · falhas {:?}",
                            tela.resumo(),
                            tela.progresso(),
                            tela.falhas()
                        );
                    }
                    _ => eprintln!("[roteiro] exportacao: gesto desconhecido {gesto}"),
                }
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
                // O app aberto pelo roteiro nasce atrás das outras janelas, e o
                // que depende do foco (o toast da mensagem, e não o aviso do
                // sistema) só aparece com ele na frente.
                ["frente"] => {
                    cx.activate(true);
                    window.activate_window();
                }
                ["fingir_envio", n] => self.sincronias_pendentes = n.parse().unwrap_or(0),
                // Uma recusa de mentira na lista local — nada vai ao site.
                ["fingir_recusa", ..] => self.recusas.push(
                    gesto
                        .trim_start()
                        .trim_start_matches("fingir_recusa")
                        .trim()
                        .to_string(),
                ),
                // O clique nos recusados do rodapé.
                ["recusas"] => self.vendo_recusas = !self.vendo_recusas,
                // 🚨 Fora deste `update`: fechar pergunta à própria janela se
                // pode, e a janela está emprestada ao passo agora.
                [gesto @ ("fechar" | "abrir")] => {
                    let gesto = gesto.to_string();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                        cx.update(|cx| crate::segundo_plano::gesto_de_roteiro(&gesto, cx));
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
            Passo::Tecla(tecla) => match gpui_kit::Keystroke::parse(tecla) {
                // 🚨 **Adiada**: o despacho chega a esta mesma raiz, que está
                // emprestada ao passo agora.
                Ok(tecla) => window.defer(cx, move |window, cx| {
                    let tratou = window.dispatch_keystroke(tecla.clone(), cx);
                    eprintln!("[roteiro] tecla {tecla:?} tratada: {tratou}");
                }),
                Err(erro) => eprintln!("[roteiro] tecla inválida '{tecla}': {erro}"),
            },
            Passo::TeclaReal {
                codigo,
                modificadores,
            } => match depuracao::tecla_nativa(window, *codigo, *modificadores) {
                Ok(()) => {
                    eprintln!("[roteiro] tecla real {codigo} mods={modificadores:#x} na fila")
                }
                Err(erro) => eprintln!("[roteiro] tecla real {codigo}: {erro}"),
            },
            Passo::MouseReal {
                tipo,
                x,
                y,
                modificadores,
                na_janela,
            } => {
                let (px_, py_) = if *na_janela {
                    (*x, *y)
                } else {
                    let palco = self.revelacao.read(cx).palco_da_foto();
                    (
                        f32::from(palco.origin.x) + f32::from(palco.size.width) * x,
                        f32::from(palco.origin.y) + f32::from(palco.size.height) * y,
                    )
                };
                match depuracao::mouse_nativo(window, tipo, px_, py_, *modificadores) {
                    Ok(()) => eprintln!("[roteiro] mouse {tipo} em ({px_:.0}, {py_:.0})"),
                    Err(erro) => eprintln!("[roteiro] mouse {tipo}: {erro}"),
                }
            }
            Passo::Pinca { delta, x, y } => {
                let palco = self.revelacao.read(cx).palco_da_foto();
                let posicao = gpui_kit::point(
                    palco.origin.x + palco.size.width * *x,
                    palco.origin.y + palco.size.height * *y,
                );
                let evento = gpui_kit::PinchEvent {
                    position: posicao,
                    delta: *delta,
                    modifiers: Default::default(),
                    phase: gpui_kit::TouchPhase::Moved,
                };
                // 🚨 Adiada, como a tecla: o despacho chega a esta raiz.
                window.defer(cx, move |window, cx| {
                    window.dispatch_event(gpui_kit::PlatformInput::Pinch(evento), cx);
                });
                eprintln!("[roteiro] pinça {delta} em ({x}, {y})");
            }
            // Desenrolados pelo laço do roteiro.
            Passo::Varrer { .. } | Passo::Rolar { .. } => {}
            Passo::Aviso { erro, texto } => self.avisar_em_toast(texto.clone(), *erro, cx),
            Passo::Novidades(acao) => {
                use crate::atualizacao::faixa::Pedido;
                let mut partes = acao.split_whitespace();
                let pedido = match (partes.next(), partes.next().and_then(|n| n.parse().ok())) {
                    (Some("desta"), _) => Some(Pedido::VerEstaVersao),
                    (Some("ver"), Some(indice)) => Some(Pedido::VerVersao(indice)),
                    (Some("fechar"), _) => Some(Pedido::FecharNovidades),
                    _ => None,
                };
                match pedido {
                    Some(pedido) => self.atender(pedido, cx),
                    None => eprintln!("[roteiro] novidades: não entendi '{acao}'"),
                }
            }
            Passo::Desempenho(acao) => {
                let painel = self.desempenho.clone();
                match acao.as_str() {
                    "abrir" | "fechar" => {
                        let aberto = painel.read(cx).aberto();
                        if aberto != (acao == "abrir") {
                            painel.update(cx, |p, cx| p.alternar(cx));
                        }
                    }
                    "iniciar" => painel.update(cx, |p, cx| p.iniciar(cx)),
                    "parar" => painel.update(cx, |p, cx| p.parar(cx)),
                    "salvar" => painel.update(cx, |p, cx| p.salvar_pelo_roteiro(cx)),
                    // `desempenho foto 03-painel` — fotografa a janela do painel.
                    foto if foto.starts_with("foto ") => {
                        let nome = foto.trim_start_matches("foto ").trim().to_string();
                        let janela = painel.read(cx).janela();
                        if let (Some(janela), Some(pasta)) = (janela, pasta) {
                            let destino = pasta.join(format!("{nome}.png"));
                            cx.defer(move |cx| {
                                let _ =
                                    janela.update(cx, |_, window, _| {
                                        match depuracao::fotografar(window, &destino) {
                                            Ok(()) => eprintln!("[foto] {}", destino.display()),
                                            Err(e) => {
                                                eprintln!("[foto] {}: {e}", destino.display())
                                            }
                                        }
                                    });
                            });
                        }
                    }
                    "relatorio" => match painel.read(cx).relatorio_em_texto() {
                        Some(texto) => eprintln!("[desempenho]\n{texto}"),
                        None => eprintln!("[desempenho] sem sessão montada ainda"),
                    },
                    outra => eprintln!("[roteiro] desempenho: ação desconhecida '{outra}'"),
                }
            }
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

/// Os eventos de um arrasto (`varrer`) ou de uma rolagem (`rolar`).
fn eventos_do_gesto(passo: &Passo) -> Option<Vec<gpui_kit::PlatformInput>> {
    use gpui_kit::{
        point, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
        ScrollDelta, ScrollWheelEvent, TouchPhase,
    };
    let ponto = |(x, y): (f32, f32)| point(px(x), px(y));
    match passo {
        Passo::Varrer { vezes, de, ate, .. } => {
            let mut eventos = vec![PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: ponto(*de),
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            })];
            let n = (*vezes).max(1);
            for i in 1..=n {
                let f = i as f32 / n as f32;
                eventos.push(PlatformInput::MouseMove(MouseMoveEvent {
                    position: ponto((de.0 + (ate.0 - de.0) * f, de.1 + (ate.1 - de.1) * f)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }));
            }
            eventos.push(PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: ponto(*ate),
                modifiers: Default::default(),
                click_count: 1,
            }));
            Some(eventos)
        }
        Passo::Rolar { vezes, em, dy, .. } => Some(
            (0..*vezes)
                .map(|i| {
                    PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position: ponto(*em),
                        delta: ScrollDelta::Pixels(point(px(0.), px(*dy))),
                        modifiers: Default::default(),
                        touch_phase: if i == 0 {
                            TouchPhase::Started
                        } else {
                            TouchPhase::Moved
                        },
                    })
                })
                .collect(),
        ),
        _ => None,
    }
}
