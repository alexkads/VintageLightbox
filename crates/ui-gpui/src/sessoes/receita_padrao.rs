//! A receita padrão revelada em segundo plano — o `receita-padrao/agendador.ts`
//! do site, portado.
//!
//! # O que faltava
//!
//! No site, escolher o preset padrão faz duas coisas: grava os PARÂMETROS e
//! **revela a foto**, na fila do trabalhador da exportação, regravando a prévia
//! local. É por isso que lá as fotos aparecem reveladas na galeria, aos poucos,
//! sem ninguém pedir.
//!
//! Aqui, até 17/set/2026, só a primeira metade existia: `aplicar_receita`
//! gravava os ajustes e parava. O operador entrava na sessão e via os brutos,
//! com o preset escolhido e nada aplicado — e a barra "Preset padrão" enchia na
//! hora, porque contava o **despacho** e não o trabalho (achado do dono).
//!
//! O [Contrato da Foto](../../../../recordarfotos-e-commerce/docs/CONTRATO_DA_FOTO.md)
//! trata a revelada e a prévia como **opcionais** no preset padrão (a linha
//! "Aplicar preset / sincronizar / preset padrão"): os PARÂMETROS são o que não
//! pode faltar. Este módulo exerce a opção, como o site exerce — e o que ele
//! escreve é **cache** (C15: a miniatura nasce da VERSÃO REVELADA), descartável
//! por definição.
//!
//! # 🚨 Chave própria, e é a lição de 7/set
//!
//! A miniatura revelada **não** pode ir para a chave do bruto. O editor lê os
//! pixels da foto local por `get_preview(&foto.id)` (`revelacao/tela.rs`), e
//! gravar a revelada ali serviria ao shader uma foto já revelada: receita por
//! cima de receita, em 640 px. É o mesmo defeito que `chave_do_trabalho`
//! existe para não repetir. Daí [`persistencia::chave_da_revelada`].
//!
//! # 🚨 O corte entra aqui, e as dimensões são as da imagem
//!
//! A etapa 2 promete **duas** coisas: a predefinição e a proporção do corte. Até
//! 17/set/2026 só a primeira chegava à tela — o corte era calculado com
//! `PhotoViewModel::width/height`, que vêm do **EXIF** (`PixelXDimension`), e
//! esse campo não existe na maioria dos arquivos: medido em dois JPEG, num NEF e
//! num CR2 reais, **nenhum** o traz. Sem dimensão, `corte_centralizado` devolve
//! "foto inteira", e o corte padrão simplesmente não acontecia — nem na imagem,
//! nem nos PARÂMETROS.
//!
//! Aqui as dimensões são as da **imagem que vai ser revelada**, que sempre
//! existem. O corte é fração (0..1), então medi-lo na prévia ou no bruto dá o
//! mesmo retângulo — é a mesma conta do site, que o mede em `integral.origem`.
//!
//! # Quem grava os PARÂMETROS é este serviço
//!
//! Pelo mesmo motivo: o corte só é conhecido depois de a imagem abrir. O site
//! faz igual — quem grava a receita é o trabalhador, depois de revelar
//! (`exportacao/worker.ts`). A tela grava sozinha só o caso de **desfazer**
//! (receita sem efeito), que não precisa de imagem nenhuma.
//!
//! # Uma por vez, numa thread só
//!
//! O mesmo arranjo de `nova/amostras.rs` e de `revelacao/processador.rs`: o
//! motor abre na thread e fica nela. Uma foto por vez, de propósito — a GPU é a
//! mesma que desenha a janela, e revelar trinta em paralelo faria a interface
//! engasgar justamente enquanto o operador escolhe o preset.
//!
//! # O progresso é contado aqui, e não no despacho
//!
//! Quem lê `Progresso` sabe quantas **terminaram**. É o que faz a barra medir
//! trabalho em vez de intenção.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::gpu_adjustments::Ajustes;

use crate::revelacao::persistencia::{chave_da_revelada, para_crop_settings, Corte, Gravador};
use crate::sessoes::nova::receita;

/// O lado maior da miniatura revelada que fica no cache.
///
/// O mesmo tamanho que `PreviewManager` usa para miniatura: a grade da sessão e
/// a do assistente desenham quadros de ~72 a 320 px, e guardar mais é pagar
/// decode por pixel que ninguém vê.
const LADO: u32 = 640;

/// O lado da miniatura gravada ao lado da revelada — o que a grade desenha.
const LADO_DA_MINIATURA: u32 = 320;

/// Quantas voltas esperar a prévia de uma foto antes de desistir dela.
///
/// ⚠️ **Desistir não é perder os PARÂMETROS**: eles são gravados assim mesmo,
/// sem corte (que é o que não se sabe sem a imagem). O que se perde é o cache,
/// que a próxima abertura refaz.
const ESPERAS_ATE_DESISTIR: u32 = 150;

/// De quanto em quanto tempo a fila é revista quando alguém está esperando a
/// prévia. Sem isto, a foto adiada só andaria no próximo pedido.
const RESPIRO: Duration = Duration::from_millis(200);

/// Uma foto esperando a receita.
struct Pedido {
    foto_id: String,
    ajustes: Ajustes,
    /// O rótulo da proporção do corte padrão (`"3:2"`, `"livre"`…), ou nenhuma.
    proporcao: Option<String>,
    /// Voltas dadas sem a prévia existir — ver [`ESPERAS_ATE_DESISTIR`].
    esperas: u32,
}

/// Quantas já terminaram, de quantas foram pedidas.
///
/// ⚠️ **`total` é o que entrou na fila desde a última limpeza**, e não o tamanho
/// da sessão: quem troca o preset três vezes não soma três lotes — `recomecar`
/// zera os dois números junto com a fila.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progresso {
    pub total: usize,
    pub prontas: usize,
}

impl Progresso {
    pub fn andando(&self) -> bool {
        self.prontas < self.total
    }
}

/// O serviço: uma fila, uma thread, um motor.
///
/// 🔑 **Vive fora das telas.** É entregue por porta ao assistente e à sessão,
/// como as outras (`PortasDaNova`): o operador cria a sessão, o assistente sai
/// de cena e a revelação continua — que é o pedido ("em segundo plano, e ao
/// abrir a sessão ir acontecendo").
pub struct ReceitaPadrao {
    fila: Arc<Mutex<VecDeque<Pedido>>>,
    progresso: Arc<Mutex<Progresso>>,
    acordar: Sender<()>,
}

impl ReceitaPadrao {
    /// Abre o serviço. `avisos` recebe o id de cada foto revelada, para a grade
    /// esquecer a miniatura velha e reler (C17).
    ///
    /// `gravador` é a mesma porta da Revelação: é por ela que os PARÂMETROS da
    /// receita padrão — ajustes **e corte** — chegam ao catálogo.
    pub fn nova(
        previews: Arc<PreviewManager>,
        gravador: Arc<dyn Gravador>,
        avisos: Sender<String>,
    ) -> Self {
        let fila: Arc<Mutex<VecDeque<Pedido>>> = Arc::new(Mutex::new(VecDeque::new()));
        let progresso = Arc::new(Mutex::new(Progresso::default()));
        let (acordar, acordou) = channel::<()>();
        let servico = Self {
            fila: fila.clone(),
            progresso: progresso.clone(),
            acordar,
        };
        std::thread::Builder::new()
            .name("receita-padrao".into())
            .spawn(move || laco(fila, progresso, previews, gravador, avisos, acordou))
            .expect("abrir a thread da receita padrão");
        servico
    }

    /// Esta foto precisa da receita. Chamar de novo com a mesma foto na fila não
    /// a duplica.
    pub fn pedir(&self, foto_id: String, ajustes: Ajustes, proporcao: Option<String>) {
        {
            let mut fila = self.fila.lock().expect("a fila da receita");
            if fila.iter().any(|p| p.foto_id == foto_id) {
                return;
            }
            fila.push_back(Pedido {
                foto_id,
                ajustes,
                proporcao,
                esperas: 0,
            });
        }
        self.progresso.lock().expect("o progresso").total += 1;
        // Se a thread morreu, o pedido fica na fila e ninguém trava: a foto
        // aparece como veio, que é o desfecho de não ter GPU.
        let _ = self.acordar.send(());
    }

    /// A receita mudou: o que ainda não saiu da fila não vale mais.
    ///
    /// 🔑 **O que já foi revelado fica.** Ele é cache de uma receita antiga e
    /// será substituído quando a foto voltar à fila; apagar aqui deixaria a
    /// grade vazia no meio da troca de preset, que é o pior momento.
    pub fn recomecar(&self) {
        self.fila.lock().expect("a fila da receita").clear();
        *self.progresso.lock().expect("o progresso") = Progresso::default();
    }

    pub fn progresso(&self) -> Progresso {
        *self.progresso.lock().expect("o progresso")
    }
}

fn laco(
    fila: Arc<Mutex<VecDeque<Pedido>>>,
    progresso: Arc<Mutex<Progresso>>,
    previews: Arc<PreviewManager>,
    gravador: Arc<dyn Gravador>,
    avisos: Sender<String>,
    acordou: Receiver<()>,
) {
    #[cfg(not(test))]
    let mut motor = infrastructure::gpu_adjustments::Motor::abrir();
    #[cfg(test)]
    let mut motor: Option<infrastructure::gpu_adjustments::Motor> = None;

    loop {
        // 🔑 **Acorda pelo pedido ou pelo relógio.** O relógio existe para a
        // foto adiada: ela espera a prévia da importação, que chega sem avisar
        // este serviço.
        match acordou.recv_timeout(RESPIRO) {
            Ok(()) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // Um aviso pode cobrir vários pedidos: esvazia a fila antes de dormir.
        let mut adiados: Vec<Pedido> = Vec::new();
        loop {
            let Some(mut pedido) = fila.lock().expect("a fila da receita").pop_front() else {
                break;
            };
            match trabalhar(motor.as_mut(), &previews, gravador.as_ref(), &pedido) {
                Desfecho::Feito { avisar } => {
                    if avisar {
                        let _ = avisos.send(pedido.foto_id.clone());
                    }
                    progresso.lock().expect("o progresso").prontas += 1;
                }
                Desfecho::SemImagem => {
                    pedido.esperas += 1;
                    if pedido.esperas >= ESPERAS_ATE_DESISTIR {
                        // Os PARÂMETROS não se perdem por falta de imagem: vão
                        // sem corte, que é o que não dá para saber sem ela.
                        gravador.gravar(pedido.foto_id.clone(), pedido.ajustes, Corte::default());
                        progresso.lock().expect("o progresso").prontas += 1;
                    } else {
                        adiados.push(pedido);
                    }
                }
            }
        }
        if !adiados.is_empty() {
            let mut fila = fila.lock().expect("a fila da receita");
            for pedido in adiados {
                fila.push_back(pedido);
            }
        }
    }
}

/// O que aconteceu com um pedido.
enum Desfecho {
    /// Trabalhado. `avisar` só quando há cache novo para a grade reler.
    Feito { avisar: bool },
    /// A prévia da foto ainda não existe — tenta na próxima volta.
    SemImagem,
}

/// Grava os PARÂMETROS desta foto e, quando há o que ver, a miniatura revelada.
///
/// 🚨 **Os PARÂMETROS primeiro, e sempre** (C8): eles são o que não pode faltar.
/// A miniatura é cache (C15) e depende de GPU; se ela não sair, a foto aparece
/// como veio e a receita continua valendo.
fn trabalhar(
    motor: Option<&mut infrastructure::gpu_adjustments::Motor>,
    previews: &PreviewManager,
    gravador: &dyn Gravador,
    pedido: &Pedido,
) -> Desfecho {
    let Some(base) = previews
        .get_preview(&pedido.foto_id)
        .or_else(|| previews.get_thumbnail(&pedido.foto_id))
    else {
        return Desfecho::SemImagem;
    };

    // O corte sai das dimensões **da imagem**, e não do EXIF. Ver o cabeçalho.
    let corte =
        receita::corte_centralizado(pedido.proporcao.as_deref(), base.width(), base.height());
    gravador.gravar(pedido.foto_id.clone(), pedido.ajustes, corte);

    let tem_ajustes = pedido.ajustes != Ajustes::default();
    let tem_corte = corte != Corte::default();
    if !tem_ajustes && !tem_corte {
        // A receita é o neutro: o bruto **é** a foto com ela.
        return Desfecho::Feito { avisar: false };
    }

    let pequena = base.thumbnail(LADO, LADO);
    let revelada = if tem_ajustes {
        // Sem GPU não há como revelar; o corte sozinho não justifica um cache
        // que mostraria a foto cortada **sem** a predefinição — que é o que o
        // site também não faz quando o trabalhador não tem motor.
        let Some(motor) = motor else {
            return Desfecho::Feito { avisar: false };
        };
        let rgba = pequena.to_rgba8();
        let (largura, altura) = rgba.dimensions();
        match motor.revelar(&Arc::new(rgba.into_raw()), largura, altura, &pedido.ajustes) {
            Some(imagem) => imagem,
            None => return Desfecho::Feito { avisar: false },
        }
    } else {
        pequena
    };

    let final_ = if tem_corte {
        // O mesmo caminho da exportação (`image_exporter.rs`): revelar primeiro,
        // enquadrar depois.
        infrastructure::transformacao::aplicar(&revelada, &para_crop_settings(&corte), true)
    } else {
        revelada
    };

    // ⚠️ Falha de gravação não é fim de fluxo: a grade continua mostrando o
    // bruto, e a próxima passada tenta de novo.
    //
    // 🚨 **As duas chaves, e é o que faltava**: o serviço gravava só o preview
    // grande (`save_preview`), e as grades leem a miniatura (`get_thumbnail`) —
    // `Large` e `Thumbnail` são caches diferentes, então a revelada nunca era
    // encontrada e a tela seguia mostrando o bruto (achado do dono, 17/set/2026).
    let chave = chave_da_revelada(&pedido.foto_id);
    let _ = previews.save_preview(&chave, &final_);
    let _ = previews.save_thumbnail(
        &chave,
        &final_.thumbnail(LADO_DA_MINIATURA, LADO_DA_MINIATURA),
    );
    Desfecho::Feito { avisar: true }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::revelacao::persistencia::mentira::GravadorDeMentira;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn previews_com(
        id: &str,
        largura: u32,
        altura: u32,
    ) -> (Arc<PreviewManager>, tempfile::TempDir) {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let previews = Arc::new(PreviewManager::new_with_path(pasta.path().to_path_buf()));
        let mut imagem = RgbaImage::new(largura, altura);
        for pixel in imagem.pixels_mut() {
            *pixel = Rgba([120, 120, 120, 255]);
        }
        previews
            .save_preview(id, &DynamicImage::ImageRgba8(imagem))
            .expect("gravar a prévia");
        (previews, pasta)
    }

    fn servico() -> (ReceitaPadrao, Receiver<String>) {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let previews = Arc::new(PreviewManager::new_with_path(pasta.path().to_path_buf()));
        let (avisa, recebe) = channel();
        // A pasta vive enquanto o teste dura: soltá-la é de propósito.
        let _ = pasta.keep();
        (
            ReceitaPadrao::nova(previews, Arc::new(GravadorDeMentira::default()), avisa),
            recebe,
        )
    }

    #[test]
    fn a_mesma_foto_nao_entra_duas_vezes_na_fila() {
        let (servico, _avisos) = servico();
        let ajustes = Ajustes {
            exposure: 0.5,
            ..Ajustes::default()
        };
        servico.pedir("foto-1".into(), ajustes, None);
        servico.pedir("foto-1".into(), ajustes, None);
        // Uma na fila (ou já colhida pela thread), nunca duas.
        assert!(
            servico.progresso().total <= 1,
            "total = {}",
            servico.progresso().total
        );
    }

    #[test]
    fn recomecar_zera_o_que_nao_saiu() {
        let (servico, _avisos) = servico();
        servico.recomecar();
        assert_eq!(servico.progresso(), Progresso::default());
        assert!(!servico.progresso().andando());
    }

    /// 🚨 **A proporção escolhida na etapa 2 recorta a imagem** — era o que
    /// faltava: o corte ia só para os PARÂMETROS (e, sem EXIF, nem isso), e a
    /// grade seguia mostrando a foto inteira.
    #[test]
    fn a_proporcao_recorta_a_miniatura_e_entra_nos_parametros() {
        let (previews, _pasta) = previews_com("foto-1", 600, 400);
        let gravador = GravadorDeMentira::default();
        let pedido = Pedido {
            foto_id: "foto-1".into(),
            ajustes: Ajustes::default(),
            proporcao: Some("1:1".into()),
            esperas: 0,
        };

        let desfecho = trabalhar(None, &previews, &gravador, &pedido);
        assert!(matches!(desfecho, Desfecho::Feito { avisar: true }));

        // Os PARÂMETROS levam o corte, medido na imagem (o EXIF não o tem).
        let gravado = gravador.gravado();
        assert_eq!(gravado.len(), 1);
        assert!(
            gravado[0].2 != Corte::default(),
            "o corte tinha de ir nos parâmetros: {:?}",
            gravado[0].2
        );

        // E a miniatura sai quadrada, como a proporção pede.
        let chave = chave_da_revelada("foto-1");
        let cortada = previews.get_preview(&chave).expect("a revelada gravada");
        assert_eq!(
            cortada.width(),
            cortada.height(),
            "1:1 tinha de sair quadrada: {}×{}",
            cortada.width(),
            cortada.height()
        );
        assert!(
            previews.get_thumbnail(&chave).is_some(),
            "a grade lê a miniatura, e é ela que faltava ser gravada"
        );
    }

    /// A receita neutra é o bruto: gravar uma cópia dele sob outra chave
    /// dobraria o cache sem mudar um pixel na tela. Os PARÂMETROS, esses vão —
    /// é assim que a troca de preset **desfaz** a anterior.
    #[test]
    fn sem_efeito_nao_grava_cache_mas_grava_os_parametros() {
        let (previews, _pasta) = previews_com("foto-1", 600, 400);
        let gravador = GravadorDeMentira::default();
        let pedido = Pedido {
            foto_id: "foto-1".into(),
            ajustes: Ajustes::default(),
            proporcao: None,
            esperas: 0,
        };
        let desfecho = trabalhar(None, &previews, &gravador, &pedido);
        assert!(matches!(desfecho, Desfecho::Feito { avisar: false }));
        assert!(previews.get_preview(&chave_da_revelada("foto-1")).is_none());
        assert_eq!(gravador.gravado().len(), 1);
    }

    /// 🔑 **Sem prévia, o pedido espera** — e não some nem grava corte errado.
    /// É o `esperando` do site (`situacaoDoItem`), do lado de cá.
    #[test]
    fn sem_previa_o_pedido_espera() {
        let pasta = tempfile::tempdir().expect("pasta temporária");
        let previews = PreviewManager::new_with_path(pasta.path().to_path_buf());
        let gravador = GravadorDeMentira::default();
        let pedido = Pedido {
            foto_id: "foto-sem-previa".into(),
            ajustes: Ajustes::default(),
            proporcao: Some("1:1".into()),
            esperas: 0,
        };
        assert!(matches!(
            trabalhar(None, &previews, &gravador, &pedido),
            Desfecho::SemImagem
        ));
        assert!(gravador.gravado().is_empty(), "nada foi gravado sem imagem");
    }
}
