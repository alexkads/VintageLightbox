//! 📤 **A esteira de envios** — o que na web é o Worker, aqui é este módulo.
//!
//! # Por que existe (dono, 18/set/2026)
//!
//! > *"Essas atividades de 'Sincronizar' e 'Salvar na galeria e sair' precisam
//! > ser realmente em segundo plano, da mesma forma que ocorre no site, pois eu
//! > sei que ele usa Service Worker. Precisa criar algum módulo no Rust que
//! > faça esse mesmo trabalho paralelo em fila!"*
//!
//! No site, quem sobe as fotos é um Worker: a tela entrega a lista e continua
//! respondendo ao operador, e o Worker leva **três por vez**
//! (`editor.tsx`, `EM_VOO`). Aqui era o contrário — cada gesto disparava uma
//! tarefa por foto na mesma passada, e dois defeitos saíram disso no mesmo dia:
//!
//! - **a memória** (*"a aplicação estoura a memória na hora de sair e salvar em
//!   segundo plano"*): cada foto no ar é um original decodificado, 96 MB em
//!   RAM. Duzentas de uma vez levam a máquina do balcão junto — e o que chega ao
//!   servidor quando a alocação falha não é imagem, daí o `400: formato de
//!   imagem não suportado` que uma sessão de 200 fotos devolveu;
//! - **a tela** (*"o usuário não consegue operar a biblioteca durante a
//!   atualização das fotos"*): sem fila, o lote inteiro competia com a interface
//!   pela GPU e pelo disco.
//!
//! # O que este módulo é, e o que não é
//!
//! É **uma fila com teto**, e nada mais: quem executa continua sendo a porta do
//! pós-venda (`Publicador`), que já roda no tokio. A esteira decide *quando*
//! cada trabalho sai — e é isso que a torna testável sem rede, sem GPU e sem
//! janela.
//!
//! ⚠️ **Ela não guarda nada em disco.** O que protege o trabalho contra um
//! fechamento no meio é o depósito do gravador (a receita) e a nota no catálogo
//! — a fila é só a ordem em que as coisas saem nesta sessão do app. É a mesma
//! divisão da web: o Worker não é a memória, o IndexedDB é.

use domain::services::pos_venda::Sessao;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::pos_venda::porta::{FotoClassificada, Publicador, Recado};
use crate::revelacao::processador::Ajustes;
use domain::value_objects::CropSettings;

/// Quantos trabalhos a esteira mantém **no ar** ao mesmo tempo.
///
/// 🔑 **Três, e o número é o do site** (`editor.tsx`, `EM_VOO`). Cada foto é
/// baixar (rede), decodificar e revelar (96 MB e a GPU, serializados pela fila
/// do motor) e subir (rede). Com uma só, a GPU fica parada durante os segundos
/// de rede de cada foto; com várias, o download de uma corre enquanto outra
/// revela e uma terceira sobe.
///
/// ⚠️ **O teto é memória, não banda.** As que esperam a vez seguram o original
/// comprimido — uns 30 MB cada. Três em voo é ~90 MB parados mais os 96 MB da
/// que está sendo decodificada; subir isso para dez leva a máquina do balcão
/// junto, que é onde este botão roda.
pub const EM_VOO: usize = 3;

/// Um trabalho da esteira.
///
/// 🔑 **Os dois caminhos que sobem foto**, e só eles: classificar (o passo 3) e
/// salvar a revelação (o botão do editor). O que não sobe arquivo — negociar,
/// mudar faixa, tirar do site — continua indo direto: é um `PATCH` de alguns
/// bytes, sem imagem e sem memória.
#[derive(Debug, Clone)]
pub enum Trabalho {
    /// O passo 3: a foto classificada sobe para a galeria.
    Classificada {
        galeria: String,
        foto: Box<FotoClassificada>,
    },
    /// O "Salvar na galeria e sair": a revelação entra no lugar do original.
    ///
    /// ⚠️ `Box` nos dois lados: `Ajustes` são 171 `f32`, e uma variante gorda
    /// faria toda a fila carregar o tamanho da maior.
    Revelacao {
        foto_no_site: String,
        ajustes: Box<Ajustes>,
        corte: CropSettings,
    },
}

impl Trabalho {
    /// O que este trabalho mexe — é por aqui que a esteira não repete a mesma
    /// foto duas vezes na fila.
    fn alvo(&self) -> &str {
        match self {
            Self::Classificada { foto, .. } => &foto.foto_id,
            Self::Revelacao { foto_no_site, .. } => foto_no_site,
        }
    }
}

/// O que aconteceu com um trabalho empurrado — quem conta respostas só soma
/// o que é [`Entrada::Nova`]: é ela que terá uma resposta a mais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entrada {
    /// Entrou na fila.
    Nova,
    /// A mesma foto esperava na fila, e recebeu a receita nova.
    Substituida,
    /// A mesma foto já esperava, e o pedido repetido não muda nada.
    JaNaFila,
    /// A mesma foto já saiu; a resposta dela vem de qualquer jeito.
    JaNoAr,
}

/// Quanto da esteira já andou — o que a bandeja e o canto dos envios mostram.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progresso {
    /// Quantos trabalhos entraram desde que a esteira esvaziou pela última vez.
    pub total: usize,
    /// Quantos já responderam (com sucesso ou falha).
    pub respondidos: usize,
    /// Quantos estão no ar agora.
    pub no_ar: usize,
    /// Alguma resposta veio como falha? O aviso do fim depende disso.
    pub houve_falha: bool,
}

impl Progresso {
    pub fn andando(&self) -> bool {
        self.respondidos < self.total
    }

    /// Quantos ainda esperam vaga.
    pub fn na_fila(&self) -> usize {
        self.total
            .saturating_sub(self.respondidos)
            .saturating_sub(self.no_ar)
    }
}

/// Quantas vezes a esteira tenta a **mesma** foto antes de desistir dela.
///
/// 🚨 **Uma rede ruim não pode custar o trabalho do operador** (dono,
/// 18/set/2026: *"essa rotina precisa ser um tanque de guerra!"*). Até aqui,
/// uma falha de rede gastava a foto: a resposta abria vaga, o lote seguia sem
/// ela, e quem estava com o cliente só descobria no canto dos envios — se
/// olhasse. Três é o número do balcão: erro de momento (um 502, o Wi-Fi que
/// oscilou) passa na segunda; o que falha três vezes tem causa, e insistir só
/// atrasa as outras.
pub const TENTATIVAS: u8 = 3;

/// A espera antes de repetir, multiplicada pela tentativa.
///
/// 🔑 **Repetir na hora é repetir o erro.** Quando a rede cai, as três em voo
/// falham juntas; sem recuo, as três voltam na mesma passada, falham de novo e
/// gastam as tentativas em menos de um segundo — a rede nem teve tempo de
/// voltar. Com o recuo, as três tentativas de uma foto cobrem uns dois
/// segundos.
pub const RECUO: Duration = Duration::from_millis(400);

/// Um trabalho e quantas vezes ele já foi tentado.
#[derive(Debug, Clone)]
struct NaEsteira {
    trabalho: Trabalho,
    tentativas: u8,
}

/// O que aconteceu com a resposta que chegou.
#[derive(Debug, Clone, PartialEq)]
pub enum Desfecho {
    /// A resposta não é de nenhum trabalho desta esteira — outro gesto (uma
    /// negociação, um "tirar do site") ou o eco de um lote que já acabou.
    ///
    /// 🔑 **Existe para a esteira não contar o que não é dela.** Antes, toda
    /// resposta que passava pelo canal abria uma vaga aqui, inclusive as de
    /// gestos que nunca entraram na fila.
    NaoEraMeu,
    /// Subiu.
    Feito,
    /// Falhou, e volta para o fim da fila.
    VaiRepetir {
        alvo: String,
        daqui_a: Duration,
        tentativa: u8,
    },
    /// Falhou [`TENTATIVAS`] vezes: a esteira larga esta foto.
    ///
    /// 🚨 **E quem pediu tem de dizer isso na tela**, com o nome do arquivo:
    /// nada pode sumir em silêncio. A receita continua no depósito e a foto
    /// continua na conta do "falta subir", então repetir o gesto a manda de
    /// novo — o que não pode é o operador não saber.
    Desistiu { tentativas: u8 },
}

/// A fila com teto.
#[derive(Debug, Default)]
pub struct Esteira {
    fila: VecDeque<NaEsteira>,
    /// O que está no site agora, **por alvo** — é o que permite saber qual
    /// foto falhou, e portanto qual repetir.
    no_ar: HashMap<String, NaEsteira>,
    progresso: Progresso,
}

impl Esteira {
    /// Põe um trabalho na fila.
    ///
    /// 🚨 **A mesma foto não entra duas vezes na fila**: classificar em lote e
    /// repetir o gesto antes de a primeira resposta voltar mandaria a mesma foto
    /// ao site duas vezes — e a segunda desfaria a primeira depois de dois
    /// downloads e dois JPEGs. É a mesma regra de `enfileirar_para_subir`.
    ///
    /// ⚠️ **O que já está no ar também não entra.** Ele já saiu e a resposta
    /// dele vem de qualquer jeito; aceitar um segundo pedido da mesma foto
    /// faria duas versões dela disputarem qual chega por último — e, agora que
    /// a esteira repete, a repetição da primeira brigaria com a segunda.
    ///
    /// 🔄 **A revelação que espera recebe a receita nova** (2026-09-21). Um
    /// segundo "Salvar na galeria" com a mesma foto ainda na fila trocava nada:
    /// a receita velha subia. É o que o Worker do site faz — a mesma foto é
    /// substituída. A que já está no ar segue como está: a raiz confere, na
    /// resposta, se a receita mudou no caminho (`receita_mudou_no_envio`).
    pub fn empurrar(&mut self, trabalho: Trabalho) -> Entrada {
        let alvo = trabalho.alvo().to_string();
        if self.no_ar.contains_key(&alvo) {
            return Entrada::JaNoAr;
        }
        if let Some(na_fila) = self.fila.iter_mut().find(|t| t.trabalho.alvo() == alvo) {
            if matches!(trabalho, Trabalho::Revelacao { .. }) {
                na_fila.trabalho = trabalho;
                return Entrada::Substituida;
            }
            return Entrada::JaNaFila;
        }
        self.fila.push_back(NaEsteira {
            trabalho,
            tentativas: 0,
        });
        self.progresso.total += 1;
        Entrada::Nova
    }

    /// Manda para a porta os próximos, até [`EM_VOO`] no ar. Devolve quantos
    /// saíram agora.
    pub fn despachar(
        &mut self,
        publicador: &dyn Publicador,
        sessao: &Sessao,
        canal: &Sender<Recado>,
    ) -> usize {
        let mut saíram = 0;
        while self.no_ar.len() < EM_VOO {
            let Some(item) = self.fila.pop_front() else {
                break;
            };
            match &item.trabalho {
                Trabalho::Classificada { galeria, foto } => {
                    publicador.subir_classificada(
                        sessao.clone(),
                        galeria.clone(),
                        (**foto).clone(),
                        canal.clone(),
                    );
                }
                Trabalho::Revelacao {
                    foto_no_site,
                    ajustes,
                    corte,
                } => {
                    publicador.salvar_revelacao(
                        sessao.clone(),
                        foto_no_site.clone(),
                        **ajustes,
                        corte.clone(),
                        canal.clone(),
                    );
                }
            }
            self.no_ar.insert(item.trabalho.alvo().to_string(), item);
            self.progresso.no_ar = self.no_ar.len();
            saíram += 1;
        }
        saíram
    }

    /// **A resposta de uma foto chegou** — e é por ela que a esteira anda.
    ///
    /// 🔑 **Quem reabastece é a resposta**, e não o relógio: a esteira não sabe
    /// quanto uma foto demora, e um temporizador acabaria mandando mais fotos
    /// enquanto as primeiras ainda decodificam — que é exatamente o que ela
    /// existe para impedir.
    ///
    /// ⚠️ **A resposta tem nome.** Sem ele não há como repetir a que falhou nem
    /// como saber se a resposta é sequer desta esteira, e era assim até
    /// 18/set/2026 (`Recado::Falhou` levava só a frase).
    pub fn respondeu(&mut self, alvo: &str, falhou: bool) -> Desfecho {
        let Some(mut item) = self.no_ar.remove(alvo) else {
            return Desfecho::NaoEraMeu;
        };
        self.progresso.no_ar = self.no_ar.len();
        if !falhou {
            self.progresso.respondidos += 1;
            self.talvez_zerar();
            return Desfecho::Feito;
        }
        item.tentativas += 1;
        if item.tentativas < TENTATIVAS {
            let desfecho = Desfecho::VaiRepetir {
                alvo: alvo.to_string(),
                daqui_a: RECUO * u32::from(item.tentativas),
                tentativa: item.tentativas,
            };
            // 🔑 **No fim da fila, e não na frente**: a foto que falhou espera a
            // vez das que ainda não tentaram. Furar a fila com ela seria deixar
            // uma foto problemática segurando as outras cento e noventa.
            self.fila.push_back(item);
            return desfecho;
        }
        self.progresso.respondidos += 1;
        self.progresso.houve_falha = true;
        self.talvez_zerar();
        // A foto que não subiu depois de todas as tentativas é o problema
        // operacional por excelência: o cliente não vai vê-la na galeria.
        crate::telemetria::erro(
            "envio",
            &format!("{alvo}: não subiu depois de {} tentativas", item.tentativas),
        );
        Desfecho::Desistiu {
            tentativas: item.tentativas,
        }
    }

    /// Esteira vazia: a contagem recomeça no próximo lote.
    fn talvez_zerar(&mut self) {
        if !self.andando() && self.progresso.respondidos >= self.progresso.total {
            self.progresso = Progresso::default();
        }
    }

    pub fn progresso(&self) -> Progresso {
        self.progresso
    }

    /// Há o que fazer? (Na fila ou no ar.)
    pub fn andando(&self) -> bool {
        !self.fila.is_empty() || !self.no_ar.is_empty()
    }

    /// Tira **este** trabalho da fila, se ele ainda não saiu. Devolve se saiu
    /// da fila.
    ///
    /// 🚨 **É o `X` chegando antes da vez da foto** (contrato C21): o ensaio
    /// inteiro entra na esteira assim que é importado, e rejeitar uma foto que
    /// ainda espera vaga tem de impedir a subida — não adiantaria marcar a foto
    /// e vê-la subir três segundos depois.
    ///
    /// ⚠️ **O que já está no ar não volta.** Ele saiu, e a resposta dele vem de
    /// qualquer jeito; quem cuida dessa foto é a conciliação de quem a mandou,
    /// que marca a rejeição no site depois que ela chega lá.
    pub fn tirar_da_fila(&mut self, alvo: &str) -> bool {
        let antes = self.fila.len();
        self.fila.retain(|t| t.trabalho.alvo() != alvo);
        let tirou = self.fila.len() < antes;
        if tirou {
            self.progresso.total -= antes - self.fila.len();
        }
        tirou
    }

    /// Esvazia o que ainda não saiu — o que está no ar continua, porque já saiu.
    pub fn esquecer_o_que_espera(&mut self) {
        self.progresso.total -= self.fila.len();
        self.fila.clear();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use std::sync::mpsc::channel;
    use std::sync::Arc;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    fn classificada(id: &str) -> Trabalho {
        Trabalho::Classificada {
            galeria: "g1".into(),
            foto: Box::new(FotoClassificada {
                foto_id: id.into(),
                ordem: 0,
                estado: None,
                nota: Some(5),
                produto_id: None,
            }),
        }
    }

    /// 🚨 **Três no ar, nunca mais** — o teto é a razão de a esteira existir.
    #[test]
    fn a_esteira_segura_o_lote_em_tres() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..10 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }

        let saíram = esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        assert_eq!(saíram, EM_VOO);
        assert_eq!(publicador.subidas().len(), EM_VOO);
        assert_eq!(esteira.progresso().no_ar, EM_VOO);
        assert_eq!(esteira.progresso().na_fila(), 7);

        // Sem resposta, um segundo despacho não manda nada: as vagas estão
        // ocupadas.
        assert_eq!(esteira.despachar(publicador.as_ref(), &sessao(), &canal), 0);
        assert_eq!(publicador.subidas().len(), EM_VOO);
    }

    /// Cada resposta abre uma vaga — e o lote inteiro sai, em ondas de três.
    #[test]
    fn cada_resposta_abre_uma_vaga() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..10 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        for i in 0..10 {
            assert_eq!(
                esteira.respondeu(&format!("f{i}"), false),
                Desfecho::Feito,
                "a resposta da f{i} é dela"
            );
            esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        }

        assert_eq!(publicador.subidas().len(), 10, "todas subiram");
        assert!(!esteira.andando(), "e a esteira esvaziou");
        assert_eq!(
            esteira.progresso(),
            Progresso::default(),
            "a contagem recomeça no próximo lote"
        );
    }

    /// 🚨 A mesma foto não entra duas vezes na fila.
    #[test]
    fn a_mesma_foto_nao_entra_duas_vezes() {
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("b"));

        assert_eq!(esteira.progresso().total, 2);
    }

    /// 🔄 A revelação que espera na fila recebe a receita do segundo "Salvar",
    /// sem contar outra resposta; a que já está no ar não é tocada.
    #[test]
    fn a_revelacao_que_espera_recebe_a_receita_nova() {
        let revelacao = |exposicao: f32| Trabalho::Revelacao {
            foto_no_site: "a".into(),
            ajustes: Box::new(Ajustes {
                exposure: exposicao,
                ..Ajustes::default()
            }),
            corte: CropSettings::default(),
        };
        let mut esteira = Esteira::default();
        assert_eq!(esteira.empurrar(revelacao(1.0)), Entrada::Nova);
        assert_eq!(esteira.empurrar(revelacao(2.0)), Entrada::Substituida);
        assert_eq!(esteira.progresso().total, 1, "uma foto, uma resposta");
        let Some(Trabalho::Revelacao { ajustes, .. }) = esteira.fila.front().map(|t| &t.trabalho)
        else {
            panic!("a revelação está na fila");
        };
        assert_eq!(ajustes.exposure, 2.0, "vale a receita do último clique");

        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        assert_eq!(esteira.empurrar(revelacao(3.0)), Entrada::JaNoAr);
    }

    /// 🚨 **A foto que falha volta para a fila** — e só depois de
    /// [`TENTATIVAS`] a esteira desiste dela.
    ///
    /// Dono, 18/set/2026: *"essa rotina precisa ser um tanque de guerra!"*. Uma
    /// falha de rede não pode custar a foto: até aqui a resposta abria vaga, o
    /// lote seguia sem ela, e quem estava com o cliente só descobria depois.
    #[test]
    fn a_foto_que_falha_vai_de_novo_ate_a_terceira() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        // Primeira falha: volta para a fila, com recuo, e o total não muda —
        // ela continua sendo **uma** foto por subir.
        let desfecho = esteira.respondeu("a", true);
        assert!(
            matches!(desfecho, Desfecho::VaiRepetir { tentativa: 1, daqui_a, .. } if daqui_a == RECUO)
        );
        assert_eq!(esteira.progresso().total, 1);
        assert_eq!(esteira.progresso().respondidos, 0, "ainda não respondeu");
        assert!(esteira.andando());

        // Segunda: de novo, com o recuo dobrado.
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        assert_eq!(publicador.subidas().len(), 2, "a segunda tentativa saiu");
        assert!(matches!(
            esteira.respondeu("a", true),
            Desfecho::VaiRepetir {
                tentativa: 2,
                daqui_a,
                ..
            } if daqui_a == RECUO * 2
        ));

        // Terceira e última: a esteira desiste, e diz isso a quem pediu.
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        assert_eq!(publicador.subidas().len(), 3);
        assert_eq!(
            esteira.respondeu("a", true),
            Desfecho::Desistiu {
                tentativas: TENTATIVAS
            }
        );
        assert!(!esteira.andando(), "a esteira largou a foto");
    }

    /// A foto que falha e depois passa não conta falha nenhuma no fim.
    #[test]
    fn a_falha_que_passa_na_segunda_nao_vira_aviso() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.empurrar(classificada("b"));
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        esteira.respondeu("a", true);
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        assert_eq!(esteira.respondeu("a", false), Desfecho::Feito);
        assert_eq!(esteira.respondeu("b", false), Desfecho::Feito);

        assert!(!esteira.andando());
        assert_eq!(
            esteira.progresso(),
            Progresso::default(),
            "sem falha pendente: a contagem recomeça limpa"
        );
    }

    /// ⚠️ **A resposta que não é desta esteira não mexe na conta dela.**
    ///
    /// Pelo mesmo canal chegam as respostas de gestos que nunca entraram na
    /// fila — uma negociação, um "tirar do site" — e o eco de um lote que já
    /// acabou. Antes de a resposta ter nome, toda uma delas abria uma vaga
    /// aqui.
    #[test]
    fn resposta_que_ninguem_pediu_nao_estraga_a_conta() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();

        assert_eq!(esteira.respondeu("fantasma", false), Desfecho::NaoEraMeu);
        assert_eq!(esteira.respondeu("fantasma", true), Desfecho::NaoEraMeu);
        assert_eq!(esteira.progresso(), Progresso::default());

        esteira.empurrar(classificada("a"));
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);
        assert_eq!(esteira.respondeu("outra", true), Desfecho::NaoEraMeu);
        assert_eq!(esteira.progresso().no_ar, 1, "a de verdade continua no ar");
        assert!(!esteira.progresso().houve_falha);
    }

    /// 🚨 **A foto que está no ar não entra de novo na fila.**
    ///
    /// Com a repetição, aceitar um segundo pedido da mesma foto faria a
    /// repetição da primeira brigar com a segunda — duas versões da mesma foto
    /// subindo, e o site ficando com a que chegasse por último.
    #[test]
    fn o_que_esta_no_ar_nao_entra_de_novo() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        esteira.empurrar(classificada("a"));
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        esteira.empurrar(classificada("a"));

        assert_eq!(esteira.progresso().total, 1);
        assert_eq!(esteira.progresso().na_fila(), 0);
    }

    /// ❌ **A rejeitada sai da fila antes de subir** — contrato C21.
    ///
    /// O ensaio inteiro entra na esteira assim que é importado (C20), e o `X`
    /// chega depois: a foto que ainda espera vaga tem de ser tirada dali. A que
    /// já está no ar não volta — ela saiu, e quem cuida dela é a conciliação
    /// de quem a mandou.
    #[test]
    fn rejeitar_tira_da_fila_o_que_ainda_nao_saiu() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..5 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        assert!(esteira.tirar_da_fila("f4"), "f4 ainda esperava vaga");
        assert_eq!(esteira.progresso().total, 4);
        assert_eq!(esteira.progresso().na_fila(), 1, "sobrou a f3");

        assert!(
            !esteira.tirar_da_fila("f0"),
            "a que já está no ar não sai da fila — ela não está nela"
        );
        assert_eq!(esteira.progresso().no_ar, EM_VOO);
        assert!(
            !esteira.tirar_da_fila("fantasma"),
            "e o que nunca entrou não mexe na conta"
        );
        assert_eq!(esteira.progresso().total, 4);
    }

    /// O que ainda não saiu pode ser esquecido; o que está no ar, não.
    #[test]
    fn esquecer_o_que_espera_nao_cancela_o_que_ja_saiu() {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let (canal, _recebe) = channel();
        let mut esteira = Esteira::default();
        for i in 0..8 {
            esteira.empurrar(classificada(&format!("f{i}")));
        }
        esteira.despachar(publicador.as_ref(), &sessao(), &canal);

        esteira.esquecer_o_que_espera();

        assert_eq!(esteira.progresso().no_ar, EM_VOO, "as três continuam");
        assert_eq!(esteira.progresso().total, EM_VOO);
        assert_eq!(esteira.progresso().na_fila(), 0);
    }
}
