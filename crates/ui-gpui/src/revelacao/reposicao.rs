//! Refazer o que o cache perdeu, enquanto o fotógrafo olha.
//!
//! 🔑 **"Não tem preview no cache" não é o fim da linha para foto local.** O
//! cache é derivado: ele nasce no passo 5 da importação
//! (`ImportWithOptionsUseCase`) e some por motivos banais — "Limpar previews"
//! nas Configurações, um catálogo copiado sem o `Previews.lrdata`, uma
//! importação que gravou a foto e falhou ao gravar o preview. Em todos esses
//! casos o **original continua no disco**, e o que a Revelação mostrava era um
//! beco sem saída: uma frase, nenhum botão, e nada acontecendo.
//!
//! 🚨 **Uma resposta por foto, e não uma no fim.** A tira toda costuma estar
//! vazia junto com o palco (as duas entradas são gravadas no mesmo passo da
//! importação, e apagadas pelos mesmos botões), e um catálogo de duzentas fotos
//! levaria minutos para repor. Esperar o lote inteiro para só então acender a
//! tira é a diferença entre "está trabalhando" e "travou": aqui cada foto
//! reposta vai pelo canal na hora em que fica pronta, e a célula dela acende
//! sozinha.
//!
//! ⚠️ **Foto do site não passa por aqui.** Ela não tem arquivo nesta máquina
//! (`path` vazio), e o bruto dela vem da cópia de trabalho do storage — o passo
//! 11, que a raiz já sabia pedir. As duas reposições respondem à mesma pergunta
//! ("de onde vêm os pixels que faltam?") por caminhos que não se substituem.
//!
//! A forma é a das outras portas: o gerador é `async` do tokio, o GPUI não roda
//! futuros dele, e o `Handle` vem do `main`.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use domain::value_objects::FilePath;
use infrastructure::cache::preview_manager::PreviewManager;

/// O lado do preview grande, o mesmo que a importação grava.
///
/// 🚨 **Tem de ser o número da importação** (`import_with_options.rs`, passo 4:
/// `generate(&dest_path, 2560)`). Repor num lado diferente faria a mesma foto
/// abrir com nitidez diferente conforme tivesse sido importada ou reposta — e
/// ninguém encontraria o motivo, porque as duas telas estariam certas.
const LADO_DO_PREVIEW: u32 = 2560;

/// E o da miniatura, pelo mesmo motivo.
const LADO_DA_MINIATURA: u32 = 300;

/// Uma foto a repor: o id com que ela mora no cache, e o arquivo de onde ela sai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct APor {
    pub foto_id: String,
    pub caminho: String,
}

/// O que a reposição responde — **uma vez por foto**.
pub enum Recado {
    /// Esta foto foi refeita e **já está no cache**.
    ///
    /// A imagem grande vem junto para o palco não ter de reler o que acabou de
    /// ser gravado; a tira relê do cache, que é barato depois de gravado.
    ///
    /// 🔑 **Leva o id**, como o `Pixels` do pós-venda: uma reposição que termina
    /// depois de a seta ter andado não pode pintar a foto errada.
    Reposta {
        foto_id: String,
        imagem: Box<image::DynamicImage>,
    },
    /// Esta foto não deu — arquivo movido, disco fora, formato que o `image`
    /// não lê.
    ///
    /// ⚠️ **Não interrompe o lote.** Um cartão com um arquivo corrompido no
    /// meio não pode impedir as outras cento e noventa e nove de voltar.
    Falhou { foto_id: String, erro: String },
}

/// Quem sabe refazer o cache de fotos catalogadas.
pub trait Repositor: Send + Sync + 'static {
    /// Devolve na hora; as respostas vêm pelo canal, uma por foto, **na ordem
    /// em que forem ficando prontas**.
    ///
    /// A ordem dos pedidos é a de urgência — quem chama põe na frente a foto
    /// que está no palco.
    fn repor(&self, fotos: Vec<APor>, canal: Sender<Recado>);
}

pub struct RepositorDoDisco {
    miniaturas: Arc<dyn domain::services::ThumbnailGenerator>,
    previews: Arc<PreviewManager>,
    tokio: tokio::runtime::Handle,
}

impl RepositorDoDisco {
    pub fn novo(
        miniaturas: Arc<dyn domain::services::ThumbnailGenerator>,
        previews: Arc<PreviewManager>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            miniaturas,
            previews,
            tokio,
        }
    }
}

impl Repositor for RepositorDoDisco {
    fn repor(&self, fotos: Vec<APor>, canal: Sender<Recado>) {
        let miniaturas = self.miniaturas.clone();
        let previews = self.previews.clone();

        self.tokio.spawn(async move {
            // 🚨 **Uma de cada vez, e na ordem pedida.** `generate_set` já entra
            // num `spawn_blocking` por foto; disparar as duzentas de uma vez
            // encheria o pool de decodes concorrentes e a **primeira** — a que
            // está no palco, a única que alguém está olhando — sairia junto com
            // a última. Em série ela sai primeiro, que é o ponto.
            for APor { foto_id, caminho } in fotos {
                let recado = refazer(&*miniaturas, &previews, &foto_id, &caminho).await;
                // O canal fechou: a janela morreu, e não há mais para quem
                // repor. Continuar o lote seria decodificar duzentos JPEGs para
                // ninguém.
                if canal.send(recado).is_err() {
                    return;
                }
            }
        });
    }
}

/// O trabalho de uma foto, separado do laço para poder ser lido de uma vez.
///
/// 🔑 **Uma decodificação, dois tamanhos.** `generate_set` abre o arquivo uma
/// vez e reduz N vezes; pedir preview e miniatura em duas chamadas dobraria a
/// parte cara — que num JPEG de 6 MB é o decode, não o resize.
///
/// ⚠️ **A miniatura vai junto de propósito**, mesmo quando quem pediu queria só
/// o palco: quando o preview falta, quase sempre a miniatura falta também, e a
/// tira fica uma fileira de retângulos pretos ao lado da foto reposta. Custa um
/// resize sobre a imagem já decodificada.
async fn refazer(
    miniaturas: &dyn domain::services::ThumbnailGenerator,
    previews: &PreviewManager,
    foto_id: &str,
    caminho: &str,
) -> Recado {
    let falha = |erro: String| Recado::Falhou {
        foto_id: foto_id.to_string(),
        erro,
    };

    let Ok(arquivo) = FilePath::new(caminho) else {
        return falha(format!("o caminho não vale: {caminho}"));
    };

    let bytes = match miniaturas
        .generate_set(&arquivo, &[LADO_DA_MINIATURA, LADO_DO_PREVIEW])
        .await
    {
        Ok(bytes) => bytes,
        Err(erro) => return falha(format!("não deu para reler {caminho}: {erro}")),
    };
    let [miniatura, preview] = bytes.as_slice() else {
        return falha(format!("{caminho} devolveu {} tamanhos", bytes.len()));
    };

    let (Ok(miniatura), Ok(preview)) = (
        image::load_from_memory(miniatura),
        image::load_from_memory(preview),
    ) else {
        return falha(format!("o preview refeito de {caminho} não abriu"));
    };

    // ⚠️ **Falha de gravação não impede de mostrar.** O cache é acelerador: sem
    // ele a foto abre igual, só volta a custar a decodificação na próxima vez.
    let _ = previews.save_thumbnail(foto_id, &miniatura);
    let _ = previews.save_preview(foto_id, &preview);

    Recado::Reposta {
        foto_id: foto_id.to_string(),
        imagem: Box::new(preview),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn jpeg_de_verdade(dir: &std::path::Path, nome: &str) -> String {
        let caminho = dir.join(nome);
        let mut imagem = image::RgbImage::new(64, 48);
        for pixel in imagem.pixels_mut() {
            *pixel = image::Rgb([200, 40, 40]);
        }
        image::DynamicImage::ImageRgb8(imagem)
            .save_with_format(&caminho, image::ImageFormat::Jpeg)
            .expect("gravar o JPEG de teste");
        caminho.to_string_lossy().into_owned()
    }

    /// 🚨 **Uma resposta por foto, e não uma no fim do lote.**
    ///
    /// É o que separa "está trabalhando" de "travou": com o cache apagado, a
    /// tira inteira entra no mesmo pedido, e um catálogo grande leva minutos. Se
    /// as respostas só saíssem no fim, o fotógrafo veria uma fileira de
    /// retângulos pretos durante todo esse tempo.
    ///
    /// O teste cobra a **primeira** resposta enquanto as outras ainda não
    /// chegaram — que é exatamente a afirmação que um lote só não sustenta.
    // Multi-thread de propósito: o `recv_timeout` bloqueia esta thread, e num
    // runtime de thread única a tarefa do repositor nunca chegaria a rodar.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cada_foto_responde_sozinha_e_a_primeira_e_a_pedida_na_frente() {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let cache = tempfile::TempDir::new().expect("diretório do cache");
        let previews = Arc::new(PreviewManager::new_with_path(cache.path().to_path_buf()));

        let fotos: Vec<APor> = ["palco.jpg", "vizinha.jpg", "longe.jpg"]
            .iter()
            .map(|nome| APor {
                foto_id: format!("id-{nome}"),
                caminho: jpeg_de_verdade(dir.path(), nome),
            })
            .collect();

        let repositor = RepositorDoDisco::novo(
            Arc::new(infrastructure::ThumbnailGeneratorImpl::new()),
            previews.clone(),
            tokio::runtime::Handle::current(),
        );

        let (envio, recebimento) = channel();
        repositor.repor(fotos, envio);

        // A primeira volta sozinha — sem esperar as outras duas.
        let primeira = recebimento
            .recv_timeout(Duration::from_secs(10))
            .expect("a primeira foto tem de voltar antes do fim do lote");
        let Recado::Reposta { foto_id, .. } = primeira else {
            panic!("a primeira reposição falhou");
        };
        assert_eq!(
            foto_id, "id-palco.jpg",
            "a ordem pedida é a ordem entregue: o palco primeiro"
        );
        assert!(
            previews.tem("id-palco.jpg", domain::services::PreviewType::Large),
            "a foto reposta já está no cache quando o recado chega"
        );
        assert!(
            previews.tem("id-palco.jpg", domain::services::PreviewType::Thumbnail),
            "a miniatura vai junto — senão a célula da tira continua preta"
        );

        for esperada in ["id-vizinha.jpg", "id-longe.jpg"] {
            let recado = recebimento
                .recv_timeout(Duration::from_secs(10))
                .expect("as seguintes também respondem, uma por uma");
            let Recado::Reposta { foto_id, .. } = recado else {
                panic!("a reposição de {esperada} falhou");
            };
            assert_eq!(foto_id, esperada);
        }
    }

    /// ⚠️ **Um arquivo ilegível não derruba o lote.**
    ///
    /// A foto movida para outro disco é o caso comum, e ela costuma estar no
    /// meio da tira. Interromper ali deixaria as seguintes sem reposição —
    /// pretas para sempre, por causa de uma que não era delas.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn o_arquivo_que_sumiu_nao_interrompe_as_seguintes() {
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let cache = tempfile::TempDir::new().expect("diretório do cache");
        let previews = Arc::new(PreviewManager::new_with_path(cache.path().to_path_buf()));

        let repositor = RepositorDoDisco::novo(
            Arc::new(infrastructure::ThumbnailGeneratorImpl::new()),
            previews.clone(),
            tokio::runtime::Handle::current(),
        );

        let (envio, recebimento) = channel();
        repositor.repor(
            vec![
                APor {
                    foto_id: "id-sumida.jpg".into(),
                    caminho: dir.path().join("nao-existe.jpg").to_string_lossy().into(),
                },
                APor {
                    foto_id: "id-boa.jpg".into(),
                    caminho: jpeg_de_verdade(dir.path(), "boa.jpg"),
                },
            ],
            envio,
        );

        assert!(
            matches!(
                recebimento.recv_timeout(Duration::from_secs(10)),
                Ok(Recado::Falhou { .. })
            ),
            "o arquivo que sumiu responde falha, e não silêncio"
        );
        assert!(
            matches!(
                recebimento.recv_timeout(Duration::from_secs(10)),
                Ok(Recado::Reposta { .. })
            ),
            "a seguinte foi reposta mesmo assim"
        );
    }
}

#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Um repositor que só anota o que lhe pediram, e responde o que mandarem.
    #[derive(Default)]
    pub struct RepositorDeMentira {
        pedidos: Mutex<Vec<APor>>,
        /// A imagem a devolver. `None` faz toda reposição falhar — que é o
        /// outro desfecho que a tela precisa saber tratar.
        resposta: Mutex<Option<image::DynamicImage>>,
    }

    impl RepositorDeMentira {
        pub fn que_devolve(imagem: image::DynamicImage) -> Self {
            Self {
                pedidos: Mutex::new(Vec::new()),
                resposta: Mutex::new(Some(imagem)),
            }
        }

        /// As fotos pedidas, na ordem — que é a ordem de urgência de quem pediu.
        pub fn pedidos(&self) -> Vec<APor> {
            self.pedidos.lock().expect("ler os pedidos").clone()
        }
    }

    impl Repositor for RepositorDeMentira {
        fn repor(&self, fotos: Vec<APor>, canal: Sender<Recado>) {
            let resposta = self.resposta.lock().expect("ler a resposta").clone();
            let mut pedidos = self.pedidos.lock().expect("anotar os pedidos");

            for foto in fotos {
                let recado = match resposta.clone() {
                    Some(imagem) => Recado::Reposta {
                        foto_id: foto.foto_id.clone(),
                        imagem: Box::new(imagem),
                    },
                    None => Recado::Falhou {
                        foto_id: foto.foto_id.clone(),
                        erro: format!("o repositor de mentira recusa {}", foto.caminho),
                    },
                };
                pedidos.push(foto);
                let _ = canal.send(recado);
            }
        }
    }
}
