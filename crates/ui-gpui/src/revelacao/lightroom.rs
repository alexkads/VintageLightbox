//! A tela de importar predefinições do Lightroom e do darktable: o seletor de
//! arquivos, o relatório do que ficou de fora e a porta de [`darktable`].
//!
//! A leitura e a tradução do Lightroom moram em `infrastructure::lightroom`
//! desde 30/set/2026, porque a importação de fotos também as usa (o XMP de
//! dentro do DNG). Tudo de lá é reexportado aqui: quem chamava
//! `revelacao::lightroom::traduzir` chama igual.

pub use infrastructure::lightroom::*;

/// Os estilos do darktable (`darktable.ts`).
pub mod darktable;

/// Um arquivo que o fotógrafo escolheu, já lido do disco.
///
/// `texto` é `None` quando a leitura falhou — permissão, arquivo binário, disco
/// removido no meio. **Não é a mesma coisa que "não é preset do Lightroom"**, e
/// o relatório separa as duas: uma se resolve escolhendo outro arquivo, a outra
/// não se resolve.
#[derive(Debug, Clone, PartialEq)]
pub struct Arquivo {
    pub nome: String,
    pub texto: Option<String>,
}

/// Quem abre o seletor do sistema e devolve o conteúdo lido.
///
/// 🔑 **Porta própria, como o [`super::presets::GuardaDePresets`]** e pelas
/// mesmas duas razões: abrir diálogo do sistema é `async` do tokio, e os testes
/// de tela precisam importar sem que nenhuma janela apareça na máquina de quem
/// roda a suíte.
///
/// ⚠️ **Responde sempre**, mesmo que com a lista vazia. Silêncio deixaria a tela
/// esperando arquivos que nunca vêm, com o botão desligado até fechar o app.
pub trait EscolhaDePresets: Send + Sync + 'static {
    fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>);
}

/// O seletor do sistema, via `rfd` — a mesma escolha da importação de fotos.
pub struct EscolhaNativa {
    tokio: tokio::runtime::Handle,
}

impl EscolhaNativa {
    pub fn nova(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }
}

impl EscolhaDePresets for EscolhaNativa {
    fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>) {
        self.tokio.spawn(async move {
            let escolhidos = rfd::AsyncFileDialog::new()
                .set_title("Importar do Lightroom ou do darktable")
                .add_filter(
                    "Predefinições do Lightroom e estilos do darktable",
                    &["lrtemplate", "xmp", "dtstyle"],
                )
                .pick_files()
                .await
                .unwrap_or_default();

            let mut arquivos = Vec::with_capacity(escolhidos.len());
            for escolhido in escolhidos {
                let caminho = escolhido.path().to_path_buf();
                let nome = caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| caminho.to_string_lossy().to_string());
                // ⚠️ **`read_to_string` recusa o que não é UTF-8**, e é o que se
                // quer: os dois formatos são texto, e um binário lido como
                // preset entraria como "sem nenhum ajuste" em vez de "não deu
                // para ler".
                arquivos.push(Arquivo {
                    nome,
                    texto: tokio::fs::read_to_string(&caminho).await.ok(),
                });
            }

            let _ = canal.send(arquivos);
        });
    }
}

/// O que a importação aproveitou — e o que não.
///
/// 🔑 **Sem isto, um preset de filme entraria como "importado com sucesso" e
/// mudaria menos do que o nome promete.** Dizer o que ficou de fora é a
/// diferença entre uma tradução e um engano silencioso: quem não vê o aviso
/// procura defeito no próprio olho, no monitor, ou no arquivo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Relatorio {
    pub arquivos: usize,
    pub criadas: usize,
    /// Já existia uma com o mesmo nome, e ela não foi tocada.
    pub repetidas: usize,
    /// Lidas, e sem nenhum ajuste que este motor aplique.
    pub sem_ajuste: Vec<String>,
    /// Nenhum dos leitores reconheceu o conteúdo.
    pub ilegiveis: Vec<String>,
    /// Recurso que este motor não tem, e em quantos arquivos apareceu — o que
    /// apareceu em mais arquivos primeiro.
    pub ignorados: Vec<(String, usize)>,
}

/// Lê os arquivos escolhidos e diz o que fazer com cada um.
///
/// Devolve as predefinições a criar (na ordem em que os arquivos vieram) e o
/// relatório do que ficou pelo caminho. **Não grava nada** — quem grava é a
/// tela, que é quem conhece a porta e a lista.
///
/// ⚠️ **Nome repetido é pulado, e não sobrescrito.** Reimportar a mesma pasta é
/// gesto comum, e sobrescrever apagaria o ajuste que o fotógrafo fez em cima da
/// predefinição depois de importá-la.
pub fn preparar(arquivos: &[Arquivo], ja_existem: &[String]) -> (Vec<PresetTraduzido>, Relatorio) {
    let mut relatorio = Relatorio {
        arquivos: arquivos.len(),
        ..Default::default()
    };
    let mut nomes: Vec<String> = ja_existem.to_vec();
    let mut criar = Vec::new();
    // Na ordem em que apareceram, como o `Map` do site: o desempate da
    // ordenação por contagem é esse.
    let mut contagem: Vec<(String, usize)> = Vec::new();

    for arquivo in arquivos {
        let Some(texto) = arquivo.texto.as_deref() else {
            relatorio.ilegiveis.push(arquivo.nome.clone());
            continue;
        };
        // 🔑 **O darktable vem primeiro, e decidido pelo conteúdo**: o `.xmp`
        // dele e o do Lightroom têm a mesma extensão, e o leitor do Lightroom
        // leria o do darktable como um preset sem ajuste nenhum.
        let traduzido = if darktable::eh_do_darktable(texto, &arquivo.nome) {
            darktable::ler(texto, &arquivo.nome)
        } else {
            ler_arquivo(texto, &arquivo.nome).map(|bruto| traduzir(&bruto))
        };
        let Some(traduzido) = traduzido else {
            relatorio.ilegiveis.push(arquivo.nome.clone());
            continue;
        };

        for rotulo in &traduzido.ignorados {
            match contagem.iter_mut().find(|(r, _)| r == rotulo) {
                Some((_, quantos)) => *quantos += 1,
                None => contagem.push((rotulo.clone(), 1)),
            }
        }

        if traduzido.ajustes.is_empty() {
            relatorio.sem_ajuste.push(traduzido.nome);
            continue;
        }
        if nomes.iter().any(|nome| nome == &traduzido.nome) {
            relatorio.repetidas += 1;
            continue;
        }

        nomes.push(traduzido.nome.clone());
        criar.push(traduzido);
    }

    relatorio.criadas = criar.len();
    relatorio.ignorados = contagem;
    // O que apareceu em mais arquivos primeiro: é o que mais falta ao motor.
    relatorio
        .ignorados
        .sort_by_key(|(_, quantos)| std::cmp::Reverse(*quantos));
    (criar, relatorio)
}

/// Uma linha por vez, para a tela desenhar o resultado.
impl Relatorio {
    pub fn linhas(&self) -> Vec<String> {
        let mut linhas = Vec::new();
        if self.repetidas > 0 {
            linhas.push(format!(
                "{} já existiam com o mesmo nome e foram puladas.",
                self.repetidas
            ));
        }
        if !self.sem_ajuste.is_empty() {
            linhas.push(format!(
                "{} não tinham nenhum ajuste que este motor aplica e ficaram de fora{}.",
                self.sem_ajuste.len(),
                if self.sem_ajuste.len() <= 3 {
                    format!(" ({})", self.sem_ajuste.join(", "))
                } else {
                    String::new()
                }
            ));
        }
        if !self.ilegiveis.is_empty() {
            linhas.push(format!("{} não puderam ser lidos.", self.ilegiveis.len()));
        }
        linhas
    }

    /// Os recursos que ficaram de fora, um por linha — `"3× remoção de névoa"`.
    pub fn linhas_dos_ignorados(&self) -> Vec<String> {
        self.ignorados
            .iter()
            .map(|(rotulo, quantos)| format!("{quantos}× {rotulo}"))
            .collect()
    }

    /// O cabeçalho: quantos arquivos entraram e quantas predefinições saíram.
    pub fn resumo(&self) -> String {
        format!(
            "{} {}: {} {}.",
            self.arquivos,
            if self.arquivos == 1 {
                "arquivo lido"
            } else {
                "arquivos lidos"
            },
            self.criadas,
            if self.criadas == 1 {
                "predefinição criada"
            } else {
                "predefinições criadas"
            }
        )
    }
}

/// A porta de mentira: responde com o que o teste mandar, sem abrir janela.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::{Arquivo, EscolhaDePresets};

    #[derive(Default)]
    pub struct EscolhaDeMentira {
        resposta: Mutex<Vec<Arquivo>>,
    }

    impl EscolhaDeMentira {
        pub fn com(arquivos: Vec<Arquivo>) -> Self {
            Self {
                resposta: Mutex::new(arquivos),
            }
        }
    }

    impl EscolhaDePresets for EscolhaDeMentira {
        fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>) {
            let resposta = self.resposta.lock().expect("a resposta do seletor").clone();
            let _ = canal.send(resposta);
        }
    }
}
