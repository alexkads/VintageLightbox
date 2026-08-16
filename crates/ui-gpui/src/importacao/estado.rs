//! O estado da importação e as regras que mexem nele.
//!
//! Sem tela e sem disco: tudo aqui é função de estado, e é onde moram as decisões
//! que a tela apenas desenha — quem está marcado, em que ordem a grade mostra, o
//! que uma resposta atrasada pode e não pode sobrescrever.
//!
//! ## 🔑 O desenho é o do legado, e é bom: `aplicar` só mexe no estado
//!
//! Toda descoberta assíncrona chega como [`Recado`] e sai como mudança de estado
//! mais, às vezes, um [`Seguimento`] — "agora vá ler isto". Quem dispara trabalho
//! é a tela, que tem o controller em mãos. É o que permite testar a máquina
//! inteira sem runtime, sem banco e sem cartão de memória plugado.

use domain::value_objects::ImportOptions;

/// Um arquivo listado na grade.
///
/// ⚠️ **Ele nasce quase vazio.** O scan devolve só caminhos, e a grade aparece
/// cheia na hora com o nome do arquivo; câmera, data e tamanho chegam depois, no
/// `Descritos`. É a ordem que faz um cartão de 2.000 RAWs abrir a tela em vez de
/// travá-la por minutos.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidato {
    pub caminho: String,
    pub nome: String,
    /// A marca da célula — o que vai ser importado.
    pub marcado: bool,
    /// Já existe no catálogo, pelo hash do conteúdo.
    pub duplicado: bool,
    pub tamanho: u64,
    pub e_raw: bool,
    pub camera: String,
    /// Data de captura no formato EXIF (`"YYYY:MM:DD HH:MM:SS"`), vazia quando
    /// não há.
    pub data: String,
    pub dimensoes: Option<String>,
    /// Se os metadados já chegaram.
    pub descrito: bool,
}

impl Candidato {
    /// O que o scan sabe: o caminho, e o nome tirado dele.
    ///
    /// 🚨 **Nasce marcado.** Quem abre a tela de importação quer importar; fazer
    /// o fotógrafo marcar 2.000 células antes de começar inverteria o trabalho.
    /// Desmarcar é a exceção, e é o que as duplicatas fazem sozinhas.
    pub fn do_caminho(caminho: String) -> Self {
        let nome = std::path::Path::new(&caminho)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| caminho.clone());

        Self {
            caminho,
            nome,
            marcado: true,
            duplicado: false,
            tamanho: 0,
            e_raw: false,
            camera: String::new(),
            data: String::new(),
            dimensoes: None,
            descrito: false,
        }
    }
}

/// Por onde a grade ordena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ordem {
    /// Hora de captura — o padrão, e o único que reconstrói a sequência do ensaio.
    #[default]
    Captura,
    Nome,
    Tamanho,
    /// RAW antes de JPEG.
    Tipo,
}

impl Ordem {
    pub const TODAS: [Ordem; 4] = [Ordem::Captura, Ordem::Nome, Ordem::Tamanho, Ordem::Tipo];

    pub fn rotulo(&self) -> &'static str {
        match self {
            Ordem::Captura => "Hora de captura",
            Ordem::Nome => "Nome do arquivo",
            Ordem::Tamanho => "Tamanho",
            Ordem::Tipo => "Tipo (RAW primeiro)",
        }
    }
}

/// O que a importação descobriu, vindo de fora.
#[derive(Debug, Clone)]
pub enum Recado {
    /// A varredura terminou. Leva a raiz junto **de propósito** — ver
    /// [`aplicar`].
    Varrido { raiz: String, arquivos: Vec<String> },
    /// Os metadados dos arquivos listados.
    Descritos(Vec<Descricao>),
    /// Os caminhos que já existem no catálogo.
    Duplicados(Vec<String>),
    /// Algo falhou; a mensagem vai para a tela.
    Falhou(String),
}

/// O que a leitura de metadados devolve, por arquivo.
#[derive(Debug, Clone, PartialEq)]
pub struct Descricao {
    pub caminho: String,
    pub tamanho: u64,
    pub e_raw: bool,
    pub camera: String,
    pub data: String,
    pub dimensoes: Option<String>,
}

/// Trabalho que um recado deixou pendente.
#[derive(Debug, Clone, PartialEq)]
pub enum Seguimento {
    /// Ler metadados e conferir duplicatas destes arquivos.
    Detalhar(Vec<String>),
}

/// Tudo que a tela de importação sabe.
///
/// `Default` é derivado: o estado inicial é "nada escolhido, nada listado, nada
/// em andamento", e é o que cada campo já dá sozinho.
#[derive(Default)]
pub struct Estado {
    /// A pasta escolhida. `None` é "ninguém escolheu origem ainda".
    pub origem: Option<String>,
    pub candidatos: Vec<Candidato>,
    pub opcoes: ImportOptions,
    pub ordem: Ordem,
    /// Esconde da grade o que já está no catálogo.
    pub so_novos: bool,
    pub varrendo: bool,
    pub descrevendo: bool,
    pub conferindo_duplicatas: bool,
    /// A mensagem que aparece sobre a grade.
    pub aviso: Option<String>,
    /// A célula em foco.
    pub focado: Option<usize>,
    /// De onde um Shift+clique conta o intervalo.
    pub ancora: Option<usize>,
}

impl Estado {
    pub fn marcados(&self) -> usize {
        self.candidatos.iter().filter(|c| c.marcado).count()
    }

    pub fn bytes_marcados(&self) -> u64 {
        self.candidatos
            .iter()
            .filter(|c| c.marcado)
            .map(|c| c.tamanho)
            .sum()
    }

    pub fn duplicados(&self) -> usize {
        self.candidatos.iter().filter(|c| c.duplicado).count()
    }

    /// Os caminhos marcados, na ordem em que a grade os mostra.
    pub fn caminhos_marcados(&self) -> Vec<String> {
        self.candidatos
            .iter()
            .filter(|c| c.marcado)
            .map(|c| c.caminho.clone())
            .collect()
    }

    /// Os índices que a grade desenha, depois do filtro.
    ///
    /// ⚠️ **Índices, e não referências**: a marcação e o foco trabalham por
    /// posição no acervo, e uma lista filtrada de cópias faria clicar na terceira
    /// célula marcar a terceira foto do acervo — que com filtro ligado é outra.
    /// É o mesmo defeito que a grade da Biblioteca teve na fase 1.
    pub fn visiveis(&self) -> Vec<usize> {
        self.candidatos
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.so_novos || !c.duplicado)
            .map(|(i, _)| i)
            .collect()
    }

    /// Reordena a grade.
    ///
    /// 🚨 **Empata sempre pelo nome do arquivo.** Sem isso, fotos disparadas no
    /// mesmo segundo — uma rajada — trocariam de lugar a cada reordenação, e a
    /// grade pareceria embaralhar sozinha.
    pub fn ordenar(&mut self) {
        match self.ordem {
            Ordem::Captura => self
                .candidatos
                .sort_by(|a, b| a.data.cmp(&b.data).then(a.nome.cmp(&b.nome))),
            Ordem::Nome => self.candidatos.sort_by(|a, b| a.nome.cmp(&b.nome)),
            Ordem::Tamanho => self
                .candidatos
                .sort_by(|a, b| b.tamanho.cmp(&a.tamanho).then(a.nome.cmp(&b.nome))),
            Ordem::Tipo => self
                .candidatos
                .sort_by(|a, b| b.e_raw.cmp(&a.e_raw).then(a.nome.cmp(&b.nome))),
        }
    }

    /// Troca a marca de uma célula e fixa a âncora nela.
    pub fn alternar(&mut self, indice: usize) {
        let Some(candidato) = self.candidatos.get_mut(indice) else {
            return;
        };
        candidato.marcado = !candidato.marcado;
        self.ancora = Some(indice);
        self.focado = Some(indice);
    }

    /// Marca de uma ponta a outra — o Shift+clique.
    ///
    /// 🔑 **O intervalo é sobre o que está visível**, e não sobre o acervo: com
    /// "só novos" ligado, arrastar a marcação por cima de uma duplicata escondida
    /// marcaria algo que a pessoa não está vendo.
    ///
    /// Funciona nos dois sentidos, e sem âncora vira um clique comum.
    pub fn marcar_ate(&mut self, indice: usize, marcado: bool) {
        let Some(ancora) = self.ancora else {
            self.alternar(indice);
            return;
        };

        let visiveis = self.visiveis();
        let posicao = |alvo: usize| visiveis.iter().position(|i| *i == alvo);
        let (Some(de), Some(ate)) = (posicao(ancora), posicao(indice)) else {
            return;
        };

        let (inicio, fim) = if de <= ate { (de, ate) } else { (ate, de) };
        for visivel in &visiveis[inicio..=fim] {
            self.candidatos[*visivel].marcado = marcado;
        }
        self.focado = Some(indice);
    }

    /// Marca ou desmarca tudo o que está visível.
    pub fn marcar_todos(&mut self, marcado: bool) {
        for indice in self.visiveis() {
            self.candidatos[indice].marcado = marcado;
        }
    }

    /// Esquece a listagem ao trocar de origem.
    pub fn esquecer_candidatos(&mut self) {
        self.candidatos.clear();
        self.focado = None;
        self.ancora = None;
        self.aviso = None;
    }
}

/// Aplica um recado ao estado e devolve o trabalho que ele deixou pendente.
///
/// 🚨 **A raiz volta no `Varrido` para poder ser recusada.** Varrer um cartão
/// demora; trocar de origem no meio é normal. Sem conferir de quem é a resposta,
/// a varredura antiga chega depois e sobrescreve a lista da origem nova — o
/// sintoma é a grade mostrar os arquivos da pasta anterior, e só quem sabe que
/// houve corrida entende.
pub fn aplicar(estado: &mut Estado, recado: Recado) -> Option<Seguimento> {
    match recado {
        Recado::Varrido { raiz, arquivos } => {
            estado.varrendo = false;
            if estado.origem.as_deref() != Some(raiz.as_str()) {
                return None;
            }

            estado.candidatos = arquivos
                .iter()
                .cloned()
                .map(Candidato::do_caminho)
                .collect();
            estado.ordenar();
            estado.descrevendo = !arquivos.is_empty();
            estado.conferindo_duplicatas = !arquivos.is_empty();

            Some(Seguimento::Detalhar(arquivos))
        }

        Recado::Descritos(descricoes) => {
            estado.descrevendo = false;

            // Casa por caminho, e não por posição: as descrições chegam em
            // paralelo e voltam fora de ordem.
            for descricao in descricoes {
                if let Some(candidato) = estado
                    .candidatos
                    .iter_mut()
                    .find(|c| c.caminho == descricao.caminho)
                {
                    candidato.tamanho = descricao.tamanho;
                    candidato.e_raw = descricao.e_raw;
                    candidato.camera = descricao.camera;
                    candidato.data = descricao.data;
                    candidato.dimensoes = descricao.dimensoes;
                    candidato.descrito = true;
                }
            }

            // ⚠️ Reordenar **aqui**: ordenar por hora de captura antes de as horas
            // chegarem ordena por string vazia, e a grade se reembaralha sozinha
            // quando elas chegam.
            estado.ordenar();
            None
        }

        Recado::Duplicados(caminhos) => {
            estado.conferindo_duplicatas = false;

            let duplicados: std::collections::HashSet<String> = caminhos.into_iter().collect();
            let pular = estado.opcoes.skip_duplicates;

            for candidato in estado.candidatos.iter_mut() {
                if duplicados.contains(&candidato.caminho) {
                    candidato.duplicado = true;
                    // Desmarcar é só quando a opção pede: quem desligou "pular
                    // duplicatas" quer reimportar, e a tela não pode decidir por
                    // ele.
                    if pular {
                        candidato.marcado = false;
                    }
                }
            }
            None
        }

        Recado::Falhou(erro) => {
            estado.varrendo = false;
            estado.descrevendo = false;
            estado.conferindo_duplicatas = false;
            estado.aviso = Some(erro);
            None
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_arquivos(raiz: &str, nomes: &[&str]) -> Estado {
        let mut estado = Estado {
            origem: Some(raiz.to_string()),
            ..Default::default()
        };
        let arquivos: Vec<String> = nomes.iter().map(|n| format!("{raiz}/{n}")).collect();
        aplicar(
            &mut estado,
            Recado::Varrido {
                raiz: raiz.to_string(),
                arquivos,
            },
        );
        estado
    }

    fn nomes(estado: &Estado) -> Vec<&str> {
        estado.candidatos.iter().map(|c| c.nome.as_str()).collect()
    }

    /// 🚨 A varredura da origem antiga **não** sobrescreve a lista da nova.
    ///
    /// Varrer um cartão demora, e trocar de origem no meio é o que qualquer um
    /// faz. Sem a conferência de raiz, a resposta atrasada chega e a grade passa a
    /// mostrar os arquivos da pasta anterior — sem erro, sem aviso, e sem nada que
    /// ligue o sintoma à causa.
    #[test]
    fn varredura_atrasada_de_outra_origem_e_ignorada() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);

        // O usuário troca de origem, e a varredura antiga chega depois.
        estado.origem = Some("/pasta".into());
        let seguimento = aplicar(
            &mut estado,
            Recado::Varrido {
                raiz: "/cartao".into(),
                arquivos: vec!["/cartao/c.NEF".into()],
            },
        );

        assert!(
            seguimento.is_none(),
            "não há o que detalhar de uma origem morta"
        );
        assert_eq!(
            nomes(&estado),
            ["a.NEF", "b.NEF"],
            "a lista não foi trocada"
        );
    }

    /// O que o scan devolve já aparece na grade, marcado e sem metadados.
    #[test]
    fn a_grade_aparece_cheia_antes_dos_metadados() {
        let estado = com_arquivos("/cartao", &["b.NEF", "a.NEF"]);

        assert_eq!(estado.candidatos.len(), 2);
        assert_eq!(estado.marcados(), 2, "nasce tudo marcado");
        assert!(!estado.candidatos[0].descrito);
        assert!(estado.descrevendo && estado.conferindo_duplicatas);
    }

    /// Varredura vazia não deixa trabalho pendente nem promete leitura.
    #[test]
    fn origem_vazia_nao_fica_descrevendo_para_sempre() {
        let mut estado = Estado {
            origem: Some("/vazia".into()),
            ..Default::default()
        };
        aplicar(
            &mut estado,
            Recado::Varrido {
                raiz: "/vazia".into(),
                arquivos: Vec::new(),
            },
        );

        assert!(!estado.descrevendo, "não há metadados para esperar");
        assert!(!estado.conferindo_duplicatas);
    }

    /// 🚨 As descrições casam por **caminho**, e não por posição.
    ///
    /// Elas são lidas em paralelo e voltam fora de ordem. Casar por índice daria
    /// a câmera de uma foto para outra — e como quase toda foto de um cartão tem
    /// a mesma câmera, isso passaria despercebido até alguém importar dois
    /// cartões juntos.
    #[test]
    fn as_descricoes_casam_por_caminho() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);

        aplicar(
            &mut estado,
            Recado::Descritos(vec![Descricao {
                caminho: "/cartao/b.NEF".into(),
                tamanho: 42,
                e_raw: true,
                camera: "Nikon Z6".into(),
                data: "2026:08:16 10:00:00".into(),
                dimensoes: Some("6000x4000".into()),
            }]),
        );

        let a = estado
            .candidatos
            .iter()
            .find(|c| c.nome == "a.NEF")
            .unwrap();
        let b = estado
            .candidatos
            .iter()
            .find(|c| c.nome == "b.NEF")
            .unwrap();
        assert_eq!(b.camera, "Nikon Z6");
        assert_eq!(b.tamanho, 42);
        assert!(b.descrito);
        assert!(!a.descrito, "quem não foi descrito continua sem descrição");
    }

    /// 🚨 A ordenação por captura só acontece **depois** das horas chegarem.
    #[test]
    fn a_grade_reordena_quando_as_horas_chegam() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        assert_eq!(
            nomes(&estado),
            ["a.NEF", "b.NEF"],
            "sem hora, ordena por nome"
        );

        aplicar(
            &mut estado,
            Recado::Descritos(vec![
                Descricao {
                    caminho: "/cartao/a.NEF".into(),
                    tamanho: 1,
                    e_raw: true,
                    camera: String::new(),
                    data: "2026:08:16 12:00:00".into(),
                    dimensoes: None,
                },
                Descricao {
                    caminho: "/cartao/b.NEF".into(),
                    tamanho: 1,
                    e_raw: true,
                    camera: String::new(),
                    data: "2026:08:16 09:00:00".into(),
                    dimensoes: None,
                },
            ]),
        );

        assert_eq!(nomes(&estado), ["b.NEF", "a.NEF"], "a mais antiga primeiro");
    }

    /// 🚨 Rajada: fotos do mesmo segundo não trocam de lugar entre ordenações.
    #[test]
    fn a_rajada_nao_embaralha() {
        let mut estado = com_arquivos("/cartao", &["c.NEF", "a.NEF", "b.NEF"]);
        let mesma_hora = "2026:08:16 10:00:00";

        aplicar(
            &mut estado,
            Recado::Descritos(
                ["a.NEF", "b.NEF", "c.NEF"]
                    .iter()
                    .map(|nome| Descricao {
                        caminho: format!("/cartao/{nome}"),
                        tamanho: 1,
                        e_raw: true,
                        camera: String::new(),
                        data: mesma_hora.into(),
                        dimensoes: None,
                    })
                    .collect(),
            ),
        );

        let primeira = nomes(&estado).join(",");
        estado.ordenar();
        estado.ordenar();
        assert_eq!(nomes(&estado).join(","), primeira);
        assert_eq!(primeira, "a.NEF,b.NEF,c.NEF", "o desempate é o nome");
    }

    /// 🚨 Duplicata só desmarca quando a opção pede.
    ///
    /// Quem desligou "pular duplicatas" quer reimportar — provavelmente porque a
    /// cópia anterior está corrompida. A tela desmarcar por conta própria seria
    /// desfazer a decisão dele em silêncio.
    #[test]
    fn duplicata_so_desmarca_se_a_opcao_pedir() {
        let mut com_pular = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        com_pular.opcoes.skip_duplicates = true;
        aplicar(
            &mut com_pular,
            Recado::Duplicados(vec!["/cartao/a.NEF".into()]),
        );

        assert_eq!(com_pular.duplicados(), 1);
        assert_eq!(com_pular.marcados(), 1, "a duplicata saiu da marcação");

        let mut sem_pular = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        sem_pular.opcoes.skip_duplicates = false;
        aplicar(
            &mut sem_pular,
            Recado::Duplicados(vec!["/cartao/a.NEF".into()]),
        );

        assert_eq!(sem_pular.duplicados(), 1, "continua sendo duplicata");
        assert_eq!(sem_pular.marcados(), 2, "mas continua marcada");
    }

    /// O filtro "só novos" tira as duplicatas da grade sem desmarcá-las.
    #[test]
    fn so_novos_esconde_sem_desmarcar() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        estado.opcoes.skip_duplicates = false;
        aplicar(
            &mut estado,
            Recado::Duplicados(vec!["/cartao/a.NEF".into()]),
        );

        assert_eq!(estado.visiveis().len(), 2);
        estado.so_novos = true;
        assert_eq!(estado.visiveis(), vec![1], "só a b.NEF fica visível");
        assert_eq!(estado.marcados(), 2, "esconder não é desmarcar");
    }

    /// 🚨 Shift+clique marca o intervalo **do que está visível**.
    ///
    /// Com "só novos" ligado, contar o intervalo sobre o acervo marcaria
    /// duplicatas escondidas — arquivos que a pessoa não está vendo entrariam na
    /// importação por causa de um gesto que parecia só pegar três células.
    #[test]
    fn o_intervalo_do_shift_respeita_o_filtro() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF", "c.NEF", "d.NEF"]);
        estado.opcoes.skip_duplicates = false;
        aplicar(
            &mut estado,
            Recado::Duplicados(vec!["/cartao/b.NEF".into()]),
        );
        estado.so_novos = true;
        // Desmarca **tudo**, inclusive o escondido: `marcar_todos` de propósito
        // não toca no que está fora da grade (tem teste próprio), e deixar a
        // duplicata marcada aqui esconderia o que este teste quer medir.
        for candidato in estado.candidatos.iter_mut() {
            candidato.marcado = false;
        }

        // Visíveis: a (0), c (2), d (3). Marca de a até c.
        estado.ancora = Some(0);
        estado.marcar_ate(2, true);

        assert!(estado.candidatos[0].marcado, "a.NEF entrou");
        assert!(
            !estado.candidatos[1].marcado,
            "b.NEF está escondida e não entra"
        );
        assert!(estado.candidatos[2].marcado, "c.NEF entrou");
        assert!(
            !estado.candidatos[3].marcado,
            "d.NEF ficou fora do intervalo"
        );
    }

    /// O intervalo funciona nos dois sentidos.
    #[test]
    fn o_intervalo_vale_de_baixo_para_cima() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF", "c.NEF"]);
        estado.marcar_todos(false);

        estado.ancora = Some(2);
        estado.marcar_ate(0, true);

        assert_eq!(estado.marcados(), 3);
    }

    /// Sem âncora, o Shift+clique vira um clique comum.
    #[test]
    fn shift_sem_ancora_e_um_clique() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        estado.ancora = None;

        estado.marcar_ate(1, true);

        assert!(
            !estado.candidatos[1].marcado,
            "alternou a partir de marcado"
        );
        assert_eq!(estado.ancora, Some(1), "e virou a âncora do próximo");
    }

    /// ⌘A marca só o que está visível.
    #[test]
    fn marcar_todos_respeita_o_filtro() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        estado.opcoes.skip_duplicates = true;
        aplicar(
            &mut estado,
            Recado::Duplicados(vec!["/cartao/a.NEF".into()]),
        );
        estado.so_novos = true;

        estado.marcar_todos(true);

        assert!(
            !estado.candidatos[0].marcado,
            "a duplicata escondida fica fora"
        );
        assert!(estado.candidatos[1].marcado);
    }

    /// Uma falha limpa os três "em andamento" — senão a tela fica girando para
    /// sempre.
    #[test]
    fn falha_para_todos_os_indicadores() {
        let mut estado = com_arquivos("/cartao", &["a.NEF"]);
        estado.varrendo = true;

        aplicar(&mut estado, Recado::Falhou("cartão removido".into()));

        assert!(!estado.varrendo && !estado.descrevendo && !estado.conferindo_duplicatas);
        assert_eq!(estado.aviso.as_deref(), Some("cartão removido"));
    }

    /// Trocar de origem esquece foco e âncora junto com a lista.
    ///
    /// 🔑 Um índice guardado de outra listagem aponta para outro arquivo — e o
    /// Shift+clique seguinte marcaria um intervalo que começa em lugar nenhum.
    #[test]
    fn esquecer_candidatos_leva_foco_e_ancora() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        estado.focado = Some(1);
        estado.ancora = Some(1);
        estado.aviso = Some("algo".into());

        estado.esquecer_candidatos();

        assert!(estado.candidatos.is_empty());
        assert_eq!(estado.focado, None);
        assert_eq!(estado.ancora, None);
        assert_eq!(estado.aviso, None);
    }

    /// A conta que o rodapé mostra: quantos e quantos bytes.
    #[test]
    fn o_rodape_conta_o_que_esta_marcado() {
        let mut estado = com_arquivos("/cartao", &["a.NEF", "b.NEF"]);
        aplicar(
            &mut estado,
            Recado::Descritos(vec![
                Descricao {
                    caminho: "/cartao/a.NEF".into(),
                    tamanho: 1000,
                    e_raw: true,
                    camera: String::new(),
                    data: String::new(),
                    dimensoes: None,
                },
                Descricao {
                    caminho: "/cartao/b.NEF".into(),
                    tamanho: 500,
                    e_raw: true,
                    camera: String::new(),
                    data: String::new(),
                    dimensoes: None,
                },
            ]),
        );

        assert_eq!(estado.bytes_marcados(), 1500);
        estado.alternar(0);
        assert_eq!(estado.marcados(), 1);
        assert_eq!(estado.bytes_marcados(), 500);
        assert_eq!(estado.caminhos_marcados(), vec!["/cartao/b.NEF"]);
    }
}
