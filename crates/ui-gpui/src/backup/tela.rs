//! A tela do backup — a mesma da web, gesto por gesto.
//!
//! ```text
//! ┌ Backup de arquivos                      [Atualizar] [Escolher pasta] ─┐
//! ├ Acervo › 2026 › ensaio-silva        [x] Gerar WebP ao lado  [2560 px] ┤
//! ├───────────────────────────────────────────────────────────────────────┤
//! │  ╭ Solte aqui pastas ou arquivos ─────────────────────────────────╮   │
//! │  │ Enviando — 12 de 31 · 240 MB de 1,4 GB                   38%   │   │
//! │  │ ▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │   │
//! │  ╰─────────────────────────────────────────────────────────────────╯  │
//! │  📁 2026                                                              │
//! │  📄 DSC_01.NEF                        42,1 MB   18/09/2026 10:12   🗑  │
//! └───────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::Sessao;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, img, prelude::*, px, Context, FocusHandle, KeyDownEvent, RenderImage, SharedString, Task,
    Window,
};

use crate::estilo;
use crate::recursos::Icone;

use super::arrastar::arquivos_soltos;
use super::compactar::{eh_visualizavel, nome_do_zip, Compactador};
use super::conversao::{converter_para_webp, Conversao};
use super::escolha::EscolhaDoBackup;
use super::fila::{andamento_de, caminho_no_acervo, em_tamanho, tipo_do_arquivo, Peca, Situacao};
use super::porta::{
    Acervo, Andamento, Arvore, DestinoDeEnvio, EntradaDoAcervo, EnviosAssinados, ParaBaixar,
};

/// Quantos arquivos sobem ao mesmo tempo.
///
/// ⚠️ **Três, e não "todos"** — o mesmo número do site, pelo mesmo motivo: com a
/// fila inteira em paralelo a banda se divide entre todas, **nenhuma** barra
/// anda, e o operador não tem como saber se travou.
const SIMULTANEOS: usize = 3;

/// O teto do lote enquanto o backend não disser o dele.
const LOTE_PADRAO: usize = 200;

/// Os tamanhos oferecidos — os mesmos do site (`opcoes.ts`).
const TAMANHOS: [(&str, u32); 5] = [
    ("Original", 0),
    ("4K (3840 px)", 3840),
    ("2560 px", 2560),
    ("Full HD (1920 px)", 1920),
    ("1280 px", 1280),
];

pub struct Backup {
    acervo: Arc<dyn Acervo>,
    escolha: Arc<dyn EscolhaDoBackup>,
    sessao: Option<Sessao>,
    /// Onde estou, relativo à raiz. Vazio é a raiz.
    pasta: String,
    entradas: Vec<EntradaDoAcervo>,
    erro: Option<String>,
    lendo: bool,
    arrastando: bool,
    converter: bool,
    conversao: Conversao,
    /// A fila do envio em andamento.
    pecas: Vec<Peca>,
    /// Onde os WebP convertidos ficam até subirem.
    ///
    /// 🚨 **Em disco, e não na memória.** A primeira versão guardava o
    /// convertido (e o original!) num `HashMap<usize, Vec<u8>>` até a vez de
    /// cada um: subindo três por vez, a pasta inteira ficava na RAM. Comeu
    /// 15,77 GB numa pasta de ~1.300 RAW (dono, 2026-09-19).
    ///
    /// 🔑 **`TempDir`, e não uma pasta com o pid no nome.** A segunda versão
    /// usava `temp_dir()/vlb-backup-{pid}` com arquivos `0.webp`, `1.webp` — e
    /// dois envios no mesmo processo escreviam por cima um do outro, com o
    /// primeiro apagando os arquivos do segundo ao terminar. O `TempDir` dá
    /// diretório **único** e o apaga no `drop`: a limpeza deixa de ser um passo
    /// que alguém pode esquecer.
    temporarios: Option<tempfile::TempDir>,
    /// As que ainda não foram despachadas, em ordem.
    pendentes: Vec<usize>,
    /// Onde gravar cada peça já assinada, por caminho.
    assinados: HashMap<String, DestinoDeEnvio>,
    /// Há um lote sendo assinado agora. Sem isto, cada peça que termina
    /// dispararia um pedido de assinatura em paralelo com o que já está indo.
    assinando: bool,
    no_ar: usize,
    maximo_por_pedido: usize,
    convertendo: Option<(usize, usize)>,
    /// A foto aberta na prévia: a imagem, o nome e a posição entre as fotos da
    /// pasta (para as setas).
    vendo: Option<Visualizacao>,
    /// O zip em andamento: o compactador aberto, o que falta e o que já foi.
    zip: Option<EmZip>,
    foco: FocusHandle,
    proximo_id: usize,
    /// ⚠️ **Guardadas, senão são canceladas.** `Task` descartada é `Task`
    /// cancelada — a armadilha nº 3 do `06-UI-ARCHITECTURE`.
    _tarefas: Vec<Task<()>>,
}

impl Backup {
    /// Quem recebe as teclas da tela do acervo.
    pub fn foco(&self) -> FocusHandle {
        self.foco.clone()
    }

    pub fn novo(
        acervo: Arc<dyn Acervo>,
        escolha: Arc<dyn EscolhaDoBackup>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            acervo,
            escolha,
            sessao: None,
            pasta: String::new(),
            entradas: Vec::new(),
            erro: None,
            lendo: false,
            arrastando: false,
            converter: true,
            conversao: Conversao::default(),
            pecas: Vec::new(),
            temporarios: None,
            pendentes: Vec::new(),
            assinados: HashMap::new(),
            assinando: false,
            no_ar: 0,
            maximo_por_pedido: LOTE_PADRAO,
            convertendo: None,
            vendo: None,
            zip: None,
            foco: cx.focus_handle(),
            proximo_id: 0,
            _tarefas: Vec::new(),
        }
    }

    /// A conta entrou (ou trocou): relê a raiz.
    pub fn com_sessao(&mut self, sessao: Sessao, window: &mut Window, cx: &mut Context<Self>) {
        self.sessao = Some(sessao);
        self.abrir(String::new(), window, cx);
    }

    pub fn pasta_aberta(&self) -> &str {
        &self.pasta
    }

    pub fn entradas(&self) -> &[EntradaDoAcervo] {
        &self.entradas
    }

    pub fn pecas(&self) -> &[Peca] {
        &self.pecas
    }

    /// Abre uma pasta.
    pub fn abrir(&mut self, caminho: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.pasta = caminho.clone();
        self.lendo = true;
        cx.notify();

        let (daqui, dali) = channel();
        self.acervo.listar(sessao, caminho, daqui);
        self.drenar(
            dali,
            window,
            cx,
            |tela, resposta: Result<Vec<EntradaDoAcervo>, String>, _window, cx| {
                tela.lendo = false;
                match resposta {
                    Ok(entradas) => {
                        tela.entradas = entradas;
                        tela.erro = None;
                    }
                    // A tela continua aberta mostrando o motivo: um armazenamento
                    // que não é acervo responde 501, e sumir com a tela faria
                    // parecer defeito.
                    Err(erro) => tela.erro = Some(erro),
                }
                cx.notify();
            },
        );
    }

    /// Apaga um arquivo e relê a pasta.
    pub fn apagar(&mut self, caminho: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let (daqui, dali) = channel();
        self.acervo.apagar(sessao, caminho, daqui);
        self.drenar(
            dali,
            window,
            cx,
            |tela, resposta: Result<(), String>, window, cx| {
                if let Err(erro) = resposta {
                    tela.erro = Some(erro);
                }
                let pasta = tela.pasta.clone();
                tela.abrir(pasta, window, cx);
            },
        );
    }

    /// O que foi solto na tela.
    ///
    /// 🔑 **Converter antes de assinar**, como no site: a assinatura vale uma
    /// hora, converter 300 fotos leva minutos, e a conversão muda o **nome** —
    /// que é o que se assina.
    pub fn receber(
        &mut self,
        soltos: Vec<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessao.is_none() || !self.pendentes.is_empty() || self.no_ar > 0 {
            return;
        }
        let arrastados = arquivos_soltos(&soltos);
        if arrastados.is_empty() {
            self.erro = Some("Nada para enviar: a pasta estava vazia ou não pôde ser lida.".into());
            cx.notify();
            return;
        }

        self.pecas.clear();
        self.limpar_os_temporarios();
        self.pendentes.clear();
        self.assinados.clear();
        self.erro = None;
        self.convertendo = Some((0, arrastados.len()));
        cx.notify();

        // A leitura e a conversão saem da thread que desenha: uma pasta de 300
        // RAW congelaria a janela inteira por minutos.
        //
        // 🚨 **No executor do GPUI, e não num `std::thread::spawn`.** A primeira
        // versão abria uma thread do sistema, e o teste da conversão nunca
        // passava: `run_until_parked` e `advance_clock` andam em tempo
        // **virtual**, e uma thread de verdade continua levando os
        // milissegundos dela — a tela ficava esperando um recado que só
        // chegaria depois de o teste terminar. O sintoma era "nada subiu", sem
        // erro nenhum, e valia igual para qualquer teste futuro desta fila.
        let pasta = self.pasta.clone();
        let conversao = self.converter.then_some(self.conversao);
        let temporarios = self.abrir_os_temporarios();
        let (daqui, dali) = channel::<DaPreparacao>();
        let tarefa = cx.background_executor().spawn(async move {
            for (feitos, arrastado) in arrastados.iter().enumerate() {
                let Ok(dados) = std::fs::metadata(&arrastado.origem) else {
                    // Arquivo que sumiu entre a varredura e o envio: some da
                    // fila em vez de derrubá-la.
                    continue;
                };
                let destino = caminho_no_acervo(&pasta, &arrastado.relativo);
                let nome = nome_do(&destino);

                // 🚨 **Só o que vira WebP é lido aqui.** O RAW e o PDF nunca
                // passam pela memória: a fila leva o caminho, e quem lê é a
                // tarefa que envia, em pedaços. Foi a leitura de todos eles,
                // guardada até a vez de cada um, que comeu 15,77 GB.
                let convertido = match (conversao, temporarios.as_ref()) {
                    (Some(c), Some(pasta_temporaria))
                        if super::conversao::eh_conversivel(std::path::Path::new(&nome)) =>
                    {
                        std::fs::read(&arrastado.origem)
                            .ok()
                            .and_then(|bruto| converter_para_webp(&nome, &bruto, c))
                            .and_then(|feito| {
                                // Vai para disco na hora: guardá-lo até a vez
                                // dele é o mesmo erro, com outro nome.
                                let arquivo = pasta_temporaria.join(format!("{feitos}.webp"));
                                let bytes = feito.bytes.len() as u64;
                                std::fs::write(&arquivo, &feito.bytes)
                                    .ok()
                                    .map(|()| (feito.nome, arquivo, bytes))
                            })
                    }
                    _ => None,
                };

                let pasta_do = destino[..destino.rfind('/').map_or(0, |i| i + 1)].to_string();
                let _ = daqui.send(DaPreparacao::Arquivo(Preparada {
                    feitos: feitos + 1,
                    nome,
                    origem: arrastado.origem.clone(),
                    bytes: dados.len(),
                    convertido: convertido.map(|(nome, arquivo, bytes)| {
                        (format!("{pasta_do}{nome}"), nome, arquivo, bytes)
                    }),
                    caminho: destino,
                }));
            }
            // 🔑 **O fim é uma mensagem, e não o canal fechando.** A desconexão
            // encerra o laço que drena sem passar pela tela — e é exatamente
            // aqui que o envio precisa começar.
            let _ = daqui.send(DaPreparacao::Fim);
        });
        // ⚠️ **Guardada, senão é cancelada** — descartar uma `Task` a cancela
        // (a armadilha nº 3 do `06-UI-ARCHITECTURE`), e a fila nasceria vazia.
        self._tarefas.push(tarefa);

        self.drenar_muitos(dali, window, cx, |tela, recado, window, cx| {
            let preparada = match recado {
                DaPreparacao::Arquivo(preparada) => preparada,
                DaPreparacao::Fim => {
                    tela.convertendo = None;
                    tela.bombear(window, cx);
                    return;
                }
            };
            let total = tela.convertendo.map_or(0, |(_, t)| t);
            tela.convertendo = Some((preparada.feitos, total));

            let id = tela.novo_id();
            tela.pecas.push(Peca {
                id,
                caminho: preparada.caminho,
                nome: preparada.nome,
                bytes: preparada.bytes,
                origem: Some(preparada.origem),
                convertida: false,
                situacao: Situacao::Esperando,
                enviados: 0,
                erro: None,
            });
            tela.pendentes.push(id);

            if let Some((caminho, nome, arquivo, bytes)) = preparada.convertido {
                let id = tela.novo_id();
                tela.pecas.push(Peca {
                    id,
                    caminho,
                    nome,
                    bytes,
                    origem: Some(arquivo),
                    convertida: true,
                    situacao: Situacao::Esperando,
                    enviados: 0,
                    erro: None,
                });
                tela.pendentes.push(id);
            }
            cx.notify();
        });
    }

    /// As **fotos** da pasta aberta, na ordem da lista.
    ///
    /// 🔑 É o que as setas da prévia percorrem — e não os arquivos todos: um
    /// PDF no meio faria a seta abrir um quadro vazio.
    fn fotos(&self) -> Vec<&EntradaDoAcervo> {
        self.entradas
            .iter()
            .filter(|e| !e.pasta && eh_visualizavel(&e.nome))
            .collect()
    }

    /// Abre a prévia de uma foto.
    ///
    /// 🔑 **Os bytes vêm direto do R2**, pela URL assinada — não passam pela
    /// API nem pela máquina do Fly.
    pub fn ver(&mut self, caminho: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let posicao = self
            .fotos()
            .iter()
            .position(|f| f.caminho == caminho)
            .unwrap_or(0);

        let (daqui, dali) = channel();
        self.acervo.link(sessao.clone(), caminho, daqui);
        self.drenar(
            dali,
            window,
            cx,
            move |tela, resposta: Result<ParaBaixar, String>, window, cx| match resposta {
                Ok(arquivo) => tela.carregar_a_previa(arquivo, posicao, window, cx),
                Err(erro) => {
                    tela.erro = Some(erro);
                    cx.notify();
                }
            },
        );
    }

    fn carregar_a_previa(
        &mut self,
        arquivo: ParaBaixar,
        posicao: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let (daqui, dali) = channel();
        self.acervo.baixar(sessao, arquivo, daqui);
        self.drenar(
            dali,
            window,
            cx,
            move |tela, resposta: Result<(String, Vec<u8>), String>, _window, cx| {
                match resposta {
                    Ok((caminho, bytes)) => match image::load_from_memory(&bytes) {
                        Ok(imagem) => {
                            tela.vendo = Some(Visualizacao {
                                caminho,
                                imagem: crate::imagem::para_gpui(imagem.clone()),
                                pixels: imagem,
                                posicao,
                                zoom: 1.0,
                                giro: 0.0,
                            });
                        }
                        // Um formato que o `image` não abre (RAW de câmera
                        // nova, PSD) não é falha do acervo: o arquivo está lá e
                        // baixa normalmente.
                        Err(_) => {
                            tela.erro = Some(
                                "Este formato não abre na prévia — baixe o arquivo para vê-lo."
                                    .into(),
                            );
                        }
                    },
                    Err(erro) => tela.erro = Some(erro),
                }
                cx.notify();
            },
        );
    }

    /// A foto vizinha, na ordem da lista.
    pub fn ver_vizinha(&mut self, passo: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(vendo) = self.vendo.as_ref() else {
            return;
        };
        let fotos = self.fotos();
        let destino = vendo.posicao as i32 + passo;
        if destino < 0 || destino as usize >= fotos.len() {
            return;
        }
        let caminho = fotos[destino as usize].caminho.clone();
        self.ver(caminho, window, cx);
    }

    pub fn fechar_a_previa(&mut self, cx: &mut Context<Self>) {
        self.vendo = None;
        cx.notify();
    }

    /// Baixa a pasta aberta compactada.
    ///
    /// 🚨 **O zip é montado aqui** — ver [`super::compactar`]. A API diz quais
    /// arquivos existem e assina; os bytes vêm direto do R2 e vão direto para o
    /// arquivo escolhido.
    pub fn baixar_pasta(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        if self.zip.is_some() {
            return;
        }
        let pasta = self.pasta.clone();
        let (daqui, dali) = channel();
        self.acervo.arvore(sessao, pasta.clone(), daqui);
        self.drenar(
            dali,
            window,
            cx,
            move |tela, resposta: Result<Arvore, String>, window, cx| match resposta {
                Ok(arvore) if arvore.arquivos.is_empty() => {
                    tela.erro = Some("Pasta vazia: não há o que compactar aqui.".into());
                    cx.notify();
                }
                Ok(arvore) => tela.pedir_o_destino(arvore, window, cx),
                Err(erro) => {
                    tela.erro = Some(erro);
                    cx.notify();
                }
            },
        );
    }

    fn pedir_o_destino(&mut self, arvore: Arvore, window: &mut Window, cx: &mut Context<Self>) {
        let nome = if arvore.nome_do_zip.is_empty() {
            nome_do_zip(&self.pasta)
        } else {
            arvore.nome_do_zip.clone()
        };
        let (daqui, dali) = channel();
        self.escolha.escolher_destino_do_zip(nome, daqui);
        self.drenar(
            dali,
            window,
            cx,
            move |tela, destinos: Vec<std::path::PathBuf>, window, cx| {
                // Fechar o diálogo é desistência, e não erro.
                let Some(destino) = destinos.first().cloned() else {
                    return;
                };
                match Compactador::no_arquivo(&destino) {
                    Ok(compactador) => {
                        tela.zip = Some(EmZip {
                            compactador,
                            destino,
                            faltam: arvore.arquivos.clone(),
                            feitos: 0,
                            total: arvore.arquivos.len(),
                        });
                        tela.proximo_do_zip(window, cx);
                    }
                    Err(erro) => {
                        tela.erro = Some(format!("não deu para criar o arquivo: {erro}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    /// Baixa o próximo arquivo do zip.
    ///
    /// ⚠️ **Um de cada vez.** Paralelizar aqui não adianta: o gargalo é o disco
    /// e a escrita sequencial do zip, e baixar três de uma vez só faria três
    /// arquivos esperarem a vez de serem escritos — com os três na memória.
    fn proximo_do_zip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(zip) = self.zip.as_mut() else {
            return;
        };
        let Some(arquivo) = zip.faltam.first().cloned() else {
            // Acabou: fechar é o que escreve o índice central — sem isso o zip
            // não abre.
            let terminado = self.zip.take().expect("acabou de ser lido");
            let destino = terminado.destino.clone();
            match terminado.compactador.fechar() {
                Ok(_) => {
                    self.erro = None;
                }
                Err(erro) => {
                    self.erro = Some(format!("o zip não pôde ser fechado: {erro}"));
                    let _ = std::fs::remove_file(&destino);
                }
            }
            cx.notify();
            return;
        };

        let (daqui, dali) = channel();
        self.acervo.baixar(sessao, arquivo, daqui);
        self.drenar(
            dali,
            window,
            cx,
            |tela, resposta: Result<(String, Vec<u8>), String>, window, cx| {
                let Some(zip) = tela.zip.as_mut() else {
                    return;
                };
                match resposta {
                    Ok((caminho, bytes)) => {
                        if let Err(erro) = zip.compactador.por(&caminho, &bytes) {
                            tela.erro = Some(format!("{caminho}: {erro}"));
                            let abortado = tela.zip.take();
                            if let Some(abortado) = abortado {
                                let _ = std::fs::remove_file(&abortado.destino);
                            }
                            cx.notify();
                            return;
                        }
                        zip.feitos += 1;
                        zip.faltam.remove(0);
                    }
                    Err(erro) => {
                        // 🚨 **Um arquivo que falha aborta o zip inteiro**, ao
                        // contrário do envio. Um zip com 299 de 300 fotos é um
                        // backup que parece completo e não é — e ninguém
                        // confere o número ao abrir.
                        tela.erro = Some(format!("o download parou: {erro}"));
                        if let Some(abortado) = tela.zip.take() {
                            let _ = std::fs::remove_file(&abortado.destino);
                        }
                        cx.notify();
                        return;
                    }
                }
                cx.notify();
                tela.proximo_do_zip(window, cx);
            },
        );
    }

    /// "Escolher pasta" e "Arquivos" — o caminho de quem não arrasta.
    ///
    /// 🔑 *"Tinha que ter opção sem arrastar e soltar pra fazer backup"* (dono,
    /// 2026-09-19). O que a janela do sistema devolve entra pelo **mesmo**
    /// [`Self::receber`] do arrasto: um caminho só, e nenhuma chance de os dois
    /// gestos divergirem no dia em que a conversão mudar.
    pub fn escolher(&mut self, pasta: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (daqui, dali) = channel();
        if pasta {
            self.escolha.escolher_pasta(daqui);
        } else {
            self.escolha.escolher_arquivos(daqui);
        }
        self.drenar(
            dali,
            window,
            cx,
            |tela, escolhidos: Vec<std::path::PathBuf>, window, cx| {
                // Lista vazia é desistência, e não erro: fechar a janela do
                // sistema não pode deixar um aviso vermelho na tela.
                if !escolhidos.is_empty() {
                    tela.receber(escolhidos, window, cx);
                }
            },
        );
    }

    /// Abre a pasta onde os WebP convertidos esperam a vez.
    ///
    /// 🔑 **Uma por envio, apagada no fim.** Fica em `std::env::temp_dir` com o
    /// pid no nome: duas janelas do app não disputam a mesma, e o que sobrar de
    /// um encerramento abrupto o sistema recolhe.
    fn abrir_os_temporarios(&mut self) -> Option<std::path::PathBuf> {
        // Sem lugar para escrever, o envio segue **sem** converter: subir o
        // original é melhor que não subir nada.
        let pasta = tempfile::Builder::new()
            .prefix("vlb-backup-")
            .tempdir()
            .ok()?;
        let caminho = pasta.path().to_path_buf();
        self.temporarios = Some(pasta);
        Some(caminho)
    }

    /// Larga a pasta temporária — o `drop` do `TempDir` a apaga.
    fn limpar_os_temporarios(&mut self) {
        self.temporarios = None;
    }

    fn novo_id(&mut self) -> usize {
        self.proximo_id += 1;
        self.proximo_id
    }

    /// Mantém [`SIMULTANEOS`] no ar, assinando quando acabam os assinados.
    ///
    /// 🔑 **Assinar e despachar são coisas separadas.** Assina-se um lote
    /// grande de uma vez (a assinatura vale uma hora), e dele saem três por
    /// vez. Assinar a cada três seria um pedido de rede por trio — e assinar
    /// tudo e despachar tudo divide a banda entre todas as barras, que é o que
    /// faz nenhuma andar.
    fn bombear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while self.no_ar < SIMULTANEOS {
            let Some(posicao) = self.pendentes.iter().position(|id| {
                self.pecas
                    .iter()
                    .find(|p| p.id == *id)
                    .is_some_and(|p| self.assinados.contains_key(&p.caminho))
            }) else {
                break;
            };
            let id = self.pendentes.remove(posicao);
            self.despachar(id, window, cx);
        }
        // Ainda há o que subir e nada assinado: pede o próximo lote.
        if self.no_ar < SIMULTANEOS && !self.pendentes.is_empty() && !self.assinando {
            self.assinar(window, cx);
        }
        cx.notify();
    }

    /// Pede a assinatura do próximo lote.
    fn assinar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let lote: Vec<usize> = self
            .pendentes
            .iter()
            .filter(|id| {
                self.pecas
                    .iter()
                    .find(|p| p.id == **id)
                    .is_some_and(|p| !self.assinados.contains_key(&p.caminho))
            })
            .take(self.maximo_por_pedido)
            .copied()
            .collect();
        if lote.is_empty() {
            return;
        }
        let caminhos: Vec<String> = lote
            .iter()
            .filter_map(|id| self.pecas.iter().find(|p| p.id == *id))
            .map(|p| p.caminho.clone())
            .collect();

        self.assinando = true;
        let (daqui, dali) = channel();
        self.acervo.assinar(sessao, caminhos, daqui);
        self.drenar(
            dali,
            window,
            cx,
            move |tela, resposta: Result<EnviosAssinados, String>, window, cx| {
                tela.assinando = false;
                match resposta {
                    Ok(assinados) => {
                        tela.maximo_por_pedido = assinados.maximo_por_pedido.max(1);
                        for envio in assinados.envios {
                            tela.assinados.insert(envio.caminho.clone(), envio);
                        }
                        tela.bombear(window, cx);
                    }
                    Err(erro) => {
                        // 🚨 **O lote inteiro cai junto**: o backend recusa o
                        // lote, e não uma peça. Marcar só uma faria a fila
                        // parecer parcial, e o resto ficaria pendurado para
                        // sempre esperando uma assinatura que não vem.
                        for id in &lote {
                            if let Some(peca) = tela.pecas.iter_mut().find(|p| p.id == *id) {
                                peca.situacao = Situacao::Falhou;
                                peca.erro = Some(erro.clone());
                            }
                        }
                        tela.pendentes.retain(|id| !lote.contains(id));
                        cx.notify();
                    }
                }
            },
        );
    }

    /// Manda **uma** peça já assinada.
    fn despachar(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(peca) = self.pecas.iter_mut().find(|p| p.id == id) else {
            return;
        };
        let Some(destino) = self.assinados.remove(&peca.caminho) else {
            return;
        };
        let Some(origem) = peca.origem.clone() else {
            return;
        };
        peca.situacao = Situacao::Enviando;
        let tipo = tipo_do_arquivo(&peca.nome);
        self.no_ar += 1;

        let (daqui, dali) = channel();
        self.acervo.enviar(sessao, id, destino, origem, tipo, daqui);
        self.drenar_muitos(
            dali,
            window,
            cx,
            |tela, andamento: Andamento, window, cx| {
                tela.aplicar(andamento, window, cx);
            },
        );
    }

    fn aplicar(&mut self, andamento: Andamento, window: &mut Window, cx: &mut Context<Self>) {
        match andamento {
            Andamento::Subiu { id, enviados } => {
                if let Some(peca) = self.pecas.iter_mut().find(|p| p.id == id) {
                    peca.enviados = enviados;
                }
            }
            Andamento::Terminou { id } => {
                if let Some(peca) = self.pecas.iter_mut().find(|p| p.id == id) {
                    peca.situacao = Situacao::Pronta;
                    peca.enviados = peca.bytes;
                }
                self.no_ar = self.no_ar.saturating_sub(1);
                self.bombear(window, cx);
                self.varrer_se_acabou();
            }
            Andamento::Falhou { id, erro } => {
                if let Some(peca) = self.pecas.iter_mut().find(|p| p.id == id) {
                    peca.situacao = Situacao::Falhou;
                    peca.erro = Some(erro);
                }
                self.no_ar = self.no_ar.saturating_sub(1);
                // ⚠️ **A falha de uma não interrompe a fila** — a mesma regra
                // da exportação em lote. Parar tudo numa pasta de 300 fotos
                // obrigaria a recomeçar por causa de uma.
                self.bombear(window, cx);
                self.varrer_se_acabou();
            }
        }
        cx.notify();
    }

    /// Apaga os convertidos assim que a fila acaba.
    ///
    /// ⚠️ **Só quando acaba de verdade**: apagar a pasta com peça pendente
    /// deixaria o envio seguinte procurando um arquivo que não existe mais.
    fn varrer_se_acabou(&mut self) {
        if self.no_ar == 0 && self.pendentes.is_empty() && self.convertendo.is_none() {
            self.limpar_os_temporarios();
        }
    }

    /// Uma resposta só.
    fn drenar<T: Send + 'static>(
        &mut self,
        dali: Receiver<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
        aplicar: impl Fn(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) {
        self.drenar_muitos(dali, window, cx, aplicar);
    }

    /// Drena o canal num laço **que acaba** — a regra da casa: um laço eterno
    /// acordaria a cada passo pelo resto da sessão.
    ///
    /// 🔑 **`update_in`, e não `update`**: quem recebe a resposta quase sempre
    /// precisa reabrir a pasta ou despachar o próximo lote, e as duas coisas
    /// pedem `Window`. Sem ele, cada resposta teria de adiar a continuação para
    /// um quadro depois só para reencontrar a janela.
    fn drenar_muitos<T: Send + 'static>(
        &mut self,
        dali: Receiver<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
        aplicar: impl Fn(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let tarefa = cx.spawn_in(window, async move |esta, cx| loop {
            match dali.try_recv() {
                Ok(recado) => {
                    if esta
                        .update_in(cx, |tela, window, cx| aplicar(tela, recado, window, cx))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        });
        self._tarefas.push(tarefa);
    }
}

/// A foto aberta na prévia.
struct Visualizacao {
    caminho: String,
    /// O que se desenha.
    imagem: Arc<RenderImage>,
    /// 🔑 **Os pixels guardados, para poder girar.** O `Img` do GPUI não gira
    /// (a `Transformation` é só do `svg`), então girar é rodar os pixels e
    /// subir outra textura. Guardar o decodificado custa a memória de **uma**
    /// foto e evita baixá-la de novo a cada quarto de volta.
    pixels: image::DynamicImage,
    /// A posição entre as **fotos** da pasta — é o que as setas percorrem.
    posicao: usize,
    zoom: f32,
    giro: f32,
}

/// Um zip sendo montado.
struct EmZip {
    compactador: Compactador<std::fs::File>,
    destino: std::path::PathBuf,
    /// O que ainda falta baixar, na ordem.
    faltam: Vec<ParaBaixar>,
    feitos: usize,
    total: usize,
}

/// O que a thread de leitura manda para a tela.
enum DaPreparacao {
    Arquivo(Preparada),
    /// Acabou de ler tudo — é aqui que o envio começa.
    Fim,
}

/// Um arquivo pronto para entrar na fila — **sem o conteúdo dele**.
struct Preparada {
    feitos: usize,
    /// No acervo.
    caminho: String,
    nome: String,
    /// No disco desta máquina. É de onde os bytes saem, na hora de subir.
    origem: std::path::PathBuf,
    bytes: u64,
    /// O irmão WebP: `(caminho no acervo, nome, arquivo temporário, bytes)`.
    convertido: Option<(String, String, std::path::PathBuf, u64)>,
}

fn nome_do(caminho: &str) -> String {
    caminho.rsplit('/').next().unwrap_or(caminho).to_string()
}

impl Render for Backup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let apagado = tema.muted_foreground;
        let andamento = andamento_de(&self.pecas);

        v_flex()
            .id("backup")
            .key_context("Backup")
            .track_focus(&self.foco)
            .relative()
            .size_full()
            .gap(px(16.))
            .p(px(24.))
            // 🔑 As mesmas teclas do site: `Esc` fecha a prévia, `←` `→`
            // percorrem as fotos da pasta, `+` `-` dão zoom e `R` gira.
            .on_key_down(cx.listener(|tela, evento: &KeyDownEvent, window, cx| {
                tela.tecla(evento, window, cx);
            }))
            .child(estilo::cabecalho_da_pagina(
                "Backup de arquivos",
                Some(
                    div()
                        .child(
                            "Arraste pastas e arquivos para guardá-los no Cloudflare R2, \
                             com a mesma árvore que têm aqui.",
                        )
                        .into_any_element(),
                ),
                None,
                Some(self.acoes(cx).into_any_element()),
                cx,
            ))
            .child(self.barra(cx))
            .when_some(self.erro.clone(), |tela, erro| {
                tela.child(estilo::aviso(erro, true, cx))
            })
            .child(self.lona(andamento, cx))
            .child(self.lista(apagado, cx))
            .children(self.previa(window, cx))
    }
}

impl Backup {
    fn acoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Enquanto a fila corre, escolher mais arquivos seria trocar a fila que
        // está na tela por outra — e a primeira sumiria sem dizer o que subiu.
        let ocupado = self.convertendo.is_some() || self.no_ar > 0 || !self.pendentes.is_empty();
        let compactando = self.zip.is_some();

        h_flex()
            .gap(px(8.))
            .child(estilo::desligado(
                estilo::botao_contorno("backup-atualizar", cx)
                    .child(Icon::new(Icone::RefreshCw).size(px(16.)))
                    .child("Atualizar")
                    .on_click(cx.listener(|tela, _ev, window, cx| {
                        let pasta = tela.pasta.clone();
                        tela.abrir(pasta, window, cx);
                    })),
                self.lendo,
            ))
            // 📦 Baixar a pasta compactada (dono, 2026-09-19). O zip é montado
            // aqui, com os bytes vindo direto do R2 — ver `super::compactar`.
            .child(estilo::desligado(
                estilo::botao_contorno("backup-baixar-pasta", cx)
                    .debug_selector(|| "backup-baixar-pasta".into())
                    .child(Icon::new(Icone::Download).size(px(16.)))
                    .child(match &self.zip {
                        Some(zip) => SharedString::from(format!(
                            "Compactando {} de {}",
                            zip.feitos, zip.total
                        )),
                        None => SharedString::from("Baixar pasta"),
                    })
                    .when(!compactando, |b| {
                        b.on_click(
                            cx.listener(|tela, _ev, window, cx| tela.baixar_pasta(window, cx)),
                        )
                    }),
                compactando,
            ))
            // 🔑 *"Tinha que ter opção sem arrastar e soltar"* (dono,
            // 2026-09-19) — os mesmos dois botões do site.
            .child(estilo::desligado(
                estilo::botao_contorno("backup-arquivos", cx)
                    .debug_selector(|| "backup-arquivos".into())
                    .child(Icon::new(Icone::Upload).size(px(16.)))
                    .child("Arquivos")
                    .when(!ocupado, |b| {
                        b.on_click(
                            cx.listener(|tela, _ev, window, cx| tela.escolher(false, window, cx)),
                        )
                    }),
                ocupado,
            ))
            .child(estilo::desligado(
                estilo::botao_primario("backup-escolher-pasta", cx)
                    .debug_selector(|| "backup-escolher-pasta".into())
                    .child(Icon::new(Icone::FolderInput).size(px(16.)))
                    .child("Escolher pasta")
                    .when(!ocupado, |b| {
                        b.on_click(
                            cx.listener(|tela, _ev, window, cx| tela.escolher(true, window, cx)),
                        )
                    }),
                ocupado,
            ))
    }

    /// A trilha de onde estou, e as opções de conversão.
    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let partes: Vec<String> = if self.pasta.is_empty() {
            Vec::new()
        } else {
            self.pasta.split('/').map(str::to_string).collect()
        };
        let apagado = cx.theme().muted_foreground;

        h_flex()
            .w_full()
            .flex_wrap()
            .gap(px(16.))
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .child(
                        estilo::botao_fantasma("backup-raiz", cx)
                            .child(Icon::new(Icone::Cloud).size(px(14.)))
                            .child("Acervo")
                            .on_click(cx.listener(|tela, _ev, window, cx| {
                                tela.abrir(String::new(), window, cx);
                            })),
                    )
                    .children(partes.iter().enumerate().map(|(i, parte)| {
                        let ate = partes[..=i].join("/");
                        h_flex()
                            .gap(px(2.))
                            .child(div().text_color(apagado).child("›"))
                            .child(
                                estilo::botao_fantasma(
                                    SharedString::from(format!("trilha-{i}")),
                                    cx,
                                )
                                .child(SharedString::from(parte.clone()))
                                .on_click(cx.listener(
                                    move |tela, _ev, window, cx| {
                                        tela.abrir(ate.clone(), window, cx);
                                    },
                                )),
                            )
                            .into_any_element()
                    })),
            )
            .child(self.opcoes(cx))
    }

    fn opcoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (primaria, borda) = (tema.primary, tema.border);
        let ligado = self.converter;

        h_flex()
            .gap(px(12.))
            .child(
                h_flex()
                    .id("backup-converter")
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.converter = !tela.converter;
                        cx.notify();
                    }))
                    // O interruptor do site, com as medidas dele.
                    .child(
                        div()
                            .w(px(36.))
                            .h(px(20.))
                            .rounded(px(10.))
                            .bg(if ligado { primaria } else { borda })
                            .p(px(2.))
                            .child(
                                div()
                                    .size(px(16.))
                                    .rounded(px(8.))
                                    .bg(gpui_kit::white())
                                    .ml(if ligado { px(16.) } else { px(0.) }),
                            ),
                    )
                    .child(div().text_sm().child("Gerar WebP ao lado")),
            )
            .children(TAMANHOS.map(|(rotulo, maior_lado)| {
                let escolhido = self.conversao.maior_lado == maior_lado;
                estilo::desligado(
                    if escolhido {
                        estilo::botao_primario(SharedString::from(format!("tam-{maior_lado}")), cx)
                    } else {
                        estilo::botao_contorno(SharedString::from(format!("tam-{maior_lado}")), cx)
                    }
                    .child(SharedString::from(rotulo))
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.conversao.maior_lado = maior_lado;
                        cx.notify();
                    })),
                    !ligado,
                )
                .into_any_element()
            }))
    }

    /// A lona de arrastar — a área inteira, e não um retângulo pequeno: quem
    /// arrasta uma pasta de 3 GB não mira.
    fn lona(&self, andamento: super::fila::Andamento, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (primaria, borda, apagado, perigo) = (
            tema.primary,
            tema.border,
            tema.muted_foreground,
            tema.danger,
        );
        let pasta = if self.pasta.is_empty() {
            "Acervo".to_string()
        } else {
            self.pasta.clone()
        };

        v_flex()
            .id("backup-lona")
            .w_full()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(10.))
            .border_2()
            .border_dashed()
            .border_color(if self.arrastando { primaria } else { borda })
            .on_drag_move(
                cx.listener(|tela, _ev: &gpui_kit::DragMoveEvent<()>, _window, cx| {
                    if !tela.arrastando {
                        tela.arrastando = true;
                        cx.notify();
                    }
                }),
            )
            .on_drop(
                cx.listener(|tela, arrastados: &gpui_kit::ExternalPaths, window, cx| {
                    tela.arrastando = false;
                    tela.receber(arrastados.paths().to_vec(), window, cx);
                }),
            )
            .when(
                self.pecas.is_empty() && self.convertendo.is_none(),
                |lona| {
                    lona.child(
                        v_flex()
                            .items_center()
                            .gap(px(6.))
                            .py(px(16.))
                            .text_sm()
                            .text_color(apagado)
                            .child(Icon::new(Icone::FolderOpen).size(px(28.)))
                            .child(SharedString::from(format!(
                                "Solte aqui pastas ou arquivos — ou use \"Escolher pasta\" acima. \
                             A árvore vai para o R2 dentro de {pasta}."
                            ))),
                    )
                },
            )
            .when_some(self.convertendo, |lona, (feitos, total)| {
                lona.child(
                    v_flex()
                        .gap(px(6.))
                        .child(div().text_sm().child(SharedString::from(format!(
                            "Lendo e convertendo… {feitos} de {total}"
                        ))))
                        .child(
                            Progress::new("progresso-do-acervo")
                                .value(if total == 0 {
                                    0.
                                } else {
                                    feitos as f32 / total as f32 * 100.
                                })
                                .h(px(6.)),
                        ),
                )
            })
            .when(!self.pecas.is_empty(), |lona| {
                lona.child(
                    v_flex()
                        .gap(px(8.))
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .text_sm()
                                .child(SharedString::from(format!(
                                    "{} — {} de {} · {} de {}",
                                    if andamento.terminado {
                                        "Envio concluído"
                                    } else {
                                        "Enviando"
                                    },
                                    andamento.prontas,
                                    andamento.total,
                                    em_tamanho(andamento.bytes_enviados),
                                    em_tamanho(andamento.bytes_totais),
                                )))
                                .child(SharedString::from(format!("{}%", andamento.porcento))),
                        )
                        .child(
                            Progress::new("progresso-do-backup")
                                .value(andamento.porcento as f32)
                                .h(px(6.)),
                        )
                        .children(self.pecas.iter().map(|peca| {
                            h_flex()
                                .w_full()
                                .gap(px(8.))
                                .text_sm()
                                .when(peca.convertida, |l| l.pl(px(16.)).text_color(apagado))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .child(SharedString::from(peca.nome.clone())),
                                )
                                .when_some(peca.erro.clone(), |l, erro| {
                                    l.child(
                                        div().text_color(perigo).child(SharedString::from(erro)),
                                    )
                                })
                                .when(peca.erro.is_none(), |l| {
                                    l.child(
                                        Progress::new(("progresso-da-peca", peca.id))
                                            .value(if peca.bytes == 0 {
                                                0.
                                            } else {
                                                peca.enviados as f32 / peca.bytes as f32 * 100.
                                            })
                                            .h(px(3.))
                                            .w(px(120.)),
                                    )
                                })
                                .child(
                                    div()
                                        .text_color(apagado)
                                        .child(SharedString::from(em_tamanho(peca.bytes))),
                                )
                                .into_any_element()
                        })),
                )
            })
    }

    /// As teclas da prévia — as mesmas do site.
    fn tecla(&mut self, evento: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.vendo.is_none() {
            return;
        }
        match evento.keystroke.key.as_str() {
            "escape" => self.fechar_a_previa(cx),
            "left" => self.ver_vizinha(-1, window, cx),
            "right" => self.ver_vizinha(1, window, cx),
            "+" | "=" => self.mexer_no_zoom(1.25, cx),
            "-" => self.mexer_no_zoom(1.0 / 1.25, cx),
            "r" => self.girar(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// Gira um quarto de volta — nos pixels, porque o `Img` não gira.
    fn girar(&mut self, cx: &mut Context<Self>) {
        let Some(vendo) = self.vendo.as_mut() else {
            return;
        };
        vendo.pixels = vendo.pixels.rotate90();
        vendo.imagem = crate::imagem::para_gpui(vendo.pixels.clone());
        vendo.giro = (vendo.giro + 90.0) % 360.0;
        cx.notify();
    }

    fn mexer_no_zoom(&mut self, fator: f32, cx: &mut Context<Self>) {
        if let Some(vendo) = self.vendo.as_mut() {
            vendo.zoom = (vendo.zoom * fator).clamp(1.0, 8.0);
            cx.notify();
        }
    }

    /// A prévia por cima de tudo.
    ///
    /// ⚠️ **`max_w`/`max_h`, e nunca `size_full` com `Contain`** — a armadilha
    /// que já cortou a foto quatro vezes (`crate::imagem`, `a_moldura_manda`).
    fn previa(&self, window: &Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let vendo = self.vendo.as_ref()?;
        let nome = vendo
            .caminho
            .rsplit('/')
            .next()
            .unwrap_or(&vendo.caminho)
            .to_string();
        let fotos = self.fotos().len();
        let (tem_antes, tem_depois) = (vendo.posicao > 0, vendo.posicao + 1 < fotos);
        let imagem = vendo.imagem.clone();
        let zoom = vendo.zoom;
        // 🚨 **O tamanho é conta, e não `size_full` com `Contain`** — a
        // armadilha que já cortou a foto quatro vezes (`crate::imagem`,
        // `a_moldura_manda`). `cabe_em` é a mesma conta da tela do cliente; o
        // zoom multiplica os dois lados.
        let moldura = window.viewport_size();
        let cabe = crate::imagem::cabe_em(
            gpui_kit::size(moldura.width, moldura.height - px(48.)),
            imagem.size(0),
        );

        Some(
            v_flex()
                .id("backup-previa")
                .absolute()
                .inset_0()
                .bg(gpui_kit::black().opacity(0.9))
                .on_click(cx.listener(|tela, _ev, _window, cx| tela.fechar_a_previa(cx)))
                .child(
                    h_flex()
                        .w_full()
                        .flex_none()
                        .items_center()
                        .gap(px(8.))
                        .px(px(16.))
                        .py(px(8.))
                        .bg(gpui_kit::black().opacity(0.6))
                        .text_color(gpui_kit::white())
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_sm()
                                .child(SharedString::from(nome)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .opacity(0.7)
                                .child(SharedString::from(format!("{}%", (zoom * 100.) as i32))),
                        )
                        .child(
                            estilo::botao_fantasma("previa-menos", cx)
                                .child(Icon::new(Icone::ZoomOut).size(px(16.)))
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.mexer_no_zoom(1.0 / 1.25, cx);
                                })),
                        )
                        .child(
                            estilo::botao_fantasma("previa-mais", cx)
                                .child(Icon::new(Icone::ZoomIn).size(px(16.)))
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.mexer_no_zoom(1.25, cx);
                                })),
                        )
                        .child(
                            estilo::botao_fantasma("previa-girar", cx)
                                .child(Icon::new(Icone::RotateCw).size(px(16.)))
                                .on_click(cx.listener(|tela, _ev, _window, cx| tela.girar(cx))),
                        )
                        .child(
                            estilo::botao_fantasma("previa-fechar", cx)
                                .child(Icon::new(Icone::X).size(px(16.)))
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.fechar_a_previa(cx)),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h(px(0.))
                        .w_full()
                        .items_center()
                        .justify_center()
                        // Com zoom a foto passa da moldura; quem corta é isto.
                        .overflow_hidden()
                        .when(tem_antes, |faixa| {
                            faixa.child(
                                estilo::botao_fantasma("previa-anterior", cx)
                                    .absolute()
                                    .left(px(8.))
                                    .child(Icon::new(Icone::ChevronLeft).size(px(28.)))
                                    .on_click(cx.listener(|tela, _ev, window, cx| {
                                        tela.ver_vizinha(-1, window, cx);
                                    })),
                            )
                        })
                        .child(
                            div()
                                .w(cabe.width * zoom)
                                .h(cabe.height * zoom)
                                .child(img(imagem).size_full()),
                        )
                        .when(tem_depois, |faixa| {
                            faixa.child(
                                estilo::botao_fantasma("previa-proxima", cx)
                                    .absolute()
                                    .right(px(8.))
                                    .child(Icon::new(Icone::ChevronRight).size(px(28.)))
                                    .on_click(cx.listener(|tela, _ev, window, cx| {
                                        tela.ver_vizinha(1, window, cx);
                                    })),
                            )
                        }),
                ),
        )
    }

    fn lista(&self, apagado: gpui_kit::Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        let borda = cx.theme().border;
        v_flex()
            .id("backup-lista")
            .w_full()
            .flex_1()
            .min_h(px(0.))
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .overflow_y_scroll()
            .when(self.entradas.is_empty(), |lista| {
                lista.child(
                    div()
                        .p(px(24.))
                        .text_sm()
                        .text_color(apagado)
                        .child("Esta pasta está vazia."),
                )
            })
            .children(self.entradas.iter().map(|entrada| {
                let caminho = entrada.caminho.clone();
                let pasta = entrada.pasta;
                h_flex()
                    .id(SharedString::from(format!("item-{}", entrada.caminho)))
                    .w_full()
                    .gap(px(12.))
                    .px(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(borda)
                    .text_sm()
                    .when(pasta, |linha| {
                        let destino = caminho.clone();
                        linha.cursor_pointer().on_click(cx.listener(
                            move |tela, _ev, window, cx| {
                                tela.abrir(destino.clone(), window, cx);
                            },
                        ))
                    })
                    // Clicar na foto abre a prévia — como no site.
                    .when(!pasta && eh_visualizavel(&entrada.nome), |linha| {
                        let destino = caminho.clone();
                        linha.cursor_pointer().on_click(cx.listener(
                            move |tela, _ev, window, cx| {
                                tela.ver(destino.clone(), window, cx);
                            },
                        ))
                    })
                    .child(
                        Icon::new(if pasta {
                            Icone::FolderOpen
                        } else {
                            Icone::File
                        })
                        .size(px(16.)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(SharedString::from(entrada.nome.clone())),
                    )
                    .child(div().text_color(apagado).child(SharedString::from(
                        entrada.bytes.map(em_tamanho).unwrap_or_default(),
                    )))
                    .when(!pasta, |linha| {
                        let alvo = caminho.clone();
                        linha.child(
                            estilo::botao_fantasma(
                                SharedString::from(format!("apagar-{caminho}")),
                                cx,
                            )
                            .child(Icon::new(Icone::Trash2).size(px(14.)))
                            .on_click(cx.listener(
                                move |tela, _ev, window, cx| {
                                    tela.apagar(alvo.clone(), window, cx);
                                },
                            )),
                        )
                    })
                    .into_any_element()
            }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::backup::escolha::mentira::EscolhaDeMentira;
    use crate::backup::porta::mentira::AcervoDeArquivosDeMentira;
    use gpui_kit::TestAppContext;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            // Prazos folgados: o que estes testes exercem é a fila, e não a
            // renovação — que tem teste próprio em `pos_venda/http.rs`.
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    /// Uma pasta com `quantos` arquivos que **não** são imagem, para a fila ter
    /// uma peça por arquivo e a conta ficar legível.
    fn pasta_com(quantos: usize) -> tempfile::TempDir {
        let raiz = tempfile::tempdir().expect("pasta temporária");
        let ensaio = raiz.path().join("ensaio-silva");
        std::fs::create_dir_all(&ensaio).expect("subpasta");
        for i in 0..quantos {
            std::fs::write(ensaio.join(format!("DSC_{i:02}.NEF")), vec![b'r'; 100 + i])
                .expect("arquivo");
        }
        raiz
    }

    /// Deixa a fila correr até o fim.
    ///
    /// 🚨 **`run_until_parked` sozinho não basta.** O laço que drena os canais
    /// espera num `timer` quando não há recado — é o padrão da casa
    /// (`exportacao/tela.rs`) — e no relógio de teste esse tempo não passa
    /// sozinho. Quando a leitura e a conversão demoram (uma foto de verdade), o
    /// primeiro `try_recv` vem vazio, o laço dorme, e a tela fica parada sem
    /// nada ter dado errado. Foi o que este teste acusou.
    fn assentar(visual: &mut gpui_kit::VisualTestContext) {
        for _ in 0..60 {
            visual.run_until_parked();
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(60));
        }
        visual.run_until_parked();
    }

    fn tela(
        cx: &mut TestAppContext,
        acervo: Arc<AcervoDeArquivosDeMentira>,
    ) -> (gpui_kit::WindowHandle<Backup>, gpui_kit::VisualTestContext) {
        com_escolha(cx, acervo, Arc::new(EscolhaDeMentira::default()))
    }

    fn com_escolha(
        cx: &mut TestAppContext,
        acervo: Arc<AcervoDeArquivosDeMentira>,
        escolha: Arc<EscolhaDeMentira>,
    ) -> (gpui_kit::WindowHandle<Backup>, gpui_kit::VisualTestContext) {
        // O `gpui-component` guarda o tema num estado global; sem ele, o
        // primeiro `cx.theme()` do `render` derruba o teste com "no state of
        // type Theme exists" — um erro que não fala de tema nenhum.
        cx.update(gpui_kit::init);
        let janela = cx.add_window(|_window, cx| Backup::novo(acervo, escolha, cx));
        let visual = gpui_kit::VisualTestContext::from_window(janela.into(), cx);
        janela
            .update(cx, |tela, _window, _cx| {
                tela.sessao = Some(sessao());
            })
            .expect("a janela abriu");
        (janela, visual)
    }

    /// 🚨 **O teste do bombeamento.** Só três sobem por vez, e as outras têm de
    /// sair conforme abrem vagas — a primeira versão desta tela despachava os
    /// três primeiros e parava, e a fila ficava pendurada para sempre sem erro
    /// nenhum. `SIMULTANEOS` sem uso foi o que denunciou.
    #[gpui_kit::test]
    async fn a_fila_inteira_sobe_mesmo_passando_do_limite(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());
        let pasta = pasta_com(10);

        janela
            .update(cx, |tela, window, cx| {
                tela.receber(vec![pasta.path().join("ensaio-silva")], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        let enviados = acervo.enviados.lock().unwrap().len();
        assert_eq!(enviados, 10, "as dez subiram, e não só as três do limite");

        janela
            .update(cx, |tela, _window, _cx| {
                let andamento = andamento_de(tela.pecas());
                assert_eq!(andamento.prontas, 10);
                assert_eq!(andamento.porcento, 100);
                assert!(andamento.terminado);
            })
            .expect("a janela abriu");
    }

    /// 🔑 A árvore arrastada é preservada, e a pasta aberta entra na frente.
    #[gpui_kit::test]
    async fn o_caminho_enviado_guarda_a_arvore_e_a_pasta_aberta(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());
        let pasta = pasta_com(1);

        janela
            .update(cx, |tela, window, cx| {
                tela.pasta = "2026".into();
                tela.receber(vec![pasta.path().join("ensaio-silva")], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        let enviados = acervo.enviados.lock().unwrap().clone();
        assert_eq!(enviados[0].0, "2026/ensaio-silva/DSC_00.NEF");
    }

    /// ⚠️ **A falha de uma não interrompe a fila** — a mesma regra da
    /// exportação em lote.
    #[gpui_kit::test]
    async fn a_falha_de_uma_nao_derruba_as_outras(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        *acervo.falham.lock().unwrap() = 2;
        let (janela, mut visual) = tela(cx, acervo.clone());
        let pasta = pasta_com(6);

        janela
            .update(cx, |tela, window, cx| {
                tela.receber(vec![pasta.path().join("ensaio-silva")], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                let andamento = andamento_de(tela.pecas());
                assert_eq!(andamento.com_erro, 2);
                assert_eq!(andamento.prontas, 4);
                assert!(andamento.terminado, "a barra fecha mesmo com falha");
            })
            .expect("a janela abriu");
    }

    /// 🚨 O lote inteiro cai quando o backend recusa a assinatura: marcar só uma
    /// deixaria as outras penduradas esperando uma assinatura que não vem.
    #[gpui_kit::test]
    async fn a_recusa_da_assinatura_derruba_o_lote(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        *acervo.recusa_a_assinatura.lock().unwrap() =
            Some("caminho inválido: ../backup".to_string());
        let (janela, mut visual) = tela(cx, acervo.clone());
        let pasta = pasta_com(3);

        janela
            .update(cx, |tela, window, cx| {
                tela.receber(vec![pasta.path().join("ensaio-silva")], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        assert!(acervo.enviados.lock().unwrap().is_empty(), "nada subiu");
        janela
            .update(cx, |tela, _window, _cx| {
                let andamento = andamento_de(tela.pecas());
                assert_eq!(andamento.com_erro, 3);
                assert!(andamento.terminado);
            })
            .expect("a janela abriu");
    }

    /// 🔑 **Os dois sobem** — o original e o WebP ao lado (decisão do dono,
    /// 2026-09-18). E o convertido é irmão, com o nome trocado.
    #[gpui_kit::test]
    async fn a_imagem_sobe_com_o_webp_ao_lado(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());

        let raiz = tempfile::tempdir().expect("pasta");
        let foto = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(900, 600, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
        }));
        let caminho = raiz.path().join("capa.png");
        foto.save(&caminho).expect("png de teste");

        janela
            .update(cx, |tela, window, cx| {
                tela.conversao.maior_lado = 300;
                tela.receber(vec![caminho], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        let enviados: Vec<String> = acervo
            .enviados
            .lock()
            .unwrap()
            .iter()
            .map(|(caminho, _)| caminho.clone())
            .collect();
        assert_eq!(enviados, ["capa.png", "capa.webp"]);
    }

    /// Com o interruptor desligado, só o original sobe.
    #[gpui_kit::test]
    async fn sem_converter_sobe_so_o_original(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());

        let raiz = tempfile::tempdir().expect("pasta");
        let foto = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(900, 600, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
        }));
        let caminho = raiz.path().join("capa.png");
        foto.save(&caminho).expect("png de teste");

        janela
            .update(cx, |tela, window, cx| {
                tela.converter = false;
                tela.receber(vec![caminho], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        let enviados = acervo.enviados.lock().unwrap().clone();
        assert_eq!(enviados.len(), 1);
        assert_eq!(enviados[0].0, "capa.png");
    }
    /// 🔑 **A opção sem arrastar** (dono, 2026-09-19). O que a janela do
    /// sistema devolve entra pelo mesmo caminho do arrasto.
    #[gpui_kit::test]
    async fn escolher_pasta_sobe_sem_arrastar_nada(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::default());
        let pasta = pasta_com(4);
        *escolha.resposta.lock().unwrap() = vec![pasta.path().join("ensaio-silva")];

        let (janela, mut visual) = com_escolha(cx, acervo.clone(), escolha.clone());
        janela
            .update(cx, |tela, window, cx| tela.escolher(true, window, cx))
            .expect("a janela abriu");
        assentar(&mut visual);

        assert_eq!(*escolha.pediram_pasta.lock().unwrap(), 1);
        assert_eq!(acervo.enviados.lock().unwrap().len(), 4);
    }

    /// Escolher arquivos avulsos: cada um sobe com o próprio nome.
    #[gpui_kit::test]
    async fn escolher_arquivos_sobe_os_avulsos(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::default());
        let pasta = pasta_com(3);
        *escolha.resposta.lock().unwrap() = vec![
            pasta.path().join("ensaio-silva").join("DSC_00.NEF"),
            pasta.path().join("ensaio-silva").join("DSC_01.NEF"),
        ];

        let (janela, mut visual) = com_escolha(cx, acervo.clone(), escolha.clone());
        janela
            .update(cx, |tela, window, cx| tela.escolher(false, window, cx))
            .expect("a janela abriu");
        assentar(&mut visual);

        assert_eq!(*escolha.pediram_arquivos.lock().unwrap(), 1);
        let enviados: Vec<String> = acervo
            .enviados
            .lock()
            .unwrap()
            .iter()
            .map(|(caminho, _)| caminho.clone())
            .collect();
        assert_eq!(enviados, ["DSC_00.NEF", "DSC_01.NEF"]);
    }

    /// ⚠️ **Fechar a janela do sistema não é erro.** Lista vazia é desistência,
    /// e um aviso vermelho ali ensinaria o operador a ignorar avisos.
    #[gpui_kit::test]
    async fn desistir_da_escolha_nao_vira_erro(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::default());

        let (janela, mut visual) = com_escolha(cx, acervo.clone(), escolha);
        janela
            .update(cx, |tela, window, cx| tela.escolher(true, window, cx))
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.erro.is_none(), "desistir não é falha");
                assert!(tela.pecas().is_empty());
            })
            .expect("a janela abriu");
        assert!(acervo.enviados.lock().unwrap().is_empty());
    }
    /// Uma foto de verdade, em PNG, para a prévia ter o que decodificar.
    fn png_de_teste(largura: u32, altura: u32) -> Vec<u8> {
        let foto =
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(largura, altura, |x, y| {
                image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
            }));
        let mut bytes = Vec::new();
        foto.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("png de teste");
        bytes
    }

    fn entrada(nome: &str, pasta: bool) -> EntradaDoAcervo {
        EntradaDoAcervo {
            caminho: nome.to_string(),
            nome: nome.to_string(),
            pasta,
            bytes: (!pasta).then_some(10),
            modificado_em: None,
        }
    }

    /// 🔑 **Visualizar a foto** (dono, 2026-09-19). Os bytes vêm pelo link
    /// assinado e viram textura.
    #[gpui_kit::test]
    async fn a_foto_abre_na_previa(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        acervo
            .conteudo
            .lock()
            .unwrap()
            .insert("capa.png".into(), png_de_teste(400, 300));

        let (janela, mut visual) = tela(cx, acervo.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.entradas = vec![entrada("capa.png", false), entrada("nota.pdf", false)];
                tela.ver("capa.png".into(), window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                let vendo = tela.vendo.as_ref().expect("a prévia abriu");
                assert_eq!(vendo.caminho, "capa.png");
                assert_eq!(vendo.pixels.width(), 400);
                assert!(tela.erro.is_none());
            })
            .expect("a janela abriu");
    }

    /// 🚨 **Girar roda os pixels**, porque o `Img` do GPUI não gira: 400×300
    /// vira 300×400.
    #[gpui_kit::test]
    async fn girar_troca_os_lados_da_foto(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        acervo
            .conteudo
            .lock()
            .unwrap()
            .insert("capa.png".into(), png_de_teste(400, 300));

        let (janela, mut visual) = tela(cx, acervo.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.entradas = vec![entrada("capa.png", false)];
                tela.ver("capa.png".into(), window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, cx| {
                tela.girar(cx);
                let vendo = tela.vendo.as_ref().expect("a prévia abriu");
                assert_eq!((vendo.pixels.width(), vendo.pixels.height()), (300, 400));
                assert_eq!(vendo.giro, 90.0);
            })
            .expect("a janela abriu");
    }

    /// 🔑 As setas percorrem **as fotos**, e pulam o que não é foto: um PDF no
    /// meio abriria um quadro vazio.
    #[gpui_kit::test]
    async fn as_setas_pulam_o_que_nao_e_foto(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        for nome in ["a.png", "c.png"] {
            acervo
                .conteudo
                .lock()
                .unwrap()
                .insert(nome.into(), png_de_teste(60, 40));
        }

        let (janela, mut visual) = tela(cx, acervo.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.entradas = vec![
                    entrada("a.png", false),
                    entrada("b.pdf", false),
                    entrada("c.png", false),
                ];
                tela.ver("a.png".into(), window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, window, cx| tela.ver_vizinha(1, window, cx))
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.vendo.as_ref().expect("prévia").caminho, "c.png");
            })
            .expect("a janela abriu");
    }

    /// 📦 **Baixar a pasta compactada** (dono, 2026-09-19): o zip sai no
    /// destino escolhido, com os caminhos da árvore dentro.
    #[gpui_kit::test]
    async fn a_pasta_baixa_compactada(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        acervo.arvores.lock().unwrap().insert(
            "2026".into(),
            vec![
                ParaBaixar {
                    caminho: "ensaio/DSC_01.NEF".into(),
                    bytes: Some(4),
                    url: "https://r2.de-mentira/1".into(),
                    com_autenticacao: false,
                },
                ParaBaixar {
                    caminho: "capa.jpg".into(),
                    bytes: Some(4),
                    url: "https://r2.de-mentira/2".into(),
                    com_autenticacao: false,
                },
            ],
        );
        {
            let mut conteudo = acervo.conteudo.lock().unwrap();
            conteudo.insert("ensaio/DSC_01.NEF".into(), b"cru!".to_vec());
            conteudo.insert("capa.jpg".into(), b"foto".to_vec());
        }

        let pasta = tempfile::tempdir().expect("pasta temporária");
        let destino = pasta.path().join("ensaio.zip");
        let escolha = Arc::new(EscolhaDeMentira::default());
        *escolha.destino.lock().unwrap() = vec![destino.clone()];

        let (janela, mut visual) = com_escolha(cx, acervo.clone(), escolha.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.pasta = "2026".into();
                tela.baixar_pasta(window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        assert_eq!(
            escolha.nome_pedido.lock().unwrap().as_deref(),
            Some("2026.zip"),
            "o zip é sugerido com o nome da pasta"
        );

        let bytes = std::fs::read(&destino).expect("o zip existe");
        let mut lido = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip legível");
        let mut nomes: Vec<String> = lido.file_names().map(str::to_string).collect();
        nomes.sort();
        assert_eq!(nomes, ["capa.jpg", "ensaio/DSC_01.NEF"]);

        let mut dentro = lido.by_name("capa.jpg").expect("a foto está lá");
        let mut conteudo = Vec::new();
        std::io::Read::read_to_end(&mut dentro, &mut conteudo).expect("ler");
        assert_eq!(conteudo, b"foto");
    }

    /// ⚠️ **Desistir do diálogo não cria arquivo nenhum.**
    #[gpui_kit::test]
    async fn desistir_do_destino_nao_deixa_zip_pela_metade(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        acervo.arvores.lock().unwrap().insert(
            "2026".into(),
            vec![ParaBaixar {
                caminho: "capa.jpg".into(),
                bytes: Some(4),
                url: "https://r2.de-mentira/2".into(),
                com_autenticacao: false,
            }],
        );

        let (janela, mut visual) = tela(cx, acervo.clone());
        janela
            .update(cx, |tela, window, cx| {
                tela.pasta = "2026".into();
                tela.baixar_pasta(window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.zip.is_none());
                assert!(tela.erro.is_none(), "desistir não é falha");
            })
            .expect("a janela abriu");
        assert!(
            acervo.baixados.lock().unwrap().is_empty(),
            "nada foi baixado"
        );
    }

    /// Pasta vazia diz o que é, em vez de abrir um diálogo de salvar para nada.
    #[gpui_kit::test]
    async fn pasta_vazia_nao_abre_dialogo(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let escolha = Arc::new(EscolhaDeMentira::default());

        let (janela, mut visual) = com_escolha(cx, acervo, escolha.clone());
        janela
            .update(cx, |tela, window, cx| tela.baixar_pasta(window, cx))
            .expect("a janela abriu");
        assentar(&mut visual);

        assert!(escolha.nome_pedido.lock().unwrap().is_none());
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.erro.as_deref().is_some_and(|e| e.contains("vazia")));
            })
            .expect("a janela abriu");
    }
    /// 🚨 **O teste da memória** (dono, 2026-09-19: *"gasta muita memória"* —
    /// 15,77 GB subindo uma pasta de RAW).
    ///
    /// A causa era estrutural: a preparação lia **todo** arquivo para a memória
    /// e mandava os bytes por um canal sem limite, e a tela os guardava num
    /// mapa até a vez de cada um — subindo três por vez, a pasta inteira ficava
    /// na RAM.
    ///
    /// O que este teste afirma é o contorno: **nenhuma peça carrega conteúdo**.
    /// Cada uma leva o caminho de onde os bytes saem, e quem lê é a tarefa que
    /// envia. Uma peça que voltasse a carregar `Vec<u8>` não caberia no tipo, e
    /// é isso que se confere aqui — junto com o tamanho, que continua sendo
    /// lido do disco para a barra andar por bytes.
    #[gpui_kit::test]
    async fn a_fila_guarda_caminhos_e_nao_o_conteudo(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());
        let pasta = pasta_com(30);

        janela
            .update(cx, |tela, window, cx| {
                tela.receber(vec![pasta.path().join("ensaio-silva")], window, cx);
            })
            .expect("a janela abriu");
        assentar(&mut visual);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.pecas().len(), 30);
                for peca in tela.pecas() {
                    assert!(
                        peca.origem.is_some(),
                        "{} tem de saber de onde ler, e não carregar o conteúdo",
                        peca.caminho
                    );
                    assert!(peca.bytes > 0, "o tamanho vem do disco, para a barra");
                }
            })
            .expect("a janela abriu");

        assert_eq!(acervo.enviados.lock().unwrap().len(), 30);
    }

    /// 🔑 **A pasta temporária é única por envio, e some no fim.**
    ///
    /// A versão anterior usava `temp_dir()/vlb-backup-{pid}` com arquivos
    /// `0.webp`, `1.webp`: dois envios no mesmo processo escreviam por cima um
    /// do outro, e o primeiro a terminar apagava os arquivos do segundo. Foi o
    /// que fez este arquivo de testes falhar de um jeito que parecia conversão
    /// quebrada — e não era.
    #[gpui_kit::test]
    async fn dois_envios_seguidos_nao_disputam_a_pasta_temporaria(cx: &mut TestAppContext) {
        let acervo = Arc::new(AcervoDeArquivosDeMentira::default());
        let (janela, mut visual) = tela(cx, acervo.clone());

        let raiz = tempfile::tempdir().expect("pasta");
        let foto = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(900, 600, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
        }));
        let caminho = raiz.path().join("capa.png");
        foto.save(&caminho).expect("png de teste");

        for volta in 1..=2 {
            janela
                .update(cx, |tela, window, cx| {
                    tela.conversao.maior_lado = 300;
                    tela.receber(vec![caminho.clone()], window, cx);
                })
                .expect("a janela abriu");
            assentar(&mut visual);

            let enviados = acervo.enviados.lock().unwrap().len();
            assert_eq!(
                enviados,
                volta * 2,
                "a volta {volta} tinha de subir o original e o WebP"
            );
        }

        // 🔑 Terminado o envio, a pasta temporária vai embora sozinha.
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(
                    tela.temporarios.is_none(),
                    "o TempDir some quando a fila acaba"
                );
            })
            .expect("a janela abriu");
    }
}
